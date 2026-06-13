"""Generate tests/golden/reliability.json from the pinned oracle (WP6.6).

For each feeder the oracle runs `relcalc` (which aborts with #52902 — caught)
and records the per-bus reliability quantities the backward/forward sweep
populates *before* the abort:

    Bus.Lambda      = BusFltRate
    Bus.N_Customers = BusTotalNumCustomers
    Bus.TotalMiles  = BusTotalMiles
    Bus.SectionID   = BusSectionID

Regenerate ONLY with the pinned versions (tools/golden/PIN.txt):
    python tools/golden/gen_reliability.py
"""
import json
import pathlib

from dss import dss, DSSException

SCHEMA = 1

SCENARIOS = [
    {
        "name": "radial-2section",
        "commands": [
            "New circuit.t basekv=12.47 bus1=src phases=3",
            "New line.l1 bus1=src bus2=b1 length=1 units=mi r1=0.1 x1=0.1 faultrate=0.2 pctperm=80 repair=4",
            "New line.l2 bus1=b1 bus2=b2 length=1 units=mi r1=0.1 x1=0.1 faultrate=0.3 pctperm=90 repair=5",
            "New load.ld1 bus1=b1 phases=3 kv=12.47 kw=100 numcust=10",
            "New load.ld2 bus1=b2 phases=3 kv=12.47 kw=200 numcust=25",
            "New energymeter.m1 element=line.l1 terminal=1",
            "Set voltagebases=[12.47]",
            "CalcVoltageBases",
            "Solve mode=snap",
        ],
        "relcalc_arg": "",
        "buses": ["src", "b1", "b2"],
    },
    {
        "name": "branching-laterals",
        "commands": [
            "New circuit.t basekv=12.47 bus1=src phases=3",
            "New line.l1 bus1=src bus2=b1 length=1 units=mi r1=0.1 x1=0.1 faultrate=0.2 pctperm=80",
            "New line.l2 bus1=b1 bus2=b2 length=1 units=mi r1=0.1 x1=0.1 faultrate=0.3 pctperm=90",
            "New line.l3 bus1=b1 bus2=b3 length=1 units=mi r1=0.1 x1=0.1 faultrate=0.5 pctperm=100",
            "New load.ld1 bus1=b1 phases=3 kv=12.47 kw=100 numcust=10",
            "New load.ld2 bus1=b2 phases=3 kv=12.47 kw=200 numcust=25",
            "New load.ld3 bus1=b3 phases=3 kv=12.47 kw=150 numcust=7",
            "New energymeter.m1 element=line.l1 terminal=1",
            "Set voltagebases=[12.47]",
            "CalcVoltageBases",
            "Solve mode=snap",
        ],
        "relcalc_arg": "",
        "buses": ["src", "b1", "b2", "b3"],
    },
]


def run(sc):
    dss.AllowForms = False
    dss.Text.Command = "clear"
    for cmd in sc["commands"]:
        dss.Text.Command = cmd

    arg = sc["relcalc_arg"]
    cmd = "relcalc" + (f" {arg}" if arg else "")
    error_number = None
    try:
        dss.Text.Command = cmd
    except DSSException as e:
        error_number = int(e.args[0])

    ckt = dss.ActiveCircuit
    buses = []
    for name in sc["buses"]:
        ckt.SetActiveBus(name)
        ab = ckt.ActiveBus
        buses.append({
            "name": name,
            "lambda": ab.Lambda,
            "n_customers": int(ab.N_Customers),
            "total_miles": ab.TotalMiles,
            "section_id": int(ab.SectionID),
        })
    return {
        "name": sc["name"],
        "commands": sc["commands"],
        "relcalc_arg": arg,
        "error_number": error_number,
        "buses": buses,
    }


def main():
    out = {"schema": SCHEMA, "scenarios": [run(sc) for sc in SCENARIOS]}
    path = pathlib.Path(__file__).resolve().parents[2] / "tests" / "golden" / "reliability.json"
    path.write_text(json.dumps(out, indent=2) + "\n")
    print(f"wrote {path}")


if __name__ == "__main__":
    main()
