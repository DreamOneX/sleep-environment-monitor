use core::fmt::{self, Write};

use crate::types::Measurement;

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
}
