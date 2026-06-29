"""Generate the Phase-8 targeted report goldens from the pinned oracle.

Phase 8 is the reporting/output layer (Export/Show/Save/Dump). Unlike the
command-replay goldens (phase5/6/7), these pin the **report file the oracle
writes**: `gen_phase8.py` runs a small fixture on the pinned engine, issues the
`Export`/`Show`/... command, and captures the produced file's exact bytes into
`tests/golden/phase8/<report>.txt`. The Rust side (`golden_phase8.rs`) replays
the same fixture, writes its own report, and diffs the two **after parsing
numbers out** via the `compare_export` harness (PHASE8_PLAN §2.3) — never a raw
float-string diff.

WP8.1 self-test: `Export Counts` (Pascal `ExportCounts`) — a text dump of every
DSS class and its instance count. The Rust class registry is a *proper subset*
of the oracle's (only a subset of classes is ported), so the Rust file is
compared as a `RustSubsetByKey` subset of the oracle file: every ported class's
count is pinned against the oracle, the classes we do not yet register are
ignored (documented in `tests/TOLERANCE_NOTES.md`).

Usage:
    python tools/golden/gen_phase8.py            # regenerate all phase-8 goldens
Regeneration is manual and must use the exact versions in tools/golden/PIN.txt.
"""

from __future__ import annotations

import json
import shutil
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from gen_checkpoints import check_pin  # noqa: E402

REPO_ROOT = Path(__file__).resolve().parents[2]
OUT_DIR = REPO_ROOT / "tests" / "golden" / "phase8"

# The Counts fixture: a tiny circuit exercising a few class counts (Line=2,
# Load=1, Vsource=1) on top of the default DSS items. Counts depend only on
# *instance counts*, not on bus names or a solve. The Rust golden test
# (`golden_phase8.rs`) reads this same deck back from the meta file, so the two
# can never drift.
COUNTS_FIXTURE = "counts8"
COUNTS_DECK = [
    f"new circuit.{COUNTS_FIXTURE} basekv=12.47 bus1=src",
    "new line.l1 bus1=src bus2=b",
    "new line.l2 bus1=b bus2=c",
    "new load.ld1 bus1=c kv=12.47 kw=100",
]


def gen_counts(d) -> None:
    """Capture the oracle's `Export Counts` for the fixture."""
    tmp = tempfile.mkdtemp(prefix="dss_gen_phase8_")
    try:
        d.Text.Command = "clear"
        for c in COUNTS_DECK:
            d.Text.Command = c
        d.Text.Command = f'set datapath="{tmp.replace(chr(92), "/")}"'
        d.Text.Command = "export counts"
        produced = Path(tmp) / f"{COUNTS_FIXTURE}_EXP_Counts.csv"
        content = produced.read_text()  # universal newlines -> LF
    finally:
        # `set datapath` moved the engine's cwd into tmp; move it out before
        # removing the dir, else Windows refuses to delete the locked cwd.
        d.Text.Command = f'set datapath="{REPO_ROOT.as_posix()}"'
        shutil.rmtree(tmp, ignore_errors=True)
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    (OUT_DIR / "export_counts.txt").write_text(content, newline="\n")
    meta = {"report": "Counts", "fixture": COUNTS_FIXTURE, "deck": COUNTS_DECK}
    (OUT_DIR / "export_counts.meta.json").write_text(
        json.dumps(meta, indent=2) + "\n", newline="\n"
    )
    print(f"wrote export_counts.txt ({len(content)} bytes), {content.count(chr(10))} lines")


def main() -> None:
    pin = check_pin()
    print(f"oracle: dss-python {pin['dss_python']}, engine {pin['engine']}")
    import dss  # noqa: F401
    from dss import DSS as d

    gen_counts(d)


if __name__ == "__main__":
    main()
