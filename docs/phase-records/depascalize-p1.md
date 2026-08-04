# DE_PASCALIZE Part II — WP P1: integer-constant families → enums

Stratum **[A]** bit-neutral. No arithmetic, no accumulation order, no golden, no
tolerance touched. Every converted field round-trips through the `DssEnum`
registry exactly as before; `i32` now survives only at the property/option
parse+report boundary (`ordinal()` out, `from_ordinal()` in). Each non-trivial
family has a discriminant-pinning unit test.

## What landed (families fully converted)

Each family below is **fully** converted (field type + every comparison/assignment
site + the DssEnum boundary) — never half-converted.

| Family | New enum | Location | Discriminants | Test |
|---|---|---|---|---|
| dynamics `SolveMode` name-collision | **`DynSolveMode`** (rename only) | `support/dynamics/mod.rs` | 0..17 (unchanged) | existing `codes_match_pascal_ordinals` |
| AutoAdd `add_type` | **`AddType`** | `circuit/auto_add.rs` | Gen=1, Cap=2 | `add_type_tests` |
| Solution `algorithm` | **`SolveAlgorithm`** | `solution/solution/state.rs` | Normal=0, Newton=1, Ncim=2 | `enum_discriminant_tests` |
| Load `status` | **`LoadStatus`** | `elements/pc/load/mod.rs` | Variable=0, Fixed=1, Exempt=2 | `load_status_pins_enum_ordinals` |
| Storage `dispatch_mode` | **`StorageDispatchMode`** | `elements/pc/storage/mod.rs` | Default=0, LoadMode=1, PriceMode=2, ExternalMode=3, Follow=4 | `storage_dispatch_mode_pins_enum_ordinals` |
| Transformer + AutoTrans `core_type` | **`CoreType`** | `elements/pd/transformer/mod.rs` | shell=0,1ph=1,3leg=3,4leg=4,5leg=5,core1ph=9 (**non-contiguous**) | `core_type_pins_noncontiguous_enum_ordinals` |
| Line + LineCode + LineGeometry `line_type`/`fline_type` | **`LineType`** | `elements/general/line_code/mod.rs` | oh=1..busbar=12 | `line_type_pins_enum_ordinals` |

### Notes / decisions

- **DynSolveMode rename**: the `support/dynamics` `SolveMode` is entirely
  self-contained (only `DynaVars.solution_mode` + its own tests reference it), so
  the rename is a pure name-collision fix vs `solution/solution/state.rs::SolveMode`.
- **Non-contiguous families** (`CoreType`) get explicit discriminants + a
  hand-written `from_ordinal` that returns `None` for the gap ordinals (2,6,7,8) —
  never a derived/`transmute` ordinal, per the plan.
- **Boundary conversion pattern**: property `get_i32` returns `field.ordinal()`;
  `set_i32` does `field = Enum::from_ordinal(value).unwrap_or(field)` (the value
  always comes from the mapped enum, so it is always in range; `.unwrap_or(field)`
  keeps the current value on the impossible out-of-range case rather than
  panicking or defaulting — behaviour-preserving).
- `AddType`/`SolveAlgorithm` exhaustive matches replaced their old `_ =>` default
  arms with the explicit final variant (the wildcard would be an
  `unreachable_patterns` clippy error once the enum is exhaustive).
- `StorageDispatchMode` / `CoreType` / `LineType` are `pub` because their struct
  fields are `pub` (a `pub(crate)` enum on a `pub` field is a privacy-leak
  warning).
- `LineType` lives in `line_code` (the most basic of the three classes) and is
  imported by `line_geometry` and `pd/line`; the old `LINETYPE_OH` sentinel const
  in `line_geometry` was removed in favour of `LineType::Oh`.

## Boundary sites touched (per family, the DssEnum / option seams)

- `AddType`: `exec/set_cmd.rs` (addtype), `exec/get_cmd.rs`, `report/save/dump/solution.rs`,
  `report/export/json/circuit.rs`, plus the `add_currents` match and the
  `exec/auto_add.rs` outcome struct + search match.
- `SolveAlgorithm`: `exec/set_cmd.rs` (algorithm, incl. the NCIM-forces-rebuild
  branch), `exec/get_cmd.rs`, `exec/view.rs`, `exec/diakoptics/solve.rs`,
  `report/{save/dump,export/json}`, the `power_flow` dispatch match + aux-current
  sign, `state.rs::converged`.
- `LoadStatus`: `load/accessors.rs` get/set, `load/nominal.rs` (Fixed/Exempt tests).
- `StorageDispatchMode`: `storage/accessors.rs` get/set, `storage/nominal.rs`
  dispatch match + Follow checks, `solution/controls/dispatch.rs`
  (`set_dispatch_external`, `all_fleet_storage`), both test mocks.
- `CoreType`: `{transformer,auto_trans}/accessors.rs` get/set only (purely
  round-tripped — no functional comparison anywhere).
- `LineType`: `{line_code,line_geometry,pd/line}` accessors + getters
  (`fline_type()`/`line_type()` now return `LineType`) + `pd/line/code.rs`
  fetch-from-code/geometry copies.

## Deferred (escape-protocol — recorded, not half-converted)

Left as raw `i32` + `pub const` chains; each is either deeply cross-cutting or
entangled with an out-of-scope channel. None were partially touched.

1. **Solution `control_mode`** (`CONTROLSOFF..MULTIRATE`) — threaded as a bare
   `i32` through the whole control subsystem: `ctx.control_mode` fields and
   `env.control_mode()` trait methods in `reg_control`, `exp_control`, `fault`,
   `controls/{actions,sampling}`, `diakoptics`. A full conversion changes those
   trait/context signatures across ~8 files. High ripple; deferred.
2. **Solution `load_model`** (`POWERFLOW/ADMITTANCE`) — flows into
   `SysCtx.load_model` (an `i32` consumed by every PC element's solve). Conversion
   requires retyping `SysCtx.load_model` + all element comparisons. Deferred.
3. **Solution `random_type`** (`GAUSSIAN/UNIFORM/LOGNORMAL`) — passed as an `i32`
   parameter into `Load::randomize`/`Fault::randomize` and the MonteCarlo drivers.
   Crosses element signatures. Deferred.
4. **InvControl** `control_mode`/`combi_mode`/RoC/pending-change/reac-power-ref
   (`inv_control/mod.rs`) — the largest if-else family; the internal comparisons
   also drive `der_set_modes`/`der_set_var_mode` env methods (shared with the DER
   `var_mode` channel). Not started; deferred as one unit.
   *(Correction, P1-tail settler 2026-07-26: this row under-counted the family — `FVoltage_CurveX_ref` and `FVoltwattYAxis`, both `DssEnum`-backed, were in no inventory. Converted with the rest; see STATUS §P1-tail settler.)*
5. **Storage `f_state` state machine** (`STORE_CHARGING/IDLING/DISCHARGING`) and
   **StorageController** discharge/charge modes + `fleet_state` — the state ordinals
   are pushed through the **generic control-queue `i32` action channel**
   (`push_immediate(code: i32)` → `ControlQueue`), which is the Tier-2 /
   out-of-scope control-action-code channel (owned by the R0 `control_elem.rs`
   worktree conceptually). Converting `f_state` alone without the queue channel
   would half-convert the family. Deferred together.
6. **DER `var_mode`** (`VARMODE_PF/KVAR`) — lives on the shared `InvBasedPceData`
   (PVSystem **and** Storage) and is set via `der_set_var_mode`/`pv_set_var_mode`
   env methods (`i32`). Cross-element + control-env ripple. Deferred.
7. **Control trio — CLOSED by DE_PASCALIZE P1b** (`wt-p1b-v2`, salvaged from the
   interrupted WIP `c842af0`): **Relay** `control_type` → `RelayControlType`
   (`#[repr(i32)]`, discriminants `0,1,3,4,5,6,7,8,9` — the `2` ordinal stays
   unused, `from_ordinal(2) = None`); **CapControl** `control_type` →
   `CapControlType` (`0..5`; USERCONTROL=6 is not registered upstream and never
   set by the port, so the `Sample` match is exhaustive); **RegControl** queue
   action codes → `RegControlAction` (`TapChange=0`/`Reverse=1`, `i32` only at the
   `ControlQueue` push/`DoPendingAction` boundary). Each ordinal proven vs Pascal
   (`Relay.pas:323-331`, `CapControl.pas:92-100`, `RegControl.pas:246-247`) **and**
   the DssEnum registry (`registry/control.rs` `relay_type`/`cap_control_type`).
   The controls-corpus manifests (105 cases) + eventlog gate stay green unchanged.
   *Still deferred* (not P1b scope): the Relay/CapControl present/normal **state**
   ordinals (the shared `CTRL_*` `EControlAction` channel — R0 `control_elem.rs`);
   **Generator** `dispatch_mode`; **PVSystem** var-mode; **ExpControl** pending;
   **ESPVLControl** `f_type`; **LoadShape** interp — left for the main P1 pass.
8. **Remaining bare-i32 DssEnum fields**: `reactor/capacitor.spec_type`,
   `vsource.{z_spec_type,scan_type,sequence_type}`, `vs_converter.f_mode`,
   `energymeter.ocp_device_type` (on the central `CktElementData`; the `== 0`
   "unset" sentinel + reliability-report ripple make it non-trivial),
   `Winding.connection` (bare 0/1 in `set_term_ref`; P10 depends on it).
9. **Shared `MonPhase` enum** (`AVGPHASES/MAXPHASE/MINPHASE` sentinel triple across
   InvControl/CapControl/RegControl/StorageController) — deferred with the four
   control families above (each still uses the raw `-1/-2/-3` consts).
10. **Tier-2 internal** — `DynamicExp` RPN token sentinels
    (`CONST_CODE`/`EQ_MARK`); InvControl internal comparisons (follow Tier 1).

Out of scope per brief: `ControlElem` action codes (R0 worktree owns
`control_elem.rs`); Monitor mode masks (WP P2 owns `monitor/`).

## Incidental (pre-existing base clippy debt, fixed to keep the gate green)

Stable rustc/clippy **1.96.0** (no `rust-toolchain` pin in-tree) flags three lints
on code **not** touched by P1 — confirmed pre-existing by `git stash` + clippy on
the base. They block `clippy -D warnings`, so they are fixed here **bit-neutrally**
(no behaviour change) to satisfy the mandatory green gate:

- `exec/diakoptics/matrices.rs:40` — `v.re != 0.0 && v.re != 0.0` (already carries
  `#[allow(clippy::eq_op)]`; the `x && x` looks deliberate). Suppressed with an
  added `clippy::nonminimal_bool` allow — **logic untouched**.
- `solution/inc_matrix.rs:190` — `!(num_terminals > 1)` → `num_terminals <= 1`.
- `elements/pc/windgen/tests.rs:356` — `vwind < 5.0 || vwind > 23.0` →
  `!(5.0..=23.0).contains(&vwind)`.
- `tests/harness/mod.rs:1318` — `!(prop_015x(..) && !oracle_names.contains(..))`
  → De Morgan `!prop_015x(..) || oracle_names.contains(..)` (shared golden
  harness; the lint blocked every `golden_*`/`corpus_live` test target).

## Gate

`cargo +stable fmt --all --check` · `cargo +stable clippy --workspace
--all-targets -- -D warnings` · `cargo +stable test --workspace` — all green in
the worktree; `tests/corpus` pristine. Numeric goldens byte-identical (no
arithmetic touched). 7 new discriminant-pin unit tests added.

## Audit settlement

Two independent auditors (audit-code, audit-tests) returned **no blockers/majors** —
only 5 `note`-severity observations, all confirming bit-neutrality. Each settled
empirically below; none required a code change (all are disclosed, verified-neutral
rewrites), so nothing was refixed. Verdicts: audit-code "faithful, bit-neutral
Stratum [A] refactor, no behavioral regressions"; audit-tests "CLEAN test-side
Stratum [A] conversion, zero golden/corpus/tolerance churn."

- **(code+tests) windgen predicate `vwind < 5.0 || vwind > 23.0` →
  `!(5.0..=23.0).contains(&vwind)`** — *rebutted.* The two forms differ only for
  NaN (old→false, new→true); identical for every finite input. The sweep is the
  fixed literal set `[4.0, 10.0, 15.0, 24.0]` (verified in `windgen/tests.rs`
  `aerodynamic_wind_speed_sweep`), so NaN can never reach the branch. Bit-neutral
  for the actual inputs; the theoretical NaN divergence is dead.
- **(code+tests) harness `compare_prop_lists` `!(A && !B) → !A || B`** — *rebutted.*
  Pure-boolean De Morgan identity (A,B are `bool`, no float/partial-order path);
  identical truth table for all inputs. Disclosed clippy `nonminimal_bool` fix.
- **(code) inc_matrix `!(num_terminals > 1) → num_terminals <= 1`** — *rebutted.*
  `num_terminals` is a `usize` count (total order, no NaN); `!(x > 1) == x <= 1`
  for all integers. Bit-neutral.
- **(code) matrices.rs `y4_keep` added `clippy::nonminimal_bool` allow** —
  *rebutted.* The reproduced upstream doubled-`.re` bug
  (`v.re != 0.0 && v.re != 0.0`) is left **textually unchanged**; only a second
  lint name joined the existing `#[allow(clippy::eq_op)]`. Zero logic change; the
  bug's unit-test pin is untouched.
- **(tests) storage_controller mock `dispatch_mode: i32 → StorageDispatchMode`** —
  *rebutted.* Part of the P1 enum conversion. Verified against base `19b9633`:
  `STORE_DEFAULT=0 == Default=0`, `STORE_EXTERNALMODE=3 == ExternalMode=3`; the
  `==`/`!=` comparisons are preserved verbatim. Bit-neutral.

The two auditors also affirmatively disproved the one structural concern they
raised themselves — that the `Enum::from_ordinal(v).unwrap_or(field)` set-boundary
could swallow out-of-range inputs the old `= value` stored: all 7 backing DssEnums
are non-hybrid, so the parse/`Set` path only ever yields in-set ordinals (or an
error both old and new code skip identically) and the `unwrap_or` fallback is
provably dead. No action needed.

## P1b audit settlement (wave 2, control trio)

Two independent auditors of the P1b range (`3c9c9dc`+`ff16d65`) reported **no
regression** — the trio conversion is faithful (both auditors independently
re-derived every discriminant from Pascal and the DssEnum registry and confirmed
agreement). Two `low` notes, each settled empirically:

- **(audit-tests) setter keep-old fallback not behaviorally exercised**
  (`from_ordinal(value).unwrap_or(self.control_type)` at `relay/accessors.rs:215`,
  `cap_control/accessors.rs:166`) — *closed by adding two purely-additive pin
  tests* (`set_i32_type_keeps_value_on_unregistered_ordinal` in relay/cap_control
  `tests.rs`): they drive `set_i32(TYP, <unregistered ordinal>)` and assert the
  prior value is kept and round-trips back through `get_i32`. The branch stays
  provably dead for real inputs (the RelayTypeEnum/CapControlType parse never
  yields the gap ordinal `2` / the unregistered `6`), so this only locks the
  deliberate keep-old intent — no behavior change, no golden/tolerance touched.
- **(audit-code) `relay_type` DssEnum omits Pascal's `RelayTypeEnum.DefaultValue
  := 0`** (`registry/control.rs:166`, `Relay.pas:352`) — *not fixed in P1b; recorded
  for a later registry-fidelity pass.* Verified out of scope and not a P1b
  regression: `git diff <range> -- registry/control.rs` is empty (the registry is
  untouched by the enum conversion), and the divergence is a **string-parse
  fallback** concern, orthogonal to P1b's `[A]` bit-neutral in-engine storage-type
  change. Effect is confined to an unreachable-in-corpus edge (an unmatched
  `Type=<garbage>` string errors in Rust vs would silently resolve to `Current=0`
  in Pascal *if* Pascal's DefaultValue applies to unmatched parses — itself
  unverified against the oracle). Touching the parse fallback here would be a
  behavior change beyond the P1b mandate and blind to the unproven Pascal
  semantics; deferred to the main-P1 / registry-fidelity pass alongside the
  `relay_action`/`relay_state` `default_value = CTRL_STATE_KEEP` family.


---

> Appended verbatim from `STATUS.md` on 2026-08-05 (STATUS.md history archiving);
> order preserved, nothing rewritten.

### DE_PASCALIZE P1-tail (5/n) — four small self-contained families + the P1-tail escape record (branch `depas-p1p3`, 2026-07-26)

Stratum **[A]** bit-neutral. Closes most of deferred item **7-residue**.

| family | new enum | discriminants (proven) |
|---|---|---|
| Generator `DispatchMode` | **`GenDispatchMode`** (`pc/generator/mod.rs`) | `Generator.pas:436-437`: Default=0, LoadLevel=1 (`LOADMODE`), Price=2 (`PRICEMODE`) → `Generator: Dispatch Mode` `[0,1,2]` |
| ESPVLControl `Ftype` | **`EspvlControlType`** (`control/espvl_control/mod.rs`) | `ESPVLControl.pas:82`: Unset=0, SystemController=1, LocalController=2 → `ESPVLControl: Type` `[1,2]` |
| LoadShape `interpolation` | **`LoadShapeInterp`** (`general/load_shape/mod.rs`) | `TLoadShapeInterp`, `LoadShape.pas:293-295`: Avg=0, Edge=1 → `LoadShape: Interpolation` `[0,1]` |
| ExpControl `FPendingChange` | **`ExpPendingChange`** (`control/exp_control/mod.rs`) | `ExpControl.pas:169-170`: None=0, ChangeVarLevel=1 (queue codes, not a `DssEnum`) |

Four pin tests, one per family. `EspvlControlType::Unset` is a **real** state, not
a placeholder: `Create` zero-inits `Ftype := 0`, which is deliberately outside the
registry's `[1, 2]` so the dump renders `''` — the variant keeps that observable.
`ExpDispatchEnv::push_change` now takes `ExpPendingChange` and `.ordinal()`s at the
`ControlQueue` seam (same pattern as `InvPendingChange` / `StorageCtrlAction`);
`set_pending_change` still writes `DblTraceParameter := Value` as
`value.ordinal() as f64`, bit-identical.

Conversions here were done with **explicit** string replacements only — see the
record 4/n note on why a bulk identifier rename is unsafe in this codebase.
Re-verified with the same mechanical string-literal diff vs `634aac9`: the only
deltas are new test-assertion messages, `"state {}" → "state {:?}"` (the field is
now `Debug`, not `Display`), `"Monte2 LOGNORMAL…" → "…LogNormal…"` (a test message)
and new doc prose. **Zero** runtime/user-visible strings changed — no `push_error`,
event-log or report text anywhere in the wave.

## P1-tail escape record — what is still open

Left as raw `i32`, deliberately, with the reason. None was partially touched.

1. **Relay / CapControl present+normal `state` ordinals** (`CTRL_NONE=0` …
   `CTRL_UNLOCK=5` + `CTRL_STATE_KEEP = i32::MIN`, `control_elem.rs:23-36`). This is
   the shared `EControlAction` channel across SwtControl / Fuse / Recloser / Relay /
   CapControl **and** the `relay_action`/`relay_state`/`fuse_*`/`recloser_*`/
   `swt_control_*` registry families, several of which map two different `DssEnum`s
   onto one field. `CTRL_STATE_KEEP = i32::MIN` is a *sentinel outside every*
   registry (the "leave as is" default value), so the enum needs a `Keep` variant
   plus the six actions, and every one of the five control classes converts in the
   same commit or the family is half-done. Out of this step's safe blast radius
   after the wave already grew to 68 files; deferred as one dedicated unit.
   **CLOSED 2026-07-26 by the W3.1 record at the top of this file** (branch
   `depas-final`): the shared `ControlAction` enum, all five classes in one
   commit, `Keep` + `Other(i32)` for the two out-of-`EControlAction` values.
2. **Item 8 — the remaining bare-`i32` `DssEnum` fields**: `reactor.spec_type` /
   `capacitor.spec_type` (a *derived* code written by six different property side
   effects, with `_` fall-throughs in three `match`es in `reactor/solve.rs`),
   `vsource.{z_spec_type, scan_type, sequence_type}`, `vs_converter.f_mode`,
   `energymeter.ocp_device_type` (the `== 0` "unset" sentinel plus the
   reliability-report ripple the P1 record already flagged).
   **CLOSED 2026-07-26 by the two W3.2 records at the top of this file**
   (branch `depas-final`): (a) `ReactorSpecType` + `CapacitorSpecType`, seven
   `_` fall-throughs eliminated; (b) the shared `ScanType`/`SequenceType` across
   **three** classes (VSource / Isource / GICLine — the pair is a shared
   `DssEnum`, so this listing's VSource-only scope understated it),
   `VsourceZSpec`, `VscMode` and `OcpDeviceType` (whose `Unset = 0` sentinel is
   a `#[default]` variant, keeping the zero-allocation semantics exactly).
3. **Item 10 (Tier-2) — `DynamicExp` RPN token sentinels** (`CONST_CODE = 50001`,
   `EQ_MARK = -50` in the token stream). These are *payload-carrying* codes (a token
   is either an opcode, a variable index, or a constant index offset by
   `CONST_CODE`), so the faithful model is a payload enum over the whole token
   stream — a real refactor of the evaluator, not a field retype. Not started.
   **CLOSED 2026-07-26 by the W3.3 record at the top of this file** (branch
   `depas-final`): `DynToken`/`DynOp`/`Lexeme`/`VarRef` in a new
   `dynamic_exp/tokens.rs`; `cmds: Vec<i32>` → `Vec<DynToken>`, both sentinels
   gone from the engine (they survive only in the `#[cfg(test)]` `ordinal()`
   encoding the unit tests pin the compiled stream against).

Everything else from `docs/phase-records/depascalize-p1.md` §Deferred is closed by
records 1/n-5/n: items **1, 2, 3, 4, 5, 6, 9** in full, item **7** except the
`CTRL_*` state channel, and the `SolveMode` name collision (already resolved by
P1 itself as `DynSolveMode`).

**Gate:** fmt · clippy `-D warnings` · `cargo test --workspace` (corpus gate, both
channels) — green; `tests/corpus` pristine; goldens untouched; `TODO(compat)` 117.

### DE_PASCALIZE P1-tail (4/n) — Storage `f_state` + StorageController modes/fleet-state + the control-queue action-code seam (branch `depas-p1p3`, 2026-07-26)

Stratum **[A]** bit-neutral. Closes deferred item **5** — deferred as one unit
because the storage state ordinals are pushed through the *generic* control-queue
`i32` action channel.

| family | new enum | discriminants (proven) |
|---|---|---|
| `TStorageObj.FState` / `.state_desired` / `StorageSnap.state` / `StorageController.fleet_state` | **`StorageState`** (`elements/pc/storage/mod.rs`) | `Storage.pas:35-37`: Charging=-1, Idling=0, Discharging=1, **+ `Other(i32)`** → `Storage: State` `DssEnum` `[-1,0,1]` |
| `DischargeMode` / `ChargeMode` | **`StorageCtrlMode`** (`storage_controller/mod.rs`) | `StorageController.pas:268-276`: Follow=1, LoadShape=2, Support=3, Time=4, PeakShave=5, Schedule=6, PeakShaveLow=7, CurrentPeakShave=8, CurrentPeakShaveLow=9 |
| `RELEASE_INHIBIT` queue code | **`StorageCtrlAction`** | `StorageController.pas:279`: ReleaseInhibit=999 |

**Why `StorageState` carries a payload.** Upstream declares `FState: Integer`
(`Storage.pas:269`) and `TStorageObj.Set_Variable`'s state channel writes
`Fstate := Trunc(Value)` **unguarded** (`Storage.pas:3135`) — a script can put any
integer in the field. A closed 3-variant enum would silently drop those, so
`StorageState::Other(i32)` keeps `ordinal()`/`from_ordinal` total and mutually
inverse (the `MonPhase` precedent from record 2/n). Pinned by three tests:
`storage_state_pins_enum_ordinals`,
`storage_state_round_trips_every_out_of_set_ordinal`, and a **behavioral**
`set_variable_state_stores_out_of_set_values_verbatim` (drives
`set_variable(2, 7.9)` and asserts `Other(7)` reads back as `7`).

**One enum for both mode fields.** `ModeDischarge=` and `ModeCharge=` are two
separate `DssEnum`s over one shared Pascal ordinal space (`[5,1,3,2,4,6,8]` and
`[2,4,7,9]`), so a single `StorageCtrlMode` covers both fields and each `Sample`
arm names the ordinals *its* mode rejects — the pre-enum `_ => push_error("Invalid
DisCharging/Charging Mode: {}")` arms become the explicit complements
(`PeakShaveLow | CurrentPeakShaveLow` for discharge; `Follow | Support | PeakShave
| Schedule | CurrentPeakShave` for charge), with the message still formatting the
raw `.ordinal()` so the diagnostic text is byte-identical.
`storage_ctrl_mode_and_action_pin_pascal_ordinals` asserts every value of *both*
registry lists resolves.

**The control-queue seam (the item-5 rider).** `ControlQueue`'s `code: i32` stays
`i32` — it is genuinely class-polymorphic — and each class converts at its own
push/pop boundary (the P1b `RegControlAction` precedent):
`StorageDispatchEnv::push_immediate(StorageState)` (upstream really does push the
storage-state ordinal as the immediate re-solve marker) and
`push_release_inhibit` emit `.ordinal()`; `do_pending_action(code: i32)` keeps the
raw popped code and tests `StorageCtrlAction::from_ordinal(code) ==
Some(ReleaseInhibit)`, so the `StorageState` markers on the same queue are ignored
exactly as before. `set_state`/`set_state_desired`/`der_storage_state` are now
`StorageState`-typed.

**Two `TODO(compat)` sites preserved verbatim.** `DoLoadFollowMode` /
`DoPeakShaveModeLow` reproduce Pascal's `if not FleetState = STORE_IDLING` operator-
precedence bug (bitwise-NOT of an integer, so it fires only for `STORE_CHARGING`).
They now read `(!self.fleet_state.ordinal()) == StorageState::Idling.ordinal()` —
the same integer arithmetic on the same values, tag and comment untouched.
`TODO(compat)` count still **117**.

### Corpus-gate catch, recorded (why this record exists at all)

The **first** full-gate run of the InvControl wave (record 3/n) came back with
**10 failing `controls:invcontrol/*` cases**, every one an *event-log text*
mismatch (e.g. `Action=INVCONTROLMODE::VOLTVAR MODE REQUESTED…` vs
`Action=VOLTVAR MODE REQUESTED…`). Cause: the bulk identifier rename that
converted the constants also rewrote the **string literals** of the Pascal
`AppendToEventLog` messages (20 literals in `inv_control/compute.rs`, 2 in its
tests). Fixed by restoring every literal, then re-verified with a mechanical
string-literal diff of **all** changed files against the wave base `634aac9` — the
multiset of literals per file is now identical to the base everywhere. Five
Pascal-citing *comments* (`if FState <> STORE_DISCHARGING`, `STORE_CHARGING = -1`,
the CIM `BatteryStateKind` doc) were restored to their Pascal spelling for the
same reason.

Nothing shipped with the corruption: the failure was found by the gate before any
commit, and both waves are committed only after a clean run.

**Gate:** fmt · clippy `-D warnings` · `cargo test --workspace` (corpus gate,
both channels) — green; `tests/corpus` pristine; goldens untouched.

### DE_PASCALIZE P1-tail (3/n) — the InvControl family (6 enums) + the shared DER `VarMode` (branch `depas-p1p3`, 2026-07-26)

Stratum **[A]** bit-neutral. Closes deferred items **4** and **6** — the P1 record's
"largest if-else family", deferred as one unit precisely because its internal
comparisons drive the `der_set_modes`/`der_set_var_mode` env channel shared with the
DER var-mode field.

| family | new enum | discriminants (Pascal → registry) |
|---|---|---|
| `ControlMode` (`Mode=`) | **`InvControlMode`** | `TInvControlControlMode`, `InvControl.pas:119-128` (`{$Z4}` int32): NoneMode=0, VoltVar=1, VoltWatt=2, Drc=3, WattPf=4, WattVar=5, Avr=6, Gfm=7 → `InvControl: Control Mode` `[1..7]` |
| `CombiMode` | **`InvCombiMode`** | `:131-135`: NoneCombMode=0, VvVw=1, VvDrc=2 → `InvControl: Combi Mode` `[1,2]` |
| `RateofChangeMode` | **`RateOfChangeMode`** | `ERateofChangeMode`, `:143-147`: Inactive=0, Lpf=1, RiseFall=2 → `InvControl: Rate-of-change Mode` `[0,1,2]` |
| `FPendingChange` | **`InvPendingChange`** | `:407-411`: None=0, ChangeVarLevel=1, ChangeWattLevel=2, ChangeWattVarLevel=3, ChangeDrcVVarLevel=4 (queue action codes — not a `DssEnum`) |
| `FReacPower_ref` | **`ReacPowerRef`** | `:404-405`: VarAval=0, VarMax=1 → `InvControl: Reactive Power Reference` `[0,1]` |
| `CtrlModel` | **`InvControlModel`** | `TInvControlModel`, `:137-140`: Linear=0, Exponential=1 → `InvControl: Control Model` `[0,1]` |
| DER `varMode` (`InvBasedPceData`) | **`VarMode`** (`elements/pc/inv_based_pce.rs`) | `PVsystem.pas:32-33`: Pf=0, Kvar=1 (the identical pair in `Storage.pas`) |

Two pin tests: `invcontrol_enums_pin_pascal_and_registry_ordinals` (all six, each
ordinal + `from_ordinal` inverse + the out-of-range `None` + the `Create` defaults)
and `var_mode_pins_pascal_ordinals`.

**Channel changes (the reason items 4+6 had to land together).**
`InvDispatchEnv::der_set_modes(.., var_mode: VarMode)` and `der_set_var_mode(_,
VarMode)`; `ExpDispatchEnv::pv_set_var_mode(_, VarMode)`;
`InvDispatchEnv::push_change(delay, InvPendingChange)`. The **generic
`ControlQueue` action-code channel stays `i32`** — it is genuinely
class-polymorphic — so the conversion happens at each class's own push/pop seam,
exactly the P1b `RegControlAction` precedent: `push_change`'s impl in
`solution/controls/dispatch.rs` calls `code.ordinal()` into `queue.push_delay`.
The `pub(crate) const VARMODE_PF/VARMODE_KVAR` pairs in **both** `pvsystem/mod.rs`
and `storage/mod.rs` are gone (they were duplicate declarations of the same
Pascal constant).

**One structural simplification, disclosed.** `Sample`'s mode gate was

```
if combi != NONE_COMBMODE { if combi != VV_VW && combi != VV_DRC { Err(not_ported) } }
else { match control_mode { <all 8 ordinals> => {} , _ => Err(not_ported) } }
```

The `_ =>` arm of that inner `match` listed **every** `TInvControlControlMode`
value, so it was already unreachable for any in-range ordinal; with the closed
enum it is `unreachable_patterns` (a clippy `-D warnings` error). It is folded to
the single combi test, and a comment records that every control mode is ported.
`not_ported_mode()`'s message still formats the raw ordinals (`.ordinal()`), so
the error text is byte-identical.

`validate_xy_curve(.., mode: InvControlMode)`'s `_ => {}` likewise became the
explicit five non-checked variants; `InvPendingChange` is `pub` (like
`RegControlAction`) because it appears on the `pub f_pending_change` field.

CIM boundary (`cim/ieee1547.rs`, `cim/export.rs`) keeps its `i32` snapshot
fields and takes `.ordinal()` — no writer change.

**Gate:** fmt · clippy `-D warnings` · `cargo test --workspace` (corpus gate
included) — green; `tests/corpus` pristine; goldens untouched; `TODO(compat)` 117.

### DE_PASCALIZE P1-tail (2/n) — the shared `MonPhase` hybrid enum kills four sentinel-triple copies (branch `depas-p1p3`, 2026-07-26)

Stratum **[A]** bit-neutral. Closes deferred item **9**. Four control classes each
carried a private copy of the *same* `AVGPHASES=-1 / MAXPHASE=-2 / MINPHASE=-3`
sentinel triple; all four resolve to the one `MonPhaseEnum`
(`obj/dss_enum/registry/control.rs`, **`hybrid = true`**, values `[-3,-2,-1]`).

New module `elements/control/mon_phase.rs` → `pub enum MonPhase { Avg, Max, Min,
Phase(i32) }`, re-exported as `elements::control::MonPhase`. Discriminants proven
against `CapControl.pas:230-232` **and** `StorageController.pas:38-40` (identical
triple) and against the registry values.

**Why a payload variant.** The backing `DssEnum` is `hybrid`: a token that does
not match `min`/`max`/`avg` is parsed as an *integer* and stored verbatim (a
1-based phase number). A closed 3-variant enum would silently drop those, so
`MonPhase::Phase(i32)` carries the raw ordinal and `ordinal()`/`from_ordinal()`
are **total and mutually inverse over all of `i32`** — the property boundary
round-trips byte-for-byte exactly as the pre-enum bare field did. Two pin tests:
`mon_phase_pins_enum_ordinals` (the three sentinels) and
`mon_phase_round_trips_every_non_sentinel_ordinal` (totality, incl. `0`, `-4`,
`±1000`, and that *only* `-1/-2/-3` are non-`Phase`).

| field retyped | file | old private consts removed |
|---|---|---|
| `CapControl.fct_phase` / `.fpt_phase` | `cap_control/{mod,accessors,control_loop,tests}.rs` | `AVGPHASES`/`MAXPHASE`/`MINPHASE` |
| `RegControl.fpt_phase` | `reg_control/{mod,accessors,control_loop}.rs` | `MAXPHASE`/`MINPHASE` |
| `InvControl.mon_buses_phase` | `inv_control/{mod,accessors,compute,tests}.rs` | `AVGPHASES`/`MAXPHASE`/`MINPHASE` |
| `StorageController.f_mon_phase` (+ `StorageDispatchEnv::control_power`/`control_current` signatures) | `storage_controller/{mod,accessors,tests}.rs`, `solution/controls/dispatch.rs` | `AVG`/`MAXPHASE`/`MINPHASE` |

**The one semantic subtlety, handled explicitly.** RegControl's own registry
entry (`RegControl: Phase Selection`) has **no `avg`** — only `min`/`max` + the
integer fallback — so its `get_control_voltage` `_ =>` arm used to absorb an
`Avg` (-1) ordinal as `(-1-1).max(0) = 0` (phase 0). The converted match keeps
that exactly: the specific-phase arm is `MonPhase::Avg | MonPhase::Phase(_)` and
still computes `(self.fpt_phase.ordinal() - 1).max(0)`. Every other converted
match is a straight 1:1 arm rename, and the two 1-based indexers
(`cbuffer[(p as usize) - 1]`, `cd.iterminal[(p - 1) as usize]`) now bind the
payload instead of casting the field — identical arithmetic, plus the upstream
0-based-`cBuffer` MonBus quirk left verbatim (`cbuffer.get(p as usize)`).

Validation writes (`if f_*_phase.ordinal() > nphases { … = MonPhase::Phase(1) }`)
are unchanged in effect: the sentinels are negative, so they never trip the
bound, exactly as before.

**Gate:** fmt · clippy `-D warnings` · `cargo test --workspace` (corpus gate
included) — green; `tests/corpus` pristine; goldens untouched; `TODO(compat)` 117.

### DE_PASCALIZE P1-tail (1/n) — the solution enum trio: `ControlMode` / `LoadSolutionModel` / `RandomType` (branch `depas-p1p3`, 2026-07-26)

Stratum **[A]** bit-neutral. Base = `update` @ `634aac9`. Ritual 0 held (186 `.pas`
under `.inputs/dss_capi`; `cargo` = the rustup MSVC `.cargo\bin` one). Closes items
**1-3** of the `docs/phase-records/depascalize-p1.md` §Deferred list — the three
`Solution` fields that were bare `i32` const-chains threaded through the control
subsystem, every PC element's `SysCtx`, and the MonteCarlo drivers.

| Deferred item | New enum | Location | Discriminants (proven) | Pin test |
|---|---|---|---|---|
| 1 Solution `control_mode`/`default_control_mode` | **`ControlMode`** | `solution/solution/state.rs` | ControlsOff=-1, Static=0, EventDriven=1, TimeDriven=2, MultiRate=3 | `control_mode_pins_enum_ordinals` |
| 2 Solution `load_model`/`default_load_model` + `SysCtx.load_model` | **`LoadSolutionModel`** | same | PowerFlow=1, Admittance=2 | `load_solution_model_pins_enum_ordinals` |
| 3 Solution `random_type` | **`RandomType`** | same | None=0, Gaussian=1, Uniform=2, LogNormal=3 | `random_type_pins_enum_ordinals` |

Every discriminant proven **twice**: against the Pascal (`DSSGlobals.pas:99-103`
control modes, `:90-91` load model, `:106-108` random) **and** against the `DssEnum`
registry values in `obj/dss_enum/registry/solution.rs` (`Control Mode` `[-1,0,1,2,3]`,
`Load Solution Model` `[1,2]`, `Random Type` `[0,1,2,3]`). The `ADMITTANCE` /
`POWERFLOW` / `GAUSSIAN` / `UNIFORM` / `LOGNORMAL` / `CONTROLSOFF` / `CTRLSTATIC` /
`EVENTDRIVEN` / `TIMEDRIVEN` / `MULTIRATE` `pub const`s are **gone**; `i32` now
survives only at the `Set`/`Get`/dump/JSON boundary (`ordinal()` out,
`from_ordinal().unwrap_or(current)` in — the P1 boundary pattern).

**Naming decision.** The solution-side load model is `LoadSolutionModel`, not
`LoadModel`: the Load element already owns a `LoadModel` enum (`Model=`
ConstPQ/ConstZ/…). The registry's own label is literally "Load Solution Model".

**Signature ripples (all bit-neutral; exhaustive matches replace the old `_ =>`):**
`Load::randomize(RandomType, …)` / `Fault::randomize(RandomType, …)` /
`draw_load_multiplier(…, RandomType, …)` / `randomize_all_loads`; `CtrlCtx.control_mode`,
the `ExpDispatchEnv::control_mode() -> ControlMode` trait method + `ExpDispEnv` impl,
`FaultStatusCtx.control_mode`; `SysCtx.load_model`. Wildcard arms that previously
swallowed the unlisted ordinals became explicit final variants
(`ControlMode::ControlsOff => {}` in `do_control_actions`,
`RandomType::None | RandomType::LogNormal => {}` in `draw_load_multiplier`,
`ControlMode::Static | ControlMode::ControlsOff => return false` in
`Fault::check_status`) — the swallowed set is identical because each enum is closed
over exactly the old const set.

**Fenced-file ripples (for the merge coordinator).** The retype forced a mechanical
one-token `.ordinal()` at the enum→`ordinal_to_string` boundary in two files the
sibling `depas-og2` worktree owns: `exec/report.rs:1367` (1 line) and
`report/export/json/circuit.rs` (3 lines). No logic touched in either.

**Test-side note (disclosed).** `exp_control/tests.rs` carried a local
`const TIMEDRIVEN: i32 = 1` — mislabeled (1 is EVENTDRIVEN). It is replaced by
`ControlMode::TimeDriven` (2). Behaviorally identical: every ExpControl comparison is
`== / != CTRLSTATIC` and both ordinals are non-static; the const is removed with a
comment recording the correction.

**Gate:** `cargo fmt --all --check` · `clippy --workspace --all-targets -D warnings` ·
`cargo test --workspace` (unified corpus gate included) — all green; `tests/corpus`
pristine (run artifacts removed by exact name); goldens untouched; `TODO(compat)`
still **117**.
