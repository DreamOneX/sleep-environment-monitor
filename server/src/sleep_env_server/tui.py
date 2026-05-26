"""Textual terminal UI for local server operation."""

from __future__ import annotations

from textual.app import App, ComposeResult
from textual.containers import Horizontal, Vertical
from textual.widgets import DataTable, Footer, Header, RichLog, Static

from sleep_env_server.config import ServerConfig


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

    def __init__(self, config: ServerConfig) -> None:
        """Initializes the TUI with server endpoint metadata."""
        super().__init__()
        self.config = config

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

    def action_clear_events(self) -> None:
        """Clears the bounded event panel."""
        self.query_one("#events", RichLog).clear()

    def action_refresh(self) -> None:
        """Records a manual refresh request in the event panel."""
        self.query_one("#events", RichLog).write("refresh_requested")

    def _status_text(self) -> str:
        """Returns the one-line service status summary."""
        return (
            f"HTTP {self.config.host}:{self.config.port} | "
            f"UDP discovery {self.config.udp_discovery_port} | "
            f"API {self.config.api_base}"
        )
