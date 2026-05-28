from __future__ import annotations

import json
from dataclasses import replace
from io import StringIO
from pathlib import Path

import pytest
from fastapi.testclient import TestClient

from sleep_env_server.app import create_app
from sleep_env_server.config import (
    AckPolicyConfig,
    HistoryApiConfig,
    ServerConfig,
    StorageConfig,
    StorageTargetConfig,
)
from sleep_env_server.output import ServerOutput
from sleep_env_server.storage import ConfiguredMeasurementSink, InMemoryMeasurementSink


class RecordingOutput(ServerOutput):
    def __init__(self) -> None:
        super().__init__("plain", stream=StringIO())
        self.dashboard_events: list[dict[str, object]] = []

    def measurement_dashboard(
        self,
        *,
        received_unix_ms: int | None = None,
        device_id: str,
        sequence: int,
        temperature_c: float | None,
        humidity_percent: float | None,
        lux: float | None,
        mic_db_rel: float,
        duplicate: bool,
    ) -> None:
        self.dashboard_events.append(
            {
                "received_unix_ms": received_unix_ms,
                "device_id": device_id,
                "sequence": sequence,
                "temperature_c": temperature_c,
                "humidity_percent": humidity_percent,
                "lux": lux,
                "mic_db_rel": mic_db_rel,
                "duplicate": duplicate,
            }
        )


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


def sqlite_storage_config(tmp_path: Path) -> StorageConfig:
    return replace(
        StorageConfig(),
        sqlite=StorageTargetConfig(
            enabled=True,
            path=str(tmp_path / "measurements.db"),
            policy="no_limit",
            ack=AckPolicyConfig(required_for_ack=True, sufficient_for_ack=True),
        ),
        jsonl=StorageTargetConfig(enabled=False),
    )


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


def test_measurement_upload_persists_to_configured_storage(tmp_path: Path) -> None:
    stream = StringIO()
    output = ServerOutput("json", stream=stream)
    storage = sqlite_storage_config(tmp_path)
    sink = ConfiguredMeasurementSink(storage)
    app = create_app(sink=sink, output=output, clock=lambda: 1_900_000_000_007)
    client = TestClient(app)

    response = client.post("/api/v1/measurements", json=valid_measurement_payload())

    assert response.status_code == 204
    assert sink.stores[0].store.list_records()[0].upload.sequence == 7


def test_measurement_dashboard_receives_server_receive_time() -> None:
    output = RecordingOutput()
    app = create_app(
        sink=InMemoryMeasurementSink(),
        output=output,
        clock=lambda: 1_700_000_000_000,
    )
    client = TestClient(app)

    response = client.post("/api/v1/measurements", json=valid_measurement_payload())

    assert response.status_code == 204
    assert output.dashboard_events[0]["received_unix_ms"] == 1_700_000_000_000


def test_measurement_upload_returns_non_2xx_when_storage_ack_rejects(
    tmp_path: Path,
) -> None:
    stream = StringIO()
    output = ServerOutput("json", stream=stream)
    storage = sqlite_storage_config(tmp_path)
    sink = ConfiguredMeasurementSink(storage)
    app = create_app(sink=sink, output=output, clock=lambda: 1_900_000_000_007)
    client = TestClient(app)
    first = valid_measurement_payload()
    second = valid_measurement_payload()
    second["temperature_c"] = 30.0
    sink.stores[0].store.dedup_strategy = "reject"

    accepted = client.post("/api/v1/measurements", json=first)
    rejected = client.post("/api/v1/measurements", json=second)

    assert accepted.status_code == 204
    assert rejected.status_code == 409
    events = [json.loads(line) for line in stream.getvalue().splitlines()]
    assert events[-1]["event"] == "upload_rejected"
    assert events[-1]["conflict"] is True
    assert events[-1]["status_code"] == 409


def test_history_api_is_not_registered_by_default() -> None:
    client = TestClient(create_app())

    response = client.get("/api/v1/history/measurements")

    assert response.status_code == 404


def test_history_api_requires_bearer_token(tmp_path: Path) -> None:
    sink = ConfiguredMeasurementSink(sqlite_storage_config(tmp_path))
    app = create_app(
        sink=sink,
        history_api=HistoryApiConfig(enabled=True, bearer_token="secret"),
    )
    client = TestClient(app)

    missing = client.get("/api/v1/history/measurements")
    wrong = client.get(
        "/api/v1/history/measurements",
        headers={"Authorization": "Bearer wrong"},
    )

    assert missing.status_code == 401
    assert missing.headers["www-authenticate"] == "Bearer"
    assert wrong.status_code == 401


def test_history_api_lists_paginated_measurements(tmp_path: Path) -> None:
    sink = ConfiguredMeasurementSink(sqlite_storage_config(tmp_path))
    app = create_app(
        sink=sink,
        clock=lambda: 1_900_000_000_007,
        history_api=HistoryApiConfig(
            enabled=True,
            bearer_token="secret",
            read_source="sqlite",
        ),
    )
    client = TestClient(app)
    first = valid_measurement_payload()
    second = valid_measurement_payload()
    second["sequence"] = 8

    assert client.post("/api/v1/measurements", json=first).status_code == 204
    assert client.post("/api/v1/measurements", json=second).status_code == 204
    response = client.get(
        "/api/v1/history/measurements?limit=1&offset=1",
        headers={"Authorization": "Bearer secret"},
    )

    assert response.status_code == 200
    body = response.json()
    assert body["limit"] == 1
    assert body["offset"] == 1
    assert len(body["records"]) == 1
    assert body["records"][0]["payload"]["sequence"] == 8


def test_history_api_returns_summary(tmp_path: Path) -> None:
    sink = ConfiguredMeasurementSink(sqlite_storage_config(tmp_path))
    app = create_app(
        sink=sink,
        clock=lambda: 1_900_000_000_007,
        history_api=HistoryApiConfig(
            enabled=True,
            bearer_token="secret",
            read_source="sqlite",
        ),
    )
    client = TestClient(app)
    first = valid_measurement_payload()
    second = valid_measurement_payload()
    second["sequence"] = 8
    second["temperature_c"] = 23.5

    assert client.post("/api/v1/measurements", json=first).status_code == 204
    assert client.post("/api/v1/measurements", json=second).status_code == 204
    response = client.get(
        "/api/v1/history/summary",
        headers={"Authorization": "Bearer secret"},
    )

    assert response.status_code == 200
    body = response.json()
    assert body["count"] == 2
    assert body["devices"] == ["sleep-env-esp32c3"]
    assert body["averages"]["temperature_c"] == pytest.approx(22.5)


def test_history_api_validates_query_and_missing_source(tmp_path: Path) -> None:
    sink = ConfiguredMeasurementSink(
        replace(
            StorageConfig(),
            sqlite=StorageTargetConfig(enabled=False),
            jsonl=StorageTargetConfig(enabled=False),
        )
    )
    app = create_app(
        sink=sink,
        history_api=HistoryApiConfig(
            enabled=True,
            bearer_token="secret",
            read_source="sqlite",
        ),
    )
    client = TestClient(app)
    headers = {"Authorization": "Bearer secret"}

    invalid = client.get("/api/v1/history/measurements?limit=0", headers=headers)
    missing = client.get("/api/v1/history/measurements", headers=headers)

    assert invalid.status_code == 422
    assert missing.status_code == 503


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
