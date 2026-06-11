# Phase 0 spike: faer complex sparse LU (GO decision)

Date: 2026-06-11 · faer 0.24.0 · Windows 11 x64, release build, default (serial-ish) parallelism.

Synthetic Y-bus on a 2D grid graph (≈5 nnz/row, complex f64, diagonally dominant).
A grid is **pessimistic** vs. real distribution feeders, which are mostly radial
(tree-like → far less LU fill-in).

| Nodes  | nnz     | symbolic | numeric factor | refactor (reuse sym) | solve (1 rhs) | ‖Ax−b‖∞ |
|--------|---------|----------|----------------|----------------------|---------------|---------|
| 2,500  | 12,300  | 1.6 ms   | 6.2 ms         | 5.7 ms               | 0.12 ms       | 3e-14   |
| 10,000 | 49,600  | 3.6 ms   | 22.6 ms        | 24.2 ms              | 1.1 ms        | 1e-13   |
| 24,964 | 124,188 | 8.2 ms   | 64.5 ms        | 63.2 ms              | 3.5 ms        | 4e-14   |
| 90,000 | 448,800 | 27.8 ms  | 319.9 ms       | 341.0 ms             | 14.4 ms       | 4e-14   |

## Verdict: **GO** — faer is adequate, no fallback needed.

- OpenDSS factors Y only when it changes (element edit, tap change) and solves
  once per fixed-point iteration. Even the IEEE 8500-node feeder (~25k nodes
  incl. phases) costs ≤65 ms per factorization and ~3.5 ms per solve — and the
  radial topology of real feeders will beat these grid-graph numbers.
- Residuals at machine precision (1e-13..1e-14) across all sizes.
- `LuError::SymbolicSingular { index }` maps 1:1 to KLUSolve `GetSingularCol`.
- `faer::c64` is literally `num_complex::Complex64` — zero conversion cost.
- Symbolic reuse across numeric refactorizations works (the KLU refactor
  pattern); note current refactor time ≈ first factor time, so symbolic reuse
  buys little in faer 0.24 — acceptable, revisit only if profiling flags it.

Reproduce: `cargo run --release -p dss-sparse --example spike`
