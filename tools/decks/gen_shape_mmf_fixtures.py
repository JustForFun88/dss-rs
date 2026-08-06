"""Regenerate the memory-mapped shape fixtures for the two `shape_mmf*` decks.

Committed deterministic inputs (not goldens). The LoadShapes read them under
`MemoryMapping=Yes` (WPG.17): the engine maps the file and reads records lazily
through the map (Pascal `InterpretDblArrayMMF`); the Rust port reads them
eagerly with identical record semantics.

tests/corpus/modes/inputformat/shape_mmf/ -- the quirk-observing deck:

  mm8.sng    -- 8 x float32 P multipliers      (sngfile= under MMF, interval=1)
  mm8.dbl    -- 8 x float64 P multipliers      (dblfile= under MMF, interval=1)
  mmpq8.csv  -- 8 x "P, Q" rows where P is written in EXPONENT notation. This is
                the feature-sensitivity axis: BOTH oracle revisions filter the
                MMF text column through an accept-set of bytes [46,58)
                (`.`/`/`/digits), dropping the sign and the `e` exponent, so they
                read `1.5e-1` as 1.51 where the file says 0.15. GOLDEN_REBASE
                G2.5 fixed that in the port (both lanes), so this deck now
                diverges from the pinned oracle wholesale and is ledgered
                (`mmf-accept-set-honoured-capi`). All lines are the SAME byte
                width (the MMF byte indexer requires it); spaces pad.

tests/corpus/modes/inputformat/shape_mmf_io/ -- the sibling that keeps the
MMF-reader coverage oracle-compared (added by G2.5 before excluding the deck
above). Same shapes, same loads, same monitors; only the pq fixture differs:

  mm8.sng, mm8.dbl -- byte-identical copies of the pair above.
  mmpq8_plain.csv  -- the same P/Q numbers written in PLAIN decimal, so every
                      byte is inside the upstream accept-set and the two engines
                      read it identically. Keeping it that way is asserted by
                      `load_shape::tests::mmf_accept_set_fix_is_gated_by_exactly_one_deck`.
"""

import os
import shutil
import struct

HERE = os.path.dirname(os.path.abspath(__file__))
CORPUS = os.path.normpath(
    os.path.join(HERE, "..", "..", "tests", "corpus", "modes", "inputformat")
)
OUT = os.path.join(CORPUS, "shape_mmf")
OUT_IO = os.path.join(CORPUS, "shape_mmf_io")

MULT = [0.40, 0.55, 0.75, 0.95, 1.00, 0.90, 0.70, 0.50]
# P in exponent notation (MMF accept-set drops sign/exponent → "X.Y1"); Q plain.
PQ_P_EXP = ["1.5e-1", "2.0e-1", "2.5e-1", "3.0e-1", "3.5e-1", "4.0e-1", "4.5e-1", "5.0e-1"]
PQ_P_PLAIN = [0.15, 0.20, 0.25, 0.30, 0.35, 0.40, 0.45, 0.50]
PQ_Q = [0.30, 0.35, 0.40, 0.45, 0.50, 0.55, 0.60, 0.65]


def write_rows(path: str, rows: list) -> None:
    width = max(len(r) for r in rows)
    rows = [r.ljust(width) for r in rows]  # uniform byte width for the MMF indexer
    with open(path, "w", newline="\n") as f:
        for r in rows:
            f.write(r + "\n")


def main() -> None:
    os.makedirs(OUT, exist_ok=True)
    os.makedirs(OUT_IO, exist_ok=True)
    with open(os.path.join(OUT, "mm8.sng"), "wb") as f:
        f.write(struct.pack("<8f", *MULT))
    with open(os.path.join(OUT, "mm8.dbl"), "wb") as f:
        f.write(struct.pack("<8d", *MULT))
    write_rows(
        os.path.join(OUT, "mmpq8.csv"),
        [f"{p},{q:.2f}" for p, q in zip(PQ_P_EXP, PQ_Q)],
    )
    for name in ("mm8.sng", "mm8.dbl"):
        shutil.copyfile(os.path.join(OUT, name), os.path.join(OUT_IO, name))
    write_rows(
        os.path.join(OUT_IO, "mmpq8_plain.csv"),
        [f"{p:.2f},{q:.2f}" for p, q in zip(PQ_P_PLAIN, PQ_Q)],
    )
    print(f"wrote fixtures to {OUT} and {OUT_IO}")


if __name__ == "__main__":
    main()
