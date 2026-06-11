"""Probe FPC Val behavior through the oracle's Parser (same TDSSParser).

A failed conversion raises a Pascal exception that crashes the process, so
each case runs in a subprocess. Run: python tools/golden/probe_val.py
"""
import subprocess
import sys

CHILD = """
import sys
from dss import dss
p = dss.Parser
kind, s = sys.argv[1], sys.argv[2]
p.CmdString = "x=" + s
_ = p.NextParam
print(repr(p.DblValue if kind == "dbl" else p.IntValue))
"""

cases = [
    ".5", "5.", "1e3", "+5", "-0.25", "$FF", "0xFF", "1.5e-3",
    "1_000", "5.5.5", "1e", "inf", "nan", "%101", "&777", "007",
    "-.5", "2.5", "1.5", "0.5", "$ff", "1e+3", "1.e3",
]

for kind in ("dbl", "int"):
    for s in cases:
        r = subprocess.run(
            [sys.executable, "-c", CHILD, kind, s],
            capture_output=True, text=True, timeout=120,
        )
        out = r.stdout.strip() if r.returncode == 0 else f"ERROR rc={r.returncode} {r.stdout.strip()[:60]}"
        print(f"{kind} {s!r:10} -> {out}", flush=True)
