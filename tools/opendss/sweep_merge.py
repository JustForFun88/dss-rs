#!/usr/bin/env python
"""WP-U0.2 helper: splice the isolated-modes run onto the shared corpus run.

The headline per-pair numbers in `docs/upgrade/sweeps/*.md` come from a MERGED
report: the shared 3-manifest corpus run (solvable_now + asymmetric + controls,
338 cases, one `ab_compare` process) with the `modes` family (40 cases) taken
from the one-case-per-process run of `sweep_modes_isolated.py` instead of the
poisoned shared-engine modes result. This tool builds that merge:

    merged.cases = [corpus cases whose path is NOT in modes/manifest.json]
                 + [every case from modes_<tag>.json]

Verified on all three pairs: the corpus non-modes (338) + isolated modes (40)
partition the 378-case universe exactly, 0 orphans (see the audit re-check).

Usage:
    python tools/opendss/sweep_merge.py \
        --corpus <scratch>/sweeps/corpus_<tag>.json \
        --modes  <scratch>/sweeps/modes_<tag>.json \
        --out    <scratch>/sweeps/merged_<tag>.json

`--corpus` may be a 338-case corpus-only run OR a full 378-case run that also
swept modes in the shared process; either way its modes rows are dropped and
replaced by `--modes`.
"""
import argparse
import json
from collections import Counter
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
MODES_MANIFEST = REPO_ROOT / "tests" / "corpus" / "modes" / "manifest.json"


def _norm(p: str) -> str:
    return p.replace("\\", "/").lower()


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--corpus", required=True, type=Path)
    ap.add_argument("--modes", required=True, type=Path)
    ap.add_argument("--out", required=True, type=Path)
    args = ap.parse_args()

    modes_paths = {
        _norm(c["path"] if isinstance(c, dict) else c)
        for c in json.loads(MODES_MANIFEST.read_text())["cases"]
    }
    corpus = json.loads(args.corpus.read_text())
    modes = json.loads(args.modes.read_text())

    non_modes = [c for c in corpus["cases"] if _norm(c["path"]) not in modes_paths]
    merged = dict(corpus)
    merged["cases"] = non_modes + modes["cases"]
    merged["counts"] = dict(Counter(c["status"] for c in merged["cases"]))
    args.out.write_text(json.dumps(merged, indent=1))
    print(f"merged {len(non_modes)} corpus non-modes + {len(modes['cases'])} isolated modes "
          f"= {len(merged['cases'])} cases -> {args.out}")
    print(f"counts={merged['counts']}")


if __name__ == "__main__":
    main()
