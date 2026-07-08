# FPC 3.2.2 RTL RNG probe (`mtwist_probe.pas`)

Regeneration probe for the Rust port of the Free Pascal 3.2.2 RTL
Mersenne-Twister RNG (`crates/dss-core/src/support/mathutil/rng.rs` `FpcRng`)
and the `Gauss`/`QuasiLognormal` helpers (`support/mathutil` `gauss` /
`quasi_log_normal`) that the Monte Carlo solve modes consume. It reproduces, on
the **real FPC 3.2.2 RTL** (the same compiler/RTL the pinned oracle's dss_capi
backend is built with), the exact fixed-seed sequences `rng.rs` pins.

**Status: not yet captured in-repo.** The committed pins in `rng.rs` are
**canonical MT19937 ground truth** (cross-verified against `mt19937ar.out` and
CPython's MT19937, below). Equivalence to FPC rests on `FpcRng` being a 1:1
transcription of the RTL `mtwist_*` source — this probe is the on-host path to
*confirm* that empirically, but running it requires an FPC 3.2.2 toolchain that
is not present in every worktree, so no FPC output has been captured here yet.
Run it on such a host to turn the transcription argument into a captured pin.

The RNG is **nondeterministic in the engine** (upstream `Shared/mathutil.pas`
does `initialization Randomize;`, time-seeding per process), so no RNG-carried
value can be golden- or oracle-pinned (GAPS_PLAN.md §2.1). The generator is
therefore gated **only** by the Rust-only fixed-seed unit tests in `rng.rs`.
This probe reproduces those exact expected sequences from a fixed `RandSeed`,
so the pins are regenerable on any FPC 3.2.2 `x86_64-win64` host.

The committed expected values were independently cross-verified against the
canonical `mt19937ar.out` reference and CPython's MT19937 (both share this
exact recurrence + tempering); this probe is the FPC-host regeneration path
(to be run when an FPC host is available), mirroring the
`tools/fpc/fmt_battery` discipline.

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
