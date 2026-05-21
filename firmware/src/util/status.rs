use crate::types::{ErrorFlags, UploadResult};

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

pub fn status_to_leds(flags: ErrorFlags, network_ready: bool) -> LedState {
    let led2 = if flags.intersects(ErrorFlags::SENSOR_MASK) {
        LedPattern::FastBlink
    } else if flags.contains(ErrorFlags::STORAGE)
        || flags.intersects(ErrorFlags::UPLOAD_MASK)
        || flags.contains(ErrorFlags::TIME)
    {
        LedPattern::On
    } else if flags.intersects(ErrorFlags::NETWORK_MASK) || !network_ready {
        LedPattern::SlowBlink
    } else {
        LedPattern::Off
    };

    LedState {
        led1: LedPattern::Heartbeat,
        led2,
    }
}

pub fn status_error_flags(
    measurement_flags: ErrorFlags,
    upload_result: UploadResult,
) -> ErrorFlags {
    let mut flags = measurement_flags;

    match upload_result {
        UploadResult::Idle | UploadResult::Success => {}
        UploadResult::Failed => flags.insert(ErrorFlags::UPLOAD),
        UploadResult::DiscoveryFailed => flags.insert(ErrorFlags::DISCOVERY),
        UploadResult::TimeFailed => flags.insert(ErrorFlags::TIME),
        UploadResult::TransportFailed => flags.insert(ErrorFlags::UPLOAD | ErrorFlags::TRANSPORT),
        UploadResult::HttpFailed => flags.insert(ErrorFlags::UPLOAD | ErrorFlags::HTTP),
    }

    flags
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_error_and_network_ready() {
        assert_eq!(
            status_to_leds(ErrorFlags::NONE, true),
            LedState {
                led1: LedPattern::Heartbeat,
                led2: LedPattern::Off,
            }
        );
    }

    #[test]
    fn no_error_and_network_not_ready() {
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
    fn storage_error_is_solid_on() {
        assert_eq!(
            status_to_leds(ErrorFlags::STORAGE, true).led2,
            LedPattern::On
        );
    }

    #[test]
    fn multiple_errors_use_sensor_priority() {
        assert_eq!(
            status_to_leds(
                ErrorFlags::SHT40 | ErrorFlags::UPLOAD | ErrorFlags::WIFI | ErrorFlags::STORAGE,
                false
            )
            .led2,
            LedPattern::FastBlink
        );
    }

    #[test]
    fn upload_failure_adds_upload_flag() {
        assert!(
            status_error_flags(ErrorFlags::NONE, UploadResult::Failed).contains(ErrorFlags::UPLOAD)
        );
    }

    #[test]
    fn detailed_upload_failures_add_distinct_flags() {
        assert!(
            status_error_flags(ErrorFlags::NONE, UploadResult::DiscoveryFailed)
                .contains(ErrorFlags::DISCOVERY)
        );
        assert!(
            status_error_flags(ErrorFlags::NONE, UploadResult::TimeFailed)
                .contains(ErrorFlags::TIME)
        );
        assert!(
            status_error_flags(ErrorFlags::NONE, UploadResult::TransportFailed)
                .contains(ErrorFlags::TRANSPORT)
        );
        assert!(
            status_error_flags(ErrorFlags::NONE, UploadResult::HttpFailed)
                .contains(ErrorFlags::HTTP)
        );
    }

    #[test]
    fn upload_success_does_not_add_upload_flag() {
        assert!(
            !status_error_flags(ErrorFlags::SHT40, UploadResult::Success)
                .contains(ErrorFlags::UPLOAD)
        );
        assert!(
            status_error_flags(ErrorFlags::SHT40, UploadResult::Success)
                .contains(ErrorFlags::SHT40)
        );
    }
}
