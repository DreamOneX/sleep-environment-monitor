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

The module includes 4 MB internal SPI flash. Firmware may use a dedicated flash storage region for persisted measurement spooling, but must not write into bootloader, partition table, app image, or RF/calibration data regions.

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
| BOOT | IO9 | Active-low |
| RESET | EN | Active-low |
| Strap pin | IO8 | Pulled high, do not use as normal I/O |

## I2C Devices

| Device | Address |
|---|---:|
| SHT40-AD1B-R2 | `0x44` |
| OPT3001IDNPRQ1 | `0x45` |

---

## 4. Recommended Source Tree

```text
src
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
│   └── spool.rs
├── tasks
│   ├── mod.rs
│   ├── sensor.rs
│   ├── mic.rs
│   ├── aggregator.rs
│   ├── storage.rs
│   ├── wifi.rs
│   ├── upload.rs
│   └── led.rs
└── util
    ├── mod.rs
    ├── queue.rs
    └── status.rs
```

---

## 5. Module Responsibilities

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

pub const FLASH_SPOOL_REGION_OFFSET: u32 = 0; // Set after partition layout is defined.
pub const FLASH_SPOOL_REGION_SIZE: u32 = 0;   // Set after partition layout is defined.
```

Flash spool constants must be resolved before flash writes are enabled. Placeholder zero values are invalid at runtime.

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
- Use an `embedded-storage` style read/write/erase interface.
- Refuse out-of-range access.
- Keep partition or fixed-region selection outside business logic.

This module may require hardware for full validation, but address arithmetic and range checks should be unit tested where possible.

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
payload: CSV Measurement bytes
padding: erase/write alignment as needed
```

Rules:

- `sequence` is monotonic and used only for ordering/recovery.
- A record is uploadable only after its CRC validates.
- A record is removed from the spool only after upload returns HTTP 2xx.
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
| 2 | Upload result is failed or `ErrorFlags::UPLOAD` is set | `On` | Measurement upload is failing |
| 3 | Wi-Fi is disconnected or `ErrorFlags::WIFI` is set | `SlowBlink` | Network is not ready |
| 4 | No current error and Wi-Fi is connected | `Off` | Normal operation |

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
- Append records to the RAM hot queue and internal SPI flash spool.
- Recover pending records from flash at boot.
- Serve the oldest pending record to the uploader.
- Acknowledge records only after successful upload.
- Serialize flash access so sensor and upload tasks do not directly block on erase/write operations.
- Report storage errors without stopping sampling.

The task should keep flash operations short and bounded. Long erase/write work must not run inside sensor sampling tasks.

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

- Read the oldest pending `Measurement` from the spool.
- Upload when network is available.
- Acknowledge the record only after HTTP 2xx.
- Do not block sensor sampling.
- Report upload errors.

Payload encoding must be unit tested.

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
                         ├── aggregator_task ── storage_task ── MeasurementSpool ── uploader_task ── Wi-Fi
mic_task ───── MicSample ┘

wifi_task ── NetworkState
led_task  ── BoardStatus / ErrorFlags
```

`MeasurementSpool` is a two-level buffer:

```text
RAM hot queue: short-term queue for fresh measurements and quick uploader access
Internal SPI flash spool: persistent FIFO log that survives reset and power loss
```

Rules:

```text
sensor_task does not depend on Wi-Fi
mic_task does not depend on Wi-Fi
aggregator_task does not upload
aggregator_task does not write flash directly
storage_task is the only task that writes the measurement flash region
uploader_task does not read sensors
uploader_task does not erase flash except through storage/spool acknowledgement
wifi_task does not process sensor data
```

Persistence rules:

```text
append measurement before treating it as durable
upload oldest valid record first
acknowledge only after HTTP 2xx
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
measurement_to_csv_line or measurement_to_json
Wi-Fi state transition
Wi-Fi backoff calculation
Spool record encode / decode
Spool CRC validation
Spool append / peek / acknowledge
Spool recovery after partial write
Spool full behavior drops oldest
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
