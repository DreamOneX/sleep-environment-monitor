pub fn load_auth_records(
    flash: &impl FlashStorage,
    expected_records_version: u32,
    expected_compatibility_checksum: u32,
    records_out: &mut [BleAuthRecord],
    scratch: &mut [u8],
) -> Result<BleAuthLoadResult, BleAuthStorageError> {
    let mut header = [0_u8; AUTH_HEADER_LEN];
    flash.read(0, &mut header)?;
    let status = inspect_auth_header(
        &header,
        expected_records_version,
        expected_compatibility_checksum,
    );
    let BleAuthRecordStatus::Valid { record_count, .. } = status else {
        return Ok(BleAuthLoadResult {
            status,
            record_count: 0,
        });
    };

    let count = record_count as usize;
    if count > records_out.len() {
        return Ok(BleAuthLoadResult {
            status: BleAuthRecordStatus::RecordCountTooLarge {
                stored: record_count,
                capacity: records_out.len().min(u16::MAX as usize) as u16,
            },
            record_count: 0,
        });
    }

    let records_len = auth_records_encoded_len(count)?;
    let records_end = AUTH_HEADER_LEN
        .checked_add(records_len)
        .ok_or(BleAuthStorageError::RecordRegionTooLarge)?;
    if records_end > flash.len() {
        return Ok(BleAuthLoadResult {
            status: BleAuthRecordStatus::RecordRegionTooLarge,
            record_count: 0,
        });
    }
    if scratch.len() < records_len {
        return Err(BleAuthStorageError::BufferTooSmall);
    }

    flash.read(AUTH_HEADER_LEN, &mut scratch[..records_len])?;
    let stored_records_checksum =
        u32::from_le_bytes([header[16], header[17], header[18], header[19]]);
    let computed_records_checksum = records_checksum(&scratch[..records_len]);
    if stored_records_checksum != computed_records_checksum {
        return Ok(BleAuthLoadResult {
            status: BleAuthRecordStatus::RecordSetChecksumMismatch {
                stored: stored_records_checksum,
                computed: computed_records_checksum,
            },
            record_count: 0,
        });
    }

    for (index, chunk) in scratch[..records_len]
        .chunks_exact(AUTH_RECORD_LEN)
        .enumerate()
    {
        match decode_auth_record(chunk) {
            Ok(record) => records_out[index] = record,
            Err(status) => {
                return Ok(BleAuthLoadResult {
                    status: status.with_record_index(index.min(u16::MAX as usize) as u16),
                    record_count: 0,
                });
            }
        }
    }

    Ok(BleAuthLoadResult {
        status,
        record_count: count,
    })
}

pub fn store_auth_records(
    flash: &mut impl FlashStorage,
    records_version: u32,
    compatibility_checksum: u32,
    records: &[BleAuthRecord],
    scratch: &mut [u8],
) -> Result<(), BleAuthStorageError> {
    if records.len() > u16::MAX as usize {
        return Err(BleAuthStorageError::TooManyRecords);
    }
    if !flash.len().is_multiple_of(flash.sector_size())
        || !AUTH_HEADER_LEN.is_multiple_of(AUTH_RECORD_FLASH_ALIGNMENT)
    {
        return Err(BleAuthStorageError::UnalignedFlash);
    }

    let records_len = auth_records_encoded_len(records.len())?;
    let used_len = AUTH_HEADER_LEN
        .checked_add(records_len)
        .ok_or(BleAuthStorageError::RecordRegionTooLarge)?;
    if used_len > flash.len() {
        return Err(BleAuthStorageError::RecordRegionTooLarge);
    }
    if scratch.len() < used_len {
        return Err(BleAuthStorageError::BufferTooSmall);
    }

    scratch[..used_len].fill(0xff);
    for (index, record) in records.iter().enumerate() {
        let offset = AUTH_HEADER_LEN + index * AUTH_RECORD_LEN;
        encode_auth_record(*record, &mut scratch[offset..offset + AUTH_RECORD_LEN])?;
    }
    let checksum = records_checksum(&scratch[AUTH_HEADER_LEN..used_len]);
    encode_auth_header(
        records_version,
        records.len() as u16,
        checksum,
        compatibility_checksum,
        &mut scratch[..AUTH_HEADER_LEN],
    )
    .map_err(|_| BleAuthStorageError::BufferTooSmall)?;

    flash.erase(0, flash.len())?;
    flash.write(0, &scratch[..used_len])?;
    Ok(())
}

pub fn clear_auth_records(flash: &mut impl FlashStorage) -> Result<(), BleAuthStorageError> {
    flash.erase(0, flash.len()).map_err(Into::into)
}
