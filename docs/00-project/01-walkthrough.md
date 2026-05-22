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
- Remaining Phase 20 checks: multi-hour/overnight soak, Wi-Fi disconnect preservation, LED2 visual confirmation for storage/upload failure, forced full-spool oldest-drop confirmation with metrics, and power interruption during or near a flash write.

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
- Phase 20 still has human/physical close-loop checks remaining: multi-hour or overnight soak, Wi-Fi-disconnect preservation separate from receiver-offline behavior, LED2 visual confirmation, and power interruption during or near a flash write.

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
- Remaining Phase 20 physical checks: multi-hour or overnight soak, Wi-Fi-disconnect preservation separate from receiver-offline behavior, LED2 visual confirmation, and power interruption during or near a flash write.

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
    unavailable and a paired central confirms complete receipt.
- Document BOOT / IO9 as a possible future runtime pairing or authorization
  input only, with constraints that preserve the default pull-up and download
  mode behavior.
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
- Updated the BLE feature target path to configure GPIO9 / BOOT as input-only
  with the default no-pull configuration.
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
