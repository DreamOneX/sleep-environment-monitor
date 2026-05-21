#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WifiState {
    Init,
    Connecting,
    Connected,
    Backoff { attempt: u8 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WifiEvent {
    Start,
    ConnectOk,
    ConnectFailed,
    Disconnected,
    RetryTimerExpired,
}

pub fn next_wifi_state(state: WifiState, event: WifiEvent) -> WifiState {
    match (state, event) {
        (WifiState::Init, WifiEvent::Start) => WifiState::Connecting,
        (WifiState::Connecting, WifiEvent::ConnectOk) => WifiState::Connected,
        (WifiState::Connecting, WifiEvent::ConnectFailed)
        | (WifiState::Connected, WifiEvent::Disconnected) => WifiState::Backoff { attempt: 1 },
        (WifiState::Backoff { .. }, WifiEvent::RetryTimerExpired) => WifiState::Connecting,
        (WifiState::Backoff { attempt }, WifiEvent::ConnectFailed | WifiEvent::Disconnected) => {
            WifiState::Backoff {
                attempt: attempt.saturating_add(1),
            }
        }
        (state, _) => state,
    }
}

pub const fn backoff_seconds(attempt: u8) -> u32 {
    match attempt {
        0 | 1 => crate::config::wifi::BACKOFF_SECONDS[0],
        2 => crate::config::wifi::BACKOFF_SECONDS[1],
        3 => crate::config::wifi::BACKOFF_SECONDS[2],
        4 => crate::config::wifi::BACKOFF_SECONDS[3],
        _ => crate::config::wifi::BACKOFF_SECONDS[4],
    }
}

#[cfg(target_arch = "riscv32")]
use defmt::{info, warn};
#[cfg(target_arch = "riscv32")]
use embassy_time::{Duration, Timer};
#[cfg(target_arch = "riscv32")]
use esp_radio::wifi::{Config, WifiController, sta::StationConfig};

#[cfg(target_arch = "riscv32")]
use crate::{config, tasks::TaskSignal, types::NetworkState};

#[cfg(target_arch = "riscv32")]
#[embassy_executor::task]
pub async fn wifi_task(
    mut controller: WifiController<'static>,
    network_state: &'static TaskSignal<NetworkState>,
) {
    let mut attempt = 0_u8;

    loop {
        network_state.signal(NetworkState::Connecting);
        info!(
            "wifi connecting ssid={=str} auth={:?}",
            config::wifi::SSID,
            config::wifi::AUTH_MODE
        );

        let station_config = match config::wifi::validate_credentials(
            config::wifi::SSID,
            config::wifi::PASSWORD,
            config::wifi::AUTH_MODE,
        ) {
            Ok(()) => station_config(),
            Err(error) => {
                network_state.signal(NetworkState::Disconnected);
                warn!("wifi config invalid error={:?}", error);
                Timer::after(Duration::from_secs(backoff_seconds(1) as u64)).await;
                continue;
            }
        };

        let station_config = Config::Station(station_config);

        let connect_result = match controller.set_config(&station_config) {
            Ok(()) => controller.connect_async().await,
            Err(error) => {
                warn!("wifi set_config failed: {:?}", error);
                Err(error)
            }
        };

        match connect_result {
            Ok(info) => {
                network_state.signal(NetworkState::Connected);
                info!(
                    "wifi connected ssid={=str} channel={=u8} aid={=u16}",
                    info.ssid.as_str(),
                    info.channel,
                    info.aid,
                );

                match controller.wait_for_disconnect_async().await {
                    Ok(info) => warn!(
                        "wifi disconnected reason={:?} rssi={=i8}",
                        info.reason, info.rssi
                    ),
                    Err(error) => warn!("wifi disconnect wait ended: {:?}", error),
                }

                network_state.signal(NetworkState::Disconnected);
                attempt = 1;
            }
            Err(error) => {
                network_state.signal(NetworkState::Disconnected);
                attempt = attempt.saturating_add(1).max(1);
                warn!("wifi connect failed: {:?}", error);
            }
        }

        let delay_seconds = backoff_seconds(attempt);
        info!(
            "wifi retry backoff_seconds={=u32} attempt={=u8}",
            delay_seconds, attempt,
        );
        Timer::after(Duration::from_secs(delay_seconds as u64)).await;
    }
}

#[cfg(target_arch = "riscv32")]
fn station_config() -> StationConfig {
    let station_config = StationConfig::default()
        .with_ssid(config::wifi::SSID)
        .with_auth_method(config::wifi::authentication_method());

    if config::wifi::PASSWORD.is_empty() {
        station_config
    } else {
        station_config.with_password(config::wifi::PASSWORD.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_start_moves_to_connecting() {
        assert_eq!(
            next_wifi_state(WifiState::Init, WifiEvent::Start),
            WifiState::Connecting
        );
    }

    #[test]
    fn connecting_ok_moves_to_connected() {
        assert_eq!(
            next_wifi_state(WifiState::Connecting, WifiEvent::ConnectOk),
            WifiState::Connected
        );
    }

    #[test]
    fn connecting_failure_moves_to_backoff() {
        assert_eq!(
            next_wifi_state(WifiState::Connecting, WifiEvent::ConnectFailed),
            WifiState::Backoff { attempt: 1 }
        );
    }

    #[test]
    fn connected_disconnected_moves_to_backoff() {
        assert_eq!(
            next_wifi_state(WifiState::Connected, WifiEvent::Disconnected),
            WifiState::Backoff { attempt: 1 }
        );
    }

    #[test]
    fn backoff_retry_moves_to_connecting() {
        assert_eq!(
            next_wifi_state(
                WifiState::Backoff { attempt: 3 },
                WifiEvent::RetryTimerExpired
            ),
            WifiState::Connecting
        );
    }

    #[test]
    fn backoff_caps_at_30_seconds() {
        assert_eq!(backoff_seconds(1), 1);
        assert_eq!(backoff_seconds(2), 2);
        assert_eq!(backoff_seconds(3), 5);
        assert_eq!(backoff_seconds(4), 10);
        assert_eq!(backoff_seconds(5), 30);
        assert_eq!(backoff_seconds(u8::MAX), 30);
    }

    #[test]
    fn attempt_count_does_not_overflow() {
        assert_eq!(
            next_wifi_state(
                WifiState::Backoff { attempt: u8::MAX },
                WifiEvent::ConnectFailed
            ),
            WifiState::Backoff { attempt: u8::MAX }
        );
    }
}
