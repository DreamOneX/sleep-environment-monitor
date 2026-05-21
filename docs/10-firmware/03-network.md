# Firmware Network

This document describes the intended firmware-side network responsibilities.
The cross-component roadmap lives in [../30-integration/00-network-roadmap.md](../30-integration/00-network-roadmap.md), and the server REST contract lives in [../20-server/01-rest-api.md](../20-server/01-rest-api.md).

## Goals

Firmware networking must support:

- Wi-Fi station connection and reconnect.
- IP configuration readiness.
- RESTful measurement upload.
- Upload acknowledgement only after server HTTP 2xx.
- Automatic server discovery when available.
- Real-world time synchronization.
- Future BLE provisioning of network and server settings.

MQTT is out of scope for the current roadmap. The persistent spool already provides offline durability, and REST keeps the server contract simple and inspectable.

## Boundaries

Network work should stay separated into these responsibilities:

| Responsibility | Owner |
|---|---|
| Wi-Fi link connection, disconnect detection, and reconnect backoff | `tasks/wifi.rs` |
| Embassy network runner | `tasks/net.rs` |
| IP/DHCP readiness observation | network orchestration code |
| REST endpoint resolution and discovery | planned Phase 22 network code |
| HTTP transport and response classification | upload transport code |
| Persistent upload ordering and acknowledgement | `tasks/storage.rs` and `tasks/upload.rs` |

Sensor sampling, microphone sampling, aggregation, and flash spooling must not depend on Wi-Fi or server availability.

## REST Upload

The firmware uploads the oldest pending measurement first.

Rules:

- The firmware sends measurements to `POST /api/v1/measurements` after endpoint resolution.
- A record is acknowledged in storage only after an HTTP 2xx response.
- TCP, timeout, response parse, and non-2xx HTTP failures preserve the record.
- Upload failures are reported through status output and LED policy, but do not stop sampling.
- The REST payload must carry enough information for the server to handle duplicates after retry.

The existing CSV upload can remain during transition, but Phase 22 should define a versioned payload shape before adding wall-clock timestamps.

## Discovery

Discovery should find a REST server without rebuilding firmware.

Resolution precedence:

1. Provisioned endpoint from future BLE or persistent configuration.
2. Automatic discovery result.
3. Static fallback endpoint from firmware configuration.

The planned discovery document is `GET /.well-known/sleep-environment-monitor` on the server. The exact transport for finding the host can be chosen in Phase 22 based on available `embassy-net` support and memory cost.

## Time

Real-world time is a first-class network requirement.

Firmware should treat time as a state:

| State | Meaning |
|---|---|
| `Unknown` | No usable clock beyond boot-relative uptime. |
| `UptimeOnly` | Measurements can be ordered by uptime, but not placed on a wall clock. |
| `WallClockSynced` | Measurements can include an absolute timestamp. |

Phase 22 should prefer SNTP/NTP after IP configuration. A REST server time endpoint, `GET /api/v1/time`, is the fallback because it can work with the same server used for uploads.

Measurements produced before wall-clock sync must remain uploadable. The payload should preserve `uptime_ms` and include wall-clock fields only when known.

## BLE Readiness

BLE is a future provisioning path, not part of Phase 21.

Network configuration should be shaped so BLE can later provide:

- Wi-Fi SSID.
- Wi-Fi authentication mode and credential material.
- REST server endpoint or discovery preference.
- Time sync preference if needed.

Firmware code should not bake deployment-specific network details into upload or Wi-Fi tasks.
