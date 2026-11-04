"""Golden tests for Phase-1 scalar blocks (hand-computed expectations)."""
import math
import os
import sys
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

from simpylink_backend.model import Model
from simpylink_backend.simulator import simulate


def run(blocks, conns, inputs=None):
    return simulate(Model.from_dict({"name": "t", "blocks": blocks, "connections": conns}), inputs or {})


def B(bid, typ, **params):
    return {"id": bid, "type": typ, "params": params}


def C(src, sport, dst, dport):
    return {"from": {"block": src, "port": sport}, "to": {"block": dst, "port": dport}}


def one(typ, value, **params):
    """Constant -> block -> Outport, return output."""
    r = run([B("c", "Constant", value=value), B("b", typ, **params), B("o", "Outport")],
            [C("c", "out", "b", "in"), C("b", "out", "o", "in")])
    return r["outputs"]["o"]


class TestScalarBlocks(unittest.TestCase):
    def test_minmax(self):
        r = run([B("a", "Constant", value=5.0), B("b", "Constant", value=3.0),
                 B("m", "MinMax", mode="min"), B("o", "Outport")],
                [C("a", "out", "m", "in1"), C("b", "out", "m", "in2"), C("m", "out", "o", "in")])
        self.assertAlmostEqual(r["outputs"]["o"], 3.0)
        r = run([B("a", "Constant", value=5.0), B("b", "Constant", value=3.0),
                 B("m", "MinMax", mode="max"), B("o", "Outport")],
                [C("a", "out", "m", "in1"), C("b", "out", "m", "in2"), C("m", "out", "o", "in")])
        self.assertAlmostEqual(r["outputs"]["o"], 5.0)

    def test_trig(self):
        self.assertAlmostEqual(one("TrigonometricFunction", math.pi / 2, function="sin"), 1.0, places=9)
        self.assertAlmostEqual(one("TrigonometricFunction", 0.0, function="cos"), 1.0, places=9)

    def test_math_function(self):
        self.assertAlmostEqual(one("MathFunction", 1.0, function="exp"), math.e, places=9)
        self.assertAlmostEqual(one("MathFunction", 100.0, function="log10"), 2.0, places=9)
        self.assertAlmostEqual(one("MathFunction", 3.0, function="pow", exponent=2.0), 9.0, places=9)
        with self.assertRaises(Exception):
            one("MathFunction", -1.0, function="log")

    def test_rounding_sign_sqrt(self):
        self.assertAlmostEqual(one("RoundingFunction", 3.7, mode="floor"), 3.0)
        self.assertAlmostEqual(one("RoundingFunction", 3.2, mode="ceil"), 4.0)
        self.assertAlmostEqual(one("Sign", -5.0), -1.0)
        self.assertAlmostEqual(one("Sign", 0.0), 0.0)
        self.assertAlmostEqual(one("Sqrt", 9.0, mode="sqrt"), 3.0)
        self.assertAlmostEqual(one("Sqrt", -9.0, mode="signed_sqrt"), -3.0)
        with self.assertRaises(Exception):
            one("Sqrt", -1.0, mode="sqrt")

    def test_polynomial(self):
        # [1, 0, -1] at x=3 -> 9-1 = 8
        self.assertAlmostEqual(one("Polynomial", 3.0, coefficients=[1.0, 0.0, -1.0]), 8.0)

    def test_product_of_elements(self):
        self.assertAlmostEqual(one("ProductOfElements", 4.0, mode="reciprocal"), 0.25)
        self.assertAlmostEqual(one("ProductOfElements", 3.0, mode="square"), 9.0)

    def test_dot_product_scalar(self):
        r = run([B("a", "Constant", value=3.0), B("b", "Constant", value=4.0),
                 B("d", "DotProduct"), B("o", "Outport")],
                [C("a", "out", "d", "in1"), C("b", "out", "d", "in2"), C("d", "out", "o", "in")])
        self.assertAlmostEqual(r["outputs"]["o"], 12.0)

    def test_sum_three_inputs(self):
        r = run([B("a", "Constant", value=1.0), B("b", "Constant", value=2.0), B("c", "Constant", value=3.0),
                 B("s", "Sum", inputs="+++"), B("o", "Outport")],
                [C("a", "out", "s", "in1"), C("b", "out", "s", "in2"), C("c", "out", "s", "in3"), C("s", "out", "o", "in")])
        self.assertAlmostEqual(r["outputs"]["o"], 6.0)

    def test_registry_coverage(self):
        from simpylink_backend.blocks import BLOCK_REGISTRY
        for t, spec in BLOCK_REGISTRY.items():
            for k in ("label", "inputs", "outputs", "params", "eval", "description", "category"):
                self.assertIn(k, spec, f"{t} missing {k}")
        self.assertIn("MinMax", BLOCK_REGISTRY)
        self.assertIn("TrigonometricFunction", BLOCK_REGISTRY)
        self.assertIn("Polynomial", BLOCK_REGISTRY)


if __name__ == "__main__":
    unittest.main()
