"""FastAPI application factory for the ingestion server."""

from __future__ import annotations

import time
from collections.abc import Callable
from typing import Any, Protocol

from fastapi import FastAPI, HTTPException, Query, Request, Response, status

from sleep_env_server.config import HistoryApiConfig, ServerConfig
from sleep_env_server.models import DiscoveryDocument, MeasurementUpload, TimeResponse
from sleep_env_server.output import NullOutput
from sleep_env_server.storage import (
    ConfiguredStore,
    InMemoryMeasurementSink,
    StorageAcceptance,
    history_record_to_dict,
    history_summary_to_dict,
    list_configured_history_records,
    summarize_configured_history_records,
)

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

    def measurement_dashboard(
        self,
        *,
        received_unix_ms: int,
        device_id: str,
        sequence: int,
        temperature_c: float | None,
        humidity_percent: float | None,
        lux: float | None,
        mic_db_rel: float,
        duplicate: bool,
    ) -> None:
        """Writes a local live measurement dashboard update."""


def current_unix_ms() -> int:
    """Returns the current Unix time in milliseconds."""
    return int(time.time() * 1000)


def create_app(
    config: ServerConfig | None = None,
    *,
    clock: Clock = current_unix_ms,
    sink: object | None = None,
    output: UploadOutput | None = None,
    history_api: HistoryApiConfig | None = None,
) -> FastAPI:
    """Creates the FastAPI application.

    Args:
        config: Active server configuration. Defaults match firmware fallback.
        clock: Injectable millisecond clock for deterministic tests.
        sink: Measurement sink. Defaults to process-local in-memory storage.
        output: Output sink for bounded upload diagnostics.
        history_api: Optional authenticated history API configuration.

    Returns:
        Configured FastAPI application.
    """
    active_config = config if config is not None else ServerConfig()
    active_sink = sink if sink is not None else InMemoryMeasurementSink()
    active_output = output if output is not None else NullOutput()
    active_history_api = history_api if history_api is not None else HistoryApiConfig()

    app = FastAPI(title="Sleep Environment Monitor Server")

    @app.post(active_config.measurement_upload_path, status_code=204)
    async def upload_measurement(upload: MeasurementUpload, request: Request) -> Response:
        body = await request.body()
        source = request.client.host if request.client is not None else "unknown"
        received_unix_ms = clock()
        accepted = _accept_measurement(
            active_sink,
            upload,
            received_unix_ms=received_unix_ms,
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
        active_output.measurement_dashboard(
            received_unix_ms=received_unix_ms,
            device_id=upload.device_id,
            sequence=upload.sequence,
            temperature_c=upload.temperature_c,
            humidity_percent=upload.humidity_percent,
            lux=upload.lux,
            mic_db_rel=upload.mic_db_rel,
            duplicate=accepted.duplicate,
        )
        return Response(status_code=204)

    @app.get(active_config.time_path, response_model=TimeResponse)
    async def get_time() -> TimeResponse:
        return TimeResponse(unix_ms=clock(), source="server")

    @app.get(active_config.discovery_document_path, response_model=DiscoveryDocument)
    async def get_discovery_document() -> DiscoveryDocument:
        return active_config.discovery_document()

    if active_history_api.enabled:

        @app.get(active_config.history_measurements_path)
        async def get_history_measurements(
            request: Request,
            device_id: str | None = None,
            start_unix_ms: int | None = Query(default=None, ge=0),
            end_unix_ms: int | None = Query(default=None, ge=0),
            limit: int = Query(default=100, ge=1, le=1000),
            offset: int = Query(default=0, ge=0),
        ) -> dict[str, object]:
            _authorize_history_request(request, active_history_api)
            _validate_time_range(start_unix_ms, end_unix_ms)
            try:
                records = list_configured_history_records(
                    _history_stores(active_sink),
                    read_source=active_history_api.read_source,
                    merge_sources=active_history_api.merge_sources,
                    merge_conflict=active_history_api.merge_conflict,
                    device_id=device_id,
                    start_unix_ms=start_unix_ms,
                    end_unix_ms=end_unix_ms,
                    limit=limit,
                    offset=offset,
                )
            except ValueError as exc:
                raise _history_read_error(exc) from exc
            return {
                "records": [history_record_to_dict(record) for record in records],
                "limit": limit,
                "offset": offset,
            }

        @app.get(active_config.history_summary_path)
        async def get_history_summary(
            request: Request,
            device_id: str | None = None,
            start_unix_ms: int | None = Query(default=None, ge=0),
            end_unix_ms: int | None = Query(default=None, ge=0),
        ) -> dict[str, object]:
            _authorize_history_request(request, active_history_api)
            _validate_time_range(start_unix_ms, end_unix_ms)
            try:
                summary = summarize_configured_history_records(
                    _history_stores(active_sink),
                    read_source=active_history_api.read_source,
                    merge_sources=active_history_api.merge_sources,
                    merge_conflict=active_history_api.merge_conflict,
                    device_id=device_id,
                    start_unix_ms=start_unix_ms,
                    end_unix_ms=end_unix_ms,
                )
            except ValueError as exc:
                raise _history_read_error(exc) from exc
            return history_summary_to_dict(summary)

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


def _authorize_history_request(request: Request, config: HistoryApiConfig) -> None:
    """Checks Bearer token authorization for history routes."""
    expected = f"Bearer {config.bearer_token}"
    if request.headers.get("authorization") != expected:
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="invalid bearer token",
            headers={"WWW-Authenticate": "Bearer"},
        )


def _validate_time_range(start_unix_ms: int | None, end_unix_ms: int | None) -> None:
    """Rejects inverted history time ranges."""
    if start_unix_ms is not None and end_unix_ms is not None and start_unix_ms > end_unix_ms:
        raise HTTPException(
            status_code=status.HTTP_422_UNPROCESSABLE_ENTITY,
            detail="start_unix_ms must be <= end_unix_ms",
        )


def _history_stores(sink: Any) -> tuple[ConfiguredStore, ...]:
    """Returns configured stores or a clear service-unavailable error."""
    stores = getattr(sink, "stores", None)
    if stores is None:
        raise HTTPException(
            status_code=status.HTTP_503_SERVICE_UNAVAILABLE,
            detail="history storage is not configured",
        )
    return stores


def _history_read_error(exc: ValueError) -> HTTPException:
    """Maps storage history read errors to HTTP errors."""
    detail = str(exc)
    status_code = (
        status.HTTP_409_CONFLICT if "conflict" in detail else status.HTTP_503_SERVICE_UNAVAILABLE
    )
    return HTTPException(status_code=status_code, detail=detail)
