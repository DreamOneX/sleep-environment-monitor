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
