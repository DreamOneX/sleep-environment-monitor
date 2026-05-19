pub mod aggregator;
pub mod led;
pub mod mic;
pub mod sensor;
pub mod upload;
pub mod wifi;

#[cfg(target_arch = "riscv32")]
pub type TaskSignal<T> =
    embassy_sync::signal::Signal<embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex, T>;

#[cfg(target_arch = "riscv32")]
pub type SampleSignal<T> = TaskSignal<T>;
