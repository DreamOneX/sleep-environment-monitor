use crate::{
    storage::spool::crc32,
    tasks::storage::StoredPayload,
    types::{ErrorFlags, FirmwareStatusSnapshot, NetworkState, NetworkUploadStatus, UploadResult},
};

pub const SERVICE_UUID: [u8; 16] = [
    0x53, 0x45, 0x4d, 0x53, 0x24, 0x0a, 0x4b, 0x1e, 0x9b, 0xb2, 0x51, 0x0e, 0x7d, 0x01, 0x00, 0x01,
];
pub const STATUS_CHARACTERISTIC_UUID: [u8; 16] = [
    0x53, 0x45, 0x4d, 0x53, 0x24, 0x0a, 0x4b, 0x1e, 0x9b, 0xb2, 0x51, 0x0e, 0x7d, 0x01, 0x01, 0x01,
];
pub const RECORD_METADATA_CHARACTERISTIC_UUID: [u8; 16] = [
    0x53, 0x45, 0x4d, 0x53, 0x24, 0x0a, 0x4b, 0x1e, 0x9b, 0xb2, 0x51, 0x0e, 0x7d, 0x01, 0x02, 0x01,
];
pub const RECORD_FRAGMENT_CHARACTERISTIC_UUID: [u8; 16] = [
    0x53, 0x45, 0x4d, 0x53, 0x24, 0x0a, 0x4b, 0x1e, 0x9b, 0xb2, 0x51, 0x0e, 0x7d, 0x01, 0x03, 0x01,
];
pub const CONTROL_CHARACTERISTIC_UUID: [u8; 16] = [
    0x53, 0x45, 0x4d, 0x53, 0x24, 0x0a, 0x4b, 0x1e, 0x9b, 0xb2, 0x51, 0x0e, 0x7d, 0x01, 0x04, 0x01,
];

pub const PROTOCOL_VERSION: u8 = 1;
pub const MAX_FRAGMENT_PAYLOAD_LEN: usize = crate::config::ble::MAX_FRAGMENT_PAYLOAD_LEN;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[cfg_attr(target_arch = "riscv32", derive(defmt::Format))]
pub enum BleRuntimeState {
    #[default]
    Disabled,
    ControllerReady,
    HostPending,
    Advertising,
    Connected,
    Error,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(target_arch = "riscv32", derive(defmt::Format))]
pub struct BleStatus {
    pub runtime_state: BleRuntimeState,
    pub network_state: NetworkState,
    pub upload_result: UploadResult,
    pub pending_record_count: u16,
    pub error_flags: ErrorFlags,
    pub boot_button_state: BootButtonState,
    pub pairing_state: BlePairingState,
    pub boot_pressed_millis: u32,
}

impl BleStatus {
    pub const LEGACY_ENCODED_LEN: usize = 10;
    pub const ENCODED_LEN: usize = 20;

    pub const fn new(
        runtime_state: BleRuntimeState,
        network_state: NetworkState,
        upload_result: UploadResult,
        pending_record_count: u16,
        error_flags: ErrorFlags,
    ) -> Self {
        Self {
            runtime_state,
            network_state,
            upload_result,
            pending_record_count,
            error_flags,
            boot_button_state: BootButtonState::Released,
            pairing_state: BlePairingState::Closed,
            boot_pressed_millis: 0,
        }
    }

    pub const fn with_pairing(
        self,
        boot_button_state: BootButtonState,
        pairing_state: BlePairingState,
        boot_pressed_millis: u32,
    ) -> Self {
        Self {
            boot_button_state,
            pairing_state,
            boot_pressed_millis,
            ..self
        }
    }

    pub fn encode(self, out: &mut [u8]) -> Result<usize, BleProtocolError> {
        if out.len() < Self::ENCODED_LEN {
            return Err(BleProtocolError::BufferTooSmall);
        }

        out[0] = PROTOCOL_VERSION;
        out[1] = self.runtime_state as u8;
        out[2] = network_state_code(self.network_state);
        out[3] = upload_result_code(self.upload_result);
        out[4..6].copy_from_slice(&self.pending_record_count.to_le_bytes());
        out[6..10].copy_from_slice(&self.error_flags.bits().to_le_bytes());
        out[10] = pairing_state_code(self.pairing_state);
        out[11] = boot_button_state_code(self.boot_button_state);
        out[12..16].copy_from_slice(&pairing_remaining_millis(self.pairing_state).to_le_bytes());
        out[16..20].copy_from_slice(&self.boot_pressed_millis.to_le_bytes());

        Ok(Self::ENCODED_LEN)
    }
}

pub const fn status_from_snapshots(
    runtime_state: BleRuntimeState,
    network_upload_status: NetworkUploadStatus,
    firmware_status: FirmwareStatusSnapshot,
) -> BleStatus {
    BleStatus::new(
        runtime_state,
        network_upload_status.network_state,
        network_upload_status.upload_result,
        firmware_status.pending_record_count,
        firmware_status.error_flags,
    )
}

pub const fn status_from_snapshots_and_pairing(
    runtime_state: BleRuntimeState,
    network_upload_status: NetworkUploadStatus,
    firmware_status: FirmwareStatusSnapshot,
    boot_button_state: BootButtonState,
    pairing_state: BlePairingState,
    boot_pressed_millis: u32,
) -> BleStatus {
    status_from_snapshots(runtime_state, network_upload_status, firmware_status).with_pairing(
        boot_button_state,
        pairing_state,
        boot_pressed_millis,
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(target_arch = "riscv32", derive(defmt::Format))]
pub struct RecordMetadata {
    pub sequence: u64,
    pub payload_len: u16,
    pub payload_flags: u8,
    pub payload_crc32: u32,
    pub current_boot: bool,
}

impl RecordMetadata {
    pub const ENCODED_LEN: usize = 17;

    pub const fn new(
        sequence: u64,
        payload_len: u16,
        payload_flags: u8,
        payload_crc32: u32,
        current_boot: bool,
    ) -> Self {
        Self {
            sequence,
            payload_len,
            payload_flags,
            payload_crc32,
            current_boot,
        }
    }

    pub fn encode(self, out: &mut [u8]) -> Result<usize, BleProtocolError> {
        if out.len() < Self::ENCODED_LEN {
            return Err(BleProtocolError::BufferTooSmall);
        }

        out[0] = PROTOCOL_VERSION;
        out[1..9].copy_from_slice(&self.sequence.to_le_bytes());
        out[9..11].copy_from_slice(&self.payload_len.to_le_bytes());
        out[11] = self.payload_flags;
        out[12..16].copy_from_slice(&self.payload_crc32.to_le_bytes());
        out[16] = u8::from(self.current_boot);

        Ok(Self::ENCODED_LEN)
    }

    pub fn from_payload<const PAYLOAD_SIZE: usize>(
        payload: &StoredPayload<PAYLOAD_SIZE>,
    ) -> Result<Self, BleTransferError> {
        let payload_len =
            u16::try_from(payload.payload_len).map_err(|_| BleTransferError::PayloadTooLarge)?;

        Ok(Self::new(
            payload.sequence,
            payload_len,
            payload.payload_flags,
            crc32(payload.as_slice()),
            payload.current_boot,
        ))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(target_arch = "riscv32", derive(defmt::Format))]
pub struct RecordFragment<'a> {
    pub sequence: u64,
    pub offset: u16,
    pub total_len: u16,
    pub payload: &'a [u8],
}

impl<'a> RecordFragment<'a> {
    pub const HEADER_LEN: usize = 13;

    pub const fn new(
        sequence: u64,
        offset: u16,
        total_len: u16,
        payload: &'a [u8],
    ) -> Result<Self, BleProtocolError> {
        if payload.len() > MAX_FRAGMENT_PAYLOAD_LEN {
            return Err(BleProtocolError::FragmentTooLarge);
        }
        if payload.len() > u16::MAX as usize {
            return Err(BleProtocolError::FragmentTooLarge);
        }
        if offset as usize + payload.len() > total_len as usize {
            return Err(BleProtocolError::FragmentOutOfBounds);
        }

        Ok(Self {
            sequence,
            offset,
            total_len,
            payload,
        })
    }

    pub fn encode(self, out: &mut [u8]) -> Result<usize, BleProtocolError> {
        let encoded_len = Self::HEADER_LEN
            .checked_add(self.payload.len())
            .ok_or(BleProtocolError::FragmentTooLarge)?;
        if out.len() < encoded_len {
            return Err(BleProtocolError::BufferTooSmall);
        }

        out[0] = PROTOCOL_VERSION;
        out[1..9].copy_from_slice(&self.sequence.to_le_bytes());
        out[9..11].copy_from_slice(&self.offset.to_le_bytes());
        out[11..13].copy_from_slice(&(self.payload.len() as u16).to_le_bytes());
        out[13..encoded_len].copy_from_slice(self.payload);

        Ok(encoded_len)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(target_arch = "riscv32", derive(defmt::Format))]
pub enum ControlOpcode {
    RequestMetadata = 1,
    RequestFragment = 2,
    CompleteRecord = 3,
    AckRecord = 4,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(target_arch = "riscv32", derive(defmt::Format))]
pub struct ControlFrame {
    pub opcode: ControlOpcode,
    pub sequence: u64,
    pub offset: u16,
    pub length: u16,
}

impl ControlFrame {
    pub const ENCODED_LEN: usize = 14;

    pub const fn new(opcode: ControlOpcode, sequence: u64, offset: u16, length: u16) -> Self {
        Self {
            opcode,
            sequence,
            offset,
            length,
        }
    }

    pub fn encode(self, out: &mut [u8]) -> Result<usize, BleProtocolError> {
        if out.len() < Self::ENCODED_LEN {
            return Err(BleProtocolError::BufferTooSmall);
        }

        out[0] = PROTOCOL_VERSION;
        out[1] = self.opcode as u8;
        out[2..10].copy_from_slice(&self.sequence.to_le_bytes());
        out[10..12].copy_from_slice(&self.offset.to_le_bytes());
        out[12..14].copy_from_slice(&self.length.to_le_bytes());

        Ok(Self::ENCODED_LEN)
    }

    pub fn decode(input: &[u8]) -> Result<Self, BleProtocolError> {
        if input.len() != Self::ENCODED_LEN {
            return Err(BleProtocolError::BadLength);
        }
        if input[0] != PROTOCOL_VERSION {
            return Err(BleProtocolError::UnsupportedVersion);
        }

        let opcode = match input[1] {
            1 => ControlOpcode::RequestMetadata,
            2 => ControlOpcode::RequestFragment,
            3 => ControlOpcode::CompleteRecord,
            4 => ControlOpcode::AckRecord,
            _ => return Err(BleProtocolError::UnknownOpcode),
        };

        Ok(Self {
            opcode,
            sequence: u64::from_le_bytes([
                input[2], input[3], input[4], input[5], input[6], input[7], input[8], input[9],
            ]),
            offset: u16::from_le_bytes([input[10], input[11]]),
            length: u16::from_le_bytes([input[12], input[13]]),
        })
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[cfg_attr(target_arch = "riscv32", derive(defmt::Format))]
pub enum BleTransferState {
    #[default]
    Idle,
    Active {
        sequence: u64,
        total_len: u16,
        delivered_until: u16,
        complete: bool,
        ack_sent: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(target_arch = "riscv32", derive(defmt::Format))]
pub enum BleAckAction {
    Suppress,
    SendStorageAck { sequence: u64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(target_arch = "riscv32", derive(defmt::Format))]
pub enum BleTransferError {
    PayloadTooLarge,
    NoActiveRecord,
    SequenceMismatch,
    UnexpectedOpcode,
    InvalidFragmentLength,
    FragmentOutOfBounds,
    FragmentOutOfOrder,
    TransferIncomplete,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[cfg_attr(target_arch = "riscv32", derive(defmt::Format))]
pub struct BleTransferSession {
    state: BleTransferState,
}

impl BleTransferSession {
    pub const fn new() -> Self {
        Self {
            state: BleTransferState::Idle,
        }
    }

    pub const fn state(&self) -> BleTransferState {
        self.state
    }

    pub fn reset_after_disconnect(&mut self) {
        self.state = BleTransferState::Idle;
    }

    pub fn start_record<const PAYLOAD_SIZE: usize>(
        &mut self,
        payload: &StoredPayload<PAYLOAD_SIZE>,
    ) -> Result<RecordMetadata, BleTransferError> {
        let metadata = RecordMetadata::from_payload(payload)?;
        self.state = BleTransferState::Active {
            sequence: metadata.sequence,
            total_len: metadata.payload_len,
            delivered_until: 0,
            complete: metadata.payload_len == 0,
            ack_sent: false,
        };

        Ok(metadata)
    }

    pub fn fragment_for_request<'a, const PAYLOAD_SIZE: usize>(
        &mut self,
        payload: &'a StoredPayload<PAYLOAD_SIZE>,
        request: ControlFrame,
    ) -> Result<RecordFragment<'a>, BleTransferError> {
        if request.opcode != ControlOpcode::RequestFragment {
            return Err(BleTransferError::UnexpectedOpcode);
        }
        if request.length == 0 {
            return Err(BleTransferError::InvalidFragmentLength);
        }

        let (sequence, total_len, delivered_until) = self.active_record_state()?;
        if request.sequence != sequence || payload.sequence != sequence {
            return Err(BleTransferError::SequenceMismatch);
        }

        let offset = request.offset as usize;
        let payload_len = payload.payload_len;
        if offset > payload_len || offset > total_len as usize {
            return Err(BleTransferError::FragmentOutOfBounds);
        }
        if request.offset > delivered_until {
            return Err(BleTransferError::FragmentOutOfOrder);
        }

        let remaining = payload_len.saturating_sub(offset);
        let fragment_len = remaining
            .min(request.length as usize)
            .min(MAX_FRAGMENT_PAYLOAD_LEN);
        if fragment_len == 0 && offset != payload_len {
            return Err(BleTransferError::InvalidFragmentLength);
        }

        let fragment = RecordFragment::new(
            sequence,
            request.offset,
            total_len,
            &payload.as_slice()[offset..offset + fragment_len],
        )
        .map_err(transfer_error_from_protocol)?;

        self.mark_delivered(request.offset.saturating_add(fragment_len as u16));
        Ok(fragment)
    }

    pub fn complete_record<const PAYLOAD_SIZE: usize>(
        &mut self,
        payload: &StoredPayload<PAYLOAD_SIZE>,
        request: ControlFrame,
    ) -> Result<(), BleTransferError> {
        if request.opcode != ControlOpcode::CompleteRecord {
            return Err(BleTransferError::UnexpectedOpcode);
        }

        let (sequence, total_len, delivered_until) = self.active_record_state()?;
        if request.sequence != sequence || payload.sequence != sequence {
            return Err(BleTransferError::SequenceMismatch);
        }
        if delivered_until < total_len || payload.payload_len != total_len as usize {
            return Err(BleTransferError::TransferIncomplete);
        }

        self.mark_complete();
        Ok(())
    }

    pub fn ack_action<const PAYLOAD_SIZE: usize>(
        &mut self,
        payload: &StoredPayload<PAYLOAD_SIZE>,
        request: ControlFrame,
        network_state: NetworkState,
        upload_result: UploadResult,
    ) -> Result<BleAckAction, BleTransferError> {
        if request.opcode != ControlOpcode::AckRecord {
            return Err(BleTransferError::UnexpectedOpcode);
        }

        let BleTransferState::Active {
            sequence,
            complete,
            ack_sent,
            ..
        } = self.state
        else {
            return Err(BleTransferError::NoActiveRecord);
        };

        if request.sequence != sequence || payload.sequence != sequence {
            return Err(BleTransferError::SequenceMismatch);
        }
        if !complete {
            return Err(BleTransferError::TransferIncomplete);
        }
        if ack_policy(network_state, upload_result) == BleAckPolicy::CopyOnly || ack_sent {
            return Ok(BleAckAction::Suppress);
        }

        self.mark_ack_sent();
        Ok(BleAckAction::SendStorageAck { sequence })
    }

    fn active_record_state(&self) -> Result<(u64, u16, u16), BleTransferError> {
        match self.state {
            BleTransferState::Active {
                sequence,
                total_len,
                delivered_until,
                ..
            } => Ok((sequence, total_len, delivered_until)),
            BleTransferState::Idle => Err(BleTransferError::NoActiveRecord),
        }
    }

    fn mark_delivered(&mut self, delivered: u16) {
        if let BleTransferState::Active {
            delivered_until, ..
        } = &mut self.state
        {
            *delivered_until = (*delivered_until).max(delivered);
        }
    }

    fn mark_complete(&mut self) {
        if let BleTransferState::Active { complete, .. } = &mut self.state {
            *complete = true;
        }
    }

    fn mark_ack_sent(&mut self) {
        if let BleTransferState::Active { ack_sent, .. } = &mut self.state {
            *ack_sent = true;
        }
    }
}

const fn transfer_error_from_protocol(error: BleProtocolError) -> BleTransferError {
    match error {
        BleProtocolError::FragmentTooLarge => BleTransferError::InvalidFragmentLength,
        BleProtocolError::FragmentOutOfBounds => BleTransferError::FragmentOutOfBounds,
        BleProtocolError::BufferTooSmall
        | BleProtocolError::BadLength
        | BleProtocolError::UnsupportedVersion
        | BleProtocolError::UnknownOpcode => BleTransferError::InvalidFragmentLength,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(target_arch = "riscv32", derive(defmt::Format))]
pub enum BleAckPolicy {
    CopyOnly,
    CanAckOldestRecord,
}

pub const fn ack_policy(network_state: NetworkState, upload_result: UploadResult) -> BleAckPolicy {
    match (network_state, upload_result) {
        (NetworkState::Connected | NetworkState::IpReady, UploadResult::Success) => {
            BleAckPolicy::CopyOnly
        }
        _ => BleAckPolicy::CanAckOldestRecord,
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[cfg_attr(target_arch = "riscv32", derive(defmt::Format))]
pub enum BootButtonState {
    Pressed,
    #[default]
    Released,
}

impl BootButtonState {
    pub const fn from_active_low(is_low: bool) -> Self {
        if is_low {
            Self::Pressed
        } else {
            Self::Released
        }
    }

    pub const fn is_pressed(self) -> bool {
        matches!(self, Self::Pressed)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[cfg_attr(target_arch = "riscv32", derive(defmt::Format))]
pub enum BlePairingState {
    #[default]
    Closed,
    Open {
        remaining_millis: u64,
    },
}

impl BlePairingState {
    pub const fn is_open(self) -> bool {
        matches!(self, Self::Open { .. })
    }

    pub const fn remaining_millis(self) -> u64 {
        match self {
            Self::Closed => 0,
            Self::Open { remaining_millis } => remaining_millis,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(target_arch = "riscv32", derive(defmt::Format))]
pub enum BlePairingEvent {
    None,
    WindowOpened,
    WindowExpired,
    AuthRecordsClearRequested,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BlePairingGesture {
    hold_millis: u64,
    clear_hold_millis: u64,
    window_millis: u64,
    pressed_millis: u64,
    opened_for_current_press: bool,
    cleared_for_current_press: bool,
    state: BlePairingState,
}

impl BlePairingGesture {
    pub const fn new(hold_millis: u64, clear_hold_millis: u64, window_millis: u64) -> Self {
        Self {
            hold_millis,
            clear_hold_millis,
            window_millis,
            pressed_millis: 0,
            opened_for_current_press: false,
            cleared_for_current_press: false,
            state: BlePairingState::Closed,
        }
    }

    pub const fn state(&self) -> BlePairingState {
        self.state
    }

    pub const fn pressed_millis(&self) -> u64 {
        self.pressed_millis
    }

    pub fn open_window(&mut self) -> BlePairingEvent {
        self.state = BlePairingState::Open {
            remaining_millis: self.window_millis,
        };
        self.opened_for_current_press = true;
        BlePairingEvent::WindowOpened
    }

    pub fn update(
        &mut self,
        button_state: BootButtonState,
        elapsed_millis: u64,
    ) -> BlePairingEvent {
        let expired = self.tick_pairing_window(elapsed_millis);

        if button_state.is_pressed() {
            self.pressed_millis = self.pressed_millis.saturating_add(elapsed_millis);
            if self.clear_hold_millis > 0
                && !self.cleared_for_current_press
                && self.pressed_millis >= self.clear_hold_millis
            {
                self.cleared_for_current_press = true;
                return BlePairingEvent::AuthRecordsClearRequested;
            }
            if !self.opened_for_current_press && self.pressed_millis >= self.hold_millis {
                return self.open_window();
            }
        } else {
            self.pressed_millis = 0;
            self.opened_for_current_press = false;
            self.cleared_for_current_press = false;
        }

        if expired {
            BlePairingEvent::WindowExpired
        } else {
            BlePairingEvent::None
        }
    }

    fn tick_pairing_window(&mut self, elapsed_millis: u64) -> bool {
        let BlePairingState::Open { remaining_millis } = self.state else {
            return false;
        };

        let remaining_millis = remaining_millis.saturating_sub(elapsed_millis);
        if remaining_millis == 0 {
            self.state = BlePairingState::Closed;
            true
        } else {
            self.state = BlePairingState::Open { remaining_millis };
            false
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(target_arch = "riscv32", derive(defmt::Format))]
pub enum BleProtocolError {
    BufferTooSmall,
    FragmentTooLarge,
    FragmentOutOfBounds,
    BadLength,
    UnsupportedVersion,
    UnknownOpcode,
}

const fn network_state_code(state: NetworkState) -> u8 {
    match state {
        NetworkState::Disconnected => 0,
        NetworkState::Connecting => 1,
        NetworkState::Connected => 2,
        NetworkState::IpReady => 3,
    }
}

const fn upload_result_code(result: UploadResult) -> u8 {
    match result {
        UploadResult::Idle => 0,
        UploadResult::Success => 1,
        UploadResult::Failed => 2,
        UploadResult::DiscoveryFailed => 3,
        UploadResult::TimeFailed => 4,
        UploadResult::TransportFailed => 5,
        UploadResult::HttpFailed => 6,
    }
}

const fn pairing_state_code(state: BlePairingState) -> u8 {
    match state {
        BlePairingState::Closed => 0,
        BlePairingState::Open { .. } => 1,
    }
}

const fn boot_button_state_code(state: BootButtonState) -> u8 {
    match state {
        BootButtonState::Released => 0,
        BootButtonState::Pressed => 1,
    }
}

const fn pairing_remaining_millis(state: BlePairingState) -> u32 {
    match state {
        BlePairingState::Closed => 0,
        BlePairingState::Open { remaining_millis } => {
            if remaining_millis > u32::MAX as u64 {
                u32::MAX
            } else {
                remaining_millis as u32
            }
        }
    }
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
const fn saturating_u64_to_u32(value: u64) -> u32 {
    if value > u32::MAX as u64 {
        u32::MAX
    } else {
        value as u32
    }
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
use crate::tasks::{
    FirmwareStatusSnapshotMutex, NetworkUploadStatusMutex, StorageRequestChannel,
    StorageResponseSignal, TaskSignal,
    storage::{StorageClient, StorageCommand, StorageResponse},
};
#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
use crate::{
    drivers::flash::RomBleAuthFlash,
    storage::ble_auth::{
        AUTH_HEADER_LEN, AUTH_RECORD_LEN, BleAuthAddressKind, BleAuthRecord, BleAuthRecordStatus,
        BleAuthRecordUpsert, BleAuthSecurityLevel, clear_auth_records, load_auth_records,
        should_auto_open_pairing_window, store_auth_records, upsert_auth_record,
    },
    util::status::{BleLedPairingStatus, BleLedRuntimeState},
};
#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
use defmt::{error, info, warn};
#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
use embassy_futures::select::{Either, select};
#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};
#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
use embassy_time::{Duration, Timer};
#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
use esp_hal::gpio::Input;
#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
use esp_radio::ble::controller::BleConnector;
#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
use rand_core_06::{CryptoRng, Error as RandError, RngCore};
#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
use static_cell::StaticCell;
#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
use trouble_host::prelude::*;

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
const GATT_CONNECTIONS_MAX: usize = 1;
#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
const GATT_L2CAP_CHANNELS_MAX: usize = 3;
#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
const GATT_ATTRIBUTE_MAX: usize = 17;
#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
const GATT_CCCD_MAX: usize = 2;
#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
const GATT_COMMAND_SLOTS: usize = 10;
#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
const GAP_APPEARANCE_GENERIC_SENSOR: [u8; 2] = [0x40, 0x05];
#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
const RANDOM_STATIC_ADDRESS: [u8; 6] = [0xc3, 0xe2, 0x24, 0x10, 0x53, 0xf3];
#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
const BLE_AUTH_WORKSPACE_LEN: usize =
    AUTH_HEADER_LEN + AUTH_RECORD_LEN * crate::config::ble::AUTH_RECORD_CAPACITY;

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
type BleController = ExternalController<BleConnector<'static>, GATT_COMMAND_SLOTS>;
#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
type BleHostResources =
    HostResources<DefaultPacketPool, GATT_CONNECTIONS_MAX, GATT_L2CAP_CHANNELS_MAX>;
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
pub struct BleAuthFlashWorkspace {
    records: [BleAuthRecord; crate::config::ble::AUTH_RECORD_CAPACITY],
    status: BleAuthRecordStatus,
    record_count: usize,
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
impl BleAuthFlashWorkspace {
    const fn new() -> Self {
        Self {
            records: [BleAuthRecord::EMPTY; crate::config::ble::AUTH_RECORD_CAPACITY],
            status: BleAuthRecordStatus::Missing,
            record_count: 0,
        }
    }
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
pub type BleAuthWorkspace = Mutex<CriticalSectionRawMutex, BleAuthFlashWorkspace>;

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
static BLE_HOST_RESOURCES: StaticCell<BleHostResources> = StaticCell::new();
#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
static BLE_AUTH_WORKSPACE: StaticCell<BleAuthWorkspace> = StaticCell::new();
#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
static BLE_PAIRING_STATE: Mutex<CriticalSectionRawMutex, BlePairingState> =
    Mutex::new(BlePairingState::Closed);
#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
static BLE_BOOT_BUTTON_STATE: Mutex<CriticalSectionRawMutex, BootButtonState> =
    Mutex::new(BootButtonState::Released);
#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
static BLE_BOOT_PRESSED_MILLIS: Mutex<CriticalSectionRawMutex, u32> = Mutex::new(0);
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
pub fn ble_auth_workspace() -> &'static BleAuthWorkspace {
    BLE_AUTH_WORKSPACE.init_with(|| Mutex::new(BleAuthFlashWorkspace::new()))
}

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
async fn clear_saved_ble_auth_records(auth_workspace: &'static BleAuthWorkspace) -> bool {
    let mut flash = match RomBleAuthFlash::new() {
        Ok(flash) => flash,
        Err(error) => {
            warn!("ble auth flash init failed before clear error={:?}", error);
            return false;
        }
    };
    let offset = flash.absolute_offset();
    let len = flash.len();
    match clear_auth_records(&mut flash) {
        Ok(()) => {
            let mut workspace = auth_workspace.lock().await;
            workspace.records = [BleAuthRecord::EMPTY; crate::config::ble::AUTH_RECORD_CAPACITY];
            workspace.status = BleAuthRecordStatus::Missing;
            workspace.record_count = 0;
            info!(
                "ble auth records cleared offset=0x{:08x} len={=usize}",
                offset, len
            );
            true
        }
        Err(error) => {
            warn!(
                "ble auth records clear failed error={:?} offset=0x{:08x} len={=usize}",
                error, offset, len
            );
            false
        }
    }
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
fn should_auto_open_pairing_window_on_boot() -> bool {
    let flash = match RomBleAuthFlash::new() {
        Ok(flash) => flash,
        Err(error) => {
            warn!("ble auth flash init failed error={:?}", error);
            return false;
        }
    };
    let mut records = [BleAuthRecord::EMPTY; crate::config::ble::AUTH_RECORD_CAPACITY];
    let mut scratch = [0_u8; BLE_AUTH_WORKSPACE_LEN];
    let load = match load_auth_records(
        &flash,
        crate::config::ble::AUTH_RECORDS_VERSION,
        crate::config::ble::AUTH_RECORDS_CHECKSUM,
        &mut records,
        &mut scratch,
    ) {
        Ok(load) => load,
        Err(error) => {
            warn!("ble auth records load failed error={:?}", error);
            return false;
        }
    };
    let should_open = should_auto_open_pairing_window(
        crate::config::ble::AUTO_PAIR_ON_AUTH_RECORD_RESET,
        load.status,
    );
    info!(
        "ble auth records status={:?} count={=usize} auto_pair={=bool} offset=0x{:08x} len={=usize}",
        load.status,
        load.record_count,
        should_open,
        flash.absolute_offset(),
        flash.len()
    );
    should_open
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
async fn restore_ble_auth_records(
    stack: &Stack<'_, BleController, DefaultPacketPool>,
    auth_workspace: &'static BleAuthWorkspace,
) {
    let flash = match RomBleAuthFlash::new() {
        Ok(flash) => flash,
        Err(error) => {
            warn!("ble auth flash init failed error={:?}", error);
            return;
        }
    };
    let mut records = [BleAuthRecord::EMPTY; crate::config::ble::AUTH_RECORD_CAPACITY];
    let mut scratch = [0_u8; BLE_AUTH_WORKSPACE_LEN];
    let load = match load_auth_records(
        &flash,
        crate::config::ble::AUTH_RECORDS_VERSION,
        crate::config::ble::AUTH_RECORDS_CHECKSUM,
        &mut records,
        &mut scratch,
    ) {
        Ok(load) => load,
        Err(error) => {
            warn!("ble auth records load failed error={:?}", error);
            let mut workspace = auth_workspace.lock().await;
            workspace.status = BleAuthRecordStatus::Missing;
            workspace.record_count = 0;
            return;
        }
    };

    let mut restored_count = 0_usize;
    for record in records[..load.record_count].iter().copied() {
        match stack.add_bond_information(bond_information_from_auth_record(record)) {
            Ok(()) => restored_count += 1,
            Err(error) => warn!("ble auth bond restore failed error={:?}", error),
        }
    }
    {
        let mut workspace = auth_workspace.lock().await;
        workspace.records = records;
        workspace.status = load.status;
        workspace.record_count = load.record_count;
    }

    info!(
        "ble auth records restored status={:?} loaded={=usize} restored={=usize} offset=0x{:08x} len={=usize}",
        load.status,
        load.record_count,
        restored_count,
        flash.absolute_offset(),
        flash.len()
    );
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
async fn persist_ble_bond_information(
    connection: &GattConnection<'_, '_, DefaultPacketPool>,
    auth_workspace: &'static BleAuthWorkspace,
    bond: &BondInformation,
) {
    let mut flash = match RomBleAuthFlash::new() {
        Ok(flash) => flash,
        Err(error) => {
            warn!(
                "ble auth flash init failed before bond store error={:?}",
                error
            );
            return;
        }
    };
    let record = auth_record_from_bond(connection.raw().peer_addr_kind(), bond);
    let (mut records, mut store_count) = {
        let workspace = auth_workspace.lock().await;
        let record_count = if matches!(workspace.status, BleAuthRecordStatus::Valid { .. }) {
            workspace.record_count
        } else {
            0
        };
        (workspace.records, record_count)
    };

    let (next_store_count, upsert_result) = upsert_auth_record(&mut records, store_count, record);
    store_count = next_store_count;
    match upsert_result {
        BleAuthRecordUpsert::Updated { index } => {
            info!("ble auth record updated index={=usize}", index);
        }
        BleAuthRecordUpsert::Appended { index } => {
            info!("ble auth record appended index={=usize}", index);
        }
        BleAuthRecordUpsert::ReplacedOldest { index } => {
            warn!(
                "ble auth record capacity full; replacing oldest bond record index={=usize}",
                index
            );
        }
        BleAuthRecordUpsert::NoCapacity => {
            warn!("ble auth record capacity is zero; bond record not stored");
            return;
        }
    }

    let mut scratch = [0_u8; BLE_AUTH_WORKSPACE_LEN];
    match store_auth_records(
        &mut flash,
        crate::config::ble::AUTH_RECORDS_VERSION,
        crate::config::ble::AUTH_RECORDS_CHECKSUM,
        &records[..store_count],
        &mut scratch,
    ) {
        Ok(()) => {
            let mut workspace = auth_workspace.lock().await;
            workspace.records = records;
            workspace.record_count = store_count;
            workspace.status = BleAuthRecordStatus::Valid {
                records_version: crate::config::ble::AUTH_RECORDS_VERSION,
                record_count: store_count as u16,
            };
            info!(
                "ble auth bond stored count={=usize} offset=0x{:08x} len={=usize}",
                store_count,
                flash.absolute_offset(),
                flash.len()
            );
        }
        Err(error) => warn!("ble auth bond store failed error={:?}", error),
    }
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
fn bond_information_from_auth_record(record: BleAuthRecord) -> BondInformation {
    BondInformation::new(
        Identity {
            bd_addr: BdAddr::new(record.identity_address),
            irk: record
                .identity_resolving_key
                .map(IdentityResolvingKey::from_le_bytes),
        },
        LongTermKey::from_le_bytes(record.long_term_key),
        security_level_from_auth_record(record.security_level),
        record.bonded,
    )
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
fn auth_record_from_bond(address_kind: AddrKind, bond: &BondInformation) -> BleAuthRecord {
    BleAuthRecord {
        address_kind: auth_address_kind_from_addr_kind(address_kind),
        identity_address: bond.identity.bd_addr.into_inner(),
        long_term_key: bond.ltk.to_le_bytes(),
        identity_resolving_key: bond.identity.irk.map(IdentityResolvingKey::to_le_bytes),
        security_level: auth_security_level_from_connection(bond.security_level),
        bonded: bond.is_bonded,
    }
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
fn auth_address_kind_from_addr_kind(address_kind: AddrKind) -> BleAuthAddressKind {
    if address_kind == AddrKind::RANDOM {
        BleAuthAddressKind::Random
    } else {
        BleAuthAddressKind::Public
    }
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
const fn security_level_from_auth_record(level: BleAuthSecurityLevel) -> SecurityLevel {
    match level {
        BleAuthSecurityLevel::NoEncryption => SecurityLevel::NoEncryption,
        BleAuthSecurityLevel::Encrypted => SecurityLevel::Encrypted,
        BleAuthSecurityLevel::EncryptedAuthenticated => SecurityLevel::EncryptedAuthenticated,
    }
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
const fn auth_security_level_from_connection(level: SecurityLevel) -> BleAuthSecurityLevel {
    match level {
        SecurityLevel::NoEncryption => BleAuthSecurityLevel::NoEncryption,
        SecurityLevel::Encrypted => BleAuthSecurityLevel::Encrypted,
        SecurityLevel::EncryptedAuthenticated => BleAuthSecurityLevel::EncryptedAuthenticated,
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
async fn peek_ble_payload(
    storage_requests: &'static StorageRequestChannel,
    storage_responses: &'static StorageResponseSignal,
) -> Option<StoredPayload> {
    storage_requests
        .send(StorageCommand::Peek(StorageClient::Ble))
        .await;
    match storage_responses.wait().await {
        StorageResponse::Peeked(payload) => payload,
        StorageResponse::Acked(_) => None,
        StorageResponse::Error(error) => {
            warn!("ble storage peek failed error={:?}", error);
            None
        }
    }
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
async fn acknowledge_ble_payload(
    storage_requests: &'static StorageRequestChannel,
    storage_responses: &'static StorageResponseSignal,
    sequence: u64,
) -> Result<bool, AttErrorCode> {
    storage_requests
        .send(StorageCommand::Ack {
            client: StorageClient::Ble,
            sequence,
        })
        .await;
    match storage_responses.wait().await {
        StorageResponse::Acked(acked) => Ok(acked),
        StorageResponse::Peeked(_) => Err(AttErrorCode::UNLIKELY_ERROR),
        StorageResponse::Error(error) => {
            warn!("ble storage ACK failed error={:?}", error);
            Err(AttErrorCode::UNLIKELY_ERROR)
        }
    }
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
async fn ensure_ble_authorized(
    connection: &GattConnection<'_, '_, DefaultPacketPool>,
    auth_workspace: &'static BleAuthWorkspace,
) -> Result<(), AttErrorCode> {
    let pairing_open = BLE_PAIRING_STATE.lock().await.is_open();
    if pairing_open {
        return Ok(());
    }

    let has_saved_auth = {
        let workspace = auth_workspace.lock().await;
        matches!(workspace.status, BleAuthRecordStatus::Valid { .. }) && workspace.record_count > 0
    };
    if matches!(
        connection.raw().security_level(),
        Ok(SecurityLevel::Encrypted | SecurityLevel::EncryptedAuthenticated)
    ) && saved_auth_matches_connection(connection, auth_workspace).await
    {
        Ok(())
    } else if has_saved_auth {
        Err(AttErrorCode::INSUFFICIENT_ENCRYPTION)
    } else {
        Err(AttErrorCode::INSUFFICIENT_AUTHORISATION)
    }
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
async fn saved_auth_matches_connection(
    connection: &GattConnection<'_, '_, DefaultPacketPool>,
    auth_workspace: &'static BleAuthWorkspace,
) -> bool {
    let peer_identity = connection.raw().peer_identity();
    let workspace = auth_workspace.lock().await;
    if !matches!(workspace.status, BleAuthRecordStatus::Valid { .. }) {
        return false;
    }

    workspace.records[..workspace.record_count]
        .iter()
        .copied()
        .any(|record| identity_from_auth_record(record).match_identity(&peer_identity))
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
fn identity_from_auth_record(record: BleAuthRecord) -> Identity {
    Identity {
        bd_addr: BdAddr::new(record.identity_address),
        irk: record
            .identity_resolving_key
            .map(IdentityResolvingKey::from_le_bytes),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn stored_payload<const N: usize>(
        sequence: u64,
        payload: &[u8],
        payload_flags: u8,
    ) -> StoredPayload<N> {
        let mut stored = StoredPayload {
            sequence,
            payload: [0_u8; N],
            payload_len: payload.len(),
            payload_flags,
            current_boot: true,
        };
        stored.payload[..payload.len()].copy_from_slice(payload);
        stored
    }

    #[test]
    fn status_encoding_is_stable() {
        let status = BleStatus::new(
            BleRuntimeState::ControllerReady,
            NetworkState::IpReady,
            UploadResult::Success,
            7,
            ErrorFlags::WIFI | ErrorFlags::UPLOAD,
        );
        let mut out = [0_u8; BleStatus::ENCODED_LEN];

        assert_eq!(status.encode(&mut out), Ok(BleStatus::ENCODED_LEN));
        assert_eq!(
            &out[..BleStatus::LEGACY_ENCODED_LEN],
            [PROTOCOL_VERSION, 1, 3, 1, 7, 0, 0x18, 0, 0, 0]
        );
        assert_eq!(
            &out[BleStatus::LEGACY_ENCODED_LEN..],
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
        );
    }

    #[test]
    fn status_encoding_includes_pairing_window_diagnostics() {
        let status = BleStatus::new(
            BleRuntimeState::Connected,
            NetworkState::Disconnected,
            UploadResult::Failed,
            32,
            ErrorFlags::NONE,
        )
        .with_pairing(
            BootButtonState::Pressed,
            BlePairingState::Open {
                remaining_millis: 59_950,
            },
            2_050,
        );
        let mut out = [0_u8; BleStatus::ENCODED_LEN];

        assert_eq!(status.encode(&mut out), Ok(BleStatus::ENCODED_LEN));
        assert_eq!(
            &out[..BleStatus::LEGACY_ENCODED_LEN],
            [PROTOCOL_VERSION, 4, 0, 2, 32, 0, 0, 0, 0, 0]
        );
        assert_eq!(out[10], 1);
        assert_eq!(out[11], 1);
        assert_eq!(&out[12..16], &59_950_u32.to_le_bytes());
        assert_eq!(&out[16..20], &2_050_u32.to_le_bytes());
    }

    #[test]
    fn status_snapshot_combines_runtime_network_storage_and_errors() {
        let status = status_from_snapshots(
            BleRuntimeState::Connected,
            NetworkUploadStatus::new(NetworkState::Connected, UploadResult::HttpFailed),
            FirmwareStatusSnapshot::new(12, ErrorFlags::STORAGE | ErrorFlags::HTTP),
        );

        assert_eq!(
            status,
            BleStatus::new(
                BleRuntimeState::Connected,
                NetworkState::Connected,
                UploadResult::HttpFailed,
                12,
                ErrorFlags::STORAGE | ErrorFlags::HTTP,
            )
        );
    }

    #[test]
    fn metadata_encoding_includes_integrity_fields() {
        let metadata = RecordMetadata::new(42, 298, 0x01, 0x1234_5678, true);
        let mut out = [0_u8; RecordMetadata::ENCODED_LEN];

        assert_eq!(metadata.encode(&mut out), Ok(RecordMetadata::ENCODED_LEN));
        assert_eq!(out[0], PROTOCOL_VERSION);
        assert_eq!(&out[1..9], &42_u64.to_le_bytes());
        assert_eq!(&out[9..11], &298_u16.to_le_bytes());
        assert_eq!(out[11], 0x01);
        assert_eq!(&out[12..16], &0x1234_5678_u32.to_le_bytes());
        assert_eq!(out[16], 1);
    }

    #[test]
    fn metadata_from_payload_uses_storage_flags_and_crc() {
        let payload = stored_payload::<16>(7, b"abcdef", 0x01);

        assert_eq!(
            RecordMetadata::from_payload(&payload),
            Ok(RecordMetadata::new(7, 6, 0x01, crc32(b"abcdef"), true))
        );
    }

    #[test]
    fn fragment_rejects_out_of_bounds_payload() {
        assert_eq!(
            RecordFragment::new(1, 8, 10, &[1, 2, 3]),
            Err(BleProtocolError::FragmentOutOfBounds)
        );
    }

    #[test]
    fn fragment_encoding_is_bounded_and_structured() {
        let fragment = RecordFragment::new(9, 4, 12, &[0xaa, 0xbb, 0xcc]).unwrap();
        let mut out = [0_u8; RecordFragment::HEADER_LEN + 3];

        assert_eq!(
            fragment.encode(&mut out),
            Ok(RecordFragment::HEADER_LEN + 3)
        );
        assert_eq!(out[0], PROTOCOL_VERSION);
        assert_eq!(&out[1..9], &9_u64.to_le_bytes());
        assert_eq!(&out[9..11], &4_u16.to_le_bytes());
        assert_eq!(&out[11..13], &3_u16.to_le_bytes());
        assert_eq!(&out[13..], &[0xaa, 0xbb, 0xcc]);
    }

    #[test]
    fn control_frame_decodes_known_opcodes() {
        let mut input = [0_u8; ControlFrame::ENCODED_LEN];
        input[0] = PROTOCOL_VERSION;
        input[1] = ControlOpcode::RequestFragment as u8;
        input[2..10].copy_from_slice(&5_u64.to_le_bytes());
        input[10..12].copy_from_slice(&16_u16.to_le_bytes());
        input[12..14].copy_from_slice(&32_u16.to_le_bytes());

        assert_eq!(
            ControlFrame::decode(&input),
            Ok(ControlFrame::new(ControlOpcode::RequestFragment, 5, 16, 32))
        );
    }

    #[test]
    fn control_frame_round_trips_through_binary_encoding() {
        let frame = ControlFrame::new(ControlOpcode::AckRecord, 12, 34, 56);
        let mut out = [0_u8; ControlFrame::ENCODED_LEN];

        assert_eq!(frame.encode(&mut out), Ok(ControlFrame::ENCODED_LEN));
        assert_eq!(ControlFrame::decode(&out), Ok(frame));
    }

    #[test]
    fn transfer_session_sends_ordered_fragments_and_ack_when_wifi_unavailable() {
        let payload = stored_payload::<16>(9, b"abcdef", 0x01);
        let mut session = BleTransferSession::new();

        assert_eq!(
            session.start_record(&payload),
            Ok(RecordMetadata::new(9, 6, 0x01, crc32(b"abcdef"), true))
        );

        let first = session
            .fragment_for_request(
                &payload,
                ControlFrame::new(ControlOpcode::RequestFragment, 9, 0, 3),
            )
            .unwrap();
        assert_eq!(first.payload, b"abc");
        assert_eq!(
            session.state(),
            BleTransferState::Active {
                sequence: 9,
                total_len: 6,
                delivered_until: 3,
                complete: false,
                ack_sent: false,
            }
        );

        let second = session
            .fragment_for_request(
                &payload,
                ControlFrame::new(ControlOpcode::RequestFragment, 9, 3, 32),
            )
            .unwrap();
        assert_eq!(second.payload, b"def");

        assert_eq!(
            session.complete_record(
                &payload,
                ControlFrame::new(ControlOpcode::CompleteRecord, 9, 0, 0),
            ),
            Ok(())
        );
        assert_eq!(
            session.ack_action(
                &payload,
                ControlFrame::new(ControlOpcode::AckRecord, 9, 0, 0),
                NetworkState::Disconnected,
                UploadResult::TransportFailed,
            ),
            Ok(BleAckAction::SendStorageAck { sequence: 9 })
        );
    }

    #[test]
    fn transfer_session_suppresses_ack_while_wifi_upload_is_succeeding() {
        let payload = stored_payload::<16>(4, b"abc", 0x01);
        let mut session = BleTransferSession::new();

        session.start_record(&payload).unwrap();
        session
            .fragment_for_request(
                &payload,
                ControlFrame::new(ControlOpcode::RequestFragment, 4, 0, 3),
            )
            .unwrap();
        session
            .complete_record(
                &payload,
                ControlFrame::new(ControlOpcode::CompleteRecord, 4, 0, 0),
            )
            .unwrap();

        assert_eq!(
            session.ack_action(
                &payload,
                ControlFrame::new(ControlOpcode::AckRecord, 4, 0, 0),
                NetworkState::IpReady,
                UploadResult::Success,
            ),
            Ok(BleAckAction::Suppress)
        );
        assert_eq!(
            session.ack_action(
                &payload,
                ControlFrame::new(ControlOpcode::AckRecord, 4, 0, 0),
                NetworkState::Disconnected,
                UploadResult::TransportFailed,
            ),
            Ok(BleAckAction::SendStorageAck { sequence: 4 })
        );
    }

    #[test]
    fn transfer_session_rejects_out_of_order_fragment_requests() {
        let payload = stored_payload::<16>(3, b"abcdef", 0x01);
        let mut session = BleTransferSession::new();
        session.start_record(&payload).unwrap();

        assert_eq!(
            session.fragment_for_request(
                &payload,
                ControlFrame::new(ControlOpcode::RequestFragment, 3, 3, 3),
            ),
            Err(BleTransferError::FragmentOutOfOrder)
        );
    }

    #[test]
    fn transfer_session_requires_complete_record_before_ack() {
        let payload = stored_payload::<16>(3, b"abcdef", 0x01);
        let mut session = BleTransferSession::new();
        session.start_record(&payload).unwrap();
        session
            .fragment_for_request(
                &payload,
                ControlFrame::new(ControlOpcode::RequestFragment, 3, 0, 3),
            )
            .unwrap();

        assert_eq!(
            session.complete_record(
                &payload,
                ControlFrame::new(ControlOpcode::CompleteRecord, 3, 0, 0),
            ),
            Err(BleTransferError::TransferIncomplete)
        );
        assert_eq!(
            session.ack_action(
                &payload,
                ControlFrame::new(ControlOpcode::AckRecord, 3, 0, 0),
                NetworkState::Disconnected,
                UploadResult::Failed,
            ),
            Err(BleTransferError::TransferIncomplete)
        );
    }

    #[test]
    fn transfer_session_suppresses_duplicate_ack_after_storage_ack_action() {
        let payload = stored_payload::<16>(11, b"abc", 0x01);
        let mut session = BleTransferSession::new();
        session.start_record(&payload).unwrap();
        session
            .fragment_for_request(
                &payload,
                ControlFrame::new(ControlOpcode::RequestFragment, 11, 0, 3),
            )
            .unwrap();
        session
            .complete_record(
                &payload,
                ControlFrame::new(ControlOpcode::CompleteRecord, 11, 0, 0),
            )
            .unwrap();

        assert_eq!(
            session.ack_action(
                &payload,
                ControlFrame::new(ControlOpcode::AckRecord, 11, 0, 0),
                NetworkState::Disconnected,
                UploadResult::Failed,
            ),
            Ok(BleAckAction::SendStorageAck { sequence: 11 })
        );
        assert_eq!(
            session.ack_action(
                &payload,
                ControlFrame::new(ControlOpcode::AckRecord, 11, 0, 0),
                NetworkState::Disconnected,
                UploadResult::Failed,
            ),
            Ok(BleAckAction::Suppress)
        );
    }

    #[test]
    fn transfer_session_disconnect_preserves_storage_by_resetting_without_ack() {
        let payload = stored_payload::<16>(5, b"abc", 0x01);
        let mut session = BleTransferSession::new();
        session.start_record(&payload).unwrap();
        session
            .fragment_for_request(
                &payload,
                ControlFrame::new(ControlOpcode::RequestFragment, 5, 0, 3),
            )
            .unwrap();

        session.reset_after_disconnect();

        assert_eq!(session.state(), BleTransferState::Idle);
        assert_eq!(
            session.ack_action(
                &payload,
                ControlFrame::new(ControlOpcode::AckRecord, 5, 0, 0),
                NetworkState::Disconnected,
                UploadResult::Failed,
            ),
            Err(BleTransferError::NoActiveRecord)
        );
    }

    #[test]
    fn ack_policy_is_copy_only_while_wifi_upload_succeeds() {
        assert_eq!(
            ack_policy(NetworkState::IpReady, UploadResult::Success),
            BleAckPolicy::CopyOnly
        );
        assert_eq!(
            ack_policy(NetworkState::Connected, UploadResult::Success),
            BleAckPolicy::CopyOnly
        );
        assert_eq!(
            ack_policy(NetworkState::Disconnected, UploadResult::Success),
            BleAckPolicy::CanAckOldestRecord
        );
        assert_eq!(
            ack_policy(NetworkState::IpReady, UploadResult::TransportFailed),
            BleAckPolicy::CanAckOldestRecord
        );
    }

    #[test]
    fn legacy_advertising_payloads_fit_31_byte_limit() {
        const LEGACY_ADV_PAYLOAD_LIMIT: usize = 31;
        const AD_STRUCTURE_HEADER_LEN: usize = 2;
        const FLAGS_PAYLOAD_LEN: usize = 1;
        const UUID128_PAYLOAD_LEN: usize = SERVICE_UUID.len();

        let advertising_len = AD_STRUCTURE_HEADER_LEN
            + FLAGS_PAYLOAD_LEN
            + AD_STRUCTURE_HEADER_LEN
            + UUID128_PAYLOAD_LEN;
        let scan_response_len =
            AD_STRUCTURE_HEADER_LEN + crate::config::ble::ADVERTISING_NAME.len();
        let previous_combined_scan_response_len =
            AD_STRUCTURE_HEADER_LEN + UUID128_PAYLOAD_LEN + scan_response_len;

        assert!(advertising_len <= LEGACY_ADV_PAYLOAD_LIMIT);
        assert!(scan_response_len <= LEGACY_ADV_PAYLOAD_LIMIT);
        assert!(previous_combined_scan_response_len > LEGACY_ADV_PAYLOAD_LIMIT);
    }

    #[test]
    fn boot_button_state_uses_active_low_semantics() {
        assert_eq!(
            BootButtonState::from_active_low(true),
            BootButtonState::Pressed
        );
        assert_eq!(
            BootButtonState::from_active_low(false),
            BootButtonState::Released
        );
    }

    #[test]
    fn pairing_gesture_ignores_short_press() {
        let mut gesture = BlePairingGesture::new(2_000, 8_000, 60_000);

        assert_eq!(
            gesture.update(BootButtonState::Pressed, 1_900),
            BlePairingEvent::None
        );
        assert_eq!(
            gesture.update(BootButtonState::Released, 100),
            BlePairingEvent::None
        );
        assert_eq!(gesture.state(), BlePairingState::Closed);
    }

    #[test]
    fn pairing_gesture_opens_window_after_long_press() {
        let mut gesture = BlePairingGesture::new(2_000, 8_000, 60_000);

        assert_eq!(
            gesture.update(BootButtonState::Pressed, 1_000),
            BlePairingEvent::None
        );
        assert_eq!(
            gesture.update(BootButtonState::Pressed, 1_000),
            BlePairingEvent::WindowOpened
        );
        assert_eq!(
            gesture.state(),
            BlePairingState::Open {
                remaining_millis: 60_000
            }
        );
    }

    #[test]
    fn pairing_gesture_can_open_window_without_button_press() {
        let mut gesture = BlePairingGesture::new(2_000, 8_000, 60_000);

        assert_eq!(gesture.open_window(), BlePairingEvent::WindowOpened);
        assert_eq!(
            gesture.state(),
            BlePairingState::Open {
                remaining_millis: 60_000
            }
        );
        assert_eq!(
            gesture.update(BootButtonState::Pressed, 2_000),
            BlePairingEvent::None
        );
    }

    #[test]
    fn pairing_gesture_does_not_retrigger_until_button_released() {
        let mut gesture = BlePairingGesture::new(2_000, 8_000, 60_000);

        assert_eq!(
            gesture.update(BootButtonState::Pressed, 2_000),
            BlePairingEvent::WindowOpened
        );
        assert_eq!(
            gesture.update(BootButtonState::Pressed, 2_000),
            BlePairingEvent::None
        );
        assert_eq!(
            gesture.update(BootButtonState::Released, 50),
            BlePairingEvent::None
        );
        assert_eq!(
            gesture.update(BootButtonState::Pressed, 2_000),
            BlePairingEvent::WindowOpened
        );
    }

    #[test]
    fn pairing_window_expires_after_configured_duration() {
        let mut gesture = BlePairingGesture::new(2_000, 8_000, 60_000);

        assert_eq!(
            gesture.update(BootButtonState::Pressed, 2_000),
            BlePairingEvent::WindowOpened
        );
        assert_eq!(
            gesture.update(BootButtonState::Released, 59_999),
            BlePairingEvent::None
        );
        assert_eq!(
            gesture.state(),
            BlePairingState::Open {
                remaining_millis: 1
            }
        );
        assert_eq!(
            gesture.update(BootButtonState::Released, 1),
            BlePairingEvent::WindowExpired
        );
        assert_eq!(gesture.state(), BlePairingState::Closed);
    }

    #[test]
    fn pairing_gesture_requests_auth_record_clear_after_longer_hold_once() {
        let mut gesture = BlePairingGesture::new(2_000, 8_000, 60_000);

        assert_eq!(
            gesture.update(BootButtonState::Pressed, 2_000),
            BlePairingEvent::WindowOpened
        );
        assert_eq!(
            gesture.update(BootButtonState::Pressed, 5_999),
            BlePairingEvent::None
        );
        assert_eq!(
            gesture.update(BootButtonState::Pressed, 1),
            BlePairingEvent::AuthRecordsClearRequested
        );
        assert_eq!(
            gesture.update(BootButtonState::Pressed, 8_000),
            BlePairingEvent::None
        );
        assert_eq!(
            gesture.update(BootButtonState::Released, 50),
            BlePairingEvent::None
        );
        assert_eq!(
            gesture.update(BootButtonState::Pressed, 8_000),
            BlePairingEvent::AuthRecordsClearRequested
        );
    }

    #[test]
    fn pairing_state_reports_open_and_remaining_time() {
        assert!(!BlePairingState::Closed.is_open());
        assert_eq!(BlePairingState::Closed.remaining_millis(), 0);

        let state = BlePairingState::Open {
            remaining_millis: 42,
        };
        assert!(state.is_open());
        assert_eq!(state.remaining_millis(), 42);
    }
}
