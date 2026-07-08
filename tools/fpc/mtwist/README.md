# FPC 3.2.2 RTL RNG probe (`mtwist_probe.pas`)

Regeneration + confirmation probe for the Rust port of the Free Pascal 3.2.2 RTL
Mersenne-Twister RNG (`crates/dss-core/src/support/mathutil/rng.rs` `FpcRng`)
and the `Gauss`/`QuasiLognormal` helpers (`support/mathutil` `gauss` /
`quasi_log_normal`) that the Monte Carlo solve modes consume. It reproduces, on
the **real FPC 3.2.2 RTL** (the same compiler/RTL the pinned oracle's dss_capi
backend is built with), the exact fixed-seed sequences `rng.rs` pins.

**Status: CAPTURED & CONFIRMED (2026-07-08).** Built with `ppcrossx64`
(FPC 3.2.2 `x86_64-win64`) and run; **all 20 pins matched `rng.rs` bit-for-bit**
— the 8 u32 (seed 12345), the seed-1 first draw (1791095845), the 8
`Random:Double` bit patterns, the 4 `Gauss(0,1)`, the 2 `Gauss(2.5,0.5)`, and
the 2 `QuasiLognormal(3.0)`. The captured output is committed as
`fpc_output_x86_64_win64.txt`. So the pins are no longer "canonical-MT assumed
== FPC"; they are captured-FPC-exact.

The pins were *first* derived as canonical MT19937 ground truth (cross-verified
against `mt19937ar.out` and CPython's MT19937 — both share this exact recurrence
+ tempering; the seed-1 = 1791095845 match is the canonical tell), which the
FPC capture then confirmed.

The RNG is **nondeterministic in the engine** (upstream `Shared/mathutil.pas`
does `initialization Randomize;`, time-seeding per process), so no RNG-carried
value can be golden- or oracle-pinned (GAPS_PLAN.md §2.1). The generator is
therefore gated **only** by the Rust-only fixed-seed unit tests in `rng.rs`
(+ the RNG-dispatch tests in `monte_carlo.rs`/`load`/`fault`); this probe backs
those pins with real FPC output.

## Self-contained

The probe uses only the RTL `Random` for the MT core (identical to dss_capi),
and copies `Gauss`/`QuasiLognormal` **verbatim** from
`.inputs/dss_capi/src/Shared/mathutil.pas:292/:304` — so it compiles with **no
dss_capi unit dependency**. The verbatim copy was confirmed bit-identical to the
real `Mathutil` unit's output.

## Regeneration — manual only (same rule as all goldens)

Requires FPC 3.2.2 with the x86_64-win64 cross-compiler (`ppcrossx64`).
**The target arch matters:** `Gauss`/`QuasiLognormal` sum 12 `Random` draws in
f64, so build **x86_64/SSE2** (matching the oracle's backend) — `i386-win32`
(x87, 80-bit intermediates) would diverge on those doubles.

```
ppcrossx64 -O2 mtwist_probe.pas
./mtwist_probe.exe          # compare against fpc_output_x86_64_win64.txt
```

Each `Double` is printed as its raw 64-bit little-endian pattern (hex) so the
comparison is bit-exact, matching the `f64::to_bits()` assertions in `rng.rs`.
The `.exe`/`.o` are build artifacts — only `mtwist_probe.pas`,
`fpc_output_x86_64_win64.txt`, and this README are committed.
