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

#[cfg(any(test, all(target_arch = "riscv32", feature = "wifi-upload")))]
fn http_response_total_len(response: &[u8]) -> Option<usize> {
    let header_end = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")?
        + 4;
    let headers = &response[..header_end];
    let content_len = http_content_length(headers)?;
    header_end.checked_add(content_len)
}

#[cfg(any(test, all(target_arch = "riscv32", feature = "wifi-upload")))]
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

#[cfg(any(test, all(target_arch = "riscv32", feature = "wifi-upload")))]
fn split_header(line: &[u8]) -> Option<(&[u8], &[u8])> {
    let index = line.iter().position(|byte| *byte == b':')?;
    Some((&line[..index], &line[index + 1..]))
}

#[cfg(any(test, all(target_arch = "riscv32", feature = "wifi-upload")))]
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

#[cfg(any(test, all(target_arch = "riscv32", feature = "wifi-upload")))]
fn ascii_eq_ignore_case(left: &[u8], right: &[u8]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| left.eq_ignore_ascii_case(right))
}

#[cfg(any(test, all(target_arch = "riscv32", feature = "wifi-upload")))]
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
