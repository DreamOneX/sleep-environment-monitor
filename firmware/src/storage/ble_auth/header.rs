pub fn encode_auth_header(
    records_version: u32,
    record_count: u16,
    records_checksum: u32,
    compatibility_checksum: u32,
    out: &mut [u8],
) -> Result<usize, BleAuthHeaderError> {
    if out.len() < AUTH_HEADER_LEN {
        return Err(BleAuthHeaderError::BufferTooSmall);
    }

    out[..AUTH_HEADER_LEN].fill(0);
    out[0..4].copy_from_slice(&AUTH_HEADER_MAGIC.to_le_bytes());
    out[4..6].copy_from_slice(&AUTH_HEADER_FORMAT_VERSION.to_le_bytes());
    out[6..8].copy_from_slice(&(AUTH_HEADER_LEN as u16).to_le_bytes());
    out[8..12].copy_from_slice(&records_version.to_le_bytes());
    out[12..14].copy_from_slice(&record_count.to_le_bytes());
    out[14..16].copy_from_slice(&0_u16.to_le_bytes());
    out[16..20].copy_from_slice(&records_checksum.to_le_bytes());
    out[20..24].copy_from_slice(&compatibility_checksum.to_le_bytes());
    out[24..28].copy_from_slice(&0_u32.to_le_bytes());

    let header_checksum = crc32(&out[..28]);
    out[28..32].copy_from_slice(&header_checksum.to_le_bytes());
    Ok(AUTH_HEADER_LEN)
}

pub fn inspect_auth_header(
    input: &[u8],
    expected_records_version: u32,
    expected_compatibility_checksum: u32,
) -> BleAuthRecordStatus {
    if input.len() < AUTH_HEADER_LEN {
        return BleAuthRecordStatus::BadHeaderLength {
            stored: input.len() as u16,
            expected: AUTH_HEADER_LEN as u16,
        };
    }
    if input[..AUTH_HEADER_LEN].iter().all(|byte| *byte == 0xff) {
        return BleAuthRecordStatus::Missing;
    }

    let magic = u32::from_le_bytes([input[0], input[1], input[2], input[3]]);
    if magic != AUTH_HEADER_MAGIC {
        return BleAuthRecordStatus::BadMagic;
    }

    let format_version = u16::from_le_bytes([input[4], input[5]]);
    if format_version != AUTH_HEADER_FORMAT_VERSION {
        return BleAuthRecordStatus::UnsupportedFormat {
            stored: format_version,
            expected: AUTH_HEADER_FORMAT_VERSION,
        };
    }

    let header_len = u16::from_le_bytes([input[6], input[7]]);
    if header_len != AUTH_HEADER_LEN as u16 {
        return BleAuthRecordStatus::BadHeaderLength {
            stored: header_len,
            expected: AUTH_HEADER_LEN as u16,
        };
    }

    let stored_checksum = u32::from_le_bytes([input[28], input[29], input[30], input[31]]);
    let computed_checksum = crc32(&input[..28]);
    if stored_checksum != computed_checksum {
        return BleAuthRecordStatus::ChecksumMismatch {
            stored: stored_checksum,
            computed: computed_checksum,
        };
    }

    let records_version = u32::from_le_bytes([input[8], input[9], input[10], input[11]]);
    if records_version != expected_records_version {
        return BleAuthRecordStatus::RecordsVersionMismatch {
            stored: records_version,
            expected: expected_records_version,
        };
    }

    let compatibility_checksum = u32::from_le_bytes([input[20], input[21], input[22], input[23]]);
    if compatibility_checksum != expected_compatibility_checksum {
        return BleAuthRecordStatus::CompatibilityChecksumMismatch {
            stored: compatibility_checksum,
            expected: expected_compatibility_checksum,
        };
    }

    let record_count = u16::from_le_bytes([input[12], input[13]]);
    if record_count == 0 {
        return BleAuthRecordStatus::NoRecords;
    }

    BleAuthRecordStatus::Valid {
        records_version,
        record_count,
    }
}
