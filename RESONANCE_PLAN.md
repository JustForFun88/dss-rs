# Plan: near-singular `Y_bus` / harmonic-resonance accuracy (post-1:1)

> **Status: DEFERRED — post-1:1.** This is *not* current porting work. During the
> 1:1 port near-resonance behaviour is a **documented conditioning exception**, not a
> fix (see §3). The improvements in §4 are a deliberate, post-acceptance divergence
> from OpenDSS, in the spirit of `PORTING_PLAN.md` §6 (final acceptance) and the
> post-port cleanup pass. It is **not** a `TODO(compat)` (we are not reproducing an
> upstream bug — we are hitting a numerical-conditioning limit).

## 1. Context

Harmonic power-flow studies excite **parallel resonances** between system inductance and
shunt capacitance. At/near a resonance the bus admittance matrix `Y_bus(h)` becomes
(nearly) **singular** (its determinant → 0; the driving-point impedance `Z = Y⁻¹` has a
pole), and the harmonic solve `Y_bus(h)·V = I` is ill-conditioned. This was observed in
the literature using OpenDSS itself: N.-C. Yang & Y.-W. Hsu, *"OpenDSS-based Harmonic
Power Flow Analysis for Power Systems with Passive Power Filters"* (IEEE Access, 2023)
reports, at resonance, OpenDSS driving-point `Z₁₁ = 70.05` vs. an analytical `5348.72`
(their Table 2), and states (paper §IV, near Eq. (28)): *"if `Y_busᵸ` is a singular or
quasisingular matrix, the inverse matrix cannot be calculated. This results in
significant errors in the voltage solutions."*

Cross-references: the harmonic-power investigation in
`investigations/oracle-powers-currents-harmonic/` (git-ignored) reproduces a *well-
conditioned* resonance-style case (harmonic Load + shunt Capacitor) where dss-rs matches
the oracle to all printed digits — the conditioning problem only appears *at* the pole.

## 2. The problem is three distinct effects (do not conflate them)

1. **Frequency-scan discretization.** A resonance peak is razor-thin. A scan on a finite
   grid (the paper used ≈0.6 Hz steps) simply *misses the vertex* and under-reports it
   (70.05 instead of 5348.72). This is a *method* artifact, not a solver fault.
2. **Ill-conditioning of the near-singular solve.** With `det(Y_bus) → 0`, solving
   `Y·V = I` amplifies rounding error. This is numerical conditioning; two correct LU
   implementations (OpenDSS's KLUSolve vs. our **faer**) will generally give *different*
   inaccurate answers near the pole.
3. **The exact lossless pole is genuinely `∞`.** With zero damping the impedance is
   truly infinite — no solver returns a meaningful finite number because none exists.
   Real systems have resistance, so the practical peak is finite and recoverable.

## 3. During the 1:1 port — handling, not fixing

Improving accuracy here **during** the port would break the oracle-match gate (we would
diverge from dss-python's 1e-6 reference). Moreover, near singularity faer and KLUSolve
diverge *anyway* (different pivoting/rounding) — a natural, expected divergence, not a
port bug. Therefore, while porting:

- Record near-singular / resonance cases in `tests/TOLERANCE_NOTES.md` as a
  **conditioning exception** (do not try to match the oracle's numerical noise at the
  pole). Per `PORTING_PLAN.md` §4 the rule is "parse numbers, never compare formatted
  strings; document exceptions in TOLERANCE_NOTES" — this is one such exception.
- For the live corpus gate (`corpus_live.rs`) and **WP7.6 step 3** (harmonics corpus
  migration): keep any case that sits *on* a resonance out of `solvable_now` until §4
  lands, or gate it with a relaxed/escape tolerance. Well-conditioned harmonic cases
  (the common case) stay at 1e-6 and must continue to pass.
- A `// near-singular: conditioning-sensitive, see RESONANCE_PLAN.md` note at the solve
  site, *not* a `TODO(compat)`.

## 4. Post-1:1 improvements (feasible — and we are better-positioned than OpenDSS)

dss-rs runs on **faer** (modern sparse + dense LA), and `dss-sparse` already exposes the
KLUSolveX-style extensions `rcond()` / `singular_col()` (`PORTING_PLAN.md` §2.4). So:

- **WP-R1 — iterative refinement on the sparse solve (low effort, high value).**
  Add residual correction to `SparseSet::solve`: after `x = solve(b)`, iterate
  `r = b − A·x; dx = solve(r); x += dx` for a few steps (gate on `rcond` / residual
  norm so well-conditioned solves pay nothing). Standard technique, ~50–100 lines, no new
  dependency. On ill-conditioned (near-resonant) systems this sharply improves accuracy →
  dss-rs becomes *more accurate than OpenDSS at the same frequency*. (Verified against the
  OpenDSS solver: it does **no** iterative refinement on the linear solve — `KLUSystem::Solve`
  issues a single `klu_z_solve`, and `klu_solve.c` itself states "no iterative refinement is"
  performed. OpenDSS's *outer* loop is a **nonlinear** power-flow fixed point on the load
  injections, not linear residual correction; and harmonics is a single direct solve — paper
  Eq. (28), "determined directly without an iterative process" — so nothing corrects the
  near-singular conditioning error.)
  **Acceptance:** on a damped resonance test, refined `Z` converges to the analytical
  value as damping → small; residual `‖b − A·x‖` falls below a set bound.

- **WP-R2 — resonance-aware analysis (more effort, a new *analysis* feature, not a
  core-solve change).** Locate resonances analytically instead of blind scanning:
  eigen-decompose `Y_bus(h) = L·Λ·Tᵀ` and find the orders where an eigenvalue → 0 (the
  method the paper itself uses, its Eqs. (2)–(4)); evaluate the driving-point impedance
  at the true pole (paper Eqs. (56)–(57)) and/or refine the scan grid around it. faer
  provides the dense eigendecomposition. This yields the true resonance vertex that a
  fixed-step scan misses (effect §2.1).
  **Acceptance:** reproduce the paper's Case Study 1 resonance orders (≈30.73 / 51.62 pu)
  and driving-point `Z₁₁ / Z₃₃` to the analytical values (their Tables 2–3), not the
  scan-limited OpenDSS values.

- **WP-R3 — the lossless pole (§2.3).** Nothing to "fix": detect via `rcond`/`singular_col`
  and report an explicit "resonance / singular at order h" diagnostic rather than a
  meaningless finite number.

## 5. Sequencing & contract

- Land **after** 1:1 acceptance (`PORTING_PLAN.md` §6). Each WP regenerates any goldens
  it intentionally changes, one at a time, and records the divergence-from-OpenDSS as a
  deliberate improvement (mirrors the §4.1 `TODO(compat)` cleanup discipline, though this
  is not itself a `TODO(compat)`).
- WP-R1 is solver-internal and broadly beneficial (any ill-conditioned system, not only
  harmonics); WP-R2 is opt-in tooling (a resonance/frequency-scan command). WP-R3 is a
  diagnostic. They are independent and can land separately.

## 6. References

- N.-C. Yang & Y.-W. Hsu, *"OpenDSS-based Harmonic Power Flow Analysis for Power Systems
  with Passive Power Filters"*, IEEE Access, 2023 — the singular-`Y_bus` observation
  (Table 2; §IV near Eq. (28)), eigenvalue resonance method (Eqs. (2)–(4)), driving-point
  impedance (Eqs. (56)–(57)).
- `PORTING_PLAN.md` §2.4 (`dss-sparse` / KLUSolveX `rcond`/`singular_col`), §4
  (tolerance policy / `TOLERANCE_NOTES`), §6 (final acceptance), §4.1 (`TODO(compat)`
  discipline — and why this is *not* one).
- `investigations/oracle-powers-currents-harmonic/` (git-ignored) — the well-conditioned
  resonance-style parity check (dss-rs ↔ oracle).
