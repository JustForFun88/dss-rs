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
import re
import sys
from pathlib import Path

# Matches one numeric token (int/float/scientific) for garbage canonicalization.
_NUM_RE = re.compile(r"[-+]?\d*\.?\d+(?:[eE][-+]?\d+)?")

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
    {
        "name": "xfmrcode_default",
        "target": "XfmrCode.xc1",
        "commands": ["New XfmrCode.xc1"],
    },
    {
        "name": "xfmrcode_full",
        "target": "XfmrCode.xc1",
        "commands": [
            "New XfmrCode.xc1 phases=3 windings=3 "
            "conns=(delta, wye, wye) kvs=(115, 12.47, 4.16) kvas=(5000, 5000, 5000) "
            "xhl=8 xht=10 xlt=9 %loadloss=0.5 %noloadloss=0.2 %imag=0.1 "
            "thermal=2 n=0.8 m=0.8 flrise=65 hsrise=15 ppm=2",
        ],
    },
    {
        "name": "xfmrcode_wdg_seq",
        "target": "XfmrCode.xc1",
        "commands": [
            "New XfmrCode.xc1 windings=2 phases=1",
            "~ wdg=1 conn=wye kv=7.2 kva=25 %r=1.2 tap=1.05 rneut=0.1 xneut=0.2 "
            "maxtap=1.1 mintap=0.9 numtaps=32 rdcohms=0.5",
            "~ wdg=2 conn=wye kv=0.24 kva=25 %r=1.2",
        ],
    },
    {
        "name": "xfmrcode_xscarray",
        "target": "XfmrCode.xc1",
        "commands": ["New XfmrCode.xc1 windings=3 xscarray=(8 10 9)"],
    },
    {
        "name": "xfmrcode_ratings",
        "target": "XfmrCode.xc1",
        "commands": ["New XfmrCode.xc1 seasons=3 ratings=(600, 700, 800)"],
    },
    {
        "name": "xfmrcode_makelike",
        "target": "XfmrCode.xc1",
        "commands": [
            "New XfmrCode.base windings=2 kvs=(115, 4.16) kvas=(3000, 3000) xhl=7",
            "New XfmrCode.xc1 like=base",
        ],
    },
    # --- Transformer (WP4.4) ---
    {
        "name": "transformer_default",
        "target": "Transformer.t1",
        "commands": ["New Transformer.t1"],
    },
    {
        "name": "transformer_sub",
        "target": "Transformer.sub",
        "commands": [
            "New Transformer.sub phases=3 windings=2 buses=(SourceBus, 650) "
            "conns=(delta, wye) kvs=(115, 4.16) kvas=(5000, 5000) xhl=8 %r=0.5",
        ],
    },
    {
        "name": "transformer_wdg_seq",
        "target": "Transformer.t2",
        "commands": [
            "New Transformer.t2 phases=1 windings=2",
            "~ wdg=1 bus=a.1 conn=wye kv=7.2 kva=25 tap=1.0 %r=0.6 "
            "rneut=0.1 xneut=0.2 maxtap=1.1 mintap=0.9 numtaps=32",
            "~ wdg=2 bus=b.1 conn=wye kv=0.24 kva=25",
        ],
    },
    {
        "name": "transformer_3wdg",
        "target": "Transformer.t3",
        "commands": [
            "New Transformer.t3 phases=3 windings=3 "
            "buses=(p, s, t) conns=(delta, wye, wye) kvs=(115, 12.47, 4.16) "
            "kvas=(5000, 5000, 5000) xhl=8 xht=10 xlt=9 %loadloss=0.5",
        ],
    },
    {
        "name": "transformer_xscarray",
        "target": "Transformer.t4",
        "commands": [
            "New Transformer.t4 phases=3 windings=3 buses=(a, b, c) "
            "kvs=(115, 12.47, 4.16) kvas=(5000, 5000, 5000) xscarray=(8 10 9)",
        ],
    },
    {
        "name": "transformer_xfmrcode",
        "target": "Transformer.t5",
        "commands": [
            "New XfmrCode.xc windings=2 kvs=(115, 4.16) kvas=(3000, 3000) "
            "xhl=7 conns=(delta, wye)",
            "New Transformer.t5 buses=(p, s) xfmrcode=xc",
        ],
    },
    {
        "name": "transformer_makelike",
        "target": "Transformer.t6",
        "commands": [
            "New Transformer.base phases=3 windings=2 buses=(p, s) "
            "conns=(delta, wye) kvs=(115, 4.16) kvas=(3000, 3000) xhl=7 %r=0.4",
            "New Transformer.t6 like=base buses=(p2, s2)",
        ],
    },
    # --- Capacitor (WP4.5) ---
    # NOTE: the oracle's `DoubleSymMatrixProperty` getter for `Capacitor.CMatrix`
    # reads uninitialized memory (it returns denormal garbage even when
    # `cmatrix=` is set — a genuine dss_capi bug), so its numbers are
    # canonicalized to zeros via `zero_garbage` in every Capacitor scenario; only
    # the matrix skeleton is pinned, and the Rust getter emits the same zeros.
    {
        "name": "cap_default",
        "target": "Capacitor.c1",
        "commands": ["New Capacitor.c1"],
        "zero_garbage": ["CMatrix"],
    },
    {
        "name": "cap_kvar",
        "target": "Capacitor.c1",
        "commands": ["New Capacitor.c1 bus1=b1 phases=3 kvar=600 kv=4.16"],
        "zero_garbage": ["CMatrix"],
    },
    {
        "name": "cap_cuf",
        "target": "Capacitor.c1",
        "commands": ["New Capacitor.c1 bus1=b1 phases=1 cuf=10 kv=2.4"],
        "zero_garbage": ["CMatrix"],
    },
    {
        "name": "cap_cmatrix",
        "target": "Capacitor.c1",
        "commands": [
            "New Capacitor.c1 bus1=b1 phases=3 "
            "cmatrix=(2.8 | -0.6 2.8 | -0.6 -0.6 2.8)",
        ],
        "zero_garbage": ["CMatrix"],
    },
    {
        "name": "cap_numsteps",
        "target": "Capacitor.c1",
        "commands": [
            "New Capacitor.c1 bus1=b1 phases=3 kvar=600 kv=4.16 numsteps=3 states=(1 1 0)",
        ],
        "zero_garbage": ["CMatrix"],
    },
    {
        "name": "cap_series_xl",
        "target": "Capacitor.c1",
        "commands": [
            "New Capacitor.c1 bus1=b1 bus2=b2 phases=3 kvar=600 kv=4.16 r=0.1 xl=1.0",
        ],
        "zero_garbage": ["CMatrix"],
    },
    {
        "name": "cap_makelike",
        "target": "Capacitor.c1",
        "commands": [
            "New Capacitor.base bus1=b1 phases=3 kvar=300 kv=4.16",
            "New Capacitor.c1 like=base",
        ],
        "zero_garbage": ["CMatrix"],
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

    # Properties whose oracle getter returns uninitialized-memory garbage (see
    # the per-scenario notes) are canonicalized: every number in the value is
    # rewritten to 0, so only the structural skeleton is pinned. The Rust engine
    # emits the same zero matrix (a deterministic repro of the broken getter).
    zero_garbage = {e.lower() for e in scenario.get("zero_garbage", [])}
    props = {}
    for name in names:
        d.Text.Command = f"? {target}.{name}"
        value = d.Text.Result
        if name.lower() in zero_garbage:
            value = _NUM_RE.sub("0", value)
        props[name] = value

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
