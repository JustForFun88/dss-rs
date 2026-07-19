# USERMODEL_ABI — the frozen WASM user-model ABI (WASM_USERMODELS plan)

**Status: FROZEN at WP-WM.0 (2026-07-18).** Changes only by a recorded decision
noted in this header and in `STATUS.md` §WASM-UM.

Recorded decisions:

- **2026-07-19 (a)** — §4 row 17 `get_node_voltages` ground-slot indexing
  documented (settles audit finding WM-AUD-1; the WM.1 crate contract is
  unchanged, the doc had omitted the decision).
- **2026-07-19 (b) — ABI re-freeze to r4133 (native side).** The frozen
  **native** `TGeneratorVars` layout moves from the 0.14.5/r3723 image to the
  **r4133** image (§2.2): r4088+/NCIM inserts `deltaQNom: array of Double` at
  offset 176, growing the record 244→**252 B** with the whole tail shifted +8
  (probe `docs/wasm/probes/p8_offsets_r4133.txt`). *Rationale:* the WM.0 freeze
  chose the 0.14.5 image because the only oracle channel then was pinned
  dss-python 0.14.5 (§2.5-1); the project is now post-acceptance and gates on
  the in-house **r4133** bridge (`crates/dss-epri`), which is the channel the
  native twin loads into (proven — `p8_twin_r4133_bridge.txt`), so the native
  record must match r4133. *The **wasm** marshaled image is UNCHANGED* — 244 B —
  because `deltaQNom` is engine-only (NCIM Q update), a managed reference with no
  wasm-linear-memory meaning, and never crosses the boundary as data; the +8
  shift affects only the native side (`TDynamicsRec` 52 B and `TDSSCallBacks`
  256 B are byte-identical r3723→r4133 — probe-confirmed). The 0.14.5/r3723
  244-B table is retained as Appendix A with its caveat inverted.

This document is the single authority for the wire contract between the dss-rs
engine (host, `crates/dss-usermodel` from WP-WM.1) and WASM user models
(guests). The *behavioral* spec is the Pascal (units cited per section); the
*numeric* gate is the pinned oracle running the native twin (plan §2.5
channel 1 — **empirically confirmed** by probe P1, see §7). Every offset table
below is transcribed from FPC probe output
(`docs/wasm/probes/p2_offsets_dss_capi.txt`, cross-checked byte-identical
against the r3723 headers in `p2_offsets_r3723.txt`), never from reading the
source — per the plan's WM.0 rule.

Conventions: all record images are **little-endian, packed** (the release cfgs
never set `DSS_CAPI_NO_PACKED_RECORDS`; probe-verified). `Complex` = two f64
(re at +0, im at +8), 16 bytes. Arrays keep the Pascal **1-based** semantics:
the host copies from/to index 1; a `pComplexArray`/`pDoubleArray` guest buffer
holds element k at byte offset `(k-1)*elemsize`. Strings crossing the boundary
are ANSI bytes + explicit `maxlen`, NUL-terminated on write (Pascal
`StrLCopy` semantics).

## 1. Interfaces and guest exports

Pascal spec: `GenUserModel.pas` (`TGenUserModel`, 15 exports, bound `:173-187`),
`StoreUserModel.pas` (`TStoreUserModel` 15, `:214-228`; `TStoreDynaModel` 13 —
no `Save`/`Restore`, `:336-348`), `PVSystemUserModel.pas` (15, `:160-174`),
`CapUserControl.pas` (7, `:176-182`). The wasm module must export, with these
exact names (lower-snake-case of the Pascal export names; missing export ⇒ the
569/1569 "Does Not Have Required Function: `<name>`" path, library rejected,
model absent):

**Common infrastructure (all interfaces):**

| Export | Signature (wasm) | Notes |
|---|---|---|
| `memory` | linear memory | typst-pattern requirement (plugin.rs:279-281) |
| `dss_alloc` | `(size: i32) -> i32` | guest-owned allocator; host allocates its per-instance buffers (record images, V/I, name scratch) once per instance |

**15-function interface** (Generator `UserModel=`/`ShaftModel=`, Storage
`UserModel=`, PVSystem `UserModel=`):

| Export | Signature (wasm) | Pascal shape |
|---|---|---|
| `new` | Generator: `(genvars: i32, dynarec: i32) -> i32`; Storage/PVSystem: `(dynarec: i32) -> i32` | `New(GenVars: Pointer; var DynaData; var CallBacks): Integer` — the CallBacks pointer has no wasm counterpart; callbacks are host imports (§4) |
| `delete` | `(id: i32)` | `Delete(var x)` |
| `select` | `(id: i32) -> i32` | `Select(var x): Integer` |
| `edit` | `(ptr: i32, len: i32)` | `Edit(s: pAnsiChar; Maxlen)` — UserData/DynaData strings |
| `init` | `(v: i32, i: i32)` | `Init(V, I: pComplexArray)` |
| `calc` | `(v: i32, i: i32)` | `Calc(V, I: pComplexArray)` |
| `integrate` | `()` | `Integrate` |
| `save` | `()` | `Save` (15-fn only) |
| `restore` | `()` | `Restore` (15-fn only) |
| `update_model` | `()` | `UpdateModel` |
| `num_vars` | `() -> i32` | `NumVars: Integer` |
| `get_all_vars` | `(ptr: i32)` | `GetAllVars(Vars: pDoubleArray)` — guest writes NumVars f64 starting at ptr (1-based semantics: var k at `ptr+(k-1)*8`) |
| `get_variable` | `(i: i32) -> f64` | `GetVariable(var i): Double` |
| `set_variable` | `(i: i32, v: f64)` | `SetVariable(var i; var value)` |
| `get_var_name` | `(i: i32, ptr: i32, maxlen: i32)` | `GetVarName(var VarNum; VarName; maxlen)` — NUL-terminated ANSI, ≤ maxlen |

**13-function interface** (Storage `DynaDLL=`, `TStoreDynaModel`): the table
above **minus `save` and `restore`**; `new` takes `(dynarec: i32) -> i32`.

**7-function interface** (CapControl `UserModel=`, `TCapUserControl`):
`new() -> i32`, `delete(id)`, `select(id) -> i32`, `edit(ptr, len)`,
`update_model()`, `sample()`, `do_pending(code: i32, proxy_hdl: i32)`.
(Pascal `DoPending(var Code, ProxyHdl)` — value semantics suffice: upstream
passes constants from the pending control action, `CapControl.pas:730`.)

Instance ids: `new` returns a nonzero i32 id (0 = creation failure ⇒ treated
as load failure, model absent). `Exists` = id ≠ 0, auto-`select` before use
(Pascal `Get_Exists`).

## 2. The record shuttle (per-call copy-in / copy-out)

Native DLLs retain live pointers from `New` and share host memory; wasm guest
memory is disjoint, so the host **writes the record images into guest buffers
before every call and reads them back after** (GenVars and DynaVars read-back
is unconditional — the Pascal contract lets the model mutate both; e.g.
IndMach012a's dynamics set `GenData^.Pshaft`/`Speed`/`dSpeed`). Buffers are
allocated once per instance via `dss_alloc`; `new` receives the genvars/dynarec
buffer pointers, and the same buffers are refreshed in place for every
subsequent call.

### 2.1 `TDynamicsRec` — 52 bytes (probe: `SizeOf(TDynamicsRec) = 52`)

Pascal `Shared/Dynamics.pas:42-52` (dss_capi 0.14.5); byte-identical in r3723.

| Offset | Size | Field | Type |
|---|---|---|---|
| 0 | 8 | `h` | f64 — dynamics step, s |
| 8 | 8 | `t` | f64 — s from top of hour |
| 16 | 8 | `tstart` | f64 |
| 24 | 8 | `tstop` | f64 |
| 32 | 4 | `IterationFlag` | i32 — 0 = new step, 1 = same step (predictor/corrector) |
| 36 | 4 | `SolutionMode` | i32 — `TSolveMode` `{$Z4}`, values 0..17 (probe: `SizeOf(TSolveMode) = 4`); DYNAMICMODE = 14 |
| 40 | 4 | `intHour` | i32 |
| 44 | 8 | `dblHour` | f64 |

### 2.2 `TGeneratorVars`

Two images, one contract (re-freeze 2026-07-19, header decision (b)):

- the **frozen native image is r4133 — 252 bytes** (the twin + the r4133 engine
  bridge share it);
- the **wasm marshaled image is 244 bytes, UNCHANGED** (the WM.0 compact subset
  — `deltaQNom` never crosses).

#### 2.2a Native (frozen) — r4133, 252 bytes (probe: `SizeOf(TGeneratorVars) = 252`)

Pascal `PCElements/GeneratorVars.pas` (OpenDSS r4088+/NCIM era; r4133 vendored
at `.inputs/electricdss-code-r4133-trunk`). Transcribed from FPC probe output
`docs/wasm/probes/p8_offsets_r4133.txt` (cross-checked +8-shift vs
`p2_offsets_r3723.txt`). This is the Generator's `PublicDataStruct` the native
twin's `New` receives from the r4133 engine and the `get_public_data` import
would serve on the native side.

| Offset | Field | | Offset | Field |
|---|---|---|---|---|
| 0 | `Theta` f64 | | 128 | `dTheta` f64 |
| 8 | `Pshaft` f64 | | 136 | `dSpeed` f64 |
| 16 | `Speed` f64 | | 144 | `ThetaHistory` f64 |
| 24 | `w0` f64 | | 152 | `SpeedHistory` f64 |
| 32 | `Hmass` f64 | | 160 | `Pnominalperphase` f64 |
| 40 | `Mmass` f64 | | 168 | `Qnominalperphase` f64 |
| 48 | `D` f64 | | **176** | **`deltaQNom` — `array of Double`, 8-byte managed ref (NCIM-only)** |
| 56 | `Dpu` f64 | | 184 | `NumPhases` i32 |
| 64 | `kVArating` f64 | | 188 | `NumConductors` i32 |
| 72 | `kVGeneratorBase` f64 | | 192 | `Conn` i32 (0 wye, 1 delta) |
| 80 | `Xd` f64 | | 196 | `VthevMag` f64 |
| 88 | `Xdp` f64 | | 204 | `VThevHarm` f64 |
| 96 | `Xdpp` f64 | | 212 | `ThetaHarm` f64 |
| 104 | `puXd` f64 | | 220 | `VTarget` f64 |
| 112 | `puXdp` f64 | | 228 | `Zthev` Complex (re 228, im 236) |
| 120 | `puXdpp` f64 | | 244 | `XRdp` f64 |

`deltaQNom` at 176 is a Delphi-managed dynamic-array reference used **only** by
the engine's NCIM Q-update; the reference model (IndMach012a) never reads or
writes it. Its 8 bytes shift every field from `NumPhases` onward by +8 vs the
0.14.5/r3723 layout (Appendix A). The tail is still packed and deliberately
unaligned (`VthevMag` at 196 after the three i32s) — a Rust-side native mirror
must be assembled field-by-field at these offsets, never via `#[repr(C)]`.

#### 2.2b Wasm marshaled image — 244 bytes, UNCHANGED

The host writes the guest a **compact 244-byte image** holding only the fields
the model reads — the crossing fields packed with **no `deltaQNom` hole** — so
`NumPhases` sits at 176, `VthevMag` at 188, `XRdp` at 236: field-for-field the
historical 0.14.5/r3723 layout of **Appendix A**. This is deliberate and
load-bearing: `deltaQNom` is engine-only and a managed reference has no meaning
in the guest's disjoint linear memory, so it never crosses as data; excluding it
keeps the wasm image (and its codecs) **identical to WM.0**. Consequently
`crates/dss-usermodel::records::GeneratorVars` (§3 host codec) and the fixture
guest decoder stay 244 bytes with the Appendix-A offsets — the re-freeze changed
**zero** wasm-side bytes, proven by the fixture self-gate staying bit-exact green
against the r4133-rebuilt twin (`p8_twin_r4133_bridge.txt`,
`p8_indmach012a_math_diff.txt`).

> **Caveat (inverted from WM.0):** never drive a DLL compiled against
> r4088+/r4133/0.15.x headers (the current native twin) through 0.14.5/r3723
> binaries, and never drive a ≤r3723/0.14.5 DLL through r4088/r4133 binaries —
> the two native images differ by the `deltaQNom` slot (252 vs 244 B). The wasm
> boundary is immune either way: it carries the 244-byte crossing subset only.

### 2.3 V/I buffers

`init`/`calc` receive guest pointers to Complex arrays of `Yorder` entries
(element k at `(k-1)*16`; re/im f64). Host writes `Vterminal` before the call
and reads the currents after; sign conventions stay at the Pascal call sites
(Generator `DoUserModel` negates into `InjCurrent`, `generator.pas:1777-1797`;
GenModel=6 dynamics takes `Iterminal` as returned, `:1900`; Storage
`DoDynaModel` negates `DESSCurr`, `Storage.pas:2206-2229`). Probe P1.d verified
the native contract end-to-end: converged Model=6 terminal currents equal the
model's `Calc` output exactly.

### 2.4 Storage / PVSystem / CapControl records

`TStorageVars` / `TPVSystemVars` / `TCapControlVars` images are frozen the same
way at their owning WPs (WM.4/WM.5) with a probe extension — same
extraction+probe machinery (`tools/fpc/usermodel_abi/`), same packed rule. The
15/13/7 function shapes and the `TDynamicsRec`/callback contracts above do not
depend on them.

## 3. Call ordering (the lifecycle contract WM.3–WM.5 reproduce)

Verbatim from the Pascal call sites (§1.1 of the plan; re-verified 2026-07-18):

- **Load** (property write, `Set_Name`): resolve path (§5) → validate export
  set → `new(...)` with fresh record images → id stored. `Edit`/`edit` fires
  only when a `UserData=`/`DynaData=` string is present or later assigned
  (`EndEdit` order: `generator.pas:752-761` — UserModel before UserData,
  ShaftModel before ShaftData).
- **Power flow** (Generator Model=User i.e. `Model=6`, `:2092`): each
  iteration `calc(V, I)` via `DoUserModel` (`:1777`); currents negated into
  `InjCurrent`.
- **Dynamics**: `InitStateVars` → `init(V, I)` on both user and shaft models
  (`:2389-2391`); per step `IntegrateStates` → `select(id)` + `integrate()`
  on both (`:2474-2476`); `DoDynamicMode` GenModel=6 → user `calc` then shaft
  `calc` (`:1900`, `:1999`).
- **Edit/update**: property re-edit → `edit`; `update_model()` after
  `RecalcElementData` (`:1266`).
- **State variables** (`:2552-2720`): totals = built-ins + `num_vars()`;
  `get_all_vars` fills the tail of `States`; `get_variable`/`set_variable`/
  `get_var_name` take 1-based user indices (host subtracts the built-in
  count). Probe P1.b/P1.c verified the native surface (stub vars appear,
  UserData reaches `edit`).
- **CapControl** (`CapControl.pas`): `edit` + `update_model` after edit
  (`:430-440`); `Sample` populates SampleP/V/Curr context then `sample()`
  (`:1026-1041`); `DoPendingAction` → `do_pending(code, proxy_hdl)` (`:730`).

## 4. Host imports — module `"dss_env"` (the 32-slot callback vtable, tiered)

Pascal spec: `Common/DSSCallBackRoutines.pas:19-67` (offsets probed: 32 slots ×
8 bytes = 256; the record itself never crosses the wasm boundary — slots become
host functions importable from module `dss_env`). Design per plan §2.3:
pre-call context snapshot + owned AuxParser + post-call effect queue in the
instance's `CallData`. "Active element" = the element owning the instance.
The upstream global-`DSSPrime` binding (`:493` "this is bad") is NOT
reproduced: dss-rs is single-context; no observable divergence vs the
single-context oracle (documented decision, no `TODO(compat)`).

P3 census (probe `p3_callback_census.txt`): the canonical example invokes
exactly one slot — `MsgCallBack` (`IndMach012Model.pas:474`); it bundles its
own parser (`ModelParser: TParser`), so the parser callbacks are exercised by
protocol tests (WM.1), not by the reference fixture. `DoDSSCommand` unused ⇒
its WM.6 deferral stands.

| # | Slot (Pascal) | wasm import (dss_env) | Tier | Used by IndMach012a |
|---|---|---|---|---|
| 1 | `MsgCallBack` | `msg_callback(ptr, len)` | B (queued → DoSimpleMsg sink) | **yes** |
| 2 | `GetIntValue` | `get_int_value(ptr)` | C (owned AuxParser) | no |
| 3 | `GetDblValue` | `get_dbl_value(ptr)` | C | no |
| 4 | `GetStrValue` | `get_str_value(ptr, maxlen)` | C | no |
| 5 | `LoadParser` | `load_parser(ptr, len)` | C | no |
| 6 | `NextParam` | `next_param(name_ptr, maxlen) -> i32` | C | no |
| 7 | `DoDSSCommand` | `do_dss_command(ptr, len)` | C — **WM.6**; until then a loud "not yet supported over WASM" error | no (census-verified) |
| 8 | `GetActiveElementBusNames` | `get_active_element_bus_names(p1, l1, p2, l2)` | A | no |
| 9 | `GetActiveElementVoltages` | `get_active_element_voltages(num_ptr, v_ptr)` | A | no |
| 10 | `GetActiveElementCurrents` | `get_active_element_currents(num_ptr, i_ptr)` | A | no |
| 11 | `GetActiveElementLosses` | `get_active_element_losses(total_ptr, load_ptr, noload_ptr)` | A | no |
| 12 | `GetActiveElementPower` | `get_active_element_power(terminal, power_ptr)` | A | no |
| 13 | `GetActiveElementNumCust` | `get_active_element_num_cust(num_ptr, total_ptr)` | A | no |
| 14 | `GetActiveElementNodeRef` | `get_active_element_node_ref(maxsize, ptr)` | A | no |
| 15 | `GetActiveElementBusRef` | `get_active_element_bus_ref(terminal) -> i32` | A | no |
| 16 | `GetActiveElementTerminalInfo` | `get_active_element_terminal_info(nt_ptr, nc_ptr, np_ptr)` | A | no |
| 17 | `GetPtrToSystemVarray` | `get_node_voltages(dest, max) -> i32` (copy, not a live pointer; **ground slot excluded** — indexing note below) | A | no |
| 18 | `GetActiveElementIndex` | `get_active_element_index() -> i32` | A | no |
| 19 | `IsActiveElementEnabled` | `is_active_element_enabled() -> i32` | A | no |
| 20 | `IsBusCoordinateDefined` | `is_bus_coordinate_defined(busref) -> i32` | A | no |
| 21 | `GetBusCoordinate` | `get_bus_coordinate(busref, x_ptr, y_ptr)` | A | no |
| 22 | `GetBuskVBase` | `get_bus_kv_base(busref) -> f64` | A | no |
| 23 | `GetBusDistFromMeter` | `get_bus_dist_from_meter(busref) -> f64` | A | no |
| 24 | `GetDynamicsStruct` | `get_dynamics_rec(dest)` (copies the 52-byte image) | A | no |
| 25 | `GetStepSize` | `get_step_size() -> f64` | A | no |
| 26 | `GetTimeSec` | `get_time_sec() -> f64` | A | no |
| 27 | `GetTimeHr` | `get_time_hr() -> f64` | A | no |
| 28 | `GetPublicDataPtr` | `get_public_data(dest, max) -> i32` (bytes copied; GenVars image for a Generator) | A | no |
| 29 | `GetActiveElementName` | `get_active_element_name(ptr, maxlen) -> i32` | A | no |
| 30 | `GetActiveElementPtr` | **not importable** — a raw host pointer has no wasm meaning; calling the import raises the loud unsupported error (documented policy; upstream models that need element internals use `get_public_data`) | — | no |
| 31 | `ControlQueuePush` | `control_queue_push(hour: i32, sec: f64, code: i32, proxy_hdl: i32) -> i32` | B (queued; drained in order after the call returns; `Owner` = the owning element's registry handle, host-side) | no |
| 32 | `GetResultStr` | `get_result_str(ptr, maxlen)` | C (paired with `do_dss_command`, WM.6) | no |

Tier meanings — **A**: pure read served from the pre-call context snapshot;
**B**: mutation recorded in the effect queue, applied by the host after the
wasm call returns (order preserved; semantically identical where effects are
only observable at the next queue pop); **C**: owned-AuxParser block +
the WM.6 re-entrancy pair. Boolean returns are i32 0/1. Every import exists at
link time (so modules validate); unimplemented-by-design slots raise the loud
attributed error of §6 when *called* — never a silent no-op (plan §2.9-5).

**Row-17 indexing (recorded decision, 2026-07-19 — WM-AUD-1 settlement):** the
native callback hands the model the raw `Solution.NodeV` pointer, whose
offset-0 element IS the ground node (`TNodeVarray = array[0..1000] of Complex`,
`Solution.pas:88`; `NodeV: pNodeVArray … allows NodeV[0]=0`, `:198`;
`DSSCallBackRoutines.pas:307-311`), together with `iNumNodes = NumNodes` (the
non-ground node count) — so a native model's natural 1-based access `V^[k]`
reads node k. The wasm copy serves `NodeV[1..NumNodes]` **without** the ground
slot: node k lives at `dest + (k-1)*16`, and the i32 return is the count
copied, `min(NumNodes, max)` — keeping the native `iNumNodes` meaning.
Rationale: a copy has no reason to spend a slot on the always-zero ground
entry, and the returned count stays 1:1 with the native out-parameter.
**Porting consequence (one-slot layout shift vs the native pointer):** a model
that indexed the native array 1-based must shift by one slot — `V^[k]` becomes
the Complex at `dest + (k-1)*16` (i.e. what was at pointer offset `k*16`
natively). P3 census: no existing model uses this slot. The dss-core
`Callbacks` implementation (lands at WM.3) must serve the slice
ground-excluded, per the trait contract at
`crates/dss-usermodel/src/callbacks.rs::node_voltages`.

## 5. Activation rule (plan §2.4 — the invariant that keeps every gate green)

1. `''`/`'none'` (case-insensitive) → unload/absent, silently
   (`GenUserModel.pas:156-157`).
2. Resolve as a path: literal → relative to the deck directory → the
   `DSSDirectory` fallback (Pascal `LoadLibrary(Value)` then
   `LoadLibrary(DSSDirectory + Value)`, `:159-163`). If the resolved file
   **exists and ends in `.wasm`** → load via `dss-usermodel`; export-set check
   (§1); on success the model Exists and every §3 call site runs it.
3. Anything else (native DLL names, missing files — every existing corpus
   deck) → exactly today's warn-and-fallback: `'<Class> User Model %s Not
   Loaded. DSS Directory = %s'` (Generator 570, Storage/PVSystem 1570,
   CapControl 570 with its own text), model absent, built-in model solves.
   The five `expect_warnings` decks stay byte-identical (plan §2.9-1).
4. Model=User/6 with no loaded model → the #567-class errors
   (`generator.pas:1795`, dynamics `:1904` + abort), surfaced loudly.

## 6. Failure & sandbox policy (new, documented policy — not `TODO(compat)`)

Native-load failure classes keep their Pascal behavior (§5.3, warn-and-fall-
back; missing export 569/1569 frees the module, model absent). **Wasm-only**
failure classes have no upstream analogue and are hard, loud engine errors
naming the model path and function — never a silent fallback (falling back
mid-run would silently change numerics):

- trap (unreachable, div-by-zero, OOB access inside the guest),
- protocol violation (bad pointer/OOB write through a shuttle buffer, wrong
  export signature discovered at call time, `dss_alloc` returning 0/OOB),
- fuel exhaustion (per-call budget; default calibrated in WM.1 so the
  reference fixture uses <1% — exhaustion = error naming the model),
- memory-cap breach (store limiter, default 64 MiB).

Determinism: relaxed SIMD disabled (typst plugin.rs:271-272), no WASI, no
clock/random/filesystem imports — module `dss_env` is the entire import
surface (plan §2.9-7); the guest is a pure function of its inputs.

## 7. Probe evidence & decisions (WP-WM.0, 2026-07-18)

| Probe | Result | Evidence |
|---|---|---|
| P1 oracle-loads-DLL | **PASS all 5 asserts** — pinned dss-python 0.15.7/0.14.5 loads a native 15-export stub, vars surface + `edit` + Model=6 solve + V/I marshalling verified (currents bit-exact) ⇒ **§2.5 channel 1 CONFIRMED**, r3723 fallback not engaged | `p1_oracle_load.txt` |
| P2 layouts | packed; TDynamicsRec 52 B / TGeneratorVars 244 B / TDSSCallBacks 256 B; dss_capi 0.14.5 ≡ r3723 byte-identical | `p2_offsets_dss_capi.txt`, `p2_offsets_r3723.txt` |
| P2 twin decision | **Plan A** — FPC 3.2.2 `-Mdelphi` builds the vendored `IndMach012a.dpr` **as-is** (search paths only, zero source edits, `{$R *.RES}` linked); the DLL loads + solves under the pinned oracle (all 14 machine vars live, converged) | `p2_indmach_fpc_build.txt` |
| P3 callback census | `MsgCallBack` only; `DoDSSCommand` unused ⇒ WM.6 deferral stands; own parser bundled | `p3_callback_census.txt` |
| P4 toolchain | stable rustc 1.96.0, `wasm32-unknown-unknown` added; `wasmi =1.0.9` `default-features=false`+`simd` compiles pure-Rust; fuel/limiter APIs verified (`instantiate_and_start` is the 1.x spelling) | `p4_toolchain.txt`, `tools/wasm_usermodel/PIN.txt` |
| §2.7 upgrade check | loader units: contract unchanged 0.14.5→0.15.x and r3723→r4133; `TGeneratorVars` gains `deltaQNom` in 0.15.x/r4088+ (see §2.2 caution) | `p5_upgrade_diff.txt` |

Re-freeze to r4133 (WM.3 pre-round, 2026-07-19 — header decision (b)):

| Probe | Result | Evidence |
|---|---|---|
| P8 r4133 layouts | FPC `-Mdelphi` probe of the r4133 headers: `TDynamicsRec` 52 B and `TDSSCallBacks` 256 B **unchanged**; `TGeneratorVars` **252 B** with `deltaQNom` at 176 and the tail +8 (§2.2a) | `p8_offsets_r4133.txt` |
| P8 model-math diff | IndMach012a example dir (`IndMach012Model`/`MainUnit`/`ParserDel`/`.dpr`) **byte-identical** r3723→r4133 (sha256s); the sole delta is `GeneratorVars.pas` `deltaQNom` ⇒ model math UNCHANGED | `p8_indmach012a_math_diff.txt` |
| P8 twin-in-r4133-engine | native twin rebuilt from r4133 (252-B layout) **loads + runs** in the r4133 engine via the `dss-epri`/`epri-worker` bridge — model=6 power-flow (converged, 14 machine vars: Slip/Is1/Ir1/StatorLoss/HPshaft) + 10 dynamics steps (Monitor mode=3 series). Bit-exact DLL-boundary pin (`twin_probe.py`→`twin_expected.rs`) re-derived from the SAME twin, **byte-identical** to the r3723 image | `p8_twin_r4133_bridge.txt` |

Probe sources: `tools/fpc/usermodel_abi/` (extraction script + probes + stub
DLL + oracle driver + build script; `abi_probe_r4133.pas` + the P8 step of
`build_probes.ps1`). The native-twin channel is
`tools/wasm_usermodel/build_native.ps1` (r4133) + `twin_probe.py`.

## 8. Porting your Delphi/C user model to WASM (skeleton — WM.6 completes)

The 15 exports keep their meaning; what changes is transport: your model reads
its inputs from the guest-memory record images (§2) refreshed before each
call, and host services arrive as `dss_env` imports (§4) instead of a callback
struct. A worked start-to-finish example (the IndMach012a fixture,
`tools/wasm_usermodel/models/indmach012a/`) lands at WM.2; the narrative
walkthrough, limits/trap policy recap and PIN workflow land at WM.6.

## Appendix A — historical 0.14.5 / r3723 `TGeneratorVars` (244 bytes)

The WM.0 frozen native layout, superseded on the native side by §2.2a (re-freeze
2026-07-19). Retained because it is **exactly the wasm marshaled image of §2.2b**
(the crossing subset with no `deltaQNom` hole), and because it documents what a
≤r3723/0.14.5-compiled DLL expects. Probe `p2_offsets_dss_capi.txt` (dss_capi
0.14.5) ≡ `p2_offsets_r3723.txt`, byte-identical.

| Offset | Field | | Offset | Field |
|---|---|---|---|---|
| 0 | `Theta` f64 | | 128 | `dTheta` f64 |
| 8 | `Pshaft` f64 | | 136 | `dSpeed` f64 |
| 16 | `Speed` f64 | | 144 | `ThetaHistory` f64 |
| 24 | `w0` f64 | | 152 | `SpeedHistory` f64 |
| 32 | `Hmass` f64 | | 160 | `Pnominalperphase` f64 |
| 40 | `Mmass` f64 | | 168 | `Qnominalperphase` f64 |
| 48 | `D` f64 | | 176 | `NumPhases` i32 |
| 56 | `Dpu` f64 | | 180 | `NumConductors` i32 |
| 64 | `kVArating` f64 | | 184 | `Conn` i32 (0 wye, 1 delta) |
| 72 | `kVGeneratorBase` f64 | | 188 | `VthevMag` f64 |
| 80 | `Xd` f64 | | 196 | `VThevHarm` f64 |
| 88 | `Xdp` f64 | | 204 | `ThetaHarm` f64 |
| 96 | `Xdpp` f64 | | 212 | `VTarget` f64 |
| 104 | `puXd` f64 | | 220 | `Zthev` Complex (re 220, im 228) |
| 112 | `puXdp` f64 | | 236 | `XRdp` f64 |
| 120 | `puXdpp` f64 | | | |

Never drive this-layout DLL through r4088/r4133 binaries, or an r4133-layout DLL
(the current native twin) through 0.14.5/r3723 binaries (§2.2b caveat).
