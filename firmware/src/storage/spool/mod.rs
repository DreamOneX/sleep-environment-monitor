use crate::{
    storage::flash_model::{FlashError, FlashStorage},
    util::queue::DropOldestQueue,
};

include!("memory.rs");
include!("codec.rs");
include!("flash.rs");

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
    fn flash_entry_buffer_covers_configured_payload_size() {
        assert!(
            encoded_record_len(crate::config::storage::MEASUREMENT_PAYLOAD_SIZE).unwrap()
                <= FLASH_ENTRY_BUFFER_LEN
        );
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
        let truncated_len = RECORD_HEADER_LEN + b"payload".len() - 1;

        assert_eq!(
            decode_record(&out[..truncated_len]),
            Err(SpoolError::BadPayloadLength)
        );
        assert!(decode_record(&out[..len - 1]).is_ok());
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
        let result = spool.append(0, b"three").unwrap();
        let dropped = result.dropped.unwrap();

        assert_eq!(result.dropped_count, 1);
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
    fn flash_backed_spool_recovery_report_counts_recovered_records() {
        let mut flash = InMemoryFlash::<128, 64>::new();
        let mut spool = FlashBackedSpool::<4, 16>::new();

        spool.append(&mut flash, 0, b"one").unwrap();
        spool.append(&mut flash, 0, b"two").unwrap();

        let (recovered, report) = FlashBackedSpool::<4, 16>::recover_with_report(&flash).unwrap();

        assert_eq!(recovered.len(), 2);
        assert_eq!(report.recovered_record_count, 2);
        assert_eq!(report.corrupt_record_count, 0);
    }

    #[test]
    fn flash_backed_spool_recovers_maximum_current_payload_size() {
        let mut flash = InMemoryFlash::<1024, 512>::new();
        let mut spool = FlashBackedSpool::<2, 384>::new();
        let payload = [0x5a_u8; 384];

        spool.append(&mut flash, 0, &payload).unwrap();

        let recovered = FlashBackedSpool::<2, 384>::recover(&flash).unwrap();

        assert_eq!(recovered.len(), 1);
        assert_eq!(recovered.peek().unwrap().payload, payload);
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
    fn flash_backed_spool_recovery_report_counts_corrupt_tail() {
        let mut flash = InMemoryFlash::<128, 64>::new();
        let mut spool = FlashBackedSpool::<4, 16>::new();

        spool.append(&mut flash, 0, b"one").unwrap();
        let partial_offset = encoded_record_len(b"one".len()).unwrap();
        flash
            .write(partial_offset, &RECORD_MAGIC.to_le_bytes())
            .unwrap();

        let (recovered, report) = FlashBackedSpool::<4, 16>::recover_with_report(&flash).unwrap();

        assert_eq!(recovered.len(), 1);
        assert_eq!(report.recovered_record_count, 1);
        assert!(report.corrupt_record_count > 0);
    }

    #[test]
    fn flash_backed_spool_drops_oldest_when_modeled_flash_fills() {
        let mut flash = InMemoryFlash::<64, 64>::new();
        let mut spool = FlashBackedSpool::<4, 16>::new();

        spool.append(&mut flash, 0, b"aaaa").unwrap();
        spool.append(&mut flash, 0, b"bbbb").unwrap();
        let result = spool.append(&mut flash, 0, b"cccc").unwrap();

        assert_eq!(result.sequence, 2);
        assert_eq!(result.dropped_count, 1);
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
