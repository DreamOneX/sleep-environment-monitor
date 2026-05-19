pub const CONFIG_CONTINUOUS: u16 = 0xce10;

pub const fn exponent(raw: u16) -> u8 {
    ((raw >> 12) & 0x0f) as u8
}

pub const fn mantissa(raw: u16) -> u16 {
    raw & 0x0fff
}

pub fn raw_to_lux(raw: u16) -> f32 {
    mantissa(raw) as f32 * 0.01 * (1_u32 << exponent(raw)) as f32
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
