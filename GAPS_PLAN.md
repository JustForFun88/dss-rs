# GAPS Plan — closing the test-blocked deferrals (synthesized decks + porting WPs)

> Companion to `PORTING_PLAN.md`; same rules of engagement as
> `PHASE4_PLAN.md` §0 / `PHASE7_PLAN.md` §0 / `PHASE8_PLAN.md` §0 (Pascal is the
> spec; probe the oracle, never guess FPC semantics; `TODO(compat)` /
> `NOT_PORTED` discipline; goldens regenerated **manually** with the pinned
> oracle `tools/golden/PIN.txt`).
>
> **Stop-and-confirm cadence (same as `PHASE8_PLAN §0`):** after each small
> step (a WPG, or a self-contained stage of one — e.g. WPG.15's stages A/B/C)
> run the per-step ritual below **autonomously, without pausing between its
> sub-steps**; the single stop point is at the very end — then wait for the
> user's explicit confirmation (unless the user authorized several WPs in one
> pass).
>
> **Per-step ritual (do every step, in order, without being told):**
> 1. **Gate green** — `cargo fmt --all --check`; `cargo clippy --workspace
>    --all-targets -- -D warnings`; `cargo test --workspace` (runs **all**
>    goldens + the always-on live corpus gates). No `#[ignore]`, no
>    name-filter that could green on zero matches. For a WPG this includes
>    its exit criteria: the deck's `pending` flipped to `false`, the live
>    compare green at the family tolerance, the deck **graduated** to its
>    permanent family (§3.1). A red test blocks the commit.
> 2. **Update `STATUS.md`** (the §1 frontier + the GAPS record), **commit**
>    (code + STATUS together).
> 3. **`/audit-code` + `/audit-tests` in parallel** — two **fresh independent
>    agents, never forks** (a scoped agent reviews more sharply than one
>    buried in your context). Each gets a self-contained brief: the step's
>    commit range (`<sha>^..HEAD`) or WPG label, the diff, the authoritative
>    Pascal units + plan/STATUS sections, and the binding rules (the PIN,
>    `TODO(compat)`/`NOT_PORTED`, the §2/§3 deck-validation protocol, the
>    oracle is the spec). Auditors are **read-only** and return findings
>    only; **you** settle each finding against the pinned oracle, fix what is
>    real, note the follow-up in `STATUS.md`, re-run the gate, commit. A
>    finding deliberately not fixed is **recorded in STATUS**, never dropped;
>    if an audit finds nothing, skip its commit (no empty commits). For a
>    trivial sub-step the inline audit skill is allowed.
> 4. **`STATUS.md` full review** — read it end to end; sync whatever the step
>    made stale (no two places disagreeing), archive dead weight to
>    `docs/phase-records/`, dedup restated paragraphs; `docs:` commit if
>    anything changed (gate re-run first).
> 5. **Only now stop** and report **in Russian** (code, identifiers, commit
>    messages and STATUS stay English): what landed, what the audits found
>    and how it was settled, gate status, next step.
>
> **What this plan is.** Three kinds of unported work, one closure plan:
>
> 1. **Test-blocked feature deferrals** (§1). During Phases 4–7 a set of
>    features was deferred with the empirical rule "port only if a corpus case
>    needs it" — and the vendored corpus (`tests/corpus/electricdss-tst`) has
>    **no deck** for them, so they stayed unported (the WP7.9
>    AutoAdd/Monte/LD decision and its smaller cousins). `PHASE8_PLAN.md` §1
>    names that rule the anti-pattern and mandates **synthesized fixtures**:
>    *test-absence is never a reason to defer a port*.
> 2. **Unported element classes** (§1b). A registry diff (Pascal
>    `DSSClassDefs.pas` `CreateDSSClasses` vs the Rust `exec/construct.rs`
>    registry) shows five upstream-registered classes with no Rust port:
>    **Isource**, **AutoTrans**, **GICLine**, **GICTransformer**,
>    **GICsource**. Isource/AutoTrans sit in the PORTING_PLAN module sketch
>    (`pc/isource.rs`, `pd/autotrans.rs`) but belong to no phase; the GIC trio
>    was parked at Phase 9. (Verified non-gaps: `ControlledTransformer.pas`
>    and the user-model DLL classes are **not registered** upstream — nothing
>    to port; `Feeder` is dead upstream, proven by probe.)
> 3. **One pulled-forward Phase-9 subsystem** (added 2026-07-06): the CIM XML
>    export (`ExportCIMXML.pas`, 4.8k lines) — **WPG.18**. Not test-blocked,
>    but *pure output over solved state*, so it gates exactly like the Phase-8
>    report work (byte-exact goldens against the pinned oracle), not like the
>    live-solve WPs. All design decisions are pre-made in the WP; the work is
>    staged transcription.
>
> Every §1/§1b item ships a **validated synthesized deck** in the
> `tests/corpus/gaps/` **staging family** (lifecycle in §3.1: a deck lives
> there only while its feature is unported, then graduates to a permanent
> family), and the porting work is packaged as independently-gated WPs (§4).
>
> **When to execute.** After (or interleaved with) Phase 8 — every WP here is
> independent of the Phase-8 report work except where a dependency is called
> out (WPG.12 GFM needs WP8.6 `BatchEdit` for its corpus decks). The WPs are
> independently gated and can run in any order; §4 orders them risk-ascending.
>
> **Prerequisites already in place** (do not rebuild): the daily/yearly/duty
> solve loops + `FinishTimeStep`/`EndOfTimeStepCleanup` hooks (Phase 5/6), the
> control sweep (WP5.7), EnergyMeter registers + DI files (WP6.5 + WP8.3),
> Monitor `SaveAll` (WP8.3), Fault + FaultStudy (WP7.2/7.9), Storage/PVSystem/
> InvControl/StorageController (WP7.3–7.5), harmonics frequency sweep (WP7.6),
> dynamics loop (WP7.7), the `Set AddType/GenkW/GenPF/Capkvar/AutoBusList=`
> option parsing + `circuit/auto_add.rs` skeleton (WP6.8).

## 1. Inventory — every deferral that was blocked on a missing test

Legend: **site** = where the deferral lives in the Rust tree today; **deck** =
the synthesized deck in `tests/corpus/gaps/` (all oracle-validated, §3).

| # | Feature | Deferred at | Site today | Deck | WP |
|---|---|---|---|---|---|
| 1 | `Set mode=Time` (`SolveGeneralTime`) | PHASE5 §4 → WP7.9 | `solution/solution/dispatch.rs` "Unknown solution mode" | `generaltime.dss` | WPG.2 |
| 2 | `Set mode=LD1/LD2` + `Set LDCurve=` | PHASE5 §4 → WP7.9 | same + `Set LDCurve` unparsed (`exec/report.rs` writes empty `ldcurve`) | `ld1.dss`, `ld2.dss` | WPG.3 |
| 3 | `Set mode=M1/M2/M3` (Monte Carlo) + `Load.Randomize` | PHASE5 §4 → WP7.9 | dispatch error; `Set random=` parses, is never consumed | `monte1/2/3.dss` | WPG.4 |
| 4 | `Set mode=MF` (`SolveMonteFault`, `PickAFault`, `Fault.Randomize`) | WP7.9 | dispatch error | `montefault.dss` | WPG.4 |
| 5 | AutoAdd solve mode (capacity search, `UseAuxCurrents`, winner instantiation, `AutoAddLog`) | PHASE6 §2.6 → WP7.9 | options parse; `circuit/auto_add.rs` skeleton `NOT_PORTED` | `autoadd.dss` | WPG.5 |
| 6 | Newton algorithm (`Set algorithm=Newton`, `DoNewtonSolution`) | Phase 3 stub, never picked up | `power_flow.rs:230` "Newton solution not ported" | `newton.dss` | WPG.6 |
| 7 | CapControl `type=Follow` + `ControlSignal=` (LoadShape ref) | PHASE5 §4 (WP5.6) | `ControlSignal` `NOT_PORTED`; `Sample` aborts on FOLLOWCONTROL | `capcontrol_follow.dss` | WPG.7 |
| 8 | LoadShape `SngFile`/`DblFile`/`PQCSVFile` (+ TShape/PriceShape `SngFile`/`DblFile`, GrowthShape `CSVFile`/`SngFile`/`DblFile`) | PHASE4 §5 / PHASE5 §4 | `NOT_PORTED` prop flags on all three shape classes | `shape_binfiles.dss` | WPG.1 |
| 9 | Reactor `RCurve`/`LCurve` (frequency-dependent R/L in the harmonic Y) | PHASE4 §5 → carried | `reactor/mod.rs:100` `NOT_PORTED` | `reactor_rlcurve.dss` | WPG.8 |
| 10 | InvControl `ControlModel=1` (Exponential — the `TPICtrl` PI controller) | WP7.5 | `inv_control/compute.rs:2005` `NOT_PORTED` | `invcontrol_expmodel.dss` | WPG.9 |
| 11 | InvControl `mode=voltwatt` and `combimode=VV_VW` over **Storage** | WP7.5 | explicit `NOT_PORTED` errors (`inv_control/tests.rs:625/653` pin them) | `invcontrol_storage_vw.dss`, `invcontrol_storage_vv_vw.dss` | WPG.10 |
| 12 | StorageController seasonal targets (`Get_DynamicTarget`) + `Set SeasonRating/SeasonSignal=` | WP7.4 | `storage_controller/compute.rs:482/720` non-seasonal fallback; the two `Set` options unparsed; `report/export/capacity.rs` seasonal branch `NOT_PORTED` | `storagecontroller_seasonal.dss` | WPG.11 |
| 13 | Relay `Type=Generic` / `Type=TD21` `Sample` logic | WP7.7 tracked-open ("Plot-blocked") | `relay/mod.rs:601` `record_not_ported_once` | corpus decks exist (see WPG.12) | WPG.12 |
| 14 | GFM grid-forming mode (InvControl `mode=GFM`/combi; Storage/PVSystem/Generator GFM voltage source path) | WP7.7 tracked-open ("Plot-blocked") | `NOT_PORTED` across `storage/dynamics.rs`, `inv_control/*`, `solution/dispatch.rs:70` GFM abort | corpus decks exist (see WPG.13) | WPG.13 |

### 1b. Inventory — the unported element classes

Upstream registration order (`DSSClassDefs.pas` `CreateDSSClasses`):
`Isource` (early, with the sources) … `IndMach012` (l.264) → **GICsource**
(l.267) → **AutoTrans** (l.270) → `InvControl` (l.273) → `ExpControl` (l.276)
→ **GICLine** (l.279) → **GICTransformer** (l.282) → `VSConverter` (l.285).
Registration order does not affect node ordering (creation order does) — the
existing `construct.rs` comments already document that convention.

Each element WP carries the full **deck matrix** {static snapshot ("sync") ×
time-series ("async") × combined snapshot→time-series ("both", the cross-mode
state-leak class)} at **both scales** — micro (hand-written) and midi (the
shared IEEE123-class scaffold, generated by `tools/decks/gen_midi_decks.py`) —
wherever the mode exists upstream. The GIC classes have **no async/both
variants**: none has a shape reference or any time-varying drive (their
corpus usage is a 0.1 Hz quasi-DC snapshot).

| Class | Pascal unit (lines) | Corpus decks blocked on it | Decks (`tests/corpus/gaps/`) | WP |
|---|---|---|---|---|
| Isource | `PCElements/Isource.pas` (541) | `Examples/Microgrid/ISource/Master.DSS`, `Examples/Matlab/pst.dss`, `Examples/FreqScan/Run_Scan.dss` | micro: `isource_snap`, `isource_daily`, `isource_both`, `isource_harm`; midi: `midi_isource_asym`, `midi_isource`, `midi_isource_both` | WPG.14 |
| AutoTrans | `PDElements/AutoTrans.pas` (2065) | `Test/AutoTrans/{Auto1bus,Auto3bus,AutoAuto}.dss` + the byte-identical `Version8/Distrib/Examples/AutoTrans/*` copies (AutoAuto needs the class; Auto1bus/3bus carry stale `fault` tags) | micro: `autotrans_snap`, `autotrans_reg`, `autotrans_both`, `autotrans_gic`; midi: `midi_autotrans_asym`, `midi_autotrans`, `midi_autotrans_both` | WPG.15 |
| GICTransformer | `PDElements/GICTransformer.pas` (595) | `Examples/GICExample/GIC_Example.dss` | `gictransformer_gic` + the combined `gic_midi` | WPG.16 |
| GICLine | `PCElements/GICLine.pas` (679) | same deck | `gicline_gic` + `gic_midi` | WPG.16 |
| GICsource | `PCElements/GICsource.pas` (478) | **zero corpus decks** (the classic gaps case) | `gicsource_gic` + `gic_midi` | WPG.16 |

**Explicitly NOT in this plan** (deferred elsewhere, with a real owner):
- `Feeder` objects — **proven dead upstream by an oracle probe** (Phase 8
  record); not test-blocked.
- `MakePosSequence` — Phase 8's on-demand stance stands (ported when a
  consuming path reaches it).
- Binary shape **outputs** (`Action=SngSave/DblSave`) — already Phase 8 scope
  (PHASE8_PLAN §"On demand", synthesized fixture there).
- LoadShape `MemoryMapping=yes` — an I/O strategy, not observable numerics;
  stays `NOT_PORTED` with a loud error until something observable hinges on it
  (this is a *behavior*-based deferral, not a test-based one; revisit if a
  corpus deck ever sets it — none does today).
- Actors/`SolveAll`, `Pstcalc` flicker — Phase 9. (CIM export sat on this
  line until 2026-07-06 — pulled into this plan as **WPG.18**.)
- All user-model DLL hooks + `ControlledTransformer.pas` — never (safe Rust /
  not registered upstream).

## 2. Phase-wide design decisions

### 2.1 Randomness: what can and cannot be oracle-pinned (probe-proven)

FPC seeds its RTL RNG **per process from the clock**: the vendored
`Shared/mathutil.pas` has `initialization Randomize;`. Therefore any value that
reaches the output through `Random`/`Gauss`/`QuasiLognormal` is
**nondeterministic across oracle processes** — it cannot be golden-pinned, and
no dss-python API exposes `RandSeed` to synchronize the two engines. The gates
are therefore designed around the RNG:

- **`Set random=none`** (`RandomType=0`) makes every Monte path deterministic:
  `Load.Randomize(0)` sets `RandomMult := 1.0` (Monte1); Monte2/Monte3's
  `case Randomtype of` has no `none` branch, so `LoadMultiplier` is simply left
  unchanged; `Fault.Randomize`'s `else` branch sets `RandomMult := 1.0`.
- **MonteFault** additionally consumes a draw in `PickAFault` — but with
  exactly **one** Fault element `Whichone = Trunc(Random*1)+1 = 1` always, so a
  single-fault deck is deterministic even though the RNG state advances.
  (`montefault.dss` is built exactly this way; proven bit-identical across two
  oracle processes.)
- The **RNG-consuming branches are still ported 1:1** (never skipped): port
  FPC's generator (the RTL Mersenne Twister over `RandSeed`, plus
  `mathutil.pas` `Gauss` = mean + stddev·(Σ of 8 `Random` − 4)·... — port the
  exact formula from the source, and `QuasiLognormal`) into
  `support/mathutil/` as pure safe Rust. Their gates are **Rust-only
  fixed-seed unit tests** transcribed from an FPC probe (the ppcrossx64
  probe pattern from the line-impedance investigation): fixed `RandSeed` →
  assert the exact draw sequence and the exact `Gauss`/`QuasiLognormal`
  values. The Rust engine's `Randomize` equivalent (time seed) is the same
  documented-nondeterministic behavior as upstream.
- **Never** loosen a gate to "statistical" comparison — if a scenario cannot
  be made deterministic, it is not a gateable scenario (CLAUDE.md no-fudging).

### 2.2 Oracle fragility around AutoAdd (probe-proven)

dss-python 0.15.7 **segfaults at process exit** after an AutoAdd solve (the
solve itself completes; `Text.Result` returns the winner, the auto-added
generator is present, and the full fingerprint is bit-identical across runs —
the crash is in teardown). The live gate's oracle-side driver must therefore
capture everything for the `autoadd` case **before interpreter teardown**
(finish that oracle process with `os._exit(0)`, or run the case in a
subprocess whose nonzero exit after complete output is tolerated — check how
the existing `corpus_live.rs` oracle driver scopes processes at WP open). Do
not "fix" this by weakening the capture. Also probe-proven: `Text.Result`
after the AutoAdd solve is `"b3, 0.0180069930672805"` for the committed deck —
the gate pins the winner bus, the improvement figure, the appended
`Generator.gadd1` (a manifest probe), and the `AutoAddLog` file contents
(via the WP8.1 `compare_export` harness).

### 2.3 Gate machinery (reuse, don't invent)

- **The decks are a live-gate family**, not goldens: `tests/corpus/gaps/` +
  `manifest.json`, exactly like the sibling `asymmetric/` and `controls/`
  families. The gate is a new **`gaps_cases_match_oracle`** section in
  `crates/dss-core/tests/corpus_live.rs` (clone the `controls_cases_match_oracle`
  machinery): full-model live compare per step plus the per-case opt-ins the
  manifest already declares — `probes` (property values), `compare_eventlog`,
  `compare_ctrlqueue`, `check_meters_monitors`. Tolerance classes per
  `tests/harness` `tol_for` / `tests/TOLERANCE_NOTES.md`; discrete state,
  event logs, iteration counts exact; **no new tolerance classes without
  empirical proof**.
- **`pending` discipline (no silent skips):** every manifest case starts
  `"pending": true`. The gaps test must treat a pending case as "the Rust
  engine errors **loudly** on the unported feature" (assert the specific
  `NOT_PORTED`/unknown-mode error — never a silent skip, never a silent
  fallback), and a non-pending case as a full live compare. The WP that ports
  a feature flips its cases to `pending: false` in the same commit, then
  **graduates** the decks out of the staging family (§3.1). WPG.17
  asserts no `pending: true` — and no deck — remains.
- **Per-deck sensitivity is already proven** (§3): removing the feature under
  test changes the oracle output, so a port that silently skips the feature
  cannot pass its live compare.
- Where **vendored** corpus decks exist (WPG.12/WPG.13), the standard live
  corpus gate is the primary gate: refresh stale tags via
  `DSS_LIVE_CLASSIFY=1` + `tools/corpus/apply_classify.py`, migrate to
  `solvable_now`, update `tests/corpus/COVERAGE.md`.
- Output hygiene: the live harness already redirects report/DI outputs away
  from the corpus tree (the WP8.2 `CorpusGuard` mandate) — `tests/corpus/gaps/`
  must stay pristine under `AutoAddLog`/DI-writing cases the same way.

### 2.4 Set-option surface this plan adds

`Set LDCurve=` (+ `Get`), `Set SeasonRating=` / `Set SeasonSignal=` (+ `Get`) —
port the exact ExecOptions spellings (`SeasonRating` is the option name; the
global it sets is `SeasonalRating` — probe-proven: `Set SeasonalRating` is
error #130). `Set random=` already parses; wire `RandomType` into the solve
paths. `Set autobuslist=` — verify the existing parse reaches
`Circuit.AutoAddBusList` semantics (`MakeBusList` falls back to *all* load
buses when the list is empty — port that branch too).

## 3. The synthesized decks (committed, oracle-validated)

Sixteen feature decks (§1) + eighteen element decks (§1b) at
`tests/corpus/gaps/` with their `manifest.json` (+ 6 committed binary/CSV
input fixtures, regenerate via `tools/corpus/gen_gaps_binshapes.py`; the
`midi_*` element decks regenerate via `tools/decks/gen_midi_decks.py`).
Validation protocol, run 2026-07-05 against the pinned oracle (dss-python
0.15.7):

1. compiles + solves + converges;
2. **bit-identical fingerprint across two separate oracle processes**
   (converged/iterations, YNodeVarray at full precision, per-monitor channels,
   meter registers, event log, `Text.Result`) — the determinism proof §2.1
   requires;
3. **feature-sensitive** where the feature could silently no-op: proven for
   `storagecontroller_seasonal.dss` (event log diverges vs `SeasonRating=no`
   from hour 12 — the 500 kW season target engages) and
   `reactor_rlcurve.dss` (harmonic monitor channels diverge vs a
   curve-less reactor); the solve-mode decks are structurally sensitive
   (sample counts pin the loop shape: 8 time steps / 16 = 4×4 LD1 points /
   5 LD2 points / 5 M1 cases / 24 M2 steps / 4 M3 cases / 3 MF picks).

Deck-level findings recorded for the port (each is a probe-proven upstream
fact, cite in the code):

- `InvControl.ControlModel` **rejects the enum name** — only the ordinal
  parses (`ControlModel=1`); the deck header documents it.
- The seasonal option spelling is **`Set SeasonRating`**, not
  `SeasonalRating`.
- `Set time=` must come **after** `Set mode=` (Set_Mode resets time) — LD2/M3
  decks encode the order.
- AutoAdd teardown segfault (§2.2).

Element-deck findings (probe-proven on the pinned oracle; all eighteen decks
compile + solve + converge, bit-identical across two oracle processes):

- The GICTransformer per-unit resistance spelling is **`%R1`/`%R2`** (the
  `pctR1` enum name does not parse — error #110).
- The AutoTrans+RegControl decks are control-active: `autotrans_reg` logs
  **10 tap-change events** over 8 hours (final `tapnum=5`), `midi_autotrans`
  **10 events** at midi scale (final `tapnum=11`) — the manifest probes +
  `compare_eventlog` pin them. RegControl reaches AutoTrans via the Pascal
  `Transf_Or_AutoTrans` proxy; the **Series-winding control path raises
  upstream** (`RegControl.pas:1009`) — the decks regulate the common (wye)
  winding 2.
- `gicsource_gic.dss` / `gic_midi.dss`: the oracle **splices** each GICsource
  into its same-named Line at edit time — the bus list gains a `gic_<name>`
  bus and the Line's `bus2` is rewritten to it (manifest `bus2` probes pin
  the splice and the resulting bus order).
- AutoTrans `? wdgcurrents` returns a live recompute (e.g. `66.95905,
  (-28.006), …` on `autotrans_reg`) — the same `READS_VTERMINAL` family as
  Transformer (see the WP8.5 follow-up in STATUS.md); the Rust prop def must
  carry that flag from day one.
- `isource_harm.dss`: the harmonics sweep collects **7 frequencies** (the
  `scan5` 1/5/7/11 merged with the load's `defaultload` 1/3/5/7/9/11/13
  spectrum) — 7 monitor samples on the oracle.

### 3.1 Deck lifecycle — `gaps/` is a STAGING family, not a destination

"Gaps" describes a deck's *state*, not its nature: once the feature is
ported there is no gap left, so the deck must not stay. The rule:

- A deck lives in `tests/corpus/gaps/` **only while** its feature is
  unported (`pending: true` — the loud-error gate).
- The WP that ports the feature, in the same change: flips `pending: false`,
  proves the live compare green, then **moves the deck + its manifest entry
  to the permanent family** it belongs to (re-cutting the deck to that
  family's drive convention where needed — gaps decks are self-driving,
  the permanent families let the harness drive):
  - static/snapshot element coverage → `tests/corpus/asymmetric/`;
  - control/time-series/both coverage → `tests/corpus/controls/`;
  - solve-mode / algorithm / input-format decks (Monte, LD, Time, AutoAdd,
    Newton, shape files, harmonic curves) → `tests/corpus/modes/` (created
    at the first such graduation, same manifest shape as the siblings).
  `midi_*` element decks also move their generator entry in
  `tools/decks/gen_midi_decks.py` to the matching target directory.
- WPG.17 (the exit sweep) asserts the staging family is **empty** and
  deletes the directory.

## 4. Work packages (independently gated; risk-ascending)

Effort ≈ share of this plan. Pascal line refs confirmed at WP open (the
PHASE7 convention).

---

### WPG.1 — Shape file inputs [7%]

**Pascal:** `General/LoadShape.pas` `DoSngFile`/`DoDblFile`/`DoCSVFile`(PQ
variant `Do2ColCSVFile`?—confirm the PQCSVFile reader name at open),
`General/TempShape.pas` + `PriceShape.pas` sng/dbl readers,
`General/GrowthShape.pas` `DoCSVFile`/sng/dbl.

Un-`NOT_PORTED` the file props; reuse the WP5.2b deferred-`FileLoad` path
(LoadShape `CSVFile` is the template — TShape/PriceShape `CSVFile` already
ported the same way). Binary readers: little-endian f32/f64 streams; the
`Interval=0` variant reads (hour, value) pairs — port both branches even
though the deck uses `interval=1` (add an `interval=0` scenario to
the family from the same fixture data). Gate: the `shape_binfiles.dss` live
case goes `pending: false` (monitor trajectories prove the shapes actually
drive the loads: P follows `mult`, Q follows the PQCSV `qmult` — first/last-
step values are in the probe record); props round-trip dumps for all touched
classes.

---

### WPG.2 — `SolveGeneralTime` (mode=Time) [5%]

**Pascal:** `Common/SolutionAlgs.pas` `SolveGeneralTime` (l.298 — ~25 lines:
per-step `DefaultHourMult` → `SolveSnap` → `FinishTimeStep`).

Wire `SolveMode::GeneralTime` in `dispatch.rs`; the loop body is all
existing machinery. Note the loop does **not** `IncrementTime` first —
`FinishTimeStep` increments at the end (time starts at the `Set_Mode`-reset
zero; hour sequence in the deck's monitors pins it). Gate: the `generaltime.dss`
live case goes `pending: false` (8 steps, Storage %stored trajectory + meter
registers + monitor hours). No corpus migration (the only `mode=time` entry points, EPRI ckt5
`Run_Master_ckt5*.dss`, are oracle-broken/`SolveAll`-blocked — recheck the
tags, don't migrate).

---

### WPG.3 — Load-duration modes LD1/LD2 + `Set LDCurve=` [8%]

**Pascal:** `SolutionAlgs.pas` `SolveLD1` (l.558)/`SolveLD2` (l.645);
`ExecOptions` `LDCurve`; `Circuit.LoadDurCurveObj`.

1. `Set LDCurve=` (+ `Get`) resolving a LoadShape by name into
   `ckt.load_dur_curve_obj` (snapshot-clone ObjectRef pattern); error #470/
   #471 when unset (port the exact `_(...)` message).
2. `solve_ld1`/`solve_ld2` verbatim: LD1 = daily outer loop
   (`NDaily = Round(24·3600/h)`) × curve-points inner loop; LD2 = fixed time,
   curve sweep. `LoadMultiplier := curve.Mult(N)`, `IntervalHrs :=
   PresentInterval`, PriceShape signal if set. The Load-side mode dispatch
   (MONTECARLO2..LOADDURATION2 → `CalcDailyMult` + `LoadMultiplier`) already
   exists — verify against `Load.pas` l.1082, don't re-derive.
3. `exec/report.rs`'s empty-`ldcurve` placeholder now reads the real name
   (the DI/`Show Meters` header column).
4. Gate: the `ld1.dss` (16 samples = 4×4) + `ld2.dss` (5 samples, fixed
   time=(14,0)) live cases go `pending: false`; meter register integration
   over `PresentInterval`.

---

### WPG.4 — Monte Carlo modes M1/M2/M3/MF + the FPC RNG port [14%]

**Pascal:** `SolutionAlgs.pas` `SolveMonte1/2/3` (l.367/418/491),
`SolveMonteFault` (l.727) + `PickAFault` (l.703); `PCElements/Load.pas`
`Randomize` (l.899) + the MONTECARLO1 arm of `SetNominalLoad` (l.1074);
`PDElements/Fault.pas` `Randomize` (l.395); `Shared/mathutil.pas` `Gauss`/
`QuasiLognormal`; the FPC RTL generator (`RandSeed`/`Random`).

1. **RNG support module** (`support/mathutil/` or a sibling): FPC-exact
   `Random` (the RTL Mersenne Twister over `RandSeed` — transcribe from the
   FPC 3.2.2 RTL source, same version as the vendored engine build), `Gauss`,
   `QuasiLognormal` — Rust-only fixed-seed unit tests transcribed from an FPC
   probe (§2.1; ppcrossx64 pattern).
2. `solve_monte1/2/3` verbatim (Monte1: hour=case counter, per-load
   `Randomize(RandomType)`; Monte2: per-day multiplier draw + daily inner
   loop + `EndOfTimeStepCleanup`; Monte3: fixed time, multiplier draw per
   case). Wire `RandomType` from the already-parsed `Set random=`.
3. `Load::randomize` + the MONTECARLO1 factor (`RandomMult · GrowthFactor ·
   LoadMultiplier`), `Fault::randomize`, `solve_monte_fault` (ADMITTANCE
   direct solves; `PickAFault` with the Pascal draw-and-clamp).
4. Gate: the four decks under `random=none` (values deterministic, loop
   shapes pinned by sample counts / case-hours); the RNG itself by step-1's
   fixed-seed tests. Never a statistical gate (§2.1).

---

### WPG.5 — AutoAdd solve mode [12%]

**Pascal:** `Common/AutoAdd.pas` (567 — `MakeBusList`, `Solve` capacity
search, `ComputekWLosses_EEN`, `AppendToFile`/log, the winner `New
Generator.gadd...`/`New Capacitor.cadd...` command emission);
`Common/Solution.pas` `UseAuxCurrents`/`AddInAuxCurrents` (the aux-injection
hook `power_flow.rs:82` already stubs).

1. Replace the `circuit/auto_add.rs` skeleton: `MakeBusList` (AutoBusList →
   else every load bus; port the hash-list dedup), the per-bus test-gen
   loop (`SolveSnap` per candidate, weighted `puLossImprovement`/
   `puEENImprovement` scoring with `LossWeight`/`UEWeight`), capacitor
   branch, and the final permanent add via the executive (`New Generator...`
   through the normal command path, exactly like Pascal).
2. `UseAuxCurrents` wiring in the injection loop (AutoAdd is its only
   consumer — flip the Phase-3 stub).
3. `AutoAddLog` file via the WP8.1 output-path machinery.
4. Gate: the `autoadd.dss` live case goes `pending: false` — winner bus +
   improvement figure in `Text.Result` (probe-pinned
   `"b3, 0.0180069930672805"`), the appended `Generator.gadd1` (manifest
   probe), final voltages, log file via `compare_export`. Mind the oracle
   teardown segfault (§2.2) in the oracle-side driver.

---

### WPG.6 — Newton algorithm [5%]

**Pascal:** `Common/Solution.pas` `DoNewtonSolution` (+ `SetSize` of the
Newton work arrays in `SolveSnap`'s init — find the exact allocation site at
WP open).

`Set algorithm=` already parses; replace the `power_flow.rs:230` error with
the ported loop (max 2× iterations budget, the |dV| convergence test — port
verbatim). Gate: the `newton.dss` live case goes
`pending: false` — converged flag, **Newton iteration count exact**, voltages
1e-6; plus one existing feeder re-solved under `algorithm=newton` as a second
manifest case (author + validate per §3; probe first that the oracle
converges on it).

---

### WPG.7 — CapControl `type=Follow` + `ControlSignal` [5%]

**Pascal:** `Controls/CapControl.pas` FOLLOWCONTROL arm of `Sample` (l.598
guard + the mode logic), `ControlSignal` DSSObjectReference (l.288).

Un-`NOT_PORTED` `ControlSignal` (LoadShape ref, snapshot-clone); port the
Follow sample logic (shape value at `dblHour` ⇒ open/close with
`Delay`/`DelayOff`). Gate: the `capcontrol_follow.dss` live case goes
`pending: false` — event log equal (probe shows OPENED@h1, CLOSED@h8), final
step states exact.

---

### WPG.8 — Reactor `RCurve`/`LCurve` [5%]

**Pascal:** `PDElements/Reactor.pas` `RCurve`/`LCurve` fetch + their use in
`CalcYPrim`'s frequency branch.

Un-`NOT_PORTED` the two XYcurve refs; apply `GetYValue(FreqMultiplier·60)`
(read the exact argument — Hz vs multiplier — at WP open) in the harmonic
rebuild. Gate: the `reactor_rlcurve.dss` live case goes `pending: false` —
per-harmonic monitor channels (probe-proven sensitive: ch values shift ~0.6%
with the curves on).

---

### WPG.9 — InvControl `ControlModel=Exponential` (TPICtrl) [6%]

**Pascal:** `Controls/InvControl.pas` `TPICtrl` (the PI controller record +
`SolvePI`/`PIController` methods — locate at WP open) and every
`CtrlModel = Exponential` branch in the volt-var/volt-watt/AVR paths.

Port the PI controller + the branch selection; keep the existing Linear path
byte-identical. Gate: the `invcontrol_expmodel.dss` live case goes
`pending: false` (8 daily steps, event-log + vars trajectories); extend
`inv_control/tests.rs:503` from "errors loudly" to the real behavior.

---

### WPG.10 — InvControl VOLTWATT / VV_VW over Storage [8%]

**Pascal:** `InvControl.pas` — the Storage-typed branches of the voltwatt
active-power limit (charging vs discharging limit selection,
`VoltwattYAxis` interplay) and the VV_VW combi dispatch for Storage.

Replace the two `NOT_PORTED` errors (`tests.rs:625/653` pin them today) with
the ported logic on the existing pair/triple-borrow fleet sweep. Gate: the
two decks' live cases go `pending: false` — per-step Storage P/Q + `%stored`
trajectory exact (probe shows the VW limit biting: −116.7 kW/phase → ~0
across the run), event logs equal.

---

### WPG.11 — StorageController seasonal targets [5%]

**Pascal:** `Controls/StorageController.pas` `Get_DynamicTarget` (l.1020,
0-based `SeasonTargets[trunc(SeasonSignal.GetYValue(intHour))]`), call sites
l.1100 (`THigh=1`)/l.1412 (`THigh=0`); `ExecOptions` 114/115.

1. `Set SeasonRating=` / `Set SeasonSignal=` (+ `Get`) → engine globals.
2. `Get_DynamicTarget` + both call-site guards (`if DSS.SeasonalRating`).
3. Fold in the `report/export/capacity.rs` seasonal-rating `NOT_PORTED`
   branch (same globals).
4. Gate: the `storagecontroller_seasonal.dss` live case goes
   `pending: false` — event log equal (probe-proven divergent from the
   non-seasonal run at hour 12: the 500 kW target engages), `%stored`
   trajectory exact.

---

### WPG.12 — Relay `Generic` + `TD21` Sample [12%]

**Pascal:** `Controls/Relay.pas` `GenericLogic` + `TD21Logic` (+ the
`LookupVariable`/`MonitorVarIndex` PC-state resolution and the `RelayTarget`
strings).

The WP7.7 "Plot-blocked, zero payoff" assessment is **stale**: `Plot` is a
headless no-op since WP8.1, and the corpus has real entry points —
`Test/TD21RelayTest.DSS`, `Test/ReverseTD21RelayTest.DSS`,
`Version8/Distrib/Examples/DistanceRelays/{TD21,ReverseTD21}RelayTest.DSS`
(currently mis-tagged `unsupported_class=relay` with pre-WP7.2 notes), and
the Generic-relay users in the `DG_Protection`/VCCS families. Steps:
1. Port `TD21Logic` (the differential time-distance buffer machinery) and
   `GenericLogic` (monitored-PC state variable vs `OverTrip`/`UnderTrip`)
   into the existing relay type dispatch (`relay/mod.rs:601`).
2. Re-classify (`DSS_LIVE_CLASSIFY=1`) → migrate the four TD21/Distance
   decks + any Generic entry points to `solvable_now`; refresh the stale
   tags; COVERAGE.md burn-down.
3. Targeted gate: the TD21 deck family runs dynamics-mode
   (`stepsize=0.001 number=1200`) — event logs equal (trip/reclose
   sequence), final states exact; tolerance class 1e-5 (dynamics) only where
   TOLERANCE_NOTES already allows it.

---

### WPG.13 — GFM grid-forming mode [18%] — **after WP8.6 (BatchEdit)**

**Pascal:** `PCElements/Storage.pas`/`PVsystem.pas` GFM branches
(`CalcGFMVoltage`, `GFM_Mode` current/power dispatch, `CheckIfDelivering`),
`PCElements/generator.pas` GFM (`dynamics.rs:10` notes it),
`Controls/InvControl.pas` `mode=GFM` + combi arms, the `VDelta`/droop props
on `InvBasedPCE` (`inv_based_pce.rs:170`).

The other stale "Plot-blocked" item: the corpus has live entry points —
`Version8/Distrib/Examples/IBRDynamics_Cases/GFM_IEEE123/Run_IEEE123Bus_GFMDaily*.DSS`
(re-probed 2026-07-03: compile, blocked **only** by BatchEdit + the GFM
`NOT_PORTED` abort) and the `GFL_IEEE123` sibling; `GFM_IEEE8500` masters
are oracle-troubled (`Master-unbal` non-convergent, `Run_RecloserSiting`
#485) — check tags, don't force. Steps:
1. Port the GFM voltage-source model (Storage first — the abort site
   `dispatch.rs:70` — then PVSystem, then the Generator GFM branch), the
   InvControl GFM/combi arms, the droop props.
2. Synthesize a micro GFM deck (`tests/corpus/gaps/gfm_micro.dss` — Storage
   GFM island + daily; author + oracle-validate it at WP open following §3's
   protocol, add it to the manifest) as the targeted live case, since the
   vendored corpus decks are large.
3. After WP8.6 lands BatchEdit: migrate the GFM_IEEE123/GFL_IEEE123 family
   to `solvable_now` (live gate = the primary gate); COVERAGE.md.

---

### WPG.14 — Isource [5%]

**Pascal:** `PCElements/Isource.pas` (541 lines) — the ideal current source.
Small and self-contained; the Rust source-element template is
`elements/pc/vsource/` (module layout + registration + `ElemKind::Source`).

Steps (in order; each cites the Pascal site to port loop-for-loop):

1. **Module** `crates/dss-core/src/elements/pc/isource/` (`mod.rs` +
   `accessors.rs` + `solve.rs`, the vsource layout). Props table (`class_props`)
   — 11 props in the Pascal enum order: `Bus1`, `Amps`, `Angle`, `Frequency`,
   `Phases`, `ScanType` (mapped enum `ScanTypeEnum`), `Sequence` (mapped enum
   `SequenceEnum`), `Yearly`, `Daily`, `Duty` (LoadShape ObjectRefs,
   snapshot-clone like Load's), `Bus2`. Copy each prop's flags from
   `DefineProperties` (l.150): `Frequency` is `DynamicDefault` =
   BaseFrequency + NonNegative|NonZero; `Amps` NoDefault; etc.
2. **Struct fields + side effects** (`PropertySideEffects`, l.221): `Bus1` set
   auto-derives `Bus2` = bus1 stripped of nodes + `.0` per phase (grounded-Y
   return) *unless* `bus2_defined`; `Phases` sets `phase_shift` (1 → 0,
   2/3 → 120, else 360/N) and `nconds = nphases`, `nterms = 2`; `Yearly`
   unset defaults to `Daily` when Daily is set.
3. **Solve surface** (`solve.rs`):
   - `calc_yprim` (l.343): an all-zero YPrim (ideal source) — allocate,
     then the standard open-conductor fold; clear `yprim_invalid`.
   - `get_base_curr` (l.370): harmonic model → `spectrum.GetMult(f/SrcFreq) ·
     Amps` rotated per harmonic; else the Daily/Duty/Yearly mult path (same
     dispatch as Load) **plus the frequency gate**: if
     `|Solution.Frequency − SrcFrequency| ≥ EPSILON2` return 0 (the source
     only injects at its own frequency — this is what makes it a FreqScan/GIC
     citizen). Port `EPSILON2` exactly.
   - `get_inj_currents` (l.463): +BaseCurr on terminal-1 conductors,
     −BaseCurr on terminal-2, rotated by `phase_shift` per phase with the
     ScanType/SequenceType rules (harmonic: ScanType 1 → keep pos-seq
     rotation, 0 → none, else rotate by `Solution.Harmonic`; fundamental:
     SequenceType −1 → neg-seq, 0 → none, else pos-seq).
   - `get_currents` (l.445): `Curr[i] = −ComplexBuffer[i]` (negated
     injection; YPrim is zero so there is no Y·V term). `inj_currents`
     (l.436) adds into the system vector via `InjCtx`.
   - `source_frequency()` → `Some(src_frequency)` (the harmonics sweep's
     source pass — `CollectAllFrequencies` — needs it, exactly like VSource).
   - Spectrum default: **inherited `default`** (`DefaultGeneral`), not
     `defaultvsource` — set the resolver name accordingly.
4. **Register** in `exec/construct.rs` with `ElemKind::Source` — confirm the
   exact Pascal slot by grepping `ISourceClass :=` in `DSSClassDefs.pas` at WP
   open, and put the Rust registration in the same relative spot among ported
   classes (add the standard "registration order does not affect node
   ordering" comment).
5. **Gates:**
   - `props_roundtrip` picks the class up automatically (it iterates the
     registry) — run it, fix display-case/dump mismatches against the oracle.
   - Byte-exact `dump isource.x` golden vs the oracle (the WP8.5 dump harness).
   - Flip the seven Isource decks (`isource_snap`/`_daily`/`_both`/`_harm` +
     `midi_isource_asym`/`midi_isource`/`midi_isource_both`) to
     `pending: false` — the live compare covers snapshot injection, the
     shape-driven daily run, the snapshot→daily transition (cross-mode
     state-leak), the harmonic spectrum sweep (7 frequencies, §3 note), and
     all of it again at midi scale (94 nodes). Then graduate them per §3.1:
     `isource_snap`/`midi_isource_asym` → `asymmetric/`, the rest →
     `controls/` (`isource_harm` → `modes/` if the harmonics decks land
     there — decide at the first graduation and stay consistent).
   - Corpus: re-classify (`DSS_LIVE_CLASSIFY=1` + `tools/corpus/
     apply_classify.py`). Expected: `Microgrid/ISource/Master.DSS` advances
     but may stay blocked on `MakeBusList` (check whether it is ported by
     then); `Matlab/pst.dss` additionally needs WPG.1 (`LoadShape File=`) and
     has its only `solve` commented out — likely `not_an_entry_point`;
     `FreqScan/Run_Scan.dss` additionally needs Spectrum `csvfile=` (check
     ported state at WP open). Update tags honestly; migrate whatever solves.

---

### WPG.15 — AutoTrans [15%]

**Pascal:** `PDElements/AutoTrans.pas` (2065 lines). **Template: clone the
ported Transformer module** (`elements/pd/transformer/` — mod/accessors/edit/
windings/yterminal/dump…) and adapt; the class shares `TControlledTransformerObj`
with Transformer upstream, so most machinery is the same. Work in three
stages, each gated; commit per stage.

**Stage A — class skeleton, props, dump (no solve changes):**

1. Module `crates/dss-core/src/elements/pd/auto_trans/` cloned from
   transformer. Props: **42** in the Pascal enum order (`DefineProperties`
   l.364): same rows as Transformer **except** — reactances are
   **`XHX`/`XHT`/`XXT`** (trap_zero 7/35/30, scale 0.01; SpecSet1
   {XHX,XHT,XXT}, SpecSet2 {XSCArray}); a dedicated connection enum
   **`AutoTransConnectionEnum`** with `Wye=0, Delta=1, Series=2` (aliases
   `y`/`ln` → wye, `ll` → delta, `s` → series) — register it in
   `EnumRegistry`; **no `XfmrCode`**, **no `RNeut`/`XNeut`** (SaveWrite
   comment l.1054/1057 removes them — the Save writer must skip them too);
   `WdgCurrents` is the read-only result string and **must carry
   `PropFlags::READS_VTERMINAL`** (the WP8.5 follow-up flag — the `?`/`Dump`
   refresh then works for free).
2. Side effects (`PropertySideEffects` l.574): `conn`/`conns` **force
   winding 1 = Series and winding 2 = Wye** regardless of input (l.587-614);
   `TAutoWinding.Init` defaults (wdg1 Series/115 kV, others Wye/12.47).
   `pctR` ↔ `pctLoadLoss` coupling: pctLoadLoss = (W1.Rpu + W2.Rpu)·100;
   `Rdcpu` default = 0.85·Rpu. `VABase` = Winding[1].kVA·1000.
3. Register in `construct.rs` between IndMach012 and InvControl (Pascal
   l.270). ElemKind: mirror Transformer's kind/memberships — grep every
   `ElemKind::Transformer` use (pd lists, meter zones, `is_pd`, Save order)
   and give AutoTrans the same treatment (Pascal has **no** special
   EnergyMeter handling for it — it zones as a generic PDElement).
4. Gate A: `props_roundtrip` + byte-exact `dump autotrans.t1` golden vs the
   oracle (deck body from `autotrans_snap.dss`).

**Stage B — the auto electrical model (the real work):**

Port these AutoTrans-specific overrides loop-for-loop, citing lines:

1. `SetBus` override (l.721): winding-2 defaults all neutral-end conductors
   to one ground node (common-winding neutral tying); honors an explicit
   non-zero neutral.
2. `SetNodeRef` override (l.875, "Magic happens here"): for terminal 2 with
   winding 1 = Series, alias the series winding's second node to the common
   winding's first: `NodeRef[Nphases+i] := NodeRef[i+Nconds]`. Find where the
   Rust Transformer materialises `node_ref` (the circuit build path) and give
   AutoTrans the override there.
3. `SetTermRef` (l.1129): the Series arm of the conductor→winding map.
4. `CalcY_Terminal` (l.1856): ZB build as Transformer, plus the auto
   corrections — **`ZCorrected = ZBase·(1+Vcommon/Vseries)²`** applied to the
   series diagonal (Dommel 6.45); the 3-winding derived `puXst` (Dommel 6.50)
   from puXHX/puXHT/puXXT with Vs, Vc; `kVSeries = (kVLL − W2.kVLL)/√3`
   (`RecalcElementData` l.1000; guard kVSeries=0); the `ZeroTapFix` nested fn
   (l.1871).
5. `GICBuildYTerminal` (l.1823): when `Solution.Frequency < 0.51` build the
   resistance-only Y_Term from `1/RdcOhms` per winding, **no inter-winding
   coupling**, ppm anti-float added as a *conductance*. Selected inside
   CalcY_Terminal (l.1881). Port the 0.51 constant exactly.
6. `GetCurrents` override (l.1663): after the base computation, **fold the
   series current into the X terminal**: `Curr[i+Nconds] += Curr[i+Nphases]`.
7. `GetAllWindingCurrents` (l.1523) / `GetWindingVoltages` (l.1604): the
   Series-case index arms (`VTerm[i+1] := Vterminal[iphase+Nphases]`;
   `VBuffer[i] = Vterminal[i+k] − Vterminal[i+Fnconds]`). `GetLosses`
   override (l.1674) is the standard total − no-load split.
8. Gate B: flip `autotrans_snap.dss`, `autotrans_gic.dss` and
   `midi_autotrans_asym.dss` to `pending: false` — full live compare
   (YNodeV, element currents/powers, YPrim, 100 nodes at midi scale) + the
   `wdgcurrents` probes; the GIC deck pins step 5.

**Stage C — RegControl + corpus:**

1. RegControl's `transformer=` resolution accepts **both** classes (the
   Pascal `Transf_Or_AutoTrans_ProxyClass`, RegControl.pas l.264/308). The
   Rust RegControl drives taps through the Transformer accessor surface
   (`present_tap`/`set_present_tap`/`winding_tap_data`/`wdg_connection`/
   `base_voltage`, `transformer/windings.rs`) — expose the same surface from
   AutoTrans (shared trait or enum dispatch at the control-loop call sites)
   and port the **Series-winding guard error** (RegControl.pas l.1009:
   "Series connection … has not been implemented or tested!").
2. Gate C: flip `autotrans_reg.dss`, `autotrans_both.dss`,
   `midi_autotrans.dss` and `midi_autotrans_both.dss` to `pending: false` —
   event logs equal (10/12/10 tap events, §3), `tapnum`/`taps`/`wdgcurrents`
   probes exact, the `both` decks pinning the snapshot→daily transition.
   Then graduate all seven AutoTrans decks per §3.1 (`autotrans_snap`/
   `autotrans_gic`/`midi_autotrans_asym` → `asymmetric/`, the reg/both
   decks → `controls/`; move the generator entries).
3. Corpus: re-classify. `AutoAuto.dss` (both copies) needs the class +
   `BatchEdit autotrans..*` (WP8.6) + exports; `Auto1bus`/`Auto3bus` build
   the same unit from **regular transformers** — their `unsupported_class=
   fault` tags are stale (Fault ported at WP7.2), so they may migrate to
   `solvable_now` on re-classification alone. Watch the corpus quirk: check 4
   of the shared test script writes `enabled=yesa` (a typo upstream parses as
   a boolean **false** via FPC semantics — verify against the oracle during
   migration, never "fix" the deck). COVERAGE.md burn-down.

---

### WPG.16 — GIC family: GICTransformer, GICLine, GICsource [8%]

**Pascal:** `PDElements/GICTransformer.pas` (595), `PCElements/GICLine.pas`
(679), `PCElements/GICsource.pas` (478). One WP — the three are small,
interlocked (the decks chain them), and share the GIC frequency conventions.
Port in this order:

1. **Prerequisite check:** `Set frequency=0.1` must reach the snapshot solve
   (rebuild YPrims at the new frequency — the WP7.6 harmonics machinery
   already rebuilds per frequency; verify the plain `Set frequency` +
   `Solve` path drives it the same way, else wire it first).
2. **GICTransformer** (simplest; a shunt PDElement):
   - Props (15): `BusH BusNH BusX BusNX Phases Type R1 R2 KVLL1 KVLL2 MVA
     VarCurve %R1 %R2 K`. `R1`/`R2` are stored as conductances G1/G2 with
     the existing **`INVERSE_VALUE`** flag; SpecSet1 {R1,R2} vs SpecSet2
     {%R1,%R2} (the `%R1` spelling — §3 finding); `Type` enum GSU/Auto/YY;
     `VarCurve` an XYcurve ObjectRef; `K` default 2.2 with the
     `KSpecified`/`FpctRSpecified` toggles.
   - `SetBusX` write-function (l.150) promotes `Nterms` to 4 before setting
     bus 3; side effects: `BusH` → auto Bus2 = `busH.0.0.0` + `IsShunt=true`;
     `BusX` → auto Bus4; `Type=Auto` → Bus2 := Bus3 and Nterms 2→4.
   - `RecalcElementData` (l.433): Zbase = kV²/MVA; %R ↔ G conversion.
     `CalcYPrim` (l.483): pure conductance diagonals into **YPrim_Shunt**
     (GSU: one G1 block on terminals 1-2; Auto/YY: G1 block + G2 block on
     terminals 3-4); no frequency dependence; `is_shunt() = true`.
   - `WriteVarOutputRecord` (l.450) — the per-phase GIC → Mvar record: port
     it **only if** the consuming `Export` verb exists by then (check the
     export list at WP open); otherwise leave a `NOT_PORTED` note on the
     export, not on the element.
3. **GICLine** (a 2-terminal series-RZ(C) voltage source, template vsource):
   - Props (15): `Bus1 Bus2 Volts Angle Frequency(=0.1) Phases R X C EN EE
     Lat1 Lon1 Lat2 Lon2`; `VoltsSpecified` toggle between SpecSet1
     {Volts,Angle} and SpecSet2 (geodesy). Bus1 side effect defaults Bus2 to
     the same bus (node-stripped). ScanType/SequenceType internal defaults
     **0/0 (zero sequence)**; spectrum NIL.
   - `Compute_VLine` (l.349): port the geodesy constants **exactly**:
     `Phi=(Lat1+Lat2)/2` in radians; `VE=(111.133−0.56·cos(2φ))·ΔLat·EN`;
     `VN=(111.5065−0.1872·cos(2φ))·cosφ·ΔLon·EE`; `Volts=VN+VE`;
     ΔLat=Lat2−Lat1, ΔLon=Lon2−Lon1.
   - `CalcYPrim` (l.445): Z diagonal R+jX·FreqMultiplier, plus the series
     blocking capacitor when `C>0`: `Xc = −1/(2π·f·C·1e-6)` on the diagonal;
     invert (EPSILON fallback on singular); assemble the 2-terminal series
     YPrim.
   - `GetVterminalForSource` (l.524): the `|f − SrcFrequency| > EPSILON2 →
     Vmag=0` gate; zero-seq default (all phases the same phasor); harmonic +
     spectrum branch like VSource. `GetInjCurrents` = YPrim·[V;0] (l.611).
4. **GICsource** (the named-Line splicer; **the Line must already exist**):
   - Props (10): `Volts Angle Frequency(=0.1) Phases EN EE Lat1 Lon1 Lat2
     Lon2` — **no buses, no R/X/C, spectrum forbidden** (forced NIL).
   - `RecalcElementData` (l.326): find the **Line with the same name**; if
     its Bus2 doesn't already start with `GIC_`, insert bus `GIC_<name>`:
     SetBus(1)=GIC bus, SetBus(2)=Line's old Bus2, and rewrite the Line's
     Bus2 through the normal property path (Pascal `ParsePropertyValue` —
     in Rust drive the Line edit through the property engine so side effects
     fire). The `gicsource_gic.dss` `bus2` probes pin the splice + bus order.
   - `Compute_VLine` (l.313): same geodesy but **sign-flipped**
     (ΔLat=Lat1−Lat2, ΔLon=Lon1−Lon2 — the Pascal comment l.319).
   - `CalcYPrim` (l.361): fixed series G=10000 (0.1 mΩ). Zero-seq
     `GetVterminalForSource` (l.403) with the EPSILON2 frequency gate
     (`phase_shift` always 0).
5. Register all three in `construct.rs` at their Pascal slots (§1b order).
6. Gates: flip the four GIC decks (`gicline_gic`, `gictransformer_gic`,
   `gicsource_gic`, the combined 33-node `gic_midi`) to `pending: false`
   (full live compare — the nets are linear, snapshot at 0.1 Hz);
   `props_roundtrip` + dump goldens for the three classes; graduate the four
   decks per §3.1 (static snapshots → `asymmetric/`); corpus: re-classify
   `GICExample/GIC_Example.dss` (also uses `LatLongCoords` + `Show Current
   Elements` + `plot circuit` — plot is a headless no-op since WP8.1; check
   the other two, tag honestly, migrate if it solves).

---

### WPG.18 — CIM XML export (`Export CIM100`/`CIM100Fragments`) [20%] — after WP8.6 (UUID substrate) + WPG.15 (AutoTrans arm)

**Pascal:** `Common/ExportCIMXML.pas` (4790 lines — the whole unit), plus
plumbing already owned elsewhere: the `Export` dispatch/option parse
(`ExportOptions.pas:141-142/185-188/227-259/351-353/539-541`) and the WP8.6
UUID substrate (`DoUuidsCmd`, `Export Uuids`, lazy-v4 object UUIDs, the
hashed-key list, `DefaultCircuitUUIDs` — PHASE8_PLAN §WP8.6 step 6 owns all of
it, with the fixture `tools/golden/phase8_decks/uuids.dss` + `uuids_pre.csv`
already authored; **do not rebuild any of it here**).

Pulled forward from PORTING_PLAN Phase 9 on 2026-07-06 (the older WPs' effort
shares are left unrenormalized). 4.8k lines, but **mechanically simple**: pure
text output over already-solved state — no numerics beyond `%.8g` rendering,
no convergence, no iteration; per-class sweeps writing fixed XML templates.
The whole port is transcription + byte-diffing. Numbering note: WPG.17 (the
exit sweep) keeps its number and still runs **last**.

**Pre-made decisions (binding — do not re-derive):**

1. **Determinism.** Every UUID in the output is either a lazy
   `TNamedObject.UUID` or `GetHashedUuid` (l.952) — both bottom out in
   `CreateUUID4` = **random v4** (`NamedObject.pas:47-52`), which can never be
   oracle-pinned. Therefore **every gate preloads every UUID** via the WP8.6
   `uuids file=<fixture>` command before exporting, on both engines. A key
   missing from the fixture ⇒ a fresh random v4 ⇒ the byte-diff fails
   **loudly** ⇒ fix the fixture (regenerate; never weaken the comparison).
2. **Gate = byte-exact goldens** (not the §2.3 live machinery): the XML is
   newline-delimited text with a fixed header (`IEC61970CIMVersion.date` is
   the constant `2019-04-01` — no timestamps, no absolute paths), so with the
   fixture preloaded two oracle processes are bit-identical and the golden
   compare is **exact bytes** (CRLF-normalized), zero tolerance — the WP8.5
   dump-golden pattern. New generator `tools/golden/gen_cim.py` (clone the
   `gen_phase8.py` deck pattern: micro decks in `tools/golden/cim_decks/`,
   goldens in `tests/golden/cim/`; corpus-feeder cases reference the vendored
   corpus master like the existing feeder goldens). Per case the generator
   must: (a) process 1 — compile → solve → `export cim100` (output discarded)
   → `export uuids` → save the fixture CSV; (b) process 2 — compile → solve →
   `uuids file=<fixture>` → `export cim100` (+ `export cim100fragments` where
   the case says so) → save goldens; (c) process 3 — repeat (b), assert
   bit-identity with (b)'s output: the fixture-completeness proof. Commit
   fixture + goldens together; regenerate only manually with the PIN.
3. **Rust test driver** `crates/dss-core/tests/golden_cim.rs` (thin, on the
   existing harness): replay the deck → `uuids file=<fixture>` →
   `export cim100 fil=<tmp>` → byte-compare against the golden.
4. **Module:** `crates/dss-core/src/cim/` (the PORTING_PLAN sketch):
   `mod.rs` (exporter state: hashed-UUID list, bank/ECP/op-limit lists),
   `writer.rs` (profiles + node helpers), `catalog.rs` (wire/cable/linecode/
   spacing/xfmrcode info), `power_xfmr.rs`, `der.rs`, `ieee1547.rs`,
   `export.rs` (`ExportCDPSM`); split further per SPLITTING_RULES as files
   grow.
5. **Formatting:** doubles are `Format('%.8g')` — reuse the WP8.5-audited
   FPC-`%g` implementation (`report::format::g` at 8 significant digits;
   uppercase `E`, the `exp < -5` fixed-notation rule). Integers `%d`; booleans
   lowercase `true`/`false`. The `CIM_ID` string form is `UUIDToCIMString`
   (`NamedObject.pas:64`) — port exactly (leading `_`, braces stripped,
   lowercase — read the source, don't guess).
6. **Ordering:** byte-diffing makes element order load-bearing. Pascal sweeps
   class lists in creation order; the Rust registries already iterate the same
   way (the WP8.5 dump/save goldens rely on it) — sweep identically and byte
   equality follows.
7. **Staging discipline (no silent omissions):** Stage A ports the **entire
   `ExportCDPSM` control flow top-to-bottom** (l.3203-4707), with every
   not-yet-ported class arm replaced by a scoped `NOT_PORTED` error that fires
   **when the circuit contains instances of that class** (empty list ⇒ the arm
   is a no-op, matching Pascal). Stages B–F replace arms with real ports;
   stage F removes the last error — a partially-ported exporter can never
   silently drop a section.

**Stage A — writer core + skeleton + sources [4%]:**

1. Writer: `ProfileChoice` (Fun/Ep/Geo/Topo/Cat/Ssh/DynPrf), **combined mode
   only** (`Separate=false`; fragments = stage F): `WriteCimLn` (l.376),
   `StartInstance` (410), `StartFreeInstance` (422), `EndInstance` (432),
   `StartCIMFile` (3188; the `CIM_NS` const), `FD_Create`/`FD_Destroy`
   (4729/4752 — combined arm + the `</rdf:RDF>` closers).
2. All scalar/enum node helpers l.1340-1626 in one mechanical pass
   (`DoubleNode`…`StringNode`, the ~20 enum writers, the `PhaseNode` family)
   + `IsGroundBus` (2025).
3. UUID surface over the WP8.6 substrate: `GetHashedUuid`/`AddHashedUuid` +
   the `GetDevUuid` `UuidChoice` key table (952-1128), `GetTermUuid` (1299),
   `GetBaseVUuid`/`GetOpLimVUuid`/`GetOpLimIUuid` + their name builders
   (1309-1339).
4. `ExportCDPSM` skeleton in source order: Region/SubRegion/Substation/
   feeder-circuit instances + CRS + Location; base-voltage and
   op-limit-type objects (`<ckt>_NormAmpsType`…); buses →
   `ConnectivityNode`/`TopologicalNode`, the single `TopologicalIsland` +
   swing bus (= first enabled Vsource); `WritePositions` (2044),
   `WriteTerminals` (2117), `WriteReferenceTerminals` (2073), `VbaseNode`
   (2124); op-limit list plumbing (806-950) + the closing
   `OperationalLimitSet`/`CurrentLimit` sweep (~4658); the **EnergySource**
   sweep (Vsource, ~3680). Every other class arm = the scoped `NOT_PORTED`
   of decision 7.
5. Dispatch: `exec/report.rs` ptr 20/21 — the option loop
   `subs/subg/g/fil/fid/sid/sg/rg` (`ExportOptions.pas:227-259`;
   `AssignNewUUID` — locate its unit at WP open), the defaults l.185-188,
   default filenames `CIM100` / `CIM100x.xml` (351/353); ptr 21 = combined,
   ptr 20 = fragments (errors `NOT_PORTED` until stage F).
6. Gate A: deck `cim_src.dss` (Vsource + buscoords, nothing else) —
   full-file byte diff.

**Stage B — loads [2%]:** the EnergyConsumer sweep + `WriteLoadModel` (1998)
+ the `LoadResponseCharacteristic` constants (~4410),
`AttachLoadPhases`/`AttachSecondaryPhases` (1749/1736), ECP machinery
(`TECPObject` 340-365, list plumbing 837-930, `AddLoadECP` 1130, the closing
`EnergyConnectionProfile` sweep ~4620). Gate: `cim_load.dss` (wye + delta +
1-phase-secondary loads; a daily shape so an ECP row with `dssDaily` exists).

**Stage C — lines, switches, conductor catalog [4%]:** the
ACLineSegment-vs-LoadBreakSwitch sweep (~4290-4410; `ParseSwitchClass` 451
reads ratings off an attached Fuse/Recloser/Relay/SwtControl),
`AttachLinePhases`/`AttachSwitchPhases` (1627/1661), the phase-string family
(`PhaseString`/`PhaseOrderString`/`DeltaPhaseString`/`FirstPhaseString`
491-697), `LineCodeRefNode`/`LineSpacingRefNode`/`PhaseWireRefNode`
(1371-1385), and the catalog: PerLength(Phase|Sequence)Impedance from
LineCodes, `WriteWireData` (2277), `WriteCableData` (2232), `WriteTapeData`
(2250), `WriteConcData` (2262), spacings + `WirePositions`. Gate:
`cim_lines.dss` — coded 3-ph + 1-ph lines, a LineGeometry line
(wiredata+spacing), a CN and a TS cable line, a switch line with a Fuse.

**Stage D — caps + reactors [2%]:** shunt capacitors →
`LinearShuntCompensator` + `AttachCapPhases` (1703, the `sections` argument),
CapControl → `RegulatingControl` (`RegulatingControlEnum`,
`MonitoredPhaseNode`), series reactors → `SeriesCompensator` (~4274; note the
upstream 3-phase-only comment). Gate: `cim_shunt.dss` — wye/delta/1-ph caps,
a voltage- and a current-mode CapControl, a series reactor.

**Stage E — transformers + AutoTrans + regulators [4%]:** `TCIMBankObject`
(698-805; `BuildVectorGroup` 724, `AddTransformer` 759, `AddAutoTransformer`
786), the three transformer cases (comment l.3934: balanced-3ph-no-code →
`PowerTransformerEnd` + mesh/core; with XfmrCode → tanks + `TankInfo` refs;
else synthesize infos), `WriteXfmrCode` (2133), `XfmrTankPhasesAndGround`
(1531), the aux Wdg/Core/Mesh lists (~3783), the AutoTrans sweep (~3804 —
YNa/YNad1 vector groups; **needs WPG.15**), the banks write (~4157),
RegControl → `RatioTapChanger` + `TapChangerControl` (~4197;
`TransformerControlEnum`; the OpenDSS-only `maxLimitVoltage` note l.4237).
Gate: `cim_xfmr.dss` (2-w + 3-w + XfmrCode-tank transformers + a RegControl +
the `autotrans_snap.dss` body) — then the first two feeder goldens:
**IEEE13** and **IEEE123** full-file byte diffs.

**Stage F — DER + IEEE1547 + fragments mode [4%]:**

1. The DER sweeps: Generator → `SynchronousMachine` (~3514;
   `SynchMachTypeEnum`/`SynchMachModeEnum`/`GeneratorControlEnum`), PVSystem →
   `PowerElectronicsConnection` + `PhotovoltaicUnit` (~3562), Storage → +
   `BatteryUnit`/`BatteryStateEnum` (~3605), their `Attach*Phases`
   (1806-1997) and `Add*ECP` (1170-1275).
2. `TIEEE1547Controller` (2318-3187): `PullFromInvControl` (2493),
   `PullFromExpControl` (2846), `SetDefaults` CatA/CatB tables (2881),
   nameplates (2958-3023), `WriteCIM` (3024), `FindSignalTerminals` (2376) +
   `TRemoteSignalObject` (2318); `ConverterControlEnum` (4774).
3. Fragments mode (`Separate=true`): `WriteCimLn`'s auto-`StartFreeInstance`
   on first child write into another profile (the "stack overflow" comment
   l.412), `EndInstance` closing **every** open profile root, the seven
   `_FUN/_GEO/_TOPO/_SSH/_CAT/_EP/_DYN.xml` files (`FD_Create` 4729), ptr 20
   wired.
4. Gate: `cim_der.dss` (Generator + PVSystem + Storage + an InvControl
   volt-var + an ExpControl) — combined golden **and** the same deck's seven
   fragment files; the last `NOT_PORTED` arm removed;
   `rg "NOT_PORTED" crates/dss-core/src/cim/` returns nothing.

**Traps (source-proven; keep in view):**

- `GetDevUuid` keys are exact strings (`'Bank=' + name + '=' + seq`,
  l.1002-1128) and the fixture round-trips through them — a typo in a key =
  a random UUID = a loud diff (good: it cannot pass silently).
- Temporary (non-DSS-managed) objects get `=`-prefixed names (comment
  l.1001) — e.g. the `Station=Station=1` key in the WP8.6 fixture.
- Disabled elements are skipped ("disabled elements don't have terminal
  references", comment ~4313) — port every `Enabled` guard.
- `FD_Create` opens `F_DYN` passing `EpPrf` (l.4745) — an upstream oddity
  with no output effect (the header is profile-independent); do the same.
- `Export CIM100` requires a solved circuit (ptr 20/21 sit in the
  `1..24` guard range) — the existing `do_export_cmd` guard already covers
  it; don't add a second check.

**STATUS/corpus:** no `tests/corpus/gaps/` decks — this WP is golden-gated
(§2.3/§3.1 do not apply to it; WPG.17's staging-empty assert is unaffected).
At close, re-check corpus tags for anything blocked on `export cim` (none
known at authoring) and record the WP in STATUS §1 per the ritual.

---

### WPG.17 — Exit sweep [2%]

1. `rg "no corpus case"` / `rg "NOT_PORTED"` / the §1 + §1b tables — every
   row either ported+gated here or explicitly re-owned (§1b "NOT in this
   plan" list); no "test-absence" deferral survives anywhere in the tree,
   and the Rust registry diff vs `DSSClassDefs.pas` is **empty** (modulo the
   never-port list).
2. The staging family is **empty** (§3.1: every deck graduated with its WP)
   — delete `tests/corpus/gaps/` and its `corpus_live.rs` gate section.
3. Full gate + live corpus run; COVERAGE.md refresh; STATUS.md record
   (§1-style); PORTING_PLAN.md cross-link ("GAPS_PLAN executed").
4. Merge per the per-phase convention — only on explicit user request.

## 5. Execution notes

- Order: WPG.1–WPG.11 are small and independent — run them in numeric order
  (cheapest first, the warm-up convention); then the element WPs by size —
  **WPG.14 (Isource)**, **WPG.16 (GIC family)**, **WPG.15 (AutoTrans,
  stage-committed A→B→C)**; WPG.12 next (corpus payoff); **WPG.18 (CIM XML
  export, stage-committed A→F)** once WP8.6 and WPG.15 are in; WPG.13 last
  (largest, and gated on WP8.6 for its corpus decks). The element WPs are
  fully independent of everything else and can also be interleaved earlier —
  WPG.14 is a good warm-up-sized package.
- Every WP: the live gate runs against the PIN (`tools/golden/PIN.txt`);
  deck or manifest edits require re-running the §3 validation protocol
  (two-process determinism + sensitivity) on the pinned oracle.
- The decks are already committed and oracle-validated — do **not** tweak
  them casually during porting; if a port needs a different scenario, add a
  new deck (same protocol) rather than repurposing a validated one.
