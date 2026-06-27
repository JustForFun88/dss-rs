# Phase 7 — WP7.4 (DER B: Storage + StorageController) — archived per-step records

> **Archived from `STATUS.md`** (2026-06-27) to keep the live handoff lean. WP7.4
> (DER B) is **✅ COMPLETE** on the `phase-7-extended-elements` branch; these are the
> frozen per-step records (step 1 the `Storage` element, step 2 the real
> `StorageController`, the audit follow-ups, and the YPrim-rebuild fix), superseded
> only by the code and tests. The live `STATUS.md` §1e keeps a one-line-per-step
> summary. §3/§4/§5 cross-references resolve against `STATUS.md`. Plan:
> `PHASE7_PLAN.md` §WP7.4.

---

**WP7.4 (DER B: Storage + StorageController) — ✅ COMPLETE** (step 1 = the
Storage element, step 2 = the real StorageController).

**Step 1 — the Storage element.** Port of `PCElements/Storage.pas` (`TStorageObj`, 3556 lines — the
largest PC element) as a directory module `pc/storage/` (mod/nominal/solve/
registers/accessors/tests) on the Generator template, embedding the WP7.3 step-1
`InvBasedPceData` the way PVSystem does. Adds the **charge/idle/discharge state
machine** (`FState` ∈ {−1,0,1}) and an **integrated state of charge**
(`kWhStored`/`%stored`) that the time-step cleanup hook advances. Scope = the
**power-flow** Storage:
- `ComputePresentkW` (state + dispatch → terminal kW: discharge `kWrating·%Discharge`,
  charge `−kWrating·%Charge`, idle `−kWOutIdling`), `CheckStateTriggerLevel` (the
  Follow / trigger-level / `ChargeTime`-of-day dispatch), `ComputeInverterPower`
  (the idling-state branch + the cut-in/out-reflected-to-AC clamp cascade, ported
  loop-for-loop), `kWOut_Calc` (the VW requesting/limiting regions).
- `CalcYPrimMatrix` (state-dependent: charge `+YeqDischarge`, idle `0`, discharge
  `−YeqDischarge`), `DoConstantPQStorageObj`/`DoConstantZStorageObj` + the inverter
  clamp, daily/yearly/duty shapes (snapshot-clone), a real `DynamicEq` ref.
- **The SOC integration:** `ComputeDCkW` (ideal-inverter signed terminal kW, or the
  efficiency-curve `GetCoefficients`+`QuadSolver` solve — `XyCurveObj::get_coefficients`
  ported alongside) + the loss split (idling/inverter/charge-discharge) +
  `UpdateStorage` (the `(DCkW+idle)/eff·Δh` charge/discharge integration, the reserve/
  full clamps and the state flip), wired into `EndOfTimeStepCleanup` (Pascal
  `StorageClass.UpdateAll`) via a new `ckt.storages` list. Registers + `TakeSample`
  (discharge-hours-only) mirror PVSystem.
- Registration: `ElemKind::Storage` + the circuit list + `construct.rs` (after the
  protection controls, before PVSystem; Pascal `Storage_ELEMENT`); two new enums
  (`storage_state` Charging/Idling/Discharging, `storage_dispatch_mode`); `is_zone_pce`
  admits Storage; the Monitor mode-3 metered-kind branch classifies Storage as
  `PcElement` (mirrors the PVSystem fix); the `dispatch.rs` GFM pre-solve guard
  rejects `ControlMode=GFM` (WP7.7) with an explicit error (no silent PQ fallback).
  `SysCtx` gained `time_of_day`/`dyna_h` (the `ChargeTime` trigger).
- **Deferred** (matching PVSystem): GFM solve (`DoGFM_Mode`/`CalcGFMYprim`), harmonic
  injection, the dynamics state machinery + the state-variable interface
  (`NumVariables`/`Get_Variable`/`VariableName`), the user-written `UserModel`/
  `DynaModel` DLLs (never ported), `MakePosSequence` → WP7.6/7.7.
- **Gate:** `props/storage.json` (7 scenarios, all round-trip exactly); goldens
  `phase7/storage_{snapshot,clamps,daily}` — the snapshot/clamps pin the three
  discrete states' terminal powers, and **`storage_daily` pins the integrated SOC
  trajectory** (a 200 kWh battery discharging 6 h depletes to the 20 kWh reserve and
  flips to Idling — `kWhStored`/`%Stored`/`State` read back via `? …` and matched at
  1e-6, the WP7.4 "%stored/state trajectory exact over a daily run" gate); exec tests
  (`storage_snapshot_solves_clean`, `storage_daily_run_depletes_soc` — the
  EndOfTimeStepCleanup wiring guard, `storage_gfm_mode_errors_not_silent`,
  `storage_accepts_mode3_monitor`). **Corpus: 0 net growth (stays 44).** No corpus
  Storage feeder is unblocked by the element alone — all are gated behind `Plot`/
  `Export` (Phase 8), `InvControl` (WP7.5), `StorageController` (step 2),
  file-backed arrays, or oracle errors; the 10 probed candidates were
  re-tagged to their **real** blocker (was stale `unsupported_class=Storage`). The
  targeted golden is the focused gate; the live-corpus burn-down for Storage waits
  on WP7.5 / step 2 / Phase 8. lib **543 → 557**.
- **audit-code follow-up:** verdict faithful 1:1 (no Critical/Major). Marked the
  `UpdateStorage` GFM absorbing/recharge branch `NOT_PORTED → WP7.7` (it was an
  implicit drop; GFM is rejected at solve time so the branch is unreachable in
  power flow, but the marker makes the WP7.10 grep sweep catch it). Surfaced-not-
  fixed (all upstream-faithful / pre-existing): `ComputeDCkW`'s `while (a≠ga AND
  b≠gb) OR (N>9)` loop is ported verbatim — a latent non-termination only if an
  `EffCurve` is non-monotonic enough to oscillate between segments (never for the
  gated cases; the ideal path early-returns, monotonic curves converge exact-float
  in 1–2 iters — matches the oracle, which would also hang); the `CalcYPrimMatrix`
  harmonic branch is dead until WP7.6; Monitor mode-7 (the Storage monitor) is
  header-only like PVSystem mode-3 (WP7.7, no gate uses it).
- **audit-tests follow-up:** closed two real gaps. (1) the daily golden only
  covered **discharge**→reserve→idle; added `phase7/storage_daily_charge` (a 20%
  battery charging at 80 kW fills to the 200 kWh rating in 4 h and flips to Idling)
  to oracle-pin the **charge** half of `UpdateStorage` (the fill branch + the
  full-clamp + state flip). (2) the `kva_clamp_backs_off_kw_on_pf` unit test pinned
  only the apparent power (a wrong-leg PF-priority regression `kw=20,kvar=15` also
  sits on the kVA circle → passed); strengthened to pin `kw_out=16.5359…`,
  `kvar_out=18.75` exactly (oracle-probed), and added a 4th battery `sd` (pf=0.8 at
  the kVA limit) to `storage_clamps` so the Q-priority back-off legs (kW=330.72,
  kvar=375) are oracle-pinned too. (3) re-probed the 2 *purely* stale
  `unsupported_class=Storage` cases (`GFM_IEEE8500/Run_8500Node_GFMDaily{,SmallerPV}`)
  → re-tagged to their real blocker. **Surfaced-not-fixed:** ~29 other Storage cases
  still carry a *superset* tag that includes the now-supported `Storage` alongside
  the real blocker (StorageController / BatchEdit / InvControl); they clear on the
  WP7.4-step-2 / WP7.5 re-probes (the bijection holds; `corpus_manifest` passes).
**Step 2 — the real `StorageController`.** Port of
`Controls/StorageController.pas` (2036 lines, the most complex control) replacing
the WP6.8 parse-only skeleton (`control/storage_controller/{mod,compute,accessors,
tests}`):
- `MakeFleetList` (real resolution: a named list → 14403 on a missing member, an
  empty list scans every enabled non-external Storage), the `SetFleet*` helpers
  (`ToCharge/ToDischarge/ToIdle/kWRate/ChargeRate/External/DesiredState`), the fleet
  kW/kWh aggregates, `GetControlPower`/`GetControlCurrent` (the `MonPhase`
  AVG/MAX/MIN/specific-phase logic, ×3 positive-sequence), and **`Sample`'s dispatch
  modes**: `DoLoadFollowMode` (Peakshave/Follow/Support/I-Peakshave — the
  weighted-share discharge + the cut-in/out + out-of-oomph + ResetLevel recovery),
  `DoTimeMode` (the trigger-time on + the RELEASE_INHIBIT delayed push),
  `DoScheduleMode` (up/flat/down ramp), `DoLoadShapeMode`, `DoPeakShaveModeLow`
  (the charge peakshave). `DoPendingAction` (RELEASE_INHIBIT) + `Reset` (idle the
  fleet).
- **Architecture (the GenDispatcher pattern):** the fleet is a *dynamic* set, so
  `Sample`/`Reset` reach the monitored element + the Storage fleet through a
  `StorageDispatchEnv` over the store (`dispatch.rs` clones the controller out,
  builds the env over the store + `Solution` fields + queue, runs, copies back).
  The fleet (`FleetPointerList`) + the `SetFleetToExternal`/`SetAllFleetValues` that
  Pascal runs in `RecalcElementData` resolve **lazily on the first `Sample`** (the
  architecture has no store access at parse-time `RecalcElementData`). Storage gained
  `pub(crate)` `set_kw`/`set_storage_state`/`present_kv` for the fleet.
- **TODO(compat):** the upstream `if not FleetState = STORE_IDLING` precedence bug
  (`not` binds tighter than `=`, so `(not FleetState) = 0`, firing only when
  FleetState = STORE_CHARGING) is reproduced verbatim in `DoLoadFollowMode` +
  `DoPeakShaveModeLow`. **NOT_PORTED:** the seasonal-rating dynamic target
  (`Get_DynamicTarget`, `DSS.SeasonalRating`/`SeasonSignal` — not in the engine;
  `CtrlTarget` always takes the non-seasonal branch), `MakePosSequence`, and the
  parse-time 37201 for a *Storage-less* circuit (the fleet resolves lazily at
  `Sample`, so a default empty fleet is a silent no-op like GenDispatcher; a
  *named-missing* element still errors 14403).
- **Gate:** golden `phase7/storagecontroller_daily` (a 2-battery PeakShave fleet
  holding a 6 MW load below a 4 MW target over a 4 h daily run — both deplete to the
  reserve and flip to Idling; the controller-driven **SOC trajectory** endpoint
  matched at 1e-6); 11 mock-env `sample_*` unit tests (the exact dispatch arithmetic:
  PeakShave discharge/in-band/weighted-split, Time-trigger, PeakShaveLow charge,
  out-of-oomph, named-missing 14403, reset, first-run external/values); 2 exec tests
  over the **real** control sweep — `storagecontroller_peakshave_dispatch` (an
  unreachable target → the fleet caps at `kWrated`, a unique converged point: exact
  `kW=2000`/`State=Discharging`) and `..._holds_target` (a reachable target → the
  monitored line power pulled into the band); and golden
  `phase7/storagecontroller_peakshave` (the same active-dispatch snapshot, the fleet
  *live* at 2000 kW each — node voltages + the fleet terminal powers/currents +
  Discharging state, all at 1e-6; see the YPrim-rebuild fix below). **Corpus: 0 net
  growth (stays 44)** — the `StorageControllerTechNote`/`StoCtrl_*` feeders embed
  `Export Eventlog`/`Export monitors` (Phase 8), set `SeasonalRating` (NOT_PORTED),
  or run an inline 8760-step yearly DemandInterval report (`corpus_live` compiles the
  master, so the inline run/Export executes). lib **557 → 564**.
- **audit-code follow-up:** verdict faithful 1:1; fixed one substantive + one
  cosmetic divergence. (1) **`GetControlPower` positive-sequence ×3** — Pascal's
  `MonitoredElement.Power[]` (`Get_Power`) already applies the posseq ×3, and
  `GetControlPower` applies it again (→ ×9 of a 1-phase / posseq-reduced monitored
  element); the port summed the conductors manually (no internal ×3) → ×3, a 3×
  dispatch divergence in posseq solves (ungated — no posseq StorageController
  golden/corpus). Routed the `NPhases=1` branch through `terminal_power` so the
  trailing ×3 double-applies, matching the oracle. (2) **named-missing 14403 emitted
  twice per Sample** — `ensure_fleet` (Sample top) + the dispatch modes' redundant
  `if FleetPointerList.Count = 0` both re-ran `MakeFleetList`; dropped the redundant
  in-mode rebuild (the `FleetSize <= 0` guard still holds) → one 14403 per Sample.
  Surfaced-not-fixed (upstream-faithful): the `if not FleetState = STORE_IDLING`
  precedence bug (already a `TODO(compat)`); the lazy `ensure_fleet` timing
  (plan-sanctioned, matches the oracle on the daily golden).
- **audit-tests follow-up:** closed the untested-modes gap. Added 8 mock-env
  `sample_*` tests for the dispatch paths the first pass missed — Support, Schedule
  (the up-ramp rate math), I-Peakshave (the amps→kW path), Time-charge opt-2 (+ the
  delayed RELEASE_INHIBIT push), `do_pending_action` (incl. the Follow-mode
  no-clear), the cut-in/out + inverter-off override, LoadShape discharge, and the
  ShowEventLog path (the plan's named event-log gate, asserted on the mock event
  sink since an oracle event-log-equal is Export-blocked). Strengthened
  `sample_named_missing_storage_errors_14403` to pin the single-emission fix
  (count == 1). lib **564 → 572**.
- **YPrim-rebuild fix (post-audit follow-up).** The active-dispatch snapshot golden
  had been dropped over a misdiagnosed "the fleet converges only to the solver
  tolerance (≈1.8e-6 residual)". Root cause was a real porting bug: when the
  controller dispatched a fleet member (idle→discharging), its Norton admittance
  `Yeq` changed (~0.013 S) but the **system Y was never rebuilt**, so the solve ran a
  stale *idle* YPrim against the *discharging* injection — an inconsistent Norton
  model that drifted the converged point ~1.8e-6 *and* cost an extra iteration. Pascal
  `set_YprimInvalid(TRUE)` raises `Solution.SystemYChanged` (CktElement.pas l.245), so
  `SetNominalDEROutput`'s state-change YPrim invalidation makes `CheckControls` rebuild
  Y before the next solve (Solution.pas l.1155) — and the solve loop rebuilds after
  `GetPCInjCurr` (l.895). The port set `cd.yprim_invalid` but never propagated it to
  `system_y_changed`. Restored that side effect at two points: the
  `StorageController` dispatch env (the `Sample` path → `CheckControls` rebuild) and
  `Storage::inj_currents` (the solve path, via a new `InjCtx.system_y_changed` — covers
  the idle/`SetFleetToIdle` and daily time-series transitions). Result: the snapshot now
  matches the oracle bit-for-bit (~1e-12, iterations 4=4); golden
  `phase7/storagecontroller_peakshave` restored. Scope is **Storage-specific**: Storage's
  YPrim is state-dependent (`YeqDischarge`) and `SetNominalDEROutput` invalidates it on a
  state change. PVSystem does **not** — PVsystem.pas `SetNominalDEROutput` (l.1146) /
  `ComputeInverterPower` (l.1351) never raise `YprimInvalid` on an inverter cut-in/out
  (its setters are all parse/MakeLike/harmonics); verified empirically too (forcing a
  rebuild on the toggle leaves `pvsystem_clamps` within 1e-6 — a cut-out drives injection
  *and* Yeq to ~0). InvControl (WP7.5) dispatches kvar/kW setpoints, not discrete state, so
  like GenDispatcher it won't invalidate YPrim. Only Storage uses the new
  `InjCtx.system_y_changed`; it stays generic plumbing but needs no PVSystem/InvControl wiring.
