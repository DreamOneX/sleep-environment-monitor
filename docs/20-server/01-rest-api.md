# REST API

This document defines the firmware/server REST contract introduced in Phase 22
and preserved by the Phase 23 formal server.

The current implementation lives in
[../../server/src/sleep_env_server/](../../server/src/sleep_env_server/).
[../../server/post_receiver.py](../../server/post_receiver.py) is now a
compatibility wrapper for the formal CLI. The old `/measurements` CSV bring-up
endpoint is not restored.

## Versioning

Initial stable endpoints live under:

```text
/api/v1
```

The discovery document remains under `/.well-known/` so clients can find API details before knowing the versioned API root.

## Endpoints

| Method | Path | Purpose |
|---|---|---|
| `GET` | `/api/v1/time` | Return server wall-clock time for firmware fallback sync. |
| `POST` | `/api/v1/measurements` | Accept measurement uploads. |
| `GET` | `/.well-known/sleep-environment-monitor` | Return discovery metadata. |

The formal server returns `404` for other `POST` paths.

## Upload Semantics

The firmware may acknowledge a persisted measurement only after `POST /api/v1/measurements` returns HTTP 2xx.

Server requirements:

- Return 2xx only after the payload is accepted.
- Return non-2xx for invalid payloads or unavailable ingestion.
- Tolerate duplicate submissions from the same device and sequence. Phase 23
  treats duplicates as idempotent success and returns `204`.
- Preserve enough request metadata for later diagnostics.

Firmware requirements:

- Preserve the record on TCP failure, timeout, parse failure, or non-2xx response.
- Retry the oldest pending record first.
- Continue sampling while uploads fail.

Phase 24 BLE upload does not change this server contract. BLE is a firmware-side
GATT upload path to a nearby central. If that central later forwards records to
the server, it should submit the same JSON schema version 1 payload to
`POST /api/v1/measurements`.

## Measurement Payload

The firmware sends JSON schema version 1:

```json
{
  "schema_version": 1,
  "device_id": "sleep-env-esp32c3",
  "sequence": 0,
  "time_status": "uptime_only",
  "wall_clock_unix_ms": 0,
  "uptime_ms": 0,
  "temperature_c": 0.0,
  "humidity_percent": 0.0,
  "lux": 0.0,
  "mic_mean": 0.0,
  "mic_rms": 0.0,
  "mic_peak": 0.0,
  "mic_db_rel": 0.0,
  "mic_clip_count": 0,
  "error_flags": 0
}
```

Rules:

- `wall_clock_unix_ms` is omitted until wall-clock time is synchronized.
- `time_status` is `uptime_only` or `wall_clock_synced` for upload payloads.
- Nullable sensor fields use JSON `null` when a sensor value is missing.
- `sequence` is the persistent spool sequence and is used with `device_id` for duplicate handling.
- The old CSV payload format is no longer part of the Phase 22 API.

## Time Response

`GET /api/v1/time` returns the server's current wall-clock time:

```json
{
  "unix_ms": 0,
  "source": "server"
}
```

Firmware keeps `uptime_ms` even after wall-clock sync succeeds.

## Discovery Document

`GET /.well-known/sleep-environment-monitor` describes the server API root and supported features:

```json
{
  "api_base": "/api/v1",
  "measurement_upload": "/api/v1/measurements",
  "time": "/api/v1/time",
  "udp_discovery_port": 39022
}
```

Discovery should complement, not replace, provisioned configuration. A static firmware fallback remains useful for bring-up.

## UDP Discovery

The formal server listens for discovery datagrams on UDP port `39022` by
default.

- Query payload: `sleep-environment-monitor.discovery`.
- Response fields: `host`, `port`, `api_base`, `measurement_upload`, and `time`.
- The firmware currently consumes IPv4 host values from this response.

## Implementation Notes

The Phase 23 formal server keeps behavior equivalent to the Phase 22 receiver
unless this contract is intentionally revised in a later phase.

Requirements:

- `POST /api/v1/measurements` returns a 2xx response only after accepting
  the upload for processing.
- Invalid JSON, invalid measurement schema, or unavailable ingestion returns
  non-2xx.
- Other `POST` paths return `404`.
- `GET /api/v1/time` returns integer `unix_ms` and source metadata.
- `GET /.well-known/sleep-environment-monitor` describes the active API
  paths and UDP discovery port.
- UDP discovery responds only to the documented query payload.
- The old `/measurements` CSV endpoint is not restored.
