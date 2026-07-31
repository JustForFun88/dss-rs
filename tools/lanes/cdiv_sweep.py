#!/usr/bin/env python3
"""Re-derive the evidence behind the Stage F **complex-division** no-split row.

`DE_PASCALIZE_PLAN.md` Part IV.2 resolves `cdiv` to a single shared kernel
(FPC/Smith) rather than a lane split, on the grounds that the naive
`(a*conj(b))/|b|^2` form is the *less* accurate and less robust of the two. That
verdict is asserted in-tree by `crates/dss-core/src/compat/tests.rs`
(`cdiv_shared_kernel_is_the_more_accurate_one`,
`naive_division_collapses_where_smith_stays_exact`), but the measurement behind
its reference table lived only in prose. This script is that measurement, so the
row can be re-run instead of believed.

Two outputs:

* ``references`` — the correctly-rounded quotient for each row of
  ``DIV_REFERENCE``, as the ``f64::from_bits`` literals the test carries.
  **Derived from the operands' exact binary values** (`Fraction(float)`), not
  from their decimal spellings: `Decimal('0.002321729484')` is not the double
  the kernel receives, and that conversion's own half-ULP slip is the same order
  as the quantity being measured. Deriving them the wrong way is what put three
  of the six rows 1 ULP off — every error in the kept kernel's favour, scoring
  the table 1 vs 14 ULP where the truth is 5 vs 12 (found in the wave-4
  settlement, 2026-08-01).

* ``sweep`` — an **unfiltered** random sweep, which is what actually settles the
  row. The committed table is a disagreement sample filtered to Smith wins, so
  "Smith is at least as close on every row" is a property of that table and not
  of the kernels: over 20 000 uniform pairs Smith is strictly *worse* on ~25%.
  What holds is the aggregate (mean and worst relative error) plus the
  robustness half no sample can average away.

Usage:
    python tools/lanes/cdiv_sweep.py references
    python tools/lanes/cdiv_sweep.py sweep [N] [seed]
"""

from __future__ import annotations

import random
import struct
import sys
from fractions import Fraction

# The six operand pairs of `DIV_REFERENCE`, spelled exactly as the Rust test
# spells them; `float()` here reproduces the doubles the kernel receives.
DIV_CASES = [
    ((0.002321729484, -0.006576942029), (4.635502e-06, -1.7351297e-05)),
    ((-4.593386e-06, -6.59001e-07), (-1.056509107804, -1.005279277728)),
    ((-622.258737287941, 3943.574810490002), (-190543.68643387672, 87389.84932548525)),
    ((37.525861080188, -25.109089063361), (-188746.93835912598, 278724.15562414465)),
    ((2.50311267631, 2.853591214084), (-17667.064091758657, -6753.628705423371)),
    ((-0.000102367606, -0.000286357051), (-8.8600379e-05, -6.1824964e-05)),
]


def bits(x: float) -> int:
    return struct.unpack("<Q", struct.pack("<d", x))[0]


def ulp_distance(a: float, b: float) -> int:
    ia = struct.unpack("<q", struct.pack("<d", a))[0]
    ib = struct.unpack("<q", struct.pack("<d", b))[0]
    return abs(ia - ib)


def exact_quotient(a: tuple[float, float], b: tuple[float, float]) -> tuple[float, float]:
    """(a / b) in exact rationals, each part rounded once to f64."""
    ar, ai = Fraction(a[0]), Fraction(a[1])
    br, bi = Fraction(b[0]), Fraction(b[1])
    den = br * br + bi * bi
    return float((ar * br + ai * bi) / den), float((ai * br - ar * bi) / den)


def smith(a: tuple[float, float], b: tuple[float, float]) -> tuple[float, float]:
    """FPC `ucomplex` / C99 `_Cdivd` / LAPACK `dladiv` — the parity kernel."""
    ar, ai = a
    br, bi = b
    if abs(br) >= abs(bi):
        r = bi / br
        den = br + bi * r
        return (ar + ai * r) / den, (ai - ar * r) / den
    r = br / bi
    den = br * r + bi
    return (ar * r + ai) / den, (ai * r - ar) / den


def naive(a: tuple[float, float], b: tuple[float, float]) -> tuple[float, float]:
    """`num_complex`'s `/` — the rejected candidate."""
    ar, ai = a
    br, bi = b
    den = br * br + bi * bi
    return (ar * br + ai * bi) / den, (ai * br - ar * bi) / den


def cmd_references() -> None:
    smith_total = naive_total = 0
    print("row |  Smith ULP | naive ULP | reference (re, im) as from_bits")
    for k, (a, b) in enumerate(DIV_CASES):
        er, ei = exact_quotient(a, b)
        s = smith(a, b)
        n = naive(a, b)
        ds = ulp_distance(s[0], er) + ulp_distance(s[1], ei)
        dn = ulp_distance(n[0], er) + ulp_distance(n[1], ei)
        smith_total += ds
        naive_total += dn
        print(f"{k:>3} | {ds:>10} | {dn:>9} | 0x{bits(er):016x}, 0x{bits(ei):016x}")
    print(f"\ntotal: Smith {smith_total} ULP, naive {naive_total} ULP")
    print("(the test pins this pair; per-row `ds <= dn` holds on this filtered")
    print(" table only — see `sweep` for the kernels' real behaviour)")


def cmd_sweep(n: int, seed: int) -> None:
    random.seed(seed)
    disagree = worse = 0
    s_sum = n_sum = 0.0
    s_max = n_max = 0.0

    def rand() -> float:
        return random.choice([1, -1]) * 10 ** random.uniform(-6, 6)

    for _ in range(n):
        a = (rand(), rand())
        b = (rand(), rand())
        er, ei = exact_quotient(a, b)
        s = smith(a, b)
        v = naive(a, b)
        ds = ulp_distance(s[0], er) + ulp_distance(s[1], ei)
        dn = ulp_distance(v[0], er) + ulp_distance(v[1], ei)
        if ds != dn:
            disagree += 1
        if ds > dn:
            worse += 1
        mag = (er * er + ei * ei) ** 0.5
        if mag > 0:
            es = ((s[0] - er) ** 2 + (s[1] - ei) ** 2) ** 0.5 / mag
            en = ((v[0] - er) ** 2 + (v[1] - ei) ** 2) ** 0.5 / mag
            s_sum += es
            n_sum += en
            s_max = max(s_max, es)
            n_max = max(n_max, en)

    print(f"pairs {n}, seed {seed}")
    print(f"  kernels disagree      : {disagree}")
    print(f"  Smith strictly worse  : {worse} ({100 * worse / n:.1f}%)")
    print(f"  Smith  mean {s_sum / n:.3e}   worst {s_max:.3e}")
    print(f"  naive  mean {n_sum / n:.3e}   worst {n_max:.3e}")
    print("\nThe aggregate is the verdict; the robustness half is separate —")
    print("the naive form squares the denominator, so it returns 0/NaN outside")
    print("|den| in [sqrt(DBL_MIN), sqrt(DBL_MAX)] where Smith stays exact.")


def main() -> int:
    args = sys.argv[1:]
    if not args or args[0] not in ("references", "sweep"):
        print(__doc__)
        return 2
    if args[0] == "references":
        cmd_references()
    else:
        n = int(args[1]) if len(args) > 1 else 20000
        seed = int(args[2]) if len(args) > 2 else 20260801
        cmd_sweep(n, seed)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
