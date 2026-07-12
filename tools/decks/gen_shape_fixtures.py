"""Regenerate the binary/CSV shape fixtures for tests/corpus/modes/inputformat/shape_binfiles/.

The fixtures are committed (they are deterministic inputs, not goldens); this
script only exists so they can be rebuilt from source if ever needed. Layouts
follow the Pascal readers (LoadShape.pas `DoSngFile`/`DoDblFile`, interval<>0
branch = a bare value stream, little-endian):
  ls8.sng    -- 8 x float32 P multipliers      (LoadShape SngFile, interval=1)
  ls8.dbl    -- 8 x float64 P multipliers      (LoadShape DblFile, interval=1)
  ls8v.sng   -- 8 x float32 (hour, mult) pairs (LoadShape SngFile, interval=0
                — WPG.1's "port both branches" case; hours 0..7 line up
                exactly with the daily solve's sample hours, so the lookup
                takes the exact-point branch on both engines, never the
                interpolation branch, which is the only place a single-
                precision-only curve could diverge from double-precision
                arithmetic — see load_shape/compute.rs::read_sng_file)
  lspq8.csv  -- 8 x "P, Q" rows                (LoadShape PQCSVFile)
  t8.sng     -- 8 x float32 temperatures       (TShape SngFile)
  p8.dbl     -- 8 x float64 prices             (PriceShape DblFile)
  g4.csv     -- 4 x "year, mult" rows          (GrowthShape CSVFile)

WPG.1 audit follow-up additions (2026-07-07):
  ls8vi.sng  -- 8 x float32 (hour, mult) pairs, hours 0,2,..,14 and full-
                significand mults: the daily solve's odd sample hours fall
                BETWEEN anchors, so the lookup takes the single-precision
                interpolation branch (GetMultAtHourSingle) — the one path the
                original exact-point deck deliberately avoided; the live
                compare now pins Pascal's f32 storage arithmetic end-to-end
                (plus Mean/StdDev probes through RCD/Curve*Single).
  t8v.sng    -- 8 x float32 (hour, temp) pairs (TShape SngFile, interval=0 —
                the ScalarShapeCore pair branch, previously unit-only)
  g5.csv     -- 4 x fractional "year, mult" rows (GrowthShape CSVFile keeps
                fractional years: Pascal DoCSVFile ignores RoundA — the
                rounding loop exists only in DoSngFile/DoDblFile)
  g4s.sng    -- 4 x float32 (year, mult) pairs (GrowthShape SngFile — DOES
                round the year column, previously unit-only)
"""
import os
import struct

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.normpath(
    os.path.join(HERE, "..", "..", "tests", "corpus", "modes", "inputformat", "shape_binfiles")
)

MULT = [0.40, 0.55, 0.75, 0.95, 1.00, 0.90, 0.70, 0.50]
QMULT = [0.30, 0.40, 0.55, 0.70, 0.75, 0.68, 0.52, 0.38]
TEMPS = [18.0, 19.5, 22.0, 26.5, 29.0, 27.5, 24.0, 20.5]
PRICE = [32.0, 30.5, 41.0, 55.5, 62.0, 58.5, 44.0, 35.5]
GROWTH = [(2000, 1.02), (2005, 1.025), (2010, 1.01), (2020, 1.0)]

# Full f32 significands so f32-vs-f64 interpolation arithmetic is observable.
MULT_I = [
    1.0 / 3.0, 0.123456789, 0.777777777, 0.987654321,
    0.555555555, 0.246813579, 0.135791357, 0.864208642,
]
GROWTH_F = [(2000.6, 1.02), (2005.4, 1.025), (2010.7, 1.01), (2020.2, 1.0)]


def main() -> None:
    with open(os.path.join(OUT, "ls8.sng"), "wb") as f:
        f.write(struct.pack("<8f", *MULT))
    with open(os.path.join(OUT, "ls8.dbl"), "wb") as f:
        f.write(struct.pack("<8d", *MULT))
    with open(os.path.join(OUT, "ls8v.sng"), "wb") as f:
        for h, m in enumerate(MULT):
            f.write(struct.pack("<ff", float(h), m))
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
    with open(os.path.join(OUT, "ls8vi.sng"), "wb") as f:
        for i, m in enumerate(MULT_I):
            f.write(struct.pack("<ff", float(2 * i), m))
    with open(os.path.join(OUT, "t8v.sng"), "wb") as f:
        for i, t in enumerate(TEMPS):
            f.write(struct.pack("<ff", float(2 * i), t))
    with open(os.path.join(OUT, "g5.csv"), "w", newline="\n") as f:
        for yr, m in GROWTH_F:
            f.write(f"{yr}, {m}\n")
    with open(os.path.join(OUT, "g4s.sng"), "wb") as f:
        for yr, m in GROWTH_F:
            f.write(struct.pack("<ff", yr, m))
    print("fixtures written to", OUT)


if __name__ == "__main__":
    main()
