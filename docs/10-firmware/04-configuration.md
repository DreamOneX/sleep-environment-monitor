# Firmware Configuration

This document defines the intended Phase 21 boundary for `firmware/src/config.rs`.

Phase 21 is behavior-preserving: move deployment and policy values into one module without changing network protocol, task behavior, or hardware validation scope.

## Config Owns

`config.rs` should own values that are deployment choices, tuning knobs, or future provisioning targets:

| Category | Examples |
|---|---|
| Wi-Fi | SSID, authentication mode, credential defaults |
| REST upload | fallback host/IP, port, path, user-agent |
| Network timing | upload retry delay, TCP/read timeouts, empty-spool poll interval |
| Network resources | stack resource count, socket buffers, request/response buffers |
| Sampling policy | environment sample period, microphone window size, microphone sample interval |
| Hardware tuning | I2C bus frequency, ADC attenuation, ADC retry count/delay |
| Storage tuning | measurement payload size, persistent spool record capacity, request channel capacity |
| Logging and status | sample log intervals, storage metrics interval, LED heartbeat and blink timing |

These values may still be compile-time constants in Phase 21. The important change is that firmware tasks depend on named configuration rather than local hardcoding.

## Config Does Not Own

Keep these values near the hardware or protocol code that defines them:

| Location | Values |
|---|---|
| `board.rs` | GPIO pin mapping, I2C addresses, flash total size, flash spool range, protected flash ranges |
| Sensor drivers | Register addresses, command bytes, CRC constants, conversion formulas |
| Microphone driver logic | ADC resolution facts such as 12-bit clip maximum |
| `storage/spool.rs` | On-flash magic, version, header layout, alignment, CRC behavior |
| Tests | Fixture literals and expected outputs |

Board facts may be referenced by config validation, but they should not be duplicated as deployment config.

## Phase 21 Checklist

Move or centralize the currently hardcoded values in these groups:

- `tasks/upload.rs`: upload endpoint, path, port, user-agent, retry delay, empty-spool poll interval, socket/read timeout, socket/request/response buffer sizes.
- `tasks/wifi.rs`: SSID, authentication method, reconnect backoff schedule.
- `bin/main.rs`: DHCP mode default, network stack resource count, heap size, I2C frequency, ADC attenuation.
- `tasks/sensor.rs`: environment sample period and SHT40 measurement wait.
- `tasks/mic.rs`: sample count, sample interval, ADC retry count, retry delay, log interval.
- `tasks/storage.rs` and `tasks/mod.rs`: payload size, persistent spool capacity, storage request channel capacity, metrics log interval.
- `tasks/aggregator.rs`, `tasks/led.rs`: log and LED timing policy.

## Verification

Phase 21 should run the standard firmware checks:

```bash
cargo fmt
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf
cargo clippy --all-targets
cargo clippy --target riscv32imc-unknown-none-elf
```

No hardware flash validation is required unless Phase 21 changes the flash range or write behavior. It should not.
