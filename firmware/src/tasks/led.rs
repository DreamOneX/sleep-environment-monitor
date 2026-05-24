use crate::types::NetworkState;

pub const fn network_is_ready(state: NetworkState) -> bool {
    matches!(state, NetworkState::IpReady)
}

#[cfg(target_arch = "riscv32")]
use embassy_time::{Duration, Timer};

#[cfg(target_arch = "riscv32")]
use esp_hal::gpio::Output;

#[cfg(target_arch = "riscv32")]
use crate::{
    config,
    tasks::TaskSignal,
    types::{ErrorFlags, UploadResult},
    util::status::{
        BleLedPairingStatus, BleLedRuntimeState, LedPattern, ble_status_to_led,
        initial_ble_indication_until_millis, status_error_flags, status_to_leds,
        update_ble_indication_until_millis,
    },
};

#[cfg(target_arch = "riscv32")]
#[embassy_executor::task]
pub async fn heartbeat_task(mut led: Output<'static>) {
    for _ in 0..config::led::BOOT_FLASH_CYCLES {
        led.set_low();
        Timer::after(Duration::from_millis(config::led::BOOT_FLASH_ON_MILLIS)).await;

        led.set_high();
        Timer::after(Duration::from_millis(config::led::BOOT_FLASH_OFF_MILLIS)).await;
    }

    loop {
        led.set_low();
        Timer::after(Duration::from_millis(config::led::HEARTBEAT_ON_MILLIS)).await;

        led.set_high();
        Timer::after(Duration::from_millis(config::led::HEARTBEAT_OFF_MILLIS)).await;
    }
}

#[cfg(target_arch = "riscv32")]
#[embassy_executor::task]
pub async fn status_task(
    mut led: Output<'static>,
    error_flags: &'static TaskSignal<ErrorFlags>,
    network_state: &'static TaskSignal<NetworkState>,
    upload_result: &'static TaskSignal<UploadResult>,
    ble_runtime_state: &'static TaskSignal<BleLedRuntimeState>,
    ble_pairing_status: &'static TaskSignal<BleLedPairingStatus>,
) {
    let mut latest_flags = ErrorFlags::NONE;
    let mut latest_network = NetworkState::Disconnected;
    let mut latest_upload = UploadResult::Idle;
    let mut latest_ble_runtime = BleLedRuntimeState::Disabled;
    let mut latest_ble_pairing = BleLedPairingStatus::default();
    let mut elapsed_millis = 0_u64;
    let mut ble_indication_until_millis = initial_ble_indication_until_millis(
        config::ble::ENABLED,
        config::led::BLE_BOOT_STATUS_WINDOW_SECS,
    );
    let mut tick = 0_u32;

    loop {
        if let Some(flags) = error_flags.try_take() {
            latest_flags = flags;
        }
        if let Some(state) = network_state.try_take() {
            latest_network = state;
        }
        if let Some(result) = upload_result.try_take() {
            latest_upload = result;
        }
        if let Some(runtime_state) = ble_runtime_state.try_take() {
            latest_ble_runtime = runtime_state;
        }
        if let Some(pairing_status) = ble_pairing_status.try_take() {
            ble_indication_until_millis = update_ble_indication_until_millis(
                ble_indication_until_millis,
                elapsed_millis,
                pairing_status,
                config::led::BLE_TRIGGER_STATUS_WINDOW_SECS,
            );
            latest_ble_pairing = pairing_status;
        }

        let display_flags = status_error_flags(latest_flags, latest_upload);
        let leds = status_to_leds(display_flags, network_is_ready(latest_network));
        let ble_indication_active =
            elapsed_millis < ble_indication_until_millis || latest_ble_pairing.window_open;
        let pattern = ble_status_to_led(
            latest_ble_runtime,
            latest_ble_pairing,
            ble_indication_active,
        )
        .unwrap_or(leds.blue_led3);
        drive_active_low_led(&mut led, pattern, tick);

        tick = tick.wrapping_add(1);
        Timer::after(Duration::from_millis(config::led::STATUS_TICK_MILLIS)).await;
        elapsed_millis = elapsed_millis.saturating_add(config::led::STATUS_TICK_MILLIS);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_ip_ready_is_network_ready_for_status() {
        assert!(!network_is_ready(NetworkState::Disconnected));
        assert!(!network_is_ready(NetworkState::Connecting));
        assert!(!network_is_ready(NetworkState::Connected));
        assert!(network_is_ready(NetworkState::IpReady));
    }
}

#[cfg(target_arch = "riscv32")]
fn drive_active_low_led(led: &mut Output<'static>, pattern: LedPattern, tick: u32) {
    let on = match pattern {
        LedPattern::Off => false,
        LedPattern::On => true,
        LedPattern::SlowBlink => (tick / config::led::SLOW_BLINK_TICKS).is_multiple_of(2),
        LedPattern::FastBlink => tick.is_multiple_of(config::led::FAST_BLINK_TICKS),
        LedPattern::Heartbeat => tick.is_multiple_of(config::led::HEARTBEAT_TICKS),
    };

    if on {
        led.set_low();
    } else {
        led.set_high();
    }
}
