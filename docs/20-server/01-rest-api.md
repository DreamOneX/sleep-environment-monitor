# REST API

This document defines the planned firmware/server REST contract. It is not yet fully implemented by the temporary receiver.

## Versioning

Initial stable endpoints should live under:

```text
/api/v1
```

The discovery document remains under `/.well-known/` so clients can find API details before knowing the versioned API root.

## Endpoints

| Method | Path | Purpose |
|---|---|---|
| `GET` | `/api/v1/health` | Confirm the server is reachable. |
| `GET` | `/api/v1/time` | Return server wall-clock time for firmware fallback sync. |
| `POST` | `/api/v1/measurements` | Accept measurement uploads. |
| `GET` | `/.well-known/sleep-environment-monitor` | Return discovery metadata. |

## Upload Semantics

The firmware may acknowledge a persisted measurement only after `POST /api/v1/measurements` returns HTTP 2xx.

Server requirements:

- Return 2xx only after the payload is accepted.
- Return non-2xx for invalid payloads or unavailable ingestion.
- Tolerate duplicate submissions from the same device and sequence.
- Preserve enough request metadata for later diagnostics.

Firmware requirements:

- Preserve the record on TCP failure, timeout, parse failure, or non-2xx response.
- Retry the oldest pending record first.
- Continue sampling while uploads fail.

## Measurement Payload

Phase 22 should define a versioned payload before adding wall-clock fields. The payload should include:

- Device identifier.
- Firmware payload schema version.
- Monotonic upload or spool sequence.
- `uptime_ms`.
- Optional wall-clock timestamp and time-sync status.
- Temperature, humidity, light, microphone, and error flag fields.

The existing CSV format can remain for transition validation, but the formal REST API should not rely on implicit column order without a schema version.

## Time Response

`GET /api/v1/time` should return the server's current wall-clock time and enough metadata for firmware to decide whether it is usable.

Minimum intended fields:

```json
{
  "unix_ms": 0,
  "source": "server"
}
```

The exact JSON shape can be adjusted during Phase 22, but firmware must keep `uptime_ms` even after wall-clock sync succeeds.

## Discovery Document

`GET /.well-known/sleep-environment-monitor` should describe the server API root and supported features.

Minimum intended fields:

```json
{
  "api_base": "/api/v1",
  "measurement_upload": "/api/v1/measurements",
  "time": "/api/v1/time"
}
```

Discovery should complement, not replace, provisioned configuration. A static firmware fallback remains useful for bring-up.
