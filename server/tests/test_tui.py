from __future__ import annotations

import asyncio

from textual.widgets import DataTable, RichLog, Static

from sleep_env_server.config import ServerConfig
from sleep_env_server.tui import ServerTuiApp


def test_server_tui_app_smoke_startup_and_quit() -> None:
    async def run() -> None:
        app = ServerTuiApp(ServerConfig(host="127.0.0.1", port=8081))
        async with app.run_test() as pilot:
            status = app.query_one("#status", Static)
            assert "127.0.0.1:8081" in str(status.content)
            assert len(app.query_one("#measurements", DataTable).ordered_columns) == 7
            assert app.query_one("#events", RichLog) is not None
            await pilot.press("q")

    asyncio.run(run())
