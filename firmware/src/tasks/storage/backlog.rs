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
