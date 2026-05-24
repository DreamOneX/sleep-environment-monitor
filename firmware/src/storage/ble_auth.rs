use crate::storage::spool::crc32;

pub const AUTH_HEADER_MAGIC: u32 = 0x5345_4241;
pub const AUTH_HEADER_FORMAT_VERSION: u16 = 1;
pub const AUTH_HEADER_LEN: usize = 24;

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
    RecordsChecksumMismatch {
        stored: u32,
        expected: u32,
    },
    NoRecords,
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
pub enum BleAuthHeaderError {
    BufferTooSmall,
}

pub fn encode_auth_header(
    records_version: u32,
    record_count: u16,
    records_checksum: u32,
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

    let header_checksum = crc32(&out[..20]);
    out[20..24].copy_from_slice(&header_checksum.to_le_bytes());
    Ok(AUTH_HEADER_LEN)
}

pub fn inspect_auth_header(
    input: &[u8],
    expected_records_version: u32,
    expected_records_checksum: u32,
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

    let stored_checksum = u32::from_le_bytes([input[20], input[21], input[22], input[23]]);
    let computed_checksum = crc32(&input[..20]);
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

    let records_checksum = u32::from_le_bytes([input[16], input[17], input[18], input[19]]);
    if records_checksum != expected_records_checksum {
        return BleAuthRecordStatus::RecordsChecksumMismatch {
            stored: records_checksum,
            expected: expected_records_checksum,
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

pub const fn should_auto_open_pairing_window(enabled: bool, status: BleAuthRecordStatus) -> bool {
    enabled && status.requires_pairing_window()
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXPECTED_VERSION: u32 = 7;
    const EXPECTED_CHECKSUM: u32 = 0x1234_5678;

    fn valid_header(
        records_version: u32,
        record_count: u16,
        records_checksum: u32,
    ) -> [u8; AUTH_HEADER_LEN] {
        let mut out = [0_u8; AUTH_HEADER_LEN];
        encode_auth_header(records_version, record_count, records_checksum, &mut out).unwrap();
        out
    }

    #[test]
    fn encode_reports_small_buffer() {
        let mut out = [0_u8; AUTH_HEADER_LEN - 1];

        assert_eq!(
            encode_auth_header(EXPECTED_VERSION, 1, 0, &mut out),
            Err(BleAuthHeaderError::BufferTooSmall)
        );
    }

    #[test]
    fn erased_flash_requires_pairing_window() {
        let header = [0xff_u8; AUTH_HEADER_LEN];
        let status = inspect_auth_header(&header, EXPECTED_VERSION, EXPECTED_CHECKSUM);

        assert_eq!(status, BleAuthRecordStatus::Missing);
        assert!(should_auto_open_pairing_window(true, status));
    }

    #[test]
    fn truncated_erased_header_reports_bad_length() {
        let header = [0xff_u8; AUTH_HEADER_LEN - 1];
        let status = inspect_auth_header(&header, EXPECTED_VERSION, EXPECTED_CHECKSUM);

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
        let header = valid_header(EXPECTED_VERSION, 0, EXPECTED_CHECKSUM);
        let status = inspect_auth_header(&header, EXPECTED_VERSION, EXPECTED_CHECKSUM);

        assert_eq!(status, BleAuthRecordStatus::NoRecords);
        assert!(should_auto_open_pairing_window(true, status));
    }

    #[test]
    fn valid_current_version_record_keeps_pairing_window_closed() {
        let header = valid_header(EXPECTED_VERSION, 1, EXPECTED_CHECKSUM);
        let status = inspect_auth_header(&header, EXPECTED_VERSION, EXPECTED_CHECKSUM);

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
        let header = valid_header(EXPECTED_VERSION - 1, 2, EXPECTED_CHECKSUM);
        let status = inspect_auth_header(&header, EXPECTED_VERSION, EXPECTED_CHECKSUM);

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
    fn records_checksum_change_requires_pairing_window_even_with_records() {
        let header = valid_header(EXPECTED_VERSION, 2, EXPECTED_CHECKSUM ^ 0x01);
        let status = inspect_auth_header(&header, EXPECTED_VERSION, EXPECTED_CHECKSUM);

        assert_eq!(
            status,
            BleAuthRecordStatus::RecordsChecksumMismatch {
                stored: EXPECTED_CHECKSUM ^ 0x01,
                expected: EXPECTED_CHECKSUM
            }
        );
        assert!(should_auto_open_pairing_window(true, status));
    }

    #[test]
    fn checksum_mismatch_requires_pairing_window_even_with_records() {
        let mut header = valid_header(EXPECTED_VERSION, 2, EXPECTED_CHECKSUM);
        header[16] ^= 0x01;
        let status = inspect_auth_header(&header, EXPECTED_VERSION, EXPECTED_CHECKSUM);

        assert!(matches!(
            status,
            BleAuthRecordStatus::ChecksumMismatch { .. }
        ));
        assert!(should_auto_open_pairing_window(true, status));
    }

    #[test]
    fn config_switch_can_disable_auto_pairing_window() {
        let header = [0xff_u8; AUTH_HEADER_LEN];
        let status = inspect_auth_header(&header, EXPECTED_VERSION, EXPECTED_CHECKSUM);

        assert!(!should_auto_open_pairing_window(false, status));
    }
}
