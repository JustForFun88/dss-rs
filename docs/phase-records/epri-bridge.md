# EPRI `dss-epri` bridge — parity round and capability round

> Moved verbatim from `STATUS.md` on 2026-08-05 (STATUS.md history archiving);
> order preserved, nothing rewritten.

## EPRI bridge parity round — `dss-epri` covers every retired-Oddie task (branch `epri-parity`, 2026-07-19)

User mandate: FULL functional parity — every task the retired Oddie/dss-python
bridge ever did must be doable through the in-house Rust bridge
(`crates/dss-epri` drives the same official r4133 engine natively). Closes the
Phase-E re-review findings F2 (dead golden-regen r4133 arms) and F3 (lost
`probe_59n` reproduction).

### Consumer inventory (everything the Oddie bridge ever served, at `ae3e6bd^`)

| consumer (pre-Phase-E) | task | disposition |
|---|---|---|
| `ab_compare.py` | corpus A/B divergence reports between engine revisions | retired-by-design — the unified two-channel gate (`capi_v0145` + `r4133` in `corpus_gate.rs`) replaced the reporting channel |
| `sweep_modes_isolated.py` / `sweep_merge.py` | WP-U0.2 `ab_compare` crash-isolation + merge helpers | retired-by-design — `ab_compare` consumers; the sweep reports are frozen in `docs/upgrade` |
| `dsspy_validation/` + `wheels/` + venv | vendored dss-python full-API crosscheck | retired-by-design — one-time Rung-1 validation, superseded by the permanent gate |
| `dsspy_crosscheck.py` | classifier-manifest crosscheck vs DSS-Python's validation list (textual, engine-free) | retired-by-design — one-time promotion audit; results absorbed into the manifests |
| `smoke.py` | binary self-smoke | parity restored in Phase A — `epri-worker --smoke` / `crates/dss-epri/src/smoke.rs` + `tests/smoke.rs` |
| `xcheck_bridge.py` | Oddie-vs-Rust-bridge bit-diff | retired-by-design — self-obsoleting Phase-A fidelity proof (its PASS licensed the retirement) |
| `gen_ad_reference.py` | A-Diakoptics matrix reference harvest (r3723) | retired-by-design with documented contingency — baseline frozen (`r3723_ref/` + PROVENANCE); `tools/opendss/README.md` mandates reimplementation over `epri-worker` if ever re-run |
| `oracle_server.py` oddie/capi015 arms | opt-in oracle engines for the live test | retired-by-design — the gate channels replaced them |
| `gen_protection.py` r4133 arm (`make_oddie`; fuse_blow, swt_manual) | golden regeneration | **parity restored (this round)** — `make_epri` drives `epri-worker` via the `IOddieDSS`-shaped shim; payload byte-parity proven below |
| `gen_flicker.py` (r3723) | flicker golden regeneration | **parity restored (this round)** — rewritten over `epri-worker` on r4133 (the only vendored official revision); cross-revision payload byte-parity proven below |
| `probe_59n.py` | 59N artifact reproduction (WP-U2.6) | **parity restored (this round)** — recreated over `epri-worker`; artifact reproduced verbatim |
| *(adjacent, not Oddie)* `gen_bh_capi015.py` / `gen_regcontrol_capi015.py` / `gen_ncim_reports.py` / `gen_der_lines_harmonics.py` capi015 runs / `gen_checkpoints.check_pin` capi015 arm / `gen_reports.py` capi015-gated seasonal arm | capi015 (dss-python 0.16.0b2 / dss_capi 0.15.0b4) golden generation | outside the mandate — capi015 was the *Python-engine* beta channel, not the official-binary Oddie bridge; the Rust bridge cannot (different binary) and should not drive it. Goldens frozen; the capi015 arms fail loudly (missing `PIN_OPENDSS.txt`), no silent pass. |
| `gen_fuse_r4133.py` / `r4133_help.py` | derived golden (`git show` overlay) / frozen r4133 `PropertyHelp` data — no engine | unaffected — engine-free; their `oddie:r4133` provenance markers are historical (original Oddie captures, recorded in their notes); neither imports `IOddieDSS` |

### Protocol extension (gate-neutral)

New `epri-worker` commands `exec` / `read` / `chdir`
(`crates/dss-epri/src/script.rs`) — the generic scripting surface replacing
Oddie's `Text.Command` + per-property reads; one `read` = one DLL accessor +
per-call error poll (dss-python raise-on-error parity), `exec` = strict errno +
actor-idle barrier (the async-solve race fix applies to scripted `solve`/inline
solves too). New FFI: `SolutionV` (mode 0 EventLog — the raw `EventStrings`
`Hour=…, Sec=…` lines; the empty-log `None` placeholder is written WITHOUT a
terminator and normalizes to `[]`, matching the retired Oddie decode — pinned by
the committed `swt_manual` golden; deliberately a separate decode from the
gate's `decode_string_array`) and `BUSF(0)` kVBase; plus `CircuitS(4)`
SetActiveBus and `MonitorsS(2)` monitor-select on existing entry points. Gate
paths untouched: `run`/`ping`/`clear`/`quit` handlers, `capture.rs`,
comparators, scheduler are byte-unchanged and the gate never sends the new
commands. SAFETY rules upheld (immediate copies, `deny(unsafe_op_in_unsafe_fn)`,
per-boundary SAFETY docs). Worker-level smoke:
`crates/dss-epri/tests/protocol.rs` spawns the real binary and pins the exact
event-log line format the committed protection goldens store
(`Hour=0, Sec=0.2, ControlIter=1, Element=Fault.f, Action=**APPLIED**`), the
read shapes (solution scalars, node arrays, element powers/currents, variables,
monitor channels, bus kVBase), the empty-log `[]` normalization, and the
error path (`ok:false`, worker stays alive).

Python client `tools/opendss/epri_worker.py`: `EpriWorker` (spawn + line-JSON,
binary resolution mirroring the gate's `epri_worker_bin`: env override →
target/{release,debug} → one cargo build) and `EpriEngine`, an
`IOddieDSS`-shaped shim so `gen_protection.build()` + `capture_element()` run
unchanged and issue the same per-property DLL call sequence Oddie did.

### Parity proof (committed goldens UNTOUCHED — scratch regen + byte-compare)

| golden | committed capture | regen path | verdict |
|---|---|---|---|
| `protection/fuse_blow.json` | Oddie r4133 | epri-worker r4133 | `scenario` payload **byte-identical** (7 071 serialized bytes: per-step voltages, event log, final elements); diff confined to the `oracle` provenance block (the raw DLL version string lacks the retired wrapper's `\nDSS-Python version: 0.16.0b2` suffix) |
| `protection/swt_manual.json` | Oddie r4133 | epri-worker r4133 | `scenario` payload **byte-identical** (8 255 bytes); same provenance-only diff (the committed block also carries the older `oddie:r4133` marker shape from its original capture flow, which even the old generator would no longer emit) |
| `flicker/pst_demo.json` | Oddie **r3723** | epri-worker **r4133** | every payload key **byte-identical** across engine revisions — `raw_mag` (494 142 serialized bytes), `flk` (588 324), `pst` (547 074), `kvbase`, `times`, `deck`, `n`/`nphases`/`fbase`; only `oracle` differs. The official flicker meter + this deck's power flow are revision-stable; the committed golden stays the frozen r3723 capture. |

Zero committed-file changes (`git status tests/golden` clean); corpus pristine
after all runs (enforced for the flicker regen by the settle round's DataPath
redirect — see below). `DSS_GOLDEN_OUT` env (new) redirects both generators'
output for scratch parity runs; default remains the committed tree.

### F3 — probe_59n reproduction restored

`tools/opendss/probe_59n.py` recreated over `epri-worker`; run 2026-07-19
reproduces the documented artifact verbatim: `Relay.State =
'[closed, closed, closed, ]'` (byte-equal to the port's `render_state_array()`),
`Line.line1 |I|max = 1381.156 A` (the "~1381 A" cited in `relay/tests.rs`),
`f = 78.88 Hz` at t=1.0 s, wander `[67.16, 115.00] Hz` over the next 15 s →
`PROBE OK`, exit 0. `relay/tests.rs` doc comment updated to cite the bridge
driver (comment-only); `skipped_needs_investigation.json` untouched.

Docs: `tools/opendss/README.md` layout rows for `epri_worker.py`/`probe_59n.py`;
`tools/golden/README.md` generator-exceptions note. `TESTING.md`/`CLAUDE.md`
deliberately untouched (Phase F owns them; its one stale F2 sentence reconciles
after both merge).

### Settle (2026-07-19) — two opus xhigh audit dispositions

Both audits independently re-derived the acceptance (scratch regen through the
real `epri-worker` + byte-compare, probe_59n rerun, protocol smoke) and
confirmed it: payload byte-parity holds for all three goldens, zero committed
bytes changed, gate semantics untouched. Findings settled:

- **Flicker-regen corpus leak (audit-code fp-1 low / audit-tests PARITY-1
  medium) — CONFIRMED, FIXED in the driver.** A standalone
  `python tools/golden/gen_flicker.py` left an untracked 730 KB
  `pst_Mon_pst_1.csv` inside `tests/corpus/.../Examples/Scripts/`
  (reproduced), falsifying the original "corpus pristine" evidence and the
  generator's own comment. Root cause (settled against the r4133 Pascal): the
  DLL initializes `DataDirectory`/`OutputDirectory` from the
  registry-persisted `DataPath` (`HKCU\Software\OpenDSS\MainSect`,
  `ReadDSS_Registry`) = the directory of the last deck ANY local run
  `Compile`d (probe_59n's 59NRelayDemo → `Examples/Scripts/`); this deck is
  exec'd line-by-line, so nothing re-pointed it, and `export monitor` wrote
  there. A `CorpusGuard` over the deck dir could NOT cover it (the leak lands
  in a different, run-history-dependent corpus dir), so the fix is a
  deterministic `set DataPath="<temp>/dss_rs_flicker_export"` before the
  export (`Set DataPath` also ChDirs the worker — harmless post-deck;
  `DSSGlobals.SetDataPath`, r4133 line 934). Re-verified: standalone regen
  leaves `git status` clean, the CSV lands in the temp dir, and every payload
  key of the regen is STILL byte-identical to the committed golden AND to the
  pre-fix regen (the redirect changes zero payload bytes). Golden untouched.
- **Flicker byte-count figures (audit-code fp-2 low) — CONFIRMED, FIXED.**
  The parity-table row overstated the serialized sizes; re-measured from the
  committed golden (`json.dumps` per key): `raw_mag` 494 142, `flk` 588 324,
  `pst` 547 074 (table corrected above). The parity VERDICT itself was
  independently re-verified byte-true by both audits and by the settle rerun.
- **Inventory enumeration (audit-tests PARITY-2 low) — CONFIRMED, FIXED.**
  `gen_reports.py`'s capi015-gated seasonal arm and `r4133_help.py` (frozen
  r4133 PropertyHelp data) carry `oddie:r4133`/oddie-venv provenance mentions
  but import no `IOddieDSS`; added to their existing inventory classes
  (capi015 Python-engine row / engine-free frozen-data row). No parity target
  was missed — all 7 actual `IOddieDSS` importers were already dispositioned.

Settle gate: fmt/clippy/test all exit 0 (`cargo test --workspace` 7 min 05 s,
0 failures across all binaries; corpus-live 25/25). One settle-gate run left
six deck-authored export leftovers (`Test/AutoTrans/auto3bus_*`,
`GFM_IEEE8500/IEEE8500u_EXP_*`) in `tests/corpus` — the pre-existing
corpus-guard parallel-run race (`corpus_guard.py` docstring: incomplete
snapshot = never delete; end-of-run `git status tests/corpus` + path-limited
clean is the documented recovery, applied). Not introduced by this round (gate
capture paths byte-unchanged); open follow-up for the gate-hygiene backlog.

## EPRI capability round (Round 2) — `dss-epri` covers everything the Oddie bridge COULD DO, and beyond (branch `epri-capability`, 2026-07-19)

User mandate: **"the Rust FFI bridge must cover everything the python Oddie
bridge COULD DO — and beyond."** Round 1 (above) closed *usage* parity (every
retired-Oddie task doable through the bridge). This round closes **capability**
parity: the full API surface the Oddie/dss-python bridge exposed over the same
r4133 engine, mapped and bound — plus cheap "beyond" items. Additive and
gate-neutral: the `run`/`ping`/`clear` capture path, `capture.rs`, the
comparators and the `corpus_gate` scheduler are byte-untouched; the new commands
are never sent by the gate.

### Coverage table — zero unclassified exports

The r4133 `OpenDSSDirect.dll` export table was dumped with a throwaway stdlib
PE-export parser (no new deps) and cross-checked against the `exports` clause of
`Version8/Source/DDLL/OpenDSSDirect.dpr`. **164 exports, all classified:**

| class | count | reached via | binding status |
|---|---|---|---|
| uniform family entry points (42 families × present `I`/`F`/`S`/`V`) | 147 | generic `ffi` `(family, kind, mode, arg)` dispatch (`src/families.rs`) | 29 already typed (gate capture) + **118 newly reachable**; all 147 now generic |
| standalone gate-path exports | 6 | typed `Engine` methods | bound pre-R2 (Phase A / R1): `DSSPut_Command`, `ErrorCode`, `ErrorDesc`, `InitAndGetYparams`, `GetCompressedYMatrix`, `getIpointer` |
| Y-matrix / injection helpers | 9 | `ymatrix` command | **newly bound (R2)**: `ZeroInjCurr`, `GetSourceInjCurrents`, `GetPCInjCurr`, `SystemYChanged`, `BuildYMatrixD`, `UseAuxCurrents`, `AddInAuxCurrents`, `getVpointer`, `SolveSystem` |
| Delphi RTL debug symbols | 2 | — | **skip-by-design (non-API)**: `__dbk_fcall_wrapper`, `dbkFCallWrapperAddr` (madExcept/debug hooks, not engine surface) |

The 42 families and the ABI shapes each exports (a `None` marks a shape the
family lacks — the registry spells out every real export symbol, since names are
not always `NameX`: `Bus`→`BUSI…`, `Loads`→`DSSLoads…`, `DSSProperties` is a
bare `S`):

`ActiveClass isv · Bus ifsv · CapControls ifsv · Capacitors ifsv · Circuit ifsv
· CktElement ifsv · CmathLib fv · CtrlQueue iv · DSS isv · DSSElement isv ·
DSSExecutive is · DSSProgress is · DSSProperties s · Fuses ifsv · GICSources
ifsv · Generators ifsv · Isource ifsv · LineCodes ifsv · Lines ifsv · Loads ifsv
· LoadShape ifsv · Meters ifsv · Monitors isv · PDElements ifs · PVsystems ifsv
· Parallel iv · Parser ifsv · Reactors ifsv · Reclosers ifsv · ReduceCkt ifs ·
RegControls ifsv · Relays isv · Sensors ifsv · Settings ifsv · Solution ifsv ·
Storages ifsv · SwtControls ifsv · Topology isv · Transformers ifsv · Vsources
ifsv · WindGens ifsv · XYCurves ifsv` = 147 entry points.

Skip-by-design detail: **`DSSProgress` (I,S)** is a headless progress-form
no-op — it IS bound in the family table (so the bridge literally covers
everything Oddie could call), but there is no observable engine state to assert
on, so no dedicated smoke drives it. The r4133 DDLL exports **no** plotting /
DSSGraph / registry / file-dialog symbols at all (the `Forms`/`Plot` units
compile in but export nothing), so the skip list stays limited to `DSSProgress`
+ the two Delphi debug symbols — the whole rest of the surface is bound.

### Systematic binding (`crates/dss-epri`, SAFETY rules upheld)

- **`src/families.rs`** (new): the 42-family registry + generic dispatch. `FnI/F/S/V`
  symbols loaded per family into a `FamilyTable` (case-insensitive lookup); one
  `Engine::ffi_dispatch(FfiCall)` reaches every mode of every family. Decodes the
  V-protocol by `myType` (1=int / 2=double / 3=complex re/im / 4=string / 5=bytes)
  with a **raw** string split (no gate-path monitor-header space strip — the gate's
  `decode_string_array` is untouched), and supports V **setters** (array-in via
  `myPointer`, e.g. `LoadShapeV(2)` PMult write). Unit-tested (`decode_v` by tag,
  raw string split, `encode_v_set`→`decode_v` round-trip).
- **`ffi.rs`**: `YMatrixFns` (the 9 standalone Y-helpers, transcribed 1:1 from
  `DYMatrix.pas`) + the family table, loaded in `Dll::load` and handed to `Engine`
  via `leak_into_parts`. `sym` is now `pub(crate)`. All FFI stays behind
  `deny(unsafe_op_in_unsafe_fn)` + per-boundary `// SAFETY`; DLL still never
  `FreeLibrary`'d; NoFormsAllowed stays set; V-buffers copied out immediately.
- **`dss.rs`**: `ffi_dispatch` + `ym_*`/`v_pointer`/`y_dims`/`solve_system` typed
  wrappers (each with a SAFETY note); `FfiCall`/`FfiOut` types.
- **`script.rs`**: `handle_ffi` + `handle_ymatrix` (parse/serialize only; all FFI
  is in `dss.rs`).

### New worker protocol surface (gate-neutral)

Four additive `epri-worker` commands (`src/bin/epri-worker.rs`):

- **`ffi`** — generic `{family, kind, mode, iarg|farg|sarg, vset?}` → kind-tagged
  reply `{kind, value|data, type, n, errno, error}`. Reaches every DDLL family
  mode: scalar get/set (i/f/s), array get (v getter), array set (v setter via
  `vset:{type,data}`).
- **`ymatrix`** — the standalone Y-matrix/injection helpers by op name
  (`y_dims`, `vpointer`, `ipointer`, `solve_system`, `system_y_changed`,
  `use_aux_currents`, `build_y`, `zero_inj`, `get_source_inj`, `get_pc_inj`,
  `add_aux`).
- **`caps`** — structured capability handshake: `protocol_version`, oracle
  identity, the family manifest (name + kinds), `family_count`,
  `family_entry_points`, the command list, and the `ymatrix` op list.
- **`batch`** — many `exec` commands in one round-trip (stop-on-first-error;
  `{replies, ran, failed_at}`).

### Beyond (what the Python/Oddie bridge never had)

1. **Per-call structured errno surface** — every `ffi`/`ymatrix` reply carries the
   `ErrorCode`/`ErrorDesc` polled *right after* the call (`{errno, error}`),
   non-fatally. dss-python only raised/aggregated; here the caller sees the exact
   engine errno per call (smoked: no-active-LoadShape read → `#61001` surfaced).
2. **Batched multi-command exec** (`batch`) — compile+build+solve in one
   round-trip instead of one command per line.
3. **Structured version/capability handshake** (`caps`) — the whole reachable
   surface introspectable in one message.
4. **Worker-pool crash isolation + recycling** (already in the gate's `EpriPool`,
   `crates/dss-core/tests/corpus_gate/engines.rs`, untouched here) — a per-request
   deadline → kill/respawn/retry-once, recycle-after-N, one-shot per case for
   serial/isolate. The single-process Oddie/dss-python host had none of this;
   documented as the standing "beyond" the transport already provides.

### Smoke evidence

- `cargo test -p dss-epri --lib`: 4 `families` unit tests green (decode-by-tag,
  raw string split, `encode`↔`decode` round-trip, `vset_len` element-count).
- `crates/dss-epri/tests/protocol.rs::capability_surface_end_to_end` (new, drives
  the REAL r4133 DLL): `caps` (42 families / 147 entry points / proto 2 / commands
  present / family-shape spot-checks), `batch` build, `ffi` i/s/v getters
  **cross-checked equal to the typed channel** (`Circuit` NumNodes == node-order
  len; `AllElementNames` == typed read), `ffi` V-set **round-trip** (LoadShape
  PMult `[1,1,1]`→set→`[5,6,7]`), `ymatrix` (`y_dims.n_bus`==NumNodes, `vpointer`
  shape `2*(N+1)`, `system_y_changed`, `solve_system` == KLU success 1 — the
  engine's own `Solution.pas` `IF SolveSystem(...) = 1` success test), structured errno
  `#61001`, and error paths (unknown family / unknown kind / absent ABI shape all
  `ok:false`, worker survives). The pre-existing `scripting_surface_end_to_end`
  smoke is byte-untouched.

Gate: fmt/clippy/`cargo test --workspace` all green at defaults (corpus pristine
after runs; a StorageControllerTechNote guard-race leftover was path-limited
cleaned — same standing gate-hygiene backlog item as Round 1, not introduced
here). Base `09d03e5`.

### Settle (two opus-xhigh audits, 2026-07-19)

Two independent xhigh audits of the round; export table re-derived independently
(164 exports = 147 family + 6 gate-path + 9 Y-helpers + 2 Delphi debug — zero
unclassified, headline confirmed). Both audits agreed the binding is faithful
and gate-neutral. Findings settled empirically (drove the worker + live DLL):

- **F1 (high, FFI-safety — the round's #1 focus): FIXED.** The generic V-set
  `call_v_set` passed the array's **byte** length as `mySize`, but the r4133 SET
  path treats `mySize` as an **element (point)** count — it clamps `LoopLimit :=
  min(mySize, NumPoints)` and steps `myPointer` one element per iteration
  (`DLoadShape.pas` PMult write; `DXYCurves.pas` XArray write). A byte count (8×
  for doubles) defeats the clamp, so the DLL over-reads the Rust buffer whenever
  the supplied array is shorter than the target's point count — a real OOB read.
  Fix: new `families::vset_len` passes the element count; `call_v_set`'s SAFETY
  note now states the true invariant (reads ≤ `min(size, NumPoints)` elements)
  and the caller-supplies-full-array contract for the setters (`DXYCurves`) that
  ignore `mySize` entirely. New tests prove element-count semantics: a 3-element
  write into a **5-point** LoadShape fills points 1..3 and clamps there
  (`[10,20,30,2,2]`, `written == 3`) — the exact `len < NumPoints` case the old
  byte-count code would have over-read; plus a `vset_len` unit test.
- **F2 (low, robustness): FIXED.** Seven DYMatrix ops
  (`system_y_changed`/`use_aux_currents`/`build_y`/`add_aux`/`vpointer`/
  `ipointer`/`solve_system`) hit unguarded `ActiveCircuit.Solution` in the DLL,
  so sending them before a circuit is compiled nil-derefs and kills the worker.
  `handle_ymatrix` now rejects the crash set with a clean error when
  `circuit_name()` is empty (`CircuitS(0)` is nil-guarded → `""` with no
  circuit). New test `ymatrix_before_compile_is_guarded_not_a_crash` drives all
  seven on a fresh worker: each returns `ok:false` and the worker stays alive.
- **Test-coverage holes (audit-tests, all closed):** batch **stop-on-first-error**
  path (bad middle command → `ran == 2`, `failed_at == 1`, trailing command not
  run); generic **`f`-kind getter happy path** (`Solution.Frequency` mode 0 ==
  60); the 7 previously-unexercised **ymatrix ops** (`use_aux_currents`,
  `build_y`, `zero_inj`, `get_source_inj`, `get_pc_inj`, `add_aux`, `ipointer`);
  and `system_y_changed` upgraded from a presence-only check to a real
  **read/write round-trip** (set-true→reads 1, set-false→reads 0).

Smoke now: `cargo test -p dss-epri` = 4 lib unit tests + `protocol.rs`'s 3
end-to-end tests (`scripting_surface_end_to_end`,
`capability_surface_end_to_end`, `ymatrix_before_compile_is_guarded_not_a_crash`)
all green against the real r4133 DLL. Nothing deliberately left unfixed.

## Moved from `STATUS.md` §6 (2026-09-11, GOLDEN_REBASE G1.10a F0′ docs pass)

The three §6 paragraphs below described the opt-in **Oddie/dss-python** tooling
(`ab_compare.py`, `known_diffs.json`, `dsspy_validation/` + `wheels/` +
`PIN_OPENDSS.txt`, `tools/corpus/dsspy_crosscheck.py`, the r3723/r4088 binaries).
All of it was retired by the round above and is no longer in the tree, so §6 — a
*how to run* section — was pointing at commands that cannot run; its editor-
suppression gotcha (1) also contradicted the three-layer bridge suppression F0′
documented (`TESTING.md` §"The bridge suppresses report auto-display …",
`tools/opendss/README.md`). The text is kept here **verbatim**; the retirement
dispositions are the consumer inventory at the head of this file.

**Official-EPRI-OpenDSS oracle (opt-in, `tools/opendss/` — 2026-07-07):**
vendored EPRI `OpenDSSDirect.dll` r3723 (9.8.0.1) / r4088 (10.2.0.1) / r4133
(11.0.0.1) driven through the AltDSS Oddie bridge (dss-python 0.16.0b2 in a
separate venv, `PIN_OPENDSS.txt`), reusing `oracle_server.py` unchanged via
`DSS_ORACLE_ENGINE=oddie`. For inventorying upstream changes ahead of porting
them; the mandatory gate is untouched. Workflows (see `tools/opendss/README.md`):
`DSS_LIVE_OPENDSS=<rev> cargo test ... corpus_live_opendss` → report
`tmp/opendss_report_<rev>.json`, now partitioned against the triage catalog
`tools/opendss/known_diffs.json` (2026-07-07, modeled on DSS-Python
`KNOWN_COM_DIFF`; substring match on case label + first-failure reason, every
entry states its cause, zero-hit entries warn). r3723: 150 matched / 82
known-diverged / **0 new** of 232 — all 82 triaged into 11 classes (19 EPRI
InvControl max-iter failures, 25 InvControl fixpoint drift, 10 iteration
deltas, 8 monitor-header whitespace, 4 property-format brackets, 4
injection FPC-vs-Delphi ulp, 3 storage kWhStored drift, 3 meter ZonePCE
count, 3 event-log trailing space, 2 GenDispatcher prop-name, 1 harmonics
Y-fingerprint) — so `DSS_LIVE_OPENDSS_ASSERT=1` (fails only on NEW) is green
for r3723. Caveat: comparison stops at a case's first divergence — a known
first divergence masks later ones in that case (accepted for inventory).
`ab_compare.py --a oddie:r3723 --b oddie:r4133` → upstream-change inventory
(baseline: 109/168 match; deltas in distance relays, harmonics, InvControl
iteration behavior); `--known-diffs tools/opendss/known_diffs.json` relabels
fully-triaged cases `known_diverged` (entries carry `ab_contains` where this
tool's issue wording differs) and exits 0 when only known diffs remain.
Two operational gotchas, both handled: (1) EPRI's Delphi `FireOffEditor`
ShellExecutes the editor on every `Show`/`Export` with NO `NoFormsAllowed`
check and Oddie can't set `AllowEditor` — a corpus sweep opened hundreds of
Notepads; `make_engine()` now issues `Set RegistryUpdate=No` + `Set
Editor=rundll32.exe` (silent no-op; registry write suppressed so the user's
OpenDSS editor setting is untouched) — verified on all 3 revisions with a
`Show` deck, zero spawns. (2) `.inputs/electricdss-tst` is now a re-checkout
with different EOLs: `tools/corpus/vendor.py --force` produces a ~1544-file
EOL-only diff — clean run pollution with `git restore tests/corpus` instead;
re-vendor only deliberately.

**DSS-Python validation harness, vendored (`tools/opendss/dsspy_validation/` — 2026-07-07):** copy of DSS-Python `fastdss` `tests/`
`_settings`/`save_outputs`/`compare_outputs` (BSD-3, attribution headers, local edits marked `# dss-rs:`): full-API-state dumps (~40
collections/case, 206 upstream-curated cases, all present in our corpus) zipped per engine + offline tolerant diff (their `KNOWN_COM_DIFF`
catalog kept as upstream) — broad-surface upstream inventory complementing `ab_compare.py`. Adaptations: corpus → vendored copy, engine spec
`DSS_EXTENSIONS_TEST_ODDIE=oddie:<rev>` via `revisions.json` (+ expect_version hard check), COM branch dropped, our
`RegistryUpdate=No`+`Editor=rundll32.exe` suppression, per-case `CorpusGuard` (lifted move-only into `tools/oracle/corpus_guard.py`, shared with
oracle_server), results → `tmp/dsspy_validation/`, and `(Oddie)`-prefixed DSSException skips for API exports absent from older official DLLs
(r3723 lacks `Transformers_Get_LossesByType`, `StoragesI`, ...). **Its `capi` side is dss_capi 0.15.0b4 — NOT the pinned 0.14.5 oracle; inventory
only, never feeds goldens/gate.** pandas+xmldiff pinned into the Oddie venv (`PIN_OPENDSS.txt`). Sweeps must end with `git status tests/corpus`
(the guard was non-recursive until GOLDEN_REBASE G1.10a, 2026-09-06; it now sweeps a run-created *subdirectory* — 123Bus `Run_YearlySim`'s
`16Nov2011/` — whole, and never descends into a pre-existing one). Full-sweep baseline 2026-07-07: capi 199/206 captured, oddie:r3723 189/206
(its 19 misses = the `epri-invcontrol-maxiter` #485 class, 1:1 with known_diffs), compare processes 3885 zip entries. The two beta packages are
vendored as wheels in `tools/opendss/wheels/` (+SHA256SUMS; offline `--find-links` install proven) — setup no longer depends on the pre-releases
staying on PyPI.

**DSS-Python corpus cross-check (`tools/corpus/dsspy_crosscheck.py` —
2026-07-07):** diffs DSS-Python's own 206-case validation list
(`.inputs/DSS-Python/tests/_settings.py::test_filenames`, extracted textually
— importing that module binds a DSS engine) against our six classifier
manifests → `tmp/dsspy_crosscheck.{json,md}`. Measured split: 133
solvable_now / 59 skipped_unsupported / 8 needs_investigation / 5
oracle_issue / 1 not_an_entry_point = **73 promotion candidates** (35
unblock at WP8.6 BatchEdit alone); all 206 exist in the vendored corpus.
`L!`-prefixed cases (55) are run line-by-line upstream with interactive
commands filtered — recorded per case so promotion work doesn't naively
`Compile` them.
