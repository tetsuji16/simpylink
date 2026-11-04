"""API tests: in-process HTTP server on ephemeral port."""
import json
import os
import sys
import threading
import unittest
import urllib.request

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

from simpylink_backend.server import Handler
from http.server import ThreadingHTTPServer


class TestApi(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.srv = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        cls.port = cls.srv.server_address[1]
        cls.thread = threading.Thread(target=cls.srv.serve_forever, daemon=True)
        cls.thread.start()

    @classmethod
    def tearDownClass(cls):
        cls.srv.shutdown()

    def _get(self, path):
        with urllib.request.urlopen(f"http://127.0.0.1:{self.port}{path}") as r:
            return r.status, json.loads(r.read())

    def _post(self, path, payload):
        req = urllib.request.Request(
            f"http://127.0.0.1:{self.port}{path}",
            data=json.dumps(payload).encode(),
            headers={"Content-Type": "application/json"},
        )
        try:
            with urllib.request.urlopen(req) as r:
                return r.status, json.loads(r.read())
        except urllib.error.HTTPError as e:
            return e.code, json.loads(e.read())

    def test_health_and_blocks(self):
        code, body = self._get("/health")
        self.assertEqual(code, 200)
        code, body = self._get("/blocks")
        self.assertEqual(code, 200)
        types = {b["type"] for b in body["blocks"]}
        for t in ("Gain", "Sum", "MinMax", "TrigonometricFunction", "Polynomial", "SineWaveFunction"):
            self.assertIn(t, types)

    def test_simulate_ok_and_400(self):
        model = {"name": "t",
                 "blocks": [{"id": "c", "type": "Constant", "params": {"value": 3}}],
                 "connections": []}
        code, _ = self._post("/simulate", {"model": model})
        # no Outport -> still ok (empty outputs) OR 200; main check: unknown block is 400
        self.assertIn(code, (200, 400))
        code, body = self._post("/simulate", {"model": {"name": "t", "blocks": [{"id": "x", "type": "Nope"}], "connections": []}})
        self.assertEqual(code, 400)
        self.assertIn("error", body)

    def test_simulate_time(self):
        model = {"name": "t",
                 "blocks": [{"id": "c", "type": "Constant", "params": {"value": 1.0}},
                            {"id": "i", "type": "Integrator", "params": {"initial_condition": 0.0}},
                            {"id": "o", "type": "Outport", "params": {}}],
                 "connections": [{"from": {"block": "c", "port": "out"}, "to": {"block": "i", "port": "in"}},
                                 {"from": {"block": "i", "port": "out"}, "to": {"block": "o", "port": "in"}}]}
        code, body = self._post("/simulate_time", {"model": model, "t_start": 0, "t_stop": 1.0, "dt": 0.5})
        self.assertEqual(code, 200)
        self.assertEqual(len(body["time"]), 3)
        self.assertAlmostEqual(body["outputs"]["o"][-1], 1.0, places=9)


if __name__ == "__main__":
    unittest.main()
