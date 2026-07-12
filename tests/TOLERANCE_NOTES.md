# Tolerance notes

Numeric tolerance policy for the golden / live gates (PORTING_PLAN.md §4).
Discrete state — taps/tap-numbers, switch & capacitor states, control-action and
island counts — is always compared **exactly** and never appears here. Text
outputs compare by numeric skeleton (structure exact, numbers with tolerance).

> **Do not loosen a tolerance to make a failing oracle comparison pass.** Every
> floor here is calibrated to *proven* f64 / f32 / faer-vs-KLU reality. A
> Rust↔oracle gap that exceeds its floor is a porting **bug** — find and fix the
> root cause, never widen the band to hide it. Floors change only with empirical
> proof (and tighten far more often than they loosen); they are never relaxed to
> mask a divergence. (CLAUDE.md, §"conditioning".)

## Tolerance classes (`harness::tol_for`)

The assembled-model gate (`golden_checkpoints.rs` + the live `corpus_live.rs`)
tiers tolerances by **conditioning**, not size:

- **micro** — tiny synthetic circuits.
- **feeder** — well-conditioned standard feeders (IEEE13/37, simpler `Test/`
  circuits).
- **large** — large or numerically-stiff networks (EPRI ckt5 / 8500-Node /
  A-Diakoptics torn zones / inverter cases / IEEE123 / 4Bus-YYD). Also the safe
  default for an unrecognized `kind`.
- **large_floating_delta** — `large` plus one documented exception: `v_abs`
  5e-4 V (see §floating-delta below). Currently one deck
  (`GFM_IEEE123/Run_IEEE123Bus_GFMSnap.DSS`).
- **large_near_ideal_source** — `large` plus two documented exceptions:
  `v_rel` 5e-6 and `i_abs` 0.1 A (see §near-ideal-source below). The 9-deck
  AutoTrans validation family + the 2 `PV_currentkvarLimit_*` decks (Thevenin
  source Z=1e-8 Ω ≈ 7e7 S: the Vsource current inherits dI = Y_src·dV from a
  ~3-ulp source-bus dV while the PVSystem itself matches); the Y channel keeps
  the tight `large` floors and is the regression sentinel.
- **large_floating_zeroseq** — `large` plus `v_abs` 3e-2 V (see
  §floating-zeroseq below). The weakly-pinned floating zero-sequence members:
  TestDDRegulator, DG_Prot_Fdr, LVTestCaseNorthAmerican Master/SecPar.
- **large_ultra_switch** — `large` plus `i_abs` 2e-3 A (see §ultra-switch
  below). A-Diakoptics torn circuits whose 1e-8 Ω stitching pseudo-switches
  (Y≈1e8 S) resolve sub-ulp voltage differences into the currents channel:
  ckt24 and EPRI_Ckt7-G Torn Master/Master_Interconnected.

| Quantity | micro | feeder | large |
|---|---|---|---|
| node voltages | 1e-9 rel, abs 1e-6 V | 1e-8 rel, abs 1e-6 V | 1e-7 rel, abs 1e-6 V |
| element currents / powers; injection | 1e-9 rel, abs 1e-6 | 1e-7 rel, abs 1e-5 | 1e-6 rel, abs 1e-4 |
| assembled Y / selected YPrim | 1e-9 rel, abs 1e-6 S | 1e-8 rel, abs 1e-6 S | 1e-8 rel, abs 1e-6 S |
| energymeter registers | 1e-4 rel, abs 1e-4 | 1e-4 rel, abs 1e-4 | 1e-4 rel, abs 1e-4 |
| Y `nnz` / sparsity pattern | exact | exact | exact |

The currents/powers floor cannot tighten below the `large` values on the stiff
cases (faer↔KLU floor: ckt5 conductor power ~1.3e-6 rel, a PVSystem `Vsource`
current ~1.2e-4 A, IEEE123's `S2` monitor channel ~1.1e-7 rel). The assembled Y
is deterministic (same YPrim stamps both engines, not a solver output), so it
holds 1e-8 in every class.

### floating-delta (`large_floating_delta`): the un-pinnable zero-sequence common mode

A bus fed by a **delta** transformer winding carrying a **delta** DER
(`GFM_IEEE123/Run_IEEE123Bus_GFMSnap.DSS`: StoBus + PVBus) has no
zero-sequence path to ground. Its common-mode voltage is pinned only by the
winding's anti-float adder (−j1.4468e-6 S = 2·`Y_PPM`) against a ~452 S
diagonal — a κ≈3.1e8 subspace inside an otherwise well-conditioned solve. The
common mode is therefore **solver-junk with a stable per-solver value**:
measured on bit-identical `(Y, I)` (hex-bit transport; a decimal JSON
round-trip perturbs the last ulp and, ×3e8, poisons the measurement), each
solver leaves a different zero-seq residual — KLU 7.3e-12 A, scipy 8.7e-12 A,
faer 8.7e-11 A — i.e. common-mode slacks of 5e-6…7e-5 V, and the engines land
9.03e-5 V apart at the gate's step 0. The proof it is a floor and not a bug
(2026-07-09/10, bitwise audit): engine solve is deterministic and bit-equal to
a fresh faer solve of its exported `(Y, I)`; the last solve's RHS bit-equals
the captured injection; the assembled Y is bit-identical across engines;
the element formulas match Pascal line-by-line; the diff is 100% common mode
(differential remainder ≤5.9e-8 V, L-L ≈2.3e-10 rel; every non-DER node
in-band at `large`).

The tier therefore keeps every `large` floor and widens **only `v_abs`, to
5e-4 V** (≈5.5× the measured worst; ~1.8e-6 rel at the 277 V DER buses). No
real coverage is lost: the DER elements' currents/powers are functions of the
L-L (differential) voltages — immune to the common mode — and stay gated at
the `large` floors, as do Y/YPrim/injection. Fix owner: `RESONANCE_PLAN.md`
WP-R1 (one iterative-refinement step measures 9.4e-6 V vs KLU — 3× under the
`large`-band); when it lands, retighten this tier back to `large`.

### floating-zeroseq (`large_floating_zeroseq`): weakly-pinned common modes

The same physical class as §floating-delta — a subsystem reachable only
through delta windings has no zero-sequence ground path, and its common-mode
voltage is pinned solely by the ppm anti-float adders — but with pinning 2–3
orders weaker than GFMSnap's (amplification 3.1e8 there), so the junk exceeds
the `large_floating_delta` band and gets its own `v_abs` **3e-2 V** (worst
measured 1.06e-2 ×2.8; at the smallest affected 346 V buses that is 8.7e-5
rel). Per-deck proof by decomposition (all 2026-07-10):

- **TestDDRegulator** — REGBUS2 sits between TWO delta windings
  (Transformer.Reg1/Reg2 winding 2): measured pinning −j2.14e-8 S vs ~303 S
  diagonal → amplification 1.4e10. The whole 5.2191e-3 V gap is common mode on
  all three REGBUS2 nodes (differential remainder ≤1.4e-10 V, L-L ≤2.7e-10 V);
  every other node matches to ~1e-14 rel; Y bit-identical; injections <1e-9 A
  apart; iterations equal. Un-pinnable across solvers at ANY LU accuracy this
  weak (a KLU-quality residual ~1e-11 A → ~5e-4 V slack), so even WP-R1
  refinement only shrinks, never closes, it.
- **DG_Prot_Fdr** — BG (0.6 kV) is a DEAD-END bus behind Transformer.Tg
  winding 2 with no zero-seq ground reference (the deck's Generator.WindGen1
  at Bg is commented out): pinning ~1e-12 S vs ~0.42 S diagonal →
  amplification 4.2e11. The whole 1.0563e-2 V gap is common mode on the three
  BG nodes (differential ≤6.5e-14 V — bit-level); every other node ~2e-14 rel;
  Y bit-identical (worst 5.8e-16 rel); injections <1e-9 A; iterations equal.
  At 4e11 even a 1e-13 A residual moves it ~0.04 V.
- **LVTestCaseNorthAmerican Master/SecPar** — the substation transformers are
  **delta–delta** (230/13.8 kV) and every distribution transformer is delta on
  the MV side, so the ENTIRE 13.8 kV system floats in zero-seq. Master: all
  MV buses shift by the identical complex 2.354e-3 V (per-bus differential
  8.2e-7 V — well inside the `large` floors); SecPar: common 1.94e-3 V,
  per-bus differential ≤1e-5 V, LV nodes ≤5.4e-8 V. Y pattern identical with
  worst entry 1.28e-15 rel (libm last-ulp in the LineGeometry line-constants —
  these decks build lines from geometry); the observed 2.9e-7-rel common mode
  ≈ (ppm-pinning amplification ~1e8) × (ulp-level Y/solve perturbations) —
  arithmetic consistent with the floor, impossible for a model bug that small.

Element currents/powers are functions of the differential voltages and are
immune to the common mode, so every other floor stays at `large` and real
model bugs at these buses remain caught.

### ultra-switch (`large_ultra_switch`): stitching pseudo-switches at Y≈1e8 S

The A-Diakoptics torn circuits stitch subsystems with deliberate near-zero
lines (`Line.other_feeders` r1=1e-8 x1=1e-9 Ω → Y≈1e8 S; EPRI_Ckt7-G
`Line.333` alike). The pseudo-switch current is `Y·(V1−V2)` where V1−V2 is a
~1e-10-rel difference of ~2e4 V nodes: the engines' (V1−V2) agree to **<2
f64-ulps** — measured dI = 6.49e-4 A on a 375 A flow (ckt24) = 1.8 ulp ×
1e8 S, a pure arithmetic bit-floor (the f32-looking reported values are the
coarse dyadic grid such near-cancellation differences live on — BOTH engines
produce them). `i_abs` **2e-3** = worst 6.5e-4 ×3; voltages hold the full
`large` floors and Y stays tight, so a real stitching/model bug still shows at
ampere scale or in Y. (The EPRI_Ckt5-G torn pair clears the plain `large`
floors — its worst pseudo-switch current diff is 1.03e-5 A — and stays kind
`large`.)

### near-ideal-source (`large_near_ideal_source`): cross-solver junk at κ≈1e12

The AutoTrans validation family (`Test/AutoTrans/*` = `Version8/.../AutoTrans/*`
byte-identical; Auto1bus/Auto3bus build the autotransformer from 1-phase
3-winding `Transformer`s, AutoHLT from one 3-phase 3-winding, AutoAuto from the
`AutoTrans` class) stacks a `mvasc3=2000000` source (≈1.7e7 S), 1e-6 Ω switch
lines (≈1e6 S), ~1e-5 S magnetizing branches and a floating delta tertiary —
assembled-Y condition ≈1e12. Proof this is a floor and not an element bug
(2026-07-10, user-challenged line-by-line audit): the assembled system Y is
**BIT-IDENTICAL** across engines on every family deck at every probed stage
(strictly stronger than an eyeball formula comparison — it pins every YPrim
formula and the whole 8-edit deck sequence); iteration counts equal at every
solve; the constant-P load loop is self-consistent (dI = |I_comp|·dV/|V| to
2%); and the **one-shot** cross-solver spread on bit-identical `(Y, I)`
reproduces the entire gap — faer-vs-KLU 5.505e-2 V at the 88 kV LOW bus vs the
engines' 5.472e-2 V; scipy+rowscale 6.9e-2 V. Family worst: Auto3bus 1.46e-6
rel.

A second, per-element decomposition round (2026-07-10, `Auto1bus-step1`,
hex-bit transport) closed every remaining channel:

- **Element formulas bit-exonerated on identical V**: substituting the oracle's
  NodeV bit-exactly into the Rust engine reproduces every element's `Currents`
  AND `Powers` **bit-for-bit** (max_ulp = 0 for all lines/transformers). The
  sole exception, `Vsource.source`, differs by 2.3e-10 A = half an ulp of the
  ≈3.35e6 A cancelling `Yprim·V − Iinj` operands — 6 orders under the band.
- **RHS**: 1 of 42 components differs by exactly 1 ulp (`inj.im` of the phase-2
  source injection, 9.2e5 A scale — libm `sin`/`cos` last-bit FPC↔Rust).
  Measured effect via same-solver substitution: **1.5e-11 V** — innocent.
- **Residual parity**: on the shared bit-identical `(Y, I)`,
  `‖Y·V − I‖₂` = 7.1e-2 (oracle/KLU) vs 1.3e-1 (Rust/faer) — the oracle's
  answer is no cleaner; both engines sit at the same junk floor, so even a
  perfectly refined Rust V would still miss the oracle's V by the oracle's own
  junk.
- **Third-solver junk span**: scipy `splu` on the same bits lands **51.6 V**
  away from BOTH engines — uniformly on the six floating-tertiary nodes (the
  zero-seq common mode) — with a residual (4.8e-2) slightly *better* than the
  oracle's own (5.6e-2). The mathematically-equivalent solution set spans ~51 V
  in the null direction; the faer↔KLU gap of 3.7e-3 V is four orders *tighter*
  than that span.

**Tier calibration (user-directed migration, 2026-07-10):** with the floor
proven by decomposition (all four legs above — the sanctioned path for a band
change), the family is gated under `large_near_ideal_source` = `large` with
two exceptions:

- `v_rel` **5e-6** — family worst 1.46e-6 rel (Auto3bus), ×3.4 headroom;
  `v_abs` stays at the `large` 1e-6 V (the relative band alone covers every
  node: 0.44 V at the 88 kV LOW bus vs 5.5e-2 V measured).
- `i_abs` **0.1 A** (user-set) — the V floor's linear image in the stiff-entry
  small currents (`dI = Y_src·dV_junk`, measured worst 9.375e-2 A on
  `Line.line1` at the no-load state = 94% of the band). A future faer/pin bump
  tripping this thin margin is a **re-triage signal, not a widen signal**. The
  voltage-scaled power floor maps it onto powers (`|V|·i_abs`), covering the
  measured 12–19 kVA junk image. Large currents (fault/full-load checks) hold
  `i_rel` = the `large` 1e-6.

**Additional members (2026-07-10):** `PV_currentkvarLimit_kvar` /
`_kvarNEG` — a deliberate "TheveninEquivalente" source with Z1=Z0=1e-8+j1e-8 Ω
(Y_src≈7e7 S). The Vsource current differs by 1.7–1.9e-4 A (vs the plain
`large` allowance ~1.4e-4) on a ~43 A flow = dI = Y_src·(~3 f64-ulps of the
7967 V source-bus dV); the PVSystem element itself matches, pinning the PV
model — only the source-current image of the V bit-floor exceeds the plain
band.

**Why the wide `i_abs` does not mask real element bugs:** the family's unique
validation surface is AutoTrans/Transformer YPrim assembly, and the **Y channel
keeps the tight `large` floors** (`y_rel` 1e-8 — today the assembled Y is
bit-identical, so ANY Y-level drift is a real regression caught immediately);
the report formulas (GetCurrents/Powers) are corpus-shared and pinned tight on
every other deck, and are bit-exonerated here (round 2b above). The only thing
the band absorbs is the proven solver-junk image. WP-R1 note: iterative
refinement DIVERGES on the floating tertiary here (one step: 1.3e-3 → 1492 V —
the `u·κ ≳ 1` limit in action), so WP-R1's gate must verify the residual
actually decreased and roll the step back otherwise; and even a perfect solve
cannot close the family gap below KLU's own junk (residual parity, leg 3), so
the tier is permanent until the oracle itself is re-pinned.

**Maintenance:** a new corpus case defaults to `large`; promote it to `feeder`
only after `corpus_live` confirms it holds the tighter floor. Golden tests that
drive a stiff network (`golden_ieee8500`, harmonics/protection/meter scenarios in
`golden_metering_monitors` / the DER/harmonics/protection scenarios) pass `"large"` explicitly.

## Field-specific exceptions

- **Y compared unfactored** — oracle `YMatrix.getYSparse(factor=False)` vs Rust
  `Dss::system_y_csc` (before `dss-sparse` row equilibration), so the comparison
  is solver-independent and a stale admittance is an order-1 relative miss.
- **Sparsity** compared over the union of both patterns with the abs floor; an
  entry below the floor on one side counts as agreement, a real pattern change is
  flagged.
- **Power abs floor is voltage-scaled** (`assert_power_close`): `P = V·conj(I)`,
  so the tolerated current error `i_abs` (A) maps to `|V|·i_abs` (VA). Using
  `|V_kv| = |P_kW|/|I_A|` keeps the power floor the exact image of the
  already-accepted current floor — without it, high-voltage near-zero
  through-currents (connector lines) would trip a flat kW floor.
- **Tap float 1e-12 rel** (`golden_feeders_controls.rs` / `golden_timeseries_controls.rs`):
  the discrete `tap_number` is exact; only the accumulated float differs by an ulp
  when regulators partition the same net movement differently.
- **Monitor channels are f32** (1:1 with Pascal `MonBuffer: pSingleArray`), so
  the comparison floor is the f32 ULP — monitor/dynamics channels at `i_rel`/`i_abs`
  (`golden_metering_monitors.rs`, `exec/tests/dynamics.rs`). The mode-5 wall-clock channels
  are skipped.
- **Dynamics fixpoint residuals** (`dSpeed`/`dTheta`/`speed`) are pinned against
  the oracle's actual (small, non-zero) value, not `≈0`: `dSpeed = (Pshaft +
  electrical_power)/Mmass` is a ~1.5e-8-rel residual the oracle reproduces; a value
  pin is a stronger guard than an `abs < ε` bound.
- **`Export Counts` is a `RustSubsetByKey` compare** (`golden_reports.rs`,
  `harness::compare_export`): the report lists every DSS class + its instance
  count, but the Rust class registry is a **proper subset** of the oracle's (only
  a subset of classes is ported so far). So the Rust file is required to be a
  *subset by class name* of the oracle file, with every shared class's count
  pinned **exactly** (integers, zero tolerance). This is **not** a blanket
  relaxation: it pins every ported class against the oracle and fails if the Rust
  engine ever reports a class the oracle doesn't have or a wrong count; the only
  thing ignored is the oracle's rows for classes we have not yet registered
  (`Isource`/`GICsource`/`AutoTrans`/`GICLine`/`GICTransformer`, …). A `require`
  key set (the deck-created + default-item classes) is additionally asserted
  present in the Rust output, so a dropped class / empty body cannot pass
  silently. Tightening to a full `ExactOrdered` compare needs **both** a complete
  registry **and** the Rust class-registration order reconciled to the oracle's
  `DSSClassList` order (they currently differ) — or an order-independent
  exact-set-by-key variant.
- **`Export`/`Show` reports (WP8): exact equality is the default**
  (`golden_reports.rs`, `harness::compare_export`). **WP8 exactness audit
  (2026-07-04):** every WP8 golden was re-measured cell-by-cell against its
  oracle capture; the produced reports are parse-value-identical (byte-identical
  modulo tokenization) on **74 of 93** compares, so those policies are pinned at
  `rel = 0, abs = 0` — the printed value *is* the contract, and the earlier
  "printing floor" tolerances (`EXPORT_REL = 1e-4`/`EXPORT_ABS = 1e-6`, the
  `%.1f`/`%.2f`/`%.3f` additive floors, `YMATRIX_REL`, the register/reliability/
  monitor floors) were **never exercised** — deleted, per the no-unproven-floors
  rule. Two independent engines rounding the same ~1e-9-agreeing f64 through a
  5–7-sig render almost never split a boundary; where a split or a genuine
  residual IS observed on a golden, that specific floor is kept, measured and
  documented below. If a new straddle ever fires (a regolden, a new fixture),
  prove it by decomposition first (both engines' f64 bracketing the print
  boundary), then add the narrowest floor. The split of duties is unchanged:
  `corpus_live.rs` gates the engine physics (1e-8 rel on the live complex
  model); the goldens gate the report-layer transform and its layout.
  The remaining non-exact floors, all **observed** on the current goldens:
  - `P_byphase` (kVA form): one `%10.3f` last-digit straddle (−1342.212↔.213)
    → `abs = 0.0011` (the MVA twin renders byte-identical → exact).
  - `Losses`: a 7-sig `%.7g` ULP straddle (65.34585↔86 W) → `rel = 1e-6`; plus
    near-zero no-load/var noise-vs-noise cells (≤ 1.28e-8 W, 6 orders below the
    smallest real loss) → `abs = 1e-7`.
  - `SeqVoltages`/`SeqCurrents`: near-zero seq-residual cells (measured 1e-11 V /
    1e-9 A) → `abs = 1e-9`/`1e-8`; the residual-numerator ratio cells
    (`%I0/I1` ≤ 1.2e-10) → `abs = 1e-9`; the one noise/noise ratio row gated.
  - `Currents`/`ElemCurrents`/`ElemPowers`/`YCurrents`: near-zero cancellation
    residuals (measured 3e-9 / 6.6e-12 / 1.6e-11 / 1.4e-14) → `abs = 1e-8` /
    `1e-10` / `1e-10` / `1e-13`; the angles of residual magnitudes gated
    (`PrevCol`).
  - `Show Mismatch`: a `%10.5f` straddle (473.76972↔71) → `abs = 1.1e-5`; the
    KCL-residual columns masked.
  - The **DI files**: the only genuine non-print floor — the per-step faer-vs-KLU
    difference integrates over the 24 daily solves into the registers (measured
    1.5e-8 rel) → `rel = 5e-8`.
  - `Summary`/`8500 Summary`: the wall-clock `DateTime` column masked.
- **WP8.2 sub-step 2a — the aggregate PD/PC power exports** (`Powers`/`Losses`/
  `P_byphase` on solved IEEE13, both kVA and MVA). All columns are real (kW/kvar/W
  or MW/Mvar); the engine V/I/P is pinned to 1e-8 by `corpus_live`.
  - `Powers` (both forms) — every `%11.1f` value renders byte-identical → **exact**
    (`rel = 0, abs = 0`).
  - `P_byphase` — values are `%10.3f`; the kVA form has an **observed** one-ULP
    last-digit straddle (measured max divergence exactly 1e-3, on the largest
    −1342.212 conductor) → `rel = 0, abs = 0.0011` (a `rel` band would mask a
    per-conductor scale drift — mutation-confirmed). The MVA twin renders
    byte-identical → **exact**.
  - `Losses` — `%.7g` (7 sig): an **observed** one-ULP straddle on a substantial
    loss (REG2's 65.34585↔65.34586 W = 1.53e-7 rel) → `rel = 1e-6`, the 7-sig
    render resolution. `abs = 1e-7` absorbs the near-zero no-load/var
    noise-vs-noise cells (faer-vs-KLU cancellation residuals, observed up to
    1.28e-8 W vs the smallest real loss 9.05e-3 W — a 6-order gap). No genuine
    line-loss cancellation floor materializes on IEEE13; should a metered feeder
    later show one, it must be **proven by decomposition** (CLAUDE.md), not by
    widening `rel`.

- **WP8.2 sub-step 2b — the symmetrical-component exports** (`SeqVoltages`/
  `SeqCurrents`/`SeqPowers` on solved IEEE13). Again a *report-layout* gate; the
  underlying V/I are pinned to 1e-8 by `corpus_live`. Column structure: the
  magnitude columns (`V1`/`V2`/`V0`/`Vresidual`, `I1`/`I2`/`I0`/`Iresidual`) are
  `%10.6g` (6 sig); the ratio/unbalance columns (`%V2/V1`, `%V0/V1`, `%NEMA`,
  `%Normal`, `%Emergency`, `%I2/I1`, `%I0/I1`) are `%8.4g` (4 sig).
  - **Magnitudes** are pinned at `rel = 0`: every real (non-residual) cell renders
    byte-identical. The `abs` floor covers only the near-zero seq residuals and is
    the **empirically measured** divergence, *not* a guess. `V0`/`V2` (and
    `I0`/`I2`) are a near-zero difference of three balanced phasors; faer and KLU
    agree on this well-conditioned solve far tighter than the crude `phase·1e-8`
    bound — the measured max abs divergence is **1e-11 V** (`SeqVoltages` V0 at
    the source bus, a 6th-sig render straddle 4.31950e-6↔4.31951e-6) and
    **1e-9 A** (`SeqCurrents` Iresidual, 3.47330e-4↔3.47331e-4). So `SeqVoltages`
    uses `abs = 1e-9` and `SeqCurrents` `abs = 1e-8` — both below the smallest
    real printed magnitude (`I1 = 5.8e-4 A`), so real cells stay pinned exactly.
  - **Ratio columns**: the `%I…`/`%NEMA` cells carry `abs = 1e-9` for the
    residual-numerator/healthy-denominator form (`%I0/I1` of a residual I0 over a
    loaded I1, observed ≤ 1.2e-10 — `rel` is meaningless there), `rel = 0` — real
    ratios (`%I2/I1 = 1.76`, the `%Normal`/`%Emergency` loadings) render
    byte-identical and are exact. Plus a **band-limited denominator gate**
    (`ColTol::gate`): `SeqCurrents` gates on `I1` (col 2) — at an open/unloaded
    terminal `I1`/`I2`/`I0`
    are ~1e-12 cancellation noise (pinned to 0 by the magnitudes' `abs`), so `%I2/I1`
    etc. are a faer-vs-KLU **noise/noise** form — e.g. `Line.671680.2` (the open 680
    end) prints `%I2/I1 = 147.7` (oracle) vs `61.8` (Rust), and the live f64 confirms
    both engines compute I1≈2e-12/I2≈2e-12 there (terminal 1 matches to 6 sig,
    Iresidual matches exactly). The gate skips the *ratio* cell **only where
    `0 < |I1| < 1e-6 A`** — the band-limit's lower bound is load-bearing: the 32
    rows where `I1` is *exactly* 0 (single-phase, non-positive-sequence) print the
    ratio as `0` (Pascal `if I1 > 0`), so `0 == 0` stays checked (audit-tests
    mutation-confirmed an un-band-limited gate masked a planted nonzero `%NEMA`
    there). Net: **1** genuine-noise row skipped, **54** rows' ratios still checked
    (22 meaningful + 32 zero; min meaningful `I1 = 5.8e-4 A`, 581× above the gate).
    The gate is **scoped to the columns that actually divide by `I1`** (`%I…`
    prefixes) or are the same noise form of the phase currents (`%NEMA`);
    `%Normal`/`%Emergency` divide by `NormAmps` (never near-zero), so they fall
    through to the exact default and stay checked even on the gated row.
    The magnitude columns are checked on **every** row. A proven cancellation floor
    (CLAUDE.md), **not** a relaxation. `SeqVoltages` needs no gate or ratio floor
    (V1 is always kV-scale; its ratio cells are byte-identical) and `SeqPowers` is
    all `…:1` fixed-decimal, byte-identical → **exact**; its PD rows carry the
    extra 4 excess-kVA columns on terminal 1 (12 fields) vs 8 for PC/term-2 rows,
    and the comparator pins each row's field count.
  - **Coverage gaps (tracked):** the `SeqPowers` MVA (`opt = 1`) path is ported but
    unreachable through `export seqpowers` dispatch (ptr 10 never reads the `m…`
    flag — `ExportOptions.pas:191`), so no golden can reach it; the `SeqCurrents`
    Faults walk and the `nphases < 3` / `< 3`-node positive-sequence branches are
    code-faithful but unexercised by IEEE13 (no Fault objects; full 3-phase deck).
    A feeder with a genuine 2-phase nonzero-`%NEMA` terminal would close the last;
    deferred (no corpus deck), recorded in STATUS §1f.
  - **`TODO(compat)` in `SeqCurrents`** (`seq_currents.rs`): `Iresidual` sums the
    *terminal-1* conductors for **every** terminal row (Pascal indexes `cBuffer^[i]`,
    not `cBuffer^[(j-1)*Ncond+i]`) — an upstream quirk reproduced verbatim so the
    golden's `Iresidual` column matches; the clean fix is the per-terminal slice.

- **WP8.2 sub-step 2c — the per-terminal/per-conductor element exports**
  (`Currents`/`ElemCurrents`/`ElemVoltages`/`ElemPowers`/`NodeOrder`/`Taps` on
  solved IEEE13). Report-layout gate again; the engine V/I are pinned to 1e-8 by
  `corpus_live`. These are magnitude (`%10.6g`, 6 sig) + angle (`%8.2f`, 2 dec)
  reports; `ElemPowers` is kW/kvar (`%10.6g`), `NodeOrder`/`Taps` are exact.
  - **Magnitudes** are pinned at `rel = 0` (real cells byte-identical); the `abs`
    floor covers only the near-zero cancellation residuals, at the **measured**
    level per report: `Currents` `abs = 1e-8` (the per-terminal `Iresid`
    residuals, observed 8.1e-9↔5.1e-9 A and 3.6e-12 vs an exact `0`),
    `ElemCurrents` `abs = 1e-10` (open-terminal conductors, observed diff
    6.6e-12 A), `ElemPowers` `abs = 1e-10` (neutral-conductor powers, observed
    1.6e-11 kW), `ElemVoltages` **exact** (`abs = 0` — the grounded-neutral
    conductors are an exact `0` on both engines; no residual class). Each floor
    is 4+ orders below the report's smallest real magnitude.
  - **Angle columns** are exact (`rel = 0, abs = 0`) on every real magnitude.
    Selected by **`ColSel::Parity`** (index parity), **not** name-prefix, because
    these reports have **truncated headers** (`…, I_1, Ang_1, ...`) that name only
    the first pair: `Parity { start, parity: 1 }` picks every angle column,
    `start = 1` for `Currents` (all columns are pairs), `start = 3` for
    `ElemCurrents` (after `Element, Nterms, Nconds`). The angle of a **near-zero**
    magnitude (a residual / open-terminal conductor) is a faer-vs-KLU noise value
    carrying no information (observed `-33.69` vs `56.31`°), so it is **gated on
    its paired magnitude** (`GateSpec::PrevCol`, the immediately-preceding
    column): skipped only where `0 < |mag| < 1e-6` — the band-limit keeps every
    exactly-zero row's `0.00 == 0.00` check and every real-magnitude row's angle
    (e.g. `Currents` `AngResid` on a real neutral-return residual stays checked).
    A proven cancellation floor (CLAUDE.md), **not** a relaxation.
  - **`ElemPowers` Vsource / order note** (`elem.rs`): the per-conductor power is
    `Vterminal·conj(Iterminal)`, and `compute_iterminal` is called **before**
    `compute_vterminal` (the reverse of Pascal's textual order). Pascal's
    `ComputeIterminal` is a post-solve no-op (`ITerminalUpdated = TRUE`), so
    `Vterminal` stays `NodeV`; our `compute_iterminal` re-runs `GetCurrents`, and
    `TVsourceObj.GetCurrents` overwrites `Vterminal` with the source EMF
    `[Vsource; 0]` (≈ but ≠ `NodeV`), which would print the isolated-source
    `-612.936` 2a surfaced. Computing `iterminal` first, then `vterminal`, restores
    `Vterminal = NodeV`, reproducing the oracle's observable `NodeV·conj(I)`
    (`-612.729`). Not a `TODO(compat)` — it reproduces the oracle exactly; the note
    records *why* the order is inverted from the Pascal source text.
  - **`Taps`** is exact (`rel = 0, abs = 0`): the tap value is the discrete
    `mid + position·increment`, so both engines print the identical value once they
    converge to the same integer tap position (pinned by `corpus_live` + the
    feeder-controls gate); a tap-position divergence (≥ 0.00625) fails loudly.
  - **`NodeOrder` guard:** `WriteNodeList`/`WriteElem*` error 222001 per element on
    an unsolved circuit and exit early (header-only file). The formatter reproduces
    the header-only body; the router records the 222001 once (Pascal once per
    element — the observable file + error presence match, no gate checks the count).
    Untested (no unsolved-circuit corpus/golden deck); the solved-circuit path is
    the gated one.
  - **Coverage gaps (tracked):** the `Currents` filler for `Nterms < TermWidth` and
    the near-zero-angle gate's exactly-zero branch are exercised by IEEE13; the
    222001 unsolved path and a Fault-object walk are code-faithful but unexercised
    (recorded in STATUS §1f).

- **WP8.2 sub-step 3 — the matrix/summary exports** (`Yprims`/`Y`/`SeqZ`/`Summary`/
  `Result`). Report-layout gates; the underlying quantities are already pinned
  entry-by-entry by `corpus_live` / the checkpoint gate / `fault_study.rs`.
  - **`Y`/`Yprims` — exact** (`rel = 0, abs = 0`). The assembled system Y and each
    element's primitive Y are **exact deterministic stamps** — no faer solve enters
    them — byte-identical at 10 sig (`%.10g`). The `Y` golden pins the
    **sparse-triplet** form
    (`export y triplet`, `Row,Col,G,B`, lower triangle `r>=c`, column-major): the
    **dense** form glues `+j` onto every imaginary token, which does not parse as a
    number, so `compare_export` cannot diff it — the dense *values* are instead
    pinned by the checkpoint + live full-Y gates. Integer Row/Col exact.
  - **`SeqZ`** (per-bus `Zsc1`/`Zsc0`, on a **FaultStudy** fixture — a snapshot
    leaves `Zsc` unallocated → the degenerate all-zero/1000-ratio report) —
    **exact**: the `R1/X1/R0/X0/Z1/Z0` magnitudes, the `X1/R1`/`X0/R0` ratios and
    the integer `NumNodes` all render byte-identical (the same `Zsc1`/`Zsc0`
    `fault_study.rs` pins to 1e-9·mag).
  - **`Summary` — `DateTime` masked.** Column 0 is `DateTimeToStr(Now)`, a
    genuinely non-deterministic wall-clock timestamp, **masked** via the new
    `ColSel::Index(0)` + `GateSpec::Mask` (a masked non-deterministic column, **not**
    a value relaxation — everything else is checked). The rest is deterministic
    and **exact**: text `Status`/`Mode`/`ControlMode` compare case-insensitively;
    the integer counts and the `%g` scalars (Max/MinPuVoltage, Total MW/Mvar,
    losses) render byte-identical. The golden uses the shared compile+solve
    fixture, so the two sides are self-consistent.
  - **`Result`** is exact text (`null`). The pinned oracle is a `DSS_CAPI_PM` build
    that never updates `@result` (`ExecCommands.pas:704` is compiled out), so it
    stays at its `'null'` init forever; our engine never writes `@result` either, so
    the single `null` line matches with zero divergence — not a masked/relaxed field.

## Live corpus gate (`corpus_live.rs`)

Reuses the same comparators and classes verbatim. Differences from the checkpoint
goldens: the **full** assembled Y is compared every case (nothing is stored, so
size is irrelevant — checkpoints store only a fingerprint for large feeders); the
Rust/oracle element name sets must be identical; monitors/meters are compared live
only for cases that sample them in deterministic modes (`check_meters_monitors` in
`solvable_now.json`) — an unsampled monitor returns a phantom channel from the
pinned oracle, an artifact, not an engine gap.

## WP8.5b property parity (`harness::compare_all_properties` / `SKIP_PROPS`)

The corpus property-parity gate compares **every** element's **every** property
value — the Rust `?`-surface (`Dss::element_properties`:
`refresh_vterminal_if_marked` + `ClassProps::get_value`) vs the pinned oracle's
`Properties(p).Val` (read via `? name.prop`) — property-name lists equal in order,
each value by numeric skeleton at the case tolerance. Values compare **case-exact**
(no lowercasing): every DSS enum getter renders the Pascal-faithful case
(`ordinal_to_string` returns the exact registry strings — `wye`/`delta` lowercase,
`Variable`/`Fixed` capitalized, booleans `Yes`/`No`), so a case divergence is a
real rendering regression, not a formatting artifact. `SKIP_PROPS` excludes a
`(class, prop)` from the **value** compare (the name is still order-checked). Each
is a proven **comparability exclusion**, never a tolerance loosening — the property
is genuinely non-comparable, not merely loose:

- **`Capacitor.CMatrix`, `Reactor.RMatrix`, `Reactor.XMatrix`, `Fault.GMatrix`**
  (`DoubleSymMatrixProperty`) — the dss_capi getter reads **uninitialized memory**
  (the same bug the `TODO(compat)` at `obj/props/class_props/value.rs` reproduces
  by rendering a deterministic zero matrix). The live oracle returns
  **process-dependent garbage** (denormals ~1e-310 in one run, huge ~1e123 in
  another — proven nondeterministic). Oracle UB → not reproduced (CLAUDE.md rule),
  so not comparable.
- **`Transformer.WdgCurrents`** — renders `mag, (angle)` phasor pairs. A winding
  whose current is ~1e-12 A (a numerically-zero internal/neutral current, e.g. the
  delta/wye tertiary) has an **undefined angle**: both engines agree the magnitude
  is ≈0 (matches within the abs floor), but its angle is faer-vs-KLU cancellation
  noise (the same class the export comparator gates via `GateSpec::PrevCol`). The
  magnitudes and all non-degenerate winding angles match; only the zero-magnitude
  angle diverges. The winding currents' physics is gated by the model compare
  (terminal currents / YPrim).
- **`Transformer.Wdg` + the per-winding SINGULAR forms** (`Bus`, `Conn`, `kV`,
  `kVA`, `Tap`, `%R`, `RNeut`, `XNeut`, `MaxTap`, `MinTap`, `NumTaps`, `RDCOhms`
  — `TRANSFORMER_CURSOR_PROPS`) — skipped **only when the two engines'
  ActiveWinding cursors disagree** (`skip_transformer_cursor`, gated on the `Wdg`
  value: Rust's from `element_properties`, the oracle's from the capture). `Wdg`
  is the transient `ActiveWinding` edit-cursor (which winding a subsequent
  `~ tap=` applies to); the singular forms render `windings[ActiveWinding]`'s
  value, so the two engines compare validly only when both cursors point to the
  same winding. The oracle's own live capture mutates the cursor:
  `gc.capture_discrete` walks `Transformers.Wdg = i` over every winding (to read
  taps) **before** the property sweep, forcing it to `NumWindings`. Rust keeps the
  deck's trailing `wdg=` — usually **also** `NumWindings` (the array-form parse
  lands there), so the cursors AGREE and every singular form is compared, incl.
  `RDCOhms`. They disagree only when the deck ends on a non-last winding: the
  3-winding `t3w` (ends `wdg=2`; oracle capture reads winding 3) and the 2-winding
  `YgD-Test.tr1` (rewired `wdg=1`; capture reads winding 2). Proven for `t3w`: in
  isolation both engines render the deck's `wdg=2`; only the post-`capture_discrete`
  sweep reads 3. **Array-form backstops** (compared whenever cursors agree, and
  independent of the cursor anyway): `Bus`→`Buses`, `Conn`→`Conns`, `kV`→`kVs`,
  `kVA`→`kVAs`, `Tap`→`Taps`, `%R`→`%Rs`; `RNeut`/`XNeut` are also in the YPrim;
  `MaxTap`/`MinTap`/`NumTaps` feed the now-live-gated `TapNum`. **`RDCOhms` has NO
  array/other backstop**, so gating on cursor-agreement (rather than dropping it
  outright) keeps a `RDCOhms` formula/scale regression caught on the overwhelming
  majority of transformers (every deck where the cursors agree).
- **`Capacitor.FaultRate` / `Capacitor.pctperm` / `Reactor.FaultRate` /
  `Reactor.pctperm`** — reliability inputs. In a **metered** deck the oracle reads
  these **uninitialized** on shunt PD elements (Capacitor/Reactor): proven
  nondeterministic across processes (`7.54e-312` vs `1.38e-311` on the same deck),
  present already at compile time. Rust keeps the correct defaults (FaultRate
  `0.0005`, pctperm `100`). Oracle UB → not reproduced. The **same** properties on
  Line/Transformer are clean and stay compared (31 lines + 6 transformers in
  `midi_controls`), so the `Double`-property render path is still gated.

A real port bug this gate caught and fixed (not a skip): **`RegControl.TapNum`**
rendered the cached `tap_snap` while Pascal `Get_TapNum` (`RegControl.pas`) reads
the controlled transformer's **live** `PresentTap[TapWinding]`; the `&self` getter
now resyncs the snapshot from the live transformer at the read choke point
(`Dss::refresh_vterminal_if_marked`).

## §AD — A-Diakoptics AD↔normal equivalence (D7 calibration, WP-AD.3)

The A-Diakoptics solve and the normal solve are two iterations to *almost* the
same fixpoint: the `Y4`/`Ic` boundary model is a **first-order** approximation of
the inter-zone coupling, so the AD fixpoint sits a **fixed** distance from the
interconnected fixpoint — a genuine method floor, not a bug. Per plan D7 the tier
is calibrated once, here, and never loosened.

Calibration (r3723 Oddie probe by the WP-AD.3 resume executor, 2026-07-12;
`solve mode=snap`, `controlmode=off`, `Num_SubCircuits=2`, worst node by max rel
`|V_ad − V_normal|`):

| fixture | oracle floor (r3723) | Rust floor | D7 tier (×4) |
|---|---|---|---|
| midi (2 zones) | 3.25e-5 | 3.21e-5 | 1.3e-4 |
| midi (3 zones) | — | 3.21e-5 | 1.3e-4 |
| macro (2 zones) | 1.318e-4 | 1.319e-4 | 5.3e-4 |

**Tolerance stability (the permanent tighten-proof).** On the oracle the floor is
bit-stable as `ConvergenceTolerance` tightens 1e-4 → 1e-10 (midi 3.265e-5 →
3.254e-5; macro 1.318e-4 flat at every tol; iteration counts `itN == itA`), i.e.
it does **not** collapse — proof the two engines share the fixpoint and the gap is
the boundary-model floor. `midi_d7_gap_stable_under_tighten` /
`macro_d7_gap_stable_under_tighten` (`tests/adiakoptics.rs`) assert the Rust
loose/tight ratio stays in `[0.5, 2.0]`; a gap that *collapses* would flag a
cross-step state leak, a gap that *balloons* a port bug. Both stay flat.

**Documented deviation folded into this floor** (`ad_solve_into_parent`): official
freezes each child's own `NodeV` at its state-2 standalone solve (probe: actor-3
`NodeV` moves `0.0` across the AD solve; that state-2 solve is already 7.9e-5 from
interconnected via `PConn_Voltages`). A byte-faithful freeze diverges in this port
to 3.46e-3 at the deep node of the **reference-free** zone (its `Start_Diakoptics`
disables all sources → `hY` anchored only by load `Yeq` shunts, near singular →
faer↔KLU factor gap amplified down the long radial). Re-seeding the child `NodeV`
with the solved column each iteration recovers the floors above — an explicit,
documented compensation (never a silent Y regularization, §5). Open item: make the
reference-free zone solve match KLU so the re-seed can be dropped.

## `TODO(compat)`

Tolerances absorb f64/ULP differences only. Deliberately-reproduced upstream
inexactnesses are **not** handled here — they are marked `TODO(compat)` in the
code and pinned by exact golden values, removed in one pass after the 1:1 port
reaches final acceptance (PORTING_PLAN.md §4.1 / §6).
