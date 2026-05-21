# Server CLI

The formal server CLI should use Python stdlib `argparse`.

Typer and Click are intentionally not the first choice for Phase 23. The CLI
surface is small, and using `argparse` keeps the dependency boundary focused on
the server framework and runtime.

## Command Shape

Planned console script:

```bash
sleep-env-server
```

Equivalent module invocation:

```bash
python -m sleep_env_server
```

Planned subcommands:

```bash
sleep-env-server serve
sleep-env-server check-config
sleep-env-server print-discovery
```

## `serve`

Runs the HTTP API and UDP discovery responder.

Planned options:

```text
--host HOST
--port PORT
--udp-discovery-port PORT
--log-level LEVEL
--json-log
--no-rich
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

- Start the FastAPI/Uvicorn HTTP server.
- Start the UDP discovery responder on the configured port.
- Print Rich startup information for local operation unless disabled.
- Expose the same API paths as the Phase 22 receiver.
- Shut down cleanly on interrupt.

## `check-config`

Validates CLI and environment-derived configuration without opening sockets.

Expected behavior:

- Validate host and port values.
- Validate logging options.
- Validate API path configuration.
- Exit with status `0` for valid config.
- Exit non-zero and print a clear error for invalid config.

## `print-discovery`

Prints the discovery document and UDP discovery payload that would be served by
the current configuration.

Expected behavior:

- Print `GET /.well-known/sleep-environment-monitor` metadata.
- Print UDP discovery response fields.
- Support Rich output for humans.
- Support plain or JSON output for tests and scripts.

## Argument Validation

Required validation:

- HTTP port must be in `1..=65535`.
- UDP discovery port must be in `1..=65535`.
- Log level must be one of the documented values.
- Mutually exclusive output modes must reject invalid combinations.

Invalid arguments should fail before opening HTTP or UDP sockets.

## Test Expectations

CLI tests should cover:

- Default `serve` configuration.
- Explicit host and ports.
- Log level parsing.
- Rich output enable/disable behavior.
- JSON/plain output switches for `print-discovery`.
- Invalid port rejection.
- Invalid log-level rejection.
- `check-config` success and failure paths.

Tests should call parser/config-building helpers directly where possible, and
only use subprocess tests for console-script integration behavior that cannot be
validated cleanly in process.
