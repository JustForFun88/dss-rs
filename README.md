# dss-rs

A 1:1 behavioral port of the Free Pascal **DSS C-API** engine (the
[dss-extensions](https://github.com/dss-extensions) build of EPRI's
[OpenDSS](https://www.epri.com/pages/sa/opendss) distribution-system simulator)
to **pure, safe Rust** — `#![forbid(unsafe_code)]` in every product crate, no C
bindings in the shipped engine, ever (the sole exception is the test-only
`crates/dss-epri` oracle bridge, which drives the official EPRI DLL as a second
live test oracle). The product is a Rust-native library (`dss-core`) plus a CLI;
the sparse solver is pure-Rust [faer](https://github.com/sarah-quinones/faer-rs)
wrapped in `dss-sparse` behind a KLUSolve-shaped API.

## Engine parity

**Engine behavior = OpenDSS 11.0.0.1 (SVN r4133, "Charlottesville") except the
documented ledger.**

The port was first calibrated 1:1 to dss_capi **0.14.5** (OpenDSS SVN r3723,
the pinned numeric oracle in `tools/golden/PIN.txt`) and then moved to the
current official release in two independently-gated rungs
(`UPGRADE_PLAN.md`):

- **Rung 1 (WP-U1.\*)** — parity with the dss_capi `0.15.x` / OpenDSS r4088 line
  (oracle `capi015`). Exited at WP-U1.10.
- **Rung 2 (WP-U2.\*)** — parity with **OpenDSS 11.0.0.1 (r4133)**, spec = the
  official Delphi `r4088 → r4133` diff, oracle = the EPRI binary `oddie:r4133`.
  Exited at WP-U2.6 (2026-07-17): the opt-in
  `DSS_LIVE_OPENDSS=r4133 DSS_LIVE_OPENDSS_ASSERT=1` sweep is **green** — every
  surviving Rust↔r4133 divergence is a deliberate, documented class.

Every deliberate divergence from r4133 (an FPC-vs-Delphi last-ulp / display
floor, a dss_capi property-format difference, or a proven upstream bug we do or
don't reproduce) is catalogued in **`docs/upgrade/DIVERGENCES.md`** and pinned,
case-by-case with measured envelopes, in the **gating divergence ledger
`tests/corpus/ledger.json`**. There are **no "not yet ported" gaps** against
r4133.

## Test gate

The mandatory gate that must be green before any commit:

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

`cargo test` includes the always-on unified live corpus gate
(`crates/dss-core/tests/corpus_gate.rs`), which compiles + solves 514 manifest
cases on the Rust engine and live-compares the full assembled model against
**two oracles**: the pinned dss-python (`tools/golden/PIN.txt`, must be
installed) and the official EPRI r4133 `OpenDSSDirect.dll` (git-tracked at
`tools/opendss/bin/r4133/`, driven by the in-house `crates/dss-epri` bridge —
Windows-only). Measured upstream divergences are pinned in the gating ledger
`tests/corpus/ledger.json`, which fails the gate when stale. See `TESTING.md`.

## Where to read next

- **`PORTING_PLAN.md`** — the authoritative roadmap and binding decisions.
- **`CLAUDE.md`** — conventions (`TODO(compat)`, known upstream bugs, the
  no-tolerance-fudging / prove-the-divergence rules).
- **`docs/plans-archive/UPGRADE_PLAN.md`** — the r3723 → r4133 upgrade, both
  rungs (archived — completed).
- **`TESTING.md`** — the map of the whole test infrastructure.
- **`STATUS.md`** — the living project-status / session-handoff snapshot.
