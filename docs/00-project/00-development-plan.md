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
firmware/src/drivers/sht40.rs
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
firmware/src/drivers/opt3001.rs
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
firmware/src/drivers/mic.rs
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
firmware/src/util/queue.rs
firmware/src/util/status.rs
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
firmware/src/tasks/aggregator.rs
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
firmware/src/tasks/upload.rs
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
firmware/src/tasks/wifi.rs
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

In `firmware/src/bin/main.rs`:

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

# Phase 16: Persistent Spool Design

## Goal

Define a hardware-independent persistent measurement spool for internal SPI flash.

The spool must survive reset and power loss, preserve upload order, and keep the newest data when storage is full.

## Work Items

- Add `storage/spool.rs`.
- Define the flash record header:
  - magic
  - version
  - flags
  - header length
  - sequence
  - payload length
  - payload CRC
- Encode `Measurement` payloads using the existing CSV payload encoder.
- Add pure append / peek / acknowledge state logic.
- Add CRC validation and corrupt-record handling.
- Add full-spool behavior that drops oldest records to make room for newest records.
- Add explicit storage status/error representation.

## Unit Tests

All tests must run on host without ESP32 hardware.

Add tests for:

```text
spool record encodes and decodes
spool rejects bad magic
spool rejects unsupported version
spool rejects bad CRC
append preserves FIFO order
ack removes only the oldest uploaded record
full spool drops oldest records
partial tail record is ignored during recovery
corrupt middle record can be skipped or reported without panic
sequence wrap handling is defined and tested
```

## Manual Integration Checks

None. This phase is pure logic only.

## Done When

- The persistent spool format is documented in code and architecture.
- Recovery and full-storage behavior are deterministic.
- Host tests cover the record format and queue state transitions.

## Git Commit Message

```text
test: add persistent spool record logic
```

---

# Phase 17: Flash Storage Model

## Goal

Model SPI flash constraints before touching hardware flash.

## Work Items

- Add an in-memory flash model for tests.
- Enforce flash-like behavior:
  - erased bytes read as `0xff`
  - writes may only clear bits
  - erase works at sector granularity
  - reads/writes outside the configured region fail
- Implement the spool over the storage model.
- Recover head/tail pointers by scanning the modeled flash region.
- Keep all storage logic independent of Embassy tasks.

## Unit Tests

Add tests for:

```text
write requires erased space
erase resets a sector to 0xff
out-of-range read/write/erase fails
append across sector boundary
recover multiple records after simulated reboot
recover after interrupted append
drop oldest after modeled flash fills
ack persists across simulated reboot
```

## Manual Integration Checks

None. This phase is pure logic only.

## Done When

- The spool works against a flash-like storage model.
- Reboot recovery is covered by host tests.
- No ESP32 flash writes are needed for this phase.

## Git Commit Message

```text
test: model persistent measurement flash spool
```

---

# Phase 18: ESP32-C3 Flash Region Bring-Up

## Goal

Safely access a dedicated internal SPI flash region on the ESP32-C3.

## Work Items

- Decide and document the flash spool region or partition layout for the 4 MB module.
- Add board constants for spool offset and size.
- Add `drivers/flash.rs` as the ESP32-C3 flash-region adapter.
- Refuse runtime flash access if the configured region is zero-sized or out of bounds.
- Add a hardware-only smoke path that can:
  - read the region
  - erase one test sector
  - write one test record
  - read it back
- Ensure the smoke path cannot run against bootloader, partition table, app, or calibration regions.

## Unit Tests

Host tests should cover:

```text
flash region range validation
sector alignment validation
storage region rejects zero size
storage region rejects app-overlap configuration
```

No host unit test should require real flash.

## Manual Integration Checks

- Build for ESP32-C3.
- Upload to the board with USB/JTAG.
- Run the flash smoke test.
- Confirm RTT logs show successful erase/write/readback.
- Confirm the board still boots after reset.

## Done When

- The firmware can safely read/write a dedicated internal SPI flash region.
- The flash smoke test passes on hardware.
- No application or bootloader flash region is touched.

## Git Commit Message

```text
feat: bring up internal flash spool region
```

---

# Phase 19: Persistent Spool Task Integration

## Goal

Use the persistent spool as the upload backlog.

## Work Items

- Add `tasks/storage.rs`.
- Replace direct aggregator-to-uploader queue ownership with a storage task interface.
- On boot, recover pending records from internal SPI flash before normal upload draining.
- Append each new `Measurement` to the persistent spool.
- Keep a RAM hot queue for fresh records and quick upload access.
- Make `uploader_task` read the oldest pending record from storage.
- Acknowledge records only after HTTP 2xx.
- Preserve records on upload failure, Wi-Fi disconnect, or reset.
- Report storage errors through status output and LED2 policy.

## Unit Tests

Existing tests must pass.

Add host tests for:

```text
storage task model appends measurements in order
upload success acknowledges exactly one record
upload failure preserves record
recovered records upload before newly appended records
full persistent spool drops oldest record
storage error sets error status
```

No real HTTP/Wi-Fi/flash unit tests.

## Manual Integration Checks

- Run with upload receiver available; queue drains.
- Stop upload receiver; measurements continue and persist.
- Reset board while receiver is stopped.
- Restart receiver; recovered records upload first.
- Disconnect Wi-Fi; sampling and storage continue.
- Reconnect Wi-Fi; backlog drains.

## Done When

- Pending measurements survive reset.
- Upload acknowledgement is tied to HTTP 2xx only.
- Wi-Fi/upload failure does not stop sampling or persistent storage.

## Git Commit Message

```text
feat: persist measurement backlog in internal flash
```

---

# Phase 20: Persistent Storage Recovery And Soak

## Goal

Validate the persistent spool under realistic overnight failure modes.

## Work Items

- Add storage metrics to RTT/status output:
  - pending record count
  - dropped-oldest count
  - recovered record count
  - corrupt record count
  - last storage error
- Reduce storage metrics log volume for overnight runs.
- Document manual recovery procedures in `01-walkthrough.md`.
- Harden any observed blocking or reset-loop paths.

## Unit Tests

All previous tests must pass.

Add tests only if new pure logic is introduced.

## Manual Integration Checks

- Run for several hours with receiver online.
- Run with receiver offline long enough to fill part of the flash spool.
- Reset board while receiver is offline.
- Confirm pending records upload after receiver returns.
- Force at least one full-spool condition and confirm oldest records are dropped.
- Interrupt power during or near a write and confirm the next boot recovers without reset loop.
- Confirm LED2/status output reports storage or upload failures.

## Done When

- Firmware can run overnight with persistent upload backlog.
- Reset does not lose already persisted pending records.
- Corrupt or partial records do not crash firmware.
- Failure modes are visible through status output or LEDs.

## Git Commit Message

```text
fix: harden persistent spool recovery
```

---

# Phase 21: Firmware Configuration Consolidation

## Goal

Centralize firmware deployment and policy constants without changing runtime behavior.

## Work Items

- Add:

```text
firmware/src/config.rs
```

- Re-export the module from `firmware/src/lib.rs`.
- Move deployment and policy values into config:
  - Wi-Fi SSID and authentication defaults.
  - REST upload fallback endpoint, port, path, user-agent, retry delay, timeouts, and buffers.
  - Network stack resource sizing and DHCP default.
  - Sensor and microphone timing policy.
  - Storage payload, queue, and metrics tuning.
  - Logging and LED timing policy.
- Keep board facts and protocol constants outside config:
  - GPIO pins, I2C addresses, and flash layout remain in `board.rs`.
  - Sensor commands, register constants, and conversion math remain in drivers.
  - Spool magic/version/header constants remain in `storage/spool.rs`.
- Update [../10-firmware/04-configuration.md](../10-firmware/04-configuration.md) if implementation changes the config boundary.

## Unit Tests

All previous tests must pass.

Add tests only for new pure config selection or validation logic.

## Manual Integration Checks

No hardware validation is required if behavior is preserved.

If flash-write validation is unexpectedly needed, state the exact flash range before running it.

## Done When

- Task-local deployment and policy hardcoding is removed.
- Existing REST upload and temporary receiver compatibility are preserved.
- Firmware builds and tests pass with the standard verification commands.

## Git Commit Message

```text
refactor: centralize firmware configuration constants
```

---

# Phase 22: REST Network, Discovery, Time, And BLE Readiness

## Goal

Replace the current catch-all network/upload path with clear REST networking responsibilities while adding server discovery and real-world time support.

Phase 22 uses the versioned JSON REST API only. The old bring-up
`/measurements` CSV receiver compatibility is intentionally not preserved.

## Work Items

- Keep REST as the primary upload protocol; do not add MQTT.
- Replace CSV upload with JSON schema version 1 at `POST /api/v1/measurements`.
- Split responsibilities for:
  - Wi-Fi link state.
  - IP/DHCP readiness.
  - REST endpoint resolution and discovery.
  - HTTP transport and response classification.
  - Upload orchestration and storage acknowledgement.
- Keep storage acknowledgement tied to HTTP 2xx only.
- Add automatic server discovery with static configured endpoint fallback:
  - UDP discovery port `39022`.
  - Query payload `sleep-environment-monitor.discovery`.
  - Discovery metadata at `GET /.well-known/sleep-environment-monitor`.
- Add real-world time support:
  - Prefer SNTP/NTP after IP configuration if practical.
  - Use `GET /api/v1/time` as REST fallback.
  - Preserve `uptime_ms` for all measurements.
  - Add wall-clock timestamp fields only when synchronized.
- Support common Wi-Fi credential modes through config:
  - Open networks.
  - WPA-Personal.
  - WPA2-Personal.
  - WPA/WPA2-Personal mixed mode.
- Defer WPA3 and Enterprise/EAP Wi-Fi until the dependency stack and target hardware are validated for those modes.
- Shape config and interfaces so future BLE provisioning can provide Wi-Fi and REST endpoint settings.
- Update [../10-firmware/03-network.md](../10-firmware/03-network.md), [../20-server/01-rest-api.md](../20-server/01-rest-api.md), and [../30-integration/00-network-roadmap.md](../30-integration/00-network-roadmap.md) as implementation decisions become concrete.

## Unit Tests

Add hardware-independent tests for:

- Endpoint resolution precedence.
- Discovery fallback behavior.
- HTTP response and upload error classification.
- Timestamp selection before and after wall-clock sync.
- Storage acknowledgement only after upload success.

## Manual Integration Checks

- Wi-Fi connects and reconnects.
- Firmware obtains IP configuration.
- Static fallback REST upload still works.
- Automatic discovery finds the server when available.
- Receiver/server outage preserves pending records.
- Server return drains records in order.
- Time synchronization is visible in logs and payloads.
- Sampling and persistent storage continue while network features fail.

## Done When

- Network status distinguishes Wi-Fi, IP, discovery, time, transport, and HTTP failures.
- Measurements upload through REST with HTTP-2xx-only acknowledgement.
- Firmware can attach wall-clock time when synchronized and still upload uptime-only records otherwise.
- Password-protected personal Wi-Fi networks can be configured for the common WPA/WPA2 PSK modes listed above.
- BLE provisioning can be added later without reshaping the config model.

## Git Commit Message

```text
feat: improve REST network discovery and time sync
```

---

# Phase 23: Formal Server Foundation

## Goal

Replace the temporary stdlib-only Phase 22 receiver with a formal Python server
foundation while preserving the Phase 22 firmware/server API contract.

The formal server should keep REST as the primary protocol and continue to
support:

- `POST /api/v1/measurements`.
- `GET /api/v1/time`.
- `GET /.well-known/sleep-environment-monitor`.
- UDP discovery on port `39022` with query payload
  `sleep-environment-monitor.discovery`.

The old `/measurements` CSV endpoint remains out of scope.

## Work Items

- Define the server package layout under `server/`.
- Replace or supersede `server/post_receiver.py` with a formal application
  entrypoint.
- Use a web framework instead of hand-written `http.server` request handling.
  The planned default is FastAPI with Uvicorn and Pydantic models.
- Add an `argparse` CLI surface for serving, configuration checks, and discovery
  metadata inspection.
- Add Rich-based human-readable console output for local operation and
  diagnostics.
- Define server toolchain, code style, test strategy, and check commands in
  [../20-server/02-toolchain.md](../20-server/02-toolchain.md).
- Define CLI behavior in [../20-server/03-cli.md](../20-server/03-cli.md).
- Keep the Phase 22 REST API contract documented in
  [../20-server/01-rest-api.md](../20-server/01-rest-api.md).

## Unit Test Coverage Requirements

All server unit tests must be automated and hardware-free.

Required coverage:

- CLI argument parsing:
  - Default host, HTTP port, and UDP discovery port.
  - Explicit host, HTTP port, and UDP discovery port.
  - Log level selection.
  - Rich output enable/disable switch.
  - Invalid port and invalid log-level rejection.
- Application configuration:
  - Defaults match the firmware fallback environment.
  - CLI overrides are applied deterministically.
  - Discovery metadata derives from the active configuration.
- REST API behavior:
  - `POST /api/v1/measurements` accepts valid schema-version-1 JSON.
  - Invalid JSON returns non-2xx.
  - Missing required measurement fields return non-2xx.
  - Other `POST` paths return `404`.
  - `GET /api/v1/time` returns integer `unix_ms` and source metadata.
  - `GET /.well-known/sleep-environment-monitor` returns paths and UDP port.
- Measurement model validation:
  - Required identity and sequence fields.
  - `time_status` allowed values.
  - Optional `wall_clock_unix_ms`.
  - Nullable sensor values.
  - Duplicate `(device_id, sequence)` acceptance or idempotent handling policy.
- UDP discovery logic:
  - Correct query payload is accepted.
  - Wrong query payload is ignored.
  - Response contains `host`, `port`, `api_base`, `measurement_upload`, and
    `time`.
  - Response host selection is deterministic in tests.
- Logging/output:
  - Human-readable Rich output can be enabled.
  - Machine-readable or plain output path remains testable.
  - Upload acceptance logs include source and payload size or equivalent
    diagnostic metadata without dumping unbounded payloads.

## Unit Test Quality Requirements

- Tests must assert externally visible behavior rather than implementation
  details unless the unit under test is explicitly a pure helper.
- Tests must use deterministic time sources where time values are asserted.
- Tests must avoid real network dependencies; use framework test clients,
  fake sockets, or isolated loopback fixtures.
- Tests must avoid sleeps except when explicitly validating timeout behavior;
  timeout behavior should prefer fake clocks or short controlled fixtures.
- Tests must not depend on test execution order.
- Tests must name the behavior being validated, not only the function being
  called.
- Test fixtures should be small and local. Shared fixtures are acceptable only
  when they reduce duplication without hiding important setup details.
- Regression tests must be added for every hardware or integration bug that can
  be reproduced without hardware.

## Style And Tooling Policy

- Comments and docstrings use Google style.
- Type hints are expected for public server functions and data models.
- Formatter and linter output is advisory only.
- Never automatically apply formatter or linter rewrites across server code.
- Do not run auto-fix or auto-format commands as an implementation step or
  commit-preparation shortcut.
- Formatter and linter commands are check-only gates. Review each suggestion
  manually before editing code.
- Narrow suppression markers such as formatter-disable regions or line-level
  linter ignores are allowed when they preserve intentional readability,
  especially manually aligned tabular data, protocol examples, or dense mapping
  tables where extra whitespace is deliberate and automatic formatting would
  make the code worse.
- Suppression markers must be local to the smallest practical block and should
  include a short reason when the reason is not obvious.

## Manual Integration Checks

- Start the formal server on `0.0.0.0:8080`.
- Confirm the server prints clear Rich console startup information.
- Confirm `GET /api/v1/time` and
  `GET /.well-known/sleep-environment-monitor` work from the host.
- Confirm UDP discovery on port `39022` responds to
  `sleep-environment-monitor.discovery`.
- Run the ESP32-C3 firmware against the formal server and confirm:
  - Wi-Fi connects.
  - Discovery finds the server.
  - Time sync succeeds.
  - JSON measurements upload through `POST /api/v1/measurements`.
  - HTTP 2xx remains the only storage ACK condition.

## Done When

- Server documentation describes the formal app structure, toolchain, CLI,
  style policy, and test expectations.
- The formal server runs through an `argparse` CLI.
- The temporary `post_receiver.py` is replaced or clearly demoted to a legacy
  smoke tool.
- The Phase 22 firmware/server API contract remains compatible.
- Automated server tests cover CLI parsing, REST behavior, model validation,
  UDP discovery helpers, and logging/output behavior.
- Check commands are documented and run without requiring hardware.

## Git Commit Message

```text
feat: add formal server foundation
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
[ ] measurement_to_json_fields normal case
[ ] measurement_to_json_fields missing values
[ ] measurement_to_json_fields buffer too small
[ ] wifi state transition
[ ] wifi backoff calculation
[ ] spool record encode / decode
[ ] spool bad magic / bad version rejection
[ ] spool CRC failure handling
[ ] spool append / peek / acknowledge order
[ ] spool recovery after simulated reboot
[ ] spool interrupted append recovery
[ ] spool full behavior drops oldest
[ ] flash model erased-byte behavior
[ ] flash model write-without-erase failure
[ ] flash region range validation
```
