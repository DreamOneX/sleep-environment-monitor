"""Textual terminal UI for local server operation."""

from __future__ import annotations

import queue
from collections import deque
from collections.abc import Callable
from dataclasses import dataclass
from typing import Any, Protocol

from textual.app import App, ComposeResult
from textual.containers import Horizontal, Vertical
from textual.widgets import DataTable, Footer, Header, RichLog, Static

from sleep_env_server.config import AppConfig
from sleep_env_server.output import ServerOutput
from sleep_env_server.runtime import start_server_runtime


@dataclass(frozen=True)
class ServerEvent:
    """One bounded server event delivered to the TUI."""

    name: str
    fields: dict[str, Any]


class RuntimeHandle(Protocol):
    """Small lifecycle surface needed by the TUI app."""

    def stop(self) -> None:
        """Stops the runtime."""


class TuiEventOutput(ServerOutput):
    """Server output adapter that writes diagnostics into a thread-safe queue."""

    def __init__(self, events: queue.Queue[ServerEvent]) -> None:
        """Initializes the queue-backed output sink."""
        super().__init__("plain")
        self._events = events

    def emit(self, event: str, **fields: Any) -> None:
        """Queues one bounded server event for the Textual app thread."""
        self._events.put(ServerEvent(event, fields))

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
        """Queues one accepted measurement for TUI table and trend updates."""
        self.emit(
            "measurement",
            device_id=device_id,
            sequence=sequence,
            temperature_c=temperature_c,
            humidity_percent=humidity_percent,
            lux=lux,
            mic_db_rel=mic_db_rel,
            duplicate=duplicate,
        )


class ServerTuiApp(App[None]):
    """Full-screen local operator UI for the ingestion server."""

    CSS = """
    Screen {
        layout: vertical;
    }

    #status {
        height: 3;
        padding: 0 1;
        border: solid $accent;
    }

    #main {
        height: 1fr;
    }

    #measurements-panel {
        width: 2fr;
    }

    #side-panel {
        width: 1fr;
    }

    DataTable, RichLog {
        height: 1fr;
        border: solid $primary;
    }
    """

    BINDINGS = [
        ("q", "quit", "Quit"),
        ("ctrl+c", "quit", "Quit"),
        ("c", "clear_events", "Clear events"),
        ("r", "refresh", "Refresh"),
    ]

    def __init__(
        self,
        app_config: AppConfig,
        *,
        start_runtime: bool = True,
        event_queue: queue.Queue[ServerEvent] | None = None,
        runtime_starter: Callable[[AppConfig, TuiEventOutput], RuntimeHandle] = start_server_runtime,
    ) -> None:
        """Initializes the TUI with server endpoint metadata."""
        super().__init__()
        self.app_config = app_config
        self.config = app_config.server
        self.start_runtime = start_runtime
        self.event_queue = event_queue if event_queue is not None else queue.Queue()
        self.output = TuiEventOutput(self.event_queue)
        self.runtime_starter = runtime_starter
        self.runtime: RuntimeHandle | None = None
        self._recent_measurements: deque[dict[str, Any]] = deque(maxlen=50)

    def compose(self) -> ComposeResult:
        """Builds the static TUI layout."""
        yield Header(show_clock=True)
        yield Static(self._status_text(), id="status")
        with Horizontal(id="main"):
            with Vertical(id="measurements-panel"):
                yield DataTable(id="measurements")
                yield RichLog(id="events", highlight=False, markup=False, wrap=True)
            with Vertical(id="side-panel"):
                yield DataTable(id="trends")
                yield RichLog(id="help", highlight=False, markup=False, wrap=True)
        yield Footer()

    def on_mount(self) -> None:
        """Initializes table headers and static operator hints."""
        measurements = self.query_one("#measurements", DataTable)
        measurements.cursor_type = "row"
        measurements.add_columns("Device", "Seq", "Temp", "RH", "Lux", "dB", "Dup")

        trends = self.query_one("#trends", DataTable)
        trends.cursor_type = "row"
        trends.add_columns("Metric", "Trend")
        for metric in ("temperature_c", "humidity_percent", "lux", "mic_db_rel"):
            trends.add_row(metric, "")

        events = self.query_one("#events", RichLog)
        events.write("server_tui_ready")

        help_log = self.query_one("#help", RichLog)
        help_log.write("q / Ctrl+C: quit")
        help_log.write("c: clear events")
        help_log.write("r: refresh")
        self.set_interval(0.1, self.drain_events)
        if self.start_runtime:
            self.runtime = self.runtime_starter(self.app_config, self.output)

    def action_clear_events(self) -> None:
        """Clears the bounded event panel."""
        self.query_one("#events", RichLog).clear()

    def action_refresh(self) -> None:
        """Records a manual refresh request in the event panel."""
        self.query_one("#events", RichLog).write("refresh_requested")

    def on_unmount(self) -> None:
        """Stops the service runtime when the TUI exits."""
        if self.runtime is not None:
            self.output.shutdown_requested()
            self.runtime.stop()
            self.output.stopped()

    def drain_events(self) -> None:
        """Consumes queued server events and refreshes visible widgets."""
        for _ in range(100):
            try:
                event = self.event_queue.get_nowait()
            except queue.Empty:
                break
            self._apply_event(event)

    def _status_text(self) -> str:
        """Returns the one-line service status summary."""
        return (
            f"HTTP {self.config.host}:{self.config.port} | "
            f"UDP discovery {self.config.udp_discovery_port} | "
            f"API {self.config.api_base}"
        )

    def _apply_event(self, event: ServerEvent) -> None:
        """Applies one server event to logs and measurement/trend widgets."""
        self.query_one("#events", RichLog).write(_format_event(event))
        if event.name != "measurement":
            return

        self._recent_measurements.append(event.fields)
        measurements = self.query_one("#measurements", DataTable)
        measurements.clear()
        for item in list(self._recent_measurements)[-20:]:
            measurements.add_row(
                str(item.get("device_id", "")),
                str(item.get("sequence", "")),
                _format_optional_number(item.get("temperature_c")),
                _format_optional_number(item.get("humidity_percent")),
                _format_optional_number(item.get("lux")),
                _format_optional_number(item.get("mic_db_rel")),
                str(item.get("duplicate", "")),
            )

        trends = self.query_one("#trends", DataTable)
        trends.clear()
        for metric in ("temperature_c", "humidity_percent", "lux", "mic_db_rel"):
            values = [
                float(value)
                for item in self._recent_measurements
                if (value := item.get(metric)) is not None
            ]
            trends.add_row(metric, _metric_trend(values[-24:]))


def _format_event(event: ServerEvent) -> str:
    """Formats one bounded event for the TUI log panel."""
    parts = " ".join(f"{key}={value}" for key, value in event.fields.items())
    return f"{event.name} {parts}".rstrip()


def _format_optional_number(value: object) -> str:
    """Formats optional numeric table cells."""
    if value is None:
        return ""
    if isinstance(value, int | float):
        return f"{value:.2f}"
    return str(value)


def _metric_trend(values: list[float]) -> str:
    """Builds one compact ASCII trend line."""
    if not values:
        return ""
    if len(values) == 1:
        return f"{values[0]:.2f}"
    minimum = min(values)
    maximum = max(values)
    if minimum == maximum:
        return f"{'=' * len(values)} {minimum:.2f}"
    ramp = " .:-=+*#%@"
    span = maximum - minimum
    chars = [ramp[round((value - minimum) / span * (len(ramp) - 1))] for value in values]
    return f"{''.join(chars)} {minimum:.2f}..{maximum:.2f}"
