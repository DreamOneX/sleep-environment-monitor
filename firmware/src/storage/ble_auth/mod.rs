use crate::storage::{
    flash_model::{FlashError, FlashStorage},
    spool::crc32,
};

include!("types.rs");
include!("upsert.rs");
include!("header.rs");
include!("record.rs");
include!("flash.rs");

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
    fn upsert_updates_existing_record_by_identity_address() {
        let original = record(0x30);
        let mut replacement = record(0x40);
        replacement.identity_address = original.identity_address;
        let mut records = [record(0x20), original, record(0x50)];

        let (count, action) = upsert_auth_record(&mut records, 2, replacement);

        assert_eq!(count, 2);
        assert_eq!(action, BleAuthRecordUpsert::Updated { index: 1 });
        assert_eq!(records[0], record(0x20));
        assert_eq!(records[1], replacement);
        assert_eq!(records[2], record(0x50));
    }

    #[test]
    fn upsert_updates_existing_record_by_identity_resolving_key() {
        let original = record(0x60);
        let mut replacement = record(0x70);
        replacement.identity_resolving_key = original.identity_resolving_key;
        let mut records = [original, record(0x61)];

        let (count, action) = upsert_auth_record(&mut records, 2, replacement);

        assert_eq!(count, 2);
        assert_eq!(action, BleAuthRecordUpsert::Updated { index: 0 });
        assert_eq!(records[0], replacement);
        assert_eq!(records[1], record(0x61));
    }

    #[test]
    fn upsert_appends_new_record_when_capacity_remains() {
        let mut records = [record(0x80), BleAuthRecord::EMPTY, BleAuthRecord::EMPTY];
        let candidate = record(0x81);

        let (count, action) = upsert_auth_record(&mut records, 1, candidate);

        assert_eq!(count, 2);
        assert_eq!(action, BleAuthRecordUpsert::Appended { index: 1 });
        assert_eq!(records[0], record(0x80));
        assert_eq!(records[1], candidate);
        assert_eq!(records[2], BleAuthRecord::EMPTY);
    }

    #[test]
    fn upsert_replaces_oldest_record_when_capacity_is_full() {
        let mut records = [record(0x90), record(0x91)];
        let candidate = record(0x92);

        let (count, action) = upsert_auth_record(&mut records, 2, candidate);

        assert_eq!(count, 2);
        assert_eq!(action, BleAuthRecordUpsert::ReplacedOldest { index: 0 });
        assert_eq!(records[0], candidate);
        assert_eq!(records[1], record(0x91));
    }

    #[test]
    fn upsert_clamps_record_count_to_capacity_before_replacement() {
        let mut records = [record(0xa0), record(0xa1)];
        let candidate = record(0xa2);

        let (count, action) = upsert_auth_record(&mut records, usize::MAX, candidate);

        assert_eq!(count, 2);
        assert_eq!(action, BleAuthRecordUpsert::ReplacedOldest { index: 0 });
        assert_eq!(records[0], candidate);
        assert_eq!(records[1], record(0xa1));
    }

    #[test]
    fn upsert_reports_no_capacity_for_empty_record_slice() {
        let mut records = [];

        let (count, action) = upsert_auth_record(&mut records, 0, record(0xb0));

        assert_eq!(count, 0);
        assert_eq!(action, BleAuthRecordUpsert::NoCapacity);
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
