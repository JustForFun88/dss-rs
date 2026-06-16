# dss-rs — Project Status & Session Handoff

> **Purpose of this file:** a living snapshot so a fresh session can resume
> without re-deriving context. It records *what is done*, *what was decided*,
> and **why**. It is **not** authoritative for the plan itself — that is
> `PORTING_PLAN.md` (roadmap + binding decisions) and `CLAUDE.md` (conventions
> + the green-gate rule). Read those two first; then read this for the current
> frontier.

Last updated: 2026-06-14, **Phase 6 COMPLETE (WP6.1–WP6.10) and MERGED to
`main`** (`--no-ff` merge `b98223a`, gate green at merge; `main` not pushed to
origin). The branch `phase-6-meters-topology` carried WP6.1–WP6.9 through
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
(`LineSpacing`) **and step 2c-i done** (the `LineGeometry` object + edit state
machine + props/`MakeLike`; the matrix wiring `UpdateLineGeometryData` is step
2c-ii), all oracle-pinned and gate-green — see §1e; **WP7.1 step 2c-ii next**
(`LineGeometry` `CalcMatrices` → the Carson engine).
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
| 7 | Extended elements: DER, protection, line constants, harmonics, dynamics | 🚧 in progress — `PHASE7_PLAN.md` written (WP7.1–WP7.10); branch `phase-7-extended-elements`; WP7.1 in progress (step 2c-ii `LineGeometry` matrices next) |

### Gate state (all green)
```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace      # dss-core lib 358, golden_feeders 1,
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

Execution plan: **`PHASE7_PLAN.md`** (WP7.1–WP7.10). Per-WP cadence: small step
→ full gate → update this file → stop for confirmation.

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
  math engine beside the other `support/` helpers; reuses `support/cmatrix.rs`
  (Kron, invert), `support/line_units.rs`, `support/mathutil.rs`
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
- `src/elements/general/conductor_data.rs` (new) — port of Pascal
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
- `src/elements/general/line_spacing.rs` (new) — port of Pascal
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
- **Audit-tests follow-up (`/audit-tests`, uncommitted):** rounded the units
  golden out to **all 9 `LineUnits` ordinals** — added `linespacing_units_{in,cm,mm}`
  (the only Minor finding; the per-class plumbing was already covered by 6
  ordinals + the full mapping by `dss_enum.rs`). Regenerated with the pinned
  oracle; only `linespacing.json` changed (11 → 14). The two Rust-only invariant
  tests (`nconds_grow_*`, negative-`nconds`) needed no change — documented
  divergences with no oracle to pin. Gate green.

**WP7.1 step 2c-i — `LineGeometry` (`TLineGeometryObj`) object + edit state
machine — ✅ done, gate-green, committed.**
- `src/elements/general/line_geometry.rs` (new) — port of Pascal
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
- **Deferred to step 2c-ii:** `UpdateLineGeometryData(f)`/`CalcMatrices` driving
  the `support::line_constants` Carson engine to cache `Zmatrix`/`YCmatrix`/
  `Rho` (the object tracks `data_changed` staleness + the engine `fline_kind`
  for it). `LineSpacing` has no `DataChanged` flag, so the geometry tracks its
  own staleness. Then step 3 un-`NOT_PORTED` Line's geometry fetch path
  (`RecalcElementData` `CalcMatrices(BaseFrequency)`), step 4 the corpus feeder
  + targeted golden gate.

---

## 2. What Phase 3 built (file-by-file map — still the architectural reference)

### Circuit model (`src/circuit/`)
- `bus.rs` — slim `Bus` (`TDSSBus`): `nodes`/`ref_no` allocation lists,
  `kv_base`, coords, `allocate_bus_state`.
- `circuit.rs` — `Circuit` (`TDSSCircuit` subset): `add_ckt_element` (device
  list + per-kind `Vec<ElemRef>` lists + 1-based handle), `add_bus`
  (find-or-create + the "Caution: Magic" node_buffer→global-ref rewrite),
  `process_bus_defs`, `reprocess_bus_defs`, `set_bus_name_redefined`,
  `node_name(i)` (= oracle `YNodeOrder` format), `losses` (now skips shunt).
- `terminal.rs` — `Terminal` (`TPowerTerminal`).

### Element base (`src/elements/`)
- `ckt.rs` — `CktElementData` (`TDSSCktElement` fields + realloc semantics,
  `set_bus`/`get_bus`, `compute_vterminal`, `do_yprim_calcs` open-conductor
  Kron, signal flags `signal_bus_name_redefined`/`yprim_invalid`).
- `traits.rs` — `CktElement` trait (+ `is_shunt` since Phase 4), `ElemRef`,
  `ElemStore`, `SysCtx`, `InjCtx`.
- `pc/vsource.rs`, `pc/load.rs` — full Phase 3 ports (all 8 load models,
  compensation currents).
- `pd/line.rs` — sym + matrix paths + LineCode fetch (Phase 4).
- `pd/{transformer,capacitor,reactor,winding}.rs`,
  `general/{line_code,xfmr_code,growth_shape}.rs`,
  `control/{control_elem,reg_control,cap_control}.rs` — Phase 4 ([record](docs/phase-records/phase-4.md)).

### Solution (`src/solution/`)
- `solution.rs` — `solve` → `solve_snap` (control loop; `controlmode=off` ⇒
  `control_actions_done` immediately) → `solve_circuit` → `do_pflow_solution`
  → `do_normal_solution`; `converged`, `solve_system`, `set_voltage_bases`.
- `ymatrix.rs` — `build_y_matrix`: reprocess buses when redefined → recalc
  invalid Yprims → stamp enabled elements (skips `yprim: None`, i.e. control
  elements) → `allocate_vi` → `initialize_node_vbase`.

### Executive (`src/exec/mod.rs`)
- Full Pascal command/option name lists; `command()` = `ProcessCommand` (incl.
  error-301 circuit gate and the property-reference fallback);
  `New circuit.x` → default Vsource; `AddObject`; `Set`/`Get`; `Solve`;
  `Redirect`/`Compile` (block-comment semantics, dir following);
  `edit_active` (split-borrow `ForeignClasses` view, deferred-error drain,
  signal-flag propagation, **deferred `RefAction` application** since Phase 4);
  `snapshot_elements`/`total_power`/`losses` public gate API (Phase 4).

### Property engine (`src/obj/`)
- `props.rs` — `PropType` (Double/Integer/Boolean/String/MakeLike/arrays/
  matrices/Bus/Complex/Enabled/ObjectRef/struct-array family), `PropFlags`
  (incl. `NOT_PORTED`, `CONDITIONAL_VALUE`, `SCALED_BY_FUNCTION`), parse/dump
  paths, `ForeignClassesView` (+ `find_full`), **`define_properties!`**.
- `base.rs` — `DssObjData` (PrpSequence, deferred errors), `DssObject` trait
  (typed accessors, `set_object_ref`, struct-array hooks, `side_effects`,
  `end_edit`, `make_like`, **`take_ref_actions`/`apply_ref_action`**).
- `dss_enum.rs` — `TDSSEnum` + registry (17 enums).

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
- Line `geometry`/`spacing`/`wires`/`cncables`/`tscables` — Phase 7
  (line constants: LineGeometry/WireData/LineSpacing/CN/TS, PORTING_PLAN §Phase 7
  sub-block 1). Not in Phase 6 scope.
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

## 7. Current frontier — Phase 7 in progress, WP7.1 step 2c (`LineGeometry`) next

Phase 6 (`PHASE6_PLAN.md`, WP6.1–WP6.10) is **complete, gate-green, and MERGED
to `main`** (`--no-ff` merge `b98223a`, gate green at merge; `main` not pushed to
origin). The work landed on branch `phase-6-meters-topology` (WP6.10 phase-exit
`207b9cb`, the post-exit live-gate deepening + audit hardening through `cc6d2e2`;
everything earlier through `d1cc68c` + the corpus infra `19a5493`/`593420f`).
**Phase 7 is now in progress:** `PHASE7_PLAN.md` is written (WP7.1–WP7.10) and
the branch `phase-7-extended-elements` is cut from `main`. **WP7.1 step 1 (the
Carson line-constants engine, `support/line_constants/`) and step 2a (the
conductor catalog `WireData`/`CNData`/`TSData`, `conductor_data.rs`) and step 2b
(`LineSpacing`, `line_spacing.rs`) are done, gate-green and committed — see
§1e**; **WP7.1 step 2c (`LineGeometry`) is next.** Execute
the rest per the plan (PORTING_PLAN.md §Phase 7, the largest phase ~18%, six
independently-gated sub-blocks ordered risk-ascending: line constants →
protection → DER → harmonics → dynamics → faultstudy/AutoAdd-modes/`Feeder` — see
PHASE7_PLAN §0).

**What Phase 7 inherits / must finish (deferrals Phase 6 left explicit):**
- **DER classes** `Storage`/`PVSystem` (+ `InvControl`/`ExpControl`) and the real
  `StorageController` behavior — the WP6.8 StorageController is a parse-only
  skeleton (empty fleet → 37201); `solution/meters.rs::is_zone_pce` carries a
  `TODO(WP7)` to add PVSystem/Storage to the zone allow-list once they exist.
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
have real bodies; the zone-build dispatcher (`solution/meters.rs`) fires from
`build_y_matrix` after bus reprocessing; `TakeSample`/`Integrate` + the
reliability fault-rate sweep are ported. Still Phase 8: the `SystemMeter`
register core and all demand-interval/phase-voltage/`Show`/`Export` files.
