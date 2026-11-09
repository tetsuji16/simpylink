# Public SLX corpus survey

## Baseline reference

The issue #143 investigation measured 226 files from 90 public repositories. The first values below were recorded against commit `6745130` before the follow-up fixes. A new run against this PR branch follows.

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

## Current survey run

The harness was run in WSL Ubuntu against all 226 downloaded files on 2026-11-10. Results are per readable model; the seven rejected files are excluded from validation and round-trip totals.

| Measure | Current run |
|---|---:|
| Read successfully | 219 / 226 |
| Rejected library roots | 5 |
| Rejected files without `[Content_Types].xml` | 2 |
| Files with no unsupported blocks | 24 |
| Compile successfully | 9 |
| Files with `E_UNKNOWN_BLOCK_TYPE` | 202 |
| Files with `E_PORT_OUT_OF_RANGE` | 151 |
| Files with `E_INVALID_PARAM` | 114 |
| System `<P ` occurrences before / after no-change export | 218,177 / 218,177 |
| Parts lost on no-change export | 0 |
| Readable files with every part unchanged | 219 / 219 |
| Readable files that re-import to the same Model | 219 / 219 |

The five rejected library roots are `CALM-Dataset/ride-models-matlab`, `mathworks/Simulink_Botnroll`, two libraries in `mathworks/simulink-cache-jenkins-workflow`, and `sopreigj/simulink-snapshot-edit`. The two files without `[Content_Types].xml` are the `caseRefs.slx` and `selfContained.slx` test fixtures from `mathworks/data-explorer-view`.

The report's exact embedded-version counts:

| Version | Files | Version | Files |
|---|---:|---|---:|
| R2027a label (test fixtures) | 5 | R2026b | 18 |
| R2026a | 30 | R2025b | 28 |
| R2025a | 27 | R2024b | 38 |
| R2024a | 11 | R2023b | 11 |
| R2023a | 7 | R2022b | 6 |
| R2022a | 12 | R2021b | 1 |
| R2021a | 5 | R2020b | 1 |
| R2020a | 1 | R2019b | 5 |
| R2019a | 2 | R2018b | 4 |
| R2018a | 5 | R2016b | 1 |
| R2014a | 3 | R2013b | 1 |
| R2013a | 2 | No embedded version (`?`) | 2 |

Top 30 unsupported types by number of readable files containing the type:

| Type | Files | Type | Files | Type | Files |
|---|---:|---|---:|---|---:|
| Demux | 103 | Terminator | 84 | S-Function | 78 |
| Mux | 68 | ToWorkspace | 64 | From | 53 |
| Goto | 52 | Display | 49 | ZeroOrderHold | 29 |
| FromWorkspace | 29 | Reference: Solver Configuration | 28 | Reference: Simulink-PS Converter | 27 |
| DataTypeConversion | 26 | Reference: PS-Simulink Converter | 25 | Math | 23 |
| PMIOPort | 21 | Trigonometry | 20 | Reference: Series RLC Branch | 17 |
| DiscretePulseGenerator | 17 | Logic | 16 | Reference: Electrical Reference | 15 |
| BusCreator | 14 | Reference: Voltage Measurement | 14 | BusSelector | 13 |
| RelationalOperator | 13 | Reference: powergui | 12 | Reference: Compare To Constant | 12 |
| Reference: Diode | 12 | Reference: Voltage Sensor | 12 | Reference: Current Sensor | 11 |

The detailed per-file output is generated as JSON by `slx_survey`; it is intentionally not committed because it contains local cache filenames. The reference names above abbreviate their full `SourceBlock` paths. No Simulink R2026b installation was used, so compatibility remains **未検証**.

Run `python tests/slx_corpus/fetch.py` and `cargo run -p sim_app --example slx_survey --release -- tests/slx_corpus/cache > report.json` to reproduce the per-file measurements.
