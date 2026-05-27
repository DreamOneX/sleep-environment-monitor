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
- Report storage errors through status output and the current LED status policy
  (currently blue LED3 for firmware status after Phase 24P; red LED2 remains
  heartbeat).

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
- Confirm status output and the current status LED report storage or upload
  failures (currently blue LED3 after Phase 24P; red LED2 remains heartbeat).

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
- Shape config and interfaces so future BLE upload can be enabled without
  reshaping Wi-Fi, REST, storage, or acknowledgement ownership.
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
- BLE upload can be added later without reshaping the storage or network
  configuration model.

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

# Phase 24: BLE Independent Upload Channel

## Goal

Add a real Bluetooth Low Energy upload path that can operate independently from
Wi-Fi.

BLE is not a provisioning-only feature in this phase. It is also not Bluetooth
Classic SPP, a transparent UART, or a Nordic UART Service style serial stream.
The firmware must expose a project-specific structured GATT service for
measurement transfer.

Phase 24 completion also requires human-visible BLE operation feedback on
LED3. Phase 24P adds the compile/unit-tested LED3 BLE status boundary, but
hardware visual acceptance of the blue LED patterns remains open.

## Phase 24A: BLE Compile Integration Boundary

Phase 24A is the first implementation slice. It proves that the BLE code path
compiles with the existing Wi-Fi firmware and coexistence feature enabled, but
it does not accept BLE runtime behavior.

Phase 24A scope:

- Add `ble-upload` and `radio-coex` firmware features.
- Keep default firmware behavior unchanged with BLE disabled.
- Define project BLE protocol/status/metadata/fragment/control helper types.
- Add a `ble_task` boundary that can own
  `esp_radio::ble::controller::BleConnector` on ESP32-C3.
- Preserve the existing Wi-Fi initialization, network stack, and REST uploader
  in BLE-enabled builds.
- Do not implement or validate advertising, pairing, GATT reads/writes,
  notifications, BLE record transfer, or BLE storage ACK.
- Do not change flash format, measurement JSON payload shape, or
  `storage_task` acknowledgement semantics.

Phase 24A verification:

```bash
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf
cargo clippy --all-targets
cargo clippy --target riscv32imc-unknown-none-elf
cargo build --target riscv32imc-unknown-none-elf --features ble-upload
cargo clippy --target riscv32imc-unknown-none-elf --features ble-upload
```

Phase 24A commit message:

```text
feat: add BLE compile integration boundary
```

## Phase 24B: BLE Transfer And ACK Core

Phase 24B adds hardware-independent transfer and acknowledgement core logic.
It still does not accept BLE runtime behavior because no GATT host/server,
advertising, pairing, or central connection path is active yet.

Phase 24B scope:

- Preserve the default non-BLE firmware path.
- Keep Wi-Fi REST upload and BLE upload routed as separate storage clients.
- Make storage acknowledgements sequence-checked so stale Wi-Fi or BLE ACKs
  cannot delete a newer oldest pending record.
- Expose stored payload flags to upload clients without changing the flash
  record format or measurement JSON payload shape.
- Add a BLE transfer session model for oldest-record metadata, ordered
  fragments, complete-record confirmation, disconnect reset, and ACK decision.
- Keep BLE ACK behavior pure; do not send `StorageCommand::Ack` from BLE
  runtime code in this slice.
- Do not implement or validate advertising, pairing, GATT reads/writes,
  notifications, BLE record transfer, or BLE storage drain on hardware.

Phase 24B verification:

```bash
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf
cargo clippy --all-targets
cargo clippy --target riscv32imc-unknown-none-elf
cargo build --target riscv32imc-unknown-none-elf --features ble-upload
cargo clippy --target riscv32imc-unknown-none-elf --features ble-upload
```

Phase 24B commit message:

```text
feat: add BLE transfer ACK core
```

## Phase 24C: BOOT / IO9 Pairing Gesture Core

Phase 24C adds the hardware-independent pairing-window gesture core and the
target-side GPIO input boundary for BOOT / IO9. It still does not accept BLE
runtime behavior because the pairing window is not connected to a GATT security
or authorization path yet.

Phase 24C scope:

- Preserve the default non-BLE firmware path.
- Read BOOT / IO9 only when building with `--features ble-upload`.
- Configure BOOT / IO9 as an input with the MCU internal pull-up explicitly
  enabled at runtime. The current board has no discrete IO9 pull-up, and the
  ESP32-C3 boot/strap weak pull-up must not be assumed to remain configured
  after firmware starts.
- Add a pure active-low BOOT button model.
- Add a long-press pairing-window state machine with tests for short press,
  long press, release/retrigger behavior, and timeout.
- Log pairing-window open/expire events from the BLE task boundary.
- Do not configure IO9 as an output.
- Do not add an internal pull-down. Treat the current IO9-to-GND capacitor in
  parallel with BOOT as a hardware fact that requires download-mode and runtime
  release validation on the actual board.
- Do not implement or validate advertising, GATT security, pairing, bonded
  state, authorization, BLE record transfer, or BLE storage drain on hardware.

Phase 24C verification:

```bash
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf
cargo clippy --all-targets
cargo clippy --target riscv32imc-unknown-none-elf
cargo build --target riscv32imc-unknown-none-elf --features ble-upload
cargo clippy --target riscv32imc-unknown-none-elf --features ble-upload
```

Phase 24C commit message:

```text
feat: add BLE pairing gesture core
```

## Phase 24D: BLE GATT Runtime Skeleton

Phase 24D adds the first real BLE GATT host/server runtime skeleton. It accepts
BLE advertising and a status characteristic as compile-validated runtime shape,
but it still does not accept full BLE upload behavior because pairing,
authorization, record transfer, and BLE storage ACK are not connected yet.

Phase 24D scope:

- Preserve the default non-BLE firmware path.
- Keep the TrouBLE host dependency enabled only by `ble-upload`.
- Run a real BLE peripheral host on top of
  `esp_radio::ble::controller::BleConnector`.
- Advertise a project-specific GATT service, not Bluetooth Classic SPP, a
  transparent UART, or Nordic UART Service style streaming.
- Define GATT characteristics for status, record metadata, record fragment, and
  control.
- Keep the status characteristic readable and update it with BLE runtime state.
- Keep record metadata, record fragment, and control characteristic access
  disabled until pairing, authorization, transfer, and ACK handling are
  implemented.
- Keep BOOT / IO9 pairing-window monitoring active while GATT advertising and
  connection waits run.
- Do not send `StorageCommand::Ack` from BLE runtime code in this slice.
- Do not implement or validate real pairing/security, live BLE record transfer,
  or BLE storage drain on hardware.

Phase 24D verification:

```bash
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf
cargo clippy --all-targets
cargo clippy --target riscv32imc-unknown-none-elf
cargo build --target riscv32imc-unknown-none-elf --features ble-upload
cargo clippy --target riscv32imc-unknown-none-elf --features ble-upload
```

Phase 24D commit message:

```text
feat: add BLE GATT runtime skeleton
```

## Phase 24E: BLE Authorized Record Read Skeleton

Phase 24E connects the BOOT / IO9 pairing window to the project GATT record
characteristics and adds a read-only oldest-record transfer path. It accepts
compile-validated GATT access control and storage peek wiring, but it still
does not accept full BLE upload behavior because BLE storage acknowledgement
and live hardware validation are not complete yet.

Phase 24E scope:

- Preserve the default non-BLE firmware path.
- Keep Wi-Fi REST upload and BLE upload routed as separate storage clients.
- Share the BOOT / IO9 pairing-window state with the GATT task.
- Reject unpaired record metadata, record fragment, and control access with ATT
  authorization errors.
- Let authorized GATT control/metadata requests read the oldest pending record
  through `storage_task` using `StorageCommand::Peek(StorageClient::Ble)`.
- Prepare structured metadata and ordered record fragments in the existing
  project GATT characteristics.
- Treat `CompleteRecord` as a transfer-session marker only.
- Explicitly reject `AckRecord`; do not send `StorageCommand::Ack` from BLE
  runtime code in this slice.
- Do not change flash format, measurement JSON payload shape, or Wi-Fi upload
  acknowledgement behavior.
- Do not validate live BLE advertising, central connection behavior, GATT
  transfer, notifications, or BOOT / IO9 behavior on hardware in this slice.

Phase 24E verification:

```bash
cargo fmt
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf
cargo clippy --all-targets
cargo clippy --target riscv32imc-unknown-none-elf
cargo build --target riscv32imc-unknown-none-elf --features ble-upload
cargo clippy --target riscv32imc-unknown-none-elf --features ble-upload
```

Phase 24E commit message:

```text
feat: add BLE authorized record read skeleton
```

## Phase 24F: BLE Runtime ACK Wiring

Phase 24F connects the BLE GATT `AckRecord` control path to the existing
sequence-checked storage acknowledgement command. It accepts compile-validated
runtime ACK wiring, but it still does not accept full BLE upload completion
because the BLE central flow, Wi-Fi/BLE race behavior, and BOOT / IO9 entry
have not been hardware-validated yet.

Phase 24F scope:

- Preserve the default non-BLE firmware path.
- Keep Wi-Fi REST upload and BLE upload routed as separate storage clients.
- Add a shared latest network/upload status snapshot so BLE can evaluate ACK
  policy without consuming the existing single-consumer status `Signal`s used
  by the LED/status task.
- Keep Wi-Fi and uploader tasks publishing their existing status `Signal`s.
- Update the shared snapshot from Wi-Fi and uploader state transitions.
- On authorized `AckRecord`, use the existing BLE transfer session ACK policy
  with the shared network/upload status snapshot.
- Suppress BLE storage ACK while Wi-Fi upload is connected/IP-ready and the
  last upload result is success.
- Send `StorageCommand::Ack { client: StorageClient::Ble, sequence }` only
  after complete-record confirmation when the policy permits BLE ACK.
- Rely on `storage_task` sequence checking so stale BLE ACKs cannot delete a
  different oldest pending record.
- Do not change flash format, measurement JSON payload shape, or Wi-Fi upload
  acknowledgement behavior.
- Do not validate live BLE advertising, central connection behavior, GATT
  transfer, notifications, storage drain, or BOOT / IO9 behavior on hardware in
  this slice.

Phase 24F verification:

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

Phase 24F commit message:

```text
feat: add BLE runtime ACK wiring
```

## Phase 24G: Independent Radio Feature Selection

Phase 24G makes BLE and Wi-Fi upload feature selection independently
compile-selectable. It accepts compile-validated feature boundaries, but it
still does not accept full BLE upload completion because live BLE central flow,
Wi-Fi/BLE race behavior, and BOOT / IO9 entry still need hardware validation.

Phase 24G scope:

- Preserve the default firmware behavior with Wi-Fi REST upload enabled and BLE
  disabled.
- Add a default `wifi-upload` feature that selects `esp-radio/wifi`.
- Keep `ble-upload` selecting project BLE code, TrouBLE, and `esp-radio/ble`
  without forcing Wi-Fi on.
- Make `radio-coex` an explicit BLE+Wi-Fi coexistence feature that selects
  `ble-upload`, `wifi-upload`, and `esp-radio/coex`.
- Do not enable `esp-radio/coex` in BLE-only builds because `esp-radio 0.18.0`
  references its Wi-Fi module when coexistence is enabled.
- Gate target-side Wi-Fi radio setup, DHCP runner, and REST uploader startup on
  `wifi-upload`.
- When `wifi-upload` is disabled, leave sensor sampling, aggregation, storage,
  status LED task, and optional BLE task running with disconnected/idle network
  upload status.
- Keep flash format, measurement JSON payload shape, and storage ACK semantics
  unchanged.
- Do not validate live BLE advertising, central connection behavior, GATT
  transfer, notifications, storage drain, or BOOT / IO9 behavior on hardware in
  this slice.

Phase 24G verification:

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

Phase 24G commit message:

```text
feat: add independent radio feature selection
```

## Phase 24H: BLE Status Runtime Snapshot

Phase 24H makes the BLE status characteristic carry the current firmware
status fields that the structured protocol already defined. It accepts
compile-validated status wiring, but it still does not accept full BLE upload
completion because live BLE central flow, Wi-Fi/BLE race behavior, and BOOT /
IO9 entry still need hardware validation.

Phase 24H scope:

- Preserve the default firmware behavior with Wi-Fi REST upload enabled and BLE
  disabled.
- Keep the existing LED/status `Signal`s for the status LED task.
- Add a shared firmware status snapshot for BLE status reads without consuming
  single-consumer status `Signal`s.
- Publish pending record count updates from `storage_task` after recovery,
  append, and ACK paths.
- Publish the latest error flags from aggregation/storage failure paths.
- Refresh the BLE status characteristic from the latest BLE runtime state,
  network/upload snapshot, pending record count, and error flags before status
  reads and on BLE runtime state transitions.
- Keep flash format, measurement JSON payload shape, and storage ACK semantics
  unchanged.
- Do not validate live BLE advertising, central connection behavior, GATT
  transfer, notifications, storage drain, or BOOT / IO9 behavior on hardware in
  this slice.

Phase 24H verification:

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

Phase 24H commit message:

```text
feat: add BLE status runtime snapshot
```

## Phase 24I: BLE Advertising Runtime Bring-Up

Phase 24I fixes the first hardware-observed BLE advertising startup issue. It
accepts board-side evidence that the BLE host enters advertising, but it still
does not accept full BLE upload completion because central-side discovery,
connection, GATT status reads, record transfer, ACK behavior, and BOOT / IO9
hardware validation still need end-to-end validation.

Phase 24I scope:

- Preserve the default firmware behavior with Wi-Fi REST upload enabled and
  BLE disabled.
- Keep the BLE+Wi-Fi coexistence build using the project GATT service and
  structured characteristics.
- Keep legacy advertising data and scan response data within the 31-byte BLE
  payload limit.
- Advertise flags and the project 128-bit service UUID in the advertising
  payload.
- Advertise the complete local name in the scan response payload.
- Add a hardware-independent regression test for the legacy advertising and
  scan response payload sizes.
- Flash a BLE+Wi-Fi build and confirm by RTT that the firmware reaches the BLE
  advertising loop.
- Do not treat a board-side advertising log as proof of central-side discovery,
  pairing/authorization, GATT transfer, BLE storage drain, or BOOT / IO9
  behavior.

Phase 24I verification:

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
git diff --check
```

Phase 24I commit message:

```text
fix: fit BLE advertising payloads
```

## Phase 24J: BLE Central Status And Closed-Window Authorization

Phase 24J validates the first central-side BLE interactions against the
advertising firmware from Phase 24I. It accepts central discovery, connection,
structured status reads, and closed-window authorization rejection for
measurement access, but it still does not accept full BLE upload completion
because authorized record transfer, BLE storage ACK, Wi-Fi/BLE race behavior,
and BOOT / IO9 hardware entry still need end-to-end validation.

Phase 24J scope:

- Preserve the default firmware behavior with Wi-Fi REST upload enabled and
  BLE disabled.
- Use the BLE+Wi-Fi coexistence firmware flashed in Phase 24I; do not require a
  new firmware flash for this validation slice.
- Confirm a Windows BLE central can discover the project advertising service
  UUID and scan-response local name.
- Confirm a Windows BLE central can connect, discover the project GATT service,
  and read the structured status characteristic.
- Confirm the status characteristic reports the BLE runtime state, network
  state, upload result, pending-record count, and error flags using the Phase
  24H binary status frame.
- Confirm that with the BOOT / IO9 pairing window closed, measurement metadata
  reads, fragment reads, and control writes are rejected with ATT
  authorization errors.
- Do not validate authorized record transfer, notifications, BLE storage ACK,
  Wi-Fi/BLE ACK race behavior, BOOT / IO9 pairing entry, or download-mode
  behavior in this slice.

Phase 24J verification:

```bash
'/mnt/c/Program Files/dotnet/dotnet.exe' build "$(wslpath -w tools/ble-watch/ble-watch.csproj)"
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-read-status 30 sleep-env-esp32c3
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-closed-window 30 sleep-env-esp32c3
git diff --check
```

Phase 24J commit message:

```text
test: validate BLE central status access
```

## Phase 24K: BLE Pairing Window Entry Diagnostics

Phase 24K validates BOOT / IO9 runtime entry for the authorization window with
central-readable status diagnostics. It accepts the BOOT / IO9 input path and
pairing-window state machine on hardware, but it still does not accept full BLE
upload completion because authorized record transfer, BLE storage ACK,
Wi-Fi/BLE race behavior, and download-mode preservation still need end-to-end
validation.

Phase 24K scope:

- Preserve the default firmware behavior with Wi-Fi REST upload enabled and
  BLE disabled.
- Keep the existing 10-byte BLE status prefix stable for protocol version, BLE
  runtime state, network state, upload result, pending-record count, and error
  flags.
- Append pairing diagnostics to the BLE status frame: pairing state, BOOT /
  IO9 button state, pairing-window remaining milliseconds, and accumulated
  BOOT press milliseconds.
- Build and flash a BLE+Wi-Fi coexistence diagnostic image.
- Confirm a Windows BLE central can read the 20-byte status frame.
- Confirm BOOT / IO9 is read as an active-low runtime input and that a long
  press opens the pairing window.
- Confirm that after the pairing window expires, the same continuous press does
  not reopen the window until BOOT / IO9 is released and pressed again.
- Do not validate authorized record transfer, notifications, BLE storage ACK,
  Wi-Fi/BLE ACK race behavior, or download-mode behavior in this slice.

Phase 24K verification:

```bash
cargo fmt
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf --features ble-upload,radio-coex
'/mnt/c/Program Files/dotnet/dotnet.exe' build "$(wslpath -w tools/ble-watch/ble-watch.csproj)"
cargo espflash save-image --chip esp32c3 --flash-size 4mb --target riscv32imc-unknown-none-elf --features ble-upload,radio-coex --merge /tmp/phase24-ble-status-pressed-image.bin
cargo espflash flash --target riscv32imc-unknown-none-elf --features ble-upload,radio-coex --chip esp32c3 --port /dev/ttyACM0 --before usb-reset --non-interactive --flash-size 4mb
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-watch-status 30 sleep-env-esp32c3 60
cargo build --target riscv32imc-unknown-none-elf
cargo clippy --all-targets
cargo clippy --target riscv32imc-unknown-none-elf
cargo clippy --target riscv32imc-unknown-none-elf --features ble-upload,radio-coex
git diff --check
```

Phase 24K commit message:

```text
test: validate BLE pairing entry diagnostics
```

## Phase 24L: BLE Record Transfer And ACK Hardware Validation

Phase 24L validates the first full BLE record transfer and BLE storage ACK
path with the Windows central tool. It accepts central-side metadata reads,
ordered fragment reads, CRC validation, `CompleteRecord`, and an ACK-mode
storage drain while Wi-Fi upload is unavailable, but it still does not accept
full BLE upload completion because post-ACK oldest-record advancement,
Wi-Fi/BLE ACK race behavior, notification behavior, disconnect preservation,
and BOOT download-mode preservation still need validation.

Phase 24L scope:

- Preserve the default firmware behavior with Wi-Fi REST upload enabled and
  BLE disabled.
- Use the BLE+Wi-Fi coexistence diagnostic firmware from Phase 24K; do not
  require a new firmware flash for this validation slice.
- Keep the Windows BLE central validation tool in
  `tools/ble-watch`.
- Confirm an authorized `scan-transfer-record ... no-ack` run reads metadata,
  reads all fragments, validates payload CRC, and accepts `CompleteRecord`
  without sending a BLE storage ACK.
- Confirm an authorized `scan-transfer-record ... ack` run reads metadata,
  reads all fragments, validates payload CRC, accepts `CompleteRecord`, and
  sends `AckRecord` when the BLE ACK policy permits drain.
- Before any ACK-mode hardware validation, declare that the firmware may
  exercise the measurement spool flash range `0x003c0000..0x00400000` through
  `storage_task`.
- Do not treat this slice as proof of Wi-Fi/BLE race behavior, notification
  delivery, disconnect preservation during live transfer, post-ACK oldest
  advancement, or BOOT download-mode preservation.

Phase 24L verification:

```bash
'/mnt/c/Program Files/dotnet/dotnet.exe' build "$(wslpath -w tools/ble-watch/ble-watch.csproj)"
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-transfer-record 30 sleep-env-esp32c3 no-ack 128
# Declare measurement spool range 0x003c0000..0x00400000 before this command.
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-transfer-record 30 sleep-env-esp32c3 ack 128
git diff --check
```

Phase 24L commit message:

```text
test: validate BLE record transfer ACK path
```

## Phase 24M: BLE Fragment Notification Hardware Validation

Phase 24M validates the fragment notification path with the Windows central
tool. It accepts subscription to the record-fragment characteristic and
central-observed notifications matching explicit fragment reads, but it still
does not accept full BLE upload completion because disconnect preservation,
post-ACK oldest-record advancement, Wi-Fi/BLE ACK race behavior, and BOOT
download-mode preservation still need validation.

Phase 24M scope:

- Rename the Windows BLE central validation tool to `tools/ble-watch`.
- Keep the BLE+Wi-Fi coexistence diagnostic firmware already on the board; do
  not require a new firmware flash for this validation slice.
- Add `scan-transfer-record-notify`, `scan-disconnect-preserves-record`, and
  `scan-ack-then-peek-next` commands to `ble-watch`.
- Confirm an authorized `scan-transfer-record-notify ... no-ack` run
  subscribes to fragment notifications, reads metadata, requests fragments,
  observes one notification per requested fragment, confirms each notification
  matches the corresponding fragment read, validates payload CRC, and completes
  the record without sending a BLE storage ACK.
- Do not treat this slice as proof of disconnect preservation during live
  transfer, post-ACK oldest advancement, Wi-Fi/BLE ACK race behavior, or BOOT
  download-mode preservation.

Phase 24M verification:

```bash
'/mnt/c/Program Files/dotnet/dotnet.exe' build "$(wslpath -w tools/ble-watch/ble-watch.csproj)"
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-read-status 30 sleep-env-esp32c3
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-transfer-record-notify 30 sleep-env-esp32c3 no-ack 128
git diff --check
```

Phase 24M commit message:

```text
test: validate BLE fragment notifications
```

## Phase 24N: Wi-Fi/BLE ACK Race Logic Guard

Phase 24N strengthens the hardware-independent ACK race coverage. It accepts
the storage-layer sequence guard that prevents a stale BLE ACK from deleting
the next oldest record after Wi-Fi has already acknowledged the raced record,
but it still does not accept full BLE upload completion because the live
Wi-Fi/BLE ACK race, disconnect preservation, post-ACK oldest-record
advancement, and BOOT download-mode preservation still need hardware
validation.

Phase 24N scope:

- Add a storage unit test that models Wi-Fi acknowledging the current oldest
  record, followed by BLE attempting to acknowledge the same stale sequence.
- Confirm the second ACK returns no acknowledgement and leaves the new oldest
  record pending.
- Keep this as pure storage/ACK semantics only. Do not treat it as hardware
  validation of simultaneous Wi-Fi and BLE runtime behavior.

Phase 24N verification:

```bash
cargo fmt
cargo test --lib
git diff --check
```

Phase 24N commit message:

```text
test: guard Wi-Fi BLE ACK race
```

## Phase 24O: BLE Authorization Metadata Auto-Pair Policy

Phase 24O adds a compile-validated flash metadata boundary for future BLE
authorization records and uses it to decide whether startup should open the
temporary authorization window. It still does not accept full BLE upload
completion because real persisted bonding/authorization records, live
disconnect preservation, post-ACK oldest-record advancement, live Wi-Fi/BLE ACK
race behavior, and BOOT download-mode preservation still need validation.

Phase 24O scope:

- Reserve `0x003bf000..0x003c0000` as a 4 KiB BLE authorization metadata
  sector immediately before the measurement spool.
- Keep the measurement spool at `0x003c0000..0x00400000` and keep its record
  format and measurement JSON payload shape unchanged.
- Add a hardware-independent BLE authorization header with magic, header
  format version, authorization-record-set version, record count,
  record-set checksum, and header checksum.
- Add a config-gated auto-pair policy:
  - missing/erased metadata opens the temporary authorization window;
  - a valid header with zero records opens the window;
  - an authorization-record-set version mismatch opens the window even if the
    header reports existing records;
  - a checksum mismatch opens the window even if the header reports existing
    records;
  - a valid current-version header with one or more records keeps the window
    closed.
- In `ble-upload` target builds, read the metadata header at boot and open the
  RAM-only BOOT / IO9 authorization window when the policy requires it.
- Do not write, erase, or persist BLE authorization metadata in this slice.
- Do not implement real BLE bonding, pairing-key storage, peer allowlists, or
  user-controlled clearing in this slice; document those as future work.

Phase 24O verification:

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

Phase 24O commit message:

```text
feat: add BLE auth metadata auto-pair policy
```

## Phase 24P: BLE Disconnect Preservation And LED Status Boundary

Phase 24P records the BLE hardware-validation slice that validated post-ACK
oldest-record advancement and adds the current LED status boundary. It accepts
post-ACK oldest-record advancement and
disconnect-before-Complete/ACK preservation on the live board, but it still
does not accept full Phase 24 completion because live Wi-Fi/BLE ACK race
behavior, BOOT download-mode preservation, persistent authorization records,
BLE auth metadata writes/erases/updates, and LED3 visual behavior still need
validation.

Phase 24P scope:

- Keep the Windows central validation tool at `tools/ble-watch`.
- Add `scan-drain-then-disconnect-preserves-record` so disconnect preservation
  can be checked after draining enough records to avoid full-spool drop-oldest
  interference.
- Validate `scan-ack-then-peek-next` advances the oldest pending record after
  a BLE ACK.
- Validate disconnect before `CompleteRecord` or `AckRecord` preserves the same
  oldest pending record across reconnect after the drain precondition.
- Correct current LED hardware facts: LED1 is the green power indicator tied to
  3.3 V and is not MCU-controlled; LED2 is the red active-low LED on IO0;
  LED3 is the blue active-low LED on IO1.
- Keep LED2 as the heartbeat indicator and allow a short boot/reset fast-flash
  before the steady heartbeat.
- Use LED3 as the normal firmware status LED and overlay time-bounded BLE
  status indications: pairing/authorization window fast blink; advertising or
  connected slow blink; boot BLE status window of 180 seconds;
  BOOT / IO9 or pairing-trigger BLE status window of at least 10 seconds, with
  fast blink continuing for the full pairing/authorization window.
- Keep Phase 24P authorization RAM-only. Later Phase 24 work may add saved
  bonding or equivalent persistent authorization records, but Phase 24P itself
  does not validate them.
- Do not flash new firmware as part of this slice unless explicitly needed.

Phase 24P verification:

```bash
'/mnt/c/Program Files/dotnet/dotnet.exe' build '\\wsl.localhost\archlinux\home\dreamonex\sleep-environment-monitor\tools\ble-watch\ble-watch.csproj'
'/mnt/c/Program Files/dotnet/dotnet.exe' '\\wsl.localhost\archlinux\home\dreamonex\sleep-environment-monitor\tools\ble-watch\bin\Debug\net10.0-windows10.0.19041.0\ble-watch.dll' scan-ack-then-peek-next 30 sleep-env-esp32c3 128
'/mnt/c/Program Files/dotnet/dotnet.exe' '\\wsl.localhost\archlinux\home\dreamonex\sleep-environment-monitor\tools\ble-watch\bin\Debug\net10.0-windows10.0.19041.0\ble-watch.dll' scan-drain-then-disconnect-preserves-record 30 sleep-env-esp32c3 128 25 40 8
cargo fmt
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf
cargo build --target riscv32imc-unknown-none-elf --features ble-upload,radio-coex
cargo clippy --all-targets
cargo clippy --target riscv32imc-unknown-none-elf
cargo clippy --target riscv32imc-unknown-none-elf --features ble-upload,radio-coex
git diff --check
```

Phase 24P flash notes:

- The BLE ACK and drain validations may exercise the measurement spool through
  `storage_task` in `0x003c0000..0x00400000`.
- No firmware image needs to be flashed for this slice unless later validation
  explicitly requires it.
- The BLE auth metadata sector `0x003bf000..0x003c0000` remains read-only in
  current firmware and is not deliberately written or erased by this slice.

Observed Phase 24P hardware results:

- `scan-ack-then-peek-next` ACKed sequence `108009` and then observed oldest
  sequence `108010`.
- `scan-drain-then-disconnect-preserves-record` drained sequences
  `109090..109129`, then disconnected before `CompleteRecord`/`AckRecord` on
  sequence `109130` and reconnected to observe the same sequence `109130`.

Phase 24P commit message:

```text
test: validate BLE disconnect preservation
```

## Phase 24Q: BLE Security And Auth Record Compile Path

Phase 24Q adds a compile-validated security and authorization-record
persistence path. It does not accept full Phase 24 completion because the saved
pairing path has not been validated on hardware and the remaining live BLE/Wi-Fi
checks are still open.

Phase 24Q scope:

- Enable TrouBLE security support in the `ble-upload` feature path and seed the
  BLE security RNG from ESP32-C3 TRNG during startup.
- Keep Wi-Fi enabled in `ble-upload,radio-coex` builds so BLE security changes
  compile with the existing Wi-Fi path.
- Add config entries for BLE authorization record capacity and security seed
  length.
- Extend the BLE auth sector model with structured authorization records:
  identity address, LTK, optional IRK, security level, bonded flag, record CRC,
  record-set checksum, record-set version, and clear/store/load helpers.
- Implement target compile paths to restore TrouBLE bond information from
  `0x003bf000..0x003c0000`, require encrypted access to measurement
  metadata/fragment/control characteristics when saved auth exists, and store a
  bond record on `PairingComplete`.
- Keep the BOOT / IO9 authorization window as the hardware-validated access
  path until saved pairing is manually validated.
- Preserve measurement spool flash format and measurement JSON payload shape.
- Do not run BLE functional pairing tests in this slice unless explicitly
  requested as a later hardware-validation step.

Phase 24Q verification:

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

Phase 24Q flash notes:

- No firmware flash is required for the compile validation slice.
- Target code can write or erase the BLE auth sector
  `0x003bf000..0x003c0000` when a BLE-enabled firmware receives
  `PairingComplete { bond: Some(..) }`, but Phase 24Q compile validation does
  not deliberately exercise that flash range.
- Measurement spool behavior remains owned by `storage_task` in
  `0x003c0000..0x00400000`.

Remaining Phase 24Q hardware checks:

- Validate real BLE pairing, saved bond restore across reboot, and rejected
  unauthorized/unencrypted access.
- Validate auth-record write/erase/update behavior, version/checksum reset
  behavior, automatic pairing-window opening after auth-record reset, and
  user-controlled clearing.
- Validate live Wi-Fi/BLE ACK race behavior, BOOT download-mode preservation,
  and LED3 BLE visual behavior.

Phase 24Q commit message:

```text
feat: add BLE auth persistence compile path
```

## Phase 24R: BLE Saved-Bond Validation And Auth Clear Gesture

Phase 24R adds saved-bond validation tooling, validates the first Windows
saved-bond restore path on hardware, and adds a documented user operation to
clear saved BLE authorization records. It does not accept full Phase 24
completion until the remaining auth-sector erase/update behavior, BOOT
download-mode preservation, LED3 visual behavior, and live Wi-Fi/BLE ACK race
checks are observed.

Phase 24R scope:

- Keep the default Wi-Fi firmware path unchanged and keep BLE disabled unless
  `ble-upload` is enabled.
- Add `tools/ble-watch scan-read-metadata-now`, which connects and requests
  protected metadata without waiting for the BOOT / IO9 temporary authorization
  window. Use `expect-success` after saved bond restore is expected and
  `expect-reject` after saved BLE authorization records are cleared.
- Make `ble-watch` print the Windows central pairing state for
  connection-oriented validation commands.
- Add a runtime-only BOOT / IO9 saved-auth clearing gesture: about 2 seconds
  still opens the temporary authorization window; continuing the same hold to
  about 8 seconds clears the saved BLE authorization sector and reopens the
  window.
- Keep BOOT / IO9 input-only with the MCU internal pull-up explicitly enabled
  at runtime.
- Preserve BOOT / IO9 reset or power-on download-mode behavior. The clear
  gesture is only a runtime gesture after firmware has booted.
- Preserve measurement spool flash format and measurement JSON payload shape.
- Validate the Windows central saved-bond path: first pairing, BLE auth-sector
  write after `PairingComplete`, reboot restore of one saved authorization
  record, and encrypted `no-pair` metadata access through the saved bond.
- Do not treat this slice as proof that phone/gateway interoperability,
  unauthorized rejection after clearing, runtime clear erase behavior,
  version/checksum reset behavior, BOOT download-mode preservation, live
  Wi-Fi/BLE ACK race behavior, or LED3 visual behavior is hardware-validated.

Phase 24R verification:

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

Phase 24R flash notes:

- No flash is required for compile/tool validation.
- Hardware validation with the BLE+Wi-Fi build flashes the app image region.
- Pairing and saved-bond validation may write the BLE auth metadata sector
  `0x003bf000..0x003c0000`.
- The runtime clear gesture erases the BLE auth metadata sector
  `0x003bf000..0x003c0000`.
- BLE ACK/drain checks may write or erase the measurement spool
  `0x003c0000..0x00400000` through `storage_task`.

Phase 24R commit message:

```text
test: validate BLE saved bond restore
```

## Phase 24S: Wi-Fi-Unready LED Status Config Boundary

Phase 24S separates plain Wi-Fi/IP-not-ready indication from explicit network
error flags so a board with local storage does not permanently slow-blink blue
LED3 just because Wi-Fi is absent.

Phase 24S scope:

- Keep `ErrorFlags::NETWORK_MASK` limited to explicit Wi-Fi, IP, and discovery
  fault flags for the REST upload path.
- Do not use `ErrorFlags::NETWORK_MASK` for BLE advertising, BLE connection,
  or BLE authorization state.
- Add `config::led::WIFI_UNREADY_STATUS_WINDOW_SECS` for the optional plain
  Wi-Fi/IP-not-ready LED3 hint. The default `0` disables this hint.
- Preserve explicit network fault behavior: `ErrorFlags::WIFI`,
  `ErrorFlags::IP`, and `ErrorFlags::DISCOVERY` still slow-blink LED3.
- Keep LED semantic authority in [../10-firmware/00-architecture.md](../10-firmware/00-architecture.md)
  and keep hardware facts in [../10-firmware/01-hardware.md](../10-firmware/01-hardware.md).
- Align BLE overlay wording to the implemented BLE runtime states:
  pairing/authorization fast blink and advertising-or-connected slow blink.

Phase 24S verification:

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

Phase 24S flash notes:

- No firmware flashing is required.
- No flash-write validation is required.
- This slice does not deliberately exercise the BLE auth metadata sector
  `0x003bf000..0x003c0000` or the measurement spool
  `0x003c0000..0x00400000`.

Phase 24S commit message:

```text
fix: make Wi-Fi-unready LED status configurable
```

## Phase 24T: BLE Auth Metadata Reset Hardware Validation

Phase 24T validates the BLE authorization metadata reset policy on hardware.
It deliberately corrupts or erases only the reserved BLE auth metadata sector
and confirms that the firmware auto-opens the temporary authorization window on
the next boot.

Phase 24T scope:

- Back up the existing BLE auth metadata sector before destructive validation.
- Exercise only `0x003bf000..0x003c0000`, the 4 KiB BLE auth metadata sector.
- Do not flash a new firmware image.
- Do not exercise BLE ACK/drain or measurement spool writes in
  `0x003c0000..0x00400000`.
- Validate automatic pairing-window opening after these auth metadata states:
  missing/erased sector, invalid header magic, empty current-version record
  set, records-version mismatch, compatibility-checksum mismatch, and header
  checksum mismatch.
- After a reset/invalid-auth pairing window closes, validate that an unpaired
  central cannot access protected metadata with `scan-read-metadata-now ...
  expect-reject no-pair`.
- Keep runtime BOOT / IO9 8 second clear-gesture validation separate; Phase 24T
  validates the metadata reset policy, not the user gesture.

Phase 24T verification:

```bash
cargo espflash read-flash --chip esp32c3 --port /dev/ttyACM0 --before usb-reset --after hard-reset --non-interactive 0x003bf000 0x1000 /tmp/ble-auth-before-phase24t.bin
cargo espflash write-bin --chip esp32c3 --port /dev/ttyACM0 --before usb-reset --after hard-reset --non-interactive 0x003bf000 /tmp/phase24-auth-patterns/badmagic-zero.bin
cargo espflash erase-region --chip esp32c3 --port /dev/ttyACM0 --before usb-reset --after hard-reset --non-interactive 0x003bf000 0x1000
cargo espflash write-bin --chip esp32c3 --port /dev/ttyACM0 --before usb-reset --after hard-reset --non-interactive 0x003bf000 /tmp/phase24-auth-patterns/empty-current.bin
cargo espflash write-bin --chip esp32c3 --port /dev/ttyACM0 --before usb-reset --after hard-reset --non-interactive 0x003bf000 /tmp/phase24-auth-patterns/version-mismatch.bin
cargo espflash write-bin --chip esp32c3 --port /dev/ttyACM0 --before usb-reset --after hard-reset --non-interactive 0x003bf000 /tmp/phase24-auth-patterns/compat-mismatch.bin
cargo espflash write-bin --chip esp32c3 --port /dev/ttyACM0 --before usb-reset --after hard-reset --non-interactive 0x003bf000 /tmp/phase24-auth-patterns/header-checksum-mismatch.bin
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-read-status 30 sleep-env-esp32c3
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-read-metadata-now 30 sleep-env-esp32c3 expect-reject no-pair
git diff --check
```

Phase 24T flash notes:

- `cargo espflash read-flash` backed up `0x003bf000..0x003c0000`.
- `cargo espflash write-bin` and `cargo espflash erase-region` deliberately
  wrote or erased only `0x003bf000..0x003c0000`.
- The measurement spool `0x003c0000..0x00400000` was not deliberately
  exercised by this validation slice.
- After Phase 24T, the board may be left with invalid BLE auth metadata and no
  Windows-side pairing record; rebooting should auto-open the temporary
  authorization window so a new pairing can be created.

Phase 24T commit message:

```text
test: validate BLE auth metadata reset policy
```

## Phase 24U: BLE Watch Windows GATT Recovery Tooling

Phase 24U hardens the Windows BLE central validation tool after repeated
WinRT/GATT stale-cache failures blocked further Phase 24 hardware validation.
It does not change firmware behavior and does not accept any remaining BLE
runtime behavior.

Phase 24U scope:

- Add retry and cached-fallback handling for Windows GATT service and
  characteristic lookup in `tools/ble-watch`.
- Add retry for status-characteristic reads without using cached status values
  for runtime decisions.
- Recreate the Windows `BluetoothLEDevice` / GATT object for
  `scan-read-status` when status reads still fail after retry.
- Reconnect inside `scan-watch-clear-gesture` after a status read failure so a
  transient stale GATT object does not end the delay-safe clear-gesture watch.
- Keep BLE protocol bytes, UUIDs, firmware code, flash ranges, and storage ACK
  behavior unchanged.
- Do not count this as runtime clear-gesture, LED, BOOT download-mode,
  phone/gateway, record replacement, or live Wi-Fi/BLE ACK-race acceptance.

Phase 24U verification:

```bash
'/mnt/c/Program Files/dotnet/dotnet.exe' build "$(wslpath -w tools/ble-watch/ble-watch.csproj)"
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-unpair 30 sleep-env-esp32c3
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-read-status 30 sleep-env-esp32c3
git diff --check
```

Phase 24U hardware and flash notes:

- No firmware image is flashed.
- No firmware flash sector is deliberately written or erased by this tooling
  change.
- `scan-unpair` changes only the Windows-side central pairing record.
- The final observed central state after recovery is Windows unpaired. The
  runtime clear-gesture validation must rebuild a saved-bond auth record before
  it can prove that the 8 second BOOT / IO9 gesture clears that record.

Phase 24U commit message:

```text
test: harden BLE watch GATT recovery
```

## Phase 24V: BLE Runtime Auth Clear Hardware Effect

Phase 24V validates the runtime saved-authorization clear effect after the
Windows central recovery tooling is in place. It accepts that the runtime clear
path removes saved authorization access, but it does not close the BOOT / IO9
release-diagnostics follow-up, BOOT download-mode preservation, LED3 visual
acceptance, phone/gateway interoperability, record replacement/update, or live
Wi-Fi/BLE ACK-race acceptance.

Phase 24V scope:

- Rebuild a Windows saved-bond authorization record from a reset state where
  missing or invalid auth metadata opens the temporary authorization window.
- Confirm protected metadata access succeeds through the saved authorization
  record without opening a new pairing flow.
- Run `scan-watch-clear-gesture 30 sleep-env-esp32c3 180 8000` with operator
  IO9/BOOT input.
- Accept the clear effect when the watch observes the press-after-release
  state, the 8 second hold threshold, the refreshed authorization window, and a
  following `scan-read-metadata-now ... expect-reject no-pair` rejects
  protected metadata access.
- Record the IO9 release-diagnostics mismatch separately if status continues
  to report `Pressed` after the operator releases IO9. Do not treat that as
  evidence that the operator held IO9 for 40 seconds or longer.
- Do not flash a new firmware image.
- Do not deliberately exercise the measurement spool
  `0x003c0000..0x00400000`.

Phase 24V verification:

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

Phase 24V hardware and flash notes:

- No firmware image is flashed.
- The runtime clear path may erase only the BLE auth metadata sector
  `0x003bf000..0x003c0000`.
- The measurement spool `0x003c0000..0x00400000` is not deliberately exercised.
- `scan-unpair` changes only the Windows-side central pairing record.
- The final Phase 24V state is Windows unpaired, with firmware auth metadata
  cleared or missing and the temporary authorization window opening after
  reset. The next saved-bond or auth replacement/update test must re-pair.

Phase 24V commit message:

```text
test: validate BLE runtime auth clear effect
```

## Phase 24W: BLE Watch Clear-Gesture Release Diagnostics

Phase 24W improves the Windows validation tool's evidence for the remaining
BOOT / IO9 release-diagnostics follow-up. It does not change firmware behavior
and does not accept the release-diagnostics hardware item by itself.

Phase 24W scope:

- Keep `scan-watch-clear-gesture` success strict: it still requires release
  before press, press-after-release, 8 second hold threshold, refreshed
  authorization window, and release after hold.
- Add `CLEAR_GESTURE_CLEAR_EFFECT_OBSERVED` once the tool has seen both the
  hold threshold and refreshed pairing window.
- Add `CLEAR_GESTURE_RELEASE_DIAGNOSTIC_MISSING` when the watch ends after
  clear-effect evidence but before a final release is observed in status.
- Include event indexes, pressed milliseconds, refreshed-window remaining
  milliseconds, and latest status fields in success and failure summaries.
- Do not treat missing final release observation as proof that the operator
  kept holding BOOT / IO9.
- Do not run hardware validation in this tooling slice.

Phase 24W verification:

```bash
'/mnt/c/Program Files/dotnet/dotnet.exe' build "$(wslpath -w tools/ble-watch/ble-watch.csproj)"
git diff --check
```

Phase 24W hardware and flash notes:

- No firmware image is flashed.
- No firmware flash sector is deliberately written or erased.
- The tool output change prepares the next manual IO9/BOOT release retest.

Phase 24W commit message:

```text
test: improve BLE clear gesture diagnostics
```

## Phase 24X: BLE Auth Record Upsert Policy Coverage

Phase 24X moves BLE authorization-record replacement policy into
hardware-independent storage logic and covers it with host tests. It also makes
the target BLE `PairingComplete` persistence path reuse that pure policy.

This is compile and pure-policy coverage only. It does not hardware-validate a
second real bond, an existing peer update, phone/gateway behavior, or the
remaining BOOT / IO9 and LED3 visual acceptance items.

Phase 24X scope:

- Match an existing saved auth record by identity address and update it in
  place.
- Match an existing saved auth record by identity resolving key when both
  records have one and update it in place.
- Append a new auth record while capacity remains.
- Replace index `0` as the oldest record when capacity is full.
- Clamp an out-of-range stored record count to available capacity before
  applying replacement policy.
- Treat zero auth-record capacity as `NoCapacity`.
- Keep the firmware `PairingComplete` persistence path using the same pure
  policy instead of a target-only duplicate matcher.

Phase 24X verification:

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

Phase 24X hardware and flash notes:

- No firmware image is flashed.
- No firmware flash sector is deliberately written or erased.
- Real auth-record replacement/update remains a hardware acceptance gap until
  another bond or an existing peer update is observed through the runtime BLE
  path.

Phase 24X commit message:

```text
test: cover BLE auth record upsert policy
```

## Phase 24Y: BOOT / IO9 Release Diagnostic Logging

Phase 24Y adds firmware-side BOOT / IO9 transition logging for the unresolved
release-diagnostics follow-up. It does not change the pairing or clear-gesture
state machine and does not accept the hardware release-diagnostics item by
itself.

Phase 24Y scope:

- Log the initial BOOT / IO9 runtime sample in BLE feature builds.
- Log each sampled BOOT / IO9 transition between `Pressed` and `Released`.
- Include current accumulated press milliseconds and pairing-window remaining
  milliseconds in those logs.
- Keep the status characteristic behavior unchanged: status reads still report
  the latest sampled BOOT / IO9 state and accumulated press time.
- Use the new logs in the next hardware retest to distinguish a GPIO-level
  low/pressed reading from a BLE status/tooling reporting issue.

Phase 24Y verification:

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

Phase 24Y hardware and flash notes:

- No firmware image is flashed by this diagnostic slice.
- No firmware flash sector is deliberately written or erased.
- The BOOT / IO9 release-diagnostics hardware item remains open until a new
  hardware run observes the release path with these logs available.

Phase 24Y commit message:

```text
test: add BOOT IO9 release diagnostics
```

## Work Items

- Add a BLE feature boundary that can be enabled or disabled independently from
  Wi-Fi.
- Keep Wi-Fi REST upload and BLE upload as separate upload paths over the same
  persistent measurement spool.
- Define a project-specific GATT service with structured characteristics for:
  - status
  - oldest-record metadata
  - record fragments
  - control and acknowledgement
- Frame measurement data with explicit sequence, offset, length, and integrity
  metadata instead of treating JSON or CSV as serial text.
- Add a `ble_task` boundary that owns BLE advertising, pairing, connection
  state, GATT transfer, and BLE-side acknowledgement handling.
- Add LED3 as the observable BLE status indicator for BLE-related operations.
  At minimum, an open pairing or authorization window must fast-blink LED3, and
  BLE advertising or connected state must slow-blink LED3.
  The final pattern table must remain documented and testable as pure status
  mapping logic.
- Keep `storage_task` as the only owner of persistent spool append, peek, and
  acknowledge operations.
- Document the current Phase 24 authorization state precisely: the temporary
  BOOT / IO9 authorization window, Windows saved-bond restore path, and auth
  metadata reset auto-pair policy are hardware-validated. Phase 24Z also
  hardware-validates the runtime saved-auth clear/release path with the
  explicit runtime GPIO9 internal pull-up firmware. Phase 24X covers the
  auth-record upsert policy in pure tests, and the 2026-05-26 Windows-central
  re-pair run validates real existing-peer update behavior. A distinct
  second-central append or full-capacity replacement run remains useful future
  coverage but is not a Phase 24 acceptance blocker.
- Preserve Wi-Fi acknowledgement semantics:
  - HTTP 2xx remains the only Wi-Fi REST ACK condition.
  - BLE may transmit copies while Wi-Fi upload is available and succeeding, but
    must not ACK the spool in that state.
  - BLE may ACK exactly one oldest record only when Wi-Fi upload is disabled or
    unavailable and an authorized central confirms complete receipt.
- Use BOOT / IO9 only as a runtime input for a future pairing or authorization
  gesture.
- Preserve BOOT / IO9 download-mode behavior during reset or power-on.
- Provide and validate a user operation to clear saved BLE authorization
  records. The current Phase 24R operation is a runtime BOOT / IO9 hold of
  about 8 seconds after firmware boot, which erases
  `0x003bf000..0x003c0000` and reopens the temporary authorization window.
- Bound BLE ownership of LED3 so status is visible without turning LED3 into a
  permanent ambiguous indicator: after boot, LED3 must represent BLE status for
  the first 180 seconds; after any BOOT / IO9 press or BLE pairing trigger,
  LED3 must represent BLE status for at least the next 10 seconds. If the
  trigger opens a longer pairing or authorization window, the pairing-window
  LED3 feedback continues for the full open window.
- Update [../10-firmware/05-ble.md](../10-firmware/05-ble.md) as implementation
  details become concrete.

## Unit Tests

Add hardware-independent tests for:

- BLE frame encode/decode.
- Fragment ordering and bounds checks.
- ACK behavior when Wi-Fi upload is available.
- ACK behavior when Wi-Fi upload is unavailable.
- Disconnect before ACK preserves the pending record.
- Wi-Fi and BLE observing the same record cannot acknowledge more than one
  oldest pending record.
- BLE enable/disable config selection.
- BOOT / IO9 pairing gesture state logic.
- BLE authorization metadata header parsing and auto-pair policy.
- BLE authorization record encode/load/store/clear behavior and version /
  checksum auto-pair policy.
- BLE authorization record upsert policy for existing-record update, append,
  full-capacity replacement, count clamping, and zero-capacity handling.
- Runtime BOOT / IO9 auth-record clear gesture timing.
- LED3 BLE status pattern selection and boot/BOOT-trigger indication-window
  timing as hardware-independent logic.

## Manual Integration Checks

- Confirm BLE advertises only when enabled.
- Confirm an authorized BLE central can connect and read structured status.
- Confirm an authorized BLE central can receive a full measurement record through
  GATT fragments.
- Confirm unpaired or unauthorized centrals cannot read measurement records.
- Confirm BLE transfer does not stop sensor sampling, aggregation, storage, or
  Wi-Fi reconnect.
- Confirm BLE does not ACK storage while Wi-Fi REST upload is succeeding.
- Disable or break Wi-Fi upload and confirm BLE can ACK after central-confirmed
  complete receipt.
- Confirm disconnect during record transfer preserves the pending record.
- Confirm BOOT / IO9 can be read as a runtime input without configuring it as an
  output or changing the hardware pull-up behavior.
- Confirm holding BOOT during reset or power-on still enters download mode.
- Confirm missing, empty, version-mismatched, or checksum-mismatched BLE
  authorization metadata opens the temporary authorization window on boot when
  the config switch is enabled.
- Confirm saved BLE authorization records survive reboot and permit only the
  matching encrypted central after hardware pairing is validated.
- Confirm a config version/checksum update can force the pairing window to open
  even when prior saved records exist.
- Confirm there is a documented user operation to clear saved BLE
  authorization records.
- Confirm LED3 gives observable feedback for BLE operations: pairing or
  authorization window open fast-blinks, and BLE advertising or connected state
  slow-blinks.
- Confirm LED3 represents BLE status for the first 180 seconds after boot when
  BLE is enabled.
- Confirm pressing BOOT / IO9 or triggering a pairing/authorization entry makes
  LED3 represent BLE status for at least the next 10 seconds, and for the full
  pairing/authorization window when that window remains open longer.

## Done When

- BLE and Wi-Fi can be independently enabled or disabled.
- BLE uses a structured project GATT protocol, not a serial-port emulation.
- BLE upload reads the oldest persisted records without bypassing
  `storage_task`.
- Storage ACK behavior is deterministic when Wi-Fi and BLE are both enabled.
- Pairing or authorization prevents unpaired BLE measurement access, and the
  final security design defines persistent bonding or an equivalent saved
  authorization record.
- BOOT / IO9 pairing entry is validated without breaking download mode.
- LED3 provides documented, observable BLE feedback for pairing/authorization,
  advertising, and connected state, including the 180 second post-boot BLE
  status window and the 10 second BOOT / IO9-triggered BLE status window.
- Hardware-independent tests cover BLE protocol framing and ACK policy.

## Git Commit Message

```text
feat: add BLE upload channel
```

---

# Phase 25: Refactor And Maintenance Split

## Goal

Reduce the Phase 24 BLE, storage, upload, and validation-tool maintenance
surface without changing runtime behavior or external contracts.

## Work Items

- Freeze BLE UUIDs, status / metadata / fragment / control frame bytes, BLE ACK
  policy, Wi-Fi HTTP 2xx ACK semantics, flash ranges, REST JSON shape,
  BOOT / IO9 behavior, and LED3 BLE overlay rules before refactoring.
- Split `tools/ble-watch/Program.cs` into BLE profile constants, scanner,
  protocol helpers, models, protected GATT helpers, WinRT pairing helpers, and
  output formatting while preserving CLI commands and output labels.
- Split `firmware/src/tasks/upload.rs` into endpoint/types, JSON, HTTP,
  discovery/time parsing, time selection, and target runtime uploader modules.
- Split `firmware/src/tasks/ble.rs` into profile, status, protocol, transfer,
  pairing, target BLE auth, storage bridge, GATT, and runtime modules while
  preserving public `tasks::ble::*` paths.
- Split `firmware/src/storage/spool.rs` into memory queue, wire codec, and
  flash-backed log modules while preserving public `storage::spool::*` paths.
- Split `firmware/src/storage/ble_auth.rs` into types/status, upsert policy,
  header codec, record codec, and flash load/store/clear modules while
  preserving public `storage::ble_auth::*` paths.
- Split `firmware/src/tasks/storage.rs` into backlog/metrics,
  command/response protocol, and target runtime modules while preserving public
  `tasks::storage::*` paths.
- Keep [../10-firmware/05-ble.md](../10-firmware/05-ble.md) as the BLE ACK and
  security rule authority during the split.
- Keep LED overlay rules centralized and avoid duplicate status policy
  implementations.

## Unit Tests

No new behavior is introduced. Existing hardware-independent tests must still
pass.

## Verification

```bash
cargo fmt
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf
cargo clippy --all-targets
cargo clippy --target riscv32imc-unknown-none-elf
cargo build --target riscv32imc-unknown-none-elf --features ble-upload,radio-coex
cargo clippy --target riscv32imc-unknown-none-elf --features ble-upload,radio-coex
'/mnt/c/Program Files/dotnet/dotnet.exe' build "$(wslpath -w tools/ble-watch/ble-watch.csproj)"
git diff --check
```

## Manual Integration Checks

None required for this phase. Do not flash firmware or deliberately write/erase
firmware flash as part of Phase 25.

## Done When

- All listed modules are split without changing public paths used by existing
  code.
- BLE UUIDs, frame layouts, ACK conditions, Wi-Fi behavior, flash ranges,
  BOOT / IO9 behavior, LED3 overlay rules, and REST measurement JSON remain
  unchanged.
- Existing Rust tests/builds/clippy checks and the Windows `ble-watch` build
  pass.
- Completion evidence is recorded in
  [01-walkthrough.md](01-walkthrough.md), and task status is synchronized in
  [03-todo.md](03-todo.md).

## Git Commit Message

```text
refactor: split Phase 24 maintenance modules
```

---

# Phase 26: Server Persistence, Configuration, And History

## Goal

Add durable server-side measurement storage, TOML configuration, Rich local
operator views, and authenticated history reads while preserving the existing
firmware upload contract.

Documentation comes first. Each Phase 26 milestone must update relevant docs,
record completion evidence in [01-walkthrough.md](01-walkthrough.md), and end
with at least one commit.

## Work Items

- Add [../20-server/04-persistence-configuration.md](../20-server/04-persistence-configuration.md)
  and keep server persistence/configuration behavior documented before code
  changes.
- Add [../../server/config.example.toml](../../server/config.example.toml) as
  the tracked configuration reference.
- Load TOML configuration from XDG defaults or an explicit `--config` path.
- Generate the XDG default config on first use when it is absent.
- Preserve CLI overrides for host, ports, log level, and output behavior.
- Add SQLite and JSONL storage backends that can be independently enabled.
- Implement configurable ACK policy with global `storage.required_for_ack`,
  backend-level `required_for_ack`, and backend-level `sufficient_for_ack`.
- Implement policy profiles for retention, deduplication, and backfill.
- Add Bearer-protected history read endpoints under `/api/v1/history/`.
- Add Rich live dashboard output for interactive `serve` sessions.
- Add a local history CLI for summary, tail, and simple metric trends.

## Unit Tests

All Phase 26 tests must remain hardware-free.

Required coverage:

- XDG config path selection, default generation, explicit config loading, and
  CLI override precedence.
- Storage policy profile inheritance, loop rejection, retention parsing,
  deduplication strategy parsing, and ACK flag parsing.
- SQLite and JSONL write/read behavior, duplicate handling, compaction, and
  retention cleanup.
- ACK matrix for global required off, sufficient success, required success,
  missing ACK paths, storage failures, and duplicate conflicts.
- Backfill startup and background helpers with deterministic fake clocks or
  direct helper calls.
- History API token validation, query validation, measurement listing, and
  summary output.
- Rich dashboard/history output paths without requiring a real terminal.

## Verification

Run from `server/`:

```bash
env UV_CACHE_DIR=.cache/uv uv run pytest
env UV_CACHE_DIR=.cache/uv uv run ruff check --diff .
env UV_CACHE_DIR=.cache/uv uv run ruff format --check .
```

## Done When

- Existing upload, time, discovery, and UDP discovery behavior remains
  compatible with firmware.
- Default config generation produces a valid TOML file.
- Measurements can persist to SQLite, JSONL, both, or neither according to
  config.
- HTTP 2xx upload ACK follows the configured storage ACK policy.
- History API requires Bearer token when enabled.
- Rich live and offline views are available without breaking plain/JSON output.
- Each milestone has at least one commit and walkthrough evidence.

## Git Commit Message

```text
feat: persist and display server measurements
```

---

# Phase 27: Server TUI Runtime

## Goal

Move local server operation out of the ad hoc `serve` Rich chart output and
into a dedicated Textual terminal UI while preserving `serve` as the scriptable
service entry point.

Documentation comes first. Each Phase 27 milestone must update relevant docs,
record completion evidence in [01-walkthrough.md](01-walkthrough.md), and end
with at least one commit.

## Work Items

- Update [../20-server/03-cli.md](../20-server/03-cli.md),
  [../20-server/04-persistence-configuration.md](../20-server/04-persistence-configuration.md),
  and [../../server/README.md](../../server/README.md) before implementation.
- Add Textual as the server TUI runtime dependency.
- Add `sleep-env-server tui` as the full-screen local operator entry point.
- Keep `sleep-env-server serve` for service/process use; it must no longer
  print live measurement charts.
- Route server upload, storage, UDP discovery, and shutdown events through a
  bounded event bridge that can feed either logs or the TUI.
- Use Rich logging for human `serve` output and keep JSONL output for machine
  consumption.
- Ensure Uvicorn logs do not corrupt the Textual screen in TUI mode.
- Keep REST, UDP discovery, storage, history API, TOML configuration, and
  compatibility wrapper behavior unchanged.

## Unit Tests

All Phase 27 tests must remain hardware-free.

Required coverage:

- `tui` argument parsing and config override behavior.
- `serve` output selection and absence of live chart rendering.
- Rich/plain/JSON service logging paths without dumping unbounded payloads.
- Event bridge delivery for upload, storage, UDP discovery, and shutdown
  events.
- Textual app smoke startup with deterministic, in-process events.
- Keyboard exit behavior for the TUI app.

## Verification

Run from `server/`:

```bash
env UV_CACHE_DIR=.cache/uv uv run pytest
env UV_CACHE_DIR=.cache/uv uv run ruff check --diff .
env UV_CACHE_DIR=.cache/uv uv run ruff format --check .
```

Manual terminal verification is useful but not required for automated
acceptance:

```bash
env UV_CACHE_DIR=.cache/uv uv run sleep-env-server tui --host 127.0.0.1 --port 8080
```

If no human operator is available, mark the manual TUI run as skipped rather
than repeatedly checking it.

## Done When

- `sleep-env-server tui` starts a Textual full-screen view with service status,
  recent measurements, metric trends, and bounded event logs.
- `sleep-env-server serve` starts the same HTTP and UDP service behavior but
  emits only logs/events, not live charts.
- JSONL output remains stable for scripts and tests.
- Uvicorn, upload, storage, discovery, and shutdown diagnostics are visible in
  the appropriate log or TUI surface.
- Existing REST and storage tests continue to pass.
- Each milestone has at least one commit and walkthrough evidence.

## Git Commit Message

```text
feat: add server tui runtime
```

---

# Phase 28: Server TUI Visual Polish And Serve Help

## Goal

Replace the bare Textual debug-style TUI with a modern, readable operator
surface that supports an explicit transparent mode, and clean up `serve` so it
has clear help and predictable service output.

Documentation comes first. Each Phase 28 milestone must update relevant docs,
record completion evidence in [01-walkthrough.md](01-walkthrough.md), and end
with at least one commit.

## Work Items

- Update [../20-server/03-cli.md](../20-server/03-cli.md),
  [../20-server/04-persistence-configuration.md](../20-server/04-persistence-configuration.md),
  [../../server/README.md](../../server/README.md), and
  [../../server/config.example.toml](../../server/config.example.toml) before
  implementation.
- Add a `[tui]` TOML table with `theme = "graphite"` and
  `transparent = false`.
- Add `sleep-env-server tui --transparent` to enable transparent background
  mode for terminals that already have window transparency configured.
- Redesign the Textual TUI with a modern graphite/cyan/emerald/amber/rose
  palette, a clear status strip, metric summary, measurement table, trend
  panel, and bounded event log.
- Keep transparent mode readable by avoiding large opaque fills and by using
  restrained borders and text contrast.
- Add an in-TUI help surface for `q`, `Ctrl+C`, `c`, `r`, and `?`.
- Make `serve` the plain, scriptable service entry point by default.
- Keep `serve --json-log` as strict JSONL and make styled Rich service logging
  explicit with `--rich-log`.
- Improve root, `serve`, `tui`, and `history` help text with descriptions,
  option help, defaults, and examples.
- Preserve REST, UDP discovery, storage, history API, TOML configuration, and
  compatibility wrapper behavior.

## Unit Tests

All Phase 28 tests must remain hardware-free.

Required coverage:

- `[tui]` config parsing and validation.
- `tui --transparent` override behavior.
- TUI smoke startup with default and transparent mode classes.
- In-TUI help action visibility.
- Measurement/event rendering does not fall back to the old raw debug layout.
- Root, `serve`, `tui`, and `history` help include examples and option
  descriptions.
- `serve` default output is plain, `--json-log` is JSONL, and `--rich-log` is
  the only RichHandler path.

## Verification

Run from `server/`:

```bash
env UV_CACHE_DIR=.cache/uv uv run pytest
env UV_CACHE_DIR=.cache/uv uv run ruff check --diff .
env UV_CACHE_DIR=.cache/uv uv run ruff format --check .
```

Manual terminal verification is useful but not required for automated
acceptance:

```bash
env UV_CACHE_DIR=.cache/uv uv run sleep-env-server tui --transparent --host 127.0.0.1 --port 8080
```

If no human operator is available, mark the manual TUI run as skipped rather
than repeatedly checking it.

## Done When

- `sleep-env-server tui` no longer looks like a debug panel and presents a
  coherent modern terminal UI.
- Transparent mode is available through TOML and CLI without degrading normal
  default readability.
- `serve` help and output modes are understandable from `--help`.
- `serve` default output is clean and not Rich-styled unless `--rich-log` is
  requested.
- Existing REST, storage, history, and TUI tests continue to pass.
- Each milestone has at least one commit and walkthrough evidence.

## Follow-Up Theme Note

Milestone 77 switches the TUI default palette to Catppuccin Mocha while keeping
`theme = "graphite"` accepted for older local configuration.

## Git Commit Message

```text
feat: polish server tui and help
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
[ ] BLE frame encode / decode
[ ] BLE fragment ordering and bounds checks
[ ] BLE ACK behavior when Wi-Fi upload is available
[ ] BLE ACK behavior when Wi-Fi upload is unavailable
[ ] BLE disconnect before ACK preserves pending record
[ ] BLE / Wi-Fi duplicate ACK prevention
[ ] BLE enable / disable config selection
[ ] BOOT / IO9 pairing gesture state logic
[ ] BLE authorization metadata header parsing and auto-pair policy
```
