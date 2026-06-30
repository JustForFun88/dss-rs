# Phase 7 — WP7.6 (Harmonics mode) — archived per-step records

> **Archived from `STATUS.md`** (2026-06-28) to keep the live handoff lean. WP7.6
> (Harmonics) steps 1–3 are **✅ COMPLETE** on the `phase-7-extended-elements`
> branch (step 1 = the current-source harmonic family VSource + Load + the
> solve-mode driver; step 2 = the Thevenin DER family Generator/PVSystem/Storage;
> step 3 = the monitor harmonic header + the `Set mode=` monitor/meter reset + the
> corpus burn-down). **next = WP7.7 (Dynamics core).** These are the frozen
> per-step records, superseded only by the code and tests. The live `STATUS.md`
> §1e keeps a one-line-per-step summary. §3/§4/§5 cross-references resolve against
> `STATUS.md`. Plan: `PHASE7_PLAN.md` §WP7.6. The harmonic-power IEEE-1459 / oracle
> stale-`Iterminal` write-up referenced in step 2 lives (git-ignored, local only)
> under `investigations/oracle-powers-currents-harmonic/`.

---

**WP7.6 (Harmonics) — ✅ COMPLETE (steps 1–3).** The harmonics solve
mode, ported in steps split by injection family.
- **step 1 — the current-source harmonic family (VSource + Load) + the solve-mode
  driver.** Pascal `SolutionAlgs.SolveHarmonic`/`SolveHarmonicT`,
  `Utilities.InitializeForHarmonics`/`savePresentVoltages`/`RetrieveSavedVoltages`,
  `Spectrum.SetMultArray`/`GetMult`, and the per-element `InitHarmonics`/`DoHarmonicMode`.
  Landed in `solution/solution/harmonics.rs` (the sweep over `CollectAllFrequencies`
  / the `harmonic_list`, the in-memory fundamental-voltage save/restore — the original
  spills to a `.dbl` file, this port keeps it on `Solution.saved_node_v` — and a
  per-frequency `SolveDirect` + monitor `SampleAll`), wired into the dispatcher and
  into `Set mode=harmonics` (the Pascal `OK_for_Harmonics` entry runs
  `InitializeForHarmonics`). **Foundational pieces already in place (verified, not
  re-ported):** the frequency-dependent Y rebuild — Line/Transformer/Reactor/Capacitor
  all already scale YPrim by `sys.frequency/base_frequency` (incl. the GIC `<0.51 Hz`
  branches), the monitor harmonic-mode *sample body* (freq/harmonic in the time
  slot), and `solve_direct`'s `is_harmonic_model` PC-injection arm. **Real fixes:** the
  `harmonic = frequency/fundamental` value (was hard-pinned `1.0`); `Spectrum`'s
  deferred `MultArray`/`GetMult`; the VSource `GetVterminalForSource` harmonic branch
  (the source is a short at harmonics under `defaultvsource`); the Load
  `InitHarmonics` (capture `HarmMag`/`HarmAng` from the fundamental `FPhaseCurr`) +
  `DoHarmonicMode` (the ideal harmonic current source) + the **harmonic Load YPrim
  `%SeriesRL` split** (`CalcYPrimMatrix`'s harmonic branch — a parallel R-L part with
  `Y.im /= h` and a series R-L part with `Z.im *= h`, *not* the naive
  `Yeq; Y.im /= h`). The spectrum is resolved + snapshot-cloned at edit-completion
  (the Fuse-curve pattern — VSource/Load report their default/explicit `spectrum=`
  name via `harmonic_spectrum_name`, the executive clones the `SpectrumObj` in). The
  `MakeLike` of both also now copies the (previously-dropped) spectrum name — a latent
  gap that was harmless until the spectrum became load-bearing. **NOT_PORTED / deferred
  (each a loud abort, never silent):** Generator/PVSystem/Storage harmonic injection
  (the voltage-source-behind-reactance family) → step 2 (a `guard_unported_harmonic_der`
  refused the solve loudly if any was enabled — **now landed + the guard removed in
  step 2**). The monitor harmonic *header* names
  (`Freq`/`Harmonic`) ride on the Phase-6-deferred monitor-reset-on-mode-change → step
  3. Isource is not ported in this crate (no source-current harmonic family).
  - **Gate:** 3 targeted oracle-pinned goldens (`gen_phase7.py` + `golden_phase7.rs`):
    `harmonics_load_h5` / `harmonics_load_h7` (the Load current-source family + the
    frequency-scaled Line/source Y at the 5th/7th — node V + Load/Line I/P + the Line
    YPrim entry-by-entry, all at the harmonic frequency, matched the oracle 1e-6) and
    `harmonics_vsource` (the VSource harmonic injection from a custom spectrum, linear
    load isolating the source path). Plus **3 exec smoke tests** (the 5th injects
    exactly 20% of fundamental; `DoAllHarmonics` sweeps the full spectrum; the
    Generator deferral aborts loudly), **3 `Spectrum` unit tests** (`SetMultArray`'s
    fundamental-rotation + nearest-0.01 `GetMult` + the pre-`EndEdit` zero), and **1
    Load unit test** (`harmonic_yprim_uses_series_rl_split_not_naive_yeq` — pins the
    YPrim split entry-by-entry *and* discriminates it from the naive path, the offline
    backstop for the bug the golden caught). **Corpus stays 84** (migration is step 3).
    lib **639 → 646**.
  - **Real bug found + fixed (the golden caught it, the smoke test didn't):** the Load
    YPrim in harmonics mode was the placeholder `Yeq; Y.im /= h`, not the `%SeriesRL`
    series/parallel split — a ~40% error in the load admittance that a network solve
    dilutes to ~0.14% on the node voltages, so the loose smoke-test voltage check
    passed while the 1e-6 oracle golden failed. The `harmonic_yprim_*` unit test is the
    direct offline pin so a regression can't slip the golden again. (Also corrected a
    `StickCurrInTerminalArray` sign inversion in `DoHarmonicMode` — the Rust
    `stick_curr` is a 1:1 of the Pascal helper, *not* a negated form — which a 180°
    voltage flip in the golden surfaced.)
  - **audit-tests follow-up:** verdict — the four *primary* paths (Load injection,
    the YPrim split, the VSource injection, the frequency-scaled Line Y) were
    strongly oracle-pinned (1e-6 + Line YPrim entry-by-entry, guarded in `must`), but
    several *reachable default* sub-paths shipped without an oracle gate. Closed the
    two Major gaps + the motor minor with **3 new oracle goldens**: `harmonics_doall`
    (the **default** `DoAllHarmonics` sweep — a `mode=0` Load monitor pins V/I on
    **every** swept harmonic, 7 samples × 16 channels, via `compare_monitor` — the
    real distortion output, not just the last harmonic), `harmonics_doall_t` (the
    same sweep through `SolveHarmonicT`, previously **untested** — its final NodeV is
    the fundamental so the monitor is the gate), and `harmonics_load_motor_h5` (the
    `puXharm>0` motor series-reactance YPrim branch the default-load goldens skip).
    Plus 3 exec tests: `second_harmonic_solve_restores_saved_voltages` (the re-entrant
    `RetrieveSavedVoltages` path) and the DER deferral guard now asserted for **all
    three** families (Generator/PVSystem/Storage), not just Generator. lib **646 →
    649**; golden_phase7 53 → 56. **Surfaced-not-fixed (acceptable):** the monitor
    *time-column* header names stay `hour`/`t(sec)` rather than `Freq`/`Harmonic` —
    `compare_monitor` skips those columns (it compares the V/I data channels), so the
    values are pinned now; the cosmetic header rename rides with the
    monitor-reset-on-mode-change (a Phase-6 deferral) in step 3. The
    `harmonic_yprim_*` unit test's `expected` re-derives the split arithmetic (a
    transcription check) — its teeth are the naive-path discriminator + the oracle
    golden anchor; left as-is.
  - **audit-code follow-up:** verdict **faithful, no Critical/Major** — the harmonic
    numeric paths are 1:1 ports (SetMultArray/GetMult, the VSource/Load harmonic
    branches, the YPrim `%SeriesRL` split, the drivers, CollectAllFrequencies) and
    oracle-verified end-to-end; the DER deferral is genuinely loud. Fixed **1
    silent-wrong gap + 1 faithfulness nit**: (1) a **typo'd `spectrum=` name** resolved
    silently to NIL (→ zero harmonic injection, no diagnostic) where the oracle raises
    `#401 …Spectrum: Spectrum object "x" not found.` (probe-confirmed) — the
    edit-completion resolver now pushes that error (`exec/command.rs`), pinned by a new
    `unknown_spectrum_name_errors_not_silent` exec test; (2) `initialize_for_harmonics`
    now early-returns the instant an element sets `solution_abort` (matching Pascal's
    `Exit`). **Surfaced-not-fixed (carry-forward to step 2, each harmless now):** (a)
    `Set mode=harmonics` runs `initialize_for_harmonics` *after* `set_mode` commits the
    flags and discards its bool — unobservable in step 1 (the Load init never aborts,
    the DER family is caught by the solve-time guard, and a real abort sets
    `solution_abort` which the next `Solve` honors), to be aligned with the Pascal
    `OK_for_Harmonics` ordering when step-2 DER `InitHarmonics` (which *can* abort)
    lands; (b) the zero-harmonic-spectrum *definition* error (Pascal 65001) loses only
    its message — the numeric behavior is faithful (both engines build no `MultArray` →
    zero injection), a pre-existing `end_edit`-has-no-error-sink limit; (c) the monitor
    harmonic *header* names (`Freq`/`Harmonic`) ride with monitor-reset-on-mode-change
    in step 3 (the sample *body* + the data-channel gate are done). lib **649 → 650**.
- **step 2 — the Thevenin DER family (Generator + PVSystem + Storage).** Pascal
  `T{Generator,PVsystem,Storage}Obj.InitHarmonics`/`DoHarmonicMode` + the harmonic
  `CalcYPrimMatrix` branch. Each DER is a **voltage source behind its subtransient
  reactance** (Generator: `Xd"`; PVSystem/Storage: `%R`/`%X` → `RThev`/`XThev`):
  `InitHarmonics` sets `Yeq := Cinv(...)` (the L-N harmonic admittance) and captures
  the Thevenin reference `Vthevharm = |Va − I·Z|` / `ThetaHarm = ∠(…)` from the present
  **fundamental** terminal current (`compute_iterminal`); `DoHarmonicMode` injects
  `InjCurrent = YPrim · (SpectrumObj.GetMult(h)·Vthevharm rotated by ThetaHarm and
  −120°/phase)`, leaving `IterminalUpdated=false` so the terminal current is derived
  from the network (unlike the Load current-source family). The harmonic
  `CalcYPrimMatrix` branch stamps `Y := Yeq` (Generator: `EPSILON` if off; **not**
  negated like power flow), delta `/3`, `Y.im /= FreqMultiplier`. **Real correctness
  fix on the way in:** `SetNominalGeneration` recomputed `Yeq`/`GenON`/`Pnom`
  unconditionally — Pascal guards each with `if not (IsDynamicModel or IsHarmonicModel)`,
  else the harmonic `Yeq` from `InitHarmonics` is clobbered before the YPrim build; the
  port now mirrors the guard (PVSystem/Storage `SetNominalDEROutput` already early-returned
  in those modes). New struct fields `v_thev_harm`/`theta_harm`/`{gen,pv_system,storage}_fundamental`
  + a resolved `spectrum_obj` (Generator default `defaultgen`; PVSystem/Storage force
  `SpectrumObj := NIL`, so they inject only from an explicit `spectrum=`). The
  `guard_unported_harmonic_der` loud abort and its 3 deferral exec tests are removed
  (replaced by 3 injection exec tests). Gate: 3 oracle-pinned goldens
  `phase7/harmonics_{generator,pvsystem,storage}_h5` (`gen_phase7.py` + `golden_phase7.rs`)
  — node V + DER/Line I/P + the Line YPrim entry-by-entry at the 5th, matched the oracle
  1e-6 (golden_phase7 56 → 59, + a delta golden in the audit-code follow-up → 60).
  **Corpus stays 84** (migration is step 3). lib **650 → 653**.
  - **The `capture_element` order quirk (golden generator, not a port bug — confirmed
    with the user "don't port the oracle's bug"):** the oracle's `CktElement.Powers`,
    when queried *after* `Currents`, returns `V_harmonic · conj(I_fundamental)` for a
    Thevenin DER in harmonics mode (a stale-`Iterminal` cross-product, physically
    meaningless), but `Powers`-first gives the consistent `V_harmonic · conj(I_harmonic)`
    the engine produces when asked directly. The Rust harness computes power single-pass
    (`node_v · conj(Iterminal)`, always consistent), so `gen_checkpoints.capture_element`
    now reads `Powers` **before** `Currents` to pin the oracle's consistent answer. The
    swap is **identical for every non-Thevenin-DER-harmonic element** (their `Iterminal`
    is stable across the two queries), so all existing goldens are byte-unchanged
    (`git status` confirmed only the 3 new files appear after a full `gen_phase7.py` regen).
    No `TODO(compat)` — there is no engine inexactness, only a query-order capture choice.
    **Investigated & RESOLVED** — verdict: a genuine long-standing OpenDSS engine bug
    (stale-`Iterminal`), reproduced identically on the pinned oracle and **real EPRI
    OpenDSS through v11.0.0.1** (also dss-python 0.16.0b2 / altdss); not fixed upstream;
    mechanism pinned to `PCElement.pas` × `CktElement.pas`. Workaround correct; no Rust
    change. Full write-up, IEEE-1459 proof, repro script, and an OpenDSS issue draft live
    in **`investigations/oracle-powers-currents-harmonic/`** (git-ignored, local only).
  - **audit-tests follow-up:** verdict **sound + strictly additive** — the 3 new goldens
    are a genuine oracle pin (full-corpus regen reproduces them byte-for-byte against the
    pinned 0.15.7/0.14.5 oracle, non-degenerate injection), the `capture_element`
    Powers-first swap is non-weakening (byte-identical for every existing golden, no
    tolerance touched, still compared vs the oracle not Rust-vs-Rust), and the
    deferral→injection conversion is correct. Fixed the **3 Minor gaps**: (1) the exec
    injection tests were smoke-only (`vmax > 0 && < 12.47e3`, a ~4600× envelope) →
    tightened to a genuine small-distortion bracket (`0.1 V ≤ vmax < 5% of L-N nominal`);
    (2) **no oracle-independent offline discriminator** for the DER harmonic injection (the
    step-1 Load had `harmonic_yprim_uses_series_rl_split_not_naive_yeq`) → added
    `harmonic_yprim_is_{subtransient,thevenin}_admittance_not_powerflow` to
    generator/pvsystem/storage `tests.rs` (each pins the harmonic `CalcYPrimMatrix` Y=Yeq
    branch entry-by-entry and discriminates it from the power-flow stamping, oracle-free);
    (3) the 3 new scenarios were missing from the `golden_phase7.rs` `must` anti-silent-drop
    guard → added. lib **650 → 653**.
  - **audit-code follow-up:** verdict **faithful 1:1 port, no Critical/Major** — the harmonic
    math (`InitHarmonics`/`DoHarmonicMode`/the `CalcYPrimMatrix` Y=Yeq branch) matches Pascal
    line-for-line for all three elements, the `SetNominalGeneration` restructure is
    byte-identical on the power-flow path and correctly Pascal-guarded in harmonic/dynamic
    mode, the dispatch order (harmonic before GFM) and the `capture_element` swap are
    confirmed faithful. **No code-behavior fix needed** — the two Minor findings are
    comment-honesty items (settled): (1) Pascal dispatches `DoDynamicMode` first for
    `IsDynamicModel`, which is WP7.7 and **unreachable today** (the Dynamic/FaultStudy/
    MonteFault solve modes error "Unknown solution mode" before any element injects —
    verified in `dispatch.rs`), so only the harmonic check is ported; the dispatch-site
    comment now says so. (2) The "unused shape `factor`" skip comment was too absolute for
    dynamic mode (the GENERALTIME arm *can* call a shape mult) — reworded to note it is
    exactly unused in harmonics and unobservable (ShapeFactor unread) in dynamics until
    WP7.7. **Added the one high-value recommended coverage:** a **delta** generator harmonic
    golden `phase7/harmonics_generator_delta_h5` (the delta `Y/3` YPrim stamping + the
    no-neutral injection buffer — paths the wye goldens never reach), matched the oracle 1e-6
    (golden_phase7 59 → 60). **Surfaced-not-fixed (acceptable):** an OFF-generator harmonic
    case (`gen_on=false` → `Vthevharm=0` + the `EPSILON` YPrim stamp) stays golden-uncovered
    (no injection → only "no distortion"; the EPSILON branch is read by the offline
    discriminators' sibling path), and a FaultStudy+Generator parity golden is impossible
    until the FaultStudy solve mode lands (WP7.9). The Generator `do_harmonic_mode`'s
    `SpectrumObj.GetMult` NIL-guard (`.unwrap_or(ZERO)`) is *more* defensive than Pascal (which
    would AV) — kept (reproducing an AV would be wrong; `defaultgen` always resolves).
- **step 3 — the monitor harmonic header + the `Set mode=` reset + the corpus burn-down.**
  Two faithful behavioral ports plus a documentation-honesty corpus refresh.
  - **Monitor harmonic header (`ClearMonitorStream`, Monitor.pas l.709).** The two leading
    time columns are now labelled `Freq`/`Harmonic` when the solution is in harmonics mode,
    else `hour`/`t(sec)` — `is_harmonic` threaded through `reset_it`/`recalc`/
    `clear_monitor_stream`. The canonical relabel path is the `Set mode=harmonics` reset
    (it carries the live `IsHarmonicModel`); `end_edit`/`do_action` build non-harmonic
    (parse/edit time is normally at fundamental — harmonics mode requires a prior solve and
    monitors are defined before it; `recalc_element_data` is **dead** in this architecture,
    never invoked). The one residual divergence — editing/clearing a monitor *after*
    entering harmonics — relabels only on the next reset; it is **oracle-invisible** (the
    C-API `Monitors_Get_Header` returns `Header.Strings[k+2]`, CAPI_Monitors.pas l.414,
    stripping the two time columns) and self-healing, so it is gated **offline** — the data
    channels stay exactly as the live/golden gates pin.
  - **Monitor + meter reset on mode change (`Set_Mode` tail, Solution.pas l.2132).** The
    `Set Mode=` handler now runs the full Pascal tail — `MonitorClass.ResetAll` +
    `EnergyMeterClass.ResetAll` ahead of the existing fault/control resets — so every mode
    change clears the monitor/meter buffers (the missing piece flagged as a Phase-6 no-op in
    the old `set_cmd.rs` comment). The **oracle already does this**, so adding it can only
    tighten Rust↔oracle agreement, never loosen it (the 84 live cases + 10 harmonic goldens
    are byte-unchanged — the reset is benign-empty in every gated deck, and the relabel only
    touches the gate-skipped time columns). It is also what rebuilds the harmonic header.
  - **Gate.** 3 new tests: `harmonic_mode_relabels_monitor_time_columns` (a real harmonic
    deck — header[0..2] flips `Freq`/`Harmonic` on `set mode=harmonics`, data channels
    unchanged, fundamental + 5th samples, `dbl_hour == [60, 300]`),
    `mode_change_resets_monitor_buffer` (a daily(2) run's 2 samples are cleared by the next
    `set mode=`), and `mode_change_resets_meter_registers` (the meter half — a daily run's
    kWh is zeroed by the next `set mode=`). lib **653 → 656**; golden_phase7 stays **60**,
    `solvable_now` stays **84**.
  - **Corpus burn-down — 0 migratable, maximal (honest refresh).** All 4 corpus harmonics
    decks re-probed: each is blocked by a Phase-8 report command or an unported feature, not
    by harmonics. `Version8/Distrib/Examples/HarmonicsTMode/IEEE_519.DSS` and
    `…/HarmonicsVariableLoad/IEEE_519.DSS` now run the **full `harmonicT` solve clean**
    (SwtControl + `Open` + `set mode=harmonicT` all ported, WP7.2/WP7.6; both CONVERGE) —
    blocked only by the trailing `export monitor` (`show monitor` is a Phase-8 no-op stub
    that returns without erroring). Their **stale** `unsupported_class=Swtcontrol` tags
    (SwtControl ported in WP7.2) were refreshed to the genuine
    `unsupported_command=Export; deferred=phase8-reporting`. `FreqScan/Run_Scan`
    needs `Isource` (no source-current harmonic family in this crate) + Plot/Export;
    `NEVTestCase/Run_NEV` needs FaultStudy (WP7.9) + Export (and does not converge at
    fundamental — a separate item). **Corpus stays 84**, consistent with the WP7.5 step-4
    policy (Export-blocked cases stay skipped until Phase 8); `COVERAGE.md` unchanged.
    The harmonics solve itself is fully gated by the 10 targeted `phase7/harmonics_*` goldens.
  - **audit-code follow-up:** verdict **substantially faithful, no Critical/Major** — the
    harmonic header (`ClearMonitorStream`), the `Set_Mode` reset tail (exact order + scope,
    header sees `IsHarmonicModel=TRUE`), and the `reset_all_meters` reborrow are 1:1, and the
    new reset makes Rust *more* oracle-faithful (the oracle resets on every mode change).
    Fixed **2 doc-honesty Minors**: (1) the `end_edit`/`do_action` comments over-claimed "the
    solution is never in harmonics mode here" — corrected to state the **real residual
    divergence** (editing/clearing a monitor *after* entering harmonics relabels only on the
    next reset; offline-only — the C-API strips both time columns — and self-healing), keeping
    the `false` (the `DssObject` edit surface carries no solution state); (2) the now-stale
    `set_mode.rs` doc ("monitor/meter resets are Phase 6 no-ops") updated to the real reset
    tail; plus the `harmonics.rs` abort comment corrected (no ported `init_harmonics` sets
    `solution_abort`; the DER early-`Exit` paths are returns, and the disk-spill abort is N/A
    to the in-memory save). **Surfaced-not-fixed (carry-forward, unreachable):** `set_cmd.rs`
    discards the `initialize_for_harmonics` bool and runs the reset tail unconditionally, where
    a Pascal `OK_for_Harmonics`-false would `Exit` (no mode change, no reset) — but no ported
    DER aborts, so it is unreachable; a code note marks it to honour when the DER abort path
    lands (extends the step-1 ordering carry-forward (a)).
  - **audit-tests follow-up:** verdict **sound + strictly additive** — both new exec tests
    are genuine regression pins (the auditor reverted each code change and confirmed the
    matching test fails), the "0 migratable" claim was re-verified by running all 4 corpus
    harmonics decks (HarmonicsTMode/VariableLoad CONVERGE through the full `harmonicT` solve,
    blocked only by the trailing `Export`), and **no golden was regenerated** (the new resets
    moved no oracle-pinned value). Fixed the **3 Minors**: (1) `Show` is a Phase-8 **no-op
    stub** (returns without erroring, `command.rs:63`), not a blocker — the two refreshed tags
    + notes corrected to `unsupported_command=Export` only (the genuine remaining error); (2)
    the harmonic time-slot *value* was ungated — strengthened `harmonic_mode_relabels_*` to
    pin `dbl_hour == [60, 300]` (the swept `Freq` held in slot 0, Pascal `TakeSample`
    l.1202-1206), the one offline surface for it; (3) the meter half of the `Set_Mode` reset
    had no direct test — added `mode_change_resets_meter_registers` (a daily run's kWh is
    zeroed by the next `set mode=`). lib **655 → 656**. **Noted (out of step scope):** an
    online `dblFreq` pin (`compare_monitor` ignores `dbl_hour`) and the stale `Show` in the
    untouched `FreqScan`/`NEVTestCase` tags (whose primary blockers — `Isource`, FaultStudy
    — are correct) are left for a future corpus-hygiene / harness pass.
