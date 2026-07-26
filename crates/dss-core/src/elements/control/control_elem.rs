//! Port of `Controls/ControlElem.pas` — `TControlElem`, the shared base of
//! every control device (RegControl, CapControl, ...). A control element is a
//! `TDSSCktElement` with **no** Yprim: it monitors one circuit element and acts
//! on another (often via the control queue), but it never stamps admittance and
//! its terminal currents are always zero.
//!
//! Phase 4 ports the *parse-time* surface only: the property state, the
//! reference resolution, and `RecalcElementData`'s bus/phase setup (so the
//! control participates in `ProcessBusDefs` without changing node order). The
//! behavioral `Sample`/`DoPendingAction` machinery and the control queue arrive
//! in Phase 5 (PHASE4_PLAN §5).

use num_complex::Complex64;

use crate::elements::ckt::CktElementData;
use crate::elements::traits::{ElemId, SysCtx};
use crate::solution::{ControlMode, ControlQueue, EventLog};

/// Pascal `EControlAction` (`Controls/ControlElem.pas:20-29`, declared under
/// `{$Z4}` so the ordinals are int32 `CTRL_NONE = 0` … `CTRL_TAPDOWN = 7`) —
/// the control-action **and** switch-state channel shared by SwtControl, Fuse,
/// Recloser, Relay and CapControl. It is simultaneously:
///
/// - a **field** type (`FPresentState` / `FNormalState` / `CurrentAction` /
///   `LockCommand` / `FPendingChange`),
/// - the **queue** action code those five classes push and pop (`ControlQueue`
///   speaks a class-polymorphic `i32`, so each class converts at its own
///   push/`DoPendingAction` boundary — the P1b `RegControlAction` precedent),
/// - and the value space of eight `DssEnum` registry entries (SwtControl /
///   Fuse / Recloser / Relay × `Action`, `State`), all of which declare exactly
///   `[2, 1]` = `Close`, `Open` (`Recloser`'s pair carries a third `trip`
///   spelling that also maps to `1`).
///
/// `CTRL_TAPUP`/`CTRL_TAPDOWN` are declared upstream but referenced nowhere in
/// the Pascal tree (RegControl runs its own `ACTION_TAPCHANGE`/`ACTION_REVERSE`
/// codes, `RegControl.pas:246-247`); they are modeled because they are part of
/// the Pascal type.
///
/// Two values sit **outside** `EControlAction`, both reproduced by a variant so
/// that [`Self::ordinal`] / [`Self::from_ordinal`] stay total and mutually
/// inverse (the `StorageState` precedent):
///
/// - [`Self::Keep`] = `i32::MIN` — the sentinel the Relay `Action`/`Normal`/
///   `State` `DssEnum`s carry as `default_value`, for a token whose first
///   character is neither `o` nor `c` (Pascal `InterpretRelayState`,
///   `Relay.pas` r4133: the `case LowerCase(param)[1]` has only `'o'`/`'c'`
///   arms, so `trip`, `xyz`, … leave the phase's state slot *unchanged*,
///   silently). It is never stored in a state array and never rendered.
/// - [`Self::Other`] — any other integer. The channel is open on one path: a
///   CapControl `USERCONTROL` guest schedules its own action code through
///   `control_queue_push` (upstream `USER_BASE_ACTION_CODE = 100`,
///   `ControlElem.pas:67`) and `DoPendingAction` assigns it straight into
///   `FPendingChange`, which is also mirrored into `DblTraceParameter`. A
///   closed enum would silently drop those.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlAction {
    /// `CTRL_NONE = 0`.
    None,
    /// `CTRL_OPEN = 1`.
    Open,
    /// `CTRL_CLOSE = 2`.
    Close,
    /// `CTRL_RESET = 3`.
    Reset,
    /// `CTRL_LOCK = 4`.
    Lock,
    /// `CTRL_UNLOCK = 5`.
    Unlock,
    /// `CTRL_TAPUP = 6` (declared upstream, never referenced).
    TapUp,
    /// `CTRL_TAPDOWN = 7` (declared upstream, never referenced).
    TapDown,
    /// Not an `EControlAction` value: the `i32::MIN` "leave this phase as is"
    /// sentinel of the Relay state enums (see the type doc).
    Keep,
    /// Any other integer arriving through the open control-queue code channel.
    Other(i32),
}

impl ControlAction {
    /// The raw `EControlAction` ordinal (the `ControlQueue` code, the `DssEnum`
    /// property value, the `DblTraceParameter` mirror).
    pub fn ordinal(self) -> i32 {
        match self {
            Self::None => 0,
            Self::Open => 1,
            Self::Close => 2,
            Self::Reset => 3,
            Self::Lock => 4,
            Self::Unlock => 5,
            Self::TapUp => 6,
            Self::TapDown => 7,
            Self::Keep => i32::MIN,
            Self::Other(n) => n,
        }
    }

    /// From a raw ordinal. Total: `from_ordinal(x).ordinal() == x` for every
    /// `x` (the `StorageState` precedent — the queue-code channel is open).
    pub fn from_ordinal(value: i32) -> Self {
        match value {
            0 => Self::None,
            1 => Self::Open,
            2 => Self::Close,
            3 => Self::Reset,
            4 => Self::Lock,
            5 => Self::Unlock,
            6 => Self::TapUp,
            7 => Self::TapDown,
            i32::MIN => Self::Keep,
            n => Self::Other(n),
        }
    }
}

/// Scalar/queue/event context handed to a control's `Sample` and
/// `DoPendingAction` (PHASE5_PLAN §2.1) — the disjoint-borrow stand-in for the
/// Pascal `ActiveCircuit.Solution.*` / `ActiveCircuit.ControlQueue` /
/// `DSS.EventStrings` global reach. The *controlled/monitored* circuit elements
/// are passed to each control's method separately (their concrete types differ
/// per control), so this context carries only the shared state.
pub struct CtrlCtx<'a> {
    /// `Solution.NodeV` (slot 0 = ground).
    pub node_v: &'a [Complex64],
    /// Snapshot of the circuit/solution scalars.
    pub sys: &'a SysCtx,
    /// `ckt.ControlQueue`.
    pub queue: &'a mut ControlQueue,
    /// `DSS.EventStrings`.
    pub events: &'a mut EventLog,
    /// `DoSimpleMsg` sink.
    pub errors: &'a mut crate::diag::ErrorLog,
    /// Raised when an action invalidates Y (tap change / capacitor step); the
    /// control loop copies it into `Solution.SystemYChanged`.
    pub system_y_changed: &'a mut bool,
    /// `Solution.ControlMode` (Static / EventDriven / TimeDriven / MultiRate).
    pub control_mode: ControlMode,
    /// `Solution.DynaVars.intHour` / `.t` / `.dblHour`.
    pub int_hour: i32,
    pub t: f64,
    pub dbl_hour: f64,
    /// `Solution.ControlIteration` (event-log field).
    pub control_iter: i32,
    /// This control's own [`ElemId`] (Pascal passes `Self` to `ControlQueue.Push`).
    pub self_ref: ElemId,
}

/// `TControlElem` shared state (the base-class fields every control carries).
/// Embeds [`CktElementData`] exactly as `TControlElem` extends
/// `TDSSCktElement`.
#[derive(Debug, Clone)]
pub struct ControlElemData {
    pub cd: CktElementData,
    /// `ElementTerminal`: 1-based terminal of the monitored/controlled element.
    pub element_terminal: i32,
    /// `FControlledElement` (the device this control acts on).
    pub controlled_element: Option<ElemId>,
    /// `FMonitoredElement` (the device this control samples).
    pub monitored_element: Option<ElemId>,
    /// `TimeDelay` (s) and `DblTraceParameter` (debug-trace scratch).
    pub time_delay: f64,
    pub dbl_trace_param: f64,
    /// `ShowEventLog` (defaults to `DSS.EventLogDefault`, which is `False`).
    pub show_event_log: bool,
}

/// Which concrete control class a control object is — the behavior-trait
/// replacement for the removed `Any`-downcast identification chain in
/// `solution/controls/dispatch.rs`. Returned by [`ControlElem::control_kind`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlClass {
    Reg,
    Cap,
    Swt,
    Fuse,
    Recloser,
    Relay,
    GenDispatch,
    StorageCtrl,
    Inv,
    Exp,
    Upfc,
    Espvl,
}

impl ControlClass {
    /// The display name the dispatcher prints in `Error Sampling Control
    /// Device "<name>"` diagnostics — byte-identical to the old per-arm
    /// `format!("RegControl.{…}", …)` prefixes (note `UPFCControl` /
    /// `ESPVLControl` / `GenDispatcher` differ from the bare class idents).
    pub fn display_name(self) -> &'static str {
        match self {
            ControlClass::Reg => "RegControl",
            ControlClass::Cap => "CapControl",
            ControlClass::Swt => "SwtControl",
            ControlClass::Fuse => "Fuse",
            ControlClass::Recloser => "Recloser",
            ControlClass::Relay => "Relay",
            ControlClass::GenDispatch => "GenDispatcher",
            ControlClass::StorageCtrl => "StorageController",
            ControlClass::Inv => "InvControl",
            ControlClass::Exp => "ExpControl",
            ControlClass::Upfc => "UPFCControl",
            ControlClass::Espvl => "ESPVLControl",
        }
    }
}

/// The shared behavior surface of every control element (`TControlElem`): its
/// [`ControlElemData`] base state, its concrete [`ControlClass`], and the
/// control-only `Reset`. Acquired from a [`DssObject`] via
/// `as_control()`/`as_control_mut()`, this replaces the `dispatch.rs`
/// identification downcast chain (R0); the per-control `Sample`/`Action`
/// behavioral entry points keep their concrete signatures until R2 hands
/// dispatch typed-arena pair access.
///
/// [`DssObject`]: crate::obj::base::DssObject
pub trait ControlElem {
    /// The `TControlElem` base state (controlled/monitored refs, terminal, …).
    fn ccd(&self) -> &ControlElemData;
    fn ccd_mut(&mut self) -> &mut ControlElemData;
    /// Which concrete control class this is.
    fn control_kind(&self) -> ControlClass;
    /// Reset only the control's own state — the `Reset` path taken when the
    /// controlled element is unset (Pascal `Reset` with no controlled handle).
    /// Only the generic-controlled protection controls (SwtControl / Recloser /
    /// Relay) reach this arm; the default is unreachable for the rest.
    fn reset_control_side(&mut self) {
        unreachable!("reset_control_side: control has no None-controlled reset path");
    }
}

impl ControlElemData {
    /// Pascal `TControlElem.Create`: zero delay/trace, no event log, no
    /// controlled element yet.
    pub fn new(name: &str, num_props: usize) -> Self {
        Self {
            cd: CktElementData::new(name, num_props),
            element_terminal: 1,
            controlled_element: None,
            monitored_element: None,
            time_delay: 0.0,
            dbl_trace_param: 0.0,
            show_event_log: false,
        }
    }
}

/// A read-only snapshot of a controlled/monitored circuit element, captured
/// when the `transformer=`/`element=`/`capacitor=` reference is resolved during
/// parsing. Pascal reads the live element through a pointer in
/// `RecalcElementData`; here that runs in `EndEdit`, after the foreign-class
/// view has gone out of scope, so the control caches what it needs at
/// resolution time. Nothing edits the referenced element between resolution and
/// `EndEdit` in practice (the IEEE masters always define the target fully
/// before the control), so the cache matches a live read; editing the target's
/// shape *afterwards* leaves the control stale in Pascal too (its bus string is
/// only refreshed by its own next `RecalcElementData`).
#[derive(Debug, Clone, Default)]
pub struct RefSnapshot {
    /// `FullName` (`Class.name`) for error messages and dumps.
    pub full_name: String,
    pub nphases: usize,
    pub nterms: usize,
    /// 1-based terminal bus names, stored 0-based (`buses[term - 1]`).
    pub buses: Vec<String>,
}

impl RefSnapshot {
    /// Capture the parse-relevant shape of a referenced circuit element.
    pub fn capture(full_name: String, elem: &dyn crate::elements::traits::CktElement) -> Self {
        let cd = elem.cd();
        Self {
            full_name,
            nphases: cd.nphases,
            nterms: cd.nterms,
            buses: (1..=cd.nterms).map(|i| cd.get_bus(i).to_string()).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ControlAction;
    use crate::obj::dss_enum::EnumRegistry;

    /// `ControlElem.pas:20-29` — `EControlAction` in declaration order under
    /// `{$Z4}` (int32 ordinals, no explicit values, so 0..7).
    #[test]
    fn control_action_pins_pascal_ordinals() {
        assert_eq!(ControlAction::None.ordinal(), 0);
        assert_eq!(ControlAction::Open.ordinal(), 1);
        assert_eq!(ControlAction::Close.ordinal(), 2);
        assert_eq!(ControlAction::Reset.ordinal(), 3);
        assert_eq!(ControlAction::Lock.ordinal(), 4);
        assert_eq!(ControlAction::Unlock.ordinal(), 5);
        assert_eq!(ControlAction::TapUp.ordinal(), 6);
        assert_eq!(ControlAction::TapDown.ordinal(), 7);
        // Outside `EControlAction`: the Relay "leave this phase as is" sentinel.
        assert_eq!(ControlAction::Keep.ordinal(), i32::MIN);
        for a in [
            ControlAction::None,
            ControlAction::Open,
            ControlAction::Close,
            ControlAction::Reset,
            ControlAction::Lock,
            ControlAction::Unlock,
            ControlAction::TapUp,
            ControlAction::TapDown,
            ControlAction::Keep,
        ] {
            assert_eq!(ControlAction::from_ordinal(a.ordinal()), a);
        }
    }

    /// The queue-code channel is open (a CapControl `USERCONTROL` guest pushes
    /// its own code, upstream `USER_BASE_ACTION_CODE = 100`), so the mapping
    /// must be **total** — no ordinal is lost or remapped, exactly like the
    /// pre-enum bare `i32` field, and `DblTraceParameter` still mirrors it.
    #[test]
    fn control_action_round_trips_every_out_of_set_ordinal() {
        for v in [-1000, -50, -1, 8, 9, 100, 101, 1000, i32::MAX, i32::MIN + 1] {
            let a = ControlAction::from_ordinal(v);
            assert_eq!(a, ControlAction::Other(v), "{v} must stay a raw code");
            assert_eq!(a.ordinal(), v);
        }
        // …and the ten named values are the only ones that are not `Other`.
        for v in 0..=7 {
            assert!(!matches!(
                ControlAction::from_ordinal(v),
                ControlAction::Other(_)
            ));
        }
        assert!(!matches!(
            ControlAction::from_ordinal(i32::MIN),
            ControlAction::Other(_)
        ));
    }

    /// The Relay `Action`/`State` `DssEnum`s carry the `Keep` sentinel as their
    /// `default_value` (an unmatched token parses to it instead of raising), so
    /// the registry and the enum must agree on the exact number — a silent
    /// mismatch would turn "leave the phase as is" into a stored state.
    #[test]
    fn relay_state_enums_default_to_the_keep_sentinel() {
        let reg = EnumRegistry::new();
        for id in [reg.relay_action, reg.relay_state] {
            let e = reg.get(id);
            assert_eq!(
                ControlAction::from_ordinal(e.default_value),
                ControlAction::Keep,
                "{} default_value must be the Keep sentinel",
                e.name
            );
        }
        // Every other CTRL-backed registry entry keeps the Pascal default 0 —
        // `Keep` is a Relay-only spelling quirk (`InterpretRelayState`).
        for id in [
            reg.swt_control_action,
            reg.swt_control_state,
            reg.fuse_action,
            reg.fuse_state,
            reg.recloser_action,
            reg.recloser_state,
        ] {
            let e = reg.get(id);
            assert_ne!(
                ControlAction::from_ordinal(e.default_value),
                ControlAction::Keep,
                "{} must not carry the Keep sentinel",
                e.name
            );
        }
    }
}
