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
it. Nothing here has an `#[ignore]` or an env gate that could green on zero
matches.

## The layers

| layer | what it checks | where | oracle |
|---|---|---|---|
| **unit tests** | per-module algorithms, Pascal-cited numerics | `crates/*/src/**` (`#[cfg(test)]`, `exec/tests/`) | pins inline in code |
| **golden gate** | committed input→output pins, replayed offline | `tests/golden/` + `crates/dss-core/tests/golden_*.rs` + `tests/harness/` | pinned dss-python, **manual** regen only |
| **live oracle gate** | full assembled model (Y / V / currents / powers / discrete state), per step, live | `corpus_live.rs` + `tools/oracle/oracle_server.py` | pinned dss-python, at test time |
| **corpus hygiene** | no silent omission: every `.dss` classified, every family a dir↔manifest bijection | `corpus_manifest.rs`, `*_manifest_is_complete` | none (structural) |
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
| `props/` | `gen_props.py` | `props_roundtrip.rs` | per-class property round-trip |
| `ieee*.json`, `slice`, `allocation`, `autoadd_reduce`, `gendispatcher`, `ieee8500`, `reliability`, `parser` | `generate.py` / `gen_<name>.py` | `golden_smoke.rs`, `golden_feeders_controls.rs`, `golden_slice.rs`, … | named feeders / features |

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
the deck (e.g. `modes/shape_binfiles/`).

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

## Procedures

**Run the gate** — the three commands above. Keep `tests/corpus` pristine
afterwards (`git status tests/corpus`): the live gate executes decks in place;
the `CorpusGuard` restores them, but re-vendor if anything lingers.

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
