"""Stdlib HTTP server: GET /health, GET /blocks, POST /simulate, POST /simulate_time."""
from __future__ import annotations

import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

from . import __version__
from .blocks import BLOCK_REGISTRY, block_public
from .model import Model, ModelError
from .plugins import load_plugins
from .simulator import simulate
from .time_simulator import simulate_time

HOST, PORT = "127.0.0.1", 8765


def _major(version: str) -> int:
    try:
        return int(str(version).split(".")[0])
    except ValueError:
        return 0


class Handler(BaseHTTPRequestHandler):
    server_version = f"simpylink-backend/{__version__}"

    def log_message(self, fmt=None, *args):  # keep console quiet
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
            self._send(404, {"error": "NotFound", "detail": f"no such path: {self.path}"})

    def _check_version(self, model: Model):
        # Additive minor versions are accepted; a newer major is rejected.
        if _major(model.version) > _major(__version__):
            raise ModelError(f"model version {model.version} is newer than backend {__version__} (major mismatch)")

    def _read_json(self):
        length = int(self.headers.get("Content-Length") or 0)
        raw = self.rfile.read(length) if length else b""
        try:
            return json.loads(raw or b"{}")
        except json.JSONDecodeError as e:
            raise ModelError(f"malformed JSON: {e}") from e

    def do_POST(self):
        if self.path not in ("/simulate", "/simulate_time"):
            return self._send(404, {"error": "NotFound", "detail": f"no such path: {self.path}"})
        try:
            req = self._read_json()
            model = Model.from_dict(req.get("model") or req)
            self._check_version(model)
            if self.path == "/simulate":
                result = simulate(model, inputs=req.get("inputs") or {})
            else:
                result = simulate_time(
                    model,
                    t_start=float(req.get("t_start", 0.0)),
                    t_stop=float(req.get("t_stop", 1.0)),
                    dt=float(req.get("dt", 0.1)),
                    inputs=req.get("inputs") or {},
                )
            self._send(200, result)
        except (ModelError, ValueError, KeyError) as e:
            self._send(400, {"error": type(e).__name__, "detail": str(e)})


def main(host: str = HOST, port: int = PORT):
    registered = load_plugins()
    if registered:
        print(f"[simpylink] plugins registered: {registered}")
    srv = ThreadingHTTPServer((host, port), Handler)
    print(f"simpylink backend on http://{host}:{port} (GET /health, GET /blocks, POST /simulate, POST /simulate_time)")
    srv.serve_forever()


if __name__ == "__main__":
    main()
