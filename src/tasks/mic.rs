#[cfg(target_arch = "riscv32")]
use defmt::info;
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
    drivers::mic::analyze_adc_samples,
    tasks::SampleSignal,
    types::{ErrorFlags, MicSample},
};

#[cfg(target_arch = "riscv32")]
const MIC_SAMPLE_COUNT: usize = 1000;

#[cfg(target_arch = "riscv32")]
#[embassy_executor::task]
pub async fn mic_task(
    mut adc: Adc<'static, ADC1<'static>, Blocking>,
    mut pin: AdcPin<GPIO3<'static>, ADC1<'static>>,
    samples: &'static SampleSignal<MicSample>,
) {
    let mut window = [0_u16; MIC_SAMPLE_COUNT];

    loop {
        for sample in &mut window {
            *sample = read_sample(&mut adc, &mut pin).await;
            Timer::after(Duration::from_millis(1)).await;
        }

        let sample = analyze_window(&window);

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
        samples.signal(sample);
    }
}

#[cfg(target_arch = "riscv32")]
async fn read_sample(
    adc: &mut Adc<'static, ADC1<'static>, Blocking>,
    pin: &mut AdcPin<GPIO3<'static>, ADC1<'static>>,
) -> u16 {
    loop {
        match adc.read_oneshot(pin) {
            Ok(value) => return value,
            Err(_) => Timer::after(Duration::from_micros(100)).await,
        }
    }
}

#[cfg(target_arch = "riscv32")]
fn analyze_window(samples: &[u16]) -> MicSample {
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

    if sample.mean <= 0.0 || sample.mean >= crate::drivers::mic::ADC_CLIP_MAX as f32 {
        sample.error_flags.insert(ErrorFlags::MIC);
    }

    sample
}
