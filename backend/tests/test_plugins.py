"""Plugin loading tests, including broken-plugin isolation."""
import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

from simpylink_backend.blocks import BLOCK_REGISTRY, get_block
from simpylink_backend.plugins import load_plugins

GOOD = '''
def _square(inputs, params):
    x = inputs["in"]
    return {"out": x * x}

BLOCKS = {
    "Square": {"label": "Square", "category": "Custom", "inputs": ["in"], "outputs": ["out"],
               "params": [], "eval": _square, "description": "x squared (example plugin)"},
}
'''

BROKEN = 'raise RuntimeError("boom on import")\n'


class TestPlugins(unittest.TestCase):
    def test_load_good_and_skip_broken(self):
        with tempfile.TemporaryDirectory() as d:
            open(os.path.join(d, "good.py"), "w").write(GOOD)
            open(os.path.join(d, "broken.py"), "w").write(BROKEN)
            registered = load_plugins([d])
        self.assertIn("Square", registered)
        spec = get_block("Square")
        self.assertEqual(spec["eval"]({"in": 4.0}, {})["out"], 16.0)
        # cleanup global registry pollution for other tests
        BLOCK_REGISTRY.pop("Square", None)


if __name__ == "__main__":
    unittest.main()
