#!/usr/bin/env python3
"""d2_probe_0145.py — drive a wasm_decks/*.dss deck through the PINNED
dss-python 0.15.7 (backend dss_capi 0.14.5) with @FIXTURE@ substituted by the
0.14.5-ABI native twin DLL (tools/wasm_usermodel/build_native_r3723.ps1), and
emit the SAME golden schema (variable_names/values + node_voltages) the r4133
bridge golden uses (crates/dss-epri/tests/gen_wasm_usermodels.rs).

This is the missing 0.14.5 channel of the WM.3 D2 three-way experiment. The
r3723 twin has the 244-byte TGeneratorVars (no deltaQNom) so it is ABI-correct
for the 0.14.5 engine; its machine math is byte-identical to the r4133 twin
(sha256-verified, USERMODEL_ABI.md P8) — the sole controlled variable is the
engine version.

Usage:  python d2_probe_0145.py <deck-basename> <path-to-244B-IndMach012a.dll>
Emits JSON to stdout.
"""

import json
import os
import sys

from dss import dss


def main():
    deck = sys.argv[1]
    twin = os.path.abspath(sys.argv[2]).replace("\\", "/")
    here = os.path.dirname(os.path.abspath(__file__))
    repo_root = os.path.abspath(os.path.join(here, "..", ".."))
    tmpl = os.path.join(repo_root, "tools", "golden", "wasm_decks", f"{deck}.dss")
    with open(tmpl, "r") as f:
        text = f.read()
    deck_text = text.replace("@FIXTURE@", twin)

    dss.Text.Command = "clear"
    for line in deck_text.splitlines():
        t = line.strip()
        if not t or t.startswith("!") or t.startswith("//"):
            continue
        dss.Text.Command = t

    ckt = dss.ActiveCircuit
    ckt.SetActiveElement("Generator.g1")
    elem = ckt.ActiveCktElement
    names = list(elem.AllVariableNames)
    values = list(elem.AllVariableValues)

    order = list(ckt.YNodeOrder)
    varray = list(ckt.YNodeVarray)
    node_v = {}
    for k, name in enumerate(order):
        node_v[name] = [varray[2 * k], varray[2 * k + 1]]

    out = {
        "deck": deck,
        "engine": "dss-python 0.15.7 / dss_capi 0.14.5 + r3723 244B twin",
        "twin": twin,
        "converged": bool(ckt.Solution.Converged),
        "iterations": int(ckt.Solution.Iterations),
        "variable_names": names,
        "variable_values": values,
        "node_voltages": node_v,
    }
    print(json.dumps(out, indent=2))


if __name__ == "__main__":
    main()
