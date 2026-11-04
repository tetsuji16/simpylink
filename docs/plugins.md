# Plugins

Plugins add custom blocks in Python — no GUI or backend fork needed.

## Trust model
Plugins are arbitrary Python code loaded at server startup. Only load plugins you trust (same trust as `pip install`).

## Discovery
Server scans (in order):
1. Each dir in `$SIMPYLINK_PLUGIN_DIR` (path-separated).
2. `./plugins` (relative to where you start the backend).

Each `*.py` file (except `_*.py`) must define one of:

```python
# Option A: BLOCKS dict
BLOCKS = {
    "Square": {
        "label": "Square", "category": "Custom",
        "inputs": ["in"], "outputs": ["out"], "params": [],
        "eval": lambda inputs, params: {"out": inputs["in"] * inputs["in"]},
        "description": "Square the input.",
    },
}

# Option B: register(dict)
def register(registry):
    registry["Square"] = {...}
```

`eval(inputs, params)` receives input values (float or numpy arrays) and returns `{"out": value}`. Raise `ValueError` for bad input — it becomes a `400`.

Broken plugins (import error, bad spec) are logged and skipped; other plugins and blocks keep working.

## Verify
```bash
cp examples/plugin_example/square_plugin.py plugins/
python backend/run.py
curl http://127.0.0.1:8765/blocks    # "Square" appears with category "Custom"
```
Plugin blocks appear in the GUI palette automatically.
