# UNIFIED_GATE_PLAN — Unified Manifest-Driven Test Gate + In-House r4133 Bridge

> **Status: PLANNED** (2026-07-18). Authored via ultracode workflow (3× Opus explore
> + Fable design, load-bearing facts verified against the tree at branch `update`).
> Execution: separate git worktrees (Phases A/B parallel; C+ serialized) — a parallel
> porting session runs in this repo, mind the CLAUDE.md junction protocol.
>
> Scope (user requirements):
> R1 — in-house the EPRI r4133 bridge (drop dss-extensions/Oddie entirely; Rust FFI
>      in a test-only unsafe-carve-out crate);
> R2 — unify the live oracle gate + synthetic families into ONE manifest-driven gate
>      with per-case `engines: capi_v0145 | r4133 | both` (default both), a gating
>      divergence ledger replacing known_diffs.json, per-channel exclusions for
>      proven upstream bugs; tolerances never loosened;
> R3 — maximum parallelism (persistent worker pools per channel, per-dir locking,
>      hand-rolled priority thread pool inside one #[test]).


Repo: `e:/RustProject/dss-rs`. All paths repo-relative unless absolute. Current baseline verified 2026-07-18: `solvable_now.json` = 293 cases (oracle specs: pinned×246, capi015×30, r4133×10, r3723×6, r4088×1), synthetic families 47/101/69 (asymmetric/controls/modes; oracle specs: capi015×29, r4133×30), 915 `.dss` bijection, 25 `known_diffs.json` entries.

## 0. Decision summary (one recommendation per decision point)

| # | Decision point | Chosen | Why (short) |
|---|---|---|---|
| D1 | r4133 bridge language | **Rust FFI worker process** (`libloading` over the Direct DLL) | Verified: the r4133 DDLL API is pure `cdecl` with C-plain types (`longint`/`double`/`PAnsiChar`/pointer+type+size out-params, NO OLE Variants, no COM) — see appendix. Rust removes the entire dss-extensions stack (beta wheels `dss-python 0.16.0b2` + `dss_python_backend 0.15.0b4`), removes ~1.5–2.5 s Python startup per call, gives resident workers for R3, and unifies the harness language. Keeping Python would still require a maintained venv + our own ctypes layer — all downside. |
| D2 | Multi-instance r4133 | **One DLL instance per worker *process***; N processes | `LoadLibrary` of the same path in *separate processes* is fully isolated — DLL-copy renaming is only needed for multi-instance in one process, which also risks Delphi RTL global collisions and shared file-output races. Process isolation additionally survives the known `#303` access-violation decks (worker dies, dispatcher respawns). |
| D3 | Channel names in `engines` | **`capi_v0145`**, **`r4133`**, **`both`** (default `both`) | The user's "capi015 (dss-python 0.15.7/dss_capi 0.14.5)" describes the *pinned* oracle; but the string `capi015` already denotes the retiring dss_capi **0.15.0b4** engine in `ORACLE_SPECS`/`make_engine` — reusing it would repurpose a live identifier and poison every historical WP note. `capi_v0145` names the *engine* version (dss_capi 0.14.5 = the vendored Pascal), avoiding both the string collision and the 0.15-engine-line connotation. **User-confirmed 2026-07-18.** |
| D4 | Fate of dss_capi 0.15.0b4 (`capi015`) engine | **Retired.** Its 59 target cases migrate to `engines:"both"` + capi_v0145-channel ledger entries where fingerprintable, else `engines:"r4133"` | 0.15.0b4 ≈ SVN r4103 ≈ r4133-line semantics; the r4133 bridge covers the same intent without dss-extensions. |
| D5 | r3723 / r4088 | **Delete bins + revisions entries + their known_diffs entries.** Their 7 solvable cases re-validated onto `r4133` and/or `capi_v0145`+ledger | User asked; nothing else consumes them once the inventory channel dies. |
| D6 | Ledger mechanics | **Field-granular, data-level envelope checks** (Option B), not panic-message substring matching | "Tight fingerprints (magnitude/location)" requires re-asserting the divergence *within a pinned envelope* on the actual data, and continuing to the next comparison channel — panic-substring matching (today's known_diffs) masks everything after the first divergence. |
| D7 | Test granularity | **One `#[test]` per manifest-union with an internal scheduler** + per-case `catch_unwind` + aggregate failure report | Keeps manifest-driven dynamism (no codegen), keeps population-lock semantics simple, reports *every* failing case (better than today's abort-at-first), and works identically under libtest and nextest. |
| D7a | Scheduler implementation | **Hand-rolled pool: pre-sorted `Vec<Task>` + `AtomicUsize` cursor + `std::thread::scope` — NOT rayon, NOT tokio** (decided 2026-07-18) | Gate tasks are blocking-I/O-heavy (seconds waiting on oracle subprocesses, deadline 120 s) — the anti-pattern for rayon's CPU-bound design; the global rayon pool is already owned by faer *inside* the engine solves, so the gate would need a dedicated oversubscribed `ThreadPoolBuilder` pool anyway; `par_iter` gives no longest-first priority (would need `par_bridge` bolt-on); and work-stealing buys nothing at ~510 coarse tasks. **Tokio also considered and rejected** (2026-07-18): concurrency is only T≈16–32 worker subprocesses (async's thread-economy buys nothing), half of each task is CPU-bound engine solve/compare that would live in `spawn_blocking` (= a plain thread pool inside the runtime anyway), async infects the whole worker-protocol client, and tokio has no priority scheduling either. The "wheel" degenerates to ~15 lines because the task set and weights are STATIC: sort `Vec<Task>` longest-first once + `AtomicUsize` cursor + `std::thread::scope` — no heap, no condvar; and the one genuinely tricky sync piece (pipe-read deadline on Windows) already exists in-tree as `corpus_live.rs::Oracle::call`'s drain-thread + `recv_timeout` pattern, reused persistently. Rayon stays where it belongs — inside faer. |
| D8 | Oracle transport | **Evolve the existing line-JSON stdio protocol to persistent workers** (the loop already exists in `oracle_server.py`) | The Rust doc comment already flags this as the intended optimization; both channels speak the identical protocol and produce the identical `CaseResult` shape, so `harness/mod.rs` comparators are untouched. |
| D9 | Case-dir contention | **Per-directory mutex in the scheduler**; within a case, Rust run → capi_v0145 run → r4133 run strictly sequential under the dir lock | Both engines and the Rust run all write into the case dir (`OutputDirectory := case dir`; CorpusGuard on both sides). Sandbox-copying breaks cross-dir `Redirect`s. Measured follow-up if the `Test/` chain dominates. |
| D10 | Goldens / harness comparators | **Untouched** (`tests/golden/`, `golden_*.rs`, `tools/golden/gen_*.py`, `harness/mod.rs` comparator internals) | Binding constraint; the ledger wraps comparator *call sites* in the gate, never the comparators. |

---

## 1. Target architecture

### 1.1 Component map

**Created**

| Path | What |
|---|---|
| `crates/dss-epri/` | New workspace member, `publish = false`, **the only crate without `#![forbid(unsafe_code)]`** (`#![deny(unsafe_op_in_unsafe_fn)]`, module-level `// SAFETY` docs, `#[cfg(windows)]`). Lib: `ffi.rs` (raw externs + `libloading`), `dss.rs` (safe-ish wrapper: command, V-protocol decode, error polling), `capture.rs` (CaseResult assembly mirroring `oracle_server.py` shapes byte-for-byte), `guard.rs` (corpus guard, port of `corpus_guard.py`). Bin: `src/bin/epri-worker.rs` — speaks the existing line-JSON protocol (`ping`/`run`/`quit`) persistently. New dep: `libloading` (workspace-pinned). |
| `crates/dss-core/tests/corpus_gate.rs` + `tests/corpus_gate/` dir module | The unified gate (successor of `corpus_live.rs`, git `mv` + refactor to preserve history). Submodules: `manifest.rs` (schema v2 + `Family` incl. the vendored family), `ledger.rs` (ledger load/validate + envelope checks + hit accounting), `engines.rs` (`WorkerPool`, persistent protocol client, respawn/timeout), `scheduler.rs` (thread pool, dir locks, longest-first ordering, aggregation), `runner.rs` (`run_rust_capture` + `compare_capture(channel, policy, ledger)` split of today's `run_and_compare`). |
| `tests/corpus/ledger.json` | The committed divergence ledger (§1.3). Replaces + absorbs `known_diffs.json`. |
| `tools/opendss/README.md` (rewritten) | New doc: r4133-only artifact, Rust worker, re-vendor procedure. |

**Kept / evolved**

| Path | Change |
|---|---|
| `tools/oracle/oracle_server.py` | Kept for the **capi_v0145 channel only**. Delete `capi015`/`oddie` branches of `make_engine`, `_oddie_get_y_sparse`, `IOddieDSS` special cases in `capture_eventlog`. The persistent `while True: readline` loop is already there — no protocol change needed server-side. Capture helpers stay shared with `tools/golden/gen_checkpoints.py`. |
| `crates/dss-core/tests/harness/` | Comparators and `Tolerances`/`tol_for` **unchanged** (shared with goldens). Only additive: nothing — envelope checks live in `corpus_gate/ledger.rs`. |
| `crates/dss-core/tests/population_lock.rs` | Rigor fingerprint v2 (§1.4); extended to family manifests. |
| `crates/dss-core/tests/corpus_manifest.rs` | Kept as-is (ownership bijection over 915 `.dss`). |
| `tools/opendss/bin/r4133/`, `bin/SHA256SUMS`, `bin/README.md`, `vendor_binaries.py`, `revisions.json` | Kept, pruned to r4133 only. |
| `tests/corpus/manifests/solvable_now.json` | Becomes the **vendored family manifest** in schema v2 (name kept — every doc/lock/tool references it). |
| `tests/corpus/{asymmetric,controls,modes}/manifest.json` | Schema v2 (mechanical: `oracle` → `engines`). |

**Deleted** (Phase 6, after cross-validation proves the new bridge)

- `tools/opendss/.venv/` (gitignored; **delete in main only**, junction protocol §5-R5), `wheels/`, `PIN_OPENDSS.txt`
- `tools/opendss/{smoke.py, ab_compare.py, dsspy_crosscheck.py, dsspy_validation/, gen_ad_reference.py, probe_59n.py, sweep_modes_isolated.py, sweep_merge.py}` (all dss-python/Oddie-dependent; smoke replaced by `epri-worker --smoke`; ab_compare subsumed by the gate's report mode; note in README that AD-reference regen needs reimplementation over `epri-worker` if ever re-run)
- `tools/opendss/bin/r3723/`, `bin/r4088/` (+ their `SHA256SUMS`/`README.md` rows, `revisions.json` entries)
- `tests/corpus/known_diffs.json`, the `corpus_live_opendss` test, `struct KnownDiff`/`load_known_diffs`, `Oracle::capi015`, `Oracle::opendss`, `DSS_LIVE_OPENDSS*` env knobs
- `crates/dss-core/tests/corpus_live.rs` (renamed into `corpus_gate.rs` — the old *path* disappears)

### 1.2 Manifest schema v2 (`SolvableCase` v2)

One schema for all four families (`asymmetric`, `controls`, `modes`, and the vendored family at `manifests/solvable_now.json`). Case key for ledger/lock purposes: `"<family>:<path>"` (vendored family = `corpus:<path>`, matching today's label convention).

| Field | Type / default | Semantics |
|---|---|---|
| `path` | string, required | Deck path relative to family dir. |
| `engines` | `"capi_v0145" \| "r4133" \| "both"`, **default `"both"`** | Which channel(s) gate this case. `capi_v0145` = pinned dss-python 0.15.7 / dss_capi 0.14.5 via `oracle_server.py`; `r4133` = official EPRI `bin/r4133/OpenDSSDirect.dll` via `epri-worker`. Replaces the `oracle` field (`ORACLE_SPECS` deleted). |
| `kind` | string, default `"feeder"` | Tolerance tier (`tol_for`) — unchanged tiers, unchanged floors, both channels use the same tier. |
| `post` | `[string]`, `[]` | Commands after `compile`. |
| `n_steps` | usize, `1` | Solve steps / checkpoints. |
| `selected_elements` | `[string]`, `[]` | YPrim focus set; `["*"]` = all (with completeness assert, unchanged). |
| `check_meters_monitors` | bool, false | Monitor channels + meter registers/zone per step. |
| `probes` | `[{element, props:[..]}]` | `? element.prop` probes. |
| `compare_variables` | `[string]` | PC-element `AllVariableValues`. |
| `compare_eventlog` / `compare_ctrlqueue` / `compare_all_properties` / `compare_global_result` / `compare_autoadd_log` | bool | Unchanged. *(Superseded 2026-09-03 by R4133_PROPS RP4.1: the compare runs on both channels; only the `kind=large*` guard survives.)* `compare_all_properties` remains family-forced **on the capi_v0145 channel only** and never for `kind=large*` (r4133's 0.15.x-shaped tables are exactly what `PROPS_015X` exists for; keep property parity pinned to capi_v0145). |
| `pending` | bool, false | Unchanged (`assert_pending_errors_loudly`; applies regardless of engines). |
| `expect_solve_abort` | string? | Unchanged; now valid on both channels (the Rust worker captures the DLL error first-class — an r4133 abort-message delta becomes a ledger `divergence` on the abort message, or the case narrows to `engines:"capi_v0145"` with cause). |
| `expect_warnings` | `[string]` | Unchanged (drives `warn_and_continue`, errnos 567/570/1570). |
| `wp` | string? | Unchanged; mandatory while `pending`. |
| `ad` | `"full"\|"pf"\|"off:<reason>"`, mandatory | Unchanged. |
| `isolate` | bool, default false | **New.** Run every engine execution of this case in a throwaway one-shot worker process (both channels). Seeded for: the AutoAdd deck (dss-python segfaults at process exit — a persistent worker would die post-reply), the YgD winding-rewire deck if the persistent-vs-oneshot A/B (Phase 3) shows order sensitivity, and any deck the A/B proof flags. |
| `note` | string | Free-text provenance (ignored by deserialize, as today). |

Structural checks kept and extended in `family_manifest_is_complete` / new vendored-family check: dir↔manifest bijection, `required` floors, `pending ⇒ wp`, valid `ad`, valid `engines`, `isolate` must carry a `note`.

### 1.3 Divergence ledger schema (`tests/corpus/ledger.json`)

Replaces `known_diffs.json` as a **gating** input (today's file is report-only). Principles: per-case-per-channel, exact case keys (no substring matching), measured envelopes, mandatory cause, **fail-on-stale**.

```json
{
  "version": 1,
  "comment": ["Read by corpus_gate; both channels gate commits. Entries mask NOTHING",
              "silently: each pins where/how-much and FAILS the gate when stale."],
  "causes": {
    "fpc-delphi-ulp": "Delphi-vs-FPC last-ulp libm differences amplified through ... (WP-U0.2)",
    "iteration-count-delta": "Solve/control fixpoint iteration deltas vs upstream r4133 ...",
    "vsconverter-getcurrents-bug": "Upstream self-aliased MVMult; reads violate KCL and mutate state (investigations/...)"
  },
  "entries": [
    {
      "id": "r4133-ieee519-harmonics-mags",
      "case": "corpus:Version8/Distrib/IEEETestCases/IEEE_519/....dss",
      "channel": "r4133",
      "kind": "divergence",
      "match": [
        {"field": "iterations", "policy": "rust_le_oracle"},
        {"field": "voltages", "steps": [2, 3], "node_re": "(?i)^busx\\.", "max_rel": 3.1e-6, "max_abs": 0.0}
      ],
      "cause_ref": "fpc-delphi-ulp",
      "source": "WP-U0.2 sweep 2026-05-30; upstream Solution.pas r4110 change",
      "measured": {"max_rel_seen": 2.4e-6, "date": "2026-08-01"}
    },
    {
      "id": "r4133-linespacing-crash",
      "case": "corpus:.../IEEE13_LineSpacing.dss",
      "channel": "r4133",
      "kind": "skip",
      "cause": "EPRI r4088+r4133 raise #303 access violation at calcv (known_diffs epri-linespacing-r4088-crash)",
      "source": "WP-U0.2"
    },
    {
      "id": "vsconverter-currents-capi_v0145",
      "case": "corpus:.../VSC_case.dss",
      "channel": "capi_v0145",
      "kind": "exclusion",
      "match": [{"field": "element", "name_re": "(?i)^vsconverter\\.", "channels": ["currents", "powers", "losses"]}],
      "cause_ref": "vsconverter-getcurrents-bug",
      "source": "investigations/, exec/tests/vs_converter.rs remains the dedicated gate"
    }
  ]
}
```

**Entry kinds**

- `divergence` — the case runs and is fully compared; the listed `match` scopes are *expected to diverge* and are re-asserted **inside their pinned envelope**; everything outside the scopes must meet the base tier. Envelope semantics per field kind:
  - `iterations`: exact `{rust, oracle}` pair per step, or `policy:"rust_le_oracle"` (the only policy value; precedent from target-rev cases).
  - `voltages` / `injection` / `element` (currents/powers/losses) / `yprim` / `y_fingerprint` / `monitor` / `meter`: location selector (`node_re`/`name_re`/`register_re`/`channel_idx`, optional `steps`) + `max_rel`/`max_abs` envelope. Gate asserts: (a) selected values differ from oracle by ≤ envelope; (b) unselected values meet the tier floor; (c) at least one selected value *exceeds* the tier floor (else the entry is stale → **gate fails** with a prune instruction).
  - `probe` / `property` / `global_result`: exact expected `{rust, oracle}` strings, or `num_rel` for numeric-skeleton values.
  - `eventlog` / `ctrlqueue`: line-level masks (`line_re` on the diffing lines, e.g. trailing-space class) — per-channel successor of `EVENTLOG_MASKS`.
- `skip` — the case is not sent to that channel at all (hard-crash decks: `#303`/`#58614`). Requires `cause` naming the crash. The other channel still gates the case (an `engines:"both"` case with an `r4133 skip` still runs on capi_v0145).
- `exclusion` — proven upstream bug poisons specific comparison scopes on a channel (VSConverter currents on **both** channels; per-channel where applicable); the scoped comparisons are skipped, everything else compared. Requires `cause_ref` into a documented investigation.

**Rules enforced by `ledger.rs` (structural, oracle-free test):** unique ids; `case` must exist in a manifest and the entry's `channel` must be in that case's `engines`; `divergence` needs non-empty `match`; every entry needs `cause` or `cause_ref`; `cause_ref` must resolve; regexes compile. **Runtime rule:** every applicable entry must be *hit* (its scoped divergence observed within envelope) — zero-hit fails the gate. Tolerance tiers are never touched: envelopes are per-case, per-channel, per-scope committed facts with provenance, and every envelope addition/change is visible in the population lock (§1.4).

**Gate flow with ledger** (`runner.rs::compare_capture`): iterate comparison channels in today's fixed order (iterations → node order → voltages → Y → fingerprint → yprims → injection → elements → discrete → monitors/meters → probes → variables → eventlog → ctrlqueue → exports → all-properties). Before each, partition by the case+channel ledger scopes: run the untouched `harness` comparator on the unscoped remainder; run the envelope assert on the scoped part; record hits. Discrete state remains exact-only — a discrete divergence can be ledgered only as exact expected `{rust, oracle}` values (`probe`-style), never an envelope.

### 1.4 Population lock v2 (`population_lock.rs` + `manifests/population.lock.json`)

- `Case::rigor` string extends to: `... engines={capi_v0145|r4133|both} isolate={bool} ledger={sorted entry ids applicable to this case, per channel}` (drop `oracle={}`).
- **Fix the documented asymmetry**: per-case rigor fingerprints now cover the three synthetic family manifests too, not just `solvable_now.json` (family counts + path lists kept as well).
- Ledger inclusion means: adding a mask entry, widening its scope set, flipping `both→capi_v0145`, cutting steps, or dropping a case **all** produce a lock diff → the same one-command regen + reviewable diff discipline as today. Same `DSS_UPDATE_POPULATION_LOCK` flow, same precise diff messages, plus new lines: `solvable_now ENGINES <path>: both -> capi_v0145`, `LEDGER <path>/<channel>: +id/-id`.

---

## 2. The r4133 bridge design

### 2.1 Choice: Rust FFI (justification)

Verified against the r4133 Pascal source (`.inputs/electricdss-code-r4133-trunk/Version8/Source/DDLL/`):

- `OpenDSSDirect.dpr` exports a flat `cdecl` surface (full list in the `exports` clause, lines 243–285). Signatures are C-plain: `DSSPut_Command(PAnsiChar): PAnsiChar`; `XxxI(longint, longint): longint`; `XxxF(longint, double, double): double`; `XxxS(longint, PAnsiChar): PAnsiChar`; `XxxV(longint; var Pointer; var longint myType; var longint mySize)`.
- The V-protocol (verified in `DCircuit.pas` + `DSSGlobals.pas`): the DLL owns global dynamic arrays (`myIntArray`/`myDBLArray`/`myCmplxArray`/`myStrArray`), fills them, returns a pointer + type tag (2=double, 3=complex, 4=string-bytes; tag 1=integer — confirm during implementation) + byte size. Strings are `\0`-separated byte runs (`WriteStr2Array` + `Char(0)`). Caller copies, never frees. Single-threaded per process — exactly our worker model.
- No COM registration, no OLE Variants, no BSTR, no callbacks, no header needed. This is a weekend-sized FFI, not a project.

Python retention would keep two beta, unmaintained dss-extensions pins alive solely to marshal a C API that Rust reads natively, and would keep per-call interpreter costs. **Rust FFI wins on every axis the requirements name.** The unsafe carve-out: `crates/dss-epri` only; product crates keep `#![forbid(unsafe_code)]`; `cargo clippy --workspace -- -D warnings` covers the new crate.

### 2.2 FFI surface (concrete entry points)

Init sequence per worker process: `LoadLibraryExW("tools/opendss/bin/r4133/OpenDSSDirect.dll", LOAD_WITH_ALTERED_SEARCH_PATH)` (resolves `KLUSolve.dll` beside it) → `DSSI(8, 0)` (**AllowForms write: arg=0 sets `NoFormsAllowed := TRUE`** — note the inversion, `DDSS.pas` line 67) → `DSSS(1, "")` (Version; assert contains `"Version 11.0.0.1"` from `revisions.json`) → ready. The DLL self-initializes its `TExecutive` in the library init block.

| Need (capture parity with `oracle_server.py`) | Entry points |
|---|---|
| compile / post / solve / `? elem.prop` probes / `export eventlog` / clear | `DSSPut_Command` (returns accumulated `GlobalResult` — also serves `global_result` capture when read immediately after `solve`) |
| error semantics after every command | `ErrorCode()`, `ErrorDesc()` (`DError.pas`); replicate dss-python's raise-on-nonzero, tolerated sets `{250}` and `{567,570,1570}` under `warn_and_continue`. No EarlyAbort exists — the raw DLL warns-and-continues, which is today's Oddie behavior (`_set_early_abort` no-ops). |
| NumNodes / YNodeOrder / YNodeVarray / AllElementNames / SetActiveElement | `CircuitI(2,·)`, `CircuitV` (mode table in `DCircuit.pas`), `CircuitS` |
| element currents/powers/losses/Yprim/NumTerminals/AllVariable* | `CktElementI/F/S/V` |
| iterations / converged / dblHour | `SolutionI`, `SolutionF` |
| sparse system Y (CSC) | `InitAndGetYparams` → `GetCompressedYMatrix` (`DYMatrix.pas`; **always factors** — port `smoke.py` step 4's solution-neutrality proof: `YNodeVarray` bit-identical around the export, as a worker self-test; on failure the worker refuses `full_csc`) |
| injection RHS | `getIpointer` (+ `CircuitI(2,·)`; length `2*(NumNodes+1)`, slot 0 = ground) |
| monitors (header/sample_count/channels) | `MonitorsI/S/V` (ByteStream parse — same binary layout dss-python decodes; cross-validated bit-for-bit in Phase 2) |
| meters (registers, zone lists) | `MetersI/F/S/V` (replicate the `NONE`-placeholder and Delphi trailing-empty-element filtering from `capture_all_meters`) |
| ctrlqueue | `CtrlQueueI`, `CtrlQueueV` (drop header/`No events` rows, as `capture_ctrlqueue`) |
| all-properties enumeration | `DSSPut_Command("? name.Like")` + `DSSElementV` (AllPropertyNames) — the WPG.1-safe path (not needed for gating: all-props stays capi_v0145-only; implement anyway for report tooling parity) *(Superseded 2026-09-03 by R4133_PROPS RP4.1: it IS needed for gating — the r4133 property table is compared.)* |
| discrete state | `TransformersI/F/S`, `RegControlsI/F/S`, `CapacitorsI/S/V` (mirror `gc.capture_discrete`) |
| eventlog | `DSSPut_Command("export eventlog")` → read CSV with UTF-8-BOM strip (port the `utf-8-sig` + per-line `\u{FEFF}` logic from `capture_eventlog`, r4133-specific WP-U2.5 behavior) |

The worker wraps each `run` in a Rust port of `corpus_guard.py` (recursive snapshot, delete created, restore <2 MiB overwrites) — same hygiene contract.

### 2.3 r4088→r4133 gaps to close

The bridge is written **directly against the r4133 source** (verified above), so r4088-era assumptions die with the old bridge. Items carried forward deliberately: the eventlog BOM behavior (r4133 DLL writes BOM); crash-skip knowledge (`#303` on `binaryshape`, `IEEE13_LineSpacing`, `IEEE13_LineAndCableSpacing`, `capcontrolfollow` decks → ledger `skip` entries on `r4133`); `harmonics-ieee519-r4133` divergence class; the always-factor CSC export neutrality re-proof.

### 2.4 Bridge fidelity validation ("how do we know the bridge is faithful?")

1. **Self-smoke** (`epri-worker --smoke`, replaces `smoke.py`): DLL loads, version string matches `revisions.json` `expect_version`, IEEE13 compile+solve converges, CSC export solution-neutral (bit-identical `YNodeVarray`), `getIpointer` length = `2*(NumNodes+1)`. Wired as an oracle-free `#[test]` in `dss-epri`.
2. **Cross-validation against the outgoing Oddie path (the decisive proof):** *before deleting anything Python*, run every case in today's `corpus_live_opendss` universe through **both** the Python/Oddie r4133 engine and `epri-worker`, and diff the raw `CaseResult` JSONs **bit-for-bit** (both drive the same DLL binary; every capture must be identical — same doubles, same strings). A small diff tool (`tools/opendss/xcheck_bridge.py`, temporary) reports any field mismatch. Known allowed differences: none — both read the same engine memory; any diff is a bridge bug. DONE bar: empty diff over the full universe, twice (order-shuffled).
3. **Ongoing:** the ledger-gated r4133 channel green under `cargo test`, with fail-on-stale keeping entries honest; the capi_v0145 channel continues to pin 0.14.5 exactly, so a bridge regression shows up as a *systematic* r4133-only divergence pattern.

---

## 3. Parallel execution design

### 3.1 Worker pools

- **capi_v0145 pool:** N persistent `python -u tools/oracle/oracle_server.py` processes (`DSS_ORACLE_ENGINE=capi_v0145`), spawned once per gate run, ping-verified (pin check per process, unchanged). Per-worker dedicated stdout/stderr drain threads (the existing anti-deadlock pattern, now long-lived); strictly one in-flight request per worker; per-request deadline (`DSS_ORACLE_TIMEOUT_SECS`, default 120) → on timeout/EOF/crash: kill, respawn, retry the case once on a fresh worker, then fail *that case*. Worker recycled after 64 cases (bounds drift) and immediately after any `isolate` case (which runs on a dedicated throwaway worker).
- **r4133 pool:** N persistent `epri-worker` processes, same protocol, same lifecycle. DLL crashes (`#303` decks not yet ledgered) kill only that worker; dispatcher respawns and attributes the failure to the case.
- Pool sizing: `DSS_GATE_JOBS` env override; default `available_parallelism()` for the scheduler thread count T, and per-channel pool size `max(2, T/2)` (oracle work per case ≈ Rust work; BOTH doubles oracle demand but the two channels run on distinct pools concurrently).
- Worker binary resolution: `DSS_EPRI_WORKER` env override → default `<workspace>/target/<profile>/epri-worker(.exe)`; if missing, the gate runs `cargo build -p dss-epri --bin epri-worker` once (OnceLock) and fails loudly with instructions if that fails. (`cargo test --workspace` builds the bin anyway; the fallback covers `cargo test -p dss-core` invocations.)

### 3.2 Protocol

Unchanged line-JSON (`{"cmd":"run"|"ping"|"quit"}` / `{"ok":…}`) — now actually used persistently, which `oracle_server.py` already supports (`while True: readline`). Additive only: `ping` result for the epri-worker carries `{"engine": "<Version string>", "epri": true, "rev": "r4133", "dll": "<abs>"}` (the Rust client asserts these markers, replacing `oddie` markers). `run` request/response shapes are byte-compatible with today's — `CaseResult { node_order, n_steps, checkpoints, autoadd_log }`.

### 3.3 Scheduling, determinism, attribution

- One `#[test]` (`corpus_gate_all_cases_match_engines`) builds the task list = union of all four manifests (510 cases today), sorts **longest-first** (static weight: `kind` tier × `n_steps`, with the known yearly/8500 decks pinned to the front), and runs it on T scheduler threads (D7a: hand-rolled pool — pre-sorted `Vec<Task>` + `AtomicUsize` cursor + `std::thread::scope`; not rayon/tokio — tasks block for seconds on oracle I/O and the global rayon pool belongs to faer inside the engine solves; T may oversubscribe cores since threads are I/O-parked most of the time). Simplification option during Phase B: make *task = case-dir group* (its cases run sequentially inside the task, longest-dir-first) — removes the D9 mutexes entirely at identical effective parallelism; pick whichever falls out cleaner.
- **Per-directory mutex** (D9): tasks keyed by canonical case-dir; a case holds its dir lock across [Rust run → capi_v0145 `run` → r4133 `run`]; comparisons (pure) run after lock release. Different dirs proceed fully parallel. The Rust engine has no global mutable state (verified: no `static mut`/`OnceLock`/`Mutex` statics in `dss-core`/`dss-parser`/`dss-sparse` sources), so per-thread `Dss` instances are safe.
- **Rust runs once per case** (`run_rust_capture`), compared against each channel's `CaseResult` with per-channel policy: capi_v0145 = exact iterations (unchanged 1:1 contract) unless a ledger `iterations` entry; r4133 = `rust_le_oracle` default (the existing precedent, now channel-wide) + ledger.
- Each case comparison wrapped in `catch_unwind`; results collected into a deterministic report (manifest order, per case × channel: `ok | diverged(first reason + full context) | ledger_hits[…] | skipped(entry id)`). The `#[test]` fails iff any case failed or any ledger entry is stale, printing the **complete** failure list — better attribution than today's abort-at-first-case.
- Determinism guard: scheduling order must not affect results. Proven in Phase 3 (§4) by shuffled-order double-run bit-diff; any order-sensitive case gets `isolate: true` with cause.

### 3.4 cargo test integration + expected speedup

- Plain `cargo test --workspace` runs everything; the internal pool is independent of libtest's `--test-threads` (the gate is one test; other test binaries — goldens, unit tests — run concurrently as today, a minor CPU tax accounted in pool sizing). nextest-compatible (the gate is one unit there too).
- Cost model: today ≈ Σ over 510 cases of (Python spawn ~1.5–2.5 s + oracle solve + Rust solve), strictly serial. New: spawn cost amortized to N one-time startups; cases parallel across dirs. Removing ~510 × ~2 s ≈ 17 min of pure spawn overhead alone; with T=16 the target is **≤ 10 min wall for the full BOTH gate** (vs. an expected 45–90 min serial baseline — Phase 0 measures the real number; the long pole becomes the largest single case ≈ the yearly EPRI feeder at opt-level 3, plus the longest same-dir chain, likely `Test/`). Recorded before/after in `STATUS.md`.

---

## 4. Migration sequence

Phases sized for parallel worktree agents; each lands green through the unchanged three-command gate. Phases A and B are independent (separate worktrees); C+ serialize on the shared `corpus_live.rs` surface — coordinate merge windows with the parallel porting session (§5-R6).

**Phase 0 — Baseline (main, no worktree, ~half a day).** Measure and record in `STATUS.md`: full-gate wall-clock (`cargo test --workspace` timed), per-manifest counts (293/47/101/69), `known_diffs` entry list, `git tag pre-unified-gate`. DONE: numbers committed.

**Phase A (worktree 1) — `dss-epri` crate + worker + smoke.** Build §2.2 exactly; `epri-worker --smoke` + `#[test]` smoke green; clippy/fmt clean; no consumer changes yet. DONE: smoke test green in `cargo test --workspace`; crate carve-out documented (crate-level doc + PORTING_PLAN note that `#![forbid(unsafe_code)]` scope is product crates).

**Phase B (worktree 2) — Persistent pools + scheduler, capi_v0145 channel only, behavior-identical.** Refactor `corpus_live.rs` → `corpus_gate.rs` module tree (git mv); split `run_and_compare` into capture+compare; implement `WorkerPool` (persistent `oracle_server.py`), scheduler, dir locks, aggregate reporting. Manifest schema untouched in this phase (old `oracle` field still honored via a temporary shim mapping `capi015|r3723|r4088|r4133` → old one-shot `Oracle` paths, so nothing shrinks mid-flight). **Contamination proof:** run the full 510-case set (a) serial one-shot (old path), (b) persistent parallel, (c) persistent parallel shuffled — all three `CaseResult`+verdict sets must be bit-identical; divergent cases get `isolate: true`. DONE: gate green, three-way bit-diff empty (minus flagged isolates), wall-clock recorded (expect the capi_v0145-only win already).

**Phase C — Manifest v2 + lock v2 (after A+B merge).** Mechanical migration: add `engines` (initial values preserving current semantics exactly: pinned→`"capi_v0145"`, `oracle:"r4133"`→`"r4133"`; `capi015`/`r3723`/`r4088` cases →`"r4133"` *provisionally marked* `pending_engine_validation` in `note`); delete `oracle` field + `ORACLE_SPECS`; wire `engines:"r4133"` cases through the epri pool (their old Oddie/capi015 comparisons retire here — these 76 cases are re-validated live against the new bridge in this phase, with `run_and_compare_abort` and iteration policy `rust_le_oracle`). Population-lock v2 regen; `corpus_manifest.rs` untouched. DONE: gate green including all 76 re-targeted cases (or individually triaged into Phase D's ledger with cause), lock diff shows field additions + zero membership loss, counts 293/47/101/69 unchanged.

**Phase D — Ledger machinery + seeding + flip to BOTH.** (1) Implement `ledger.rs` (schema, structural test, envelope checks, hit accounting, fail-on-stale). (2) **Seeding run:** `DSS_GATE_SEED_LEDGER=1` runs every currently-`capi_v0145` case against r4133 in report mode, emitting *candidate* entries with measured locations/envelopes to `tmp/ledger_candidates.json` (never committed as-is). (3) Triage in batches by divergence class, importing prose/provenance from the 21 surviving `known_diffs` classes (drop r3723/r4088-only entries: `epri-invcontrol-maxiter`, `invcontrol-fixpoint-drift*`, `monitor-header-whitespace`, `meter-zonepce-count`, `harmonics-yfingerprint-drift`); every entry hand-reviewed for cause. (4) Flip triaged cases to `engines:"both"` in batches (suggest 4 batches: micro/feeder synthetics → feeder corpus → controls/modes → large tiers), lock regen per batch. Cases whose capi_v0145-vs-r4133 delta is not tightly fingerprintable (wholesale numeric shifts, e.g. Carson `De` change decks) stay single-channel with `note` cause. DONE: target ≥ 90% of cases at `engines:"both"`; gate green on both channels; every ledger entry hit; `known_diffs.json` + `corpus_live_opendss` + `DSS_LIVE_OPENDSS*` deleted.

**Phase E — Retire the Python EPRI stack + r3723/r4088 (MAIN repo, not a worktree).** Delete the Phase-6 list from §1.1 (Oddie venv **first neutralizing junctions per CLAUDE.md** — the venv is gitignored and junctioned into worktrees; delete only after all worktrees from A–D are removed via the reparse-point protocol); prune `oracle_server.py` engines; delete `bin/r3723`, `bin/r4088`, prune `revisions.json`/`SHA256SUMS`/`vendor_binaries.py`; verify `(gci .inputs -Force | measure).Count` unchanged after each removal. DONE: `rg -i "oddie|capi015|r3723|r4088|dss_python_backend|0\.16\.0b2"` returns only historical docs (STATUS/archives); gate green.

**Phase F — Docs + acceptance.** Rewrite `TESTING.md` layer map (unit / golden / **unified corpus gate (manifest-driven, two gating channels, ledger)** / corpus hygiene), procedures ("add a case", "triage a divergence into the ledger", "re-vendor r4133", "run the seeding report"); update `CLAUDE.md` (replace the opt-in-Oddie bullet; note the `dss-epri` unsafe carve-out; gate description), `tools/opendss/README.md`, `STATUS.md` (wall-clock before/after table). DONE: three-command gate green from a clean clone + pinned python + r4133 bin; docs merged.

---

## 5. Risk register

| # | Risk | Mitigation |
|---|---|---|
| R1 | **Bridge infidelity** (subtle capture mismatch vs dss-python's decoding — monitor ByteStream, string trailing-separators, meter `NONE` placeholders) | Phase A/§2.4-2 bit-for-bit cross-validation against the outgoing Oddie path over the full universe *before* any Python deletion; the Oddie venv survives until that diff is empty twice. |
| R2 | **Persistent-worker state contamination → order-dependent results** (`clear` does not reset all OpenDSS globals, e.g. `Set` options) | Phase B three-way bit-diff (serial vs parallel vs shuffled); `isolate:true` manifest flag for proven-sensitive decks (AutoAdd exit-segfault deck seeded); worker recycle every 64 cases; strictly per-request `clear`+Compile (already in `run_case`). |
| R3 | **Ledger becomes a soft-tolerance backdoor** (envelope creep, entry churn on upstream-sensitive cases) | Envelopes are per-case measured facts with mandatory cause+source; fail-on-stale forces pruning; every ledger change trips the population lock (reviewable diff); tier floors in `harness` untouched and structurally unreachable from ledger code; discrete state ledgerable only as exact expected pairs. |
| R4 | **BOTH-gating wall-clock blows the dev loop** | Resident workers kill spawn cost; per-dir parallelism; longest-first scheduling; Phase 0/B/D wall-clock checkpoints with a ≤ 10 min target; if the `Test/`-dir serial chain dominates, follow-up: sandboxed copies for same-dir *small* decks (explicitly deferred, measured first). |
| R5 | **Worktree junction data loss** (`.inputs`, `.venv`, `tools/opendss/.venv` are junctions in worktrees; recursive deletes follow them into main — has already wiped `.inputs` once) | All venv/bin deletions happen in **main only**, in Phase E, after worktrees are removed via CLAUDE.md's reparse-point-only protocol; never batch-remove worktrees; verify `.inputs` count after each removal. The r4133 DLLs are git-*tracked* (verified), so worktrees need no junctions for the bridge. |
| R6 | **Parallel porting session conflicts** (same repo; it edits manifests, `corpus_live.rs`, STATUS) | Phases B/C (the rename + schema flip) are announced merge windows kept mechanical and short; manifest edits are append-only JSON (low-conflict); the porting session's `oracle:`-field additions during the window are converted by a one-shot migration script re-run at merge. |
| R7 | **DLL licensing/redistribution** | Unchanged posture: the binaries are already committed; EPRI's `License.txt` (BSD-style) ships beside the DLL and stays; `bin/README.md` retains provenance + SHA256. Deleting r3723/r4088 *reduces* surface. METIS exes retained as-is (needed only if an EPRI-side AD path is ever exercised). |
| R8 | **FFI UB / crash decks destabilize the gate** | All FFI in one `#[cfg(windows)]` carve-out crate with `deny(unsafe_op_in_unsafe_fn)` + SAFETY docs; V-protocol data copied immediately; DLL only ever touched in worker processes — a `#303` access violation kills one worker, the dispatcher respawns and attributes; known crashers are ledger `skip` entries so they are never sent. |
| R9 | **Former target-rev cases fail re-validation on the new bridge** (capi015 0.15.0b4 ≠ r4133 exactly; 7 r3723/r4088 cases) | Phase C validates each live; fallback ladder per case: `engines:"r4133"` + ledger → `engines:"capi_v0145"` + ledger → (last resort) `pending:true` + `wp` — membership never shrinks, and the lock proves it. |

---

## 6. Verification (per phase and final)

- **Every phase:** `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace` green; `population_lock` green (regen only in C/D with reviewed diffs).
- **Counts that must never shrink:** 915 `.dss` bijection (`every_dss_is_accounted_for_exactly_once`); 293 vendored + 47/101/69 synthetic cases; `required` family floors; ≥ 1 multistep-meters case + ≥ 1 selected-elements case (`solvable_now_has_multistep_depth`); after Phase D: `engines:"both"` count is lock-pinned and any reduction is a lock diff.
- **Phase A:** `epri-worker --smoke` output (version/CSC-neutrality/getI) committed to STATUS; smoke `#[test]` green.
- **Phase B:** three-way result bit-diff artifact (empty) recorded; wall-clock table row 2.
- **Phase C:** lock diff reviewed: only field additions; 76 re-targeted cases individually listed green in the phase note.
- **Phase D:** gate report shows every ledger entry with hit-count ≥ 1; a deliberate canary (temporarily widen one envelope → gate must fail stale; revert) proves fail-on-stale live; wall-clock table row 3 (full BOTH).
- **Phase E:** `rg` sweeps prove no live references to oddie/capi015/r3723/r4088/dss-extensions; `.inputs` file count unchanged; clean-clone gate green.
- **Final acceptance:** before/after wall-clock in `STATUS.md`; TESTING.md layer map matches reality; `git tag unified-gate-v1`.

---

## 7. Key evidence appendix (implementer's map)

**FFI ground truth (r4133 Pascal source):**
- `E:\RustProject\dss-rs\.inputs\electricdss-code-r4133-trunk\Version8\Source\DDLL\OpenDSSDirect.dpr` — the complete `exports` clause (lines 243–285): `DSSPut_Command`, `{Circuit,CktElement,Solution,Monitors,Meters,CtrlQueue,DSSElement,ActiveClass,Capacitors,Transformers,SwtControls,CapControls,RegControls,Reclosers,Relays,Fuses,Sensors,LoadShape,Parser,PDElements,Storages,WindGens,Reactors,LineCodes,Lines,Loads,Generators,PVsystems,Vsources,Isource,GICSources,Topology,Settings,XYCurves,CmathLib,Parallel,ReduceCkt}{I,F,S,V}`, `ErrorCode`, `ErrorDesc`, `DSSI/DSSS/DSSV`, `InitAndGetYparams`, `GetCompressedYMatrix`, `getIpointer`, `getVpointer`, `SystemYChanged`, `BuildYMatrixD`, `SolveSystem`, …
- `...\DDLL\DDSS.pas` — `DSSI(8, 0)` ⇒ `NoFormsAllowed := TRUE` (inverted arg!); `DSSS(1,·)` = Version; `DSSS(3,·)` = DataPath write.
- `...\DDLL\DCircuit.pas` — V-protocol reference implementation (type tags 2/3/4 verified at lines 295–758; `\0`-separated string arrays at mode 6/7; `mySize` = bytes).
- `...\DDLL\DYMatrix.pas` — `InitAndGetYparams` **always** `FactorSparseMatrix` before export (the solution-neutrality proof obligation); CSC layout `ColPtr[nBus+1]/RowIdx[nNZ]/cVals[nNZ complex]`, DLL-owned heap.
- `...\DDLL\DText.pas` — `DSSPut_Command` splits on `\n`, accumulates `GlobalResult`, stops on `ErrorNumber > 0`, resets `SolutionAbort` on entry.

**Capture-shape contract to replicate byte-for-byte:**
- `E:\RustProject\dss-rs\tools\oracle\oracle_server.py` — `run_case` (lines ~376–535: checkpoint dict shape, `_RUN_ATTEMPTS=3`, `_TOLERATED_COMPILE_ERRNOS={250}`, `_USER_MODEL_ERRNOS={567,570,1570}`, `selected==["*"]` YPrim-bearing filter, GlobalResult-read-before-probes ordering, AutoAddLog read-inside-guard); `capture_eventlog` BOM handling (lines 109–147); `capture_all_meters` NONE/trailing-empty filtering (lines 255–295).
- `E:\RustProject\dss-rs\tools\golden\gen_checkpoints.py` — `capture_system_y` (`{n,rows,cols,re,im}`), `capture_yprim` (`{name,yorder,re,im}`), `capture_fingerprint`, `capture_injection` (`{re,im}`), `capture_element`, `capture_discrete` key shapes.
- `E:\RustProject\dss-rs\tools\oracle\corpus_guard.py` — guard semantics to port into the worker.

**Rust gate surfaces to refactor:**
- `E:\RustProject\dss-rs\crates\dss-core\tests\corpus_live.rs` — `struct Oracle` (136), `Oracle::call` one-shot spawn+drain+deadline (281), `run_case` request builder (425), `CorpusGuard` (518), `run_and_compare` (722; iteration policy 795–821), `SolvableCase` (1059), `Family` (1310), `family_manifest_is_complete` (1380), `family_cases_match_oracle` (1452), `run_and_compare_abort` (1516), `OraclePool` (1227), `corpus_live_solvable_cases_match_oracle` (1242), `corpus_live_opendss` (2364) + `KnownDiff` (2303) — the seed logic to absorb into `ledger.rs`.
- `E:\RustProject\dss-rs\crates\dss-core\tests\harness\mod.rs` — comparators + `tol_for` + `SKIP_PROPS`/`PROPS_015X`/`EVENTLOG_MASKS` (untouched; ledger wraps call sites only).
- `E:\RustProject\dss-rs\crates\dss-core\tests\population_lock.rs` — `Case::rigor` (~109), `data_eq`, regen flow.
- `E:\RustProject\dss-rs\crates\dss-core\tests\corpus_manifest.rs` — bijection gate (kept).

**Data/config:**
- `E:\RustProject\dss-rs\tests\corpus\manifests\solvable_now.json` (293; oracle: 246 pinned / 30 capi015 / 10 r4133 / 6 r3723 / 1 r4088), `population.lock.json`, family manifests, `tests\corpus\known_diffs.json` (25 entries → ledger seed), `tools\opendss\revisions.json` (`r4133` `expect_version` = "Version 11.0.0.1 (64-bit build) - Charlottesville"), `tools\opendss\bin\r4133\` (git-tracked: `OpenDSSDirect.dll` 11,823,616 B SHA `74f8bab3…`, `KLUSolve.dll`, License), root `Cargo.toml` (workspace members + the dev-profile opt-level-3 engine overrides that must extend to `dss-epri`).
- Docs to update: `TESTING.md`, `CLAUDE.md` (worktree junction protocol at lines 102–128 governs Phase E), `STATUS.md`, `tools\opendss\README.md`, `tests\TOLERANCE_NOTES.md` (add a "ledger is not a tolerance" paragraph).

---

### Critical Files for Implementation
- E:\RustProject\dss-rs\crates\dss-core\tests\corpus_live.rs
- E:\RustProject\dss-rs\tools\oracle\oracle_server.py
- E:\RustProject\dss-rs\.inputs\electricdss-code-r4133-trunk\Version8\Source\DDLL\OpenDSSDirect.dpr
- E:\RustProject\dss-rs\crates\dss-core\tests\population_lock.rs
- E:\RustProject\dss-rs\tests\corpus\manifests\solvable_now.json