# Protocol — GUI <-> Backend

Two transports, same model JSON:
1. **Codegen (primary):** `python -m simpylink_backend.codegen model.json --output model.py --run` — generates and executes standalone Python (see `docs/architecture.md`).
2. **HTTP (optional):** base URL `http://127.0.0.1:8765` (override: env `SIMPYLINK_BACKEND_URL`). Endpoints below; current GUI uses this.

## Endpoints
- `GET /health` → `200 {"status":"ok","version":"0.1.0"}`
- `GET /blocks` → `{"blocks":[{"type","label","category","description","inputs","outputs","params":[{"name","type","default","label"}]}]}`
- `POST /simulate` — one-shot. Request `{"model":{...},"inputs":{...}}` → `{"status":"ok","outputs":{...},"trace":[...]}`. `inputs` values may be numbers or nested lists (vectors).
- `POST /simulate_time` — fixed-step. Request `{"model":{...},"inputs":{...},"t_start":0.0,"t_stop":1.0,"dt":0.1}` → `{"status":"ok","time":[...],"outputs":{outport_id:[...]}}`. `Inport` inputs may be a scalar (held) or a list of length N.

Example: `curl -X POST http://127.0.0.1:8765/simulate -H 'Content-Type: application/json' -d @examples/sum_example.json`

## Model JSON rules
- `blocks[].id` unique; `position` is a GUI-only hint, ignored by backend.
- `connections[].from/to` are `{block, port}`. Multi-input blocks use `in1`, `in2`, … (variable count per `inputs` param for Sum/Product; any `inN` for MinMax/Concat).
- Signal values: scalar = JSON number; vector/matrix = nested float arrays (numpy float64, column-major for Reshape). Broadcasting: scalar↔vector allowed; incompatible shapes → 400.
- Contract: `schemas/model.schema.json`. Examples round-trip through `Model.from_dict`→`to_dict` (contract-tested).

## Frozen decisions
- Errors: always `400 {"error": "<ExceptionClass>", "detail": "..."}` for client errors; `404 {"error":"NotFound",...}` for unknown paths. Malformed JSON → 400, never a traceback.
- Unknown block type, duplicate block id, duplicate wire into one input, unconnected required input, algebraic loop, divide-by-zero → all 400 with a naming message.
- Version rule: minor/additive model versions are accepted; a model `version` with a newer major than the backend is rejected (400). GUI tolerates unknown block types by showing the backend error, not crashing.
- Plugin blocks (docs/plugins.md) appear in `GET /blocks` like built-ins. Broken plugins are skipped at load.
