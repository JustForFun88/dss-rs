"""Generate tests/golden/gendispatcher.json from the pinned oracle (WP6.8).

For each scenario the oracle builds a small circuit with a GenDispatcher, runs a
snapshot solve (which iterates the control loop to convergence), and records the
resulting dispatched `kWBase`/`kvarBase` of every generator the dispatcher
touches:

    Generators.kW   = Gen.kWBase
    Generators.kvar = Gen.kvarBase

This pins the full redispatch feedback path (monitored-power read → weighted
PDiff/QDiff share → Max(1.0,…)/Max(0.0,…) floors → LoadsNeedUpdating re-solve),
end to end, against dss-python — complementary to the per-arithmetic unit tests
in `elements::control::gen_dispatcher`.

Regenerate ONLY with the pinned versions (tools/golden/PIN.txt):
    python tools/golden/gen_gendispatcher.py
"""
import json
import pathlib

from dss import dss

SCHEMA = 1

# A shared two-bus feeder (line → load + two generators at the load bus); each
# scenario only varies the generator power factor and the GenDispatcher props.
def feeder(gen_pf, gd_props):
    return [
        "New circuit.a basekv=12.47 bus1=src phases=3",
        "New line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6",
        "New load.ld1 bus1=b1 phases=3 kv=12.47 kw=5000 pf=0.95",
        f"New generator.g1 bus1=b1 phases=3 kv=12.47 kw=1000 pf={gen_pf} model=1",
        f"New generator.g2 bus1=b1 phases=3 kv=12.47 kw=1000 pf={gen_pf} model=1",
        f"New gendispatcher.gd1 element=line.l1 terminal=1 {gd_props}",
        "Set voltagebases=[12.47]",
        "CalcVoltageBases",
        "Solve mode=snap",
    ]


SCENARIOS = [
    {
        "name": "equal-weights",
        "commands": feeder("1.0", "kwlimit=2000 kwband=100 genlist=[g1,g2] weights=[1,1]"),
        "generators": ["g1", "g2"],
    },
    {
        "name": "weighted-3-1",
        "commands": feeder("1.0", "kwlimit=2000 kwband=100 genlist=[g1,g2] weights=[3,1]"),
        "generators": ["g1", "g2"],
    },
    {
        "name": "no-genlist-all-gens",
        "commands": feeder("1.0", "kwlimit=2000 kwband=100"),
        "generators": ["g1", "g2"],
    },
    {
        "name": "kvar-redispatch",
        "commands": feeder(
            "0.95", "kwlimit=2000 kwband=100 kvarlimit=500 genlist=[g1,g2] weights=[1,1]"
        ),
        "generators": ["g1", "g2"],
    },
    {
        "name": "monitored-terminal-2",
        # terminal=2 overrides the default terminal=1 (last-wins); terminal 2 sits
        # at the load/gen bus, so PDiff is strongly negative → gens floor at 1.0.
        "commands": feeder(
            "1.0", "kwlimit=2000 kwband=100 terminal=2 genlist=[g1,g2] weights=[1,1]"
        ),
        "generators": ["g1", "g2"],
    },
]


def run(sc):
    dss.AllowForms = False
    dss.Text.Command = "clear"
    for cmd in sc["commands"]:
        dss.Text.Command = cmd

    # Build a name -> (kW, kvar) map of every generator in the circuit.
    g = dss.ActiveCircuit.Generators
    table = {}
    g.First
    while True:
        table[g.Name.lower()] = (g.kW, g.kvar)
        if g.Next == 0:
            break

    gens = []
    for name in sc["generators"]:
        kw, kvar = table[name.lower()]
        gens.append({"name": name, "kw": kw, "kvar": kvar})

    return {
        "name": sc["name"],
        "commands": sc["commands"],
        "generators": gens,
    }


def main():
    out = {"schema": SCHEMA, "scenarios": [run(sc) for sc in SCENARIOS]}
    path = pathlib.Path(__file__).resolve().parents[2] / "tests" / "golden" / "gendispatcher.json"
    path.write_text(json.dumps(out, indent=2) + "\n")
    print(f"wrote {path}")


if __name__ == "__main__":
    main()
