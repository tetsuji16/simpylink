# Third-party licenses

## Project license

simpylink itself is released under the MIT license (see `LICENSE`).

## Allowed licenses for dependencies

The list below is enforced in CI by `cargo-deny` using `deny.toml`. Do not add a
license here without agreeing on it in an issue first.

- MIT
- Apache-2.0
- Apache-2.0 WITH LLVM-exception
- BSD-2-Clause
- BSD-3-Clause
- ISC
- Zlib
- Unicode-3.0
- Unicode-DFS-2016
- MPL-2.0
- BSL-1.0
- CC0-1.0
- OFL-1.1
- Ubuntu-font-1.0

## Python dependencies

The generated Python code imports NumPy (BSD-3-Clause) and SciPy (BSD-3-Clause)
at run time. They are **not** bundled with simpylink; the user installs them with
`python -m pip install -r python/requirements.txt`.

## Checking licenses locally

```bash
cargo install cargo-deny --locked
cargo deny check licenses sources
```

Expected result: exit code 0 and `licenses ok` / `sources ok`.

## Generating the bundled third-party license list

This is done when a release package is built (see issue #100), not on every commit:

```bash
cargo install cargo-about --locked
cargo about generate about.hbs > THIRD_PARTY_LICENSES.html
```

`about.hbs` is the template from the cargo-about documentation. The generated
`THIRD_PARTY_LICENSES.html` is attached to the release artifacts.
