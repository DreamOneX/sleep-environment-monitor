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
