# Server Toolchain

This document defines the Python toolchain, code style, and test standards for
the formal ingestion server.

## Toolchain

Implemented defaults:

- Python package metadata in [../../server/pyproject.toml](../../server/pyproject.toml).
- Reproducible dependency resolution in [../../server/uv.lock](../../server/uv.lock).
- Dependency and command execution through `uv`.
- Web framework: FastAPI.
- ASGI server: Uvicorn.
- Data validation: Pydantic models.
- Human-readable console output: Rich.
- CLI implementation: Python stdlib `argparse`.
- Configuration parser: Python stdlib `tomllib`.
- Durable local storage: Python stdlib `sqlite3` and JSONL files.
- Tests: pytest.
- Formatter and linter: Ruff, used as check-only guidance.

Primary commands:

```bash
uv run sleep-env-server serve --host 0.0.0.0 --port 8080 --udp-discovery-port 39022
uv run sleep-env-server check-config
uv run sleep-env-server print-discovery
uv run sleep-env-server history
```

## Package Layout

Implemented structure:

```text
server/
├── pyproject.toml
├── uv.lock
├── README.md
├── config.example.toml
├── post_receiver.py
├── src/
│   └── sleep_env_server/
│       ├── __init__.py
│       ├── __main__.py
│       ├── app.py
│       ├── cli.py
│       ├── config.py
│       ├── discovery.py
│       ├── models.py
│       ├── output.py
│       └── storage.py
└── tests/
    ├── test_api.py
    ├── test_cli.py
    ├── test_config.py
    ├── test_discovery.py
    ├── test_models.py
    └── test_output.py
```

`server/post_receiver.py` is a compatibility wrapper. It dispatches to
`sleep-env-server serve` when no subcommand is provided.

## Check Commands

Run from the `server/` directory:

```bash
uv run pytest
uv run ruff check --diff .
uv run ruff format --check .
```

If type checking is added later:

```bash
uv run mypy src tests
```

## Formatter And Linter Policy

Formatter and linter output is advisory only.

Rules:

- Never automatically apply formatter or linter rewrites across server code.
- Do not run auto-fix or auto-format commands as an implementation step or
  commit-preparation shortcut.
- Use formatter and linter commands as check-only gates.
- Review each suggestion manually before editing code.
- Treat tool output as one source of engineering feedback, not as authority.
- Prefer local edits that preserve protocol readability and maintenance intent.
- Keep suppressions narrow and explicit.

Ruff may be used for diagnostics, but `ruff format` and `ruff check --fix`
must not be run as automatic rewrite steps for normal development.

Formatter/linter suppression is allowed when it protects intentional
readability. Examples include:

- Manually aligned protocol tables.
- Dense field maps where vertical alignment helps review.
- JSON examples or payload fixtures that are easier to compare when aligned.
- Small compatibility shims where a linter suggestion would obscure intent.

Suppression requirements:

- Use tool-supported local markers such as formatter-disable regions or
  line-level linter ignores.
- Scope the suppression to the smallest practical block, line, or file.
- Include a short reason when the reason is not obvious.
- Do not use broad file-level suppressions for convenience.
- Revisit suppressions when the surrounding code changes.

## Comment And Docstring Style

Comments and docstrings use Google style.

Guidelines:

- Public modules, public classes, public functions, and non-obvious helpers
  should have Google-style docstrings.
- Keep comments focused on why the code exists, protocol constraints, or
  non-obvious operational behavior.
- Avoid comments that restate obvious code.
- Use type hints for public server functions and data models.

Example:

```python
def build_discovery_payload(config: ServerConfig, peer_host: str) -> DiscoveryPayload:
    """Builds the UDP discovery response for one peer.

    Args:
        config: Active server configuration.
        peer_host: IPv4 address of the peer that sent the discovery query.

    Returns:
        Discovery payload containing the HTTP endpoint and API paths.
    """
```

## Unit Test Coverage

All server unit tests must be automated and hardware-free.

Required coverage:

- CLI argument parsing:
  - Default host, HTTP port, and UDP discovery port.
  - Explicit host, HTTP port, and UDP discovery port.
  - Log level selection.
  - Rich output enable/disable switch.
  - Invalid port and invalid log-level rejection.
- Application configuration:
  - Defaults match the firmware fallback environment.
  - CLI overrides are applied deterministically.
  - Discovery metadata derives from the active configuration.
- REST API behavior:
  - `POST /api/v1/measurements` accepts valid schema-version-1 JSON.
  - Invalid JSON returns non-2xx.
  - Missing required measurement fields return non-2xx.
  - Other `POST` paths return `404`.
  - `GET /api/v1/time` returns integer `unix_ms` and source metadata.
  - `GET /.well-known/sleep-environment-monitor` returns paths and UDP port.
- Measurement model validation:
  - Required identity and sequence fields.
  - `time_status` allowed values.
  - Optional `wall_clock_unix_ms`.
  - Nullable sensor values.
  - Duplicate `(device_id, sequence)` idempotency.
- UDP discovery logic:
  - Correct query payload is accepted.
  - Wrong query payload is ignored.
  - Response contains `host`, `port`, `api_base`, `measurement_upload`, and
    `time`.
  - Response host selection is deterministic in tests.
- Logging/output:
  - Rich output can be enabled for human-readable local operation.
  - Plain or machine-readable output path remains testable.
  - Upload acceptance logs include source and payload size or equivalent
    diagnostic metadata without dumping unbounded payloads.

## Unit Test Quality

Test quality requirements:

- Assert externally visible behavior rather than incidental implementation
  details unless testing a pure helper.
- Use deterministic time sources where time values are asserted.
- Avoid real network dependencies; use framework test clients, fake sockets, or
  isolated loopback fixtures.
- Avoid sleeps except when explicitly validating timeout behavior.
- Prefer fake clocks or short controlled fixtures for timeout behavior.
- Do not depend on test execution order.
- Name tests after the behavior being validated.
- Keep fixtures small and local. Shared fixtures are acceptable only when they
  reduce duplication without hiding important setup.
- Add regression tests for every hardware or integration bug that can be
  reproduced without hardware.

## Manual Validation

Manual server validation should remain short and repeatable:

```bash
uv run sleep-env-server serve --host 0.0.0.0 --port 8080 --udp-discovery-port 39022
```

Then verify:

- `GET /api/v1/time`.
- `GET /.well-known/sleep-environment-monitor`.
- UDP query `sleep-environment-monitor.discovery` on port `39022`.
- ESP32-C3 discovery, time sync, JSON upload, and HTTP-2xx-only ACK behavior.
