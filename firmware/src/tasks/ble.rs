use crate::types::{ErrorFlags, NetworkState, UploadResult};

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
use esp_radio::ble::controller::BleConnector;

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
#[embassy_executor::task]
pub async fn ble_task(mut connector: BleConnector<'static>) {
    info!(
        "ble controller initialized name={=str} protocol_version={=u8}",
        crate::config::ble::ADVERTISING_NAME,
        PROTOCOL_VERSION
    );
    warn!("ble GATT host is not active in Phase 24A compile integration");

    let mut hci_scratch = [0_u8; crate::config::ble::HCI_SCRATCH_BUFFER_LEN];
    loop {
        match connector.next(&mut hci_scratch) {
            Ok(len) if len > 0 => info!("ble hci packet pending len={=usize}", len),
            Ok(_) => {}
            Err(error) => warn!("ble hci poll failed error={:?}", error),
        }

        Timer::after(Duration::from_secs(crate::config::ble::IDLE_POLL_SECS)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
