#[cfg(target_arch = "riscv32")]
use embassy_time::{Duration, Timer};

#[cfg(target_arch = "riscv32")]
use esp_hal::gpio::Output;

#[cfg(target_arch = "riscv32")]
use crate::{
    tasks::TaskSignal,
    types::{ErrorFlags, NetworkState, UploadResult},
    util::status::{LedPattern, status_error_flags, status_to_leds},
};

#[cfg(target_arch = "riscv32")]
#[embassy_executor::task]
pub async fn heartbeat_task(mut led: Output<'static>) {
    loop {
        led.set_low();
        Timer::after(Duration::from_millis(100)).await;

        led.set_high();
        Timer::after(Duration::from_millis(900)).await;
    }
}

#[cfg(target_arch = "riscv32")]
#[embassy_executor::task]
pub async fn status_task(
    mut led: Output<'static>,
    error_flags: &'static TaskSignal<ErrorFlags>,
    network_state: &'static TaskSignal<NetworkState>,
    upload_result: &'static TaskSignal<UploadResult>,
) {
    let mut latest_flags = ErrorFlags::NONE;
    let mut latest_network = NetworkState::Disconnected;
    let mut latest_upload = UploadResult::Idle;
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

        let display_flags = status_error_flags(latest_flags, latest_upload);
        let leds = status_to_leds(display_flags, latest_network == NetworkState::Connected);
        drive_active_low_led(&mut led, leds.led2, tick);

        tick = tick.wrapping_add(1);
        Timer::after(Duration::from_millis(100)).await;
    }
}

#[cfg(target_arch = "riscv32")]
fn drive_active_low_led(led: &mut Output<'static>, pattern: LedPattern, tick: u32) {
    let on = match pattern {
        LedPattern::Off => false,
        LedPattern::On => true,
        LedPattern::SlowBlink => (tick / 5) % 2 == 0,
        LedPattern::FastBlink => tick % 2 == 0,
        LedPattern::Heartbeat => tick % 10 == 0,
    };

    if on {
        led.set_low();
    } else {
        led.set_high();
    }
}
