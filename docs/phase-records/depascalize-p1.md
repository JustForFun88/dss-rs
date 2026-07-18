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
