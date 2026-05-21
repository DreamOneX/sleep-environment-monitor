# Server

This directory is reserved for the measurement ingestion server. Keep server implementation files here rather than in the repository root.

Current contents:

- `post_receiver.py`: stdlib-only Phase 22 local receiver for REST upload, time, and discovery validation.

The Phase 23 formal server should replace or supersede this local validation receiver.

Planned formal command shape:

```bash
uv run sleep-env-server serve --host 0.0.0.0 --port 8080 --udp-discovery-port 39022
uv run sleep-env-server check-config
uv run sleep-env-server print-discovery
```

See:

- [../docs/20-server/00-overview.md](../docs/20-server/00-overview.md)
- [../docs/20-server/02-toolchain.md](../docs/20-server/02-toolchain.md)
- [../docs/20-server/03-cli.md](../docs/20-server/03-cli.md)

## Phase 22 Receiver

```bash
python3 server/post_receiver.py
```

It listens for HTTP on `0.0.0.0:8080` and UDP discovery on `0.0.0.0:39022`.

HTTP behavior:

- `POST /api/v1/measurements`: accepts a JSON request body and returns `204`.
- Other `POST` paths: returns `404`.
- `GET /api/v1/time`: returns `{"unix_ms": <current epoch millis>, "source": "server"}`.
- `GET /.well-known/sleep-environment-monitor`: returns discovery metadata with `api_base`, `measurement_upload`, `time`, and `udp_discovery_port`.

UDP discovery:

- Port: `39022`.
- Query payload: `sleep-environment-monitor.discovery`.
- Response: compact JSON containing `host`, `port`, `api_base`, `measurement_upload`, and `time`.
