#[cfg(target_arch = "riscv32")]
use embassy_net::Runner;
#[cfg(target_arch = "riscv32")]
use esp_radio::wifi::Interface;

#[cfg(target_arch = "riscv32")]
#[embassy_executor::task]
pub async fn net_task(mut runner: Runner<'static, Interface<'static>>) -> ! {
    runner.run().await
}
