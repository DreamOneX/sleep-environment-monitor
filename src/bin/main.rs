#![cfg_attr(target_arch = "riscv32", no_std)]
#![cfg_attr(target_arch = "riscv32", no_main)]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

#[cfg(target_arch = "riscv32")]
use defmt::info;
#[cfg(target_arch = "riscv32")]
use embassy_executor::Spawner;
#[cfg(target_arch = "riscv32")]
use embassy_net::StackResources;
#[cfg(target_arch = "riscv32")]
use embassy_sync::{
    blocking_mutex::{Mutex, raw::CriticalSectionRawMutex},
    signal::Signal,
};
#[cfg(target_arch = "riscv32")]
use embassy_time::{Duration, Timer};
#[cfg(target_arch = "riscv32")]
use esp_hal::analog::adc::{Adc, AdcConfig, Attenuation};
#[cfg(target_arch = "riscv32")]
use esp_hal::clock::CpuClock;
#[cfg(target_arch = "riscv32")]
use esp_hal::gpio::{Level, Output, OutputConfig};
#[cfg(target_arch = "riscv32")]
use esp_hal::i2c::master::{Config as I2cConfig, I2c};
#[cfg(target_arch = "riscv32")]
use esp_hal::rng::Rng;
#[cfg(target_arch = "riscv32")]
use esp_hal::time::Rate;
#[cfg(target_arch = "riscv32")]
use esp_hal::timer::timg::TimerGroup;
#[cfg(target_arch = "riscv32")]
use panic_rtt_target as _;
#[cfg(target_arch = "riscv32")]
use sleep_environment_monitor::{
    tasks::{
        MeasurementQueue, aggregator::aggregator_task, led::heartbeat_task, mic::mic_task,
        net::net_task, sensor::sensor_task, upload::uploader_task, wifi::wifi_task,
    },
    types::{EnvSample, MicSample, NetworkState, UploadResult},
    util::queue::DropOldestQueue,
};

#[cfg(target_arch = "riscv32")]
static ENV_SAMPLE_SIGNAL: Signal<CriticalSectionRawMutex, EnvSample> = Signal::new();
#[cfg(target_arch = "riscv32")]
static MIC_SAMPLE_SIGNAL: Signal<CriticalSectionRawMutex, MicSample> = Signal::new();
#[cfg(target_arch = "riscv32")]
static NETWORK_STATE_SIGNAL: Signal<CriticalSectionRawMutex, NetworkState> = Signal::new();
#[cfg(target_arch = "riscv32")]
static UPLOAD_RESULT_SIGNAL: Signal<CriticalSectionRawMutex, UploadResult> = Signal::new();
#[cfg(target_arch = "riscv32")]
static MEASUREMENT_QUEUE: MeasurementQueue =
    Mutex::new(core::cell::RefCell::new(DropOldestQueue::new()));
#[cfg(target_arch = "riscv32")]
static NET_RESOURCES: static_cell::StaticCell<StackResources<3>> = static_cell::StaticCell::new();

#[cfg(target_arch = "riscv32")]
extern crate alloc;

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
#[cfg(target_arch = "riscv32")]
esp_bootloader_esp_idf::esp_app_desc!();

#[cfg(not(target_arch = "riscv32"))]
fn main() {
    println!(
        "sleep-environment-monitor firmware: build with --target riscv32imc-unknown-none-elf for ESP32-C3"
    );
}

#[allow(
    clippy::large_stack_frames,
    reason = "it's not unusual to allocate larger buffers etc. in main"
)]
#[cfg(target_arch = "riscv32")]
#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    // generator version: 1.3.0
    // generator parameters: --chip esp32c3 -o esp32c3-wroom-02 -o unstable-hal -o alloc -o wifi -o embassy -o probe-rs -o defmt -o panic-rtt-target -o neovim -o vscode

    rtt_target::rtt_init_defmt!();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    // The following pins are used to bootstrap the chip. They are available
    // for use, but check the datasheet of the module for more information on them.
    // - GPIO2
    // - GPIO8
    // - GPIO9
    // These GPIO pins are in use by some feature of the module and should not be used.
    let _ = peripherals.GPIO11;
    let _ = peripherals.GPIO12;
    let _ = peripherals.GPIO13;
    let _ = peripherals.GPIO14;
    let _ = peripherals.GPIO15;
    let _ = peripherals.GPIO16;
    let _ = peripherals.GPIO17;

    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 66320);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_interrupt =
        esp_hal::interrupt::software::SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_interrupt.software_interrupt0);

    let led1 = Output::new(peripherals.GPIO0, Level::High, OutputConfig::default());
    let heartbeat = heartbeat_task(led1).expect("heartbeat task should spawn once");
    spawner.spawn(heartbeat);

    let i2c_config = I2cConfig::default().with_frequency(Rate::from_khz(100));
    let i2c = I2c::new(peripherals.I2C0, i2c_config)
        .expect("I2C0 configuration should be valid")
        .with_sda(peripherals.GPIO4)
        .with_scl(peripherals.GPIO5);
    let sensors = sensor_task(i2c, &ENV_SAMPLE_SIGNAL).expect("sensor task should spawn once");
    spawner.spawn(sensors);

    let mut adc1_config = AdcConfig::new();
    let mic_pin = adc1_config.enable_pin(peripherals.GPIO3, Attenuation::_11dB);
    let adc1 = Adc::new(peripherals.ADC1, adc1_config);
    let mic =
        mic_task(adc1, mic_pin, &MIC_SAMPLE_SIGNAL).expect("microphone task should spawn once");
    spawner.spawn(mic);

    let aggregator = aggregator_task(&ENV_SAMPLE_SIGNAL, &MIC_SAMPLE_SIGNAL, &MEASUREMENT_QUEUE)
        .expect("aggregator task should spawn once");
    spawner.spawn(aggregator);

    let (wifi_controller, wifi_interfaces) =
        esp_radio::wifi::new(peripherals.WIFI, Default::default())
            .expect("Wi-Fi controller should initialize after scheduler start");
    let net_config = embassy_net::Config::dhcpv4(Default::default());
    let random_seed = Rng::new().random() as u64;
    let (stack, runner) = embassy_net::new(
        wifi_interfaces.station,
        net_config,
        NET_RESOURCES.init(StackResources::new()),
        random_seed,
    );
    let network = net_task(runner).expect("network task should spawn once");
    spawner.spawn(network);

    let wifi =
        wifi_task(wifi_controller, &NETWORK_STATE_SIGNAL).expect("wifi task should spawn once");
    spawner.spawn(wifi);

    let uploader = uploader_task(stack, &MEASUREMENT_QUEUE, &UPLOAD_RESULT_SIGNAL)
        .expect("uploader task should spawn once");
    spawner.spawn(uploader);

    info!("measurement aggregation, Wi-Fi manager, and uploader initialized");

    loop {
        Timer::after(Duration::from_secs(60)).await;
    }

    // for inspiration have a look at the examples at https://github.com/esp-rs/esp-hal/tree/esp-hal-v1.1.0/examples
}
