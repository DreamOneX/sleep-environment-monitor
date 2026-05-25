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
