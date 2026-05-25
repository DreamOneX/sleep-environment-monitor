pub fn upsert_auth_record(
    records: &mut [BleAuthRecord],
    record_count: usize,
    candidate: BleAuthRecord,
) -> (usize, BleAuthRecordUpsert) {
    let active_count = record_count.min(records.len());
    if let Some(index) = find_auth_record_index(&records[..active_count], candidate) {
        records[index] = candidate;
        return (active_count, BleAuthRecordUpsert::Updated { index });
    }

    if active_count < records.len() {
        records[active_count] = candidate;
        return (
            active_count + 1,
            BleAuthRecordUpsert::Appended {
                index: active_count,
            },
        );
    }

    if records.is_empty() {
        return (0, BleAuthRecordUpsert::NoCapacity);
    }

    records[0] = candidate;
    (
        active_count,
        BleAuthRecordUpsert::ReplacedOldest { index: 0 },
    )
}

fn find_auth_record_index(records: &[BleAuthRecord], candidate: BleAuthRecord) -> Option<usize> {
    records
        .iter()
        .position(|record| auth_records_match(*record, candidate))
}

fn auth_records_match(left: BleAuthRecord, right: BleAuthRecord) -> bool {
    left.identity_address == right.identity_address
        || match (left.identity_resolving_key, right.identity_resolving_key) {
            (Some(left_irk), Some(right_irk)) => left_irk == right_irk,
            _ => false,
        }
}
