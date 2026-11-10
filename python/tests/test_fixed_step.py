"""The generated fixed-step integrator matches the Butcher tables (Issue #159)."""
import importlib.util

import numpy as np
import pytest

from conftest import EXPECTED_DIR


def load_script(name):
    """Import a generated script (its top-level code only defines functions)."""
    path = EXPECTED_DIR / name
    spec = importlib.util.spec_from_file_location(path.stem, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


# dx/dt = 1 - x, x(0) = 0, four steps of h = 0.5 (DES-07 §2.3).
EXPECTED = {
    "ode1": 0.9375,
    "ode2": 0.847412109375,
    "ode3": 0.8667623260874807,
    "ode4": 0.8644502294928204,
    "ode5": 0.8646595413005078,
}


@pytest.mark.parametrize("method,expected", sorted(EXPECTED.items()))
def test_fixed_step_tableaus(method, expected):
    script = load_script("first_order_lag.py")

    def f(_t, x):
        return np.array([1.0 - x[0]])

    x = np.array([0.0])
    for _ in range(4):
        x = script.fixed_step(f, 0.0, 0.5, x, method)
    assert x[0] == pytest.approx(expected, abs=1e-12)


def test_fixed_methods_are_listed():
    script = load_script("first_order_lag.py")
    assert script.FIXED_METHODS == ("ode1", "ode2", "ode3", "ode4", "ode5")
    assert script.FIXED_STEP is None
