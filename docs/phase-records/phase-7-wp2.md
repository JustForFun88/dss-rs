# Phase 7 — WP7.2 (Protection) — archived per-step records

> **Archived from `STATUS.md`** (steps 1–2c moved 2026-06-22; steps 2d/3/4 moved
> 2026-06-23 on WP7.2 completion) to keep the live handoff lean. WP7.2 (Protection)
> is **✅ COMPLETE** on the `phase-7-extended-elements` branch; these are the frozen
> records of all its sub-steps (1 `Fault`, 2a `SwtControl`, 2b `Fuse`, 2c
> `Recloser`, 2d `Relay`, 3 reliability activation, 4 protection gate + corpus
> migration), superseded only by the code and tests. The live `STATUS.md` §1e keeps
> a one-line-per-step summary + the cross-cutting **Phase-7 carry-forward** rules
> (the dirty-edge discipline, the reliability single-flag model, the Generic/TD21
> WP7.7 deferral). §3/§4/§5 cross-references resolve against `STATUS.md`. Plan:
> `PHASE7_PLAN.md` §WP7.2.

---

### 1e WP7.2 step 1 — the `Fault` element (`pd/fault.rs`) — ✅ done, gate-green

First step of WP7.2 (Protection): the `Fault` object (`PDElements/Fault.pas`,
`TFaultObj`) — an uncoupled multi-phase **conductance** branch and the FaultStudy
input (WP7.9). Landed registered, snapshot-solvable, and pinned offline.
- `crates/dss-core/src/elements/pd/fault.rs` (+ `fault/tests.rs`): the element
  (`define_properties!`-style `class_props`, struct, side effects, `MakeLike`),
  `CalcYPrim` (SpecType 1 = single `G=1/r` diagonal; SpecType 2 = `Gmatrix`), and
  the time-mode `CheckStatus`/`Reset`/`FaultStillGoing` (enable past `ONtime`,
  temporary self-clear below `MinAmps`). `r` stores its inverse `G`
  (`InverseValue`); `GMatrix` shares the oracle's DoubleSymMatrix garbage-getter
  bug (rendered zeros, like Capacitor.CMatrix).
- Class type `FAULTOBJECT or NON_PCPD_ELEM`: a `TPDElement` with a YPrim in the
  system Y but **excluded** from `pd_elements` (Pascal `AddCktElement`) — added to
  `ckt_elements` (so it stamps) + a new `Circuit.faults` list only. New
  `ElemKind::Fault`; registered after Reactor (`construct.rs`, DSSClassDefs:222).
- **Control-loop wiring** (the dormant Phase-7 placeholders, now live): a new
  `solution/faults.rs` with `check_fault_status` (the control-iteration loop,
  `power_flow.rs` — sets `system_y_changed` when a fault toggles `Is_ON`, per
  Pascal `Set_YprimInvalid`→`SystemYChanged`) and `reset_faults` (`DoResetFaults`,
  wired into the `Set mode=` tail and `Reset Faults` 'F' selector). Duty/event/time
  modes set `control_mode = TIMEDRIVEN`, so `CheckStatus` fires; snapshot is
  `CTRLSTATIC` (no-op).
- **MonteFault** randomization (`Randomize` + `RandomMult` jitter) is deferred with
  its solve loop to WP7.9; `CalcYPrim` keeps the Pascal `RandomMult = 1.0` guard
  for every non-MonteFault mode, so the field is inert but faithful.
- Gates (all oracle-probed): `props.json` `fault.json` (6 scenarios) via
  `gen_props.py`; 8 inline tests pinning YPrim entry-by-entry (1φ `r`, 3φ `r`, 2φ
  `Gmatrix`, off=zero), a snapshot 3φ fault drawing **1342.808 A** (full-circuit),
  and a duty-mode temporary fault logging **`**APPLIED**`** at the probed step.
  `gen_props.py` `_NUM_RE` extended to zero the non-finite `Nan`/`Inf` the oracle's
  garbage matrix getter can emit (3φ `GMatrix`). dss-core lib **392→400** (→401
  with the audit-tests follow-up); full three-command gate green on **stable**.
- **Corpus migration deferred to the WP7.2 gate (step 4):** the
  `unsupported_class={Fault,Fuse,Recloser,Relay,SwtControl}` cases migrate once the
  protection set is complete (PHASE7_PLAN §3 WP7.2 step 4). A targeted
  `phase7/protection*.json` golden is part of that gate.
- **Audit-code follow-up:** `MakeLike` was missing the `TPDElement` rating fields
  (`NormAmps`/`EmergAmps`/`FaultRate`/`PctPerm`/`HrsToRepair`) that
  `TPDElement.MakeLike` copies — added them; pinned by extending the
  `fault_makelike` props scenario with `faultrate=0.5 pctperm=80 repair=4`.
  (Verified clear: the `phases` side effect signals `bus_name_redefined` through
  `set_nconds` — no divergence; `CalcYPrim`/`CheckStatus` checked against Pascal +
  the oracle.)
- **Audit-tests follow-up:** the temporary-fault coverage exercised only the
  `**APPLIED**` path; added `temporary_fault_clears_below_minamps` (MinAmps above
  the fault current → self-clear), pinning the `**CLEARED**` event from the oracle
  probe. dss-core lib **400→401**. Gate green.

### 1e WP7.2 step 2a — the `SwtControl` switch control (`control/swt_control/`) — ✅ done, gate-green

The first protection **control** (`Controls/SwtControl.pas`, `TSwtControlObj`): a
manual/automatic **switch** that opens or closes every phase conductor of a
controlled element's terminal after a time delay, and can be *locked*. The
simplest control of the WP7.2 set — no TCC/sensing — so it lands the generic
"control opens/closes a controlled PD terminal" machinery the protection devices
(Fuse/Recloser/Relay, step 2b) reuse.
- `crates/dss-core/src/elements/control/swt_control/{mod,accessors,tests}.rs`: the
  element on the WP5.7 control sweep — `Sample` (queue the pending lock/switch
  action; reads **no** monitored quantity), `DoPendingAction` (lock/unlock or
  open/close all phases of the switched terminal + `Opened`/`Closed` event log),
  `Reset` (restore to `NormalState`). Registered after StorageController (Pascal
  `DSSClassDefs.pas:249`); `ElemKind::Control`, joins `ckt.controls`.
- **Generic conductor-open machinery:** new `CktElementData::set_terminal_closed`
  / `terminal_all_phases_closed` (Pascal `Set_/Get_ConductorClosed(0)` with the
  active terminal = the switch terminal) — opens/closes a *generic* controlled
  element (Line, etc.), marking `yprim_invalid` (the open-conductor Kron reduce in
  the existing `do_yprim_calcs` does the rest). The control-loop dispatch
  (`solution/controls/dispatch.rs`) gained `ControlKind::Swt`: `Sample` borrows
  only the control; `Action`/`Reset` borrow the control + the controlled element
  via `pair_mut` + `as_ckt_element_mut` (generic — any switched class).
- **Property quirks settled against the oracle** (probed, `swtcontrol.json`):
  `Action`/`Normal`/`State` all map onto the one `CurrentAction` field — the text
  `?` dump renders it (Action `close`/`open`, Normal/State `closed`/`open`); the
  `State` read-function `GetState` is **not** used by the dump (proved: `action=open`
  leaves the line *closed* yet `State` dumps `open`). They are `ConditionalReadOnly`
  on `Locked` — a write while locked is **ignored** (`lock=yes action=open` ⇒
  `Action=close`; parse-order sensitive). `State=` additionally forces the
  controlled element to that state at parse time, deferred as a new
  `RefAction::SetSwitchClosed` (applied generically by the executive through the
  CktElement base, since the switched element can be any class) — the established
  RegControl-`TapNum` deferred-write pattern.
- Two `SwtControl` enums registered (`swt_control_action` close/open,
  `swt_control_state` closed/open; EControlAction ordinals CTRL_CLOSE=2/CTRL_OPEN=1),
  plus CTRL_RESET/CTRL_LOCK/CTRL_UNLOCK added to the shared `EControlAction` set.
- Gates (all oracle-probed): `props.json` `swtcontrol.json` (7 scenarios —
  default, action/normal/state=open, lock+delay, the locked-read-only path, and
  makelike) via `gen_props.py`; **14 inline tests** — `Sample` arm/no-arm,
  `DoPendingAction` open/close (terminal flipped + `OPENED`/`CLOSED` event), the
  lock-blocks-open and locked-read-only paths, the `State=` deferred-force queue,
  `Reset`, `MakeLike`, plus two executive tests (a parallel-fed switch opening on
  `action=open` in a duty solve; `state=open` forcing the line open at parse). The
  event-log line format is `Element=SwtControl.sw1, Action=OPENED` (probe-confirmed
  it logs without `Set Log=yes`). dss-core lib **401→415** (→418 with the
  audit-tests follow-up); full three-command gate green on **stable**.
- **Corpus migration deferred to the WP7.2 gate (step 4):** the lone corpus
  SwtControl case (only bare `switchedobj=` appears — no `action`/`state`/`lock`
  usage across the corpus) migrates with the rest of the
  `unsupported_class={Fault,Fuse,Recloser,Relay,SwtControl}` set once the protection
  block is complete (PHASE7_PLAN §3 WP7.2 step 4), alongside the targeted
  `phase7/protection*.json` golden.
- **Audit-code follow-up:** of the three surfaced notes, one is a non-bug kept
  as-is (the typed `GetState` accessor is unreachable in this CAPI-less port — the
  `?` dump faithfully returns `CurrentAction` on **both** engines, so reading the
  live `Closed[0]` instead would *diverge* from the oracle; the live semantics
  belong to the unported typed getter / a future report path). The
  `ActiveTerminalIdx` note was **fixed** for literalness — `DoPendingAction` now
  sets the controlled element's `active_terminal := ElementTerminal` before the
  case (for every code, incl. LOCK/UNLOCK), as Pascal does (behaviorally inert —
  nothing reads it after a lock — but a 1:1 port; covered by
  `do_pending_sets_controlled_active_terminal_even_for_lock`). The third — *Reset
  gated `system_y_changed` on an all-or-nothing change check* — was promoted to a
  **real fix** on review: Pascal
  `Reset` does `Closed[0] := …` unconditionally, and the change-gated version
  could miss a real Y change on a **partially-open** terminal (one phase closed,
  the rest open ⇒ `terminal_all_phases_closed` reads false ⇒ `was == want` ⇒ the
  rebuild is skipped ⇒ stale system Y, since `build_y_matrix` is gated solely on
  `system_y_changed`). Refactored the SwtControl Reset into a `reset_with(ctrl)`
  method (mirroring `CapControl::reset_with`) that raises `system_y_changed`
  unconditionally when a force is applied; **fixed the same dirty edge in
  `CapControl::reset_with`** (`want_closed.is_some()`, was
  `is_some_and(want != was)`). `RegControl::Reset` is clean (touches no element —
  just `PendingTapChange=0; Armed=FALSE`). Both fixes carry a **fail-on-regression
  partial-open test** (proven: re-introducing the change-gate makes each test
  fail). dss-core lib **418→420** (SwtControl 18 + a CapControl partial-open test;
  net of the `reset_control_side` test reshape). Gate green.
- **Forward guard for WP7.2 step 2b (Recloser/Relay/Fuse) — `system_y_changed`
  discipline + required tests:** a protection *trip* opens the controlled
  element's conductors, so every `DoPendingAction`/reset that forces conductors
  (`set_terminal_closed`/`Closed[0] := …`) must raise `system_y_changed`
  **unconditionally** (or via an *exact* per-conductor check), **never** via an
  all-or-nothing `terminal_all_phases_closed`/`is_closed` aggregate — the
  `build_y_matrix` trigger is gated solely on `system_y_changed`, so a
  partially-open terminal otherwise slips a real change past the rebuild → stale Y
  (the step-2a Reset dirty edge, `d0addb4`). **Each protection device must ship a
  partial-open fail-on-regression test** modeled on
  `reset_with_partial_open_*` (SwtControl/CapControl). The verified sweep found no
  *other* current sites (every existing `system_y_changed` write is unconditional
  or gated on an exact check — scalar tap `v != putap`, `add_step`/`subtract_step`,
  fault `Is_ON` toggle, direct `yprim_invalid` propagation); per-phase open is a
  single shared base path (`apply_yprim_open_conductor_calcs`), and a winding tap
  is scalar (per-phase regulation = separate single-phase regulators). The
  unported `open`/`close` exec commands must likewise set the flag when ported.
  Recorded in PHASE7_PLAN §3 WP7.2 step 2.
- **Audit-tests follow-up:** the audit found two genuinely-untested new paths and
  one under-pinned guard; added 3 tests (dss-core lib **415→418**): an executive
  `reset_restores_switch_to_normal_via_dispatch` (the dispatch `Reset` element-force
  — `reset` re-closes a `state=open` line; oracle-probed), a `reset_yes_unlocks_and_
  restores_with_force` (the `Reset=yes`/DoReset unlock + restore + deferred force),
  and `locked_ignores_normal_and_state_writes` (the ConditionalReadOnly guard on
  Normal/State, previously pinned only for Action). Gate green.

### 1e WP7.2 step 2b — the `Fuse` per-phase TCC protection (`pd/fuse/`) — ✅ done, gate-green

The first **TCC/sensing** protection device (`PDElements/fuse.pas`, `TFuseObj`):
a per-phase fuse on the WP5.7 control sweep that, despite living in the Pascal
`PDElements/` tree, is a `TControlElem` (zero Yprim/current). It monitors one
element's terminal currents and **blows individual phases** of the controlled
element when a phase current stays above the `FuseCurve` pickup. Each phase has
its own link state, arm flag, and queue action — so it lands the
sensing/TCC/per-phase machinery Recloser (2c) and Relay (2d) reuse.
- `crates/dss-core/src/elements/pd/fuse/{mod,accessors,tests}.rs`: `Sample`
  reads `MonitoredElement.GetCurrents` and, per closed phase, evaluates
  `FuseCurve.GetTCCTime(Cmag / RatedCurrent)`, arming/disarming a per-phase
  `ControlQueue` action at `TripTime + Delay`; `DoPendingAction(phase)` opens
  that one conductor and logs `Phase N Blown`; `Reset` restores each phase to
  `Normal`. Registered with the protection controls (Pascal `DSSClassDefs`
  Relay/Recloser/Fuse), `ElemKind::Control`; dispatch (`ControlKind::Fuse`)
  borrows the control + controlled + monitored (the default fuse monitors its
  own controlled element — the same-element clone path, like CapControl).
- **`TCC_Curve.GetTCCTime` ported** (`tcc_curve/mod.rs`): the log-log
  interpolation over the (already-precomputed) `log_c`/`log_t` arrays, including
  the `LastValueAccessed` hunt cache (now a per-owner field — identical results
  to Pascal's per-curve field for the monotonic curves that are the only kind in
  practice). Unit-tested against the built-in `tlink` curve (Python reference).
- **New shared machinery (Recloser/Relay will reuse):** per-conductor
  `CktElementData::set_conductor_closed`/`conductor_closed` (Pascal
  `Set_/Get_ConductorClosed(index>0)` — a single phase, vs step-2a's
  whole-terminal `set_terminal_closed`); `RefAction::SetConductorsClosed` (the
  per-phase `State=`/`Action=` parse-time force, applied generically by the
  executive through the CktElement base); and a new property type
  `PropType::MappedStringEnumArray` (a `SizeIsFunction` mapped-enum array sized
  by `DssObject::array_size`, dumped `[s1, s2, ]`) for the per-phase
  `Normal`/`State`, with `get_enum_array`/`set_enum_array` base hooks.
- **`FuseCurve` default `tlink`:** Pascal's constructor does
  `Find('tlink')`; our constructor cannot reach the registry, so the executive
  resolves the (default or explicit) curve name through the same `foreign` view
  the property edits use — cloning the `TccCurveObj` into the Fuse for
  solve-time `GetTCCTime`, after the edit loop in `command.rs`. The built-in
  `tlink`/`klink`/… curves already exist (`CreateDefaultDSSItems`).
- **Property quirks settled against the oracle** (probed, `fuse.json`):
  `MonitoredObj` defaults `SwitchedObj` to the same element (and `MonitoredTerm`
  → `SwitchedTerm`); `Normal`/`State` are per-phase enum arrays sized by
  `ControlledElement.NPhases`, dumped `[closed, closed, closed, ]` (a short
  input sets only the leading phases — `state=[open]` opens only phase 1);
  `State=` forces the controlled conductors **per phase** at parse time;
  `Action` (deprecated close/open) sets all phases then runs the State side
  effect and dumps empty; `MakeLike` copies the refs/rating/states but **not**
  `DelayTime` or `NormalStateSet` (Pascal omits them).
- Two `Fuse` enums registered (`fuse_action` close/open, `fuse_state`
  closed/open; EControlAction ordinals CTRL_CLOSE=2/CTRL_OPEN=1).
- Gates (all oracle-probed): `props.json` `fuse.json` (8 scenarios — default,
  1-phase, action=open, state all/partial, normal partial, explicit
  curve+rating+switched, makelike) via `gen_props.py`; **20 inline tests** — 8
  Fuse (Sample arm/disarm, per-phase blow + `PHASE N BLOWN` event, disarmed
  no-op, Reset, the `state=[open]` partial force, MakeLike) incl. a partial-open
  `reset_with` fail-on-regression guard + 3 executive tests (per-phase parse
  force, default `tlink` resolution, end-to-end overcurrent blow), and 4
  `TccCurveObj::get_tcc_time` tests. dss-core lib **421→435**; full
  three-command gate green on **stable** (incl. the always-on `corpus_live`).
- **`system_y_changed` discipline (the step-2a guard):** the per-phase blow
  (`DoPendingAction`) and `reset_with` raise `system_y_changed` **unconditionally**
  on a conductor flip (never gated on an aggregate); `reset_with` carries a
  **partial-open fail-on-regression test** as required by the step-2b guard.
- **`HasOCPDevice` / recalc-Closed-resync deferred to WP7.2 step 3** (the
  reliability activation, as PHASE7_PLAN §3 sections it): the Fuse's
  `RecalcElementData` would `Include(ControlledElement.Flags, HasOCPDevice)` and
  resync the controlled `Closed[i]`, both reaching the controlled element which
  `recalc` cannot see; they land with `GetOCPDeviceType` in step 3 (the
  parse-time `State=` force already drives the element). Noted in `fuse/mod.rs`.
- **Corpus migration blocked on co-occurring classes:** every Fuse-using corpus
  case is also tagged `unsupported_class=…Recloser,Relay,PVSystem,Storage…`
  (e.g. `Test/IEEE13_CDPSM.dss`), so none can migrate to `solvable_now` until
  Recloser/Relay (2c/2d) and the DER block land — migration stays at the WP7.2
  gate (step 4), with the targeted `phase7/protection*.json` golden.
- **Audit-code follow-up:** the `/audit-code` pass confirmed a faithful 1:1 port
  (no Critical/Major); the one fixed finding was the dropped FUSEMAXDIM-overflow
  **warning** (Pascal `fuse.pas` DoSimpleMsg 404 when the monitored element has
  >6 phases) — now emitted in `recalc`. The other notes are kept as-is: the
  per-phase arrays / `GetFuseStateSize` are deliberately `FUSEMAXDIM`-capped
  (Pascal's own getter carries a documented invalid-access risk for >6 phases, and
  such fuses don't exist in the corpus); the recalc `Closed[i]` resync is redundant
  at parse (the `State=`/`Action=` force already drives it) and rides with the
  step-3 `HasOCPDevice` work; `CondOffset` is confirmed vestigial in Pascal (Sample
  reads `cBuffer[i]`). Gate green.
- **Audit-tests follow-up:** the `/audit-tests` pass found the partial-open
  `reset_with` guard ineffective — its "all-closed target" seed made a reintroduced
  aggregate gate (`was != want`) compute the same result as the correct code, so it
  could not fail on the regression it names. **Reseeded** it to terminal
  `[open, closed, closed]` + `normal=[CLOSE,OPEN,CLOSE]` so the aggregate reads false
  **both before and after** the reset while phases 0/1 actually flip (proven: the
  reseeded test now fails when the aggregate gate is reintroduced; the old seed did
  not). Also strengthened the event-log assertion to the full normalized line
  (`Element=Fuse.f1, Action=PHASE N BLOWN`) and added two tests: a `RatedCurrent`
  divisor trip (`5 A / 10 A = 0.5 pu` below pickup → no arm — pins `Cmag/RatedCurrent`
  against a dropped/inverted divisor) and the missing-`SwitchedObj` error path
  (Pascal `#405`, oracle-confirmed). dss-core lib **435→437**. Gate green.

### 1e WP7.2 step 2c — the `Recloser` overcurrent recloser (`control/recloser/`) — ✅ done, gate-green

The second **TCC/sensing** protection device (`Controls/Recloser.pas`,
`TRecloserObj`): an overcurrent recloser on the WP5.7 control sweep that monitors
a PD terminal's currents, trips the controlled element's **whole terminal** open
on a phase/ground TCC pickup, then **recloses** it after a configured interval —
repeating up to `Shots` operations before locking out, the first `NumFast` on the
*fast* curves and the rest on the *delayed* curves. Reuses the step-2b sensing
machinery (`TccCurveObj::get_tcc_time`, the control-queue arm/disarm, the
`SetSwitchClosed` whole-terminal force from step 2a).
- `crates/dss-core/src/elements/control/recloser/{mod,accessors,tests}.rs`:
  `Sample` reads `MonitoredElement.GetCurrents` and evaluates the ground-sum and
  per-phase `GetTCCTime(Cmag / Trip)` (plus the inst-trip on operation 1),
  arming an `OPEN` then a reclose `CLOSE` on the queue; `DoPendingAction(OPEN/
  CLOSE/RESET)` flips the whole controlled terminal and advances/locks/resets the
  operation count, logging `Opened, Fast`/`Opened, Delayed`/`Opened, Locked Out`/
  `Closed` + `Phase`/`Ground Target`; `Reset` restores `NormalState`. Registered
  **before Fuse** (Pascal `DSSClassDefs.pas:243`, Relay/Recloser/Fuse order;
  Relay/2d still pending), `ElemKind::Control`; dispatch (`ControlKind::Recloser`)
  borrows control + controlled + monitored with the same-element clone path the
  Fuse uses.
- **New engine machinery (Relay/2d reuses both):**
  - `PropFlags::ARRAY_MAX_SIZE` + `PropDef::double_v_array_max(name, max)` — the
    Pascal `DoubleVArrayProperty` + `ArrayMaxSize` parse (`ParseAsVector(maxSize,
    array)`): reads **up to** `max` values, the object sets its own element count,
    the dump renders `array_size` of a fixed buffer. Drives `RecloseIntervals`.
  - the **integer-dump `VALUE_OFFSET`** (Pascal `GetObjInteger` subtracts the
    offset) — `Shots` stores `NumReclose = Shots − 1` and dumps `NumReclose + 1`.
- Two `Recloser` enums registered (`recloser_action` close/open/trip,
  `recloser_state` closed/open/trip → ordinals 2/1/1; `trip` aliases `open`, so an
  opened recloser dumps `open`). Default `PhaseFast`/`PhaseDelayed` resolve to the
  built-in `a`/`d` curves (`CreateDefaultDSSItems`) via the same `command.rs`
  `foreign`-view resolution the Fuse `tlink` uses (all four curves cloned in for
  solve-time `GetTCCTime`).
- **Property quirks settled against the oracle** (probed, `recloser.json`):
  `Action`/`State` map onto `FPresentState`, `Normal` onto `NormalState`; the
  first `State`/`Action` defaults `Normal` (`NormalStateSet`). `Shots` **and**
  `RecloseIntervals=(…)` both set `NumReclose` (last write wins): `shots=2
  recloseintervals=(1 3)` ⇒ Shots 3 / `[ 1 3]`, `shots=1` ⇒ NumReclose 0 ⇒ `[]`.
  `RecalcElementData` syncs the controlled terminal to `FPresentState` (the
  `SetSwitchClosed` RefAction — so `state=open` opens the line at parse). `MakeLike`
  copies the trips/curves/shots/intervals/normal state but **not** `DelayTime` or
  the TD* time dials (Pascal omits them).
- **`HasOCPDevice`/`HasAutoOCPDevice` deferred to WP7.2 step 3** (as the Fuse —
  the reliability flags reach the controlled element which `recalc` cannot see;
  they land with `GetOCPDeviceType`). The whole-terminal Closed[0] sync **is**
  done here (queued from recalc) — unlike the Fuse, the recloser has no parse-time
  element force, so the recalc sync is its only path for `state=`.
- Gates (all oracle-probed): `props.json` `recloser.json` (7 scenarios — default,
  full+ground curves+switched, shots-then-intervals aliasing, one-shot empty
  array, state=open, normal=trip, makelike) via `gen_props.py`; **22 inline
  tests** (Sample arm/reclose/disarm, fast↔delayed + inst selection, ground-sum
  trip, terminal-open skip; DoPendingAction trip/lockout/delayed/reclose/
  reset; Reset closed/open; MakeLike-omits-time-dials; 4 executive tests — default
  dump, shots/intervals aliasing, `state=open` forces the controlled terminal,
  end-to-end overcurrent trip). dss-core lib **437→459**; full three-command gate
  green on **stable** (incl. the always-on `corpus_live`).
- **Corpus migration blocked (same as 2b):** Recloser-using corpus cases also tag
  `Relay`/`PVSystem`/`Storage`, so migration stays at the WP7.2 gate (step 4)
  with the targeted `phase7/protection*.json` golden.
- **Audit-code follow-up (`/audit-code` step 2c):** the pass confirmed a faithful
  1:1 port — **no Critical/Major**. `Sample`/`DoPendingAction`/`RecalcElementData`/
  `Reset`/`MakeLike` and the two engine pieces (`ARRAY_MAX_SIZE`, integer-dump
  `VALUE_OFFSET`) all match Pascal line-for-line; `reset_with` raises
  `system_y_changed` unconditionally (the step-2a dirty-edge discipline). The only
  fix was a **doc-comment clarity tweak** (`reclose_intervals[4]` is dead only for
  the default `Shots ≤ 4`; a larger `Shots` reads it, but Pascal's slot is
  uninitialized there too — the oracle dumps `Nan`, so `0.0` is the safe defined
  choice, probe-confirmed). The MakeLike whole-array copy vs Pascal's
  `[1..NumReclose]` and the NIL-element generic-abort text are unpinnable/degenerate
  (documented, no change). Gate green.
- **Audit-tests follow-up (`/audit-tests` step 2c):** closed the two Major test
  gaps the audit flagged plus the mandated dirty-edge guard. (1) **No trip *time*
  was pinned** — every Sample assertion was `queue_size`-only, so a dropped
  `+DelayTime`, a missing time-dial, or an interval-index off-by-one passed: added
  `sample_queues_trip_and_reclose_at_correct_times`, which `pop_time`s the OPEN and
  reclose CLOSE and asserts `TripTime = TDPhFast·GetTCCTime = 0.2`, OPEN at `+Delay
  = 0.25`, reclose at `+RecloseIntervals[0] = 0.75`. (2) The **end-to-end test
  under-asserted** (log substring only) — strengthened to `OPENED, FAST` **and**
  `line_term1_max_current(Line.l1) < 1.0` (the line actually opens, the Fuse
  precedent). (3) Added the **partial-open `reset_with` fail-on-regression guard**
  the WP7.2 step-2a rule (`d0addb4`/`d1f48231`) mandates for every protection
  control — seed `[closed, open, open]` + `normal=OPEN` ⇒ aggregate false before
  **and** after while phase 0 flips, so a reintroduced `was != want` aggregate gate
  fails it. Plus three Minor gaps: the `GROUND TARGET` event line, the OPEN-when-open
  / CLOSE-when-closed no-op guards, and the MakeLike curve-clone copy (a `like=`
  recloser still trips). dss-core lib **459→463**. Gate green. *(The trip/reclose
  **sequence**-level event-log-equality + final-state golden remains the planned
  step-4 `phase7/protection*.json` gate.)*

### 1e WP7.2 step 2d — the `Relay` general protection control (`control/relay/`) — ✅ done

The general protection control (`Relay.pas`, 2354 lines — the largest WP7.2 unit).
Nine `Type=` sub-types over one 50-property surface + the shared `Closed[0]`
trip/reclose/reset machine. Module split `mod.rs`
(props/struct/recalc/Sample dispatch/DoPendingAction/Reset) + `logic.rs` (the
per-sub-type sensing) + `accessors.rs` + `tests.rs`. **Ported live:** `Current`
(overcurrent 50/51), `Voltage` (27/59 + voltage reclose), `ReversePower` (32),
`46`/`47` (neg-seq), `Distance` (21), `DOC` (directional overcurrent — the dominant
corpus type, with the `DOC_P1Blocking` forward-power block + the
circle/high-line/inner-zone decision tree ported verbatim). **Deferred to WP7.7
(`NOT_PORTED` if `Sample` reached):** `Generic` (needs PC state `Variable[]`) and
`TD21` (needs `DynaVars.h`/`Frequency`/`IterationFlag` + the per-cycle ring buffer)
— both fully parse + dump. New shared machinery:
`TccCurveObj::get_ov_time`/`get_uv_time` (definite-time scans) and
`PropFlags::ALLOW_NONE` (zero-count `DoubleVArray` ⇒ `[NONE]`, the `Type=DOC` ⇒
NumReclose 0 case). Relay-specific quirks vs the Recloser: event-log lines are
**gated on `ShowEventLog`** (`if ShowEventLog then AppendToEventLog`), the queue
`CTRL_RESET` runs the full `Reset()` (logs "Resetting" + re-forces the element),
and `MakeLike` **copies** `DelayTime`/`BreakerTime`. `relay.json` (9) + 27 inline.
- *audit-code follow-up:* verdict faithful, no Critical/Major; tidied the
  Generic/TD21 `NOT_PORTED` log to fire **once** per object (a `not_ported_logged`
  latch, not once per control iteration), and pinned the dead `Type=Voltage`
  `RecloseIntervals[3]=5.0` upstream quirk with a "don't simplify" comment.
- *audit-tests follow-up:* filled the ported-but-untested gaps (no oracle backstop
  until the step-4 corpus gate) — Distance (in/out-of-reach + `DistReverse`
  negation), DOC end-to-end through `Sample` (3-phase reverse-power trip +
  forward-power block via the `Phase2SymComp` path), `NegSeq47`, the Voltage
  reclose branch, the queue-driven `DoPendingAction(CTRL_RESET)` entry, and the
  `recloseintervals=NONE` parse; **27 → 37 inline**, lib **492 → 502**.

### 1e WP7.2 step 3 — reliability activation (OCP devices) — ✅ done

An enabled Relay/Recloser/Fuse now marks its controlled element with
`Flg.HasOCPDevice` (Pascal `RecalcElementData`'s `Include(...)`), deferred as a new
`RefAction::SetOcpDevice` queued from each control's `recalc` (the property engine
holds no mutable view of the controlled element; the executive applies it).
Relay/Recloser are auto-reclosing → also set `HasAutoOCPDevice`; the Fuse sets only
`HasOCPDevice`. `GetOCPDeviceType` (1=Fuse/2=Recloser/3=Relay) is recorded as a new
`CktElementData.ocp_device_type` when the flag is set (first-registered OCP control
wins, matching the Pascal `ControlElementList` scan that stops at the first match);
the reliability sweep reads it + `SeqIndex` (the 1-based `SequenceList` index) into
the section record. The Fuse `recalc` also now does the deferred per-phase
`Closed[i]` resync. The dormant Phase-6 `RelCalc` SAIFI/SAIDI/section math goes
**live**: a protected zone no longer aborts #52902. Tests: `exec/tests/
reliability.rs` +4 — per-class flags + `GetOCPDeviceType` ordinal, disabled-control
sets-no-flag, and oracle-pinned SAIFI/SAIDI/SAIFIkW/CustInterrupts/CAIDI for a
head-line and a downstream-line recloser (`tools/golden/probe_reliability.py`).
**lib 502 → 506** (the +4 land in the `exec::tests::reliability` module).
- *audit-code follow-up:* verdict **faithful, no Critical/Major** — device-type
  ordinals, the auto-vs-non-auto split, the enabled-gated `Include`, the 1-based
  `SeqIndex`, and the Fuse `Closed[i]` resync all match Pascal; the observable
  indices are oracle-pinned. Two Minor edge-gaps, both **unobservable** (the section
  `OCPDeviceType`/`SeqIndex` are written but never read by the SAIFI/SAIDI/CAIDI
  math — they surface only via the un-exported `Meters_Get_OCPDeviceType`/`SectSeqIdx`
  C-API): (i) `GetOCPDeviceType` is derived from *enabled* controls only, vs Pascal's
  `ControlElementList` scan that ignores `Enabled`; (ii) no `Exclude` on
  move/disable, so re-pointing or disabling a control leaves the old element's flag
  stale (consistent with the existing force model). Both documented deferrals — no
  code change.
- *audit-tests follow-up:* the four step-3 tests are genuine oracle-pinned guards.
  Added: `disabled_ocp_device_sets_no_flag` now asserts the promised `RelCalc` abort;
  `ocp_device_type_first_registered_wins` (oracle `Meters.OCPDeviceType` 1 vs 2 by
  definition order); `relcalc_assume_restoration_changes_auto_ocp_interruptions` (a
  3-section auto-recloser feeder — SAIFI 0.5133→0.4417, CustInt 21.56→18.55, SAIDI
  2.49 both, oracle-pinned); `enable_then_disable_leaves_ocp_flag_stale` pins the
  documented move/disable deferral explicitly. **lib 506 → 509**.

### 1e WP7.2 step 4 — protection gate + corpus migration — ✅ done

The WP7.2 exit gate (two tiers, the established Phase-7 pattern).
- **Targeted golden `phase7_protection/*.json`** (`gen_phase7_protection.py` +
  `golden_phase7_protection.rs`, **5 scenarios**): a fault + protection
  **trip/reclose sequence** driven through the **ported** `mode=duty controlmode=time`
  control sweep (the corpus relay demos use the unported dynamics mode → WP7.7, so
  the golden uses duty), pinning per-step `dblHour` + iteration count + node voltages
  (the feeder collapses to ~0 on every step the line is held open — the voltage
  trajectory encodes the discrete state), the **event log line-for-line** (normalized,
  skeleton-exact — FAST/DELAYED/LOCKED OUT/CLOSED/PHASE TARGET/BLOWN/RESETTING), and
  every element's final-step currents/powers + name-set equality (a held-open line
  carries ~0 A, a reclosed line full load — the final switch/recloser/fuse state
  pinned exactly). Scenarios: `recloser_temp` (temporary fault → FAST trip →
  self-clear → reclose), `recloser_perm` (permanent → FAST → reclose → DELAYED →
  reclose → LOCKED OUT — NumFast / RecloseIntervals / Shots / lockout),
  `relay_current` (`Type=Current`, `eventlog=yes` → RESETTING + OPENED ON PH & LOCKED
  OUT), `fuse_blow` (per-phase tlink fuse → PHASE 3/2/1 BLOWN), `swt_manual` (a
  mid-run `edit swtcontrol.x action=open` — the corpus civanlar pattern). The Rust
  engine reproduced every oracle sequence **exactly on the first run** — strong
  end-to-end validation of the steps 1–3 ports through a multi-step control sweep.
- **`Open`/`Close` exec verbs ported** (`command.rs::do_open_close_cmd`, Pascal
  `DoOpenCmd`/`DoCloseCmd` ExecHelper.pas:1451/1484; `cmd::OPEN`=17/`CLOSE`=18):
  `Open class.name term cond` forces a terminal (cond 0 ⇒ whole terminal via
  `set_terminal_closed`; cond>0 ⇒ one conductor via `set_conductor_closed`) and
  raises `system_y_changed` (the step-2a dirty edge), reusing the protection
  switching machinery; `set_active_ckt_element` mirrors the Pascal helper
  (253/254/259 diagnostics). Pascal's `SetActiveBus` side effect is inert here (no
  ported verb reads an active bus). **4 oracle-pinned tests** (`exec/tests/
  open_close.rs`): Line whole-terminal open/close round-trip (24.384 A ↔ 0),
  single-conductor open ([24.377, 0, 24.401]), **transformer winding** open (the
  DG_Prot_Fdr pattern — load 500 kW ↔ 0), and the #259 error path.
- **Corpus migration:** the protection-only-blocked cases were probed live via
  `DSS_LIVE_CLASSIFY=1 corpus_live_classify` + `apply_classify.py`. **`civanlar`
  (SwtControl) + `IEEE_519` (SwtControl) migrated into `solvable_now` (35→37)**;
  `COVERAGE.md` refreshed (37 = 11.0% of entry points). Most protection corpus cases
  stay skipped because they embed *unported* commands/modes — Phase-8
  `Show`/`Export`/`Plot`/`BatchEdit` and the WP7.7 `dynamics` solve mode (the
  Distance/TD21 relay demos, the DOCTechNote/HarmonicsTMode feeders) — not the
  protection classes, which are all ported now.
- **Tracked-open (needs_investigation): `DG_Prot_Fdr.dss`** — the canonical
  Fault/Fuse/Recloser/Relay feeder now compiles (protection + `Open` landed it), but
  the live **system Y diverges ~3e-5 rel at a line node** (entry 23, |diff|=8.0e-3 >
  allowed 3.5e-4). A **WP7.1 Carson line-constants precision divergence** on the
  feeder's `linegeometry`/`linespacing`/`wiredata`, **not** a protection/`Open`
  regression — `Open` is oracle-verified on Line + Transformer terminals and
  protection controls carry no YPrim. Parked for the WP7.1 / Phase-7
  geometry-precision follow-up (the plural-cable note's family).
- dss-core lib **509 → 513** (the 4 `open_close` tests); golden suite +1
  (`golden_phase7_protection`).
- *audit-code follow-up:* verdict faithful, no Critical/Major. One Minor fixed:
  `set_conductor_closed`/`conductor_closed` guarded `<= Nphases`, but Pascal
  `Set_/Get_ConductorClosed(index>0)` guard `<= Fnconds` — the new single-conductor
  `Open`/`Close` path silently no-oped on a **neutral** conductor that Pascal opens.
  Relaxed both guards to `Nconds` (behaviour-preserving for the Fuse/parse-force
  callers, which only pass `1..=Nphases`); pinned by a `ckt::tests` conductor-guard
  unit test. lib **513 → 514**.
- *audit-tests follow-up:* real oracle-pinned gates, no Critical/Major. One Minor
  strengthened: `golden_phase7_protection` now asserts the **element name sets match
  exactly** (Rust == oracle, like `corpus_live.rs`) so an *extra* Rust element is
  caught, not just a dropped one.
