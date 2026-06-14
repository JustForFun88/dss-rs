"""Live oracle server for the corpus comparison gate (CORPUS_TEST_PLAN.md §3).

A persistent process the Rust harness (`corpus_live.rs`) drives at test time:
read one JSON request per line on stdin, run it through the pinned dss-python
oracle, write one JSON response per line on stdout. This is the *live* oracle —
no goldens are written; both engines compile the **same** copied `.dss` file and
the harness compares their results in memory.

Reuses the `capture_*` helpers from `tools/golden/gen_checkpoints.py` so the
oracle side of the live gate and the checkpoint goldens stay byte-for-byte
consistent (same unfactored-Y CSC, same column-major YPrim, same injection
slicing, ...).

Protocol — one compact JSON object per line, no embedded newlines:

    request : {"cmd":"run","case_path":<abs path>,"post":[...],"n_steps":N,
               "selected_elements":[...],"full_csc":true}
              {"cmd":"ping"} | {"cmd":"quit"}
    response: {"ok":true,"result":{...}} | {"ok":false,"error":"..."}

stdout carries responses ONLY; all diagnostics go to stderr, and the Rust side
ignores any stdout line that is not a JSON object with an "ok" field, so stray
engine output cannot corrupt the stream. One failing case returns
`{"ok":false,...}` rather than killing the server; a hard crash is handled by the
Rust side restarting the process and recording the case as a failure.

Startup hard-asserts the pin in tools/golden/PIN.txt (dss-python 0.15.7); a wrong
oracle version exits non-zero, never a silent pass.

Usage (normally spawned by the Rust gate; manual smoke test):
    echo {"cmd":"ping"} | python tools/oracle/oracle_server.py
"""

from __future__ import annotations

import json
import sys
import traceback
from pathlib import Path

# Reuse the exact capture helpers the checkpoint goldens use, so both sides of
# the live gate agree by construction.
GOLDEN = Path(__file__).resolve().parent.parent / "golden"
sys.path.insert(0, str(GOLDEN))
import gen_checkpoints as gc  # noqa: E402


def log(msg: str) -> None:
    print(msg, file=sys.stderr, flush=True)


def reply(obj: dict) -> None:
    """Write one response line to stdout and flush (compact, single line)."""
    sys.stdout.write(json.dumps(obj, separators=(",", ":")) + "\n")
    sys.stdout.flush()


def capture_all_elements(ckt) -> list:
    """Every circuit element's terminal currents (A) and powers (kW/kvar).

    The plan mandates comparing *all* element currents/powers (not just the
    selected set), so the live gate captures the whole element list here.
    """
    out = []
    for name in ckt.AllElementNames:
        out.append(gc.capture_element(ckt, name))
    return out


def run_case(d, req: dict) -> dict:
    """Compile one copied `.dss` case, run `n_steps` solves, return the full
    per-step model (the shape `harness::*` / corpus_live.rs deserialize)."""
    case_path = req["case_path"]
    post = req.get("post") or []
    n_steps = int(req.get("n_steps", 1))
    selected = req.get("selected_elements") or []
    full_csc = bool(req.get("full_csc", True))

    d.Text.Command = "clear"
    d.Text.Command = f'Compile "{case_path}"'
    for c in post:
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
        disc = gc.capture_discrete(ckt)
        checkpoints.append(
            {
                "dbl_hour": float(sol.dblHour),
                "iterations": int(sol.Iterations),
                "converged": bool(sol.Converged),
                "v_re": varray[0::2],
                "v_im": varray[1::2],
                "y": gc.capture_system_y(d) if full_csc else None,
                "y_fingerprint": gc.capture_fingerprint(d),
                "yprims": [gc.capture_yprim(ckt, nm) for nm in selected],
                "elements": capture_all_elements(ckt),
                "injection": gc.capture_injection(d),
                "transformers": disc["transformers"],
                "regcontrols": disc["regcontrols"],
                "capacitors": disc["capacitors"],
            }
        )
    return {"node_order": node_order, "n_steps": n_steps, "checkpoints": checkpoints}


def main() -> None:
    oracle = gc.check_pin()  # hard-asserts dss-python 0.15.7 / engine 0.14.5
    from dss import DSS as d

    d.AllowForms = False
    log(f"oracle_server ready: {oracle}")

    # NB: read with readline(), not `for line in sys.stdin` — the latter reads
    # ahead in large blocks and would deadlock the request/response protocol
    # (the server would not see a request until its read buffer filled).
    while True:
        raw = sys.stdin.readline()
        if not raw:  # EOF: stdin closed
            break
        line = raw.strip()
        if not line:
            continue
        try:
            req = json.loads(line)
        except json.JSONDecodeError as e:
            reply({"ok": False, "error": f"bad request json: {e}"})
            continue
        cmd = req.get("cmd")
        if cmd == "quit":
            break
        if cmd == "ping":
            reply({"ok": True, "result": {"pong": True, "oracle": oracle}})
            continue
        if cmd != "run":
            reply({"ok": False, "error": f"unknown cmd {cmd!r}"})
            continue
        try:
            reply({"ok": True, "result": run_case(d, req)})
        except Exception as e:  # one bad case must not kill the server
            log("case failed:\n" + traceback.format_exc())
            reply({"ok": False, "error": f"{type(e).__name__}: {e}"})


if __name__ == "__main__":
    main()
