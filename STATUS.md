# dss-rs — Project Status & Session Handoff

> **Purpose of this file:** a living snapshot so a fresh session can resume
> without re-deriving context. It records *what is done*, *what was decided*,
> and **why**. It is **not** authoritative for the plan itself — that is
> `PORTING_PLAN.md` (roadmap + binding decisions) and `CLAUDE.md` (conventions
> + the green-gate rule). Read those two first; then read this for the current
> frontier.

Last updated: 2026-06-21 — **Phase 7 IN PROGRESS** (branch
`phase-7-extended-elements`): **WP7.1 step 3b done** (Line `spacing=`/`wires=`/
`cncables=`/`tscables=` Carson path, gate-green), **step 4 next** (the geometry/
spacing corpus feeder + targeted golden). The last merged phase was **Phase 6
(WP6.1–WP6.10), MERGED to `main`** (`--no-ff` merge `b98223a`, gate green at
merge; `main` not pushed to origin). The branch `phase-6-meters-topology` carried WP6.1–WP6.9 through
`d1cc68c` + the Yeq/checkpoint follow-ups, the live-corpus infra
`19a5493`/`593420f`, WP6.10 phase-exit `207b9cb`, and the post-exit live-gate
deepening + audit hardening through `cc6d2e2`. Phase 5 had merged earlier
(`10d3550`).
Execution plan: **`PHASE6_PLAN.md`** (WP6.1–WP6.10: meters/monitors/topology/
Generator, 8500-node gate). **The phase gate passes: the unmodified IEEE
8500-Node master (+ `Energymeter.m1` + a 24-step daily run) converges in 67
iterations with `YNodeOrder` exact (8531 nodes), node voltages / total power /
losses at 1e-6 rel, all 12 RegControl tap numbers + 10 capacitor states exact,
and all 67 EnergyMeter registers at 1e-4 rel.** All WPs done: **WP6.1 (topology
foundations), WP6.2 (Generator), WP6.3 (MeterElement + Monitor), WP6.4
(EnergyMeter + zone build), WP6.5 (EnergyMeter registers + TakeSample), WP6.6
(reliability: fault-rate sweep + `RelCalc`), WP6.7 (Sensor + load allocation),
WP6.8 (GenDispatcher + StorageController & AutoAdd skeletons + ReduceAlgs
basic), WP6.9 (goldens + the 8500-node gate), WP6.10 (phase exit)** — see the [Phase 6 record](docs/phase-records/phase-6.md).
**Phase 7 IN PROGRESS** — `PHASE7_PLAN.md` written (WP7.1–WP7.10); branch
`phase-7-extended-elements` cut from `main`; **WP7.1 step 1 done** (the Carson
line-constants engine `support/line_constants/`), **step 2a done** (the
conductor catalog `WireData`/`CNData`/`TSData`), **step 2b done**
(`LineSpacing`), **step 2c-i done** (the `LineGeometry` object + edit state
machine + props/`MakeLike`), **step 2c-ii done** (the matrix wiring —
`UpdateLineGeometryData`/`CalcMatrices` driving the Carson engine, Z/Yc pinned
to the oracle), **step 3a done** (Line's `geometry=` Carson path —
`FetchGeometryCode`/`FMakeZFromGeometry`, Z/Yc/YPrim pinned to the oracle)
**and step 3b done** (Line's `spacing`/`wires`/`cncables`/`tscables` path —
`FetchLineSpacing`/`SetWires`/`LoadSpacingAndWires`/`FMakeZFromSpacing`,
incl. the buried-neutral form, Z/Yc pinned to the oracle),
all gate-green — see §1e; **WP7.1 step 4 next** (the geometry/spacing corpus
feeder migration + targeted golden `phase7/line_geometry*.json`).
Phase 7 = DER, protection, line constants, harmonics, dynamics; PORTING_PLAN.md
§Phase 7, the largest phase ~18%.

Earlier — **Phase 5 COMPLETE (WP5.1–WP5.10), gate-green, merged** —
the **phase gate passes: the unmodified IEEE13/IEEE37/IEEE123 masters
(controls ACTIVE) compile, solve and match the Phase-0 goldens** — iteration
counts exact (ieee13: 11), final taps / RegControl tap numbers / capacitor
states, node voltages and per-element powers/currents at 1e-6 rel, and every
element's full property dump (numeric skeleton). **The ieee34mod1 stretch goal
also passes.** The `phase5.json` command-replay gate (daily/duty/event-log/
capcontrol scenarios) matches the oracle — the 24-hour tap-change trajectory is
event-log-identical. See the [Phase 5 record](docs/phase-records/phase-5.md).

Earlier — **Phase 4 COMPLETE (WP4.1–WP4.10)** — all core PD
elements (Transformer/Capacitor/Reactor), catalog objects
(LineCode/XfmrCode/GrowthShape), the Line→LineCode fetch path, parse-only
RegControl/CapControl, the `define_properties!` macro (bounded scope), and the
**phase gate: the IEEE13/IEEE37/IEEE123 controls-off feeders solve and match
the oracle** (exact iteration counts + node order; voltages, per-element
powers/currents, total power and losses at 1e-6 rel). Merged to `main`
(`5f27a25`); developed on branch `phase-4-pd-elements`.

> **Working cadence (per PHASE4_PLAN §0.8):** finish one small step → run the
> full gate → update this file → **stop and wait for explicit user
> confirmation** before the next step. (WP4.7–4.10 were executed in one pass on
> explicit user instruction.)

---

## 1. Where we are

| Phase | Scope | Status |
|------|-------|--------|
| 0 | Tooling, oracle, faer spike, CI, Phase-0 goldens | ✅ done (committed) |
| 1 | Shared math (`support/`) + full `TDSSParser` port | ✅ done (commit `729eb77`) |
| 2 | Object model, property engine, executive skeleton | ✅ done (commit `22f861d`) |
| 3 | ★ Vertical slice: parse → circuit → Y matrix → solve → voltages | ✅ done (commit `2ac8691`) |
| **4** | **Transformer/Capacitor/Reactor/LineCode + controls (parse-only) + macro + feeder gate** | ✅ done (merged to main, `5f27a25`); `PHASE4_PLAN.md` |
| **5** | **LoadShape/XYcurve/controls behavior, control queue, time modes + feeder gate (controls active)** | ✅ done (merged to main, `10d3550`); `PHASE5_PLAN.md` |
| **6** | **Meters/Monitors/topology/Generator + 8500-node gate + live corpus gate** | ✅ done (merged to main, `b98223a`); `PHASE6_PLAN.md` |
| 7 | Extended elements: DER, protection, line constants, harmonics, dynamics | 🚧 in progress — `PHASE7_PLAN.md` written (WP7.1–WP7.10); branch `phase-7-extended-elements`; WP7.1 in progress (step 3b Line `spacing`/`wires`/`cncables`/`tscables` Carson path done; step 4 corpus feeder + golden next) |

### Gate state (all green)
```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace      # dss-core lib 388, golden_feeders 1,
                            # golden_feeders_controls 4, golden_phase5 1,
                            # golden_phase6 1, golden_checkpoints 1,
                            # golden_ieee8500 1, golden_reliability 1,
                            # golden_allocation 1, golden_gendispatcher 1,
                            # golden_autoadd_reduce 1, golden_slice 2,
                            # golden_smoke 3, props_roundtrip 1,
                            # corpus_manifest 1, corpus_live 3
                            #   (solvable_now_has_multistep_depth always-on;
                            #    solvable + classify auto-skip w/o
                            #    DSS_LIVE_ORACLE / DSS_LIVE_CLASSIFY),
                            # dss-parser 62+1, dss-sparse 5
```

### Phase 5 gate — green
- `golden_feeders_controls.rs`: the **unmodified** IEEE13/IEEE37/IEEE123
  masters (controls active; IEEE123 issues the `post: ["solve"]` from
  `cases.json`) match the committed Phase-0 goldens
  `tests/golden/{ieee13,ieee37,ieee123}.json`: converged + total iterations
  **exact** (ieee13: 11), `YNodeOrder` exact, RegControl `tap_number` and
  capacitor `states` **exact**, final transformer taps at 1e-12 rel (not
  bitwise: the engines' ~1e-9 sparse-solver voltage differences can shift a
  banker's-rounding boundary and repartition the *same net* tap movement into
  a different step sequence, leaving the float accumulation an ulp apart —
  the integer tap_number is the exact discrete check), node voltages /
  element powers / currents at 1e-6 rel, total power + losses at 1e-6, and
  **every element's full property dump** via the numeric-skeleton comparator.
  **`ieee34mod1` (stretch) passes too** — no `#[ignore]` needed.
- `golden_phase5.rs` vs `tests/golden/phase5/*.json` (one file per scenario;
  `tools/golden/gen_phase5.py`,
  command-replay like slice.json): `daily_ieee13` (24 hourly steps, every load
  on a 24-pt shape, regcontrol event logs on), `duty_2bus` (12×300 s steps,
  TIMEDRIVEN), `eventlog_ieee13` (`Set Log=yes`), `capcontrol_micro` (kvar
  control opens Cap1). Per-step `dblHour` exact; **the event logs match the
  oracle line-for-line** (normalized), pinning every tap change/cap switch of
  the trajectories; final taps/tap numbers/states exact. **Per-step iteration
  counts exact on every step and per-step voltages at 1e-6 rel** — the whole
  daily trajectory tracks the oracle since `build_y_matrix` restamps each
  load's shape-scaled `Yeq` per Y build (commit `a6903f1`).

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
- **Live comparison (`DSS_LIVE_ORACLE=1`; runs in the `live-oracle` CI job).**
  For each of the **17** `solvable_now` cases the gate compiles+solves on the Rust
  engine and on the pinned dss-python oracle (`tools/oracle/oracle_server.py`, a
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

### Phase 4 gate (`crates/dss-core/tests/golden_feeders.rs`) — green
The three committed **controls-off variants** (`tests/golden/phase4/
{ieee13,ieee37,ieee123}_controlsoff.dss`, generated from the unmodified IEEE
masters by `tools/golden/gen_phase4.py`) compile and solve in both engines;
against `tests/golden/phase4.json` (pinned oracle) the Rust engine matches:
- converged flag and fixed-point iteration counts **exactly** (3/3/3);
- `YNodeOrder` **exactly** (41 / 117 / 278 nodes — control elements attach to
  existing buses and add none);
- node voltages within **1e-6 rel** (1e-9 abs floor);
- **every element's** terminal powers and currents within 1e-6 rel
  (1e-4 abs floor — dead-end branch currents are differences of nearly equal
  voltages, so 1e-6-rel voltage agreement caps absolute current agreement at
  the µA scale), in the oracle's First/Next (= creation) order, names checked;
- total power and total losses within 1e-6 rel.

The Phase 3 gate (`golden_slice.rs`, 13 scenarios) stays green, and the CLI
runs the real masters end to end: `cargo run -p dss-cli -- script.dss`.

---

## 1b–1d. Completed-phase records (archived)

The full work-package logs for the completed, merged phases live under
`docs/phase-records/` to keep this handoff lean. They are frozen history,
superseded only by the code and tests:

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

---

## 1e. Phase 7 record (branch `phase-7-extended-elements`) — IN PROGRESS

Execution plan: **`PHASE7_PLAN.md`** (WP7.1–WP7.10). Per-WP cadence — the full
ritual in `PHASE7_PLAN.md §0`, run autonomously per step: gate green → update this
file + commit → `/audit-code <step scope>` → fix + commit → `/audit-tests <step
scope>` → fix + commit → **full `STATUS.md` review + sync + archive-cleanup** +
commit → **then** stop for confirmation.

**WP7.1 step 1 — Carson line-constants engine (all 4 specializations) — ✅ done, gate-green, committed.**
- `src/support/line_constants/` — `mod.rs` (`LineConstants` = Pascal
  `TLineConstants`, `General/LineConstants.pas`) + the **four specializations**
  the plan calls for: `oh.rs` (`OhLineConstants`, a plain alias — `TOHLineConstants`
  adds nothing), `cable.rs` (`TCableConstants` shared cable data + its
  `ConductorsInSameSpace` override), `cn.rs` (`CnLineConstants` =
  `TCNLineConstants`), `ts.rs` (`TsLineConstants` = `TTSLineConstants`). No Rust
  inheritance: a `LineConstantsKind` enum (Overhead/ConcentricNeutral/TapeShield)
  on the one struct switches `Calc`/`ConductorsInSameSpace`; the cable arrays are
  allocated only for the cable kinds (`new`/`new_cn`/`new_ts` constructors). Pure
  math engine beside the other `support/` helpers; reuses `support/cmatrix/mod.rs`
  (Kron, invert), `support/line_units/mod.rs`, `support/mathutil/mod.rs`
  (`bessel_i0`/`bessel_i1` for the DERI skin-effect `Zint`). 0-based indices.
- Ported verbatim: base `Calc(f, EarthModel)` (self/mutual Z, the P→invert→Yc
  path), `Get_Zint` (SimpleCarson/FullCarson no-skin vs DERI Bessel skin effect),
  `Get_Ze` (all three earth models — SimpleCarson, FullCarson Tleis series, DERI
  complex earth factor `Fme`), `Kron`/`Reduce`, the unit-converting
  `z_matrix`/`yc_matrix` getters, GMR↔radius defaulting setters, and overhead
  `conductors_in_same_space`. **CN `Calc`** (`CNLineConstants.pas`): append the
  concentric neutrals as extra conductors, build with the strand
  resistance/GMR/`RadCN` power-mean spacing, Kron the neutrals out, build Yc
  directly as the coaxial insulation admittance. **TS `Calc`**
  (`TSLineConstants.pas`): same shape with the tape-shield resistance/GMR.
  **Cable `ConductorsInSameSpace`**: no height check, `0.5*DiaCable` radius for
  neutral conductors. (`TCableConstants.Kron` is identical to the base, so it is
  not re-implemented.)
- **`TODO(compat)`:** truncated upstream constants `mu0 = 12.56637e-7`,
  `Twopi = 6.283185307` (a *distinct* quantity from `2·PI` — FullCarson/Zint use
  full `std::f64::consts::PI`), `e0 = 8.854e-12`; plus the tape-shield `0.3183`
  (truncated `1/pi`) in `ts.rs`. All carry `#[allow(clippy::approx_constant)]`.
  Zero-pivot in `invert`/`kron` unchecked (existing cmatrix `TODO(compat)`).
- **Precondition documented** on `calc`: geometry must be filled first
  (`Rdc`/`radius`/`GMR` init to the `-1.0` sentinel → non-finite entry, not an
  error, if left unset — the geometry layer / `ConductorData`'s `Rdc = Rac/1.02`
  default is responsible). Data flow for steps 2–3:
  `TLineGeometryObj.UpdateLineGeometryData` sets the engine arrays from the wire
  objects (incl. the CN/TS cable fields), then `Calc(f, ActiveEarthModel)` + an
  optional `Reduce`; `capradius` defaults to `radius`.
- **Gate:** 17 inline tests pinned against the dss-python oracle (PIN.txt 0.15.7 /
  backend 0.14.5), probed (via `tools/golden/probe_line_constants_phase7.py`) by
  building the geometry through a `Line` and reading `Rmatrix`/`Xmatrix` (ohm/m) +
  `Cmatrix` (nF/m): 3-phase overhead under **all three earth models** + a 4→3 Kron
  reduce; **3-phase CN cable and TS cable** each under **all three earth models**
  (full Z + coaxial C); **CN and TS cable** at a **non-power-frequency** (f = 5 kHz
  → the radius/`Zi.im`-retained branch, Z only — the oracle `Cmatrix` getter scales
  reported nF by the solve frequency); a **CN cable 4→3 Kron reduce** (3 phases + a
  bare-neutral core, the cable reduced-Z/Yc + `reduced_size>0` re-reduce path); a
  **non-power-frequency** overhead case; a **rho_earth = 200** recalc
  (`set_rho_earth` + `z_matrix` `frho_changed` path); a **unit/length conversion**
  (ohm·km over 2 km); overhead + cable `ConductorsInSameSpace`. Entry-by-entry at
  1e-8 rel. dss-core lib **319 → 336**. Full three-command gate green.
  - *(audit-tests follow-up)* The earlier suite ran the cable `Calc` under DERI /
    60 Hz / unreduced only; the 6 added cable tests close the earth-model,
    high-frequency, and reduction branch gaps the test audit flagged.
- **Deferred (tracked):** the units-converting per-conductor *read* getters
  (`Get_GMR`/`radius`/`Rdc`/`Rac`/`X`/`Y`/`Capradius`) are not ported — they have
  no consumer until the LineGeometry report/dump path; they land in step 3 with it.

**WP7.1 step 2a — conductor catalog (`WireData`/`CNData`/`TSData`) — ✅ done,
gate-green, committed.**
- `src/elements/general/conductor_data/mod.rs` (new) — port of Pascal
  `General/{ConductorData,WireData,CNData,TSData,CableData}.pas`. Pascal's type
  hierarchy is `TConductorDataObj → TWireDataObj` and `TConductorDataObj →
  TCableDataObj → TCNDataObj/TTSDataObj`; Rust has no inheritance, so the shared
  blocks live in two private cores — `ConductorDataCore` (the 13 `TConductorData`
  props: Rdc/Rac + units, GMR, radius/diam, norm/emerg amps, Seasons/Ratings,
  CapRadius, with the full side-effect web) and `CableDataCore` (EpsR/InsLayer/
  DiaIns/DiaCable + error checks). Each concrete class embeds the cores it needs
  and maps its own 1-based ordinal onto the relevant block.
- **Property order matches the oracle exactly** (probed): a leaf's own props
  lead, then `CableData`, then `ConductorData` — the Pascal `inherited
  DefineProperties` chain. So WireData = 13 ConductorData props; CNData = 4 own
  (k/DiaStrand/GMRStrand/RStrand) + 4 cable + 13 conductor = 21; TSData = 3 own
  (DiaShield/TapeLayer/TapeLap) + 4 cable + 13 conductor = 20. Each via
  `define_properties!` (ordinals inline — the leaves differ, so the conductor
  table is repeated per class rather than shared by a fn).
- **Side-effect web ported verbatim:** Rac↔Rdc (`×1.02`/`÷1.02`), GMRac→radius
  (`÷0.7788`) + radius-zero error, radius/diam→GMR (`×0.7788`) + CapRadius default,
  GMRunits↔radunits seeding, normamps↔emergamps (`×1.5`), Seasons→`AmpRatings`
  resize; CN DiaStrand→GmrStrand (`0.7788·0.5·Dia`); the critical-error checks
  (k<2, EpsR<1, Ins/Dia/shield/tape positivity, TapeLap∈[0,100]) as deferred
  messages. `diam` shares the radius field via the engine's 0.5 prop scale
  (`Diam` dumps `FRadius/0.5`). **`MakeLike` copies neither `NumAmpRatings` nor
  `AmpRatings`** (Pascal quirk) — a `like=` conductor keeps its own `[ -1]`.
- Defaults reproduced exactly (probed): every spec field inits to the `-1.0`
  sentinel (so a bare WireData dumps `Rdc=-1 … Diam=-2 Ratings=[ -1]`), units
  ordinal 0 dumps `none`, `EpsR=2.3`, `TapeLap=20`, `k=2`. Registered in
  `exec::Dss::new` after the shape classes (Pascal DSSClassDefs.pas registers
  WireData/CNData/TSData after Spectrum, before LineGeometry).
- **Gate:** 8 inline tests (diam/dynamic-default couplings, GMR-seeds-radius,
  MakeLike-skips-ratings, CN strand-GMR default, the k<2 / EpsR<1 / TapeLap-range
  error paths, TS defaults) + 14 oracle-pinned `props.json` scenarios across new
  `props/{wiredata,cndata,tsdata}.json` (default/full/abbrev/diam/gmr-only/
  ratings/makelike for wire; default/full/strand-default/makelike for CN;
  default/full/makelike for TS); `props_roundtrip` green. dss-core lib **336 →
  344**. Full three-command gate green.
- **Deferred (tracked):** the units-converting per-conductor *read* getters
  (`Get_GMR`/`radius`/`Rdc`/`Rac`/`X`/`Y`/`Capradius`) land in step 3 with the
  LineGeometry data-flow.

**WP7.1 step 2b — `LineSpacing` (`TLineSpacingObj`) — ✅ done, gate-green, committed (`620cf89`).**
- `src/elements/general/line_spacing/mod.rs` (new) — port of Pascal
  `General/LineSpacing.pas`. A `DSS_OBJECT` catalog class: 5 props (`nconds`
  [SuppressJSON], `nphases`, `x`, `h` [both `DoubleVArray` sized by `FNConds`
  via `PropertyOffset2 = @FNConds`], `units` [mapped string enum]). Registered
  after `TSData`, before `LineGeometry` (Pascal `DSSClassDefs.pas`).
- **Side effects ported verbatim:** the `nconds` setter reallocates `FX`/`FY`
  to the new count (grown tail zero-filled — Pascal's `ReAllocmem` leaves it
  uninitialized, undefined memory the goldens do not pin) and resets `Units` to
  `ft`; `MakeLike` copies `FNConds`/`NPhases`/`FX`/`FY` then `Units :=
  Other.Units` (overriding the side-effect's ft reset).
- `nconds=0` empties the buffers: Pascal `ReAllocmem(FX, 0)` nils the pointer,
  so `GetDSSArray` returns `''` (not `'[]'`) — `get_f64_array` mirrors this by
  reading an empty coordinate array as nil (audit follow-up; was `'[]'`).
- Oracle-pinned: 14 `linespacing_*` `props.json` scenarios (default, full,
  units=m, array clamp/zero-fill, shrink-nconds truncation+units-reset, makelike,
  zero-nconds empty-string, units mi/kft/km/none/in/cm/mm) + 7 inline unit tests;
  `props_roundtrip` green. dss-core lib **344 → 351** (audit follow-ups added the
  `nconds_grow_*` zero-fill invariant, the `nconds=0`→`''` fix + golden, and the
  negative-`nconds` clamp invariant test — the oracle raises on negative
  `nconds`, so that path is Rust-only). Full three-command gate green.
- **Audit-tests follow-up (`/audit-tests` step 2b, `b5d5201`):** rounded the units
  golden out to **all 9 `LineUnits` ordinals** — added `linespacing_units_{in,cm,mm}`
  (the only Minor finding; the per-class plumbing was already covered by 6
  ordinals + the full mapping by `dss_enum/mod.rs`). Regenerated with the pinned
  oracle; only `linespacing.json` changed (11 → 14). The two Rust-only invariant
  tests (`nconds_grow_*`, negative-`nconds`) needed no change — documented
  divergences with no oracle to pin. Gate green.

**WP7.1 step 2c-i — `LineGeometry` (`TLineGeometryObj`) object + edit state
machine — ✅ done, gate-green, committed.**
- `src/elements/general/line_geometry/mod.rs` (new) — port of Pascal
  `General/LineGeometry.pas` (the object, props, side-effect web, `MakeLike`).
  19 props via `define_properties!` in the exact oracle order
  (`nconds`/`nphases`/`cond`/`wire`/`x`/`h`/`units`/`normamps`/`emergamps`/
  `reduce`/`spacing`/`wires`/`cncable`/`tscable`/`cncables`/`tscables`/`Seasons`/
  `Ratings`/`LineType`). Registered after `LineSpacing` (Pascal
  `DSSClassDefs.pas`).
- **Per-conductor state machine** keyed by `cond=` (`FActiveCond`): `wire=`/
  `cncable=`/`tscable=`/`x=`/`h=`/`units=` route to the active conductor's slot
  internally (no engine change — the active index is object state). `cond` is
  range-guarded `1..=NConds` (Pascal `set_ActiveCond` ignores out-of-range; the
  generic CAPI struct-index error is not reproduced, the transformer `wdg`
  precedent). Units are sticky via `FLastUnit`. `spacing=` copies a
  `LineSpacing`'s coordinates into every conductor (and clears the `X`/`H` set
  marks); `wires=`/`cncables=`/`tscables=` fill all slots (Pascal `SetWires`,
  count-validated, the `AllowAllConductors`/JSON branch skipped as JSON-only).
  Conductor/spacing refs are resolved + **snapshot-cloned** at edit time
  (WP4.2 `FetchLineCode` pattern); the cloned conductors seed `NormAmps`/
  `EmergAmps`/`NumAmpRatings`/`AmpRatings` from the first conductor.
- **New property kind:** `PropType::ObjectRefArray` + `PropDef::object_ref_array`
  + `set_object_ref_array`/`get_object_ref_names` (the `wires`/`cncables`/
  `tscables` `DSSObjectReferenceArrayProperty`; renders `[a, b, c]`, empty `[]`).
- Oracle-pinned: 7 `linegeometry_*` `props.json` scenarios (default —
  `X`/`H`/`Units` skipped, the oracle raises on the unallocated `NConds=0`
  arrays; overhead cond/wire + reduce; spacing form; CN cable; makelike;
  multi-season ratings default; buried-neutral `cncable`+`wires=`) + 10 inline
  unit tests + an exec parse-path test; `props_roundtrip` green. dss-core lib
  **351 → 362**. Full three-command gate green.
- **Audit follow-up (`/audit-code` step 2c-i):** three findings settled against
  the pinned oracle and fixed. (1) The `wire`/`cncable`/`tscable` side effect now
  logs the Pascal 10103 "WireData/CNData/TSData object was not defined" when the
  active conductor is NIL (the generic ObjectRef 401 stays — upstream emits
  both). (2) `PropType::ObjectRefArray` now `Exit`s on the first unresolved token
  (Pascal `DSSObjectHelper` array property), so a bad name no longer drops the
  token and trips a spurious "Unexpected number" count error. (3) The
  `NumAmpRatings>1`/`AmpRatings` ratings-default branches (and the `cond`
  out-of-range clamp) were untested — added the two goldens above plus the
  exec/inline tests; the conductor invariant `NumAmpRatings == len(AmpRatings)`
  makes the array-branch `take(n)` copy identical to Pascal's full-length copy
  (no code change needed there).
- **Audit-tests follow-up (`/audit-tests` step 2c-i, `2b54849`):** the test
  audit found the **tape-shield path had zero executable coverage** (no test ever
  wrote `tscable=`/`tscables=`, so the `conductor_amps` `TsDataObj` arm was dead)
  plus minor happy-path-only gaps. Added 4 oracle-pinned scenarios (linegeometry
  7 → 11), each settled empirically against the pinned oracle first:
  `linegeometry_ts` (scalar `tscable=` + `linetype=ug_ts` — exercises TSData
  resolution, TapeShield kind, the `TsDataObj` amps default, and a non-default
  LineType); `linegeometry_nphases_gt_nconds` (NPhases stored raw at parse —
  confirms the `FLineData.Nphases` clamp is correctly deferred to 2c-ii);
  `linegeometry_seasons_direct` (the `Seasons` resize side effect); and
  `linegeometry_normamps_explicit` (explicit amps survive a later conductor).
  Also corrected the inline `scalar`-helper doc comment, which over-claimed full
  resolution coverage. `props_roundtrip` green (now exercises the TS path).
- **Surfaced, NOT fixed (needs investigation, tracked):** the **plural cable**
  forms `cncables=`/`tscables=` leave the oracle's active conductor at `Cond=1`
  for a bare assignment, whereas Rust's `set_wires` leaves it at `istop` (and
  overhead `wires=` stays at `istop` in the oracle too). The exact rule is
  intricate — after a prior `cond=2 cncable=`, a following `cncables=[…]` reads
  `Cond=2` (a buried-`istart` count-error Exit), and the **vendored
  `LineGeometry.pas` SetWires/ChangeLineConstantsType do not contain this reset**
  (source says `istop`), so the pinned 0.14.5 binary diverges from the vendored
  revision here. Porting it faithfully needs that reconciliation, so it was not
  guessed/hacked and no failing golden was added; the plural-cable forms stay
  un-pinned for now (their CN/TS *data* paths are covered via the scalar
  scenarios). Empirically probed against the oracle (`/audit-tests` follow-up).
**WP7.1 step 2c-ii — `LineGeometry` matrix wiring (`UpdateLineGeometryData`/
`CalcMatrices`) — ✅ done, gate-green, committed `0258911` (+ audit-code
follow-up, gate-green).**
- `line_geometry/mod.rs` now holds a real `FLineData: Option<LineConstants>` Carson
  engine (replacing the placeholder `fline_kind` tracker). Ported:
  - `change_line_constants_type` — the Pascal `needNew` allocate/swap (kind ≠ the
    active conductor's choice, or `FLineData` absent / wrong conductor count),
    preserving `Nphases`/`RhoEarth` across the swap; allocates only the three
    concrete kinds (an `Unknown` request leaves the engine, as Pascal).
  - `realloc_conductors` (the `nconds` side effect) now rebuilds a fresh overhead
    engine sized `FNConds` (Pascal's per-conductor `ChangeLineConstantsType`
    loop), `None` when `NConds=0` (Pascal NIL).
  - the `nphases` side effect clamps `FLineData.Nphases` to `min(NPhases, NConds)`
    (the previously-deferred clamp; `UpdateLineGeometryData` later re-sets it
    unclamped, as upstream).
  - `update_line_geometry_data(f, earth_model)` — pushes every conductor's
    geometry into the engine (X/Y in `FUnits`, radius/capradius/GMR/Rdc/Rac, and
    the CN/TS cable extras), sets `Nphases`, clears `data_changed`, runs
    `ConductorsInSameSpace` → `Calc(f, earth_model)` → `Reduce` (if `FReduce`).
    Returns `Err` for the two Pascal abort paths: a NIL conductor slot
    (`raise Exception` "not correctly initialized") and a failed geometry check
    (`ELineGeometryProblem`/`SolutionAbort`).
  - `z_matrix`/`yc_matrix`/`rho_earth`/`set_rho_earth` — the `Get_Zmatrix`/
    `Get_YCmatrix`/`Get_/Set_RhoEarth` accessors (recompute when `data_changed`),
    the public surface step 3's Line consumes.
- **Conductor catalog** (`conductor_data/mod.rs`): new `ConductorGeom`/`CableGeom`
  + `geom()` on each of `WireData`/`CNData`/`TSData` + a `conductor_geom(&dyn)`
  dispatch — the engine inputs Pascal reads off `FWireData[i]` (in the object's
  own unit codes; the engine converts).
- **`MakeLike` divergence (documented):** Pascal rebuilds an overhead engine then
  runs `UpdateLineGeometryData`, which for a *cable* source raises `EInvalidCast`
  (FLineData overhead, conductors CN/TS). We instead **clone the source engine**
  so the kind matches and defer the recompute (`data_changed=true`); observable
  props are unchanged (no matrix props are dumped), so `props_roundtrip` is
  unaffected. Not a `TODO(compat)` (no golden pins it; it averts an upstream
  crash on an untested path).
- Oracle-pinned: 3 new inline matrix tests drive the full object path
  (`nconds`/`cond`/`wire`/`x`/`h`/`units`) and assert Z/Yc against the **same
  dss-python references** the Carson-engine unit tests pin —
  `matrices_overhead_match_oracle` (3-phase OH, DERI, Z + full C),
  `matrices_reduce_neutral_to_phases` (4→3 Kron reduce), `matrices_cn_cable_
  match_oracle` (CN cable param transfer) — plus 2 error-path tests
  (`update_uninitialized_conductor_errors`, `update_conductors_in_same_space_
  errors`). `LineConstants` gained `#[derive(Clone)]`. dss-core lib **362 → 367**.
  Full three-command gate green.
- **Audit-code follow-up (committed separately):** (1) `change_line_constants_type`
  `needNew` restored to Pascal's exact boolean **OR** (choice-changed *or*
  engine-NIL/wrong-count) — previously a `match` that skipped the NIL/count clause
  when the active choice was unchanged; behaviourally identical under the realloc
  invariant, now a literal 1:1 port that self-heals if the invariant is ever broken.
  (2) Test coverage closed for the paths 2c-ii added but left unexercised:
  `matrices_ts_cable_match_oracle` (the **TS** object→engine transfer — DiaShield/
  TapeLayer/TapeLap — vs the engine `ts_cable_deri_3cond` reference),
  `make_like_cn_cable_recomputes` (pins the documented MakeLike clone divergence:
  `like=` a CN geometry does not crash and reproduces the source Z), and
  `z_matrix_recomputes_on_frequency_change` (guards `f` forwarding / the engine's
  `f != FFrequency` recompute branch — all prior matrix tests used 60 Hz only).
  dss-core lib **367 → 370**. Gate green. *(Not addressed: the `Get_Zmatrix`/
  `Get_YCmatrix` pre-existing-`SolutionAbort` NIL gate — it needs the solution
  handle the geometry object lacks; correctly belongs to the step-3 `Line`
  consumer and is tracked there.)*
- **Audit-tests follow-up (`/audit-tests` step 2c-ii, committed `6fa2f1c`):**
  strengthened the matrix unit tests where the object→engine *forwarding* of the
  `z_matrix`/`yc_matrix` args was under-exercised — every prior matrix test used
  `length = 1`, `units = m`, `earth_model = DERI`, so only `f` was proven to reach
  the engine. Added `matrices_overhead_km_scaled` (length = 2 / units = km ⇒ Z/Yc =
  the per-meter result × 1000 × 2 — catches a hardcoded 1.0/meters or a swapped
  length/units arg) and `matrices_overhead_simple_carson` (pins the engine
  `simple_carson_full_3cond` reference — proves `earth_model` is forwarded, not
  hardcoded); extended `matrices_reduce_neutral_to_phases` with the reduced-**Yc**
  assertion (engine `deri_reduce_4cond_to_3` capacitance, previously Z-only); and
  replaced the soft `z_matrix_recomputes_on_frequency_change` `>1.5×` inequality
  with an oracle-pinned 5 kHz recompute (engine `overhead_high_freq_radius_branch`)
  plus the return-to-60 reproduction. Factored the 4× duplicated overhead build
  into a `build_overhead_3()` helper. dss-core lib **370 → 372**. Gate green.
- **Still open (tracked):** the step-2c-i plural-cable `cncables=`/`tscables=`
  active-conductor divergence (above) — independent of the matrix wiring.

**WP7.1 step 3a — Line `geometry=` Carson path (`FetchGeometryCode`/
`FMakeZFromGeometry`) — ✅ done, gate-green, committed.**
- `line/mod.rs`: un-`NOT_PORTED` the **`geometry`** scalar ref
  (`object_ref_class("LineGeometry", "geometry")`); the other geometry forms
  (`spacing`/`wires`/`cncables`/`tscables`) stay `NOT_PORTED` for step 3b. New
  fields `geometry_obj: Option<LineGeometryObj>` (snapshot-cloned at resolve time,
  the WP4.2 `FetchLineCode` pattern), `geometry_name`, `fz_frequency` (Pascal
  `FZFrequency`, `-1` sentinel).
- Ported verbatim: **`FetchGeometryCode`** (clone the geometry, push a pre-set
  `rho` in, copy NormAmps/EmergAmps/NumAmpRatings/AmpRatings/LineType, set
  `NPhases := geom.Nconds` *reduce-aware* + `set_nconds`, clear the superseded
  sym/linecode seq marks, `SymComponentsModel := False`); **`FMakeZFromGeometry(f)`**
  (the `f = FZFrequency` skip-guard, then `Z := geom.Zmatrix[f, len, units]` /
  `Yc := geom.YCmatrix[…]` under the Line's `FEarthModel` — Z/Yc are **total**,
  length+units already folded in); **`KillGeometrySpecified`**. `set_object_ref`
  fetches on resolve (the `linecode` pattern); the sym/matrix/switch side effects
  and a post-`geometry` `rho=` now drive `KillGeometrySpecified` / push `rho` into
  the geometry; the `phases=` guard rejects a phase change under a geometry (as
  under a matrix model — it reverts `nphases` and logs 18101; see the audit
  follow-up below). `MakeLike` carries the three new fields.
- **`CalcYPrim` split into two paths** (Pascal `CalcYPrim`): the geometry branch
  inverts the total `Z` directly (no length/freq/Rg/Xg scaling) and adds the
  **full** `Yc/2` shunt; the sym/linecode branch is byte-for-byte the old code
  (per-unit-length Z scaled by length·freq + earth return). Shared Kron embed +
  CAP_EPSILON + open-conductor tail.
- **`LineGeometryObj`** gained the public accessors `FetchGeometryCode` consumes
  (`nconds` reduce-aware = Pascal `Get_Nconds`; `norm_amps`/`emerg_amps`/
  `num_amp_ratings`/`amp_ratings`/`line_type`) + a manual `Debug` (it owns
  `Box<dyn DssObject>` conductor slots and `Line` derives `Debug`).
- **Deferred F2 now lands (audit follow-up):** a geometry `Zmatrix` error
  (`ELineGeometryProblem`/NIL conductor) is recorded via `push_error`; the Y-build
  loop (`ymatrix::build_y_matrix`) drains the queued message into `env.errors` and
  sets `solution_abort` — the faithful equivalent of Pascal `SolutionAbort` + Exit
  (the trait still has no direct abort channel, so the drain is the sink). To keep
  the abort robust across re-solves, `update_line_geometry_data` now clears
  `data_changed` only on a *successful* calc (Pascal clears it before the check but
  relies on the exception halting the solve outright). Test:
  `line_geometry_conductors_in_same_space_aborts_solve`.
- **Audit follow-ups (matrix getter / rho / FYprimFreq):** `GetZmatScale`/
  `GetYCScale` (the `rmatrix`/`xmatrix`/`cmatrix` getter) now divide the stored
  *total* matrix by `Len` when a geometry is attached (Pascal Line.pas:261-283) —
  previously echoed the total (off by `Len`); test
  `line_geometry_rmatrix_is_per_unit_length`. A `rho=` without a geometry no longer
  invalidates YPrim (Pascal invalidates only when a geometry is present,
  Line.pas:772-780). The geometry branch no longer writes `FYprimFreq` (Pascal sets
  it only in the per-unit-length path).
- **Audit follow-ups (18101 / singular-matrix abort) — the two deferred items,
  settled live against the oracle and fixed.** (1) **18101:** an illegal `phases=`
  change on a matrix/geometry model now logs `Illegal change of number of phases for
  "Line.<name>"` (Pascal Line.pas:643, `DoSimpleMsg` — so it reverts `nphases` but
  does *not* set `SolutionAbort`; `Redirect_Abort` is not modeled). Probe-confirmed
  exact text/number. Test `line_illegal_phase_change_reverts_and_logs`. (2)
  **Singular series Z (error 183):** `CalcYPrim` no longer silently embeds
  `epsilon·I` and continues — it pushes a `Matrix Inversion Error for Line "…"`
  message and exits, and the Y-build drain sets `solution_abort`. The probe showed
  Pascal `DoErrorMsg` sets `SolutionAbort := True` *unconditionally*
  (DSSGlobals.pas:265), so the solve aborts in **both** `EARLY_ABORT` modes (oracle
  default = `True`, DSSGlobals.pas:781) and `BuildYMatrix` Exits before adding any
  primitive — the embed was dead weight (NOT_PORTED). `calc_yprim` now does Pascal
  `ClearYPrim` (Line.pas:1170) up front, so both abort paths leave the element
  contributing nothing. Test `line_singular_matrix_aborts_solve`.
- Oracle-pinned: 3 inline `geometry_tests` drive a `Line` through the property
  engine + a `build_overhead_3` geometry and assert `Z`/`Yc` == the geometry's
  total matrices entry-by-entry, anchored to the `deri_full_3cond` diagonal, plus
  the Zinv Kron embed and `Yc/2` shunt (`geometry_path_builds_oracle_z_and_yc`);
  the length/units forward (`…_length_units_scale_the_total_z`: 2 km = 1 m ×
  1000 × 2); and `sym_scalar_detaches_geometry` (an `r1=` after `geometry=` runs
  `KillGeometrySpecified`). + 1 exec test `line_geometry_specified_resolves_and_solves`
  (full parse → `LineGeometry` foreign-class resolve → solve; `r1` hidden = `----`).
  dss-core lib **372 → 376** (+ 4 audit follow-up tests → **380**). Full
  three-command gate green.
**WP7.1 step 3b — Line `spacing=`/`wires=`/`cncables=`/`tscables=` Carson path
(`FetchLineSpacing`/`SetWires`/`LoadSpacingAndWires`/`FMakeZFromSpacing`) — ✅
done, gate-green, committed (`c2a81d0` + audit follow-ups `5eda50a`/`4aeda24`).**
- `line/mod.rs`: un-`NOT_PORTED` the four props — `spacing` (scalar
  `object_ref_class("LineSpacing")`), `wires`/`cncables`/`tscables` (array
  `object_ref_array` over WireData/CNData/TSData). New `Line` fields
  `line_spacing_obj: Option<LineSpacingObj>`, `line_wire_data:
  Vec<Option<Box<dyn DssObject>>>` (Pascal `LineWireData`), `fphase_choice:
  ConductorChoice` (`FPhaseChoice`), `got_ratings_after_spacing_conds`. The
  trait-object `Vec` forces a **manual `Clone`/`Debug`** (the `LineGeometryObj`
  precedent — the derive is gone).
- Ported verbatim: **`FetchLineSpacing`** (Line.pas:1853 — drop linecode/geometry,
  `NPhases := spacing.NPhases`, allocate the empty `NWires`-slot wire array);
  **`SetWires`** (Line.pas:803 — overhead `istart=1` when `FPhaseChoice=Unknown`,
  else bare neutrals at `istart=NPhases+1`; count-validate `(NWires-istart+1)`;
  seed `NormAmps`/`EmergAmps`/ratings from the wires); **`KillSpacingSpecified`**
  (Line.pas:2042) wired into `FetchLineCode`/`FetchGeometryCode` and the
  sym/matrix/switch side effects; **`SpacingSpecified`**; the
  `spacing`/`wires`/`cncables`/`tscables` `PropertySideEffects` (the three Pascal
  case blocks — cable forms pick the model, the common block switches off the sym
  model + clears the superseded marks, ratings-after-conds latch). **`LoadSpacingAndWires`**
  (LineGeometry.pas) added on `LineGeometryObj`: builds a throwaway geometry from
  the spacing + conductors, picks OH/CN/TS from the wire kinds, runs the Carson
  calc. **`FMakeZFromSpacing`** (Line.pas:1964): the `pGeo` temp-geometry path →
  **total** `Z`/`Yc` (length+units folded in), so `CalcYPrim` reuses the geometry
  branch (`total_z_path = geometry || spacing_specified`); the `rmatrix`/`cmatrix`
  per-unit-length getters and the `Yc/2` shunt likewise.
- **Routing decision (probed):** the `wires=` prop runs the `SetWires` state
  machine (with the buried-neutral `istart`); `cncables=`/`tscables=` use Pascal's
  *generic* `DSSObjectReferenceArrayProperty` fill (`set_cables`, fill from
  conductor 1, no `istart`), with the side effect setting `FPhaseChoice`. Routing
  the cables through `SetWires` would break the buried-neutral case
  (`cncables=[3] wires=[1 bare neutral]`, NWires=4/NPhases=3) — confirmed against
  the oracle.
- **Gate:** `probe_line_spacing_phase7.py` builds each form **both** ways in the
  oracle (`geometry=` vs `spacing=`+`wires=`/`cncables=`) and confirms they agree
  **exactly** (maxdiff 0) for overhead, CN, and CN+bare-neutral. 5 inline tests
  pin the spacing `Z`/`Yc` to the **same `deri_full_3cond`/CN oracle anchors** the
  step-3a + line_geometry tests use (overhead `Z00`+`Yc00`, CN `Z00`, buried-
  neutral `Z00`) and assert entry-by-entry equality with the equivalent geometry:
  `spacing_wires_match_geometry_and_oracle`, `spacing_cncables_match_geometry`,
  `spacing_buried_neutral_via_cncables_then_wires` (validates the `istart` offset
  + 4→3 Kron reduce), `set_wires_wrong_count_errors` (18102 count error),
  `sym_scalar_detaches_spacing` (`KillSpacingSpecified`). dss-core lib **380 →
  385**. Full three-command gate green.
- **Also fixed (faithfulness):** `FetchLineCode` now calls
  `KillSpacing`/`KillGeometry` (Line.pas:590-591 tail — a `linecode=` supersedes a
  prior spacing/geometry; was a latent step-3a gap).
- **Audit-code follow-up (`/audit-code` step 3b):** one Major finding fixed —
  `cncables=`/`tscables=` *before* any `spacing=` left `LineWireData` unallocated
  and **silently no-op'd into the sym model**; the oracle raises error 402 (`No
  objects are expected!`) at that generic-array fill (probe-confirmed), so
  `set_cables` now reproduces 402 instead of swallowing it (`set_wires` already had
  the parallel 18102 guard). Test `cncables_without_spacing_errors`. dss-core lib
  **385 → 386**. The audit also *settled four divergence risks against the live
  oracle and found the port already faithful* (no change): cncables/tscables use a
  partial fill from conductor 1 with **no** count-#406 check (a NIL slot aborts at
  solve with the exact "WireData is not correctly initialized" text — matched);
  the spacing path uses the **line's** `FEarthModel`, not the ambient
  `ActiveEarthModel` (CN mixed-earth-model probe: global Carson + line Deri ⇒ Deri
  Z, identical to the geometry path); `AllowAllConductors` is JSON-only; and
  `GetZmatScale`/`GetYCScale` include `SpacingSpecified` (Line.pas:261-283).
- **Audit-tests follow-up (`/audit-tests` step 3b):** two coverage gaps closed.
  (1) The **tape-shield form had zero executable coverage** (no test wrote
  `tscables=`, so the `TapeShield`/`TsDataObj` arms were dead — the same gap the
  step-2c-i test audit caught for LineGeometry): added inline
  `spacing_tscables_match_geometry`, oracle-pinned (`Z00 = 4.675825330004e-04`,
  probed). (2) The inline `spacing_*` tests call `set_object_ref_array` **directly**,
  bypassing the executive's `wires=[…]` array parse and never querying the
  `spacing`/`wires` dumps (the new `get_string`/`get_object_ref_names` accessors):
  added the full-pipeline exec test `line_spacing_specified_resolves_and_solves`
  (`exec/tests/line_fetch.rs` — `New Line … spacing=s wires=[w w w]` → solve, with
  `?spacing`="s" / `?wires`="[w, w, w]" round-trip), the step-3a
  `line_geometry_specified_resolves_and_solves` parallel. dss-core lib **386 →
  388**.
- **Audit Question settled (cncables/tscables count > NWires):** probed — the
  oracle fills the `NWires` slots and **silently drops the extras** (`cncables=[4]`
  on a 3-wire spacing solves identically to `cncables=[3]`); `set_cables` already
  does exactly this (`if k < NWires`), so the port was already faithful. Pinned by
  `cncables_excess_count_drops_extras`. dss-core lib **388 → 389**. No open
  step-3b tails remain.
- **Next — step 4:** the geometry/spacing corpus feeder migration
  (`unsupported_class={LineSpacing,WireData,LineGeometry,…}` → `solvable_now`) +
  targeted golden `phase7/line_geometry*.json`.

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

Grep `rg "TODO\(compat\)"` for the full marker list (26 sites). Notable:
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
(Phase 7); RegControl/CapControl `Sample`/`DoPendingAction` **wired into the
control loop (WP5.7)**; RegControl/ControlQueue debug-trace files (flag
stored, no file — port with Monitors, Phase 6+); `MakePosSequence` everywhere
(Phase 6+); `BusCoords` **ported (WP5.8)**; Monitors/EnergyMeters
`sample_all`/`EndOfTimeStepCleanup` are no-op hook stubs at the SolveDaily/
Yearly/Duty call sites (Phase 6); Newton algorithm, harmonics/dynamics/
faultstudy/Monte-Carlo/load-duration/`SolveGeneralTime` solve modes (Phase 7);
`Show`/`Export`/`Dump`/`Select`/... executive verbs record "not ported".

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
  `StorageController` behavior — the WP6.8 StorageController is a parse-only
  skeleton (empty fleet → 37201); `solution/meters/zones/build.rs::is_zone_pce`
  carries a `TODO(WP7)` to add PVSystem/Storage to the zone allow-list once they
  exist.
- **Protection** `Relay`/`Recloser`/`Fuse`/`SwtControl`/`Fault` — until one sets
  `Flg.HasOCPDevice`, `RelCalc` aborts with #52902 (oracle-faithful) and the
  ported SAIFI/SAIDI/section math below the abort stays dormant
  (`GetOCPDeviceType` inlined to 0, `TODO(WP7)`).
- **Line constants** `WireData/CNData/TSData/CableData/LineSpacing/LineGeometry`
  + Carson — Line's `geometry`/`spacing`/`wires`/`cncables`/`tscables` are
  `NOT_PORTED` (round-trip empty only).
- **Dynamics & harmonics** (Generator/Storage `DoDynamicMode`/`DoHarmonicMode`,
  state vars beyond names/count) + `MakePosSequence` everywhere; Monitor modes
  3/4/7/8/10/12 build their header but defer the sample body; Transformer GIC
  (<0.51 Hz).
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
