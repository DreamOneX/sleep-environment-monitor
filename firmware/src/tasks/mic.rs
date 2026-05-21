#[cfg(target_arch = "riscv32")]
use defmt::{info, warn};
#[cfg(target_arch = "riscv32")]
use embassy_time::{Duration, Instant, Timer};
#[cfg(target_arch = "riscv32")]
use esp_hal::{
    Blocking,
    analog::adc::{Adc, AdcPin},
    peripherals::{ADC1, GPIO3},
};

#[cfg(target_arch = "riscv32")]
use crate::{
    config,
    drivers::mic::analyze_adc_samples,
    tasks::SampleSignal,
    types::{ErrorFlags, MicSample},
    util::logging::should_log_sample,
};

#[cfg(target_arch = "riscv32")]
#[embassy_executor::task]
pub async fn mic_task(
    mut adc: Adc<'static, ADC1<'static>, Blocking>,
    mut pin: AdcPin<GPIO3<'static>, ADC1<'static>>,
    samples: &'static SampleSignal<MicSample>,
) {
    let mut window = [0_u16; config::mic::SAMPLE_COUNT];
    let mut window_count = 0_u32;

    loop {
        let mut read_error_count = 0_u32;

        for sample in &mut window {
            match read_sample(&mut adc, &mut pin).await {
                Ok(value) => *sample = value,
                Err(()) => {
                    *sample = 0;
                    read_error_count = read_error_count.saturating_add(1);
                }
            }
            Timer::after(Duration::from_millis(config::mic::SAMPLE_INTERVAL_MILLIS)).await;
        }

        let sample = analyze_window(&window, read_error_count > 0);

        if read_error_count > 0 {
            warn!("mic adc read failures count={=u32}", read_error_count);
        }
        if should_log_sample(
            window_count,
            config::mic::LOG_EVERY_WINDOWS,
            sample.error_flags,
        ) {
            info!(
                "mic sample uptime_ms={} mean={=f32} rms={=f32} peak={=f32} db_rel={=f32} clip_count={=u32} error_flags={=u32}",
                sample.uptime_ms,
                sample.mean,
                sample.rms,
                sample.peak,
                sample.db_rel,
                sample.clip_count,
                sample.error_flags.bits(),
            );
        }
        samples.signal(sample);

        window_count = window_count.wrapping_add(1);
    }
}

#[cfg(target_arch = "riscv32")]
async fn read_sample(
    adc: &mut Adc<'static, ADC1<'static>, Blocking>,
    pin: &mut AdcPin<GPIO3<'static>, ADC1<'static>>,
) -> Result<u16, ()> {
    for _ in 0..config::mic::READ_MAX_RETRIES {
        match adc.read_oneshot(pin) {
            Ok(value) => return Ok(value),
            Err(_) => {
                Timer::after(Duration::from_micros(config::mic::READ_RETRY_DELAY_MICROS)).await
            }
        }
    }

    Err(())
}

#[cfg(target_arch = "riscv32")]
fn analyze_window(samples: &[u16], had_read_error: bool) -> MicSample {
    let stats = analyze_adc_samples(samples);
    let mut sample = MicSample {
        uptime_ms: Instant::now().as_millis(),
        mean: stats.mean,
        rms: stats.rms,
        peak: stats.peak,
        db_rel: stats.db_rel,
        clip_count: stats.clip_count,
        error_flags: ErrorFlags::NONE,
    };

    if had_read_error
        || sample.mean <= 0.0
        || sample.mean >= crate::drivers::mic::ADC_CLIP_MAX as f32
    {
        sample.error_flags.insert(ErrorFlags::MIC);
    }

    sample
}
