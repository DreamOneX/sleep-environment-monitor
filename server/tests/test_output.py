from __future__ import annotations

import json
import logging
from io import StringIO

from sleep_env_server.config import ServerConfig
from sleep_env_server.discovery import DISCOVERY_QUERY, build_udp_discovery_payload
from sleep_env_server.logging_config import configure_service_logging
from sleep_env_server.output import ServerOutput


def test_plain_upload_log_is_bounded() -> None:
    stream = StringIO()
    output = ServerOutput("plain", stream=stream)

    output.upload_accepted(
        source="10.0.0.2",
        byte_count=300,
        device_id="device-1",
        sequence=42,
        duplicate=False,
    )

    line = stream.getvalue()
    assert "upload_accepted" in line
    assert "source=10.0.0.2" in line
    assert "bytes=300" in line
    assert "device_id=device-1" in line
    assert "sequence=42" in line
    assert "temperature_c" not in line


def test_json_upload_log_is_jsonl_and_bounded() -> None:
    stream = StringIO()
    output = ServerOutput("json", stream=stream)

    output.upload_accepted(
        source="10.0.0.2",
        byte_count=300,
        device_id="device-1",
        sequence=42,
        duplicate=True,
    )

    event = json.loads(stream.getvalue())
    assert event == {
        "bytes": 300,
        "device_id": "device-1",
        "duplicate": True,
        "event": "upload_accepted",
        "sequence": 42,
        "source": "10.0.0.2",
    }


def test_rich_output_path_is_callable() -> None:
    stream = StringIO()
    output = ServerOutput("rich", stream=stream, force_terminal=False)

    output.startup(ServerConfig(), "info")

    assert "server_starting" in stream.getvalue()


def test_discovery_snapshot_json_contains_document_and_udp_payload() -> None:
    stream = StringIO()
    output = ServerOutput("json", stream=stream)
    config = ServerConfig()

    output.discovery_snapshot(
        document=config.discovery_document(),
        udp_query=DISCOVERY_QUERY,
        udp_response=build_udp_discovery_payload(
            config,
            "10.0.0.99",
            host_resolver=lambda _peer: "10.0.0.5",
        ),
    )

    event = json.loads(stream.getvalue())
    assert event["document"]["measurement_upload"] == "/api/v1/measurements"
    assert event["udp_response"]["host"] == "10.0.0.5"


def test_history_snapshot_json_contains_summary_records_and_trends() -> None:
    stream = StringIO()
    output = ServerOutput("json", stream=stream)

    output.history_snapshot(
        summary={
            "count": 1,
            "devices": ["device-1"],
            "first_received_unix_ms": 1000,
            "last_received_unix_ms": 1000,
            "averages": {"temperature_c": 21.5},
        },
        records=[
            {
                "received_unix_ms": 1000,
                "source": "test",
                "display_unix_ms": 900,
                "display_time_source": "device_reported",
                "payload": {
                    "device_id": "device-1",
                    "sequence": 1,
                    "temperature_c": 21.5,
                    "humidity_percent": 45.0,
                    "lux": 10.0,
                    "mic_db_rel": 20.0,
                },
            }
        ],
        trends={"temperature_c": ".@"},
    )

    event = json.loads(stream.getvalue())
    assert event["event"] == "history_snapshot"
    assert event["summary"]["count"] == 1
    assert event["records"][0]["payload"]["sequence"] == 1
    assert event["trends"]["temperature_c"] == ".@"


def test_history_snapshot_rich_output_is_callable() -> None:
    stream = StringIO()
    output = ServerOutput("rich", stream=stream, force_terminal=False)

    output.history_snapshot(
        summary={
            "count": 0,
            "devices": [],
            "first_received_unix_ms": None,
            "last_received_unix_ms": None,
            "averages": {},
        },
        records=[],
        trends={"temperature_c": ""},
    )

    text = stream.getvalue()
    assert "History Summary" in text
    assert "Recent Measurements" in text
    assert "Metric Trends" in text


def test_service_output_ignores_measurement_dashboard() -> None:
    stream = StringIO()
    output = ServerOutput("rich", stream=stream, force_terminal=False)

    output.measurement_dashboard(
        device_id="device-1",
        sequence=1,
        temperature_c=21.5,
        humidity_percent=45.0,
        lux=10.0,
        mic_db_rel=20.0,
        duplicate=False,
    )

    assert stream.getvalue() == ""


def test_service_logging_json_handler_writes_jsonl() -> None:
    stream = StringIO()
    configure_service_logging("json", stream=stream, log_level="info")

    logging.getLogger("uvicorn").info("server ready")

    event = json.loads(stream.getvalue())
    assert event == {
        "event": "log",
        "level": "info",
        "logger": "uvicorn",
        "message": "server ready",
    }


def test_service_logging_rich_handler_is_callable() -> None:
    stream = StringIO()
    configure_service_logging("rich", stream=stream, log_level="info", force_terminal=False)

    logging.getLogger("uvicorn").info("server ready")

    assert "server ready" in stream.getvalue()
