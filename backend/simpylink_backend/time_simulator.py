"""Fixed-step time simulation: reuses stateless block evals, adds state.

Stateful blocks: Integrator (x += u*dt), UnitDelay (previous input).
Sources: Clock (t), SineWaveFunction (A*sin(2*pi*f*t+phase)+offset).
All other blocks are evaluated per step with scalar values. Inport values may
be a scalar (held) or a list of length N (per-step).
"""
from __future__ import annotations

import math
from typing import Any

import numpy as np

from .blocks import BlockError, _num, _to_array, get_block, to_jsonable
from .model import Model
from .simulator import AlgebraicLoopError, _valid_target_port
from collections import defaultdict, deque


def _topo_order_time(model):
    """Topo order where UnitDelay inputs don't constrain order (state breaks the loop)."""
    indegree = {b.id: 0 for b in model.blocks}
    adj = defaultdict(list)
    bmap = model.block_map()
    for c in model.connections:
        if bmap[c.target.block].type == "UnitDelay":
            continue  # UnitDelay output comes from stored state, not this step
        adj[c.source.block].append(c.target.block)
        indegree[c.target.block] += 1
    q = deque(sorted(b for b, d in indegree.items() if d == 0))
    order = []
    while q:
        bid = q.popleft()
        order.append(bid)
        for nxt in adj[bid]:
            indegree[nxt] -= 1
            if indegree[nxt] == 0:
                q.append(nxt)
    if len(order) != len(model.blocks):
        raise AlgebraicLoopError(f"algebraic loop involving blocks: {sorted(set(indegree) - set(order))}")
    return order
from collections import defaultdict


def simulate_time(model: Model, t_start: float = 0.0, t_stop: float = 1.0,
                  dt: float = 0.1, inputs: dict | None = None) -> dict:
    inputs = inputs or {}
    if dt <= 0:
        raise BlockError("simulate_time: dt must be > 0")
    if t_stop <= t_start:
        raise BlockError("simulate_time: t_stop must be > t_start")
    n_steps = int(round((t_stop - t_start) / dt)) + 1
    if n_steps > 1_000_000:
        raise BlockError(f"simulate_time: too many steps ({n_steps}); increase dt")
    times = [t_start + i * dt for i in range(n_steps)]

    block_map = model.block_map()
    wiring: dict[str, dict[str, tuple[str, str]]] = defaultdict(dict)
    incoming: dict[tuple[str, str], str] = {}
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

    order = _topo_order_time(model)
    # per-block state
    integrator_state: dict[str, float] = {}
    delay_state: dict[str, Any] = {}
    for bid in order:
        b = block_map[bid]
        if b.type == "Integrator":
            integrator_state[bid] = _num(b.params, "initial_condition", 0.0)
        if b.type == "UnitDelay":
            delay_state[bid] = _num(b.params, "initial_condition", 0.0)

    series: dict[str, list] = {bid: [] for bid in order if block_map[bid].type == "Outport"}
    delay_pending: dict[str, Any] = {}

    for step, t in enumerate(times):
        values: dict[tuple[str, str], Any] = {}
        step_outs: dict[str, Any] = {}
        for bid in order:
            block = block_map[bid]
            spec = get_block(block.type)
            in_vals: dict[str, Any] = {}
            for port, (src_bid, src_port) in wiring.get(bid, {}).items():
                if block.type == "UnitDelay":
                    continue  # input is evaluated later in topo order; state was used this step
                if (src_bid, src_port) not in values:
                    raise BlockError(f"upstream block {src_bid!r} produced no {src_port!r} at t={t}")
                in_vals[port] = values[(src_bid, src_port)]
            if block.type == "Clock":
                out = {"out": float(t)}
            elif block.type == "SineWaveFunction":
                amp = _num(block.params, "amplitude", 1.0)
                freq = _num(block.params, "frequency", 1.0)
                phase = _num(block.params, "phase", 0.0)
                offset = _num(block.params, "offset", 0.0)
                out = {"out": float(amp * math.sin(2 * math.pi * freq * t + phase) + offset)}
            elif block.type == "Inport":
                if bid not in inputs:
                    # unconnected Inport holds its initial_condition or 0
                    out = {"out": float(_num(block.params, "initial_condition", 0.0))}
                else:
                    v = inputs[bid]
                    if isinstance(v, (list, tuple)):
                        if len(v) != n_steps:
                            raise BlockError(f"Inport {bid!r}: expected {n_steps} values, got {len(v)}")
                        out = {"out": float(v[step])}
                    else:
                        out = {"out": float(v)}
            elif block.type == "Integrator":
                if "in" not in in_vals:
                    raise BlockError(f"Integrator {bid!r}: input not connected")
                u = float(_to_array(in_vals["in"]))
                if step > 0:
                    integrator_state[bid] += u * dt
                out = {"out": float(integrator_state[bid])}
            elif block.type == "UnitDelay":
                out = {"out": delay_state[bid]}
                # state update deferred to end of step (input may be evaluated later)
            elif block.type == "Outport":
                if "in" not in in_vals:
                    raise BlockError(f"Outport {bid!r}: input not connected")
                step_outs[bid] = to_jsonable(in_vals["in"])
                continue
            else:
                for port in spec["inputs"]:
                    if port not in in_vals:
                        raise BlockError(f"{block.type} block {bid!r}: input {port!r} not connected")
                out = spec["eval"](in_vals, block.params)
            for port, v in out.items():
                values[(bid, port)] = v
        for bid in list(delay_state):
            w = wiring.get(bid, {})
            if "in" in w:
                src_bid, src_port = w["in"]
                if (src_bid, src_port) in values:
                    delay_state[bid] = values[(src_bid, src_port)]
        for bid, v in step_outs.items():
            series[bid].append(v)

    return {"status": "ok", "time": times, "outputs": series}
