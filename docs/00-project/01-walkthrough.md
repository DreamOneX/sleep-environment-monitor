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
- Add `tasks::led::heartbeat_task` for LED1. Later hardware clarification: this
  name was stale; the MCU heartbeat indicator is red LED2 on IO0.
- Use active-low LED behavior: `LOW = on`, `HIGH = off`.
- Remove early Wi-Fi initialization from the boot path so board bring-up can be validated before network work.

Expected manual check:

- Flash the firmware to the ESP32-C3 board.
- Confirm the board does not repeatedly reconnect over USB.
- Confirm LED1 pulses once per second. Later hardware clarification: this is a
  historical stale expectation; LED1 is the green 3.3 V power indicator and is
  not MCU-controlled.
- Confirm RESET still restarts the board.
- Confirm BOOT still allows download mode.

Verification:

```bash
cargo build
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf
```

Later hardware clarification: this milestone's LED1 heartbeat naming is a
historical stale expectation. LED1 is the green power indicator tied to the
3.3 V rail and is not MCU-controlled; the MCU heartbeat indicator is red LED2
on IO0.

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
- Connect as a station to the open `FZU` network from `../environment.md`.
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

Later hardware clarification: normal firmware status and BLE status indication
belong to blue LED3 on IO1. Red LED2 on IO0 is the heartbeat indicator, and
green LED1 is tied to the 3.3 V rail.

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

- Add a LED2 status task driven by the existing `status_to_leds` mapping. Later
  hardware clarification moved normal status/BLE indication to blue LED3 on
  IO1; red LED2 on IO0 is heartbeat.
- Publish latest measurement error flags from aggregation to the status task.
- Fold upload failures into status error flags so upload errors are visible
  through LED2. Later hardware clarification: current status indication belongs
  to blue LED3.
- Document the LED2 priority table and blink timing in [../10-firmware/00-architecture.md](../10-firmware/00-architecture.md).
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
- Physical LED2 visual behavior and multi-hour overnight soak still require
  longer observation. Later hardware clarification moved firmware status
  indication to blue LED3 on IO1; red LED2 on IO0 is heartbeat.

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

- Update [../10-firmware/00-architecture.md](../10-firmware/00-architecture.md) to add a two-level measurement backlog: RAM hot queue plus internal SPI flash persistent spool.
- Document that the ESP32-C3-WROOM-02-N4 4 MB internal SPI flash may be used only through a dedicated spool region that must not overlap bootloader, partition table, app image, or calibration data.
- Add planned `drivers/flash.rs`, `storage/spool.rs`, and `tasks/storage.rs` responsibilities.
- Define the persistent record format with magic, version, sequence, payload length, and CRC.
- Specify FIFO upload, HTTP-2xx-only acknowledgement, cross-reset recovery, corrupt-record handling, and drop-oldest behavior when storage is full.
- Extend [00-development-plan.md](00-development-plan.md) with Phase 16 through Phase 20 for spool pure logic, flash model tests, ESP32-C3 flash bring-up, storage task integration, and recovery/soak validation.
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
- Update [../10-firmware/00-architecture.md](../10-firmware/00-architecture.md) with the concrete flash map and validation rule.

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

## Milestone 13: Internal Flash Smoke Bring-Up

Development phase:

```text
Phase 18B: ESP32-C3 Flash Region Bring-Up
```

Scope:

- Add a `flash-smoke` Cargo feature so the destructive flash smoke path is never part of default firmware startup.
- Implement `RomSpoolFlash` over ESP32-C3 ROM SPI flash functions behind `#[cfg(target_arch = "riscv32")]`.
- Expose the project `FlashStorage` trait for the reserved spool region only.
- Refuse out-of-range access and require sector-aligned erases plus 4-byte-aligned ROM read/write operations.
- Add a hardware-only smoke test that erases, verifies erased bytes, writes a 16-byte test pattern, reads it back, and erases the sector again.
- Run the smoke test only against `0x003c_0000..0x003c_1000`, the first sector of the reserved spool region.
- Fix target-side clippy warnings in LED blink and upload logging code.

Verification:

```bash
cargo fmt --check
cargo build
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf
cargo build --target riscv32imc-unknown-none-elf --features flash-smoke
cargo clippy --all-targets
cargo clippy --target riscv32imc-unknown-none-elf --features flash-smoke
```

Unit test result:

```text
96 passed
```

Hardware validation:

```bash
timeout 75s cargo run --target riscv32imc-unknown-none-elf --features flash-smoke
timeout 45s cargo run --target riscv32imc-unknown-none-elf
```

Observed smoke result:

```text
[INFO ] flash smoke test passed spool_offset=0x003c0000
```

Observed default-firmware result:

```text
[INFO ] measurement aggregation, Wi-Fi manager, and uploader initialized
```

Notes:

- The smoke firmware erased/wrote/read back only `0x003c_0000..0x003c_1000`.
- The smoke test performs a cleanup erase of that sector after readback.
- The board was then reflashed with the default firmware without `flash-smoke`.
- The default firmware startup log did not include `flash smoke test`, confirming normal boots do not run the destructive smoke path.
- This validates real internal SPI flash read/erase/write/readback, but not yet persistent measurement retention across reset. Cross-reset retention belongs to Phase 19 and Phase 20 after the storage task uses the persistent spool.

## Milestone 14: Persistent Spool Task Integration

Development phase:

```text
Phase 19: Persistent Spool Task Integration
```

Scope:

- Add `src/tasks/storage.rs` with a host-testable `StorageBacklog` model over `FlashBackedSpool`.
- Store measurement payloads as CSV bytes in the internal flash spool with `MEASUREMENT_PAYLOAD_SIZE = 192` and `PERSISTENT_SPOOL_CAPACITY = 32`.
- Recover pending flash records at storage task startup before upload draining.
- Replace direct aggregator-to-uploader RAM queue ownership with a `StorageCommand` / `StorageResponse` protocol.
- Make `aggregator_task` submit merged measurements through `StorageCommand::Append`.
- Make `uploader_task` request `StorageCommand::Peek`, upload the oldest pending CSV payload, and issue `StorageCommand::Ack` only after HTTP 2xx.
- Add `ErrorFlags::STORAGE` and include it in LED2 status policy as a solid-on error state.
  Later hardware clarification: current normal status and BLE indication belong
  to blue LED3 on IO1; red LED2 on IO0 is heartbeat.
- Keep storage flash access isolated to `storage_task`; sensor, microphone, aggregator, and uploader tasks do not write flash directly.
- Keep the Phase 18B flash-spool write alignment changes: records and ACK entries are 4-byte aligned, and recovery scans with small fixed buffers instead of reading the whole spool region into stack memory.
- Update [../10-firmware/00-architecture.md](../10-firmware/00-architecture.md) with the implemented storage task protocol and persistent backlog data flow.

Verification:

```bash
cargo fmt
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf
```

Unit test result:

```text
102 passed
```

Added Phase 19 host tests:

```text
storage task model appends measurements in order
upload success acknowledges exactly one record
upload failure preserves record
recovered records upload before newly appended records
full persistent spool drops oldest record
storage error sets error status
```

Hardware validation:

- Completed a focused hardware recovery check on the ESP32-C3 board using the normal firmware path only.
- The `flash-smoke` feature was not enabled.
- Normal firmware startup may program the application image in the reserved app region, but persistent measurement writes were exercised only through `storage_task` in the measurement spool region:

```text
0x003c_0000..0x0040_0000
```

Commands used:

```bash
python3 post_receiver.py
timeout 90s cargo run --target riscv32imc-unknown-none-elf
probe-rs reset --chip esp32c3
timeout 60s cargo run --target riscv32imc-unknown-none-elf
```

Observed online startup and upload:

```text
[INFO ] storage spool flash range offset=0x003c0000 len=262144
[INFO ] storage recovered pending_len=0
[INFO ] network ipv4 config=StaticConfigV4 { address: 10.133.4.241/16, ... }
[INFO ] upload success sequence=0 acked=true
```

The local receiver accepted repeated POSTs from:

```text
10.133.4.241 /measurements ...
```

Receiver-offline behavior:

```text
[WARN ] upload failed error=ConnectReset sequence=41
[INFO ] storage append pending_len=8
```

Reset/recovery behavior:

- The receiver was stopped while firmware continued sampling.
- `probe-rs reset --chip esp32c3` was executed while the receiver remained offline.
- A subsequent default firmware run, still without `flash-smoke`, recovered a full pending backlog:

```text
[INFO ] storage spool flash range offset=0x003c0000 len=262144
[INFO ] storage recovered pending_len=32
[INFO ] storage append pending_len=32
```

Receiver-return behavior:

- After the receiver was restarted, recovered records uploaded before newly appended records.
- The first receiver records after return were pre-reset high-uptime measurements:

```text
59381
60596
61812
63005
64251
65450
66644
67846
69041
70234
```

- They were followed by post-reset low-uptime measurements:

```text
1848
3041
4331
5544
6747
```

- RTT also showed successful acknowledgement after receiver return:

```text
[INFO ] upload success sequence=114 acked=true
```

Notes:

- This validates persistent measurement retention across reset and recovered-before-new upload ordering on hardware.
- The pending queue reached its configured capacity of 32 during the offline interval. Because Phase 20 storage metrics are not implemented yet, oldest-drop behavior was observed only indirectly and still needs explicit dropped-oldest metrics.
- Remaining Phase 20 checks: multi-hour/overnight soak, Wi-Fi disconnect
  preservation, status LED visual confirmation for storage/upload failure
  (later hardware clarification: blue LED3, while red LED2 is heartbeat),
  forced full-spool oldest-drop confirmation with metrics, and power
  interruption during or near a flash write.

## Milestone 15: Persistent Storage Metrics And Limited Phase 20 Validation

Phase 20 code progress:

- Add `FlashRecoverReport` and `FlashBackedSpool::recover_with_report` so recovery reports recovered pending records and corrupt/interrupted log entries.
- Add `AppendResult::dropped_count` for both in-RAM capacity drops and flash-compaction drops.
- Add `StorageMetrics` with:
  - pending record count
  - dropped-oldest count
  - recovered record count
  - corrupt record count
  - last storage error
- Keep `StorageBacklog` metrics coherent across recovery, append, ACK, and storage error paths.
- Emit target-side `storage metrics ...` RTT logs at recovery, first storage event, every 16 storage events, on errors, and on the first dropped-oldest event. This keeps the first full-spool transition visible while reducing steady-state log volume for longer runs.

Verification:

```bash
cargo fmt
cargo test --lib storage::spool
cargo test --lib tasks::storage
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf
cargo clippy --all-targets
cargo clippy --target riscv32imc-unknown-none-elf
```

Observed unit test result:

```text
108 passed
```

Added or extended Phase 20 host tests:

```text
flash_backed_spool_recovery_report_counts_recovered_records
flash_backed_spool_recovery_report_counts_corrupt_tail
full_spool_drops_oldest_records
flash_backed_spool_drops_oldest_when_modeled_flash_fills
recovery_metrics_report_pending_and_recovered_records
full_spool_metrics_count_dropped_oldest_records
ack_metrics_update_pending_record_count
storage_error_metrics_record_last_error_and_preserve_pending_count
```

Limited hardware validation:

- Board probe was visible as:

```text
ESP JTAG -- 303a:1001:8C:BF:EA:44:F7:3C (EspJtag)
```

- The local receiver was started with:

```bash
python3 post_receiver.py
```

- The `flash-smoke` feature was not enabled.
- Normal firmware startup may program the application image in the app region, but persistent measurement writes in this validation were exercised only through `storage_task` in the measurement spool region:

```text
0x003c_0000..0x0040_0000
```

Hardware commands used:

```bash
timeout 60s cargo run --target riscv32imc-unknown-none-elf
timeout 90s cargo run --target riscv32imc-unknown-none-elf
timeout 60s cargo run --target riscv32imc-unknown-none-elf
```

Online metrics and ACK observation:

```text
[INFO ] storage spool flash range offset=0x003c0000 len=262144
[INFO ] storage recovered pending_len=0
[INFO ] storage metrics pending=0 recovered=0 dropped_oldest=0 corrupt=0 last_error=none
[INFO ] storage metrics pending=1 recovered=0 dropped_oldest=0 corrupt=0 last_error=none
[INFO ] upload success sequence=1271 acked=true
```

Receiver-offline metrics observation:

```text
[INFO ] storage recovered pending_len=15
[INFO ] storage metrics pending=15 recovered=15 dropped_oldest=0 corrupt=0 last_error=none
[WARN ] upload failed error=ConnectReset sequence=1309
[INFO ] storage metrics pending=31 recovered=15 dropped_oldest=0 corrupt=0 last_error=none
[INFO ] storage metrics pending=32 recovered=15 dropped_oldest=1 corrupt=0 last_error=none
[INFO ] storage metrics pending=32 recovered=15 dropped_oldest=24 corrupt=0 last_error=none
```

Final short online run with the log-volume reduction in place:

```text
[INFO ] storage spool flash range offset=0x003c0000 len=262144
[INFO ] storage recovered pending_len=0
[INFO ] storage metrics pending=0 recovered=0 dropped_oldest=0 corrupt=0 last_error=none
[INFO ] storage metrics pending=1 recovered=0 dropped_oldest=0 corrupt=0 last_error=none
[INFO ] upload success sequence=1515 acked=true
```

Notes:

- The short hardware run directly confirmed that the RTT/status metrics expose pending, recovered, corrupt, and dropped-oldest counts on the normal firmware path.
- The receiver-offline run confirmed an explicit full-spool transition on hardware: `pending=32` with `dropped_oldest` increasing.
- `timeout` terminated `probe-rs run` at the configured limit and printed SIGTERM stack frames; this was the host command ending the debug session, not a firmware panic.
- Phase 20 still has human/physical close-loop checks remaining: multi-hour or
  overnight soak, Wi-Fi-disconnect preservation separate from receiver-offline
  behavior, status LED visual confirmation (later hardware clarification: blue
  LED3, while red LED2 is heartbeat), and power interruption during or near a
  flash write.

## Milestone 16: Receiver-Offline Reset Recovery With Metrics

Phase 20 hardware validation update:

- Re-ran the receiver-offline reset and receiver-return flow with the Phase 20 storage metrics enabled.
- Used elevated `probe-rs attach` for RTT capture in the WSL/USBIP environment; non-elevated attach can fail with transient USB open errors even when `probe-rs list` sees the probe.
- The `flash-smoke` feature was not enabled.
- Normal firmware startup may program the application image in the app region, but persistent measurement writes in this validation were exercised only through `storage_task` in the measurement spool region:

```text
0x003c_0000..0x0040_0000
```

Commands used:

```bash
timeout 45s probe-rs attach --chip esp32c3 target/riscv32imc-unknown-none-elf/debug/sleep-environment-monitor
probe-rs reset --chip esp32c3
timeout 45s probe-rs attach --chip esp32c3 target/riscv32imc-unknown-none-elf/debug/sleep-environment-monitor
python3 post_receiver.py
```

Receiver-offline attach observation before reset:

```text
[WARN ] upload failed error=ConnectReset sequence=2667
[INFO ] storage metrics pending=20 recovered=0 dropped_oldest=0 corrupt=0 last_error=none
[INFO ] storage metrics pending=32 recovered=0 dropped_oldest=1 corrupt=0 last_error=none
[INFO ] storage metrics pending=32 recovered=0 dropped_oldest=36 corrupt=0 last_error=none
```

Receiver-offline recovery observation after reset:

```text
[INFO ] storage spool flash range offset=0x003c0000 len=262144
[INFO ] storage recovered pending_len=32
[INFO ] storage metrics pending=32 recovered=32 dropped_oldest=0 corrupt=0 last_error=none
[INFO ] storage metrics pending=32 recovered=32 dropped_oldest=1 corrupt=0 last_error=none
[WARN ] upload failed error=ConnectReset sequence=2740
```

Receiver-return upload observation:

```text
[INFO ] upload success sequence=2769 acked=true
[INFO ] storage metrics pending=29 recovered=32 dropped_oldest=29 corrupt=0 last_error=none
[INFO ] storage metrics pending=13 recovered=32 dropped_oldest=29 corrupt=0 last_error=none
[INFO ] storage metrics pending=1 recovered=32 dropped_oldest=29 corrupt=0 last_error=none
```

The local receiver accepted reset-recovered records after coming back online. The first rows included pre-reset high-uptime records followed by post-reset low-uptime records:

```text
204243
205439
206636
1907
3112
4318
```

Notes:

- Reset while receiver was offline recovered a full pending backlog with explicit metrics: `pending=32 recovered=32`.
- Receiver return drained the recovered backlog and ACKed successful uploads.
- Full-spool oldest-drop behavior remained visible through `dropped_oldest` during both pre-reset and post-reset offline windows.
- Remaining Phase 20 physical checks: multi-hour or overnight soak,
  Wi-Fi-disconnect preservation separate from receiver-offline behavior, status
  LED visual confirmation (later hardware clarification: blue LED3, while red
  LED2 is heartbeat), and power interruption during or near a flash write.

## Milestone 17: Repository Split For Firmware And Server

Repository layout update:

- Move the ESP32-C3 firmware package into `firmware/`.
- Move the temporary upload receiver into `server/post_receiver.py`.
- Add a root Cargo workspace with `firmware` as the default member, keeping existing root-level Cargo commands usable.
- Add `firmware/README.md` and `server/README.md` as directory entrypoints.
- Update tracked documentation and `AGENTS.md` to describe the current `firmware/`, `server/`, and `docs/` layout.

Validation commands run from the repository root:

```bash
cargo fmt
python3 -m py_compile server/post_receiver.py
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf
cargo clippy --all-targets
cargo clippy --target riscv32imc-unknown-none-elf
```

Observed results:

- `cargo test --lib` passed with 108 tests.
- The firmware package still builds as `sleep-environment-monitor` from the root workspace.
- Target firmware build and both clippy runs completed without warnings or errors.
- No hardware validation was run for this layout-only milestone, and no firmware flash-write range was exercised.

## Milestone 18: Numbered Documentation And Network Roadmap

Documentation organization update:

- Keep [../README.md](../README.md) as the tracked documentation entrypoint.
- Move project planning docs into `docs/00-project/`.
- Move firmware architecture, hardware, and firmware conventions into `docs/10-firmware/`.
- Add firmware network and configuration planning docs.
- Add server overview and REST API planning docs under `docs/20-server/`.
- Add a cross-component network roadmap under `docs/30-integration/`.
- Update `AGENTS.md` and documentation links for the numbered layout.
- Add Phase 21 for firmware configuration consolidation.
- Add Phase 22 for REST networking, discovery, time sync, and BLE readiness.

Validation:

```bash
find docs -maxdepth 3 -type f | sort
rg -n 'docs/(development_plan|walkthrough|architecture|conventions|hardware_information)\.md|]\((development_plan|walkthrough|architecture|conventions|hardware_information)\.md\)|`(development_plan|walkthrough|architecture|conventions|hardware_information)\.md`' AGENTS.md docs
```

Observed results:

- The numbered documentation layout contains project, firmware, server, and integration sections.
- No old root-level doc links remain in `AGENTS.md` or `docs/`.
- No firmware code was changed.
- No hardware validation was run for this docs-only milestone, and no firmware flash-write range was exercised.

## Milestone 19: Firmware Configuration Consolidation

Phase 21 code progress:

- Add `firmware/src/config.rs` as the central source for deployment and behavior-policy constants.
- Re-export `config` from `firmware/src/lib.rs`.
- Move task-local Wi-Fi, upload, network, runtime, sensor, microphone, storage, aggregator, and LED policy values to `config`.
- Preserve existing public const-generic aliases:
  - `tasks::storage::MEASUREMENT_PAYLOAD_SIZE`
  - `tasks::storage::PERSISTENT_SPOOL_CAPACITY`
  - `tasks::STORAGE_REQUEST_CAPACITY`
- Preserve current runtime behavior:
  - Wi-Fi SSID remains `FZU`.
  - REST upload remains `10.133.56.218:8080/measurements`.
  - HTTP user agent remains `sleep-environment-monitor/0.1`.
  - CSV measurement payload encoding is unchanged.

Verification commands run from the repository root:

```bash
cargo fmt
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf
cargo clippy --all-targets
cargo clippy --target riscv32imc-unknown-none-elf
```

Observed results:

- `cargo test --lib` passed with 108 tests.
- Target firmware build completed without errors.
- Both clippy runs completed without warnings or errors.
- A targeted search found the Wi-Fi SSID, REST endpoint/path, user-agent, and migrated timing/buffer values only in `config.rs` or test fixtures.
- No hardware validation was run for this behavior-preserving configuration refactor, and no firmware flash-write range was exercised.

## Milestone 20: REST Network Discovery And Time Sync

Phase 22 implementation:

- Replace the old `/measurements` CSV upload path with JSON schema version 1 at `POST /api/v1/measurements`.
- Keep persistent storage FIFO semantics and acknowledge records only after HTTP 2xx.
- Store measurement JSON field fragments in the flash spool so upload can add `device_id`, spool `sequence`, `time_status`, and optional `wall_clock_unix_ms`.
- Mark new JSON field-fragment spool records and skip legacy unflagged CSV records recovered from flash.
- Keep recovered records from previous boots as `uptime_only` so current boot time sync cannot synthesize false wall-clock timestamps.
- Add endpoint resolution with future provisioned endpoint precedence, UDP discovery, and static fallback.
- Add UDP discovery on port `39022` using query payload `sleep-environment-monitor.discovery`.
- Add SNTP/NTP time sync with `GET /api/v1/time` as REST fallback.
- Add detailed network/upload status flags for IP, discovery, time, transport, and HTTP failures.
- Add compile-time Wi-Fi credential support for open, WPA-Personal, WPA2-Personal, and WPA/WPA2-Personal mixed networks.
- Keep WPA3 and Enterprise/EAP Wi-Fi deferred until the dependency stack and target hardware are validated for those modes.
- Update the local server receiver to accept JSON uploads, serve time, serve discovery metadata, and answer UDP discovery queries.

Validation commands run from the repository root:

```bash
cargo fmt
python3 -m py_compile server/post_receiver.py
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf
cargo clippy --all-targets
cargo clippy --target riscv32imc-unknown-none-elf
```

Observed results:

- `cargo test --lib` passed with 121 tests.
- Target firmware build completed without errors.
- Both clippy runs completed without warnings or errors.
- `python3 -m py_compile server/post_receiver.py` completed without errors.
- Local HTTP/UDP smoke testing of the Phase 22 receiver verified `POST /api/v1/measurements` returns `204` for JSON, other `POST` paths return `404`, `GET /api/v1/time` returns server time JSON, the well-known discovery document is served, and UDP discovery returns endpoint JSON.
- No hardware validation was run for this milestone, and no firmware flash-write range was exercised.

## Milestone 21: Phase 22 Hardware Validation And Spool Buffer Fix

Hardware validation for Phase 22 was run against the ESP32-C3 board on the open
`FZU` network with the local Phase 22 receiver listening on HTTP `8080` and UDP
`39022`.

Flash range statement before the default firmware runs:

- Normal firmware startup may program the application image in the app region,
  but persistent measurement writes in this validation were exercised only
  through `storage_task` in the measurement spool region:

```text
0x003c_0000..0x0040_0000
```

- The `flash-smoke` feature was not enabled, so the first-sector smoke-test
  range `0x003c_0000..0x003c_1000` was not erased or written.

Validation commands run from the repository root:

```bash
python3 server/post_receiver.py
probe-rs list
timeout 120s cargo run --target riscv32imc-unknown-none-elf
cargo fmt
cargo test --lib storage::spool
cargo test --lib tasks::storage
python3 -m py_compile server/post_receiver.py
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf
cargo clippy --all-targets
cargo clippy --target riscv32imc-unknown-none-elf
timeout 120s cargo run --target riscv32imc-unknown-none-elf
python3 -c 'import socket; s=socket.socket(socket.AF_INET, socket.SOCK_DGRAM); s.settimeout(2); s.sendto(b"sleep-environment-monitor.discovery", ("127.0.0.1", 39022)); print(s.recvfrom(1024))'
git diff --check
```

Initial hardware run exposed a real Phase 22 storage regression:

```text
[INFO ] storage spool flash range offset=0x003c0000 len=262144
[WARN ] storage recovery failed error=Spool(BufferTooSmall)
[WARN ] storage peek failed error=Spool(Flash(OutOfBounds))
```

Root cause and fix:

- Phase 22 raised `config::storage::MEASUREMENT_PAYLOAD_SIZE` to `384`, but
  `storage::spool::FLASH_ENTRY_BUFFER_LEN` was still `256`.
- A maximum-sized 384-byte payload encodes to 408 bytes with the spool header
  and 4-byte alignment, so `FlashBackedSpool::recover_with_report` failed before
  reading flash.
- Increase the spool flash-entry scratch buffer to `512`.
- Add tests that assert the configured payload size fits the flash-entry buffer
  and that a 384-byte payload can be recovered from a flash-backed spool.
- Preserve the original recovery error for later storage `Peek` and `Ack`
  responses instead of reporting a misleading synthetic flash out-of-bounds
  error.

Observed fixed hardware run:

```text
[INFO ] storage spool flash range offset=0x003c0000 len=262144
[INFO ] storage recovered pending_len=0
[INFO ] storage metrics pending=0 recovered=32 dropped_oldest=0 skipped_legacy=32 corrupt=0 last_error=none
[INFO ] wifi connected ssid=FZU channel=1 aid=41172
[INFO ] network ipv4 config=StaticConfigV4 { address: 10.133.2.168/16, gateway: Some(10.133.255.254), dns_servers: [114.114.114.114, 210.34.48.34] }
[INFO ] time synced unix_ms=1779399528010
[INFO ] upload success sequence=8522 acked=true
[INFO ] storage metrics pending=0 recovered=32 dropped_oldest=0 skipped_legacy=32 corrupt=0 last_error=none
```

Receiver observations:

```text
http on 0.0.0.0:8080
udp discovery on 0.0.0.0:39022
upload accepted from 10.133.2.168 bytes=338
upload accepted from 10.133.2.168 bytes=345
```

Discovery observations:

- The board repeatedly logged `discovery failed error=Discovery` in this
  WSL/USBIP plus campus Wi-Fi environment.
- Static fallback to `10.133.56.218:8080` succeeded and uploaded JSON
  measurements to `POST /api/v1/measurements`.
- A host-side UDP smoke query to `127.0.0.1:39022` returned the expected compact
  discovery JSON, so the local receiver's UDP responder was functioning.
- The remaining discovery gap is hardware-path validation of LAN UDP broadcast
  from the board to the receiver environment, likely dependent on Windows/WSL
  networking or firewall handling of UDP `39022`.

Observed software verification results:

- `cargo test --lib storage::spool` passed with 28 tests.
- `cargo test --lib tasks::storage` passed with 11 tests.
- `cargo test --lib` passed with 123 tests.
- Target firmware build completed without errors.
- Both clippy runs completed without warnings or errors.
- `python3 -m py_compile server/post_receiver.py` completed without errors.

The second `timeout 120s cargo run ...` ended by host-side timeout while the
target was in the idle hook; this was not a firmware panic.

## Milestone 22: UDP Discovery Hardware Validation

After allowing Windows inbound firewall traffic for UDP port `39022`, Phase 22
UDP discovery was revalidated on the ESP32-C3 hardware with the local receiver
listening on HTTP `8080` and UDP `39022`.

Flash range statement before the default firmware run:

- Normal firmware startup may program the application image in the app region,
  but persistent measurement writes in this validation were exercised only
  through `storage_task` in the measurement spool region:

```text
0x003c_0000..0x0040_0000
```

- The `flash-smoke` feature was not enabled, so the first-sector smoke-test
  range `0x003c_0000..0x003c_1000` was not erased or written.

Validation commands run from the repository root:

```bash
probe-rs list
python3 server/post_receiver.py
timeout 120s cargo run --target riscv32imc-unknown-none-elf
```

Observed firmware results:

```text
[INFO ] storage spool flash range offset=0x003c0000 len=262144
[INFO ] storage recovered pending_len=1
[INFO ] storage metrics pending=1 recovered=1 dropped_oldest=0 skipped_legacy=0 corrupt=0 last_error=none
[INFO ] wifi connected ssid=FZU channel=1 aid=41172
[INFO ] network ipv4 config=StaticConfigV4 { address: 10.133.2.168/16, gateway: Some(10.133.255.254), dns_servers: [114.114.114.114, 210.34.48.34] }
[INFO ] discovery endpoint ipv4=10.133.56.218 port=8080
[INFO ] time synced unix_ms=1779400151794
[INFO ] upload success sequence=8762 acked=true
[INFO ] discovery endpoint ipv4=10.133.56.218 port=8080
[INFO ] time synced unix_ms=1779400213016
[INFO ] upload success sequence=8822 acked=true
```

Receiver observations:

```text
http on 0.0.0.0:8080
udp discovery on 0.0.0.0:39022
upload accepted from 10.133.2.168 bytes=298
upload accepted from 10.133.2.168 bytes=344
```

Notes:

- The earlier hardware discovery failure was caused by the Windows firewall not
  allowing inbound UDP `39022`.
- With UDP `39022` allowed, the board repeatedly discovered the receiver as
  `10.133.56.218:8080`.
- The discovered endpoint was then used for REST time sync and JSON measurement
  uploads.
- The `timeout 120s cargo run ...` command ended by host-side timeout while the
  target was in the idle hook; this was not a firmware panic.

## Milestone 23: Formal Server Documentation Plan

Phase 23 documentation planning:

- Add Phase 23 to [00-development-plan.md](00-development-plan.md) for replacing
  or superseding the temporary stdlib-only Phase 22 receiver with a formal
  Python server foundation.
- Keep the Phase 22 REST API contract unchanged:
  - `POST /api/v1/measurements`.
  - `GET /api/v1/time`.
  - `GET /.well-known/sleep-environment-monitor`.
  - UDP discovery on port `39022`.
- Document the planned server stack: FastAPI, Uvicorn, Pydantic, Rich,
  `argparse`, pytest, and Ruff as check-only guidance.
- Add [../20-server/02-toolchain.md](../20-server/02-toolchain.md) for Python
  server toolchain, style policy, formatter/linter policy, and detailed
  hardware-free unit-test expectations.
- Add [../20-server/03-cli.md](../20-server/03-cli.md) for the planned
  `argparse` CLI commands and options.
- Update [../20-server/00-overview.md](../20-server/00-overview.md),
  [../20-server/01-rest-api.md](../20-server/01-rest-api.md),
  [../30-integration/00-network-roadmap.md](../30-integration/00-network-roadmap.md),
  [../README.md](../README.md), [../../server/README.md](../../server/README.md),
  [../../AGENTS.md](../../AGENTS.md), and
  [../10-firmware/02-conventions.md](../10-firmware/02-conventions.md) to point
  at the formal server plan.

Server test requirements now explicitly cover:

- CLI argument parsing and invalid argument rejection.
- Application configuration and discovery metadata derivation.
- REST API behavior for valid uploads, invalid JSON/schema, time, discovery
  document, and unknown POST paths.
- Measurement model validation and duplicate handling policy.
- UDP discovery helper behavior.
- Rich/plain logging and bounded upload diagnostics.

Server test quality requirements now explicitly require deterministic,
hardware-free tests that assert observable behavior, avoid real network
dependencies where possible, avoid sleeps except for controlled timeout tests,
do not depend on execution order, and add regressions for reproducible
integration bugs.

Server style policy now states:

- Comments and docstrings use Google style.
- Formatter and linter output is advisory only.
- Automatic formatter or linter rewrites must not be applied across server code.
- Auto-fix and auto-format commands must not be used as implementation or
  commit-preparation shortcuts.
- Local suppression markers are allowed when they protect intentional
  readability, especially manually aligned protocol tables, dense field maps,
  or payload fixtures.

Validation commands run from the repository root:

```bash
find docs/20-server -maxdepth 1 -type f | sort
rg -n 'Phase 23|02-toolchain|03-cli|advisory only|Never automatically|Google style' docs server AGENTS.md
git diff --check
```

Observed results:

- The server documentation set now contains overview, REST API, toolchain, and
  CLI documents.
- The new Phase 23 plan is linked from the project plan, server docs,
  integration roadmap, documentation index, server README, firmware conventions,
  and agent entrypoint.
- No firmware or server implementation code was changed.
- No hardware validation was run for this docs-only milestone, and no firmware
  flash-write range was exercised.

## Milestone 24: Formal Server Foundation

Phase 23 implementation:

- Added the formal `server/` Python package with FastAPI, Uvicorn, Pydantic,
  Rich, `argparse`, pytest, and Ruff.
- Added `server/pyproject.toml` and `server/uv.lock`.
- Implemented `sleep-env-server` with:
  - `serve`
  - `check-config`
  - `print-discovery`
- Preserved the Phase 22 firmware/server contract:
  - `POST /api/v1/measurements`.
  - `GET /api/v1/time`.
  - `GET /.well-known/sleep-environment-monitor`.
  - UDP discovery query `sleep-environment-monitor.discovery` on port `39022`.
- Implemented schema-version-1 measurement validation and process-local
  duplicate tracking by `(device_id, sequence)`.
- Kept duplicate uploads idempotent: repeated valid measurements return `204`.
- Added bounded plain, Rich, and JSONL output paths that log source, byte count,
  device id, sequence, and duplicate status without dumping the full payload.
- Demoted `server/post_receiver.py` to a compatibility wrapper that dispatches
  to the formal CLI. With no subcommand it runs `sleep-env-server serve`.
- Updated server documentation to describe the implemented package structure,
  command surface, check commands, API behavior, and compatibility wrapper.

Validation commands run from `server/`:

```bash
env UV_CACHE_DIR=.cache/uv uv lock
env UV_CACHE_DIR=.cache/uv uv run pytest
env UV_CACHE_DIR=.cache/uv uv run ruff check --diff .
env UV_CACHE_DIR=.cache/uv uv run ruff format --check .
```

Observed results:

- `uv lock` resolved 33 packages.
- `uv run pytest` collected and passed 40 hardware-free tests.
- `uv run ruff check --diff .` completed with no diagnostics after manual
  review and edits.
- `uv run ruff format --check .` completed with all server files formatted
  after manual review and edits.

Additional CLI smoke checks:

```bash
env UV_CACHE_DIR=.cache/uv uv run sleep-env-server check-config
env UV_CACHE_DIR=.cache/uv uv run sleep-env-server print-discovery --output json
python3 server/post_receiver.py check-config
```

Live loopback smoke checks:

```bash
env UV_CACHE_DIR=.cache/uv uv run sleep-env-server serve --host 127.0.0.1 --port 18080 --udp-discovery-port 39024 --no-rich
curl -fsS http://127.0.0.1:18080/api/v1/time
curl -fsS http://127.0.0.1:18080/.well-known/sleep-environment-monitor
env UV_CACHE_DIR=.cache/uv uv run python -c 'import socket; sock=socket.socket(socket.AF_INET, socket.SOCK_DGRAM); sock.settimeout(2); sock.sendto(b"sleep-environment-monitor.discovery", ("127.0.0.1", 39024)); print(sock.recvfrom(512)[0].decode())'
```

Observed loopback responses:

```text
{"unix_ms":1779403882960,"source":"server"}
{"api_base":"/api/v1","measurement_upload":"/api/v1/measurements","time":"/api/v1/time","udp_discovery_port":39024}
{"host":"127.0.0.1","port":18080,"api_base":"/api/v1","measurement_upload":"/api/v1/measurements","time":"/api/v1/time"}
```

Notes:

- The first sandboxed `curl` attempt failed with TCP socket permission errors;
  the same loopback HTTP checks passed when run with approval outside the
  sandbox.
- No ESP32-C3 hardware validation was run for this implementation milestone.
- No firmware flashing was performed.
- No firmware flash-write range was exercised.

## Milestone 25: BLE Upload Channel Documentation Plan

Phase 24 documentation planning:

- Add Phase 24 to [00-development-plan.md](00-development-plan.md) for a real
  Bluetooth Low Energy upload channel that can operate independently from
  Wi-Fi.
- Define BLE as a structured GATT upload path, not Bluetooth Classic SPP, a
  transparent UART, or Nordic UART Service style serial streaming.
- Add [../10-firmware/05-ble.md](../10-firmware/05-ble.md) to describe the BLE
  role, GATT protocol boundary, storage ACK policy, Wi-Fi coexistence, pairing
  entry, and security expectations.
- Document that BLE and Wi-Fi can be independently enabled or disabled.
- Document BLE storage ACK rules:
  - Wi-Fi REST still ACKs only after HTTP 2xx.
  - BLE may transmit copies while Wi-Fi upload is available and succeeding, but
    must not ACK storage in that state.
  - BLE may ACK exactly one oldest record only when Wi-Fi upload is disabled or
    unavailable and an authorized central confirms complete receipt.
- Document BOOT / IO9 as a possible future runtime pairing or authorization
  input only, with constraints that preserve download-mode behavior.
- Clarify that Phase 24 BLE upload does not add a server-side BLE protocol; a
  phone or gateway that forwards BLE records should use the existing REST API.
- Update the documentation index, AGENTS entrypoint, firmware architecture,
  hardware notes, firmware network/configuration docs, server docs, and
  integration roadmap to point at the Phase 24 boundary.

Validation commands run from the repository root:

```bash
find docs -maxdepth 3 -type f | sort
rg -n 'Phase 24|05-ble|Bluetooth Low Energy|Nordic UART|BOOT / IO9|serial-port emulation|BLE ACK|Wi-Fi upload is unavailable' docs AGENTS.md server/README.md
rg -n 'BLE readiness|BLE provisioning|future BLE provisioning|future provisioned config' AGENTS.md docs/README.md docs/10-firmware docs/20-server docs/30-integration server/README.md
git diff --check
```

Observed results:

- The documentation set now contains the BLE upload design boundary document.
- Phase 24 is linked from the project plan, firmware docs, integration roadmap,
  documentation index, server boundary docs, and agent entrypoint.
- Current docs explicitly prohibit using BLE as a transparent serial port.
- Current docs describe BOOT / IO9 reuse only as a future runtime input and do
  not change the hardware requirement that BOOT still enter download mode during
  reset or power-on.
- No firmware or server implementation code was changed.
- No hardware validation was run for this docs-only milestone.
- No firmware flashing was performed.
- No firmware flash-write range was exercised.

## Milestone 26: BLE Compile Integration Boundary

Phase 24A implementation:

- Added firmware feature gates:
  - `ble-upload` enables project BLE code and `esp-radio/ble`.
  - `radio-coex` enables `esp-radio/coex`; `ble-upload` selects it so BLE
    feature builds compile Wi-Fi and BLE with coexistence support.
- Added `tasks::ble` with project-specific BLE protocol constants and
  structured status, record metadata, record fragment, control, and ACK-policy
  helper types.
- Added hardware-independent BLE helper tests for status encoding, metadata
  encoding, fragment bounds/encoding, control frame decoding, and Wi-Fi/BLE ACK
  policy.
- Added an ESP32-C3 BLE task boundary that owns
  `esp_radio::ble::controller::BleConnector` when built with
  `--features ble-upload`.
- Preserved the existing Wi-Fi initialization, network stack, and REST uploader
  in BLE-enabled builds so compile-time coexistence is checked.
- Kept BLE runtime disabled in default builds.
- Did not add a GATT host/server, advertising, pairing, BLE central connection,
  record transfer, or BLE storage acknowledgement behavior.
- Did not change flash format, measurement JSON payload shape, or
  `storage_task` ACK semantics.
- Updated [00-development-plan.md](00-development-plan.md) and
  [../10-firmware/05-ble.md](../10-firmware/05-ble.md) to mark Phase 24A as
  compile integration only.

Validation commands run from the repository root:

```bash
cargo fmt
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf
cargo clippy --all-targets
cargo clippy --target riscv32imc-unknown-none-elf
cargo build --target riscv32imc-unknown-none-elf --features ble-upload
cargo clippy --target riscv32imc-unknown-none-elf --features ble-upload
```

Observed results:

- `cargo test --lib` passed 130 hardware-independent tests.
- Normal ESP32-C3 target build passed without `ble-upload`.
- Normal host/all-target Clippy passed without warnings after removing a
  constant-only assertion from the new config test.
- Normal ESP32-C3 target Clippy passed without `ble-upload`.
- BLE ESP32-C3 target build passed with `--features ble-upload`.
- BLE ESP32-C3 target Clippy passed with `--features ble-upload`.
- The BLE feature build compiled the existing Wi-Fi/uploader path and the new
  BLE controller boundary together.

Notes:

- BLE functionality was not tested in this milestone: no advertising check, no
  central connection, no pairing/security validation, no GATT read/write/notify
  validation, no BLE record transfer, and no BLE ACK or record-drain validation.
- No ESP32-C3 hardware validation was run.
- No firmware flashing was performed.
- No firmware flash-write range was exercised.

## Milestone 27: BLE Transfer ACK Core

Phase 24B implementation:

- Added storage payload flag exposure so BLE metadata can report the persisted
  record payload format without changing the flash record format.
- Changed storage ACK requests to include the record sequence and acknowledge
  only when the current oldest pending record still matches that sequence.
  This prevents stale Wi-Fi or future BLE ACKs from deleting a newer oldest
  record.
- Split target-side storage responses into Wi-Fi and BLE response signals so
  concurrent upload clients cannot consume each other's storage replies.
- Updated the Wi-Fi REST uploader to use the Wi-Fi storage client route while
  preserving the existing HTTP 2xx-only ACK condition.
- Added a hardware-independent BLE transfer session model for:
  - metadata derived from the stored payload
  - ordered fragment requests
  - complete-record confirmation
  - ACK suppression while Wi-Fi upload is succeeding
  - ACK eligibility when Wi-Fi upload is unavailable
  - duplicate ACK suppression
  - disconnect reset without storage acknowledgement
- Kept the target BLE task as an HCI/controller boundary only. It still does
  not advertise, run a GATT server, accept central connections, transfer
  records, or ACK storage at runtime.
- Updated [00-development-plan.md](00-development-plan.md),
  [../10-firmware/00-architecture.md](../10-firmware/00-architecture.md), and
  [../10-firmware/05-ble.md](../10-firmware/05-ble.md) to record Phase 24B as
  transfer/ACK core work, not full BLE runtime bring-up.

Validation commands run from the repository root:

```bash
cargo fmt
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf
cargo build --target riscv32imc-unknown-none-elf --features ble-upload
cargo clippy --all-targets
cargo clippy --target riscv32imc-unknown-none-elf
cargo clippy --target riscv32imc-unknown-none-elf --features ble-upload
```

Observed results:

- `cargo test --lib` passed 140 hardware-independent tests.
- Normal ESP32-C3 target build passed without `ble-upload`.
- BLE ESP32-C3 target build passed with `--features ble-upload`.
- Normal host/all-target Clippy passed.
- Normal ESP32-C3 target Clippy passed without `ble-upload`.
- BLE ESP32-C3 target Clippy passed with `--features ble-upload`.

Notes:

- BLE functionality was not tested in this milestone: no advertising check, no
  central connection, no pairing/security validation, no GATT read/write/notify
  validation, no live BLE record transfer, and no BLE storage-drain validation.
- No ESP32-C3 hardware validation was run.
- No firmware flashing was performed.
- No firmware flash-write range was exercised.

## Milestone 28: BLE Pairing Gesture Core

Phase 24C implementation:

- Added BLE pairing-window timing constants in `config::ble`.
- Added a hardware-independent active-low BOOT button model.
- Added a pure BLE pairing-window gesture state machine for long press, short
  press rejection, release-before-retrigger behavior, and timeout.
- Updated the BLE feature target path to configure GPIO9 / BOOT as input-only.
  Later hardware review on 2026-05-25 corrected the runtime electrical
  assumption: the board has no discrete IO9 pull-up, BOOT/IO9 has a capacitor
  to GND in parallel with the BOOT button, and firmware must explicitly enable
  the MCU internal pull-up when reading IO9 at runtime.
- Updated the BLE task boundary to monitor BOOT / IO9 and log pairing-window
  open/expire events.
- Kept the target BLE task as an HCI/controller boundary only. The pairing
  window is not yet connected to GATT security, authorization, bonded state, or
  record access.
- Updated [00-development-plan.md](00-development-plan.md),
  [../10-firmware/00-architecture.md](../10-firmware/00-architecture.md),
  [../10-firmware/01-hardware.md](../10-firmware/01-hardware.md),
  [../10-firmware/03-network.md](../10-firmware/03-network.md),
  [../10-firmware/04-configuration.md](../10-firmware/04-configuration.md),
  and [../10-firmware/05-ble.md](../10-firmware/05-ble.md) to record Phase
  24C as pairing gesture core work, not full BLE pairing/security bring-up.

Validation commands run from the repository root:

```bash
cargo fmt
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf
cargo build --target riscv32imc-unknown-none-elf --features ble-upload
cargo clippy --all-targets
cargo clippy --target riscv32imc-unknown-none-elf
cargo clippy --target riscv32imc-unknown-none-elf --features ble-upload
```

Observed results:

- `cargo test --lib` passed 146 hardware-independent tests.
- Normal ESP32-C3 target build passed without `ble-upload`.
- BLE ESP32-C3 target build passed with `--features ble-upload`.
- Normal host/all-target Clippy passed.
- Normal ESP32-C3 target Clippy passed without `ble-upload`.
- BLE ESP32-C3 target Clippy passed with `--features ble-upload`.

Notes:

- BOOT / IO9 was not hardware-validated in this milestone.
- BLE functionality was not tested in this milestone: no advertising check, no
  central connection, no real pairing/security validation, no GATT
  read/write/notify validation, no live BLE record transfer, and no BLE
  storage-drain validation.
- No ESP32-C3 hardware validation was run.
- No firmware flashing was performed.
- No firmware flash-write range was exercised.

## Milestone 29: BLE GATT Runtime Skeleton

Phase 24D implementation:

- Added TrouBLE BLE host dependencies behind the `ble-upload` feature so the
  default non-BLE firmware path does not enable the GATT host stack.
- Replaced the BLE feature target task's HCI polling loop with a real TrouBLE
  peripheral host built on `esp_radio::ble::controller::BleConnector`.
- Added a project-specific GATT service skeleton with characteristics for:
  - status
  - record metadata
  - record fragment
  - control
- Kept the status characteristic readable and updated it with BLE runtime state
  for host-pending, advertising, connected, and error states.
- Left record metadata, record fragment, and control characteristic access
  disabled until pairing, authorization, record transfer, and BLE ACK behavior
  are implemented.
- Split BOOT / IO9 pairing-window monitoring into its own BLE feature task so
  GATT advertising and connection waits do not stop the pairing gesture state
  machine.
- Preserved the existing Wi-Fi initialization, network stack, REST uploader,
  and storage task behavior in BLE feature builds.
- Kept BLE runtime code from sending `StorageCommand::Ack` in this milestone.
- Updated [00-development-plan.md](00-development-plan.md),
  [../10-firmware/00-architecture.md](../10-firmware/00-architecture.md), and
  [../10-firmware/05-ble.md](../10-firmware/05-ble.md) to record Phase 24D as
  a GATT runtime skeleton, not full BLE upload completion.

Validation commands run from the repository root:

```bash
cargo fmt
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf
cargo build --target riscv32imc-unknown-none-elf --features ble-upload
cargo clippy --all-targets
cargo clippy --target riscv32imc-unknown-none-elf
cargo clippy --target riscv32imc-unknown-none-elf --features ble-upload
```

Observed results:

- `cargo test --lib` passed 146 hardware-independent tests.
- Normal ESP32-C3 target build passed without `ble-upload`.
- BLE ESP32-C3 target build passed with `--features ble-upload`.
- Normal host/all-target Clippy passed.
- Normal ESP32-C3 target Clippy passed without `ble-upload`.
- BLE ESP32-C3 target Clippy passed with `--features ble-upload`.

Notes:

- BLE hardware behavior was not tested in this milestone: no live advertising
  scan, no central connection, no real pairing/security validation, no live GATT
  read/write/notify validation, no BLE record transfer, and no BLE storage-drain
  validation.
- BOOT / IO9 was not hardware-validated in this milestone.
- No ESP32-C3 hardware validation was run.
- No firmware flashing was performed.
- No firmware flash-write range was exercised.

## Milestone 30: BLE Authorized Record Read Skeleton

Phase 24E implementation:

- Shared the BOOT / IO9 pairing-window state with the BLE GATT task.
- Changed record metadata, record fragment, and control characteristics from
  attribute-level disabled access to task-level authorization checks.
- Rejects closed-window record metadata, record fragment, and control access
  with ATT authorization errors.
- Wired authorized BLE metadata/control requests to read the oldest pending
  record through `storage_task` using `StorageCommand::Peek(StorageClient::Ble)`.
- Added a per-connection transfer runtime that prepares structured metadata and
  ordered fragments in the project GATT characteristics.
- Sends fragment notifications when subscribed, while keeping the same
  structured fragment characteristic readable for centrals that poll after a
  control request.
- Treats `CompleteRecord` as an in-memory transfer-session marker only.
- Explicitly rejects `AckRecord`; BLE runtime still does not send
  `StorageCommand::Ack` or delete storage records.
- Preserved the existing Wi-Fi initialization, network stack, REST uploader,
  storage task behavior, flash format, and measurement JSON payload shape.
- Updated [00-development-plan.md](00-development-plan.md),
  [../10-firmware/00-architecture.md](../10-firmware/00-architecture.md), and
  [../10-firmware/05-ble.md](../10-firmware/05-ble.md) to record Phase 24E as
  an authorized read-only transfer skeleton, not full BLE upload completion.

Validation commands run from the repository root:

```bash
cargo fmt
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf --features ble-upload
cargo clippy --target riscv32imc-unknown-none-elf --features ble-upload
cargo build --target riscv32imc-unknown-none-elf
cargo clippy --target riscv32imc-unknown-none-elf
cargo clippy --all-targets
git diff --check
```

Observed results:

- `cargo test --lib` passed 146 hardware-independent tests.
- Normal ESP32-C3 target build passed without `ble-upload`.
- BLE ESP32-C3 target build passed with `--features ble-upload`.
- Normal host/all-target Clippy passed.
- Normal ESP32-C3 target Clippy passed without `ble-upload`.
- BLE ESP32-C3 target Clippy passed with `--features ble-upload`.
- `git diff --check` passed.

Notes:

- BLE hardware behavior was not tested in this milestone: no live advertising
  scan, no central connection, no real pairing/security validation, no live GATT
  read/write/notify validation, no BLE record transfer, and no BLE storage-drain
  validation.
- BLE ACK behavior remains disabled in runtime code. ACK policy is still only
  hardware-independent logic until live transfer and Wi-Fi-unavailable cases are
  validated.
- BOOT / IO9 was not hardware-validated in this milestone.
- No ESP32-C3 hardware validation was run.
- No firmware flashing was performed.
- No firmware flash-write range was exercised.

## Milestone 31: BLE Runtime ACK Wiring

Phase 24F implementation:

- Added a shared latest network/upload status snapshot.
- Kept Wi-Fi and uploader tasks publishing their existing `Signal`s for the
  LED/status task while also updating the shared snapshot.
- Updated the BLE task to read the shared snapshot instead of consuming the
  existing single-consumer status `Signal`s.
- Wired authorized `AckRecord` requests through the existing BLE transfer ACK
  policy.
- Suppressed BLE storage ACK while Wi-Fi is connected or IP-ready and the last
  upload result is success.
- Sent `StorageCommand::Ack { client: StorageClient::Ble, sequence }` when a
  complete record is confirmed and the ACK policy permits BLE drain.
- Kept `storage_task` as the flash-backed deletion owner; its existing sequence
  check prevents stale BLE ACKs from deleting a different oldest pending record.
- Preserved the Wi-Fi upload path, flash format, and measurement JSON payload
  shape.
- Updated [00-development-plan.md](00-development-plan.md),
  [../10-firmware/00-architecture.md](../10-firmware/00-architecture.md), and
  [../10-firmware/05-ble.md](../10-firmware/05-ble.md) to record Phase 24F as
  runtime ACK wiring, not full BLE upload completion.

Validation commands run from the repository root:

```bash
cargo fmt
cargo build --target riscv32imc-unknown-none-elf --features ble-upload
cargo clippy --target riscv32imc-unknown-none-elf --features ble-upload
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf
cargo clippy --all-targets
cargo clippy --target riscv32imc-unknown-none-elf
git diff --check
```

Observed results:

- `cargo test --lib` passed 147 hardware-independent tests.
- Normal ESP32-C3 target build passed without `ble-upload`.
- BLE ESP32-C3 target build passed with `--features ble-upload`.
- Normal host/all-target Clippy passed.
- Normal ESP32-C3 target Clippy passed without `ble-upload`.
- BLE ESP32-C3 target Clippy passed with `--features ble-upload`.
- `git diff --check` passed.

Notes:

- BLE hardware behavior was not tested in this milestone: no live advertising
  scan, no central connection, no real pairing/security validation, no live GATT
  read/write/notify validation, no BLE record transfer, and no BLE storage-drain
  validation.
- BOOT / IO9 was not hardware-validated in this milestone.
- No ESP32-C3 hardware validation was run.
- No firmware flashing was performed.
- No firmware flash-write range was exercised.

## Milestone 32: Independent Radio Feature Selection

Phase 24G implementation:

- Added the default `wifi-upload` firmware feature and moved `esp-radio/wifi`
  behind it, preserving the existing default Wi-Fi REST upload behavior.
- Kept `ble-upload` independent so the BLE upload boundary can compile without
  Wi-Fi when the build uses `--no-default-features --features ble-upload`.
- Made `radio-coex` the explicit BLE+Wi-Fi coexistence feature; it selects
  `ble-upload`, `wifi-upload`, and `esp-radio/coex`.
- Left `esp-radio/coex` disabled in BLE-only builds because `esp-radio 0.18.0`
  references its Wi-Fi module when coexistence is enabled.
- Gated target-side Wi-Fi radio setup, DHCP runner, and REST uploader startup
  on `wifi-upload`.
- Kept sampling, aggregation, persistent storage, status LED, and optional BLE
  startup compiling when `wifi-upload` is disabled.
- Preserved the flash format, measurement JSON payload shape, and existing
  storage ACK semantics.
- Updated [00-development-plan.md](00-development-plan.md),
  [../10-firmware/00-architecture.md](../10-firmware/00-architecture.md),
  [../10-firmware/03-network.md](../10-firmware/03-network.md),
  [../10-firmware/04-configuration.md](../10-firmware/04-configuration.md),
  and [../10-firmware/05-ble.md](../10-firmware/05-ble.md) to record Phase 24G
  as compile-validated independent radio feature selection, not full BLE upload
  completion.

Validation commands run from the repository root:

```bash
cargo fmt
cargo test --lib
cargo clippy --all-targets
cargo build --target riscv32imc-unknown-none-elf
cargo clippy --target riscv32imc-unknown-none-elf
cargo build --target riscv32imc-unknown-none-elf --no-default-features
cargo clippy --target riscv32imc-unknown-none-elf --no-default-features
cargo build --target riscv32imc-unknown-none-elf --no-default-features --features ble-upload
cargo clippy --target riscv32imc-unknown-none-elf --no-default-features --features ble-upload
cargo build --target riscv32imc-unknown-none-elf --features ble-upload,radio-coex
cargo clippy --target riscv32imc-unknown-none-elf --features ble-upload,radio-coex
git diff --check
```

Observed results:

- `cargo test --lib` passed 147 hardware-independent tests.
- Host/all-target Clippy passed.
- Default ESP32-C3 target build and Clippy passed with default `wifi-upload`.
- No-radio ESP32-C3 target build and Clippy passed with
  `--no-default-features`.
- BLE-only ESP32-C3 target build and Clippy passed with
  `--no-default-features --features ble-upload`.
- BLE+Wi-Fi coexistence ESP32-C3 target build and Clippy passed with
  `--features ble-upload,radio-coex`.
- `git diff --check` passed.

Notes:

- BLE hardware behavior was not tested in this milestone: no live advertising
  scan, no central connection, no real pairing/security validation, no live GATT
  read/write/notify validation, no BLE record transfer, and no BLE storage-drain
  validation.
- BOOT / IO9 was not hardware-validated in this milestone.
- No ESP32-C3 hardware validation was run.
- No firmware flashing was performed.
- No firmware flash-write range was exercised.

## Milestone 33: BLE Status Runtime Snapshot

Phase 24H implementation:

- Added a shared latest firmware status snapshot for BLE status reads.
- Kept the existing LED/status task `Signal`s unchanged and single-consumer.
- Updated aggregation paths to publish the latest firmware error flags to the
  shared snapshot.
- Updated `storage_task` to publish pending record count after recovery,
  append, and ACK paths.
- Updated storage failure paths to preserve the storage error flag in the
  shared snapshot.
- Updated BLE status encoding to combine BLE runtime state, latest
  network/upload state, pending record count, and error flags.
- Refreshed the BLE status characteristic before status reads and on BLE
  runtime state transitions.
- Preserved the Wi-Fi upload path, flash format, measurement JSON payload
  shape, and storage ACK semantics.
- Updated [00-development-plan.md](00-development-plan.md),
  [../10-firmware/00-architecture.md](../10-firmware/00-architecture.md), and
  [../10-firmware/05-ble.md](../10-firmware/05-ble.md) to record Phase 24H as
  runtime status snapshot wiring, not full BLE upload completion.

Validation commands run from the repository root:

```bash
cargo fmt
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf --features ble-upload,radio-coex
cargo clippy --target riscv32imc-unknown-none-elf --features ble-upload,radio-coex
cargo build --target riscv32imc-unknown-none-elf
cargo clippy --target riscv32imc-unknown-none-elf
cargo clippy --all-targets
cargo build --target riscv32imc-unknown-none-elf --no-default-features --features ble-upload
cargo build --target riscv32imc-unknown-none-elf --no-default-features
cargo clippy --target riscv32imc-unknown-none-elf --no-default-features --features ble-upload
cargo clippy --target riscv32imc-unknown-none-elf --no-default-features
git diff --check
```

Observed results:

- `cargo test --lib` passed 148 hardware-independent tests.
- Host/all-target Clippy passed.
- Default ESP32-C3 target build and Clippy passed with default `wifi-upload`.
- No-radio ESP32-C3 target build and Clippy passed with
  `--no-default-features`.
- BLE-only ESP32-C3 target build and Clippy passed with
  `--no-default-features --features ble-upload`.
- BLE+Wi-Fi coexistence ESP32-C3 target build and Clippy passed with
  `--features ble-upload,radio-coex`.
- `git diff --check` passed.

Notes:

- BLE hardware behavior was not tested in this milestone: no live advertising
  scan, no central connection, no real pairing/security validation, no live GATT
  read/write/notify validation, no BLE record transfer, and no BLE storage-drain
  validation.
- BOOT / IO9 was not hardware-validated in this milestone.
- No ESP32-C3 hardware validation was run.
- No firmware flashing was performed.
- No firmware flash-write range was exercised.

## Milestone 34: BLE Advertising Runtime Bring-Up

Phase 24I implementation:

- Fixed the hardware-observed BLE advertising startup failure where the scan
  response tried to carry both the 128-bit project service UUID and complete
  local name, exceeding the 31-byte legacy BLE payload limit.
- Moved the project 128-bit service UUID into the advertising payload with the
  BLE flags and kept the complete local name in the scan response payload.
- Added a hardware-independent regression test that keeps the advertising and
  scan response payloads under the 31-byte limit and documents that the
  previous combined scan response shape was too large.
- Preserved the project GATT service shape, Wi-Fi upload path, flash format,
  measurement JSON payload shape, and storage ACK semantics.
- Updated [00-development-plan.md](00-development-plan.md),
  [../10-firmware/00-architecture.md](../10-firmware/00-architecture.md), and
  [../10-firmware/05-ble.md](../10-firmware/05-ble.md) to record Phase 24I as
  board-side advertising runtime bring-up, not full BLE upload completion.

Validation commands run from the repository root:

```bash
cargo fmt
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf
cargo build --target riscv32imc-unknown-none-elf --features ble-upload,radio-coex
cargo clippy --all-targets
cargo clippy --target riscv32imc-unknown-none-elf --features ble-upload,radio-coex
cargo espflash save-image --chip esp32c3 --flash-size 4mb --target riscv32imc-unknown-none-elf --features ble-upload,radio-coex --merge /tmp/phase24-ble-fixed-image.bin
cargo espflash flash --target riscv32imc-unknown-none-elf --features ble-upload,radio-coex --chip esp32c3 --port /dev/ttyACM0 --before usb-reset --non-interactive --flash-size 4mb
probe-rs reset --chip esp32c3
timeout 45s probe-rs attach --chip esp32c3 --rtt-scan-memory target/riscv32imc-unknown-none-elf/debug/sleep-environment-monitor
```

Observed results:

- `cargo test --lib` passed 149 hardware-independent tests.
- Default ESP32-C3 target build passed with default `wifi-upload`.
- BLE+Wi-Fi coexistence ESP32-C3 target build and Clippy passed with
  `--features ble-upload,radio-coex`.
- Host/all-target Clippy passed.
- Before the fix, RTT showed the BLE task reached controller initialization
  and then stopped the GATT loop with
  `ble scan response encode failed error=InsufficientSpace`.
- The fixed BLE+Wi-Fi image reported `App/part. size:
  910,048/4,128,768 bytes, 22.04%`.
- The fixed image was flashed successfully to an ESP32-C3 rev v0.4 with 4MB
  flash and MAC address `8c:bf:ea:44:f7:3c`; `espflash` reported
  `Features: WiFi, BLE` and `Flashing has completed!`.
- After flashing and reset, RTT showed:
  - `ble controller initialized name=sleep-env-esp32c3 protocol_version=1`
  - `ble advertising name=sleep-env-esp32c3 protocol_version=1`
  - sensor, microphone, aggregation, and storage tasks continued running while
    BLE advertising was active.

Flash range declared before flashing:

- Bootloader: `0x00000000..0x00008000`.
- Partition table: `0x00008000..0x00009000`.
- Factory app: `0x00010000..0x000ee2e0`; sector erase may cover through
  `0x000ef000`.
- No active flash erase/write validation targeted the measurement spool range
  `0x003c0000..0x00400000`. Normal firmware runtime storage continued to use
  that spool region during validation.

Notes:

- Board-side advertising startup is validated by RTT logs, but central-side
  discovery was not accepted as complete in this milestone. A Windows BLE
  watcher scan returned `NOT_FOUND target=sleep-env-esp32c3 seconds=30`, and an
  all-device watcher scan returned `SEEN count=0 seconds=15`, so the central
  scan path itself still needs investigation.
- No BLE central connection was validated.
- No structured status read, pairing/security validation, live record transfer,
  notification flow, BLE storage ACK, or BLE storage drain was validated.
- BOOT / IO9 runtime input and download-mode behavior were not
  hardware-validated in this milestone.

## Milestone 35: BLE Central Status And Closed-Window Authorization

Phase 24J validation:

- Reused the BLE+Wi-Fi coexistence firmware flashed in Milestone 34; no new
  firmware image was built for flashing and no firmware flashing command was
  run in this milestone.
- Built the temporary Windows/.NET BLE central validation tool from
  `/tmp/ble-watch`.
- Confirmed a Windows BLE central can discover the ESP32-C3 by the project
  service UUID in the connectable advertisement and by the
  `sleep-env-esp32c3` scan-response local name.
- Confirmed the central can connect, discover the project GATT service, find
  the status characteristic, and read the Phase 24H structured status frame.
- Confirmed the closed BOOT / IO9 pairing window rejects measurement metadata
  reads, fragment reads, and control writes with ATT authorization errors.
- Updated [00-development-plan.md](00-development-plan.md),
  [../10-firmware/00-architecture.md](../10-firmware/00-architecture.md), and
  [../10-firmware/05-ble.md](../10-firmware/05-ble.md) to record Phase 24J as
  central-side status and closed-window authorization validation, not full BLE
  upload completion.

Validation commands run from the repository root:

```bash
'/mnt/c/Program Files/dotnet/dotnet.exe' build '\\wsl.localhost\archlinux\tmp\ble-watch\ble-watch.csproj'
'/mnt/c/Program Files/dotnet/dotnet.exe' '\\wsl.localhost\archlinux\tmp\ble-watch\bin\Debug\net10.0-windows10.0.19041.0\ble-watch.dll' scan-read-status 30 sleep-env-esp32c3
'/mnt/c/Program Files/dotnet/dotnet.exe' '\\wsl.localhost\archlinux\tmp\ble-watch\bin\Debug\net10.0-windows10.0.19041.0\ble-watch.dll' scan-closed-window 30 sleep-env-esp32c3
```

Observed central status read:

- The scan found address `0xf3531024e2c3` with random address type.
- The connectable advertisement contained project service UUID
  `0100017d-0e51-b29b-1e4b-0a24534d4553` as observed by Windows.
- The scan response contained local name `sleep-env-esp32c3`.
- GATT service discovery returned `Success`.
- Status characteristic discovery returned `Success` with properties
  `Read, Notify`.
- Status read returned `Success`.
- Status bytes were `01040002200000000000`.
- Decoded status: protocol version `1`, BLE runtime `Connected`, network
  `Disconnected`, upload `Failed`, pending records `32`, error flags
  `0x00000000`.

Observed closed-window authorization result:

- Metadata characteristic lookup returned `Success`.
- Fragment characteristic lookup returned `Success`.
- Control characteristic lookup returned `Success`.
- Metadata read failed with `ProtocolError`, ATT protocol error `0x08`.
- Fragment read failed with `ProtocolError`, ATT protocol error `0x08`.
- Control write failed with exception `0x80650008`.
- The validation tool reported
  `metadata_rejected=True fragment_rejected=True control_rejected=True`.

Notes:

- This milestone did not validate authorized record transfer, notification
  behavior, BLE storage ACK, Wi-Fi/BLE ACK race behavior, BLE storage drain, or
  BOOT / IO9 runtime button entry.
- No firmware flashing was performed.
- No firmware flash-write range was exercised.

## Milestone 36: BLE Pairing Entry Diagnostics

Phase 24K validation:

- Extended the BLE status frame from 10 bytes to 20 bytes while preserving the
  original 10-byte prefix used by Phase 24H and Phase 24J.
- Added central-readable diagnostics for BOOT / IO9 pairing entry:
  - pairing state
  - BOOT / IO9 button state
  - pairing-window remaining milliseconds
  - accumulated BOOT press milliseconds
- Built and flashed a BLE+Wi-Fi coexistence diagnostic image.
- Built the temporary Windows/.NET BLE central validation tool from
  `/tmp/ble-watch`.
- Confirmed a Windows BLE central can read the 20-byte status frame.
- Confirmed BOOT / IO9 is read as an active-low runtime input and that a long
  press opens the pairing window.
- Confirmed the existing no-retrigger rule: after the pairing window expires,
  the same continuous press does not reopen the window until BOOT / IO9 is
  released and pressed again.
- Updated [00-development-plan.md](00-development-plan.md),
  [../10-firmware/00-architecture.md](../10-firmware/00-architecture.md), and
  [../10-firmware/05-ble.md](../10-firmware/05-ble.md) to record Phase 24K as
  pairing-entry diagnostics, not full BLE upload completion.

Validation commands run from the repository root:

```bash
cargo fmt
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf --features ble-upload,radio-coex
'/mnt/c/Program Files/dotnet/dotnet.exe' build '\\wsl.localhost\archlinux\tmp\ble-watch\ble-watch.csproj'
cargo espflash save-image --chip esp32c3 --flash-size 4mb --target riscv32imc-unknown-none-elf --features ble-upload,radio-coex --merge /tmp/phase24-ble-status-pressed-image.bin
cargo espflash flash --target riscv32imc-unknown-none-elf --features ble-upload,radio-coex --chip esp32c3 --port /dev/ttyACM0 --before usb-reset --non-interactive --flash-size 4mb
'/mnt/c/Program Files/dotnet/dotnet.exe' '\\wsl.localhost\archlinux\tmp\ble-watch\bin\Debug\net10.0-windows10.0.19041.0\ble-watch.dll' scan-watch-status 30 sleep-env-esp32c3 60
'/mnt/c/Program Files/dotnet/dotnet.exe' '\\wsl.localhost\archlinux\tmp\ble-watch\bin\Debug\net10.0-windows10.0.19041.0\ble-watch.dll' scan-transfer-record 30 sleep-env-esp32c3 no-ack 128
'/mnt/c/Program Files/dotnet/dotnet.exe' '\\wsl.localhost\archlinux\tmp\ble-watch\bin\Debug\net10.0-windows10.0.19041.0\ble-watch.dll' scan-read-status 30 sleep-env-esp32c3
cargo build --target riscv32imc-unknown-none-elf
cargo clippy --all-targets
cargo clippy --target riscv32imc-unknown-none-elf
cargo clippy --target riscv32imc-unknown-none-elf --features ble-upload,radio-coex
```

Observed build results:

- `cargo test --lib` passed 150 hardware-independent tests.
- BLE+Wi-Fi coexistence ESP32-C3 target build passed with
  `--features ble-upload,radio-coex`.
- Default ESP32-C3 target build passed with default `wifi-upload`.
- Host/all-target Clippy passed.
- Default ESP32-C3 target Clippy passed.
- BLE+Wi-Fi coexistence ESP32-C3 target Clippy passed with
  `--features ble-upload,radio-coex`.
- The temporary Windows/.NET BLE central validation tool built successfully.

Flash range declared before flashing:

- Bootloader: `0x00000000..0x00008000`.
- Partition table: `0x00008000..0x00009000`.
- Factory app: `0x00010000..0x000ee510`; sector erase may cover through
  `0x000ef000`.
- No deliberate erase/write targeted the measurement spool range
  `0x003c0000..0x00400000`. The normal firmware runtime storage path may still
  use that spool region.

Observed flash result:

- The diagnostic image reported `App/part. size:
  910,608/4,128,768 bytes, 22.06%`.
- `espflash` identified an ESP32-C3 rev v0.4 with 4MB flash and MAC address
  `8c:bf:ea:44:f7:3c`.
- `espflash` reported `Features: WiFi, BLE` and `Flashing has completed!`.

Observed 20-byte status reads:

- Initial status after diagnostic flash decoded as runtime `Connected`,
  network `Disconnected`, upload `Failed`, pending records `32`, error flags
  `0x00000000`, pairing `Closed`, BOOT / IO9 `Released`, remaining `0 ms`,
  pressed `0 ms`.
- During a BOOT / IO9 long press, status read index 32 reported pairing
  `Closed`, BOOT / IO9 `Pressed`, remaining `0 ms`, pressed `800 ms`.
- Status read index 33 reported pairing `Open`, BOOT / IO9 `Pressed`,
  remaining `59950 ms`, pressed `2050 ms`.
- Subsequent reads showed pairing `Open`, decreasing remaining time, and
  increasing pressed time, confirming the hardware input and state machine
  crossed the configured 2-second threshold.
- Later reads after the pairing window had expired showed BOOT / IO9 still
  `Pressed` with a large accumulated pressed time and pairing `Closed`,
  confirming the expected no-retrigger-until-release behavior.

Notes:

- Authorized BLE record transfer was attempted with `scan-transfer-record ... no-ack 128`
  but did not complete. The tool repeatedly observed authorization errors or
  timed out waiting for `pairing=Open` because the board continued reporting
  BOOT / IO9 as `Pressed` after the previous pairing window had expired. No
  successful metadata read, fragment transfer, CRC-validated payload, or
  `CompleteRecord` acceptance was recorded in this milestone.
- The attempted `no-ack` transfer did not request BLE storage ACK and did not
  validate BLE storage drain.
- BLE notification behavior, BLE storage ACK, Wi-Fi/BLE ACK race behavior,
  disconnect preservation during live transfer, and BOOT download-mode
  preservation remain unvalidated.

## Milestone 37: BLE Record Transfer And ACK Path Validation

Phase 24L validation:

- Reused the BLE+Wi-Fi coexistence diagnostic firmware flashed in Milestone
  36; no new firmware was flashed for this validation slice.
- Moved the Windows/.NET BLE central validation tool into the repository at
  [../../tools/ble-watch/](../../tools/ble-watch/).
- Confirmed an authorized `scan-transfer-record ... no-ack 128` run can read
  metadata, read all ordered fragments, validate the payload CRC, and send
  `CompleteRecord` without requesting BLE storage ACK.
- Confirmed an authorized `scan-transfer-record ... ack 128` run can read
  metadata, read all ordered fragments, validate the payload CRC, send
  `CompleteRecord`, and then send `AckRecord` while Wi-Fi upload is
  unavailable.
- Updated [00-development-plan.md](00-development-plan.md),
  [../10-firmware/00-architecture.md](../10-firmware/00-architecture.md),
  [../10-firmware/05-ble.md](../10-firmware/05-ble.md), and
  [../../tools/ble-watch/README.md](../../tools/ble-watch/README.md)
  to record Phase 24L as record-transfer and ACK-path validation, not full BLE
  upload completion.

Validation commands run from the repository root:

```bash
'/mnt/c/Program Files/dotnet/dotnet.exe' build "$(wslpath -w tools/ble-watch/ble-watch.csproj)"
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-transfer-record 30 sleep-env-esp32c3 no-ack 128
# Declared measurement spool range 0x003c0000..0x00400000 before this command.
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-transfer-record 30 sleep-env-esp32c3 ack 128
probe-rs reset --chip esp32c3
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-read-status 30 sleep-env-esp32c3
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-watch-status 30 sleep-env-esp32c3 60
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-transfer-record 30 sleep-env-esp32c3 no-ack 128
```

Observed BLE transfer results:

- The no-ACK transfer read sequence `100291`, payload length `202`, and CRC
  `0x09fe19ba`; the computed payload CRC matched the metadata CRC.
- The no-ACK transfer completed successfully with `CompleteRecord`,
  `ack_requested=False`, and no BLE storage ACK.
- The ACK-mode transfer read sequence `101350`, payload length `203`, and CRC
  `0x1f882667`; the computed payload CRC matched the metadata CRC.
- The ACK-mode transfer completed successfully with `CompleteRecord`,
  `AckRecord`, and `ack_requested=True`.

Flash range declared before ACK-mode validation:

- Measurement spool: `0x003c0000..0x00400000`.
- No firmware image was flashed in this milestone, so the bootloader,
  partition table, and factory app ranges were not deliberately written by the
  validation commands.
- The ACK-mode run may have exercised normal firmware measurement spool
  writes/erases through `storage_task` in the declared spool range.

Observed post-ACK recheck:

- A post-ACK no-ACK transfer recheck was attempted to confirm the oldest
  sequence advanced after ACK.
- A non-flashing `probe-rs reset --chip esp32c3` restored BLE status to
  pairing `Closed`, BOOT / IO9 `Released`, and `pressed_ms=0`.
- A `scan-watch-status` run confirmed BOOT / IO9 could still be observed as
  `Pressed`, opened the pairing window, and reached about `59,400 ms`
  remaining.
- Two later `scan-transfer-record ... no-ack 128` attempts timed out waiting
  for `pairing=Open`; during the transfer waits the central repeatedly read
  pairing `Closed`, BOOT / IO9 `Released`, and `pressed_ms=0`.
- No metadata read, fragment read, `CompleteRecord`, or BLE ACK occurred during
  those post-ACK recheck attempts.
- Therefore, post-ACK oldest-record advancement remained unvalidated in this
  milestone. Milestone 44 / Phase 24P later validated post-ACK oldest-record
  advancement with `scan-ack-then-peek-next`.

Notes:

- Current BLE authorization is not persisted. It uses a RAM-only BOOT / IO9
  authorization window and does not save bonded peers, pairing keys, allowlists,
  or other pairing records in firmware flash. Future Phase 24 work must define
  and validate real BLE bonding or an equivalent persistent authorization
  record, including storage location, update rules, and user-controlled
  clearing.
- BLE notification behavior remains unvalidated; this milestone used explicit
  characteristic reads for metadata and fragments.
- Wi-Fi/BLE ACK race behavior remains unvalidated.
- Disconnect preservation during live transfer remains unvalidated.
- BOOT / IO9 download-mode preservation remains unvalidated.
- Phase 24 still has remaining hardware validation work and should not be
  treated as fully complete.

## Milestone 41: Phase 24M BLE Fragment Notification Validation

Phase 24M validation:

- Renamed the Windows/.NET BLE central validation tool to
  [../../tools/ble-watch/](../../tools/ble-watch/).
- Added `scan-transfer-record-notify`, `scan-disconnect-preserves-record`, and
  `scan-ack-then-peek-next` commands to the tool.
- Added an early diagnostic for the expired-held BOOT / IO9 state:
  `PAIRING_HELD_AFTER_EXPIRED`.
- Confirmed `scan-transfer-record-notify ... no-ack 128` can subscribe to
  record-fragment notifications, request metadata and ordered fragments,
  observe one notification per requested fragment, confirm each notification
  matches the corresponding fragment read, validate payload CRC, and send
  `CompleteRecord` without requesting BLE storage ACK.
- No firmware image was flashed for this milestone.

Validation commands run from the repository root:

```bash
'/mnt/c/Program Files/dotnet/dotnet.exe' build '\\wsl.localhost\archlinux\home\dreamonex\sleep-environment-monitor\tools\ble-watch\ble-watch.csproj'
'/mnt/c/Program Files/dotnet/dotnet.exe' '\\wsl.localhost\archlinux\home\dreamonex\sleep-environment-monitor\tools\ble-watch\bin\Debug\net10.0-windows10.0.19041.0\ble-watch.dll' scan-read-status 30 sleep-env-esp32c3
'/mnt/c/Program Files/dotnet/dotnet.exe' '\\wsl.localhost\archlinux\home\dreamonex\sleep-environment-monitor\tools\ble-watch\bin\Debug\net10.0-windows10.0.19041.0\ble-watch.dll' scan-transfer-record-notify 30 sleep-env-esp32c3 no-ack 128
'/mnt/c/Program Files/dotnet/dotnet.exe' '\\wsl.localhost\archlinux\home\dreamonex\sleep-environment-monitor\tools\ble-watch\bin\Debug\net10.0-windows10.0.19041.0\ble-watch.dll' scan-disconnect-preserves-record 30 sleep-env-esp32c3 128
'/mnt/c/Program Files/dotnet/dotnet.exe' build '\\wsl.localhost\archlinux\home\dreamonex\sleep-environment-monitor\tools\ble-watch\ble-watch.csproj'
```

Observed BLE notification result:

- The status check discovered `sleep-env-esp32c3`, connected to the project
  GATT service, read a 20-byte status frame, and decoded runtime `Connected`,
  network `Disconnected`, upload `Failed`, pending `32`, error flags
  `0x00000000`, pairing `Closed`, BOOT / IO9 `Pressed`, remaining `0 ms`, and
  `pressed_ms=1800`.
- The notification transfer subscribed successfully with
  `NOTIFY_TRANSFER_NOTIFY_SUBSCRIBE status=Success`.
- The authorized transfer read sequence `104145`, payload length `206`, and
  CRC `0x904c92fc`.
- The central observed two fragment notifications: offset `0`, length `128`;
  and offset `128`, length `78`.
- Both notifications matched the corresponding explicit fragment reads.
- The computed payload CRC was `0x904c92fc`, matching metadata.
- The transfer completed successfully with `CompleteRecord`,
  `ack_requested=False`, and no BLE storage ACK.

Observed disconnect-preservation attempt:

- `scan-disconnect-preserves-record 30 sleep-env-esp32c3 128` did not reach
  metadata or fragment access.
- During the pairing wait, the board continuously reported pairing `Closed`,
  BOOT / IO9 `Pressed`, remaining `0 ms`, and an increasing `pressed_ms`,
  ending at `157650 ms`.
- The command timed out with `PAIRING_TIMEOUT attempts=151`.
- A follow-up status read still reported pairing `Closed`, BOOT / IO9
  `Pressed`, remaining `0 ms`, and `pressed_ms=200500`.
- Therefore, disconnect preservation remained unvalidated in this milestone.
  This attempt only confirms the already documented no-retrigger behavior after
  an expired continuous BOOT / IO9 press. Milestone 44 / Phase 24P later
  validated disconnect-before-Complete/ACK preservation after a drain
  precondition.

Notes:

- No ACK-mode validation was run in this milestone, so no deliberate
  measurement-spool ACK or firmware flash-write range was exercised.
- Wi-Fi/BLE ACK race behavior remains unvalidated.
- Disconnect preservation during live transfer remains unvalidated.
- Post-ACK oldest-record advancement remains unvalidated.
- BOOT / IO9 download-mode preservation remains unvalidated.
- Phase 24 still has remaining hardware validation work and should not be
  treated as fully complete.

## Milestone 42: Phase 24N Wi-Fi/BLE ACK Race Logic Guard

Phase 24N implementation:

- Added a storage unit test for the Wi-Fi/BLE ACK race guard.
- The test models Wi-Fi acknowledging the current oldest record, then BLE
  attempting to acknowledge the same stale sequence.
- The expected result is that the stale BLE ACK returns no acknowledgement and
  leaves the next oldest record pending.
- This is hardware-independent coverage only. It does not validate live
  Wi-Fi/BLE runtime race behavior on the ESP32-C3.

Validation commands run from the repository root:

```bash
cargo fmt
cargo test --lib
```

Observed test result:

- `cargo test --lib` passed with `151 passed; 0 failed`.
- New test:
  `tasks::storage::tests::ble_ack_after_wifi_ack_does_not_remove_next_oldest_record`.

Notes:

- No firmware image was flashed for this milestone.
- No ACK-mode BLE hardware validation was run in this milestone, so no
  deliberate measurement-spool ACK or firmware flash-write range was exercised.
- Live Wi-Fi/BLE ACK race behavior remains unvalidated.
- Disconnect preservation during live transfer remains unvalidated.
- Post-ACK oldest-record advancement remains unvalidated.
- BOOT / IO9 download-mode preservation remains unvalidated.
- Phase 24 still has remaining hardware validation work and should not be
  treated as fully complete.

## Milestone 43: Phase 24O BLE Auth Metadata Auto-Pair Policy

Phase 24O implementation:

- Reserved the BLE authorization metadata sector
  `0x003bf000..0x003c0000` immediately before the measurement spool.
- Kept the measurement spool at `0x003c0000..0x00400000` and did not change the
  spool record format or measurement JSON payload shape.
- Added `storage::ble_auth` for the future BLE authorization metadata header:
  magic, format version, header length, authorization-record-set version,
  record count, record-set checksum, and header checksum.
- Added hardware-independent tests for erased/missing metadata, empty current
  headers, version mismatch, record-set checksum mismatch, header checksum
  mismatch, valid headers, and the config switch disabling auto-open.
- Added config constants for BLE authorization record version/checksum and the
  auto-pair-on-auth-record-reset switch.
- In `ble-upload` target builds, startup reads the auth metadata header and can
  open the RAM-only BOOT / IO9 authorization window when policy requires it.
- Kept Wi-Fi upload code included in BLE+coexistence builds.
- Moved the Wi-Fi ESP radio authentication adapter to the Wi-Fi use site and
  tightened Wi-Fi credential validation around byte limits and 64-byte hex
  PSKs.
- Updated Phase 24 documentation, architecture/config notes, and handoff notes.

Validation commands run from the repository root:

```bash
cargo fmt
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf
cargo build --target riscv32imc-unknown-none-elf --features ble-upload,radio-coex
cargo clippy --all-targets
cargo clippy --target riscv32imc-unknown-none-elf
cargo clippy --target riscv32imc-unknown-none-elf --features ble-upload,radio-coex
git diff --check
```

Observed validation results:

- `cargo test --lib` passed with `168 passed; 0 failed`.
- The normal ESP32-C3 target build passed.
- The BLE+Wi-Fi coexistence ESP32-C3 target build passed.
- Host clippy, normal target clippy, and BLE+coex target clippy all passed.
- `git diff --check` passed.

Flash and hardware notes:

- No firmware image was flashed for this milestone.
- No hardware BLE central, pairing, GATT read/write/notify, or transfer
  validation was run for this milestone.
- No deliberate BLE auth metadata sector write or erase was exercised. The
  Phase 24O target code reads only the header in
  `0x003bf000..0x003c0000`.
- No deliberate measurement-spool write/erase or BLE ACK hardware validation
  was exercised in this milestone. The measurement spool range remains
  `0x003c0000..0x00400000`.
- Phase 24O does not persist real BLE bonded peers, pairing keys, allowlists,
  authorization records, or user-controlled clearing. The current effective
  authorization state remains RAM-only.

Remaining Phase 24 validation:

- Live Wi-Fi/BLE ACK race behavior remains unvalidated.
- Disconnect preservation during live transfer remains unvalidated.
- Post-ACK oldest-record advancement remains unvalidated.
- BLE auth metadata write/erase/update behavior remains future work.
- Real persisted BLE bonding or equivalent authorization records remain future
  work.
- BOOT / IO9 download-mode preservation remains unvalidated.

## Milestone 44: Phase 24P BLE Disconnect Preservation And LED Status Boundary

Phase 24P validation and implementation:

- Added `scan-drain-then-disconnect-preserves-record` to the Windows BLE
  central validation tool in [../../tools/ble-watch/](../../tools/ble-watch/).
- Confirmed `scan-ack-then-peek-next` can ACK one BLE-transferred oldest record
  and then reconnect to observe the next oldest record.
- Confirmed disconnect before `CompleteRecord` or `AckRecord` preserves the
  same oldest pending record across reconnect after first draining enough
  records to avoid full-spool drop-oldest interference.
- Corrected LED hardware facts: LED1 is the green power indicator tied to the
  3.3 V rail and is not MCU-controlled; LED2 is the red active-low LED on IO0;
  LED3 is the blue active-low LED on IO1.
- Kept LED2 as heartbeat and added a short boot/reset fast-flash boundary.
- Routed normal firmware status to blue LED3 and added a BLE status overlay
  boundary for pairing-window fast blink, advertising/connected slow blink,
  the 180 second boot BLE status window, and the 10 second BOOT / IO9-triggered
  BLE status window.
- Documented that pairing records are not persisted yet. Current authorization
  remains RAM-only; future work must define real BLE bonding or equivalent
  persistent authorization records, record contents, write/erase/update rules,
  version/checksum migration behavior, and user-controlled clearing.

Validation commands run from the repository root:

```bash
'/mnt/c/Program Files/dotnet/dotnet.exe' build '\\wsl.localhost\archlinux\home\dreamonex\sleep-environment-monitor\tools\ble-watch\ble-watch.csproj'
'/mnt/c/Program Files/dotnet/dotnet.exe' '\\wsl.localhost\archlinux\home\dreamonex\sleep-environment-monitor\tools\ble-watch\bin\Debug\net10.0-windows10.0.19041.0\ble-watch.dll' scan-ack-then-peek-next 30 sleep-env-esp32c3 128
'/mnt/c/Program Files/dotnet/dotnet.exe' '\\wsl.localhost\archlinux\home\dreamonex\sleep-environment-monitor\tools\ble-watch\bin\Debug\net10.0-windows10.0.19041.0\ble-watch.dll' scan-drain-then-disconnect-preserves-record 30 sleep-env-esp32c3 128 25 40 8
cargo fmt
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf
cargo build --target riscv32imc-unknown-none-elf --features ble-upload,radio-coex
```

Observed post-ACK oldest advancement:

- Flash range declared before validation: measurement spool
  `0x003c0000..0x00400000`.
- Command: `scan-ack-then-peek-next 30 sleep-env-esp32c3 128`.
- ACKed sequence `108009`.
- Reconnected oldest sequence was `108010`.
- Result: `success=True`.

Observed disconnect preservation after drain:

- Flash range declared before validation: measurement spool
  `0x003c0000..0x00400000`.
- The user held BOOT / IO9 for about 4 seconds to open the authorization
  window.
- Command:
  `scan-drain-then-disconnect-preserves-record 30 sleep-env-esp32c3 128 25 40 8`.
- Drain phase ACKed 40 records, from sequence `109090` through `109129`.
- Target pending after drain was 25; final pending was 24.
- The first partial read after drain observed sequence `109130`, payload length
  `199`, then disconnected before `CompleteRecord` or `AckRecord`.
- Reconnect metadata again reported sequence `109130`, payload length `199`.
- Result:
  `DRAIN_THEN_DISCONNECT_RESULT success=True ... first_sequence=109130 second_sequence=109130`.

Flash and hardware notes:

- No firmware image was flashed for this milestone.
- The BLE ACK and drain validations may have exercised normal measurement
  spool writes/erases through `storage_task` in `0x003c0000..0x00400000`.
- The BLE auth metadata sector `0x003bf000..0x003c0000` was not deliberately
  written or erased.
- LED3 BLE pattern logic is compile/unit validated only in this milestone;
  actual blue LED visual behavior has not been manually accepted on hardware.

Remaining Phase 24 validation:

- Live Wi-Fi/BLE ACK race behavior remains unvalidated on hardware.
- BOOT / IO9 download-mode preservation remains unvalidated.
- BLE auth metadata write/erase/update behavior remains future work.
- Real persisted BLE bonding or equivalent authorization records remain future
  work.
- LED3 BLE hardware visual behavior remains unvalidated.

## Milestone 45: Phase 24Q BLE Auth Persistence Compile Path

Phase 24Q implementation:

- Enabled TrouBLE security support in the `ble-upload` feature path and added a
  BLE security seed sourced from ESP32-C3 TRNG during firmware startup.
- Added BLE config constants for authorization record capacity and security
  seed length.
- Extended `storage::ble_auth` from header-only metadata into a structured BLE
  authorization record set with identity address, LTK, optional IRK, security
  level, bonded flag, record CRC, record-set checksum, version policy, and
  load/store/clear helpers.
- Added a target compile path that restores TrouBLE bond information from
  `0x003bf000..0x003c0000`, requests security when a pairing window or saved
  auth exists, requires encrypted matching saved auth outside the BOOT / IO9
  authorization window, and stores a bond record on `PairingComplete`.
- Kept Wi-Fi upload code included in BLE+coexistence builds.
- Updated BLE, architecture, configuration, network, integration, development
  plan, and handoff docs to record the saved-record compile path and the
  remaining hardware validation gaps.

Validation commands run from the repository root:

```bash
cargo fmt
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf
cargo build --target riscv32imc-unknown-none-elf --features ble-upload,radio-coex
cargo clippy --all-targets
cargo clippy --target riscv32imc-unknown-none-elf
cargo clippy --target riscv32imc-unknown-none-elf --features ble-upload,radio-coex
git diff --check
```

Observed validation results:

- `cargo test --lib` passed with `181 passed; 0 failed`.
- The normal ESP32-C3 target build passed.
- The BLE+Wi-Fi coexistence ESP32-C3 target build passed.
- Host clippy, normal target clippy, and BLE+coex target clippy passed.
- `git diff --check` passed.

Flash and hardware notes:

- No firmware image was flashed for this milestone.
- No hardware BLE central, pairing/security, GATT transfer, reboot restore, or
  saved-pairing validation was run for this milestone.
- No deliberate BLE auth sector write or erase was exercised. Target code can
  write or erase `0x003bf000..0x003c0000` after
  `PairingComplete { bond: Some(..) }` in BLE-enabled firmware, but this
  compile-validation milestone did not trigger that path.
- No deliberate measurement-spool write/erase or BLE ACK hardware validation
  was exercised in this milestone. The measurement spool range remains
  `0x003c0000..0x00400000`.
- Saved pairing records now have a compile-validated target path, but they are
  not accepted as hardware-validated behavior until real pairing, auth-sector
  write/erase/update, reboot restore, version/checksum reset, and user clearing
  are tested.

Remaining Phase 24 validation:

- Live Wi-Fi/BLE ACK race behavior remains unvalidated on hardware.
- BOOT / IO9 download-mode preservation remains unvalidated.
- Real BLE pairing, saved bond restore across reboot, and rejected
  unauthorized/unencrypted access remain unvalidated on hardware.
- BLE auth metadata write/erase/update behavior, version/checksum reset
  behavior, automatic pairing-window opening after auth-record reset, and user
  clearing remain unvalidated.
- LED3 BLE hardware visual behavior remains unvalidated.

## Milestone 46: Phase 24R BLE Saved-Bond Restore Hardware Validation

Phase 24R implementation and validation:

- Added `scan-read-metadata-now` to [../../tools/ble-watch/](../../tools/ble-watch/)
  for protected metadata access without waiting for the BOOT / IO9 temporary
  authorization window.
- Added Windows Custom ConfirmOnly pairing support, Windows pairing-state
  logging, and `no-pair` mode to the BLE watch tool.
- Added a runtime BOOT / IO9 saved-auth clearing gesture: about 2 seconds opens
  the temporary authorization window, and about 8 seconds requests clearing the
  BLE auth sector and reopens the window.
- Changed BLE security startup so the firmware requests security proactively
  only while the pairing window is open. Saved-bond reconnects rely on
  encrypted measurement characteristics to trigger link encryption.
- Kept LED facts documented: LED1 is the green 3.3 V power LED and is not
  MCU-controlled; LED2 is the red heartbeat LED on IO0; LED3 is the blue
  status/BLE LED on IO1.

Validation commands run from the repository root:

```bash
cargo fmt
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf
cargo build --target riscv32imc-unknown-none-elf --features ble-upload,radio-coex
cargo clippy --all-targets
cargo clippy --target riscv32imc-unknown-none-elf
cargo clippy --target riscv32imc-unknown-none-elf --features ble-upload,radio-coex
'/mnt/c/Program Files/dotnet/dotnet.exe' build "$(wslpath -w tools/ble-watch/ble-watch.csproj)"
git diff --check
```

Observed validation results:

- `cargo test --lib` passed with `182 passed; 0 failed`.
- The normal ESP32-C3 target build passed.
- The BLE+Wi-Fi coexistence ESP32-C3 target build passed.
- Host clippy, normal target clippy, and BLE+coex target clippy passed.
- The Windows .NET `tools/ble-watch` build passed.
- `git diff --check` passed.

Hardware validation setup and flash ranges:

- Firmware was flashed with the BLE+Wi-Fi build through `probe-rs` JTAG using
  the ESP JTAG device.
- Declared flash ranges before flashing and validation:
  - app firmware region: approximately `0x00010000..0x003bf000`;
  - BLE auth metadata sector: `0x003bf000..0x003c0000`;
  - measurement spool: `0x003c0000..0x00400000`.
- The successful pairing path deliberately exercised a BLE auth-sector write
  in `0x003bf000..0x003c0000`.
- The runtime 8 second clear gesture did not exercise the BLE auth-sector erase
  path on hardware in this milestone.
- The measurement spool continued normal runtime appends and drop-oldest
  behavior in `0x003c0000..0x00400000` during the long run.

Observed saved-bond pairing and restore:

- Initial startup reported missing BLE auth records and auto-opened the
  temporary pairing window.
- A first implicit encrypted GATT access without explicit Windows Custom
  Pairing failed before the tool was updated.
- After adding Custom ConfirmOnly pairing, `scan-read-metadata-now 30
  sleep-env-esp32c3 expect-success` paired successfully, wrote the protected
  metadata request successfully, and read metadata successfully.
- Firmware RTT showed `Pairing method JustWorks`, `Link encrypted!`,
  `ble pairing complete security_level=Encrypted bonded=true saved_bonds=1`,
  and `ble auth bond stored count=1 offset=0x003bf000 len=4096`.
- After reboot, startup reported
  `ble auth records restored status=Valid { records_version: 1, record_count: 1 } loaded=1 restored=1`
  and `auto_pair=false`.
- `scan-read-metadata-now 30 sleep-env-esp32c3 expect-success no-pair`
  succeeded with Windows reporting `is_paired=True`, metadata write success,
  metadata read success, and `METADATA_NOW_RESULT success=True`.
- Firmware RTT showed the link encrypting through the saved bond and metadata
  being prepared without requiring a new BOOT / IO9 authorization gesture.

Observed non-passing hardware checks:

- Runtime 8 second BOOT / IO9 saved-auth clearing was implemented but not
  validated. Two `scan-watch-status` runs continued to report
  `boot_button=Released pressed_ms=0`, and firmware RTT did not show the clear
  requested or clear completed logs.
- LED3 hardware visual behavior was not accepted in this milestone. The
  firmware logic and docs still require future visual confirmation of pairing
  fast blink, advertising-or-connected slow blink, the 180 second
  post-boot BLE status window, and the 10 second BOOT / IO9-triggered BLE
  status window.
- BOOT / IO9 download-mode preservation was not validated.
- Rejection after saved-auth clearing was not validated because the clear
  gesture was not observed.
- Version/checksum reset hardware behavior and record replacement behavior were
  not validated.
- Live Wi-Fi/BLE ACK race behavior was not validated. In the BLE+coex runtime,
  Wi-Fi controller initialization failed with `error: 257`, so network and
  uploader tasks were disabled during the observed run.

Remaining Phase 24 validation:

- Validate live Wi-Fi/BLE ACK race behavior on hardware/runtime.
- Validate BOOT / IO9 still enters download mode during reset or power-on.
- Validate BLE auth metadata erase/update behavior beyond the observed first
  bond write, including version/checksum reset behavior, automatic
  pairing-window opening after auth-record reset, record replacement, and user
  clearing.
- Validate rejected unauthorized/unencrypted access after saved-auth clearing.
- Validate phone/gateway interoperability beyond the Windows central.
- Manually accept LED3 hardware visual behavior.

## Milestone 47: Phase 24S Wi-Fi-Unready LED Status Config Boundary

Phase 24S implementation:

- Separated plain Wi-Fi/IP-not-ready indication from explicit network error
  flags in the blue LED3 status policy.
- Added `config::led::WIFI_UNREADY_STATUS_WINDOW_SECS`, with default `0`, so a
  board with local storage does not permanently slow-blink LED3 just because
  Wi-Fi is absent.
- Preserved explicit network fault behavior: `ErrorFlags::WIFI`,
  `ErrorFlags::IP`, and `ErrorFlags::DISCOVERY` still slow-blink LED3 through
  `ErrorFlags::NETWORK_MASK`.
- Documented that `ErrorFlags::NETWORK_MASK` is only for the Wi-Fi/IP/discovery
  REST upload path and does not include BLE advertising, BLE connection, or BLE
  authorization state.
- Moved LED state semantics into
  [../10-firmware/00-architecture.md](../10-firmware/00-architecture.md) and
  kept [../10-firmware/01-hardware.md](../10-firmware/01-hardware.md) limited
  to physical LED wiring and polarity.
- Aligned BLE LED wording to the implemented runtime states:
  pairing/authorization fast blink and advertising-or-connected slow blink.

Validation commands run from the repository root:

```bash
cargo fmt
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf
cargo build --target riscv32imc-unknown-none-elf --features ble-upload,radio-coex
cargo clippy --all-targets
cargo clippy --target riscv32imc-unknown-none-elf
cargo clippy --target riscv32imc-unknown-none-elf --features ble-upload,radio-coex
git diff --check
```

Observed validation results:

- `cargo test --lib` passed with `185 passed; 0 failed`.
- The normal ESP32-C3 target build passed.
- The BLE+Wi-Fi coexistence ESP32-C3 target build passed.
- Host clippy, normal target clippy, and BLE+coex target clippy passed.
- `git diff --check` passed.

Flash and hardware notes:

- No firmware image was flashed for this milestone.
- No hardware BLE central, pairing/security, GATT transfer, LED visual
  acceptance, or BOOT / IO9 validation was run for this milestone.
- No deliberate BLE auth sector write or erase was exercised. The BLE auth
  metadata sector remains `0x003bf000..0x003c0000`.
- No deliberate measurement-spool write or erase was exercised. The measurement
  spool range remains `0x003c0000..0x00400000`.

Remaining Phase 24 validation:

- Runtime 8 second BOOT / IO9 saved-auth clearing remains unvalidated on
  hardware.
- BOOT / IO9 download-mode preservation remains unvalidated.
- Protected-characteristic rejection after the runtime saved-auth clear gesture
  remains unvalidated because that gesture itself remains unvalidated.
- BLE auth record replacement/update and phone/gateway interoperability remain
  unvalidated.
- LED3 BLE hardware visual behavior remains unvalidated.
- Live Wi-Fi/BLE ACK race behavior remains unvalidated.

## Milestone 48: Phase 24T BLE Auth Metadata Reset Hardware Validation

Phase 24T hardware validation:

- Backed up the current BLE auth metadata sector
  `0x003bf000..0x003c0000` to `/tmp/ble-auth-before-phase24t.bin`.
- Deliberately wrote or erased only the BLE auth metadata sector
  `0x003bf000..0x003c0000` using `cargo espflash write-bin` and
  `cargo espflash erase-region`.
- Confirmed auto-opening of the temporary BLE authorization window after
  missing/erased metadata, invalid header magic, an empty current-version record
  set, records-version mismatch, compatibility-checksum mismatch, and header
  checksum mismatch.
- Removed the stale Windows-side pairing record with `scan-unpair` after the
  first invalid-auth run exposed stale central pairing state.
- Confirmed that after the temporary authorization window closed, an unpaired
  central using `scan-read-metadata-now ... expect-reject no-pair` could not
  access protected metadata.

Validation commands run from the repository root:

```bash
cargo espflash read-flash --chip esp32c3 --port /dev/ttyACM0 --before usb-reset --after hard-reset --non-interactive 0x003bf000 0x1000 /tmp/ble-auth-before-phase24t.bin
cargo espflash write-bin --chip esp32c3 --port /dev/ttyACM0 --before usb-reset --after hard-reset --non-interactive 0x003bf000 /tmp/phase24-auth-patterns/badmagic-zero.bin
cargo espflash erase-region --chip esp32c3 --port /dev/ttyACM0 --before usb-reset --after hard-reset --non-interactive 0x003bf000 0x1000
cargo espflash write-bin --chip esp32c3 --port /dev/ttyACM0 --before usb-reset --after hard-reset --non-interactive 0x003bf000 /tmp/phase24-auth-patterns/empty-current.bin
cargo espflash write-bin --chip esp32c3 --port /dev/ttyACM0 --before usb-reset --after hard-reset --non-interactive 0x003bf000 /tmp/phase24-auth-patterns/version-mismatch.bin
cargo espflash write-bin --chip esp32c3 --port /dev/ttyACM0 --before usb-reset --after hard-reset --non-interactive 0x003bf000 /tmp/phase24-auth-patterns/compat-mismatch.bin
cargo espflash write-bin --chip esp32c3 --port /dev/ttyACM0 --before usb-reset --after hard-reset --non-interactive 0x003bf000 /tmp/phase24-auth-patterns/header-checksum-mismatch.bin
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-read-status 30 sleep-env-esp32c3
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-unpair 30 sleep-env-esp32c3
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-read-metadata-now 30 sleep-env-esp32c3 expect-reject no-pair
```

Observed validation results:

- `badmagic-zero.bin`: status read succeeded with
  `pairing=Open boot_button=Released remaining_ms=44100 pressed_ms=0`.
- Missing/erased sector: status read succeeded with
  `pairing=Open boot_button=Released remaining_ms=26850 pressed_ms=0`.
- `empty-current.bin`: status read succeeded with
  `pairing=Open boot_button=Released remaining_ms=33950 pressed_ms=0`.
- `version-mismatch.bin`: status read succeeded with
  `pairing=Open boot_button=Released remaining_ms=34550 pressed_ms=0`.
- `compat-mismatch.bin`: status read succeeded with
  `pairing=Open boot_button=Released remaining_ms=34550 pressed_ms=0`.
- `header-checksum-mismatch.bin`: status read succeeded with
  `pairing=Open boot_button=Released remaining_ms=31800 pressed_ms=0`.
- After waiting for the final temporary authorization window to close, status
  read showed `pairing=Closed boot_button=Released remaining_ms=0
  pressed_ms=0`.
- `scan-read-metadata-now 30 sleep-env-esp32c3 expect-reject no-pair` reported
  `METADATA_NOW_RESULT success=True metadata_success=False rejected=True
  phase=control_write`.

Flash and hardware notes:

- The only deliberate flash-write/erase range was the BLE auth metadata sector
  `0x003bf000..0x003c0000`.
- The measurement spool range `0x003c0000..0x00400000` was not deliberately
  exercised through BLE ACK/drain in this milestone.
- No new firmware image was flashed in this milestone.
- After this milestone, the board may be left with invalid BLE auth metadata
  and no Windows-side pairing record. Rebooting should auto-open the temporary
  authorization window so a new pairing can be created.
- A non-flashing `probe-rs reset --chip esp32c3` cleared a stale runtime status
  where BOOT / IO9 had been reported as pressed for about 43 minutes; after the
  reset, `scan-read-status` reported `boot_button=Released pressed_ms=0`.

Remaining Phase 24 validation:

- Runtime 8 second BOOT / IO9 saved-auth clearing remains unvalidated on
  hardware.
- BOOT / IO9 download-mode preservation remains unvalidated.
- Protected-characteristic rejection after the runtime saved-auth clear gesture
  remains unvalidated because that gesture itself remains unvalidated.
- BLE auth record replacement/update and phone/gateway interoperability remain
  unvalidated.
- LED3 BLE hardware visual behavior remains unvalidated.
- Live Wi-Fi/BLE ACK race behavior remains unvalidated.

## Milestone 49: Phase 24U BLE Watch Windows GATT Recovery Tooling

Phase 24U hardened the Windows `tools/ble-watch` validation tool after WinRT
stale GATT objects repeatedly blocked further Phase 24 hardware validation
with `Unreachable` service, characteristic, or status-read results.

Validation commands run from the repository root:

```bash
'/mnt/c/Program Files/dotnet/dotnet.exe' build "$(wslpath -w tools/ble-watch/ble-watch.csproj)"
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-unpair 30 sleep-env-esp32c3
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-read-status 30 sleep-env-esp32c3
git diff --check
```

Observed validation results:

- Windows GATT service lookup retried with Uncached lookup and Cached fallback.
- Characteristic lookup retried with Uncached lookup and Cached fallback.
- Status reads retried with Uncached values only for runtime decisions.
- `scan-read-status` can recreate the WinRT `BluetoothLEDevice` / GATT objects
  after repeated status-read failures.
- `scan-watch-clear-gesture` can reconnect after a transient status-read
  failure instead of ending the delay-safe clear-gesture watch immediately.
- `scan-unpair` reported `UNPAIR_RESULT status=Unpaired`, clearing the
  Windows-side central pairing/cache state enough for a later status read.
- A following `scan-read-status` succeeded and decoded
  `runtime=Connected network=Disconnected upload=Failed pending=32
  error_flags=0x00000000 pairing=Closed boot_button=Released remaining_ms=0
  pressed_ms=0`.

Flash and hardware notes:

- No firmware image was flashed for this milestone.
- No firmware flash sector was deliberately written or erased by this tooling
  change.
- `scan-unpair` changed only the Windows-side central pairing record.
- The final central state after this recovery is Windows unpaired, so runtime
  clear-gesture validation must first rebuild a saved-bond auth record before
  it can prove that the 8 second BOOT / IO9 gesture clears that record.

Remaining Phase 24 validation:

- Runtime 8 second BOOT / IO9 saved-auth clearing remains unvalidated on
  hardware.
- BOOT / IO9 download-mode preservation remains unvalidated.
- Protected-characteristic rejection after the runtime saved-auth clear gesture
  remains unvalidated because that gesture itself remains unvalidated.
- BLE auth record replacement/update and phone/gateway interoperability remain
  unvalidated.
- LED3 BLE hardware visual behavior remains unvalidated.
- Live Wi-Fi/BLE ACK race behavior remains unvalidated.

## Milestone 50: Phase 24V BLE Runtime Auth Clear Hardware Effect

Phase 24V validated the runtime saved-authorization clear effect on hardware
after Phase 24U hardened the Windows `tools/ble-watch` GATT recovery path. This
milestone does not close all BOOT / IO9 diagnostics because the status stream
continued to report `boot_button=Pressed` after the operator released IO9; the
operator did not hold IO9 for 40 seconds or longer.

Validation commands run from the repository root:

```bash
probe-rs reset --chip esp32c3
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-read-status 30 sleep-env-esp32c3
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-read-metadata-now 30 sleep-env-esp32c3 expect-success auto-pair
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-read-metadata-now 30 sleep-env-esp32c3 expect-success no-pair
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-watch-clear-gesture 30 sleep-env-esp32c3 180 8000
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-read-metadata-now 30 sleep-env-esp32c3 expect-reject no-pair
probe-rs reset --chip esp32c3
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-unpair 30 sleep-env-esp32c3
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-read-status 30 sleep-env-esp32c3
git diff --check
```

Observed validation results:

- Initial non-flashing reset status read decoded `pairing=Open
  boot_button=Released remaining_ms=41400 pressed_ms=0`.
- `scan-read-metadata-now ... expect-success auto-pair` rebuilt one Windows
  saved-bond authorization record.
- `scan-read-metadata-now ... expect-success no-pair` then confirmed protected
  metadata access through the saved authorization record without opening a new
  pairing flow.
- `scan-watch-clear-gesture 30 sleep-env-esp32c3 180 8000` observed
  `CLEAR_GESTURE_RELEASED index=1`,
  `CLEAR_GESTURE_PRESSED_AFTER_RELEASE index=11`,
  `CLEAR_GESTURE_HOLD_THRESHOLD index=18 pressed_ms=8000`, and
  `CLEAR_GESTURE_WINDOW_REFRESHED index=18 remaining_ms=60000 min_ms=55000`.
- The same watch did not produce final `CLEAR_GESTURE_RESULT success=True`
  because, after operator release, firmware status frames continued to report
  `boot_button=Pressed` with increasing `pressed_ms`. This is recorded as an
  IO9 release-diagnostics mismatch, not as evidence that the operator kept
  holding IO9.
- `scan-read-metadata-now ... expect-reject no-pair` after the clear watch
  reported `METADATA_NOW_RESULT success=True metadata_success=False
  rejected=True phase=control_write`, proving the previous saved authorization
  no longer grants protected metadata access.
- After another non-flashing `probe-rs reset --chip esp32c3`, `scan-unpair`
  reported `UNPAIR_RESULT status=Unpaired`.
- The final status read decoded `pairing=Open boot_button=Released
  remaining_ms=39050 pressed_ms=0`, consistent with cleared or missing auth
  metadata auto-opening the temporary authorization window on boot.

Flash and hardware notes:

- No firmware image was flashed in this milestone.
- The runtime clear path may erase only the BLE auth metadata sector
  `0x003bf000..0x003c0000`.
- The measurement spool `0x003c0000..0x00400000` was not deliberately
  exercised in this milestone.
- `scan-unpair` changed only the Windows-side central pairing record.
- The final state after this milestone is Windows unpaired, with firmware auth
  metadata cleared or missing and a boot-time temporary authorization window
  open after reset. The next saved-bond test must re-pair first.

Remaining Phase 24 validation:

- BOOT / IO9 release diagnostics after the runtime clear hold need follow-up;
  the operator released IO9, but status kept reporting `Pressed` until reset.
- BOOT / IO9 download-mode preservation remains unvalidated.
- BLE auth record replacement/update and phone/gateway interoperability remain
  unvalidated.
- LED3 BLE hardware visual behavior remains unvalidated.
- Live Wi-Fi/BLE ACK race behavior remains unvalidated.

## Milestone 51: Phase 24W BLE Watch Clear-Gesture Release Diagnostics

Phase 24W improved `tools/ble-watch` diagnostics for the remaining BOOT / IO9
release follow-up. This is a tool-only milestone; it does not change firmware
behavior and does not accept the hardware release-diagnostics item by itself.

Validation commands run from the repository root:

```bash
'/mnt/c/Program Files/dotnet/dotnet.exe' build "$(wslpath -w tools/ble-watch/ble-watch.csproj)"
git diff --check
```

Observed validation results:

- Windows .NET build succeeded with 0 warnings and 0 errors.
- `scan-watch-clear-gesture` still requires release before press,
  press-after-release, 8 second hold threshold, refreshed authorization window,
  and release after hold for `CLEAR_GESTURE_RESULT success=True`.
- The tool now prints `CLEAR_GESTURE_CLEAR_EFFECT_OBSERVED` once the hold
  threshold and refreshed pairing window have both been observed.
- If the watch ends after clear-effect evidence but before final release is
  observed, it now prints `CLEAR_GESTURE_RELEASE_DIAGNOSTIC_MISSING` with event
  indexes, hold milliseconds, refreshed-window remaining milliseconds, and the
  latest status fields.
- Success and failure summaries now include event indexes and timing fields.
- Missing final release observation remains a failed clear-gesture watch and
  must not be treated as proof that the operator kept holding BOOT / IO9.

Flash and hardware notes:

- No firmware image was flashed for this milestone.
- No firmware flash sector was deliberately written or erased.
- No hardware validation was run for this tooling milestone.
- A local `environment.md` note records that manual hardware tests requiring
  operator timing must use PowerShell `New-BurntToastNotification` and must not
  assume human cooperation until the operator replies that the notification was
  received. That file is intentionally ignored by `.gitignore`.

Remaining Phase 24 validation:

- BOOT / IO9 release diagnostics after the runtime clear hold still need
  hardware retest with the improved tool output.
- BOOT / IO9 download-mode preservation remains unvalidated.
- BLE auth record replacement/update and phone/gateway interoperability remain
  unvalidated.
- LED3 BLE hardware visual behavior remains unvalidated.
- Live Wi-Fi/BLE ACK race behavior remains unvalidated.

## Milestone 52: Phase 24X BLE Auth Record Upsert Policy Coverage

Phase 24X moved BLE authorization-record upsert policy into
hardware-independent storage logic and made the target BLE `PairingComplete`
persistence path reuse that pure policy. This is pure logic and compile
coverage only; it does not hardware-validate another real bond or an existing
peer update.

Validation commands run from the repository root:

```bash
cargo fmt
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf
cargo clippy --all-targets
cargo clippy --target riscv32imc-unknown-none-elf
cargo build --target riscv32imc-unknown-none-elf --features ble-upload,radio-coex
cargo clippy --target riscv32imc-unknown-none-elf --features ble-upload,radio-coex
git diff --check
```

Observed validation results:

- `cargo test --lib` reported `191 passed; 0 failed`.
- The new pure tests cover same identity-address update, same IRK update,
  append while capacity remains, full-capacity replacement of index `0` as the
  oldest record, record-count clamping before replacement, and zero-capacity
  `NoCapacity` handling.
- The default non-BLE ESP32-C3 target build passed.
- `cargo clippy --all-targets` and default ESP32-C3 target clippy passed.
- The BLE+Wi-Fi target build passed with `ble-upload,radio-coex`.
- The BLE+Wi-Fi target clippy run passed with `ble-upload,radio-coex`.
- `git diff --check` passed.
- `firmware/src/tasks/ble.rs` now uses the same pure upsert policy for
  target-side `PairingComplete` auth-record persistence instead of keeping a
  target-only duplicate matcher.

Flash and hardware notes:

- No firmware image was flashed for this milestone.
- No firmware flash sector was deliberately written or erased.
- Hardware validation of real BLE auth record replacement/update remains open
  until another bond or an existing peer update is observed through the runtime
  BLE path.

Remaining Phase 24 validation:

- BOOT / IO9 release diagnostics after the runtime clear hold still need
  hardware retest with the improved tool output.
- BOOT / IO9 download-mode preservation remains unvalidated.
- Real BLE auth record replacement/update and phone/gateway interoperability
  remain unvalidated.
- LED3 BLE hardware visual behavior remains unvalidated.
- Live Wi-Fi/BLE ACK race behavior remains unvalidated.

## Milestone 53: Phase 24Y BOOT / IO9 Release Diagnostic Logging

Phase 24Y added firmware-side BOOT / IO9 transition logs for the remaining
release-diagnostics follow-up. The BLE pairing task now logs the initial
runtime BOOT / IO9 sample and each sampled transition between `Pressed` and
`Released`, including the current accumulated press milliseconds and
pairing-window remaining milliseconds.

This is diagnostic preparation only. It does not change the pairing-window or
saved-auth clear state machine, does not flash firmware, and does not accept
the BOOT / IO9 release-diagnostics hardware item without a new hardware run.

Validation commands run from the repository root:

```bash
cargo fmt
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf
cargo clippy --all-targets
cargo clippy --target riscv32imc-unknown-none-elf
cargo build --target riscv32imc-unknown-none-elf --features ble-upload,radio-coex
cargo clippy --target riscv32imc-unknown-none-elf --features ble-upload,radio-coex
```

Observed validation results:

- `cargo test --lib` reported `191 passed; 0 failed`.
- The default non-BLE ESP32-C3 target build passed.
- `cargo clippy --all-targets` and default ESP32-C3 target clippy passed.
- The BLE+Wi-Fi target build passed with `ble-upload,radio-coex`.
- The BLE+Wi-Fi target clippy run passed with `ble-upload,radio-coex`.

Flash and hardware notes:

- No firmware image was flashed for this milestone.
- No firmware flash sector was deliberately written or erased.
- The next BOOT / IO9 release retest should use both the `ble-watch`
  `scan-watch-clear-gesture` output and the firmware BOOT / IO9 transition logs
  to distinguish a GPIO-level low reading from a BLE status/tooling issue.

Remaining Phase 24 validation:

- BOOT / IO9 release diagnostics after the runtime clear hold still need
  hardware retest with the improved tool output and firmware transition logs.
- BOOT / IO9 download-mode preservation remains unvalidated.
- Real BLE auth record replacement/update and phone/gateway interoperability
  remain unvalidated.
- LED3 BLE hardware visual behavior remains unvalidated.
- Live Wi-Fi/BLE ACK race behavior remains unvalidated.

## Milestone 54: Phase 24Z BOOT / IO9 Runtime Pull-Up Retest

Phase 24Z retested the runtime BOOT / IO9 clear gesture after the hardware
review corrected the IO9 pull-up assumption. The flashed BLE+Wi-Fi firmware
configures GPIO9 as input-only with the MCU internal pull-up enabled at
runtime, because the current board has no discrete IO9 pull-up and the
ESP32-C3 boot/strap weak pull-up must not be assumed to remain configured
after boot. The BOOT button still pulls IO9 to GND and has an IO9-to-GND
capacitor in parallel.

Before flashing, the declared flash range was the application image only:
approximately `0x00010000..0x003bf000`. The normal measurement spool region
`0x003c0000..0x00400000` was not intentionally erased or rewritten by the
flash command. The runtime clear gesture deliberately exercised the BLE auth
metadata sector `0x003bf000..0x003c0000`.

Validation commands and tools run:

```bash
cargo fmt
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf
cargo build --target riscv32imc-unknown-none-elf --features ble-upload,radio-coex
cargo clippy --all-targets
cargo clippy --target riscv32imc-unknown-none-elf
cargo clippy --target riscv32imc-unknown-none-elf --features ble-upload,radio-coex
cargo run --target riscv32imc-unknown-none-elf --features ble-upload,radio-coex
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-watch-clear-gesture 30 sleep-env-esp32c3 180 8000
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-read-metadata-now 30 sleep-env-esp32c3 expect-reject no-pair
```

Observed validation results:

- `cargo test --lib` reported `191 passed; 0 failed`.
- Default non-BLE target build and clippy passed.
- BLE+Wi-Fi target build and clippy passed with `ble-upload,radio-coex`.
- The first hardware run failed when the USB/JTAG connection dropped. After the
  probe reappeared, the second flash/run succeeded.
- Runtime logs reported `ble auth records restored status=Missing loaded=0
  restored=0 offset=0x003bf000 len=4096`, then auto-opened the temporary
  pairing window because no saved auth record was present.
- Runtime logs also reported `Wi-Fi controller initialization failed; network
  and uploader disabled` with ESP Wi-Fi init error `257`. This is a Phase 24
  Wi-Fi/BLE coexistence runtime observation and remains separate from the
  successful BLE/IO9 retest.
- `scan-watch-clear-gesture` observed `boot_button=Released` before the press,
  then `Pressed`, `CLEAR_GESTURE_HOLD_THRESHOLD pressed_ms=8100`, a refreshed
  temporary authorization window with `remaining_ms=59900`, and final
  `boot_button=Released` after the hold.
- The tool ended with `CLEAR_GESTURE_RESULT success=True` and
  `released_after_hold=True`.
- Firmware RTT logs matched the central-observed state transitions:
  `ble boot/io9 transition state=Pressed`, `ble auth records clear requested
  pressed_ms=8000`, `ble auth records cleared offset=0x003bf000 len=4096`, and
  `ble boot/io9 transition state=Released pressed_ms=19700`.
- After the refreshed authorization window expired, `scan-read-metadata-now 30
  sleep-env-esp32c3 expect-reject no-pair` reported
  `METADATA_NOW_RESULT success=True metadata_success=False rejected=True
  phase=control_write`, confirming the cleared saved authorization no longer
  grants protected metadata access.

Flash and hardware notes:

- The BOOT / IO9 runtime clear gesture now has complete release-diagnostics
  evidence with the explicit runtime GPIO9 internal pull-up firmware.
- This milestone deliberately erased the BLE auth metadata sector
  `0x003bf000..0x003c0000` through the 8 second runtime BOOT / IO9 clear
  gesture.
- This milestone did not intentionally exercise the measurement spool flash
  region `0x003c0000..0x00400000`.
- This milestone did not validate LED3 visual behavior, BOOT download-mode
  preservation, phone/gateway interoperability, real auth-record
  replacement/update, or live Wi-Fi/BLE ACK race behavior.

Remaining Phase 24 validation:

- BOOT / IO9 download-mode preservation remains unvalidated.
- Real BLE auth record replacement/update and phone/gateway interoperability
  remain unvalidated.
- LED3 BLE hardware visual behavior remains unvalidated.

## Milestone 55: Phase 24 BLE+Wi-Fi Coexistence Heap Startup Fix

This milestone resolves the Phase 24Z BLE+Wi-Fi controller startup blocker.
The BLE+Wi-Fi feature build now adds a second 64 KiB internal heap region in
addition to the reclaimed heap, matching the `esp-generate` Wi-Fi+BLE template
guidance that coexistence needs more RAM. Wi-Fi initialization failure logging
also preserves the concrete `WifiError` enum for future diagnosis.

Before flashing, the declared flash range was the application image only:
approximately `0x00010000..0x003bf000`. The BLE auth metadata sector
`0x003bf000..0x003c0000` was not deliberately written or erased. The BLE
status read did not perform BLE ACK. Normal firmware storage was running during
the test, so measurement spool appends/drop-oldest behavior may continue to
write the measurement spool range `0x003c0000..0x00400000`.

Validation commands and tools run:

```bash
cargo fmt
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf
cargo build --target riscv32imc-unknown-none-elf --features ble-upload,radio-coex
cargo clippy --all-targets
cargo clippy --target riscv32imc-unknown-none-elf
cargo clippy --target riscv32imc-unknown-none-elf --features ble-upload,radio-coex
cargo run --target riscv32imc-unknown-none-elf --features ble-upload,radio-coex
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-read-status 30 sleep-env-esp32c3
```

Observed validation results:

- `cargo test --lib` reported `191 passed; 0 failed`.
- Default non-BLE target build and clippy passed.
- BLE+Wi-Fi target build and clippy passed with `ble-upload,radio-coex`.
- `cargo run --target riscv32imc-unknown-none-elf --features
  ble-upload,radio-coex` flashed and started the BLE+Wi-Fi firmware.
- RTT logs showed `wifi connecting ssid=FZU auth=Open`, `wifi connected
  ssid=FZU`, an IPv4 configuration, `ble advertising
  name=sleep-env-esp32c3 protocol_version=1`, and BLE central
  connect/disconnect events.
- The previous Phase 24Z `Wi-Fi controller initialization failed; network and
  uploader disabled` error did not appear in this run.
- `scan-read-status 30 sleep-env-esp32c3` succeeded and decoded
  `runtime=Connected network=IpReady upload=TimeFailed pending=32
  error_flags=0x00000000`.
- This proves BLE GATT status and IP-ready Wi-Fi status coexist in the same
  runtime image. It does not accept the live Wi-Fi/BLE ACK race item because
  no BLE ACK/drain command was run in this milestone.

Flash and hardware notes:

- This milestone resolves the BLE+Wi-Fi runtime Wi-Fi init error `257` as a
  Phase 24 startup blocker.
- The board was left running the BLE+Wi-Fi firmware with the extra coexistence
  heap.
- BLE auth metadata was missing or cleared at boot, so the temporary
  authorization window opened and later expired. The status read after expiry
  reported `pairing=Closed boot_button=Released remaining_ms=0 pressed_ms=0`.
- Time sync and upload still reported runtime failures in this environment
  (`TimeFailed`, `ConnectReset`, or discovery failure). That is separate from
  Wi-Fi controller startup and must not be counted as a successful REST upload.

Remaining Phase 24 validation:

- BOOT / IO9 download-mode preservation remains unvalidated.
- Real BLE auth record replacement/update and phone/gateway interoperability
  remain unvalidated.
- LED3 BLE hardware visual behavior remains unvalidated.

## Milestone 56: Phase 24 Live Wi-Fi/BLE ACK Suppression Validation

This milestone validates the live BLE ACK policy while Wi-Fi upload is also
running. The Windows central requested BLE ACK for a transferred record, but
the firmware observed Wi-Fi/IP ready and the last upload result as successful,
so it suppressed the BLE storage ACK instead of deleting the persistent spool
record through the BLE path.

No new firmware image was flashed for this milestone. It reused the
BLE+Wi-Fi firmware from Milestone 55. The app image range last flashed for that
firmware was approximately `0x00010000..0x003bf000`. During this runtime
validation, normal firmware storage and successful REST uploads may write or
erase the measurement spool region `0x003c0000..0x00400000`. BLE pairing or
saved-bond refresh may write the BLE auth metadata sector
`0x003bf000..0x003c0000`.

Validation commands and tools run:

```bash
env UV_CACHE_DIR=.cache/uv uv run sleep-env-server serve --host 0.0.0.0 --port 8080 --udp-discovery-port 39022 --no-rich
'/mnt/c/Program Files/dotnet/dotnet.exe' build "$(wslpath -w tools/ble-watch/ble-watch.csproj)"
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-transfer-record-now 30 sleep-env-esp32c3 ack 128 auto-pair
```

Observed validation results:

- The server accepted repeated board uploads from `10.133.15.188` with HTTP
  `204`.
- Firmware RTT logs showed discovery, time sync, and REST upload success, for
  example `discovery endpoint ipv4=10.133.56.218 port=8080`, `time synced`,
  and `upload success sequence=136824 acked=true`.
- `scan-transfer-record-now` first decoded
  `runtime=Connected network=IpReady upload=Success pending=0`.
- Windows Custom ConfirmOnly pairing completed or refreshed successfully:
  `TRANSFER_NOW_PAIR_CUSTOM_RESULT status=Paired`.
- The central transferred record `136853`, validated the payload, wrote
  `CompleteRecord`, and requested `AckRecord`.
- Firmware RTT logs matched the transfer and ACK-policy decision:
  `ble record marked complete without storage ACK sequence=136853` followed by
  `ble storage ACK suppressed sequence=136853 network_state=IpReady
  upload_result=Success`.
- The final status snapshot still decoded
  `runtime=Connected network=IpReady upload=Success pending=0`.

Representative tool output:

```text
STATUS_DECODED version=1 runtime=Connected network=IpReady upload=Success pending=0
TRANSFER_NOW_PAIR_CUSTOM_RESULT status=Paired
TRANSFER_NOW_METADATA_METADATA_DECODED version=1 sequence=136853 payload_len=201
TRANSFER_NOW_COMPLETE_RECORD_WRITE status=Success
TRANSFER_NOW_ACK_RECORD_WRITE status=Success
TRANSFER_NOW_RESULT success=True sequence=136853 ack_requested=True
STATUS_DECODED version=1 runtime=Connected network=IpReady upload=Success pending=0
TRANSFER_NOW_SUMMARY success=True sequence=136853 ack_requested=True payload_len=201
```

Flash and hardware notes:

- This accepts the live Wi-Fi/BLE ACK race-policy behavior on hardware for the
  observed case where Wi-Fi/IP is ready and REST upload is succeeding.
- BLE did not delete the spool record through `StorageCommand::Ack`; the ACK
  was deliberately suppressed by policy.
- Normal Wi-Fi upload remains the primary durable ACK path while it is
  succeeding.

Remaining Phase 24 validation:

- BOOT / IO9 download-mode preservation remains unvalidated.
- Real BLE auth record replacement/update and phone/gateway interoperability
  remain unvalidated.
- LED3 BLE hardware visual behavior remains unvalidated.

## Milestone 57: Phase 24 Auth-Record Re-Pair Validation Tooling

This milestone prepares the remaining real BLE auth record update/replacement
validation by adding a single `ble-watch` flow for the existing Windows central.
The new command removes the Windows-side pairing record, waits for the
firmware BOOT / IO9 authorization window, reconnects so the firmware can mark
the new link bondable, performs Windows Custom ConfirmOnly pairing, and reads
protected metadata.

This is tooling only. It does not accept the firmware auth-record
replacement/update item by itself because the acceptance signal must include
firmware RTT logs from `persist_ble_bond_information`, such as
`ble auth record updated`, `ble auth record appended`, or
`ble auth record capacity full; replacing oldest bond record`, followed by
`ble auth bond stored`.

Validation command added:

```bash
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-unpair-then-pair-metadata 30 sleep-env-esp32c3 90
```

Expected manual validation flow:

1. Capture firmware RTT logs.
2. Run `scan-unpair-then-pair-metadata`.
3. When the tool prints `PAIRING_WAIT`, hold BOOT / IO9 long enough to open the
   authorization window.
4. Accept the run only when the tool reports
   `UNPAIR_PAIR_METADATA_SUMMARY success=True` and RTT logs show the expected
   auth-record action plus `ble auth bond stored`.

Flash and hardware notes:

- No firmware image is flashed by this tooling milestone.
- Running the command during future hardware validation may write the BLE auth
  metadata sector `0x003bf000..0x003c0000` when pairing completes and the
  firmware stores the refreshed bond.
- A true second-bond or full-capacity replacement validation still needs a
  distinct central device; the Windows re-pair flow only targets the
  existing-peer update case.

Remaining Phase 24 validation:

- BOOT / IO9 download-mode preservation remains unvalidated.
- Real BLE auth record replacement/update remains unvalidated until the tool is
  run with firmware RTT evidence.
- Phone/gateway interoperability remains unvalidated.
- LED3 BLE hardware visual behavior remains unvalidated.

## Milestone 58: Phase 24 BLE LED And Existing-Peer Auth Hardware Evidence

This milestone records 2026-05-26 hardware evidence gathered from the running
BLE+Wi-Fi firmware without flashing a new application image.

No app image was flashed for this milestone. The last flashed app-image range
remains approximately `0x00010000..0x003bf000`. Normal firmware storage was
running during the checks and may write the measurement spool region
`0x003c0000..0x00400000`. Pairing, saved-bond update, and the later long BOOT /
IO9 hold may write or erase the BLE authorization metadata sector
`0x003bf000..0x003c0000`.

Validation commands and tools run:

```bash
probe-rs attach --chip esp32c3 --no-location target/riscv32imc-unknown-none-elf/debug/sleep-environment-monitor
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-unpair-then-pair-metadata 30 sleep-env-esp32c3 90
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-read-metadata-now 30 sleep-env-esp32c3 expect-success auto-pair
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-watch-status 30 sleep-env-esp32c3 120
cargo espflash board-info --chip esp32c3 --port /dev/ttyACM0 --before no-reset --after no-reset --non-interactive
cargo espflash board-info --chip esp32c3 --port /dev/ttyACM0 --before usb-reset --after no-reset --non-interactive
cargo espflash board-info --chip esp32c3 --port /dev/ttyACM0 --before default-reset --after hard-reset --non-interactive
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-unpair 30 sleep-env-esp32c3
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-read-status 30 sleep-env-esp32c3
```

Observed validation results:

- The existing Windows central re-pair/update path was accepted with firmware
  RTT evidence. `scan-unpair-then-pair-metadata` timed out near the manual
  BOOT / IO9 timing boundary, but it opened the authorization window. The
  follow-up `scan-read-metadata-now ... expect-success auto-pair` completed
  pairing and read protected metadata:
  `METADATA_NOW_PAIR_CUSTOM_RESULT status=Paired`,
  `METADATA_NOW_METADATA_DECODED version=1 sequence=156735 payload_len=206`,
  and `METADATA_NOW_RESULT success=True metadata_success=True rejected=False`.
- RTT logs matched the existing-peer update acceptance signal:
  `ble pairing complete security_level=Encrypted bonded=true saved_bonds=1`,
  `ble auth record updated index=0`, `ble auth bond stored count=1
  offset=0x003bf000 len=4096`, and `ble metadata prepared sequence=156735
  payload_len=206`.
- BLE connected/advertising LED3 slow blink was manually accepted. While
  `scan-watch-status` held a BLE status connection and decoded
  `runtime=Connected network=IpReady ... pairing=Closed boot_button=Released`,
  the operator reported `LED3 慢闪`.
- BOOT / IO9-triggered pairing/authorization LED3 fast blink was manually
  accepted. `scan-watch-status` decoded `boot_button=Pressed`, then
  `pairing=Open` with `remaining_ms` near `60000`; during that open window the
  operator reported `LED3 快闪`.
- The fast-blink check held BOOT / IO9 long enough to trigger the runtime
  saved-auth clear threshold. RTT logs showed `ble auth records clear requested
  pressed_ms=8000` and `ble auth records cleared offset=0x003bf000 len=4096`.
  This deliberately exercised only the BLE auth metadata sector.
- A hard reset after the clear restored the normal application BLE path.
  Windows-side stale pairing first caused `Unreachable` GATT reads. After
  `scan-unpair`, a follow-up `scan-read-status` succeeded and decoded
  `runtime=Connected network=IpReady upload=TransportFailed pending=32` and
  `pairing=Closed boot_button=Released remaining_ms=0 pressed_ms=0`.
- A download-mode preservation attempt was not accepted. Read-only
  `cargo espflash board-info` probes using `--before no-reset --after
  no-reset`, `--before usb-reset --after no-reset`, and `--before
  default-reset --after hard-reset` all failed with `Error while connecting to
  device`; a following BLE scan also did not find the board until the operator
  hard-reset the board back to application mode. After recovery,
  `scan-read-status 20 sleep-env-esp32c3` again decoded `runtime=Connected
  network=IpReady upload=TransportFailed pending=32` and
  `pairing=Closed boot_button=Released`. This is recorded as
  inconclusive/failed evidence for the current serial download-mode evidence
  chain, not as download-mode validation.

Flash and hardware notes:

- This milestone did not flash a new firmware image.
- The existing-peer auth update and the later runtime clear gesture exercised
  the BLE auth metadata sector `0x003bf000..0x003c0000`.
- Normal storage may continue to write the measurement spool region
  `0x003c0000..0x00400000` while the firmware runs.
- LED3 slow blink and BOOT-triggered pairing/authorization fast blink are now
  visually accepted for the exercised states.

Remaining Phase 24 validation:

- The 180 second post-boot BLE status window on LED3 still needs explicit
  manual visual acceptance after reset or power-on.
- BOOT / IO9 download-mode preservation remains unvalidated.
- Phone/gateway interoperability remains unvalidated beyond the Windows
  `ble-watch` central.

## Milestone 59: Phase 24 Final LED And BOOT Acceptance

This milestone closes Phase 24 acceptance. It records the final 2026-05-26
manual hardware checks and the explicit Phase 24 scope decision for non-Windows
centrals.

No firmware image was flashed for this milestone. The last flashed app-image
range remains approximately `0x00010000..0x003bf000`. No deliberate flash
write or erase was performed by these final checks. Normal firmware storage may
continue to write the measurement spool region `0x003c0000..0x00400000` while
the firmware runs.

Tooling update:

- `tools/ble-watch scan-watch-status` now catches recoverable stale WinRT/GATT
  status-read exceptions such as `ObjectDisposedException`, releases the stale
  status connection, opens a fresh GATT status connection, and continues until
  the original watch deadline.
- The runtime clear-gesture watch uses the same recoverable status-snapshot
  helper.

Validation commands and tools run:

```bash
'/mnt/c/Program Files/dotnet/dotnet.exe' build "$(wslpath -w tools/ble-watch/ble-watch.csproj)"
probe-rs reset --chip esp32c3
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-watch-status 30 sleep-env-esp32c3 180
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-read-status 30 sleep-env-esp32c3
```

Observed validation results:

- The Windows .NET build passed with 0 warnings and 0 errors.
- After `probe-rs reset --chip esp32c3`, the 180 second status watch started
  and decoded early post-boot status frames with `runtime=Connected`,
  `network=IpReady`, `pairing=Open`, and `boot_button=Released`. The operator
  manually accepted that blue LED3 represented BLE status for the 180 second
  post-boot indication window.
- BOOT / IO9 download-mode preservation during reset or power-on was accepted
  by operator-assisted validation. Holding BOOT / IO9 at reset or power-on
  still selects ESP32-C3 download mode rather than runtime BLE pairing or BLE
  auth-record clearing.
- Phone/gateway interoperability beyond the Windows `ble-watch` central is
  `skipped / not planned` for Phase 24. This repository does not implement a
  mobile app or gateway in Phase 24; Windows `ble-watch` remains the accepted
  Phase 24 validation central.
- A final `scan-read-status 30 sleep-env-esp32c3` confirmed that the
  application BLE path was reachable after the acceptance steps and decoded
  `runtime=Connected network=IpReady upload=TimeFailed pending=32` and
  `pairing=Closed boot_button=Released remaining_ms=0 pressed_ms=0`.

Flash and hardware notes:

- This milestone did not flash a new firmware image.
- No deliberate BLE auth metadata sector write or erase was performed.
- No deliberate measurement spool ACK or drain was performed.
- Future BOOT / IO9 hardware changes or runtime GPIO policy changes should
  revalidate both runtime release behavior and reset/power-on download-mode
  behavior.

Phase 24 status:

- Phase 24 is complete.
- Distinct second-central append or full-capacity BLE auth-record replacement
  remains useful future coverage, but it is no longer a Phase 24 acceptance
  blocker because the existing Windows-central update path has hardware
  evidence.

Milestone commit message:

```text
test: close Phase 24 BLE acceptance
```
