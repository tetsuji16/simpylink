"""Codegen tests: generated model.py runs standalone and matches simulator."""
import json
import os
import sys
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

from simpylink_backend.codegen import generate_and_run
from simpylink_backend.model import Model
from simpylink_backend.simulator import simulate


def B(bid, typ, **params):
    return {"id": bid, "type": typ, "params": params}


def C(src, sport, dst, dport):
    return {"from": {"block": src, "port": sport}, "to": {"block": dst, "port": dport}}


class TestCodegen(unittest.TestCase):
    def test_sum_matches_simulator(self):
        m = Model.from_dict({"name": "t", "blocks": [
            B("a", "Constant", value=3.0), B("b", "Constant", value=4.0),
            B("s", "Sum", inputs="++"), B("o", "Outport")],
            "connections": [C("a", "out", "s", "in1"), C("b", "out", "s", "in2"), C("s", "out", "o", "in")]})
        self.assertEqual(simulate(m)["outputs"]["o"], 7.0)
        self.assertEqual(generate_and_run(m)["outputs"]["o"], 7.0)

    def test_vector_gain(self):
        m = Model.from_dict({"name": "t", "blocks": [
            B("a", "Constant", value=[1.0, 2.0, 3.0]), B("g", "Gain", gain=2.0), B("o", "Outport")],
            "connections": [C("a", "out", "g", "in"), C("g", "out", "o", "in")]})
        self.assertEqual(generate_and_run(m)["outputs"]["o"], [2.0, 4.0, 6.0])

    def test_examples_via_codegen(self):
        root = os.path.join(os.path.dirname(__file__), "..", "..")
        for name in ("sum_example", "gain_chain", "product_divide"):
            with self.subTest(name=name):
                m = Model.from_dict(json.load(open(os.path.join(root, "examples", name + ".json"))))
                out = generate_and_run(m)
                self.assertEqual(out["status"], "ok")
                self.assertTrue(out["outputs"])

    def test_time_codegen_integrator(self):
        m = Model.from_dict({"name": "t", "blocks": [
            B("c", "Constant", value=1.0), B("i", "Integrator", initial_condition=0.0), B("o", "Outport")],
            "connections": [C("c", "out", "i", "in"), C("i", "out", "o", "in")]})
        out = generate_and_run(m, mode="time", t_start=0.0, t_stop=1.0, dt=0.5)
        self.assertEqual(len(out["time"]), 3)
        self.assertAlmostEqual(out["outputs"]["o"][-1], 1.0, places=9)


if __name__ == "__main__":
    unittest.main()
