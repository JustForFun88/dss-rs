# USERMODEL_ABI — the frozen WASM user-model ABI (WASM_USERMODELS plan)

**Status: FROZEN at WP-WM.0 (2026-07-18).** Changes only by a recorded decision
noted in this header and in `STATUS.md` §WASM-UM.

Recorded decisions:

- **2026-07-20 (e) — WP-WM.6 callback tail: `DoDSSCommand`/`GetResultStr`
  implemented as an opt-in deferred-drain mechanism; the callback table is now
  complete (no wire-ABI change).** The two re-entrant slots (7/32) leave the
  "loud unsupported until WM.6" state. `do_dss_command(ptr, len)` **queues** the
  command in the instance context; `get_result_str(ptr, maxlen)` serves the last
  captured `GlobalResult`. A host that can re-enter the executive between calls
  opts in (`UserModelInstance::enable_dss_commands`), then after each guest call
  drains the queue (`drain_dss_commands`), runs each command through the
  executive, and feeds the result back (`set_result_str`) — so the guest's
  **next** call sees it. **The one observable ordering difference from Pascal's
  synchronous `DoDSSCommand`→`GetResultStr`** (documented at §4): a
  `GetResultStr` in the *same* guest call as its `DoDSSCommand` sees the
  *previous* result, not the just-run one. **The dss-rs element call sites do
  NOT opt in**: they hold a disjoint borrow and cannot reach `Dss::command`, and
  inventing a re-entrant `&mut` scheme is forbidden (plan §WP-WM.6); with the
  mechanism disabled both callbacks raise the loud `Fault::Unsupported` (never a
  silent drop). No reference model needs it — P3 census, `IndMach012a` bundles
  its own parser. The mechanism is exercised by the channel-2 protocol tests
  (which act as the re-entrant host). Additive; the 15/13/7 shapes and every
  record/callback contract are unchanged. See §4 rows 7/32 + STATUS §WASM-UM
  WP-WM.6.
- **2026-07-20 (d) — WP-WM.5 CapControl round 2 (build): a native `TCapUserControl`
  twin CANNOT be the gate oracle → the r4133 BUILT-IN VOLTAGE control is
  (fixture/gate-design, NOT a wire-ABI change).** Round 1 proved `get_public_data`
  un-gatable for CapControl; round 2 proves the *same* for `control_queue_push`
  from a native twin, so **no native CapUserControl twin can drive the r4133
  control queue at all.** Source-definitive: the 7-fn `New(var CallBacks)` hands
  the model ONLY the callback vtable (`CapUserControl.pas:36`) — no owning-element
  pointer; neither `SampleControlDevices` (r4133 `Solution.pas:3611-3621`) nor
  `CapControl.Sample` (USERCONTROL arm `:1054-1069`) sets `ActiveCktElement` to the
  CapControl; and `ControlQueue.Push` stores its `Owner` as the `ControlElement`
  whose `DoPendingAction` runs on pop (`ControlQueue.pas:145,193`). A twin can only
  pass `Owner := GetActiveElementPtr()` (the wrong element) — empirically this
  **hangs the r4133 engine** (a corrupt control-queue pop, observed 2026-07-20).
  Resolution (within fixture/gate authority — the wire ABI is UNCHANGED: the 7-fn
  shape, `get_node_voltages`, `get_dynamics_rec`, `control_queue_push` are all as
  frozen): the reference `capuserctl` fixture's deadband logic is made **identical
  to the built-in VOLTAGE control** (`vlow`/`vhigh` = `OnSetting`/`OffSetting`), so
  the r4133 **built-in** VOLTAGE control is a sound cross-engine oracle for the
  Rust USERCONTROL wiring (the gate matches its switch sequence + final cap state +
  node voltages at the faer-vs-KLU floor, worst rel 2.5e-15). See §2.5 + STATUS
  §WASM-UM WP-WM.5 round 2.
- **2026-07-19 (c) — WP-WM.5 CapControl: `TCapControlVars` image frozen (r4133
  layout) + the `get_public_data` asymmetry finding → the reference fixture uses
  `get_node_voltages` (§2.5).** The CapControl `get_public_data` image is frozen
  from the FPC probe of the **r4133** engine record
  (`docs/wasm/probes/p9_offsets_capcontrolvars_r4133.txt` — 184 B,
  `EControlAction` 1 B, `Voverride` Boolean). Corrections to an earlier draft of
  this decision (both settled from source, both engines probed):
  (i) **BOTH engines set `PublicDataStruct := @ControlVars`** ("So User-written
  models can access" — 0.14.5 `CapControl.pas:535`, r4133 `:518`); the earlier
  "0.14.5 never sets it / interface inert on 0.14.5" claim was wrong. The real
  0.14.5-vs-r4133 difference is the **record layout**: (ii) r4133 `Voverride`
  `Boolean` (1 B) vs 0.14.5 `LongBool` (4 B); (iii) r4133 `EControlAction` no
  `{$Z4}` (Delphi-default 1 B) vs 0.14.5's int32 — shifting the whole tail. The
  r4133 layout is frozen (WM.3 re-freeze precedent — decision (b)). **Key
  finding:** `get_public_data` (`GetPublicDataPtr`) is **NOT oracle-gatable** for
  CapControl — it returns the *global* `ActiveCktElement`, which neither engine
  sets to the CapControl during control sampling
  (`SampleControlDevices`/`CapControl.Sample` on both), so a native twin cannot
  reproduce the dss-rs owning-element-bound `get_public_data` payload. The
  reference `capuserctl` fixture therefore reads its control voltage through the
  **symmetric** `get_node_voltages` (`GetPtrToSystemVarray` → converged
  `Solution.NodeV`) channel instead — that IS reproducible on the r4133 native
  twin, keeping the WM.5 gate a real cross-engine oracle check. All additive to
  the WASM path (no corpus deck loads a `.wasm` CapControl). See §2.5 + STATUS
  §WASM-UM WP-WM.5.
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

### 2.4 Storage / PVSystem records

`TStorageVars` / `TPVSystemVars` images would be frozen the same way at their
owning WP with a probe extension — but WM.4 did **not** need them: the authored
Storage/PVSystem fixtures read only `V` + `TDynamicsRec` (no `get_public_data`),
so no StorageVars/PVSystemVars image crosses the boundary (STATUS §WM.4
deviation (a)). `TCapControlVars` is documented as the dss-rs `get_public_data`
payload at §2.5, but the reference CapControl fixture likewise avoids it — it
reads voltage via the symmetric `get_node_voltages` because `get_public_data` is
not oracle-gatable for CapControl (the asymmetry finding at §2.5).

### 2.5 `TCapControlVars` — the CapControl public-data image + the `get_public_data` asymmetry (WP-WM.5)

Unlike the 15/13-function interfaces (which receive `V`/`TDynamicsRec` as explicit
`calc`/`new` pointer arguments), the 7-function CapControl `sample()` takes **no
arguments**. **Both** engines set `PublicDataStruct := @ControlVars` ("So
User-written models can access" — 0.14.5 `CapControl.pas:535`, r4133 `:518`), so
`get_public_data` is the dss-rs channel that carries a CapControl's `Sample`
context (`SampleP`/`SampleV`/`SampleCurr`) + bank state to a wasm model. This
image is that record.

**Layout is the r4133 engine image — 184 bytes packed** (probe
`docs/wasm/probes/p9_offsets_capcontrolvars_r4133.txt`; the gate's oracle is the
r4133 bridge, so r4133 is authoritative — header decision (c)). `EControlAction`
is **1 byte** (r4133 has no `{$Z4}`) and `Voverride` is **Boolean (1 byte)**;
both differ from dss_capi 0.14.5 (int32 / `LongBool`), which shifts every field
from `Voverride` onward — the reason the whole record must be probe-derived, not
hand-derived. The Rust host codec is
`crates/dss-usermodel::records::CapControlVars` (the fields a model interacts
with; the rest of the 184 B is other public data the model never reads and stays
zero on the wasm side).

**The `get_public_data` asymmetry — why the reference fixture does NOT use it
(proven, both engines).** On the **dss-rs** side `get_public_data` is bound to the
*owning* element (plan §2.3 / §4), so it reliably returns the owning CapControl's
`@ControlVars`. On the **native** side `GetPublicDataPtr` returns
`ActiveCircuit.ActiveCktElement.PublicDataStruct` — the *global* active element —
and **neither engine sets `ActiveCktElement` to the CapControl during control
sampling**: `SampleControlDevices` iterates `DSSControls` without touching it
(0.14.5 `Solution.pas`, r4133 `Solution.pas:3606`), and `CapControl.Sample` sets
only `ControlledElement.ActiveTerminalIdx` (r4133 `:909`), while
`MonitoredElement.Power[]`/`.GetCurrents` set only `ActiveTerminalIdx`
(`CktElement.pas`). So a **native twin's `GetPublicDataPtr` does NOT return
`@ControlVars`** and cannot reproduce the dss-rs payload — the same holds for
every `GetActiveElement*` tier-A read during `Sample`. (This is almost certainly
why no vendored CapUserControl example exists.) Consequence: a wasm CapControl
model that reads `get_public_data` works on dss-rs but is **not oracle-gatable**.

The reference **`capuserctl` fixture therefore reads its control voltage through
`get_node_voltages`** (`GetPtrToSystemVarray` → converged `Solution.NodeV`, row
17), a **symmetric** channel identical on both engines after convergence, with the
monitored node index supplied via `UserData`. This keeps the WM.5 gate a real
cross-engine oracle comparison. The `CapControlVars` codec above stays as the
documented dss-rs `get_public_data` contract for models that opt into it (with the
un-gatable caveat), mirroring WM.4's `TStorageVars`/`TPVSystemVars` decision (§2.4:
the authored fixture chooses the channel it can gate).

| Offset | Field | Type | Role |
|---|---|---|---|
| 0 | `FCTPhase` | i32 | — |
| 4 | `FPTPhase` | i32 | — |
| 8…80 | `ON_Value`…`LastOpenTime` | 10× f64 | — |
| 88 | `Voverride` | Boolean (1 B) | — |
| 89 | `VoverrideEvent` | Boolean (1 B) | — |
| 90 | `VoverrideBusSpecified` | Boolean (1 B) | — |
| 91 | `VOverrideBusIndex` | i32 | — |
| 95 | `Vmax` | f64 (unaligned) | — |
| 103 | `Vmin` | f64 | — |
| **111** | **`FPendingChange`** | EControlAction (1 B) | model's desired action (native write-back) |
| **112** | **`ShouldSwitch`** | Boolean (1 B) | action pending (native write-back) |
| 113 | `Armed` | Boolean (1 B) | — |
| **114** | **`PresentState`** | EControlAction (1 B) | bank open/closed (model reads) |
| 115 | `InitialState` | EControlAction (1 B) | — |
| **116** | **`SampleP`** | Complex (re 116 / im 124) | monitored terminal power kW+jkvar (`:1057`) |
| **132** | **`SampleV`** | f64 | control voltage, PT-ratio+phase applied (`:1060`) |
| **140** | **`SampleCurr`** | f64 | control current (`:1063`) |
| **148** | **`NumCapSteps`** | i32 | (`:1067` in dss_capi 0.14.5) |
| **152** | **`AvailableSteps`** | i32 | (`:1068`) |
| **156** | **`LastStepInService`** | i32 | (`:1069`) |
| 160 | `VOverrideBusName` | String (8 B ref) | — |
| 168 | `CapacitorName` | String (8 B ref) | — |
| 176 | `ControlActionHandle` | i32 | — |
| 180 | `CondOffset` | i32 | — |

**Decision signalling — `control_queue_push`, a plain queue push.** A model
signals its switch decision through `control_queue_push(hour, sec, code,
proxy_hdl)` — a plain push onto the control queue (Effect tier B), forwarded to
`ControlQueue.Push` (`DSSCallBackRoutines.pas:444`). On the **dss-rs** side the
host supplies the owning CapControl as the queue action's owner, so the pop
routes `DoPendingAction` back to it correctly.

> **Round-2 finding (2026-07-20 — header decision (d)): a NATIVE `TCapUserControl`
> twin cannot use this channel as a gate oracle.** `ControlQueue.Push` stores its
> `Owner` as the `ControlElement` whose `DoPendingAction` runs on pop
> (`ControlQueue.pas:145,193`), but the 7-fn `New(var CallBacks)` gives a native
> model NO owning-element pointer (`CapUserControl.pas:36`) and neither
> `SampleControlDevices` (r4133 `Solution.pas:3611-3621`) nor `CapControl.Sample`
> (USERCONTROL arm `:1054-1069`) sets `ActiveCktElement` to the CapControl — so a
> twin can only pass `Owner := GetActiveElementPtr()` (the wrong element), which
> **hangs/corrupts the r4133 engine** (observed). This extends round 1's
> `get_public_data` asymmetry: no native CapUserControl twin can drive the r4133
> control queue at all. The reference fixture therefore gates against the r4133
> **built-in VOLTAGE** control (its deadband made identical to `OnSetting`/
> `OffSetting`), not a twin. The dss-rs `control_queue_push` wiring itself is
> correct and gated (it owns the queue owner host-side); only a *native twin* lacks
> the owner. See STATUS §WASM-UM WP-WM.5 round 2.

Note this deliberately **diverges from the native USERCONTROL timing path.** In
Pascal, `CapControl.Sample`'s `USERCONTROL` arm calls `UserModel.Sample` "Sets the
switching flags" (`CapControl.pas:1069`) — the model writes `ShouldSwitch`/
`PendingChange` into `@ControlVars` — and then the **shared `Sample` tail**
(`CapControl.pas:1180-1205`) computes `TimeDelay` from `DeadTime`/`ONDelay`/
`OFFDelay`, does `ControlQueue.Push`, arms (`Armed := TRUE`), and disarms on
`PendingChange = CTRL_NONE`. The WM.5 gate twin cannot use that path: it needs a
public-data write-back the frozen ABI does not have, AND — per the asymmetry above
— a native twin cannot even read/write `@ControlVars` reliably. So the twin (and
the wasm guest) instead pushes the queue **directly**, meaning the engine's
`DeadTime`/`ONDelay`/`OFFDelay` arm/disarm tail is bypassed and **the model owns
the timing**. When the queue pops, `DoPendingAction(Code, ProxyHdl)` runs: the
host sets `PendingChange = Code`, calls the guest `do_pending`, and the shared
switch block acts on `PendingChange`.

**Successor caveat (deferred to the element-wiring build round):** because the
direct push skips the shared-tail timing, the `capuserctl` deck must keep
`ONDelay`/`OFFDelay`/`DeadTime` at values (e.g. 0) where the engine tail would add
no delay, so the direct-push schedule and any Pascal-faithful shared-tail schedule
coincide and the r4133 twin and Rust engine cannot diverge on action *time*. The
dss-rs `USERCONTROL` wiring must choose the push channel **deliberately** (direct
push, matching the gate twin) rather than fall into it — do not route the guest's
decision through the shared arm/disarm tail without also reconciling the twin.
This is a WP-WM.5 element-wiring decision recorded in STATUS §WASM-UM WP-WM.5, not
an ABI change. The 15/13/7 function shapes and the `TDynamicsRec`/callback contracts
above do not depend on the `get_public_data` image.

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
its WM.6 treatment is the opt-in deferred-drain mechanism below (no model needs
the executive re-entry). **The table is complete as of WP-WM.6**: all 32 slots
are either implemented (tier A/B/C) or a permanent loud attributed error (slot
30), and each is covered by a channel-2 protocol test (WM.1 tier-A/B/C sweeps +
`missing_export…`/`unsupported_imports…`/`wm6_deferred_…`).

| # | Slot (Pascal) | wasm import (dss_env) | Tier | Used by IndMach012a |
|---|---|---|---|---|
| 1 | `MsgCallBack` | `msg_callback(ptr, len)` | B (queued → DoSimpleMsg sink) | **yes** |
| 2 | `GetIntValue` | `get_int_value(ptr)` | C (owned AuxParser) | no |
| 3 | `GetDblValue` | `get_dbl_value(ptr)` | C | no |
| 4 | `GetStrValue` | `get_str_value(ptr, maxlen)` | C | no |
| 5 | `LoadParser` | `load_parser(ptr, len)` | C | no |
| 6 | `NextParam` | `next_param(name_ptr, maxlen) -> i32` | C | no |
| 7 | `DoDSSCommand` | `do_dss_command(ptr, len)` | C — **WM.6**: enabled → queues the command for the host to run through the executive after the call (deferred drain, ordering note below); disabled (default) → loud `Unsupported` | no (census-verified) |
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
| 32 | `GetResultStr` | `get_result_str(ptr, maxlen)` | C — **WM.6**: enabled → StrLCopy of the last captured `GlobalResult` (persists across calls); disabled (default) → loud `Unsupported`. Paired with `do_dss_command` | no |

Tier meanings — **A**: pure read served from the pre-call context snapshot;
**B**: mutation recorded in the effect queue, applied by the host after the
wasm call returns (order preserved; semantically identical where effects are
only observable at the next queue pop); **C**: owned-AuxParser block +
the WM.6 deferred-command pair (below). Boolean returns are i32 0/1. Every
import exists at link time (so modules validate); the permanently-unsupported
slot 30 and the not-opted-in WM.6 pair raise the loud attributed error of §6
when *called* — never a silent no-op (plan §2.9-5).

**Tier reconciliation vs plan §2.3.** Plan §2.3 provisionally listed
`GetResultStr` among the tier-A "pure reads served from the pre-call context
snapshot". This frozen ABI doc reclassifies it to **tier C**, paired with
`DoDSSCommand` (row 7), because `GlobalResult` is *produced by* a `DoDSSCommand`
run, not a snapshot the host captures before the call — the two share one
opt-in mechanism and one ordering note. This is the recorded WM.6 decision (e);
the ABI doc is the authoritative artifact where the plan and it differ.

**Slots 7 + 32 — `DoDSSCommand`/`GetResultStr`, the deferred-drain re-entrancy
pair (WP-WM.6, header decision (e)).** Upstream `DoDSSCommandCallBack`
(`DSSCallBackRoutines.pas:150-154`) runs `DSSExecutive.ParseCommand(S)`
**synchronously mid-model-call**, mutating engine state, and
`GetResultStrCallBack` (`:449-452`) then `StrLCopy`s `GlobalResult`. A wasmi
host function sees only the `Store` data, never the live `&mut Dss`, so the
synchronous form is impossible; the design is an **immediate deferred drain**:

- `do_dss_command(ptr, len)` copies the command string and **queues** it in the
  instance's `CallData` (order preserved). It does **not** run mid-call.
- After the guest call returns, a host holding the executive
  (`UserModelInstance::drain_dss_commands`) runs each queued command through
  `Dss::command` (= `ParseCommand`) and captures the resulting `GlobalResult`
  back into the instance (`set_result_str`) — **before the next guest call on
  the same instance**. To stay Pascal-faithful the host must clear
  `SolutionAbort` **before** each `ParseCommand`: `DoDSSCommandCallBack`
  (`DSSCallBackRoutines.pas:152-153`) does `DSSPrime.SolutionAbort := FALSE;`
  then `ParseCommand` — so a queued command runs even if the in-progress solve
  had raised the abort flag. The ordering difference below is thus **not** the
  only semantic a real executive host reproduces; the abort-flag reset is the
  second (both are the future host's responsibility, `Dss`-side, since the
  drain API has no `&mut Dss`).
- `get_result_str(ptr, maxlen)` serves that captured `GlobalResult` (persists
  across calls; `StrLCopy` semantics).

**The one observable ordering difference from Pascal** (accepted, documented,
not a `TODO(compat)` — it is a new WASM-only mechanism with no upstream
byte-golden): a `GetResultStr` issued in the **same** guest call as its
`DoDSSCommand` sees the *previous* result (the command has only been queued, not
yet run), whereas Pascal's synchronous call would see the just-run result. A
`GetResultStr` on the **next** call sees it. No reference model exercises this
pair (P3 census).

**Opt-in / not enabled by default.** Running a queued command needs a host that
holds `&mut Dss`. The dss-rs element call sites (`DoUserModel`, CapControl
`Sample`, …) hold a *disjoint* borrow of the circuit — they cannot reach
`Dss::command`, and inventing a re-entrant `&mut` scheme is forbidden (plan
§WP-WM.6). So the elements leave the mechanism **disabled**, and both callbacks
raise the loud `Unsupported` error (§6) — never a silent drop. The mechanism
(`enable_dss_commands` + `drain_dss_commands`/`set_result_str`) is complete and
gated by the channel-2 protocol tests, which act as the re-entrant executive
host; a future integration point that holds the executive (or a hardened
re-entrancy design) can opt in without any wire-ABI change. Because no
reference model needs the executive re-entry, this leaves no gated feeder
behavior unported.

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
| P3 callback census | `MsgCallBack` only; `DoDSSCommand` unused ⇒ its WM.6 treatment is the opt-in deferred-drain pair (header decision (e)); own parser bundled | `p3_callback_census.txt` |
| P4 toolchain | stable rustc 1.96.0, `wasm32-unknown-unknown` added; `wasmi =1.0.9` `default-features=false`+`simd` compiles pure-Rust; fuel/limiter APIs verified (`instantiate_and_start` is the 1.x spelling) | `p4_toolchain.txt`, `tools/wasm_usermodel/PIN.txt` |
| §2.7 upgrade check | loader units: contract unchanged 0.14.5→0.15.x and r3723→r4133; `TGeneratorVars` gains `deltaQNom` in 0.15.x/r4088+ (see §2.2 caution) | `p5_upgrade_diff.txt` |

Re-freeze to r4133 (WM.3 pre-round, 2026-07-19 — header decision (b)):

| Probe | Result | Evidence |
|---|---|---|
| P8 r4133 layouts | FPC `-Mdelphi` probe of the r4133 headers: `TDynamicsRec` 52 B and `TDSSCallBacks` 256 B **unchanged**; `TGeneratorVars` **252 B** with `deltaQNom` at 176 and the tail +8 (§2.2a) | `p8_offsets_r4133.txt` |
| P8 model-math diff | IndMach012a example dir (`IndMach012Model`/`MainUnit`/`ParserDel`/`.dpr`) **byte-identical** r3723→r4133 (sha256s); the sole delta is `GeneratorVars.pas` `deltaQNom` ⇒ model math UNCHANGED | `p8_indmach012a_math_diff.txt` |
| P8 twin-in-r4133-engine | native twin rebuilt from r4133 (252-B layout) **loads + runs** in the r4133 engine via the `dss-epri`/`epri-worker` bridge — model=6 power-flow (converged, 14 machine vars: Slip/Is1/Ir1/StatorLoss/HPshaft) + 10 dynamics steps (Monitor mode=3 series). Bit-exact DLL-boundary pin (`twin_probe.py`→`twin_expected.rs`) re-derived from the SAME twin, **byte-identical** to the r3723 image | `p8_twin_r4133_bridge.txt` |

CapControl public-data image (WP-WM.5, 2026-07-19 — header decision (c)):

| Probe | Result | Evidence |
|---|---|---|
| P9 `TCapControlVars` (r4133) | FPC `-Mdelphi -dUSER_DLL` compile of the REAL vendored `Version8/Source/Controls/CapControlVars.pas`: **184 B**; `EControlAction` **1 B** (no `{$Z4}`), `Voverride` **Boolean 1 B** — both differ from 0.14.5 (int32 / `LongBool`); `SampleP`@116, `SampleV`@132, `FPendingChange`@111, `ShouldSwitch`@112, `PresentState`@114 (§2.5) | `p9_offsets_capcontrolvars_r4133.txt` |

Probe sources: `tools/fpc/usermodel_abi/` (extraction script + probes + stub
DLL + oracle driver + build script; `abi_probe_r4133.pas` + the P8 step,
`abi_probe_capcontrolvars.pas` + the P9 step, of `build_probes.ps1`). The
native-twin channel is `tools/wasm_usermodel/build_native.ps1` (r4133) +
`twin_probe.py`.

## 8. Porting your Delphi/C user model to WASM (worked example)

The exports keep their meaning; what changes is **transport**. A native DLL
retained live host pointers from `New` and read/wrote them during every call; a
WASM guest has its own disjoint linear memory, so the host **copies the record
images into guest buffers before each call and reads them back after** (§2), and
the `TDSSCallBacks` struct becomes a set of `dss_env` host **imports** (§4). The
worked example below is the committed reference fixture
`tools/wasm_usermodel/models/indmach012a/` — a loop-for-loop port of the
vendored `IndMach012a` Delphi DLL — which compiles to
`tests/fixtures/wasm/indmach012a.wasm` and is gated bit-exact against the native
twin (`models/indmach012a/tests/twin_parity.rs`) and the r4133 oracle
(`crates/dss-core/tests/wasm_usermodels.rs`). Every step here is a real part of
that crate; read its module docs for the full detail.

**Step 1 — pick the interface and export the required set (§1).** Generator
`UserModel=`/`ShaftModel=`, Storage `UserModel=`, PVSystem `UserModel=` are the
**15-function** form; Storage `DynaDLL=` is the **13-function** form (no
`save`/`restore`); CapControl `UserModel=` is the **7-function** form. Export
`memory`, `dss_alloc`, and the interface functions with the *exact* wasm names
and signatures of §1 — a missing/mis-typed export is the 569/1569
"Does Not Have Required Function" load rejection. IndMach012a is a Generator, so
it exports the 15-function set with `new(genvars: i32, dynarec: i32) -> i32`
(`src/wasm_exports.rs`).

**Step 2 — provide the guest allocator (§1, common infra).** The host calls
`dss_alloc(size) -> ptr` once per instance to carve out each shuttle buffer
(genvars/dynarec/V/I/name scratch); return a pointer into your linear memory or
0 to signal failure (a protocol violation, §6). A bump/registry allocator is
enough — the fixture keeps a `BTreeMap<addr, Box<[u8]>>` so a stray pointer
traps instead of corrupting memory:

```rust
#[no_mangle]
pub extern "C" fn dss_alloc(size: i32) -> i32 {
    if size <= 0 { return 0; }
    with_guest(|g| {
        let buf = vec![0u8; size as usize].into_boxed_slice();
        let addr = buf.as_ptr() as usize as u32;
        g.allocs.insert(addr, buf);       // registry: pointer -> owned bytes
        addr as i32
    })
}
```

**Step 3 — decode/encode the record images (§2).** The pointers the host passes
to `new`/`calc`/… address the packed, little-endian record images at the frozen
offsets (§2.1 `TDynamicsRec`, §2.2b the 244-byte wasm `TGeneratorVars`; complex
= re then im f64; arrays 1-based, element k at `(k-1)*16`). Decode from the
buffer, compute, and — for `GenVars`/`DynaVars` — write back **unconditionally**
(the contract lets the model mutate both, e.g. dynamics sets `Pshaft`/`Speed`).
Never use `#[repr(C)]`: the tail is deliberately unaligned — assemble
field-by-field at the documented offsets.

**Step 4 — implement the lifecycle (§3).** `new` captures the buffer pointers
and returns a nonzero id (0 ⇒ model absent); `edit(ptr, len)` receives the
NUL-terminated `UserData=`/`DynaData=` string; `calc(v, i)` reads `Vterminal`,
dispatches on the refreshed `TDynamicsRec.SolutionMode` (power-flow vs dynamics),
and writes the terminal currents (the host applies the Pascal sign convention at
its call site); `init`/`integrate` run the dynamics state; `num_vars` /
`get_all_vars` / `get_variable` / `set_variable` / `get_var_name` are the
monitoring surface (1-based user indices). The fixture's `calc`:

```rust
#[no_mangle]
pub extern "C" fn calc(v: i32, i: i32) {
    with_guest(|g| {
        let Some((gv_ptr, dyn_ptr)) = active_ptrs(g) else { return };
        let varr = read_v3(g, v);                 // decode 3 complex from guest mem
        let dyna = read_dynarec(g, dyn_ptr);      // refreshed TDynamicsRec image
        let mut gv = read_genvars(g, gv_ptr);
        let mut iarr = [CZERO; 3];
        if mainunit::calc(&mut g.main, &varr, &mut iarr, &mut gv, &dyna) {
            write_i3(g, i, &iarr);                // currents back to the host
        }
        write_genvars(g, gv_ptr, &gv);            // read-back is unconditional
    });
}
```

**Step 5 — host services are `dss_env` imports (§4).** Declare only what you use
(`#[link(wasm_import_module = "dss_env")]`). IndMach012a needs exactly one —
`msg_callback` (P3 census) — and bundles its own parser, so it touches none of
the tier-C parser slots. Tier-A reads (voltages, dynamics time, public data, …)
return the owning element's state; tier-B `control_queue_push`/`msg_callback`
are queued and applied after your call returns; the deferred pair
`do_dss_command`/`get_result_str` (slots 7/32) works only when the host opts in
(§4) — a command you issue runs **after** the call, so a `get_result_str` in the
same call sees the previous result. `get_active_element_ptr` (slot 30) is never
importable. Calling an unsupported/not-opted-in slot is a loud attributed error
(§6), never a silent no-op.

**Step 6 — build, PIN, gate (§2.6, the goldens rule for binaries).** Build for
`wasm32-unknown-unknown`, release, with the locked flags in the crate
`Cargo.toml` (`opt-level=3, lto=true, codegen-units=1, panic=abort`), commit the
`.wasm`, and record its SHA-256 in `tools/wasm_usermodel/PIN.txt`. Regenerate
**manually only** — never to make a test pass. For the reference fixture:

```powershell
pwsh tools/wasm_usermodel/build_wasm.ps1     # rebuilds the .wasm, prints its SHA-256
# then update the sha256(...) line in tools/wasm_usermodel/PIN.txt
```

A unit test (`crates/dss-usermodel/tests/fixture_pin.rs`) asserts the committed
file's hash matches the PIN, so drift is red in every `cargo test`; the numeric
gate compares the guest against the native twin / r4133 oracle at the calibrated
harness floors (`tools/wasm_usermodel/README.md` has the full chain).

**Limits & trap policy (§6 recap).** The sandbox is deterministic: relaxed SIMD
off, no WASI, no clock/random/filesystem — `dss_env` is the entire import
surface, so the guest is a pure function of its inputs. Each call has a fuel
budget (default generous; the fixture uses <1%) and a linear-memory cap (default
64 MiB); a trap, protocol violation (bad pointer/OOB, `dss_alloc` returning 0),
fuel exhaustion, or memory-cap breach is a hard engine error naming your model
and function — there is **no** mid-run fallback to the built-in model (that
would silently change numerics). Only *load-time* failures (not a `.wasm` /
missing file / missing export) follow Pascal's warn-and-fall-back (§5).

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
