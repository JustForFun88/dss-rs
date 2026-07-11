# dss_capi delta inventory: tag `0.14.5` → branch `0.15.x` HEAD (`e936d210`, ≈0.15.0b4)

Scoping input for the dss-rs upgrade-porting plan. dss-rs = 1:1 port of dss_capi **0.14.5** (= OpenDSS SVN **r3723**).
dss_capi 0.15.x tracks OpenDSS up to ≈ **r4119** (selectively; base claim in changelog: r4088 era, with cherry-picks to r4119).

- Repo: `E:/RustProject/dss-rs/.inputs/dss_capi_with_git`, 249 commits touch `src/`, `git diff --stat`: **371 files, +85,511 / −15,625**.
- ~60–70% of the src/ line churn is bucket F/G (property-system refactor, syntax normalization `Foo;`→`Foo()`, `ptruint`→`PtrInt`, capitalization, CAPI/Alt/Obj API growth) and new non-engine subprojects (Oddie ≈17k lines C, COM bridge ≈20k lines, loader).
- Sources: `docs/changelog.md` §0.15.0, `docs/known_differences.md` diff, per-commit inspection of the key units (cited below).

**Upstream features explicitly NOT yet ported into 0.15.x** (changelog): conditional `BatchEdit` (r4106), `pyControl` component (commands ARE ported), the big r4079→r4116 `Recloser`/`Relay`/`SwtControl` overhaul (breaking, deferred), Fuse `CurveMultiplier`/`InterruptingRating` r4102/r4103 (**ported then reverted**, commit `8d095a95`). These are *not* in scope of a 0.15.x-parity port.

## Summary table

| Bucket | Item count | Est. porting weight for dss-rs |
|---|---|---|
| A. New elements / solve modes | 3 active (+2 disabled, +1 ifdef'd) | **XL** (NCIM XL, WindGen XL, PCE force-flags M) |
| B. Algorithm/numerics in existing code | 6 | **M** (each S, but golden-sensitive) |
| C. New/changed properties, commands, options, parser | ~20 | **L** (line-constants property cluster is L by itself) |
| D. Upstream bug fixes (changed results, same input) | ~16 | **M** (mostly S each; InvControl cluster M) |
| E. Export/report/format changes | ~7 | **M** |
| F. API/plumbing only, no engine effect | large | excluded |
| G. Infrastructure (Oddie, COM bridge, loader, build) | large | excluded |

Overall: an upgrade to 0.15.x-parity is dominated by **NCIM**, **WindGen**, and the **line/cable-constants property cluster**; the rest is a long tail of S-size behavioral fixes, most of which are golden-observable.

---

## A. New element classes / new solve modes

### A1. NCIM solution algorithm (Newton Current-Injection Method) — **XL**
- Units: `src/Common/NCIMSolutionHelper.pas` (**new, 1048 lines**), hooks in `Solution.pas` (+~250 lines of NCIM state: `NCIM_Jacobian`, `NCIM_NodeType` PQ/PV, `NCIM_deltaF/deltaZ`, Q-limits...), `Generator.pas` (PV-bus participation: `NCIM_Idx`, `deltaQNom`, `NCIM_InitPVBusJac`), `VSource.pas` (`CalcInjCurrAtBus` refactor), `Load.pas`, `ExecOptions.pas`.
- Ported from EPRI OpenDSS (v10 era, incl. r3755 fix). Third `Algorithm` value: `NORMALSOLVE=0, NEWTONSOLVE=1, NCIMSOLVE=2` (`set algorithm=NCIM`). Full Newton power-flow with PV/PQ node classification, generator Q-limit handling (`set IgnoreGenQLimits`, `set NCIMQGain`), its own convergence test (`NCIM_Converged`), flat-start logic, and a **real-valued sparse Jacobian** solved via KLUSolveX extensions (`SetMatrixElement`, real matrix support).
- Adds surface (new mode + 2 options) and results (a new solver). For dss-rs this also implies extending `dss-sparse` with a real-valued KLU-shaped path. Weight: **XL**.

### A2. WindGen element + WTG3 dynamics model — **XL**
- Units: `src/PCElements/WindGen.pas` (**2272**), `WTG3_Model.pas` (**1335**), `WindGenVars.pas`, `WindGenUserModel.pas` (user-model support subsequently removed), `CAPI_WindGens.pas` (997, bucket F).
- New PCE (registered, **enabled**): wind generator with power-flow models (model=1 const-PQ fixed to use YPrim currents, commit `082900eb`) and a WTG type-3 dynamics model. dss_capi cleaned it up vs upstream: removed user-model code, removed unused `Xd/puXd`, PV-mode vars, `SetkWkvar` side-effects; harmonics mode **disabled** for now; uses user-provided ZThev in `InitStateVars` (`f56b1729`); compat-flag handling added. Zero-to-tiny (`1e-8`) kVA/kW guards behind `PermissiveProperties`.
- Adds surface + results. Weight: **XL** (a full new element with dynamics).

### A3. PCE current/YPrim forcing ("pyControl" plumbing) — **M**
- Units: `ExecOptions.pas`, `Common/CktElement.pas`/PCElement, Alt API.
- New set/get options `InjCurrent`, `ITerminal`, `YPrim`, `IterNumber`, `CtrlIterNumber`, `IntegrationFlag` + element flags `Flg.ForceInjCurrents`, `Flg.ForceYPrim`: user can override a PC element's injection currents / terminal currents / YPrim from the DSS language, and the solution loop honors the forced values. Ported from EPRI SVN (pyControl infrastructure); the `pyControl` component itself is NOT ported. New `SampleControlDevices` event hook in the control sampling loop.
- Adds surface; changes results only when used. Weight: **M**.

### A4. Generic5OrderMach + FMonitor — ported but **disabled by default** → skip
- Units: `PCElements/Generic5OrderMach.pas` (**2565**), `Generic5Helper.pas`, `Meters/fMonitor.pas` (**3205**).
- Refactored/cleaned from upstream, registered as classes, but constructing one errors out: "Generic5/FMonitor is currently disabled. It has not been fully validated." Not engine-reachable → **out of porting scope** (S note: reserve class-index constants `GENERIC5ORDERMACH_ELEMENT=39*8`, `WINDGEN_ELEMENT=43*8`).

### A5. A-Diakoptics — partially re-enabled behind `DSS_CAPI_ADIAKOPTICS` ifdef, commands disabled → skip
- Commits `d3229bc7`, `f8bb8161` (port of r3901 fix), `ede026e6`. Not built by default; commands not exposed. **Out of scope.**

---

## B. Algorithm/numerics changes in existing elements / solution loop

### B1. Capacitor Cmatrix YPrim: diagonal ×1.000001 before inversion — S, **changes numerics**
- `PDElements/Capacitor.pas`: in the `SpecType=3` (Cmatrix) branch of YPrim construction, each diagonal of the work matrix is multiplied by `1.000001` before `Invert()` ("same trick" as other spec types) so singular C matrices invert. This perturbs **every** Cmatrix capacitor's YPrim by ~1e-6 relative — golden-visible.

### B2. Simple-Carson earth-return constant: `658.5` → `658.8530451057239` — S, **changes numerics**
- `General/LineConstants.pas`, `GetZearth` `SIMPLECARSON` branch (part of the r3913-era line/conductor port). Changes Z for any line using `EarthModel=Carson-approx`. (Default earth model DERI unaffected.)

### B3. Line constants: εr of surrounding medium + height offset + equivalent-spacing model — M/L, **changes numerics when used**
- `LineConstants.pas` (+462), `CableConstants.pas` (+461; `CNLineConstants.pas`/`TSLineConstants.pas` merged in), `LineSpacing.pas`, `LineGeometry.pas` (+1013), `Line.pas` (+978).
- Shunt-capacitance `Pfactor` now divides by `(e0 * epsRMedium)` (default `epsRMedium=1.0` → **no change unless set**; comment notes `0.9993366876323544` matches Synergi). New "detailed vs equivalent" spacing path: `EqDistPhPh`/`EqDistPhN`/`AvgPhaseHeight`/`AvgNeutralHeight` replace per-conductor coordinates in the D_ij/height terms when specified; `HeightOffset`/`HeightUnit` shift conductor heights. CN cable capacitance: with `SemiconLayer=false` (new CNData flag, default **true** = old behavior) uses the Kersting/Synergi denominator `ln(RadCN/RadIn) − (1/k)·ln(k·RadStrand/RadCN)` instead of `ln(RadOut/RadIn)`.
- Defaults preserve 0.14.5 numerics; new properties open new numeric paths. See C1 for the property surface.

### B4. Harmonics: abort solution if a component fails to initialize — S
- `Common/SolutionAlgs.pas` (`6ad39597`): `InitializeForHarmonics` now returns Boolean; on failure the solution aborts (and `Redirect_Abort` inside redirects) instead of proceeding with a half-initialized harmonics state. Error-path behavior change.

### B5. GFM (grid-forming) YPrim: Isc1 factor-1000 fix — S, **changes numerics** (GFM dynamics)
- `Shared/InvDynamics.pas` (`de6a5a42`, port of SVN r3865): `Isc1 := (mKVARating * 1000.0 / (sqrt(3)*RatedkVLL))/NPhases` → `(mKVARating / ...)` — "preventing oversizing the model". Affects PVSystem/Storage GFM mode YPrim. (Note: GFM is a tracked-open gap in dss-rs anyway.)
- Also in dynamics: IBR current limit base changed, see D7.

### B6. YPrim-invalid reset semantics — **net no-change at HEAD** (note only)
- `aa991f24` removed the conditional flag when resetting `YPrimInvalid` (matching official r3859), a `DontResetYPrimInvalid` compat flag was added (`d74d62f5`), then **partially reverted** (`0b9afe75`) because EPRI reverted upstream. At HEAD: old `TSolverOptions.AlwaysResetYPrimInvalid` behavior is back; `DSSCompatFlags_DontResetYPrimInvalid` is documented "RESERVED... does not do anything yet". **Nothing to port**; watch for the flag flipping in later releases (convergence-pattern impact documented in changelog).

---

## C. New/changed properties, commands, options, parser behavior

### C1. Line/conductor property cluster (port up to SVN r3913 + r3902) — **L**
- `Line.pas`: new props `EpsRMedium` (31), `HeightOffset` (32), `HeightUnit` (33), `Conductors` (34). `Conductors` is the new first-class name for `Wires`/`CNCables`/`TSCables` (which become redundant aliases with `AllowNoneItem`); replaces the removed `SetterFlags_AllowAllConductors` special-case. Mixed-type conductor lists and `none` entries allowed (parser gains `AllowNoneItem` handling). Fix: re-specifying wires after spacing resets conductor state correctly (`43972098`).
- `LineSpacing.pas`: new props `Detailed` (bool), `EqDistPhPh`, `EqDistPhN`, `AvgPhaseHeight`, `AvgNeutralHeight`.
- `LineGeometry.pas`: new `Conductors` property (+ `none` handling, null-safety, ratings-after-spacing tracking `gotRatingsAfterSpacingConds`).
- `CNData.pas`: new bool prop `SemiconLayer` (default true = legacy formula; see B3).
- `LineCode.pas`: `FaultRate`, `PctPerm`, `Repair` marked unused/deprecated (`b0bc32bc`).
- `18952d20`: `LineType` enum fixed (allowed abbrev/length 4→5 — one more line type recognized correctly).
- Adds surface; numerics only via B3 paths.

### C2. `PermissiveProperties` compat flag + stricter default validation — **M**, deliberate divergence (default = stricter than EPRI)
- Property system (`General/DSSObjectHelper.pas`, per-element flags): by default the engine now **errors** where 0.14.5/EPRI silently accepted: (a) setting read-only props (e.g. locked SwtControl state); (b) array size mismatch vs `NPts`/`NConds` (EPRI copies what fits); (c) zero in essential props (`Generator.kVA/MVA/kW`, `Load.kVA/kW`, `PVSystem.kVA`, `Storage.kVA/kW`, `WindGen.kVA/MVA/kW`) — with the flag set, zero→`1e-8` like OpenDSS v10.2.0.1 (new `TPropertyFlag.NonZero`/`TrapZero`/`ReplaceZero` machinery); (d) Transformer/AutoTrans zero-replaced-by-default props now error. `Load.kW=0` check is conditional on property tracking (`fd021f18`, kW-only vs kW+kvar spec-sets).
- Parser/`ParserDel.pas` (+480): error paths return defined zero values, better error messages for missing sizing props.

### C3. Compat-flag DSS commands — S
- `Executive/ExecCommands.pas`/`ExecHelper.pas` (`eadf2547`): new commands `PushCompatFlags`, `PopCompatFlags`, `SetCompatFlag`, `UnsetCompatFlag`, `ClearCompatFlags` + magic comment prefix `//!AltDSS` (line executed as command by AltDSS, comment for EPRI). AltDSS-only surface.

### C4. `Solve all` / `Clear all` arguments (SVN r3903) — S
- `ExecHelper.pas` (`fc836e41`): `Solve`/`Clear` accept `all` param, aliasing `SolveAll`/`ClearAll`.

### C5. RegControl `FwdThreshold` (SVN r4086) — S
- `Controls/RegControl.pas` (`8a898cba`): new property `FwdThreshold` (kW); `RevThreshold` semantics reworked into signed `RevPowerThreshold`/`FwdPowerThreshold` pair with a compat rule (setting only RevThreshold sets Fwd=abs(Rev), Rev=−Fwd). Reverse-power detection logic now uses both thresholds → control behavior addition.

### C6. Transformer/AutoTrans `BHpoints`/`BHcurrent`/`BHflux` (SVN r4064) — S
- `PDElements/Transformer.pas`, `AutoTrans.pas` (`90962ae8`): data-holding properties for GICharm; no solve effect.

### C7. TCC_Curve named "none" forbidden + `AllowNone` on curve refs (SVN r4119) — S
- `General/TCC_Curve.pas`, Relay/Recloser/Fuse curve properties (`fd034bb0`): creating a TCC_Curve literally named `none` errors; `PhaseFast/PhaseDelayed/GroundFast/GroundDelayed` accept `none` to clear.

### C8. InvControl surface — S
- `VV_RefReactivePower` property **fully removed** (`c44e5873`; was deprecated since 2020). Two new validation errors for `MonBus` (missing nodes / invalid bus, `e6607efd`).

### C9. New settings/options (AltDSS-specific, mostly API but reachable from scripts) — S each
- `set OpenDSSViewer=` (renamed from `DSSVisualizationTool`, `96e97097`); `AllowForms`, `AllowProgressBar` options ported as no-op-ish compatibility (`7eb89c8f`); `DSSSettings_PreserveCase` bitflag setting (affects stored/reported name casing, see E5); `SkipFileRegExp`/`SkipCommands` (skip matching redirects/commands, also inside ZIP; reset on `Clear`); `ctx_ShareGeneral` (share LoadShapes/Spectrum/GrowthShape/XfmrCode across DSSContexts); Bus `Latitude`/`Longitude` as synonyms of Y/X (`51ff779e`); UPFC legacy property-name fixes (`5ae50298`); `set SeasonalRating` path now syncs a global `SeasonalRatingIdx` (see E2).
- Locale: engine pins "." decimal separator at library init (`87a882d4`) — n/a for Rust (already invariant).

### C10. `LegacySMARTDS` compat flag — S (opt-in parser workarounds for the SMART-DS dataset; off by default).

### C11. DSSClassDefs r3875: `New`/class commands always activate the selected class — S (`7457fc0b`).

---

## D. Upstream bug fixes (changed results for same input, default-on)

| # | Unit / commit | Fix | Numeric impact |
|---|---|---|---|
| D1 | `InvControl.pas` — **InvControlDeltaV** (dss-ext original fix) | Voltage-change across control iterations was tracked via a shared 3-slot `FVpuSolution` rolling buffer; rewritten as per-control 2-slot with correct indexing (`FVpuSolutionIdx` init −1). Affects volt-var with multiple DERs per InvControl. **Deliberate divergence from EPRI** (flag `InvControlDeltaV=0x100` restores old/EPRI behavior). | control convergence / final Q |
| D2 | `InvControl.pas` (changelog) | Correct per-DER base voltage when multiple InvControls/DERs present | volt-var setpoints |
| D3 | `InvControl.pas` `f10ca21f` (r4056) | volt-var `Q_Ppriority`: guard `SQR(kVA)−SQR(kW)` against tiny negative before `Sqrt` (was NaN/precision issue in 64-bit) | edge cases |
| D4 | `InvControl.pas` `7d802ff0` (r3822) | monitored voltages for **delta-connected** controlled DERs now computed as LL (`V[j]−V[next]`) | delta DER volt-var |
| D5 | `InvControl9611` flag re-confirmed: the 9.6.1.1 behavior **was an upstream regression**, fixed in OpenDSS v10; dss_capi default already had the fix — flag retained temporarily | — |
| D6 | `Transformer.pas` `4ed59416` (r4033) | seasonal-rating AmpRatings: remove spurious `1.1 *` factor (`AmpRatings[i] := kVARatings[i]/Fnphases/Vfactor`) | seasonal ratings |
| D7 | `PVsystem.pas` `32db066f` (r3868), `Storage.pas` `e97cbc8c` (r3582) | dynamics current limit base: `IMaxPPhase = kVArating/BasekV/NumPhases` (was `PanelkW`/`kW_out`) — "IBR operational range"; plus safe-voltage=0 init fix | IBR dynamics |
| D8 | `Transformer.pas` `69fca934` | X23/X13 "trap zero" wasn't triggering (zero → default replacement broken); `XSCArray` now requires non-zeros | 3-winding Z |
| D9 | `EnergyMeter.pas` `fb728364` (r4115) | `AllocateLoad` ignores **disabled** meters and sensors | load allocation |
| D10 | `StorageController.pas` `1b3123ce`+`a14c3f1f` (r4058) | peakshave/peakshavelow: force new power flow on first control iteration when fleet set to (dis)charge; `%kWBandLow`/`kWBandLow` kept in sync | storage dispatch |
| D11 | `CapControl.pas` `b9bc87b8` | `PTPhase`/`CTPhase` > Nphases: old code silently clamped to 1 at edit time; new handles the check properly (moved/reworked, incl. PF control) | monitored phase |
| D12 | `SwtControl.pas` `bb9c9785`,`1f0ebb7b` | `Normal`/`State` properties were both mapped onto `CurrentAction`; now correctly map to `NormalState`/`PresentState`; no-element misuse → state `none` | switch state |
| D13 | `LoadShape.pas` `c4590d16` | memory-mapped-file init had inverted condition (missing `not`) + disposal leak | MM loadshapes |
| D14 | `DynamicExp` `2a8bdb78` | RPN evaluator index bug fixed (+reuse) | DynamicEq elements |
| D15 | `PCElement` `4366b126` | `LookupVariable` ignored lowercase variable names | state-var access |
| D16 | Meters/zone lists (`690e02f9` etc.) | zone-list counter ignores disabled devices *and non-PD elements*; overload-report file handling fixed | meter zones |

Also D-adjacent, API-level (engine state via classic API): `Generators_Set_kvar` now calls `RecalcElementData` (r3746); Reactors/Loads/Vsources classic-API side-effects aligned (behind `SkipSideEffects` compat flag); `Capacitors_Set_State` typo fix.

---

## E. Export/report/format changes

- **E1. Monitor CSV/header** (`Meters/Monitor.pas`, `a6d3aa2c`, `6b54aba5`): quotes omitted from exported header; strict comma delimiter; new **`MonitorHeader`** compat flag (0x80) restores EPRI's extra spaces + trailing comma. (dss-rs already ported the 0.14.5 no-extra-spaces variant; delta = quote removal + flag.)
- **E2. SeasonalRating reimplementation** (`EnergyMeter.pas` +1087, `ExportResults.pas`, `55400a29`): per-solve `DSS.SeasonalRatingIdx` synced globally (`SyncSeasonalRatingIdx` on `set hour`/season changes) instead of each report re-reading the XYCurve; overload report/exports now apply the seasonal index to **any PDElement with NumAmpRatings>1** (old code: `ClassName='line'` only) with proper range check. Changes `DI_Overloads` / `Export Overloads` results under seasonal ratings + r4115 (D9) + `WriteOverloadReport` skips disabled.
- **E3. JSON exports**: new experimental `DSSJSONOptions_State` (circuit-element + bus state, registers) and `DSSJSONOptions_Reliability` flags; JSON property-tracking off-by-one fix; schema/units metadata. (dss-rs: only if/when JSON export surface exists.)
- **E4. ExportCIMXML.pas** (+744, `5dd24211`, `bb44d12b` r3902): CIM XML updates incl. `None`-conductor handling for spacing/conductor-array lines. (Out of dss-rs scope unless CIM export is ported.)
- **E5. PreserveCase / DSSUpperCase**: all `AnsiUpperCase` in `ShowResults.pas`/`ExportResults.pas` replaced by `DSSUpperCase` function pointer — default `AnsiUpperCase` (unchanged output), becomes no-op when `DSSSettings_PreserveCase` is set. Show/export text otherwise refactor-only.
- **E6. Transformer `SaveWrite` fix** (`97b98654`): PDE properties emitted correctly on `save circuit`; Line ratings property-tracking tweak (`01ceeb8e`).
- **E7. Monitors API `Header`** matches E1; `COMErrorResults` default flipped to false (API arrays, no engine effect — F-adjacent).

---

## F. API-only / plumbing (no engine-observable effect) — EXCLUDED from porting scope

Brief list: property-system internals (`DSSObjectHelper.pas` +2547 — setter-flags plumbed through `InterpretIntArray/InterpretDblArray`, Get/Set-ArrayElement placeholders, `ImplicitSizes` for shape files); whole-codebase syntax refactor (parentheses, Pascal-property removal → getter/setter functions, `ptruint`→`PtrInt`, identifier capitalization, `CAPI_Types` merge); classic→Alt API bridges, `Batch` extensions, `Broadcast` flag, `PDElements/ActiveClass idx`, `Bus_Get_idx`, `Bus_Get_Lines/PDElements` typo fix, `Alt_PCE` additions, `LoadShape MultAtHour`, `Storages`/`WindGens` classic APIs, `Loads_Get_Sensor` returns sensor name (r3835-equivalent), `Text_Set_Command` multi-command handling, GR-string-API drop, `DSS_Set_EnableArrayDimensions` deprecation (dims default on), `COMErrorResults` default false, `CktElement BusNames` optional node-stripping, YMatrix low-level exposure + `SaveAsMarketFiles`, `ctx_ShareGeneral` internals, event-API generalization, many null-pointer/validation guards, perf work (element-lookup regression fix `06a9ea23`, cached total losses `149ad015`, `LastValueAccessed` removal in shapes, faster YNodeVarray, `AllPDEatBus`), memory-leak fixes, macOS/ARM FPU resets, packed-record fixes.

## G. Infrastructure — EXCLUDED

Oddie (EPRI-binary bridge, `src/altdss_oddie/` ≈17k lines C — dss-rs already consumes this via `tools/opendss/`), DSS-Extensions COM bridge DLL (`src/COM/` ≈20k lines), AltDSS C-API loader (`src/altdss_capi_loader/`), FastDSS common code, CMake/build/cfg, library rename `dss_capi`→`altdss`, header reorganization, i18n/message wording ("official OpenDSS"→"EPRI's OpenDSS").

---

## Deliberate divergences from EPRI OpenDSS introduced/retained in 0.15.x (per `known_differences.md` @HEAD)

1. **InvControlDeltaV**: fixed by default; EPRI (current + past ~9 years) has the bug → compat flag to restore.
2. **PermissiveProperties**: strict validation errors by default (EPRI silently ignores / auto-replaces; OpenDSS v10.2.0.1 replaces zero with 1e-8 — that behavior is behind the flag, not default).
3. **Monitor header**: no extra spaces/trailing comma (since 0.12); now flag-restorable (`MonitorHeader`).
4. **InvControl9611**: confirmed upstream regression; dss_capi default = fixed (matches OpenDSS v10, diverges from r3723).
5. **SeasonalRating**: reimplemented tracking (index synced globally, applies beyond Line class).
6. Property tracking / `save circuit` correctness, `COMErrorResults=false`, `PreserveCase` — surface-level.
7. `DSS_ATAN2` rename only (custom atan2 semantics unchanged — relevant to dss-rs `DssComplex64` work: no numeric change).

## Porting-relevance notes for dss-rs (0.14.5-parity baseline)

- The two XL items (NCIM, WindGen) are self-contained additions; everything else is a long tail of S/M deltas concentrated in InvControl, EnergyMeter/seasonal, Transformer, dynamics current limits, and the line-constants property cluster.
- Golden-sensitive default-behavior changes (would move existing 0.14.5 goldens if adopted): B1 (Capacitor Cmatrix ×1.000001), B2 (658.85 Carson constant), D1–D16, E2. Adopting any of these means regenerating goldens against a 0.15.x oracle — i.e., these belong to a coordinated oracle-bump, not piecemeal fixes.
- Opt-in/off-by-default items (no golden movement): B3/C1 new line-constant paths (defaults preserve old numerics), C2 PermissiveProperties (note: **default is the new strict behavior** — parser-error surface changes even without the flag!), C10, A3 (only when used).
- Explicitly skippable: A4 (disabled), A5 (ifdef'd off), F, G, and the upstream features 0.15.x itself hasn't ported (Recloser/Relay/SwtControl r4079–r4116 overhaul, pyControl component, conditional BatchEdit, Fuse CurveMultiplier).

---

## WP-U0.2 sweep reconciliation (2026-07-11)

Empirical check of this inventory against the `capi ↔ capi015` sweep (378
cases: 334 match, 17 diverged, 27 error). Full analysis:
`docs/upgrade/sweeps/capi_vs_capi015.md`. Legend: **[witness]** = a corpus deck
observably moves; **[no corpus witness — synthesize (WP-U…)]** = real per source
but no corpus deck exercises it (needs a synthesized deck at its WP).

**Confirmed by a corpus witness:**
- **B1** Capacitor Cmatrix ×1.000001 — **[witness]** `Local/Mon_voltage_*-2`,
  `YYD-Master-step1` (Y-fingerprint ~1.3–1.8e-6, solved V unchanged).
- **B2 / D1** SimpleCarson `658.85` (and/or **B3** CN-cable) — **[witness]**
  `Test/Cable_constants.DSS` (Yf 2.6e-6).
- **B5** GFM `Isc1` ×1000 removal — **[witness]** `gfm_micro/gfm_invcontrol/
  gfm_dynamics/pv_gfm_dynamics` (Yf 3.6e-3, op-point stable).
- **C2** PermissiveProperties strict default + **B7** parser strict — **[witness,
  dominant]** 16 decks flip solve→parse-error (kW-zero `#2025111`,
  read-only `#2024101`, CSV/array `#2024110`/`#20241024`/`#20241011`, spectrum
  `#65001` — 16 decks). Confirms C2's "default is the new strict behavior."
  Ledger L2. (Separately: 2 `#58614` crash decks + 8 `#303`/timeout intrinsic.)
- **C5 / B4** RegControl reverse/idle rework — **[witness]** `midi_controls`
  (iter 68→78, reg-tap + xfmr discrete state differ).
- **D1–D4** InvControl cluster — **[witness]** `midi_invcontrol` (iter 66→106,
  V 3.4e-2).
- **D10** StorageController + **E2** SeasonalRating — **[witness]**
  `storagecontroller_seasonal` (Yf 2.1e-5 + event-log line-count).
- **D14** DynamicExp RPN-index fix — **[witness]** `Dynamic_KundurDynExp`
  (V **0.87** — the largest Rung-1 move).

**Real per source but NO corpus witness — synthesize:**
- **A1** NCIM (`Algorithm=NCIM` opt-in) — no corpus deck. **synthesize (WP-U1.7)**.
- **A2** WindGen 0.15.x form — no corpus deck defines it. **synthesize (WP-U1.8)**.
- **A3/A5** force hooks (`InjCurrent`/`ITerminal`/`Yprim`/`StateVar`) — opt-in,
  no witness. **synthesize (WP-U1.9)**.
- **B3/C1** new line-constant paths (`EpsRMedium`/`HeightOffset`/equivalent
  spacing/`SemiconLayer`/CNTS) — defaults preserve numerics, no corpus deck sets
  the new props. **synthesize (WP-U1.4)**.
- **C4** `Solve/Clear all`, **C6** Transformer BH curves, **C7** `TCC_Curve.none`,
  **D15** `LookupVariable` case-insensitivity — no witness. **synthesize
  (WP-U1.1/U1.6)**.
- **D9/D16** meter/allocation disabled-skip — needs a disabled-meter deck.
  **synthesize (WP-U1.5)**.
- **D3** sqrt-guard, **D7** IBR `IMaxPPhase`, **D8** X23/X13 trap-zero, **D2/D4**
  InvControl edge fixes — edge-only, partially inside `midi_invcontrol`; each WP
  adds a targeted deck. **synthesize (WP-U1.2/U1.3)**.

**Observed diff with NO inventory row — ADDED:**
- **B9 (new)** Binary/MMF shape + XYcurve file parsing **access-violation crash**
  (`#58614`) in dss_capi **0.15.0b4**, absent in 0.14.5. Decks
  `modes/shape_binfiles` (`g4.csv` GrowthShape), `modes/shape_mmf`,
  `modes/xycurve_files` (`rc.csv`). A 0.15.x-era regression in binary/memory-mapped
  shape parsing — the same crash also fires in EPRI r4088/r4133 (see
  `delta_r4088_r4133.md`). **No port action** (the Rust engine reads these
  files correctly); recorded so the oracle-side crash is not mistaken for a port
  bug, and because it forced the modes family to be swept one-case-per-process.
