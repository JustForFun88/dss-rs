"""Regenerate the non-memory-mapped file-backed array fixtures for
tests/corpus/modes/inputformat/shape_filearr/ (WPG.19).

Committed deterministic inputs (not goldens). These exercise the Pascal
`InterpretDblArray` file grammar (`Common/Utilities.pas:461-566`) for array
properties WITHOUT `MemoryMapping=Yes` — the last blocker of the ckt24
MemoryMappingLoadShapes family (LoadShape LS_PhaseB/C use `mult=(file=…) ln`):

  fa_c8.csv    -- 8 single-column mult values         (mult=(file=…))
  fa_2col.csv  -- 8 rows "hour,mult"                  (mult=(file=…, column=2))
  fa_hdr.csv   -- header line + 8 single-column mults (mult=(file=…, header=yes))
  fa8.sng      -- 8 x float32 mults                   (mult=(sngfile=…), widened)
  fa8.dbl      -- 8 x float64 mults                   (mult=(dblfile=…))
  fa5.csv      -- 5 single-column mults (npts=8 → NumPoints shrinks to 5)
  fa_norm.csv  -- 8 mults whose peak is 2.0     (mult=(file=…) action=normalize → /2)
  spec_mag.csv -- 4 Spectrum %mag values              (generic DoubleArray file=)
  xy_y.csv     -- 4 XYcurve Yarray values             (generic DoubleArray file=)

Feature sensitivity (recorded in manifest.note): every LoadShape's daily power
depends on the read multipliers, so deleting a `mult=(file=…)` directive (or the
`action=normalize`) changes the load power the monitors capture; the short file shrinks
`NumPoints` (8→5) and `fa_norm` divides by the peak (2.0). Regenerate via
`python tools/decks/gen_shape_filearr_fixtures.py`.
"""

import os
import struct

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.normpath(
    os.path.join(HERE, "..", "..", "tests", "corpus", "modes", "inputformat", "shape_filearr")
)

MULT = [0.40, 0.55, 0.75, 0.95, 1.00, 0.90, 0.70, 0.50]
# peak 2.0 so `ln` (normalize) divides by 2.0 — visibly != identity.
NORM = [0.50, 1.00, 2.00, 1.50, 0.80, 0.40, 1.20, 0.60]
SPEC_MAG = [100.0, 33.0, 20.0, 14.0]
XY_Y = [1.00, 0.90, 0.80, 0.70]


def w(name, text):
    with open(os.path.join(OUT, name), "w", newline="\n") as f:
        f.write(text)


def wb(name, data):
    with open(os.path.join(OUT, name), "wb") as f:
        f.write(data)


def main() -> None:
    os.makedirs(OUT, exist_ok=True)
    w("fa_c8.csv", "".join(f"{v}\n" for v in MULT))
    w("fa_2col.csv", "".join(f"{i},{v}\n" for i, v in enumerate(MULT)))
    w("fa_hdr.csv", "hour_mult\n" + "".join(f"{v}\n" for v in MULT))
    wb("fa8.sng", struct.pack("<8f", *MULT))
    wb("fa8.dbl", struct.pack("<8d", *MULT))
    w("fa5.csv", "".join(f"{v}\n" for v in MULT[:5]))
    w("fa_norm.csv", "".join(f"{v}\n" for v in NORM))
    w("spec_mag.csv", "".join(f"{v}\n" for v in SPEC_MAG))
    w("xy_y.csv", "".join(f"{v}\n" for v in XY_Y))
    print(f"wrote fixtures to {OUT}")


if __name__ == "__main__":
    main()
