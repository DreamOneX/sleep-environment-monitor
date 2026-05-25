#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
async fn peek_ble_payload(
    storage_requests: &'static StorageRequestChannel,
    storage_responses: &'static StorageResponseSignal,
) -> Option<StoredPayload> {
    storage_requests
        .send(StorageCommand::Peek(StorageClient::Ble))
        .await;
    match storage_responses.wait().await {
        StorageResponse::Peeked(payload) => payload,
        StorageResponse::Acked(_) => None,
        StorageResponse::Error(error) => {
            warn!("ble storage peek failed error={:?}", error);
            None
        }
    }
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
async fn acknowledge_ble_payload(
    storage_requests: &'static StorageRequestChannel,
    storage_responses: &'static StorageResponseSignal,
    sequence: u64,
) -> Result<bool, AttErrorCode> {
    storage_requests
        .send(StorageCommand::Ack {
            client: StorageClient::Ble,
            sequence,
        })
        .await;
    match storage_responses.wait().await {
        StorageResponse::Acked(acked) => Ok(acked),
        StorageResponse::Peeked(_) => Err(AttErrorCode::UNLIKELY_ERROR),
        StorageResponse::Error(error) => {
            warn!("ble storage ACK failed error={:?}", error);
            Err(AttErrorCode::UNLIKELY_ERROR)
        }
    }
}
