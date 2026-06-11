"""Generate golden reference JSON for the Rust port, using dss-python as oracle.

dss-python wraps the very same dss_capi engine whose Pascal source lives in
.inputs/dss_capi, so its outputs are the ground truth the Rust engine must
reproduce (see PORTING_PLAN.md, Phase 0 / Testing Strategy).

Usage:
    python tools/golden/generate.py [case-name ...]   # default: all cases

Reads  tools/golden/cases.json
Writes tests/golden/<name>.json

Regeneration is manual and must use the exact versions in tools/golden/PIN.txt.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
TST_ROOT = REPO_ROOT / ".inputs" / "electricdss-tst"
OUT_DIR = REPO_ROOT / "tests" / "golden"
SCHEMA = 1


def _numpy_to_py(o):
    """dss-python returns numpy scalars/arrays; make them JSON-serializable."""
    if hasattr(o, "item") and not hasattr(o, "__len__"):
        return o.item()
    if hasattr(o, "tolist"):
        return o.tolist()
    raise TypeError(f"not JSON serializable: {type(o)}")


def check_pin() -> str:
    import dss

    pins = {}
    for line in (REPO_ROOT / "tools" / "golden" / "PIN.txt").read_text().splitlines():
        line = line.strip()
        if line and not line.startswith("#"):
            k, v = line.split("==")
            pins[k] = v
    if dss.__version__ != pins["dss-python"]:
        sys.exit(
            f"dss-python {dss.__version__} != pinned {pins['dss-python']}; "
            f"install the pinned version or update PIN.txt deliberately"
        )
    return dss.__version__


def dump_transformers(ckt) -> dict:
    out = {}
    tr = ckt.Transformers
    idx = tr.First
    while idx:
        taps = []
        for w in range(1, tr.NumWindings + 1):
            tr.Wdg = w
            taps.append(tr.Tap)
        out[tr.Name] = {"taps": taps}
        idx = tr.Next
    return out


def dump_regcontrols(ckt) -> dict:
    out = {}
    rc = ckt.RegControls
    idx = rc.First
    while idx:
        out[rc.Name] = {"tap_number": rc.TapNumber}
        idx = rc.Next
    return out


def dump_capacitors(ckt) -> dict:
    out = {}
    cap = ckt.Capacitors
    idx = cap.First
    while idx:
        out[cap.Name] = {"states": list(cap.States)}
        idx = cap.Next
    return out


def dump_elements(ckt, with_props: bool) -> dict:
    out = {}
    for name in ckt.AllElementNames:
        ckt.SetActiveElement(name)
        el = ckt.ActiveCktElement
        rec = {
            "enabled": bool(el.Enabled),
            "bus_names": list(el.BusNames),
            "powers": list(el.Powers),      # kW/kvar interleaved, per conductor/terminal
            "currents": list(el.Currents),  # A, re/im interleaved
        }
        if with_props:
            rec["properties"] = {p: el.Properties(p).Val for p in el.AllPropertyNames}
        out[name] = rec
    return out


def run_case(d, case: dict, oracle_version: str) -> dict:
    master = TST_ROOT / case["dir"] / case["master"]
    if not master.exists():
        sys.exit(f"master not found: {master}")

    d.Text.Command = "clear"
    d.Text.Command = f'compile "{master}"'
    for cmd in case.get("post", []):
        d.Text.Command = cmd

    ckt = d.ActiveCircuit
    sol = ckt.Solution
    return {
        "schema": SCHEMA,
        "name": case["name"],
        "master": (Path(case["dir"]) / case["master"]).as_posix(),
        "post": case.get("post", []),
        "oracle": {"dss_python": oracle_version, "engine": d.Version},
        "solution": {
            "converged": bool(sol.Converged),
            "iterations": int(sol.Iterations),
            "mode": int(sol.Mode),
            "tolerance": float(sol.Tolerance),
        },
        "circuit": {
            "num_buses": int(ckt.NumBuses),
            "num_nodes": int(ckt.NumNodes),
            "num_ckt_elements": int(ckt.NumCktElements),
        },
        "node_order": list(ckt.YNodeOrder),
        "node_voltages": list(ckt.YNodeVarray),  # V, re/im interleaved, YNodeOrder order
        "total_power_kw_kvar": list(ckt.TotalPower),
        "losses_w_var": list(ckt.Losses),
        "transformers": dump_transformers(ckt),
        "regcontrols": dump_regcontrols(ckt),
        "capacitors": dump_capacitors(ckt),
        "elements": dump_elements(ckt, case.get("props", True)),
    }


def main() -> None:
    oracle_version = check_pin()
    from dss import dss as d

    manifest = json.loads((REPO_ROOT / "tools" / "golden" / "cases.json").read_text())
    wanted = set(sys.argv[1:])
    cases = [c for c in manifest["cases"] if not wanted or c["name"] in wanted]
    if wanted and len(cases) != len(wanted):
        known = {c["name"] for c in manifest["cases"]}
        sys.exit(f"unknown case(s): {sorted(wanted - known)}")

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    for case in cases:
        data = run_case(d, case, oracle_version)
        out = OUT_DIR / f"{case['name']}.json"
        out.write_text(json.dumps(data, indent=1, default=_numpy_to_py))
        n = len(data["node_order"])
        it = data["solution"]["iterations"]
        conv = data["solution"]["converged"]
        print(f"{case['name']}: {n} nodes, converged={conv}, iterations={it} -> {out.relative_to(REPO_ROOT)}")


if __name__ == "__main__":
    main()
