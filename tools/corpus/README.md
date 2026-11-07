# tools/corpus — corpus fetching + classification tooling

Machinery for the OpenDSS test suite in `tests/corpus/electricdss-tst` and its
manifests (the "no silent omission" bijection enforced by `corpus_manifest.rs`).

- **`vendor.py`** — fetch the commit `tests/corpus/PIN.json` pins from GitHub
  into the gitignored `tests/corpus/electricdss-tst`, checked against
  `tests/corpus/SHA256SUMS`. Run it once per checkout or worktree (the tarball
  is cached in `DSS_CORPUS_CACHE`, default `~/.cache/dss-rs`), and again after a
  manual deck run: it lists and undoes what the run wrote. To move the pin, edit
  `PIN.json`, run `python tools/corpus/vendor.py --rehash` and review the
  `SHA256SUMS` diff.
- **`seed_manifests.py`**, **`apply_classify.py`** — build / update the
  classification manifests (`solvable_now` vs `skipped_*`); `apply_classify.py`
  turns a `DSS_LIVE_CLASSIFY=1` report into manifest moves.
- **`coverage_report.py`** — corpus coverage summary (`tests/corpus/COVERAGE.md`).

## Procedures

Every cargo command below goes through the build queue
(`python tools/gate/gatelock.py --label "<who>" -- cargo …`, `tools/gate/README.md`).
The corpus gate's knobs are listed in the module doc of
`crates/dss-core/tests/corpus_gate.rs`.

**Add a corpus case.** Pick the family by content (static → `asymmetric`,
control/time-series → `controls`, mode/algorithm/format/verb → `modes`; vendored
decks are classified in `manifests/solvable_now.json`). Add the `.dss` (a
subfolder if it needs fixtures) and a manifest entry with its
`kind`/`n_steps`/`probes`. `engines` defaults to `"both"`: narrow it to one
channel only when the other channel **cannot** gate the case (the feature is
absent from dss_capi 0.14.5, or the divergence cannot be fingerprinted) and say
why in `note`. Set `pending: true` + `wp` if the feature is not implemented,
`isolate: true` + `note` if the deck holds files open or corrupts a reused
engine process. Validate on the pinned dss-python first (two-process
determinism, and the case must be sensitive to the feature it covers), then run
the seeding report (below) to measure the r4133 side. The
`*_manifest_is_complete` tests enforce the dir↔manifest bijection. Regenerate
the population lock in the same change:
`DSS_UPDATE_POPULATION_LOCK=1 cargo test -p dss-core --test population_lock`.

**Run the seeding report.** `DSS_GATE_SEED_LEDGER=1` (optionally
`DSS_GATE_SEED_ONLY=<substr>`) `cargo test -p dss-core --test corpus_gate`
runs every case on BOTH channels with no ledger, asserts nothing, and writes
candidate entries with measured locations and envelopes to
`tmp/ledger_candidates.json`. Triage the candidates by hand into real entries
(rules below). Never commit the candidate file as-is.

**Triage a divergence into the ledger** — only for a **measured OpenDSS
divergence**, never a shortcut past a red gate:

1. Prove first that the gap is not a bug of this engine: tighten the loop
   tolerance, isolate the suspect, diff the per-iteration trajectories, read
   the live f64 state on both engines. An entry that papers over a fixable bug
   is the worst outcome. If this engine is wrong, fix it.
2. Measure it: `DSS_GATE_SEED_LEDGER=1 DSS_GATE_SEED_ONLY=<case>` for the raw
   both-channel report, and/or `DSS_LEDGER_MEASURE=1 DSS_GATE_ONLY=<case>` to
   size an envelope to the observed maximum.
3. Write the entry in `ledger.json`: a unique `id`, the exact `case` key,
   `channel`, `kind`, `match` scopes (implemented fields only), the envelope
   (`max_rel`/`max_abs`) or `num_rel` or exact `rust` + `oracle` pins, and a
   `cause` (or a `cause_ref` into `causes`) + `source` + `measured`
   provenance. Pair the entry with an expected-value test that pins our value.
   An entry without one is a mask.
4. Run the full gate: the structural test must pass, the entry must be HIT,
   and a divergence entry must still exceed the tier floor. A too-wide or
   obsolete entry fails as STALE.
5. Regenerate the population lock (the ledger entries' digests are part of
   each case's rigor fingerprint) and commit ledger and lock together.

**Triage a property divergence on the r4133 channel.** The four links are
tried in this order, and only after the value of this engine is shown right:

0. Locate the cell. The failure message carries `class.prop` and both raw
   spellings. Measure the whole pair rather than the one case:
   `DSS_PROPS_CENSUS=claims cargo test -p dss-core --test corpus_gate corpus_gate_props_census -- --nocapture`
   writes `tmp/props_census/<channel>/claims{,_unclaimed_pairs,_summary}`.
   Read the per-cell `disposition` in `tmp/props_census.json`.
1. Normalize (`harness::props_norm`, `PROPS_NORM_R4133`): only if both sides
   spell the SAME value differently, through a typed rule whose predicate
   proves it. Never a regex, never a "close enough".
2. Echo-exclude (`PROPS_ECHO_R4133`, same module): only if r4133 renders an
   echo or different live state instead of the value. The row needs a witness:
   capi coverage, or a named expected-value pin in
   `crates/dss-core/tests/props_r4133_pins.rs`.
3. Display floor (`R4133_DISPLAY_FLOOR`, same module): a cell predicate, never
   widened to make a cell fit.
4. Ledger: a `property`-scoped entry per case and channel, by the procedure
   above, for a genuine OpenDSS divergence.

A cell that reaches none of the four is a bug: in this engine (fix it) or in
r4133 (fix this engine if it follows r4133, ledger the divergence and write the
OpenDSS report).

**A red that is environmental.** Wait until no other run's `epri-worker`,
`python` or `corpus_gate` process is alive, then re-run the failing case alone
(`DSS_GATE_ONLY=<label>`) and, if it passes, the full gate. An
`oracle timeout` on a heavy `large`/`ckt24` deck is load-sensitive, and a
base-frequency-shaped red is the registry leak `tools/opendss/README.md`
describes. Neither is triaged into the ledger. A file-in-use error
(`#303`/`#711`) inside the gate is a finding, not noise: every (case,
producer) run compiles its own scratch copy, so measure the holder first.
