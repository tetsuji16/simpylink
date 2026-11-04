"""Tests for fixed-step time simulation."""
import math
import os
import sys
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

from simpylink_backend.model import Model
from simpylink_backend.time_simulator import simulate_time


def run(blocks, conns, **kw):
    return simulate_time(Model.from_dict({"name": "t", "blocks": blocks, "connections": conns}), **kw)


def B(bid, typ, **params):
    return {"id": bid, "type": typ, "params": params}


def C(src, sport, dst, dport):
    return {"from": {"block": src, "port": sport}, "to": {"block": dst, "port": dport}}


class TestTimeSim(unittest.TestCase):
    def test_integrator_of_constant(self):
        r = run([B("c", "Constant", value=1.0), B("i", "Integrator", initial_condition=0.0), B("o", "Outport")],
                [C("c", "out", "i", "in"), C("i", "out", "o", "in")], t_start=0.0, t_stop=1.0, dt=0.1)
        self.assertEqual(len(r["time"]), 11)
        self.assertAlmostEqual(r["outputs"]["o"][-1], 1.0, places=9)

    def test_sine_wave(self):
        r = run([B("s", "SineWaveFunction", amplitude=1.0, frequency=1.0, phase=0.0, offset=0.0), B("o", "Outport")],
                [C("s", "out", "o", "in")], t_start=0.0, t_stop=1.0, dt=0.25)
        for t, v in zip(r["time"], r["outputs"]["o"]):
            self.assertAlmostEqual(v, math.sin(2 * math.pi * t), places=9)

    def test_unit_delay(self):
        r = run([B("i", "Inport"), B("d", "UnitDelay", initial_condition=0.0), B("o", "Outport")],
                [C("i", "out", "d", "in"), C("d", "out", "o", "in")],
                t_start=0.0, t_stop=0.2, dt=0.1, inputs={"i": [1.0, 2.0, 3.0]})
        self.assertEqual(r["outputs"]["o"], [0.0, 1.0, 2.0])

    def test_clock(self):
        r = run([B("c", "Clock"), B("o", "Outport")], [C("c", "out", "o", "in")],
                t_start=0.0, t_stop=0.2, dt=0.1)
        self.assertEqual(r["outputs"]["o"], [0.0, 0.1, 0.2])

    def test_bad_dt(self):
        with self.assertRaises(Exception):
            run([B("o", "Outport")], [], t_start=0.0, t_stop=1.0, dt=0.0)


if __name__ == "__main__":
    unittest.main()
