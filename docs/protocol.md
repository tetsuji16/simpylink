# Protocol — GUI <-> Backend

Base URL: `http://127.0.0.1:8765` (override: env `SIMPYLINK_BACKEND_URL`).

## Endpoints

### GET /health
`200 {"status": "ok", "version": "0.1.0"}`

### GET /blocks
List of block definitions for the GUI palette:
```json
{"blocks": [{"type": "Gain", "label": "Gain", "description": "...",
  "inputs": ["in"], "outputs": ["out"],
  "params": [{"name": "gain", "type": "number", "default": 1.0, "label": "Gain"}]}]}
```

### POST /simulate
Request: a model (see below) plus optional `inputs` for `Inport` blocks.
```json
{
  "model": {
    "name": "sum_example",
    "blocks": [
      {"id": "c1", "type": "Constant", "position": {"x": 40, "y": 40}, "params": {"value": 3.0}},
      {"id": "c2", "type": "Constant", "position": {"x": 40, "y": 140}, "params": {"value": 4.0}},
      {"id": "s",  "type": "Sum", "position": {"x": 240, "y": 90}, "params": {"inputs": "++"}},
      {"id": "o",  "type": "Outport", "position": {"x": 420, "y": 90}, "params": {"port": 1}}
    ],
    "connections": [
      {"from": {"block": "c1", "port": "out"}, "to": {"block": "s", "port": "in1"}},
      {"from": {"block": "c2", "port": "out"}, "to": {"block": "s", "port": "in2"}},
      {"from": {"block": "s", "port": "out"}, "to": {"block": "o", "port": "in"}}
    ]
  },
  "inputs": {}
}
```
Response:
```json
{"outputs": {"o": 7.0}, "trace": [{"block": "s", "output": 7.0}], "status": "ok"}
```
Errors: `400 {"error": "...", "detail": "..."}` for invalid model, unknown block, missing input, algebraic loop, divide-by-zero.

## Model JSON rules

- `blocks[].id`: unique non-empty string. `position` is GUI-only hint, ignored by backend.
- `connections[].from/to`: `{block, port}`. Multi-input blocks use `in1`, `in2`, ... (Sum/Product/Divide in skeleton).
- Every `Outport` input must be driven; every block input must be connected or have a default.
- Full machine-readable contract: `schemas/model.schema.json`.

## Versioning

`version` field on model and `/health` response. Bump minor on additive changes, major on breaking. GUI must tolerate unknown block types by showing an error, not crashing.
