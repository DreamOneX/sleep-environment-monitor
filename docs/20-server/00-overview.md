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
- Returns HTTP 2xx only after the upload passes validation and is accepted by
  the in-process sink.
- Treats duplicate `(device_id, sequence)` uploads as idempotent success.
- Provides a server time endpoint so firmware can obtain real-world time when
  NTP is unavailable.
- Publishes a discovery document for automatic endpoint discovery.
- Responds to the UDP discovery query used by Phase 22 firmware.
- Provides an `argparse` CLI for serving, configuration checks, and discovery
  metadata inspection.
- Uses FastAPI, Uvicorn, Pydantic, and Rich instead of raw `http.server`
  request handling.
- Keeps Ruff formatter and linter output advisory and check-only.

## Boundaries

Phase 23 intentionally uses process-local duplicate tracking only. Durable
storage, deployment service management, authentication, authorization, and
long-term retention policy remain future work.

## Related Docs

- [01-rest-api.md](01-rest-api.md): firmware/server REST API contract.
- [02-toolchain.md](02-toolchain.md): Python toolchain, style policy,
  formatter/linter policy, and unit-test expectations.
- [03-cli.md](03-cli.md): `argparse` command surface.
- [../10-firmware/03-network.md](../10-firmware/03-network.md): firmware network responsibilities.
- [../30-integration/00-network-roadmap.md](../30-integration/00-network-roadmap.md): cross-component roadmap.
