# tools/oracle — the live pinned-oracle server

The subprocess the unified corpus gate (`crates/dss-core/tests/corpus_gate.rs`,
the `capi_v0145` channel) calls at test time to compare the Rust engine against
the pinned dss-python oracle (`tools/golden/PIN.txt`).

- **`oracle_server.py`** — persistent line-delimited-JSON server: reads a case
  request, compiles + solves on the pinned engine, returns the full captured
  model (node order, system Y, YPrim, injection, element powers/currents,
  discrete state, monitors/meters, probes). This server drives **only** the
  pinned dss-python `capi` engine; `DSS_ORACLE_ENGINE` accepts only `"capi"`
  (the default) and exits non-zero on anything else. The old opt-in EPRI channel
  (`DSS_ORACLE_ENGINE=oddie` / `capi015`, an EPRI DLL via the AltDSS Oddie
  bridge) was retired with the Python EPRI stack (`UNIFIED_GATE_PLAN.md` §4-E);
  the `r4133` channel is now the in-house `crates/dss-epri` Rust bridge
  (`tools/opendss/`), not this server.
  The `"all_properties": true` request field adds `capture_all_properties`
  (WP8.5b): every `AllElementNames` element's every property, `[[prop, Val]]` in
  `AllPropertyNames` order, read via `? name.prop` (the WPG.1-safe probe path).
  Property parity was gated only against this pinned capi oracle until
  R4133_PROPS RP4.1 (2026-09-03); the r4133 bridge's own `capture_all_properties`
  now gates the r4133 channel the same way.
- **`corpus_guard.py`** — restores the vendored corpus tree after a run (the
  engine writes reports/DI files next to each deck); the Rust side has a mirror
  `CorpusGuard`.

Requires the pinned oracle installed — without it `cargo test` fails rather
than skips (by design). See `TESTING.md`.
