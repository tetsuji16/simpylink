"""One-shot scalar simulator: topological evaluation of the connection graph."""
from __future__ import annotations

from collections import defaultdict, deque
from typing import Any

from .blocks import BLOCK_REGISTRY, BlockError, get_block
from .model import Model, ModelError


class AlgebraicLoopError(ModelError):
    pass


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


def simulate(model: Model, inputs: dict[str, float] | None = None) -> dict:
    """Evaluate model. Returns {"outputs": {outport_id: value}, "trace": [...]}."""
    inputs = inputs or {}
    # validate block types + ports up front
    incoming: dict[tuple[str, str], str] = {}  # (dst_block, dst_port) -> src_block
    wiring: dict[str, dict[str, tuple[str, str]]] = defaultdict(dict)  # dst -> port -> (src, sport)
    for c in model.connections:
        src_spec = get_block(model.block_map()[c.source.block].type)
        dst_spec = get_block(model.block_map()[c.target.block].type)
        if c.source.port not in src_spec["outputs"]:
            raise BlockError(f"{c.source.block}.{c.source.port} is not an output of {src_spec}")
        if c.target.port not in dst_spec["inputs"]:
            raise BlockError(f"{c.target.block}.{c.target.port} is not an input of {dst_spec}")
        if (c.target.block, c.target.port) in incoming:
            raise BlockError(f"input {c.target.block}.{c.target.port} is driven more than once")
        incoming[(c.target.block, c.target.port)] = c.source.block
        wiring[c.target.block][c.target.port] = (c.source.block, c.source.port)

    values: dict[tuple[str, str], float] = {}  # (block, out_port) -> value
    outputs: dict[str, float] = {}
    trace: list[dict[str, Any]] = []

    for bid in _topo_order(model):
        block = model.block_map()[bid]
        spec = get_block(block.type)
        in_vals: dict[str, float] = {}
        for port, (src_bid, src_port) in wiring.get(bid, {}).items():
            if (src_bid, src_port) not in values:
                raise BlockError(f"upstream block {src_bid!r} produced no {src_port!r}")
            in_vals[port] = values[(src_bid, src_port)]
        if block.type == "Inport":
            key = block.id
            if key not in inputs:
                raise BlockError(f"Inport {bid!r}: no value in simulate(inputs=...)")
            out = {"out": float(inputs[key])}
        elif block.type == "Outport":
            if "in" not in in_vals:
                raise BlockError(f"Outport {bid!r}: input not connected")
            outputs[bid] = in_vals["in"]
            trace.append({"block": bid, "output": in_vals["in"]})
            continue
        else:
            for port in spec["inputs"]:
                if port not in in_vals:
                    raise BlockError(f"{block.type} block {bid!r}: input {port!r} not connected")
            out = spec["eval"](in_vals, block.params)
        for port, v in out.items():
            values[(bid, port)] = float(v)
        trace.append({"block": bid, "output": next(iter(out.values()), None)})

    return {"status": "ok", "outputs": outputs, "trace": trace}
