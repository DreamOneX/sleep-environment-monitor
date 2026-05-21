#!/usr/bin/env python3
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


class Receiver(BaseHTTPRequestHandler):
    def do_POST(self):
        length = int(self.headers.get("Content-Length", "0"))
        body = self.rfile.read(length)
        text = body.decode("utf-8", errors="replace")
        print(f"{self.client_address[0]} {self.path} {text}", flush=True)
        self.send_response(204)
        self.end_headers()

    def log_message(self, format, *args):
        return


def main():
    server = ThreadingHTTPServer(("0.0.0.0", 8080), Receiver)
    print("listening on 0.0.0.0:8080", flush=True)
    server.serve_forever()


if __name__ == "__main__":
    main()
