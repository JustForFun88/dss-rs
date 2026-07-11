# tools/corpus — corpus vendoring + classification tooling

Machinery for the vendored `tests/corpus/electricdss-tst` mirror and its
manifests (the "no silent omission" bijection enforced by `corpus_manifest.rs`).

- **`vendor.py`** — copy `.inputs/electricdss-tst` → `tests/corpus/electricdss-tst`
  and prune extras (`--force`); writes `SHA256SUMS`. Run to restore the pristine
  mirror after a live-gate run dirties it.
- **`seed_manifests.py`**, **`apply_classify.py`** — build / update the
  classification manifests (`solvable_now` vs `skipped_*`); `apply_classify.py`
  turns a `DSS_LIVE_CLASSIFY=1` report into manifest moves.
- **`coverage_report.py`** — corpus coverage summary (`tests/corpus/COVERAGE.md`).
- **`gen_gaps_binshapes.py`** moved to `tools/decks/gen_shape_fixtures.py`;
  `dsspy_crosscheck.py` moved to `tools/opendss/` (EPRI-channel concern).

See `CORPUS_TEST_PLAN.md` and `TESTING.md`.
