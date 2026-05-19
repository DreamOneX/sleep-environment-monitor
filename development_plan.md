# Development Plan

## Phase 1: Project Skeleton

## Goal

Create the project structure and basic shared types.

## Work Items

- Create module tree:
  - `board.rs`
  - `types.rs`
  - `drivers/mod.rs`
  - `tasks/mod.rs`
  - `util/mod.rs`
- Keep `main.rs` minimal.
- Add basic data types:
  - `EnvSample`
  - `MicSample`
  - `Measurement`
  - `ErrorFlags`

## Unit Tests

- `ErrorFlags::insert`
- `ErrorFlags::contains`
- Default values for shared sample types

## Done When

- Project builds.
- `cargo test --lib` passes.
- No hardware access exists in unit-tested modules.

## Git Commit Message

```text
chore: scaffold firmware modules and shared data types
```

---

# Phase 2: SHT40 Pure Logic

## Goal

Implement SHT40 CRC and raw data conversion.

## Work Items

Create:

```text
src/drivers/sht40.rs
```

Implement:

```rust
pub fn crc8(data: &[u8]) -> u8;
pub fn convert_temperature(raw: u16) -> f32;
pub fn convert_humidity(raw: u16) -> f32;
pub fn parse_measurement(buf: [u8; 6]) -> Result<(f32, f32), Sht40Error>;
```

## Unit Tests

- CRC known vector
- Valid measurement frame parses correctly
- Invalid temperature CRC returns error
- Invalid humidity CRC returns error
- `raw = 0` temperature conversion
- `raw = 65535` temperature conversion
- Humidity clamps to `0..100`

## Done When

- SHT40 parsing works without I2C.
- All SHT40 tests pass.

## Git Commit Message

```text
test: add SHT40 CRC and conversion logic
```

---

# Phase 3: OPT3001 Pure Logic

## Goal

Implement OPT3001 lux conversion.

## Work Items

Create:

```text
src/drivers/opt3001.rs
```

Implement:

```rust
pub const CONFIG_CONTINUOUS: u16 = 0xCE10;
pub fn raw_to_lux(raw: u16) -> f32;
```

## Unit Tests

- `0x0000 -> 0 lux`
- `0x0001 -> 0.01 lux`
- `0x1001 -> 0.02 lux`
- `0x2001 -> 0.04 lux`
- exponent and mantissa extraction

## Done When

- Lux conversion is correct.
- No I2C required for tests.

## Git Commit Message

```text
test: add OPT3001 lux conversion logic
```

---

# Phase 4: Microphone Signal Logic

## Goal

Implement ADC sample statistics.

## Work Items

Create:

```text
src/drivers/mic.rs
```

Implement:

```rust
pub struct MicStats {
    pub mean: f32,
    pub rms: f32,
    pub peak: f32,
    pub db_rel: f32,
    pub clip_count: u32,
}

pub fn analyze_adc_samples(samples: &[u16]) -> MicStats;
```

## Unit Tests

- Constant samples produce `rms = 0`
- Symmetric samples produce correct mean and peak
- Clipped samples increment `clip_count`
- Empty input does not panic
- `db_rel` never becomes NaN

## Done When

- Microphone stats are deterministic.
- No ADC hardware required for tests.

## Git Commit Message

```text
test: add microphone ADC statistics logic
```

---

# Phase 5: Queue and Status Logic

## Goal

Implement upload buffering and LED status mapping.

## Work Items

Create:

```text
src/util/queue.rs
src/util/status.rs
```

Implement queue:

```rust
pub struct DropOldestQueue<T, const N: usize>;
```

Implement LED patterns:

```rust
pub enum LedPattern {
    Off,
    On,
    SlowBlink,
    FastBlink,
    Heartbeat,
}

pub fn status_to_leds(flags: ErrorFlags, wifi_connected: bool) -> LedState;
```

## Unit Tests

Queue:

- Empty queue pop returns `None`
- Normal push/pop order
- Full queue drops oldest
- Repeated overflow does not panic

LED status:

- No error + Wi-Fi connected
- No error + Wi-Fi disconnected
- Sensor error
- Upload error
- Multiple errors with priority

## Done When

- Queue and LED policy are fully testable without hardware.

## Git Commit Message

```text
test: add upload queue and status mapping logic
```

---

# Phase 6: Measurement Aggregation

## Goal

Merge environment and microphone samples into one record.

## Work Items

Create:

```text
src/tasks/aggregator.rs
```

Implement pure function:

```rust
pub fn merge_measurement(env: EnvSample, mic: MicSample) -> Measurement;
```

## Unit Tests

- Copies temperature / humidity / lux
- Copies mic fields
- Merges error flags
- Handles missing sensor fields
- Selects timestamp correctly

Recommended timestamp rule:

```text
Measurement.uptime_ms = max(env.uptime_ms, mic.uptime_ms)
```

## Done When

- Aggregation works without Embassy.
- Aggregation tests pass.

## Git Commit Message

```text
test: add measurement aggregation logic
```

---

# Phase 7: Upload Payload Encoding

## Goal

Encode `Measurement` into a network payload.

## Work Items

Create encoding function in:

```text
src/tasks/upload.rs
```

First version may use CSV or JSON.

Example CSV fields:

```text
uptime_ms,temp_c,rh_percent,lux,mic_mean,mic_rms,mic_peak,mic_db_rel,mic_clip_count,error_flags
```

Implement:

```rust
pub fn measurement_to_csv_line(
    m: &Measurement,
    out: &mut [u8],
) -> Result<usize, EncodeError>;
```

## Unit Tests

- Complete measurement encodes correctly
- Missing values encode as `nan` or empty fields
- Small output buffer returns error
- Error flags encode correctly
- Function never panics

## Done When

- Payload encoding is tested without Wi-Fi.

## Git Commit Message

```text
test: add measurement payload encoding
```

---

# Phase 8: Wi-Fi State Machine

## Goal

Implement testable Wi-Fi connection state logic.

## Work Items

In:

```text
src/tasks/wifi.rs
```

Implement:

```rust
pub enum WifiState {
    Init,
    Connecting,
    Connected,
    Backoff { attempt: u8 },
}

pub enum WifiEvent {
    Start,
    ConnectOk,
    ConnectFailed,
    Disconnected,
    RetryTimerExpired,
}

pub fn next_wifi_state(state: WifiState, event: WifiEvent) -> WifiState;
pub fn backoff_seconds(attempt: u8) -> u32;
```

Backoff sequence:

```text
1s, 2s, 5s, 10s, 30s, 30s...
```

## Unit Tests

- `Init + Start -> Connecting`
- `Connecting + ConnectOk -> Connected`
- `Connecting + ConnectFailed -> Backoff`
- `Connected + Disconnected -> Backoff`
- `Backoff + RetryTimerExpired -> Connecting`
- Backoff caps at 30 seconds
- Attempt count does not overflow

## Done When

- Wi-Fi state logic is tested without real Wi-Fi.

## Git Commit Message

```text
test: add Wi-Fi reconnect state machine
```

---

# Phase 9: Hardware Bring-Up Minimal Firmware

## Goal

Bring up the board with minimal hardware use.

## Work Items

In `src/bin/main.rs`:

- Initialize clocks.
- Initialize GPIO.
- Initialize Embassy executor.
- Start LED heartbeat task only.

## Unit Tests

None.

This is hardware integration work.

## Manual Integration Checks

- Board boots.
- No USB reset loop.
- LED task runs.
- Reset button works.
- BOOT button still allows download mode.

## Done When

- Board runs stable for at least several minutes.
- No repeated USB reconnects.

## Git Commit Message

```text
feat: bring up minimal Embassy runtime and LED heartbeat
```

---

# Phase 10: I2C Sensor Bring-Up

## Goal

Bring up SHT40 and OPT3001 on the real board.

## Work Items

- Initialize I2C on:
  - SDA = IO4
  - SCL = IO5
- Implement SHT40 hardware read.
- Implement OPT3001 hardware init and read.
- Create `sensor_task`.

## Unit Tests

No new hardware unit tests.

Existing pure tests must still pass:

```text
SHT40 CRC / conversion
OPT3001 lux conversion
```

## Manual Integration Checks

- I2C scan finds:
  - `0x44`
  - `0x45`
- SHT40 returns reasonable values.
- OPT3001 returns reasonable lux.
- Sensor failures set error flags instead of panicking.

## Done When

- `sensor_task` produces `EnvSample` periodically.
- I2C errors do not crash the firmware.

## Git Commit Message

```text
feat: add I2C sensor task for SHT40 and OPT3001
```

---

# Phase 11: Microphone ADC Bring-Up

## Goal

Bring up analog microphone sampling.

## Work Items

- Initialize ADC on IO3 / ADC1_CH3.
- Create `mic_task`.
- Sample ADC in 1-second windows.
- Reuse `analyze_adc_samples`.

## Unit Tests

Existing microphone pure tests must pass.

No hardware-dependent unit tests.

## Manual Integration Checks

- `mic_mean` is within ADC range.
- `mic_rms` is low in quiet room.
- `mic_rms` and `mic_peak` increase with sound.
- Clipping counter remains zero in normal conditions.

## Done When

- `mic_task` produces `MicSample` periodically.
- ADC task does not block other tasks.

## Git Commit Message

```text
feat: add microphone ADC sampling task
```

---

# Phase 12: Aggregation and Local Output

## Goal

Generate complete `Measurement` records.

## Work Items

- Add channels:
  - `EnvSample`
  - `MicSample`
  - `Measurement`
- Spawn:
  - `sensor_task`
  - `mic_task`
  - `aggregator_task`
- Add local debug output.

## Unit Tests

Existing aggregation and payload tests must pass.

## Manual Integration Checks

- One complete measurement is produced periodically.
- Missing sensor values do not crash output.
- Error flags appear in output.

## Done When

- Board can produce continuous measurement records without Wi-Fi.

## Git Commit Message

```text
feat: aggregate sensor and microphone measurements
```

---

# Phase 13: Wi-Fi Connection

## Goal

Connect to Wi-Fi without affecting sampling.

## Work Items

- Initialize Wi-Fi.
- Spawn network stack task if required.
- Spawn `wifi_task`.
- Implement reconnect backoff.
- Publish `NetworkState`.

## Unit Tests

Existing Wi-Fi state machine tests must pass.

No real Wi-Fi unit tests.

## Manual Integration Checks

- Wi-Fi connects.
- Wrong credentials do not crash firmware.
- Disconnect triggers backoff.
- Sampling continues while Wi-Fi is disconnected.

## Done When

- Wi-Fi state is visible to the rest of the application.
- Network failure does not stop measurements.

## Git Commit Message

```text
feat: add Wi-Fi connection manager
```

---

# Phase 14: Upload Task

## Goal

Upload measurements over Wi-Fi.

## Work Items

- Implement `uploader_task`.
- Read from measurement queue.
- Upload payload.
- Retry or preserve data on failure.
- Drop oldest data when queue is full.

## Unit Tests

Existing tests must pass:

```text
payload encoding
drop-oldest queue
Wi-Fi state machine
```

No real HTTP/Wi-Fi unit tests.

## Manual Integration Checks

- Fake payload upload works.
- Real `Measurement` upload works.
- Wi-Fi disconnect does not stop sampling.
- Queue fills during disconnect.
- Queue drains after reconnect.
- Queue drops oldest when full.

## Done When

- Board can collect and upload measurements continuously.
- Upload failure does not crash firmware.

## Git Commit Message

```text
feat: add measurement upload task with offline queue
```

---

# Phase 15: System Hardening

## Goal

Make the firmware stable enough for overnight use.

## Work Items

- Remove all non-critical `unwrap`.
- Add error flags.
- Add timeout handling.
- Add LED status mapping.
- Reduce excessive logging.
- Verify no task blocks indefinitely.

## Unit Tests

All previous tests must pass.

Add tests if new pure logic is introduced.

## Manual Integration Checks

- Run continuously for several hours.
- Unplug and reconnect Wi-Fi router.
- Cover/uncover light sensor.
- Generate sound near microphone.
- Confirm no reset loop.

## Done When

- Firmware can run overnight.
- Failure modes are visible through status output or LEDs.
- No known blocking failure path remains.

## Git Commit Message

```text
fix: harden runtime error handling and status reporting
```

---

# Final Required Unit Test Checklist

All must be automated and hardware-free.

```text
[ ] ErrorFlags insert / contains
[ ] SHT40 CRC
[ ] SHT40 raw temperature conversion
[ ] SHT40 raw humidity conversion
[ ] SHT40 CRC failure handling
[ ] OPT3001 raw_to_lux
[ ] Mic mean / RMS / peak
[ ] Mic clip_count
[ ] Mic empty input handling
[ ] DropOldestQueue normal push/pop
[ ] DropOldestQueue full queue drops oldest
[ ] status_to_leds normal state
[ ] status_to_leds sensor error
[ ] status_to_leds Wi-Fi/upload error
[ ] merge_measurement field mapping
[ ] merge_measurement error flag merge
[ ] measurement_to_csv_line normal case
[ ] measurement_to_csv_line missing values
[ ] measurement_to_csv_line buffer too small
[ ] wifi state transition
[ ] wifi backoff calculation
```