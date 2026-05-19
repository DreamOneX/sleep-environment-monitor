#[cfg(target_arch = "riscv32")]
use embassy_time::{Duration, Timer};

#[cfg(target_arch = "riscv32")]
use esp_hal::gpio::Output;

#[cfg(target_arch = "riscv32")]
#[embassy_executor::task]
pub async fn heartbeat_task(mut led: Output<'static>) {
    loop {
        led.set_low();
        Timer::after(Duration::from_millis(100)).await;

        led.set_high();
        Timer::after(Duration::from_millis(900)).await;
    }
}
