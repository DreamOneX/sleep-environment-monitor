pub fn encode_auth_record(
    record: BleAuthRecord,
    out: &mut [u8],
) -> Result<usize, BleAuthStorageError> {
    if out.len() < AUTH_RECORD_LEN {
        return Err(BleAuthStorageError::BufferTooSmall);
    }

    out[..AUTH_RECORD_LEN].fill(0);
    out[0..4].copy_from_slice(&AUTH_RECORD_MAGIC.to_le_bytes());
    out[4..6].copy_from_slice(&AUTH_RECORD_FORMAT_VERSION.to_le_bytes());
    out[6..8].copy_from_slice(&(AUTH_RECORD_LEN as u16).to_le_bytes());
    out[8] = record.address_kind.code();
    out[9] = record.security_level.code();
    let bonded_flag = u8::from(record.bonded) * AUTH_RECORD_FLAG_BONDED;
    let irk_flag = u8::from(record.identity_resolving_key.is_some()) * AUTH_RECORD_FLAG_IRK_PRESENT;
    out[10] = bonded_flag | irk_flag;
    out[12..18].copy_from_slice(&record.identity_address);
    out[18..34].copy_from_slice(&record.long_term_key);
    if let Some(irk) = record.identity_resolving_key {
        out[34..50].copy_from_slice(&irk);
    }

    let checksum = crc32(&out[..60]);
    out[60..64].copy_from_slice(&checksum.to_le_bytes());
    Ok(AUTH_RECORD_LEN)
}

pub fn decode_auth_record(input: &[u8]) -> Result<BleAuthRecord, BleAuthRecordStatus> {
    if input.len() < AUTH_RECORD_LEN {
        return Err(BleAuthRecordStatus::BadRecordLength {
            index: 0,
            stored: input.len() as u16,
            expected: AUTH_RECORD_LEN as u16,
        });
    }

    let stored_checksum = u32::from_le_bytes([input[60], input[61], input[62], input[63]]);
    let computed_checksum = crc32(&input[..60]);
    if stored_checksum != computed_checksum {
        return Err(BleAuthRecordStatus::RecordCrcMismatch {
            index: 0,
            stored: stored_checksum,
            computed: computed_checksum,
        });
    }

    let magic = u32::from_le_bytes([input[0], input[1], input[2], input[3]]);
    if magic != AUTH_RECORD_MAGIC {
        return Err(BleAuthRecordStatus::BadRecordMagic { index: 0 });
    }

    let format_version = u16::from_le_bytes([input[4], input[5]]);
    if format_version != AUTH_RECORD_FORMAT_VERSION {
        return Err(BleAuthRecordStatus::UnsupportedRecordFormat {
            index: 0,
            stored: format_version,
            expected: AUTH_RECORD_FORMAT_VERSION,
        });
    }

    let record_len = u16::from_le_bytes([input[6], input[7]]);
    if record_len != AUTH_RECORD_LEN as u16 {
        return Err(BleAuthRecordStatus::BadRecordLength {
            index: 0,
            stored: record_len,
            expected: AUTH_RECORD_LEN as u16,
        });
    }

    let Some(address_kind) = BleAuthAddressKind::from_code(input[8]) else {
        return Err(BleAuthRecordStatus::BadRecordAddressKind {
            index: 0,
            stored: input[8],
        });
    };
    let Some(security_level) = BleAuthSecurityLevel::from_code(input[9]) else {
        return Err(BleAuthRecordStatus::BadRecordSecurityLevel {
            index: 0,
            stored: input[9],
        });
    };

    let flags = input[10];
    let mut identity_address = [0_u8; 6];
    identity_address.copy_from_slice(&input[12..18]);
    let mut long_term_key = [0_u8; 16];
    long_term_key.copy_from_slice(&input[18..34]);
    let identity_resolving_key = if flags & AUTH_RECORD_FLAG_IRK_PRESENT != 0 {
        let mut irk = [0_u8; 16];
        irk.copy_from_slice(&input[34..50]);
        Some(irk)
    } else {
        None
    };

    Ok(BleAuthRecord {
        address_kind,
        identity_address,
        long_term_key,
        identity_resolving_key,
        security_level,
        bonded: flags & AUTH_RECORD_FLAG_BONDED != 0,
    })
}

pub fn records_checksum(encoded_records: &[u8]) -> u32 {
    crc32(encoded_records)
}

pub const fn should_auto_open_pairing_window(enabled: bool, status: BleAuthRecordStatus) -> bool {
    enabled && status.requires_pairing_window()
}

fn auth_records_encoded_len(record_count: usize) -> Result<usize, BleAuthStorageError> {
    record_count
        .checked_mul(AUTH_RECORD_LEN)
        .ok_or(BleAuthStorageError::RecordRegionTooLarge)
}
impl BleAuthRecordStatus {
    const fn with_record_index(self, index: u16) -> Self {
        match self {
            Self::BadRecordMagic { .. } => Self::BadRecordMagic { index },
            Self::UnsupportedRecordFormat {
                stored, expected, ..
            } => Self::UnsupportedRecordFormat {
                index,
                stored,
                expected,
            },
            Self::BadRecordLength {
                stored, expected, ..
            } => Self::BadRecordLength {
                index,
                stored,
                expected,
            },
            Self::BadRecordAddressKind { stored, .. } => {
                Self::BadRecordAddressKind { index, stored }
            }
            Self::BadRecordSecurityLevel { stored, .. } => {
                Self::BadRecordSecurityLevel { index, stored }
            }
            Self::RecordCrcMismatch {
                stored, computed, ..
            } => Self::RecordCrcMismatch {
                index,
                stored,
                computed,
            },
            status => status,
        }
    }
}
