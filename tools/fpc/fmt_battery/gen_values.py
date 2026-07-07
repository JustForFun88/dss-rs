"""Deterministic f64 battery for the FPC float->ASCII golden
(`tests/golden/fmt_battery.csv`).

Emits `values.txt`: one lowercase hex u64 bit pattern per line. The paired
`fmt_battery.pas` (compiled with the SAME FPC 3.2.2 the pinned oracle's
dss_capi backend is built with) renders each value through the RTL entry
points the Rust port mirrors:

  FloatToStr            -> util::float_to_str        (15-sig FloatToStrF)
  FloatToStrF ffGeneral -> util::fmt_g(v, sig)        (Format('%-.Ng'))
  Str(v:w)              -> report::format::fpc_sci_w  (width-form scientific)

Regenerate manually only (same rule as all goldens):
  python gen_values.py > values.txt
  ppcrossx64 -O2 fmt_battery.pas && fmt_battery.exe   # values.txt -> fmt_battery.csv
"""

import math
import random
import struct

random.seed(20260707)

out = []
seen = set()


def emit(v: float) -> None:
    if isinstance(v, float) and (math.isnan(v) or math.isinf(v)):
        return  # out of contract: engine formats finite values only
    bits = struct.unpack("<Q", struct.pack("<d", v))[0]
    if bits in seen:
        return
    seen.add(bits)
    out.append(bits)


# --- directed: zeros, subnormal range, normal-range endpoints ---------------
emit(0.0)
emit(-0.0)
emit(5e-324)                       # min subnormal
emit(2.2250738585072014e-308)      # min normal
emit(2.225073858507201e-308)       # max subnormal
emit(1.7976931348623157e308)       # max finite
emit(4.9406564584124654e-324)

# --- directed: powers of ten across the full range (both signs) -------------
for e in range(-320, 309):
    for s in (1.0, -1.0):
        emit(s * float(f"1e{e}"))

# --- directed: the fixed/scientific threshold band (FPC keeps fixed to 1e-5) -
for m in (1.0, 1.2345678901234567, 9.99999999999999, 5.0000000000000004):
    for e in range(-9, 4):
        emit(m * 10.0**e)
        emit(-m * 10.0**e)

# --- directed: digit-count / rounding boundaries -----------------------------
for v in (
    0.8258283333333335,   # the TODO(compat) half-away re-round pin
    999999.5, 999999.4999999999, 0.99999999999999994,
    1e15 - 1.0, 1e15, 1e16 - 2.0, 1e16, 1e17,
    123456789012345.6, 1234567890123456.7, 12345678901234567.0,
    2.53510362191938e-12, 7.2e-8, 0.00001234, 1e-6, 0.001,
):
    emit(v)
    emit(-v)

# --- tie-band: force a '5' + zeros tail at several significant positions ----
for _ in range(2000):
    v = random.uniform(0.1, 10.0) * 10.0 ** random.randint(-12, 12)
    s = f"{v:.17e}"
    mant, exp = s.split("e")
    for cut in (14, 15, 16):
        forced = mant[: cut + 1] + "5"  # sign/dot offset is close enough: tie-ish band
        emit(float(forced + "e" + exp))

# --- random: uniform bit patterns (finite only) ------------------------------
n = 0
while n < 3000:
    bits = random.getrandbits(64)
    (v,) = struct.unpack("<d", struct.pack("<Q", bits))
    if math.isnan(v) or math.isinf(v):
        continue
    emit(v)
    n += 1

# --- random: human-scale magnitudes ------------------------------------------
for _ in range(3000):
    emit(random.uniform(-1.0, 1.0) * 10.0 ** random.randint(-30, 30))

for bits in out:
    print(f"{bits:016x}")
