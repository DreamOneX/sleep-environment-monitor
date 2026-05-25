use crate::{
    config,
    storage::{
        flash_model::FlashStorage,
        spool::{FlashBackedSpool, SpoolError},
    },
    tasks::upload::{EncodeError, measurement_to_json_fields},
    types::{ErrorFlags, Measurement},
};

include!("types.rs");
include!("backlog.rs");
include!("protocol.rs");
include!("runtime.rs");

#[cfg(test)]
mod tests {
    use crate::{
        storage::flash_model::{FlashError, FlashStorage, InMemoryFlash},
        types::ErrorFlags,
    };

    use super::*;

    fn measurement(uptime_ms: u64) -> Measurement {
        Measurement {
            uptime_ms,
            temperature_c: Some(21.5),
            humidity_percent: Some(45.25),
            lux: Some(9.75),
            mic_mean: 2048.0,
            mic_rms: 10.5,
            mic_peak: 99.0,
            mic_db_rel: 20.4,
            mic_clip_count: 2,
            error_flags: ErrorFlags::SHT40 | ErrorFlags::UPLOAD,
        }
    }

    fn payload_str<const N: usize>(payload: &StoredPayload<N>) -> &str {
        core::str::from_utf8(payload.as_slice()).unwrap()
    }

    fn assert_payload_uptime<const N: usize>(payload: &StoredPayload<N>, uptime_ms: u64) {
        assert!(
            payload_str(payload).contains(&format!("\"uptime_ms\":{uptime_ms}")),
            "payload did not contain expected uptime: {}",
            payload_str(payload)
        );
    }

    #[test]
    fn storage_task_model_appends_measurements_in_order() {
        let mut flash = InMemoryFlash::<512, 128>::new();
        let mut backlog = StorageBacklog::<4, 192>::new();

        backlog
            .append_measurement(&mut flash, measurement(1))
            .unwrap();
        backlog
            .append_measurement(&mut flash, measurement(2))
            .unwrap();

        assert_payload_uptime(&backlog.peek_payload().unwrap(), 1);
        assert_eq!(
            backlog.peek_payload().unwrap().payload_flags,
            PAYLOAD_FLAG_JSON_FIELDS
        );
        backlog.acknowledge(&mut flash).unwrap();
        assert_payload_uptime(&backlog.peek_payload().unwrap(), 2);
    }

    #[test]
    fn upload_success_acknowledges_exactly_one_record() {
        let mut flash = InMemoryFlash::<512, 128>::new();
        let mut backlog = StorageBacklog::<4, 192>::new();

        backlog
            .append_measurement(&mut flash, measurement(10))
            .unwrap();
        backlog
            .append_measurement(&mut flash, measurement(20))
            .unwrap();

        let acknowledged = backlog.acknowledge(&mut flash).unwrap().unwrap();

        assert_payload_uptime(&acknowledged, 10);
        assert_eq!(backlog.len(), 1);
        assert_payload_uptime(&backlog.peek_payload().unwrap(), 20);
    }

    #[test]
    fn sequence_ack_removes_matching_oldest_record() {
        let mut flash = InMemoryFlash::<512, 128>::new();
        let mut backlog = StorageBacklog::<4, 192>::new();

        backlog
            .append_measurement(&mut flash, measurement(10))
            .unwrap();
        backlog
            .append_measurement(&mut flash, measurement(20))
            .unwrap();
        let oldest = backlog.peek_payload().unwrap();

        let acknowledged = backlog
            .acknowledge_sequence(&mut flash, oldest.sequence)
            .unwrap()
            .unwrap();

        assert_payload_uptime(&acknowledged, 10);
        assert_eq!(backlog.len(), 1);
        assert_payload_uptime(&backlog.peek_payload().unwrap(), 20);
    }

    #[test]
    fn stale_sequence_ack_does_not_remove_new_oldest_record() {
        let mut flash = InMemoryFlash::<512, 128>::new();
        let mut backlog = StorageBacklog::<4, 192>::new();

        backlog
            .append_measurement(&mut flash, measurement(10))
            .unwrap();
        backlog
            .append_measurement(&mut flash, measurement(20))
            .unwrap();
        let stale_sequence = backlog.peek_payload().unwrap().sequence;
        backlog
            .acknowledge_sequence(&mut flash, stale_sequence)
            .unwrap();

        let not_acknowledged = backlog
            .acknowledge_sequence(&mut flash, stale_sequence)
            .unwrap();

        assert_eq!(not_acknowledged, None);
        assert_eq!(backlog.len(), 1);
        assert_payload_uptime(&backlog.peek_payload().unwrap(), 20);
    }

    #[test]
    fn ble_ack_after_wifi_ack_does_not_remove_next_oldest_record() {
        let mut flash = InMemoryFlash::<512, 128>::new();
        let mut backlog = StorageBacklog::<4, 192>::new();

        backlog
            .append_measurement(&mut flash, measurement(10))
            .unwrap();
        backlog
            .append_measurement(&mut flash, measurement(20))
            .unwrap();

        let raced_sequence = backlog.peek_payload().unwrap().sequence;
        let wifi_ack = backlog
            .acknowledge_sequence(&mut flash, raced_sequence)
            .unwrap()
            .unwrap();
        let ble_ack = backlog
            .acknowledge_sequence(&mut flash, raced_sequence)
            .unwrap();

        assert_payload_uptime(&wifi_ack, 10);
        assert_eq!(ble_ack, None);
        assert_eq!(backlog.len(), 1);
        assert_payload_uptime(&backlog.peek_payload().unwrap(), 20);
    }

    #[test]
    fn upload_failure_preserves_record() {
        let mut flash = InMemoryFlash::<512, 128>::new();
        let mut backlog = StorageBacklog::<4, 192>::new();

        backlog
            .append_measurement(&mut flash, measurement(30))
            .unwrap();
        let before = backlog.peek_payload().unwrap();

        assert_eq!(
            payload_str(&before),
            payload_str(&backlog.peek_payload().unwrap())
        );
        assert_eq!(backlog.len(), 1);
    }

    #[test]
    fn recovered_records_upload_before_newly_appended_records() {
        let mut flash = InMemoryFlash::<512, 128>::new();
        {
            let mut backlog = StorageBacklog::<4, 192>::new();
            backlog
                .append_measurement(&mut flash, measurement(40))
                .unwrap();
        }

        let mut recovered = StorageBacklog::<4, 192>::recover(&mut flash).unwrap();
        recovered
            .append_measurement(&mut flash, measurement(50))
            .unwrap();

        let recovered_payload = recovered.peek_payload().unwrap();
        assert_payload_uptime(&recovered_payload, 40);
        assert!(!recovered_payload.current_boot);
        recovered.acknowledge(&mut flash).unwrap();
        let current_boot_payload = recovered.peek_payload().unwrap();
        assert_payload_uptime(&current_boot_payload, 50);
        assert!(current_boot_payload.current_boot);
    }

    #[test]
    fn recovery_metrics_report_pending_and_recovered_records() {
        let mut flash = InMemoryFlash::<512, 128>::new();
        {
            let mut backlog = StorageBacklog::<4, 192>::new();
            backlog
                .append_measurement(&mut flash, measurement(40))
                .unwrap();
            backlog
                .append_measurement(&mut flash, measurement(50))
                .unwrap();
        }

        let recovered = StorageBacklog::<4, 192>::recover(&mut flash).unwrap();

        assert_eq!(recovered.metrics().pending_record_count, 2);
        assert_eq!(recovered.metrics().recovered_record_count, 2);
        assert_eq!(recovered.metrics().skipped_legacy_record_count, 0);
        assert_eq!(recovered.metrics().corrupt_record_count, 0);
        assert_eq!(recovered.metrics().last_error, None);
    }

    #[test]
    fn recovery_skips_legacy_unflagged_payloads() {
        let mut flash = InMemoryFlash::<512, 128>::new();
        let mut spool = FlashBackedSpool::<4, 192>::new();
        spool.append(&mut flash, 0, b"1,2,3,4").unwrap();
        spool
            .append(
                &mut flash,
                PAYLOAD_FLAG_JSON_FIELDS,
                br#""uptime_ms":20,"temperature_c":null"#,
            )
            .unwrap();

        let recovered = StorageBacklog::<4, 192>::recover(&mut flash).unwrap();

        assert_eq!(recovered.metrics().pending_record_count, 1);
        assert_eq!(recovered.metrics().recovered_record_count, 2);
        assert_eq!(recovered.metrics().skipped_legacy_record_count, 1);
        assert_payload_uptime(&recovered.peek_payload().unwrap(), 20);
    }

    #[test]
    fn full_persistent_spool_drops_oldest_record() {
        let mut flash = InMemoryFlash::<512, 128>::new();
        let mut backlog = StorageBacklog::<2, 192>::new();

        backlog
            .append_measurement(&mut flash, measurement(1))
            .unwrap();
        backlog
            .append_measurement(&mut flash, measurement(2))
            .unwrap();
        backlog
            .append_measurement(&mut flash, measurement(3))
            .unwrap();

        assert_eq!(backlog.len(), 2);
        assert_payload_uptime(&backlog.peek_payload().unwrap(), 2);
    }

    #[test]
    fn full_spool_metrics_count_dropped_oldest_records() {
        let mut flash = InMemoryFlash::<512, 128>::new();
        let mut backlog = StorageBacklog::<2, 192>::new();

        backlog
            .append_measurement(&mut flash, measurement(1))
            .unwrap();
        backlog
            .append_measurement(&mut flash, measurement(2))
            .unwrap();
        backlog
            .append_measurement(&mut flash, measurement(3))
            .unwrap();

        assert_eq!(backlog.metrics().pending_record_count, 2);
        assert_eq!(backlog.metrics().dropped_oldest_count, 1);
        assert_eq!(backlog.metrics().last_error, None);
    }

    #[test]
    fn ack_metrics_update_pending_record_count() {
        let mut flash = InMemoryFlash::<512, 128>::new();
        let mut backlog = StorageBacklog::<4, 192>::new();

        backlog
            .append_measurement(&mut flash, measurement(10))
            .unwrap();
        backlog
            .append_measurement(&mut flash, measurement(20))
            .unwrap();
        assert_eq!(backlog.metrics().pending_record_count, 2);

        backlog.acknowledge(&mut flash).unwrap();

        assert_eq!(backlog.metrics().pending_record_count, 1);
        assert_eq!(backlog.metrics().last_error, None);
    }

    #[test]
    fn storage_error_sets_error_status() {
        let mut flash = FailingFlash::new();
        let mut backlog = StorageBacklog::<2, 192>::new();

        assert_eq!(
            backlog.append_measurement(&mut flash, measurement(1)),
            Err(StorageError::Spool(SpoolError::Flash(
                FlashError::WriteRequiresErase
            )))
        );

        assert!(backlog.has_error());
        assert!(backlog.error_flags().contains(ErrorFlags::STORAGE));
    }

    #[test]
    fn storage_error_metrics_record_last_error_and_preserve_pending_count() {
        let mut flash = FailingFlash::new();
        let mut backlog = StorageBacklog::<2, 192>::new();

        assert_eq!(
            backlog.append_measurement(&mut flash, measurement(1)),
            Err(StorageError::Spool(SpoolError::Flash(
                FlashError::WriteRequiresErase
            )))
        );

        assert_eq!(backlog.metrics().pending_record_count, 0);
        assert_eq!(
            backlog.metrics().last_error,
            Some(StorageError::Spool(SpoolError::Flash(
                FlashError::WriteRequiresErase
            )))
        );
    }

    struct FailingFlash;

    impl FailingFlash {
        const fn new() -> Self {
            Self
        }
    }

    impl FlashStorage for FailingFlash {
        fn len(&self) -> usize {
            512
        }

        fn sector_size(&self) -> usize {
            128
        }

        fn read(&self, _offset: usize, out: &mut [u8]) -> Result<(), FlashError> {
            out.fill(0xff);
            Ok(())
        }

        fn write(&mut self, _offset: usize, _data: &[u8]) -> Result<(), FlashError> {
            Err(FlashError::WriteRequiresErase)
        }

        fn erase(&mut self, _offset: usize, _len: usize) -> Result<(), FlashError> {
            Ok(())
        }
    }
}
