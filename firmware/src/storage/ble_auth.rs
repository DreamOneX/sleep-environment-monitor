use crate::storage::{
    flash_model::{FlashError, FlashStorage},
    spool::crc32,
};

pub const AUTH_HEADER_MAGIC: u32 = 0x5345_4241;
pub const AUTH_HEADER_FORMAT_VERSION: u16 = 2;
pub const AUTH_HEADER_LEN: usize = 32;
pub const AUTH_RECORD_MAGIC: u32 = 0x5345_4252;
pub const AUTH_RECORD_FORMAT_VERSION: u16 = 1;
pub const AUTH_RECORD_LEN: usize = 64;
pub const AUTH_RECORD_FLASH_ALIGNMENT: usize = 4;
pub const AUTH_RECORD_FLAG_BONDED: u8 = 0x01;
pub const AUTH_RECORD_FLAG_IRK_PRESENT: u8 = 0x02;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(target_arch = "riscv32", derive(defmt::Format))]
pub enum BleAuthRecordStatus {
    Missing,
    BadMagic,
    UnsupportedFormat {
        stored: u16,
        expected: u16,
    },
    BadHeaderLength {
        stored: u16,
        expected: u16,
    },
    ChecksumMismatch {
        stored: u32,
        computed: u32,
    },
    RecordsVersionMismatch {
        stored: u32,
        expected: u32,
    },
    CompatibilityChecksumMismatch {
        stored: u32,
        expected: u32,
    },
    NoRecords,
    RecordCountTooLarge {
        stored: u16,
        capacity: u16,
    },
    RecordRegionTooLarge,
    RecordSetChecksumMismatch {
        stored: u32,
        computed: u32,
    },
    BadRecordMagic {
        index: u16,
    },
    UnsupportedRecordFormat {
        index: u16,
        stored: u16,
        expected: u16,
    },
    BadRecordLength {
        index: u16,
        stored: u16,
        expected: u16,
    },
    BadRecordAddressKind {
        index: u16,
        stored: u8,
    },
    BadRecordSecurityLevel {
        index: u16,
        stored: u8,
    },
    RecordCrcMismatch {
        index: u16,
        stored: u32,
        computed: u32,
    },
    Valid {
        records_version: u32,
        record_count: u16,
    },
}

impl BleAuthRecordStatus {
    pub const fn requires_pairing_window(self) -> bool {
        !matches!(self, Self::Valid { .. })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(target_arch = "riscv32", derive(defmt::Format))]
pub enum BleAuthStorageError {
    BufferTooSmall,
    TooManyRecords,
    RecordRegionTooLarge,
    UnalignedFlash,
    Flash(FlashError),
}

impl From<FlashError> for BleAuthStorageError {
    fn from(error: FlashError) -> Self {
        Self::Flash(error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(target_arch = "riscv32", derive(defmt::Format))]
pub enum BleAuthHeaderError {
    BufferTooSmall,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(target_arch = "riscv32", derive(defmt::Format))]
pub enum BleAuthAddressKind {
    Public,
    Random,
}

impl BleAuthAddressKind {
    const fn code(self) -> u8 {
        match self {
            Self::Public => 0,
            Self::Random => 1,
        }
    }

    const fn from_code(code: u8) -> Option<Self> {
        match code {
            0 => Some(Self::Public),
            1 => Some(Self::Random),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(target_arch = "riscv32", derive(defmt::Format))]
pub enum BleAuthSecurityLevel {
    NoEncryption,
    Encrypted,
    EncryptedAuthenticated,
}

impl BleAuthSecurityLevel {
    const fn code(self) -> u8 {
        match self {
            Self::NoEncryption => 0,
            Self::Encrypted => 1,
            Self::EncryptedAuthenticated => 2,
        }
    }

    const fn from_code(code: u8) -> Option<Self> {
        match code {
            0 => Some(Self::NoEncryption),
            1 => Some(Self::Encrypted),
            2 => Some(Self::EncryptedAuthenticated),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BleAuthRecord {
    pub address_kind: BleAuthAddressKind,
    pub identity_address: [u8; 6],
    pub long_term_key: [u8; 16],
    pub identity_resolving_key: Option<[u8; 16]>,
    pub security_level: BleAuthSecurityLevel,
    pub bonded: bool,
}

impl BleAuthRecord {
    pub const EMPTY: Self = Self {
        address_kind: BleAuthAddressKind::Public,
        identity_address: [0_u8; 6],
        long_term_key: [0_u8; 16],
        identity_resolving_key: None,
        security_level: BleAuthSecurityLevel::NoEncryption,
        bonded: false,
    };
}

impl Default for BleAuthRecord {
    fn default() -> Self {
        Self::EMPTY
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BleAuthLoadResult {
    pub status: BleAuthRecordStatus,
    pub record_count: usize,
}

pub fn encode_auth_header(
    records_version: u32,
    record_count: u16,
    records_checksum: u32,
    compatibility_checksum: u32,
    out: &mut [u8],
) -> Result<usize, BleAuthHeaderError> {
    if out.len() < AUTH_HEADER_LEN {
        return Err(BleAuthHeaderError::BufferTooSmall);
    }

    out[..AUTH_HEADER_LEN].fill(0);
    out[0..4].copy_from_slice(&AUTH_HEADER_MAGIC.to_le_bytes());
    out[4..6].copy_from_slice(&AUTH_HEADER_FORMAT_VERSION.to_le_bytes());
    out[6..8].copy_from_slice(&(AUTH_HEADER_LEN as u16).to_le_bytes());
    out[8..12].copy_from_slice(&records_version.to_le_bytes());
    out[12..14].copy_from_slice(&record_count.to_le_bytes());
    out[14..16].copy_from_slice(&0_u16.to_le_bytes());
    out[16..20].copy_from_slice(&records_checksum.to_le_bytes());
    out[20..24].copy_from_slice(&compatibility_checksum.to_le_bytes());
    out[24..28].copy_from_slice(&0_u32.to_le_bytes());

    let header_checksum = crc32(&out[..28]);
    out[28..32].copy_from_slice(&header_checksum.to_le_bytes());
    Ok(AUTH_HEADER_LEN)
}

pub fn inspect_auth_header(
    input: &[u8],
    expected_records_version: u32,
    expected_compatibility_checksum: u32,
) -> BleAuthRecordStatus {
    if input.len() < AUTH_HEADER_LEN {
        return BleAuthRecordStatus::BadHeaderLength {
            stored: input.len() as u16,
            expected: AUTH_HEADER_LEN as u16,
        };
    }
    if input[..AUTH_HEADER_LEN].iter().all(|byte| *byte == 0xff) {
        return BleAuthRecordStatus::Missing;
    }

    let magic = u32::from_le_bytes([input[0], input[1], input[2], input[3]]);
    if magic != AUTH_HEADER_MAGIC {
        return BleAuthRecordStatus::BadMagic;
    }

    let format_version = u16::from_le_bytes([input[4], input[5]]);
    if format_version != AUTH_HEADER_FORMAT_VERSION {
        return BleAuthRecordStatus::UnsupportedFormat {
            stored: format_version,
            expected: AUTH_HEADER_FORMAT_VERSION,
        };
    }

    let header_len = u16::from_le_bytes([input[6], input[7]]);
    if header_len != AUTH_HEADER_LEN as u16 {
        return BleAuthRecordStatus::BadHeaderLength {
            stored: header_len,
            expected: AUTH_HEADER_LEN as u16,
        };
    }

    let stored_checksum = u32::from_le_bytes([input[28], input[29], input[30], input[31]]);
    let computed_checksum = crc32(&input[..28]);
    if stored_checksum != computed_checksum {
        return BleAuthRecordStatus::ChecksumMismatch {
            stored: stored_checksum,
            computed: computed_checksum,
        };
    }

    let records_version = u32::from_le_bytes([input[8], input[9], input[10], input[11]]);
    if records_version != expected_records_version {
        return BleAuthRecordStatus::RecordsVersionMismatch {
            stored: records_version,
            expected: expected_records_version,
        };
    }

    let compatibility_checksum = u32::from_le_bytes([input[20], input[21], input[22], input[23]]);
    if compatibility_checksum != expected_compatibility_checksum {
        return BleAuthRecordStatus::CompatibilityChecksumMismatch {
            stored: compatibility_checksum,
            expected: expected_compatibility_checksum,
        };
    }

    let record_count = u16::from_le_bytes([input[12], input[13]]);
    if record_count == 0 {
        return BleAuthRecordStatus::NoRecords;
    }

    BleAuthRecordStatus::Valid {
        records_version,
        record_count,
    }
}

pub fn load_auth_records(
    flash: &impl FlashStorage,
    expected_records_version: u32,
    expected_compatibility_checksum: u32,
    records_out: &mut [BleAuthRecord],
    scratch: &mut [u8],
) -> Result<BleAuthLoadResult, BleAuthStorageError> {
    let mut header = [0_u8; AUTH_HEADER_LEN];
    flash.read(0, &mut header)?;
    let status = inspect_auth_header(
        &header,
        expected_records_version,
        expected_compatibility_checksum,
    );
    let BleAuthRecordStatus::Valid { record_count, .. } = status else {
        return Ok(BleAuthLoadResult {
            status,
            record_count: 0,
        });
    };

    let count = record_count as usize;
    if count > records_out.len() {
        return Ok(BleAuthLoadResult {
            status: BleAuthRecordStatus::RecordCountTooLarge {
                stored: record_count,
                capacity: records_out.len().min(u16::MAX as usize) as u16,
            },
            record_count: 0,
        });
    }

    let records_len = auth_records_encoded_len(count)?;
    let records_end = AUTH_HEADER_LEN
        .checked_add(records_len)
        .ok_or(BleAuthStorageError::RecordRegionTooLarge)?;
    if records_end > flash.len() {
        return Ok(BleAuthLoadResult {
            status: BleAuthRecordStatus::RecordRegionTooLarge,
            record_count: 0,
        });
    }
    if scratch.len() < records_len {
        return Err(BleAuthStorageError::BufferTooSmall);
    }

    flash.read(AUTH_HEADER_LEN, &mut scratch[..records_len])?;
    let stored_records_checksum =
        u32::from_le_bytes([header[16], header[17], header[18], header[19]]);
    let computed_records_checksum = records_checksum(&scratch[..records_len]);
    if stored_records_checksum != computed_records_checksum {
        return Ok(BleAuthLoadResult {
            status: BleAuthRecordStatus::RecordSetChecksumMismatch {
                stored: stored_records_checksum,
                computed: computed_records_checksum,
            },
            record_count: 0,
        });
    }

    for (index, chunk) in scratch[..records_len]
        .chunks_exact(AUTH_RECORD_LEN)
        .enumerate()
    {
        match decode_auth_record(chunk) {
            Ok(record) => records_out[index] = record,
            Err(status) => {
                return Ok(BleAuthLoadResult {
                    status: status.with_record_index(index.min(u16::MAX as usize) as u16),
                    record_count: 0,
                });
            }
        }
    }

    Ok(BleAuthLoadResult {
        status,
        record_count: count,
    })
}

pub fn store_auth_records(
    flash: &mut impl FlashStorage,
    records_version: u32,
    compatibility_checksum: u32,
    records: &[BleAuthRecord],
    scratch: &mut [u8],
) -> Result<(), BleAuthStorageError> {
    if records.len() > u16::MAX as usize {
        return Err(BleAuthStorageError::TooManyRecords);
    }
    if !flash.len().is_multiple_of(flash.sector_size())
        || !AUTH_HEADER_LEN.is_multiple_of(AUTH_RECORD_FLASH_ALIGNMENT)
    {
        return Err(BleAuthStorageError::UnalignedFlash);
    }

    let records_len = auth_records_encoded_len(records.len())?;
    let used_len = AUTH_HEADER_LEN
        .checked_add(records_len)
        .ok_or(BleAuthStorageError::RecordRegionTooLarge)?;
    if used_len > flash.len() {
        return Err(BleAuthStorageError::RecordRegionTooLarge);
    }
    if scratch.len() < used_len {
        return Err(BleAuthStorageError::BufferTooSmall);
    }

    scratch[..used_len].fill(0xff);
    for (index, record) in records.iter().enumerate() {
        let offset = AUTH_HEADER_LEN + index * AUTH_RECORD_LEN;
        encode_auth_record(*record, &mut scratch[offset..offset + AUTH_RECORD_LEN])?;
    }
    let checksum = records_checksum(&scratch[AUTH_HEADER_LEN..used_len]);
    encode_auth_header(
        records_version,
        records.len() as u16,
        checksum,
        compatibility_checksum,
        &mut scratch[..AUTH_HEADER_LEN],
    )
    .map_err(|_| BleAuthStorageError::BufferTooSmall)?;

    flash.erase(0, flash.len())?;
    flash.write(0, &scratch[..used_len])?;
    Ok(())
}

pub fn clear_auth_records(flash: &mut impl FlashStorage) -> Result<(), BleAuthStorageError> {
    flash.erase(0, flash.len()).map_err(Into::into)
}

pub fn encode_auth_record(
    record: BleAuthRecord,
    out: &mut [u8],
) -> Result<usize, BleAuthStorageError> {
    if out.len() < AUTH_RECORD_LEN {
        return Err(BleAuthStorageError::BufferTooSmall);
    }

    out[..AUTH_RECORD_LEN].fill(0);
    out[0..4].copy_from_slice(&AUTH_RECORD_MAGIC.to_le_bytes());
    out[4..6].copy_from_slice(&AUTH_RECORD_FORMAT_VERSION.to_le_bytes());
    out[6..8].copy_from_slice(&(AUTH_RECORD_LEN as u16).to_le_bytes());
    out[8] = record.address_kind.code();
    out[9] = record.security_level.code();
    let bonded_flag = u8::from(record.bonded) * AUTH_RECORD_FLAG_BONDED;
    let irk_flag = u8::from(record.identity_resolving_key.is_some()) * AUTH_RECORD_FLAG_IRK_PRESENT;
    out[10] = bonded_flag | irk_flag;
    out[12..18].copy_from_slice(&record.identity_address);
    out[18..34].copy_from_slice(&record.long_term_key);
    if let Some(irk) = record.identity_resolving_key {
        out[34..50].copy_from_slice(&irk);
    }

    let checksum = crc32(&out[..60]);
    out[60..64].copy_from_slice(&checksum.to_le_bytes());
    Ok(AUTH_RECORD_LEN)
}

pub fn decode_auth_record(input: &[u8]) -> Result<BleAuthRecord, BleAuthRecordStatus> {
    if input.len() < AUTH_RECORD_LEN {
        return Err(BleAuthRecordStatus::BadRecordLength {
            index: 0,
            stored: input.len() as u16,
            expected: AUTH_RECORD_LEN as u16,
        });
    }

    let stored_checksum = u32::from_le_bytes([input[60], input[61], input[62], input[63]]);
    let computed_checksum = crc32(&input[..60]);
    if stored_checksum != computed_checksum {
        return Err(BleAuthRecordStatus::RecordCrcMismatch {
            index: 0,
            stored: stored_checksum,
            computed: computed_checksum,
        });
    }

    let magic = u32::from_le_bytes([input[0], input[1], input[2], input[3]]);
    if magic != AUTH_RECORD_MAGIC {
        return Err(BleAuthRecordStatus::BadRecordMagic { index: 0 });
    }

    let format_version = u16::from_le_bytes([input[4], input[5]]);
    if format_version != AUTH_RECORD_FORMAT_VERSION {
        return Err(BleAuthRecordStatus::UnsupportedRecordFormat {
            index: 0,
            stored: format_version,
            expected: AUTH_RECORD_FORMAT_VERSION,
        });
    }

    let record_len = u16::from_le_bytes([input[6], input[7]]);
    if record_len != AUTH_RECORD_LEN as u16 {
        return Err(BleAuthRecordStatus::BadRecordLength {
            index: 0,
            stored: record_len,
            expected: AUTH_RECORD_LEN as u16,
        });
    }

    let Some(address_kind) = BleAuthAddressKind::from_code(input[8]) else {
        return Err(BleAuthRecordStatus::BadRecordAddressKind {
            index: 0,
            stored: input[8],
        });
    };
    let Some(security_level) = BleAuthSecurityLevel::from_code(input[9]) else {
        return Err(BleAuthRecordStatus::BadRecordSecurityLevel {
            index: 0,
            stored: input[9],
        });
    };

    let flags = input[10];
    let mut identity_address = [0_u8; 6];
    identity_address.copy_from_slice(&input[12..18]);
    let mut long_term_key = [0_u8; 16];
    long_term_key.copy_from_slice(&input[18..34]);
    let identity_resolving_key = if flags & AUTH_RECORD_FLAG_IRK_PRESENT != 0 {
        let mut irk = [0_u8; 16];
        irk.copy_from_slice(&input[34..50]);
        Some(irk)
    } else {
        None
    };

    Ok(BleAuthRecord {
        address_kind,
        identity_address,
        long_term_key,
        identity_resolving_key,
        security_level,
        bonded: flags & AUTH_RECORD_FLAG_BONDED != 0,
    })
}

pub fn records_checksum(encoded_records: &[u8]) -> u32 {
    crc32(encoded_records)
}

pub const fn should_auto_open_pairing_window(enabled: bool, status: BleAuthRecordStatus) -> bool {
    enabled && status.requires_pairing_window()
}

fn auth_records_encoded_len(record_count: usize) -> Result<usize, BleAuthStorageError> {
    record_count
        .checked_mul(AUTH_RECORD_LEN)
        .ok_or(BleAuthStorageError::RecordRegionTooLarge)
}

impl BleAuthRecordStatus {
    const fn with_record_index(self, index: u16) -> Self {
        match self {
            Self::BadRecordMagic { .. } => Self::BadRecordMagic { index },
            Self::UnsupportedRecordFormat {
                stored, expected, ..
            } => Self::UnsupportedRecordFormat {
                index,
                stored,
                expected,
            },
            Self::BadRecordLength {
                stored, expected, ..
            } => Self::BadRecordLength {
                index,
                stored,
                expected,
            },
            Self::BadRecordAddressKind { stored, .. } => {
                Self::BadRecordAddressKind { index, stored }
            }
            Self::BadRecordSecurityLevel { stored, .. } => {
                Self::BadRecordSecurityLevel { index, stored }
            }
            Self::RecordCrcMismatch {
                stored, computed, ..
            } => Self::RecordCrcMismatch {
                index,
                stored,
                computed,
            },
            status => status,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::storage::flash_model::InMemoryFlash;

    use super::*;

    const EXPECTED_VERSION: u32 = 7;
    const EXPECTED_COMPATIBILITY_CHECKSUM: u32 = 0x1234_5678;
    const FLASH_LEN: usize = 4096;
    const SECTOR_SIZE: usize = 4096;

    fn valid_header(
        records_version: u32,
        record_count: u16,
        records_checksum: u32,
        compatibility_checksum: u32,
    ) -> [u8; AUTH_HEADER_LEN] {
        let mut out = [0_u8; AUTH_HEADER_LEN];
        encode_auth_header(
            records_version,
            record_count,
            records_checksum,
            compatibility_checksum,
            &mut out,
        )
        .unwrap();
        out
    }

    fn record(seed: u8) -> BleAuthRecord {
        BleAuthRecord {
            address_kind: BleAuthAddressKind::Random,
            identity_address: [seed, seed + 1, seed + 2, seed + 3, seed + 4, seed + 5],
            long_term_key: [seed; 16],
            identity_resolving_key: Some([seed.wrapping_add(0x80); 16]),
            security_level: BleAuthSecurityLevel::Encrypted,
            bonded: true,
        }
    }

    #[test]
    fn encode_reports_small_buffer() {
        let mut out = [0_u8; AUTH_HEADER_LEN - 1];

        assert_eq!(
            encode_auth_header(EXPECTED_VERSION, 1, 0, 0, &mut out),
            Err(BleAuthHeaderError::BufferTooSmall)
        );
    }

    #[test]
    fn erased_flash_requires_pairing_window() {
        let header = [0xff_u8; AUTH_HEADER_LEN];
        let status =
            inspect_auth_header(&header, EXPECTED_VERSION, EXPECTED_COMPATIBILITY_CHECKSUM);

        assert_eq!(status, BleAuthRecordStatus::Missing);
        assert!(should_auto_open_pairing_window(true, status));
    }

    #[test]
    fn truncated_erased_header_reports_bad_length() {
        let header = [0xff_u8; AUTH_HEADER_LEN - 1];
        let status =
            inspect_auth_header(&header, EXPECTED_VERSION, EXPECTED_COMPATIBILITY_CHECKSUM);

        assert_eq!(
            status,
            BleAuthRecordStatus::BadHeaderLength {
                stored: (AUTH_HEADER_LEN - 1) as u16,
                expected: AUTH_HEADER_LEN as u16
            }
        );
        assert!(should_auto_open_pairing_window(true, status));
    }

    #[test]
    fn empty_current_version_record_requires_pairing_window() {
        let header = valid_header(EXPECTED_VERSION, 0, 0, EXPECTED_COMPATIBILITY_CHECKSUM);
        let status =
            inspect_auth_header(&header, EXPECTED_VERSION, EXPECTED_COMPATIBILITY_CHECKSUM);

        assert_eq!(status, BleAuthRecordStatus::NoRecords);
        assert!(should_auto_open_pairing_window(true, status));
    }

    #[test]
    fn valid_current_version_record_keeps_pairing_window_closed() {
        let header = valid_header(EXPECTED_VERSION, 1, 0, EXPECTED_COMPATIBILITY_CHECKSUM);
        let status =
            inspect_auth_header(&header, EXPECTED_VERSION, EXPECTED_COMPATIBILITY_CHECKSUM);

        assert_eq!(
            status,
            BleAuthRecordStatus::Valid {
                records_version: EXPECTED_VERSION,
                record_count: 1
            }
        );
        assert!(!should_auto_open_pairing_window(true, status));
    }

    #[test]
    fn records_version_change_requires_pairing_window_even_with_records() {
        let header = valid_header(EXPECTED_VERSION - 1, 2, 0, EXPECTED_COMPATIBILITY_CHECKSUM);
        let status =
            inspect_auth_header(&header, EXPECTED_VERSION, EXPECTED_COMPATIBILITY_CHECKSUM);

        assert_eq!(
            status,
            BleAuthRecordStatus::RecordsVersionMismatch {
                stored: EXPECTED_VERSION - 1,
                expected: EXPECTED_VERSION
            }
        );
        assert!(should_auto_open_pairing_window(true, status));
    }

    #[test]
    fn compatibility_checksum_change_requires_pairing_window_even_with_records() {
        let header = valid_header(
            EXPECTED_VERSION,
            2,
            0,
            EXPECTED_COMPATIBILITY_CHECKSUM ^ 0x01,
        );
        let status =
            inspect_auth_header(&header, EXPECTED_VERSION, EXPECTED_COMPATIBILITY_CHECKSUM);

        assert_eq!(
            status,
            BleAuthRecordStatus::CompatibilityChecksumMismatch {
                stored: EXPECTED_COMPATIBILITY_CHECKSUM ^ 0x01,
                expected: EXPECTED_COMPATIBILITY_CHECKSUM
            }
        );
        assert!(should_auto_open_pairing_window(true, status));
    }

    #[test]
    fn checksum_mismatch_requires_pairing_window_even_with_records() {
        let mut header = valid_header(EXPECTED_VERSION, 2, 0, EXPECTED_COMPATIBILITY_CHECKSUM);
        header[16] ^= 0x01;
        let status =
            inspect_auth_header(&header, EXPECTED_VERSION, EXPECTED_COMPATIBILITY_CHECKSUM);

        assert!(matches!(
            status,
            BleAuthRecordStatus::ChecksumMismatch { .. }
        ));
        assert!(should_auto_open_pairing_window(true, status));
    }

    #[test]
    fn config_switch_can_disable_auto_pairing_window() {
        let header = [0xff_u8; AUTH_HEADER_LEN];
        let status =
            inspect_auth_header(&header, EXPECTED_VERSION, EXPECTED_COMPATIBILITY_CHECKSUM);

        assert!(!should_auto_open_pairing_window(false, status));
    }

    #[test]
    fn auth_record_round_trips() {
        let source = record(0x10);
        let mut encoded = [0_u8; AUTH_RECORD_LEN];

        assert_eq!(
            encode_auth_record(source, &mut encoded),
            Ok(AUTH_RECORD_LEN)
        );
        assert_eq!(decode_auth_record(&encoded), Ok(source));
    }

    #[test]
    fn auth_record_crc_mismatch_is_rejected() {
        let mut encoded = [0_u8; AUTH_RECORD_LEN];
        encode_auth_record(record(0x20), &mut encoded).unwrap();
        encoded[18] ^= 0x01;

        assert!(matches!(
            decode_auth_record(&encoded),
            Err(BleAuthRecordStatus::RecordCrcMismatch { .. })
        ));
    }

    #[test]
    fn erased_flash_loads_as_missing() {
        let flash = InMemoryFlash::<FLASH_LEN, SECTOR_SIZE>::new();
        let mut out = [record(0); 2];
        let mut scratch = [0_u8; 2 * AUTH_RECORD_LEN];

        let result = load_auth_records(
            &flash,
            EXPECTED_VERSION,
            EXPECTED_COMPATIBILITY_CHECKSUM,
            &mut out,
            &mut scratch,
        )
        .unwrap();

        assert_eq!(
            result,
            BleAuthLoadResult {
                status: BleAuthRecordStatus::Missing,
                record_count: 0
            }
        );
    }

    #[test]
    fn valid_record_set_loads_records() {
        let mut flash = InMemoryFlash::<FLASH_LEN, SECTOR_SIZE>::new();
        let records = [record(1), record(2)];
        let mut scratch = [0_u8; FLASH_LEN];
        store_auth_records(
            &mut flash,
            EXPECTED_VERSION,
            EXPECTED_COMPATIBILITY_CHECKSUM,
            &records,
            &mut scratch,
        )
        .unwrap();

        let mut loaded = [record(0); 2];
        let result = load_auth_records(
            &flash,
            EXPECTED_VERSION,
            EXPECTED_COMPATIBILITY_CHECKSUM,
            &mut loaded,
            &mut scratch,
        )
        .unwrap();

        assert_eq!(
            result,
            BleAuthLoadResult {
                status: BleAuthRecordStatus::Valid {
                    records_version: EXPECTED_VERSION,
                    record_count: 2
                },
                record_count: 2
            }
        );
        assert_eq!(loaded, records);
    }

    #[test]
    fn record_set_checksum_mismatch_reopens_pairing() {
        let mut flash = InMemoryFlash::<FLASH_LEN, SECTOR_SIZE>::new();
        let records = [record(3)];
        let mut scratch = [0_u8; FLASH_LEN];
        store_auth_records(
            &mut flash,
            EXPECTED_VERSION,
            EXPECTED_COMPATIBILITY_CHECKSUM,
            &records,
            &mut scratch,
        )
        .unwrap();
        flash.as_mut_slice()[AUTH_HEADER_LEN + 18] ^= 0x01;

        let mut loaded = [record(0); 1];
        let result = load_auth_records(
            &flash,
            EXPECTED_VERSION,
            EXPECTED_COMPATIBILITY_CHECKSUM,
            &mut loaded,
            &mut scratch,
        )
        .unwrap();

        assert!(matches!(
            result.status,
            BleAuthRecordStatus::RecordSetChecksumMismatch { .. }
        ));
        assert!(should_auto_open_pairing_window(true, result.status));
    }

    #[test]
    fn record_crc_mismatch_reopens_pairing_when_set_checksum_matches() {
        let mut flash = InMemoryFlash::<FLASH_LEN, SECTOR_SIZE>::new();
        let mut encoded = [0_u8; AUTH_RECORD_LEN];
        encode_auth_record(record(4), &mut encoded).unwrap();
        encoded[18] ^= 0x01;
        let checksum = records_checksum(&encoded);
        let mut header = [0_u8; AUTH_HEADER_LEN];
        encode_auth_header(
            EXPECTED_VERSION,
            1,
            checksum,
            EXPECTED_COMPATIBILITY_CHECKSUM,
            &mut header,
        )
        .unwrap();
        flash.as_mut_slice()[..AUTH_HEADER_LEN].copy_from_slice(&header);
        flash.as_mut_slice()[AUTH_HEADER_LEN..AUTH_HEADER_LEN + AUTH_RECORD_LEN]
            .copy_from_slice(&encoded);

        let mut loaded = [record(0); 1];
        let mut scratch = [0_u8; AUTH_RECORD_LEN];
        let result = load_auth_records(
            &flash,
            EXPECTED_VERSION,
            EXPECTED_COMPATIBILITY_CHECKSUM,
            &mut loaded,
            &mut scratch,
        )
        .unwrap();

        assert!(matches!(
            result.status,
            BleAuthRecordStatus::RecordCrcMismatch { index: 0, .. }
        ));
        assert!(should_auto_open_pairing_window(true, result.status));
    }

    #[test]
    fn storing_records_erases_and_replaces_old_content() {
        let mut flash = InMemoryFlash::<FLASH_LEN, SECTOR_SIZE>::new();
        let mut scratch = [0_u8; FLASH_LEN];
        store_auth_records(
            &mut flash,
            EXPECTED_VERSION,
            EXPECTED_COMPATIBILITY_CHECKSUM,
            &[record(5), record(6)],
            &mut scratch,
        )
        .unwrap();
        store_auth_records(
            &mut flash,
            EXPECTED_VERSION,
            EXPECTED_COMPATIBILITY_CHECKSUM,
            &[record(7)],
            &mut scratch,
        )
        .unwrap();

        let mut loaded = [record(0); 2];
        let result = load_auth_records(
            &flash,
            EXPECTED_VERSION,
            EXPECTED_COMPATIBILITY_CHECKSUM,
            &mut loaded,
            &mut scratch,
        )
        .unwrap();

        assert_eq!(result.record_count, 1);
        assert_eq!(loaded[0], record(7));
        assert!(flash.as_slice()[AUTH_HEADER_LEN + AUTH_RECORD_LEN] == 0xff);
    }

    #[test]
    fn clear_erases_sector_and_reopens_pairing() {
        let mut flash = InMemoryFlash::<FLASH_LEN, SECTOR_SIZE>::new();
        let mut scratch = [0_u8; FLASH_LEN];
        store_auth_records(
            &mut flash,
            EXPECTED_VERSION,
            EXPECTED_COMPATIBILITY_CHECKSUM,
            &[record(8)],
            &mut scratch,
        )
        .unwrap();
        clear_auth_records(&mut flash).unwrap();

        assert!(flash.as_slice().iter().all(|byte| *byte == 0xff));
        let mut loaded = [record(0); 1];
        let result = load_auth_records(
            &flash,
            EXPECTED_VERSION,
            EXPECTED_COMPATIBILITY_CHECKSUM,
            &mut loaded,
            &mut scratch,
        )
        .unwrap();
        assert_eq!(result.status, BleAuthRecordStatus::Missing);
        assert!(should_auto_open_pairing_window(true, result.status));
    }
}
