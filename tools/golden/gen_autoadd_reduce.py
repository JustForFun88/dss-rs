"""Generate tests/golden/autoadd_reduce.json from the pinned oracle (WP6.8 3/4 + 4/4).

Pins the *observable* surface of the AutoAdd option object and the circuit-
reduction option/command surface — i.e. every `Get` echo the dss-python COM API
exposes, plus the error/edge behaviors that the WP6.8 audit hardened:

  - AutoAdd option `Get` formatting (floats via `%-g`, int-arrays via
    `IntArrayToString`, AddType as the lowercase device word);
  - `Set addtype=<unknown>` -> `StringToOrdinal` default (CAPADD), no error;
  - `Set ueregs=(10 abc 13)` -> `parseIntArray` raises on the bad token, the
    array is left zero-filled from there (`[10, 0, 0]`), and a conversion error
    is logged (oracle wraps it as #303);
  - a roundable decimal still rounds (`13.7 -> 14`);
  - Reduce option `Get` echoes (ReductionStrategyString / Zmag / KeepLoad);
  - `Reduce` with no meter -> #1890; `Reduce <missing>` -> #262 (uppercased).

`Bus.Keep` (set by `MarkCapandReactorBuses`) and the reduction itself are NOT
exposed by the COM API, so those stay pinned by the `exec` unit tests.

Regenerate ONLY with the pinned versions (tools/golden/PIN.txt):
    python tools/golden/gen_autoadd_reduce.py
"""
import json
import pathlib

from dss import dss, DSSException

SCHEMA = 1

# Each scenario: replay `commands` (the last may legitimately raise), then read
# back each `Get` query. `error_contains`, when set, is the substring the Rust
# port must surface (and which the generator asserts the oracle really raised).
SCENARIOS = [
    {
        "name": "autoadd-defaults",
        "commands": ["New circuit.c1"],
        "gets": ["genkw genpf capkvar addtype ueweight lossweight ueregs lossregs", "autobuslist"],
        "error_contains": None,
    },
    {
        "name": "autoadd-set",
        "commands": [
            "New circuit.c1",
            "Set genkw=500 genpf=0.95 capkvar=1200 addtype=capacitor "
            "ueweight=2 lossweight=3 ueregs=[1,2,3] lossregs=[13,14]",
        ],
        "gets": ["genkw genpf capkvar addtype ueweight lossweight ueregs lossregs"],
        "error_contains": None,
    },
    {
        "name": "autoadd-buslist",
        "commands": ["New circuit.c1", "Set autobuslist=[b1, b2, b3]"],
        "gets": ["autobuslist"],
        "error_contains": None,
    },
    {
        "name": "addtype-unknown-falls-back",
        "commands": ["New circuit.c1", "Set addtype=foo"],
        "gets": ["addtype"],
        "error_contains": None,
    },
    {
        "name": "ueregs-nonnumeric-token",
        "commands": ["New circuit.c1", "Set ueregs=(10 abc 13)"],
        "gets": ["ueregs"],
        "error_contains": 'Integer number conversion error for string: "abc"',
    },
    {
        "name": "lossregs-decimal-rounds",
        "commands": ["New circuit.c1", "Set lossregs=(13.7 14)"],
        "gets": ["lossregs"],
        "error_contains": None,
    },
    {
        "name": "reduce-option-defaults",
        "commands": ["New circuit.c1"],
        "gets": ["zmag keepload", "reduceoption"],
        "error_contains": None,
    },
    {
        "name": "reduce-option-set",
        "commands": ["New circuit.c1", "Set reduceoption=shortlines zmag=0.05 keepload=no"],
        "gets": ["reduceoption zmag keepload"],
        "error_contains": None,
    },
    {
        "name": "reduce-no-meter-1890",
        "commands": ["New circuit.c1 basekv=12.47 bus1=src phases=3", "reduce"],
        "gets": [],
        "error_contains": "An energy meter is required to use this feature.",
    },
    {
        "name": "reduce-named-meter-not-found-262",
        "commands": [
            "New circuit.c1 basekv=12.47 bus1=src phases=3",
            "New line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6",
            "New energymeter.m1 element=line.l1 terminal=1",
            "Set voltagebases=[12.47]",
            "CalcVoltageBases",
            "Solve",
            "reduce nope",
        ],
        "gets": [],
        "error_contains": 'EnergyMeter "NOPE" not found.',
    },
]


def run(sc):
    dss.AllowForms = False
    dss.Text.Command = "clear"

    error_number = None
    error_message = None
    for cmd in sc["commands"]:
        try:
            dss.Text.Command = cmd
        except DSSException as e:
            # Record the first raise; keep going so the `Get` probes still run.
            if error_number is None:
                error_number = int(e.args[0])
                error_message = e.args[1] if len(e.args) > 1 else ""

    # Sanity: the spec and the oracle must agree on whether an error happened.
    expected = sc["error_contains"]
    if expected is None:
        assert error_number is None, f"{sc['name']}: unexpected oracle error {error_number}: {error_message}"
    else:
        assert error_number is not None, f"{sc['name']}: expected an oracle error, none raised"
        assert expected in error_message, (
            f"{sc['name']}: oracle message {error_message!r} lacks {expected!r}"
        )

    gets = []
    for query in sc["gets"]:
        dss.Text.Command = "Get " + query
        gets.append({"query": query, "result": dss.Text.Result})

    return {
        "name": sc["name"],
        "commands": sc["commands"],
        "gets": gets,
        "error_number": error_number,
        "error_contains": expected,
    }


def main():
    out = {"schema": SCHEMA, "scenarios": [run(sc) for sc in SCENARIOS]}
    path = pathlib.Path(__file__).resolve().parents[2] / "tests" / "golden" / "autoadd_reduce.json"
    path.write_text(json.dumps(out, indent=2) + "\n")
    print(f"wrote {path}")


if __name__ == "__main__":
    main()
