//! Port of `Controls/CapControl.pas` — `TCapControlObj`, the capacitor-bank
//! switching control. **Phase 4 ports the parse-time surface only**
//! (properties, `capacitor=`/`element=` resolution, `RecalcElementData`'s
//! bus/phase setup, `MakeLike`); the `Sample`/`DoPendingAction` switching
//! machinery is Phase 5 (PHASE4_PLAN §WP4.7).
//!
//! Like every `TControlElem`, a CapControl builds **no Yprim** and its terminal
//! currents are zero; its single terminal attaches to the monitored element's
//! terminal bus (or the capacitor's, for Time/Follow control types).
//!
//! The class is split by concern: this file holds the property metadata, the
//! [`CapControl`] struct, its construction/reset lifecycle, the parse-time
//! [`CapControl::recalc`], and the `PF1to2` helper; [`control_loop`] holds the
//! solve-time `Sample`/`DoPendingAction` machinery; [`accessors`] holds the
//! [`CktElement`](crate::elements::traits::CktElement)/[`DssObject`](crate::obj::base::DssObject)
//! trait impls.

#[cfg(test)]
mod tests;

mod accessors;
mod control_loop;

use num_complex::Complex64;

use crate::elements::control::control_elem::{
    CTRL_CLOSE, CTRL_NONE, CTRL_OPEN, ControlElemData, RefSnapshot,
};
use crate::elements::general::load_shape::LoadShapeObj;
use crate::elements::pd::capacitor::ControlledCapacitor;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};

/// `ECapControlType` ordinals.
mod ctrl_type {
    pub const CURRENT: i32 = 0;
    pub const VOLTAGE: i32 = 1;
    pub const KVAR: i32 = 2;
    pub const TIME: i32 = 3;
    pub const PF: i32 = 4;
    pub const FOLLOW: i32 = 5;
}

/// `CapControl.pas` monitored-phase pseudo-phases (the `mon_phase` hybrid enum's
/// avg/max/min, mirrored in `RegControl`).
const AVGPHASES: i32 = -1;
const MAXPHASE: i32 = -2;
const MINPHASE: i32 = -3;

/// 1-based property ordinals (Pascal `TCapControlProp` + class tails).
pub mod prop {
    pub const ELEMENT: usize = 1;
    pub const TERMINAL: usize = 2;
    pub const CAPACITOR: usize = 3;
    pub const TYPE: usize = 4;
    pub const PTRATIO: usize = 5;
    pub const CTRATIO: usize = 6;
    pub const ONSETTING: usize = 7;
    pub const OFFSETTING: usize = 8;
    pub const DELAY: usize = 9;
    pub const VOLTOVERRIDE: usize = 10;
    pub const VMAX: usize = 11;
    pub const VMIN: usize = 12;
    pub const DELAYOFF: usize = 13;
    pub const DEADTIME: usize = 14;
    pub const CTPHASE: usize = 15;
    pub const PTPHASE: usize = 16;
    pub const VBUS: usize = 17;
    pub const EVENTLOG: usize = 18;
    pub const USERMODEL: usize = 19;
    pub const USERDATA: usize = 20;
    pub const PCTMINKVAR: usize = 21;
    pub const RESET: usize = 22;
    pub const CONTROLSIGNAL: usize = 23;
    // TCktElementClass tail:
    pub const BASE_FREQ: usize = 24;
    pub const ENABLED: usize = 25;
    pub const NUM_PROPS: usize = 26; // incl. Like
}

/// `TCapControl.DefineProperties`.
pub fn class_props(enums: &EnumRegistry) -> ClassProps {
    use prop::*;
    let defs = vec![
        // Pascal `PropertyOffset2 = 0`: any circuit element by full name.
        PropDef::object_ref_any("Element"),
        PropDef::integer("Terminal"),
        // Pascal flags CheckForVar + Required (both inert here).
        PropDef::object_ref_class("Capacitor", "Capacitor"),
        PropDef::mapped_string_enum("Type", enums.cap_control_type),
        PropDef::double("PTRatio"),
        PropDef::double("CTRatio"),
        PropDef::double("OnSetting"),
        PropDef::double("OffSetting"),
        PropDef::double("Delay"),
        PropDef::boolean("VoltOverride"),
        PropDef::double("VMax"),
        PropDef::double("VMin"),
        PropDef::double("DelayOff"),
        PropDef::double("DeadTime"),
        PropDef::mapped_string_enum("CTPhase", enums.mon_phase),
        PropDef::mapped_string_enum("PTPhase", enums.mon_phase),
        PropDef::string("VBus"),
        PropDef::boolean("EventLog"),
        // User-written control DLLs are never ported (no DLL loading in safe
        // Rust — PHASE4_PLAN §5); setting them is a hard error.
        PropDef::string("UserModel").flags(PropFlags::NOT_PORTED | PropFlags::IS_FILENAME),
        PropDef::string("UserData").flags(PropFlags::NOT_PORTED),
        PropDef::double("pctMinkvar"),
        // Pascal: BooleanActionProperty (DoReset); the getter is always 0.
        PropDef::boolean("Reset"),
        // `ctrlSignalShape` (`CapControl.pas` l.288): a LoadShape ref read by the
        // FOLLOWCONTROL arm of `Sample`. Bound to the LoadShape class
        // (`PropertyOffset2 := ptruint(DSS.LoadShapeClass)`); no WriteByFunction
        // in Pascal (the commented-out `CheckForVar` flag is also inert).
        PropDef::object_ref_class("LoadShape", "ControlSignal"),
        // TCktElementClass tail:
        PropDef::double("BaseFreq").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::enabled("Enabled"),
    ];
    debug_assert_eq!(defs.len(), NUM_PROPS - 1);
    ClassProps::new("CapControl", defs, true)
}

/// `TCapControlObj` (+ the parse-relevant `TCapControlVars` fields).
#[derive(Debug, Clone)]
pub struct CapControl {
    pub ccd: ControlElemData,
    /// Dump name of the controlled capacitor (Pascal renders `Name`).
    controlled_name: String,
    /// Dump name of the monitored element (Pascal renders `FullName`).
    monitored_full_name: String,
    /// Parse-time shape snapshots of the two references.
    ctrl_snap: Option<RefSnapshot>,
    mon_snap: Option<RefSnapshot>,
    /// Dump name of the `ControlSignal` LoadShape (bare object name, like
    /// `StorageController`'s `Yearly`/`Daily`/`Duty`).
    control_signal_name: String,
    /// `ctrlSignalShape`: a snapshot-clone of the resolved `ControlSignal`
    /// LoadShape (the WP4.2 `FetchLineCode` pattern), read by the
    /// FOLLOWCONTROL arm of `Sample`.
    ctrl_signal_shape: Option<LoadShapeObj>,

    /// `ECapControlType` ordinal (0=Current ... 5=Follow).
    control_type: i32,
    fct_phase: i32,
    fpt_phase: i32,
    pt_ratio: f64,
    ct_ratio: f64,
    on_value: f64,
    off_value: f64,
    pfon_value: f64,
    pfoff_value: f64,
    on_delay: f64,
    off_delay: f64,
    dead_time: f64,
    last_open_time: f64,
    voverride: bool,
    voverride_bus_specified: bool,
    voverride_bus_name: String,
    vmax: f64,
    vmin: f64,
    fpct_minkvar: f64,
    // Runtime switching state (`TCapControlVars`), driven by `Sample`/
    // `DoPendingAction` (wired into the control loop in WP5.7):
    /// `FPendingChange` (CTRL_NONE/OPEN/CLOSE).
    pending_change: i32,
    /// `ShouldSwitch`: an action is pending.
    should_switch: bool,
    /// `Armed`: a queue action is outstanding (deleted on disarm).
    armed: bool,
    /// `PresentState`/`InitialState` (CTRL_OPEN/CTRL_CLOSE).
    present_state: i32,
    initial_state: i32,
    /// `VoverrideEvent`.
    voverride_event: bool,
    /// `ControlActionHandle` (the queue handle to delete when disarming).
    control_action_handle: i32,
}

impl CapControl {
    /// Pascal `TCapControlObj.Create`.
    pub fn new(name: &str) -> Self {
        let mut ccd = ControlElemData::new(name, prop::NUM_PROPS);
        ccd.cd.nphases = 3;
        ccd.cd.nconds = 3;
        ccd.cd.set_nterms(1); // forces allocation of terminals and conductors
        ccd.element_terminal = 1;

        let dead_time = 300.0;
        Self {
            ccd,
            controlled_name: String::new(),
            monitored_full_name: String::new(),
            ctrl_snap: None,
            mon_snap: None,
            control_signal_name: String::new(),
            ctrl_signal_shape: None, // Pascal `ctrlSignalShape := NIL;`
            control_type: ctrl_type::CURRENT,
            fct_phase: 1,
            fpt_phase: 1,
            pt_ratio: 60.0,
            ct_ratio: 60.0,
            on_value: 300.0,
            off_value: 200.0,
            pfon_value: 0.95,
            pfoff_value: 1.05,
            on_delay: 15.0,
            off_delay: 15.0,
            dead_time,
            last_open_time: -dead_time,
            voverride: false,
            voverride_bus_specified: false,
            voverride_bus_name: String::new(),
            vmax: 126.0,
            vmin: 115.0,
            fpct_minkvar: 50.0,
            pending_change: CTRL_NONE,
            should_switch: false,
            armed: false,
            present_state: CTRL_CLOSE,
            initial_state: CTRL_CLOSE,
            voverride_event: false,
            control_action_handle: 0,
        }
    }

    /// Pascal `TCapControlObj.Reset` (the `Reset` action property). The
    /// `ControlledElement.Closed[0] := InitialState` restore needs the
    /// controlled capacitor, which the property setter cannot reach; it is
    /// applied by the control-loop reset path ([`Self::reset_with`]) — here we
    /// restore the control's own switching state.
    fn reset(&mut self) {
        self.set_pending_change(CTRL_NONE);
        self.should_switch = false;
        self.armed = false;
        self.last_open_time = -self.dead_time;
        self.present_state = self.initial_state;
    }

    /// The full Pascal `Reset` (the `DoResetControls` path): restore the
    /// control state *and* drive the bank back to `InitialState`. Returns
    /// whether a force was applied (the caller raises `SystemYChanged`).
    ///
    /// Pascal `Reset` does `ControlledElement.Closed[0] := …` (`CapControl.pas`)
    /// for an `InitialState` of OPEN/CLOSE, which raises `SystemYChanged`
    /// **unconditionally** (the `case` has no else for `CTRL_NONE`). We mirror
    /// that — return `true` whenever a force was applied, **not** gated on an
    /// all-or-nothing change check (`is_closed()` reads "all phases closed", so a
    /// partially-open bank could otherwise slip a real change past the Y
    /// rebuild). Reset is rare, so a redundant rebuild is negligible.
    pub(crate) fn reset_with(&mut self, cap: &mut dyn ControlledCapacitor) -> bool {
        let want_closed = match self.initial_state {
            CTRL_OPEN => Some(false),
            CTRL_CLOSE => Some(true),
            _ => None,
        };
        if let Some(want) = want_closed {
            cap.set_closed(want);
        }
        self.reset();
        want_closed.is_some()
    }

    /// Pascal `Set_PendingChange` (also mirrors to `DblTraceParameter`).
    fn set_pending_change(&mut self, value: i32) {
        self.pending_change = value;
        self.ccd.dbl_trace_param = value as f64;
    }

    /// Pascal `TCapControlObj.RecalcElementData` (parse-time subset).
    fn recalc(&mut self) {
        // Check for existence of capacitor.
        if self.ccd.controlled_element.is_none() {
            // Pascal raises here (surfaces as command error 303 in the oracle).
            self.ccd.cd.obj.push_error(format!(
                "\"CapControl.{}\": Capacitor is not set, aborting.",
                self.ccd.cd.obj.name()
            ));
            return;
        }
        let ctrl = self.ctrl_snap.clone().unwrap_or_default();

        // Force number of phases to be same as the controlled capacitor.
        self.ccd.cd.nphases = ctrl.nphases;
        self.ccd.cd.set_nconds(ctrl.nphases);
        // Pascal syncs `ControlledElement.Closed[0]` with AvailableSteps and
        // derives PresentState/InitialState here — control actions are Phase 5
        // and capacitor terminals start (and stay) closed during parse.

        let eff = if self.control_type != ctrl_type::TIME && self.control_type != ctrl_type::FOLLOW
        {
            if self.mon_snap.is_none() {
                self.ccd.cd.obj.push_error(format!(
                    "CapControl.{}: Element is not set, aborting.",
                    self.ccd.cd.obj.name()
                ));
                return;
            }
            self.mon_snap.clone().unwrap_or_default()
        } else {
            // force terminal to 1 if no monitored element is provided
            self.ccd.element_terminal = 1;
            ctrl
        };

        if self.ccd.element_terminal > eff.nterms as i32 {
            // DoErrorMsg 362.
            self.ccd.cd.obj.push_error(format!(
                "CapControl.{}: Terminal number {} does not exist in \"{}\". Re-specify terminal number. (Error 362)",
                self.ccd.cd.obj.name(),
                self.ccd.element_terminal,
                eff.full_name
            ));
            return;
        }

        // Sets the name of the 1st terminal's connected bus.
        let t = self.ccd.element_terminal;
        let bus = if t >= 1 && (t as usize) <= eff.buses.len() {
            eff.buses[(t - 1) as usize].clone()
        } else {
            String::new() // Pascal GetBus(i) out of range yields ''
        };
        self.ccd.cd.set_bus(1, &bus);
        // cBuffer/CondOffset (sampling buffers) are Phase 5.

        // Alternative override bus: the circuit bus list is only built at
        // solve time, so during parsing the lookup always misses — exactly as
        // in Pascal, which warns and reverts to the default.
        if self.voverride_bus_specified {
            self.ccd.cd.obj.push_error(format!(
                "CapControl.{}: Voltage override Bus \"{}\" not found. Did you wait until buses were defined? Reverting to default.",
                self.ccd.cd.obj.name(),
                self.voverride_bus_name
            ));
            self.voverride_bus_specified = false;
        }
    }
}

/// Pascal `Sample`'s local `PF1to2`: power factor mapped onto `0..2` with the
/// leading range `1..2` (`im < 0` ⇒ `2 − PF`); unity when the apparent power is
/// zero.
fn pf_1to2(s: Complex64) -> f64 {
    let sabs = s.norm();
    let mut result = if sabs != 0.0 { s.re.abs() / sabs } else { 1.0 };
    if s.im < 0.0 {
        result = 2.0 - result;
    }
    result
}
