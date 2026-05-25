# Server CLI

The formal server CLI uses Python stdlib `argparse`.

Typer and Click are intentionally not the first choice for Phase 23. The CLI
surface is small, and using `argparse` keeps the dependency boundary focused on
the server framework and runtime.

## Command Shape

Console script:

```bash
sleep-env-server
```

Equivalent module invocation:

```bash
python -m sleep_env_server
```

Subcommands:

```bash
sleep-env-server serve
sleep-env-server check-config
sleep-env-server print-discovery
sleep-env-server history
```

## `serve`

Runs the HTTP API and UDP discovery responder.

Options:

```text
--host HOST
--port PORT
--udp-discovery-port PORT
--log-level LEVEL
--json-log
--no-rich
--config PATH
```

Defaults:

```text
host: 0.0.0.0
port: 8080
udp-discovery-port: 39022
log-level: info
rich: enabled when stdout is interactive
json-log: disabled
```

Behavior:

- Load XDG or explicit TOML configuration and apply CLI overrides.
- Build configured SQLite/JSONL storage targets before opening network sockets.
- Run startup backfill and retention cleanup when enabled, and start the
  background storage maintenance loop when at least one backend is active.
- Start the FastAPI/Uvicorn HTTP server.
- Start the UDP discovery responder on the configured port.
- Print Rich startup information for local operation unless disabled.
- In interactive Rich mode, show the live measurement/status dashboard.
- Expose the same API paths as the Phase 22 contract.
- Shut down cleanly on interrupt.

## `check-config`

Validates CLI, XDG TOML, explicit TOML, and CLI-overridden configuration without
opening sockets.

Behavior:

- Validate host and port values.
- Validate logging options.
- Validate API path configuration.
- Validate storage, ACK, backfill, history API, and Rich output configuration.
- Exit with status `0` for valid config.
- Exit non-zero and print a clear error for invalid config.

## `print-discovery`

Prints the discovery document and UDP discovery payload that would be served by
the current configuration.

Behavior:

- Print `GET /.well-known/sleep-environment-monitor` metadata.
- Print UDP discovery response fields.
- Support Rich output for humans.
- Support plain or JSON output for tests and scripts.

## `history`

Prints persisted measurement summaries, recent rows, and simple metric trends.

Options:

```text
--config PATH
--output auto|rich|plain|json
--read-source merge|sqlite|jsonl
--device-id DEVICE_ID
--start-unix-ms UNIX_MS
--end-unix-ms UNIX_MS
--limit COUNT
```

Behavior:

- Read from the configured history source.
- Use `[history_cli].read_source`, `[history_cli].tail_count`, and
  `[history_cli].metrics` when corresponding CLI options are omitted.
- Use the history merge source/conflict settings from `[history_api]` for local
  merged reads.
- Show summary counts, device ids, receive time bounds, recent rows, and simple
  ASCII metric trends.
- Support Rich output for humans.
- Support plain or JSON output for tests and scripts.
- Require no HTTP token when reading local storage directly.

## Argument Validation

Required validation:

- HTTP port must be in `1..=65535`.
- UDP discovery port must be in `1..=65535`.
- Log level must be one of the documented values.
- Mutually exclusive output modes must reject invalid combinations.
- Explicit `--config` paths must exist.
- Generated default config must be valid before serving.

Invalid arguments should fail before opening HTTP or UDP sockets.

## Test Expectations

CLI tests should cover:

- Default `serve` configuration.
- Explicit host and ports.
- Log level parsing.
- Rich output enable/disable behavior.
- JSON/plain output switches for `print-discovery`.
- XDG default config generation and explicit config loading.
- Storage ACK policy parsing.
- History command output.
- Invalid port rejection.
- Invalid log-level rejection.
- `check-config` success and failure paths.

Tests should call parser/config-building helpers directly where possible, and
only use subprocess tests for console-script integration behavior that cannot be
validated cleanly in process.
