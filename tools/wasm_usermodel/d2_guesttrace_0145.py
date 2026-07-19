#!/usr/bin/env python3
"""d2_guesttrace_0145.py — WM.3 D2 sub-bug #2 investigation.

Enable the IndMach012 model's OWN built-in DebugTrace (`option=Debug`) on the
pinned 0.14.5 oracle + r3723 244-B twin and dump IndMach012_Trace.CSV, which
records the GUEST-INTERNAL state per CalcDynamic:
  Time, IterationFlag, S1, |Is1|, |Is2|, |E1|, |dE1dt|, |E2|, |dE2dt|, |V1|, |V2|

This is the only channel that exposes the guest flux `E1` on the oracle side
(it is not in the 14-var monitoring surface). With `H` it varies the step size:
at h=1e-9 the guest |E1| still jumps to the dynamic operating point (7412.15,
not the pflow 7412.52), proving the D2 first-step E1 move is h-INDEPENDENT.

Usage: python d2_guesttrace_0145.py <twin.dll> <outdir> [nsteps]
Env:   DECK (deck path; default tools/golden/wasm_decks/wasm_gen_dyn.dss),
       H (dynamics step, default 0.000166667).
"""
import os
import sys

from dss import dss


def main():
    twin = os.path.abspath(sys.argv[1]).replace("\\", "/")
    outdir = os.path.abspath(sys.argv[2]).replace("\\", "/")
    nsteps = int(sys.argv[3]) if len(sys.argv) > 3 else 2
    here = os.path.dirname(os.path.abspath(__file__))
    root = os.path.abspath(os.path.join(here, "..", ".."))
    tmpl = os.environ.get("DECK", os.path.join(root, "tools", "golden", "wasm_decks", "wasm_gen_dyn.dss"))
    text = open(tmpl).read().replace("@FIXTURE@", twin)
    # Inject option=Debug into every UserData/ShaftData so the guest dumps its
    # internal E1/dE1dt/V1 (DebugTrace is a unit-global; instances share the CSV).
    text = text.replace("MaxSlip=0.1)", "MaxSlip=0.1 option=Debug)")

    os.makedirs(outdir, exist_ok=True)
    csv = os.path.join(outdir, "IndMach012_Trace.CSV")
    if os.path.exists(csv):
        os.remove(csv)

    dss.Text.Command = "clear"
    dss.DataPath = outdir
    lines = [l.strip() for l in text.splitlines()
             if l.strip() and not l.strip().startswith(("!", "//"))]
    for cmd in lines:
        if cmd.lower().startswith("solve"):
            break
        dss.Text.Command = cmd
    dss.Text.Command = "set datapath=" + outdir
    dss.Text.Command = "Solve"  # snapshot
    h = os.environ.get("H", "0.000166667")
    dss.Text.Command = f"Set mode=dynamics number=1 h={h}"
    for _ in range(nsteps):
        dss.Text.Command = "Solve"

    print("converged:", dss.ActiveCircuit.Solution.Converged)
    print("csv:", csv)
    if os.path.exists(csv):
        with open(csv) as f:
            for ln in f:
                print(ln.rstrip())


if __name__ == "__main__":
    main()
