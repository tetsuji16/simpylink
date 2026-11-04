# Roadmap

- Phase 0 (skeleton, this commit): scalar one-shot simulation, 7 arithmetic blocks + Constant/Inport/Outport, egui canvas with drag/connect/inspect, HTTP+JSON.
- Phase 1: remaining scalar Math Operations blocks (MinMax, Trig, Math Function, Rounding, Sign, Sqrt, Polynomial, Dot Product, Product of Elements), golden tests, undo/redo, save/load JSON models.
- Phase 2: vector/matrix signals (numpy), time-stepped simulation, Sources blocks (Sine Wave, Clock, Ramp), Scopes/plots, stateful blocks (Integrator, Unit Delay, Transfer Fcn).
- Phase 3: differential/algebraic solvers, algebraic loops, code generation, .slx import (best effort), plugin/custom blocks, debugger/Profiler equivalents.

See GitHub issues for the detailed, delegatable breakdown. When in doubt: scalar first, JSON contract first, tests before features.
