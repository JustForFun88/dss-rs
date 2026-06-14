"""Generate tests/golden/ieee8500.json from the pinned oracle (PHASE6_PLAN WP6.9).

The headline Phase-6 gate: compile the **unmodified** IEEE 8500-Node master
(`Version8/Distrib/IEEETestCases/8500-Node/Master.dss`), then, per
`Run_8500Node.dss`, attach `Energymeter.m1` on `Line.ln5815900-1`, raise the
iteration cap to 20 and `Solve` (snapshot). This is the controls-at-scale
regression — the feeder has 12 single-phase RegControls (4 banks) and 10
CapControls over ~8.5k nodes / ~6k branches / 1190 transformers.

Two capture points:
  - **snap**: after the snapshot solve — iteration count, `YNodeOrder`, node
    voltages, total power + losses, the regulated transformer taps, every
    RegControl tap number and every capacitor state.
  - **registers**: after an additional 24-step daily segment (`set mode=daily
    number=24 stepsize=1h; solve`) so the EnergyMeter integrates a full day
    over its whole zone — pins the zone kWh / loss-split / sequence /
    voltage-base-bucket / EEN-UE register machinery at scale.

The `Interpolate` command from `Run_8500Node.dss` is dropped (coordinate-only,
not ported — `InterpolateCoordinates` is Phase 8), as are all Show/Export/Plot
commands (file/UI output, Phase 8). Neither affects the solution or registers.

Usage:
    python tools/golden/gen_ieee8500.py

Writes tests/golden/ieee8500.json. Regeneration is manual and must use the
exact versions in tools/golden/PIN.txt.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
OUT = REPO_ROOT / "tests" / "golden" / "ieee8500.json"
SCHEMA = 1

# Master path. This generator (manual, maintainer-only) reads the master from
# `.inputs/electricdss-tst`; the gate (`golden_ieee8500.rs`) reads the same
# relative path from the vendored `tests/corpus/electricdss-tst` copy.
MASTER_REL = "Version8/Distrib/IEEETestCases/8500-Node/Master.dss"

# Replayed after `Compile <master>` (the harness issues the compile itself).
SNAP_COMMANDS = [
    "New Energymeter.m1 Line.ln5815900-1 1",
    "Set Maxiterations=20",
    "Solve",
]
DAILY_COMMANDS = [
    "Set mode=daily number=24 stepsize=1h",
    "Solve",
]


def check_pin() -> dict:
    import dss

    if dss.__version__ != "0.15.7":
        sys.exit(f"dss-python {dss.__version__} != pinned 0.15.7 (tools/golden/PIN.txt)")
    from dss import DSS

    return {"dss_python": dss.__version__, "engine": DSS.Version}


def capture_controls(ckt) -> tuple[dict, dict, dict]:
    """Regulated transformer taps (only those moved off 1.0), RegControl tap
    numbers, capacitor states — the exact controls-at-scale checks."""
    transformers = {}
    tr = ckt.Transformers
    i = tr.First
    while i:
        taps = []
        moved = False
        for w in range(1, tr.NumWindings + 1):
            tr.Wdg = w
            tap = float(tr.Tap)
            taps.append(tap)
            if abs(tap - 1.0) > 1e-9:
                moved = True
        # Only the regulated transformers move; 1178 fixed load xfmrs stay at
        # 1.0 and would bloat the golden with redundant float comparisons.
        if moved:
            transformers[tr.Name] = taps
        i = tr.Next

    regcontrols = {}
    rc = ckt.RegControls
    i = rc.First
    while i:
        regcontrols[rc.Name] = int(rc.TapNumber)
        i = rc.Next

    capacitors = {}
    cap = ckt.Capacitors
    i = cap.First
    while i:
        capacitors[cap.Name] = [int(s) for s in cap.States]
        i = cap.Next

    return transformers, regcontrols, capacitors


def main() -> None:
    oracle = check_pin()
    from dss import DSS as d

    master_abs = (REPO_ROOT / ".inputs" / "electricdss-tst" / MASTER_REL).resolve()
    if not master_abs.is_file():
        sys.exit(f"master not found: {master_abs}")

    d.Text.Command = "clear"
    d.Text.Command = f'Compile "{master_abs}"'
    for cmd in SNAP_COMMANDS:
        d.Text.Command = cmd

    ckt = d.ActiveCircuit
    sol = ckt.Solution
    assert sol.Converged, "oracle snapshot did not converge"

    varray = list(ckt.YNodeVarray)
    transformers, regcontrols, capacitors = capture_controls(ckt)
    snap = {
        "iterations": int(sol.Iterations),
        "converged": bool(sol.Converged),
        "v_re": varray[0::2],
        "v_im": varray[1::2],
        "total_power": list(ckt.TotalPower),
        "losses": list(ckt.Losses),
        "transformers": transformers,
        "regcontrols": regcontrols,
        "capacitors": capacitors,
    }
    node_order = list(ckt.YNodeOrder)

    # Daily segment: integrate the meter registers over a full day.
    for cmd in DAILY_COMMANDS:
        d.Text.Command = cmd
    assert ckt.Solution.Converged, "oracle daily segment did not converge"
    m = ckt.Meters
    m.First
    registers = {
        "names": list(m.RegisterNames),
        "values": list(m.RegisterValues),
    }

    out = {
        "schema": SCHEMA,
        "oracle": oracle,
        "master": MASTER_REL,
        "snap_commands": SNAP_COMMANDS,
        "daily_commands": DAILY_COMMANDS,
        "node_order": node_order,
        "snap": snap,
        "registers": registers,
    }
    OUT.write_text(json.dumps(out, indent=1) + "\n")
    print(
        f"wrote {OUT}: {len(node_order)} nodes, {snap['iterations']} iterations, "
        f"{len(regcontrols)} regcontrols, {len(capacitors)} caps, "
        f"{len(transformers)} moved xfmrs, {len(registers['names'])} registers"
    )


if __name__ == "__main__":
    main()
