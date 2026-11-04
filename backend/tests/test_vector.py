"""Golden tests for vector/matrix signals (numpy)."""
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


class TestVector(unittest.TestCase):
    def test_constant_list_gain(self):
        r = run([B("c", "Constant", value=[1.0, 2.0, 3.0]), B("g", "Gain", gain=2.0), B("o", "Outport")],
                [C("c", "out", "g", "in"), C("g", "out", "o", "in")])
        self.assertEqual(r["outputs"]["o"], [2.0, 4.0, 6.0])

    def test_scalar_broadcast(self):
        r = run([B("a", "Constant", value=[1.0, 2.0, 3.0]), B("b", "Constant", value=10.0),
                 B("s", "Sum", inputs="++"), B("o", "Outport")],
                [C("a", "out", "s", "in1"), C("b", "out", "s", "in2"), C("s", "out", "o", "in")])
        self.assertEqual(r["outputs"]["o"], [11.0, 12.0, 13.0])

    def test_shape_mismatch(self):
        with self.assertRaises(Exception):
            run([B("a", "Constant", value=[1.0, 2.0]), B("b", "Constant", value=[1.0, 2.0, 3.0]),
                 B("s", "Sum", inputs="++"), B("o", "Outport")],
                [C("a", "out", "s", "in1"), C("b", "out", "s", "in2"), C("s", "out", "o", "in")])

    def test_vector_concatenate_and_dot(self):
        r = run([B("a", "Constant", value=[1.0, 2.0]), B("b", "Constant", value=[3.0, 4.0]),
                 B("v", "VectorConcatenate"), B("o", "Outport")],
                [C("a", "out", "v", "in1"), C("b", "out", "v", "in2"), C("v", "out", "o", "in")])
        self.assertEqual(r["outputs"]["o"], [1.0, 2.0, 3.0, 4.0])
        r = run([B("a", "Constant", value=[1.0, 2.0]), B("b", "Constant", value=[3.0, 4.0]),
                 B("d", "DotProduct"), B("o", "Outport")],
                [C("a", "out", "d", "in1"), C("b", "out", "d", "in2"), C("d", "out", "o", "in")])
        self.assertAlmostEqual(r["outputs"]["o"], 11.0)

    def test_matrix_reshape_permute_squeeze(self):
        r = run([B("c", "Constant", value=[[1.0, 2.0], [3.0, 4.0]]), B("r", "Reshape", size=[4]), B("o", "Outport")],
                [C("c", "out", "r", "in"), C("r", "out", "o", "in")])
        # column-major reshape: [[1,2],[3,4]] -> [1,3,2,4]
        self.assertEqual(r["outputs"]["o"], [1.0, 3.0, 2.0, 4.0])
        r = run([B("c", "Constant", value=[[1.0, 2.0], [3.0, 4.0]]), B("p", "PermuteDimensions", dimensions=[2, 1]), B("o", "Outport")],
                [C("c", "out", "p", "in"), C("p", "out", "o", "in")])
        self.assertEqual(r["outputs"]["o"], [[1.0, 3.0], [2.0, 4.0]])
        r = run([B("c", "Constant", value=[[[1.0], [2.0]]]), B("s", "Squeeze"), B("o", "Outport")],
                [C("c", "out", "s", "in"), C("s", "out", "o", "in")])
        self.assertEqual(r["outputs"]["o"], [1.0, 2.0])


if __name__ == "__main__":
    unittest.main()
