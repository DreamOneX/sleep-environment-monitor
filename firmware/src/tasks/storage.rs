use crate::{
    config,
    storage::{
        flash_model::FlashStorage,
        spool::{FlashBackedSpool, SpoolError},
    },
    tasks::upload::{EncodeError, measurement_to_json_fields},
    types::{ErrorFlags, Measurement},
};

pub const MEASUREMENT_PAYLOAD_SIZE: usize = config::storage::MEASUREMENT_PAYLOAD_SIZE;
pub const PERSISTENT_SPOOL_CAPACITY: usize = config::storage::PERSISTENT_SPOOL_CAPACITY;
pub const PAYLOAD_FLAG_JSON_FIELDS: u8 = 0x01;
pub type MeasurementStorageBacklog =
    StorageBacklog<PERSISTENT_SPOOL_CAPACITY, MEASUREMENT_PAYLOAD_SIZE>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(target_arch = "riscv32", derive(defmt::Format))]
pub enum StorageError {
    Encode,
    Spool(SpoolError),
}

impl From<EncodeError> for StorageError {
    fn from(_: EncodeError) -> Self {
        Self::Encode
    }
}

impl From<SpoolError> for StorageError {
    fn from(error: SpoolError) -> Self {
        Self::Spool(error)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[cfg_attr(target_arch = "riscv32", derive(defmt::Format))]
pub struct StorageMetrics {
    pub pending_record_count: usize,
    pub dropped_oldest_count: usize,
    pub recovered_record_count: usize,
    pub skipped_legacy_record_count: usize,
    pub corrupt_record_count: usize,
    pub last_error: Option<StorageError>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StoredPayload<const PAYLOAD_SIZE: usize = MEASUREMENT_PAYLOAD_SIZE> {
    pub sequence: u64,
    pub payload: [u8; PAYLOAD_SIZE],
    pub payload_len: usize,
    pub payload_flags: u8,
    pub current_boot: bool,
}

impl<const PAYLOAD_SIZE: usize> StoredPayload<PAYLOAD_SIZE> {
    fn from_record(record: crate::storage::spool::SpoolRecord<'_>, current_boot: bool) -> Self {
        let mut payload = [0_u8; PAYLOAD_SIZE];
        let payload_len = record.payload.len().min(PAYLOAD_SIZE);
        payload[..payload_len].copy_from_slice(&record.payload[..payload_len]);

        Self {
            sequence: record.sequence,
            payload,
            payload_len,
            payload_flags: record.flags,
            current_boot,
        }
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.payload[..self.payload_len]
    }
}

pub struct StorageBacklog<
    const CAPACITY: usize = PERSISTENT_SPOOL_CAPACITY,
    const PAYLOAD_SIZE: usize = MEASUREMENT_PAYLOAD_SIZE,
> {
    spool: FlashBackedSpool<CAPACITY, PAYLOAD_SIZE>,
    metrics: StorageMetrics,
    current_boot_first_sequence: u64,
}

impl<const CAPACITY: usize, const PAYLOAD_SIZE: usize> StorageBacklog<CAPACITY, PAYLOAD_SIZE> {
    pub const fn new() -> Self {
        Self {
            spool: FlashBackedSpool::new(),
            metrics: StorageMetrics {
                pending_record_count: 0,
                dropped_oldest_count: 0,
                recovered_record_count: 0,
                skipped_legacy_record_count: 0,
                corrupt_record_count: 0,
                last_error: None,
            },
            current_boot_first_sequence: 0,
        }
    }

    pub fn recover(flash: &mut impl FlashStorage) -> Result<Self, StorageError> {
        let (spool, report) = FlashBackedSpool::recover_with_report(flash)?;
        let metrics = StorageMetrics {
            pending_record_count: spool.len(),
            dropped_oldest_count: 0,
            recovered_record_count: report.recovered_record_count,
            skipped_legacy_record_count: 0,
            corrupt_record_count: report.corrupt_record_count,
            last_error: None,
        };

        let current_boot_first_sequence = spool.next_sequence();
        let mut backlog = Self {
            spool,
            metrics,
            current_boot_first_sequence,
        };
        backlog.metrics.skipped_legacy_record_count =
            backlog.discard_unsupported_front_payloads(flash)?;
        backlog.refresh_pending_count();

        Ok(backlog)
    }

    pub fn append_measurement(
        &mut self,
        flash: &mut impl FlashStorage,
        measurement: Measurement,
    ) -> Result<(), StorageError> {
        let mut payload = [0_u8; PAYLOAD_SIZE];
        let payload_len = match measurement_to_json_fields(&measurement, &mut payload) {
            Ok(len) => len,
            Err(error) => {
                let error = StorageError::from(error);
                self.set_error(error);
                return Err(error);
            }
        };

        match self
            .spool
            .append(flash, PAYLOAD_FLAG_JSON_FIELDS, &payload[..payload_len])
        {
            Ok(result) => {
                self.metrics.dropped_oldest_count = self
                    .metrics
                    .dropped_oldest_count
                    .saturating_add(result.dropped_count);
                self.refresh_pending_count();
                self.clear_error();
                Ok(())
            }
            Err(error) => {
                let error = StorageError::Spool(error);
                self.set_error(error);
                Err(error)
            }
        }
    }

    pub fn peek_payload(&self) -> Option<StoredPayload<PAYLOAD_SIZE>> {
        self.spool.peek().map(|record| {
            StoredPayload::from_record(record, self.sequence_is_from_current_boot(record.sequence))
        })
    }

    pub fn acknowledge(
        &mut self,
        flash: &mut impl FlashStorage,
    ) -> Result<Option<StoredPayload<PAYLOAD_SIZE>>, StorageError> {
        match self.spool.acknowledge(flash) {
            Ok(record) => {
                self.refresh_pending_count();
                self.clear_error();
                Ok(record.map(|record| {
                    StoredPayload::from_record(
                        record.as_record(),
                        self.sequence_is_from_current_boot(record.sequence),
                    )
                }))
            }
            Err(error) => {
                let error = StorageError::Spool(error);
                self.set_error(error);
                Err(error)
            }
        }
    }

    pub fn acknowledge_sequence(
        &mut self,
        flash: &mut impl FlashStorage,
        sequence: u64,
    ) -> Result<Option<StoredPayload<PAYLOAD_SIZE>>, StorageError> {
        match self.spool.peek() {
            Some(record) if record.sequence == sequence => self.acknowledge(flash),
            Some(_) | None => Ok(None),
        }
    }

    pub fn len(&self) -> usize {
        self.spool.len()
    }

    pub fn is_empty(&self) -> bool {
        self.spool.is_empty()
    }

    pub const fn metrics(&self) -> StorageMetrics {
        self.metrics
    }

    pub fn has_error(&self) -> bool {
        self.metrics.last_error.is_some()
    }

    pub fn error_flags(&self) -> ErrorFlags {
        if self.has_error() {
            ErrorFlags::STORAGE
        } else {
            ErrorFlags::NONE
        }
    }

    fn refresh_pending_count(&mut self) {
        self.metrics.pending_record_count = self.spool.len();
    }

    fn clear_error(&mut self) {
        self.metrics.last_error = None;
    }

    fn set_error(&mut self, error: StorageError) {
        self.metrics.last_error = Some(error);
    }

    fn sequence_is_from_current_boot(&self, sequence: u64) -> bool {
        sequence.wrapping_sub(self.current_boot_first_sequence) < (u64::MAX / 2)
    }

    fn discard_unsupported_front_payloads(
        &mut self,
        flash: &mut impl FlashStorage,
    ) -> Result<usize, StorageError> {
        let mut skipped = 0;

        while let Some(record) = self.spool.peek() {
            if payload_flags_are_supported(record.flags) {
                break;
            }

            match self.spool.acknowledge(flash) {
                Ok(Some(_)) => {
                    skipped += 1;
                }
                Ok(None) => break,
                Err(error) => {
                    let error = StorageError::Spool(error);
                    self.set_error(error);
                    return Err(error);
                }
            }
        }

        Ok(skipped)
    }
}

const fn payload_flags_are_supported(flags: u8) -> bool {
    flags & PAYLOAD_FLAG_JSON_FIELDS == PAYLOAD_FLAG_JSON_FIELDS
}

impl<const CAPACITY: usize, const PAYLOAD_SIZE: usize> Default
    for StorageBacklog<CAPACITY, PAYLOAD_SIZE>
{
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(target_arch = "riscv32")]
#[derive(Clone, Copy)]
#[cfg_attr(target_arch = "riscv32", derive(defmt::Format))]
pub enum StorageClient {
    Wifi,
    Ble,
}

#[cfg(target_arch = "riscv32")]
#[derive(Clone, Copy)]
pub enum StorageCommand {
    Append(Measurement),
    Peek(StorageClient),
    Ack {
        client: StorageClient,
        sequence: u64,
    },
}

#[cfg(target_arch = "riscv32")]
#[derive(Clone, Copy)]
#[allow(
    clippy::large_enum_variant,
    reason = "StorageResponse crosses an Embassy Signal with a fixed payload buffer; boxing would require heap allocation on the target."
)]
pub enum StorageResponse {
    Peeked(Option<StoredPayload>),
    Acked(bool),
    Error(StorageError),
}

#[cfg(target_arch = "riscv32")]
use crate::{
    drivers::flash::RomSpoolFlash,
    storage::flash_model::FlashError,
    tasks::{
        FirmwareStatusSnapshotMutex, StorageRequestChannel, StorageResponseSignal, TaskSignal,
    },
};
#[cfg(target_arch = "riscv32")]
use defmt::{info, warn};

#[cfg(target_arch = "riscv32")]
const STORAGE_METRICS_LOG_EVERY_EVENTS: u32 = config::storage::METRICS_LOG_EVERY_EVENTS;

#[cfg(target_arch = "riscv32")]
#[embassy_executor::task]
pub async fn storage_task(
    requests: &'static StorageRequestChannel,
    wifi_responses: &'static StorageResponseSignal,
    ble_responses: &'static StorageResponseSignal,
    error_flags: &'static TaskSignal<ErrorFlags>,
    firmware_status: &'static FirmwareStatusSnapshotMutex,
) {
    let mut flash = match RomSpoolFlash::new() {
        Ok(flash) => flash,
        Err(error) => {
            warn!("storage flash init failed error={:?}", error);
            publish_storage_error(error_flags, firmware_status).await;
            loop {
                match requests.receive().await {
                    StorageCommand::Append(_) => {
                        publish_storage_error(error_flags, firmware_status).await;
                    }
                    StorageCommand::Peek(client) | StorageCommand::Ack { client, .. } => {
                        signal_storage_response(
                            wifi_responses,
                            ble_responses,
                            client,
                            StorageResponse::Error(StorageError::Spool(SpoolError::Flash(
                                FlashError::OutOfBounds,
                            ))),
                        );
                    }
                }
            }
        }
    };

    info!(
        "storage spool flash range offset=0x{:08x} len={=usize}",
        flash.absolute_offset(),
        flash.len()
    );

    let mut storage_event_count = 0_u32;
    let (mut backlog, storage_unavailable_error) =
        match MeasurementStorageBacklog::recover(&mut flash) {
            Ok(backlog) => {
                info!("storage recovered pending_len={=usize}", backlog.len());
                log_storage_metrics(backlog.metrics(), true, storage_event_count);
                publish_pending_record_count(firmware_status, backlog.metrics()).await;
                (Some(backlog), None)
            }
            Err(error) => {
                warn!("storage recovery failed error={:?}", error);
                publish_storage_error(error_flags, firmware_status).await;
                (None, Some(error))
            }
        };

    loop {
        match requests.receive().await {
            StorageCommand::Append(measurement) => {
                let Some(backlog) = backlog.as_mut() else {
                    publish_storage_error(error_flags, firmware_status).await;
                    continue;
                };

                let dropped_oldest_before = backlog.metrics().dropped_oldest_count;
                match backlog.append_measurement(&mut flash, measurement) {
                    Ok(()) => {
                        storage_event_count = storage_event_count.wrapping_add(1);
                        let metrics = backlog.metrics();
                        log_storage_metrics(
                            metrics,
                            dropped_oldest_before == 0 && metrics.dropped_oldest_count > 0,
                            storage_event_count,
                        );
                        publish_pending_record_count(firmware_status, metrics).await;
                    }
                    Err(error) => {
                        warn!("storage append failed error={:?}", error);
                        storage_event_count = storage_event_count.wrapping_add(1);
                        let metrics = backlog.metrics();
                        log_storage_metrics(metrics, true, storage_event_count);
                        publish_pending_record_count(firmware_status, metrics).await;
                        publish_storage_error(error_flags, firmware_status).await;
                    }
                }
            }
            StorageCommand::Peek(client) => {
                let response = match backlog.as_ref() {
                    Some(backlog) => StorageResponse::Peeked(backlog.peek_payload()),
                    None => StorageResponse::Error(storage_unavailable_error.unwrap_or(
                        StorageError::Spool(SpoolError::Flash(FlashError::OutOfBounds)),
                    )),
                };
                signal_storage_response(wifi_responses, ble_responses, client, response);
            }
            StorageCommand::Ack { client, sequence } => {
                let Some(backlog) = backlog.as_mut() else {
                    signal_storage_response(
                        wifi_responses,
                        ble_responses,
                        client,
                        StorageResponse::Error(storage_unavailable_error.unwrap_or(
                            StorageError::Spool(SpoolError::Flash(FlashError::OutOfBounds)),
                        )),
                    );
                    publish_storage_error(error_flags, firmware_status).await;
                    continue;
                };

                match backlog.acknowledge_sequence(&mut flash, sequence) {
                    Ok(acknowledged) => {
                        storage_event_count = storage_event_count.wrapping_add(1);
                        let metrics = backlog.metrics();
                        log_storage_metrics(metrics, false, storage_event_count);
                        publish_pending_record_count(firmware_status, metrics).await;
                        signal_storage_response(
                            wifi_responses,
                            ble_responses,
                            client,
                            StorageResponse::Acked(acknowledged.is_some()),
                        );
                    }
                    Err(error) => {
                        warn!("storage ack failed error={:?}", error);
                        storage_event_count = storage_event_count.wrapping_add(1);
                        log_storage_metrics(backlog.metrics(), true, storage_event_count);
                        signal_storage_response(
                            wifi_responses,
                            ble_responses,
                            client,
                            StorageResponse::Error(error),
                        );
                        publish_storage_error(error_flags, firmware_status).await;
                    }
                }
            }
        }
    }
}

#[cfg(target_arch = "riscv32")]
async fn publish_pending_record_count(
    firmware_status: &'static FirmwareStatusSnapshotMutex,
    metrics: StorageMetrics,
) {
    let pending_record_count = u16::try_from(metrics.pending_record_count).unwrap_or(u16::MAX);
    let mut status = firmware_status.lock().await;
    let current = *status;
    *status = current.with_pending_record_count(pending_record_count);
}

#[cfg(target_arch = "riscv32")]
async fn publish_storage_error(
    error_flags: &'static TaskSignal<ErrorFlags>,
    firmware_status: &'static FirmwareStatusSnapshotMutex,
) {
    error_flags.signal(ErrorFlags::STORAGE);
    let mut status = firmware_status.lock().await;
    let current = *status;
    *status = current.with_error_flags(current.error_flags | ErrorFlags::STORAGE);
}

#[cfg(target_arch = "riscv32")]
fn signal_storage_response(
    wifi_responses: &'static StorageResponseSignal,
    ble_responses: &'static StorageResponseSignal,
    client: StorageClient,
    response: StorageResponse,
) {
    match client {
        StorageClient::Wifi => wifi_responses.signal(response),
        StorageClient::Ble => ble_responses.signal(response),
    }
}

#[cfg(target_arch = "riscv32")]
fn log_storage_metrics(metrics: StorageMetrics, force: bool, event_count: u32) {
    if !force
        && event_count != 1
        && !event_count.is_multiple_of(STORAGE_METRICS_LOG_EVERY_EVENTS)
        && metrics.last_error.is_none()
    {
        return;
    }

    match metrics.last_error {
        Some(error) => info!(
            "storage metrics pending={=usize} recovered={=usize} dropped_oldest={=usize} skipped_legacy={=usize} corrupt={=usize} last_error={:?}",
            metrics.pending_record_count,
            metrics.recovered_record_count,
            metrics.dropped_oldest_count,
            metrics.skipped_legacy_record_count,
            metrics.corrupt_record_count,
            error
        ),
        None => info!(
            "storage metrics pending={=usize} recovered={=usize} dropped_oldest={=usize} skipped_legacy={=usize} corrupt={=usize} last_error=none",
            metrics.pending_record_count,
            metrics.recovered_record_count,
            metrics.dropped_oldest_count,
            metrics.skipped_legacy_record_count,
            metrics.corrupt_record_count
        ),
    }
}

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
