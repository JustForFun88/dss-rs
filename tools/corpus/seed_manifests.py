"""Bootstrap the corpus manifests (CORPUS_TEST_PLAN.md §2).

Scans the vendored corpus and writes one JSON manifest per status under
`tests/corpus/manifests/`, so that **every** `.dss` is accounted for in exactly
one manifest from day one (the bijection enforced by `corpus_manifest.rs`).

Seeding rules (a conservative first cut; the classifier + human curation refine
these over time):
  - a file that creates a circuit (`new circuit ...`), or that pulls one in via
    `redirect`/`compile` and is **not itself** redirected/compiled by any other
    file, is an *entry-point candidate* -> `skipped_needs_investigation`;
  - everything else is `not_an_entry_point` (an include fragment, or an assembler
    that is itself included by a master).

`solvable_now` and the other skip buckets start empty; the classifier
(`tools/corpus/classify.py`) and curation promote entries into them.

This is a one-time bootstrap. After it runs the manifests are hand-curated; do
not blindly re-run it (it would overwrite curation). Re-run only to re-seed from
scratch, and review the diff.

    python tools/corpus/seed_manifests.py            # refuses if manifests exist
    python tools/corpus/seed_manifests.py --force     # overwrite
"""

from __future__ import annotations

import json
import os
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
DST = REPO_ROOT / "tests" / "corpus" / "electricdss-tst"
MANIFESTS = REPO_ROOT / "tests" / "corpus" / "manifests"

NEW_CIRCUIT = re.compile(r"\bnew\s+circuit", re.IGNORECASE)
REDIR = re.compile(r"(?im)^\s*(?:redirect|compile)\s+(.+?)\s*$")


def read_text(p: Path) -> str:
    return p.read_text(encoding="utf-8", errors="ignore")


def redirect_targets(f: Path, text: str) -> list[Path]:
    """Resolve a file's redirect/compile targets to paths under the corpus."""
    out = []
    for m in REDIR.finditer(text):
        raw = m.group(1).strip().strip('"').strip("'").replace("\\", "/")
        if not raw:
            continue
        cand = (f.parent / raw)
        for c in (cand, cand.with_name(cand.name + ".dss")):
            try:
                rc = Path(os.path.normpath(c))
            except Exception:  # noqa: BLE001
                continue
            if rc.is_file():
                out.append(rc)
                break
    return out


def main() -> None:
    force = "--force" in sys.argv[1:]
    if not DST.is_dir():
        sys.exit(f"vendored corpus not found: {DST}\n  run tools/corpus/vendor.py first")
    MANIFESTS.mkdir(parents=True, exist_ok=True)
    existing = list(MANIFESTS.glob("*.json"))
    if existing and not force:
        sys.exit(f"manifests already exist in {MANIFESTS} ({len(existing)} files); use --force")

    dss_files = sorted(DST.rglob("*.dss"))
    texts = {f: read_text(f) for f in dss_files}

    # Mark every file that is redirected/compiled by some other file.
    referenced: set[Path] = set()
    for f, text in texts.items():
        for t in redirect_targets(f, text):
            referenced.add(t)

    needs_investigation = []
    not_entry = []
    for f in dss_files:
        rel = f.relative_to(DST).as_posix()
        text = texts[f]
        has_circuit = bool(NEW_CIRCUIT.search(text))
        has_redir = bool(REDIR.search(text))
        is_referenced = f in referenced
        if has_circuit or (has_redir and not is_referenced):
            needs_investigation.append(
                {
                    "path": rel,
                    "tag": "entry_point" if has_circuit else "wrapper",
                    "note": "auto-seeded entry-point candidate; verify on both engines",
                }
            )
        else:
            not_entry.append(
                {
                    "path": rel,
                    "tag": "include_assembler" if has_redir else "include_fragment",
                    "note": "auto-seeded; redirected/compiled by a master or a pure fragment",
                }
            )

    def write(name: str, cases: list, comment: str) -> None:
        path = MANIFESTS / f"{name}.json"
        path.write_text(
            json.dumps({"comment": comment, "cases": cases}, indent=1) + "\n"
        )
        print(f"wrote {path.relative_to(REPO_ROOT)} ({len(cases)} cases)")

    write(
        "solvable_now",
        [],
        "Entry points both engines solve; live-compared by corpus_live.rs. "
        "Fields: path, kind(micro|feeder|large), post[], n_steps, selected_elements[].",
    )
    write(
        "skipped_unsupported",
        [],
        "Entry points using a class/command/mode the port lacks. "
        "Required tag: unsupported_class=.. | unsupported_command=.. | unsupported_mode=.. | missing_feature=..",
    )
    write(
        "skipped_oracle_issue",
        [],
        "Cases where the pinned oracle itself errors / is unreliable. tag + note (+ upstream ref).",
    )
    write(
        "missing_dependency",
        [],
        "Entry points referencing a data file absent from the copy (corpus integrity gap).",
    )
    write(
        "skipped_needs_investigation",
        needs_investigation,
        "Entry-point candidates not yet verified/diagnosed. The working queue that drains over time.",
    )
    write(
        "not_an_entry_point",
        not_entry,
        "Not run standalone: include fragments / assemblers exercised via their master. "
        "tag: include_fragment | include_assembler | helper | plot_or_export_only.",
    )

    print(
        f"\nseeded {len(dss_files)} .dss: "
        f"{len(needs_investigation)} entry candidates, {len(not_entry)} not-an-entry-point"
    )


if __name__ == "__main__":
    main()
