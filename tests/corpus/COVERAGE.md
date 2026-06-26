# Corpus coverage

_Generated 2026-06-26 14:49:06 UTC by
`tools/corpus/coverage_report.py`. Do not edit by hand._

Live oracle-comparison coverage of the vendored `electricdss-tst` corpus
(`crates/dss-core/tests/corpus_live.rs`). Every `.dss` is accounted for in exactly
one manifest (enforced by `corpus_manifest.rs`).

| manifest | `.dss` files |
|---|---|
| `missing_dependency` | 10 |
| `not_an_entry_point` | 580 |
| `skipped_needs_investigation` | 12 |
| `skipped_oracle_issue` | 31 |
| `skipped_unsupported` | 199 |
| `solvable_now` | 83 |
| **total** | **915** |

- **Runnable entry points:** 335 (everything except `not_an_entry_point`).
- **Live-compared now (`solvable_now`):** 83 — **24.8%** of entry points.
- **Remaining queue:** `skipped_needs_investigation` drains as the port grows;
  `skipped_unsupported` drains as classes/commands/modes land;
  `skipped_oracle_issue` is the documented oracle residue.

Goal: `solvable_now` grows until it covers 100% of the runnable entry points
(i.e. `solvable_now ∪ not_an_entry_point` = the whole corpus).
