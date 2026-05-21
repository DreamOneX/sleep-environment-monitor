# Network Roadmap

This roadmap coordinates firmware, server, discovery, time, and future provisioning work.

## Direction

- Keep REST as the primary upload protocol.
- Do not introduce MQTT in the current plan.
- Make automatic discovery available while preserving a static fallback endpoint.
- Make real-world time synchronization a required network capability.
- Shape configuration so BLE can later provision Wi-Fi and server settings.

## Phase 21

Configuration consolidation.

- Add `firmware/src/config.rs`.
- Move deployment and policy constants out of task-local hardcoding.
- Keep behavior unchanged.
- Preserve the then-current REST upload behavior and local receiver compatibility.
- Do not implement discovery, time sync, or BLE yet.

See [../10-firmware/04-configuration.md](../10-firmware/04-configuration.md).

## Phase 22

REST network redesign.

- Split Wi-Fi link, IP readiness, endpoint resolution, HTTP transport, and upload orchestration.
- Keep upload acknowledgement tied to HTTP 2xx.
- Add structured network and upload error reporting.
- Replace the old `/measurements` CSV upload with JSON schema version 1 at `POST /api/v1/measurements`.
- Add automatic REST server discovery with UDP discovery and static fallback.
- Add real-world time synchronization through SNTP/NTP and REST server time fallback.
- Extend the measurement payload so wall-clock time is included when known while preserving `uptime_ms`.
- Support open, WPA-Personal, WPA2-Personal, and WPA/WPA2-Personal mixed Wi-Fi configuration.
- Defer WPA3 and Enterprise/EAP Wi-Fi until validated separately.
- Keep BLE as a future provisioning path, with config shaped for it.

See [../10-firmware/03-network.md](../10-firmware/03-network.md) and [../20-server/01-rest-api.md](../20-server/01-rest-api.md).

## Phase 23

Formal server foundation.

- Replace or supersede the stdlib-only Phase 22 receiver with a packaged Python
  server.
- Preserve the Phase 22 REST API, time endpoint, discovery document, and UDP
  discovery behavior.
- Use a formal web framework, with FastAPI/Uvicorn/Pydantic as the planned
  default stack.
- Add an `argparse` CLI for serving, configuration checks, and discovery
  metadata inspection.
- Use Rich for human-readable local operation output.
- Define server check commands, code style, formatter/linter policy, and
  hardware-free unit-test expectations.
- Treat formatter and linter output as advisory check-only input; review
  suggestions manually before editing code.

See [../20-server/00-overview.md](../20-server/00-overview.md),
[../20-server/02-toolchain.md](../20-server/02-toolchain.md), and
[../20-server/03-cli.md](../20-server/03-cli.md).

## Discovery Precedence

Endpoint selection uses:

1. Provisioned endpoint from future BLE or persistent config.
2. Automatically discovered endpoint.
3. Static fallback endpoint from firmware config.

This order keeps development and recovery practical while allowing deployment without recompiling firmware.

## Time Strategy

The firmware should always preserve boot-relative `uptime_ms`.

Wall-clock time is added when available:

1. Synchronize through SNTP/NTP after IP configuration.
2. Fall back to `GET /api/v1/time` from the REST server.
3. Continue uploading uptime-only measurements if no wall-clock source is available.

Records collected before synchronization remain valid and uploadable.
