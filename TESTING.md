# How dss-rs is tested

The single entry point for the test infrastructure. `CLAUDE.md` owns the
**gate rule** (what must be green to commit); this file explains the **layers**,
the **knobs**, and the **procedures** (regenerate goldens, add a corpus case,
triage a divergence into the ledger, re-vendor the r4133 binary).

## The mandatory gate

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --workspace --all-targets --features dss-core/oracle-parity -- -D warnings
cargo test --workspace
cargo test --workspace --features dss-core/oracle-parity
```

Since DE_PASCALIZE **Stage F** the engine ships in two builds, so the gate runs
in **two lanes** — see [The two lanes](#the-two-lanes-stage-f) below for what
each one asserts, and [the differential gate](#the-paritydefault-differential-gate)
for the job that compares them (run on demand, not per commit).

`cargo test` runs everything below. The unified corpus gate compares the Rust
engine live against **two oracles**, and both are mandatory prerequisites:

- **`capi_v0145`** — the pinned dss-python (`tools/golden/PIN.txt`, 0.15.7 /
  dss_capi 0.14.5 — the exact vendored Pascal source), served by persistent
  `tools/oracle/oracle_server.py` worker processes on the system `python`
  (`DSS_ORACLE_PYTHON` overrides). The gate **fails (not skips)** without it;
  every worker's ping re-verifies the pin.
- **`r4133`** — the official EPRI `OpenDSSDirect.dll` release 11.0.0.1 (SVN
  r4133), **git-tracked** at `tools/opendss/bin/r4133/` (no download, no venv),
  driven by the in-house `crates/dss-epri` bridge worker (`epri-worker`, built
  by `cargo test` itself; `DSS_EPRI_WORKER` overrides the binary path). The
  bridge is `#[cfg(windows)]`, so the mandatory gate needs Windows (CI runs
  `windows-latest` only, documented in `.github/workflows/ci.yml`).

No test that **participates** in the gate can green on zero matches or silently
skip: the corpus/family gates assert a non-empty, count-locked population
(`population_lock.rs`, `*_manifest_is_complete`, `solvable_now_has_multistep_depth`),
a `DSS_GATE_ONLY` filter that matches nothing panics instead of greening a 0/0
run, and every divergence-ledger entry must be *hit* every run (fail-on-stale,
below). The gate carries exactly **two** ignored items, both non-gating and
deliberate: one `#[ignore]`d diagnostic (`adiakoptics::ckt24_graph_diagnostic`
— a `.graph` inventory probe, run with `--ignored`, pending the WP-AD.5 driver)
and one illustrative ` ```ignore ` doctest (the `define_properties!` macro-DSL
snippet in `obj/props/mod.rs`, which cannot compile standalone). The `DSS_LIVE_*`
/ `DSS_EXPENSIVE_TESTS` / `DSS_AD_*` env knobs below are opt-in **diagnostics**
outside the gate — they print `SKIPPED` when unset and never gate a commit.

The dev/test profile carries `opt-level = 3` overrides for the engine crates
(`dss-core`/`dss-sparse`/`dss-parser`/`dss-usermodel`/`dss-epri`) and all
dependencies (workspace `Cargo.toml`): the live gate runs yearly/8500-node decks
through the engine, and unoptimized codegen makes a single yearly EPRI-feeder
deck cost ~14 min (~×10). Test binaries themselves stay at opt 0; float results
are opt-level-independent (no fast-math in Rust — pinned empirically by the
corpus-wide exact-iteration-count contract). The safety knobs are orthogonal to
opt-level and stay on: slice bounds checks are never removed at any opt-level,
and `overflow-checks`/`debug-assertions` are pinned `true` explicitly in the
overrides — the reason the gate uses this instead of `--release` (which sets
overflow-checks=false).

### The two lanes (Stage F)

`DE_PASCALIZE_PLAN.md` Part IV.2 split the engine into two builds of the same
source. The difference is confined to three `compat` modules
(`crates/{dss-core,dss-parser,dss-sparse}/src/compat.rs`), each of which selects
between two always-compiled sibling implementations; `oracle_parity_cfg_gate.rs`
fails if the cfg string appears anywhere else.

| lane | build | what it asserts |
|---|---|---|
| **parity** | `--features dss-core/oracle-parity` | the bit-compat engine: byte goldens, checkpoint Y, corpus tier floors, **exact** iteration counts and discrete state, every upstream quirk reproduced. This lane is the permanent 1:1 record and **never re-baselines**. |
| **default** | no features | the idiomatic product: the F.3 upstream-bug fixes are live, report text is rendered natively (F.4). Continuous quantities keep the **same** oracle floors, discrete state stays exact, iteration counts get a documented ±1 band, and each deliberate divergence is excluded field-by-field and pinned by its own expected-value test. |

The whole lane policy lives in **one** file, `crates/dss-core/tests/harness/lane.rs`
(`PARITY`, `ITER_SLACK`, `compare_report`, `expected_eventlog`, the
field-scoped exclusion lists) — no golden driver reads the cfg directly, and
the module's own unit tests assert the *split itself* (a rendering-only
difference must pass in the default lane and fail in the parity lane, in
whichever lane the suite runs). Stage F introduces **no** tolerance: the
default-lane report policy is `rel = abs = 0`, so only the spelling of a number
may move, never its value.

Both lanes must be green before any commit. Everything else in this document —
oracles, goldens, ledger, knobs — is identical in the two lanes unless the table
above says otherwise.

### Precision-compat rows still split by lane

The split is being taken apart again. Since the 2026-08-02 policy no upstream bug
is reproduced in **any** lane, so `GOLDEN_REBASE_PLAN.md` WP-G2 tears down the
bug-reproducing kernels and WP-G4 the FPC print emulation. Five rows survive both
and are the only lane split expected to outlive this plan: `compat::PI`,
`compat::round_f64`, `compat::round_i32` (`crates/dss-parser/src/compat.rs`),
`compat::kv_base_search_scale` and `compat::profile_ll_pu_divisor`
(`crates/dss-core/src/compat.rs`) — truncated upstream constants and FPC `Round`
semantics *in computation*, owned by the UPGRADE line. None of them is a bug
reproduction or a rendering row.

| row | parity kernel | default kernel | pinned at |
|---|---|---|---|
| `compat::PI` | the RPN calculator's degree conversions use upstream's shortened `3.14159265359` (`Parser/RPN.pas:69-70`, r4133 `:247`), 2.07e-13 above π | `std::f64::consts::PI` | `dss_parser::parser::tests::rpn_degree_trig_is_the_lane_kernel` |
| `compat::round_i32` | FPC `Round` into an `Integer`: ties-to-even, and out-of-range/non-finite inputs wrap the x87 integer-indefinite sentinel (`inf` → `0`). Both oracles: r4133 `General/GrowthShape.pas:224` assigns `Round(…)` into the `pIntegerArray` `Year` | ties-to-even, then a saturating cast | `dss_parser::parser::tests::make_integer_out_of_range_is_the_lane_kernel` |
| `compat::round_f64` | the same `Round` written back into a `Double`, so `1e20` becomes `-9.22337203685478e18`. **Capi-only mechanism**: its one call site is the pinned oracle's `TPropertyFlag.ApplyRound` array path (`General/DSSObjectHelper.pas`), which r4133 does not have at all — there `GrowthShape.Year` is a `pIntegerArray` (`:82`), i.e. the `round_i32` row above | ties-to-even in place, magnitude preserved | `exec::tests::compat_quirks::apply_round_out_of_range_is_the_lane_kernel` |
| `compat::kv_base_search_scale` | `CalcVoltageBases` scales the solved L-N magnitude by the truncated `0.001732` before the legal-base argmin (`Common/Solution.pas:1103`, r4133 `:2541`) | `SQRT3 / 1000`, the constant the same statement names one operator later | `solution::solution::dispatch::tests` (a constructed tie, then the whole command) |
| `compat::profile_ll_pu_divisor` | `Export Profile`'s three line-to-line arms divide by the truncated `1732.0` (`Common/ExportResults.pas:3207/3231/3256`, r4133 `:3455/3471/3488`) | `1000·√3`, matching the eight line-to-neutral divisions' exact `1000.0` | `exec::tests::compat_quirks::export_profile_ll_pu_is_the_lane_kernel` |

The middle column is the pinned dss_capi 0.14.5 kernel, which is what the parity
lane reproduces; every row but `compat::round_f64` is the same statement in the
r4133 behavioural authority, cited inline.

Four rails keep the operational documents honest as the teardown proceeds, all
in `crates/dss-core/tests/oracle_parity_cfg_gate.rs`: every `compat::` alias any
operational document names must still be declared by a compat module (so the
sentences above cannot survive their rows), and `TORN_DOWN_ROWS` records every
row that *left* the split — its census decrement, the evidence in the tree, and
the expected-value pin that became unconditional. Teardown commits mark their
sites with the greppable comments `// LANE-EXCLUSION(<row>): <why>` at an
exclusion made unconditional and `// EXPECTED-VALUE-PIN(<row>): <why>` at the
pin. Both are checked against the register in either direction: a marker naming
an unregistered row fails, a registered row whose pin file lost its marker
fails, and so does one whose evidence declares a harness exclusion that carries
no marker. The register also re-reads what a teardown is *for* — the recorded
pin must be a declared `#[test]` whose body no longer branches on the lane,
because once the row leaves the census nothing else looks at that test.

The third and fourth rails are about *citations* rather than rows, and run in
both directions. `operational_docs_line_citations_point_at_the_line_they_name`
(§RP5.1's settlement) covers this file and `tests/TOLERANCE_NOTES.md` wherever
they name a Rust line: every `<file>.rs:<line>` — and every bare `` `:<line>` ``
continuation — must name exactly one file, land on a line that exists and is
not blank, and sit within ±3 lines of something the sentence itself backticks;
since §RP5.2's settlement an *unanchored* citation **fails** too, so nothing is
checked for existence only. Since G1.7's audit settlement a `:A-B` **range** is
read whole: the end must be a real line of the same file and not precede the
start, and the anchor must sit inside the range rather than near its start — a
range that has drifted off the block it names now reds (a range still
overlapping that block does not, so a re-point still needs reading). The fourth,
`rust_comments_citing_a_record_line_point_at_the_passage_they_name`, runs the
other way: a Rust comment citing `<record>.md:LO-HI` must land inside that
record, on a passage it quotes, inside the `§`section it names, or sharing a
backticked symbol with it. Floors keep both walks from going vacuous.

### The parity↔default differential gate

The two lanes are two *builds*, so no `#[test]` can compare them. That
comparison is a **scripted job**:

```
pwsh -File tools/lanes/lane_diff.ps1            # build both lanes, dump, diff
pwsh -File tools/lanes/lane_diff.ps1 -SkipDump  # re-diff existing dumps
```

It builds `crates/dss-core/examples/lane_dump.rs` once per lane (each into its
own target dir under `target/lanes/`, so re-runs do not thrash the other lane's
cache), walks **all 526 manifest cases** on each engine — solving the 522 that
are not abort-by-design — and writes one record per compared quantity: engine
error *count* (not the message text; the corpus gate reconciles that), per-step
convergence flag and iteration count, every node voltage, every element's
terminal currents, powers and losses, and the assembled system Y. That list is
the whole dump: meter registers, monitor channels, the event log, the control
queue, property probes and report text are **not** in it — the corpus gate
compares those live against the oracles, in both lanes. The diff compares record *keys* exactly
and in order (a renamed, reordered, dropped or added record fails structurally)
and the values against the **tightest** calibrated oracle tier
(`tol_for("micro")`: `|Δ| ≤ 1e-6 + 1e-9·|parity|`) plus the same ±1 iteration
band the default lane uses against the oracle.

Why it is the strongest default-lane test. The parity lane is byte-exact
against the committed goldens and, on the live oracles, compared at the
calibrated floors of `tests/TOLERANCE_NOTES.md` (with its own pinned entries in
`tests/corpus/ledger.json`) — it is *not* bitwise equal to the oracle, since the
faer-vs-KLU last-ulp floors `CLAUDE.md` documents are real in both lanes. So the
chain is the triangle inequality
`|default − oracle| ≤ |default − parity| + |parity − oracle|`, and what carries
the transitive proof is the **measured** left term rather than an assumed
premise: the landing run came back `max |Δ| = 0` exactly on every gated kind,
which makes the default lane bit-identical to the parity lane and so gives it
precisely the parity lane's oracle standing. While that holds, the job is also
sharper than the oracle comparison — the floors are 1e-6-class, the lanes differ
only by kernel ulps — so a default-lane kernel regression hiding inside a tier
floor passes the corpus gate and fails here. The moment `|Δ|` stops being zero,
the bound on `|default − oracle|` is this job's bound **plus** the case's tier,
not this job's bound alone.

The job also checks itself: the two dumps must declare *different* lanes
(`{default, parity}`), a non-finite value on either side is a hard failure
rather than a comparison that silently evaluates false, and every
`DOCUMENTED_DIVERGENCES` entry must still be hit.

It is **not** part of `cargo test`: it costs two release builds and ~3 minutes
of solving, and it writes ~215 MB per lane into `target/lanes/`. Run it when a
`compat` kernel, a lane alias or the solver changes — and expect it in the
`MULTITHREADING` M3c and `RESONANCE` WP-R1 rungs, which are the two planned
changes that will make the lanes genuinely diverge.

The Stage F landing measurement (2026-07-31, 520 cases / 3 219 862 records /
~4.8 M compared values) is recorded in `STATUS.md`: every gated kind
**bit-identical** (`max |Δ| = 0` on `v`, `cur`, `pow`, `loss`, `y`, `errs`,
`conv`, `iter`), the only measurable divergence being the then-deliberate Newton
`Powers`/`Losses` row — which `GOLDEN_REBASE_PLAN.md` G2.3 tore down, leaving
`DOCUMENTED_DIVERGENCES` **empty**: both lanes now recompute at the converged
`NodeV`, so every record in the dump is held to the ordinary bound. The list is **hand**-mirrored
from `harness::lane`'s field-scoped exclusions — an example cannot import the
test harness, and nothing checks the two lists against each other, so keep them
in step by hand. What *is* checked is that every entry still fires: a stale one
fails the job rather than quietly exempting a field. A divergence there is
measured and printed, never silently skipped.

Corpus hygiene is part of the job: decks write their reports next to
themselves, so the script deletes the untracked artifacts it produced and
`git restore`s the three vendored files some decks overwrite
(`Test/LineConstantsCode.DSS`, the two `IEEE_519_Mon_mpcc_1.csv`) — by exact
path, never a wide `git clean`.

## The layers

| layer | what it checks | where | oracle |
|---|---|---|---|
| **unit tests** | per-module algorithms, Pascal-cited numerics | `crates/*/src/**` (`#[cfg(test)]`, `exec/tests/`) | pins inline in code |
| **golden gate** | committed input→output pins, replayed offline | `tests/golden/` + `crates/dss-core/tests/golden_*.rs` + `tests/harness/` | pinned dss-python, **manual** regen only |
| **unified corpus gate** | full assembled model (Y / V / currents / powers / losses / YPrims / injection / discrete state / monitors / meters / probes / eventlog / …), per step, live, on the channel(s) each case's `engines` field names, partitioned by the divergence ledger | `corpus_gate.rs` + `tests/corpus_gate/` submodules + `tools/oracle/oracle_server.py` + `crates/dss-epri` | pinned dss-python (`capi_v0145`) **and** EPRI r4133 DLL (`r4133`) — both gating |
| **corpus hygiene** | no silent omission: every `.dss` classified, every family a dir↔manifest bijection; no silent **shrink** of the gated population; the ledger structurally valid | `corpus_manifest.rs`, `population_lock.rs`, `*_manifest_is_complete`, `ledger_is_structurally_valid` | none (structural) |

The former opt-in EPRI report channel (AltDSS Oddie bridge, separate venv,
r3723/r4088 binaries, `known_diffs.json`, `DSS_LIVE_OPENDSS*`) was retired by
`UNIFIED_GATE_PLAN.md` Phases D/E: the r4133 engine is now a first-class
**gating** channel through the in-house Rust bridge, and divergences are pinned
in the gating ledger instead of a report-only catalog.

### Golden families (`tests/golden/` ↔ `tools/golden/gen_*.py` ↔ `golden_*.rs`)

Command-replay goldens are named for **what they cover** (porting-era `phaseN`
names were retired 2026-07-07 — see the STATUS.md rename map):

| golden dir | generator | Rust gate | covers |
|---|---|---|---|
| `feeders_controlsoff/` + `.json` | `gen_feeders_controlsoff.py` | `golden_feeders.rs` | IEEE13/37/123 snapshot, controls off |
| `timeseries_controls/` | `gen_timeseries_controls.py` | `golden_timeseries_controls.rs` | daily/duty loadshapes, Reg/Cap control, event log |
| `metering_monitors/` | `gen_metering_monitors.py` | `golden_metering_monitors.rs` | EnergyMeter registers/zones, Monitor channels, generators |
| `line_constants/` | `gen_der_lines_harmonics.py` | `golden_line_constants.rs` | Carson Z/Yc via geometry/spacing/cable |
| `der_controls/` | ″ (same generator) | `golden_der_controls.rs` | PVSystem, Storage, StorageController, InvControl, ExpControl |
| `harmonics/` | ″ (same generator) | `golden_harmonics.rs` | harmonic frequency sweep |
| `protection/` | `gen_protection.py` | `golden_protection.rs` | Recloser/Relay/Fuse/SwtControl trip-reclose |
| `reports/` | `gen_reports.py` | `golden_reports.rs` | Export/Show/Dump/Save byte-exact output (decks in `tools/golden/report_decks/`) |
| `checkpoints/` | `gen_checkpoints.py` | `golden_checkpoints.rs` | per-step assembled model (Y, YPrim, injection, discrete state) |
| `plot_callback/` | `gen_plot_callback.py` | `golden_plot_callback.rs` | `Plot`/`Visualize` callback `plotParams` JSON payload (captured via the oracle's `DSS_RegisterPlotCallback`; structural compare, numbers by tolerance) |
| `props/` | `gen_props.py` | `props_roundtrip.rs` | per-class property round-trip |
| `cim/` | `gen_cim.py` | `golden_cim.rs` | CIM/XML export round-trip |
| `ncim/` | `gen_ncim_reports.py` | `ncim_reports.rs` | NCIM Jacobian/deltaF/deltaZ/PV2PQ reports |
| `inc_matrix/` | `gen_inc_matrix.py` | `inc_matrix_reports.rs` | incidence/BusLevel/Laplacian reports |
| `flicker/` | `gen_flicker.py` | `golden_flicker.rs` | Pst flicker meter |
| `pstcalc/` | `gen_pstcalc.py` | `golden_pstcalc.rs` | IEC Pst calculator |
| `json/` | `gen_json.py`, `gen_schema.py` | `golden_json.rs`, `golden_schema.rs` | AltDSS JSON export byte goldens + the AltDSS JSON-schema golden |
| `json_import/` | `gen_json_import.py` | `golden_json_import.rs` | `Circuit_FromJSON` round-trip (import → re-export == oracle J1) |
| `adiakoptics/` | in-test (`DSS_REGEN_AD_GOLDEN=1`) | `adiakoptics.rs` | A-Diakoptics init/solve matrices (ZLL/ZCC/Y4) |
| `ieee*.json`, `slice`, `allocation`, `autoadd_reduce`, `gendispatcher`, `ieee8500`, `reliability`, `parser` | `generate.py` / `gen_<name>.py` | `golden_smoke.rs`, `golden_feeders_controls.rs`, `golden_slice.rs`, … | named feeders / features |

**Frozen historical generator arms.** A few generators (or arms of them)
targeted the retired Oddie/EPRI-python engines and can no longer run:
`gen_bh_capi015.py`, `gen_regcontrol_capi015.py`, `gen_fuse_r4133.py`
(individual `props/` captures against the deleted Oddie venv), and
`gen_checkpoints.py`'s `DSS_ORACLE_ENGINE=capi015` arm (reads the deleted
`tools/opendss/PIN_OPENDSS.txt`). They are **dead paths kept as provenance**:
the goldens they produced are pinned and frozen, the live `capi` regen arms
are unaffected, and regenerating them would require restoring the non-pinned
environment (Oddie venv, 0.16.0b2 wheels, `PIN_OPENDSS.txt`) from git history.
The former Oddie arms of **`gen_flicker.py`** and **`gen_protection.py`**
(`fuse_blow`/`swt_manual`) are NOT frozen anymore: the EPRI-bridge parity
round re-hosted them on the `epri-worker` bridge (they drive the git-tracked
r4133 DLL natively; scratch regen via `DSS_GOLDEN_OUT` was proven
payload-byte-identical to the committed goldens — see the STATUS "EPRI bridge
parity round" record). The committed `flicker/` golden remains the frozen
r3723 capture; the r4133 regen reproduced it byte-identically (the flicker
payload is revision-stable). The frozen A-Diakoptics trusted baseline
(`crates/dss-core/tests/data/adiakoptics/r3723_ref/`, gated by
`ad_reference.rs`) did lose its harvester — see
`tools/opendss/README.md` for the mandated epri-worker contingency.

**Retired probe scripts.** The Phase E retirement deleted the one-off oracle
probes `sweep_modes_isolated.py`, `sweep_merge.py`, `ab_compare.py`
(retired by design — the two-channel gate replaced their A/B reporting).
`tools/opendss/probe_59n.py` — the checked-in reproduction artifact for the
59N relay chaotic-dynamics floor (2026-07-17 reproducibility remediation) —
was deleted with them but has since been **recreated over the `epri-worker`
bridge** (EPRI-bridge parity round): it reproduces the artifact verbatim
(all-closed no-trip, ~1381 A, chaotic 67–115 Hz pole-slip; exit 0), so the
citations at `crates/dss-core/src/elements/control/relay/tests.rs` and
`tests/corpus/manifests/skipped_needs_investigation.json` are live again.

The three `der_controls` / `line_constants` / `harmonics` gates share one
replay engine, `tests/harness/scenario.rs::check_family`.

**Which oracle ever sees a `Save`/`Dump` byte** (R4133_PROPS RP3.11,
2026-09-03). Only one: the pinned dss-python capture behind the `reports/`
family (`gen_reports.py` → `golden_reports.rs`, the `save*` lines and the 44
`dump*` artifacts). **No channel of the unified corpus gate compares this
surface at all** — `capi_v0145` and `r4133` compare the assembled model and,
since RP4.1, the property table through `harness::compare_all_properties`
(`exec/view.rs::element_properties`), which reads the same `ClassProps::
get_value` renderer but never a serialized `Save`/`Dump` line. So a divergence
between the port's re-serializer and **r4133's** can be caught by no golden and
no ledger row, and is carried instead by expected-value pins that quote **both**
engines' bytes: `save_renders_the_live_model_after_ncim_pv2pq`,
`dump_renders_the_live_model_after_ncim_pv2pq`,
`save_membership_follows_property_tracking_not_prpsequence` and
`save_omits_the_tapwinding_that_r4133_stamps`
(`crates/dss-core/src/exec/tests/report.rs`), with the r4133 side measured
through `epri-worker`. RP3.11's verdict for both surfaces is `KEEP_LIVE_PINNED`
— values are the live field, membership is the explicitly-set chain — and it is
documented where the code lives (`report/save/save.rs` and `report/save/
dump.rs` module docs) and recorded in STATUS §RP3.11. Adding a `Save`/`Dump`
case to the corpus gate is therefore not a thing you can do by editing a
manifest: it would need a new comparison kind on both channels.

### The unified corpus gate (`crates/dss-core/tests/corpus_gate.rs`)

One scheduler-driven `#[test]` — `corpus_gate_all_cases_match_engines` — runs
the union of all four case manifests live: the vendored family
`tests/corpus/manifests/solvable_now.json` (294 decks from the
`tests/corpus/electricdss-tst` mirror) plus the synthetic families
`asymmetric` (53) / `controls` (107) / `modes` (72) — 526 cases. Each case's
**`engines`** field names its gating channel(s): `"capi_v0145"` (59), `"r4133"`
(101), or `"both"` (the default; 366 cases gate on both channels). Case key = the gate
label `solvable_now:<path>` / `<family>:<path>`. The test fails iff any case
failed **or any ledger entry is stale**, printing the complete failure list
(manifest order), and reports per-entry ledger hit counts.

The module tree under `crates/dss-core/tests/corpus_gate/`:

- **`manifest.rs`** — schema + loading + structural family gates: dir↔manifest
  bijection, `required` floors, `pending ⇒ wp`, `isolate ⇒ note`, valid
  `engines`/`ad` values.
- **`engines.rs`** — the transports. Persistent worker pools per channel
  (`WorkerPool` = `python -u oracle_server.py`, `EpriPool` = `epri-worker`),
  both speaking the same line-JSON `ping`/`run`/`quit` protocol and returning
  the identical `CaseResult` shape; one-shot variants back `isolate`/serial
  runs. Per-request deadline (`DSS_ORACLE_TIMEOUT_SECS`, default 120 s) →
  kill/respawn/retry-once-then-fail-the-case. Workers are **recycled after
  every case by default** (`DSS_GATE_RECYCLE_AFTER` default 1): each case sees
  a never-used engine process, which is what makes the gate deterministic
  (persistent-worker state that `clear` does not reset — `Set` options,
  loadshape file handles — was proven to leak cross-deck otherwise).
- **`runner.rs`** — `run_rust_capture` + `compare_capture`: the untouched
  `harness/mod.rs` comparators run on the ledger-unscoped remainder of every
  comparison field; `CorpusGuard` restores each case dir (recursive — an
  overwritten entry is put back, a deleted one stays deleted; the
  `crates/dss-epri/src/guard.rs` port covers the r4133 side).
- **`scheduler.rs`** — task = case-dir group (cases sequential inside, so no
  two threads ever touch one dir), pre-sorted longest-first, drained by
  `DSS_GATE_JOBS` (default `available_parallelism()`) threads via an
  `AtomicUsize` cursor + `std::thread::scope`; per-channel pool size
  `max(2, jobs/2)`; per-case `catch_unwind`.
- **`ledger.rs`** — the divergence ledger (next section).

Case classes beyond plain live-compare:

- **`expect_solve_abort`** — the deck must abort with the pinned message (valid
  on both channels).
- **`pending: true`** — covers a feature the port does not implement yet: the
  gate asserts the Rust engine **errors loudly** (never a silent fallback);
  the WP named in `wp` flips the flag when it ports the feature.
- **`defer_ledger: "<cause>"`** (+ mandatory `wp`) — parked from live oracle
  comparison because the case reproduces on **neither** surviving channel or
  needs its own WP first (7 cases: NCIM×4 op-point → WP-U1.7, DynExp×2 +
  regcontrol_idle → ORPHANED_GAPS §1.9). Still **Rust-smoke-run** every gate:
  compile + every-step convergence + no new errors (a numeric regression that
  still converges is NOT covered — that returns when the case is ledgered or
  its WP lands).
- **`isolate: true`** (+ mandatory `note`) — every engine execution of the case
  runs on a throwaway one-shot worker process (33 cases: held-open
  trace/DI-CSV file handles, the AutoAdd process-exit corruption, file-backed
  loadshape decks).

**A deck whose oracle disagrees with itself does not gate on that channel**
(2026-09-04, GOLDEN_REBASE G1.4a, coordinator decisions D12/D14). The pinned capi
0.14.5 oracle is nondeterministic **across fresh processes** on any deck that
instantiates a `GICTransformer` — measured 7 bad runs of 60 with the element and 0
of 40 without, and 3 distinct bus-`kVBase` outcomes in 8 runs while the element's
YPrim and the assembled system Y come back bit-identical. The divergence sits in
`SetVoltageBases`' zero-load snapshot (`.inputs/dss_capi/src/Common/Solution.pas:1083`
→ `:1025`-`:1051` → `:1103`): `NodeV` is `ReAllocMem`'d, not zeroed
(`Common/YMatrix.pas:416`, only `NodeV[0]`), `SolveSystem`'s return code is
discarded, and whatever a node keeps is read straight into `nearestBasekV`, which
punts `kVBase` to 0. r4133 carries the same code but is deterministic on these
decks (80/80 runs bit-identical), and a channel that disagrees with itself cannot
gate, so all four GICTransformer decks carry `engines: "r4133"` and the guard test
`no_capi_gated_case_instantiates_a_gictransformer` (`corpus_gate/manifest.rs`)
refuses a capi-gated deck that `new`s one. The WPG.21 `MakePosSequence` coverage
that used to ride `modes:makeposseq/makeposseq_shunt.dss` moved into the new
r4133-gated micro deck `modes/makeposseq/makeposseq_gic.dss` instead of flipping
the shunt deck, because r4133's `MakePosSequence` round-trips reduced parameters
through a 5-significant-digit command string (`Capacitor.pas:801`/`:806`/`:829`)
and would quantize exactly what that deck exists to compare. Evidence:
`docs/upgrade/DIVERGENCES.md` §D12/D14, `investigations/issue-37-…`,
`investigations/to_opendss/58-…`.

**WP-G1 compare-depth flags (the vocabulary, declared once).**
`GOLDEN_REBASE_PLAN.md` WP-G1 widens the live gate to fastdss parity one surface
at a time, and each surface is opt-in per case. All ten flags were declared in
**one** commit (sub-step G1.0, 2026-09-04) rather than one per sub-step, because
each addition to the rigor fingerprint rewrites all 526 rows of
`population.lock.json`, and a flag that is *not* in that fingerprint can be
switched off later with no lock diff at all:

| manifest flag | rigor token | owning sub-step | surface |
| --- | --- | --- | --- |
| `compare_derived` | `derived=` | G1.3a–c | per-element `CurrentsMagAng`/`VoltagesMagAng`/`Residuals`, `SeqCurrents`/`SeqVoltages`/`SeqPowers`, `CplxSeq*`, `TotalPowers`. **Wired since G1.3a (2026-09-04)** — `Enabled` plus the three polar channels — extended by **G1.3b (2026-09-05)** with the three sequence surfaces over the same forced population (`FORCED_DERIVED_POPULATION` = (445, 314, 87, 44); each sub-step widened the flag's *fields*, not its population), and **completed by G1.3c (2026-09-06)** with `CplxSeqCurrents`, `CplxSeqVoltages` and `TotalPowers` over the same cases — all **thirteen** fields compare on both channels |
| `compare_element_extras` | `elemx=` | G1.3d | `PhaseLosses`, `NodeOrder`, `EnergyMeter`, `OCPDevType`/`OCPDevIndex`, `HasVoltControl`/`HasSwitchControl`, `NumControls`, `NumTerminals`/`NumPhases`/`NumConductors`. **Complete since G1.3d(ii) (2026-09-05)**: G1.3d(i) landed the index/name scalars (the three counts, `NodeOrder`, `EnergyMeter`), G1.3d(ii) `PhaseLosses` and the five control-derived scalars — all ten fields compare on both channels, over the same **443** forced cases — `FORCED_ELEMENT_EXTRAS_POPULATION` = (443, 312, 87, 44), the population G1.3d(i) already had (the sub-step widened the flag's *fields*, not its population; the lane measured it at 440 before G1.4a/G1.5/G1.6(i)'s corpus moves reached it at this merge) |
| `compare_bus` | `bus=` | G1.4a–d | **live (G1.4a):** `Nodes`/`kVBase`, `puVoltages`/`puVmagAngle`/`VMagAngle`, `AllBusVmagPu`; **live (G1.4c):** `SeqVoltages`/`CplxSeqVoltages`, `VLL`/`puVLL`; **live (G1.4b, 2026-09-05):** `Distance`/`AllBusDistances`/`AllNodeDistances`; **live (G1.4d, 2026-09-06):** `AllPCEatBus`/`AllPDEatBus` |
| `compare_zsc` | `zsc=` | G1.5 | **live (G1.5):** `Zsc1`/`Zsc0`/`ZscMatrix`/`YscMatrix`/`Isc`/`Voc`, per bus, on the same walk as `compare_bus` |
| `compare_reliability` | `rel=` | G1.6 | meter extras + the per-bus reliability columns; also drives the executive `RelCalc` |
| `compare_pdelements` | `pde=` | G1.6b | the `PDElements` interface walk — **wired 2026-09-04** |
| `compare_topology` | `topo=` | G1.7 | `NumLoops`/`NumIsolated*`/`AllLoopedPairs`/`AllIsolated*` |
| `compare_inc_matrix` | `incm=` | G1.8 | `IncMatrix`/`IncMatrixCols`/`IncMatrixRows`/`Laplacian` |
| `compare_run_files` | `runf=` | G1.10a | the SET of filesystem entries the run creates under DataPath (names, incl. created directories) — **wired 2026-09-06**; contents are G1.10b/c |
| `compare_di` | `di=` | G1.10b | the DI tree |

**G1.9 — circuit aggregates + solution scalars (unflagged, universal).** G1.9
deliberately gets **no** flag — it is universal and cheap — so nobody adds an
eleventh flag and a second lock regen. Both transports emit two *unconditional*
checkpoint members: `aggregates` (`losses_w`, the one W/var member, plus
`line_losses_kw`, `substation_losses_kw`, `total_power_kw`,
`all_element_losses_kw` — upstream rescales four of the five arms and not
`Circuit.Losses`, so the unit is in the key name) and `solution_scalars`
(`mode`, `hour`, `year`, `control_iterations`, `total_iterations`,
`most_iterations_done`, `control_actions_done`, `system_y_changed`, `seconds`,
`load_mult`). `harness::aggregates::compare_aggregates`
(`crates/dss-core/tests/harness/aggregates.rs:251`) and its
`compare_solution_scalars` sibling run on **every** live case of every gating
channel; because there is no flag to name, the surface refuses an absent capture
with its own assert naming surface, channel tag and case rather than with
`capture_guard::require_capture`, whose message is manifest-flag shaped. The
capture sits in **group A** — ahead of every `Currents` read *and* ahead of every
`First/Next` walk, since each arm leaves its own
`PDElements`/`Lines`/`Transformers`/`Sources`/`CktElements` cursor at the end —
and `crates/dss-core/tests/capture_order.rs` asserts that position in both
transports' sources — `capi_capture_reads_the_aggregates_before_any_currents_read`,
`r4133_capture_reads_the_aggregates_before_any_currents_read` and the
self-test `the_gate_rejects_a_swapped_or_renamed_capture`. r4133 returns
`SystemYChanged`/`ControlActionsDone` as `0|1`
ints (`DDLL/DSolution.pas:192-196`, `:226-230`); the **bridge** normalizes them to
the capi transport's JSON `bool` so both transports stay byte-identical in shape,
pinned by `r4133_solution_flags_are_zero_one_ints`, and refuses a `myType=3`
reply that is not exactly two doubles rather than padding it into a plausible
`(0, 0)` (`complex_pair_refuses_a_reply_that_is_not_two_doubles`). Both booleans
are the same value on every corpus checkpoint measured, so their two-sidedness is
witnessed in-engine instead: `the_two_boolean_solution_flags_take_both_values`.
The G1.9 pin names these docs cite are machine-checked by
`the_g1_9_pins_the_docs_cite_exist_exactly_once`
(`crates/dss-core/tests/oracle_parity_cfg_gate.rs`), so renaming one reds the
gate instead of silently falsifying this file.

Two quantities on the surface's list are captured or witnessed but deliberately
**not** compared a second time. `Solution.Totaliterations` *is*
`Solution.Iteration` upstream (`DDLL/DSolution.pas:218-220`; `CAPI_Solution.pas`
carries the comment "Same as Iterations interface"), so the gate asserts that
oracle-side alias live on every checkpoint instead of re-comparing the already
compared `iterations`. `Circuit.YCurrents` (`DDLL/DCircuit.pas:777-787`) returns
`Solution.Currents` — byte-for-byte the `injection` vector
`tools/golden/gen_checkpoints.py` captures and the gate already compares — so no
`YCurrents` channel was built at all.

**The value arms inherit the ledger; they never re-pin it.** An aggregate is a
linear functional of the per-element `currents`/`powers`/`losses` the ledger
already partitions, so re-pinning a scoped element's echo in the sum would grow
the ledger for a divergence it already owns. `compare_aggregates` therefore feeds
its value arm from the runner's accepted `LedgerView::element_rewrites`
(`crates/dss-core/tests/corpus_gate/ledger.rs:1205`) — the same caps
`compare_element_channels` is handed one loop above. Where a **deck-wide**
`element` scope selects `losses`, every summand is rewritten and the value arm
is then a self-comparison on that deck. That is inherent, not a comparator
choice: re-stating the arm against the oracle's own aggregate with the accepted
divergence added to the envelope is a tautology (triangle inequality), so once
the ledger owns every summand no bound on their sum carries oracle content the
entries do not already own. What the G1.9 audit settlement adds is **visibility**
— the 14 (case, channel) pairs where that happens are recorded and asserted
exactly by
`corpus_gate::ledger::the_aggregate_value_arms_inherit_exactly_the_recorded_element_scopes`,
so a new deck-wide element scope reds until its author acknowledges that it also
switches that deck's aggregate value arm off (coordinator decision D11(2)'s rule
for the analogous bus-array suppression). `Circuit.TotalPower` is unrebuildable
from a per-element cap (it reads terminal 1 and the capture carries no
`nconds`), so instead of being dropped whenever a source merely appears in the
rewrite map it absorbs the accepted `powers` divergence summed over **all** of
that source's conductors — the same documented conservative superset its
allowance already uses. An entry that scopes only `currents` no longer switches
the arm off. The membership and identity arms never soften: they run on the raw
oracle capture on every case, ledger-scoped ones included. Net effect on the
ledger: **0** entries and 0 new `LEDGER_FIELDS`. Bands and their derivations:
`tests/TOLERANCE_NOTES.md` §"G1.9 circuit aggregates + solution scalars".

**A flag may not be set before its surface exists.** `G1_SURFACE_FLAGS`
(`crates/dss-core/tests/corpus_gate/manifest.rs:618`) carries each flag's owning
sub-step and a `wired` bit; the structural family gate refuses any manifest case
that sets a flag whose `wired` is still `false`, because the request builder
would not send it, the oracle would return nothing, and the case would compare an
empty capture against an empty capture — green and vacuous. Each sub-step flips
its own row in the **same commit** that adds the request field and the
comparator (`no_unwired_g1_surface_flag_is_set_in_any_manifest`, driven
non-vacuously by `an_unwired_g1_surface_flag_on_a_case_is_refused`).

Two rails keep that refusal from being opted out of by accident. `SolvableCase`
is `#[serde(deny_unknown_fields)]`: every field is `#[serde(default)]`, so
without it a misspelled key (`compare_zsc_`, `compare_zsC`) would deserialize to
`false`, the gate would never fire and the lock would record the flag as off
while its author believed the surface was on. And
`every_manifest_compare_flag_has_a_rigor_token` asserts the two tables
**partition** the vocabulary: every `compare_*` field is either a named pre-WP-G1
flag or owns a `G1_SURFACE_FLAGS` row — never both, never neither — so an
eleventh flag cannot own the fingerprint table and silently miss the refusal.

**And a wired flag may not compare nothing.** Every flag-gated comparator calls
`harness::capture_guard::require_capture`
(`crates/dss-core/tests/harness/capture_guard.rs:68`) — or `require_capture_opt`
(`capture_guard.rs:86`) in place of an `unwrap` — **before** it compares: if the
flag is on for a (case, channel) and that channel's capture for the surface is
absent or empty, the case **fails**, naming the flag, the channel tag and the
case. It is never a silent skip, because every comparator in the harness is a
"for each item the oracle sent" walk and an empty capture passes one trivially.
The two day-one callers are `compare_all_properties` and `compare_autoadd_log`
in `corpus_gate/runner.rs`; G1.6b's `PDElements` walk (below) is the third,
G1.7's `compare_topology` (2026-09-05) the fourth and G1.8's
`compare_inc_matrix` (2026-09-05) the fifth. All three later ones use the `_opt`
form: an empty PD walk is legal, and so is a case that never requested topology
or the incidence pair.
`compare_eventlog`,
`compare_ctrlqueue` and `compare_global_result` deliberately do **not** call it:
an empty event log or control queue is a legitimate reading there, so the rail
would change live gate behaviour. Adding it belongs to whoever proves the
emptiness contract for those three surfaces. The guard's own non-vacuity is
offline (`capture_guard::tests::{a_present_capture_passes, an_empty_capture_fails,
an_absent_capture_fails, the_message_names_the_flag_the_channel_the_context_and_the_shape}`).

**G1.3b — the per-element sequence transform** (2026-09-05, lane `lane-e`).
`SeqCurrents`, `SeqVoltages` (012 magnitudes, A and V) and `SeqPowers`
(complex kW/kvar) compare live on both channels through
`harness::compare_element_seq`
(`crates/dss-core/tests/harness/mod.rs:4910`), a **sibling** of
`compare_element_derived` rather than an edit of it, over the same
`compare_derived` flag and the same 442 forced cases. The engine side is one new
accessor inside `exec/view.rs::snapshot_elements` (never `report/export/seq_*`,
which branch on `nphases >= 3` and carry the report's rating/`Iresidual` logic),
and the r4133 side needed no new mode — `WP_G1_MODES` stays 97. Five things that
are not obvious from the field names:

* **The three-arm selector is discrete state, compared at zero tolerance.** Both
  engines branch on `NPhases <> 3` and the circuit's positive-sequence flag
  (r4133 `DDLL/DCktElement.pas:41`/`:43`, `:92`/`:94`, `:752`/`:754`; capi
  `CAPI/CAPI_Alt.pas:245`/`:248`, `:305`/`:308`, `:551`/`:553`), so the port
  carries the arm as `ElementSnapshot::seq_arm` and the comparator asserts the
  *shape that arm forces on the oracle*: exact `1.0` magnitudes plus the
  channel's exact `SeqPowers` sentinel on `NotAvailable`, exact `0.0` in slots
  `3t` and `3t+2` on `PosSeqSinglePhase`, real values everywhere on
  `ThreePhase`. The arm is taken from the port's own structure, never inferred
  from a value.
* **The n/A `SeqPowers` sentinel is normalized, not ledgered.** On the
  not-available arm the two oracles spell the same "no answer" differently —
  r4133 `cmplx(-1.0, 0)` (`DCktElement.pas:772`), capi
  `cmplx(-1.0, -1.0)` (`CAPI_Alt.pas:567`) — while both magnitude arrays read
  `Cabs(-1 + 0j) = 1.0` on both channels (`DCktElement.pas:680`/`:719`,
  `CAPI_Alt.pas:268`/`:324`). The engine emits r4133's spelling and
  `harness::na_seq_power` (`mod.rs:4586`) folds the capi capture onto it, gated
  on the structurally derived arm and on that channel's own spelling only. It is
  a comparator-level normalization in the `PROPS_NORM_R4133` sense — **not a
  tolerance and not a ledger row** (it would otherwise cost hundreds, i.e. the
  kill criterion); `tests/TOLERANCE_NOTES.md` §G1.3b states the argument, and
  `the_na_power_sentinel_fold_is_channel_scoped`,
  `the_capi_sentinel_on_the_r4133_channel_fails` and
  `the_port_emitting_the_capi_sentinel_fails` pin all three directions.
* **The r4133 channel carries one extra absolute term, and only there.** The two
  oracles use *different* 012 matrices: r4133 builds `Ap2s` by numerically
  inverting `As2p` built from the truncated `0.866025403`
  (`Shared/mathutil.pas:302-303` + `:562-564`), capi uses
  `0.8660254037844387` with the analytic inverse. `harness::seq_band`
  (`mod.rs:4477`) is therefore `abs + rel·mean_j|Xph_j|` **plus**
  `SEQ_C012 · max_j|Xph_j|` (`SEQ_C012` = `5.229590094302253e-10`,
  `mod.rs:4445`) on the r4133 three-phase arm alone — the capi channel keeps the
  ordinary `i_*`/`v_*` bands unwidened, because the transform is linear with
  unit row sums. The base is the **phase** magnitude, not the sequence one: the
  sequence base is measurably not a bound (`the_sequence_magnitude_base_is_not_a_bound`),
  the phase base is attained exactly
  (`the_c012_bound_is_attained_by_the_aligned_phase_vector`), and the whole
  derivation with its measured headroom is `tests/TOLERANCE_NOTES.md` §G1.3b.
* **The ledger's two handlers cover exactly the slots the comparator bands.**
  On the n/A arm the whole payload is discrete and on the posseq arm slots
  `3t`/`3t+2` are exact zeros, so a scope widened onto `seq_powers` would blow
  its own envelope on a folded capi sentinel and would neutralize a discrete
  miss the comparator asserts exactly. `envelope_element` and
  `rewrite_element_selected` are therefore both keyed on one predicate,
  `seq_slot_is_banded` (`crates/dss-core/tests/corpus_gate/ledger.rs:1759`),
  pinned slot by slot on all three arms by
  `the_seq_rewrite_and_the_seq_envelope_cover_the_same_slots`
  (`ledger.rs:3953`); a `seq_*` scope over an element with no banded slot
  measures nothing and is reported STALE rather than passing silently.
* **The 1φ-positive-sequence population is a fail-on-stale census, recorded by
  the runner.** r4133's `SeqPowers` writes the single positive-sequence value
  into slot `3t+2` of each terminal instead of `3t+1` (`Count := 2` at
  `DCktElement.pas:760` plus `inc(count)` at `:768`, on a 0-based array; capi's
  `iCount := 1` / `inc(icount, 3)` at `CAPI_Alt.pas:555`/`:562` is right), so the
  engine emits capi's correct layout and the gate must prove exactly which cases
  reach that arm on the r4133 channel. Measured over the whole gated population,
  identical in both lanes: **297 867** compared element rows, **78** on the 1φ arm
  via `capi_v0145`, **4** via `r4133` — the four elements of the single deck
  `modes/makeposseq/makeposseq_gic.dss`, which coordinator decisions D12/D14 moved
  onto that channel and whose `makeposseq` leaves every element 1-phase. Those
  four rows are the ledger entry `r4133-posseq-seqpowers-slot-gic` (scope
  `element`/`seq_powers` alone — `SeqCurrents`/`SeqVoltages` are right on r4133
  there and stay compared) with the both-numbers pin
  `the_gic_posseq_deck_keeps_every_terminal_power_in_its_positive_slot`;
  everywhere else the arm stays off `r4133`, which is what
  `harness::assert_seq_arm_population` (`mod.rs:4451`) re-derives every run.
  On that arm the zero- and negative-sequence slots are compared **exactly**, and
  D31 is the ONE case where a ledger scope reaches them: the whole `SeqPowers`
  result there is a single mode-9 write and r4133 misplaces it, so an
  **`exclusion`** naming `seq_powers` neutralizes every slot of that arm's power
  array (`corpus_gate::ledger::rewrite_element_selected`) — otherwise it would
  exclude nothing. A `divergence` still does not (its envelope must keep covering
  everything it neutralizes), `seq_currents`/`seq_voltages` and the
  not-available arm are untouched in both kinds, and the **port's own** four
  zeros are asserted outside every scope, so an engine writing r4133's slot reds
  even on the excluded case. Measured slot by slot on all three arms and both
  kinds by `an_exclusion_scope_neutralizes_the_whole_posseq_seq_powers_array`,
  beside the unchanged `the_seq_rewrite_and_the_seq_envelope_cover_the_same_slots`.
  The census stands beside the engine pin
  `seq_powers_positive_sequence_lands_in_the_positive_slot_of_each_terminal`.
  The census was `(297 896, 79, 0)` on lane `lane-e` and was re-derived on the
  merged tree as `(297 867, 78, 4)` — `update`'s own population, and one capi 1φ
  row fewer because D14 moved the `GICTransformer` off `makeposseq_shunt.dss` —
  with that entry and that pin at the landing (**coordinator decision D31**,
  2026-09-05) — the guard's own precondition for moving.
  What that guard enforces (G1.3b audit settlement, 2026-09-05, the
  `check_control_census` shape): the **r4133 count is pinned exactly** — the
  one number that would hide a wrong comparison, and it moves only together with
  a ledger entry and a pin (both directions red:
  `the_seq_arm_population_fires_when_another_deck_brings_the_arm_to_r4133`,
  `the_seq_arm_population_fires_when_the_r4133_arm_stops_gating`) — while the two
  measured counts
  are railed by the documented floors `SEQ_ARM_CENSUS_FLOORS` (200 000 rows,
  60 capi 1φ rows) against `SEQ_ARM_CENSUS_MEASURED`, so a lane that retires or
  re-gates a deck passes while the two failures the guard exists for — the
  compare not running, and the capi-side gating of the 1φ layout collapsing to
  one deck's worth of rows — red
  (`the_seq_arm_population_fires_when_the_capi_arm_collapses`).
  The count is incremented **only** from the corpus-gate runner's loop
  (`harness::record_seq_arm`,
  `crates/dss-core/tests/corpus_gate/runner.rs:1175`), the
  `record_control_census` precedent, because `harness::seq_floors`' fixture calls
  run inside the same test binary and would otherwise move the shipped statics —
  coordinator decision D24, pinned by
  `a_fixture_call_on_the_r4133_posseq_arm_does_not_move_the_census`, which since
  D31 drives the comparator more times than the gate's whole run can contribute
  and bounds the movement by that contribution (no absolute value of a shipped
  counter is assertable from a thread running beside the gate). The rule
  generalises: **a population census is recorded by the runner, never by a
  comparator, and the numbers it judges are the ones measured under the full
  test binary**, not a filtered run.

**G1.3c — the complex sequence pair and the terminal totals** (2026-09-06, lane
`lane-e`). `CplxSeqCurrents`, `CplxSeqVoltages` (the un-`Cabs`'d 012 vectors, A
and V) and `TotalPowers` (per-terminal kW/kvar) compare live on both channels
through two more siblings of `compare_element_derived` —
`harness::compare_element_cplx_seq`
(`crates/dss-core/tests/harness/mod.rs:5269`) and
`harness::compare_element_total_powers` (`mod.rs:5541`) — over the same
`compare_derived` flag and the same 442 forced cases, completing the flag's
thirteen fields. The engine side is again additive inside
`exec/view.rs::snapshot_elements` — `cplx_seq_currents`/`cplx_seq_voltages`
(`crates/dss-core/src/exec/view.rs:324`) and `total_powers`
(`:357`) — which already computed the 012 vectors and discarded them and already
held the per-conductor `V·conj(I)`; the r4133 side needed no new mode
(`WP_G1_MODES` stays 97 — modes 13/14/20 were proven `Served` by G1.0) and two
additive helpers, `Engine::element_total_powers`
(`crates/dss-epri/src/dss.rs:1135`) and `Engine::element_cplx_seq` (`:1198`).
Five things that are not obvious from the field names:

* **The complex compare is strictly stronger than the magnitude one, at the
  same number.** Modes 13/14 call the very `CalcSeqCurrents`/`CalcSeqVoltages`
  helpers modes 7/8 call and skip the `Cabs` (r4133
  `DDLL/DCktElement.pas:931-975`/`:885-928`; capi `CAPI/CAPI_Alt.pas:898-925`/
  `:872-895`), so the floor is G1.3b's `harness::seq_band` unchanged — a bound
  on the **complex** difference in the first place, which
  `compare_element_seq` spends on magnitudes through the weaker
  `||a| − |b|| ≤ |a − b|` and `compare_element_cplx_seq` spends directly. What
  that buys is the angle: a pure phase rotation of `1e-6` rad on the port's 012
  currents reds this comparator on both channels while
  `compare_element_seq` — running on the identical snapshot immediately before
  it — stays green (measured live, `[CapiV0145] line_asym` |Δ|
  `5.620180314736508e-5` A > `1.1198951792527244e-6`; `[R4133]
  line_geometry_asym` `1.2703834141305406e-5` > `1.1511897967148642e-6`). No
  coefficient moved, no new class, and D10's polar-band derivation does not
  apply: this is a rectangular/modulus compare, so no angle band exists here.
* **Unlike `SeqPowers`, the not-available sentinel needs no fold.** Both
  engines write `(-1, 0)` on this surface — r4133 `Cmplx(-1.0, 0.0)`
  (`DCktElement.pas:60`/`:106`), capi's `i012[i] := -1` / `V012[i] := -1`
  (`CAPI_Alt.pas:268`/`:324`) being the same complex value — measured
  cross-channel at max `|Δ| = 0.0`. So there is **no `na_seq_power` twin** and
  no comparator normalization at all: the slot is asserted exactly, on both
  channels, under every channel policy, and pinned in the engine by
  `cplx_seq_na_sentinel_is_minus_one_plus_zero_j_on_both_engines`. r4133's
  1φ-posseq slot/stride defect likewise does not reach modes 13/14 (it lives in
  mode 9's inlined copy; the shared helper's `iV := 2`/`Inc(iV, 3)` indexes a
  1-based buffer), so **no new census** is added either (D24).
* **`TotalPowers` joins the Newton lane exclusion; the two `CplxSeq*` do not.**
  `GetPhasePower` opens with the cache-aware `ComputeIterminal` (r4133
  `Common/CktElement.pas:1049`), so `TotalPowers` carries CLAUDE.md's stale-
  `Iterminal` bug 5 exactly as `Powers`/`Losses`/`PhaseLosses` do — measured on
  the **live gate, on both channels, before the bit was written**: ~20x the
  band on `modes:newton/newton.dss` and `newton_feeder.dss` (the four
  measurements are in `harness::lane::elem_channels_for`'s G1.3c section and in
  `tests/TOLERANCE_NOTES.md` §G1.3c derivation 4), pinned by
  `newton_total_powers_match_the_normal_algorithm`. It is the fourth channel of
  `LANE_SKIP_ELEM_POWERS` and its deck list does not change. The two
  `CplxSeq*` reach `GetCurrents` into a scratch buffer, so those two decks
  **gain** two oracle-compared channels; only `TotalPowers`' *values* are
  dropped, its array shape is still asserted there.
* **Reads are `Enabled`-only, and two shape asymmetries survive that.** A
  never-enabled element answers with three different shapes on the two channels
  (r4133 mode 20 has no `NodeRef` guard and returns `NTerms` zeros where capi's
  `MissingSolution or (NodeRef = NIL)` returns one complex), so the capture
  skips disabled elements on both channels and each comparator asserts the
  oracle side is silent — the rule G1.3a and G1.3b already set for this flag.
  What remains is a **0-terminal** element (`UPFCControl`): capi's early
  returns still put `[0.0]`/`[0.0]` on `tp_kw`/`tp_kvar` and `[0.0]` on
  `cseq_v_re` (`CAPI_Alt.pas:1119-1123`, `:878`) where r4133's
  `setlength(…, NTerms)` collapses to nothing. Both are admitted by two-sided
  emptiness predicates measured from the wire, not assumed — the
  `no_seq_payload` precedent, i.e. comparator-level normalizations with **0
  ledger rows** (D4) — and pinned from both sides
  (`the_zero_terminal_total_power_shapes_are_the_measured_ones`,
  `a_zero_terminal_element_accepts_the_measured_capi_sentinels`).
* **Two bands that were written twice are now written once.** The per-terminal
  `(bv, bi)` walk lives in `harness::seq_terminal_bands` (`mod.rs:4542`), used
  by `compare_element_seq`, `compare_element_cplx_seq` **and** the ledger's
  `envelope_element`, so a ledger envelope can no longer be evaluated at a
  different number than the compare it bounds; and the per-conductor power
  expression is the single `power_slot_band` (`mod.rs:3224`) that both
  `harness::phase_loss_band` and the new `harness::total_power_band`
  (`mod.rs:3274`) sum — over a phase's conductors and over a terminal's
  respectively. Both extractions are byte-faithful and pinned against literal
  transcriptions of the copies they replaced.

**G1.3d(i) — the per-element discrete extras** (2026-09-04/05, lane `lane-e`).
`harness::compare_element_extras`
(`crates/dss-core/tests/harness/mod.rs:2860`) compares `NumTerminals`,
`NumConductors`, `NumPhases`, `NodeOrder` and `EnergyMeter` **exactly** — and,
since G1.3d(ii), the five control-derived scalars `NumControls`, `OCPDevIndex`,
`OCPDevType`, `HasVoltControl` and `HasSwitchControl` on the same terms: it takes
no `Tolerances` argument and owns **no** ledger sub-channel, so a divergence in
discrete state cannot be masked by a committed `element` scope — it is a gate red
and a STOP (`tests/TOLERANCE_NOTES.md` §G1.3d(i) says why no floor exists here).
Five things about it that are not obvious from the field names:

* **`NodeOrder` is captured for `Enabled` elements with `NumTerminals > 0` only**,
  and the predicate is source-derived rather than defensive: r4133's
  `CktElementV(17)` dereferences `NodeRef^[j]` with no nil guard
  (`DDLL/DCktElement.pas:1048`) and kills the worker on a never-enabled element,
  where capi raises 15013 (`CAPI/CAPI_CktElement.pas:900-906` — the legacy
  `CktElement_Get_NodeOrder`, which is the entry point the pinned dss-python
  calls, not the equivalent `Alt_CE_Get_NodeOrder` twin), and on a 0-terminal
  element (`UPFCControl`, `Controls/UPFCControl.pas:230-246`) r4133 answers a
  0-length array where capi raises. Not issuing the read removes the shape
  asymmetry instead of normalizing it away.
* **The `!enabled` branch asserts the ORACLE side is silent, not both — and that
  leaves one subset uncompared, named here rather than discovered later.** The
  predicate is `Enabled`, but the hazard is a nil `NodeRef`: an element disabled
  *after* a solve keeps its mapping on all three engines (`Set_Enabled` only sets
  the flag and raises `BusNameRedefined`, r4133 `Common/CktElement.pas:438-465`;
  neither mode-17 arm has an `Enabled` guard), so both oracles WOULD answer for
  it and the capture still skips it. The port's vector there is therefore pinned
  in-engine only (`exec::tests::element_extras::a_disabled_element_keeps_the_node_order_it_was_given`,
  `a_stale_node_ref_reads_the_missing_slots_as_ground`), never against an oracle.
  A symmetric capture cannot close it: nothing on either transport reports
  whether `NodeRef` was ever allocated, and guessing wrong kills the r4133
  worker. Demanding an empty port vector instead would red every deck that
  switches an element out. Pinned both ways in `element_extras_pins`
  (`mod.rs:3847`).
* **"No meter" is a two-channel spelling, folded at the capture boundary — each
  channel's OWN sentinel, and the census is what makes it safe** (coordinator
  decision D4, 0 ledger rows): capi returns `''` (`Result := NIL`,
  `CAPI/CAPI_CktElement.pas:672-687`), r4133 returns `'0'` (the `CktElementS`
  pre-`case` default, `DDLL/DCktElement.pas:421`; arm 4 at `:442-449` is guarded
  by `HasEnergyMeter` at `:444`). `harness::oracle_meter_name`
  (`mod.rs:2754`) takes the channel and folds only that channel's spelling, so on
  the capi side a meter literally named `0` is a name like any other and a port
  that lost it reds (`a_meter_named_zero_reds_instead_of_passing`,
  `mod.rs:4040`). On the **r4133** side the collision is genuinely undecidable and
  cuts BOTH ways — a port that lost a meter named `0` would compare `None ==
  None` and pass (asserted, not assumed, by
  `the_r4133_zero_sentinel_is_undecidable_and_the_census_is_the_guard`). What
  keeps that unreachable is the corpus census below, not the fold; earlier
  wording here claimed the fold was self-detecting, which was true in one
  direction only (G1.3d(i) audit settlement, 2026-09-05).
* **`EnergyMeter` is sparse by design — do not read it as zone coverage.**
  `SetHasMeterFlag` sets `HasEnergyMeter` on each enabled meter's
  `MeteredElement` only (r4133 `Meters/EnergyMeter.pas:1712-1719`; port
  `solution/meters/zones/flags.rs::set_has_meter_flag`), so exactly one element
  per enabled meter carries a name and every other element of the zone reports
  none.
* **Two unrelated `node_order` fields share the name.** The per-element
  `ElementCap::node_order` (`mod.rs:1003`) is the bus-local node number per
  conductor per terminal; the checkpoint-level `CaseResult::node_order`
  (`crates/dss-epri/src/capture.rs:161`) is the **Y node name order** of the
  whole circuit. They live in different JSON objects and share nothing but the
  word; both field docs say so.

**G1.3d(ii) — `PhaseLosses` and the control-derived extras** (2026-09-05, lane
`lane-e`). The same flag, the same population (**443** on `update`; 440 on the lane), the last six fields of the surface.
The five control scalars fold into `compare_element_extras` above (exact, no
`Tolerances`, no sub-channel); `PhaseLosses` is numeric and therefore gets its
own comparator, `harness::compare_element_phase_losses`
(`crates/dss-core/tests/harness/mod.rs:3348`), precisely so that function's
"everything here is discrete" contract stays literally true. Six things worth
knowing:

* **The band is a derivation, not a new class.** `harness::phase_loss_band`
  (`mod.rs:3202`) sums `assert_power_close`'s per-conductor floor
  `abs·max(1,|V_k|) + rel·|S_k|` over exactly the conductors `GetPhaseLosses`
  sums (`k = j·NConds + i`, r4133 `Common/CktElement.pas:1093-1112`) — the same
  construction `compare_element_channels` already applies to `Get_Losses` over
  *all* conductors, restricted to one phase, hence strictly tighter. No
  `Tolerances` field is read or written; the derivation and its measured
  headroom are `tests/TOLERANCE_NOTES.md` §G1.3d(ii). The `Get_Losses`
  oracle-self-consistency trust escape is deliberately **not** copied here (it
  exists for a capi015 quirk on a rev this gate does not run, so a twin could
  only mask).
* **Shape is asserted under every channel policy, the value only under the
  channel bit.** Both oracle halves must be the same length and both sides must
  carry exactly `NumPhases` samples — the zero of a 0-phase `UPFCControl`
  included — so a capi one-element `DefaultResult` sentinel reds instead of
  de-interleaving into a silent `[0.0]`. The engine keeps `PhaseLosses` in
  **W/var** (`CktElement::phase_losses`,
  `crates/dss-core/src/elements/traits.rs:966`); the oracles' ×0.001 (r4133
  `DDLL/DCktElement.pas:651`, capi `CAPI/CAPI_Alt.pas:466`) is a
  capture-boundary encoding applied at exactly one site, this comparator.
* **`PhaseLosses` is the first channel to JOIN `LANE_SKIP_ELEM_POWERS`**
  (`crates/dss-core/tests/harness/lane.rs:145`), where G1.3a's three polar
  channels did not: `GetPhaseLosses` opens with the same cache-aware
  `ComputeIterminal` as `Get_Powers`/`Get_Losses`, so on the two `newton*` decks
  no oracle reports it at the converged `NodeV` (CLAUDE.md bug 5 / G2.3). The
  seventh `ElemChannels` bit (`ElemChannels::phase_losses`, `mod.rs:1707`) is
  `true` in `ALL` and `false` in `CURRENTS_ONLY`, and the exclusion was
  **measured before it was added, on BOTH gating channels**: both decks red on
  `Vsource.source` phase 0 at 55.5× / 34.4× the band, on figures bit-identical
  to the `Powers` numbers the same row already records. The `r4133` half needed
  its own scratch run (a case aborts on its first failing channel, so the capi
  red hides the r4133 one) — G1.3d(ii) audit settlement, 2026-09-05. The five
  discrete scalars stay compared there.
* **The control-derived scalars read a DERIVED list, with no `Enabled`
  filter.** Pascal keeps a per-element `ControlElementList`; the port keeps a
  circuit-wide attach order (`Circuit::reattach_control`,
  `crates/dss-core/src/circuit/circuit.rs:704`) and buckets it per element on
  demand (`circuit::controls::derive_control_lists`,
  `crates/dss-core/src/circuit/controls.rs:119`), which `Show Controlled` and
  the reliability sweep's live `GetOCPDeviceType` scan (`live_ocp_device_type`,
  `crates/dss-core/src/solution/meters/reliability.rs:50`) now share, so the
  report, the API surface and the sweep cannot drift apart. A **disabled** OCP
  control still holds its slot and still wins the scan on both oracles (r4133
  `Common/Utilities.pas:3165-3184` has no `Enabled` test), which is why the
  accessors recompute instead of reading the registration latch.
* **The population guard behind D-ii-1's zero ledger rows** (added by the
  G1.3d(ii) audit settlement, 2026-09-05, in the D15/D16 shape): the divergence
  between r4133's per-edit re-attach and capi 0.14.5's needs an element whose list
  holds an OCP control **beside a control of another class** before
  `OCPDevIndex`/`OCPDevType` can move. The whole-gate census — the oracle's own
  `NumControls`/`OCPDevType`, recorded at the gating call site
  (`harness::record_control_census`) and checked in the gate's epilogue
  (`harness::assert_no_multi_control_element`) — measures **298 565** compared
  (case, channel, step, element) rows, **3 184** with a control and **18** with two
  or more, every one of the 18 a **Relay-only** list (`Line.thev` in the eight
  Distance/TD21 relay decks, `Line.motorleads` in the two `indmach_r4133` decks),
  so every permutation answers the same `OCPDevIndex = 1` / `OCPDevType = 3`. The
  `(18, 18)` pair is pinned exactly and fails on stale in **both** directions (a
  new or retired multi-control element must be re-triaged against D-ii-1); `seen`
  and `controlled` carry non-vacuity floors. Silent under `DSS_GATE_ONLY`, which
  is not the population. **It also corrected the sub-step's own claim**: the
  census quoted in the record covered `tests/corpus/controls/**` only, where
  `max NumControls` really is 1.
* **Mode capability, measured, not assumed (D2's record):** all six r4133 modes
  already existed and were proven `Served` by G1.0 — `WP_G1_MODES` stays 97 and
  `crates/dss-epri/src/modes.rs` is byte-untouched — and on a **0-phase**
  element (`UPFCControl`, `controls/upfc/upfc_dual.dss`) `CktElementV(6)`
  returns a 0-length array and the worker survives, where capi returns `[]`. No
  capture predicate, no sentinel normalization and **no `DoNotCall` row** is
  owed for this group. The runner's non-vacuity rail counts elements with a
  **non-empty** `pl_kw` (`require_capture`,
  `crates/dss-core/tests/corpus_gate/runner.rs:1229`) rather than key presence,
  because r4133 omits the key on a 0-phase element while capi sends `[]`.
* **Trap, measured while proving the r4133 rail non-vacuous:** `cargo test -p
  dss-core` recompiles the `dss-epri` *library* but does **not** rebuild the
  `epri-worker` *binary* the gate actually launches, so a scoped `-p dss-core`
  run after an r4133-capture edit can pass **vacuously** against a stale worker.
  Run `cargo build -p dss-epri --bins` first; `cargo test --workspace` builds it,
  so the commit gate is unaffected. Second observation for the same registry
  channel as coordinator decision D13: the DLL reads **`DataPath`** from
  `HKCU\Software\OpenDSS` too, so a fresh `epri-worker` in one worktree can
  resolve a *relative* deck path against another worktree's last-used directory
  — probes pass absolute paths (D13/D14 pending sync on this lane).

**The corpus meter-name census lives in the oracle-free binary, on purpose.**
`no_corpus_energymeter_is_named_zero` (`crates/dss-core/tests/corpus_manifest.rs:292`,
module `extras_population` at `:200`) walks every vendored deck and asserts no
`New EnergyMeter.<name>` is spelled `0` — measured: **1310** deck files, **92**
distinct meter names, none of them `0`. This census, not the sentinel fold, is
the load-bearing guard behind the r4133 `'0'` collision above. It started in
`corpus_gate/runner.rs` and **flaked** there: the live gate in the same binary has
decks writing exports into the same tree, and a Windows sharing violation on a
gate-written `.txt` made the walk panic (an instrumented run counted 1311 files —
the census was reading the gate's own droppings). `cargo` runs one test binary at
a time, so moving the module to the oracle-free corpus-hygiene binary removes the
race structurally: no assertion was weakened, and there is no retry, skip or
`#[ignore]` anywhere in it. **The premise, stated so it is not silently lost:**
that removal buys separation only while the runner is `cargo`'s own sequential
one and only one gate runs per worktree (coordinator decision D13). A parallel
test runner (`cargo-nextest`) or two concurrent `cargo test` invocations in one
worktree would put a live-gate writer back beside this walk.

**The AD sweep is the residual co-tenant of that binary** (measured 2026-09-05,
GOLDEN_REBASE G1.3d(ii) commit gate). `corpus_ad_matches_normal_mode` still runs
in `corpus_gate` beside the live gate, and it compiles corpus decks under a
`CorpusGuard` that snapshots and restores the deck's directory while the live
gate is writing exports into the same tree. Under cross-lane load (D7 permits
concurrent gates in *different* worktrees) it red once on
`Test/IEEE13_LineAndCableSpacing.dss` with `ad-init: You must create a new
circuit object first`, then passed in **41.62 s** run alone in the same tree and
again inside a full unfiltered drive. Two things to know before believing such a
red: it is **not** a divergence — that message is only the first error recorded
after `set ADiakoptics=True`, i.e. the benign `Set DefaultBaseFrequency` the deck
re-issues on the tearing re-compile, not the reason `solution.adiakoptics` stayed
false; and the test's code path is byte-untouched by the surfaces WP-G1 adds
(`crates/dss-core/tests/corpus_gate.rs`, and `CorpusGuard` at
`crates/dss-core/tests/corpus_gate/runner.rs:112`). Re-run it with the worktree
and the machine quiet before triaging it, exactly as D13 prescribes for the
`ckt24` oracle-budget reds below.

**`Lines.Yprim` is already witnessed; `LineGeometries.Rmatrix/Xmatrix/Zmatrix`
leaves the parity claim** (the two G1.3d documentation verdicts). `Lines_Get_Yprim`
(`CAPI/CAPI_Lines.pas:777-796`) and `CktElement_Get_Yprim`
(`CAPI/CAPI_CktElement.pas:583-599`) are the same two statements —
`GetYprimValues(ALL_YPRIM)` and a bulk `Move` of `2·Yorder²` doubles
(`Move(cValues[1], …)` and `Move(cValues^, …)` are the same address for a 1-based
`pComplexArray`) — and r4133's `LinesV` mode 7 (`DDLL/DLines.pas:771-796`) and
`CktElementV` mode 12 (`DDLL/DCktElement.pas:856-883`) likewise copy `SQR(Yorder)`
complexes out of one such call; on the capi channel the two are exactly
equivalent modulo the `Lines` path's `IsLine()` type filter, and on r4133 they
differ only in bookkeeping outside the payload — mode 12 `Exit`s on a nil
`cValues` (`DCktElement.pas:869-872`) *before* assigning `myPointer`/`mySize`
(`:882-883`), while mode 7 keeps its one-element `CZero` array and assigns them
regardless (`DLines.pas:794-795`). Whenever there is a YPrim at all, both arms
copy the same `GetYprimValues(ALL_YPRIM)` block. The gate already captures and compares
`CktElement.Yprim` live on both channels (`tools/oracle/oracle_server.py:1357`,
`crates/dss-epri/src/capture.rs:930` over `Engine::element_yprim`
(`crates/dss-epri/src/dss.rs:879`), Rust side `compare_yprim` at
`corpus_gate/runner.rs:980` → `harness::compare_yprim`
(`crates/dss-core/tests/harness/mod.rs:1581`)), so a
second `Lines`-shaped capture would add no information. **The honest residual is
coverage, not spelling:** YPrim is compared only for a case's
`selected_elements`, and over the four manifests **238 of 526** cases declare a
non-empty list (195 `["*"]`, 43 explicit) while the other **288** compare no YPrim
at all — that gap closes by flipping `selected_elements` on more cases, never by a
Lines-specific capture. `LineGeometries.Rmatrix/Xmatrix/Zmatrix` are dropped from
the parity target on two independent kills: in fastdss they are computing
**methods** taking `(Frequency, Length, Units)` (`origin/fastdss`
`dss/ILineGeometries.py:84`/`:88`/`:92`), so `adjust_to_json` raises
`StopIteration` on them (`tests/save_outputs.py:140-141`) and the caller swallows
it (`:277-279`) — they are skipped in *every* fastdss run and are therefore not in
the surface this gate is reaching parity with; and the r4133 DLL has no
`LineGeometr*` family at all (no `DDLL/DLineGeometries.pas`, and
`OpenDSSDirect.dpr`'s `exports` clause carries only `LinesI/F/S/V`), so the
surface is capi-only by capability. Neither is captured; neither costs a ledger
row.

**Capture order is contractual on the capi channel** (`GOLDEN_REBASE_PLAN.md`
§1.1(a), restated 2026-09-04). Per element, in three groups:

* **(A)** the cache-aware quantities that go through `ComputeIterminal` —
  `Powers`, `TotalPowers`, `Losses`, `PhaseLosses` — are read **first**;
* **(B)** then every read that calls `GetCurrents` into a scratch buffer —
  `SeqPowers`, `SeqCurrents`, `CplxSeqCurrents`, `Residuals`, `CurrentsMagAng`,
  `Currents`;
* **(C)** order-free reads — `SeqVoltages`, the other voltages, discrete state —
  go anywhere.

The reason is CLAUDE.md's upstream bug 4 (harmonics stale-`Iterminal`): a group-B
read poisons the cache a group-A read would otherwise have refreshed, so the
oracle's answer depends on request order. `SeqPowers` is a poisoner, not a
victim. `SeqCurrents`/`CplxSeqCurrents` are already the same kind of read the
golden path states at `tools/golden/gen_checkpoints.py::capture_element`.

Since **G1.3c (2026-09-06)** all three of the names above that the partition
listed but nothing issued are actually issued: `TotalPowers` is read at the head
of the element on both transports, beside `PhaseLosses` and ahead of the group-B
`Currents` (it sums `GetPhasePower`, whose first act is `ComputeIterminal` —
r4133 `Common/CktElement.pas:1049`), and `CplxSeqCurrents` (B) /
`CplxSeqVoltages` (C) join the derived block after `SeqVoltages`. The rule the
sub-step therefore adds a member to is asserted positively **and** negatively by
`total_powers_is_a_group_a_read_issued_before_the_currents_read`; the three
mutation tests that used to use `TotalPowers` as a stand-in for "a read this body
does not perform" were re-anchored on `Voltages`, because a real declared read
would have made them assert their rule for the wrong reason.

**The rule is enforced, since G1.3a (2026-09-04).** Every read line inside a
capture body carries a `capture-order: <Name> (<A|B|C>)` marker, and
`crates/dss-core/tests/capture_order.rs` parses all eight capture bodies —
`tools/oracle/oracle_server.py::capture_all_elements` (plus the
`gen_checkpoints.capture_element` it calls), the five r4133 element helpers
`crates/dss-epri/src/dss.rs::element_phase_losses`, `element_pcl`, `element_polar`,
`element_seq` and `element_extras`, and
`crates/dss-epri/src/capture.rs::capture_all_elements` —
asserting that every A marker precedes every B marker, that the marker names are
exactly that transport's declared set, and that an **unmarked** read line inside a
capture body fails, so a later sub-step cannot add an invisible read. The A/B/C
group of a name is not a second table: it is
`dss_epri::modes::ModeEffect::capture_group()` on the shared WP-G1 mode table.
G1.3a's own reorder is part of the same contract — `Losses` (A) is now read
**before** `Powers`/`Currents` on both transports, where it used to follow them
(A → B → A); that is a pure reordering of two group-A reads, verified
byte-for-byte on IEEE13, two harmonics decks and the two user-model decks.

**The `PDElements` walk (G1.6b, 2026-09-04).** `compare_pdelements` turns on the
per-PD-element walk fastdss compares wholesale — the **13** `IPDElements._columns`
of `DSS-Python@origin/fastdss` plus `parent_name` — on every live non-`large`
case (`force_pdelements`, `crates/dss-core/tests/corpus_gate/scheduler.rs:335`;
the forced split is re-derived and pinned by `FORCED_PDELEMENTS_POPULATION`,
`crates/dss-core/tests/corpus_gate/scheduler.rs:356`). The port side is
`Dss::pd_elements` (`crates/dss-core/src/exec/view.rs:2561`), a `&self` read of
`CktElementData`; the comparator is `harness::compare_pd_elements`
(`crates/dss-core/tests/harness/mod.rs:11214`), which asserts the walk first
(length, then the name sequence case-insensitively — the oracle's
`PDElements.Count` is the raw `ListSize` and is deliberately **not** used) and
then all fourteen fields **exactly**, `rel = abs = 0`: nothing on this surface is
computed on either side (derivation in `tests/TOLERANCE_NOTES.md`). Three rules
come with it.

* **`ParentPDElement` is read last, per element, on both transports.** That read
  re-points `ActiveCktElement` at the parent and never restores it (r4133
  `DDLL/DPDELements.pas:88-97`, capi `CAPI/CAPI_PDElements.pas:245-257`), so
  reading it in the fastdss column position contaminates every later field of the
  same record — 215 cells on IEEE 123, measured. The contract is asserted
  statically over both capture sources by
  `the_capi_pd_capture_reads_parentpdelement_last` and its r4133 twin
  (`crates/dss-core/tests/pd_elements_pins.rs:361`), whose own non-vacuity test
  requires the scan to reject corrupted copies of the real capture bodies, and
  behaviourally against the DLL in `crates/dss-epri/tests/modes.rs`. The same file
  asserts each capture sits **between** the meters capture and the probes capture
  in its transport's `run_case`.
* **An empty walk is legal; an empty comparison is not.** 96 of the 372 walked
  live capi cases hold no PD element at all, so the per-case rail is
  `require_capture_opt` (presence, not count), and the capture is `null` when the
  flag is off and `[]` when it is on over a PD-less circuit. The hole that leaves
  is closed globally: `assert_pd_elements_compare_ran`
  (`crates/dss-core/tests/harness/mod.rs:11294`) fails the run unless **each**
  gating channel compared at least one non-empty walk.
* **`PD_SKIP_FIELDS` is fail-on-stale.** Both oracles read four cells out of
  uninitialized memory on in-zone shunt Capacitors/Reactors (`EnergyMeter.pas`
  assigns through `pPCelem: TPCElement`; nondeterministic across processes and, on
  r4133, within one), so those cells are excluded per (channel, class, field) in
  `PD_SKIP_FIELDS` (`crates/dss-core/tests/harness/mod.rs:11028`) — never
  enveloped, and never wider than measured. The scope is the element too, not the
  class: a row is consulted only where that write lands, on an in-zone **shunt**
  Capacitor/Reactor (`pd_skip_applies`,
  `crates/dss-core/tests/harness/mod.rs:11152`, port state
  `PdElementView::in_meter_zone`), so a series member of either class and a shunt
  one outside every zone stay fully compared — the defect is measured on 22
  (capi) / 29 (r4133) of the 372 / 431 walked cases. `fault_rate`/`pct_permanent`
  also stay compared on `r4133`, `lambda`/`accumulated_l` on `capi_v0145`, and
  Line / Transformer / AutoTrans / GICTransformer keep all four on both channels.
  Every row carries its Pascal citation and the pin that holds its value, a
  register test refuses a silent add or drop, a second one refuses a `pin` no
  `#[test]` defines, and `assert_pd_skip_rows_are_live`
  (`crates/dss-core/tests/harness/mod.rs:11347`) fails a row that excluded nothing
  in the whole run. The gate epilogue prints every row's visit/hit counts.

**The bus voltage surface (`compare_bus`, live since G1.4a, 2026-09-04).** Per
bus, in the engine's own `BusList` order: `Nodes`, `kVBase`, `puVoltages`,
`VMagAngle`, `puVmagAngle`, plus the checkpoint-level `AllBusVmagPu` — the four
quantities capi 0.14.5 and r4133 compute by byte-identical algorithms. The port
side is `Dss::all_bus_voltages` / `Dss::all_bus_vmag_pu`
(`crates/dss-core/src/exec/view.rs:2327`, `:2385`), the comparators are
`harness::compare_bus` (`crates/dss-core/tests/harness/mod.rs:11939`) and
`compare_all_bus_vmag_pu` (`mod.rs:12047`). Four things about it are worth
knowing:

* **Three ordering conventions meet here and must not be mixed.** The per-bus
  arrays are in ascending node **number** (the `repeat NodeIdx := FindIdx(jj)`
  walk, `CAPI_Alt.pas:2270-2275` == `DDLL/DBus.pas:415-421`), `AllBusVmagPu` is
  bus-list order × the bus's **internal node index** (the `AllNodeNames`
  permutation), and the gated `node_order` is `YNodeOrder` — a third one. The
  comparator asserts `Σ nodes == len(AllBusVmagPu)` so the first two cannot
  drift apart silently.
* **No manifest case sets the flag; the scheduler forces it.** `force_bus`
  (`crates/dss-core/tests/corpus_gate/scheduler.rs:609`) turns the surface on for
  every live case whose `kind` does not start with `large`, exactly like
  `force_properties`. `population.lock.json` fingerprints the **manifest** flag,
  so it cannot see that rule at all: the guard is the pinned
  `FORCED_BUS_POPULATION` (`scheduler.rs:626`) plus the oracle-free
  re-derivation `the_bus_forcing_rule_is_every_live_non_large_case`.
* **The bands add no tolerance constant.** Each one is the exact image of the
  already-calibrated node-voltage band over the same `Solution.NodeV`
  (`tests/TOLERANCE_NOTES.md` §"Bus voltage surface"); `kv_base` and `nodes` are
  compared **exactly**.
* **One documented normalization, printed by the gate.** On a case whose
  `voltages` field is already ledger-excluded **deck-wide**, the three continuous
  per-bus arrays are suppressed — they are exact images of a divergence that is
  already triaged and pinned there, so comparing them again would mean ten new
  ledger rows for one cause (coordinator decision **D11(2)**, 2026-09-04).
  "Deck-wide" is enforced, not assumed: the predicate is
  `LedgerView::bus_arrays_suppressed`, which honours only a `voltages` scope with
  neither `name_re` nor `node_re` — a scope naming a node SUBSET excludes fewer
  nodes than the bus arrays cover, so it suppresses nothing here (G1.4a audit
  settlement, 2026-09-05). Bus count,
  name sequence, `nodes`, `kv_base` and every array length stay compared, which
  `the_voltage_exclusion_still_pins_kv_base` (`mod.rs:13395`) drives negatively.
  It is not a mask and does not read as one: every suppressed case is listed in
  the gate summary next to the entry that caused it
  (`ledger::LedgerRuntime::bus_array_suppressions`,
  `crates/dss-core/tests/corpus_gate/ledger.rs:527`).

**Bus reads are group C on both transports, and their order is a contract.**
Every one goes straight to `Solution.NodeV` (`CAPI_Alt.pas:2276` == r4133
`DDLL/DBus.pas:423`) and moves only `ActiveBusIndex`, so no bus read stales a
cached `Iterminal` or is staled by one. What
`the_bus_capture_reads_in_one_fixed_order_on_both_transports`
(`crates/dss-core/tests/corpus_gate.rs:709`) pins is therefore the agreement of
the two transports, not a staleness hazard: the same five per-bus quantities in
the same order on the capi and r4133 sides, the bus block after
`variables`/`eventlog`/`ctrlqueue` and before `all_properties` (which stays the
last read of the step on both), and no element-scoped read inside either bus
capture. G1.4c's four arms are group C for the same reason and extend that
contract to **sixteen** per-bus reads in one order across the three tables
(fifteen until G1.4b added `Bus.Distance` to `BUS_READ_ORDER`)
(`BUS_READ_ORDER` → `SEQ_VLL_READ_ORDER` → `SC_READ_ORDER`), asserted by
`the_sequence_and_line_to_line_capture_reads_in_one_fixed_order_on_both_transports`
(`corpus_gate.rs:984`), which also refuses the unguarded `bus_vll` /
`bus_pu_vll` accessors inside the r4133 capture. **Corrected 2026-09-06 (G1.4d):**
the chain is now **eighteen** per-bus reads across **four** tables — the fourth is
`AT_BUS_READ_ORDER`, the two at-bus lists, asserted to come after every entry of
the other three by
`the_at_bus_capture_reads_last_in_one_fixed_order_on_both_transports`.

**The bus sequence and line-to-line surface (`compare_bus`, live since G1.4c,
2026-09-05).** `Bus.SeqVoltages`, `Bus.CplxSeqVoltages`, `Bus.VLL` and
`Bus.puVLL`, read on the same per-bus walk, on the same flag (no new flag, no
force rule and no `population.lock.json` move — the lock fingerprints the
*manifest* flag and no case sets `compare_bus`). This is the first WP-G1 surface
where the port, capi 0.14.5 and r4133 give **three different answers on the same
bus**, so the whole surface is about *which* answer is right and how the other
two are witnessed. Port side: `Dss::all_bus_voltages`
(`crates/dss-core/src/exec/view.rs:2327`), whose `bus_seq_voltages`
(`view.rs:1095`) and `bus_line_to_line` (`view.rs:1124`) publish the port's own
semantics; comparator: `harness::compare_bus_seq_and_vll`
(`crates/dss-core/tests/harness/mod.rs:14580`). Five things about it:

* **The port answers what the quantity means (S-SEQ, S-VLL).** Symmetrical
  components exist only with three phase voltages, so `SeqVoltages` /
  `CplxSeqVoltages` are published **iff the bus carries nodes 1, 2 and 3** —
  r4133's own comment says "Signify seq voltages n/A for less then 3 phases"
  (`DDLL/DBus.pas:299`) while its code tests the node *count* (`<> 3`), and both
  oracles substitute **ground** for a missing phase (`Find(i) = 0` ⇒ `NodeV[0]`,
  `DBus.pas:305` == `CAPI_Alt.pas:2190`). And `VLL` / `puVLL` are the
  line-to-line voltages over the phase nodes the bus actually carries (three
  pairs, the single pair, or nothing below two phases) — which is the pairing
  ORDER r4133's own report path uses (`Common/ShowResults.pas:193-194` wraps the
  phase number *before* `FindIdx`; that path is not S-VLL on every bus, since it
  still pairs against ground where a phase is missing), while both API arms poll
  `FindIdx(jj)` **before** the `jj > 3` ⇒ `jj := 1` wrap (`DBus.pas:575-584` ==
  `CAPI_Alt.pas:2500-2523`) and so pair phase 3 with node 4, or a node with
  itself. Behaviour contradicts stated intent inside one engine, so per **D4** /
  **D8** / **D21** the port computes the correct value and reproduces neither
  walk. The *report* paths still carry upstream's conventions: `report/show/`'s
  wrap-first pairing is r4133's own correct order, while the ground substitution
  in `report/export/seq_voltages.rs` is the defect and is **not** protected by
  golden bytes (measured 2026-09-05: every voltage-report golden runs
  `IEEE13Nodeckt.dss`, whose buses carry only node numbers 1-3, so the
  substituting branch is never reached there). What an export should print
  instead is a report-semantics decision outside WP-G1, tracked as
  `ORPHANED_GAPS.md` §1.19; the split between the report convention and the API
  semantics is deliberate and both halves are pinned.
* **Nothing is excluded; the oracle's own walk is asserted.** Rather than
  suppressing the divergent buses, the comparator classifies each bus **from its
  node set alone** and closes the divergent classes with a *positive* assertion
  `oracle == upstream_walk(port state)` (the **D15** / **D16** shape):
  `seq_channel_declines` (`mod.rs:14212`) is each channel's own decline rule
  (capi `n < 3` after its `Nvalues > 3` clamp, r4133 `n != 3` with no clamp —
  `DBus.pas:298`), asserted in **both** directions against the `-1` sentinel, and
  `upstream_vll` (`mod.rs:14256`) is a literal transcription of the two Pascal
  pairing loops, applied to the **port's own** node voltages. Result: **0 ledger
  rows**, and no bus left unwitnessed on either channel — a bus whose class the
  comparator cannot name fails the run.
* **The r4133 bridge refuses `VLL`/`puVLL` on the buses where the DLL would
  hang.** Not a mask: §"The r4133 bridge" below carries the predicate, the
  register and the measured TIMEOUT. The harness replay and the bridge predicate
  are two independent transcriptions of the same loop, and each one's verdict is
  asserted against the other's, so a refusal on one side alone reds.
* **Four run-wide populations, fail-on-stale in both directions.** The gate
  epilogue prints `corpus_gate seq/vll:` and asserts, as `(walks, buses)`:
  `R4133_SEQ_SENTINEL_POPULATION` = (10, 129) (`mod.rs:14429`, r4133 declining a
  `> 3`-node bus capi answers), `SEQ_GROUND_SUBSTITUTION_POPULATION` = (4, 54)
  (`mod.rs:14435`, both oracles transforming a fabricated 0 V phase),
  `VLL_UPSTREAM_PAIRING_DECLINES` = (16, 196) (`mod.rs:14440`, the pairing walk
  differing from the port's) and `R4133_VLL_HANG_POPULATION` = (2, 12)
  (`mod.rs:14444`, the refused calls) — `assert_seq_vll_populations`
  (`mod.rs:14499`), silent under `DSS_GATE_ONLY`, both directions pinned offline.
  A drop *or* a growth is a review, exactly as for `SC_STUDY_POPULATION`. Note
  `modes:makeposseq/makeposseq_gic.dss` carries none of them: `makeposseq` leaves
  its buses with one node each.
* **One tolerance constant, `SEQ_C012` = 5.229590094302253e-10** — r4133's own
  truncated `sin60` in `Ap2s`, on sequence rows 1 and 2 only; every other band on
  this surface is the image of the already-calibrated node-voltage band. It is
  the constant G1.3b derives for the element sequence surface: the 2026-09-06
  landing deduped the two to ONE definition (D21), reconciled by the analytic
  ceiling and kept at the tight row sum, never at the `5.30e-10` this sub-step
  had rounded it up to. The derivations, the ceiling and the measured worst ratio
  are in `tests/TOLERANCE_NOTES.md` §"Bus sequence and line-to-line voltages"
  and §G1.3b.

**The short-circuit surface (`compare_zsc`, live since G1.5, 2026-09-05).**
`Zsc1`, `Zsc0`, `ZscMatrix`, `YscMatrix`, `Isc` and `Voc`, per bus, appended to
the *same* per-bus walk `compare_bus` already runs — the flag implies
`compare_bus` and the implication is asserted three times (the request builder
and a loud refusal in each transport), never written as an `||`. The port side
is `Dss::all_bus_short_circuit` (`crates/dss-core/src/exec/view.rs:2307`), the
comparator is `harness::compare_bus_short_circuit`
(`crates/dss-core/tests/harness/mod.rs:12895`). Five things about it are worth
knowing:

* **The gate reads what the deck's own solve populated; it never runs a study.**
  `Zsc`/`Ysc` exist only after a fault study or a `ZscRefresh`, so a gate that
  refreshed them would be self-fulfilling on the great majority of the 443
  forced cases, whose decks run none (five run one). The capture-order test
  asserts that the short-circuit segment calls no refresh on either transport
  (`the_short_circuit_capture_reads_in_one_fixed_order_on_both_transports`,
  `crates/dss-core/tests/corpus_gate.rs:870`), and the comparator's **first**
  assertion is the discrete "study ran" bit, before any number.
* **A third ordering convention.** These arrays are indexed by the bus's
  *internal* (insertion) node index — `for i … for j … Zsc.GetElement(i, j)`,
  row-major (r4133 `DDLL/DBus.pas:445-450` == capi `CAPI_Alt.pas:2318-2330`) —
  **not** by ascending node number like `compare_bus`'s arrays. `CMatrix` stores
  column-major, so the flatten has to be an explicit `(i, j)` walk. The
  convention has two live witnesses: `modes:faultstudy/faultstudy_micro.dss`
  (a `.2.1.3` bus with a 1-phase 5 Ω shunt on its *first* node) at the `micro`
  band and `Run_NEV` at the `feeder` band — sorting the arrays by node number
  moves a diagonal entry by 1.35 Ω / 0.67 Ω, six orders above any band.
* **The two channels publish different "no matrix" sentinels, normalized at the
  comparator.** capi returns **1** double (`CAPI_Utils.pas:212-221`'s
  `DefaultResult` under `DSS_CAPI_COM_DEFAULTS`, `CAPI_SC_SENTINEL_LEN`,
  `crates/dss-core/tests/harness/mod.rs:12657`), r4133 **2** (the
  `setlength(…,1); [0] := CZero` prelude at `DDLL/DBus.pas:433-434`,
  `R4133_SC_SENTINEL_LEN`, `mod.rs:12689`); at a **0-node** bus the same split
  hits `Isc`/`Voc` (capi 0 doubles, r4133 2 — `AllocMem`'s non-nil 0-byte block
  vs `Reallocmem`'s free, capi `Common/Bus.pas:250-256` vs r4133
  `Common/Bus.pas:246-260`). Both are shape differences of an empty quantity, so
  they are normalized **per channel** (never "any short array is a sentinel")
  and pinned by `the_two_channels_publish_different_zsc_sentinels` — 0 ledger
  rows. The one place the shapes genuinely collide — r4133 at a 1-node bus,
  where a real 1×1 `Zsc` and the sentinel are both 2 doubles — is closed
  *positively*: with no matrix on the port side the oracle's pair must **be**
  `CZero`.
* **The bands add no tolerance constant.** `v_*` for `Zsc1`/`Zsc0`/`ZscMatrix`
  (a `Y·V = e_i` solve at exactly 1 A, read as ohms) and `Voc` (a copy of
  `NodeV`), `y_*` for `YscMatrix` (`= Zsc⁻¹`), `i_*` for `Isc` (`= Ysc·Voc`);
  the derivations, the dense-inversion conditioning argument and the measured
  headroom are in `tests/TOLERANCE_NOTES.md` §"Short-circuit surface". Measured
  worst over the whole forced population: **0.61** of the allowed band (`Voc` at
  `IEEE123Master-SC` bus `610`, |V| = 277 V — ~1.6× headroom at the surface's
  tightest point; the worst over the five impedance/current arms alone is 0.42,
  `Zsc0` at `ieee37_SC_Currents` bus `775`), and the two conditioning outliers
  (`IEEE123Master-SC:610`, κ = 1.10e8; `Run_NEV:tertiary`, κ = 9.52e6) land
  **below** the predicted `κ·u·‖Ysc‖∞` floor. **0** new ledger entries on either
  channel.
* **The non-trivial half is fail-on-stale** (G1.5 audit settlement). The
  comparator's content gate is `port_ran == oracle_ran`, which is equally true
  when NEITHER side ran a study — so a deck that stopped solving one would leave
  the whole surface green over sentinels and zeros, invisible to
  `population.lock.json` (which fingerprints the manifest flag) and to
  `MODES_REQUIRED` (the deck path). `harness::compare_bus_short_circuit`
  therefore returns how many buses it walked a full `n×n` matrix on, the runner
  records it (`harness::record_sc_study_compare`, the gate's only call site —
  the harness' own drives are deliberately not counted), and the gate epilogue
  prints `corpus_gate short-circuit: …` and asserts
  `SC_STUDY_POPULATION = (10, 646)` **exactly**: five study decks
  (`IEEE123Master-SC`, `ieee34Mod2_SC_Case_II`, `ieee37_SC_Currents`,
  `NEVTestCase/Run_NEV`, `modes:faultstudy/faultstudy_micro`) × two channels,
  323 buses each way. It fails on a drop AND on a growth, is silent under
  `DSS_GATE_ONLY`, and both directions are pinned offline in `corpus_gate.rs`.
  The two transports are also compared **to each other** on all six arms of the
  micro deck, at twice the tier band
  (`the_two_transports_agree_on_the_short_circuit_capture_of_a_gated_both_case`,
  **D2**).
* **The D11(2) suppression extends here, narrowly.** On a case whose `voltages`
  field is ledger-excluded deck-wide, the surface drops the `Voc` and `Isc`
  *values* — and nothing else: `Zsc1`/`Zsc0`/`ZscMatrix`/`YscMatrix` stay
  compared (they are functions of `Y` alone, not of `NodeV`), as do the study
  bit and every array length. Three negative drives hold that line
  (`the_voltage_exclusion_drops_only_the_voc_and_isc_values`,
  `the_voltage_exclusion_still_pins_zsc_ysc_and_the_lengths`), and the suppressed
  cases are printed beside the causing entry exactly as for `compare_bus`.

Like `compare_bus`, no *vendored* case sets the flag: `force_zsc`
(`crates/dss-core/tests/corpus_gate/scheduler.rs:709`) turns it on for every
live non-`large*` case, guarded by the pinned `FORCED_ZSC_POPULATION`
(`scheduler.rs:723`) and its oracle-free re-derivation. The one hand-set row is
the new `modes/faultstudy` sub-family's `faultstudy_micro.dss` — the corpus's
only short-circuit deck at the `micro` band (the four vendored ones —
`IEEE123Master-SC`, `ieee34Mod2_SC_Case_II`, `Run_NEV` and `ieee37_SC_Currents`,
which spells it `solve mode=f`, so `grep faultstudy` misses it — are all
`kind: feeder`) — so `population.lock.json` fingerprints the surface on it
(`zsc=1`) and any later narrowing shows up in the lock.

**The bus distance surface (rides `compare_bus`, live since G1.4b, 2026-09-05).**
`Bus.Distance`, `Circuit.AllBusDistances` and `Circuit.AllNodeDistances` — three
views of the ONE field `TDSSBus.DistFromMeter` (r4133 `DDLL/DBus.pas:122-128`
`BUSF` 5, `DDLL/DCircuit.pas:566-580` `CircuitV` 12 and `:582-604` `CircuitV` 13;
capi `CAPI/CAPI_Bus.pas:419-427` → `CAPI/CAPI_Alt.pas:2071-2074`,
`CAPI/CAPI_Circuit.pas:671-688` and `:697-722`). The port side is
`BusVoltageView::distance` / `Dss::all_bus_distances` / `Dss::all_node_distances`
(`crates/dss-core/src/exec/view.rs`, all three read **by reference** off the zone
accumulator, so the API surface and the profile report cannot disagree); the
comparator is `harness::compare_bus_distances`. Four things about it are worth
knowing:

* **It rides `compare_bus` — no flag of its own, no force rule, no lock *flag* cell** (the one `population.lock.json` cell that moved is a `ledger=` digest).
  The three reads join the per-bus walk `compare_bus` already pays for, so the
  population is that flag's 443 forced live non-`large` cases.
* **It is compared EXACTLY (`rel = abs = 0`) and takes no `Tolerances` at all.**
  `DistFromMeter` is a zone-build output, not a solve output: a running sum of
  `len · ConvertLineUnits(units, UNITS_KM)` over the same breadth-first walk on
  all three engines, never touching the solver. Measured bit-identical
  port ↔ capi ↔ r4133 on ten decks including a `units=miles` one — which also
  settles that the shipped r4133 DLL uses `Version8/Source/Shared/LineUnits.pas:81`
  (`1609.344`), not the `1609.3` of the two deprecated `LineUnits.pas` copies in
  the same tree. Derivation: `tests/TOLERANCE_NOTES.md` §"Bus distance surface".
  A measured upstream divergence is therefore **never** absorbed by a band — it is
  excluded bus by bus through the `distance` ledger field and pinned.
* **`DISTANCE_POPULATION = (867, 79_137)` is the fail-on-stale.** Gating compares
  that carried at least one non-zero distance, and the buses that carried one,
  re-derived on every run, asserted in **both** directions and printed on the
  gate's own `corpus_gate distance:` line (identical in both lanes). Without it a
  regression that stopped the zone walk writing distances at all would leave the
  ~370 meterless cases green, because both sides would then report the meterless
  `0.0`; `population.lock.json` cannot see it either, since it fingerprints the
  manifest FLAG and this surface sets none. (For scale, not for gating: a static
  scan following `Redirect`/`Compile` finds **70** of the 443 forced cases
  instantiating an EnergyMeter. The R part's "40 metered cases" was an undercount
  — its probe reached only 369 of the 442 cases it meant to cover.)
* **It is the live observable of the `MakeBusList` zone fix (D9).** The two decks
  that issue `MakeBusList` after defining an EnergyMeter report all-zero distances
  without it, so this comparison would be green over nothing;
  `the_make_bus_list_decks_report_the_zone_distances_both_oracles_measure`
  (`corpus_gate.rs`) pins their 20 distance literals — 9 of them non-zero — against both oracles.

**`modes:reduce/midi_reduce.dss` stays `capi_v0145`-gated, and a deck whose
`Reduce` RENAMES an element cannot be gated on r4133 (G1.4b, coordinator decision
D29 step 3).** The one distance divergence on the corpus is capi's: `DoReduceDefault`
loses the merged lines' `LengthUnits` (capi `src/PDElements/Line.pas:1806-1817` vs
r4133 `:1794-1796`), so capi consumes `length=4 kft` as 4 **km** —
`ConvertLineUnits` returns `1.0` whenever either side is `UNITS_NONE`
(`Shared/LineUnits.pas:110-115`) — and buses `l2e`/`l3e`/`l9e` read
`5.524`/`6.1335999999999995`/`13.145000000000003` km against the port's and
r4133's `2.7432`/`3.3528000000000002`/`10.364200000000004`. D29 step 1 (re-gate
the deck on r4133) was **measured first** and refused: r4133's `TLineObj.MergeWith`
renames the surviving object in place (`Version8/Source/PDElements/Line.pas:1684`,
right after the two `UpdateControlElements` calls at `:1682-1683` that DO re-point
the control links) without updating `TDSSCircuit.DeviceList`, so `SetElementActive`
(`Common/Circuit.pas:2195-2214`) finds nothing, leaves `ActiveCktElement` where it
was and the DDLL silently captures **another element** — measured cursor by cursor
(`Line.l2a~l2b` returns whatever element was active before it), and reported as
`-1` by `DDLL/DCircuit.pas:239-246`. capi 0.14.5 does not share it. Re-gating would
have ledgered a mis-addressed capture instead of a divergence (reported upstream as
`investigations/to_opendss/68-mergewith-rename-leaves-devicelist-stale.md`), so the deck stays
capi-gated and the divergence is excluded by the single `distance` entry
`reduce-merge-units-lost-midi-capi-distance` (`cause_ref:
line-merge-length-units-reset`, three `name_re` bus scopes), pinned by
`the_reduced_midi_deck_reports_the_merged_lines_kft_distances`. **Rule:** until
upstream fixes the rename, no deck whose reduce merges (and therefore renames) an
element may be flipped to `engines: "r4133"` — today's corpus has none gated that
way, measured on the live DLL over `reduce_dangling` / `reduce_laterals` /
`reduce_breakloop` / `reduce_remove`.

**That rule is machine-checked since the G1.4b audit settlement (2026-09-06).**
`no_r4133_gated_case_reduces_by_merging` (`corpus_gate/manifest.rs`, the
`no_capi_gated_case_instantiates_a_gictransformer` shape) pins a corpus-wide
census of the decks whose `Reduce` runs a MERGING strategy — r4133 picks the
strategy off the option's FIRST LETTER (`Executive/ExecHelper.pas:3089-3129`),
and `D`/`M`/`S` plus the empty/unknown fall-through reach `TLineObj.MergeWith`
(`Meters/ReduceAlgs.pas:54`/`:214`/`:258`/`:319`/`:361`) while `B`/`E`/`L` never
do — and walks the manifest so every gated case naming one declares
`engines: "capi_v0145"`; its non-vacuity drive is
`the_reduce_merge_channel_guard_refuses_an_r4133_gated_deck`. The same
settlement gave the comparator its committed offline drives
(`harness::bus_distance_comparator_tests` — positive control
`compare_bus_distances_accepts_the_engines_own_surface` plus nine negative rails,
replacing the plan-sanctioned scratch corruption), pinned the two transports
against each other on a METERED `both` case
(`the_two_transports_agree_on_the_bus_distances_of_a_metered_both_case` —
`line_asym.dss` has no meter, so the bus-capture check compares `0.0` with
`0.0` there), added the population guard's growth direction
(`the_distance_guard_is_silent_on_the_measured_population` /
`the_distance_guard_fires_when_a_deck_stops_building_its_meter_zone` /
`the_distance_guard_fires_when_a_metered_case_arrives`) and registered every
G1.4b name in `G1_4B_PINS` (`oracle_parity_cfg_gate.rs`, the `G1_9_PINS`
precedent), including the capture-order claim
`the_distance_surface_is_order_free_in_the_mode_table` and the four engine-side
identities `bus_distance_is_the_zone_walk_accumulator`,
`a_meterless_circuit_has_no_distances`, `all_bus_distances_is_the_bus_list_order`
and `all_node_distances_repeats_each_bus_value_per_node` (`exec/view.rs`).

**The bus at-bus surface (rides `compare_bus`, live since G1.4d, 2026-09-06).**
`Bus.AllPCEatBus` and `Bus.AllPDEatBus` — the qualified names of the
power-conversion / power-delivery elements at a bus (r4133 `DDLL/DBus.pas:840-866`
`BUSV` 18 and `:867-898` `BUSV` 19, over `Common/Circuit.pas:1540-1583` /
`:1493-1536`; capi `CAPI/CAPI_Bus.pas:773-788` / `:790-805` over
`Common/Circuit.pas:1797-1870` / `:1712-1794`). Port side:
`Dss::all_bus_elements` / `Dss::bus_elements` → `BusElementsView`
(`crates/dss-core/src/exec/view.rs`); comparator `harness::compare_bus_at_bus`.
It is the WP-G1 surface where **the two oracles answer different questions and
each contradicts its own stated intent** (coordinator decision D26, the D4 chain),
so five things about it are worth knowing:

* **The port answers neither oracle's question — it answers the physically
  correct one (S4).** `AllPDEatBus` = a PD-class element (r4133's
  `InheritsFrom(TPDClass)` set) with **any** terminal whose bus name is this bus,
  under r4133's own `bus1 <> bus2` shunt filter; `AllPCEatBus` = a PC-class
  element (plus `Capacitor`/`Reactor` by name, `Circuit.pas:1559`) whose terminal
  1 names this bus; `Enabled` is never consulted, by either oracle or here. The
  class sets are `ElemKind::is_power_delivery` / `is_power_conversion` — **not**
  the port's `pd_elements`/`pc_elements` lists, which answer a different question
  (`Fault` is on neither; `Capacitor`/`Reactor` only on the PD one), pinned by
  `the_two_class_predicates_are_the_upstream_class_sets`,
  `at_bus_lists_follow_the_s4_rule` and
  `bus_elements_answers_one_bus_and_agrees_with_the_sweep` — the single-bus
  accessor `Dss::bus_elements` runs the same whole-circuit builder and keeps one
  row, so it can never drift from the sweep (`src/exec/tests/bus_elements.rs`).
* **Each channel is closed by a POSITIVE mechanism assertion, not an exclusion —
  0 ledger rows.** The port publishes the raw attachment facts beside its answer
  (`BusAttachment`: `by_name`, `by_node_ref`, `series`, `enabled`, per element per
  bus — `attachments_publish_the_raw_terminal_facts_both_upstream_walks_read`),
  and the comparator requires each oracle's reply to EQUAL that oracle's own walk
  replayed over those facts (case-insensitive sets with cardinality): r4133's name
  test over terminals 1/2 (`Circuit.pas:1520-1522`, `:1566-1567`), capi's node-ref
  fast path over terminals 1..3 / terminal 1 (`:1746-1767`, `:1833-1852`). This is
  the D15/D16/D21 shape — where the engines disagree, the disagreement itself is
  asserted to be exactly the upstream rule applied to our state. The comparator's
  committed offline drives are
  `compare_bus_at_bus_accepts_each_channels_own_walk_and_counts_the_divergence`
  (positive control), `each_channel_is_held_to_its_own_walk_not_the_other_ones`
  (feeding each channel the other one's answer must red — the drive a naive
  "port == oracle" comparator would pass) and
  `the_capi_walk_takes_its_name_test_fallback_on_a_node_less_bus` (the node-less
  arm capi selects when `SetLength(nodes, 0)` leaves `NIL`, on a fixture of its
  own so the arm's coverage does not depend on one corpus deck), in
  `harness::bus_at_bus_comparator_tests`.
* **One direction is the port's alone and is asserted per bus:** each oracle's
  walk is a PROJECTION of the same attachment facts, so an element the port
  silently dropped from its own list is missing from both sides of the mechanism
  assertion and every case still passes (measured). `assert_port_at_bus_is_s4`
  therefore recomputes S4 from the raw facts and requires exactly the published
  list, driven offline by
  `a_port_at_bus_list_that_drops_an_s4_element_reds_per_case` (G1.4d audit
  settlement).
* **Four run-wide fail-on-stale populations carry what is not equal**, printed on
  the gate's own `corpus_gate at-bus:` line, asserted exactly (a drop AND a growth
  fail) and identical in both lanes: `PDE_TERMINAL3_DECLINES = (279, 279)` (r4133
  misses every winding past the second, although its header promises *"all PDE
  connected to the bus"*, `Circuit.pas:1490-1492`), `CAPI_NODEREF_DROPS =
  (18, 147)` and `CAPI_NODEREF_ADDS = (8, 11)` (capi answers from a `TermNodeRef`
  that `ReProcessBusDefs` refreshes for enabled elements only — capi
  `Circuit.pas:2196-2202` — so a disabled element is dropped from the bus it names
  and reported at the bus that inherited its stale reference), and
  `PCE_AT_BUS_DECLINES = (0, 0)` (the PC criterion IS r4133's, so this list cannot
  diverge; witnessed in the growth direction by a scratch drive adding one disabled
  `Load`, since the corpus has no disabled PC element). Guards:
  `the_at_bus_guard_is_silent_on_the_measured_populations`,
  `the_at_bus_guard_fires_when_a_deck_stops_carrying_its_class`,
  `the_at_bus_guard_fires_when_a_pce_divergence_appears`,
  `the_at_bus_guard_fires_when_the_terminal3_class_grows` and
  `the_at_bus_guard_fires_when_a_stale_node_ref_stops_naming_an_element` (one
  drive per tuple and direction — G1.4d audit settlement). The four are not
  symmetric across channels: `PCE_AT_BUS_DECLINES` can only be reached from
  `CapiV0145`, since r4133's replayed PCE predicate is the very property
  `assert_port_at_bus_is_s4` asserts of the port's own list, and
  `CAPI_NODEREF_DROPS` would also absorb capi's terminal-window arm
  (`Min(High(Terminals), 2)`, `Circuit.pas:1750`) if the corpus ever grew a
  4-winding transformer — re-measured 2026-09-06: **0** decks declare
  `windings`/`wdg` ≥ 4, and one would earn its own deck and counter, never a
  silent merge. All three upstream
  behaviours are reported at `investigations/to_opendss/69-…`, `70-…` and `71-…`.
  **Note the corollary:** "disabled elements are dropped" is *not* the capi rule
  (measured: of 223 (disabled element, own bus) pairs, 15 ARE listed), which
  is why the capi channel is an equality over our own node references and not a
  two-sided approximation.
* **The wire conventions differ and are asserted per channel, in both
  directions.** r4133 emits `['None']` for an empty answer and nothing else — the
  DDLL filters `getP*atBus`' trailing empty slot and re-emits the lone sentinel
  (`DBus.pas:853`, `:862-863`, `:880`, `:894-895`). The capi channel emits
  `['None']` too **and** one trailing `''` on every non-empty reply — but both are
  the pinned **dss-python facade's**, not dss_capi's: the C API passes
  `useNone = False` (`CAPI_Bus.pas:784`, `:801`) and returns `[]`, and
  `dss/IBus.py` substitutes `['None']` and appends `''` *"for full compatibility
  with COM"*. Neither transport normalizes; the comparator asserts each convention
  and only then strips it. Both are pinned on one deck by
  `the_makeposseq_xfmr_at_bus_wires_are_each_channels_own_walk`, beside
  `the_makeposseq_xfmr_deck_reports_the_at_bus_lists_the_port_computes`, which
  holds the port's own answer and the attachment facts behind all three divergence
  classes (`modes:makeposseq/makeposseq_xfmr.dss` carries them in one deck).
* **The pair is read LAST in the per-bus walk on both transports**, asserted from
  the two capture sources by
  `the_at_bus_capture_reads_last_in_one_fixed_order_on_both_transports` — see the
  mode-capability record below for why.

**Four bus quantities are stronger than fastdss, not at parity.** The fastdss
harness drops `puVLL`, `VLL`, `AllPCEatBus` and `AllPDEatBus` from
`IBus._columns` under `COM_VLL_BROKEN` in the Oddie configuration
(`origin/fastdss` `tests/save_outputs.py:197-209`). `VLL`/`puVLL` have been live
here since G1.4c (2026-09-05) and `AllPCEatBus`/`AllPDEatBus` since **G1.4d**
(2026-09-06, split out of G1.4b by coordinator decision D26), so on those four
the gate compares **more** than the parity target does — never describe them as
fastdss parity.

**G1.7's topology surface is group C but is read after `all_properties`** on
both transports — only G1.8's incidence pair follows it (see below) — and
`crates/dss-core/tests/capture_order.rs` asserts both orderings from the two
capture sources. Its six rows (`NumLoops`,
`NumIsolatedBranches`, `NumIsolatedLoads`, `AllLoopedPairs`,
`AllIsolatedBranches`, `AllIsolatedLoads`) are order-free in themselves, but the
FIRST `Topology` read of a step is what BUILDS the memoized branch tree and
rewrites `Checked`/`IsIsolated`/`BusChecked` on every element on the way (r4133
`Common/Circuit.pas:2932-2950`, `:2937-2947`), and `TopologyI(1)`/`(2)` +
`TopologyV(1)`/`(2)` walk `PDElements`/`PCElements` `.First`/`.Next` to
exhaustion (`DDLL/DTopology.pas:75-94`, `:319-390`), leaving those cursors at the
end. Nothing in today's capture reads those flags; reading last makes that
independent of every future addition — the argument that put `all_properties`
last, applied to itself.

**The B16 parity gap is justified, and enforced from the sources.**
`ITopology` has eighteen members (nine `_columns` on `origin/fastdss:dss/ITopology.py:10-20`
plus nine cursor methods); the gate captures **six**. The other twelve —
`ActiveLevel`, `BranchName`, `ActiveBranch` and the nine cursor rows — each
reassign `ActiveCircuit.ActiveCktElement` (capi `CAPI/CAPI_Topology.pas:98-110`;
r4133 `DTopology.pas:29-54`, `:96-160`, `:170-186`), so reading one would change
which element the *same step's* per-element capture describes. This is a stronger
reason than "they are iteration cursors", and it is a test, not a comment:
`capture_order.rs` blanks comments, docstrings and string literals out of both
capture sources and then requires the set of `ITopology` members read on the capi
side and of `topology_*` bridge accessors called on the r4133 side to be exactly
those six — plus that `crates/dss-epri/src/dss.rs` **binds** only those six, so a
cursor row cannot even be reached. Both transports name all twelve forbidden rows
in their doc text on purpose (that is where the reason is written down), which is
why the check reads code and not prose; a synthetic pair of sources proves both
directions.

**Two shape normalizations, both transport-side.** (1) An empty list comes back
as the single token `NONE` on both channels — r4133 pre-seeds `TStr[0] := 'NONE'`
(`DTopology.pas:271-275`), capi answers `DefaultResult(…, 'NONE')`
(`CAPI_Utils.pas:115`) — and is decoded to `[]`, so an empty list compares as
empty and not as a phantom one-element list. (2) A **non-empty**
`AllIsolatedBranches`/`AllIsolatedLoads` on the capi channel carries exactly one
trailing `''` (`SetLength(Result, k + 1)`, `CAPI_Topology.pas:126-132`), which
r4133 filters at the source and which `AllLoopedPairs` never carries on either
channel (`k := -1`); exactly that one is dropped, and any other empty entry
**raises** in the transport rather than silently losing an element name. Measured
over the capi corpus replay (422 cases / 2 155 step-blocks): 134 `AllIsolatedBranches`
and 46 `AllIsolatedLoads` reads carry it — 100 % of the non-empty isolated lists,
over 65 and 21 distinct cases — against 0 of the 611 non-empty `AllLoopedPairs`
reads and 0 on r4133. The
comparator (`harness::topology::normalize_topo_names`) does not repeat the repair:
it asserts the capture is already at the fixpoint, so a transport that stops
normalizing fails there instead of comparing a phantom entry as if it were empty.

**Population.** `compare_topology` is forced on **every live non-`large` case** —
443 of the 522 live cases (312 `both`, 87 `r4133`, 44 `capi_v0145`; 442 =
311/87/44 before G1.5's `faultstudy_micro` deck reached it at the G1.5 merge, 441 =
310/87/44 before G1.6(i)'s `midi_relcalc` deck reached it at the G1.6(i) merge,
440 = 313/83/44 before G1.4a's D12/D14 corpus flips reached it at the G1.7 merge) — the
same population `force_properties` uses, which
`scheduler::the_topology_forcing_rule_is_every_live_non_large_case` re-derives
from the four manifests and pins as `FORCED_TOPOLOGY_POPULATION`, asserting the
equality rather than commenting it. Seven decks *also* declare the flag in their
manifests (`scheduler::TOPOLOGY_DECLARED_IN_MANIFEST`, one witness per gating
channel), because `population.lock.json` fingerprints the manifest flag and
cannot see scheduler-side forcing — the same blind spot the property surface's
lock has; deleting one of the seven reds twice. The surface is fully discrete
(counts and identifier lists compared with zero tolerance, names
case-insensitively, lists **ordered** because both sides are pointer-list walks in
creation order), so it introduces no floor anywhere; see
`tests/TOLERANCE_NOTES.md` §"G1.7 topology interface".

**The two topology settlements (G1.7, coordinator decisions D15/D16) — 0 ledger
rows, two pinned populations.** The port neither memoizes the branch tree nor
reproduces upstream's looped-pair dedup, so it differs from both oracles in two
measured, fully explained ways; neither is excluded. (1) *Stale tree.* Both
oracles memoize `Branch_List` (r4133 `Common/Circuit.pas:2932-2950`, freed only
in `Destroy` `:703` and `DoResetMeterZones` `:2308`) and invalidate nothing when a
conductor opens, so after the first Relay/Recloser/SwtControl operation their four
isolation answers come from the step-0 tree. `harness::topology::compare_topology`
compares those four fields at step 0 always, and at step `k > 0` while the port's
own isolation topology still equals its step-0 one; where it has moved it asserts
`oracle(k) == port(step 0)` — the memoization contract — instead of comparing
against the fresh answer. `num_loops` and the pair list stay compared at every
step. (2) *Window dedup.* Upstream scans its flat pair buffer in overlapping
windows (`i := i + 1`, r4133 `DDLL/DTopology.pas:286-296`, capi
`CAPI_Topology.pas:180-190`) and so drops a genuinely new pair that coincides with
a straddling window; the comparator asserts
`oracle.looped_pairs == window_dedup(port candidates)` (the model transcribed from
the Pascal indices, applied to `TopologyView::looped_pair_candidates`), while the
port's own `looped_pairs` is held to `per_pair_dedup` of the same candidates so it
cannot drift. Both declines are counted per `(case, step)` on every run and pinned
in `corpus_gate/scheduler.rs` beside the `FORCED_*` constants —
`TOPOLOGY_STALE_DECLINES = (16, 135)` and `LOOPED_PAIR_WINDOW_DECLINES = (8, 96)`,
fail-on-stale in **both** directions
(`assert_topology_declines_are_the_pinned_population`, called from the gate
epilogue; it prints the full per-case table, requires `compared > 0` once any live
case requests the surface, and is silent only under `DSS_GATE_ONLY` or while no
case requests it). The expected-value pins naming both numbers are
`topology_pins::topology_reads_a_freshly_built_tree` and
`topology_pins::looped_pairs_lose_the_straddling_window`; the upstream reports are
`investigations/to_opendss/59-topology-branchlist-stale-after-switching.md` and
`60-alllooped-pairs-window-scan-drops-pairs.md` (local only).
`LOOPED_PAIR_WINDOW_DECLINES` is a function of the forced population — the
LVTestCase and ckt24 feeders diverge the same way but are `kind=large*` and out of
the compare, so narrowing the population means re-measuring it.

**Two port gaps the topology surface exposed** (fixed in-part per CLAUDE.md's
"port gaps immediately", both inside G1.7's own surface commit): the topology
adjacency lists routed PD elements by `TPDElement.IsShunt` where both oracles use
the class-switched `IsShuntElement` (capi `Shared/CktTree.pas:522-528`, r4133
`Common/Utilities.pas:1262-1274`), which made every `GICTransformer` a shunt and
hid the loops it closes; and `CktElementData::set_nconds` forced a terminal
reallocation that Pascal's own `Set_NTerms` guard (`Common/CktElement.pas:349-361`,
`:386`) would have skipped, so a no-op `Phases=` re-set — what a second
`MakePosSequence` does — wiped every terminal's bus and node refs without raising
`BusNameRedefined`, leaving the model bus-unresolved. The topology walk was the
first compared surface that could see either.

**G1.8 — the incidence matrix and the Laplacian, read strictly last.**
`CalcIncMatrix` then `CalcLaplacian` are issued on both transports and the four flat
quantities (`Solution.IncMatrix`, `Laplacian`, `IncMatrixRows`, `IncMatrixCols`) read
back in that order, after G1.7's topology block and therefore last in the step. Two
independent reasons, both written into the capture doc comments. (i) On r4133 the pair
is not a pure read: `AddSeriesReac2IncMatrix` re-points `LastClassReferenced` and
`ActiveDSSClass` and then calls `ActiveDSSClass.First` (`Common/Solution.pas:3007-3010`),
which moves `ActiveCircuit.ActiveCktElement`, so it must not precede any per-element or
property read. The pinned capi 0.14.5 walks the reactors with a typed class iterator and
leaves `ActiveCktElement` alone, so the rule comes from the stronger channel and is
applied to both. (ii) It must follow the topology read that memoizes `Branch_List`, on
which G1.7's two censuses are defined. `crates/dss-core/tests/capture_order.rs` asserts
all of it from the two capture sources — the incidence call follows `topology`, which
follows `all_properties` (`capi_capture_reads_the_incidence_surface_last` and its r4133
twin); exactly those two commands are issued, in that order, and exactly those four
members are read (`the_incidence_capture_issues_calcincmatrix_then_calclaplacian`); with
a synthetic negative drive for each direction
(`the_incidence_gates_reject_a_swapped_or_early_capture`). The order *inside* the pair is
a contract, not a convention: r4133's `CalcLaplacian` has no `Assigned(IncMat)` guard
(`Executive/ExecCommands.pas:911-917`) and nil-derefs inside the DLL, while dss_capi
(`Executive/ExecCommands.pas:421-433`) and the port both raise 8877.

**The ordered builder and `BusLevels` are deliberately out of the live gate.**
`Calc_Inc_Matrix_Org` calls `GetTopology` (`Common/Solution.pas:3146`, `:3173`), which
would rebuild and re-memoize the branch tree G1.7's `TOPOLOGY_STALE_DECLINES` and
`LOOPED_PAIR_WINDOW_DECLINES` are defined on; and `BusLevels` can never be read on r4133
— `DSolution.pas:580-582` does `setlength(myIntArray, ArrSize)` and then
`for IMIdx := 0 to ArrSize`, a one-element heap overflow — so it sits on the bridge's
do-not-call register — the `BusLevels` row, `crates/dss-epri/src/modes.rs:309-312` —
and is refused before any FFI reaches the DLL. Neither is issued or read on either transport, asserted from the
source text by `capture_order.rs::neither_capture_calls_calcincmatrix_o_or_reads_buslevels`
(whose second half re-asserts the register row itself). That is also why
`GOLDEN_REBASE_PLAN.md` §G3.2c keeps the twenty `*_org_*` golden stems of
`tests/golden/inc_matrix/` — they are the only witness of the ordered builder, of
`BusLevels` and of the CSV writer `report/export/inc_matrix.rs`, and the writer and the
getter deliberately differ on `IncMatrixCols` — and deletes only the eight `*_flat_*`
ones, whose values are exactly what this surface now gates live.

**Three transport normalizations, all asserted, none of them in the comparator.**
(N1) capi over-allocates one cell for each integer array (`CAPI_Solution.pas:910`,
`:873`, both carrying the upstream "TODO: remove the +1"): `oracle_server.py` checks
that the trailing cell is 0, drops it, and refuses any length that is not `3*NZero + 1`
— measured 0 non-zero cells and 0 bad lengths over 1 733 steps, which is this sub-step's
kill criterion expressed in the transport. (N2) r4133 answers a nil or empty matrix with
the one-cell `[0]` sentinel (`DSolution.pas:544`, `:642`), decoded to the empty list;
every other length must be `3*NZero` — measured 104 sentinel steps of 1 756 and no third
shape. (N3) an empty name list is a single blank token on capi
(`CAPI_Solution.pas:961`) and the single token `None` on r4133 (`DSolution.pas:605`);
each is decoded to the empty list only where the engine can actually reach it, and any
other empty entry raises rather than silently losing a name. `harness::inc_matrix` does
not repeat the repairs — it asserts the capture is already at the fixpoint, so a
transport that stops normalizing fails there instead of comparing a phantom cell as
data. After N1-N3 the two channels were byte-identical on all four quantities across the
358 both-gated cases of the sub-step's own sweeps.

**Names are byte-identical by construction; the compare is case-insensitive anyway.**
Both oracles store bus names lowercased — `THashList.Add` keeps `LowerCase(S)` ("make
copy of whole string, lower case", r4133 `Shared/HashList.pas:268`, `:281`; capi
`HashList.pas:224`) — and a row label is a hardcoded capitalized class prefix plus the
element's already-lowercase `Name` (`Common/Solution.pas:3019` and its three siblings),
which is exactly the port's spelling. Measured on a probe circuit whose buses are
declared `SourceBus`, `BusUpper`, `MiXeD`: the oracle answers `sourcebus`, `busupper`,
`mixed` and rows `Line.l1`, `Line.l2`. The ASCII-case-insensitive equality in the
comparator is therefore belt-and-braces rather than load-bearing, and it is the only
slack: both lists are compared in **sequence** order, at zero tolerance, because both
sides are creation-order walks.

**Settlement S-INC — the reactor row cursor is asserted, not excluded (0 ledger rows).**
Both oracles advance the incidence row cursor for EVERY reactor: `inc(ActiveIncCell[0])`
sits at `Common/Solution.pas:3039`, outside the `:3015` emit guard, unlike the three
sibling walks (Lines `:2885`, Transformers `:2938`, series Capacitors `:2986`). A series
reactor that follows a skipped shunt one therefore carries a row index that does not
index `Inc_Mat_Rows` — contradicting the surface's own contract, since `Inc_Mat_Rows` is
the PD-element name per incidence row and is exported under the header
`B2N Incidence Matrix Row Names (PDElements)`. Per CLAUDE.md the port emits **dense**
rows (`solution/inc_matrix.rs::add_series_reactors`, fixed in its own commit ahead of the
surface) and the comparator re-derives upstream's numbering positively:
`remap(port.inc_matrix, upstream_row_index) == oracle.inc_matrix`, where
`IncMatrixView::upstream_row_index` maps the j-th reactor of the circuit's own reactor
list to `base + j`. The Laplacian arm compares **unmapped**, because the Laplacian is
blind to an empty row — measured on both channels: the same circuit declared shunt-first
and series-first returns the same thirty Laplacian integers in the same order while the
incidence row moves 3 to 2. The declining population is re-derived on every run and
pinned in both directions as `INC_UPSTREAM_ROW_DECLINES = (4 cases, 5 case-steps)`
(`harness::inc_matrix::assert_declines_are_the_pinned_population`, called from the gate
epilogue, silent only under `DSS_GATE_ONLY` or when the manifests request the surface on
neither channel — the arming predicate is read off the four manifests by
`scheduler::inc_matrix_requested_channels`, never off the run's own counters, so a
deleted or per-channel-narrowed `if c.compare_inc_matrix` block in the runner reds here
instead of silencing the surface with a green gate):
`NEVMASTER.DSS`, `Run_NEV.dss`, `asymmetric:reactor/reactor_asym.dss` and
`midi_reactor_asym.dss` steps 0 and 1 — 10 channel visits over 3 316 compared
`(case, step, channel)` triples. The expected-value pins naming both numbers are
`inc_matrix_pins::the_incidence_row_cursor_skips_a_shunt_reactor` (port row 2 against
oracle row 3 with three row names),
`inc_matrix_pins::the_row_cursor_settlement_holds_on_the_corpus_witness` and
`inc_matrix_pins::the_laplacian_is_blind_to_the_row_cursor`; the upstream report is
`investigations/to_opendss/63-incidence-row-cursor-counts-skipped-reactors.md` (local only).

**Two further defects of the same walk are reproduced on purpose, and registered.**
`GetBus(2)` on a one-terminal reactor returns a blank string, so the `.0` test that would
classify the reactor as *shunt* never fires, the bus search then misses and the column
falls back to the LAST bus of the list: `asymmetric:reactor/reactor_asym.dss`'s
`Reactor.rdel` gets an edge from `b3` to `b4` in a five-bus list — the port's dense
triples `(4,3,+1)` `(4,4,-1)`, which both oracles emit at row **5** because the same deck
also trips the row cursor above (`ASYM_INC` in `inc_matrix_pins.rs`; only the column pair
is channel-independent). And the reactor walk carries no `Enabled` test where its three
siblings do (r4133 `Common/Solution.pas:2862`, `:2916`, `:2964`), so a disabled series
reactor is still a row.
Both are *which element becomes a row* — a modelling decision, not an indexing one — so
they are left as they are for now, stated with both numbers by
`exec::tests::inc_matrix::a_one_terminal_reactor_becomes_a_phantom_branch_to_the_last_bus`
and `::a_disabled_series_reactor_is_still_a_row`, and registered as teardown candidates
in `GOLDEN_REBASE_PLAN.md` §WP-G2 with the same upstream report.

**Population, flag, lock and contamination.** `compare_inc_matrix` (`incm=`) is forced on
every live non-`large` case — the same 443 (312 `both`, 87 `r4133`, 44 `capi_v0145`) that
carry the property and topology surfaces, which
`scheduler::the_inc_matrix_forcing_rule_is_every_live_non_large_case` re-derives from the
four manifests and pins as `FORCED_INC_MATRIX_POPULATION`, asserting equality with both
sibling constants instead of commenting it. `large*` stays out on cost: 65 capi cases /
88 steps are 56 % of that channel's time in the pair and 57 % of its payload. Six decks
also *declare* the flag (`scheduler::INC_MATRIX_DECLARED_IN_MANIFEST`, all three gating
channels represented; one of them, `solvable_now:Test/CableParameters.dss`, has an EMPTY
`IncMat`, so the sentinel path is declared and not merely forced), because
`population.lock.json` fingerprints the manifest flag and cannot see scheduler-side
forcing — the same blind spot the property and topology surfaces have. The lock moved by
exactly six `incm=0` to `incm=1` tokens, and `population_lock` was red before the regen,
which is the proof the six declarations reach it. The surface is fully discrete, so it
introduces no floor anywhere; see `tests/TOLERANCE_NOTES.md` §G1.8. Contamination: the
whole 523-case manifest was dumped three ways — serial one-shot, persistent-parallel and
persistent-parallel-shuffled — and all 523 labels are bit-identical; no `write_gate_dump`
strip rule is owed for capi's `+1` cell, because the transport drops it before the
checkpoint exists.

**The G1.8 pins, by name** — registered in
`oracle_parity_cfg_gate.rs::the_g1_8_pins_the_docs_cite_exist_exactly_once`, so a rename
or a deletion reds here instead of leaving this section stale. Transport and getter
shape, in `crates/dss-core/tests/inc_matrix_pins.rs`:
`capi_incmatrix_carries_one_trailing_zero` and
`capi_and_r4133_incmatrix_lengths_differ_by_one` state the two raw lengths a later
cleanup of N1/N2 would have to move (IEEE13 `IncMatrix` 103 capi against 102 r4133,
`Laplacian` 139 against 138); `an_empty_incidence_matrix_reads_back_as_no_rows` states
the sentinel triple (capi `['']`, r4133 `['None']`, port `[]`);
`inc_matrix_cols_are_the_bus_list_when_unordered` states the getter branch (the sixteen
IEEE13 columns in `BusList` order against the `CalcIncMatrix_O` order, which first
differs at index 3); and `calclaplacian_without_calcincmatrix_raises_8877` states the
port's own guard. The r4133 twin named above is literally
`r4133_capture_reads_the_incidence_surface_last`, and its strictly-last rule is the
predicate `check_inc_matrix_last`; the dense-row fix is pinned in-engine
by `solution::inc_matrix::tests::the_reactor_row_cursor_advances_only_on_an_emitted_row`
and the six manifest declarations by
`scheduler::the_inc_matrix_surface_is_declared_on_every_gating_channel`. The capi
transport's own two refusals — the kill criterion (the `+1` cell must be 0, the length
`3·NZero + 1`) and the empty-name sentinel — live in Python, where no Rust test reaches
them, so they are gated from the source text by
`capture_order::the_capi_incidence_transport_refuses_a_shape_it_was_not_written_for`
(the r4133 twins have real unit tests in `crates/dss-epri/src/capture.rs`).

**The `Meters` reliability surface (G1.6(i), 2026-09-05).** `compare_reliability` is
the only compare flag that **drives a command**: on a flagged case the gate runs the
executive `RelCalc` once, on the **last** step, after the per-step error assert and
after `Text.Result`/`GlobalResult` has been read (the command overwrites it) and
before every capture of that checkpoint — on all three engines
(the `RelCalc` drive, `crates/dss-core/tests/corpus_gate/runner.rs:831`;
`tools/oracle/oracle_server.py:1307`, `Engine::relcalc`,
`crates/dss-epri/src/dss.rs:708`). *Once*, because `RelCalc` is **not idempotent**: a
second run re-accumulates `Bus.TotalMiles` (`13.825757575757578 →
22.348484848484844`, measured on both oracles and pinned in the port by
`relcalc_is_not_idempotent_and_the_gate_runs_it_once`). The payload therefore lives
on the last checkpoint only, and the runner asserts exactly that before it compares
anything. Port side: `Dss::meter_reliability` / `Dss::meter_totals`
(`crates/dss-core/src/exec/view.rs:3436` / `:3565`) — `&self` reads of solved state,
the reliability math itself untouched. Comparator: `harness::compare_reliability`
(`crates/dss-core/tests/harness/mod.rs:18027`). The flag is **manifest-set, never
forced** (six cases): "has an EnergyMeter" is not a manifest field, and forcing it
circuit-wide would fire `28724 No EnergyMeter Objects Defined` on ~340 meterless
decks — so there is no `FORCED_RELIABILITY_POPULATION` lock, deliberately. `large*`
decks stay out for the same cost reason `force_properties`/`force_pdelements` keep
them out, and since the G1.6(i) audit settlement that is an enforced guard, not just
a decision: `reliability_pins.rs::no_large_case_gates_the_reliability_surface` fails
if any `kind=large*` manifest row sets the flag. That is why the multi-meter
`Bus_Int_Duration` divergence keeps its existing witness (the
`export_busreliability_multimeter` golden and its G2.2a pin) and gains no live one.
Seven rules come with the surface.

* **Driving `RelCalc` is state-neutral, and the partition it rests on is what makes
  that claim falsifiable.** The command runs at the SAME point on all three engines and
  after every per-step assert, so each comparator of the last checkpoint sees
  post-calc state on both sides. What it is ALLOWED to move is exactly the
  reliability-derived state: EnergyMeter properties #19-23 (`SAIFI`, `SAIDI`,
  `CustInterrupts`, `SAIFIkW`, `CAIDI`) plus `TotalCustomers`/`NumSections`, the
  reliability halves of `PDElements.*` (`AccumulatedL`, `Lambda`, `TotalMiles`,
  `SectionID`, `Numcustomers`, `Totalcustomers`) and of `Bus.*` (G1.6(ii)'s columns),
  and `Meters.Totals` via `TotalizeMeters`. Anything else moving — node voltages, `Y`,
  element currents/powers, monitor channels, energy registers, the event log, the
  control queue — is an engine finding and a STOP, never a re-baseline. Nothing
  enforces the partition by construction; it is carried by the existing comparators,
  which all run after the drive on the six flagged cases in both lanes, and by
  `lane_diff` over the same state (G1.6(i) measured them unmoved before the surface
  was wired).
* **Errno 52902 is tolerated — and only it — on both transports.** A zone with no
  OCP device aborts the calc (r4133 `Meters/EnergyMeter.pas:2502`, capi `:2456`),
  which is a **compared observable**, not an error to swallow:
  `_RELCALC_TOLERATED_ERRNOS` (`tools/oracle/oracle_server.py:1126`) and
  `RELCALC_TOLERATED` (`crates/dss-epri/src/dss.rs:167`) are separate single-value
  scopes, every other errno (28724 included) still fails the case, and the comparator
  asserts abort symmetry on the **boolean and the message, never the count** — the
  port reports once per failing meter, dss-python once per command
  (`relcalc_abort_is_symmetric_on_a_zone_without_ocp`).
* **Read order inside the surface is contractual, and statically enforced.** Per
  meter, the non-section fields in `IMeters._columns` order; then
  `Meters.SetActiveSection(k)` before **every** section block (`ActiveSection` is a
  per-meter field the `First`/`Next` walk never resets — r4133
  `DDLL/DMeters.pas:254-264`); then `Meters.Totals` **last, after the walk**, because
  it re-runs `TotalizeMeters` (`Common/Circuit.pas:2520-2538`) and destroys the meter
  cursor — measured: a mid-walk read drops the second meter of a two-meter deck.
  `crates/dss-core/tests/reliability_pins.rs` reads both capture bodies as text and
  checks each transport against its own pinned 24-read sequence, the three rules over
  the sequence, the two transports against each other (the per-meter prefix
  element-by-element, the section block as a set — the two channels legitimately order
  the section getters differently, and a section field is a pure getter on the
  already-selected section) and the payload's slot between the meters and the PD
  elements, with a negative drive over corrupted copies of the real bodies. This
  surface is group **(C)** of the capture-order partition above: no read of it goes
  through `GetCurrents`, so it neither imposes nor inherits an element order.
* **Exact, with three cells banded from existing tiers.** Every reliability value is
  compared at `rel = abs = 0`: the two independent oracle engines return bit-identical
  doubles for the whole payload on the flagged decks, so a gap is an order bug, not a
  floor (derivations in `tests/TOLERANCE_NOTES.md`). The exceptions are
  `Meters.Totals`, which is `Σ registers·Mask` and rides the **energy** tier its own
  summands ride — so a `Totals` regression demo must exceed 1e-4 relative to be a demo
  at all, so the 67 slots are not part of the exactness headline — and
  `calc_current`/`alloc_factors`, which ride the **current** tier and its image under
  `SensorCurrent/|I|`. That image is **band-limited from below**: a metered current
  inside its own `i_abs` yields no band at all and the comparator then fails loudly
  for triage instead of admitting the cell (the `SeqCurrents %I` precedent;
  unit-tested over the three regimes). `AverageRepairTime` is an unguarded division on all
  three engines, so `NaN`/`±inf` agreement counts as agreement while `NaN` against a
  finite number fails.
* **An exact float compare needs `float_roundtrip`** (decisions **D11/D18**). The
  gate decodes oracle JSON with `serde_json`'s `float_roundtrip` feature; without it a
  17-significant-digit token decodes up to 1 ULP off and every exact compare on this
  surface — and on the `PDElements` one — reports phantom gaps (measured: the wire
  tokens round-trip to the port's own f64 15/15, the non-`float_roundtrip` parser
  misparses 9/9).
* **The zone lists gain an ordered arm, unconditionally.** `compare_meter`'s
  `cmp_members` set compare is deliberately order-independent and is untouched;
  `compare_reliability` layers length → case-insensitive membership →
  element-by-element sequence on top, each with its own message (a set compare can see
  neither a reordering nor a multiplicity error — `midi_energymeter` legitimately
  lists `Transformer.t8` twice in its ends list). Both oracles walk
  `BranchList.First`/`GoForward` (capi `CAPI/CAPI_Meters.pas:589-595`, r4133
  `DDLL/DMeters.pas:706-733`) and the port pushes `sequence_list()` from the same tree
  walk; the order was measured identical on all six flagged cases before the arm was
  turned on, and reversing the port's list reds 12 cells on both channels while no
  membership assertion fires.
* **`RELIABILITY_SKIP_FIELDS` is fail-on-stale, and not a permanent hole.**
  `Meters.CalcCurrent`/`AllocFactors` are read out of uninitialized memory on both
  oracles until a deck runs `AllocateLoads` (`TMeterElement.AllocateSensorArrays`
  ReallocMems both arrays without zeroing, r4133 `Meters/MeterElement.pas:45-52`; only
  `CalcAllocationFactors` `:54-72` writes them, and its sole driver is
  `TExecHelper.DoAllocateLoadsCmd`, `Executive/ExecHelper.pas:2624-2683`) — measured
  denormal garbage that changes across processes, hence excluded per (channel, case,
  field) in `RELIABILITY_SKIP_FIELDS` (`crates/dss-core/tests/harness/mod.rs:17763`),
  never enveloped, each row carrying its citation and a pin that a register test
  requires to name a real `#[test]`. The corpus's only `AllocateLoads` deck,
  `tests/corpus/controls/energymeter/midi_relcalc.dss` (`both`, three sections), is
  where both fields **are** compared live on both channels; the global rails
  `assert_reliability_compare_ran` and `assert_reliability_skip_rows_are_live` fail a
  run in which a gating channel compared nothing or a row was never consulted. Note
  what that liveness rule polices: **visits, not hits**. The garbage the oracles read
  is denormal (`~2.8e-309`), inside `i_abs` anyway, so the rows record 50 visits and 0
  hits on the current population and are not load-bearing there — they exist for the
  values that would NOT be denormal, and the regime itself is proven by the
  three-process probe. Reported upstream as
  `investigations/to_opendss/62-metered-sensor-arrays-are-never-initialised.md`.

Three parity notes, recorded rather than assumed. We capture **every** section, where
fastdss reads section 1 only (`save_outputs.py:283-291`) — strictly stronger.
`SeqListSize`/`CountBranches`/`CountEndElements` are the lengths of the three zone
lists we compare in full, and `MeteredElement`/`MeteredTerminal`/`Peakcurrent` are
EnergyMeter properties #0/#1/#6 already live-compared by `compare_all_properties` — as
is **CAIDI**, which has no API mode on either channel (capi exports none; r4133's
`MetersF` stops at mode 6) but is EnergyMeter property **#22** and moves the moment
`RelCalc` runs, so it is *property-compared*, never "not comparable"
(`caidi_is_saidi_over_saifi_on_a_reliability_deck`). And a deck whose zone loads are
`xfkva`/`kwh`-spec makes `SAIFIkW` solve-derived
(`Σ kWBase·RelWeighting·Bus_Num_Interrupt / Σ kWBase`) and drops it out of the exact
set — measured `0.18812283916834857` (capi) vs `0.18812283916834877` (r4133) — which
is why `midi_relcalc.dss`'s loads are kW literals; anyone adding a reliability deck
should keep them so.
**G1.10a — the created-file SET (`compare_run_files`, live 2026-09-06, lane `lane-s`).**
The surface is, for one (case, channel, port-run), the **set of filesystem entries the run
created under the case directory** — which is `OutputDirectory` after `Compile` (r4133
`Version8/Source/Common/DSSGlobals.pas:962`, capi 0.14.5 twin `src/Common/DSSGlobals.pas:563`;
every executive writer prefixes it, r4133 `Executive/ExportOptions.pas:401`,
`Executive/ShowOptions.pas:208-211`) — over the run `clear → compile → post → n × solve`, as
`/`-joined case-dir-relative names, a trailing `/` marking a created **directory**, sorted,
ASCII-case-folded, compared as an exact set at `rel = abs = 0`. Precisely: *the case dir's own
entries, plus everything under a directory the run created* — the classification does **not**
descend into a pre-existing subdirectory, because the gate's task unit is the case *dir*, not the
dir *tree*, so an entry appearing under a pre-existing subdirectory belongs to a concurrently
running sibling case (measured over two full 526-case drives: 9 unique such members, every one of
them under another manifest case's directory — `Test/AutoTrans/Auto3bus_*.txt` under `Test/`; and
over the whole vendored corpus exactly one `set datapath` line, an absolute path outside the tree,
and no `export`/`show`/`dump` line carrying a path separator). Nothing is added to any run: the
surface is purely observational, on all three producers. Monitor CSV **names** are members
(discrete metadata — circuit, monitor, channel index); their *contents* are G1.10b's and stay out
per this file's unsampled-monitor rule. `compare_export` is untouched — G1.10a compares names, not
bytes. Provenance, so nobody reads this as catch-up: upstream's own harness archives the CSVs
(`origin/fastdss` `tests/save_outputs.py:597-609`) but its comparison is **non-gating** — a name
missing on the other side is skipped (`tests/compare_outputs.py:412-421`) and a CSV mismatch is
*printed* (`:517-524`) — so the file set was never gated anywhere upstream. This is new coverage.

**One classification, three consumers; a leaked dropping fails the case.** The oracle transports
report from the very `CorpusGuard` that sweeps the case dir, and the port from a newtype over the
same Rust guard, so the reported set can never disagree with the swept set:
`dss_epri::guard::classify_created` (with `normalize_created_name`, `is_engine_scratch_file`,
`split_engine_scratch`) is the single implementation, its Python twin
`tools/oracle/corpus_guard.py::_classify` is held to the same synthetic fixture from both sides
(`guard::tests::classifies_the_shared_synthetic_fixture` and `python tools/oracle/corpus_guard.py
--self-test`, whose name lists are byte-identical constants — and
`guard::tests::the_python_twin_shares_this_fixture_and_passes_its_self_test` is what makes both
halves gated rather than claimed: it compares the two `SELF_TEST_*`/`const` name lists and RUNS the
Python self-test inside `cargo test`, resolving the interpreter through `DSS_ORACLE_PYTHON` and
failing rather than skipping when it is absent; `normalize_created_name_is_the_python_twin` pins the
fold), and since D32(3) the gate's own outer
guard (`corpus_gate/runner.rs::CorpusGuard`) is re-based on it too — which is what closes the
sibling-case *deletion* hazard everywhere
(`guard::tests::a_sibling_cases_files_under_a_pre_existing_subdirectory_are_neither_reported_nor_swept`).
A report that would be dishonest is `None`, never `[]`
(`an_incomplete_snapshot_refuses_to_report_and_never_deletes`), and
`harness::capture_guard::require_capture_opt` turns that into a failed case while "asked, and this
deck creates nothing" stays a legitimate answer. **The sweep no longer swallows a failed removal:**
a created entry still present after the sweep comes back as `sweep_failed` and the runner fails the
case with a *leaked dropping* message naming the producer
(`guard::tests::a_created_file_the_sweep_cannot_remove_is_reported_as_sweep_failed`, the port-side
twin `harness::run_files::tests::a_port_dropping_the_probe_cannot_remove_fails_the_case` through
`RunFileProbe::finish_and_clean`; all three producers report it since the G1.10a audit settlement — the outer
guard prints what its own sweep could not remove
(`runner::the_outer_guard_reports_a_created_file_it_cannot_remove`), the only sweeper on a
`kind=large*` case, and a transport reply that OMITS the report is a broken transport, not a clean
sweep (`runner::a_transport_reply_without_a_sweep_report_fails_the_case`,
`runner::a_transport_reporting_a_leaked_dropping_fails_the_case`)). That rail exists because an order-coupling hid a real gap for a
while: dss_capi 0.14.5 opens the Storage `DebugTrace` stream at edit time and never closes it
(`src/PCElements/Storage.pas:868-885`, freed only at `:871`/`:1199`), its guard's `os.remove` failed
silently, and every later producer of the same case then snapshotted the leaked file as
*pre-existing*. The capi transport therefore issues one `clear` as the last statement of its guard
scope — after the classification, so the compared surface is already captured — which runs the
destructors that release the stream; r4133 closes its own trace file as it writes the header
(`Version8/Source/PCElements/Storage.pas:1085`) and needs no counterpart.

**That teardown `clear` is guarded (D33(1)).** The pinned dss_capi 0.14.5 faults on a second `clear`
after an AutoAdd solve — `DSSException (#303) … ProcessCommand … clear … Access violation`,
deterministic on `modes:autoadd/autoadd.dss` and `autoadd_cap.dss` — so it is wrapped: the exception
is caught, reported in a run-level `teardown_error` reply key that the runner prints (channel +
message) without failing the case, the sweep still runs and still reports `sweep_failed`, and a
*persistent* worker whose teardown raised replies in full and then leaves the request loop so the
pool respawns it (a raised access violation may have poisoned the process). Pinned by
`engines::a_capi_worker_whose_teardown_clear_raises_replies_in_full_then_exits_for_respawn`, which
drives a scratch copy of the deck, asserts the whole surface survives the fault (checkpoints, the
`b3, 0.0180069930672805` winner, 4 AutoAddLog rows, the two created names) and reds in **both**
directions — if dss_capi is ever fixed, the guard is reported as retirable. It is an
outdated-oracle artifact: recorded in `docs/upgrade/DIVERGENCES.md` (2026-09-06), never reproduced,
never a ledger row.

**The case-fold is a cross-oracle normalization, not a tolerance, and it is structural.** The two
oracles disagree with each other on the spelling: r4133 writes `EXP_VOLTAGES.CSV`
(`Version8/Source/Executive/ExportOptions.pas:333-356`), dss_capi 0.14.5 writes `EXP_VOLTAGES.csv`
(`src/Executive/ExportOptions.pas:314,343,345`), and r4133 additionally lowercases the whole
deck-supplied stem (`auto1bus_hl_current.txt` against capi's `Auto1bus_HL_current.txt`). The port
follows capi's spelling — `export_with` / `write_export` name the lower-case stems at
`crates/dss-core/src/exec/report.rs:314`, `:370`, `:399` and `:1411` — and the comparator
folds ASCII case on all three sides, so both channels gate the same set — the R-18 decision and its
four reasons are recorded in `docs/upgrade/DIVERGENCES.md` and pinned literally by
`run_files_pins::the_two_oracle_spellings_of_auto1bus_fold_to_one_member` (the nine raw names per
side, so a later normalizer change cannot silently drop the case). Operational caveat, measured
2026-09-06: that pin reads the real created-file set of the vendored deck, so a dropping an earlier
drive left in `Test/AutoTrans/` (the STATUS-tracked `kind=large*` leak) is *pre-existing* on the
next run and shrinks the expected nine names to seven — clean the untracked files in that directory
before a gate run; never widen the pin. The fold is ASCII-only on
purpose: a non-ASCII name is **refused** loudly instead of being folded by one language's locale
rule. No floor is introduced anywhere; `tests/TOLERANCE_NOTES.md` §G1.10a records why.

**The D25/Q2 engine-scratch split — one structural normalization, 0 ledger rows.**
`<CircuitName_>SavedVoltages.dbl` is the Pascal engines' harmonics disk round-trip (r4133
`Common/Utilities.pas:1512-1521` `SavePresentVoltages`, reached only from `InitializeForHarmonics`
`:1599-1608`, read back by `RetrieveSavedVoltages` `:1554-1564` and consumed at
`Common/SolutionAlgs.pas:1056,1131`; capi twin `src/Common/Utilities.pas:883`/`:914-923`); the port
keeps the fundamental solution in memory (`solution/solution/state.rs`, `harmonics.rs`) and must
never write the file. It is split off **symmetrically on every producer** and **counted**, not
dropped: the decline table is re-derived on every run and pinned in both directions as
`SCRATCH_FILE_DECLINES = (9 cases, 9 names)` — nine harmonics decks, one name each
(`modes:harmonics/{harmonic_hlist,harmonict,isource_harm,reactor_rlcurve}.dss`,
`modes:inputformat/xycurve_files/xycurve_files.dss`, `solvable_now:Test/PVSystemTestHarm.dss`,
`…/FreqScan/Run_Scan.dss`, `…/HarmonicsVariableLoad/IEEE_519.DSS`, `…/NEVTestCase/Run_NEV.dss`) —
by `scheduler::assert_scratch_declines_are_the_pinned_population` over
`harness::run_files::scratch_decline_table`, called from the gate epilogue, armed off the manifests
and **refusing** any label that is neither a manifest case nor a `unit:` fixture. The unit half is
`the_engine_scratch_file_is_split_off_and_counted`, the anti-abuse half
`the_port_may_never_report_a_scratch_file` (the port side of the split is asserted empty), and the
deliberate non-member is `<CircuitName_>SavedVoltages.Txt` — the user-visible `Save Voltages` output
`VDIFF` reads back, which all three engines write. Both-numbers pin:
`run_files_pins::the_harmonics_scratch_file_is_declined_on_the_nev_deck` (oracle 7 names on both
channels, port 6).

**The one ledger row: `Visualize` writes a DSSView file pair.** On `solvable_now:Test/YgD-Test.dss`
the r4133 channel creates `testYgD_Transformer_tr1_PQ.DSV` + `.dbl` for `Visualize powers
Transformer.TR1` (`Executive/ExecHelper.pas:4071`); dss_capi 0.14.5 fires a callback and writes
nothing, and the port emits a JSON plot payload instead (`exec/command.rs`). That is a **product**
divergence, not an upstream defect — no `investigations/to_opendss/` note is owed, the decision is
in `docs/upgrade/DIVERGENCES.md` — so it is one `r4133` `exclusion` entry
(`r4133-visualize-writes-a-dssview-file-pair`, cause `visualize-dssview-file-pair`) scoped by
`name_re` to the two `_pq` names, plus the both-numbers pin
`run_files_pins::visualize_writes_a_dssview_pair_on_r4133_and_a_json_payload_in_the_port` (r4133 6
names, port 4, capi 4; the port's single plot payload asserted non-empty; the pin drives the
un-excluded compare, the excluded compare and a widening check). `run_files` joins the ledger
vocabulary as the fourth **per-value** exclusion field (`LEDGER_FIELDS` 16 → 17, `EXCLUSION_FIELDS`
12 → 13, `EXCLUSION_ONLY_FIELDS` 7 → 8, `PER_VALUE_EXCLUSION_FIELDS` 3 → 4): a discrete set at zero
tolerance has no envelope a `divergence` could re-assert, and the per-value list is what makes a
dead `name_re` report as stale.

**Population, flag and lock.** `compare_run_files` (`runf=`) is forced on every live non-`large`
case — the same 443 (312 `both`, 87 `r4133`, 44 `capi_v0145`) that carry the property, topology and
incidence surfaces, re-derived by
`scheduler::the_run_files_forcing_rule_is_every_live_non_large_case` as
`FORCED_RUN_FILES_POPULATION` and asserted equal to all three sibling constants. Six decks also
*declare* the flag (`scheduler::RUN_FILES_DECLARED_IN_MANIFEST`, pinned by
`the_run_files_surface_is_declared_on_every_gating_channel`), because `population.lock.json`
fingerprints the manifest flag and cannot see scheduler-side forcing: `Test/REACTORTest.DSS` (8
files, `Show`+`Dump`, two circuits in one deck), `NEVTestCase/Run_NEV.dss` (7 names, one of them the
scratch `.dbl`), `StoCtrl_SeasonTarget/Run_example.dss` (the created **directory** tree
`ieee13nodecktmod/di_yr_0/` + 6 CSVs — G1.10c's hook), `Test/DistanceRelayTest.DSS` (the `r4133`-only
witness, 1 file), `HarmonicsTMode/IEEE_519.DSS` (the `capi_v0145`-only witness, an honest **empty**
set: its two writers overwrite *vendored* files, and it is the only candidate — of the 44 capi-only
live cases only three reach a file-writing command) and `Examples/CIM/IEEE13_CDPSM.dss`
(`kind=large`, so the declaration is the ONLY thing that reaches it). The lock moved by exactly six
`runf=0 → runf=1` tokens plus the YgD row's `ledger=` digest, and no family manifest declares the
flag because no `asymmetric`/`controls`/`modes` family deck writes a file at all.

**One producer per case directory (D33(2)).** `runner::CorpusGuard` holds an exclusive claim on the
*canonical* case directory, taken before the pre-run photograph and released only after the sweep
and the restore, so a case owns its folder from the first oracle capture through the port run to
the last byte put back. It is reentrant for the owning thread (nested guards share the one pristine
snapshot), the key is `fs::canonicalize`d so two spellings contend instead of racing, the
photograph happens outside the registry mutex, and a wait past `DIR_CLAIM_DEADLINE` (600 s) fails
loudly rather than hanging. The claim sits in the guard and not in the scheduler because the
scheduler *already* serializes a case-dir group into one task — the measured concurrent producer is
a sibling `#[test]` of the same binary, `corpus_gate.rs::corpus_ad_matches_normal_mode`, which
compiles `ad_sweep.json` decks **in place**, so three `8500-Node` decks execute their own
`Show`/`Export` lines during `compile`, before `datapath` is re-pointed at a scratch dir. Pinned by
`runner::corpus_guard_serializes_two_threads_in_one_case_directory`,
`runner::corpus_guard_does_not_serialize_two_different_case_directories` (per directory, never a
global corpus lock) and `scheduler::two_manifest_rows_in_one_case_directory_land_in_one_task`.
Since **D35(3)** *every* producer in that directory takes the claim, the two cross-transport
`#[test]`s included: `the_two_transports_agree_on_the_bus_capture_of_a_gated_both_case` and its
short-circuit twin drive both oracle transports over one corpus deck and were the last guard-less
producers — each now opens with a `CorpusGuard` (`corpus_gate.rs:1246`, `:1546`). Measured after the
change: the intermittent single-case capi red ("You must create a new circuit object first") did
not recur in four consecutive full default-lane drives — it did recur once on 2026-09-06, on a
parity drive taken while three other lanes were building on the same machine (CPU at 100 %), and
passed both scoped and on the quiet re-drive, so the flake is narrowed but **not** closed: read a
red of that shape as load-dependent only after re-running it on a quiet machine.
Measured cost: **none** — the gate's own wall time went 221.7 s (before) to 188.6 / 186.8 / 165.8 s
over three post-change default-lane drives (−33.1 / −34.9 / −55.9 s, −15 / −16 / −25 %) and 174.0 s
on the parity lane; the claim removed work rather than adding it (contaminated cases used to fail
late, and the guards no longer photograph each other's output). Read the sign, not the third digit
— other lanes were building on the same machine.

**A guard restores what was overwritten; it never resurrects what was deleted** (G1.10a audit
settlement, 2026-09-06). `Test/` holds 36 manifest cases and `Test/AutoTrans/` five — two claim
keys, so a parent-directory case and a child-directory case run concurrently by design — and the
parent's recursive photograph covers the child's directory too. Restoring every buffered entry
whose bytes no longer match therefore wrote back files the child's own guard had just **swept**,
which is the mechanism behind the long-standing `Test/AutoTrans/*` residue and behind "a survivor
reads as pre-existing next run and silently shrinks a created-file set". Both Rust guards
(`corpus_gate/runner.rs`, `crates/dss-epri/src/guard.rs`) now leave a `NotFound` entry gone, restore
an entry that still exists and differs (`corpus_guard_restores_case_dir_recursively`) and still
attempt an unreadable one; `corpus_guard.py` always behaved this way, so the three producers agree.
Pinned by `runner::a_parent_guard_does_not_resurrect_a_sibling_cases_swept_output` (a deterministic
child-writes / parent-photographs / child-sweeps / parent-restores interleaving, red before the
fix); measured 9 leaked files before, **0** over four consecutive full drives after.

**The run-file read is per-RUN, and strictly last.** It is not an A/B/C capture group: the
classification is one read of the filesystem for the whole run, and
`capture_order::check_run_files_last` asserts its position from **both transports' source text** —
exactly once in the code view, after every per-step capture the `Anchors` table names, after the
WPG.5 `autoadd_log` read (which reads one of the created files off disk), lexically inside the
guard scope and as a direct statement of it (a classification nested in the retry loop, which
recompiles in-process, is rejected), and — D33(3) — with the tail after it equal to the declared
teardown: `["clear"]` on capi, where the command is the SOLE statement of its `try` block, and
empty on r4133. Gates: `capture_order::capi_capture_classifies_the_run_files_last` and
`capture_order::r4133_capture_classifies_the_run_files_last`, with teeth proven on synthetic *and*
real sources by `the_run_file_gate_rejects_an_early_escaped_or_nested_classification`,
`the_run_file_gates_have_teeth_on_the_real_transport_sources` and
`the_run_file_gate_rejects_a_classification_after_the_sweep` (14 mutations, all in memory).

**Two files the r4133 bridge itself used to write are gone.** They mattered the moment the
created-file set became a compared surface. (1) `capture_eventlog` no longer issues `export
eventlog` — which dropped `<CircuitName>_EXP_EventLog.CSV` into the case dir and accounted for 59
of the 61 red (case, channel) pairs of the first full drive — and reads `Solution.EventLog` in
memory instead (`DDLL/DSolution.pas:518`, `:526-541`), the same list the capi transport
(`ckt.Solution.EventLog` → `CAPI/CAPI_Solution.pas:525-540`) and the port read; the two were
byte-identical corpus-wide over a full 526-case drive (0 mismatches, 83 `evlog=1` cases), and the
equivalence is now a bridge test, `protocol::the_in_memory_event_log_equals_the_exported_file`,
beside the behavioural `protocol::the_event_log_capture_creates_no_file`. Upstream never compared
this surface either (`origin/fastdss` `tests/compare_outputs.py:289-292` skips `EventLog` as "too
textual"). (2) The bridge suppresses report auto-display at init — §"The bridge suppresses report
auto-display" below (three layers; `protocol::init_overrides_the_os_editor_and_never_writes_it_back`
and the two D39 tests beside it). Meanwhile the port
*gained* a writer: Storage `DebugTrace` was unported, so the port created nothing where both
oracles create `STOR_<name>.CSV` at edit time (r4133 `Version8/Source/PCElements/Storage.pas:1073-1085`)
— a port gap on the authority channel, fixed in its own commit ahead of the surface (CLAUDE.md's
"port gaps immediately"), open-append-close on every write so the guards can always delete the
file. G1.10a compares that name; its contents are G1.10b's, where the two ORACLES disagree with
each other in 16 columns (FPC `%-.g` prints 2 significant digits, the Delphi-built r4133 ~15).

**`save circuit` has no corpus surface (rider C3, recorded rather than built).** A grep for a
leading `save` over the reachable corpus finds **zero** `save` commands, so the plan's "compare the
emitted file set vs the oracle's" has nothing to run on. The gate for it stays
`crates/dss-core/tests/save_roundtrip.rs::save_forms_structural_file_set`, a hardcoded 14-entry
**fixture** — recorded here as a fixture test and deliberately left as one: upgrading it to fetch an
oracle's file set live would add a second oracle transport for a single test, and inventing a corpus
deck to carry it would be a synthetic surface with no upstream provenance.


**The per-bus reliability arm (G1.6(ii), 2026-09-05).** The eight columns
`Export BusReliability` renders — `Lambda`, `N_interrupts`, `N_Customers`,
`Cust_Interrupts`, `Cust_Duration`, `Int_Duration`, `TotalMiles`, `SectionID`
(`IBus._columns` on `origin/fastdss`) — ride **inside** the reliability payload
(`ReliabilityCap.buses`), so they inherit the whole G1.6(i) protocol above: the
same once-per-case `RelCalc`, the same last-checkpoint rule, the same six
manifest-set cases (4 capi-gating / 50 buses, 5 r4133-gating / 84 buses). Port
side `Dss::bus_reliability` / `BusReliabilityView`
(`crates/dss-core/src/exec/view.rs:3665` / `:3616`) — `&self` reads, the
reliability math untouched; transports
`capture_bus_reliability` on both arms
(`tools/oracle/oracle_server.py:664`, `crates/dss-epri/src/capture.rs:1753`); comparator
`harness::compare_bus_reliability`
(`crates/dss-core/tests/harness/mod.rs:18380`), called from inside
`compare_reliability` so the payload keeps one entry point. Six rules come with
the arm.

* **Bus selection before every read.** Each column is a plain
  `Buses^[ActiveBusIndex]` field read guarded only by `ActiveBusIndex > 0`
  (r4133 `DDLL/DBus.pas:129-170` and `:60-73`), so a transport that reads a
  column before selecting its bus silently reports the previous bus. Both
  capture bodies are read as text and checked against their own pinned
  eight-field order **and** against the selection rule, with a negative drive
  over a synthetic body that reads before it selects. Like the meter arm this is
  group **(C)** of the capture-order partition: no read of it reaches
  `GetCurrents`, so it neither imposes nor inherits an element order.
* **Counts and names are unconditional.** The bus count and the whole name
  sequence are asserted *before* the ledger hook is consulted — an exclusion can
  drop a value compare, never hide a missing or misaligned bus.
* **Every value is compared exactly, `rel = abs = 0`** — no `Tolerances`
  parameter at all, `NaN`/`±inf` agreement via the meter arm's `rel_num_eq`
  (`Int_Duration` inherits the unguarded `AverageRepairTime` division).
  Derivation in `tests/TOLERANCE_NOTES.md`; no band, no new tier, no existing
  floor moved.
* **Ledger keys are `bus:<busname>:<field>`** (lowercased) on the **existing**
  `reliability` per-value field, so no `LEDGER_FIELDS` change is owed and a bus
  exclusion can never drop the meter half. The namespace is *proved* disjoint
  from the meter arm's `<meter>:<field>` rather than assumed: a meter whose own
  name begins `bus:` is refused. **Measured: 0 ledger entries on either
  channel** — the two oracles return this whole surface bit-identically
  (400/400 shared cells equal under `==`; the two `combo` decks are r4133-only,
  `makeposseq_ctrl` capi-only), so a red here is a *port* question, never a
  channel question.
* **The arm cannot collapse silently.** `BUS_RELIABILITY_WALKS` /
  `BUS_RELIABILITY_BUSES` feed `assert_bus_reliability_compare_ran`, printed and
  asserted from the gate epilogue as `corpus_gate bus reliability: capi_v0145 …
  payload(s) / … bus(es), r4133 …` — the meter half's guard cannot see this half
  vanish, because the bus rows ride inside the meter payload.
* **`SectionID = -1` is legal, and never read as a sentinel.** The zone zeroing
  writes `-1; // signify not set` (`PDElements/PDElement.pas:326`) before the
  forward sweep re-stamps the head bus (`Meters/EnergyMeter.pas:2494`) and every
  TO bus (`PDElement.pas:179-181`), which collides with the r4133 `I` sentinel —
  so that row's identity is proven by value, never by a `Served` verdict.
  No bus on the six flagged decks reads `-1` today.

Two measurements are recorded rather than assumed. The **multi-meter
`Bus_Int_Duration`** divergence (the upstream all-buses duration loop indexing
foreign section ids into this meter's `FeederSections`, r4133
`Meters/EnergyMeter.pas:2567-2575`) needs two meters **and** a completed section
allocation: of the six flagged cases exactly one has two meters
(`midi_energymeter`) and both of its meters abort at 52902 before any section
exists, so the divergence is **not reachable on this population** — and on the
four `DOCTechNote` decks where it *is* reachable (out of the population by
D-i-2) the regime is the deterministic in-range overwrite, not the
nondeterministic out-of-range read, measured over repeated fresh oracle
processes on both channels. Its witness therefore stays the
`export_busreliability_multimeter` golden and its G2.2a pin, as G1.6(i) said.
And **`calc_reliability_indices` regained `TotalUpDownstreamCustomers`**
(r4133 `Meters/EnergyMeter.pas:2466-2468`, a 1:1 gap fixed under CLAUDE.md's
"port gaps immediately" rule): it moves nothing on the corpus, on goldens or in
the ledger, and diverges from the pinned dss_capi 0.14.5 oracle only under
`RelCalc <restore>`, which no deck drives — see `docs/upgrade/DIVERGENCES.md`
(2026-09-05, D22).

The pins: `bus_reliability_columns_match_both_oracles_on_the_duty_deck`,
`bus_reliability_sections_and_durations_on_the_relcalc_deck`,
`bus_reliability_survives_the_52902_abort`,
`bus_reliability_columns_on_the_capi_only_and_r4133_only_decks`,
`bus_int_duration_stays_in_the_meters_zone_on_the_live_population` (the
population and Q4 record),
`every_bus_reliability_column_is_read_by_some_transport` (the census), plus the
two that carry the restored call —
`relcalc_recomputes_the_customer_totals_it_depends_on` and
`relcalc_assume_restoration_changes_auto_ocp_interruptions`.
Two offline drives sit beside them in the harness (G1.6(ii) audit settlement):
`every_bus_reliability_column_is_compared_per_bus` corrupts each of the eight
columns in turn, on both channels, and requires the comparator to red naming
that column (the per-column non-vacuity of §1.1(f), in the tree rather than in a
log), and `a_non_finite_reliability_cell_fails_the_decode_on_both_transports`
pins what actually happens to a `NaN`: capi puts the bare token `NaN` on the
wire, r4133 `null`, and both caps refuse it, so a non-finite cell fails the
decode loudly instead of comparing as a plausible `0`.

### The divergence ledger (`tests/corpus/ledger.json`)

The gating successor of the report-only `known_diffs.json`. Every entry pins
**where** and **how much** one case may diverge from one channel's oracle, with
a mandatory documented cause — and **fails the gate when stale**, so the ledger
can never rot into a soft-tolerance backdoor. Tier floors in
`tests/harness` (`Tolerances`/`tol_for`, `tests/TOLERANCE_NOTES.md`) are
structurally unreachable from ledger code and never change here.

**Oracle floats are parsed exactly since G1.4a.** The workspace `Cargo.toml`
enables `serde_json`'s `float_roundtrip` feature, so every float either transport
sends decodes to the value it was written from. Without it `serde_json`'s default
parser is not correctly rounded and returns some values 1 ULP low, which every
tolerance-based comparator had been absorbing silently since the gate existed;
`kv_base` is the first **exact** float compare the gate ever ran and it reddened
4 of the 523 cases then in the manifest at exactly 1 ULP, with the port, the capi
transport and the r4133 transport all agreeing bit-for-bit (the decomposition, including the three JSON
strings' bit patterns, is in the G1.4a record). Two things follow. A 1-ULP gap on
an exactly-compared oracle value is a **transport** question before it is a
numeric one — decompose it, never band it (banding a decode defect is exactly the
tolerance-widening CLAUDE.md forbids). And any floor measured before that feature
landed can only be re-measured tighter, never looser.

Entry kinds (`kind`):

- **`divergence`** — the case runs and is fully compared; the `match` scopes
  are *expected to diverge* and are re-asserted inside their pinned envelope.
  The gate asserts: (a) selected values differ from the oracle by ≤ the
  envelope (`max_rel`/`max_abs`); (b) unselected values still meet the tier
  floor (the untouched `harness` comparator runs on the remainder); (c) at
  least one selected value **exceeds** the tier floor — else the entry is
  **STALE and the gate fails** with a prune instruction. `probe`/`property`
  scopes take either a `num_rel` numeric-skeleton envelope (asserted against
  the live Rust value) or **exact pins**: non-numeric requires the `oracle`
  pin; exact-pair-numeric requires **both** `rust` and `oracle` pins (so a
  port regression to a third value cannot pass silently). Discrete state is
  exact-pair only — never an envelope. `eventlog`/`ctrlqueue` scopes are
  `line_re` masks bounded to the trailing-whitespace class.
- **`skip`** — the case is not sent to that channel at all (hard-crash decks,
  e.g. the r4133 `#303` access-violation line-spacing/GrowthShape class);
  requires a `cause` naming the crash. The other channel still gates the case,
  and structurally at least one non-skipped channel must remain. A `skip`
  cannot go stale by construction — re-validate manually with
  `DSS_GATE_SEED_LEDGER=1 DSS_GATE_SEED_ONLY=<case>`.
- **`exclusion`** — a proven upstream bug poisons specific comparison scopes on
  a channel (`cause_ref` into a documented investigation); those scopes are
  skipped, everything else compared. Since `GOLDEN_REBASE_PLAN.md` G2.5 this is
  also how an engine fix that *declines* an upstream bug is paid for: the fixed
  engine's answer is deliberately unlike the oracle's, so there is no envelope
  to re-assert — the correct value is pinned by an expected-value test the
  entry's `cause` names by full test path, and the scopes it moves are dropped
  here. Still hit-accounted: an exclusion whose scope stops matching fails the
  gate as NEVER APPLIED. It is also **half fail-on-stale**: a `voltages` scope
  is measured node-by-node against the tier floor exactly as a `divergence` is,
  so an exclusion carrying one and never exceeding is reported STALE (the
  engine fix it paid for always moves node voltages — that is what makes an
  entry of this kind necessary). The coarser scopes carry no verdict — the
  runner skips the artifact instead of comparing it — so for them the anti-rot
  guard is the **expected-value pin** the `cause` names, which is mandatory and
  registered both ways in `oracle_parity_cfg_gate.rs::TORN_DOWN_ROWS`: revert
  the engine fix and the pin reds, whether or not the corpus gate notices.
  A scope may carry only its **selectors** — `max_rel`/`max_abs`/`num_rel`/
  `rust`/`oracle`/`policy`/`line_re` on an `exclusion` are refused at load,
  because the exclusion path ignores them and they would read as a promise the
  gate never keeps.
  *(**2026-09-04**, G1.4a: a **deck-wide** `voltages` scope — no `name_re`, no
  `node_re` — now also suppresses that case's three continuous per-bus arrays —
  §"The bus voltage surface" above. A node-scoped one keeps its node-by-node
  meaning and suppresses no bus array. The suppression is reported case by case
  in the gate summary and changes nothing about this scope's own node-by-node
  fail-on-stale measurement.)*

Scope `field` must be one of the **17 implemented** handlers — `iterations`,
`voltages`, `injection`, `element`, `probe`, `property`, `monitor`, `eventlog`,
`ctrlqueue`, plus the eight **exclusion-only** ones `y`, `y_fingerprint`,
`yprim`, `meter`, `variables`, `reliability`, `distance`, `run_files` — anything
else (typo or the §1.3-planned but unimplemented `global_result`) is rejected
loudly at load. Four of the exclusion-only eight name a whole compared artifact rather
than a value with a natural envelope (the assembled system Y, its fingerprint,
one element's YPrim, one EnergyMeter's register block), so `assert_structural`
refuses them on a `divergence`. `variables` (added by `R4133_PROPS_PLAN.md`
RP3.10) is exclusion-only for the other reason: a PC element's state variable
does have the `i_abs + i_rel·|oracle|` envelope, but no handler re-asserts one,
so a `divergence` naming it would promise a measurement the runtime never
makes; `reliability` (G1.6(i)) is exclusion-only for the same reason.
`distance` (**G1.4b**, coordinator decision D29 step 3) is exclusion-only
because its surface is compared **exactly** (`rel = abs = 0`), so there is no
envelope a `divergence` could re-assert at all. It is **per-VALUE**, keyed by
the BUS name (`PER_VALUE_EXCLUSION_FIELDS` polices each `name_re` scope
individually, so a scope that stops masking fails the gate as STALE), and it is
consulted only **after** the port↔oracle equality has already failed on that
bus — which is what makes a scope's `hit` mean *masked a real divergence*
rather than *matched a name*. Everything else about that bus stays compared:
the bus count, the name sequence, all three array lengths, the port-internal
identity `AllBusDistances[i] == Bus.Distance == AllNodeDistances[k]`, the
oracle-internal one, and the run-wide `DISTANCE_POPULATION`. `run_files`
(**G1.10a**) is the fourth of that per-VALUE shape — keyed by one created-file
name, compared exactly, with the presence rail, the ASCII refusal and the
engine-scratch census staying unconditional whatever the ledger says.
Location selectors: `node_re`/`name_re`/`channel_idx` (0-based)/`channels`,
optional `steps` (0-based). `iterations` takes exact `{rust, oracle}` pairs or
`policy: "rust_le_oracle"`; `yprim`/`monitor`/`meter`/`probe` exclusions select
the artifact by `name_re` (absent ⇒ all), and a `variables` exclusion selects
**one state variable** by `name_re` over the lowercased `element:variable` key
(`windgen.w1:pgen`) — deliberately finer than the element, because the coarse
alternative (dropping the element from the manifest's `variables` list) would
mask a dynamics deck's whole 22-variable surface and shrink the population lock.
`compare_variables` keeps its variable-COUNT assertion unconditional, and
asserts that both engines spell any excluded index the same, so a mask can
neither hide a missing variable nor slide onto a clean one. The reverse rule
holds too: an `exclusion` may name only a field the exclusion path actually
handles (`voltages`, `element`, `injection`, `monitor`, `probe` + the five
above) — `iterations`, `property`, `eventlog` and `ctrlqueue` are
divergence-only, because their handlers re-assert a pin or rewrite the oracle's
line, and a scope that loads cleanly and then never applies is the one thing the
field whitelist exists to prevent.

Runtime rules: every applicable entry must be **hit** ≥ 1 (never-applied →
gate fails), every `divergence` — and every `exclusion` carrying a `voltages`
scope — must still exceed the tier floor somewhere (fail-on-stale; the
`divergence` half proven live by canary in the Phase D/E audits, the
`exclusion` half by `a_voltages_exclusion_that_masks_nothing_is_stale`).
Since the R4133_PROPS §RP3.10 audit settlement every **`variables`** scope
carries its own liveness on top of that: those flags are per *entry*, so an
entry that also excludes `voltages`/`element` would stay alive through them
while a renamed state variable silently emptied its per-value mask — therefore
each `variables` scope must itself have matched ≥ 1 variable this run, and
`assert_all_hit` names the dead scope's `name_re` when it has not
(`a_voltages_exclusion_that_masks_nothing_is_stale` drives that rule both
ways). The
oracle-free
structural test (`ledger_is_structurally_valid`) checks unique ids, case ∈
manifest, channel ∈ the case's `engines`, non-empty `match` for divergences,
resolvable `cause`/`cause_ref`, compiling regexes, and the two kind↔field rules
above; `every_exclusion_field_is_honoured_by_the_runtime` drives
`LedgerView::excluded` synthetically over **every** whitelisted exclusion field,
so the two (`probe`, `meter`) with no live entry today are still proven to
apply. Every entry is fingerprinted
into the population lock as `id@FNV-1a64(entry JSON)` per channel — adding,
widening, or re-scoping an entry is always a reviewable lock diff.

Current contents (re-counted off the file 2026-09-06, after G1.4a's D12/D14
channel flip, G1.3b's D31 entry, G1.4b's D29 `distance` entry, G1.3c's
**25** widenings of existing `element` scopes — which added no entry and no
cause — and G1.10a's `run_files` entry): 57 entries over 33 documented
causes — 5 r4133 `skip`
(the four #303 crash decks plus `r4133-espvlcontrol-uninstantiable`, where the
r4133 DLL cannot construct an `ESPVLControl` at all), 29 r4133 `divergence`
(Delphi 6-sig-fig display-precision
probes on Storage/PVSystem, FPC-vs-Delphi injection/element ulp floors on the
IndMach asymmetric decks, one monitor sequence-magnitude drift, the GFM
`%stored` rounding class, the RegControl `idle`
revThreshold/fwdThreshold getter-convention exact-pair, and the **eight**
`property` entries R4133_PROPS RP4.1 landed with its unmask — `swtcontrol.delay`
×2 (RP3.1), `windgen.kvar` ×4 (RP3.2) and `gictransformer.r2` ×2 (RP3.4), each
an exact pair on the r4133 channel), 11 capi_v0145
`divergence` (the `line_spacing_asym` and the Generator `MakePosSequence`
exact-pair-numeric upgrade pins, one G2.5 property-jump entry —
`Capacitor.cap_cmat.Cuf`/`NormAmps`/`EmergAmps`, pinned as an exact pair rather
than skipped; the two `GICTransformer.tg3/tg5.R2` twins went with the D12/D14
flip below — the three `property` entries the
R4133_PROPS line-merge/switch fixes landed on the live capi compare
(`reduce-merge-units-restored-midi-capi-props`, RP3.5, and
`line-switch-keeps-linecode-zone2/zone3-capi-props`, RP3.6a) and the five
`swtcontrol-per-phase-state-*-capi-props` entries RP3.7 landed), and 12
`exclusion` — 4 capi_v0145 + 8 r4133, from four engine fixes, three measured
oracle-channel defects and one product divergence. Four are
`GOLDEN_REBASE_PLAN.md` G2.5's (2 capi_v0145 + 2 r4133), where the engine
stopped reproducing three
upstream bugs (GICTransformer `%R2`, Capacitor `MakePosSequence` `Cuf`,
LoadShape MMF accept-set) and the four decks that observe them therefore
diverge from their gating channel(s) across the solved model. The other four
are R4133_PROPS RP3.10's (2026-09-04, all r4133, one cause
`windgen-qmode0-no-arm`): the engine implements the constant-Q `QMode=0` arm
`TWindGenObj.SetNominalGeneration` never had (`WindGen.pas:1276-1322` falls to
`Else kvarCalc := 0`), so the four corpus decks that declare a WindGen without a
`QMode=` token diverge from r4133 across `voltages`, `injection` and `element`,
on the two power-flow decks also across `y`/`y_fingerprint`/`yprim`, and on the
two dynamics decks across three (`windgen_dyn`) and four (`windgen_dyn_fault`)
WTG3 state `variables` — the exclusion field that same sub-step added. The
third capi_v0145 `exclusion` is GOLDEN_REBASE G1.3a's
`capi-capcontrol-time-bus-is-the-capacitors`: a `type=time` CapControl binds its
own bus 1 to the **monitored element's** terminal on r4133 and in the port
(`Controls/CapControl.pas:605` + `:622`) and to the controlled capacitor's bus on
dss_capi 0.14.5 (`:597-608` → `:619`), so the deck's two CapControls read a
different bus's voltage on that one channel (`docs/upgrade/DIVERGENCES.md` L8;
the r4133 channel of the same case needs no entry). The seventh r4133
`exclusion` is `r4133-posseq-seqpowers-slot-gic` (coordinator decision **D31**,
2026-09-05, taken at G1.3b's landing): r4133's `SeqPowers` writes the
1φ-positive-sequence arm one slot late with a stride of one
(`DDLL/DCktElement.pas:760`/`:768`, against capi's correct
`CAPI_Alt.pas:555`/`:562`), and `modes/makeposseq/makeposseq_gic.dss` — the deck
D12/D14 moved to that channel — is the only corpus case that reaches the arm
there, so its four elements' `seq_powers` are excluded on `r4133` alone while
`seq_currents`/`seq_voltages`, which r4133 gets right, stay compared. The fourth
capi_v0145 `exclusion` is GOLDEN_REBASE G1.4b's `reduce-merge-units-lost-midi-capi-distance`
(2026-09-05, cause `line-merge-length-units-reset`): the only `distance` entry on
the corpus, three bus scopes on `modes:reduce/midi_reduce.dss`, where capi
0.14.5 loses the merged lines' `LengthUnits` and its zone walk consumes `4 kft`
as 4 km — the surface paragraph above and D29 step 3 carry the evidence.
The **eighth** r4133 `exclusion` is GOLDEN_REBASE G1.10a's
`r4133-visualize-writes-a-dssview-file-pair` (cause
`visualize-dssview-file-pair`): `Visualize` writes a DSSView `.DSV`/`.dbl`
pair on r4133 where the port emits a JSON plot payload, so those two names
— and only those two — are excluded from that deck's created-file SET; a
**product** divergence, so `docs/upgrade/DIVERGENCES.md` and no
`investigations/to_opendss/` report, pinned by
`visualize_writes_a_dssview_pair_on_r4133_and_a_json_payload_in_the_port`.

**`channels` names sub-channels, and must be spelled out** (GOLDEN_REBASE G1.0,
2026-09-04). `element` is today the only field whose comparison has sub-channels
(`currents`, `powers`, `losses`, since G1.3a `currents_mag_ang`,
`voltages_mag_ang`, `residuals`, since G1.3d(ii) `phase_losses`, since G1.3b
`seq_currents`, `seq_voltages`, `seq_powers` and since G1.3c `cplx_seq_currents`,
`cplx_seq_voltages`, `total_powers` — thirteen), and
the runtime reads a scope's `channels` as
*empty ⇒ all of them* — so a committed entry written for the original three would
silently widen onto every new element sub-channel WP-G1 adds (G1.3a–c), with no
ledger diff and no population-lock trip. `SUBCHANNEL_FIELDS`
(`crates/dss-core/tests/corpus_gate/ledger.rs:765`) closes that with three
load-time rules: a scope on such a field must carry a **non-empty** `channels`;
every name in it must be one of that field's declared sub-channels (a typo like
`"curents"` otherwise loads cleanly, selects nothing, and leaves the entry
reporting itself applied while masking not one value); and a scope on any other
field must carry no `channels` at all, since the runtime would never read it.
"All sub-channels" survives only as a named, reviewed exception in
`BARE_CHANNELS_ALLOWED` (`ledger.rs:790`), which is **empty**. The rules are
driven both ways by `a_scope_that_misuses_channels_is_refused_at_load`. When a
sub-step adds a new element sub-channel it adds the name to `SUBCHANNEL_FIELDS`
**in the same commit**, so the committed exclusions keep the width they were
reviewed for instead of quietly gaining one. G1.3a (2026-09-04) is the first to
use it: the `element` list grew to six names — `currents`, `powers`, `losses`,
`currents_mag_ang`, `voltages_mag_ang`, `residuals` — and, because none of the 14
committed `element` scopes could widen silently, the sub-step then **measured**
which of them the new channels break and widened **13** of them deliberately, one
sub-channel at a time, iterating the live gate to a fixpoint (each entry's
`measured.g13a_polar_first_failure` carries the sample that justified it;
`r4133-indmachmidi-injection-ulp` was measured NOT to fail and keeps the original
three). A ledger envelope on a polar channel is measured on the comparator's own
scale, and a **masked** angle — magnitude at or under its own band, where
`harness::polar_close` does not compare it either — is not envelope-checked at
all: bounding an angle the gate never reads would need a ±180 ° "envelope" that
bounds nothing (`a_masked_polar_angle_is_not_envelope_checked` /
`an_unmasked_polar_angle_still_hits_the_envelope`).

G1.3d(ii) (2026-09-05) is the second: `phase_losses` joins the list in the commit
that starts comparing it, and both handlers honour it —
`rewrite_element_selected` writes the port's kW/kvar back with the comparator's
own ×0.001, and `envelope_element` bands each phase with `harness::phase_loss_band`
so a ledger envelope is again measured on the comparator's own scale. **8** of the
13 committed `element` scopes carry it, each **only** after the
live gate printed its own failing sample on its own channel
(`measured.g13d2_phase_losses_first_failure`), iterating to a fixpoint:
`mmf-accept-set-honoured-capi`, `makeposseq-cuf-applied-capi`,
`gic-pct-r2-honoured-{gictransformer,midi}-r4133` and
`windgen-qmode0-constant-q-{daily,snapdelta,dyn,dynfault}-r4133`. (The lane measured
**ten**: `gic-pct-r2-honoured-{gictransformer,midi}-capi` were widened too, and were
already deleted on `update` by G1.4a's **D12**/**D14** — those decks gate `r4133`-only
now — so the widening went with the entry at the merge. `makeposseq-cuf-applied-capi`'s
sample was re-measured there, the same deck having moved under D14.) The four
`r4133-*-injection-ulp` entries select `losses` but were measured **not** to fail on
`phase_losses`, and keep their committed lists; so does
`capi-capcontrol-time-bus-is-the-capacitors` (a CapControl carries no `Iterminal`,
so its `PhaseLosses` is zero on both sides). **All eight are `exclusion`s**, which the
per-sub-channel staleness rule below does not police — what stands behind each is
the per-entry measurement, and the `divergence` half of the same rule was proved
live by a deliberate eleventh widening of `r4133-indmach-injection-ulp` that an
unfiltered run reported STALE before it was reverted.

G1.3b (2026-09-05) is the third, and the widest: the three sequence names join in
the commit that starts comparing them, and both handlers honour them on the
`seq_slot_is_banded` predicate above (`envelope_element` bands each banded slot
with the comparator's own `harness::seq_band` / `harness::seq_power_band` over
the same oracle-side `CurrentsMagAng`/`VoltagesMagAng` magnitudes, the r4133
`SEQ_C012` term keyed on the entry's own channel; `rewrite_element_selected`
copies the snapshot straight through, both sides already being kW/kvar). **31**
per-sub-channel widenings over **11** of the 15 committed `element` scopes
followed, each **only** after the live gate printed its own failing sample on its
own channel (`measured.g13b_seq_first_failure`), iterating to a fixpoint —
`makeposseq-cuf-applied-capi`, `mmf-accept-set-honoured-capi`,
`gic-pct-r2-honoured-{gictransformer,midi}-{capi,r4133}`,
`windgen-qmode0-constant-q-{daily,snapdelta,dyn,dynfault}-r4133` on all three
names, and `capi-capcontrol-time-bus-is-the-capacitors` on `seq_voltages`
**alone** (a control element carries no terminal current, so its
`seq_currents`/`seq_powers` compare clean). The four `r4133-*-injection-ulp`
entries were again measured **not** to fail on any of the three and keep their
committed lists. **0 new entries and 0 new causes** (58 / 31 unchanged): every
sequence divergence in the corpus is the 012 image of the same already-triaged
cause as the rectangular channels the scope already named.

G1.3c (2026-09-06) is the fourth and closes the flag: `cplx_seq_currents`,
`cplx_seq_voltages` and `total_powers` join `SUBCHANNEL_FIELDS`
(`crates/dss-core/tests/corpus_gate/ledger.rs:765`) in the commit that starts
comparing them, taking the `element` row to **thirteen** names. The complex pair
rides the *same* `seq_slot_is_banded` predicate as the magnitudes — modes 13/14
are the same `Calc*` output read without the `Cabs`, so the arms and the banded
slots are identical — with `envelope_element` bounding the complex modulus at the
comparator's own `harness::seq_terminal_bands` numbers (r4133's `SEQ_C012` term
keyed on the entry's own channel) and `rewrite_element_selected` writing both
halves straight through; `total_powers` is banded per terminal by
`harness::total_power_band` and rewritten unscaled, both sides being kW/kvar.
**31** per-sub-channel widenings over **11** of the 15 committed `element` scopes
followed — six scoped rounds to a fixpoint, each widening only entries whose own
channel had just printed its own failing sample
(`measured.g13c_cplx_first_failure` / `measured.g13c_total_powers_first_failure`)
— ten entries on all three names and
`capi-capcontrol-time-bus-is-the-capacitors` on `cplx_seq_voltages` **alone**,
with the four `r4133-*-injection-ulp` entries again measured **not** to fail.
**0 new entries and 0 new causes** (58 / 31 unchanged). Two guarantees the
handlers owe and the fixtures pin: **no `cplx_seq_*` scope can mask a discrete
miss** — the `(-1, 0)` not-available payload and the exact zeros beside the
positive-sequence slot are neither envelope-checked nor rewritten
(`a_discrete_cplx_slot_is_never_neutralized_by_a_scope`,
`the_cplx_rewrite_and_the_cplx_envelope_cover_the_same_slots`) — and no
`total_powers` scope can mask a **shape** miss, the terminal walk being
`.take(nterms)` over both sides' lengths so an over-long payload reaches the
comparator's own length assert
(`a_discrete_total_power_shape_is_never_neutralized_by_a_scope`). The 0-terminal
capi sentinel (two doubles from the `NodeRef = NIL` early return) is
deliberately **not** envelope-checked — it is not a reading — and that inertness
is itself pinned (`a_zero_terminal_total_power_sentinel_is_not_envelope_checked`).
Each of the two new comparators has its own `require_capture` rail with its own
sentence, because the four element comparators read disjoint capture fields —
`require_capture` on `tp_kw`
(`crates/dss-core/tests/corpus_gate/runner.rs:1117`), on `cseq_i_re` (`:1128`) and,
since the G1.3c audit settlement (2026-09-06), on `cseq_v_re` (`:1145`): the
voltage half is the one field of this surface no other rail counts, and the one
whose guards differ per channel (capi's `CplxSeqVoltages` alone tests
`NodeRef = NIL`, `CAPI/CAPI_Alt.pas:878`), so a transport that dropped exactly
that key would otherwise reach the comparator's length assert rather than a
sentence naming it. All three sentences were driven empty on **both** channels
(capi `oracle_server.py`, r4133 `capture.rs` + a worker rebuild — see the trap
above); none records a census (D24).

**A widened sub-channel has to keep masking something** (G1.3a audit settlement,
2026-09-04). `applied`/`exceeded_floor` are per ENTRY, so a scope widened onto a
sub-channel that diverges by nothing rides on a sibling channel's divergence for
ever and fail-on-stale cannot see it. `Scope::channels_exceeded` therefore
attributes every floor-exceed to the sub-channel that produced it, and
`assert_all_hit` reports each `channels` name of a **`divergence`** entry that
never exceeded (`a_widened_sub_channel_that_masks_nothing_is_reported_stale`).
Its first run pruned four dead masks: the `powers` sub-channel of the four
`*-injection-ulp` entries, whose p_kW/p_kvar samples are all inside the tier
floor (the comparator's own power floor, `assert_power_close`'s voltage-scaled
`abs·max(1,|V_kv|) + rel·|P|`, is looser still, so dropping the mask cannot red
the case). An **`exclusion`** entry is not policed this way, for the reason
`LedgerView::excluded` gives: it names whole artifacts the runner never fetches
a verdict for, so there is no measurement to attribute; its sub-channels stay
backed by the `measured` provenance the entry itself carries.

**Why that asymmetry stays** (recorded 2026-09-05, G1.3d(ii) audit settlement,
after the `exclusion` scopes were widened onto `phase_losses`): an `exclusion` is
what a cause gets precisely when its channel cannot be measured *reliably*. The
worked case is the one the settlement met — two of the widenings it made sat on
GICTransformer decks whose pinned capi 0.14.5 oracle disagrees with itself across
fresh processes (coordinator decision **D12**), so a
"this mask caught nothing on this run" verdict would be a coin flip and the
resulting STALE report a flaky gate — the opposite of fail-on-stale's purpose.
(Those two capi entries are gone on `update`, D12/D14 having moved their decks to
`r4133`; the reason is kept because it is the general rule, and the structural
half above — an exclusion names artifacts no verdict is ever fetched for — holds
on every channel.) An
exclusion's liveness is therefore carried by its own recorded first failure
(`source` + `measured.*`, both mandatory) and by the entry-level "every entry
must be hit" rule, not by a per-run re-measurement. Widening one is a *measured*
act (`GOLDEN_REBASE_PLAN.md` §G1.3d AS EXECUTED lists each of the ten with its
sample); pruning one is a manual re-measurement.

**Adding a comparison surface to the ledger** — the recipe every WP-G1 surface
sub-step follows, since no field or handler may exist before the comparator it
serves (a dead handler is a promise the gate does not keep):

1. one name in `LEDGER_FIELDS`;
2. the kind split — `EXCLUSION_ONLY_FIELDS` if no handler re-asserts an
   envelope for it, otherwise `EXCLUSION_FIELDS`;
3. `EXCLUSION_FIELDS_WITH_PARTITIONING_HANDLER` as well if the exclusion
   partitions the artifact rather than dropping it whole;
4. `SUBCHANNEL_FIELDS` if the comparison has named sub-channels;
5. one runtime handler, plus a case in the synthetic drive
   `every_exclusion_field_is_honoured_by_the_runtime`, so a field with no live
   entry is still proven to apply.

A surface whose values are a *function* of an already-ledgered field adds
nothing here: it consumes that field's accepted scopes instead of re-pinning
their echo — the G1.9 aggregates are the worked example (above), and they added
0 fields and 0 entries.

`compile_scope` panics on a field outside `LEDGER_FIELDS`, naming
`global_result` as the worked example — still accurate: it has a comparator and
no ledger handler, so it is deliberately not a ledgerable field yet.

**The ledger is not a tolerance.** Envelopes are per-case, per-channel,
per-scope **measured facts** (size them with `DSS_LEDGER_MEASURE=1`, record
`measured` provenance) that assert a *specific known upstream divergence* keeps
holding; the calibrated tier floors apply unchanged everywhere else. A
divergence may be ledgered **only after** it is proven NOT to be a port bug
(CLAUDE.md divergence rules) — an entry that papers over a fixable bug is the
worst outcome (UNIFIED_GATE_PLAN §5-R3).

### Anti-shrink population lock (`population_lock.rs`)

The mandatory gate defines its own population — the four manifests — so a port
regression could be silently neutralized by moving a deck out of the gate, or
weakening it in place, in a one-line manifest edit.

`tests/corpus/manifests/population.lock.json` is a committed fingerprint of the
population: per-manifest case counts, and for **every case in all four
manifests** its path plus a per-case rigor fingerprint — kind/tolerance-tier,
`n_steps`, every compare-depth flag (selected_elements/meters-monitors/probes/
variables/eventlog/ctrlqueue/all-properties/global-result/autoadd-log, then the
ten WP-G1 surface flags derived/elemx/bus/zsc/rel/pde/topo/incm/runf/di, then
pending/solve-abort), `engines`, `isolate`, `defer`, and the per-channel ledger
entry digests. `population_lock.rs` (unconditional, plain `cargo test`) rebuilds the
fingerprint from the current manifests + ledger and asserts it equals the lock;
any drift — a path leaving `solvable_now`, a retained deck weakened in place, a
`both → capi_v0145` engine narrowing, a new/widened ledger entry, or any count
change — fails with a precise diff and the one-command regeneration path, so a
shrink lands as a **reviewable diff in the lock file**, never unnoticed.

**Every compare-depth flag must own a rigor token.** A flag that never reaches
the fingerprint is invisible here: switching it **off** on a case that had it on
would leave `population.lock.json` byte-identical — exactly the silent shrink
this file exists to make loud. `RIGOR_FLAG_TOKENS`
(`crates/dss-core/tests/population_lock.rs:543`) pairs every `compare_*` field of
`SolvableCase` with the token it occupies, and
`every_manifest_compare_flag_has_a_rigor_token` reads both source files and
refuses drift in either direction — a field with no row, a row with no field, a
token the `format!` string does not actually emit, a duplicate token. It is a
source-text gate in the `oracle_parity_cfg_gate.rs` style, so the *eleventh*
flag is safe even though nobody will remember this rule (GOLDEN_REBASE G1.0).
Its scanners strip **any** visibility keyword, not just `pub(crate) ` — a
`pub compare_x: bool` used to escape every assertion, one spelling of the very
drift the guard exists to catch — and both are driven over synthetic source text
by `the_drift_guard_scanners_see_every_visibility_and_every_flag_row`.

**The lock records the MANIFEST flag, not the scheduler's effective one.**
`population_lock.rs` is a manifest-only reader in its own test binary and cannot
link `corpus_gate`; recording the effective flag would mean a second copy of
`force_properties`' rule, free to drift silently — precisely the failure the lock
exists to prevent. The effective set is instead pinned where it is computed, and
more strongly than a lock column could be: `corpus_gate/scheduler.rs` re-derives
the forced population from the four manifests on **every** run and asserts it
against `FORCED_PROPS_POPULATION` = (443, 312, 87, 44), which also pins the
per-`engines` split. G1.3a added the second such rule and its own pin:
`force_derived` turns `compare_derived` on for every live case except
`kind=large*` on the `solvable_now` arm, and `FORCED_DERIVED_POPULATION` =
(445, 314, 87, 44) — the same 443 plus the two `Test/AutoTrans/{Auto3bus,AutoHLT}`
manifest opt-ins, both `engines: "both"` — is re-derived on every run by
`the_derived_forcing_rule_is_every_live_non_large_case_plus_the_opt_ins`. The two
opt-ins are listed in Rust as `DERIVED_MANIFEST_OPT_INS`, each required to be a
live case the rule would not already cover. G1.4a added the third such rule and
its own pin: `force_bus` turns `compare_bus` on over the same live non-`large*`
population, and `FORCED_BUS_POPULATION` = (443, 312, 87, 44) is re-derived by
`the_bus_forcing_rule_is_every_live_non_large_case` on every run — the manifests
set the flag on no case (`bus=0` on all 526), so this const is the only guard
the lock cannot supply. G1.3d(i) added the fourth:
`force_element_extras` (`crates/dss-core/tests/corpus_gate/scheduler.rs:203`)
turns `compare_element_extras` on for every live case except `kind=large*` on the
`solvable_now` arm — no opt-in table, because fastdss's `KNOWN_COM_DIFF` carries
no row for any of the five fields — and `FORCED_ELEMENT_EXTRAS_POPULATION` =
(443, 312, 87, 44) (`scheduler.rs:231`) is re-derived on every run by
`the_element_extras_forcing_rule_is_every_live_non_large_case` (`:803`). It is the
same 443 as `FORCED_PROPS_POPULATION`, which is the same rule without opt-ins.
**Cost (G1.3d(i)):** the five extra reads per element bought **no measurable gate
time** — `cargo test -p dss-core --test corpus_gate` measured 218.7 s / 179.3 s /
180.6 s / 180.9 s (parity) / 164.8 s across five drives, plus the post-D19
re-drive's 201.7 s (default) / 180.6 s (parity), inside this machine's own
run-to-run spread on the G1.3a tree (177.6 s … 198.6 s over five runs), with the
cleanest post-move run *faster* than every G1.3a measurement. None of these runs
had the machine to itself (D7 lanes gate concurrently from separate `target`
dirs), so a number tighter than "no signal above the ±20 s spread" would need
repeated paired runs on a quiet machine.
**Cost (G1.3d(ii)):** the six further reads per element (one of them a
`2·NPhases` array) again show **no signal above the concurrency spread** on the
default lane — the final unfiltered `cargo test -p dss-core --test corpus_gate`
drive measured **181.07 s**, inside G1.3d(i)'s own 164.8 s … 218.7 s. The parity
drive of the same bytes measured **246.93 s** against G1.3d(i)'s 180.6 s /
180.9 s, and that gap is **not** attributed here: none of these runs had the
machine to itself (D7 lanes gate concurrently), and the intermediate drives of
the same tree ranged 232 s … 650 s purely with load — one of them tripping the
fixed 120 s per-case oracle budget on the three `large_ultra_switch` `ckt24`
decks, which this surface does not even flag (they pass in 61.85 s when the
worktree is quiet, D13). A paired quiet-machine measurement is what would settle
it, and none was run.
**Cost (G1.3c):** the three further reads per element — one of them the group-A
`TotalPowers`, two of them `2*3*NTerms`-double arrays — again show **no signal above
the concurrency spread**. Measured inside the sub-step's own full `cargo test
--workspace` drives on a quiet worktree (D13), `corpus_gate` took **174.33 s**
(default) and **162.45 s** (parity), against G1.3d(ii)'s 181.07 s / 246.93 s and
G1.3d(i)'s 164.8 s ... 218.7 s — i.e. the widest surface of the three landed at the
*fast* end of the existing band. The sub-step's earlier ledger-fixpoint drives of
the same bytes measured 599.84 s / 672.84 s (default) and 175.41 s (parity) for
`--test corpus_gate --test population_lock` while four other lanes were gating from
their own `target` dirs; that ~4x is load, not surface — the same tree, minutes
apart. A paired quiet-machine measurement is still what would resolve the per-read
cost, and none was run.
**Cost (G1.3a, Q-8):** the three extra
reads per enabled element cost ~+3 µs/element on the capi channel and roughly
double the per-element JSON payload; end to end
`cargo test -p dss-core --test corpus_gate` went **≈155 s → 171.7 s** (default
lane, 165 tests, the 523-case gate inside it), i.e. **+11 %** for the first
surface — worth re-reading as G1.3b/c add theirs. **Obligation (GOLDEN_REBASE G1.0):** a scheduler force rule
for a WP-G1 flag ships in the same commit as its own `FORCED_<FLAG>_POPULATION`
pin and its re-derivation test — the RP4.1 precedent — or the effective
population is pinned by nothing at all.

**Regenerate the population lock** (deliberate — after intentionally
re-classifying decks or editing the ledger, never to silence an unreviewed
failure):

```
DSS_UPDATE_POPULATION_LOCK=1 cargo test -p dss-core --test population_lock
```

writes the lock from the current manifests. Commit the `population.lock.json`
diff **together with** the manifest/ledger change that caused it.

### Golden provenance lock (`golden_lock.rs`) and the self-golden write rails

The population lock above protects the *live* gate's population; this one
protects the **committed golden corpus** — and, unlike the population lock, it
also decides who is allowed to rewrite a golden at all.

`tests/golden/golden.lock.json` fingerprints every committed golden artifact:
the 727 files under `tests/golden/**` plus the registered out-of-tree witness
`crates/dss-core/tests/data/adiakoptics/r3723_ref/` (10 files) — 737 rows of
`{path, sha256, anchor, reason, produced_by}`.

| field | meaning |
|---|---|
| `sha256` | digest over the **committed** content — CRLF→LF for text artifacts, raw for the `reports/*.bin` streams (declared `binary` in `.gitattributes`; the test binds its classifier to that declaration, both directions, over every scanned path) |
| `anchor` | where the truth in those bytes comes from: `capi_v0145` (the pinned dss-python oracle), `r4133`, `r3723`, `capi015` (a dead 0.15.x beta stack), `fpc_3.2.2` (an FPC RTL print capture), or `self` (our own engine — a pure anti-regression snapshot of report *form*) |
| `reason` | mandatory on every `self` row (G3.6 extends the requirement to the rest) |
| `produced_by` | which lane may *write* a `self` artifact: `parity` or `lane-invariant`. `null` on every oracle-anchored row — nothing in this repo produces those bytes |

`crates/dss-core/tests/golden_lock.rs` (unconditional, plain `cargo test`)
asserts, fail-on-stale in both directions: every artifact on disk has a row,
every row has an artifact on disk, every digest matches, and every row's
`anchor`+`reason` equal what the in-test provenance registers (`DEANCHORED`,
`CAPI015_ARTIFACTS` — plus `CAPI015_OVERLAYS`, which appends the derivation note
for a capi015 artifact whose bytes a later WP partly overlaid from another engine
without moving its anchor — `R4133_FAMILIES`, `FPC_ARTIFACT`, `R3723_TREE`, with
`capi_v0145` as the residue) derive for its path — every register entry must
cover at least one row, and `anchor == self` holds **iff** `produced_by` is set.
Re-anchoring an artifact is therefore a reviewed Rust edit to a register, never
a lock hand-edit and never a side effect of pressing a regen button. Anchor
histogram today: 700 `capi_v0145`, 11 `capi015`, 11 `r4133`, 10 `r3723`, 1
`fpc_3.2.2`, 4 `self`. One golden-shaped tree is deliberately **outside** the
lock's scope and named with its reason in the test (`EXCLUDED_TREES`):
`crates/dss-metis/tests/golden`, the vendored METIS 5.2.1 partitioner fixtures,
which witness a third-party C algorithm rather than any DSS oracle.

**The write rails** (`crates/dss-core/tests/harness/regen.rs`).
`harness::regen()` arms `harness::snapshot_text()` / `snapshot_bytes()` when
`DSS_UPDATE_GOLDENS` is set; without the knob they are inert, so a driver can
never bless the output it is about to compare. Armed, every write passes two
guards read from the lock:

- **anchor** — an artifact anchored anywhere but `self` is refused: those bytes
  are another engine's capture, and de-anchoring is the reviewed register edit
  above. This is what keeps the frozen sets (`capi015`, `fpc_3.2.2`, `r3723`,
  the r4133 families) unwritable while a family regen sweeps past them.
- **producing lane** — until WP-G4 the two lanes render different report/JSON
  bytes, so a `parity`-produced family is refused from the default build.
  `lane-invariant` — set only after a cross-lane regen has *measured* it, never
  assumed — is writable from either lane. A self-golden is produced by the lane
  holding the family's strictest contract, which today (parity-only byte arms in
  `lane::compare_report` / `lane::compare_json`) is the parity lane.

A refusal is a **skip**, not a panic — a family regen legitimately sweeps
artifacts it must not touch — announced on stderr with its remedy (visible only
with `-- --nocapture`; see the procedure below) and returned to the call site as
a `#[must_use]` `Outcome`, leaving the artifact byte-identical. The rails
**read** the lock and never write it: after a
regen run `golden_lock.rs` is red with `DIGEST MOVED` until the operator reviews
the diff and regenerates the lock deliberately (below). That red is the feature.

No golden driver calls the helpers yet — wiring them is WP-G3 of
`GOLDEN_REBASE_PLAN.md`; the rails, their guards and the R1–R4 rules below land
first, so the capability to rewrite a golden never exists unguarded.

### Vendored r4133 property census (`props_r4133_evidence_lock.rs`)

`tests/corpus/props_r4133/` is frozen **evidence**, not a golden: five
byte-identical copies of extracts whose source is local-only and gitignored,
plus five derivations of a 270 MiB census that stays out of the repo
(`R4133_PROPS_PLAN.md` RP0.1; the directory's own `README.md` is the map).
It is deliberately outside `golden_lock.rs`'s scope — recorded there in
`EXCLUDED_TREES` — and locked instead by
`crates/dss-core/tests/props_r4133_evidence_lock.rs` (unconditional, plain
`cargo test`, no oracle): SHA-256 + length over the five verbatim copies and the
`.gitattributes` `-text` stanza that keeps them raw, the derived files' row
counts, their cross-file equalities, the plan §1.1 per-bin totals, and the two
counted data traps the README documents. From RP2.1 on `examples_full.txt` is
the *input* of the replay-accounting test, where a silently dropped row would
shrink what that test proves instead of failing it — hence the lock. There is no
regeneration env var: re-measurement is the RP0.2 `DSS_PROPS_CENSUS` knob, and a
disagreement between the knob and these files is a finding, not a rewrite.

**Re-measuring (`DSS_PROPS_CENSUS=1`).** The knob reproduces the walk that
produced these extracts: every live non-`large` case on BOTH channels, the r4133
property masks bypassed, the plain comparator in collect-don't-panic mode.

```
DSS_PROPS_CENSUS=1 cargo test -p dss-core --test corpus_gate \
    corpus_gate_props_census -- --nocapture
```

The census is its **own** `#[test]` (`corpus_gate_props_census`), not a diversion
of the mandatory gate: the var arms it and nothing else, so a stray
`DSS_PROPS_CENSUS=1` left in a shell can never turn
`corpus_gate_all_cases_match_engines` into a green no-op. Unset, the census test
is a no-op. Name it in the filter as above — otherwise the same run also executes
the full live gate.

It writes `tmp/props_census.json` (one row per divergent cell, plus a `channel`
column the vendored r4133-only census does not have) and
`tmp/props_census/<channel>/{structural_pairs,numeric_pairs,examples_full,shape,
summary}` plus `tmp/props_census/run.json`; add `DSS_GATE_ONLY=<substr>` for a
bounded run — the filter is stamped into the census header's `gate_only` and into
`run.json`, so a family-bounded artifact set can never be mistaken for a full
census. `bins.tsv` and the three `*_in_scope` extracts are NOT re-derived: they
need the §1.1 bin policy and the in-scope case filter.

**The disposition mode (`DSS_PROPS_CENSUS=claims`, RP2.1).** Same walk, same
rows, same silence — each **value** row additionally annotated with what the
r4133 value policy does with that cell: `normalized-by-<rule>` (the
`PROPS_NORM_R4133` table) / `echo-row` (RP2.3's table) / `under-floor` (RP2.4's
floor) / `ledger-hit` (a `property`-scoped `ledger.json` entry names it) /
`UNCLAIMED`. Every verdict comes from the shipped predicate the live comparator
uses (`harness::props_norm::claim_value`, `LedgerView::property_scope_keys`),
never a copy of it, and the mode also carries the in-scope flag
(`engines ∈ {both, r4133}`) so its tallies line up with `bins.tsv`'s
`cells_in_scope` column. The verdict is **channel-scoped**: both channels' rows
are annotated, but the first three links are r4133 mechanisms and answer nothing
on capi, so a capi row is only ever `ledger-hit` or `UNCLAIMED` — the capi zero
is the contract, not a measurement. A spelling whose cells disagree (only the
per-(case, channel) ledger link can do that) is reported as the **weakest** of
them and counted in `claims_summary.json`'s `mixed_disposition_spellings`; the
per-row `disposition` in `props_census.json` stays lossless. **Read an acceptance
question off `props_census.json`, never off the two per-spelling artifacts** —
after RP4.1 landed its two `swtcontrol.delay` entries the summary still reports
that spelling as `UNCLAIMED` with 24 in-scope cells, because its out-of-scope
cells are UNCLAIMED and the fold takes the weakest; since the RP4.1 audit
settlement (2026-09-03) both artifacts carry that rule in themselves (a
`reading_rule` field and a header note). It adds three per-channel files — `claims.txt`
(`examples_full.txt`'s rows plus `count_in_scope` and the disposition),
`claims_unclaimed_pairs.txt` (the pairs that still owe a rule/exclusion row) and
`claims_summary.json` (per-disposition cell tallies, zeros included) — and two
columns to `props_census.json`'s rows. **Plain mode is untouched by all of it**
(byte-identical artifacts, verified A/B), and a plain run removes a previous
claims run's three files rather than leaving them to be misread.

This is the per-cell counterpart of the offline replay below: the replay proves
spelling-level completeness before RP4.1's unmask, the claims census reads
cell-level closure after it (`R4133_PROPS_PLAN.md` RP4.1's acceptance is **zero
UNCLAIMED cells** in scope).

### The r4133 props replay accounting (`props_r4133_replay.rs`)

`crates/dss-core/tests/props_r4133_replay.rs` (RP2.1, unconditional plain
`cargo test`, no oracle and no solve) pushes every example row of the vendored
`examples_full.txt` **plus** `examples_supplement.txt` through the r4133 claim
chain in its documented order (shape allowlist → normalization → echo table →
display floor), asserting that each row is claimed by the first matching link or
is **declared** for the sub-step that will claim it (including the cells an echo
row deliberately does not cover — `props_norm::ECHO_CARVE_OUTS` and their
`ECHO_CARVE_OUT_ROUTING` owner, matched both ways), that every
`PROPS_NORM_R4133` row claims at least one row (offline liveness — the live half
is the per-row hit accounting, dormant until RP4.1's unmask and live since it,
2026-09-03), and that each table row's
`(pair, bin, cells)` citation matches the vendored evidence it names. The
supplement (`tests/corpus/props_r4133/examples_supplement.txt`) is measured, not
frozen: the 26 pairs no 2026-08-08 row can carry (the WP-RP1 shape closures plus
the two `regcontrol` threshold pairs), and the replay parses the README's WP-RP1
tables directly — **reformatting that README section reds a test on purpose**.
Where the vendored evidence is knowingly behind the live population it says so
with an assertion rather than a comment: `LIVE_ONLY_SPELLINGS` names the one
spelling the claims census sees and no file may carry (`autotrans.conn
'series'/'Series'`, on a pair whose `bins.tsv` row predates RP1.2's deck),
reconciles the replay's normalization-link spelling count with the census's
(748/749 at RP2.1, 854/855 at RP2.3, **852/853 since RP4.1** — the two dead
`swtcontrol` `ArrayForm` rows dropped from both sides at once), and pins that the
shipped rule still claims it.

Since **RP2.2** the file also carries that sub-step's dossier as data:
`RP22_ROUTING` is one row per pair RP2.2 read and routed, each citing the r4133
`.pas:` line that decides it, and `rp22_settled_every_pair_it_was_handed` proves
its closed input list is partitioned into "claimed by `PROPS_NORM_R4133`" and
"routed with a citation" with no third state. A pair on that list which neither
the chain claims nor the routing names is an **error**, not a declaration — the
same for any bin-3 cell anywhere — because RP2.2's acceptance is that bin 3 is
claimed per cell, not per pair.

Comparing against the vendored files: filter to `channel == "r4133"`, drop that
key, and compare **cell multisets** — the pair extracts' `example` cells and
`bins.tsv` labels are representative-cell artifacts and are case-order dependent
on the 17 heterogeneous pairs the directory's `README.md` tables. **Expect three
known differences on the r4133 side, all documented in that README**: the two
RP0.2 incompleteness corrections (§"Corrections measured after freezing") and one
*policy* change — RP2.1 unmasked `RegControl.RevThreshold` on r4133, so a
re-census reports the numeric pair `regcontrol.revthreshold` (888 cells, an
`EchoDefault`) that no frozen extract carries (§"The RP2.1 policy change a
re-census now reports"). The `capi_v0145` channel is unaffected by all three. The `channels`
block reports what the walk could NOT look at, so a partial census never reads as
a complete one: `unaligned_cells` counts what a desynchronized name list hides
from any index-ordered compare (an insertion at property *k* makes every cell
after *k* on that element uncomparable — the reason the vendored value population
of the five shape-gap classes is a lower bound; WP-RP1 closed all five, and each
sub-step recorded the pairs its closure made live in the vendored `README.md`
§"Pairs the WP-RP1 shape closures make live" — the knob now reports
`unaligned_cells` 0 and 0 shape classes on both channels);
`skipped_elements`/`skipped_element_cells` count elements dropped WHOLE (the
capi-only Recloser/Relay skip, invisible to `unaligned_cells`); and
`heterogeneous_shape_classes` counts classes whose members do not share one
property-table shape, which the one-row-per-class `shape.txt` cannot show.
`oracle_errors`/`rust_errors` close the same gap for a case that fell over rather
than diverged: a channel hiccup turns that case's whole divergence population
into ONE `oracle_error` row, so a run can come back short without walking fewer
cases. They ride the **printed** per-channel summary line as well as the JSON
(RP1.4 audit — a short run used to be invisible on stdout). Measured on the
post-RP1.4 tree (2026-08-23): **5** on r4133 (the #303 crash decks the vendored
census also carries) and **22** on `capi_v0145` — the decks the 0.14.5 oracle
cannot compile or solve at all, i.e. post-0.14.5 spellings (`xfmrcode`,
`curvemultiplier`, `singlephtrip`, `phfastcurve`, `idle`, the WindGen class, the
`modes:upgrade/*` family) plus two capi-side aborts. A count above the baseline
means this run measured less than the recorded census, and its totals must not
be compared against the recorded numbers.

### The r4133 echo-exclusion pins (`props_r4133_pins.rs`)

`crates/dss-core/tests/props_r4133_pins.rs` (RP2.3, unconditional plain
`cargo test`, no oracle) holds the expected-value tests every
`PROPS_ECHO_R4133` row names in its `EchoWitness`. Each one compiles the vendored
corpus deck the claims census flagged for that pair, reads the port's live render
with `? Class.Name.Prop` — the same getter the gate's property walk reads — and
asserts it literally, plus a discriminating second reading so the assertion is
about the value and not about a constant. They live in their own binary on
purpose: a pin in `tests/harness/` would recompile and re-solve every deck in
each of the 22 binaries that include the harness.
`props_r4133_replay::every_echo_row_pin_is_a_test_that_exists` reads the names
back both ways, so a renamed or orphaned pin fails rather than leaving a row
citing a witness that is not there (its `NOT_A_PIN` exemption list is pinned
literally beside it).

**How wide a row is.** 62 of the 82 rows are *pair-scoped* (the shape plan
§1.2 prescribes, `SKIP_PROPS`'), minus the one cited counterexample in
`props_norm::ECHO_CARVE_OUTS`. The other 20 — the pairs that also carry a
`PROPS_NORM_R4133` normalization row — are **per cell** since RP4.1's
precondition 1: `props_norm::ECHO_NARROWED` lists the 66 measured `(rust, r4133)`
spellings those rows cover, and a cell of such a pair that the typed rule
*refused* (a wrong resolved loadshape name, a wrong ZIPV vector, a wrong `Bus2`
terminal spelling) is compared on r4133 instead of being swallowed by the pair.
The 66 are derived from the frozen census rather than chosen —
`props_r4133_replay::the_narrowed_echo_rows_carry_exactly_the_spellings_the_typed_rules_leave`
recomputes them from `tests/corpus/props_r4133/` and matches the table both ways.
Because they are derived *mechanically* (the pair's census rows the typed rule
does not claim), the narrowing changes no cell on the measured population; what
it buys is the next, unmeasured spelling, which is compared instead of inheriting
the citation. Staleness is reported for them the other way round
(`props_norm::check_echo_rows_are_live`, RP4.1 audit settlement): a narrowed row
counts only cells it covers, so `visits == hits` always and the "excluded
nothing" arm cannot fire — instead a narrowed row that is **never visited** in a
full run is the stale one.

**Which rows owe a pin** — plan §1.2 mechanic (c), both halves. A row whose pair
the capi channel cannot compare at all (`PROPS_015X`, `SKIP_PROPS`, the
whole-element skip) obviously does; so does a row that masks cells on
`engines: "r4133"` **cases**, where the capi channel never runs — measured by
crossing the claims census with each case's manifest flag and recorded as
`props_norm::ECHO_ROWS_ON_R4133_ONLY_CASES` (**58** of the 82 rows, **34 971**
cells — the 57/34 969 first written here predates RP3.3's `generator.model` row;
re-derived from the table on 2026-09-04, RP5.1), with
`every_row_exposed_on_r4133_only_cases_names_a_pin` enforcing it. That rule is
why the file holds **30** pins covering **64** of the 82 rows rather than the 20
RP2.3 first landed (RP2.3 audit settlement, 2026-08-23). The complementary
guard `a_capi_witness_is_a_pair_the_capi_channel_can_compare` keeps a `Capi(n)`
witness from naming a pair that channel never sees.

Two of the flagged decks write into the
vendored tree while they run (`Test/TD21RelayTest.DSS` ends in `show eventlog`,
`StorageControllerTechNote/Schedule/ScheduleRun.dss` in nine `Export`s); the
file's own `DeckDirGuard` sweeps what a run created and fails if a run changed a
vendored file, the same contract the corpus gate's `CorpusGuard` carries for the
live walk.

### 0.15.x property-table allowlist (`PROPS_015X`)

The corpus gate's property-parity check (`harness::compare_all_properties`)
asserts the Rust property-table **shape** (count + name order) against the
oracle capture. It runs on **both channels** since R4133_PROPS RP4.1
(2026-09-03) deleted the per-channel mask the scheduler used to apply to the
r4133 request: `force_properties` (`corpus_gate/scheduler.rs`) now forces the
compare on every live non-`large` case whatever its `engines` key, and all three
synthetic families force it on for their live cases. That population is pinned,
not described: `scheduler::the_property_forcing_rule_is_every_live_non_large_case`
walks the four manifests with no oracle and asserts the forced set is exactly the
live non-`large` one — **443** cases = 312 `both` + 87 r4133-only + 44 capi-only
(`FORCED_PROPS_POPULATION`) — which is what makes a *partial* re-mask loud;
`props_norm::assert_r4133_props_compare_ran` only sees a wholesale one. The
guard's cost is that the **54** `kind=large*` `both` decks (and 14 r4133-only,
11 capi-only) compare no property table at all. (Until RP4.1 it was
capi_v0145-only, which is why the allowlist is framed around the 0.15.x-shaped
tables r4133 renders — on an r4133 capture an r4133-only prop is carried by the
oracle's own name list, so it is kept by `filter_015x` and compares in full.)
The pinned oracle is dss_capi
**0.14.5**, so a deliberately ported 0.15.x property (which cannot appear in a
0.14.5 capture) is declared in the named per-class allowlist `PROPS_015X`
(`tests/harness/mod.rs`): a Rust-side prop in the allowlist and absent from the
capture is excluded from the shape walk (handles inserted props, not just
trailing). Present-in-capture props are NOT excluded — full name+value compare
still applies. It relaxes shape only, never a value tolerance, and a
non-allowlisted extra/missing/misordered prop still fails. The test is
channel-agnostic — it asks only whether **this** capture's name list carries the
prop — so one row runs the other way round: `GenDispatcher.weights`
(R4133_PROPS RP1.4) is a prop the 0.14.5 capture reports fine and **r4133's own
table loses to a registration off-by-one**, so it is inert here and relieves the
r4133 shape walk only. Rules + row-documentation requirements:
`tests/TOLERANCE_NOTES.md` §"0.15.x property-table allowlist (shape
relaxation)".

### The r4133 property policy — the claim chain (R4133_PROPS)

Since RP4.1 unmasked the r4133 property request (§`PROPS_015X` above), a
property cell of a live non-`large` case is asserted on **both** channels. The
two channels do not spell values identically, so the r4133 side runs a
**channel-scoped claim chain** whose links are consulted in one fixed order and
never on `capi_v0145`. One function holds the whole order —
`harness::compare_prop_lists` (`crates/dss-core/tests/harness/mod.rs:9445`) —
and links 2-4 are `PropsPolicy` methods gated on `is_r4133` (`mod.rs:9725`,
`:9764`; the channel type is `PropsChannel`, `mod.rs:9615`). Link 1 is the
deliberate exception: `skip_prop` is a free function taking the channel, so its
`LANE_SKIP_PROPS` half stays channel-blind (row 1 below says so).

| # | link | seam | what it does | if it does not claim |
|---|---|---|---|---|
| 0 | shape allowlist `PROPS_015X` | `filter_015x`, `mod.rs:9245` | drops a Rust-side prop the capture cannot carry — **shape only** | the name walk fails |
| 1 | skip rows `SKIP_PROPS` / `LANE_SKIP_PROPS` | `skip_prop`, `mod.rs:8225` (channel rule at `:8226`) | value-only skip, per channel | fall through |
| 2 | normalization `PROPS_NORM_R4133` | `PropsPolicy::normalize`, `mod.rs:9718` | **re-spells** the oracle side when a typed rule proves the two are the same value | both raw spellings continue |
| 3 | echo table `PROPS_ECHO_R4133` | `PropsPolicy::echo_excluded`, `mod.rs:9732` | drops the **value** compare of that cell (name + index order still assert) | fall through |
| 4 | display floor `R4133_DISPLAY_FLOOR` | `PropsPolicy::under_display_floor`, `mod.rs:9740` | passes a numeric cell that is our value rendered to r4133's own digits | the cell reaches the assert |
| 5 | the assert | `assert_value_matches_tol`, `mod.rs:365` | the case's tier floors (`tol_for`) | **gate red, both spellings in the message** |

A divergence the ledger owns is handled outside this chain, by the case's
`property`-scoped `ledger.json` entry (`corpus_gate/ledger.rs:1413`, `:1433`) —
which is why the triage order below ends there and not before.

**Link 2 — the normalization table** `PROPS_NORM_R4133` (`harness/props_norm.rs:560`). **168 rows**
of `(class, prop)` → `NormRule`, each citing the vendored census by
*(pair, bin, cells)*. The contract is **value-preserving spelling only**
(`NormRule::claims`, `props_norm.rs:911`): a rule may change how a value is
written, never which value it is; anything else is an exclusion, not a rule.

| rule | rows | folds |
|---|---|---|
| `BoolFold` | 77 | `{yes,y,true}` / `{no,n,false}`, case-insensitively — `''` is **not** a boolean |
| `CaseFold` | 65 | case + leading/trailing blanks |
| `ArrayForm` | 21 | delimiters/brackets only; a token-count difference is a value difference |
| `EnumSynonym` | 5 | an explicit per-row spelling map, each mapping citing the r4133 line that prints it |

*Count lock:* `props_norm::tests::count_locks_hold` asserts the total **and**
that the four per-kind locks (`NORM_ROWS` … `NORM_ENUM_SYNONYM_ROWS`,
`props_norm.rs:742-764`) partition it, so a row cannot be added without moving a
documented number. *Liveness:* `props_norm::assert_norm_rows_are_live`
(`props_norm.rs:1412`, called in the gate epilogue, `corpus_gate.rs:276`) fails
a full run in which a row was visited and folded nothing — the fail-on-stale
half. The offline half is the replay (§"The r4133 props replay accounting"):
every row must claim at least one vendored example row. Only the **live** half
reads `DSS_GATE_ONLY` (`props_norm.rs:1413`), and returns on it: a row spans
cases, so a filtered run cannot make a whole-population staleness claim and
makes none. The offline half needs no such check and has none — it reads the
frozen census files, not the gate population, so it runs unconditionally
(`props_r4133_replay.rs` holds zero `env::var` calls).

**Link 3 — the echo table** `PROPS_ECHO_R4133` (`props_norm.rs:1772`). **82 rows**, each a
value-only exclusion carrying (a) the r4133 `Version8/Source` line that proves
its category and (b) a **witness** that still holds the port's value
(`EchoWitness`, `props_norm.rs:1558`: `Capi(n)` cases, a named expected-value
`Pin`, or both).

| category | rows | mechanism |
|---|---|---|
| `EchoDefault` | 50 | r4133 has no `GetPropertyValue` arm and echoes `InitPropertyValues`' default |
| `EchoParse` | 8 | it echoes what the deck's own command string wrote |
| `EmptyCollectionRender` | 14 | an unset collection renders empty |
| `LiveSemanticsDiffer` | 10 | both sides render live state, but of different things |

*Count lock:* the same `count_locks_hold`, on `ECHO_ROWS` and the four category
locks (`props_norm.rs:2024-2032`), plus
`props_norm::tests::the_echo_table_is_the_measured_row_set`, which rebuilds the
row set from the frozen census. *Row width:* 62 rows are pair-scoped (minus the
one cited counterexample in `ECHO_CARVE_OUTS`, `props_norm.rs:2238`); the other
20 — the pairs that also carry a normalization row — are **per cell** since
RP4.1, covering only the 66 measured spellings of `ECHO_NARROWED`
(`props_norm.rs:2344`; locks `ECHO_NARROWED_PAIRS` `:2459` and
`ECHO_NARROWED_SPELLINGS` `:2463`), derived from the census by
`props_r4133_replay::the_narrowed_echo_rows_carry_exactly_the_spellings_the_typed_rules_leave`.
*Witness obligation:* **58** of the 82 rows mask cells on `engines: "r4133"`
cases (**34 971** cells), where the capi channel never runs; each must name a
pin (`ECHO_ROWS_ON_R4133_ONLY_CASES`, `props_norm.rs:2082`, enforced by
`props_norm::tests::every_row_exposed_on_r4133_only_cases_names_a_pin`), and the
converse guard `a_capi_witness_is_a_pair_the_capi_channel_can_compare` refuses a
`Capi(n)` witness on a pair that channel cannot compare. The pins themselves are
**30** expected-value tests covering **64** of the 82 rows, in
`crates/dss-core/tests/props_r4133_pins.rs` (§"The r4133 echo-exclusion pins"),
tied to the table both ways by
`props_r4133_replay::every_echo_row_pin_is_a_test_that_exists`. *Liveness:*
`props_norm::assert_echo_rows_are_live` (`props_norm.rs:2657`,
`corpus_gate.rs:287`) — `visits > 0 && hits == 0` for a pair-scoped row,
`visits == 0` for a narrowed one (a narrowed row counts only covered cells, so
`visits == hits` by construction and the first arm cannot fire on it).

**Link 4 — the display floor** (`R4133_DISPLAY_FLOOR` = 2e-4 rel,
`props_norm.rs:895`). The **only** tolerance this plan introduced: r4133-only,
property-cells-only, and a *cell* predicate — it reads the two rendered strings,
so no `(class, prop)` is masked by name and there is no row to go stale. Two
clauses, both necessary: the **metric** (`display_rel ≤ 2e-4`) and the
**mechanism** (`display_is_render`, `props_norm.rs:1082` — every number on the
r4133 side must be our number rounded to the significant digits r4133 itself
printed). It touches no `Tolerances` field, no `tol_for` tier, no golden and no
model quantity, and it is unreachable on `capi_v0145`
(`props_policy_tests::the_capi_channel_never_applies_the_display_floor`,
`mod.rs:8661`). Its derivation — the measured worst cell, the empty band, the
`%.Ng` site table and the 55 refused spellings — is
`tests/TOLERANCE_NOTES.md` §"r4133 props display floor".

**The `SKIP_PROPS` dispositions (plan §1.2).** `skip_prop` is channel-aware
since RP2.1 (`skip_prop`, `mod.rs:8225`), because after RP4.1 a channel-blind row would
value-mask the r4133 channel by accident. Every one of the **17** `SKIP_PROPS`
rows (`SKIP_PROPS`, `mod.rs:7782`) is dispositioned exactly once, in its own row comment —
**17 = 10 + 7**, the first two lists below. The third list is a separate table
(`LANE_SKIP_PROPS` is not a `SKIP_PROPS` row and the partition lock does not
union it), shown here because `skip_prop` consults it on the same call:

| list | rows | on r4133 | why |
|---|---|---|---|
| `SKIP_PROPS_CAPI_ONLY` (`mod.rs:8122`) | 10 | **compared** | the justification is a 0.14.5-capture fact: the three changed defaults (`Fuse.FuseCurve`, `Fuse.RatedCurrent`, `RegControl.RevThreshold`), the two `pctperm` rows (`Capacitor`, `Reactor`), and RP3.8's five `''`-render rows (`IndMach012.PF`, the four `StorageController` totals) |
| `SKIP_PROPS_BOTH_CHANNELS` (`mod.rs:8099`) | 7 | **skipped** | channel-independent facts — the heap-garbage matrix reads (`Capacitor.CMatrix`, `Reactor.RMatrix`/`XMatrix`, `Fault.GMatrix`, `Transformer.WdgCurrents`) and the two `FaultRate` rows |
| `LANE_SKIP_PROPS` (`mod.rs:8201`) | 1 | **skipped, deliberately channel-blind** | `(Monitor, BaseFreq)` — an upstream bug BOTH gating oracles share (`Monitor.pas` r4133:552); the port's correct value is pinned by `monitor_basefreq_inherits_the_fundamental` |

*Partition lock:*
`skip_props_disposition_tests::every_skip_props_row_has_an_r4133_disposition`
(`mod.rs:8269`) fails on a row listed twice, in neither list, or deleted from
`SKIP_PROPS` — a new skip cannot silently inherit "masked on r4133 too". The
channel-blindness of the `LANE_SKIP_PROPS` row has its own pin
(`the_monitor_basefreq_exclusion_is_channel_blind`, `mod.rs:8426`). The two
**whole-element** skips are channel-scoped the same way: Recloser and Relay are
skipped on capi only, because their Rust tables are r4133-shaped
(`skip_whole_element`, `mod.rs:9398`;
`recloser_and_relay_are_whole_element_skipped_on_capi_only`, `mod.rs:8453`).

**Did the chain run at all?** `props_norm::assert_r4133_props_compare_ran`
(`props_norm.rs:2841`) runs first in the gate epilogue (`corpus_gate.rs:264`),
so a wholesale re-mask reports as one line instead of 19 stale-row messages; a
*partial* re-mask is caught instead by the forcing-rule lock
`scheduler::the_property_forcing_rule_is_every_live_non_large_case`
(`corpus_gate/scheduler.rs:252`, `FORCED_PROPS_POPULATION` = (443, 312, 87, 44)
at `:175`).

**Where the rest of the machinery is documented:** the measurement knob
`DSS_PROPS_CENSUS` and its `claims` disposition mode — §"Vendored r4133 property
census" and the §Environment variables row; the offline accounting that proves
every table row claims a vendored example — §"The r4133 props replay
accounting"; the expected-value pins — §"The r4133 echo-exclusion pins"; the
frozen evidence itself — `tests/corpus/props_r4133/README.md`.


### The r4133 bridge — entry points, mode capability, do-not-call

The r4133 channel talks to the vendored `tools/opendss/bin/r4133/OpenDSSDirect.dll`
through `crates/dss-epri` (test-only, `publish = false`, `#[cfg(windows)]`,
`#![deny(unsafe_op_in_unsafe_fn)]` with module-level `// SAFETY` docs — no product
crate ever loses `#![forbid(unsafe_code)]`). This section is the map of what that
DLL can and cannot serve; the rails below landed in `GOLDEN_REBASE_PLAN.md`
sub-step G1.0 (2026-09-04) and are the executable half of its **G1.11′** record.

**It is a grouped API, not a per-property one.** The DLL exports one entry point
per (family, ABI shape) — `CktElementI/F/S/V`, `BUSI/F/S/V`, `CircuitI/F/S/V`,
`MetersI/F/S/V`, `TopologyI/S/V` (no `F`), `SolutionI/F/S/V`, `PDElementsI/F/S`
(no `V`), … — and the property is selected by an integer **mode** argument.
`FamilyTable` binds all of them at load and hard-errors on a miss: 42 families /
147 entry points (`entry_point_count`, `crates/dss-epri/src/families.rs:475`).
There are **no** `*_Get_*` symbols in this DLL — that spelling is dss_capi's — so
"the symbol is absent" is not a failure mode here, and `GetProcAddress` can never
tell you a property is missing.

**An unknown mode falls into the family's `else` branch and returns a
sentinel**, which is what `crates/dss-epri/src/modes.rs` classifies into
`ModeStatus::Served` / `UnknownMode` / `DoNotCall`. The sentinels were read out of
the vendored DDLL sources exhaustively and confirmed live:

| shape | unknown-mode reply | source |
| --- | --- | --- |
| `I` | `-1` | `DCktElement.pas:308`, `DBus.pas:75`, `DCircuit.pas:187`, `DMeters.pas:316`, `DTopology.pas:168`, `DSolution.pas:295`, `DPDELements.pas:120` |
| `F` | `-1.0` | `DCktElement.pas:414`, `DBus.pas:202`, `DCircuit.pas:207`, `DMeters.pas:397`, `DSolution.pas:455`, `DPDELements.pas:212`, `DCmathLib.pas:22` |
| `V` | `myType = 4` carrying one of "parameter not recognized" / "Command not recognized" / "parameter not valid" | `SENTINEL_V_PHRASES`; matched by **containment**, never equality |
| `S` | a **per-family** literal | `S_SENTINELS`; one row per WP-G1 family, six distinct spellings among them |

The `S` literals are transcribed verbatim, upstream wording and typo included —
`CktElement` "Error", `Bus` "Error, Parameter non recognized", `Circuit`
"Error, parameter not recognized", `Meters` "Error, Parameter not recognized",
`Topology` "Error, parameter not valid", `Solution`
"Error, paratemer not recognized", `PDElements` "Error, parameter not valid"
(`S_SENTINELS`, `crates/dss-epri/src/modes.rs:231`). Three consequences:

* **Containment is mandatory for `V`.** `DBusV`'s `else` is the one branch that
  omits the `setlength(myStrArray, 0)` its siblings do, so it *appends* to the
  DLL-global string buffer. Measured verbatim after priming with `CircuitV(987)`:
  "Error, parameter not recognizedCommand not recognized" — equality misses it.
* **`CktElementS`'s bare "Error" is ambiguous** — the served **mode 6**
  `CktElement.ActiveVariableName` returns the same string as its own default when
  the variable does not exist (`DCktElement.pas:461-462`) — so it is usable by the
  probe/diagnostic path only, never as a capture's miss test. Mode 6 is not a
  table row; the one `CktElementS` row WP-G1 reads is mode 4
  (`CktElement.EnergyMeter`), whose default is the function default `'0'`
  (`DCktElement.pas:421`). G1.3d(i) is the sub-step that reads it live, and
  that default is exactly why the capture folds `'0'` (r4133) and `''` (capi) to
  "no meter" before comparing — see the D4 fold above.
* **A family this crate has not measured is refused, not guessed.** `probe_mode`
  (`crates/dss-epri/src/dss.rs:1583`) refuses an `S` probe on a family with no
  `S_SENTINELS` row, and a `V` probe on a family in `V_WITHOUT_SENTINEL`
  (`crates/dss-epri/src/modes.rs:180` — `CapacitorsV` writes **no** sentinel at
  all), rather than reporting a silent `Served`.

**Mode capability — measured, and the acceptance for all of WP-G1.** The modes
WP-G1 needs live once, as `ModeSpec` rows in `WP_G1_MODES`
(`crates/dss-epri/src/modes.rs:1725`), each carrying its (family, kind, mode)
triple, the `D*.pas` line of the `case` arm it transcribes, the `myType` tag a
`V` arm assigns, and any state the arm moves. `Engine::read_mode`
(`crates/dss-epri/src/dss.rs:1775`) takes the row **by reference** — a mode number
cannot drift between the table and its reader — and rejects a reply whose shape is
not the row's, so a future DLL revision fails loudly instead of decoding garbage.
`r4133_mode_capability_is_complete_for_wp_g1`
(`crates/dss-epri/tests/modes.rs:114`) proves against the real DLL, on a solved
IEEE13 with an EnergyMeter attached, that **all 111 rows classify `Served`** and
decode into their declared shape: **zero misses, the expected-miss list is empty,
no per-channel r4133 mask is owed by any WP-G1 surface sub-step.** Its non-vacuity
is in the same test — a mode index past every family's last arm (`987`) classifies
`UnknownMode` with that family's own sentinel, across all 26 (family, shape) pairs.
A `V` reply also goes through `classify_v` when its tag is 4, because the
unknown-mode reply carries tag 4 too: without it the two
`Solution.IncMatrix{Rows,Cols}` rows would decode
"Error, paratemer not recognized" as data. On `I`/`F` no such check is possible —
the sentinel is the plain value `-1` / `-1.0`, which several served modes return
legally — so that shape's guarantee is this acceptance test, which trips the
moment a re-vendored DLL drops a mode.

That acceptance is **capability, not correctness**: `Served` means "not this
family's `else` sentinel", and `read_mode` additionally validates the `V` shape,
so a mode index transposed with a same-shape sibling of the same family would
still pass. Exact readings therefore pin identity family by family —
`Circuit.Losses` vs `LineLosses` vs `TotalPower`, `Topology.NumLoops`,
`PDElements.TotalCustomers`/`FromTerminal`/`PctPermanent`/`RepairTime`, on top of
the fixture's own seven
(`distinguishing_readings_separate_same_shape_modes_within_a_family`). `Meters` is
deliberately absent: every one of its reliability registers reads `0` / `0.0` on
this fixture (no `RelCalc`), so no pin there could discriminate; G1.6 wires that
surface and gets its own — two further phases run after the fixture’s `RelCalc`:
`the_relcalc_protocol_and_the_section_cursor` and, for the eight per-bus columns,
`the_bus_reliability_columns_are_live_after_relcalc`, which pins exact literals on six
buses and requires every same-shape `F`/`I` pair to disagree on some bus. Per-mode
*value* validation against the capi channel stays each surface sub-step's job
(decision D2).

Three rules that table carries, each of which a capture must respect:

* **Getters only.** The generic reader supplies a neutral argument, so a DDLL
  *write* arm would be executed. `PDElements F:1` (`FaultRate`) and `F:3`
  (`PctPermanent`) are write arms that return the pre-`case` default `0.0` rather
  than the `-1.0` sentinel — invisible downstream — so they are recorded in
  `EXCLUDED_WRITE_MODES` (`crates/dss-epri/src/modes.rs:2014`) instead of the
  table, alongside their readers (`F:0`, `F:2`). G1.6b added a third row of a
  different shape: `PDElements S:1` (`Name`) is a **write** arm that re-selects
  `ActiveCktElement` by searching the whole `PDElements` list for its argument,
  so the generic reader's neutral `""` matches nothing and leaves the
  pointer-list cursor past the end of the list — it silently truncates an
  in-progress walk instead of storing a number. G1.3a added a fourth, on another
  family: `CktElement I:13` (`Enabled`) is the write arm of the `I:12` reader its
  enabled-only polar capture needs, and the neutral argument `0` would DISABLE
  the active element. G1.6(ii) added a fifth of yet another shape: `Bus F:4` is
  `Bus.Y - Write` (`DDLL/DBus.pas:113-121`), a *shape collision* with the
  `Bus I:4` (`N_Customers`) row this sub-step does bind rather than a write twin
  of a reader we take, so the neutral `0.0` would store into the bus coordinate;
  it is pinned by an explicit shape assertion, not by prose. **One deliberate non-getter is in the table**:
  `Meters I:22` (`SetActiveSection`, added by G1.6(i)) is a *selector* — it stores
  no caller data in the model, only moves the per-meter section cursor the eight
  `MetersI(23..27)` / `MetersF(4..6)` reads answer from, and its neutral argument
  is the arm's own documented deselect (`Else pMeter.ActiveSection := 0`,
  `DMeters.pas:261`), so the generic table walk stays sound while the capture
  drives it with a real 1-based index. Hence **111** rows today, not 116 (the
  table was 96 before G1.6b's three `PDElements` walk arms, G1.3a's
  `CktElementI(12)` — its `CktElement.Enabled` twin is an
  `EXCLUDED_WRITE_MODES` row, never a table row — G1.4a's `Bus.Nodes` +
  `Circuit.AllBusNames`, G1.6(i)'s `Meters.SetActiveSection` and G1.6(ii)'s
  **eight** per-bus reliability arms — `BusF` 6-11 `Lambda`/`N_interrupts`/
  `Int_Duration`/`Cust_Interrupts`/`Cust_Duration`/`TotalMiles`
  (`DDLL/DBus.pas:129/136/143/150/157/164`) and `BusI` 4-5
  `N_Customers`/`SectionID` (`:60/67`), all `ModeEffect::Pure`).
  G1.3d(i) adds no row and live-compares **five more** of the table —
  `CktElementI` 0/1/2 (`NumTerminals`/`NumConductors`/`NumPhases`),
  `CktElementV(17)` (`NodeOrder`) and `CktElementS(4)` (`EnergyMeter`).
  G1.3b and G1.3c add no row either and live-compare **six more** —
  `CktElementV` 7/8/9 (`SeqCurrents`/`SeqVoltages`/`SeqPowers`) and 13/14/20
  (`CplxSeqVoltages`/`CplxSeqCurrents`/`TotalPowers`), the last three reached
  through `Engine::element_cplx_seq` and `Engine::element_total_powers`
  (`crates/dss-epri/src/dss.rs:1198`, `:1135`).
* **`ModeEffect` is the authority on what a row moves, and it carries the
  capture-order partition.** `Impure` rows move state: `Meters.Totals` re-runs
  `TotalizeMeters`; `PDElements.ParentPDElement` re-points `ActiveCktElement`;
  the `Topology` rows build and memoize `GetTopology` and walk a `PointerList`
  cursor to exhaustion; `CktElement.Has{Switch,Volt}Control` walk a `PointerList`
  to exhaustion; G1.4d's two `Bus` at-bus rows walk every PD/PC class's
  `DSS_Class.First`/`Next` and leave `ActiveCktElement` on the last element
  walked (2026-09-06, corrected from `Pure` — see "Impure but order-free" below);
  and all five `Circuit` aggregate rows — `Losses` (V:0),
  `LineLosses` (V:1), `SubstationLosses` (V:2), `TotalPower` (V:3) and
  `AllElementLosses` (V:8) — walk a `TPointerList` to exhaustion *and* call
  `Get_Losses`/`Get_Power` → `ComputeIterminal` on everything they walk
  (`Common/CktElement.pas:743`, `:677-680`; `Circuit.Losses` does it one level
  down, in `TDSSCircuit.Get_Losses`, `Common/Circuit.pas:2436-2443`). Those five
  read `Pure` until G1.9 measured them against the criterion the `Topology` rows
  already used (`CIRCUIT_LOSSES`, `crates/dss-epri/src/modes.rs:1090`); the label
  is load-bearing rather than cosmetic, because the cursor `Circuit.Losses` moves
  is exactly the one `Circuit.NextPDElement` resumes from — G1.6b's surface.
  A capture re-selects after them. The other two variants **are** the A/B
  partition below: `ReadsIterminalCache` (group A — `CktElement.PhaseLosses`,
  `TotalPowers`, which reach `ComputeIterminal`) and `PoisonsIterminalCache`
  (group B — `SeqCurrents`, `SeqPowers`, `Residuals`, `CplxSeqCurrents`,
  `CurrentsMagAng`, which call `GetCurrents` into a scratch buffer). Membership is
  pinned as data by `the_capture_order_partition_is_the_one_d3_names`, so a row
  cannot be annotated `Pure` and quietly tell a capture author that the reads
  commute. Measured on the vendored DLL (snapshot and harmonics, G1.0 settlement,
  2026-09-04) the poisoning is *latent* on IEEE13 — the converged solve leaves
  the counter current, so nothing refreshes and nothing is starved — which is
  precisely why the rule is recorded as data instead of as a live test that would
  pass vacuously.
* **Selection order matters.** `MetersI(0)` (`First`) sets `ActiveCktElement` to
  the meter object, so a fixture that selects the element *before* the meter
  reads the meter everywhere and still classifies every mode `Served` — green and
  vacuous. The acceptance test selects the meter first and pins the difference by
  exact equality.

The acceptance is a **single `#[test]`**: the DLL is a process-global,
single-threaded singleton, and five parallel `#[test]`s in one binary produced a
measured access violation inside it. Each phase stays a named `fn`, so a failure
still names the phase.

**The two-double `F` families.** Of the 42 families exactly two declare a second
double — `CircuitF` (`DCircuit.pas:27`; mode 0 `Circuit.Capacity`) and `CmathLibF`
(`DCmathLib.pas:5`; mode 0 `Cabs`, mode 1 `Cdang`) — while the registry bound
every `F` as `fn(i32, f64)`, leaving `XMM2` to carry whatever the caller happened
to leave there. Fixed by `FnF2` (`crates/dss-epri/src/ffi.rs:58`) plus
`TWO_DOUBLE_F` (`crates/dss-epri/src/families.rs:60`), with load-time asserts that
no family carries both shapes and that the split moves no entry point. Pinned by
`cmath_lib_f_takes_two_doubles`: `Cabs(3, 4)` = **5.0** exactly (pre-fix **3.0**)
and `Cdang(3, 4)` = **53.13010235129776** (pre-fix ≈ 0), plus `Cdang(0, 1)` =
**89.99999999516423** = `(3.14159265359 / 2) · 57.29577951`, the r4133 value with
its own truncated constants (`Ucomplex.pas:96-121`) — an oracle reading nothing in
`dss-core` reproduces, so no compat-tagged site is owed for it. `Circuit.Capacity`
is deliberately never driven: it writes `CapacityStart`/`CapacityIncrement` and
runs `ComputeCapacity`, and its correctness rides the same code path as the
`CmathLib` pin.

**Do-not-call modes.** Two DDLL arms are memory-unsafe and are refused **before
any FFI** by `check_callable`, consulted at the crate's single dispatch
chokepoint `Engine::ffi_dispatch` — so the worker's raw `{"cmd":"ffi"}` channel,
the one a probe author reaches for first, is refused as well
(`the_raw_ffi_command_refuses_the_do_not_call_modes`, which then pings the worker
to prove it survived) — as well as by `probe_mode`, which reports the refusal as a
typed `ModeStatus` rather than an error; no accessor exists for either
(`DO_NOT_CALL`,
`crates/dss-epri/src/modes.rs:325`):

* `Solution` V:2 `BusLevels` — `DSolution.pas:580-582` does
  `setlength(myIntArray, ArrSize)` then `for IMIdx := 0 to ArrSize`, i.e.
  `ArrSize + 1` writes into an `ArrSize`-long array. Source-proven; never called.
* `Bus` V:17 `ZSC012Matrix` — `DBus.pas:803-838` calls `Zsc.MtrxMult(As2p)` with
  no `Assigned(Zsc)` guard, so a bus whose `Zsc` was never built (no fault study)
  nil-derefs. **Measured process kill** of the worker on IEEE13 (G1.0 probe,
  2026-09-04). Modes 6/7/8/9 (`ZscMatrix`/`Zsc1`/`Zsc0`/`YscMatrix`) are guarded
  and returned `[0.0, 0.0]` on the same bus.

Neither is on WP-G1's mode list, so neither costs a comparison channel. Both are
upstream r4133 defects; the refusal lives in code, so the register cannot rot into
a comment nobody reads
(`do_not_call_refuses_the_two_unsafe_modes_without_touching_the_dll`).

**Topology (G1.7): nothing was owed here.** G1.0 had already bound and classified
`TopologyI(0..2)` and `TopologyV(0..2)` as `Served` (`modes.rs`
`TOPOLOGY_NUM_LOOPS` … `TOPOLOGY_ALL_ISOLATED_LOADS`, effects `TOPO_TREE` /
`TOPO_PD_LIST` / `TOPO_PC_LIST`) with one typed accessor each, from
`topology_num_loops` to `topology_all_isolated_loads`
(`crates/dss-epri/src/dss.rs:2455-2484`), so G1.7 added **no FFI**. The family's
remaining modes — `TopologyI(3..12)`, all of `TopologyS`, and `TopologyV`'s cursor
arms — are never bound and never called: they reassign
`ActiveCircuit.ActiveCktElement` and would poison the per-element capture of the
same step. That is not a capability gap (the DLL serves them) but a deliberate
non-use, asserted from the source text by
`capture_order.rs::the_r4133_bridge_exposes_only_the_six_order_free_topology_rows`.
Note for any later surface that walks `PDElements`/`PCElements`: the four list
modes leave those `TPointerList` cursors at the end, so a reader after the
topology capture must re-seek with `.First`.

**Incidence (G1.8): nothing was owed here either.** G1.0 had already bound and
classified `SolutionV(1)`/`(3)`/`(4)`/`(5)` as `Served`
(`modes::SOLUTION_INC_MATRIX`, `_ROWS`, `_COLS`, `SOLUTION_LAPLACIAN`) with one
typed accessor each (`solution_inc_matrix` … `solution_laplacian`,
`crates/dss-epri/src/dss.rs:2554-2573`), so G1.8 added **no FFI and no mode**;
its `dss.rs` diff is the block comment recording the read order and what is not
bound. `SolutionV(2)` `Solution.BusLevels` stays on the do-not-call register and
is never bound (the `BusLevels` row, `crates/dss-epri/src/modes.rs:309-312`); since the
capture never
issues `CalcIncMatrix_O`, `IncMat_Ordered` is always FALSE and `IncMatrixCols`
therefore always answers the whole `BusList` (`DSolution.pas:616-632`) — which is
the branch the port's accessor has to reproduce and the CSV writer deliberately
does not.

A third unsafe arm is **recorded and never bound**: `Meters I:13`
(`CountEndElements`) dereferences `pMeter.BranchList.ZoneEndsList` behind a
`pMeter <> Nil` test only (`DMeters.pas:157-163`), so a meter whose zone was never
built nil-derefs inside the DLL — where capi guards the same property with
`CheckBranchList(5500)` (`CAPI/CAPI_Meters.pas:535-544`). WP-G1 never needs it: it
is the length of a list G1.6(i)'s reliability capture reads in full, so it has **no
`ModeSpec` row** and the bridge has no way to reach it (`read_mode` takes a row by
reference). A later sub-step that wants it must put it in `DO_NOT_CALL` first.

**State-dependent refusals — served, but not on every bus (G1.4c, 2026-09-05).**
`Bus` V:11 (`VLL`) and V:12 (`puVLL`) are `Served` modes that answer correctly on
almost every bus and **never return** on a few: their partner scan is an
unbounded `repeat` whose probe set is `{jj₀} ∪ {1, 2, 3, 4}`, `jj₀` being the
node the first loop found **plus one** (the spelling `modes::bus_vll_would_hang`
uses)
(`DDLL/DBus.pas:580-584`, `:636-640`), so a bus carrying none of those node
numbers spins forever — measured on `NEVTestCase` `double-1`
(`Bus.Nodes = [10, 31, 32, 33, 41, 42, 43]`): **TIMEOUT at 20.011 s**, against
**0.000 s** on the 5-node `13kvbus` in the same session. capi 0.14.5 bounded the
same loop to three tries in 2020 (`CAPI_Alt.pas:2512-2514`, its own comment
naming the infinite loops), so this is r4133-only. A hang has no oracle value and
cannot be pinned by a tolerance, so the bridge **refuses the call** on exactly
those buses: `modes::bus_vll_would_hang` (`crates/dss-epri/src/modes.rs:378`) is
an FFI-free transcription of the loop over the bus's own node numbers, the two
modes are registered in `STATE_DEPENDENT_REFUSALS` (`modes.rs:411`, asserted
**disjoint** from `DO_NOT_CALL`, which keeps meaning "unsafe in every state"),
and `Engine::bus_vll_pair` (`crates/dss-epri/src/dss.rs:2195`) is their single
dispatcher — it re-reads `Bus.Nodes` itself (that arm's own scan is bounded), so
no caller can pass it a stale node list and one verdict decides both arms. A
refusal is published as `vll_declined = true` with empty arrays, and the harness
asserts it against its own independent replay of the same loop, in both
directions. This is the **D2** mode-capability record for the group: the modes
are served, the refusal is state-dependent and measured, and no group is silently
capi-only.

**Impure but order-free — served on both channels (G1.4d, 2026-09-06).** `Bus`
V:18 (`AllPCEatBus`) and V:19 (`AllPDEatBus`) are `Served` on the r4133 DLL and on
the capi channel alike; neither is capi-only and neither is refused. What G1.4d
corrected is their `ModeEffect`: both were registered `Pure` and both are
**`Impure`**, because `getPCEatBus`/`getPDEatBus` walk every PD/PC class with
`DSS_Class.First`/`Next` and `TDSSClass.Get_First`/`Get_Next` assign
`ActiveCircuit.ActiveCktElement` and `ActiveDSSObject`
(`Common/DSSClass.pas:342-371`), leaving the cursor on the last element of the
last class walked and each walked class's own `ActiveElement` past its end.
Measured live: the active element moved on **199 of 199** decks probed
(`Line.650632 → Capacitor.cap2` on IEEE13, the last element of the last class walked), never unchanged —
`the_two_at_bus_rows_are_impure` (offline, the row itself) and
`the_at_bus_reads_move_the_active_element` (live, inside
`r4133_mode_capability_is_complete_for_wp_g1`, with an `assert_ne!` so it cannot
become a tautology) hold both halves. capi's twin iterates a
`TDSSPointerEnumerator` (`Shared/DSSPointerList.pas:17-27`) carrying its own index
and touches no global cursor, so the hazard is r4133-only and belongs on the mode
row, not in the capture.
`ModeEffect::Impure` is still capture group **`'C'`** — neither read moves
`ActiveBusIndex` (measured: `Bus.Nodes` and `Bus.kVBase` unchanged across both) nor
any `Iterminal` cache — so the pair is order-free with respect to the other bus
arms and the D3 partition census does not move
(`the_at_bus_surface_is_order_free_in_the_mode_table`). Both transports
nevertheless read it **last** in the per-bus walk: it is that walk's only impure
read, and reading it last makes the placement independent of every later addition
(the argument that put `all_properties` last, applied here). Any per-element read
that follows must re-select its fixture — `capture_all_properties` already does
(`? name.Like`).


## Environment variables

All verified against the consumers named. The `DSS_GATE_*` knobs live in
`corpus_gate/{scheduler,engines,ledger}.rs`.

| var | consumer | meaning |
|---|---|---|
| `DSS_ORACLE_PYTHON` | corpus_gate | interpreter for the **pinned** oracle (default `python`) |
| `DSS_ORACLE_TIMEOUT_SECS` | corpus_gate | per-request worker deadline in seconds (default 120; CI uses 600) |
| `DSS_EPRI_WORKER` | corpus_gate | path of the `epri-worker` binary (default `target/<profile>/epri-worker`, auto-built via `cargo build -p dss-epri` if missing) |
| `DSS_EPRI_ACTOR_TIMEOUT_SECS` | dss-epri (`dss.rs::wait_for_actor`) | deadline for a wedged r4133 solve-actor (default 300); on expiry the case fails instead of hanging the worker |
| `DSS_EPRI_DLL` | dss-epri (`smoke.rs::dll_path`) | override the r4133 DLL path (default the git-tracked `tools/opendss/bin/r4133/OpenDSSDirect.dll`) |
| `DSS_EPRI_EXPECT` | dss-epri (`smoke.rs::expect_version`) | override the version-pin substring (default read from `tools/opendss/revisions.json`) |
| `DSS_GATE_JOBS` | corpus_gate | scheduler thread count (default `available_parallelism()`; per-channel worker pools size to `max(2, jobs/2)`) |
| `DSS_GATE_RECYCLE_AFTER` | corpus_gate | cases served per worker before respawn. **Default 1** (fresh worker per case = deterministic); raise for a faster, non-deterministic dev loop |
| `DSS_GATE_ONLY` | corpus_gate | comma-separated substring filter over case labels — a loud PARTIAL run for triage; **panics if nothing matches** |
| `DSS_GATE_SERIAL` | corpus_gate | `1` → T=1 and a fresh one-shot process per case (the contamination-proof reference mode) |
| `DSS_GATE_SHUFFLE` | corpus_gate | `<seed>` → shuffle the task order (order-sensitivity probe) |
| `DSS_GATE_DUMP` | corpus_gate | `<path>` → write a label-sorted `{verdict, result}` artifact (three-way bit-diff proofs) |
| `DSS_GATE_SEED_LEDGER` | corpus_gate | `1` → seeding **report** mode: measure every case on BOTH channels, write `tmp/ledger_candidates.json`, assert nothing |
| `DSS_GATE_SEED_ONLY` | corpus_gate | substring filter for the seeding run |
| `DSS_PROPS_CENSUS` | corpus_gate | `1` → arms the property **census** test `corpus_gate_props_census` (`R4133_PROPS_PLAN.md` RP0.2; a separate `#[test]`, so the var can never divert the mandatory gate): walk every live non-`large` case on BOTH channels with `all_properties` forced on — the case's own `engines` key ignored, which before R4133_PROPS RP4.1 (2026-09-03) also meant bypassing the §1.1 r4133 masks the gate applied — collect every divergent cell instead of asserting, write `tmp/props_census.json` + `tmp/props_census/run.json` + `tmp/props_census/<channel>/{structural_pairs,numeric_pairs,examples_full,shape,summary}` in the RP0.1 extract format. Honors `DSS_GATE_ONLY` (stamped into the artifacts); **asserts nothing** — a divergence is the measurement. `claims` → the same walk in RP2.1's **disposition** mode: every value row annotated `normalized-by-<rule>`/`echo-row`/`under-floor`/`ledger-hit`/`UNCLAIMED` through the shipped policy predicates, plus `claims{,_unclaimed_pairs,_summary}` per channel and the in-scope split; plain-mode artifacts are byte-identical either way. Any other value fails loudly |
| `DSS_LEDGER_MEASURE` | corpus_gate | `1` → numeric ledger handlers print the live divergence per scope (envelope sizing; no gating change) |
| `DSS_UPDATE_POPULATION_LOCK` | population_lock | `1` → rewrite `population.lock.json` from the current manifests (deliberate regen) |
| `DSS_UPDATE_GOLDENS` | `harness::regen` | `1` → arm the self-golden write rails (`snapshot_text`/`snapshot_bytes`). Refuses any artifact not anchored `self` in `golden.lock.json`, and any family whose `produced_by` is not this build's lane. Inert otherwise; no driver calls the helpers yet (WP-G3) |
| `DSS_UPDATE_GOLDEN_LOCK` | golden_lock | `1` → recompute every digest and rewrite `tests/golden/golden.lock.json` (deliberate regen). Provenance is re-derived from the test's registers, never carried over from the stored row, and every re-anchored or newly seeded path is announced on stderr |
| `DSS_LIVE_CLASSIFY` | corpus_gate | `1` → probe `skipped_needs_investigation` candidates, write `tmp/classify_report.json` |
| `DSS_LIVE_PROPS` / `DSS_LIVE_PROPS_MAX` | corpus_gate | `1` → opt-in all-property parity sweep over the capi-gating corpus (diagnostic); `_MAX` caps the case count |
| `DSS_EXPENSIVE_TESTS` | adiakoptics | `1` → run the 168-step (yearly) A-Diakoptics time-series variant instead of daily-24 |
| `DSS_REGEN_AD_GOLDEN` | adiakoptics | `1` → rewrite the committed A-Diakoptics matrix golden (deliberate regen; a `self`-anchored artifact written directly, bypassing the rails until G3.6) |
| `REGEN_SCHEMA_PORT` | golden_schema | `1` → rewrite `json/schema_full_port.json`, the port's own schema document (deliberate regen; the second direct `self` writer, bypassing the rails until G3.6) |
| `DSS_AD_CLASSIFY`, `DSS_AD_DECOMPOSE` | corpus_gate | throwaway A-Diakoptics triage probes |
| `DSS_ORACLE_ENGINE` | oracle_server | only `"capi"` is accepted (the default); anything else exits non-zero — the retired `capi015`/`oddie` engines never silently pass |

## Procedures

**Run the gate** — the three commands above. Keep `tests/corpus` pristine
afterwards (`git status tests/corpus`): the live gate executes decks in place;
the `CorpusGuard` (recursive; ported to the r4133 side as
`crates/dss-epri/src/guard.rs`) restores each case dir on every engine side,
but a run that writes OUTSIDE the case-dir tree (e.g. a manual `dss-cli`
invocation, or the known export-CWD corner) is uncoverable —
`git restore`/path-limited `git clean` the subtree if anything lingers. **Probe a
vendored deck on a copy, never in place:** several of them write next to
themselves over *tracked* files — compiling any of the three `IEEE_519.DSS`
copies rewrites `IEEE_519_Mon_mpcc_1.csv` / `IEEE_519_SavedVoltages.dbl`, and
`Test/LineConstantsCode.DSS` is the third of the trio `lane_diff.ps1` restores by
exact path (measured, RP3.7 2026-09-02).

**One gate or probe per worktree at a time** (2026-09-04, GOLDEN_REBASE G1.4a,
coordinator decision D13). Two runs in the same worktree that touch the same case
fight over the files the deck writes beside itself: measured on
`modes:harmonics/harmonict.dss` (which does `Save Voltages`), two concurrent scoped
runs of the same filter **both** failed — `#711 Unable to create file
"…/modes/harmonics/gaps_harmt_SavedVoltages.dbl": The process cannot access the file
because it is being used by another process` on one channel, `#715 Error reading
file to retrieve saved voltages: Read beyond end of file` on the other (`CorpusGuard`
still restored the tree). Two full gates always overlap, so: one gate — or probe —
per worktree. Runs in **different** worktrees are allowed (each has its own
`tests/corpus` copy and its own target dir) and rely on the r4133 worker no longer
leaking through the one channel that is machine-wide, `HKCU\Software\OpenDSS\MainSect`:
r4133 reads `BaseFrequency` from it at DLL load (`Common/DSSGlobals.pas:975`/`:1005`,
from `TExecutive.Create`, `Executive/Executive.pas:124`) and writes it back at
process exit (`:1013`-`:1022`, from `Executive.pas:141`) — both legs measured, the
write included, which the leaked-DLL exit path does **not** skip. Before the bridge
fix a worker that had run the corpus's single 50 Hz deck
(`…/IEEETestCases/LVTestCase/Master.dss:3`) therefore left `50` in the key and every
worker starting next solved its case at 50 Hz; that, and not the scheduler, is what
the "21 red, all `R4133`" parity run of 2026-09-04 was — injecting 50 Hz into the
oracle reproduces its `expected` numbers bit-for-bit (`modes:newton/newton.dss` node 0
`(7160.292512029644, -49.84781354621654)`, `modes:harmonics/reactor_rlcurve.dss` node 0
`(0.3285732746696498, 7.718915548595326)`). The worker now issues
`Set RegistryUpdate=No` and `Set DefaultBaseFrequency=60` at init and repeats the
frequency reset after every `clear` (`crates/dss-epri/src/dss.rs`, pinned by
`crates/dss-epri/tests/protocol.rs`); `tools/oracle/oracle_server.py` mirrors the
reset on the capi channel.

**Re-run a suspect red quiet.** Wait until no other run's `epri-worker` / `python` /
`corpus_gate` process is alive (`Get-Process`), then re-run the failing case alone
(`DSS_GATE_ONLY=<label>`) and, if it passes, the full gate. Three shapes are
environmental rather than numeric and are **never** triaged into the ledger: a
base-frequency-shaped red (`Vsource … frequency 60 vs 50`, `system Y mismatch`,
~1e-3 V gaps at 7 kV) on decks that were green before — check that
`reg query HKCU\Software\OpenDSS\MainSect /v BaseFrequency` reads `60` and that
every worktree has the bridge fix; and `oracle timeout after 120s` on a heavy
`large`/`ckt24` deck, which is load-sensitive (measured: such a deck passes alone in
11-19 s, while a full run under two or three other lanes' gates took 418-803 s
against a 182 s baseline). A third shape, seen once under that same load
(2026-09-04, one unfiltered parity run, no second run in this worktree): a case that
writes a report beside its deck reds with the DSS `#303`/`#711` *"The process cannot
access the file because it is being used by another process"* — measured on
`solvable_now:…/8500-Node/Run_8500Node_Unbal.dss` (`Show Powers kVA elem`), which
then passed alone in 96 s. Two jobs inside a single run can occupy one case directory
by design — `CorpusGuard` refcounts a shared pristine snapshot instead of serializing
(`corpus_gate/runner.rs`, "Another case in this folder is already running") — so two
decks in the same folder writing the same report name, or the gate test and
`corpus_ad_matches_normal_mode` walking the corpus at the same time, can collide;
which pair collided here was **not** isolated. Treat it as environmental, re-run the
case alone, and escalate only if it recurs with the machine quiet.

**A stale `epri-worker.exe` is rebuilt, not used as-is** (G1.4a audit settlement,
2026-09-05). `engines.rs::epri_worker_bin` used to build the worker only when the file
was **missing**, so a bridge change reached `cargo test --workspace` /
`cargo test -p dss-epri` but not a scoped `cargo test -p dss-core --test corpus_gate`
— measured 2026-09-04: a binary four minutes older than the D13 commit still inherited
`DefaultBaseFrequency=50` from the registry. It now compares the binary's mtime against
the newest of `crates/dss-epri/{src/**,Cargo.toml}` and rebuilds an older one (drive:
`touch crates/dss-epri/src/dss.rs` then run
`the_epri_worker_binary_is_not_older_than_its_bridge_sources`, which logs the rebuild).
An explicit `DSS_EPRI_WORKER` is honoured verbatim — its freshness is the operator's.

**The D13 registry tests write a machine-global key.** `crates/dss-epri/tests/protocol.rs`
poisons `HKCU\Software\OpenDSS\MainSect\BaseFrequency` (or a `37` sentinel) for the
duration of a worker spawn and restores it on every exit path — rewriting the saved value,
or DELETING it when the machine had none (`reg_restore`). The key is shared by every lane
and every worktree, so two lanes running those tests at the same instant can briefly see
each other's value; the `Engine::new` reset makes that harmless (each worker pins 60 Hz
regardless of what it read), which is exactly why the reset, not the restore, is the
load-bearing fix.

**The bridge suppresses report auto-display — with the engine's own switches, and the
editor no-op only as the safety net** (GOLDEN_REBASE G1.10a F0 + F0′, coordinator
decisions **D25** and **D39**, 2026-09-11). r4133 keeps `AutoDisplayShowReport := TRUE`
(`Common/DSSGlobals.pas:2052`) and calls `FireOffEditor` from **55** places in
`Version8/Source`; on Windows that is a `ShellExecute` of `DefaultEditor`
(`Common/Utilities.pas:298`, `:304`), read from `HKCU\Software\OpenDSS` at DLL load with
the factory default `'Notepad.exe'` (`Common/DSSGlobals.pas:990`, `:2122`) — one OS
process per report, ~900 orphaned Notepad windows across the lanes before this landed.
`Engine::new` (`crates/dss-epri/src/dss.rs`) therefore issues
`Set RegistryUpdate=No` → `Set AllowForms=No` → `new circuit.dssrs_bridge_init` →
`Set ShowReports=No` → `Set ShowExport=No` → `clear` → `Set Editor=rundll32.exe` →
`Set DefaultBaseFrequency=60`, i.e. **three layers**:

1. **`Set AllowForms=No`** (option 149, `Executive/ExecOptions.pas:640`/`:1118`, served
   with no circuit) sets `NoFormsAllowed`, which gates the three hash-list `Dump`
   branches (`Executive/ExecHelper.pas:1223`, `:1232`, `:1241`) and — the reason it is
   issued rather than left to `DSSI(8, 0)` — every modal `DoSimpleMsg` form
   (`Common/DSSGlobals.pas:615`, `:651`, `:676`), which would hang a headless worker
   for good. `Set AllowForms=Yes` is never driven, by design.
2. **`Set ShowReports=No`** (138, `ExecOptions.pas:975`) covers **34** sites — all 31 of
   `Common/ShowResults.pas` plus `Common/ControlQueue.pas:482`, `Common/Solution.pas:3543`,
   `Meters/Monitor.pas:1774` — and **`Set ShowExport=No`** (71, `:826`) covers
   `Executive/ExportOptions.pas:517`. Neither option is served by `DoSetCmd_NoCircuit`
   (`:645-649` answers `#301`), so a throwaway circuit `dssrs_bridge_init` carries them
   and `clear` drops it again; `clear` (`Executive/Executive.pas:234-276`) resets neither
   flag, so both survive every `clear` and every `Compile` (asserted after a real deck's
   `Compile`). A **deck** can still turn `ShowExport` back on — five live corpus decks do
   (`EPRITestCircuits/ckt5/Run_ckt5.dss:66`, `ckt7/RunDSS_ckt7.dss:61`,
   `IEEETestCases/8500-Node/Run_8500Node.dss:27`, `Run_8500Node_Unbal.dss:28`,
   `Microgrid/…/GFM_IEEE8500/Run_8500Node_Unbal.dss:28`), as does `Estimate`
   (`ExecHelper.pas:3779`) — and the flag would then leak into every later case of a pooled
   worker, so `Engine::clear` re-asserts both per case behind the same throwaway circuit (the
   D13 `DefaultBaseFrequency` shape; pinned by `::clear_re_asserts_the_report_switches`, which
   measured `Yes` on both flags after a bare `clear` before the fix — audit settlement
   2026-09-11).
3. **`Set Editor=rundll32.exe`** is the **safety net for the 12 sites upstream left
   unguarded** — `Dump` and `Dump alloc` (`ExecHelper.pas:1357`, `:1249`), `FileEdit`
   (`:1674`), `AlignFile` (`:3209`), `VDIFF` (`:3373`), `CvrtLoadshapes` (`:4071`),
   `Show AutoAdded` (`ShowOptions.pas:208`, `:209`), `Show QueryLog` (`:385`),
   `Rephase` (`Common/Utilities.pas:2817`) and the CN/CNTS cable-constants debug dumps
   (`General/CNLineConstants.pas:219`, `CNTSLineConstants.pas:355`). `rundll32.exe` exits
   at once on a non-DLL argument; it is issued **after** `Set RegistryUpdate=No` so the
   value can never reach the user's key (`Common/DSSGlobals.pas:1017` under the guard at
   `:1015`). The unguarded sites are reported upstream as
   `investigations/to_opendss/73-dll-fires-editor-despite-noformsallowed.md` (suggested
   fix: guard inside `FireOffEditor`, the DSS-Extensions `AllowEditor` precedent).

**No report is suppressed — only the viewer launch.** Every writer does `CloseFile(F)`
and *then* consults its switch (`Common/ShowResults.pas:401-403`, `ControlQueue.pas:482`,
`Solution.pas:3543`, `Monitor.pas:1774`, `ExportOptions.pas:517`), so the created-file set
(G1.10a), the file contents (G1.10b/c), the `Show`/`Export` goldens and `GlobalResult` are
untouched. Measured 2026-09-11 over 14 report-writing corpus decks (`Test/REACTORTest.DSS`,
`YgD-Test.dss`, `AutoTrans/Auto1bus.dss`, `NEVTestCase/Run_NEV.dss`,
`StoCtrl_SeasonTarget/Run_example.dss`, `CIM/IEEE13_CDPSM.dss`, the `IEEE-TIA-LV Model`
`Dump` deck, …): **56** created entries with the switches on, **56** with `ShowReports`
restored to its default `Yes` in-session, **0** differing decks, set-for-set equal per deck.
The same probe proves the layers are live rather than vacuous: with `Set Editor=<a sentinel
no machine can start>`, `Show Voltages LN Nodes` raises `#702` (`Utilities.pas:310`) when
`ShowReports` is back at `Yes` and stays silent with the bridge's `No`, while `Dump` raises
`#702` either way — the unguarded class the safety net exists for. Pinned by
`crates/dss-epri/tests/protocol.rs::report_switches_survive_a_compile_and_gag_every_guarded_editor_site`,
`::the_editor_safety_net_covers_the_sites_no_switch_guards`,
`::clear_re_asserts_the_report_switches`, `::command_lines_reads_this_process` and
`::init_overrides_the_os_editor_and_never_writes_it_back`. The two process assertions of the
safety-net test are attributed by command line to the test's own scratch directory —
`rundll32.exe` is a busy Windows image, and a machine-wide PID diff would red on a process
the bridge never started (audit settlement 2026-09-11).

Two riders. (a) `SetLastResultFile` sits *inside* `FireOffEditor`
(`Common/Utilities.pas:305`), so a gagged `Show` no longer updates `LastResultFile` /
`@lastfile` (`Common/DSSGlobals.pas:1058-1063`). That **aligns** the two oracles rather
than splitting them: dss_capi returns before the same call when the editor is off
(`.inputs/dss_capi/src/Common/Utilities.pas:231`) and `tools/oracle/oracle_server.py`
already sets `d.AllowEditor = False`; no corpus deck reads `@lastfile`/`%result%`.
(b) `Estimate` force-issues `Set showexport=yes` inside its own command
(`ExecHelper.pas:3779`) and never restores it, and `clear` does not reset `AutoShowExport`
— the safety net is the only cover there (0 corpus decks run it today). **Never restore a
bridge source file with a timestamp-preserving copy** (`Copy-Item`, `cp -p`) after a scratch
experiment: cargo's mtime fingerprint and `engines.rs::epri_worker_bin` both then keep the
*experiment's* binary — measured while taking the D25 numbers, where a probe silently ran
the un-suppressed worker. Touch the file (or `cargo clean -p dss-epri`) and re-verify.

**And the bridge writes no file of its own** (GOLDEN_REBASE G1.10a, coordinator decision
D30(1), 2026-09-06). `crates/dss-epri/src/capture.rs` issues **no** `export` command: the
event-log capture reads `Solution.EventLog` in memory (`DDLL/DSolution.pas:518`, `:526-541`)
instead of running `export eventlog`, which used to drop `<CircuitName>_EXP_EventLog.CSV` into
the case directory — invisible to every model comparison, and the largest single class of red
the moment G1.10a made the created-file set a compared surface. The two reads were measured
byte-identical corpus-wide (0 mismatches over a full 526-case drive, 83 `evlog=1` cases) and the
equivalence is pinned by `the_in_memory_event_log_equals_the_exported_file`; the behavioural half
— the capture creates nothing — is `the_event_log_capture_creates_no_file`. If a future capture
needs a surface only an `export` can reach, it belongs behind the same rule: read it in memory, or
write it in a scratch directory the test cleans, never in the case directory.

**Regenerate a golden** (manual, deliberate — never in CI): install the pinned
venv from `tools/golden/PIN.txt`, then run the matching `tools/golden/gen_*.py`.
Goldens pin intentional upstream inexactnesses (`TODO(compat)`), so improved
precision reads as a porting bug — do not regenerate to "fix" a diff. **Adding
a new deck is NOT a reason to rewrite the existing bytes:** `gen_json.py` takes
an optional deck-name filter — `python tools/golden/gen_json.py <deck> [<deck>…]`
writes only those files (an unknown name is rejected). Use it whenever you add
a deck; the unfiltered run rewrites all `tests/golden/json/*.json` and would
silently fold in any oracle/toolchain drift present since they were last
captured. Every committed deck golden must also have a `run_deck("<stem>")`
driver in `crates/dss-core/tests/golden_json.rs` — a directory guard
(`json_every_deck_golden_has_a_driver`) fails the gate otherwise. This
procedure covers the **pinned-oracle (capi) arms** plus — since the
EPRI-bridge parity round — the **r4133 arms of `gen_protection.py` and
`gen_flicker.py`**, which drive the vendored r4133 DLL through `epri-worker`
(no venv needed; use `DSS_GOLDEN_OUT=<scratch>` for a parity-check run —
payload byte-identity vs the committed golden is the acceptance). The
capi015-arm generators are frozen dead paths whose environment no longer
exists (see "Frozen historical generator arms" above); their goldens stay
pinned as-is. It covers every **externally anchored** artifact — everything
`golden.lock.json` anchors `capi_v0145`, `r4133`, `r3723`, `capi015` or
`fpc_3.2.2` — and, for now, two of the four `self` rows as well: `gen_props.py`
writes `props/recloser.json` and `props/relay.json`, whose committed values are
the port's own renders (a regen must repeat the manual r4133 cross-validation
their lock reasons describe). The other two `self` artifacts have their own
in-test knobs — `DSS_REGEN_AD_GOLDEN` (`tests/adiakoptics.rs:578`) and
`REGEN_SCHEMA_PORT` (`golden_schema.rs:629`).

**Regenerate a self-golden (R1–R4)** — the *other* regeneration procedure, for
artifacts the lock anchors `self`. All four of them are still written by the
three bypasses named just above; WP-G3 routes them through the write rails
(G3.6 for the two born-`self` knobs), and from then on a self-golden regen never
touches an oracle capture, because the rails refuse every other anchor (see
"Golden provenance lock" above). The four rules already govern every one of
those runs, bypass or rails:

- **R1 — clean tree only.** Regenerate from a committed state, so the diff a
  reviewer reads is exactly what the run produced and nothing else.
- **R2 — never to fix a red gate.** The sequence is: fix the engine → pin the
  intended new value with an expected-value test → regenerate → verify the diff
  moved **only** the predicted cells. A golden byte moved to make a gate green
  is the single failure this whole mechanism exists to prevent.
- **R3 — per family.** No bare "regenerate everything": a run rewrites the
  family under review and nothing else.
- **R4 — the lock digest diff is the review artifact.** The commit body names
  every moved artifact and its cause, and `tests/golden/golden.lock.json` is
  committed together with the change that moved the bytes.

Mechanics, once WP-G3 has wired a family's driver to the rails: run that driver
in the family's producing lane with the knob set —
`DSS_UPDATE_GOLDENS=1 cargo test -p dss-core --features dss-core/oracle-parity --test <driver> -- --nocapture` —
review the resulting artifact diff against the prediction R2 required, then
`DSS_UPDATE_GOLDEN_LOCK=1 cargo test -p dss-core --test golden_lock -- --nocapture`,
then re-run the full five-command gate in both lanes. **`-- --nocapture` is not
optional:** both runs *pass* (they write instead of comparing), and libtest
discards a passing test's output — without the flag the rails' `SNAPSHOT` /
`REFUSED` lines and the lock's `SEEDED` / `RE-ANCHORED` announcements are
dropped, and a sweep that silently refused half a family looks exactly like one
that wrote it. A refusal means the artifact is not yours to rewrite, never that
the knob needs forcing; the outcome is also returned to the call site
(`harness::regen::Outcome`, `#[must_use]`), so a driver can assert its family's
expected refusals instead of relying on the operator reading stderr.

**Re-vendor the corpus** — `python tools/corpus/vendor.py --force`, then review
the `tests/corpus/SHA256SUMS` diff.

**Add a corpus case** — pick the family by content (static→`asymmetric`,
control/time-series→`controls`, mode/algorithm/format/verb→`modes`; vendored
decks are classified in `manifests/solvable_now.json`); add the `.dss` (a
subfolder if it needs fixtures) + a manifest entry with its
`kind`/`n_steps`/`probes`. `engines` defaults to `"both"`; narrow to a single
channel only when the other channel **cannot** gate it (feature absent from
0.14.5, or wholesale non-fingerprintable divergence) and say why in `note`.
Set `pending: true` + `wp` if the feature is unported; `isolate: true` +
`note` if the deck holds files open or corrupts a reused engine process.
Validate on the pinned oracle first (two-process determinism + feature
sensitivity, GAPS_PLAN.md §2.1); run the seeding report (below) to measure the
r4133 side. The `*_manifest_is_complete` gate enforces the dir↔manifest
bijection; regenerate the population lock in the same commit.

**Triage a property divergence (the r4133 channel)** — the specialization of
the ledger triage below, and the *only* order in which the four links may be
tried. Every step is CLAUDE.md-bound first: a Rust↔oracle gap is a port bug
until proven otherwise, so step 0 is always "is our value right?" — if it is
not, fix the port and stop. Then, and only then:

0. **Locate the cell.** The failure message carries `class.prop` and both raw
   spellings. Measure the whole pair rather than the one case:
   `DSS_PROPS_CENSUS=claims cargo test -p dss-core --test corpus_gate
   corpus_gate_props_census -- --nocapture` writes
   `tmp/props_census/<channel>/claims{,_unclaimed_pairs,_summary}` — read the
   per-cell `disposition` in `tmp/props_census.json`, never the per-spelling
   fold (§"The disposition mode").
1. **Normalize** — `harness/props_norm.rs`, `PROPS_NORM_R4133`. Only if the two
   sides are the SAME value differently spelled, and only through a typed rule
   (`BoolFold`/`CaseFold`/`ArrayForm`/`EnumSynonym`) whose predicate proves it.
   The row cites the vendored census by *(pair, bin, cells)* and moves a
   per-kind count lock. Never a regex, never a "close enough".
2. **Echo-exclude** — same file, `PROPS_ECHO_R4133`. Only if the two sides do
   not spell one value at all because r4133 renders an echo (no
   `GetPropertyValue` arm, a parsed command string, an empty collection) or
   different live state. The row needs the r4133 `Version8/Source` unit:line
   that proves its category **and** a witness: capi coverage, or a named
   expected-value pin in `crates/dss-core/tests/props_r4133_pins.rs`. A row
   whose cells reach `engines: "r4133"` cases needs the pin regardless
   (`ECHO_ROWS_ON_R4133_ONLY_CASES`). On one of the 20 mixed pairs, extend
   `ECHO_NARROWED` with the measured spelling — do not widen the row back to
   pair scope.
3. **Display floor** — same file, `R4133_DISPLAY_FLOOR`. Nothing to edit: it is
   a cell predicate. If it does not claim the cell, the gap is either above
   `2e-4` or not a `%.Ng` render of our number, and **the floor is never
   widened** to make it fit (`tests/TOLERANCE_NOTES.md` §"r4133 props display
   floor" carries the derivation the widening would have to overturn).
4. **Ledger** — `tests/corpus/ledger.json`, a `property`-scoped entry, per case
   and per channel. This is where a *genuine* measured upstream divergence
   goes: the port computes the correct value, r4133 does not, and the entry
   pins both numbers (`rust`+`oracle`) with a `cause` and `source`. Follow the
   full procedure below, and pair the entry with the expected-value test that
   names both numbers — an entry without one is a mask.

If the cell reaches none of the four, it is a bug — in the port (fix it) or in
r4133 (fix the port, ledger the divergence, and file the report under
`investigations/to_opendss/`). "Skip it for now" is not a step: `SKIP_PROPS` is
capi-capture bookkeeping, not a place to park an r4133 divergence, and every one
of its rows is dispositioned for r4133 by a test.

**Triage a divergence into the ledger** — only for a **measured upstream
divergence**, never a shortcut past a red gate:

1. Prove it is not a port bug first (CLAUDE.md divergence rules: tighten the
   loop tolerance, isolate the suspect, diff per-iteration trajectories, read
   live f64 state). An entry that papers over a fixable bug is the worst
   outcome (UNIFIED_GATE_PLAN §5-R3). If the port is wrong — fix the port.
2. Measure it: `DSS_GATE_SEED_LEDGER=1 DSS_GATE_SEED_ONLY=<case>` for the raw
   both-channel report, and/or `DSS_LEDGER_MEASURE=1 DSS_GATE_ONLY=<case>` to
   size an envelope to the observed max.
3. Write the entry in `tests/corpus/ledger.json`: unique `id`, exact `case`
   key, `channel`, `kind`, `match` scopes (implemented fields only), envelope
   (`max_rel`/`max_abs`) or `num_rel` or exact `rust`+`oracle` pins, and a
   `cause` (or `cause_ref` into `causes`) + `source` + `measured` provenance.
4. Run the full gate: the structural test must pass, the entry must be HIT,
   and (for divergences) still exceed the tier floor — a too-wide or obsolete
   entry fails as STALE.
5. Regenerate the population lock (ledger digests are part of the case rigor)
   and commit ledger + lock together.

**Re-vendor the r4133 binary** — needs the vendored EPRI source/distrib tree
`.inputs/electricdss-code-r4133-trunk`:

```
python tools/opendss/vendor_binaries.py --force   # wipe + recopy bin/r4133
git diff tools/opendss/bin/SHA256SUMS             # review the checksum diff
cargo test -p dss-epri --test smoke               # version + CSC solution-neutrality
```

See `tools/opendss/README.md` for the artifact layout and bridge rules.

**Run the seeding report** — the measurement pass behind every ledger/`engines`
decision: `DSS_GATE_SEED_LEDGER=1` (optionally `DSS_GATE_SEED_ONLY=<substr>`)
`cargo test -p dss-core --test corpus_gate` runs every case on BOTH channels
with no ledger, asserts nothing, and writes candidate entries with measured
locations/envelopes to `tmp/ledger_candidates.json`. Hand-triage candidates
into real entries (rules above) — **never** commit the candidate file as-is.

## See also

- `CLAUDE.md` — the gate rule, `TODO(compat)` convention, known upstream bugs.
- `tests/TOLERANCE_NOTES.md` — the calibrated tolerance tiers and why each holds.
- `tests/corpus/README.md` — the vendored corpus + the synthetic families.
- `tools/opendss/README.md` — the r4133 binary artifact + the `dss-epri` bridge.
- `tools/oracle/README.md` — the pinned capi oracle server protocol.
- `UNIFIED_GATE_PLAN.md` — the unified-gate design decisions (D1–D10) + phases.
