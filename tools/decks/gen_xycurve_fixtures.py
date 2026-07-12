"""Regenerate the CSV/binary XYcurve fixtures for tests/corpus/modes/inputformat/xycurve_files/.

The fixtures are committed (deterministic inputs, not goldens); this script only
exists so they can be rebuilt from source if ever needed. Layouts follow the
Pascal readers (Common/Utilities.pas `DoCSVFile`/`DoSngFile`/`DoDblFile`, with
`pA=XValues`, `pB=YValues`, `OnlyLoadB=False`, little-endian):
  rc.csv    -- 4 x "x, y" rows              (XYcurve CSVFile, an R-vs-freq curve)
  lc.sng    -- 4 x float32 (x, y) pairs     (XYcurve SngFile, an L-vs-freq curve)
  rc2.dbl   -- 4 x float64 (x, y) pairs     (XYcurve DblFile, a 2nd R-vs-freq curve)

The x column is the harmonic frequency (Hz); the y column is the R (or L) scale.
The CSV writer uses newline="\n" (cf. gen_shape_fixtures.py) so the byte-position
read guard is reproducible across platforms.
"""
import os
import struct

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.normpath(
    os.path.join(HERE, "..", "..", "tests", "corpus", "modes", "inputformat", "xycurve_files")
)

# R-vs-frequency (csv) — same values reactor_rlcurve validated for rvsf.
RC = [(60.0, 1.0), (180.0, 1.4), (300.0, 2.1), (420.0, 3.0)]
# L-vs-frequency (sng) — same values reactor_rlcurve validated for lvsf.
LC = [(60.0, 1.0), (180.0, 0.95), (300.0, 0.88), (420.0, 0.80)]
# A 2nd R-vs-frequency (dbl) for the second reactor.
RC2 = [(60.0, 1.0), (180.0, 1.2), (300.0, 1.5), (420.0, 2.0)]


def main() -> None:
    os.makedirs(OUT, exist_ok=True)
    with open(os.path.join(OUT, "rc.csv"), "w", newline="\n") as f:
        for x, y in RC:
            f.write(f"{x}, {y}\n")
    with open(os.path.join(OUT, "lc.sng"), "wb") as f:
        for x, y in LC:
            f.write(struct.pack("<ff", x, y))
    with open(os.path.join(OUT, "rc2.dbl"), "wb") as f:
        for x, y in RC2:
            f.write(struct.pack("<dd", x, y))
    print("fixtures written to", OUT)


if __name__ == "__main__":
    main()
