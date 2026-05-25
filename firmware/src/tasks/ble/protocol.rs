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
