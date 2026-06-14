# Vendored `electricdss-tst` corpus

Verbatim copy of the OpenDSS test corpus, used by the live oracle-comparison gate
(`crates/dss-core/tests/corpus_live.rs`) and as the in-repo replacement for the
temporary `.inputs/electricdss-tst`. See `CORPUS_TEST_PLAN.md`.

**Do not edit these files by hand.** Re-vendor with `python tools/corpus/vendor.py
--force` and review the `SHA256SUMS` diff.

**Note:** running the live gate (`DSS_LIVE_ORACLE=1`) or the classifier
(`DSS_LIVE_CLASSIFY=1`) executes cases *in place*, so OpenDSS writes outputs
(EnergyMeter `DI_*.csv`, exports, `LineConstantsCode.dss`, …) into this tree.
Re-run `python tools/corpus/vendor.py --force` (it copies the source and prunes
any extra files) to restore the pristine mirror before committing or before
running the always-on `corpus_manifest` gate.

| field | value |
|---|---|
| source | `.inputs/electricdss-tst` |
| source commit | `3b208397160213cae4a9e2d0a7d1aa3528ce26e1` |
| vendored (UTC) | 2026-06-14 12:27:55 |
| files copied | 1544 |
| of which `.dss` | 915 |
| total size | 122.0 MiB |
| excluded | `.git/`, gitignore `['*.pickle.*']` (0 files) |

Relative paths are preserved exactly, so masters resolve their `redirect`/
`compile` data dependencies internally with no path rewrites. `SHA256SUMS` lists
every copied file (paths relative to `tests/corpus/`); verify with
`sha256sum -c SHA256SUMS` from this directory.

Every `.dss` file here is accounted for in exactly one manifest under
`tests/corpus/manifests/` — enforced by `corpus_manifest.rs`.
