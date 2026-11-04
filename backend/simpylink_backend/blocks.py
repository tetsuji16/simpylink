"""Block registry: pure functions + metadata for the GUI palette.

Each entry maps a Simulink-ish type name to inputs/outputs port names,
parameter specs, category, and an eval function.
Eval signature: fn(inputs: dict[str, Any], params: dict) -> dict[str, Any]
Signal values are float (scalar) or numpy arrays (vector/matrix). Incompatible
broadcast shapes raise BlockError. See docs/protocol.md (JSON = nested lists).
"""
from __future__ import annotations

from typing import Any, Callable

import numpy as np

from .model import ModelError


class BlockError(ModelError):
    pass


def _num(params: dict, name: str, default: float) -> float:
    try:
        return float(params.get(name, default))
    except (TypeError, ValueError) as e:
        raise BlockError(f"param {name!r} must be a number: {e}") from e


def _str_param(params: dict, name: str, default: str) -> str:
    v = params.get(name, default)
    if not isinstance(v, str):
        raise BlockError(f"param {name!r} must be a string, got {type(v).__name__}")
    return v


def _one_in(inputs, name="in"):
    if name not in inputs:
        raise BlockError(f"missing input {name!r}")
    return inputs[name]


def _signs(spec: str, default: str = "++") -> list[str]:
    s = (spec or default).strip()
    if not s or any(c not in "+-" for c in s):
        raise BlockError(f"inputs spec must be +/- chars, got {spec!r}")
    return list(s)


def _ops(spec: str, default: str = "**") -> list[str]:
    s = (spec or default).strip() or default
    if any(c not in "*/" for c in s):
        raise BlockError(f"inputs spec must be */ chars, got {spec!r}")
    return list(s)


def _to_array(v):
    if isinstance(v, np.ndarray):
        return v
    return np.asarray(v, dtype=np.float64)


def _broadcast2(a, b):
    try:
        return np.broadcast_arrays(_to_array(a), _to_array(b))
    except ValueError as e:
        raise BlockError(f"shape mismatch ({np.shape(a)} vs {np.shape(b)}): {e}") from e


def _check_zero(a, what="denominator"):
    if np.any(_to_array(a) == 0):
        raise BlockError(f"division by zero ({what} contains 0)")


def ev_constant(inputs, params):
    v = params.get("value", 0.0)
    if isinstance(v, (list, tuple)):
        return {"out": np.asarray(v, dtype=np.float64)}
    return {"out": _num(params, "value", 0.0)}


def ev_inport(inputs, params):
    raise BlockError("Inport must be driven by simulate(inputs=...)")


def ev_outport(inputs, params):
    return {}


def ev_clock(inputs, params):
    raise BlockError("Clock requires time-stepped simulation (POST /simulate_time)")


def ev_sine_wave(inputs, params):
    raise BlockError("SineWaveFunction requires time-stepped simulation (POST /simulate_time)")


def ev_integrator(inputs, params):
    raise BlockError("Integrator requires time-stepped simulation (POST /simulate_time)")


def ev_unit_delay(inputs, params):
    raise BlockError("UnitDelay requires time-stepped simulation (POST /simulate_time)")


def ev_gain(inputs, params):
    return {"out": _to_array(_one_in(inputs)) * _num(params, "gain", 1.0)}


def ev_bias(inputs, params):
    return {"out": _to_array(_one_in(inputs)) + _num(params, "bias", 0.0)}


def ev_abs(inputs, params):
    return {"out": np.abs(_to_array(_one_in(inputs)))}


def ev_unary_minus(inputs, params):
    return {"out": -_to_array(_one_in(inputs))}


def ev_sign(inputs, params):
    return {"out": np.sign(_to_array(_one_in(inputs)))}


_TRIG = {
    "sin": np.sin, "cos": np.cos, "tan": np.tan,
    "asin": np.arcsin, "acos": np.arccos, "atan": np.arctan,
    "sinh": np.sinh, "cosh": np.cosh, "tanh": np.tanh,
}


def ev_trig(inputs, params):
    fn = _str_param(params, "function", "sin").lower()
    if fn not in _TRIG:
        raise BlockError(f"TrigonometricFunction: unknown function {fn!r} (known: {sorted(_TRIG)})")
    return {"out": _TRIG[fn](_to_array(_one_in(inputs)))}


def ev_math_function(inputs, params):
    fn = _str_param(params, "function", "exp").lower()
    x = _to_array(_one_in(inputs))
    if fn == "exp":
        return {"out": np.exp(x)}
    if fn == "log":
        if np.any(x <= 0):
            raise BlockError("MathFunction log: input must be > 0")
        return {"out": np.log(x)}
    if fn == "log10":
        if np.any(x <= 0):
            raise BlockError("MathFunction log10: input must be > 0")
        return {"out": np.log10(x)}
    if fn == "sqrt":
        if np.any(x < 0):
            raise BlockError("MathFunction sqrt: input must be >= 0")
        return {"out": np.sqrt(x)}
    if fn == "square":
        return {"out": x * x}
    if fn == "reciprocal":
        _check_zero(x, "reciprocal input")
        return {"out": 1.0 / x}
    if fn == "pow":
        return {"out": np.power(x, _num(params, "exponent", 2.0))}
    if fn == "abs":
        return {"out": np.abs(x)}
    raise BlockError(f"MathFunction: unknown function {fn!r} (known: exp, log, log10, sqrt, square, reciprocal, pow, abs)")


def ev_rounding(inputs, params):
    mode = _str_param(params, "mode", "round").lower()
    x = _to_array(_one_in(inputs))
    if mode == "floor":
        return {"out": np.floor(x)}
    if mode == "ceil":
        return {"out": np.ceil(x)}
    if mode == "round":
        return {"out": np.round(x)}
    if mode == "trunc":
        return {"out": np.trunc(x)}
    raise BlockError(f"RoundingFunction: unknown mode {mode!r} (known: floor, ceil, round, trunc)")


def ev_sqrt(inputs, params):
    mode = _str_param(params, "mode", "sqrt").lower()
    x = _to_array(_one_in(inputs))
    if mode == "sqrt":
        if np.any(x < 0):
            raise BlockError("Sqrt: negative input for sqrt mode")
        return {"out": np.sqrt(x)}
    if mode == "signed_sqrt":
        return {"out": np.sign(x) * np.sqrt(np.abs(x))}
    if mode == "reciprocal_sqrt":
        _check_zero(x, "reciprocal sqrt input")
        return {"out": 1.0 / np.sqrt(np.abs(x))}
    raise BlockError(f"Sqrt: unknown mode {mode!r} (known: sqrt, signed_sqrt, reciprocal_sqrt)")


def ev_polynomial(inputs, params):
    coeffs = params.get("coefficients")
    if not isinstance(coeffs, (list, tuple)) or not coeffs:
        raise BlockError("Polynomial: coefficients must be a non-empty list (highest degree first)")
    x = _to_array(_one_in(inputs))
    result = np.zeros_like(x, dtype=np.float64)
    for c in coeffs:
        result = result * x + float(c)
    return {"out": result}


def ev_product_of_elements(inputs, params):
    mode = _str_param(params, "mode", "identity").lower()
    x = _to_array(_one_in(inputs))
    if mode == "identity":
        return {"out": x}
    if mode == "reciprocal":
        _check_zero(x)
        return {"out": 1.0 / x}
    if mode == "square":
        return {"out": x * x}
    if mode == "cube":
        return {"out": x * x * x}
    if mode == "reciprocal_square":
        _check_zero(x)
        return {"out": 1.0 / (x * x)}
    if mode == "reciprocal_cube":
        _check_zero(x)
        return {"out": 1.0 / (x * x * x)}
    if mode == "exponentiation":
        return {"out": np.power(x, _num(params, "exponent", 2.0))}
    raise BlockError(f"ProductOfElements: unknown mode {mode!r}")


def ev_sum(inputs, params):
    signs = _signs(_str_param(params, "inputs", "++"))
    result = None
    for i, s in enumerate(signs, start=1):
        key = f"in{i}"
        if key not in inputs:
            raise BlockError(f"Sum: missing input {key}")
        v = _to_array(inputs[key]) if s == "+" else -_to_array(inputs[key])
        if result is None:
            result = v
        else:
            a, b = _broadcast2(result, v)
            result = a + b
    return {"out": result}


def ev_product(inputs, params):
    ops = _ops(_str_param(params, "inputs", "**"))
    result = None
    for i, op in enumerate(ops, start=1):
        key = f"in{i}"
        if key not in inputs:
            raise BlockError(f"Product: missing input {key}")
        v = _to_array(inputs[key])
        if result is None:
            result = v
            continue
        a, b = _broadcast2(result, v)
        if op == "*":
            result = a * b
        else:
            _check_zero(b)
            result = a / b
    return {"out": result}


def ev_divide(inputs, params):
    if "numerator" not in inputs or "denominator" not in inputs:
        raise BlockError("Divide: needs numerator and denominator inputs")
    a, b = _broadcast2(inputs["numerator"], inputs["denominator"])
    _check_zero(b)
    return {"out": a / b}


def ev_minmax(inputs, params):
    mode = _str_param(params, "mode", "min").lower()
    if mode not in ("min", "max"):
        raise BlockError(f"MinMax: mode must be min|max, got {mode!r}")
    keys = sorted(k for k in inputs if k.startswith("in"))
    if not keys:
        raise BlockError("MinMax: no inputs connected")
    result = _to_array(inputs[keys[0]])
    for k in keys[1:]:
        a, b = _broadcast2(result, inputs[k])
        result = np.minimum(a, b) if mode == "min" else np.maximum(a, b)
    return {"out": result}


def ev_dot_product(inputs, params):
    if "in1" not in inputs or "in2" not in inputs:
        raise BlockError("DotProduct: needs in1 and in2")
    a, b = _to_array(inputs["in1"]), _to_array(inputs["in2"])
    if a.ndim == 0 and b.ndim == 0:
        return {"out": float(a * b)}
    if a.shape != b.shape:
        raise BlockError(f"DotProduct: shape mismatch {a.shape} vs {b.shape}")
    return {"out": float(np.dot(a.ravel(), b.ravel()))}


def _concat(inputs):
    keys = sorted(k for k in inputs if k.startswith("in"))
    if not keys:
        raise BlockError("concatenate: no inputs connected")
    arrays = [_to_array(inputs[k]) for k in keys]
    try:
        return np.concatenate(arrays, axis=0)
    except ValueError as e:
        raise BlockError(f"concatenate: shapes {[a.shape for a in arrays]} incompatible: {e}") from e


def ev_vector_concat(inputs, params):
    return {"out": _concat(inputs)}


def ev_matrix_concat(inputs, params):
    return {"out": _concat(inputs)}


def ev_reshape(inputs, params):
    size = params.get("size")
    if not isinstance(size, (list, tuple)) or not size:
        raise BlockError("Reshape: size must be a non-empty list of ints")
    x = _to_array(_one_in(inputs))
    try:
        return {"out": np.reshape(x, tuple(int(s) for s in size), order="F")}
    except ValueError as e:
        raise BlockError(f"Reshape: cannot reshape {x.shape} to {size}: {e}") from e


def ev_permute(inputs, params):
    dims = params.get("dimensions")
    if not isinstance(dims, (list, tuple)) or not dims:
        raise BlockError("PermuteDimensions: dimensions must be a non-empty list (1-based)")
    x = _to_array(_one_in(inputs))
    perm = tuple(int(d) - 1 for d in dims)
    if sorted(perm) != list(range(x.ndim)):
        raise BlockError(f"PermuteDimensions: dimensions {dims} invalid for {x.ndim}D input")
    return {"out": np.transpose(x, axes=perm)}


def ev_squeeze(inputs, params):
    return {"out": np.squeeze(_to_array(_one_in(inputs)))}


EvalFn = Callable[[dict[str, Any], dict[str, Any]], dict[str, Any]]

_MATH = "Math Operations"
_SOURCES = "Sources"
_SINKS = "Sinks"

BLOCK_REGISTRY: dict[str, dict] = {
    "Constant": {"label": "Constant", "category": _SOURCES, "inputs": [], "outputs": ["out"],
                 "params": [{"name": "value", "type": "number", "default": 0.0, "label": "Value"}], "eval": ev_constant,
                 "description": "Emit a constant value."},
    "Inport": {"label": "Inport", "category": _SOURCES, "inputs": [], "outputs": ["out"],
               "params": [{"name": "port", "type": "integer", "default": 1, "label": "Port"}], "eval": ev_inport,
               "description": "Model input, value supplied via simulate(inputs=...)."},
    "Outport": {"label": "Outport", "category": _SINKS, "inputs": ["in"], "outputs": [],
                "params": [{"name": "port", "type": "integer", "default": 1, "label": "Port"}], "eval": ev_outport,
                "description": "Model output sink."},
    "Clock": {"label": "Clock", "category": _SOURCES, "inputs": [], "outputs": ["out"], "params": [],
              "eval": ev_clock, "description": "Simulation time (time-stepped mode)."},
    "SineWaveFunction": {"label": "Sine Wave Function", "category": _SOURCES, "inputs": [], "outputs": ["out"],
                         "params": [{"name": "amplitude", "type": "number", "default": 1.0, "label": "Amplitude"},
                                    {"name": "frequency", "type": "number", "default": 1.0, "label": "Frequency"},
                                    {"name": "phase", "type": "number", "default": 0.0, "label": "Phase (rad)"},
                                    {"name": "offset", "type": "number", "default": 0.0, "label": "Offset"}],
                         "eval": ev_sine_wave, "description": "Sine wave source (time-stepped mode)."},
    "Integrator": {"label": "Integrator", "category": _MATH, "inputs": ["in"], "outputs": ["out"],
                   "params": [{"name": "initial_condition", "type": "number", "default": 0.0, "label": "Initial condition"}],
                   "eval": ev_integrator, "description": "Integrate input over time (time-stepped mode)."},
    "UnitDelay": {"label": "Unit Delay", "category": _MATH, "inputs": ["in"], "outputs": ["out"],
                  "params": [{"name": "initial_condition", "type": "number", "default": 0.0, "label": "Initial condition"}],
                  "eval": ev_unit_delay, "description": "Delay input by one sample (time-stepped mode)."},
    "Gain": {"label": "Gain", "category": _MATH, "inputs": ["in"], "outputs": ["out"],
             "params": [{"name": "gain", "type": "number", "default": 1.0, "label": "Gain"}], "eval": ev_gain,
             "description": "Multiply input by a constant (Simulink Gain)."},
    "Bias": {"label": "Bias", "category": _MATH, "inputs": ["in"], "outputs": ["out"],
             "params": [{"name": "bias", "type": "number", "default": 0.0, "label": "Bias"}], "eval": ev_bias,
             "description": "Add a bias to the input (Simulink Bias)."},
    "Abs": {"label": "Abs", "category": _MATH, "inputs": ["in"], "outputs": ["out"], "params": [], "eval": ev_abs,
            "description": "Absolute value of the input."},
    "UnaryMinus": {"label": "Unary Minus", "category": _MATH, "inputs": ["in"], "outputs": ["out"], "params": [], "eval": ev_unary_minus,
                   "description": "Negate the input."},
    "Sign": {"label": "Sign", "category": _MATH, "inputs": ["in"], "outputs": ["out"], "params": [], "eval": ev_sign,
             "description": "Sign of the input (-1/0/1)."},
    "Sum": {"label": "Sum", "category": _MATH, "inputs": ["in1", "in2"], "outputs": ["out"],
            "params": [{"name": "inputs", "type": "string", "default": "++", "label": "Inputs (+/-)"}], "eval": ev_sum,
            "description": "Add/subtract inputs per +/- spec."},
    "Product": {"label": "Product", "category": _MATH, "inputs": ["in1", "in2"], "outputs": ["out"],
                "params": [{"name": "inputs", "type": "string", "default": "**", "label": "Inputs (*/)"}], "eval": ev_product,
                "description": "Multiply/divide inputs per */ spec."},
    "Divide": {"label": "Divide", "category": _MATH, "inputs": ["numerator", "denominator"], "outputs": ["out"],
               "params": [], "eval": ev_divide, "description": "Numerator divided by denominator."},
    "MinMax": {"label": "MinMax", "category": _MATH, "inputs": ["in1", "in2"], "outputs": ["out"],
               "params": [{"name": "mode", "type": "string", "default": "min", "label": "Mode (min/max)"}], "eval": ev_minmax,
               "description": "Minimum or maximum of the inputs (Simulink MinMax)."},
    "TrigonometricFunction": {"label": "Trigonometric Function", "category": _MATH, "inputs": ["in"], "outputs": ["out"],
                              "params": [{"name": "function", "type": "string", "default": "sin", "label": "Function"}],
                              "eval": ev_trig, "description": "Trigonometric/hyperbolic function of the input."},
    "MathFunction": {"label": "Math Function", "category": _MATH, "inputs": ["in"], "outputs": ["out"],
                     "params": [{"name": "function", "type": "string", "default": "exp", "label": "Function"},
                                {"name": "exponent", "type": "number", "default": 2.0, "label": "Exponent (pow)"}],
                     "eval": ev_math_function, "description": "Mathematical function of the input."},
    "RoundingFunction": {"label": "Rounding Function", "category": _MATH, "inputs": ["in"], "outputs": ["out"],
                         "params": [{"name": "mode", "type": "string", "default": "round", "label": "Mode"}],
                         "eval": ev_rounding, "description": "Round the input (floor/ceil/round/trunc)."},
    "Sqrt": {"label": "Sqrt", "category": _MATH, "inputs": ["in"], "outputs": ["out"],
             "params": [{"name": "mode", "type": "string", "default": "sqrt", "label": "Mode"}],
             "eval": ev_sqrt, "description": "Square root, signed square root, or reciprocal square root."},
    "Polynomial": {"label": "Polynomial", "category": _MATH, "inputs": ["in"], "outputs": ["out"],
                   "params": [{"name": "coefficients", "type": "array", "default": [1.0, 0.0], "label": "Coefficients (highest first)"}],
                   "eval": ev_polynomial, "description": "Evaluate polynomial at the input."},
    "ProductOfElements": {"label": "Product of Elements", "category": _MATH, "inputs": ["in"], "outputs": ["out"],
                          "params": [{"name": "mode", "type": "string", "default": "identity", "label": "Mode"},
                                     {"name": "exponent", "type": "number", "default": 2.0, "label": "Exponent"}],
                          "eval": ev_product_of_elements, "description": "Elementwise power/reciprocal of the input."},
    "DotProduct": {"label": "Dot Product", "category": _MATH, "inputs": ["in1", "in2"], "outputs": ["out"], "params": [],
                   "eval": ev_dot_product, "description": "Dot product of two vectors."},
    "VectorConcatenate": {"label": "Vector Concatenate", "category": _MATH, "inputs": ["in1", "in2"], "outputs": ["out"], "params": [],
                          "eval": ev_vector_concat, "description": "Concatenate input vectors."},
    "MatrixConcatenate": {"label": "Matrix Concatenate", "category": _MATH, "inputs": ["in1", "in2"], "outputs": ["out"], "params": [],
                          "eval": ev_matrix_concat, "description": "Concatenate input matrices along rows."},
    "Reshape": {"label": "Reshape", "category": _MATH, "inputs": ["in"], "outputs": ["out"],
                "params": [{"name": "size", "type": "array", "default": [1], "label": "Size"}],
                "eval": ev_reshape, "description": "Reshape the input to the given size (column-major)."},
    "PermuteDimensions": {"label": "Permute Dimensions", "category": _MATH, "inputs": ["in"], "outputs": ["out"],
                          "params": [{"name": "dimensions", "type": "array", "default": [1], "label": "Dimensions (1-based)"}],
                          "eval": ev_permute, "description": "Permute the dimensions of the input."},
    "Squeeze": {"label": "Squeeze", "category": _MATH, "inputs": ["in"], "outputs": ["out"], "params": [],
                "eval": ev_squeeze, "description": "Remove dimensions of size 1."},
}


def block_public(spec: dict) -> dict:
    return {k: v for k, v in spec.items() if k != "eval"}


def get_block(type_name: str) -> dict:
    try:
        return BLOCK_REGISTRY[type_name]
    except KeyError:
        raise BlockError(f"unknown block type: {type_name!r} (known: {sorted(BLOCK_REGISTRY)})") from None


def register_block(type_name: str, spec: dict) -> None:
    """Register a plugin block (see simpylink_backend/plugins.py)."""
    if not isinstance(spec, dict) or "eval" not in spec or "inputs" not in spec or "outputs" not in spec:
        raise BlockError(f"plugin block {type_name!r} needs eval/inputs/outputs")
    spec.setdefault("label", type_name)
    spec.setdefault("category", "Custom")
    spec.setdefault("params", [])
    spec.setdefault("description", "")
    BLOCK_REGISTRY[type_name] = spec


def to_jsonable(v):
    """Convert a signal value to JSON-serializable Python (float / nested lists)."""
    if isinstance(v, np.ndarray):
        if v.ndim == 0:
            return float(v)
        return v.tolist()
    if isinstance(v, (np.floating, np.integer)):
        return float(v)
    return v
