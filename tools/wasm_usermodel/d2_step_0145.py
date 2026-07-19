#!/usr/bin/env python3
"""Per-step dynamics trajectory of wasm_gen_dyn on the pinned 0.14.5 oracle +
r3723 244B twin. Emits Slip/dSpeed/Is1u/Is1s/Freq after the snapshot (step 0)
and after each of the first 5 dynamics steps (range(1, 6)), to find the first
divergence vs the Rust engine — step 1 already exposes it, so a short run
suffices; the full end-state is captured separately by d2_probe_0145.py.
Usage: python d2_step_0145.py <twin.dll>"""
import os
import sys
from dss import dss


def state():
    ckt = dss.ActiveCircuit
    ckt.SetActiveElement("Generator.g1")
    e = ckt.ActiveCktElement
    v = list(e.AllVariableValues)
    # 6 built-in + 14 UserModel (13=Is1u) + 14 ShaftModel (27=Is1s)
    return {"Slip": v[6], "dSpeed": v[4], "Is1u": v[13], "Is1s": v[27], "Freq": v[0]}


def main():
    twin = os.path.abspath(sys.argv[1]).replace("\\", "/")
    here = os.path.dirname(os.path.abspath(__file__))
    root = os.path.abspath(os.path.join(here, "..", ".."))
    tmpl = os.path.join(root, "tools", "golden", "wasm_decks", "wasm_gen_dyn.dss")
    text = open(tmpl).read().replace("@FIXTURE@", twin)

    dss.Text.Command = "clear"
    # run everything up to (and including) the snapshot Solve; stop before dynamics
    lines = [l.strip() for l in text.splitlines()
             if l.strip() and not l.strip().startswith(("!", "//"))]
    i = 0
    while i < len(lines):
        cmd = lines[i]
        i += 1
        dss.Text.Command = cmd
        if cmd.lower() == "solve":
            break  # snapshot done
    d = state()
    keys = ["Slip", "dSpeed", "Is1u", "Is1s", "Freq"]
    print("step " + " ".join(f"{k:>22}" for k in keys))
    print("0    " + " ".join(f"{d[k]:22.14e}" for k in keys))

    import os as _os
    if _os.environ.get("D2TIGHT"):
        dss.Text.Command = "Set tolerance=1e-12 maxiterations=1000"
    dss.Text.Command = "Set mode=dynamics number=1 h=0.000166667"
    for step in range(1, 6):
        dss.Text.Command = "Solve"
        d = state()
        print(f"{step:<5}" + " ".join(f"{d[k]:22.14e}" for k in keys))


if __name__ == "__main__":
    main()
