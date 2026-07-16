"""Regenerate the RegControl property golden against the **capi015** engine.

RegControl's property surface changed at dss_capi 0.15.x r4086 (commit
`8a898cba`, WP-U1.6 C5): the `RevThreshold` default flipped to −100 kW (signed
W field), a `FwdThreshold` (+100 kW) plus three idle-zone flags were added, and
the legacy fallback moved into `EndEdit`. The default-oracle (0.14.5) props
golden therefore no longer matches the port — this generator captures the
`regcontrol_*` scenarios (defined in `gen_props.py`) from the 0.15.x-line
oracle instead, exactly like the linespacing equivalent-spacing golden.

Run with the Oddie venv python (dss-python 0.16.0b2, backend 0.15.0b4 = r4103),
which the `check_pin(DSS_ORACLE_ENGINE=capi015)` gate in gen_checkpoints.py
validates:

    DSS_ORACLE_ENGINE=capi015 tools/opendss/.venv/Scripts/python.exe \
        tools/golden/gen_regcontrol_capi015.py

Writes tests/golden/props/regcontrol.json (engine_spec="capi015").
"""

from __future__ import annotations

import json
from pathlib import Path

from gen_checkpoints import check_pin
from gen_props import SCENARIOS, run_scenario

REPO_ROOT = Path(__file__).resolve().parents[2]
OUT = REPO_ROOT / "tests" / "golden" / "props" / "regcontrol.json"
SCHEMA = 1


def main() -> None:
    prov = check_pin()  # asserts capi015 engine/backend, fails loudly otherwise
    if prov.get("engine_spec") != "capi015":
        raise SystemExit("run with DSS_ORACLE_ENGINE=capi015 (the Oddie venv python)")
    from dss import dss as d

    scenarios = [
        run_scenario(d, s) for s in SCENARIOS if s["name"].split("_", 1)[0] == "regcontrol"
    ]
    oracle = {
        "engine_spec": "capi015",
        "engine": prov["engine"],
        "note": (
            "WP-U1.6 C5 RegControl 0.15.x surface (r4086, 8a898cba): signed "
            "RevThreshold default −100 kW, FwdThreshold, Idle/IdleReverse/"
            "IdleForward, EndEdit legacy band. Probed on capi015."
        ),
    }
    OUT.write_text(
        json.dumps(
            {"schema": SCHEMA, "oracle": oracle, "class": "regcontrol", "scenarios": scenarios},
            indent=1,
        )
        + "\n"
    )
    print(f"wrote {OUT.relative_to(REPO_ROOT)} ({len(scenarios)} scenarios)")


if __name__ == "__main__":
    main()
