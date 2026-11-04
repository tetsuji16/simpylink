# Releasing

Versions: workspace `Cargo.toml` `version`, `backend/simpylink_backend/__init__.py` `__version__`, and model JSON `version` — all `0.1.0`. CI enforces the first two match.

## Bump procedure
1. Update all three version strings (keep them equal).
2. Merge to `main`; confirm CI is green (backend 3.10/3.12/3.13, GUI linux+windows, version-check).
3. Tag: `git tag v0.1.0 && git push origin v0.1.0`.
4. Create a GitHub Release from the tag with notes (what's new, test results, known gaps from `docs/blocks.md`).
5. For v0.1.0 the source zip is sufficient; prebuilt binaries are optional follow-ups (not signed/notarized).

## Local verification before release
```bash
pip install -e backend
python -m unittest discover -s backend/tests -v
cargo check --workspace && cargo clippy --workspace -- -D warnings
python backend/run.py & curl http://127.0.0.1:8765/health
```
