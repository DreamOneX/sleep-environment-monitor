#![cfg_attr(target_arch = "riscv32", no_std)]
#![cfg_attr(target_arch = "riscv32", no_main)]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

#[cfg(target_arch = "riscv32")]
use defmt::{info, warn};
#[cfg(target_arch = "riscv32")]
use embassy_executor::{SpawnError, SpawnToken, Spawner};
#[cfg(all(target_arch = "riscv32", feature = "wifi-upload"))]
use embassy_net::StackResources;
#[cfg(target_arch = "riscv32")]
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel, signal::Signal,
};
#[cfg(target_arch = "riscv32")]
use embassy_time::{Duration, Timer};
#[cfg(target_arch = "riscv32")]
use esp_hal::analog::adc::{Adc, AdcConfig};
#[cfg(target_arch = "riscv32")]
use esp_hal::clock::CpuClock;
#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
use esp_hal::gpio::{Input, InputConfig};
#[cfg(target_arch = "riscv32")]
use esp_hal::gpio::{Level, Output, OutputConfig};
#[cfg(target_arch = "riscv32")]
use esp_hal::i2c::master::{Config as I2cConfig, I2c};
#[cfg(all(target_arch = "riscv32", feature = "wifi-upload"))]
use esp_hal::rng::Rng;
#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
use esp_hal::rng::{Trng, TrngSource};
#[cfg(target_arch = "riscv32")]
use esp_hal::time::Rate;
#[cfg(target_arch = "riscv32")]
use esp_hal::timer::timg::TimerGroup;
#[cfg(target_arch = "riscv32")]
use panic_rtt_target as _;
#[cfg(all(target_arch = "riscv32", feature = "flash-smoke"))]
use sleep_environment_monitor::drivers::flash::run_flash_smoke_test;
#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
use sleep_environment_monitor::tasks::ble::{
    BleTaskResources, ble_auth_workspace, ble_pairing_task, ble_task,
};
#[cfg(all(target_arch = "riscv32", feature = "wifi-upload"))]
use sleep_environment_monitor::tasks::{net::net_task, upload::uploader_task, wifi::wifi_task};
#[cfg(target_arch = "riscv32")]
use sleep_environment_monitor::{
    config,
    tasks::{
        FirmwareStatusSnapshotMutex, NetworkUploadStatusMutex, StorageRequestChannel,
        StorageResponseSignal,
        aggregator::aggregator_task,
        led::{heartbeat_task, status_task},
        mic::mic_task,
        sensor::sensor_task,
        storage::{StorageCommand, StorageResponse, storage_task},
    },
    types::{
        EnvSample, ErrorFlags, FirmwareStatusSnapshot, MicSample, NetworkState,
        NetworkUploadStatus, UploadResult,
    },
    util::status::{BleLedPairingStatus, BleLedRuntimeState},
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
static ERROR_FLAGS_SIGNAL: Signal<CriticalSectionRawMutex, ErrorFlags> = Signal::new();
#[cfg(target_arch = "riscv32")]
static BLE_LED_RUNTIME_STATE: Signal<CriticalSectionRawMutex, BleLedRuntimeState> = Signal::new();
#[cfg(target_arch = "riscv32")]
static BLE_LED_PAIRING_STATUS: Signal<CriticalSectionRawMutex, BleLedPairingStatus> = Signal::new();
#[cfg(target_arch = "riscv32")]
static NETWORK_UPLOAD_STATUS: NetworkUploadStatusMutex = NetworkUploadStatusMutex::new(
    NetworkUploadStatus::new(NetworkState::Disconnected, UploadResult::Idle),
);
#[cfg(target_arch = "riscv32")]
static FIRMWARE_STATUS: FirmwareStatusSnapshotMutex =
    FirmwareStatusSnapshotMutex::new(FirmwareStatusSnapshot::new(0, ErrorFlags::NONE));
#[cfg(target_arch = "riscv32")]
static STORAGE_REQUESTS: StorageRequestChannel = Channel::<
    CriticalSectionRawMutex,
    StorageCommand,
    { sleep_environment_monitor::tasks::STORAGE_REQUEST_CAPACITY },
>::new();
#[cfg(target_arch = "riscv32")]
static WIFI_STORAGE_RESPONSES: StorageResponseSignal =
    Signal::<CriticalSectionRawMutex, StorageResponse>::new();
#[cfg(target_arch = "riscv32")]
static BLE_STORAGE_RESPONSES: StorageResponseSignal =
    Signal::<CriticalSectionRawMutex, StorageResponse>::new();
#[cfg(all(target_arch = "riscv32", feature = "wifi-upload"))]
static NET_RESOURCES: static_cell::StaticCell<
    StackResources<{ config::network::STACK_RESOURCE_COUNT }>,
> = static_cell::StaticCell::new();

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

#[cfg(target_arch = "riscv32")]
fn spawn_task<S>(
    spawner: &Spawner,
    task: Result<SpawnToken<S>, SpawnError>,
    name: &'static str,
) -> bool {
    match task {
        Ok(token) => {
            spawner.spawn(token);
            true
        }
        Err(error) => {
            warn!("task spawn failed name={=str} error={:?}", name, error);
            false
        }
    }
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

    let hal_config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    #[cfg(feature = "ble-upload")]
    let mut peripherals = esp_hal::init(hal_config);
    #[cfg(not(feature = "ble-upload"))]
    let peripherals = esp_hal::init(hal_config);

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

    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: config::runtime::HEAP_SIZE_BYTES);

    #[cfg(feature = "ble-upload")]
    let ble_security_seed = {
        let mut seed = [0_u8; config::ble::SECURITY_SEED_LEN];
        let trng_source = TrngSource::new(peripherals.RNG, peripherals.ADC1.reborrow());
        let seed = match Trng::try_new() {
            Ok(trng) => {
                trng.read(&mut seed);
                drop(trng);
                Some(seed)
            }
            Err(error) => {
                warn!("BLE security TRNG initialization failed error={:?}", error);
                None
            }
        };
        drop(trng_source);
        seed
    };

    #[cfg(feature = "flash-smoke")]
    {
        match run_flash_smoke_test() {
            Ok(offset) => {
                info!("flash smoke test passed spool_offset=0x{:08x}", offset);
            }
            Err(error) => {
                warn!("flash smoke test failed error={:?}", error);
            }
        }
    }

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_interrupt =
        esp_hal::interrupt::software::SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_interrupt.software_interrupt0);

    let led2_red = Output::new(peripherals.GPIO0, Level::High, OutputConfig::default());
    spawn_task(&spawner, heartbeat_task(led2_red), "heartbeat");

    let led3_blue = Output::new(peripherals.GPIO1, Level::High, OutputConfig::default());
    spawn_task(
        &spawner,
        status_task(
            led3_blue,
            &ERROR_FLAGS_SIGNAL,
            &NETWORK_STATE_SIGNAL,
            &UPLOAD_RESULT_SIGNAL,
            &BLE_LED_RUNTIME_STATE,
            &BLE_LED_PAIRING_STATUS,
        ),
        "status",
    );

    let i2c_config =
        I2cConfig::default().with_frequency(Rate::from_khz(config::sensor::I2C_FREQUENCY_KHZ));
    match I2c::new(peripherals.I2C0, i2c_config) {
        Ok(i2c) => {
            let i2c = i2c.with_sda(peripherals.GPIO4).with_scl(peripherals.GPIO5);
            if !spawn_task(&spawner, sensor_task(i2c, &ENV_SAMPLE_SIGNAL), "sensor") {
                let flags = ErrorFlags::SHT40 | ErrorFlags::OPT3001;
                ENV_SAMPLE_SIGNAL.signal(EnvSample {
                    error_flags: flags,
                    ..EnvSample::default()
                });
                ERROR_FLAGS_SIGNAL.signal(flags);
            }
        }
        Err(_) => {
            let flags = ErrorFlags::SHT40 | ErrorFlags::OPT3001;
            warn!("I2C0 configuration failed; sensor task disabled");
            ENV_SAMPLE_SIGNAL.signal(EnvSample {
                error_flags: flags,
                ..EnvSample::default()
            });
            ERROR_FLAGS_SIGNAL.signal(flags);
        }
    }

    let mut adc1_config = AdcConfig::new();
    let mic_pin = adc1_config.enable_pin(peripherals.GPIO3, config::mic::adc_attenuation());
    let adc1 = Adc::new(peripherals.ADC1, adc1_config);
    if !spawn_task(&spawner, mic_task(adc1, mic_pin, &MIC_SAMPLE_SIGNAL), "mic") {
        MIC_SAMPLE_SIGNAL.signal(MicSample {
            error_flags: ErrorFlags::MIC,
            ..MicSample::default()
        });
        ERROR_FLAGS_SIGNAL.signal(ErrorFlags::MIC);
    }

    spawn_task(
        &spawner,
        storage_task(
            &STORAGE_REQUESTS,
            &WIFI_STORAGE_RESPONSES,
            &BLE_STORAGE_RESPONSES,
            &ERROR_FLAGS_SIGNAL,
            &FIRMWARE_STATUS,
        ),
        "storage",
    );

    spawn_task(
        &spawner,
        aggregator_task(
            &ENV_SAMPLE_SIGNAL,
            &MIC_SAMPLE_SIGNAL,
            &STORAGE_REQUESTS,
            &ERROR_FLAGS_SIGNAL,
            &FIRMWARE_STATUS,
        ),
        "aggregator",
    );

    #[cfg(feature = "ble-upload")]
    let ble_auth_workspace = ble_auth_workspace();
    #[cfg(feature = "ble-upload")]
    let boot_button = Input::new(peripherals.GPIO9, InputConfig::default());
    #[cfg(feature = "ble-upload")]
    if !spawn_task(
        &spawner,
        ble_pairing_task(boot_button, &BLE_LED_PAIRING_STATUS),
        "ble-pairing",
    ) {
        warn!("BLE pairing task spawn failed; BOOT/IO9 pairing window disabled");
    }
    #[cfg(feature = "ble-upload")]
    match esp_radio::ble::controller::BleConnector::new(peripherals.BT, Default::default()) {
        Ok(connector) => {
            if let Some(security_seed) = ble_security_seed {
                if !spawn_task(
                    &spawner,
                    ble_task(
                        connector,
                        security_seed,
                        BleTaskResources {
                            storage_requests: &STORAGE_REQUESTS,
                            storage_responses: &BLE_STORAGE_RESPONSES,
                            network_upload_status: &NETWORK_UPLOAD_STATUS,
                            firmware_status: &FIRMWARE_STATUS,
                            led_runtime_state: &BLE_LED_RUNTIME_STATE,
                            auth_workspace: ble_auth_workspace,
                        },
                    ),
                    "ble",
                ) {
                    warn!("BLE task spawn failed; BLE upload boundary disabled");
                }
            } else {
                warn!("BLE task not started because security seed is unavailable");
            }
        }
        Err(error) => {
            warn!("BLE controller initialization failed error={:?}", error);
        }
    }

    #[cfg(feature = "wifi-upload")]
    start_wifi_upload(peripherals.WIFI, &spawner).await;
    #[cfg(not(feature = "wifi-upload"))]
    {
        NETWORK_STATE_SIGNAL.signal(NetworkState::Disconnected);
        UPLOAD_RESULT_SIGNAL.signal(UploadResult::Idle);
        set_network_upload_status(NetworkState::Disconnected, UploadResult::Idle).await;
        info!("Wi-Fi upload feature disabled; REST uploader not started");
    }

    info!(
        "measurement aggregation initialized wifi_upload={=bool} ble_upload={=bool}",
        config::wifi::ENABLED,
        config::ble::ENABLED
    );

    loop {
        Timer::after(Duration::from_secs(config::runtime::MAIN_IDLE_SLEEP_SECS)).await;
    }

    // for inspiration have a look at the examples at https://github.com/esp-rs/esp-hal/tree/esp-hal-v1.1.0/examples
}

#[cfg(all(target_arch = "riscv32", feature = "wifi-upload"))]
async fn update_network_upload_status(upload_result: UploadResult) {
    let mut status = NETWORK_UPLOAD_STATUS.lock().await;
    *status = status
        .with_network_state(NetworkState::Disconnected)
        .with_upload_result(upload_result);
}

#[cfg(all(target_arch = "riscv32", not(feature = "wifi-upload")))]
async fn set_network_upload_status(network_state: NetworkState, upload_result: UploadResult) {
    let mut status = NETWORK_UPLOAD_STATUS.lock().await;
    *status = status
        .with_network_state(network_state)
        .with_upload_result(upload_result);
}

#[cfg(all(target_arch = "riscv32", feature = "wifi-upload"))]
async fn start_wifi_upload(wifi: esp_hal::peripherals::WIFI<'static>, spawner: &Spawner) {
    match esp_radio::wifi::new(wifi, Default::default()) {
        Ok((wifi_controller, wifi_interfaces)) => {
            let net_config = config::network::default_config();
            let random_seed = Rng::new().random() as u64;
            let (stack, runner) = embassy_net::new(
                wifi_interfaces.station,
                net_config,
                NET_RESOURCES.init_with(StackResources::new),
                random_seed,
            );
            let net_started = spawn_task(spawner, net_task(runner), "network");
            let wifi_started = spawn_task(
                spawner,
                wifi_task(
                    wifi_controller,
                    &NETWORK_STATE_SIGNAL,
                    &NETWORK_UPLOAD_STATUS,
                ),
                "wifi",
            );

            if net_started && wifi_started {
                if !spawn_task(
                    spawner,
                    uploader_task(
                        stack,
                        &STORAGE_REQUESTS,
                        &WIFI_STORAGE_RESPONSES,
                        &NETWORK_STATE_SIGNAL,
                        &UPLOAD_RESULT_SIGNAL,
                        &NETWORK_UPLOAD_STATUS,
                    ),
                    "uploader",
                ) {
                    UPLOAD_RESULT_SIGNAL.signal(UploadResult::Failed);
                    update_network_upload_status(UploadResult::Failed).await;
                }
            } else {
                NETWORK_STATE_SIGNAL.signal(NetworkState::Disconnected);
                UPLOAD_RESULT_SIGNAL.signal(UploadResult::Failed);
                update_network_upload_status(UploadResult::Failed).await;
            }
        }
        Err(_) => {
            warn!("Wi-Fi controller initialization failed; network and uploader disabled");
            NETWORK_STATE_SIGNAL.signal(NetworkState::Disconnected);
            UPLOAD_RESULT_SIGNAL.signal(UploadResult::Failed);
            update_network_upload_status(UploadResult::Failed).await;
        }
    }
}
