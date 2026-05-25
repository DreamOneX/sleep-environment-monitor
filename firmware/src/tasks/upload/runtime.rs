#[cfg(all(target_arch = "riscv32", feature = "wifi-upload"))]
use crate::{
    tasks::{
        NetworkUploadStatusMutex, StorageRequestChannel, StorageResponseSignal, TaskSignal,
        storage::{StorageClient, StorageCommand, StorageResponse, StoredPayload},
    },
    types::{NetworkState, UploadResult},
};
#[cfg(all(target_arch = "riscv32", feature = "wifi-upload"))]
use defmt::{info, warn};
#[cfg(all(target_arch = "riscv32", feature = "wifi-upload"))]
use embassy_net::{
    IpAddress, IpEndpoint, Stack,
    tcp::{ConnectError, TcpSocket},
    udp::{PacketMetadata, UdpSocket},
};
#[cfg(all(target_arch = "riscv32", feature = "wifi-upload"))]
use embassy_time::{Duration, Instant, Timer, with_timeout};
#[cfg(all(target_arch = "riscv32", feature = "wifi-upload"))]
use embedded_io_async::Write as _;

#[cfg(all(target_arch = "riscv32", feature = "wifi-upload"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq, defmt::Format)]
enum UploadError {
    Encode,
    Discovery,
    Time,
    ConnectInvalidState,
    ConnectReset,
    ConnectTimedOut,
    NoRoute,
    Write,
    Read,
    Timeout,
    Response,
}

#[cfg(all(target_arch = "riscv32", feature = "wifi-upload"))]
#[embassy_executor::task]
pub async fn uploader_task(
    stack: Stack<'static>,
    storage_requests: &'static StorageRequestChannel,
    storage_responses: &'static StorageResponseSignal,
    network_state: &'static TaskSignal<NetworkState>,
    upload_result: &'static TaskSignal<UploadResult>,
    network_upload_status: &'static NetworkUploadStatusMutex,
) {
    let mut logged_network_config = false;
    let mut upload_success_count = 0_u32;
    let mut discovered_endpoint = None;
    let mut time_sync = None;
    let mut last_discovery_attempt_ms = None;
    let mut last_time_attempt_ms = None;

    loop {
        stack.wait_config_up().await;
        publish_network_state(network_state, network_upload_status, NetworkState::IpReady).await;
        if !logged_network_config && let Some(config) = stack.config_v4() {
            info!("network ipv4 config={:?}", config);
            logged_network_config = true;
        }

        let now_ms = Instant::now().as_millis();
        if should_retry(
            last_discovery_attempt_ms,
            now_ms,
            config::upload::DISCOVERY_RETRY_SECS,
        ) {
            last_discovery_attempt_ms = Some(now_ms);
            match discover_endpoint(stack).await {
                Ok(endpoint) => {
                    info!(
                        "discovery endpoint ipv4={=u8}.{=u8}.{=u8}.{=u8} port={=u16}",
                        endpoint.ipv4[0],
                        endpoint.ipv4[1],
                        endpoint.ipv4[2],
                        endpoint.ipv4[3],
                        endpoint.port
                    );
                    discovered_endpoint = Some(endpoint);
                }
                Err(error) => {
                    publish_upload_result(
                        upload_result,
                        network_upload_status,
                        UploadResult::DiscoveryFailed,
                    )
                    .await;
                    warn!("discovery failed error={:?}", error);
                }
            }
        }

        let endpoint = resolve_endpoint(EndpointCandidates {
            provisioned: None,
            discovered: discovered_endpoint,
            static_fallback: Endpoint::static_fallback(),
        });

        if should_retry(
            last_time_attempt_ms,
            now_ms,
            config::upload::TIME_SYNC_RETRY_SECS,
        ) {
            last_time_attempt_ms = Some(now_ms);
            match sync_time(stack, endpoint).await {
                Ok(unix_ms) => {
                    time_sync = Some(TimeSyncState::new(Instant::now().as_millis(), unix_ms));
                    info!("time synced unix_ms={=u64}", unix_ms);
                }
                Err(error) => {
                    publish_upload_result(
                        upload_result,
                        network_upload_status,
                        UploadResult::TimeFailed,
                    )
                    .await;
                    warn!("time sync failed error={:?}", error);
                }
            }
        }

        let Some(payload) = peek_payload(storage_requests, storage_responses).await else {
            Timer::after(Duration::from_millis(
                config::upload::EMPTY_SPOOL_POLL_MILLIS,
            ))
            .await;
            continue;
        };

        match post_json_payload(stack, endpoint, &payload, time_sync).await {
            Ok(()) => {
                let acked =
                    acknowledge_payload(storage_requests, storage_responses, payload.sequence)
                        .await;
                publish_upload_result(upload_result, network_upload_status, UploadResult::Success)
                    .await;
                if !acked
                    || upload_success_count == 0
                    || upload_success_count.is_multiple_of(config::upload::SUCCESS_LOG_EVERY)
                {
                    info!(
                        "upload success sequence={=u64} acked={=bool}",
                        payload.sequence, acked
                    );
                }
                upload_success_count = upload_success_count.wrapping_add(1);
            }
            Err(error) => {
                publish_upload_result(
                    upload_result,
                    network_upload_status,
                    upload_result_for_error(error),
                )
                .await;
                warn!(
                    "upload failed error={:?} sequence={=u64}",
                    error, payload.sequence
                );
                Timer::after(Duration::from_secs(config::upload::RETRY_DELAY_SECS)).await;
            }
        }
    }
}

#[cfg(all(target_arch = "riscv32", feature = "wifi-upload"))]
async fn publish_network_state(
    network_state: &'static TaskSignal<NetworkState>,
    network_upload_status: &'static NetworkUploadStatusMutex,
    state: NetworkState,
) {
    network_state.signal(state);
    let mut status = network_upload_status.lock().await;
    *status = status.with_network_state(state);
}

#[cfg(all(target_arch = "riscv32", feature = "wifi-upload"))]
async fn publish_upload_result(
    upload_result: &'static TaskSignal<UploadResult>,
    network_upload_status: &'static NetworkUploadStatusMutex,
    result: UploadResult,
) {
    upload_result.signal(result);
    let mut status = network_upload_status.lock().await;
    *status = status.with_upload_result(result);
}

#[cfg(all(target_arch = "riscv32", feature = "wifi-upload"))]
fn should_retry(last_attempt_ms: Option<u64>, now_ms: u64, interval_secs: u64) -> bool {
    last_attempt_ms
        .is_none_or(|last| now_ms.saturating_sub(last) >= interval_secs.saturating_mul(1000))
}

#[cfg(all(target_arch = "riscv32", feature = "wifi-upload"))]
async fn peek_payload(
    storage_requests: &StorageRequestChannel,
    storage_responses: &StorageResponseSignal,
) -> Option<StoredPayload> {
    storage_requests
        .send(StorageCommand::Peek(StorageClient::Wifi))
        .await;
    match storage_responses.wait().await {
        StorageResponse::Peeked(payload) => payload,
        StorageResponse::Acked(_) => None,
        StorageResponse::Error(error) => {
            warn!("storage peek failed error={:?}", error);
            None
        }
    }
}

#[cfg(all(target_arch = "riscv32", feature = "wifi-upload"))]
async fn acknowledge_payload(
    storage_requests: &StorageRequestChannel,
    storage_responses: &StorageResponseSignal,
    sequence: u64,
) -> bool {
    storage_requests
        .send(StorageCommand::Ack {
            client: StorageClient::Wifi,
            sequence,
        })
        .await;
    match storage_responses.wait().await {
        StorageResponse::Acked(acked) => acked,
        StorageResponse::Peeked(_) => false,
        StorageResponse::Error(error) => {
            warn!("storage ack response failed error={:?}", error);
            false
        }
    }
}

#[cfg(all(target_arch = "riscv32", feature = "wifi-upload"))]
async fn post_json_payload(
    stack: Stack<'static>,
    endpoint: Endpoint,
    payload: &StoredPayload,
    time_sync: Option<TimeSyncState>,
) -> Result<(), UploadError> {
    let mut body = [0_u8; config::upload::REQUEST_BUFFER_SIZE];
    let timestamp = select_payload_timestamp(
        payload.current_boot,
        time_sync,
        stored_payload_uptime(payload.as_slice()),
    );
    let body_len = build_measurement_json(
        config::upload::DEVICE_ID,
        payload.sequence,
        payload.as_slice(),
        timestamp,
        &mut body,
    )
    .map_err(|_| UploadError::Encode)?;

    let mut response = [0_u8; config::upload::RESPONSE_BUFFER_SIZE];
    let response = send_http_request(
        stack,
        endpoint,
        "POST",
        config::upload::MEASUREMENT_PATH,
        Some("application/json"),
        &body[..body_len],
        &mut response,
    )
    .await?;

    if http_response_is_success(response) {
        Ok(())
    } else {
        Err(UploadError::Response)
    }
}

#[cfg(all(target_arch = "riscv32", feature = "wifi-upload"))]
async fn send_http_request<'a>(
    stack: Stack<'static>,
    endpoint: Endpoint,
    method: &str,
    path: &str,
    content_type: Option<&str>,
    body: &[u8],
    response: &'a mut [u8],
) -> Result<&'a [u8], UploadError> {
    let mut rx_buffer = [0_u8; config::upload::RX_BUFFER_SIZE];
    let mut tx_buffer = [0_u8; config::upload::TX_BUFFER_SIZE];
    let mut request = [0_u8; config::upload::REQUEST_BUFFER_SIZE];
    let mut host_header = [0_u8; 22];
    let host_header_len =
        endpoint_host_header(endpoint, &mut host_header).map_err(|_| UploadError::Encode)?;
    let host_header =
        core::str::from_utf8(&host_header[..host_header_len]).map_err(|_| UploadError::Encode)?;

    let request_len =
        build_http_request(method, host_header, path, content_type, body, &mut request)
            .map_err(|_| UploadError::Encode)?;

    let [a, b, c, d] = endpoint.ipv4;
    let remote = IpEndpoint::new(IpAddress::v4(a, b, c, d), endpoint.port);
    let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);
    socket.set_timeout(Some(Duration::from_secs(
        config::upload::SOCKET_TIMEOUT_SECS,
    )));

    socket.connect(remote).await.map_err(map_connect_error)?;
    socket
        .write_all(&request[..request_len])
        .await
        .map_err(|_| UploadError::Write)?;
    socket.flush().await.map_err(|_| UploadError::Write)?;

    let read_len = read_http_response(&mut socket, response).await?;

    socket.close();

    if read_len == 0 {
        return Err(UploadError::Response);
    }

    Ok(&response[..read_len])
}

#[cfg(all(target_arch = "riscv32", feature = "wifi-upload"))]
async fn read_http_response(
    socket: &mut TcpSocket<'_>,
    response: &mut [u8],
) -> Result<usize, UploadError> {
    let mut total_len = 0;
    let mut expected_len = None;

    loop {
        if total_len == response.len() {
            break;
        }

        let read_len = with_timeout(
            Duration::from_secs(config::upload::READ_TIMEOUT_SECS),
            socket.read(&mut response[total_len..]),
        )
        .await
        .map_err(|_| UploadError::Timeout)?
        .map_err(|_| UploadError::Read)?;

        if read_len == 0 {
            break;
        }

        total_len += read_len;
        if expected_len.is_none() {
            expected_len = http_response_total_len(&response[..total_len]);
        }
        if expected_len.is_some_and(|len| total_len >= len) {
            break;
        }
    }

    if total_len == 0 {
        return Err(UploadError::Response);
    }

    Ok(total_len)
}

#[cfg(all(target_arch = "riscv32", feature = "wifi-upload"))]
async fn discover_endpoint(stack: Stack<'static>) -> Result<Endpoint, UploadError> {
    let Some(config) = stack.config_v4() else {
        return Err(UploadError::Discovery);
    };
    let Some(broadcast) = config.address.broadcast() else {
        return Err(UploadError::Discovery);
    };

    let mut rx_meta = [PacketMetadata::EMPTY];
    let mut tx_meta = [PacketMetadata::EMPTY];
    let mut rx_buffer = [0_u8; config::upload::UDP_BUFFER_SIZE];
    let mut tx_buffer = [0_u8; config::upload::UDP_BUFFER_SIZE];
    let mut response = [0_u8; config::upload::UDP_BUFFER_SIZE];
    let mut socket = UdpSocket::new(
        stack,
        &mut rx_meta,
        &mut rx_buffer,
        &mut tx_meta,
        &mut tx_buffer,
    );
    socket.bind(0).map_err(|_| UploadError::Discovery)?;

    let endpoint = IpEndpoint::new(
        IpAddress::Ipv4(broadcast),
        config::upload::UDP_DISCOVERY_PORT,
    );
    socket
        .send_to(config::upload::UDP_DISCOVERY_QUERY.as_bytes(), endpoint)
        .await
        .map_err(|_| UploadError::Discovery)?;
    let (len, _) = with_timeout(
        Duration::from_secs(config::upload::READ_TIMEOUT_SECS),
        socket.recv_from(&mut response),
    )
    .await
    .map_err(|_| UploadError::Discovery)?
    .map_err(|_| UploadError::Discovery)?;

    parse_discovery_endpoint(&response[..len]).map_err(|_| UploadError::Discovery)
}

#[cfg(all(target_arch = "riscv32", feature = "wifi-upload"))]
async fn sync_time(stack: Stack<'static>, endpoint: Endpoint) -> Result<u64, UploadError> {
    match sync_time_sntp(stack).await {
        Ok(unix_ms) => Ok(unix_ms),
        Err(_) => sync_time_rest(stack, endpoint).await,
    }
}

#[cfg(all(target_arch = "riscv32", feature = "wifi-upload"))]
async fn sync_time_sntp(stack: Stack<'static>) -> Result<u64, UploadError> {
    let [a, b, c, d] = config::upload::NTP_SERVER_IPV4_OCTETS;
    let remote = IpEndpoint::new(IpAddress::v4(a, b, c, d), config::upload::NTP_PORT);
    let mut rx_meta = [PacketMetadata::EMPTY];
    let mut tx_meta = [PacketMetadata::EMPTY];
    let mut rx_buffer = [0_u8; config::upload::UDP_BUFFER_SIZE];
    let mut tx_buffer = [0_u8; config::upload::UDP_BUFFER_SIZE];
    let mut request = [0_u8; 48];
    let mut response = [0_u8; 48];
    request[0] = 0b00_100_011;

    let mut socket = UdpSocket::new(
        stack,
        &mut rx_meta,
        &mut rx_buffer,
        &mut tx_meta,
        &mut tx_buffer,
    );
    socket.bind(0).map_err(|_| UploadError::Time)?;
    socket
        .send_to(&request, remote)
        .await
        .map_err(|_| UploadError::Time)?;
    let (len, _) = with_timeout(
        Duration::from_secs(config::upload::READ_TIMEOUT_SECS),
        socket.recv_from(&mut response),
    )
    .await
    .map_err(|_| UploadError::Time)?
    .map_err(|_| UploadError::Time)?;

    parse_sntp_unix_ms(&response[..len]).map_err(|_| UploadError::Time)
}

#[cfg(all(target_arch = "riscv32", feature = "wifi-upload"))]
async fn sync_time_rest(stack: Stack<'static>, endpoint: Endpoint) -> Result<u64, UploadError> {
    let mut response = [0_u8; config::upload::RESPONSE_BUFFER_SIZE];
    let response = send_http_request(
        stack,
        endpoint,
        "GET",
        config::upload::TIME_PATH,
        None,
        &[],
        &mut response,
    )
    .await?;

    if !http_response_is_success(response) {
        return Err(UploadError::Time);
    }

    parse_rest_time_unix_ms(response).map_err(|_| UploadError::Time)
}

#[cfg(all(target_arch = "riscv32", feature = "wifi-upload"))]
fn stored_payload_uptime(payload: &[u8]) -> u64 {
    parse_json_u64(payload, "uptime_ms").unwrap_or(0)
}

#[cfg(all(target_arch = "riscv32", feature = "wifi-upload"))]
fn endpoint_host_header(endpoint: Endpoint, out: &mut [u8]) -> Result<usize, EncodeError> {
    let mut writer = FixedBufferWriter::new(out);
    write!(
        writer,
        "{}.{}.{}.{}:{}",
        endpoint.ipv4[0], endpoint.ipv4[1], endpoint.ipv4[2], endpoint.ipv4[3], endpoint.port
    )
    .map_err(|_| EncodeError::BufferTooSmall)?;
    Ok(writer.len())
}

#[cfg(all(target_arch = "riscv32", feature = "wifi-upload"))]
fn upload_result_for_error(error: UploadError) -> UploadResult {
    match error {
        UploadError::Discovery => UploadResult::DiscoveryFailed,
        UploadError::Time => UploadResult::TimeFailed,
        UploadError::Response => UploadResult::HttpFailed,
        UploadError::ConnectInvalidState
        | UploadError::ConnectReset
        | UploadError::ConnectTimedOut
        | UploadError::NoRoute
        | UploadError::Write
        | UploadError::Read
        | UploadError::Timeout => UploadResult::TransportFailed,
        UploadError::Encode => UploadResult::Failed,
    }
}

#[cfg(all(target_arch = "riscv32", feature = "wifi-upload"))]
fn map_connect_error(error: ConnectError) -> UploadError {
    match error {
        ConnectError::InvalidState => UploadError::ConnectInvalidState,
        ConnectError::ConnectionReset => UploadError::ConnectReset,
        ConnectError::TimedOut => UploadError::ConnectTimedOut,
        ConnectError::NoRoute => UploadError::NoRoute,
    }
}
