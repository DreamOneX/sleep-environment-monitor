from __future__ import annotations

import pytest
from pydantic import ValidationError

from sleep_env_server.models import MeasurementUpload
from sleep_env_server.storage import InMemoryMeasurementSink


def valid_payload() -> dict[str, object]:
    return {
        "schema_version": 1,
        "device_id": "sleep-env-esp32c3",
        "sequence": 0,
        "time_status": "wall_clock_synced",
        "wall_clock_unix_ms": 1_700_000_000_000,
        "uptime_ms": 1234,
        "temperature_c": 21.5,
        "humidity_percent": 45.25,
        "lux": 9.75,
        "mic_mean": 2048.0,
        "mic_rms": 10.5,
        "mic_peak": 99.0,
        "mic_db_rel": 20.4,
        "mic_clip_count": 2,
        "error_flags": 0,
    }


def test_schema_version_must_be_one() -> None:
    payload = valid_payload()
    payload["schema_version"] = 2

    with pytest.raises(ValidationError):
        MeasurementUpload.model_validate(payload)


@pytest.mark.parametrize("field", ["device_id", "sequence", "uptime_ms"])
def test_identity_sequence_and_uptime_are_required(field: str) -> None:
    payload = valid_payload()
    payload.pop(field)

    with pytest.raises(ValidationError):
        MeasurementUpload.model_validate(payload)


def test_nullable_sensor_fields_are_accepted() -> None:
    payload = valid_payload()
    payload["temperature_c"] = None
    payload["humidity_percent"] = None
    payload["lux"] = None

    measurement = MeasurementUpload.model_validate(payload)

    assert measurement.temperature_c is None
    assert measurement.humidity_percent is None
    assert measurement.lux is None


def test_wall_clock_unix_ms_can_be_omitted_or_present() -> None:
    with_wall_clock = MeasurementUpload.model_validate(valid_payload())
    without_wall_clock_payload = valid_payload()
    without_wall_clock_payload.pop("wall_clock_unix_ms")
    without_wall_clock = MeasurementUpload.model_validate(without_wall_clock_payload)

    assert with_wall_clock.wall_clock_unix_ms == 1_700_000_000_000
    assert without_wall_clock.wall_clock_unix_ms is None


def test_time_status_is_limited_to_firmware_values() -> None:
    payload = valid_payload()
    payload["time_status"] = "unknown"

    with pytest.raises(ValidationError):
        MeasurementUpload.model_validate(payload)


def test_duplicate_measurements_are_idempotent_success_in_sink() -> None:
    sink = InMemoryMeasurementSink()
    upload = MeasurementUpload.model_validate(valid_payload())

    first = sink.accept(upload)
    second = sink.accept(upload)

    assert first.duplicate is False
    assert second.duplicate is True
    assert len(sink) == 1
