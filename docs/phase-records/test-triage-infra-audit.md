# Test-infrastructure audit (wt-t6)

Full revision of the test infrastructure against the tree: every TESTING.md claim
verified layer-by-layer (unit / byte-goldens / generators+PIN / checkpoint / live
corpus_live + multi-oracle + iteration policy / manifests + bijection +
population.lock + anti-deletion floors / EPRI sweeps + known_diffs / AD sweep /
props-roundtrip / save-roundtrip / eventlog+export comparators + masks +
TOLERANCE_NOTES). Source-integrity gate: `.inputs/dss_capi` = 186 `.pas` (PASS).

Overall the harness is strong: the comparators compare every field they claim
(Y / YPrim / injection / currents / powers / losses / discrete state / monitors /
meters / probes / variables / eventlog / ctrlqueue / all-properties), tolerances
are proof-backed, the population lock fingerprints per-case compare depth so a
retained deck cannot be weakened in place, and the manifest bijection forbids
silent omission. Findings below are almost entirely **doc staleness**; no
weakened-verification or coverage-hole regression was found in the gate itself.

---

## Fixed here (docs sync, committed on wt-t6)

### F1 — TESTING.md falsely claimed "Nothing here has an `#[ignore]`" (staleness)
**Evidence.** TESTING.md §"mandatory gate" asserted *"Nothing here has an
`#[ignore]` or an env gate that could green on zero matches."* The tree carries
**two** ignored items that show as `ignored` in the gate output:
- `crates/dss-core/tests/adiakoptics.rs:770` — `#[ignore]` on `ckt24_graph_diagnostic`
  (a non-gating `.graph` inventory probe; currently a partial stub pending the
  WP-AD.5 compile driver — it only records the vendored artifact and returns).
- `crates/dss-core/src/obj/props/mod.rs:49` — a ` ```ignore ` doctest illustrating
  the `define_properties!` macro DSL (cannot compile standalone).

Both are justified and non-gating, but the blanket wording was inaccurate.
**Disposition: FIXED** — reworded to state the real invariant (no *gate-participating*
test greens on zero matches / silently skips), name the two deliberate ignored
items, and clarify the `DSS_LIVE_*`/`DSS_EXPENSIVE_TESTS`/`DSS_AD_*` knobs are
opt-in diagnostics outside the gate.

### F2 — TESTING.md env-var table missing six live knobs (staleness/incompleteness)
**Evidence.** `grep env::var` over `crates/dss-core/tests` yields these vars NOT
in the TESTING.md table: `DSS_LIVE_PROPS`, `DSS_LIVE_PROPS_MAX`
(corpus_live.rs:2119-2165 all-property sweep), `DSS_EXPENSIVE_TESTS`
(adiakoptics.rs:1875 yearly variant), `DSS_REGEN_AD_GOLDEN` (adiakoptics.rs:578),
`DSS_AD_CLASSIFY`/`DSS_AD_DECOMPOSE` (corpus_live.rs:2863/2875). The table
presents itself as the knob map.
**Disposition: FIXED** — six rows added, each marked as an opt-in diagnostic.

### F3 — TESTING.md golden-families table omitted seven dedicated families (staleness)
**Evidence.** `tests/golden/` has `cim/ ncim/ inc_matrix/ flicker/ pstcalc/ json/
adiakoptics/` dirs, each with a generator (`gen_cim.py`, `gen_ncim_reports.py`,
`gen_inc_matrix.py`, `gen_flicker.py`, `gen_pstcalc.py`, `gen_json.py`; AD via
in-test `DSS_REGEN_AD_GOLDEN`) and a gate (`golden_cim.rs`, `ncim_reports.rs`,
`inc_matrix_reports.rs`, `golden_flicker.rs`, `golden_pstcalc.rs`,
`golden_json.rs`, `adiakoptics.rs`). None appeared in the golden-families table
and the catch-all row did not list them.
**Disposition: FIXED** — seven rows added; note added for the capi015/r4133
props-golden regenerators and `gen_help_catalog.py`.

### F4 — `micro_wtg3_dynamics` tolerance tier had no TOLERANCE_NOTES proof entry (weakened-verification doc gap)
**Evidence.** `harness::tol_for` defines a `micro_wtg3_dynamics` tier
(`crates/dss-core/tests/harness/mod.rs:713-722`; `v_rel` 8e-6 / `i_rel` 2e-5 /
`i_abs` 1e-4) with a full inline decomposition proof, but `tests/TOLERANCE_NOTES.md`
never listed it in the tolerance-classes summary (`grep -i wtg3` → 0 tier hits).
CLAUDE.md requires every band to carry a TOLERANCE_NOTES proof entry.
**Disposition: FIXED** — added a class-list bullet and a dedicated §wtg3-dynamics
section transcribing the PLL derivative-gain cancellation proof (non-PLL vars
≤2e-7, only `dOmg`/`Pgen`/`Qgen` loose via the 6e4× `Vq` amplification, gap
decays with the transient → not a state-leak). No numeric floor changed.

---

## Orchestrator-action / no-action-because (no code-behavioral change made here)

### N1 — Stale capture-machine paths in `skipped_oracle_issue.json` notes
**Evidence.** Eight+ `note` fields embed `E:/RustProject/dss-rs/crates/dss-core/../../…`
(a different machine — `E:` drive, `RustProject`, not this `D:/Rust/dss-rs`), e.g.
`skipped_oracle_issue.json:7,12,17,22,27,32,37,42,55,60`. These are verbatim
oracle-error captures; the useful content (the DSS error, e.g. `#130 Unknown
parameter "Num_SubCircuits"`, `TypeError: cannot unpack…`) is intact — only the
absolute path prefix is stale noise. `population.lock` fingerprints these
manifests by **count only** (not note text), so trimming the prefix would not trip
any gate.
**Disposition: NO-ACTION-BECAUSE** frozen diagnostic provenance; low value and
would churn corpus data across many entries. Flag for the orchestrator if a
cosmetic cleanup pass is wanted (safe: relativize the `E:/RustProject/...` prefix
in note strings only). NOTE: the `C:\Users\prdu001\…` and `C:\Users\User\Desktop\TCC\…`
paths in `missing_dependency.json` / `skipped_oracle_issue.json` are **upstream
deck-author** paths quoted as the *cause* — those are correct and must stay.

### N2 — Several `known_diffs.json` entries are self-documented as likely zero-hit on their retained rev
**Evidence.** The opt-in EPRI channel (`corpus_live_opendss`, gated behind
`DSS_LIVE_OPENDSS`, never in the mandatory gate) only **warns** on zero-hit
entries (corpus_live.rs:2506-2511). Two entries carry explicit notes that they are
*probably already dead on their retained rev*:
- `monitor-header-whitespace` (revs `[r3723]`) — note: harness header
  normalization is rev-INDEPENDENT, "most likely already dead on r3723 too; a
  future r3723 re-sweep … will confirm and prune it."
- `meter-zonepce-count` (revs `[r3723]`) — narrowed to r3723; the six witness
  decks now match r4088+r4133; retained only on r3723's original provenance,
  "not re-swept this WP."
These are honestly retained pending an r3723 re-sweep and cost only a zero-hit
prune warning — not a coverage hole in the gate (the catalog is never consulted by
the mandatory gate).
**Disposition: ORCHESTRATOR-ACTION (optional)** — an r3723 re-sweep
(`DSS_LIVE_OPENDSS=r3723 DSS_LIVE_OPENDSS_ASSERT=1 cargo test -p dss-core --test
corpus_live corpus_live_opendss`) would confirm-and-prune both. Not fixable
here without running the opt-in EPRI channel (Oddie venv + r3723 DLL), and
pruning a still-live-on-r3723 entry blind would be unsafe. No mandatory-gate impact.

---

## Verified sound (no finding)

- **Population lock rigor fingerprint** (`population_lock.rs:109-128`) pins exactly
  the compare-depth flags TESTING claims (kind/steps/sel/mm/probes/vars/evlog/
  ctrlq/props/gresult/aalog/pending/abort/oracle) — a retained deck cannot be
  weakened in place (kind→looser band, meters/probes cut) without a reviewable lock
  diff. The runtime props override (corpus_live.rs:1277) is deterministic from
  kind+oracle (both fingerprinted), so it opens no hole.
- **Manifest bijection** (`corpus_manifest.rs`) enforces every `.dss` in exactly
  one manifest, disk↔manifest both directions, `ad_sweep.json` correctly excluded
  as an overlay. **No silent omission possible.**
- **Family floors** — `ASYMMETRIC_REQUIRED`/`CONTROLS_REQUIRED`/`MODES_REQUIRED`
  pin per-family deck lists; `check_asymmetric_case` forces `selected_elements`
  (the transposed-stamp YPrim catch); `solvable_now_has_multistep_depth` pins ≥1
  multi-step+meters+monitor and ≥1 YPrim case. Pending cases must name a `wp`;
  every family case must carry a valid `ad` disposition.
- **Comparators compare what they claim** — `compare_element` covers currents +
  voltage-scaled powers + losses (with a documented capi015 stale-losses self-gate
  that skips ONLY the redundant losses channel when the oracle's own
  losses≠Σpowers); `compare_all_properties` order-checks names + value-compares
  with proof-backed `SKIP_PROPS`/`TRANSFORMER_CURSOR_PROPS`/`PROPS_015X`;
  `compare_eventlog`/`compare_ctrlqueue`/`compare_monitor`/`compare_meter`/
  `compare_discrete`/`compare_variables` all length-check then field-check.
- **Export comparators** — `compare_export` gates near-zero cancellation cells by
  band-limited denominator (`GateSpec`, `0<|v|<thresh` only, exact-zero rows stay
  checked); `RustSubsetByKey` carries a `require` presence guard so a dropped class
  cannot pass silently. `assert_value_matches_tol` BOM strip is leading-only (self-
  tested), identity-vs-distinct-inf-token behavior self-tested.
- **Tolerance tiers** — every banded tier (`large_floating_delta`,
  `large_floating_zeroseq`, `large_ultra_switch`, `large_near_ideal_source`,
  `micro_wtg3_dynamics` after F4) now has a TOLERANCE_NOTES proof-by-decomposition
  entry; `SecondaryTestCircuit_modified` is documented as an un-banded floor in
  `skipped_needs_investigation.json` rather than tolerance-widened.
- **PIN drift** — `tools/golden/PIN.txt` (0.15.7 / dss_capi 0.14.5) is the pinned
  default oracle; `PIN_OPENDSS.txt` pins the Oddie/EPRI channel. The mandatory gate
  fails-not-skips without the pinned oracle installed (asserted by design in
  corpus_live). (Note: the PIN files are procedural pins consumed by the Python
  oracle scripts, not asserted by a Rust test — same as upstream.)
