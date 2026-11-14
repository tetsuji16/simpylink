# Public SLX corpus survey

This corpus is a reproducible reference set for measuring SLX import, compilation, and export behavior. The repository stores only `manifest.json`, which records upstream repository, commit, path, and SHA-256. SLX files are downloaded into the ignored `cache/` directory and are never committed.

Each upstream file remains subject to the license and terms of its source repository. Check those terms before using a downloaded file beyond local testing. The survey does not imply that the files are licensed for redistribution.

```powershell
python tests/slx_corpus/fetch.py
cargo run -p sim_app --example slx_survey --release -- tests/slx_corpus/cache > report.json
```

The manifest reflects the investigation performed on 2026-11-09. The five `mathworks/data-explorer-*` entries labeled R2027a are test fixtures and are not evidence of the R2027a file format.
