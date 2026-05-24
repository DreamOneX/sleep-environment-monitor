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
    pub red_led2: LedPattern,
    pub blue_led3: LedPattern,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[cfg_attr(target_arch = "riscv32", derive(defmt::Format))]
pub enum BleLedRuntimeState {
    #[default]
    Disabled,
    Idle,
    Advertising,
    Connected,
    Error,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[cfg_attr(target_arch = "riscv32", derive(defmt::Format))]
pub struct BleLedPairingStatus {
    pub window_open: bool,
    pub button_pressed: bool,
}

pub const fn ble_status_to_led(
    runtime_state: BleLedRuntimeState,
    pairing_status: BleLedPairingStatus,
    indication_active: bool,
) -> Option<LedPattern> {
    if pairing_status.window_open {
        return Some(LedPattern::FastBlink);
    }
    if !indication_active {
        return None;
    }

    Some(match runtime_state {
        BleLedRuntimeState::Advertising | BleLedRuntimeState::Connected => LedPattern::SlowBlink,
        BleLedRuntimeState::Error => LedPattern::On,
        BleLedRuntimeState::Disabled | BleLedRuntimeState::Idle => LedPattern::Off,
    })
}

pub const fn initial_ble_indication_until_millis(ble_enabled: bool, boot_window_secs: u64) -> u64 {
    if ble_enabled {
        boot_window_secs.saturating_mul(1_000)
    } else {
        0
    }
}

pub const fn update_ble_indication_until_millis(
    current_until_millis: u64,
    elapsed_millis: u64,
    pairing_status: BleLedPairingStatus,
    trigger_window_secs: u64,
) -> u64 {
    if !(pairing_status.button_pressed || pairing_status.window_open) {
        return current_until_millis;
    }

    let trigger_until_millis =
        elapsed_millis.saturating_add(trigger_window_secs.saturating_mul(1_000));
    if trigger_until_millis > current_until_millis {
        trigger_until_millis
    } else {
        current_until_millis
    }
}

pub fn status_to_leds(flags: ErrorFlags, wifi_unready_visible: bool) -> LedState {
    let blue_led3 = if flags.intersects(ErrorFlags::SENSOR_MASK) {
        LedPattern::FastBlink
    } else if flags.contains(ErrorFlags::STORAGE)
        || flags.intersects(ErrorFlags::UPLOAD_MASK)
        || flags.contains(ErrorFlags::TIME)
    {
        LedPattern::On
    } else if flags.intersects(ErrorFlags::NETWORK_MASK) || wifi_unready_visible {
        LedPattern::SlowBlink
    } else {
        LedPattern::Off
    };

    LedState {
        red_led2: LedPattern::Heartbeat,
        blue_led3,
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
    fn no_error_and_wifi_unready_indicator_hidden() {
        assert_eq!(
            status_to_leds(ErrorFlags::NONE, false),
            LedState {
                red_led2: LedPattern::Heartbeat,
                blue_led3: LedPattern::Off,
            }
        );
    }

    #[test]
    fn no_error_and_wifi_unready_indicator_visible() {
        assert_eq!(
            status_to_leds(ErrorFlags::NONE, true).blue_led3,
            LedPattern::SlowBlink
        );
    }

    #[test]
    fn explicit_network_error_slow_blinks_even_without_wifi_unready_indicator() {
        assert_eq!(
            status_to_leds(ErrorFlags::WIFI, false).blue_led3,
            LedPattern::SlowBlink
        );
    }

    #[test]
    fn sensor_error_has_fast_blink() {
        assert_eq!(
            status_to_leds(ErrorFlags::OPT3001, true).blue_led3,
            LedPattern::FastBlink
        );
    }

    #[test]
    fn upload_error_is_solid_on() {
        assert_eq!(
            status_to_leds(ErrorFlags::UPLOAD, true).blue_led3,
            LedPattern::On
        );
    }

    #[test]
    fn storage_error_is_solid_on() {
        assert_eq!(
            status_to_leds(ErrorFlags::STORAGE, true).blue_led3,
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
            .blue_led3,
            LedPattern::FastBlink
        );
    }

    #[test]
    fn ble_pairing_window_fast_blinks_even_outside_indication_window() {
        assert_eq!(
            ble_status_to_led(
                BleLedRuntimeState::Advertising,
                BleLedPairingStatus {
                    window_open: true,
                    button_pressed: false,
                },
                false,
            ),
            Some(LedPattern::FastBlink)
        );
    }

    #[test]
    fn ble_advertising_slow_blinks_only_inside_indication_window() {
        let pairing = BleLedPairingStatus::default();
        assert_eq!(
            ble_status_to_led(BleLedRuntimeState::Advertising, pairing, true),
            Some(LedPattern::SlowBlink)
        );
        assert_eq!(
            ble_status_to_led(BleLedRuntimeState::Advertising, pairing, false),
            None
        );
    }

    #[test]
    fn ble_idle_overrides_blue_status_as_off_inside_indication_window() {
        assert_eq!(
            ble_status_to_led(
                BleLedRuntimeState::Idle,
                BleLedPairingStatus::default(),
                true,
            ),
            Some(LedPattern::Off)
        );
    }

    #[test]
    fn ble_boot_indication_window_depends_on_feature_enablement() {
        assert_eq!(initial_ble_indication_until_millis(true, 180), 180_000);
        assert_eq!(initial_ble_indication_until_millis(false, 180), 0);
    }

    #[test]
    fn ble_trigger_extends_indication_window_without_shortening_it() {
        let status = BleLedPairingStatus {
            window_open: false,
            button_pressed: true,
        };

        assert_eq!(
            update_ble_indication_until_millis(1_000, 5_000, status, 10),
            15_000
        );
        assert_eq!(
            update_ble_indication_until_millis(20_000, 5_000, status, 10),
            20_000
        );
        assert_eq!(
            update_ble_indication_until_millis(20_000, 5_000, BleLedPairingStatus::default(), 10,),
            20_000
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
