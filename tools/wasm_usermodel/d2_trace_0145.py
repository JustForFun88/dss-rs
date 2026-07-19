#!/usr/bin/env python3
"""Enable the Generator DebugTrace on the pinned 0.14.5 oracle + r3723 twin and
run wasm_gen_dyn — dumps GEN_g1.csv (Speed/dSpeed/Pshaft/P=TracePower.re/M per
integrate). Usage: python d2_trace_0145.py <twin.dll> <outdir>"""
import os
import sys
from dss import dss


def main():
    twin = os.path.abspath(sys.argv[1]).replace("\\", "/")
    outdir = os.path.abspath(sys.argv[2]).replace("\\", "/")
    here = os.path.dirname(os.path.abspath(__file__))
    root = os.path.abspath(os.path.join(here, "..", ".."))
    tmpl = os.path.join(root, "tools", "golden", "wasm_decks", "wasm_gen_dyn.dss")
    text = open(tmpl).read().replace("@FIXTURE@", twin)

    dss.Text.Command = "clear"
    dss.DataPath = outdir
    dss.Text.Command = f"set datapath={outdir}"
    lines = [l.strip() for l in text.splitlines()
             if l.strip() and not l.strip().startswith(("!", "//"))]
    enabled = False
    for cmd in lines:
        dss.Text.Command = cmd
        if cmd.lower() == "solve" and not enabled:
            # snapshot done; enable trace ONCE before the dynamics section
            dss.Text.Command = "Generator.g1.DebugTrace=yes"
            enabled = True
    print("outdir:", outdir)
    print("converged:", dss.ActiveCircuit.Solution.Converged)


if __name__ == "__main__":
    main()
