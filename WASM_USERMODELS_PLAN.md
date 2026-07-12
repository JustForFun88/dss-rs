# WASM User Models Plan — replacing the user-written-DLL mechanism with WebAssembly

## Source-integrity gate — ritual step 0 (before the model-tier check)

The Pascal we port FROM — `.inputs/dss_capi` (186 `.pas` files), plus
`.inputs/electricdss-tst` for oracle/live work — is the **specification**. Before doing
anything, and re-checked continuously (not only at kickoff), confirm that folder exists
and is non-empty. If it has vanished — missing or empty — at **any** point in the work,
**STOP immediately**: make no edits, run no gate, and do **not** reconstruct, guess, or
"port" a source you cannot read. Tell the user the vendored source is gone and must be
re-vendored, then wait. Reply exactly:
**«Исходник порта (`.inputs/dss_capi`) отсутствует или пуст — работа остановлена. Восстанови
vendored-исходник (re-vendor) и повтори команду.»**
No spec → nothing to port; fabricating one from memory is a silent, unverifiable
divergence — far worse than stopping. This gate runs **ahead of the tier/refuse check**
(`PLAN_SEQUENCE.md` §Model-tier protocol).

**This plan needs two additional vendored sources** (checked by the same gate, same
STOP rule): `.inputs/electricdss-code-r3723-trunk` (the official trunk carrying the
canonical example user-model DLL source, `Version8/Source/IndMach012a/`) and
`.inputs/typst` (the vendored reference implementation of a pure-Rust wasmi plugin
host, `crates/typst-library/src/foundations/plugin.rs`). Missing → same stop-and-report,
naming the folder that is gone.

> Companion to `PORTING_PLAN.md`; same rules of engagement as `PHASE8_PLAN.md` §0 /
> `GAPS_PLAN.md` §0 (probe the oracle, never guess; `TODO(compat)`/`NOT_PORTED`
> discipline; goldens regenerated **manually** with the pinned oracle
> `tools/golden/PIN.txt`; the standard three-command gate green per step; commit only
> on explicit user request).
>
> **What this plan is — and is NOT.** This is **post-acceptance improvement-era work**
> (PLAN_SEQUENCE stage 9), *not* a 1:1 port item. Upstream loads user-written **native
> DLLs** (`LoadLibrary`/`GetProcAddress`, Stdcall) — permanently impossible under
> `#![forbid(unsafe_code)]` / "no C bindings ever" (CLAUDE.md), which is why every
> prior plan lists the DLL hooks under "never (safe Rust)". This plan replaces the
> *transport* (native DLL → sandboxed WebAssembly over pure-Rust **wasmi**) while
> keeping the *contract* 1:1: the same element properties (`UserModel=`, `UserData=`,
> `ShaftModel=`, `ShaftData=`, `DynaDLL=`, `DynaData=` — **no new property names**),
> the same 15/13/7-function interface shapes, the same call ordering, the same
> shared-record semantics, and the same warn-and-fallback failure behavior. The Pascal
> stays the spec for the **contract**; the vendored typst plugin host is the
> **implementation pattern** for the wasmi side; the pinned oracle **with a real
> native DLL loaded** is the numeric gate (§2.5).
>
> **The non-negotiable invariant:** every existing gate stays green untouched. The
> current corpus decks that set these properties (`Test/indmachtest`,
> `Kersting4wire_Lagging/Leading`, `SimpleStorageTest*`) reference native DLL names
> that resolve to **no `.wasm` file** — they MUST keep today's warn-and-fallback path
> and their `expect_warnings=["Not Loaded"]` gating **byte-identical** (§2.4). WASM
> activation is strictly additive: it triggers only when the property value resolves
> to an actual `.wasm` module.
>
> **Stop-and-confirm cadence (same as `PHASE8_PLAN §0`):** after each WP (or a
> self-contained step of one) run the per-step ritual below **autonomously, without
> pausing between its sub-steps**; the single stop point is at the very end — then
> wait for the user's explicit confirmation (unless the user authorized several WPs
> in one pass).
>
> **Per-step ritual (do every step, in order, without being told):**
> 0. **Tier check** (protocol: `PLAN_SEQUENCE.md` §Model-tier protocol). Look up the
>    WP's **exec tier** in the tier table below (§0-tiers); spawn both auditors
>    (`/audit-code` + `/audit-tests`) with an **explicit model/effort override**
>    matching the WP's audit tier (never "whatever the session runs"). If the session
>    is below the exec tier, do NOT execute; reply exactly: «Этот шаг требует <exec
>    tier>. Переключи сессию (/model + reasoning effort) и повтори команду.» and stop.
> 1. **Gate green** — `cargo fmt --all --check`; `cargo clippy --workspace
>    --all-targets -- -D warnings`; `cargo test --workspace` (all goldens + the
>    always-on live corpus gates + the new `wasm_usermodels` tests as they land). No
>    `#[ignore]`, no name-filter that could green on zero matches. A red test blocks
>    the commit. The committed `.wasm` fixtures make the suite hermetic — `cargo test`
>    never needs FPC, a wasm toolchain, or a native DLL (§2.6).
> 2. **Update `STATUS.md`** (the §1 frontier + a §WASM-UM record), **commit** (code +
>    STATUS together).
> 3. **`/audit-code` + `/audit-tests` in parallel** — two **fresh independent agents,
>    never forks**, spawned at the audit tier. Each gets a self-contained brief: the
>    step's commit range (`<sha>^..HEAD`), the diff, the authoritative Pascal units
>    (§1.1) + `docs/wasm/USERMODEL_ABI.md` + this plan's WP section, and the binding
>    rules (the PIN, the §2.4 invariant, the §2.9 forbidden moves, the oracle is the
>    spec). Auditors are **read-only** and return findings only; **you** settle each
>    finding against the pinned oracle / the ABI doc, fix what is real, note the
>    follow-up in `STATUS.md`, re-run the gate, commit. A finding deliberately not
>    fixed is **recorded in STATUS**, never dropped; if an audit finds nothing, skip
>    its commit. For a trivial sub-step the inline audit skill is allowed.
> 4. **`STATUS.md` full review** — read end to end; sync whatever the step made stale;
>    archive dead weight to `docs/phase-records/`; `docs:` commit if anything changed
>    (gate re-run first).
> 5. **Only now stop** and report **in Russian** (code, identifiers, commit messages
>    and STATUS stay English): what landed, what the audits found and how it was
>    settled, gate status, next step.
>
> **On Pascal line references:** this plan cites Pascal units + identifiers plus line
> numbers verified 2026-07-12 against the vendored source; **re-confirm at each WP
> open** and record corrections in `STATUS.md` (the established convention).

**Per-WP model tiers (§0-tiers)** — exec tier / audit tier (audit tier applies to
**both** `/audit-code` and `/audit-tests`):

| WP | Exec tier | Audit tier | Why |
|---|---|---|---|
| WP-WM.0 (ABI freeze + probes) | **`opus-high+`** | `opus-high+` | designs the cross-boundary ABI everything else builds on; wrong choices here are expensive to unwind |
| WP-WM.1 (`dss-usermodel` crate: wasmi host) | `opus-medium+` | `opus-high+` | mechanical-with-guardrails: the typst pattern is the vendored template, the ABI doc is frozen, protocol unit tests are prescribed |
| WP-WM.2 (reference model fixtures + native DLL twin + PIN) | **`opus-high+`** | **`opus-xhigh`** | ports the IndMach012a machine math (numeric model, both targets); the fixture IS the spec-carrier for every later gate — an error here poisons all downstream goldens |
| WP-WM.3 (Generator integration + oracle gate) | **`opus-high+`** | **`opus-xhigh`** | dynamics-adjacent numerics (the WPG.13 class); first end-to-end oracle comparison through the new channel; divergence triage needs the CLAUDE.md prove-it discipline |
| WP-WM.4 (Storage + PVSystem) | `opus-medium+` | `opus-high+` | repeats the WM.3 pattern on two more elements; decks and gates pre-shaped |
| WP-WM.5 (CapUserControl) | `opus-medium+` | `opus-high+` | small control-surface interface (7 functions, no dynamics records) |
| WP-WM.6 (callback tail + SDK doc) | **`opus-high+`** | `opus-high+` | the re-entrant callbacks (`DoDSSCommand`) need a real architectural treatment, not a stub |
| WP-WM.7 (exit sweep) | `sonnet-high+` | `opus-high+` | marker sweep + coverage proof + docs, mechanical |

**Escape protocol (every WP):** when stuck — leave the tree green (revert to the last
green commit if needed), record exactly where and why in `STATUS.md`, and surface to
the user. Never park a half-wired code path behind a silent stub.

## 1. Inventory — what exists on each side

### 1.1 The Pascal DLL contract (the spec; verified 2026-07-12)

Exactly **four** units implement DLL-backed user models — the only
`LoadLibrary`/`GetProcAddress` users in the engine:

| Unit | Class | Bound by | Interface |
|---|---|---|---|
| `PCElements/GenUserModel.pas` | `TGenUserModel` | Generator `UserModel=`/`ShaftModel=` | 15 exports; `New(GenVars: Pointer; var DynaData: TDynamicsRec; var CallBacks: TDSSCallBacks): Integer` (`:27`, bound `:173-187`, instantiated `:196`) |
| `PCElements/StoreUserModel.pas` | `TStoreUserModel` | Storage `UserModel=` | 15 exports; `New` without GenVars (`:77`, bound `:214-228`) |
| `PCElements/StoreUserModel.pas` | `TStoreDynaModel` | Storage `DynaDLL=` | **13** exports (no `Save`/`Restore`; `:18-62`, bound `:336-348`) |
| `PCElements/PVSystemUserModel.pas` | `TPVsystemUserModel` | PVSystem `UserModel=` | 15 exports, same shape as TStoreUserModel (`:13-64`, bound `:160-174`) |
| `Controls/CapUserControl.pas` | `TCapUserControl` | CapControl `UserModel=` | **7** exports: `New(var CallBacks): Integer`, `Delete`, `Select`, `UpdateModel`, `Sample`, `DoPending(var Code, ProxyHdl)`, `Edit` (`:26-66`, bound `:176-182`) |

The full 15-export table (Generator/Storage/PVSystem `UserModel`):
`New, Delete, Select, Edit(s, maxlen), Init(V, I), Calc(V, I), Integrate, Save,
Restore, UpdateModel, NumVars, GetAllVars(dbls), GetVariable(i), SetVariable(i, val),
GetVarName(i, buf, maxlen)` — all `Stdcall`, strings ANSI + explicit maxlen, arrays
1-based `pComplexArray`/`pDoubleArray`.

**Shared boundary records** (packed by default — `DSS_CAPI_NO_PACKED_RECORDS` is unset
in all release cfgs):
- `TDSSCallBacks` — `Common/DSSCallBackRoutines.pas:19-67`: a **32-slot** function-
  pointer vtable (the host services the model calls back into). One global singleton
  (`:70`) populated at `initialization` (`:455-491`); every callback dereferences the
  global `DSSPrime` context, not the model's own — a documented upstream weakness
  (`:493` "this is bad"). Full slot list + our tiering: §2.3.
- `TDynamicsRec` — `Shared/Dynamics.pas:42-52`: `h, t, tstart, tstop: Double;
  IterationFlag: Integer; SolutionMode: TSolveMode ({$Z4} = int32, values 0..17);
  intHour: Integer; dblHour: Double`. Passed by reference; the DLL reads mode/step and
  may mutate.
- `TGeneratorVars` — `PCElements/generator.pas:178-214`: 22 `Double` (Theta…
  Qnominalperphase) + 3 `Integer` (NumPhases, NumConductors, Conn) + the appended-
  for-ABI-stability tail (VthevMag, VThevHarm, ThetaHarm, VTarget, Zthev: Complex,
  XRdp). The DLL **mutates** it (e.g. sets `Pshaft`); the host reads it back. Also
  exposed to callbacks as the element's `PublicDataStruct` (`generator.pas:996`).

**Call sites in the element lifecycle** (the ordering contract WM.3–WM.5 reproduce):
- Generator (`generator.pas`): props 33-36 (`:111-114`), edit-time `Set_Name`/`Edit`
  dispatch in `EndEdit` (`:752-761`); `FUpdateModel` after RecalcElementData
  (`:1266`); power-flow `DoUserModel` → `FCalc(Vterminal, Iterminal)`, result negated
  into `InjCurrent` (`:1777-1797`); Model=User path `:1900`; **GenModel=6** shaft
  `FCalc` `:1999`; dynamics `InitStateVars` → `FInit` both models (`:2389-2391`);
  `IntegrateStates` → `Select(FID)` + `FIntegrate` both (`:2474-2476`); state-var
  surface `NumVars/GetVariable/SetVariable/GetAllVars/GetVarName` (`:2552-2720`).
  Missing model with Model=User/6 → DoSimpleMsg #567 "…user-written model is not
  defined" (`:1795`).
- Storage (`Storage.pas`): props `DynaDLL/DynaData/UserModel/UserData` (`:98-101`),
  edit dispatch `:856-871`; `DoDynaModel` (`:2206-2229`) sets `StorageVars.w_grid`,
  calls `FCalc(Vterminal, @DESSCurr)` into a host-owned 6-complex buffer, sticks
  `-DESSCurr` into `ITerminal`; dynamics gating `IsDynamicModel and IsUserModel`
  (`:2135`, `:2497`); `InitStateVars` → `FInit` `:2782`; `IntegrateStates` `:2856`;
  vars `:3087-3312`.
- PVSystem (`PVsystem.pas`): props 30/31 (`:68-69`), edit `:634-638`, `DoUserModel`
  `:1828/:1885/:2022`, `InitStateVars` `:2170`, `IntegrateStates` `:2264`, vars
  `:2449-2630`.
- CapControl (`CapControl.pas`): prop 19 + UserData (`:58,84`; edit `:430-437`,
  `UpdateModel` after edit `:440`); `Sample` sets `SampleP/SampleV/SampleCurr`
  (`:1026-1035`) then `FSample` (`:1041`) — the DLL reads state via callbacks and
  schedules via the `ControlQueuePush` callback; `DoPendingAction` → `FDoPending`
  (`:730`).

**Failure handling to reproduce** (the exact warn-and-fallback contract):
load failure → DoSimpleMsg `'<Class> User Model %s Not Loaded. DSS Directory = %s'`
(codes: Generator 570 / Storage+PVSystem 1570 / CapControl 570 with its own text,
`GenUserModel.pas:166`, `StoreUserModel.pas:207/:329`, `PVSystemUserModel.pas:153`,
`CapUserControl.pas:169`), model stays absent, engine solves on the built-in model.
Missing export → `'…Does Not Have Required Function: %s'` (569/1569), library freed,
model absent. `'none'`/blank name unloads silently. `.Exists` = `FID<>0` and
auto-`Select`s.

**The canonical example DLL** (spec-carrier for WM.2):
`.inputs/electricdss-code-r3723-trunk/Version8/Source/IndMach012a/` —
`IndMach012a.dpr` (exports clause `:33-51` = exactly the 15 names), `MainUnit.pas`
(all exports; `Calc` switches on `DynaData^.SolutionMode` between
`CalcDynamic`/`CalcPflow`, `:129-169`; bundles its own `ParserDel` — it does **not**
depend on the host parser callbacks), `IndMach012Model.pas` (the induction-machine
math). It compiles against `..\PCElements\GeneratorVars.pas` (the record definitions;
dss_capi inlined them into `generator.pas` — same layout).

### 1.2 What the Rust port has today (verified 2026-07-12)

No loader exists anywhere; the property surface is split two ways:
- **Warn + fallback (live-gated):** Generator `UserModel`/`UserData`
  (`generator/mod.rs:160-161`, `accessors.rs:519-531` — set fires non-fatal "Not
  Loaded", falls back to built-in), Storage `DynaDLL`/`DynaData`
  (`storage/accessors.rs:636-643`). Backing string fields exist
  (`generator/mod.rs:303-305`, `inv_based_pce.rs:512-516`).
- **`NOT_PORTED` hard error:** Generator `ShaftModel`/`ShaftData`
  (`generator/mod.rs:164-165`), Storage `UserModel`/`UserData` (`storage/mod.rs:
  223-224`), PVSystem `UserModel`/`UserData` (`pvsystem/mod.rs:165-166`), CapControl
  `UserModel`/`UserData` (`cap_control/mod.rs:107-108`).
- **Hook sites** (where WM.3 plugs in): `generator/dynamics.rs` —
  `init_state_vars` (`:146` NOT_PORTED note), `integrate_states_impl` (`:224-225`),
  `do_dynamic_mode` model-6 abort (`:239-254`), state-var surface (`:408-434`).
- **Gating today:** `corpus_live.rs` `expect_warnings` (`:1133-1140`, `:446-452`,
  `assert_expected_warnings` `:707/:759`) + oracle `warn_and_continue`
  (`tools/oracle/oracle_server.py`, `_USER_MODEL_ERRNOS`). Five promoted decks use
  it; `Kersting4wireIndMotor` stays parked (its blocker is a malformed 4-phase
  Cmatrix, not user models).

### 1.3 What the vendored typst proves (the host-side template)

`.inputs/typst/crates/typst-library/src/foundations/plugin.rs` (613 lines, the whole
subsystem): **wasmi 1.0.9**, `default-features = false, features = ["simd"]`,
`wasm_relaxed_simd(false)` for determinism (`:271-272`) — a pure-Rust interpreter
with **zero C/FFI** in its dependency subtree (wasmi_core → libm, wasmparser →
bitflags, spin; verified). Module loading + "must export `memory`" check
(`:279-281`); host imports registered on a `Linker` (`:284-297`); per-instance
`Store` with pooled reusable instances (drop-on-failure, `:311-363`); marshalling via
two host functions writing into/reading from guest linear memory with OOB →
user-facing error (`:576-612`); traps → error messages (`:492`). Gaps we do NOT
inherit: typst has **no fuel metering and no memory cap** — we add both (§2.2).

## 2. Design decisions (pre-made — WPs execute, they don't re-litigate)

### 2.1 New leaf crate `dss-usermodel`, wasmi pinned, `forbid(unsafe_code)` intact

A new workspace crate `crates/dss-usermodel/` wraps wasmi behind a
**GenUserModel-shaped API** — exactly the `dss-sparse` pattern (pure-Rust dep behind
a Pascal-shaped surface). `dss-core` consumes it; `dss-parser`-style leaf (no
dependency on dss-core). `#![forbid(unsafe_code)]` like every crate — wasmi is an
interpreter in pure Rust, no C, no JIT (§1.3). Pin `wasmi` to an exact version in
`[workspace.dependencies]` (adopt 1.0.9, the typst-verified release, unless the WM.0
probe finds a blocker; record the final pin in `tools/wasm_usermodel/PIN.txt`).
Public surface (names final at WM.0):

```
UserModelHost      // Engine + Linker + compiled Module (one per loaded .wasm path)
UserModelInstance  // Store + instance + guest buffer handles (one per element binding)
  new/edit/init/calc/integrate/save/restore/update_model
  num_vars/get_all_vars/get_variable/set_variable/get_var_name
CapControlInstance // the 7-function control variant: new/edit/update_model/sample/do_pending
Callbacks trait    // the host-services surface dss-core implements (§2.3)
```

**Sandbox limits (beyond typst):** fuel metering ON (per-call budget, default
generous — calibrated in WM.1 so the IndMach012a fixture uses <1% of it; exhaustion =
loud engine error naming the model), linear-memory cap via wasmi's store limiter
(default 64 MiB; exceeding = loud error). Determinism: relaxed SIMD off; no WASI; no
clock/random imports — the guest is a pure function of its inputs, which is what
makes golden-gating sound.

### 2.2 The wire ABI: same records, explicit copy-in/copy-out ("record shuttle")

Native DLLs share live host memory (the DLL retains `@GenVars`/`@DynaVars`/callback
pointers from `New` and reads/writes them during every call — §1.1). WASM guest
memory is disjoint, so the ABI becomes **copy-in / call / copy-out**, per call:

- **Record images are the packed Pascal layouts, little-endian** — byte-for-byte
  `TDynamicsRec`, `TGeneratorVars`, complex = (f64 re, f64 im), arrays 1-based
  semantics preserved by copying from index 1. Keeping the packed layout (a) lets one
  reference-model source compile to both the native DLL (oracle side) and the wasm
  module (Rust side) with identical struct definitions, and (b) keeps the ABI
  familiar to anyone porting an existing Delphi/C user model. Exact offset tables
  are frozen in **`docs/wasm/USERMODEL_ABI.md`** at WM.0, cross-checked by probe
  (§WM.0-P2) — never derived from memory.
- **Guest exports** (checked at load; missing → the 569/1569 "Does Not Have Required
  Function" path): `memory`, `dss_alloc(size: i32) -> i32` (guest-owned allocator,
  the typst-protocol convention), plus the model functions with the Pascal names
  lower-snake-cased and pointer-argument signatures:
  `new(genvars: i32, dynarec: i32) -> i32` (Generator form; Storage/PVSystem form
  omits `genvars`; CapControl form takes nothing), `delete(id)`, `select(id) -> i32`,
  `edit(ptr, len)`, `init(v: i32, i: i32)`, `calc(v: i32, i: i32)`, `integrate()`,
  `save()`, `restore()`, `update_model()`, `num_vars() -> i32`,
  `get_all_vars(ptr)`, `get_variable(i: i32) -> f64`, `set_variable(i: i32, v: f64)`,
  `get_var_name(i: i32, ptr: i32, maxlen: i32)`; CapControl adds `sample()` and
  `do_pending(code: i32, proxy_hdl: i32)`. The host allocates guest buffers via
  `dss_alloc` once per instance (genvars/dynarec/V/I/name scratch), writes the
  current record images before each call, reads them back after (GenVars and
  DynaVars are read-back **always** — the DLL contract lets the model mutate both).
- **V/I buffers:** `init`/`calc` receive guest pointers to complex arrays sized
  `Yorder`; host writes `Vterminal` in, reads `Iterminal`/`DESSCurr` out, then
  applies the Pascal sign convention at the call site (Generator negates into
  `InjCurrent`, Storage negates `DESSCurr` into `ITerminal` — §1.1).
- **Protocol violations** (bad pointer/OOB, wrong export signature, trap, fuel/memory
  exhaustion) are **hard, loud engine errors** naming the model and function — never
  a silent fallback (falling back mid-run would silently change numerics; upstream
  has no trap analogue, so this is a new, documented policy in the ABI doc, not a
  `TODO(compat)`).

### 2.3 The 32-slot callback vtable → tiered host imports (module `"dss_env"`)

The `TDSSCallBacks` slots become wasm host imports. Implementation reality: wasmi
host functions see only the `Store` data, not the live `&mut Dss` — so the design is
**pre-call context + owned parser + post-call effect queue**, all inside the
instance's `CallData`:

- **Tier A — pure reads, served from the per-call context** (populated by the host at
  each call site from the same disjoint-borrow view the element itself uses):
  `GetStepSize`, `GetTimeSec`, `GetTimeHr`, `GetDynamicsStruct`→`get_dynamics_rec`
  (copies the DynaVars image), `GetActiveElement{Name,Index,BusNames,Voltages,
  Currents,Losses,Power,NumCust,NodeRef,BusRef,TerminalInfo}`, `IsActiveElementEnabled`,
  `IsBusCoordinateDefined`, `GetBusCoordinate`, `GetBuskVBase`, `GetBusDistFromMeter`,
  `GetPtrToSystemVarray`→`get_node_voltages(dest, max) -> count` (copy, not a live
  pointer), `GetPublicDataPtr`→`get_public_data(dest, max) -> bytes` (the GenVars
  image for a Generator), `GetResultStr`. "Active element" = the element that owns
  the instance (see the multi-context note below).
- **Tier B — mutations applied by the host after the call returns:**
  `ControlQueuePush(hour, sec, code, proxy_hdl) -> handle` — queued in `CallData`,
  drained into the real control queue immediately after the wasm call, order
  preserved (the `Owner: Pointer` becomes the owning element's registry handle,
  managed host-side). This is semantically identical for the CapControl use (the
  push's effects are only observable at the next control-queue pop).
- **Tier C — the parser + command block:** `LoadParser`, `NextParam`, `GetIntValue`,
  `GetDblValue`, `GetStrValue` operate on an **owned AuxParser living in `CallData`**
  (fully implementable with no engine access — the ported parser is a value type).
  `DoDSSCommand` is the one genuinely re-entrant slot (executes an arbitrary DSS
  command mid-element-call): **deferred to WP-WM.6** with a loud "not yet supported
  over WASM" engine error until then (never a silent no-op). WM.0-P3 verifies the
  reference model doesn't need it (IndMach012a bundles its own parser — §1.1).
- `MsgCallBack` → routes into the engine's DoSimpleMsg-equivalent error/warning sink
  (queued, tier B).
- **Multi-context quirk NOT reproduced:** upstream binds every callback to the global
  `DSSPrime`, not the model's own context (`DSSCallBackRoutines.pas:493` "this is
  bad"). dss-rs is single-context per `Dss`; our callbacks bind to the owning `Dss`
  — with the oracle also running single-context, no observable divergence exists;
  documented in the ABI doc, no `TODO(compat)`.

### 2.4 Activation rule + failure semantics — the invariant that keeps every gate green

Property resolution (all six properties, all four elements), replacing today's
warn-or-NOT_PORTED split with one uniform rule:

1. Value `''`/`'none'` → unload/absent (Pascal `:156-157` semantics).
2. Resolve the value as a path: literal, then relative to the deck's directory, then
   the Pascal `DSSDirectory` fallback order. **If the resolved file exists and has a
   `.wasm` extension** → load through `dss-usermodel`; export-set check; on success
   the model `Exists` and every §1.1 call site runs it.
3. **Anything else** (native DLL names, missing files — i.e. every existing corpus
   deck) → exactly today's warn-and-fallback: the same `'… Not Loaded. …'` DoSimpleMsg
   texts/codes (570/569/1570/1569 analogues), model absent, built-in fallback. The
   `expect_warnings` decks (`indmachtest`, `Kersting4wire_*`, `SimpleStorageTest*`)
   must pass **unchanged** — this is asserted by the WM.3/WM.4 exit criteria.
4. Model=User/6 with no loaded model → the existing #567-class error paths (now made
   loud where the current port drops them — `generator/dynamics.rs:239-254` fixes to
   a surfaced error, matching Pascal `generator.pas:1795`).

The four hard-`NOT_PORTED` property pairs (Generator Shaft*, Storage UserModel/Data,
PVSystem UserModel/Data, CapControl UserModel/Data) flip to the uniform rule in their
owning WPs — parse + store + warn-or-load, never a parse error (upstream never
errors on these properties).

### 2.5 The gate: three channels, strongest available per claim

**No tolerance is ever loosened to pass; divergences get the CLAUDE.md prove-it
discipline.** The channels:

1. **Pinned-oracle golden channel (primary, numeric).** The pinned dss-python
   oracle runs the *very same* engine code that loads native user-model DLLs
   (§1.1) — so the oracle **with the native twin of our reference model loaded** is
   a true numeric spec. Synthesized decks (`tools/golden/wasm_decks/`, the
   `@FIXTURES@` token pattern from the WP8.6 uuids recipe) run on the oracle with
   `UserModel=<abs path>/IndMach012a.dll` via a new manual generator
   `tools/golden/gen_wasm_usermodels.py` → committed goldens
   (`tests/golden/wasm_usermodels/`): checkpointed node voltages, iteration counts,
   element powers/currents, the model's state variables (`AllVariableValues` — the
   f64 channel, per the CLAUDE.md dSpeed lesson), and monitor channels (f32 tier).
   `crates/dss-core/tests/wasm_usermodels.rs` replays each deck with the `.wasm`
   twin and compares at the calibrated harness floors (`tests/TOLERANCE_NOTES.md`
   discipline; expect the faer-vs-KLU class floors, plus a possible guest-libm
   transcendental floor — if one appears, prove it by decomposition before pinning,
   never widen a band).
2. **Rust-vs-Rust protocol tests (hermetic, per-commit).** Unit tests in
   `dss-usermodel` drive the whole protocol against tiny inline-WAT guests (dev-dep
   `wat`, pure Rust): export checking, record round-trip byte-exactness, callback
   tiers, fuel/memory-limit errors, trap surfacing, the 569-path on a missing
   export. These run in every `cargo test` with zero external tooling.
3. **Diagnostic (non-binding):** IndMach012a-over-WASM vs the built-in `IndMach012`
   element on matched decks — same machine, historically near-identical trajectories;
   compared loosely and *reported*, never gated (the two implementations are not
   contractually identical upstream). Optionally cross-checked on the EPRI r3723
   channel (`ab_compare.py`) — never a substitute for channel 1.

**Fallback if WM.0-P1 disproves channel 1** (the pinned wheel cannot load a win64
user-model DLL for any reason): the plan does NOT silently downgrade — the oracle
channel moves to the official EPRI r3723 binary (Oddie bridge, `tools/opendss/`),
which loads user-model DLLs natively (the same `TDSSCallBacks` contract — §1.1 item
7.4), goldens generated from it with the divergence-inventory caveats of
`tools/opendss/README.md`, and **WM.3's audit tier escalates to `opus-xhigh` exec**
(the AD.3 no-pinned-oracle precedent). Either way the gate is a real engine running
the real native twin — never "the Rust engine agrees with itself".

### 2.6 Fixtures are pinned artifacts (the goldens rule, extended to binaries)

- **Guest source:** `tools/wasm_usermodel/models/indmach012a/` — a small Rust crate,
  **workspace-excluded** (its own `Cargo.toml`, `crate-type = ["cdylib"]`,
  `#![no_std]`-lean, no host deps), a faithful port of `IndMach012Model.pas` +
  `MainUnit.pas` (loop-for-loop, Pascal cited in doc comments — the standard porting
  convention applies even though it ships as a fixture). Its `UserData=` parser is a
  minimal hand-rolled key=value scanner mirroring what `MainUnit.pas` accepts (no
  dss-parser dependency — keeps the module small and the toolchain surface stable).
- **Committed artifacts:** `tests/fixtures/wasm/indmach012a.wasm` (built
  `wasm32-unknown-unknown`, release, locked flags) — committed binary, regenerated
  **manually only**, exactly like goldens. `tools/wasm_usermodel/PIN.txt` records:
  rustc version, target, profile/flags, wasmi version, and the artifact's SHA-256; a
  unit test asserts the committed file's hash matches the PIN (drift = red).
- **Native twin for golden generation (not committed as a binary):** built on demand
  by `tools/wasm_usermodel/build_native.ps1` — **plan A:** compile the vendored
  Delphi example itself (`IndMach012a.dpr`, FPC `-Mdelphi`, x86_64-win64 — zero new
  unsafe/C code anywhere; the DLL is literally upstream's own example; ppcrossx64 is
  already proven present from the line-impedance probes); **plan B** (if P2 fails):
  the same Rust reference-model core compiled as a native cdylib by a
  workspace-excluded shim under `tools/` (tooling, like the Python oracle scripts —
  outside the product's `forbid(unsafe_code)` scope; the shim is used only during
  manual golden generation, never by `cargo test`). WM.0-P2 settles A vs B; the
  choice and proof land in `STATUS.md` + the ABI doc.
- The Rust guest port's fidelity to the Pascal is itself gated by channel 1: the
  native twin (upstream's own Pascal, plan A) and the wasm guest (our port) must
  produce oracle-vs-Rust trajectories at floor — a real bug in the guest port shows
  up as a divergence, not as a self-consistent wrong answer.

### 2.7 Threading & future-plan interplay

- Instances are **per-element-owned** (`UserModelInstance` lives on the element,
  like the Pascal `UserModel: TGenUserModel` field) — no global registry, no shared
  `Store` — so MULTITHREADING M3's disjoint-`&mut` `par_iter_mut` pattern holds
  without redesign (wasmi stores are `Send`).
- DE_PASCALIZE Stage F (`oracle-parity` lane): WASM user models are **additive**
  behavior with no upstream-observable counterpart on the pinned decks; the
  `wasm_usermodels` goldens gate both lanes identically. No lane split needed.
- UPGRADE interplay: none of the four Pascal units changed across
  0.14.5→0.15.x→r4133 in a way that affects this contract (the delta inventories do
  not touch them); re-verify with a one-line diff check at WM.0.

### 2.8 Documentation deliverables

- **`docs/wasm/USERMODEL_ABI.md`** (WM.0, frozen; updated only by a recorded
  decision): the export list per interface, the packed record offset tables, the
  callback import table with tiers, the failure/trap policy, the activation rule,
  a "porting your Delphi/C user model to WASM" section (WM.6 finishes it with a
  worked example).
- `tools/wasm_usermodel/README.md` (WM.2): how to rebuild the fixture + native twin,
  the PIN discipline, how `gen_wasm_usermodels.py` is run.

### 2.9 Forbidden moves (binding for every WP)

1. **Never touch the existing warn-and-fallback contract** — the five
   `expect_warnings` corpus decks and their manifests stay byte-identical; any WP
   whose diff touches `expect_warnings` machinery must call it out to the auditors
   explicitly.
2. No `unsafe`, no C, no JIT runtime (wasmtime/cranelift are off the table), no
   process-level dynamic loading of anything — wasmi only.
3. No new property names, no new commands — the six existing properties are the
   whole user surface.
4. `.wasm` fixtures and goldens regenerate **manually only** with the pinned
   toolchain (PIN.txt); a WP never regenerates them to make itself pass.
5. No silent stubs: an unimplemented callback or interface function is a loud,
   attributed engine error.
6. No tolerance loosening; divergences get the prove-it/decomposition discipline
   (CLAUDE.md) before any pin.
7. Guest fixtures never read the host filesystem/clock/env — the sandbox imports
   only `dss_env`.

## 3. Work packages

> Each WP ends gate-green per the §0 ritual. Line references re-verified at WP open.

---

### WP-WM.0 — ABI freeze + probes [15%]

Deliverable: `docs/wasm/USERMODEL_ABI.md` frozen + probe results recorded in
`STATUS.md`. No engine code changes (docs + probe scripts only; probe scratch under
the session scratchpad, committed evidence under `docs/wasm/probes/`).

1. **P1 — oracle loads a native user-model DLL.** Build a minimal 15-export stub DLL
   (FPC, or the plan-B shim) whose `New` returns 1 and whose `Calc` writes a
   recognizable current; point `Generator.UserModel=` at it via the pinned
   dss-python (probe_val.py pattern); assert no "Not Loaded" and the current shows
   up. Settles §2.5 channel 1 vs the r3723 fallback.
2. **P2 — record layouts.** FPC probe printing `SizeOf`/field offsets of
   `TDynamicsRec`/`TGeneratorVars`/`TDSSCallBacks` compiled with the release cfg
   defines (packed expected); freeze the offset tables in the ABI doc from the probe
   output, not from reading. Also settles plan A vs plan B for the native twin
   (§2.6): can FPC `-Mdelphi` build `IndMach012a.dpr` win64 as-is?
3. **P3 — callback usage census.** Read `MainUnit.pas`/`IndMach012Model.pas` and
   list every `CallBacks.*` slot the example actually invokes; confirm
   `DoDSSCommand` is unused (else WM.6 moves earlier). Record the census in the ABI
   doc's tier table.
4. **P4 — toolchain pins.** `rustup target add wasm32-unknown-unknown` availability,
   rustc version, wasmi crate version compile-check under
   `default-features=false` + our feature set; write the initial
   `tools/wasm_usermodel/PIN.txt`.
5. Freeze the ABI doc (§2.2/§2.3 tables made concrete), run the ritual (audits review
   the ABI doc against the Pascal units — this WP's "diff" is the document).

---

### WP-WM.1 — the `dss-usermodel` crate [15%]

The wasmi host per §2.1–§2.3, template = the vendored typst `plugin.rs` (cite it in
doc comments the way Pascal is cited elsewhere).

1. Crate skeleton, workspace deps (`wasmi` pinned, dev-dep `wat`), `forbid(unsafe_code)`.
2. `UserModelHost` (engine config: relaxed-SIMD off, fuel on, store limits;
   module load + export-set validation returning the exact missing-export name for
   the 569-path), `UserModelInstance`/`CapControlInstance` (guest `dss_alloc`
   buffers, record shuttle write/read helpers over the ABI-doc offsets),
   `Callbacks` trait + `CallData` (context snapshot, owned AuxParser, effect queue).
3. **Protocol unit tests (channel 2)** — inline-WAT guests covering: happy-path
   15-function round trip; record byte-exact round-trip (write image → guest
   increments a field → read back); each callback tier (incl. parser callbacks
   against the owned AuxParser and `ControlQueuePush` queuing); missing export →
   the right error name; trap, fuel-exhaustion, OOB, memory-cap → loud typed errors;
   hash-vs-PIN test scaffold (activated when WM.2 commits the fixture).
4. Ritual (audits at `opus-high+`: template-fidelity vs typst pattern, limit
   enforcement really wired, no silent error swallowing).

---

### WP-WM.2 — reference model fixtures: IndMach012a to wasm + native twin + PIN [20%]

1. Port `IndMach012Model.pas` + `MainUnit.pas` (r3723 Version8) loop-for-loop into
   `tools/wasm_usermodel/models/indmach012a/` (§2.6): the machine math, the
   `Edit` key=value handling, the `Calc` SolutionMode dispatch
   (CalcDynamic/CalcPflow), the 15 exports over the ABI. Pascal citations in doc
   comments; `TODO(compat)` discipline applies to any deliberate quirk reproduction
   inside the fixture too.
2. Build + commit `tests/fixtures/wasm/indmach012a.wasm`; fill `PIN.txt` (+ hash
   test goes green).
3. Native twin per the WM.0-P2 decision (plan A: FPC build of the vendored `.dpr`);
   `build_native.ps1` + README. The twin is built on demand for golden generation,
   not committed.
4. **Fixture self-gate (pre-integration):** a `dss-usermodel`-level test drives the
   committed `.wasm` directly through the crate API with hand-fed V/records and
   pins a handful of computed currents/state-vars against values probed from the
   native twin driven by a tiny FPC/Python harness (recorded under
   `docs/wasm/probes/`). This catches guest-port math errors before the engine
   integration exists.
5. Ritual (audit tier `opus-xhigh`: the auditors re-derive the machine equations
   from `IndMach012Model.pas` and check the port line-by-line — this fixture
   underwrites every later gate).

---

### WP-WM.3 — Generator integration + the oracle golden gate [25%]

The flagship: Generator `UserModel`/`UserData`/`ShaftModel`/`ShaftData` over WASM,
all §1.1 Generator call sites.

1. Element wiring: replace the warn-only accessors with the §2.4 activation rule
   (Shaft* flip from NOT_PORTED to the uniform rule); `EndEdit` dispatch order per
   `generator.pas:752-761`; `update_model` after RecalcElementData (`:1266`).
2. Call sites: `DoUserModel` (`:1777-1797` — FCalc + negate into InjCurrent; the
   Model=User power-flow dispatch `:2092`), GenModel=6 shaft path (`:1999`),
   `init_state_vars` (`:2389-2391` — both models), `integrate_states`
   (`:2474-2476` — select+integrate both), and the state-variable surface
   (`:2552-2720`: NumVars totals, GetAllVars into `States`, name lookup) feeding
   the existing `get_all_variables`/monitor plumbing. Fix the dropped model-6 error
   (`dynamics.rs:239-254`) to a surfaced #567-analogue.
3. Decks (`tools/golden/wasm_decks/`, oracle-validated before use — the GAPS §2
   deck-validation protocol): `wasm_gen_pflow.dss` (Model=User snapshot),
   `wasm_gen_dyn.dss` (GenModel=6 + ShaftModel, dynamics run, monitors modes 1/3),
   `wasm_gen_vars.dss` (state-var surface + `? Generator.g1.<var>` probes),
   `wasm_gen_edit.dss` (UserData= mid-script re-edit). Each has the `@FIXTURES@`
   twin form (`.dll` for the oracle, `.wasm` for Rust).
4. `tools/golden/gen_wasm_usermodels.py` + goldens + `wasm_usermodels.rs` (channel
   1, §2.5); the diagnostic IndMach012-builtin comparison (channel 3) as a reported,
   non-gating test.
5. **Invariant check:** the untouched `expect_warnings` decks re-verified green with
   zero manifest edits (an explicit test-run line in the WP record).
6. Ritual (audit tier `opus-xhigh`; auditors specifically hunt: sign conventions at
   the InjCurrent boundary, read-back ordering of GenVars mutations, state-var
   off-by-one vs the 1-based Pascal arrays, silent fallback on trap).

---

### WP-WM.4 — Storage (`DynaDLL` + `UserModel`) and PVSystem (`UserModel`) [12%]

1. Storage: flip `UserModel`/`UserData` from NOT_PORTED; wire `TStoreUserModel`
   (15-fn) and `TStoreDynaModel` (13-fn, no save/restore) instances;
   `DoDynaModel` (`Storage.pas:2206-2229`: w_grid, DESSCurr buffer, negate),
   dynamics gating (`:2135/:2497`), init/integrate (`:2782/:2856`), vars
   (`:3087-3312`).
2. PVSystem: flip props; wire per `PVsystem.pas` sites (`:634-638`, `:1828/:1885/
   :2022`, `:2170`, `:2264`, `:2449-2630`).
3. A storage dyna reference fixture: extend the indmach fixture crate family with a
   minimal `storagedyn` model (there is no vendored upstream example for Storage —
   author a small physically-plausible model, build BOTH targets, and gate it
   through channel 1 exactly like WM.3; its native twin uses the same shim path).
   Decks: `wasm_storage_dyn.dss`, `wasm_pv_pflow.dss` + twins; goldens; the
   `SimpleStorageTest*` `expect_warnings` decks re-verified untouched.
4. Ritual.

---

### WP-WM.5 — CapControl user control [8%]

1. Flip CapControl `UserModel`/`UserData` from NOT_PORTED; wire `CapControlInstance`
   (7-fn): edit dispatch + post-edit `UpdateModel` (`CapControl.pas:430-440`),
   `Sample` (SampleP/V/Curr context then `sample()`, `:1026-1041`),
   `DoPendingAction` → `do_pending` (`:730`). The control path exercises tier-B
   `ControlQueuePush` + tier-A element reads for real.
2. Fixture: a minimal `capuserctl` guest (author both targets — no upstream example
   exists) implementing a deadband voltage control; deck `wasm_capcontrol.dss` +
   twin; golden = the control-action sequence (event log) + final states vs the
   oracle-with-DLL.
3. Ritual.

---

### WP-WM.6 — callback tail + SDK documentation [10%]

1. `DoDSSCommand` + `GetResultStr` re-entrancy: implement via the effect-queue with
   an immediate-drain design — the host runs the queued command through the
   executive **after** the wasm call returns but **before** the next wasm call on
   the same instance, capturing `GlobalResult` into the instance context so a
   subsequent `GetResultStr` sees it (the one observable ordering difference from
   Pascal's synchronous call is documented in the ABI doc; if a probe shows a real
   model pattern that breaks under it, STOP and surface — do not invent a
   re-entrant `&mut` scheme ad hoc).
2. Sweep the callback table: every remaining slot implemented or carrying a loud
   attributed error + an ABI-doc row stating why (nothing silent); extend protocol
   tests to cover each.
3. Finish `USERMODEL_ABI.md` §"porting your model": a worked start-to-finish example
   (the indmach fixture as the narrative), the limits/trap policy, the PIN workflow.
4. Ritual.

---

### WP-WM.7 — exit sweep [5%]

1. `rg "NOT_PORTED"` over the six properties' sites → zero hits;
   `rg "TODO\(WM\)"` → zero; every callback slot accounted for in the ABI doc.
2. Full gate + the goldens + the hash-vs-PIN test; re-verify the five
   `expect_warnings` decks byte-identical manifests (git diff proof in the record).
3. `DSS_LIVE_CLASSIFY=1` pass — confirm no corpus deck changes classification
   (expected: none — no vendored deck ships a `.wasm`); `COVERAGE.md` note.
4. Docs: STATUS §WASM-UM record finalized; `PORTING_PLAN.md` "never (DLLs)" lines
   annotated with a pointer here (historical plan texts themselves stay unedited);
   `PLAN_SEQUENCE.md` stage marked complete.
5. Merge per the standing convention — **only on explicit user request**.

## 4. Exit criteria (the whole plan)

1. All six properties on all four elements follow the §2.4 uniform rule; the WASM
   path is live end-to-end for Generator (user + shaft), Storage (user + dyna),
   PVSystem, CapControl.
2. Channel-1 goldens green at calibrated floors for every reference fixture; the
   hash-vs-PIN test green; channel-2 protocol suite green; channel-3 diagnostic
   reported in STATUS.
3. The five `expect_warnings` corpus decks and their manifests untouched and green.
4. `docs/wasm/USERMODEL_ABI.md` complete (worked example included);
   `tools/wasm_usermodel/` self-contained (README + PIN + build scripts + model
   sources).
5. Standard three-command gate green; no `unsafe`, no C, wasmi only.
