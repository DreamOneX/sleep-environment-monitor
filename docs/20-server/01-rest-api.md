# REST API

This document defines the Phase 22 firmware/server REST contract.

The current local receiver, [../../server/post_receiver.py](../../server/post_receiver.py), implements the Phase 22 JSON upload, time, and discovery endpoints for validation. It does not preserve the old `/measurements` CSV bring-up endpoint.

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

The Phase 22 receiver returns `404` for other `POST` paths.

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

The Phase 22 receiver listens for discovery datagrams on UDP port `39022`.

- Query payload: `sleep-environment-monitor.discovery`.
- Response fields: `host`, `port`, `api_base`, `measurement_upload`, and `time`.
- The firmware currently consumes IPv4 host values from this response.
