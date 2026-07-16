# dss-rs — Project Status & Session Handoff

> **Purpose of this file:** a living snapshot so a fresh session can resume
> without re-deriving context. It records *what is done*, *what was decided*,
> and **why**. It is **not** authoritative for the plan itself — that is
> `PORTING_PLAN.md` (roadmap + binding decisions) and `CLAUDE.md` (conventions
> + the green-gate rule). Read those two first; then read this for the current
> frontier.

Last updated: 2026-07-16 (parallel Rung-1 round, 6 worktrees, each port +
dual-audit + fix, all merged to `update` and pushed: **WP-U1.5 complete**
(seasonal/allocation/zonelist), **WP-U1.9 complete** (PCE force hooks),
**WP-U1.7 Stages 2–4 core** (NCIM solver; corpus decks + Export/Show reports
remaining), **WP-U1.4 partial** (equivalent-spacing landed; Line-props/CNTS
blocked, see below), **WP-U1.6 partial** (B3/D10/D15/A7; C5/C6/D11/D13/C4
remaining, D12 reverted for re-land), **coverage parked test RESOLVED**. Two
merge-integration fixes (dump3 golden mask supersedes hand-added lines;
population.lock regen 56→58). Earlier 2026-07-12 merge summary → git history.
Integration branch is `update` (pushed to origin); main untouched until an
explicit merge request.)

**Rung-1 remaining tail (2026-07-16, the short list to rung exit):**
1. **PENDING USER DECISION — property-count harness accommodation:**
   `compare_all_properties` asserts Rust property count == 0.14.5-oracle count
   per circuit element, so any 0.15.x-appended property (U1.4 Line
   EpsRMedium/HeightOffset/HeightUnit/Conductors — engine numerics already
   ported; U1.6 C5 RegControl FwdThreshold/Idle*, C6 BH-props, C5-r3723
   LoadShape Mode) breaks nearly every default-oracle deck. Proposed: a named
   per-class allowlist of 0.15.x-only trailing props (§1.3-style relaxation).
2. WP-U1.7 tail: `modes/ncim/` corpus deck matrix (numerics already unit-pinned
   vs capi015), `Export Jacobian/deltaF/deltaZ` + `Show PV2PQ_Conversions`,
   `VSource.NCIM_CalcInjCurrAtBus` (reporting), r4088 cross-check.
3. WP-U1.6 small tail: C4 `Solve all` alias; D13 (needs a non-crashing MMF
   deck — corpus one hits capi015 #58614); D11 part 2 (capcontrol_time deck
   entanglement); D12 re-land (needs multi-step capi015 oracle support).
4. WP-U1.4 heavy tail: merged `CNTSLineConstants` per-conductor refactor +
   `CNData.SemiconLayer`; LineCode deprecation flags; LineType enum width;
   U1.2-D3 spacing ratings + overload deck.
5. WP-U1.10 rung exit (r4088 sweep + known_diffs burn-down) — after 1–4.

**PARKED TEST — RESOLVED (2026-07-16, branch wt-coverage):**
`circuit::coverage::tests::refine_bus_levels_reports_paths_on_radial` is
un-ignored and green. Root cause was the **test harness, not the port**: the
test passed its whole 8-line deck string to a single `Dss::command` call —
`command` is Pascal `ProcessCommand` (ONE command line), so everything after
`new circuit.covtest …` became extra parameters of that command (the trailing
`bus1=b5` from load ld2 landed on the Vsource; no lines were ever created).
`CalcIncMatrix_O` on that 1-bus circuit yields `Inc_Mat_Cols=["b5"]`,
`levels=[0]`: every traced path covers 0, the coverage plateau is 0, and the
state machine's sole exit (Circuit.pas:909, "changed AND >= Coverage") can then
never fire — a genuine, upstream-faithful degenerate-input nontermination fed
by a corrupted circuit. Fed line-by-line, the port terminates instantly and
bit-matches the official r3723 engine (Oddie probe 2026-07-16, both variants
< 50 µs): `set coverage=0.5` → "0 new paths detected", `Actual_Coverage =
0.666666666666667`; default 0.9 → "2 new paths detected", `Actual_Coverage =
1`. Both are now pinned in the tests (including the `get coverage` strings).
The second old hypothesis ("0.9 default unreachable, plateau 5/6") was also
disproven: `Buses_Covered` entries are bus-index SPANS whose sum overshoots
`Sys_Size` (r3723 reaches 1.0); the function's NOTE(upstream-quirk) was
corrected accordingly. See the WP-COV-PARKED record in §UPGRADE.

**AD dispositions — `off:unclassified-new-deck` bucket (2026-07-12):** at the
part2→update integration merge, every deck added after the WP-AD.4 sweep
(9 skipped-sweep promotions in `ad_sweep.json` + 63 new family decks: windgen,
U1.3 invcontrol, LINE-DEEP, coverage waves) received the explicit pending
disposition `off:unclassified-new-deck` (allowlisted in `AD_OFF_REASONS` with
the same note). This is a declared backlog, not a measured verdict — a
follow-up classification round runs DSS_AD_CLASSIFY/DSS_AD_DECOMPOSE over the
bucket and retires the reason.

**Standing toolchain note:** the gate runs on **`stable`** (`cargo +stable …`),
matching CI (`dtolnay/rust-toolchain@stable`) — no nightly dependency. `dss-core`
carries `#![allow(clippy::collapsible_match)]` (`d85d026`): clippy 0.1.96 (now on
stable) mis-fires that lint on the byte-faithful `match prop { CONST => if cond
{..} }` port idiom, and its autofix even drops `else` branches.

> **Working cadence:** finish one small step → run the full gate → update this
> file → **stop and wait for explicit user confirmation** before the next step.
> The full per-step ritual (gate, STATUS sync, the two audits) is
> **`PHASE7_PLAN.md §0`**, run per **§1e**. (Earlier phases sometimes executed
> several WPs in one pass on explicit user instruction.)

---

## 1. Where we are

**Era: post-final-acceptance UPGRADE (PLAN_SEQUENCE stages 4+).** The 1:1 port
reached FINAL ACCEPTANCE (2026-07-11, referee ACCEPT) and the post-acceptance
corpus-completeness rounds + JSON export merged to `main`. Active work is
**UPGRADE_PLAN Rung 1** (dss_capi 0.15.x / EPRI r4088 parity) on `upgrade-rung1`,
now merged with post-acceptance main. Post-acceptance sequence:
UPGRADE Rung 1 → Rung 2 (OpenDSS 11.0.0.1 / r4133) → DE_PASCALIZE (the dedicated
`TODO(compat)` wipe + golden regen, PORTING_PLAN §4.1/§6) → RESONANCE →
MULTITHREADING M0–M4. Part II A-Diakoptics (WP-AD.2–AD.6) is sequenced after
MULTITHREADING M2.

**In flight / next.**
- **D14 (DynamicExp RPN "index-bug fix") — landed, pulled ahead of WP-U1.6** (branch
  `dynexp-d14`). Upstream `2a8bdb78` adds an `Exit` to `SolveEq` that returns before
  evaluating the RHS, making it a no-op evaluator: DynExp state variables freeze at
  their `InitStateVars` seed (no rotor swing / inverter ramp). Ported 1:1
  (`dynamic_exp.rs::solve_eq`), matching capi015 to the f32 floor (probed: generator
  `speed`/`theta` frozen vs 0.14.5 swing). Unstraddled the parked
  `GFLDaily_DynExp` deck (re-promoted `oracle:capi015`); flipped `Dynamic_KundurDynExp`
  to capi015; re-pinned 7 `exec/tests/dynamics.rs` DynExp gates to the frozen values
  (now D14 regression guards). See DIVERGENCES.md §D14.
- **WP-U1.8 (WindGen + WTG3 dynamics) — LANDED** on branch `wp-u18` (new PC element +
  the general dynamics-entry Y-rebuild fix + the `micro_wtg3_dynamics` floor tier).
  See the UPGRADE record below.
- **WP-U1.7 (NCIM solver)** — **Stages 1–4 core landed** (branch `wt-u17`): Stage 1
  (`RealSparseSet`) + the full `NCIMSolutionHelper.pas` port (`ncim.rs`), PDE_ONLY Y
  build, generator PV fields, `Set Algorithm=NCIM` dispatch, and the
  `IgnoreGenQLimits`/`NCIMQGain` options. Post-audit, every electrical value in
  `exec/tests/ncim.rs` is pinned against capi015 NCIM captures (incl. the PV-bus
  path: regulating / Q-limit PV→PQ / faithful nonconvergence). **Remaining:** the
  capi015 corpus deck matrix (micro/PV/midi decks flip `pending:false`) and the
  `Export Jacobian/deltaF/deltaZ` + `Show PV2PQ_Conversions` reports. See the
  §UPGRADE WP-U1.7 record below.
- **WP-U1.2 (numeric long tail)** — rows B2/D1, D7, D6, B1, D8 landed; **B3-r3723**
  (Load.GrowthFactor Year=0) landed under WP-U1.6; **remaining: D3** (report-only
  spacing ratings — needs an overload-report deck). See the resume note under
  §UPGRADE below.
- **B5 GFM `Isc1` ×1000 — SETTLED (GFM WP, branch `gfm-wp`).** Adopted the r4133
  `Isc1` (drop the `·1000`) in `calc_gfm_yprim`. The feared "injection-vs-YPrim
  gap" does **not** exist at this base: the Rust GFM op-point is already
  Isc1-invariant (proven — the positive-sequence Norton impedance `Zs−Zm = R1+jX1`
  is `Isc1`-free; only the zero sequence moves, and delta/balanced GFM loads see
  only the positive sequence). 8 vendored + 4 controls GFM decks flipped to
  `oracle:capi015` (whole-model green); non-discharging-GFM decks stay 0.14.5-green.
  Also settled the unrelated capi015 daily `CktElement.Losses` staleness quirk (not
  reproduced; harness `loss_w` self-validating skip). See §UPGRADE + DIVERGENCES.md.
- **WP-U1.5 — LANDED complete** and **WP-U1.9 — LANDED complete** (2026-07-16
  round); **WP-U1.4 / WP-U1.6 — PARTIAL** (blockers in the Rung-1 remaining-tail
  list above). Full records in §UPGRADE below.
- Part II A-Diakoptics (AD.2–AD.5) merged 2026-07-12 — records in
  `docs/phase-records/part2-adiakoptics.md`; AD.6 (threaded children) remains
  sequenced after MULTITHREADING M2.

**SKIPPED-SWEEP (branch `skipped-sweep`, 2026-07-12).** Gave every in-scope entry
in `skipped_needs_investigation.json` a real disposition (19 entries; the 2
`upgrade_straddle_gfm_wp` + 1 `upgrade_straddle_u16_dynexp` decks were left to their
dedicated WPs). Probed each on the official EPRI engines (r3723/r4088/r4133 via the
Oddie bridge) and the pinned 0.14.5 oracle; verified Rust behaviour first-hand.
**9 promoted to `solvable_now`, 10 kept parked with refreshed evidence** (settle
2026-07-12 promoted 2 more 4wire-Delta decks — see the settle note below).

- **Root cause found for 6 "oracle_nonconvergence" decks: the deck's `maxiterations`
  cap was simply below what the (convergent) circuit needs** — not a solver defect.
  With `post: ["Set maxiterations=200"]` the pinned 0.14.5 oracle AND Rust converge
  at the **exact same** iteration count with bit-identical node voltages:
  StevensonPflow (90), StevensonPflow-3ph (106), IEEE 30 Bus/Master (19; its own
  sibling `Run_IEEE30.DSS` sets maxiterations=100), 8500-Node/Master-unbal (62),
  GFM_IEEE8500/Master (67), GFM_IEEE8500/Master-unbal (62). Promoted `kind=large`.
- **1 "user_model" deck promoted via a harness change:** `Test/indmachtest/Master.DSS`
  (Generator model=6 with an unvendored user-model DLL). The oracle server learned a
  `warn_and_continue` mode (driven by the case's existing `expect_warnings`): set
  `DSS.Error.EarlyAbort=False` + tolerate the user-model DoSimpleMsg (#567/#570/#1570)
  at compile, every solve, and the priming currents read — 1:1 with the official
  Direct DLL's warn-and-solve. Rust 8 iters == pinned 8, node V bit-identical.
- **Kept parked (refreshed):** `vsctest` + `Torn_Circuit`
  Master/Interconnected (GENUINE non-convergence on r3723/r4088/r4133 even at
  maxiterations=1000); `IEEE118Bus` (convergence is an r4088/r4133-only solver change,
  Rust mirrors r3723=NO → BLOCKED_PENDING UPGRADE); the 2 `oracle_timeout` TnD decks
  (A-Diakoptics not ported → not promotable regardless); `ieee9500_base` (pathological
  voltage-collapse deck, NO on r3723/r4088/r4133); the 2 `conditioning_floor` decks
  (`CIM/IEEE13_Assets`, `SecondaryTestCircuit_modified` — proven cross-solver floors
  re-affirmed by decomposition, no honest band fits).
- Harness change: `tools/oracle/oracle_server.py` (`warn_and_continue`,
  `_USER_MODEL_ERRNOS`, EarlyAbort toggle in `main`, priming retry in
  `capture_all_elements`) + `corpus_live.rs` (send `warn_and_continue` when
  `expect_warnings` is set). `solvable_now` 283→290, `skipped_needs_investigation`
  22→15; `population.lock` regenerated in-commit.

**SKIPPED-SWEEP settle (2026-07-12).** Audit found the two 4wire-Delta `Kersting4wire_Lagging`
/ `Kersting4wire_Leading` decks were parked on a false premise. The "Rust 3 iters vs oracle 2"
claim compared Rust COLD (the deck's internal compile-time Solve, 3) against the oracle WARM
re-solve (2). Measured symmetrically on both engines: **cold 3 / warm 2 on BOTH**, and the live
harness (compile + explicit solve) compares the WARM re-solve → 2 == 2. Warm node V is
bit-identical (Lagging SOURCEBUS.1 7200.002150, Leading 7199.557591; all 12 nodes agree to 6+
figures on both engines). Promoted both via the same `warn_and_continue` path as `indmachtest`
(`kind=feeder`, `expect_warnings=["Not Loaded"]`). `Kersting4wireIndMotor` stays parked, note
corrected: its real blocker is NOT iterations (also cold 3 / warm 2) but a genuine 4th-wire/
neutral-node divergence — its `LineCode.556MCM` declares `nphases=4` yet supplies only a 3×3
(6-entry) cmatrix; safe Rust rejects the malformed 4-phase Cmatrix while the oracle tolerates
it, so PRIMARY.4 diverges 13.363452 vs 13.363170 (|diff| 2.82e-4), above the feeder floor. The
`warn_and_continue`→`expect_warnings` coupling remains non-masking (all 5 opted-in decks are
genuine user-model DoSimpleMsg cases; Rust independently enforces convergence + exact iterations
+ bit-identical V). `solvable_now` 290→292, `skipped_needs_investigation` 15→13; `population.lock`
regenerated in-commit.

> Working cadence and the standing toolchain note are just below; the full
> per-step ritual is `PLAN_SEQUENCE.md` / the active plan's §0.

---

### Condensed era summaries (full records in `docs/phase-records/`)

**FINAL ACCEPTANCE — ACCEPT (2026-07-11).** Max-effort referee round on
`final-acceptance` returned `criteria_met=true`, `blocking_items=[]` (B1 CIM
cross-class panic, B2 8500 save-floor, B3 anti-shrink guard all resolved with
re-verified evidence). §6 criteria met: `corpus_live` live-compares every
`solvable_now` deck (full Y/V/I/P + injection + discrete state per step) at the
calibrated floors; `save_roundtrip` 6/6 (IEEE 13/34/37/123/8500 + structural);
`golden_reports` 193/0; gate green (pinned dss-python 0.15.7 / dss_capi 0.14.5).
Gated population at acceptance **245/335 solvable_now (73.1%)**; the unported tail
is exclusively Phase-9 actor/parallel mode + `CapControl.ControlSignal`, explicitly
outside 1:1 acceptance. **Named non-blocking residuals** (all documented): the
RegControl/LDC `SubXFMR` family, `LVTestCase` entry-0 unenergized, UTF-8 BOM,
`CapControl.ControlSignal`, SecondaryTestCircuit_modified, GFM_IEEE8500 near-floor.
Several were later resolved (see corpus summary). Detail + the §6 criterion→evidence
table + FA settle + FA fix 1/2/3: **`docs/phase-records/final-acceptance.md`**.

**Corpus completeness.** Post-acceptance CF / CF2 / coverage-wave rounds drove
`solvable_now` to **~286/329 entry points (86.9%)**; synthetic families reorganized
into per-element subfolders and grown to **asymmetric 47 / controls 92 / modes 49**
live decks. Full blow-by-blow: **`docs/phase-records/corpus-rounds.md`**.
- Fixed real port bugs (one line each):
  - LongLineCorrection stored-but-never-applied → ported `TLineObj.DoLongLine` (asymmetric wave).
  - DIRECT-mode PCElement `GetCurrents` `LastSolutionWasDirect` shortcut → ported (FIX-DIRECT).
  - Dynamics-mode `ShapeFactor` ignored `ActiveLoadShapeClass` (PVSystem + the VSource/Isource/IndMach012 siblings) → fixed (CF2-G).
  - Element `base_frequency` hardcoded 60 instead of circuit fundamental → fixed (CF-A); resolves `LVTestCase`.
  - UTF-8 BOM not stripped on redirect; undefined-monitor export hard-error; bare-quote inline comment → fixed (CF-A).
  - CapControl `Type=follow` self-monitor dispatch aborted with "Monitored element not set" → fixed (CF-C).
- Proven floors (NOT bugs, reproduced/documented, never band-fudged):
  - `SubXFMR` family = ultra-switch conditioning (1e-8 Ω switch, Y≈1e8 S) + Carson libm floor (CF-D) — the acceptance "RegControl/LDC" label was disproven.
  - #485 control-settling family = hunting-truncation reproduced 1:1; the "divergence" was a harness second-solve measurement artifact (CF2-R).
  - DOCTechNote ×4 + GFMSnap = floating-zeroseq common-mode floor (FA fix 2).
  - SecondaryTestCircuit_modified / IEEE13_Assets = conditioning floors (documented, too wide to band).
  - GFM_IEEE8500 Snap/Daily = near-floor faer-vs-KLU.

**JSON export (A+B) — COMPLETE.** `Obj_ToJSON` / `Batch_ToJSON` (Stage A) +
`Obj_Circuit_ToJSON_` (Stage B), ~130-byte goldens. Detail in
`docs/phase-records/gaps.md`.

**Part I DIAKOPTICS/PSTCALC — COMPLETE; Part II sequenced later.** WP-PF.1
(`Pstcalc` command), WP-PF.2 (Monitor mode-4 flicker), WP-AD.1 (incidence matrix +
`Sparse_Math` + exports 53–57) all gate-green. Part II (A-Diakoptics WP-AD.2–AD.6)
is deliberately outside final acceptance, sequenced after MULTITHREADING M2. Detail:
**`docs/phase-records/part2-adiakoptics.md`**.

**UPGRADE Rung 1 (U0 / U1.1 / U1.2).** Multi-oracle test infra (WP-U0: per-case
`oracle` manifest field routing to capi015 / r3723 / r4088 / r4133; `capi015`
engine; `Rust ≤ oracle` iteration policy for target-rev cases; r4133 pilot in every
`cargo test`). **WP-U0.2** = report-only `ab_compare` inventory sweeps over 378
cases/pair across the three rung pairs + reconciliation of the `delta_*.md` ledgers.
**WP-U1.1 (parser/property semantics) — items 1–5 all landed:** L2 `DblValueNZ`
zero-`kW`/`kVA` clamp, `ParseAsSymMatrix` incomplete-matrix reject, `AllowNoneItem`
(`none` in conductor lists), `TCC_Curve.none`, C11 class-command activation.
**WP-U1.2 (numeric long tail) — landed rows:** B2/D1 (SimpleCarson De
`658.5→658.8530451057239`), D7 (PVSystem dynamics current-limit base
`PanelkW→FkVArating`), D6 (Transformer seasonal AmpRatings drop `1.1×`), B1
(Capacitor Cmatrix YPrim diagonal `×1.000001`), D8 (settled, no code change — not a
0.14.5→0.15.x delta). **WP-U1.3 (InvControl cluster) — all 6 rows settled:** D1/ledger-L1
InvControlDeltaV per-control 2-slot buffer (adopt capi015 fix; the r4133 `i=1`
cursor gating cataloged as a known upstream bug), D2 per-DER basekV, D3
sqrt-guard (EPSILON=1e-12), D4 delta-DER LL monitored voltage (sign-flipping,
capi015==r4133; unit + capi015 deck pinned), D5 no-delta, C8 (a)
`VV_RefReactivePower` removal NOT adopted (r4133 keeps it) + (b) MonBus
#2024111/#2024112 validations. Known limit: the capi015 oracle cannot gate
multi-step decks (per-step capture re-nominalizes shapes) — capi015 corpus cases
are snapshots; follow-up logged for the oracle-infra owner. Full detail:
**`docs/phase-records/upgrade-rung1.md`**.

**WP-U1.5 (EnergyMeter seasonal / allocation / monitor-header) — LANDED (branch
`wt-u15`).** Five spec rows settled (detail in `docs/upgrade/DIVERGENCES.md`):
- **E2 / ledger L4 (SeasonalRating reimplementation) — adopted capi015 = r4133.**
  New global `Circuit::seasonal_rating_idx` synced by
  `solution::meters::sync_seasonal_rating_idx` at every solve (Pascal
  `SyncSeasonalRatingIdx`, `55400a29`); new `CktElement::get_ratings(idx)` trait
  method (`TPDElement.GetRatings`) overridden by Line + Transformer `num_amp_ratings`/
  `amp_ratings`; wired into `export_capacity`/`export_overloads`/`write_overload_report`
  so the seasonal `AmpRatings[idx]` applies to ANY PDElement (0.14.5 restricted
  `DI_Overloads` to lines and never applied it in `Export Overloads`). The
  0.14.5 state-mutating `SeasonalRating := FALSE`-on-miss read is not reproduced
  (precomputed index removes it). Gate: 2 new **capi015** report goldens
  (`export_{overloads,capacity}_seasonal`, `.meta.json` `oracle:capi015`,
  regenerated via the new `DSS_ORACLE_ENGINE=capi015` branch in `gen_reports.py`;
  deck = overhead Line + Transformer + CN cable, all `Seasons=4`, validated
  bit-identical capi015 == oddie:r4133 §1.7) + feature-sensitive `get_ratings`
  unit tests (Line + Transformer). Probe: 0.14.5 reports base `%Normal=134.5`,
  capi015/r4133 the seasonal `336.3` — revision-sensitive.
- **D9 (AllocateLoad/CalcAllocationFactors skip disabled meters/sensors) — adopted
  capi015 (r4115 `fb728364`).** Two `if !enabled` guards in
  `sampling/allocate.rs`. 0.14.5 hit an Access Violation walking a disabled
  meter (UB, not reproduced). Gate: `allocateloads_ignores_disabled_meter`
  (feature-sensitive — meter enabled at zone-build then disabled; factors stay 0.5
  vs 6.3725 enabled).
- **D8-r3723 (manual-ZoneList child from-bus/terminal) — adopted r4133.**
  `zones/build.rs`: `add_new_child(terminals[0].bus_ref, 1)` (was `(NO_BUS, 0)` =
  0.14.5). IS a code delta (verify verdict), but its effect is **masked** in our
  path (from-bus already volt-base-listed; DistFromMeter not propagated for manual
  zones); 0.14.5 AVs so no oracle golden — the memory-safe
  `energymeter_manual_zonelist` test guards it.
- **D16 (zone counter skips disabled + non-PD) — NOT a delta for us.** Already in
  the 0.14.5 baseline and already ported (`zones/build.rs` `if !enabled ||
  !is_pd_element`); 0.15.x only flattened the guard. No code change.
- **E1 / ledger L3 (Monitor header) — keep the dss_capi form.** The quote-removal
  + `MonitorHeader` flag are CSV-render only (flag off by default); probed
  capi015 `Monitors.Header` tokens == 0.14.5 == Rust. No code change; the
  `monitor-header-whitespace` known_diff (KEPT vs EPRI) stays.
- known_diffs burn-down: no seasonal/allocation entry ever existed (reports were
  NOT_PORTED); `meter-zonepce-count` (r3723-only) + `monitor-header-whitespace`
  both document behaviors this WP does not change → retained.
- **Audit fixes (branch `wt-u15`).** (1) `get_ratings` blocker REBUTTED: the
  auditor cited r4133/pre-refactor `NumAmpRatings > 1`, but the port ports
  `55400a29`'s `GetRatings` guard `0 <= idx < NumAmpRatings` (no `>1`); the pinned
  capi015 oracle (0.15.0b4/SVN4103) confirms it — a single-season Line at idx 0
  reports `%Normal == %Emergency` (AmpRatings[0] overrides both). No code change;
  docs corrected to stop misquoting the guard. (2) Set-command sync (real
  divergence): `Set Hour`/`SeasonRating`/`SeasonSignal` now re-sync
  `seasonal_rating_idx` (`55400a29` ExecOptions 3/114/115), verified on capi015
  (`solve; set hour; export` reads the new index) —
  `set_commands_resync_seasonal_rating_idx`. (3) Seasonal report goldens tightened
  to exact `0.0/0.0` (Rust == capi015 byte-for-byte; the faer-vs-KLU floor was
  unnecessary). (4) Added `di_overloads_applies_seasonal_rating` (DI-path seasonal
  wiring).

**WP-U1.9 (PCE force hooks) — LANDED (branch `wt-u19`).** Ported the dss_capi
0.15.0b4 pyControl engine hooks — spec read via `git show 0.15.0b4:` because the
vendored working tree `.inputs/dss_capi_with_git` sits at a later `master`
(`f5728aec`) where these options were **removed** upstream; the capi015 oracle
(tag `e936d210`) still carries them, so it remains the authoritative spec.
Landed: `Set`/`Get` `InjCurrent`/`ITerminal`/`YPrim`/`StateVar`/`IterNumber`/
`CtrlIterNumber`/`IntegrationFlag` + `Flg.ForceInjCurrents`/`ForceYPrim`, honored
in the injection loop (`solution/solution/power_flow.rs` injects the stored
`InjCurrent` directly, per `TPCElement.InjCurrents`) and `ReCalcAllYPrims`
(`solution/ymatrix.rs` skips `CalcYPrim` when `ForceYPrim`); the five PCE
`GetTerminalCurrents` (Load/Generator/PVsystem/Storage/IndMach012) skip the model
recompute when forced. `Set IterNumber`/`CtrlIterNumber`/`IntegrationFlag` are
read-only; `Set PyPath=` + the pyControl component stay NOT_PORTED (loud, §0).
`SampleControlDevices` was already ported (present in 0.14.5, `Solution.pas:1974`
→ `solution/controls/sampling.rs`) — NOT a delta; `delta_capi_0145_015x.md` A3
over-claimed it as new. Parser gained `make_complex`/`parse_as_complex_vector`/
`parse_as_complex_matrix` (`ParserDel.pas`, `(f64,f64)`-tuple, dep-free). Gated by
the live `modes/upgrade_forcehooks.dss` (oracle:capi015, validated bit-identical
across two capi015 processes; `Set InjCurrent=[80 0 80 0 80 0]` moves b2 Vmag
7187.45→7224.14 V) + the capi015-pinned unit suite `exec/tests/force_hooks.rs`
(forced Vmag 7224.143523 @1e-6, frozen `Get InjCurrent`/`ITerminal`, read-only
sets, `Set YPrim` survives a rebuild, `Set/Get StateVar`, and **`Clear` resets
the force flags**). Ledger: `docs/upgrade/DIVERGENCES.md §A3/A5`.

**WP-U1.9 audit follow-up — LANDED (branch `wt-u19`).** Addressed 7 audit
findings against the capi015 oracle. Fixed: `Set/Get AllowForms`/
`AllowProgressBar` now accepted headless no-ops (round-trip, default `No`) —
were erroring "not ported"; `Set/Get StateVar` non-PCE now gives the Pascal 7103
"is not a valid PC element" (guard runs before the 7101 NumVariables check); the
force-hook error arms now `Exit` (break) the option loop like Pascal. Corrected
the false "Set/Get StateVar covered" claim: `Set StateVar` via text is
**upstream-broken** (positional `DoSetCmd` parse never reaches the arm →
capi015 `#303`, reproduced as error + no write); `Get StateVar` is the
functional read path. Grew the unit suite to 12 tests (added: `Set ITerminal`
freeze, Generator 2nd-PCE force-skip, oversize-row `#3004`, natural-syntax `Set
StateVar` error, both 7103 guards, AllowForms round-trip). The `#3004` YPrim
error zeroes capi015's live matrix (its own known error-state imperfection) —
**not reproduced** (scratch-buffer parse leaves the real YPrim intact; transient,
next `ReCalcAllYPrims` recomputes). Ledger updated in DIVERGENCES §A3/A5.

**WP-U1.8 (WindGen + WTG3 dynamics) — LANDED (branch `wp-u18`).** New PC element
`elements/pc/windgen/` (Generator-shaped negative load): aerodynamic power-flow
(`Pm=0.5·ρ·π·Rad²·v³·Cp`, the load shape supplies WIND SPEED not a pu multiplier;
kWBase curtailment + cut-in/cut-out; 4 models 1/2/4/5) + the embedded GE WTG type-3
dynamics (`wtg3.rs`, 1:1 of `WTG3_Model.pas`: PLL, seq-current PI regulators,
LVPL/LVQL ride-through, Cp 5×5 aero, MPPT/torque/pitch/inertia, one-mass swing, the
**odd-substep 50 µs trapezoidal sub-cycle**, all 22 state vars). Harmonics DISABLED
upstream → reproduced as a loud abort. `windgen_model`/`windgen_qmode` enums,
`ElemKind::WindGen`, PASCAL_CLASS_ORDER slot after Generator. 5 live `modes/windgen/`
capi015 decks (snap wye/delta, daily single-step, dynamics, dynamics+fault) + 11
unit tests. Two cross-cutting findings: (1) a **general dynamics-entry Y-rebuild
fix** — `calc_initial_machine_states` now raises `system_y_changed` (Pascal
`InitStateVars`→`SetYprimInvalid`→`SystemYChanged`, lost in the port), without which
the WTG3 Norton injection ran the terminal voltage away; (2) the new
`micro_wtg3_dynamics` tolerance tier (decomposition-proven: snapshot input matches
1e-8, the PLL derivative `×60000` amplifies the near-cancellation `Vq` into a ~1e-5
state / ~1e-6-rel terminal-V floor that DECAYS as the transient settles — a WPG.13
amplification floor, NOT a bug). WindGen energy-meter registers not ported
(`EnergyMeter.SampleAll` never samples WindGenClass — unreachable). Daily deck is
single-step: dss_capi 0.15.x caches per-element `Losses` and WindGen doesn't
invalidate it (bucket-F API quirk, out of scope).
- **WP-U1.8 settle (audit).** (1) The `micro_wtg3_dynamics` tier is empirically
  confirmed a genuine cancellation floor, not a masked state-leak: a per-variable +
  per-node decomposition vs capi015 (throwaway probe, reverted) shows the gap is
  confined to exactly the 3 PLL-derivative-fed vars (`dOmg`/`Pgen`/`Qgen`, ~1e-5
  healthy / ~4e-5 fault) while 14 of 22 vars are bit-exact and the other 5 are ≤5e-7;
  the worst node-V is always WBUS (the terminal bus) at 9.6e-7 rel healthy / 3.4e-6
  fault, so the default `v_rel=1e-7` genuinely fails and `8e-6` covers it at ×2.3
  (not over-loose). (2) Fixed a robustness defect: a 1-phase WindGen entering
  dynamics used to **panic** (OOB in `wtg3` `instrumentation`, which reads V[1..3] —
  the WTG3 model is 3-phase-only; upstream over-reads = heap UB, NOT reproduced). Now
  a loud clean abort — `init_state_vars` aborts non-3φ before the model init, and
  `do_dynamic_mode` guards the per-step path (external `solve` clears the init abort);
  `calc_initial_machine_states` now drains+propagates element init aborts to
  `solution_abort` (also surfaces the latent >3φ silent-garbage path for all
  machines). New unit test `single_phase_dynamics_aborts_cleanly`. (3) The 5
  `windgen/*` decks joined the `MODES_REQUIRED` anti-deletion floor.
- **Resume note (WP-U1.2 remaining).** Row **D3** (report-only spacing ratings —
  overload-report deck) still to port. **B3-r3723** (Load.GrowthFactor Year=0)
  LANDED under WP-U1.6 (branch wt-u16). The golden engine switch
  (`gen_checkpoints::check_pin` `DSS_ORACLE_ENGINE`) and the
  same-commit flip/regen/retire workflow are proven. NB the modes manifest is NOT
  `json.dumps`-round-trippable (mixed manual `\uXXXX` escaping + CRLF) — append new
  cases with a surgical text edit.

**WP-U1.6 (controls & misc long tail) — PARTIAL, LANDED (branch wt-u16).**
Landed rows (each unit/deck-pinned, gate-green):
- **B3-r3723** (own commit) — `Load.GrowthFactor` Year=0 with a GrowthShape now
  tracks the simulated hours (`calcYear=dblHour/8760`; `GetMult(Ceil)` or
  `GetMultIdx(1)` when firstY=0 & <1yr) instead of a flat 1.0. Added GrowthShape
  `get_year`/`get_mult_idx`. **Oracle-validated + deck-gated** (audit-U1.6
  settlement): `git show 0.15.0b4:src/PCElements/Load.pas` carries the rewrite
  verbatim (the working-tree checkout f5728aec predates it, see DIVERGENCES.md
  version note), and capi015 probes confirm 120 kW (factor 1.2) vs 0.14.5's flat
  100 kW. New corpus deck `modes/upgrade/upgrade_growth_year0.dss` (`oracle:
  "capi015"`, snapshot, feature-sensitive 120-vs-100 kW; §1.7 two-process
  determinism confirmed). Unit tests
  `growth_factor_year0_tracks_simulated_hours_with_growthshape` (probe-cited) +
  `get_year_and_mult_idx_are_one_based`.
- **D10** StorageController — (a) `a14c3f1f` FpctkWBandLow typo fix (was reproduced
  as `TODO(compat)`; adopted, `FpctkWBandLow := FkWBandLow/FkWTargetLow*100`);
  (b) `1b3123ce` force a new power flow on control iter 1 when peakshave(-low)
  moves the fleet into (dis)charge even when the condition matched last step
  (added `control_iteration()` to `StorageDispatchEnv`). Both in capi015 0.15.0b4
  (= r4103; `StorageController.pas:547`). **Oracle-validated** (audit-U1.6):
  capi015 `%kWBand=16.667`/`%kWBandLow=20` vs 0.14.5 typo `6.667`/`2`. Unit tests
  `kw_band_low_side_effect_syncs_the_low_pct_pair` (property sync, added under
  audit) + `d10_discharge_transition_forces_resolve_on_first_iteration`
  (force-resolve). Unit-pinned (no single-step corpus witness: property-only sync +
  multi-step force-resolve; precedent B1/D6/D7).
- **D15** (`4366b126`) — `LookupVariable` case-insensitivity: the only
  equivalent in the port (relay) already uses `eq_ignore_ascii_case` (= the fixed
  side); not-a-delta, upper-case query pinned in `lookup_variable_prefix_match`.
- **A7-r3723** — GenController deregistration: the r3723 port never registered the
  class, so `New GenController.…` already errors "not found"; not-a-delta, pinned
  by `gen_controller_class_is_not_registered`.

Not landed (documented for a follow-up — each needs oracle-validated decks and/or a
property-count-comparison flip beyond this pass's safe budget):
- **C5** RegControl `FwdThreshold` (`8a898cba`, SVN r4086) — the flagship: adds 4
  new props (`Idle`/`IdleReverse`/`IdleForward` [new in 0.15.x, absent from the
  r3723 port] + `FwdThreshold`) → RegControl property-count change (entangles the
  RegControl deck property-count compare, C8-class), PLUS the signed
  `RevPowerThreshold`/`FwdPowerThreshold` rework (defaults −100kW/+100kW, EndEdit
  legacy fallback `Fwd:=abs(Rev); Rev:=−Fwd`), the idle-zone `SetPointCalc` logic,
  and the reverse-power detection rewrite. Precise hunks in
  `.inputs/dss_capi_with_git` commit `8a898cba`. Needs the RegControl reverse-power
  deck matrix (legacy-input equivalence + new-property divergence) on capi015.
- **C6** Transformer/AutoTrans `BHpoints`/`BHcurrent`/`BHflux` (`90962ae8`) — 3
  new `Unused` data props on BOTH classes → property-count change entangles every
  default-oracle transformer/autotrans deck (C8-class); needs a coordinated flip.
- **C5-r3723** LoadShape `Mode` prop (22) + `Interpolation` shift 22→23 — new
  prop → property-count change (LoadShape decks) + property-index parity; same
  entanglement class as C6.
- **D12** SwtControl `Normal`/`State` field mapping (`bb9c9785`) — **ported +
  reverted** (commit 82d62c3 reverted by 0c918ab). The field-mapping change was
  correct (props golden re-baselined to capi015, 22 unit tests + props_roundtrip
  green), BUT it moves the `Normal`/`State` *readback* on TWO default-oracle
  **multi-step** live decks: `controls/swtcontrol/swtcontrol_time.dss` (manifest
  probe `SwtControl.sw.state`: 0.14.5 reads `CurrentAction`="open" after an armed
  `action=open`, the port reads `PresentState`="closed" until the switch operates)
  and `Version8/.../civanlar model/civanlar.dss` (`SwtControl.5_11` `Normal`
  readback). Neither can flip to capi015 (multi-step decks re-nominalize on the
  capi015 capture, L1 note), and they cannot stay 0.14.5 (deliberate mismatch,
  §1.2). Landing D12 needs the oracle-infra multi-step-capi015 support OR reworking
  those decks' probes off the moved readback. Code hunks are in commit 82d62c3 for
  the re-land.
- **D11** CapControl — part 1 (PT/CTPhase validation scope: PF-value validation
  gated behind `control_type==PF`, phase validation unconditional) is ALREADY
  aligned in the port. Part 2 (`b9bc87b8`: TIMECONTROL now REQUIRES a monitored
  element AND uses it as `effElement`, was ControlledElement) changes TIME
  effElement semantics → entangles the `capcontrol_time.dss` corpus deck (default
  oracle) + `time_control_forces_terminal_1` unit; needs capi015 deck validation.
  Code change is a one-liner in `cap_control/mod.rs::recalc` (drop the `!= TIME`).
- **D13** LoadShape MMF fixes (`c4590d16`) — the corpus MMF deck crashes #58614 on
  capi015 (B9); needs a non-crashing MMF deck; not assessed this pass.
- **B4-capi** harmonics init-failure abort (`6ad39597`) — the port's
  `solve_harmonic_t_body` ALREADY returns on `!initialize_for_harmonics` (aborts
  the sweep); the `In_ReDirect → Redirect_Abort` nuance is unreachable (no ported
  `init_harmonics` sets `solution_abort`; see `harmonics.rs` doc). Faithful as-is;
  no feature-sensitive deck possible.
- **C4** `Clear all`/`ClearAll` already handled (`cmd::CLEAR|CLEAR_ALL`);
  `Solve all`/`SolveAll` (`cmd::SOLVE_ALL`=123) not yet dispatched — small command
  alias (single-actor = plain solve), unstarted.

**WP-U1.4 (line/cable-constants cluster) — PARTIAL: equivalent-spacing model LANDED
(branch wt-u14).** Ported the **B3/C1 equivalent-spacing model** (dss_capi 0.15.x
`LineSpacing.pas`/`LineConstants.pas`, SVN r3913-era) end-to-end, gate-safe (defaults
preserve 0.14.5 numerics):
- `LineSpacing` gains `Detailed` (bool, default `true`) + `EqDistPhPh`/`EqDistPhN`/
  `AvgPhaseHeight`/`AvgNeutralHeight` (double, default 0) + `EquivalentSpacing() =
  !detailed` + the `Detailed` prop-tracking side effect.
- `LineConstants` engine gains `equivalent_spacing`/`eps_r_medium` (1.0)/`height_offset`
  (0)/`user_height_unit` + the four equivalent distances; `calc_overhead`/`get_ze`/
  `cisp_overhead` branch on equivalent spacing 1:1 with the Pascal. `EpsRMedium`
  (`pfactor /= E0*eps_r_medium`, `E0*1.0==E0`) and `HeightOffset` engine numerics are
  ported default-off (the Line-level *properties* that drive them are deferred, below).
- `LineGeometry` copies the spacing's equivalent state (`apply_spacing`/
  `load_spacing_and_wires`) and threads it into `UpdateLineGeometryData`
  (distances × `To_Meters(FLastUnit)`; skips `SetX`/`SetY`).
- **Validation:** unit `line_geometry::tests::matrices_equivalent_spacing_match_capi015`
  (reduced 3×3 Z = capi015 to 1e-8), new capi015 corpus deck
  `modes/upgrade/upgrade_linecs_eqspacing.dss` (live YPrim compare green; §1.7
  two-process bit-identical), new capi015 props golden
  `props/linespacing_eqspacing.json`. No existing golden/live case moves
  (`Detailed` default true; 0 corpus decks set the props). DIVERGENCES.md §B3/C1.
- **Audit follow-up (both minor findings fixed):** the always-on unit test now also
  pins the equivalent-spacing **Yc/capacitance** branch (reduced 3×3 C = capi015
  `? line.l1.cmatrix` 16.16 / -4.087 nF/mi to 1e-8), so the shunt branch no longer
  relies solely on the live YPrim compare; and the `Set_FUserHeightUnit` meters-value
  re-conversion quirk in `support/line_constants` now carries a greppable
  `TODO(compat)` marker (dead scaffolding today — height_offset is always 0 and the
  Line-level HeightUnit prop is deferred; golden pins it when that slice lands).
- **Remaining WP-U1.4 rows (documented, not landed):** `Line.EpsRMedium`/
  `HeightOffset`/`HeightUnit`/`Conductors` **Line-level properties** — BLOCKED on the
  `compare_all_properties` count-equality harness (adding a property to the
  circuit-element class `Line` breaks every default-oracle feeder's property-table-
  shape assert; needs a harness accommodation for 0.15.x-only trailing props, a
  design decision — the engine numerics are already in place); the merged
  `CNTSLineConstants` mixed-conductor class + `CNData.SemiconLayer` capacitance
  (an engine architectural refactor: 0.15.x moves the CN/TS choice per-conductor via
  `SetCondType(i, CN|TS)` — the port still has per-*engine* CN/TS kinds); `LineCode`
  FaultRate/PctPerm/Repair deprecation (catalog, adds Deprecated/Unused flags);
  LineType enum width; and **WP-U1.2 D3** spacing ratings + overload deck.

**GFM WP (branch `gfm-wp`) — B5 + injection-vs-YPrim + 0.15.x YPrim delta —
SETTLED.** Adopted B5 (`calc_gfm_yprim` `Isc1` drops the `·1000`, dss_capi
`de6a5a42` = SVN r3865). The deliverable-3 "0.15.x GFM Storage YPrim delta" is the
SAME one-line change (Storage/PVSystem share `CalcGFMYprim`; the Storage
`CalcYPrimMatrix` GFM branch is otherwise byte-identical across 0.14.5/0.15.x). The
deliverable-1 "pre-existing injection-vs-YPrim gap" was **disproven at this base** —
the Rust GFM op-point is already Isc1-invariant (positive-seq Norton impedance
`Zs−Zm = R1+jX1` is Isc1-free; only the zero seq moves; delta/balanced GFM loads see
only the positive seq). Empirically: `gfm_micro` `Load.isl`/`islbus` bit-identical
under old vs new `Isc1`, both equal to the bit-identical 0.14.5/capi015 value; the
storage YPrim moves to the capi015 live-probe value. **8 vendored GFM decks flipped
to `oracle:capi015`** (2 re-promoted from `skipped_needs_investigation` +
`CannotPickUpLoad` + 5 Microgrid GFMSnap/SwapRef/8500-GFMSnap whose gated state ends
discharging-GFM) **+ 4 `controls/gfm` decks** — all whole-model live green vs
capi015 (system Y + V/I/P per step). Non-discharging-GFM decks (Microgrid GFMDaily/
Snap-A/B/WholeDaily, 8500 Daily/Unbal) stay 0.14.5-green (their YPrim never reaches
`CalcGFMYprim`; confirmed by per-deck capi015-vs-0.14.5 assembled-Y diff). Also
settled the **unrelated capi015 daily `CktElement.Losses` staleness** (Losses freezes
at step 0 while Powers scale; general 0.15.x quirk, NOT reproduced — Rust matches
0.14.5/r4133 fresh losses; `harness::compare_element` now self-validating-skips the
redundant `loss_w` channel when the oracle's own Losses ≠ Σ its own Powers). New unit
tests: `gfm_calc_yprim_matches_capi015_isc1_no_1000`,
`gfm_norton_positive_seq_admittance_is_isc1_invariant`,
`storage_gfm_micro_op_point_isc1_invariant`. Detail: DIVERGENCES.md §B5 +
§capi015-daily-losses.

**WP-U1.7 (NCIM solver) — Stage 1 landed; Stages 2–4 handed off.** Spec = A1,
`Common/NCIMSolutionHelper.pas` (1048, FPC). **Stage 1 (done, own commit,
gate-green):** the `dss-sparse` **real-valued** KLU-shaped path
(`crates/dss-sparse/src/real.rs`, `RealSparseSet`) that the NCIM Jacobian needs —
Pascal `NewSparseSet` + `SetOptions(…MatrixFormat_DoublePrecisionReal)` +
`SetMatrixElement`/`SolveSparseSet`. Mirrors the complex `SparseSet` (triplet
accumulate in insertion order = CSparse `cs_dupl`; KLU `scale=2` row
equilibration) but over `f64`. **`set_element` ACCUMULATES** (not replace): NCIM
stamps each non-swing diagonal 2×2 block from the PDE-only `Y_ii` (`[B,G;G,−B]`)
in `NCIM_BuildJacobian`, then adds the load/gen injection derivative onto the same
cells in `NCIM_ApplyCurr`; the current-injection Newton diagonal is
`Y_ii_block + g'_ii_block`, so the two stamps must sum (under replace a PQ node
loses its network coupling → wrong Jacobian). 9 unit tests from hand Jacobians
(2×2, a 4×4 two-block CI-shaped Jacobian, insertion-order sum, singular, bad
scaling, zero/rebuild, dim-mismatch, **zero-stamp-dropped**). The accumulate
semantics are proven from the NCIM algorithm AND corroborated by the vendored
EPRI KLUSolve C++ (`VersionC/klusolve/KLUSolve/Source/KLUSystem.cpp`:
`SetMatrixElement`→`AddElement` appends, `GetElement` sums duplicates); the
DSS-Extensions KLUSolveX *fork* (the real `DoublePrecisionReal` format) is not
vendored but inherits the CSparse pipeline. **Settle fix:** `set_element` now
drops a zero value (`if value == 0.0 return`), matching `AddElement`
(`KLUSystem.cpp:442-444`) — an earlier doc comment claimed the no-op but the code
did not implement it, so an exact-zero cell (pure-R load `B`-diagonal, pure-R/-X
branch off-diagonal) would have inflated `nnz`/`Export Jacobian` vs the capi015
oracle in Stage 3; a covering test (`zero_stamp_is_dropped`) was added.
- **Stages 2–4 remaining (integration map for the next executor):**
  - **State** (`solution/solution/state.rs`): add `NCIMSOLVE=2` + the ~15 NCIM
    fields (Solution.pas l.243-271). Node i (1-based, ground=0) → Jacobian
    0-based rows `2*(i-1)`, `2*(i-1)+1`; swing = nodes 1..3 → rows 0..5 (the
    `<6` guards).
  - **Y build PDE_ONLY** (`solution/ymatrix.rs`): add `BuildOption::PdeOnly` —
    stamps **ALL_YPRIM** for PD **or SOURCE** (VSource) elements into the series
    handle; PC elements excluded (YMatrix.pas l.442-497). NCIM reads it back via
    the triplet dump (`coo_entries`) into `ncim_y/row/col`.
  - **NCIM helper** (new `solution/solution/ncim.rs`): port
    `NCIMSolutionHelper.pas` loop-for-loop — `NCIM_GetPowers` (Load ConstZ→ZBus
    else PQ; Gen model 3=PV/4=PQ/else Z), `NCIM_Do{PV,PQ,Z}Bus`,
    `NCIM_CalcInjCurr` (`I=Y·V`, first 6 deltaF=0), `NCIM_BuildJacobian` (fresh
    `RealSparseSet` each iter), `NCIM_GetNumGenerators`, `NCIM_UpdateGenQ`
    (PV↔PQ switching + Q-limits), `NCIM_Init`, `DoNCIMSolution` (repeat:
    CalcInjCurr→BuildJacobian→GetPowers→ApplyCurr→solve→`NodeV -= dV`→Converged→
    UpdateGenQ), `NCIM_Converged` (`max|deltaF| <= ConvergenceTolerance`).
  - **Generator** (`elements/pc/generator/`): `GenVars.delta_q_nom: Vec<f64>`,
    `vtarget`, `ncim_idx`, `NCIM_InitPVBusJac`, a `NCIM_ExPV` flag; GenModel 3
    (PV) / 4 (PQ) semantics + kvarMax/kvarMin. **VSource** `CalcInjCurrAtBus`.
  - **Dispatch**: `do_pflow_solution` match gains `NCIMSOLVE => do_ncim_solution`
    (Solution.pas l.1031-1037); `converged()` gains the NCIM branch (l.730-733);
    `check_controls` resets `ncim_ready=false` + early-returns when
    `system_y_changed && algorithm==NCIM` (l.1182-1186).
  - **Options** (`exec/set_cmd.rs`): add `NCIM` to `solve_alg` at ordinal 2
    (prefix `nc`); new `IgnoreGenQLimits`→`ncim_ignore_q_limit`,
    `NCIMQGain`→`ncim_gen_gain` (ExecOptions.pas l.794-797) + `Get` readback.
  - **Reports**: `Export Jacobian/deltaF/deltaZ`, `Show PV2PQ_Conversions`
    (numeric-token gates).
  - **Decks** (`tests/corpus/modes/ncim/`, all `oracle:"capi015"`,
    `pending:true` until the WP flips): micro PQ-only snapshot; PV-bus generator
    deck (Q-limit hit → PV→PQ via `Show PV2PQ_Conversions` token + iter ≤); midi
    IEEE123-class re-solve. Cross-check one on `oddie:r4088`. Iteration policy:
    Rust ≤ oracle (§1.3-1); first-divergence trajectory dump on any gap.

**WP-COV-PARKED (Refine_BusLevels parked-test closure) — LANDED (branch
wt-coverage).** Root-caused and closed the `#[ignore]`d
`circuit::coverage::tests::refine_bus_levels_reports_paths_on_radial` "infinite
loop": a **test-harness bug**, not a port bug. The test fed its whole 8-line
deck to ONE `Dss::command` call (`command` = Pascal `ProcessCommand`, one
command line) — the lines/loads after `new circuit.covtest …` were consumed as
extra parameters of the `new circuit` command (last `bus1=b5` re-based the
Vsource; no lines existed), so `CalcIncMatrix_O` yielded a 1-bus incidence
matrix (`cols=["b5"]`, `levels=[0]`) whose coverage plateau is 0 — on such a
degenerate input the state machine's sole exit (Circuit.pas:909) genuinely
never fires, faithfully to upstream. WP-AD.5's `Get_paths_4_Coverage` /
`get_longest_path` / `Normalize_graph` and WP-AD.1's `Calc_Inc_Matrix_Org` are
verified correct line-by-line vs r3723 Delphi AND empirically: fed line-by-line
the port bit-matches the official r3723 engine (Oddie probes 2026-07-16,
both < 50 µs wall): cov=0.5 → "0 new paths detected"/`Actual_Coverage
0.666666666666667`; default 0.9 → "2 new paths detected"/`1`. Tests un-ignored
+ a new default-coverage test, both pinning result strings, `ad.actual_coverage`
values, and the `get coverage` formatted strings to the official engine. Also
disproved the old "0.9 unreachable, plateau 5/6" hypothesis (`Buses_Covered`
are index spans; the sum overshoots `Sys_Size`) and corrected the
NOTE(upstream-quirk) at `get_paths_4_coverage` accordingly. No engine code
changed; no new decks.

**WP-U1.7 (NCIM solver) Stages 2–4 core — LANDED (branch `wt-u17`).** Spec = A1,
`Common/NCIMSolutionHelper.pas` (1047, FPC) + the Solution/YMatrix/Generator hooks.
Three commits on top of Stage 1:
- **State** (`solution/solution/state.rs`): `NCIMSOLVE=2`, `NCIM_PQ_NODE`/
  `NCIM_PV_NODE`, the ~15 `ncim_*` fields (1-based-with-slot-0 node arrays; the
  Jacobian's own 0-based `2*(i-1)` layout), and the `Converged` NCIM branch
  (`ncim_converged` = max|deltaF| ≤ ConvergenceTolerance).
- **Y build** (`solution/ymatrix.rs`): `BuildOption::PdeOnly` — stamps the FULL
  (`ALL_YPRIM`) primitive of every PD element **or** SOURCE into the series handle,
  PC elements excluded (Ymatrix.pas l.442-497); NCIM reads it back via `coo_entries`.
- **NCIM helper** (new `solution/solution/ncim.rs`, ~660 lines): loop-for-loop port
  of every `NCIMSolutionHelper.pas` routine — `NCIM_GetPowers` (Load ConstZ→ZBus
  else PQ; Gen model 3=PV/4=PQ/else Z), `Do{PV,PQ,Z}Bus`, `CalcInjCurr` (I=Y·V,
  first-6 deltaF=0), `BuildJacobian` (fresh `RealSparseSet`, `[B,G;G,−B]` blocks,
  swing identity via the `<6` guards, PV-bus `InitPVBusJac` placeholder cells),
  `GetNumGenerators` (PV-bus indexing + Q-limits), `UpdateGenQ` (PV↔PQ switching),
  `Init` (PDE_ONLY build + flat start), `DoNCIMSolution`. KLUSolveX `SetMatrixElement`
  is 1-based → mapped to the 0-based `RealSparseSet::set_element` by `−1`.
- **Generator** (`elements/pc/generator/mod.rs`): `delta_q_nom`/`ncim_idx`/`ncim_expv`
  (transient solver state, not copied by MakeLike — same convention as dynamics state).
- **Dispatch**: `do_pflow_solution` NCIMSOLVE → `do_ncim_solution`; `check_controls`
  resets `ncim_ready=false`+early-returns when `system_y_changed && algorithm==NCIM`.
- **Options** (`exec/set_cmd.rs`/`get_cmd.rs`/`tables.rs`, `dss_enum/registry/solution.rs`):
  `solve_alg` enum gains `NCIM` (ordinal 2, min-abbrev 2 = prefix `nc`);
  `IgnoreGenQLimits`→`ncim_ignore_q_limit`, `NCIMQGain`→`ncim_gen_gain` at ordinals
  129/130 (the `DSS_CAPI_ADIAKOPTICS` block is ifdef'd out of the capi oracle, so the
  NCIM options follow `NUMANodes=128`) + Get readback. `dump3_commands`: the two
  execoptions lines are dropped by the WP-U1.9 `run_deck_dump_exact_block_masked`
  0.15.x-options mask (integration fix at the wt-u17×wt-u19 merge — u17's
  hand-added golden lines were superseded by u19's uniform mask; the golden stays
  the pure 0.14.5-oracle text).
- **Validation**: `exec/tests/ncim.rs` — every electrical assertion now pinned
  against **capi015** NCIM captures (dss_capi 0.15.0b4 / SVN r4103; the pinned 0.14.5
  gate oracle has no NCIM), embedded golden-style, matched to <5e-11 V (faer-vs-KLU
  floor) under a 1e-6 V band: PQ, ConstZ, PV **regulating within Q-limits** (vpu=1.0,
  Q≈1217 kvar, reported `present_kvar` matched), PV **Q-limit → PV→PQ** (vpu=1.01,
  8 iters, Q=1500), and the **faithful shared non-convergence** (vpu=1.02: capi015
  NCIM also stalls at max iters at the identical `|genbus|=7343.55` fixpoint — pinned
  so a future silent "fix" that diverges from the oracle is caught). Plus warm-resolve
  stability and option round-trip. Gate green (fmt/clippy/`cargo test --workspace`).
- **Audit (Stages 2-4) findings addressed (branch `wt-u17`):**
  - PV-bus path is oracle-validated (above); the earlier "PV bus does not converge"
    concern is a **faithful upstream limitation**, not a port bug — capi015 NCIM fails
    on the same aggressive deck node-for-node.
  - The source bus sitting at the ideal EMF (`7199.56+0i`, no droop) under NCIM —
    flagged as an unported `VSource.NCIM_CalcInjCurrAtBus` bug — is the **correct**
    NCIM value (matches capi015 exactly). `NCIM_CalcInjCurrAtBus` is a *reporting*
    path (`GetCurrents` at the source terminal); it does **not** touch node voltages.
    The old vs-`Normal` self-consistency comparison was the wrong baseline and is
    replaced by the vs-oracle pins.
  - `NCIM_GetPowers` now persists `deltaQNom → Qnominalperphase` (Pascal l.121) so the
    reported model-3 generator Q matches the oracle (was a stale-nominal reporting
    divergence). `exec::Dss::generator_present_kw_kvar` reads the solved `(kW,kvar)`.
  - The two new exec-option help rows (129/130) render the catalog-miss placeholder
    (raw key) — verified empirically that the **pinned 0.15.7 catalog lacks both
    keys**, so `help_catalog.rs` is not stale; identical to the `LongLineCorrection`
    precedent, resolved in the acceptance help-catalog regeneration pass.
- **Remaining (Stage 3 infra / reporting — keeps the gate green because NCIM only
  activates on `Set Algorithm=NCIM` and no corpus deck does yet):**
  1. Fold the above decks into the live-gate `tests/corpus/modes/ncim/` matrix
     (`oracle:"capi015"`, §1.7 manifest + population.lock) — the numerics are already
     oracle-pinned in `exec/tests/ncim.rs`; this is the corpus/manifest plumbing.
  2. `Export Jacobian/deltaF/deltaZ` + `Show PV2PQ_Conversions` reports (numeric-token
     gates) and `VSource.NCIM_CalcInjCurrAtBus` (swing-source reported *currents* under
     NCIM — reporting-only, node voltages already correct).
  3. `oddie:r4088` cross-check of one deck (report-only).

**WP-H015 — 0.15.x property-table allowlist (`PROPS_015X`).** Resolves Rung-1
remaining-tail item 1. The corpus property-parity gate
(`harness::compare_all_properties`) asserts the Rust property-table **shape**
(count + name order) against the oracle capture; the pinned default oracle is
dss_capi **0.14.5**, so a deliberately ported 0.15.x-added property (Line
`EpsRMedium`/`HeightOffset`/`HeightUnit`/`Conductors`; RegControl `Idle`/
`IdleReverse`/`IdleForward`/`FwdThreshold`; Transformer+AutoTrans `BHpoints`/
`BHcurrent`/`BHflux`; LoadShape `Mode`) would break nearly every default-oracle
deck's shape assert.
- **Decision (adopted, coordinator-approved): a named per-class allowlist** —
  `PROPS_015X: &[(&str, &[&str])]` in `tests/harness/mod.rs`. A Rust-side prop
  whose `(class, name)` is in the table **and** whose name is absent from the
  oracle capture is excluded from the count/order/name/value walk (a §1.3-style
  *shape* relaxation, NEVER a value-tolerance change). Handles **inserted** props,
  not only trailing (the Rust list is filtered to what a 0.14.5 capture can know,
  then compared position-for-position). If the capture DOES contain the prop
  (capi015-regenerated), it is NOT excluded → full name+value compare, so capi015
  decks keep pinning the new props' values. A non-allowlisted extra/missing/
  misordered prop still fails exactly as before; the count-mismatch panic names
  the allowlist for triage.
- **Implementation:** extracted the list-comparison core into `compare_prop_lists`
  (allowlist injectable) so the shipped-empty table is validated by 6 inline
  `props_015x_tests` self-tests with synthetic data (trailing extra passes;
  inserted extra passes with order preserved; non-allowlisted extra panics;
  allowlisted-present-in-oracle value mismatch panics + match passes; missing +
  misordered panic).
- **Surface survey (item 3):** `compare_all_properties` is the **only** surface
  that asserts Rust property count/order against a 0.14.5-pinned capture.
  `props_roundtrip.rs` + the props goldens (`golden_feeders_controls`,
  `scenario.rs`) iterate only the props present in the (0.14.5) golden — no
  Rust-side shape assert. `save_roundtrip.rs` round-trips through our own engine
  (node-V + discrete state), not oracle property tables. `population_lock.rs`
  fingerprints manifest membership/rigor, not property-table shape. None need the
  allowlist.
- **Ships EMPTY** — rows land with the three sibling Rung-1 WPs that port each
  property (one class per line, merge-friendly; duplicate class rows OR).
- Docs: `tests/TOLERANCE_NOTES.md` §"0.15.x property-table allowlist (shape
  relaxation)"; `TESTING.md` pointer. Gate green (fmt/clippy/`cargo test
  --workspace`) — nothing moves, the table is empty.

**GAPS (WPG.*), Phase 8, Phase 7.** The per-WP GAPS_PLAN records (WPG.1/10/12/13/
14/15/16/17/18/19/20/21 + CIM XML export stages) are archived in
**`docs/phase-records/gaps.md`**. Phase 8 (reporting/executive) is COMPLETE — detail
in **`docs/phase-records/phase-8.md`**. Phase 7 (DER/protection/line-constants/
harmonics/dynamics) is COMPLETE on `phase-7-extended-elements` (not merged to `main`)
— roll-up in §1e and **`docs/phase-records/phase-7.md`**.

### Standing open follow-ups (actionable)
- **WP-U1.2 row D3** — port with its overload deck (B3-r3723 landed under WP-U1.6).
- **WP-U1.6 remaining** (branch wt-u16 §UPGRADE): C5 RegControl FwdThreshold+idle
  props, C6 Transformer BH props, C5-r3723 LoadShape Mode index, D11 CapControl
  TIMECONTROL effElement, D13 LoadShape MMF, C4 `Solve all` alias — see the
  WP-U1.6 §UPGRADE block for the property-count/deck entanglements.
- **ckt24 RegControl/LDC `SubXFMR`** ~4.7e-5 rel tap-current — now floored as
  ultra-switch conditioning (CF-D), watch on re-touch.
- **Monitor modes 8/10/12** (winding I/V, LL) have a deferred stub sample body
  (`sample.rs` `_ => return`) while `header.rs` declares `record_size` → a monitor
  using them PANICS (OOB in `channel()`); uncovered pending the monitor-winding port.
- **UPFC modes 2/3/5**, `midi_relay_dist` deferred (budget); Kersting4wire #567
  UserModel decks parked (no oracle channel tolerates the DoSimpleMsg).
- **actor / parallel mode** + `CapControl.ControlSignal` + UTF-8-BOM edge cases —
  MULTITHREADING M2+ (actor) / GAPS follow-up.

---

### Gate state (all green)
```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace      # 45 test binaries, 0 failures (2026-07-16 round):
                            # dss-core lib 1171, golden_reports 197 (incl. the
                            # capi015 seasonal pair), corpus_live (292
                            # solvable_now cases live-compared; modes family 58)
                            #   (corpus_live_solvable_cases_match_oracle +
                            #    solvable_now_has_multistep_depth run
                            #    UNCONDITIONALLY — the pinned oracle MUST be
                            #    installed (it fails, not skips, without it);
                            #    only corpus_live_classify is opt-in, via
                            #    DSS_LIVE_CLASSIFY=1 — the growth/classify probe),
                            # dss-parser 62+1, dss-sparse 15
                            #   (6 complex SparseSet + 9 real RealSparseSet —
                            #    WP-U1.7 Stage 1, the NCIM Jacobian path)
```

### Phase 5 gate — green  *(detail → `docs/phase-records/phase-5.md`)*
- `golden_feeders_controls.rs`: the unmodified IEEE13/IEEE37/IEEE123 masters
  (controls active) + `ieee34mod1` match the Phase-0 goldens — converged + total
  iterations exact, `YNodeOrder` exact, RegControl `tap_number` / capacitor
  `states` exact, final taps 1e-12 rel (the integer `tap_number` is the exact
  discrete check), V/I/P 1e-6, and every element's full property dump.
- `golden_phase5.rs` vs `tests/golden/phase5/*.json` (`gen_phase5.py`):
  `daily_ieee13`, `duty_2bus`, `eventlog_ieee13`, `capcontrol_micro` — per-step
  `dblHour` + iteration counts exact, **event logs line-for-line** (normalized),
  per-step V 1e-6 (the shape-scaled `Yeq` restamp per Y build, `a6903f1`).

### Checkpointed-model gate (`crates/dss-core/tests/golden_checkpoints.rs`) — green
- `gen_checkpoints.py` → `tests/golden/checkpoints/<scenario>.json` (schema 2,
  one file per scenario; the gate runs every file in the directory, so adding a
  scenario is just adding a file). Unlike the
  other command-replay gates (which compare only converged outputs), this one
  captures the **assembled electrical model after every committed time step** —
  the unfactored system Y, selected element YPrim blocks, the injection vector,
  node voltages, and discrete control state — and compares each to the oracle.
  A stale Y/YPrim fails at the step and matrix entry it first goes wrong, not as
  downstream register drift. Scenarios: `micro_yeq_steps` (control-free daily,
  full-CSC per-step pin), `ieee13_daily` (24-step daily with regulator tap
  changes — full CSC + fingerprint; the direct regression guard for the
  "frozen load Yeq" bug: reverting commit `a6903f1` makes it fail at step 6,
  `Y[634.1]`), `ieee123_snap` (large-feeder fingerprint-only + selected YPrim
  path). Tolerances: `tests/TOLERANCE_NOTES.md`. The assembled Y is compared
  **unfactored** so the `dss-sparse` row equilibration is out of scope.

### Live corpus oracle gate (`crates/dss-core/tests/corpus_live.rs`) — opt-in
See `CORPUS_TEST_PLAN.md`. The whole `electricdss-tst` corpus is **vendored** into
`tests/corpus/electricdss-tst/` (1544 files, 122 MiB; `tools/corpus/vendor.py`,
`.git` excluded, with `SHA256SUMS` + `README.md` provenance) so tests no longer
depend on the temporary `.inputs/electricdss-tst`.
- **Manifest accounting (always-on).** Every `.dss` (915) is in exactly one
  manifest under `tests/corpus/manifests/` (`solvable_now`, `skipped_unsupported`,
  `skipped_oracle_issue`, `skipped_needs_investigation`, `missing_dependency`,
  `not_an_entry_point`). `corpus_manifest.rs` enforces the bijection — no silent
  omissions — and runs in the normal `cargo test`: adding/removing a `.dss` fails
  it until the file is classified.
- **Live comparison (runs unconditionally in `cargo test`; the pinned oracle must
  be installed).** For each of the **84** `solvable_now` cases the gate
  compiles+solves on the Rust engine and on the pinned dss-python oracle
  (`tools/oracle/oracle_server.py`, a
  one-shot subprocess over JSON), and compares the full assembled model per step —
  node order, **full** system Y (entry-by-entry, no fingerprint substitution),
  node voltages, **every** element's currents/powers, selected YPrim blocks (a
  guard fails the case if the oracle returns no YPrim for a named selected
  element), the injection vector, and discrete state — reusing the `harness/mod.rs`
  comparators and the checkpoint gate's tolerance policy.
  - **Three control-diverse 24-step daily runs** — `IEEE13Nodeckt` (wye gang
    reg), `ieee37` (delta, open-delta LDC reg bank) and `IEEE123Master` (multiple
    cascaded reg banks) — each with a meter + three monitors (modes 0/1/2) +
    selected elements, so the **multi-step per-step**, **YPrim**,
    **monitor-channel** and **EnergyMeter-register/zone** paths are all exercised
    live (`compare_monitor`/`compare_meter`, the *same* comparators
    `golden_phase6.rs` now routes through, gated per case by
    `check_meters_monitors`). Incidental master-defined monitors are *not*
    compared — the pinned oracle returns a phantom `Channel(i)` for an unsampled
    monitor (see `tests/TOLERANCE_NOTES.md`).
  - The **IEEE 8500-Node master is promoted** (snapshot; `post: Set
    Maxiterations=20` to converge — the bare probe didn't, which is why the
    classifier had parked it), so the full 8531-node Y, every element's I/P, and a
    YPrim block are compared live at scale (complementing the always-on
    `golden_ieee8500.rs` golden, whose `compare_discrete` also pins the full
    1190-transformer tap set here).
  - **Depth is guarded always-on.** `solvable_now_has_multistep_depth` (no oracle)
    asserts `solvable_now` keeps ≥1 multi-step `check_meters_monitors` case and ≥1
    case with selected elements, so the deep coverage can't silently revert to
    snapshots. The solvable + classify tests **auto-skip (pass)** without the env
    var / oracle, so `cargo test --workspace` stays green everywhere; the
    **`live-oracle` GitHub Actions job** installs the pinned oracle (PIN.txt) and
    runs the **whole `corpus_live` binary** (not a name filter that could green on
    zero matched tests). The oracle server hard-asserts **both** dss-python 0.15.7
    **and** engine 0.14.5 (PIN.txt). No goldens are written; the oracle is
    consulted live.
- **Growth.** `DSS_LIVE_CLASSIFY=1 corpus_live_classify` probes the
  `skipped_needs_investigation` candidates with the full comparison and writes
  `tmp/classify_report.json`; `tools/corpus/apply_classify.py` promotes the
  passing cases into `solvable_now` (and routes oracle/engine failures to the
  right skip bucket). `tools/corpus/coverage_report.py` →
  `tests/corpus/COVERAGE.md` tracks the burn-down toward 100% of entry points.

### Phase 4 gate (`golden_feeders.rs`) — green  *(detail → `docs/phase-records/phase-4.md`)*
The controls-off IEEE13/37/123 variants (`gen_phase4.py`) match `phase4.json`
(pinned oracle): converged + iterations exact (3/3/3), `YNodeOrder` exact
(41/117/278), V 1e-6, every element's I/P 1e-6 (creation order), total
power/losses 1e-6. The Phase-3 `golden_slice.rs` (13 scenarios) stays green; the
CLI runs the real masters (`cargo run -p dss-cli -- script.dss`).

---

## 1b–1d. Completed-phase records (archived)

The full work-package logs for the completed, merged phases (and the completed
Phase-7/Phase-8 work packages) live under `docs/phase-records/` to keep this
handoff lean. They are frozen history, superseded only by the code and tests.
The two roll-ups that back the compact §1e/§1f frontier below:
[`phase-7.md`](docs/phase-records/phase-7.md) (the WP7.1–7.10 record) and
[`phase-8.md`](docs/phase-records/phase-8.md) (the completed WP8.1/8.2/8.3-step
detail). The per-phase entries:

- **Phase 3** — the vertical-slice file-by-file map (circuit model / element base /
  solution / executive / property engine) — still the architectural reference §2
  points to. → [`docs/phase-records/phase-3.md`](docs/phase-records/phase-3.md)
- **Phase 4** — PD elements (Transformer/Capacitor/Reactor), catalog objects
  (LineCode/XfmrCode/GrowthShape), the Line→LineCode fetch path, parse-only
  RegControl/CapControl, the `define_properties!` macro, and the controls-off
  feeder gate. → [`docs/phase-records/phase-4.md`](docs/phase-records/phase-4.md)
- **Phase 5** — controls + time series: XYcurve / LoadShape / TShape /
  PriceShape, ControlQueue + event log, RegControl/CapControl behavior, the
  control loop (`Sample_DoControlActions`), and the time-series solve modes.
  → [`docs/phase-records/phase-5.md`](docs/phase-records/phase-5.md)
- **Phase 6** — meters + topology: CktTree, Generator, Monitor, EnergyMeter +
  zone build, registers/TakeSample, reliability (`RelCalc`), Sensor + load
  allocation, the GenDispatcher/StorageController/AutoAdd/ReduceAlgs skeletons,
  and the 8500-node gate. Merged to `main` `b98223a`.
  → [`docs/phase-records/phase-6.md`](docs/phase-records/phase-6.md)
- **Phase 7 WP7.1** (Line constants & geometry) — the Carson engine, the
  WireData/CNData/TSData/LineSpacing/LineGeometry catalog, Line's geometry/spacing
  Carson path, the corpus migration, and the offline geometry golden. **Complete +
  gate-green on the `phase-7-extended-elements` branch (not yet merged);** the live
  §1e keeps a step summary + the tracked-open plural-cable note.
  → [`docs/phase-records/phase-7-wp1.md`](docs/phase-records/phase-7-wp1.md)
- **Phase 7 WP7.2** (Protection) — Fault, SwtControl, Fuse, Recloser, Relay (9
  sub-types), reliability activation (`HasOCPDevice` + live `RelCalc`), and the
  step-4 gate (the `phase7_protection` trip/reclose golden, the `Open`/`Close` exec
  verbs, the SwtControl corpus migration). **Complete + gate-green on the
  `phase-7-extended-elements` branch (not yet merged);** the live §1e keeps a
  per-step summary + the Phase-7 carry-forward rules + the `DG_Prot_Fdr` tracked-open.
  → [`docs/phase-records/phase-7-wp2.md`](docs/phase-records/phase-7-wp2.md)
- **Phase 7 WP7.3** (DER A) — `DynamicExp` (the diff-eq catalog object + its RPN
  expression interpreter), `InvBasedPceData` (the shared inverter PC-element base),
  and `PVSystem` (the power-flow PV element + zone admission + the Monitor mode-3
  fix + the GFM loud-abort). **Complete + gate-green on the branch (not yet merged).**
  → [`docs/phase-records/phase-7-wp3.md`](docs/phase-records/phase-7-wp3.md)
- **Phase 7 WP7.4** (DER B) — the `Storage` element (the charge/idle/discharge state
  machine + integrated SOC) and the real `StorageController` fleet/dispatch (replacing
  the WP6.8 skeleton), plus the Storage-specific YPrim-rebuild fix. **Complete +
  gate-green on the branch (not yet merged).**
  → [`docs/phase-records/phase-7-wp4.md`](docs/phase-records/phase-7-wp4.md)
- **Phase 7 WP7.5** (DER C, steps 1–4) — `RollAvgWindow`, the full `InvControl` (8
  modes + LPF/RiseFall + MonBus, both PVSystem and Storage DERs), `ExpControl`
  (the adaptive-`Vreg` volt-var control), and the step-4 corpus burn-down review.
  **COMPLETE + gate-green on the branch (not yet merged).**
  → [`docs/phase-records/phase-7-wp5.md`](docs/phase-records/phase-7-wp5.md)
- **Phase 7 WP7.6** (Harmonics, steps 1–3) — the harmonics solve mode: the
  current-source family (VSource + Load) + the `SolveHarmonic`/`SolveHarmonicT`
  driver, the Thevenin DER family (Generator/PVSystem/Storage behind their
  subtransient reactance), and the monitor harmonic header + the `Set mode=`
  monitor/meter reset (Pascal `Set_Mode` tail); harmonics corpus burn-down is 0
  migratable (Phase-8/`Isource`/FaultStudy-blocked). **COMPLETE + gate-green on the
  branch (not yet merged).**
  → [`docs/phase-records/phase-7-wp6.md`](docs/phase-records/phase-7-wp6.md)
- **Phase 7 WP7.7** (Dynamics core, steps 1–3b cont.) — the `SolveDynamic`
  predictor/corrector driver + per-element dynamics state machinery for Generator /
  PVSystem / Storage / IndMach012, Monitor mode 3, the `Open`-verb fix, the
  `set_ITerminalUpdated` stamp sweep, and the DynEqPCE user-`DynamicExp` integration
  for all three PCE families. **✅ COMPLETE** (steps 1–4; GFM deferred,
  tracked-open). The completed-step detail (incl. all audit follow-ups) is archived;
  the live §1e keeps the concise per-step summary.
  → [`docs/phase-records/phase-7-wp7.md`](docs/phase-records/phase-7-wp7.md)

---

## 1e. Phase 7 record (branch `phase-7-extended-elements`) — ✅ COMPLETE

The full per-WP roll-up (WP7.1–WP7.10 step summaries, decisions, audits, gate
detail, and the cross-cutting carry-forward rules) is **archived** at
[`docs/phase-records/phase-7.md`](docs/phase-records/phase-7.md); the deeper
per-step detail lives in the sibling `phase-7-wp{1..7}.md` archives. Phase 7 is
COMPLETE + gate-green on the branch, **NOT merged to `main`** (per-phase merge =
explicit-request-only HARD STOP). Headline: **WP7.1** line constants & geometry,
**WP7.2** protection (Fault/Fuse/Recloser/Relay/SwtControl + reliability
activation), **WP7.3–7.5** DER (DynamicExp/InvBasedPCE/PVSystem;
Storage/StorageController; InvControl/ExpControl), **WP7.6** Harmonics, **WP7.7**
Dynamics core (SolveDynamic + Generator/PVSystem/Storage/IndMach012 + DynEqPCE),
**WP7.8** Converter/FACTS (VSConverter/VCCS/UPFC+UPFCControl/ESPVLControl),
**WP7.9** FaultStudy (AutoAdd/Monte/LD/Feeder empirically deferred — zero corpus
cases), **WP7.10** exit. Retro audit (WP7.7 step 4 → WP7.8): no Critical/Major
correctness bug. Two real port bugs found+fixed in WP7.5 (the cross-step
`FFlagVWOperates` latch + the missing post-`DoPendingAction` `LoadsNeedUpdating`),
each a [[dont-rationalize-conditioning]] instance. **Tracked-open** (both
Plot-blocked, zero corpus payoff): GFM grid-forming mode + Generic/TD21 relay
`Sample`. lib **713**; `solvable_now` **88** at Phase-7 exit.

---

## 1f. Phase 8 record (`PHASE8_PLAN.md`) — ✅ COMPLETE (2026-07-10; WP8.8 exit record in §1)

Execution plan: **`PHASE8_PLAN.md`** (WP8.1–WP8.8, the reporting/output + full
executive layer; per-step cadence = `PHASE8_PLAN.md §0`). Phase 8 is almost
entirely *read-and-format* — no new electrical math, no new solve mode; the risk
is faithful report layout and **not silently faking output**. The full per-step
log for the **completed** work (decisions, audits, gate detail, the real-gap
write-ups) is **archived** at
[`docs/phase-records/phase-8.md`](docs/phase-records/phase-8.md). Headline:
**WP8.1** report infrastructure, **WP8.2** solution exports, **WP8.3** device/
reliability + logs, **WP8.4** `Show` reports, **WP8.5** `Save circuit`, **WP8.6**
executive verbs, **WP8.7** ReduceAlgs, **WP8.8** exit + corpus classify — all
COMPLETE, gate-green. The full per-step WP8.1–WP8.4 detail is archived there.

---

## 2. What Phase 3 built (file-by-file map) — archived

The Phase-3 vertical-slice **file-by-file architectural map** moved to
[`docs/phase-records/phase-3.md`](docs/phase-records/phase-3.md) (2026-06-21) to
keep this handoff lean. It is still the architectural reference §3/§4/§5 below
build on — only its location changed.

---

## 3. Key design decisions & rationale

### 3.1 Element storage stays in the executive; the solver sees `ElemStore`
`Vec<Box<dyn DssObject>>` per class; the circuit holds `Vec<ElemRef>` lists;
solution machinery walks them through `ElemStore` + `as_ckt_element_mut()`.
Zero unsafe, no double ownership.

### 3.2 Signal flags instead of `ActiveCircuit` globals *(load-bearing)*
Elements set `cd.signal_bus_name_redefined`/`cd.yprim_invalid`; the executive
propagates after the edit loop. Equivalent because nothing reads the globals
mid-edit (first reader is `BuildYMatrix` at solve time).

### 3.3 Compensation-current loads
Loads are stamped into Y **and** inject `Yprim·V − model current`; iteration
equality in the gates is the regression test for this.

### 3.4 Parse-time reference snapshots + deferred cross-element writes (Phase 4)
Pascal resolves object references mid-parse against live pointers and lets
recalc *read* (and `Set_TapNum` *write*) the target at any time. The Rust edit
loop holds only a read view of foreign classes, so:
- reads needed later (control `RecalcElementData` at `EndEdit`) come from a
  `RefSnapshot` captured at resolution time — same staleness semantics as
  Pascal (a control refreshes only on its own recalc);
- writes (`TapNum`) become queued `RefAction`s the executive applies right
  after the edit, with the writer keeping its snapshot in sync via the
  identical clamp. Nothing observes the target in between.

### 3.5 `NOT_PORTED` property flag
Catalog/machinery references that belong to later phases hard-error on set —
a script that needs unported machinery cannot produce silently-wrong numbers.

### 3.6 Controls are invisible to Y
`ElemKind::Control` elements join the device list (so `ProcessBusDefs` walks
them in creation order — node order matches the oracle) but never the PD/PC
lists; `yprim` stays `None` and the Y build skips them.

---

## 4. Empirical oracle facts (cumulative highlights)

- `?`, `Edit`, `~`, `Solve`, `Set`, `Get` are **circuit-gated** (error 301).
- `Solution.Iterations` = the total over control iterations, assigned at the
  end of `SolveSnap`.
- `YNodeOrder` = `ProcessBusDefs` allocation order over enabled elements in
  creation order — **including control elements** (their bus is set in
  `RecalcElementData`; they add no nodes in the IEEE feeders).
- The `DoubleSymMatrixProperty` getter is broken upstream (reads a field
  address as the array) — Capacitor `CMatrix`, Reactor `RMatrix`/`XMatrix`
  always dump garbage; canonicalized to zeros on both sides.
- `Circuit.Losses` skips shunt PD elements; `Circuit.TotalPower` = Σ sources
  `Power[1]`·1e-3 (not negated); element `Powers` = `GetPhasePower`·1e-3 over
  all conductors/terminals.
- RegControl `TapNum` get/set maps tap↔integer through the *controlled
  winding's* (TapWinding) Min/Max/Increment: `tapnum=5` on a 32-tap winding
  moves the tap to 1.03125; reads back 5. `winding=` resets `TapWinding`.
- CapControl `type=time` (and `follow`) forces `Terminal=1` and monitors the
  capacitor itself; a missing `capacitor=`/`element=` raises (303); `vbus=`
  set during parse warns "Did you wait until buses were defined?" and reverts
  the flag (bus list doesn't exist yet) — faithfully reproduced.
- Controls-off feeders: IEEE13 41 nodes, IEEE37 117, IEEE123 278; all solve in
  exactly 3 fixed-point iterations.

---

## 5. `TODO(compat)` / deferrals

Grep `rg "TODO\(compat\)"` for the full marker list (39 sites). Notable:
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

## 7. Phase 7 — inherited deferrals & architecture in place

> **The current frontier** (active step, branch, what's next, commit state) lives
> in the header up top and in **§1e** — not restated here, to avoid the two drifting
> apart. This section is the stable Phase-7 reference: what the phase inherits and
> what is already wired for it. Execute per `PHASE7_PLAN.md §0` (six
> independently-gated sub-blocks, risk-ascending: line constants → protection →
> DER → harmonics → dynamics → faultstudy/AutoAdd-modes/`Feeder`).

**What Phase 7 inherits / must finish (deferrals Phase 6 left explicit):**
- **DER classes** `Storage`/`PVSystem` (+ `InvControl`/`ExpControl`) and the real
  `StorageController` behavior — ✅ **all done**: `PVSystem` (WP7.3), `Storage` +
  `StorageController` (WP7.4), and `InvControl` + `ExpControl` (WP7.5) (the WP6.8
  StorageController parse-only skeleton is replaced by the real fleet dispatch;
  `is_zone_pce` now admits Storage/PVSystem).
- **Protection** `Relay`/`Recloser`/`Fuse`/`SwtControl`/`Fault` — ✅ **done
  (WP7.2)**: all five classes ported on the control sweep, the `Open`/`Close` exec
  verbs landed, and an enabled Relay/Recloser/Fuse sets `Flg.HasOCPDevice` so
  `RelCalc` no longer aborts (#52902) and `GetOCPDeviceType` is live — the
  SAIFI/SAIDI/section math runs on a protected zone.
- **Line constants** `WireData/CNData/TSData/CableData/LineSpacing/LineGeometry`
  + Carson — ✅ **done (WP7.1)**: Line's
  `geometry`/`spacing`/`wires`/`cncables`/`tscables` resolve and drive the Carson
  Z/Yc (one plural-cable reset + the `DG_Prot_Fdr` ~3e-5 line-Y precision case
  tracked-open, §1e).
- **Harmonics** (`DoHarmonicMode` for VSource/Load + Generator/PVSystem/Storage,
  the frequency sweep + the harmonic monitor header) — ✅ **done (WP7.6)**.
- **Dynamics** (Generator/Storage `DoDynamicMode`, state vars beyond names/count)
  + `MakePosSequence` everywhere; Monitor modes 3/4/7/8/10/12 build their header
  but defer the sample body; Transformer GIC (<0.51 Hz) elements — WP7.7+ / Phase 9.
- **AutoAdd solve mode** (`circuit/auto_add.rs` skeleton) — needs aux-current
  injection (`UseAuxCurrents`) + meter-register sampling in the solve loop; the
  options round-trip but the mode keeps its "Unknown solution mode" error.
- **ReduceAlgs** zone reduction — blocked on the unported `TLineObj.MergeWith`.

**Architecture already in place for Phase 7:** the control loop dispatches
through `ElemStore::{obj,pair_mut,triple_mut}` + `DssObject::as_any_mut`; the
meter/monitor `sample_all_monitors_and_meters`/`end_of_time_step_cleanup` hooks
have real bodies; the zone-build dispatcher (`solution/meters/mod.rs`) fires from
`build_y_matrix` after bus reprocessing; `TakeSample`/`Integrate` + the
reliability fault-rate sweep are ported. Still Phase 8: the `SystemMeter`
register core and all demand-interval/phase-voltage/`Show`/`Export` files.
