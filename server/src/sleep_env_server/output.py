"""Console and machine-readable output helpers."""

from __future__ import annotations

import json
import sys
from typing import Any, Literal, TextIO

from rich.console import Console
from rich.table import Table

from sleep_env_server.config import ServerConfig
from sleep_env_server.models import DiscoveryDocument, UdpDiscoveryPayload

OutputMode = Literal["rich", "plain", "json"]


class ServerOutput:
    """Writes bounded server diagnostics in Rich, plain, or JSONL form."""

    def __init__(
        self,
        mode: OutputMode = "plain",
        stream: TextIO | None = None,
        *,
        force_terminal: bool | None = None,
    ) -> None:
        """Initializes output targeting a text stream.

        Args:
            mode: Output mode. JSON mode emits one compact JSON object per line.
            stream: Destination stream. Defaults to stdout.
            force_terminal: Optional Rich terminal override for tests.
        """
        self.mode = mode
        self.stream = stream if stream is not None else sys.stdout
        self._console = Console(
            file=self.stream,
            force_terminal=force_terminal,
            highlight=False,
            markup=False,
        )

    def startup(self, config: ServerConfig, log_level: str) -> None:
        """Writes server startup metadata."""
        self.emit(
            "server_starting",
            host=config.host,
            port=config.port,
            udp_discovery_port=config.udp_discovery_port,
            api_base=config.api_base,
            log_level=log_level,
        )

    def udp_started(self, config: ServerConfig) -> None:
        """Writes UDP discovery startup metadata."""
        self.emit(
            "udp_discovery_started",
            host=config.host,
            udp_discovery_port=config.udp_discovery_port,
        )

    def udp_disabled(self, error: str) -> None:
        """Writes UDP discovery bind failure metadata."""
        self.emit("udp_discovery_disabled", error=error)

    def udp_response_failed(self, error: str) -> None:
        """Writes UDP discovery response failure metadata."""
        self.emit("udp_discovery_response_failed", error=error)

    def upload_accepted(
        self,
        *,
        source: str,
        byte_count: int,
        device_id: str,
        sequence: int,
        duplicate: bool,
    ) -> None:
        """Writes bounded upload acceptance metadata without dumping payloads."""
        self.emit(
            "upload_accepted",
            source=source,
            bytes=byte_count,
            device_id=device_id,
            sequence=sequence,
            duplicate=duplicate,
        )

    def upload_rejected(
        self,
        *,
        source: str,
        byte_count: int,
        device_id: str,
        sequence: int,
        status_code: int,
        duplicate: bool,
        conflict: bool,
        reason: str,
    ) -> None:
        """Writes bounded upload rejection metadata without dumping payloads."""
        self.emit(
            "upload_rejected",
            source=source,
            bytes=byte_count,
            device_id=device_id,
            sequence=sequence,
            status_code=status_code,
            duplicate=duplicate,
            conflict=conflict,
            reason=reason,
        )

    def storage_reconciled(self, *, copied: int) -> None:
        """Writes one storage reconciliation event."""
        self.emit("storage_reconciled", copied=copied)

    def measurement_dashboard(
        self,
        *,
        device_id: str,
        sequence: int,
        temperature_c: float | None,
        humidity_percent: float | None,
        lux: float | None,
        mic_db_rel: float,
        duplicate: bool,
    ) -> None:
        """Ignores live chart updates for the scriptable service output."""

    def shutdown_requested(self) -> None:
        """Writes interrupt-driven shutdown metadata."""
        self.emit("shutdown_requested")

    def stopped(self) -> None:
        """Writes server stopped metadata."""
        self.emit("server_stopped")

    def config_ok(self, config: ServerConfig) -> None:
        """Writes validated configuration metadata."""
        self.emit(
            "config_ok",
            host=config.host,
            port=config.port,
            udp_discovery_port=config.udp_discovery_port,
            api_base=config.api_base,
        )

    def discovery_snapshot(
        self,
        *,
        document: DiscoveryDocument,
        udp_query: str,
        udp_response: UdpDiscoveryPayload,
    ) -> None:
        """Writes discovery document and UDP response metadata."""
        if self.mode == "json":
            self._write_json_line(
                {
                    "event": "discovery_snapshot",
                    "document": document.model_dump(),
                    "udp_query": udp_query,
                    "udp_response": udp_response.model_dump(),
                }
            )
            return

        if self.mode == "rich":
            table = Table(title="Discovery", show_header=True, header_style="bold")
            table.add_column("Field")
            table.add_column("Value")
            for key, value in document.model_dump().items():
                table.add_row(f"document.{key}", str(value))
            table.add_row("udp_query", udp_query)
            for key, value in udp_response.model_dump().items():
                table.add_row(f"udp_response.{key}", str(value))
            self._console.print(table)
            return

        self.stream.write("discovery document\n")
        for key, value in document.model_dump().items():
            self.stream.write(f"document.{key}={value}\n")
        self.stream.write(f"udp_query={udp_query}\n")
        for key, value in udp_response.model_dump().items():
            self.stream.write(f"udp_response.{key}={value}\n")
        self.stream.flush()

    def history_snapshot(
        self,
        *,
        summary: dict[str, Any],
        records: list[dict[str, Any]],
        trends: dict[str, str],
    ) -> None:
        """Writes local history summary, recent rows, and metric trends."""
        if self.mode == "json":
            self._write_json_line(
                {
                    "event": "history_snapshot",
                    "summary": summary,
                    "records": records,
                    "trends": trends,
                }
            )
            return

        if self.mode == "rich":
            self._print_history_rich(summary=summary, records=records, trends=trends)
            return

        self.stream.write("history summary\n")
        self.stream.write(f"count={summary['count']}\n")
        self.stream.write(f"devices={','.join(summary['devices'])}\n")
        self.stream.write(f"first_received_unix_ms={summary['first_received_unix_ms']}\n")
        self.stream.write(f"last_received_unix_ms={summary['last_received_unix_ms']}\n")
        for key, value in summary["averages"].items():
            self.stream.write(f"average.{key}={value:.3f}\n")
        self.stream.write("recent measurements\n")
        for record in records:
            payload = record["payload"]
            self.stream.write(
                " ".join(
                    (
                        f"display_unix_ms={record['display_unix_ms']}",
                        f"device_id={payload['device_id']}",
                        f"sequence={payload['sequence']}",
                        f"temperature_c={payload['temperature_c']}",
                        f"humidity_percent={payload['humidity_percent']}",
                        f"lux={payload['lux']}",
                        f"mic_db_rel={payload['mic_db_rel']}",
                    )
                )
                + "\n"
            )
        self.stream.write("metric trends\n")
        for metric, trend in trends.items():
            self.stream.write(f"{metric}={trend}\n")
        self.stream.flush()

    def emit(self, event: str, **fields: Any) -> None:
        """Writes one event in the selected output mode."""
        if self.mode == "json":
            self._write_json_line({"event": event, **fields})
            return
        if self.mode == "rich":
            parts = " ".join(f"{key}={value}" for key, value in fields.items())
            self._console.print(f"{event} {parts}".rstrip())
            return

        parts = " ".join(f"{key}={value}" for key, value in fields.items())
        self.stream.write(f"{event} {parts}".rstrip() + "\n")
        self.stream.flush()

    def _write_json_line(self, payload: dict[str, Any]) -> None:
        """Writes one compact JSON object followed by a newline."""
        self.stream.write(json.dumps(payload, separators=(",", ":"), sort_keys=True))
        self.stream.write("\n")
        self.stream.flush()

    def _print_history_rich(
        self,
        *,
        summary: dict[str, Any],
        records: list[dict[str, Any]],
        trends: dict[str, str],
    ) -> None:
        """Writes Rich tables for local history output."""
        summary_table = Table(title="History Summary", show_header=True, header_style="bold")
        summary_table.add_column("Field")
        summary_table.add_column("Value")
        summary_table.add_row("count", str(summary["count"]))
        summary_table.add_row("devices", ", ".join(summary["devices"]))
        summary_table.add_row("first_received_unix_ms", str(summary["first_received_unix_ms"]))
        summary_table.add_row("last_received_unix_ms", str(summary["last_received_unix_ms"]))
        for key, value in summary["averages"].items():
            summary_table.add_row(f"average.{key}", f"{value:.3f}")
        self._console.print(summary_table)

        records_table = Table(title="Recent Measurements", show_header=True, header_style="bold")
        records_table.add_column("Display ms")
        records_table.add_column("Device")
        records_table.add_column("Seq", justify="right")
        records_table.add_column("Temp", justify="right")
        records_table.add_column("RH", justify="right")
        records_table.add_column("Lux", justify="right")
        records_table.add_column("dB", justify="right")
        for record in records:
            payload = record["payload"]
            records_table.add_row(
                str(record["display_unix_ms"]),
                str(payload["device_id"]),
                str(payload["sequence"]),
                _format_optional_number(payload["temperature_c"]),
                _format_optional_number(payload["humidity_percent"]),
                _format_optional_number(payload["lux"]),
                _format_optional_number(payload["mic_db_rel"]),
            )
        self._console.print(records_table)

        trends_table = Table(title="Metric Trends", show_header=True, header_style="bold")
        trends_table.add_column("Metric")
        trends_table.add_column("Trend")
        for metric, trend in trends.items():
            trends_table.add_row(metric, trend)
        self._console.print(trends_table)


class NullOutput:
    """No-op output sink for tests or callers that do not want diagnostics."""

    def upload_accepted(
        self,
        *,
        source: str,
        byte_count: int,
        device_id: str,
        sequence: int,
        duplicate: bool,
    ) -> None:
        """Ignores upload acceptance metadata."""

    def upload_rejected(
        self,
        *,
        source: str,
        byte_count: int,
        device_id: str,
        sequence: int,
        status_code: int,
        duplicate: bool,
        conflict: bool,
        reason: str,
    ) -> None:
        """Ignores upload rejection metadata."""

    def measurement_dashboard(
        self,
        *,
        device_id: str,
        sequence: int,
        temperature_c: float | None,
        humidity_percent: float | None,
        lux: float | None,
        mic_db_rel: float,
        duplicate: bool,
    ) -> None:
        """Ignores dashboard measurement updates."""


def _format_optional_number(value: object) -> str:
    """Formats optional numeric values for tables."""
    if value is None:
        return ""
    if isinstance(value, int | float):
        return f"{value:.2f}"
    return str(value)
