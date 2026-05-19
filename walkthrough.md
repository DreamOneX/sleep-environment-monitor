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
