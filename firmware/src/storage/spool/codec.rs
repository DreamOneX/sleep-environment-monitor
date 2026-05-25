pub fn encoded_record_len(payload_len: usize) -> Result<usize, SpoolError> {
    if payload_len > MAX_PAYLOAD_LEN {
        return Err(SpoolError::PayloadTooLarge);
    }

    let len = RECORD_HEADER_LEN
        .checked_add(payload_len)
        .ok_or(SpoolError::PayloadTooLarge)?;

    Ok(align_up(len, FLASH_WRITE_ALIGNMENT))
}

pub fn encoded_log_entry_len(input: &[u8]) -> Result<usize, SpoolError> {
    if input.len() < 4 {
        return Err(SpoolError::BufferTooSmall);
    }

    let magic = u32::from_le_bytes([input[0], input[1], input[2], input[3]]);
    match magic {
        RECORD_MAGIC => {
            if input.len() < RECORD_HEADER_LEN {
                return Err(SpoolError::BufferTooSmall);
            }
            let payload_len = u16::from_le_bytes([input[16], input[17]]) as usize;
            encoded_record_len(payload_len)
        }
        ACK_MAGIC => Ok(encoded_ack_len()),
        _ => Err(SpoolError::BadMagic),
    }
}

pub fn encode_record(record: SpoolRecord<'_>, out: &mut [u8]) -> Result<usize, SpoolError> {
    if record.payload.len() > MAX_PAYLOAD_LEN {
        return Err(SpoolError::PayloadTooLarge);
    }

    let len = encoded_record_len(record.payload.len())?;
    if out.len() < len {
        return Err(SpoolError::BufferTooSmall);
    }

    out[0..4].copy_from_slice(&RECORD_MAGIC.to_le_bytes());
    out[4] = RECORD_VERSION;
    out[5] = record.flags;
    out[6..8].copy_from_slice(&(RECORD_HEADER_LEN as u16).to_le_bytes());
    out[8..16].copy_from_slice(&record.sequence.to_le_bytes());
    out[16..18].copy_from_slice(&(record.payload.len() as u16).to_le_bytes());
    out[18..22].copy_from_slice(&crc32(record.payload).to_le_bytes());
    let payload_end = RECORD_HEADER_LEN + record.payload.len();
    out[RECORD_HEADER_LEN..payload_end].copy_from_slice(record.payload);
    out[payload_end..len].fill(0xff);

    Ok(len)
}

pub fn decode_record(input: &[u8]) -> Result<SpoolRecord<'_>, SpoolError> {
    if input.len() < RECORD_HEADER_LEN {
        return Err(SpoolError::BufferTooSmall);
    }

    let magic = u32::from_le_bytes([input[0], input[1], input[2], input[3]]);
    if magic != RECORD_MAGIC {
        return Err(SpoolError::BadMagic);
    }

    let version = input[4];
    if version != RECORD_VERSION {
        return Err(SpoolError::UnsupportedVersion);
    }

    let flags = input[5];
    let header_len = u16::from_le_bytes([input[6], input[7]]) as usize;
    if header_len != RECORD_HEADER_LEN {
        return Err(SpoolError::BadHeaderLength);
    }

    let sequence = u64::from_le_bytes([
        input[8], input[9], input[10], input[11], input[12], input[13], input[14], input[15],
    ]);
    let payload_len = u16::from_le_bytes([input[16], input[17]]) as usize;
    let expected_crc = u32::from_le_bytes([input[18], input[19], input[20], input[21]]);
    let end = header_len
        .checked_add(payload_len)
        .ok_or(SpoolError::BadPayloadLength)?;
    let payload = input
        .get(header_len..end)
        .ok_or(SpoolError::BadPayloadLength)?;

    if crc32(payload) != expected_crc {
        return Err(SpoolError::BadCrc);
    }

    Ok(SpoolRecord {
        sequence,
        flags,
        payload,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AckRecord {
    pub sequence: u64,
}

pub fn encode_ack(sequence: u64, out: &mut [u8]) -> Result<usize, SpoolError> {
    let len = encoded_ack_len();
    if out.len() < len {
        return Err(SpoolError::BufferTooSmall);
    }

    out[0..4].copy_from_slice(&ACK_MAGIC.to_le_bytes());
    out[4] = RECORD_VERSION;
    out[5] = 0;
    out[6..14].copy_from_slice(&sequence.to_le_bytes());
    out[ACK_RECORD_LEN..len].fill(0xff);

    Ok(len)
}

pub fn decode_ack(input: &[u8]) -> Result<AckRecord, SpoolError> {
    if input.len() < ACK_RECORD_LEN {
        return Err(SpoolError::BufferTooSmall);
    }

    let magic = u32::from_le_bytes([input[0], input[1], input[2], input[3]]);
    if magic != ACK_MAGIC {
        return Err(SpoolError::BadMagic);
    }

    if input[4] != RECORD_VERSION {
        return Err(SpoolError::UnsupportedVersion);
    }

    Ok(AckRecord {
        sequence: u64::from_le_bytes([
            input[6], input[7], input[8], input[9], input[10], input[11], input[12], input[13],
        ]),
    })
}

pub fn recover_records<'a>(region: &'a [u8], records: &mut [Option<SpoolRecord<'a>>]) -> usize {
    let mut cursor = 0;
    let mut count = 0;

    while cursor + RECORD_HEADER_LEN <= region.len() && count < records.len() {
        if region[cursor..].iter().all(|byte| *byte == 0xff) {
            break;
        }

        match decode_record(&region[cursor..]) {
            Ok(record) => {
                let next = match encoded_record_len(record.payload.len()) {
                    Ok(len) => cursor + len,
                    Err(_) => break,
                };
                records[count] = Some(record);
                count += 1;
                cursor = next;
            }
            Err(SpoolError::BadMagic) => {
                cursor += 1;
            }
            Err(_) => {
                break;
            }
        }
    }

    count
}
