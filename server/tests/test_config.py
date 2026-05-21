from __future__ import annotations

import pytest

from sleep_env_server.config import ServerConfig


def test_defaults_match_firmware_fallback_environment() -> None:
    config = ServerConfig()

    assert config.host == "0.0.0.0"
    assert config.port == 8080
    assert config.udp_discovery_port == 39022
    assert config.api_base == "/api/v1"
    assert config.measurement_upload_path == "/api/v1/measurements"
    assert config.time_path == "/api/v1/time"


def test_discovery_metadata_derives_from_active_configuration() -> None:
    config = ServerConfig(port=8081, udp_discovery_port=39023)

    assert config.discovery_document().model_dump() == {
        "api_base": "/api/v1",
        "measurement_upload": "/api/v1/measurements",
        "time": "/api/v1/time",
        "udp_discovery_port": 39023,
    }


def test_invalid_ports_are_rejected() -> None:
    with pytest.raises(ValueError, match="port"):
        ServerConfig(port=0)
    with pytest.raises(ValueError, match="udp_discovery_port"):
        ServerConfig(udp_discovery_port=65536)


def test_invalid_api_base_is_rejected() -> None:
    with pytest.raises(ValueError, match="api_base"):
        ServerConfig(api_base="api/v1")
    with pytest.raises(ValueError, match="api_base"):
        ServerConfig(api_base="/api/v1/")
