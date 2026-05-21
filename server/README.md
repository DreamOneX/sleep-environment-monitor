# Server

This directory contains the formal measurement ingestion server.

The Phase 23 server is a Python package built with FastAPI, Uvicorn, Pydantic,
Rich, `argparse`, pytest, and Ruff. It preserves the Phase 22 firmware/server
contract while replacing the stdlib-only receiver implementation.

## Commands

Run commands from this directory:

```bash
uv run sleep-env-server serve --host 0.0.0.0 --port 8080 --udp-discovery-port 39022
uv run sleep-env-server check-config
uv run sleep-env-server print-discovery
```

Legacy hardware validation commands still work:

```bash
python3 server/post_receiver.py
```

`post_receiver.py` is now only a compatibility wrapper. With no subcommand it
dispatches to `sleep-env-server serve` using the default host, HTTP port, and
UDP discovery port.

## API

HTTP behavior:

- `POST /api/v1/measurements`: validates JSON schema version 1 and returns
  `204` after process-local acceptance.
- Duplicate `(device_id, sequence)` uploads return `204` as idempotent success.
- Invalid JSON or invalid schema returns FastAPI/Pydantic validation errors.
- Other `POST` paths return `404`.
- `GET /api/v1/time`: returns `{"unix_ms": <current epoch millis>, "source": "server"}`.
- `GET /.well-known/sleep-environment-monitor`: returns discovery metadata with
  `api_base`, `measurement_upload`, `time`, and `udp_discovery_port`.

UDP discovery:

- Port: `39022` by default.
- Query payload: `sleep-environment-monitor.discovery`.
- Response: compact JSON containing `host`, `port`, `api_base`,
  `measurement_upload`, and `time`.
- Other payloads are ignored silently.

## Checks

Run from this directory:

```bash
uv run pytest
uv run ruff check --diff .
uv run ruff format --check .
```

Ruff is check-only guidance. Do not use auto-fix or auto-format as a normal
implementation step.

## Package Layout

```text
server/
├── pyproject.toml
├── uv.lock
├── post_receiver.py
├── src/sleep_env_server/
│   ├── app.py
│   ├── cli.py
│   ├── config.py
│   ├── discovery.py
│   ├── models.py
│   ├── output.py
│   └── storage.py
└── tests/
```

See:

- [../docs/20-server/00-overview.md](../docs/20-server/00-overview.md)
- [../docs/20-server/01-rest-api.md](../docs/20-server/01-rest-api.md)
- [../docs/20-server/02-toolchain.md](../docs/20-server/02-toolchain.md)
- [../docs/20-server/03-cli.md](../docs/20-server/03-cli.md)
