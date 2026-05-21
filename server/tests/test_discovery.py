from __future__ import annotations

import json

from sleep_env_server.config import ServerConfig
from sleep_env_server.discovery import (
    DISCOVERY_QUERY,
    build_udp_discovery_payload,
    build_udp_discovery_response,
)


def test_udp_discovery_payload_contains_endpoint_paths_and_resolved_host() -> None:
    config = ServerConfig(port=8081)

    payload = build_udp_discovery_payload(
        config,
        "10.0.0.99",
        host_resolver=lambda peer: f"local-for-{peer}",
    )

    assert payload.model_dump() == {
        "host": "local-for-10.0.0.99",
        "port": 8081,
        "api_base": "/api/v1",
        "measurement_upload": "/api/v1/measurements",
        "time": "/api/v1/time",
    }


def test_valid_udp_discovery_query_generates_compact_json_response() -> None:
    response = build_udp_discovery_response(
        ServerConfig(),
        DISCOVERY_QUERY.encode("utf-8"),
        "10.0.0.99",
        host_resolver=lambda _peer: "10.0.0.5",
    )

    assert response is not None
    assert b" " not in response
    assert json.loads(response) == {
        "host": "10.0.0.5",
        "port": 8080,
        "api_base": "/api/v1",
        "measurement_upload": "/api/v1/measurements",
        "time": "/api/v1/time",
    }


def test_wrong_udp_discovery_query_is_ignored() -> None:
    response = build_udp_discovery_response(
        ServerConfig(),
        b"wrong",
        "10.0.0.99",
        host_resolver=lambda _peer: "10.0.0.5",
    )

    assert response is None


def test_invalid_utf8_udp_discovery_query_is_ignored() -> None:
    response = build_udp_discovery_response(
        ServerConfig(),
        b"\xff",
        "10.0.0.99",
        host_resolver=lambda _peer: "10.0.0.5",
    )

    assert response is None
