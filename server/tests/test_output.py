from __future__ import annotations

import json
from io import StringIO

from sleep_env_server.config import ServerConfig
from sleep_env_server.discovery import DISCOVERY_QUERY, build_udp_discovery_payload
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
