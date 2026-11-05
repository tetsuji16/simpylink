"""Contract tests: examples validate + round-trip through Model + schema + error shapes."""
import glob
import json
import os
import sys
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

from simpylink_backend.model import Model
from simpylink_backend.simulator import simulate
from simpylink_backend.blocks import BlockError

ROOT = os.path.join(os.path.dirname(__file__), "..", "..")
EXAMPLE_FILES = sorted(glob.glob(os.path.join(ROOT, "backend", "examples", "*.json")) +
                       glob.glob(os.path.join(ROOT, "examples", "*.json")))

SCHEMA_PATH = os.path.join(ROOT, "schemas", "model.schema.json")


class TestProtocol(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        with open(SCHEMA_PATH) as f:
            cls.schema = json.load(f)
        try:
            import jsonschema
            cls.validator = jsonschema.Draft7Validator(cls.schema)
            cls.has_jsonschema = True
        except ImportError:
            cls.has_jsonschema = False

    def validate_against_schema(self, model_dict):
        if self.has_jsonschema:
            self.validator.validate(model_dict)
        else:
            # minimal manual validation
            self.assertIn("name", model_dict)
            self.assertIn("blocks", model_dict)
            self.assertIn("connections", model_dict)
            for b in model_dict["blocks"]:
                self.assertIn("id", b)
                self.assertIn("type", b)
                self.assertTrue(b["id"])
            for c in model_dict["connections"]:
                self.assertIn("from", c)
                self.assertIn("to", c)
                self.assertIn("block", c["from"])
                self.assertIn("port", c["from"])
                self.assertIn("block", c["to"])
                self.assertIn("port", c["to"])

    def test_examples_exist(self):
        self.assertGreaterEqual(len(EXAMPLE_FILES), 2, "need at least 2 example models")

    def test_examples_validate_against_schema(self):
        for path in EXAMPLE_FILES:
            with self.subTest(path=path):
                with open(path) as f:
                    data = json.load(f)
                self.validate_against_schema(data)

    def test_examples_roundtrip_and_run(self):
        for path in EXAMPLE_FILES:
            with self.subTest(path=path):
                with open(path) as f:
                    data = json.load(f)
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

    def test_expected_values(self):
        for path in EXAMPLE_FILES:
            with open(path) as f:
                data = json.load(f)
            expected = data.get("expected_outputs")
            if expected is None:
                continue
            result = simulate(Model.from_dict(data))
            for bid, want in expected.items():
                got = result["outputs"][bid]
                if isinstance(want, list):
                    self.assertEqual(got, want, path)
                else:
                    self.assertAlmostEqual(got, want, places=9, msg=path)

    def test_unknown_block_rejected(self):
        with self.assertRaises(BlockError):
            simulate(Model.from_dict({"name": "t", "blocks": [{"id": "x", "type": "Nope"}], "connections": []}))

    def test_duplicate_id_rejected(self):
        with self.assertRaises(Exception):
            Model.from_dict({"name": "t", "blocks": [{"id": "a", "type": "Constant"}, {"id": "a", "type": "Constant"}], "connections": []})

    def test_divide_by_zero_error_shape(self):
        model_dict = {
            "name": "err2",
            "blocks": [
                {"id": "a", "type": "Constant", "params": {"value": 1.0}},
                {"id": "b", "type": "Constant", "params": {"value": 0.0}},
                {"id": "d", "type": "Divide"},
                {"id": "o", "type": "Outport"}
            ],
            "connections": [
                {"from": {"block": "a", "port": "out"}, "to": {"block": "d", "port": "numerator"}},
                {"from": {"block": "b", "port": "out"}, "to": {"block": "d", "port": "denominator"}},
                {"from": {"block": "d", "port": "out"}, "to": {"block": "o", "port": "in"}}
            ]
        }
        with self.assertRaises(BlockError) as cm:
            simulate(Model.from_dict(model_dict))
        self.assertIn("division by zero", str(cm.exception).lower())

    def test_algebraic_loop_error_shape(self):
        # Gain -> Gain loop
        model_dict = {
            "name": "loop",
            "blocks": [
                {"id": "g1", "type": "Gain"},
                {"id": "g2", "type": "Gain"},
                {"id": "o", "type": "Outport"}
            ],
            "connections": [
                {"from": {"block": "g1", "port": "out"}, "to": {"block": "g2", "port": "in"}},
                {"from": {"block": "g2", "port": "out"}, "to": {"block": "g1", "port": "in"}},
                {"from": {"block": "g1", "port": "out"}, "to": {"block": "o", "port": "in"}}
            ]
        }
        from simpylink_backend.simulator import AlgebraicLoopError
        with self.assertRaises(AlgebraicLoopError):
            simulate(Model.from_dict(model_dict))

    def test_missing_input_error(self):
        model_dict = {
            "name": "miss",
            "blocks": [
                {"id": "g", "type": "Gain"},
                {"id": "o", "type": "Outport"}
            ],
            "connections": [
                {"from": {"block": "g", "port": "out"}, "to": {"block": "o", "port": "in"}}
            ]
        }
        with self.assertRaises(BlockError) as cm:
            simulate(Model.from_dict(model_dict))
        self.assertIn("not connected", str(cm.exception).lower())

    def test_duplicate_connection_error(self):
        model_dict = {
            "name": "dup",
            "blocks": [
                {"id": "c", "type": "Constant", "params": {"value": 1.0}},
                {"id": "g", "type": "Gain"},
                {"id": "o", "type": "Outport"}
            ],
            "connections": [
                {"from": {"block": "c", "port": "out"}, "to": {"block": "g", "port": "in"}},
                {"from": {"block": "c", "port": "out"}, "to": {"block": "g", "port": "in"}},  # duplicate
                {"from": {"block": "g", "port": "out"}, "to": {"block": "o", "port": "in"}}
            ]
        }
        with self.assertRaises(BlockError) as cm:
            simulate(Model.from_dict(model_dict))
        self.assertIn("driven more than once", str(cm.exception).lower())

    def test_bare_model_accepted(self):
        # POST /simulate accepts bare model (without "model" wrapper)
        model_dict = {
            "name": "bare",
            "blocks": [
                {"id": "c", "type": "Constant", "params": {"value": 1.0}},
                {"id": "o", "type": "Outport"}
            ],
            "connections": [
                {"from": {"block": "c", "port": "out"}, "to": {"block": "o", "port": "in"}}
            ]
        }
        result = simulate(Model.from_dict(model_dict))
        self.assertEqual(result["status"], "ok")
        self.assertEqual(result["outputs"]["o"], 1.0)

    def test_unknown_output_port_rejected(self):
        model_dict = {
            "name": "badport",
            "blocks": [
                {"id": "c", "type": "Constant", "params": {"value": 1.0}},
                {"id": "o", "type": "Outport"}
            ],
            "connections": [
                {"from": {"block": "c", "port": "notanoutput"}, "to": {"block": "o", "port": "in"}}
            ]
        }
        with self.assertRaises(BlockError) as cm:
            simulate(Model.from_dict(model_dict))
        self.assertIn("not an output", str(cm.exception).lower())

    def test_unknown_input_port_rejected(self):
        model_dict = {
            "name": "badport2",
            "blocks": [
                {"id": "c", "type": "Constant", "params": {"value": 1.0}},
                {"id": "g", "type": "Gain"}
            ],
            "connections": [
                {"from": {"block": "c", "port": "out"}, "to": {"block": "g", "port": "notaninput"}}
            ]
        }
        with self.assertRaises(BlockError) as cm:
            simulate(Model.from_dict(model_dict))
        self.assertIn("not an input", str(cm.exception).lower())


if __name__ == "__main__":
    unittest.main()
