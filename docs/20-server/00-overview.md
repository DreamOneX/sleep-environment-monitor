# Server Overview

The `server/` directory is reserved for the measurement ingestion server.

The current implementation is intentionally temporary:

- [../../server/post_receiver.py](../../server/post_receiver.py) listens on `0.0.0.0:8080`.
- It accepts firmware POST requests and prints payloads for local validation.
- It is not the final ingestion service, storage layer, or API implementation.

## Intended Role

The formal server should:

- Accept RESTful measurement uploads from one or more devices.
- Return HTTP 2xx only after the upload is accepted for processing.
- Provide a health endpoint for firmware and manual checks.
- Provide a server time endpoint so firmware can obtain real-world time when NTP is unavailable.
- Publish a discovery document for automatic endpoint discovery.
- Handle duplicate uploads safely because firmware may retry after transport failures.

Server toolchain, storage, deployment, authentication, and code style are intentionally not defined yet.

## Related Docs

- [01-rest-api.md](01-rest-api.md): planned REST API contract.
- [../10-firmware/03-network.md](../10-firmware/03-network.md): firmware network responsibilities.
- [../30-integration/00-network-roadmap.md](../30-integration/00-network-roadmap.md): cross-component roadmap.
