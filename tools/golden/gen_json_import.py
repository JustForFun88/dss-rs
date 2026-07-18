"""Generate the AltDSS JSON-import round-trip byte goldens from dss-python.

The import path (`Circuit_FromJSON`) is the inverse of the export
(`Circuit_ToJSON`, ported in JSON_EXPORT). For each deck we:

  1. run the deck on the pinned oracle and export the whole circuit -> J0;
  2. feed J0 back through `Circuit_FromJSON` and re-export -> J1;
  3. feed J1 back through `Circuit_FromJSON` and re-export -> J2.

The re-export order is `AltPropertyOrder` (not the original deck's set order), so
J0 != J1 in general, but the round trip is idempotent after one cycle
(J1 == J2 -- asserted here). The Rust engine (`golden_json_import.rs`) imports the
SAME J0 bytes, re-exports, and must reproduce J1 verbatim -- a byte-exact,
oracle-backed gate. Captured PRE-SOLVE (import is a pure model rebuild).

All captures use SkipTimestamp (the `! Last saved ...` stamp is non-deterministic).

Usage:
    python tools/golden/gen_json_import.py
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
OUT_DIR = REPO_ROOT / "tests" / "golden" / "json_import"
SCHEMA = 1
SKIP_TIMESTAMP = 512

# Each deck: a name + either `commands` (run after clear) or `master` (compiled).
DECKS = [
    {
        # Simple feeder: vsource(source edit) + linecode + 2 lines + load +
        # capacitor. No transformer/struct arrays. Exercises the specialFirst
        # vsource edit, the ObjectRef (linecode=), sym-component + the bus array.
        "name": "rt_micro",
        "commands": [
            "new circuit.probe basekv=12.47 bus1=sourcebus",
            "new linecode.lc1 nphases=3 r1=0.05 x1=0.1 c1=3 r0=0.15 x0=0.4",
            "new line.ln1 bus1=sourcebus bus2=b2 linecode=lc1 length=1.2",
            "new line.ln2 bus1=b2 bus2=b3 phases=3 r1=0.05 x1=0.1 r0=0.15 x0=0.4 c1=3 c0=1.5 length=0.8",
            "new load.l1 bus1=b3.1.2.3 kV=12.47 kW=10 pf=0.95",
            "new capacitor.c1 bus1=b3 phases=3 kvar=600 kv=12.47",
            "makebuslist",
            "setbusxy bus=sourcebus x=100.5 y=-200.25",
            "set voltagebases=[12.47]",
            "calcvoltagebases",
            "set keep=[b2]",
            "open line.ln1 term=2",
            "open line.ln2 term=1 cond=2",
        ],
    },
    {
        # Adds a two-winding transformer with explicit per-winding ON_ARRAY
        # scalars (RDCOhms/MaxTap/MinTap/NumTaps/RNeut) -> exercises the JSON
        # struct-array import + the array_alternative redirect (Bus/Conn/kV/kVA/%R).
        "name": "rt_transformer",
        "commands": [
            "new circuit.probe basekv=12.47 bus1=sourcebus",
            "new transformer.t1 windings=2 buses=(sourcebus, b2) "
            "conns=(delta, wye) kvs=(12.47, 0.48) kvas=(1000, 1000) "
            "xhl=6 %rs=(0.5, 0.5) "
            "wdg=1 rdcohms=0.11 maxtap=1.1 mintap=0.9 numtaps=32 rneut=0.5 "
            "wdg=2 rdcohms=0.22 maxtap=1.2 mintap=0.8 numtaps=16 rneut=1.5",
            "new load.l1 bus1=b2 kV=0.48 kW=100 pf=0.9",
            "makebuslist",
        ],
    },
    {
        # The unmodified IEEE13 feeder: transformers, matrix-model LineCodes,
        # capacitors, regcontrols, switches (Line.Switch ORDERING_FIRST) -- the
        # broadest whole-circuit round trip.
        "name": "rt_ieee13",
        "master": "Version8/Distrib/IEEETestCases/13Bus/IEEE13Nodeckt.dss",
    },
    {
        # Default DSS_OBJECT edit path: `edit spectrum.defaultload` marks the
        # default spectrum edited, so J0 carries it. `FillObjFromJSON` never
        # BeginEdit's, so the flag is NOT re-cleared on import and the re-export
        # (J1) DROPS it -- the oracle round trip is lossy for edited defaults.
        # Guards the `set_default_and_unedited` regression.
        "name": "rt_edited_default",
        "commands": [
            "new circuit.probe basekv=12.47",
            "edit spectrum.defaultload %mag=(100 1.5 20 14 1 9 7)",
            "makebuslist",
        ],
    },
    {
        # AllowDuplicates + positive-sequence model: exercises the dupsAllowed
        # create branch and the `Set CktModel=positive` PreCommand emission.
        "name": "rt_positive_seq",
        "commands": [
            "new circuit.probe basekv=12.47",
            "set cktmodel=positive",
            "set allowduplicates=yes",
            "set longlinecorrection=yes",
            "set ueweight=0.125",
            "set lossweight=2.675",
            "makebuslist",
        ],
    },
    {
        # Thevenin-DER coverage: a Generator (bus1/kV Required) widens the import
        # gate beyond the Vsource/Line/Load/Capacitor/Transformer/RegControl set.
        "name": "rt_generator",
        "commands": [
            "new circuit.probe basekv=12.47 bus1=sourcebus",
            "new generator.g1 bus1=sourcebus kv=12.47 kw=500 pf=0.9 model=1",
            "new load.l1 bus1=sourcebus kv=12.47 kw=100 pf=0.95",
            "makebuslist",
        ],
    },
]


def check_pin() -> str:
    import dss

    pins = {}
    for line in (REPO_ROOT / "tools" / "golden" / "PIN.txt").read_text().splitlines():
        line = line.strip()
        if line and not line.startswith("#"):
            k, v = line.split("==")
            pins[k] = v
    if dss.__version__ != pins["dss-python"]:
        sys.exit(
            f"dss-python {dss.__version__} != pinned {pins['dss-python']}; "
            f"install the pinned version or update PIN.txt deliberately"
        )
    return dss.__version__


def run_deck(d, au, lib, deck: dict) -> dict:
    def to_json() -> str:
        return au.get_string(lib.Circuit_ToJSON(SKIP_TIMESTAMP))

    def from_json(s: str) -> None:
        lib.Circuit_FromJSON(s.encode("utf-8"), 0)

    d.Text.Command = "clear"
    master = deck.get("master")
    if master is not None:
        master_abs = (
            REPO_ROOT / "tests" / "corpus" / "electricdss-tst" / master
        ).resolve()
        d.Text.Command = f'compile "{master_abs}"'
    for cmd in deck.get("commands", []):
        d.Text.Command = cmd

    j0 = to_json()
    from_json(j0)
    j1 = to_json()
    from_json(j1)
    j2 = to_json()

    if j1 != j2:
        sys.exit(
            f"deck {deck['name']}: oracle round trip not idempotent (J1 != J2); "
            f"cannot pin a stable import golden"
        )

    out = {
        "name": deck["name"],
        "commands": deck.get("commands", []),
        "input_json": j0,
        "expected_json": j1,
    }
    if master is not None:
        out["master"] = master
    return out


def main() -> None:
    oracle_version = check_pin()
    from dss import dss as d

    au = d._api_util
    lib = au.lib

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    for deck in DECKS:
        out = run_deck(d, au, lib, deck)
        out["schema"] = SCHEMA
        out["oracle"] = {"dss_python": oracle_version, "engine": d.Version}
        path = OUT_DIR / f"{deck['name']}.json"
        path.write_text(json.dumps(out, indent=2) + "\n", encoding="utf-8")
        print(
            f"wrote {path} (J0 {len(out['input_json'])} B -> J1 {len(out['expected_json'])} B)"
        )


if __name__ == "__main__":
    main()
