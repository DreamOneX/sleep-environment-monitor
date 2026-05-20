use core::fmt::{self, Write};

use crate::types::Measurement;

#[cfg(target_arch = "riscv32")]
const UPLOAD_HOST_HEADER: &str = "10.133.56.218:8080";
#[cfg(target_arch = "riscv32")]
const UPLOAD_PATH: &str = "/measurements";
#[cfg(target_arch = "riscv32")]
const UPLOAD_PORT: u16 = 8080;
#[cfg(target_arch = "riscv32")]
const UPLOAD_RETRY_DELAY_SECS: u64 = 2;
#[cfg(target_arch = "riscv32")]
const UPLOAD_SUCCESS_LOG_EVERY: u32 = 60;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EncodeError {
    BufferTooSmall,
}

pub fn measurement_to_csv_line(m: &Measurement, out: &mut [u8]) -> Result<usize, EncodeError> {
    let mut writer = FixedBufferWriter::new(out);

    write!(writer, "{},", m.uptime_ms).map_err(|_| EncodeError::BufferTooSmall)?;
    write_optional_f32(&mut writer, m.temperature_c).map_err(|_| EncodeError::BufferTooSmall)?;
    writer
        .write_char(',')
        .map_err(|_| EncodeError::BufferTooSmall)?;
    write_optional_f32(&mut writer, m.humidity_percent).map_err(|_| EncodeError::BufferTooSmall)?;
    writer
        .write_char(',')
        .map_err(|_| EncodeError::BufferTooSmall)?;
    write_optional_f32(&mut writer, m.lux).map_err(|_| EncodeError::BufferTooSmall)?;
    write!(
        writer,
        ",{},{},{},{},{},{}",
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

pub fn build_http_post_request(
    host: &str,
    path: &str,
    body: &[u8],
    out: &mut [u8],
) -> Result<usize, EncodeError> {
    let mut writer = FixedBufferWriter::new(out);

    write!(writer, "POST {path} HTTP/1.1\r\n").map_err(|_| EncodeError::BufferTooSmall)?;
    write!(writer, "Host: {host}\r\n").map_err(|_| EncodeError::BufferTooSmall)?;
    writer
        .write_str("User-Agent: sleep-environment-monitor/0.1\r\n")
        .map_err(|_| EncodeError::BufferTooSmall)?;
    writer
        .write_str("Content-Type: text/csv\r\n")
        .map_err(|_| EncodeError::BufferTooSmall)?;
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

pub fn http_response_is_success(response: &[u8]) -> bool {
    let Some(status) = response.get(9..12) else {
        return false;
    };

    response.starts_with(b"HTTP/1.")
        && status[0] == b'2'
        && status[1].is_ascii_digit()
        && status[2].is_ascii_digit()
}

fn write_optional_f32(writer: &mut FixedBufferWriter<'_>, value: Option<f32>) -> fmt::Result {
    match value {
        Some(value) if value.is_nan() => writer.write_str("nan"),
        Some(value) => write!(writer, "{value}"),
        None => writer.write_str("nan"),
    }
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
        let bytes = s.as_bytes();
        let end = self.len.checked_add(bytes.len()).ok_or(fmt::Error)?;
        let destination = self.buf.get_mut(self.len..end).ok_or(fmt::Error)?;

        destination.copy_from_slice(bytes);
        self.len = end;

        Ok(())
    }
}

#[cfg(target_arch = "riscv32")]
use crate::{
    tasks::{
        StorageRequestChannel, StorageResponseSignal, TaskSignal,
        storage::{StorageCommand, StorageResponse, StoredPayload},
    },
    types::UploadResult,
};
#[cfg(target_arch = "riscv32")]
use defmt::{info, warn};
#[cfg(target_arch = "riscv32")]
use embassy_net::{
    IpAddress, IpEndpoint, Stack,
    tcp::{ConnectError, TcpSocket},
};
#[cfg(target_arch = "riscv32")]
use embassy_time::{Duration, Timer, with_timeout};
#[cfg(target_arch = "riscv32")]
use embedded_io_async::Write as _;

#[cfg(target_arch = "riscv32")]
#[derive(Clone, Copy, Debug, Eq, PartialEq, defmt::Format)]
enum UploadError {
    Encode,
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
    upload_result: &'static TaskSignal<UploadResult>,
) {
    let mut logged_network_config = false;
    let mut upload_success_count = 0_u32;

    loop {
        stack.wait_config_up().await;
        if !logged_network_config && let Some(config) = stack.config_v4() {
            info!("network ipv4 config={:?}", config);
            logged_network_config = true;
        }

        let Some(payload) = peek_payload(storage_requests, storage_responses).await else {
            Timer::after(Duration::from_millis(250)).await;
            continue;
        };

        match post_csv_payload(stack, payload.as_slice()).await {
            Ok(()) => {
                let acked = acknowledge_payload(storage_requests, storage_responses).await;
                upload_result.signal(UploadResult::Success);
                if !acked
                    || upload_success_count == 0
                    || upload_success_count.is_multiple_of(UPLOAD_SUCCESS_LOG_EVERY)
                {
                    info!(
                        "upload success sequence={=u64} acked={=bool}",
                        payload.sequence, acked
                    );
                }
                upload_success_count = upload_success_count.wrapping_add(1);
            }
            Err(error) => {
                upload_result.signal(UploadResult::Failed);
                warn!(
                    "upload failed error={:?} sequence={=u64}",
                    error, payload.sequence
                );
                Timer::after(Duration::from_secs(UPLOAD_RETRY_DELAY_SECS)).await;
            }
        }
    }
}

#[cfg(target_arch = "riscv32")]
async fn peek_payload(
    storage_requests: &StorageRequestChannel,
    storage_responses: &StorageResponseSignal,
) -> Option<StoredPayload> {
    storage_requests.send(StorageCommand::Peek).await;
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
) -> bool {
    storage_requests.send(StorageCommand::Ack).await;
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
async fn post_csv_payload(stack: Stack<'static>, body: &[u8]) -> Result<(), UploadError> {
    let mut rx_buffer = [0_u8; 512];
    let mut tx_buffer = [0_u8; 512];
    let mut request = [0_u8; 512];
    let mut response = [0_u8; 128];

    let request_len = build_http_post_request(UPLOAD_HOST_HEADER, UPLOAD_PATH, body, &mut request)
        .map_err(|_| UploadError::Encode)?;

    let endpoint = IpEndpoint::new(IpAddress::v4(10, 133, 56, 218), UPLOAD_PORT);
    let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);
    socket.set_timeout(Some(Duration::from_secs(10)));

    socket.connect(endpoint).await.map_err(map_connect_error)?;
    socket
        .write_all(&request[..request_len])
        .await
        .map_err(|_| UploadError::Write)?;
    socket.flush().await.map_err(|_| UploadError::Write)?;

    let read_result = with_timeout(Duration::from_secs(10), socket.read(&mut response))
        .await
        .map_err(|_| UploadError::Timeout)?
        .map_err(|_| UploadError::Read)?;

    socket.close();

    if read_result == 0 {
        return Err(UploadError::Response);
    }

    if http_response_is_success(&response[..read_result]) {
        Ok(())
    } else {
        Err(UploadError::Response)
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

    fn encode_to_str<'a>(m: &Measurement, out: &'a mut [u8]) -> Result<&'a str, EncodeError> {
        let len = measurement_to_csv_line(m, out)?;
        Ok(core::str::from_utf8(&out[..len]).unwrap())
    }

    #[test]
    fn complete_measurement_encodes_correctly() {
        let mut out = [0_u8; 128];

        assert_eq!(
            encode_to_str(&complete_measurement(), &mut out).unwrap(),
            "1234,21.5,45.25,9.75,2048,10.5,99,20.4,2,17"
        );
    }

    #[test]
    fn missing_values_encode_as_nan() {
        let mut measurement = complete_measurement();
        let mut out = [0_u8; 128];
        measurement.temperature_c = None;
        measurement.humidity_percent = None;
        measurement.lux = None;

        assert_eq!(
            encode_to_str(&measurement, &mut out).unwrap(),
            "1234,nan,nan,nan,2048,10.5,99,20.4,2,17"
        );
    }

    #[test]
    fn small_output_buffer_returns_error() {
        let mut out = [0_u8; 8];

        assert_eq!(
            measurement_to_csv_line(&complete_measurement(), &mut out),
            Err(EncodeError::BufferTooSmall)
        );
    }

    #[test]
    fn error_flags_encode_as_bits() {
        let mut measurement = complete_measurement();
        let mut out = [0_u8; 128];
        measurement.error_flags = ErrorFlags::OPT3001 | ErrorFlags::MIC;

        assert!(
            encode_to_str(&measurement, &mut out)
                .unwrap()
                .ends_with(",6")
        );
    }

    #[test]
    fn function_never_panics_on_tiny_buffers() {
        for size in 0..8 {
            let mut out = [0_u8; 8];
            let _ = measurement_to_csv_line(&complete_measurement(), &mut out[..size]);
        }
    }

    #[test]
    fn http_post_request_wraps_csv_body() {
        let mut body = [0_u8; 128];
        let mut request = [0_u8; 512];
        let body_len = measurement_to_csv_line(&complete_measurement(), &mut body).unwrap();

        let request_len = build_http_post_request(
            "10.133.56.218:8080",
            "/measurements",
            &body[..body_len],
            &mut request,
        )
        .unwrap();
        let request = core::str::from_utf8(&request[..request_len]).unwrap();

        assert!(request.starts_with("POST /measurements HTTP/1.1\r\n"));
        assert!(request.contains("Host: 10.133.56.218:8080\r\n"));
        assert!(request.contains("Content-Type: text/csv\r\n"));
        assert!(request.contains(&format!("Content-Length: {body_len}\r\n")));
        assert!(request.ends_with("1234,21.5,45.25,9.75,2048,10.5,99,20.4,2,17"));
    }

    #[test]
    fn http_post_request_reports_small_buffer() {
        let mut request = [0_u8; 16];

        assert_eq!(
            build_http_post_request("host", "/measurements", b"1,2,3", &mut request),
            Err(EncodeError::BufferTooSmall)
        );
    }

    #[test]
    fn http_response_success_accepts_2xx_only() {
        assert!(http_response_is_success(b"HTTP/1.1 200 OK\r\n\r\n"));
        assert!(http_response_is_success(b"HTTP/1.0 204 No Content\r\n\r\n"));
        assert!(!http_response_is_success(
            b"HTTP/1.1 500 Internal Server Error\r\n\r\n"
        ));
        assert!(!http_response_is_success(b"HTTP/1.1 302 Found\r\n\r\n"));
        assert!(!http_response_is_success(b"bad response"));
    }
}
