"""Generate tests/golden/allocation.json from the pinned oracle (WP6.7).

For each feeder the oracle runs `allocateloads` and records, per load, the
allocated quantities the EnergyMeter/Sensor allocation loop produces:

    Loads.kW               = kWBase        (after ComputeAllocatedLoad)
    Loads.AllocationFactor = FAllocationFactor

Covers every allocation path: 3-phase ConnectedkVA, kWh/Cfactor spec, unbalanced
single-phase (distinct per-phase PhsAllocationFactor), a current-spec Sensor and
a P/Q (kWs/kvars) Sensor driving a downstream load, plus NumAllocIterations.

Regenerate ONLY with the pinned versions (tools/golden/PIN.txt):
    python tools/golden/gen_allocation.py
"""
import json
import pathlib

from dss import dss, DSSException

SCHEMA = 1

SCENARIOS = [
    {
        # EnergyMeter drives two 3-phase ConnectedkVA loads (default 2 iters).
        "name": "meter-connectedkva",
        "commands": [
            "New circuit.a basekv=12.47 bus1=src phases=3",
            "New line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6",
            "New line.l2 bus1=b1 bus2=b2 length=1 r1=0.3 x1=0.6",
            "New load.ld1 bus1=b1 phases=3 kv=12.47 xfkva=500 allocationfactor=0.5 pf=0.9",
            "New load.ld2 bus1=b2 phases=3 kv=12.47 xfkva=800 allocationfactor=0.5 pf=0.9",
            "New energymeter.m1 element=line.l1 terminal=1",
            "Set voltagebases=[12.47]",
            "CalcVoltageBases",
            "Solve mode=snap",
        ],
        "loads": ["ld1", "ld2"],
    },
    {
        # Same feeder, more allocation iterations (NumAllocIterations).
        "name": "meter-numalloc4",
        "commands": [
            "New circuit.a basekv=12.47 bus1=src phases=3",
            "New line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6",
            "New line.l2 bus1=b1 bus2=b2 length=1 r1=0.3 x1=0.6",
            "New load.ld1 bus1=b1 phases=3 kv=12.47 xfkva=500 allocationfactor=0.5 pf=0.9",
            "New load.ld2 bus1=b2 phases=3 kv=12.47 xfkva=800 allocationfactor=0.5 pf=0.9",
            "New energymeter.m1 element=line.l1 terminal=1",
            "Set voltagebases=[12.47]",
            "CalcVoltageBases",
            "Solve mode=snap",
            "Set NumAllocIterations=4",
        ],
        "loads": ["ld1", "ld2"],
    },
    {
        # kWh / Cfactor spec loads (LoadSpecType = kWh_PF).
        "name": "meter-kwh-spec",
        "commands": [
            "New circuit.a basekv=12.47 bus1=src phases=3",
            "New line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6",
            "New line.l2 bus1=b1 bus2=b2 length=1 r1=0.3 x1=0.6",
            "New load.ld1 bus1=b1 phases=3 kv=12.47 kwh=200000 cfactor=0.3 pf=0.9",
            "New load.ld2 bus1=b2 phases=3 kv=12.47 kwh=350000 cfactor=0.3 pf=0.9",
            "New energymeter.m1 element=line.l1 terminal=1",
            "Set voltagebases=[12.47]",
            "CalcVoltageBases",
            "Solve mode=snap",
        ],
        "loads": ["ld1", "ld2"],
    },
    {
        # Unbalanced single-phase loads → distinct per-phase allocation factors.
        "name": "meter-single-phase",
        "commands": [
            "New circuit.a basekv=12.47 bus1=src phases=3",
            "New line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6",
            "New load.ld1 bus1=b1.1 phases=1 kv=7.2 xfkva=200 allocationfactor=0.5 pf=0.9",
            "New load.ld2 bus1=b1.2 phases=1 kv=7.2 xfkva=400 allocationfactor=0.5 pf=0.9",
            "New load.ld3 bus1=b1.3 phases=1 kv=7.2 xfkva=600 allocationfactor=0.5 pf=0.9",
            "New energymeter.m1 element=line.l1 terminal=1",
            "Set voltagebases=[12.47,7.2]",
            "CalcVoltageBases",
            "Solve mode=snap",
        ],
        "loads": ["ld1", "ld2", "ld3"],
    },
    {
        # A current-spec Sensor on the mid-feeder line gives its downstream load
        # its own target; the meter still drives the upstream load. The measured
        # currents are set in a *separate* edit so they survive ZeroSensorArrays.
        "name": "sensor-currents",
        "commands": [
            "New circuit.a basekv=12.47 bus1=src phases=3",
            "New line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6",
            "New line.l2 bus1=b1 bus2=b2 length=1 r1=0.3 x1=0.6",
            "New load.ld1 bus1=b1 phases=3 kv=12.47 xfkva=500 allocationfactor=0.5 pf=0.9",
            "New load.ld2 bus1=b2 phases=3 kv=12.47 xfkva=800 allocationfactor=0.5 pf=0.9",
            "New energymeter.m1 element=line.l1 terminal=1",
            "New sensor.s1 element=line.l2 terminal=1 kvbase=12.47",
            "Edit sensor.s1 currents=[20,20,20]",
            "Set voltagebases=[12.47]",
            "CalcVoltageBases",
            "Solve mode=snap",
        ],
        "loads": ["ld1", "ld2"],
    },
    {
        # A P/Q (kWs/kvars) Sensor: UpdateCurrentVector converts |S|/Vbase to a
        # per-phase current target that drives the downstream load.
        "name": "sensor-pq",
        "commands": [
            "New circuit.a basekv=12.47 bus1=src phases=3",
            "New line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6",
            "New line.l2 bus1=b1 bus2=b2 length=1 r1=0.3 x1=0.6",
            "New load.ld1 bus1=b1 phases=3 kv=12.47 xfkva=500 allocationfactor=0.5 pf=0.9",
            "New load.ld2 bus1=b2 phases=3 kv=12.47 xfkva=800 allocationfactor=0.5 pf=0.9",
            "New energymeter.m1 element=line.l1 terminal=1",
            "New sensor.s1 element=line.l2 terminal=1 kvbase=12.47",
            "Edit sensor.s1 kWs=[400,400,400] kvars=[200,200,200]",
            "Set voltagebases=[12.47]",
            "CalcVoltageBases",
            "Solve mode=snap",
        ],
        "loads": ["ld1", "ld2"],
    },
]


def run(sc):
    dss.AllowForms = False
    dss.Text.Command = "clear"
    for cmd in sc["commands"]:
        dss.Text.Command = cmd

    error_number = None
    try:
        dss.Text.Command = "allocateloads"
    except DSSException as e:
        error_number = int(e.args[0])

    ckt = dss.ActiveCircuit
    loads = []
    for name in sc["loads"]:
        ckt.Loads.Name = name
        loads.append({
            "name": name,
            "kw": ckt.Loads.kW,
            "allocation_factor": ckt.Loads.AllocationFactor,
        })
    return {
        "name": sc["name"],
        "commands": sc["commands"],
        "error_number": error_number,
        "loads": loads,
    }


def main():
    out = {"schema": SCHEMA, "scenarios": [run(sc) for sc in SCENARIOS]}
    path = pathlib.Path(__file__).resolve().parents[2] / "tests" / "golden" / "allocation.json"
    path.write_text(json.dumps(out, indent=2) + "\n")
    print(f"wrote {path}")


if __name__ == "__main__":
    main()
