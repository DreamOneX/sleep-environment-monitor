"""Pydantic models for the Phase 22-compatible server contract."""

from __future__ import annotations

from typing import Literal

from pydantic import BaseModel, ConfigDict, Field

TimeStatus = Literal["uptime_only", "wall_clock_synced"]


class MeasurementUpload(BaseModel):
    """Schema-version-1 measurement upload accepted by the server."""

    schema_version: Literal[1]
    device_id: str = Field(min_length=1)
    sequence: int = Field(ge=0)
    time_status: TimeStatus
    wall_clock_unix_ms: int | None = Field(default=None, ge=0)
    uptime_ms: int = Field(ge=0)
    temperature_c: float | None
    humidity_percent: float | None
    lux: float | None
    mic_mean: float
    mic_rms: float
    mic_peak: float
    mic_db_rel: float
    mic_clip_count: int = Field(ge=0)
    error_flags: int = Field(ge=0)

    model_config = ConfigDict(extra="forbid")


class TimeResponse(BaseModel):
    """Server wall-clock time response."""

    unix_ms: int = Field(ge=0)
    source: Literal["server"] = "server"


class DiscoveryDocument(BaseModel):
    """Well-known HTTP discovery metadata."""

    api_base: str
    measurement_upload: str
    time: str
    udp_discovery_port: int = Field(ge=1, le=65535)


class UdpDiscoveryPayload(BaseModel):
    """UDP discovery response consumed by the firmware."""

    host: str
    port: int = Field(ge=1, le=65535)
    api_base: str
    measurement_upload: str
    time: str
