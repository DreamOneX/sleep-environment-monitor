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
- Status LEDs:
  - LED1: runtime heartbeat
  - LED2: error / Wi-Fi / upload status
- Hardware-independent unit tests for all pure logic.

---

## 3. Hardware Summary

## MCU

```text
ESP32-C3-WROOM-02-N4
```

The module includes 4 MB internal SPI flash. Firmware uses a dedicated flash storage region for persisted measurement spooling and must not write into bootloader, partition table, app image, or RF/calibration data regions.

Flash spool layout:

```text
total flash:            0x0000_0000..0x0040_0000  4 MiB
system reserved:        0x0000_0000..0x0001_0000  bootloader / partition / RF data area
application reserved:   0x0001_0000..0x003c_0000  firmware image growth area
measurement spool:      0x003c_0000..0x0040_0000  256 KiB, 4 KiB sector aligned
```

The firmware validates this layout at runtime before enabling flash writes. A zero-sized spool, sector-unaligned range, out-of-bounds range, or overlap with protected regions is treated as a configuration error.

## Pin Mapping

| Function | GPIO | Notes |
|---|---:|---|
| USB D- | IO18 | Native USB |
| USB D+ | IO19 | Native USB |
| I2C SDA | IO4 | Shared I2C bus |
| I2C SCL | IO5 | Shared I2C bus |
| Microphone ADC | IO3 / ADC1_CH3 | Analog input |
| LED1 | IO0 | Active-low |
| LED2 | IO1 | Active-low |
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

## 4. Current Source Tree

```text
Cargo.toml
firmware/
├── Cargo.toml
├── build.rs
├── README.md
└── src
    ├── bin
    │   └── main.rs
    ├── lib.rs
    ├── board.rs
    ├── types.rs
    ├── drivers
    │   ├── mod.rs
    │   ├── sht40.rs
    │   ├── opt3001.rs
    │   ├── mic.rs
    │   └── flash.rs
    ├── storage
    │   ├── mod.rs
    │   ├── flash_model.rs
    │   └── spool.rs
    ├── tasks
    │   ├── mod.rs
    │   ├── sensor.rs
    │   ├── mic.rs
    │   ├── aggregator.rs
    │   ├── storage.rs
    │   ├── net.rs
    │   ├── wifi.rs
    │   ├── upload.rs
    │   ├── ble.rs
    │   └── led.rs
    └── util
        ├── mod.rs
        ├── logging.rs
        ├── queue.rs
        └── status.rs
server/
├── README.md
└── post_receiver.py
docs/
├── README.md
├── 00-project/
├── 10-firmware/
├── 20-server/
└── 30-integration/
```

The root `Cargo.toml` is a workspace manifest. The firmware package remains named `sleep-environment-monitor`, and root-level Cargo commands target `firmware` by default.

---

## 5. Module Responsibilities

Firmware module paths in this section are relative to `firmware/src/`.

## `board.rs`

Contains board constants only.

```rust
pub const PIN_I2C_SDA: u8 = 4;
pub const PIN_I2C_SCL: u8 = 5;

pub const PIN_MIC_ADC: u8 = 3;

pub const PIN_LED1: u8 = 0;
pub const PIN_LED2: u8 = 1;

pub const I2C_ADDR_SHT40: u8 = 0x44;
pub const I2C_ADDR_OPT3001: u8 = 0x45;

pub const FLASH_TOTAL_SIZE_BYTES: u32 = 4 * 1024 * 1024;
pub const FLASH_SECTOR_SIZE_BYTES: u32 = 4096;

pub const FLASH_SPOOL_REGION_OFFSET: u32 = 0x003c_0000;
pub const FLASH_SPOOL_REGION_SIZE: u32 = 0x0004_0000;

pub const FLASH_SYSTEM_RESERVED_OFFSET: u32 = 0x0000_0000;
pub const FLASH_SYSTEM_RESERVED_SIZE: u32 = 0x0001_0000;

pub const FLASH_APP_RESERVED_OFFSET: u32 = 0x0001_0000;
pub const FLASH_APP_RESERVED_SIZE: u32 =
    FLASH_SPOOL_REGION_OFFSET - FLASH_APP_RESERVED_OFFSET;
```

Flash spool constants must pass `drivers::flash::validate_default_flash_spool_region()` before flash writes are enabled.

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
- `config.rs` owns Wi-Fi defaults, REST fallback endpoint details, timing policy, buffer sizes, logging intervals, and other deployment knobs.
- Drivers keep protocol constants and conversion math.

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
  or after future BLE upload receives paired-central complete-record
  confirmation while Wi-Fi upload is unavailable.
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

LED2 policy:

| Priority | Condition | LED2 pattern | Meaning |
|---:|---|---|---|
| 1 | `ErrorFlags::SENSOR_MASK` intersects current flags | `FastBlink` | SHT40, OPT3001, or microphone failure |
| 2 | `ErrorFlags::UPLOAD_MASK` intersects current flags, `ErrorFlags::TIME` is set, or `ErrorFlags::STORAGE` is set | `On` | Measurement upload, time sync, or persistent storage is failing |
| 3 | Wi-Fi is disconnected or `ErrorFlags::NETWORK_MASK` intersects current flags | `SlowBlink` | Network is not ready |
| 4 | No current error and IP networking is ready | `Off` | Normal operation |

Timing:

- LED outputs are active-low: `LOW = on`, `HIGH = off`.
- `FastBlink` toggles every 100 ms.
- `SlowBlink` toggles every 500 ms.
- LED1 is a separate runtime heartbeat: 100 ms on, 900 ms off.

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

Current Phase 24A/24B/24C/24D/24E/24F/24G/24H/24I/24J/24K responsibilities:

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

Future runtime responsibilities:

- Replace the pairing-window authorization skeleton with validated real BLE
  pairing/security or a documented equivalent authorization flow.
- Validate live GATT record transfer and BLE storage drain with a BLE central.
- Never write flash directly.
- Never block sensor sampling, microphone sampling, aggregation, Wi-Fi
  reconnect, or REST upload.

BLE protocol framing, fragment ordering, sequence-checked ACK policy,
disconnect reset, and BOOT / IO9 pairing-window gesture logic have
hardware-independent Phase 24A/24B/24C tests. The Phase 24D GATT skeleton,
Phase 24E authorized read-only record path, Phase 24F runtime BLE ACK wiring,
Phase 24G independent radio feature matrix, Phase 24H BLE status runtime
snapshot, Phase 24I advertising payload sizing, and Phase 24J central-side
status and closed-window authorization behavior compile or run against the
ESP32-C3 target. Phase 24K adds central-readable pairing diagnostics to the
status frame. Phase 24I hardware validation confirms that the BLE+Wi-Fi
firmware reaches the board-side advertising loop. Phase 24J central validation
confirms discovery, connection, structured status reads, and closed-window
measurement access rejection. Phase 24K central validation confirms BOOT / IO9
active-low runtime input, long-press pairing-window entry, and the expected
no-retrigger behavior until release. Live GATT record transfer and runtime BLE
storage ACK behavior still need future hardware/runtime validation.

---

## `tasks/led.rs`

Embassy task for LED status.

Responsibilities:

- Drive active-low LEDs.
- Reflect board status.
- No business logic inside this task.

LED status mapping should be tested in `util/status.rs`.

---

## 6. Data Flow

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
ble_task does not erase flash except through storage/spool acknowledgement
wifi_task does not process sensor data
```

Persistence rules:

```text
append measurement before treating it as durable
upload oldest valid record first
acknowledge only after HTTP 2xx
for BLE, acknowledge only after paired-central confirmation when Wi-Fi upload is unavailable
preserve pending records across reset
drop oldest when the configured persistent spool is full
never write outside the configured flash spool region
```

---

## 7. Unit Test Policy

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

## 8. Unit Test Scope

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

## 9. Integration Test Scope

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
