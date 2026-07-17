# DE_PASCALIZE Part I — Stage R0: behavior traits on current `Box<dyn>` storage

Stratum **[A]** (bit-neutral). No storage change: `Vec<Box<dyn DssObject>>` stays.
Every golden/test unchanged. Element storage refactor is R1/R2.

## Metric — downcast sites (`rg "downcast_ref|downcast_mut|as_any" crates/dss-core/src | wc -l`)

- **Before:** 766
- **After:** 699
- **Removed: 67**

`dispatch.rs` alone: 187 → 169 (−18).

## What landed

### Part 1 — `ControlElem` trait (Category A, identification + generic-controlled reset)

- New `ControlElem` trait + `ControlClass` enum in
  `elements/control/control_elem.rs`: `ccd()`/`ccd_mut()`, `control_kind()`,
  `reset_control_side()` (default `unreachable!`, overridden by Swt/Recloser/Relay).
- New `DssObject::as_control()`/`as_control_mut()` (default `None`), overridden by
  all 12 control classes (Reg/Cap/Swt/Fuse/Recloser/Relay/GenDispatcher/
  StorageController/Inv/Exp/Upfc/Espvl) in their `accessors.rs`.
- `solution/controls/dispatch.rs`: the ~12-arm `as_any().downcast_ref::<…>()`
  identification chain → one `obj.as_control()?` peek + `control_kind()` match that
  builds the existing `ControlKind` mirror from `ccd()`. Error-message prefixes kept
  byte-identical via `ControlClass::display_name()` (`GenDispatcher`/`UPFCControl`/
  `ESPVLControl` preserved).
- ESPVL fleet filter `is::<EspvlControl>()` → `as_control()` + `control_kind()`.
- The 3 `Reset`-with-no-controlled-element arms (Swt/Recloser/Relay) → `as_control_mut()`
  + trait `reset_control_side()`.

**Deferred to R2 (recorded, not attempted):** the per-control behavioral
`sample()`/`do_pending_action()`/`reset_with()` downcasts of the generic-controlled
controls (Swt/Fuse/Recloser/Relay) and the fleet-clone downcasts
(Gen/Storage/Inv/Exp/Upfc/Espvl), plus the Reg→Transformer/AutoTrans and
Cap→Capacitor concrete borrows. These have per-control heterogeneous signatures;
collapsing them needs typed-arena pair/triple access (R2), not a behavior trait —
matches the plan's "dispatch is reduced in R0, emptied in R2".

### Part 2 — `ElemStore::kind(r) -> ElemKind` (Category B type-guards)

- New `ElemStore::kind()` (returns `DssClass.kind`, panics on a general class ref);
  impl in `exec/registry.rs` and the two test mocks (`ymatrix.rs`, `monte_carlo.rs`).
- `meters/zones/build.rs`: `is_line`/`is_zone_pce`/`is_pd_element` → `matches!(store.kind(r), …)`.
  WindGen and GIC PD types stay excluded exactly as the old concrete-downcast probes had them.
- `sampling/take_sample.rs`: `branch_kind` (is_line/is_xfmr) and the shunt
  `is_load`/`is_gen` guards → `matches!(store.kind(…), …)`.

**Not applicable / deferred:** `sampling/allocate.rs` holds only concrete reads over
homogeneous class lists + a Sensor/EnergyMeter polymorphic field read (no PD/PCE
boolean guard) → R2. `energymeter/accessors.rs::capture_metered` and
`monitor/accessors.rs::recalc` classify a **bare `&dyn DssObject`** (no `ElemRef`/store
in scope), so `store.kind` cannot apply; they wait for the arena work (R1/R2).

### Part 3 — `ConductorData` trait (Category C, WireData/CNData/TSData polymorphism)

- New `ConductorData` trait + `ConductorKind` enum in
  `elements/general/conductor_data/mod.rs`: `geom()`, `amps()`, `conductor_kind()`.
  Impl'd for `WireDataObj`/`CnDataObj`/`TsDataObj` (delegating to their inherent methods).
- New `DssObject::as_conductor()` (default `None`), overridden in the 3 concrete classes.
- Rewrote the `is T…DataObj` downcast dispatch chains via `as_conductor()`:
  `conductor_data/mod.rs::conductor_geom`, `line/code.rs` (choice_of + amps),
  `line_geometry/edit.rs` (choice_of + amps), `line_geometry/matrix.rs` (choice loop +
  norm_emerg), `line/save.rs` + `line_geometry/save.rs` (`conductor_kind` label).

**Deferred:** retyping the `Line.line_wire_data` / `fwiredata` **storage** from
`Box<dyn DssObject>` to the trait/enum removes no further downcasts (the helpers already
read through `as_conductor()`) and is a storage-shape change better carried by the arena
work; recorded for R1/R2. The `make_like` downcasts in wire/cn/ts `accessors.rs` are
Category E (R2).

### Part 4 — small typed `Option` reads on `CktElement`

- `line_length_km() -> Option<f64>` (Line override), `load_num_customers() -> Option<i32>`
  (Load override), `present_tap(terminal) -> Option<f64>` (Transformer + AutoTrans overrides).
- `meters/zones/build.rs`: active-branch line length + `active_is_line` (one `line_length_km()`
  read, `.is_some()`/`.unwrap_or(0.0)`) and the zone-load `num_customers` guard.
- `monitor/sample.rs` mode 2 (tap monitor): xfmr/autotrans `present_tap` downcast pair →
  `as_ckt_element().and_then(|e| e.present_tap(w))`.

**Not done (out of the named scope):** monitor modes 6/7 (cap `states()`, storage state)
and the "generator power-by-phase" accumulation in `take_sample.rs` are concrete reads
over homogeneous lists / niche per-type state, better folded into R2 typed arenas; the
plan named only present_tap for the monitor read.

## Escape-protocol / deferral summary (all left gate-green, for R2)

1. Generic-controlled + fleet behavioral downcasts in `dispatch.rs` (heterogeneous
   signatures → typed-arena pair/triple access).
2. `allocate.rs`, `take_sample.rs` DER loops: concrete reads over homogeneous class lists.
3. `capture_metered` / monitor `recalc`: kind-classify a bare `&dyn DssObject` (no ref/store).
4. `Line.line_wire_data`/`fwiredata` storage retype: no downcast payoff at R0; arena work.
5. `make_like` downcasts (Category E) throughout.

## Baseline clippy (separate commit, NOT R0 downcast work)

Stable toolchain drifted to 1.96.0 (2026-05-25), whose newer lints (`nonminimal_bool`,
`manual_range_contains`) fire on **pre-existing** sites the `update` base predates —
proven pre-existing by stashing all R0 changes and re-running clippy. Fixed
behavior-neutrally so the gate is green:

- `exec/diakoptics/matrices.rs`: extended `#[allow(clippy::eq_op)]` →
  `+ clippy::nonminimal_bool` (preserves the deliberately-reproduced doubled-`.re`
  upstream bug **verbatim**; no code change).
- `solution/inc_matrix.rs`: `!(n > 1)` → `n <= 1`.
- `elements/pc/windgen/tests.rs`: `vwind < 5.0 || vwind > 23.0` → `!(5.0..=23.0).contains(&vwind)`.
- `tests/harness/mod.rs`: De Morgan on a filter predicate (clippy's exact rewrite).

## Gate

- `cargo +stable fmt --all --check` — clean.
- `cargo +stable clippy --workspace --all-targets -- -D warnings` — clean.
- `cargo +stable test --workspace` — see final report.
- `tests/corpus` — pristine.
