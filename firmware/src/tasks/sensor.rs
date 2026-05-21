#[cfg(target_arch = "riscv32")]
use defmt::{info, warn};
#[cfg(target_arch = "riscv32")]
use embassy_time::{Duration, Instant, Timer};
#[cfg(target_arch = "riscv32")]
use esp_hal::{Blocking, i2c::master::I2c};

#[cfg(target_arch = "riscv32")]
use crate::{
    board::{I2C_ADDR_OPT3001, I2C_ADDR_SHT40},
    config,
    drivers::{opt3001, sht40},
    tasks::SampleSignal,
    types::{EnvSample, ErrorFlags},
    util::logging::should_log_sample,
};

#[cfg(target_arch = "riscv32")]
#[embassy_executor::task]
pub async fn sensor_task(
    mut i2c: I2c<'static, Blocking>,
    samples: &'static SampleSignal<EnvSample>,
) {
    match opt3001::configure_continuous(&mut i2c, I2C_ADDR_OPT3001) {
        Ok(()) => info!("OPT3001 configured at address 0x45"),
        Err(_) => warn!("OPT3001 configuration failed at address 0x45"),
    }

    let mut sample_count = 0_u32;

    loop {
        let sample = read_env_sample(&mut i2c).await;

        if should_log_sample(
            sample_count,
            config::sensor::LOG_EVERY_SAMPLES,
            sample.error_flags,
        ) {
            info!(
                "env sample uptime_ms={} temp_c={=f32} rh_percent={=f32} lux={=f32} error_flags={=u32}",
                sample.uptime_ms,
                sample.temperature_c.unwrap_or(f32::NAN),
                sample.humidity_percent.unwrap_or(f32::NAN),
                sample.lux.unwrap_or(f32::NAN),
                sample.error_flags.bits(),
            );
        }
        samples.signal(sample);

        sample_count = sample_count.wrapping_add(1);
        Timer::after(Duration::from_secs(config::sensor::SAMPLE_PERIOD_SECS)).await;
    }
}

#[cfg(target_arch = "riscv32")]
async fn read_env_sample(i2c: &mut I2c<'static, Blocking>) -> EnvSample {
    let mut sample = EnvSample {
        uptime_ms: Instant::now().as_millis(),
        ..EnvSample::default()
    };

    match read_sht40(i2c).await {
        Ok((temperature_c, humidity_percent)) => {
            sample.temperature_c = Some(temperature_c);
            sample.humidity_percent = Some(humidity_percent);
        }
        Err(_) => {
            sample.error_flags.insert(ErrorFlags::SHT40);
            warn!("SHT40 read failed");
        }
    }

    match opt3001::read_lux(i2c, I2C_ADDR_OPT3001) {
        Ok(lux) => sample.lux = Some(lux),
        Err(_) => {
            sample.error_flags.insert(ErrorFlags::OPT3001);
            warn!("OPT3001 read failed");
        }
    }

    sample
}

#[cfg(target_arch = "riscv32")]
async fn read_sht40(i2c: &mut I2c<'static, Blocking>) -> Result<(f32, f32), sht40::Sht40Error> {
    sht40::start_measurement(i2c, I2C_ADDR_SHT40)?;
    Timer::after(Duration::from_millis(
        config::sensor::SHT40_MEASUREMENT_WAIT_MILLIS,
    ))
    .await;
    sht40::read_measurement(i2c, I2C_ADDR_SHT40)
}
