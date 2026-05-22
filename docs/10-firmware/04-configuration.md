# Firmware Configuration

This document defines the firmware configuration boundary for `firmware/src/config.rs`.

Phase 21 was behavior-preserving. Phase 22 extends this module with the JSON REST API paths, discovery/time settings, and common Wi-Fi credential modes. Phase 24 should extend it with independent BLE enablement and BLE upload policy.

## Config Owns

`config.rs` should own values that are deployment choices, tuning knobs, or future runtime configuration targets:

| Category | Examples |
|---|---|
| Wi-Fi | SSID, authentication mode, credential defaults, credential validation |
| BLE | BLE feature enablement, advertising name, pairing-window timing, GATT transfer sizing, ACK policy tuning |
| REST upload | fallback host/IP, port, JSON upload path, time path, discovery path, user-agent |
| Network timing | upload retry delay, TCP/read timeouts, discovery retry, time-sync retry, empty-spool poll interval |
| Network resources | stack resource count, socket buffers, request/response buffers |
| Sampling policy | environment sample period, microphone window size, microphone sample interval |
| Hardware tuning | I2C bus frequency, ADC attenuation, ADC retry count/delay |
| Storage tuning | measurement payload size, persistent spool record capacity, request channel capacity |
| Logging and status | sample log intervals, storage metrics interval, LED heartbeat and blink timing |

These values are currently compile-time constants. The important boundary is
that firmware tasks depend on named configuration rather than local hardcoding,
so Wi-Fi, BLE, REST endpoint, and upload-policy settings can later become
persistent or provisioned values without reshaping task ownership.

## Config Does Not Own

Keep these values near the hardware or protocol code that defines them:

| Location | Values |
|---|---|
| `board.rs` | GPIO pin mapping, I2C addresses, flash total size, flash spool range, protected flash ranges |
| Sensor drivers | Register addresses, command bytes, CRC constants, conversion formulas |
| Microphone driver logic | ADC resolution facts such as 12-bit clip maximum |
| BLE protocol module | GATT UUIDs, binary frame field layout, protocol version, fragment integrity rules |
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

## Phase 24 BLE Checklist

When BLE upload is implemented, keep configuration ownership explicit:

- Add independent Wi-Fi and BLE enablement flags.
- Keep BLE upload disabled by default until the BLE stack and pairing behavior
  are validated on hardware.
- Add BLE advertising and pairing-window settings without embedding them inside
  upload or storage task logic.
- Keep BOOT / IO9 pairing-window timing in config and keep the pin input-only
  with no internal pull resistor.
- Add GATT fragment-size and transfer-timeout policy values.
- Keep project GATT protocol constants in the BLE protocol module, not in
  `config.rs`.
- Preserve the rule that BLE is a structured low-power upload channel, not a
  serial-port emulation.
