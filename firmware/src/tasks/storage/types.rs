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
