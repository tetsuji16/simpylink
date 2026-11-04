# Architecture

## Components

| Component | Language | Role |
|---|---|---|
| `crates/simpylink-gui` | Rust + egui/eframe | Block canvas, palette, inspector, run button, results view |
| `backend/simpylink_backend` | Python 3.10+ stdlib | Model validation, topological evaluation, block math, HTTP server |
| `schemas/model.schema.json` | JSON Schema | Single contract for model JSON (both sides validate against it) |

## Data flow

```
[egui GUI] --POST /simulate (model JSON)--> [Python backend] --{outputs, trace}--> [GUI results panel]
     ^                                            |
     |                                            v
     +---------- GET /blocks (palette) ----- block registry (blocks.py)
```

GUI spawns/assumes backend at `http://127.0.0.1:8765` (configurable via `SIMPYLINK_BACKEND_URL` env or `--backend` flag).
GUI must never reimplement block math — if a value is shown, it came from the backend response.

## Why Python backend, Rust GUI

- Python: fastest path to correct numerics, numpy later, easy golden testing.
- Rust/egui: single binary, no web stack, immediate-mode UI fits canvas editing, compiles to native on Win/Linux/macOS.
- HTTP+JSON between them: debuggable with curl, replaceable later by stdio/gRPC without changing model schema.

## Simulator model (skeleton)

- Scalar float signals only. One-shot evaluation (no time, no solver) — this is `simulate(model, inputs)`.
- Blocks are pure functions of their input ports (see `blocks.py: BLOCK_REGISTRY`).
- Evaluation order = topological sort of the connection graph; cycles raise `AlgebraicLoopError` (real Simulink would try to solve them — out of scope, see roadmap).
- Time-based / stateful blocks (`Integrator`, `Unit Delay`, `State-Space`, ...) are phase 2 and need a stepped solver, not this one-shot evaluator.

## Non-goals (explicit)

- No `.slx`/`.mdl` binary compatibility. Interchange is our JSON model format.
- No MATLAB engine, no code generation, no FMI in skeleton.
- No vector/matrix signals in skeleton (scalar only); design ports so vector support is additive.
