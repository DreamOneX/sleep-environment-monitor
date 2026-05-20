# Walkthrough

This document records each implementation milestone, the decisions made, and the commands used to verify it.

## Milestone 1: Firmware Core Logic Foundation

Commit:

```text
test: add firmware core logic foundation
```

Scope:

- Added board constants, shared sample/measurement types, and the module tree from the development plan.
- Added hardware-independent logic for SHT40, OPT3001, microphone statistics, upload queue, LED status policy, measurement aggregation, CSV payload encoding, and Wi-Fi reconnect state transitions.
- Adjusted Cargo configuration so host unit tests can run while ESP32-C3 firmware builds remain explicit.

Verification:

```bash
cargo build
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf
```

Notes:

- This milestone intentionally avoids hardware access in unit-tested modules.
- ESP32-C3 firmware builds should use `--target riscv32imc-unknown-none-elf`.

## Milestone 2: Minimal Hardware Bring-Up

Commit:

```text
feat: bring up minimal LED heartbeat firmware
```

Scope:

- Keep `main.rs` focused on clocks, GPIO setup, Embassy runtime startup, and one LED heartbeat task.
- Add `tasks::led::heartbeat_task` for LED1.
- Use active-low LED behavior: `LOW = on`, `HIGH = off`.
- Remove early Wi-Fi initialization from the boot path so board bring-up can be validated before network work.

Expected manual check:

- Flash the firmware to the ESP32-C3 board.
- Confirm the board does not repeatedly reconnect over USB.
- Confirm LED1 pulses once per second.
- Confirm RESET still restarts the board.
- Confirm BOOT still allows download mode.

Verification:

```bash
cargo build
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf
```

## Milestone 3: I2C Sensor Bring-Up

Commit:

```text
feat: add I2C sensor bring-up task
```

Scope:

- Initialize I2C0 on the board pins:
  - SDA: GPIO4
  - SCL: GPIO5
- Add hardware I2C wrappers for:
  - SHT40 high-precision measurement command and six-byte measurement read.
  - OPT3001 continuous-mode configuration and result-register lux read.
- Spawn `sensor_task` alongside the LED heartbeat task.
- Log periodic environment samples over defmt RTT.
- Preserve host unit-test behavior by keeping pure sensor conversion tests hardware independent.

Verification:

```bash
cargo build
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf
```

Hardware validation:

- `probe-rs list` saw the board as `ESP JTAG -- 303a:1001`.
- USB/JTAG access initially failed with `failed to open device (errno 13)`.
- Reloading udev rules and re-entering the session fixed probe access.
- `cargo run --target riscv32imc-unknown-none-elf` uploaded and ran the firmware.
- RTT logs confirmed OPT3001 configuration and continuous SHT40/OPT3001 readings with `error_flags=0`.

Observed sample:

```text
[INFO ] I2C sensor bring-up initialized
[INFO ] OPT3001 configured at address 0x45
[INFO ] env sample uptime_ms=374 temp_c=25.753036 rh_percent=40.75937 lux=2.59 error_flags=0
[INFO ] env sample uptime_ms=2386 temp_c=25.739685 rh_percent=40.093693 lux=2.2 error_flags=0
```

## Milestone 4: Microphone ADC Bring-Up

Commit:

```text
feat: add microphone ADC sampling task
```

Scope:

- Initialize ADC1 on the board microphone input:
  - Microphone ADC: GPIO3 / ADC1_CH3
  - Attenuation: 11 dB
- Add `tasks::mic::mic_task` for periodic microphone sampling.
- Sample 1000 ADC values with 1 ms spacing per window.
- Reuse `drivers::mic::analyze_adc_samples` to produce `MicSample` fields:
  - mean
  - RMS
  - peak
  - relative dB
  - clip count
- Spawn the microphone task alongside LED heartbeat and I2C sensor sampling.
- Keep host-side tests hardware independent by compiling the hardware task only for `riscv32`.

Verification:

```bash
cargo build
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf
```

Hardware validation:

- `probe-rs list` saw the board as `ESP JTAG -- 303a:1001`.
- `cargo run --target riscv32imc-unknown-none-elf` initially failed inside the sandbox with `device not found (errno 2)` while opening the probe.
- Re-running the same upload command outside the sandbox opened the USB/JTAG probe, flashed the board, and streamed RTT logs.
- RTT logs confirmed:
  - Firmware starts with sensor and microphone tasks active.
  - SHT40 and OPT3001 sampling continue while the ADC task runs.
  - Microphone `mean` stays within the 12-bit ADC range.
  - `clip_count` remains zero during baseline observation.
  - `error_flags=0` for both environment and microphone samples.

Observed sample:

```text
[INFO ] sensor and microphone bring-up initialized
[INFO ] OPT3001 configured at address 0x45
[INFO ] env sample uptime_ms=373 temp_c=24.03601 rh_percent=46.872513 lux=2.1499999 error_flags=0
[INFO ] mic sample uptime_ms=1620 mean=2662.224 rms=2.905139 peak=8.224121 db_rel=9.263338 clip_count=0 error_flags=0
[INFO ] mic sample uptime_ms=2800 mean=2663.331 rms=1.7359304 peak=8.331055 db_rel=4.790646 clip_count=0 error_flags=0
```

Notes:

- The baseline microphone check was completed without human interaction.
- The manual "sound near the microphone increases RMS/peak" check still requires a person to make sound near the board while RTT logs are observed.

## Milestone 5: Local Measurement Aggregation

Commit:

```text
feat: aggregate sensor and microphone measurements
```

Scope:

- Add `embassy-sync` as a direct target dependency for hardware task signaling.
- Add a `SampleSignal<T>` alias using `CriticalSectionRawMutex`.
- Publish the latest `EnvSample` from `sensor_task`.
- Publish the latest `MicSample` from `mic_task`.
- Add `aggregator_task` to wait for both sample streams, merge the latest values with `merge_measurement`, and emit local CSV output through `measurement_to_csv_line`.
- Spawn the aggregator alongside LED, I2C sensor, and microphone tasks.

Verification:

```bash
cargo build
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf
cargo clippy --all-targets
```

Hardware validation:

- `cargo run --target riscv32imc-unknown-none-elf` uploaded and ran the firmware through the ESP32-C3 USB/JTAG probe.
- RTT logs confirmed:
  - Aggregation starts after boot.
  - Environment and microphone samples continue to be produced.
  - Complete `Measurement` CSV records are emitted continuously without Wi-Fi.
  - Latest environment values are reused between slower I2C updates and faster microphone windows.
  - `error_flags=0` during the validation run.

Observed sample:

```text
[INFO ] local measurement aggregation initialized
[INFO ] OPT3001 configured at address 0x45
[INFO ] env sample uptime_ms=318 temp_c=23.069351 rh_percent=42.266193 lux=2.36 error_flags=0
[INFO ] mic sample uptime_ms=1569 mean=2661.5 rms=6.3592453 peak=18.5 db_rel=16.067617 clip_count=0 error_flags=0
[INFO ] measurement csv=1569,23.069351,42.266193,2.36,2661.5,6.3592453,18.5,16.067617,0,0
[INFO ] measurement csv=2751,23.114746,42.28336,2.3799999,2658.731,4.996472,9.269043,13.973222,0,0
```

## Milestone 6: Wi-Fi Connection Manager

Commit:

```text
feat: add Wi-Fi connection manager
```

Scope:

- Add a hardware `wifi_task` that owns the ESP32-C3 Wi-Fi controller.
- Connect as a station to the open `FZU` network from `environment.md`.
- Publish `NetworkState` through a `TaskSignal<NetworkState>`.
- Reuse the existing Wi-Fi backoff policy after connection failures or disconnects.
- Keep Wi-Fi independent from sensor sampling and local aggregation.

Verification:

```bash
cargo build
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf
cargo clippy --all-targets
```

Hardware validation:

- `cargo run --target riscv32imc-unknown-none-elf` uploaded and ran the firmware through the ESP32-C3 USB/JTAG probe.
- RTT logs confirmed:
  - Wi-Fi station connects to `FZU`.
  - Environment and microphone sampling continue after Wi-Fi comes up.
  - Complete `Measurement` CSV records continue while Wi-Fi is connected.
  - A real disconnect event enters the one-second retry backoff.
  - The Wi-Fi task reconnects automatically.
  - Measurement output continues through disconnect, backoff, and reconnect.

Observed sample:

```text
[INFO ] local measurement aggregation and Wi-Fi manager initialized
[INFO ] wifi connecting ssid=FZU
[INFO ] OPT3001 configured at address 0x45
[INFO ] env sample uptime_ms=445 temp_c=23.987946 rh_percent=41.148468 lux=2.86 error_flags=0
[INFO ] wifi connected ssid=FZU channel=1 aid=54690
[INFO ] measurement csv=1707,23.987946,41.148468,2.86,2661.743,4.0128613,14.74292,12.069078,0,0
[WARN ] wifi disconnected reason=DisassociatedDueToInactivity rssi=-49
[INFO ] wifi retry backoff_seconds=1 attempt=1
[INFO ] measurement csv=31489,25.149536,40.74983,2.87,2662.631,3.1633568,46.631104,10.002962,0,0
[INFO ] wifi connecting ssid=FZU
[INFO ] wifi connected ssid=FZU channel=1 aid=54690
```

## Milestone 7: Measurement Upload With Offline Queue

Development phase:

```text
Phase 14: Upload Task
```

Scope:

- Add a shared `MeasurementQueue` between aggregation and upload.
- Keep `aggregator_task` responsible only for producing and enqueueing `Measurement` records.
- Add an `embassy-net` runner for the ESP32-C3 Wi-Fi station interface.
- Add `uploader_task` that waits for DHCP network configuration, posts CSV payloads to `10.133.56.218:8080/measurements`, and retries failures.
- Preserve queued data on upload failure, remove the oldest record only after a successful 2xx HTTP response, and drop the oldest record when the queue is full.
- Add a minimal `post_receiver.py` for manual POST receiver checks.

Verification:

```bash
cargo fmt --check
cargo build
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf
cargo clippy --all-targets
```

Hardware validation:

- `python3 post_receiver.py` starts a local POST receiver on `0.0.0.0:8080`.
- `cargo run --target riscv32imc-unknown-none-elf` uploaded and ran the firmware through the ESP32-C3 USB/JTAG probe after the board was manually reset and reattached through usbipd.
- RTT logs confirmed:
  - Wi-Fi station connects to `FZU`.
  - DHCP config is obtained: board address `10.133.20.144/16`, gateway `10.133.255.254`, DNS `114.114.114.114` and `210.34.48.34`.
  - Sensor and microphone sampling continue while the uploader is retrying.
  - Measurements are enqueued while upload is unavailable.
  - Queue length reaches 16 and then drops the oldest records, preserving the newest samples.
- Initial upload attempts to `10.133.56.218:8080` returned `ConnectReset` while the Windows host was not accepting inbound TCP/8080 traffic from the board.
- After the Windows inbound TCP/8080 rule was enabled, the same firmware successfully posted real `Measurement` CSV payloads from the ESP32-C3 to the local receiver.
- The uploader reported repeated success with `queue_len=0`, confirming that the queue drains after successful HTTP 2xx responses.

Observed sample:

```text
[INFO ] network ipv4 config=StaticConfigV4 { address: 10.133.20.144/16, gateway: Some(10.133.255.254), dns_servers: [114.114.114.114, 210.34.48.34] }
[INFO ] measurement csv=11685,31.961548,32.300144,9.889999,2654.151,12.112822,25.849121,21.664906,0,0
[WARN ] upload failed error=ConnectReset queue_len=9
[WARN ] measurement queue full; dropped oldest len=16
[WARN ] upload failed error=ConnectReset queue_len=16
[INFO ] network ipv4 config=StaticConfigV4 { address: 10.133.20.144/16, gateway: Some(10.133.255.254), dns_servers: [114.114.114.114, 210.34.48.34] }
[INFO ] upload success uptime_ms=1844 queue_len=0
[INFO ] upload success uptime_ms=3148 queue_len=0
10.133.20.144 /measurements 1844,32.986954,33.627678,13.98,2651.469,1.966991,13.468994,5.876047,0,0
10.133.20.144 /measurements 3148,32.981613,33.373997,14.04,2656.915,5.4669895,10.084961,14.754845,0,0
```

## Milestone 8: Runtime Hardening And Status Reporting

Development phase:

```text
Phase 15: System Hardening
```

Scope:

- Add a LED2 status task driven by the existing `status_to_leds` mapping.
- Publish latest measurement error flags from aggregation to the status task.
- Fold upload failures into status error flags so upload errors are visible through LED2.
- Document the LED2 priority table and blink timing in `architecture.md`.
- Replace firmware startup `.expect` calls with logged failures and fallback status/sample signals where the firmware can continue.
- Bound microphone ADC read retries so the microphone task cannot loop forever on repeated ADC failures.
- Reduce normal RTT log volume for environment, microphone, measurement, and upload-success logs while keeping warnings immediate.

Verification:

```bash
cargo fmt --check
cargo build
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf
cargo clippy --all-targets
```

Hardware validation:

- `probe-rs list` found the ESP32-C3 USB/JTAG probe.
- `python3 post_receiver.py` started a local POST receiver on `0.0.0.0:8080`.
- `timeout 90s cargo run --target riscv32imc-unknown-none-elf` uploaded and ran the firmware through the ESP32-C3 USB/JTAG probe.
- RTT logs confirmed:
  - The firmware booted without a reset loop or panic.
  - Wi-Fi station connected to `FZU`.
  - DHCP config was obtained: board address `10.133.20.144/16`, gateway `10.133.255.254`, DNS `114.114.114.114` and `210.34.48.34`.
  - Environment and microphone sampling still start correctly with reduced normal log volume.
  - The upload queue drained from startup backlog to normal operation.
  - Real `Measurement` CSV payloads reached the local receiver.
- The run ended because the host-side `timeout` command stopped `probe-rs`; the captured stack was the idle hook, not a firmware crash.
- Physical LED2 visual behavior and multi-hour overnight soak still require longer observation.

Observed sample:

```text
[INFO ] IPv4: DOWN
[INFO ] measurement aggregation, Wi-Fi manager, and uploader initialized
[INFO ] wifi connecting ssid=FZU
[INFO ] OPT3001 configured at address 0x45
[INFO ] env sample uptime_ms=554 temp_c=32.124435 rh_percent=32.008316 lux=4.39 error_flags=0
[INFO ] wifi connected ssid=FZU channel=1 aid=16360
[INFO ] network ipv4 config=StaticConfigV4 { address: 10.133.20.144/16, gateway: Some(10.133.255.254), dns_servers: [114.114.114.114, 210.34.48.34] }
[INFO ] mic sample uptime_ms=1791 mean=2654.089 rms=4.955911 peak=17.089111 db_rel=13.902428 clip_count=0 error_flags=0
[INFO ] measurement csv=1791,32.124435,32.008316,4.39,2654.089,4.955911,17.089111,13.902428,0,0
[INFO ] upload success uptime_ms=1791 queue_len=5
[INFO ] upload success uptime_ms=3112 queue_len=4
[INFO ] upload success uptime_ms=4311 queue_len=3
[INFO ] upload success uptime_ms=5534 queue_len=2
[INFO ] upload success uptime_ms=6734 queue_len=1
10.133.20.144 /measurements 1791,32.124435,32.008316,4.39,2654.089,4.955911,17.089111,13.902428,0,0
10.133.20.144 /measurements 3112,32.129776,32.111313,5.85,2653.121,4.0423193,14.121094,12.132611,0,0
```

## Milestone 9: Persistent SPI Flash Spool Plan

Development phase:

```text
Phase 16-20 planning: Internal SPI flash persistent spool
```

Scope:

- Update `architecture.md` to add a two-level measurement backlog: RAM hot queue plus internal SPI flash persistent spool.
- Document that the ESP32-C3-WROOM-02-N4 4 MB internal SPI flash may be used only through a dedicated spool region that must not overlap bootloader, partition table, app image, or calibration data.
- Add planned `drivers/flash.rs`, `storage/spool.rs`, and `tasks/storage.rs` responsibilities.
- Define the persistent record format with magic, version, sequence, payload length, and CRC.
- Specify FIFO upload, HTTP-2xx-only acknowledgement, cross-reset recovery, corrupt-record handling, and drop-oldest behavior when storage is full.
- Extend `development_plan.md` with Phase 16 through Phase 20 for spool pure logic, flash model tests, ESP32-C3 flash bring-up, storage task integration, and recovery/soak validation.
- Extend the final hardware-free unit test checklist for spool and flash model behavior.

Verification:

```bash
git diff --check
```

Hardware validation:

- Not required for this documentation milestone.
- Phase 18 will require ESP32-C3 hardware validation for internal SPI flash read/erase/write/readback.
- Phase 19 and Phase 20 will require manual reset or power interruption checks to prove cross-reset recovery.

## Milestone 10: Persistent Spool Record Logic

Development phase:

```text
Phase 16: Persistent Spool Design
```

Scope:

- Add `src/storage/spool.rs` and expose the new `storage` module from `src/lib.rs`.
- Define the persistent record header with magic, version, flags, header length, sequence, payload length, and CRC.
- Implement `encode_record` / `decode_record` for CSV payload bytes.
- Add CRC32 validation using the standard `123456789` check vector.
- Add `PersistentSpool` pure FIFO state with append, peek, acknowledge, sequence tracking, and drop-oldest behavior.
- Add recovery scanning for erased flash tails, bad-magic resynchronization, and partial-tail handling.
- Keep this phase hardware-independent; actual flash modeling and flash writes are deferred to Phase 17 and Phase 18.

Verification:

```bash
cargo fmt --check
cargo build
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf
cargo clippy --all-targets
```

Unit test result:

```text
76 passed
```

Hardware validation:

- Not required for this pure-logic milestone.
- No internal SPI flash erase/write/readback was attempted in this phase.

## Milestone 11: Flash Storage Model

Development phase:

```text
Phase 17: Flash Storage Model
```

Scope:

- Add `src/storage/flash_model.rs` with an `InMemoryFlash` implementation for host tests.
- Model SPI-flash constraints: erased bytes are `0xff`, writes may only clear bits, erases must be sector aligned, and out-of-range read/write/erase operations fail.
- Extend `storage/spool.rs` with `FlashBackedSpool`, data records, ack records, recovery scanning, and compaction over the flash model.
- Preserve FIFO order across ack holes in the flash log.
- Preserve monotonic `next_sequence` across compaction, ack-to-empty cases, and simulated reboot recovery.
- Add `DropOldestQueue::get()` for FIFO-order iteration without exposing queue internals.
- Keep this phase hardware-independent; no ESP32-C3 internal flash driver is used yet.

Verification:

```bash
cargo fmt --check
cargo build
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf
cargo clippy --all-targets
```

Unit test result:

```text
89 passed
```

Covered flash-spool cases:

```text
write requires erased space
erase resets a sector to 0xff
out-of-range read/write/erase fails
append across sector boundary
recover multiple records after simulated reboot
recover after interrupted append
drop oldest after modeled flash fills
ack persists across simulated reboot
ack-hole recovery preserves FIFO order
ack compaction preserves next_sequence
```

Hardware validation:

- Not required for this pure-logic milestone.
- No firmware was flashed for this milestone.
- No real internal SPI flash erase/write/readback was attempted.
- Reboot retention and blocked-upload persistent buffering are validated only against the modeled flash in this phase; real ESP32-C3 validation is deferred to Phase 18 through Phase 20.

## Milestone 12: Flash Region Safety Boundary

Development phase:

```text
Phase 18A: ESP32-C3 Flash Region Layout
```

Scope:

- Define the ESP32-C3-WROOM-02-N4 4 MiB flash geometry in `src/board.rs`.
- Reserve `0x003c_0000..0x0040_0000` as the measurement spool region.
- Keep `0x0000_0000..0x0001_0000` protected for bootloader, partition, and RF/calibration data.
- Keep `0x0001_0000..0x003c_0000` protected for the firmware image growth area.
- Add `src/drivers/flash.rs` host-testable flash layout validation.
- Reject zero-sized flash/spool regions, sector-unaligned spool ranges, out-of-bounds ranges, and protected-region overlaps.
- Update `architecture.md` with the concrete flash map and validation rule.

Verification:

```bash
cargo fmt --check
cargo build
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf
cargo clippy --all-targets
```

Unit test result:

```text
96 passed
```

Hardware validation:

- Not required for this safety-boundary milestone.
- No firmware was flashed for this milestone.
- No real internal SPI flash erase/write/readback was attempted.
- The next Phase 18 step must add the ESP32-C3 flash adapter and run the hardware smoke test before claiming real flash access works.
