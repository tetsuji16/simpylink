# simpylink

OSS alternative to Simulink (MathWorks). Rust/egui GUI + Python backend.
Honest status: basic arithmetic parity with Simulink R2026b Math Operations (see `docs/blocks.md`).
Pixel-level GUI compatibility and full Simulink parity are NOT claimed — see `docs/roadmap.md` and `docs/slx-import-spike.md`.

![status](https://img.shields.io/badge/status-v0.1.0-green) ![license](https://img.shields.io/badge/license-MIT-blue)

## Quick start
```bash
pip install -e backend
python backend/run.py          # http://127.0.0.1:8765
curl -X POST http://127.0.0.1:8765/simulate -H 'Content-Type: application/json' -d @examples/sum_example.json
cargo run -p simpylink-gui
```
Full guide: `docs/quickstart.md`. Tests: `python -m unittest discover -s backend/tests -v` and `cargo check --workspace`.

## Layout
```
Cargo.toml                  workspace (crates/simpylink-gui)
backend/                    Python HTTP sim backend (numpy)
  simpylink_backend/        model.py / blocks.py / simulator.py / time_simulator.py / plugins.py / server.py
examples/                   runnable example models (+ plugin example)
crates/simpylink-gui/       egui/eframe GUI: palette, canvas, inspector, dialog, file I/O, results/plot
schemas/model.schema.json   model JSON contract
docs/                       architecture / protocol / blocks / quickstart / troubleshooting / plugins / releasing / slx-import-spike
plugins/                    drop custom block plugins here (git-ignored except .gitkeep)
```

## Supported blocks
28 blocks incl. Sources/Sinks plus the R2026b Math Operations arithmetic set (Gain, Sum, Product, Divide, MinMax, Trigonometric/Math/Rounding/Sqrt/Polynomial/…). Vector/matrix signals via numpy. Time-stepped mode with Clock/Sine/Integrator/UnitDelay. See `docs/blocks.md` for the full parity table — ❌ rows are not implemented.

## Docs
- `docs/quickstart.md` — zero to running model
- `docs/architecture.md` — components and data flow
- `docs/protocol.md` — HTTP API + model JSON (frozen decisions)
- `docs/blocks.md` — block-by-block parity vs R2026b
- `docs/plugins.md` — custom blocks
- `docs/troubleshooting.md` — common failures
- `docs/releasing.md` — how to cut a release
- `docs/slx-import-spike.md` — .slx import decision (no-go for production)

## License
MIT (see LICENSE).
