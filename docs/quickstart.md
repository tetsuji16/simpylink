# Quickstart

## Prerequisites
- Python 3.10+ and Rust stable (`rustup default stable`).
- No MATLAB/Simulink needed (and not used — see `docs/slx-import-spike.md`).
- simpylink is an OSS functional subset of Simulink R2026b Math Operations, not a pixel-level clone. Parity is defined by `docs/blocks.md` (✅/❌).

## 1. Codegen path (recommended, no server)
The GUI Run button defaults to codegen: model -> standalone Python (numpy) -> subprocess -> results. No port or server process needed.
```bash
pip install -e backend          # pulls numpy
# backend smoke (optional):
curl -X POST http://127.0.0.1:8765/simulate -H 'Content-Type: application/json' -d @examples/sum_example.json
# expect: {"status":"ok","outputs":{"o":7.0}, ...}
```
Or CLI codegen directly:
```bash
python -m simpylink_backend.codegen examples/sum_example.json --output model.py --run
# wrote model.py; stdout JSON has outputs.o = 7.0
```

## 2. HTTP path (opt-in, current GUI transport)
```bash
python backend/run.py           # http://127.0.0.1:8765
curl -X POST http://127.0.0.1:8765/simulate -H 'Content-Type: application/json' -d @examples/sum_example.json
curl -X POST http://127.0.0.1:8765/simulate_time -H 'Content-Type: application/json' -d '{"model": {...}, "t_start": 0, "t_stop": 1.0, "dt": 0.1}'
```

## 3. GUI (Rust/egui)
```bash
cargo run -p simpylink-gui
# (HTTP mode: ensure backend is running; codegen mode: set SIMPYLINK_PYTHON / SIMPYLINK_BACKEND_DIR if needed)
```
The GUI opens with `sum_example` (3+4) loaded. Click **Run** — bottom Results shows `o: 7.0`.
Top ribbon: SIMULATION (FILE New/Open/Save, SIMULATE Stop Time, PUBLISH Export .py / Library). Dark theme via top-bar checkbox.

### 60-second tour (mirrors Simulink workflow; labels are simpylink's, not Simulink's)
1. **Library (left):** search `Gain`, click to add, drag on canvas. Groups: Sources / Math Operations / Sinks / Custom.
2. **Wire:** click Constant's output port (right dot), then Gain's input port (left dot). Wires are straight lines and follow dragged blocks. Esc cancels; click a wire then Delete to remove one; duplicate input wiring is rejected.
3. **Parameters:** double-click a block (or Inspector > Open Parameters…). Number/string/boolean/array fields come from backend `GET /blocks`; array params are JSON text (e.g. `[1, 0, -1]` for Polynomial).
4. **Edit:** Ctrl+Z / Ctrl+Shift+Z undo/redo; arrow keys nudge selection (Shift = 10px); middle-drag or Space+drag pans, Ctrl+wheel zooms (percent shown top bar).
5. **Run:** pick One-shot or Time-stepped (`t_stop`, `dt`), click Run. Codegen subprocess is default (checkbox `codegen (no HTTP)`); uncheck to use HTTP to `http://127.0.0.1:8765`.
6. **Inspect results (Scope/Data-Inspector analogue):** bottom Results panel shows `outputs` table. Time-stepped runs plot the first output series (axes auto-scaled). For per-block traces use HTTP `trace` or open generated `model.py` (Export .py / temp dir path in status).
7. **Persist:** Save/Save As writes model JSON (`examples/*.json` are Open-able). Corrupt files show an error and keep the current model.

### ABS example (time mode)
Open `examples/time/abs_wheel_speed.json`, set mode Time-stepped, inputs ctrl=1, bp=2 (via HTTP `inputs` or codegen CLI `--mode time`). Brake torque -> net (UnitDelay feedback) -> 1/I Gain -> Integrator -> wheel speed. Verified: sim and codegen agree (last wheel speed ≈ -3.187 for t_stop=1, dt=0.1).

### Plugins (Python Code analogue)
Copy `examples/plugin_example/square_plugin.py` to `plugins/`, restart backend (or codegen) — `Square` appears in Library under Custom. Only trusted code; broken plugins are skipped. See `docs/plugins.md`.

### Themes / layout
Light/Dark checkbox (top bar). Canvas is pan/zoom; palette is searchable; results are docked below (no dockable multi-Scope yet — single docked plot only).

## Special Simulink notes (do not expect)
Publish Model / Protect Model / FMU export / Simulation App / Copilot are **not** implemented. Export is JSON + generated Python only. Algebraic Constraint / complex / Slider Gain / matrix-multiply Product are ❌ in `docs/blocks.md`.

## Tests
```bash
python -m unittest discover -s backend/tests -v
cargo check --workspace
cargo clippy --workspace -- -D warnings
```
