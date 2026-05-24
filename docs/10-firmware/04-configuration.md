# Firmware Configuration

This document defines the firmware configuration boundary for `firmware/src/config.rs`.

Phase 21 was behavior-preserving. Phase 22 extended this module with the JSON REST API paths, discovery/time settings, and common Wi-Fi credential modes. Phase 24 adds independent BLE enablement, BLE upload policy, and the BLE authorization metadata startup policy.

## Config Owns

`config.rs` should own values that are deployment choices, tuning knobs, or future runtime configuration targets:

| Category | Examples |
|---|---|
| Wi-Fi | SSID, authentication mode, credential defaults, byte-length validation, 64-byte hex PSK validation |
| BLE | BLE feature enablement, advertising name, pairing-window timing, GATT transfer sizing, ACK policy tuning, authorization metadata version, auto-pair-on-auth-reset policy |
| REST upload | fallback host/IP, port, JSON upload path, time path, discovery path, user-agent |
| Network timing | upload retry delay, TCP/read timeouts, discovery retry, time-sync retry, empty-spool poll interval |
| Network resources | stack resource count, socket buffers, request/response buffers |
| Sampling policy | environment sample period, microphone window size, microphone sample interval |
| Hardware tuning | I2C bus frequency, ADC attenuation, ADC retry count/delay |
| Storage tuning | measurement payload size, persistent spool record capacity, request channel capacity |
| Logging and status | sample log intervals, storage metrics interval, LED heartbeat, boot flash, status blink cadence, Wi-Fi-unready status hint window, and BLE indication timing |

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

## Current Wi-Fi Credential Rules

`config::wifi` keeps the current bring-up default as SSID `FZU`, empty
password, and open authentication. That is a local development default, not a
deployment credential model.

Credential validation uses IEEE byte limits rather than character counts:

- SSIDs must be non-empty and at most 32 bytes.
- Open networks must not provide a password.
- WPA/WPA2 personal passwords must be 8 to 64 bytes.
- A 64-byte WPA/WPA2 personal password is treated as a raw PSK and must be
  hexadecimal.

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

## Phase 24 BLE Configuration

The Phase 24 BLE upload path keeps configuration ownership explicit:

- `wifi-upload` is the default firmware feature and keeps the current REST
  upload behavior enabled unless the build explicitly uses
  `--no-default-features`.
- `ble-upload` enables the BLE upload boundary independently from Wi-Fi.
- `radio-coex` is the explicit BLE+Wi-Fi coexistence feature; it selects both
  `wifi-upload` and `ble-upload` plus `esp-radio/coex`.
- BLE upload remains disabled by default unless the build enables
  `ble-upload`.
- BLE advertising and pairing-window settings live in config without being
  embedded inside
  upload or storage task logic.
- Keep BOOT / IO9 pairing-window timing in config and keep the pin input-only
  with the MCU internal pull-up explicitly enabled at runtime. Phase 24R also
  keeps the runtime BLE
  auth-record clear hold duration in config: about 8 seconds of BOOT / IO9 hold
  after firmware boot clears saved BLE authorization records.
- Keep LED timing and display-window knobs in config: red LED2 boot/reset fast
  flash, red LED2 heartbeat, blue LED3 normal status blink cadence, the
  optional blue LED3 Wi-Fi-unready hint window, the blue LED3 180 second
  post-boot BLE status window, and the blue LED3 10 second BOOT /
  IO9-triggered BLE status window. Config owns durations and cadences here;
  the LED state priority policy remains in
  [00-architecture.md](00-architecture.md#4-firmware-led-semantics) and
  `util::status`.
- `config::led::WIFI_UNREADY_STATUS_WINDOW_SECS` controls only the plain
  Wi-Fi/IP-not-ready hint. The default `0` disables this slow-blink hint so a
  board with local storage does not report a permanent visible problem just
  because Wi-Fi is absent. Explicit Wi-Fi/IP/discovery error flags still use
  the normal LED3 slow-blink network fault path.
- Keep the BLE authorization record-set version, record-set compatibility
  checksum, auth record capacity, security seed length, and
  auto-pair-on-auth-record-reset switch in config. A version mismatch,
  compatibility-checksum mismatch, missing/invalid header, invalid records, or
  empty record set may open the temporary authorization window on boot when the
  switch is enabled. The runtime clear gesture erases the same BLE auth sector
  and relies on that startup policy after the next boot.
- Keep GATT fragment-size policy values in config. BLE transfer timeout policy
  values are not currently implemented as config entries.
- Keep project GATT protocol constants in the BLE protocol module, not in
  `config.rs`.
- Preserve the rule that BLE is a structured low-power upload channel, not a
  serial-port emulation.
