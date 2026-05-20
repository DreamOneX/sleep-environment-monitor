use crate::types::{EnvSample, Measurement, MicSample};

#[cfg(target_arch = "riscv32")]
const MEASUREMENT_LOG_EVERY_SAMPLES: u32 = 60;

pub fn merge_measurement(env: EnvSample, mic: MicSample) -> Measurement {
    Measurement {
        uptime_ms: env.uptime_ms.max(mic.uptime_ms),
        temperature_c: env.temperature_c,
        humidity_percent: env.humidity_percent,
        lux: env.lux,
        mic_mean: mic.mean,
        mic_rms: mic.rms,
        mic_peak: mic.peak,
        mic_db_rel: mic.db_rel,
        mic_clip_count: mic.clip_count,
        error_flags: env.error_flags | mic.error_flags,
    }
}

#[cfg(target_arch = "riscv32")]
use super::{MeasurementQueue, SampleSignal, TaskSignal, upload::measurement_to_csv_line};
#[cfg(target_arch = "riscv32")]
use crate::{types::ErrorFlags, util::logging::should_log_sample};
#[cfg(target_arch = "riscv32")]
use defmt::{info, warn};

#[cfg(target_arch = "riscv32")]
#[embassy_executor::task]
pub async fn aggregator_task(
    env_samples: &'static SampleSignal<EnvSample>,
    mic_samples: &'static SampleSignal<MicSample>,
    measurements: &'static MeasurementQueue,
    error_flags: &'static TaskSignal<ErrorFlags>,
) {
    let mut latest_env = env_samples.wait().await;
    let mut latest_mic = mic_samples.wait().await;
    let mut measurement_count = 0_u32;

    loop {
        if let Some(env) = env_samples.try_take() {
            latest_env = env;
        }

        let measurement = merge_measurement(latest_env, latest_mic);
        if should_log_sample(
            measurement_count,
            MEASUREMENT_LOG_EVERY_SAMPLES,
            measurement.error_flags,
        ) {
            log_measurement(&measurement);
        }
        enqueue_measurement(measurements, measurement);
        error_flags.signal(measurement.error_flags);

        measurement_count = measurement_count.wrapping_add(1);
        latest_mic = mic_samples.wait().await;
    }
}

#[cfg(target_arch = "riscv32")]
fn log_measurement(measurement: &Measurement) {
    let mut line = [0_u8; 192];

    match measurement_to_csv_line(measurement, &mut line) {
        Ok(len) => match core::str::from_utf8(&line[..len]) {
            Ok(csv) => info!("measurement csv={=str}", csv),
            Err(_) => warn!("measurement csv output was not utf-8"),
        },
        Err(_) => warn!("measurement csv buffer too small"),
    }
}

#[cfg(target_arch = "riscv32")]
fn enqueue_measurement(measurements: &MeasurementQueue, measurement: Measurement) {
    measurements.lock(|cell| {
        let mut queue = cell.borrow_mut();
        if queue.push(measurement).is_some() {
            warn!(
                "measurement queue full; dropped oldest len={=usize}",
                queue.len()
            );
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ErrorFlags;

    fn env_sample() -> EnvSample {
        EnvSample {
            uptime_ms: 100,
            temperature_c: Some(22.5),
            humidity_percent: Some(48.0),
            lux: Some(12.25),
            error_flags: ErrorFlags::SHT40,
        }
    }

    fn mic_sample() -> MicSample {
        MicSample {
            uptime_ms: 120,
            mean: 2048.0,
            rms: 12.5,
            peak: 40.0,
            db_rel: 21.9,
            clip_count: 2,
            error_flags: ErrorFlags::UPLOAD,
        }
    }

    #[test]
    fn copies_temperature_humidity_and_lux() {
        let measurement = merge_measurement(env_sample(), mic_sample());

        assert_eq!(measurement.temperature_c, Some(22.5));
        assert_eq!(measurement.humidity_percent, Some(48.0));
        assert_eq!(measurement.lux, Some(12.25));
    }

    #[test]
    fn copies_mic_fields() {
        let measurement = merge_measurement(env_sample(), mic_sample());

        assert_eq!(measurement.mic_mean, 2048.0);
        assert_eq!(measurement.mic_rms, 12.5);
        assert_eq!(measurement.mic_peak, 40.0);
        assert_eq!(measurement.mic_db_rel, 21.9);
        assert_eq!(measurement.mic_clip_count, 2);
    }

    #[test]
    fn merges_error_flags() {
        let measurement = merge_measurement(env_sample(), mic_sample());

        assert!(measurement.error_flags.contains(ErrorFlags::SHT40));
        assert!(measurement.error_flags.contains(ErrorFlags::UPLOAD));
    }

    #[test]
    fn handles_missing_sensor_fields() {
        let mut env = env_sample();
        env.temperature_c = None;
        env.lux = None;

        let measurement = merge_measurement(env, mic_sample());

        assert_eq!(measurement.temperature_c, None);
        assert_eq!(measurement.humidity_percent, Some(48.0));
        assert_eq!(measurement.lux, None);
    }

    #[test]
    fn selects_latest_timestamp() {
        let mut env = env_sample();
        let mut mic = mic_sample();
        env.uptime_ms = 200;
        mic.uptime_ms = 120;

        assert_eq!(merge_measurement(env, mic).uptime_ms, 200);

        env.uptime_ms = 100;
        mic.uptime_ms = 250;

        assert_eq!(merge_measurement(env, mic).uptime_ms, 250);
    }
}
