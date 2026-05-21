# Server Overview

The `server/` directory is reserved for the measurement ingestion server.

The current implementation is intentionally temporary:

- [../../server/post_receiver.py](../../server/post_receiver.py) listens on `0.0.0.0:8080`.
- It accepts `POST /api/v1/measurements` JSON payloads and prints them for local validation.
- It serves `GET /api/v1/time`.
- It serves `GET /.well-known/sleep-environment-monitor`.
- It responds to UDP discovery on port `39022`.
- It is not the final ingestion service or storage layer.

## Intended Role

The formal server should:

- Accept RESTful measurement uploads from one or more devices.
- Return HTTP 2xx only after the upload is accepted for processing.
- Provide a server time endpoint so firmware can obtain real-world time when NTP is unavailable.
- Publish a discovery document for automatic endpoint discovery.
- Respond to the UDP discovery query used by Phase 22 firmware.
- Handle duplicate uploads safely because firmware may retry after transport failures.

Server toolchain, storage, deployment, authentication, and code style are intentionally not defined yet.

## Related Docs

- [01-rest-api.md](01-rest-api.md): Phase 22 REST API contract.
- [../10-firmware/03-network.md](../10-firmware/03-network.md): firmware network responsibilities.
- [../30-integration/00-network-roadmap.md](../30-integration/00-network-roadmap.md): cross-component roadmap.
