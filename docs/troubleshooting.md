# Troubleshooting

## GUI shows empty palette / "backend unreachable"
Start the backend first, then Refresh library:
```bash
pip install -e backend && python backend/run.py
```
If the GUI reports a different URL, set `SIMPYLINK_BACKEND_URL=http://127.0.0.1:8765`.

## Port 8765 already in use
Another backend instance is running (or a previous one didn't exit). Kill it (`Ctrl+C` in its terminal, or find the process listening on 8765), or start with a different port by editing `backend/simpylink_backend/server.py` HOST/PORT and setting `SIMPYLINK_BACKEND_URL` accordingly.

## `400 {"error": "BlockError", ...}`
| Message | Meaning | Fix |
|---|---|---|
| `division by zero` | denominator/reciprocal input contains 0 | change the upstream Constant or add a guard block |
| `shape mismatch` | vector shapes can't broadcast | make Constant arrays the same length (or scalar) |
| `input not connected` | a block input has no wire | wire every declared input (Sum/Product need in1..inN per spec) |
| `driven more than once` | two wires into one input port | delete one wire (click it, then Delete) |
| `algebraic loop` | feedback with no delay | route through `UnitDelay` and use time-stepped mode |
| `requires time-stepped simulation` | one-shot run on Clock/Sine/Integrator/UnitDelay | switch GUI mode to Time-stepped (`/simulate_time`) |
| `unknown block type` | typo or plugin not loaded | check `GET /blocks`; plugins go in `./plugins` or `$SIMPYLINK_PLUGIN_DIR` |

## Firewall blocks localhost
Some corporate firewalls block loopback HTTP. Allow Python on localhost, or run GUI and backend on the same machine (the default) — remote backends are not the intended deployment.

## Build failures
- `reqwest` needs OpenSSL/SChannel per platform — on Linux install `libssl-dev`; on Windows the default Schannel backend works.
- `cargo check` failing on clippy `-D warnings`: run `cargo clippy --workspace` to see the warnings.
