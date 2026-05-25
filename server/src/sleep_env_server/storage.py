"""Measurement acceptance, durable storage, and history helpers."""

from __future__ import annotations

import json
import os
import sqlite3
import tempfile
from dataclasses import dataclass
from pathlib import Path
from threading import Lock
from typing import Any, Protocol

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

    def __init__(self, path: str | Path) -> None:
        """Initializes a SQLite store."""
        self.path = Path(path)
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
        """Writes one record with keep-first duplicate behavior."""
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
    """JSONL-backed append store with keep-first canonical reads."""

    name = "jsonl"

    def __init__(self, path: str | Path) -> None:
        """Initializes a JSONL store."""
        self.path = Path(path)
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
        """Reads canonical keep-first records from the JSONL file."""
        if not self.path.exists():
            return {}
        records: dict[tuple[str, int], MeasurementRecord] = {}
        with self.path.open("r", encoding="utf-8") as stream:
            for line in stream:
                if not line.strip():
                    continue
                event = json.loads(line)
                record = MeasurementRecord.from_jsonl_event(event)
                records.setdefault(record.key, record)
        return records


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
