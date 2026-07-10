# Plan: near-singular `Y_bus` / harmonic-resonance accuracy (post-1:1)

## Source-integrity gate — ritual step 0 (before the model-tier check)

The Pascal we port FROM — `.inputs/dss_capi` (186 `.pas` files), plus
`.inputs/electricdss-tst` for oracle/live work — is the **specification**. Before doing
anything, and re-checked continuously (not only at kickoff), confirm that folder exists
and is non-empty. If it has vanished — missing or empty — at **any** point in the work,
**STOP immediately**: make no edits, run no gate, and do **not** reconstruct, guess, or
"port" a source you cannot read. Tell the user the vendored source is gone and must be
re-vendored, then wait. Reply exactly:
**«Исходник порта (`.inputs/dss_capi`) отсутствует или пуст — работа остановлена. Восстанови
vendored-исходник (re-vendor) и повтори команду.»**
No spec → nothing to port; fabricating one from memory is a silent, unverifiable
divergence — far worse than stopping. This gate runs **ahead of the tier/refuse check**
(`PLAN_SEQUENCE.md` §Model-tier protocol).

> **Status: DEFERRED — post-1:1.** This is *not* current porting work. During the
> 1:1 port near-resonance behaviour is a **documented conditioning exception**, not a
> fix (see §3). The improvements in §4 are a deliberate, post-acceptance divergence
> from OpenDSS, in the spirit of `PORTING_PLAN.md` §6 (final acceptance) and the
> post-port cleanup pass. It is **not** a `TODO(compat)` (we are not reproducing an
> upstream bug — we are hitting a numerical-conditioning limit).
>
> **Sequencing (see `PLAN_SEQUENCE.md`):** this plan runs **after `DE_PASCALIZE_PLAN.md`
> Stage F** (the `oracle-parity` feature split) and **before `MULTITHREADING_PLAN.md`**.
> Stage F is what lets WP-R1 land cleanly: refinement is **on in the default build** and
> `#[cfg(feature = "oracle-parity")]`-**off in the parity build**, so every 1:1 oracle
> gate stays untouched while the product gets the accuracy win.

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
  **First clear the `CLAUDE.md` bar** ("never wave off a divergence as conditioning
  without empirical proof"): confirm the case is genuinely near-singular (tiny `rcond` /
  a real `singular_col`, gap collapses under a tighter solve rather than a cross-step
  state leak) *before* tagging it a conditioning exception.
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
  dss-rs becomes *more accurate than OpenDSS at the same frequency*. (Confirmed against the
  upstream KLU sources — not the vendored tree, which carries only the Pascal `KLUSolve.pas`
  externals into `libklusolvex`: KLU does **no** iterative refinement on the linear solve —
  `KLUSystem::Solve` issues a single `klu_z_solve`, and `klu_solve.c` itself states "no
  iterative refinement is" performed. OpenDSS's *outer* loop is a **nonlinear** power-flow
  fixed point on the load injections, not linear residual correction; and harmonics is a
  single direct solve — paper
  Eq. (28), "determined directly without an iterative process" — so nothing corrects the
  near-singular conditioning error.)
  **Acceptance:** on a damped resonance test, refined `Z` converges to the analytical
  value as damping is reduced *so long as `u·κ(Y) ≲ 1`* (below that, `Y` is too
  ill-conditioned w.r.t. machine precision to rescue — see Limit); residual `‖b − A·x‖`
  falls below a set bound. (The residual test checks backward stability; only the
  `Z`-convergence checks the forward accuracy that needs the compensated residual.)

  **Second acceptance case — the floating-delta zero-sequence class (measured
  2026-07-10, `Run_IEEE123Bus_GFMSnap.DSS` investigation).** This is a *power-flow*
  (not harmonics) beneficiary, proving WP-R1's "any ill-conditioned system" scope on a
  vendored deck. A bus fed by a delta xfmr winding with a delta DER (StoBus/PVBus)
  has no zero-sequence path to ground; its common mode is pinned only by the
  transformer anti-float adder (−j1.4468e-6 S = 2·Y_PPM) against a ~452 S diagonal —
  a κ≈3.1e8 subspace inside an otherwise well-conditioned solve. Measured on
  bit-identical `(Y, I)` (exact hex-bit transport — decimal JSON round-trip through
  serde_json *without* the `float_roundtrip` feature perturbs the last ulp and, ×3e8,
  poisons such measurements):
    - zero-seq residual per solve: KLU 7.3e-12 A / scipy 8.7e-12 A / **faer 8.7e-11 A**
      (~12× worse) → common-mode slack |r₀|/|y₀| ≈ 6e-5 V — the entire above-band
      live-gate failure of that deck (band 2.96e-5 V);
    - **one** refinement step: faer residual → 2.1e-11 A, faer-vs-KLU common-mode gap
      6.6e-5 → **9.4e-6 V** (3× under the band); a second step adds nothing — the
      remaining ~1e-5 V is the irreducible cross-solver spread (scipy-vs-KLU measures
      1.2e-5 V on the same bits).
  **Acceptance:** with refinement on, the one-shot faer-vs-KLU common-mode gap on the
  captured GFMSnap `(Y, I)` stays under the live band, and the deck's full live compare
  goes green (migrate it from `skipped_needs_investigation` to `solvable_now` in the
  default lane). Full measurement record: the deck's note in
  `tests/corpus/manifests/skipped_needs_investigation.json` + STATUS.md 2026-07-09/10.

  **Gate nuance (from the same investigation):** the *global* `rcond` of such a matrix
  looks healthy — the junk lives in one tiny subspace, invisible to a whole-matrix
  condition estimate. The refinement trigger must therefore be the **residual norm**
  (cheap: one mat-vec on the unscaled `A`), not `rcond` alone.

  **Divergence guard (measured 2026-07-10, AutoTrans family / `u·κ ≳ 1` in action):**
  on `Test/AutoTrans/Auto1bus.dss` (assembled-Y κ≈1e12: mvasc3=2e6 source + 1e-6 Ω
  switches + floating delta tertiary) ONE refinement step made the tertiary-subspace
  answer **worse by 6 orders** (1.3e-3 V → 1492 V vs KLU) while helping the LOW bus.
  The refinement loop must therefore verify the residual norm actually DECREASED after
  each step and roll the step back (keep the pre-step `x`) otherwise — "apply refinement
  when the residual is large" alone is not safe near a genuinely singular subspace.

  **Confirmed by the standard numerical-LA literature (this is a named, textbook
  technique, not a homegrown trick).** Both texts describe exactly the three-step
  process `r = b − A·x; solve A·d = r; x += d`:
  - **Golub & Van Loan, *Matrix Computations* (4th ed.), §3.5.3 "Iterative
    Improvement" (heading p. 139, body p. 140).** States *our* case verbatim: with
    partial pivoting the computed `x̂` already solves a nearby system, "*However, this
    may not be the case for certain pivot strategies used to preserve sparsity. In this
    situation, the fixed precision iterative improvement step can be worthwhile and
    cheap.*" (cites Arioli, Demmel & Duff — GVL's running text prints "1988", but its
    own bibliography and the paper itself give **1989**, SIAM J. Matrix Anal. Appl. 10).
    Also: "*The original A must be used in the high-precision computation of r.*" The
    forward-accuracy ("correct digits") result is the **mixed/extended-precision**
    regime — *not* the fixed-precision sentence above — namely GVL's Heuristic III: with
    the residual computed at precision `u²`, after `k` steps `x` has ≈`min{d, k(d−q)}`
    correct digits (`u=10⁻ᵈ`, `κ∞(A)≈10^q`). GVL gives the per-step cost as `O(n²)` vs
    the one-time `O(n³)` factorization; for our sparse factors that per-step cost is
    `O(nnz)` of the LU factors (our extension, not GVL's figure).
  - **Higham, *Accuracy and Stability of Numerical Algorithms* (2nd ed.), Ch. 12
    "Iterative Refinement."** "*The economics … are favourable for solvers based on a
    factorization of A, because the factorization used to compute x̂ can be reused*" (in
    the correction solve). The solver is treated as a black box — "*the solver need not
    be LU factorization or even a factorization method*" (only backward-stability,
    Eq. 12.1, is assumed), so it composes with our `SparseSet` as-is. Directly on motive:
    "*sparse GE is performed without pivoting, for speed, and iterative refinement is
    used to regain stability*" (Li & Demmel 1998; Dongarra et al. 2000). Fixed-precision
    refinement restores **backward** stability — normwise for an arbitrary solver
    (Jankowski & Woźniakowski 1977), componentwise for GEPP (Skeel 1980); **forward**
    accuracy near the pole needs an extended-precision residual.
  - Russian canon (cited in Venikov's own bibliography, see below): **Фаддеев &
    Фаддеева, «Вычислительные методы линейной алгебры» (1963)** — "уточнение по
    невязкам"; **Брамеллер/Аллан/Хэмэм, «Слабозаполненные матрицы» (1979)** — sparse
    factorization in the power-systems (Tinney/KLU) lineage.

  **Integration recipe — drop-in over the existing `SparseSet` (no `solve_one`
  change).** Iterative refinement is a strict *outer* wrapper around the current
  single direct solve (`crates/dss-sparse/src/lib.rs`, `solve_one`):
  1. The factorization (`self.factors`) is computed once and reused for every
     correction solve — exactly the "favourable economics" above. Cost per step is the
     triangular solves only, no refactor.
  2. **Row-equilibration is already handled.** The factored matrix is `diag(s)·A`, but
     `solve_one` pre-scales the RHS by `row_scale`. Passing the *true* residual
     `r = b − A·x` into `solve_one(r, dx)` therefore solves `diag(s)·A·dx = diag(s)·r`,
     i.e. `A·dx = r` — correct. **`solve_one` needs no change.**
  3. **Compute the residual on the unscaled assembled `self.matrix`** (not the factored
     `diag(s)·A`) — both texts insist on the original `A`.
  4. **Residual precision.** Fixed f64 already restores backward stability (the
     "worthwhile and cheap" sparsity case). For forward accuracy at a near-singular `Y`,
     accumulate `b − A·x` with a compensated (Kahan/two-product) complex dot — pure
     Rust, no new dependency, honours `#![forbid(unsafe_code)]`.
  5. **Gate on `rcond` / residual norm** (both already exposed): well-conditioned corpus
     solves skip refinement entirely → bit-identical to today. During the port: off
     entirely. Post-acceptance: the on/off switch lives in the Stage-F `compat` module —
     **on in the default build, off under `oracle-parity`** (the parity lane's oracle
     gates never see it; the default lane pins the improvement with its own
     analytical-value tests from the Acceptance bullet above).
  **Limit (= §2.3):** rescue needs `u·κ(Y) ≲ 1`, not merely nonsingularity. A truly
  singular `Y` (lossless pole) is hopeless, but so is a *nonsingular* `Y` once it is
  "*badly conditioned w.r.t. the machine precision*" (GVL: "*no improvement may result*"
  there). The target regime is the damped resonance that stays inside `u·κ(Y) ≲ 1`.

  **Not to be confused with the two *other* iterations in the power-systems texts**
  (Venikov, *«Математические задачи электроэнергетики»*): (a) §2-4 stationary linear
  solvers — простая итерация (Jacobi) / Зейдель (Gauss-Seidel), `x⁽ᵏ⁾ = B + C·x⁽ᵏ⁻¹⁾`,
  converge iff the spectral radius `ρ(C)<1` (diagonal dominance is the usual sufficient
  condition) — these solve `Ax=b` from scratch
  and *diverge* near a singular `Y`; (b) Appendix 8 / the nonlinear power-flow outer loop
  (simple iteration / Newton) — the OpenDSS outer loop already noted above. Iterative
  refinement is a third, distinct thing: it polishes one *direct* solve and is generic
  NLA, orthogonal to the PF outer loop. Venikov's books do **not** describe it; they cite
  the linear-algebra texts that do (Фаддеев & Фаддеева).

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

- Land **after** 1:1 acceptance (`PORTING_PLAN.md` §6) **and after `DE_PASCALIZE_PLAN.md`
  Stage F** (position 4 in `PLAN_SEQUENCE.md`, before `MULTITHREADING_PLAN.md`). With the
  `oracle-parity` split, WPs no longer regenerate parity goldens at all: the divergence
  lives in the **default lane only** (its self-goldens + the analytical-value acceptance
  tests), while the parity lane remains byte-stable.
- WP-R1 is solver-internal and broadly beneficial (any ill-conditioned system, not only
  harmonics); WP-R2 is opt-in tooling (a resonance/frequency-scan command). WP-R3 is a
  diagnostic. They are independent and can land separately.
- **Extra reference engines (2026-07-07):** the opt-in official-EPRI-binary oracle
  (`tools/opendss/`, Oddie bridge) makes r3723/r4088/r4133 available for cross-engine
  probes — useful in WP-R2/R3 validation to check whether newer upstream OpenDSS changed
  near-resonance behavior (`ab_compare.py` on the harmonics decks). Reference-only; the
  acceptance spec stays the analytical values (§4), not any engine.
- **Testing note:** WP-R1's `rcond` gate means well-conditioned solves are bit-identical
  with refinement on — so the parity↔default differential gate (`DE_PASCALIZE_PLAN` Part
  IV.2) is unaffected on the corpus except at documented ill-conditioned cases, where the
  default lane's analytical-value acceptance tests take over. Iteration counts on refined
  solves are naturally unpinned (default-lane policy).
- **Per-WP ritual — same as `PHASE8_PLAN`/`DE_PASCALIZE_PLAN`:** (0) tier check against
  the tiers below — below tier → do NOT execute, reply exactly «Этот шаг требует <exec
  tier>. Переключи сессию (/model + reasoning effort) и повтори команду.» and stop
  (`PLAN_SEQUENCE.md` §Model-tier protocol); (1) gate green in both lanes + differential
  job; (2) STATUS.md update + commit; (3) `/audit-code` + `/audit-tests` as fresh parallel
  agents **spawned with an explicit model/effort override matching the WP's audit tier**,
  findings settled empirically and recorded; (4) STATUS full review; (5) stop and report
  in Russian.
- **Tiers:** WP-R1 — exec `opus-high+`, audit `opus-high+`; **WP-R2 — exec+audit
  `opus-xhigh`**; WP-R3 — exec `opus-medium+`, audit `opus-high+`.
- **Executor guidance.** WP-R1 is **recipe-grade** — follow integration steps 1–5 in §4
  literally; definition of done: (a) the refinement wrapper in `dss-sparse` behind the
  Stage-F `compat` knob, (b) the damped-resonance convergence test + residual-bound test
  from the Acceptance bullet, (c) corpus untouched (differential gate unchanged on
  well-conditioned decks — the `rcond` gate is mandatory, refinement must never run
  unconditionally), (d) parity lane provably refinement-free (a test asserts the knob).
  WP-R3 is trivial (diagnostic on existing `rcond`/`singular_col`). **WP-R2 is the one
  design-heavy WP** (eigen-analysis command: new user surface, faer dense eig) — treat the
  paper's Case Study 1 numbers (§4 Acceptance) as the binding spec. Forbidden moves: never
  enable refinement in the parity lane; never weaken the `u·κ(Y) ≲ 1` limit note into
  "refinement fixes singular systems".

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
- **G. H. Golub & C. F. Van Loan, *Matrix Computations*, 4th ed., Johns Hopkins, 2013 —
  §3.5.3 "Iterative Improvement" (heading p. 139, body p. 140)** (local copy
  `.inputs/Solve_books/2013 Matrix Computations 4th.pdf`). The sparsity-preserving-pivot
  case + mixed-precision Heuristic III. Underlying analysis: Skeel (1980); Arioli, Demmel
  & Duff (1989 — GVL's running text misprints the year as "1988").
- **N. J. Higham, *Accuracy and Stability of Numerical Algorithms*, 2nd ed., SIAM, 2002 —
  Ch. 12 "Iterative Refinement"** (local copy
  `.inputs/Solve_books/Higham_2002_Accuracy and Stability of Numerical Algorithms.pdf`).
  Definitive treatment; factorization-reuse economics, solver-agnostic assumption
  (Eq. 12.1), fixed- vs extended-precision behaviour, sparse-GE-without-pivoting use case.
  Fixed-precision backward stability: Jankowski & Woźniakowski (1977, normwise, arbitrary
  solver); Skeel (1980, componentwise, GEPP).
- **Фаддеев Д. К., Фаддеева В. Н., «Вычислительные методы линейной алгебры», ГИФМЛ,
  1963** and **Брамеллер А., Аллан Р., Хэмэм Я., «Слабозаполненные матрицы: анализ
  электроэнергетических систем», Энергия, 1979** — Russian-canon sources for residual
  refinement and sparse factorization, both cited in V. A. Venikov, *«Электрические
  системы. Математические задачи электроэнергетики»* (bibliography "К главам 1 и 2").
  Note: Venikov's §2-4 ("итерационные методы") covers *stationary* solvers
  (Jacobi/Seidel), not residual refinement — see the WP-R1 "not to be confused" note.
