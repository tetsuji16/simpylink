"""Stdlib HTTP server: GET /health, GET /blocks, POST /simulate."""
from __future__ import annotations

import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

from . import __version__
from .blocks import BLOCK_REGISTRY, block_public
from .model import Model, ModelError
from .simulator import simulate

HOST, PORT = "127.0.0.1", 8765


class Handler(BaseHTTPRequestHandler):
    server_version = f"simpylink-backend/{__version__}"

    def log_message(self, *args):  # keep console quiet
        pass

    def _send(self, code: int, payload: dict):
        body = json.dumps(payload).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        if self.path == "/health":
            self._send(200, {"status": "ok", "version": __version__})
        elif self.path == "/blocks":
            self._send(200, {"blocks": [dict(block_public(s), type=t) for t, s in BLOCK_REGISTRY.items()]})
        else:
            self._send(404, {"error": "not found"})

    def do_POST(self):
        if self.path != "/simulate":
            return self._send(404, {"error": "not found"})
        length = int(self.headers.get("Content-Length") or 0)
        try:
            req = json.loads(self.rfile.read(length) or b"{}")
            model = Model.from_dict(req.get("model") or req)  # accept bare model too
            result = simulate(model, inputs=req.get("inputs") or {})
            self._send(200, result)
        except (ModelError, ValueError, KeyError) as e:
            self._send(400, {"error": type(e).__name__, "detail": str(e)})


def main(host: str = HOST, port: int = PORT):
    srv = ThreadingHTTPServer((host, port), Handler)
    print(f"simpylink backend on http://{host}:{port} (GET /health, POST /simulate, GET /blocks)")
    srv.serve_forever()


if __name__ == "__main__":
    main()
