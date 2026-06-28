# Tolerance notes

The numeric tolerance policy for the golden gates (PORTING_PLAN.md §4) and every
**field-specific exception** to it. The baseline is:

> pure math 1e-12..1e-9 abs · node voltages 1e-6 rel (floor 1e-9 pu) ·
> angles 1e-6 rad · powers/losses 1e-6 rel · energymeter accumulations 1e-4 rel ·
> discrete states (taps, switch states, control-action counts, island counts)
> **exact** · text outputs compared by numeric skeleton.

Discrete state is always compared exactly and never appears below. Everything
listed here is a deliberate, scoped loosening with its reason; there are no
blanket relaxations.

## Checkpointed-model gate (`golden_checkpoints.rs` / `gen_checkpoints.py`)

This gate captures the **assembled electrical model** after every committed time
step (the system Y, selected element YPrim blocks, the injection vector, node
voltages, and discrete control state) and compares it to the pinned oracle. See
the gate's module doc for the per-step structure.

| Quantity | Micro circuits | Large feeders (IEEE13/123/…) |
|---|---|---|
| node voltages | 1e-9 rel, abs floor 1e-6 V | 1e-6 rel, abs floor 1e-6 V |
| selected element currents / powers; injection vector | 1e-9 rel, abs floor 1e-6 | 1e-6 rel, abs floor 1e-4 |
| assembled Y values; selected YPrim values | 1e-9 rel, abs floor 1e-6 S | 1e-6 rel, abs floor 1e-3 S |
| Y fingerprint (Frobenius / trace / max\|diag\|) | — | 1e-6 rel |
| Y `nnz` (above 1e-9 floor), sparsity pattern, taps/tap-numbers/cap-states | exact | exact |

Field-specific points:

- **The assembled Y is compared *unfactored*.** Oracle side:
  `YMatrix.getYSparse(factor=False)`. Rust side: `Dss::system_y_csc`, which reads
  the assembled `SparseSet` matrix *before* the KLU-style row equilibration that
  `dss-sparse` applies only to the factored copy. So the solver's scaling never
  enters the comparison — the Y check is solver-independent, and the two sides
  match to f64 assembly-summation order (~1e-9 rel) because they sum the same
  ported element YPrims. A stale/leftover admittance is an order-1 relative miss
  at the affected entry and is caught immediately.

- **YPrim layout.** Compared column-major on both sides (`out[col*yorder+row]`),
  the raw layout of the oracle `CktElement.Yprim` (Pascal `TcMatrix`) and of
  `CMatrix`'s own storage — no transpose reasoning, so an asymmetric (e.g.
  regulator) block can't hide a transpose.

- **Sparsity pattern** is compared over the *union* of both patterns with the
  1e-9 abs floor: an entry present on one side but below the floor on the other
  counts as agreement; a genuine pattern change is reported (and flagged
  `SPARSITY PATTERN CHANGED`). `nnz` is counted above the same floor on both
  sides, so an explicit numerical zero kept by one solver and dropped by the
  other is not a spurious failure.

- **Currents / powers / injection abs floor (large feeders, 1e-4).** Dead-end
  branch currents and powers that sum to ~0 are differences of nearly equal
  quantities; 1e-6-rel voltage agreement caps their absolute agreement at the
  µA / fraction-of-a-watt scale. Same rationale as the Phase-4/5 feeder gate.

- **Power abs floor is voltage-scaled: `i_abs · max(1, |V_kv|)`
  (`assert_power_close`).** Terminal power is `P = V·conj(I)`, so a tolerated
  terminal-current error `i_abs` (amps) maps to a power error of `|V|·i_abs` (VA).
  A *flat* `i_abs` kW power floor next to a flat `i_abs` A current floor is
  internally inconsistent — they agree only at |V| = 1 V — so above a few kV a
  current that passes its own floor can push the derived power past a flat kW
  floor. This is not hypothetical: large meshed circuits with near-zero-impedance
  connector lines (1.5 m `BUSBAR` segments and `switch=y` lines, |Yprim| ~ 1e6)
  carry a through-current `I = Yprim·(V1−V2)` that is a catastrophic cancellation
  of two large terms; the ~1e-8-rel node-voltage roundoff that any backward-stable
  solver leaves on such an ill-conditioned Y (cond ~ 1e7 — faer here vs the
  oracle's KLU, *unimprovable*: tightening the solve tolerance to 1e-9 / 12
  iterations leaves it unchanged) amplifies through that cancellation to ~3e-6 rel
  in the current, hence identically in `P = V·conj(I)`. The current's own 1e-4 A
  floor absorbs it (~5–7e-5 A); a flat 1e-4 kW power floor cannot (~5e-4 kW at
  7.2 kV). So the power floor is the **image of the current floor through the
  terminal voltage**: `|V_kv| = |P_kW| / |I_A|`, recovered from the captured power
  and current (self-consistent under positive-sequence ×3, where `|P|` and the
  accepted `δP` scale together), and `max(1, |V_kv|)` never tightens below the
  established floor. This forgives only power error that is the exact image of an
  already-accepted current error — the Yprim (compared at 1e-6), node voltages
  (1e-6), and currents (1e-4 A) all still pin a real regression independently, so
  a genuine power bug shows up in one of those first. It unblocked three large
  EPRI / ADiakoptics cases — `EPRITestCircuits/ckt5`,
  `ADiakoptics/EPRI_Ckt5-G/.../zone_2`, `ADiakoptics/TnDSystem/.../zone_2` — whose
  Yprim is *bit-identical* to the oracle and whose only divergence was this
  connector-line cancellation.

- **Large feeders use the Y fingerprint, not the full CSC.** Storing the full
  assembled-Y CSC every step does not scale (the golden would balloon), so large
  feeders pin a compact fingerprint (`nnz`, Frobenius norm, complex trace,
  max\|diag\|) plus **full-precision YPrim blocks for selected elements**
  (regulators + a load bus). The fingerprint is the cheap structural/magnitude
  guard; the selected YPrim blocks are the precise stale-Y catch at scale (a
  handful of changed diagonal entries do not move a feeder-scale Frobenius norm,
  but they show up exactly in the element's YPrim). The 8500-node snapshot stays
  in its own dedicated gate (`golden_ieee8500.rs`) — duplicating its ~8.5k-node
  voltage/injection arrays here would add ~1 MB for no extra coverage, since the
  fingerprint code path is identical at any scale.

- **Tap float value: 1e-12 rel (not bitwise).** The regulators may partition the
  same net tap movement into different step sequences (node voltages differing at
  the ~1e-9 level can shift a banker's-rounding boundary in one control
  iteration), so the accumulated tap *float* differs by an ulp. The **discrete**
  position is the exact `tap_number` check; the float is 1e-12 rel. Shared with
  `golden_feeders_controls.rs` / `golden_phase5.rs`.

## Live corpus gate (`corpus_live.rs` / `tools/oracle/oracle_server.py`)

The opt-in live gate (CORPUS_TEST_PLAN.md) reuses this gate's comparison layer
and tolerance classes verbatim (`harness::{compare_system_y, compare_yprim,
compare_injection, compare_element, compare_discrete}`, `harness::tol_for`), so
the policy above applies unchanged. Two gate-specific points:

- **Full Y, every case — no fingerprint substitution.** Unlike the checkpoint
  goldens (which store only a fingerprint for large feeders to keep the committed
  file small), the live gate compares the **full** assembled Y entry-by-entry for
  every case (nothing is stored, so size is irrelevant). The fingerprint is still
  checked as a cheap additional guard. There is no exception to the full Y / full
  voltage / full current comparison.

- **Element set compared for equality.** The gate asserts the Rust and oracle
  element name sets are identical (case-insensitive) before comparing each
  element's currents/powers, so an element present on only one side is a failure,
  never a silent omission. Control elements (RegControl, …) carry empty terminal
  arrays on both sides and compare trivially.

- **Monitors / EnergyMeter registers / zones — compared live, opt-in per case.**
  Three cases (`IEEE13Nodeckt`, `ieee37`, `IEEE123Master`) are promoted to 24-step
  daily runs, each with a meter (`m1`) and three deterministic-mode monitors
  (mode 0/1/2) plus `selected_elements` (so they exercise the multi-step per-step
  path **and** YPrim **and** meters/monitors live, across wye / open-delta-LDC /
  multi-bank-regulator control topologies). `compare_monitor` (header + sample
  count exact, channels at `i_rel`/`i_abs` = 1e-6/1e-4) and `compare_meter`
  (register names exact, values 1e-4 rel, zone branch/end/PCE counts exact) are
  the **same** comparators (`harness/mod.rs`) `golden_phase6.rs` routes through —
  one implementation, no drift. Comparison is gated by a per-case
  `check_meters_monitors` flag (`solvable_now.json`), set only for cases that
  define meters/monitors in **deterministic** modes; the always-on
  `solvable_now_has_multistep_depth` test pins that ≥1 such case persists.
- **Why opt-in (oracle quirk, not a Rust gap).** Comparing *every* master's
  incidental monitors would spuriously fail: the pinned dss-python returns a
  **phantom** element from `Monitors.Channel(i)` for an *unsampled* monitor
  (`SampleCount == 0` but `len(Channel(i)) == 1`, verified on EPRI J1's `subVI`,
  which `monitors.dss` defines after the master's only `Solve`). Rust is
  self-consistent there (`channel().len() == sample_count == 0`), so the
  divergence is an oracle artifact, not an engine bug — hence only cases that
  actually sample their monitors (deterministic modes, ≥1 step) opt in. The full
  Y / voltage / current / power / YPrim / injection / discrete comparison has
  **no** such exception and runs on every case.

## Pre-existing exceptions in other gates

- **Property dumps — abs floor 1e-9 (`golden_feeders_controls.rs`).** A few
  per-element property dumps are near-zero cancellation quantities (e.g. a
  near-balanced transformer's ~5e-5 A winding current) whose last printed digit
  sits at the LU solver's backward-error floor and flips under any solve-path
  change (the `dss-sparse` row equilibration). 1e-9 amps/volts/watts is below
  physical significance; the 1e-9 *relative* term keeps significant quantities
  tight.

- **Monitor channels — f32, 1e-6 rel / 1e-4 abs (`golden_phase6.rs`).** Monitor
  data is stored f32 by design (matching the engine); the underlying f64 daily
  trajectory tracks the oracle to ~1e-9, so the narrowed samples match to a few
  ULPs at 1e-6 rel / 1e-4 abs. The mode-5 wall-clock timing channels
  (`SolveSnap_uSecs` / `TimeStep_uSecs`) are not modeled and are skipped.

- **Dynamics mode-3 state-variable trajectories — same monitor policy, 1e-6
  (`exec/tests/dynamics.rs`).** The dynamics gates (Kundur generator — classic and
  DynamicExp — plus PV/Storage/IndMach012) record state variables through the same
  f32 monitor stream, so they follow the monitor-channel policy above. The realized
  Rust↔oracle match on the Kundur gates is ~1e-8 on **every** channel (the swing
  extrema over 11071 steps included), so they are pinned at the standard **1e-6
  rel**, not the looser `1e-5` PORTING_PLAN §Phase 7 *allows* for dynamics (that is
  an upper bound we do not need). Two subtleties, both pinned against the oracle's
  *actual* value rather than against 0:
  - the swing-equation **fixpoint residual** channels (`dSpeed`, `dTheta`, `speed`,
    `dspeed`) are not zero at steady state — `dSpeed = (Pshaft + electrical_power)/
    Mmass` where `Pshaft` is fixed at init but the electrical power is recomputed
    each step, a ~1.5e-8-rel power mismatch / `Mmass`, ≈ −4.3093074e-5 deg/s. The
    oracle reproduces it identically (Rust matches to ~9e-8 rel), so it is pinned at
    its real value, which is a stronger guard than an `abs < ε ≈ 0` bound.
  - the `rel` helper floors its denominator at 1.0, so for these sub-1 residuals the
    `1e-6` pin is effectively a `1e-6` *absolute* band around the oracle value —
    still ~10⁴× tighter than the residual's own magnitude and catching any sign flip
    or order-1 divergence.

- **EnergyMeter registers — 1e-4 rel (`golden_phase6.rs`, 8500 gate).** The
  PORTING_PLAN §4 energy-accumulation policy.

## `TODO(compat)`

Tolerances absorb f64/ULP differences. They are **not** the mechanism for
deliberately-reproduced upstream inexactnesses — those are marked `TODO(compat)`
in the code and pinned by exact golden values, to be removed in one pass after
the 1:1 port reaches final acceptance (PORTING_PLAN.md §4.1 / §6).
