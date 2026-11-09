# Public SLX corpus survey

## Baseline reference

The issue #143 investigation measured 226 files from 90 public repositories. The following values were recorded against commit `6745130` before the follow-up fixes:

| Embedded release label | Files |
|---|---:|
| R2026b | 18 |
| R2026a | 30 |
| R2025b | 28 |
| R2025a | 27 |
| R2024b | 38 |
| R2024a | 11 |
| R2023b | 11 |
| Through R2023a | 58 |
| R2027a label (MathWorks test fixtures) | 5 |

| Measure | Baseline |
|---|---:|
| Read successfully | 222 / 226 |
| Files with no unsupported blocks | 16 |
| Compile successfully | 9 |
| `<P ` occurrences before export | 219,574 |
| `<P ` occurrences after export | 69,010 |
| Parts with `_rels` lost | 110 |
| `E_PORT_OUT_OF_RANGE` | 202 files |
| `E_DUPLICATE_SID` | 65 files |
| `E_INVALID_PARAM` | 91 files |
| R2026b files with numeric StopTime not imported | 9 / 9 |
| `<PortCounts>` / `<Mask>` / `<PortProperties>` before vs after export | 8,971 → 0 |

The four read failures consisted of two `<Library>` roots and two test files without `[Content_Types].xml`. The values are issue-provided baseline observations; the survey harness can produce a new report with the corpus and current code. No Simulink R2026b installation was used, so compatibility with that release is **未検証**.

Most frequently unsupported block types by file count in the baseline (the investigation comment supplied the first 15):

| BlockType | Files |
|---|---:|
| Reference | 142 |
| Demux | 104 |
| Terminator | 85 |
| S-Function | 80 |
| Mux | 69 |
| ToWorkspace | 64 |
| From | 53 |
| Goto | 52 |
| Display | 49 |
| FromWorkspace | 30 |
| ZeroOrderHold | 29 |
| DataTypeConversion | 26 |
| Math | 23 |
| PMIOPort | 21 |
| Trigonometry | 20 |

The available baseline notes did not include per-validation-code file counts or ranks 16–30. Those values remain **未検証** until the survey example can be run in an environment with the Windows GNU linker installed.

Run `python tests/slx_corpus/fetch.py` and `cargo run -p sim_app --example slx_survey --release -- tests/slx_corpus/cache > report.json` to reproduce the per-file measurements.
