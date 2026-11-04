"""Block registry: pure scalar functions + metadata for the GUI palette.

Each entry maps a Simulink-ish type name to:
  inputs/outputs port names, parameter specs, and an eval function.
Eval signature: fn(inputs: dict[str, float], params: dict) -> dict[str, float]
"""
from __future__ import annotations

import math
from typing import Any, Callable

from .model import ModelError


class BlockError(ModelError):
    pass


def _num(params: dict, name: str, default: float) -> float:
    try:
        return float(params.get(name, default))
    except (TypeError, ValueError) as e:
        raise BlockError(f"param {name!r} must be a number: {e}") from e


def _one_in(inputs, name="in"):
    if name not in inputs:
        raise BlockError(f"missing input {name!r}")
    return float(inputs[name])


def _signs(spec: str, default: str = "++") -> list[str]:
    s = (spec or default).strip()
    if not s or any(c not in "+-" for c in s):
        raise BlockError(f"inputs spec must be +/- chars, got {spec!r}")
    return list(s)


# --- block eval functions -------------------------------------------------
def ev_constant(inputs, params): return {"out": _num(params, "value", 0.0)}
def ev_inport(inputs, params):  # value injected by simulator from `inputs` payload
    raise BlockError("Inport must be driven by simulate(inputs=...)")
def ev_outport(inputs, params): return {}  # sink; simulator records its input
def ev_gain(inputs, params):    return {"out": _one_in(inputs) * _num(params, "gain", 1.0)}
def ev_bias(inputs, params):    return {"out": _one_in(inputs) + _num(params, "bias", 0.0)}
def ev_abs(inputs, params):     return {"out": abs(_one_in(inputs))}
def ev_unary_minus(inputs, params): return {"out": -_one_in(inputs)}


def ev_sum(inputs, params):
    signs = _signs(str(params.get("inputs", "++")))
    total = 0.0
    for i, s in enumerate(signs, start=1):
        key = f"in{i}"
        if key not in inputs:
            raise BlockError(f"Sum: missing input {key}")
        total += float(inputs[key]) if s == "+" else -float(inputs[key])
    return {"out": total}


def ev_product(inputs, params):
    spec = str(params.get("inputs", "**")).strip() or "**"
    if any(c not in "*/" for c in spec):
        raise BlockError(f"Product inputs spec must be */ chars, got {spec!r}")
    result = None
    for i, op in enumerate(spec, start=1):
        key = f"in{i}"
        if key not in inputs:
            raise BlockError(f"Product: missing input {key}")
        v = float(inputs[key])
        if result is None:
            result = v
        elif op == "*":
            result *= v
        else:
            if v == 0.0:
                raise BlockError("Product: division by zero")
            result /= v
    return {"out": float(result)}


def ev_divide(inputs, params):
    num, den = float(inputs.get("numerator", math.nan)), float(inputs.get("denominator", math.nan))
    if "numerator" not in inputs or "denominator" not in inputs:
        raise BlockError("Divide: needs numerator and denominator inputs")
    if den == 0.0:
        raise BlockError("Divide: division by zero")
    return {"out": num / den}


EvalFn = Callable[[dict[str, float], dict[str, Any]], dict[str, float]]

BLOCK_REGISTRY: dict[str, dict] = {
    "Constant":    {"label": "Constant",    "inputs": [], "outputs": ["out"],
                    "params": [{"name": "value", "type": "number", "default": 0.0, "label": "Value"}], "eval": ev_constant,
                    "description": "Emit a constant value."},
    "Inport":      {"label": "Inport",      "inputs": [], "outputs": ["out"],
                    "params": [{"name": "port", "type": "integer", "default": 1, "label": "Port"}], "eval": ev_inport,
                    "description": "Model input, value supplied via simulate(inputs=...)."},
    "Outport":     {"label": "Outport",     "inputs": ["in"], "outputs": [],
                    "params": [{"name": "port", "type": "integer", "default": 1, "label": "Port"}], "eval": ev_outport,
                    "description": "Model output sink."},
    "Gain":        {"label": "Gain",        "inputs": ["in"], "outputs": ["out"],
                    "params": [{"name": "gain", "type": "number", "default": 1.0, "label": "Gain"}], "eval": ev_gain,
                    "description": "Multiply input by a constant (Simulink Gain)."},
    "Bias":        {"label": "Bias",        "inputs": ["in"], "outputs": ["out"],
                    "params": [{"name": "bias", "type": "number", "default": 0.0, "label": "Bias"}], "eval": ev_bias,
                    "description": "Add a bias to the input (Simulink Bias)."},
    "Abs":         {"label": "Abs",         "inputs": ["in"], "outputs": ["out"], "params": [], "eval": ev_abs,
                    "description": "Absolute value of the input."},
    "UnaryMinus":  {"label": "Unary Minus", "inputs": ["in"], "outputs": ["out"], "params": [], "eval": ev_unary_minus,
                    "description": "Negate the input."},
    "Sum":         {"label": "Sum",         "inputs": ["in1", "in2"], "outputs": ["out"],
                    "params": [{"name": "inputs", "type": "string", "default": "++", "label": "Inputs (+/-)"}], "eval": ev_sum,
                    "description": "Add/subtract inputs per +/- spec."},
    "Product":     {"label": "Product",     "inputs": ["in1", "in2"], "outputs": ["out"],
                    "params": [{"name": "inputs", "type": "string", "default": "**", "label": "Inputs (*/)"}], "eval": ev_product,
                    "description": "Multiply/divide inputs per */ spec."},
    "Divide":      {"label": "Divide",      "inputs": ["numerator", "denominator"], "outputs": ["out"],
                    "params": [], "eval": ev_divide,
                    "description": "Numerator divided by denominator."},
}


def block_public(spec: dict) -> dict:
    return {k: v for k, v in spec.items() if k != "eval"}


def get_block(type_name: str) -> dict:
    try:
        return BLOCK_REGISTRY[type_name]
    except KeyError:
        raise BlockError(f"unknown block type: {type_name!r} (known: {sorted(BLOCK_REGISTRY)})") from None
