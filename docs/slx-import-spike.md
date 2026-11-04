# Spike: .slx/.mdl import feasibility (timeboxed)

Date: 2026-11-04. Question: should simpylink import MATLAB/Simulink `.slx`/`.mdl` files?

## Background
- `.slx` is a zip archive containing XML (`simulink/blockdiagram.xml` etc.).
- `.mdl` is a legacy text format.
- Both are proprietary MathWorks formats; no public complete specification. Compatibility would require a MATLAB/Simulink license for verification (not available in this environment) and rights to redistribute sample files.

## Prototype (throwaway, not shipped)
Wrote a minimal zip/XML reader mentally scoped to 8 block types (Constant, Gain, Sum, Product, Inport/Outport + connections, masks→params). Even this subset hits problems:
- Masks, annotations, solver settings, LookUnderMasks, subsystems, and S-functions have no simpylink equivalent.
- Parameter expressions (MATLAB syntax, workspace variables) can't be evaluated without MATLAB.
- Port/spec details differ per block and per Simulink version; R2026b files may not parse with older assumptions.

## Recommendation: NO-GO for production importer now
- Cost/benefit is poor without a license to test against; a lossy importer that silently drops masks would produce wrong simulations.
- Instead: keep JSON as the interchange format; users can recreate small arithmetic models by hand (palette + wiring is fast), and the `examples/` gallery covers common patterns.

## Follow-ups (only if a license becomes available)
1. Import only Constant/Gain/Sum/Product/Inport/Outport subset with an explicit loss report.
2. Reject (don't silently convert) anything using masks, subsystems, or S-functions.
3. Add golden tests with pinned sample files and their licenses.
