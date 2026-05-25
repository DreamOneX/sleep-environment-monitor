"""FastAPI application factory for the ingestion server."""

from __future__ import annotations

import time
from collections.abc import Callable
from typing import Any, Protocol

from fastapi import FastAPI, Request, Response

from sleep_env_server.config import ServerConfig
from sleep_env_server.models import DiscoveryDocument, MeasurementUpload, TimeResponse
from sleep_env_server.output import NullOutput
from sleep_env_server.storage import InMemoryMeasurementSink, StorageAcceptance

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
        """Writes bounded upload rejection metadata."""


def current_unix_ms() -> int:
    """Returns the current Unix time in milliseconds."""
    return int(time.time() * 1000)


def create_app(
    config: ServerConfig | None = None,
    *,
    clock: Clock = current_unix_ms,
    sink: object | None = None,
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
        source = request.client.host if request.client is not None else "unknown"
        accepted = _accept_measurement(
            active_sink,
            upload,
            received_unix_ms=clock(),
            source=source,
        )
        if not accepted.accepted:
            active_output.upload_rejected(
                source=source,
                byte_count=len(body),
                device_id=upload.device_id,
                sequence=upload.sequence,
                status_code=accepted.status_code,
                duplicate=accepted.duplicate,
                conflict=accepted.conflict,
                reason=accepted.reason or "storage ACK policy was not satisfied",
            )
            return Response(status_code=accepted.status_code)
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


def _accept_measurement(
    sink: Any,
    upload: MeasurementUpload,
    *,
    received_unix_ms: int,
    source: str,
) -> StorageAcceptance:
    """Accepts an upload through either the configured or legacy sink API."""
    accept_upload = getattr(sink, "accept_upload", None)
    if callable(accept_upload):
        return accept_upload(upload, received_unix_ms=received_unix_ms, source=source)

    accepted = sink.accept(upload)
    return StorageAcceptance(
        accepted=True,
        duplicate=accepted.duplicate,
        conflict=False,
    )
