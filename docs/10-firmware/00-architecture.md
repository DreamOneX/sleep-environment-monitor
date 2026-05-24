# Architecture

## 1. Development Goal

Build firmware for the sleep-environment monitor board.

The firmware shall collect:

- Temperature
- Relative humidity
- Ambient light
- Relative acoustic noise level
- Wi-Fi upload status
- Board health status

The firmware shall be structured so that:

- Sensor sampling does not depend on Wi-Fi.
- Wi-Fi failure does not stop local sampling.
- Hardware drivers are separated from pure calculation logic.
- Unit tests run on host without hardware or human observation.

---

## 2. Expected Development Result

The final firmware should provide:

- Periodic SHT40 temperature / humidity readings.
- Periodic OPT3001 lux readings.
- Periodic microphone ADC statistics:
  - mean
  - RMS
  - peak
  - relative dB
  - clip count
- Aggregated `Measurement` records.
- Wi-Fi connection and reconnection.
- Upload queue with "drop oldest when full" behavior.
- Internal SPI flash persistent spool for measurements that cannot be uploaded immediately.
- Recovery of pending measurements after reset or power loss.
- RESTful upload, server discovery, and real-world time support as described in [03-network.md](03-network.md).
- Future Bluetooth Low Energy upload through a structured project GATT protocol
  as described in [05-ble.md](05-ble.md).
- LED indicators:
  - LED1: green power indicator tied to the 3.3 V rail; not firmware-controlled.
  - LED2: red active-low heartbeat on IO0, with a short boot/reset fast-flash
    before the steady heartbeat.
  - LED3: blue active-low combined status/BLE indicator on IO1. It shows
    normal firmware status outside BLE indication windows and time-bounded BLE
    status when BLE is enabled or triggered.
- Hardware-independent unit tests for all pure logic.

---

## 3. Hardware Summary

## MCU

```text
ESP32-C3-WROOM-02-N4
```

The module includes 4 MB internal SPI flash. Firmware uses a dedicated flash storage region for persisted measurement spooling and must not write into bootloader, partition table, app image, or RF/calibration data regions.

Flash layout:

```text
total flash:            0x0000_0000..0x0040_0000  4 MiB
system reserved:        0x0000_0000..0x0001_0000  bootloader / partition / RF data area
application reserved:   0x0001_0000..0x003b_f000  firmware image growth area
BLE auth metadata:      0x003b_f000..0x003c_0000  4 KiB, 4 KiB sector aligned
measurement spool:      0x003c_0000..0x0040_0000  256 KiB, 4 KiB sector aligned
```

The firmware validates this layout at runtime before enabling flash access. A
zero-sized BLE auth or spool region, sector-unaligned range, out-of-bounds
range, overlap with protected regions, or BLE-auth/spool overlap is treated as
a configuration error.

## Pin Mapping

| Function | GPIO | Notes |
|---|---:|---|
| USB D- | IO18 | Native USB |
| USB D+ | IO19 | Native USB |
| I2C SDA | IO4 | Shared I2C bus |
| I2C SCL | IO5 | Shared I2C bus |
| Microphone ADC | IO3 / ADC1_CH3 | Analog input |
| LED1 | 3.3 V rail | Green power indicator; not MCU-controlled |
| LED2 | IO0 | Red LED; active-low |
| LED3 | IO1 | Blue LED; active-low |
| UART RX | IO20 | Debug header |
| UART TX | IO21 | Debug header |
| BOOT | IO9 | Active-low; future BLE pairing input only after boot |
| RESET | EN | Active-low |
| Strap pin | IO8 | Pulled high, do not use as normal I/O |

## I2C Devices

| Device | Address |
|---|---:|
| SHT40-AD1B-R2 | `0x44` |
| OPT3001IDNPRQ1 | `0x45` |

---

## 4. Firmware LED Semantics

LED hardware facts live in [01-hardware.md](01-hardware.md). Firmware state
semantics live here.

Board LEDs:

| LED | Firmware role | Pattern owner |
|---|---|---|
| LED1 green | Power indicator only; tied to the 3.3 V rail and not MCU-controlled | Hardware |
| LED2 red | Heartbeat / reset-life indicator | `tasks::led` heartbeat path |
| LED3 blue | Firmware status indicator with time-bounded BLE overlay | `util::status` policy plus `tasks::led` GPIO output |

LED2 red semantics:

- On boot or reset, firmware may fast-flash LED2 briefly to show that the
  application started.
- After the boot flash, LED2 is the heartbeat indicator: a short pulse once per
  second while the firmware scheduler is alive.
- LED2 is not used for upload failure, storage failure, BLE pairing, or BLE
  connection state in the current firmware policy.

LED3 blue has two policy modes. Normal firmware status is used when no BLE
indication window is active. BLE overlay status is used only while the BLE
indication window is active or while the BLE pairing/authorization window is
open.

Normal firmware status uses `ErrorFlags` and an optional Wi-Fi-unready startup
hint. In this table, "network" means the Wi-Fi/IP/discovery path used by REST
upload. It does not include BLE advertising, BLE connection state, or BLE
authorization state.

| Priority | Source state | LED3 pattern | Meaning |
|---:|---|---|---|
| 1 | `ErrorFlags::SENSOR_MASK` intersects current flags | `FastBlink` | SHT40, OPT3001, or microphone path failure |
| 2 | `ErrorFlags::STORAGE`, `ErrorFlags::TIME`, or `ErrorFlags::UPLOAD_MASK` is set | `On` | Persistent storage, time sync, or REST upload failure |
| 3 | `ErrorFlags::NETWORK_MASK` is set, or the optional Wi-Fi-unready hint is visible | `SlowBlink` | Explicit Wi-Fi/IP/discovery fault, or a configured startup hint that Wi-Fi is not ready |
| 4 | No matching error and the Wi-Fi-unready hint is not visible | `Off` | Normal operation; local sampling and storage may continue even if Wi-Fi is absent |

Plain Wi-Fi/IP unready state is not treated as an error by itself. The
`config::led::WIFI_UNREADY_STATUS_WINDOW_SECS` knob controls whether LED3
briefly slow-blinks for Wi-Fi-not-ready after boot; `0` disables that hint.
Explicit `ErrorFlags::WIFI`, `ErrorFlags::IP`, and `ErrorFlags::DISCOVERY`
remain `ErrorFlags::NETWORK_MASK` faults and still slow-blink LED3.

BLE overlay status uses BLE runtime and pairing-window state. It does not read
or set `ErrorFlags::NETWORK_MASK`.

| Priority | Source state | LED3 pattern | Meaning |
|---:|---|---|---|
| 1 | BLE pairing or authorization window is open | `FastBlink` | User-visible BLE authorization opportunity |
| 2 | BLE runtime state is `Error` | `On` | BLE runtime error |
| 3 | BLE runtime state is `Advertising` or `Connected` | `SlowBlink` | BLE peripheral is discoverable/connectable or connected |
| 4 | BLE runtime state is `Idle` or `Disabled` | `Off` | No active BLE operation to surface |

BLE indication on LED3 is time-bounded so the blue LED does not permanently
hide normal firmware status:

- After boot, when BLE is enabled, LED3 represents BLE status for the first
  180 seconds.
- After any BOOT / IO9 press or pairing/authorization trigger, LED3 represents
  BLE status for at least the next 10 seconds.
- If a trigger opens a longer pairing or authorization window, LED3 keeps the
  pairing-window `FastBlink` feedback for the full open window.
- When no BLE indication window is active, LED3 returns to the normal firmware
  status policy above.

Timing and polarity:

- LED outputs are active-low: `LOW = on`, `HIGH = off`.
- `FastBlink` toggles every 100 ms.
- `SlowBlink` toggles every 500 ms.
- LED2 boot/reset fast-flashes for 15 cycles at 100 ms on / 100 ms off, then
  becomes the heartbeat: 100 ms on, 900 ms off.

`util::status` owns the pure policy mapping. `tasks::led` owns active-low GPIO
output and should not contain business policy. Phase 24P compile-validates the
LED3 BLE overlay path and pure timing/pattern tests. Hardware visual acceptance
of the LED3 BLE blink patterns remains a manual Phase 24 check.

---

## 5. Current Source Tree

```text
docs/
├── 00-project/
├── 10-firmware/
├── 20-server/
├── 30-integration/
└── README.md
firmware/
├── src
│   ├── bin
│   │   └── main.rs
│   ├── drivers
│   │   ├── flash.rs
│   │   ├── mic.rs
│   │   ├── mod.rs
│   │   ├── opt3001.rs
│   │   └── sht40.rs
│   ├── storage
│   │   ├── ble_auth.rs
│   │   ├── flash_model.rs
│   │   ├── mod.rs
│   │   └── spool.rs
│   ├── tasks
│   │   ├── aggregator.rs
│   │   ├── ble.rs
│   │   ├── led.rs
│   │   ├── mic.rs
│   │   ├── mod.rs
│   │   ├── net.rs
│   │   ├── sensor.rs
│   │   ├── storage.rs
│   │   ├── upload.rs
│   │   └── wifi.rs
│   ├── util
│   │   ├── logging.rs
│   │   ├── mod.rs
│   │   ├── queue.rs
│   │   └── status.rs
│   ├── board.rs
│   ├── config.rs
│   ├── lib.rs
│   └── types.rs
├── Cargo.toml
├── build.rs
└── README.md
server/
├── README.md
└── post_receiver.py
Cargo.toml
```

High-level firmware groups:

- `bin/`: target executable entry point.
- `drivers/`: hardware adapters and protocol/conversion logic.
- `storage/`: flash-backed spool and BLE authorization metadata models.
- `tasks/`: Embassy runtime task boundaries.
- `util/`: shared hardware-independent helpers.
- `board.rs`: physical board constants.
- `config.rs`: deployment knobs and behavior policy constants for runtime,
  networking, BLE, Wi-Fi, upload, sampling, storage, aggregation, and LED
  behavior. It intentionally does not own physical board facts or driver
  protocol constants.
- `lib.rs`: firmware library module root.
- `types.rs`: shared data types.

The root `Cargo.toml` is a workspace manifest. The firmware package remains named `sleep-environment-monitor`, and root-level Cargo commands target `firmware` by default.

---

## 6. Module Responsibilities

Firmware module paths in this section are relative to `firmware/src/`.

## `board.rs`

Contains board constants only.

```rust
pub const PIN_I2C_SDA: u8 = 4;
pub const PIN_I2C_SCL: u8 = 5;

pub const PIN_MIC_ADC: u8 = 3;

pub const PIN_LED2: u8 = 0;
pub const PIN_LED3: u8 = 1;

pub const I2C_ADDR_SHT40: u8 = 0x44;
pub const I2C_ADDR_OPT3001: u8 = 0x45;

pub const FLASH_TOTAL_SIZE_BYTES: u32 = 4 * 1024 * 1024;
pub const FLASH_SECTOR_SIZE_BYTES: u32 = 4096;

pub const FLASH_BLE_AUTH_REGION_OFFSET: u32 = 0x003b_f000;
pub const FLASH_BLE_AUTH_REGION_SIZE: u32 = 0x0000_1000;

pub const FLASH_SPOOL_REGION_OFFSET: u32 = 0x003c_0000;
pub const FLASH_SPOOL_REGION_SIZE: u32 = 0x0004_0000;

pub const FLASH_SYSTEM_RESERVED_OFFSET: u32 = 0x0000_0000;
pub const FLASH_SYSTEM_RESERVED_SIZE: u32 = 0x0001_0000;

pub const FLASH_APP_RESERVED_OFFSET: u32 = 0x0001_0000;
pub const FLASH_APP_RESERVED_SIZE: u32 =
    FLASH_BLE_AUTH_REGION_OFFSET - FLASH_APP_RESERVED_OFFSET;
```

Flash constants must pass `drivers::flash::validate_default_flash_spool_region()`
before spool writes or BLE auth metadata reads are enabled.

---

## `types.rs`

Defines shared data types.

Core types:

```text
EnvSample
MicSample
Measurement
ErrorFlags
NetworkState
UploadResult
```

---

## `config.rs`

Firmware configuration module for deployment and behavior policy values.

The boundary is:

- `board.rs` keeps physical board facts such as pins, I2C addresses, and flash layout.
- `config.rs` owns Wi-Fi, BLE, REST upload, API endpoint, time sync, sensor,
  microphone, storage, runtime, network, aggregation, and LED policy values.
- Drivers keep protocol constants and conversion math.

Current `config.rs` groups:

- `runtime`: heap size and main idle sleep policy.
- `network`: network stack sizing and DHCP configuration.
- `ble`: BLE feature enablement, advertising name, fragment and HCI buffer
  sizes, polling intervals, pairing hold/window timing, authorization record
  capacity, authorization record-set version/checksum, security seed length,
  and `AUTO_PAIR_ON_AUTH_RECORD_RESET`.
- `wifi`: Wi-Fi feature enablement, SSID/password/auth-mode defaults,
  reconnect backoff, credential validation, SSID 32-byte limit, WPA password
  8-to-64-byte limits, open-network empty-password rule, and the rule that a
  64-byte WPA password must be a hex PSK.
- `upload`: device ID, JSON schema version, REST fallback host/IP/port, API
  paths, discovery, NTP/time sync, retry, timeout, buffer, and success-log
  policy.
- `sensor`: I2C bus speed, sample period, SHT40 wait, and sensor log cadence.
- `mic`: microphone sample window, retry, delay, log cadence, and target ADC
  attenuation.
- `storage`: measurement payload, persistent spool, command queue, and metrics
  log sizing.
- `aggregator`: measurement aggregation log cadence.
- `led`: heartbeat and status blink timing.

`AUTO_PAIR_ON_AUTH_RECORD_RESET` allows BLE startup to open the BOOT / IO9
authorization window when the BLE authorization record set is absent, invalid,
empty, version-incompatible, or checksum-mismatched. Phase 24Q adds a
compile-validated target path for restoring TrouBLE bond records and storing a
new bond record on `PairingComplete`, but that flash write/erase path and
saved-pairing behavior are not hardware-validated yet.

See [04-configuration.md](04-configuration.md).

---

## `drivers/sht40.rs`

Responsibilities:

- SHT40 CRC calculation.
- Raw temperature conversion.
- Raw humidity conversion.
- Measurement frame parsing.
- Hardware I2C read wrapper.

Pure logic must be testable without I2C.

---

## `drivers/opt3001.rs`

Responsibilities:

- OPT3001 register constants.
- Raw result register to lux conversion.
- Hardware I2C config/read wrapper.

Pure lux conversion must be testable without I2C.

---

## `drivers/mic.rs`

Responsibilities:

- ADC sample analysis.
- Compute:
  - mean
  - RMS
  - peak
  - relative dB
  - clip count

Pure sample analysis must be testable without ADC.

---

## `drivers/flash.rs`

ESP32-C3 internal SPI flash adapter.

Responsibilities:

- Expose a bounded storage region for the measurement spool.
- Expose a bounded BLE authorization metadata region for Phase 24 boot-time
  pairing policy checks and future BLE bond-record persistence.
- Implement the project `FlashStorage` interface over the ESP32-C3 ROM SPI flash functions on the embedded target.
- Refuse out-of-range access.
- Require sector-aligned erase operations and 4-byte-aligned ROM read/write operations.
- Run an explicit hardware smoke test only when the `flash-smoke` Cargo feature is enabled.
- Keep partition or fixed-region selection outside business logic.

The smoke test erases, verifies, writes, reads back, and erases again only the first spool sector:

```text
0x003c_0000..0x003c_1000
```

Default firmware builds must not enable `flash-smoke`, so normal boot does not erase the spool test sector.

---

## `storage/ble_auth.rs`

Hardware-independent BLE authorization record-set logic.

Responsibilities:

- Define the BLE authorization records header magic, header format version,
  record-set version, record count, records checksum, and header checksum.
- Define structured authorization records for identity address, long-term key,
  optional identity resolving key, security level, bonded flag, record CRC, and
  fixed record length.
- Treat erased flash, invalid header data, invalid records, empty record sets,
  record-set version mismatch, or checksum mismatch as requiring a new
  pairing/authorization window when the config switch allows it.
- Encode, load, store, replace, and clear authorization records through the
  project `FlashStorage` interface.
- Keep this metadata separate from the measurement spool format and JSON
  payload shape.

Phase 24Q target code can restore TrouBLE bond information from this sector and
can store a bond record after `PairingComplete` in BLE-enabled firmware. That
path is compile/static verified only; real pairing persistence, reboot restore,
flash write/erase/update behavior, version/checksum migration, and user
clearing have not been accepted on hardware yet.

---

## `storage/spool.rs`

Hardware-independent persistent measurement spool logic.

Responsibilities:

- Encode and decode flash records.
- Maintain FIFO append / peek / acknowledge semantics.
- Recover valid records after reset by scanning the storage region.
- Drop oldest records when the spool is full.
- Detect corrupt or incomplete records through CRC and record headers.

Record format:

```text
magic:u32
version:u8
flags:u8
header_len:u16
sequence:u64
payload_len:u16
payload_crc:u32
payload: JSON measurement field-fragment bytes
padding: erase/write alignment as needed
```

Rules:

- `sequence` is monotonic and used only for ordering/recovery.
- Phase 22 JSON field-fragment records use a payload flag so legacy unflagged CSV records can be skipped during migration.
- A record is uploadable only after its CRC validates.
- A record is removed from the spool only after Wi-Fi upload returns HTTP 2xx,
  or after BLE upload receives authorized-central complete-record confirmation
  while Wi-Fi upload is unavailable.
- Upload acknowledgements are sequence-checked so a stale Wi-Fi or BLE ACK does
  not delete a different oldest pending record.
- If the storage region is full, delete the oldest acknowledged or pending record required to make room, preserving the newest measurements.
- A corrupt tail record is ignored during recovery; earlier valid records remain uploadable.
- A corrupt middle record is skipped and reported through storage error status; later valid records may still be recovered if scanning can resynchronize on `magic`.

---

## `util/queue.rs`

Responsibilities:

- Fixed-capacity queue.
- When full, drop oldest and keep newest.

This module must be fully unit tested.

---

## `util/status.rs`

Responsibilities:

- Convert board state and error flags into LED patterns.
- No GPIO access.

This module must be fully unit tested.

Normal blue LED3 status policy:

See [Firmware LED Semantics](#4-firmware-led-semantics). Keep policy changes
there and in `util::status` tests together.

---

## `tasks/sensor.rs`

Embassy task for I2C sensors.

Responsibilities:

- Read SHT40.
- Read OPT3001.
- Produce `EnvSample`.
- Never block on Wi-Fi.

---

## `tasks/mic.rs`

Embassy task for microphone ADC.

Responsibilities:

- Sample ADC.
- Generate `MicSample`.
- Never store raw audio long-term.

---

## `tasks/aggregator.rs`

Embassy task for data aggregation.

Responsibilities:

- Merge `EnvSample` and `MicSample`.
- Produce `Measurement`.
- Submit `Measurement` records to the storage path.

Pure merge logic should be unit tested.

---

## `tasks/storage.rs`

Embassy task for persistent measurement spooling.

Responsibilities:

- Receive measurements from aggregation.
- Append records to the internal SPI flash spool and maintain a RAM pending mirror.
- Recover pending records from flash at boot.
- Skip legacy unflagged payloads from the previous CSV spool format.
- Serve the oldest pending JSON field-fragment payload to the uploader.
- Acknowledge records only after successful upload.
- Serialize flash access so sensor and upload tasks do not directly block on erase/write operations.
- Report storage errors without stopping sampling.

The task should keep flash operations short and bounded. Long erase/write work must not run inside sensor sampling tasks.

Target-side protocol:

```text
StorageCommand::Append(Measurement)
StorageCommand::Peek
StorageCommand::Ack

StorageResponse::Peeked(Option<StoredPayload>)
StorageResponse::Acked(bool)
StorageResponse::Error(StorageError)
```

`Append` has no response so aggregation cannot block behind upload response handling. `Peek` returns the oldest pending JSON field-fragment payload copied out of the spool RAM mirror. `Ack` removes exactly one oldest pending record and is issued only by the uploader after HTTP 2xx.

---

## `tasks/wifi.rs`

Embassy task for Wi-Fi state management.

Responsibilities:

- Connect to Wi-Fi.
- Reconnect on failure.
- Maintain network state.
- Use backoff on repeated failures.

Pure Wi-Fi state transition logic should be unit tested.

---

## `tasks/upload.rs`

Embassy task for upload.

Responsibilities:

- Read the oldest pending JSON field-fragment payload from the storage task.
- Resolve the REST endpoint from persistent config, UDP discovery, or static fallback.
- Synchronize wall-clock time with SNTP/NTP or `GET /api/v1/time` when possible.
- Build the JSON schema version 1 upload body.
- Upload when IP networking is available.
- Classify transport, malformed response, and non-2xx HTTP failures.
- Acknowledge the record only after HTTP 2xx.
- Do not block sensor sampling.
- Report upload errors.

Payload encoding must be unit tested.

---

## `tasks/ble.rs`

Embassy task boundary and pure transfer core for Bluetooth Low Energy upload.

Current Phase 24A through Phase 24Q
responsibilities:

- Define project-specific protocol constants and structured status, metadata,
  fragment, control, and ACK-policy helper types.
- Model oldest-record metadata, ordered fragment delivery, complete-record
  confirmation, disconnect reset, and ACK decisions without requiring hardware.
- Keep BLE and Wi-Fi storage responses routed as separate clients.
- Monitor BOOT / IO9 as an active-low input in BLE feature builds and maintain
  a pure pairing-window gesture state machine.
- Keep BOOT / IO9 configured as input-only with no internal pull resistor.
- Own `esp_radio::ble::controller::BleConnector` and a TrouBLE peripheral host
  when the firmware is built with `--features ble-upload`.
- Advertise a project-specific GATT service skeleton with status, record
  metadata, record fragment, and control characteristics.
- Keep status readable for BLE runtime state.
- Share the BOOT / IO9 pairing-window state with the GATT task and reject
  closed-window record metadata, record fragment, and control access with ATT
  authorization errors.
- Read the oldest pending record through `storage_task` using the BLE storage
  client and prepare structured metadata and ordered fragments for authorized
  GATT requests.
- Mark `CompleteRecord` in the in-memory transfer session.
- Maintain a shared latest network/upload status snapshot for BLE ACK policy
  decisions without consuming the existing single-consumer status `Signal`s
  used by the LED/status task.
- Maintain a shared latest firmware status snapshot for BLE status reads,
  including pending storage record count and latest firmware error flags,
  without consuming the LED/status task signals.
- On authorized `AckRecord`, suppress BLE storage ACK while Wi-Fi upload is
  connected/IP-ready and the last upload result is success.
- When the ACK policy permits BLE drain, send
  `StorageCommand::Ack { client: StorageClient::Ble, sequence }`; storage still
  owns flash-backed deletion and sequence-checks the ACK.
- Keep BLE and Wi-Fi upload code paths independently compile-selectable:
  `wifi-upload` is the default REST upload feature, `ble-upload` can compile
  without Wi-Fi, and `radio-coex` explicitly selects both radios plus
  `esp-radio/coex`.
- Keep legacy advertising data and scan response data within the 31-byte BLE
  payload limit. The BLE advertising payload carries flags plus the project
  128-bit service UUID, and the scan response carries the complete local name.
- Support central-side discovery, connection, project GATT service discovery,
  and structured status reads.
- Reject closed-window record metadata reads, record fragment reads, and
  control writes with ATT authorization errors.
- Keep the original 10-byte BLE status prefix stable and append
  central-readable BOOT / IO9 pairing diagnostics: pairing state, button state,
  pairing-window remaining milliseconds, and accumulated press milliseconds.
- Support authorized central-side full-record transfer through metadata,
  ordered fragment reads, CRC-validated payload reconstruction, and
  `CompleteRecord`.
- Support ACK-mode BLE storage drain through the existing
  sequence-checked `StorageCommand::Ack { client: StorageClient::Ble, sequence }`
  path when the BLE ACK policy permits it.
- In `ble-upload` target builds, inspect BLE authorization metadata at startup
  and use the config-gated policy to open the temporary BOOT / IO9
  authorization window when the record set is absent, invalid, empty,
  version-incompatible, or checksum-mismatched.
- In `ble-upload` target builds, load structured BLE authorization records
  from the reserved auth sector, restore TrouBLE bond information before host
  build, seed TrouBLE security from TRNG, proactively request security only
  while the BOOT / IO9 pairing window is open, require encryption for
  measurement metadata/fragment/control access when saved records exist, and
  store a bond record on `PairingComplete`.
- Clear saved BLE authorization records from `0x003bf000..0x003c0000` after an
  8 second runtime BOOT / IO9 hold, then reopen the temporary authorization
  window.
- Publish BLE runtime and pairing-window status to the LED status task so blue
  LED3 can show time-bounded BLE indications while red LED2 remains heartbeat.
- Provide pure LED3 BLE timing and pattern helpers for the 180 second boot
  window, 10 second BOOT / IO9 trigger window, pairing-window fast blink, and
  advertising/connected slow blink.

Future runtime responsibilities:

- Validate the remaining BLE pairing/security and authorization-record paths on
  hardware, including runtime user-controlled clearing, rejected access after
  that runtime clear gesture, and record replacement/update. Phase 24R has
  already validated the first Windows saved-bond write and reboot-restore path.
  Phase 24T has already validated auth metadata reset auto-pair behavior and
  unpaired protected-metadata rejection after a reset/invalid-auth window
  closes.
- Validate remaining live Wi-Fi/BLE ACK race behavior, BOOT download-mode
  preservation, BLE auth record replacement/update behavior beyond the first
  saved bond write, and LED3 hardware visual behavior.
- Never write the measurement spool directly; use `storage_task` for all
  measurement append/peek/ACK behavior. BLE auth-sector writes are limited to
  the reserved `0x003bf000..0x003c0000` authorization record sector.
- Never block sensor sampling, microphone sampling, aggregation, Wi-Fi
  reconnect, or REST upload.

BLE protocol framing, fragment ordering, sequence-checked ACK policy,
disconnect reset, BOOT / IO9 pairing-window gesture logic, BLE authorization
metadata header parsing, and auto-pair policy decisions have
hardware-independent Phase 24A/24B/24C/24O tests. The Phase 24D GATT skeleton,
Phase 24E authorized read-only record path, Phase 24F runtime BLE ACK wiring,
Phase 24G independent radio feature matrix, Phase 24H BLE status runtime
snapshot, Phase 24I advertising payload sizing, Phase 24J central-side status
and closed-window authorization behavior, and Phase 24O startup auth-header
read policy compile against the ESP32-C3 target. Phase 24K adds
central-readable pairing diagnostics to the status frame. Phase 24I hardware
validation confirms that the BLE+Wi-Fi firmware reaches the board-side
advertising loop. Phase 24J central validation confirms discovery, connection,
structured status reads, and closed-window measurement access rejection. Phase
24K central validation confirms BOOT / IO9 active-low runtime input,
long-press pairing-window entry, and the expected no-retrigger behavior until
release. Phase 24L central validation confirms full BLE record transfer, CRC
validation, `CompleteRecord`, and ACK-mode BLE storage drain while Wi-Fi upload
is unavailable. Phase 24M central validation confirms fragment notifications
matching requested fragment reads. Phase 24N adds storage-level unit coverage
that a stale BLE ACK after Wi-Fi ACK does not remove the next oldest record.
Phase 24P central validation confirms post-ACK oldest-record advancement and
disconnect-before-Complete/ACK preservation after draining enough records to
avoid full-spool drop-oldest interference. Phase 24P also compile-validates the
LED3 BLE overlay path and adds pure timing/pattern tests. Phase 24Q
compile-validates the TrouBLE security/bond-record persistence path and adds
authorization record load/store tests. Phase 24R hardware-validates Windows
Custom ConfirmOnly pairing, BLE auth-sector write after `PairingComplete`,
startup restore of one saved authorization record after reboot, and encrypted
`no-pair` metadata access through the saved bond. Phase 24T hardware-validates
auth metadata reset auto-pair behavior for missing, invalid, empty,
records-version-mismatched, compatibility-checksum-mismatched, and
header-checksum-mismatched metadata, and confirms unpaired protected-metadata
rejection after a reset/invalid-auth window closes.
Live Wi-Fi/BLE ACK race behavior, runtime saved-auth clearing, rejection after
the runtime clear gesture, BLE auth record replacement/update behavior, LED3
BLE indication hardware behavior, and BOOT download-mode preservation still
need future hardware/runtime validation.

---

## `tasks/led.rs`

Embassy task for LED status.

Responsibilities:

- Drive active-low LEDs.
- Reflect board status.
- No business logic inside this task.

LED status mapping should be tested in `util/status.rs`.

---

## 7. Data Flow

```text
sensor_task ── EnvSample ┐
                         ├── aggregator_task ── storage_task ── MeasurementSpool ── uploader_task ── Wi-Fi REST
mic_task ───── MicSample ┘
                                                    └────────── ble_task boundary ── future BLE GATT

wifi_task ── NetworkState
led_task  ── BoardStatus / ErrorFlags
```

`MeasurementSpool` is owned by `storage_task`:

```text
RAM pending mirror: recovered/active pending records for quick oldest-payload access
Internal SPI flash spool: persistent FIFO append/ack log that survives reset and power loss
```

Rules:

```text
sensor_task does not depend on Wi-Fi
sensor_task does not depend on BLE
mic_task does not depend on Wi-Fi
mic_task does not depend on BLE
aggregator_task does not upload
aggregator_task does not write flash directly
storage_task is the only task that writes the measurement flash region
uploader_task does not read sensors
uploader_task does not erase flash except through storage/spool acknowledgement
ble_task does not erase measurement flash except through storage/spool acknowledgement
ble_task may update only the reserved BLE auth sector after BLE PairingComplete
wifi_task does not process sensor data
```

Persistence rules:

```text
append measurement before treating it as durable
upload oldest valid record first
acknowledge only after HTTP 2xx
for BLE, acknowledge only after authorized central confirmation when Wi-Fi upload is unavailable
preserve pending records across reset
drop oldest when the configured persistent spool is full
never write measurement records outside the configured flash spool region
never write BLE auth records outside the configured BLE auth sector
```

---

## 8. Unit Test Policy

Unit tests must:

- Run on host.
- Not require ESP32 hardware.
- Not require I2C devices.
- Not require ADC input.
- Not require Wi-Fi.
- Not require LED observation.
- Not require button pressing.
- Not depend on timing visible to a human.

Unit tests should test only deterministic pure logic.

---

## 9. Unit Test Scope

Unit test these:

```text
ErrorFlags insert / contains
SHT40 CRC
SHT40 raw temperature conversion
SHT40 raw humidity conversion
SHT40 measurement frame parsing
OPT3001 raw_to_lux
Microphone mean / RMS / peak
Microphone clip_count
Microphone empty input handling
DropOldestQueue push / pop
DropOldestQueue full behavior
status_to_leds
merge_measurement
measurement_to_json_fields
build_measurement_json
resolve_endpoint
http_response_class
select_timestamp
BLE frame encode / decode
BLE fragment ordering
BLE ACK policy with Wi-Fi available / unavailable
BLE authorization metadata header parsing and auto-pair policy
Wi-Fi state transition
Wi-Fi backoff calculation
Spool record encode / decode
Spool CRC validation
Spool append / peek / acknowledge
Spool recovery after partial write
Spool full behavior drops oldest
StorageBacklog append order
StorageBacklog HTTP-success ack behavior
StorageBacklog upload-failure preservation
StorageBacklog recovery order
StorageBacklog error flag reporting
```

Do not unit test these:

```text
LED actually blinking
I2C device detection
Real SHT40 read
Real OPT3001 read
Real ADC microphone response
Real Wi-Fi connection
Real HTTP request
Real internal SPI flash erase / write
USB enumeration
Button behavior
```

Those are board integration tests, not unit tests.

---

## 10. Integration Test Scope

Integration tests run on real hardware.

Suggested checks:

```text
Board boots without reset loop
I2C scan finds 0x44 and 0x45
SHT40 returns reasonable temperature / humidity
OPT3001 returns reasonable lux
Microphone RMS changes when sound is present
Wi-Fi connects
Upload succeeds
Wi-Fi disconnect does not stop sampling
Queue keeps latest data when upload is unavailable
Pending data survives reset and uploads after reconnect
Flash spool full condition drops oldest records
Interrupted write does not cause reset loop
```

These tests may require hardware, but they are not unit tests.
