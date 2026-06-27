# Phase 7 — WP7.5 (DER C: InvControl + ExpControl) — archived per-step records

> **Archived from `STATUS.md`** (2026-06-27) to keep the live handoff lean. WP7.5
> (DER C) steps 1–3 are **✅ COMPLETE** on the `phase-7-extended-elements` branch
> (step 1 `RollAvgWindow`; step 2 `InvControl` sub-steps 2a parse-skeleton / 2b
> VOLTVAR / 2c VOLTWATT+VV_VW / 2d DRC+VV_DRC / 2e-i WATTPF+WATTVAR / 2e-ii AVR /
> 2e-iii LPF/RiseFall+MonBus; step 3 `ExpControl`); **next = step 4 (the gate /
> corpus burn-down review)**. These are the frozen per-step records, superseded only
> by the code and tests. The live `STATUS.md` §1e keeps a one-line-per-step summary.
> §3/§4/§5 cross-references resolve against `STATUS.md`. Plan: `PHASE7_PLAN.md` §WP7.5.

---

**WP7.5 (DER C: InvControl + ExpControl) — 🚧 IN PROGRESS** (step 1 = the
`RollAvgWindow` helper **COMPLETE**; step 2 = `InvControl` **COMPLETE** — sub-steps
2a (parse-only skeleton) / 2b (VOLTVAR) / 2c (VOLTWATT + VV_VW) / 2d (DRC + VV_DRC) /
2e-i (WATTPF + WATTVAR) / 2e-ii (AVR) / 2e-iii (LPF/RiseFall + MonBus) **all done**;
**next = step 3 = `ExpControl`**; step 4 = the gate).

**Step 1 — `RollAvgWindow` (`control/roll_avg_window.rs`).** Port of
`Controls/RollAvgWindow.pas` (`TRollAvgWindow`, 105 lines) — the fixed-capacity
FIFO of (value, time) samples with O(1) running sums that backs InvControl's
volt-var / DRC **rolling-average voltage** (`FRollAvgWindow` + `FDRCRollAvgWindow`,
fed `solnvoltage` + `DynaVars.h` each `Sample`). A plain helper struct (not a DSS
object — no props, never New-able), two `VecDeque<f64>` queues + `add`/`set_length`/
`avg_val`/`accum_sec` ported 1:1; latches `buffer_full` by count *or* the
accumulated-time threshold, then evicts oldest-first; `bufferlength=0` forces stored
values to 0 (times still recorded). **Reproduced verbatim (plain comment, not
`TODO(compat)`):** Pascal's `Add` updates the *time* running-sum by subtracting
`sampletime.front` read *after* the pop+push (the new front) — asymmetric with the
*value* sum (which subtracts the pre-pop front) — so `runningsumsampletime` drifts
from the true Σ. Harmless/unobservable: its only reader `AccumSec` is **dead in the
upstream tree** (no caller anywhere in `dss_capi`), so no golden pins it — hence a
documented faithful reproduction rather than a `TODO(compat)` (which is reserved for
goldens-pinned numeric reproductions). **Gate:** 4 spec-pinned unit tests (empty
read-zero; fill-by-count then evict; `bufferlength=0` value-zeroing; fill-by-time
below capacity — each pinning the exact running-sum arithmetic incl. the asymmetric
`accum_sec` value). Spec-pinned (Pascal is the spec): the oracle exposes no
`RollAvgWindow` outside a full InvControl solve; its numeric oracle pinning arrives
with InvControl (step 2). **Self-contained: no solve-loop change, no class
registration, corpus stays 44.** lib **572 → 576**.
- **audit-code follow-up:** verdict faithful 1:1 — every method maps line-for-line to
  the Pascal; **no fix needed**. Confirmed the one judgment call (the `runningsumsampletime`
  asymmetry documented with a plain comment, not `TODO(compat)`) is correct: its sole reader
  `AccumSec` is **dead in the vendored tree** (`grep -rn AccumSec .inputs/dss_capi/src` →
  definition only), so no golden pins the drift and `TODO(compat)` (reserved for
  goldens-pinned reproductions) would be wrong. Verified the two `.front().unwrap()` are
  panic-safe (the eviction branch is `!is_empty()`-guarded; the two queues move in lockstep)
  and the `len() as i64 == buffer_length as i64` count-latch matches FPC's signed/unsigned
  promotion for a negative `bufferlength`.
- **audit-tests follow-up:** verdict strong (exact Pascal-derived literals, not
  copied-from-output; the asymmetry quirk and both latch conditions decisively pinned).
  Closed the one minor gap — the time-threshold latch is strict `>` (`sum == window` must
  NOT latch), which no test sat exactly on. Added `time_threshold_is_strict_greater` (a
  size-100 window, `sum-time == length == 3` stays open so the next add grows to `avg=15`;
  a `>=` regression would evict to `20`). lib **576 → 577**.
**Step 2 — `InvControl`** (`Controls/InvControl.pas`, 3586 lines, the single
largest unit in the phase: 8 control modes — VOLTVAR / VOLTWATT / DRC / WATTPF /
WATTVAR / AVR / GFM + the VV_VW / VV_DRC combi modes). Ported in sub-steps:
**2a (the parse-only skeleton) COMPLETE**; 2b–2e add the DER-fleet build + the
per-mode `Sample`/`DoPendingAction` dispatch (2b VOLTVAR, 2c VOLTWATT/VV_VW, 2d
DRC/VV_DRC, 2e WATTPF/WATTVAR/AVR + LPF/RiseFall + MonBus — 2e split into 2e-i
WATTPF/WATTVAR, 2e-ii AVR, 2e-iii LPF/RiseFall + MonBus).

- **step 2a — the parse-only skeleton (`control/inv_control/{mod,accessors,tests}`).**
  The class on the `TControlElem` base with the full property surface: the 34
  properties via `class_props`, the **seven smart-inverter enums**
  (`invcontrol_{mode,combi,voltage_curvex,voltwatt_yaxis,roc,reac_power,model}`
  in the control enum registry; `MonVoltageCalc` reuses `mon_phase`), the five
  XYcurve refs (`VVC_Curve1` / `VoltWatt_Curve` / `VoltWattCH_Curve` /
  `WattPF_Curve` / `WattVar_Curve`, snapshot-clone), the DERList / MonBus string
  lists + the MonBusesVBase function-sized array, `Create` defaults,
  `PropertySideEffects` (the `ValidateXYCurve` per-mode Y-range check that nils an
  out-of-band VOLTWATT/WATTPF/WATTVAR curve with error 381; the DbVMin/DbVMax,
  LPFTau/RiseFall→INACTIVE, Mode→clears-CombiMode, and PVSystemList-prepend
  guards), `MakeLike`, and class registration after PVSystem (Pascal
  DSSClassDefs.pas:273). `ControlModel` is a `MappedIntEnum` (parses + dumps the
  number, like Generator/PVSystem `model`); `VV_RefReactivePower` is
  DeprecatedAndRemoved (read-only `''`, like Storage `%Idlingkvar`);
  `PVSystemList` shares the DERList backing (prepends `PVSystem.`).
  - **Deferred to step 2b+ (the behavior):** `MakeDERList` (the PVSystem/Storage
    fleet resolution), `RecalcElementData`'s bus/monitored-element setup, the
    `monBus` per-bus node parsing (`FMonBuses`/`FMonBusesNodes`, consumed only by
    `Sample`'s `GetMonVoltage`), the per-DER `TInvVars` runtime state, and the
    whole `Sample`/`DoPendingAction` dispatch. **Never a silent skip:** the
    control-sweep dispatcher (`solution/controls/dispatch.rs`) gives InvControl a
    dedicated arm — `Reset` is a Pascal no-op (`// inherited`),
    `Sample`/`DoPendingAction` record an explicit "InvControl … not yet ported
    (WP7.5 step 2b)" abort. **NOT_PORTED:** `MakePosSequence`.
  - **Gate:** `props/invcontrol.json` (**20** oracle-pinned scenarios — default,
    the six single-mode setups, the VV_VW/VV_DRC combis, MonBus, LPF/RiseFall
    rate-of-change, all four curve-nulling arms + the VOLTVAR unchecked path, the
    every-enum-slot coverage, the PVSystemList prepend, and MakeLike, all
    round-trip exactly) + **8 spec-pinned unit tests** (`Create` defaults + the
    side-effect guards). NOTE the oracle quirk pinned in the scenarios: an
    InvControl with an *empty* DERList auto-populates DERNameList from the circuit
    in `RecalcElementData` (which the deferred 2a recalc skips), so every fleet
    scenario names the DER list explicitly (the curve/enum scenarios are DER-free
    so the empty list stays empty). **Corpus stays 44** (the `Test/InvControl*`
    family — volt-var/volt-watt/VV_VW/VV_DRC/DRC/watt-pf/watt-var/MonitoredVoltage
    — needs the step-2b+ dispatch). lib **577 → 585**.
  - **audit-code follow-up:** verdict faithful 1:1 — the property table (34
    ordinals + the tail), the seven enums, `ValidateXYCurve`, the side-effect
    guards, `Create` defaults, and `MakeLike`'s copy set all match the Pascal +
    the oracle goldens; **no fix needed**. Surfaced-not-fixed (both non-defects):
    (1) the inert JSON-schema flags `IntervalUnits`/`Units_s`/`Deprecated` on
    AvgWindowLen/DynReacAvgWindowLen/LPFTau/VV_RefReactivePower/PVSystemList aren't
    recorded — `PropFlags` has no such variants (consistent with the existing
    infra, which records inert flags only when the enum defines them); (2)
    `recalc_element_data` is a no-op, so an InvControl's terminal bus is unset in
    2a — harmless now (no gate builds Y with an InvControl; any solve aborts at
    `Sample`), but a **step-2b carry-forward**: `RecalcElementData` must restore
    `Setbus(1, MonitoredElement.Firstbus)` before `ProcessBusDefs` walks the
    control, or an empty terminal bus could perturb node ordering.
  - **audit-tests follow-up:** verdict solid (the props.json is a genuine oracle
    baseline; the unit tests pin exact Pascal-derived ctor defaults + every
    side-effect guard; MakeLike decisively pins both copied and not-copied
    fields). Closed the real coverage gaps — `props/invcontrol.json` **13 → 20**:
    the WATTPF/WATTVAR/VoltWattCH curve-nulling arms (each a distinct band/message
    the VOLTWATT scenario didn't reach), the VOLTVAR *unchecked* path (a Y=2 curve
    is kept, not nilled), and the previously-unpinned enum reverse-render slots
    (CombiMode VV_DRC, Voltage_CurveX_Ref Avg/RAvg, VoltWattYAxis
    PAvailablePU/PctPMPPPU/KVARatingPU, RateOfChangeMode RiseFall, Mode GFM,
    MonVoltageCalc min) + a non-default RiseFallLimit. No lib count change
    (golden-only).
- **step 2b — the VOLTVAR dispatch (`control/inv_control/compute.rs` +
  `dispatch.rs` env).** The real DER-fleet control: `MakeDERList` (named list →
  14403 on a missing PVSystem/Storage; an empty list scans every PVSystem then
  Storage, appending each `FullName` to `DERNameList`), the per-DER `TInvVars`
  runtime state (`ctrl_vars`), `UpdateDERParameters` / `GetMonVoltage` (the
  no-`MonBus` self-monitoring path), `Sample`'s VOLTVAR trigger + the
  `DoPendingAction` `CHANGEVARLEVEL` dispatch (`CalcQVVcurve_desiredpu` incl. the
  hysteresis state machine, `Check_Qlimits`, `Calc_QHeadRoom`, `CalcVoltVar_vars`'s
  delta-Q convergence + `Change_deltaQ_factor`), and `UpdateInvControl` (the
  rolling-average feed, wired into `EndOfTimeStepCleanup`'s `UpdateAll`).
  - **Architecture (the StorageController/GenDispatcher pattern):** the fleet is a
    *dynamic* set, so `Sample`/`DoPendingAction`/`UpdateAll` reach the
    PVSystem/Storage fleet through an `InvDispatchEnv` over the store
    (`dispatch.rs` clones the control out, builds the env, runs, copies back). The
    fleet resolves **lazily on the first `Sample`**. The control's terminal bus
    (Pascal `Setbus(1, MonitoredElement.Firstbus)`) is resolved at **parse-time
    edit-completion** instead (the step-2b carry-forward): `exec/command.rs`
    resolves the first DER's bus + phase count through the foreign view (a named
    list → its first entry; an empty list → `ForeignClasses::first_enabled` over
    PVSystem then Storage) and hands them to `set_resolved_monitored`; `end_edit`
    → `recalc` attaches the terminal **before `ProcessBusDefs`**, so node ordering
    matches the oracle.
  - **The VOLTVAR channel:** `DoPendingAction` sets the DER's `kvar_requested` +
    `varMode=KVAR` + `vv_mode`, calls `SetNominalDEROutput`, reads back
    `Get_Presentkvar`. PVSystem's `SetNominalDEROutput` does **not** raise
    `YprimInvalid` on an inverter cut-in/out (verified in WP7.4's YPrim note), so —
    like GenDispatcher — InvControl needs **no** `system_y_changed`: the re-solve
    carries the kvar change via the compensation-current injection.
  - **The pinned oracle runs `CompatFlags=0`**, so a *set* `deltaQ_factor` is used
    directly each iteration (`FdeltaQFactor := FdeltaQ_factor`); only the unset
    sentinel `FLAGDELTAQ` takes the adaptive `Change_deltaQ_factor` path.
  - **NOT_PORTED / deferred (each an explicit error, never a silent skip):** every
    non-VOLTVAR mode (VOLTWATT / DRC / WATTPF / WATTVAR / AVR + the VV_VW / VV_DRC
    combis → 2c–2e), the explicit-`MonBus` `GetMonVoltage` path (`FUsingMonBuses`
    → 2e), the LPF / Rise-Fall rate-of-change limiting (→ 2e), and the Exponential
    `ControlModel` (the `TPICtrl` PI controller → WP7.7). `MakePosSequence` stays
    NOT_PORTED.
  - **Gate:** targeted golden `phase7/invcontrol_voltvar` (a PVSystem on a weak
    line absorbing vars per the volt-var curve — node voltages + the PVSystem
    terminal powers 1e-6 **and the exact 18 iters / 9 control iters**, the delta-Q
    convergence path; matched the oracle first-run) + **5 mock-env unit tests**
    (the named-missing 14403, the empty auto-populate, the ControlIteration-1
    push, the first-DoPendingAction curve→clamp→delta-Q step pinned to
    `QDesiredVV=-75.8`, and `Calc_QHeadRoom` VARMAX vs VARAVAL). **Corpus 44 → 50:**
    the **6 SnapShot volt-var cases** (`Standard`, `Standard_varaval`,
    `varaval_kvarlimitation`, `greater_kVA_ppriority`/`qpriority`, `pctPmpp60`)
    migrate into `solvable_now` and match the oracle full-model live (voltages,
    powers, currents, YPrim — covering Check_Qlimits clamps + watt/var priority).
    The **7 Daily volt-var cases** stay `skipped_unsupported` — blocked by `Export`
    (Phase 8), **not** numerics (so the `avg`/`ravg` rolling-average path is
    exercised but its oracle comparison waits on Phase 8). lib **585 → 590**.
    *(Also fixed a pre-existing latent clippy `manual_range_contains` warning in
    the step-2a `validate_xy_curve` — kept the readable `y < 0 || y > 1` with a
    local allow rather than the double-negative range-contains.)*
  - **audit-code follow-up:** verdict faithful 1:1 for the VOLTVAR happy path
    (golden + 6 corpus cases prove it); fixed 1 Major + 4 Minor. (1) **Major —
    Exponential `ControlModel` silently froze** (`CalcVoltVar_vars` else-branch set
    `QDesiredVV := QOldVV`, a plausible no-change, instead of the unported PICtrl
    PI solve) → now a `Sample`-time abort (the deferral-is-never-silent rule) + an
    exec-style unit test. (2) `Sample` set `Varmode`/`VWmode` but Pascal sets only
    `VVmode` there (the rest in `DoPendingAction`) → new `der_set_vv_mode` env
    method. (3) event-log `{:.5}` → `fmt_g(.., 5)` (`%.5g` significant-figures,
    matching StorageController). (4) a named DERList resolved the bus from the
    first *named* entry → now the first *enabled* one (Pascal
    `FDERPointerList.Get(1)`). (5) a partial named list `[valid, missing]`
    re-emitted the 14403 every `Sample` → gate the lazy build on an **empty** fleet
    (Pascal `FDERPointerList.Count = 0`), so a non-empty partial fleet errors once
    (+ `CtrlVars` sized to the actual fleet in `ensure_fleet`); dropped the
    now-redundant `fleet_list_changed` flag. Surfaced-not-fixed (both unobservable,
    single-InvControl / homogeneous-fleet gated cases match): the `FVpuSolutionIdx`
    `i=1`-only multi-InvControl quirk (needs the element-list index the per-element
    env doesn't carry) and `FNphases` first-vs-last-DER.
  - **audit-tests follow-up:** verdict strong (the golden pins the converged kvar +
    exact 18/9 iters; the −75.8 mock is an independent hand-derivation). Closed the
    Major gap — the rolling-average path (`avg`/`ravg` + `UpdateInvControl`) had
    **zero** oracle pin (the corpus `avg`/`ravg` cases are Export-blocked): added
    the **daily golden `phase7/invcontrol_voltvar_avg`** (`voltage_curvex_ref=avg`,
    no Export, 8-step daily) that oracle-pins the rolling-average integration — a
    discriminator, since the avg path settles at kvar ≈ −3.9 where the rated path
    would give ≈ −63, so a window/avg-branch regression fails the PV-power pin.
    Added mock tests for the **inject** direction (vpu 0.90 → `QDesiredVV=+119.2`,
    the `QHeadRoom` branch) + the named-missing-once fix. Surfaced-not-fixed
    (acceptable): the kVA/kvar-limit clamp is validated **live** (the
    `greater_kVA`/`varaval_kvarlimitation` corpus cases) but has no committed
    offline golden; the multi-DER fleet stays untested (every gated case is
    single-DER). lib **590 → 593**.
- **step 2c — the VOLTWATT + VV_VW dispatch (`control/inv_control/compute.rs`).**
  Adds the **volt-watt** control: `Sample`'s VOLTWATT trigger (note the
  inverter-off check is `FInverterON=FALSE` alone — no `VarFollowInverter`, unlike
  the var modes) + the `DoPendingAction` `CHANGEWATTLEVEL` dispatch
  (`CalcPVWcurve_limitpu` → the curve kW-limit; `Check_Plimits` → the var-priority
  kVA + pctPmpp clamp; `Calc_PBase` from `VoltWattYAxis`; `CalcVoltWatt_watts` → the
  delta-P convergence with `Change_deltaP_factor`); and the **VV_VW combi**: both a
  volt-watt and a volt-var trigger in `Sample` (both queue `CHANGEWATTVARLEVEL`) and
  the joint `DoPendingAction` that runs `CalcVoltWatt_watts` *and* `CalcVoltVar_vars`,
  setting the DER's kW *and* kvar in one `SetNominalDEROutput`. `Sample`/
  `do_pending_action` refactored into per-mode helpers (`sample_voltvar`/
  `sample_voltwatt`/`sample_vv_vw`; `do_pending_voltvar`/`_voltwatt`/`_vv_vw`).
  - **Key fix — the `FPendingChange` reset (Pascal l.1606).** Pascal resets
    `FPendingChange := NONE` at the **end of every DER's `DoPendingAction` loop
    body**, so the VV_VW **double-push** (the volt-watt *and* the volt-var trigger
    both fire while the voltage is changing, queuing `CHANGEWATTVARLEVEL` twice) is
    dispatched **once per control iteration**, not once per queued action — the
    second popped action finds `FPendingChange = NONE` and is a no-op. The port
    initially missed this reset and over-converged (45 iters vs the oracle's 34);
    adding it makes `invcontrol_vv_vw` match the oracle bit-for-bit (34 iters, the
    same converged kW+kvar). The reset is harmless for the single-push VOLTVAR/
    VOLTWATT modes (Sample re-sets the pending change each iteration).
  - **Storage VOLTWATT/VV_VW deferred (explicit error, never a silent skip):** the
    Storage-specific volt-watt machinery (`TStorageObj.DCkW`/`StorageState`/
    `FVWStateRequested` curve selection) is unverified by any gate, and a Storage
    state flip during InvControl dispatch would not propagate `system_y_changed`
    through the per-element env (the WP7.4 YPrim-rebuild bug class). PVSystem
    volt-watt is fully ported + gated; a Storage in VOLTWATT/VV_VW errors at
    `Sample` (`guard_storage_vw`). **NOT_PORTED (each an explicit error):** the
    remaining modes (DRC/VV_DRC → 2d; WATTPF/WATTVAR/AVR → 2e; GFM/Exponential →
    WP7.7), the `MonBus` path + LPF/RiseFall (→ 2e).
  - **Gate:** targeted goldens `phase7/invcontrol_voltwatt` (a 1000 kW PV on a weak
    line driving V > 1.02 pu → the volt-watt curve limits the kW; node voltages +
    PV terminal power 1e-6 **and the exact 13 iters**) + `phase7/invcontrol_vv_vw`
    (the same fleet with both curves, the vw curve limiting from 1.0 pu so **both**
    functions engage — kW limited to ~978 *and* ~106 kvar absorbed; the exact **34
    iters**, the double-push/pending-reset path) + **5 mock-env tests** (the
    VOLTWATT curve→clamp→delta-P step pinned to `PLimitVW=498.75`, the no-limit
    path, the Storage-deferred error, the VV_VW joint kW+kvar, and the
    double-push-dispatches-once pending-reset guard). **Corpus 50 → 74:** the 18
    SnapShot volt-watt + 6 SnapShot VV_VW cases (all PVSystem, tagged purely
    `unsupported_class=InvControl`) migrate into `solvable_now` and match the oracle
    full-model live (the Daily volt-watt/VV_VW cases stay Export-blocked, Phase 8).
    lib **593 → 598**.
  - **audit-code follow-up:** verdict faithful 1:1 (no Critical/Major) — every math
    helper, both `Sample` triggers, and both `DoPendingAction` branches match the
    Pascal line-for-line, incl. the subtle bits (VOLTWATT's inverter-off check
    without `VarFollowInverter`; the no-`abs(PLimitVW)>0` guard in VV_VW's
    `FVWOperation` reset; the `FPendingChange:=NONE` end-of-loop reset). **No code
    fix needed.** Surfaced-not-fixed (all confirmed acceptable): (1) the Storage
    VOLTWATT/VV_VW deferral is a *deliberate, loud* deferral (explicit error +
    documented), not silent degradation — accepted vs the plan's "verbatim" wording;
    (2) a missing/untied volt-watt curve `return Err`s (solve abort) where Pascal
    `DoSimpleMsg(381)+exit` logs and continues with the DER uncontrolled — a
    **pre-existing** pattern (step 2b does the same for `vvc_curve`), no gated case
    hits it, tracked for a uniform fix; (3) Pascal's `LoadsNeedUpdating := TRUE`
    (l.1605) has no Rust equivalent — a no-op in this architecture (`GetPCInjCurr`
    recomputes PC injections every iteration; confirmed by the exact 13/34 iteration
    pins); (4) `Calc_PBase`/`kw_out_desiredpu` moved from the Pascal `DoPendingAction`
    header into the VW/VV_VW branches — numerically equivalent (read only by the VW
    path) and sidesteps the deferred Storage DCkW read.
  - **audit-tests follow-up:** verdict strong (the goldens are oracle-pinned and the
    decks exercise the real behavior; the iteration-count pin caught the missing
    `FPendingChange` reset — 45 vs 34; mock values are independent hand-derivations).
    Closed the Major gap — the **adaptive `Change_deltaP_factor`** path (the
    `DeltaP_factor` unset / `FLAGDELTAP` branch) had **zero** coverage: the
    fixed-factor `invcontrol_voltwatt` golden + all 24 migrated corpus cases set
    `DeltaP_factor` explicitly. Added the snapshot golden **`phase7/invcontrol_voltwatt_adaptive`**
    (the same deck with `DeltaP_factor` *unset* → the adaptive bands run for all 13
    iterations; matches the oracle bit-for-bit). Added `invcontrol_voltwatt` /
    `_adaptive` / `vv_vw` to the `must` required-scenario guard (`golden_phase7.rs`),
    and a VV_VW Storage-deferred mock (symmetry with the VOLTWATT one). lib **598 →
    599**. **Daily volt-watt was a real PORT BUG (found + fixed) — the earlier
    "ill-conditioned, not a bug" note was WRONG:** a *daily* (multi-step) run diverged
    from the oracle by ~kW at the limiting steps — and crucially a *fixed* `DeltaP_factor`
    diverged just as much as the adaptive one, so the adaptive bands were never the
    cause. Per-control-iteration trajectory comparison (Rust ↔ oracle) pinned it:
    `update_inv_control` was missing Pascal `UpdateInvControl`'s per-step reset block
    (l.2555-2575) — **`FFlagVWOperates` latched across time steps**, forcing the damped
    "requesting" VW branch on every later step (even non-limiting steps then ran 6-8
    control iters vs the oracle's 2, slowly ramping `PLimitVW` instead of taking the
    curve point directly), and `FdeltaPFactor` was not reset to `DELTAPDEFAULT` each
    step (`FdeltaQFactor` deliberately is not — Pascal l.2574). Added the reset (flag +
    P factor + mode/operation flags); Rust now matches the oracle to full precision at
    every step. Gated by the new daily golden **`phase7/invcontrol_voltwatt_daily`**
    (the previously-rejected daily case, now committed — the snapshot golden alone could
    not catch a cross-step bug).
    (2) the `Check_Plimits` kVA/pctPmpp clamp arms have **live-only** coverage (the
    10 `*kVAlimitation/kvarlimitation/varP/wattP/pmpp_greater_kva` corpus cases) — no
    committed offline pin, mirroring the step-2b live-only kVA/kvar-clamp note.
- **step 2d — the DRC + VV_DRC dispatch (`control/inv_control/compute.rs`).**
  Adds the **DRC** (dynamic reactive current) single mode and the **VV_DRC** combi
  mode. DRC needs **no curve**: `CalcQDRC_desiredpu` derives the desired Q from the
  per-step voltage *change* vs the DRC rolling-average window
  (`deltaVDynReac = FPresentDRCVpu − FDRCRollAvgWindow.AvgVal/Vbase`), clamped by a
  deadband (`DbVMin`/`DbVMax`) × slope (`ArGraLowV`/`ArGraHiV`); `CalcDRC_vars` is
  the delta-Q convergence over `QOldDRC` (same shape as `CalcVoltVar_vars` minus the
  curve-hysteresis branch). **VV_DRC** sums the volt-var curve Q *and* the DRC Q in
  one `CHANGEDRCVVARLEVEL` action (`CalcVVDRC_vars`). `Sample`'s DRC/VV_DRC triggers,
  the `DoPendingAction` DRC/VV_DRC branches, and the `Check_Qlimits` error bands
  (DRC = 0.0005, VV_DRC = 0.005) + operation-flag assignments (`FDRCOperation`/
  `FVVDRCOperation`) ported. The shared `Change_deltaQ_factor` adaptive band logic
  was **extracted from `CalcVoltVar_vars`'s inline** into `change_deltaq_factor`/
  `update_deltaq_factor` so VOLTVAR/DRC/VV_DRC share it (the existing VOLTVAR
  golden + 6 corpus cases prove the refactor is behavior-preserving). The
  `UpdateInvControl` per-step reset now also clears `DRCmode`/`FDRCOperation`/
  `FVVDRCOperation` (the cross-step-leak class, like `FFlagVWOperates` in 2c).
  - **DRC is a no-op in a pure snapshot** (the DRC rolling-average window is fed only
    by the time-series `EndOfTimeStepCleanup`, so `AvgVal = 0` → `deltaV = 0` →
    `QDesireDRCpu = 0`), so the targeted goldens are **daily** runs (like
    `invcontrol_voltvar_avg`), not snapshots. New env method `der_set_drc_mode` +
    `dyna_t` (the `Dynavars.t = 1` guard); new `MonitorVar::{DrcAvg,DrcOperation,
    VvDrcOperation}` (mode-3 monitor state, unobservable until WP7.7).
  - **Also landed the `IntervalUnits` time-unit suffix parse** (the user-requested
    1:1 parser fidelity): `AvgWindowLen` / `DynReacAvgWindowLen` accept a trailing
    `h` (×3600) / `m` (×60) / `s` (×1) char (a bare number = seconds), matching
    Pascal `DSSObjectHelper.pas` l.273/325 exactly (lowercase-only `case`, bad
    number/unit logs error 2020034/2020035 and leaves the field unchanged). New
    `PropFlags::INTERVAL_UNITS` + `parse_interval_units_{i32,f64}` in the shared
    property engine (Integer + Double arms); the corpus DRC/VV_DRC family uses the
    `2s` form.
  - **Storage DRC/VV_DRC:** DRC is **not** Storage-guarded (unlike VOLTWATT/VV_VW) —
    `do_pending_drc`/`_vv_drc` set `var_mode = KVAR` via `der_set_modes` for both
    PV and Storage, and DRC dispatches kvar setpoints (no Storage state flip), so
    the WP7.4 YPrim-rebuild concern that gated Storage volt-watt does not apply.
  - **Gate:** targeted goldens `phase7/invcontrol_drc` (a daily PV on a weak line;
    the DRC absorbs vars per `CalcQDRC_desiredpu` → ~3.4 kvar/phase final, 20 iters)
    + `phase7/invcontrol_vv_drc` (the joint VV+DRC Q, ~14.6 kvar/phase, 14 iters) +
    **5 mock-env tests** (DRC absorb pinned to `QDesireDRCpu=-2.5`/`QDesiredDRC=-120.8`;
    the snapshot no-op `QDesireDRCpu=0`; the VV_DRC sum `QDesireVVpu=-0.125`+
    `QDesireDRCpu=-0.5`→`QDesiredVVDRC=-75.8`; the `CHANGEDRCVVARLEVEL` push; the
    still-deferred WATTPF aborts loudly) + 3 oracle-pinned `props/invcontrol.json`
    suffix scenarios (`s`/`m`/`h` on both props) + 3 `setters` unit tests (the
    suffix conversions + the bad-unit/uppercase/empty error path). **Corpus stays
    74** (the DRC/VV_DRC corpus family is all daily + `Export`/`Plot`-blocked →
    Phase 8, like the Daily volt-var/volt-watt cases). lib **599 → 607**.
  - **audit-code follow-up:** verdict faithful 1:1 (no Critical/Major) — every DRC/
    VV_DRC math helper, both `Sample` triggers, both `DoPendingAction` branches, the
    `Check_Qlimits` error-band/flag additions, the per-step reset extension, and the
    IntervalUnits parse map line-for-line to the Pascal; **no fix needed**.
    Surfaced-not-fixed (both unreachable/unobservable): (1) `drc_avg_pu` guards
    `f_vbase=0`→0 where Pascal divides unconditionally — but that value feeds only the
    mode-3 monitor `Set_Variable(6/15)` (unobservable until WP7.7) and `f_vbase` is
    never 0; the *observable* `CalcQDRC_desiredpu` path is unguarded, matching Pascal;
    (2) the WATTPF/WATTVAR/AVR `Check_Qlimits` error bands are omitted (those modes are
    rejected at `Sample`, so unreachable until 2e). Confirmed `val_i32`/`val_f64` are
    strict (reject `2h`/`2s`) so the suffix path fires — empirically pinned by the
    props golden (`1h`→3600, `2h`→7200).
  - **audit-tests follow-up:** verdict strong (the daily goldens are oracle-pinned and
    DRC genuinely operates — a broken DRC→Q=0 fails both the element-power and the
    node-voltage checks; the mock values are independent hand-derivations; the suffix
    conversions are pinned against the **oracle**, not regenerated Rust output). Closed
    the one gap — added an **end-to-end IntervalUnits error-path test** (a bad unit
    `avgwindowlen=2x` logs the error + leaves the field at its default through the parse
    engine; the happy path was already oracle-pinned). Surfaced-not-fixed (both
    acceptable): the daily golden pins the final converged state + last-step iteration
    count (not a per-step trajectory) — adequate for DRC (no convergence latch like
    `FFlagVWOperates`; matches the `voltvar_avg` precedent), but a per-step pin will be
    needed if step 2e's LPF/RoC (real cross-step state) reuses this golden shape; and
    the **Storage DRC/VV_DRC** path is unexercised (consistent with the existing
    untested Storage-VOLTVAR / multi-DER coverage, not a 2d regression). lib **607 →
    608**.
- **step 2e-i — the WATTPF + WATTVAR dispatch (`control/inv_control/compute.rs`).**
  Adds the two **curve-based watt modes** (both single, both `CHANGEVARLEVEL`):
  - **WATTPF** (watt-pf): `CalcQWPcurve_desiredpu` reads the power factor off
    `wattpf_curve` at the panel pu (`FDCkW·FEffFactor·FpctDCkWRated/FDCkWRated`)
    into the object-level `pf_wp_nominal`, then the desired kvar = `p·tan(acos(pf))·
    sign(pf)` (`p` = the panel power off-priority, else `kW_out_desired`);
    `Check_Qlimits` clamps it (WATTPF forces `QHeadRoom := kvarLimit` and takes the
    watt-priority arm); `CalcWATTPF_vars` turns the clamped pu Q into the kvar
    set-point (no deltaQ convergence). A PVSystem also stores `pf_wp_nominal` (so
    its nominal applies the pf via the already-ported `wp_mode` branch).
  - **WATTVAR** (watt-var): `CalcQWVcurve_desiredpu` reads Q (pu of headroom) off
    `wattvar_curve` at the panel pu (`Pbase = min(kVArating, DCkWrated)`);
    `Check_Qlimits_WV` (the WV-specific clamp — **no** watt-priority arm) limits it;
    `Calc_PQ_WV` keeps the final (P, Q) on the watt-var curve and inside the kVA
    circle, solving the **watt-var-line ∩ kVA-circle quadratic** (`GetCoefficients`/
    `GetXValue`/`GetYValue`) when the request exceeds `kVArating`; `CalcWATTVAR_vars`
    sets the kvar. A PVSystem also sets its kW = `PLimitEndpu·min(kVArating,
    DCkWrated)`.
  - **Shared:** the `Sample` WATTPF/WATTVAR triggers (both use `QoutputVVpu`, the
    var-mode inverter-off check); the `DoPendingAction` branches; `check_qlimits`
    gained the WATTPF error band (0.005) + `FWPOperation` arm; the per-step
    `UpdateInvControl` reset now also clears `FWPOperation`/`FWVOperation`. New
    `DerSnap.pf_priority` + `InvVars.f_pf_priority` (cached `GetPFPriority`, read by
    `CalcQWPcurve`); new env methods `der_set_wp_mode`/`der_set_wv_mode`/
    `der_set_pf_wp_nominal`/`der_is_pvsystem`; new `MonitorVar::{WpOperation,
    WvOperation}`.
  - **Storage WATTPF/WATTVAR** is **not** guarded (unlike VOLTWATT/VV_VW): both push
    kvar set-points (WATTVAR's PVSystem-only kW push is gated on `der_is_pvsystem`),
    so the WP7.4 YPrim-state-flip concern does not apply — but the Storage path was
    **unexercised** here (no Storage WATTPF/WATTVAR gate), and was in fact *silently
    broken* (the `Varmode` gap the step-2e-ii audit-code found + then ported to working;
    see the step-2e-ii Storage follow-up).
  - **NOT_PORTED / deferred (each an explicit error):** AVR (→ 2e-ii), the `MonBus`
    path + LPF/RiseFall (→ 2e-iii), GFM/Exponential (→ WP7.7).
  - **Gate:** targeted goldens `phase7/invcontrol_wattpf` (a 1000 kW PV, pf=-0.9 at
    full output → ~484 kvar absorbed, 6 iters) + `phase7/invcontrol_wattvar` (the
    watt-var Q request + P land **exactly on the 1000-kVA circle** via the quadratic,
    6 iters) — both matched the oracle bit-for-bit first-run — + **3 mock-env tests**
    (the WATTPF curve→kvar step pinned to `p·tan(acos 0.9)`, the WATTVAR curve→kvar,
    the still-deferred AVR aborts loudly). **Corpus 74 → 76:** the 2 SnapShot
    WATTPF/WATTVAR cases (`watt-pf_watt-var/dss/SnapShot_{wattpf,wattvar}.dss`)
    migrate into `solvable_now` and match the oracle full-model live. lib **608 →
    610**.
  - **audit-code follow-up:** verdict faithful 1:1 (no Critical/Major) — every
    WATTPF/WATTVAR math helper, both `Sample` triggers, both `DoPendingAction`
    branches, the `check_qlimits` WATTPF extension, and the per-step reset map
    line-for-line to the Pascal + the oracle goldens. Fixed **1 Minor ordering
    divergence**: `Calc_PQ_WV` read `Qbase` from the *post*-`CalcWATTVAR_vars`
    `QDesiredWV` where Pascal reads it at the procedure top (the *prior* value, 0 on
    the first firing) — moved the `Pbase`/`Qbase` read before the first
    `CalcWATTVAR_vars` (Pascal l.3348-3359). The probe showed this only perturbs the
    iteration-1 *transient* (the control fixpoint is `QDesiredWV`-sign-consistent, so
    the converged P/Q are identical even for asymmetric kvar limits — the simulated
    pre-fix ordering still passed every golden), so it is a faithfulness/trajectory
    fix, not a converged-value bug. Added golden `phase7/invcontrol_wattvar_asym`
    (kvarMax=800/kvarMaxAbs=400 → the kVA-circle quadratic with QHeadRoom ≠
    QHeadRoomNeg; lands on 1000 kVA, matched the oracle) — the only gate exercising
    the quadratic with asymmetric headroom. Surfaced-not-fixed (unobservable until
    WP7.7): the Storage mode-3 monitor var-index aliasing (Pascal writes both
    `FWPOperation` and `FWVOperation` to Storage `Set_Variable(16)`; the Rust env
    keeps separate `wp_operation`/`wv_operation` fields — only one fires per Sample,
    so it is functionally equivalent and the slot is unread until the mode-3 monitor
    body lands). lib **610** (golden-only add).
  - **audit-tests follow-up:** verdict strong (the goldens are oracle-pinned —
    `gen_phase7.py` asserts the PIN versions, values from the oracle not regenerated
    Rust — and compared exact-iteration + node-order + 1e-6 V/P; the 2 corpus cases
    are live full-model-compared; the mock values are independent hand-derivations;
    the replaced `wattpf_mode_aborts_not_silently` → `avr_mode_aborts_not_silently`
    keeps the deferral-never-silent guard on the now-deferred mode). Closed the one
    real gap — the kvar-limited `Calc_PQ_WV` `GetXValue(QDesireEndpu)` branch
    (`|FWVOperation| = 0.2`) was covered **live-only** (the corpus `SnapShot_wattvar`
    curve reaches the full-headroom limit; my other wattvar goldens stay below it):
    added the offline golden `phase7/invcontrol_wattvar_qlim` (a y=-1.0 curve → the
    kvar limit fires → `GetXValue`; matched the oracle). Strengthened
    `wattvar_first_step_curve_to_kvar` to also pin the PVSystem kW push
    (`requested_kw == 600`). Surfaced-not-fixed (consistent with the established
    Storage-fleet gap): the Storage WATTPF/WATTVAR path stays unexercised (every gate
    is PVSystem-only, like Storage-VOLTVAR/DRC). lib **610** (golden + mock-assert
    only).
- **step 2e-ii — the AVR dispatch (`control/inv_control/compute.rs`).** Adds the
  **AVR** (active voltage regulation) single mode — the 3-stage DQDV regulator
  (`CHANGEVARLEVEL`, no curve): `Sample`'s AVR trigger (three voltage/var conditions
  OR `ControlIteration=1`; PVSystem sets `AVRmode`, Storage `VVmode` — verbatim
  Pascal l.2051-2054) + the `DoPendingAction` control-iteration state machine —
  **iter 1** seeds `FAvgpVpuPrior`/`FAvgpAVRVpuPrior` and pushes `QHeadRoom/2` kvar,
  **iter 2** estimates the `DQDV` sensitivity from the resulting voltage change,
  **iter 3+** runs the regulator (`CalcQAVR_desiredpu` → `Check_Qlimits` →
  `CalcAVR_vars`) driving the monitored voltage toward `Vsetpoint`. The
  `check_qlimits` AVR error band (0.005) + `FAVROperation` arm, and the
  `UpdateInvControl` per-step reset of `DQDV`/`FAVROperation`, were added. New
  `InvVars` AVR fields (`QDesiredAVR`/`QOldAVR`/`QoutputAVRpu`/`QDesireAVRpu`/
  `FAVROperation`/`DQDV`/`Fv_setpointLimited`/`FAvgpAVRVpuPrior`) + the env method
  `der_set_avr_mode`. `Fv_setpoint` (the `Vsetpoint` prop) was already wired in 2a.
  - **REAL PORT BUG found + fixed — the earlier step-2c "`LoadsNeedUpdating` is a
    no-op in this architecture" note was WRONG.** AVR's iter-1/2 dispatch sets
    `kvarRequested` *without* calling `SetNominalDEROutput` (verbatim Pascal — iter 1
    pushes `QHeadRoom/2`, iter 2 only reads it back for `DQDV`); it relies on Pascal
    `DoPendingAction`'s `LoadsNeedUpdating := TRUE` (l.1605) to make the *next* solve
    re-run `SetNominalDEROutput` over the fleet. The Rust `InvDispEnv` never set that
    flag (the other modes call `der_set_nominal` explicitly), so AVR iter-2 read a
    **stale kvar = 0 → DQDV = 0 → the regulator never moved the voltage → max control
    iterations exceeded (no convergence)**. Fixed by adding
    `env.set_loads_need_updating()` at the end of `do_pending_action` (matching Pascal
    l.1605, the GenDispatcher/StorageController pattern). **Idempotent for the other
    modes** (re-applying `SetNominalDEROutput` from the same request is a no-op): the
    VOLTVAR/VOLTWATT/VV_VW/DRC/VV_DRC/WATTPF/WATTVAR goldens + all 76 corpus cases
    still match the oracle at their exact prior iteration counts. (Per the
    [[dont-rationalize-conditioning]] discipline — a deferred "no-op" claim hid a real
    gap until the mode that depended on it landed.)
  - **AVR is a snapshot mode** (the regulator converges across *control* iterations
    within one solve, not across time steps; it uses `FPresentVpu`/`Vsetpoint`/`DQDV`,
    not the rolling-average window), so the targeted golden is a snapshot. The DQDV law
    is heavily damped (a hard-coded 0.2 step + the `DQmax` clamp), so convergence takes
    ~250 control-loop iterations — pinned exactly.
  - **NOT_PORTED / deferred (each an explicit error):** GFM/Exponential (→ WP7.7); the
    `MonBus` path + LPF/RiseFall (→ 2e-iii). *(The Storage AVR path was initially
    unexercised + silently broken — found by the audit-code follow-up, then ported to
    working; see the step-2e-ii Storage follow-up.)*
  - **Reproduced verbatim (plain comment, not `TODO(compat)`):** the dead
    damping-band block in `CalcQAVR_desiredpu` (Pascal l.3170-3182), immediately
    overwritten by the unconditional `FdeltaQFactor := 0.2` (l.3184); and the AVR
    event-log string that literally reads "VOLTVAR mode requested …" (a Pascal
    copy-paste at l.1124-1126).
  - **Gate:** targeted golden `phase7/invcontrol_avr` (a 600 kVA PV on a weak line,
    `Vsetpoint=0.98`, VARMAX → the AVR absorbs ~119 kvar/phase to pull the bus voltage
    to exactly 0.9801 pu; node voltages + the PV terminal power 1e-6 **and the exact
    250 iters** — matched the oracle bit-for-bit after the `loads_need_updating` fix) +
    **3 mock-env tests** (the iter-1 `QHeadRoom/2` seed, the iter-2 `DQDV` estimate, the
    iter-3 regulator step with the `DQmax` clamp pinned to `QDesireAVRpu=-0.1`/
    `QDesiredAVR=-12`); the deferral guard flipped `avr_mode_aborts_not_silently` →
    `gfm_mode_aborts_not_silently` (GFM is the now-deferred mode). **Corpus stays 76**
    (the `Test/InvControl*` family has no AVR case — the only AVR full-solve gate is
    the targeted golden). lib **610 → 613**.
  - **audit-code follow-up:** verdict faithful 1:1 for the **PVSystem** AVR path
    (golden-pinned; every helper/branch matches Pascal line-for-line incl. the dead
    band block, the ControlIteration=3 reset, the literal-0.2 step, and the
    `LoadsNeedUpdating` fix wired into both env sites). Fixed **1 Major** silent
    degradation: **Storage AVR ran but regulated nothing.** Pascal's AVR
    `DoPendingAction` sets `Varmode := VARMODEKVAR` for *every* DER, but the Rust port
    routes that side effect through `der_set_kvar_requested`, which sets `var_mode`
    only for PVSystem — so a Storage kept its `VARMODE_PF` default and
    `set_nominal_der_output` discarded `kvar_requested` (the AVR kvar silently lost →
    `DQDV≈0` → no regulation, no error). The original STATUS claim "like Storage-
    VOLTVAR/DRC coverage" was **wrong**: VOLTVAR/DRC set `var_mode` via `der_set_modes`
    (both DER types) so they would work; AVR/WATTPF/WATTVAR did not. **Same root cause
    in the prior step 2e-i** (Storage WATTPF/WATTVAR shared the gap). Fixed all three
    with a loud `guard_storage_var_mode` (the `guard_storage_vw` pattern) — a Storage
    in AVR/WATTPF/WATTVAR now errors explicitly (the deferral-is-never-a-silent-skip
    rule); PVSystem unaffected (all PVSystem goldens/corpus still match). 3 mock tests
    (`{avr,wattpf,wattvar}_storage_is_deferred_not_silent`, later replaced by the
    positive `*_storage_dispatches_in_kvar_mode` tests when the path was ported) + the
    corrected
    `do_pending_avr` `Varmode` comment. **Deferred (loud):** porting the Storage
    `Varmode` + the Storage iter-2 DQDV source (Pascal reads `kvarRequested`, not the
    achieved kvar) for these modes, until a Storage smart-inverter gate exists. lib
    **613 → 616**. *(A second instance of [[dont-rationalize-conditioning]] — a
    "deferred no-op / unexercised" justification hid a real gap.)* **The guarded
    Storage paths were subsequently ported to working — see the Storage follow-up
    below.**
  - **audit-tests follow-up:** verdict strong (the `invcontrol_avr` golden is a real
    oracle pin compared at full strength — exact 250 iters + node order + 1e-6 V/P; the
    3 mock values are independent hand-derivations, not snapshots). Closed the two real
    gaps with **two oracle-pinned goldens**: (1) **`phase7/invcontrol_avr_daily`** — the
    Major finding (no multi-step AVR coverage): a 6-step daily run re-regulating to
    `Vsetpoint` each step (final ~117 kvar/phase, V=0.98), the AVR analog of
    `invcontrol_voltwatt_daily`/`_voltvar_avg`, guarding the EndOfTimeStepCleanup →
    `UpdateInvControl` → next-step-restart path + fleet persistence (matched the oracle
    bit-for-bit, 598 iters). (2) **`phase7/invcontrol_avr_kvarlim`** — the
    `Fv_setpointLimited` LIMITED branch + the AVR arm of `Check_Qlimits` (Minor
    findings): `Vsetpoint=0.95` with kvarMax=50 makes the setpoint unreachable, so the
    kvar limit clamps the request and `|QEnd−QLimited| < 0.05` drives
    `Fv_setpointLimited := FPresentVpu` — the branch the unclamped main golden never
    reaches (fleet caps at ~16.3 kvar/phase, V≈1.005, 110 iters). **Surfaced-not-fixed
    (accepted):** the per-step `DQDV`/`FAVROperation` resets in `UpdateInvControl` are
    faithful-to-Pascal but **unobservable** (DQDV is re-estimated every step's iter-2;
    FAVROperation is write-only until the mode-3 monitor, WP7.7), so no test (the daily
    golden included) discriminates *those exact resets* — the daily golden pins the
    multi-step convergence broadly instead. The check_qlimits *clamp arithmetic* is
    shared with VOLTVAR (already tested); only the AVR error-band → FAVROperation delta
    is AVR-specific (and unobservable). lib **616** (golden-only add).
  - **Storage smart-inverter follow-up (user-requested) — Storage AVR/WATTPF/WATTVAR
    now WORK** (the audit-code follow-up had *guarded* them loudly; this ports them).
    Root fix: the dispatch now sets the DER `Varmode := VARMODEKVAR` for **both** DER
    types via a new `der_set_var_mode` env method (Pascal's explicit
    `Varmode := VARMODEKVAR`, l.1059/1134/1189) — so a Storage's `kvarRequested` is
    applied by `set_nominal` instead of discarded by its `VARMODE_PF` default. Also: the
    AVR iter-2 `DQDV` reads `kvarRequested` for a Storage (Pascal l.1081; PVSystem reads
    the achieved `Presentkvar`) via `der_requested_kvar`; the WATTVAR kW push stays
    PVSystem-only. The three `guard_storage_var_mode` guards are removed. **Empirical
    note (oracle-probed):** Storage **AVR regulates** identically to PVSystem (Q≈119.5
    kvar/phase → V=0.98); Storage **WATTPF/WATTVAR** read their curves at panel pu **0**
    (Pascal `FDCkW := 0.0` for Storage, l.1749, "not using it"), so they regulate only
    via a non-trivial curve `y(0)` (WATTVAR) or `WattPriority` making the watt term
    non-zero (WATTPF) — not degenerate-zero in general, but driven by `y(0)`. **Gate:**
    3 oracle-pinned goldens `phase7/invcontrol_{avr,wattpf,wattvar}_storage` (Storage
    DER, full-model + Storage SOC/state, matched the oracle bit-for-bit:
    AVR≈119.5, WATTPF≈54.8, WATTVAR≈60 kvar/phase) + the 3 mock
    `*_storage_is_deferred_not_silent` tests replaced by positive
    `*_storage_dispatches_in_kvar_mode` tests (pin `Varmode := VARMODE_KVAR` is set +
    the Storage kvar request). **Corpus stays 76** (no Storage InvControl corpus case).
    lib **616** (deferred tests → positive tests, net 0; goldens added).
    - **audit-code follow-up (Storage port):** verdict **faithful 1:1, no fix** —
      an independent agent verified every Storage path matches Pascal line-for-line
      (the `Varmode := VARMODEKVAR` set for both DER types; the AVR iter-2 DQDV source
      PVSystem=achieved/Storage=requested; the WATTVAR kW push correctly PVSystem-only;
      `pf_wp_nominal` PVSystem-only), confirmed `guard_storage_vw` (VOLTWATT/VV_VW) is
      preserved + still called, and re-ran the gate green. No empty fix-commit.
    - **audit-tests follow-up (Storage port):** verdict strong with two real gaps,
      both proven by **controlled revert** (the auditor showed the suspect change
      survived with all tests green). (1) **Major — the AVR iter-2 `der_requested_kvar`
      Storage source was unpinned** (in `invcontrol_avr_storage` the achieved kvar ==
      the requested kvar at iter-2, so reverting to `der_present_kvar` passed). Closed
      with golden **`phase7/invcontrol_avr_storage_wattprio`** — a `WattPriority`
      Storage backs off kvar to the kVA circle, so `present_kvar < requested_kvar` at
      iter-1; reverting the Storage DQDV source to the achieved kvar now shifts the
      iteration count (60 vs 58) → the golden **fails**, proving it discriminates. (2)
      Minor — the WATTVAR PVSystem-only kW-push gate + the degenerate WATTPF mock:
      strengthened `wattvar_storage_dispatches_in_kvar_mode` to pin the Storage kW is
      **not** pushed (`requested_kw` unchanged at 400 — a dropped `if is_pv` would
      overwrite it with 600), and `wattpf_storage_dispatches_in_kvar_mode` to pin a
      **non-zero** kvar (curve y(0)=-0.95 + WattPriority → -131.47, not just
      `var_mode==1`). lib **616** (golden + mock-assert only).
    - **24-hour multi-step endurance goldens (user-requested).** A full 24-step hourly
      run per mode × DER type — `phase7/invcontrol_{avr,wattpf,wattvar}_24h` (PVSystem,
      a varying day-shape) + `phase7/invcontrol_{avr,wattpf,wattvar}_storage_24h`
      (Storage, low kWrated so it stays discharging at hour 24). Each ends on an active
      step (so the final-step golden is non-trivial) and is matched against the oracle
      bit-for-bit (full model + Storage SOC/state + exact iteration count). **How much
      cross-step state each carries** (so the framing doesn't overclaim — an
      audit-tests finding below): the **Storage** trio carries the integrated SOC
      genuinely (**%stored 100% → 32.7%** over 24 steps, revert-proven); **AVR**
      (PVSystem + Storage) warm-starts each step via `QOldAVR`, so its iteration-count
      pin is cross-step sensitive; the **PVSystem WATTPF/WATTVAR** pair are
      feed-forward/memoryless (no deltaQ convergence, no rolling window), so those two
      verify the daily-mode run *completes* + pin a real operating point but do **not**
      carry cross-step state (the PVSystem rolling-window cross-step path is covered by
      `invcontrol_voltvar_avg`). **6 new goldens; corpus stays 76.**
    - **audit-tests follow-up (commit 279aed9: wattprio + 24h goldens):** an
      independent agent **reproduced all three controlled reverts** — the
      `invcontrol_avr_storage_wattprio` discriminator (forcing the achieved kvar for
      Storage's iter-2 DQDV fails it, 60 vs 58), the Storage-AVR-24h SOC carry (zeroing
      the discharge integral fails the `%stored` pin), and the WATTVAR no-kW-push mock
      (dropping `if is_pv` pushes `requested_kw` to 600) — confirming each is a genuine
      gate. The WATTPF non-zero-kvar mock value (−131.47) is independently formula-
      derived, not a snapshot. **One real finding (Minor): the STATUS note + the
      `gen_phase7.py` deck comment over-claimed "each step carries from the previous"
      for the PVSystem WATTPF/WATTVAR 24h pair** — those modes are memoryless, so those
      two goldens are single-operating-point pins of the daily-mode path, not endurance
      discriminators. Fixed by correcting both docs to state per-golden exactly what
      carries (Storage SOC + AVR `QOldAVR`; WATTPF/WATTVAR PV = feed-forward). The
      goldens are kept (they still pin a real oracle operating point + prove the
      daily-mode run completes for those modes). **Surfaced-not-added (redundant):** a
      VV_avg 24h or Storage-AVR-WattPriority 24h deck — the PVSystem rolling-window
      cross-step carry is already pinned by `invcontrol_voltvar_avg`, and the iter-2
      requested-kvar source by the single-step `invcontrol_avr_storage_wattprio`.
    - **Genuine rolling-window endurance goldens (user-requested follow-up).** Since a
      WATTPF/WATTVAR rolling-window test provably has **no teeth** (their Q is
      feed-forward — a controlled ×2 corruption of the windowed `present_vpu` passes,
      because `present_vpu` only feeds their trigger), two VOLTVAR-avg 24h goldens were
      added instead, where the window *does* drive the output (the curve is read at
      `present_vpu = vpresent/avg_val`): **`invcontrol_voltvar_avg_24h`** (single
      PVSystem, `AvgWindowLen=6h` → a real 6-step rolling average; oracle avg Q≈1.6 vs
      rated ≈12.8 kvar/phase) and **`invcontrol_voltvar_mixed_24h`** — one InvControl
      driving a **mixed PVSystem + Storage fleet** (the **first multi-DER gated case**,
      closing the single-DER-only gap audit-tests flagged at step 2b; it exercises the
      fleet loop + the window + the Storage SOC carry together). Both have **teeth,
      proven by controlled revert**: corrupting the avg branch ×1.02 fails each (the
      mixed one isolated to `|diff|=0.82 > 7.2e-3`). **2 new goldens; corpus stays 76.**
    - **Per-STEP (per-hour) comparison for every multi-step `golden_phase7` deck
      (user-requested).** The phase7 command-replay golden previously compared only the
      **final** step's state; the daily/duty decks now also pin the **per-hour
      trajectory**. `gen_phase7.py` `add_step_monitors` auto-adds a `mode=1 ppolar=no`
      (rectangular P/Q) Monitor on every controlled DER of any `mode=daily`/`mode=duty`
      deck; `build()` captures every Monitor's channels; `golden_phase7.rs` compares
      them elementwise via the shared `compare_monitor` (the same comparator
      `golden_phase6`/`corpus_live` use). So all **16 daily phase7 goldens** (the
      InvControl daily/24h family + `storage_daily{,_charge}` + `storagecontroller_daily`)
      now pin each DER's P/Q at **every step** against the oracle, not just the endpoint
      — closing the "final-state only" gap. **`ppolar=no` (rectangular P/Q) is a
      deliberate, evidence-checked choice, not a workaround:** it pins the *actual* P/Q
      (so a real reactive divergence is fully caught), whereas the polar form's power
      angle `atan2(Q,−P)` has an ill-defined sign when Q is numerically zero — at a
      unity-pf hour the WATTPF curve gives pf=1 → Q=0, and a probe confirmed **oracle
      Q=0.0 vs port Q=−6.3e−15** (machine epsilon) there, i.e. +180° vs −180° is the
      sign-of-zero, NOT an opposite reactive flow (the port and oracle agree on Q to 15
      sig figs; at the next hour, where Q is a real +4e−12, both agree on sign too). So
      no bug was hidden — rectangular keeps full sensitivity (|ΔQ|~1e-15 ≪ the power
      floor). **Survey of the other stages:**
      `golden_phase5` already steps per-hour (`steps[]`: per-step dblHour/iters/V),
      `golden_phase6`/`corpus_live` already compare monitor channels per-step, and
      `golden_checkpoints` pins the assembled model every step. The last gap —
      `golden_ieee8500`'s 24-step daily segment (which compared only the **cumulative**
      EnergyMeter registers) — is now also per-step: a `mode=1 ppolar=no` Monitor on the
      metered feeder head pins the per-hour P/Q (24 samples) via `compare_monitor`. So
      **every multi-step golden across all stages now compares per-step**, not just the
      final/cumulative state.
- **step 2e-iii — the LPF/RiseFall rate-of-change limiting + the explicit-`MonBus`
  monitored-voltage path (`control/inv_control/compute.rs` + `accessors.rs` + the
  dispatch env).**
  - **LPF / Rise-Fall (`CalcLPF`/`CalcRF`).** A first-order low-pass filter
    (`out = desired·(1−α) + prior·α`, `α = exp(−h/LPFTau)`) or a ramp limiter (clamp
    the per-step change to `±RiseFallLimit·h`) smooths/ramps the desired var/watt
    output against the **prior time step's** `QDesireOptionpu`/`PLimitOptionpu`
    (`FPrior*Optionpu`, latched once per step in `UpdateInvControl`, Pascal
    l.2551-2552). Wired into the VOLTVAR / DRC / VV_DRC / VOLTWATT / VV_VW
    `DoPendingAction` branches via two shared tails (`apply_roc_qlimit` /
    `apply_roc_plimit`): the INACTIVE arm is the unchanged clamp; the LPF/RF arm runs
    the filter then `Check_Qlimits` / `Check_Plimits` on the smoothed value.
    **Genuinely cross-step** (the filter references the prior step), so gated by DAILY
    goldens. Reproduced verbatim: the watts LPF/RF `PLimitEndpu := Min(PLimitLimitedpu,
    PLimitOptionpu)` is a **plain** min (Pascal l.1390/1478), not the abs/sign form the
    INACTIVE/var arms use.
  - **MonBus (`GetMonVoltage`'s `FUsingMonBuses` branch).** The `monBus` side-effect
    splits each `MonBus=` string into `FMonBuses` + `FMonBusesNodes` via
    `ParseAsBusName`; `GetMonVoltage` reads each monitored bus's complex node voltage
    (single-node, or a 2-node line-to-line difference) scaled by
    `BasekV·1000 / FMonBusesVbase`, reduced by `MonVoltageCalc` (AVG/MAX/MIN, or a
    specific phase — the last unexercised). The env gains `mon_bus_node_v(j, node)`
    (the `NodeV[Bus.GetRef(node)]` read, Pascal's 1-based-index quirk preserved),
    resolved to per-bus `RefNo` arrays at env build; **both** env sites (the
    Sample/Action dispatch + the UpdateAll feed) build it. `UpdateInvControl`'s
    `BasekV` faithfully uses `CtrlVars[i]` (i=1 — the InvControl-index `//TODO: check
    (i,j)` quirk, identical for the homogeneous gated fleets). MakeLike now copies
    `FMonBuses`/`FMonBusesNodes` (Pascal l.788-789).
  - **Gate:** targeted goldens `phase7/invcontrol_voltvar_lpf` + `_risefall` (daily
    VOLTVAR, a swinging irradiance shape so the desired Q jumps each hour; the per-step
    monitor pins the lag/ramp Q trajectory — LPF Q ≠ RiseFall Q step-for-step, both ≠
    INACTIVE; a **controlled revert** forcing INACTIVE fails the LPF golden at the
    iteration-count assertion + the step-2 monitor — the step-1 monitor matches, the PV
    being below cut-in at the low-irradiance first step) +
    `phase7/invcontrol_voltvar_monbus` (a snapshot monitoring an upstream bus `m` ≠ the
    PV's bus `b`, so MonBus ≠ self-monitoring) + **4 mock-env tests** (MonBus
    single-node AVG overriding the self voltage; the L-L node difference; the LPF
    smoothing formula; the RiseFall ramp clamp — all hand-derived). **Corpus 76 → 83:**
    the **3 SnapShot MonBus VOLTVAR cases** (`Mon_voltage_average{,_LL,_Mix}-2` —
    single-node / line-to-line / mixed specs, each monitoring the source bus A ≠ the
    PV's bus B) migrate into `solvable_now` and match the oracle full-model live, **plus
    4 `Local_voltage_*` self-monitoring cases** (a stale `unsupported_class=InvControl,
    PVSystem` tag — the self path was already ported in 2b; re-probed + migrated here).
    No `.dss` corpus deck sets `RateofChangeMode`, so LPF/RiseFall has corpus-free
    targeted-golden coverage only (like AVR). lib **616 → 620**.
  - **audit-code follow-up:** verdict faithful 1:1 for every supported (single-
    InvControl, valid-config) shape — the LPF/RF math, the five `DoPendingAction`
    tails (incl. the plain-`Min` watts quirk), the MonBus reduce + `GetRef` 1-based
    indexing, the `FPrior*Optionpu` latch, and `MakeLike` all match Pascal
    line-for-line. Fixed 2 Minor faithfulness items: (1) **`FUsingMonBuses` keyed off
    `mon_buses_name_list`** instead of the parsed `mon_buses` — Pascal `RecalcElementData`
    l.925 uses `Length(FMonBuses)` (which `MakeLike` copies; the name list it does
    not), so a `like=`-derived MonBus control silently took the self-monitoring path;
    re-keyed off `mon_buses` (+ a `make_like_preserves_monbus_path` test). (2) the
    `MonBusesVbase` scale guarded `vbase != 0.0` → 0 where Pascal divides
    unconditionally (l.1633/1637); restored the unconditional divide (the `.get` stays
    only for the never-hit OOB index). Surfaced-not-fixed (all unreachable / pre-
    existing): the `UpdateInvControl` `BasekV := CtrlVars[0]` (Pascal's `CtrlVars[i]`,
    i = the InvControl element index = 1 for every gated single-InvControl case —
    multi-MonBus-InvControl is both unreached *and* Pascal-self-buggy, like the
    existing `FVpuSolutionIdx` i=1 quirk); the `ParseAsBusName` error fallback + the
    empty-node / unresolved-bus ground reads (defensive, unreachable for valid MonBus
    specs). lib **620 → 621**.
  - **audit-tests follow-up:** an independent agent ran controlled reverts confirming
    the LPF golden + the MonBus golden have real teeth (forcing INACTIVE /
    self-monitoring fails them), the mock values are independent hand-derivations, and
    the corpus migration is legitimate (bijection holds, full-model live-compared).
    Closed 2 Major + 2 Minor gaps: (M1) **the `invcontrol_voltvar_risefall` golden had
    no teeth** — `RiseFallLimit=0.00005` (a 0.18-pu/step cap) never bound (the per-step
    Q swing is ~0.07 pu), so it reproduced the *unfiltered* trajectory byte-for-byte and
    a regression dropping RiseFall would have passed it; oracle-probed for a binding
    value → `RiseFallLimit=0.00001` (0.036-pu cap) makes the ramp bind (the Q trajectory
    now differs from INACTIVE by ≤7.6 kvar and from LPF), regenerated from the oracle
    (the iter count happens to equal INACTIVE=15, so the teeth come from the per-step
    monitor). (M2) **the active-power ROC path (`apply_roc_plimit`, VOLTWATT/VV_VW + the
    plain-`Min` clamp) had no test** — all 3 original goldens were VOLTVAR; added 2 daily
    VOLTWATT ROC goldens `phase7/invcontrol_voltwatt_{lpf,risefall}` (oracle-probed to
    bind: LPF τ=7200 lags the kW limit ~93 kW, RiseFall 0.00002 ramps it as a clean
    monotonic curve, both ≠ unfiltered), exercising the watts LPF + RF branches and the
    plain-`Min(PLimitLimitedpu, PLimitOptionpu)` clamp. (m1/m2) **the MonBus MAX/MIN +
    specific-phase reduce arms were untested** (the 3 migrated corpus cases + the AVG
    mocks only cover AVGPHASES; the MAX corpus cases are daily+Export-blocked) — added
    `monbus_max_min_reduce_folds_the_buffer` (MAX→1.05 / MIN→0.98 over a 1.00/1.05/0.98
    buffer where AVG would give 1.01) + `monbus_specific_phase_indexes_buffer_zero_based`
    (the 0-based cBuffer quirk). **Surfaced-not-fixed (accepted):** Storage-DER + MonBus
    and a multi-DER fleet sharing a monitored bus stay untested (the MonBus path
    selection is DER-orthogonal — low risk). lib **621 → 623**.
- **step 3 — `ExpControl` (`control/exp_control/{mod,accessors,compute,tests}`).**
  Port of `Controls/ExpControl.pas` (749 lines, upstream "adapted and simplified from
  InvControl for adaptive controller research"): an adaptive-`Vreg` volt-var control
  over a **PVSystem-only** fleet, on the InvControl/StorageController clone-out dispatch
  pattern (a PVSystem-typed `ExpDispatchEnv` in `dispatch.rs`, the fleet resolved lazily
  on the first `Sample`).
  - **The 14 properties + the list sync.** `PVSystemList` (bare names) and `DERList`
    (class-prefixed `PVSystem.<n>`) share **no backing** (distinct upstream StringLists)
    but the side effects keep them in lockstep — a write to either clears the fleet,
    sets `FListSize`, and rebuilds the other (prefix on / `StripClassName` off). No new
    enums (all scalars/bools/lists). `props/expcontrol.json` (4 oracle-pinned scenarios —
    defaults, full, list-sync, MakeLike) round-trips exactly; `VregTau` dumps plain (its
    `Units_s` is JSON-only).
  - **`MakePVSystemList`.** A named list keeps each found+enabled PVSystem (silently
    skipping missing/disabled — **no** 14403, unlike InvControl); an empty list scans
    every PVSystem (adding enabled ones to the fleet, every name to `FPVSystemNameList`).
    Each fleet member gets `AVRmode := TRUE`; `CtrlVars` (the per-DER
    `FPriorVpu`/`FPresentVpu`/`FLastIterQ`/`FLastStepQ`/`FTargetQ`/`FWithinTol`/`FVregs`)
    init to the Pascal seeds (last-iter/step kvar = −1, `Vreg := FVregInit`).
  - **`Sample`.** Present per-unit voltage = avg `|Vterminal|` / (`kVBase·1000`); the
    static-init `FVregInit ≤ 0` branch finds `Vreg` from the present voltage clamped into
    `[VregMin,VregMax]`; the not-injecting (`InverterON=false ∧ VarFollowInverter`) branch
    tracks `Vreg` and skips; otherwise a `Verr`/`Qerr`/iter-1 trigger queues
    `CHANGEVARLEVEL`.
  - **`DoPendingAction`.** `Qpu = −QVSlope·(Vpu − Vreg) + Qbias`, then `SetNominalDEROutput`
    → clamp to the dynamic headroom `√(1−(kW/kVA)²)` (or `1` if `PreferQ`) / inverter
    `kvarLimit/kVA` / `±QmaxLead`/`QmaxLag`; `PreferQ` curtails kW to `Plimit =
    kVA·√(1−Qpu²)` (writes `PresentkW`+`puPmpp`); the `FOpenTau` (= `Tresponse/2.3026`,
    the truncated-ln(10) `TODO(compat)`) low-pass lags the target (non-static modes only);
    `DeltaQ_Factor` moves the kvar one step; `LoadsNeedUpdating := TRUE`.
  - **`UpdateExpControl`** (the `ExpControlClass.UpdateAll` hook, `SolutionAlgs` l.92,
    right after the InvControl `UpdateAll` in `end_of_time_step_cleanup`): snapshot
    `FLastStepQ`, slew `Vreg` toward the present voltage by `VregTau`, clamp to
    `[VregMin,VregMax]`, write it back as PVSystem state var 5 (`Set_Variable(5,…)`).
  - **Gate.** `phase7/expcontrol_{daily,daily_preferq,24h}` — daily/24h runs where the
    per-step PV kvar (the auto-added `mode=1 ppolar=no` monitor) tracks the **adapting
    `Vreg` + the `FOpenTau` lag**, each step seeded by the prior step's converged voltage;
    `expcontrol_24h` (the VARY24 ramp over 24 continuous steps) is the cross-step
    state-carry endurance guard, `_daily_preferq` exercises the kW-curtail + `Qbias`
    branches. **13 mock-env tests** pin the per-call arithmetic (the `−264.0`/`−185.1`
    curve→clamp→delta step, the `Vreg` slew, the not-injecting / `PreferQ` / static-init
    branches, the list sync). **Corpus stays 83** — the only corpus ExpControl example
    (`Examples/ExpControl/Master.dss`) is Phase-8-blocked by `file=`-backed arrays +
    Export, independent of the class; no corpus case carries a stale
    `unsupported_class=ExpControl` tag (verified). lib **623 → 636**.
  - **audit-code follow-up:** verdict faithful 1:1 — no Critical/Major/Minor, 4 inert
    Nits. Fixed the one worth fixing: `pv_set_present_kvar` (the `Presentkvar :=` env
    write) also set `Varmode := VARMODEKVAR` with a comment claiming a `Set_Presentkvar`
    side effect, but `PVsystem.pas` l.334 declares `property Presentkvar … WRITE
    kvarRequested` — a *plain field write*, no var-mode side effect (DoPendingAction
    already set `Varmode := VARMODEKVAR` at its top, so the write was redundant +
    harmless). Dropped it + corrected the comment (goldens/tests unchanged — var-mode is
    KVAR either way). Surfaced-not-fixed (all inert): `ActiveTerminalIdx := 1` (PVSystem
    is single-terminal); the control phase count from the *first* vs *last* fleet member
    (ExpControl builds no Yprim — numerically irrelevant); `VregTau`'s `Units_s` flag
    (JSON-schema metadata only). lib stays **636**.
  - **audit-tests follow-up:** an independent agent confirmed the Vreg-slew goldens, the
    dispatch-math mocks, PreferQ, and the props/MakeLike surface all have real teeth
    (controlled break-tests), but flagged 2 gaps. (M1) **the FOpenTau LPF was exercised by
    nothing** — the daily goldens run under the default `CTRLSTATIC` control mode, where
    Pascal gates the filter OFF (`ControlMode<>CTRLSTATIC`, l.505), so `Tresponse` was
    inert decoration (proven: removing it from the daily/24h/preferq decks left the
    trajectories **byte-identical**). Added `phase7/expcontrol_duty` (a duty-cycle run →
    `TIMEDRIVEN`, where the LPF fires — oracle-probed off=`[18.97,16.90,…]` vs
    on=`[5.91,8.75,10.01,…]`, a lagged ramp) as the end-to-end FOpenTau gate + a mock
    `fopen_tau_lpf_lags_target_in_timedriven_mode` (hand-derived filtered target), and
    corrected the daily/24h deck comments (they pin the Vreg slew only). (M2) the 4
    `expcontrol_*` goldens were **absent from the `golden_phase7.rs` must-list** deletion
    guard — added. Also closed the multi-DER coverage gap
    (`do_pending_dispatches_each_fleet_member_independently` — a 2-PV fleet, distinct
    per-member Q) + the static-init low-clamp / in-band arms
    (`static_init_clamps_low_and_keeps_in_band`). lib **636 → 639**; +1 golden.

_(The live frontier pointer — "next = WP7.5 step 4, then WP7.6 (Harmonics)" — lives in
`STATUS.md` §1e, not here; archives are frozen records.)_
