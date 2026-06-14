"""Generate the Phase-6 goldens from the pinned oracle (PHASE6_PLAN WP6.9 §1.2).

Command-replay style like the phase5 goldens. Four scenarios exercise the Phase-6
meter/monitor/generator/topology machinery against the oracle; the Rust harness
(`golden_phase6.rs`) replays the identical command lists and must match.

  - monitor_daily_ieee13: IEEE13 (controls active, inlined) + a 24-pt daily
    shape on every load, with four monitors on `line.650632` / `transformer.reg1`
    — mode 0 (V&I), 1 (powers), 2 (tap), 5 (solution vars); 24 daily steps.
    (The plan suggested `line.671680`, but bus 680 is a dead-end stub with no
    load, so that line carries only tiny charging current whose angle is
    noise-dominated; `line.650632` is the feeder head with large, well-
    conditioned current. Settled empirically per the project convention.)
    Per-monitor header strings + SampleCount exact, channel sample arrays
    elementwise at 1e-6 rel (mode-5 wall-clock channels SolveSnap_uSecs /
    TimeStep_uSecs are skipped — the port records 0 for them).
  - meter_daily_ieee13: IEEE13 + the same daily shape + `energymeter.m1` on
    `line.650632`; 24 daily steps. Registers 1e-4 rel + names exact; zone
    branch / end / PCE counts exact.
  - generator_snap: IEEE13 + two generators (model 1 PQ wye + model 3 PV delta);
    snapshot solve. Iteration count + node order exact, voltages 1e-6 rel,
    each generator's terminal powers 1e-6 rel.
  - meter_zone_micro: a hand-built radial with a branch and a mid-feeder
    sub-meter — zone membership (`AllBranchesInZone` / `AllEndElements` /
    `ZonePCE`) exact for both meters (the parent zone stops at the sub-meter).

Usage:
    python tools/golden/gen_phase6.py

Writes one file per scenario under tests/golden/phase6/ (<name>.json). Regeneration
is manual and must use the exact versions in tools/golden/PIN.txt.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from gen_phase5 import IEEE13, IEEE13_LOADS, DAY_CURVE  # noqa: E402

REPO_ROOT = Path(__file__).resolve().parents[2]
OUT_DIR = REPO_ROOT / "tests" / "golden" / "phase6"
SCHEMA = 1


def check_pin() -> dict:
    import dss

    if dss.__version__ != "0.15.7":
        sys.exit(f"dss-python {dss.__version__} != pinned 0.15.7 (tools/golden/PIN.txt)")
    from dss import DSS

    return {"dss_python": dss.__version__, "engine": DSS.Version}


def ieee13_daily() -> list[str]:
    """IEEE13 (controls active) + a 24-pt daily shape on every load."""
    cmds = list(IEEE13)
    cmds.append(f"New loadshape.day npts=24 interval=1 mult={DAY_CURVE}")
    for name in IEEE13_LOADS:
        cmds.append(f"Load.{name}.daily=day")
    return cmds


# ---------------------------------------------------------------------------
# Scenario builders: each returns the command list and (after replay) the
# generator captures the oracle outputs the scenario pins.
# ---------------------------------------------------------------------------


def scenario_monitor_daily(d) -> dict:
    cmds = ieee13_daily()
    cmds += [
        "New monitor.m0 element=line.650632 terminal=1 mode=0",
        "New monitor.m1 element=line.650632 terminal=1 mode=1",
        "New monitor.m2 element=transformer.reg1 terminal=2 mode=2",
        "New monitor.m5 element=line.650632 terminal=1 mode=5",
        "set mode=daily stepsize=1h number=24",
    ]
    d.Text.Command = "clear"
    for c in cmds:
        d.Text.Command = c
    d.Text.Command = "solve"

    mon = d.ActiveCircuit.Monitors
    monitors = []
    for nm in ("m0", "m1", "m2", "m5"):
        mon.Name = nm
        header = list(mon.Header)
        nch = mon.NumChannels
        channels = [[float(x) for x in mon.Channel(i)] for i in range(1, nch + 1)]
        # Mode-5 skipped channels (0-based): only the wall-clock timings
        #   10/11 SolveSnap_uSecs / TimeStep_uSecs — the port records 0 for them.
        # The iteration-count channels (0/1 TotalIterations / ControlIteration)
        # are NOT skipped: with the load-Yeq restamp fix in build_y_matrix the
        # per-step iteration counts now match the oracle exactly over the daily
        # trajectory (they used to drift ±1 at the 1e-4 convergence tolerance).
        skip = [10, 11] if nm == "m5" else []
        # Those two channels are real microsecond wall-clock timings — they vary
        # every run. The harness skips them, so zero them in the golden too,
        # otherwise every regeneration churns the file with timing noise.
        for ch in skip:
            channels[ch] = [0.0] * len(channels[ch])
        monitors.append(
            {
                "name": nm,
                "header": header,
                "sample_count": int(mon.SampleCount),
                "channels": channels,
                "skip_channels": skip,
            }
        )
    return {"name": "monitor_daily_ieee13", "commands": cmds, "monitors": monitors}


def scenario_meter_daily(d) -> dict:
    cmds = ieee13_daily()
    cmds += [
        "New energymeter.m1 element=line.650632 terminal=1",
        "set mode=daily stepsize=1h number=24",
    ]
    d.Text.Command = "clear"
    for c in cmds:
        d.Text.Command = c
    d.Text.Command = "solve"

    ckt = d.ActiveCircuit
    m = ckt.Meters
    m.First
    meter = {
        "name": "m1",
        "register_names": list(m.RegisterNames),
        "register_values": list(m.RegisterValues),
        "n_branches": len(list(m.AllBranchesInZone)),
        "n_ends": len(list(m.AllEndElements)),
        "n_pce": len(list(m.ZonePCE)),
    }
    return {"name": "meter_daily_ieee13", "commands": cmds, "meters": [meter]}


def scenario_generator_snap(d) -> dict:
    cmds = list(IEEE13)
    cmds += [
        "New generator.g1 bus1=675 phases=3 kv=4.16 kw=500 pf=0.95 model=1 conn=wye",
        "New generator.g2 bus1=634 phases=3 kv=0.48 kw=100 model=3 vpu=1.0 conn=delta",
    ]
    d.Text.Command = "clear"
    for c in cmds:
        d.Text.Command = c
    d.Text.Command = "solve"

    ckt = d.ActiveCircuit
    sol = ckt.Solution
    varray = list(ckt.YNodeVarray)
    generators = []
    for nm in ("Generator.g1", "Generator.g2"):
        ckt.SetActiveElement(nm)
        generators.append({"name": nm, "powers": list(ckt.ActiveElement.Powers)})
    return {
        "name": "generator_snap",
        "commands": cmds,
        "iterations": int(sol.Iterations),
        "converged": bool(sol.Converged),
        "node_order": list(ckt.YNodeOrder),
        "v_re": varray[0::2],
        "v_im": varray[1::2],
        "generators": generators,
    }


MICRO = [
    "new circuit.micro basekv=12.47 phases=3 bus1=src",
    "new line.l1 bus1=src bus2=b1 length=1 r1=0.1 x1=0.2 c1=0 c0=0 r0=0.1 x0=0.2",
    "new line.l2 bus1=b1 bus2=b2 length=1 r1=0.1 x1=0.2 c1=0 c0=0 r0=0.1 x0=0.2",
    "new line.l3 bus1=b2 bus2=b3 length=1 r1=0.1 x1=0.2 c1=0 c0=0 r0=0.1 x0=0.2",
    "new line.l4 bus1=b1 bus2=b4 length=1 r1=0.1 x1=0.2 c1=0 c0=0 r0=0.1 x0=0.2",
    "new load.ld3 bus1=b3 phases=3 kv=12.47 kw=100 model=1",
    "new load.ld4 bus1=b4 phases=3 kv=12.47 kw=100 model=1",
    "new energymeter.m1 element=line.l1 terminal=1",
    "new energymeter.m2 element=line.l2 terminal=1",
    "set voltagebases=[12.47]",
    "calcvoltagebases",
]


def scenario_meter_zone_micro(d) -> dict:
    cmds = list(MICRO)
    d.Text.Command = "clear"
    for c in cmds:
        d.Text.Command = c
    d.Text.Command = "solve"

    m = d.ActiveCircuit.Meters
    zones = []
    for nm in ("m1", "m2"):
        m.Name = nm
        zones.append(
            {
                "name": nm,
                "branches": list(m.AllBranchesInZone),
                "ends": list(m.AllEndElements),
                "pce": list(m.ZonePCE),
            }
        )
    return {"name": "meter_zone_micro", "commands": cmds, "meter_zones": zones}


def main() -> None:
    oracle = check_pin()
    from dss import DSS as d

    scenarios = [
        scenario_monitor_daily(d),
        scenario_meter_daily(d),
        scenario_generator_snap(d),
        scenario_meter_zone_micro(d),
    ]
    # One file per scenario under OUT_DIR; golden_phase6.rs runs every *.json in
    # the directory, so adding a scenario is just dropping a new file.
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    names = {sc["name"] for sc in scenarios}
    for sc in scenarios:
        path = OUT_DIR / f"{sc['name']}.json"
        path.write_text(
            json.dumps({"schema": SCHEMA, "oracle": oracle, "scenario": sc}, indent=1) + "\n"
        )
        print(f"wrote {path.relative_to(REPO_ROOT)}")
    for p in OUT_DIR.glob("*.json"):
        if p.stem not in names:
            p.unlink()
            print(f"removed stale {p.relative_to(REPO_ROOT)}")


if __name__ == "__main__":
    main()
