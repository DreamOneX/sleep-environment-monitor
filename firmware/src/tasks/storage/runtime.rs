#[cfg(target_arch = "riscv32")]
use crate::{
    drivers::flash::RomSpoolFlash,
    storage::flash_model::FlashError,
    tasks::{
        FirmwareStatusSnapshotMutex, StorageRequestChannel, StorageResponseSignal, TaskSignal,
    },
};
#[cfg(target_arch = "riscv32")]
use defmt::{info, warn};

#[cfg(target_arch = "riscv32")]
const STORAGE_METRICS_LOG_EVERY_EVENTS: u32 = config::storage::METRICS_LOG_EVERY_EVENTS;

#[cfg(target_arch = "riscv32")]
#[embassy_executor::task]
pub async fn storage_task(
    requests: &'static StorageRequestChannel,
    wifi_responses: &'static StorageResponseSignal,
    ble_responses: &'static StorageResponseSignal,
    error_flags: &'static TaskSignal<ErrorFlags>,
    firmware_status: &'static FirmwareStatusSnapshotMutex,
) {
    let mut flash = match RomSpoolFlash::new() {
        Ok(flash) => flash,
        Err(error) => {
            warn!("storage flash init failed error={:?}", error);
            publish_storage_error(error_flags, firmware_status).await;
            loop {
                match requests.receive().await {
                    StorageCommand::Append(_) => {
                        publish_storage_error(error_flags, firmware_status).await;
                    }
                    StorageCommand::Peek(client) | StorageCommand::Ack { client, .. } => {
                        signal_storage_response(
                            wifi_responses,
                            ble_responses,
                            client,
                            StorageResponse::Error(StorageError::Spool(SpoolError::Flash(
                                FlashError::OutOfBounds,
                            ))),
                        );
                    }
                }
            }
        }
    };

    info!(
        "storage spool flash range offset=0x{:08x} len={=usize}",
        flash.absolute_offset(),
        flash.len()
    );

    let mut storage_event_count = 0_u32;
    let (mut backlog, storage_unavailable_error) =
        match MeasurementStorageBacklog::recover(&mut flash) {
            Ok(backlog) => {
                info!("storage recovered pending_len={=usize}", backlog.len());
                log_storage_metrics(backlog.metrics(), true, storage_event_count);
                publish_pending_record_count(firmware_status, backlog.metrics()).await;
                (Some(backlog), None)
            }
            Err(error) => {
                warn!("storage recovery failed error={:?}", error);
                publish_storage_error(error_flags, firmware_status).await;
                (None, Some(error))
            }
        };

    loop {
        match requests.receive().await {
            StorageCommand::Append(measurement) => {
                let Some(backlog) = backlog.as_mut() else {
                    publish_storage_error(error_flags, firmware_status).await;
                    continue;
                };

                let dropped_oldest_before = backlog.metrics().dropped_oldest_count;
                match backlog.append_measurement(&mut flash, measurement) {
                    Ok(()) => {
                        storage_event_count = storage_event_count.wrapping_add(1);
                        let metrics = backlog.metrics();
                        log_storage_metrics(
                            metrics,
                            dropped_oldest_before == 0 && metrics.dropped_oldest_count > 0,
                            storage_event_count,
                        );
                        publish_pending_record_count(firmware_status, metrics).await;
                    }
                    Err(error) => {
                        warn!("storage append failed error={:?}", error);
                        storage_event_count = storage_event_count.wrapping_add(1);
                        let metrics = backlog.metrics();
                        log_storage_metrics(metrics, true, storage_event_count);
                        publish_pending_record_count(firmware_status, metrics).await;
                        publish_storage_error(error_flags, firmware_status).await;
                    }
                }
            }
            StorageCommand::Peek(client) => {
                let response = match backlog.as_ref() {
                    Some(backlog) => StorageResponse::Peeked(backlog.peek_payload()),
                    None => StorageResponse::Error(storage_unavailable_error.unwrap_or(
                        StorageError::Spool(SpoolError::Flash(FlashError::OutOfBounds)),
                    )),
                };
                signal_storage_response(wifi_responses, ble_responses, client, response);
            }
            StorageCommand::Ack { client, sequence } => {
                let Some(backlog) = backlog.as_mut() else {
                    signal_storage_response(
                        wifi_responses,
                        ble_responses,
                        client,
                        StorageResponse::Error(storage_unavailable_error.unwrap_or(
                            StorageError::Spool(SpoolError::Flash(FlashError::OutOfBounds)),
                        )),
                    );
                    publish_storage_error(error_flags, firmware_status).await;
                    continue;
                };

                match backlog.acknowledge_sequence(&mut flash, sequence) {
                    Ok(acknowledged) => {
                        storage_event_count = storage_event_count.wrapping_add(1);
                        let metrics = backlog.metrics();
                        log_storage_metrics(metrics, false, storage_event_count);
                        publish_pending_record_count(firmware_status, metrics).await;
                        signal_storage_response(
                            wifi_responses,
                            ble_responses,
                            client,
                            StorageResponse::Acked(acknowledged.is_some()),
                        );
                    }
                    Err(error) => {
                        warn!("storage ack failed error={:?}", error);
                        storage_event_count = storage_event_count.wrapping_add(1);
                        log_storage_metrics(backlog.metrics(), true, storage_event_count);
                        signal_storage_response(
                            wifi_responses,
                            ble_responses,
                            client,
                            StorageResponse::Error(error),
                        );
                        publish_storage_error(error_flags, firmware_status).await;
                    }
                }
            }
        }
    }
}

#[cfg(target_arch = "riscv32")]
async fn publish_pending_record_count(
    firmware_status: &'static FirmwareStatusSnapshotMutex,
    metrics: StorageMetrics,
) {
    let pending_record_count = u16::try_from(metrics.pending_record_count).unwrap_or(u16::MAX);
    let mut status = firmware_status.lock().await;
    let current = *status;
    *status = current.with_pending_record_count(pending_record_count);
}

#[cfg(target_arch = "riscv32")]
async fn publish_storage_error(
    error_flags: &'static TaskSignal<ErrorFlags>,
    firmware_status: &'static FirmwareStatusSnapshotMutex,
) {
    error_flags.signal(ErrorFlags::STORAGE);
    let mut status = firmware_status.lock().await;
    let current = *status;
    *status = current.with_error_flags(current.error_flags | ErrorFlags::STORAGE);
}

#[cfg(target_arch = "riscv32")]
fn signal_storage_response(
    wifi_responses: &'static StorageResponseSignal,
    ble_responses: &'static StorageResponseSignal,
    client: StorageClient,
    response: StorageResponse,
) {
    match client {
        StorageClient::Wifi => wifi_responses.signal(response),
        StorageClient::Ble => ble_responses.signal(response),
    }
}

#[cfg(target_arch = "riscv32")]
fn log_storage_metrics(metrics: StorageMetrics, force: bool, event_count: u32) {
    if !force
        && event_count != 1
        && !event_count.is_multiple_of(STORAGE_METRICS_LOG_EVERY_EVENTS)
        && metrics.last_error.is_none()
    {
        return;
    }

    match metrics.last_error {
        Some(error) => info!(
            "storage metrics pending={=usize} recovered={=usize} dropped_oldest={=usize} skipped_legacy={=usize} corrupt={=usize} last_error={:?}",
            metrics.pending_record_count,
            metrics.recovered_record_count,
            metrics.dropped_oldest_count,
            metrics.skipped_legacy_record_count,
            metrics.corrupt_record_count,
            error
        ),
        None => info!(
            "storage metrics pending={=usize} recovered={=usize} dropped_oldest={=usize} skipped_legacy={=usize} corrupt={=usize} last_error=none",
            metrics.pending_record_count,
            metrics.recovered_record_count,
            metrics.dropped_oldest_count,
            metrics.skipped_legacy_record_count,
            metrics.corrupt_record_count
        ),
    }
}
