pub const ADC_CLIP_MAX: u16 = 4095;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MicStats {
    pub mean: f32,
    pub rms: f32,
    pub peak: f32,
    pub db_rel: f32,
    pub clip_count: u32,
}

impl Default for MicStats {
    fn default() -> Self {
        Self {
            mean: 0.0,
            rms: 0.0,
            peak: 0.0,
            db_rel: 0.0,
            clip_count: 0,
        }
    }
}

pub fn analyze_adc_samples(samples: &[u16]) -> MicStats {
    if samples.is_empty() {
        return MicStats::default();
    }

    let len = samples.len() as f32;
    let sum = samples.iter().fold(0.0, |acc, sample| acc + *sample as f32);
    let mean = sum / len;
    let mut sum_squares = 0.0;
    let mut peak = 0.0_f32;
    let mut clip_count = 0;

    for sample in samples {
        let value = *sample as f32;
        let centered = value - mean;
        let abs_centered = centered.abs();

        sum_squares += centered * centered;
        peak = peak.max(abs_centered);

        if *sample == 0 || *sample >= ADC_CLIP_MAX {
            clip_count += 1;
        }
    }

    let rms = sqrt_f32(sum_squares / len);
    let db_rel = if rms > 0.0 {
        20.0 * log10_f32(rms)
    } else {
        0.0
    };

    MicStats {
        mean,
        rms,
        peak,
        db_rel,
        clip_count,
    }
}

fn sqrt_f32(value: f32) -> f32 {
    if value <= 0.0 {
        return 0.0;
    }

    let mut x = value.max(1.0);
    for _ in 0..16 {
        x = 0.5 * (x + value / x);
    }

    x
}

fn log10_f32(value: f32) -> f32 {
    if value <= 0.0 {
        return 0.0;
    }

    let mut normalized = value;
    let mut exponent = 0.0_f32;

    while normalized >= 10.0 {
        normalized *= 0.1;
        exponent += 1.0;
    }

    while normalized < 1.0 {
        normalized *= 10.0;
        exponent -= 1.0;
    }

    // Mercator-series transform: ln(x) = 2 * (z + z^3/3 + z^5/5 + ...),
    // z = (x - 1) / (x + 1). Converges well for normalized x in [1, 10).
    let z = (normalized - 1.0) / (normalized + 1.0);
    let z2 = z * z;
    let mut term = z;
    let mut ln = 0.0;
    let mut divisor = 1.0;

    for _ in 0..12 {
        ln += term / divisor;
        term *= z2;
        divisor += 2.0;
    }

    exponent + (2.0 * ln) / core::f32::consts::LN_10
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(actual: f32, expected: f32, epsilon: f32) {
        assert!(
            (actual - expected).abs() <= epsilon,
            "actual={actual}, expected={expected}, epsilon={epsilon}"
        );
    }

    #[test]
    fn constant_samples_produce_zero_rms() {
        let stats = analyze_adc_samples(&[2048, 2048, 2048, 2048]);

        approx_eq(stats.mean, 2048.0, 0.001);
        approx_eq(stats.rms, 0.0, 0.001);
        approx_eq(stats.peak, 0.0, 0.001);
        approx_eq(stats.db_rel, 0.0, 0.001);
    }

    #[test]
    fn symmetric_samples_produce_expected_mean_rms_and_peak() {
        let stats = analyze_adc_samples(&[1000, 3000]);

        approx_eq(stats.mean, 2000.0, 0.001);
        approx_eq(stats.rms, 1000.0, 0.001);
        approx_eq(stats.peak, 1000.0, 0.001);
    }

    #[test]
    fn clipped_samples_increment_count() {
        let stats = analyze_adc_samples(&[0, 1, ADC_CLIP_MAX - 1, ADC_CLIP_MAX, 5000]);

        assert_eq!(stats.clip_count, 3);
    }

    #[test]
    fn empty_input_does_not_panic() {
        assert_eq!(analyze_adc_samples(&[]), MicStats::default());
    }

    #[test]
    fn db_rel_never_becomes_nan() {
        assert!(!analyze_adc_samples(&[]).db_rel.is_nan());
        assert!(!analyze_adc_samples(&[2048, 2048]).db_rel.is_nan());
        assert!(!analyze_adc_samples(&[1000, 3000]).db_rel.is_nan());
    }
}
