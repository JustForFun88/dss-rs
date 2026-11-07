# tools/golden — golden generation (manual only)

Generators (`gen_*.py`) and probes for the **committed** goldens under
`tests/golden/`, plus the version pin of the dss-python that captures them.

- **`PIN.txt`** — the exact capture engine: dss-python 0.15.7 / dss_capi 0.14.5.
  Every capture anchored `capi_v0145` in `tests/golden/golden.lock.json` is
  produced with this and only this.
- **`gen_*.py`** — one generator per golden family (table below). Each runs the
  pinned dss-python, captures the result, and writes `tests/golden/<family>/…`.
  `generate.py` + `cases.json` drive the named-feeder goldens. Exceptions run
  other engines: the r4133-specific arms (`gen_protection.py`
  fuse_blow/swt_manual, `gen_flicker.py`) drive the official EPRI r4133 DLL
  through the in-house `epri-worker` bridge (`tools/opendss/epri_worker.py`),
  and the frozen capi015 captures ran on a dss_capi 0.15.0b4 stack that no
  longer exists: `gen_ncim_reports.py`, `gen_bh_capi015.py`,
  `gen_regcontrol_capi015.py` and the `DSS_ORACLE_ENGINE=capi015` arms of
  `gen_reports.py` (the seasonal exports) and `gen_der_lines_harmonics.py`
  (`line_geometry_carson`) wrote them, and three `props/` dumps (`swtcontrol`,
  `linemedium`, `linespacing_eqspacing`) came from capi015 runs no committed
  generator reproduces as-is (each one's `oracle` block names its engine). Every
  file one of those calls wrote, payload and `.meta.json` sidecar alike, is
  locked `capi015` — the seventeen of `golden_lock.rs::CAPI015_ARTIFACTS`. Not
  every file a generator writes is a capture: `gen_feeders_controlsoff.py` also
  writes the controls-off INPUT decks, and
  `tests/golden/json/schema_divergences.json` is hand-authored (no generator).
  Their lock rows say so in `reason`. Nine `capi_v0145` Dump goldens
  (`reports/dump3_{bare,commands,debug}.txt`,
  `reports/dump_{autotrans,autotrans3,regcontrol,transformer,transformer3,transformer_disabled}.txt`)
  also carry hand-landed lines the pinned dss-python cannot render. Their
  `reason` names those lines, and a pinned-dss-python regen must re-land them.
- **`report_decks/`** — fixture decks the report goldens (`gen_reports.py`)
  replay.
- **`probe_*.py`** — one-off empirical probes that measure what an OpenDSS
  engine does, not part of any gate.

## Families

| golden dir | generator | Rust driver | covers |
|---|---|---|---|
| `feeders_controlsoff/` + `.json` | `gen_feeders_controlsoff.py` | `golden_feeders.rs` | IEEE13/37/123 snapshot, controls off |
| `timeseries_controls/` | `gen_timeseries_controls.py` | `golden_timeseries_controls.rs` | daily/duty loadshapes, Reg/Cap control, event log |
| `metering_monitors/` | `gen_metering_monitors.py` | `golden_metering_monitors.rs` | EnergyMeter registers/zones, Monitor channels, generators |
| `line_constants/` | `gen_der_lines_harmonics.py` | `golden_line_constants.rs` | Carson Z/Yc via geometry/spacing/cable |
| `der_controls/` | the same generator | `golden_der_controls.rs` | PVSystem, Storage, StorageController, InvControl, ExpControl |
| `harmonics/` | the same generator | `golden_harmonics.rs` | harmonic frequency sweep |
| `protection/` | `gen_protection.py` | `golden_protection.rs` | Recloser/Relay/Fuse/SwtControl trip-reclose |
| `reports/` | `gen_reports.py` | `golden_reports.rs` | Export/Show/Dump/Save byte-exact output (decks in `report_decks/`) |
| `checkpoints/` | `gen_checkpoints.py` | `golden_checkpoints.rs` | per-step assembled model (Y, YPrim, injection, discrete state) |
| `plot_callback/` | `gen_plot_callback.py` | `golden_plot_callback.rs` | `Plot`/`Visualize` callback `plotParams` JSON payload |
| `props/` | `gen_props.py` | `props_roundtrip.rs` | per-class property round-trip |
| `cim/` | `gen_cim.py` | `golden_cim.rs` | CIM/XML export round-trip |
| `ncim/` | `gen_ncim_reports.py` | `ncim_reports.rs` | NCIM Jacobian/deltaF/deltaZ/PV2PQ reports |
| `inc_matrix/` | `gen_inc_matrix.py` | `inc_matrix_reports.rs` | incidence/BusLevel/Laplacian reports |
| `flicker/` | `gen_flicker.py` | `golden_flicker.rs` | Pst flicker meter |
| `pstcalc/` | `gen_pstcalc.py` | `golden_pstcalc.rs` | IEC Pst calculator |
| `json/` | `gen_json.py`, `gen_schema.py` | `golden_json.rs`, `golden_schema.rs` | AltDSS JSON export byte goldens + the AltDSS JSON-schema golden |
| `json_import/` | `gen_json_import.py` | `golden_json_import.rs` | `Circuit_FromJSON` round-trip (import, then re-export) |
| `wasm_usermodels/` | the ignored tests `crates/dss-epri/tests/gen_wasm_usermodels*.rs` | `wasm_usermodels*.rs` | WASM user models against their native twins on r4133 (`tools/wasm_usermodel/README.md`) |
| `adiakoptics/` | in-test (`DSS_REGEN_AD_GOLDEN`) | `adiakoptics.rs` | A-Diakoptics init/solve matrices (ZLL/ZCC/Y4) |
| `ieee*.json`, `slice`, `allocation`, `autoadd_reduce`, `gendispatcher`, `ieee8500`, `reliability`, `parser` | `generate.py` / `gen_<name>.py` | `golden_smoke.rs`, `golden_feeders_controls.rs`, `golden_slice.rs`, … | named feeders / features |

The Rust drivers live in `crates/dss-core/tests/`. The `der_controls`,
`line_constants` and `harmonics` drivers share one replay engine,
`harness::scenario::check_family`.

**Frozen generator arms.** `gen_bh_capi015.py`, `gen_regcontrol_capi015.py`,
`gen_fuse_r4133.py` and `gen_checkpoints.py`'s `DSS_ORACLE_ENGINE=capi015` arm
(it reads `tools/opendss/PIN_OPENDSS.txt`, which is not in the tree) target
engine stacks that are not installed. They are dead paths kept as provenance:
the goldens they produced are pinned and frozen, and the live `capi` arms are
unaffected. The r4133 arms of `gen_flicker.py` and `gen_protection.py` run:
they drive the git-tracked r4133 DLL through `epri-worker`. The committed
`flicker/` golden is an r3723 capture, and the r4133 regen reproduces its
payload byte for byte.

## Regenerate a golden

Manual and deliberate, never in CI. Goldens pin intentional OpenDSS
inexactnesses (`TODO(compat)`), so improved precision reads as a bug: do not
regenerate to "fix" a diff.

1. Install the pinned dss-python from `PIN.txt`, then run the matching
   `gen_*.py`. The r4133 arms of `gen_protection.py` and `gen_flicker.py` need
   no venv. `DSS_GOLDEN_OUT=<scratch dir>` redirects their output for a parity
   run, whose acceptance is payload byte-identity with the committed golden.
2. **Adding a deck is not a reason to rewrite the existing bytes.**
   `python tools/golden/gen_json.py <deck> [<deck>…]` writes only those files
   (an unknown name is rejected). The unfiltered run rewrites all
   `tests/golden/json/*.json` and would fold in any engine or toolchain drift
   since they were captured. Every committed deck golden needs a
   `run_deck("<stem>")` driver in `crates/dss-core/tests/golden_json.rs`: the
   directory guard `json_every_deck_golden_has_a_driver` fails otherwise.
3. This covers every **externally anchored** artifact (anchored `capi_v0145`,
   `r4133`, `r3723`, `capi015` or `fpc_3.2.2` in `golden.lock.json`), and two of
   the four `self` rows: `gen_props.py` writes `props/recloser.json` and
   `props/relay.json`, whose committed values are this engine's own renders (a
   regen repeats the manual r4133 cross-validation their lock reasons
   describe). The other two `self` artifacts have in-test knobs:
   `DSS_REGEN_AD_GOLDEN` (`adiakoptics.rs::torn_tree_matches_golden`) and
   `REGEN_SCHEMA_PORT` (`golden_schema.rs::full_document_matches_port_golden`).
4. Refresh the provenance lock and review its digest diff:
   `DSS_UPDATE_GOLDEN_LOCK=1 cargo test -p dss-core --test golden_lock -- --nocapture`
   (through the build queue, `tools/gate/README.md`). Commit
   `tests/golden/golden.lock.json` with the change that moved the bytes, and
   name every moved artifact and its cause in the commit body.

The rules for a self-anchored artifact (clean tree, never to fix a red gate,
one family per run, the lock digest diff as the review artifact) and the
`DSS_UPDATE_GOLDENS` write rails are in the module doc of `harness::regen`
(`crates/dss-test-harness/src/harness/regen.rs`). `-- --nocapture` is required
on every regen run: the run passes, and libtest drops a passing test's output,
including the rails' `SNAPSHOT` / `REFUSED` lines and the lock's `SEEDED` /
`RE-ANCHORED` announcements.
