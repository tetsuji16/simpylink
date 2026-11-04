"""Code generation: model JSON -> standalone Python (numpy) source.

This is the primary execution path (see docs/architecture.md):
  GUI model --codegen--> model.py --subprocess--> outputs.json --> GUI plot
The HTTP server remains as an optional transport; semantics are shared
because both paths use the same topological order and block formulas below.

Generated code is plain, readable, and runnable without simpylink installed
(only numpy). Run: python model.py  (writes outputs.json next to it)
"""
from __future__ import annotations

import json

import numpy as np

from .blocks import BlockError, get_block
from .model import Model
from .simulator import _topo_order

_TRIG = {"sin": "np.sin", "cos": "np.cos", "tan": "np.tan", "asin": "np.arcsin",
         "acos": "np.arccos", "atan": "np.arctan", "sinh": "np.sinh", "cosh": "np.cosh", "tanh": "np.tanh"}


def _var(block_id: str, port: str) -> str:
    return f"v_{block_id}_{port}"


def _lit(value) -> str:
    return repr(value)


def _expr(block, in_vars: dict[str, str]) -> str:
    t, p = block.type, block.params
    v = lambda name="in": in_vars.get(name, f"missing_{name}")  # noqa: E731
    if t == "Constant":
        return _lit(p.get("value", 0.0))
    if t == "Gain":
        return f"(np.asarray({v()}) * {_lit(p.get('gain', 1.0))})"
    if t == "Bias":
        return f"(np.asarray({v()}) + {_lit(p.get('bias', 0.0))})"
    if t == "Abs":
        return f"np.abs(np.asarray({v()}))"
    if t == "UnaryMinus":
        return f"(-np.asarray({v()}))"
    if t == "Sign":
        return f"np.sign(np.asarray({v()}))"
    if t == "Sum":
        spec = str(p.get("inputs", "++"))
        terms = []
        for i, s in enumerate(spec, start=1):
            vv = in_vars[f"in{i}"]
            terms.append(vv if s == "+" else f"(-np.asarray({vv}))")
        return "(" + " + ".join(f"np.asarray({x})" for x in terms) + ")"
    if t == "Product":
        spec = str(p.get("inputs", "**"))
        expr = f"np.asarray({in_vars['in1']})"
        for i, op in enumerate(spec, start=1):
            if i == 1:
                continue
            vv = f"np.asarray({in_vars[f'in{i}']})"
            expr = f"({expr} {'*' if op == '*' else '/'} {vv})"
        return expr
    if t == "Divide":
        return f"(np.asarray({in_vars['numerator']}) / np.asarray({in_vars['denominator']}))"
    if t == "MinMax":
        keys = sorted(k for k in in_vars if k.startswith("in"))
        fn = "np.minimum" if str(p.get("mode", "min")) == "min" else "np.maximum"
        expr = f"np.asarray({in_vars[keys[0]]})"
        for k in keys[1:]:
            expr = f"{fn}({expr}, np.asarray({in_vars[k]}))"
        return expr
    if t == "TrigonometricFunction":
        fn = _TRIG[str(p.get("function", "sin")).lower()]
        return f"{fn}(np.asarray({v()}))"
    if t == "MathFunction":
        fn = str(p.get("function", "exp")).lower()
        if fn == "exp":
            return f"np.exp(np.asarray({v()}))"
        if fn == "log":
            return f"np.log(np.asarray({v()}))"
        if fn == "log10":
            return f"np.log10(np.asarray({v()}))"
        if fn == "sqrt":
            return f"np.sqrt(np.asarray({v()}))"
        if fn == "square":
            x = f"np.asarray({v()})"
            return f"({x} * {x})"
        if fn == "reciprocal":
            return f"(1.0 / np.asarray({v()}))"
        if fn == "pow":
            return f"np.power(np.asarray({v()}), {_lit(p.get('exponent', 2.0))})"
        if fn == "abs":
            return f"np.abs(np.asarray({v()}))"
        raise BlockError(f"codegen: MathFunction {fn!r} unsupported")
    if t == "RoundingFunction":
        mode = str(p.get("mode", "round")).lower()
        fn = {"floor": "np.floor", "ceil": "np.ceil", "round": "np.round", "trunc": "np.trunc"}[mode]
        return f"{fn}(np.asarray({v()}))"
    if t == "Sqrt":
        mode = str(p.get("mode", "sqrt")).lower()
        if mode == "sqrt":
            return f"np.sqrt(np.asarray({v()}))"
        if mode == "signed_sqrt":
            return f"(np.sign(np.asarray({v()})) * np.sqrt(np.abs(np.asarray({v()}))))"
        if mode == "reciprocal_sqrt":
            return f"(1.0 / np.sqrt(np.abs(np.asarray({v()}))))"
        raise BlockError(f"codegen: Sqrt mode {mode!r} unsupported")
    if t == "Polynomial":
        coeffs = p.get("coefficients") or [1.0]
        x = f"np.asarray({v()})"
        # Horner
        expr = f"np.asarray({_lit(float(coeffs[0]))}) * np.zeros_like({x})"
        for c in coeffs:
            expr = f"(({expr}) * {x} + {_lit(float(c))})"
        return expr
    if t == "ProductOfElements":
        mode = str(p.get("mode", "identity")).lower()
        x = f"np.asarray({v()})"
        if mode == "identity":
            return x
        if mode == "reciprocal":
            return f"(1.0 / {x})"
    # (remaining modes fall through to runtime helper)
        if mode == "square":
            return f"({x} * {x})"
        if mode == "cube":
            return f"({x} * {x} * {x})"
        if mode == "exponentiation":
            return f"np.power({x}, {_lit(p.get('exponent', 2.0))})"
        raise BlockError(f"codegen: ProductOfElements mode {mode!r} unsupported")
    if t == "DotProduct":
        return f"float(np.dot(np.asarray({in_vars['in1']}).ravel(), np.asarray({in_vars['in2']}).ravel()))"
    if t in ("VectorConcatenate", "MatrixConcatenate"):
        keys = sorted(k for k in in_vars if k.startswith("in"))
        return f"np.concatenate([np.asarray({in_vars[k]}) for k in {keys!r}], axis=0)"
    if t == "Reshape":
        return f"np.reshape(np.asarray({v()}), tuple({_lit(list(p.get('size', [1])))}), order='F')"
    if t == "PermuteDimensions":
        dims = [int(d) - 1 for d in (p.get("dimensions") or [1])]
        return f"np.transpose(np.asarray({v()}), axes={dims!r})"
    if t == "Squeeze":
        return f"np.squeeze(np.asarray({v()}))"
    raise BlockError(f"codegen: block type {t!r} requires the simulator (or time mode)")


def generate_code(model: Model, inputs: dict | None = None, mode: str = "oneshot",
                  t_start: float = 0.0, t_stop: float = 1.0, dt: float = 0.1) -> str:
    inputs = inputs or {}
    order = _topo_order(model)
    # wiring: dst block -> port -> (src block, src port)
    wiring: dict[str, dict[str, tuple[str, str]]] = {}
    for c in model.connections:
        wiring.setdefault(c.target.block, {})[c.target.port] = (c.source.block, c.source.port)

    lines: list[str] = []
    lines.append(f"# generated by simpylink codegen from model {model.name!r} (do not edit by hand)")
    lines.append("import json")
    lines.append("import numpy as np")
    lines.append("")
    lines.append("def _jsonable(v):")
    lines.append("    if isinstance(v, np.ndarray):")
    lines.append("        return float(v) if v.ndim == 0 else v.tolist()")
    lines.append("    if isinstance(v, (np.floating, np.integer)):")
    lines.append("        return float(v)")
    lines.append("    return v")
    lines.append("")
    lines.append("def main():")
    if mode == "time":
        n = int(round((t_stop - t_start) / dt)) + 1
        lines.append(f"    outputs = {{}}")
        lines.append(f"    times = [{t_start!r} + i * {dt!r} for i in range({n})]")
        lines.append(f"    out_series = {{}}")
    else:
        lines.append("    outputs = {}")

    if mode == "time":
        # In time mode everything is emitted inside the per-step loop below;
        # the header pass only records topology (avoids using time vars early).
        for bid in order:
            block = model.block_map()[bid]
            lines.append(f"    # {block.type} [{bid}] params={_lit(block.params)}")
    else:
        for bid in order:
            block = model.block_map()[bid]
            lines.append(f"    # {block.type} [{bid}] params={_lit(block.params)}")
            in_vars: dict[str, str] = {}
            for port, (src_bid, src_port) in wiring.get(bid, {}).items():
                in_vars[port] = _var(src_bid, src_port)
            if block.type == "Outport":
                lines.append(f"    outputs[{bid!r}] = _jsonable({in_vars.get('in', 'None')})")
                continue
            if block.type == "Inport":
                lines.append(f"    {_var(bid, 'out')} = np.asarray({_lit(inputs.get(bid, 0.0))})")
                continue
            try:
                expr = _expr(block, in_vars)
            except KeyError as e:
                raise BlockError(f"codegen: {block.type} [{bid}] missing input {e}") from e
            lines.append(f"    {_var(bid, 'out')} = {expr}")

    if mode == "time":
        # header pass above computed stateless sources only; now wrap time-dependent
        # blocks in a per-step loop (simplified: full per-step codegen)
        body: list[str] = []
        body.append("    state = {}")
        for bid in order:
            block = model.block_map()[bid]
            if block.type == "Integrator":
                body.append(f"    state[{bid!r}] = float({block.params.get('initial_condition', 0.0)!r})")
            if block.type == "UnitDelay":
                body.append(f"    state[{bid!r}] = {block.params.get('initial_condition', 0.0)!r}")
        body.append("    for _step, t in enumerate(times):")
        inner: list[str] = []
        inner.append("        vals = {}")
        # recompute everything per step using the same expressions,
        # with time sources replaced by their per-step formulas.
        for bid in order:
            block = model.block_map()[bid]
            in_vars = {}
            for port, (src_bid, src_port) in wiring.get(bid, {}).items():
                in_vars[port] = f"v_{src_bid}_{src_port}"
            if block.type == "Clock":
                inner.append(f"        v_{bid}_out = float(t)")
            elif block.type == "SineWaveFunction":
                p = block.params
                amp = p.get("amplitude", 1.0)
                freq = p.get("frequency", 1.0)
                phase = p.get("phase", 0.0)
                off = p.get("offset", 0.0)
                inner.append(f"        v_{bid}_out = float({amp!r} * np.sin(2 * np.pi * {freq!r} * t + {phase!r}) + {off!r})")
            elif block.type == "Integrator":
                inner.append(f"        if _step > 0:")
                inner.append(f"            state[{bid!r}] = state[{bid!r}] + float(np.asarray({in_vars.get('in', '0.0')})) * {dt!r}")
                inner.append(f"        v_{bid}_out = float(state[{bid!r}])")
            elif block.type == "UnitDelay":
                inner.append(f"        v_{bid}_out = state[{bid!r}]")
                inner.append(f"        state[{bid!r}] = {in_vars.get('in', '0.0')}")
            elif block.type == "Inport":
                inner.append(f"        v_{bid}_out = float(inputs.get({bid!r}, 0.0))")
            elif block.type == "Outport":
                inner.append(f"        out_series.setdefault({bid!r}, []).append(_jsonable({in_vars.get('in', 'None')}))")
            else:
                try:
                    expr = _expr(block, in_vars)
                except KeyError as e:
                    raise BlockError(f"codegen(time): {block.type} [{bid}] missing input {e}") from e
                inner.append(f"        v_{bid}_out = {expr}")
        lines.extend(body)
        lines.extend(inner)
        lines.append("    outputs = out_series")
        lines.append("    payload = {'status': 'ok', 'time': times, 'outputs': outputs}")
    else:
        lines.append("    payload = {'status': 'ok', 'outputs': outputs}")
    lines.append("    with open('outputs.json', 'w') as f:")
    lines.append("        json.dump(payload, f)")
    lines.append("")
    lines.append("if __name__ == '__main__':")
    lines.append("    main()")
    header_inputs = f"INPUTS = {_lit(inputs)}\n"
    return header_inputs + "\n".join(lines) + "\n"


def generate_and_run(model: Model, inputs: dict | None = None, mode: str = "oneshot",
                     t_start: float = 0.0, t_stop: float = 1.0, dt: float = 0.1,
                     workdir: str | None = None) -> dict:
    """Generate code and execute it in a subprocess, returning parsed outputs."""
    import os
    import subprocess
    import sys
    import tempfile

    code = generate_code(model, inputs=inputs, mode=mode, t_start=t_start, t_stop=t_stop, dt=dt)
    with tempfile.TemporaryDirectory(dir=workdir) as d:
        py = os.path.join(d, "model.py")
        with open(py, "w") as f:
            f.write(code)
        r = subprocess.run([sys.executable, py], cwd=d, capture_output=True, text=True, timeout=120)
        if r.returncode != 0:
            raise BlockError(f"generated model failed: {r.stderr[-2000:]}")
        with open(os.path.join(d, "outputs.json")) as f:
            return json.load(f)


if __name__ == "__main__":
    import argparse

    ap = argparse.ArgumentParser(description="simpylink codegen")
    ap.add_argument("model", help="model JSON file")
    ap.add_argument("--output", default="model.py", help="generated python file")
    ap.add_argument("--run", action="store_true", help="execute and print outputs")
    ap.add_argument("--mode", default="oneshot", choices=["oneshot", "time"])
    ap.add_argument("--t_stop", type=float, default=1.0)
    ap.add_argument("--dt", type=float, default=0.1)
    args = ap.parse_args()
    _model = Model.from_dict(json.load(open(args.model)))
    _code = generate_code(_model, mode=args.mode, t_stop=args.t_stop, dt=args.dt)
    open(args.output, "w").write(_code)
    if not args.run:
        print(f"wrote {args.output}")
    if args.run:
        import sys as _sys
        _res = generate_and_run(_model, mode=args.mode, t_stop=args.t_stop, dt=args.dt)
        print(json.dumps(_res))
