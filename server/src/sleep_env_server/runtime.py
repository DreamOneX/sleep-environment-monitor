"""Shared server runtime lifecycle for CLI and TUI entry points."""

from __future__ import annotations

import threading
from dataclasses import dataclass

import uvicorn

from sleep_env_server.app import create_app
from sleep_env_server.config import AppConfig
from sleep_env_server.discovery import UdpDiscoveryResponder
from sleep_env_server.output import ServerOutput
from sleep_env_server.storage import ConfiguredMeasurementSink, StorageMaintenanceThread


@dataclass(frozen=True)
class ServerRuntime:
    """Owns the running HTTP, UDP, storage, and maintenance components."""

    sink: ConfiguredMeasurementSink
    discovery: UdpDiscoveryResponder
    uvicorn_server: uvicorn.Server
    uvicorn_thread: threading.Thread
    maintenance: StorageMaintenanceThread | None = None

    def stop(self) -> None:
        """Requests service shutdown and waits briefly for background threads."""
        self.uvicorn_server.should_exit = True
        self.discovery.stop()
        if self.maintenance is not None:
            self.maintenance.stop()
        self.uvicorn_thread.join(timeout=2.0)
        self.discovery.join(timeout=1.0)
        if self.maintenance is not None:
            self.maintenance.join(timeout=1.0)


def start_server_runtime(app_config: AppConfig, output: ServerOutput) -> ServerRuntime:
    """Starts the configured server stack in background threads.

    Args:
        app_config: Fully loaded TOML and CLI-overridden configuration.
        output: Event sink for bounded diagnostics.

    Returns:
        Started runtime components. Call ``stop`` to shut them down.
    """
    config = app_config.server
    sink = ConfiguredMeasurementSink(app_config.storage)
    app = create_app(config, sink=sink, output=output, history_api=app_config.history_api)
    discovery = UdpDiscoveryResponder(config, output)
    maintenance: StorageMaintenanceThread | None = None

    output.startup(config, config.log_level)
    if app_config.storage.reconcile_on_start:
        output.storage_reconciled(copied=sink.reconcile_once())
    sink.enforce_retention_once()
    if app_config.storage.reconcile_interval_seconds > 0 and sink.stores:
        maintenance = StorageMaintenanceThread(
            sink,
            app_config.storage.reconcile_interval_seconds,
        )
        maintenance.start()
    discovery.start()

    uvicorn_config = uvicorn.Config(
        app,
        host=config.host,
        port=config.port,
        log_level=config.log_level,
    )
    uvicorn_server = uvicorn.Server(uvicorn_config)
    uvicorn_thread = threading.Thread(
        target=uvicorn_server.run,
        name="uvicorn-server",
        daemon=True,
    )
    uvicorn_thread.start()

    return ServerRuntime(
        sink=sink,
        discovery=discovery,
        maintenance=maintenance,
        uvicorn_server=uvicorn_server,
        uvicorn_thread=uvicorn_thread,
    )
