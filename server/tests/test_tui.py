from __future__ import annotations

import asyncio
import queue

from textual.command import CommandPalette
from textual.widgets import DataTable, RichLog, Static

from sleep_env_server.config import AppConfig, ServerConfig, TuiConfig
from sleep_env_server.tui import ServerEvent, ServerTuiApp, TuiEventOutput


class FakeRuntime:
    def __init__(self) -> None:
        self.stopped = False

    def stop(self) -> None:
        self.stopped = True


def test_server_tui_app_smoke_startup_and_quit() -> None:
    async def run() -> None:
        app_config = AppConfig(server=ServerConfig(host="127.0.0.1", port=8081))
        app = ServerTuiApp(app_config, start_runtime=False)
        async with app.run_test() as pilot:
            status = app.query_one("#status", Static)
            assert "127.0.0.1:8081" in str(status.content)
            assert "Service STOPPED" in str(status.content)
            assert "theme catppuccin-mocha/solid" in str(status.content)
            assert app.screen.has_class("theme_catppuccin_mocha")
            assert len(app.query_one("#measurements", DataTable).ordered_columns) == 7
            assert app.query_one("#events", RichLog) is not None
            assert app.query_one("#help-panel", Static) is not None
            await pilot.press("q")

    asyncio.run(run())


def test_server_tui_app_transparent_mode_class() -> None:
    async def run() -> None:
        app_config = AppConfig(tui=AppConfig().tui.__class__(transparent=True))
        app = ServerTuiApp(app_config, start_runtime=False)
        async with app.run_test():
            sequence = app.screen._compositor.render_update(full=True).render_segments(app.console)
            assert app.native_ansi_color is True
            assert app.has_class("transparent")
            assert app.styles.background.ansi == -1
            assert app.screen.has_class("transparent")
            assert app.screen.styles.background.ansi == -1
            assert app.query_one("#status", Static).styles.background.ansi == -1
            assert app.query_one("#measurements", DataTable).styles.background.ansi == -1
            assert app.query_one("#events", RichLog).styles.background.ansi == -1
            assert "transparent" in str(app.query_one("#status", Static).content)
            assert "\x1b[40m" not in sequence
            assert "48;2;12;12;12" not in sequence
            assert "48;2;0;0;0" not in sequence
            assert "48;" not in sequence
            assert "49m" in sequence

    asyncio.run(run())


def test_server_tui_app_help_toggle() -> None:
    async def run() -> None:
        app = ServerTuiApp(AppConfig(), start_runtime=False)
        async with app.run_test() as pilot:
            help_panel = app.query_one("#help-panel", Static)
            assert "q quit" in str(help_panel.content)
            assert "s start/stop" in str(help_panel.content)
            await pilot.press("?")
            assert "s start/stop service" in str(help_panel.content)
            assert "collapse help" in str(help_panel.content)

    asyncio.run(run())


def test_server_tui_command_palette_uses_catppuccin_theme() -> None:
    async def run() -> None:
        app = ServerTuiApp(AppConfig(), start_runtime=False)
        async with app.run_test(size=(100, 30)) as pilot:
            assert app.theme == "catppuccin-mocha"
            await pilot.press("ctrl+p")
            await pilot.pause()

            assert isinstance(app.screen, CommandPalette)
            assert app.screen.has_class("theme_catppuccin_mocha")
            assert app.screen.styles.background.rgb == (30, 30, 46)
            assert app.screen.styles.color.rgb == (205, 214, 244)

            container = app.screen.query_one("#--container")
            command_input_shell = app.screen.query_one("#--input")
            command_input = app.screen.query_one("CommandInput")
            command_list = app.screen.query_one("CommandList")
            results = app.screen.query_one("#--results")
            search_icon = app.screen.query_one("SearchIcon")
            loading_indicator = app.screen.query_one("LoadingIndicator")

            assert container.styles.background.rgb == (24, 24, 37)
            assert container.styles.color.rgb == (205, 214, 244)
            assert command_input_shell.styles.background.rgb == (30, 30, 46)
            assert command_input_shell.styles.color.rgb == (205, 214, 244)
            assert command_input.styles.color.rgb == (205, 214, 244)
            assert results.styles.background.rgb == (24, 24, 37)
            assert results.styles.color.rgb == (205, 214, 244)
            assert command_list.styles.background.rgb == (24, 24, 37)
            assert command_list.styles.color.rgb == (205, 214, 244)
            assert search_icon.styles.color.rgb == (137, 180, 250)
            assert loading_indicator.styles.color.rgb == (137, 180, 250)
            assert str(command_input.get_component_rich_style("input--cursor")) == (
                "#11111b on #f5e0dc"
            )
            assert str(command_input.get_component_rich_style("input--placeholder")) == (
                "#6c7086 on #1e1e2e"
            )
            assert str(
                command_list.get_component_rich_style("option-list--option-highlighted")
            ) == ("bold #f5e0dc on #45475a")

    asyncio.run(run())


def test_server_tui_transparent_command_palette_uses_default_background() -> None:
    async def run() -> None:
        app_config = AppConfig(tui=AppConfig().tui.__class__(transparent=True))
        app = ServerTuiApp(app_config, start_runtime=False)
        async with app.run_test(size=(100, 30)) as pilot:
            await pilot.press("ctrl+p")
            await pilot.pause()

            assert isinstance(app.screen, CommandPalette)
            assert app.screen.has_class("theme_catppuccin_mocha")
            assert app.screen.has_class("transparent")
            for selector in (
                "#--container",
                "#--input",
                "#--results",
                "CommandInput",
                "CommandList",
            ):
                assert app.screen.query_one(selector).styles.background.ansi == -1

            sequence = app.screen._compositor.render_update(full=True).render_segments(app.console)
            assert "\x1b[40m" not in sequence
            assert "48;" not in sequence
            assert "49m" in sequence

    asyncio.run(run())


def test_tui_event_output_queues_server_events() -> None:
    events: queue.Queue[ServerEvent] = queue.Queue()
    output = TuiEventOutput(events)

    output.upload_accepted(
        source="10.0.0.2",
        byte_count=300,
        device_id="device-1",
        sequence=42,
        duplicate=False,
    )
    event = events.get_nowait()

    assert event.name == "upload_accepted"
    assert event.fields["source"] == "10.0.0.2"
    assert event.fields["sequence"] == 42
    assert "temperature_c" not in event.fields


def test_tui_event_output_queues_measurement_events() -> None:
    events: queue.Queue[ServerEvent] = queue.Queue()
    output = TuiEventOutput(events)

    output.measurement_dashboard(
        device_id="device-1",
        sequence=42,
        temperature_c=21.5,
        humidity_percent=45.0,
        lux=12.0,
        mic_db_rel=20.0,
        duplicate=False,
    )
    event = events.get_nowait()

    assert event.name == "measurement"
    assert event.fields["temperature_c"] == 21.5


def test_tui_event_output_queues_storage_udp_and_shutdown_events() -> None:
    events: queue.Queue[ServerEvent] = queue.Queue()
    output = TuiEventOutput(events)

    output.storage_reconciled(copied=2)
    output.udp_started(ServerConfig(host="127.0.0.1", udp_discovery_port=39023))
    output.udp_disabled("address already in use")
    output.shutdown_requested()
    output.stopped()

    queued = [events.get_nowait() for _ in range(5)]
    assert [event.name for event in queued] == [
        "storage_reconciled",
        "udp_discovery_started",
        "udp_discovery_disabled",
        "shutdown_requested",
        "server_stopped",
    ]
    assert queued[0].fields["copied"] == 2
    assert queued[1].fields["udp_discovery_port"] == 39023
    assert queued[2].fields["error"] == "address already in use"


def test_server_tui_app_drains_measurement_events() -> None:
    async def run() -> None:
        events: queue.Queue[ServerEvent] = queue.Queue()
        app = ServerTuiApp(AppConfig(), start_runtime=False, event_queue=events)
        async with app.run_test():
            events.put(
                ServerEvent(
                    "measurement",
                    {
                        "device_id": "device-1",
                        "sequence": 42,
                        "duplicate": False,
                        "temperature_c": 21.5,
                        "humidity_percent": 45.0,
                        "lux": 12.0,
                        "mic_db_rel": 20.0,
                    },
                )
            )
            app.drain_events()
            measurements = app.query_one("#measurements", DataTable)
            assert measurements.row_count == 1
            assert app.query_one("#trends", DataTable).row_count == 4
            assert "21.50 C" in str(app.query_one("#metric-temperature", Static).content)

    asyncio.run(run())


def test_server_tui_app_graphite_theme_class() -> None:
    async def run() -> None:
        app_config = AppConfig(tui=AppConfig().tui.__class__(theme="graphite"))
        app = ServerTuiApp(app_config, start_runtime=False)
        async with app.run_test():
            assert app.screen.has_class("theme_graphite")
            assert "theme graphite/solid" in str(app.query_one("#status", Static).content)

    asyncio.run(run())


def test_server_tui_app_uses_configured_no_autostart() -> None:
    async def run() -> None:
        started: list[AppConfig] = []

        def starter(app_config: AppConfig, output: TuiEventOutput) -> FakeRuntime:
            started.append(app_config)
            return FakeRuntime()

        app_config = AppConfig(tui=TuiConfig(autostart=False))
        app = ServerTuiApp(app_config, runtime_starter=starter)
        async with app.run_test():
            assert started == []
            assert app.runtime is None
            assert "Service STOPPED" in str(app.query_one("#status", Static).content)

    asyncio.run(run())


def test_server_tui_app_starts_and_stops_runtime_without_network() -> None:
    async def run() -> None:
        fake = FakeRuntime()
        started: list[AppConfig] = []

        def starter(app_config: AppConfig, output: TuiEventOutput) -> FakeRuntime:
            started.append(app_config)
            output.startup(app_config.server, app_config.server.log_level)
            return fake

        app_config = AppConfig(server=ServerConfig(host="127.0.0.1", port=8082))
        app = ServerTuiApp(app_config, runtime_starter=starter)
        async with app.run_test():
            assert started == [app_config]
            assert "Service RUNNING" in str(app.query_one("#status", Static).content)
            app.drain_events()

        assert fake.stopped is True

    asyncio.run(run())


def test_server_tui_app_toggles_runtime_with_keyboard() -> None:
    async def run() -> None:
        fake = FakeRuntime()
        started: list[AppConfig] = []

        def starter(app_config: AppConfig, output: TuiEventOutput) -> FakeRuntime:
            started.append(app_config)
            output.startup(app_config.server, app_config.server.log_level)
            return fake

        app_config = AppConfig(tui=TuiConfig(autostart=False))
        app = ServerTuiApp(app_config, runtime_starter=starter)
        async with app.run_test() as pilot:
            await pilot.press("s")
            await pilot.pause()

            assert started == [app_config]
            assert app.runtime is fake
            assert "Service RUNNING" in str(app.query_one("#status", Static).content)

            await pilot.press("s")
            await pilot.pause()

            assert fake.stopped is True
            assert app.runtime is None
            assert "Service STOPPED" in str(app.query_one("#status", Static).content)

    asyncio.run(run())
