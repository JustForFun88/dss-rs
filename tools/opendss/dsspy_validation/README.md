# DSS-Python validation harness (vendored copy) — broad-surface A/B inventory

Copied from [DSS-Python](https://github.com/dss-extensions/DSS-Python)
`tests/` (branch `fastdss` = 0.16.0b2, BSD-3-Clause — `LICENSE` here), with
surgical local modifications marked `# dss-rs:` so a future re-sync stays a
three-file diff. It dumps **~40 API collections per case** (every `_columns`
field of every object, embedded `ActiveCktElement` records, Solution
inc-matrix/Laplacian, meter sections/totals, saved circuit, CIM XML, monitor
CSVs) into one `results-<engine>.zip`, then diffs two such zips offline —
complementing `ab_compare.py`, which compares the per-solve electrical state
only.

**The `capi` engine here is dss_capi 0.15.0b4 (the Oddie venv's bundled
backend), NOT the repo's pinned 0.14.5 oracle. Everything under this
directory is upstream-porting inventory only — it never feeds goldens, the
mandatory gate, or any commit decision.**

## What was changed vs upstream (`# dss-rs:` markers)

- `_settings.py`: corpus anchored to `tests/corpus/electricdss-tst` (override:
  `DSS_RS_TST_DIR`); engine spec `DSS_EXTENSIONS_TEST_ODDIE=oddie:<rev>`
  resolves the vendored EPRI DLL via `../revisions.json` and hard-verifies
  `expect_version`; upstream's unused `ZIP_FN` dropped. The 206-case list is
  verbatim upstream (all 206 exist in the vendored corpus —
  `tools/corpus/dsspy_crosscheck.py`).
- `save_outputs.py`: COM/comtypes branch dropped (machine-global regsvr32 —
  ruled out, see `../README.md`); editor suppression replaced with the proven
  `Set RegistryUpdate=No` + `Set Editor=rundll32.exe` pair (upstream's
  `true.exe` path is machine-dependent and persists into the user's OpenDSS
  registry settings); each case runs inside the shared `CorpusGuard`
  (`tools/oracle/corpus_guard.py`) with run products zipped before the guard
  deletes/restores them; results zip goes to `tmp/dsspy_validation/`.
- `compare_outputs.py`: as upstream (its `KNOWN_COM_DIFF` + per-field skips +
  loosened categories — Residuals/magang 5e-4/1e-2, `Seq*` 1e-2, complex
  rtol 1e-3, >1000-vector 0.1% leniency — encode years of dss_capi-vs-EPRI
  triage and stay untouched). Needs `pandas` + `xmldiff`
  (`PIN_OPENDSS.txt`).

## Usage (Oddie venv only; run from this directory)

```
cd tools/opendss/dsspy_validation

# capi side (dss_capi 0.15.0b4 via the venv's default backend):
../.venv/Scripts/python save_outputs.py dss-extensions

# EPRI side (vendored official DLL, one revision per process):
DSS_EXTENSIONS_TEST_ODDIE=oddie:r3723 ../.venv/Scripts/python save_outputs.py dss-extensions-odd

# diff the two zips (JSON + CSV; -v verbose, -p multiprocessing):
../.venv/Scripts/python compare_outputs.py \
    ../../../tmp/dsspy_validation/results-dssx-....zip \
    ../../../tmp/dsspy_validation/results-dssx_oddd-....zip
```

The zip is append-mode: re-running skips cases already captured (delete the
zip for a fresh sweep). A case list override is available via
`DSS_EXTENSIONS_TEST_SYSTEMS=<json>` (`{"testSystems": [...]}` replaces,
`{"extraTestSystems": [...]}` prepends).

**After every sweep:** `git status tests/corpus` MUST be clean — the guard is
non-recursive, so files created in case *sub*directories escape it. Recovery:
`git restore tests/corpus` (never `vendor.py --force` casually — EOL trap,
see STATUS §6).
