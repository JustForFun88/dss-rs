"""Generate the NCIM report goldens from the **capi015** oracle (UPGRADE_PLAN
WP-U1.7 tail).

Pins the files the oracle writes for `Export Jacobian` / `Export deltaF` /
`Export deltaZ` / `Show PV2PQ_Conversions` after a `Set Algorithm=NCIM` solve.
The pinned 0.15.7 gate oracle (tools/golden/PIN.txt) has **no NCIM**, so — like
the embedded numerics in `crates/dss-core/src/exec/tests/ncim.rs` — these goldens
come from the capi015 line (dss-python 0.16.0b2 / dss_capi 0.15.0b4, OpenDSS SVN
r4103) in tools/opendss/.venv.

    tools/opendss/.venv/Scripts/python tools/golden/gen_ncim_reports.py

Each fixture writes a self-contained `<stem>.meta.json` (the exact deck) plus the
oracle report bytes:
  * `<stem>_Jacobian.csv`  — numeric-token compared (row/col exact, value tol);
  * `<stem>_PV2PQ.txt`     — byte-exact.
`deltaF`/`deltaZ` are the converged mismatch/correction, i.e. at the ~1e-11
faer-vs-KLU floor: their per-line VALUES are noise, so the Rust driver gates them
structurally (line count, six leading swing zeros, converged magnitude) rather
than value-pinning them. Regeneration is manual and MUST use the capi015 venv.
"""

from __future__ import annotations

import json
import os
import tempfile
from pathlib import Path

import dss

OUT = Path(__file__).resolve().parents[1] / ".." / "tests" / "golden" / "ncim"
OUT = OUT.resolve()

# --- fixtures: name -> deck lines (mirror `exec/tests/ncim.rs` pq/pv circuits) --
FIXTURES = {
    # PQ-only micro feeder (sourcebus -> mid -> loadbus), 9 nodes.
    "pq": [
        "clear",
        "New circuit.ncimtest basekv=12.47 phases=3 bus1=sourcebus",
        "New Line.l1 bus1=sourcebus bus2=mid phases=3 r1=0.12 x1=0.35 length=2",
        "New Line.l2 bus1=mid bus2=loadbus phases=3 r1=0.12 x1=0.35 length=1",
        "New Load.ld1 bus1=loadbus phases=3 kv=12.47 kw=1200 kvar=500 model=1",
        "New Load.ld2 bus1=mid phases=3 kv=12.47 kw=600 kvar=200 model=1",
        "Set voltagebases=[12.47]",
        "Calcvoltagebases",
        "Set algorithm=NCIM",
        "Solve",
    ],
    # PV-bus generator hitting its +Q limit -> PV->PQ conversion, 6 nodes.
    "pv_qlimit": [
        "clear",
        "New circuit.ncimpv basekv=12.47 phases=3 bus1=sourcebus",
        "New Line.l1 bus1=sourcebus bus2=genbus phases=3 r1=0.12 x1=0.35 length=3",
        "New Load.ld1 bus1=genbus phases=3 kv=12.47 kw=2000 kvar=800 model=1",
        "New Generator.g1 bus1=genbus phases=3 kv=12.47 kw=800 model=3 "
        "maxkvar=1500 minkvar=-1500 vpu=1.01",
        "Set voltagebases=[12.47]",
        "Calcvoltagebases",
        "Set algorithm=NCIM",
        "Solve",
    ],
}


def main() -> None:
    d = dss.DSS
    d.AllowForms = False
    try:
        d.AllowEditor = False
    except Exception:
        pass
    ver = str(d.Version)
    if "0.15.0b4" not in ver:
        raise SystemExit(f"expected capi015 backend 0.15.0b4, got {ver!r}")
    OUT.mkdir(parents=True, exist_ok=True)

    for stem, deck in FIXTURES.items():
        scratch = tempfile.mkdtemp(prefix=f"ncimgold_{stem}_")
        for c in deck:
            d.Text.Command = c
        d.Text.Command = f'set datapath="{scratch}"'
        ckt = d.ActiveCircuit
        assert ckt.Solution.Converged, f"{stem}: capi015 NCIM did not converge"
        n_nodes = ckt.NumNodes

        d.Text.Command = "Export Jacobian"
        jac = Path(str(d.Text.Result)).read_text()
        (OUT / f"{stem}_Jacobian.csv").write_text(jac, newline="")

        # deltaF/deltaZ line counts (structural gate anchor).
        d.Text.Command = "Export deltaF"
        n_df = len(Path(str(d.Text.Result)).read_text().splitlines())
        d.Text.Command = "Export deltaZ"
        n_dz = len(Path(str(d.Text.Result)).read_text().splitlines())

        d.Text.Command = "Show PV2PQ_Conversions"
        pv2pq = os.path.join(scratch, f"{ckt.Name}_PV2PQ_Generators.csv")
        (OUT / f"{stem}_PV2PQ.txt").write_text(
            Path(pv2pq).read_text(), newline=""
        )

        meta = {
            "deck": deck,
            "n_nodes": int(n_nodes),
            "iterations": int(ckt.Solution.Iterations),
            "delta_len": n_df,
            "oracle": "capi015",
            "engine": ver,
        }
        assert n_df == n_dz, f"{stem}: deltaF ({n_df}) != deltaZ ({n_dz}) length"
        (OUT / f"{stem}.meta.json").write_text(json.dumps(meta, indent=1) + "\n")
        print(f"{stem}: nodes={n_nodes} iters={meta['iterations']} deltaLen={n_df}")


if __name__ == "__main__":
    main()
