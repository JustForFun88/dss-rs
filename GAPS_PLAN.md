# GAPS Plan — closing the test-blocked deferrals (synthesized decks + porting WPs)

> Companion to `PORTING_PLAN.md`; same rules of engagement as
> `PHASE4_PLAN.md` §0 / `PHASE7_PLAN.md` §0 / `PHASE8_PLAN.md` §0 (Pascal is the
> spec; probe the oracle, never guess FPC semantics; `TODO(compat)` /
> `NOT_PORTED` discipline; goldens regenerated **manually** with the pinned
> oracle `tools/golden/PIN.txt`; the standard three-command gate green per WP;
> the full per-step ritual — gate → STATUS.md → commit → `/audit-code` +
> `/audit-tests` by independent agents → fix-commits → STATUS sync — applies
> per WP; commit only on explicit user request; **final per-step report to the
> user in Russian**).
>
> **What this plan is.** During Phases 4–7 a set of features was deferred with
> the empirical rule "port only if a corpus case needs it" — and the vendored
> corpus (`tests/corpus/electricdss-tst`) has **no deck** for them, so they
> stayed unported (the WP7.9 AutoAdd/Monte/LD decision and its smaller
> cousins). `PHASE8_PLAN.md` §1 already names that rule the anti-pattern and
> mandates **synthesized fixtures**: *test-absence is never a reason to defer a
> port*. This plan is the promised correction: it inventories every
> test-blocked deferral, ships a **validated synthesized deck** for each
> (committed at `tests/corpus/gaps/` — the third first-party live-gate family
> beside `asymmetric/` and `controls/`, see §3), and packages the porting work.
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
- GIC elements, CIM export, actors/`SolveAll`, `Pstcalc` flicker — Phase 9.
- All user-model DLL hooks — never (safe Rust).

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
  a feature flips its cases to `pending: false` in the same commit. WPG.14
  asserts no `pending: true` remains.
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

Sixteen decks at `tests/corpus/gaps/` with their `manifest.json` (+ 6
committed binary/CSV input fixtures; regenerate via
`tools/corpus/gen_gaps_binshapes.py`). Validation protocol, run 2026-07-05
against the pinned oracle (dss-python 0.15.7):

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

### WPG.14 — Exit sweep [2%]

1. `rg "no corpus case"` / `rg "NOT_PORTED"` / the §1 table — every row
   either ported+gated here or explicitly re-owned (§1 "NOT in this plan"
   list); no "test-absence" deferral survives anywhere in the tree.
2. Full gate + live corpus run; COVERAGE.md refresh; STATUS.md record
   (§1-style); PORTING_PLAN.md cross-link ("GAPS_PLAN executed").
3. Merge per the per-phase convention — only on explicit user request.

## 5. Execution notes

- Order: WPG.1–WPG.11 are small and independent — run them in numeric order
  (cheapest first, the warm-up convention); WPG.12 next (corpus payoff);
  WPG.13 last (largest, and gated on WP8.6 for its corpus decks).
- Every WP: the live gate runs against the PIN (`tools/golden/PIN.txt`);
  deck or manifest edits require re-running the §3 validation protocol
  (two-process determinism + sensitivity) on the pinned oracle.
- The decks are already committed and oracle-validated — do **not** tweak
  them casually during porting; if a port needs a different scenario, add a
  new deck (same protocol) rather than repurposing a validated one.
