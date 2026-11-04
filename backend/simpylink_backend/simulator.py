"""One-shot simulator: topological evaluation of the connection graph.

Signals are float (scalar) or numpy arrays. Outputs are converted to
JSON-serializable values (float / nested lists) via blocks.to_jsonable.
"""
from __future__ import annotations

import re
from collections import defaultdict, deque
from typing import Any

import numpy as np

from .blocks import BlockError, get_block, to_jsonable
from .model import Model, ModelError


class AlgebraicLoopError(ModelError):
    pass


_VARIABLE_INPUT_TYPES = {"Sum", "Product", "MinMax", "VectorConcatenate", "MatrixConcatenate"}


def _topo_order(model: Model) -> list[str]:
    indegree: dict[str, int] = {b.id: 0 for b in model.blocks}
    adj: dict[str, list[str]] = defaultdict(list)
    for c in model.connections:
        adj[c.source.block].append(c.target.block)
        indegree[c.target.block] += 1
    q = deque(sorted(bid for bid, d in indegree.items() if d == 0))
    order: list[str] = []
    while q:
        bid = q.popleft()
        order.append(bid)
        for nxt in adj[bid]:
            indegree[nxt] -= 1
            if indegree[nxt] == 0:
                q.append(nxt)
    if len(order) != len(model.blocks):
        remaining = sorted(set(indegree) - set(order))
        raise AlgebraicLoopError(f"algebraic loop involving blocks: {remaining}")
    return order


def _valid_target_port(block_type: str, port: str, spec: dict) -> bool:
    if port in spec["inputs"]:
        return True
    if block_type in _VARIABLE_INPUT_TYPES and re.fullmatch(r"in\d+", port):
        return True
    return False


def _required_inputs(block) -> list[str] | None:
    """None means: at least one inN connected (checked later)."""
    if block.type == "Sum":
        spec_s = str(block.params.get("inputs", "++"))
        return [f"in{i}" for i in range(1, len(spec_s) + 1)] if spec_s else ["in1", "in2"]
    if block.type == "Product":
        spec_s = str(block.params.get("inputs", "**"))
        return [f"in{i}" for i in range(1, len(spec_s) + 1)] if spec_s else ["in1", "in2"]
    if block.type in ("MinMax", "VectorConcatenate", "MatrixConcatenate"):
        return None
    return None  # caller falls back to spec["inputs"]


def simulate(model: Model, inputs: dict | None = None) -> dict:
    """Evaluate model. Returns {"outputs": {...}, "trace": [...], "status": "ok"}."""
    inputs = inputs or {}
    block_map = model.block_map()
    incoming: dict[tuple[str, str], str] = {}
    wiring: dict[str, dict[str, tuple[str, str]]] = defaultdict(dict)
    for c in model.connections:
        src_spec = get_block(block_map[c.source.block].type)
        dst_spec = get_block(block_map[c.target.block].type)
        if c.source.port not in src_spec["outputs"]:
            raise BlockError(f"{c.source.block}.{c.source.port} is not an output of {block_map[c.source.block].type}")
        if not _valid_target_port(block_map[c.target.block].type, c.target.port, dst_spec):
            raise BlockError(f"{c.target.block}.{c.target.port} is not an input of {block_map[c.target.block].type}")
        if (c.target.block, c.target.port) in incoming:
            raise BlockError(f"input {c.target.block}.{c.target.port} is driven more than once")
        incoming[(c.target.block, c.target.port)] = c.source.block
        wiring[c.target.block][c.target.port] = (c.source.block, c.source.port)

    values: dict[tuple[str, str], Any] = {}
    outputs: dict[str, Any] = {}
    trace: list[dict[str, Any]] = []

    for bid in _topo_order(model):
        block = block_map[bid]
        spec = get_block(block.type)
        in_vals: dict[str, Any] = {}
        for port, (src_bid, src_port) in wiring.get(bid, {}).items():
            if (src_bid, src_port) not in values:
                raise BlockError(f"upstream block {src_bid!r} produced no {src_port!r}")
            in_vals[port] = values[(src_bid, src_port)]
        if block.type == "Inport":
            if bid not in inputs:
                raise BlockError(f"Inport {bid!r}: no value in simulate(inputs=...)")
            v = inputs[bid]
            out = {"out": np.asarray(v, dtype=np.float64) if isinstance(v, (list, tuple)) else float(v)}
        elif block.type == "Outport":
            if "in" not in in_vals:
                raise BlockError(f"Outport {bid!r}: input not connected")
            outputs[bid] = to_jsonable(in_vals["in"])
            trace.append({"block": bid, "output": to_jsonable(in_vals["in"])})
            continue
        else:
            required = _required_inputs(block)
            if required is None:
                required = list(spec["inputs"])
            for port in required:
                if port not in in_vals:
                    raise BlockError(f"{block.type} block {bid!r}: input {port!r} not connected")
            out = spec["eval"](in_vals, block.params)
        for port, v in out.items():
            values[(bid, port)] = v
        trace.append({"block": bid, "output": to_jsonable(next(iter(out.values()), None))})

    return {"status": "ok", "outputs": outputs, "trace": trace}
