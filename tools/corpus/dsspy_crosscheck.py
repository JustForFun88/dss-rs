#!/usr/bin/env python3
"""Cross-check DSS-Python's validation corpus against our classifier manifests.

DSS-Python (vendored checkout `.inputs/DSS-Python`, branch `fastdss`) runs a
hand-curated list of ~206 electricdss-tst cases through its engine-vs-engine
validation harness (`tests/_settings.py::test_filenames`).  Every case they
validate is a case the upstream maintainers consider load-bearing — so any of
them that WE classify as skipped/unsupported is a promotion candidate for the
mandatory corpus gate.

The case list is extracted from `_settings.py` **textually** (regex over the
triple-quoted block).  Never import that module: importing it binds a DSS
engine (`from dss import DSS`) and would drag the whole dss-python runtime
into this repo-hygiene script.

`L!`-prefixed cases are run line-by-line upstream with interactive commands
(show/plot/export/dump/...) filtered out — meaningfully different from our
gate's compile+post model.  The flag is recorded per case so promotion work
does not naively `Compile` an interactive deck.

Outputs (re-runnable snapshots, `tmp/` per the classify_report convention):
  tmp/dsspy_crosscheck.json  - per-case rows + summary counts
  tmp/dsspy_crosscheck.md    - human report grouped by bucket, promotion
                               candidates sorted by tag, reverse view

Usage:  python tools/corpus/dsspy_crosscheck.py
"""

from __future__ import annotations

import json
import re
import sys
from collections import Counter, defaultdict
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
SETTINGS = REPO / ".inputs" / "DSS-Python" / "tests" / "_settings.py"
CORPUS = REPO / "tests" / "corpus" / "electricdss-tst"
MANIFESTS = REPO / "tests" / "corpus" / "manifests"
OUT_DIR = REPO / "tmp"

# Which planned work package unblocks a given skip-tag fragment.  Purely
# informational for the report; keyed by substring of the manifest `tag`.
TAG_TO_WP = [
    ("unsupported_command=BatchEdit", "WP8.6 (BatchEdit)"),
    ("unsupported_command=Distribute", "WP8.6 (Distribute)"),
    ("unsupported_command=Interpolate", "WP8.6 (Interpolate)"),
    ("unsupported_command=MakeBusList", "WP8.6 (MakeBusList)"),
    ("unsupported_command=Reduce", "WP8.7 (ReduceAlgs)"),
    ("unsupported_command=Remove", "WP8.7 (Remove)"),
    ("unsupported_command=Save", "WP8.5 (Save circuit)"),
    ("unsupported_command=Dump", "WP8.5 (Dump)"),
    ("unsupported_command=Export", "WP8.2/8.3 (Export; tag may predate)"),
    ("unsupported_command=Show", "WP8.4 (Show; tag may predate)"),
]


def extract_case_list(settings_py: Path) -> tuple[list[str], list[str]]:
    """Textually pull `test_filenames` / `cimxml_test_filenames` blocks."""
    text = settings_py.read_text(encoding="utf-8")
    lists = {}
    for name in ("test_filenames", "cimxml_test_filenames"):
        m = re.search(rf"^{name}\s*=\s*'''(.*?)'''", text, re.S | re.M)
        if not m:
            sys.exit(f"ERROR: could not find `{name} = '''...'''` in {settings_py}")
        lists[name] = [ln.strip() for ln in m.group(1).splitlines() if ln.strip()]
    return lists["test_filenames"], lists["cimxml_test_filenames"]


def load_manifest_index() -> dict[str, tuple[str, str, str]]:
    """lowercased '/'-normalized path -> (bucket, tag, note)."""
    index: dict[str, tuple[str, str, str]] = {}
    for mf in sorted(MANIFESTS.glob("*.json")):
        bucket = mf.stem
        data = json.loads(mf.read_text(encoding="utf-8"))
        for case in data["cases"]:
            key = case["path"].replace("\\", "/").lower()
            if key in index:
                sys.exit(f"ERROR: {key} appears in both {index[key][0]} and {bucket}")
            index[key] = (bucket, case.get("tag", ""), case.get("note", ""))
    return index


def wp_hint(tag: str) -> str:
    hits = [wp for frag, wp in TAG_TO_WP if frag.lower() in tag.lower()]
    return "; ".join(dict.fromkeys(hits))


def main() -> None:
    cases, cim_cases = extract_case_list(SETTINGS)
    index = load_manifest_index()

    rows = []
    for raw in cases:
        line_by_line = raw.startswith("L!")
        rel = raw[2:] if line_by_line else raw
        rel_norm = rel.replace("\\", "/")
        key = rel_norm.lower()
        exists = (CORPUS / rel_norm).is_file()
        bucket, tag, note = index.get(key, ("NOT_IN_ANY_MANIFEST", "", ""))
        rows.append(
            {
                "path": rel_norm,
                "line_by_line": line_by_line,
                "exists": exists,
                "bucket": bucket,
                "tag": tag,
                "note": note,
                "wp": wp_hint(tag),
            }
        )

    buckets = Counter(r["bucket"] for r in rows)
    missing = [r for r in rows if not r["exists"]]
    candidates = [
        r
        for r in rows
        if r["bucket"] not in ("solvable_now",) and r["exists"]
    ]

    # Reverse view: our solvable_now cases DSS-Python does not validate.
    theirs = {r["path"].lower() for r in rows}
    solvable = json.loads(
        (MANIFESTS / "solvable_now.json").read_text(encoding="utf-8")
    )["cases"]
    ours_only = sorted(
        c["path"] for c in solvable if c["path"].replace("\\", "/").lower() not in theirs
    )

    summary = {
        "dsspy_total": len(cases),
        "dsspy_cim_only": len(cim_cases),
        "missing_on_disk": len(missing),
        "buckets": dict(sorted(buckets.items(), key=lambda kv: -kv[1])),
        "promotion_candidates": len(candidates),
        "ours_solvable_not_in_dsspy": len(ours_only),
    }

    OUT_DIR.mkdir(exist_ok=True)
    (OUT_DIR / "dsspy_crosscheck.json").write_text(
        json.dumps(
            {"summary": summary, "cases": rows, "cim_cases": cim_cases,
             "ours_solvable_not_in_dsspy": ours_only},
            indent=1,
        )
        + "\n",
        encoding="utf-8",
    )

    md = ["# DSS-Python corpus cross-check", ""]
    md.append(f"DSS-Python validates **{len(cases)}** cases "
              f"(+{len(cim_cases)} CIM-XML-only). Missing on disk: "
              f"**{len(missing)}**. Bucket split:")
    md.append("")
    md.append("| bucket | cases |")
    md.append("|---|---|")
    for b, n in summary["buckets"].items():
        md.append(f"| {b} | {n} |")
    md.append("")

    if missing:
        md.append("## MISSING ON DISK (vendoring gap!)")
        md += [f"- `{r['path']}`" for r in missing]
        md.append("")

    md.append(f"## Promotion candidates ({len(candidates)}) — they validate it, we skip it")
    md.append("")
    by_bucket: dict[str, list[dict]] = defaultdict(list)
    for r in candidates:
        by_bucket[r["bucket"]].append(r)
    for bucket in sorted(by_bucket):
        group = by_bucket[bucket]
        md.append(f"### {bucket} ({len(group)})")
        md.append("")
        md.append("| case | L! | tag | unblocked by |")
        md.append("|---|---|---|---|")
        for r in sorted(group, key=lambda r: (r["tag"], r["path"])):
            lb = "L!" if r["line_by_line"] else ""
            md.append(f"| `{r['path']}` | {lb} | {r['tag']} | {r['wp']} |")
        md.append("")

    md.append(f"## Reverse view ({len(ours_only)}) — in our solvable_now, not in their list (informational)")
    md.append("")
    md += [f"- `{p}`" for p in ours_only]
    md.append("")

    (OUT_DIR / "dsspy_crosscheck.md").write_text("\n".join(md), encoding="utf-8")

    print(f"dsspy cases: {len(cases)} (+{len(cim_cases)} CIM), "
          f"missing on disk: {len(missing)}")
    for b, n in summary["buckets"].items():
        print(f"  {b:35} {n}")
    print(f"promotion candidates: {len(candidates)}")
    print(f"reports: {OUT_DIR / 'dsspy_crosscheck.json'}, "
          f"{OUT_DIR / 'dsspy_crosscheck.md'}")
    if missing:
        sys.exit(1)


if __name__ == "__main__":
    main()
