# tools/oracle — the live pinned-oracle server

The subprocess the always-on live gate (`crates/dss-core/tests/corpus_live.rs`)
calls at test time to compare the Rust engine against the pinned dss-python
oracle (`tools/golden/PIN.txt`).

- **`oracle_server.py`** — one-shot line-delimited-JSON server: reads a case
  request, compiles + solves on the pinned engine, returns the full captured
  model (node order, system Y, YPrim, injection, element powers/currents,
  discrete state, monitors/meters, probes). `DSS_ORACLE_ENGINE=oddie` rebinds
  it to an EPRI DLL for the opt-in channel (`tools/opendss/`).
  The opt-in `"all_properties": true` request field adds `capture_all_properties`
  (WP8.5b): every `AllElementNames` element's every property, `[[prop, Val]]` in
  `AllPropertyNames` order, read via `? name.prop` (the WPG.1-safe probe path).
  It is **suppressed on the EPRI/Oddie channel** (`corpus_live_opendss` forces it
  off) — different engine revisions render property strings differently (the
  known bracket/echo class), so property parity is gated only against pinned capi.
- **`corpus_guard.py`** — restores the vendored corpus tree after a run (the
  engine writes reports/DI files next to each deck); the Rust side has a mirror
  `CorpusGuard`.

Requires the pinned oracle installed — without it `cargo test` fails rather
than skips (by design). See `TESTING.md`.
