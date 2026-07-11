#!/usr/bin/env python
"""WP-U0.2 helper: run the `modes` corpus family ONE CASE PER ab_compare PROCESS.

Two modes decks hard-crash a *shared* engine and poison every following case in
the same process (`#303 on clear`): the `#58614` binary/MMF-shape access
violation (`shape_binfiles`/`shape_mmf`/`xycurve_files`) and `newton_feeder.dss`
(a non-converging Newton solve that hits the request timeout). Running each modes
case in its own subprocess confines a crasher to its own row. Results are merged
into `<out-dir>/modes_<tag>.json`, mirroring `ab_compare.py`'s report shape, for
`sweep_merge.py` to splice onto the shared corpus run.

Usage (from the repo root, Oddie venv reachable):
    export DSS_OPENDSS_PYTHON=".../tools/opendss/.venv/Scripts/python.exe"
    python tools/opendss/sweep_modes_isolated.py \
        --a capi --b capi015 --tag capi_vs_capi015 --out-dir <scratch>/sweeps
"""
import argparse
import json
import os
import subprocess
import sys
from collections import Counter
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
MODES_MANIFEST = REPO_ROOT / "tests" / "corpus" / "modes" / "manifest.json"
AB_COMPARE = REPO_ROOT / "tools" / "opendss" / "ab_compare.py"


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--a", required=True, help="engine A spec (capi|capi015|oddie:<rev>)")
    ap.add_argument("--b", required=True, help="engine B spec")
    ap.add_argument("--tag", required=True, help="output tag, e.g. capi_vs_capi015")
    ap.add_argument("--out-dir", required=True, type=Path, help="dir for modes_<tag>.json")
    ap.add_argument("--timeout", type=float, default=45.0, help="per-case ab_compare --timeout")
    ap.add_argument("--proc-timeout", type=float, default=200.0, help="hard subprocess wall cap")
    args = ap.parse_args()

    args.out_dir.mkdir(parents=True, exist_ok=True)
    cases = [
        c["path"] if isinstance(c, dict) else c
        for c in json.loads(MODES_MANIFEST.read_text())["cases"]
    ]

    merged: dict = {"a": {"spec": args.a}, "b": {"spec": args.b}, "cases": []}
    for i, path in enumerate(cases):
        tmp = args.out_dir / f"_tmp_modes_{args.tag}.json"
        cmd = [
            sys.executable, str(AB_COMPARE), "--a", args.a, "--b", args.b,
            "--manifest", str(MODES_MANIFEST), "--case", path,
            "--timeout", str(args.timeout), "--out", str(tmp),
            "--md", str(args.out_dir / f"_tmp_modes_{args.tag}.md"),
        ]
        try:
            subprocess.run(
                cmd, cwd=str(REPO_ROOT), env=dict(os.environ),
                timeout=args.proc_timeout,
                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
            )
        except subprocess.TimeoutExpired:
            merged["cases"].append({"path": path, "status": "harness_timeout",
                                    "first_divergence": f"isolated run exceeded {args.proc_timeout}s"})
            print(f"[{args.tag}] {i + 1}/{len(cases)} {path} -> harness_timeout", flush=True)
            continue
        if not tmp.exists():
            merged["cases"].append({"path": path, "status": "no_output",
                                    "first_divergence": "ab_compare wrote no json"})
            print(f"[{args.tag}] {i + 1}/{len(cases)} {path} -> no_output", flush=True)
            continue
        rep = json.loads(tmp.read_text())
        # exact path match — a substring --case filter can pull siblings.
        hit = next((c for c in rep["cases"] if c["path"] == path), None)
        if hit is None and rep["cases"]:
            hit = rep["cases"][0]
        if hit is None:
            hit = {"path": path, "status": "no_case", "first_divergence": ""}
        merged["cases"].append(hit)
        if "engine" in rep.get("a", {}):
            merged["a"]["engine"] = rep["a"]["engine"]
            merged["b"]["engine"] = rep["b"]["engine"]
        if "tolerances" in rep:
            merged["tolerances"] = rep["tolerances"]
        print(f"[{args.tag}] {i + 1}/{len(cases)} {path} -> {hit['status']}", flush=True)

    merged["counts"] = dict(Counter(c["status"] for c in merged["cases"]))
    merged.setdefault("tolerances", {})
    out = args.out_dir / f"modes_{args.tag}.json"
    out.write_text(json.dumps(merged, indent=1))
    print(f"[{args.tag}] DONE counts={merged['counts']} -> {out}", flush=True)


if __name__ == "__main__":
    main()
