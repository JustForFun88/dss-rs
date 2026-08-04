# dss-rs — Project Status & Session Handoff

> **Purpose of this file:** a living snapshot so a fresh session can resume
> without re-deriving context. It records *what is done*, *what was decided*,
> and **why**. It is **not** authoritative for the plan itself — that is
> `PORTING_PLAN.md` (roadmap + binding decisions) and `CLAUDE.md` (conventions
> + the green-gate rule). Read those two first; then read this for the current
> frontier.

> **Full history archived (2026-08-05).** This file was 16 368 lines; every
> historical session record was moved **verbatim** into `docs/phase-records/`.
> New this round: [`golden-rebase.md`](docs/phase-records/golden-rebase.md),
> [`depascalize-stagef.md`](docs/phase-records/depascalize-stagef.md),
> [`depascalize-w3.md`](docs/phase-records/depascalize-w3.md),
> [`depascalize-r2b-r3.md`](docs/phase-records/depascalize-r2b-r3.md),
> [`depascalize-r1-r2.md`](docs/phase-records/depascalize-r1-r2.md),
> [`depascalize-p-series.md`](docs/phase-records/depascalize-p-series.md),
> [`unified-gate.md`](docs/phase-records/unified-gate.md),
> [`epri-bridge.md`](docs/phase-records/epri-bridge.md),
> [`wasm-usermodels.md`](docs/phase-records/wasm-usermodels.md),
> [`orphaned-gaps.md`](docs/phase-records/orphaned-gaps.md),
> [`bug-wps.md`](docs/phase-records/bug-wps.md),
> [`adopt-015x.md`](docs/phase-records/adopt-015x.md),
> [`era-summaries.md`](docs/phase-records/era-summaries.md),
> [`gate-history.md`](docs/phase-records/gate-history.md),
> [`design-decisions.md`](docs/phase-records/design-decisions.md),
> [`phase-index.md`](docs/phase-records/phase-index.md). Appended to:
> `depascalize-p1.md`, `phase-7.md`, `upgrade-rung1.md`. The pre-existing
> `phase-3..8`, `gaps`, `corpus-rounds`, `part2-adiakoptics`,
> `final-acceptance`, `test-triage-*` records are unchanged.

## 1. Where we are

**Era.** Post-acceptance. The 1:1 behavioral port is finished and
referee-certified (2026-07-11); `PORTING_PLAN.md` is history. The order of all
active work is `PLAN_SEQUENCE.md`; the binding conventions and the five-command
gate are `CLAUDE.md`. The 2026-08-02 user policy is in force: **EPRI r4133 is
the behavioral authority, the pinned dss_capi 0.14.5 is a numeric oracle only,
and upstream bugs are never reproduced in any lane** — the `oracle-parity` lane
has shrunk to a precision-compat lane and is scheduled for full teardown.

**In flight.** `GOLDEN_REBASE_PLAN.md` on branch **`golden-g2`**. WP-G0 (safety
rails) is complete and merged to `update`; WP-G2 (tear down the shared-with-r4133
bug kernels) is running — G2.0 rails + G2.1a…G2.1h + G2.2a landed,
`SPLIT_ALIAS_POPULATION` **31 → 21**, and the WP acceptance criterion still holds
at HEAD: `git diff --stat -- tests/golden` over the whole range is **empty** in
both lanes. Next step: **G2.2b** — the property-exclusion rows
`monitor_base_frequency` and `ISOURCE_BUS2_NEVER_LATCHES`. Queued behind
GOLDEN_REBASE: `WASM_USERMODELS` follow-ups, RESONANCE, MULTITHREADING, the
UPGRADE line.

**Sequenced after / parked.** DIAKOPTICS Part II WP-AD.6 (threaded children,
needs MULTITHREADING M2); the IEEE118Bus NCIM switching-cadence rung; the
`UpgradeRung` escape rows below. Details in *Standing open follow-ups*.

**2026-08-05 — STATUS.md archived.** 16 368 → 415 lines: every historical
record moved verbatim into `docs/phase-records/` (see the pointer above); the
GOLDEN_REBASE records below are condensed here and kept in full in
`golden-rebase.md`. Docs-only commit, no behavior change — no test parses this
file (`oracle_parity_cfg_gate.rs::operational_docs` deliberately excludes it).

### GOLDEN_REBASE WP-G0 / WP-G2 — condensed records

> Full session records (bug analysis, Pascal citations, pin inventories, audit
> settlements) → [`docs/phase-records/golden-rebase.md`](docs/phase-records/golden-rebase.md).
> Every step below ran the full five-command gate green before its commit, and
> every one was audited by two fresh opus auditors plus a fix agent on the same
> branch.

- **G0.1** (`5261266c`, fix `9de32e82`, branch `golden-g0`, 2026-08-02) — the
  provenance lock `tests/golden/golden.lock.json`: 737 rows (`path, sha256,
  anchor, reason, produced_by`), anchors 700 `capi_v0145` / 11 `capi015` / 11
  `r4133` / 10 `r3723` / 1 `fpc_3.2.2` / 4 `self`, enforced fail-on-stale in both
  directions by the new `golden_lock.rs`. No golden byte moved. Audit: 12
  findings, all settled, 0 digests moved.
- **G0.2** (`1a841cb1`, fix `0cad83dc`, 2026-08-02; WP-G0 merged to `update` as
  `34baa5b7`) — `harness::regen()` + `snapshot_text/bytes()`, the guarded writer
  behind `DSS_UPDATE_GOLDENS`: refuses `ExternallyAnchored`, `WrongLane`,
  `Unlocked` and `NoProducingLane`. Still no golden byte moved. Audit: 11
  findings, all settled.
- **G2.0** (`ef1ba28c`, fix `5d712272`, branch `golden-g2`, 2026-08-02) — WP-G2
  rails only, no kernel touched: the TESTING.md re-anchor onto the five numeric
  survivors (`compat::PI`, `round_f64`, `round_i32`, `kv_base_search_scale`,
  `profile_ll_pu_divisor`), the `TORN_DOWN_ROWS` register with its six rot checks
  and two census ties, and the `// LANE-EXCLUSION(<row>)` markers. `lane_diff.ps1`
  re-run: max |Δ| = 0. Audit: 9 findings (2 major), all fixed.
- **G2.1a** (`2d3b3d11`, fix `0e89651f`, 2026-08-03) — `stddev_single_point`
  deleted: a one-element sample has no spread, so all four `support::mathutil`
  entry points return `0.0` (r4133 `mathutil.pas:405/:429`, 0.14.5 ×4). Only the
  parity lane moves. 31 → 30. Audit: 6 minor findings, settled.
- **G2.1b** (`20179793`, docs `b7e5c867`, fix `add7e0ef`, 2026-08-03) —
  `CAPCONTROL_MAKELIKE_DROPS_CONTROL_SIGNAL` deleted: `make_like` copies
  `control_signal_name` + `ctrl_signal_shape` unconditionally. 30 → 29. Audit: 3
  minor, all fixed.
- **G2.1c** (`91df2a25`, fix `04d845ab`, 2026-08-03) —
  `SEQ_CURRENTS_PRINTS_RAW_NONPOSITIVE_RATING` deleted: one `pct_of_rating`
  closure returns `0.0` for a non-positive rating (an undefined rating is not a
  percentage). The same loop's `IRESIDUAL_FROM_TERMINAL_1` is untouched — G2.2a
  owns it. 29 → 28. Audit: 4 minor, all fixed.
- **G2.1d** (`1c7f0501`, fix `68f5dade`, 2026-08-03) —
  `REDUCE_SCANS_ONLY_THE_FIRST_PARENT_SHUNT` deleted: `red_short_line_step` scans
  the parent's whole shunt list with the same predicate the merge-with-child
  branch always used. 28 → 27. Audit: 3 findings (1 filed major), settled with a
  non-vacuity input the fix agent added.
- **G2.1e** (`87a64820`, fix `9b5da5a6`, 2026-08-03) —
  `STORAGE_CONTROLLER_IDLE_TEST_COMPLEMENTS_THE_ORDINAL` deleted:
  `fleet_needs_idling` is one unconditional `fleet_state != StorageState::Idling`.
  27 → 26. Audit: 4 minor, three fixed as stated.
- **G2.1f** (`f990dbed`, fix `08131c08`, 2026-08-03) —
  `STORAGE_MULTIFILE_USES_THE_PV_PREFIX` deleted: the `RegKind::Storage` arm of
  `gather_register_rows` returns `"EXP_STORAGE_"` plainly. 26 → 25. Audit: 5
  minor, one refuted with a worked counter-example.
- **G2.1g** (`359a2ca3`, fix `6f178b93`, 2026-08-04) —
  `CIM_WYE_GROUNDED_IS_HARDCODED_TRUE` deleted: both `cim/export.rs` writers read
  the neutral (capacitor `term2_nodes.all(== 0)`, load `neutral_node == 0`) — the
  same test the unit's own transformer writer applies. 25 → 24. Audit: 3 minor,
  fixed with a mixed-grounding deck added to the pin.
- **G2.1h** (`a7b42939`, fix `848802dc`, 2026-08-05) —
  `HEIGHT_UNIT_CHANGE_REREADS_THE_METRES_FIELD` deleted, the **last of the eight
  zero-footprint G2.1 rows**: `set_user_height_unit` reads `height_offset()`
  before the unit field moves, so a typed height is not converted twice (r4133
  `LineConstants.pas:689-696`; the surface does not exist in 0.14.5, so the row is
  cited and gated on `r4133` only). 24 → 23. Audit: both findings upheld,
  docs-only.
- **G2.2a** (2026-08-05) — the first two rows whose teardown a **golden compare
  observes**, both dismantled by making an existing exclusion unconditional
  rather than by moving a golden byte. `IRESIDUAL_FROM_TERMINAL_1`:
  `CalcAndWriteSeqCurrents` applies the `(j-1)*Ncond` offset to its symmetric
  components (r4133 `Version8/Source/Common/ExportResults.pas:323`) but not to
  the residual sum (`:365-366`, dss_capi `:422-424`), so every terminal row
  repeats terminal 1's residual; both lanes now sum the row's own terminal, and
  the golden's `Terminal >= 2` `Iresidual` cells are excluded in **both** lanes
  and pinned by `export_seqcurrents_iresidual_sums_the_rows_own_terminal`
  (derived from `Export Currents`' `Iresid_j`, an independent anchor).
  `BUS_INT_DURATION_WALKS_ALL_BUSES`: `CalcReliabilityIndices` sizes
  `FeederSections` to its own zone (r4133 `Meters/EnergyMeter.pas:2507`) but
  writes bus durations while walking every circuit bus (`:2567-2574`), and the
  section-id zeroing that would clear a foreign id is itself per-zone (`:2472`),
  so with two meters the later one overwrites the earlier one's durations; both
  lanes now walk only their own zone, the `Duration` column of
  `export_busreliability_multimeter` is masked in both lanes and pinned
  literally by `export_busreliability_multimeter_duration_stays_in_the_meters_zone`
  (`B1 = 4`, `B2 = 5`, not upstream's 6/9). 23 → 21; both rows carry
  `Evidence::Exclusion`, the first use of that variant. The pin-walk
  non-vacuity anchor for the integration-test shape moved off the torn-down
  `IRESIDUAL_FROM_TERMINAL_1` onto the numeric survivor `PI`
  (`crates/dss-parser/tests/parser_golden.rs`), which outlives WP-G2 and WP-G4.
  Doc strikes in the same commit: CLAUDE.md's two bug bullets and its WP-G2
  status line, `tests/TOLERANCE_NOTES.md`'s `Iresidual` note (rewritten in
  place — it is a note about the `SeqCurrents` compare policy, and the
  "deliberately-reproduced" section it might otherwise move to is about
  reproductions, which this no longer is). No ledger or `population.lock.json`
  movement: no live gate reads `Bus.Int_Duration` or report text yet (WP-G1's
  G1.6 adds the reliability columns, which is why the plan orders this row
  first), and the corpus gate stayed green in both lanes.
  Audit: 6 minor, **all upheld and fixed** (docs plus one rail), no engine code
  touched. Five were doc-rot the teardown left behind — `run_deck_export_capture`
  and `GateSpec::Mask` still described their pre-G2.2a roles, the
  `branches_on_lane` census still said `golden_reports.rs` ×15 (now ×9), the
  `Evidence` `expect(dead_code)` note still claimed only `Site` is constructed,
  and `GOLDEN_REBASE_PLAN.md`'s G5.1 list still promised a TOLERANCE_NOTES
  *move* that was a rewrite-in-place (both plan lines now say so, so G5.1 does
  not chase it). The sixth was real coverage: the re-anchored pin-walk
  non-vacuity const keys on the bare token `PI`, which
  `parser_golden.rs`'s incidental `f64::consts::PI` satisfies, so deleting the
  deliberate `compat::PI` citation left the anchor green on a std-library
  homonym — reproduced, then closed by re-checking the **qualified**
  `compat::<alias>` spelling in the region the walk credited (the probe now
  fails on that assert alone). `lane_diff.ps1` not re-run: the fix touches no
  compat kernel, lane alias or solver.

### Live escape register — the 18 surviving `TODO(compat)` markers

The register itself is executable: `oracle_parity_cfg_gate.rs::ESCAPE_REGISTER`
(+ `EXIT_POPULATION`) checks it **both ways** — an unregistered marker fails, a
registered marker that no longer exists fails. Rows below are the prose index;
the site comment carries each row's measured cost.

| Owner | Site | Row |
| --- | --- | --- |
| `UpgradeRung` | `support/line_constants/mod.rs` | truncated `mu0` / `Twopi` (35/520 corpus cases) |
| `UpgradeRung` | `support/line_constants/cable.rs` | truncated `1/pi` = `0.3183` in the tape-shield resistance (8/520) |
| `UpgradeRung` | `util.rs` | Pascal `CALPHA = (-0.5, -0.866025)` (33/520 + `dump_reactor_symcomp`) |
| `UpgradeRung` | `elements/pd/reactor/solve.rs` | the same truncated `CALPHA`, second site |
| `UpgradeRung` | `support/complexutil/mod.rs` | truncated constants from `DSSUcomplex.pas` (`pi`) |
| `UpgradeRung` | `support/complexutil/mod.rs` | rad→deg `57.29577951` — "replace with `f64::atan2`" |
| `UpgradeRung` | `elements/pd/line/mod.rs` | Carson earth-return depth `658.5` (not `658.8530451057239`) |
| `UpgradeRung` | `elements/pd/line/code.rs` | the same `658.5` |
| `UpgradeRung` | `elements/pd/line/accessors.rs` | the same `658.5` |
| `UpgradeRung` | `elements/control/exp_control/accessors.rs` | `FOpenTau := Tresponse / 2.3026` (documented r4133 model constant) |
| `UpgradeRung` | `cim/ieee1547.rs` | `LPFTau * 2.3026` |
| `WholeCase` | `elements/pd/gic_transformer/solve.rs` | Pascal uses `FPctR1`, not `FPctR2` (`gictransformer_gic`, `gic_midi`) |
| `WholeCase` | `elements/pd/capacitor/solve.rs` | `SetDouble(ord(TProp.Cuf), Cs - Cm)` (`makeposseq_shunt`) |
| `WholeCase` | `elements/general/load_shape/compute.rs` | the MMF accept-set keeps only bytes in `[46, 58)` (`shape_mmf`) |
| `WholeCase` | `elements/pc/generator/user_model.rs` | the Model=6 dynamics-entry `E1` seed (`wasm_gen_dyn`) |
| `WasmGuest` | `tools/wasm_usermodel/models/indmach012a/src/symcomp.rs` | truncated `sqrt(3)/2` = `0.866025403` |
| `WasmGuest` | `.../indmach012a/src/model.rs` | truncated `sqrt(3)` = `1.732` |
| `WasmGuest` | `.../indmach012a/src/model.rs` | FPC single-precision folding of `3.0/746.0` |

> **Reading note (2026-08-05).** The two lists below are kept **verbatim** from
> the pre-archiving STATUS. Rows closed by a later round say so in place, with
> two exceptions the archiving supersedes: the "`TODO(compat)` bug-for-bug sweep
> → Stage F — NOT started" and "`HIDE_015X` → Stage F — NOT started" handoffs
> were executed by DE_PASCALIZE Stage F (complete, `depascalize-stagef.md`) and
> are now finished by GOLDEN_REBASE WP-G2/WP-G4; the live marker population is
> the 18-row table above, not §5's 2026-07-17 count of 123. Their in-place
> back-references now resolve outside this file: `§OG-1.x … below` in
> [`orphaned-gaps.md`](docs/phase-records/orphaned-gaps.md), `§1a archive` in
> [`era-summaries.md`](docs/phase-records/era-summaries.md). §7 states the
> general forwarding rule.

### Standing open follow-ups (actionable)

**Carried-forward handoffs — work a *declared-complete* plan deferred to a
successor plan that has NOT finished it** (audited 2026-07-17; surfaced here so the
open item is not buried in the §1a archive):
- **TODO(compat) bug-for-bug sweep → DE_PASCALIZE Stage F — NOT started.** 123
  `TODO(compat)` shims across 71 files still in-tree (PORTING_PLAN §4.1 rule 4, the
  single dedicated post-acceptance cleanup pass, re-assigned to Stage F). Stage F is
  unbuilt (DE_PASCALIZE paused after wave 1) → cleanup unexecuted.
- **UPGRADE §5 exit criterion `rg HIDE_015X` empty → DE_PASCALIZE Stage F — NOT
  started.** 15 `HIDE_015X` refs in 5 files (line / line_geometry / prop_flags /
  save/dump). UPGRADE was declared COMPLETE having consciously waived this own-§5
  criterion to Stage F (byte-neutral, non-rung-blocking); still unmet.
- **IEEE118Bus NCIM PV→PQ switching-cadence → a future UPGRADE rung — NOT started.**
  Port matches its capi015 oracle loop-for-loop incl. non-convergence, but not
  r4133's newer cadence; parked `skipped_needs_investigation`, report-only in
  DIVERGENCES.md.
- **GICMvars export (verb 36) / GICTransformer `WriteVarOutputRecord` → Phase 9 —
  ✅ PORTED 2026-07-18** (orphaned-gaps round OG-1.1, branch `og11-gicmvars`; see
  §OG-1.1 below). Was GAPS WPG.16's only deferred piece.
- **AltDSS JSON `DynInit` tail + Full-mode Transformer WdgCurrents — DONE
  (og1213, 2026-07-18; see §OG-1.2+1.3).** Capacitor CMatrix = proven UB
  non-port (uninitialized heap, nondeterministic across processes). New
  sub-follow-ups surfaced (below).
- **AutoTrans JSON array-alternative metadata + golden → ✅ CLOSED 2026-07-26**
  (orphaned-gaps round OG-1.3a, branch `depas-og`; see §OG-1.3a below). The
  metadata premise was **stale**: `430d033` (og15c-B6) had already ported the
  singular/plural `array_alternative` + `REDUNDANT` + `ON_ARRAY` block into
  `auto_trans/mod.rs`, so only the golden was missing. Now pinned by
  `autotrans_micro` + `autotrans_solved`.
- **Generator/PVSystem/Storage `ShaftModel`/`ShaftData` under JSON Full → ✅
  CLOSED 2026-07-26** (OG-1.3a). Re-triaged empirically: the WM.3/WM.4
  NOT_PORTED removal did make them render, and the Full JSON matches the pinned
  0.14.5 oracle exactly (all six surfaces `""`). Pinned by `der_usermodel_full`
  (empty default) and `der_usermodel_assigned` (assigned data strings).
- **DER user-model FILENAME render (`UserModel`/`ShaftModel`/`DynaDLL`) still
  unpinned — OPEN (OG-1.3a settle).** Probed: the oracle stores and renders an
  unresolvable name but raises `#570 … Not Loaded` doing it, so a byte golden
  needs `gen_json.py` to tolerate a `DSSException` on selected commands.
  Deliberately not built — do it together with the WASM loader's own
  error-path gating, not by weakening the generator.
- **WindGen `Spectrum` FullNames render ungated — OPEN (OG-1.3a settle).** The
  other twelve Spectrum-bearing classes are pinned (`spectrum_refs` +
  `der_usermodel_full`); WindGen has no capi channel because the class does not
  exist in the pinned 0.14.5 oracle. Needs an r4133-side JSON channel, or a
  UPGRADE-line rung that gives WindGen an oracle.
- **A-Diakoptics `AggregateProfiles` command + D9(d) official-r3723 AD-replay →
  DIAKOPTICS Part II WP-AD.5 — partial.** `exec/command.rs:69` `NOT_PORTED`; WP-AD.6
  threaded children not started (needs MULTITHREADING M2).
- **User-model native DLLs (Gen/PVSystem/Storage/CapControl UserModel) →
  WASM_USERMODELS COMPLETE (WM.0–WM.7 all done, 2026-07-25).** All six properties
  × four elements (Generator `UserModel`/`ShaftModel` WM.3; Storage
  `UserModel`/`DynaDLL` + PVSystem `UserModel` WM.4; CapControl `UserModel` WM.5;
  callback tail WM.6; exit sweep WM.7) follow the plan §2.4 uniform rule (a
  `.wasm` loads through the sandboxed `dss-usermodel` ABI; a native-DLL name warns
  #570/#1570 + falls back to built-in), gated bit-exact vs the r4133 oracle
  (`indmach012a` + `wm4model` + `capuserctl` twin-pinned `.wasm` fixtures). **Zero
  live `PropFlags::NOT_PORTED`** on any user-model property. `PLAN_SEQUENCE.md`
  stage 9 COMPLETE.

**Residual floors / parked (documented, not bugs):**
- **ckt24 RegControl/LDC `SubXFMR`** ~4.7e-5 rel tap-current — ultra-switch
  conditioning floor (CF-D), watch on re-touch.
- ~~**UPFC modes 2/3/5**~~ **CLOSED 2026-07-18 (OG-1.7)** — see §OG-1.7 record;
  `midi_relay_dist` deferred (budget); Kersting4wire #567
  UserModel decks parked (no oracle channel tolerates the DoSimpleMsg).
- **UTF-8-BOM edge cases** — GAPS follow-up. (`CapControl.ControlSignal` FOLLOW path
  is in fact *ported* and live in `cap_control` — the old "unported" note was stale
  and is retired.)

Retired (done): combo fuse-save restore (wt-combo); WP-U1.2 D3 / WP-U1.6 tail (all
landed pre-rung-exit); Monitor modes 8/10/12 (test-triage wt-t3).

## 5. `TODO(compat)` / deferrals

Grep `rg "TODO\(compat\)"` for the full marker list (**123 sites across 71 files** as of 2026-07-17 — Phase 7/8/GAPS/UPGRADE added many; the whole set is wiped in one DE_PASCALIZE Stage F pass, not yet run). Notable:
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

**Official-EPRI-OpenDSS oracle (opt-in, `tools/opendss/` — 2026-07-07):**
vendored EPRI `OpenDSSDirect.dll` r3723 (9.8.0.1) / r4088 (10.2.0.1) / r4133
(11.0.0.1) driven through the AltDSS Oddie bridge (dss-python 0.16.0b2 in a
separate venv, `PIN_OPENDSS.txt`), reusing `oracle_server.py` unchanged via
`DSS_ORACLE_ENGINE=oddie`. For inventorying upstream changes ahead of porting
them; the mandatory gate is untouched. Workflows (see `tools/opendss/README.md`):
`DSS_LIVE_OPENDSS=<rev> cargo test ... corpus_live_opendss` → report
`tmp/opendss_report_<rev>.json`, now partitioned against the triage catalog
`tools/opendss/known_diffs.json` (2026-07-07, modeled on DSS-Python
`KNOWN_COM_DIFF`; substring match on case label + first-failure reason, every
entry states its cause, zero-hit entries warn). r3723: 150 matched / 82
known-diverged / **0 new** of 232 — all 82 triaged into 11 classes (19 EPRI
InvControl max-iter failures, 25 InvControl fixpoint drift, 10 iteration
deltas, 8 monitor-header whitespace, 4 property-format brackets, 4
injection FPC-vs-Delphi ulp, 3 storage kWhStored drift, 3 meter ZonePCE
count, 3 event-log trailing space, 2 GenDispatcher prop-name, 1 harmonics
Y-fingerprint) — so `DSS_LIVE_OPENDSS_ASSERT=1` (fails only on NEW) is green
for r3723. Caveat: comparison stops at a case's first divergence — a known
first divergence masks later ones in that case (accepted for inventory).
`ab_compare.py --a oddie:r3723 --b oddie:r4133` → upstream-change inventory
(baseline: 109/168 match; deltas in distance relays, harmonics, InvControl
iteration behavior); `--known-diffs tools/opendss/known_diffs.json` relabels
fully-triaged cases `known_diverged` (entries carry `ab_contains` where this
tool's issue wording differs) and exits 0 when only known diffs remain.
Two operational gotchas, both handled: (1) EPRI's Delphi `FireOffEditor`
ShellExecutes the editor on every `Show`/`Export` with NO `NoFormsAllowed`
check and Oddie can't set `AllowEditor` — a corpus sweep opened hundreds of
Notepads; `make_engine()` now issues `Set RegistryUpdate=No` + `Set
Editor=rundll32.exe` (silent no-op; registry write suppressed so the user's
OpenDSS editor setting is untouched) — verified on all 3 revisions with a
`Show` deck, zero spawns. (2) `.inputs/electricdss-tst` is now a re-checkout
with different EOLs: `tools/corpus/vendor.py --force` produces a ~1544-file
EOL-only diff — clean run pollution with `git restore tests/corpus` instead;
re-vendor only deliberately.

**DSS-Python validation harness, vendored (`tools/opendss/dsspy_validation/`
— 2026-07-07):** copy of DSS-Python `fastdss` `tests/`
`_settings`/`save_outputs`/`compare_outputs` (BSD-3, attribution headers,
local edits marked `# dss-rs:`): full-API-state dumps (~40 collections/case,
206 upstream-curated cases, all present in our corpus) zipped per engine +
offline tolerant diff (their `KNOWN_COM_DIFF` catalog kept as upstream) —
broad-surface upstream inventory complementing `ab_compare.py`. Adaptations:
corpus → vendored copy, engine spec `DSS_EXTENSIONS_TEST_ODDIE=oddie:<rev>`
via `revisions.json` (+ expect_version hard check), COM branch dropped, our
`RegistryUpdate=No`+`Editor=rundll32.exe` suppression, per-case `CorpusGuard`
(lifted move-only into `tools/oracle/corpus_guard.py`, shared with
oracle_server), results → `tmp/dsspy_validation/`, and `(Oddie)`-prefixed
DSSException skips for API exports absent from older official DLLs (r3723
lacks `Transformers_Get_LossesByType`, `StoragesI`, ...). **Its `capi` side
is dss_capi 0.15.0b4 — NOT the pinned 0.14.5 oracle; inventory only, never
feeds goldens/gate.** pandas+xmldiff pinned into the Oddie venv
(`PIN_OPENDSS.txt`). Sweeps must end with `git status tests/corpus` (guard is
non-recursive; a sweep-created *subdirectory* — 123Bus `Run_YearlySim` makes
`16Nov2011/` — escapes it: `git clean -fd` that path). Full-sweep baseline
2026-07-07: capi 199/206 captured, oddie:r3723 189/206 (its 19 misses = the
`epri-invcontrol-maxiter` #485 class, 1:1 with known_diffs), compare
processes 3885 zip entries. The two beta packages are vendored as wheels in
`tools/opendss/wheels/` (+SHA256SUMS; offline `--find-links` install proven)
— setup no longer depends on the pre-releases staying on PyPI.

**DSS-Python corpus cross-check (`tools/corpus/dsspy_crosscheck.py` —
2026-07-07):** diffs DSS-Python's own 206-case validation list
(`.inputs/DSS-Python/tests/_settings.py::test_filenames`, extracted textually
— importing that module binds a DSS engine) against our six classifier
manifests → `tmp/dsspy_crosscheck.{json,md}`. Measured split: 133
solvable_now / 59 skipped_unsupported / 8 needs_investigation / 5
oracle_issue / 1 not_an_entry_point = **73 promotion candidates** (35
unblock at WP8.6 BatchEdit alone); all 206 exist in the vendored corpus.
`L!`-prefixed cases (55) are run line-by-line upstream with interactive
commands filtered — recorded per case so promotion work doesn't naively
`Compile` them.

---


## 7. Archived history — index of `docs/phase-records/`

Frozen history, superseded only by the code and tests. Every file is verbatim
STATUS.md text; nothing here is a summary.

**Forwarding rule for `STATUS §X` citations.** Code comments, plans and manifests
across the repo cite this file by section (~70 such references outside
`docs/phase-records/` at archiving time). A citation is history — it names the
section it was written against, and the call sites were deliberately **not**
rewritten. Resolve it in whichever file below holds §X; `rg '§X'
docs/phase-records/` finds it. The moved anchors most often cited: §1a →
`era-summaries.md`; §1b–1f and §2 → `phase-index.md`; §3/§3.4 and §4 →
`design-decisions.md`; §OG-1.x → `orphaned-gaps.md`; §7 (**caution** — the old
"Phase 7 — inherited deferrals" section, unrelated to *this* §7) →
`phase-7.md`; the per-WP records (§WP7.5, §WP8.3, §WP-AD.*, §WM.*, §W3.*,
§R2/R3, …) → the per-plan file named in the table.

| File | What it holds |
| --- | --- |
| `golden-rebase.md` | GOLDEN_REBASE WP-G0 (G0.1/G0.2) and WP-G2 (G2.0, G2.1a–h) full records — the source of §1's condensations |
| `depascalize-stagef.md` | DE_PASCALIZE Stage F: the `oracle-parity` lane split, F.1 → F.5 and the wave-4 settlement (incl. the escape-register steps F.3, F.3aa–F.3ag) |
| `depascalize-w3.md` | DE_PASCALIZE wave 3 (W3.1–W3.5), the wave-2a settler, R3iv, P3 |
| `depascalize-r2b-r3.md` | R2b + R3: the `ElemRef → ElemId` store flip, typed accessors, the `Any`/`as_ckt_element` removal |
| `depascalize-r1-r2.md` | R1 typed arenas (`Idx<T>`/`ElemId`/`Elements`) and the R2 injection seam |
| `depascalize-p1.md` | P1 + the whole P1-tail series (1/n–5/n) and its escape record (all three rows closed by W3.1–W3.3) |
| `depascalize-p-series.md` | P1b, P5a/b/c (miette diagnostics), P8, P9, P10, P11, P12, P13, P14, P15 |
| `depascalize-p2.md`, `-p6.md`, `-p12.md`, `-p13.md`, `-r0.md` | the earlier standalone DE_PASCALIZE records |
| `unified-gate.md` | UNIFIED_GATE Phase 0 and A–F, the pre-E/F cross-phase audit and the post-audit fix round |
| `epri-bridge.md` | the `dss-epri` bridge parity round and capability round (Oddie retirement) |
| `wasm-usermodels.md` | WASM_USERMODELS WM.0–WM.7, the ABI re-freeze to r4133, the D2 sub-bug trace and its fix |
| `orphaned-gaps.md` | OG-1.1 … OG-1.10 (GICMvars, AltDSS JSON tails, `CAPI_Schema` walks, UPFC modes, NCIM cadence, `Export Estimation`) |
| `bug-wps.md` | the standalone BUG work packages: livectx and DynExp |
| `adopt-015x.md` | the 0.15.x adoption sweep fix round and its settle |
| `upgrade-rung1.md` | UPGRADE Rung 1/2 records + the NCIM oracle-of-record re-gate |
| `corpus-rounds.md` | the corpus classification/coverage rounds |
| `part2-adiakoptics.md` | DIAKOPTICS/PSTCALC Part II (AD.1–AD.5) |
| `gaps.md` | the GAPS plan (WPG.1–WPG.21) |
| `final-acceptance.md` | the 1:1 final-acceptance round (§6) and the referee certification |
| `era-summaries.md` | §1a archived completed-plan records + the condensed era summaries |
| `gate-history.md` | the historical gate-state snapshots (pre-`corpus_gate.rs`) |
| `design-decisions.md` | §3 key design decisions & rationale + §4 empirical oracle facts |
| `phase-index.md` | the old §1b–1f/§2 index of the completed phases |
| `phase-3.md` … `phase-8.md`, `phase-7-wp1..7.md` | the per-phase and per-work-package logs |
| `test-triage-*.md` | the test-triage rounds (AD classify, IndMach, infra audit, monitor windings, promotions) |
