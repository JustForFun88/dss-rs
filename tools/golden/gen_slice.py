"""Generate the Phase 3 vertical-slice golden from dss-python (oracle).

Each scenario is a complete command script (`clear` through `solve`). The
oracle records the global node order, the complex node voltages, the
fixed-point iteration count and the convergence flag; the Rust engine must
replay the identical script and match (PORTING_PLAN.md Phase 3 gate):

  (a) 2-bus vsource+line+load voltages to 1e-9 rel,
  (b) the "IEEE13-flat" variant (transformers/regulators/caps stripped,
      linecodes inlined as per-line matrices) to 1e-6 rel,
  (c) iteration counts exactly equal,
  (d) all 8 load models exercised and matching.

Usage:
    python tools/golden/gen_slice.py

Reads  nothing (scenarios are defined below)
Writes tests/golden/slice.json

Regeneration is manual and must use the exact versions in tools/golden/PIN.txt.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
OUT = REPO_ROOT / "tests" / "golden" / "slice.json"
SCHEMA = 1

TWOBUS_TOL = 1e-9
IEEE13_TOL = 1e-6


def twobus(load_props: str, name: str, tol: float = TWOBUS_TOL, km: float = 1) -> dict:
    """The 2-bus vsource + line + load case, parameterized on the load."""
    return {
        "name": name,
        "tol": tol,
        "commands": [
            "clear",
            "Set DefaultBaseFrequency=60",
            "new circuit.twobus basekv=12.47 pu=1.0 phases=3 bus1=sourcebus "
            "mvasc3=2000 mvasc1=2100",
            f"new line.l1 bus1=sourcebus bus2=loadbus length={km} units=km "
            "r1=0.1 x1=0.4 r0=0.3 x0=1.2 c1=3.4 c0=1.6",
            f"new load.ld1 bus1=loadbus {load_props}",
            "Set voltagebases=[12.47]",
            "calcvoltagebases",
            "solve",
        ],
    }


def _mi(feet: float) -> str:
    """Original lengths are in ft; the inlined matrices are per mile (the
    linecodes carried `units=mi`), so lengths convert to miles. Both engines
    parse the same decimal string, so the conversion itself is not compared.
    """
    return repr(feet / 5280.0)


# IEEE13 with the substation/regulator/XFM1 transformers, regcontrols and
# capacitors stripped; the source feeds bus 650 at 4.16 kV directly, the
# linecodes are inlined as per-line matrices (LineCode arrives in Phase 4) and
# the 0.48 kV loads on 634 are re-homed to 633 at 2.4 kV. Matrix values are
# the mtx601..mtx607 entries from IEEE13Nodeckt.dss; the cmatrix for codes
# that had none uses the engine-default C1=3.4/C0=1.6 sym-derived entries
# (Cs=2.8, Cm=-0.6; 1-phase keeps C1).
IEEE13_FLAT = {
    "name": "ieee13_flat",
    "tol": IEEE13_TOL,
    "commands": [
        "clear",
        "Set DefaultBaseFrequency=60",
        "new circuit.ieee13flat basekv=4.16 pu=1.0001 phases=3 bus1=650 "
        "Angle=30 MVAsc3=20000 MVASC1=21000",
        # mtx601 lines
        "New Line.650632 Phases=3 Bus1=650.1.2.3 Bus2=632.1.2.3 "
        "rmatrix=(0.3465 | 0.1560 0.3375 | 0.1580 0.1535 0.3414) "
        "xmatrix=(1.0179 | 0.5017 1.0478 | 0.4236 0.3849 1.0348) "
        "cmatrix=(2.8 | -0.6 2.8 | -0.6 -0.6 2.8) "
        f"Length={_mi(2000)} units=mi",
        "New Line.632670 Phases=3 Bus1=632.1.2.3 Bus2=670.1.2.3 "
        "rmatrix=(0.3465 | 0.1560 0.3375 | 0.1580 0.1535 0.3414) "
        "xmatrix=(1.0179 | 0.5017 1.0478 | 0.4236 0.3849 1.0348) "
        "cmatrix=(2.8 | -0.6 2.8 | -0.6 -0.6 2.8) "
        f"Length={_mi(667)} units=mi",
        "New Line.670671 Phases=3 Bus1=670.1.2.3 Bus2=671.1.2.3 "
        "rmatrix=(0.3465 | 0.1560 0.3375 | 0.1580 0.1535 0.3414) "
        "xmatrix=(1.0179 | 0.5017 1.0478 | 0.4236 0.3849 1.0348) "
        "cmatrix=(2.8 | -0.6 2.8 | -0.6 -0.6 2.8) "
        f"Length={_mi(1333)} units=mi",
        "New Line.671680 Phases=3 Bus1=671.1.2.3 Bus2=680.1.2.3 "
        "rmatrix=(0.3465 | 0.1560 0.3375 | 0.1580 0.1535 0.3414) "
        "xmatrix=(1.0179 | 0.5017 1.0478 | 0.4236 0.3849 1.0348) "
        "cmatrix=(2.8 | -0.6 2.8 | -0.6 -0.6 2.8) "
        f"Length={_mi(1000)} units=mi",
        # mtx602
        "New Line.632633 Phases=3 Bus1=632.1.2.3 Bus2=633.1.2.3 "
        "rmatrix=(0.7526 | 0.1580 0.7475 | 0.1560 0.1535 0.7436) "
        "xmatrix=(1.1814 | 0.4236 1.1983 | 0.5017 0.3849 1.2112) "
        "cmatrix=(2.8 | -0.6 2.8 | -0.6 -0.6 2.8) "
        f"Length={_mi(500)} units=mi",
        # mtx603
        "New Line.632645 Phases=2 Bus1=632.3.2 Bus2=645.3.2 "
        "rmatrix=(1.3238 | 0.2066 1.3294) "
        "xmatrix=(1.3569 | 0.4591 1.3471) "
        "cmatrix=(2.8 | -0.6 2.8) "
        f"Length={_mi(500)} units=mi",
        "New Line.645646 Phases=2 Bus1=645.3.2 Bus2=646.3.2 "
        "rmatrix=(1.3238 | 0.2066 1.3294) "
        "xmatrix=(1.3569 | 0.4591 1.3471) "
        "cmatrix=(2.8 | -0.6 2.8) "
        f"Length={_mi(300)} units=mi",
        # mtx606 (the corrected 2016 values)
        "New Line.692675 Phases=3 Bus1=692.1.2.3 Bus2=675.1.2.3 "
        "rmatrix=[0.791721 | 0.318476 0.781649 | 0.28345 0.318476 0.791721] "
        "xmatrix=[0.438352 | 0.0276838 0.396697 | -0.0184204 0.0276838 0.438352] "
        "cmatrix=[383.948 | 0 383.948 | 0 0 383.948] "
        f"Length={_mi(500)} units=mi",
        # mtx604
        "New Line.671684 Phases=2 Bus1=671.1.3 Bus2=684.1.3 "
        "rmatrix=(1.3238 | 0.2066 1.3294) "
        "xmatrix=(1.3569 | 0.4591 1.3471) "
        "cmatrix=(2.8 | -0.6 2.8) "
        f"Length={_mi(300)} units=mi",
        # mtx605
        "New Line.684611 Phases=1 Bus1=684.3 Bus2=611.3 "
        "rmatrix=(1.3292) xmatrix=(1.3475) cmatrix=(3.4) "
        f"Length={_mi(300)} units=mi",
        # mtx607
        "New Line.684652 Phases=1 Bus1=684.1 Bus2=652.1 "
        "rmatrix=(1.3425) xmatrix=(0.5124) cmatrix=[236] "
        f"Length={_mi(800)} units=mi",
        # switch
        "New Line.671692 Phases=3 Bus1=671 Bus2=692 Switch=y "
        "r1=1e-4 r0=1e-4 x1=0.000 x0=0.000 c1=0.000 c0=0.000",
        # loads (634 loads re-homed to 633 at 2.4 kV; caps stripped)
        "New Load.671 Bus1=671.1.2.3 Phases=3 Conn=Delta Model=1 kV=4.16 kW=1155 kvar=660",
        "New Load.633a Bus1=633.1 Phases=1 Conn=Wye Model=1 kV=2.4 kW=160 kvar=110",
        "New Load.633b Bus1=633.2 Phases=1 Conn=Wye Model=1 kV=2.4 kW=120 kvar=90",
        "New Load.633c Bus1=633.3 Phases=1 Conn=Wye Model=1 kV=2.4 kW=120 kvar=90",
        "New Load.645 Bus1=645.2 Phases=1 Conn=Wye Model=1 kV=2.4 kW=170 kvar=125",
        "New Load.646 Bus1=646.2.3 Phases=1 Conn=Delta Model=2 kV=4.16 kW=230 kvar=132",
        "New Load.692 Bus1=692.3.1 Phases=1 Conn=Delta Model=5 kV=4.16 kW=170 kvar=151",
        "New Load.675a Bus1=675.1 Phases=1 Conn=Wye Model=1 kV=2.4 kW=485 kvar=190",
        "New Load.675b Bus1=675.2 Phases=1 Conn=Wye Model=1 kV=2.4 kW=68 kvar=60",
        "New Load.675c Bus1=675.3 Phases=1 Conn=Wye Model=1 kV=2.4 kW=290 kvar=212",
        "New Load.611 Bus1=611.3 Phases=1 Conn=Wye Model=5 kV=2.4 kW=170 kvar=80",
        "New Load.652 Bus1=652.1 Phases=1 Conn=Wye Model=2 kV=2.4 kW=128 kvar=86",
        "New Load.670a Bus1=670.1 Phases=1 Conn=Wye Model=1 kV=2.4 kW=17 kvar=10",
        "New Load.670b Bus1=670.2 Phases=1 Conn=Wye Model=1 kV=2.4 kW=66 kvar=38",
        "New Load.670c Bus1=670.3 Phases=1 Conn=Wye Model=1 kV=2.4 kW=117 kvar=68",
        "Set Voltagebases=[4.16]",
        "calcv",
        "Solve",
    ],
}

SCENARIOS = [
    # (a) the basic 2-bus case
    twobus("phases=3 kv=12.47 kw=600 pf=0.95 model=1", "twobus"),
    # (d) all 8 load models
    twobus("phases=3 kv=12.47 kw=600 pf=0.95 model=2", "twobus_model2"),
    twobus("phases=3 kv=12.47 kw=600 pf=0.95 model=3", "twobus_model3"),
    twobus("phases=3 kv=12.47 kw=600 pf=0.95 model=4", "twobus_model4"),
    twobus(
        "phases=3 kv=12.47 kw=600 pf=0.95 model=4 CVRwatts=0.8 CVRvars=2.5",
        "twobus_model4_powf",
    ),
    twobus("phases=3 kv=12.47 kw=600 pf=0.95 model=5", "twobus_model5"),
    twobus("phases=3 kv=12.47 kw=600 kvar=200 model=6", "twobus_model6"),
    twobus("phases=3 kv=12.47 kw=600 kvar=200 model=7", "twobus_model7"),
    twobus(
        "phases=3 kv=12.47 kw=600 pf=0.95 model=8 "
        "zipv=[0.4 0.4 0.2 0.5 0.3 0.2 0.6]",
        "twobus_model8",
    ),
    # delta connection variant
    twobus("phases=3 kv=12.47 kw=600 pf=0.95 model=1 conn=delta", "twobus_delta"),
    # heavy load on a long line: pulls |V| below 0.95 pu to exercise the
    # low-voltage interpolation zones (and a longer fixed-point iteration)
    twobus("phases=3 kv=12.47 kw=8000 pf=0.90 model=1", "twobus_heavy", km=5),
    twobus("phases=3 kv=12.47 kw=8000 pf=0.90 model=5", "twobus_heavy_model5", km=5),
    # (b) the IEEE13-flat multi-phase case
    IEEE13_FLAT,
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


def run_scenario(d, scenario: dict) -> dict:
    for cmd in scenario["commands"]:
        d.Text.Command = cmd

    varray = list(d.ActiveCircuit.YNodeVarray)
    return {
        "name": scenario["name"],
        "tol": scenario["tol"],
        "commands": scenario["commands"],
        "converged": bool(d.ActiveCircuit.Solution.Converged),
        "iterations": int(d.ActiveCircuit.Solution.Iterations),
        "node_order": list(d.ActiveCircuit.YNodeOrder),
        "v_re": varray[0::2],
        "v_im": varray[1::2],
    }


def main() -> None:
    oracle_version = check_pin()
    from dss import dss as d

    data = {
        "schema": SCHEMA,
        "oracle": {"dss_python": oracle_version, "engine": d.Version},
        "scenarios": [run_scenario(d, s) for s in SCENARIOS],
    }
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(data, indent=1))
    for s in data["scenarios"]:
        print(
            f"{s['name']}: {len(s['node_order'])} nodes, "
            f"{s['iterations']} iterations, converged={s['converged']}"
        )
    print(f"-> {OUT.relative_to(REPO_ROOT)}")


if __name__ == "__main__":
    main()
