# Corpus-round records (CF / CF2 / coverage waves / LINE-DEEP / FIX-DIRECT / reorg)

> **Archived verbatim from `STATUS.md` on 2026-07-12** to keep the living handoff lean. These are the frozen
detailed corpus-completeness and coverage-wave records; the live `STATUS.md`
keeps a condensed summary (proven floors vs fixed bugs) with a pointer here.

**Corpus coverage wave: asymmetric (Phase 3), 2026-07-12.** Added 7 boundary-
coverage decks + 1 extension to the `asymmetric/` family from
`corpus_matrix/asymmetric.md` (36 → 43 cases). Each validated on the pinned
oracle (converges + bit-identical across 2 oracle processes + a
feature-sensitivity probe) and green on the live family gate at **micro** tier.
- **Line:** `line/line_geometry_asym` (LineGeometry+WireData full-asym Carson
  Z/Yc + 1φ reduced spur + the LongLineCorrection branch), `line/line_cable_asym`
  (CNData + TSData reduced-Y), `combo/midi_geometry_cable_asym` (geometry→cable→
  delta-wye ground-return coupling).
- **Transformer:** `transformer/transformer_wyedelta_asym` (wye-delta delta-lead
  +30° + ungrounded delta-delta with a wye cap pin).
- **Load:** `load/midi_load_vregion_asym` — a weak-source radial that develops
  physical sag so every Load voltage-region branch fires (interpolate_y95_ylow,
  const-Z below Vlowpu, Yeq105 clamp above Vmaxpu) across models 1/3/5 and the
  previously-unpinned **models 6/7** (constP-fixedQ / constP-constZ<Vmin, feature-
  sensitive only off-normal — hence a vregion deck, not the flat load_asym).
- **Generator:** `generator/gen_currentlimited_asym` (Model 7 DoCurrentLimitedPQ
  clamp fired unbalanced below Vminpu + Model 4 + Model 3 latched at a tight
  maxkvar).
- **DER:** `der/der_state_asym` (Storage IDLING, PVSystem below %CutOut → P=0,
  PVSystem at the kvar clamp with WattPriority; all wye-3φ).
- **Extension:** `capacitor/capacitor_asym` +`cfilt` (series-tuned R+XL filter →
  full R+L+C YPrim off the pure-C diagonal).
- **Real port bug caught + fixed:** `line_geometry_asym` exposed that
  `Set LongLineCorrection=yes` was stored on the circuit but **never applied** to
  the sym-components Line YPrim (the port doc even flagged it "Phase 7+"; the
  flag was threaded through SysCtx but unused). Ported `TLineObj.DoLongLine`
  (Line.pas:1046) + the `long_line` Z/Yc/series/shunt branches (Line.pas:
  1199-1269, :1369-1377) in `elements/pd/line/solve.rs`, using the RTL-faithful
  `csqrt_fpc`/`cinv_fpc`/`cdiv_fpc` (naive FPC `cinv`; `csqrt_fpc` exposed
  `pub(crate)`). Matches the oracle bit-exactly (2e-3 V gap → under the micro
  floor). Regression: `long_line_correction_matches_oracle_yprim` (oracle-anchored
  YPrim) + `_changes_the_long_line_yprim` (flag-respected guard).
- **Discarded / deferred (documented, no escalation):** `vsconverter_asym`
  discarded — the asymmetric family runs a full-model compare of *every* element's
  currents (no per-element opt-out), and the upstream VSConverter `GetCurrents`
  bug makes the oracle's self-reported currents violate KCL + mutate state on
  every read (CLAUDE.md / corpus_live.rs:1558 explicitly forbid VSConverter in a
  live full-model compare); it is already correctly gated via source-current KCL
  in `exec/tests/vs_converter.rs`. UPFC modes 2/3 deferred (low priority): the
  series-injection YPrim stamp is control-mode-independent and already pinned by
  `upfc_asym` mode 1; modes 2/3 only vary the control dispatch and would perturb
  that deck's tol=1e-12 pin for negligible stamp coverage.
- **Settle pass (audit findings, 2026-07-12).** (1) *Fixed:* the 7 new decks were
  in `manifest.json` (bijection-guarded) but absent from the `ASYMMETRIC_REQUIRED`
  anti-shrink floor in `corpus_live.rs`, so a joint deck+entry deletion would pass
  undetected — and flooring combo decks but not the new `midi_geometry_cable_asym`
  was inconsistent. Added all 7 to `ASYMMETRIC_REQUIRED` and widened the floor's
  doc comment; gate stays green. (2) *Cleaned:* removed the untracked, out-of-scope
  `tools/oracle/validate_deck.py` ad-hoc wave helper (never committed; leftover
  clutter outside the family dir). (3) *No defect:* the `vsconverter_asym` discard
  was flagged as possibly under-documented in the handoff text, but it is already
  fully recorded above with the correct rationale (full-model current compare has
  no per-element opt-out; the `GetCurrents` bug forbids a live compare) — nothing
  to change.

**Corpus coverage wave — Line DEEP boundaries (branch `line-deep`), 2026-07-12.**
Added 4 live boundary decks to `asymmetric/line/` per
`corpus_matrix/line_deep.md`, closing the ranked solve-level Line gaps (43 → 47
asymmetric cases). Each validated on the pinned oracle (converges + bit-identical
across 2 oracle processes + a feature-sensitivity probe) and green on the live
family gate at **micro** tier; all 4 added to both `manifest.json` and the
`ASYMMETRIC_REQUIRED` anti-shrink floor. Family gate wall-time ~8.4 → ~9.1 s.
- `line_spacing_asym` (gap 1, the biggest): `FMakeZFromSpacing` (Line.pas:1964) —
  the LineSpacing path, DISTINCT from geometry. Overhead `Spacing` 4→3 auto-reduce
  (NWires>NPhases) + a spacing+cable (`TSCables`/`Wires`) 1φ spur firing the
  `gotRatingsAfterSpacingConds` seed. **DERI** default → U1.2-STABLE; also the
  U1.4 spacing default-path no-change anchor.
- `line_llc_harm_asym` (gap 2): LongLineCorrection at f≠base — two SymComponents
  long lines at the 5th harmonic (`Set frequency=300`) → per-freq `DoLongLine`,
  the 0-seq sub-branch, LLC FreqMultiplier Z-assembly + shunt-C; `llh2` c1=0 hits
  the `G_h:=EPSILON` no-skip case. Set-frequency snapshot + direct YPrim compare
  (deviation from matrix P2's mode=harmonics/monitor — strictly stronger, no
  nonlinear-load-at-harmonic; recorded in the manifest note).
- `line_ground_z_asym` (gap 3): explicit `Rg/Xg/rho` at the 7th harmonic
  (`Set frequency=420`) → `Xgmod` and `Rg·(FreqMult−1)` per-term isolation.
  U1.2-INVARIANT regression — `Kxg` deliberately keeps the 658.5 constant (ledger
  B2/D1) while Get_Ze moves to 658.853.
- `line_fullcarson_asym` (gap 4, matrix-optional): FullCarson EarthModel at
  solve level (only DERI+SimpleCarson had live decks). Also a U1.2-invariant
  control (FullCarson Get_Ze unchanged).
- **No port bugs caught** — all four matched the oracle first try (the Line
  surface was already hardened by the 1-ULP saga + the LLC stored-but-never-
  applied fix from the prior asymmetric wave). No decks discarded; no escalation.
- Set-frequency-snapshot pattern (per `autotrans_gic`) with linear shunt-reactor
  excitation keeps the harmonic solves frequency-well-defined on both engines and
  makes the Line YPrim the direct comparand. `population.lock.json` regenerated
  locally only (coordinator regenerates at merge).
- **Settle (two Minor doc/convention findings, no behavior change):**
  (1) `line_ground_z_asym` header cited the LLC branch (`:1263`/`:1269`) but the
  deck runs the non-LLC else branch (no `LongLineCorrection`, default FALSE,
  Circuit.pas:492) — retargeted the citations to `:1281-1287` and spelled out that
  the two branches share the `Xgmod` formula but differ in the `FreqMultiplier`
  scaling of the `ZinvValues` assembly (manifest note updated too). (2) Removed the
  misplaced `TODO(compat)` tag from the deck comment: the 658.5-in-`Kxg` constant
  is the faithful 0.14.5 spec literal (Line.pas:520/713/967) that even the newer
  engine keeps, so it is NOT a section-6 precision shim and gets no compat tag on
  the three port sites (`line/accessors.rs:453`, `code.rs:60`, `mod.rs:366`); it is
  an upgrade-ledger item (B2/D1). Reworded to plain prose that avoids the literal
  tag string, keeping the `TODO(compat)` grep namespace a pure index of cleanup
  code sites. Both are doc-only; the deck's boundary, oracle validation, and gate
  are unchanged.

**Corpus coverage wave — controls family (branch `cgen-ctrl`), 2026-07-12.**
35 new live decks (34 mini + 1 midi) closing Pascal-branch gaps per
`corpus_matrix/controls.md`; each oracle-validated (two-process determinism +
feature-sensitivity, probe-proven) and green on `controls_cases_match_oracle`
with the full-model + property-parity mandate (controls family 57 → 92 live
decks). Per element group: RegControl (ldc / reverse / remotebus /
inversetime+vlimit), CapControl (pf lead-fold+pctMinkvar / time midnight-wrap /
voltoverride), InvControl (drc / vv_drc / wattpf / wattvar / avr-linear /
monbus + midi mixed-fleet drc), StorageController (follow / support /
i-peakshave / loadshape / chargelow), Relay (voltage / revpower / generic /
distance / td21 / doc), Recloser-ground, Fuse-3ph, SwtControl-lock,
GenDispatcher-kvarlimit, EnergyMeter-options, Monitor (modes 6/9/11, seq/mag
flag bits), ExpControl, UPFC (mode 1 / mode 4 — see settlement below). Family gate wall-time ~21 s →
~24 s. No port bugs (one SUPPORT deck's first draft hit a control-iteration
knife-edge from a co-located gen + railing storage; a gentler redesign matched
exactly — the shared do_load_follow_mode is not at fault). **Findings/open
items:** (1) Monitor modes 8/10/12 (winding I/V, LL) have a deferred stub
sample body (`sample.rs` `_ => return`) while `header.rs` declares
`record_size`, so a monitor using them PANICS (OOB in `channel()`) rather than
erroring cleanly — excluded from the new decks, remains uncovered pending that
port work; (2) `midi_relay_dist` (distance+DOC coordination), UPFC modes 3/5
deferred (budget).

**Controls-wave settlement (branch `cgen-ctrl`), 2026-07-12.** Audit of the 35
wave decks flagged four decks whose pinned-oracle feature-sensitivity did not
hold as claimed. Each settled empirically (pinned dss-python 0.15.7, two-process
determinism), fixed, and re-green on `controls_cases_match_oracle` (91 decks
match, 1 both-abort):
- **`recloser_ground` (was a phase trip).** With no ground TCC assigned,
  `GroundFast/GroundDelayed` default NIL and Recloser.pas l.578 skips the whole
  ground path; the phase curve tripped (eventlog PHASE TARGET) and groundtrip was
  dead (30→3000 bit-identical). Fix: assign `groundfast=a grounddelayed=d` and
  raise `phasetrip` to 3000 (above the ~2585 A faulted-phase current) so only the
  low groundtrip=30 pickup fires on the ~2558 A residual. Now GROUND TARGET;
  proven feature-sensitive — groundtrip=3000 or dropping the ground curves stops
  the trip, phasetrip=30000 is bit-identical (phase path inert).
- **`upfc_pac`→`upfc_vreg` (mode 2 was a stub).** UPFC mode 2 (phase-angle) is
  bit-identical to mode 0 (off) in dss_capi 0.14.5: `GetOutputCurr` l.605 hardcodes
  `CurrOut:=0`, and the mode-2 shunt reactive-comp path (l.787) requires the UPFC's
  `element=` monitor (else `checkPF` false → no `UploadCurrents`) and does NOT
  converge in the interface-xfmr topology (max control iterations). Mode 3 collapses
  onto mode 1 here. Repointed the deck to **mode 1 (series voltage regulator)** —
  the canonical UPFC branch, uncovered in the controls family — renamed to
  `upfc_vreg.dss`. Feature-sensitive: mode=0 removes the injection and refkV
  ±0.008 shifts the regulated output. (Manifest path + `corpus_live.rs` list
  updated.)
- **`regcontrol_inversetime` (both knobs inert).** In daily 1h mode the sub-second
  inverse-time delay scaling collapses to the same hourly tap, and vreg=123 sat
  below Vlimit=124 so the clamp never bit (both 24-step fingerprints bit-identical
  to neutralized). Rebuilt as **duty mode, 2 s steps, 40 steps**, overvoltage
  source (pu=1.045) driving a multi-tap buck: inverse-time now places each tap on
  a different sub-step (first tap Sec=10 vs Sec=18 with `inversetime=no`) and
  Vlimit=119 (< the vreg=120/band=4 window) bucks the endpoint to tap 0.95625 vs
  0.975 with `vlimit=0`. Both knobs now provably bind.
- **`capcontrol_pf` (note fix only).** Core PFCONTROL coverage is valid and
  feature-sensitive (type=pf vs type=kvar differ; offsetting=-0.99 vs +0.90
  differ). Two secondary claims were false — pctMinkvar=25 does not bite
  (pctminkvar=0 bit-identical) and no step re-arms ON (onsetting inert). Header +
  manifest note corrected to state only the proven boundaries.

No-fix findings (refuted / documentation-only, disclosed here): the report's
"1 midi" wording refers to the multi-element filename convention — the manifest
tier stays `kind=micro` per the family convention (no tier invented); the UPFC
decks are probes-only / n_steps=1 (a UPFC snapshot emits no control-round
eventlog) — a defensible deviation from the matrix's suggested EL flag; and the
Monitor modes 8/10/12 deferred-stub port gap (`sample.rs` `_ => return`) is
pre-existing and correctly avoided (the wave's `monitor_modes_hi` uses supported
modes 6/9/11 only), owned by the future monitor-winding port, not this wave.

**FIX-DIRECT (PCElement LastSolutionWasDirect shortcut), 2026-07-12.** Ported the
escalated DIRECT-mode `TPCElement.GetCurrents` shortcut (PCElement.pas l.137,
branch `fix-direct`): after a direct solve (`LastSolutionWasDirect` set at the end
of `SolveDirect`, Solution.pas l.1282; cleared at the end of `DoPFLOWsolution`,
l.1022 — the Rust flag lifecycle in `power_flow.rs` was already 1:1) and outside
dynamics/harmonics, PC terminal currents are `CalcYPrimContribution` =
`YPrim·Vterminal` (frozen shadow-admittance), NOT the model `conj(S/V)`.
- Threading: `SysCtx.last_solution_was_direct` + `SysCtx::pc_direct_shortcut()`
  (the exact l.137 condition) + `CktElementData::calc_yprim_contribution`
  (PCElement.pas l.162 — no InjCurrent subtraction, no Iterminal marking).
- Wired per the Pascal class hierarchy: Load / Generator / IndMach012 inherit the
  base `GetCurrents` → shortcut; Storage / PVSystem get it only when NOT in GFM
  mode (`TInvBasedPCE.GetCurrents` calls `inherited` on the non-GFM branch,
  InvBasedPCE.pas l.216-219, and never on the GFM branch); the seven overrides
  that never call `inherited` (VSource, Isource, GICLine, GICsource, VCCS, UPFC,
  VSConverter) are untouched.
- Powers/Losses interplay: `compute_iterminal`/`refresh_iterminal` both route
  through `get_currents`, so the cache-aware Powers/Losses and the fresh Currents
  agree after direct (oracle probe: Powers-before-Currents shows no order
  dependence; P = V·conj(YPrim·V)).
- Probe facts (pinned dss-python 0.15.7): `Set mode=direct` WITHOUT a solve does
  NOT flip currents; a snapshot solve after direct reverts to model currents;
  `Set loadmodel=admittance` + `Solve` in snap mode takes the same shortcut
  (SolveCircuit→SolveDirect, 1 iteration); direct solve leaves `Iterations`=1.
- Regression: new live deck `tests/corpus/modes/time/direct.dss` (snapshot solve
  then `Set mode=direct; Solve`; oracle-validated: converged, bit-identical
  across two processes; feature-sensitive: ld1 |I1| 38.461 A shortcut vs 39.540 A
  model, and a negative-control run with the shortcut disabled fails the gate at
  |diff| 1.06 A vs 1.0e-6 band) + unit test
  `load::tests::direct_shortcut_selects_yprim_currents` (shortcut = YPrim·V,
  harmonics exclusion, snapshot-after-direct reversion).
- Note: `sum_all_currents` builds `sys_ctx` fresh inside the Newton loop, so a
  PFLOW solve issued right after a direct one sees the flag still true during
  `SumAllCurrents` — exactly Pascal's live-variable read (cleared only at
  `DoPFLOWsolution`'s end).

**FIX-DIRECT settle (audit findings), 2026-07-12.** Five auditor findings on the
shortcut wave (all coverage, no code defect) settled empirically:
- *GFM-exclusion branch untested (Major).* Added hermetic dispatch unit tests
  `storage::tests::direct_shortcut_excluded_in_gfm_mode` and the PVSystem twin:
  with a known diagonal YPrim + nonzero InjCurrent and the model recompute
  suppressed (`iterminal_solution_count == solution_count`), a GFM unit in DIRECT
  mode reports `YPrim·V − InjCurrent` (GFM path, InvBasedPCE.pas l.211-219) while a
  non-GFM unit reports the frozen `YPrim·V` (shortcut) — the two differ by exactly
  InjCurrent, so inverting/dropping the `!self.base.gfm_mode` guard fails the test.
- *Generator / IndMach012 per-class wiring untested (Minor).* Added
  `generator::tests` + `ind_mach012::tests` `direct_shortcut_selects_yprim_currents`
  (same hermetic method: direct read = `YPrim·V`, normal read = `YPrim·V − Inj`,
  difference = InjCurrent). All five inheriting classes (Load, Generator,
  IndMach012, non-GFM PVSystem, non-GFM Storage) now have a real-`get_currents`
  dispatch guard; a live oracle deck was judged lower-value than the hermetic
  guards since the base shortcut numerics are already oracle-pinned by
  `time/direct.dss` and the wiring is a shared one-liner.
- *Dynamics exclusion arm not asserted (Minor).* Added the `is_dynamic_model:true`
  mirror to the Load test — both OR terms of the l.137 `not(IsDynamicModel or
  IsHarmonicModel)` guard now assert.
- *Direct→Newton SumAllCurrents interaction not oracle-gated (Minor, no action).*
  Confirmed a faithful 1:1 of Pascal's live-variable read; the default NORMALSOLVE
  never calls `SumAllCurrents`, so it is a no-op there. Rests on code fidelity by
  the auditor's own assessment — no defect.
- *Deck captures only final direct state; revert only in-process (Minor).* The
  post-direct reversion is pinned by the Load unit test (`snap2` → model current at
  1e-9); oracle-gating the revert (a trailing snapshot step) was deferred as
  low-value — the revert is just the flag-cleared default path.
- Set/CLEAR completeness (brief item 1): the Pascal `:= TRUE` at Solution.pas
  l.2704 is inside `SolveAD` under `{$IFDEF DSS_CAPI_ADIAKOPTICS}` (A-Diakoptics
  Part II, not in this base) — out of scope; the three non-ADIAKOPTICS lifecycle
  points (init l.464, clear l.1022, set l.1282) are ported 1:1.

**Corpus family reorg (Phase 1), 2026-07-12.** Reorganized the three synthetic
deck families into per-element/method subfolders (branch `corpus-reorg`); a
pure move — **no deck content changed** (every family deck is self-contained;
the only external fixture refs are bare same-dir names inside the multi-file
`inputformat/*` subfolders, which move as a unit, so no depth `../` fix was
needed). Per-family case counts unchanged (asymmetric 36, controls 57, modes 40).
Folder map:
- `asymmetric/<element>/`: line, transformer, capacitor, reactor, load, vsource,
  isource, generator, der, indmach, vccs, upfc, fault, autotrans (autotrans_snap
  / midi_autotrans_asym / autotrans_gic), gic (gicline/gictransformer/gicsource/
  gic_midi), combo (combo_chain/combo_mesh/midi_asym).
- `controls/<control>/`: regcontrol, capcontrol, invcontrol, storagecontroller,
  gendispatcher, recloser, relay, fuse, swtcontrol, energymeter, monitor, sensor,
  isource, autotrans, gfm, combo (combo_protection/combo_voltvar/combo_metering/
  midi_controls/midi_protection).
- `modes/<method>/`: time (generaltime{,_yearly,_duty}/ld1/ld2/peakday),
  montecarlo, autoadd, newton, harmonics (reactor_rlcurve/isource_harm),
  inputformat (shape_binfiles/shape_mmf/shape_filearr/xycurve_files multi-file
  subfolders), batchedit, reduce, makeposseq, pstcalc, upgrade.
- Remap updates: the three family `manifest.json` `path` fields; the
  `ASYMMETRIC/CONTROLS/MODES_REQUIRED` floors + population lock `family_paths`
  in `corpus_live.rs`/`population.lock.json`; the fixture/midi generators under
  `tools/decks/` (subfolder-aware `dest()` resolver in `gen_midi_decks.py`);
  doc-comment deck paths in a few `src/` tests; and the current-layout deck
  paths in the operational docs (TESTING.md, tests/corpus/README.md,
  tools/opendss/README.md → `modes/inputformat/shape_binfiles/`,
  `modes/upgrade/upgrade_pilot.dss`). Plan docs (CONTROL_COVERAGE/
  GAPS/DIAKOPTICS/UPGRADE) keep their historical flat paths as history.

**CF2-G (GFL/GFM daily dynamics divergences), 2026-07-12.** One real-bug fix +
4 deck migrations (branch `cf2-g`). The four IBRDynamics_Cases whole-IEEE123
GFL/GFM daily decks were ABOVE-BAND (GFL source-node imag; GFM islanded node ~0.9 V).
- **Root cause (both signatures, one bug):** `PVSystem::InitStateVars` /
  `IntegrateStates` hardcoded `ShapeFactor = 1+j1` in dynamics mode, assuming
  `ActiveLoadShapeClass == USENONE`. Pascal (PVsystem.pas l.2192 & l.2281)
  dispatches on `ActiveLoadShapeClass` **even in dynamics**, so a deck that does
  `set loadshapeclass=daily; set time=(10,0)` samples the irradiance shape at that
  hour. The port applied full sun (`PanelkW=800` vs oracle `594.78`); the GFL PV
  over-injected, moving node V ~16 V near the PV, and in the GFM decks the islanded
  PV perturbed the storage-formed island voltage (amplified to ~0.9 V). Fix: honor
  the load-shape class in the dynamics init/integrate (shared helper
  `apply_dynamics_load_shape`); Storage needs no change (its Pascal `IntegrateStates`
  does not re-dispatch — its ambient ShapeFactor is already 1). Empirically: node V
  → faer floor (2.3e-6 V) on all four decks; snapshot solve was already clean, so
  the bug was born entering dynamics.
- **Migrated → solvable_now** (`large_floating_delta`): GFL_IEEE123 Daily + Daily_DynExp,
  GFM_IEEE123 Daily, GFM_IEEE123_AmpLimit Daily_CurrentLimit. `solvable_now` **+4**.
- Note (AmpLimit deck): the storage `it[last]` state var drifts on the long
  trajectory (open-loop AC integration; in GFM only `it[0]` feeds the injection→node-V
  fixpoint, so it stays pinned while it[1]/it[2] drift). It is not a gated quantity
  and node V/currents/powers all match.
- **Escalation pass (fable), 2026-07-12.** Verified the fix 1:1 vs PVsystem.pas
  l.2192-2210/l.2281-2299 (mult+temperature per class at `DynaVars.dblHour`, else
  `ShapeFactor := 1+j1` with `TShapeValue` untouched), the Storage counter-claim
  (Storage.pas `InitStateVars`/`IntegrateStates` never dispatch on the class),
  and the deck-4 drift claim (Storage.pas:2142 — only `it[0]` scales `BaseV`;
  the per-phase integrate loop is 1:1, so the closed loop pins phase 0 while
  phases 1+ feed nothing gated; `compare_variables` is not enabled for these
  decks). No tolerance/band/manifest gaming vs base `97b186c`. Added the missing
  regression test (`pvsystem::tests::dynamics_loadshapeclass_selects_mult_and_temperature`,
  incl. the USENONE keeps-TShapeValue pin) and ported the three remaining
  same-family gaps found by sweeping every Pascal `case ActiveLoadShapeClass`
  site: **VSource** (`GetVterminalForSource` DYNAMICMODE arm + DYNAMICMODE in the
  loadshape-Vmag branch, VSource.pas:1006-1026), **Isource** (`GetBaseCurr`
  DYNAMICMODE arm, Isource.pas:403-416), **IndMach012** (`SetNominalPower`
  GENERALTIME/DYNAMICMODE arm, IndMach012.pas:1091-1105) — each with a unit test.
  With `USENONE` (every existing green deck) all three reduce to the previous
  behavior; Load/Generator already dispatched correctly.

**CF2-R (#485 control-settling family — 3 decks migrated, no bug), 2026-07-12.**
Branch `cf2-r`. `solvable_now` **279 → 282** (+3 `expect_solve_abort` cases,
migrated out of `skipped_needs_investigation`). **No engine change** — the port
already reproduces #485 exactly.
- **Decks:** `Examples/ADiakoptics/IEEE_123_Bus-G/Torn_Circuit/Master_Interconnected.dss`
  (plain interconnected model, no AD commands), `IEEETestCases/8500-Node/Run_RecloserSiting.DSS`,
  `Examples/Microgrid/GridFormingInverter/GFM_IEEE8500/Run_RecloserSiting.DSS`.
- **The "divergence" was a measurement artifact.** The park notes claimed "Rust
  67 clean vs r3723 109-with-#485" — but that compared Rust's **first** solve
  (the deck's own `Solve`, clean 67 total power-flow iters) against the oracle
  harness's **second** solve. The harness `run_case` issues an extra `solve`
  after `Compile`; that re-runs the control loop from the settled taps, a
  regulator sits on a band edge and re-arms ±1 tap each control iteration
  (hunting), never drains the control queue, hits `MaxControlIter=10` and
  aborts with **#485** on **both** engines. Driven identically (Compile + one
  extra solve), Rust and the official r3723 are **bit-for-bit equivalent**:
  all RegControl taps + capacitor states **exact**, node V to **5.5e-11** rel
  (8500-node) / **8.1e-9** rel (IEEE123) over every node, same 10 control iters
  / same total iters (109 / 30), same 261 / 92 event-log lines.
- **Accounting answer (a):** both engines reach the SAME control-limited state
  by the SAME control path — a *truncation*, NOT a settled fixpoint. `#485` is
  raised because `ControlActionsDone` never becomes true within `MaxControlIter`
  (Pascal `SolveSnap`, `Solution.pas:1189-1209`): a regulator fires a tap change
  every control iteration (the event log rebuilds Y at each of the 10 control
  iters on all three decks — verified against r3723), so the loop is truncated
  at `ControlIter=10` mid-adjustment. The captured taps/caps/V are that
  identical truncation point, reproduced 1:1 by `solve_snap`
  (`solution/solution/power_flow.rs`); the per-deck manifest notes give each
  deck's exact hunting/re-arm sub-mechanism. The reported "iterations"
  (67/109/30) is `Solution.Iterations` = **total power-flow iterations**, not
  control iterations (always 10 = the cap).
- **Migration mechanism:** the pinned oracle *raises* #485 at solve (dss-python
  surfaces `DoSimpleMsg` as an exception), so a per-step compare is impossible;
  gated instead via `expect_solve_abort: "Max Control Iterations Exceeded"`
  (`run_and_compare_abort`) — both engines abort the solve with the same
  message, Rust setting `solution_abort`. `post: ["Solve"]` supplies the
  harness's extra solve on the Rust abort path (the deck's own solve is clean).
  Verified full-state identity against official r3723 via the Oddie bridge; the
  gate itself uses the pinned 0.14.5 oracle (also aborts, confirmed).
- **Audit settle (2 auditors, all findings Minor, no code change).**
  - *r3723 is not warn-only (refuted).* An auditor claimed r3723 via Oddie emits
    #485 as a non-fatal warning with no raise. Empirically it RAISES the same
    #485 at solve, exactly like the pinned oracle (dss-python's error check
    elevates the Direct DLL `DoSimpleMsg` to a `DSSException` — probed on all 3
    decks). So the `corpus_live_opendss` exclusion premise stands; its comment
    and the `expect_solve_abort` doc are corrected to say BOTH channels raise.
  - *Why the numerical identity is not a bespoke committed state-compare.* BOTH
    oracle channels raise #485 at solve, so `run_and_compare`'s checkpoint
    capture cannot line up a solved state on either — the full Rust==oracle
    settled state (taps/caps exact, V 5.5e-11 / 8.1e-9) is only reachable via an
    exception-tolerant capture (the offline probe). The mandatory gate instead
    pins the #485 *mechanism* via the message, which on the Rust side already
    implies `converged_flag` + `control_iteration==MaxControlIter` (see
    `solve_snap`). The 8500-node / IEEE123 regulator machinery these decks
    exercise is also heavily gated by many CONVERGING sibling decks
    (`8500-Node/Master.dss`, `Run_8500Node*`, `123Bus/IEEE123Master.dss`, the
    torn `Master.DSS`, GFM variants) that DO full pinned-oracle numeric compares,
    so the only residual unguarded surface is a regression that shifts the
    *hunting-truncation* state while leaving every converging solution
    bit-identical — a narrow class both auditors rated LOW. Closing it fully is
    feasible future work (verified: both oracles leave a readable converged state
    post-catch, and a compare would pass): an opt-in `tolerate_solve_abort` flag
    in `oracle_server.py` + a state-compare in the abort gate; deferred as
    disproportionate for this narrow LOW risk.
  - *population.lock is regenerated in the worktree, NOT committed (per brief).*
    The gate is green with the regenerated lock; the coordinator must run
    `DSS_UPDATE_POPULATION_LOCK=1` and commit the lock at merge, else merged main
    is red (manifests 282 solvable / 23 skipped vs the committed lock's 279/26).

**Corpus coverage wave — `modes` family (branch `cgen-mode`), 2026-07-12.**
Added 8 feature-sensitive, two-process-validated solve-mode decks closing the
zero-coverage `TSolveMode` branch gaps (modes family 40 → 48). Each deck: pinned
Pascal-cited boundary, oracle-solvable, bit-identical fingerprint across two
oracle processes, feature-sensitive (neutralizing the pinned feature moves the
oracle output). Live-green under `modes_cases_match_oracle` (full model + all
element currents/powers/losses + meters/monitors, `micro` tier).
- **`time/daily`** (DAILYMODE, 24-step loop + global LoadMult applied — PeakDay
  ignores it); **`time/daily_bigstep`** (stepsize=7200s → the IncrementTime
  `while t>=3600` multi-decrement branch, unreachable at h≤3600); **`time/yearly`**
  (YEARLYMODE shape + no-LoadMult + the PriceCurve→PriceSignal→price-dispatched
  Generator arm); **`time/duty`** (DUTYCYCLE h=1s + ControlMode=TIMEDRIVEN +
  `duty=` shape); **`time/midi_duty_ctrl`** (DUTY default TIMEDRIVEN lets a
  recloser fire a timed trip/reclose sequence — eventlog + ctrlqueue compared);
  **`harmonics/harmonic_hlist`** (HARMONICMODE explicit `Set harmonics=(1 3 5 13)`
  list + skip-fundamental gate + a zero-injection frequency); **`harmonics/harmonict`**
  (HARMONICMODET sequential-time harmonic sweep); **`reset/mode_reset`** (the
  Set_Mode reset tail — ResetAll meters/monitors/controls on a mode change).
- **Real port bug caught + ESCALATED (not fixed here): DIRECT-mode PC-element
  current reporting.** A `direct` deck (constant-power loads, `Set mode=direct`)
  found node voltages match the oracle but Load currents diverge (42.056 vs
  40.731 A, ~1.4 A). Root cause: Pascal `TPCElement.GetCurrents` (PCElement.pas
  :137) takes a `LastSolutionWasDirect` shortcut — report `YPrim·Vterminal` (the
  frozen shadow-admittance current), NOT the load-model current — which the port
  omits (acknowledged TODO at `pc/load/accessors.rs:133`). The fix is 1:1 but
  broad (a new `SysCtx.last_solution_was_direct` threaded through ~13 PC
  `get_currents` impls + ~10 test-literal builders); its blast radius is
  DIRECT-mode-only (untested until now), so it is deferred to a dedicated WP. The
  `direct` deck is discarded (no feature-sensitive DIRECT deck can avoid the bug:
  only constant-Z loads dodge it, and those make DIRECT ≡ snapshot). See report.
- Deferred (matrix lower-priority, live-channel overlap with existing unit tests):
  `dynamic` (dSpeed f32-cancellation floor caveat) and `faultstudy` (needs a
  bus-SC compare surface the harness lacks) — `exec/tests/dynamics.rs` /
  `fault_study.rs` still cover the numerics.
- **Audit settle (2 Minor).** (1) Added the 8 GEN-MODE decks to the
  `MODES_REQUIRED` anti-deletion floor (`corpus_live.rs`), per the WPG.13/WPG.17
  convention that every feature deck joins the floor — the bijection guard only
  catches a single-sided drop, the floor catches a coordinated file+manifest
  removal. (2) Fixed the daily/yearly manifest-note engine citations: the
  per-mode LOAD multiplier lives in `nominal.rs` (Daily `f *= load_multiplier`
  :138-141; Yearly :146-148), not `set_generator_disp_ref` (`power_flow.rs`
  :284-285, the generator dispatch reference); corrected the yearly note's
  imprecise "loads apply no LoadMultiplier" (loads DO scale by LoadMult in every
  mode — the deck just leaves it at 1.0; it is the generator dispatch reference
  that omits it in YEARLY).

**CF-A (corpus completeness: base-freq inheritance + BOM + monitor-export +
quote), 2026-07-12.** Four small real-bug fixes + 4 deck migrations (branch
`cf-a`). `solvable_now` **245 → 249**.
- **Base-frequency inheritance (TC-1).** `add_object` now seeds every circuit
  element's `base_frequency` from the circuit fundamental at creation (Pascal
  `TDSSCktElement.Create` `BaseFrequency := ActiveCircuit.Fundamental`,
  CktElement.pas:203) instead of the hardcoded 60; VSource/Isource `src_frequency`
  follows (`SrcFrequency := BaseFrequency`, VSource.pas:644 / Isource.pas:319); a
  LineCode inherits it too (LineCode.pas:493). Monitor is the lone exception —
  hard-pinned to 60 (Monitor.pas:472, oracle-verified). Fixes the European LV
  feeder that had its source Vmag zeroed by a 60-vs-50 freq mismatch (the
  previously-named LVTestCase residual is now resolved and migrated).
- **UTF-8 BOM strip (TB-U3).** `do_redirect` strips a leading U+FEFF from every
  compiled/redirected file (Pascal loads via `TStringList.LoadFromFile`); nested
  redirects covered.
- **Undefined-monitor export → warn (TA-3).** `export_monitors` reports a missing
  named monitor on `GlobalResult` and continues (official Direct DLL
  DoSimpleMsg-2-arg is non-fatal, r3723 DSSGlobals.pas:600) instead of a hard
  error; dss_capi's #250-raise is the divergence. The oracle server tolerates the
  same #250 during the deck `Compile` (`_TOLERATED_COMPILE_ERRNOS`).
- **Bare-quote inline comment (TA-3).** `set …` get-only arms (ProcessTime/StepTime)
  no longer evaluate their value token, matching Pascal's `else`-ignore no-op; a
  trailing `' comment` (a begin-quote string, ParserDel.pas:270) landing on the
  incremented pointer no longer triggers a spurious "Invalid inline math entry".
- **Migrated** (live-compared, green): `LVTestCase/Master` + `Test/Source012Test`
  (pinned oracle, full property parity); `EPRITestCircuits/ckt5/Run_ckt5` +
  `ckt7/RunDSS_ckt7` (`oracle: r3723`, `post: set mode=snapshot`).
- **Audit settle (3 Minor findings).** (1) *Fixed:* the undefined-monitor Export
  warning now carries the `CRLF + Parser.CmdString` suffix, matching the full
  Pascal `#250 'Monitor "%s" not found. %s'` (`ExportOptions.pas:497`, official
  r3723 `:441`) — the port had dropped the `%s`; verified the other not-found
  messages (Bus #219, EnergyMeter #220, Object #256) genuinely carry no suffix,
  so only the two Monitor sites did, and the port already reproduces `CmdString`
  suffixes at command.rs #240/#267. Written to `last_result` only (not
  gate-compared on this path; overwritten by later `?`-probes). (2) *No-fix,
  proven:* EARLY_ABORT `Redirect_Abort` is not set on this warning — verified
  vendored dss_capi sets it unconditionally (`DSSGlobals.pas:291`, default True
  `:781`) but official r3723 only sets it inside `IF Not NoFormsAllowed` on a
  dialog abort (`:606-611`), so headless it never fires; the port matches r3723,
  which is also identical for ckt5/ckt7 since Export is the deck's last command,
  and it is not a regression (pre-PR code did not set `redirect_abort` either).
  (3) *No-fix, proven:* `post=[set mode=snapshot]` on ckt5/ckt7 is a symmetric
  migration idiom — the harness applies `post` to BOTH engines
  (`corpus_live.rs:712`, `oracle.run_case`) before its forced solve, converting
  the post-yearly extra solve to a single well-defined snapshot instead of a
  redundant second 8760-step run; the yearly trajectory is not deep-compared
  (records not flagged `check_meters_monitors`), a bounded coverage note, not a
  criterion weakening.

**CF-B (corpus disposition: official-oracle migrations + reclassifications)
2026-07-11, gate-green.** A corpus-completeness round: migrate decks the pinned
0.14.5 oracle can't gate (it *raises* on headless `Show`/`ShowCurrents`) to the
official EPRI **r3723** oracle via the Oddie bridge, promote the floor-proven
whole-IEEE123 GFM decks, and fix misfiled classifications. Every migration was
validated live through the real harness (`corpus_live_solvable_cases_match_oracle`),
not the triage ballpark. What landed:
- **+3 r3723-gated** (T-A #29/#27/#28): `4Bus-YYD/YYD-Master`, `34Bus/Run_IEEE34Mod1`,
  `Run_IEEE34Mod2` — r3723 treats the decks' headless `Show`/`ShowCurrents` as
  non-fatal and solves through, matching Rust (full-model compare green). Iteration
  caveat reconciled: the harness compares the deck's *final* forced-tap
  `Controlmode=OFF` solve, where Rust iterations **== r3723** (the triage's 4-vs-2 was
  the first controlled run's control-loop count, not the gated solve → the Rust≤oracle
  policy is not violated).
- **+10 large_floating_delta** (T-B U1a/U1b, pinned oracle): 4 GFM snapshots + 6 GFM
  daily/whole-day trajectories on IEEE123 — all live-green at the floating-delta
  common-mode floor. 4 of the 14 GFM/GFL trajectory decks are **above-band** and went
  to `needs_investigation` with per-deck first-divergence facts (2 GFL-daily source-node
  phase gaps ~2.8e-3; 2 GFM-daily islanded-section gaps 9.1e-1 / 1.2e-2) — NOT forced.
- **Reclassify → not_an_entry_point (+6)**: 5 fragments/stubs (T-A #30/#31/#32/#33/#18:
  34Bus/IEEELineCodes stub, MultstepDG how-to, ckt7+epri_dpv Substation fragments, TnD
  Distribution sub-model) + ckt24 `main_template.dss` (T-B D5 template via unset
  `@loadshape_script_dss`) — each verified by grepping its including master.
- **Note refreshes only** (no migration): 7 D1–D4 `missing_dependency` (hardcoded
  foreign abs-path / genuinely-absent file / off-by-one vendored stub / wrong filename,
  BOTH engines fail); the blocked families (6 AD masters → WP-AD.3; ckt5+actor family →
  M2, r3723 segfaults multi-actor; WindGen ×2 + NCIM → UPGRADE, solve on r4133;
  IEEE118 → r4133-only convergence). The 3 #485 recloser/Torn decks moved to
  `needs_investigation` (control-settling: Rust settles without #485 where the official
  engine hits it).

Population (before → after; total 915 conserved):

| manifest | before | after |
|---|---|---|
| solvable_now | 245 | **258** |
| skipped_oracle_issue | 33 | 22 |
| skipped_unsupported | 17 | 3 |
| missing_dependency | 10 | 9 |
| skipped_needs_investigation | 30 | 37 |
| not_an_entry_point | 580 | 586 |

Coverage: **245/335 (73.1%) → 258/329 (78.4%)** of entry-point decks. Full gate green
(`corpus_live` all 258 solvable cases match, incl. the 3 new r3723-gated). The
`population.lock.json` is regenerated locally to run the gate but left uncommitted (the
coordinator regenerates at merge).

**CF-B settle (audit findings, 2026-07-12).** Six findings triaged empirically; no
engine code changed (this is a manifest/doc-only branch).
- **`large_floating_delta` doc was stale (Minor, fixed).** `TOLERANCE_NOTES.md` still
  said "Currently one deck" while CF-B grew the tier to 11 (the whole-IEEE123 GFM
  family). Updated the tier-list entry + §floating-delta to list the family and state
  honestly *what is proven vs inherited*: the bitwise decomposition proof stands for the
  original `GFMSnap` deck; the CF-B daily/snapshot members are the SAME floating-delta
  circuit admitted under that precedent (not a per-deck decomposition), safe because
  `v_abs` alone is widened and every common-mode-immune channel (Y at 1e-8, exact
  iterations, differential currents/powers at `large`) stays the sentinel.
- **corpus_live "258/258 green" reproducible (Major → refuted).** Re-ran the mandatory
  `corpus_live_solvable_cases_match_oracle` clean here: **1 passed; 0 failed, 216.83s,
  258 cases matched** — the run reached and validated the CF-B tail migrations. The
  auditor's one-off failure was on the pre-existing (base-3cca7d3) `StoCtrl_Current_PeakShave/master.dss`
  DIVerbose *yearly* deck erroring on its `ckt7/DI_yr_0/` output dir — a Windows
  file-handle/AV race in the oracle→Rust corpus-dir handoff on that deck's DI output,
  NOT a CF-B change (CF-B touched zero code and zero StoCtrl entries) and not
  deterministic here. Recorded as a pre-existing gate-infra transient for coordinator
  awareness; deliberately NOT "fixed" by touching engine code on a manifest-only branch
  (would mask nothing here and needs its own audit).
- **Committed lock stale vs manifests (Minor ×2, expected).** The committed
  `population.lock.json` still carries base counts (solvable_now 245); the regenerated
  258-lock is left uncommitted per the brief. The branch as-committed therefore trips
  `population_lock_matches_manifests` until the lock is regenerated — a **hard merge-time
  dependency**: the coordinator MUST run `DSS_UPDATE_POPULATION_LOCK=1 cargo test -p
  dss-core --test population_lock` before/at merge. The settle gate was witnessed with
  the regenerated lock in the working tree.
- **Full-gate witnessed (Minor, done).** `cargo fmt --all --check` + `cargo clippy
  --workspace --all-targets -D warnings` + `cargo test --workspace` all exit 0 (pinned
  dss-python 0.15.7; regenerated lock in working tree, not committed).
- **Manifest edits coverage-neutral-or-positive (positive, confirmed).** Population
  conserved at 915 with a clean bijection (0 dups); 13 decks ADDED to the live-compared
  `solvable_now`; the GFM promotions reuse the existing tier keeping i/y at the tight
  `large` floors. No looser-band-in-place, no probe/step/meter cut.


---

### CF-C — CapControl FOLLOWCONTROL + user-model property surface (2026-07-12)

Corpus-completeness fix round. **solvable_now 245 → 248.**

- **Port 1 (CapControl `Type=follow` / `ControlSignal`).** The FOLLOWCONTROL
  machinery was present but a control-dispatch bug (`solution/controls/dispatch.rs`)
  aborted the sample with "Monitored element not set" whenever a CapControl had no
  monitored element — but Pascal `RecalcElementData` (CapControl.pas l.598-609)
  leaves `MonitoredElement = NIL` for TIME/FOLLOW and uses `effElement :=
  ControlledElement`. Fix: `monitored.unwrap_or(target)` (self-monitor), since every
  other control type without a monitored element already errors at parse. Migrated
  `Test/CapControlFollow.dss` skipped_unsupported → solvable_now (24-step daily walk,
  Cap1Mon/Cap2Mon power channels + full V compare pin the FOLLOW switching schedule
  vs the pinned oracle; `compare_eventlog` deliberately not used — the deck solves
  the whole day at compile with eventlog off, so a post-compile eventlog is logged
  asymmetrically at the arm/fire boundary). +4 FOLLOW sample-arm unit tests.
- **Port 2 (Generator UserModel/UserData + Storage DynaDLL/DynaData surface).**
  Removed `NOT_PORTED` from these four props; they now parse, store, and dump. The
  `UserModel`/`DynaDLL` side effects emit a non-fatal "Not Loaded" diagnostic and
  fall back to the built-in model — matching the official Direct DLL's warn-and-solve
  (Pascal `TGenUserModel`/`TStoreDynaModel.Set_Name`, DoSimpleMsg 570/1570), never
  loading a DLL (loader permanently out of scope, `forbid(unsafe_code)`). The DLL
  loader remains out of scope; ShaftModel/ShaftData + Storage UserModel/UserData stay
  NOT_PORTED (no owned deck exercises them). +6 surface unit tests.
  - Harness: new `expect_warnings` field on `SolvableCase` (corpus_live) tolerates a
    deck's declared non-fatal diagnostics (asserts each fires and nothing else errors),
    mirroring `expect_solve_abort`.
  - **Migrated** (vs `oracle: "r3723"`, since the pinned oracle raises #1570):
    `SimpleStorageTest.dss`, `SimpleStorageTest-1ph.dss` (Rust iter 2 == r3723 iter 2).
  - **Parked** in skipped_needs_investigation (4 Generator model=6 UserModel decks:
    `indmachtest/Master`, Kersting4wire ×3): the dss-python-over-Oddie r3723/r4133
    harness **raises #567** ("model designated to use user-written model, but
    user-written model is not defined") at solve — the DoSimpleMsg is non-fatal in the
    raw DLL (hence T-A's "r3723 YES" raw probes) but fatal through dss-python, so no
    oracle channel yields a checkpoint. Rust reproduces Pascal `DoUserModel` 1:1
    (Yprim-only + #567/iter) and converges via the built-in fallback. Also Kersting
    iter 3 > raw-r3723 2 and Kersting4wireIndMotor rel 2.9e-4 stay open. Unblocking
    needs an oracle harness that tolerates the #567/#570 DoSimpleMsg.


---

**CF-D (substation-transformer current root-cause, 2026-07-12).** Root-caused the
"RegControl/LDC SubXFMR" family — the label was **wrong** (RegControl + delta-wye
transformer exonerated on every member). The real cause is **ultra-switch
conditioning** at the substation-transformer bus (a 1e-8 Ω "switch" line, Y≈1e8 S)
and, for the CIM decks, the **Carson earth-model line-constant libm floor**. Per-deck
verdict (proofs: TOLERANCE_NOTES.md §ultra-switch / §conditioning_floor). The
ckt24 switch (`Line.Other_Feeders`, r1=1e-8 Ω at default length 1) has Y≈**1e8** S,
not the 1e10 S the CF-D commit 6200fe0 message stated (a 100× typo, corrected on
settle; only SecondaryTest's 1 mm `MDV_SUB_1_HSB` busbar genuinely reaches Y≈1e10):

| deck(s) | verdict | evidence |
|---|---|---|
| ckt24 `Run_Ckt24` + `master_ckt24` + 7 MM `ckt24` variants | **floor → solvable_now `large_ultra_switch`** | `Line.Other_Feeders` r1=1e-8 at default length 1 (Y≈1e8 S) → SubXFMR current 7.2e-4 A = ultra-switch `Y·(V1−V2)` image = `1.8·ulp(2e4 V)·1e8 S` = 6.5e-4-class (< i_abs 2e-3); node V + Y at floor; regulator lands identical tap; per-element decomposition (`DSS_DUMP_IDIFF`, CF-D settle): two dominant diffs family-wide — `Line.other_feeders` 7.1e-4–7.7e-4 A + `Transformer.subxfmr` 5.1e-4–7.2e-4 A (same switch image), both <2e-3; third tier ≤7.4e-5 A |
| CIM `IEEE13_CDPSM` | **floor → solvable_now `large`** | differs from passing `Test/IEEE13_CDPSM` only by `set earthmodel=carson`; V rel 7.7e-8 < `large` 1e-7 (Carson line-constant floor); worst meaningful per-element current diff 4.3e-6 A / rel 1.1e-7 @ `Line.fuse1` (~23× under `i_abs`) |
| `SecondaryTestCircuit_modified` | **proven floor, documented (not banded)** | cond(Y)=9.79e11 (the 1 mm `MDV_SUB_1_HSB` BUSBAR line Y≈1e10 dominates; `SSswitch` is 1 m, Y≈1e7); Y **bit-identical**, Vsource inj `Yprim·E` **bit-identical**, load base = 7 figs; 3-solver spread faer/KLU/scipy 0.5–0.8 V (gap 0.758 V inside it); residual parity 4.40e-2 vs 4.56e-2. 0.55 V (2e-5) too wide to band; stays `needs_investigation` (conditioning_floor) |
| CIM `IEEE13_Assets` | **floor, documented (no band fits)** | Carson floor + short line (Length=0.0568); V rel 4.18e-7 — above `large` (1e-7), below `large_near_ideal_source` (5e-6); stays `needs_investigation` (conditioning_floor) |
| GFM_IEEE8500 Snap/Daily/DailySmallerPV, Storage `Run_Demo1` (TC-3) | **near-floor → solvable_now `large`** | first-failing node V rel 1.5e-8–3.7e-8 < `large` 1e-7; **per-element current decomposition** (`DSS_DUMP_IDIFF`, CF-D settle): worst `|dI|` GFM ≤4.0e-7 A @ `Line.hvmv_sub_connector` (~250× under `i_abs` 1e-4 — islanded-node V offset is common-mode, currents stay sub-µA), Storage 9.1e-5 A (0.91× floor, tightest); all decompose to faer-vs-KLU floor, no element above band (TOLERANCE_NOTES §TC-3) |

Net: `solvable_now` **245 → 259** (14 migrated); `needs_investigation` retains the
2 documented conditioning floors + the LVTestCase real gap + oracle-side blocks.

**CF-D settle — audit findings settled (2026-07-12).** Six Minor findings (code +
tests audits); none overturned a verdict — all documentation-rigor. Two fixed, two
strengthened with committed empirical evidence, one recorded no-fix, one hand-off
caveat:
- **#1/#4 (ckt24 floor-proof stated `Y≈1e10 S`, a 100× error) — FIXED.** The
  ckt24 `Line.Other_Feeders` (r1=1e-8 Ω at default length 1) has **Y≈1e8 S**, not
  the `1e10 S` the note/commit-6200fe0 message stated (cross-contaminated from
  SecondaryTest's genuine 1e10 busbar). The arithmetic closes only at 1e8
  (`1.8·ulp(2e4 V)·1e8 S = 6.5e-4 A`, matching the pre-existing `harness/mod.rs`
  band note and the measured 7.2e-4 A); at 1e10 it would be 6.5e-2 A. Corrected in
  TOLERANCE_NOTES §ultra-switch, STATUS, and the manifest. Band/gate/verdict
  unaffected.
- **#3 (SecondaryTest "each busbar length=0.001 m → Y≈1e10 S" imprecise) — FIXED.**
  Only `Line.MDV_SUB_1_HSB` is length=0.001 m (→ Y≈1e10 S, drives κ); `Line.SSswitch`
  is length=1 m (→ Y≈1e7 S). Corrected in TOLERANCE_NOTES §conditioning_floor,
  STATUS, and the manifest note (verified against `Substation.DSS`).
- **#2/#5 (per-element current decomposition not recorded for the TC-3 near-floor
  + ckt24 families) — STRENGTHENED with committed evidence.** Ran the deciding
  diagnostic (`DSS_DUMP_IDIFF` live probe, worst per-element `|dI|` vs the pinned
  oracle) the triage had flagged as not-yet-done: ckt24 family worst 7.2e-4–7.7e-4
  A all on `Line.other_feeders`/`Transformer.subxfmr` (< `i_abs` 2e-3); GFM ≤4.0e-7
  A (~250× under floor, currents sub-µA while node V shifts at floor = common-mode);
  Storage 9.1e-5 A (0.91× floor); CDPSM 4.3e-6 A. Every element decomposes to the
  floor → migrations shown honest, not asserted. Recorded in TOLERANCE_NOTES §TC-3
  and the per-deck table above.
- **#3-b (SecondaryTest/IEEE13_Assets floor proofs rest on scratchpad probes) —
  NO-FIX (rationale recorded).** Both decks stay SKIPPED (`conditioning_floor`), so
  no gate depends on them; their cross-solver-spread proof follows the accepted
  `large_near_ideal_source` in-tree-prose convention. Not reproduced into a
  committed probe (matches project precedent); the floor verdict is unchanged.
- **#6 (committed tree not gate-green until the lock is regenerated) — hand-off
  caveat, by design.** `population.lock.json` is deliberately NOT committed (brief);
  after merging all CF branches the coordinator must run
  `DSS_UPDATE_POPULATION_LOCK=1 cargo test -p dss-core --test population_lock`. The
  settle gate below was witnessed with a locally-regenerated lock (solvable_now 259,
  skipped 16), reverted before commit.

