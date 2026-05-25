pub const RECORD_MAGIC: u32 = 0x5345_4d53;
pub const ACK_MAGIC: u32 = 0x5345_414b;
pub const RECORD_VERSION: u8 = 1;
pub const RECORD_HEADER_LEN: usize = 22;
pub const ENCODED_RECORD_HEADER_LEN: usize = align_up(RECORD_HEADER_LEN, FLASH_WRITE_ALIGNMENT);
pub const ACK_RECORD_LEN: usize = 14;
pub const FLASH_WRITE_ALIGNMENT: usize = 4;
pub const ENCODED_ACK_RECORD_LEN: usize = align_up(ACK_RECORD_LEN, FLASH_WRITE_ALIGNMENT);
pub const FLASH_ENTRY_BUFFER_LEN: usize = 512;
pub const MAX_PAYLOAD_LEN: usize = u16::MAX as usize;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(target_arch = "riscv32", derive(defmt::Format))]
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
    pub dropped_count: usize,
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

        Ok(AppendResult {
            sequence,
            dropped,
            dropped_count: usize::from(dropped.is_some()),
        })
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
