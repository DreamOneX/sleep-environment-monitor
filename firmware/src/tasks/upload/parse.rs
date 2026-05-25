pub fn parse_rest_time_unix_ms(response: &[u8]) -> Result<u64, ParseError> {
    let body = http_body(response).ok_or(ParseError::InvalidField)?;
    parse_json_u64(body, "unix_ms")
}

pub fn parse_discovery_endpoint(response: &[u8]) -> Result<Endpoint, ParseError> {
    let ipv4 = parse_json_ipv4(response, "host")?;
    let port = parse_json_u16(response, "port")?;
    Ok(endpoint_from_parts(ipv4, port, EndpointSource::Discovered))
}
fn endpoint_from_parts(ipv4: [u8; 4], port: u16, source: EndpointSource) -> Endpoint {
    Endpoint { ipv4, port, source }
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
