"""Example plugin: Square block (x^2). Copy into ./plugins/ to use it.

See docs/plugins.md. This file is importable with load_plugins([this dir]).
"""
def _square(inputs, params):
    x = inputs["in"]
    return {"out": x * x}


BLOCKS = {
    "Square": {
        "label": "Square",
        "category": "Custom",
        "inputs": ["in"],
        "outputs": ["out"],
        "params": [],
        "eval": _square,
        "description": "Square the input (example plugin).",
    },
}
