# Standalone BUG work packages (livectx, DynExp)

> Moved verbatim from `STATUS.md` on 2026-08-05 (STATUS.md history archiving);
> order preserved, nothing rewritten.

### BUG WP livectx — thread the live ctx into every recalc; kill `default_recalc_ctx` substitution (2026-07-25)

Follow-up to gicfix, on `bug-livectx` (base `update` @ gicfix merge). User
directive: "всё замаскированное надо пофиксить — неправильно, значит неправильно."
Every production site that fed a synthetic `default_recalc_ctx()` snapshot into a
routine whose Pascal twin reads the live `ActiveCircuit.Solution` now threads the
live `sys_ctx`. Full gate green; corpus + goldens bit-neutral (the masks were
real). Success metric met: `rg default_recalc_ctx crates/dss-core/src --glob
'!*tests*'` shows only the 3 (now test-only) definitions — zero production call
sites.

- **A — `end_edit(&mut self, sys: &SysCtx)`.** Widened the `DssObject::end_edit`
  trait method (all ~40 impls; the 6 PC classes Load/Generator/WindGen/Storage/
  PVSystem/IndMach012 consume it as `self.recalc(sys)`, every other class ignores
  it). The executive builds the live snapshot from `circuit` (same `sys_ctx`
  source the gicfix pos-seq/frequency sync uses — no parallel mechanism) at
  command.rs (New/Edit), make_pos_seq, and json_import. The 6 PC constructors no
  longer call `recalc(&default_recalc_ctx())`; the executive runs the Create-time
  live recalc in `create_object_no_edit` via the shared `recalc_pc_create` helper
  (Pascal `T<PC>Obj.Create → RecalcElementData`), so a property setter / reader
  during the edit block sees Pascal's live value.
- **B — user-model plumbing.** Threaded the live `sys` through `apply_user_model_load`
  (load/edit) and `set_variable` (both trait methods widened) into the WASM
  callbacks + `dyn_rec_from`/`snapshot` for Generator/Storage/PVSystem, matching
  Pascal handing the guest live `ActiveCircuit.Solution` pointers. All 18 WASM
  twin gates (`wasm_usermodels.rs`, r4133 oracle-of-record) stay green — the gates
  parse at a fresh circuit where live == parse-default, so bit-neutral.
- **C — WindGen WTG3 seed.** Dropped the hardcoded `recalc_element_data(0.0, 0.0)`
  seed in `Wtg3Model::new`. Pascal `TGE_WTG3_Model.Create` reads live
  `DynaData^.h/t` = `ActiveCircuit.Solution.DynaVars` (WindGen.pas:1018); the
  WindGen owner's `recalc(sys)` re-seeds with `sys.dyna_h/dyna_t`, which the
  executive runs at create/end_edit.
- **D — mask re-verification.** The corpus gate (514 cases, both channels) +
  byte-exact goldens are bit-neutral → the recalc-derived fields (yeq*/pnominal/
  zs/…) have no runtime reader between end_edit and the next live recalc for any
  of the 6 classes (masks real; no corpus pin needed). The lone default-value
  observable that surfaced — the JSON `sample_for_defaults` object, which the
  defaults-introspection path builds via the factory and never routes through the
  executive — is fixed by recalc'ing the sample with the parse-time default in
  `schema_class_def`, exactly as Pascal `cls.NewObject` Create does; restores the
  byte-exact schema goldens.
- **E — Monitor BaseFrequency 60.0 (6th proven upstream bug).** Added a
  `TODO(compat)` at the Monitor override in `create_object_no_edit`:
  `TMonitorObj.Create` hard-pins `Basefrequency := 60.0` (Monitor.pas:472 ==
  r4133:552), overriding the base-class `Fundamental` inherit. Physical
  consequence documented: a mode-4 monitor in a 50 Hz circuit feeds 60 into
  `FlickerMeter` (`fBase`), selecting the wrong IEC 61000-4-15 120V/60Hz lamp
  curve instead of the 230V/50Hz set (Pstcalc.pas:609-626) unless `basefreq=50`
  is set. Reproduced 1:1 (both oracles pin 60.0); dedicated pin test
  `monitor_basefreq_pins_60hz_upstream_bug`. Clean fix deferred to Stage F.

**Settler pass (2026-07-25).** Both audits (code + tests) returned ACCEPT-clean;
every finding settled empirically:

- **Masks (D), empirical enumeration.** The audits' mask-safety proof was
  coverage-based (corpus + goldens bit-neutral); confirmed structurally for all
  six PC classes: `calc_yprim(sys)` re-runs `set_nominal_*(sys)` at the top for
  Generator/Load/Storage/PVSystem/WindGen, and IndMach012 (by design, matching
  Pascal `CalcYPrim` which does *not* self-recalc) re-derives via `inj_currents →
  set_nominal_power(sys)` when `loads_need_updating`. So an `end_edit`-derived
  nominal/`yeq*` is always overwritten by the LIVE ctx before any Y/current read —
  no leaky reader beyond the one already found+fixed (JSON `sample_for_defaults`,
  which uses the sanctioned no-circuit `SysCtx::parse_default()`, not
  `default_recalc_ctx`). Masks real; no corpus pin.
- **Positive divergent-ctx guard added (settles both audits' lone finding).** The
  behavioral delta was asserted nowhere — a bit-neutral revert to
  `default_recalc_ctx()` at any PC create/edit site would pass the whole suite. New
  `exec/tests/live_ctx.rs`: move the live `GenMultiplier` off 1.0, then `New` /
  `Edit` a Generator and read the create-/edit-time `p_nominal_per_phase` before any
  solve. Guards the executive threading at both `recalc_pc_create` and `end_edit`;
  a re-substituted default ctx now fails a test, not just the `rg` metric.
- **PVSystem (audit-code minor).** Accepted — its 4 sites were production and the
  success metric requires them; extension correct.
- **Monitor 60.0 investigations decision (brief-E).** Settled: NO separate
  `investigations/` report; `TODO(compat)` + pin + STATUS + the CLAUDE.md
  Known-upstream-bugs list is the durable record. Rationale: (1) `investigations/`
  is **gitignored/local-only** (not even present in a worktree) — an untracked note
  is lost on worktree removal and never ships; the git-tracked canonical index is
  CLAUDE.md, now updated Five→Six with the Monitor bullet. (2) The reproduced-1:1
  rule prescribes exactly `TODO(compat)` + golden/pin, which is done. (3) The full
  source+physics verification already lives inline in the greppable `TODO(compat)`.
  (4) The five deep-dive reports cover nuanced dispositions (not-reproduced / UB /
  gated-around); Monitor 60.0 is a plain hardcoded constant with one fully-traced
  consumer and does not rise to that threshold.
- **Environmental incident (RESOLVED 2026-07-25 — shadowed compiler, no repo
  change needed).** Mid-WP, an *adjacent session's* provisioning script ran
  `pacman -Sy --noconfirm ... mingw-w64-x86_64-rust`, dropping a GNU rustc/cargo
  1.97.0 (LLVM 22.1.8) into `C:\msys64\mingw64\bin` — which precedes
  `C:\Users\Admin\.cargo\bin` in the user PATH. Bare `cargo` silently switched
  from the project's real toolchain (**rustup nightly-x86_64-pc-windows-msvc**)
  to the MSYS2 GNU one. Three symptoms, all environmental: (1)
  `golden_reports::dump3_debug_matches_oracle` failed on ONE field — the
  near-cancellation imaginary residual of a real value (~1e-22 abs, 10 sig figs
  agreement) — GNU/LLVM-22 codegen vs MSVC last-ULP, proven by the same HEAD
  passing pre-install and failing post-install; (2) the dss-core merged-doctest
  runner crashed silently (incompatible runtime DLLs from PATH); (3) fresh
  build-script exes hit 0xC0000005 (GNU rustc linked via Strawberry Perl's gcc).
  The settler triaged this mid-incident as a "toolchain update" and its notes
  (commit `5a22e5f`) recommended pinning/tolerance — superseded by this record:
  the MSYS2 rust package was removed the same day, `cargo` reverted to rustup
  nightly MSVC, and the **full gate went green with zero repo changes**
  (golden_reports 207/207 incl. dump3, doctests ok). The WP itself was already
  exonerated (audits ran green pre-incident on the real toolchain). Standing
  guard: before trusting a gate run, `(Get-Command cargo).Source` must be
  `C:\Users\Admin\.cargo\bin\cargo.exe`; never regenerate goldens under a
  shadowed compiler.

Gate after the settler edits, re-run on the restored rustup MSVC toolchain
(2026-07-25, coordinator): `cargo fmt --all --check` green, `cargo clippy
--workspace --all-targets -- -D warnings` green, `cargo test --workspace` green
**in full** — golden_reports 207/207 (incl. `dump3_debug`), corpus_gate 25/25
both channels, lib 1269 incl. the new `live_ctx` guards, golden_schema 9,
wasm_usermodels 18, doctests ok. Corpus pristine; source-integrity 186 `.pas`.



Five related holes in the `< 0.51 Hz` GIC gate and the pos-seq collapse, all on
`bug-posseq-gic`. Empirically settled against the pinned oracle (dss-python
0.15.7) and the r4133 DLL; full gate green; corpus pristine.

- **Hole 1 (real gap): Transformer GIC branch was never ported.** `TTransfObj.
  CalcY_Terminal` (`Transformer.pas:1879`) builds a resistance-only Y_Terminal
  below 0.51 Hz via `GICBuildYTerminal` (`:1823`) — `1/RdcOhms` per winding, no
  inter-winding coupling, empty `Y_Term_NL`, anti-float adder as a real
  conductance (`-Y_PPM`). The port had NO `<0.51` branch. Ported loop-for-loop
  (`transformer/yterminal.rs::gic_build_y_terminal`, twin of the working
  AutoTrans one). `CalcYPrim` still stamps the empty `Y_Term_NL` and runs
  `AddNeutralToY` unconditionally — matches Pascal `:1202-1204`. New corpus deck
  `asymmetric/transformer/transformer_gic.dss` (twin of `autotrans_gic.dss`,
  engines=both) + unit test `gic_build_y_terminal_rdc_only_below_051hz`. Oracle
  feature-check: rdcohms 2.5→YPrim 0.4 (=1/2.5), 5.0→0.2.

- **Hole 2 (documented benign divergence — now eliminated).** The gate had been
  reconstructed as `freq_mult·base_frequency < 0.51` (the true `Solution.
  Frequency` was out of scope), exact on the CalcYPrim path but diverging on the
  `RecalcElementData → calc_y_terminal(1.0)` path (rebuilt at 60 Hz, not the live
  frequency). This closes out the WPG.15 finding-1 "documented benign divergence"
  (the long comment at `auto_trans/yterminal.rs:144-156`, audited 2026-07-09):
  the comment is removed and `calc_y_terminal(freq_mult, frequency)` now takes the
  frequency explicitly — CalcYPrim passes `sys.frequency`, RecalcElementData passes
  a cached `live_frequency` synced by the executive at New/Edit and refreshed by
  CalcYPrim each solve (the Rust stand-in for the Pascal global). Bit-neutral on
  every `≥0.51 Hz` path (all existing goldens/corpus green). The recalc-path
  frequency itself stays observationally masked (CalcYPrim always rebuilds Y_Term
  at the solve frequency); bonus deck `asymmetric/autotrans/autotrans_gic_edit.dss`
  pins the AutoTrans edit-at-GIC path but does not isolate the gate.

- **Holes 3/4/5 (real, observable): Line pos-seq collapse faked `false` at the
  three edit-time recalc sites.** `TLineObj.RecalcElementData` reads the live
  `ActiveCircuit.PositiveSequence` (`Line.pas:1085`) and collapses R0:=R1, X0:=X1,
  C0:=C1 whenever it runs — immediately in the `phases=` side effect (`:628`, Hole
  3), `FetchLineCode` (`:572`, Hole 4) and `Create` (`:1001`, Hole 5). All three
  Rust sites called `recalc(false)`. **Settled empirically first** (oracle probe,
  a `CktModel=Positive` circuit): the collapse is VISIBLE on `? Line.x.r0`
  readback *immediately* after each trigger — so the fix must be edit/parse/create
  time, NOT a deferred `sym_components_changed` revisit (which would leave r0
  un-collapsed until solve). `recalc` keeps its `bool` parameter; the three sites
  now pass a cached `Line.positive_sequence`, the executive-synced stand-in for the
  global (set at edit_active start and at construction; the defaults case re-runs
  the collapse post-construct via `recalc_pos_seq`). Rust readback now matches the
  oracle exactly (phases/linecode 0.1/0.2/3; ctor r0=r1=0.058; non-PS keeps
  r0=0.5). Three corpus decks `asymmetric/line/line_posseq_{ctor,linecode,phases}_asym.dss`
  (engines=both) with `r0/x0/c0` probes pin the readback on both channels; the
  full-model YPrim compare pins the effect. `TLine.EndEdit` (`:782`) confirms Line
  never recalcs on impedance edits, so that path (already `sym_components_changed`)
  is consistent — NOT a fourth hole.

- **Rider (doc-only, separate commit): stale "Phase 7/8/unported" comments** wiped
  where the sweep proved them false: `monitor/sample.rs`, `capacitor/mod.rs`,
  `transformer/{mod.rs,yterminal.rs}` (also removed by Hole 1), `exec/construct.rs`,
  `json/schema/mod.rs`, `monitor/mod.rs`. `schema_skeleton`/`extract_schema_skeleton_json`
  (zero callers) flagged for an R3 dead-code pass, not deleted here.

- **R3 candidate (noted, not touched):** `CktElement::recalc_element_data(sys)` is
  never dispatched by the executive for these classes (dead code).

- **Settler pass (2026-07-25, both audits settled empirically — no code defect).**
  audit-code + audit-tests raised four items; all disposed non-defect / intentional,
  none a tolerance change:
  - *Transient revert seen by audit-code (Question):* the sibling audit-tests agent
    ran fix-revert experiments in this shared worktree; it restored clean. HEAD
    `a6a3d44` is stable — `git status` clean, `grep -c gic_build_y_terminal
    transformer/yterminal.rs = 4`, `cargo check -p dss-core` clean (a reverted tree
    would not compile: `command.rs` calls the reverted-away `set_live_frequency`).
    Environmental, not a HEAD defect.
  - *Dead code left in place (Minor):* `recalc_element_data`, `schema_skeleton`,
    `extract_schema_skeleton_json` — brief directs flag-for-R3, not delete; already
    recorded above.
  - *`autotrans_gic_edit.dss` gates no new Hole-2 regression (Minor):* confirmed —
    the recalc-path frequency is masked (CalcYPrim always rebuilds Y_Term at the
    solve frequency), so reverting the Hole-2 fix leaves this deck green (auditor's
    revert experiment). Kept deliberately: it is a valid converging, two-process
    bit-identical case that exercises the Edit→RecalcElementData→calc_y_terminal
    edit-at-GIC path (distinct from the fresh-CalcYPrim path in `autotrans_gic.dss`)
    and is feature-sensitive on rdcohms. The deck header + Hole-2 bullet already
    state it does not isolate the gate — honest coverage, not a fake gate.
  - *r4133 channel (Question):* re-ran the FULL gate here — both `capi_v0145` and
    `r4133` legs green on all five new decks (corpus gate compiles + solves the full
    manifest against each case's channels; epri-worker built by `cargo test`).

Root-caused and resolved the `controls/regcontrol/regcontrol_idle.dss`
`defer_ledger` occurrence (ORPHANED_GAPS §1.9). The 8.8 % MV-bus divergence
(port MV.1 = 7030.24 V / tapnum 0 vs r4133 7650.08 V / tapnum 15) was a **port
bug relative to r4133**, not a ledgerable divergence.

- **Cause.** The port faithfully reproduced dss_capi 0.15.x's idle no-load-zone
  test written as an **OR** — `(FwdPower ≥ RevThr) or (FwdPower ≤ FwdThr)` — a
  **tautology** under the default symmetric ±100 kW band (every value is
  ≥ −100 kW OR ≤ +100 kW), so an idling reg NEVER taps for any load. EPRI
  r4088/r4133 use the correct **bounded AND** (`RegControl.pas:1218`):
  `(FwdPower ≤ FwdThr) and (FwdPower ≥ RevThr)` — idle only when the
  through-power is *inside* [RevThr, FwdThr].
- **Dated** (git `.inputs/dss_capi_with_git`): the OR is `8a898cba` "port SVN
  r4086" and is STILL OR at the 0.15.x branch tip (`e936d210`); r4088 & r4133
  have AND; r3723 & 0.14.5 have no idle feature.
- **Live r4133 probes** (`epri-worker`): with the default band idle=yes taps
  identically to idle=no (tapnum 15, MV.1 7650.08 V — FwdPower ≈ 7438 kW, far
  outside the band). Widening `fwdThreshold` to 1e6 kW so the bounded zone
  brackets the ~7.03e6 W throughput makes r4133 *idle* to exactly the port's old
  MV.1 7030.24 V (probe D) — a direct proof of the bounded-AND mechanism.
- **Resolution (verdict b — adopt r4133).** `idle` is 0.15-only (0.14.5 rejects
  #110; the one pinned idle golden `props/regcontrol.json::regcontrol_idlezones`
  pins property readback only, no no-load-zone solve). The port adopts r4133's
  AND (`reg_control/control_loop.rs`) + a regression unit test pinning the
  high-power (out-of-band) tap. Flipped the case from `defer_ledger` to live
  `engines:"r4133"` gating — tap/voltage/full-model now match r4133 exactly.
- **Residual ledgered.** The revThreshold/fwdThreshold **getter convention**
  differs: r4133 returns the `InitPropertyValues` display strings
  (revThreshold '100', fwdThreshold '') decoupled from the signed internal,
  while the port reports the signed effective thresholds in kW (−100 / +100 —
  which matches dss_capi 0.15.x, pinned by `regcontrol_idlezones`). Definitional,
  not physics → exact-pair `probe` divergence
  `r4133-regcontrol-idle-threshold-display` (cause `regcontrol-idle` rewritten).
  Population lock regenerated (one-digest diff: defer 1→0 + ledger digest).
- **Gate green** (`cargo fmt`/`clippy`/`test --workspace`, wall ≈ 340 s;
  dss-core lib 1247 + corpus_gate 25 incl. the full unified gate). Corpus
  pristine.
- **Audit settle (2 opus xhigh audits, both PASS — no weakening).** Three low
  findings settled:
  - *(F1, both audits — fixed)* The `regcontrol_idle.dss` header comment still
    described the removed OR behavior ("tap stays at neutral, |V|≈0.864 pu").
    Rewritten to the adopted r4133 bounded-AND semantics (deep under-voltage →
    large through-power → outside the ±100 kW no-load band → taps to tapnum 15 /
    MV.1 ≈ 7650.08 V, identical to idle=no). Comment-only; solver ignores it.
  - *(F2, audit-tests — fixed)* `TESTING.md` ledger count was stale (25→26
    entries; the r4133 divergence sub-count 20→21). Updated; the "20 documented
    causes" line is unchanged (the new entry reuses the pre-existing
    `regcontrol-idle` cause_ref, present in the causes dict at both base and head).
  - *(F2, audit-code — deliberate NON-fix, rationale recorded)* `end_edit`
    (`accessors.rs:347-348`) mirrors the RevThreshold-only legacy band with
    `Fwd := abs(Rev); Rev := -Fwd`, faithfully porting **dss_capi 0.15.x**
    (`8a898cba` — whose own comment states *"'abs' added to ensure correct
    behavior (RevPowerThreshold < FwdPowerThreshold)"*, an intentional fix of
    EPRI's inverted-band quirk). r4133 `RegControl.pas:503-506` instead does
    plain `kWFwd := kWRev; kWRev := -kWRev`. **Kept the abs**, NOT changed to
    r4133's negate: (1) pre-existing, not touched by this WP (out of scope — the
    WP adopted r4133 only for the no-load *zone* AND); (2) the two conventions
    are behaviorally **identical** on every existing test — all pinned goldens
    (`props/regcontrol.json`) use positive-revThreshold-only or both-set inputs
    where abs≡negate; they diverge only on an untested NEGATIVE-revThreshold-only
    input, which no deck or golden exercises; (3) the port's abs matches the
    capi015-probed `regcontrol_idlezones` props golden convention (RevThreshold
    signed display), so adopting r4133's negate would REINTRODUCE the inverted/
    empty band that dss_capi deliberately fixed and create an unpinned, ungated
    behavior. Verified empirically: filtered gate (`DSS_GATE_ONLY=
    regcontrol/regcontrol_idle`) green, port matches r4133 live, ledger entry
    hit 2×.

### BUG WP DynExp — reverted the D14 `SolveEq` no-op; port swings, matches both oracles (branch `bug-dynexp`, 2026-07-19)

The DynExp decks' `defer_ledger` said the port matched **neither** surviving
oracle (0.14.5 and r4133 agree; port off both). Root cause: the earlier **D14**
work adopted upstream `2a8bdb78`'s no-op `SolveEq` (an `Exit` before the RHS is
evaluated) to match the retired **non-gating** capi015 (dss_capi 0.15.x), which
**froze** the DynExp state at its `InitStateVars` seed. Both *gating* oracles run
the full evaluator: vendored **0.14.5** `SolveEq` (`DynamicExp.pas:377`) and EPRI
**r4133** `SolveEq` (`:497`) are byte-identical in structure and integrate — the
rotor swings.

- **First divergence** (Kundur DynExp, 1 dynamics substep): the DynExp derivative
  slot `dspeed` — both oracles compute **-1.6169543e-6**; the D14 no-op port left
  it at **0**. It compounds via the trapezoidal integrator; by the deck's 5 s
  endpoint the port's node V diverged catastrophically at the deep nodes (HT.1:
  oracle 122713 V @ 69.1° vs D14-frozen 193725 V @ 24.8°), while the quasi-ideal
  source bus barely moved (~2.6 V, 1.5e-5 — the misleading "entry 0").
- **Fix**: reverted `dynamic_exp.rs::solve_eq` to the full 0.14.5/r4133 evaluator
  (full `0..cmds.len()` loop, safe `cmds.get(idx+1)` for the benign OOB read, no
  early return, final upload after the loop); restored `get_out_idx`; fixed the
  `dyneq_pce.rs` doc. Cited to `DynamicExp.pas:377`/`:497`.
- **After** (measured live): the reverted port reproduces the oracle's step-1
  `dspeed` -1.6169543e-6 to the f32 monitor floor, and the 5 s endpoint matches
  (rotor `theta` 2.036 rad / `speed` 0.626; node V matches 0.14.5).
- **Gating**: both `Dynamic_KundurDynExp.dss` and `GFLDaily_DynExp` had their
  `defer_ledger` removed. Kundur gates on **both** channels at the feeder floor
  (no ledger entry — the two oracles agree ~4.8e-10, the port matches both). GFL
  gates on **r4133** (off `capi_v0145` for the separate D7 PVSystem-dynamics
  reason, like its non-DynExp sibling). The pre-staged `dynexp-d14` ledger cause
  is removed (no envelope needed).
- **Tests**: the 7 `exec/tests/dynamics.rs` DynExp gates + the `dynamic_exp` unit
  tests restored to their pre-D14 swinging-oracle pins (now guard against
  re-introducing the no-op). All 34 dynexp/dynamics unit tests green.
- **Settled** (two independent opus xhigh audits, `19e0890..5a6c6d4`): audit-code
  returned zero findings (faithful, Pascal-cited semantics correction). audit-tests
  raised two **low/INFO** notes, both settled empirically as *strengthenings, not
  weaknesses* — neither warrants a code change:
  - *GFL DynExp deck is r4133-only, not both.* Verified: its non-DynExp sibling
    `Run_IEEE123Bus_GFLDaily.DSS` is likewise `engines:r4133` for the pre-existing
    WP-U1.2 D7 reason (daily-shape `PanelkW` < `FkVArating` shifts the dynamics
    current limit off 0.14.5). The 0.14.5-side DynExp evaluator proof therefore
    lives in the **Kundur** deck, which gates on **both** channels — the filtered
    corpus gate (`DSS_GATE_ONLY=DynExp`, 3/3 pass) engages `capi_v0145`+`r4133`
    there. So the r4133-only GFL leaves no DynExp channel unverified.
  - *`dynamic_exp` interpreter unit values are hand-derived, not oracle-captured.*
    Matches the pre-D14 state and is the correct comparator for a
    compile/`SolveEq` path the oracle does not expose outside a dynamics solve.
    `kundur_expression_evaluates` computes `d(speed) = -1/mass·(pterm+damp·speed
    −pshaft)` to `1e-15` — which the D14 no-op leaves at 0, so the test is a real
    anti-no-op guard. It is backstopped end-to-end by the oracle-anchored exec
    swing gate (θ 0.42211992/1.7127246 rad = 24.18569/98.131889 deg) and the live
    corpus gate on both channels. No tolerance touched; goldens/harness untouched.
