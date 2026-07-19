# Official EPRI OpenDSS r4133 oracle — the unified gate's `r4133` channel

This directory holds the **vendored official EPRI OpenDSS r4133 binary** and its
provenance. It is the second live oracle of the unified corpus gate
(`crates/dss-core/tests/corpus_gate.rs`): the `r4133` channel compares the Rust
port against EPRI's `OpenDSSDirect.dll` release 11.0.0.1, alongside the pinned
dss-python `capi_v0145` channel (`tools/oracle/oracle_server.py`).

The engine is driven by the **in-house `crates/dss-epri` Rust bridge**
(`epri-worker`), which loads the DLL via `libloading` and speaks the same
line-JSON `ping`/`run`/`quit` protocol as the oracle server. There is **no Python
in this channel** — the retired AltDSS Oddie bridge, its separate venv, the
`dss-python 0.16.0b2` / `dss-python-backend` wheels, and the r3723/r4088 binaries
were removed with the Python EPRI stack (`UNIFIED_GATE_PLAN.md` §4-E).

## Layout

| path | what |
|---|---|
| `bin/r4133/` | git-tracked `OpenDSSDirect.dll` (11.0.0.1) + `KLUSolve.dll` (its only non-system import) + `kmetis.exe`/`pmetis.exe` (METIS partitioners the engine spawns from the DLL's own dir for A-Diakoptics tearing) + `License.txt`. |
| `bin/SHA256SUMS` | checksums; verify from `bin/` with `sha256sum -c SHA256SUMS`. |
| `bin/README.md` | binary provenance (sizes, build dates, source tree). |
| `revisions.json` | `r4133` → `dll` path + `expect_version` (the substring `crates/dss-epri/src/smoke.rs` and `tools/golden/gen_protection.py` assert against — never empty, no silent pass). |
| `vendor_binaries.py` | re-vendors `bin/r4133/` from `.inputs/electricdss-code-r4133-trunk/Version8/Distrib/x64` and rewrites `SHA256SUMS` + `bin/README.md`. |

The DLLs are **git-tracked**, so worktrees need no junction for the bridge and
the gate has no external download step. `DSSProgress.exe` is deliberately not
vendored (no progress popups possible).

## Re-vendor procedure

```
python tools/opendss/vendor_binaries.py --force   # wipe + recopy bin/r4133
git diff tools/opendss/bin/SHA256SUMS             # review the checksum diff
cargo test -p dss-epri --test smoke               # r4133 self-smoke (version + CSC neutrality)
```

`smoke.rs` (the Rust successor of the old `smoke.py`) proves the DLL loads,
matches `revisions.json` `expect_version`, compiles + solves + converges IEEE13,
and that EPRI's CSC export is solution-neutral (`YNodeVarray` bit-identical
before/after `getYSparse`). Run `epri-worker --smoke` for the same checks via the
worker binary.

## Rules & caveats

- **The engine chdirs the process on `Compile`** — never rely on relative paths
  after a compile; the bridge resolves everything up front and the corpus guard
  (`crates/dss-epri/src/guard.rs`, a port of `tools/oracle/corpus_guard.py`)
  snapshots + restores the case dir so `Show`/`Export`/`Save` writes never
  pollute the vendored corpus (`git status tests/corpus` must stay clean).
- **Editor / registry suppression** is handled inside the bridge (`Set
  RegistryUpdate=No`, `Set Editor=rundll32.exe`) so a corpus sweep opens no
  Notepad windows and never persists an override into the user's OpenDSS
  registry settings.
- The binaries are © EPRI, distributed under the BSD-style license in each
  `bin/r4133/License.txt`.
