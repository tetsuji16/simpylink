# Block parity matrix — Simulink R2026b Math Operations

Reference: https://jp.mathworks.com/help/simulink/math-operations.html (R2026b)

| Simulink block | simpylink (skeleton) | Notes / remaining |
|---|---|---|
| Constant (Sources) | ✅ `Constant` | param `value` |
| Inport / Outport (Sources/Sinks) | ✅ | `inputs` / `outputs` |
| Gain | ✅ `Gain` | param `gain` |
| Sum | ✅ `Sum` | param `inputs` e.g. `++`, `+--` |
| Product | ✅ `Product` | param `inputs` e.g. `**`, `*/` |
| Divide | ✅ `Divide` | `numerator / denominator` |
| Unary Minus | ✅ `UnaryMinus` | |
| Abs | ✅ `Abs` | |
| Bias | ✅ `Bias` | param `bias` |
| MinMax | ❌ | issue #3 |
| Trigonometric Function | ❌ | issue #3 |
| Math Function | ❌ | issue #3 |
| Rounding Function | ❌ | issue #3 |
| Sign | ❌ | issue #3 |
| Sqrt | ❌ | issue #3 |
| Polynomial | ❌ | issue #3 |
| Dot Product | ❌ | issue #3 |
| Algebraic Constraint | ❌ | needs solver, phase 2+ |
| Complex <-> Real-Imag / Magnitude-Angle | ❌ | issue #3 (needs complex signals) |
| Matrix/Vector Concatenate, Reshape, Permute, Squeeze | ❌ | needs vector/matrix signals, issue #2 |
| Sine Wave Function, Slider Gain | ❌ | Sources library, phase 2 |
| Product of Elements | ❌ | issue #3 |
| MinMax Running Resettable, Weighted Sample Time Math | ❌ | needs time/state, phase 2 |
| Assignment, Find Nonzero Elements | ❌ | phase 2 |

✅ = implemented + tested in skeleton. Everything else is tracked in GitHub issues — do not claim parity beyond this table.
