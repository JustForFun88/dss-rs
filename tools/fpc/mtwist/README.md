# FPC 3.2.2 RTL RNG probe (`mtwist_probe.pas`)

Pins the Rust port of the Free Pascal 3.2.2 RTL Mersenne-Twister RNG
(`crates/dss-core/src/support/mathutil/rng.rs` `FpcRng`) and the
`Gauss`/`QuasiLognormal` helpers (`support/mathutil` `gauss` /
`quasi_log_normal`) that the Monte Carlo solve modes consume — against the
**real FPC 3.2.2 RTL**, the same compiler/RTL the pinned oracle's dss_capi
backend is built with.

The RNG is **nondeterministic in the engine** (upstream `Shared/mathutil.pas`
does `initialization Randomize;`, time-seeding per process), so no RNG-carried
value can be golden- or oracle-pinned (GAPS_PLAN.md §2.1). The generator is
therefore gated **only** by the Rust-only fixed-seed unit tests in `rng.rs`.
This probe reproduces those exact expected sequences from a fixed `RandSeed`,
so the pins are regenerable on any FPC 3.2.2 `x86_64-win64` host.

The committed expected values were independently cross-verified against the
canonical `mt19937ar.out` reference and CPython's MT19937 (both share this
exact recurrence + tempering); this probe is the FPC-host regeneration path,
mirroring the `tools/fpc/fmt_battery` discipline.

## Regeneration — manual only (same rule as all goldens)

Requires FPC 3.2.2 with the x86_64-win64 cross-compiler (`ppcrossx64`):

```
ppcrossx64 -O2 mtwist_probe.pas
./mtwist_probe.exe
```

Each `Double` is printed as its raw 64-bit little-endian pattern (hex) so the
comparison is bit-exact, matching the `f64::to_bits()` assertions in `rng.rs`.
The `.exe`/`.o` are build artifacts — only `mtwist_probe.pas` and this README
are committed.
