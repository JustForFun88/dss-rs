"""Corpus coverage report (CORPUS_TEST_PLAN.md §6).

Summarizes the manifests under `tests/corpus/manifests/` and writes
`tests/corpus/COVERAGE.md`: how many `.dss` are in each bucket, and what fraction
of the *runnable entry points* are currently live-compared (`solvable_now`). This
is the burn-down toward 100% as the port matures.

    python tools/corpus/coverage_report.py
"""

from __future__ import annotations

import json
from datetime import datetime, timezone
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
MANIFESTS = REPO_ROOT / "tests" / "corpus" / "manifests"
OUT = REPO_ROOT / "tests" / "corpus" / "COVERAGE.md"

# Buckets that represent runnable entry points (the denominator for "solvable").
ENTRY_BUCKETS = [
    "solvable_now",
    "skipped_unsupported",
    "skipped_oracle_issue",
    "skipped_needs_investigation",
    "missing_dependency",
]
NON_ENTRY = "not_an_entry_point"


def count(name: str) -> int:
    p = MANIFESTS / f"{name}.json"
    if not p.is_file():
        return 0
    return len(json.loads(p.read_text()).get("cases", []))


def main() -> None:
    counts = {p.stem: count(p.stem) for p in sorted(MANIFESTS.glob("*.json"))}
    total = sum(counts.values())
    entry = sum(counts.get(b, 0) for b in ENTRY_BUCKETS)
    solvable = counts.get("solvable_now", 0)
    pct = (100.0 * solvable / entry) if entry else 0.0

    rows = "\n".join(f"| `{name}` | {n} |" for name, n in sorted(counts.items()))
    md = f"""# Corpus coverage

_Generated {datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M:%S")} UTC by
`tools/corpus/coverage_report.py`. Do not edit by hand._

Live oracle-comparison coverage of the vendored `electricdss-tst` corpus
(`crates/dss-core/tests/corpus_live.rs`). Every `.dss` is accounted for in exactly
one manifest (enforced by `corpus_manifest.rs`).

| manifest | `.dss` files |
|---|---|
{rows}
| **total** | **{total}** |

- **Runnable entry points:** {entry} (everything except `{NON_ENTRY}`).
- **Live-compared now (`solvable_now`):** {solvable} — **{pct:.1f}%** of entry points.
- **Remaining queue:** `skipped_needs_investigation` drains as the port grows;
  `skipped_unsupported` drains as classes/commands/modes land;
  `skipped_oracle_issue` is the documented oracle residue.

Goal: `solvable_now` grows until it covers 100% of the runnable entry points
(i.e. `solvable_now ∪ {NON_ENTRY}` = the whole corpus).
"""
    OUT.write_text(md, encoding="utf-8")
    print(f"wrote {OUT.relative_to(REPO_ROOT)}")
    print(f"  total .dss: {total}; entry points: {entry}; solvable_now: {solvable} ({pct:.1f}%)")


if __name__ == "__main__":
    main()
