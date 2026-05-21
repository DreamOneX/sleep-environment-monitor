#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Sht40Error {
    I2c,
    InvalidTemperatureCrc,
    InvalidHumidityCrc,
}

pub const CMD_MEASURE_HIGH_PRECISION: u8 = 0xfd;

pub fn crc8(data: &[u8]) -> u8 {
    let mut crc = 0xff;

    for byte in data {
        crc ^= byte;
        for _ in 0..8 {
            if (crc & 0x80) != 0 {
                crc = (crc << 1) ^ 0x31;
            } else {
                crc <<= 1;
            }
        }
    }

    crc
}

pub fn convert_temperature(raw: u16) -> f32 {
    -45.0 + 175.0 * (raw as f32 / u16::MAX as f32)
}

pub fn convert_humidity(raw: u16) -> f32 {
    (-6.0 + 125.0 * (raw as f32 / u16::MAX as f32)).clamp(0.0, 100.0)
}

pub fn parse_measurement(buf: [u8; 6]) -> Result<(f32, f32), Sht40Error> {
    let temp_bytes = [buf[0], buf[1]];
    let humidity_bytes = [buf[3], buf[4]];

    if crc8(&temp_bytes) != buf[2] {
        return Err(Sht40Error::InvalidTemperatureCrc);
    }

    if crc8(&humidity_bytes) != buf[5] {
        return Err(Sht40Error::InvalidHumidityCrc);
    }

    let temp_raw = u16::from_be_bytes(temp_bytes);
    let humidity_raw = u16::from_be_bytes(humidity_bytes);

    Ok((
        convert_temperature(temp_raw),
        convert_humidity(humidity_raw),
    ))
}

pub fn start_measurement<I2C>(i2c: &mut I2C, address: u8) -> Result<(), Sht40Error>
where
    I2C: embedded_hal::i2c::I2c,
{
    i2c.write(address, &[CMD_MEASURE_HIGH_PRECISION])
        .map_err(|_| Sht40Error::I2c)
}

pub fn read_measurement<I2C>(i2c: &mut I2C, address: u8) -> Result<(f32, f32), Sht40Error>
where
    I2C: embedded_hal::i2c::I2c,
{
    let mut buf = [0_u8; 6];
    i2c.read(address, &mut buf).map_err(|_| Sht40Error::I2c)?;
    parse_measurement(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(actual: f32, expected: f32, epsilon: f32) {
        assert!(
            (actual - expected).abs() <= epsilon,
            "actual={actual}, expected={expected}, epsilon={epsilon}"
        );
    }

    #[test]
    fn crc_matches_known_vector() {
        assert_eq!(crc8(&[0xbe, 0xef]), 0x92);
    }

    #[test]
    fn valid_measurement_frame_parses() {
        let temp_raw = 0x6666_u16;
        let humidity_raw = 0x8000_u16;
        let [t_msb, t_lsb] = temp_raw.to_be_bytes();
        let [h_msb, h_lsb] = humidity_raw.to_be_bytes();
        let frame = [
            t_msb,
            t_lsb,
            crc8(&[t_msb, t_lsb]),
            h_msb,
            h_lsb,
            crc8(&[h_msb, h_lsb]),
        ];

        let (temperature, humidity) = parse_measurement(frame).unwrap();

        approx_eq(temperature, convert_temperature(temp_raw), 0.001);
        approx_eq(humidity, convert_humidity(humidity_raw), 0.001);
    }

    #[test]
    fn invalid_temperature_crc_returns_error() {
        let frame = [0x66, 0x66, 0x00, 0x80, 0x00, crc8(&[0x80, 0x00])];

        assert_eq!(
            parse_measurement(frame),
            Err(Sht40Error::InvalidTemperatureCrc)
        );
    }

    #[test]
    fn invalid_humidity_crc_returns_error() {
        let frame = [0x66, 0x66, crc8(&[0x66, 0x66]), 0x80, 0x00, 0x00];

        assert_eq!(
            parse_measurement(frame),
            Err(Sht40Error::InvalidHumidityCrc)
        );
    }

    #[test]
    fn raw_zero_temperature_conversion() {
        approx_eq(convert_temperature(0), -45.0, 0.001);
    }

    #[test]
    fn raw_max_temperature_conversion() {
        approx_eq(convert_temperature(u16::MAX), 130.0, 0.001);
    }

    #[test]
    fn humidity_clamps_to_sensor_range() {
        approx_eq(convert_humidity(0), 0.0, 0.001);
        approx_eq(convert_humidity(u16::MAX), 100.0, 0.001);
    }
}
