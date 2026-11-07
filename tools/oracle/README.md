# tools/oracle — the server of the `capi_v0145` channel

The subprocess the unified corpus gate (`crates/dss-core/tests/corpus_gate.rs`,
the `capi_v0145` channel) calls at test time to compare the Rust engine against
the pinned dss-python (`tools/golden/PIN.txt`).

- **`oracle_server.py`** — persistent line-delimited-JSON server: reads a case
  request, compiles + solves on the pinned engine, returns the full captured
  model (node order, system Y, YPrim, injection, element powers/currents,
  discrete state, monitors/meters, probes). This server drives **only** the
  pinned dss-python `capi` engine: `DSS_ORACLE_ENGINE` accepts only `"capi"`
  (the default) and exits non-zero on anything else. The `r4133` channel is the
  in-house `crates/dss-epri` Rust bridge (`tools/opendss/`), not this server.
  The `"all_properties": true` request field adds `capture_all_properties`:
  every `AllElementNames` element's every property, `[[prop, Val]]` in
  `AllPropertyNames` order, read via `? name.prop`. The r4133 bridge's own
  `capture_all_properties` gates the r4133 channel the same way.
  The `"topology": true` request field adds
  `capture_topology`: the six order-free `ITopology` rows (`NumLoops`,
  `NumIsolated*`, `AllLoopedPairs`, `AllIsolated*`), read after every other read
  of the step but the incidence pair below, because the first `Topology` read
  builds the memoized branch tree. The twelve cursor rows are never read (they
  reassign `ActiveCktElement`).
  The `"inc_matrix": true` request field adds
  `capture_inc_matrix`: it issues `CalcIncMatrix` then `CalcLaplacian` and reads
  the four flat quantities (`IncMatrix`, `Laplacian`, `IncMatrixRows`,
  `IncMatrixCols`) **after** the topology capture, i.e. last of the whole step —
  the pair rewrites solution state and must not precede the read that memoizes
  the branch tree. `CalcIncMatrix_O` and `BusLevels` are never issued. Two
  transport normalizations make this channel byte-identical to the r4133 bridge
  and both RAISE on anything unexpected: the unwritten `+1` cell capi allocates
  for each integer array is dropped after asserting it is 0, and the
  one-element `''` sentinel of an absent name list becomes `[]` only where the
  engine can reach it (no incidence rows / no buses).
- **`corpus_guard.py`** — restores the vendored corpus tree after a run (the
  engine writes reports/DI files next to each deck); the Rust side has a mirror
  `CorpusGuard`.

Requires the pinned dss-python installed: without it the corpus gate fails
rather than skips (by design). Its interpreter is `python` unless
`DSS_ORACLE_PYTHON` names another (read by the corpus gate,
`corpus_gate::engines`).
