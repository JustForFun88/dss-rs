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

Two rails keep this list honest as the teardown proceeds, both in
`crates/dss-core/tests/oracle_parity_cfg_gate.rs`: every `compat::` alias any
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

### The parity↔default differential gate

The two lanes are two *builds*, so no `#[test]` can compare them. That
comparison is a **scripted job**:

```
pwsh -File tools/lanes/lane_diff.ps1            # build both lanes, dump, diff
pwsh -File tools/lanes/lane_diff.ps1 -SkipDump  # re-diff existing dumps
```

It builds `crates/dss-core/examples/lane_dump.rs` once per lane (each into its
own target dir under `target/lanes/`, so re-runs do not thrash the other lane's
cache), walks **all 521 manifest cases** on each engine — solving the 517 that
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

### The unified corpus gate (`crates/dss-core/tests/corpus_gate.rs`)

One scheduler-driven `#[test]` — `corpus_gate_all_cases_match_engines` — runs
the union of all four case manifests live: the vendored family
`tests/corpus/manifests/solvable_now.json` (293 decks from the
`tests/corpus/electricdss-tst` mirror) plus the synthetic families
`asymmetric` (47) / `controls` (105) / `modes` (69) — 514 cases. Each case's
**`engines`** field names its gating channel(s): `"capi_v0145"`, `"r4133"`, or
`"both"` (the default; 360 cases gate on both channels). Case key = the gate
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

### The divergence ledger (`tests/corpus/ledger.json`)

The gating successor of the report-only `known_diffs.json`. Every entry pins
**where** and **how much** one case may diverge from one channel's oracle, with
a mandatory documented cause — and **fails the gate when stale**, so the ledger
can never rot into a soft-tolerance backdoor. Tier floors in
`tests/harness` (`Tolerances`/`tol_for`, `tests/TOLERANCE_NOTES.md`) are
structurally unreachable from ledger code and never change here.

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

Scope `field` must be one of the **13 implemented** handlers — `iterations`,
`voltages`, `injection`, `element`, `probe`, `property`, `monitor`, `eventlog`,
`ctrlqueue`, plus the four **exclusion-only** ones `y`, `y_fingerprint`,
`yprim`, `meter` — anything else (typo or the §1.3-planned but unimplemented
`global_result`) is rejected loudly at load. The exclusion-only four name a
whole compared artifact rather than a value with a natural envelope (the
assembled system Y, its fingerprint, one element's YPrim, one EnergyMeter's
register block), so `assert_structural` refuses them on a `divergence`.
Location selectors: `node_re`/`name_re`/`channel_idx` (0-based)/`channels`,
optional `steps` (0-based). `iterations` takes exact `{rust, oracle}` pairs or
`policy: "rust_le_oracle"`; `yprim`/`monitor`/`meter`/`probe` exclusions select
the artifact by `name_re` (absent ⇒ all). The reverse rule holds too: an
`exclusion` may name only a field the exclusion path actually handles
(`voltages`, `element`, `injection`, `monitor`, `probe` + the four above) —
`iterations`, `property`, `eventlog` and `ctrlqueue` are divergence-only,
because their handlers re-assert a pin or rewrite the oracle's line, and a scope
that loads cleanly and then never applies is the one thing the field whitelist
exists to prevent.

Runtime rules: every applicable entry must be **hit** ≥ 1 (never-applied →
gate fails), every `divergence` — and every `exclusion` carrying a `voltages`
scope — must still exceed the tier floor somewhere (fail-on-stale; the
`divergence` half proven live by canary in the Phase D/E audits, the
`exclusion` half by `a_voltages_exclusion_that_masks_nothing_is_stale`). The
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

Current contents: 36 entries over 23 documented causes — 4 r4133 `skip`
(#303 crash decks), 21 r4133 `divergence` (Delphi 6-sig-fig display-precision
probes on Storage/PVSystem, FPC-vs-Delphi injection/element ulp floors on the
IndMach asymmetric decks, one monitor sequence-magnitude drift, the GFM
`%stored` rounding class, and the RegControl `idle`
revThreshold/fwdThreshold getter-convention exact-pair), 5 capi_v0145
`divergence` (the `line_spacing_asym` and the Generator `MakePosSequence`
exact-pair-numeric upgrade pins, plus three G2.5 property-jump entries —
`GICTransformer.tg3/tg5.R2` and `Capacitor.cap_cmat.Cuf`/`NormAmps`/`EmergAmps`,
pinned as exact pairs rather than skipped), and 6 `exclusion` — 4 capi_v0145 + 2
r4133 — from
`GOLDEN_REBASE_PLAN.md` G2.5, where the engine stopped reproducing three
upstream bugs (GICTransformer `%R2`, Capacitor `MakePosSequence` `Cuf`,
LoadShape MMF accept-set) and the four decks that observe them therefore
diverge from their gating channel(s) across the solved model.

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
variables/eventlog/ctrlqueue/all-properties/global-result/autoadd-log/pending/
solve-abort), `engines`, `isolate`, `defer`, and the per-channel ledger entry
digests. `population_lock.rs` (unconditional, plain `cargo test`) rebuilds the
fingerprint from the current manifests + ledger and asserts it equals the lock;
any drift — a path leaving `solvable_now`, a retained deck weakened in place, a
`both → capi_v0145` engine narrowing, a new/widened ledger entry, or any count
change — fails with a precise diff and the one-command regeneration path, so a
shrink lands as a **reviewable diff in the lock file**, never unnoticed.

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
`CAPI015_ARTIFACTS`, `R4133_FAMILIES`, `FPC_ARTIFACT`, `R3723_TREE`, with
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
per-row `disposition` in `props_census.json` stays lossless. It adds three per-channel files — `claims.txt`
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
is **declared** for the sub-step that will claim it, that every
`PROPS_NORM_R4133` row claims at least one row (offline liveness — the live half
is the per-row hit accounting, dormant until RP4.1), and that each table row's
`(pair, bin, cells)` citation matches the vendored evidence it names. The
supplement (`tests/corpus/props_r4133/examples_supplement.txt`) is measured, not
frozen: the 26 pairs no 2026-08-08 row can carry (the WP-RP1 shape closures plus
the two `regcontrol` threshold pairs), and the replay parses the README's WP-RP1
tables directly — **reformatting that README section reds a test on purpose**.
Where the vendored evidence is knowingly behind the live population it says so
with an assertion rather than a comment: `LIVE_ONLY_SPELLINGS` names the one
spelling the claims census sees and no file may carry (`autotrans.conn
'series'/'Series'`, on a pair whose `bins.tsv` row predates RP1.2's deck),
reconciles the replay's 748 claimed spellings with the census's 749, and pins
that the shipped rule still claims it.

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

### 0.15.x property-table allowlist (`PROPS_015X`)

The corpus gate's property-parity check (`harness::compare_all_properties`)
asserts the Rust property-table **shape** (count + name order) against the
oracle capture. It runs on the **capi_v0145 channel only** — the scheduler
masks `all_properties` off the r4133 request (r4133's 0.15.x-shaped tables are
exactly what the allowlist exists to bridge); all three synthetic families
force it on for their capi-gating live cases. The pinned oracle is dss_capi
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
| `DSS_PROPS_CENSUS` | corpus_gate | `1` → arms the property **census** test `corpus_gate_props_census` (`R4133_PROPS_PLAN.md` RP0.2; a separate `#[test]`, so the var can never divert the mandatory gate): walk every live non-`large` case on BOTH channels with `all_properties` forced on (the §1.1 r4133 masks bypassed), collect every divergent cell instead of asserting, write `tmp/props_census.json` + `tmp/props_census/run.json` + `tmp/props_census/<channel>/{structural_pairs,numeric_pairs,examples_full,shape,summary}` in the RP0.1 extract format. Honors `DSS_GATE_ONLY` (stamped into the artifacts); **asserts nothing** — a divergence is the measurement. `claims` → the same walk in RP2.1's **disposition** mode: every value row annotated `normalized-by-<rule>`/`echo-row`/`under-floor`/`ledger-hit`/`UNCLAIMED` through the shipped policy predicates, plus `claims{,_unclaimed_pairs,_summary}` per channel and the in-scope split; plain-mode artifacts are byte-identical either way. Any other value fails loudly |
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
`git restore`/path-limited `git clean` the subtree if anything lingers.

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
in-test knobs — `DSS_REGEN_AD_GOLDEN` (`adiakoptics.rs:578`) and
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
