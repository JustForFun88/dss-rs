# Phase 7 — WP7.3 (DER A: DynamicExp + InvBasedPCE + PVSystem) — archived per-step records

> **Archived from `STATUS.md`** (2026-06-27) to keep the live handoff lean. WP7.3
> (DER A) is **✅ COMPLETE** on the `phase-7-extended-elements` branch; these are the
> frozen per-step records (step 0 `DynamicExp`, step 1 `InvBasedPceData`, step 2
> `PVSystem` + steps 3–4 zone + gate), superseded only by the code and tests. The
> live `STATUS.md` §1e keeps a one-line-per-step summary. §3/§4/§5 cross-references
> resolve against `STATUS.md`. Plan: `PHASE7_PLAN.md` §WP7.3.

---

**WP7.3 (DER A: DynamicExp + InvBasedPCE + PVSystem) — ✅ COMPLETE.**
- **step 0 — `DynamicExp` (`general/dynamic_exp.rs`):** the user-defined
  differential-equation catalog object (`General/DynamicExp.pas`), a `DSS_OBJECT`
  registered before Generator/PVSystem/Storage (Pascal "before Generator,
  PVsystem, Storage"). Lands the object **and its expression interpreter**: setting
  `Expression` compiles the RPN diff-eq (`InterpretDiffEq`) into a flat `cmds`
  automation array (operator codes + variable/constant slots), evaluated each step
  by a stack machine (`SolveEq`) over a per-element `[value, derivative]` memory
  space — both ported loop-for-loop on the done `RPNCalculator`. Props: `NVariables`
  / `VarNames` (lowercased StringList) / `var` (active-var, drives `VarIdx`) /
  `VarIdx` (Pascal `SilentReadOnly`) / `Expression` (kept verbatim, cleared on a
  compile error) / `Domain` (`Time`/`dq`; parse default `dq`, field default `Time`).
  New enum `dynamic_exp_domain`; `MakeLike` is a no-op-with-error (Pascal 50099).
  Gate: `props/dynamicexp.json` (**9** oracle-pinned scenarios incl. the vendored
  Kundur expression, the bad-expr clear, the `var`/MakeLike/empty-`dt` error paths)
  + **13 interpreter unit tests** (cmds compilation + numeric `SolveEq` for the
  Kundur/π/trivial + operator-dispatch expressions + the
  `Get_*`/`IsInitVal`/`Check_If_CalcValue` accessors). The
  evaluator's *numeric* oracle pinning comes with the dynamics solve (WP7.7); here it
  is spec-pinned (Pascal is the spec) since the oracle exposes no `cmds`/`SolveEq`
  outside a dynamics run. *Self-contained: no solve-loop change.*
  *audit-code:* faithful interpreter/evaluator port; fixed a **panic** — an empty
  operand before `dt` (e.g. `expression=[ dt = b]`) hit `vars[0]` on an empty list
  and aborted the process, where Pascal raises a *catchable* `EStringListError`
  ("List index (0) out of bounds"), logs it, and leaves the expression as written.
  Now a recoverable error (+ a `props/dynamicexp.json` scenario + a unit test
  pinning it to the oracle). Surfaced-not-reproduced: on that error path Pascal's
  `SetLength(Cmds,+2)` leaves two zero `cmds` slots — unobservable (`cmds` on an
  errored expression is never evaluated; not a dumped property), so not reproduced.
  *audit-tests:* strong (exact `cmds` + numeric `SolveEq` + oracle props); closed
  the one gap — `SolveEq`'s dispatch was exercised for only 5/22 opcodes. Added an
  end-to-end operator-dispatch test (sqr/inv/ln/exp/`^`/swap) **and** a guard pinning
  the substring tie-break that makes `sqrt`/`atan2` dead opcodes (`sqr`/`atan` shadow
  them — verbatim Pascal quirk). lib **514 → 527**.
- **step 1 — `InvBasedPceData` (`pc/inv_based_pce.rs`):** the shared
  inverter-based PC-element base (`PCElements/InvBasedPCE.pas`, `TInvBasedPCE`) plus
  the `DynamicEq`/`DynOut` fields its parent `PCElements/DynEqPCE.pas` contributes.
  An **abstract base** — not New-able (`CreateDSSClasses` never registers it; not in
  `construct.rs`); PVSystem (step 2) and Storage (WP7.4) embed it the way Generator
  flattens `GenVars`, and dispatch the virtuals through the `InvBasedPce` trait.
  Lands: the `InvBasedPceData` data record; the `InvDynamicVars` **scalar**
  sub-record (`Shared/InvDynamics.pas` `TInvDynamicVars` — only the scalars, which
  back the PVSystem/Storage props `kVDC`/`kP`/`PITol`/`SafeVoltage`/`AmpLimit`/
  `AmpLimitGain`/`SafeMode`, so they must exist before those classes); the
  `InvBasedPce` virtual trait (`IsPVSystem`/`IsStorage`/`GetPFPriority`, base
  `False`); and the three power-flow shared methods —
  `StickCurrInTerminalArray` (wye/delta current routing, same sign convention as
  Generator), `Get_Presentkvar` (`Qnominalperphase·0.001·Fnphases`),
  `UsingCIMDynamics` (`VW|VV|WV|AVR|DRC`, WPMode deliberately excluded). **Deferred
  to WP7.7 (dynamics/GFM):** the `TInvDynamicVars` per-phase arrays + every method
  (`SolveDynamicStep`/`SolveModulation`/`CalcGFM*`/`InitDynArrays`/`Get_/Set_InvDyn*`),
  the `PICtrl` PI-controller array, `CheckAmpsLimit`, the GFM `GetCurrents`
  override, and the `DynEqPCE` dynamics memory (`DynamicEqVals`/`DynamicEqPair`/
  `UserDynInit`). Gate: **6 spec-pinned unit tests** (base `Create` defaults,
  `Get_Presentkvar` scaling, `UsingCIMDynamics` WPMode exclusion, wye/delta
  `StickCurr` routing, the trait default/override via a mock implementor) — spec-
  pinned (Pascal is the spec) since the oracle exposes none of these helpers outside
  a full PVSystem/Storage solve (numeric pinning arrives with PVSystem, step 2).
  *Self-contained: no solve-loop change, no class registration.* *audit-code:*
  faithful + complete (all 55 `TInvBasedPCE` fields incl. the full 18-scalar
  `TInvDynamicVars` set; `StickCurr` 1-based→0-based routing verified vs the
  oracle-validated `Generator::stick_curr`; deferrals all dynamics/GFM →
  plan-sanctioned WP7.7) — **no fix needed**. *audit-tests:* strong (exact values,
  the WPMode edge, the delta wrap); closed one gap — the `StickCurr` tests seeded a
  zero array, so they could not distinguish accumulate (`+=`/`-=`) from overwrite
  (`=`). Added `stick_curr_wye_accumulates_neutral` (non-zero seed + two phase
  currents stacked onto the wye neutral — the `+=`→`=` regression guard). lib
  **527 → 534**.
- **step 2 — `PVSystem` (`pc/pvsystem/`) + steps 3–4 (zone + gate):** the
  photovoltaic PC element (`PCElements/PVsystem.pas`, `TPVsystemObj`) on the
  Generator injection template (WP6.2) with the step-1 `InvBasedPceData` base
  embedded (flattened like `GenVars`); a directory module mirroring
  `generator/` (`mod`/`nominal`/`solve`/`registers`/`accessors`/`tests`). Lands the
  **power-flow** PVSystem: the PV-panel + inverter model `ComputePanelPower`
  (irradiance·shape·`Pmpp`·temp-derate) → `ComputeInverterPower` (cut-in/cut-out,
  efficiency curve, watt/var priority, the kvar + `kVA` clamps — ported
  loop-for-loop, with a Pascal-faithful `Sign` that returns 0 at zero) →
  `kWOut_Calc`; `SetNominalDEROutput` (= `SetNominalPVSystem`: shape/temperature
  by solve mode → per-phase P/Q → `YEQ`/`YEQ_Min`/`YEQ_Max`/`PhaseCurrentLimit`);
  `CalcYPrim`/`CalcYPrimMatrix`; `DoConstantPQPVsystemObj` (model 1, with the
  current-limited + impedance-outside-band branches) / `DoConstantZPVsystemObj`
  (model 2) + the `ForceBalanced` pos-seq path; energy-meter registers +
  `TakeSample`. Curves/shapes resolve via the snapshot-clone ObjectRef pattern:
  irradiance `daily`/`yearly`/`duty` (LoadShape), `Tdaily`/`Tyearly`/`Tduty`
  (TShape), `EffCurve`/`P-TCurve` (XYcurve), and **`DynamicEq` as a real
  `DynamicExp` ref** (step 0). Two new enums: `pvsystem_model`
  (ConstantP_PF/ConstantY/UserModel) and `inv_control_mode` (GFL/GFM). **Step 3:**
  `ElemKind::PVSystem` + a `pv_systems` circuit list; registered in `construct.rs`
  after the protection controls (Pascal `PVSYSTEM_ELEMENT`); `is_zone_pce` admits
  PVSystem (`EnergyMeter.pas:1911` — the zone walk still ignores it for
  accumulation, matching Pascal "ignore other PC elements"). Also fixed a **real
  bug** surfaced by the corpus probe: the Monitor mode-3 metered-kind detection
  classified only Load/Generator as `PcElement`, so a `mode=3` monitor on a
  PVSystem (Pascal: any `TPCElement`) errored "must be a power conversion element"
  and left a singular Y — `Test/PVSystemTest.dss`; PVSystem now classifies as
  `PcElement` (the mode-3 channel stays the Phase-6 empty stub, like Generator).
  **Deferred (WP7.6/7.7, matching Generator):** the GFM mode
  (`DoGFM_Mode`/`CalcGFMYprim`/`CheckOLInverter`), harmonics
  (`DoHarmonicMode`/`InitHarmonics`), dynamics
  (`DoDynamicMode`/`InitStateVars`/`IntegrateStates` + the state-variable
  interface `NumVariables`/`Get_/Set_Variable`/`VariableName`), the user-written
  DLL model (model 3 → error 567, never ported), and `MakePosSequence`.
  Gate: **`props/pvsystem.json`** (10 oracle-pinned scenarios — default, PF, kvar/
  delta, cut-in/out + model 2, eff/P-T curves, all six shapes, the kvar/Pmin
  limits, the inverter params, a `DynamicEq` ref, MakeLike — all round-trip
  exactly); two targeted goldens **`phase7/pvsystem_{snapshot,curves}`** (voltages
  + element powers 1e-6, the second exercising the eff curve + P-T derate + the
  `kVA` clamp at pf=0.95); **corpus migration 37 → 44** (7 PVSystem cases:
  EPRI `Master_withPV`, the 2 `CurrentkvarLimite` kvar/kvarNEG, the 4 ConstantPF
  `SnapShot_PFP_*`), with the InvControl/Export/Plot cases re-tagged accurately
  (→ `unsupported_class=InvControl` / `unsupported_command=Export`) and **2**
  `varCapability` cases held in `needs_investigation` (live ~4e-6 mismatch:
  near-ideal-source `Z=1e-8` amplifies a sub-1e-6 eff-curve interpolation
  difference — the kvar-clamp path is validated by the sibling
  `CurrentkvarLimite` cases that pass at full tolerance; conditioning, not a
  logic bug — diagnosed in the manifest note). lib **534 → 539**.
  *audit-code:* faithful port (the intricate `ComputeInverterPower` clamp cascade,
  `SetNominalDEROutput`, `CalcYPrimMatrix`, `DoConstantPQ`/`Z`, `MakeLike`, the
  side-effects all match Pascal + the oracle goldens). Fixed one real
  silent-degradation: **`ControlMode=GFM`** is a settable property (round-trips)
  but its solve behavior (`DoGFM_Mode`/`CalcGFMYprim`) is WP7.7 — the model dispatch
  ignored `gfm_mode` and silently ran the regular PQ model (plausible-but-wrong
  numbers). Now a **pre-solve guard** (`solution/dispatch.rs`) aborts the solve with
  an explicit "not ported (WP7.7)" error + `solution_abort` (the
  deferral-is-never-a-silent-fallback convention), plus a defensive
  contribution-path guard mirroring the user-model path; 2 exec tests pin it (the
  clean snapshot solve + the GFM error). Surfaced-not-fixed (recorded, both **exact
  Generator parity**, not new): the GENERALTIME arm ignores `ActiveLoadShapeClass`
  (`SysCtx` carries no class — shared with `generator/nominal.rs`); and
  `Set_ConductorClosed` is not wired (`pv_system_obj_switch_open` never set, like
  Generator's `gen_switch_open`). lib **539 → 541**.
  *audit-tests:* the property + solve goldens are genuinely oracle-pinned (10 props
  scenarios round-trip exactly; `pvsystem_{snapshot,curves}` pin V + element powers
  at 1e-6) — but the plan's "inverter control discrete state exact" was guarded by a
  unit assertion that **could not fail** (`kva_clamp_backs_off_kw` checked only an
  upper bound — kw=0/kvar=0 passed it) and the Monitor mode-3 fix had **no fast
  regression test**. Closed both: added golden **`phase7/pvsystem_clamps`** (3
  PVSystems oracle-pinning the three discrete states at 1e-6 — non-priority kVA
  back-off `kw=300,kvar=400`; cut-out `kw=0`; absorption-clamp + back-off
  `kvar=-300,kw=400`), strengthened the unit test to pin `kw=300,kvar=400` exactly +
  added `kvar_absorption_clamp_then_backoff`, and added an exec regression test
  (`pvsystem_accepts_mode3_monitor`) for the mode-3 metered-kind fix. Corrected an
  **overclaiming** deferral note: the passing `CurrentkvarLimite/kvarNEG` sibling
  uses `kvar=+500` (generation), so it does **not** cover the absorption direction —
  now independently pinned by `pvsystem_clamps` (pvc) + the unit test. Surfaced-not-
  fixed (pre-existing, WP7.5): 11 combined-class cases still carry a stale
  `unsupported_class=InvControl,PVSystem` tag (they need InvControl; a re-probe lands
  with WP7.5). lib **541 → 543**.
- **next:** **WP7.3 (DER A) COMPLETE** → **WP7.4 (DER B): `pc/storage.rs`
  (`TStorageObj` on the inverter base — the charge/idle/discharge state machine +
  integrated `%stored`) + the real `StorageController` fleet/dispatch.**
