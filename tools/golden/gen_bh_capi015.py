"""Pin the default BH-curve props (Transformer/AutoTrans) against **capi015**.

dss_capi 0.15.x r4064 (commit `90962ae8`, WP-U1.6 C6) adds three `Unused`
GICharm data props — `BHpoints`/`BHcurrent`/`BHflux` — to both transformer
classes. Only the DEFAULT state is oracle-probable: parsing a non-empty
`BHcurrent`/`BHflux` **segfaults** the capi015 backend (an upstream crash in the
`Unused` DoubleVArray parse path — UB, not reproduced; the port parses safely
and is pinned for the set-state by the Rust unit tests
`transformer::tests::bh_curve_props_parse_and_store` + the AutoTrans sibling).

So this golden pins just the empty default (`BHpoints=0`, arrays ''), proving
the props exist with the right names/order/defaults on both classes. Run with
the Oddie venv python:

    DSS_ORACLE_ENGINE=capi015 tools/opendss/.venv/Scripts/python.exe \
        tools/golden/gen_bh_capi015.py

Writes tests/golden/props/transformer_bh.json + autotrans_bh.json.
"""

from __future__ import annotations

import json
from pathlib import Path

from gen_checkpoints import check_pin
from gen_props import run_scenario

REPO_ROOT = Path(__file__).resolve().parents[2]
OUT_DIR = REPO_ROOT / "tests" / "golden" / "props"
SCHEMA = 1

XF = (
    "New Transformer.t1 phases=3 windings=2 buses=(a.1.2.3, b.1.2.3) "
    "kvs=(115 12.47) kvas=(1000 1000) xhl=5"
)
AT = (
    "New AutoTrans.a1 phases=3 windings=2 buses=(a.1.2.3, b.1.2.3, b.0) "
    "kvs=(115 12.47) kvas=(1000 1000) xhx=5"
)

SCENARIOS = {
    "transformer_bh": {
        "name": "transformerbh_default",
        "target": "Transformer.t1",
        "commands": [XF],
        "skip_props": [],  # only the three BH props are captured below
    },
    "autotrans_bh": {
        "name": "autotransbh_default",
        "target": "AutoTrans.a1",
        "commands": [AT],
        "skip_props": [],
    },
}
BH = ["BHpoints", "BHcurrent", "BHflux"]


def main() -> None:
    prov = check_pin()
    if prov.get("engine_spec") != "capi015":
        raise SystemExit("run with DSS_ORACLE_ENGINE=capi015 (the Oddie venv python)")
    from dss import dss as d

    oracle = {
        "engine_spec": "capi015",
        "engine": prov["engine"],
        "note": (
            "WP-U1.6 C6 default BH-curve props (r4064, 90962ae8). Only the empty "
            "default is oracle-probable (a non-empty BHcurrent parse segfaults the "
            "capi015 backend); the set-state is pinned by Rust unit tests."
        ),
    }
    for cls, sc in SCENARIOS.items():
        full = run_scenario(d, sc)
        # Keep only the three BH props (the rest are 0.14.5-identical, pinned by
        # the class's own default-oracle golden). Match the oracle's canonical
        # case (AllPropertyNames), then relabel to the porting-plan casing.
        by_lower = {k.lower(): (k, v) for k, v in full["properties"].items()}
        full["properties"] = {name: by_lower[name.lower()][1] for name in BH}
        OUT_DIR.joinpath(f"{cls}.json").write_text(
            json.dumps(
                {"schema": SCHEMA, "oracle": oracle, "class": cls, "scenarios": [full]},
                indent=1,
            )
            + "\n"
        )
        print(f"wrote {cls}.json: {full['properties']}")


if __name__ == "__main__":
    main()
