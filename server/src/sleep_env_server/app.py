"""FastAPI application factory for the ingestion server."""

from __future__ import annotations

import time
from collections.abc import Callable
from typing import Protocol

from fastapi import FastAPI, Request, Response

from sleep_env_server.config import ServerConfig
from sleep_env_server.models import DiscoveryDocument, MeasurementUpload, TimeResponse
from sleep_env_server.output import NullOutput
from sleep_env_server.storage import InMemoryMeasurementSink

Clock = Callable[[], int]


class UploadOutput(Protocol):
    """Output protocol needed by the FastAPI upload route."""

    def upload_accepted(
        self,
        *,
        source: str,
        byte_count: int,
        device_id: str,
        sequence: int,
        duplicate: bool,
    ) -> None:
        """Writes bounded upload acceptance metadata."""


def current_unix_ms() -> int:
    """Returns the current Unix time in milliseconds."""
    return int(time.time() * 1000)


def create_app(
    config: ServerConfig | None = None,
    *,
    clock: Clock = current_unix_ms,
    sink: InMemoryMeasurementSink | None = None,
    output: UploadOutput | None = None,
) -> FastAPI:
    """Creates the FastAPI application.

    Args:
        config: Active server configuration. Defaults match firmware fallback.
        clock: Injectable millisecond clock for deterministic tests.
        sink: Measurement sink. Defaults to process-local in-memory storage.
        output: Output sink for bounded upload diagnostics.

    Returns:
        Configured FastAPI application.
    """
    active_config = config if config is not None else ServerConfig()
    active_sink = sink if sink is not None else InMemoryMeasurementSink()
    active_output = output if output is not None else NullOutput()

    app = FastAPI(title="Sleep Environment Monitor Server")

    @app.post(active_config.measurement_upload_path, status_code=204)
    async def upload_measurement(upload: MeasurementUpload, request: Request) -> Response:
        body = await request.body()
        accepted = active_sink.accept(upload)
        source = request.client.host if request.client is not None else "unknown"
        active_output.upload_accepted(
            source=source,
            byte_count=len(body),
            device_id=upload.device_id,
            sequence=upload.sequence,
            duplicate=accepted.duplicate,
        )
        return Response(status_code=204)

    @app.get(active_config.time_path, response_model=TimeResponse)
    async def get_time() -> TimeResponse:
        return TimeResponse(unix_ms=clock(), source="server")

    @app.get(active_config.discovery_document_path, response_model=DiscoveryDocument)
    async def get_discovery_document() -> DiscoveryDocument:
        return active_config.discovery_document()

    return app
