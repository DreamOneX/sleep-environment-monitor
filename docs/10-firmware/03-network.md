# Firmware Network

This document describes the firmware-side network responsibilities.
The cross-component roadmap lives in [../30-integration/00-network-roadmap.md](../30-integration/00-network-roadmap.md), and the server REST contract lives in [../20-server/01-rest-api.md](../20-server/01-rest-api.md).

## Goals

Firmware networking must support:

- Wi-Fi station connection and reconnect.
- IP configuration readiness.
- RESTful measurement upload.
- Upload acknowledgement only after server HTTP 2xx.
- Automatic server discovery when available.
- Real-world time synchronization.
- Future BLE measurement upload through a real low-power GATT protocol.

MQTT is out of scope for the current roadmap. The persistent spool already
provides offline durability, REST keeps the server contract simple and
inspectable, and BLE is planned as a local upload path rather than another
server-side protocol.

## Boundaries

Network work should stay separated into these responsibilities:

| Responsibility | Owner |
|---|---|
| Wi-Fi link connection, disconnect detection, and reconnect backoff | `tasks/wifi.rs` |
| Embassy network runner | `tasks/net.rs` |
| IP/DHCP readiness observation | `tasks/wifi.rs`, `tasks/upload.rs` |
| REST endpoint resolution and discovery | `tasks/upload.rs` |
| HTTP transport and response classification | `tasks/upload.rs` |
| Persistent upload ordering and acknowledgement | `tasks/storage.rs` and `tasks/upload.rs` |
| BLE compile boundary and protocol helpers | `tasks/ble.rs` |
| Future BLE advertising, pairing, GATT transfer, and BLE ACK handling | `tasks/ble.rs` runtime bring-up |

Sensor sampling, microphone sampling, aggregation, and flash spooling must not
depend on Wi-Fi, BLE, or server availability.

## REST Upload

The firmware uploads the oldest pending measurement first.

Rules:

- The firmware sends measurements to `POST /api/v1/measurements` after endpoint resolution.
- A record is acknowledged in storage only after an HTTP 2xx response.
- TCP, timeout, response parse, and non-2xx HTTP failures preserve the record.
- Upload failures are reported through status output and LED policy, but do not stop sampling.
- The REST payload must carry enough information for the server to handle duplicates after retry.

Phase 22 replaced the old bring-up CSV upload with JSON schema version 1. The
storage spool persists measurement JSON field fragments so upload can add the
device id, spool sequence, time status, and optional wall-clock timestamp at the
moment the record is sent.

Upload payloads include:

- `schema_version`.
- `device_id`.
- `sequence`.
- `time_status`.
- Optional `wall_clock_unix_ms`.
- `uptime_ms`.
- Temperature, humidity, light, microphone, and error flag fields.

The firmware no longer targets the old `/measurements` CSV endpoint.

During the Phase 22 transition, recovered spool records without the JSON field-fragment payload flag are treated as legacy CSV records and skipped. Recovered records from a previous boot remain uploadable, but they stay `uptime_only`; the firmware does not project old-boot uptime values through the current boot's time-sync anchor.

## Discovery

Discovery should find a REST server without rebuilding firmware.

Resolution precedence:

1. Provisioned endpoint from persistent configuration.
2. Automatic discovery result.
3. Static fallback endpoint from firmware configuration.

Phase 22 implements LAN discovery with UDP:

- UDP port: `39022`.
- Query payload: `sleep-environment-monitor.discovery`.
- Response: compact JSON containing `host`, `port`, `api_base`, `measurement_upload`, and `time`.
- Server metadata document: `GET /.well-known/sleep-environment-monitor`.

The firmware currently parses IPv4 discovery results. If discovery fails, the
static fallback endpoint remains usable.

## Time

Real-world time is a first-class network requirement.

Firmware should treat time as a state:

| State | Meaning |
|---|---|
| `Unknown` | No usable clock beyond boot-relative uptime. |
| `UptimeOnly` | Measurements can be ordered by uptime, but not placed on a wall clock. |
| `WallClockSynced` | Measurements can include an absolute timestamp. |

Phase 22 attempts SNTP/NTP after IP configuration. A REST server time endpoint,
`GET /api/v1/time`, is the fallback because it can work with the same server
used for uploads.

Measurements produced before wall-clock sync must remain uploadable. The payload should preserve `uptime_ms` and include wall-clock fields only when known.

Recovered records from a previous boot must not receive a synthesized wall-clock timestamp from the current boot's sync state. They are uploaded with `time_status` set to `uptime_only` unless future persistent time metadata can prove the uptime origin.

## Wi-Fi Credentials

Firmware configuration supports:

- Open networks.
- WPA-Personal PSK.
- WPA2-Personal PSK.
- WPA/WPA2-Personal mixed PSK.

WPA3 and Enterprise/EAP are intentionally deferred. The current dependency stack
exposes some WPA3 variants, but the crate documentation does not make WPA3 a
validated target capability, and the firmware does not enable EAP features.

## BLE Upload

BLE is an independent upload path, not part of Phase 22 or Phase 23. Phase 24A
adds only the compile boundary and protocol helpers; advertising, pairing, GATT
transfer, and BLE ACK behavior remain future runtime work. See
[05-ble.md](05-ble.md).

BLE must be implemented as Bluetooth Low Energy:

- Use project-specific GATT services and characteristics.
- Do not use Bluetooth Classic SPP.
- Do not use transparent UART or Nordic UART Service style byte streams.
- Do not push CSV or JSON as unframed serial text.

Wi-Fi and BLE are independent features. Either can be enabled or disabled
without disabling sensor sampling, aggregation, or persistent storage.

BLE upload reads from the same persistent measurement spool as Wi-Fi REST
upload. `storage_task` remains the only owner of append, peek, and acknowledge
operations.

ACK rules:

- Wi-Fi REST upload acknowledges a record only after HTTP 2xx.
- BLE may transmit copies while Wi-Fi upload is available and succeeding, but it
  must not acknowledge storage in that state.
- BLE may acknowledge exactly one oldest pending record only when Wi-Fi upload
  is disabled or unavailable and a paired central confirms complete receipt.
- BLE disconnect before confirmation preserves the pending record.

Wi-Fi upload is unavailable when Wi-Fi is disabled, disconnected, lacks IP
configuration, cannot resolve an endpoint, fails transport, or receives a
non-2xx HTTP response.
