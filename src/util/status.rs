use crate::types::ErrorFlags;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LedPattern {
    Off,
    On,
    SlowBlink,
    FastBlink,
    Heartbeat,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LedState {
    pub led1: LedPattern,
    pub led2: LedPattern,
}

pub fn status_to_leds(flags: ErrorFlags, wifi_connected: bool) -> LedState {
    let led2 = if flags.intersects(ErrorFlags::SENSOR_MASK) {
        LedPattern::FastBlink
    } else if flags.contains(ErrorFlags::UPLOAD) {
        LedPattern::On
    } else if flags.contains(ErrorFlags::WIFI) || !wifi_connected {
        LedPattern::SlowBlink
    } else {
        LedPattern::Off
    };

    LedState {
        led1: LedPattern::Heartbeat,
        led2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_error_and_wifi_connected() {
        assert_eq!(
            status_to_leds(ErrorFlags::NONE, true),
            LedState {
                led1: LedPattern::Heartbeat,
                led2: LedPattern::Off,
            }
        );
    }

    #[test]
    fn no_error_and_wifi_disconnected() {
        assert_eq!(
            status_to_leds(ErrorFlags::NONE, false).led2,
            LedPattern::SlowBlink
        );
    }

    #[test]
    fn sensor_error_has_fast_blink() {
        assert_eq!(
            status_to_leds(ErrorFlags::OPT3001, true).led2,
            LedPattern::FastBlink
        );
    }

    #[test]
    fn upload_error_is_solid_on() {
        assert_eq!(
            status_to_leds(ErrorFlags::UPLOAD, true).led2,
            LedPattern::On
        );
    }

    #[test]
    fn multiple_errors_use_sensor_priority() {
        assert_eq!(
            status_to_leds(
                ErrorFlags::SHT40 | ErrorFlags::UPLOAD | ErrorFlags::WIFI,
                false
            )
            .led2,
            LedPattern::FastBlink
        );
    }
}
