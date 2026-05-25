pub fn parse_sntp_unix_ms(packet: &[u8]) -> Result<u64, ParseError> {
    if packet.len() < 48 {
        return Err(ParseError::InvalidField);
    }

    let mode = packet[0] & 0b0000_0111;
    if mode != 4 && mode != 5 {
        return Err(ParseError::InvalidField);
    }

    let seconds = u32::from_be_bytes([packet[40], packet[41], packet[42], packet[43]]) as u64;
    if seconds < config::upload::NTP_UNIX_EPOCH_DELTA_SECS {
        return Err(ParseError::InvalidField);
    }
    let fraction = u32::from_be_bytes([packet[44], packet[45], packet[46], packet[47]]) as u64;
    let unix_secs = seconds - config::upload::NTP_UNIX_EPOCH_DELTA_SECS;
    let millis = ((fraction as u128) * 1000_u128 / (1_u128 << 32)) as u64;
    unix_secs
        .checked_mul(1000)
        .and_then(|base| base.checked_add(millis))
        .ok_or(ParseError::InvalidField)
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TimeSyncState {
    pub boot_ms_at_sync: u64,
    pub unix_ms_at_sync: u64,
}

impl TimeSyncState {
    pub const fn new(boot_ms_at_sync: u64, unix_ms_at_sync: u64) -> Self {
        Self {
            boot_ms_at_sync,
            unix_ms_at_sync,
        }
    }

    pub fn wall_clock_for_uptime(self, uptime_ms: u64) -> Option<u64> {
        if uptime_ms >= self.boot_ms_at_sync {
            self.unix_ms_at_sync
                .checked_add(uptime_ms - self.boot_ms_at_sync)
        } else {
            self.unix_ms_at_sync
                .checked_sub(self.boot_ms_at_sync - uptime_ms)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimestampSelection {
    pub status: TimeStatus,
    pub wall_clock_unix_ms: Option<u64>,
}

pub fn select_timestamp(sync: Option<TimeSyncState>, uptime_ms: u64) -> TimestampSelection {
    match sync.and_then(|sync| sync.wall_clock_for_uptime(uptime_ms)) {
        Some(unix_ms) => TimestampSelection {
            status: TimeStatus::WallClockSynced,
            wall_clock_unix_ms: Some(unix_ms),
        },
        None => TimestampSelection {
            status: TimeStatus::UptimeOnly,
            wall_clock_unix_ms: None,
        },
    }
}

#[cfg(any(test, all(target_arch = "riscv32", feature = "wifi-upload")))]
fn select_payload_timestamp(
    current_boot: bool,
    sync: Option<TimeSyncState>,
    uptime_ms: u64,
) -> TimestampSelection {
    let sync = if current_boot { sync } else { None };
    select_timestamp(sync, uptime_ms)
}
