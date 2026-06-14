"""Apply a classify report to the corpus manifests (CORPUS_TEST_PLAN.md §2/§6).

Reads `tmp/classify_report.json` (written by the `corpus_live_classify` Rust test)
and moves entries out of `skipped_needs_investigation`, preserving the bijection
(every path stays in exactly one manifest). Each moved entry records the **exact**
reason, both as a machine-readable `tag` parsed from the engine/oracle error and
as the full error text in `note`:

  - `solvable`                          -> `solvable_now` (kind=feeder, n_steps=1);
  - missing input data / redirect file  -> `missing_dependency`
                                           (tag `missing_data_file=<name>`);
  - Rust engine error                   -> `skipped_unsupported`
                                           (tag `unsupported_class=..` /
                                            `unsupported_command=..` / `..feature=..`);
  - other oracle error                  -> `skipped_oracle_issue`
                                           (tag `oracle_error_#<code>`);
  - timeout / live mismatch / non-convergence
                                        -> stays in `skipped_needs_investigation`
                                           (note = observed symptom).

Existing destination entries are preserved; new ones merge/de-dup by path.

    python tools/corpus/apply_classify.py            # dry run (prints summary)
    python tools/corpus/apply_classify.py --write      # rewrite the manifests
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
MANIFESTS = REPO_ROOT / "tests" / "corpus" / "manifests"
REPORT = REPO_ROOT / "tmp" / "classify_report.json"

# Reasons come from the Rust panic message (engine error list) or the oracle
# DSSException text; pull the specific unsupported item / file / error code out.
RE_CMD = re.compile(r'Command\s*\\?"([^"\\]+)\\?"\s*is not ported', re.I)
RE_CLASS = re.compile(r'Object Type\s*\\?"([^"\\]+)\\?"\s*not found', re.I)
RE_FEATURE = re.compile(r'\\?"([^"\\]+)\\?"\s*is not ported yet', re.I)
# Capture the rest of the clause; missing_tag() trims the escape markers and
# basenames it (the reason is Rust-Debug-escaped, so quotes are \" and path
# separators are \\, and the path may be truncated by the report's reason cap).
RE_FILE = re.compile(r"(?:opening file|file not found)\s*:?\s*(\S.*)", re.I)
RE_CODE = re.compile(r"\(#(\d+)\)")


def load(name: str) -> dict:
    return json.loads((MANIFESTS / f"{name}.json").read_text())


def save(name: str, data: dict) -> None:
    (MANIFESTS / f"{name}.json").write_text(json.dumps(data, indent=1) + "\n")


def by_path(cases: list) -> dict:
    return {c["path"]: c for c in cases}


def categorize(reason: str) -> str:
    r = reason.lower()
    if "timeout" in r:
        return "investigate"  # long simulation; revisit (reduced steps / own harness)
    if "error opening file" in r or "redirect file" in r or "(#243)" in r or "(#613)" in r:
        return "missing_dependency"
    if "rust engine errors" in r:
        return "unsupported"
    if "oracle case" in r and "failed" in r:
        return "oracle_issue"
    return "investigate"  # live mismatch / non-convergence / other


def unsupported_tag(reason: str) -> str:
    classes = sorted(set(RE_CLASS.findall(reason)))
    cmds = sorted(set(RE_CMD.findall(reason)))
    parts = []
    if classes:
        parts.append("unsupported_class=" + ",".join(classes))
    if cmds:
        parts.append("unsupported_command=" + ",".join(cmds))
    if not parts:
        if "file-backed numeric arrays" in reason:
            parts.append("unsupported_feature=file-backed-arrays")
        else:
            feats = sorted(set(RE_FEATURE.findall(reason)))
            if feats:
                parts.append("unsupported_feature=" + ",".join(feats))
    return "; ".join(parts) if parts else "unsupported_unknown"


def missing_tag(reason: str) -> str:
    # The reason is Rust-Debug-escaped (\" for quotes, \\ for path separators,
    # \r\n literal), so extract the rest of the "opening file:"/"file not found:"
    # clause, trim at the escape markers, then basename it.
    m = RE_FILE.search(reason)
    if not m:
        return "missing_data_file"
    s = m.group(1)
    for cut in ("\\r", "\\n", "[file:"):
        i = s.find(cut)
        if i != -1:
            s = s[:i]
    s = s.replace('\\"', "").replace('"', "").strip()
    s = s.replace("\\\\", "/").replace("\\", "/").rstrip("/")
    name = s.split("/")[-1].strip()
    return f"missing_data_file={name}" if name else "missing_data_file"


def oracle_tag(reason: str) -> str:
    m = RE_CODE.search(reason)
    return f"oracle_error_#{m.group(1)}" if m else "oracle_error"


def main() -> None:
    write = "--write" in sys.argv[1:]
    if not REPORT.is_file():
        sys.exit(f"no report at {REPORT}; run the DSS_LIVE_CLASSIFY=1 corpus_live_classify test first")
    report = json.loads(REPORT.read_text())
    solvable = set(report.get("solvable", []))
    failures = {f["path"]: f.get("reason", "") for f in report.get("failures", [])}

    names = [
        "skipped_needs_investigation",
        "solvable_now",
        "skipped_unsupported",
        "skipped_oracle_issue",
        "missing_dependency",
    ]
    data = {n: load(n) for n in names}
    bp = {n: by_path(data[n]["cases"]) for n in names}

    counts = {"solvable": 0, "missing_dependency": 0, "unsupported": 0, "oracle_issue": 0}
    new_needs: list = []
    for path, case in bp["skipped_needs_investigation"].items():
        if path in solvable:
            bp["solvable_now"].setdefault(path, {"path": path, "kind": "feeder", "n_steps": 1})
            counts["solvable"] += 1
            continue
        if path not in failures:
            new_needs.append(case)  # not in report (stale) — keep as-is
            continue
        reason = failures[path]
        note = reason[:300]
        cat = categorize(reason)
        if cat == "missing_dependency":
            bp["missing_dependency"].setdefault(path, {"path": path, "tag": missing_tag(reason), "note": note})
            counts["missing_dependency"] += 1
        elif cat == "unsupported":
            bp["skipped_unsupported"].setdefault(path, {"path": path, "tag": unsupported_tag(reason), "note": note})
            counts["unsupported"] += 1
        elif cat == "oracle_issue":
            bp["skipped_oracle_issue"].setdefault(path, {"path": path, "tag": oracle_tag(reason), "note": note})
            counts["oracle_issue"] += 1
        else:
            case = dict(case)
            case["note"] = f"live probe: {note}"
            new_needs.append(case)

    data["skipped_needs_investigation"]["cases"] = sorted(new_needs, key=lambda c: c["path"])
    for n in names[1:]:
        data[n]["cases"] = [bp[n][k] for k in sorted(bp[n])]

    print(f"report: {report.get('total')} candidates probed")
    print(f"  -> solvable_now          +{counts['solvable']:<4} (now {len(data['solvable_now']['cases'])})")
    print(f"  -> missing_dependency    +{counts['missing_dependency']:<4} (now {len(data['missing_dependency']['cases'])})")
    print(f"  -> skipped_unsupported   +{counts['unsupported']:<4} (now {len(data['skipped_unsupported']['cases'])})")
    print(f"  -> skipped_oracle_issue  +{counts['oracle_issue']:<4} (now {len(data['skipped_oracle_issue']['cases'])})")
    print(f"  -> needs_investigation    {len(data['skipped_needs_investigation']['cases'])} remain")

    if not write:
        print("\ndry run; re-run with --write to update the manifests")
        return
    for n in names:
        save(n, data[n])
    print("\nmanifests updated.")


if __name__ == "__main__":
    main()
