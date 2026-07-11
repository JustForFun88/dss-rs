"""Oracle-side `Save circuit` round-trip probe for IEEE 8500-Node (FA settle).

Purpose: make the IEEE-8500 save-roundtrip node-voltage floor
(`crates/dss-core/tests/save_roundtrip.rs::save_roundtrip_ieee8500`,
`IEEE8500_SAVE_VTOL = 3e-4`) **reproducible from the repo**, not prose-only. The
Rust test loosens that deck's node-V self-consistency band from the other four
feeders' 1e-6 to 3e-4 and justifies it as an inherent OpenDSS `Save circuit`
precision floor. This probe proves that on the **pinned dss-python oracle**
itself: it saves a solved 8500 circuit, clears, recompiles the emitted
`Master.dss`, re-solves, and reports the worst per-node relative voltage shift
plus the pre/post total power — the same round-trip the Rust test performs, on
the same corpus bytes.

Runs with the pinned oracle only (tools/golden/PIN.txt: dss-python 0.15.7 /
dss_capi 0.14.5 — the vendored Pascal source). Read-only w.r.t. the repo; writes
only a scratch dir it deletes.

Usage:
    python tools/golden/probe_save_roundtrip_8500.py

Records its numbers in tests/TOLERANCE_NOTES.md (§save-roundtrip). Re-run to
re-verify the floor after an oracle re-pin.
"""

from __future__ import annotations

import math
import shutil
import sys
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
# The gate round-trips the vendored corpus copy (what save_roundtrip.rs reads),
# so probe the identical bytes.
MASTER = (
    REPO_ROOT
    / "tests"
    / "corpus"
    / "electricdss-tst"
    / "Version8"
    / "Distrib"
    / "IEEETestCases"
    / "8500-Node"
    / "Master.dss"
)


def check_pin() -> None:
    import dss

    if dss.__version__ != "0.15.7":
        sys.exit(f"dss-python {dss.__version__} != pinned 0.15.7 (tools/golden/PIN.txt)")


def snapshot(ckt) -> tuple[dict[str, complex], int, tuple[float, float]]:
    """(node name -> complex V), iteration count, total power (kW, kvar)."""
    order = list(ckt.YNodeOrder)
    varr = list(ckt.YNodeVarray)
    v = {order[i]: complex(varr[2 * i], varr[2 * i + 1]) for i in range(len(order))}
    tp = list(ckt.TotalPower)
    return v, int(ckt.Solution.Iterations), (tp[0], tp[1])


def main() -> None:
    check_pin()
    from dss import DSS as d

    if not MASTER.is_file():
        sys.exit(f"master not found: {MASTER}")

    out = Path(tempfile.mkdtemp(prefix="oracle_saveroundtrip_8500_"))
    try:
        # Pre-save: compile, raise the iteration cap (Run_8500Node.dss), warm solve.
        d.Text.Command = "clear"
        d.Text.Command = f'compile "{MASTER}"'
        d.Text.Command = "Set Maxiterations=20"
        d.Text.Command = "solve"
        d.Text.Command = "solve"
        ckt = d.ActiveCircuit
        assert ckt.Solution.Converged, "oracle pre-save solve did not converge"
        pre_v, pre_iter, pre_p = snapshot(ckt)

        # Save circuit, clear, recompile the emitted Master, cold+warm re-solve.
        out_fwd = str(out).replace("\\", "/")
        d.Text.Command = f'save circuit dir="{out_fwd}"'
        emitted = out / "Master.dss"
        assert emitted.is_file(), f"no Master.dss emitted to {out}"

        d.Text.Command = "clear"
        d.Text.Command = f'compile "{str(emitted).replace(chr(92), "/")}"'
        d.Text.Command = "Set Maxiterations=20"
        d.Text.Command = "solve"
        d.Text.Command = "solve"
        ckt = d.ActiveCircuit
        assert ckt.Solution.Converged, "oracle post-save solve did not converge"
        post_v, post_iter, post_p = snapshot(ckt)

        # Worst per-node relative voltage shift (matched by node name).
        worst_rel = 0.0
        worst_node = ""
        n_over_1e6 = 0
        for name, pv in pre_v.items():
            qv = post_v.get(name)
            if qv is None:
                continue
            mag = abs(pv)
            d_ = abs(qv - pv)
            rel = d_ / mag if mag > 0 else d_
            if rel > 1e-6:
                n_over_1e6 += 1
            if rel > worst_rel:
                worst_rel = rel
                worst_node = name

        print("=== IEEE-8500 Save-circuit round-trip (pinned oracle dss-python 0.15.7) ===")
        print(f"nodes:                 {len(pre_v)}")
        print(f"iterations pre/post:   {pre_iter} / {post_iter}  ({'EXACT' if pre_iter == post_iter else 'DIFFER'})")
        print(f"worst node:            {worst_node}")
        print(f"worst rel shift:       {worst_rel:.6e}")
        print(f"nodes over 1e-6 rel:   {n_over_1e6} / {len(pre_v)}")
        print(f"total power pre  (kW): {pre_p[0]:.6f}   (kvar): {pre_p[1]:.6f}")
        print(f"total power post (kW): {post_p[0]:.6f}   (kvar): {post_p[1]:.6f}")
        print(f"total power dP (kW):   {post_p[0] - pre_p[0]:.6f}")
        # Sanity: the Rust test's band is 3e-4; the observed floor must sit under it.
        verdict = "UNDER 3e-4 (floor consistent with the Rust band)" if worst_rel < 3e-4 else "OVER 3e-4"
        print(f"vs Rust IEEE8500_SAVE_VTOL=3e-4:  {verdict}")
    finally:
        shutil.rmtree(out, ignore_errors=True)


if __name__ == "__main__":
    main()
