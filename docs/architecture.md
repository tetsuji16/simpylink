# Architecture

## Components

| Component | Language | Role |
|---|---|---|
| `crates/simpylink-gui` | Rust + egui/eframe | Block canvas, palette, inspector, run button, results view |
| `backend/simpylink_backend` | Python 3.10+ stdlib | Model validation, topological evaluation, block math, HTTP server |
| `schemas/model.schema.json` | JSON Schema | Single contract for model JSON (both sides validate against it) |

## Execution model (codegen-first)

Primary path — blocking-to-Python-code generation, matching the intended UX
("connect blocks -> Python code appears -> Run analysis -> visualize"):

```
[model JSON] --codegen.py--> model.py (readable numpy source) --subprocess--> outputs.json --> [GUI results / plot]
```

- `codegen.py` emits standalone `model.py` (needs only numpy, no server).
- Generated code is committed/debuggable: one line per block, in topological order.
- `generate_and_run()` executes it in a subprocess; CLI: `python -m simpylink_backend.codegen model.json --output model.py --run`.

Optional transport (kept for the current GUI and remote use):
`GET /health`, `GET /blocks`, `POST /simulate`, `POST /simulate_time` over
`http://127.0.0.1:8765`. The GUI currently uses this path; migrating the GUI
to spawn codegen subprocesses (no listening port) is tracked as a follow-up
issue. Both paths share block formulas, so results must agree.

GUI must never reimplement block math — if a value is shown, it came from
executed backend/codegen output.

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
