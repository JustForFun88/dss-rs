# dss-rs — Pascal → Rust port of DSS C-API (OpenDSS engine)

1:1 behavioral port of the Free Pascal "DSS C-API" engine (vendored at
`.inputs/dss_capi`) to pure safe Rust. **Read `PORTING_PLAN.md` first** — it is the
authoritative roadmap and encodes binding decisions:

- `#![forbid(unsafe_code)]` in every crate; no C bindings ever.
- Sparse solver is pure-Rust **faer**, wrapped in `dss-sparse` behind a
  KLUSolve-shaped API.
- No C-API export layer; the product is a Rust-native library (`dss-core`) + CLI.
- The numeric oracle is **dss-python pinned in `tools/golden/PIN.txt`**
  (0.15.7, backend = dss_capi 0.14.5 — the exact vendored Pascal source).
  Goldens live in `tests/golden/`; regenerate only manually, with the pinned
  versions, via `tools/golden/*.py`.
- Later phases may freely refactor earlier code; passing tests are the only contract.

## `TODO(compat)` convention (see PORTING_PLAN.md §4.1)

Every place where we deliberately reproduce an upstream inexactness or bug
(truncated `pi = 3.14159265359`, `rad→deg = 57.29577951`, FPC `Round`'s
integer-indefinite path, single-point stddev = the value itself, ...) **must** be
marked `TODO(compat):` with an explanation and the intended clean fix.

- Do **not** "fix" these during the port — goldens pin them; improved precision is
  indistinguishable from a porting bug in the gates.
- Do not use the `TODO(compat)` tag for anything else; it must stay greppable
  (`rg "TODO\(compat\)"`).
- They are all wiped out in one dedicated pass after the 1:1 port reaches final
  acceptance (PORTING_PLAN.md §6), regenerating goldens deliberately.

## Gate (must be green before any commit)

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Those three commands are the mandatory gate. The live oracle-comparison gate
(`crates/dss-core/tests/corpus_live.rs`) is now part of `cargo test` and runs
**unconditionally** (no `DSS_LIVE_ORACLE` env gate): it compiles + solves the
vendored corpus `tests/corpus/electricdss-tst` on both the Rust engine and the
pinned oracle and compares the full model live. The pinned dss-python oracle
(`tools/golden/PIN.txt`) must therefore be installed to run `cargo test` — without
it `corpus_live_solvable_cases_match_oracle` fails rather than skipping. New tests
read feeders from that vendored corpus, never from `.inputs/` at runtime.

## Conventions

- Unit tests inline as `#[cfg(test)]` modules; integration tests are thin drivers
  over the golden harness (`crates/dss-core/tests/harness/`).
- Pascal is the spec: port algorithms loop-for-loop where numerics matter, and cite
  the Pascal unit/identifier in the doc comment (`Pascal \`TcMatrix.Invert\``).
- 0-based indexing everywhere except the ground-node convention (`NodeRef == 0` =
  ground), converted only at parse/report boundaries.
- Case-insensitive identifiers via lowercase-normalized keys (THashList semantics).
- New behavior questions are settled empirically against the oracle (see
  `tools/golden/probe_val.py` for the pattern), not by guessing FPC semantics.
