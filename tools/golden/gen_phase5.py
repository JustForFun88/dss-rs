"""Generate the Phase 5 time-series/control golden from dss-python (oracle).

Command-replay style like slice.json (PHASE5_PLAN.md WP5.9): each scenario
stores its full command list, the per-`solve`-step captures (dblHour,
iterations, node voltages), the final control state (transformer taps,
RegControl tap numbers, capacitor states) and the event log. The Rust engine
replays the identical commands and must match:

  - per-step voltages at 1e-6 rel, iterations and dblHour exactly,
  - final taps / tap numbers / capacitor states exactly,
  - the event log normalized (numeric-skeleton comparison per line).

Scenarios:
  - daily_ieee13:    IEEE13 (controls active, commands inlined — the
                     IEEELineCodes redirect is dropped because the master only
                     uses the inline mtx601..607 codes) + a 24-point daily
                     shape on every load, `mode=daily`, 24 hourly steps.
  - duty_2bus:       the Phase 3 two-bus circuit + a 12-point 300 s duty
                     shape, `mode=duty` (TIMEDRIVEN control), 12 steps.
  - eventlog_ieee13: IEEE13 with `Set Log=yes` (ckt.LogEvents) + one solve.
  - capcontrol_micro:IEEE13 + a kvar CapControl that opens Cap1 (probed: the
                     500/300 on/off settings toggle the bank), one solve.

Usage:
    python tools/golden/gen_phase5.py

Writes one file per scenario under tests/golden/phase5/ (<name>.json).

Regeneration is manual and must use the exact versions in tools/golden/PIN.txt.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
OUT_DIR = REPO_ROOT / "tests" / "golden" / "phase5"
SCHEMA = 1

# The unmodified IEEE13 master, inlined (Clear/Redirect/Solve/BusCoords
# stripped; the runner issues `clear` itself and the scenarios append their
# own settings + solves). Controls stay ACTIVE.
IEEE13 = [
    "Set DefaultBaseFrequency=60",
    "new circuit.IEEE13Nodeckt",
    "~ basekv=115 pu=1.0001 phases=3 bus1=SourceBus",
    "~ Angle=30",
    "~ MVAsc3=20000 MVASC1=21000",
    "New Transformer.Sub Phases=3 Windings=2   XHL=(8 1000 /)",
    "~ wdg=1 bus=SourceBus   conn=delta  kv=115  kva=5000   %r=(.5 1000 /)",
    "~ wdg=2 bus=650             conn=wye    kv=4.16  kva=5000   %r=(.5 1000 /)",
    "New Transformer.Reg1 phases=1 bank=reg1 XHL=0.01 kVAs=[1666 1666]",
    "~ Buses=[650.1 RG60.1] kVs=[2.4  2.4] %LoadLoss=0.01",
    "new regcontrol.Reg1  transformer=Reg1 winding=2  vreg=122  band=2  ptratio=20 ctprim=700  R=3   X=9",
    "New Transformer.Reg2 phases=1 bank=reg1 XHL=0.01 kVAs=[1666 1666]",
    "~ Buses=[650.2 RG60.2] kVs=[2.4  2.4] %LoadLoss=0.01",
    "new regcontrol.Reg2  transformer=Reg2 winding=2  vreg=122  band=2  ptratio=20 ctprim=700  R=3   X=9",
    "New Transformer.Reg3 phases=1 bank=reg1 XHL=0.01 kVAs=[1666 1666]",
    "~ Buses=[650.3 RG60.3] kVs=[2.4  2.4] %LoadLoss=0.01",
    "new regcontrol.Reg3  transformer=Reg3 winding=2  vreg=122  band=2  ptratio=20 ctprim=700  R=3   X=9",
    "New Transformer.XFM1  Phases=3   Windings=2  XHL=2",
    "~ wdg=1 bus=633       conn=Wye kv=4.16    kva=500    %r=.55",
    "~ wdg=2 bus=634       conn=Wye kv=0.480    kva=500    %r=.55",
    "New linecode.mtx601 nphases=3 BaseFreq=60",
    "~ rmatrix = (0.3465 | 0.1560 0.3375 | 0.1580 0.1535 0.3414 )",
    "~ xmatrix = (1.0179 | 0.5017 1.0478 | 0.4236 0.3849 1.0348 )",
    "~ units=mi",
    "New linecode.mtx602 nphases=3 BaseFreq=60",
    "~ rmatrix = (0.7526 | 0.1580 0.7475 | 0.1560 0.1535 0.7436 )",
    "~ xmatrix = (1.1814 | 0.4236 1.1983 | 0.5017 0.3849 1.2112 )",
    "~ units=mi",
    "New linecode.mtx603 nphases=2 BaseFreq=60",
    "~ rmatrix = (1.3238 | 0.2066 1.3294 )",
    "~ xmatrix = (1.3569 | 0.4591 1.3471 )",
    "~ units=mi",
    "New linecode.mtx604 nphases=2 BaseFreq=60",
    "~ rmatrix = (1.3238 | 0.2066 1.3294 )",
    "~ xmatrix = (1.3569 | 0.4591 1.3471 )",
    "~ units=mi",
    "New linecode.mtx605 nphases=1 BaseFreq=60",
    "~ rmatrix = (1.3292 )",
    "~ xmatrix = (1.3475 )",
    "~ units=mi",
    "New Linecode.mtx606 nphases=3  Units=mi",
    "~ Rmatrix=[0.791721  |0.318476  0.781649  |0.28345  0.318476  0.791721  ]",
    "~ Xmatrix=[0.438352  |0.0276838  0.396697  |-0.0184204  0.0276838  0.438352  ]",
    "~ Cmatrix=[383.948  |0  383.948  |0  0  383.948  ]",
    "New linecode.mtx607 nphases=1 BaseFreq=60",
    "~ rmatrix = (1.3425 )",
    "~ xmatrix = (0.5124 )",
    "~ cmatrix = [236]",
    "~ units=mi",
    "New Load.671 Bus1=671.1.2.3  Phases=3 Conn=Delta Model=1 kV=4.16   kW=1155 kvar=660",
    "New Load.634a Bus1=634.1     Phases=1 Conn=Wye  Model=1 kV=0.277  kW=160   kvar=110",
    "New Load.634b Bus1=634.2     Phases=1 Conn=Wye  Model=1 kV=0.277  kW=120   kvar=90",
    "New Load.634c Bus1=634.3     Phases=1 Conn=Wye  Model=1 kV=0.277  kW=120   kvar=90",
    "New Load.645 Bus1=645.2       Phases=1 Conn=Wye  Model=1 kV=2.4      kW=170   kvar=125",
    "New Load.646 Bus1=646.2.3    Phases=1 Conn=Delta Model=2 kV=4.16    kW=230   kvar=132",
    "New Load.692 Bus1=692.3.1    Phases=1 Conn=Delta Model=5 kV=4.16    kW=170   kvar=151",
    "New Load.675a Bus1=675.1    Phases=1 Conn=Wye  Model=1 kV=2.4  kW=485   kvar=190",
    "New Load.675b Bus1=675.2    Phases=1 Conn=Wye  Model=1 kV=2.4  kW=68   kvar=60",
    "New Load.675c Bus1=675.3    Phases=1 Conn=Wye  Model=1 kV=2.4  kW=290   kvar=212",
    "New Load.611 Bus1=611.3      Phases=1 Conn=Wye  Model=5 kV=2.4  kW=170   kvar=80",
    "New Load.652 Bus1=652.1      Phases=1 Conn=Wye  Model=2 kV=2.4  kW=128   kvar=86",
    "New Load.670a Bus1=670.1    Phases=1 Conn=Wye  Model=1 kV=2.4  kW=17    kvar=10",
    "New Load.670b Bus1=670.2    Phases=1 Conn=Wye  Model=1 kV=2.4  kW=66    kvar=38",
    "New Load.670c Bus1=670.3    Phases=1 Conn=Wye  Model=1 kV=2.4  kW=117  kvar=68",
    "New Capacitor.Cap1 Bus1=675 phases=3 kVAR=600 kV=4.16",
    "New Capacitor.Cap2 Bus1=611.3 phases=1 kVAR=100 kV=2.4",
    "New Line.650632    Phases=3 Bus1=RG60.1.2.3   Bus2=632.1.2.3  LineCode=mtx601 Length=2000 units=ft",
    "New Line.632670    Phases=3 Bus1=632.1.2.3    Bus2=670.1.2.3  LineCode=mtx601 Length=667  units=ft",
    "New Line.670671    Phases=3 Bus1=670.1.2.3    Bus2=671.1.2.3  LineCode=mtx601 Length=1333 units=ft",
    "New Line.671680    Phases=3 Bus1=671.1.2.3    Bus2=680.1.2.3  LineCode=mtx601 Length=1000 units=ft",
    "New Line.632633    Phases=3 Bus1=632.1.2.3    Bus2=633.1.2.3  LineCode=mtx602 Length=500  units=ft",
    "New Line.632645    Phases=2 Bus1=632.3.2      Bus2=645.3.2    LineCode=mtx603 Length=500  units=ft",
    "New Line.645646    Phases=2 Bus1=645.3.2      Bus2=646.3.2    LineCode=mtx603 Length=300  units=ft",
    "New Line.692675    Phases=3 Bus1=692.1.2.3    Bus2=675.1.2.3  LineCode=mtx606 Length=500  units=ft",
    "New Line.671684    Phases=2 Bus1=671.1.3      Bus2=684.1.3    LineCode=mtx604 Length=300  units=ft",
    "New Line.684611    Phases=1 Bus1=684.3        Bus2=611.3      LineCode=mtx605 Length=300  units=ft",
    "New Line.684652    Phases=1 Bus1=684.1        Bus2=652.1      LineCode=mtx607 Length=800  units=ft",
    "New Line.671692    Phases=3 Bus1=671   Bus2=692  Switch=y  r1=1e-4 r0=1e-4 x1=0.000 x0=0.000 c1=0.000 c0=0.000",
    "Set Voltagebases=[115, 4.16, .48]",
    "calcv",
]

IEEE13_LOADS = [
    "671", "634a", "634b", "634c", "645", "646", "692",
    "675a", "675b", "675c", "611", "652", "670a", "670b", "670c",
]

DAY_CURVE = (
    "(.30 .29 .28 .27 .27 .30 .36 .48 .60 .72 .85 .95 "
    "1.05 1.10 1.08 1.02 .96 .94 1.00 1.05 .95 .75 .55 .40)"
)

DUTY_CURVE = "(.8 .9 1.1 .75 .95 1.2 .6 1.05 .85 1.15 .7 1.0)"


def scenario_daily_ieee13() -> dict:
    cmds = list(IEEE13)
    cmds.append(f"New loadshape.day npts=24 interval=1 mult={DAY_CURVE}")
    for name in IEEE13_LOADS:
        cmds.append(f"Load.{name}.daily=day")
    # Per-control event logging (default off) so tap changes land in the log.
    for reg in ("reg1", "reg2", "reg3"):
        cmds.append(f"RegControl.{reg}.eventlog=yes")
    cmds.append("set mode=daily stepsize=1h number=1")
    # Per-step tolerance 1e-6 (same as the snapshot gate). The daily trajectory
    # tracks the oracle to ~1e-9 once build_y_matrix restamps each load's
    # shape-scaled Yeq per step; the whole discrete trajectory — per-step
    # iteration counts, dblHour, every tap change in the event log, final
    # taps/tap numbers — matches exactly too. (Was 1e-5 with a ±1-iteration
    # allowance while the Yeq accelerator was frozen at the first step's load.)
    return {"name": "daily_ieee13", "tol": 1e-6, "commands": cmds, "n_steps": 24}


def scenario_duty_2bus() -> dict:
    cmds = [
        "Set DefaultBaseFrequency=60",
        "new circuit.twobus basekv=12.47 pu=1.0 phases=3 bus1=sourcebus "
        "mvasc3=2000 mvasc1=2100",
        "new line.l1 bus1=sourcebus bus2=loadbus length=1 units=km "
        "r1=0.1 x1=0.4 r0=0.3 x0=1.2 c1=3.4 c0=1.6",
        f"new loadshape.dty npts=12 sinterval=300 mult={DUTY_CURVE}",
        "new load.ld1 bus1=loadbus phases=3 kv=12.47 kw=4000 kvar=1800 "
        "model=1 duty=dty",
        "Set voltagebases=[12.47]",
        "calcvoltagebases",
        "set mode=duty",
        "set number=1 stepsize=300",
    ]
    return {"name": "duty_2bus", "tol": 1e-6, "commands": cmds, "n_steps": 12}


def scenario_eventlog_ieee13() -> dict:
    cmds = list(IEEE13)
    cmds.append("set log=yes")  # ckt.LogEvents (TExecOption 'Log')
    return {"name": "eventlog_ieee13", "tol": 1e-6, "commands": cmds, "n_steps": 1}


def scenario_capcontrol_micro() -> dict:
    cmds = list(IEEE13)
    # Probed (see module docstring): kvar control on line.692675 with
    # on=500/off=300 opens Cap1 during the snapshot solve.
    cmds.append(
        "New capcontrol.cc element=line.692675 terminal=1 capacitor=cap1 "
        "type=kvar onsetting=500 offsetting=300 eventlog=yes delay=0 delayoff=0"
    )
    return {"name": "capcontrol_micro", "tol": 1e-6, "commands": cmds, "n_steps": 1}


def check_pin() -> dict:
    import dss

    if dss.__version__ != "0.15.7":
        sys.exit(f"dss-python {dss.__version__} != pinned 0.15.7 (tools/golden/PIN.txt)")
    from dss import DSS

    return {"dss_python": dss.__version__, "engine": DSS.Version}


def run_scenario(d, scenario: dict) -> dict:
    d.Text.Command = "clear"
    for cmd in scenario["commands"]:
        d.Text.Command = cmd

    steps = []
    for _ in range(scenario["n_steps"]):
        d.Text.Command = "solve"
        varray = list(d.ActiveCircuit.YNodeVarray)
        sol = d.ActiveCircuit.Solution
        steps.append(
            {
                "dbl_hour": float(sol.dblHour),
                "iterations": int(sol.Iterations),
                "converged": bool(sol.Converged),
                "v_re": varray[0::2],
                "v_im": varray[1::2],
            }
        )

    ckt = d.ActiveCircuit
    transformers = {}
    tr = ckt.Transformers
    i = tr.First
    while i:
        taps = []
        for w in range(1, tr.NumWindings + 1):
            tr.Wdg = w
            taps.append(float(tr.Tap))
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

    return {
        "name": scenario["name"],
        "tol": scenario["tol"],
        "commands": scenario["commands"],
        "n_steps": scenario["n_steps"],
        "node_order": list(ckt.YNodeOrder),
        "steps": steps,
        "transformers": transformers,
        "regcontrols": regcontrols,
        "capacitors": capacitors,
        "event_log": list(ckt.Solution.EventLog),
    }


def main() -> None:
    oracle = check_pin()
    from dss import DSS

    scenarios = [
        scenario_daily_ieee13(),
        scenario_duty_2bus(),
        scenario_eventlog_ieee13(),
        scenario_capcontrol_micro(),
    ]
    results = [run_scenario(DSS, sc) for sc in scenarios]

    # One file per scenario under OUT_DIR; golden_phase5.rs runs every *.json in
    # the directory, so adding a scenario is just dropping a new file.
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    names = {sc["name"] for sc in results}
    for sc in results:
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
