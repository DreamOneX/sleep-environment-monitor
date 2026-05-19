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
        0 | 1 => 1,
        2 => 2,
        3 => 5,
        4 => 10,
        _ => 30,
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
