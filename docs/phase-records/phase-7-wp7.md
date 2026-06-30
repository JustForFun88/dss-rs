# Phase 7 — WP7.7 (Dynamics core) — archived per-step records

> **Archived from `STATUS.md`** (2026-06-29) to keep the live handoff lean. WP7.7
> (Dynamics core) is **🚧 IN PROGRESS** on the `phase-7-extended-elements`
> branch; steps 1, 2a, 2b, 3a, 3b, and 3b cont. are **done + oracle-pinned** (the
> `SolveDynamic` driver → Generator/PVSystem/Storage dynamics + Monitor mode 3
> → IndMach012 → the DynEqPCE user-`DynamicExp` integration for all three
> PCE families). **next = WP7.7 step 4** (dynamics corpus burn-down + the offline
> `phase7/dynamics*.json` golden); the GFM grid-forming inverter mode stays
> deferred. These are the frozen per-step records (decisions, the real-port-bug
> write-ups, the `set_ITerminalUpdated` stamp sweep, the dynamics-tolerance
> reviews, and every audit follow-up); the live `STATUS.md` §1e keeps the
> concise per-step summary + the active frontier.

---

**WP7.7 (Dynamics core) — 🚧 IN PROGRESS.**
- **step 1 — the `SolveDynamic` driver (`solution/solution/dynamics.rs`):** wired
  `SolveMode::Dynamic` → `solve_dynamic`, the predictor/corrector step loop over
  `DynaVars.h` (per step: `IncrementTime` → `DefaultHourMult` → predictor
  [`IterationFlag = NewTimeStep`, `IntegratePCStates`, `SolveSnap`] → corrector
  [`SameTimeStep`, …] → `MonitorClass.SampleAll` → `EndOfTimeStepCleanup`), with
  `SolutionInitialized` forced true so the inner power flow does not re-init per
  step. Added `IntegratePCStates` (`SolutionAlgs.pas` l.321 — the full PCElements
  list, no `Enabled` test) and `calcInitialMachineStates` (`Solution.pas` l.2156 —
  the dynamics-entry machine-state init, enabled-gated), wired into the
  `Set mode=dynamic` handler and fired only on a *fresh* entry into a dynamics mode
  from a solved circuit (`was_dynamic` capture). Critically it runs **before**
  `set_mode` commits `is_dynamic_model`/`h`/`mode` — Pascal's `OK_for_Dynamics`
  timing — so each machine's `InitStateVars`/`ComputeIterminal` captures its
  operating point from the power-flow state, not the dynamic Norton branch (the
  harmonics entry stays post-commit; only dynamics needs the pre-commit order).
  New no-op-default `CktElement` trait hooks `init_state_vars`/`integrate_states`
  (base `TPCElement` does nothing); `Solution.iteration_flag` set by the driver
  (surfaced to `SysCtx` in step 2 when a machine consumes it). Ported the Load
  `GENERALTIME`/`DYNAMICMODE` `SetNominalLoad` case (growth × load-multiplier,
  `ShapeFactor` 1+j1 under the `USENONE` `ActiveLoadShapeClass` default — the
  established Generator/Storage/PVSystem assumption; the old `_ =>` arm wrongly
  claimed Dynamic was unreachable + dropped the load-multiplier). Gate: 3 driver
  integration tests (`exec/tests/dynamics.rs`) — a static-circuit dynamics solve
  holds the snapshot fixpoint to 1 ppm across 5 steps + one monitor sample/step, a
  `loadmult=2` solve drops the loadbus voltage (proves the re-solve + covers the
  new multiplier arm), and the `OK_for_Dynamics` unsolved-entry guard. The focused
  oracle gate over a real dynamics machine is step 4. lib **656 → 659**;
  `solvable_now` **84**.
  - **audit-code follow-up:** verdict — no step-1 bug; the driver is a faithful 1:1
    `SolveDynamic` port (predictor/corrector order, `IterationFlag`, the
    `IntegratePCStates`-all vs `calcInitialMachineStates`-enabled-gated distinction,
    the Load `DYNAMICMODE` arm all confirmed against Pascal). Acted on three
    forward-risk items it surfaced: **(1)** the entry hook was moved **ahead of**
    `set_mode`'s mode commit (above) — it was post-commit, which would have made a
    step-2 Generator `InitStateVars` capture the dynamic-branch current instead of
    the power-flow one; **(2)** leaving dynamics mode does not yet
    `InvalidateAllPCELEMENTS` — marked `NOT_PORTED(WP7.7 step 2)` in `set_mode`
    (inert until a machine presents a mode-dependent Norton YPrim); **(3)**
    `preserve_node_voltages` is set but `build_y_matrix` does not yet honour
    `UpdateVBus`/`RestoreNodeVfromVbus` — marked `NOT_PORTED(WP7.7)` at the build
    site (pre-existing since WP7.6-harmonics; inert with no mid-step Y rebuild).
  - **audit-tests follow-up:** verdict — genuine guards, not smoke (loop-bound /
    clock / sample-count / unsolved-guard all real). Closed the one gap it found:
    the 1-ppm voltage-hold check is tautological for a static fixture (a no-solve
    driver passes it; only `is_solved` caught that), and the new Load `DYNAMICMODE`
    multiplier arm was exercised only at loadmult 1 — added
    `dynamic_mode_load_multiplier_moves_operating_point` (loadmult=2 → loadbus
    voltage must drop), which makes the re-solve observable and gives the arm its
    first non-trivial coverage. lib 658 → **659**.
- **step 2a — Generator dynamics state machinery + Monitor mode 3 + the `Open`-verb
  fix.** Ported the classic (`DynamicEqObj = NIL`) Generator dynamics: the shared
  state-var trait surface (`SysCtx.iteration_flag` + the no-op-default
  `num_variables`/`variable_name`/`get_all_variables`), 15 GenVars dynamics fields,
  and `generator/dynamics.rs` (`InitStateVars` — `Zthev` model-7/machine branch,
  `Yeq = Cinv(Zthev)`, 1-/3-phase `Edp`, `theta = cang(Edp)`, `w0`/`Mmass`/`D`,
  `Pshaft = -Power[1].re`; `IntegrateStates` — trapezoidal half-step, history seeded
  only on `NewTimeStep`; `DoDynamicMode` — voltage-source-behind-`Zthev` injection,
  model-7 PLL + current limit, neg/zero-seq, neutral; `CalcVthev_Dyn`/`_Mod7`; the
  6 GenVars variables). `calc_gen_model_contribution` now dispatches `DoDynamicMode`
  first under `is_dynamic_model`; the `CalcYPrimMatrix` `Y := Yeq` branch covers
  `is_dynamic_model || is_harmonic_model`. **Monitor mode 3** got its real
  sample/header body (`GetAllVariables` → `MeteredSnapshot.variable_names`).
  `DynamicEqObj`/`DynamicExp`, UserModel/ShaftModel DLLs and GFM stay NOT_PORTED.
  **Constant note:** `RadiansToDegrees`/`TwoPi` are full-precision (`180/PI`,
  `2*PI`) — the vendored 0.14.5 `DSSGlobals.pas` l.84-85 are the active defs (the
  `57.29577951` line above them is commented out); using the truncated value would
  *diverge* from the oracle (so **not** a `TODO(compat)` on this path).
  - **Real bug found + fixed (the `Open`/`Close` exec verb, not dynamics-specific).**
    `Open class.name` with an omitted `term=` was a full no-op (`set_terminal_closed(0,…)`
    rejected by the `terminal>=1` guard), so the element never opened. Pascal
    `DoOpenCmd` sets `ActiveTerminalIdx := Terminal` (`Set_ActiveTerminal`
    *ignores* a 0/out-of-range terminal, leaving the active terminal = terminal 1)
    then `Closed[Conductor] := FALSE` on that **active** terminal. Fixed
    `do_open_close_cmd` to mirror this: keep the active terminal when `term=` is
    omitted and open it (default terminal 1). Surfaced by the Kundur deck's
    `Open Line.Source_HT_2` (no `term=`); the prior WP7.2 Open/Close gates only used
    explicit terminals, so the bug was latent.
  - **Gate (the focused step-2a oracle gate, done now not deferred to step 4):** 3
    oracle-pinned Generator-dynamics tests on the canonical Kundur Ex.13.1 deck
    (`exec/tests/dynamics.rs`, dss-python 0.15.7): (1) **steady mode-3 trajectory** —
    all 6 GenVars channels hold the operating point across 1001 samples (Theta
    41.77272, Vd 1.1625859, PShaft 1.998e9, Freq 60) at 1e-5; (2) **fault response**
    — the 3-phase fault accelerates the rotor, Theta rises 41.77→48.48 monotonically
    matching the oracle; (3) **full swing** — fault cleared by `Open`ing the weaker
    line, the undamped (D=0) rotor swing min/max (24.18569 / 98.131889 deg) match the
    oracle to 5 digits (the regression guard for the `Open` fix). The post-Open
    divergence was *proven* a real bug (wrong-sign dSpeed at the first post-open step;
    line 2 kept carrying 1480 A instead of 0) — **not** conditioning, **not** the
    `UpdateVBus` path — and root-caused to the `Open`-verb no-op above. lib
    **659 → 662**; `solvable_now` **84** (the full Kundur deck migration is step 4).
  - **audit-code follow-up:** verdict **faithful** — the `InitStateVars`/
    `IntegrateStates`/`DoDynamicMode`/`Get_Variable` ports and the `Open` fix were
    confirmed line-for-line against Pascal (incl. the `active_terminal`↔`FActiveTerminal`
    persistence). Two **surfaced-not-fixed** edge cases (out of corpus, the pre-existing
    Generator error-swallowing convention — recorded, not a regression): a bare
    `Model=6` generator in dynamics, and a **>3-phase** generator in dynamics, push a
    NOT_PORTED error into `inj_currents`'s local `errors` vec, which is *dropped* — so
    the solve continues silently instead of Pascal's `SolutionAbort` (the >3-phase case
    additionally leaves `m_mass = 0`, so a following `integrate_states` would yield NaN).
    Both unreachable in the vendored corpus (1-/3-phase, `UserModel` un-configurable);
    the misleading "mirror the error path" comments were corrected to state this
    honestly, with `TODO(WP7.7)` to surface a loud abort if a case ever forces it. Fixed
    one cosmetic nit (the out-of-range `VariableName` returns `"ERROR"` like Pascal, not
    `""`). The model-6 dynamics message text differs from Pascal msg 5671 but is not
    oracle-pinned (left as the clearer NOT_PORTED wording).
  - **audit-tests follow-up:** verdict **sound + strictly additive** — the auditor
    independently re-ran the deck on the pinned dss-python 0.15.7 oracle and confirmed
    **every** pinned constant bit-for-bit (steady/fault/swing), and **proved the swing
    test is a non-vacuous guard for the `Open` fix**: reverting the fix moves the swing
    min/max by ~0.34/0.27 rel (≈3400× the 1e-4 tolerance) → the test fails. Tolerances
    tight, no smoke/skip/ignore, the deck transcription (inlined `@Zbase`, `enabled=no`
    for `Disable`) is empirically equivalent. No test fix needed.
- **step 2b — PVSystem + Storage grid-following (GFL) inverter dynamics.** Ported the
  classic (`DynamicEqObj = NIL`) inverter dynamics for both DER PCEs + the shared
  `TInvDynamicVars` machinery (`Shared/InvDynamics.pas`). **`inv_based_pce.rs`:** the
  per-phase arrays (`vgrid`/`dit`/`it`/`it_history`/`m`/`isp_delta`/`ang_delta`/
  `sf_mode_phase`) on `InvDynamicVars` + a `pi_ctrl: Vec<PiCtrl>` on `InvBasedPceData`
  (kept disjoint for the per-phase borrow), and `init_dyn_arrays`/`solve_modulation`/
  `solve_dynamic_step`/`get_inv_dyn_value`/`get_inv_dyn_name`/`set_inv_dyn_value`
  (`NUM_INV_DYN_VARS = 9`). **`pvsystem/dynamics.rs` + `storage/dynamics.rs`:**
  `InitStateVars` (PICtrl seed `kNum=0.9502`/`kDen=0.04979`/`kP`, `BaseZt`/`MaxVS`/
  `MinVS`/`MinAmps`/`iMaxPPhase`, `pctX`→50 default, `Zthev`/`Yeq`/`LS`, per-phase
  `Vgrid`/`it`/`m` seed; Storage gated on `FState = DISCHARGING`), `IntegrateStates`
  (the `it`/`dit`/`itHistory` trapezoidal current loop via `SolveDynamicStep`; PV
  recomputes `iMaxPPhase` from `PanelkW`; Storage MinVS/MaxVS idle-trip), and
  `DoDynamicMode` (the `topolar(iActual, Vgrid.ang)` injection; PV `it<=iMaxPPhase`
  clamp, Storage `MinAmps` cutoff + non-discharge idling current). The full mode-3
  state-variable interface: **PV 22 vars** (13 classic + 9 InvDyn), **Storage 34**
  (25 + 9) — incl. Storage loss getters (`get_inverter_losses`/`get_kw_chdch_losses`/
  `get_kw_total_losses`/`get_kw_desired`, `update_efficiency_factor`).
  `calc_{pvsystem,storage}_model_contribution` now dispatch `do_dynamic_mode` first
  under `is_dynamic_model` (Pascal `CalcPVSystemModelContribution` l.1999 order).
  GFM, the `DynamicEqObj`/`DynamicExp` path (step 3) and UserModel/DynaModel DLLs stay
  NOT_PORTED; `VDelta` (GFM-only) is absent.
  - **Real fix (latent WP7.4 simplification).** `update_storage` early-returned on
    `is_dynamic_model`, but Pascal `UpdateStorage` (l.2496) exits only for
    `IsDynamicModel AND IsUserModel`; user models are NOT_PORTED (always false), so the
    SOC **must** integrate during dynamics. Removed the early-return — the oracle's
    discharging-storage `kWh` drops 1000→999.98 over the run, which the new gate pins.
    (Unexercised before step 2b: WP7.4 ran only power-flow.)
  - **Compat note.** Storage `IntegrateStates` non-discharge `OFFVal` is uninitialised
    in Pascal when `Vgrid.mag < MinVS AND NOT ResetIBR`; ported as `0.0` + `TODO(compat)`
    (unreachable in the gated decks).
  - **Gate:** 4 oracle-pinned tests in `exec/tests/dynamics.rs` (dss-python 0.15.7,
    1e-4/1e-5): PV **steady mode-3** (all 22 vars; the duty rails at 1, `it` relaxes
    20.55→6.17, `di/dt`→0) + **safe-mode under fault** (Vgrid collapses <MinVS → `it`/
    duty→0, target→0.01); Storage **steady mode-3** (all 34 vars incl. the SOC
    trajectory + discharge/idle/total loss split; PI-ramped `it` 0→44.98, `kWOut`→500)
    + **idle-trip under fault** (State 1→0, output→0). lib **662 → 666**;
    `solvable_now` **84** (no corpus migration — every PV/Storage dynamics deck is
    also blocked on Phase-8 `BatchEdit`/`DynamicExp`-integration/GFM, recon-confirmed).
  - **audit-code follow-up:** verdict **mostly faithful** (variable interfaces, loss
    getters, node-ref indexing, iMaxPPhase, GFM/DynEq/UserModel deferrals, the
    `update_storage` SOC fix all confirmed line-for-line). Fixes: (1) **real
    deviation** — `SolveModulation` had an `ISP!=0` divide guard; Pascal
    (`InvDynamics.pas:190`) divides unconditionally → reverted to verbatim
    `iError/ISP` (the divergence is unobservable: `ISP==0 ⟹ PanelkW==0 ⟹ it→0 ⟹
    iError→0`, so guard and verbatim agree — an `irradiance=0` probe confirmed no
    discriminating trajectory, so no test added); (2) PV `Get_Variable(1)` now
    returns `PresentIrradiance` (`irradiance*ShapeFactor.re`), not raw `FIrradiance`;
    (3) dropped a spurious `TShapeValue` reset in PV `InitStateVars` (Pascal USENONE
    leaves it); (4) Storage `VariableName` out-of-range-high → `""` (Pascal), not
    `"ERROR"`; (5) a non-discharging Storage entering dynamics has empty per-phase
    arrays (Pascal skips `InitDynArrays` then derefs nil = crash, no oracle baseline)
    → added a no-op guard instead of a Rust panic. **Set_Variable decision:** the
    auditor flagged `set_pv_variable`/`set_storage_variable`/`set_inv_dyn_value` as
    dead code behind `#[allow(dead_code)]`; first dropped, then **restored** (1:1
    fidelity) and wired through a new `CktElement::set_variable` trait method — now
    reachable, no `#[allow]`. (Generator's symmetric `Set_Variable` is still absent —
    a pre-existing step-2a gap, noted for a later sweep.)
  - **audit-tests follow-up:** verdict **sound + non-vacuous** — the auditor
    re-derived every pinned constant from the pinned oracle and proved the Storage
    SOC test fails if the `update_storage` fix is reverted. Hardened two Minor items:
    pinned all 34 Storage mode-3 channels by value (was 21/34) and tightened the
    near-vacuous `it[0]` tolerance to an absolute band.
- **step 3a — IndMach012 (the symmetrical-component induction machine).** Ported
  `PCElements/IndMach012.pas` as `pc/ind_mach012/` (mod/solve/dynamics/accessors) on
  the Generator template, reusing `TGeneratorVars` (flattened as the `MachineData`
  shaft fields). Power flow: an equivalent-circuit motor whose slip floats to the
  shaft-power target (`Get_PFlowModelCurrent` + the fixed-slope `dSdP` slip-Newton in
  `CalcPFlow`; symmetrical-component `CalcModel`). Dynamics: a voltage source behind
  the transient reactance `Zsp` whose pos/neg-seq internal voltages `E1`/`E2`
  (`Integrate`, trapezoidal) and shaft speed/angle (`IntegrateStates`) are integrated;
  `InitStateVars` seeds them from the converged PF; the harmonic/dynamic
  `CalcYPrimMatrix` `Y=Yeq` branch (wye = diagonal-only, no neutral; the delta
  floating-trick). 22 mode-3 state variables + `Set_Variable`. New `ElemKind::IndMach012`
  + `Circuit.ind_machines`; registered after PVSystem; Monitor mode-3 metered-kind admits
  it; new `SlipOption` enum. NOT_PORTED: DebugTrace CSV, `MakePosSequence` (empty
  upstream), the `IndMach012SwitchOpen` Open flag (carried, never set — the latent
  Generator `gen_switch_open` gap). `DoHarmonicMode` ported verbatim incl. the upstream
  commented-out-`E` quirk (injects ~0). The `DynamicEqObj <> NIL` path is step 3b.
  - **Real port bug found + fixed — the "conditioning" was a stateful extra slip
    step ([[dont-rationalize-conditioning]]).** The IndMach012 PF operating point
    differed Rust↔oracle ~4.7e-4 in P at the default tolerance with the *same*
    network iteration count (4). Step 3a originally mis-filed this as conditioning
    (the fixed-slope slip-Newton lagging the node-voltage tolerance, "faer vs KLU
    stopping at different iter-4 points," a tolerance sweep "confirming" it). That was
    exactly the trap: tightening the tolerance *masks* the bug (once the slip sits at
    its root an extra step is a no-op) — it does not prove conditioning, and the
    hidden tell was a *different per-element slip-step count* (Rust 5, oracle 4)
    behind the matching network count. Root cause (instrumented `CalcPFlow` + the
    pinned oracle): `do_indmach_model` set `iterminal_updated = true` as a plain field
    write, dropping the Pascal `TPCElement.set_ITerminalUpdated` side effect
    `IterminalSolutionCount := SolutionCount`. After the converged loop the motor's
    `iterminal_solution_count` stayed −1 ≠ `solution_count`, so the *first* post-solve
    `ComputeIterminal`/`GetCurrents` (any power/current read) re-ran the **stateful**
    `CalcPFlow` — a 5th slip step the oracle never takes (its stamp makes the counts
    equal → the cached terminal current is reused). Generator/Load/PVSystem/Storage
    `put_curr` already carried the stamp; IndMach012 was the lone straggler, *and* the
    only PC element whose `GetTerminalCurrents` recompute is stateful — so it was the
    only one that ever *showed*. Fix = the one missing stamp line. Rust now reproduces
    the oracle bit-for-bit at **both** the default (slip 0.0159858, P 1200.687 kW,
    Is1 1594.017 A) and tight (slip 0.0159741, P 1200.000 kW) tolerances. Regression
    `indmach012_snapshot_default_tol_matches_oracle` pins the default-tolerance
    operating point against the pinned oracle (fails by ~4.7e-4 without the stamp).
    The dynamics gate keeps `tolerance=1e-8` — now just a clean
    electromechanical-fixpoint start, **not** a bug workaround.
  - **Cross-element 1:1 sweep of the `set_ITerminalUpdated` stamp.** The stamp
    (`IterminalSolutionCount := SolutionCount`, set by the Pascal `ITerminalUpdated`
    property setter / `set_ITerminalUpdated`) makes a post-solve `ComputeIterminal`/
    `GetCurrents` reuse the **cached** terminal current instead of **recomputing** the
    model; without it the count stays stale and the read recomputes. It only changes
    *numbers* when the recompute is **stateful** — `IndMach012.CalcPFlow` (the per-call
    slip step) is why only it produced a kW-scale bug — but model-3 generators
    (`DoPVTypeGen`'s per-call dQ/dV var step) are stateful too, so a 1:1 port must
    match Pascal's cache choice at *every* site. Audited all sites **case-insensitively**
    (a case-sensitive `grep` for `ITerminalUpdated` first hid the lowercase-`t`
    `IterminalUpdated` assignments and wrongly suggested Generator only cached
    model-7 — corrected). Pascal **every** PC element caches in **every** PF model +
    dynamics (Generator `DoConstantPQGen`..`DoCurrentLimitedPQ` *and* `DoDynamicMode`
    at generator.pas:1990; Load/Storage/PVSystem likewise) plus Load `DoHarmonicMode`;
    only the bare harmonic injections (Generator/IndMach012 `DoHarmonicMode`) recompute.
    The Rust ports had the stamp **missing at 5 model-contribution sites**: IndMach012
    PF (the visible bug), **Storage/PVSystem `DoDynamicMode`, Load `DoHarmonicMode`,
    and Generator `DoDynamicMode`** — all now stamped to match Pascal (full gate incl.
    live oracle + the model-3 PV snapshot stays green). The interim mistake of making
    Generator PF models 1-6 *recompute* (from the bad grep) broke the oracle-pinned
    `generator_model3_pv_snapshot` (the stateful dQ/dV) and was reverted — its failure
    is exactly why the case-insensitive re-audit happened. Net: **all 5 missing-stamp
    sites fixed; no remaining known divergence.**
  - **Regression coverage for the stamp.** Only the two **stateful** recomputes are
    numerically observable, and both now have tight, oracle-pinned guards that FAIL if
    the stamp is dropped: IndMach012 PF (`indmach012_snapshot_default_tol_matches_oracle`)
    and model-3 generator dQ/dV (`generator_model3_pv_snapshot` tightened to 1e-3 +
    the dedicated `generator_model3_power_read_uses_cached_stamp_vs_oracle`, both
    verified to fail without the `put_curr` stamp). The other three stamps
    (Storage/PVSystem `DoDynamicMode`, Load `DoHarmonicMode`) are **behavior-neutral**
    (idempotent recompute = cached value) — empirically confirmed by removing the
    Generator `DoDynamicMode` stamp and seeing **no** test move — so they have no
    distinguishing numeric test; the existing oracle dynamics/harmonics gates cover
    them against gross regressions. They are kept stamped purely for 1:1 fidelity.
  - **audit-tests follow-up (the stamp fix):** verdict **genuine, non-vacuous,
    oracle-pinned** — independently reproduced the pinned-oracle baseline byte-for-byte
    and confirmed `indmach012_snapshot_default_tol_matches_oracle` FAILS without the
    stamp at exactly the buggy `P1 = 1200.1275` (rel 4.66e-4 ≫ 1e-6) and PASSES with
    it; no coverage loss. Fixed the one Minor finding (a stale "conditioning /
    different iter-4 points / tolerance sweep" sentence left in the sibling
    `indmach012_dynamics_mode3_*` docstring) and the Nit (deduped the duplicated deck
    into a shared `indmach_deck()` builder so the snapshot + dynamics gates can't drift).
  - **Real bug fixed during the port (Monitor mode-3 metered-kind).** Like PVSystem
    (WP7.3) / Storage (WP7.4), the Monitor mode-3 metered-kind classifier had to admit
    IndMach012 (`accessors.rs` downcast list) or a mode-3 monitor aborts "must be a power
    conversion element". Caught by the focused gate.
  - **New prop-flag (`SILENT_READ_ONLY`).** IndMach012 `pf` is Pascal
    `[SilentReadOnly, ReadByFunction]` → `PowerFactor(Power[1])`; the oracle raises
    "solution not initialized" on the unsolved props-probe circuit, so the `?` dump is
    `""`. Added the behavioral flag (set ignored; text dump empty) — the `&self` getter
    has no solution access either, so empty is the faithful match. The PF *value* is
    state variable #21, computed where the solution exists.
  - **Gate (focused step-3a oracle gate):** 2 oracle-pinned IndMach012-dynamics tests
    (`exec/tests/dynamics.rs`, dss-python 0.15.7, tol 1e-8) on a self-contained
    reproduction of the corpus `InductionMachine` example (12.47 kV source → 1500 kVA
    step-down xfmr → 600 kvar cap + 1200 kW delta motor): (1) **steady mode-3** — all 22
    variables, the slipping equilibrium holds (slip/currents/losses/power constant, Theta
    drifts linearly, dSpeed≈0, neg-seq quiescent) matching the oracle at 1e-5; (2)
    **3-phase fault** — the inrush + deceleration (slip rises, rotor frequency falls)
    matches elementwise through 50 fault steps. Plus (3) **default-tolerance snapshot**
    (`indmach012_snapshot_default_tol_matches_oracle`, added with the extra-slip-step
    fix) — pins the 1e-4 terminal power/current to the pinned oracle (P 1200.687 kW,
    Is1 1594.017 A), the regression guard for the `set_ITerminalUpdated` stamp. Plus 3
    construction/slip-clamp unit tests and `props/indmach012.json` (5 scenarios). **No
    corpus migration** (the `InductionMachine` Master.DSS is also blocked on a
    `LoadShape action=normalize` CSV + `Plot`; the `Test/indmachtest` deck uses a
    NOT_PORTED user model — both step-4/Phase-8). lib **666 → 672**; `solvable_now`
    **84**.
  - **audit-code follow-up:** verdict **faithful, no real bug** — every formula
    confirmed line-for-line against `IndMach012.pas` (the swing sign/abs,
    `Pshaft=+Power[1].re`, the D/Dpu undamped wiring, the commented-out harmonic `E`,
    the wye no-neutral diagonal stamping + delta floating-trick, `MakeLike`'s copy
    subset). (The audit's "conditioning independently re-confirmed by a tolerance
    sweep" verdict was **later overturned** — the Rust↔oracle gap was the missing
    `set_ITerminalUpdated` stamp / extra slip step fixed above; the sweep masked it,
    it did not prove conditioning.) Fixed 3 Minor items: (1) **infidelity** — `update_vbase` used
    `(kV·1000)/√3` instead of Pascal's `kV·InvSQRT3x1000` constant (sub-ULP, but now uses
    the `inv_sqrt3_x1000()` helper like the Generator port); (2) removed the write-only
    `power1` cache (its only reader, `get_f64(PF)`, is unreachable — the dump is
    intercepted by `SILENT_READ_ONLY`; the `&self` getter has no solution access, so the
    arm now returns the unsolved `PowerFactor(0)=1` placeholder with a comment; the live
    PF stays state var #21); (3) documented the `calc_model` `<3`-phase zero-padding
    (well-defined vs Pascal's read-past-buffer UB, unreachable in the corpus).
  - **audit-tests follow-up:** verdict **sound + non-vacuous** — the auditor
    independently re-ran the pinned oracle and confirmed every dynamics constant and
    all 5 props scenarios reproduce exactly (real oracle output, not regenerated Rust).
    (The "`tolerance=1e-8` is genuinely necessary because the conditioning is real"
    finding was **later overturned**: the default-tolerance gap was the extra-slip-step
    bug fixed above, not conditioning. `tolerance=1e-8` is retained only as a clean
    dynamics-fixpoint start; the default-tolerance match is now pinned by
    `indmach012_snapshot_default_tol_matches_oracle`.) Fixed 3 items: (1) **Major** — the `indmach012_makelike` props scenario left
    `Slip`/`SlipOption`/`Conn` at defaults on `base`, so a MakeLike that wrongly *copied*
    those non-copied fields would pass; `base` now sets `conn=wye slip=0.05
    SlipOption=fixedslip D=3` and the regenerated golden pins the non-copy (m1 reads back
    0.007/VariableSlip/delta) vs the copied-via-record `D=3`; (2) tightened the steady
    Is2/Ir2 quiescence bound `1e-3 → 1e-5` (~20× over the ~4.79e-7 actual); (3) added a
    value pin for the `dTheta` state var (`rel(last(5), -6.022097) < 1e-5`) — previously
    only `dSpeed` among the rate vars was checked. No corpus migration (recon-confirmed).
- **step 3b — DynEqPCE integration (Generator).** Ported `PCElements/DynEqPCE.pas`
  (`TDynEqPCE`) as the shared `pc/dyneq_pce.rs` `DynEqPceData` (+ the `DynEqPce`
  host trait): the `DynamicEqObj`/`DynamicEqVals`/`DynamicEqPair`/`DynOut`/`UserDynInit`
  memory, `ParseDynVar` (the inline `<dynvar>=<value>` initializers — calc-value
  operands → `DynamicEqPair`, constants → `DynamicEqVals` via an RPN `make_double`),
  `Set/GetDynOutputNames` (resolve `DynOut=[..]` to output indices / reconstruct for
  the dump), the `DynamicEq=` sizing side effect, and the `NumVariables`/`VariableName`/
  `GetAllVariables`/`SolveEq` surface. **Generator** now embeds `DynEqPceData` (its
  Phase-6 `NOT_PORTED` `DynamicEq`/`DynOut` strings flipped to a real `object_ref_class`
  + `string_list`), resolves the `DynamicExp` snapshot-clone like its shape refs, and
  its `dynamics.rs` `InitStateVars`/`IntegrateStates` branch on `DynamicEqObj <> NIL`
  (zero the derivatives + apply `IsInitVal` P0/Q0/edp seeds; per-step load the
  calc-values [`Get_PCE_Value` for P/Q/Vmag/.../S; `Cang(Edp)` for edp] → `SolveEq` →
  trapezoidal `Speed`/`Theta` from `DynOut[0]`/`DynOut[1]` written back to GenVars so
  `DoDynamicMode` reads the new angle). `Get_PCE_Value` (CktElement.pas l.828) ported
  on the Generator. The edit loop (`exec/command.rs`) gained the Pascal `DSSClass.Edit`
  l.1656 `ParseDynVar` fallback on the "unknown parameter" branch, via a new
  `DssObject::parse_dyn_var` (default `false`).
  - **Gate:** 3 oracle-pinned tests (`exec/tests/dynamics.rs`, dss-python 0.15.7) on
    the corpus `Dynamic_Expressions/Dynamic_KundurDynExp.dss` (Kundur Ex.13.1 with the
    6-var swing `DynamicExp` replacing `H`/`D`): **steady mode-3** — the 12 DynamicExp
    memory slots (named from the lowercased varnames) hold the swing fixpoint across
    1001 samples (theta 0.7290715 rad, mass 41221132, pshaft 1.9979999e9, pterm 1.998e9)
    at 1e-5; **fault response** — the 70-step bolted fault accelerates the rotor
    (theta 0.7290715 → 0.84611225 rad = 41.77 → 48.48 deg, speed → 3.368602, monotonic);
    **full swing** — fault + clear-by-`Open`, the undamped theta slot (radians) swings
    0.42211992 → 1.7127246 rad. Each also asserts the DynExp trajectory **equals the
    classic Kundur gate** scaled by 180/π (theta_rad·180/π = the classic Theta in
    degrees), proving the user equation reproduces the built-in shaft model exactly.
    lib **673 → 676**; `solvable_now` **84** (the `Dynamic_KundurDynExp` deck is also
    `Plot`/`Export`-blocked — Phase-8). `props/generator.json` is unaffected (default
    `DynamicEq`/`DynOut` dumps stay empty).
  - **audit-tests follow-up:** verdict **sound + non-vacuous** — the auditor
    independently re-ran the pinned oracle and reproduced **every** constant
    bit-for-bit (steady/swing) and confirmed the value pins sit ~100–1000 f32 ULPs
    above the quantization floor (real constraints, not noise). Acted on its three
    Minors: added the **fault-response** test (parity with the classic 3-test set;
    oracle-pinned theta/speed @1070 + monotonic rise), pinned the two meaningful
    unpinned channels (**ch1 `dspeed`** ~0 + **ch8 `damp`** exactly 0), and tightened
    the per-sample fixpoint-hold loop **1e-4 → 1e-5** (the classic sibling's bound; the
    real drift is ≤1 ULP). lib **675 → 676**.
  - **audit-code follow-up:** verdict **faithful, no critical bug** — every focus
    area (calc-value/const branches, the `(0..50000)` guard, `Set/GetDynOutputNames`,
    the `on_dynamic_eq_set` sizing order, the `InitStateVars`/`IntegrateStates`
    `DynamicEqObj <> NIL` branches, `Get_PCE_Value` codes 0-8, the DynExp-first
    state-var dispatch, MakeLike not copying `dyneq`, the `solve_eq` disjoint borrow)
    confirmed line-for-line against Pascal. Fixed its one real Minor: `set_dyn_output_names`
    sized `DynOut` to `names.len()`; Pascal `SetLength(DynOut, 2)` is **unconditional**,
    so a 1-element `DynOut=[Speed]` would panic on `dyn_out[1]` in `IntegrateStates`
    — now `vec![0; 2]` (1:1, slot 1 = 0; >2 names overrun like Pascal's range error).
    Plus the Nit (the 50007 message's per-element trailing comma). **Surfaced-not-fixed:**
    Generator has no `Set_Variable`, so the DynamicEq `msg 566` guard is absent — a
    **pre-existing** documented gap (no caller yet), not a step-3b regression; the
    `UserDynInit` number-vs-string `requiredRPN` distinction is deferred with
    `SaveWrite` (Phase 8). lib stays **676**.
  - **Tolerance review (prompted by a "why 1e-4/1e-5, not the planned 1e-6?"
    question).** TOLERANCE_NOTES.md pins monitor channels at **1e-6 rel / 1e-4 abs**
    (they are f32); the dynamics gates had copied the classic step-2a `1e-5`/`1e-4`
    bounds. Measured the actual Rust↔oracle delta on every channel of both Kundur
    gates (classic + DynExp): **~1e-8 or tighter** (steady theta 2e-9, pshaft 1.4e-8;
    fault freq 3.6e-11; swing extrema 3e-9/2e-8) — the looseness was masking nothing.
    Tightened **all** dynamics monitor pins to the standard **1e-6** (both gates).
    Critically, investigated the largest "small" channel: classic `dSpeed` =
    −4.3093074e-5 deg/s — **proved (oracle probe) it is not a bug**: the oracle
    produces the identical value (Rust matches to ~9e-8 rel). It is the swing-equation
    fixpoint *residual* (`Pshaft` fixed at init vs the per-step recomputed electrical
    power, ~1.5e-8-rel mismatch / `Mmass`), reproduced by both engines. The residual
    channels (`dSpeed`/`dTheta`/`speed`/`dspeed`) are now pinned against the oracle's
    **actual value** (a stronger guard than `abs < ε ≈ 0`), and TOLERANCE_NOTES.md
    documents the dynamics monitor policy + the residual rationale. No new exception;
    the dynamics tests now obey the standard monitor-channel tolerance. lib stays **676**.
  - **Tolerance review, part 2 (extended to PVSystem/Storage/IndMach012 on a
    follow-up request).** Same treatment for the other dynamics gates (step 2b/3a),
    which also carried copied `1e-4`/`1e-5` bounds: measured every pinned channel
    (steady + fault) — all match the oracle to **~1e-8 or tighter** — and tightened
    them to **1e-6**. The IndMach `dSpeed` residual is pinned against the oracle's
    actual `1.742747e-5` (same fixpoint-residual reasoning as Kundur, oracle-probed).
    Storage SOC@100-drop literal corrected `0.0046997 → 0.004699707` (the 5-sf value
    couldn't support 1e-6); `kWInvLosses` shown exactly 0 by the loss decomposition.
    The few channels left looser are documented, not slack: the IndMach Is2/Ir2
    negative-seq quiescence is a `<1e-5` *upper bound* on a ~4.79e-7 quantity, and the
    near-zero kW/kvar (Storage kWIn/kvarOut) sit at the 1e-4 monitor abs floor.
    **Clarification (also requested): all computation is f64**; only the monitor
    *recording buffer* is f32 — a 1:1 port of Pascal `TMonitorObj.MonBuffer:
    pSingleArray` (`AddDblToBuffer` narrows `Double`→`Single`) — so the mode-3 reads
    on both sides are f32, which sets the ~1e-7 comparison floor (hence 1e-6, ≈10× it,
    is the tightest meaningful monitor tolerance). lib stays **676**.
- **step 3b cont. — DynEqPCE integration for the inverters (PVSystem + Storage).**
  Closed the WP7.7-step-3 inverter `DynamicEqObj <> NIL` path. **Refactor:**
  `InvBasedPceData` dropped its bare `dynamic_eq`/`dynamic_eq_obj`/`dynamic_eq_ref`/
  `dyn_out`-string fields and now **embeds the shared `DynEqPceData`** (`pc/dyneq_pce.rs`,
  the same record Generator carries), so PVSystem/Storage get the real
  `DynamicEqVals`/`DynamicEqPair`/`DynOut`-indices memory + `parse_dyn_var` +
  `set/get_dyn_output_names` + the `DynamicEq=` sizing side effect + the `DynEqPce`
  trait — exactly the Generator wiring (`accessors.rs`). **Dynamics (`pvsystem/`+
  `storage/dynamics.rs`):** `InitStateVars` appends the derivative-column zero-out
  (Pascal PVsystem.pas l.2258 / Storage l.2834 — no init-value seeding, unlike the
  Generator: the classic per-phase `it`/`Vgrid`/`m` seed *is* the state);
  `IntegrateStates` gained the per-phase `integrate_dyn_eq_phase` (Pascal l.2356 /
  l.2919) — load `it[i]`→`DynOut[0]`/`dit[i]`, load the calc-values with the inverter
  overrides (`2`→`Vgrid[i].mag`, `4`→nothing [the current is `DynOut[0]`],
  `10`→`RatedVDC`, `11`→`SolveModulation`+`m[i]`, else the generic `Get_PCE_Value`),
  `SolveEq`, then `dit[i] := DynamicEqVals[DynOut[0]][1]` and the common trapezoidal
  `it[i]`. Added `get_pce_value` (CktElement.pas l.828, P/Q/Vang/Iang/S — full for
  fidelity, though the GFL deck intercepts every code) and the
  `NumVariables`/`VariableName`/`GetAllVariables`/`Get_Variable` DynExp-first dispatch
  + the `Set_Variable` msg-566 read-only guard. GFM, DynaModel/UserModel DLLs stay
  NOT_PORTED.
  - **Gate:** 2 oracle-pinned tests (`exec/tests/dynamics.rs`, dss-python 0.15.7) on
    self-contained PV/Storage micro feeders driven by the corpus GFL_IEEE123 DynExp
    filter equations (`myDiffEq`/`myDiffEq2`, `it dt = (1/L)(modul·vdc − R·it − vac)`):
    the mode-3 monitor records the **8 DynamicExp memory slots** (`it vdc modul vac` ×
    value/derivative) instead of the 22/34 classic vars, and the per-phase current
    `it` (DynOut[0]) integrates to the ISP setpoint (23.14571 A — *distinct* from the
    classic gate's 23.148539, i.e. the user equation drove it). PV: the tiny filter L
    (0.61 mH) makes the start-up stiff, so the settled slots (sample 100+) are pinned;
    Storage seeds `it=0` and ramps smoothly, so the first derivative `dit@0 = 2358.4167`
    (deterministic from the init seed) + the ramp are pinned too. Matched the oracle to
    1e-6 first-run (no fudging). lib **676 → 678**; `solvable_now` **84** (the corpus
    GFL_IEEE123 DynExp deck is a daily/`Plot`-blocked Phase-8 case — no migration).
  - **audit-code follow-up:** verdict **faithful, no behavioral deviation** — the
    refactor + both inverters' `InitStateVars`/`IntegrateStates`/`Get_Variable`/
    `Set_Variable`/`GetAllVariables`/`NumVariables`/`VariableName` DynamicEq branches
    confirmed line-for-line against `PVsystem.pas`/`Storage.pas`/`CktElement.pas`
    `Get_PCE_Value` and the accepted Generator reference (the `integrate_dyn_eq_phase`
    body, the `InitStateVars` no-init-value zero-out, the inverter-byte-identical
    `get_pce_value`, MakeLike not copying `dyneq`, the `Get_Variable` underflow guard).
    Fixed its one real **Minor**: **8 stale function-level doc comments** still marked
    the `DynamicEqObj` path `NOT_PORTED` after it was ported — 4 inverter + **4
    Generator** (a step-3b leftover the step-3b audit missed, surfaced by extending the
    `rg NOT_PORTED` sweep) — now corrected so the greppable convention holds.
    **Surfaced-not-fixed (recorded):** (1) `get_pce_value` codes 0/1/3/5/6 (P/Q/Vang/
    Iang/S) are not oracle-pinned by the inverter gates — but the body is byte-identical
    to the Generator's, whose Kundur DynExp gate **does** pin code 0 (P) + code 9 (edp),
    so the inverter branch is faithful-by-construction; a dedicated p/q-referencing
    inverter deck is deferred as low-value. (2) `Set_Variable` with `i = 0` + a linked
    DynamicEq emits Pascal msg-566 where Pascal emits msg-565 first — unreachable
    (`set_variable` is only called with `i ≥ 1`, no oracle error-channel compare), a Nit
    consistent with the pre-existing no-`i<1`-565 pattern.
  - **audit-tests follow-up:** verdict **sound + non-vacuous** — the auditor
    independently re-derived **every** pinned literal from the live pinned oracle (all
    ~1e-8 rel, ≈60× inside the 1e-6 f32 floor), and an adversarial **mutation pass**
    proved the gate is a genuine guard (a wrong derivative-readback slot or a skipped
    `SolveModulation` each fail well above tolerance, on **both** PV and Storage; the
    `4 =>`-nothing arm is provably equivalent — code 4 binds the `DynOut[0]` current —
    so its neutral mutation is correct, not a hole). Closed the one **Major** gap
    (no disturbance test — every sibling dynamics gate has one): added
    `pvsystem_dynexp_dynamics_safe_mode_under_fault_matches_oracle` (the bolted-fault
    safe-mode branch: `vac` collapses, `modul` → 0, the equation settles `it`/`dit` at
    the safe-mode limit) and `storage_dynexp_dynamics_trips_under_fault_matches_oracle`
    (trip-to-idle freezes the DynExp `it` slot), plus a **PV sample-0 pin** (the
    deterministic first dynamics step `it@0 = 673.266`) — all oracle-pinned, matched
    first-run. **Recorded-not-fixed:** the `InitStateVars` derivative zero-out is
    behavior-neutral on a single dynamics entry (the memory is already zero-sized; it
    only bites on dynamics *re-entry* after a prior run left `dit ≠ 0`) — a faithful
    Pascal port, untested ≠ buggy; a re-entry scenario would cover it. lib **678 → 680**.
- **next:** WP7.7 step 4 — the dynamics corpus burn-down (probe the
  `Test/`/`Version8` dynamics decks for any now-migratable case) + the targeted
  offline `phase7/dynamics*.json` golden (the focused regression guard the live gate
  doesn't replace). The **GFM grid-forming inverter mode** stays deferred
  (NOT_PORTED loud abort across Generator/PVSystem/Storage `DoDynamicMode`).
