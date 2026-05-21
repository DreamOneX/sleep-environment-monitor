"""Server configuration and derived API paths."""

from __future__ import annotations

from dataclasses import dataclass

from sleep_env_server.models import DiscoveryDocument

DEFAULT_HOST = "0.0.0.0"
DEFAULT_PORT = 8080
DEFAULT_UDP_DISCOVERY_PORT = 39022
DEFAULT_API_BASE = "/api/v1"
DISCOVERY_DOCUMENT_PATH = "/.well-known/sleep-environment-monitor"


def validate_port(value: int, name: str) -> int:
    """Validates a TCP or UDP port.

    Args:
        value: Candidate port value.
        name: Human-readable field name for error messages.

    Returns:
        The validated port.

    Raises:
        ValueError: If the port is outside the valid user-visible range.
    """
    if not 1 <= value <= 65535:
        raise ValueError(f"{name} must be in 1..65535")
    return value


@dataclass(frozen=True)
class ServerConfig:
    """Runtime configuration for HTTP and UDP discovery."""

    host: str = DEFAULT_HOST
    port: int = DEFAULT_PORT
    udp_discovery_port: int = DEFAULT_UDP_DISCOVERY_PORT
    api_base: str = DEFAULT_API_BASE

    def __post_init__(self) -> None:
        """Validates configuration values after dataclass initialization."""
        if not self.host:
            raise ValueError("host must not be empty")
        validate_port(self.port, "port")
        validate_port(self.udp_discovery_port, "udp_discovery_port")
        if not self.api_base.startswith("/"):
            raise ValueError("api_base must start with '/'")
        if self.api_base != "/" and self.api_base.endswith("/"):
            raise ValueError("api_base must not end with '/'")

    @property
    def measurement_upload_path(self) -> str:
        """Returns the versioned measurement upload path."""
        return f"{self.api_base}/measurements"

    @property
    def time_path(self) -> str:
        """Returns the server time endpoint path."""
        return f"{self.api_base}/time"

    @property
    def discovery_document_path(self) -> str:
        """Returns the well-known discovery document path."""
        return DISCOVERY_DOCUMENT_PATH

    def discovery_document(self) -> DiscoveryDocument:
        """Builds the HTTP discovery document for this configuration.

        Returns:
            Discovery metadata served from the well-known endpoint.
        """
        return DiscoveryDocument(
            api_base=self.api_base,
            measurement_upload=self.measurement_upload_path,
            time=self.time_path,
            udp_discovery_port=self.udp_discovery_port,
        )
