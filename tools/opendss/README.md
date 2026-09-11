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
  argument), and restates it through the executive with `Set AllowForms=No`
  (option 149, `Executive/ExecOptions.pas:640`/`:1118`, served with no circuit),
  so a corpus sweep opens no forms/popups and no modal `DoSimpleMsg`
  (`Common/DSSGlobals.pas:615`, `:651`, `:676`) can hang the headless worker.
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
- **No report auto-display, in three layers** (GOLDEN_REBASE G1.10a F0 + F0′,
  coordinator decisions D25 and D39). r4133 calls `FireOffEditor` from 55 places;
  on Windows that is a `ShellExecute` of `DefaultEditor` (`Common/Utilities.pas:298`,
  `:304`), read from the machine key at DLL load with the default `'Notepad.exe'`
  (`Common/DSSGlobals.pas:990`, `:2122`) — one leaked OS process per report.
  `Engine::new` therefore issues, in order,
  `Set RegistryUpdate=No` → `Set AllowForms=No` → `new circuit.dssrs_bridge_init` →
  `Set ShowReports=No` → `Set ShowExport=No` → `clear` → `Set Editor=rundll32.exe` →
  `Set DefaultBaseFrequency=60`:
  (1) `AllowForms=No` covers the three hash-list `Dump` branches
  (`Executive/ExecHelper.pas:1223`/`:1232`/`:1241`) and the modal forms;
  (2) `ShowReports=No` (option 138) covers 34 sites — all of `Common/ShowResults.pas`
  plus `ControlQueue.pas:482`, `Solution.pas:3543`, `Monitor.pas:1774` — and
  `ShowExport=No` (option 71) covers `Executive/ExportOptions.pas:517`; neither is
  served by `DoSetCmd_NoCircuit` (`ExecOptions.pas:645-649` → `#301`), hence the
  throwaway circuit the following `clear` drops — `clear` resets neither flag
  (`Executive/Executive.pas:234-276`), so they hold for the worker's lifetime;
  (3) `Set Editor=rundll32.exe` is the **safety net** for the 12 sites no switch
  guards (`Dump`, `Dump alloc`, `FileEdit`, `AlignFile`, `VDIFF`,
  `CvrtLoadshapes`, `Show AutoAdded` ×2, `Show QueryLog`, `Rephase`,
  the CN/CNTS debug dumps) — `rundll32.exe` exits at once on a non-DLL argument, and
  the option is issued after `Set RegistryUpdate=No` so it never reaches the user's
  registry (`Common/DSSGlobals.pas:1017` under the guard at `:1015`). Those 12 sites
  are reported upstream in
  `investigations/to_opendss/73-dll-fires-editor-despite-noformsallowed.md`.
  **No report is suppressed — only the viewer launch:** every writer does
  `CloseFile(F)` and *then* consults its switch (`Common/ShowResults.pas:401-403`),
  measured 2026-09-11 as an identical created-file set over 14 report decks
  (56 entries with the switches on, 56 with `ShowReports` back at its default `Yes`,
  0 differing decks). Pinned by `crates/dss-epri/tests/protocol.rs`
  (`report_switches_survive_a_compile_and_gag_every_guarded_editor_site`,
  `the_editor_safety_net_covers_the_sites_no_switch_guards`,
  `init_overrides_the_os_editor_and_never_writes_it_back`). The capi channel needs no
  counterpart — dss_capi gates the same `FireOffEditor` on `DSS_CAPI_ALLOW_EDITOR`
  (`.inputs/dss_capi/src/Common/Utilities.pas:231`) and
  `tools/oracle/oracle_server.py:1974` already clears it.
- **The DLL is never `FreeLibrary`'d** (`Dll::leak()`): the r4133 unit
  finalization tears down its Delphi solver actor thread through a
  message-pumping `TThread.WaitFor` that deadlocks in a headless process. The
  worker leaks the library and lets process exit reclaim it (root-caused by
  minidump, STATUS "UNIFIED_GATE Phase A"). Do not "fix" this by unloading.
- The binaries are © EPRI, distributed under the BSD-style license in each
  `bin/r4133/License.txt`.
