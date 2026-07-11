"""Regenerate the memory-mapped shape fixtures for tests/corpus/modes/shape_mmf/.

Committed deterministic inputs (not goldens). The LoadShapes read them under
`MemoryMapping=Yes` (WPG.17): the engine maps the file and reads records lazily
through the map (Pascal `InterpretDblArrayMMF`); the Rust port reads them
eagerly with identical record semantics.

  mm8.sng    -- 8 x float32 P multipliers      (sngfile= under MMF, interval=1)
  mm8.dbl    -- 8 x float64 P multipliers      (dblfile= under MMF, interval=1)
  mmpq8.csv  -- 8 x "P, Q" rows where P is written in EXPONENT notation. This
                is the feature-sensitivity axis: the MMF text reader's accept-set
                keeps only bytes [46,58) (`.`/`/`/digits), dropping the sign and
                the `e` exponent, so `1.5e-1` reads as 1.51 under MMF but as 0.15
                without it. All lines are the SAME byte width (the MMF byte
                indexer requires it); spaces pad and are dropped by the accept-set.

Feature sensitivity (recorded in manifest.note): flipping `MemoryMapping=Yes` →
`No` on the pq shape changes P from {1.51,2.01,…} to {0.15,0.20,…} — a large,
oracle-observable divergence in the ld_pq load power (the sng/dbl shapes are
fixed-interval, so MMF is I/O-only and numerically identical to non-MM there —
they cover the sng/dbl MMF readers; the pq shape provides the numeric axis).
"""

import os
import struct

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.normpath(
    os.path.join(HERE, "..", "..", "tests", "corpus", "modes", "shape_mmf")
)

MULT = [0.40, 0.55, 0.75, 0.95, 1.00, 0.90, 0.70, 0.50]
# P in exponent notation (MMF accept-set drops sign/exponent → "X.Y1"); Q plain.
PQ_P_EXP = ["1.5e-1", "2.0e-1", "2.5e-1", "3.0e-1", "3.5e-1", "4.0e-1", "4.5e-1", "5.0e-1"]
PQ_Q = [0.30, 0.35, 0.40, 0.45, 0.50, 0.55, 0.60, 0.65]


def main() -> None:
    os.makedirs(OUT, exist_ok=True)
    with open(os.path.join(OUT, "mm8.sng"), "wb") as f:
        f.write(struct.pack("<8f", *MULT))
    with open(os.path.join(OUT, "mm8.dbl"), "wb") as f:
        f.write(struct.pack("<8d", *MULT))
    rows = [f"{p},{q:.2f}" for p, q in zip(PQ_P_EXP, PQ_Q)]
    width = max(len(r) for r in rows)
    rows = [r.ljust(width) for r in rows]  # uniform byte width for the MMF indexer
    with open(os.path.join(OUT, "mmpq8.csv"), "w", newline="\n") as f:
        for r in rows:
            f.write(r + "\n")
    print(f"wrote fixtures to {OUT}")


if __name__ == "__main__":
    main()
