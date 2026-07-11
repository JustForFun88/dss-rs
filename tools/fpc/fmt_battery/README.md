# FPC float→ASCII battery (`tests/golden/fmt_battery.csv`)

Pins the Rust port of the FPC 3.2.2 RTL float→string pipeline
(`util::float_to_str`, `util::fmt_g`, `report::format::fpc_sci_w` — the
Grisu1 `str_real` + `FloatToStrF`/`Str(v:w)` post-processing every
Show/Export/Dump/Save report shares) against the **real FPC 3.2.2 RTL** —
the same compiler/RTL the pinned oracle's dss_capi backend is built with.

13 198 deterministic f64 values (directed edge cases: zeros, subnormals,
range endpoints, powers of ten −320…308, the fixed/scientific threshold
band, digit-count and half-away re-round boundaries; plus seeded tie-band
and random batteries) × 7 render forms:

| CSV column | FPC entry point | Rust port |
|---|---|---|
| 2 | `FloatToStr` | `util::float_to_str` |
| 3–6 | `FloatToStrF(v, ffGeneral, N, 0)`, N = 2/5/8/15 | `util::fmt_g(v, N)` |
| 7–8 | `Str(v : W)`, W = 0/14 | `report::format::fpc_sci_w(v, W)` |

Consumed by `crates/dss-core/tests/fmt_battery.rs` (byte-equality on all
92 386 renders). NaN/Inf are out of contract (the engine formats finite
values only; NaN is caught earlier by `float_to_str_ex` → `----`).

## Regeneration — manual only (same rule as all goldens)

Requires FPC 3.2.2 with the x86_64-win64 cross-compiler (`ppcrossx64`),
e.g. `C:\FPC\3.2.2\bin\i386-Win32\ppcrossx64.exe`:

```
python gen_values.py > values.txt
ppcrossx64 -O2 fmt_battery.pas
./fmt_battery.exe          # values.txt -> fmt_battery.csv
cp fmt_battery.csv ../../../tests/golden/fmt_battery.csv
```

`values.txt`, the `.exe`/`.o` and the produced CSV are build artifacts —
only `gen_values.py`, `fmt_battery.pas`, this README and the golden CSV
under `tests/golden/` are committed.
