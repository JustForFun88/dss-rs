"""Generate the Monitor mode-4 flicker golden (WP-PF.2).

UNUSUAL ORACLE — read this before regenerating. The pinned dss_capi oracle
(dss-python 0.15.7) **cannot** produce a mode-4 flicker result: its
`DoFlickerCalculations` (`Monitor.pas:1655`) indexes `Terminals[MeteredTerminal]`
with the 1-based terminal number, but the dss_capi rewrite made `Terminals` a
0-based array, so `Terminals[1]` on a 1-terminal element reads one entry past the
array -> an **unconditional access violation** on any `export monitor` / `Monitors.
Process()` of a mode-4 monitor (probe-verified 2026-07-11: hard segfault). The
**official** Delphi engine keeps `Terminals` 1-based (`pTerminalList`), so it runs
the flicker calc correctly.

Therefore this golden is captured from the **official EPRI r3723 binary** through
the Oddie bridge (the D9 reference channel), NOT the pinned oracle. It pins the
ported `flicker_meter` (support/flicker.rs) which is bit-exact vs r3723
(f64-arithmetic / f32-storage model). Run with the Oddie venv:

    tools/opendss/.venv/Scripts/python.exe tools/golden/gen_flicker.py

The Rust replay (`crates/dss-core/tests/golden_flicker.rs`) is solve-decoupled: it
feeds the captured raw per-phase magnitudes straight into `flicker_meter` and
compares the flicker + Pst channels **f32-exact**, so no power-flow floor enters.
"""

import json
import os
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
DLL = REPO / "tools" / "opendss" / "bin" / "r3723" / "OpenDSSDirect.dll"
DEMO = REPO / "tests/corpus/electricdss-tst/Version8/Distrib/Examples/Matlab"
OUT = REPO / "tests" / "golden" / "flicker" / "pst_demo.json"

# The upstream flicker demo (Examples/Matlab/pst.dss): a stiff source + isource +
# a model=2 load driven by the vendored WindRmsV.csv duty shape, monitored mode 4.
DECK = [
    "clear",
    "Set DefaultBaseFrequency=60",
    "new circuit.pst bus1=EQSRC basekv=1.73205 pu=1 r1=0.00001 x1=0.00001 r0=0.00001 x0=0.00001",
    "New LoadShape.pst npts=8640 interval=(10.0 3600 /)",
    "~   mult=(File=WindRmsV.csv) Action=Normalize",
    "New isource.pst bus1=PCC amps=1 angle=0",
    "New load.pst    bus1=PCC kV=1.73205 kW=3.0 pf=1.0 conn=wye duty=pst vminpu=0.01  Model=2",
    "new Monitor.pst element=load.pst terminal=1 mode=4",
    'set voltagebases="1.73205"',
    "calcvoltagebases",
    "solve mode=duty stepsize=10 number=8640",
    "wait",  # PM/async solve barrier (D9 rule) before reading.
]


def main() -> None:
    from dss import IOddieDSS  # Oddie venv only

    d = IOddieDSS(library_path=str(DLL))
    d.AllowForms = False
    try:
        d.AllowEditor = False
    except Exception:
        pass

    os.chdir(DEMO)  # File=WindRmsV.csv is relative to the deck
    for c in DECK:
        d.Text.Command = c

    ckt = d.ActiveCircuit
    mon = ckt.Monitors
    mon.Name = "pst"
    n = int(mon.SampleCount)
    nph = int(mon.NumChannels) // 2

    # Raw per-phase RMS magnitudes (odd data channels) BEFORE export — the exact
    # f32 inputs DoFlickerCalculations feeds into FlickerMeter.
    raw_mag = [[float(x) for x in mon.Channel(2 * p + 1)] for p in range(nph)]

    ckt.SetActiveBus("PCC")
    kvbase = float(ckt.ActiveBus.kVBase)

    # Export triggers the official DoFlickerCalculations, rewriting the stream in
    # place; read the post-processed channels back at full f32 precision.
    d.Text.Command = "export monitor pst"
    flk = [[float(x) for x in mon.Channel(2 * p + 1)] for p in range(nph)]
    pst = [[float(x) for x in mon.Channel(2 * p + 2)] for p in range(nph)]

    # Absolute sample times (uniform 10 s duty step): t[i] = 10*(i+1) s.
    times = [10.0 * (i + 1) for i in range(n)]

    data = {
        "schema": 1,
        "oracle": {
            "engine": "official-EPRI-r3723 (Oddie)",
            "version": str(d.Version),
            "note": "pinned dss_capi oracle crashes on mode-4 export (Terminals OOB); "
            "official r3723 is the correct reference (D9 channel).",
        },
        "deck": DECK,
        "n": n,
        "nphases": nph,
        "fbase": 60.0,
        "kvbase": kvbase,
        "times": times,
        "raw_mag": raw_mag,
        "flk": flk,
        "pst": pst,
    }
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(data))
    print(f"wrote {OUT} (N={n}, nphases={nph}, engine={d.Version})")


if __name__ == "__main__":
    main()
