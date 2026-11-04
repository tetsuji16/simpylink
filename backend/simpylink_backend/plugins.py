"""Plugin loading: discover Python plugin modules that register custom blocks.

A plugin is a ``*.py`` file that defines either:
  - ``BLOCKS: dict[str, dict]`` mapping type name -> spec (same shape as
    ``BLOCK_REGISTRY`` entries), or
  - ``register(registry_dict)`` called with a dict to populate.

Discovery order: ``$SIMPYLINK_PLUGIN_DIR`` (colon/pathsep separated), then
``./plugins`` relative to cwd. A broken plugin is logged and skipped; the
server keeps running. Plugins are arbitrary Python code — only load trusted
plugins (documented in docs/plugins.md).
"""
from __future__ import annotations

import importlib.util
import os
import sys

from .blocks import register_block


def _load_module(path: str):
    name = f"simpylink_plugin_{os.path.splitext(os.path.basename(path))[0]}"
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise ValueError(f"cannot load plugin {path}")
    mod = importlib.util.module_from_spec(spec)
    sys.modules[name] = mod
    spec.loader.exec_module(mod)
    return mod


def load_plugins(dirs: list[str] | None = None) -> list[str]:
    """Load all plugins from dirs. Returns list of registered type names."""
    if dirs is None:
        env = os.environ.get("SIMPYLINK_PLUGIN_DIR", "")
        dirs = [d for d in env.split(os.pathsep) if d] + ["plugins"]
    registered: list[str] = []
    for d in dirs:
        if not os.path.isdir(d):
            continue
        for fname in sorted(os.listdir(d)):
            if not fname.endswith(".py") or fname.startswith("_"):
                continue
            path = os.path.join(d, fname)
            try:
                mod = _load_module(path)
            except Exception as e:  # broken plugin must not crash the server
                print(f"[simpylink] plugin {path} failed to load, skipped: {e}")
                continue
            try:
                blocks: dict = {}
                if hasattr(mod, "register"):
                    mod.register(blocks)
                elif hasattr(mod, "BLOCKS"):
                    blocks = dict(mod.BLOCKS)
                else:
                    print(f"[simpylink] plugin {path} defines neither BLOCKS nor register(), skipped")
                    continue
                for type_name, spec in blocks.items():
                    register_block(type_name, spec)
                    registered.append(type_name)
            except Exception as e:
                print(f"[simpylink] plugin {path} registration failed, skipped: {e}")
                continue
    return registered
