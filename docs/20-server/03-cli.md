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
sleep-env-server tui
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
--rich-log
--config PATH
```

Defaults:

```text
host: 0.0.0.0
port: 8080
udp-discovery-port: 39022
log-level: info
output: plain service events by default
json-log: disabled
rich-log: disabled
```

Behavior:

- Load XDG or explicit TOML configuration and apply CLI overrides.
- Build configured SQLite/JSONL storage targets before opening network sockets.
- Run startup backfill and retention cleanup when enabled, and start the
  background storage maintenance loop when at least one backend is active.
- Start the FastAPI/Uvicorn HTTP server.
- Start the UDP discovery responder on the configured port.
- Print plain service events by default.
- Print JSONL service events when `--json-log` is used.
- Use styled Rich service logging only when `--rich-log` is used.
- Do not render live measurement charts. `serve` is the scriptable service
  entry point; use `tui` for the full-screen local operator view.
- Expose the same API paths as the Phase 22 contract.
- Shut down cleanly on interrupt.

## `tui`

Runs the HTTP API and UDP discovery responder under a Textual full-screen
terminal UI.

Options:

```text
--host HOST
--port PORT
--udp-discovery-port PORT
--log-level LEVEL
--transparent
--no-autostart
--config PATH
```

Behavior:

- Load the same TOML configuration and CLI overrides as `serve`.
- Start the same configured storage, backfill, retention, FastAPI/Uvicorn HTTP
  service, and UDP discovery responder by default.
- Support `[tui].autostart = false` or `--no-autostart` to open the operator
  interface with the service stopped.
- Show service status, metric cards, recent measurements, metric trend charts,
  and bounded event logs in a full-screen TUI.
- Use `[tui].measurements_limit` to control how many accepted measurements are
  retained in the live TUI table and chart window.
- Use Catppuccin Mocha by default. Existing `theme = "graphite"` configuration
  remains accepted for compatibility.
- Enable transparent-background styling with `[tui].transparent = true` or
  `--transparent` for terminals that already provide window transparency.
- Keep Uvicorn and server diagnostics inside the TUI event panel rather than
  writing over the terminal screen.
- Support `s`, `q`, `Ctrl+C`, `c`, `r`, and `?` as operator actions. `s`
  starts the service when stopped and stops it when running.
- Preserve REST, UDP discovery, and upload ACK behavior exactly as `serve`.

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
- Default `tui` configuration.
- Explicit host and ports.
- Log level parsing.
- Rich output enable/disable behavior.
- `serve --rich-log` explicit Rich logging behavior.
- `tui --transparent` configuration override.
- `tui --no-autostart` configuration override.
- Textual TUI service start/stop behavior.
- `serve` does not render live measurement charts.
- Textual TUI smoke startup and keyboard exit behavior.
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
