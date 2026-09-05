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
cache), walks **all 524 manifest cases** on each engine — solving the 520 that
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
`asymmetric` (53) / `controls` (106) / `modes` (71) — 524 cases. Each case's
**`engines`** field names its gating channel(s): `"capi_v0145"` (59), `"r4133"`
(101), or `"both"` (the default; 364 cases gate on both channels). Case key = the gate
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
  comparison field; `CorpusGuard` restores each case dir (recursive; the
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
each addition to the rigor fingerprint rewrites all 524 rows of
`population.lock.json`, and a flag that is *not* in that fingerprint can be
switched off later with no lock diff at all:

| manifest flag | rigor token | owning sub-step | surface |
| --- | --- | --- | --- |
| `compare_derived` | `derived=` | G1.3a–c | per-element `CurrentsMagAng`/`VoltagesMagAng`/`Residuals`, `SeqCurrents`/`SeqVoltages`/`SeqPowers`, `CplxSeq*`, `TotalPowers`. **Wired since G1.3a (2026-09-04)**, which serves the first third — `Enabled` plus the three polar channels; the sequence surfaces follow in G1.3b/c |
| `compare_element_extras` | `elemx=` | G1.3d | `PhaseLosses`, `NodeOrder`, `EnergyMeter`, `OCPDevType`/`OCPDevIndex`, `HasVoltControl`/`HasSwitchControl`, `NumControls`, `NumTerminals`/`NumPhases`/`NumConductors`. **Wired since G1.3d(i) (2026-09-04/05)**, which serves the index/name scalars — the three counts, `NodeOrder` and `EnergyMeter`; `PhaseLosses` and the control-derived extras follow in G1.3d(ii) |
| `compare_bus` | `bus=` | G1.4a–c | **live (G1.4a):** `Nodes`/`kVBase`, `puVoltages`/`puVmagAngle`/`VMagAngle`, `AllBusVmagPu`; **owed:** `Distance`/`AllBusDistances`/`AllNodeDistances` + `AllPCEatBus`/`AllPDEatBus` (G1.4b), `SeqVoltages`/`CplxSeqVoltages` + `VLL`/`puVLL` (G1.4c) |
| `compare_zsc` | `zsc=` | G1.5 | `Zsc1`/`Zsc0`/`ZscMatrix`/`YscMatrix`/`Isc`/`Voc` |
| `compare_reliability` | `rel=` | G1.6 | meter extras + the per-bus reliability columns; also drives the executive `RelCalc` |
| `compare_pdelements` | `pde=` | G1.6b | the `PDElements` interface walk — **wired 2026-09-04** |
| `compare_topology` | `topo=` | G1.7 | `NumLoops`/`NumIsolated*`/`AllLoopedPairs`/`AllIsolated*` |
| `compare_inc_matrix` | `incm=` | G1.8 | `IncMatrix`/`IncMatrixCols`/`IncMatrixRows`/`Laplacian` |
| `compare_run_files` | `runf=` | G1.10a | the non-monitor CSV set + contents the deck emits under DataPath, and the `save circuit` file set |
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
(`crates/dss-core/tests/corpus_gate/ledger.rs:1056`) — the same caps
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
(`crates/dss-core/tests/corpus_gate/manifest.rs:571`) carries each flag's owning
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
in `corpus_gate/runner.rs`; G1.6b's `PDElements` walk (below) is the third and
G1.7's `compare_topology` (2026-09-05) the fourth. Both use the `_opt` form: an
empty PD walk is legal, and so is a case that never requested topology.
`compare_eventlog`,
`compare_ctrlqueue` and `compare_global_result` deliberately do **not** call it:
an empty event log or control queue is a legitimate reading there, so the rail
would change live gate behaviour. Adding it belongs to whoever proves the
emptiness contract for those three surfaces. The guard's own non-vacuity is
offline (`capture_guard::tests::{a_present_capture_passes, an_empty_capture_fails,
an_absent_capture_fails, the_message_names_the_flag_the_channel_the_context_and_the_shape}`).

**G1.3d(i) — the per-element discrete extras** (2026-09-04/05, lane `lane-e`).
`harness::compare_element_extras`
(`crates/dss-core/tests/harness/mod.rs:2505`) compares `NumTerminals`,
`NumConductors`, `NumPhases`, `NodeOrder` and `EnergyMeter` **exactly**: it takes
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
  (`mod.rs:2645`).
* **"No meter" is a two-channel spelling, folded at the capture boundary — each
  channel's OWN sentinel, and the census is what makes it safe** (coordinator
  decision D4, 0 ledger rows): capi returns `''` (`Result := NIL`,
  `CAPI/CAPI_CktElement.pas:672-687`), r4133 returns `'0'` (the `CktElementS`
  pre-`case` default, `DDLL/DCktElement.pas:421`; arm 4 at `:442-449` is guarded
  by `HasEnergyMeter` at `:444`). `harness::oracle_meter_name`
  (`mod.rs:2436`) takes the channel and folds only that channel's spelling, so on
  the capi side a meter literally named `0` is a name like any other and a port
  that lost it reds (`a_meter_named_zero_reds_instead_of_passing`,
  `mod.rs:2765`). On the **r4133** side the collision is genuinely undecidable and
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
  `ElementCap::node_order` (`mod.rs:968`) is the bus-local node number per
  conductor per terminal; the checkpoint-level `CaseResult::node_order`
  (`crates/dss-epri/src/capture.rs:123`) is the **Y node name order** of the
  whole circuit. They live in different JSON objects and share nothing but the
  word; both field docs say so.

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
`CktElement.Yprim` live on both channels (`tools/oracle/oracle_server.py:556`,
`crates/dss-epri/src/capture.rs:524` over `Engine::element_yprim`
(`crates/dss-epri/src/dss.rs:542`), Rust side `compare_yprim` at
`corpus_gate/runner.rs:498` → `harness::compare_yprim`
(`crates/dss-core/tests/harness/mod.rs:1369`)), so a
second `Lines`-shaped capture would add no information. **The honest residual is
coverage, not spelling:** YPrim is compared only for a case's
`selected_elements`, and over the four manifests **235 of 523** cases declare a
non-empty list (192 `["*"]`, 43 explicit) while the other **288** compare no YPrim
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
* **(C)** order-free reads (voltages, discrete state) go anywhere.

The reason is CLAUDE.md's upstream bug 4 (harmonics stale-`Iterminal`): a group-B
read poisons the cache a group-A read would otherwise have refreshed, so the
oracle's answer depends on request order. `SeqPowers` is a poisoner, not a
victim. `SeqCurrents`/`CplxSeqCurrents` are already the same kind of read the
golden path states at `tools/golden/gen_checkpoints.py::capture_element`.

**The rule is enforced, since G1.3a (2026-09-04).** Every read line inside a
capture body carries a `capture-order: <Name> (<A|B|C>)` marker, and
`crates/dss-core/tests/capture_order.rs` parses the three capture bodies —
`tools/oracle/oracle_server.py::capture_all_elements` (plus the
`gen_checkpoints.capture_element` it calls), `crates/dss-epri/src/dss.rs::element_pcl`
and `element_polar`, and `crates/dss-epri/src/capture.rs::capture_all_elements` —
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
case (`force_pdelements`, `crates/dss-core/tests/corpus_gate/scheduler.rs:252`;
the forced split is re-derived and pinned by `FORCED_PDELEMENTS_POPULATION`,
`crates/dss-core/tests/corpus_gate/scheduler.rs:338`). The port side is
`Dss::pd_elements` (`crates/dss-core/src/exec/view.rs:1321`), a `&self` read of
`CktElementData`; the comparator is `harness::compare_pd_elements`
(`crates/dss-core/tests/harness/mod.rs:6489`), which asserts the walk first
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
  (`crates/dss-core/tests/harness/mod.rs:6569`) fails the run unless **each**
  gating channel compared at least one non-empty walk.
* **`PD_SKIP_FIELDS` is fail-on-stale.** Both oracles read four cells out of
  uninitialized memory on in-zone shunt Capacitors/Reactors (`EnergyMeter.pas`
  assigns through `pPCelem: TPCElement`; nondeterministic across processes and, on
  r4133, within one), so those cells are excluded per (channel, class, field) in
  `PD_SKIP_FIELDS` (`crates/dss-core/tests/harness/mod.rs:6312`) — never
  enveloped, and never wider than measured. The scope is the element too, not the
  class: a row is consulted only where that write lands, on an in-zone **shunt**
  Capacitor/Reactor (`pd_skip_applies`,
  `crates/dss-core/tests/harness/mod.rs:6436`, port state
  `PdElementView::in_meter_zone`), so a series member of either class and a shunt
  one outside every zone stay fully compared — the defect is measured on 22
  (capi) / 29 (r4133) of the 372 / 431 walked cases. `fault_rate`/`pct_permanent`
  also stay compared on `r4133`, `lambda`/`accumulated_l` on `capi_v0145`, and
  Line / Transformer / AutoTrans / GICTransformer keep all four on both channels.
  Every row carries its Pascal citation and the pin that holds its value, a
  register test refuses a silent add or drop, a second one refuses a `pin` no
  `#[test]` defines, and `assert_pd_skip_rows_are_live`
  (`crates/dss-core/tests/harness/mod.rs:6622`) fails a row that excluded nothing
  in the whole run. The gate epilogue prints every row's visit/hit counts.

**The bus voltage surface (`compare_bus`, live since G1.4a, 2026-09-04).** Per
bus, in the engine's own `BusList` order: `Nodes`, `kVBase`, `puVoltages`,
`VMagAngle`, `puVmagAngle`, plus the checkpoint-level `AllBusVmagPu` — the four
quantities capi 0.14.5 and r4133 compute by byte-identical algorithms. The port
side is `Dss::all_bus_voltages` / `Dss::all_bus_vmag_pu`
(`crates/dss-core/src/exec/view.rs:1178`, `:1202`), the comparators are
`harness::compare_bus` (`crates/dss-core/tests/harness/mod.rs:7112`) and
`compare_all_bus_vmag_pu` (`mod.rs:7224`). Four things about it are worth
knowing:

* **Three ordering conventions meet here and must not be mixed.** The per-bus
  arrays are in ascending node **number** (the `repeat NodeIdx := FindIdx(jj)`
  walk, `CAPI_Alt.pas:2270-2275` == `DDLL/DBus.pas:415-421`), `AllBusVmagPu` is
  bus-list order × the bus's **internal node index** (the `AllNodeNames`
  permutation), and the gated `node_order` is `YNodeOrder` — a third one. The
  comparator asserts `Σ nodes == len(AllBusVmagPu)` so the first two cannot
  drift apart silently.
* **No manifest case sets the flag; the scheduler forces it.** `force_bus`
  (`crates/dss-core/tests/corpus_gate/scheduler.rs:590`) turns the surface on for
  every live case whose `kind` does not start with `large`, exactly like
  `force_properties`. `population.lock.json` fingerprints the **manifest** flag,
  so it cannot see that rule at all: the guard is the pinned
  `FORCED_BUS_POPULATION` (`scheduler.rs:607`) plus the oracle-free
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
  `the_voltage_exclusion_still_pins_kv_base` (`mod.rs:7461`) drives negatively.
  It is not a mask and does not read as one: every suppressed case is listed in
  the gate summary next to the entry that caused it
  (`ledger::LedgerRuntime::bus_array_suppressions`,
  `crates/dss-core/tests/corpus_gate/ledger.rs:499`).

**Bus reads are group C on both transports, and their order is a contract.**
Every one goes straight to `Solution.NodeV` (`CAPI_Alt.pas:2276` == r4133
`DDLL/DBus.pas:423`) and moves only `ActiveBusIndex`, so no bus read stales a
cached `Iterminal` or is staled by one. What
`the_bus_capture_reads_in_one_fixed_order_on_both_transports`
(`crates/dss-core/tests/corpus_gate.rs:467`) pins is therefore the agreement of
the two transports, not a staleness hazard: the same five per-bus quantities in
the same order on the capi and r4133 sides, the bus block after
`variables`/`eventlog`/`ctrlqueue` and before `all_properties` (which stays the
last read of the step on both), and no element-scoped read inside either bus
capture.

**Two bus quantities will be stronger than fastdss, not at parity.** The
fastdss harness drops `puVLL`, `VLL`, `AllPCEatBus` and `AllPDEatBus` from
`IBus._columns` under `COM_VLL_BROKEN` in the Oddie configuration
(`origin/fastdss` `tests/save_outputs.py:197-209`), so when G1.4b/G1.4c land
them the gate compares more than the parity target does — never describe those
four as fastdss parity.

**G1.7's topology surface is group C but is read strictly last** — after
`all_properties` — on both transports, and `crates/dss-core/tests/capture_order.rs`
asserts that from the two capture sources. Its six rows (`NumLoops`,
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
441 of the 520 live cases (310 `both`, 87 `r4133`, 44 `capi_v0145`; 440 =
313/83/44 before G1.4a's D12/D14 corpus flips reached it at the G1.7 merge) — the
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

Scope `field` must be one of the **14 implemented** handlers — `iterations`,
`voltages`, `injection`, `element`, `probe`, `property`, `monitor`, `eventlog`,
`ctrlqueue`, plus the five **exclusion-only** ones `y`, `y_fingerprint`,
`yprim`, `meter`, `variables` — anything else (typo or the §1.3-planned but
unimplemented `global_result`) is rejected loudly at load. Four of the
exclusion-only five name a whole compared artifact rather than a value with a
natural envelope (the assembled system Y, its fingerprint, one element's YPrim,
one EnergyMeter's register block), so `assert_structural` refuses them on a
`divergence`. `variables` (added by `R4133_PROPS_PLAN.md` RP3.10) is
exclusion-only for the other reason: a PC element's state variable does have the
`i_abs + i_rel·|oracle|` envelope, but no handler re-asserts one, so a
`divergence` naming it would promise a measurement the runtime never makes.
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

Current contents (re-counted off the file 2026-09-04, after G1.4a's D12/D14
channel flip): 54 entries over 31 documented causes — 5 r4133 `skip`
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
`swtcontrol-per-phase-state-*-capi-props` entries RP3.7 landed), and 9
`exclusion` — 3 capi_v0145 + 6 r4133, from three engine fixes. Four are
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
the r4133 channel of the same case needs no entry).

**`channels` names sub-channels, and must be spelled out** (GOLDEN_REBASE G1.0,
2026-09-04). `element` is today the only field whose comparison has sub-channels
(`currents`, `powers`, `losses`, and since G1.3a `currents_mag_ang`,
`voltages_mag_ang`, `residuals`), and the runtime reads a scope's `channels` as
*empty ⇒ all of them* — so a committed entry written for the original three would
silently widen onto every new element sub-channel WP-G1 adds (G1.3a–c), with no
ledger diff and no population-lock trip. `SUBCHANNEL_FIELDS`
(`crates/dss-core/tests/corpus_gate/ledger.rs:623`) closes that with three
load-time rules: a scope on such a field must carry a **non-empty** `channels`;
every name in it must be one of that field's declared sub-channels (a typo like
`"curents"` otherwise loads cleanly, selects nothing, and leaves the entry
reporting itself applied while masking not one value); and a scope on any other
field must carry no `channels` at all, since the runtime would never read it.
"All sub-channels" survives only as a named, reviewed exception in
`BARE_CHANNELS_ALLOWED` (`ledger.rs:641`), which is **empty**. The rules are
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
against `FORCED_PROPS_POPULATION` = (441, 310, 87, 44), which also pins the
per-`engines` split. G1.3a added the second such rule and its own pin:
`force_derived` turns `compare_derived` on for every live case except
`kind=large*` on the `solvable_now` arm, and `FORCED_DERIVED_POPULATION` =
(443, 312, 87, 44) — the same 441 plus the two `Test/AutoTrans/{Auto3bus,AutoHLT}`
manifest opt-ins, both `engines: "both"` — is re-derived on every run by
`the_derived_forcing_rule_is_every_live_non_large_case_plus_the_opt_ins`. The two
opt-ins are listed in Rust as `DERIVED_MANIFEST_OPT_INS`, each required to be a
live case the rule would not already cover. G1.4a added the third such rule and
its own pin: `force_bus` turns `compare_bus` on over the same live non-`large*`
population, and `FORCED_BUS_POPULATION` = (441, 310, 87, 44) is re-derived by
`the_bus_forcing_rule_is_every_live_non_large_case` on every run — the manifests
set the flag on no case (`bus=0` on all 524), so this const is the only guard
the lock cannot supply. G1.3d(i) added the fourth:
`force_element_extras` (`crates/dss-core/tests/corpus_gate/scheduler.rs:185`)
turns `compare_element_extras` on for every live case except `kind=large*` on the
`solvable_now` arm — no opt-in table, because fastdss's `KNOWN_COM_DIFF` carries
no row for any of the five fields — and `FORCED_ELEMENT_EXTRAS_POPULATION` =
(441, 310, 87, 44) (`scheduler.rs:212`) is re-derived on every run by
`the_element_extras_forcing_rule_is_every_live_non_large_case` (`:684`). It is the
same 441 as `FORCED_PROPS_POPULATION`, which is the same rule without opt-ins.
**Cost (G1.3d(i)):** the five extra reads per element bought **no measurable gate
time** — `cargo test -p dss-core --test corpus_gate` measured 218.7 s / 179.3 s /
180.6 s / 180.9 s (parity) / 164.8 s across five drives, plus the post-D19
re-drive's 201.7 s (default) / 180.6 s (parity), inside this machine's own
run-to-run spread on the G1.3a tree (177.6 s … 198.6 s over five runs), with the
cleanest post-move run *faster* than every G1.3a measurement. None of these runs
had the machine to itself (D7 lanes gate concurrently from separate `target`
dirs), so a number tighter than "no signal above the ±20 s spread" would need
repeated paired runs on a quiet machine.
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
live non-`large` one — **441** cases = 310 `both` + 87 r4133-only + 44 capi-only
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
`harness::compare_prop_lists` (`crates/dss-core/tests/harness/mod.rs:4729`) —
and links 2-4 are `PropsPolicy` methods gated on `is_r4133()` (`mod.rs:4272`,
`:4972`; the channel type is `PropsChannel`, `mod.rs:4238`). Link 1 is the
deliberate exception: `skip_prop` is a free function taking the channel, so its
`LANE_SKIP_PROPS` half stays channel-blind (row 1 below says so).

| # | link | seam | what it does | if it does not claim |
|---|---|---|---|---|
| 0 | shape allowlist `PROPS_015X` | `filter_015x`, `mod.rs:4529` | drops a Rust-side prop the capture cannot carry — **shape only** | the name walk fails |
| 1 | skip rows `SKIP_PROPS` / `LANE_SKIP_PROPS` | `skip_prop`, `mod.rs:3507` (channel rule at `:3522`) | value-only skip, per channel | fall through |
| 2 | normalization `PROPS_NORM_R4133` | `PropsPolicy::normalize`, `mod.rs:4993` | **re-spells** the oracle side when a typed rule proves the two are the same value | both raw spellings continue |
| 3 | echo table `PROPS_ECHO_R4133` | `PropsPolicy::echo_excluded`, `mod.rs:5032` | drops the **value** compare of that cell (name + index order still assert) | fall through |
| 4 | display floor `R4133_DISPLAY_FLOOR` | `PropsPolicy::under_display_floor`, `mod.rs:5089` | passes a numeric cell that is our value rendered to r4133's own digits | the cell reaches the assert |
| 5 | the assert | `assert_value_matches_tol`, `mod.rs:347` | the case's tier floors (`tol_for`) | **gate red, both spellings in the message** |

A divergence the ledger owns is handled outside this chain, by the case's
`property`-scoped `ledger.json` entry (`corpus_gate/ledger.rs:1264`, `:1278`) —
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
(`props_norm.rs:1412`, called in the gate epilogue, `corpus_gate.rs:216`) fails
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
`corpus_gate.rs:227`) — `visits > 0 && hits == 0` for a pair-scoped row,
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
`mod.rs:3945`). Its derivation — the measured worst cell, the empty band, the
`%.Ng` site table and the 55 refused spellings — is
`tests/TOLERANCE_NOTES.md` §"r4133 props display floor".

**The `SKIP_PROPS` dispositions (plan §1.2).** `skip_prop` is channel-aware
since RP2.1 (`skip_prop`, `mod.rs:3507`), because after RP4.1 a channel-blind row would
value-mask the r4133 channel by accident. Every one of the **17** `SKIP_PROPS`
rows (`SKIP_PROPS`, `mod.rs:3066`) is dispositioned exactly once, in its own row comment —
**17 = 10 + 7**, the first two lists below. The third list is a separate table
(`LANE_SKIP_PROPS` is not a `SKIP_PROPS` row and the partition lock does not
union it), shown here because `skip_prop` consults it on the same call:

| list | rows | on r4133 | why |
|---|---|---|---|
| `SKIP_PROPS_CAPI_ONLY` (`mod.rs:3422`) | 10 | **compared** | the justification is a 0.14.5-capture fact: the three changed defaults (`Fuse.FuseCurve`, `Fuse.RatedCurrent`, `RegControl.RevThreshold`), the two `pctperm` rows (`Capacitor`, `Reactor`), and RP3.8's five `''`-render rows (`IndMach012.PF`, the four `StorageController` totals) |
| `SKIP_PROPS_BOTH_CHANNELS` (`mod.rs:3448`) | 7 | **skipped** | channel-independent facts — the heap-garbage matrix reads (`Capacitor.CMatrix`, `Reactor.RMatrix`/`XMatrix`, `Fault.GMatrix`, `Transformer.WdgCurrents`) and the two `FaultRate` rows |
| `LANE_SKIP_PROPS` (`mod.rs:3503`) | 1 | **skipped, deliberately channel-blind** | `(Monitor, BaseFreq)` — an upstream bug BOTH gating oracles share (`Monitor.pas` r4133:552); the port's correct value is pinned by `monitor_basefreq_inherits_the_fundamental` |

*Partition lock:*
`skip_props_disposition_tests::every_skip_props_row_has_an_r4133_disposition`
(`mod.rs:3553`) fails on a row listed twice, in neither list, or deleted from
`SKIP_PROPS` — a new skip cannot silently inherit "masked on r4133 too". The
channel-blindness of the `LANE_SKIP_PROPS` row has its own pin
(`the_monitor_basefreq_exclusion_is_channel_blind`, `mod.rs:3704`). The two
**whole-element** skips are channel-scoped the same way: Recloser and Relay are
skipped on capi only, because their Rust tables are r4133-shaped
(`skip_whole_element`, `mod.rs:4682`;
`recloser_and_relay_are_whole_element_skipped_on_capi_only`, `mod.rs:3735`).

**Did the chain run at all?** `props_norm::assert_r4133_props_compare_ran`
(`props_norm.rs:2841`) runs first in the gate epilogue (`corpus_gate.rs:204`),
so a wholesale re-mask reports as one line instead of 19 stale-row messages; a
*partial* re-mask is caught instead by the forcing-rule lock
`scheduler::the_property_forcing_rule_is_every_live_non_large_case`
(`corpus_gate/scheduler.rs:234`, `FORCED_PROPS_POPULATION` = (441, 310, 87, 44)
at `:157`).

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
(`S_SENTINELS`, `crates/dss-epri/src/modes.rs:204`). Three consequences:

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
  (`crates/dss-epri/src/dss.rs:985`) refuses an `S` probe on a family with no
  `S_SENTINELS` row, and a `V` probe on a family in `V_WITHOUT_SENTINEL`
  (`crates/dss-epri/src/modes.rs:180` — `CapacitorsV` writes **no** sentinel at
  all), rather than reporting a silent `Served`.

**Mode capability — measured, and the acceptance for all of WP-G1.** The modes
WP-G1 needs live once, as `ModeSpec` rows in `WP_G1_MODES`
(`crates/dss-epri/src/modes.rs:1499`), each carrying its (family, kind, mode)
triple, the `D*.pas` line of the `case` arm it transcribes, the `myType` tag a
`V` arm assigns, and any state the arm moves. `Engine::read_mode`
(`crates/dss-epri/src/dss.rs:1139`) takes the row **by reference** — a mode number
cannot drift between the table and its reader — and rejects a reply whose shape is
not the row's, so a future DLL revision fails loudly instead of decoding garbage.
`r4133_mode_capability_is_complete_for_wp_g1`
(`crates/dss-epri/tests/modes.rs:111`) proves against the real DLL, on a solved
IEEE13 with an EnergyMeter attached, that **all 102 rows classify `Served`** and
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
surface and gets its own. Per-mode *value* validation against the capi channel
stays each surface sub-step's job (decision D2).

Three rules that table carries, each of which a capture must respect:

* **Getters only.** The generic reader supplies a neutral argument, so a DDLL
  *write* arm would be executed. `PDElements F:1` (`FaultRate`) and `F:3`
  (`PctPermanent`) are write arms that return the pre-`case` default `0.0` rather
  than the `-1.0` sentinel — invisible downstream — so they are recorded in
  `EXCLUDED_WRITE_MODES` (`crates/dss-epri/src/modes.rs:1693`) instead of the
  table, alongside their readers (`F:0`, `F:2`). G1.6b added a third row of a
  different shape: `PDElements S:1` (`Name`) is a **write** arm that re-selects
  `ActiveCktElement` by searching the whole `PDElements` list for its argument,
  so the generic reader's neutral `""` matches nothing and leaves the
  pointer-list cursor past the end of the list — it silently truncates an
  in-progress walk instead of storing a number. G1.3a added a fourth, on another
  family: `CktElement I:13` (`Enabled`) is the write arm of the `I:12` reader its
  enabled-only polar capture needs, and the neutral argument `0` would DISABLE
  the active element. Hence 102 rows, not 106 (the table was 96 before G1.6b's
  three `PDElements` walk arms, G1.3a's `CktElement.Enabled` + `CktElementI(12)`
  and G1.4a's `Bus.Nodes` + `Circuit.AllBusNames`).
  G1.3d(i) adds no row and live-compares **five more** of the 102 —
  `CktElementI` 0/1/2 (`NumTerminals`/`NumConductors`/`NumPhases`),
  `CktElementV(17)` (`NodeOrder`) and `CktElementS(4)` (`EnergyMeter`).
* **`ModeEffect` is the authority on what a row moves, and it carries the
  capture-order partition.** `Impure` rows move state: `Meters.Totals` re-runs
  `TotalizeMeters`; `PDElements.ParentPDElement` re-points `ActiveCktElement`;
  the `Topology` rows build and memoize `GetTopology` and walk a `PointerList`
  cursor to exhaustion; `CktElement.Has{Switch,Volt}Control` walk a `PointerList`
  to exhaustion; and all five `Circuit` aggregate rows — `Losses` (V:0),
  `LineLosses` (V:1), `SubstationLosses` (V:2), `TotalPower` (V:3) and
  `AllElementLosses` (V:8) — walk a `TPointerList` to exhaustion *and* call
  `Get_Losses`/`Get_Power` → `ComputeIterminal` on everything they walk
  (`Common/CktElement.pas:743`, `:677-680`; `Circuit.Losses` does it one level
  down, in `TDSSCircuit.Get_Losses`, `Common/Circuit.pas:2436-2443`). Those five
  read `Pure` until G1.9 measured them against the criterion the `Topology` rows
  already used (`CIRCUIT_LOSSES`, `crates/dss-epri/src/modes.rs:880`); the label
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
`crates/dss-epri/src/modes.rs:298`):

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
(`crates/dss-epri/src/dss.rs:1638`), so G1.7 added **no FFI**. The family's
remaining modes — `TopologyI(3..12)`, all of `TopologyS`, and `TopologyV`'s cursor
arms — are never bound and never called: they reassign
`ActiveCircuit.ActiveCktElement` and would poison the per-element capture of the
same step. That is not a capability gap (the DLL serves them) but a deliberate
non-use, asserted from the source text by
`capture_order.rs::the_r4133_bridge_exposes_only_the_six_order_free_topology_rows`.
Note for any later surface that walks `PDElements`/`PCElements`: the four list
modes leave those `TPointerList` cursors at the end, so a reader after the
topology capture must re-seek with `.First`.


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
