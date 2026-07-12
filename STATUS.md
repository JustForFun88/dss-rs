# dss-rs — Project Status & Session Handoff

> **Purpose of this file:** a living snapshot so a fresh session can resume
> without re-deriving context. It records *what is done*, *what was decided*,
> and **why**. It is **not** authoritative for the plan itself — that is
> `PORTING_PLAN.md` (roadmap + binding decisions) and `CLAUDE.md` (conventions
> + the green-gate rule). Read those two first; then read this for the current
> frontier.

Last updated: 2026-07-12 (merged `upgrade-rung1` — U0.2 sweeps + WP-U1.1 + WP-U1.2
numeric long tail — with the post-acceptance main: corpus CF/CF2/coverage rounds,
families 47/92/49, JSON export, FIX-DIRECT, LINE-DEEP; both records below).

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
- **WP-U1.2 (numeric long tail)** — rows B2/D1, D7, D6, B1, D8 landed; **remaining:
  D3** (report-only spacing ratings — needs an overload-report deck) and **B3-r3723**
  (Load.GrowthFactor Year=0 — needs a growthshape + multi-hour year-0 run). See the
  resume note under §UPGRADE below.
- **Open follow-up — B5 GFM `Isc1` ×1000 (deferred).** Adopting the r4133 `Isc1`
  change unmasks a **pre-existing Rust GFM power-flow injection-vs-YPrim consistency
  gap** (injection does not track `YPrim·Vset`; the Pascal op-point is Isc1-invariant,
  the Rust one is not). Needs a dedicated GFM WP, out of numeric-long-tail scope.
- Part II A-Diakoptics AD.2/AD.3 progressing on a separate `part2-adiakoptics`
  branch (not in this main).

**SKIPPED-SWEEP (branch `skipped-sweep`, 2026-07-12).** Gave every in-scope entry
in `skipped_needs_investigation.json` a real disposition (19 entries; the 2
`upgrade_straddle_gfm_wp` + 1 `upgrade_straddle_u16_dynexp` decks were left to their
dedicated WPs). Probed each on the official EPRI engines (r3723/r4088/r4133 via the
Oddie bridge) and the pinned 0.14.5 oracle; verified Rust behaviour first-hand.
**7 promoted to `solvable_now`, 12 kept parked with refreshed evidence.**

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
- **Kept parked (refreshed):** the 3 `4wire-Delta` user-model decks (converged state
  bit-identical but Rust needs 3 iters vs the oracle's 2 — a genuine model=6
  fallback-vs-DLL trajectory difference, NOT fudgeable); `vsctest` + `Torn_Circuit`
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
0.14.5→0.15.x delta). Full detail: **`docs/phase-records/upgrade-rung1.md`**.
- **Resume note (WP-U1.2 remaining).** Rows **D3** (report-only spacing ratings —
  overload-report deck) and **B3-r3723** (Load.GrowthFactor Year=0) still to port;
  the golden engine switch (`gen_checkpoints::check_pin` `DSS_ORACLE_ENGINE`) and the
  same-commit flip/regen/retire workflow are proven. **B5**'s GFM gap is the one hard
  blocker (a control-consistency bug, not a numeric constant) — needs a dedicated GFM
  WP. NB the modes manifest is NOT `json.dumps`-round-trippable (mixed manual
  `\uXXXX` escaping + CRLF) — append new cases with a surgical text edit.

**GAPS (WPG.*), Phase 8, Phase 7.** The per-WP GAPS_PLAN records (WPG.1/10/12/13/
14/15/16/17/18/19/20/21 + CIM XML export stages) are archived in
**`docs/phase-records/gaps.md`**. Phase 8 (reporting/executive) is COMPLETE — detail
in **`docs/phase-records/phase-8.md`**. Phase 7 (DER/protection/line-constants/
harmonics/dynamics) is COMPLETE on `phase-7-extended-elements` (not merged to `main`)
— roll-up in §1e and **`docs/phase-records/phase-7.md`**.

### Standing open follow-ups (actionable)
- **B5 GFM injection-vs-YPrim consistency gap** — dedicated GFM WP (above).
- **WP-U1.2 rows D3 + B3-r3723** — port with their overload/growthshape decks.
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
cargo test --workspace      # dss-core lib 748, golden_feeders 1,
                            # golden_feeders_controls 4, golden_phase5 1,
                            # golden_phase6 1, golden_phase7 1,
                            # golden_phase7_protection 1, golden_phase8 130,
                            # golden_checkpoints 1, golden_ieee8500 1,
                            # golden_reliability 1, golden_allocation 1,
                            # golden_gendispatcher 1, golden_autoadd_reduce 1,
                            # golden_slice 2, golden_smoke 3, props_roundtrip 1,
                            # corpus_manifest 1, corpus_live 3 (168 solvable_now
                            #   cases live-compared)
                            #   (corpus_live_solvable_cases_match_oracle +
                            #    solvable_now_has_multistep_depth run
                            #    UNCONDITIONALLY — the pinned oracle MUST be
                            #    installed (it fails, not skips, without it);
                            #    only corpus_live_classify is opt-in, via
                            #    DSS_LIVE_CLASSIFY=1 — the growth/classify probe),
                            # dss-parser 62+1, dss-sparse 5
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
