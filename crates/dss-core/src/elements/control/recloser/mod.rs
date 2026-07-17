//! Port of `Controls/Recloser.pas` (**EPRI OpenDSS r4133**) — `TRecloserObj`, a
//! **per-phase overcurrent recloser**: a `TControlElem` that monitors one PD
//! element's terminal currents and, when a phase or ground current exceeds its
//! `TCC_Curve` pickup, trips the controlled (switched) element open, then
//! recloses after a configured interval — repeating up to `Shots` operations
//! before locking out. The first `NumFast` operations use the *fast* curves, the
//! rest the *slow* curves.
//!
//! **r4133 rewrite (WP-U2.2, delta_r4088_r4133 rows B3/B4/C2/D2/D3/E2/E3):**
//! - **Per-phase state machine.** `FPresentState`/`FNormalState` are per-phase
//!   arrays (`StateArray[1..RECLOSERCONTROLMAXDIM=6]`); `OperationCount`,
//!   `LockedOut`, `ArmedForOpen/Close`, `PhaseTarget`, `RecloserTarget` are
//!   per-phase with an extra `IdxMultiPh = NPhases+1` slot for the ganged path.
//! - **Single-phase tripping/reclosing/lockout** (`SinglePhTrip`,
//!   `SinglePhLockout`): the phase index rides the control-queue proxy handle.
//! - **Fast/slow pickup split** (`PhFastPickup`/`PhSlowPickup`,
//!   `GndFastPickup`/`GndSlowPickup`); legacy `PhaseTrip`/`GroundTrip` set both.
//! - **Breaking default (D2):** the `A`/`D` default curves are removed — a
//!   default-constructed recloser is **inert** (all four curves NIL/`none`).
//! - **Inst-trip delay single-count (D3):** the instantaneous trip time is a bare
//!   `0.01` (the `MechanicalDelay` is added once, at the queue push), so inst
//!   operations fire one `MechanicalDelay` earlier than r4088.
//! - `MaxOperatingCount` curve selection over non-locked-out phases; sampling
//!   continues while ≥1 phase is closed (B4); state resync from
//!   `ControlledElement.Closed[i]` at the top of `Sample`.
//! - **Property table 24 → 46** with deprecated aliases (`PhaseFast→PhFastCurve`,
//!   `PhaseDelayed→PhSlowCurve`, `Reset→ResetTime`, `Delay→MechanicalDelay`, TD
//!   renames…), plus `Lock`/`Reset` actions, `EventLog`/`DebugTrace`,
//!   `RatedCurrent`/`InterruptingRating`, `Normal`/`State` per-phase arrays.
//! - **Event-log wording overhaul (E2/E3):** per-phase `'Phase %d opened on %s
//!   (…trip) & locked out (…lockout)'` / `'Phase %d closed (…reclosing)'` /
//!   `'Phase ALL reset (3ph reset)'`, and the `[closed, closed, closed, ]`
//!   render for `Normal`/`State`.
//!
//! `GetTccCurve('none')` returns NIL silently (r4133 C5): the four curve names
//! default to `none`, resolving to no curve without error.
//!
//! Concern split mirrors the other controls: this file holds the property
//! metadata, the [`Recloser`] struct, construction/`recalc`, and the
//! `Sample`/`DoPendingAction`/`Reset` behavior; [`accessors`] holds the trait
//! impls.

#[cfg(test)]
mod tests;

mod accessors;

use num_complex::Complex64;

use crate::elements::control::control_elem::{
    CTRL_CLOSE, CTRL_OPEN, CTRL_RESET, ControlElemData, CtrlCtx, RefSnapshot,
};
use crate::elements::general::tcc_curve::TccCurveObj;
use crate::elements::traits::CktElement;
use crate::obj::base::RefAction;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};

/// `RecloseIntervals` fixed allocation (Pascal `Reallocmem(…, SizeOf(Double) *
/// 4)`).
const RECLOSE_MAX: usize = 4;

/// `RECLOSERCONTROLMAXDIM` — the per-phase state-array bound (Pascal `const`).
const RCMAX: usize = 6;

/// Per-phase array length. The arrays are indexed **1-based** (Pascal
/// `Array[1..RECLOSERCONTROLMAXDIM]`); slot 0 is unused so `arr[i]` lines up with
/// the Pascal phase index. The ganged slot lives at `IdxMultiPh = NPhases+1`
/// (= 4, frozen at the `Create`-time `NPhases=3`), which is `< ARR`. Sizing to
/// `RCMAX + 2` covers per-phase indices `1..=RCMAX` plus that ganged slot without
/// the Pascal >3-phase out-of-bounds hazard.
const ARR: usize = RCMAX + 2;

/// 1-based property ordinals (Pascal `TRecloser.DefineProperties` r4133 + the
/// `TCktElementClass` tail).
pub mod prop {
    pub const MONITORED_OBJ: usize = 1;
    pub const MONITORED_TERM: usize = 2;
    pub const SWITCHED_OBJ: usize = 3;
    pub const SWITCHED_TERM: usize = 4;
    pub const NUM_FAST: usize = 5;
    pub const PH_FAST_CURVE: usize = 6;
    pub const PH_SLOW_CURVE: usize = 7;
    pub const GND_FAST_CURVE: usize = 8;
    pub const GND_SLOW_CURVE: usize = 9;
    pub const PH_FAST_PICKUP: usize = 10;
    pub const GND_FAST_PICKUP: usize = 11;
    pub const PH_INST: usize = 12;
    pub const GND_INST: usize = 13;
    pub const RESET_TIME: usize = 14;
    pub const SHOTS: usize = 15;
    pub const RECLOSE_INTERVALS: usize = 16;
    pub const MECHANICAL_DELAY: usize = 17;
    pub const ACTION: usize = 18;
    pub const TD_PH_FAST: usize = 19;
    pub const TD_GND_FAST: usize = 20;
    pub const TD_PH_SLOW: usize = 21;
    pub const TD_GND_SLOW: usize = 22;
    pub const NORMAL: usize = 23;
    pub const STATE: usize = 24;
    pub const SINGLE_PH_TRIP: usize = 25;
    pub const SINGLE_PH_LOCKOUT: usize = 26;
    pub const LOCK: usize = 27;
    pub const RESET_ACTION: usize = 28;
    pub const EVENT_LOG: usize = 29;
    pub const DEBUG_TRACE: usize = 30;
    pub const RATED_CURRENT: usize = 31;
    pub const INTERRUPTING_RATING: usize = 32;
    // Deprecated aliases (share fields with the canonical props above):
    pub const PHASE_FAST: usize = 33; // -> PH_FAST_CURVE
    pub const PHASE_DELAYED: usize = 34; // -> PH_SLOW_CURVE
    pub const GROUND_FAST: usize = 35; // -> GND_FAST_CURVE
    pub const GROUND_DELAYED: usize = 36; // -> GND_SLOW_CURVE
    pub const PHASE_TRIP: usize = 37; // -> PhFastPickup + PhSlowPickup
    pub const GROUND_TRIP: usize = 38; // -> GndFastPickup + GndSlowPickup
    pub const PH_SLOW_PICKUP: usize = 39;
    pub const GND_SLOW_PICKUP: usize = 40;
    pub const TD_PH_DELAYED: usize = 41; // -> TDPhSlow
    pub const TD_GR_DELAYED: usize = 42; // -> TDGndSlow
    pub const DELAY: usize = 43; // -> MechanicalDelay
    pub const PHASE_INST: usize = 44; // -> PhInst
    pub const GROUND_INST: usize = 45; // -> GndInst
    pub const TD_GR_FAST: usize = 46; // -> TDGndFast
    // TCktElementClass tail:
    pub const BASE_FREQ: usize = 47;
    pub const ENABLED: usize = 48;
    pub const NUM_PROPS: usize = 49; // incl. Like
}

/// `TRecloser.DefineProperties` (r4133).
pub fn class_props(enums: &EnumRegistry) -> ClassProps {
    let defs = vec![
        PropDef::object_ref_any("MonitoredObj"),
        PropDef::integer("MonitoredTerm"),
        PropDef::object_ref_any("SwitchedObj"),
        PropDef::integer("SwitchedTerm"),
        PropDef::integer("NumFast"),
        // The four TCC curves. `GetTccCurve('none')` -> NIL silently; the default
        // names are `none` (constructor NIL curves — the D2 inert default).
        PropDef::object_ref_class("TCC_Curve", "PhFastCurve"),
        PropDef::object_ref_class("TCC_Curve", "PhSlowCurve"),
        PropDef::object_ref_class("TCC_Curve", "GndFastCurve"),
        PropDef::object_ref_class("TCC_Curve", "GndSlowCurve"),
        PropDef::double("PhFastPickup"),
        PropDef::double("GndFastPickup"),
        PropDef::double("PhInst"),
        PropDef::double("GndInst"),
        PropDef::double("ResetTime"),
        PropDef::integer("Shots")
            .flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO | PropFlags::VALUE_OFFSET)
            .value_offset(-1.0),
        PropDef::double_v_array_max("RecloseIntervals", RECLOSE_MAX),
        PropDef::double("MechanicalDelay"),
        // Action: deprecated ganged StringEnumActionProperty (getter dumps empty).
        PropDef::action("Action", enums.recloser_action),
        PropDef::double("TDPhFast"),
        PropDef::double("TDGndFast"),
        PropDef::double("TDPhSlow"),
        PropDef::double("TDGndSlow"),
        PropDef::mapped_string_enum_array("Normal", enums.recloser_state)
            .flags(PropFlags::DYNAMIC_DEFAULT),
        PropDef::mapped_string_enum_array("State", enums.recloser_state),
        PropDef::boolean("SinglePhTrip"),
        PropDef::boolean("SinglePhLockout"),
        PropDef::boolean("Lock"),
        PropDef::boolean("Reset"),
        PropDef::boolean("EventLog"),
        PropDef::boolean("DebugTrace"),
        PropDef::double("RatedCurrent"),
        PropDef::double("InterruptingRating"),
        // Deprecated aliases (props 33-46):
        PropDef::object_ref_class("TCC_Curve", "PhaseFast"),
        PropDef::object_ref_class("TCC_Curve", "PhaseDelayed"),
        PropDef::object_ref_class("TCC_Curve", "GroundFast"),
        PropDef::object_ref_class("TCC_Curve", "GroundDelayed"),
        PropDef::double("PhaseTrip"),
        PropDef::double("GroundTrip"),
        PropDef::double("PhSlowPickup"),
        PropDef::double("GndSlowPickup"),
        PropDef::double("TDPhDelayed"),
        PropDef::double("TDGrDelayed"),
        PropDef::double("Delay"),
        PropDef::double("PhaseInst"),
        PropDef::double("GroundInst"),
        PropDef::double("TDGrFast"),
        // TCktElementClass tail:
        PropDef::double("BaseFreq").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::enabled("Enabled"),
    ];
    debug_assert_eq!(defs.len(), prop::NUM_PROPS - 1);
    ClassProps::new("Recloser", defs, true)
}

/// `TRecloserObj` (r4133).
#[derive(Debug, Clone)]
pub struct Recloser {
    pub ccd: ControlElemData,
    /// Dump name of the monitored element (Pascal `FullName`).
    monitored_full_name: String,
    /// Dump name of the switched (controlled) element (Pascal `FullName`).
    switched_full_name: String,
    /// Parse-time shape snapshots of the two references.
    mon_snap: Option<RefSnapshot>,
    ctrl_snap: Option<RefSnapshot>,
    /// `MonitoredElementTerminal`.
    monitored_element_terminal: i32,

    /// Dump names of the four TCC curves (default `none`).
    ph_fast_name: String,
    ph_slow_name: String,
    gnd_fast_name: String,
    gnd_slow_name: String,
    /// Resolved curve clones (resolved by the executive from the names). A `None`
    /// curve never trips its branch (the D2 inert default).
    ph_fast: Option<TccCurveObj>,
    ph_slow: Option<TccCurveObj>,
    gnd_fast: Option<TccCurveObj>,
    gnd_slow: Option<TccCurveObj>,

    /// `NumFast` — operations on the fast curves before switching to slow.
    num_fast: i32,
    /// Fast/slow phase and ground pickup divisors.
    ph_fast_pickup: f64,
    gnd_fast_pickup: f64,
    ph_slow_pickup: f64,
    gnd_slow_pickup: f64,
    /// `PhInst`/`GndInst` instantaneous-trip thresholds (0 = disabled).
    ph_inst: f64,
    gnd_inst: f64,
    /// Informational ratings (not used in the power flow).
    rated_current: f64,
    interrupting_rating: f64,
    /// `ResetTime` (s) — the reset delay queued when current drops below pickup.
    reset_time: f64,
    /// `MechanicalDelay` (s) — added to every trip time before queuing.
    mechanical_delay: f64,
    /// `NumReclose` (= `Shots - 1`) — the count of reclose operations.
    num_reclose: i32,
    /// `RecloseIntervals[1..4]` (s) — the per-shot reclose delays.
    reclose_intervals: [f64; RECLOSE_MAX],
    /// Time-dial multipliers for the fast/slow phase/ground curves.
    td_ph_fast: f64,
    td_gnd_fast: f64,
    td_ph_slow: f64,
    td_gnd_slow: f64,

    /// `FPresentState[1..RCMAX]` (per phase) — the recloser's live position.
    present_state: [i32; ARR],
    /// `FNormalState[1..RCMAX]` (per phase) — the reset target.
    normal_state: [i32; ARR],
    /// `OperationCount[1..IdxMultiPh]` — per-phase (+ ganged) operation index.
    operation_count: [i32; ARR],
    /// `LockedOut[1..IdxMultiPh]` — a phase (or the ganged group) exhausted shots.
    locked_out: [bool; ARR],
    /// `ArmedForOpen`/`ArmedForClose[1..IdxMultiPh]` — a trip/reclose is queued.
    armed_for_open: [bool; ARR],
    armed_for_close: [bool; ARR],
    /// `PhaseTarget[1..IdxMultiPh]` — which phase tripped on a phase element.
    phase_target: [bool; ARR],
    /// `RecloserTarget[1..IdxMultiPh]` — the human-readable trip cause per slot.
    recloser_target: [String; ARR],
    /// `GroundTarget` — scalar; a ground element tripped.
    ground_target: bool,
    /// `IdxMultiPh = NPhases+1` — the ganged operation slot (frozen at 4).
    idx_multi_ph: usize,

    /// `NormalStateSet` — Normal defaults to the first State specified.
    normal_state_set: bool,
    /// `SinglePhTrip`/`SinglePhLockout` — single-phase operation modes.
    single_ph_trip: bool,
    single_ph_lockout: bool,
    /// `FLocked` — the `Lock` property (blocks manual/internal state changes).
    f_locked: bool,
    /// `DebugTrace` — write extra `Debug Sample:` lines to the event log.
    debug_trace: bool,

    /// Deferred parse-time element forces (the `RecalcElementData` Closed[i] sync).
    pending_ref_actions: Vec<RefAction>,
}

impl Recloser {
    /// Pascal `TRecloserObj.Create` (r4133).
    pub fn new(name: &str) -> Self {
        let mut ccd = ControlElemData::new(name, prop::NUM_PROPS);
        ccd.cd.nphases = 3; // directly set conds and phases
        ccd.cd.nconds = 3;
        ccd.cd.set_nterms(1); // forces allocation of terminals and conductors
        ccd.element_terminal = 1;
        // Pascal `TControlElem.Create`: `ShowEventLog := EventLogDefault` (global
        // `False`). The Recloser does NOT override it (the help text's "Default is
        // Yes" is aspirational — the oracle logs nothing until `EventLog=yes`), so
        // `ControlElemData::new`'s `show_event_log = false` is correct.

        Self {
            ccd,
            monitored_full_name: String::new(),
            switched_full_name: String::new(),
            mon_snap: None,
            ctrl_snap: None,
            monitored_element_terminal: 1,
            // D2: default curves removed -> NIL/`none` -> inert.
            ph_fast_name: "none".to_string(),
            ph_slow_name: "none".to_string(),
            gnd_fast_name: "none".to_string(),
            gnd_slow_name: "none".to_string(),
            ph_fast: None,
            ph_slow: None,
            gnd_fast: None,
            gnd_slow: None,
            num_fast: 1,
            ph_fast_pickup: 1.0,
            gnd_fast_pickup: 1.0,
            ph_slow_pickup: 1.0,
            gnd_slow_pickup: 1.0,
            ph_inst: 0.0,
            gnd_inst: 0.0,
            rated_current: 0.0,
            interrupting_rating: 0.0,
            reset_time: 15.0,
            mechanical_delay: 0.0,
            num_reclose: 3, // Shots default 4 => NumReclose 3
            reclose_intervals: [0.5, 2.0, 2.0, 0.0],
            td_ph_fast: 1.0,
            td_gnd_fast: 1.0,
            td_ph_slow: 1.0,
            td_gnd_slow: 1.0,
            present_state: [CTRL_CLOSE; ARR],
            normal_state: [CTRL_CLOSE; ARR],
            operation_count: [1; ARR],
            locked_out: [false; ARR],
            armed_for_open: [false; ARR],
            armed_for_close: [false; ARR],
            phase_target: [false; ARR],
            recloser_target: std::array::from_fn(|_| String::new()),
            ground_target: false,
            idx_multi_ph: 3 + 1, // NPhases(=3) + 1, frozen at Create
            normal_state_set: false,
            single_ph_trip: false,
            single_ph_lockout: false,
            f_locked: false,
            debug_trace: false,
            pending_ref_actions: Vec::new(),
        }
    }

    /// The control's own `FullName` (`Recloser.<name>`), for the event log.
    fn full_name(&self) -> String {
        format!("Recloser.{}", self.ccd.cd.obj.name())
    }

    /// Per-phase state-array count (Pascal `Min(RECLOSERCONTROLMAXDIM, NPhases)`).
    fn state_size(&self) -> usize {
        RCMAX.min(self.ccd.cd.nphases.max(1))
    }

    /// Pascal `Edit` CASE `18,24`: default `NormalState` per phase from
    /// `PresentState` on the first `State`/`Action` write (`NormalStateSet`).
    fn state_side_effect(&mut self) {
        if !self.normal_state_set {
            let n = self.state_size();
            for i in 1..=n {
                self.normal_state[i] = self.present_state[i];
            }
        }
        self.normal_state_set = true;
    }

    /// Pascal `InterpretRecloserState` ganged path (deprecated `Action=` and the
    /// unquoted scalar `State=`/`Normal=`): set **every** phase. Blocked while
    /// `Locked` for `State`/`Action`.
    fn set_all_present(&mut self, state: i32) {
        let n = self.state_size();
        for i in 1..=n {
            self.present_state[i] = state;
        }
    }

    fn set_all_normal(&mut self, state: i32) {
        let n = self.state_size();
        for i in 1..=n {
            self.normal_state[i] = state;
        }
    }

    /// Pascal `Action`'s `DoAction`: ganged set of the present state + the `State`
    /// side effect + the per-phase `RecalcElementData` element force (deferred at
    /// `EndEdit`).
    fn do_action(&mut self, ordinal: i32) {
        if self.f_locked {
            return; // Pascal `InterpretRecloserState`: blocked while Locked.
        }
        self.set_all_present(ordinal);
        self.state_side_effect();
    }

    /// Queue the per-phase `RecalcElementData` Closed[i] sync (Pascal
    /// `ControlledElement.Closed[i] := …` per phase). Deferred as a [`RefAction`]
    /// since the property engine holds no mutable view of the target.
    fn queue_switch_force(&mut self) {
        if let Some(target) = self.ccd.controlled_element {
            let n = self.state_size();
            let closed: Vec<bool> = (1..=n)
                .map(|i| self.present_state[i] == CTRL_CLOSE)
                .collect();
            self.pending_ref_actions
                .push(RefAction::SetConductorsClosed {
                    target,
                    terminal: self.ccd.element_terminal.max(1) as usize,
                    closed,
                });
        }
    }

    /// Queue the `RecalcElementData` reliability flag (Pascal
    /// `Include(ControlledElement.Flags, Flg.HasOCPDevice/HasAutoOCPDevice)`).
    /// Only an enabled recloser sets it; it reports `GetOCPDeviceType` ordinal 2
    /// and is an auto-reclosing device.
    fn queue_ocp_flag(&mut self) {
        if let Some(target) = self.ccd.controlled_element
            && self.ccd.cd.enabled
        {
            self.pending_ref_actions.push(RefAction::SetOcpDevice {
                target,
                device_type: 2,
                auto: true,
            });
        }
    }

    /// Pascal `TRecloserObj.RecalcElementData` (r4133): take the phase count from
    /// the monitored element, attach the control's terminal to the monitored bus,
    /// and sync the controlled element **per phase** to `FPresentState`.
    fn recalc(&mut self) {
        if let Some(mon) = self.mon_snap.clone() {
            self.ccd.cd.nphases = mon.nphases;
            if self.monitored_element_terminal > mon.nterms as i32 {
                self.ccd.cd.obj.push_error(format!(
                    "Recloser: \"{}\": Terminal no. \"{}\" does not exist. Re-specify terminal no. (Error 392)",
                    self.ccd.cd.obj.name(),
                    self.monitored_element_terminal
                ));
                return;
            }
            let t = self.monitored_element_terminal;
            let bus = if t >= 1 && (t as usize) <= mon.buses.len() {
                mon.buses[(t - 1) as usize].clone()
            } else {
                String::new()
            };
            self.ccd.cd.set_bus(1, &bus);
        }

        if self.ccd.controlled_element.is_some() {
            self.queue_ocp_flag();
            let n = self.state_size();
            for i in 1..=n {
                if self.present_state[i] == CTRL_CLOSE {
                    self.locked_out[i] = false;
                    self.operation_count[i] = 1;
                    self.armed_for_open[i] = false;
                } else {
                    self.locked_out[i] = true;
                    self.operation_count[i] = self.num_reclose + 1;
                    self.armed_for_close[i] = false;
                }
            }
            self.queue_switch_force();
        }
    }

    /// The fast/slow ground curve, time-dial, pickup and label for a given
    /// operating count (Pascal `Sample` ground-curve selection).
    fn ground_selection(&self, max_op: i32) -> (Option<&TccCurveObj>, f64, f64, &'static str) {
        if max_op > self.num_fast {
            (
                self.gnd_slow.as_ref(),
                self.td_gnd_slow,
                self.gnd_slow_pickup,
                "Slow",
            )
        } else {
            (
                self.gnd_fast.as_ref(),
                self.td_gnd_fast,
                self.gnd_fast_pickup,
                "Fast",
            )
        }
    }

    /// The fast/slow phase curve selection (as [`Self::ground_selection`]).
    fn phase_selection(&self, op: i32) -> (Option<&TccCurveObj>, f64, f64, &'static str) {
        if op > self.num_fast {
            (
                self.ph_slow.as_ref(),
                self.td_ph_slow,
                self.ph_slow_pickup,
                "Slow",
            )
        } else {
            (
                self.ph_fast.as_ref(),
                self.td_ph_fast,
                self.ph_fast_pickup,
                "Fast",
            )
        }
    }

    /// Build the `RecloserTarget` string (Pascal `Sample`): a combination of the
    /// ground and phase trip causes, deciding instantaneous vs curve by the
    /// `Abs(time - 0.01) < EPSILON` test.
    fn build_target(
        trip_time: f64,
        ground_time: f64,
        phase_time: f64,
        ground_type: &str,
        phase_type: &str,
    ) -> String {
        const EPSILON: f64 = 1.0e-12;
        let mut s = String::new();
        if trip_time == ground_time {
            if (ground_time - 0.01).abs() < EPSILON {
                s = "Gnd Instantaneous".to_string();
            } else {
                s = format!("Ground {ground_type}");
            }
        }
        if trip_time == phase_time {
            if !s.is_empty() {
                s.push_str(" + ");
            }
            if (phase_time - 0.01).abs() < EPSILON {
                s.push_str("Ph Instantaneous");
            } else {
                s.push_str(&format!("Ph {phase_type}"));
            }
        }
        s
    }

    /// Pascal `TRecloserObj.Sample` (r4133): resync the live per-phase state,
    /// continue while ≥1 phase is closed, then (single- or three-phase) evaluate
    /// the ground-sum and per-phase TCC trip times on the monitored currents,
    /// arming/disarming the open+reclose queue actions.
    pub(crate) fn sample(
        &mut self,
        ctrl: &mut dyn CktElement,
        mon: &mut dyn CktElement,
        ctx: &mut CtrlCtx,
    ) {
        let element_terminal = self.ccd.element_terminal.max(1) as usize;
        if element_terminal <= ctrl.cd().nterms {
            ctrl.cd_mut().active_terminal = element_terminal - 1;
        }

        let nphases = mon.cd().nphases;
        let cond_offset = (self.monitored_element_terminal.max(1) as usize - 1) * mon.cd().nconds;
        let mut cbuffer = vec![Complex64::ZERO; mon.cd().yorder.max(1)];
        mon.get_currents(ctx.sys, ctx.node_v, &mut cbuffer);
        // Pascal `cBuffer` is 1-based; index arithmetic below stays 1-based
        // (`cur(1 + CondOffset)` … `cur(Fnphases + CondOffset)`).
        let cur = |i: usize| cbuffer.get(i - 1).copied().unwrap_or(Complex64::ZERO);

        // Resync FPresentState[i] from the controlled terminal (external changes).
        let n = self.state_size();
        for i in 1..=n {
            self.present_state[i] = if ctrl.cd().conductor_closed(element_terminal, i) {
                CTRL_CLOSE
            } else {
                CTRL_OPEN
            };
        }
        if self.debug_trace {
            let s = self.render_state_array(&self.present_state);
            self.dbg(ctx, &format!("FPresentState: {s} "));
        }

        // Continue only if at least one phase is closed.
        let any_closed = (1..=n).any(|i| self.present_state[i] == CTRL_CLOSE);
        if !any_closed {
            return;
        }

        // MaxOperatingCount picks the curve tier.
        let max_op = if self.single_ph_trip {
            let mut m: Option<i32> = None;
            for i in 1..=n {
                if self.locked_out[i] {
                    continue; // skip locked-out phases
                }
                m = Some(match m {
                    None => self.operation_count[i],
                    Some(prev) => prev.max(self.operation_count[i]),
                });
            }
            m.unwrap_or(self.operation_count[self.idx_multi_ph])
        } else {
            self.operation_count[self.idx_multi_ph]
        };

        // ---- Ground trip (shared) ----
        let (mut gc_clone, td_ground, gnd_mult, ground_type) = {
            let (gc, td, mult, ty) = self.ground_selection(max_op);
            (gc.cloned(), td, mult, ty)
        };
        let mut ground_time = -1.0_f64;
        if let Some(gc) = &mut gc_clone {
            let mut csum = Complex64::ZERO;
            for i in (1 + cond_offset)..=(nphases + cond_offset) {
                csum += cur(i);
            }
            let cmag = csum.norm();
            ground_time = if self.gnd_inst > 0.0 && cmag >= self.gnd_inst && max_op == 1 {
                0.01 // D3: bare inst trip, delay added once at push
            } else {
                td_ground * gc.get_tcc_time(cmag / gnd_mult)
            };
            if ground_time > 0.0 && self.debug_trace {
                self.dbg(
                    ctx,
                    &format!("Gnd {ground_type} Curve Trip: Mag={cmag:.3}, Time={ground_time:.3}"),
                );
            }
        }
        if ground_time > 0.0 {
            self.ground_target = true;
        }

        if self.single_ph_trip {
            self.sample_single_phase(ctx, cond_offset, ground_time, ground_type, &cur, n);
        } else {
            self.sample_three_phase(
                ctx,
                cond_offset,
                nphases,
                max_op,
                ground_time,
                ground_type,
                &cur,
            );
        }
    }

    /// The single-phase branch of `Sample` (Pascal `if SinglePhTrip`).
    #[allow(clippy::too_many_arguments)]
    fn sample_single_phase(
        &mut self,
        ctx: &mut CtrlCtx,
        cond_offset: usize,
        ground_time: f64,
        ground_type: &str,
        cur: &dyn Fn(usize) -> Complex64,
        n: usize,
    ) {
        for i in 1..=n {
            if self.present_state[i] != CTRL_CLOSE {
                continue;
            }
            let mut trip_time = if ground_time > 0.0 { ground_time } else { -1.0 };

            let (mut pc_clone, td_phase, ph_mult, phase_type) = {
                let (pc, td, mult, ty) = self.phase_selection(self.operation_count[i]);
                (pc.cloned(), td, mult, ty)
            };
            let mut phase_time = -1.0_f64;
            if let Some(pc) = &mut pc_clone {
                let cmag = cur(i + cond_offset).norm();
                if self.ph_inst > 0.0 && cmag >= self.ph_inst && self.operation_count[i] == 1 {
                    phase_time = 0.01; // D3: bare inst
                    if self.debug_trace {
                        self.dbg(
                            ctx,
                            &format!(
                                "Ph Instantaneous (1-Phase) Trip: Phase={i}, Mag={cmag:.3}, Time={phase_time:.3}"
                            ),
                        );
                    }
                } else {
                    let time_test = td_phase * pc.get_tcc_time(cmag / ph_mult);
                    if time_test > 0.0 {
                        phase_time = time_test;
                    }
                }
            }
            if phase_time > 0.0 {
                self.phase_target[i] = true;
                trip_time = if trip_time > 0.0 {
                    trip_time.min(phase_time)
                } else {
                    phase_time
                };
            }

            if trip_time > 0.0 {
                if !self.armed_for_open[i] {
                    self.recloser_target[i] = Self::build_target(
                        trip_time,
                        ground_time,
                        phase_time,
                        ground_type,
                        phase_type,
                    );
                    ctx.queue.push_delay(
                        ctx.int_hour,
                        ctx.t,
                        trip_time + self.mechanical_delay,
                        CTRL_OPEN,
                        i as i32,
                        ctx.self_ref,
                    );
                    if self.operation_count[i] <= self.num_reclose {
                        let interval = self
                            .reclose_intervals
                            .get((self.operation_count[i] - 1).max(0) as usize)
                            .copied()
                            .unwrap_or(0.0);
                        ctx.queue.push_delay(
                            ctx.int_hour,
                            ctx.t,
                            trip_time + self.mechanical_delay + interval,
                            CTRL_CLOSE,
                            i as i32,
                            ctx.self_ref,
                        );
                    }
                    self.armed_for_open[i] = true;
                    self.armed_for_close[i] = true;
                }
            } else if self.armed_for_open[i] {
                ctx.queue.push_delay(
                    ctx.int_hour,
                    ctx.t,
                    self.reset_time,
                    CTRL_RESET,
                    i as i32,
                    ctx.self_ref,
                );
                self.armed_for_open[i] = false;
                self.armed_for_close[i] = false;
                self.ground_target = false;
                self.phase_target[i] = false;
            }
        }
    }

    /// The three-phase (ganged) branch of `Sample` (Pascal `else`).
    #[allow(clippy::too_many_arguments)]
    fn sample_three_phase(
        &mut self,
        ctx: &mut CtrlCtx,
        cond_offset: usize,
        nphases: usize,
        max_op: i32,
        ground_time: f64,
        ground_type: &str,
        cur: &dyn Fn(usize) -> Complex64,
    ) {
        let g = self.idx_multi_ph;
        let (mut pc_clone, td_phase, ph_mult, phase_type) = {
            let (pc, td, mult, ty) = self.phase_selection(max_op);
            (pc.cloned(), td, mult, ty)
        };
        let mut trip_time = if ground_time > 0.0 { ground_time } else { -1.0 };
        let mut phase_time = -1.0_f64;
        if let Some(pc) = &mut pc_clone {
            for i in (1 + cond_offset)..=(nphases + cond_offset) {
                let cmag = cur(i).norm();
                if self.ph_inst > 0.0 && cmag >= self.ph_inst && self.operation_count[g] == 1 {
                    phase_time = 0.01; // D3: bare inst
                    if self.debug_trace {
                        self.dbg(
                            ctx,
                            &format!(
                                "Ph Instantaneous (3-Phase) Trip: Phase={}, Mag={cmag:.3}, Time={phase_time:.3}",
                                i - cond_offset
                            ),
                        );
                    }
                    break; // if inst, no sense checking other phases
                }
                let time_test = td_phase * pc.get_tcc_time(cmag / ph_mult);
                if time_test > 0.0 {
                    phase_time = if phase_time < 0.0 {
                        time_test
                    } else {
                        phase_time.min(time_test)
                    };
                }
            }
        }
        if phase_time > 0.0 {
            self.phase_target[g] = true;
            trip_time = if trip_time > 0.0 {
                trip_time.min(phase_time)
            } else {
                phase_time
            };
        }

        if trip_time > 0.0 {
            if !self.armed_for_open[g] {
                self.recloser_target[g] =
                    Self::build_target(trip_time, ground_time, phase_time, ground_type, phase_type);
                ctx.queue.push_delay(
                    ctx.int_hour,
                    ctx.t,
                    trip_time + self.mechanical_delay,
                    CTRL_OPEN,
                    0,
                    ctx.self_ref,
                );
                if max_op <= self.num_reclose {
                    let interval = self
                        .reclose_intervals
                        .get((max_op - 1).max(0) as usize)
                        .copied()
                        .unwrap_or(0.0);
                    ctx.queue.push_delay(
                        ctx.int_hour,
                        ctx.t,
                        trip_time + self.mechanical_delay + interval,
                        CTRL_CLOSE,
                        0,
                        ctx.self_ref,
                    );
                }
                self.armed_for_open[g] = true;
                self.armed_for_close[g] = true;
            }
        } else if self.armed_for_open[g] {
            ctx.queue.push_delay(
                ctx.int_hour,
                ctx.t,
                self.reset_time,
                CTRL_RESET,
                0,
                ctx.self_ref,
            );
            self.armed_for_open[g] = false;
            self.armed_for_close[g] = false;
            self.ground_target = false;
            self.phase_target[g] = false;
        }
    }

    /// Pascal `TRecloserObj.DoPendingAction` (r4133). `proxy` is the phase index
    /// carried in the control-queue handle (single-phase trip); the ganged path
    /// uses `IdxMultiPh`.
    pub(crate) fn do_pending_action(
        &mut self,
        code: i32,
        proxy: i32,
        ctrl: &mut dyn CktElement,
        ctx: &mut CtrlCtx,
    ) {
        let ph_idx = if self.single_ph_trip {
            proxy.max(1) as usize
        } else {
            self.idx_multi_ph
        };
        let element_terminal = self.ccd.element_terminal.max(1) as usize;
        if element_terminal <= ctrl.cd().nterms {
            ctrl.cd_mut().active_terminal = element_terminal - 1;
        }
        let nphases = self.state_size();

        match code {
            CTRL_OPEN => {
                if self.single_ph_trip {
                    self.do_open_single(ph_idx, nphases, ctrl, ctx);
                } else {
                    self.do_open_ganged(ph_idx, nphases, ctrl, ctx);
                }
            }
            CTRL_CLOSE => {
                if self.single_ph_trip {
                    if self.present_state[ph_idx] == CTRL_OPEN
                        && self.armed_for_close[ph_idx]
                        && !self.locked_out[ph_idx]
                    {
                        ctrl.cd_mut()
                            .set_conductor_closed(element_terminal, ph_idx, true);
                        self.present_state[ph_idx] = CTRL_CLOSE;
                        let m = format!("Phase {ph_idx} closed (1ph reclosing)");
                        self.log_if(ctx, &self.full_name(), &m);
                        self.operation_count[ph_idx] += 1;
                        self.armed_for_close[ph_idx] = false;
                        *ctx.system_y_changed = true;
                    }
                } else {
                    for i in 1..=nphases {
                        if self.present_state[i] == CTRL_OPEN
                            && self.armed_for_close[ph_idx]
                            && !self.locked_out[i]
                            && !self.locked_out[ph_idx]
                        {
                            ctrl.cd_mut()
                                .set_conductor_closed(element_terminal, i, true);
                            self.present_state[i] = CTRL_CLOSE;
                            let m = format!("Phase {i} closed (3ph reclosing)");
                            self.log_if(ctx, &self.full_name(), &m);
                            *ctx.system_y_changed = true;
                        }
                    }
                    self.armed_for_close[ph_idx] = false;
                    self.operation_count[ph_idx] += 1;
                }
            }
            CTRL_RESET => {
                if self.single_ph_trip {
                    if self.present_state[ph_idx] == CTRL_CLOSE && !self.armed_for_open[ph_idx] {
                        self.operation_count[ph_idx] = 1;
                        let m = format!("Phase {ph_idx} reset (1ph reset)");
                        self.log_if(ctx, &self.full_name(), &m);
                    }
                } else {
                    for i in 1..=nphases {
                        if self.present_state[i] == CTRL_CLOSE {
                            if !self.armed_for_open[ph_idx] {
                                self.operation_count[ph_idx] = 1;
                                self.log_if(ctx, &self.full_name(), "Phase ALL reset (3ph reset)");
                            }
                            break; // no need to loop over all closed phases
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// Pascal `DoPendingAction` CTRL_OPEN, single-phase branch.
    fn do_open_single(
        &mut self,
        ph_idx: usize,
        nphases: usize,
        ctrl: &mut dyn CktElement,
        ctx: &mut CtrlCtx,
    ) {
        let element_terminal = self.ccd.element_terminal.max(1) as usize;
        if self.present_state[ph_idx] != CTRL_CLOSE || !self.armed_for_open[ph_idx] {
            return;
        }
        ctrl.cd_mut()
            .set_conductor_closed(element_terminal, ph_idx, false);
        self.present_state[ph_idx] = CTRL_OPEN;
        *ctx.system_y_changed = true;

        if self.operation_count[ph_idx] > self.num_reclose {
            self.locked_out[ph_idx] = true;
            if self.single_ph_lockout {
                let msg = format!(
                    "Phase {ph_idx} opened on {} (1ph trip) & locked out (1ph lockout)",
                    self.recloser_target[ph_idx]
                );
                self.log_if(ctx, &self.full_name(), &msg);
            } else {
                let msg = format!(
                    "Phase {ph_idx} opened on {} (1ph trip) & locked out (3ph lockout)",
                    self.recloser_target[ph_idx]
                );
                self.log_if(ctx, &self.full_name(), &msg);
                // 3-phase lockout: open every other not-yet-locked-out phase.
                for i in 1..=nphases {
                    if i != ph_idx && !self.locked_out[i] {
                        ctrl.cd_mut()
                            .set_conductor_closed(element_terminal, i, false);
                        self.present_state[i] = CTRL_OPEN;
                        self.locked_out[i] = true;
                        if self.armed_for_open[i] {
                            self.armed_for_open[i] = false;
                        }
                        let m = format!(
                            "Phase {i} opened on 3ph lockout (1ph trip) & locked out (3ph lockout)"
                        );
                        self.log_if(ctx, &self.full_name(), &m);
                        *ctx.system_y_changed = true;
                    }
                }
            }
        } else {
            let msg = format!(
                "Phase {ph_idx} opened on {} (1ph trip)",
                self.recloser_target[ph_idx]
            );
            self.log_if(ctx, &self.full_name(), &msg);
        }
        self.armed_for_open[ph_idx] = false;
    }

    /// Pascal `DoPendingAction` CTRL_OPEN, three-phase (ganged) branch.
    fn do_open_ganged(
        &mut self,
        ph_idx: usize,
        nphases: usize,
        ctrl: &mut dyn CktElement,
        ctx: &mut CtrlCtx,
    ) {
        let element_terminal = self.ccd.element_terminal.max(1) as usize;
        for i in 1..=nphases {
            if self.present_state[i] == CTRL_CLOSE && self.armed_for_open[ph_idx] {
                ctrl.cd_mut()
                    .set_conductor_closed(element_terminal, i, false);
                self.present_state[i] = CTRL_OPEN;
                *ctx.system_y_changed = true;
                if self.operation_count[ph_idx] > self.num_reclose {
                    self.locked_out[ph_idx] = true;
                    let m = format!(
                        "Phase {i} opened on {} (3ph trip) & locked out (3ph lockout)",
                        self.recloser_target[ph_idx]
                    );
                    self.log_if(ctx, &self.full_name(), &m);
                } else {
                    let m = format!(
                        "Phase {i} opened on {} (3ph trip)",
                        self.recloser_target[ph_idx]
                    );
                    self.log_if(ctx, &self.full_name(), &m);
                }
            }
        }
        self.armed_for_open[ph_idx] = false;
    }

    /// `AppendtoEventLog` helper (unconditional).
    fn log(&self, ctx: &mut CtrlCtx, element: &str, action: &str) {
        ctx.events
            .append(element, action, ctx.int_hour, ctx.t, ctx.control_iter);
    }

    /// `if ShowEventLog then AppendtoEventLog` — the r4133 event-log guard.
    fn log_if(&self, ctx: &mut CtrlCtx, element: &str, action: &str) {
        if self.ccd.show_event_log {
            self.log(ctx, element, action);
        }
    }

    /// `if DebugTrace then AppendToEventLog('Debug Sample: Recloser.<name>', …)`.
    fn dbg(&self, ctx: &mut CtrlCtx, action: &str) {
        let el = format!("Debug Sample: Recloser.{}", self.ccd.cd.obj.name());
        self.log(ctx, &el, action);
    }

    /// Render a per-phase state array as `[closed, closed, closed, ]` (the
    /// `GetPropertyValue(24)` form used by the `DebugTrace` line).
    fn render_state_array(&self, arr: &[i32; ARR]) -> String {
        let n = self.state_size();
        let mut s = String::from("[");
        for &v in arr.iter().take(n + 1).skip(1) {
            s.push_str(if v == CTRL_OPEN { "open" } else { "closed" });
            s.push_str(", ");
        }
        s.push(']');
        s
    }

    /// Pascal `TRecloserObj.Reset` control-side state (`FPresentState`/armed/
    /// targets) restored to `NormalState` per phase. A `Locked` recloser does not
    /// reset.
    pub(crate) fn reset_control_side(&mut self) {
        if self.f_locked {
            return;
        }
        let n = self.state_size();
        for i in 1..=n {
            self.present_state[i] = self.normal_state[i];
            self.armed_for_open[i] = false;
            self.armed_for_close[i] = false;
            self.phase_target[i] = false;
        }
        self.ground_target = false;
    }

    /// Pascal `TRecloserObj.Reset` (full, r4133): restore per-phase state and
    /// force the controlled element's conductors to `NormalState`. Returns whether
    /// a force was applied (the caller raises `SystemYChanged`). A `Locked`
    /// recloser does not reset.
    pub(crate) fn reset_with(&mut self, ctrl: &mut dyn CktElement) -> bool {
        if self.f_locked {
            return false;
        }
        let element_terminal = self.ccd.element_terminal.max(1) as usize;
        if element_terminal <= ctrl.cd().nterms {
            ctrl.cd_mut().active_terminal = element_terminal - 1;
        }
        let n = self.state_size();
        for i in 1..=n {
            self.present_state[i] = self.normal_state[i];
            self.armed_for_open[i] = false;
            self.armed_for_close[i] = false;
            self.ground_target = false;
            self.phase_target[i] = false;
            if self.normal_state[i] == CTRL_OPEN {
                ctrl.cd_mut()
                    .set_conductor_closed(element_terminal, i, false);
                self.locked_out[i] = true;
                self.operation_count[i] = self.num_reclose + 1;
            } else {
                ctrl.cd_mut()
                    .set_conductor_closed(element_terminal, i, true);
                self.locked_out[i] = false;
                self.operation_count[i] = 1;
            }
        }
        true
    }

    /// Pascal prop 28 (`Reset=Yes`): clear `Lock`, run `Reset`, and force the
    /// controlled element per phase (deferred at `EndEdit`).
    fn reset_action(&mut self) {
        self.f_locked = false;
        self.reset_control_side();
        let n = self.state_size();
        for i in 1..=n {
            if self.normal_state[i] == CTRL_OPEN {
                self.locked_out[i] = true;
                self.operation_count[i] = self.num_reclose + 1;
            } else {
                self.locked_out[i] = false;
                self.operation_count[i] = 1;
            }
        }
        if let Some(target) = self.ccd.controlled_element {
            let closed: Vec<bool> = (1..=n)
                .map(|i| self.normal_state[i] == CTRL_CLOSE)
                .collect();
            self.pending_ref_actions
                .push(RefAction::SetConductorsClosed {
                    target,
                    terminal: self.ccd.element_terminal.max(1) as usize,
                    closed,
                });
        }
    }
}
