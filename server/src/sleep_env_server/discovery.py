"""UDP discovery helpers and responder thread."""

from __future__ import annotations

import json
import socket
import threading
from collections.abc import Callable

from sleep_env_server.config import ServerConfig
from sleep_env_server.models import UdpDiscoveryPayload
from sleep_env_server.output import ServerOutput

DISCOVERY_QUERY = "sleep-environment-monitor.discovery"
HostResolver = Callable[[str], str]


def compact_json(payload: dict[str, object]) -> bytes:
    """Encodes a payload as compact UTF-8 JSON."""
    return json.dumps(payload, separators=(",", ":")).encode("utf-8")


def local_address_for_peer(peer_host: str) -> str:
    """Selects the local IPv4 address that would reach a peer.

    Args:
        peer_host: Peer IPv4 address from the incoming discovery datagram.

    Returns:
        Local IPv4 address suitable for the peer to use in HTTP requests.
    """
    probe = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    try:
        probe.connect((peer_host, 9))
        return probe.getsockname()[0]
    except OSError:
        return socket.gethostbyname(socket.gethostname())
    finally:
        probe.close()


def build_udp_discovery_payload(
    config: ServerConfig,
    peer_host: str,
    *,
    host_resolver: HostResolver = local_address_for_peer,
) -> UdpDiscoveryPayload:
    """Builds the UDP discovery response for one peer.

    Args:
        config: Active server configuration.
        peer_host: Peer IPv4 address from the incoming datagram.
        host_resolver: Injectable host-selection function for tests.

    Returns:
        UDP discovery payload with HTTP endpoint and API paths.
    """
    return UdpDiscoveryPayload(
        host=host_resolver(peer_host),
        port=config.port,
        api_base=config.api_base,
        measurement_upload=config.measurement_upload_path,
        time=config.time_path,
    )


def build_udp_discovery_response(
    config: ServerConfig,
    query: bytes,
    peer_host: str,
    *,
    host_resolver: HostResolver = local_address_for_peer,
) -> bytes | None:
    """Builds a compact UDP response for a valid discovery query.

    Args:
        config: Active server configuration.
        query: Incoming UDP datagram payload.
        peer_host: Peer IPv4 address from the incoming datagram.
        host_resolver: Injectable host-selection function for tests.

    Returns:
        Compact JSON bytes for the response, or ``None`` when the query should
        be ignored silently.
    """
    try:
        decoded = query.decode("utf-8")
    except UnicodeDecodeError:
        return None
    if decoded.strip() != DISCOVERY_QUERY:
        return None

    payload = build_udp_discovery_payload(
        config,
        peer_host,
        host_resolver=host_resolver,
    )
    return compact_json(payload.model_dump())


class UdpDiscoveryResponder(threading.Thread):
    """Background UDP responder for firmware server discovery."""

    def __init__(
        self,
        config: ServerConfig,
        output: ServerOutput,
        *,
        host_resolver: HostResolver = local_address_for_peer,
    ) -> None:
        """Initializes the responder thread.

        Args:
            config: Active server configuration.
            output: Output sink for bounded diagnostics.
            host_resolver: Injectable host-selection function for tests.
        """
        super().__init__(name="udp-discovery", daemon=True)
        self._config = config
        self._output = output
        self._host_resolver = host_resolver
        self._stop_requested = threading.Event()
        self._socket: socket.socket | None = None

    def stop(self) -> None:
        """Requests responder shutdown and closes the bound socket."""
        self._stop_requested.set()
        if self._socket is not None:
            self._socket.close()

    def run(self) -> None:
        """Runs the UDP discovery loop until stopped or socket setup fails."""
        sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self._socket = sock
        try:
            sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            sock.bind((self._config.host, self._config.udp_discovery_port))
            sock.settimeout(0.5)
            self._output.udp_started(self._config)
            while not self._stop_requested.is_set():
                try:
                    data, addr = sock.recvfrom(512)
                except TimeoutError:
                    continue
                except OSError:
                    break

                response = build_udp_discovery_response(
                    self._config,
                    data,
                    addr[0],
                    host_resolver=self._host_resolver,
                )
                if response is None:
                    continue
                try:
                    sock.sendto(response, addr)
                except OSError as exc:
                    self._output.udp_response_failed(str(exc))
        except OSError as exc:
            self._output.udp_disabled(str(exc))
        finally:
            self._close_socket(sock)

    def _close_socket(self, sock: socket.socket) -> None:
        """Closes the socket and clears responder state."""
        try:
            sock.close()
        finally:
            self._socket = None
