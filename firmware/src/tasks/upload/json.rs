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
