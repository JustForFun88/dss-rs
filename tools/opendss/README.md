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
| `epri_worker.py` | Python client for the worker's `exec`/`read`/`chdir` scripting surface (`crates/dss-epri/src/script.rs`) + an `IOddieDSS`-shaped shim — the functional-parity replacement for the retired Oddie bridge's ad-hoc scripting. Used by the manual golden regen (`tools/golden/gen_protection.py` r4133 arm, `tools/golden/gen_flicker.py`) and probes; never by the gate. Auto-builds `epri-worker` on first use. |
| `probe_59n.py` | re-runnable 59NRelayDemo oracle read on r4133 (WP-U2.6 no-trip + chaotic pole-slip classification; see `relay/tests.rs` and `skipped_needs_investigation.json`), driven through `epri_worker.py`. |

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

## Gate wiring

The channel is consumed by `crates/dss-core/tests/corpus_gate/engines.rs`
(`EpriPool`/`EpriOneShot`): it resolves the worker binary via `DSS_EPRI_WORKER`
→ `target/<profile>/epri-worker` → a one-time `cargo build -p dss-epri`
fallback, ping-asserts `{"epri": true, "rev": "r4133"}`, and recycles the
worker per case by default (determinism). Which cases gate on this channel is
the manifests' `engines` field; measured r4133 divergences are pinned in the
gating ledger `tests/corpus/ledger.json` (kinds, envelopes, and the triage
procedure: `TESTING.md`). Since R4133_PROPS RP4.1 (2026-09-03) the r4133 request
carries `all_properties` like the capi one — the per-channel mask is gone, and
the bridge's own `capture_all_properties` is a **gating** capture, not report
tooling. (`PROPS_015X` still bridges the 0.15.x-shaped tables r4133 renders; on
an r4133 capture an r4133-only prop is carried by the oracle's own name list and
therefore compares in full.)

**What this channel does *not* compare: `Save`/`Dump` bytes.** The property
capture reads the engine's property getters; no comparison kind on either channel
reads a serialized `Save circuit` / `Dump` line, and the only oracle that ever
sees those bytes is the pinned dss-python capture behind the `reports/` goldens.
R4133_PROPS RP3.11 (2026-09-03) settled that surface for every class at once —
verdict `KEEP_LIVE_PINNED` on both `Save` and `Dump` (values = the live field,
membership = the explicitly-set chain) — and the divergences from r4133's own
serializer are pinned in-engine, quoting both engines' bytes, in
`crates/dss-core/src/exec/tests/report.rs`
(`save_renders_the_live_model_after_ncim_pv2pq` and its three siblings). The
r4133 side of those pins was measured through this bridge, with
`epri_worker.py`; re-measuring one is a probe, not a gate run.

## A-Diakoptics reference regen (no tool ships anymore)

The committed A-Diakoptics trusted baseline
(`crates/dss-core/tests/data/adiakoptics/r3723_ref/{ieee13,ieee123}/`, consumed by
`crates/dss-core/tests/ad_reference.rs`) was harvested by the retired
`gen_ad_reference.py`, which drove the r3723 EPRI DLL through the deleted
Oddie/dss-python channel and was removed with the rest of the Python EPRI stack
(`UNIFIED_GATE_PLAN.md` §4-E). The `PROVENANCE.txt` in each ref dir still cites it
as the historical source of record. **There is no regen tool anymore:** the
baseline is frozen and treated as a trusted external oracle. If it ever needs to be
regenerated, the harvester must be **reimplemented over `epri-worker`** (the
`crates/dss-epri` bridge) — reading `ZLL`/`ZCC`/`Y4` and post-AD solved node
voltages from the DLL via the line-JSON protocol — since the old dss-python path no
longer exists.

## Rules & caveats

- **The engine chdirs the process on `Compile`** — never rely on relative paths
  after a compile; the bridge resolves everything up front and the corpus guard
  (`crates/dss-epri/src/guard.rs`, a port of `tools/oracle/corpus_guard.py`)
  snapshots + restores the case dir so `Show`/`Export`/`Save` writes never
  pollute the vendored corpus (`git status tests/corpus` must stay clean).
- **UI suppression**: the bridge's init sequence calls `DSSI(8, 0)`, which sets
  the engine's `NoFormsAllowed := TRUE` (`DDSS.pas` mode 8 — note the inverted
  argument), so a corpus sweep opens no forms/popups.
- **No process-global state escapes the worker** (GOLDEN_REBASE G1.4a, coordinator
  decision D13). r4133 persists `DefaultBaseFreq`, `LastFile` and `DataPath` in
  `HKCU\Software\OpenDSS\MainSect` — read at DLL load in `TExecutive.Create`
  (`Common/DSSGlobals.pas:1005`, `Executive/Executive.pas:124`), written back at
  process exit in `TExecutive.Destroy` when `UpdateRegistry` is true
  (`Common/DSSGlobals.pas:1015,1022`, `Executive/Executive.pas:141`) — so an
  unguarded worker leaks its last base frequency to every later worker on the
  machine, in any worktree. The bridge therefore issues **`Set RegistryUpdate=No`
  at init** (`crates/dss-epri/src/dss.rs`, `Engine::new`), which stops the write
  back, and **`Set DefaultBaseFrequency=60` both at init and after every
  `clear`** (`Engine::new` / `Engine::clear`) — the registry *read* has already
  happened by the time the bridge gets control, and r4133's `clear` does not
  reset the value either (`Executive/Executive.pas:234-275`), so a bare probe
  session and every gate case alike start where the port does, at 60 Hz
  (`crates/dss-core/src/exec/construct.rs:173`). `tools/oracle/oracle_server.py`
  mirrors the frequency reset on the capi channel (which has no registry and
  rejects `Set RegistryUpdate` outright). Both are pinned by
  `crates/dss-epri/tests/protocol.rs`.
- **No OS editor is fired** (GOLDEN_REBASE G1.10a, coordinator decision D25). r4133
  ends every `Show` writer with
  `If AutoDisplayShowReport Then FireOffEditor(FileNm)` (`Common/ShowResults.pas`,
  20+ sites; `AutoDisplayShowReport := TRUE` at `Common/DSSGlobals.pas:2052`) and
  calls it unconditionally from `Dump` (`Executive/ExecHelper.pas:1357`), the
  hash-list dumps (`:1223`-`:1249`), `VDIFF` (`:3373`) and `Show autoadded`
  (`Executive/ShowOptions.pas:208`); `DoShowCmd` (`Executive/ShowOptions.pas:156`)
  has **no** `NoFormsAllowed` guard, so `DSSI(8, 0)` does not cover it. On Windows
  `FireOffEditor` `ShellExecute`s `DefaultEditor` (`Common/Utilities.pas:304`),
  which the DLL read from the machine key at load with the default `'Notepad.exe'`
  (`Common/DSSGlobals.pas:990`) — one leaked OS process per report on every gate
  run. The bridge therefore issues **`Set Editor=rundll32.exe` at init**, right
  after `Set RegistryUpdate=No` so the value never reaches the user's registry
  (`Common/DSSGlobals.pas:1017`, guarded by `:1015`); `rundll32.exe` exits at once
  on a non-DLL argument (measured: 0 processes, 0 windows, no created-file name
  moves). Pinned by `crates/dss-epri/tests/protocol.rs`. The capi channel already
  clears `DSS_CAPI_ALLOW_EDITOR` (`tools/oracle/oracle_server.py:1585`).
- **The DLL is never `FreeLibrary`'d** (`Dll::leak()`): the r4133 unit
  finalization tears down its Delphi solver actor thread through a
  message-pumping `TThread.WaitFor` that deadlocks in a headless process. The
  worker leaks the library and lets process exit reclaim it (root-caused by
  minidump, STATUS "UNIFIED_GATE Phase A"). Do not "fix" this by unloading.
- The binaries are © EPRI, distributed under the BSD-style license in each
  `bin/r4133/License.txt`.
