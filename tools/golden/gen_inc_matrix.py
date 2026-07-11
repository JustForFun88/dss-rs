"""Generate the incidence-matrix report goldens from the pinned oracle (WP-AD.1,
DIAKOPTICS_PSTCALC_PLAN §2).

Pins the CSV files the oracle writes for the `CalcIncMatrix`/`CalcIncMatrix_O`/
`CalcLaplacian` commands + exports 53–57 (`IncMatrix`/`IncMatrixRows`/
`IncMatrixCols`/`BusLevels`/`Laplacian`). Each golden is a self-contained replay:
the meta records the master (or inline deck) + the exact command sequence, so the
Rust driver (`crates/dss-core/tests/inc_matrix_reports.rs`) can never drift from
the oracle fixture. The exports are integer/text CSV, compared **byte-exact**
(line-for-line, CRLF→LF normalized) — there are no floats to tokenize.

Four fixtures cover all four flat element walks + the hierarchical build:
  * ieee13   — Lines + Transformers (corpus IEEE 13-bus).
  * ieee123  — scale (corpus IEEE 123-bus).
  * sercap   — a series Capacitor (+ a shunt cap that must be filtered out).
  * serreac  — a series Reactor (+ a shunt reactor that must be filtered out).

Each fixture emits the flat build (`CalcIncMatrix`), the hierarchical build
(`CalcIncMatrix_O`), and the Laplacian (`CalcLaplacian` after the flat build).

Usage:
    python tools/golden/gen_inc_matrix.py
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
OUT_DIR = REPO_ROOT / "tests" / "golden" / "inc_matrix"
CORPUS = REPO_ROOT / "tests" / "corpus" / "electricdss-tst"

# A small series-Capacitor feeder: `c1` bridges buses a→b (2-terminal = series);
# `csh` is a 1-terminal shunt cap that the series-cap walk must skip.
CAP_DECK = [
    "new circuit.sercapckt basekv=12.47 bus1=sourcebus",
    "new line.l1 phases=3 bus1=sourcebus bus2=a length=1",
    "new capacitor.c1 phases=3 bus1=a bus2=b kvar=600 kv=12.47",
    "new line.l2 phases=3 bus1=b bus2=c length=1",
    "new load.ld1 phases=3 bus1=c kv=12.47 kw=500 pf=0.95",
    "new capacitor.csh phases=3 bus1=c kvar=300 kv=12.47",
    "set voltagebases=[12.47]",
    "calcvoltagebases",
    "solve",
]
# A small series-Reactor feeder: `r1` bridges a→b (series); `rsh` is a shunt
# reactor (bus2 grounded, `.0` token) that the series-reactor walk must skip.
REAC_DECK = [
    "new circuit.serreacckt basekv=12.47 bus1=sourcebus",
    "new line.l1 phases=3 bus1=sourcebus bus2=a length=1",
    "new reactor.r1 phases=3 bus1=a bus2=b R=1 X=5",
    "new line.l2 phases=3 bus1=b bus2=c length=1",
    "new load.ld1 phases=3 bus1=c kv=12.47 kw=500 pf=0.95",
    "new reactor.rsh phases=3 bus1=c kvar=300 kv=12.47",
    "set voltagebases=[12.47]",
    "calcvoltagebases",
    "solve",
]

# (name, master-relpath | None, inline-deck | None, post-before-calc)
FIXTURES = [
    ("ieee13", "Version8/Distrib/IEEETestCases/13Bus/IEEE13Nodeckt.dss", None, ["solve"]),
    ("ieee123", "Version8/Distrib/IEEETestCases/123Bus/IEEE123Master.dss", None, ["solve"]),
    ("sercap", None, CAP_DECK, []),
    ("serreac", None, REAC_DECK, []),
]

# (report keyword, stem-suffix, the calc commands to run before exporting)
REPORTS = [
    ("IncMatrix", "flat_incmatrix", ["CalcIncMatrix"]),
    ("IncMatrixRows", "flat_rows", ["CalcIncMatrix"]),
    ("IncMatrix", "org_incmatrix", ["CalcIncMatrix_O"]),
    ("IncMatrixRows", "org_rows", ["CalcIncMatrix_O"]),
    ("IncMatrixCols", "org_cols", ["CalcIncMatrix_O"]),
    ("BusLevels", "org_levels", ["CalcIncMatrix_O"]),
    ("Laplacian", "org_laplacian", ["CalcIncMatrix", "CalcLaplacian"]),
]


def _build(d, master, deck) -> str:
    """Clear + compile-or-replay the fixture; return the CaseName."""
    d.Text.Command = "clear"
    if master is not None:
        master_abs = (CORPUS / master).resolve()
        if not master_abs.is_file():
            sys.exit(f"master not found: {master_abs}")
        d.Text.Command = f'compile "{master_abs}"'
    else:
        for c in deck:
            d.Text.Command = c
    return d.ActiveCircuit.Name


def gen_fixture(d, name, master, deck, post) -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    for report, suffix, calc in REPORTS:
        tmp = tempfile.mkdtemp(prefix="dss_gen_incm_")
        try:
            case = _build(d, master, deck)
            for c in post:
                d.Text.Command = c
            d.Text.Command = f'set datapath="{tmp.replace(chr(92), "/")}"'
            for c in calc:
                d.Text.Command = c
            d.Text.Command = f"export {report}"
            produced = Path(d.Text.Result)
            if not produced.is_file():
                sys.exit(f"{name}/{suffix}: no produced file at {produced}")
            content = produced.read_text()  # universal newlines -> LF
        finally:
            d.Text.Command = f'set datapath="{REPO_ROOT.as_posix()}"'
            shutil.rmtree(tmp, ignore_errors=True)
        stem = f"{name}_{suffix}"
        (OUT_DIR / f"{stem}.txt").write_text(content, newline="\n")
        meta = {
            "master": master,
            "deck": deck,
            "post": post,
            "calc": calc,
            "report": report,
            "fixture": case,
        }
        (OUT_DIR / f"{stem}.meta.json").write_text(
            json.dumps(meta, indent=2) + "\n", newline="\n"
        )
        print(f"wrote {stem}.txt ({len(content)} bytes, {content.count(chr(10))} lines)")


def main() -> None:
    pin = check_pin()
    print(f"oracle: dss-python {pin['dss_python']}, engine {pin['engine']}")
    import dss  # noqa: F401
    from dss import DSS as d

    d.AllowEditor = False
    for name, master, deck, post in FIXTURES:
        gen_fixture(d, name, master, deck, post)


if __name__ == "__main__":
    main()
