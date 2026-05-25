
pub struct FlashBackedSpool<const CAPACITY: usize, const PAYLOAD_SIZE: usize> {
    spool: PersistentSpool<CAPACITY, PAYLOAD_SIZE>,
    write_offset: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FlashRecoverReport {
    pub recovered_record_count: usize,
    pub corrupt_record_count: usize,
}

impl<const CAPACITY: usize, const PAYLOAD_SIZE: usize> FlashBackedSpool<CAPACITY, PAYLOAD_SIZE> {
    pub const fn new() -> Self {
        Self {
            spool: PersistentSpool::new(),
            write_offset: 0,
        }
    }

    pub fn recover(flash: &impl FlashStorage) -> Result<Self, SpoolError> {
        Ok(Self::recover_with_report(flash)?.0)
    }

    pub fn recover_with_report(
        flash: &impl FlashStorage,
    ) -> Result<(Self, FlashRecoverReport), SpoolError> {
        if encoded_record_len(PAYLOAD_SIZE)? > FLASH_ENTRY_BUFFER_LEN {
            return Err(SpoolError::BufferTooSmall);
        }

        let mut records = [const { None }; CAPACITY];
        let mut max_sequence_seen = None;
        let mut cursor = 0;
        let mut header = [0_u8; ENCODED_RECORD_HEADER_LEN];
        let mut entry = [0_u8; FLASH_ENTRY_BUFFER_LEN];
        let mut report = FlashRecoverReport::default();

        while cursor + 4 <= flash.len() {
            flash.read(cursor, &mut header[..FLASH_WRITE_ALIGNMENT])?;
            if header[..FLASH_WRITE_ALIGNMENT]
                .iter()
                .all(|byte| *byte == 0xff)
            {
                break;
            }
            if cursor + ENCODED_RECORD_HEADER_LEN > flash.len() {
                report.corrupt_record_count += 1;
                break;
            }

            flash.read(cursor, &mut header)?;
            match encoded_log_entry_len(&header) {
                Ok(len) if cursor + len <= flash.len() && len <= entry.len() => {
                    flash.read(cursor, &mut entry[..len])?;

                    let magic = u32::from_le_bytes([entry[0], entry[1], entry[2], entry[3]]);
                    match magic {
                        RECORD_MAGIC => match decode_record(&entry[..len]) {
                            Ok(record) => {
                                max_sequence_seen =
                                    Some(max_sequence(max_sequence_seen, record.sequence));
                                push_stored_record(
                                    &mut records,
                                    StoredRecord::new(
                                        record.sequence,
                                        record.flags,
                                        record.payload,
                                    )?,
                                );
                            }
                            Err(_) => {
                                report.corrupt_record_count += 1;
                                break;
                            }
                        },
                        ACK_MAGIC => match decode_ack(&entry[..len]) {
                            Ok(ack) => {
                                max_sequence_seen =
                                    Some(max_sequence(max_sequence_seen, ack.sequence));
                                remove_stored_record_by_sequence(&mut records, ack.sequence);
                            }
                            Err(_) => {
                                report.corrupt_record_count += 1;
                                break;
                            }
                        },
                        _ => {
                            report.corrupt_record_count += 1;
                            break;
                        }
                    }
                    cursor += len;
                }
                Ok(_) => {
                    report.corrupt_record_count += 1;
                    break;
                }
                Err(SpoolError::BadMagic) => {
                    report.corrupt_record_count += 1;
                    cursor += FLASH_WRITE_ALIGNMENT;
                }
                Err(_) => {
                    report.corrupt_record_count += 1;
                    break;
                }
            }
        }

        let mut compact = [StoredRecord::<PAYLOAD_SIZE>::new(0, 0, &[])?; CAPACITY];
        let mut stored_len = 0;
        for record in records.into_iter().flatten() {
            compact[stored_len] = record;
            stored_len += 1;
        }
        report.recovered_record_count = stored_len;

        Ok((
            Self {
                spool: PersistentSpool::recover_from_records_with_next_sequence(
                    &compact[..stored_len],
                    max_sequence_seen.map_or(0, |sequence| sequence.wrapping_add(1)),
                ),
                write_offset: cursor,
            },
            report,
        ))
    }

    pub fn len(&self) -> usize {
        self.spool.len()
    }

    pub fn is_empty(&self) -> bool {
        self.spool.is_empty()
    }

    pub fn next_sequence(&self) -> u64 {
        self.spool.next_sequence()
    }

    pub fn peek(&self) -> Option<SpoolRecord<'_>> {
        self.spool.peek()
    }

    pub fn append(
        &mut self,
        flash: &mut impl FlashStorage,
        flags: u8,
        payload: &[u8],
    ) -> Result<AppendResult<PAYLOAD_SIZE>, SpoolError> {
        let sequence = self.spool.next_sequence();
        let stored = StoredRecord::new(sequence, flags, payload)?;
        let record = stored.as_record();
        let mut encoded = [0_u8; FLASH_ENTRY_BUFFER_LEN];
        let len = encode_record(record, &mut encoded)?;

        if self.write_offset + len <= flash.len() {
            flash.write(self.write_offset, &encoded[..len])?;
            self.write_offset += len;
            return self.spool.append(flags, payload);
        }

        self.append_with_compaction(flash, stored)
    }

    pub fn acknowledge(
        &mut self,
        flash: &mut impl FlashStorage,
    ) -> Result<Option<StoredRecord<PAYLOAD_SIZE>>, SpoolError> {
        let Some(record) = self.spool.peek() else {
            return Ok(None);
        };

        let mut encoded = [0_u8; ENCODED_ACK_RECORD_LEN];
        let len = encode_ack(record.sequence, &mut encoded)?;

        if self.write_offset + len <= flash.len() {
            flash.write(self.write_offset, &encoded[..len])?;
            self.write_offset += len;
            return Ok(self.spool.acknowledge());
        }

        let acknowledged = self.spool.acknowledge();
        self.rewrite_pending_records(flash)?;
        Ok(acknowledged)
    }

    fn append_with_compaction(
        &mut self,
        flash: &mut impl FlashStorage,
        incoming: StoredRecord<PAYLOAD_SIZE>,
    ) -> Result<AppendResult<PAYLOAD_SIZE>, SpoolError> {
        if encoded_record_len(incoming.payload_len)? > flash.len() {
            return Err(SpoolError::FlashFull);
        }

        let mut compacted = DropOldestQueue::<StoredRecord<PAYLOAD_SIZE>, CAPACITY>::new();
        let mut dropped = None;
        let mut dropped_count = 0;

        for index in 0..self.spool.len() {
            if let Some(record) = self.spool.queue.get(index) {
                compacted.push(*record);
            }
        }
        if let Some(removed) = compacted.push(incoming) {
            dropped = Some(removed);
            dropped_count += 1;
        }

        while queue_encoded_len(&compacted)? > flash.len() {
            let removed = compacted.pop();
            if let Some(removed) = removed {
                dropped = dropped.or(Some(removed));
                dropped_count += 1;
            }
            if compacted.is_empty() {
                return Err(SpoolError::FlashFull);
            }
        }

        let mut records = [StoredRecord::<PAYLOAD_SIZE>::new(0, 0, &[])?; CAPACITY];
        let len = copy_queue_records(&compacted, &mut records);
        self.rewrite_records(flash, &records[..len], incoming.sequence.wrapping_add(1))?;

        Ok(AppendResult {
            sequence: incoming.sequence,
            dropped,
            dropped_count,
        })
    }

    fn rewrite_pending_records(&mut self, flash: &mut impl FlashStorage) -> Result<(), SpoolError> {
        let mut records = [StoredRecord::<PAYLOAD_SIZE>::new(0, 0, &[])?; CAPACITY];
        let len = self.copy_spool_records(&mut records);
        self.rewrite_records(flash, &records[..len], self.spool.next_sequence())
    }

    fn rewrite_records(
        &mut self,
        flash: &mut impl FlashStorage,
        records: &[StoredRecord<PAYLOAD_SIZE>],
        next_sequence: u64,
    ) -> Result<(), SpoolError> {
        if records_encoded_len(records)? > flash.len() {
            return Err(SpoolError::FlashFull);
        }
        flash.erase(0, flash.len())?;
        self.write_offset = 0;
        for record in records {
            let mut encoded = [0_u8; FLASH_ENTRY_BUFFER_LEN];
            let len = encode_record(record.as_record(), &mut encoded)?;
            flash.write(self.write_offset, &encoded[..len])?;
            self.write_offset += len;
        }
        self.spool =
            PersistentSpool::recover_from_records_with_next_sequence(records, next_sequence);

        Ok(())
    }

    fn copy_spool_records(&self, out: &mut [StoredRecord<PAYLOAD_SIZE>]) -> usize {
        let mut len = 0;
        for index in 0..self.spool.len() {
            if let Some(record) = self.spool.queue.get(index)
                && len < out.len()
            {
                out[len] = *record;
                len += 1;
            }
        }

        len
    }
}

impl<const CAPACITY: usize, const PAYLOAD_SIZE: usize> Default
    for FlashBackedSpool<CAPACITY, PAYLOAD_SIZE>
{
    fn default() -> Self {
        Self::new()
    }
}

fn push_stored_record<const CAPACITY: usize, const PAYLOAD_SIZE: usize>(
    records: &mut [Option<StoredRecord<PAYLOAD_SIZE>>; CAPACITY],
    record: StoredRecord<PAYLOAD_SIZE>,
) {
    if CAPACITY == 0 {
        return;
    }

    if let Some(slot) = records.iter_mut().find(|slot| slot.is_none()) {
        *slot = Some(record);
        return;
    }

    records.rotate_left(1);
    records[CAPACITY - 1] = Some(record);
}

fn max_sequence(current: Option<u64>, sequence: u64) -> u64 {
    current.map_or(sequence, |current| current.max(sequence))
}

fn remove_stored_record_by_sequence<const CAPACITY: usize, const PAYLOAD_SIZE: usize>(
    records: &mut [Option<StoredRecord<PAYLOAD_SIZE>>; CAPACITY],
    sequence: u64,
) {
    if let Some(index) = records
        .iter()
        .position(|record| record.is_some_and(|record| record.sequence == sequence))
    {
        records[index..].rotate_left(1);
        records[CAPACITY - 1] = None;
    }
}

fn queue_encoded_len<const CAPACITY: usize, const PAYLOAD_SIZE: usize>(
    queue: &DropOldestQueue<StoredRecord<PAYLOAD_SIZE>, CAPACITY>,
) -> Result<usize, SpoolError> {
    let mut len = 0_usize;
    for index in 0..queue.len() {
        if let Some(record) = queue.get(index) {
            len = len
                .checked_add(encoded_record_len(record.payload_len)?)
                .ok_or(SpoolError::PayloadTooLarge)?;
        }
    }

    Ok(len)
}

fn records_encoded_len<const N: usize>(records: &[StoredRecord<N>]) -> Result<usize, SpoolError> {
    let mut len = 0_usize;
    for record in records {
        len = len
            .checked_add(encoded_record_len(record.payload_len)?)
            .ok_or(SpoolError::PayloadTooLarge)?;
    }

    Ok(len)
}

pub const fn encoded_ack_len() -> usize {
    ENCODED_ACK_RECORD_LEN
}

const fn align_up(value: usize, alignment: usize) -> usize {
    if alignment == 0 {
        return value;
    }

    let remainder = value % alignment;
    if remainder == 0 {
        value
    } else {
        value + (alignment - remainder)
    }
}

fn copy_queue_records<const CAPACITY: usize, const PAYLOAD_SIZE: usize>(
    queue: &DropOldestQueue<StoredRecord<PAYLOAD_SIZE>, CAPACITY>,
    out: &mut [StoredRecord<PAYLOAD_SIZE>],
) -> usize {
    let mut len = 0;
    for index in 0..queue.len() {
        if let Some(record) = queue.get(index)
            && len < out.len()
        {
            out[len] = *record;
            len += 1;
        }
    }

    len
}

pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffff_u32;

    for byte in data {
        crc ^= *byte as u32;
        for _ in 0..8 {
            let mask = 0_u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }

    !crc
}
