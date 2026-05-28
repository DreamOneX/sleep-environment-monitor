from __future__ import annotations

from pathlib import Path

import pytest

from sleep_env_server.config import (
    AppConfig,
    ServerConfig,
    TuiConfig,
    app_config_from_mapping,
    default_config_path,
    load_app_config,
    resolve_config_path,
)


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


def test_xdg_default_config_path_uses_environment(tmp_path: Path) -> None:
    path = default_config_path(environ={"XDG_CONFIG_HOME": str(tmp_path)})

    assert path == tmp_path / "sleep-env-server" / "config.toml"


def test_xdg_default_config_path_falls_back_to_home(tmp_path: Path) -> None:
    path = default_config_path(environ={}, home=tmp_path)

    assert path == tmp_path / ".config" / "sleep-env-server" / "config.toml"


def test_load_app_config_generates_default_config(tmp_path: Path) -> None:
    config = load_app_config(environ={"XDG_CONFIG_HOME": str(tmp_path)})

    assert config.generated_config is True
    assert config.config_path == tmp_path / "sleep-env-server" / "config.toml"
    assert config.config_path.exists()
    assert config.storage.sqlite.enabled is True
    assert config.storage.jsonl.enabled is False


def test_explicit_missing_config_is_rejected(tmp_path: Path) -> None:
    with pytest.raises(ValueError, match="config file does not exist"):
        resolve_config_path(str(tmp_path / "missing.toml"))


def test_explicit_config_file_is_loaded(tmp_path: Path) -> None:
    config_path = tmp_path / "server.toml"
    config_path.write_text(
        """
        [server]
        host = "127.0.0.1"
        port = 8082
        udp_discovery_port = 39024
        api_base = "/custom"
        log_level = "debug"

        [output]
        mode = "plain"
        dashboard = false

        [storage]
        enabled = true
        required_for_ack = false

        [storage.jsonl]
        enabled = true
        path = "./measurements.jsonl"

        [history_cli]
        tail_count = 5
        metrics = ["temperature_c"]
        """,
        encoding="utf-8",
    )

    config = load_app_config(str(config_path))

    assert config.generated_config is False
    assert config.server.host == "127.0.0.1"
    assert config.server.port == 8082
    assert config.server.udp_discovery_port == 39024
    assert config.server.api_base == "/custom"
    assert config.server.log_level == "debug"
    assert config.output.mode == "plain"
    assert config.output.dashboard is False
    assert config.tui.theme == "catppuccin-mocha"
    assert config.tui.transparent is False
    assert config.tui.autostart is True
    assert config.tui.measurements_limit == 200
    assert config.storage.required_for_ack is False
    assert config.storage.jsonl.enabled is True
    assert config.history_cli.tail_count == 5
    assert config.history_cli.metrics == ("temperature_c",)


def test_storage_policy_retention_limits_are_parsed(tmp_path: Path) -> None:
    config_path = tmp_path / "server.toml"
    config_path.write_text(
        """
        [storage.policy]
        default_profile = "default"

        [storage.policy.profile.default]
        limit = { time_limit = "2h", size_limit = "4MB" }

        [storage.sqlite]
        enabled = true
        path = "./measurements.db"
        policy = "default"
        """,
        encoding="utf-8",
    )

    config = load_app_config(str(config_path))

    profile = config.storage.policy.profiles["default"]
    assert profile.limit.time_limit == "2h"
    assert profile.limit.size_limit == "4MB"


def test_tui_config_is_parsed() -> None:
    config = app_config_from_mapping(
        {
            "tui": {
                "theme": "catppuccin-mocha",
                "transparent": True,
                "autostart": False,
                "measurements_limit": 500,
            }
        }
    )

    assert config.tui.theme == "catppuccin-mocha"
    assert config.tui.transparent is True
    assert config.tui.autostart is False
    assert config.tui.measurements_limit == 500


def test_tui_rejects_non_positive_measurement_limit() -> None:
    with pytest.raises(ValueError, match="measurements_limit"):
        app_config_from_mapping({"tui": {"measurements_limit": 0}})


def test_tui_accepts_graphite_compatibility_theme() -> None:
    assert TuiConfig(theme="graphite").theme == "graphite"


def test_tui_rejects_unknown_theme() -> None:
    with pytest.raises(ValueError, match="tui.theme"):
        app_config_from_mapping({"tui": {"theme": "purple-rain"}})


def test_history_api_requires_token_when_enabled() -> None:
    with pytest.raises(ValueError, match="bearer_token"):
        app_config_from_mapping({"history_api": {"enabled": True}})


def test_storage_policy_profile_loop_is_rejected() -> None:
    with pytest.raises(ValueError, match="inheritance loop"):
        app_config_from_mapping(
            {
                "storage": {
                    "policy": {
                        "default_profile": "a",
                        "profile": {
                            "a": {"parent": "b"},
                            "b": {"parent": "a"},
                        },
                    }
                }
            }
        )


def test_app_config_defaults_are_valid() -> None:
    config = AppConfig()

    assert config.storage.sqlite.enabled is True
    assert config.storage.sqlite.ack.required_for_ack is True
    assert config.storage.sqlite.ack.sufficient_for_ack is True
    assert config.history_api.enabled is False
