"""Generate the Phase 2 property round-trip golden from dss-python (oracle).

For each scenario we run a short command script through the oracle, then read
back every property of the target object via the `?` query. The Rust engine
must replay the identical scripts and produce the same property values
(numbers compared with tolerance, structure exactly — see PORTING_PLAN.md §4).

Only fully-specified or all-default states are probed: arrays sized by `Npts`
but never filled contain uninitialized heap garbage in the oracle (genuine UB,
non-deterministic), so those states are deliberately excluded.

Usage:
    python tools/golden/gen_props.py

Reads  nothing (scenarios are defined below)
Writes tests/golden/props.json

Regeneration is manual and must use the exact versions in tools/golden/PIN.txt.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
OUT = REPO_ROOT / "tests" / "golden" / "props.json"
SCHEMA = 1

# Each scenario: a name, the command script (run after `clear; new circuit`),
# and the target object whose properties are read back.
SCENARIOS = [
    {
        "name": "tcc_default",
        "target": "TCC_Curve.tcc1",
        "commands": ["New TCC_Curve.tcc1"],
    },
    {
        "name": "tcc_full",
        "target": "TCC_Curve.tcc1",
        "commands": ["New TCC_Curve.tcc1 npts=3 C_array=(1 2 3) T_array=(0.1 0.2 0.3)"],
    },
    {
        "name": "tcc_abbrev",
        "target": "TCC_Curve.tcc1",
        "commands": ["New TCC_Curve.tcc1 np=2 c=(10 20) t=(1 2)"],
    },
    {
        "name": "tcc_edit_then_more",
        "target": "TCC_Curve.tcc1",
        "commands": [
            "New TCC_Curve.tcc1 npts=2",
            "Edit TCC_Curve.tcc1 C_array=(5 6)",
            "~ T_array=(9 8)",
        ],
    },
    {
        "name": "tcc_shrink_npts",
        "target": "TCC_Curve.tcc1",
        "commands": [
            "New TCC_Curve.tcc1 npts=3 C_array=(1 2 3) T_array=(7 8 9)",
            "Edit TCC_Curve.tcc1 npts=2",
        ],
    },
    {
        "name": "tcc_makelike",
        "target": "TCC_Curve.tcc1",
        "commands": [
            "New TCC_Curve.base npts=2 C_array=(1 2) T_array=(3 4)",
            "New TCC_Curve.tcc1 like=base",
        ],
    },
    {
        "name": "spectrum_default",
        "target": "Spectrum.s1",
        "commands": ["New Spectrum.s1"],
    },
    {
        "name": "spectrum_full",
        "target": "Spectrum.s1",
        "commands": [
            "New Spectrum.s1 NumHarm=3 harmonic=(1 5 7) %mag=(100 20 14.3) angle=(0 0 0)",
        ],
    },
    {
        "name": "spectrum_abbrev",
        "target": "Spectrum.s1",
        "commands": ["New Spectrum.s1 numh=2 harm=(1 3) %m=(100 33) ang=(0 180)"],
    },
    {
        "name": "spectrum_makelike",
        "target": "Spectrum.s1",
        "commands": [
            "New Spectrum.base NumHarm=2 harmonic=(1 3) %mag=(100 33) angle=(0 180)",
            "New Spectrum.s1 like=base",
        ],
    },
    {
        "name": "linecode_default",
        "target": "LineCode.lc1",
        "commands": ["New LineCode.lc1"],
    },
    {
        "name": "linecode_sym",
        "target": "LineCode.lc1",
        "commands": [
            "New LineCode.lc1 nphases=3 r1=0.1 x1=0.3 r0=0.2 x0=0.6 "
            "c1=3.0 c0=1.5 normamps=500 emergamps=700 units=kft linetype=ug",
        ],
    },
    {
        "name": "linecode_sym_b",
        "target": "LineCode.lc1",
        "commands": [
            "New LineCode.lc1 nphases=1 r1=0.05 x1=0.1 b1=2.0 b0=1.0 units=mi",
        ],
    },
    {
        "name": "linecode_matrix",
        "target": "LineCode.lc1",
        "commands": [
            "New LineCode.lc1 nphases=3 "
            "rmatrix=(0.09 | 0.04 0.09 | 0.04 0.04 0.09) "
            "xmatrix=(0.2 | 0.09 0.2 | 0.09 0.09 0.2) "
            "cmatrix=(2.8 | -0.6 2.8 | -0.6 -0.6 2.8)",
        ],
    },
    {
        "name": "linecode_code_then_units",
        "target": "LineCode.lc1",
        "commands": [
            "New LineCode.lc1 nphases=2 rmatrix=(0.1 | 0.05 0.1) "
            "xmatrix=(0.2 | 0.07 0.2) cmatrix=(3 | -1 3) units=mi",
        ],
    },
    {
        "name": "linecode_kron",
        "target": "LineCode.lc1",
        "commands": [
            "New LineCode.lc1 nphases=4 "
            "rmatrix=(0.1 | 0.04 0.1 | 0.04 0.04 0.1 | 0.04 0.04 0.04 0.1) "
            "xmatrix=(0.2 | 0.09 0.2 | 0.09 0.09 0.2 | 0.09 0.09 0.09 0.2) "
            "cmatrix=(2.8 | -0.6 2.8 | -0.6 -0.6 2.8 | -0.6 -0.6 -0.6 2.8) "
            "kron=y",
        ],
    },
    {
        "name": "linecode_seasons",
        "target": "LineCode.lc1",
        "commands": [
            "New LineCode.lc1 nphases=3 seasons=3 ratings=(400 500 600)",
        ],
    },
    {
        "name": "linecode_makelike",
        "target": "LineCode.lc1",
        "commands": [
            "New LineCode.base nphases=2 r1=0.2 x1=0.4 r0=0.3 x0=0.7 c1=2.5 c0=1.2",
            "New LineCode.lc1 like=base",
        ],
    },
    # --- Line + LineCode fetch (WP4.2) ---
    {
        "name": "line_code_sym",
        "target": "Line.l1",
        "commands": [
            "New LineCode.mtx601 nphases=3 r1=0.1 x1=0.2 r0=0.3 x0=0.6 "
            "c1=3 c0=1 units=mi normamps=500 emergamps=700",
            "New Line.l1 bus1=a bus2=b linecode=mtx601 length=2000 units=ft",
        ],
    },
    {
        "name": "line_code_then_units",
        "target": "Line.l1",
        "commands": [
            "New LineCode.mtx601 nphases=3 r1=0.1 x1=0.2 r0=0.3 x0=0.6 "
            "c1=3 c0=1 units=mi",
            "New Line.l1 bus1=a bus2=b linecode=mtx601 units=ft length=2000",
        ],
    },
    {
        "name": "line_code_matrix",
        "target": "Line.l1",
        "commands": [
            "New LineCode.mx nphases=2 rmatrix=(0.1 | 0.05 0.1) "
            "xmatrix=(0.2 | 0.07 0.2) cmatrix=(3 | -1 3) units=mi",
            "New Line.l1 bus1=a.1.2 bus2=b.1.2 linecode=mx length=1 units=mi",
        ],
    },
    {
        "name": "line_code_then_r1",
        "target": "Line.l1",
        "commands": [
            "New LineCode.mtx601 nphases=3 r1=0.1 x1=0.2 r0=0.3 x0=0.6 units=mi",
            "New Line.l1 bus1=a bus2=b linecode=mtx601 r1=0.5 length=1 units=mi",
        ],
    },
    # --- GrowthShape (WP4.3) ---
    {
        "name": "growthshape_default",
        "target": "GrowthShape.gs1",
        "commands": ["New GrowthShape.gs1"],
    },
    {
        "name": "growthshape_full",
        "target": "GrowthShape.gs1",
        "commands": [
            "New GrowthShape.gs1 npts=5 "
            "year=(1999 2000 2001 2005 2010) "
            "mult=(1.10 1.07 1.05 1.025 1.01)",
        ],
    },
    {
        "name": "growthshape_year_rounds",
        "target": "GrowthShape.gs1",
        "commands": [
            "New GrowthShape.gs1 npts=3 year=(2000.4 2001.6 2002.5) mult=(1.05 1.04 1.03)",
        ],
    },
    {
        "name": "growthshape_edit_shrink",
        "target": "GrowthShape.gs1",
        "commands": [
            "New GrowthShape.gs1 npts=4 year=(2000 2001 2002 2003) mult=(1.05 1.04 1.03 1.02)",
            "Edit GrowthShape.gs1 npts=2",
        ],
    },
    {
        "name": "growthshape_makelike",
        "target": "GrowthShape.gs1",
        "commands": [
            "New GrowthShape.base npts=2 year=(2000 2010) mult=(1.05 1.02)",
            "New GrowthShape.gs1 like=base",
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


def run_scenario(d, scenario: dict) -> dict:
    d.Text.Command = "clear"
    d.Text.Command = "new circuit.propsprobe"
    for cmd in scenario["commands"]:
        d.Text.Command = cmd

    target = scenario["target"]
    # Activate the target object (the `?` query sets it active) and read names.
    d.Text.Command = f"? {target}.Like"
    names = list(d.ActiveCircuit.ActiveDSSElement.AllPropertyNames)

    props = {}
    for name in names:
        d.Text.Command = f"? {target}.{name}"
        props[name] = d.Text.Result

    return {
        "name": scenario["name"],
        "commands": scenario["commands"],
        "target": target,
        "properties": props,
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
        print(f"{s['name']}: {s['properties']} -> {OUT.relative_to(REPO_ROOT)}")


if __name__ == "__main__":
    main()
