from __future__ import annotations

import json
from argparse import Namespace
from io import StringIO

import pytest

from sleep_env_server.cli import (
    config_from_args,
    main,
    parse_args,
    run_check_config,
    run_print_discovery,
    select_serve_output_mode,
)


def test_serve_defaults_match_firmware_fallback() -> None:
    args = parse_args(["serve"])
    config = config_from_args(args)

    assert config.host == "0.0.0.0"
    assert config.port == 8080
    assert config.udp_discovery_port == 39022
    assert config.log_level == "info"
    assert args.log_level is None


def test_serve_explicit_host_ports_and_log_level_are_applied() -> None:
    args = parse_args(
        [
            "serve",
            "--host",
            "127.0.0.1",
            "--port",
            "8081",
            "--udp-discovery-port",
            "39023",
            "--log-level",
            "debug",
        ]
    )
    config = config_from_args(args)

    assert config.host == "127.0.0.1"
    assert config.port == 8081
    assert config.udp_discovery_port == 39023
    assert args.log_level == "debug"


def test_serve_output_modes_are_selected_from_flags_and_tty() -> None:
    assert (
        select_serve_output_mode(Namespace(json_log=True, no_rich=False), stdout_isatty=True)
        == "json"
    )
    assert (
        select_serve_output_mode(Namespace(json_log=False, no_rich=True), stdout_isatty=True)
        == "plain"
    )
    assert (
        select_serve_output_mode(Namespace(json_log=False, no_rich=False), stdout_isatty=True)
        == "rich"
    )
    assert (
        select_serve_output_mode(Namespace(json_log=False, no_rich=False), stdout_isatty=False)
        == "plain"
    )
    assert (
        select_serve_output_mode(
            Namespace(json_log=False, no_rich=False),
            stdout_isatty=False,
            configured_mode="rich",
        )
        == "rich"
    )


def test_json_log_and_no_rich_are_mutually_exclusive() -> None:
    with pytest.raises(SystemExit):
        parse_args(["serve", "--json-log", "--no-rich"])


def test_invalid_port_is_rejected() -> None:
    with pytest.raises(SystemExit):
        parse_args(["serve", "--port", "0"])


def test_invalid_log_level_is_rejected() -> None:
    with pytest.raises(SystemExit):
        parse_args(["serve", "--log-level", "trace"])


def test_check_config_prints_valid_config(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: object,
) -> None:
    stream = StringIO()
    monkeypatch.setenv("XDG_CONFIG_HOME", str(tmp_path))
    args = parse_args(["check-config", "--host", "127.0.0.1", "--port", "8081"])

    result = run_check_config(args, stream=stream)

    assert result == 0
    assert "config_ok host=127.0.0.1 port=8081" in stream.getvalue()


def test_check_config_rejects_empty_host(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: object,
) -> None:
    monkeypatch.setenv("XDG_CONFIG_HOME", str(tmp_path))
    args = parse_args(["check-config", "--host", ""])

    with pytest.raises(ValueError, match="host"):
        run_check_config(args, stream=StringIO())


def test_main_reports_check_config_failure_without_traceback(
    capsys: pytest.CaptureFixture[str],
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: object,
) -> None:
    monkeypatch.setenv("XDG_CONFIG_HOME", str(tmp_path))
    result = main(["check-config", "--host", ""])

    captured = capsys.readouterr()
    assert result == 2
    assert "error: host must not be empty" in captured.err
    assert "Traceback" not in captured.err


def test_print_discovery_plain_output(monkeypatch: pytest.MonkeyPatch, tmp_path: object) -> None:
    stream = StringIO()
    monkeypatch.setenv("XDG_CONFIG_HOME", str(tmp_path))
    args = parse_args(["print-discovery", "--output", "plain", "--port", "8081"])

    result = run_print_discovery(args, stream=stream, host_resolver=lambda _peer: "10.0.0.5")

    assert result == 0
    output = stream.getvalue()
    assert "document.measurement_upload=/api/v1/measurements" in output
    assert "udp_response.host=10.0.0.5" in output
    assert "udp_response.port=8081" in output


def test_print_discovery_json_output(monkeypatch: pytest.MonkeyPatch, tmp_path: object) -> None:
    stream = StringIO()
    monkeypatch.setenv("XDG_CONFIG_HOME", str(tmp_path))
    args = parse_args(["print-discovery", "--output", "json"])

    result = run_print_discovery(args, stream=stream, host_resolver=lambda _peer: "10.0.0.5")

    assert result == 0
    payload = json.loads(stream.getvalue())
    assert payload["event"] == "discovery_snapshot"
    assert payload["document"]["udp_discovery_port"] == 39022
    assert payload["udp_response"]["host"] == "10.0.0.5"


def test_print_discovery_rich_output_is_callable(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: object,
) -> None:
    stream = StringIO()
    monkeypatch.setenv("XDG_CONFIG_HOME", str(tmp_path))
    args = parse_args(["print-discovery", "--output", "rich"])

    result = run_print_discovery(args, stream=stream, host_resolver=lambda _peer: "10.0.0.5")

    assert result == 0
    assert "Discovery" in stream.getvalue()
