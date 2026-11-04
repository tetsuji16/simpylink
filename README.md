# simpylink

OSS alternative to Simulink (MathWorks). Rust/egui GUI + Python backend.
Target: latest Simulink behavior (R2026b Math Operations library) for basic arithmetic blocks first; full parity is a roadmap, not a claim.

![status](https://img.shields.io/badge/status-skeleton-yellow) ![license](https://img.shields.io/badge/license-MIT-blue)

## Quick start

```bash
# backend (stdlib only, no pip install needed)
python backend/run.py          # http://127.0.0.1:8765  (GET /health, POST /simulate, GET /blocks)

# in another terminal, smoke test
curl -X POST http://127.0.0.1:8765/simulate -H 'Content-Type: application/json' \
  -d @backend/examples/sum_example.json

# GUI
cargo run -p simpylink-gui
```

Tests: `python -m pytest backend/tests -v` or `python -m unittest discover -s backend/tests -v`

## Layout

```
Cargo.toml                  workspace (crates/simpylink-gui)
backend/                    Python stdlib HTTP sim backend (no hard deps by design)
  simpylink_backend/        model.py / blocks.py / simulator.py / server.py
  tests/  examples/
crates/simpylink-gui/       egui/eframe GUI: palette, canvas, inspector, API client
schemas/model.schema.json   model JSON schema (contract GUI <-> backend)
docs/                       architecture.md / protocol.md / blocks.md / roadmap.md
```

## Supported blocks (skeleton MVP)

Sources/sinks: `Constant`, `Inport`, `Outport`
Arithmetic (scalar, R2026b Math Operations subset): `Gain`, `Sum`, `Product`, `Divide`, `UnaryMinus`, `Abs`, `Bias`
See `docs/blocks.md` for the full R2026b parity matrix and what is intentionally NOT done yet.

## Design principles

1. Python backend is the source of truth for semantics; GUI never computes block math.
2. Transport is plain HTTP+JSON on localhost (see `docs/protocol.md`). No gRPC/protobuf in skeleton.
3. Backend uses stdlib only so CI and contributors need no pip install; numpy comes later (issue #2).
4. All block semantics get golden tests with hand-computed expected values.

## Docs

- `docs/architecture.md` — components, data flow, why this split
- `docs/protocol.md` — HTTP API + model JSON format
- `docs/blocks.md` — block-by-block parity matrix vs Simulink R2026b
- `docs/roadmap.md` — phases; remaining work is tracked as GitHub issues

## License

MIT (see LICENSE).
