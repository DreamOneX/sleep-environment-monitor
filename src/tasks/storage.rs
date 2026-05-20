use crate::{
    storage::{
        flash_model::FlashStorage,
        spool::{FlashBackedSpool, SpoolError},
    },
    tasks::upload::{EncodeError, measurement_to_csv_line},
    types::{ErrorFlags, Measurement},
};

pub const MEASUREMENT_PAYLOAD_SIZE: usize = 192;
pub const PERSISTENT_SPOOL_CAPACITY: usize = 32;
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StoredPayload<const PAYLOAD_SIZE: usize = MEASUREMENT_PAYLOAD_SIZE> {
    pub sequence: u64,
    pub payload: [u8; PAYLOAD_SIZE],
    pub payload_len: usize,
}

impl<const PAYLOAD_SIZE: usize> StoredPayload<PAYLOAD_SIZE> {
    fn from_record(record: crate::storage::spool::SpoolRecord<'_>) -> Self {
        let mut payload = [0_u8; PAYLOAD_SIZE];
        let payload_len = record.payload.len().min(PAYLOAD_SIZE);
        payload[..payload_len].copy_from_slice(&record.payload[..payload_len]);

        Self {
            sequence: record.sequence,
            payload,
            payload_len,
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
    last_error: Option<StorageError>,
}

impl<const CAPACITY: usize, const PAYLOAD_SIZE: usize> StorageBacklog<CAPACITY, PAYLOAD_SIZE> {
    pub const fn new() -> Self {
        Self {
            spool: FlashBackedSpool::new(),
            last_error: None,
        }
    }

    pub fn recover(flash: &impl FlashStorage) -> Result<Self, StorageError> {
        Ok(Self {
            spool: FlashBackedSpool::recover(flash)?,
            last_error: None,
        })
    }

    pub fn append_measurement(
        &mut self,
        flash: &mut impl FlashStorage,
        measurement: Measurement,
    ) -> Result<(), StorageError> {
        let mut payload = [0_u8; PAYLOAD_SIZE];
        let payload_len = match measurement_to_csv_line(&measurement, &mut payload) {
            Ok(len) => len,
            Err(error) => {
                self.last_error = Some(error.into());
                return Err(error.into());
            }
        };

        match self.spool.append(flash, 0, &payload[..payload_len]) {
            Ok(_) => {
                self.last_error = None;
                Ok(())
            }
            Err(error) => {
                self.last_error = Some(StorageError::Spool(error));
                Err(error.into())
            }
        }
    }

    pub fn peek_payload(&self) -> Option<StoredPayload<PAYLOAD_SIZE>> {
        self.spool.peek().map(StoredPayload::from_record)
    }

    pub fn acknowledge(
        &mut self,
        flash: &mut impl FlashStorage,
    ) -> Result<Option<StoredPayload<PAYLOAD_SIZE>>, StorageError> {
        match self.spool.acknowledge(flash) {
            Ok(record) => {
                self.last_error = None;
                Ok(record.map(|record| StoredPayload::from_record(record.as_record())))
            }
            Err(error) => {
                self.last_error = Some(StorageError::Spool(error));
                Err(error.into())
            }
        }
    }

    pub fn len(&self) -> usize {
        self.spool.len()
    }

    pub fn is_empty(&self) -> bool {
        self.spool.is_empty()
    }

    pub fn has_error(&self) -> bool {
        self.last_error.is_some()
    }

    pub fn error_flags(&self) -> ErrorFlags {
        if self.has_error() {
            ErrorFlags::STORAGE
        } else {
            ErrorFlags::NONE
        }
    }
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
pub enum StorageCommand {
    Append(Measurement),
    Peek,
    Ack,
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
    tasks::{StorageRequestChannel, StorageResponseSignal, TaskSignal},
};
#[cfg(target_arch = "riscv32")]
use defmt::{info, warn};

#[cfg(target_arch = "riscv32")]
#[embassy_executor::task]
pub async fn storage_task(
    requests: &'static StorageRequestChannel,
    responses: &'static StorageResponseSignal,
    error_flags: &'static TaskSignal<ErrorFlags>,
) {
    let mut flash = match RomSpoolFlash::new() {
        Ok(flash) => flash,
        Err(error) => {
            warn!("storage flash init failed error={:?}", error);
            error_flags.signal(ErrorFlags::STORAGE);
            loop {
                match requests.receive().await {
                    StorageCommand::Append(_) => {}
                    StorageCommand::Peek | StorageCommand::Ack => {
                        responses.signal(StorageResponse::Error(StorageError::Spool(
                            SpoolError::Flash(crate::storage::flash_model::FlashError::OutOfBounds),
                        )));
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

    let mut backlog = match MeasurementStorageBacklog::recover(&flash) {
        Ok(backlog) => {
            info!("storage recovered pending_len={=usize}", backlog.len());
            Some(backlog)
        }
        Err(error) => {
            warn!("storage recovery failed error={:?}", error);
            error_flags.signal(ErrorFlags::STORAGE);
            None
        }
    };

    loop {
        match requests.receive().await {
            StorageCommand::Append(measurement) => {
                let Some(backlog) = backlog.as_mut() else {
                    error_flags.signal(ErrorFlags::STORAGE);
                    continue;
                };

                match backlog.append_measurement(&mut flash, measurement) {
                    Ok(()) => {
                        if backlog.len().is_multiple_of(8) {
                            info!("storage append pending_len={=usize}", backlog.len());
                        }
                    }
                    Err(error) => {
                        warn!("storage append failed error={:?}", error);
                        error_flags.signal(ErrorFlags::STORAGE);
                    }
                }
            }
            StorageCommand::Peek => match backlog.as_ref() {
                Some(backlog) => responses.signal(StorageResponse::Peeked(backlog.peek_payload())),
                None => responses.signal(StorageResponse::Error(StorageError::Spool(
                    SpoolError::Flash(crate::storage::flash_model::FlashError::OutOfBounds),
                ))),
            },
            StorageCommand::Ack => {
                let Some(backlog) = backlog.as_mut() else {
                    responses.signal(StorageResponse::Error(StorageError::Spool(
                        SpoolError::Flash(crate::storage::flash_model::FlashError::OutOfBounds),
                    )));
                    error_flags.signal(ErrorFlags::STORAGE);
                    continue;
                };

                match backlog.acknowledge(&mut flash) {
                    Ok(acknowledged) => {
                        responses.signal(StorageResponse::Acked(acknowledged.is_some()));
                    }
                    Err(error) => {
                        warn!("storage ack failed error={:?}", error);
                        responses.signal(StorageResponse::Error(error));
                        error_flags.signal(ErrorFlags::STORAGE);
                    }
                }
            }
        }
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

        assert!(payload_str(&backlog.peek_payload().unwrap()).starts_with("1,"));
        backlog.acknowledge(&mut flash).unwrap();
        assert!(payload_str(&backlog.peek_payload().unwrap()).starts_with("2,"));
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

        assert!(payload_str(&acknowledged).starts_with("10,"));
        assert_eq!(backlog.len(), 1);
        assert!(payload_str(&backlog.peek_payload().unwrap()).starts_with("20,"));
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

        let mut recovered = StorageBacklog::<4, 192>::recover(&flash).unwrap();
        recovered
            .append_measurement(&mut flash, measurement(50))
            .unwrap();

        assert!(payload_str(&recovered.peek_payload().unwrap()).starts_with("40,"));
        recovered.acknowledge(&mut flash).unwrap();
        assert!(payload_str(&recovered.peek_payload().unwrap()).starts_with("50,"));
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
        assert!(payload_str(&backlog.peek_payload().unwrap()).starts_with("2,"));
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
