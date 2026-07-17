# How dss-rs is tested

The single entry point for the test infrastructure. `CLAUDE.md` owns the
**gate rule** (what must be green to commit); this file explains the **layers**,
the **knobs**, and the **procedures** (regenerate goldens, add a corpus deck,
triage an EPRI divergence). Read `PORTING_PLAN.md` first for the binding
decisions the tests encode.

## The mandatory gate

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

`cargo test` runs everything below except the opt-in EPRI channel. The pinned
dss-python oracle (`tools/golden/PIN.txt`, 0.15.7 / dss_capi 0.14.5) **must be
installed** — the live gate calls it in-process and fails (not skips) without
it. No test that **participates** in the gate can green on zero matches or
silently skip: the corpus/family gates assert a non-empty, count-locked
population (`population_lock.rs`, `*_manifest_is_complete`, `solvable_now_has_multistep_depth`),
and the live comparison fails (not skips) if the oracle is absent. The gate
carries exactly **two** ignored items, both non-gating and deliberate: one
`#[ignore]`d diagnostic (`adiakoptics::ckt24_graph_diagnostic` — a `.graph`
inventory probe, run with `--ignored`, pending the WP-AD.5 driver) and one
illustrative ` ```ignore ` doctest (the `define_properties!` macro-DSL snippet
in `obj/props/mod.rs`, which cannot compile standalone). The `DSS_LIVE_*` /
`DSS_EXPENSIVE_TESTS` / `DSS_AD_*` env knobs below are opt-in **diagnostics**
outside the gate — they print `SKIPPED` when unset and never gate a commit.

The dev/test profile carries `opt-level = 3` overrides for the engine crates
and all dependencies (workspace `Cargo.toml`): the live gate runs
yearly/8500-node decks through the engine, and unoptimized codegen makes a
single yearly EPRI-feeder deck cost ~14 min (~×10). Test binaries themselves
stay at opt 0; float results are opt-level-independent (no fast-math in Rust —
pinned empirically by the corpus-wide exact-iteration-count contract). The
safety knobs are orthogonal to opt-level and stay on: slice bounds checks are
never removed at any opt-level, and `overflow-checks`/`debug-assertions` are
pinned `true` explicitly in the overrides — the reason the gate uses this
instead of `--release` (which sets overflow-checks=false).

## The layers

| layer | what it checks | where | oracle |
|---|---|---|---|
| **unit tests** | per-module algorithms, Pascal-cited numerics | `crates/*/src/**` (`#[cfg(test)]`, `exec/tests/`) | pins inline in code |
| **golden gate** | committed input→output pins, replayed offline | `tests/golden/` + `crates/dss-core/tests/golden_*.rs` + `tests/harness/` | pinned dss-python, **manual** regen only |
| **live oracle gate** | full assembled model (Y / V / currents / powers / discrete state), per step, live | `corpus_live.rs` + `tools/oracle/oracle_server.py` | pinned dss-python, at test time |
| **corpus hygiene** | no silent omission: every `.dss` classified, every family a dir↔manifest bijection; no silent **shrink** of the gated population | `corpus_manifest.rs`, `population_lock.rs`, `*_manifest_is_complete` | none (structural) |
| **EPRI channel** (opt-in) | inventory upstream deltas vs official EPRI binaries | `corpus_live_opendss`, `tools/opendss/` | Oddie-bridged EPRI DLLs |

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
| `ncim/` | `gen_ncim_reports.py` | `ncim_reports.rs` | NCIM Jacobian/deltaF/deltaZ/PV2PQ reports (vs capi015 oracle) |
| `inc_matrix/` | `gen_inc_matrix.py` | `inc_matrix_reports.rs` | incidence/BusLevel/Laplacian reports |
| `flicker/` | `gen_flicker.py` | `golden_flicker.rs` | Pst flicker meter |
| `pstcalc/` | `gen_pstcalc.py` | `golden_pstcalc.rs` | IEC Pst calculator |
| `json/` | `gen_json.py` | `golden_json.rs` | AltDSS JSON export (`DSSElement_ToJSON`/`ToJSON`) byte goldens |
| `adiakoptics/` | in-test (`DSS_REGEN_AD_GOLDEN=1`) | `adiakoptics.rs` | A-Diakoptics init/solve matrices (ZLL/ZCC/Y4) |
| `ieee*.json`, `slice`, `allocation`, `autoadd_reduce`, `gendispatcher`, `ieee8500`, `reliability`, `parser` | `generate.py` / `gen_<name>.py` | `golden_smoke.rs`, `golden_feeders_controls.rs`, `golden_slice.rs`, … | named feeders / features |

(The capi015/r4133 props goldens — `gen_bh_capi015.py`, `gen_regcontrol_capi015.py`,
`gen_fuse_r4133.py` — regenerate individual `props/` captures against the Oddie
oracle; `gen_help_catalog.py` pins the `help` catalog. They share `props_roundtrip.rs`.)

The three `der_controls` / `line_constants` / `harmonics` gates share one
replay engine, `tests/harness/scenario.rs::check_family`.

### Synthetic corpus families (`tests/corpus/{asymmetric,controls,modes}/`)

Hand-written / generated decks that run the **live** mandate beside the vendored
`electricdss-tst` mirror, each with a `manifest.json` and a
`<family>_manifest_is_complete` + `<family>_cases_match_oracle` gate pair:

- `asymmetric/` — orientation-sensitive YPrim stamping (the transposed-stamp class);
- `controls/` — control/protection/metering with per-step state channels;
- `modes/` — solve modes, algorithms, input formats, executive verbs.

A case marked **`pending: true`** covers a feature the port does not implement
yet: the gate asserts the Rust engine **errors loudly** on it (never a silent
fallback), and the WP named in its `wp` field flips the flag when it ports the
feature (GAPS_PLAN.md §3.1). Multi-file cases live in a subfolder named after
the deck (e.g. `modes/inputformat/shape_binfiles/`).

A case with an **`oracle`** field (UPGRADE_PLAN.md target-rev gating) is
live-compared against **that** engine instead of the pinned capi oracle:
`"capi015"` = the dss_capi 0.15.x-line oracle (dss-python 0.16.0b2 from the
Oddie venv, backend 0.15.0b4 / OpenDSS r4103), `"r3723"|"r4088"|"r4133"` = an
official EPRI `OpenDSSDirect.dll` via the Oddie bridge. For such a case the
iteration policy relaxes to **Rust ≤ oracle** (exact equality stays mandatory
against the pinned 0.14.5 oracle); everything else (voltages, Y,
currents/powers, discrete state, tolerances) uses the same shared comparators,
never weakened. An upgrade WP flips a case's `oracle` in the **same commit**
that ports the newer upstream behavior the case covers. The
`modes/upgrade/upgrade_pilot.dss` case keeps this machinery permanently exercised,
which makes the Oddie venv + `tools/opendss/bin/` binaries a **mandatory**
`cargo test` prerequisite (like the pinned oracle itself; setup in
`tools/opendss/README.md`).

### Anti-shrink population lock (`population_lock.rs`)

The mandatory gate defines its own population — `solvable_now.json` is the set of
decks `corpus_live.rs` compiles + live-compares, and the other manifests bucket
the rest. Because that classification is *self-defined*, a port regression could
be silently neutralized by moving a deck out of `solvable_now` into a skip bucket:
`cargo test` stays green while real coverage shrinks in a one-line manifest edit.

`tests/corpus/manifests/population.lock.json` is a committed fingerprint of the
population — per-manifest case counts; for every `solvable_now` case its path **and
a per-case rigor fingerprint** (kind/tolerance-tier, oracle target, `n_steps`, and
every compare-depth flag — selected_elements/meters-monitors/probes/variables/
eventlog/ctrlqueue/all-properties/global-result/autoadd-log/pending/solve-abort);
and the three synthetic families' case counts **and path lists**. `population_lock.rs`
(unconditional, plain `cargo test`) rebuilds that fingerprint from the current
manifests and asserts it equals the lock; any drift — a path leaving `solvable_now`,
a **retained deck weakened in place** (kind flipped to a looser band, steps/probes/
meters cut — a value-fingerprint change), a family deck swapped at equal count, or
any count change — fails with a precise diff and the one-command regeneration path,
so a shrink lands as a **reviewable diff in the lock file**, never unnoticed.

**Regenerate the population lock** (deliberate — after intentionally re-classifying
decks, never to silence an unreviewed failure):

```
DSS_UPDATE_POPULATION_LOCK=1 cargo test -p dss-core --test population_lock
```

writes the lock from the current manifests. Commit the `population.lock.json`
diff **together with** the manifest change that caused it.

### 0.15.x property-table allowlist (`PROPS_015X`)

`corpus_live`'s property-parity gate (`harness::compare_all_properties`) asserts
the Rust property-table **shape** (count + name order) against the oracle capture.
The pinned default oracle is dss_capi **0.14.5**, so a deliberately ported 0.15.x
property (which cannot appear in a 0.14.5 capture) is declared in the named
per-class allowlist `PROPS_015X` (`tests/harness/mod.rs`): a Rust-side prop in the
allowlist and absent from the capture is excluded from the shape walk (handles
inserted props, not just trailing). Present-in-capture (capi015) props are NOT
excluded — full name+value compare still applies. It relaxes shape only, never a
value tolerance, and a non-allowlisted extra/missing/misordered prop still fails.
Rules + row-documentation requirements: `tests/TOLERANCE_NOTES.md` §"0.15.x
property-table allowlist (shape relaxation)".

## Environment variables

| var | consumer | meaning |
|---|---|---|
| `DSS_ORACLE_PYTHON` | corpus_live / golden gen | interpreter for the **pinned** oracle (default `python`) |
| `DSS_ORACLE_TIMEOUT_SECS` | corpus_live | per-case oracle timeout (default 120) |
| `DSS_LIVE_CLASSIFY` | corpus_live | `1` → probe `skipped_needs_investigation` candidates, write `tmp/classify_report.json` |
| `DSS_LIVE_OPENDSS` | corpus_live | `r3723`/`r4088`/`r4133` → run the opt-in EPRI A/B report |
| `DSS_LIVE_OPENDSS_ASSERT` | corpus_live | `1` → fail on **new** (uncataloged) EPRI divergences |
| `DSS_ORACLE_ENGINE`, `DSS_OPENDSS_REV`, `DSS_OPENDSS_DLL`, `DSS_OPENDSS_EXPECT` | oracle_server | select/point the EPRI engine (see `tools/opendss/README.md`) |
| `DSS_OPENDSS_PYTHON` | corpus_live / ab_compare | the separate Oddie venv interpreter |
| `DSS_UPDATE_POPULATION_LOCK` | population_lock | `1` → rewrite `population.lock.json` from the current manifests (deliberate regen) |
| `DSS_LIVE_PROPS` | corpus_live | `1` → opt-in all-property parity sweep over the corpus (diagnostic; prints `SKIPPED` unset) |
| `DSS_LIVE_PROPS_MAX` | corpus_live | cap the `DSS_LIVE_PROPS` sweep at `<n>` cases |
| `DSS_EXPENSIVE_TESTS` | adiakoptics | `1` → run the 168-step (yearly) A-Diakoptics time-series variant instead of daily-24 |
| `DSS_REGEN_AD_GOLDEN` | adiakoptics | `1` → rewrite the committed A-Diakoptics matrix golden (deliberate regen) |
| `DSS_AD_CLASSIFY`, `DSS_AD_DECOMPOSE` | corpus_live | throwaway A-Diakoptics triage probes (classify off-tier decks / split the AD-vs-normal gap into save+stitch legs) |

## Procedures

**Run the gate** — the three commands above. Keep `tests/corpus` pristine
afterwards (`git status tests/corpus`): the live gate executes decks in place;
the `CorpusGuard` (recursive since WP8.8 — it also removes run-created files
inside pre-existing fixture subdirs) restores each case dir on both engine
sides, but a run that writes OUTSIDE the case-dir tree (e.g. a manual
`dss-cli` invocation) is uncoverable — `git restore`/`git clean` the subtree
if anything lingers.

**Regenerate a golden** (manual, deliberate — never in CI): install the pinned
venv from `tools/golden/PIN.txt`, then run the matching `tools/golden/gen_*.py`.
Goldens pin intentional upstream inexactnesses (`TODO(compat)`), so improved
precision reads as a porting bug — do not regenerate to "fix" a diff.

**Re-vendor the corpus** — `python tools/corpus/vendor.py --force`, then review
the `tests/corpus/SHA256SUMS` diff.

**Add a corpus deck** — pick the family by content (static→`asymmetric`,
control/time-series→`controls`, mode/algorithm/format/verb→`modes`); add the
`.dss` (a subfolder if it needs fixtures) + a `manifest.json` entry with its
`kind`/`n_steps`/`probes`; set `pending: true` + `wp` if the feature is
unported. Validate on the pinned oracle first (two-process determinism +
feature sensitivity, GAPS_PLAN.md §2.1). The `*_manifest_is_complete` gate
enforces the dir↔manifest bijection.

**Triage an EPRI divergence** — add an entry to `tests/corpus/known_diffs.json`
(`kind: "diff"` matched on the failure reason, or `kind: "skip"` for a case a
revision can't run at all), always with a `cause` and the `revs` it applies to.
Re-run `DSS_LIVE_OPENDSS=<rev> DSS_LIVE_OPENDSS_ASSERT=1 cargo test -p dss-core
--test corpus_live corpus_live_opendss`. The catalog is **never** consulted by
the mandatory gate.

**Diff two EPRI revisions** — `tools/opendss/ab_compare.py --a oddie:r3723 --b
oddie:r4133` (upstream-change inventory ahead of porting). Broad-surface
state dumps: `tools/opendss/dsspy_validation/`.

## See also

- `CLAUDE.md` — the gate rule, `TODO(compat)` convention, known upstream bugs.
- `tests/TOLERANCE_NOTES.md` — the calibrated tolerance tiers and why each holds.
- `tests/corpus/README.md` — the vendored corpus + the synthetic families.
- `tools/opendss/README.md` — the opt-in EPRI channel in full.
- `CORPUS_TEST_PLAN.md`, `CONTROL_COVERAGE_PLAN.md`, `GAPS_PLAN.md` — the plans.
