pub const CONFIG_CONTINUOUS: u16 = 0xce10;
pub const REG_RESULT: u8 = 0x00;
pub const REG_CONFIG: u8 = 0x01;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Opt3001Error {
    I2c,
}

pub const fn exponent(raw: u16) -> u8 {
    ((raw >> 12) & 0x0f) as u8
}

pub const fn mantissa(raw: u16) -> u16 {
    raw & 0x0fff
}

pub fn raw_to_lux(raw: u16) -> f32 {
    mantissa(raw) as f32 * 0.01 * (1_u32 << exponent(raw)) as f32
}

pub fn configure_continuous<I2C>(i2c: &mut I2C, address: u8) -> Result<(), Opt3001Error>
where
    I2C: embedded_hal::i2c::I2c,
{
    write_register(i2c, address, REG_CONFIG, CONFIG_CONTINUOUS)
}

pub fn read_lux<I2C>(i2c: &mut I2C, address: u8) -> Result<f32, Opt3001Error>
where
    I2C: embedded_hal::i2c::I2c,
{
    let raw = read_register(i2c, address, REG_RESULT)?;
    Ok(raw_to_lux(raw))
}

fn write_register<I2C>(
    i2c: &mut I2C,
    address: u8,
    register: u8,
    value: u16,
) -> Result<(), Opt3001Error>
where
    I2C: embedded_hal::i2c::I2c,
{
    let [msb, lsb] = value.to_be_bytes();
    i2c.write(address, &[register, msb, lsb])
        .map_err(|_| Opt3001Error::I2c)
}

fn read_register<I2C>(i2c: &mut I2C, address: u8, register: u8) -> Result<u16, Opt3001Error>
where
    I2C: embedded_hal::i2c::I2c,
{
    let mut buf = [0_u8; 2];
    i2c.write_read(address, &[register], &mut buf)
        .map_err(|_| Opt3001Error::I2c)?;
    Ok(u16::from_be_bytes(buf))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_raw_is_zero_lux() {
        assert_eq!(raw_to_lux(0x0000), 0.0);
    }

    #[test]
    fn exponent_zero_lsb_is_001_lux() {
        assert_eq!(raw_to_lux(0x0001), 0.01);
    }

    #[test]
    fn exponent_one_doubles_lsb() {
        assert_eq!(raw_to_lux(0x1001), 0.02);
    }

    #[test]
    fn exponent_two_quadruples_lsb() {
        assert_eq!(raw_to_lux(0x2001), 0.04);
    }

    #[test]
    fn extracts_exponent_and_mantissa() {
        let raw = 0xa123;

        assert_eq!(exponent(raw), 0x0a);
        assert_eq!(mantissa(raw), 0x0123);
    }
}
