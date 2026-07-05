"""Regenerate the binary/CSV shape fixture files for tests/corpus/gaps/shape_binfiles.dss.

The fixtures are committed (they are deterministic inputs, not goldens); this
script only exists so they can be rebuilt from source if ever needed. Layouts
follow the Pascal readers (LoadShape.pas `DoSngFile`/`DoDblFile`, interval<>0
branch = a bare value stream, little-endian):
  ls8.sng    -- 8 x float32 P multipliers      (LoadShape SngFile)
  ls8.dbl    -- 8 x float64 P multipliers      (LoadShape DblFile)
  lspq8.csv  -- 8 x "P, Q" rows                (LoadShape PQCSVFile)
  t8.sng     -- 8 x float32 temperatures       (TShape SngFile)
  p8.dbl     -- 8 x float64 prices             (PriceShape DblFile)
  g4.csv     -- 4 x "year, mult" rows          (GrowthShape CSVFile)
"""
import os
import struct

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.normpath(os.path.join(HERE, "..", "..", "tests", "corpus", "gaps"))

MULT = [0.40, 0.55, 0.75, 0.95, 1.00, 0.90, 0.70, 0.50]
QMULT = [0.30, 0.40, 0.55, 0.70, 0.75, 0.68, 0.52, 0.38]
TEMPS = [18.0, 19.5, 22.0, 26.5, 29.0, 27.5, 24.0, 20.5]
PRICE = [32.0, 30.5, 41.0, 55.5, 62.0, 58.5, 44.0, 35.5]
GROWTH = [(2000, 1.02), (2005, 1.025), (2010, 1.01), (2020, 1.0)]


def main() -> None:
    with open(os.path.join(OUT, "ls8.sng"), "wb") as f:
        f.write(struct.pack("<8f", *MULT))
    with open(os.path.join(OUT, "ls8.dbl"), "wb") as f:
        f.write(struct.pack("<8d", *MULT))
    with open(os.path.join(OUT, "lspq8.csv"), "w", newline="\n") as f:
        for p, q in zip(MULT, QMULT):
            f.write(f"{p}, {q}\n")
    with open(os.path.join(OUT, "t8.sng"), "wb") as f:
        f.write(struct.pack("<8f", *TEMPS))
    with open(os.path.join(OUT, "p8.dbl"), "wb") as f:
        f.write(struct.pack("<8d", *PRICE))
    with open(os.path.join(OUT, "g4.csv"), "w", newline="\n") as f:
        for yr, m in GROWTH:
            f.write(f"{yr}, {m}\n")
    print("fixtures written to", OUT)


if __name__ == "__main__":
    main()
