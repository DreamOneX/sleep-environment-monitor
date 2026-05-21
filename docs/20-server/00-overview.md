# Server Overview

The `server/` directory is reserved for the measurement ingestion server.

The current implementation is still the Phase 22 validation receiver:

- [../../server/post_receiver.py](../../server/post_receiver.py) listens on
  `0.0.0.0:8080`.
- It accepts `POST /api/v1/measurements` JSON payloads and prints them for local
  validation.
- It serves `GET /api/v1/time`.
- It serves `GET /.well-known/sleep-environment-monitor`.
- It responds to UDP discovery on port `39022`.
- It is not the final ingestion service or storage layer.

Phase 23 should replace or supersede that temporary receiver with a formal
Python server foundation.

## Intended Role

The formal server should:

- Accept RESTful measurement uploads from one or more devices.
- Return HTTP 2xx only after the upload is accepted for processing.
- Provide a server time endpoint so firmware can obtain real-world time when NTP is unavailable.
- Publish a discovery document for automatic endpoint discovery.
- Respond to the UDP discovery query used by Phase 22 firmware.
- Handle duplicate uploads safely because firmware may retry after transport failures.
- Provide an `argparse` CLI for serving, configuration checks, and discovery
  metadata inspection.
- Use a formal web framework instead of raw `http.server` request handling.
- Use Rich for human-readable local console output.
- Keep formatter and linter output advisory and check-only.

## Phase 23 Direction

Planned defaults:

- FastAPI for HTTP routing.
- Uvicorn for ASGI serving.
- Pydantic for request and response models.
- Rich for local operator output.
- `argparse` for the CLI.
- pytest for automated hardware-free tests.
- Ruff as a check-only formatter/linter reference.

Storage, deployment, authentication, and long-term retention policy remain
future decisions unless Phase 23 implementation explicitly scopes a minimal
local persistence layer.

## Related Docs

- [01-rest-api.md](01-rest-api.md): Phase 22 REST API contract.
- [02-toolchain.md](02-toolchain.md): planned Python toolchain, style policy,
  formatter/linter policy, and unit-test expectations.
- [03-cli.md](03-cli.md): planned `argparse` command surface.
- [../10-firmware/03-network.md](../10-firmware/03-network.md): firmware network responsibilities.
- [../30-integration/00-network-roadmap.md](../30-integration/00-network-roadmap.md): cross-component roadmap.
