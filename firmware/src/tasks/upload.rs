use core::fmt::{self, Write};

use crate::{
    config,
    types::{Measurement, TimeStatus},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EncodeError {
    BufferTooSmall,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParseError {
    MissingField,
    InvalidField,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResponseClass {
    Success,
    HttpFailure(u16),
    Malformed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EndpointSource {
    Provisioned,
    Discovered,
    StaticFallback,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Endpoint {
    pub ipv4: [u8; 4],
    pub port: u16,
    pub source: EndpointSource,
}

impl Endpoint {
    pub const fn static_fallback() -> Self {
        Self {
            ipv4: config::upload::FALLBACK_IPV4_OCTETS,
            port: config::upload::FALLBACK_PORT,
            source: EndpointSource::StaticFallback,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EndpointCandidates {
    pub provisioned: Option<Endpoint>,
    pub discovered: Option<Endpoint>,
    pub static_fallback: Endpoint,
}

impl EndpointCandidates {
    pub const fn fallback_only() -> Self {
        Self {
            provisioned: None,
            discovered: None,
            static_fallback: Endpoint::static_fallback(),
        }
    }
}

pub fn resolve_endpoint(candidates: EndpointCandidates) -> Endpoint {
    if let Some(endpoint) = candidates.provisioned {
        endpoint
    } else if let Some(endpoint) = candidates.discovered {
        endpoint
    } else {
        candidates.static_fallback
    }
}

pub fn measurement_to_json_fields(m: &Measurement, out: &mut [u8]) -> Result<usize, EncodeError> {
    let mut writer = FixedBufferWriter::new(out);

    write!(writer, "\"uptime_ms\":{},", m.uptime_ms).map_err(|_| EncodeError::BufferTooSmall)?;
    writer
        .write_str("\"temperature_c\":")
        .map_err(|_| EncodeError::BufferTooSmall)?;
    write_optional_json_f32(&mut writer, m.temperature_c)
        .map_err(|_| EncodeError::BufferTooSmall)?;
    writer
        .write_str(",\"humidity_percent\":")
        .map_err(|_| EncodeError::BufferTooSmall)?;
    write_optional_json_f32(&mut writer, m.humidity_percent)
        .map_err(|_| EncodeError::BufferTooSmall)?;
    writer
        .write_str(",\"lux\":")
        .map_err(|_| EncodeError::BufferTooSmall)?;
    write_optional_json_f32(&mut writer, m.lux).map_err(|_| EncodeError::BufferTooSmall)?;
    write!(
        writer,
        ",\"mic_mean\":{},\"mic_rms\":{},\"mic_peak\":{},\"mic_db_rel\":{},\"mic_clip_count\":{},\"error_flags\":{}",
        m.mic_mean,
        m.mic_rms,
        m.mic_peak,
        m.mic_db_rel,
        m.mic_clip_count,
        m.error_flags.bits()
    )
    .map_err(|_| EncodeError::BufferTooSmall)?;

    Ok(writer.len())
}

pub fn build_measurement_json(
    device_id: &str,
    sequence: u64,
    measurement_fields: &[u8],
    timestamp: TimestampSelection,
    out: &mut [u8],
) -> Result<usize, EncodeError> {
    let mut writer = FixedBufferWriter::new(out);

    write!(
        writer,
        "{{\"schema_version\":{},\"device_id\":\"",
        config::upload::SCHEMA_VERSION
    )
    .map_err(|_| EncodeError::BufferTooSmall)?;
    write_json_escaped_str(&mut writer, device_id).map_err(|_| EncodeError::BufferTooSmall)?;
    write!(
        writer,
        "\",\"sequence\":{},\"time_status\":\"{}\"",
        sequence,
        timestamp.status.as_json_str()
    )
    .map_err(|_| EncodeError::BufferTooSmall)?;
    if let Some(unix_ms) = timestamp.wall_clock_unix_ms {
        write!(writer, ",\"wall_clock_unix_ms\":{unix_ms}")
            .map_err(|_| EncodeError::BufferTooSmall)?;
    }
    writer
        .write_char(',')
        .map_err(|_| EncodeError::BufferTooSmall)?;
    writer
        .write_bytes(measurement_fields)
        .map_err(|_| EncodeError::BufferTooSmall)?;
    writer
        .write_char('}')
        .map_err(|_| EncodeError::BufferTooSmall)?;

    Ok(writer.len())
}

pub fn build_http_request(
    method: &str,
    host: &str,
    path: &str,
    content_type: Option<&str>,
    body: &[u8],
    out: &mut [u8],
) -> Result<usize, EncodeError> {
    let mut writer = FixedBufferWriter::new(out);

    write!(writer, "{method} {path} HTTP/1.1\r\n").map_err(|_| EncodeError::BufferTooSmall)?;
    write!(writer, "Host: {host}\r\n").map_err(|_| EncodeError::BufferTooSmall)?;
    write!(writer, "User-Agent: {}\r\n", config::upload::USER_AGENT)
        .map_err(|_| EncodeError::BufferTooSmall)?;
    if let Some(content_type) = content_type {
        write!(writer, "Content-Type: {content_type}\r\n")
            .map_err(|_| EncodeError::BufferTooSmall)?;
    }
    write!(writer, "Content-Length: {}\r\n", body.len())
        .map_err(|_| EncodeError::BufferTooSmall)?;
    writer
        .write_str("Connection: close\r\n\r\n")
        .map_err(|_| EncodeError::BufferTooSmall)?;
    writer
        .write_bytes(body)
        .map_err(|_| EncodeError::BufferTooSmall)?;

    Ok(writer.len())
}

pub fn http_response_class(response: &[u8]) -> ResponseClass {
    let Some(status) = http_status_code(response) else {
        return ResponseClass::Malformed;
    };

    if (200..=299).contains(&status) {
        ResponseClass::Success
    } else {
        ResponseClass::HttpFailure(status)
    }
}

pub fn http_response_is_success(response: &[u8]) -> bool {
    http_response_class(response) == ResponseClass::Success
}

pub fn parse_rest_time_unix_ms(response: &[u8]) -> Result<u64, ParseError> {
    let body = http_body(response).ok_or(ParseError::InvalidField)?;
    parse_json_u64(body, "unix_ms")
}

pub fn parse_discovery_endpoint(response: &[u8]) -> Result<Endpoint, ParseError> {
    let ipv4 = parse_json_ipv4(response, "host")?;
    let port = parse_json_u16(response, "port")?;
    Ok(endpoint_from_parts(ipv4, port, EndpointSource::Discovered))
}

pub fn parse_sntp_unix_ms(packet: &[u8]) -> Result<u64, ParseError> {
    if packet.len() < 48 {
        return Err(ParseError::InvalidField);
    }

    let mode = packet[0] & 0b0000_0111;
    if mode != 4 && mode != 5 {
        return Err(ParseError::InvalidField);
    }

    let seconds = u32::from_be_bytes([packet[40], packet[41], packet[42], packet[43]]) as u64;
    if seconds < config::upload::NTP_UNIX_EPOCH_DELTA_SECS {
        return Err(ParseError::InvalidField);
    }
    let fraction = u32::from_be_bytes([packet[44], packet[45], packet[46], packet[47]]) as u64;
    let unix_secs = seconds - config::upload::NTP_UNIX_EPOCH_DELTA_SECS;
    let millis = ((fraction as u128) * 1000_u128 / (1_u128 << 32)) as u64;
    unix_secs
        .checked_mul(1000)
        .and_then(|base| base.checked_add(millis))
        .ok_or(ParseError::InvalidField)
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TimeSyncState {
    pub boot_ms_at_sync: u64,
    pub unix_ms_at_sync: u64,
}

impl TimeSyncState {
    pub const fn new(boot_ms_at_sync: u64, unix_ms_at_sync: u64) -> Self {
        Self {
            boot_ms_at_sync,
            unix_ms_at_sync,
        }
    }

    pub fn wall_clock_for_uptime(self, uptime_ms: u64) -> Option<u64> {
        if uptime_ms >= self.boot_ms_at_sync {
            self.unix_ms_at_sync
                .checked_add(uptime_ms - self.boot_ms_at_sync)
        } else {
            self.unix_ms_at_sync
                .checked_sub(self.boot_ms_at_sync - uptime_ms)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimestampSelection {
    pub status: TimeStatus,
    pub wall_clock_unix_ms: Option<u64>,
}

pub fn select_timestamp(sync: Option<TimeSyncState>, uptime_ms: u64) -> TimestampSelection {
    match sync.and_then(|sync| sync.wall_clock_for_uptime(uptime_ms)) {
        Some(unix_ms) => TimestampSelection {
            status: TimeStatus::WallClockSynced,
            wall_clock_unix_ms: Some(unix_ms),
        },
        None => TimestampSelection {
            status: TimeStatus::UptimeOnly,
            wall_clock_unix_ms: None,
        },
    }
}

#[cfg(any(test, target_arch = "riscv32"))]
fn select_payload_timestamp(
    current_boot: bool,
    sync: Option<TimeSyncState>,
    uptime_ms: u64,
) -> TimestampSelection {
    let sync = if current_boot { sync } else { None };
    select_timestamp(sync, uptime_ms)
}

fn endpoint_from_parts(ipv4: [u8; 4], port: u16, source: EndpointSource) -> Endpoint {
    Endpoint { ipv4, port, source }
}

fn http_status_code(response: &[u8]) -> Option<u16> {
    let status = response.get(9..12)?;
    if !response.starts_with(b"HTTP/1.") || !status.iter().all(u8::is_ascii_digit) {
        return None;
    }

    Some(
        ((status[0] - b'0') as u16) * 100
            + ((status[1] - b'0') as u16) * 10
            + (status[2] - b'0') as u16,
    )
}

fn http_body(response: &[u8]) -> Option<&[u8]> {
    response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|index| &response[index + 4..])
}

#[cfg(any(test, target_arch = "riscv32"))]
fn http_response_total_len(response: &[u8]) -> Option<usize> {
    let header_end = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")?
        + 4;
    let headers = &response[..header_end];
    let content_len = http_content_length(headers)?;
    header_end.checked_add(content_len)
}

#[cfg(any(test, target_arch = "riscv32"))]
fn http_content_length(headers: &[u8]) -> Option<usize> {
    for line in headers.split(|byte| *byte == b'\n') {
        let line = trim_http_line(line);
        let Some((name, value)) = split_header(line) else {
            continue;
        };
        if ascii_eq_ignore_case(name, b"Content-Length") {
            return parse_usize_decimal(trim_http_line(value));
        }
    }

    None
}

#[cfg(any(test, target_arch = "riscv32"))]
fn split_header(line: &[u8]) -> Option<(&[u8], &[u8])> {
    let index = line.iter().position(|byte| *byte == b':')?;
    Some((&line[..index], &line[index + 1..]))
}

#[cfg(any(test, target_arch = "riscv32"))]
fn trim_http_line(mut input: &[u8]) -> &[u8] {
    while input
        .first()
        .is_some_and(|byte| matches!(*byte, b' ' | b'\t' | b'\r'))
    {
        input = &input[1..];
    }
    while input
        .last()
        .is_some_and(|byte| matches!(*byte, b' ' | b'\t' | b'\r'))
    {
        input = &input[..input.len() - 1];
    }
    input
}

#[cfg(any(test, target_arch = "riscv32"))]
fn ascii_eq_ignore_case(left: &[u8], right: &[u8]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| left.eq_ignore_ascii_case(right))
}

#[cfg(any(test, target_arch = "riscv32"))]
fn parse_usize_decimal(input: &[u8]) -> Option<usize> {
    let mut parsed = 0_usize;
    let mut consumed = false;
    for byte in input {
        if !byte.is_ascii_digit() {
            break;
        }
        parsed = parsed
            .checked_mul(10)?
            .checked_add((byte - b'0') as usize)?;
        consumed = true;
    }

    consumed.then_some(parsed)
}

fn parse_json_u64(input: &[u8], field: &str) -> Result<u64, ParseError> {
    let value = field_value(input, field).ok_or(ParseError::MissingField)?;
    let mut parsed = 0_u64;
    let mut consumed = false;

    for byte in value {
        if byte.is_ascii_digit() {
            parsed = parsed
                .checked_mul(10)
                .and_then(|current| current.checked_add((byte - b'0') as u64))
                .ok_or(ParseError::InvalidField)?;
            consumed = true;
        } else {
            break;
        }
    }

    if consumed {
        Ok(parsed)
    } else {
        Err(ParseError::InvalidField)
    }
}

fn parse_json_u16(input: &[u8], field: &str) -> Result<u16, ParseError> {
    let value = parse_json_u64(input, field)?;
    u16::try_from(value).map_err(|_| ParseError::InvalidField)
}

fn parse_json_ipv4(input: &[u8], field: &str) -> Result<[u8; 4], ParseError> {
    let value = field_value(input, field).ok_or(ParseError::MissingField)?;
    if value.first() != Some(&b'"') {
        return Err(ParseError::InvalidField);
    }

    let end = value[1..]
        .iter()
        .position(|byte| *byte == b'"')
        .ok_or(ParseError::InvalidField)?
        + 1;
    parse_ipv4_str(&value[1..end])
}

fn parse_ipv4_str(input: &[u8]) -> Result<[u8; 4], ParseError> {
    let mut octets = [0_u8; 4];
    let mut index = 0_usize;
    let mut current = 0_u16;
    let mut has_digit = false;

    for byte in input {
        match *byte {
            b'0'..=b'9' => {
                current = current
                    .checked_mul(10)
                    .and_then(|value| value.checked_add((*byte - b'0') as u16))
                    .ok_or(ParseError::InvalidField)?;
                if current > u8::MAX as u16 {
                    return Err(ParseError::InvalidField);
                }
                has_digit = true;
            }
            b'.' => {
                if !has_digit || index >= 3 {
                    return Err(ParseError::InvalidField);
                }
                octets[index] = current as u8;
                index += 1;
                current = 0;
                has_digit = false;
            }
            _ => return Err(ParseError::InvalidField),
        }
    }

    if !has_digit || index != 3 {
        return Err(ParseError::InvalidField);
    }
    octets[index] = current as u8;
    Ok(octets)
}

fn field_value<'a>(input: &'a [u8], field: &str) -> Option<&'a [u8]> {
    let needle_len = field.len() + 3;
    let mut needle = [0_u8; 64];
    if needle_len > needle.len() {
        return None;
    }
    needle[0] = b'"';
    needle[1..1 + field.len()].copy_from_slice(field.as_bytes());
    needle[1 + field.len()] = b'"';
    needle[2 + field.len()] = b':';

    let start = input
        .windows(needle_len)
        .position(|window| window == &needle[..needle_len])?
        + needle_len;
    Some(trim_json_ws(&input[start..]))
}

fn trim_json_ws(input: &[u8]) -> &[u8] {
    let mut start = 0;
    while input
        .get(start)
        .is_some_and(|byte| matches!(*byte, b' ' | b'\n' | b'\r' | b'\t'))
    {
        start += 1;
    }
    &input[start..]
}

fn write_optional_json_f32(writer: &mut FixedBufferWriter<'_>, value: Option<f32>) -> fmt::Result {
    match value {
        Some(value) if value.is_finite() => write!(writer, "{value}"),
        _ => writer.write_str("null"),
    }
}

fn write_json_escaped_str(writer: &mut FixedBufferWriter<'_>, value: &str) -> fmt::Result {
    for byte in value.bytes() {
        match byte {
            b'"' => writer.write_str("\\\"")?,
            b'\\' => writer.write_str("\\\\")?,
            0x20..=0x7e => writer.write_char(byte as char)?,
            _ => writer.write_char('?')?,
        }
    }
    Ok(())
}

struct FixedBufferWriter<'a> {
    buf: &'a mut [u8],
    len: usize,
}

impl<'a> FixedBufferWriter<'a> {
    fn new(buf: &'a mut [u8]) -> Self {
        Self { buf, len: 0 }
    }

    fn len(&self) -> usize {
        self.len
    }

    fn write_bytes(&mut self, bytes: &[u8]) -> fmt::Result {
        let end = self.len.checked_add(bytes.len()).ok_or(fmt::Error)?;
        let destination = self.buf.get_mut(self.len..end).ok_or(fmt::Error)?;

        destination.copy_from_slice(bytes);
        self.len = end;

        Ok(())
    }
}

impl Write for FixedBufferWriter<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_bytes(s.as_bytes())
    }
}

#[cfg(target_arch = "riscv32")]
use crate::{
    tasks::{
        NetworkUploadStatusMutex, StorageRequestChannel, StorageResponseSignal, TaskSignal,
        storage::{StorageClient, StorageCommand, StorageResponse, StoredPayload},
    },
    types::{NetworkState, UploadResult},
};
#[cfg(target_arch = "riscv32")]
use defmt::{info, warn};
#[cfg(target_arch = "riscv32")]
use embassy_net::{
    IpAddress, IpEndpoint, Stack,
    tcp::{ConnectError, TcpSocket},
    udp::{PacketMetadata, UdpSocket},
};
#[cfg(target_arch = "riscv32")]
use embassy_time::{Duration, Instant, Timer, with_timeout};
#[cfg(target_arch = "riscv32")]
use embedded_io_async::Write as _;

#[cfg(target_arch = "riscv32")]
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

#[cfg(target_arch = "riscv32")]
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

#[cfg(target_arch = "riscv32")]
async fn publish_network_state(
    network_state: &'static TaskSignal<NetworkState>,
    network_upload_status: &'static NetworkUploadStatusMutex,
    state: NetworkState,
) {
    network_state.signal(state);
    let mut status = network_upload_status.lock().await;
    *status = status.with_network_state(state);
}

#[cfg(target_arch = "riscv32")]
async fn publish_upload_result(
    upload_result: &'static TaskSignal<UploadResult>,
    network_upload_status: &'static NetworkUploadStatusMutex,
    result: UploadResult,
) {
    upload_result.signal(result);
    let mut status = network_upload_status.lock().await;
    *status = status.with_upload_result(result);
}

#[cfg(target_arch = "riscv32")]
fn should_retry(last_attempt_ms: Option<u64>, now_ms: u64, interval_secs: u64) -> bool {
    last_attempt_ms
        .is_none_or(|last| now_ms.saturating_sub(last) >= interval_secs.saturating_mul(1000))
}

#[cfg(target_arch = "riscv32")]
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

#[cfg(target_arch = "riscv32")]
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

#[cfg(target_arch = "riscv32")]
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

#[cfg(target_arch = "riscv32")]
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

#[cfg(target_arch = "riscv32")]
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

#[cfg(target_arch = "riscv32")]
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

#[cfg(target_arch = "riscv32")]
async fn sync_time(stack: Stack<'static>, endpoint: Endpoint) -> Result<u64, UploadError> {
    match sync_time_sntp(stack).await {
        Ok(unix_ms) => Ok(unix_ms),
        Err(_) => sync_time_rest(stack, endpoint).await,
    }
}

#[cfg(target_arch = "riscv32")]
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

#[cfg(target_arch = "riscv32")]
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

#[cfg(target_arch = "riscv32")]
fn stored_payload_uptime(payload: &[u8]) -> u64 {
    parse_json_u64(payload, "uptime_ms").unwrap_or(0)
}

#[cfg(target_arch = "riscv32")]
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

#[cfg(target_arch = "riscv32")]
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

#[cfg(target_arch = "riscv32")]
fn map_connect_error(error: ConnectError) -> UploadError {
    match error {
        ConnectError::InvalidState => UploadError::ConnectInvalidState,
        ConnectError::ConnectionReset => UploadError::ConnectReset,
        ConnectError::TimedOut => UploadError::ConnectTimedOut,
        ConnectError::NoRoute => UploadError::NoRoute,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ErrorFlags;

    fn complete_measurement() -> Measurement {
        Measurement {
            uptime_ms: 1234,
            temperature_c: Some(21.5),
            humidity_percent: Some(45.25),
            lux: Some(9.75),
            mic_mean: 2048.0,
            mic_rms: 10.5,
            mic_peak: 99.0,
            mic_db_rel: 20.4,
            mic_clip_count: 2,
            error_flags: ErrorFlags::SHT40 | ErrorFlags::UPLOAD,
        }
    }

    fn encode_fields_to_str<'a>(
        m: &Measurement,
        out: &'a mut [u8],
    ) -> Result<&'a str, EncodeError> {
        let len = measurement_to_json_fields(m, out)?;
        Ok(core::str::from_utf8(&out[..len]).unwrap())
    }

    #[test]
    fn measurement_fields_encode_json_members() {
        let mut out = [0_u8; 256];

        assert_eq!(
            encode_fields_to_str(&complete_measurement(), &mut out).unwrap(),
            "\"uptime_ms\":1234,\"temperature_c\":21.5,\"humidity_percent\":45.25,\"lux\":9.75,\"mic_mean\":2048,\"mic_rms\":10.5,\"mic_peak\":99,\"mic_db_rel\":20.4,\"mic_clip_count\":2,\"error_flags\":17"
        );
    }

    #[test]
    fn missing_values_encode_as_json_null() {
        let mut measurement = complete_measurement();
        let mut out = [0_u8; 256];
        measurement.temperature_c = None;
        measurement.humidity_percent = None;
        measurement.lux = None;

        assert!(
            encode_fields_to_str(&measurement, &mut out)
                .unwrap()
                .contains("\"temperature_c\":null,\"humidity_percent\":null,\"lux\":null")
        );
    }

    #[test]
    fn json_payload_includes_sequence_and_wall_clock_when_synced() {
        let mut fields = [0_u8; 256];
        let fields_len = measurement_to_json_fields(&complete_measurement(), &mut fields).unwrap();
        let mut payload = [0_u8; 512];
        let payload_len = build_measurement_json(
            "device-1",
            7,
            &fields[..fields_len],
            TimestampSelection {
                status: TimeStatus::WallClockSynced,
                wall_clock_unix_ms: Some(1_700_000_000_000),
            },
            &mut payload,
        )
        .unwrap();
        let payload = core::str::from_utf8(&payload[..payload_len]).unwrap();

        assert!(payload.starts_with("{\"schema_version\":1,\"device_id\":\"device-1\""));
        assert!(payload.contains("\"sequence\":7"));
        assert!(payload.contains("\"time_status\":\"wall_clock_synced\""));
        assert!(payload.contains("\"wall_clock_unix_ms\":1700000000000"));
        assert!(payload.contains("\"uptime_ms\":1234"));
    }

    #[test]
    fn json_payload_omits_wall_clock_when_unknown() {
        let fields = b"\"uptime_ms\":42,\"temperature_c\":null";
        let mut payload = [0_u8; 256];
        let payload_len = build_measurement_json(
            "device-1",
            1,
            fields,
            TimestampSelection {
                status: TimeStatus::UptimeOnly,
                wall_clock_unix_ms: None,
            },
            &mut payload,
        )
        .unwrap();
        let payload = core::str::from_utf8(&payload[..payload_len]).unwrap();

        assert!(payload.contains("\"time_status\":\"uptime_only\""));
        assert!(!payload.contains("wall_clock_unix_ms"));
    }

    #[test]
    fn http_post_request_wraps_json_body() {
        let mut request = [0_u8; 512];
        let request_len = build_http_request(
            "POST",
            "10.133.56.218:8080",
            "/api/v1/measurements",
            Some("application/json"),
            b"{\"ok\":true}",
            &mut request,
        )
        .unwrap();
        let request = core::str::from_utf8(&request[..request_len]).unwrap();

        assert!(request.starts_with("POST /api/v1/measurements HTTP/1.1\r\n"));
        assert!(request.contains("Host: 10.133.56.218:8080\r\n"));
        assert!(request.contains("Content-Type: application/json\r\n"));
        assert!(request.contains("Content-Length: 11\r\n"));
        assert!(request.ends_with("{\"ok\":true}"));
    }

    #[test]
    fn http_response_class_accepts_2xx_only() {
        assert_eq!(
            http_response_class(b"HTTP/1.1 204 No Content\r\n\r\n"),
            ResponseClass::Success
        );
        assert_eq!(
            http_response_class(b"HTTP/1.1 500 Internal Server Error\r\n\r\n"),
            ResponseClass::HttpFailure(500)
        );
        assert_eq!(
            http_response_class(b"HTTP/1.1 302 Found\r\n\r\n"),
            ResponseClass::HttpFailure(302)
        );
        assert_eq!(
            http_response_class(b"bad response"),
            ResponseClass::Malformed
        );
    }

    #[test]
    fn http_response_total_len_uses_content_length() {
        assert_eq!(
            http_response_total_len(
                b"HTTP/1.1 200 OK\r\nContent-Length: 13\r\n\r\n{\"unix_ms\":1}"
            ),
            Some(52)
        );
        assert_eq!(
            http_response_total_len(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n"),
            Some(46)
        );
        assert_eq!(
            http_response_total_len(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\n{}"),
            Some(40)
        );
        assert_eq!(http_response_total_len(b"HTTP/1.1 200 OK\r\n\r\n{}"), None);
    }

    #[test]
    fn endpoint_resolution_prefers_provisioned_then_discovered_then_static() {
        let static_fallback = Endpoint::static_fallback();
        let discovered = Endpoint {
            ipv4: [192, 168, 1, 10],
            port: 8080,
            source: EndpointSource::Discovered,
        };
        let provisioned = Endpoint {
            ipv4: [10, 0, 0, 2],
            port: 9000,
            source: EndpointSource::Provisioned,
        };

        assert_eq!(
            resolve_endpoint(EndpointCandidates {
                provisioned: Some(provisioned),
                discovered: Some(discovered),
                static_fallback,
            }),
            provisioned
        );
        assert_eq!(
            resolve_endpoint(EndpointCandidates {
                provisioned: None,
                discovered: Some(discovered),
                static_fallback,
            }),
            discovered
        );
        assert_eq!(
            resolve_endpoint(EndpointCandidates::fallback_only()),
            static_fallback
        );
    }

    #[test]
    fn discovery_response_parses_endpoint() {
        let endpoint = parse_discovery_endpoint(
            br#"{"host":"192.168.1.44","port":8080,"api_base":"/api/v1"}"#,
        )
        .unwrap();

        assert_eq!(endpoint.ipv4, [192, 168, 1, 44]);
        assert_eq!(endpoint.port, 8080);
        assert_eq!(endpoint.source, EndpointSource::Discovered);
    }

    #[test]
    fn rest_time_response_parses_unix_ms() {
        assert_eq!(
            parse_rest_time_unix_ms(
                b"HTTP/1.1 200 OK\r\n\r\n{\"unix_ms\":1700000000123,\"source\":\"server\"}"
            ),
            Ok(1_700_000_000_123)
        );
    }

    #[test]
    fn sntp_response_parses_transmit_timestamp() {
        let mut packet = [0_u8; 48];
        packet[0] = 0b00_100_100;
        let seconds = config::upload::NTP_UNIX_EPOCH_DELTA_SECS + 1_700_000_000;
        packet[40..44].copy_from_slice(&(seconds as u32).to_be_bytes());
        packet[44..48].copy_from_slice(&0x8000_0000_u32.to_be_bytes());

        assert_eq!(parse_sntp_unix_ms(&packet), Ok(1_700_000_000_500));
    }

    #[test]
    fn timestamp_selection_uses_wall_clock_when_available() {
        let sync = TimeSyncState::new(1_000, 1_700_000_000_000);

        assert_eq!(
            select_timestamp(Some(sync), 1_250),
            TimestampSelection {
                status: TimeStatus::WallClockSynced,
                wall_clock_unix_ms: Some(1_700_000_000_250),
            }
        );
        assert_eq!(
            select_timestamp(None, 1_250),
            TimestampSelection {
                status: TimeStatus::UptimeOnly,
                wall_clock_unix_ms: None,
            }
        );
    }

    #[test]
    fn recovered_payloads_do_not_use_current_boot_time_sync() {
        let sync = TimeSyncState::new(1_000, 1_700_000_000_000);

        assert_eq!(
            select_payload_timestamp(false, Some(sync), 1_250),
            TimestampSelection {
                status: TimeStatus::UptimeOnly,
                wall_clock_unix_ms: None,
            }
        );
        assert_eq!(
            select_payload_timestamp(true, Some(sync), 1_250),
            TimestampSelection {
                status: TimeStatus::WallClockSynced,
                wall_clock_unix_ms: Some(1_700_000_000_250),
            }
        );
    }
}
