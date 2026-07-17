"""Regenerate the Fuse property golden for the **r4133** surface (WP-U2.1).

Fuse's defaults + property table changed in OpenDSS r4133 (delta rows C3/D1):
`RatedCurrent` repurposed to an informational continuous rating (default
1.0 -> 0.0), new `CurveMultiplier` (default 1.0, the TCC divisor) and
`InterruptingRating` (default 0), default `FuseCurve` `tlink` -> `none` (a
default-constructed fuse never blows), properties 10 -> 12.

Why this golden is DERIVED, not captured:

  * No capi-line engine has the r4133 fuse behavior (capi015/r4103 still carries
    the 0.14.5 fuse surface — probed), so the golden cannot be captured on the
    engine whose property renderer matches the port.
  * The official EPRI r4133 engine (Oddie) DOES have the behavior, but its Delphi
    property renderer diverges from the dss_capi / Rust property system on props
    unrelated to this delta: object-ref props render the raw never-set
    `PropertyValue` (SwitchedObj -> "" instead of the defaulted ControlledElement,
    MonitoredObj lowercased), booleans render "true"/"false" not "Yes"/"No".
    capi015 in turn capitalizes the `State`/`Normal` enum labels ("Closed"),
    whereas 0.14.5, r4133-EPRI, and the port all render them lowercase.

The trusted rendering baseline that the port already reproduces bit-for-bit is
therefore the RETIRED 0.14.5 capi golden. This generator reads that pre-WP
golden from git and overlays the r4133 value deltas — each verified against the
official EPRI r4133 engine directly (Oddie capture, WP-U2.1): default fuse
FuseCurve="none" RatedCurrent="0" CurveMultiplier="1.0" InterruptingRating="0";
explicit/makelike scenarios keep their explicit/inherited RatedCurrent and curve.

    tools/opendss/.venv/Scripts/python.exe tools/golden/gen_fuse_r4133.py
    # or any python; only `git show` is used, no engine.

Writes tests/golden/props/fuse.json (engine_spec="r4133").
"""

from __future__ import annotations

import json
import subprocess
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
OUT = REPO_ROOT / "tests" / "golden" / "props" / "fuse.json"
# The pre-WP-U2.1 0.14.5 golden (branch base) — the rendering baseline the port
# reproduces. `update` tip that wt-u21 branched from.
BASELINE_REF = "1287ec4:tests/golden/props/fuse.json"
SCHEMA = 1


def apply_r4133_deltas(props: dict) -> dict:
    """Overlay the WP-U2.1 r4133 value deltas onto the 0.14.5 dump.

    Deterministic and value-verified against the EPRI r4133 engine (Oddie):
      * FuseCurve default `tlink` -> `none` (explicit curves untouched);
      * RatedCurrent moved default `1` -> `0` (no scenario uses ratedcurrent=1,
        so the substitution only ever hits the old default);
      * new CurveMultiplier (`1.0`) + InterruptingRating (`0`), inserted after
        `State` to match the r4133 property ordinals 11/12.
    """
    out = {}
    for name, val in props.items():
        if name == "FuseCurve" and val.lower() == "tlink":
            val = "none"
        elif name == "RatedCurrent" and val == "1":
            val = "0"
        out[name] = val
        if name == "State":
            out["CurveMultiplier"] = "1.0"
            out["InterruptingRating"] = "0"
    return out


def main() -> None:
    baseline = json.loads(
        subprocess.check_output(["git", "show", BASELINE_REF], cwd=REPO_ROOT, text=True)
    )
    for s in baseline["scenarios"]:
        s["properties"] = apply_r4133_deltas(s["properties"])

    baseline["oracle"] = {
        "engine_spec": "r4133",
        "engine": "Version 11.0.0.1 (64-bit build) - Charlottesville (EPRI OpenDSS r4133, Oddie)",
        "note": (
            "WP-U2.1 Fuse r4133 surface (delta C3/D1): RatedCurrent repurposed to "
            "informational (default 0), new CurveMultiplier (default 1.0, the TCC "
            "divisor) + InterruptingRating (default 0), default FuseCurve tlink->none "
            "(never blows), props 10->12. DERIVED from the retired 0.14.5 golden "
            "(rendering the port reproduces; r4133-EPRI renders state lowercase too), "
            "r4133 value deltas verified on the EPRI r4133 engine (Oddie). See "
            "gen_fuse_r4133.py."
        ),
    }
    OUT.write_text(json.dumps(baseline, indent=1) + "\n")
    print(f"wrote {OUT.relative_to(REPO_ROOT)} ({len(baseline['scenarios'])} scenarios)")


if __name__ == "__main__":
    main()
