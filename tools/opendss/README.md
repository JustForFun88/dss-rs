# Official EPRI OpenDSS r4133 — the unified gate's `r4133` channel

This directory holds the **vendored official EPRI OpenDSS r4133 binary**, the
OpenDSS documentation and their provenance. It is the second OpenDSS engine of the unified corpus gate
(`crates/dss-core/tests/corpus_gate.rs`): the `r4133` channel compares this
engine against EPRI's `OpenDSSDirect.dll` release 11.0.0.1, alongside the pinned
dss-python `capi_v0145` channel (`tools/oracle/oracle_server.py`).

The engine is driven by the **in-house `crates/dss-epri` Rust bridge**
(`epri-worker`), which loads the DLL via `libloading` and speaks the same
line-JSON `ping`/`run`/`quit` protocol as `oracle_server.py`. There is **no
Python in this channel**.

## Layout

| path | what |
|---|---|
| `bin/r4133/` | git-tracked `OpenDSSDirect.dll` (11.0.0.1) + `KLUSolve.dll` (its only non-system import) + `kmetis.exe`/`pmetis.exe` (METIS partitioners the engine spawns from the DLL's own dir for A-Diakoptics tearing) + `License.txt` + KLUSolve's `KLUSolve_License.txt`/`KLUSolve_lgpl_2_1.txt` + `OpenDSS_Documentation.chm` (the OpenDSS documentation, the DSS language specification; how to read it: `bin/README.md`). |
| `bin/SHA256SUMS` | checksums; verify from `bin/` with `sha256sum -c SHA256SUMS`. |
| `bin/README.md` | binary provenance (sizes, build dates, source tree). |
| `revisions.json` | `r4133` → `dll` path + `expect_version` (the substring `crates/dss-epri/src/smoke.rs` and `tools/golden/gen_protection.py` assert against — never empty, no silent pass). |
| `epri_worker.py` | Python client for the worker's `exec`/`read`/`chdir` scripting surface (`crates/dss-epri/src/script.rs`) + an `IOddieDSS`-shaped shim for ad-hoc scripting. Used by the manual golden regen (`tools/golden/gen_protection.py` r4133 arm, `tools/golden/gen_flicker.py`) and probes, never by the gate. Builds `epri-worker` on first use. `DSS_EPRI_WORKER` names a worker binary to use instead. |
| `probe_59n.py` | re-runnable 59NRelayDemo read on r4133 (the no-trip and chaotic pole-slip classification; see `relay/tests.rs` and `skipped_needs_investigation.json`), driven through `epri_worker.py`. |

The DLLs are **git-tracked**, so worktrees need no junction for the bridge and
the gate has no external download step. `DSSProgress.exe` is deliberately not
vendored (no progress popups possible).

## Updating the binaries

Copy into `bin/r4133/` the five files from the official distribution's
`Version8/Distrib/x64`, `Version8/Distrib/Doc/OpenDSS Documentation.chm` as
`OpenDSS_Documentation.chm`, and `VersionC/klusolve/License.txt` and
`lgpl_2_1.txt` as `KLUSolve_License.txt` and `KLUSolve_lgpl_2_1.txt`, then rewrite the checksums and the tables in
`bin/README.md`, and run the smoke test:

```
cd tools/opendss/bin && sha256sum r4133/* > SHA256SUMS && cd -
python tools/gate/gatelock.py --label vendor -- cargo test -p dss-epri --test smoke
```

`smoke.rs` proves the DLL loads,
matches `revisions.json` `expect_version`, compiles + solves + converges IEEE13,
and that EPRI's CSC export is solution-neutral (`YNodeVarray` bit-identical
before/after `getYSparse`). Run `epri-worker --smoke` for the same checks via the
worker binary.

## Gate wiring

The channel is consumed by `crates/dss-core/tests/corpus_gate/engines.rs`
(`EpriPool`/`EpriOneShot`): it resolves the worker binary via `DSS_EPRI_WORKER`
→ `target/<profile>/epri-worker` → a one-time `cargo build -p dss-epri`
fallback, ping-asserts `{"epri": true, "rev": "r4133"}`, and recycles the
worker per case by default (determinism). A worker binary older than the newest
file of `crates/dss-epri/src` or its `Cargo.toml` is rebuilt, since a scoped
`cargo test -p dss-core --test corpus_gate` does not rebuild the bridge. An
explicit `DSS_EPRI_WORKER` is used as given: its freshness is the caller's.
Which cases gate on this channel is the manifests' `engines` field. Measured
r4133 divergences are pinned in the gating ledger `tests/corpus/ledger.json`
(kinds, envelopes and the triage procedure: `tools/corpus/README.md`). The r4133
request carries `all_properties` like the capi one, and the bridge's own
`capture_all_properties` is a **gating** capture. (`PROPS_015X` bridges the
0.15.x-shaped tables r4133 renders. On an r4133 capture an r4133-only prop is
carried by r4133's own name list and therefore compares in full.)

**What this channel does *not* compare: `Save`/`Dump` bytes.** The property
capture reads the engine's property getters; no comparison kind on either channel
reads a serialized `Save circuit` / `Dump` line, and the only OpenDSS engine
that ever sees those bytes is the pinned dss-python capture behind the
`reports/` goldens. `Save` and `Dump` render the live field values and, as
membership, the explicitly-set chain, and the divergences from r4133's own
serializer are pinned in-engine, quoting both engines' bytes, in
`crates/dss-core/src/exec/tests/report.rs`
(`save_renders_the_live_model_after_ncim_pv2pq` and its three siblings). The
r4133 side of those pins was measured through this bridge, with
`epri_worker.py`; re-measuring one is a probe, not a gate run.

## A-Diakoptics reference (no regen tool)

The committed A-Diakoptics trusted baseline
(`crates/dss-core/tests/data/adiakoptics/r3723_ref/{ieee13,ieee123}/`, consumed by
`crates/dss-core/tests/ad_reference.rs`) is an r3723 capture, and the
`PROVENANCE.txt` in each ref dir names the harvester that took it. **No regen
tool ships:** the baseline is frozen and treated as a trusted external
reference. To regenerate it, a harvester has to be written over `epri-worker`
(the `crates/dss-epri` bridge), reading `ZLL`/`ZCC`/`Y4` and the post-AD solved
node voltages from the DLL via the line-JSON protocol.

## Rules & caveats

- **The engine chdirs the process on `Compile`** — never rely on relative paths
  after a compile; the bridge resolves everything up front and the corpus guard
  (`crates/dss-epri/src/guard.rs`, a port of `tools/oracle/corpus_guard.py`)
  snapshots + restores the case dir so `Show`/`Export`/`Save` writes never
  pollute the vendored corpus (`python tools/corpus/vendor.py` lists and undoes
  any that did).
- **UI suppression**: the bridge's init sequence calls `DSSI(8, 0)`, which sets
  the engine's `NoFormsAllowed := TRUE` (`DDSS.pas` mode 8 — note the inverted
  argument), and restates it through the executive with `Set AllowForms=No`
  (option 149, `Executive/ExecOptions.pas:640`/`:1118`, served with no circuit),
  so a corpus sweep opens no forms/popups and no modal `DoSimpleMsg`
  (`Common/DSSGlobals.pas:615`, `:651`, `:676`) can hang the headless worker.
- **No process-global state escapes the worker.** r4133 persists
  `DefaultBaseFreq`, `LastFile` and `DataPath` in
  `HKCU\Software\OpenDSS\MainSect`: it reads them at DLL load and writes them
  back at process exit when `UpdateRegistry` is true, so an unguarded worker
  leaks its last base frequency to every later worker on the machine, in any
  worktree. The bridge therefore issues **`Set RegistryUpdate=No` at init**
  (`crates/dss-epri/src/dss.rs`, `Engine::new`), which stops the write back, and
  **`Set DefaultBaseFrequency=60` both at init and after every `clear`**
  (`Engine::new` / `Engine::clear`): the registry *read* has already happened by
  the time the bridge gets control, and r4133's `clear` does not reset the value
  either, so a bare probe session and every gate case alike start where this
  engine does, at 60 Hz. `tools/oracle/oracle_server.py` mirrors the frequency
  reset on the capi channel (which has no registry and rejects
  `Set RegistryUpdate` outright). Both are pinned by
  `crates/dss-epri/tests/protocol.rs`. A base-frequency-shaped red (`Vsource …
  frequency 60 vs 50`, `system Y mismatch`, ~1e-3 V gaps at 7 kV) on decks that
  were green before is environmental: check that
  `reg query HKCU\Software\OpenDSS\MainSect /v BaseFrequency` reads `60`.
- **No report auto-display, in three layers.** r4133 launches its report viewer
  from 55 places. On Windows that is a `ShellExecute` of `DefaultEditor`, read
  from the machine key at DLL load with the default `'Notepad.exe'`: one leaked
  OS process per report. `Engine::new` therefore issues, in order,
  `Set RegistryUpdate=No` → `Set AllowForms=No` → `new circuit.dssrs_bridge_init` →
  `Set ShowReports=No` → `Set ShowExport=No` → `clear` → `Set Editor=rundll32.exe` →
  `Set DefaultBaseFrequency=60`:
  (1) `AllowForms=No` covers the three hash-list `Dump` branches and the modal
  forms.
  (2) `ShowReports=No` (option 138) covers 34 sites, and `ShowExport=No`
  (option 71) covers the export one. Neither option is served without a
  circuit (`#301`), hence the throwaway circuit the following `clear` drops.
  `clear` resets neither flag, so they survive every `clear`. A deck can still
  set `ShowExport` itself (5 live corpus decks do), so `Engine::clear`
  re-asserts both per case.
  (3) `Set Editor=rundll32.exe` is the **safety net** for the 12 sites no switch
  guards (`Dump`, `Dump alloc`, `FileEdit`, `AlignFile`, `VDIFF`,
  `CvrtLoadshapes`, `Show AutoAdded` ×2, `Show QueryLog`, `Rephase`,
  the CN/CNTS debug dumps). `rundll32.exe` exits at once on a non-DLL argument,
  and the option is issued after `Set RegistryUpdate=No` so it never reaches
  the user's registry. Those 12 sites are an OpenDSS bug, filed in the OpenDSS
  reports.
  **No report is suppressed — only the viewer launch:** every writer closes its
  file and *then* consults its switch, so a deck creates the same files with
  the switches on or off. Pinned by
  `crates/dss-epri/tests/protocol.rs`
  (`report_switches_survive_a_compile_and_gag_every_guarded_editor_site`,
  `the_editor_safety_net_covers_the_sites_no_switch_guards`,
  `clear_re_asserts_the_report_switches`,
  `init_overrides_the_os_editor_and_never_writes_it_back`). The capi channel needs
  no counterpart: dss_capi gates the same viewer launch on its
  `AllowEditor` switch, which `tools/oracle/oracle_server.py` clears.
- **The bridge writes no file of its own.** The event-log capture
  (`crates/dss-epri/src/capture.rs`) reads the log in memory instead of running
  `export eventlog`, which would drop a CSV into the case directory. A capture
  that needs a surface only an `export` reaches reads it in memory too, or
  writes into a scratch directory it cleans, never into the case directory.
- **Never restore a bridge source file with a timestamp-preserving copy**
  (`Copy-Item`, `cp -p`) after a scratch experiment: cargo's mtime fingerprint
  and the gate's worker-freshness check then both keep the experiment's binary.
  Touch the file (or `cargo clean -p dss-epri`) and re-verify.
- **A worker wedged at exit.** A process caught spinning in the DLL's exit
  teardown has already set its exit code, so `Get-Process -Id`, `Stop-Process`
  and `taskkill /F` answer that it does not exist, while `tasklist` still lists
  it and it keeps burning a core.
  `Get-CimInstance Win32_Process -Filter "ProcessId=<pid>" | Invoke-CimMethod -MethodName Terminate`
  ends it, and so does a kill through a handle held since the spawn
  (`Child::kill`, `Popen.kill`). Dropping an `Engine` issues `ClearAll` once every
  actor is idle, which ends the solver threads while the process is still whole
  (pinned by `protocol::a_worker_that_quits_right_after_new_circuit_exits`).
- **The DLL is never `FreeLibrary`'d** (`Dll::leak()`): the r4133 unit
  finalization tears down its Delphi solver actor thread through a
  message-pumping `TThread.WaitFor` that deadlocks in a headless process. The
  worker leaks the library and lets process exit reclaim it (root-caused by
  minidump). Do not "fix" this by unloading.
- The files of `bin/r4133/` serve the tests only, and their licences are in
  `bin/README.md`.
