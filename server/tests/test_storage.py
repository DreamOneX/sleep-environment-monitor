from __future__ import annotations

import json
from dataclasses import replace
from pathlib import Path

from sleep_env_server.config import AckPolicyConfig, StorageConfig, StorageTargetConfig
from sleep_env_server.models import MeasurementUpload
from sleep_env_server.storage import (
    ConfiguredMeasurementSink,
    ConfiguredStore,
    JsonlMeasurementStore,
    MeasurementRecord,
    SQLiteMeasurementStore,
    StorageWriteResult,
    canonical_payload_json,
    summarize_records,
)


def payload(sequence: int = 1, *, temperature_c: float | None = 21.5) -> dict[str, object]:
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
        "mic_db_rel": 20.4,
        "mic_clip_count": 2,
        "error_flags": 0,
    }


def record(sequence: int = 1, *, temperature_c: float | None = 21.5) -> MeasurementRecord:
    upload = MeasurementUpload.model_validate(payload(sequence, temperature_c=temperature_c))
    return MeasurementRecord.from_upload(
        upload,
        received_unix_ms=1_800_000_000_000 + sequence,
        source="10.0.0.2",
    )


class FailingStore:
    name = "failing"

    def write(self, record: MeasurementRecord) -> StorageWriteResult:
        return StorageWriteResult(backend=self.name, stored=False, error="boom")

    def list_records(
        self,
        *,
        device_id: str | None = None,
        start_unix_ms: int | None = None,
        end_unix_ms: int | None = None,
        limit: int = 100,
        offset: int = 0,
    ) -> list[MeasurementRecord]:
        return []

    def summary(
        self,
        *,
        device_id: str | None = None,
        start_unix_ms: int | None = None,
        end_unix_ms: int | None = None,
    ) -> object:
        return summarize_records([])


def upload(sequence: int = 1, *, temperature_c: float | None = 21.5) -> MeasurementUpload:
    return MeasurementUpload.model_validate(payload(sequence, temperature_c=temperature_c))


def test_measurement_record_uses_device_time_when_available() -> None:
    item = record(1)

    assert item.display_unix_ms == 1_700_000_000_001
    assert item.display_time_source == "device_reported"


def test_measurement_record_falls_back_to_server_receive_time() -> None:
    raw = payload(1)
    raw.pop("wall_clock_unix_ms")
    raw["time_status"] = "uptime_only"
    upload = MeasurementUpload.model_validate(raw)

    item = MeasurementRecord.from_upload(upload, received_unix_ms=1_800_000_000_001)

    assert item.display_unix_ms == 1_800_000_000_001
    assert item.display_time_source == "server_received"


def test_canonical_payload_json_is_stable() -> None:
    upload = MeasurementUpload.model_validate(payload(1))

    decoded = json.loads(canonical_payload_json(upload))

    assert decoded["device_id"] == "sleep-env-esp32c3"
    assert decoded["sequence"] == 1


def test_sqlite_store_writes_lists_and_summarizes_records(tmp_path: Path) -> None:
    store = SQLiteMeasurementStore(tmp_path / "measurements.db")

    first = store.write(record(1))
    second = store.write(record(2, temperature_c=None))

    assert first.stored is True
    assert first.duplicate is False
    assert second.stored is True
    rows = store.list_records()
    assert [row.upload.sequence for row in rows] == [1, 2]
    summary = store.summary()
    assert summary.count == 2
    assert summary.devices == ("sleep-env-esp32c3",)
    assert summary.averages["temperature_c"] == 21.5


def test_sqlite_store_reports_duplicate_and_conflict(tmp_path: Path) -> None:
    store = SQLiteMeasurementStore(tmp_path / "measurements.db")

    assert store.write(record(1)).duplicate is False
    duplicate = store.write(record(1))
    conflict = store.write(record(1, temperature_c=30.0))

    assert duplicate.duplicate is True
    assert duplicate.conflict is False
    assert conflict.duplicate is True
    assert conflict.conflict is True
    assert store.list_records()[0].upload.temperature_c == 21.5


def test_sqlite_store_rejects_conflicting_duplicate_when_configured(tmp_path: Path) -> None:
    store = SQLiteMeasurementStore(tmp_path / "measurements.db", dedup_strategy="reject")

    assert store.write(record(1)).stored is True
    conflict = store.write(record(1, temperature_c=30.0))

    assert conflict.stored is False
    assert conflict.duplicate is True
    assert conflict.conflict is True
    assert store.list_records()[0].upload.temperature_c == 21.5


def test_jsonl_store_appends_and_lists_canonical_records(tmp_path: Path) -> None:
    store = JsonlMeasurementStore(tmp_path / "measurements.jsonl")

    assert store.write(record(1)).duplicate is False
    assert store.write(record(2)).duplicate is False
    duplicate = store.write(record(1, temperature_c=30.0))

    assert duplicate.duplicate is True
    assert duplicate.conflict is True
    rows = store.list_records()
    assert [row.upload.sequence for row in rows] == [1, 2]
    assert rows[0].upload.temperature_c == 21.5


def test_jsonl_store_keep_last_uses_latest_conflicting_record(tmp_path: Path) -> None:
    store = JsonlMeasurementStore(tmp_path / "measurements.jsonl", dedup_strategy="keep_last")

    assert store.write(record(1)).stored is True
    assert store.write(record(1, temperature_c=30.0)).conflict is True

    rows = store.list_records()
    assert len(rows) == 1
    assert rows[0].upload.temperature_c == 30.0


def test_jsonl_store_compacts_to_canonical_records(tmp_path: Path) -> None:
    path = tmp_path / "measurements.jsonl"
    store = JsonlMeasurementStore(path)
    store.write(record(1))
    store.write(record(1, temperature_c=30.0))
    store.write(record(2))

    written = store.compact()

    assert written == 2
    assert len(path.read_text(encoding="utf-8").splitlines()) == 2
    assert [row.upload.sequence for row in store.list_records()] == [1, 2]


def test_summarize_records_ignores_missing_values() -> None:
    summary = summarize_records([record(1), record(2, temperature_c=None)])

    assert summary.count == 2
    assert summary.first_received_unix_ms == 1_800_000_000_001
    assert summary.last_received_unix_ms == 1_800_000_000_002
    assert summary.averages["temperature_c"] == 21.5


def test_configured_sink_ack_ignores_storage_when_global_required_is_false() -> None:
    sink = ConfiguredMeasurementSink(
        StorageConfig(required_for_ack=False),
        stores=(
            ConfiguredStore(
                target="failing",
                store=FailingStore(),
                ack=AckPolicyConfig(required_for_ack=True, sufficient_for_ack=True),
            ),
        ),
    )

    result = sink.accept_upload(upload(1), received_unix_ms=1_900_000_000_001)

    assert result.accepted is True
    assert result.status_code == 204
    assert result.results[0].stored is False


def test_configured_sink_ack_accepts_sufficient_backend_success(tmp_path: Path) -> None:
    config = replace(
        StorageConfig(),
        sqlite=StorageTargetConfig(
            enabled=True,
            path=str(tmp_path / "measurements.db"),
            policy="no_limit",
            ack=AckPolicyConfig(required_for_ack=True, sufficient_for_ack=True),
        ),
        jsonl=StorageTargetConfig(enabled=False),
    )
    sink = ConfiguredMeasurementSink(config)

    result = sink.accept_upload(upload(1), received_unix_ms=1_900_000_000_001)

    assert result.accepted is True
    assert result.status_code == 204
    assert result.results[0].backend == "sqlite"
    assert result.results[0].stored is True


def test_configured_sink_ack_falls_back_to_required_backends(tmp_path: Path) -> None:
    jsonl = JsonlMeasurementStore(tmp_path / "measurements.jsonl")
    sink = ConfiguredMeasurementSink(
        StorageConfig(),
        stores=(
            ConfiguredStore(
                target="failing",
                store=FailingStore(),
                ack=AckPolicyConfig(sufficient_for_ack=True),
            ),
            ConfiguredStore(
                target="jsonl",
                store=jsonl,
                ack=AckPolicyConfig(required_for_ack=True),
            ),
        ),
    )

    result = sink.accept_upload(upload(1), received_unix_ms=1_900_000_000_001)

    assert result.accepted is True
    assert result.status_code == 204
    assert [item.stored for item in result.results] == [False, True]


def test_configured_sink_ack_rejects_missing_ack_path() -> None:
    config = replace(
        StorageConfig(),
        sqlite=StorageTargetConfig(enabled=False),
        jsonl=StorageTargetConfig(enabled=False),
    )
    sink = ConfiguredMeasurementSink(config)

    result = sink.accept_upload(upload(1), received_unix_ms=1_900_000_000_001)

    assert result.accepted is False
    assert result.status_code == 503
    assert result.reason == "storage required for ACK but no storage backend is enabled"


def test_configured_sink_ack_rejects_conflicting_duplicate_when_required(
    tmp_path: Path,
) -> None:
    store = SQLiteMeasurementStore(tmp_path / "measurements.db", dedup_strategy="reject")
    sink = ConfiguredMeasurementSink(
        StorageConfig(),
        stores=(
            ConfiguredStore(
                target="sqlite",
                store=store,
                ack=AckPolicyConfig(required_for_ack=True, sufficient_for_ack=True),
            ),
        ),
    )

    assert sink.accept_upload(upload(1), received_unix_ms=1_900_000_000_001).accepted is True
    result = sink.accept_upload(
        upload(1, temperature_c=30.0),
        received_unix_ms=1_900_000_000_002,
    )

    assert result.accepted is False
    assert result.status_code == 409
    assert result.duplicate is True
    assert result.conflict is True


def test_configured_sink_reconcile_copies_missing_records_between_stores(
    tmp_path: Path,
) -> None:
    sqlite = SQLiteMeasurementStore(tmp_path / "measurements.db")
    jsonl = JsonlMeasurementStore(tmp_path / "measurements.jsonl")
    sqlite.write(record(1))
    jsonl.write(record(2))
    sink = ConfiguredMeasurementSink(
        StorageConfig(),
        stores=(
            ConfiguredStore(target="sqlite", store=sqlite, ack=AckPolicyConfig()),
            ConfiguredStore(target="jsonl", store=jsonl, ack=AckPolicyConfig()),
        ),
    )

    copied = sink.reconcile_once()

    assert copied == 2
    assert [item.upload.sequence for item in sqlite.list_records()] == [1, 2]
    assert [item.upload.sequence for item in jsonl.list_records()] == [1, 2]
