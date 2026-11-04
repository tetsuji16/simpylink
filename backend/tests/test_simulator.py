"""Golden tests: hand-computed expected values. Run: python -m unittest discover -s backend/tests -v"""
import os
import sys
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

from simpylink_backend.model import Model
from simpylink_backend.simulator import simulate, AlgebraicLoopError


def run(blocks, conns, inputs=None):
    return simulate(Model.from_dict({"name": "t", "blocks": blocks, "connections": conns}), inputs or {})


def B(bid, typ, **params):
    return {"id": bid, "type": typ, "params": params}


def C(src, sport, dst, dport):
    return {"from": {"block": src, "port": sport}, "to": {"block": dst, "port": dport}}


class TestArithmetic(unittest.TestCase):
    def test_gain(self):
        r = run([B("c", "Constant", value=3.0), B("g", "Gain", gain=2.5), B("o", "Outport")],
                [C("c", "out", "g", "in"), C("g", "out", "o", "in")])
        self.assertAlmostEqual(r["outputs"]["o"], 7.5)

    def test_sum_pm(self):
        r = run([B("a", "Constant", value=10.0), B("b", "Constant", value=4.0),
                 B("s", "Sum", inputs="+--") if False else B("s", "Sum", inputs="+-"), B("o", "Outport")],
                [C("a", "out", "s", "in1"), C("b", "out", "s", "in2"), C("s", "out", "o", "in")])
        self.assertAlmostEqual(r["outputs"]["o"], 6.0)

    def test_product_divide(self):
        r = run([B("a", "Constant", value=12.0), B("b", "Constant", value=3.0),
                 B("p", "Product", inputs="*/"), B("o", "Outport")],
                [C("a", "out", "p", "in1"), C("b", "out", "p", "in2"), C("p", "out", "o", "in")])
        self.assertAlmostEqual(r["outputs"]["o"], 4.0)

    def test_divide_block(self):
        r = run([B("a", "Constant", value=1.0), B("b", "Constant", value=4.0),
                 B("d", "Divide"), B("o", "Outport")],
                [C("a", "out", "d", "numerator"), C("b", "out", "d", "denominator"), C("d", "out", "o", "in")])
        self.assertAlmostEqual(r["outputs"]["o"], 0.25)

    def test_abs_unary_bias(self):
        r = run([B("c", "Constant", value=-5.0), B("a", "Abs"), B("u", "UnaryMinus"),
                 B("b", "Bias", bias=1.0), B("o", "Outport")],
                [C("c", "out", "a", "in"), C("a", "out", "u", "in"), C("u", "out", "b", "in"), C("b", "out", "o", "in")])
        self.assertAlmostEqual(r["outputs"]["o"], -4.0)

    def test_inport(self):
        r = run([B("i", "Inport"), B("o", "Outport")], [C("i", "out", "o", "in")], inputs={"i": 42.0})
        self.assertAlmostEqual(r["outputs"]["o"], 42.0)

    def test_divide_by_zero(self):
        with self.assertRaises(Exception):
            run([B("a", "Constant", value=1.0), B("b", "Constant", value=0.0),
                 B("d", "Divide"), B("o", "Outport")],
                [C("a", "out", "d", "numerator"), C("b", "out", "d", "denominator"), C("d", "out", "o", "in")])

    def test_algebraic_loop(self):
        with self.assertRaises(AlgebraicLoopError):
            run([B("g1", "Gain"), B("g2", "Gain"), B("o", "Outport")],
                [C("g1", "out", "g2", "in"), C("g2", "out", "g1", "in"), C("g1", "out", "o", "in")])


if __name__ == "__main__":
    unittest.main()
