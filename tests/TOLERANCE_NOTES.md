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
  primary voltage/current correctness gate stays the live full-model compare
  (`corpus_live.rs`, 1e-8 rel) — a report bug cannot hide there, and a physics bug
  shows there first. Concretely (`Export Voltages` on IEEE13):
  - default value columns (`Magnitude%d` `%10.6g`, `pu%d` `%9.5g`, `BasekV`
    `%.5g`): **1e-4 rel, 1e-6 abs** — clears the 5–6-significant-digit `%g`
    rounding floor (~1e-5 rel) plus the two independent solves, with margin.
  - the `Angle%d` columns (`%6.1f`, one decimal): a **per-column override** of
    **1e-3 rel, 0.11 abs**. Two independent solves round that last 0.1 digit
    independently, so a ±0.1 boundary difference is a formatting artifact, not a
    divergence. This is the **only** loosened column and it is named explicitly
    via `ExportPolicy::col_tol` (a header-name-prefix match) — **not** a blanket
    relaxation: every magnitude/pu/coordinate column keeps the tight default, and
    the angle is otherwise pinned by the magnitude+pu it derives from. `BusCoords`
    (`%-13.11g`, 11 sig, loaded from the same file) and the text reports
    (`NodeNames`/`YNodeList`) need no override.

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
