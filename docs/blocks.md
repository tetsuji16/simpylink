# Block parity matrix — Simulink R2026b Math Operations

Reference: https://jp.mathworks.com/help/simulink/math-operations.html (R2026b)
Gen verification: 36 backend tests (`python -m unittest discover -s backend/tests -v`), GUI `cargo check`.

| Simulink block | simpylink | Notes (differences vs Simulink) |
|---|---|---|
| Constant (Sources) | ✅ `Constant` | param `value` (number or JSON array for vectors) |
| Inport / Outport | ✅ | one-shot: `inputs` dict; time: scalar or length-N list |
| Gain | ✅ `Gain` | param `gain` |
| Sum | ✅ `Sum` | param `inputs` e.g. `++`, `+--`; N inputs (in1..inN); numpy broadcasting |
| Product | ✅ `Product` | param `inputs` e.g. `**`, `*/`; N inputs |
| Divide | ✅ `Divide` | `numerator / denominator` |
| Unary Minus | ✅ `UnaryMinus` | |
| Abs | ✅ `Abs` | |
| Bias | ✅ `Bias` | param `bias` |
| MinMax | ✅ `MinMax` | param `mode`: `min`/`max`; N inputs |
| Trigonometric Function | ✅ `TrigonometricFunction` | param `function`: sin/cos/tan/asin/acos/atan/sinh/cosh/tanh |
| Math Function | ✅ `MathFunction` | exp/log/log10/sqrt/square/reciprocal/pow/abs; `pow` takes `exponent` |
| Rounding Function | ✅ `RoundingFunction` | mode: floor/ceil/round/trunc |
| Sign | ✅ `Sign` | -1/0/1 |
| Sqrt | ✅ `Sqrt` | mode: sqrt/signed_sqrt/reciprocal_sqrt |
| Polynomial | ✅ `Polynomial` | `coefficients` highest-degree first (Simulink convention) |
| Product of Elements | ✅ `ProductOfElements` | modes: identity/reciprocal/square/cube/reciprocal_square/reciprocal_cube/exponentiation |
| Dot Product | ✅ `DotProduct` | vectors -> scalar; scalars -> product |
| Vector Concatenate | ✅ `VectorConcatenate` | along dim 0 |
| Matrix Concatenate | ✅ `MatrixConcatenate` | along rows (dim 0) |
| Reshape | ✅ `Reshape` | column-major (Fortran) order, like MATLAB |
| Permute Dimensions | ✅ `PermuteDimensions` | 1-based `dimensions`, e.g. [2,1] |
| Squeeze | ✅ `Squeeze` | removes size-1 dims |
| Sine Wave Function | ✅ `SineWaveFunction` | time-stepped only: A*sin(2*pi*f*t+phase)+offset |
| Clock | ✅ `Clock` | time-stepped only: outputs t |
| Integrator | ✅ `Integrator` | time-stepped only: x += u*dt |
| Unit Delay | ✅ `UnitDelay` | time-stepped only: previous input |
| Algebraic Constraint | ❌ | needs DAE solver |
| Complex <-> Real-Imag / Magnitude-Angle | ❌ | needs complex signals |
| Slider Gain | ❌ | GUI slider gain — not implemented |
| MinMax Running Resettable, Weighted Sample Time Math | ❌ | needs sample-time semantics |
| Assignment, Find Nonzero Elements | ❌ | out of scope |
| Matrix Multiply / Product (matrix) | ❌ | Product block covers scalar/vector elementwise only; true matrix multiply not implemented |

✅ = implemented + golden-tested. Vector signals are numpy float64; JSON transport is nested lists (`docs/protocol.md`).
❌ rows are NOT implemented — no parity claimed.
