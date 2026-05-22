use crate::{
    storage::spool::crc32,
    tasks::storage::StoredPayload,
    types::{ErrorFlags, NetworkState, UploadResult},
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
}

impl BleStatus {
    pub const ENCODED_LEN: usize = 10;

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

        Ok(Self::ENCODED_LEN)
    }
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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BlePairingGesture {
    hold_millis: u64,
    window_millis: u64,
    pressed_millis: u64,
    opened_for_current_press: bool,
    state: BlePairingState,
}

impl BlePairingGesture {
    pub const fn new(hold_millis: u64, window_millis: u64) -> Self {
        Self {
            hold_millis,
            window_millis,
            pressed_millis: 0,
            opened_for_current_press: false,
            state: BlePairingState::Closed,
        }
    }

    pub const fn state(&self) -> BlePairingState {
        self.state
    }

    pub fn update(
        &mut self,
        button_state: BootButtonState,
        elapsed_millis: u64,
    ) -> BlePairingEvent {
        let expired = self.tick_pairing_window(elapsed_millis);

        if button_state.is_pressed() {
            self.pressed_millis = self.pressed_millis.saturating_add(elapsed_millis);
            if !self.opened_for_current_press && self.pressed_millis >= self.hold_millis {
                self.state = BlePairingState::Open {
                    remaining_millis: self.window_millis,
                };
                self.opened_for_current_press = true;
                return BlePairingEvent::WindowOpened;
            }
        } else {
            self.pressed_millis = 0;
            self.opened_for_current_press = false;
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

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
use defmt::{info, warn};
#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
use embassy_time::{Duration, Timer};
#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
use esp_hal::gpio::Input;
#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
use esp_radio::ble::controller::BleConnector;

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
#[embassy_executor::task]
pub async fn ble_task(mut connector: BleConnector<'static>, boot_button: Input<'static>) {
    info!(
        "ble controller initialized name={=str} protocol_version={=u8}",
        crate::config::ble::ADVERTISING_NAME,
        PROTOCOL_VERSION
    );
    warn!("ble GATT host is not active; BOOT/IO9 pairing gesture is monitored only");

    let mut hci_scratch = [0_u8; crate::config::ble::HCI_SCRATCH_BUFFER_LEN];
    let mut pairing_gesture = BlePairingGesture::new(
        crate::config::ble::PAIRING_HOLD_MILLIS,
        crate::config::ble::PAIRING_WINDOW_SECS.saturating_mul(1_000),
    );

    loop {
        let pairing_event = pairing_gesture.update(
            BootButtonState::from_active_low(boot_button.is_low()),
            crate::config::ble::PAIRING_BUTTON_POLL_MILLIS,
        );
        match pairing_event {
            BlePairingEvent::WindowOpened => info!(
                "ble pairing window opened remaining_ms={=u64}",
                pairing_gesture.state().remaining_millis()
            ),
            BlePairingEvent::WindowExpired => info!("ble pairing window expired"),
            BlePairingEvent::None => {}
        }

        match connector.next(&mut hci_scratch) {
            Ok(len) if len > 0 => info!("ble hci packet pending len={=usize}", len),
            Ok(_) => {}
            Err(error) => warn!("ble hci poll failed error={:?}", error),
        }

        Timer::after(Duration::from_millis(
            crate::config::ble::PAIRING_BUTTON_POLL_MILLIS,
        ))
        .await;
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
        assert_eq!(out, [PROTOCOL_VERSION, 1, 3, 1, 7, 0, 0x18, 0, 0, 0]);
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
        let mut gesture = BlePairingGesture::new(2_000, 60_000);

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
        let mut gesture = BlePairingGesture::new(2_000, 60_000);

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
    fn pairing_gesture_does_not_retrigger_until_button_released() {
        let mut gesture = BlePairingGesture::new(2_000, 60_000);

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
        let mut gesture = BlePairingGesture::new(2_000, 60_000);

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
