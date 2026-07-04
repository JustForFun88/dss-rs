# dss-rs — Project Status & Session Handoff

> **Purpose of this file:** a living snapshot so a fresh session can resume
> without re-deriving context. It records *what is done*, *what was decided*,
> and **why**. It is **not** authoritative for the plan itself — that is
> `PORTING_PLAN.md` (roadmap + binding decisions) and `CLAUDE.md` (conventions
> + the green-gate rule). Read those two first; then read this for the current
> frontier.

Last updated: 2026-07-04 — **Phase 8 IN PROGRESS** (`PHASE8_PLAN.md` —
reporting/exports/Save; branch **`phase-8-reporting`**, branched from the
gate-green Phase-7 tip). **WP8.1–8.3 COMPLETE + audited; WP8.4 (Show) steps 1–8
gate-green** (Buses/Losses/Taps/Voltages/Currents/Powers seq+elem + Elements +
Result/EventLog/Ratings/Variables/Mismatch/monitor + step 7: Convergence/Y/
controlqueue/kvbasemismatch + **step 8: Meters/Generators** register tables +
dispatcher; ~21 `Show` reports ported).
**~10 `Show` reports remain** (all *silent* no-ops, `TODO(WP8)`: Controlled/Zone/
Overloads/Unserved/FaultStudy/Yprim/LineConstants/Isolated/Loops/Topology/busflow/
autoadded + deltaV/QueryLog). Full Phase-8 detail is in **§1f**;
the current frontier:

- **WP8.1 COMPLETE** (dispatch skeleton + output-path machinery + `Export Counts`
  + the `compare_export` golden harness).
- **WP8.2 COMPLETE** — the solution/power/symmetrical-component/per-terminal/matrix
  export families (`Voltages`…`Currents`…`Yprims`/`Y`/`SeqZ`/`Summary`/`Result`) +
  the mutable element-walk infra; the completion gate migrated the `Export`-unblocked
  corpus (`solvable_now` **88→119**, COVERAGE **26.3%→35.5%**) + landed the Rust
  `CorpusGuard`. The "9 decks hang" and "live-gate flake" tracked-opens are both
  **RESOLVED** (§1f Issue-1/Issue-2).
- **WP8.3 COMPLETE (steps 1–5 + both audit follow-ups), gate-green** — the
  device/meter/reliability/log exports (`Monitors`/`Meters`/DER/`EventLog`/
  `Faultstudy`/`BusReliability`…/`Sections`/`Profile`) + the `TSystemMeter` core
  and the full demand-interval (`DI_*`) file machinery + its `Set`/`Set year=`
  wiring (§2.6). The completion gate migrated `solvable_now` **119→168** (COVERAGE
  **50.1%**), incl. fixing the **silent Spectrum `CSVFile` no-op** two IEEE_519
  harmonicT decks exposed. The two independent audits found **no correctness bug**;
  two LOW code findings fixed (`Export Profile` `1732.0` `TODO(compat)` marker; the
  `Spectrum.read_csv_file` byte-position EOF guard, oracle-confirmed) + five
  oracle-pinned coverage tests (incl. the multi-meter `Bus_Int_Duration` cross-zone
  bug — filed upstream + in-range regime gated). golden_phase8 **59**; lib **731**.
  Detail in §1f.
- **WP8.4 (Show reports) — step 5 COMPLETE, gate-green** (audited with step 6, see
  the next bullet). The `do_show_cmd` dispatcher (`ShowOptions.pas` option/solve-guard) + the new
  `report/show/` module of fixed-width text formatters: `Show Buses`/`Losses`/`Taps`
  + `panel`→#999 (step 1); `Voltages` seq (step 2); `Currents`/`Powers` seq (step 3);
  `Voltages` node/elem + `Currents` elem + `Elements` (step 4); **`Powers` elem +
  `Result`/`EventLog`/`Ratings`/`Variables`** (step 5). Remaining unported keywords
  (meters/zone/topology/lineconstants/yprim/y/faults/mismatch/…) stay *silent*
  headless no-ops. Shared machinery: `format.rs` `Pad`/`PadDots`/width formatters, the
  whitespace+comma `compare_export` tokenizer (`sep: ' '`) + `ColSel::AfterToken` +
  `GateSpec::MinCols` (PF-of-degenerate-power gate). **Key finding (step 5):**
  `MaxBusNameLength` is an **inconsistent per-report backend quirk** (`ShowVoltages`→12,
  `ShowPowers`→~5, even in isolation) — *not* a consistent floor, so the step-4
  floor-12 `TODO(compat)` was withdrawn; instead the comparator **drops pure dot-run
  tokens** (`PadDots` padding carries no data), making the gate immune to the quirk,
  and `max_bus_name_length` keeps the clean source value. `MaxDeviceNameLength=0`
  `TODO(compat)` stands. golden_phase8 **59→74**.
- **WP8.4 (Show reports) — steps 5–6 COMPLETE + audited, gate-green.** Step 6:
  `Show monitor` (`TranslateToCSV`, corpus×24 — reuses the monitor CSV; golden via the
  daily monitor fixture) + `Show Mismatch` (`ShowNodeCurrentSum`, per-node KCL sum).
  golden_phase8 **74→78**. **Both audits ran on steps 5–6** (`263898f`): **one real
  bug found + fixed** — the `Show Result` output filename (`Result.txt` → the spec's
  `Result.csv`, `ShowOptions.pas:432`) + its/`EventLog`'s `GlobalResult` side-effect
  (`write_show_global`). The audit-tests-recommended **`Show Variables` golden
  (generator fixture) surfaced a real Phase-7 port bug**: the generator's `w0` (base
  angular frequency) was 0 pre-dynamics, so the classic `Frequency` state var read 0
  not 60 — fixed by initialising `w0 = TwoPi·base_frequency` at construction
  (`Generator.pas:986`); safe across the full suite (dynamics `InitStateVars`
  overwrites `w0` anyway). Test follow-ups: `Show Mismatch` upgraded from a structural
  smoke test to a **value golden** (pins `Max Current` via the new `ColSel::FromEnd`,
  gates the `Current Sum`/`%error` faer-vs-KLU residuals via `GateSpec::Mask` — those
  are inherently not cross-engine-pinnable); new `show_variables`/`show_result`
  goldens; the powers PF gate doc corrected (`min(kW,kvar)`, not kVA) + tolerance
  tightened `abs 1e-3→2e-4`; a powers code-1 whitespace-variant `TODO(WP8)`
  breadcrumb. **All 19 `Show` goldens converted to exact equality** (`rel=0, abs=0`):
  against a fixed oracle-bytes golden the deterministic Rust output is byte-identical,
  so the prior fuzzy tolerances only hid that — 14 are fully exact, the other 5 gate
  out only the genuinely-non-comparable cells (near-zero faer-vs-KLU cancellation
  residuals V0/V2/I0/I2/I1/`|I|`/kvar, skipped incl. exact-0 via `GateSpec::MinCols`)
  + one real `%10.5f` rounding-boundary straddle (`mismatch` Max Current → the
  `1.1e-5` render floor). **DeltaV deferred** (silent no-op, `TODO(WP8)`):
  `WriteElementDeltaVoltages`' `NodeRef[i+NCond]` cross-terminal read yields 0 rows for
  the delta-primary
  `Transformer.sub` where the oracle writes 3 — the delta-winding node_ref layout
  needs investigation (deltaV is not corpus-used).
- **WP8.4 (Show reports) — step 7 COMPLETE, gate-green** (the diagnostic/matrix
  cluster): `Show Convergence` (`Solution.WriteConvergenceReport` — per-node saved
  error/`|V|`/`Vbase` + Max Error footer, over the existing `error_saved`/
  `vmag_saved`/`node_vbase`/`max_error` solution arrays), `Show Y` (`ShowY` — the
  assembled system Y, lower triangle by columns, `[row,col] = G + jB` `%13.10g`,
  reusing `system_y_csc` + the column-major-lower-triangle order that matches KLU's
  `GetTripletMatrix`), `Show controlqueue` (`ControlQueue.WriteQueue` — the pending
  action queue, drained to the header alone after a converged snapshot; new
  `ControlQueue::queue_rows` accessor), and `Show kvbasemismatch`
  (`ShowkVBaseMismatch` — loads/generators >10% off their bus base, LN/LL forms +
  the per-family header). New `report/show/matrix.rs`; the three diagnostics in
  `report/show/diagnostics.rs`; dispatcher arms 4/26/27/30 in `exec/report.rs`. New
  `format::fpc_sci_w` reproduces FPC `Str(v:width)` (scientific, `width-8` frac
  digits, ≥3-digit exponent) for the convergence `:14` columns. golden_phase8
  **78→83**: `show_{convergence,y,controlqueue,kvbasemismatch}` on solved IEEE13 +
  the synthesized `show_kvbasemismatch_vals` (4 kV-mismatched load/gen elements
  exercising both LN/LL forms + the GENERATOR block). **All 5 are exact equality**
  (`rel=0, abs=0`), incl. the convergence `|V|` column: the preemptive `rel=1e-6`
  "7th-sig printing floor" shipped with step 7 was never exercised — the produced
  file is byte-identical to the oracle golden (faer-vs-KLU voltage gap is orders
  below the 7-sig print step) — so it was tightened back to exact per the
  no-unproven-floors rule; Y's G/B are byte-identical (bit-exact assembled
  Y on the LineCode-based IEEE13). **Remaining unported Show keywords** (all still
  *silent* no-ops, `TODO(WP8)` in `do_show_cmd`): `Controlled` (needs the
  `ControlElementList` accessor), `Meters`/`Generators`/`Zone`/`Overloads`/
  `Unserved`, `FaultStudy`, `Yprim` (needs the active-element surface + its
  non-`CircuitName_` filename), `LineConstants`, `Isolated`/`Loops`/`Topology`
  (CktTree walks), `busflow` (`ShowBusPowers`), `autoadded`/`QueryLog` (headless
  FireOffEditor no-ops), `deltaV` (deferred, above).
  **Both audits ran on step 7.** **audit-code — one real Minor bug found + fixed:**
  `Show Convergence` (arm 4) + `Show controlqueue` (arm 27) were setting
  `@lastshowfile`, but Pascal dispatches them *inline* with only `FireOffEditor`
  (`ShowOptions.pas:187-197`/`404-414`) and does **not** — fixed via
  `write_show_named(…, set_last=false)`, the `write_show` doc corrected, and pinned
  by the new `show_lastshowfile_semantics` test (Y/kvbasemismatch set it;
  Convergence/controlqueue don't). **audit-tests — one real Major gap + closed:**
  `show_controlqueue` only exercised the drained (empty) queue, so the new
  `queue_rows`/row-formatting path shipped uncovered. The row body is **unreachable
  via the executive** — a `show controlqueue` after any `solve` always sees a
  drained queue (probe-proven, incl. the low-level `SolveNoControl`+`Sample` split),
  so no oracle golden can reach it; covered instead by a `control_queue_row_format`
  unit test against the Pascal `WriteQueue` format. The Sec `%-.g` precision (FPC
  empty-precision `ffGeneral`) is unverifiable against the always-drained oracle
  queue → documented `TODO(compat)`, 6-sig stand-in flagged for the WP8.8 byte pass.
  Also refactored `run_feeder_show` to locate the report by its fixed
  `<CaseName_><suffix>` name in the datapath (the oracle-generator glob) — robust to
  the `@lastshowfile` split and still filename-pinning. Tracked-not-fixed (audit
  notes): `show_kvbasemismatch` (plain IEEE13) is near-vacuous but backstopped by
  `_vals`; the `show_convergence` Error column is exact-compared (`rel=0`) —
  honest today (all `0.00000`), a robustness note only.
- **WP8.4 (Show reports) — step 8 COMPLETE, gate-green** (the register tables):
  `Show Meters` (`ShowMeters` → `EMout.txt`, dispatcher arm 9) and `Show Generators`
  (`ShowGenMeters` → `GenMeterOut.txt`, arm 8) — each element's accumulated
  energy-meter registers in Pascal's fixed-width layout: the register-name legend
  (`Reg i = <name>`, meters only), the per-register column header, then one
  `%10.0f`-per-register row per **enabled** element (a disabled element emits only
  its trailing newline — Pascal writes the newline outside the `if Enabled` guard;
  the blank line is stripped by the comparator). New `report/show/meters.rs` reuses
  the WP8.3 register access (`EnergyMeter::register_names`/`registers`, the
  class-fixed `GEN_REGISTER_NAMES`) and the WP8.3 register fixtures
  (`REGISTER_A_POST` metered IEEE13, `REGISTER_B_POST` g1/g2 + disabled g3). golden
  `show_meters`/`show_generators` (golden_phase8 **83→86**), both **exact equality**
  (`rel=0, abs=0`): the register values are the same daily-solved meter/generator
  paths `corpus_live.rs` + `export_meters`/`export_generators` already pin (both
  render `%10.0f`), so every rounded integer cell is byte-identical; g3's absence
  pins the enabled-filter. **Remaining unported Show keywords** (all still *silent*
  no-ops, `TODO(WP8)`): `Controlled`, `Zone`/`Isolated`/`Loops`/`Topology` (CktTree
  walks), `Overloads`/`Unserved`, `FaultStudy`, `Yprim` (needs the active-ckt-element
  surface + its non-`CircuitName_` filename), `LineConstants`, `busflow`
  (`ShowBusPowers`), `autoadded`/`QueryLog` (headless FireOffEditor no-ops), `deltaV`
  (deferred). **Both independent audits ran (`059c2aa`): no correctness bug** — the
  two formatters reproduce `ShowMeters`/`ShowGenMeters` field-for-field (banner/
  legend/header widths, the enabled-filter, the disabled-element blank line, the
  empty-list guards, `%10.0f`), and the dispatcher wiring (filenames/solution-guard/
  `@lastshowfile`) is faithful. Two follow-ups landed (both **test-only**): **(F1)**
  the generator `$` register sits **exactly** on the `%10.0f` half-boundary
  (`7.5 → 8`) — documented at the `show_generators` pin as *stable*, not a
  knife's-edge: it derives from the stiff clean `kWh = 300` (`model=1` holds
  P = 100 kW → ∫P dt = 300 exactly, bit-identical both engines, no faer-vs-KLU
  residual) and `7.5 → 8` under both round-half-to-even and round-half-away, so the
  exact compare cannot straddle; **(F2)** three coverage goldens added —
  `show_meters_multi` (two **partitioned-zone** meters: em1 stops at em2, so the two
  rows carry distinct per-zone registers → pins the legend-emitted-once-from-`meters[0]`
  + per-row-registers path) and `show_meters_none` / `show_generators_none` (the
  empty-list banner branches: `No Energymeter Elements Defined.` / the two-line
  Generators banner). golden_phase8 **86→89**. Left as noted (not a bug): the
  tokenizer is field-width-blind (the whole Show family's known limit, F3). **next =
  continue WP8.4** (Yprim + the active-ckt-element surface, then the overload/fault +
  CktTree families).
- **WP8 goldens exactness audit — ✅ COMPLETE (2026-07-04), gate-green.** All 93
  `compare_export` compares in `golden_phase8.rs` re-measured cell-by-cell against
  their oracle captures (a temporary harness audit mode collecting max deviations
  instead of asserting): **74 are parse-value-identical** → pinned at exact
  equality (`rel=0, abs=0`; the never-exercised `EXPORT_REL`/`EXPORT_ABS`/
  `YMATRIX_REL`/`LOSSES_ABS`-class preemptive print floors deleted, incl. the
  Voltages/8500-Voltages angle 0.11, Powers/SeqPowers 0.11, P_byphase-MVA 0.0011,
  Taps 1e-4, monitors 1e-4/1e-5, registers 0.5, Loads 0.05, reliability 1e-8/1e-9,
  capacity/faultstudy/SeqZ/Y/Yprims/overloads/unserved/sections/profile/
  allocation floors, and the `show_convergence` |V| `rel=1e-6`). The **19**
  non-exact compares all trace to three observed classes, each kept at its
  measured floor: (1) last-digit render straddles — `P_byphase` kVA 1e-3 (kept
  0.0011), `Losses` 7-sig 65.34585↔86 (kept `rel=1e-6`), `show_mismatch` %10.5f
  (kept 1.1e-5); (2) near-zero cancellation residuals — `Losses` noise cells
  ≤1.28e-8 W (`abs` 1e-6→**1e-7**), `SeqVoltages`/`SeqCurrents` residuals (abs
  1e-9/1e-8 kept, ratio-cell abs →**1e-9**, `rel` 1e-4→**0**), `Currents`/
  `ElemCurrents`/`ElemPowers`/`YCurrents` (`abs` 1e-6→**1e-8/1e-10/1e-10/1e-13**,
  `rel`→0; noise-angle `PrevCol` gates kept, `ElemVoltages` gates dropped —
  byte-exact); (3) the DI files — the only genuine non-print floor, the per-step
  faer-vs-KLU diff integrated over 24 daily solves (measured 1.5e-8 rel):
  `rel` 1e-6→**5e-8**, `abs` 1e-8→0. Gates kept only where observed firing on
  noise (show_losses/voltages/currents/currents_elem/powers_elem, seqcurrents I1,
  ang_tol PrevCol, the two Summary DateTime Masks + show_mismatch residual Masks);
  no-op/unfired ColTols removed. `tests/TOLERANCE_NOTES.md` WP8 sections rewritten
  to the exact-by-default regime. golden_phase8 84/84 green.

**Phase 7 COMPLETE** (WP7.1–WP7.10, branch `phase-7-extended-elements`,
gate-green) but **NOT merged to `main`** (per-phase merge = explicit-request-only
HARD STOP; `phase-8-reporting` builds on top of it). Roll-up + archives in **§1e**
([`docs/phase-records/phase-7.md`](docs/phase-records/phase-7.md) +
`phase-7-wp{1..7}.md`). Tracked-open Phase-7 deferrals (both Plot-blocked, zero
corpus payoff): the **GFM grid-forming inverter mode** and the **Generic/TD21
relay `Sample`**. Current scores: dss-core **lib 732**, **`solvable_now` 168**;
oracle pinned to dss-python 0.15.7 (backend = dss_capi 0.14.5,
`tools/golden/PIN.txt`).

Phase 7 = DER, protection, line constants, harmonics, dynamics (PORTING_PLAN.md
§Phase 7, the largest phase ~18%). Earlier phases merged to `main` (newest first):
**Phase 6** (WP6.1–WP6.10 — meters/monitors/topology/Generator + the 8500-node gate
+ the live corpus gate; `--no-ff` `b98223a`, `main` not pushed to origin)
→ [record](docs/phase-records/phase-6.md); **Phase 5** (`10d3550`), **Phase 4**
(`5f27a25`). Their full logs and the per-WP detail live under `docs/phase-records/`
(§1b–1d indexes them) and the §1 table below.

**Standing toolchain note:** the gate runs on **`stable`** (`cargo +stable …`),
matching CI (`dtolnay/rust-toolchain@stable`) — no nightly dependency. `dss-core`
carries `#![allow(clippy::collapsible_match)]` (`d85d026`): clippy 0.1.96 (now on
stable) mis-fires that lint on the byte-faithful `match prop { CONST => if cond
{..} }` port idiom, and its autofix even drops `else` branches.

> **Working cadence:** finish one small step → run the full gate → update this
> file → **stop and wait for explicit user confirmation** before the next step.
> The full per-step ritual (gate, STATUS sync, the two audits) is
> **`PHASE7_PLAN.md §0`**, run per **§1e**. (Earlier phases sometimes executed
> several WPs in one pass on explicit user instruction.)

---

## 1. Where we are

| Phase | Scope | Status |
|------|-------|--------|
| 0 | Tooling, oracle, faer spike, CI, Phase-0 goldens | ✅ done (committed) |
| 1 | Shared math (`support/`) + full `TDSSParser` port | ✅ done (commit `729eb77`) |
| 2 | Object model, property engine, executive skeleton | ✅ done (commit `22f861d`) |
| 3 | ★ Vertical slice: parse → circuit → Y matrix → solve → voltages | ✅ done (commit `2ac8691`) |
| **4** | **Transformer/Capacitor/Reactor/LineCode + controls (parse-only) + macro + feeder gate** | ✅ done (merged to main, `5f27a25`); `PHASE4_PLAN.md` |
| **5** | **LoadShape/XYcurve/controls behavior, control queue, time modes + feeder gate (controls active)** | ✅ done (merged to main, `10d3550`); `PHASE5_PLAN.md` |
| **6** | **Meters/Monitors/topology/Generator + 8500-node gate + live corpus gate** | ✅ done (merged to main, `b98223a`); `PHASE6_PLAN.md` |
| 7 | Extended elements: DER, protection, line constants, harmonics, dynamics | ✅ **COMPLETE** (WP7.1–WP7.10) — `PHASE7_PLAN.md`; branch `phase-7-extended-elements`, gate-green, **NOT merged to `main`** (explicit-request-only HARD STOP). WP7.1–7.6 (line constants, protection, DER, harmonics), WP7.7 (Dynamics core), WP7.8 (Converter/FACTS), WP7.9 (FaultStudy + AutoAdd/Feeder-deferred), WP7.10 (phase exit). Tracked-open deferrals: GFM grid-forming mode + Generic/TD21 relay `Sample` (both Plot-blocked, 0 corpus payoff). Per-step detail in §1e + `docs/phase-records/phase-7-wp{1..6}.md` |
| **8** | **Reporting: Export/Show/Save/Dump + executive tail + full ReduceAlgs** | 🚧 **IN PROGRESS** — `PHASE8_PLAN.md`. **WP8.1 COMPLETE, gate-green** (dispatch skeleton + GUI no-ops `82b50fe`; output-path machinery + `Export Counts` + the `compare_export` golden harness `929145c`). **WP8.2 COMPLETE, gate-green:** sub-step 1 (`71067f7`) = bus/node solution exports; sub-step 2a (`668bd18`) = the aggregate PD/PC power exports `Powers`/`Losses`/`P_byphase` + the mutable element-walk infra + the MVA/kVA `Parm2` pre-parse; **sub-step 2b** = the symmetrical-component family `SeqVoltages`/`SeqCurrents`/`SeqPowers` + the `ColTol::gate` denominator-gate harness machinery; **sub-step 2c** = the per-terminal/per-conductor element exports `Currents`/`NodeOrder`/`ElemCurrents`/`ElemVoltages`/`ElemPowers`/`Taps` + the `ColSel` name-prefix\|index-parity harness refactor + the `ElemPowers` Vsource order fix; **sub-step 3** = the matrix/summary exports `Yprims`/`Y`/`SeqZ`/`Summary`/`Result` + the `Y` triplet Parm2 flag + the `Summary` append/`DateTime`-mask (`ColSel::Index`+`GateSpec::Mask`) + the `SeqZ`-faultstudy fixture + the PM-build-faithful always-`null` `Result`; **completion gate** = the IEEE8500 `Voltages`/`Summary`/`Counts` goldens (`run_shared_exports`) + the `Export`-unblocked corpus migration (`solvable_now` **88→119**, COVERAGE **26.3%→35.5%**) + the Rust `CorpusGuard` (corpus stays pristine under report-writing decks). The "9 decks hang" tracked-open is **RESOLVED — no hang** (all complete + converge; watchdog artifact; stale tags refreshed, see §1f). Branch `phase-8-reporting`. **WP8.3 COMPLETE (steps 1–5 + both audit follow-ups), gate-green** — the device/meter/reliability/log exports (`Monitors`/`Meters`/DER/`EventLog`/`Faultstudy`/`BusReliability`…/`Sections`/`Profile`) + the `TSystemMeter` core and the full demand-interval (`DI_*`) file machinery + its `Set`/`Set year=` wiring (§2.6); the completion gate migrated `solvable_now` **119→168** (COVERAGE **50.1%**, incl. the silent `Spectrum.CSVFile` no-op fix). Both independent audits found **no correctness bug**; 2 LOW code findings fixed (the `Export Profile` `1732.0` `TODO(compat)` marker; the `Spectrum.read_csv_file` byte-position EOF guard, oracle-confirmed) + 5 oracle-pinned coverage tests (golden_phase8 **59**; lib **731**). **WP8.4 (Show) step 1 COMPLETE, gate-green** — the `do_show_cmd` dispatcher + `report/show/` (`Show Buses`/`Losses`/`Taps` + `Show panel`→#999; unported keywords stay silent no-ops) + the `format.rs` `Pad`/`PadDots`/width formatters + the whitespace+comma `compare_export` tokenizer (golden_phase8 **59→62**). **WP8.4 step 2 COMPLETE, gate-green** — `Show Voltages` code 0 (`WriteSeqVoltages`, the seq V1/V2/V0 + %ratios form, the bare `Show Voltage`/`v` default) + the ptr-13 LL/node/elem option parse (codes 1/2 angle-forms deferred, `TODO(WP8)`); golden_phase8 **62→63**. **WP8.4 step 3 COMPLETE, gate-green** — `Show Currents`/`Powers` code 0 (seq forms, ptr-3/12 option parse); the empirically-zero `MaxDeviceNameLength` reproduced (`TODO(compat)`, probe-proven); the `%I2/I1` ratio gate at the proven `1e-6 A` floor; `gen_phase8.py DSS.AllowEditor=False` (no Notepad spawn); golden_phase8 **63→65**. **WP8.4 step 4 COMPLETE, gate-green** — the element/node forms `Show Voltages` code 1/2 (`WriteBusVoltages`/`WriteElementVoltages`) + `Show Currents` code 1 (`WriteTerminalCurrents`, +residual) + `Show Elements` (`ShowElements`/`WriteElementRecord`, two-file main+`_Disabled`); the `SetMaxBusNameLength` floor-12 backend divergence reproduced (`TODO(compat)`, probe-proven `max(12,longest)`); golden_phase8 **65→69**. Both audits clean (no correctness bug); follow-ups: disabled-element blank line (`WriteElementVoltages`, byte-probed), floor-12 header/data refinement, angle-floor `ColSel::AfterToken` selector, currents-elem gate `1e-6→1e-4`, +2 coverage goldens (class-filter, LL node), + a step-4 test-tree-leak fix (`show_reports_are_silent_noops` datapath); golden_phase8 **69→71**. **Steps 5–6 COMPLETE + audited** — `Powers` elem + `Result`/`EventLog`/`Ratings`/`Variables`/`Mismatch`/`monitor`; the step-4 floor-12 `TODO(compat)` **withdrawn** (`MaxBusNameLength` is an inconsistent per-report backend quirk → comparator drops pure dot-runs); harness `ColSel::AfterToken`/`FromEnd` + `GateSpec::MinCols`; **real bugs found+fixed** (`Show Result` filename `.txt→.csv`; generator `w0` snapshot-`Frequency`); golden_phase8 **71→78**. **Step 7 COMPLETE, gate-green** (the diagnostic/matrix cluster) — `Show Convergence`/`Y`/`controlqueue`/`kvbasemismatch` (`report/show/matrix.rs` + the three diagnostics; dispatcher arms 4/26/27/30; new `format::fpc_sci_w` FPC-`Str(v:width)`; `ControlQueue::queue_rows`); golden_phase8 **78→83** (all exact equality except the convergence `|V|` 7-sig printing floor). **Step 8 COMPLETE, gate-green** — the register tables `Show Meters` (`ShowMeters`→`EMout.txt`) + `Show Generators` (`ShowGenMeters`→`GenMeterOut.txt`), reusing the WP8.3 register fixtures (`%10.0f` integer registers, exact equality); **both audits clean** (no correctness bug) + 3 coverage goldens (multi-meter + the two empty-list banners) + the `$`=7.5 half-boundary documented; golden_phase8 **83→89**. **~10 `Show` reports remain** (silent no-ops, `TODO(WP8)`). **next = continue WP8.4** (Yprim/topology/CktTree Shows). Detail in §1f |

### Gate state (all green)
```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace      # dss-core lib 732, golden_feeders 1,
                            # golden_feeders_controls 4, golden_phase5 1,
                            # golden_phase6 1, golden_phase7 1,
                            # golden_phase7_protection 1, golden_phase8 89,
                            # golden_checkpoints 1, golden_ieee8500 1,
                            # golden_reliability 1, golden_allocation 1,
                            # golden_gendispatcher 1, golden_autoadd_reduce 1,
                            # golden_slice 2, golden_smoke 3, props_roundtrip 1,
                            # corpus_manifest 1, corpus_live 3 (168 solvable_now
                            #   cases live-compared)
                            #   (corpus_live_solvable_cases_match_oracle +
                            #    solvable_now_has_multistep_depth run
                            #    UNCONDITIONALLY — the pinned oracle MUST be
                            #    installed (it fails, not skips, without it);
                            #    only corpus_live_classify is opt-in, via
                            #    DSS_LIVE_CLASSIFY=1 — the growth/classify probe),
                            # dss-parser 62+1, dss-sparse 5
```

### Phase 5 gate — green  *(detail → `docs/phase-records/phase-5.md`)*
- `golden_feeders_controls.rs`: the unmodified IEEE13/IEEE37/IEEE123 masters
  (controls active) + `ieee34mod1` match the Phase-0 goldens — converged + total
  iterations exact, `YNodeOrder` exact, RegControl `tap_number` / capacitor
  `states` exact, final taps 1e-12 rel (the integer `tap_number` is the exact
  discrete check), V/I/P 1e-6, and every element's full property dump.
- `golden_phase5.rs` vs `tests/golden/phase5/*.json` (`gen_phase5.py`):
  `daily_ieee13`, `duty_2bus`, `eventlog_ieee13`, `capcontrol_micro` — per-step
  `dblHour` + iteration counts exact, **event logs line-for-line** (normalized),
  per-step V 1e-6 (the shape-scaled `Yeq` restamp per Y build, `a6903f1`).

### Checkpointed-model gate (`crates/dss-core/tests/golden_checkpoints.rs`) — green
- `gen_checkpoints.py` → `tests/golden/checkpoints/<scenario>.json` (schema 2,
  one file per scenario; the gate runs every file in the directory, so adding a
  scenario is just adding a file). Unlike the
  other command-replay gates (which compare only converged outputs), this one
  captures the **assembled electrical model after every committed time step** —
  the unfactored system Y, selected element YPrim blocks, the injection vector,
  node voltages, and discrete control state — and compares each to the oracle.
  A stale Y/YPrim fails at the step and matrix entry it first goes wrong, not as
  downstream register drift. Scenarios: `micro_yeq_steps` (control-free daily,
  full-CSC per-step pin), `ieee13_daily` (24-step daily with regulator tap
  changes — full CSC + fingerprint; the direct regression guard for the
  "frozen load Yeq" bug: reverting commit `a6903f1` makes it fail at step 6,
  `Y[634.1]`), `ieee123_snap` (large-feeder fingerprint-only + selected YPrim
  path). Tolerances: `tests/TOLERANCE_NOTES.md`. The assembled Y is compared
  **unfactored** so the `dss-sparse` row equilibration is out of scope.

### Live corpus oracle gate (`crates/dss-core/tests/corpus_live.rs`) — opt-in
See `CORPUS_TEST_PLAN.md`. The whole `electricdss-tst` corpus is **vendored** into
`tests/corpus/electricdss-tst/` (1544 files, 122 MiB; `tools/corpus/vendor.py`,
`.git` excluded, with `SHA256SUMS` + `README.md` provenance) so tests no longer
depend on the temporary `.inputs/electricdss-tst`.
- **Manifest accounting (always-on).** Every `.dss` (915) is in exactly one
  manifest under `tests/corpus/manifests/` (`solvable_now`, `skipped_unsupported`,
  `skipped_oracle_issue`, `skipped_needs_investigation`, `missing_dependency`,
  `not_an_entry_point`). `corpus_manifest.rs` enforces the bijection — no silent
  omissions — and runs in the normal `cargo test`: adding/removing a `.dss` fails
  it until the file is classified.
- **Live comparison (runs unconditionally in `cargo test`; the pinned oracle must
  be installed).** For each of the **84** `solvable_now` cases the gate
  compiles+solves on the Rust engine and on the pinned dss-python oracle
  (`tools/oracle/oracle_server.py`, a
  one-shot subprocess over JSON), and compares the full assembled model per step —
  node order, **full** system Y (entry-by-entry, no fingerprint substitution),
  node voltages, **every** element's currents/powers, selected YPrim blocks (a
  guard fails the case if the oracle returns no YPrim for a named selected
  element), the injection vector, and discrete state — reusing the `harness/mod.rs`
  comparators and the checkpoint gate's tolerance policy.
  - **Three control-diverse 24-step daily runs** — `IEEE13Nodeckt` (wye gang
    reg), `ieee37` (delta, open-delta LDC reg bank) and `IEEE123Master` (multiple
    cascaded reg banks) — each with a meter + three monitors (modes 0/1/2) +
    selected elements, so the **multi-step per-step**, **YPrim**,
    **monitor-channel** and **EnergyMeter-register/zone** paths are all exercised
    live (`compare_monitor`/`compare_meter`, the *same* comparators
    `golden_phase6.rs` now routes through, gated per case by
    `check_meters_monitors`). Incidental master-defined monitors are *not*
    compared — the pinned oracle returns a phantom `Channel(i)` for an unsampled
    monitor (see `tests/TOLERANCE_NOTES.md`).
  - The **IEEE 8500-Node master is promoted** (snapshot; `post: Set
    Maxiterations=20` to converge — the bare probe didn't, which is why the
    classifier had parked it), so the full 8531-node Y, every element's I/P, and a
    YPrim block are compared live at scale (complementing the always-on
    `golden_ieee8500.rs` golden, whose `compare_discrete` also pins the full
    1190-transformer tap set here).
  - **Depth is guarded always-on.** `solvable_now_has_multistep_depth` (no oracle)
    asserts `solvable_now` keeps ≥1 multi-step `check_meters_monitors` case and ≥1
    case with selected elements, so the deep coverage can't silently revert to
    snapshots. The solvable + classify tests **auto-skip (pass)** without the env
    var / oracle, so `cargo test --workspace` stays green everywhere; the
    **`live-oracle` GitHub Actions job** installs the pinned oracle (PIN.txt) and
    runs the **whole `corpus_live` binary** (not a name filter that could green on
    zero matched tests). The oracle server hard-asserts **both** dss-python 0.15.7
    **and** engine 0.14.5 (PIN.txt). No goldens are written; the oracle is
    consulted live.
- **Growth.** `DSS_LIVE_CLASSIFY=1 corpus_live_classify` probes the
  `skipped_needs_investigation` candidates with the full comparison and writes
  `tmp/classify_report.json`; `tools/corpus/apply_classify.py` promotes the
  passing cases into `solvable_now` (and routes oracle/engine failures to the
  right skip bucket). `tools/corpus/coverage_report.py` →
  `tests/corpus/COVERAGE.md` tracks the burn-down toward 100% of entry points.

### Phase 4 gate (`golden_feeders.rs`) — green  *(detail → `docs/phase-records/phase-4.md`)*
The controls-off IEEE13/37/123 variants (`gen_phase4.py`) match `phase4.json`
(pinned oracle): converged + iterations exact (3/3/3), `YNodeOrder` exact
(41/117/278), V 1e-6, every element's I/P 1e-6 (creation order), total
power/losses 1e-6. The Phase-3 `golden_slice.rs` (13 scenarios) stays green; the
CLI runs the real masters (`cargo run -p dss-cli -- script.dss`).

---

## 1b–1d. Completed-phase records (archived)

The full work-package logs for the completed, merged phases (and the completed
Phase-7/Phase-8 work packages) live under `docs/phase-records/` to keep this
handoff lean. They are frozen history, superseded only by the code and tests.
The two roll-ups that back the compact §1e/§1f frontier below:
[`phase-7.md`](docs/phase-records/phase-7.md) (the WP7.1–7.10 record) and
[`phase-8.md`](docs/phase-records/phase-8.md) (the completed WP8.1/8.2/8.3-step
detail). The per-phase entries:

- **Phase 3** — the vertical-slice file-by-file map (circuit model / element base /
  solution / executive / property engine) — still the architectural reference §2
  points to. → [`docs/phase-records/phase-3.md`](docs/phase-records/phase-3.md)
- **Phase 4** — PD elements (Transformer/Capacitor/Reactor), catalog objects
  (LineCode/XfmrCode/GrowthShape), the Line→LineCode fetch path, parse-only
  RegControl/CapControl, the `define_properties!` macro, and the controls-off
  feeder gate. → [`docs/phase-records/phase-4.md`](docs/phase-records/phase-4.md)
- **Phase 5** — controls + time series: XYcurve / LoadShape / TShape /
  PriceShape, ControlQueue + event log, RegControl/CapControl behavior, the
  control loop (`Sample_DoControlActions`), and the time-series solve modes.
  → [`docs/phase-records/phase-5.md`](docs/phase-records/phase-5.md)
- **Phase 6** — meters + topology: CktTree, Generator, Monitor, EnergyMeter +
  zone build, registers/TakeSample, reliability (`RelCalc`), Sensor + load
  allocation, the GenDispatcher/StorageController/AutoAdd/ReduceAlgs skeletons,
  and the 8500-node gate. Merged to `main` `b98223a`.
  → [`docs/phase-records/phase-6.md`](docs/phase-records/phase-6.md)
- **Phase 7 WP7.1** (Line constants & geometry) — the Carson engine, the
  WireData/CNData/TSData/LineSpacing/LineGeometry catalog, Line's geometry/spacing
  Carson path, the corpus migration, and the offline geometry golden. **Complete +
  gate-green on the `phase-7-extended-elements` branch (not yet merged);** the live
  §1e keeps a step summary + the tracked-open plural-cable note.
  → [`docs/phase-records/phase-7-wp1.md`](docs/phase-records/phase-7-wp1.md)
- **Phase 7 WP7.2** (Protection) — Fault, SwtControl, Fuse, Recloser, Relay (9
  sub-types), reliability activation (`HasOCPDevice` + live `RelCalc`), and the
  step-4 gate (the `phase7_protection` trip/reclose golden, the `Open`/`Close` exec
  verbs, the SwtControl corpus migration). **Complete + gate-green on the
  `phase-7-extended-elements` branch (not yet merged);** the live §1e keeps a
  per-step summary + the Phase-7 carry-forward rules + the `DG_Prot_Fdr` tracked-open.
  → [`docs/phase-records/phase-7-wp2.md`](docs/phase-records/phase-7-wp2.md)
- **Phase 7 WP7.3** (DER A) — `DynamicExp` (the diff-eq catalog object + its RPN
  expression interpreter), `InvBasedPceData` (the shared inverter PC-element base),
  and `PVSystem` (the power-flow PV element + zone admission + the Monitor mode-3
  fix + the GFM loud-abort). **Complete + gate-green on the branch (not yet merged).**
  → [`docs/phase-records/phase-7-wp3.md`](docs/phase-records/phase-7-wp3.md)
- **Phase 7 WP7.4** (DER B) — the `Storage` element (the charge/idle/discharge state
  machine + integrated SOC) and the real `StorageController` fleet/dispatch (replacing
  the WP6.8 skeleton), plus the Storage-specific YPrim-rebuild fix. **Complete +
  gate-green on the branch (not yet merged).**
  → [`docs/phase-records/phase-7-wp4.md`](docs/phase-records/phase-7-wp4.md)
- **Phase 7 WP7.5** (DER C, steps 1–4) — `RollAvgWindow`, the full `InvControl` (8
  modes + LPF/RiseFall + MonBus, both PVSystem and Storage DERs), `ExpControl`
  (the adaptive-`Vreg` volt-var control), and the step-4 corpus burn-down review.
  **COMPLETE + gate-green on the branch (not yet merged).**
  → [`docs/phase-records/phase-7-wp5.md`](docs/phase-records/phase-7-wp5.md)
- **Phase 7 WP7.6** (Harmonics, steps 1–3) — the harmonics solve mode: the
  current-source family (VSource + Load) + the `SolveHarmonic`/`SolveHarmonicT`
  driver, the Thevenin DER family (Generator/PVSystem/Storage behind their
  subtransient reactance), and the monitor harmonic header + the `Set mode=`
  monitor/meter reset (Pascal `Set_Mode` tail); harmonics corpus burn-down is 0
  migratable (Phase-8/`Isource`/FaultStudy-blocked). **COMPLETE + gate-green on the
  branch (not yet merged).**
  → [`docs/phase-records/phase-7-wp6.md`](docs/phase-records/phase-7-wp6.md)
- **Phase 7 WP7.7** (Dynamics core, steps 1–3b cont.) — the `SolveDynamic`
  predictor/corrector driver + per-element dynamics state machinery for Generator /
  PVSystem / Storage / IndMach012, Monitor mode 3, the `Open`-verb fix, the
  `set_ITerminalUpdated` stamp sweep, and the DynEqPCE user-`DynamicExp` integration
  for all three PCE families. **✅ COMPLETE** (steps 1–4; GFM deferred,
  tracked-open). The completed-step detail (incl. all audit follow-ups) is archived;
  the live §1e keeps the concise per-step summary.
  → [`docs/phase-records/phase-7-wp7.md`](docs/phase-records/phase-7-wp7.md)

---

## 1e. Phase 7 record (branch `phase-7-extended-elements`) — ✅ COMPLETE

The full per-WP roll-up (WP7.1–WP7.10 step summaries, decisions, audits, gate
detail, and the cross-cutting carry-forward rules) is **archived** at
[`docs/phase-records/phase-7.md`](docs/phase-records/phase-7.md); the deeper
per-step detail lives in the sibling `phase-7-wp{1..7}.md` archives. Phase 7 is
COMPLETE + gate-green on the branch, **NOT merged to `main`** (per-phase merge =
explicit-request-only HARD STOP). Headline: **WP7.1** line constants & geometry,
**WP7.2** protection (Fault/Fuse/Recloser/Relay/SwtControl + reliability
activation), **WP7.3–7.5** DER (DynamicExp/InvBasedPCE/PVSystem;
Storage/StorageController; InvControl/ExpControl), **WP7.6** Harmonics, **WP7.7**
Dynamics core (SolveDynamic + Generator/PVSystem/Storage/IndMach012 + DynEqPCE),
**WP7.8** Converter/FACTS (VSConverter/VCCS/UPFC+UPFCControl/ESPVLControl),
**WP7.9** FaultStudy (AutoAdd/Monte/LD/Feeder empirically deferred — zero corpus
cases), **WP7.10** exit. Retro audit (WP7.7 step 4 → WP7.8): no Critical/Major
correctness bug. Two real port bugs found+fixed in WP7.5 (the cross-step
`FFlagVWOperates` latch + the missing post-`DoPendingAction` `LoadsNeedUpdating`),
each a [[dont-rationalize-conditioning]] instance. **Tracked-open** (both
Plot-blocked, zero corpus payoff): GFM grid-forming mode + Generic/TD21 relay
`Sample`. lib **713**; `solvable_now` **88** at Phase-7 exit.

---

## 1f. Phase 8 record (`PHASE8_PLAN.md`) — 🚧 IN PROGRESS

Execution plan: **`PHASE8_PLAN.md`** (WP8.1–WP8.8, the reporting/output + full
executive layer; per-step cadence = `PHASE8_PLAN.md §0`). Phase 8 is almost
entirely *read-and-format* — no new electrical math, no new solve mode; the risk
is faithful report layout and **not silently faking output**. The full per-step
log for the **completed** work (decisions, audits, gate detail, the real-gap
write-ups) is **archived** at
[`docs/phase-records/phase-8.md`](docs/phase-records/phase-8.md). Live frontier:

- **WP8.1 (Report infrastructure) — ✅ COMPLETE, gate-green** (`82b50fe` dispatch
  skeleton + GUI/`Plot`/`Visualize` headless no-ops with the #301 pre-circuit
  guard; `929145c` output-path machinery + `Set DataPath=` + `Export Counts`
  end-to-end + the `compare_export` golden harness). The `Show`-silent vs
  `Export/Save/Dump`-loud-`NOT_PORTED` asymmetry is forced + proven-safe.

- **WP8.2 (Export: solution outputs) — ✅ COMPLETE, gate-green.** The full export
  families, read-only/mutating over the solved circuit: bus/node (`Voltages`/
  `BusCoords`/`NodeNames`/`YNodeList`), aggregate power (`Powers`/`Losses`/
  `P_byphase` + the `for_each_enabled_elem`/`export_with_mut` mutable element-walk
  infra + the MVA `Parm2` pre-parse), symmetrical-component (`SeqVoltages`/
  `SeqCurrents`/`SeqPowers` + the `ColTol::gate` denominator gate), per-terminal
  (`Currents`/`NodeOrder`/`ElemCurrents`/`ElemVoltages`/`ElemPowers`/`Taps` + the
  `ColSel` harness refactor), and matrix/summary (`Yprims`/`Y`/`SeqZ`/`Summary`/
  `Result`). **Completion gate:** the IEEE8500 `Voltages`/`Summary`/`Counts`
  goldens + the `Export`-unblocked corpus migration (`solvable_now` **88→119**,
  COVERAGE **26.3%→35.5%**) + the Rust `CorpusGuard` (corpus stays pristine under
  report-writing decks). Two tracked-opens **RESOLVED** here (detail in the
  archive): the "9 decks hang" was a debug-build watchdog artifact (all 9 converge,
  ≤3.4s release), and the rare live-gate flake was a per-process convergence misfire
  *inside the pinned oracle* (now retried in-process) — neither a Rust bug.

- **WP8.3 (Export: device/reliability + logs) — ✅ COMPLETE (steps 1–5 + both
  audit follow-ups), gate-green.** The device/meter/reliability/log exports over
  the solved circuit + the demand-interval file machinery: `Monitors` (step 1);
  the `Meters`/`Generators`/`Loads`/`PVSystem_Meters`/`Storage_Meters` register
  dumps (step 2, which surfaced + fixed the never-wired DER `SampleAll`/`ResetAll`
  solve-loop tail); `EventLog`/`ErrorLog` (step 3a, + the missing circuit-build
  `LogThisEvent` markers); `Faultstudy` (step 3b, read-only over the WP7.9
  `Ysc`/`BusCurrent`); `BusReliability`/`BranchReliability`/`Capacity`/`Overloads`/
  `Unserved`/`AllocationFactors`/`Sections`/`Profile` (step 3c, over the `RelCalc`
  fields — incl. the meter `SectionCount`/`FeederSections` persistence, the seven
  `Profile` `PhasesToPlot` branches, and the multi-meter `Bus_Int_Duration`
  cross-zone bug — filed + gated, see below); the `TSystemMeter` core + the full demand-interval
  (`DI_*`/phase-voltage/overload/volt-exception) writers + their `Set` handlers +
  the `Set year=` `Set_Year` side effects + the Solve*/DI open-close wiring
  (step 4, §2.6); and the completion gate (step 5) — 47 probe-clean decks migrated
  + the **silent `Spectrum.CSVFile` no-op** fixed → 2 IEEE_519 harmonicT decks
  (`solvable_now` **119→168**, COVERAGE **50.1%**). The two independent audits
  (`bacaf13` code, `8d58a8e` tests) found **no correctness bug**; two LOW code
  findings fixed (the `Export Profile` `1732.0` truncated-√3 `TODO(compat)` marker;
  the `Spectrum.read_csv_file` byte-position `(F.Position+1) < F.Size` EOF guard,
  an oracle-confirmed divergence over `str::lines()`), and five oracle-pinned tests
  closed the coverage gaps (`Set year=` lifecycle, the five `Get` DI echoes, the
  `NPhases<3` overload I2-column mapping, the Spectrum-`FileLoad` deck round-trip,
  and — after the user flagged the parked multi-meter `Bus_Int_Duration` note — the
  cross-zone reliability contamination, now **empirically settled**: a new upstream
  bug report (`investigations/reliability_bus_int_duration_oob_bug_report.md`), the
  in-range regime gated by `export_busreliability_multimeter`, the out-of-range OOB
  proven-nondeterministic across processes). Full per-step + audit detail in
  [`docs/phase-records/phase-8.md`](docs/phase-records/phase-8.md).
  golden_phase8 **59**; lib **731**; `solvable_now` **168**.
- **WP8.4 (Show reports) — step 1 COMPLETE, gate-green.** `do_show_cmd` is now a
  real dispatcher (option/solve-guard per `ShowOptions.pas`), routing the first
  ported keywords to fixed-width text formatters in the new `report/show/` module:
  `Show Buses`/`Losses`/`Taps` + `Show panel`→#999. Unported `Show` keywords stay a
  *silent* headless no-op (the `solvable_now` `Show Power`/`Voltage`/… decks don't
  regress). Shared machinery: `format.rs` `Pad`/`PadDots`/`EncloseQuotes` +
  width-aware `%W.Df`/`%Wd`/`%W.Pg` formatters, the `@lastshowfile`/`last_show_file`
  bookkeeping, and a new **whitespace+comma tokenizer** in `harness::compare_export`
  (`sep: ' '`, `header_lines: 0`) that diffs the fixed-width tables token-for-token.
  golden_phase8 **59→62** (`gen_show_reports` + `show_{buses,losses,taps}`). The two
  independent audits found **no correctness bug** (the three formatters reproduce
  `ShowBuses`/`ShowLosses`/`ShowRegulatorTaps` field-for-field; the goldens are
  genuine pinned-oracle bytes). Three follow-ups fixed: (audit-tests) the
  `show_losses` policy split into per-column floors — the coarse `%8.2f` `% of Power`
  floor (`abs=0.011`) isolated to token index 2 via `col_tol`, kW/kvar held to the
  tight default (so a small-cell formatting regression fails); (audit-code) the
  deferred `Show` keywords + the deferred unknown→#24700 now carry a greppable
  `TODO(WP8)` tag (the WP8.8 exit sweep) instead of prose-only — the deferral stays a
  *silent* no-op by design (erroring would regress the live `Show Power`/`Voltage`
  decks); and the `Pad`/`max_*_name_length` width helpers switched to byte length
  (`str::len`) for byte-1:1 with Pascal `Length(AnsiString)`.
- **WP8.4 (Show reports) — step 2 COMPLETE, gate-green.** `Show Voltages` code 0
  (Pascal `ShowVoltages` case 0 + `WriteSeqVoltages`): the symmetrical-component
  voltages by bus — V1 (kV)/pu/V2/%V2·V1⁻¹/V0/%V0·V1⁻¹, the bare `Show Voltage`/`v`
  default (82 live decks). The dispatcher's ptr-13 arm ports the `ShowOptions.pas`
  option parse (first param `LL`→phase-phase file `VLL`, else `VLN`; second param
  `N`/`E`→the node/element form). The angle-bearing node/element forms
  (`ShowOptionCode` 1/2) stay a silent no-op with a greppable `TODO(WP8)`. New
  `report/show/voltages.rs` reuses the `SymComp`/`bus.find` helpers; note Show's
  `<3`-node V1 = `|V|` of the first node **unconditionally** (Pascal
  `WriteSeqVoltages`, unlike `ExportSeqVoltages`' `PositiveSequence` gate). golden
  `show_voltages` (golden_phase8 **62→63**). Both independent audits found **no
  correctness bug** (the `<3`-node unconditional-first-node V1 trap is handled
  right, and the ptr-13 LL/N/E parse + code-1/2 silent no-op match `ShowOptions.pas`
  exactly). One audit-tests LOW fixed: the `show_voltages` golden abs tightened
  **1e-5 → 1e-8** (the proven faer-vs-KLU floor is 1e-12 — one `sourcebus` V0 cell;
  every significant cell is bit-identical), so a real small-cell error can no longer
  hide under the old blanket 1e-5.
- **WP8.4 (Show reports) — step 3 COMPLETE, gate-green.** `Show Currents` +
  `Show Powers` code 0 (Pascal `ShowCurrents`/`ShowPowers` case 0 +
  `WriteSeqCurrents`/`GetI0I1I2`): the per-element sequence currents (I1/I2/%I2·I1⁻¹
  /I0/%I0·I1⁻¹/%Normal/%Emergency, with `Cmax`-based ratings, the CAP exclusion, and
  the unconditional `<3`-phase I1) and sequence powers (P1/Q1/P2/Q2/P0/Q0 + PD
  terminal-1 excess + the total-loss footer). The ptr-3/ptr-12 dispatcher arms port
  the `ShowOptions.pas` residual/`m`/`e` option+filename parse (`Curr_Seq`,
  `Power_seq_{kVA|MVA}`); the element forms (code 1) stay a `TODO(WP8)` no-op. golden
  `show_currents`/`show_powers` (golden_phase8 **63→65**). **Two findings settled
  during the step:** (1) the oracle's `SetMaxDeviceNameLength` is empirically **0**
  in the pinned dss_capi 0.14.5 (device-name-independent — probe-proven; the vendored
  source would give 16), so the `Paddots` device-name column is never padded —
  reproduced 1:1 with a `TODO(compat)` (`max_device_name_length → 0`), which also
  makes the step-1 `Show Losses` names byte-faithful; (2) the `%I2/I1`/`%I0/I1` ratio
  gate threshold is **1e-6 A** — provably between the one noise row (a switch's
  floating terminal, `I1 ≈ 1.8e-12 A`) and the smallest *real* current (`Line.671680`,
  `5.8e-4 A`, whose ratio IS checked); an 8-order gap, so `1e-6` never gates a physical
  current (a coarser `1e-3` would wrongly skip the real row). Also fixed a
  **usability bug**: `gen_phase8.py` now sets `DSS.AllowEditor = False` so
  regenerating the `Show` goldens no longer spawns a Notepad per report. Both
  independent audits ran: **no correctness bug** (the seq math, `Cmax` ratings, CAP
  exclusion, unconditional `<3`-phase I1, `×0.003` power scaling, the footer-loss
  walk, and the `mdnl=0` `TODO(compat)` are all faithful; the `1e-6 A` gate proven
  to skip only the one `1.8e-12 A` noise cell). Two Minor **byte-faithfulness** code
  findings fixed (the numeric comparator masked both): the currents `%s %3d` literal
  space between name and terminal, and the powers footer `%6.1f` field width (was
  widthless); plus the continuation-label width switched to the un-uppercased byte
  length.
- **WP8.4 (Show reports) — step 4 COMPLETE, gate-green.** The remaining
  solution-report Shows' **element/node forms** + `Show Elements`: `Show Voltages`
  code 1 (`WriteBusVoltages` — line-ground **and** line-line by bus & node,
  mag/angle/pu/base-kV, the `jj`-cursor node walk + the wrapping LL partner) and code
  2 (`WriteElementVoltages` — node-ground by element, Sources+PD then PC); `Show
  Currents` code 1 (`WriteTerminalCurrents` — per-terminal branch currents + the PD
  residual row, Sources+PD+Faults then PC); and `Show Elements` (`ShowElements` +
  `WriteElementRecord` — the element↔bus listing, PD then PC, split into the main
  `Elements.txt` + the `_Disabled.txt` companion; the optional class-name filter).
  The dispatcher arms 3/13 now route codes 0/1(/2), and the new **arm 5** parses the
  class filter and writes the two files (disabled first, no `@lastshowfile`; main
  sets it — via the new `write_show_named(set_last)` split). golden_phase8 **65→69**
  (`show_{voltages_node,voltages_elem,currents_elem,elements}`). **One finding settled
  during the step:** the pinned dss_capi 0.14.5 `SetMaxBusNameLength` **floors at 12**,
  not the source's 4 — probe-proven `max(12, longest_bus_name)` (a 20-char bus widens
  to 20, a 5-char one floors at 12), so IEEE13 (`sourcebus`=9) pads to 12. Reproduced
  1:1 (`TODO(compat)` in `report/show/mod.rs::max_bus_name_length`), same
  backend-vs-source class as the `MaxDeviceNameLength=0` finding; it only shows up in
  the dot-padded `WriteBusVoltages` column (the space-padded reports are
  token-invariant, which is why steps 1–3 didn't surface it). Both independent audits
  ran (`0ceb615`); **no correctness bug** — audit-code verified every walk set, index
  translation, formula and dispatcher arm against Pascal + the oracle. **audit-code
  follow-ups** (2, both settled empirically): (1) `WriteElementVoltages` (Voltages
  code 2) writes the per-element separating blank line **outside** the `Enabled`
  guard, so a *disabled* element still emits its blank — reproduced (raw
  `walk_element_voltages` over all refs, blank per element; oracle-probed on a
  disabled-load deck, byte-match); (2) the floor-12 `TODO(compat)` refined — the
  oracle floors the **data** (`PadDots`) rows at 12 but the **column-header** row's
  `Bus` at 4 (a backend within-report inconsistency, probe-measured), a masked
  whitespace-only byte divergence noted for the WP8.8 byte pass. **audit-tests
  follow-ups** (3): (1) the angle `%.1f` printing-floor overrides were mis-indexed —
  the first row of each bus carries an extra `..` dots token (and the elem form
  splits `(pu)` into two tokens), shifting the angle off the fixed `Index`; fixed via
  a new **content-relative `ColSel::AfterToken("/_")`** selector (the angle is the
  column after the `/_` glyph, robust to the row shift); (2) the `show_currents_elem`
  angle gate raised `1e-6 → 1e-4 A` — this report's residual rows push the noise floor
  to ~1.08e-5 A (633/634 near-balanced-transformer residuals) while the smallest real
  current is 5.72e-4 A, so `1e-4` brackets them (the `1e-6` code-0 threshold didn't);
  (3) two coverage goldens added — `show_elements_class` (the class-filter path) +
  `show_voltages_ll_node` (the LL branch). Also fixed a step-4 **test-tree leak**: the
  `show_reports_are_silent_noops` unit test ran `Show Voltage LN Nodes` (now a *real*
  report since step 4) with no datapath → wrote `t_VLN_Node.txt` into the source tree;
  set a scratch datapath + switched to still-unported keywords. golden_phase8 **69→71**.
- **WP8.4 (Show reports) — steps 5–8 + the WP8 goldens-exactness audit** continue in
  the **header frontier** (§ top): that block carries the recent-step detail (step 5
  `Powers` elem + `Result`/`EventLog`/`Ratings`/`Variables`/`Mismatch`/`monitor`;
  step 6; step 7 the diagnostic/matrix cluster `Convergence`/`Y`/`controlqueue`/
  `kvbasemismatch`; step 8 the register tables `Meters`/`Generators`; and the
  exact-equality re-measure) until the **WP8.4-completion archive pass** folds the
  whole Show record into [`docs/phase-records/phase-8.md`](docs/phase-records/phase-8.md),
  the established phase-record pattern. The current active step / next is authoritative
  in the header frontier.

---

## 2. What Phase 3 built (file-by-file map) — archived

The Phase-3 vertical-slice **file-by-file architectural map** moved to
[`docs/phase-records/phase-3.md`](docs/phase-records/phase-3.md) (2026-06-21) to
keep this handoff lean. It is still the architectural reference §3/§4/§5 below
build on — only its location changed.

---

## 3. Key design decisions & rationale

### 3.1 Element storage stays in the executive; the solver sees `ElemStore`
`Vec<Box<dyn DssObject>>` per class; the circuit holds `Vec<ElemRef>` lists;
solution machinery walks them through `ElemStore` + `as_ckt_element_mut()`.
Zero unsafe, no double ownership.

### 3.2 Signal flags instead of `ActiveCircuit` globals *(load-bearing)*
Elements set `cd.signal_bus_name_redefined`/`cd.yprim_invalid`; the executive
propagates after the edit loop. Equivalent because nothing reads the globals
mid-edit (first reader is `BuildYMatrix` at solve time).

### 3.3 Compensation-current loads
Loads are stamped into Y **and** inject `Yprim·V − model current`; iteration
equality in the gates is the regression test for this.

### 3.4 Parse-time reference snapshots + deferred cross-element writes (Phase 4)
Pascal resolves object references mid-parse against live pointers and lets
recalc *read* (and `Set_TapNum` *write*) the target at any time. The Rust edit
loop holds only a read view of foreign classes, so:
- reads needed later (control `RecalcElementData` at `EndEdit`) come from a
  `RefSnapshot` captured at resolution time — same staleness semantics as
  Pascal (a control refreshes only on its own recalc);
- writes (`TapNum`) become queued `RefAction`s the executive applies right
  after the edit, with the writer keeping its snapshot in sync via the
  identical clamp. Nothing observes the target in between.

### 3.5 `NOT_PORTED` property flag
Catalog/machinery references that belong to later phases hard-error on set —
a script that needs unported machinery cannot produce silently-wrong numbers.

### 3.6 Controls are invisible to Y
`ElemKind::Control` elements join the device list (so `ProcessBusDefs` walks
them in creation order — node order matches the oracle) but never the PD/PC
lists; `yprim` stays `None` and the Y build skips them.

---

## 4. Empirical oracle facts (cumulative highlights)

- `?`, `Edit`, `~`, `Solve`, `Set`, `Get` are **circuit-gated** (error 301).
- `Solution.Iterations` = the total over control iterations, assigned at the
  end of `SolveSnap`.
- `YNodeOrder` = `ProcessBusDefs` allocation order over enabled elements in
  creation order — **including control elements** (their bus is set in
  `RecalcElementData`; they add no nodes in the IEEE feeders).
- The `DoubleSymMatrixProperty` getter is broken upstream (reads a field
  address as the array) — Capacitor `CMatrix`, Reactor `RMatrix`/`XMatrix`
  always dump garbage; canonicalized to zeros on both sides.
- `Circuit.Losses` skips shunt PD elements; `Circuit.TotalPower` = Σ sources
  `Power[1]`·1e-3 (not negated); element `Powers` = `GetPhasePower`·1e-3 over
  all conductors/terminals.
- RegControl `TapNum` get/set maps tap↔integer through the *controlled
  winding's* (TapWinding) Min/Max/Increment: `tapnum=5` on a 32-tap winding
  moves the tap to 1.03125; reads back 5. `winding=` resets `TapWinding`.
- CapControl `type=time` (and `follow`) forces `Terminal=1` and monitors the
  capacitor itself; a missing `capacitor=`/`element=` raises (303); `vbus=`
  set during parse warns "Did you wait until buses were defined?" and reverts
  the flag (bus list doesn't exist yet) — faithfully reproduced.
- Controls-off feeders: IEEE13 41 nodes, IEEE37 117, IEEE123 278; all solve in
  exactly 3 fixed-point iterations.

---

## 5. `TODO(compat)` / deferrals

Grep `rg "TODO\(compat\)"` for the full marker list (39 sites). Notable:
truncated `CALPHA`/`pi`/`0.001732`/`57.29577951` constants, FPC banker's
`Round` shims, LineCode `Repair`=0 default, the `DoubleSymMatrix` zero-matrix
getter.

`NOT_PORTED` (hard parse error; every site points at its phase):
- Line `geometry` — **ported (WP7.1 step 3a)**: resolves a `LineGeometry`, runs
  `FetchGeometryCode` + `FMakeZFromGeometry` (the Carson `Zmatrix`/`YCmatrix`).
  Line `spacing`/`wires`/`cncables`/`tscables` stay `NOT_PORTED` until step 3b
  (the `FetchLineSpacing`/`SetWires`/`FMakeZFromSpacing` path, PORTING_PLAN
  §Phase 7 sub-block 1).
- Reactor `RCurve`/`LCurve` — Phase 5 (XYcurve) — XYcurve is now ported; the
  fetch is still `NOT_PORTED` (only the harmonic `CalcYPrim` consumes it, Phase 7).
- CapControl `ControlSignal` — Phase 5 (LoadShape); still `NOT_PORTED` (the
  `Follow` control type that consumes it has no corpus case — WP5.6's `Sample`
  records the Pascal abort error if reached); `UserModel`/`UserData` — never
  (no DLL loading in safe Rust).
- LoadShape `CSVFile` — **ported (WP5.2b)** via the deferred-`FileLoad` path.
  `SngFile`/`DblFile`/`PQCSVFile` (binary/2-col input) stay `NOT_PORTED` until a
  gate needs them. Single-precision arrays + `MemoryMapping` (MMF) and
  `Action=DblSave`/`SngSave` (binary output) — not ported (no corpus case).
- TempShape (`TShape`)/PriceShape `CSVFile` — **ported (WP5.2c)** via the same
  deferred-`FileLoad` path. `SngFile`/`DblFile` (binary input) and
  `Action=DblSave`/`SngSave` (binary output) stay `NOT_PORTED`.
- GrowthShape `CSVFile`/`SngFile`/`DblFile` — file-input machinery, when a
  gate needs it.

Other deferrals: Transformer GIC path (<0.51 Hz) + harmonics interplay
(the frequency-scaled Y + the <0.51 Hz branch are **exercised by WP7.6
harmonics**; the GIC *elements* stay Phase 9); RegControl/CapControl
`Sample`/`DoPendingAction` **wired into the
control loop (WP5.7)**; RegControl/ControlQueue debug-trace files (flag
stored, no file — port with Monitors, Phase 6+); `MakePosSequence` everywhere
(Phase 6+); `BusCoords` **ported (WP5.8)**; Monitors/EnergyMeters
`sample_all`/`EndOfTimeStepCleanup` are no-op hook stubs at the SolveDaily/
Yearly/Duty call sites (Phase 6); the **dynamics** (WP7.7), **harmonics/harmonicT**
(WP7.6) and **faultstudy** (WP7.9) solve modes are **ported**; the Newton algorithm
and the Monte-Carlo/load-duration/AutoAdd/`SolveGeneralTime` solve modes keep the
"Unknown solution mode" error (no corpus case — WP7.9 empirical decision);
the report verbs are **routed (Phase 8)**: `Export Counts` (WP8.1) and the
bus/node solution exports `Voltages`/`BusCoords`/`NodeNames`/`YNodeList` (WP8.2
sub-step 1) are **real**, the remaining `Export` keywords + `Save`/`Dump` record a
scoped `NOT_PORTED` (real formatters in WP8.2–8.5), `Show` is a faithful silent
no-op (real `ShowResults` in WP8.4), `Plot`/`Visualize` are headless no-ops
(§2.5); `Select`/... remaining executive verbs still record "not ported"
(Phase 8 WP8.6).

---

## 6. How to run / regenerate

```bash
# Gate (must be green before any commit)
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# Run a script
cargo run -p dss-cli -- path\to\script.dss

# Regenerate goldens (MANUAL ONLY, pinned versions in tools/golden/PIN.txt)
python tools/golden/gen_props.py       # -> tests/golden/props/<class>.json   (Phase 2+)
python tools/golden/gen_slice.py       # -> tests/golden/slice.json           (Phase 3)
python tools/golden/gen_phase4.py      # -> tests/golden/phase4/*.dss + phase4.json
python tools/golden/gen_phase5.py      # -> tests/golden/phase5/<scenario>.json
python tools/golden/gen_phase6.py      # -> tests/golden/phase6/<scenario>.json
python tools/golden/gen_checkpoints.py # -> tests/golden/checkpoints/<scenario>.json
```
Oracle pin: Python 3.12.4, dss-python 0.15.7, dss-python-backend 0.14.5
(the same dss_capi release vendored in `.inputs/dss_capi`). `python` works in
this environment; the `py` launcher is broken — use `python` directly.

---

## 7. Phase 7 — inherited deferrals & architecture in place

> **The current frontier** (active step, branch, what's next, commit state) lives
> in the header up top and in **§1e** — not restated here, to avoid the two drifting
> apart. This section is the stable Phase-7 reference: what the phase inherits and
> what is already wired for it. Execute per `PHASE7_PLAN.md §0` (six
> independently-gated sub-blocks, risk-ascending: line constants → protection →
> DER → harmonics → dynamics → faultstudy/AutoAdd-modes/`Feeder`).

**What Phase 7 inherits / must finish (deferrals Phase 6 left explicit):**
- **DER classes** `Storage`/`PVSystem` (+ `InvControl`/`ExpControl`) and the real
  `StorageController` behavior — ✅ **all done**: `PVSystem` (WP7.3), `Storage` +
  `StorageController` (WP7.4), and `InvControl` + `ExpControl` (WP7.5) (the WP6.8
  StorageController parse-only skeleton is replaced by the real fleet dispatch;
  `is_zone_pce` now admits Storage/PVSystem).
- **Protection** `Relay`/`Recloser`/`Fuse`/`SwtControl`/`Fault` — ✅ **done
  (WP7.2)**: all five classes ported on the control sweep, the `Open`/`Close` exec
  verbs landed, and an enabled Relay/Recloser/Fuse sets `Flg.HasOCPDevice` so
  `RelCalc` no longer aborts (#52902) and `GetOCPDeviceType` is live — the
  SAIFI/SAIDI/section math runs on a protected zone.
- **Line constants** `WireData/CNData/TSData/CableData/LineSpacing/LineGeometry`
  + Carson — ✅ **done (WP7.1)**: Line's
  `geometry`/`spacing`/`wires`/`cncables`/`tscables` resolve and drive the Carson
  Z/Yc (one plural-cable reset + the `DG_Prot_Fdr` ~3e-5 line-Y precision case
  tracked-open, §1e).
- **Harmonics** (`DoHarmonicMode` for VSource/Load + Generator/PVSystem/Storage,
  the frequency sweep + the harmonic monitor header) — ✅ **done (WP7.6)**.
- **Dynamics** (Generator/Storage `DoDynamicMode`, state vars beyond names/count)
  + `MakePosSequence` everywhere; Monitor modes 3/4/7/8/10/12 build their header
  but defer the sample body; Transformer GIC (<0.51 Hz) elements — WP7.7+ / Phase 9.
- **AutoAdd solve mode** (`circuit/auto_add.rs` skeleton) — needs aux-current
  injection (`UseAuxCurrents`) + meter-register sampling in the solve loop; the
  options round-trip but the mode keeps its "Unknown solution mode" error.
- **ReduceAlgs** zone reduction — blocked on the unported `TLineObj.MergeWith`.

**Architecture already in place for Phase 7:** the control loop dispatches
through `ElemStore::{obj,pair_mut,triple_mut}` + `DssObject::as_any_mut`; the
meter/monitor `sample_all_monitors_and_meters`/`end_of_time_step_cleanup` hooks
have real bodies; the zone-build dispatcher (`solution/meters/mod.rs`) fires from
`build_y_matrix` after bus reprocessing; `TakeSample`/`Integrate` + the
reliability fault-rate sweep are ported. Still Phase 8: the `SystemMeter`
register core and all demand-interval/phase-voltage/`Show`/`Export` files.
