use crate::util::queue::DropOldestQueue;

pub const RECORD_MAGIC: u32 = 0x5345_4d53;
pub const RECORD_VERSION: u8 = 1;
pub const RECORD_HEADER_LEN: usize = 22;
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
}
