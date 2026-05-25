use crate::{
    storage::spool::crc32,
    tasks::storage::StoredPayload,
    types::{ErrorFlags, FirmwareStatusSnapshot, NetworkState, NetworkUploadStatus, UploadResult},
};

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

include!("profile.rs");
include!("protocol.rs");
include!("pairing.rs");
include!("status.rs");
include!("transfer.rs");
include!("auth.rs");
include!("storage_bridge.rs");
include!("gatt.rs");
include!("runtime.rs");

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
