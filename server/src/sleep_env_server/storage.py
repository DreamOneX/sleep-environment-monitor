"""Measurement acceptance, durable storage, and history helpers."""

from __future__ import annotations

import json
import os
import sqlite3
import tempfile
from dataclasses import dataclass, replace
from pathlib import Path
from threading import Event, Lock, Thread
from typing import Any, Protocol

from sleep_env_server.config import (
    AckPolicyConfig,
    PolicyProfileConfig,
    StorageConfig,
    StoragePolicyConfig,
    StorageTargetConfig,
)
from sleep_env_server.models import MeasurementUpload


@dataclass(frozen=True)
class AcceptedMeasurement:
    """Result returned after accepting an upload into the process-local sink."""

    duplicate: bool


@dataclass(frozen=True)
class MeasurementRecord:
    """Server-side representation of one received measurement."""

    upload: MeasurementUpload
    received_unix_ms: int
    source: str
    display_unix_ms: int
    display_time_source: str

    @classmethod
    def from_upload(
        cls,
        upload: MeasurementUpload,
        *,
        received_unix_ms: int,
        source: str = "unknown",
    ) -> MeasurementRecord:
        """Builds a persisted record from a validated upload."""
        if upload.wall_clock_unix_ms is None:
            display_unix_ms = received_unix_ms
            display_time_source = "server_received"
        else:
            display_unix_ms = upload.wall_clock_unix_ms
            display_time_source = "device_reported"
        return cls(
            upload=upload,
            received_unix_ms=received_unix_ms,
            source=source,
            display_unix_ms=display_unix_ms,
            display_time_source=display_time_source,
        )

    @property
    def key(self) -> tuple[str, int]:
        """Returns the idempotency key."""
        return (self.upload.device_id, self.upload.sequence)

    @property
    def payload_json(self) -> str:
        """Returns canonical payload JSON for comparisons and storage."""
        return canonical_payload_json(self.upload)

    def to_jsonl_event(self) -> dict[str, Any]:
        """Returns a JSONL-serializable event."""
        return {
            "received_unix_ms": self.received_unix_ms,
            "source": self.source,
            "display_unix_ms": self.display_unix_ms,
            "display_time_source": self.display_time_source,
            "payload": self.upload.model_dump(mode="json"),
        }

    @classmethod
    def from_jsonl_event(cls, event: dict[str, Any]) -> MeasurementRecord:
        """Builds a measurement record from a JSONL event."""
        upload = MeasurementUpload.model_validate(event["payload"])
        received_unix_ms = int(event["received_unix_ms"])
        default = cls.from_upload(
            upload,
            received_unix_ms=received_unix_ms,
            source=str(event.get("source", "unknown")),
        )
        return cls(
            upload=upload,
            received_unix_ms=received_unix_ms,
            source=str(event.get("source", "unknown")),
            display_unix_ms=int(event.get("display_unix_ms", default.display_unix_ms)),
            display_time_source=str(event.get("display_time_source", default.display_time_source)),
        )


@dataclass(frozen=True)
class StorageWriteResult:
    """Result from one persistent backend write."""

    backend: str
    stored: bool
    duplicate: bool = False
    conflict: bool = False
    error: str | None = None


@dataclass(frozen=True)
class StorageAcceptance:
    """Composite upload acceptance result."""

    accepted: bool
    duplicate: bool
    conflict: bool
    results: tuple[StorageWriteResult, ...] = ()
    status_code: int = 204
    reason: str | None = None


@dataclass(frozen=True)
class ConfiguredStore:
    """Persistent backend with effective ACK settings."""

    target: str
    store: MeasurementStore
    ack: AckPolicyConfig


@dataclass(frozen=True)
class HistorySummary:
    """Small summary for history views and APIs."""

    count: int
    devices: tuple[str, ...]
    first_received_unix_ms: int | None
    last_received_unix_ms: int | None
    averages: dict[str, float]


class MeasurementStore(Protocol):
    """Persistent measurement store protocol."""

    name: str

    def write(self, record: MeasurementRecord) -> StorageWriteResult:
        """Writes one measurement record."""

    def list_records(
        self,
        *,
        device_id: str | None = None,
        start_unix_ms: int | None = None,
        end_unix_ms: int | None = None,
        limit: int = 100,
        offset: int = 0,
    ) -> list[MeasurementRecord]:
        """Lists canonical records."""

    def summary(
        self,
        *,
        device_id: str | None = None,
        start_unix_ms: int | None = None,
        end_unix_ms: int | None = None,
    ) -> HistorySummary:
        """Returns a summary for canonical records."""


class InMemoryMeasurementSink:
    """Stores accepted upload keys for process-local idempotency.

    The sink intentionally does not provide durable persistence. It records the
    first payload seen for each ``(device_id, sequence)`` key so firmware retries
    can be acknowledged as idempotent success.
    """

    def __init__(self) -> None:
        """Initializes an empty in-memory sink."""
        self._measurements: dict[tuple[str, int], MeasurementUpload] = {}
        self._lock = Lock()

    def accept(self, measurement: MeasurementUpload) -> AcceptedMeasurement:
        """Accepts a measurement upload.

        Args:
            measurement: Validated schema-version-1 upload.

        Returns:
            Acceptance metadata including whether this key was already seen.
        """
        key = (measurement.device_id, measurement.sequence)
        with self._lock:
            duplicate = key in self._measurements
            if not duplicate:
                self._measurements[key] = measurement
        return AcceptedMeasurement(duplicate=duplicate)

    def __len__(self) -> int:
        """Returns the number of unique measurement keys accepted."""
        with self._lock:
            return len(self._measurements)


class SQLiteMeasurementStore:
    """SQLite-backed canonical measurement store."""

    name = "sqlite"

    def __init__(self, path: str | Path, *, dedup_strategy: str = "keep_first") -> None:
        """Initializes a SQLite store."""
        self.path = Path(path)
        self.dedup_strategy = dedup_strategy
        self._initialized = False
        self._lock = Lock()

    def initialize(self) -> None:
        """Creates the database schema if needed."""
        if self._initialized:
            return
        if self.path.parent != Path("."):
            self.path.parent.mkdir(parents=True, exist_ok=True)
        with self._connect() as conn:
            conn.execute(
                """
                CREATE TABLE IF NOT EXISTS measurements (
                    device_id TEXT NOT NULL,
                    sequence INTEGER NOT NULL,
                    received_unix_ms INTEGER NOT NULL,
                    source TEXT NOT NULL,
                    display_unix_ms INTEGER NOT NULL,
                    display_time_source TEXT NOT NULL,
                    payload_json TEXT NOT NULL,
                    schema_version INTEGER NOT NULL,
                    time_status TEXT NOT NULL,
                    wall_clock_unix_ms INTEGER,
                    uptime_ms INTEGER NOT NULL,
                    temperature_c REAL,
                    humidity_percent REAL,
                    lux REAL,
                    mic_mean REAL NOT NULL,
                    mic_rms REAL NOT NULL,
                    mic_peak REAL NOT NULL,
                    mic_db_rel REAL NOT NULL,
                    mic_clip_count INTEGER NOT NULL,
                    error_flags INTEGER NOT NULL,
                    duplicate_count INTEGER NOT NULL DEFAULT 0,
                    updated_unix_ms INTEGER NOT NULL,
                    PRIMARY KEY (device_id, sequence)
                )
                """
            )
            conn.execute(
                """
                CREATE INDEX IF NOT EXISTS idx_measurements_display_time
                ON measurements(display_unix_ms)
                """
            )
        self._initialized = True

    def write(self, record: MeasurementRecord) -> StorageWriteResult:
        """Writes one record using the configured duplicate behavior."""
        with self._lock:
            self.initialize()
            with self._connect() as conn:
                row = conn.execute(
                    """
                    SELECT payload_json FROM measurements
                    WHERE device_id = ? AND sequence = ?
                    """,
                    record.key,
                ).fetchone()
                if row is not None:
                    conflict = row["payload_json"] != record.payload_json
                    if conflict and self.dedup_strategy == "reject":
                        return StorageWriteResult(
                            backend=self.name,
                            stored=False,
                            duplicate=True,
                            conflict=True,
                            error="duplicate conflict rejected",
                        )
                    if conflict and self.dedup_strategy in ("keep_last", "overwrite"):
                        conn.execute(
                            """
                            UPDATE measurements
                            SET received_unix_ms = ?,
                                source = ?,
                                display_unix_ms = ?,
                                display_time_source = ?,
                                payload_json = ?,
                                schema_version = ?,
                                time_status = ?,
                                wall_clock_unix_ms = ?,
                                uptime_ms = ?,
                                temperature_c = ?,
                                humidity_percent = ?,
                                lux = ?,
                                mic_mean = ?,
                                mic_rms = ?,
                                mic_peak = ?,
                                mic_db_rel = ?,
                                mic_clip_count = ?,
                                error_flags = ?,
                                duplicate_count = duplicate_count + 1,
                                updated_unix_ms = ?
                            WHERE device_id = ? AND sequence = ?
                            """,
                            (
                                record.received_unix_ms,
                                record.source,
                                record.display_unix_ms,
                                record.display_time_source,
                                record.payload_json,
                                record.upload.schema_version,
                                record.upload.time_status,
                                record.upload.wall_clock_unix_ms,
                                record.upload.uptime_ms,
                                record.upload.temperature_c,
                                record.upload.humidity_percent,
                                record.upload.lux,
                                record.upload.mic_mean,
                                record.upload.mic_rms,
                                record.upload.mic_peak,
                                record.upload.mic_db_rel,
                                record.upload.mic_clip_count,
                                record.upload.error_flags,
                                record.received_unix_ms,
                                *record.key,
                            ),
                        )
                        return StorageWriteResult(
                            backend=self.name,
                            stored=True,
                            duplicate=True,
                            conflict=True,
                        )
                    conn.execute(
                        """
                        UPDATE measurements
                        SET duplicate_count = duplicate_count + 1,
                            updated_unix_ms = ?
                        WHERE device_id = ? AND sequence = ?
                        """,
                        (record.received_unix_ms, *record.key),
                    )
                    return StorageWriteResult(
                        backend=self.name,
                        stored=True,
                        duplicate=True,
                        conflict=conflict,
                    )

                conn.execute(
                    """
                    INSERT INTO measurements (
                        device_id, sequence, received_unix_ms, source,
                        display_unix_ms, display_time_source, payload_json,
                        schema_version, time_status, wall_clock_unix_ms,
                        uptime_ms, temperature_c, humidity_percent, lux,
                        mic_mean, mic_rms, mic_peak, mic_db_rel,
                        mic_clip_count, error_flags, duplicate_count,
                        updated_unix_ms
                    ) VALUES (
                        ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, ?
                    )
                    """,
                    _sqlite_values(record),
                )
        return StorageWriteResult(backend=self.name, stored=True)

    def list_records(
        self,
        *,
        device_id: str | None = None,
        start_unix_ms: int | None = None,
        end_unix_ms: int | None = None,
        limit: int = 100,
        offset: int = 0,
    ) -> list[MeasurementRecord]:
        """Lists canonical records ordered by display time then key."""
        self.initialize()
        where, params = _history_where(device_id, start_unix_ms, end_unix_ms)
        with self._connect() as conn:
            rows = conn.execute(
                f"""
                SELECT * FROM measurements
                {where}
                ORDER BY display_unix_ms ASC, device_id ASC, sequence ASC
                LIMIT ? OFFSET ?
                """,
                (*params, limit, offset),
            ).fetchall()
        return [_record_from_sqlite_row(row) for row in rows]

    def summary(
        self,
        *,
        device_id: str | None = None,
        start_unix_ms: int | None = None,
        end_unix_ms: int | None = None,
    ) -> HistorySummary:
        """Returns a summary for canonical records."""
        records = self.list_records(
            device_id=device_id,
            start_unix_ms=start_unix_ms,
            end_unix_ms=end_unix_ms,
            limit=1_000_000,
        )
        return summarize_records(records)

    def _connect(self) -> sqlite3.Connection:
        """Opens a configured SQLite connection."""
        conn = sqlite3.connect(self.path)
        conn.row_factory = sqlite3.Row
        return conn


class JsonlMeasurementStore:
    """JSONL-backed append store with configurable canonical reads."""

    name = "jsonl"

    def __init__(self, path: str | Path, *, dedup_strategy: str = "keep_first") -> None:
        """Initializes a JSONL store."""
        self.path = Path(path)
        self.dedup_strategy = dedup_strategy
        self._lock = Lock()

    def initialize(self) -> None:
        """Creates the parent directory and file if needed."""
        if self.path.parent != Path("."):
            self.path.parent.mkdir(parents=True, exist_ok=True)
        self.path.touch(exist_ok=True)

    def write(self, record: MeasurementRecord) -> StorageWriteResult:
        """Appends one event and reports duplicate/conflict status."""
        with self._lock:
            self.initialize()
            canonical = self._canonical_records()
            existing = canonical.get(record.key)
            duplicate = existing is not None
            conflict = existing is not None and existing.payload_json != record.payload_json
            if conflict and self.dedup_strategy == "reject":
                return StorageWriteResult(
                    backend=self.name,
                    stored=False,
                    duplicate=True,
                    conflict=True,
                    error="duplicate conflict rejected",
                )
            with self.path.open("a", encoding="utf-8") as stream:
                stream.write(json.dumps(record.to_jsonl_event(), separators=(",", ":")))
                stream.write("\n")
        return StorageWriteResult(
            backend=self.name,
            stored=True,
            duplicate=duplicate,
            conflict=conflict,
        )

    def list_records(
        self,
        *,
        device_id: str | None = None,
        start_unix_ms: int | None = None,
        end_unix_ms: int | None = None,
        limit: int = 100,
        offset: int = 0,
    ) -> list[MeasurementRecord]:
        """Lists canonical records ordered by display time then key."""
        records = list(self._canonical_records().values())
        records = _filter_records(
            records,
            device_id=device_id,
            start_unix_ms=start_unix_ms,
            end_unix_ms=end_unix_ms,
        )
        records.sort(key=lambda record: (record.display_unix_ms, *record.key))
        return records[offset : offset + limit]

    def summary(
        self,
        *,
        device_id: str | None = None,
        start_unix_ms: int | None = None,
        end_unix_ms: int | None = None,
    ) -> HistorySummary:
        """Returns a summary for canonical records."""
        return summarize_records(
            self.list_records(
                device_id=device_id,
                start_unix_ms=start_unix_ms,
                end_unix_ms=end_unix_ms,
                limit=1_000_000,
            )
        )

    def compact(self) -> int:
        """Atomically rewrites the file with canonical records only.

        Returns:
            Number of canonical records written.
        """
        with self._lock:
            records = list(self._canonical_records().values())
            if self.path.parent != Path("."):
                self.path.parent.mkdir(parents=True, exist_ok=True)
            with tempfile.NamedTemporaryFile(
                "w",
                encoding="utf-8",
                dir=self.path.parent,
                delete=False,
            ) as tmp:
                tmp_path = Path(tmp.name)
                for record in records:
                    tmp.write(json.dumps(record.to_jsonl_event(), separators=(",", ":")))
                    tmp.write("\n")
            os.replace(tmp_path, self.path)
        return len(records)

    def _canonical_records(self) -> dict[tuple[str, int], MeasurementRecord]:
        """Reads canonical records from the JSONL file."""
        if not self.path.exists():
            return {}
        records: dict[tuple[str, int], MeasurementRecord] = {}
        with self.path.open("r", encoding="utf-8") as stream:
            for line in stream:
                if not line.strip():
                    continue
                event = json.loads(line)
                record = MeasurementRecord.from_jsonl_event(event)
                existing = records.get(record.key)
                if existing is None:
                    records[record.key] = record
                    continue
                if existing.payload_json != record.payload_json and self.dedup_strategy in (
                    "keep_last",
                    "overwrite",
                ):
                    records[record.key] = record
        return records


class ConfiguredMeasurementSink:
    """Composite sink that writes configured stores and evaluates ACK policy."""

    def __init__(
        self,
        storage_config: StorageConfig,
        *,
        stores: tuple[ConfiguredStore, ...] | None = None,
    ) -> None:
        """Initializes a configured sink."""
        self.storage_config = storage_config
        self.memory = InMemoryMeasurementSink()
        self.stores = stores if stores is not None else build_configured_stores(storage_config)

    def accept_upload(
        self,
        measurement: MeasurementUpload,
        *,
        received_unix_ms: int,
        source: str = "unknown",
    ) -> StorageAcceptance:
        """Accepts an upload into memory and configured durable stores."""
        memory_result = self.memory.accept(measurement)
        if not self.storage_config.enabled:
            if self.storage_config.required_for_ack:
                return StorageAcceptance(
                    accepted=False,
                    duplicate=memory_result.duplicate,
                    conflict=False,
                    status_code=503,
                    reason="storage required for ACK but storage is disabled",
                )
            return StorageAcceptance(
                accepted=True,
                duplicate=memory_result.duplicate,
                conflict=False,
            )

        record = MeasurementRecord.from_upload(
            measurement,
            received_unix_ms=received_unix_ms,
            source=source,
        )
        if self.storage_config.time_source == "server_received":
            record = replace(
                record,
                display_unix_ms=received_unix_ms,
                display_time_source="server_received",
            )
        results = tuple(_write_store(configured, record) for configured in self.stores)
        accepted, reason = self._ack_satisfied(results)
        status_code = 204 if accepted else _unsatisfied_status(results)
        return StorageAcceptance(
            accepted=accepted,
            duplicate=memory_result.duplicate or any(result.duplicate for result in results),
            conflict=any(result.conflict for result in results),
            results=results,
            status_code=status_code,
            reason=reason,
        )

    def _ack_satisfied(self, results: tuple[StorageWriteResult, ...]) -> tuple[bool, str | None]:
        """Evaluates the configured ACK policy."""
        if not self.storage_config.required_for_ack:
            return True, None
        if not self.stores:
            return False, "storage required for ACK but no storage backend is enabled"

        result_by_backend = {result.backend: result for result in results}
        sufficient = [configured for configured in self.stores if configured.ack.sufficient_for_ack]
        if any(result_by_backend[configured.target].stored for configured in sufficient):
            return True, None

        required = [configured for configured in self.stores if configured.ack.required_for_ack]
        if not required:
            return False, "storage required for ACK but no required backend is configured"
        if all(result_by_backend[configured.target].stored for configured in required):
            return True, None
        return False, "storage ACK policy was not satisfied"

    def reconcile_once(self) -> int:
        """Copies missing canonical records between enabled stores once."""
        if len(self.stores) < 2:
            return 0
        by_key: dict[tuple[str, int], MeasurementRecord] = {}
        present: dict[str, set[tuple[str, int]]] = {}
        for configured in self.stores:
            records = configured.store.list_records(limit=1_000_000)
            present[configured.target] = {record.key for record in records}
            for record in records:
                by_key.setdefault(record.key, record)

        writes = 0
        for configured in self.stores:
            missing = [
                record for key, record in by_key.items() if key not in present[configured.target]
            ]
            for record in missing:
                if configured.store.write(record).stored:
                    writes += 1
        return writes

    def compact_jsonl_once(self) -> int:
        """Compacts configured JSONL stores once."""
        written = 0
        for configured in self.stores:
            if isinstance(configured.store, JsonlMeasurementStore):
                written += configured.store.compact()
        return written


class StorageMaintenanceThread(Thread):
    """Background storage reconciliation loop."""

    def __init__(self, sink: ConfiguredMeasurementSink, interval_seconds: int) -> None:
        """Initializes a daemon maintenance thread."""
        super().__init__(name="storage-maintenance", daemon=True)
        self._sink = sink
        self._interval_seconds = interval_seconds
        self._stop_requested = Event()

    def stop(self) -> None:
        """Requests maintenance shutdown."""
        self._stop_requested.set()

    def run(self) -> None:
        """Runs periodic reconciliation until stopped."""
        while not self._stop_requested.wait(self._interval_seconds):
            self._sink.reconcile_once()
            self._sink.compact_jsonl_once()


def build_configured_stores(config: StorageConfig) -> tuple[ConfiguredStore, ...]:
    """Builds enabled durable stores from configuration."""
    if not config.enabled:
        return ()
    stores: list[ConfiguredStore] = []
    if config.sqlite.enabled:
        stores.append(_configured_store("sqlite", config.sqlite, config))
    if config.jsonl.enabled:
        stores.append(_configured_store("jsonl", config.jsonl, config))
    return tuple(stores)


def list_configured_history_records(
    stores: tuple[ConfiguredStore, ...],
    *,
    read_source: str,
    merge_sources: tuple[str, ...] = ("sqlite", "jsonl"),
    merge_conflict: str = "error",
    device_id: str | None = None,
    start_unix_ms: int | None = None,
    end_unix_ms: int | None = None,
    limit: int = 100,
    offset: int = 0,
) -> list[MeasurementRecord]:
    """Lists records from one configured source or a merged view."""
    configured_by_target = {configured.target: configured for configured in stores}
    if read_source != "merge":
        configured = configured_by_target.get(read_source)
        if configured is None:
            raise ValueError(f"history source is not configured: {read_source}")
        return configured.store.list_records(
            device_id=device_id,
            start_unix_ms=start_unix_ms,
            end_unix_ms=end_unix_ms,
            limit=limit,
            offset=offset,
        )

    records_by_key: dict[tuple[str, int], MeasurementRecord] = {}
    used_sources = 0
    for source in merge_sources:
        configured = configured_by_target.get(source)
        if configured is None:
            continue
        used_sources += 1
        records = configured.store.list_records(
            device_id=device_id,
            start_unix_ms=start_unix_ms,
            end_unix_ms=end_unix_ms,
            limit=1_000_000,
        )
        for record in records:
            _merge_history_record(records_by_key, record, merge_conflict)

    if used_sources == 0:
        raise ValueError("no configured history sources are available")
    records = list(records_by_key.values())
    records.sort(key=lambda record: (record.display_unix_ms, *record.key))
    return records[offset : offset + limit]


def summarize_configured_history_records(
    stores: tuple[ConfiguredStore, ...],
    *,
    read_source: str,
    merge_sources: tuple[str, ...] = ("sqlite", "jsonl"),
    merge_conflict: str = "error",
    device_id: str | None = None,
    start_unix_ms: int | None = None,
    end_unix_ms: int | None = None,
) -> HistorySummary:
    """Summarizes records from one configured source or a merged view."""
    return summarize_records(
        list_configured_history_records(
            stores,
            read_source=read_source,
            merge_sources=merge_sources,
            merge_conflict=merge_conflict,
            device_id=device_id,
            start_unix_ms=start_unix_ms,
            end_unix_ms=end_unix_ms,
            limit=1_000_000,
        )
    )


def history_record_to_dict(record: MeasurementRecord) -> dict[str, Any]:
    """Returns a public history representation for API and CLI output."""
    return {
        "received_unix_ms": record.received_unix_ms,
        "source": record.source,
        "display_unix_ms": record.display_unix_ms,
        "display_time_source": record.display_time_source,
        "payload": record.upload.model_dump(mode="json"),
    }


def history_summary_to_dict(summary: HistorySummary) -> dict[str, Any]:
    """Returns a public summary representation for API and CLI output."""
    return {
        "count": summary.count,
        "devices": list(summary.devices),
        "first_received_unix_ms": summary.first_received_unix_ms,
        "last_received_unix_ms": summary.last_received_unix_ms,
        "averages": summary.averages,
    }


def _configured_store(
    name: str,
    target: StorageTargetConfig,
    config: StorageConfig,
) -> ConfiguredStore:
    """Builds one configured store with effective profile settings."""
    profile = _effective_policy_profile(target.policy, config.policy)
    if name == "sqlite":
        store: MeasurementStore = SQLiteMeasurementStore(
            target.path,
            dedup_strategy=profile.deduplication.strategy,
        )
    elif name == "jsonl":
        store = JsonlMeasurementStore(
            target.path,
            dedup_strategy=profile.deduplication.strategy,
        )
    else:
        raise ValueError(f"unsupported storage target: {name}")
    return ConfiguredStore(
        target=name,
        store=store,
        ack=_effective_ack(target, profile),
    )


def _merge_history_record(
    records_by_key: dict[tuple[str, int], MeasurementRecord],
    record: MeasurementRecord,
    merge_conflict: str,
) -> None:
    """Merges one record into a canonical history view."""
    existing = records_by_key.get(record.key)
    if existing is None:
        records_by_key[record.key] = record
        return
    if existing.payload_json == record.payload_json:
        return
    if merge_conflict == "keep":
        return
    if merge_conflict == "overwrite":
        records_by_key[record.key] = record
        return
    if merge_conflict == "earliest":
        if record.display_unix_ms < existing.display_unix_ms:
            records_by_key[record.key] = record
        return
    if merge_conflict == "latest":
        if record.display_unix_ms > existing.display_unix_ms:
            records_by_key[record.key] = record
        return
    raise ValueError(
        f"history merge conflict for {record.upload.device_id}:{record.upload.sequence}"
    )


def _effective_policy_profile(
    name: str,
    policy: StoragePolicyConfig,
) -> PolicyProfileConfig:
    """Returns a profile with parent settings applied.

    Profile parsing keeps normal dataclass defaults for omitted nested fields.
    During inheritance, a child field that still equals its type default is
    treated as omitted so common child profiles can inherit parent ACK,
    deduplication, limit, and backfill settings without repeating them.
    """
    profile = policy.profiles[name]
    parent_name = profile.parent or policy.default_parent
    if not parent_name:
        return profile
    parent = _effective_policy_profile(parent_name, policy)
    default = PolicyProfileConfig()
    return PolicyProfileConfig(
        parent=profile.parent,
        limit=profile.limit if profile.limit != default.limit else parent.limit,
        deduplication=(
            profile.deduplication
            if profile.deduplication != default.deduplication
            else parent.deduplication
        ),
        ack=profile.ack if profile.ack != default.ack else parent.ack,
        backfill=profile.backfill if profile.backfill != default.backfill else parent.backfill,
    )


def _effective_ack(
    target: StorageTargetConfig,
    profile: PolicyProfileConfig,
) -> AckPolicyConfig:
    """Returns target ACK settings, falling back to the policy profile."""
    if target.ack.required_for_ack or target.ack.sufficient_for_ack:
        return target.ack
    return profile.ack


def _write_store(
    configured: ConfiguredStore,
    record: MeasurementRecord,
) -> StorageWriteResult:
    """Writes to one store and converts backend exceptions into status."""
    try:
        result = configured.store.write(record)
    except Exception as exc:  # noqa: BLE001 - backend failures must not crash upload handling
        return StorageWriteResult(
            backend=configured.target,
            stored=False,
            error=str(exc),
        )
    if result.backend == configured.target:
        return result
    return StorageWriteResult(
        backend=configured.target,
        stored=result.stored,
        duplicate=result.duplicate,
        conflict=result.conflict,
        error=result.error,
    )


def _unsatisfied_status(results: tuple[StorageWriteResult, ...]) -> int:
    """Maps an unsatisfied ACK policy to a firmware-visible status code."""
    if any(result.conflict for result in results):
        return 409
    return 503


def canonical_payload_json(upload: MeasurementUpload) -> str:
    """Returns stable JSON for a measurement upload."""
    return json.dumps(upload.model_dump(mode="json"), separators=(",", ":"), sort_keys=True)


def summarize_records(records: list[MeasurementRecord]) -> HistorySummary:
    """Builds a small summary for canonical records."""
    devices = tuple(sorted({record.upload.device_id for record in records}))
    first = min((record.received_unix_ms for record in records), default=None)
    last = max((record.received_unix_ms for record in records), default=None)
    averages: dict[str, float] = {}
    for field in ("temperature_c", "humidity_percent", "lux", "mic_db_rel"):
        values = [
            value for record in records if (value := getattr(record.upload, field)) is not None
        ]
        if values:
            averages[field] = sum(values) / len(values)
    return HistorySummary(
        count=len(records),
        devices=devices,
        first_received_unix_ms=first,
        last_received_unix_ms=last,
        averages=averages,
    )


def _sqlite_values(record: MeasurementRecord) -> tuple[object, ...]:
    """Returns SQLite insert values for a record."""
    upload = record.upload
    return (
        upload.device_id,
        upload.sequence,
        record.received_unix_ms,
        record.source,
        record.display_unix_ms,
        record.display_time_source,
        record.payload_json,
        upload.schema_version,
        upload.time_status,
        upload.wall_clock_unix_ms,
        upload.uptime_ms,
        upload.temperature_c,
        upload.humidity_percent,
        upload.lux,
        upload.mic_mean,
        upload.mic_rms,
        upload.mic_peak,
        upload.mic_db_rel,
        upload.mic_clip_count,
        upload.error_flags,
        record.received_unix_ms,
    )


def _record_from_sqlite_row(row: sqlite3.Row) -> MeasurementRecord:
    """Builds a record from a SQLite row."""
    upload = MeasurementUpload.model_validate(json.loads(row["payload_json"]))
    return MeasurementRecord(
        upload=upload,
        received_unix_ms=int(row["received_unix_ms"]),
        source=str(row["source"]),
        display_unix_ms=int(row["display_unix_ms"]),
        display_time_source=str(row["display_time_source"]),
    )


def _history_where(
    device_id: str | None,
    start_unix_ms: int | None,
    end_unix_ms: int | None,
) -> tuple[str, tuple[object, ...]]:
    """Builds a SQLite WHERE clause and parameter tuple."""
    clauses: list[str] = []
    params: list[object] = []
    if device_id is not None:
        clauses.append("device_id = ?")
        params.append(device_id)
    if start_unix_ms is not None:
        clauses.append("display_unix_ms >= ?")
        params.append(start_unix_ms)
    if end_unix_ms is not None:
        clauses.append("display_unix_ms <= ?")
        params.append(end_unix_ms)
    if not clauses:
        return "", ()
    return "WHERE " + " AND ".join(clauses), tuple(params)


def _filter_records(
    records: list[MeasurementRecord],
    *,
    device_id: str | None,
    start_unix_ms: int | None,
    end_unix_ms: int | None,
) -> list[MeasurementRecord]:
    """Filters records using the same semantics as SQLite history reads."""
    filtered = records
    if device_id is not None:
        filtered = [record for record in filtered if record.upload.device_id == device_id]
    if start_unix_ms is not None:
        filtered = [record for record in filtered if record.display_unix_ms >= start_unix_ms]
    if end_unix_ms is not None:
        filtered = [record for record in filtered if record.display_unix_ms <= end_unix_ms]
    return filtered
