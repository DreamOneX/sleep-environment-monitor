# Firmware Toolchain and Coding Style

This document applies to the ESP32-C3 firmware package. Server toolchain and server code style are intentionally not defined here.

## Toolchain

Target Rust version:

```text
rust-version = 1.88
edition = 2024
```

Target platform:

```text
ESP32-C3
Target triple: riscv32imc-unknown-none-elf
```

Primary crates:

```text
esp-hal
esp-rtos
esp-radio
embassy-executor
embassy-time
embassy-net
defmt
static_cell
critical-section
```

The firmware is `no_std` for the embedded target.

Host-side unit tests may use `std` where necessary, but hardware-independent logic should remain portable and deterministic.

---

## Repository Layout

The repository root is a Cargo workspace:

```text
Cargo.toml
firmware/
server/
docs/
```

The `firmware` package contains the ESP32-C3 firmware and is the workspace default member. Run standard Cargo commands from the repository root unless a task explicitly says otherwise.

The `server` directory is reserved for measurement ingestion server work. The current `server/post_receiver.py` file is a temporary local receiver for firmware upload validation.

---

## Formatting

All Rust code must be formatted with:

```bash
cargo fmt
```

Formatting is required before commit.

No manual alignment style is required beyond `rustfmt`.

---

## Linting

Run:

```bash
cargo clippy --all-targets
```

Suggested policy:

```text
Warnings should be fixed before merge.
Do not silence warnings unless there is a clear reason.
Use #[allow(...)] only locally, not globally.
```

Avoid:

```rust
.unwrap()
.expect("...")
todo!()
unimplemented!()
```

Allowed exceptions:

```text
early bring-up code
clearly unreachable hardware initialization failure
test code
```

Production tasks should return errors or set error flags instead of panicking.

---

## Unit Tests

Run host-side unit tests with:

```bash
cargo test --lib
```

Unit tests must not require:

```text
ESP32 hardware
I2C devices
ADC input
Wi-Fi
USB connection
button presses
LED observation
human confirmation
```

Unit tests should cover only pure logic:

```text
CRC
raw data conversion
RMS / peak calculation
queue behavior
status mapping
payload encoding
Wi-Fi state machine
```

---

## Embedded Build

Build firmware with:

```bash
cargo build --target riscv32imc-unknown-none-elf
```

Build should not depend on host-only test code.

Host-test-only code must be behind:

```rust
#[cfg(test)]
```

or equivalent feature gating.

---

## Module Style

Keep hardware access separate from pure logic.

Recommended pattern:

```text
firmware/src/drivers/sht40.rs
  pure logic:
    crc8()
    convert_temperature()
    convert_humidity()
    parse_measurement()

  hardware wrapper:
    Sht40<I2C>::read()
```

Pure logic should be unit tested.  
Hardware wrappers are verified in board integration testing.

---

## Error Handling

Use explicit errors or error flags.

Preferred:

```rust
Result<T, DriverError>
```

or:

```rust
ErrorFlags
```

Avoid panics in runtime tasks.

Tasks should generally:

```text
record error
set error flag
continue running
retry later
```

Sensor failure must not stop:

```text
mic_task
wifi_task
led_task
uploader_task
```

Wi-Fi failure must not stop:

```text
sensor_task
mic_task
aggregator_task
```

---

## Async Task Rules

Each task owns one responsibility.

```text
sensor_task: I2C sensors
mic_task: ADC microphone sampling
aggregator_task: merge samples
wifi_task: connect / reconnect Wi-Fi
uploader_task: upload measurements
led_task: display status
```

Do not mix responsibilities.

Avoid long blocking loops inside Embassy tasks.

If a task performs repeated work, it should periodically yield by awaiting timers, channels, or I/O futures.

---

## Shared State

Prefer message passing:

```text
Channel
Signal
```

Avoid global mutable state unless there is a clear reason.

If shared state is required, wrap it explicitly with an appropriate synchronization primitive.

---

## Naming

Use clear hardware names:

```rust
PIN_I2C_SDA
PIN_I2C_SCL
PIN_MIC_ADC
I2C_ADDR_SHT40
I2C_ADDR_OPT3001
```

Use clear sample types:

```rust
EnvSample
MicSample
Measurement
NetworkState
UploadResult
ErrorFlags
```

LED names should include polarity in comments:

```rust
// Active-low LED
pub const PIN_LED1: u8 = 0;
pub const PIN_LED2: u8 = 1;
```

---

## Numeric Values

Avoid magic numbers in task code.

Put hardware constants in `board.rs`:

```rust
pub const I2C_ADDR_SHT40: u8 = 0x44;
pub const I2C_ADDR_OPT3001: u8 = 0x45;

pub const PIN_I2C_SDA: u8 = 4;
pub const PIN_I2C_SCL: u8 = 5;
pub const PIN_MIC_ADC: u8 = 3;
```

Put algorithm constants near the algorithm:

```rust
pub const ADC_MAX: u16 = 4095;
pub const ADC_CLIP_LOW: u16 = 8;
pub const ADC_CLIP_HIGH: u16 = 4087;
```

Put deployment and behavior policy constants in `firmware/src/config.rs` once Phase 21 is implemented:

```text
Wi-Fi credentials or defaults
REST fallback endpoint and path
retry delays and timeouts
sampling cadence
buffer sizes
log and LED timing intervals
```

The intended config boundary is documented in [04-configuration.md](04-configuration.md).

---

## Commit Checklist

Before committing, run:

```bash
cargo fmt
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf
cargo clippy --all-targets
cargo clippy --target riscv32imc-unknown-none-elf
```

A commit should not mix unrelated work.

Good commit examples:

```text
test: add SHT40 CRC and conversion logic
test: add OPT3001 lux conversion logic
feat: add I2C sensor task
feat: add Wi-Fi connection manager
fix: remove panic path from uploader task
```

Avoid:

```text
update
fix stuff
misc
final
```

---

## Documentation Policy

Every hardware-facing module should document:

```text
which chip it talks to
which bus it uses
which address or pin it uses
what errors it can return
```

Every pure conversion function should document:

```text
input format
output unit
important formula
edge-case behavior
```
