"""Contract tests: examples validate + round-trip through Model."""
import glob
import json
import os
import sys
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

from simpylink_backend.model import Model
from simpylink_backend.simulator import simulate

ROOT = os.path.join(os.path.dirname(__file__), "..", "..")
EXAMPLE_FILES = sorted(glob.glob(os.path.join(ROOT, "backend", "examples", "*.json")) +
                       glob.glob(os.path.join(ROOT, "examples", "*.json")))


class TestProtocol(unittest.TestCase):
    def test_examples_exist(self):
        self.assertGreaterEqual(len(EXAMPLE_FILES), 2, "need at least 2 example models")

    def test_examples_roundtrip_and_run(self):
        for path in EXAMPLE_FILES:
            with self.subTest(path=path):
                data = json.load(open(path))
                self.assertIn("blocks", data)
                self.assertIn("connections", data)
                model = Model.from_dict(data)
                # round-trip: to_dict -> from_dict preserves blocks/connections
                again = Model.from_dict(model.to_dict())
                self.assertEqual(len(again.blocks), len(model.blocks))
                self.assertEqual(len(again.connections), len(model.connections))
                result = simulate(model)
                self.assertEqual(result["status"], "ok")
                self.assertTrue(result["outputs"], f"{path} has no outputs")

    def test_unknown_block_rejected(self):
        with self.assertRaises(Exception):
            simulate(Model.from_dict({"name": "t", "blocks": [{"id": "x", "type": "Nope"}], "connections": []}))

    def test_duplicate_id_rejected(self):
        with self.assertRaises(Exception):
            Model.from_dict({"name": "t", "blocks": [{"id": "a", "type": "Constant"}, {"id": "a", "type": "Constant"}], "connections": []})

    def test_expected_values(self):
        for path in EXAMPLE_FILES:
            data = json.load(open(path))
            expected = data.pop("expected_outputs", None)
            if expected is None:
                continue
            result = simulate(Model.from_dict(data))
            for bid, want in expected.items():
                got = result["outputs"][bid]
                if isinstance(want, list):
                    self.assertEqual(got, want, path)
                else:
                    self.assertAlmostEqual(got, want, places=9, msg=path)


if __name__ == "__main__":
    unittest.main()
