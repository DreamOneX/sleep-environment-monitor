use crate::types::ErrorFlags;

pub const fn should_log_sample(sample_index: u32, period: u32, flags: ErrorFlags) -> bool {
    !flags.is_empty() || period == 0 || sample_index == 0 || sample_index.is_multiple_of(period)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_sample_is_logged() {
        assert!(should_log_sample(0, 60, ErrorFlags::NONE));
    }

    #[test]
    fn periodic_sample_is_logged() {
        assert!(should_log_sample(120, 60, ErrorFlags::NONE));
        assert!(!should_log_sample(121, 60, ErrorFlags::NONE));
    }

    #[test]
    fn error_sample_is_always_logged() {
        assert!(should_log_sample(121, 60, ErrorFlags::MIC));
    }

    #[test]
    fn zero_period_logs_every_sample() {
        assert!(should_log_sample(5, 0, ErrorFlags::NONE));
    }
}
