# Phase 7 record (branch `phase-7-extended-elements`) — ✅ COMPLETE

> Archived from `STATUS.md §1e` (2026-07-03) to keep the live handoff lean.
> This is the per-WP roll-up (one-line-per-step summaries). The deeper per-step
> detail lives in the sibling `phase-7-wp{1..7}.md` archives; the code and tests
> are the contract. Phase 7 is COMPLETE + gate-green on the branch but **NOT
> merged to `main`** (per-phase merge = explicit-request-only HARD STOP).

Execution plan: **`PHASE7_PLAN.md`** (WP7.1–WP7.10). Per-WP cadence — the full
ritual in `PHASE7_PLAN.md §0`, run autonomously per step: gate green → update
`STATUS.md` + commit → `/audit-code <step scope>` → fix + commit → `/audit-tests
<step scope>` → fix + commit → **full `STATUS.md` review + sync + archive-cleanup**
+ commit → **then** stop for confirmation.

**WP7.1 (Line constants & geometry) — ✅ COMPLETE (steps 1–5), gate-green.** The
detailed per-step records are archived at
[`phase-7-wp1.md`](phase-7-wp1.md). In brief:
- **step 1** — the Carson line-constants engine `support/line_constants/`
  (`LineConstants` + OH/CN/TS/cable specializations, `Calc(f)` → Z/Yc, Kron),
  **frequency-parameterized — the WP7.6 harmonics hook** (`z_matrix(f)`/`yc_matrix(f)`).
- **step 2** — the catalog classes `WireData`/`CNData`/`TSData` (`conductor_data/`),
  `LineSpacing`, `LineGeometry` (object + per-conductor edit state machine +
  `UpdateLineGeometryData`/`CalcMatrices`); new prop kind `PropType::ObjectRefArray`.
- **step 3** — Line's `geometry=`/`spacing=`/`wires=`/`cncables=`/`tscables=` Carson
  path (the `total_z_path` branch in `CalcYPrim`; total Z/Yc with length+units folded in).
- **step 4** — geometry/cable corpus feeder migration (`solvable_now` 17→35), incl.
  the new `Set EarthModel=Carson|FullCarson|Deri` option and the EPRI/ADiakoptics
  power-floor fix (`c7c6649`/`6d3b9ac`, the 3 meshed cases, `solvable_now` 32→35).
- **step 5** — the targeted offline golden `phase7/line_geometry*.json`
  (`gen_phase7.py` + `golden_phase7.rs`, 5 scenarios pinning the Carson Line YPrim
  entry-by-entry — the focused regression guard the live gate doesn't replace).
- **Tracked-open (un-pinned, needs investigation):** the **plural-cable**
  `cncables=`/`tscables=` active-conductor reset diverges between the vendored
  `LineGeometry.pas` (`istop`) and the pinned 0.14.5 binary (`Cond=1`); the scalar
  CN/TS *data* paths are covered, so the plural forms stay un-pinned pending that
  source/binary reconciliation (full note in the archive).

**WP7.2 (Protection) — ✅ COMPLETE.** Steps **1 (`Fault`), 2a (`SwtControl`),
2b (`Fuse`), 2c (`Recloser`), 2d (`Relay`), 3 (reliability activation), 4
(protection gate + corpus migration) done + gate-green**. Full per-step records
(decisions, audits, gate detail) archived at
[`phase-7-wp2.md`](phase-7-wp2.md). In brief:
- **step 1 — `Fault` (`pd/fault.rs`):** an uncoupled multi-phase **conductance**
  branch (`G=1/r` / `Gmatrix`) + the FaultStudy input (WP7.9); `ElemKind::Fault` +
  `Circuit.faults`, `check_fault_status`/`reset_faults` control-loop wiring.
- **step 2a — `SwtControl`:** a manual whole-terminal switch; lands the generic
  `set_terminal_closed` + `RefAction::SetSwitchClosed` and the **dirty-edge rule**.
- **step 2b — `Fuse` (`pd/fuse/`):** per-phase TCC; **`TccCurveObj::get_tcc_time`
  ported**, per-conductor `set_conductor_closed`, `MappedStringEnumArray`.
- **step 2c — `Recloser`:** whole-terminal trip + reclose to `Shots`, fast→delayed;
  `PropFlags::ARRAY_MAX_SIZE` + integer-dump `VALUE_OFFSET`.
- **step 2d — `Relay`:** the general control (`Relay.pas`, 9 `Type=` sub-types);
  `Current`/`Voltage`/`ReversePower`/`46`/`47`/`Distance`/`DOC` live, `Generic`/`TD21`
  parse+dump but defer `Sample` to WP7.7 (`NOT_PORTED`). `get_ov_time`/`get_uv_time`,
  `PropFlags::ALLOW_NONE`; event log gated on `ShowEventLog`.
- **step 3 — reliability activation:** enabled Relay/Recloser/Fuse set
  `Flg.HasOCPDevice` (+ `HasAutoOCPDevice` for the auto-reclosers) via a deferred
  `RefAction::SetOcpDevice`; `GetOCPDeviceType` live; the Phase-6 `RelCalc`
  SAIFI/SAIDI/section math runs (no more #52902 abort on a protected zone).
- **step 4 — protection gate + corpus migration:** the targeted golden
  `phase7_protection/*.json` (`gen_phase7_protection.py` + `golden_phase7_protection.rs`,
  **5 trip/reclose scenarios** — event log line-for-line + per-step + final-state vs
  the pinned oracle, reproduced exactly first-run); the **`Open`/`Close` exec verbs**
  ported (`do_open_close_cmd`, reusing the protection switching machinery; 4
  oracle-pinned tests); **`civanlar` + `IEEE_519` (SwtControl) migrated into
  `solvable_now` (35→37)**, `COVERAGE.md` refreshed. *audit-code:* faithful, fixed
  the `set_/get_conductor_closed` guard (`Nphases` → `Nconds`, matching Pascal's
  `Fnconds` — the new `Open` neutral-conductor path) + a unit test. *audit-tests:*
  real oracle-pinned gates; strengthened the golden with element-name-set equality.
  **Tracked-open:** `DG_Prot_Fdr.dss` compiles now but the live system Y diverges
  ~3e-5 rel at a **line** node — a WP7.1 Carson line-constants precision item (not a
  protection/`Open` regression), parked in `needs_investigation`. lib **502 → 514**
  across steps 2d–4.

**WP7.3 (DER A: DynamicExp + InvBasedPCE + PVSystem) — ✅ COMPLETE.** Full per-step
records (decisions, audits, gate detail) archived at
[`phase-7-wp3.md`](phase-7-wp3.md). In brief:
- **step 0 — `DynamicExp` (`general/dynamic_exp.rs`):** the user-defined diff-eq
  catalog object + its RPN expression interpreter (`InterpretDiffEq` → a flat `cmds`
  array, `SolveEq` stack machine over `[value, derivative]` memory); registered
  before Generator/PVSystem/Storage. Numeric pinning is spec-pinned here (the oracle
  exposes no `cmds`/`SolveEq` outside a dynamics run, WP7.7). Gate: 9 oracle props
  scenarios + 13 interpreter unit tests. lib **514 → 527**.
- **step 1 — `InvBasedPceData` (`pc/inv_based_pce.rs`):** the shared inverter PC base
  (`TInvBasedPCE` + the scalar `TInvDynamicVars`), an abstract base PVSystem/Storage
  embed (flattened like `GenVars`); the three power-flow shared methods
  (`StickCurrInTerminalArray`, `Get_Presentkvar`, `UsingCIMDynamics`). The per-phase
  dynamics arrays + GFM are deferred to WP7.7. Gate: 6 spec-pinned unit tests. lib
  **527 → 534**.
- **step 2 — `PVSystem` (`pc/pvsystem/`) + steps 3–4 (zone + gate):** the power-flow
  PV element on the Generator template with the InvBasedPceData base embedded
  (`ComputePanelPower` → `ComputeInverterPower`'s clamp cascade → `kWOut_Calc`;
  `SetNominalDEROutput`; the two models + the `ForceBalanced` path;
  registers/TakeSample); `ElemKind::PVSystem` + zone admission. Fixed a **real**
  Monitor mode-3 metered-kind bug (PVSystem now classifies as `PcElement`) and a GFM
  silent-degradation (now a loud pre-solve abort, WP7.7). GFM/harmonics/dynamics/
  UserModel/`MakePosSequence` deferred. Gate: `props/pvsystem.json` (10 scenarios) +
  goldens `phase7/pvsystem_{snapshot,curves,clamps}`; **corpus 37 → 44** (7 PVSystem
  cases). lib **534 → 543** (incl. audit follow-ups).

**WP7.4 (DER B: Storage + StorageController) — ✅ COMPLETE.** Full per-step records
archived at [`phase-7-wp4.md`](phase-7-wp4.md).
In brief:
- **step 1 — the `Storage` element (`pc/storage/`):** `TStorageObj` (the largest PC
  element) on the Generator template with the InvBasedPceData base — the
  charge/idle/discharge state machine (`FState`) + the integrated SOC
  (`kWhStored`/`%stored`, advanced in `EndOfTimeStepCleanup` via `UpdateStorage`, with
  the efficiency-curve `ComputeDCkW`/`QuadSolver` DC solve). Registered
  (`ElemKind::Storage`, `is_zone_pce`, the Monitor mode-3 + GFM-guard mirrors of
  PVSystem). GFM/harmonics/dynamics/UserModel deferred. Gate: `props/storage.json` +
  goldens `phase7/storage_{snapshot,clamps,daily,daily_charge}` (the SOC trajectory
  exact). **Corpus stays 44.** lib **543 → 557**.
- **step 2 — the real `StorageController` (`control/storage_controller/`):** replaces
  the WP6.8 parse-only skeleton with `MakeFleetList`, the `SetFleet*` helpers + fleet
  aggregates, `GetControlPower`/`GetControlCurrent`, and all the `Sample` dispatch
  modes (`DoLoadFollowMode` Peakshave/Follow/Support/I-Peakshave, `DoTimeMode`,
  `DoScheduleMode`, `DoLoadShapeMode`, `DoPeakShaveModeLow`) + `DoPendingAction`/
  `Reset`. The fleet resolves lazily through a `StorageDispatchEnv` (the GenDispatcher
  pattern). One `TODO(compat)` (the `if not FleetState = …` precedence bug) +
  SeasonalRating NOT_PORTED. Gate: goldens `phase7/storagecontroller_{daily,peakshave}`
  + 19 mock `sample_*` tests + 2 exec tests. **Corpus stays 44.** lib **557 → 572**.
- **The YPrim-rebuild fix** (post-audit): a Storage state flip (idle→discharging)
  changes its Norton `Yeq` but the port never propagated `yprim_invalid` to
  `system_y_changed`, so the solve ran a stale idle YPrim against the discharging
  injection (~1.8e-6 drift + an extra iteration). Restored that side effect at the
  StorageController dispatch env and `Storage::inj_currents` (via a new
  `InjCtx.system_y_changed`); the snapshot golden now matches bit-for-bit. **Scope is
  Storage-specific** — PVSystem/InvControl dispatch kvar/kW setpoints, not discrete
  state, so they never invalidate YPrim.

**WP7.5 (DER C: InvControl + ExpControl) — ✅ COMPLETE (steps 1–4).** Full
per-step records (incl. the two real-port-bug write-ups and the Storage
smart-inverter follow-up) archived at
[`phase-7-wp5.md`](phase-7-wp5.md). In brief:
- **step 1 — `RollAvgWindow` (`control/roll_avg_window.rs`):** the fixed-capacity FIFO
  with O(1) running sums backing InvControl's volt-var/DRC rolling-average voltage;
  ported 1:1 (incl. the faithfully-reproduced asymmetric `accum_sec` drift, whose only
  reader is dead upstream — a plain comment, not `TODO(compat)`). 4 spec-pinned tests.
  lib **572 → 577**.
- **step 2 — `InvControl` (`control/inv_control/`):** the single largest unit in the
  phase (`InvControl.pas`, 3586 lines), ported across sub-steps — **2a** parse-only
  skeleton (34 props + 7 enums + `ValidateXYCurve` + MakeLike), **2b** VOLTVAR, **2c**
  VOLTWATT + VV_VW, **2d** DRC + VV_DRC (+ the `IntervalUnits` time-suffix parse),
  **2e-i** WATTPF + WATTVAR, **2e-ii** AVR, **2e-iii** LPF/RiseFall rate-of-change +
  the explicit-`MonBus` path — on the StorageController clone-out `InvDispatchEnv`
  dispatch pattern (the fleet resolves lazily; the terminal bus is resolved at
  parse-time edit-completion). Storage AVR/WATTPF/WATTVAR ported to working (Storage
  VOLTWATT/VV_VW stay loudly guarded). Gate: `props/invcontrol.json` (20 scenarios) +
  a large `phase7/invcontrol_*` golden family (per-mode, daily, 24h, LPF/RiseFall,
  MonBus, Storage — and the per-step monitor comparison now runs on every multi-step
  phase7 golden) + many mock-env tests; **corpus 44 → 83**. lib **577 → 623**. **Two
  real port bugs found + fixed here** (each a [[dont-rationalize-conditioning]]
  instance): the cross-step `FFlagVWOperates` latch (the missing `UpdateInvControl`
  per-step reset — daily VOLTWATT diverged ~kW) and the missing `LoadsNeedUpdating :=
  TRUE` after `DoPendingAction` (without it AVR's iter-2 read a stale kvar = 0 →
  `DQDV = 0` → never converged).
- **step 3 — `ExpControl` (`control/exp_control/`):** the adaptive-`Vreg` volt-var
  control over a PVSystem-only fleet (`ExpControl.pas`, "adapted and simplified from
  InvControl") — the 14 props + the PVSystemList↔DERList sync, `MakePVSystemList`,
  `Sample`, `DoPendingAction` (slope-at-`Vreg` + `Qbias` → headroom/`PreferQ` curtail
  → `FOpenTau` low-pass → `DeltaQ_Factor` step), and `UpdateExpControl` (the per-step
  `Vreg` slew by `VregTau`). Gate: `props/expcontrol.json` + goldens
  `phase7/expcontrol_{daily,daily_preferq,24h,duty}` (the duty golden is the only
  `FOpenTau` LPF gate — daily runs under `CTRLSTATIC` gate it off) + 16 mock tests.
  One `TODO(compat)` (`FOpenTau := Tresponse/2.3026`, the truncated ln(10)). **Corpus
  stays 83.** lib **623 → 639**.
- **step 4 — the gate / corpus burn-down review (DER C).** No Rust code change (lib
  stays **639**); the gate is green and the DER-C corpus migration is confirmed
  **maximal**. A fresh `DSS_LIVE_CLASSIFY=1` re-probe of **all 48**
  InvControl/ExpControl-tagged `skipped_unsupported` cases (the bulk migrated in steps
  2b–2e; many of the rest still carried *stale* `unsupported_class=InvControl`/`PVSystem`
  tags from before the class landed) found exactly **1 newly-solvable** case —
  `…/PVSystem/CurrentkvarLimite/PV_currentkvarLimit_VV.dss` (a near-ideal-Thevenin
  snapshot: PVSystem + VOLTVAR InvControl regulating kvar from the file-set `kvar=500`
  to the curve zero-crossing at v≈1.0 pu, 56 control iters; full-model live-compared to
  the oracle, **corpus 83 → 84**). **Commit delta: 1 migrated + 39 stale tags
  refreshed** to the genuine current blocker (the other 8 of the 47 still-blocked cases
  already carried a correct `Export,Plot` tag). The **47 still-blocked** cases are
  blocked by Phase-8 / later work, **not** by DER-C numerics — current-state breakdown:
  **42** by a Phase-8 command (Export / Export+Plot — the Daily/MonitoredVoltage
  families), **3** GFM-mode-7 cases (`unsupported_command=BatchEdit; deferred=gfm-WP7.7`,
  also `File=`-blocked), **1** `ExpControl/Master.dss`
  (`unsupported_feature=file-backed-arrays`), and **1** `11_2_kWRatedViolation`
  (`deferred=storage-voltwatt-WP7.5`, the loud Storage-VOLTWATT guard from step 2c).
  `tools/corpus/COVERAGE.md` regenerated (84 → **25.1%** of entry points); the
  `corpus_manifest` bijection holds. *(The stale tags were a documentation-honesty fix
  only — they never affected the gate, which keys on the bijection + the `solvable_now`
  live compare.)*
  - **audit-code follow-up:** verdict **correct** — the full live gate matched all 84
    cases, the migrated VV case is non-trivial (the regulator moved kvar 500 → ~0 over
    56 iters, not an empty pass), the 39 refreshed tags each match the genuine re-probed
    Rust-engine error, and no hidden-migratable DER-C case was left behind (all 40
    InvControl/PVSystem candidates still error; the only solvable one is migrated). Two
    doc-only fixes applied: (1) the **GFM tags** were committed as plain
    `unsupported_command=BatchEdit` (the classify report truncates the reason at 400
    chars, dropping the `mode=7 … (GFM)` clause my appender keyed on), out of sync with
    this record's claim — re-set to `unsupported_command=BatchEdit; deferred=gfm-WP7.7`
    with a hand-transcribed full-blocker note; (2) this record's accounting was
    sharpened to separate the *commit delta* (1 migrated + 39 retagged) from the
    *current-state* family breakdown. **Surfaced-not-fixed (out of DER-C scope, tracked
    for a future corpus-hygiene pass):** ~16 *non*-DER-C `skipped_unsupported` cases
    (line-constants / `Show` / `Open`/`Close`) now compile+solve clean on the Rust
    engine but still carry possibly-stale tags — a clean Rust compile ≠ migratable (the
    gate also needs the oracle full-model match), so these need their own
    `DSS_LIVE_CLASSIFY` re-probe, not a blind migration.
  - **audit-tests follow-up:** verdict **sound + strictly additive** — the new case
    adds real verification (the harness pins the exact 56 control iters Rust↔oracle and
    the full unrelaxed model compare), it is **not flaky** (the gate ran green twice,
    84/84), and nothing was weakened (no tolerance loosened, no case removed/downgraded,
    the depth guard + bijection hold). **Minor (recorded, no fix):** the near-ideal-source
    `currentkvarLimit` *family* is borderline as a class (a sibling under
    `…/NewFeatures/varCapability/` is parked `live_mismatch_near_ideal_source`), but the
    migrated `_VV` variant sits in its stable zone — VOLTVAR drives Q→0 at v=1.0 pu so
    the reactive source current is ~1e-4 A and the ill-conditioned-Y mismatch never
    amplifies past tolerance (the parked siblings force a fixed `kvar=` → ~20 A reactive
    → the mismatch that parks them). No offline golden was added (the live gate is the
    pin); **no fix needed**.
- **next:** WP7.6 (Harmonics) — the first cross-cutting solve mode.

**WP7.6 (Harmonics) — ✅ COMPLETE (steps 1–3).** Full per-step records (decisions,
the two real-port-bug write-ups, the `capture_element` oracle-quirk investigation,
and all six audit follow-ups) archived at
[`phase-7-wp6.md`](phase-7-wp6.md). In brief:
- **step 1 — current-source family (VSource + Load) + the solve-mode driver:**
  `SolveHarmonic`/`SolveHarmonicT`, `InitializeForHarmonics` + the in-memory
  fundamental save/restore, `Spectrum.SetMultArray`/`GetMult`, the `harmonic =
  frequency/fundamental` fix, the VSource short-at-harmonics branch, and the Load
  `InitHarmonics`/`DoHarmonicMode` + the harmonic YPrim `%SeriesRL` split (the ~40%
  load-admittance bug the golden caught, not the smoke test). lib 639 → 650.
- **step 2 — Thevenin DER family (Generator/PVSystem/Storage):** each a voltage
  source behind its subtransient reactance — `InitHarmonics` (Yeq + the
  `Vthevharm`/`ThetaHarm` capture) + `DoHarmonicMode` (spectrum-scaled,
  phase-rotated injection through YPrim) + the harmonic `CalcYPrimMatrix` Y=Yeq
  branch + the `SetNominalGeneration` harmonic guard; the `guard_unported_harmonic_der`
  removed. The `capture_element` `Powers`-before-`Currents` swap pins the oracle's
  consistent harmonic power past a confirmed upstream stale-`Iterminal` engine bug
  (write-up in `investigations/`, git-ignored). lib 650 → 653.
- **step 3 — monitor harmonic header + the `Set mode=` reset + the corpus
  burn-down:** `ClearMonitorStream` labels the two time columns `Freq`/`Harmonic`
  in harmonics mode (offline-gated — the C-API `Monitors_Get_Header` strips them),
  and the `Set Mode=` handler now runs the full Pascal `Set_Mode` reset tail
  (monitors + meters ahead of faults + controls). Harmonics corpus burn-down is
  **0 migratable** — all 4 decks are Phase-8 (`Export`) / `Isource` /
  FaultStudy-blocked, not harmonics-blocked (2 stale `Swtcontrol` tags refreshed).
  lib 653 → 656; golden_phase7 **60**; `solvable_now` **84**.

**WP7.7 (Dynamics core) — ✅ COMPLETE (steps 1–4).** Full per-step records (decisions,
the real-port-bug write-ups — the `Open`-verb no-op, the `set_ITerminalUpdated`
stamp sweep, the per-step InvControl `FFlagVWOperates` reset — the
dynamics-tolerance reviews, and every audit follow-up) archived at
[`phase-7-wp7.md`](phase-7-wp7.md). In brief:
- **step 1 — the `SolveDynamic` predictor/corrector driver**
  (`solution/solution/dynamics.rs`): `SolveMode::Dynamic` → `solve_dynamic`, the step
  loop over `DynaVars.h` (`IntegratePCStates` + the pre-`set_mode`
  `calcInitialMachineStates` entry hook), and the Load `GENERALTIME`/`DYNAMICMODE`
  `SetNominalLoad` arm. lib 656 → 659.
- **step 2a — Generator dynamics + Monitor mode 3 + the `Open`-verb fix:** the classic
  (`DynamicEqObj = NIL`) shaft-swing machinery (voltage source behind `Zthev`,
  trapezoidal `Speed`/`Theta`), Monitor mode 3's real sample body, and a real
  `Open class.name` (omitted `term=`) no-op bug fix. Oracle-pinned on Kundur Ex.13.1
  (steady + fault + the full undamped swing). lib 659 → 662.
- **step 2b — PVSystem/Storage GFL inverter dynamics** (`InvDynamics.TInvDynamicVars`):
  the shared per-phase PI current loop + the 22/34-var mode-3 interface + a latent
  Storage SOC-in-dynamics fix; oracle-pinned on 4 PV/Storage decks. lib 662 → 666.
- **step 3a — IndMach012** (`pc/ind_mach012/`): the symmetrical-component induction
  machine (slip-Newton power flow + voltage-behind-`Zsp` dynamics, 22 mode-3 vars).
  Found + fixed the missing `set_ITerminalUpdated` stamp (a stateful extra slip step
  first mis-filed as "conditioning" — a [[dont-rationalize-conditioning]] catch),
  then swept the stamp across all 5 model-contribution sites. lib 666 → 672.
- **step 3b — DynEqPCE integration for the Generator** (`pc/dyneq_pce.rs`
  `DynEqPceData` + the edit-loop `ParseDynVar` fallback): a Generator driven by a user
  `DynamicExp` (`DynamicEq=`/`DynOut=`/inline initializers) instead of its built-in
  shaft model; oracle-pinned on the Kundur DynExp deck, reproducing the classic gate's
  physics exactly. Plus the two-part dynamics-tolerance review (all monitor pins
  tightened to 1e-6; the `dSpeed`/`dTheta` cancellation-floor residuals pinned against
  the oracle's actual value, not ≈0). lib 672 → 676.
- **step 3b cont. — DynEqPCE integration for the inverters (PVSystem + Storage):**
  `InvBasedPceData` now embeds the shared `DynEqPceData` (dropping its bare
  `DynamicEq`/`DynOut` fields), so PVSystem/Storage get the same machinery as the
  Generator (the `DynEqPce` trait, `parse_dyn_var`, `DynOut` resolution, the
  `DynamicEq=` sizing side effect). The per-phase `InitStateVars`/`IntegrateStates`
  `DynamicEqObj <> NIL` branches (`it[i]`/`dit[i]` ↔ `DynOut[0]`; the inverter
  calc-value overrides `2`→`Vgrid[i].mag` / `4`→nothing / `10`→`RatedVDC` /
  `11`→`SolveModulation`+`m[i]`, else `Get_PCE_Value`) + the DynExp
  `NumVariables`/`VariableName`/`GetAllVariables`/`Get_Variable`/`Set_Variable`
  interface. Oracle-pinned on self-contained PV/Storage GFL-DynExp mode-3 decks (the
  corpus `myDiffEq`/`myDiffEq2` filter equations) — steady + the two fault/disturbance
  gates + the PV sample-0 step — matched first-run, no fudging. *audit-code:* faithful
  (no behavioral deviation); fixed 8 stale `NOT_PORTED: DynamicEqObj` docs (4 inverter
  + 4 Generator). *audit-tests:* sound + non-vacuous (mutation-verified: a wrong
  readback slot / skipped `SolveModulation` each fail on both PCEs); added the two fault
  gates + the PV sample-0 pin. lib 676 → 680; `solvable_now` **84** (the corpus
  GFL_IEEE123 DynExp deck is a daily/`Plot`-blocked Phase-8 case — no migration).
- **step 4 — dynamics gate finalize + corpus burn-down (WP7.7 COMPLETE).** The
  focused dynamics gate is **`exec/tests/dynamics.rs`** (the comprehensive
  oracle-pinned mode-3 + fault tests for Generator / PVSystem / Storage / IndMach012
  / the DynExp variants — already the gate for steps 2a–3b cont.); a separate
  `phase7/dynamics*.json` command-replay golden would be redundant (the harness pins
  power-flow/control monitors, not dynamics mode-3 — the exec tests are the stronger
  guard). **Corpus burn-down = 0 migratable** (recon-confirmed): all 20 dynamics-mode
  corpus decks are blocked by a Phase-8 verb or a deferral, **not** by the dynamics
  engine — e.g. `DistanceRelayTest` *converges* on Rust (2 iters) and is blocked only
  by trailing `Plot`; the others by `var`/`@Zbase` (Kundur), `BatchEdit` (GFL_IEEE123),
  `MakeBusList`/`Plot` + `LoadShape action=normalize` (InductionMachine), the GFM mode
  (deferred), the `WindGen` class (Phase 9), or an oracle-side missing data file.
  `solvable_now` stays **84**. **WP7.7 (Dynamics core) is COMPLETE** (steps 1–3b cont.
  + this finalize). The **GFM grid-forming inverter mode** stays deferred (NOT_PORTED
  loud abort across Generator/PVSystem/Storage `DoDynamicMode`).
  - **Tracked-open (Generic/TD21 relay Sample).** A WP7.2 carry-forward: the dynamics
    machinery these need landed in WP7.7, so both are now *portable*, but they stay
    deferred — every Generic/TD21 (and Distance) corpus deck is Phase-8 `Plot`-blocked
    so they can never enter `solvable_now`, and the corpus-backed WP7.8 classes take
    precedence. GenericLogic is ~26 Pascal lines (reads `MonitoredElement.Variable[idx]`
    via the now-live state-var interface); TD21Logic is ~253 lines (a ring-buffer
    time-domain distance relay). The `NOT_PORTED` message text was updated to the
    honest framing (Phase-8 `Plot`-blocked, not "needs WP7.7"). Revisit in a focused
    follow-up or once Phase-8 `Plot` lands a no-op.
- **next:** WP7.8 (VCCS, UPFC + UPFCControl, VSConverter, ESPVLControl) — the
  converter/FACTS dynamics family.

**WP7.8 (Converter/FACTS family) — ✅ COMPLETE.**
- **VSConverter (`pc/vs_converter/`) — done, gate-green.** A 2-terminal AC/DC bridge
  (power-flow only, no dynamics state): the first `phases-Ndc` conductors are AC (a
  voltage source `Vdc·0.353553·m0∠d0` behind `Rac+jXac`, a `YPrim_series` block), the
  last `Ndc` are DC (a power-balance current source `Idc = Pac/|Vdc|` clamped to
  `±IDCMax·kW/kVDC`). 19 props + the `VSCMode` enum (the 5 modes parse/dump but Pascal
  `GetInjCurrents` only ever uses the fixed `m0/d0` — no mode-dependent behavior).
  **Proven upstream oracle bug (NOT reproduced).** `VSConverter.GetCurrents` →
  `GetInjCurrents(ComplexBuffer)` self-aliases `YPrim.MVMult(Curr, ComplexBuffer)`
  (`Curr == ComplexBuffer`) then re-reads the post-mult buffer for the `Pac` estimate,
  so the oracle's *reported* converter currents violate KCL (oracle-probed:
  |I_ac| ≈ 1248 A self-report vs the physical ≈ 390 A; the oracle's *source* current is
  the correct 390 A and ≠ −(self-report)). The port computes the physically-correct,
  KCL-consistent current and deliberately does **not** reproduce the self-report bug
  (cf. WP7.6 "не порти баг эталона"). Gate: `exec/tests/vs_converter.rs` pins the
  oracle's correctly-reported **source** currents + the KCL tie + the DC power-balance +
  the term-2 series mirror (a non-vacuous oracle gate that sidesteps the buggy
  self-report); + `props/vsconverter.json` (3 scenarios). **No corpus migration** — the
  3 VSConverter corpus decks compare the oracle's buggy currents (`vsc0/vsc1test` →
  `skipped_oracle_issue`) or don't converge on either engine (`vsctest`, near-short).
  lib 680 → **681**; `solvable_now` 84. (Fork-drafted; oracle bug + numerics
  independently re-verified in the main loop before commit.)
- **VCCS (`pc/vccs/`) — done, gate-green.** The HW-inverter voltage-controlled
  current source — a full 1:1 port incl. the z-domain ring-buffer filter dynamics.
  Power flow: ideal current source (`YPrim = 0`) injecting `BaseCurr` at the
  terminal-voltage angle (the `BP1`→scale→`BP2` PWL map of the pos-seq voltage,
  XYcurve refs). Dynamics: **both** the time-domain **waveform** ring-buffer path
  (`InitStateVars`/`IntegrateStates`, predictor/corrector via `IterationFlag`) **and**
  the **RMS/PLL** path (`InitPhasorStates`/`IntegratePhasorStates`, `RmsMode=true`) +
  the 6-var mode-3 interface; the ring buffers (`z`/`whist`/`zlast`/`wlast`/`y2`) kept
  1-indexed so the error-prone `MapIdx`/`OffsetIdx` wraparound ports verbatim. 13 props.
  `MakePosSequence` NOT_PORTED (the shared deferral); no oracle bugs found (KCL-clean).
  New `SysCtx.dyna_t` (`DynaVars.t`). **Gate:** `HWtest.dss` (snapshot) migrated to
  `solvable_now` (full-model live oracle match) — `solvable_now` 84 → **85**; +
  `exec/tests/vccs.rs` 3 oracle-pinned mode-3 dynamics tests (HWDyn waveform +
  HWPLL/HWPLL3 RMS, each pinning the t=0.1 s fault transient — the discriminating
  ring-buffer check; **independently re-verified in the main loop** by re-probing
  dss-python 0.15.7 — the HWDyn sample-50 fault transient matched the pins bit-for-bit)
  + `props/vccs.json`. `HWDyn`/`HWPLL`/`HWPLL3` stay `skipped_unsupported` (built-in
  `set mode=dynamic; solve` + `plot`, not snapshot-gateable); `DG_Prot_Fdr` now
  converges (only `plot`-blocked). lib 681 → **687**. (Fresh-agent-drafted; numerics
  re-verified before commit.)
- **UPFC + UPFCControl (`pc/upfc/` + `control/upfc_control/`) — done, gate-green.**
  The unified power-flow controller (PC element, power-flow only — the `Sr0`/`Sr1`
  shift registers are persistent control state, not differential) + its control
  element. UPFC: 17 props, the series `Xs` YPrim block, the 5-mode `GetOutputCurr`
  (dead-band / `VpqMax`-clamp / loss-curve; mode 1 = voltage regulator is what the
  corpus exercises), `GetInputCurr`/`CalcUPFCPowers`/`CalcUPFCLosses`, 14 mode-3 vars.
  UPFCControl: the `UPFCList` + `CheckStatus`→`Sample`→`UploadCurrents` control-sweep
  coupling (a dynamic fleet via a `UpfcDispatchEnv`, the GenDispatcher pattern; lazy
  list resolution). New `exec/view.rs::element_variables` (live f64 `AllVariableValues`
  analogue). **4 proven upstream oracle quirks, each handled** (oracle-probed): (1)
  *not reproduced* — a **2nd UPFC crashes** the oracle (Access Violation —
  `TUPFCObj.Create` casts the first UPFC to `TUPFCControlObj`, UB) → no UPFC MakeLike
  scenario; (2) *not reproduced* — `MakeUPFCList`'s name-list branch is dead+broken
  (clears then reads `FUPFCNameList`); (3) ***reproduced faithfully*** — the loss-curve
  `MakeLike` self-assign no-op (`UPFCLossCurveObj := UPFCLossCurveObj`): the curve is
  **not** copied to a `like=` UPFC, matching upstream (untestable — a 2nd UPFC crashes
  the oracle and `props/upfc.json` carries no `like` scenario; prose-doc, not
  `TODO(compat)`); (4) *not reproduced* — mode-3 monitor records nothing in a snapshot
  (`SampleCount=0`) → the 14 vars are pinned against the live f64 `AllVariableValues`,
  not the empty f32 channel (CLAUDE.md). **Convergence floor proven, not fudged:**
  `Vbin`/`Vbout` are mid-iteration snapshots, so at the default 1e-4 tol the engines
  stop ~4e-5 rel apart in the convergence band — but tightening to **1e-12** collapses
  the gap (both reach the identical fixpoint `Vbin = 236.41620285` to 12 digits in the
  **same 17 iterations**, oracle-probed), the CLAUDE.md proof of a shared fixpoint; the
  gate pins the tight-tol fixpoint. **Gate:** `exec/tests/upfc.rs` (transcribed
  UPFC_test_3 snapshot — `show`/`plot`-blocked so no corpus migration) pins the 14
  mode-3 vars + UPFC currents/powers + the controlled transformers' powers + the
  mode-3 header, all against dss-python 0.15.7 (**independently re-verified in the main
  loop**: 17 iters + all 14 vars + currents + powers matched the pins bit-for-bit) +
  `props/{upfc,upfccontrol}.json`. lib 687 → **693**. (Fresh-agent-drafted; numerics +
  oracle-bug claims re-verified before commit.)
- **ESPVLControl (`control/espvl_control/`) — done, gate-green. WP7.8 COMPLETE.**
  The storage/PV "local controller" — a **faithful no-op on circuit state** (oracle
  proven). The premise that `Sample` redispatches generators is a Pascal misread:
  `MakeLocalControlList` populates from *other ESPVLControl* objects, then `Sample`
  type-confuses each as a `TGeneratorObj` and writes `Gen.kWBase` onto another control's
  non-electrical memory (modeled as an unobservable `phantom_kw_base` field). There is
  no `kWLimit` prop (hardcoded 8000); only a `SystemController` acts; `Sample` never
  pushes a control action. Net: with vs without the control the solution is
  **byte-identical** and `ControlIterations` stays 1 (independently re-verified vs
  dss-python 0.15.7). Ported all 11 props + the lazy `MakeLocalControlList` (Ftype-1
  gate, name-list/scan-all, uniform weights) + `Sample`/`RecalcElementData` (err
  371/372)/`MakeLike`; the dead PVSystem/Storage pointer lists round-trip but never
  dispatch; `MakePosSequence` NOT_PORTED (shared deferral). Not a `TODO(compat)` — the
  type-confusion is dead/harmless upstream code with no golden-pinned value (prose-doc,
  per the convention). **Gate:** `exec/tests/espvl_control.rs` (6 synthetic oracle tests
  incl. the with==without byte-identity) + `props/espvlcontrol.json`. lib 693 → **711**.
  (Fresh-agent-drafted; the no-op + oracle behavior re-verified before commit.)
- **WP7.8 (Converter/FACTS family) COMPLETE** — VSConverter, VCCS, UPFC + UPFCControl,
  ESPVLControl all done, gate-green.
- **Retro audit (WP7.7 step 4 + all of WP7.8, done 2026-06-29).** The mandatory
  `/audit-code` + `/audit-tests` ritual steps (PHASE7_PLAN §3–4) were skipped from
  WP7.7 step 4 (`fb0be61`) through WP7.8 (`c8e23d0`); run retroactively here
  (inline, no agents — full Pascal-vs-Rust read of every element + the targeted
  tests, 31/31 green). **Verdict: faithful ports, no Critical/Major correctness
  bug.** Findings, all settled:
  - **WP7.7 step 4** — message-text + comment only (`relay/mod.rs` `Generic`/`TD21`
    NOT_PORTED loud-abort unchanged); no test files touched. Nothing to fix.
  - **VSConverter** — `GetInjCurrents`/`CalcYPrim`/`GetCurrents` 1:1 (incl. the
    one-iteration `ITerminal` lag, EPSILON, `VscMode` enum). **FIXED:** the loose
    `1e-3 rel` on 5-sig-fig pins in `vs_converter.rs` was a transcription artifact,
    **not** a bug — re-probed dss-python 0.15.7 at full f64 and the Rust source
    currents match the oracle **bit-for-bit (~1e-10 rel)**; the DC-source current
    to ~2e-12. The test now pins the full-precision oracle values at the standard
    **1e-6** current floor (1000× tighter), with the converter's own DC terminal a
    documented tight regression guard (the oracle masks it via the self-alias bug).
    No corpus deck still (the 3 decks stay `skipped_oracle_issue`).
  - **VCCS** — the strongest port: ring-buffer `MapIdx`/`OffsetIdx`, all 3 inj
    regimes + both dynamics paths verified; the local-`z_iu` accumulator proven safe
    (`MapIdx(iu-k+1)`, k≥2, never returns `iu`). Gate strong (`HWtest` live +
    waveform/RMS mode-3 1e-6). *Surfaced:* the `>3-phase` `FrmsMode` branch is an
    untested unreachable edge.
  - **UPFC/UPFCControl** — all 5 modes + `CheckStatus`/`checkPF`/`CalcYPrim`/the
    FPC short-circuit `or` in `Sample` faithful. *Surfaced, deferred (the one
    Major-level gap):* **only mode 1 (voltage regulator) is oracle-gated**; modes
    0/2/3/4/5 and the PF-compensation/`MonElm` path (`get_input_curr` 2/3/5) are
    ported loop-for-loop but behaviorally unverified (no corpus deck; would need
    synthetic probe decks). *Minor:* `calc_upfc_losses` silently returns `1.0` when
    no loss curve (Pascal NIL-crashes) — benign fallback, no corpus path. The
    `MakeLike` self-assign wording above (quirk 3) corrected this pass.
  - **ESPVLControl** — type-confusion no-op modeled correctly; tests are exemplary
    (redispatch fires both PDiff signs, cross-object + named-subordinate phantom
    writes, and `control_present_equals_control_absent` byte-identity — the no-op is
    *proven*, not rationalized). *Surfaced:* no-op gated only for snapshot solves
    (no multi-step corpus deck).
  - **Fixes applied:** (1) the VSConverter test tightened to full-precision oracle
    pins at 1e-6 (above — the only loose oracle tolerance in the range, empirically
    proven a no-bug); (2) the UPFC quirk-3 STATUS wording reconciled. **Genuinely
    still open (not "settled" — untested coverage):** the UPFC modes 0/2/3/4/5 +
    PF-compensation path (only mode 1 oracle-gated) — closing it needs synthetic
    probe decks. The remaining items (VCCS `>3-phase` RMS edge; ESPVL multi-step)
    are unreachable/zero-corpus edges justified by PHASE7_PLAN §2.6.

**WP7.9 (FaultStudy + AutoAdd + Feeder) — ✅ COMPLETE.**
- **step 1 — FaultStudy mode (`solution/solution/fault_study.rs`) — done, gate-green.**
  Ported `TSolutionAlgs.SolveFaultStudy` and its `TSolutionObj` helpers
  (`DisableAllFaults` → `SolveDirect` for the open-circuit Voc → `AllocateAllSCParms`
  → `UpdateVBus` → `ComputeAllYsc` → `ComputeIsc`). Each bus's `Zsc` is built column
  by column by injecting 1 A at each node and re-solving the **already-factored**
  system Y (`SparseSet::solve` reuses the cached LU), i.e. each `Zsc` column is a
  column of `Y⁻¹` restricted to the bus's nodes; `Ysc = Zsc⁻¹` (reusing
  `support/cmatrix` `Invert`, whose singular path matches Pascal — degenerate buses
  e.g. a delta-isolated zero sequence leave `Ysc` partially transformed, exactly like
  upstream); `Isc = Ysc·VBus`. New `Bus` fields `zsc`/`ysc` (`Option<CMatrix>`) +
  `allocate_bus_quantities`/`get_zsc1`/`get_zsc0`; new `exec/view.rs::bus_short_circuit`
  (dss-python `Bus.Zsc1`/`Zsc0`/`Isc`). `SolveFaultStudy` sets `LoadModel=ADMITTANCE`
  (faithful; no corpus FaultStudy deck has active loads). `MonteFault` still errors
  (no corpus case). **Gate:** `exec/tests/fault_study.rs` — a self-contained radial
  feeder whose bus `Zsc1`/`Zsc0` are the analytic series sums (e.g. b2 = source
  0.5+2.0j + line 0.2+0.6j = 0.7+2.6j), pinned to dss-python 0.15.7 along with the
  full-complex `Isc`. **Corpus 85 → 88:** the 3 `ShortCircuitCases` decks
  (`ieee37_SC_Currents`, `ieee34Mod2_SC_Case_II`, `IEEE123Master-SC`) classify
  **solvable** (full-model live oracle match — the post-study `NodeV` is the last
  `ComputeYsc` column on both engines and agrees). lib 711 → **712**.
  - **audit-code:** verdict **clean** (no blocker/major) — the port is loop-for-loop
    faithful (order, `ComputeYsc` indices + ground convention, single-LU factor reuse,
    bit-faithful singular invert, dynamics-entry timing all re-verified vs the oracle).
    Confirmed `LoadModel=ADMITTANCE` is **faithful but inert for `Zsc`**: Pascal
    `TLoadObj.CalcYPrim` runs identical code in the POWERFLOW/ADMITTANCE branches, so
    the load YPrim (already in Y from the snapshot) is LoadModel-independent. Fixed a
    `BusScView` doc nit (`vbus` is the stored `VBus`/Voc, **not** dss-python
    `Bus.Voltages`, which returns the live residual `NodeV`).
  - **audit-tests:** verdict **sound + non-vacuous** (every pin independently
    re-derived from dss-python 0.15.7). Both audits flagged that the **corpus
    migration validates the full power-flow model + FaultStudy mode behavior (node
    order, residual `NodeV`, Y, element I/P) but NOT the per-bus `Zsc`/`Ysc`/`Isc`
    deliverable** — that is the targeted test's job, and the Phase-8 `Export/Show
    FaultStudy` reports (WP8.3/8.4) will systematically gate the formatted Zsc/Isc on
    the corpus (a dedicated corpus Zsc capture now would duplicate that and need a
    bespoke tolerance for the near-singular delta-isolated `Zsc0`). **Fix applied:**
    extended `exec/tests/fault_study.rs` with a second oracle-pinned deck covering the
    paths the balanced anchor can't — a **single-phase** bus (the `n=1`
    `avg_off_diagonal`=0 branch + 1×1 invert), an **asymmetric** bus behind a
    full-matrix line (non-circulant `Ysc` → distinct per-node `Isc`, exercising
    `Ysc·VBus` + the `Zsc[j,i]` indexing), and a **delta-isolated** bus (near-singular
    `Zsc0`≈5.2e7j — the deliberately-ignored `invert()` failure path; pinned as the
    robust facts: well-conditioned `Zsc1` tight + `Zsc0` blows up + fault `Isc`
    matches). lib 712 → **713**.
- **step 2 — AutoAdd / MonteCarlo / LoadDuration / MonteFault — kept deferred (no
  port).** Corpus probe found **zero** decks using `mode=autoadd`/`A`, `mode=M1/M2/M3`
  (MonteCarlo), `mode=MF` (MonteFault), or `mode=LD1/LD2` (LoadDuration). Per the
  PHASE7_PLAN §2.6 empirical rule ("port only if a corpus case needs it"), each keeps
  the Pascal `Unknown solution mode.` error (the `dispatch.rs` catch-all, stale
  "Phase 5" suffix replaced with an honest "no corpus case" note + a comment naming
  the deferred modes). The `circuit/auto_add.rs` option skeleton (options round-trip)
  is unchanged. No code beyond the message/comment.
- **step 3 — Feeder — documented dead (no port).** Corpus probe found **zero**
  `New Feeder.` instantiations; `Feeder.pas` is largely dead upstream (Phase 6 found
  `DoFeederStuff` remnants dead). Nothing to port. **WP7.9 COMPLETE** (FaultStudy is
  the only real deliverable; AutoAdd/Feeder are empirical no-ops per the plan).

**WP7.10 (Phase 7 exit) — ✅ COMPLETE (docs/verification only, no code change).**
- **Marker sweep clean:** no `TODO`/`NOT_PORTED` orphan points at WP7.9/7.10. The
  remaining deferrals all have a documented home — **GFM** grid-forming mode (the one
  Phase-7-planned item descoped, tracked-open, NOT_PORTED loud abort) and the
  **Generic/TD21** relay `Sample` (tracked-open) — both **Plot-blocked with zero
  corpus payoff**; DLLs/UserModel = "never"; `MakePosSequence` = on-demand;
  AutoAdd/Monte/LD/Feeder = no corpus case (WP7.9); 39 `TODO(compat)` = the deliberate
  upstream-inexactness set wiped in the dedicated post-acceptance §6 pass (PORTING_PLAN
  §6), not now. 3 residual `TODO(WP7.7)` are unreachable-edge-case hardening notes
  (>3-phase dynamics, Model=6 UserModel generator).
- **Gate:** full three-command gate + the always-on live corpus gate (88 cases) green;
  `tests/corpus/COVERAGE.md` refreshed (solvable_now 85 → **88**, 26.3% of entry
  points; the WP7.9 burn-down = the 3 ShortCircuitCases decks).
- **No code/test audit:** WP7.10 changed only `STATUS.md` (docs/verification), so there
  is nothing for `/audit-code`/`/audit-tests` to review.
- **Phase 7 COMPLETE.** **next = Phase 8** (`PHASE8_PLAN.md` drafted). **Merge to
  `main` (`--no-ff`, per-phase convention) is the HARD STOP — explicit user request
  only; not done.**

**Phase-7 carry-forward (cross-cutting, beyond WP7.2):**
- **Dirty-edge discipline (all four controls + the `Open`/`Close` verbs).** Every
  trip/close/reset/Open forces conductors via `Closed[]` →
  `TDSSCktElement.Set_ConductorClosed` (`CktElement.pas:287`) sets `YPrimInvalid :=
  TRUE` → `SystemYChanged := TRUE` (`:240`) **unconditionally**, no
  change-comparison. So each raises `system_y_changed` **unconditionally** (or via
  an exact per-conductor check), **never** gated on a
  `terminal_all_phases_closed`/`is_closed` aggregate (a partial-open terminal
  otherwise slips a real change past the rebuild → stale Y); each control ships a
  partial-open fail-on-regression test (`d0addb4`/`d1f48231`), and the `Open` verb
  carries the Line/transformer round-trip guards.
- **Reliability (step 3).** OCP flags + `GetOCPDeviceType` + the live `RelCalc`
  SAIFI/SAIDI are in. The single-int `ocp_device_type` + single-flag model is exact
  for the realistic one-OCP-per-element case; the move/re-enable reassignment edge
  (a control redefined onto a different element, leaving the old element's flag
  stale) is **not** un-set — consistent with the existing controlled-element force
  model (the `SetSwitchClosed`/`SetConductorsClosed`/`Open` forces likewise never
  un-force a previous target). Not exercised by any gate.
- **Generic/TD21 Relay Sample logic deferred to WP7.7** (dynamics): the relay
  parses + dumps `Type=Generic`/`TD21` but the live sensing records a `NOT_PORTED`
  error. The corpus Distance/TD21 relay demos also need the dynamics solve mode.
