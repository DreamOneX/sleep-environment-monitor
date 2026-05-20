use crate::{
    storage::flash_model::{FlashError, FlashStorage},
    util::queue::DropOldestQueue,
};

pub const RECORD_MAGIC: u32 = 0x5345_4d53;
pub const ACK_MAGIC: u32 = 0x5345_414b;
pub const RECORD_VERSION: u8 = 1;
pub const RECORD_HEADER_LEN: usize = 22;
pub const ACK_RECORD_LEN: usize = 14;
pub const MAX_PAYLOAD_LEN: usize = u16::MAX as usize;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpoolError {
    BufferTooSmall,
    PayloadTooLarge,
    BadMagic,
    UnsupportedVersion,
    BadHeaderLength,
    BadPayloadLength,
    BadCrc,
    Flash(FlashError),
    FlashFull,
}

impl From<FlashError> for SpoolError {
    fn from(error: FlashError) -> Self {
        Self::Flash(error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpoolRecord<'a> {
    pub sequence: u64,
    pub flags: u8,
    pub payload: &'a [u8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StoredRecord<const N: usize> {
    pub sequence: u64,
    pub flags: u8,
    pub payload: [u8; N],
    pub payload_len: usize,
}

impl<const N: usize> StoredRecord<N> {
    pub fn new(sequence: u64, flags: u8, payload: &[u8]) -> Result<Self, SpoolError> {
        if payload.len() > N {
            return Err(SpoolError::BufferTooSmall);
        }

        let mut stored = Self {
            sequence,
            flags,
            payload: [0_u8; N],
            payload_len: payload.len(),
        };
        stored.payload[..payload.len()].copy_from_slice(payload);

        Ok(stored)
    }

    pub fn as_record(&self) -> SpoolRecord<'_> {
        SpoolRecord {
            sequence: self.sequence,
            flags: self.flags,
            payload: &self.payload[..self.payload_len],
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct AppendResult<const N: usize> {
    pub sequence: u64,
    pub dropped: Option<StoredRecord<N>>,
}

pub struct PersistentSpool<const CAPACITY: usize, const PAYLOAD_SIZE: usize> {
    queue: DropOldestQueue<StoredRecord<PAYLOAD_SIZE>, CAPACITY>,
    next_sequence: u64,
}

impl<const CAPACITY: usize, const PAYLOAD_SIZE: usize> PersistentSpool<CAPACITY, PAYLOAD_SIZE> {
    pub const fn new() -> Self {
        Self {
            queue: DropOldestQueue::new(),
            next_sequence: 0,
        }
    }

    pub const fn len(&self) -> usize {
        self.queue.len()
    }

    pub const fn capacity(&self) -> usize {
        self.queue.capacity()
    }

    pub const fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub fn append(
        &mut self,
        flags: u8,
        payload: &[u8],
    ) -> Result<AppendResult<PAYLOAD_SIZE>, SpoolError> {
        let sequence = self.next_sequence;
        let record = StoredRecord::new(sequence, flags, payload)?;
        let dropped = self.queue.push(record);
        self.next_sequence = self.next_sequence.wrapping_add(1);

        Ok(AppendResult { sequence, dropped })
    }

    pub fn peek(&self) -> Option<SpoolRecord<'_>> {
        self.queue.front().map(StoredRecord::as_record)
    }

    pub fn acknowledge(&mut self) -> Option<StoredRecord<PAYLOAD_SIZE>> {
        self.queue.pop()
    }

    pub fn recover_from_records(records: &[StoredRecord<PAYLOAD_SIZE>]) -> Self {
        let mut spool = Self::new();
        let mut max_sequence = None;

        for record in records {
            let dropped = spool.queue.push(*record);
            let _ = dropped;
            max_sequence = Some(match max_sequence {
                Some(current) if current > record.sequence => current,
                _ => record.sequence,
            });
        }

        spool.next_sequence = max_sequence.map_or(0, |sequence| sequence.wrapping_add(1));
        spool
    }

    fn recover_from_records_with_next_sequence(
        records: &[StoredRecord<PAYLOAD_SIZE>],
        next_sequence: u64,
    ) -> Self {
        let mut spool = Self::recover_from_records(records);
        spool.next_sequence = next_sequence;
        spool
    }
}

impl<const CAPACITY: usize, const PAYLOAD_SIZE: usize> Default
    for PersistentSpool<CAPACITY, PAYLOAD_SIZE>
{
    fn default() -> Self {
        Self::new()
    }
}

pub fn encoded_record_len(payload_len: usize) -> Result<usize, SpoolError> {
    if payload_len > MAX_PAYLOAD_LEN {
        return Err(SpoolError::PayloadTooLarge);
    }

    RECORD_HEADER_LEN
        .checked_add(payload_len)
        .ok_or(SpoolError::PayloadTooLarge)
}

pub fn encoded_log_entry_len(input: &[u8]) -> Result<usize, SpoolError> {
    if input.len() < 4 {
        return Err(SpoolError::BufferTooSmall);
    }

    let magic = u32::from_le_bytes([input[0], input[1], input[2], input[3]]);
    match magic {
        RECORD_MAGIC => {
            if input.len() < RECORD_HEADER_LEN {
                return Err(SpoolError::BufferTooSmall);
            }
            let payload_len = u16::from_le_bytes([input[16], input[17]]) as usize;
            encoded_record_len(payload_len)
        }
        ACK_MAGIC => Ok(ACK_RECORD_LEN),
        _ => Err(SpoolError::BadMagic),
    }
}

pub fn encode_record(record: SpoolRecord<'_>, out: &mut [u8]) -> Result<usize, SpoolError> {
    if record.payload.len() > MAX_PAYLOAD_LEN {
        return Err(SpoolError::PayloadTooLarge);
    }

    let len = encoded_record_len(record.payload.len())?;
    if out.len() < len {
        return Err(SpoolError::BufferTooSmall);
    }

    out[0..4].copy_from_slice(&RECORD_MAGIC.to_le_bytes());
    out[4] = RECORD_VERSION;
    out[5] = record.flags;
    out[6..8].copy_from_slice(&(RECORD_HEADER_LEN as u16).to_le_bytes());
    out[8..16].copy_from_slice(&record.sequence.to_le_bytes());
    out[16..18].copy_from_slice(&(record.payload.len() as u16).to_le_bytes());
    out[18..22].copy_from_slice(&crc32(record.payload).to_le_bytes());
    out[RECORD_HEADER_LEN..len].copy_from_slice(record.payload);

    Ok(len)
}

pub fn decode_record(input: &[u8]) -> Result<SpoolRecord<'_>, SpoolError> {
    if input.len() < RECORD_HEADER_LEN {
        return Err(SpoolError::BufferTooSmall);
    }

    let magic = u32::from_le_bytes([input[0], input[1], input[2], input[3]]);
    if magic != RECORD_MAGIC {
        return Err(SpoolError::BadMagic);
    }

    let version = input[4];
    if version != RECORD_VERSION {
        return Err(SpoolError::UnsupportedVersion);
    }

    let flags = input[5];
    let header_len = u16::from_le_bytes([input[6], input[7]]) as usize;
    if header_len != RECORD_HEADER_LEN {
        return Err(SpoolError::BadHeaderLength);
    }

    let sequence = u64::from_le_bytes([
        input[8], input[9], input[10], input[11], input[12], input[13], input[14], input[15],
    ]);
    let payload_len = u16::from_le_bytes([input[16], input[17]]) as usize;
    let expected_crc = u32::from_le_bytes([input[18], input[19], input[20], input[21]]);
    let end = header_len
        .checked_add(payload_len)
        .ok_or(SpoolError::BadPayloadLength)?;
    let payload = input
        .get(header_len..end)
        .ok_or(SpoolError::BadPayloadLength)?;

    if crc32(payload) != expected_crc {
        return Err(SpoolError::BadCrc);
    }

    Ok(SpoolRecord {
        sequence,
        flags,
        payload,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AckRecord {
    pub sequence: u64,
}

pub fn encode_ack(sequence: u64, out: &mut [u8]) -> Result<usize, SpoolError> {
    if out.len() < ACK_RECORD_LEN {
        return Err(SpoolError::BufferTooSmall);
    }

    out[0..4].copy_from_slice(&ACK_MAGIC.to_le_bytes());
    out[4] = RECORD_VERSION;
    out[5] = 0;
    out[6..14].copy_from_slice(&sequence.to_le_bytes());

    Ok(ACK_RECORD_LEN)
}

pub fn decode_ack(input: &[u8]) -> Result<AckRecord, SpoolError> {
    if input.len() < ACK_RECORD_LEN {
        return Err(SpoolError::BufferTooSmall);
    }

    let magic = u32::from_le_bytes([input[0], input[1], input[2], input[3]]);
    if magic != ACK_MAGIC {
        return Err(SpoolError::BadMagic);
    }

    if input[4] != RECORD_VERSION {
        return Err(SpoolError::UnsupportedVersion);
    }

    Ok(AckRecord {
        sequence: u64::from_le_bytes([
            input[6], input[7], input[8], input[9], input[10], input[11], input[12], input[13],
        ]),
    })
}

pub fn recover_records<'a>(region: &'a [u8], records: &mut [Option<SpoolRecord<'a>>]) -> usize {
    let mut cursor = 0;
    let mut count = 0;

    while cursor + RECORD_HEADER_LEN <= region.len() && count < records.len() {
        if region[cursor..].iter().all(|byte| *byte == 0xff) {
            break;
        }

        match decode_record(&region[cursor..]) {
            Ok(record) => {
                let next = match encoded_record_len(record.payload.len()) {
                    Ok(len) => cursor + len,
                    Err(_) => break,
                };
                records[count] = Some(record);
                count += 1;
                cursor = next;
            }
            Err(SpoolError::BadMagic) => {
                cursor += 1;
            }
            Err(_) => {
                break;
            }
        }
    }

    count
}

pub struct FlashBackedSpool<const CAPACITY: usize, const PAYLOAD_SIZE: usize> {
    spool: PersistentSpool<CAPACITY, PAYLOAD_SIZE>,
    write_offset: usize,
}

impl<const CAPACITY: usize, const PAYLOAD_SIZE: usize> FlashBackedSpool<CAPACITY, PAYLOAD_SIZE> {
    pub const fn new() -> Self {
        Self {
            spool: PersistentSpool::new(),
            write_offset: 0,
        }
    }

    pub fn recover(flash: &impl FlashStorage) -> Result<Self, SpoolError> {
        let mut image = [0_u8; 2048];
        if flash.len() > image.len() {
            return Err(SpoolError::BufferTooSmall);
        }
        flash.read(0, &mut image[..flash.len()])?;

        let mut records = [const { None }; CAPACITY];
        let mut max_sequence_seen = None;
        let mut cursor = 0;

        while cursor + 4 <= flash.len() {
            if image[cursor..flash.len()].iter().all(|byte| *byte == 0xff) {
                break;
            }

            match encoded_log_entry_len(&image[cursor..flash.len()]) {
                Ok(len) if cursor + len <= flash.len() => {
                    let magic = u32::from_le_bytes([
                        image[cursor],
                        image[cursor + 1],
                        image[cursor + 2],
                        image[cursor + 3],
                    ]);
                    match magic {
                        RECORD_MAGIC => match decode_record(&image[cursor..cursor + len]) {
                            Ok(record) => {
                                max_sequence_seen =
                                    Some(max_sequence(max_sequence_seen, record.sequence));
                                push_record(&mut records, record);
                            }
                            Err(_) => break,
                        },
                        ACK_MAGIC => match decode_ack(&image[cursor..cursor + len]) {
                            Ok(ack) => {
                                max_sequence_seen =
                                    Some(max_sequence(max_sequence_seen, ack.sequence));
                                remove_record_by_sequence(&mut records, ack.sequence);
                            }
                            Err(_) => break,
                        },
                        _ => break,
                    }
                    cursor += len;
                }
                Ok(_) => break,
                Err(SpoolError::BadMagic) => {
                    cursor += 1;
                }
                Err(_) => break,
            }
        }

        let mut stored_records = [const { None }; CAPACITY];
        let mut stored_len = 0;
        for record in records.into_iter().flatten() {
            if stored_len < stored_records.len() {
                stored_records[stored_len] = Some(StoredRecord::new(
                    record.sequence,
                    record.flags,
                    record.payload,
                )?);
                stored_len += 1;
            }
        }

        let mut compact = [StoredRecord::<PAYLOAD_SIZE>::new(0, 0, &[])?; CAPACITY];
        for (index, record) in stored_records[..stored_len].iter().flatten().enumerate() {
            compact[index] = *record;
        }

        Ok(Self {
            spool: PersistentSpool::recover_from_records_with_next_sequence(
                &compact[..stored_len],
                max_sequence_seen.map_or(0, |sequence| sequence.wrapping_add(1)),
            ),
            write_offset: cursor,
        })
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
        let mut encoded = [0_u8; 256];
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

        let mut encoded = [0_u8; ACK_RECORD_LEN];
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

        for index in 0..self.spool.len() {
            if let Some(record) = self.spool.queue.get(index) {
                compacted.push(*record);
            }
        }
        dropped = compacted.push(incoming).or(dropped);

        while queue_encoded_len(&compacted)? > flash.len() {
            let removed = compacted.pop();
            dropped = dropped.or(removed);
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
            let mut encoded = [0_u8; 256];
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

fn push_record<'a, const N: usize>(
    records: &mut [Option<SpoolRecord<'a>>; N],
    record: SpoolRecord<'a>,
) {
    if N == 0 {
        return;
    }

    if let Some(slot) = records.iter_mut().find(|slot| slot.is_none()) {
        *slot = Some(record);
        return;
    }

    records.rotate_left(1);
    records[N - 1] = Some(record);
}

fn max_sequence(current: Option<u64>, sequence: u64) -> u64 {
    current.map_or(sequence, |current| current.max(sequence))
}

fn remove_record_by_sequence<const N: usize>(
    records: &mut [Option<SpoolRecord<'_>>; N],
    sequence: u64,
) {
    if let Some(index) = records
        .iter()
        .position(|record| record.is_some_and(|record| record.sequence == sequence))
    {
        records[index..].rotate_left(1);
        records[N - 1] = None;
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

#[cfg(test)]
mod tests {
    use crate::storage::flash_model::{FlashStorage, InMemoryFlash};

    use super::*;

    fn record<'a>(sequence: u64, payload: &'a [u8]) -> SpoolRecord<'a> {
        SpoolRecord {
            sequence,
            flags: 0x5a,
            payload,
        }
    }

    #[test]
    fn crc32_matches_standard_check_value() {
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
    }

    #[test]
    fn record_encodes_and_decodes() {
        let mut out = [0_u8; 64];
        let len = encode_record(record(42, b"payload"), &mut out).unwrap();
        let decoded = decode_record(&out[..len]).unwrap();

        assert_eq!(decoded.sequence, 42);
        assert_eq!(decoded.flags, 0x5a);
        assert_eq!(decoded.payload, b"payload");
    }

    #[test]
    fn encode_reports_small_buffer() {
        let mut out = [0_u8; 4];

        assert_eq!(
            encode_record(record(1, b"payload"), &mut out),
            Err(SpoolError::BufferTooSmall)
        );
    }

    #[test]
    fn decode_rejects_bad_magic() {
        let mut out = [0_u8; 64];
        let len = encode_record(record(1, b"payload"), &mut out).unwrap();
        out[0] = 0;

        assert_eq!(decode_record(&out[..len]), Err(SpoolError::BadMagic));
    }

    #[test]
    fn decode_rejects_unsupported_version() {
        let mut out = [0_u8; 64];
        let len = encode_record(record(1, b"payload"), &mut out).unwrap();
        out[4] = RECORD_VERSION + 1;

        assert_eq!(
            decode_record(&out[..len]),
            Err(SpoolError::UnsupportedVersion)
        );
    }

    #[test]
    fn decode_rejects_bad_header_length() {
        let mut out = [0_u8; 64];
        let len = encode_record(record(1, b"payload"), &mut out).unwrap();
        out[6..8].copy_from_slice(&0_u16.to_le_bytes());

        assert_eq!(decode_record(&out[..len]), Err(SpoolError::BadHeaderLength));
    }

    #[test]
    fn decode_rejects_truncated_payload() {
        let mut out = [0_u8; 64];
        let len = encode_record(record(1, b"payload"), &mut out).unwrap();

        assert_eq!(
            decode_record(&out[..len - 1]),
            Err(SpoolError::BadPayloadLength)
        );
    }

    #[test]
    fn decode_rejects_bad_crc() {
        let mut out = [0_u8; 64];
        let len = encode_record(record(1, b"payload"), &mut out).unwrap();
        out[RECORD_HEADER_LEN] ^= 0x01;

        assert_eq!(decode_record(&out[..len]), Err(SpoolError::BadCrc));
    }

    #[test]
    fn append_preserves_fifo_order() {
        let mut spool = PersistentSpool::<4, 16>::new();

        spool.append(0, b"one").unwrap();
        spool.append(0, b"two").unwrap();

        assert_eq!(spool.peek().unwrap().payload, b"one");
        assert_eq!(spool.acknowledge().unwrap().as_record().payload, b"one");
        assert_eq!(spool.peek().unwrap().payload, b"two");
    }

    #[test]
    fn acknowledge_removes_only_oldest_uploaded_record() {
        let mut spool = PersistentSpool::<3, 16>::new();

        spool.append(0, b"one").unwrap();
        spool.append(0, b"two").unwrap();
        spool.append(0, b"three").unwrap();

        assert_eq!(spool.acknowledge().unwrap().as_record().payload, b"one");
        assert_eq!(spool.peek().unwrap().payload, b"two");
        assert_eq!(spool.len(), 2);
    }

    #[test]
    fn full_spool_drops_oldest_records() {
        let mut spool = PersistentSpool::<2, 16>::new();

        assert!(spool.append(0, b"one").unwrap().dropped.is_none());
        assert!(spool.append(0, b"two").unwrap().dropped.is_none());
        let dropped = spool.append(0, b"three").unwrap().dropped.unwrap();

        assert_eq!(dropped.as_record().payload, b"one");
        assert_eq!(spool.acknowledge().unwrap().as_record().payload, b"two");
        assert_eq!(spool.acknowledge().unwrap().as_record().payload, b"three");
    }

    #[test]
    fn payload_larger_than_stored_record_is_rejected() {
        let mut spool = PersistentSpool::<2, 4>::new();

        assert_eq!(spool.append(0, b"12345"), Err(SpoolError::BufferTooSmall));
    }

    #[test]
    fn sequence_wrap_is_defined() {
        let stored = StoredRecord::<8>::new(u64::MAX, 0, b"last").unwrap();
        let mut spool = PersistentSpool::<2, 8>::recover_from_records(&[stored]);

        assert_eq!(spool.next_sequence(), 0);
        let result = spool.append(0, b"next").unwrap();
        assert_eq!(result.sequence, 0);
    }

    #[test]
    fn recover_from_records_sets_next_sequence() {
        let first = StoredRecord::<8>::new(7, 0, b"one").unwrap();
        let second = StoredRecord::<8>::new(8, 0, b"two").unwrap();
        let spool = PersistentSpool::<4, 8>::recover_from_records(&[first, second]);

        assert_eq!(spool.next_sequence(), 9);
        assert_eq!(spool.peek().unwrap().payload, b"one");
    }

    #[test]
    fn recover_records_reads_encoded_records() {
        let mut region = [0xff_u8; 128];
        let first_len = encode_record(record(10, b"one"), &mut region).unwrap();
        let second_len = encode_record(record(11, b"two"), &mut region[first_len..]).unwrap();
        let mut records = [None; 4];

        let count = recover_records(&region[..first_len + second_len], &mut records);

        assert_eq!(count, 2);
        assert_eq!(records[0].unwrap().sequence, 10);
        assert_eq!(records[0].unwrap().payload, b"one");
        assert_eq!(records[1].unwrap().sequence, 11);
        assert_eq!(records[1].unwrap().payload, b"two");
    }

    #[test]
    fn recover_records_ignores_partial_tail() {
        let mut region = [0xff_u8; 128];
        let first_len = encode_record(record(10, b"one"), &mut region).unwrap();
        let partial = &mut region[first_len..first_len + 8];
        partial.copy_from_slice(&RECORD_MAGIC.to_le_bytes().repeat(2));
        let mut records = [None; 4];

        let count = recover_records(&region, &mut records);

        assert_eq!(count, 1);
        assert_eq!(records[0].unwrap().payload, b"one");
    }

    #[test]
    fn recover_records_resynchronizes_after_bad_magic_bytes() {
        let mut region = [0xff_u8; 128];
        region[0..3].copy_from_slice(&[0, 1, 2]);
        let len = encode_record(record(12, b"after"), &mut region[3..]).unwrap();
        let mut records = [None; 2];

        let count = recover_records(&region[..3 + len], &mut records);

        assert_eq!(count, 1);
        assert_eq!(records[0].unwrap().sequence, 12);
        assert_eq!(records[0].unwrap().payload, b"after");
    }

    #[test]
    fn flash_backed_spool_recovers_records_after_simulated_reboot() {
        let mut flash = InMemoryFlash::<128, 64>::new();
        let mut spool = FlashBackedSpool::<4, 16>::new();

        spool.append(&mut flash, 0, b"one").unwrap();
        spool.append(&mut flash, 0, b"two").unwrap();

        let recovered = FlashBackedSpool::<4, 16>::recover(&flash).unwrap();

        assert_eq!(recovered.len(), 2);
        assert_eq!(recovered.next_sequence(), 2);
        assert_eq!(recovered.peek().unwrap().payload, b"one");
    }

    #[test]
    fn flash_backed_spool_ack_persists_after_simulated_reboot() {
        let mut flash = InMemoryFlash::<128, 64>::new();
        let mut spool = FlashBackedSpool::<4, 16>::new();

        spool.append(&mut flash, 0, b"one").unwrap();
        spool.append(&mut flash, 0, b"two").unwrap();
        assert_eq!(
            spool
                .acknowledge(&mut flash)
                .unwrap()
                .unwrap()
                .as_record()
                .payload,
            b"one"
        );

        let recovered = FlashBackedSpool::<4, 16>::recover(&flash).unwrap();

        assert_eq!(recovered.len(), 1);
        assert_eq!(recovered.next_sequence(), 2);
        assert_eq!(recovered.peek().unwrap().payload, b"two");
    }

    #[test]
    fn flash_backed_spool_recovery_preserves_order_after_ack_hole() {
        let mut flash = InMemoryFlash::<160, 32>::new();
        let mut spool = FlashBackedSpool::<4, 16>::new();

        spool.append(&mut flash, 0, b"one").unwrap();
        spool.append(&mut flash, 0, b"two").unwrap();
        spool.acknowledge(&mut flash).unwrap();
        spool.append(&mut flash, 0, b"three").unwrap();

        let mut recovered = FlashBackedSpool::<4, 16>::recover(&flash).unwrap();

        assert_eq!(recovered.next_sequence(), 3);
        assert_eq!(recovered.peek().unwrap().payload, b"two");
        assert_eq!(
            recovered
                .acknowledge(&mut flash)
                .unwrap()
                .unwrap()
                .as_record()
                .payload,
            b"two"
        );
        assert_eq!(recovered.peek().unwrap().payload, b"three");
    }

    #[test]
    fn flash_backed_spool_appends_across_sector_boundary() {
        let mut flash = InMemoryFlash::<128, 32>::new();
        let mut spool = FlashBackedSpool::<4, 16>::new();

        spool.append(&mut flash, 0, b"aaaa").unwrap();
        spool.append(&mut flash, 0, b"bbbb").unwrap();
        spool.append(&mut flash, 0, b"cccc").unwrap();

        let recovered = FlashBackedSpool::<4, 16>::recover(&flash).unwrap();

        assert_eq!(recovered.len(), 3);
        assert_eq!(recovered.peek().unwrap().payload, b"aaaa");
    }

    #[test]
    fn flash_backed_spool_ignores_interrupted_append_tail() {
        let mut flash = InMemoryFlash::<128, 64>::new();
        let mut spool = FlashBackedSpool::<4, 16>::new();

        spool.append(&mut flash, 0, b"one").unwrap();
        let partial_offset = encoded_record_len(b"one".len()).unwrap();
        flash
            .write(partial_offset, &RECORD_MAGIC.to_le_bytes())
            .unwrap();

        let recovered = FlashBackedSpool::<4, 16>::recover(&flash).unwrap();

        assert_eq!(recovered.len(), 1);
        assert_eq!(recovered.next_sequence(), 1);
        assert_eq!(recovered.peek().unwrap().payload, b"one");
    }

    #[test]
    fn flash_backed_spool_drops_oldest_when_modeled_flash_fills() {
        let mut flash = InMemoryFlash::<64, 64>::new();
        let mut spool = FlashBackedSpool::<4, 16>::new();

        spool.append(&mut flash, 0, b"aaaa").unwrap();
        spool.append(&mut flash, 0, b"bbbb").unwrap();
        let result = spool.append(&mut flash, 0, b"cccc").unwrap();

        assert_eq!(result.sequence, 2);
        assert_eq!(result.dropped.unwrap().as_record().payload, b"aaaa");

        let recovered = FlashBackedSpool::<4, 16>::recover(&flash).unwrap();

        assert_eq!(recovered.len(), 2);
        assert_eq!(recovered.next_sequence(), 3);
        assert_eq!(recovered.peek().unwrap().payload, b"bbbb");
    }

    #[test]
    fn flash_backed_spool_ack_compaction_preserves_next_sequence() {
        let mut flash = InMemoryFlash::<64, 64>::new();
        let mut spool = FlashBackedSpool::<4, 16>::new();

        spool.append(&mut flash, 0, b"aaaa").unwrap();
        spool.append(&mut flash, 0, b"bbbb").unwrap();
        spool.acknowledge(&mut flash).unwrap();
        spool.acknowledge(&mut flash).unwrap();

        assert_eq!(spool.len(), 0);
        assert_eq!(spool.next_sequence(), 2);

        let result = spool.append(&mut flash, 0, b"cccc").unwrap();

        assert_eq!(result.sequence, 2);
        assert_eq!(spool.next_sequence(), 3);
        assert_eq!(spool.peek().unwrap().payload, b"cccc");
    }
}
