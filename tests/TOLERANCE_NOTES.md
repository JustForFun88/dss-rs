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

**Maintenance:** a new corpus case defaults to `large`; promote it to `feeder`
only after `corpus_live` confirms it holds the tighter floor. Golden tests that
drive a stiff network (`golden_ieee8500`, harmonics/protection/meter scenarios in
`golden_phase6/7`) pass `"large"` explicitly.

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
- **Tap float 1e-12 rel** (`golden_feeders_controls.rs` / `golden_phase5.rs`):
  the discrete `tap_number` is exact; only the accumulated float differs by an ulp
  when regulators partition the same net movement differently.
- **Monitor channels are f32** (1:1 with Pascal `MonBuffer: pSingleArray`), so
  the comparison floor is the f32 ULP — monitor/dynamics channels at `i_rel`/`i_abs`
  (`golden_phase6.rs`, `exec/tests/dynamics.rs`). The mode-5 wall-clock channels
  are skipped.
- **Dynamics fixpoint residuals** (`dSpeed`/`dTheta`/`speed`) are pinned against
  the oracle's actual (small, non-zero) value, not `≈0`: `dSpeed = (Pshaft +
  electrical_power)/Mmass` is a ~1.5e-8-rel residual the oracle reproduces; a value
  pin is a stronger guard than an `abs < ε` bound.
- **`Export Counts` is a `RustSubsetByKey` compare** (`golden_phase8.rs`,
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
- **`Export` solution reports are a layout gate, not the physics gate**
  (`golden_phase8.rs`, `harness::compare_export`). The targeted CSV golden pins
  the report **structure** — header tokens verbatim, column set/order, row order
  (Pascal bus/element iteration order), per-field numeric-vs-text classification,
  the zero-fill, the scaling/getter wiring — against the oracle's exact file. The
  **value** floors are deliberately the report's own *formatting* resolution, not
  the engine floor: the oracle writes each number with `Format('%…g')` /
  `Format('%…f')`, so the file only knows the value to its printed precision. The
  split of duties is precise: **`corpus_live.rs` gates the engine voltage
  physics** — it compares IEEE13's complex node voltages directly at
  `tol_for("feeder")` (1e-8 rel) across the daily operating points, so a
  systematic physics bug surfaces there; **the export golden alone gates the
  snapshot report-layer transform** (`|V|`, `|V|/kVbase`, `arg(V)`, the `×√3`
  line-to-line base, zero-fill, column order) — exact arithmetic, so a wrong
  scale/convention is a gross error caught easily at 1e-4. Concretely
  (`Export Voltages` on IEEE13):
  - default value columns (`Magnitude%d` `%10.6g`, `pu%d` `%9.5g`, `BasekV`
    `%.5g`): **1e-4 rel, 1e-6 abs** — clears the 5–6-significant-digit `%g`
    rounding floor (~1e-5 rel) plus the two independent solves, with margin.
  - the `Angle%d` columns (`%6.1f`, one decimal): a **per-column override** of
    **rel 0, abs 0.11**. The `%f` floor is purely *additive* — two independent
    solves round that last 0.1 digit independently, so a ±0.1 boundary difference
    is a formatting artifact, not a divergence — hence `rel = 0` (no multiplicative
    `%f` component) and `abs = 0.11` is the exact proven printing floor. This is
    the **only** loosened column and it is named explicitly via
    `ExportPolicy::col_tol` (a header-name-prefix match, applying to `Angle1/2/3`
    only) — **not** a blanket relaxation: every magnitude/pu/coordinate column
    keeps the tight default. The angle is `arg(V)`, independent of `|V|`/pu; its
    engine correctness is gated by the `corpus_live` voltage compare (a gross
    rad↔deg / sign-flip bug is far above 0.1 and caught either way). `BusCoords`
    (`%-13.11g`, 11 sig, loaded from the same file) and the text reports
    (`NodeNames`/`YNodeList`) need no override.
- **WP8.2 sub-step 2a — the aggregate PD/PC power exports** (`Powers`/`Losses`/
  `P_byphase` on solved IEEE13, both kVA and MVA). All columns are real (kW/kvar/W
  or MW/Mvar) — no angle or sequence columns — so the floors are the plain
  fixed-decimal / `%g` *printing* floors, **not** a physics relaxation (the engine
  V/I/P is pinned to 1e-8 by `corpus_live`; here the golden gates only the report
  layout/scaling/element order). Each floor below is the **empirically measured**
  max Rust↔oracle divergence with margin (audit-tests WP8.2 mutation-verified each):
  - `Powers` — every value column is `%11.1f` (one decimal); like the `Angle%d`
    floor this is purely *additive*, so the **whole policy** is `rel = 0, abs = 0.11`
    — measured max divergence 0.0 (identical strings), `rel = 0` so no multiplicative
    band masks a per-terminal error; the integer `Terminal` column is exact within
    abs. (MVA twin: same `%11.1f` floor, now in MW.)
  - `P_byphase` — values are `%10.3f` (three decimals); the floor is purely
    *additive* (measured max divergence **exactly 1e-3** — one ULP, on the largest
    −1342.212 conductor; its 7.45e-7 "rel" is just `abs/value`), so the policy is
    `rel = 0, abs = 0.0011` — a `rel` band would be superfluous and would mask a
    per-conductor scale drift (mutation-confirmed: 0.005% passes under `rel=1e-4` but
    fails under `rel=0`). The integer NumTerminals/NumConductors/NumPhases columns are
    exact within abs. (MVA twin: same `%10.3f` floor, in MW.)
  - `Losses` — `%.7g` (7 sig), so the floor is the **7-sig printing floor**, not the
    looser 6-sig `EXPORT_REL` of the Voltages report: measured max divergence on a
    substantial loss is **1.53e-7 rel** (one ULP at 7 sig, REG2's 65.3 W), so
    `rel = 1e-6` (≈6× over the floor) is the report's printing resolution. `abs = 1e-6`
    absorbs a few near-zero no-load/var cells (cancellation noise ≤ 5.6e-9 W) with
    ~180× margin. No genuine line-loss cancellation floor materializes on IEEE13 (the
    line/transformer losses agree to the 7-sig printing floor); should a metered feeder
    later show one, it must be **proven by decomposition** (CLAUDE.md), not by widening
    `rel`.

- **WP8.2 sub-step 2b — the symmetrical-component exports** (`SeqVoltages`/
  `SeqCurrents`/`SeqPowers` on solved IEEE13). Again a *report-layout* gate; the
  underlying V/I are pinned to 1e-8 by `corpus_live`. Column structure: the
  magnitude columns (`V1`/`V2`/`V0`/`Vresidual`, `I1`/`I2`/`I0`/`Iresidual`) are
  `%10.6g` (6 sig); the ratio/unbalance columns (`%V2/V1`, `%V0/V1`, `%NEMA`,
  `%Normal`, `%Emergency`, `%I2/I1`, `%I0/I1`) are `%8.4g` (4 sig).
  - **Magnitudes** keep the 6-sig `EXPORT_REL = 1e-4` with `abs = 1e-3`. The `abs`
    floor absorbs the **cancellation residual** of the sequence quantities on a
    balanced feeder: `V0`/`V2` (and `I0`/`I2`) are a near-zero difference of three
    balanced phasors, each pinned to 1e-8 rel by `corpus_live`, so the residual's
    *absolute* error is ~phase_scale·1e-8 (≈7e-4 V at the 66 kV source bus) while
    its relative error is large. `abs = 1e-3` clears that with margin and cannot
    mask a layout bug (a mis-scaled/mis-placed column lands orders of magnitude off).
  - **Ratio columns** use a `ColTol` prefix `%` with `rel = 1e-3` — the exact 4-sig
    `%8.4g` printing floor (one ulp in the 4th significant digit) — and a
    **denominator gate** (`ColTol::gate`). `SeqCurrents` gates on `I1` (col 2,
    threshold `1e-6 A`): at an open/unloaded terminal `I1`/`I2`/`I0` are ~1e-12
    cancellation noise (pinned to 0 by the magnitudes' `abs`), so `%I2/I1` etc. are
    a faer-vs-KLU **noise/noise** form — e.g. `Line.671680.2` (the open 680 end)
    prints `%I2/I1 = 147.7` (oracle) vs `61.8` (Rust), uncheckable. The gate skips
    the *ratio* cell only there; on IEEE13 that is **1 genuine-noise row** plus 32
    rows where `I1` is *exactly* 0 (single-phase elements, non-positive-sequence →
    ratio trivially `0 = 0`, would pass anyway), against **22** meaningful rows whose
    ratios stay checked at `rel = 1e-3` (min meaningful `I1 = 5.8e-4 A`, 581× above
    the gate). The magnitude columns are checked on **every** row — only the derived
    ratio is gated, only where its denominator is below the physical scale. A proven
    cancellation floor (CLAUDE.md), **not** a relaxation. `SeqVoltages` needs no gate
    (V1 is always kV-scale) and `SeqPowers` is all `…:1` fixed-decimal, so its policy
    is the `Powers` additive floor (`rel = 0, abs = 0.11`); its PD rows carry the
    extra 4 excess-kVA columns on terminal 1 (12 fields) vs 8 for PC/term-2 rows, and
    the comparator pins each row's field count.
  - **`TODO(compat)` in `SeqCurrents`** (`seq_currents.rs`): `Iresidual` sums the
    *terminal-1* conductors for **every** terminal row (Pascal indexes `cBuffer^[i]`,
    not `cBuffer^[(j-1)*Ncond+i]`) — an upstream quirk reproduced verbatim so the
    golden's `Iresidual` column matches; the clean fix is the per-terminal slice.

## Live corpus gate (`corpus_live.rs`)

Reuses the same comparators and classes verbatim. Differences from the checkpoint
goldens: the **full** assembled Y is compared every case (nothing is stored, so
size is irrelevant — checkpoints store only a fingerprint for large feeders); the
Rust/oracle element name sets must be identical; monitors/meters are compared live
only for cases that sample them in deterministic modes (`check_meters_monitors` in
`solvable_now.json`) — an unsampled monitor returns a phantom channel from the
pinned oracle, an artifact, not an engine gap.

## `TODO(compat)`

Tolerances absorb f64/ULP differences only. Deliberately-reproduced upstream
inexactnesses are **not** handled here — they are marked `TODO(compat)` in the
code and pinned by exact golden values, removed in one pass after the 1:1 port
reaches final acceptance (PORTING_PLAN.md §4.1 / §6).
