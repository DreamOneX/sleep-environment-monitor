#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
const fn saturating_u64_to_u32(value: u64) -> u32 {
    if value > u32::MAX as u64 {
        u32::MAX
    } else {
        value as u32
    }
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
const GATT_CONNECTIONS_MAX: usize = 1;
#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
const GATT_L2CAP_CHANNELS_MAX: usize = 3;
#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
const GATT_COMMAND_SLOTS: usize = 10;
#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
const RANDOM_STATIC_ADDRESS: [u8; 6] = [0xc3, 0xe2, 0x24, 0x10, 0x53, 0xf3];

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
type BleController = ExternalController<BleConnector<'static>, GATT_COMMAND_SLOTS>;
#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
type BleHostResources =
    HostResources<DefaultPacketPool, GATT_CONNECTIONS_MAX, GATT_L2CAP_CHANNELS_MAX>;

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
#[derive(Clone, Copy)]
pub struct BleTaskResources {
    pub storage_requests: &'static StorageRequestChannel,
    pub storage_responses: &'static StorageResponseSignal,
    pub network_upload_status: &'static NetworkUploadStatusMutex,
    pub firmware_status: &'static FirmwareStatusSnapshotMutex,
    pub led_runtime_state: &'static TaskSignal<BleLedRuntimeState>,
    pub auth_workspace: &'static BleAuthWorkspace,
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
#[derive(Clone, Copy)]
struct GattRuntime<'server> {
    stack: &'server Stack<'server, BleController, DefaultPacketPool>,
    server: &'server GattServer<'static>,
    handles: GattHandles,
    storage_requests: &'static StorageRequestChannel,
    storage_responses: &'static StorageResponseSignal,
    network_upload_status: &'static NetworkUploadStatusMutex,
    firmware_status: &'static FirmwareStatusSnapshotMutex,
    led_runtime_state: &'static TaskSignal<BleLedRuntimeState>,
    auth_workspace: &'static BleAuthWorkspace,
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
struct BleSecuritySeedRng {
    seed: [u8; crate::config::ble::SECURITY_SEED_LEN],
    cursor: usize,
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
impl BleSecuritySeedRng {
    const fn new(seed: [u8; crate::config::ble::SECURITY_SEED_LEN]) -> Self {
        Self { seed, cursor: 0 }
    }

    fn read_seed_bytes(&mut self, dest: &mut [u8]) {
        for byte in dest {
            *byte = self.seed[self.cursor % self.seed.len()];
            self.cursor = self.cursor.wrapping_add(1);
        }
    }
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
impl RngCore for BleSecuritySeedRng {
    fn next_u32(&mut self) -> u32 {
        let mut bytes = [0_u8; 4];
        self.read_seed_bytes(&mut bytes);
        u32::from_le_bytes(bytes)
    }

    fn next_u64(&mut self) -> u64 {
        let mut bytes = [0_u8; 8];
        self.read_seed_bytes(&mut bytes);
        u64::from_le_bytes(bytes)
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        self.read_seed_bytes(dest);
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), RandError> {
        self.fill_bytes(dest);
        Ok(())
    }
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
impl CryptoRng for BleSecuritySeedRng {}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
static BLE_HOST_RESOURCES: StaticCell<BleHostResources> = StaticCell::new();
#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
static BLE_PAIRING_STATE: Mutex<CriticalSectionRawMutex, BlePairingState> =
    Mutex::new(BlePairingState::Closed);
#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
static BLE_BOOT_BUTTON_STATE: Mutex<CriticalSectionRawMutex, BootButtonState> =
    Mutex::new(BootButtonState::Released);
#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
static BLE_BOOT_PRESSED_MILLIS: Mutex<CriticalSectionRawMutex, u32> = Mutex::new(0);

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
#[embassy_executor::task]
pub async fn ble_task(
    connector: BleConnector<'static>,
    security_seed: [u8; crate::config::ble::SECURITY_SEED_LEN],
    task_resources: BleTaskResources,
) {
    info!(
        "ble controller initialized name={=str} protocol_version={=u8}",
        crate::config::ble::ADVERTISING_NAME,
        PROTOCOL_VERSION
    );

    let controller = ExternalController::new(connector);
    let host_resources = BLE_HOST_RESOURCES.init(BleHostResources::new());
    let mut security_seed_rng = BleSecuritySeedRng::new(security_seed);
    let stack = trouble_host::new(controller, host_resources)
        .set_random_address(Address::random(RANDOM_STATIC_ADDRESS))
        .set_random_generator_seed(&mut security_seed_rng);
    stack.set_io_capabilities(IoCapabilities::NoInputNoOutput);
    restore_ble_auth_records(&stack, task_resources.auth_workspace).await;
    let Host {
        mut peripheral,
        mut runner,
        ..
    } = stack.build();

    let initial_network_upload_status = { *task_resources.network_upload_status.lock().await };
    let initial_firmware_status = { *task_resources.firmware_status.lock().await };
    let gatt = build_gatt_server(encode_status(
        BleRuntimeState::HostPending,
        initial_network_upload_status,
        initial_firmware_status,
        BootButtonState::Released,
        BlePairingState::Closed,
        0,
    ));

    let runtime = GattRuntime {
        stack: &stack,
        server: &gatt.server,
        handles: gatt.handles,
        storage_requests: task_resources.storage_requests,
        storage_responses: task_resources.storage_responses,
        network_upload_status: task_resources.network_upload_status,
        firmware_status: task_resources.firmware_status,
        led_runtime_state: task_resources.led_runtime_state,
        auth_workspace: task_resources.auth_workspace,
    };

    match select(runner.run(), gatt_advertise_loop(&mut peripheral, runtime)).await {
        Either::First(Ok(())) => warn!("ble host runner stopped"),
        Either::First(Err(error)) => error!("ble host runner failed error={:?}", error),
        Either::Second(()) => warn!("ble GATT loop stopped"),
    }

    loop {
        Timer::after(Duration::from_secs(crate::config::ble::IDLE_POLL_SECS)).await;
    }
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
#[embassy_executor::task]
pub async fn ble_pairing_task(
    boot_button: Input<'static>,
    led_pairing_status: &'static TaskSignal<BleLedPairingStatus>,
    auth_workspace: &'static BleAuthWorkspace,
) {
    let mut pairing_gesture = BlePairingGesture::new(
        crate::config::ble::PAIRING_HOLD_MILLIS,
        crate::config::ble::AUTH_CLEAR_HOLD_MILLIS,
        crate::config::ble::PAIRING_WINDOW_SECS.saturating_mul(1_000),
    );
    let initial_boot_button_state = BootButtonState::from_active_low(boot_button.is_low());
    let mut last_boot_button_state = initial_boot_button_state;
    log_boot_button_sample("initial", initial_boot_button_state, &pairing_gesture);
    if should_auto_open_pairing_window_on_boot() {
        let pairing_event = pairing_gesture.open_window();
        publish_pairing_snapshot(
            &pairing_gesture,
            initial_boot_button_state,
            led_pairing_status,
        )
        .await;
        log_pairing_event(pairing_event, &pairing_gesture);
    }

    loop {
        let boot_button_state = BootButtonState::from_active_low(boot_button.is_low());
        if boot_button_state != last_boot_button_state {
            log_boot_button_sample("transition", boot_button_state, &pairing_gesture);
            last_boot_button_state = boot_button_state;
        }
        let pairing_event = pairing_gesture.update(
            boot_button_state,
            crate::config::ble::PAIRING_BUTTON_POLL_MILLIS,
        );
        log_pairing_event(pairing_event, &pairing_gesture);
        if matches!(pairing_event, BlePairingEvent::AuthRecordsClearRequested)
            && clear_saved_ble_auth_records(auth_workspace).await
        {
            log_pairing_event(pairing_gesture.open_window(), &pairing_gesture);
        }
        publish_pairing_snapshot(&pairing_gesture, boot_button_state, led_pairing_status).await;

        Timer::after(Duration::from_millis(
            crate::config::ble::PAIRING_BUTTON_POLL_MILLIS,
        ))
        .await;
    }
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
fn log_boot_button_sample(
    label: &'static str,
    boot_button_state: BootButtonState,
    pairing_gesture: &BlePairingGesture,
) {
    info!(
        "ble boot/io9 {=str} state={:?} pressed_ms={=u64} pairing_remaining_ms={=u64}",
        label,
        boot_button_state,
        pairing_gesture.pressed_millis(),
        pairing_gesture.state().remaining_millis()
    );
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
async fn publish_pairing_snapshot(
    pairing_gesture: &BlePairingGesture,
    boot_button_state: BootButtonState,
    led_pairing_status: &'static TaskSignal<BleLedPairingStatus>,
) {
    led_pairing_status.signal(BleLedPairingStatus {
        window_open: pairing_gesture.state().is_open(),
        button_pressed: boot_button_state.is_pressed(),
    });
    {
        let mut state = BLE_BOOT_BUTTON_STATE.lock().await;
        *state = boot_button_state;
    }
    {
        let mut pressed_millis = BLE_BOOT_PRESSED_MILLIS.lock().await;
        *pressed_millis = saturating_u64_to_u32(pairing_gesture.pressed_millis());
    }
    {
        let mut state = BLE_PAIRING_STATE.lock().await;
        *state = pairing_gesture.state();
    }
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
fn log_pairing_event(pairing_event: BlePairingEvent, pairing_gesture: &BlePairingGesture) {
    match pairing_event {
        BlePairingEvent::WindowOpened => info!(
            "ble pairing window opened remaining_ms={=u64}",
            pairing_gesture.state().remaining_millis()
        ),
        BlePairingEvent::WindowExpired => info!("ble pairing window expired"),
        BlePairingEvent::AuthRecordsClearRequested => info!(
            "ble auth records clear requested pressed_ms={=u64}",
            pairing_gesture.pressed_millis()
        ),
        BlePairingEvent::None => {}
    }
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
fn encode_status(
    runtime_state: BleRuntimeState,
    network_upload_status: NetworkUploadStatus,
    firmware_status: FirmwareStatusSnapshot,
    boot_button_state: BootButtonState,
    pairing_state: BlePairingState,
    boot_pressed_millis: u32,
) -> [u8; BleStatus::ENCODED_LEN] {
    let status = status_from_snapshots_and_pairing(
        runtime_state,
        network_upload_status,
        firmware_status,
        boot_button_state,
        pairing_state,
        boot_pressed_millis,
    );
    let mut out = [0_u8; BleStatus::ENCODED_LEN];
    let _ = status.encode(&mut out);
    out
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
async fn set_gatt_status(
    server: &GattServer<'static>,
    handles: GattHandles,
    network_upload_status: &'static NetworkUploadStatusMutex,
    firmware_status: &'static FirmwareStatusSnapshotMutex,
    led_runtime_state: &'static TaskSignal<BleLedRuntimeState>,
    runtime_state: BleRuntimeState,
) {
    led_runtime_state.signal(ble_led_runtime_state_from_runtime(runtime_state));
    let network_upload_status = { *network_upload_status.lock().await };
    let firmware_status = { *firmware_status.lock().await };
    let boot_button_state = { *BLE_BOOT_BUTTON_STATE.lock().await };
    let pairing_state = { *BLE_PAIRING_STATE.lock().await };
    let boot_pressed_millis = { *BLE_BOOT_PRESSED_MILLIS.lock().await };
    let status = encode_status(
        runtime_state,
        network_upload_status,
        firmware_status,
        boot_button_state,
        pairing_state,
        boot_pressed_millis,
    );
    if let Err(error) = handles.status.set(server, &status) {
        warn!("ble status characteristic update failed error={:?}", error);
    }
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
const fn ble_led_runtime_state_from_runtime(runtime_state: BleRuntimeState) -> BleLedRuntimeState {
    match runtime_state {
        BleRuntimeState::Disabled => BleLedRuntimeState::Disabled,
        BleRuntimeState::ControllerReady | BleRuntimeState::HostPending => BleLedRuntimeState::Idle,
        BleRuntimeState::Advertising => BleLedRuntimeState::Advertising,
        BleRuntimeState::Connected => BleLedRuntimeState::Connected,
        BleRuntimeState::Error => BleLedRuntimeState::Error,
    }
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
async fn gatt_advertise_loop(
    peripheral: &mut Peripheral<'_, BleController, DefaultPacketPool>,
    runtime: GattRuntime<'_>,
) {
    let mut adv_data = [0_u8; 31];
    let adv_len = match AdStructure::encode_slice(
        &[
            AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
            AdStructure::ServiceUuids128(&[SERVICE_UUID]),
        ],
        &mut adv_data,
    ) {
        Ok(len) => len,
        Err(error) => {
            error!("ble advertising data encode failed error={:?}", error);
            return;
        }
    };

    let mut scan_data = [0_u8; 31];
    let scan_len = match AdStructure::encode_slice(
        &[AdStructure::CompleteLocalName(
            crate::config::ble::ADVERTISING_NAME.as_bytes(),
        )],
        &mut scan_data,
    ) {
        Ok(len) => len,
        Err(error) => {
            error!("ble scan response encode failed error={:?}", error);
            return;
        }
    };

    loop {
        set_gatt_status(
            runtime.server,
            runtime.handles,
            runtime.network_upload_status,
            runtime.firmware_status,
            runtime.led_runtime_state,
            BleRuntimeState::Advertising,
        )
        .await;
        info!(
            "ble advertising name={=str} protocol_version={=u8}",
            crate::config::ble::ADVERTISING_NAME,
            PROTOCOL_VERSION
        );

        let acceptor = match peripheral
            .advertise(
                &Default::default(),
                Advertisement::ConnectableScannableUndirected {
                    adv_data: &adv_data[..adv_len],
                    scan_data: &scan_data[..scan_len],
                },
            )
            .await
        {
            Ok(acceptor) => acceptor,
            Err(error) => {
                set_gatt_status(
                    runtime.server,
                    runtime.handles,
                    runtime.network_upload_status,
                    runtime.firmware_status,
                    runtime.led_runtime_state,
                    BleRuntimeState::Error,
                )
                .await;
                warn!("ble advertise failed error={:?}", error);
                Timer::after(Duration::from_secs(crate::config::ble::IDLE_POLL_SECS)).await;
                continue;
            }
        };

        let connection = match acceptor.accept().await {
            Ok(connection) => connection,
            Err(error) => {
                warn!("ble advertise accept failed error={:?}", error);
                continue;
            }
        };
        let connection = match connection.with_attribute_server(runtime.server) {
            Ok(connection) => connection,
            Err(error) => {
                set_gatt_status(
                    runtime.server,
                    runtime.handles,
                    runtime.network_upload_status,
                    runtime.firmware_status,
                    runtime.led_runtime_state,
                    BleRuntimeState::Error,
                )
                .await;
                warn!("ble GATT attach failed error={:?}", error);
                continue;
            }
        };

        configure_ble_connection_security(&connection, runtime.auth_workspace).await;
        set_gatt_status(
            runtime.server,
            runtime.handles,
            runtime.network_upload_status,
            runtime.firmware_status,
            runtime.led_runtime_state,
            BleRuntimeState::Connected,
        )
        .await;
        info!("ble central connected");
        run_gatt_connection(connection, runtime).await;
    }
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
async fn configure_ble_connection_security(
    connection: &GattConnection<'_, '_, DefaultPacketPool>,
    _auth_workspace: &'static BleAuthWorkspace,
) {
    let pairing_open = { BLE_PAIRING_STATE.lock().await.is_open() };
    if let Err(error) = connection.raw().set_bondable(pairing_open) {
        warn!(
            "ble connection bondable configuration failed error={:?}",
            error
        );
    }
    if pairing_open && let Err(error) = connection.raw().request_security() {
        warn!("ble security request failed error={:?}", error);
    }
}
