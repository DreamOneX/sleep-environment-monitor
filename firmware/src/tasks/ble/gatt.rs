#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
const GATT_ATTRIBUTE_MAX: usize = 17;
#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
const GATT_CCCD_MAX: usize = 2;
#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
const GAP_APPEARANCE_GENERIC_SENSOR: [u8; 2] = [0x40, 0x05];

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
type GattMutex = embassy_sync_07::blocking_mutex::raw::NoopRawMutex;
#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
type GattTable<'values> = AttributeTable<'values, GattMutex, GATT_ATTRIBUTE_MAX>;
#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
type GattServer<'values> = AttributeServer<
    'values,
    GattMutex,
    DefaultPacketPool,
    GATT_ATTRIBUTE_MAX,
    GATT_CCCD_MAX,
    GATT_CONNECTIONS_MAX,
>;

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
struct BleGatt {
    server: GattServer<'static>,
    handles: GattHandles,
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
#[derive(Clone, Copy)]
struct GattHandles {
    status: Characteristic<[u8; BleStatus::ENCODED_LEN]>,
    metadata: Characteristic<[u8; RecordMetadata::ENCODED_LEN]>,
    fragment: Characteristic<[u8; RecordFragment::HEADER_LEN + MAX_FRAGMENT_PAYLOAD_LEN]>,
    control: Characteristic<[u8; ControlFrame::ENCODED_LEN]>,
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
struct BleRecordTransferRuntime {
    payload: Option<StoredPayload>,
    session: BleTransferSession,
    last_fragment: [u8; RecordFragment::HEADER_LEN + MAX_FRAGMENT_PAYLOAD_LEN],
    last_fragment_len: usize,
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
impl BleRecordTransferRuntime {
    const fn new() -> Self {
        Self {
            payload: None,
            session: BleTransferSession::new(),
            last_fragment: [0_u8; RecordFragment::HEADER_LEN + MAX_FRAGMENT_PAYLOAD_LEN],
            last_fragment_len: 0,
        }
    }

    fn reset_after_disconnect(&mut self) {
        self.payload = None;
        self.session.reset_after_disconnect();
        self.last_fragment = [0_u8; RecordFragment::HEADER_LEN + MAX_FRAGMENT_PAYLOAD_LEN];
        self.last_fragment_len = 0;
    }
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
static BLE_DEVICE_NAME: StaticCell<[u8; crate::config::ble::ADVERTISING_NAME.len()]> =
    StaticCell::new();
#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
static BLE_APPEARANCE: StaticCell<[u8; 2]> = StaticCell::new();
#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
static BLE_STATUS_VALUE: StaticCell<[u8; BleStatus::ENCODED_LEN]> = StaticCell::new();
#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
static BLE_RECORD_METADATA_VALUE: StaticCell<[u8; RecordMetadata::ENCODED_LEN]> = StaticCell::new();
#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
static BLE_RECORD_FRAGMENT_VALUE: StaticCell<
    [u8; RecordFragment::HEADER_LEN + MAX_FRAGMENT_PAYLOAD_LEN],
> = StaticCell::new();
#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
static BLE_CONTROL_VALUE: StaticCell<[u8; ControlFrame::ENCODED_LEN]> = StaticCell::new();
#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
static BLE_STATUS_STORE: StaticCell<[u8; BleStatus::ENCODED_LEN]> = StaticCell::new();
#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
static BLE_METADATA_STORE: StaticCell<[u8; RecordMetadata::ENCODED_LEN]> = StaticCell::new();
#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
static BLE_FRAGMENT_STORE: StaticCell<[u8; RecordFragment::HEADER_LEN + MAX_FRAGMENT_PAYLOAD_LEN]> =
    StaticCell::new();
#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
static BLE_CONTROL_STORE: StaticCell<[u8; ControlFrame::ENCODED_LEN]> = StaticCell::new();

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
fn build_gatt_server(initial_status: [u8; BleStatus::ENCODED_LEN]) -> BleGatt {
    let device_name = BLE_DEVICE_NAME.init_with(|| {
        let mut name = [0_u8; crate::config::ble::ADVERTISING_NAME.len()];
        name.copy_from_slice(crate::config::ble::ADVERTISING_NAME.as_bytes());
        name
    });
    let appearance = BLE_APPEARANCE.init_with(|| GAP_APPEARANCE_GENERIC_SENSOR);
    let status_value = BLE_STATUS_VALUE.init_with(|| initial_status);
    let metadata_value = BLE_RECORD_METADATA_VALUE.init([0_u8; RecordMetadata::ENCODED_LEN]);
    let fragment_value = BLE_RECORD_FRAGMENT_VALUE
        .init([0_u8; RecordFragment::HEADER_LEN + MAX_FRAGMENT_PAYLOAD_LEN]);
    let control_value = BLE_CONTROL_VALUE.init([0_u8; ControlFrame::ENCODED_LEN]);

    let mut table = GattTable::new();

    let mut gap_service = table.add_service(Service::new(0x1800_u16));
    let _ = gap_service.add_characteristic_ro(0x2a00_u16, &device_name[..]);
    let _ = gap_service.add_characteristic_ro(0x2a01_u16, &appearance[..]);
    gap_service.build();

    table.add_service(Service::new(0x1801_u16)).build();

    let mut project_service = table.add_service(Service::new(Uuid::new_long(SERVICE_UUID)));
    let status_handle = project_service
        .add_characteristic(
            Uuid::new_long(STATUS_CHARACTERISTIC_UUID),
            [CharacteristicProp::Read, CharacteristicProp::Notify],
            *status_value,
            BLE_STATUS_STORE.init([0_u8; BleStatus::ENCODED_LEN]),
        )
        .build();
    let metadata_handle = project_service
        .add_characteristic(
            Uuid::new_long(RECORD_METADATA_CHARACTERISTIC_UUID),
            [CharacteristicProp::Read],
            *metadata_value,
            BLE_METADATA_STORE.init([0_u8; RecordMetadata::ENCODED_LEN]),
        )
        .read_permission(PermissionLevel::EncryptionRequired)
        .build();
    let fragment_handle = project_service
        .add_characteristic(
            Uuid::new_long(RECORD_FRAGMENT_CHARACTERISTIC_UUID),
            [CharacteristicProp::Read, CharacteristicProp::Notify],
            *fragment_value,
            BLE_FRAGMENT_STORE.init([0_u8; RecordFragment::HEADER_LEN + MAX_FRAGMENT_PAYLOAD_LEN]),
        )
        .read_permission(PermissionLevel::EncryptionRequired)
        .build();
    let control_handle = project_service
        .add_characteristic(
            Uuid::new_long(CONTROL_CHARACTERISTIC_UUID),
            [CharacteristicProp::Write],
            *control_value,
            BLE_CONTROL_STORE.init([0_u8; ControlFrame::ENCODED_LEN]),
        )
        .write_permission(PermissionLevel::EncryptionRequired)
        .build();
    project_service.build();

    BleGatt {
        server: AttributeServer::new(table),
        handles: GattHandles {
            status: status_handle,
            metadata: metadata_handle,
            fragment: fragment_handle,
            control: control_handle,
        },
    }
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
async fn run_gatt_connection(
    connection: GattConnection<'_, '_, DefaultPacketPool>,
    runtime: GattRuntime<'_>,
) {
    let mut transfer = BleRecordTransferRuntime::new();

    loop {
        match connection.next().await {
            GattConnectionEvent::Disconnected { reason } => {
                info!("ble central disconnected reason={:?}", reason);
                transfer.reset_after_disconnect();
                runtime
                    .led_runtime_state
                    .signal(BleLedRuntimeState::Advertising);
                break;
            }
            GattConnectionEvent::Gatt { event } => {
                handle_gatt_event(&connection, runtime, &mut transfer, event).await;
            }
            GattConnectionEvent::RequestConnectionParams(_) => {
                warn!("ble connection parameter update request ignored in GATT skeleton");
            }
            GattConnectionEvent::PhyUpdated { .. }
            | GattConnectionEvent::ConnectionParamsUpdated { .. }
            | GattConnectionEvent::DataLengthUpdated { .. } => {}
            GattConnectionEvent::PairingComplete {
                security_level,
                bond,
            } => {
                info!(
                    "ble pairing complete security_level={:?} bonded={=bool} saved_bonds={=usize}",
                    security_level,
                    bond.is_some(),
                    runtime.stack.get_bond_information().len()
                );
                if let Some(bond) = bond {
                    persist_ble_bond_information(&connection, runtime.auth_workspace, &bond).await;
                }
            }
            GattConnectionEvent::PairingFailed(error) => {
                warn!("ble pairing failed error={:?}", error);
            }
            GattConnectionEvent::PassKeyDisplay(_) => {
                warn!("ble passkey display request ignored; device has no display");
            }
            GattConnectionEvent::PassKeyConfirm(_) => {
                warn!("ble passkey confirm request ignored; device has no input");
            }
            GattConnectionEvent::PassKeyInput => {
                warn!("ble passkey input request ignored; device has no input");
            }
        }
    }
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
async fn handle_gatt_event(
    connection: &GattConnection<'_, '_, DefaultPacketPool>,
    runtime: GattRuntime<'_>,
    transfer: &mut BleRecordTransferRuntime,
    event: GattEvent<'_, '_, DefaultPacketPool>,
) {
    match event {
        GattEvent::Read(event) => {
            let handle = event.handle();
            let result = if handle == runtime.handles.metadata.handle {
                prepare_metadata_read(connection, runtime, transfer).await
            } else if handle == runtime.handles.fragment.handle {
                prepare_fragment_read(
                    connection,
                    runtime.server,
                    runtime.handles,
                    runtime.auth_workspace,
                    transfer,
                )
                .await
            } else if handle == runtime.handles.status.handle {
                prepare_status_read(runtime, BleRuntimeState::Connected).await
            } else {
                Ok(())
            };
            send_gatt_reply(
                match result {
                    Ok(()) => event.accept(),
                    Err(error) => event.reject(error),
                },
                handle,
                "read",
            )
            .await;
        }
        GattEvent::Write(event) => {
            let handle = event.handle();
            let result = if handle == runtime.handles.control.handle {
                handle_control_write(connection, runtime, transfer, event.data()).await
            } else {
                Ok(())
            };
            send_gatt_reply(
                match result {
                    Ok(()) => event.accept(),
                    Err(error) => event.reject(error),
                },
                handle,
                "write",
            )
            .await;
        }
        event => {
            let handle = event.payload().handle().unwrap_or(0);
            send_gatt_reply(event.accept(), handle, "other").await;
        }
    }
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
async fn prepare_status_read(
    runtime: GattRuntime<'_>,
    runtime_state: BleRuntimeState,
) -> Result<(), AttErrorCode> {
    set_gatt_status(
        runtime.server,
        runtime.handles,
        runtime.network_upload_status,
        runtime.firmware_status,
        runtime.led_runtime_state,
        runtime_state,
    )
    .await;
    Ok(())
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
async fn send_gatt_reply(
    reply: Result<Reply<'_, DefaultPacketPool>, trouble_host::Error>,
    handle: u16,
    operation: &'static str,
) {
    match reply {
        Ok(reply) => reply.send().await,
        Err(error) => warn!(
            "ble GATT event handling failed op={=str} handle={=u16} error={:?}",
            operation, handle, error
        ),
    }
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
async fn handle_control_write(
    connection: &GattConnection<'_, '_, DefaultPacketPool>,
    runtime: GattRuntime<'_>,
    transfer: &mut BleRecordTransferRuntime,
    data: &[u8],
) -> Result<(), AttErrorCode> {
    ensure_ble_authorized(connection, runtime.auth_workspace).await?;
    let request = ControlFrame::decode(data).map_err(att_error_from_protocol)?;

    match request.opcode {
        ControlOpcode::RequestMetadata => start_metadata_request(runtime, transfer).await,
        ControlOpcode::RequestFragment => {
            prepare_fragment_request(
                connection,
                runtime.server,
                runtime.handles,
                transfer,
                request,
            )
            .await
        }
        ControlOpcode::CompleteRecord => complete_record_request(transfer, request),
        ControlOpcode::AckRecord => {
            ack_record_request(
                runtime.storage_requests,
                runtime.storage_responses,
                runtime.network_upload_status,
                transfer,
                request,
            )
            .await
        }
    }
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
async fn prepare_metadata_read(
    connection: &GattConnection<'_, '_, DefaultPacketPool>,
    runtime: GattRuntime<'_>,
    transfer: &mut BleRecordTransferRuntime,
) -> Result<(), AttErrorCode> {
    ensure_ble_authorized(connection, runtime.auth_workspace).await?;

    if transfer.payload.is_none() {
        start_metadata_request(runtime, transfer).await
    } else {
        let metadata = RecordMetadata::from_payload(
            transfer
                .payload
                .as_ref()
                .ok_or(AttErrorCode::ATTRIBUTE_NOT_FOUND)?,
        )
        .map_err(att_error_from_transfer)?;
        set_metadata_value(runtime.server, runtime.handles, metadata)
    }
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
async fn start_metadata_request(
    runtime: GattRuntime<'_>,
    transfer: &mut BleRecordTransferRuntime,
) -> Result<(), AttErrorCode> {
    let Some(payload) = peek_ble_payload(runtime.storage_requests, runtime.storage_responses).await
    else {
        transfer.reset_after_disconnect();
        return Err(AttErrorCode::ATTRIBUTE_NOT_FOUND);
    };

    let metadata = transfer
        .session
        .start_record(&payload)
        .map_err(att_error_from_transfer)?;
    set_metadata_value(runtime.server, runtime.handles, metadata)?;
    transfer.payload = Some(payload);
    transfer.last_fragment = [0_u8; RecordFragment::HEADER_LEN + MAX_FRAGMENT_PAYLOAD_LEN];
    transfer.last_fragment_len = 0;

    info!(
        "ble metadata prepared sequence={=u64} payload_len={=u16}",
        metadata.sequence, metadata.payload_len
    );
    Ok(())
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
async fn prepare_fragment_read(
    connection: &GattConnection<'_, '_, DefaultPacketPool>,
    server: &GattServer<'static>,
    handles: GattHandles,
    auth_workspace: &'static BleAuthWorkspace,
    transfer: &mut BleRecordTransferRuntime,
) -> Result<(), AttErrorCode> {
    ensure_ble_authorized(connection, auth_workspace).await?;
    if transfer.last_fragment_len == 0 {
        return Err(AttErrorCode::VALUE_NOT_ALLOWED);
    }
    handles
        .fragment
        .set(server, &transfer.last_fragment)
        .map_err(|_| AttErrorCode::UNLIKELY_ERROR)
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
async fn prepare_fragment_request(
    connection: &GattConnection<'_, '_, DefaultPacketPool>,
    server: &GattServer<'static>,
    handles: GattHandles,
    transfer: &mut BleRecordTransferRuntime,
    request: ControlFrame,
) -> Result<(), AttErrorCode> {
    let Some(payload) = transfer.payload.as_ref() else {
        return Err(AttErrorCode::VALUE_NOT_ALLOWED);
    };

    let mut encoded = [0_u8; RecordFragment::HEADER_LEN + MAX_FRAGMENT_PAYLOAD_LEN];
    let encoded_len = transfer
        .session
        .fragment_for_request(payload, request)
        .map_err(att_error_from_transfer)?
        .encode(&mut encoded)
        .map_err(att_error_from_protocol)?;

    transfer.last_fragment = [0_u8; RecordFragment::HEADER_LEN + MAX_FRAGMENT_PAYLOAD_LEN];
    transfer.last_fragment[..encoded_len].copy_from_slice(&encoded[..encoded_len]);
    transfer.last_fragment_len = encoded_len;
    handles
        .fragment
        .set(server, &transfer.last_fragment)
        .map_err(|_| AttErrorCode::UNLIKELY_ERROR)?;

    if let Err(error) = handles
        .fragment
        .notify(connection, &transfer.last_fragment)
        .await
    {
        warn!("ble fragment notify failed error={:?}", error);
    }
    info!(
        "ble fragment prepared sequence={=u64} offset={=u16} encoded_len={=usize}",
        request.sequence, request.offset, encoded_len
    );
    Ok(())
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
fn complete_record_request(
    transfer: &mut BleRecordTransferRuntime,
    request: ControlFrame,
) -> Result<(), AttErrorCode> {
    let Some(payload) = transfer.payload.as_ref() else {
        return Err(AttErrorCode::VALUE_NOT_ALLOWED);
    };
    transfer
        .session
        .complete_record(payload, request)
        .map_err(att_error_from_transfer)?;
    info!(
        "ble record marked complete without storage ACK sequence={=u64}",
        request.sequence
    );
    Ok(())
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
async fn ack_record_request(
    storage_requests: &'static StorageRequestChannel,
    storage_responses: &'static StorageResponseSignal,
    network_upload_status: &'static NetworkUploadStatusMutex,
    transfer: &mut BleRecordTransferRuntime,
    request: ControlFrame,
) -> Result<(), AttErrorCode> {
    let Some(payload) = transfer.payload.as_ref() else {
        return Err(AttErrorCode::VALUE_NOT_ALLOWED);
    };
    let status = { *network_upload_status.lock().await };
    match transfer
        .session
        .ack_action(payload, request, status.network_state, status.upload_result)
        .map_err(att_error_from_transfer)?
    {
        BleAckAction::Suppress => {
            info!(
                "ble storage ACK suppressed sequence={=u64} network_state={:?} upload_result={:?}",
                request.sequence, status.network_state, status.upload_result
            );
            Ok(())
        }
        BleAckAction::SendStorageAck { sequence } => {
            let acked =
                acknowledge_ble_payload(storage_requests, storage_responses, sequence).await?;
            info!(
                "ble storage ACK requested sequence={=u64} acked={=bool}",
                sequence, acked
            );
            Ok(())
        }
    }
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
fn set_metadata_value(
    server: &GattServer<'static>,
    handles: GattHandles,
    metadata: RecordMetadata,
) -> Result<(), AttErrorCode> {
    let mut encoded = [0_u8; RecordMetadata::ENCODED_LEN];
    metadata
        .encode(&mut encoded)
        .map_err(att_error_from_protocol)?;
    handles
        .metadata
        .set(server, &encoded)
        .map_err(|_| AttErrorCode::UNLIKELY_ERROR)
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
const fn att_error_from_protocol(error: BleProtocolError) -> AttErrorCode {
    match error {
        BleProtocolError::BufferTooSmall => AttErrorCode::INSUFFICIENT_RESOURCES,
        BleProtocolError::FragmentTooLarge => AttErrorCode::INVALID_ATTRIBUTE_VALUE_LENGTH,
        BleProtocolError::FragmentOutOfBounds => AttErrorCode::OUT_OF_RANGE,
        BleProtocolError::BadLength => AttErrorCode::INVALID_ATTRIBUTE_VALUE_LENGTH,
        BleProtocolError::UnsupportedVersion | BleProtocolError::UnknownOpcode => {
            AttErrorCode::VALUE_NOT_ALLOWED
        }
    }
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
const fn att_error_from_transfer(error: BleTransferError) -> AttErrorCode {
    match error {
        BleTransferError::PayloadTooLarge => AttErrorCode::INSUFFICIENT_RESOURCES,
        BleTransferError::NoActiveRecord => AttErrorCode::VALUE_NOT_ALLOWED,
        BleTransferError::SequenceMismatch => AttErrorCode::VALUE_NOT_ALLOWED,
        BleTransferError::UnexpectedOpcode => AttErrorCode::VALUE_NOT_ALLOWED,
        BleTransferError::InvalidFragmentLength => AttErrorCode::INVALID_ATTRIBUTE_VALUE_LENGTH,
        BleTransferError::FragmentOutOfBounds => AttErrorCode::OUT_OF_RANGE,
        BleTransferError::FragmentOutOfOrder => AttErrorCode::VALUE_NOT_ALLOWED,
        BleTransferError::TransferIncomplete => AttErrorCode::VALUE_NOT_ALLOWED,
    }
}
