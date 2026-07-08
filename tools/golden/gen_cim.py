"""Generate the CIM100 XML export goldens from the pinned oracle (GAPS_PLAN
WPG.18).

Every UUID `Export CIM100` prints is either a lazy random-v4 `TNamedObject.UUID`
or a hashed `GetDevUuid` (also bottoming out in a random v4 on first creation,
`NamedObject.pas:47-52`) -- neither can ever be oracle-pinned directly. So the
gate instead preloads *every* UUID the export touches via the WP8.6 `uuids
file=<fixture>` command before exporting, on both engines, making two runs
bit-identical and the golden compare **exact bytes** (CRLF-normalized), zero
tolerance (GAPS_PLAN WPG.18 decision 1-2).

Recipe per case (`CASES` below), all on the pinned oracle:
  1. compile the deck -> solve -> `export cim100` (output discarded, but it
     lazily creates every hashed key `ExportCDPSM` visits) -> `export uuids`
     (dumps the *complete* key set) -> rewrite that into the `uuids file=`
     input format (comma-delimited) -> save as the fixture CSV.
  2. fresh compile -> solve -> `uuids file=<fixture>` (preload) -> `export
     cim100` -> save the produced bytes as the golden.
  3. repeat step 2 verbatim and assert byte-identity with step 2's output --
     the fixture-completeness proof (a missing key would draw a *different*
     random v4 on this third run and the assert would catch it loudly).

Usage:
    python tools/golden/gen_cim.py
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
DECKS_DIR = REPO_ROOT / "tools" / "golden" / "cim_decks"
OUT_DIR = REPO_ROOT / "tests" / "golden" / "cim"

# One case per Stage (A-F); each names a deck file under `cim_decks/` and the
# circuit name that deck declares (`New Circuit.<name>` -- also the
# `<name>_CIM100x.xml`/`<name>_EXP_UUIDS.csv` filename prefix, Pascal
# `CircuitName_`). Stage A: `cim_src.dss` (Vsource + buscoords only).
CASES = [
    {"deck": "cim_src.dss", "circuit": "cim_src"},
    {"deck": "cim_load.dss", "circuit": "cim_load"},
    {"deck": "cim_lines.dss", "circuit": "cim_lines"},
]


def _rewrite_uuids_to_fixture(export_uuids_text: str) -> str:
    """`export uuids` writes `<Name> {UUID}` (space-delimited, one per line,
    `ExportResults.pas` `ExportUuids`); `uuids file=` reads **comma**-delimited
    `<Name>, {UUID}` (`ExecHelper.pas` `DoUuidsCmd`, `AuxParser.Delimiters :=
    ','`). Rewrite one format into the other -- split on the last ` {`, the
    unambiguous boundary since every value is a braced GUID."""
    lines = []
    for line in export_uuids_text.splitlines():
        line = line.rstrip("\r\n")
        if not line.strip():
            continue
        idx = line.rfind(" {")
        if idx < 0:
            sys.exit(f"gen_cim: malformed export-uuids line (no ' {{'): {line!r}")
        name_part, uuid_part = line[:idx], line[idx + 1 :]
        lines.append(f"{name_part}, {uuid_part}")
    return "\n".join(lines) + "\n"


def _run_deck(d, deck_path: Path) -> None:
    d.Text.Command = "clear"
    d.Text.Command = f'compile "{deck_path.as_posix()}"'


def _produce_cim100(d, deck_path: Path, circuit: str, tmp: str, fixture_path: Path) -> str:
    """One (b)/(c)-style run: fresh compile, preload the fixture, export
    CIM100, return the produced bytes (LF-normalized)."""
    _run_deck(d, deck_path)
    d.Text.Command = f'set datapath="{tmp.replace(chr(92), "/")}"'
    d.Text.Command = f'uuids file="{fixture_path.as_posix()}"'
    d.Text.Command = "export cim100"
    matches = list(Path(tmp).glob(f"{circuit}_CIM100x.xml"))
    if len(matches) != 1:
        sys.exit(f"gen_cim: expected exactly one {circuit}_CIM100x.xml in {tmp}, found {matches}")
    return matches[0].read_text()  # universal newlines -> LF


def gen_case(d, case: dict) -> None:
    deck_path = DECKS_DIR / case["deck"]
    circuit = case["circuit"]
    assert deck_path.is_file(), f"missing deck: {deck_path}"

    # --- process 1: compute + capture the fixture ------------------------
    tmp1 = tempfile.mkdtemp(prefix="dss_gen_cim_")
    try:
        _run_deck(d, deck_path)
        d.Text.Command = f'set datapath="{tmp1.replace(chr(92), "/")}"'
        d.Text.Command = "export cim100"  # discarded; populates the hashed-key list
        d.Text.Command = "export uuids"
        uuid_matches = list(Path(tmp1).glob(f"{circuit}_EXP_UUIDS.csv"))
        if len(uuid_matches) != 1:
            sys.exit(f"gen_cim: expected exactly one {circuit}_EXP_UUIDS.csv, found {uuid_matches}")
        fixture_text = _rewrite_uuids_to_fixture(uuid_matches[0].read_text())
    finally:
        d.Text.Command = f'set datapath="{REPO_ROOT.as_posix()}"'
        shutil.rmtree(tmp1, ignore_errors=True)

    DECKS_DIR.mkdir(parents=True, exist_ok=True)
    fixture_path = DECKS_DIR / f"{circuit}_fixture.csv"
    fixture_path.write_text(fixture_text, newline="\n")

    # --- process 2: produce the golden ------------------------------------
    tmp2 = tempfile.mkdtemp(prefix="dss_gen_cim_")
    try:
        golden = _produce_cim100(d, deck_path, circuit, tmp2, fixture_path)
    finally:
        d.Text.Command = f'set datapath="{REPO_ROOT.as_posix()}"'
        shutil.rmtree(tmp2, ignore_errors=True)

    # --- process 3: fixture-completeness proof ----------------------------
    tmp3 = tempfile.mkdtemp(prefix="dss_gen_cim_")
    try:
        repeat = _produce_cim100(d, deck_path, circuit, tmp3, fixture_path)
    finally:
        d.Text.Command = f'set datapath="{REPO_ROOT.as_posix()}"'
        shutil.rmtree(tmp3, ignore_errors=True)

    if repeat != golden:
        sys.exit(
            f"gen_cim: {circuit} fixture is INCOMPLETE -- a repeat `export cim100` run "
            "diverged from the first (some UUID was not preloaded and drew a fresh "
            "random v4). Fix the fixture, never weaken the compare."
        )

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    (OUT_DIR / f"{circuit}.xml").write_text(golden, newline="\n")
    meta = {
        "report": "cim100",
        "circuit": circuit,
        "deck": case["deck"],
        "fixture": f"{circuit}_fixture.csv",
        "suffix": "CIM100x.xml",
    }
    (OUT_DIR / f"{circuit}.meta.json").write_text(json.dumps(meta, indent=2) + "\n", newline="\n")
    print(
        f"wrote {circuit}.xml ({len(golden)} bytes, {golden.count(chr(10))} lines), "
        f"fixture {circuit}_fixture.csv ({fixture_text.count(chr(10))} keys)"
    )


def main() -> None:
    pin = check_pin()
    print(f"oracle: dss-python {pin['dss_python']}, engine {pin['engine']}")
    import dss  # noqa: F401
    from dss import DSS as d

    d.AllowEditor = False
    for case in CASES:
        gen_case(d, case)


if __name__ == "__main__":
    main()
