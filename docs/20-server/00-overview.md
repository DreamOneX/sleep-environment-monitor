# Server Overview

The `server/` directory contains the formal measurement ingestion server.

The Phase 23 implementation replaces the stdlib-only Phase 22 receiver with a
packaged Python application:

- [../../server/pyproject.toml](../../server/pyproject.toml) defines the
  package, dependencies, console script, and check tooling.
- [../../server/src/sleep_env_server/](../../server/src/sleep_env_server/)
  contains the FastAPI app, CLI, configuration, models, UDP discovery, output,
  and in-process storage helpers.
- [../../server/post_receiver.py](../../server/post_receiver.py) remains as a
  compatibility wrapper for old hardware validation commands.

## Implemented Role

The formal server:

- Accepts RESTful measurement uploads from one or more devices.
- Returns HTTP 2xx only after the upload passes validation and the configured
  storage ACK policy is satisfied.
- Treats duplicate `(device_id, sequence)` uploads according to the configured
  storage deduplication policy.
- Provides a server time endpoint so firmware can obtain real-world time when
  NTP is unavailable.
- Publishes a discovery document for automatic endpoint discovery.
- Responds to the UDP discovery query used by Phase 22 firmware.
- Provides an `argparse` CLI for serving, configuration checks, and discovery
  metadata inspection.
- Uses FastAPI, Uvicorn, Pydantic, and Rich instead of raw `http.server`
  request handling.
- Keeps Ruff formatter and linter output advisory and check-only.

## Current Observable Behavior

This section records the behavior implemented by
[../../server/src/sleep_env_server/](../../server/src/sleep_env_server/) rather
than future storage or deployment intent.

### HTTP API

| Request | Current behavior |
|---|---|
| `POST /api/v1/measurements` with a valid schema-version-1 body | Accepts the upload into the configured sink, writes enabled durable stores, emits bounded acceptance metadata, and returns `204 No Content` when the ACK policy is satisfied. |
| `POST /api/v1/measurements` with invalid JSON or an invalid model | Returns a FastAPI/Pydantic request-validation error response and does not accept the upload. |
| Repeated `POST /api/v1/measurements` with the same `(device_id, sequence)` | Follows the configured deduplication strategy. The default `keep_first` behavior returns `204 No Content` and preserves the first canonical record. A `reject` policy returns non-2xx for conflicting duplicates when that rejection prevents the ACK policy from being satisfied. |
| `POST` to another path, including the removed `/measurements` CSV path | Returns `404 Not Found`. |
| `GET /api/v1/time` | Returns `{"unix_ms": <server wall-clock epoch milliseconds>, "source": "server"}` at request time. |
| `GET /api/v1/history/measurements` | Registered only when `[history_api].enabled = true`; requires `Authorization: Bearer <token>` and returns filtered, paginated persisted records. |
| `GET /api/v1/history/summary` | Registered only when `[history_api].enabled = true`; requires `Authorization: Bearer <token>` and returns counts and metric averages for the selected range. |
| `GET /.well-known/sleep-environment-monitor` | Returns API path metadata and the configured UDP discovery port. It does not include the server HTTP host or port. |

The accepted upload model currently enforces:

- `schema_version` is exactly `1`.
- `device_id` is non-empty; `sequence`, `uptime_ms`, `mic_clip_count`, and
  `error_flags` are non-negative integers.
- `time_status` is either `uptime_only` or `wall_clock_synced`.
- `wall_clock_unix_ms` may be omitted or set to a non-negative integer.
- The model does not currently enforce a relationship between `time_status`
  and the presence of `wall_clock_unix_ms`; firmware is responsible for
  following the contract documented in [01-rest-api.md](01-rest-api.md).
- `temperature_c`, `humidity_percent`, and `lux` may be JSON `null`.
- Microphone fields remain required numbers.
- Undeclared JSON fields are rejected.

### State And Diagnostics

- Accepted records are tracked in the running process and, when configured,
  written to SQLite, JSONL, or both. The generated default configuration enables
  SQLite at `./sleep-environment.db` and requires it for upload ACK.
- JSONL compaction and cross-store backfill helpers are implemented. Backfill
  runs once at startup when enabled and can run periodically in a background
  maintenance thread.
- The history read API is disabled by default. When enabled, it requires a
  configured Bearer token and reads from SQLite, JSONL, or a configured merged
  view.
- Successful upload diagnostics include client source address, request byte
  count, `device_id`, `sequence`, and duplicate status. Sensor values and the
  unbounded body are not written to the server event output.
- Storage ACK failures emit bounded rejection diagnostics and return non-2xx to
  preserve the firmware retry contract.
- There is no HTTP authentication, authorization, or transport-security setup
  for upload, time, or discovery endpoints.

### UDP Discovery

- `serve` starts a background IPv4 UDP responder bound to the configured host
  and discovery port; defaults are `0.0.0.0:39022`.
- A UTF-8 datagram whose trimmed contents equal
  `sleep-environment-monitor.discovery` receives compact JSON containing a
  peer-reachable server `host`, configured HTTP `port`, `api_base`,
  `measurement_upload`, and `time`.
- Non-matching or invalid-UTF-8 datagrams are ignored without a response.
- If UDP discovery cannot bind, the server emits a
  `udp_discovery_disabled` event and the independently started HTTP serving
  path is not stopped by the UDP responder.

### Command Line

| Command | Current behavior |
|---|---|
| `sleep-env-server serve` | Loads TOML configuration, starts configured storage and optional maintenance backfill, then starts Uvicorn HTTP serving and the UDP responder. Defaults to host `0.0.0.0`, HTTP port `8080`, UDP port `39022`, and log level `info`. Rich event output is used for an interactive stdout unless `--no-rich` is passed; `--json-log` selects JSONL event output. |
| `sleep-env-server check-config` | Validates XDG or explicit TOML plus CLI overrides without opening sockets and prints a plain `config_ok` event on success. |
| `sleep-env-server print-discovery` | Prints the HTTP discovery document and an example UDP discovery response in `rich`, `plain`, or `json` output. |
| `python3 server/post_receiver.py` | Compatibility entry point; inserts `serve` when invoked without an explicit subcommand. |

The current configuration surface is TOML plus CLI overrides. Without
`--config`, commands read or generate
`$XDG_CONFIG_HOME/sleep-env-server/config.toml`, falling back to
`~/.config/sleep-env-server/config.toml` when `XDG_CONFIG_HOME` is unset.

## Boundaries

Durable SQLite/JSONL storage, upload ACK policy, and authenticated history
reads are implemented for Phase 26. Deployment service management, upload
authentication/authorization, and retention cleanup enforcement remain future
work.

Phase 26 continues to add the planned local operator surfaces:

- Rich live dashboard and offline history views.

Phase 24 BLE upload planning does not add a server-side BLE protocol. If a
future phone or gateway receives measurements over BLE and forwards them to the
server, it should use the existing REST API contract.

## Related Docs

- [01-rest-api.md](01-rest-api.md): firmware/server REST API contract.
- [02-toolchain.md](02-toolchain.md): Python toolchain, style policy,
  formatter/linter policy, and unit-test expectations.
- [03-cli.md](03-cli.md): `argparse` command surface.
- [04-persistence-configuration.md](04-persistence-configuration.md): persistence,
  TOML configuration, history API, and Rich display plan.
- [../10-firmware/03-network.md](../10-firmware/03-network.md): firmware network responsibilities.
- [../30-integration/00-network-roadmap.md](../30-integration/00-network-roadmap.md): cross-component roadmap.
