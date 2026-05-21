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

pub mod wifi {
    pub const SSID: &str = "FZU";
    pub const BACKOFF_SECONDS: [u32; 5] = [1, 2, 5, 10, 30];

    #[cfg(target_arch = "riscv32")]
    pub fn authentication_method() -> esp_radio::wifi::AuthenticationMethod {
        esp_radio::wifi::AuthenticationMethod::None
    }
}

pub mod upload {
    pub const HOST_HEADER: &str = "10.133.56.218:8080";
    pub const IPV4_OCTETS: [u8; 4] = [10, 133, 56, 218];
    pub const PORT: u16 = 8080;
    pub const PATH: &str = "/measurements";
    pub const USER_AGENT: &str = "sleep-environment-monitor/0.1";
    pub const RETRY_DELAY_SECS: u64 = 2;
    pub const EMPTY_SPOOL_POLL_MILLIS: u64 = 250;
    pub const SOCKET_TIMEOUT_SECS: u64 = 10;
    pub const READ_TIMEOUT_SECS: u64 = 10;
    pub const RX_BUFFER_SIZE: usize = 512;
    pub const TX_BUFFER_SIZE: usize = 512;
    pub const REQUEST_BUFFER_SIZE: usize = 512;
    pub const RESPONSE_BUFFER_SIZE: usize = 128;
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
    pub const MEASUREMENT_PAYLOAD_SIZE: usize = 192;
    pub const PERSISTENT_SPOOL_CAPACITY: usize = 32;
    pub const REQUEST_CAPACITY: usize = 8;
    pub const METRICS_LOG_EVERY_EVENTS: u32 = 16;
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
