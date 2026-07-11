"""Generate the checkpointed-model goldens from the pinned oracle.

The *checkpointed-model* gate (see the golden-infrastructure plan). Unlike the
timeseries_controls/metering_monitors command-replay goldens — which compare only converged outputs
(voltages, currents, registers) — these goldens capture the **assembled
electrical model itself** after every committed time step: the system Y matrix,
selected element YPrim blocks, the injection vector, and discrete control state.
A stale Y/YPrim then fails at the step it first goes wrong, at the matrix entry
that is wrong, instead of as downstream accumulated drift.

A "checkpoint" = the committed circuit state after one `solve` (number=1)
returns: controls converged, taps/caps final, Y rebuilt. No internal solver
iteration is ever captured.

Output layout: one file per scenario under tests/golden/checkpoints/
(<name>.json, schema 2). golden_checkpoints.rs runs *every* file in that
directory, so adding a scenario is just dropping a new file — and each
scenario's diff stays isolated to its own file.

Scenarios (one builder each, registered in SCENARIOS):
  - micro_yeq_steps: a tiny radial + a flat-then-jump daily shape. Full
    assembled Y (CSC) + load YPrim each step — control-free per-step pin.
  - ieee13_daily: IEEE13 (controls active) + the 24-pt daily shape. The direct
    regression guard for the "frozen load Yeq" bug — at each tap-change rebuild
    the recompute-all path re-stamps the current Yeq.
  - ieee123_snap: IEEE123 master, snapshot — the large-feeder fingerprint-only
    + selected-YPrim path.

Usage:
    python tools/golden/gen_checkpoints.py                # regenerate all
    python tools/golden/gen_checkpoints.py ieee13_daily   # one scenario

Regeneration is manual and must use the exact versions in tools/golden/PIN.txt.
"""

from __future__ import annotations

import json
import sys
from math import isqrt
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from gen_timeseries_controls import IEEE13, IEEE13_LOADS, DAY_CURVE  # noqa: E402
from gen_metering_monitors import MICRO  # noqa: E402

REPO_ROOT = Path(__file__).resolve().parents[2]
OUT_DIR = REPO_ROOT / "tests" / "golden" / "checkpoints"
SCHEMA = 2


def check_pin() -> dict:
    import dss

    if dss.__version__ != "0.15.7":
        sys.exit(f"dss-python {dss.__version__} != pinned 0.15.7 (tools/golden/PIN.txt)")
    from dss import DSS

    # Also hard-assert the engine/backend (dss-python-backend 0.14.5 == the
    # vendored Pascal at .inputs/dss_capi). DSS.Version is e.g. "DSS C-API
    # Library version 0.14.5 revision ...". A backend mismatch is a different
    # oracle and must fail loudly, not silently change the numbers.
    if "0.14.5" not in DSS.Version:
        sys.exit(f"engine {DSS.Version!r} != pinned backend 0.14.5 (tools/golden/PIN.txt)")

    return {"dss_python": dss.__version__, "engine": DSS.Version}


def _get_y_sparse(d):
    """`getYSparse(False)` with a `BuildY` retry: on a step where the solve
    rebuilt Y mid-step (e.g. a Fault applying at its ontime, a protection trip
    opening a switch), the pinned engine's compressed export is unavailable and
    `getYSparse(False)` returns None. The executive `BuildY` command rebuilds
    the same assembled matrix and makes the export available again — proven
    trajectory-neutral (identical per-step iterations/voltages/event log with
    and without the retry over the recloser trip/reclose deck). Do NOT use
    `getYSparse(True)` here: the factored path corrupts the solution vector
    (YNodeVarray returns injection-scale garbage afterwards, empirically)."""
    r = d.YMatrix.getYSparse(False)
    if r is None:
        d.Text.Command = "BuildY"
        r = d.YMatrix.getYSparse(False)
    if r is None:
        sys.exit("YMatrix.getYSparse returned None even after a BuildY retry")
    return r


def capture_system_y(d) -> dict:
    """The assembled, UNFACTORED system Y as coordinate lists.

    `getYSparse(False)` returns the CSC triple (data, row_indices, col_ptr) of
    the matrix as stamped from element YPrims — before KLU's factorization
    scaling — so it lines up with the Rust side's unscaled assembled `y_system`.
    Row/col are 0-based; row i corresponds to YNodeOrder[i].
    """
    data, row_idx, col_ptr = _get_y_sparse(d)
    n = len(col_ptr) - 1
    rows, cols, re, im = [], [], [], []
    for col in range(n):
        for k in range(int(col_ptr[col]), int(col_ptr[col + 1])):
            rows.append(int(row_idx[k]))
            cols.append(col)
            re.append(float(data[k].real))
            im.append(float(data[k].imag))
    return {"n": n, "rows": rows, "cols": cols, "re": re, "im": im}


def capture_yprim(ckt, name: str) -> dict:
    """An element's Yprim in COLUMN-MAJOR order (the raw `CktElement.Yprim`
    layout — Pascal TcMatrix is column-major), so it matches the Rust
    `Dss::element_yprim` flat array entry-for-entry with no transpose."""
    ckt.SetActiveElement(name)
    flat = list(ckt.ActiveCktElement.Yprim)  # 2 * yorder^2 floats, re/im interleaved
    yorder = isqrt(len(flat) // 2)
    assert 2 * yorder * yorder == len(flat), f"{name}: non-square Yprim ({len(flat)})"
    re = [float(x) for x in flat[0::2]]
    im = [float(x) for x in flat[1::2]]
    return {"name": name, "yorder": yorder, "re": re, "im": im}


def capture_fingerprint(d, floor: float = 1e-9) -> dict:
    """A compact, solver-independent fingerprint of the assembled Y for large
    feeders where storing the full CSC every step is too big: number of
    nonzeros above `floor`, Frobenius norm, complex trace, and max |diagonal|.
    The precise stale-Y catch at scale is the selected YPrim blocks; this is the
    cheap structural+magnitude guard."""
    data, row_idx, col_ptr = _get_y_sparse(d)
    n = len(col_ptr) - 1
    nnz = sum(1 for x in data if abs(x) > floor)
    frob = sum(abs(x) ** 2 for x in data) ** 0.5
    tr = 0j
    maxdiag = 0.0
    for col in range(n):
        for k in range(int(col_ptr[col]), int(col_ptr[col + 1])):
            if int(row_idx[k]) == col:
                tr += complex(data[k])
                maxdiag = max(maxdiag, abs(data[k]))
    return {
        "nnz": nnz,
        "frob": float(frob),
        "tr_re": float(tr.real),
        "tr_im": float(tr.imag),
        "maxdiag": float(maxdiag),
    }


def capture_injection(d) -> dict:
    """The node injection-current vector (`YMatrix.getI()`, the RHS of Y*V=I),
    nodes 1..n only — slot 0 (ground) is dropped so it aligns with the Rust
    `Dss::node_injection_currents()[1..=n]`."""
    cur = list(d.YMatrix.getI())  # length 2*(n+1), re/im interleaved, slot 0 = ground
    re = [float(x) for x in cur[0::2]][1:]
    im = [float(x) for x in cur[1::2]][1:]
    return {"re": re, "im": im}


def capture_element(ckt, name: str) -> dict:
    """A selected element's terminal currents (A, re/im) and powers (kW/kvar),
    the oracle `CktElement.Currents` / `Powers`.

    `Powers` is read BEFORE `Currents`: for a Thevenin-family DER
    (Generator/PVSystem/Storage) in harmonics mode the oracle's `Currents` query
    leaves the cached `Iterminal` stale, so a following `Powers` query returns
    `V_harmonic · conj(I_fundamental)` — a physically meaningless cross-product
    (the engine's own order-dependent quirk, NOT a stable output worth pinning).
    Reading `Powers` first yields the consistent `V_harmonic · conj(I_harmonic)`
    the engine produces when asked directly, matching the Rust harness's
    single-pass `node_v · conj(Iterminal)`. For every non-harmonic / current-source
    element the two orders are identical, so existing captures are unchanged."""
    ckt.SetActiveElement(name)
    el = ckt.ActiveCktElement
    pwr = list(el.Powers)
    cur = list(el.Currents)
    return {
        "name": name,
        "i_re": cur[0::2],
        "i_im": cur[1::2],
        "p_kw": pwr[0::2],
        "p_kvar": pwr[1::2],
    }


def capture_discrete(ckt) -> dict:
    """Per-step discrete control state: transformer winding taps, RegControl tap
    numbers, capacitor states (all compared EXACTLY on the Rust side)."""
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
        "transformers": transformers,
        "regcontrols": regcontrols,
        "capacitors": capacitors,
    }


def scenario_micro_yeq(d) -> dict:
    selected = ["Load.ld3", "Load.ld4"]
    cmds = list(MICRO)
    cmds += [
        "New loadshape.flatjump npts=8 interval=1 mult=(1 1 1 1 1.5 1.5 1.5 1.5)",
        "Load.ld3.daily=flatjump",
        "Load.ld4.daily=flatjump",
        "set mode=daily stepsize=1h number=1",
    ]
    node_order, checkpoints = run_steps(d, cmds, 8, selected)
    return {
        "name": "micro_yeq_steps",
        "kind": "micro",
        "master": None,
        "commands": cmds,
        "n_steps": 8,
        "node_order": node_order,
        "selected_elements": selected,
        "checkpoints": checkpoints,
    }


def run_steps(
    d,
    cmds: list[str],
    n_steps: int,
    selected: list[str],
    master: str | None = None,
    full_csc: bool = True,
) -> tuple[list, list]:
    """Optionally `Compile <master>`, replay `cmds`, then `solve` `n_steps`
    times capturing a checkpoint each step. `full_csc=False` stores only the
    Y fingerprint (large feeders). Returns (node_order, checkpoints)."""
    d.Text.Command = "clear"
    if master is not None:
        master_abs = (REPO_ROOT / "tests" / "corpus" / "electricdss-tst" / master).resolve()
        if not master_abs.is_file():
            sys.exit(f"master not found: {master_abs}")
        d.Text.Command = f'Compile "{master_abs}"'
    for c in cmds:
        d.Text.Command = c
    ckt = d.ActiveCircuit
    node_order = None
    checkpoints = []
    for _ in range(n_steps):
        d.Text.Command = "solve"
        sol = ckt.Solution
        if node_order is None:
            node_order = list(ckt.YNodeOrder)
        varray = list(ckt.YNodeVarray)
        disc = capture_discrete(ckt)
        checkpoints.append(
            {
                "dbl_hour": float(sol.dblHour),
                "iterations": int(sol.Iterations),
                "converged": bool(sol.Converged),
                "v_re": varray[0::2],
                "v_im": varray[1::2],
                "y": capture_system_y(d) if full_csc else None,
                "y_fingerprint": capture_fingerprint(d),
                "yprims": [capture_yprim(ckt, nm) for nm in selected],
                "elements": [capture_element(ckt, nm) for nm in selected],
                "injection": capture_injection(d),
                "transformers": disc["transformers"],
                "regcontrols": disc["regcontrols"],
                "capacitors": disc["capacitors"],
            }
        )
    return node_order, checkpoints


def scenario_ieee13_daily(d) -> dict:
    """IEEE13 (controls active) + the 24-pt daily shape on every load.

    The real regression scenario for the "stale load Yeq" bug: the load Yeq is
    re-stamped into the system Y only when Y is rebuilt, and the daily run
    rebuilds at every tap change (oracle: steps 0, 6-12, 21-23). At each such
    rebuild the load level has moved since the previous rebuild, so the
    recompute-all path (oracle) re-stamps the current Yeq while a
    recompute-only-invalid bug keeps a stale one — a divergence in the assembled
    Y and in the selected load YPrim blocks at the rebuild step. A strict
    superset of one isolated flat-then-jump.
    """
    selected = [
        "Transformer.reg1",
        "Transformer.reg2",
        "Transformer.reg3",
        "Load.634a",
        "Load.671",
    ]
    cmds = list(IEEE13)
    cmds.append(f"New loadshape.day npts=24 interval=1 mult={DAY_CURVE}")
    for nm in IEEE13_LOADS:
        cmds.append(f"Load.{nm}.daily=day")
    cmds.append("set mode=daily stepsize=1h number=1")
    node_order, checkpoints = run_steps(d, cmds, 24, selected)
    return {
        "name": "ieee13_daily",
        "kind": "feeder",
        "master": None,
        "commands": cmds,
        "n_steps": 24,
        "node_order": node_order,
        "selected_elements": selected,
        "checkpoints": checkpoints,
    }


# IEEE 123-Bus master, relative to tests/corpus/electricdss-tst (compiled by the
# harness, like golden_feeders_controls.rs).
IEEE123_MASTER = "Version8/Distrib/IEEETestCases/123Bus/IEEE123Master.dss"


def scenario_ieee123_snap(d) -> dict:
    """The large-feeder fingerprint path: compile the unmodified IEEE123 master
    and snapshot solve (1 step). Stores the Y *fingerprint* only (no full CSC)
    plus selected regulator/load YPrim blocks, exercising the
    fingerprint-only + selected-block machinery on a real ~280-node feeder with
    no per-step full-CSC cost. (The 8500-node snapshot is already pinned in
    full by golden_ieee8500.rs; duplicating its ~8.5k-node voltage/injection
    arrays here would bloat the golden by ~1 MB for no extra coverage — the
    fingerprint code path is identical at any scale. The per-step stale-Y catch
    itself is covered by ieee13_daily.)"""
    selected = ["Transformer.reg1a", "Load.s1a"]
    node_order, checkpoints = run_steps(
        d, [], 1, selected, master=IEEE123_MASTER, full_csc=False
    )
    return {
        "name": "ieee123_snap",
        "kind": "large",
        "master": IEEE123_MASTER,
        "commands": [],
        "n_steps": 1,
        "node_order": node_order,
        "selected_elements": selected,
        "checkpoints": checkpoints,
    }


# name -> builder. One file per entry under OUT_DIR; golden_checkpoints.rs runs
# every *.json in the directory, so this is the single source of truth for which
# scenarios exist.
SCENARIOS = {
    "micro_yeq_steps": scenario_micro_yeq,
    "ieee13_daily": scenario_ieee13_daily,
    "ieee123_snap": scenario_ieee123_snap,
}


def main() -> None:
    oracle = check_pin()
    from dss import DSS as d

    wanted = set(sys.argv[1:])
    unknown = wanted - set(SCENARIOS)
    if unknown:
        sys.exit(f"unknown scenario(s): {sorted(unknown)}; known: {sorted(SCENARIOS)}")
    names = [n for n in SCENARIOS if not wanted or n in wanted]

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    for name in names:
        sc = SCENARIOS[name](d)
        assert sc["name"] == name, f"{name}: builder returned name {sc['name']!r}"
        path = OUT_DIR / f"{name}.json"
        path.write_text(
            json.dumps({"schema": SCHEMA, "oracle": oracle, "scenario": sc}, indent=1) + "\n"
        )
        print(f"wrote {path.relative_to(REPO_ROOT)} ({sc['n_steps']} steps)")

    # On a full regen, drop any stale scenario files no longer in the registry.
    if not wanted:
        for p in OUT_DIR.glob("*.json"):
            if p.stem not in SCENARIOS:
                p.unlink()
                print(f"removed stale {p.relative_to(REPO_ROOT)}")


if __name__ == "__main__":
    main()
