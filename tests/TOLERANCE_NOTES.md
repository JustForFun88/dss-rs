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

## `TODO(compat)`

Tolerances absorb f64/ULP differences only. Deliberately-reproduced upstream
inexactnesses are **not** handled here — they are marked `TODO(compat)` in the
code and pinned by exact golden values, removed in one pass after the 1:1 port
reaches final acceptance (PORTING_PLAN.md §4.1 / §6).
