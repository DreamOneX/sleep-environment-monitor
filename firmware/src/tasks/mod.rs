pub mod aggregator;
pub mod ble;
pub mod led;
pub mod mic;
pub mod net;
pub mod sensor;
pub mod storage;
pub mod upload;
pub mod wifi;

#[cfg(target_arch = "riscv32")]
pub type TaskSignal<T> =
    embassy_sync::signal::Signal<embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex, T>;

#[cfg(target_arch = "riscv32")]
pub type NetworkUploadStatusMutex = embassy_sync::mutex::Mutex<
    embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex,
    crate::types::NetworkUploadStatus,
>;

#[cfg(target_arch = "riscv32")]
pub type SampleSignal<T> = TaskSignal<T>;

#[cfg(target_arch = "riscv32")]
pub const STORAGE_REQUEST_CAPACITY: usize = crate::config::storage::REQUEST_CAPACITY;

#[cfg(target_arch = "riscv32")]
pub type StorageRequestChannel = embassy_sync::channel::Channel<
    embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex,
    storage::StorageCommand,
    STORAGE_REQUEST_CAPACITY,
>;

#[cfg(target_arch = "riscv32")]
pub type StorageResponseSignal = TaskSignal<storage::StorageResponse>;
