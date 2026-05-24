pub mod runtime {
    pub const HEAP_SIZE_BYTES: usize = 66_320;
    pub const MAIN_IDLE_SLEEP_SECS: u64 = 60;
}

pub mod network {
    pub const STACK_RESOURCE_COUNT: usize = 3;

    #[cfg(all(target_arch = "riscv32", feature = "wifi-upload"))]
    pub fn default_config() -> embassy_net::Config {
        embassy_net::Config::dhcpv4(Default::default())
    }
}

pub mod ble {
    pub const ENABLED: bool = cfg!(feature = "ble-upload");
    pub const ADVERTISING_NAME: &str = "sleep-env-esp32c3";
    pub const MAX_FRAGMENT_PAYLOAD_LEN: usize = 128;
    pub const HCI_SCRATCH_BUFFER_LEN: usize = 260;
    pub const IDLE_POLL_SECS: u64 = 5;
    pub const PAIRING_BUTTON_POLL_MILLIS: u64 = 50;
    pub const PAIRING_HOLD_MILLIS: u64 = 2_000;
    pub const PAIRING_WINDOW_SECS: u64 = 60;
    pub const AUTH_RECORD_CAPACITY: usize = 10;
    pub const AUTH_RECORDS_VERSION: u32 = 1;
    pub const AUTH_RECORDS_CHECKSUM: u32 = 0;
    pub const AUTO_PAIR_ON_AUTH_RECORD_RESET: bool = cfg!(feature = "ble-upload");
    pub const SECURITY_SEED_LEN: usize = 32;
}

pub mod wifi {
    pub const ENABLED: bool = cfg!(feature = "wifi-upload");

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[cfg_attr(target_arch = "riscv32", derive(defmt::Format))]
    pub enum AuthMode {
        Open,
        WpaPersonal,
        Wpa2Personal,
        WpaWpa2Personal,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[cfg_attr(target_arch = "riscv32", derive(defmt::Format))]
    pub enum WifiConfigError {
        EmptySsid,
        SsidTooLong,
        OpenNetworkWithPassword,
        PasswordTooLong,
        PasswordTooShort,
        PasswordHexRequired,
    }

    pub const SSID_MAX_BYTES: usize = 32;
    pub const PASSWORD_MIN_BYTES: usize = 8;
    pub const PASSWORD_MAX_BYTES: usize = 64;

    pub const SSID: &str = "FZU";
    pub const PASSWORD: &str = "";
    pub const AUTH_MODE: AuthMode = AuthMode::Open;
    pub const BACKOFF_SECONDS: [u32; 5] = [1, 2, 5, 10, 30];

    pub const fn validate_credentials(
        ssid: &str,
        password: &str,
        auth_mode: AuthMode,
    ) -> Result<(), WifiConfigError> {
        if ssid.is_empty() {
            return Err(WifiConfigError::EmptySsid);
        }
        if ssid.len() > SSID_MAX_BYTES {
            return Err(WifiConfigError::SsidTooLong);
        }
        match auth_mode {
            AuthMode::Open => {
                if !password.is_empty() {
                    return Err(WifiConfigError::OpenNetworkWithPassword);
                }
            }
            AuthMode::WpaPersonal | AuthMode::Wpa2Personal | AuthMode::WpaWpa2Personal => {
                if password.len() < PASSWORD_MIN_BYTES {
                    return Err(WifiConfigError::PasswordTooShort);
                }
                if password.len() > PASSWORD_MAX_BYTES {
                    return Err(WifiConfigError::PasswordTooLong);
                }
                if password.len() == PASSWORD_MAX_BYTES && !is_hex_password(password) {
                    return Err(WifiConfigError::PasswordHexRequired);
                }
            }
        }

        Ok(())
    }

    pub const fn is_hex_password(password: &str) -> bool {
        let bytes = password.as_bytes();
        let mut index = 0;
        while index < bytes.len() {
            let byte = bytes[index];
            if !byte.is_ascii_hexdigit() {
                return false;
            }
            index += 1;
        }

        true
    }
}

pub mod upload {
    pub const DEVICE_ID: &str = "sleep-env-esp32c3";
    pub const SCHEMA_VERSION: u8 = 1;
    pub const FALLBACK_HOST_HEADER: &str = "10.133.56.218:8080";
    pub const FALLBACK_IPV4_OCTETS: [u8; 4] = [10, 133, 56, 218];
    pub const FALLBACK_PORT: u16 = 8080;
    pub const MEASUREMENT_PATH: &str = "/api/v1/measurements";
    pub const TIME_PATH: &str = "/api/v1/time";
    pub const DISCOVERY_PATH: &str = "/.well-known/sleep-environment-monitor";
    pub const USER_AGENT: &str = "sleep-environment-monitor/0.1";
    pub const RETRY_DELAY_SECS: u64 = 2;
    pub const EMPTY_SPOOL_POLL_MILLIS: u64 = 250;
    pub const DISCOVERY_RETRY_SECS: u64 = 30;
    pub const TIME_SYNC_RETRY_SECS: u64 = 60;
    pub const SOCKET_TIMEOUT_SECS: u64 = 10;
    pub const READ_TIMEOUT_SECS: u64 = 10;
    pub const RX_BUFFER_SIZE: usize = 512;
    pub const TX_BUFFER_SIZE: usize = 512;
    pub const REQUEST_BUFFER_SIZE: usize = 1024;
    pub const RESPONSE_BUFFER_SIZE: usize = 512;
    pub const UDP_BUFFER_SIZE: usize = 256;
    pub const UDP_DISCOVERY_PORT: u16 = 39022;
    pub const UDP_DISCOVERY_QUERY: &str = "sleep-environment-monitor.discovery";
    pub const NTP_PORT: u16 = 123;
    pub const NTP_SERVER_IPV4_OCTETS: [u8; 4] = [129, 6, 15, 28];
    pub const NTP_UNIX_EPOCH_DELTA_SECS: u64 = 2_208_988_800;
    pub const SUCCESS_LOG_EVERY: u32 = 60;
}

pub mod sensor {
    pub const I2C_FREQUENCY_KHZ: u32 = 100;
    pub const SAMPLE_PERIOD_SECS: u64 = 2;
    pub const SHT40_MEASUREMENT_WAIT_MILLIS: u64 = 10;
    pub const LOG_EVERY_SAMPLES: u32 = 30;
}

pub mod mic {
    pub const SAMPLE_COUNT: usize = 1_000;
    pub const SAMPLE_INTERVAL_MILLIS: u64 = 1;
    pub const READ_MAX_RETRIES: u8 = 8;
    pub const READ_RETRY_DELAY_MICROS: u64 = 100;
    pub const LOG_EVERY_WINDOWS: u32 = 60;

    #[cfg(target_arch = "riscv32")]
    pub fn adc_attenuation() -> esp_hal::analog::adc::Attenuation {
        esp_hal::analog::adc::Attenuation::_11dB
    }
}

pub mod storage {
    pub const MEASUREMENT_PAYLOAD_SIZE: usize = 384;
    pub const PERSISTENT_SPOOL_CAPACITY: usize = 32;
    pub const REQUEST_CAPACITY: usize = 8;
    pub const METRICS_LOG_EVERY_EVENTS: u32 = 16;
}

#[cfg(test)]
mod tests {
    use super::{
        ble,
        wifi::{self, AuthMode, WifiConfigError},
    };

    #[test]
    fn radio_feature_selection_matches_cargo_features() {
        assert_eq!(ble::ENABLED, cfg!(feature = "ble-upload"));
        assert_eq!(wifi::ENABLED, cfg!(feature = "wifi-upload"));
        assert_eq!(ble::ADVERTISING_NAME, "sleep-env-esp32c3");
    }

    #[test]
    fn open_network_allows_empty_password() {
        assert_eq!(
            wifi::validate_credentials("FZU", "", AuthMode::Open),
            Ok(())
        );
    }

    #[test]
    fn open_network_rejects_password() {
        assert_eq!(
            wifi::validate_credentials("FZU", "password", AuthMode::Open),
            Err(WifiConfigError::OpenNetworkWithPassword)
        );
    }

    #[test]
    fn wifi_rejects_bad_ssid_lengths() {
        assert_eq!(
            wifi::validate_credentials("", "", AuthMode::Open),
            Err(WifiConfigError::EmptySsid)
        );
        assert_eq!(
            wifi::validate_credentials("123456789012345678901234567890123", "", AuthMode::Open),
            Err(WifiConfigError::SsidTooLong)
        );
    }

    #[test]
    fn wpa_modes_require_password_byte_bounds() {
        assert_eq!(
            wifi::validate_credentials("FZU", "short", AuthMode::Wpa2Personal),
            Err(WifiConfigError::PasswordTooShort)
        );
        assert_eq!(
            wifi::validate_credentials(
                "FZU",
                "12345678901234567890123456789012345678901234567890123456789012345",
                AuthMode::Wpa2Personal
            ),
            Err(WifiConfigError::PasswordTooLong)
        );
        assert_eq!(
            wifi::validate_credentials("FZU", "12345678", AuthMode::Wpa2Personal),
            Ok(())
        );
        assert_eq!(
            wifi::validate_credentials("FZU", "12345678", AuthMode::WpaPersonal),
            Ok(())
        );
        assert_eq!(
            wifi::validate_credentials("FZU", "12345678", AuthMode::WpaWpa2Personal),
            Ok(())
        );
    }

    #[test]
    fn sixty_four_byte_password_must_be_hex_psk() {
        assert_eq!(
            wifi::validate_credentials(
                "FZU",
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaZ",
                AuthMode::Wpa2Personal
            ),
            Err(WifiConfigError::PasswordHexRequired)
        );
        assert_eq!(
            wifi::validate_credentials(
                "FZU",
                "0123456789abcdef0123456789abcdef0123456789ABCDEF0123456789ABCDEF",
                AuthMode::Wpa2Personal
            ),
            Ok(())
        );
    }
}

pub mod aggregator {
    pub const MEASUREMENT_LOG_EVERY_SAMPLES: u32 = 60;
}

pub mod led {
    pub const BOOT_FLASH_CYCLES: u8 = 15;
    pub const BOOT_FLASH_ON_MILLIS: u64 = 100;
    pub const BOOT_FLASH_OFF_MILLIS: u64 = 100;
    pub const BLE_BOOT_STATUS_WINDOW_SECS: u64 = 180;
    pub const BLE_TRIGGER_STATUS_WINDOW_SECS: u64 = 10;
    pub const HEARTBEAT_ON_MILLIS: u64 = 100;
    pub const HEARTBEAT_OFF_MILLIS: u64 = 900;
    pub const STATUS_TICK_MILLIS: u64 = 100;
    pub const SLOW_BLINK_TICKS: u32 = 5;
    pub const FAST_BLINK_TICKS: u32 = 2;
    pub const HEARTBEAT_TICKS: u32 = 10;
}
