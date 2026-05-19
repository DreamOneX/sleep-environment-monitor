use core::ops::{BitOr, BitOrAssign};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ErrorFlags(u32);

impl ErrorFlags {
    pub const NONE: Self = Self(0);
    pub const SHT40: Self = Self(1 << 0);
    pub const OPT3001: Self = Self(1 << 1);
    pub const MIC: Self = Self(1 << 2);
    pub const WIFI: Self = Self(1 << 3);
    pub const UPLOAD: Self = Self(1 << 4);

    pub const SENSOR_MASK: Self = Self(Self::SHT40.0 | Self::OPT3001.0 | Self::MIC.0);

    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    pub const fn bits(self) -> u32 {
        self.0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn insert(&mut self, flags: Self) {
        self.0 |= flags.0;
    }

    pub const fn contains(self, flags: Self) -> bool {
        (self.0 & flags.0) == flags.0
    }

    pub const fn intersects(self, flags: Self) -> bool {
        (self.0 & flags.0) != 0
    }
}

impl BitOr for ErrorFlags {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for ErrorFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.insert(rhs);
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EnvSample {
    pub uptime_ms: u64,
    pub temperature_c: Option<f32>,
    pub humidity_percent: Option<f32>,
    pub lux: Option<f32>,
    pub error_flags: ErrorFlags,
}

impl Default for EnvSample {
    fn default() -> Self {
        Self {
            uptime_ms: 0,
            temperature_c: None,
            humidity_percent: None,
            lux: None,
            error_flags: ErrorFlags::NONE,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MicSample {
    pub uptime_ms: u64,
    pub mean: f32,
    pub rms: f32,
    pub peak: f32,
    pub db_rel: f32,
    pub clip_count: u32,
    pub error_flags: ErrorFlags,
}

impl Default for MicSample {
    fn default() -> Self {
        Self {
            uptime_ms: 0,
            mean: 0.0,
            rms: 0.0,
            peak: 0.0,
            db_rel: 0.0,
            clip_count: 0,
            error_flags: ErrorFlags::NONE,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measurement {
    pub uptime_ms: u64,
    pub temperature_c: Option<f32>,
    pub humidity_percent: Option<f32>,
    pub lux: Option<f32>,
    pub mic_mean: f32,
    pub mic_rms: f32,
    pub mic_peak: f32,
    pub mic_db_rel: f32,
    pub mic_clip_count: u32,
    pub error_flags: ErrorFlags,
}

impl Default for Measurement {
    fn default() -> Self {
        Self {
            uptime_ms: 0,
            temperature_c: None,
            humidity_percent: None,
            lux: None,
            mic_mean: 0.0,
            mic_rms: 0.0,
            mic_peak: 0.0,
            mic_db_rel: 0.0,
            mic_clip_count: 0,
            error_flags: ErrorFlags::NONE,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum NetworkState {
    #[default]
    Disconnected,
    Connecting,
    Connected,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum UploadResult {
    #[default]
    Idle,
    Success,
    Failed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_flags_insert_adds_bits() {
        let mut flags = ErrorFlags::NONE;

        flags.insert(ErrorFlags::SHT40);
        flags.insert(ErrorFlags::UPLOAD);

        assert_eq!(
            flags.bits(),
            ErrorFlags::SHT40.bits() | ErrorFlags::UPLOAD.bits()
        );
    }

    #[test]
    fn error_flags_contains_checks_all_requested_bits() {
        let mut flags = ErrorFlags::SHT40;
        flags.insert(ErrorFlags::UPLOAD);

        assert!(flags.contains(ErrorFlags::SHT40));
        assert!(flags.contains(ErrorFlags::SHT40 | ErrorFlags::UPLOAD));
        assert!(!flags.contains(ErrorFlags::OPT3001));
    }

    #[test]
    fn default_shared_samples_are_empty() {
        let env = EnvSample::default();
        let mic = MicSample::default();
        let measurement = Measurement::default();

        assert_eq!(env.uptime_ms, 0);
        assert_eq!(env.temperature_c, None);
        assert_eq!(env.humidity_percent, None);
        assert_eq!(env.lux, None);
        assert!(env.error_flags.is_empty());

        assert_eq!(mic.uptime_ms, 0);
        assert_eq!(mic.mean, 0.0);
        assert_eq!(mic.rms, 0.0);
        assert_eq!(mic.peak, 0.0);
        assert_eq!(mic.db_rel, 0.0);
        assert_eq!(mic.clip_count, 0);
        assert!(mic.error_flags.is_empty());

        assert_eq!(measurement.uptime_ms, 0);
        assert_eq!(measurement.temperature_c, None);
        assert_eq!(measurement.humidity_percent, None);
        assert_eq!(measurement.lux, None);
        assert_eq!(measurement.mic_mean, 0.0);
        assert_eq!(measurement.mic_rms, 0.0);
        assert_eq!(measurement.mic_peak, 0.0);
        assert_eq!(measurement.mic_db_rel, 0.0);
        assert_eq!(measurement.mic_clip_count, 0);
        assert!(measurement.error_flags.is_empty());
    }
}
