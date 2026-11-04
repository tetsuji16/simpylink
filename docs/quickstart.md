# Quickstart

## Prerequisites
- Python 3.10+ and Rust stable (`rustup default stable`).
- No MATLAB/Simulink needed (and not used — see `docs/slx-import-spike.md`).

## 1. Backend
```bash
pip install -e backend          # pulls numpy
python backend/run.py           # http://127.0.0.1:8765
```

## 2. Smoke test (another terminal)
```bash
curl -X POST http://127.0.0.1:8765/simulate -H 'Content-Type: application/json' -d @examples/sum_example.json
# expect: {"status":"ok","outputs":{"o":7.0}, ...}
```

Time-stepped:
```bash
curl -X POST http://127.0.0.1:8765/simulate_time -H 'Content-Type: application/json' \
  -d '{"model": {...}, "t_start": 0, "t_stop": 1.0, "dt": 0.1}'
```

## 3. GUI
```bash
cargo run -p simpylink-gui
```
The GUI opens with `sum_example` (3+4) loaded. Click **Run** — the bottom panel shows `o: 7.0`.

### Try it
1. In the Library, search "Gain", click to add, drag it on the canvas.
2. Click the Constant's output port (right dot), then Gain's input port (left dot) to wire.
3. Double-click Gain to edit its parameters, Run again.
4. File > Save As to persist the model as JSON.

Windows PowerShell: the same commands work; replace `curl -d @file` with `curl.exe -d "@examples\sum_example.json"` if the alias interferes.

## Tests
```bash
python -m unittest discover -s backend/tests -v
cargo check --workspace
cargo clippy --workspace -- -D warnings
```
