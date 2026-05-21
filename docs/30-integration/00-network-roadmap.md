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
- Keep REST upload behavior and temporary receiver compatibility.
- Do not implement discovery, time sync, or BLE yet.

See [../10-firmware/04-configuration.md](../10-firmware/04-configuration.md).

## Phase 22

REST network redesign.

- Split Wi-Fi link, IP readiness, endpoint resolution, HTTP transport, and upload orchestration.
- Keep upload acknowledgement tied to HTTP 2xx.
- Add structured network and upload error reporting.
- Add automatic REST server discovery with static fallback.
- Add real-world time synchronization through SNTP/NTP when practical and REST server time as fallback.
- Extend the measurement payload so wall-clock time is included when known while preserving `uptime_ms`.
- Keep BLE as a future provisioning path, with config shaped for it.

See [../10-firmware/03-network.md](../10-firmware/03-network.md) and [../20-server/01-rest-api.md](../20-server/01-rest-api.md).

## Discovery Precedence

Endpoint selection should use:

1. Provisioned endpoint from future BLE or persistent config.
2. Automatically discovered endpoint.
3. Static fallback endpoint from firmware config.

This order keeps development and recovery practical while allowing deployment without recompiling firmware.

## Time Strategy

The firmware should always preserve boot-relative `uptime_ms`.

Wall-clock time is added when available:

1. Synchronize through SNTP/NTP after IP configuration if supported within memory and dependency limits.
2. Fall back to `GET /api/v1/time` from the REST server.
3. Continue uploading uptime-only measurements if no wall-clock source is available.

Records collected before synchronization remain valid and uploadable.
