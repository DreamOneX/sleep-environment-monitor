#!/usr/bin/env python3
import json
import socket
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlsplit


HTTP_HOST = "0.0.0.0"
HTTP_PORT = 8080
API_BASE = "/api/v1"
MEASUREMENT_UPLOAD_PATH = f"{API_BASE}/measurements"
TIME_PATH = f"{API_BASE}/time"
DISCOVERY_PATH = "/.well-known/sleep-environment-monitor"
DISCOVERY_PORT = 39022
DISCOVERY_QUERY = "sleep-environment-monitor.discovery"


def compact_json(payload):
    return json.dumps(payload, separators=(",", ":")).encode("utf-8")


def current_unix_ms():
    return int(time.time() * 1000)


def discovery_document():
    return {
        "api_base": API_BASE,
        "measurement_upload": MEASUREMENT_UPLOAD_PATH,
        "time": TIME_PATH,
        "udp_discovery_port": DISCOVERY_PORT,
    }


def local_address_for_peer(peer_host):
    probe = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    try:
        probe.connect((peer_host, 9))
        return probe.getsockname()[0]
    except OSError:
        return socket.gethostbyname(socket.gethostname())
    finally:
        probe.close()


def discovery_response(peer_host):
    payload = {
        "host": local_address_for_peer(peer_host),
        "port": HTTP_PORT,
        "api_base": API_BASE,
        "measurement_upload": MEASUREMENT_UPLOAD_PATH,
        "time": TIME_PATH,
    }
    return compact_json(payload)


class DiscoveryResponder(threading.Thread):
    def __init__(self):
        super().__init__(name="udp-discovery")
        self._stop_requested = threading.Event()
        self._socket = None

    def stop(self):
        self._stop_requested.set()
        if self._socket is not None:
            self._socket.close()

    def run(self):
        sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self._socket = sock
        try:
            sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            sock.bind((HTTP_HOST, DISCOVERY_PORT))
            sock.settimeout(0.5)
            print(f"udp discovery on {HTTP_HOST}:{DISCOVERY_PORT}", flush=True)
            while not self._stop_requested.is_set():
                try:
                    data, addr = sock.recvfrom(512)
                except socket.timeout:
                    continue
                except OSError:
                    break
                if data.decode("utf-8", errors="ignore").strip() != DISCOVERY_QUERY:
                    continue
                try:
                    sock.sendto(discovery_response(addr[0]), addr)
                except OSError as exc:
                    print(f"udp discovery response failed: {exc}", flush=True)
        except OSError as exc:
            print(f"udp discovery disabled: {exc}", flush=True)
        finally:
            try:
                sock.close()
            finally:
                self._socket = None


class Receiver(BaseHTTPRequestHandler):
    def do_POST(self):
        path = urlsplit(self.path).path
        if path != MEASUREMENT_UPLOAD_PATH:
            self._send_json(404, {"error": "not_found"})
            return

        try:
            length = int(self.headers.get("Content-Length", "0"))
        except ValueError:
            self._send_json(400, {"error": "invalid_content_length"})
            return

        body = self.rfile.read(length)

        try:
            json.loads(body.decode("utf-8"))
        except (UnicodeDecodeError, json.JSONDecodeError):
            print(f"upload rejected from {self.client_address[0]} invalid_json", flush=True)
            self._send_json(400, {"error": "invalid_json"})
            return

        print(f"upload accepted from {self.client_address[0]} bytes={length}", flush=True)
        self.send_response(204)
        self.send_header("Content-Length", "0")
        self.end_headers()

    def do_GET(self):
        path = urlsplit(self.path).path
        if path == TIME_PATH:
            self._send_json(200, {"unix_ms": current_unix_ms(), "source": "server"})
            return
        if path == DISCOVERY_PATH:
            self._send_json(200, discovery_document())
            return
        self._send_json(404, {"error": "not_found"})

    def _send_json(self, status, payload):
        body = compact_json(payload)
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, format, *args):
        return


class Server(ThreadingHTTPServer):
    daemon_threads = True


def main():
    discovery = DiscoveryResponder()
    server = Server((HTTP_HOST, HTTP_PORT), Receiver)

    discovery.start()
    print(f"http on {HTTP_HOST}:{HTTP_PORT}", flush=True)
    try:
        server.serve_forever(poll_interval=0.5)
    except KeyboardInterrupt:
        print("shutdown requested", flush=True)
    finally:
        discovery.stop()
        server.server_close()
        discovery.join(timeout=1.0)
        print("stopped", flush=True)


if __name__ == "__main__":
    main()
