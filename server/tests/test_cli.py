from __future__ import annotations

import json
from argparse import Namespace
from io import StringIO
from pathlib import Path

import pytest

from sleep_env_server.cli import (
    build_metric_trends,
    config_from_args,
    main,
    parse_args,
    run_check_config,
    run_history,
    run_print_discovery,
    select_history_output_mode,
    select_serve_output_mode,
)
from sleep_env_server.models import MeasurementUpload
from sleep_env_server.storage import MeasurementRecord, SQLiteMeasurementStore


def history_payload(sequence: int = 1, *, temperature_c: float = 21.5) -> dict[str, object]:
    return {
        "schema_version": 1,
        "device_id": "sleep-env-esp32c3",
        "sequence": sequence,
        "time_status": "wall_clock_synced",
        "wall_clock_unix_ms": 1_700_000_000_000 + sequence,
        "uptime_ms": 1234 + sequence,
        "temperature_c": temperature_c,
        "humidity_percent": 45.25,
        "lux": 9.75,
        "mic_mean": 2048.0,
        "mic_rms": 10.5,
        "mic_peak": 99.0,
        "mic_db_rel": 20.4 + sequence,
        "mic_clip_count": 2,
        "error_flags": 0,
    }


def history_record(sequence: int = 1, *, temperature_c: float = 21.5) -> MeasurementRecord:
    return MeasurementRecord.from_upload(
        MeasurementUpload.model_validate(history_payload(sequence, temperature_c=temperature_c)),
        received_unix_ms=1_800_000_000_000 + sequence,
        source="test",
    )


def write_history_config(tmp_path: Path) -> tuple[Path, Path]:
    config_path = tmp_path / "server.toml"
    db_path = tmp_path / "measurements.db"
    config_path.write_text(
        f"""
        [output]
        mode = "plain"

        [storage.sqlite]
        enabled = true
        path = "{db_path}"
        policy = "no_limit"

        [storage.jsonl]
        enabled = false

        [history_cli]
        read_source = "sqlite"
        tail_count = 1
        metrics = ["temperature_c", "mic_db_rel"]
        """,
        encoding="utf-8",
    )
    return config_path, db_path


def test_serve_defaults_match_firmware_fallback() -> None:
    args = parse_args(["serve"])
    config = config_from_args(args)

    assert config.host == "0.0.0.0"
    assert config.port == 8080
    assert config.udp_discovery_port == 39022
    assert config.log_level == "info"
    assert args.log_level is None


def test_tui_defaults_match_firmware_fallback() -> None:
    args = parse_args(["tui"])
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


def test_history_output_modes_are_selected_from_flags_config_and_tty() -> None:
    assert select_history_output_mode(Namespace(output="json"), stdout_isatty=True) == "json"
    assert select_history_output_mode(Namespace(output="plain"), stdout_isatty=True) == "plain"
    assert select_history_output_mode(Namespace(output="rich"), stdout_isatty=False) == "rich"
    assert (
        select_history_output_mode(
            Namespace(output="auto"),
            stdout_isatty=False,
            configured_mode="rich",
        )
        == "rich"
    )
    assert select_history_output_mode(Namespace(output="auto"), stdout_isatty=False) == "plain"


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


def test_history_json_output_reads_configured_storage(tmp_path: Path) -> None:
    config_path, db_path = write_history_config(tmp_path)
    store = SQLiteMeasurementStore(db_path)
    store.write(history_record(1, temperature_c=21.5))
    store.write(history_record(2, temperature_c=23.5))
    stream = StringIO()
    args = parse_args(
        [
            "history",
            "--config",
            str(config_path),
            "--output",
            "json",
            "--limit",
            "1",
        ]
    )

    result = run_history(args, stream=stream, stdout_isatty=False)

    assert result == 0
    event = json.loads(stream.getvalue())
    assert event["event"] == "history_snapshot"
    assert event["summary"]["count"] == 2
    assert event["records"][0]["payload"]["sequence"] == 2
    assert "temperature_c" in event["trends"]


def test_history_rich_output_is_callable(tmp_path: Path) -> None:
    config_path, db_path = write_history_config(tmp_path)
    store = SQLiteMeasurementStore(db_path)
    store.write(history_record(1))
    stream = StringIO()
    args = parse_args(["history", "--config", str(config_path), "--output", "rich"])

    result = run_history(args, stream=stream, stdout_isatty=True)

    assert result == 0
    assert "History Summary" in stream.getvalue()


def test_build_metric_trends_uses_recent_numeric_values() -> None:
    records = [
        history_record(1, temperature_c=20.0),
        history_record(2, temperature_c=22.0),
        history_record(3, temperature_c=24.0),
    ]

    trends = build_metric_trends(records, ("temperature_c",), width=2)

    assert trends["temperature_c"].endswith("22.00..24.00")
