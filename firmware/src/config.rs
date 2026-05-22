pub mod runtime {
    pub const HEAP_SIZE_BYTES: usize = 66_320;
    pub const MAIN_IDLE_SLEEP_SECS: u64 = 60;
}

pub mod network {
    pub const STACK_RESOURCE_COUNT: usize = 3;

    #[cfg(target_arch = "riscv32")]
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
}

pub mod wifi {
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
    }

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
        if ssid.len() > 32 {
            return Err(WifiConfigError::SsidTooLong);
        }
        match auth_mode {
            AuthMode::Open => {
                if !password.is_empty() {
                    return Err(WifiConfigError::OpenNetworkWithPassword);
                }
            }
            AuthMode::WpaPersonal | AuthMode::Wpa2Personal | AuthMode::WpaWpa2Personal => {
                if password.len() < 8 {
                    return Err(WifiConfigError::PasswordTooShort);
                }
                if password.len() > 64 {
                    return Err(WifiConfigError::PasswordTooLong);
                }
            }
        }

        Ok(())
    }

    #[cfg(target_arch = "riscv32")]
    pub fn authentication_method() -> esp_radio::wifi::AuthenticationMethod {
        match AUTH_MODE {
            AuthMode::Open => esp_radio::wifi::AuthenticationMethod::None,
            AuthMode::WpaPersonal => esp_radio::wifi::AuthenticationMethod::Wpa,
            AuthMode::Wpa2Personal => esp_radio::wifi::AuthenticationMethod::Wpa2Personal,
            AuthMode::WpaWpa2Personal => esp_radio::wifi::AuthenticationMethod::WpaWpa2Personal,
        }
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
    fn ble_feature_selection_matches_cargo_feature() {
        assert_eq!(ble::ENABLED, cfg!(feature = "ble-upload"));
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
    fn wpa2_requires_reasonable_password_length() {
        assert_eq!(
            wifi::validate_credentials("FZU", "short", AuthMode::Wpa2Personal),
            Err(WifiConfigError::PasswordTooShort)
        );
        assert_eq!(
            wifi::validate_credentials("FZU", "12345678", AuthMode::Wpa2Personal),
            Ok(())
        );
    }
}

pub mod aggregator {
    pub const MEASUREMENT_LOG_EVERY_SAMPLES: u32 = 60;
}

pub mod led {
    pub const HEARTBEAT_ON_MILLIS: u64 = 100;
    pub const HEARTBEAT_OFF_MILLIS: u64 = 900;
    pub const STATUS_TICK_MILLIS: u64 = 100;
    pub const SLOW_BLINK_TICKS: u32 = 5;
    pub const FAST_BLINK_TICKS: u32 = 2;
    pub const HEARTBEAT_TICKS: u32 = 10;
}
