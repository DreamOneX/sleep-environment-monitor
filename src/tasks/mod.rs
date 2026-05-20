pub mod aggregator;
pub mod led;
pub mod mic;
pub mod net;
pub mod sensor;
pub mod upload;
pub mod wifi;

#[cfg(target_arch = "riscv32")]
pub type TaskSignal<T> =
    embassy_sync::signal::Signal<embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex, T>;

#[cfg(target_arch = "riscv32")]
pub type SampleSignal<T> = TaskSignal<T>;

#[cfg(target_arch = "riscv32")]
pub const MEASUREMENT_QUEUE_CAPACITY: usize = 16;

#[cfg(target_arch = "riscv32")]
pub type MeasurementQueue = embassy_sync::blocking_mutex::Mutex<
    embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex,
    core::cell::RefCell<
        crate::util::queue::DropOldestQueue<crate::types::Measurement, MEASUREMENT_QUEUE_CAPACITY>,
    >,
>;
