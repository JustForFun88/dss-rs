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

> **The divergence ledger is not a tolerance.** `tests/corpus/ledger.json`
> (TESTING.md §"The divergence ledger") pins *specific, measured upstream
> divergences* per case / per channel / per scope — it never widens a floor:
> everything a ledger entry does not explicitly scope is still held to the
> tiers below by the untouched `harness` comparators, at least one scoped value
> must still **exceed** its tier floor every run (else the entry is stale and
> the gate fails), and every entry change is fingerprinted into the population
> lock. Ledger envelopes are sized to live measurements (`DSS_LEDGER_MEASURE`),
> require a documented cause + source, and may be added only after the
> divergence is proven not to be a port bug. Tier floors are structurally
> unreachable from ledger code.

## Tolerance classes (`harness::tol_for`)

The assembled-model gate (`golden_checkpoints.rs` + the live `corpus_gate.rs`)
tiers tolerances by **conditioning**, not size:

- **micro** — tiny synthetic circuits.
- **feeder** — well-conditioned standard feeders (IEEE13/37, simpler `Test/`
  circuits).
- **large** — large or numerically-stiff networks (EPRI ckt5 / 8500-Node /
  A-Diakoptics torn zones / inverter cases / IEEE123 / 4Bus-YYD). Also the safe
  default for an unrecognized `kind`.
- **large_floating_delta** — `large` plus one documented exception: `v_abs`
  5e-4 V (see §floating-delta below). The whole-IEEE123 grid-forming-inverter
  (GFM) family (11 decks): the original decomposition-proven
  `GFM_IEEE123/Run_IEEE123Bus_GFMSnap.DSS` plus the CF-B-migrated GFM IEEE123
  snapshots (`GFMSnap`, `GFMSnap-A/-B/-C`) and daily/whole-day trajectories
  (`GFMDaily`, `GFMDailySwapRef`, `GFMWholeDaily`, the AmpsLimit variants, the
  IBRDynamics `CannotPickUpLoad`). All share the identical floating-delta
  topology — the proof and the coverage argument are in §floating-delta below.
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
- **micro_wtg3_dynamics** — a `micro`-scale WindGen (WTG3) dynamics tier with
  `v_rel` 8e-6, `i_rel` 2e-5, `i_abs` 1e-4 (Y stays tight at `y_rel` 1e-9); see
  §wtg3-dynamics below. Two members (`windgen_dyn`, `windgen_dyn_fault`): the
  phase-locked-loop derivative term amplifies a last-ulp `Vq` cancellation by
  6e4×, loosening only the three PLL-fed quantities (`dOmg`/`Pgen`/`Qgen`) while
  every non-PLL state variable and the assembled Y hold tight.

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

**Family admission (CF-B, 2026-07-12).** The tier now also gates the rest of
the whole-IEEE123 GFM family — the `GFM_IEEE123` / `GFM_AmpsLimit_123` /
`IBRDynamics_Cases/GFM_IEEE123` snapshots (`GFMSnap-A/-B/-C`) and the
daily/whole-day trajectories (`GFMDaily`, `GFMDailySwapRef`, `GFMWholeDaily`,
the AmpsLimit variants, `CannotPickUpLoad`). **What is proven vs what is
inherited:** the decomposition above (bitwise Y/RHS/residual audit, 100%
common mode) was performed on `GFMSnap.DSS`; the other decks are the *same
physical circuit* (the whole IEEE123 feeder with delta-connected Storage/PV
GFM inverters and no zero-sequence ground path) driven to a different operating
point — snapshot vs a scripted daily trajectory whose gated compare is the
final converged step. They are admitted under that precedent, **not** an
independent per-deck decomposition of each trajectory. This is honest and safe
because the floor is widened on **exactly one channel** (`v_abs`) and every
channel that could expose a real model/tap/control divergence is held at its
tight floor and is *immune to the common mode*: the assembled Y at `y_rel`
1e-8 (deterministic stamps, today bit-identical), the injection, the **exact**
iteration count, and the DER currents/powers (differential-voltage functions)
at the `large` floors. A daily deck whose regulators or GFM controllers landed
a different state, or whose model drifted, would move a tap / an iteration
count / a differential current far above these floors and fail loudly — the
5e-4 V common-mode band absorbs only the proven un-pinnable zero-sequence
junk, which the topology guarantees is present in every family member. Each
deck was additionally validated live-green against the pinned oracle before
migration (see the per-deck notes in `solvable_now.json`). If WP-R1 ever
retightens this tier, the whole family retightens with it.

### floating-zeroseq (`large_floating_zeroseq`): weakly-pinned common modes

The same physical class as §floating-delta — a subsystem reachable only
through delta windings has no zero-sequence ground path, and its common-mode
voltage is pinned solely by the ppm anti-float adders — but with pinning 2–3
orders weaker than GFMSnap's (amplification 3.1e8 there), so the junk exceeds
the `large_floating_delta` band and gets its own `v_abs` **3e-2 V** (worst
measured 1.06e-2 ×2.8; at the smallest affected 346 V buses that is 8.7e-5
rel). Per-deck proof by decomposition (2026-07-10, DOCTechNote family
2026-07-11):

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
- **DOCTechNote family** (`DOCTechNote/1_1`, `1_2`, `2_1`, `2_2`) — the SAME
  circuit as LVTestCaseNorthAmerican (each `Redirect`s its `Master.dss` +
  `network_protectors.dss` + `energy_meters.dss`) plus one perturbation:
  1_1/1_2 add an SLG fault on an **LV** secondary bus (S21 / S203), 2_1 opens a
  network-protector breaker (`Line.10_sw`), 2_2 adds OC relays + an **L-L**
  (phase-to-phase) fault on MV bus p105 under `controlmode=event`. None of
  these introduces an MV zero-seq ground path — the SLG grounds only the
  wye-grounded LV (blocked from MV by the delta distribution primaries), the
  L-L fault couples phases 1–2 without touching ground — so the whole 13.8 kV
  system still floats, exactly as in the base deck. Decomposition (2026-07-11,
  full 1170-node dump both engines): the gap is a per-bus **zero-sequence
  common mode** of 1.81e-3…2.85e-3 V on the MV buses (well inside the 3e-2 band;
  ~2.2e-7…3.6e-7 rel at the 8 kV buses). Proof it is 100% common mode: the
  **L-L (differential) node voltages agree to ≤1.4e-10 rel** (max |ΔV_LL|
  2.1e-5 V — ~1.5 f64-ulp, four orders under `large`'s 1e-7), and **removing
  each bus's mean shift leaves 0/1170 nodes above the `large` band** (max
  residual 1.15e-5 V, ≤1.8e-9 rel). Un-pinnable, not iteration-driven: at
  `ConvergenceTolerance 1e-10` the gap is byte-unchanged (2.389e-3 → 2.389e-3)
  while Rust can no longer converge (15 iters, non-converged on 1_1/2_1 — the
  residual floors out in the near-null zero-seq direction); at the default
  tolerance both engines converge in the identical iteration count. Y
  bit-identical, injections match, iterations equal and element currents/powers
  within `large` — all verified by the `run_and_compare` full compare passing
  at `large_floating_zeroseq` (which pins Y at 1e-8, injection, iterations
  exactly, and currents/powers at the `large` floors).

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

**The full (un-torn) ckt24 substation family (CF-D, 2026-07-12).** The whole
EPRI ckt24 family — `EPRITestCircuits/ckt24/Run_Ckt24` + `master_ckt24`, and the
seven `MemoryMappingLoadShapes/ckt24/master_ckt24{,-mm-csv-pq,-mm-dbl-p,-mm-sng-p,
-mm-txt-p,-mm-txt-pq,-nomm}` file-array variants — carries the **same**
`Line.Other_Feeders` (`r1=1e-8 x1=1e-9 Ω` at the default `length=1` → `|Z|≈1e-8 Ω`
→ **Y≈1e8 S**) that stitches the substation transformer low side (`SubXfmr_LSB`)
to `Feeders`. It was long mislabeled a "RegControl/LDC SubXFMR tap-current
divergence" (STATUS open follow-up). CF-D root-cause proved that wrong: the node V
is at the faer-vs-KLU floor (rel 1.8e-8 – 4.7e-8), the regulator lands the
**identical** tap, and the "SubXFMR current" gate (|diff| 7.2e-4 A, ~4.7e-5 rel)
is exactly the ultra-switch `Y·(V1−V2)` near-cancellation image through
`Line.Other_Feeders` — the same class as the torn ckt24 already banded here:
`1.8 ulp × 1e8 S = 6.5e-4 A` on the ~2e4 V nodes (the identical arithmetic the
`large_ultra_switch` band note carries in `harness/mod.rs`), of which the measured
7.2e-4 A SubXFMR terminal diff is one instance, all < the `i_abs` 2e-3 band. The
regulator and the delta-wye substation transformer are exonerated. So the family
migrates to `large_ultra_switch` with the existing band (V + Y stay tight; only
the pseudo-switch current image is absorbed).

*Per-element current decomposition* (CF-D settle, `DSS_DUMP_IDIFF` live probe over
the whole family — not by analogy): the two dominant per-element terminal-current
diffs on every member are `Line.other_feeders` (the ultra-switch, **7.1e-4 –
7.7e-4 A**) and `Transformer.subxfmr` (**5.1e-4 – 7.2e-4 A** — the substation
transformer whose LSB terminal feeds that switch, i.e. the *same* `Y·(V1−V2)`
image; this is the "SubXFMR current" the label pointed at). Both are 2.6–4× under
the `i_abs` 2e-3 band. The third tier drops an order of magnitude to ≤7.4e-5 A (a
downstream transformer/load, ~27× under the band); nothing else is close. So the
dominant current diff *is* the pseudo-switch image on the switch and its
transformer, shown sub-band; `v_rel` 1e-7 / `y_rel` 1e-8 stay tight, so a real
transformer/YPrim regression would still trip the gate.
**(NB: `Y≈1e8 S`, not the `1e10 S`
the CF-D commit 6200fe0 message and the first draft of this note stated — a 100×
typo cross-contaminated from SecondaryTest's genuine 1e10 busbar below; the band
value, gate, and floor verdict are unaffected — the arithmetic closes only at 1e8,
`1.8·ulp(2e4 V)≈6.5e-12 V × 1e8 S = 6.5e-4 A`, corrected CF-D settle 2026-07-12.)**

**TC-3 near-floor family — per-element current decomposition (CF-D settle,
2026-07-12).** The five decks migrated to plain `large` (GFM_IEEE8500
Snap/Daily/DailySmallerPV, Storage `Run_Demo1`, CIM `IEEE13_CDPSM`) tripped only
the `large`-tier *absolute* voltage band at their first-failing node (V rel
1.5e-8 – 7.7e-8, i.e. the faer-vs-KLU floor made an abs V diff at 7–20 kV nodes).
Node-V floor alone does NOT clear a case (CLAUDE.md) — the deciding diagnostic is
the per-element current/power decomposition, which the T-C triage flagged as not
yet done. Run here via the `DSS_DUMP_IDIFF` live probe (worst per-element terminal
`|dI|` vs the pinned oracle); all are comfortably inside the `large` floors
(`i_rel` 1e-6, `i_abs` 1e-4):

| deck | worst per-element `|dI|` | element | vs `large` `i_abs` 1e-4 |
|---|---|---|---|
| GFM Snap | 2.46e-7 A (rel 5.3e-10) | `Line.hvmv_sub_connector` | ~400× under |
| GFM DailySmallerPV | 2.39e-7 A (rel 7.3e-10) | `Line.hvmv_sub_connector` | ~400× under |
| GFM Daily | 4.00e-7 A (rel 1.3e-9) | `Line.hvmv_sub_connector` | ~250× under |
| Storage `Run_Demo1` | 9.11e-5 A (rel 2.0e-6) | `Line.mdv201_c_1_266_abc8079` | 0.91× (tightest) |
| CIM `IEEE13_CDPSM` | 4.26e-6 A (rel 1.1e-7) | `Line.fuse1` | ~23× under |

The GFM result is the tell: the islanded-GFM node-V offset is common-mode-like —
the *absolute* node V shifts at the faer-vs-KLU floor (rel ~2e-8) but every
element current (driven by V *differences* across it) stays sub-microamp (≤4e-7 A,
≥250× under the current floor), the same shape as `large_near_ideal_source`.
`Run_Demo1` is the only tight one (9.1e-5 A = 91 % of the 1e-4 floor, still a
genuine faer-vs-KLU current-floor instance, no element above it). No element on
any of the five exceeds its floor → all decompose to the floor, migration honest;
`i_rel`/`i_abs` were NOT touched. (CIM `IEEE13_CDPSM`'s largest *raw* `|dI|` is a
1.38e-5 A near-zero winding current on `Transformer.sub3` — a small abs on a
~0-magnitude terminal, absorbed by `i_abs`; the largest *meaningful*-magnitude
diff is the `Line.fuse1` row above.)

(`DSS_DUMP_IDIFF` was a throwaway settle-time probe — a per-element `|dI|` dump in
`harness::compare_element`, reverted after capture; it is not in the tree. The
figures above are the recorded artifact. What is *permanent* is stronger than the
probe: the live gate's `compare_element` asserts every element's currents/powers
against these `large`/`large_ultra_switch` floors on every run, so a future
transformer/YPrim/switch regression that pushed any of these above the floor would
fail the gate — the decomposition is continuously enforced, not a one-time claim.)

### conditioning_floor (documented, NOT banded): SecondaryTestCircuit_modified

`IEEETestCases/SecondaryTestCircuit_modified/Master.DSS` was also mislabeled the
"RegControl/LDC SubXFMR" family. CF-D root-cause (2026-07-12) proved it is a
**cross-solver conditioning floor**, not a RegControl/transformer bug, and — unlike
the ckt24 family — its junk lands in the *voltage* (not just a switch current), too
large for any existing band, so it stays documented in
`skipped_needs_investigation.json` (the `floating_zeroseq_bus` precedent: a
root-caused floor that cannot yet be banded honestly).

- **Structure.** Two `Linecode.BUSBAR` lines (`r1=x1=r0=x0=1e-4 Ω/km`, `c=0`,
  `units=km`) in series with the 46 kV `Vsource`: `Line.MDV_SUB_1_HSB`
  (source→SubXfmr-HV) at `length=0.001 units=m` = 1e-6 km → ~1e-10 Ω → **Y ≈ 1e10
  S**, and `Line.SSswitch` (feederhead→BusPrim1) at `length=1 units=m` = 1e-3 km →
  ~1e-7 Ω → Y ≈ 1e7 S. The κ dominance comes from `MDV_SUB_1_HSB`; assembled-Y
  condition number **9.79e11**.
- **Symptom.** Step-0 entry-0 node V at BUSSOURCE/MDV_SUB_1_HSB |diff| = **0.553 V**
  (rel 2.08e-5), with a uniform ~8e-4° angle offset; the regulated 12.47 kV bus
  matches ~1e-6.
- **Proof it is a floor, by decomposition (not a tolerance sweep):**
  1. The **assembled system Y is BIT-IDENTICAL** between engines — all 570 CSC
     entries, max |ΔY| = 0 (so the linear operator is not the difference).
  2. The **Vsource injection** `Yprim·E` (backed out as `Yprim·[V,0] − Iterm`) is
     **BIT-IDENTICAL** (max 2.3e-13 A, matrix-multiply noise) — the source EMF
     matches; the `duty=SubVoltage` shape is applied identically.
  3. Load base kW/kvar (derived from the `UseActual` duty shapes) match to 7 sig
     figs; the constant-PQ branch is `conj(S/V)` on both.
  4. **Cross-solver spread on the equivalent linear system** (the sanctioned
     floor-proof, cf. §near-ideal-source): faer (Rust) vs KLU (oracle) = 0.758 V,
     faer vs scipy(equilibrated) = 0.555 V, KLU vs scipy = 0.776 V — a tight
     ~0.5–0.8 V triangle; the Rust↔oracle gap sits *inside* the cross-solver span.
  5. **Residual parity:** ‖Y·V_rust − inj‖∞ = 4.40e-2 vs ‖Y·V_oracle − inj‖∞ =
     4.56e-2 — neither engine's answer is cleaner; both sit at the same κ≈1e12 junk
     floor, so even a perfectly refined Rust V would still miss the oracle's V by
     the oracle's own junk.
  6. The regulator lands the identical tap; disabling it and fixing the tap leaves
     the 0.55 V gap **unchanged** (RegControl exonerated), and tightening
     `tolerance` to 1e-10 leaves it byte-unchanged (not a convergence artifact).
- **Why not banded:** admitting it needs `v_abs ≈ 1 V` (~4e-5 rel at the 46 kV
  buses) — 33× the `large_floating_zeroseq` 3e-2 V and wide enough to mask a real
  substation-transformer bug. The deck's 1 mm busbar lines are a modeling choice
  that manufactures κ≈1e12; documented as a floor, not forced into `solvable_now`.

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
only after `corpus_gate` confirms it holds the tighter floor. Golden tests that
drive a stiff network (`golden_ieee8500`, harmonics/protection/meter scenarios in
`golden_metering_monitors` / the DER/harmonics/protection scenarios) pass `"large"` explicitly.

### wtg3-dynamics (`micro_wtg3_dynamics`): the PLL derivative-gain cancellation floor

The WindGen (WTG3) dynamics decks (`windgen_dyn`, `windgen_dyn_fault`, WP-U1.8).
Proven by decomposition (CLAUDE.md — a floor changes only by proof, never a
sweep; the full leg-by-leg audit lives at the tier's inline comment in
`harness::tol_for`, this is its summary):

- The snapshot operating point that seeds dynamics matches the oracle to the
  solver floor (node V ≤3.7e-9 abs on the 398 V L-N buses, feeder-tight), and
  every WTG3 state variable **not** touched by the phase-locked loop matches to
  ≤2e-7 (Pcmd 4.8e-8, Vref 1.2e-7, Vmag 4.6e-7, WtAct 1.9e-8, thetaPitch
  1.5e-7; Pg/Ps/Pr/s bit-exact).
- Only the three PLL-fed quantities are loose — `dOmg` (9.1e-6), `Pgen`
  (9.7e-6), `Qgen` (3.4e-6): `PllLogic`'s derivative term
  `KpPLL·(Vq−VqOld)/deltSim = 60·Δ/0.001 = 60000·Δ` amplifies the last-ulp
  difference in `Vq` (itself the small imaginary residual ~3.5e-3 of a voltage
  the PLL rotates onto the real axis — a near-cancellation) by 6e4×; the
  amplified `dOmg` then feeds the whole trajectory (the WPG.13/`dSpeed`
  cancellation-floor class).
- Not a divergent state-leak: the gap **decays** as the startup transient
  settles (node V |Δ| 8e-5 @ 1 step → 4.4e-6 @ 20 → 1.6e-6 @ 100). The amplified
  injection current reaches the WindGen *terminal* bus through the small series
  line (`V_term = V_src − I·Z_line`); the far *source* bus stays feeder-tight
  (3.5e-8 rel) because the ideal source buffers it.

Calibration (binding fault case × ~2): `windgen_dyn` wbus |Δ|3.9e-4 V (9.6e-7
rel); `windgen_dyn_fault` (sustained 3φ fault → LVPL/LVQL ride-through) wbus
|Δ|1.23e-3 V (3.4e-6 rel), srcbus 3.5e-8 rel. `v_rel` 8e-6 covers the fault wbus
3.4e-6 rel (×2.3); `i_abs` 1e-4 covers the per-unit small-variable amplification
(`dOmg` 0.08, |Δ| 4.2e-5); `i_rel` 2e-5 covers the ~1e-5-rel amplified
per-unit/ampere currents. `y_rel`/`y_abs` stay tight (the dynamic Norton YPrim is
a deterministic closed-form) — a real WTG3 model bug moves the non-PLL variables
(all ≤2e-6 here) or the assembled Y far past these floors.

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
- **§monitor-f32-floor** (`harness::compare_monitor`, 2026-07-17): the per-sample
  band is `max(i_abs + i_rel·|v|, ulp_f32(v))` — the f32-ULP floor stated in the
  bullet above, now actually implemented. For tiers with `i_rel < 1.19e-7`
  (feeder 1e-7, micro 1e-9) the bare band is *narrower than one ulp of the
  recording format* at rel-dominated magnitudes, so a sub-f64-floor trajectory
  straddling an f32 rounding midpoint failed spuriously. **Proof by
  decomposition** (InductionMachine r4133 twin `controls/fuse/indmach_r4133/
  indmach_dyn.dss`, 5001-step dynamics vs oddie:r4133): at the failing sample
  (f2 `V2`, t=0.35917 s, mid-SLG-fault) the **live f64** |V(B4.2)| was
  8939.842285059083 (Rust) vs 8939.842285310006 (r4133) — **2.8e-11 rel**,
  350× inside the feeder `v_rel` floor — yet the two f64s straddle the f32
  midpoint 8939.84228515625, so the recorded f32s differ by a full ulp
  (9.77e-4). Across all 490,098 samples of the run: 5,622 differ, **99.25%
  by exactly 1 ulp**, none growing in time (diffs *decrease* toward the end
  of the transient), and the same step's f64 node-V/Y/element/property
  surfaces all pass the full feeder floors. A real defect is ≥2 ulps or
  visible in the (unchanged, tight) f64 surfaces.
  Companion term, same commit: **polar ANGLE channels** (`VAngle<n>`/`IAngle<n>`
  only — mode-3 state names like `Theta (deg)` deliberately excluded) get
  `max(band, rad2deg·(i_abs + i_rel·|mag|)/|mag|)` where `mag` is the *same
  sample* of the preceding magnitude channel — the exact angular image of the
  already-accepted magnitude floor (the voltage-scaled power-floor
  construction above). At healthy magnitudes the image is far *tighter* than
  the base band (1.7e-5 ° at 50 A); it opens only where the magnitude carries
  no angular information (indmach_dyn: 0.10–0.24 A residual `I3` during the
  phase-1 fault, dI = 1.3e-7 A cross-solver floor → 3–6e-5 ° swing, 3 samples).
  The magnitude channel itself stays fully banded, so a real current defect
  cannot hide behind its angle.
  **Scope — deliberately global, not per-deck** (T4 audit settlement): the floor
  applies to every `compare_monitor` call, pinned-0.14.5 decks included, because
  the recording format is f32 on *every* engine generation — the pinned oracle's
  own `Monitor.pas:143` declares `MonBuffer: pSingleArray` and each buffered
  sample is an f64→f32 store (`Monitor.pas:1599`); the r4088/r4133 trunks use the
  same single-precision buffer. Scoping the floor to r4133 decks would assert
  that 0.14.5 monitor data carries sub-ulp information, which is false by
  representation. Bounds of the widening where the floor engages (base band
  < 1 ulp ≈ 1.19e-7 rel): feeder tier `i_rel` 1e-7 → ≤19% on rel-dominated
  samples; micro tier (`i_rel` 1e-9, `i_abs` 1e-6) the ulp term dominates for
  |v| ≳ 8.4, where the old band *de facto* demanded bit-identical f32 samples —
  a requirement no f64-correct trajectory can guarantee (any midpoint straddle
  breaks it, proven above at 2.8e-11 f64 rel). And the floor cannot mask any
  live divergence: the full gate was green under the narrower pre-floor bands
  immediately before this change, so no existing comparison sits in the
  newly-opened sub-ulp window; a future defect is ≥2 ulps there or visible in
  the same steps' f64 surfaces (node V / currents / powers / variables), which
  keep full tier floors.
- **§G1.3a — the derived polar channels** (`harness::compare_element_derived`,
  `GOLDEN_REBASE_PLAN.md` WP-G1 G1.3a): `CktElement.CurrentsMagAng`,
  `VoltagesMagAng` and `Residuals` are *renderings* of quantities the gate
  already compares, so **no floor is calibrated for them** — each one is the
  image of an existing, already-calibrated tier band.

  **0. The set they are images OF is a disc, not a rectangle** (written down
  explicitly because the G1.3a F5 measurement raised the question; coordinator
  decision D10 asked for a `√2` correction and the code refutes it). Element
  currents go through `harness::compare_element_channels` →
  `harness::assert_complex_close_c`, whose three operative lines are
  `let diff = ((ar - er).powi(2) + (ai - ei).powi(2)).sqrt();`,
  `let mag = (er * er + ei * ei).sqrt();` and
  `let allowed = abs_floor + rel * mag;` — it bands the **modulus** of the
  complex difference, `|Δz| ≤ abs + rel·|z|`. Node voltages reach the same
  function through `harness::assert_complex_close`
  (`corpus_gate/runner.rs:927,930`). So the admitted error set is the closed
  **disc** `D(z, ρ)` with `ρ = abs + rel·|z|`, and derivations 1, 2 and 4 below
  are images of that disc.
  *Had* the gate banded `re` and `im` separately at `abs + rel·|component|`, the
  admitted set would have been an axis-aligned rectangle whose modulus reaches
  `√2·abs + rel·|z|` at 45° (Minkowski:
  `√((abs+rel|re|)²+(abs+rel|im|)²) ≤ √2·abs + rel·√(re²+im²)`, attained iff
  `|re| = |im|`) — up to `√2` looser than the disc. It does not, so **no `√2`
  enters any band here and no band moves**. Pinned in both directions by
  `harness::derived_polar_floors::the_inherited_current_band_is_a_disc_not_a_rectangle`
  and its rejection leg `…::the_rectangles_diagonal_reach_fails_the_disc_band`
  (feeder tier, 600 A at 45°: disc radius 7e-5 A, the rectangle's diagonal reach
  7.414213562373095e-5 A = 1.0591733660532994× — admitted by a rectangle,
  rejected by the gate).
  The two live angle failures that raised the question are **not** in that gap
  either way (`asymmetric:indmach/indmach_asym.dss` and
  `asymmetric:combo/combo_mesh_asym.dss`, both `kind=micro`, `Transformer.tg`
  `CurrentsMagAng[4]` on the r4133 channel): measured `1.0811686479428317e-7 °`
  against a band of `8.916392577029153e-8 °` (×1.2125628594777123) at
  `|I| = 1.7979012350026755e3 A`, and `1.3473783155859564e-7 °` against
  `9.649498570331597e-8 °` (×1.3963195141855491) at
  `|I| = 1.4616566273058197e3 A`. A rectangle would have inflated those bands by
  only `(√2·abs + rel|z|)/(abs + rel|z|)` = ×1.148044383122301 and
  ×1.1682661821224167 (the micro tier's `abs = 1e-6` is the only term a `√2`
  touches), i.e. **both would still have failed**. They are genuine divergences
  on decks whose `element` ledger entry already excludes the `currents` channel
  they are the polar rendering of — settled by widening those scopes
  (`tests/corpus/ledger.json`), never by a band.

  The five derivations:

  1. **Magnitude is the exact image of that disc.** `| |a| − |b| | ≤
     |a − b|` (reverse triangle inequality), so a complex value already inside
     `abs + rel·|oracle|` is inside the same band in magnitude; the bound is
     attained at `a = z(1 ± ρ/|z|)`, so the magnitude channel admits neither more
     nor less than the disc does. `CurrentsMagAng`
     is the polar rendering of `CktElement.Currents` (`i_rel/i_abs`, gated
     element-by-element by `compare_element_channels`); `VoltagesMagAng` is the
     rendering of `NodeV[NodeRef[·]]` (`v_rel/v_abs`, gated node-by-node in
     `harness::assert_complex_close`,
     `corpus_gate/runner.rs:927-930`). The two evaluations of `|·|` themselves
     differ by at most an ulp each (`num_complex::norm` = hypot vs the naive FPC
     `Cabs`, proven equal on the whole reachable domain by
     `line_constants::tests::naive_modulus_equals_hypot_until_the_square_overflows`),
     i.e. ≤ 2·2.2e-16 rel — six orders under the tightest `rel` in the table
     (micro 1e-9). Nothing widens.
  2. **Angle = the conservative linearization of that disc's angular image.**
     `arg` maps `D(z, ρ)` onto exactly `±asin(ρ/|z|)` radians — the tangent from
     the origin to the error circle, valid while `ρ < |z|` (derivation 3 covers
     the rest). The band `harness::polar_angle_band` emits is that image's
     **linearization**, `rad2deg · allowed_mag/|z|` degrees — the same
     construction as the voltage-scaled power floor and the `compare_monitor`
     angle companion term above. Because `asin(x) ≥ x` the linearization is never
     *looser* than the exact image: the angle channel can only be **stricter**
     than the complex band it renders, never more permissive, so it can produce a
     false failure but never a false pass. The gap is
     `asin(x)/x − 1 = x²/6 + O(x⁴)`: at the magnitudes the corpus actually judges
     it is below one f64 ulp (`x = 1.6841552121877197e-9` on the
     `combo_mesh_asym` sample above → `x²/6 = 4.727297964565105e-19`; the emitted
     `9.649498570331597e-8 °` and the exact `9.649498570331596e-8 °` differ only by
     the ulp of the multiplication order), while at the widest band the live
     corpus has ever emitted (`37.0083678180735 °`, `x = 0.6459178692144925`) the
     exact image would be `40.23452908031853 °` — the band is 8.7 % tighter there,
     with no consequence: the whole `midi_fuse` residual-angle channel measures
     ≤ 3.9428730418000316e-7 of its band. Pinned by
     `harness::derived_polar_floors::the_angle_band_is_the_conservative_linearization_of_its_exact_image`.
     If a future sample ever fails **only** inside that gap, the fix is to emit
     the exact `asin` image — a re-derivation, not a widened tolerance.
     The band uses the **full-precision**
     `57.29577951308232`: a tolerance is not a printed value, so the truncated
     `57.29577951` of `CDANG` (r4133 `Shared/Ucomplex.pas:118`) belongs only
     inside the kernel that renders the angle. At healthy magnitudes the image is
     far *tighter* than any base band — 6.57e-6 ° at 683 A on the feeder tier,
     and the largest band the whole IEEE13 `VoltagesMagAng` channel ever emits is
     7.83e-7 °.
  3. **The near-zero mask, and why the band cannot exceed 57.3 °.** Where
     `|oracle mag| ≤ allowed_mag` the phasor is indistinguishable from zero at
     the accepted precision and its argument carries **no** information; the
     angle is then skipped while the magnitude — never masked — keeps the channel
     two-sided. Because the mask fires exactly where the image would reach
     `rad2deg · 1`, the emitted angle band can never exceed `57.29577951308232 °`
     by construction (pinned by
     `harness::derived_polar_floors::the_angle_band_never_exceeds_one_radian_in_degrees`;
     largest band measured on live data: 37.0083678180735 ° on `midi_fuse`
     residuals, 23.9211430402150 ° on IEEE13 residuals — residuals are a
     near-cancellation by construction, so their band is the widest). The mask is
     load-bearing, not cosmetic: the worst *masked* sample disagrees by
     165.564025012130 ° at |I| = 5.550898829703193e-12 A (`midi_fuse`
     `CurrentsMagAng`) and 179.999999990328 ° at |res| = 1.1368683772161605e-13 A
     — angles of numerical zero. Every masked `VoltagesMagAng` sample is an
     exactly-grounded conductor (`NodeRef = 0`, `NodeV[0] = 0`), where both
     engines report 0 ° and the mask therefore hides nothing (measured: 29 of
     134 conductors on IEEE13, 42 of 275 on `midi_fuse`, worst masked Δ = 0 ° in
     both). For comparison the `CurrentsMagAng` mask fires on 7 of 134 / 10 of
     275 conductors and the `Residuals` mask on 27 of 58 / 37 of 106 terminals.
  4. **`Residuals` = a sum of conductor currents, so the band is the sum of their
     bands.** `|δ(Σ_c I_c)| ≤ Σ_c (i_abs + i_rel·|I_c|) = nconds·i_abs +
     i_rel·Σ_c|I_c|` (`harness::residual_band`) — the Minkowski sum of the
     conductors' discs is the disc of the summed radius, so the bound is exact
     rather than slack (attained when the conductor errors are collinear), and it
     is exactly the derivation
     `compare_element_channels` already uses for `Get_Losses` (`losses = Σ_k
     S_k`), transplanted from powers to currents. No new tolerance class. It
     matters because the residual is a near-cancellation by construction (≈0 on a
     balanced terminal): IEEE13 `Line.671680` terminal 1 reads 2.83e-5 A against a
     band of 3.0e-5 A, i.e. entirely inside the absolute floor, while
     `Line.650632` carries ~144 A on both terminals. Honesty note: on today's
     corpus the plain per-sample band `i_abs + i_rel·|res|` would also have
     passed, so this derivation is not fitted to a failure — it is written down so
     nobody later "tightens" it into a false failure on a balanced terminal.
  5. **Angle differences are taken on the circle.** `CDANG` returns
     `(−180, 180]`, so a phasor astride the negative real axis reads
     `+179.99999999032846 °` on one engine and `−179.99999999032846 °` on the
     other for an imaginary part of ±1e-18: the raw difference is
     `359.9999999806569 °` for a physical difference of
     `1.9343133317306638e-08 °`. `harness::wrapped_deg` folds the difference into
     `[−180, 180]` first. This is not a relaxation — it is what "angle" means — and
     it is bounded: a genuine sign flip still measures a full `180 °`, which is
     above the 57.3 ° ceiling of derivation 3 and therefore fails in every case
     (rejection leg:
     `harness::derived_polar_floors::a_sign_flipped_angle_still_fails_the_band`).

  **Measured headroom** (port vs the pinned `capi_v0145` oracle, `feeder` tier,
  plain snapshot, worst |Δ|/band over every conductor and terminal of every
  enabled element — `< 1` means inside the band):

  | deck | cma.mag | cma.ang | vma.mag | vma.ang | res.mag | res.ang |
  |---|---|---|---|---|---|---|
  | IEEE13Nodeckt (38 elements) | 0.2509 | 0.04539 | 0.04588 | 0.05124 | 0.01393 | 0.01032 |
  | controls/fuse/midi_fuse (66) | 9.006e-6 | 1.012e-5 | 3.037e-4 | 3.313e-4 | 1.018e-5 | 3.943e-7 |

  The cross-*oracle* twin of the same measurement (capi_v0145 vs r4133, same
  decks, same request) sits at the same scale — worst ratio 0.2509 on IEEE13
  `Line.671692[1]`, `|Δ| = 4.228323e-06 A` against a band of `1.685074e-05 A` —
  so the band is neither vacuous nor exceeded on either channel.

  *Ledger side (F6′, 2026-09-04 — no band moved).* The full-corpus measurement
  put **12** of the 442 gated cases over one of these bands, every one of them on
  a deck whose committed `element` ledger scope already excludes the rectangular
  channel the polar one renders; the scopes were widened per sub-channel, iterated
  to a fixpoint (`tests/corpus/ledger.json`,
  `measured.g13a_polar_first_failure`), plus one genuinely new capi-only entry
  (`capi-capcontrol-time-bus-is-the-capacitors`, a different bus — not a floor
  question). Two consequences for the derivations above. (1) The `envelope_element`
  handler applies **exactly** these two floors, so a ledger envelope is measured on
  the comparator's own scale. (2) A **masked** angle — magnitude at or under its
  band, where derivation 3 says the angle carries no information and
  `polar_close` skips it — is not envelope-checked either: measured on
  `r4133-combomidi-injection-ulp`, a numerically-zero conductor read
  `Transformer.t8 cma[9].ang` **139.4 °** from the oracle, so the only envelope
  that could admit it is ±180 °, i.e. one that bounds nothing. The magnitude is
  never skipped, so the sample stays two-sided and the entry can still go stale
  (`ledger::a_masked_polar_angle_is_not_envelope_checked` and its rejecting twin
  `::an_unmasked_polar_angle_still_hits_the_envelope`).

- **§G1.3d(i) — the per-element discrete extras** (`harness::compare_element_extras`,
  `GOLDEN_REBASE_PLAN.md` WP-G1 G1.3d): `CktElement.NumTerminals`,
  `NumConductors`, `NumPhases`, `NodeOrder` and `EnergyMeter` are **discrete** —
  three counts, a vector of bus-local node numbers and a name — so they are
  compared **exactly**, with no `Tolerances` argument, no tier lookup and no
  band of any kind. **No floor is introduced, and no existing floor moves**:
  this sub-step neither reads nor writes `Tolerances`/`tol_for`. Two
  consequences worth stating so a later reader does not look for a band that
  does not exist. (1) The surface takes no ledger sub-channel either
  (`SUBCHANNEL_FIELDS` is unchanged), so a divergence here cannot be masked by a
  committed `element` scope — it is a gate red and a STOP, which is the whole
  point of gating discrete state. (2) The one normalization it does apply is a
  **capture-boundary spelling fold, not a tolerance**: "no meter" arrives as
  `''` from capi (`Result := NIL`, `CAPI/CAPI_CktElement.pas:672-687`) and as
  `'0'` from r4133 (the `CktElementS` pre-`case` default,
  `DDLL/DCktElement.pas:421`), and each folds **on its own channel only** before
  the exact compare — value-preserving in the `PROPS_NORM_R4133` sense. It is not
  self-detecting: on r4133 a meter literally named `0` reds when the port HAS the
  name (`element_extras_pins::a_meter_named_zero_reds_instead_of_passing`) but
  passes when the port LOST it
  (`element_extras_pins::the_r4133_zero_sentinel_is_undecidable_and_the_census_is_the_guard`),
  so the corpus census `extras_population::no_corpus_energymeter_is_named_zero` —
  not the fold — is what keeps that unreachable (G1.3d(i) audit settlement,
  2026-09-05).

- **§G1.3d(ii) — `PhaseLosses`** (`harness::compare_element_phase_losses`,
  `GOLDEN_REBASE_PLAN.md` WP-G1 G1.3d): the one numeric field of the surface, and
  the **only** new band in it. **No existing floor moves and no new tolerance
  class is introduced** — the band is derived from `assert_power_close`'s
  per-conductor policy, which is the floor the gate already applies to every
  `Powers` cell.

  1. **The derivation.** r4133 `Common/CktElement.pas:1093-1112` forms
     `PhaseLosses[i] = Σ_{j=0}^{nterms-1} V_k·conj(I_k)` with `k = j·NConds + i`,
     i.e. one phase's entry is the sum of the *same* per-conductor products the
     `powers` channel bands one at a time. The admitted error of a sum is the sum
     of the admitted errors, so

         allowed_kW(i) = Σ_j ( i_abs·max(1, |V_kj|) + i_rel·|S_kj| ),
         |V_kj| = |P_kW[k]| / |I_A[k]|   (assert_power_close's own recovery)

     — `harness::phase_loss_band`. This is the identical construction
     `compare_element_channels` already uses for `Get_Losses` (`losses = Σ_k S_k`
     over **all** conductors) and `residual_band` uses for a terminal's current
     sum (§G1.3a derivation 4), restricted to the conductors of one phase. It is
     therefore **strictly tighter** than the whole-element `Get_Losses` band it is
     a subset of: measured on the `phase_loss_bands::line_pair` fixture the three
     phase bands are `0.00031648320000000007`, `0.00030929039999999996`,
     `0.00031360607999999997` kW, summing to `0.00093937968` = the `Get_Losses`
     band exactly when `NConds == NPhases`, while adding a neutral conductor moves
     the whole-element band to `0.0009593886185452729` and leaves the phase sum
     unchanged.
  2. **Why the rejection leg is expressed in bands, not in "1e-6 rel".** A phase
     loss is a near-cancellation of two large summands (~863 kVA each on that
     fixture against a 1.25 kW answer), so the band is `2.53e-4` *relative to the
     answer* while being tight against the summands that produce it. A relative
     mutation smaller than that proves nothing: the pins therefore drive the
     comparator at 0.5 band (passes) and 1.5 bands (fails)
     (`harness::phase_loss_bands::an_error_inside_the_band_passes_and_one_outside_it_fails`),
     and the live non-vacuity demo used a 1e-2 relative mutation, which cleared
     the band by 4–6 orders on every gated case.
  3. **The oracle-side identity that makes it a derivation, not a guess.**
     Measured on the real capture path (both transports, three decks): for every
     element with `NConds == NPhases`, `Σ_i pl_kw[i] == loss_w[0]·1e-3` and
     `Σ_i pl_kvar[i] == loss_w[1]·1e-3` to better than `1e-9` relative — the
     oracle's own `PhaseLosses` really is its `Losses` bucketed by phase, so
     inheriting the `Losses` policy is inheritance, not analogy. The port side of
     the same identity is pinned in-engine
     (`exec::tests::element_extras::phase_losses_are_watts_and_sum_to_get_losses`,
     banded at `1e-13·Σ_k |V_k·conj(I_k)|` — the reassociation bound, ~75× the
     `n·ε·Σ|terms|` estimate and ~12 orders below the smallest real disagreement;
     measured gap `7.275958e-12` W).
  4. **The `Get_Losses` trust guard is deliberately NOT copied.** The
     whole-element band carries an oracle-self-consistency escape for a *measured*
     capi015 daily-freeze quirk on a rev this gate does not run
     (`docs/upgrade/DIVERGENCES.md` §"capi015 daily `CktElement.Losses`
     staleness"). An untriggered twin
     here could only mask, so `compare_element_phase_losses` has none; the
     identity is asserted positively instead (3 above).
  5. **Cross-channel headroom** (capi_v0145 vs r4133 on the same request, three
     control decks): the worst absolute gap is `1.672560756560415e-09` kW on
     `vsource.source.pl_kw[2]` of `Test/indmachtest/Master.DSS` (capi
     `-8039.588751957046`, r4133 `-8039.5887519553735`, `2.08e-13` relative), and
     the worst *relative* gap `1.4966018598108593e-11` on
     `vsource.source.pl_kvar[1]` of `controls/relay/relay_oc_sym.dss` — orders
     under the band on both channels, so it is neither vacuous nor exceeded by the
     transports themselves.
  6. **Where the band is deliberately not consulted at all.** On the two `newton*`
     decks `PhaseLosses` joins `LANE_SKIP_ELEM_POWERS` (both lanes) for the same
     stale-`Iterminal` reason as `Powers`/`Losses`; that is a lane exclusion, not a
     floor, and it was measured first — 55.5× / 34.4× the band on `Vsource.source`
     phase 0. Ten committed `element` ledger scopes were widened onto the new
     `phase_losses` sub-channel, again by measurement; a ledger envelope there is
     evaluated with this same band, so no scope can admit more than the floor
     plus its own pinned envelope.
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
  `corpus_gate.rs` gates the engine physics (1e-8 rel on the live complex
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
  or MW/Mvar); the engine V/I/P is pinned to 1e-8 by `corpus_gate`.
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
  underlying V/I are pinned to 1e-8 by `corpus_gate`. Column structure: the
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
  - **Excluded `Iresidual` cells in `SeqCurrents`** (`seq_currents.rs`): the
    captured oracle sums the *terminal-1* conductors for **every** terminal row
    (Pascal indexes `cBuffer^[i]`, not `cBuffer^[(j-1)*Ncond+i]` — an upstream
    bug both gating engines share). Since `GOLDEN_REBASE_PLAN.md` G2.2a neither
    lane reproduces it: both sum the row's own terminal, which moves only the
    `Terminal >= 2` cells. Those cells alone are excluded, in both lanes
    (`GateSpec::ColAbove(1, 1.5)`) — the terminal-1 cells, where the two
    readings coincide, stay oracle-compared — and the excluded ones are pinned
    by `export_seqcurrents_iresidual_sums_the_rows_own_terminal`, which derives
    them from `Export Currents`' own `Iresid_j` column. This is an exclusion,
    never a loosened tolerance: the column keeps its `abs = 1e-8`.

- **WP8.2 sub-step 2c — the per-terminal/per-conductor element exports**
  (`Currents`/`ElemCurrents`/`ElemVoltages`/`ElemPowers`/`NodeOrder`/`Taps` on
  solved IEEE13). Report-layout gate again; the engine V/I are pinned to 1e-8 by
  `corpus_gate`. These are magnitude (`%10.6g`, 6 sig) + angle (`%8.2f`, 2 dec)
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
    (`-612.729`). Not a compat row at all — it reproduces the oracle exactly; the note
    records *why* the order is inverted from the Pascal source text.
  - **`Taps`** is exact (`rel = 0, abs = 0`): the tap value is the discrete
    `mid + position·increment`, so both engines print the identical value once they
    converge to the same integer tap position (pinned by `corpus_gate` + the
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
  entry-by-entry by `corpus_gate` / the checkpoint gate / `fault_study.rs`.
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

## `Save circuit` round-trip floor — IEEE-8500 (`save_roundtrip.rs`)

`save_roundtrip.rs` re-solves an emitted `Save circuit` deck on our own engine and
pins the recompiled node voltages to the pre-save solution. Four feeders (IEEE
13/34/37/123) round-trip to **1e-6 rel**. IEEE-8500 does **not**, and this is an
inherent property of OpenDSS `Save circuit`, not the port: `Save` re-emits derived
quantities (the substation reactor `X`, every `%g`-rendered parameter) at 15
significant digits; the sub-ulp re-parse perturbation is amplified by the ~thousand
`Model=1`/`Vminpu=0.88` constant-Z service loads into a worst ~2e-4 rel shift at the
deepest 0.208 kV secondaries. So `save_roundtrip_ieee8500` uses `IEEE8500_SAVE_VTOL
= 3e-4` (~1.5× the observed worst) for **node voltages only**; the discrete control
state (12 RegControl tap numbers + 10 capacitor bank states) and the warm-re-solve
iteration count are asserted **exactly** on every deck including 8500.

**Proven a floor (not a port slack) by the pinned oracle itself.** The oracle's own
`Save → recompile → resolve` of this deck reproduces the identical worst node and
pre/post power — the floor is a property of the writer/re-parse, not of faer vs KLU.
Reproducible probe: `python tools/golden/probe_save_roundtrip_8500.py` (pinned
dss-python 0.15.7 / dss_capi 0.14.5, on the vendored corpus bytes the Rust test
round-trips). Measured 2026-07-11:

| quantity | oracle Save round-trip |
|---|---|
| worst node | `SX3312692A.1` |
| worst rel voltage shift | **2.022253e-4** (< the 3e-4 band) |
| nodes over 1e-6 rel | 8354 / 8531 |
| iterations pre/post | 2 / 2 (exact) |
| total power pre (kW) | −11983.486783 |
| total power post (kW) | −11983.420712 |
| ΔP (kW) | +0.066071 |

Because the whole-feeder shift is a smooth continuous-parameter re-parse artifact
with **no discrete-state signature**, the exactly-pinned taps/banks + exact
iteration count are what keep the gate strong: a real regulator/cap/element
regression moves a tap, a bank, or the profile by far more than 3e-4 and fails,
while the 3e-4 band absorbs only the proven, oracle-reproduced Save-precision floor.
The pre-save solve is *independently* oracle-pinned by `corpus_gate` (8500-Node
`Master.dss` is in `solvable_now` at the tight `large` floors, ~300× under 3e-4), so
the operating point itself is gated far tighter than this round-trip band.

## Live corpus gate (`corpus_gate.rs`)

Reuses the same comparators and classes verbatim. Differences from the checkpoint
goldens: the **full** assembled Y is compared every case (nothing is stored, so
size is irrelevant — checkpoints store only a fingerprint for large feeders); the
Rust/oracle element name sets must be identical (**2026-09-04, G1.9**: the
`AllElementLosses` arm additionally asserts the two name vectors match
*element by element in creation order* — see §"G1.9 circuit aggregates + solution
scalars"); monitors/meters are compared live
only for cases that sample them in deterministic modes (`check_meters_monitors` in
`solvable_now.json`) — an unsampled monitor returns a phantom channel from the
pinned oracle, an artifact, not an engine gap.

## `PDElements` walk — exact, and why it earns no floor (G1.6b, 2026-09-04)

`harness::compare_pd_elements` (`crates/dss-core/tests/harness/mod.rs:7595`)
compares all fourteen fields of the per-PD-element walk with **`rel = abs = 0`**
and takes no `Tolerances` argument at all. That is a derivation, not an
optimism: on every gated case today each compared value is one of

* a **class default** the constructor stores verbatim (r4133 line numbers) —
  `Capacitor` / `Reactor` `FaultRate` 0.0005, `PctPerm` 100.0, `HrsToRepair` 3.0
  (`Capacitor.pas:555-557`, `Reactor.pas:601-603`), `Line` 0.1 / 20.0 / 3.0
  (`Line.pas:839-841`), `Transformer` 0.007 / 0.0 / 0.0 (`Transformer.pas:959`
  sets only `FaultRate`; the other two keep Pascal's zero-initialized 0.0);
* a **deck literal** parsed independently by both engines — FPC `Val` and Rust
  `str::parse::<f64>` are both correctly-rounded decimal→binary64, so the two
  bit patterns are identical, not merely close;
* an **untouched `0.0`** (the four `RelCalc`-dependent fields, below);
* or a **discrete** integer / bool / name string.

**No arithmetic is performed on either side of this surface**, so there is no
rounding to absorb and any difference at all is a bug, not a floor. The one
divergence the corpus does measure is not numeric drift but an uninitialized read
in both oracles, which is excluded field-by-field in `PD_SKIP_FIELDS`
(`crates/dss-core/tests/harness/mod.rs:7409`) and pinned — an envelope over a
value that changes every process would not be a fact. See TESTING.md
§"The `PDElements` walk".

**Standing obligation for G1.6(i) — discharged (micro-part F2s, 2026-09-04).**
G1.6(i) drives the executive `RelCalc`, so `Lambda`, `AccumulatedL` and
`TotalMiles` are live accumulated sums over the zone walk
(`PDElement.pas:106-110`) and summation order is observable. **All three stay in
the exact set**, and the re-derivation is a measurement, not an argument. The
live gate first reported twelve `PDElements` cells (`accumulated_l`,
`total_miles`) as 1-ULP Rust↔oracle gaps on BOTH channels. They are not gaps:
the raw JSON number token each oracle puts on the wire was read for every one of
them — and for the three reliability cells below — and **15/15 round-trip to the
port's own f64 bit for bit**, on the capi (`tools/oracle/oracle_server.py`) and
r4133 (`epri-worker`) transports alike. What lost the last bit was the gate's own
decoder: `serde_json` without the `float_roundtrip` feature decodes a
17-significant-digit token as `significand as f64` followed by one divide by a
power of ten — two roundings — landing 1 ULP away (measured directly against
`str::parse`: 9 of 9 tokens misparsed, each to exactly the value the gate printed
as `oracle`). That is the defect coordinator decision **D11** fixes
workspace-wide (`serde_json` + `float_roundtrip`); **D18** landed the same
hunk on this branch, so the gate decodes every oracle float exactly. With the
feature on, the five reliability-flagged cases are green in both lanes
(measured). `SectionID` is discrete and untouched. Both numbers are pinned by
`exec::tests::reliability::reliability_accumulators_are_correctly_rounded_f64_sums`
(`0.24000000000000002` vs the decoded `0.24`; `0.9469696969696969` vs
`0.9469696969696968`). **No floor is added to this surface**, and
`compare_pd_elements` keeps its no-`Tolerances` signature.

## The `Meters` reliability surface (G1.6(i)) — exact, with one energy-tier exception

`harness::compare_reliability` compares the `RelCalc` payload with
**`rel = abs = 0`** on every reliability value, discrete and continuous alike.
The justification is measured twice over:

* the two independent oracle engines return **bit-identical** doubles for every
  reliability number on `modes:time/midi_duty_ctrl.dss` (`SAIFI
  0.05600000000000001`, `SAIDI 0.16799999999999998`, `CustInterrupts
  0.11200000000000002`, `SumBranchFltRates 0.0031360000000000008`,
  `AvgRepairTime 2.999999999999999`, `FaultRateXRepairHrs 0.009408`) — the
  arithmetic is sums over integer customer counts and deck literals in a fixed
  zone-walk order, and the walk order itself was measured bit-identical to both
  oracles on all five flagged cases;
* every cell where the port looked 1 ULP off was traced to the wire and found
  bit-identical (the `PDElements` paragraph above; the reliability cells are
  `cust_interrupts`, `saifikw` and a section's `sum_branch_flt_rates`).

So a Rust gap on this surface is an **order bug**, never a floor.

**The one exception is `Meters.Totals`** (coordinator decision **D17a**), which
is compared at the existing energy-accumulation tier `tol.energy_rel` /
`tol.energy_abs` (both `1e-4`). It is not a reliability quantity at all:
`TotalizeMeters` (r4133 `Common/Circuit.pas:2520-2538`, capi `:2347-2360`) is
`Σ_meters Registers[i] · TotalsMask[i]` over the circuit's meter list, i.e. a
masked sum of the very energy registers `compare_meter` already compares at that
tier (PORTING_PLAN §4). A sum of quantities that are not exact cannot itself be
exact, so it **inherits** the summands' floor — this is an existing band applied
to the same physical quantity, not a new one. Measured on
`controls:energymeter/midi_energymeter.dss` (the only live two-meter deck): 64
non-zero slots over the two channels, worst relative deviation
**7.450981e-10** (slot 24, r4133: `3263.4261264650568` vs `3263.4261288966295`),
five orders of magnitude inside the tier. The structural identity behind the
inheritance — masked sum, every meter, creation order — is pinned by
`exec::tests::reliability::meter_totals_is_the_masked_register_sum`.

**What that tier does NOT guard, stated plainly** (G1.6(i) audit settlement,
finding AT-2): `1e-4` rel / `1e-4` abs against a measured spread of
`7.450981e-10` means a `Totals` regression below `1e-4` relative — and, below
`|value| ≈ 0.111`, below `1e-4` absolute — passes here. The 67 slots are
therefore **not** covered by this surface's exactness headline; the real guard
on their shape is the in-engine identity pin named above (masked sum, every
meter, creation order, all 67 slots asserted against the registers), and each
summand is separately compared by `compare_meter` at the same tier. Tightening
the band would be a change to the ENERGY tier itself — a PORTING_PLAN §4
decision, not a reliability one — so it is not made here.

### The unguarded `AverageRepairTime` division — NaN and ±inf agree, NaN vs a number does not

`AverageRepairTime := SumFltRatesXRepairHrs / SumBranchFltRates` is written
without a zero test on **all three** engines: r4133
`Version8/Source/Meters/EnergyMeter.pas:2563`, capi 0.14.5
`src/Meters/EnergyMeter.pas:2518`, port `average_repair_time`
(`crates/dss-core/src/solution/meters/reliability.rs:293`). A feeder section whose
branches all carry `faultrate=0` therefore evaluates `0.0 / 0.0` and yields
`NaN` — identically everywhere, since IEEE-754 fixes that result and the port
performs the same single division on the same two f64 accumulators.

`harness::rel_num_eq` treats two `NaN`s as **agreement** and `±inf` as agreement
only with the same sign; `NaN` against any finite number is a **failure**, as is
`+inf` against `-inf`. That is not a tolerance and not a mask: it is the only
equality relation under which "both engines computed `0/0` here" is expressible,
and it is strictly narrower than dropping the cell (which is what a skip row
would do). The regime does not occur in today's flagged population — every
section measured has `SumBranchFltRates > 0` — so the arm is proved by a harness
unit test that drives the comparator with `NaN` on both sides (passes) and `NaN`
against `0.0` (fails):
`harness::reliability_tests::nan_agrees_with_nan_and_never_with_a_number`. The
same rule is applied to `Meters.Totals` on top of its energy band.

### `Meters.CalcCurrent` / `Meters.AllocFactors` — the current tier, and where they are not defined at all

These two per-phase arrays are the only cells of this surface that are **not**
deck-literal arithmetic, so they are the only ones that do not ride the
exactness rule above. `TMeterElement.CalcAllocationFactors` (r4133
`Version8/Source/Meters/MeterElement.pas:54-72`) writes

```
CalculatedCurrent[i] := <the metered element's terminal current>
PhsAllocationFactor[i] := SensorCurrent[i] / Cabs(CalculatedCurrent[i])   { else 1.0 }
```

so `CalcCurrent` is literally `|GetCurrents|` of the metered element — the same
quantity `harness::compare_element` already gates — and `AllocFactors` is that
quantity in a denominator.

**`calc_current` takes the current tier verbatim**: `i_abs + i_rel·|oracle|`
(`harness::reliability_array_band`). No new band; the same numbers
`compare_element` uses.

**`alloc_factors` takes the *image* of that band under `f = S / |I|`**, not a
band of its own. With `S` fixed (a deck literal, parsed identically on both
sides) and `|I|` admitted to move by `δ = i_abs + i_rel·|I|`, the induced
first-order motion of `f` is

```
|Δf| = |f| · δ / |I| = |f| · (i_rel + i_abs / |I|)
```

which is what the harness computes, with `|I| = max(|I_port|, |I_oracle|)` as
the denominator (the larger of the two, so a near-zero port current cannot
inflate the band) and an **exact** compare when both currents are zero — which
is exactly the Pascal's `ELSE PhsAllocationFactor^[i] := 1.0` branch
(`MeterElement.pas:68`), where no division happened on either side.

**The denominator is band-limited from below, and that bound is enforced.**
The expression above is a band only while `|I|` is distinguishable from zero:
once `|I| < i_abs` the term `i_abs/|I|` exceeds 1 and the "band" admits the
whole value, so the cell would stop being compared with no counter and no
message. `harness::reliability_array_band` therefore returns **no band** for
`0 < |I| < i_abs` (and for a non-finite current), and `compare_reliability`
turns that into a **loud triage failure** rather than a pass; `|I| == 0` on
both sides stays the exact arm. This is the same lower bound the
band-limited-denominator rule for the `SeqCurrents %I…` columns calls
load-bearing (`0 < |I1| < 1e-6 A`, `ColTol::gate` above) — a ratio inherits its
numerator's band divided by a denominator that is never allowed to vanish, and
before the G1.6(i) audit settlement (finding A/4) only the sentence said so:
the code admitted `|f|·1e3` at `|I| = 1e-9 A`. The three regimes are unit-tested
by `harness::reliability_tests::the_alloc_factors_band_is_band_limited_from_below`.

Measured on `controls:energymeter/midi_relcalc.dss`, the only corpus deck that
runs `AllocateLoads` (G1.6(i) part F3; port, capi 0.14.5 and EPRI r4133 all read
through the transports the live gate uses):

| k | port | capi 0.14.5 | EPRI r4133 | capi↔r4133 rel | port↔worst-oracle rel |
|---|---|---|---|---|---|
| `calc_current[0]` | `115.69585353499207` | `115.69585353499478` | `115.69585353499097` | 3.29e-14 | 2.34e-14 (capi) |
| `calc_current[1]` | `84.8661220964024` | `84.86612209639719` | `84.86612209639911` | 2.26e-14 | **6.14e-14** (capi) |
| `calc_current[2]` | `85.38036092989663` | `85.38036092989765` | `85.38036092989825` | 6.99e-15 | 1.90e-14 (r4133) |
| `alloc_factors[0]` | `1.0372022534386347` | `1.0372022534386103` | `1.0372022534386445` | 3.30e-14 | 2.35e-14 (capi) |
| `alloc_factors[1]` | `1.1783264927129167` | `1.178326492712989` | `1.1783264927129624` | 2.26e-14 | **6.14e-14** (capi) |
| `alloc_factors[2]` | `1.0541065769667621` | `1.0541065769667495` | `1.0541065769667421` | 6.95e-15 | 1.90e-14 (r4133) |

The two **independent oracle engines**, both KLU-based, already disagree with
each other by up to `3.30e-14` relative here while agreeing bit-for-bit on every
scalar, every section field and all 67 `Meters.Totals` slots of the same
payload — that is the proof these two arrays alone are solve-derived. The port's
worst gap against either of them is `6.14e-14` = `1.9×` the spread the two
oracles leave between themselves, i.e. faer-vs-KLU on the same footing as
KLU-vs-KLU. The micro tier (`i_rel = 1e-9`, `i_abs = 1e-6`) leaves four to five
orders of headroom; the concrete bands the shipped comparator computes on this
deck are `1.116e-6` for `calc_current[0]` (`1e-6 + 1e-9·115.7`) and `1.000e-8`
for `alloc_factors[0]` (`1.0372·(1e-9 + 1e-6/115.7)`) — the propagated band is
~100× *tighter* than applying `i_abs + i_rel·|f|` to a dimensionless ~1.0
quantity would have been. Both are shown non-vacuous by perturbing the port's
arrays by 1e-6 relative in a scratch copy, which reds the case on both channels
(G1.6(i) part F3), and the identity itself is pinned with all three engines'
numbers by
`tests/reliability_pins.rs::meter_allocation_factors_are_the_peak_current_over_the_metered_current`.

**Everywhere else the two fields are excluded, and that is not a floor.** Until a
deck runs the executive `AllocateLoads`, both oracles read the arrays out of
**uninitialized memory**: `TMeterElement.AllocateSensorArrays` `ReallocMem`s
`CalculatedCurrent` and `PhsAllocationFactor` without zeroing them (r4133
`Version8/Source/Meters/MeterElement.pas:45-52`), only
`CalcAllocationFactors` (`:54-72`) ever writes them, and its sole driver is
`TExecHelper.DoAllocateLoadsCmd` (r4133
`Version8/Source/Executive/ExecHelper.pas:2624-2683`). Measured on
`controls:combo/combo_protection.dss` with three fresh `epri-worker` processes
(G1.6(i) part R): `Meters.AllocFactors` =
`[2.806806272625585e-309, 2.121995791e-314, 2.37e-322]` in run 1 and
`[…, …, 2.4e-322]` in runs 2-3 — denormal garbage whose third slot **changes
between processes**. A value that changes every run cannot be enveloped, so
there is no band to derive and no `tests/corpus/ledger.json` row to write
(coordinator decision D4, the `PD_SKIP_FIELDS` precedent): the cells are dropped
field-by-field by `harness::RELIABILITY_SKIP_FIELDS`, whose predicate is the
**port's own** regime (both arrays still exactly zero) so it evaporates on the
one deck that allocates, and the port's side is pinned by
`tests/reliability_pins.rs::meter_alloc_factors_are_zero_until_allocateloads_runs`.

### Two notes that keep this section true

**The zone lists are compared in order, not as sets.** The exactness argument
above rests on both engines accumulating in the same zone-walk order, so since
G1.6(i) part F4 `compare_reliability` asserts the sequence of
`AllBranchesInZone` / `AllEndElements` / `ZonePCE` element by element and not
merely their membership (`compare_meter`'s deliberate order-independence is
untouched — this surface layers the stronger contract on top). Measured
bit-identical, port vs both oracles, on every flagged case.

**A corpus deck that makes `SAIFIkW` solve-derived would break this section.**
`SAIFIkW = Σ kWBase·RelWeighting·Bus_Num_Interrupt / Σ kWBase` (r4133
`Version8/Source/Meters/EnergyMeter.pas:2605,2632`), so it is deck-literal
arithmetic only while the zone's loads carry a literal `kW`. An early draft of
`midi_relcalc.dss` used `xfkva=`/`allocationfactor=` loads, whose `kWBase` is
recomputed from the solve: `SAIFIkW` then measured `0.18812283916834857` on capi
against `0.18812283916834877` on r4133 (1.06e-15 relative) while every other
scalar stayed bit-identical. The shipped deck uses kW-spec loads for exactly
this reason (recorded in its header and manifest note); a future reliability
deck must do the same, or `SAIFIkW` leaves the exact set and needs its own
derivation here.

That choice costs no coverage of `AllocateLoads` itself: the kW-rewriting
branch of `Set_AllocationFactor` — the one this deck deliberately avoids — is
oracle-pinned in-engine by `exec::tests::allocation::allocateloads_kwh_spec_loads`
(kWh/`cfactor` spec) and `::allocateloads_single_phase_per_phase_factor`
(`xfkva`/`allocationfactor` spec), both asserting the rewritten `Loads.kW`,
plus the data-driven `tests/golden_allocation.rs`. What the corpus deck adds is
the LIVE half — `Meters.CalcCurrent`/`AllocFactors` defined on both oracle
channels — and that is all it is asked to add (G1.6(i) audit settlement,
finding AT-5).

## Bus voltage surface (GOLDEN_REBASE G1.4a, `harness::compare_bus`)

`compare_bus` / `compare_all_bus_vmag_pu` gate the bus flavours of the solved
node voltages — `Bus.puVoltages`, `Bus.VMagAngle`, `Bus.puVMagAngle` and
`Circuit.AllBusVmagPu`. **The surface adds no tolerance constant.** Every band is
the *exact image* of the already-calibrated node-voltage band
`eps = v_abs + v_rel*|V|` (the `assert_complex_close` in `corpus_gate/runner.rs`,
over the very same `Solution.NodeV` these quantities are read from) under a
transformation both engines run identically. Derivations, in the order the
comparator applies them:

1. **`kv_base`, `nodes`, the bus-name sequence — exact, no band.** `kVBase` is
   `NearestBasekV/SQRT3` off the deck's own legal-base list (`Solution.pas:1103`
   == r4133 `:2541`), `Nodes` and `BusList` are discrete. A disagreement is a
   finding, not a floor. (Lane note: the base *search* scale is a Stage-F row,
   `compat::kv_base_search_scale` — truncated `0.001732` in the parity lane vs
   `SQRT3/1000` in the default lane — which can only select a different legal
   base when the estimate lands within 2.93e-5 of a tie between two adjacent
   bases; a lane-dependent failure here is that knife edge, never a band to
   widen.)
2. **`puVoltages`, `puVMagAngle`.mag, `AllBusVmagPu`** — the quantity is
   `NodeV / BaseFactor` with `BaseFactor = 1000*kVBase` (or `1.0`, 11 480 corpus
   buses) an *exact, engine-identical* constant by (1), so
   `|dV|/BF <= v_abs/BF + v_rel*|V/BF|`:

   > `allowed = v_abs / BaseFactor + v_rel * |expected|`

   The scaled absolute term is the point: the raw `v_abs` applied to a per-unit
   number would be an 8e-6 pu band on a 0.12 kV bus and 7e-12 pu on a 138 kV
   bus, while the image means the same thing everywhere. `AllBusVmagPu` takes
   the `BaseFactor` of the bus each entry belongs to (convention 2 — bus-list
   order x internal node index), rebuilt from the port's own bus list, whose
   per-bus `kv_base` (1) pins exactly.
3. **`VMagAngle`.mag** — `| |V_a| - |V_e| | <= |V_a - V_e|` (reverse triangle),
   so the node-voltage band carries over unchanged: `allowed = v_abs +
   v_rel*|expected|`.
4. **The two angle channels** — the *exact* angular image, compared wrap-aware.
   Phasors within `eps` of `V` subtend a half-angle `asin(eps/|V|)` about
   `arg V` while `eps < |V|`, and the whole circle once `eps >= |V|`:

   > `allowed_deg = if eps >= |V| { 180 } else { degrees(asin(eps / |V|)) }`,
   > with `|V|` the **same sample's magnitude in volts** (the pu channel
   > multiplies its per-unit magnitude back by `BaseFactor`, so both polar
   > flavours share one physical band).

   `f64::to_degrees` is `180/PI = 57.29577951308232` — the same full-precision
   constant §monitor-f32-floor's polar-ANGLE band uses. That band is this one
   *linearized* (`asin x ~ x`); the two agree to <2e-3 relative while
   `eps/|V| <= 0.1` (the whole healthy regime) but the linearization
   *under*-estimates the image as `eps/|V| -> 1`, and bus magnitudes legitimately
   reach the absolute floor (unenergized buses; the `NEVTestCase` neutral-earth
   buses sit at ~2 V on a 7.6 kV base). Saturating at 180 deg is not a free pass:
   the magnitude channel still pins `|V|` itself inside `eps` on its own row, so
   an unconstrained angle is exactly a bus whose voltage is at or below the floor
   in **both** engines. The compare folds the difference into `(-180, 180]`
   because `ctopolardeg` returns that range and a phasor on the seam flips sign
   between engines on a 1-ulp difference.

**Measured headroom** (2026-09-04, lane `lane-b`; port vs each case's *gating*
channel(s), all live, 24 356 bus-step comparisons; every case was additionally
run against its NON-gating channel, which is how `4Bus-YYD/YYD-Master` shows a
153x capi divergence — precisely why it is r4133-gated). The number is
`worst |diff| / allowed`; 1.0 would be a failure:

| case (kind) | gating | puVoltages | VMagAngle mag / ang | puVMagAngle mag / ang | AllBusVmagPu |
|---|---|---|---|---|---|
| `IEEE13_CDPSM` (large) | both | **0.658** | 0.494 / 0.435 | 0.494 / 0.435 | 0.494 |
| `Master_ckt5` (large) | both | 0.397 | 0.356 / 0.176 | 0.356 / 0.176 | 0.356 |
| `4Bus-YYD/YYD-Master` (large) | r4133 | 0.355 | 0.310 / 0.300 | 0.310 / 0.300 | 0.310 |
| `Auto3bus` (large_near_ideal_source) | both | 0.291 | 0.026 / 0.290 | 0.026 / 0.290 | 0.026 |
| `TestDDRegulator` (large_floating_zeroseq) | both | 0.268 | 0.256 / 0.262 | 0.256 / 0.262 | 0.256 |
| `IEEE13Nodeckt` (feeder, 24 steps) | both | 0.266 | 0.149 / 0.221 | 0.149 / 0.221 | 0.149 |
| `GFM_IEEE123 GFMSnap` (large_floating_delta) | both | 0.112 | 0.108 / 0.108 | 0.108 / 0.108 | 0.108 |
| `IEEE123Master` (large, 24 steps) | both | 0.080 | 0.070 / 0.078 | 0.070 / 0.078 | 0.070 |
| `LVTestCaseNorthAmerican` (large_floating_zeroseq) | both | 0.076 | 0.070 / 0.076 | 0.070 / 0.076 | 0.070 |
| `NEVMASTER` (feeder, 55 `>3`-node buses) | both | 0.053 | 0.053 / 0.049 | 0.053 / 0.049 | 0.053 |
| `8500-Node/Master` (large, 4 876 buses) | both | 2.60e-4 | 2.60e-4 / 1.32e-4 | 2.60e-4 / 1.32e-4 | 2.60e-4 |
| `indmachtest/Master` (feeder) | both | 1.32e-7 | 1.35e-7 / 5.0e-8 | 1.35e-7 / 5.0e-8 | 1.35e-7 |

`kv_base`, `nodes` and the bus-name sequence matched **exactly on every one of
those buses, on both channels** (0 mismatches in all 24 390 bus-steps measured,
the non-gating and ledger-excluded runs included).

The worst ratio (0.658, `IEEE13_CDPSM` bus `650.4` at 59.86 V) is *identically*
the ratio the node-voltage channel already runs at on that node — algebraically
so for `puVoltages`, since `|dV|/BF / (v_abs/BF + v_rel*|V|/BF) = |dV| / (v_abs +
v_rel*|V|)`. That is the point of the construction: **`compare_bus` cannot red
where `voltages` is green**, so it introduces no new numeric risk and its whole
gating value is the discrete content — the three ordering conventions, the bus
identity/`BusList` order, the node sets and the voltage bases.

**The converse, and why `voltages_excluded` exists.** Because the bands are exact
images, a case whose node voltages are ledger-*excluded* diverges on the bus
surface by the same factor. Measured on the eight such cases, then carrying ten
`{"field": "voltages"}` scopes in `tests/corpus/ledger.json` (**eight** since the
same sub-step's D12/D14 flip moved the two GIC decks to the r4133 channel alone —
the same eight cases): `gic_midi` **1.8e5x** over band on both channels,
`makeposseq_shunt` **7.5e5x** (capi), `windgen_daily` **3.3e6x** (r4133).
Re-comparing them would demand one ledger row per case/channel — ten as measured,
at the §1.1(f) kill threshold — for a cause already triaged and pinned. So `compare_bus`/`compare_all_bus_vmag_pu` take a
`voltages_excluded` flag that suppresses **only** the three continuous arrays;
the bus count, name sequence, `nodes`, `kv_base` and every array length stay
compared, so nothing the bus surface uniquely witnesses is dropped. One
structural rule, no ledger rows, and the negative drive
`the_voltage_exclusion_still_pins_kv_base` proves the suppression is not a mask.

## Short-circuit surface (GOLDEN_REBASE G1.5, `harness::compare_bus_short_circuit`)

`compare_bus_short_circuit` gates the six short-circuit arms of the same `IBus`
facade the section above gates the voltage arms of — `Bus.Zsc1`, `Bus.Zsc0`,
`Bus.ZscMatrix`, `Bus.YscMatrix`, `Bus.Isc`, `Bus.Voc` — read by the SAME
per-bus walk (six reads appended after the five voltage ones, on both
transports). **This surface adds no tolerance constant either**: every band is
an existing tier number applied to the quantity whose derivation already covers
it, and no tier constant moves. In the order the comparator applies them:

1. **Two discrete rows come first, and they are shapes, not numbers.** `Zsc`/`Ysc`
   do not exist until `AllocateAllSCParms` runs inside the FaultStudy solve
   (`Common/SolutionAlgs.pas:773-781`), and the two channels publish DIFFERENT
   not-run sentinels: capi 0.14.5 one double (`DefaultResult`,
   `CAPI/CAPI_Utils.pas:212-221`), r4133 two (the `CZero` prelude,
   `DDLL/DBus.pas:433-434`) — and, at a 0-node bus, capi ZERO doubles for
   `Isc`/`Voc` (`AllocMem(0)` is non-nil, `Common/Bus.pas:250-256`) against
   r4133's two (`Reallocmem(VBus, 0)` frees the pointer, `:246-260`). The
   comparator compares the per-circuit "study ran" bit first and normalizes both
   sentinel shapes to "no matrix" / "no nodes" (coordinator decision D4: a
   sentinel-SHAPE difference is a comparator-level normalization plus a pin,
   never a ledger row and never a tolerance). Both numbers are pinned by
   `the_two_channels_publish_different_zsc_sentinels`. Nothing here is a floor.
2. **`ZscMatrix` is a SOLVE, not an assembly — so it takes the VOLTAGE band.**
   Column `i` of `Zsc` is the node-voltage answer to `Y*V = e_i` on the same
   factored `Y` the power flow uses, restricted to the bus (`ComputeYsc`,
   `Common/SolutionAlgs.pas:800-832` ==
   `solution/solution/fault_study.rs::compute_ysc`); the injection is exactly
   `1 + 0j` A (`Common/SolutionAlgs.pas:812-819`), so the node-voltage band maps onto it numerically
   unchanged — `v_abs` re-read as ohms, `v_rel` being scale-invariant:

   > `allowed = v_abs + v_rel * |expected|`, in ohms.

   `y_abs`/`y_rel` is an *assembly* floor (the stamped `Yprim` sum) and is the
   wrong tier for a solved quantity; it is deliberately not used for `Zsc`.
3. **`Voc` is a copy of `NodeV`** (`UpdateVBus`, `Common/Solution.pas:4070-4083`
   == `solution::ymatrix::update_vbus`), so it takes the node-voltage band
   unchanged — the identity image, not merely a bound. It is live on far more
   than the fault-study decks: `BuildYMatrix` refreshes `VBus` whenever
   `PreserveNodeVoltages` is on (`Common/Ymatrix.pas:170`), i.e. on every
   harmonics/dynamics deck, and it is exactly zero (on all three engines)
   everywhere else.
4. **`Zsc1`/`Zsc0` are averages of `Zsc` entries** — `Zs -/+ Zm` over the
   averaged diagonal and upper-triangle off-diagonal (`Shared/Ucmatrix.pas:356-383`
   == `support/cmatrix::avg_diagonal` / `avg_off_diagonal`), so the same band is
   applied to them. The bound is not free: `Zsc1 = Zs - Zm` is a DIFFERENCE, and
   on a floating-zero-sequence bus the two summands share a huge common mode, so
   an entrywise band on `Zsc` does not formally imply one on `Zsc1`. The
   measurement settles it, and settles it the *good* way — the error cancels
   with the common mode: at the corpus's extreme buses (`ieee37_SC_Currents:775`,
   `IEEE123Master-SC:610`, `Run_NEV:tertiary`) `Zsc0`, the arm that KEEPS the
   common mode, runs at 0.415 / 0.362 / 0.047 of its band while `Zsc1`, the
   differential arm, runs at 1.3e-5 / 1.0e-3 / 4.6e-4 — two to four orders
   TIGHTER. The table below is the standing evidence; growth of the `Zsc1`
   column relative to the `Zsc0` one is a finding, not a band to widen.
   (`AvgOffDiagonal` divides only `If Ntimes > 0`, so `Zm = 0` on a 1-node bus
   and `Zsc1 == Zsc0 == Zsc[0][0]` there — a shape fact, not a tolerance fact;
   pinned by `zsc1_collapses_to_the_single_entry_on_a_one_node_bus`.)
5. **`YscMatrix` and `Isc` carry an inversion cancellation that the tier's
   ABSOLUTE term already covers.** `Ysc = Zsc^-1` through the Gauss-Jordan
   no-exchange kernel all three engines share
   (`Common/SolutionAlgs.pas:828-829`, `support/cmatrix::invert` ==
   `compat::invert_gj_no_exchange_impl`), and `Isc = Ysc * Voc` (`ComputeIsc`,
   `:785-796`). On a floating-zero-sequence bus `Zsc` is a huge common mode — the
   reciprocal of the anti-float `Y_PPM` adder — plus a tiny differential part, so
   the inversion loses `log10 K` digits with `K = ||Zsc||_inf * ||Ysc||_inf`.
   Measured (G1.5 part R) over the three decks surveyed there: `K` is 1..12 on
   every bus except `IEEE123Master-SC:610` (`K = 1.10e8`; common mode
   `+j1.536e6` ohm over a ~0.017 ohm differential) and `Run_NEV:tertiary`
   (`K = 9.52e6`, `+j3.227e6` ohm over ~0.053 ohm), where the predicted floor
   `K * u * ||Ysc||_inf` is `5.847e-7` S and `2.081e-9` S respectively
   (re-derived from each bus's own `||Zsc||_inf`/`||Ysc||_inf` in the F5
   instrumented run; the spec's single 2.9e-7 estimate predated the
   measurement). The live run adds a fourth deck with the same signature —
   `ieee37_SC_Currents` bus `775`, a 1.38e6 ohm common mode against a ~34 S
   `Ysc` entry — and it is the worst of this conditioning family (the worst over
   all six arms is item 6's `Voc`). What admits all
   of them is the tier's ABSOLUTE term: the worst `|dYsc|` anywhere is 1.37e-7 S
   against `y_abs = 1e-6` (0.117 of band), and `Isc` inherits the statement at
   `i_abs` (worst 0.065). **No relative band is widened, no tier constant moves,
   and no `Ysc`/`Isc` entry is excluded anywhere** — the R1 decision tree of the
   sub-step spec lands in branch (i).
6. **Measured headroom** (2026-09-05, lane `lane-b`; the live corpus gate over
   the whole forced population — the 442 cases x their gating channel(s), every
   (case, step, channel) short-circuit comparison green). The number is
   `worst |diff| / allowed`; 1.0 would be a failure. `Zsc`/`Ysc`/`Isc` are
   non-trivial on the four vendored decks that run a fault study (the fourth
   spells it `solve mode=f`, `ieee37_SC_Currents.dss:111`), listed per channel
   below, and on the `micro`-band `faultstudy_micro` deck (worst 1.9e-6, `Voc`);
   the rest compares sentinel shapes, exact zeros and a live `Voc`:

   | case (`feeder`) | channel | Zsc1 | Zsc0 | ZscMatrix | YscMatrix | Isc | Voc |
   |---|---|---|---|---|---|---|---|
   | `ieee37_SC_Currents` | capi | 1.34e-5 | **0.4155** | **0.4154** | **0.1167** | 0.0409 | 0.4422 |
   | `ieee37_SC_Currents` | r4133 | 8.33e-6 | 0.4155 | 0.4154 | 0.0872 | 0.0348 | 0.4422 |
   | `IEEE123Master-SC` | capi | 1.04e-3 | 0.3620 | 0.3619 | 0.0858 | 0.0544 | **0.6098** |
   | `IEEE123Master-SC` | r4133 | 1.04e-3 | 0.3620 | 0.3619 | 0.0935 | **0.0650** | 0.6098 |
   | `NEVTestCase/Run_NEV` | capi | 4.59e-4 | 0.0469 | 0.0469 | 1.12e-4 | 2.26e-3 | 5.64e-6 |
   | `NEVTestCase/Run_NEV` | r4133 | 4.59e-4 | 0.0469 | 0.0469 | 1.40e-4 | 2.96e-3 | 5.63e-6 |
   | `ieee34Mod2_SC_Case_II` | capi | 2.59e-4 | 2.59e-4 | 2.63e-4 | 8.21e-7 | 3.51e-5 | 7.32e-4 |
   | `ieee34Mod2_SC_Case_II` | r4133 | 2.56e-4 | 3.32e-4 | 3.29e-4 | 1.06e-6 | 4.32e-5 | 9.34e-4 |

   Worst `Voc` on a deck that never ran a study: `fuse/indmach_r4133/indmach_dyn`
   0.110 (r4133), `Test/Dynamic_Kundur` / `Dynamic_KundurDynExp` 0.0445 (r4133),
   `FreqScan/Run_Scan` 3.80e-3 (capi) — the same node-voltage floor those decks
   already run at on the `voltages` channel.

   The population worst over all six arms is 0.61 (`Voc` at `IEEE123Master-SC`
   bus `610`, `|V| = 277 V`), i.e. the surface has ~1.6x headroom at its tightest
   point and cannot red where `voltages` is green — the same statement, and for
   the same algebraic reason, as the bus voltage surface above.

**The narrowed `voltages_excluded`.** The bus voltage surface suppresses its
three continuous arrays on a case whose `voltages` field is ledger-excluded
deck-wide (D11(2), section above). This surface reuses the SAME flag but narrows
it to `Voc` and `Isc` alone: `Voc` is a snapshot of the very `Solution.NodeV`
that was triaged and `Isc = Ysc * Voc` is its image, while `Zsc`, `Ysc`, `Zsc1`
and `Zsc0` are functions of `Y` alone — independent of the solution vector — and
therefore stay fully compared on those cases, as do the study bit, the bus
identity and every array length. The negative drives
`the_voltage_exclusion_still_pins_zsc_ysc_and_the_lengths` and
`the_voltage_exclusion_drops_only_the_voc_and_isc_values` prove both halves.

## r4133 event-log masks (`harness::EVENTLOG_MASKS`, §1.3-3)

`compare_eventlog` compares the cumulative event log line-for-line (numeric
skeleton at 1e-6 rel), pinning **when** each control action happened, not just
the final set. UPGRADE_PLAN §1.3-3 allows exact-string comparison against an
EPRI-rev oracle **only after masking documented per-rev format deltas** — the
sequence (order / hours / devices / actions) is never relaxed.

The mask infra (created by **WP-U2.2**) is `EVENTLOG_MASKS` in
`tests/harness/mod.rs`: a per-oracle-spec table of literal `(find → to)`
substitutions applied to **both** the Rust and the oracle line before the
skeleton comparison. A mask only folds a cosmetic text delta — it never drops or
reorders a line (masks are applied *after* the length/order gate, and a
line-dropping "mask" would be a real divergence to fix in the port, not a format
delta). Rows are keyed by the comparison **channel** (`r4133`, …); the pinned
0.14.5 `capi_v0145` channel is never masked. Content-level wording deltas
(actor suffixes, `DER`-vs-`PVSYSTEM OUTPUT` phrasing) are NOT maskable — such
cases stay single-channel with a `note` cause (see the fix-round record).

**The shipped `r4133` table is EMPTY.** The WP-U2.2 Recloser per-phase rewrite
reproduces the r4133 event-log wording byte-for-byte — proven against the
oracle's `export eventlog` CSV (`Phase %d opened on %s (…trip) & locked out
(…lockout)`, `Phase %d closed (…reclosing)`, `Phase ALL reset (3ph reset)`), so
no recloser mask is required; the empty table *is* the proof the port is exact.
The mechanism (and its self-tests in `harness::eventlog_mask_tests`) exists so
WP-U2.3 (Relay) and later revs can add documented rows without restructuring —
one row per delta, `note` citing the delta row this file references.

**r4133 event-log capture (`crates/dss-epri/src/capture.rs`):** the official
EPRI DLL has no populated `Solution.EventLog`-style accessor, so the `dss-epri`
bridge captures the log via `export eventlog` and reads back the CSV, stripping
the UTF-8 BOM per line (the r4133 DLL writes BOM-prefixed CSVs — the WP-U2.2/
U2.5-era discovery, carried into the bridge). The pinned dss-python
`capi_v0145` oracle keeps the direct `Solution.EventLog` read in
`oracle_server.py`.

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
  (`src/General/DSSObjectHelper.pas:2296-2313` addresses the pointer field as if
  it were the array; its JSON arm at `:1242-1266` repeats the slip). The live
  oracle returns **process-dependent garbage** (denormals ~1e-310 in one run,
  huge ~1e123 in another — proven nondeterministic); the captured `props`
  goldens froze it as an all-zero matrix. Oracle UB → not reproduced (CLAUDE.md
  rule), so not comparable. **Both lanes** render the stored lower triangle, as
  the authority does (r4133 `PDElements/Fault.pas:695-717`, `General/
  DSSObject.pas:112-115`), and neither lane compares these values against an
  oracle channel: the corpus gate skips them here, the `props` goldens keep only
  their shape (`props_roundtrip::LANE_SKIP_PROP_VALUES`), and the real numbers
  are pinned by `exec::tests::compat_quirks::
  sym_matrix_text_getter_renders_the_stored_matrix`.
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

- **`IndMach012.PF` / `StorageController.kWhTotal` / `kWTotal` / `kWhActual` /
  `kWActual`** (row group (g), `R4133_PROPS_PLAN.md` §RP3.8, 2026-09-02) — the
  five read-only *results* r4133 renders live. dss_capi 0.14.5 flags each of
  them `[SilentReadOnly, ReadByFunction]`
  (`src/PCElements/IndMach012.pas:288-289`, read function
  `PowerFactorProperty` `:264-267`; `src/Controls/StorageController.pas:416-423`,
  read functions `:309-338`) and never assigns their `PropertyOffset`, so it
  stays `-1` and `TDSSClassHelper.GetObjPropertyValue`
  (`src/General/DSSObjectHelper.pas:2189`) exits at its
  `(PropertyOffset[Index] <> -1)` guard (`:2203-2204`) leaving the string empty
  — on a solved circuit as much as on an unsolved one, on `? name.prop` and on
  `Properties(p).Val` alike (both are that one path; measured at every step of
  six decks). The authority renders the live number instead:
  `Version8/Source/PCElements/IndMach012.pas:1790`
  (`Format('%.6g',[PowerFactor(Power[1,ActiveActor])])`, `PowerFactor` =
  `Common/Utilities.pas:1821`) and `Version8/Source/Controls/
  StorageController.pas:991-994` → `GetkWhTotal`/`GetkWTotal`/`GetkWhActual`/
  `GetkWActual` (`:1162-1197`, all `Format('%-.8g',…)`), so under the 2026-08-02
  policy the engine renders it too, in **both** lanes
  (`PropFlags::RENDERS_LIVE_RESULT`). The capi cell is then `number` vs `''` — a
  structure difference, non-comparable by construction, **never** a tolerance
  question: no floor is involved and none is moved. 24 gating cases / 84
  (case, element, property) cells / 1 006 (cell × step) comparisons, identical
  in both lanes. The exclusion is **capi-only** (`SKIP_PROPS_CAPI_ONLY`): r4133
  shares the render, so that channel keeps comparing all five, where they are the
  ordinary display class (the port prints `float_to_str_ex`, r4133 its
  `%.6g`/`%-.8g` of the same double) that `R4133_DISPLAY_FLOOR` already claims.
  r4133's own read-writes-state — `GetkWhTotal(Var Sum)` is handed the object's
  `TotalkWhCapacity` (`:991-992`) — is not reproduced: nothing upstream ever
  reads those fields. Pinned by
  `props_r4133_pins::{indmach012_pf_renders_the_live_power_factor,
  storagecontroller_fleet_aggregates_render_the_live_fleet,
  the_silent_readonly_capture_cells_are_empty}` plus the two classes' unit pins.

A real port bug this gate caught and fixed (not a skip): **`RegControl.TapNum`**
rendered the cached `tap_snap` while Pascal `Get_TapNum` (`RegControl.pas`) reads
the controlled transformer's **live** `PresentTap[TapWinding]`; the `&self` getter
now resyncs the snapshot from the live transformer at the read choke point
(`Dss::refresh_vterminal_if_marked`) — and, since the RP3.8 audit settlement,
once up front over the store before the `Save` serializer renders
(`Dss::refresh_render_caches_for_save`), which fixed the same latency for
`Transformer.WdgCurrents` and for RP3.8's five live-result rows.

### 0.15.x property-table allowlist (shape relaxation)

`compare_all_properties` also asserts the property-table **shape**: the Rust
`?`-surface name list must equal the oracle capture's name list **in order**, and
the counts must match. The pinned **default** oracle is dss_capi **0.14.5**, so a
deliberately ported 0.15.x-added property (Rung-1: Line `EpsRMedium`/
`HeightOffset`/`HeightUnit`/`Conductors`; RegControl `Idle`/`IdleReverse`/
`IdleForward`/`FwdThreshold`; Transformer+AutoTrans `BHpoints`/`BHcurrent`/
`BHflux`; LoadShape `Mode`) exists on the Rust side but **cannot** appear in a
0.14.5 capture — it would break the count/order/name walk of nearly every
default-oracle deck.

`PROPS_015X` (in `tests/harness/mod.rs`) is a **named per-class allowlist** of
those 0.15.x-only property names. In `compare_all_properties`, a Rust-side
property whose `(class, name)` is in `PROPS_015X` **and** whose name is **absent**
from the oracle capture's name list is excluded from the whole walk (count, order,
name, value) before the comparison. This handles **inserted** props, not only
trailing ones (LoadShape `Mode` lands at index 22, shifting `Interpolation`
22→23): the Rust list is filtered down to the props the 0.14.5 oracle can know,
then compared position-for-position against the capture.

What it **relaxes**: the property-table *shape* (count/order/existence) of a
0.14.5-pinned capture, and only for the named allowlisted props. This is a
§1.3-style shape relaxation, the exact analogue of `SKIP_PROPS` for value
comparability.

What it **never** relaxes:
- **No value tolerance changes** — allowlisted props that ARE present in the
  oracle capture (a **capi015**-regenerated capture) are **not** excluded; the
  full name+value compare applies, so capi015 decks keep pinning the new props'
  values to the case tolerance. Only 0.14.5-oracle decks skip their existence.
- **No masking of real shape bugs** — a NON-allowlisted extra/missing/misordered
  property still fails exactly as before (count mismatch on an extra/missing; name
  mismatch on a reorder). The count-mismatch panic names the allowlist so triage
  finds it.

**Row-documentation requirement:** each `PROPS_015X` row (`(class, &[names])`,
matched case-insensitively) must cite its upstream commit / UPGRADE_PLAN row in a
comment, same style as `SKIP_PROPS`. Rows land with the WP that ports each
property; the table ships **empty** (the mechanism is validated by inline
`props_015x_tests` self-tests with synthetic data). Keep it one class per line so
parallel WP branches each add a line without conflict (duplicate class rows are
fine — the predicate ORs every matching row). Adding a row is a *shape*
declaration only — it never touches any numeric floor.

**r4133 extension (WP-U2.1, Rung 2).** r4133 adds properties and changes defaults
the pinned 0.14.5 oracle cannot report. Rather than mint parallel `PROPS_R4133` /
`HIDE_R4133` mechanisms, the *identical* Rung-1 machinery is reused (the semantics
are the same — a post-0.14.5 surface the 0.14.5 oracle predates):
- **Added props** (Fuse `CurveMultiplier`/`InterruptingRating`): a `PROPS_015X`
  Fuse row (shape walk) + `PropFlags::HIDE_015X` (byte Dump/`Dump commands`/JSON
  goldens), exactly as for the 0.15.x-line additions.
- **Changed defaults** (Fuse `FuseCurve` tlink→none, `RatedCurrent` 1→0): a
  `SKIP_PROPS` row each — category (f) — masking the VALUE only on the 0.14.5
  all-props walk (name still order-checked), the same treatment as RegControl
  `RevThreshold` (e). The r4133 values are pinned on the r4133 side (the
  `fuse.json` props golden + the `fuse_curvemult_blow`/`fuse_legacy_noblow`
  controls decks), never masked there.
- **Upstream stubs** (R4133_PROPS RP1.1 — Generator `Rneut`/`Xneut`, Sensor
  `Action`): properties r4133 still *registers* but no longer implements. Same
  two mechanisms as an added prop — a `PROPS_015X` row per class (shape walk) +
  `PropFlags::HIDE_R4133` (Dump/`Dump commands`/JSON/schema) — plus the new
  `PropFlags::UPSTREAM_STUB`, which is a *behavior* declaration, not a
  comparison relaxation: the write stores the parse string and logs the class's
  soft r4133 message, and nothing masks the resulting value anywhere. On the
  r4133 channel the oracle's own name list carries all three, so `filter_015x`
  keeps them and they are compared in full — which is what retires the census's
  in-scope generator shape rows (137): the port's table is now name-for-name
  r4133's, so no generator element can misalign. Measured on one
  generator-heavy `both` case (RP0.2 knob): r4133 shape classes 1 → 0.
  `Save` is deliberately outside the hidden set — it writes only explicitly-set
  props, exactly as r4133's flag-blind `SaveWrite` does (pinned by
  `exec::tests::upstream_stubs::save_writes_the_stub_names_like_r4133`); no
  corpus deck or `save*` golden writes any of the three.
- **r4133-only props the port really implements** (R4133_PROPS RP1.2 — AutoTrans
  `XfmrCode`): the same two mechanisms again — a `PROPS_015X` AutoTrans row
  (shape walk) + `PropFlags::HIDE_R4133` (Dump/`Dump commands`/JSON/schema) —
  and nothing else. The flag says only "absent from both pinned tables", never
  "not implemented": `xfmrcode=` on an AutoTrans resolves the code and copies
  the electrical model exactly as `TAutoTransObj.FetchXfmrCode` does
  (`Version8/Source/PDElements/AutoTrans.pas:2339-2396`), and no value is masked
  anywhere. On the r4133 channel the oracle's own name list carries `XfmrCode`,
  so `filter_015x` keeps it and the row compares in full — which is what retires
  the census's 7 in-scope autotrans shape rows. The behaviour is gated live on
  the r4133 channel by `asymmetric:autotrans/autotrans_xfmrcode.dss` (Y,
  voltages, currents and 32 property probes at the micro floor, no ledger entry
  and no `expect_warnings`), and its multi-phase form — which r4133 itself
  cannot witness, see the deck's own header — by
  `exec::tests::autotrans_xfmrcode`.
- **A prop r4133's own table LOSES to a registration bug** (R4133_PROPS RP1.4 —
  GenDispatcher `weights`): the same `PROPS_015X` mechanism, run the other way
  round. `TGenDispatcher.DefineProperties` names seven properties but declares
  `NumPropsThisClass = 6` (`Version8/Source/Controls/GenDispatcher.pas:92,133`),
  so `TCktElementClass.DefineProperties` overwrites slot 7 with `basefreq`
  (`Common/CktElementClass.pas:98`) and r4133's `AllPropertyNames` returns 9
  names without `weights` (measured on the DLL; upstream report
  `investigations/to_opendss/40-gendispatcher-weights-registration-off-by-one.md`).
  The port is correct and matches dss_capi, which counts the enum — so the
  `("GenDispatcher", &["weights"])` row is **inert on the capi channel** (that
  name list has `weights`, so `filter_015x` keeps it and the value is fully
  compared, as it has been all along) and relieves the shape walk on r4133 only.
  There is no `HIDE_R4133` flag and no value mask: the prop is real, ported and
  gated everywhere else. The row is **dormant on the live gate** — all three
  `controls:gendispatcher/*` decks are `engines: "capi_v0145"` and stay so,
  because r4133 cannot receive their `weights=` at all and dispatches the equal
  split instead (measured over the 12 steps of `gendispatcher.dss`: on kW, 48 %
  apart at the worst of steps 1-11 and **54.5x at step 0**, where r4133 holds
  both machines at the `Max(1.0, …)` floor while the port dispatches 55.52 kW —
  the census's `generator.kw` `max_rel` 5.45e+01; kvar reaches 1.59e+00 — a
  whole-solution divergence, not a `property`-scoped one). Its only exerciser
  today is
  `harness::props_015x_tests::shipped_gendispatcher_weights_row_is_inert_when_the_oracle_knows_it`
  and — since RP2.1 landed `crates/dss-core/tests/props_r4133_replay.rs` — the
  replay's offline sweep of the full vendored `shape.txt`, where this is the one
  allowlist row that fires on the r4133 side.
This is a *shape/version-mismatch* declaration only — no numeric floor moves.

### r4133 value-spelling normalization (`PROPS_NORM_R4133`) — not a tolerance

`R4133_PROPS_PLAN.md` RP2.1 adds a second, **channel-scoped** relaxation to
`compare_all_properties`, and it is worth being exact about what it does to the
"values compare case-exact (no lowercasing)" rule above: that rule is the
**capi_v0145** contract and stays verbatim there (structurally — every RP2.1
behavior hangs off `PropsPolicy::is_r4133`, pinned by
`props_policy_tests::the_capi_channel_never_normalizes`, and measured by an A/B
run whose capi artifacts are byte-identical). On the **r4133** channel the two
engines legitimately *spell* the same value differently — FPC `Yes`/`No` vs
eleven Delphi boolean spellings, `THashList`-lowercased names vs the as-declared
case, `GetDSSArray`'s `[ 400]` vs r4133's per-class comma/paren forms — so a
table of typed rows (`tests/harness/props_norm.rs`) re-spells the two sides
before the assert, per `(class, prop)`, by one of `BoolFold` / `CaseFold`
(+trim) / `ArrayForm` / `EnumSynonym`.

**It is a spelling rule, never a numeric band.** A rule may change how a value is
written, never which value it is (plan mechanic (c), the
`lane::expected_rerounded` discipline); anything that cannot satisfy that is an
*exclusion* with its own pin, not a rule. RP2.1 therefore introduced **no floor
anywhere**: `ArrayForm`'s numeric tokens compared EXACTLY, because
`props_norm::R4133_DISPLAY_FLOOR` was `None` — the slot RP2.4 has since filled
from the vendored in-scope numeric extract, and its derivation is the **next
section**, which is the only tolerance this plan adds. A wrong number inside an
array still fails (proven by RP2.1's non-vacuity probes: `[ 400]` vs `[ 404]`
reds at 1e-2, and a corrupted token or boolean reds too — since RP2.4 "wrong"
means *outside the floor*, and those probes were re-measured to stay so), and no
`Tolerances` field or tier is touched by either sub-step.

The rows are evidence-bound and both-ways live: each cites its census pair by
`(pair, bin, cells)` in `tests/corpus/props_r4133/`, the offline replay proves
every row claims at least one real census spelling, and per-row hit counters
(dormant until RP4.1 unmasked the r4133 props path on 2026-09-03, live since)
fail on a row that stops folding anything.

The **channel dispositions of `SKIP_PROPS`** land in the same sub-step and are
the other half of this: a row justified by a *channel-independent* fact (the
uninitialized-memory matrix reads, `Transformer.WdgCurrents`' undefined
zero-current angle) stays skipped on both channels, while the *changed-default*
rows — Fuse `FuseCurve`/`RatedCurrent`, RegControl `RevThreshold` — compare on
r4133, exactly as the paragraphs above require ("the r4133 values are pinned on
the r4133 side, never masked there"). `LANE_SKIP_PROPS`' Monitor `BaseFreq`
stays channel-blind: r4133 shares that bug (`Monitor.pas` r4133:552).

Unmasking `RegControl.RevThreshold` on r4133 makes 888 previously invisible cells
appear in the census (`'-100'` ours vs `'100'` r4133, 864 + 24 cells) — an
`EchoDefault` (`RegControl.pas:1448` freezes `PropertyValue[23] := '100'`, the
sibling of `remoteptratio` `:1452`), not a value delta. It is measured, vendored
in `examples_supplement.txt` and owed an RP2.3 echo row plus its pin; RP2.1
deliberately leaves it UNCLAIMED rather than hide it behind a mask.

### r4133 props display floor (`R4133_DISPLAY_FLOOR` = `2e-4` rel, RP2.4)

**The named exception, and its exact scope.** On the **`r4133`** channel only,
and inside `compare_all_properties` only, a divergent **property value cell**
compares within a relative floor of **`2e-4`** instead of exactly — but only when
its r4133 side is *our value rounded to the significant digits r4133 printed*.
Two clauses, both necessary: the **metric** (`display_rel ≤ 2e-4`, the size of
the divergence) and the **mechanism** (`display_is_render`, a `%.Ng` render of
our number). Nothing else in the repo is touched: no `Tolerances` field, no
`tol_for` tier, no golden, no model quantity (Y / V / I / P / losses), no report
text, and not one cell on the `capi_v0145` channel, whose property compare stays
at the case's own tier floors. It is the only tolerance `R4133_PROPS_PLAN.md`
introduces (plan §1.2 last bullet, §1.3 "No tolerance tier moves"), and it is a
**cell** predicate — it reads the two rendered strings and nothing else, so no
`(class, prop)` pair is masked by name and there is no row to go stale. Code:
`harness/props_norm.rs` (`R4133_DISPLAY_FLOOR`, `display_rel`,
`display_is_render`, `under_display_floor{,_r4133}`), seamed into
`harness/mod.rs::compare_prop_lists` through `PropsPolicy::under_display_floor`
(gated on `is_r4133()`) and into `NormRule::ArrayForm`'s per-element compare
through `numbers_match`.

> **The mechanism clause is the RP2.4 audit settlement (2026-08-23).** As landed,
> the floor was the metric alone, and this section asserted — universally — that
> "every claimed cell is r4133 printing a live double in its own
> `GetPropertyValue`" and that "both engines hold the *same* double". Both
> auditors disproved that independently: **55** of the 2 006 claimed vendored
> spellings (70 cells, 27 pairs) are gaps *no* `%.Ng` rounding of our value can
> produce. They are now refused by the predicate and owned by **RP3.9**
> (`props_r4133_replay::RP39_ROUTING`), so the sentence above is a property of
> the code rather than of a survey. The floor value did not move; what moved is
> what it is allowed to claim (2 006 → **1 951** vendored spellings, live
> 49 451 → **49 381** cells over 79 → **69** pairs, with the in-scope count
> unchanged at 46 538 because not one of the 55 has an in-scope cell).

**Measured worst, and the ratio.** Derived measure-first over the vendored
evidence at `tests/corpus/props_r4133/` — `examples_full.txt` +
`examples_supplement.txt`, i.e. **every distinct `(rust, r4133)` spelling of
every census pair**, so the worst *spelling* is the worst *cell* — scored with
the shipped metric (`display_rel`: the largest `|a-b| / max(|a|,|b|)` over the
two renders' numbers).

| quantity | value | what it is |
|---|---|---|
| worst cell the floor claims | **6.431124e-05** | `load.pf` `'0.747651914485831'` vs `'0.7477'` (33 cells, in scope) |
| **the floor** | **2e-4** | **3.110×** above that worst |
| nearest row *above* the band | 1.374769e-03 | `storagecontroller.kwneed` `'-4387.3616098756'` vs `'-4381.33'` — 6.874× above the floor |
| nearest genuine value jump | 4.404256e-03 | `generator.kvar`, the GenDispatcher `weights` decks — 22.02× above the floor |
| smallest **in-scope** genuine jump | 5.524501e-02 | `regcontrol.remoteptratio` — 276.2× above the floor |

The full claims census then re-measured the same number **live** over 439 cases
× 2 channels: the worst cell the floor claims on the whole corpus is that same
`load.pf` spelling at 6.431124e-05, and the derivation's left-hand side is
pinned in three places
(`props_norm::tests::the_display_floor_metric_reads_the_numeric_skeleton`,
`props_r4133_replay::the_echo_table_claims_only_its_cited_pairs_and_the_floor_only_its_derivation`,
and the constant's own doc).

> **Two recalibrations against the plan's provisional numbers**, recorded
> because the plan ordered a re-derivation and got a different second half.
> (1) The plan's worst — `6.43e-5` on `load.pf` — is **confirmed exactly**.
> (2) The plan's "smallest genuine jump `1.00e-3`, `invcontrol.lpftau`" is a
> number in the *census's* metric, which reports the ABSOLUTE difference when
> the expected side is 0 (`harness::value_verdict`). Under this floor's
> symmetric metric that cell (`'0.001'` vs `'0.0'`) is rel **1.0**, not 1e-3 — a
> 0-vs-nonzero pair can never be claimed at any magnitude. The re-derived
> neighbours above the band are the three rows in the table; do not repeat
> "1.00e-3" as the floor's upper neighbour.

**Mechanism — checked per cell, not named in prose.** `display_is_render` asks
of every number of the cell: is r4133's number our number rounded to the
significant digits r4133 *printed* (read off its own spelling, `0.5·10^(e-N+1)`
plus a tie margin and one half-unit of the 15-digit grid FPC's own conversion may
round through)? Exactly two upstream mechanisms answer yes:

1. r4133 **printing** the double both engines hold, through a
   fixed-significant-digit `Format` in its `GetPropertyValue` — the engines hold
   the same double and only the renders differ;
2. r4133 **holding** a double that is itself the re-parse of one such `%.Ng`
   command string an upstream kernel wrote (`MakePosSequence`, the
   `Save`/`PropertyValue[]` round trips) while the port sets the value directly
   — the engines hold different doubles, one rounding step apart, and ours is the
   exact one.

Anything else is refused: a chained or derived round trip (r4133's `load.kva`
recomputed from an already round-tripped `kW`/`kvar` token pair; `vsource.puz*`
from a round-tripped `BasekV`; `line.b0`/`b1` from a round-tripped C), or a plain
state difference that happens to be small. Those 27 pairs are RP3.9's, and it
read every chain off the Pascal — the three heads named here are its measured
ones, not the `pf`/`Z` the RP2.4 settlement first guessed.

The getters that print at a fixed precision, for the reader tracing a cell — the
floor reads the precision off the r4133 spelling itself, never off this table.
`%.Ng` rounds a normalized mantissa `m ∈ [1,10)` to N digits, so its class
ceiling is `0.5·10^(1-N)/m ≤ 0.5·10^(1-N)`. Sites, all `Version8/Source/`:

| formatter | class ceiling | r4133 sites |
|---|---|---|
| `%-.4g` | 5.0e-4 | `PCElements/Load.pas:2345` (`pf`), `:2353` (`CFactor`, no census row) — the only 4-digit property getters in the surface, and the worst cell's |
| `%-.5g` | 5.0e-5 | `PCElements/Vsource.pas:1327-1335` (`angle`/`mvasc3`/`mvasc1`/`isc3`/`isc1`/`r1`/`x1`/`r0`/`x0`) and `:1343` (`basemva`); `PDElements/Transformer.pas:1842-1843` and `PDElements/AutoTrans.pas:1886-1887` (`normamps`/`emergamps`); the `MakePosSequence` command-string round-trips `Transformer.pas:1982-1991`, `AutoTrans.pas:2021-2030`, `Reactor.pas:1145-1201` |
| `%.6g` | 5.0e-6 | `PCElements/Storage.pas:1531-1562` + the PVSystem analogues; `Common/Utilities.pas:2600-2607` `GetDSSArray_Real` (`'[' + ' %-.6g'×n + ']'`) |
| `%-.7g` | 5.0e-7 | `PDElements/Line.pas:1358-1365` (`length`/`r*`/`x*`/`c*`), `:1406-1407` (`b1`/`b0`) |
| `%-.8g` | 5.0e-8 | `Vsource.pas:1337-1342` (`Z*`/`puZ*`) and `:1344` (`puzideal`); `PDElements/Reactor.pas:1091-1098` (`r`/`x`/`z*`/`lmh`) |

`vsource.basekv` is claimed but is deliberately **not** in that table: it is
Vsource property 2 (`Vsource.pas:171`), absent from the getter's `Case`, so it
falls through to `Inherited` and returns the stored `PropertyValue[]` string —
an echoed command-string token whose precision is whatever wrote it. The render
clause reads that precision off the spelling and holds all the same. (Both the
line ranges and the `basekv` attribution were wrong in the as-landed table; the
audit round corrected them here, on `R4133_DISPLAY_FLOOR` and in STATUS.)

The plan's naming of the family as "`%-.5g` with a `%-.8g` `puZ*`/`Z*`
sub-family" is *incomplete*, not wrong: the worst cell is `%-.4g`, a formatter
the plan does not mention.

**The decomposition argument** (this is what makes the number a classification
and not a fudge). The census is itself the decomposition: the divergent numeric
cells of the r4133 property surface split into two populations that do not
touch. Everything explained by the table above lies at or below 6.431124e-05;
the next thing of *any* kind is 1.374769e-03. The band
`(6.431124e-05, 1.374769e-03)` is **empty and 21.38× wide**, so every cut inside
it partitions the population identically — the floor is not tuned, it is placed.
That is the same argument `bins.tsv`'s own 1e-4 bin cut rests on (vendored
`README.md` §"Numeric pairs (bins 6-7)"), re-measured at spelling granularity.

> **The `%-.4g` residual is stated, not absorbed.** `load.pf`'s *theoretical*
> class ceiling is 5e-4, which does not fit the band at all (only 2.75× under
> its upper neighbour). The floor is therefore derived from the MEASUREMENT:
> the census's 293 `load.pf` spellings have a smallest mantissa of **5.653**
> (`pf = 0.5653`), i.e. a population ceiling of **8.845e-05**, and 2e-4 sits
> 2.26× above that. The residual risk is deliberately left loud: a future
> in-scope load with `pf ∈ [0.1, 0.25)` could print a cell up to 5e-4, the floor
> would **refuse** it and the gate would red, with both spellings in the message
> and this derivation to re-run. Widening the floor to the class ceiling so such
> a cell passes is exactly what the CLAUDE.md tolerance discipline forbids.

**Why no real coverage is lost.** (a) Every `engines: "both"` case is *also*
value-compared on the `capi_v0145` channel, where this floor is structurally
unreachable and the compare runs at the case's own tier floors — `micro`
1e-9 rel / 1e-6 abs, `feeder` 1e-7 / 1e-5, i.e. two to five orders under this
floor, so on those cases a genuine numeric change still fails on the other
channel. **Not** "at zero tolerance": `compare_all_properties` hands
`compare_prop_lists` the tier's `i_rel`/`i_abs` and `value_verdict` passes a
number when `|a−e| ≤ abs + rel·|e|` (the as-landed text claimed exactness in five
places; corrected by the audit round). The bound is therefore magnitude-aware,
and the two loosest kinds in the corpus name its hole: `midi` (10 cases; no
`tol_for` arm, so the 1e-6 / 1e-4 fallback) and `micro_wtg3_dynamics` (2 cases,
2e-5 / 1e-4) do not bound a value below **0.5**, and `feeder` does not bound one
below **0.05**. Pinned, with those magnitudes, by
`props_policy_tests::the_capi_property_compare_runs_at_the_case_tier_floors`.
(b) On `engines: "r4133"`-only cases the model gate (Y, node voltages, element
currents/powers/losses, iteration counts, discrete state) runs at the calibrated
`tol_for` tier floors, which are 1e-6-class — orders of magnitude under this
floor — so a property that is genuinely wrong by 2e-4 has to be wrong *only* in
its own render to escape. (c) Every genuine jump the census knows exceeds the
floor by ≥5×: the nearest is 22.02× and the nearest in scope 276.2×; the four
root-cause pairs (`swtcontrol.delay`, `windgen.kvar`, `generator.model`,
`gictransformer.r2`) and RP3.5–RP3.9's residual are all refused by **this
floor**. What happened to them afterwards is *not* "still compared raw", and the
RP5.1 cross-check (2026-09-04) corrects the as-landed wording: RP3.3 gave
`generator.model` a `PROPS_ECHO_R4133` row with its own pin, and RP4.1 landed
eight `property`-scoped `ledger.json` entries — two for `swtcontrol.delay`, four
for `windgen.kvar`, two for `gictransformer.r2` — each pinning both numbers on
its own case (`props_r4133_replay::LEDGER_ENTRY_PINS`). They are therefore
handled by the link *after* the floor, per case and per channel, and every other
cell of those pairs still reaches the assert raw. RP3.9's 55 refused spellings
do stay UNCLAIMED (`count_in_scope = 0`, no entry owed). (d) The floor's refusals are pinned as tests, not assumed: a
4.7e-4 error on a floor-*claimed* property still fails, a 1.1e-5 error on it that
is not a `%.Ng` render fails too, a non-numeric cell on it still fails raw, a
neighbour property 2.1e-4 out still fails, and capi fails on all of it
(`props_policy_tests::the_display_floor_drops_only_the_cell_it_claims_only_on_r4133`).

**Scope justification — r4133 only, by construction.** Both callers sit behind
`PropsPolicy::is_r4133()`, the offline/measurement twin (`claim_value`) refuses
every channel but `PropsChannel::R4133`, and the plain census policy reaches the
floor on neither channel. This is the arm where a leak would cost the most: the
capi property compare runs at the case's tier floors (1e-9 to 1e-6 rel), so a
channel-blind floor would relax every numeric property of every `both` case to
2e-4 at once. Pinned by
`props_policy_tests::the_capi_channel_never_applies_the_display_floor` and
measured by the claims census, which reports **0** capi cells on `under-floor`
(and on every other r4133 disposition). A mutation that drops the `is_r4133()`
gate is caught by two tests; one that widens the floor 10× by nine, across three
binaries.

**Fix owner — none for what the floor claims; RP3.9 for what it refuses.**

*What it claims* is not a defect. Either the two engines hold the same double and
r4133 prints fewer digits, or r4133's double is one `%.Ng` command-string round
trip away from ours and ours is the exact one. Either way the port's value is
right and its render the more informative; there is nothing to fix upstream, no
`investigations/to_opendss/` report, no ledger entry, and no `TODO(compat)` (the
port does **not** reproduce the truncated render — it prints the full value and
the floor classifies the difference). If r4133 ever widened those `Format`
strings, the floor would simply stop claiming; nothing would break.

*What it refuses for the mechanism* was a **work list**: 55 spellings over 27
pairs (`RP39_ROUTING`) where the two engines genuinely hold different doubles.
No cell of any of them is in scope today — the claims census measures
`count_in_scope = 0` on all 55, which is why the finding blocked nothing — and
they sit in `claims_unclaimed_pairs.txt` where WP-RP3 reads.
**Settled by RP3.9 (2026-09-02, audit settled 2026-09-03):** every one of the 27
pairs has its round-trip chain read off the Pascal and a recorded verdict — all
27 `PRECISION_ROUNDTRIP` (the port's double is the exact one; r4133's comes from
the `Format('%-.5g'/'%-.8g', …)` command string its own `MakePosSequence` builds
and re-parses *upstream* of the getter, which then derives at full precision) —
held by ten expected-value pins in `crates/dss-core/tests/props_r4133_pins.rs`
that recompute r4133's literal from the port's own number. No ledger entry is
owed while `count_in_scope` stays 0; the drafts, should a deck's `engines` key
change, are staged in `tests/corpus/props_r4133/README.md`. The floor still
refuses these cells, which is why they stay UNCLAIMED — that is the accounting,
not an open defect. STATUS §RP3.9.

**What it relaxes / what it never relaxes.**

*Relaxes:* on the r4133 channel, the exactness of a property **value** compare
for one cell whose two renders carry the same count of numbers in the same
non-numeric skeleton, every pair of which agrees to within 2e-4 relative **and**
reads as our number rounded to the digits r4133 printed — for scalars, bracketed
vectors and `|`-separated matrices alike.

*Never relaxes:* the capi channel (anything, ever); the property **name** and
index-order walk; the property **count**/shape checks; any cell above 2e-4 at
any magnitude; **any gap, however small, that no `%.Ng` render of our value
explains** (the mechanism clause — `load.kva` at 1.2e-6 and `vsource.puz1` at
1.6e-5 are refused); a 0-vs-nonzero pair (rel is 1 by construction, so bin 7's
frozen-default echoes such as `invcontrol.lpftau` `'0.001'` vs `'0.0'` can never
be mistaken for a render); a cell whose two sides differ in their non-numeric
skeleton or in how many numbers they carry (`'Yes'` vs `'true'`, `'Positive'`
vs `'Pos'`, `''` vs `'[]'`, `'[ 400]'` vs a three-element array, `'17'` vs the
RPN source `'1 16 +'`); a non-finite value on either side unless both are
literally equal; and any `Tolerances` tier, golden byte or model quantity.

*On integers and other discrete values spelled as numbers:* the predicate has no
notion of discreteness — `'4'` vs `'3'` is refused because the gap is 2.5e-1 and
because `'3'` is not `4` rounded to one digit, not because the value is an
ordinal. The mechanism clause is what makes the general case hold: a one-unit
difference at a magnitude the metric alone would fold (`'5001'` vs `'5000'`,
2.0e-4) is refused, since a `%.4g` render of 5001 is `5001`. The as-landed text
listed discreteness as a categorical guarantee, which the metric alone did not
provide (audit round); both refusals are pinned in
`props_policy_tests::the_r4133_channel_claims_the_measured_display_cells`.

*The honest limit:* a floor cannot distinguish a display artifact from a
**genuine** numeric difference that is both under 2e-4 *and* shaped exactly like
a rounding of our value to the digits r4133 printed — no floor can, and the
mechanism clause narrows that residue without closing it. What bounds it is (a)
and (b) above, with (a)'s own magnitude hole stated there.

**RP5.1 cross-check against the landed tree (2026-09-04).** Every number this
section states was re-read from the code at HEAD rather than carried over, and
one claim was corrected (the (c) clause above; the correction is inline and
dated). What was checked, and against what:

| claim here | landed at | verdict |
|---|---|---|
| the floor is `2e-4` relative | `R4133_DISPLAY_FLOOR` at `harness/props_norm.rs:895` (`Option<f64>` = `Some(2e-4)`) | unchanged |
| both clauses ship (metric + mechanism) | `display_rel` / `display_is_render` (`props_norm.rs:1082`), seamed at `under_display_floor_r4133` (`:1175`) and called from `PropsPolicy::under_display_floor` (`harness/mod.rs:6186`) | unchanged |
| the four derivation rows (6.431124e-05 / 1.374769e-03 / 4.404256e-03 / 5.524501e-02) | the constant's own doc table, each row's gap measured as `display_rel` (`props_norm.rs:783-792`) | identical, both places |
| 1 951 vendored spellings claimed (from 2 006, less the 55 the mechanism clause refuses) | `props_r4133_replay::CLAIMED_DISPLAY_FLOOR` = 1951 (`props_r4133_replay.rs:565`) | unchanged |
| capi tier floors the bound rests on — `micro` 1e-9/1e-6, `feeder` 1e-7/1e-5 | `harness::tol_for`, `mod.rs:1087-1096` and `:1104-1113` (`i_rel`/`i_abs`) | unchanged |
| the two loosest kinds — `midi` 1e-6/1e-4 (no arm of its own: the `_` fallback `Tolerances`), `micro_wtg3_dynamics` 2e-5/1e-4 | `mod.rs:1279-1288` and `:1268-1277` | unchanged |
| the magnitudes the bound does not cover — 0.5 / 0.5 / 0.05 | `props_policy_tests::the_capi_property_compare_runs_at_the_case_tier_floors`, `mod.rs:5078` (asserted as `i_abs / floor`) | unchanged |
| no `Tolerances` field, no `tol_for` tier moved by this plan | `Tolerances` has no props field; the floor is read only by `props_norm` | unchanged |

The floor therefore still sits **3.110×** above the worst cell it claims and
**6.874×** under the nearest row above the band, and the band
`(6.431124e-05, 1.374769e-03)` is still the empty one the placement argument
rests on. Nothing in this section was widened; the one edit tightened a
description (RP5.1, docs-only).


## G1.9 circuit aggregates + solution scalars (`harness::aggregates`)

`GOLDEN_REBASE_PLAN.md` WP-G1 G1.9 put the five `Circuit` aggregates
(`Losses`, `LineLosses`, `SubstationLosses`, `TotalPower`, `AllElementLosses`)
and the ten `Solution` scalars on the live corpus gate, unflagged and universal
(all 519 live cases, every gating channel). **No tolerance class changed and no
new floor was introduced**; the two bands below are f64 *identity* bands over
quantities that are the same sum on both sides, and every value comparison
reuses an existing floor. Numbers measured 2026-09-04 on the whole corpus
(3 493 checkpoints across both channels), lane `lane-s`.

### P1 — membership reconstruction (`AGG_SUM_REL` 1e-12 / `AGG_SUM_ABS` 1e-9)

Each loss aggregate is rebuilt from the **oracle's own** per-element `Losses`
capture over the **port's** summand list (`Dss::aggregate_terms`) and compared
to the oracle's own reported aggregate. Both sides are the same f64
accumulation of the same terms in the same order — upstream walks `PDElements`
/ `Lines` / `Transformers` in list (creation) order (`Common/Circuit.pas`
:2436-2444, `DDLL/DCircuit.pas`:313-320, :335-342) and the port mirrors those
lists — so the only admissible difference is `N * eps` accumulation noise:
`4889 * 2.22e-16 ~= 1.1e-12` relative on the largest corpus deck. Hence
`1e-12 * sum|term| + 1e-9`.

Measured worst `|recon - oracle| / sum|term|`:

| aggregate | worst ratio | worst case |
|---|---|---|
| `Circuit.Losses` | `8.259339087510259e-16` | `StorageControllerTechNote/Schedule/ScheduleRun.dss` |
| `Circuit.LineLosses` | `1.610151713239062e-14` | `EPRITestCircuits/ckt7/Master_ckt7.dss` |
| `Circuit.SubstationLosses` | `1.497413754786237e-16` | `StorageControllerTechNote/PeakShaveDch_PeakShaveLow_Ch` |

i.e. 60x (LineLosses) to 1 200x (Losses) inside the band. Rows over the band:
**0 of 3 493**. This band may only ever tighten — it is not a floor absorbing a
physical difference, and a failure here means the summand SET differs (one
whole element's loss moves the reconstruction), not that a number drifted.

The dossier's pre-measurement model for this surface — that `Circuit.Losses`
sums *throughput* and therefore carries a 1e2..1e3 cancellation amplification
between elements — is **refuted by measurement**: it sums each element's own
loss, and those are same-signed. `sum|term| / |sum term|` measured `1.000 …
1.503` (worst 1.5036 on ckt24). No between-element cancellation exists, so no
looser floor is owed anywhere; the amplification that does exist is *inside*
each element's `Get_Losses` and is already priced by
`aggregates::element_loss_allowance_kw` (below).

### P1b — `AllElementLosses` identity (`AEL_IDENT_REL` 1e-12 / `AEL_IDENT_ABS` 1e-9, W)

The oracle's `AllElementLosses[i]` and its own `elements[i].Losses` are the same
`Get_Losses` one `x 0.001` apart (`DDLL/DCircuit.pas`:471 vs
`Common/CktElement.pas`:707-767), i.e. one multiply-rounding. Measured worst
absolute `4.768371582031250e-07 W` (`Test/Dynamic_Kundur.dss`, whose worst
relative is `1.935e-16`), worst relative `6.449284436551366e-16`
(`StorageControllerTechNote/PeakShave`) — **1 550x** inside the relative band.
Rows over the band: **0 of 3 493**.

The same arm asserts `len(AllElementLosses) == 2 * NumDevices` and — new
coverage the gate did not have, since `corpus_gate/runner.rs` compares element
names as `BTreeSet`s — that the port's `ckt_elements` creation order equals the
oracle's `AllElementNames` order, element by element. Measured: **0 order
mismatches** in 3 493 checkpoints on both channels.

### P2 — value: a propagated bound, NOT a calibrated floor

The port's aggregate is compared to the accepted per-element reference inside

    allowed = sum over the aggregate's summands of
              element_loss_allowance_kw(e, tol)          [x1000 for the W-valued Losses]

which is exactly `harness::aggregates::element_loss_allowance_kw` — the
per-conductor envelope `compare_element_channels` has always applied to
`CktElement.Losses` (`sum_k (i_abs * max(1,|V_k|) + i_rel * |S_k|)`, with
`|V_k| = |S_k| / |I_k|`), extracted verbatim so the two can never drift apart.
**No new constant enters**: `losses = sum_k S_k` propagates the conductor
policy, so an aggregate can never pass on a floor its own summands would fail.

Be honest about what that bound is worth. Its *effective* relative width is a
per-case quantity, not a tolerance: `allow / |value|` ranges from `6.5e-4`
(`SubstationLosses` on a feeder) through `6.1e-2` (`Losses`,
`large_floating_zeroseq`) to `2.7` (`Losses` on `Auto1bus-step1`, whose tier
`i_abs` is 0.1 A by construction) and larger still where the aggregate is
near-zero. On the stiff tiers P2 is therefore weak, and it is not the arm that
holds this surface: the numbers are already gated element-by-element upstream of
the sum, and what an aggregate adds is **membership, units and aggregation** —
which P1, P1b and the in-engine pins (`exec::tests::aggregates`, seven of them)
hold. Do not dress P2 up as a tight floor and do not invent a tighter guessed
one.

Measured `|delta| / allowed` on the fully oracle-gated (case, channel) set
(3 460 checkpoints; see "ledger-scoped summands" below), worst per arm:

| arm | worst `\|d\|/allowed` | case |
|---|---|---|
| `Circuit.Losses` | `9.938936237724940e-04` | `Examples/AutoTrans/Auto1bus.dss` |
| `Circuit.LineLosses` | `1.510781045877623e-03` | `Test/CapControlFollow.dss` step 23 |
| `Circuit.SubstationLosses` | `2.494356373731410e-03` | `ADiakoptics/EPRI_Ckt5-G/Torn_Circuit` |
| `Circuit.TotalPower` | `5.825499501120617e-01` | `Examples/AutoTrans/Auto1bus.dss` |
| `AllElementLosses[i]` | `5.825499501120617e-01` | same (`Vsource.source`) |

The tightest arm therefore still has 1.7x headroom, and it sits on the
`large_near_ideal_source` tier where the source current is ill-determined by
construction — the same per-element floor the element comparator already
accepts for that `Vsource`.

**`GOLDEN_REBASE_PLAN.md` §G1.9 kill criterion (`Circuit.Losses` would need a
floor looser than `1e-4` relative on a `feeder`-tier case): NOT met.** Measured
feeder-tier `max |delta| / |Losses|` = `2.719409449622587e-08`
(`Test/CapControlFollow.dss` step 0: `|d| = 4.774932e-03 W` against
`|Losses| = 1.755871e+05 W`) — 3 700x under the threshold, and no floor is
written for it at all.

### Ledger-scoped summands (why this surface added 0 ledger rows)

An aggregate is a linear functional of per-element quantities the gate already
partitions. Where `ledger.json` scopes an element's `powers`/`losses`
sub-channels on a (case, channel), `LedgerView::element_rewrites` hands the
element comparator the accepted cap; the aggregate value arm consumes exactly
those accepted caps, so an already-excluded, already-pinned divergence is
inherited field-by-field instead of being re-stated as an `aggregates` row on
every deck it touches. Twelve corpus cases carry a deck-wide element scope
(the `%R2`-honoured GIC pair, the MMF text reader, `makeposseq_shunt`, the four
WindGen qmode0 decks, the four asym combo/indmach envelope rows); re-pinning
their echo would have cost ~14 rows and tripped the §1.1(f) "> ~10 entries"
kill criterion for a divergence the ledger already owns. `Circuit.TotalPower`
sums `Power[1]`, a per-terminal quantity the capture does not split out, so it
cannot be rebuilt from an accepted cap: since the G1.9 audit settlement its
envelope instead absorbs the accepted `powers` divergence summed over **all**
of a scoped source's conductors (a conservative superset of the terminal-1
part), so the arm keeps running and an entry that scopes only `currents` no
longer switches it off. **P1 and P1b never soften** — they run on the raw
oracle capture on every case, so no deck loses the arms with the teeth.

Where a deck-wide scope selects `losses`, the loss-aggregate value arms are a
self-comparison on that deck, and that is **inherent**: restating them against
the oracle's own aggregate with the accepted divergence added to the envelope is
a tautology (`|Σ(r−o)| ≤ Σ|r−a| + |Σ(a−o)|`), so once the ledger owns every
summand no bound on their sum can carry oracle content the entries do not
already own. The settlement therefore adds *visibility*, not a wider arm: the 14
(case, channel) pairs are recorded and asserted exactly by
`corpus_gate::ledger::the_aggregate_value_arms_inherit_exactly_the_recorded_element_scopes`,
so a new deck-wide element scope reds until its author acknowledges the
consequence (coordinator decision D11(2)'s rule for the bus arrays).

### Solution scalars — every policy reused, none invented

| field | policy | why it is not a new floor |
|---|---|---|
| `mode`, `hour`, `year`, `control_actions_done`, `system_y_changed` | exact, both lanes | discrete state (CLAUDE.md keeps discrete exact in the default lane too) |
| `load_mult` | exact, both lanes | a user/mode-set scalar the engine never computes |
| `seconds` | `abs < 1e-9` (`CLOCK_ABS_S`) | the same constant the already-gated `dblHour` compare uses in `corpus_gate/runner.rs`; `Seconds` is `DynaVars.t` and `dblHour` is maintained from it, so a second value would be incoherent |
| `control_iterations` | exact on `capi_v0145`, `rust <= oracle` on `r4133`, **both lanes** | the `ITER_SLACK` drift model is about the inner power-flow convergence boundary and does not transfer to control-loop passes; a difference here is a control-loop divergence, i.e. a bug |
| `most_iterations_done` | `harness::lane::compare_iterations` / `_le` | it is a max over the step's inner solves (`Common/Solution.pas`:2568 resets it per step, :2701 raises it), so it inherits the inner count's existing policy verbatim |
| `total_iterations` | not compared against the port at all | `SolutionI(40)` returns `Solution.Iteration` verbatim (`DDLL/DSolution.pas`:218-220; capi `CAPI_Solution.pas`:731-738 "Same as Iterations interface"); the oracle-side alias is asserted live on every checkpoint and the port-side one is pinned in-engine |

Measured: **0 scalar mismatches of any kind** in 3 493 checkpoints across both
channels — including `SystemYChanged`, whose agreement answers the G1.9 open
question Q6 (the two engines schedule the Y rebuild identically; nothing to
exclude) and `ControlIterations`, whose §G1.9 kill criterion ("differs anywhere
on the `capi_v0145` channel") is therefore **NOT met**. Witness census over the
same run: `mode` 18 distinct values,
`hour` 79, `year` 2, `load_mult` 7, `seconds` 128, `control_iterations` 39,
`most_iterations_done` 14.

**Two of the ten scalars are one-sided, and the census does not cover them**
(G1.9 audit settlement, correcting an earlier "nothing is vacuous by
construction" here). `control_actions_done` and `system_y_changed` were *not* in
the measured field list, and on every checkpoint that was dumped both are
constant (`true` / `false`): a converged solve settles its controls and leaves Y
freshly built, so the corpus witnesses only one value of each. Their exact
`assert_eq!`s still catch a port that flips one, but the corpus supplies no
witness of the other value, so the two-sidedness is pinned **in-engine** instead
— `dss_core::exec::tests::aggregates::the_two_boolean_solution_flags_take_both_values`
drives the `MaxControlIter` exit (`ControlActionsDone` clear) and a post-solve
structural edit (`SystemYChanged` set). Likewise `control_iterations` is
one-sided on the `r4133` channel by policy (`rust <= oracle`, the row above), so
an *under*-counting control loop is caught only on the 422 capi-served live
cases; measured, the two counts are equal on every one of the 3 493 checkpoints,
so the one-sidedness costs nothing today. `load_mult` is compared with an exact
f64 `assert_eq!` on a lane that does not yet carry coordinator decision D11's
`serde_json` `float_roundtrip` fix; it is green today and can only tighten after
that sync ("D11 — pending sync").


## G1.7 topology interface (`harness::topology`) — **no floor, deliberately**

The topology surface introduces **no tolerance of any kind**, in either lane, and
the absence is a decision rather than an omission. All six compared quantities are
discrete: `NumLoops`, `NumIsolatedBranches` and `NumIsolatedLoads` are counts, and
`AllLoopedPairs`, `AllIsolatedBranches` and `AllIsolatedLoads` are lists of
qualified element names. Counts are compared with `assert_eq!` and names with an
ASCII-case-insensitive equality (r4133 emits `QualifiedName`, capi `FullName`;
measured byte-identical, same case and same order, on all 336 both-gated cases),
in **sequence** order — both sides walk a `TPointerList` in creation order, and
that order is exactly what a topology gate exists to catch, so no set comparison
and no reordering is admitted either.

Nothing here can accumulate a floating-point error: no quantity is a sum, a
product or a solve output, so the "prove the floor by decomposition" rule has
nothing to bite on and a band would only be able to hide a real divergence. The
two measured Rust↔oracle differences on this surface are structural upstream
defects, not numerics, and are handled by positive assertions with zero ledger
rows (`TESTING.md` §"The two topology settlements"), never by a tolerance.


## G1.8 incidence matrix / Laplacian (`harness::inc_matrix`) — **no floor, deliberately**

The incidence surface introduces **no tolerance of any kind**, in either lane, and — as
with G1.7 — the absence is a decision. All four compared quantities are discrete:
`Solution.IncMatrix` and `Solution.Laplacian` are flat `(row, col, value)` triples of
`i32` (the values are `+1` / `-1` in the incidence matrix and small integer degrees and
off-diagonals in the Laplacian), and `IncMatrixRows` / `IncMatrixCols` are lists of
qualified element names and bus names. Integers are compared with `assert_eq!` at every
position and lengths first; names with an ASCII-case-insensitive equality, in **sequence**
order, because both sides are creation-order walks (`Inc_Mat_Rows` grows one entry per
emitted row, r4133 `Common/Solution.pas:3018`; the columns are `BusList` order).

The case-insensitivity is belt-and-braces, not a band: both oracles store bus names
lowercased (`THashList.Add` keeps `LowerCase(S)`, r4133 `Shared/HashList.pas:268`,
`:281`) and build a row label as a hardcoded capitalized class prefix plus the element's
already-lowercase `Name` (`Common/Solution.pas:3019`), so the spellings coincide with the
port's — measured, `SourceBus`/`BusUpper`/`MiXeD` come back as `sourcebus`/`busupper`/
`mixed`. It admits case, never a different name, a different order or a missing entry.

Nothing on this surface can accumulate a floating-point error: no quantity is a sum, a
product or a solve output — the builder walks four element lists and writes `+1` / `-1`
per terminal, and the Laplacian is an exact integer product of that matrix with its own
transpose — so the "prove the floor by decomposition" rule has nothing to bite on and a
band could only hide a real divergence. The three shape differences between the two
channels (capi's over-allocated trailing cell, r4133's one-cell `[0]` sentinel, the two
spellings of an empty name list) are decoded in the **transports** with asserts, and the
comparator asserts the fixpoint rather than repeating the repair; they are not tolerances
and no value passes through them. The one measured Rust↔oracle difference — upstream's
incidence row cursor advancing for a skipped shunt reactor
(`Common/Solution.pas:3039` against its three siblings) — is a structural upstream
defect, handled by a positive assertion of upstream's own numbering with zero ledger rows
and a fail-on-stale population (`TESTING.md` §"Settlement S-INC"), never by a tolerance.

## G1.10a run-file artifacts (`harness::run_files`) — **no floor, deliberately**

The created-file SET introduces **no tolerance of any kind**, in either lane, and — as with G1.7
and G1.8 — the absence is a decision, not an omission. The compared quantity is a *set of
filesystem names*: `compare_run_files` puts both sides in a `BTreeSet` and asserts equality,
reporting the two symmetric differences. There is no number in it, so `rel = abs = 0` is not a
tightened band but the only band the type admits, and the "prove the floor by decomposition" rule
has nothing to bite on. Nothing on this surface is a sum, a product or a solve output; a name is
either created or it is not.

**The ASCII case-fold is a normalization, not a band, and it is forced by the two oracles
disagreeing with each other.** r4133 writes its export stems in upper case —
`FileName := 'EXP_VOLTAGES.CSV'` at `Version8/Source/Executive/ExportOptions.pas:333-356` — while
the pinned dss_capi 0.14.5 writes them in lower case at the same switch,
`src/Executive/ExportOptions.pas:314,343,345`, and r4133 goes further and lowercases the
*deck-supplied* stem as well (`auto1bus_hl_current.txt` against capi's `Auto1bus_HL_current.txt`,
measured on `Test/AutoTrans/Auto1bus.dss`). There is therefore no single "upstream spelling" the
port could adopt without diverging from the other gating channel, and NTFS makes the difference
unobservable to any behaviour. Folding ASCII case on all three producers is what lets one set be
compared against both channels; it is in the `PROPS_NORM_R4133` tradition (a documented shape
normalization with a pin), and it is deliberately **ASCII-only** so that a non-ASCII name is
refused loudly rather than folded by one language's locale rule. What it admits is case, and
nothing else: a different name, a different extension, an extra member or a missing member all
still fail. The literal pin, both spellings written out, is
`run_files_pins::the_two_oracle_spellings_of_auto1bus_fold_to_one_member`, and
`harness::run_files::tests::the_fold_keeps_the_extension` is the negative side.

The other two deliberate reductions of the set are likewise structural, counted and pinned, never
bands. The engine-internal harmonics scratch file `<CircuitName_>SavedVoltages.dbl` (r4133
`Common/Utilities.pas:1512-1521`, read back at `:1554-1564`) is split off symmetrically on every
producer and its declining population is re-derived on every run as `SCRATCH_FILE_DECLINES = (9, 9)`,
fail-on-stale in both directions, with `run_files_pins::the_harmonics_scratch_file_is_declined_on_the_nev_deck`
naming both numbers (oracle 7, port 6). The r4133-only `Visualize` DSSView pair is a single
`ledger.json` exclusion scoped by `name_re` with
`run_files_pins::visualize_writes_a_dssview_pair_on_r4133_and_a_json_payload_in_the_port` naming
r4133's 6 against the port's 4. Both are exclusions of *named members*, decided per name and pinned
by value — the field-by-field shape this file's rules require — and neither widens anything for any
other name on any other case. See `TESTING.md` §"G1.10a — the created-file SET".


## §AD — A-Diakoptics AD↔normal equivalence (D7 calibration, WP-AD.3)

A-Diakoptics is an **EXACT** domain decomposition: at convergence the AD stitch
reproduces the interconnected-coordinator solution, not an approximation of it.
Proven by decomposition (WP-AD.3 audit finding #2, not a tolerance sweep): a
`Set algorithm=Newton` AD deck runs a full-system Newton on the *closed*
interconnected coordinator (Solution.pas:1018 — `DoNewtonSolution` has no
ADiakoptics branch, `SolveSystem(dV,1)` uses `@V[1]`) that never touches the
children or the re-seed, and its solved NodeV matches the fixed-point AD stitch to
**f64 ulp** (midi `7e-13`, macro `1.3e-12`;
`{midi,macro}_newton_ad_matches_fixedpoint_ad`).

The AD↔**normal** "floor" below is therefore *not* a boundary-model approximation:
it is the difference between the reconstructed interconnected-coordinator deck and
the original deck — **shared** by the fixed-point and Newton AD paths (both
reproduce the coordinator solve) and matched to the r3723 oracle. It is a genuine
floor of OpenDSS's own AD (reproduced 1:1), and it does not collapse under tighter
tolerance because it is a deck-structure difference, not an iteration residual. Per
plan D7 the tier is calibrated once, here, and never loosened.

Calibration (r3723 Oddie probe, 2026-07-12; `solve mode=snap`, `controlmode=off`,
worst node by max rel `|V_ad − V_normal|`):

| fixture | oracle floor (r3723) | Rust floor | D7 tier (×4) |
|---|---|---|---|
| midi (2 zones) | 3.25e-5 | 3.21e-5 | 1.3e-4 |
| midi (3 zones) | — | 3.21e-5 | 1.3e-4 |
| macro (2 zones) | 1.318e-4 | 1.319e-4 | 5.3e-4 |

Time-series legs (`midi_daily24_matches_normal`, `macro_yearly168_matches_normal`)
run the same comparison over a daily-24 / yearly-168 horizon (constant-load
fixtures → each step is a snapshot); they exercise `ad_solve_time_series` and pin
the same floors plus a clock-advance assertion.

**Oracle-anchored AD SOLVE voltages** (`tests/ad_reference.rs`, D9 b/c — the AD
solve output vs a trusted external baseline, not just AD↔normal self-consistency):

| deck (manual cut) | ZLL | ZCC | Y4 | solved NodeV | node tier |
|---|---|---|---|---|---|
| IEEE-13 (`Line.670671`, 2 zones) | 3.8e-15 | 6.2e-8 | 2.7e-8 | **1.2e-6** | 1e-5 |
| IEEE-123 (`Line.l10,l73`, 3 zones, 2 ref-free) | 3.0e-15 | 7.4e-10 | 4.9e-10 | **4.85e-6** | 2e-5 |

**Tolerance stability (the permanent tighten-proof).** On the oracle the floor is
bit-stable as `ConvergenceTolerance` tightens 1e-4 → 1e-10 (midi 3.265e-5 →
3.254e-5; macro 1.318e-4 flat; iteration counts `itN == itA`), i.e. it does **not**
collapse. `midi_d7_gap_stable_under_tighten` / `macro_d7_gap_stable_under_tighten`
(`tests/adiakoptics.rs`) assert the Rust loose/tight ratio stays in `[0.5, 2.0]`; a
gap that *collapses* would flag a cross-step state leak, a *balloon* a port bug.

**WP-AD.4 corpus sweep tier (`AD_SWEEP_TIER = 2e-3`, `corpus_ad_matches_normal_mode`).**
The per-fixture D7 tiers above are individually calibrated for the synthesized
midi/macro fixtures. The corpus-wide sweep compares AD↔normal (snapshot,
name-keyed node V) across ~420 heterogeneous decks rust-vs-rust; it uses ONE
conservative ceiling every eligible (`pf`) deck clears empirically — `2e-3`, ~15×
the macro fixture floor, still far below any physical significance so a real port
bug blows straight past it. A deck whose measured gap exceeds it is classified
`off:<specific-reason>` in `ad_sweep.json` / the family manifests, **never**
tolerance-widened (§5). The tier is the sweep's coarse net; the tight per-fixture
D7 tiers remain the fine-grained equivalence proof.

**Sweep finding — the canonical-feeder gap is the D7 round-trip (save) leg, not
AD.** The standard feeders (IEEE13/34/37/123, 8500-Node, StoCtrl, VSConverter,
SolarRamp, …) show 2–66 % AD-vs-normal snapshot gaps — *far* above the fixture
floor. The `DSS_AD_DECOMPOSE` probe splits each into the two D7 legs and proves
the gap is **entirely** leg 1 (`save circuit` fidelity: original-solved-normally
vs saved `Master_Interconnected.dss`-solved-normally), while leg 2 (the AD stitch
proper: interconnected-normal vs AD) is clean at the fixture floor:

| deck | leg 1 (save round-trip) | leg 2 (AD proper) |
|---|---|---|
| IEEE13Nodeckt | 5.88e-2 | **1.63e-6** |
| ieee37 | 1.16e-1 | **1.28e-5** |
| 123Bus Run_YearlySim | 6.25e-2 | **7.96e-6** |
| 8500-Node Voltage_Profile | 2.54e-1 | **7.6e-6** |

So the AD engine reproduces the interconnected solve to the fixture floor on the
real corpus too; the gap is a `save circuit` regulator/transformer-state (and
LineGeometry/WireData/relative-file) fidelity gap — fixed in save, not AD (exactly
D7's round-trip-leg guidance). These decks are `off:save-roundtrip-*` in the
manifests. Genuine AD-leg exceptions are narrow and separately labelled:
`off:ad-floor-above-tier` (IEEE34's long-radial stitch floor ≈2.1e-3, just over
the tier — a real floor, not widened) and `off:ad-islanded-divergence` (Microgrid/
ISource, GFM-8500 snapshot: source-free islanded topologies where the AD leg
itself diverges — leg 2 = 2.0 / 7.4e-2).

**Documented deviation** (`ad_solve_into_parent`): official freezes each child's
own `NodeV` at its state-2 standalone solve (probe: actor-3 `NodeV` moves `0.0`
across the AD solve; that state-2 solve is already 7.9e-5 from interconnected via
`PConn_Voltages`). A byte-faithful freeze diverges in this port to 3.46e-3 at the
deep node of a **reference-free** zone: `Start_Diakoptics` disables its sources so
`GetPCInjCurr` linearises the constant-power loads at the frozen (7.9e-5-off)
voltage, and that current error is amplified down the long radial through the
ill-conditioned (source-free) child `hY`. Pascal (KLU) tolerates the same freeze;
this port (faer) does not — the leading, **not-yet-bit-proven** hypothesis is a
faer-vs-KLU difference on the near-singular child factorization. Re-seeding the
child `NodeV` with the solved column each iteration removes the frozen-linearisation
error at its source and is *proven* to recover the exact interconnected answer (the
f64-ulp Newton match above) — an explicit, documented compensation (never a silent
Y regularization, §5). Open item (WP-AD.4): bit-level confirm the freeze cause
(run the frozen reference-free child through faer AND KLU) so the re-seed can drop.

## Deliberately-reproduced upstream inexactnesses

Tolerances absorb f64/ULP differences only. A deliberately-reproduced upstream
inexactness is **never** handled here — widening a band to cover one is exactly
the fudging this file forbids.

DE_PASCALIZE Stage F was that "one pass" (PORTING_PLAN.md §4.1 / §6). Its
outcome is not a deletion but a **lane split**: each resolved inexactness became
a `crate::compat::` row with two always-compiled impls, the parity lane
(`--features dss-core/oracle-parity`) keeping the upstream answer its goldens
pin and the default lane taking the clean fix. Consequences for tolerance work:

- **The parity lane's floors never move.** It is the oracle-compared lane, so
  every number in this file applies to it unchanged.
- **The lanes exclude, they do not loosen.** Where a row makes the product
  answer something an oracle channel does not, the affected *fields* drop out
  of the oracle compare (`GateSpec`/`LANE_SKIP_*`) and are pinned by their own
  expected-value tests. Since the 2026-08-02 policy (CLAUDE.md) a bug fix lands
  in **both** lanes, so such an exclusion is unconditional rather than
  default-lane-only — `LANE_SKIP_ELEM_POWERS` (the two `modes:newton` decks'
  element powers/losses, no oracle rev reports them at the converged `NodeV`)
  is the live example. A tolerance is never widened to cover either shape; if
  you find yourself wanting to, the row is mis-scoped.
- A marker still spelled `TODO(compat)` in the tree is one Stage F **escaped**
  with a measured blocker, and every survivor is registered in
  `oracle_parity_cfg_gate::ESCAPE_REGISTER` with its owner.
