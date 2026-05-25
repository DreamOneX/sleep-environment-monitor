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
