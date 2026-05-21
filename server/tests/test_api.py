from __future__ import annotations

import json
from io import StringIO

from fastapi.testclient import TestClient

from sleep_env_server.app import create_app
from sleep_env_server.config import ServerConfig
from sleep_env_server.output import ServerOutput
from sleep_env_server.storage import InMemoryMeasurementSink


def valid_measurement_payload() -> dict[str, object]:
    return {
        "schema_version": 1,
        "device_id": "sleep-env-esp32c3",
        "sequence": 7,
        "time_status": "uptime_only",
        "uptime_ms": 1234,
        "temperature_c": 21.5,
        "humidity_percent": 45.25,
        "lux": 9.75,
        "mic_mean": 2048.0,
        "mic_rms": 10.5,
        "mic_peak": 99.0,
        "mic_db_rel": 20.4,
        "mic_clip_count": 2,
        "error_flags": 17,
    }


def test_valid_measurement_upload_returns_204_and_logs_bounded_metadata() -> None:
    stream = StringIO()
    output = ServerOutput("json", stream=stream)
    app = create_app(sink=InMemoryMeasurementSink(), output=output)
    client = TestClient(app)

    response = client.post("/api/v1/measurements", json=valid_measurement_payload())

    assert response.status_code == 204
    logged = json.loads(stream.getvalue())
    assert logged["event"] == "upload_accepted"
    assert logged["device_id"] == "sleep-env-esp32c3"
    assert logged["sequence"] == 7
    assert logged["duplicate"] is False
    assert "temperature_c" not in logged


def test_duplicate_measurement_upload_returns_204_with_duplicate_log() -> None:
    stream = StringIO()
    output = ServerOutput("json", stream=stream)
    app = create_app(sink=InMemoryMeasurementSink(), output=output)
    client = TestClient(app)
    payload = valid_measurement_payload()

    first = client.post("/api/v1/measurements", json=payload)
    second = client.post("/api/v1/measurements", json=payload)

    assert first.status_code == 204
    assert second.status_code == 204
    events = [json.loads(line) for line in stream.getvalue().splitlines()]
    assert [event["duplicate"] for event in events] == [False, True]


def test_invalid_json_returns_non_2xx() -> None:
    client = TestClient(create_app())

    response = client.post(
        "/api/v1/measurements",
        content=b"{not-json",
        headers={"content-type": "application/json"},
    )

    assert response.status_code >= 400


def test_missing_required_measurement_fields_returns_non_2xx() -> None:
    client = TestClient(create_app())
    payload = valid_measurement_payload()
    payload.pop("uptime_ms")

    response = client.post("/api/v1/measurements", json=payload)

    assert response.status_code >= 400


def test_invalid_time_status_returns_non_2xx() -> None:
    client = TestClient(create_app())
    payload = valid_measurement_payload()
    payload["time_status"] = "ntp_pending"

    response = client.post("/api/v1/measurements", json=payload)

    assert response.status_code >= 400


def test_unknown_post_path_returns_404() -> None:
    client = TestClient(create_app())

    response = client.post("/measurements", json=valid_measurement_payload())

    assert response.status_code == 404


def test_time_endpoint_uses_injected_clock() -> None:
    client = TestClient(create_app(clock=lambda: 1_700_000_000_123))

    response = client.get("/api/v1/time")

    assert response.status_code == 200
    assert response.json() == {"unix_ms": 1_700_000_000_123, "source": "server"}


def test_discovery_document_reflects_active_config() -> None:
    config = ServerConfig(host="127.0.0.1", port=8081, udp_discovery_port=39023)
    client = TestClient(create_app(config))

    response = client.get("/.well-known/sleep-environment-monitor")

    assert response.status_code == 200
    assert response.json() == {
        "api_base": "/api/v1",
        "measurement_upload": "/api/v1/measurements",
        "time": "/api/v1/time",
        "udp_discovery_port": 39023,
    }
