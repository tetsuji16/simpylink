# Plugin example

`square_plugin.py` adds a `Square` block (x^2).

## Use it

```bash
cp examples/plugin_example/square_plugin.py plugins/
python backend/run.py
curl http://127.0.0.1:8765/blocks   # Square now appears (category "Custom")
```

Then build Constant -> Square -> Outport in the GUI. See `docs/plugins.md`.
