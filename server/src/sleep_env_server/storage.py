"""In-process measurement acceptance and duplicate tracking."""

from __future__ import annotations

from dataclasses import dataclass
from threading import Lock

from sleep_env_server.models import MeasurementUpload


@dataclass(frozen=True)
class AcceptedMeasurement:
    """Result returned after accepting an upload into the process-local sink."""

    duplicate: bool


class InMemoryMeasurementSink:
    """Stores accepted upload keys for process-local idempotency.

    The sink intentionally does not provide durable persistence in Phase 23.
    It records the first payload seen for each ``(device_id, sequence)`` key so
    firmware retries can be acknowledged as idempotent success.
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
