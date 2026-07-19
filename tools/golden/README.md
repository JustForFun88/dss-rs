# tools/golden — pinned-oracle golden generation (manual only)

Generators (`gen_*.py`) and probes for the **committed** goldens under
`tests/golden/`, plus the pinned-oracle version pin.

- **`PIN.txt`** — the exact oracle: dss-python 0.15.7 / dss_capi 0.14.5 (the
  Pascal source vendored at `.inputs/dss_capi`). Every golden is produced with
  this and only this.
- **`gen_*.py`** — one generator per golden family (see the table in
  `TESTING.md`). Each runs the pinned oracle, captures the result, and writes
  `tests/golden/<family>/…`. `generate.py` + `cases.json` drive the named-feeder
  goldens. Exceptions run other engines: the r4133-specific arms
  (`gen_protection.py` fuse_blow/swt_manual, `gen_flicker.py`) drive the official
  EPRI r4133 DLL through the in-house `epri-worker` bridge
  (`tools/opendss/epri_worker.py`; payload byte-parity with the retired
  Oddie-era captures is proven — STATUS "EPRI bridge parity round"), and the
  frozen capi015-era generators document their own retired engines.
- **`report_decks/`** — fixture decks the report goldens (`gen_reports.py`)
  replay.
- **`probe_*.py`** — one-off empirical probes (the project's "settle it against
  the oracle" convention), not part of any gate.

**Regeneration is manual and deliberate.** Goldens pin intentional upstream
inexactnesses (`TODO(compat)`); never regenerate to "fix" a divergence. See
`TESTING.md` → *Regenerate a golden*.
