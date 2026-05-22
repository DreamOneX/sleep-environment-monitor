#[cfg(all(target_arch = "riscv32", feature = "wifi-upload"))]
use embassy_net::Runner;
#[cfg(all(target_arch = "riscv32", feature = "wifi-upload"))]
use esp_radio::wifi::Interface;

#[cfg(all(target_arch = "riscv32", feature = "wifi-upload"))]
#[embassy_executor::task]
pub async fn net_task(mut runner: Runner<'static, Interface<'static>>) -> ! {
    runner.run().await
}
