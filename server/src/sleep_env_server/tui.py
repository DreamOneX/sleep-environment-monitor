"""Textual terminal UI for local server operation."""

from __future__ import annotations

import queue
from collections import deque
from collections.abc import Callable
from dataclasses import dataclass
from typing import Any, Protocol

from textual.app import App, ComposeResult
from textual.color import Color
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
        content-align: left middle;
    }

    #metrics {
        height: 4;
        padding: 0 1;
    }

    .metric {
        width: 1fr;
        height: 3;
        margin-right: 1;
        padding: 0 1;
        content-align: left middle;
    }

    #main {
        height: 1fr;
        padding: 0 1;
    }

    #measurements-panel {
        width: 2fr;
        margin-right: 1;
    }

    #side-panel {
        width: 1fr;
    }

    .panel-title {
        height: 1;
        text-style: bold;
    }

    DataTable {
        height: 1fr;
    }

    #events {
        height: 7;
        margin: 0 1;
    }

    #help-panel {
        height: 3;
        padding: 0 1;
        content-align: left middle;
    }

    Screen.theme_catppuccin_mocha {
        background: #1e1e2e;
        color: #cdd6f4;
    }

    Screen.theme_catppuccin_mocha Header,
    Screen.theme_catppuccin_mocha Footer {
        background: #181825;
        color: #cdd6f4;
    }

    Screen.theme_catppuccin_mocha #status {
        background: #181825;
        border-bottom: solid #45475a;
        color: #cdd6f4;
    }

    Screen.theme_catppuccin_mocha #metrics,
    Screen.theme_catppuccin_mocha #main,
    Screen.theme_catppuccin_mocha .panel-title,
    Screen.theme_catppuccin_mocha #help-panel {
        background: #1e1e2e;
    }

    Screen.theme_catppuccin_mocha .metric {
        background: #313244;
        border: solid #45475a;
        color: #cdd6f4;
    }

    Screen.theme_catppuccin_mocha #metric-temperature {
        border: solid #89dceb;
    }

    Screen.theme_catppuccin_mocha #metric-humidity {
        border: solid #a6e3a1;
    }

    Screen.theme_catppuccin_mocha #metric-lux {
        border: solid #f9e2af;
    }

    Screen.theme_catppuccin_mocha #metric-sound {
        border: solid #f38ba8;
    }

    Screen.theme_catppuccin_mocha .panel-title {
        color: #bac2de;
    }

    Screen.theme_catppuccin_mocha DataTable,
    Screen.theme_catppuccin_mocha #events {
        background: #181825;
        border: solid #45475a;
        color: #cdd6f4;
    }

    Screen.theme_catppuccin_mocha #help-panel {
        border-top: solid #45475a;
        color: #a6adc8;
    }

    Screen.theme_graphite {
        background: #0b0f14;
        color: #d7e0ea;
    }

    Screen.theme_graphite Header,
    Screen.theme_graphite Footer {
        background: #101820;
        color: #d7e0ea;
    }

    Screen.theme_graphite #status {
        background: #101820;
        border-bottom: solid #243244;
        color: #d7e0ea;
    }

    Screen.theme_graphite #metrics,
    Screen.theme_graphite #main,
    Screen.theme_graphite .panel-title,
    Screen.theme_graphite #help-panel {
        background: #0b0f14;
    }

    Screen.theme_graphite .metric {
        background: #111827;
        border: solid #243244;
        color: #d7e0ea;
    }

    Screen.theme_graphite #metric-temperature {
        border: solid #22d3ee;
    }

    Screen.theme_graphite #metric-humidity {
        border: solid #10b981;
    }

    Screen.theme_graphite #metric-lux {
        border: solid #f59e0b;
    }

    Screen.theme_graphite #metric-sound {
        border: solid #f43f5e;
    }

    Screen.theme_graphite .panel-title {
        color: #94a3b8;
    }

    Screen.theme_graphite DataTable,
    Screen.theme_graphite #events {
        background: #111827;
        border: solid #243244;
        color: #d7e0ea;
    }

    Screen.theme_graphite #help-panel {
        color: #94a3b8;
        border-top: solid #243244;
    }

    Screen.transparent,
    Screen.transparent Header,
    Screen.transparent Footer,
    Screen.transparent FooterLabel,
    Screen.transparent FooterKey,
    Screen.transparent HorizontalGroup,
    Screen.transparent KeyGroup,
    Screen.transparent #status,
    Screen.transparent #metrics,
    Screen.transparent #main,
    Screen.transparent #events,
    Screen.transparent #help-panel,
    Screen.transparent .metric,
    Screen.transparent .panel-title,
    Screen.transparent DataTable {
        background: transparent;
    }

    Screen.transparent RichLog,
    Screen.transparent DataTable {
        background-tint: transparent;
    }

    Screen.transparent DataTable > .datatable--header,
    Screen.transparent DataTable > .datatable--fixed,
    Screen.transparent DataTable > .datatable--odd-row,
    Screen.transparent DataTable > .datatable--even-row,
    Screen.transparent DataTable > .datatable--cursor,
    Screen.transparent DataTable > .datatable--fixed-cursor,
    Screen.transparent DataTable > .datatable--header-cursor,
    Screen.transparent DataTable > .datatable--header-hover,
    Screen.transparent DataTable > .datatable--hover {
        background: transparent;
        background-tint: transparent;
    }
    """

    BINDINGS = [
        ("q", "quit", "Quit"),
        ("ctrl+c", "quit", "Quit"),
        ("c", "clear_events", "Clear events"),
        ("r", "refresh", "Refresh"),
        ("?", "toggle_help", "Help"),
    ]

    def __init__(
        self,
        app_config: AppConfig,
        *,
        start_runtime: bool = True,
        event_queue: queue.Queue[ServerEvent] | None = None,
        runtime_starter: Callable[
            [AppConfig, TuiEventOutput], RuntimeHandle
        ] = start_server_runtime,
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
        self._help_expanded = False

    def compose(self) -> ComposeResult:
        """Builds the static TUI layout."""
        yield Header(show_clock=True)
        yield Static(self._status_text(), id="status")
        with Horizontal(id="metrics"):
            yield Static(_metric_card("TEMP", None, "C"), id="metric-temperature", classes="metric")
            yield Static(
                _metric_card("HUMIDITY", None, "%"), id="metric-humidity", classes="metric"
            )
            yield Static(_metric_card("LIGHT", None, "lx"), id="metric-lux", classes="metric")
            yield Static(_metric_card("SOUND", None, "dB"), id="metric-sound", classes="metric")
        with Horizontal(id="main"):
            with Vertical(id="measurements-panel"):
                yield Static("MEASUREMENTS", classes="panel-title")
                yield DataTable(id="measurements")
            with Vertical(id="side-panel"):
                yield Static("TRENDS", classes="panel-title")
                yield DataTable(id="trends")
        yield Static("EVENTS", classes="panel-title")
        yield RichLog(id="events", highlight=False, markup=False, wrap=True)
        yield Static(self._help_text(), id="help-panel")
        yield Footer()

    def on_mount(self) -> None:
        """Initializes table headers and static operator hints."""
        self.set_class(self.app_config.tui.transparent, "transparent")
        if self.app_config.tui.transparent:
            self.styles.background = Color.parse("transparent")
        self.screen.set_class(
            self.app_config.tui.theme == "catppuccin-mocha",
            "theme_catppuccin_mocha",
        )
        self.screen.set_class(self.app_config.tui.theme == "graphite", "theme_graphite")
        self.screen.set_class(self.app_config.tui.transparent, "transparent")

        measurements = self.query_one("#measurements", DataTable)
        measurements.cursor_type = "row"
        measurements.add_columns("Device", "Seq", "Temp", "RH", "Lux", "dB", "Dup")

        trends = self.query_one("#trends", DataTable)
        trends.cursor_type = "row"
        trends.add_columns("Metric", "Trend")
        for metric in ("temperature_c", "humidity_percent", "lux", "mic_db_rel"):
            trends.add_row(metric, "")

        events = self.query_one("#events", RichLog)
        events.write("server ready")

        self.set_interval(0.1, self.drain_events)
        if self.start_runtime:
            self.runtime = self.runtime_starter(self.app_config, self.output)

    def action_clear_events(self) -> None:
        """Clears the bounded event panel."""
        self.query_one("#events", RichLog).clear()

    def action_refresh(self) -> None:
        """Records a manual refresh request in the event panel."""
        self.query_one("#events", RichLog).write("manual refresh requested")

    def action_toggle_help(self) -> None:
        """Toggles expanded operator help."""
        self._help_expanded = not self._help_expanded
        self.query_one("#help-panel", Static).update(self._help_text())

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
        transparent = "transparent" if self.app_config.tui.transparent else "solid"
        return (
            f"HTTP {self.config.host}:{self.config.port} | "
            f"UDP discovery {self.config.udp_discovery_port} | "
            f"API {self.config.api_base} | "
            f"theme {self.app_config.tui.theme}/{transparent}"
        )

    def _apply_event(self, event: ServerEvent) -> None:
        """Applies one server event to logs and measurement/trend widgets."""
        self.query_one("#events", RichLog).write(_format_event(event))
        if event.name != "measurement":
            return

        self._recent_measurements.append(event.fields)
        self._update_metric_cards(event.fields)
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

    def _help_text(self) -> str:
        """Returns compact or expanded operator help."""
        if self._help_expanded:
            return (
                "q quit | Ctrl+C quit | c clear event log | r record refresh request | "
                "? collapse help | serve remains the scriptable log mode"
            )
        return "q quit | c clear events | r refresh | ? help"

    def _update_metric_cards(self, item: dict[str, Any]) -> None:
        """Updates the top metric strip from the latest accepted measurement."""
        self.query_one("#metric-temperature", Static).update(
            _metric_card("TEMP", item.get("temperature_c"), "C")
        )
        self.query_one("#metric-humidity", Static).update(
            _metric_card("HUMIDITY", item.get("humidity_percent"), "%")
        )
        self.query_one("#metric-lux", Static).update(_metric_card("LIGHT", item.get("lux"), "lx"))
        self.query_one("#metric-sound", Static).update(
            _metric_card("SOUND", item.get("mic_db_rel"), "dB")
        )


def _format_event(event: ServerEvent) -> str:
    """Formats one bounded event for the TUI log panel."""
    if event.name == "measurement":
        return (
            "measurement "
            f"{event.fields.get('device_id', '')} "
            f"seq {event.fields.get('sequence', '')} "
            f"temp {_format_optional_number(event.fields.get('temperature_c'))} "
            f"rh {_format_optional_number(event.fields.get('humidity_percent'))} "
            f"lux {_format_optional_number(event.fields.get('lux'))}"
        )
    if event.name == "upload_accepted":
        duplicate = " duplicate" if event.fields.get("duplicate") else ""
        return (
            "accepted "
            f"{event.fields.get('device_id', '')} "
            f"seq {event.fields.get('sequence', '')}{duplicate}"
        )
    if event.name == "upload_rejected":
        return (
            "rejected "
            f"{event.fields.get('device_id', '')} "
            f"seq {event.fields.get('sequence', '')} "
            f"status {event.fields.get('status_code', '')}"
        )
    if event.name == "udp_discovery_started":
        return f"udp discovery listening on {event.fields.get('udp_discovery_port', '')}"
    if event.name == "udp_discovery_disabled":
        return f"udp discovery disabled: {event.fields.get('error', '')}"
    if event.name == "storage_reconciled":
        return f"storage reconciled, copied {event.fields.get('copied', 0)} records"
    if event.name == "shutdown_requested":
        return "shutdown requested"
    if event.name == "server_stopped":
        return "server stopped"
    if event.fields:
        parts = " ".join(f"{key}={value}" for key, value in event.fields.items())
        return f"{event.name} {parts}".rstrip()
    return event.name


def _metric_card(label: str, value: object, unit: str) -> str:
    """Formats one top-strip metric card."""
    formatted = "--" if value is None else _format_optional_number(value)
    return f"{label}\n{formatted} {unit}"


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
