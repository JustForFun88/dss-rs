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
mod user_model;

pub use user_model::CapControlUserModelSlot;

use num_complex::Complex64;

use crate::elements::control::control_elem::{ControlAction, ControlElemData, RefSnapshot};
use crate::elements::control::mon_phase::MonPhase;
use crate::elements::general::load_shape::LoadShapeObj;
use crate::elements::pd::capacitor::ControlledCapacitor;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};

/// `ECapControlType` (`CapControl.pas` TypeEnum). Discriminants are user-visible
/// and frozen (round-trip through the `DssEnum` registry); `i32` survives only at
/// the property parse/report boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum CapControlType {
    #[default]
    Current = 0,
    Voltage = 1,
    Kvar = 2,
    Time = 3,
    Pf = 4,
    Follow = 5,
    /// `USERCONTROL` (`CapControl.pas:99`): NOT a user-selectable `Type=` value
    /// (upstream comments it out of `CapControlTypeEnum`, `:245-249`) — set
    /// internally by `PropertySideEffects` when a `UserModel=` loads
    /// (`:439-440`). `ordinal_to_string(6)` renders empty (the enum table stops
    /// at Follow), matching Pascal's `OrdinalToString` of an unmapped ordinal.
    UserControl = 6,
}

impl CapControlType {
    /// The `CapControlTypeEnum` ordinal (`Type=`/`?`/dump boundary value).
    pub fn ordinal(self) -> i32 {
        self as i32
    }

    /// From the enum-registry ordinal; out-of-range yields `None`. `6`
    /// (`USERCONTROL`) is accepted so a stored control-type round-trips even
    /// though it is never reachable through the `Type=` enum (set internally).
    pub fn from_ordinal(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::Current),
            1 => Some(Self::Voltage),
            2 => Some(Self::Kvar),
            3 => Some(Self::Time),
            4 => Some(Self::Pf),
            5 => Some(Self::Follow),
            6 => Some(Self::UserControl),
            _ => None,
        }
    }
}

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
        PropDef::object_ref_class("Capacitor", "Capacitor").flags(PropFlags::REQUIRED),
        PropDef::mapped_string_enum("Type", enums.cap_control_type),
        PropDef::double("PTRatio"),
        PropDef::double("CTRatio"),
        PropDef::double("OnSetting"),
        PropDef::double("OffSetting"),
        PropDef::double("Delay"),
        PropDef::boolean("VoltOverride"),
        PropDef::double("VMax").flags(PropFlags::UNITS_V),
        PropDef::double("VMin").flags(PropFlags::UNITS_V),
        PropDef::double("DelayOff").flags(PropFlags::UNITS_S),
        PropDef::double("DeadTime").flags(PropFlags::UNITS_S),
        PropDef::mapped_string_enum("CTPhase", enums.mon_phase),
        PropDef::mapped_string_enum("PTPhase", enums.mon_phase),
        PropDef::string("VBus"),
        PropDef::boolean("EventLog"),
        // WASM_USERMODELS WM.5 — the §2.4 uniform activation rule: a `.wasm`
        // path loads the sandboxed 7-fn CapControl model; a native-DLL name /
        // missing file warns "Not Loaded" (570) and the built-in control solves
        // (`CapUserControl.pas`, `CapControl.pas:429-440`).
        PropDef::string("UserModel").flags(PropFlags::IS_FILENAME),
        PropDef::string("UserData"),
        PropDef::double("pctMinkvar"),
        // Pascal: BooleanActionProperty (DoReset); the getter is always 0.
        PropDef::boolean("Reset").flags(PropFlags::BOOLEAN_ACTION),
        // `ctrlSignalShape` (`CapControl.pas` l.288): a LoadShape ref read by the
        // FOLLOWCONTROL arm of `Sample`. Bound to the LoadShape class
        // (`PropertyOffset2 := ptruint(DSS.LoadShapeClass)`); no WriteByFunction
        // in Pascal (the commented-out `CheckForVar` flag is also inert).
        PropDef::object_ref_class("LoadShape", "ControlSignal"),
        // TCktElementClass tail:
        PropDef::double("BaseFreq").flags(
            PropFlags::DYNAMIC_DEFAULT
                | PropFlags::NON_NEGATIVE
                | PropFlags::NON_ZERO
                | PropFlags::UNITS_HZ,
        ),
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

    /// `ECapControlType` (0=Current ... 5=Follow).
    control_type: CapControlType,
    fct_phase: MonPhase,
    fpt_phase: MonPhase,
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
    pending_change: ControlAction,
    /// `ShouldSwitch`: an action is pending.
    should_switch: bool,
    /// `Armed`: a queue action is outstanding (deleted on disarm).
    armed: bool,
    /// `PresentState`/`InitialState` (CTRL_OPEN/CTRL_CLOSE).
    present_state: ControlAction,
    initial_state: ControlAction,
    /// `VoverrideEvent`.
    voverride_event: bool,
    /// `ControlActionHandle` (the queue handle to delete when disarming).
    control_action_handle: i32,

    // WASM_USERMODELS WM.5 — the 7-function CapControl user model
    // (`UserModel:TCapUserControl`, `CapControl.pas:165-166`):
    /// `UserModelNameStr` — the `.wasm` path exactly as written.
    user_model_name: String,
    /// `UserModelEditStr` — the last `UserData=` string.
    user_model_edit: String,
    /// `IsUserModel` — set true once the model loads (`:432`); forces
    /// `ControlType := USERCONTROL` (`:439-440`).
    is_user_model: bool,
    /// The bound sandboxed model (Pascal `UserModel`). `None` = absent (built-in
    /// control solves). Re-created lazily after a `MakeLike` clone.
    user_model: Option<Box<user_model::CapControlUserModelSlot>>,
    /// Deferred `UserModel=`/`UserData=` load/edit requests queued by the
    /// property side effects, resolved by the executive before `EndEdit`
    /// (`WASM_USERMODELS` §2.4; the setter cannot reach the filesystem).
    pending_user_model_loads: Vec<crate::obj::base::UserModelLoad>,
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
            control_type: CapControlType::Current,
            fct_phase: MonPhase::Phase(1),
            fpt_phase: MonPhase::Phase(1),
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
            pending_change: ControlAction::None,
            should_switch: false,
            armed: false,
            present_state: ControlAction::Close,
            initial_state: ControlAction::Close,
            voverride_event: false,
            control_action_handle: 0,
            user_model_name: String::new(),
            user_model_edit: String::new(),
            is_user_model: false,
            user_model: None,
            pending_user_model_loads: Vec::new(),
        }
    }

    /// Pascal `TCapControlObj.CapControlType` (`Capacitor.pas`... `CapControl.pas:194`
    /// = `ControlType`: `ECapControlType` ordinal, 0=Current … 5=Follow).
    /// Read-only accessor for the CIM export (`RegulatingControlEnum`, GAPS_PLAN
    /// WPG.18 Stage D).
    pub fn control_type(&self) -> i32 {
        self.control_type.ordinal()
    }

    /// Pascal `TCapControlObj.PTPhase` (property `CapControl.pas:207` =
    /// `ControlVars.FPTPhase`; "ALL"/avg/max/min are ≤ 0). Read-only accessor for
    /// the CIM export (`MonitoredPhaseNode`).
    pub fn pt_phase(&self) -> i32 {
        self.fpt_phase.ordinal()
    }

    /// Pascal `TCapControlObj.PTRatioVal` (property `CapControl.pas:199` =
    /// `ControlVars.PTratio`). Read-only accessor for the CIM export.
    pub fn pt_ratio_val(&self) -> f64 {
        self.pt_ratio
    }

    /// Pascal `TCapControlObj.CTRatioVal` (property `CapControl.pas:200` =
    /// `ControlVars.CTratio`). Read-only accessor for the CIM export.
    pub fn ct_ratio_val(&self) -> f64 {
        self.ct_ratio
    }

    /// Pascal `TCapControlObj.OnValue` (property `CapControl.pas:195` =
    /// `ControlVars.ON_Value`). Read-only accessor for the CIM export.
    pub fn on_value(&self) -> f64 {
        self.on_value
    }

    /// Pascal `TCapControlObj.OffValue` (property `CapControl.pas:196` =
    /// `ControlVars.OFF_Value`). Read-only accessor for the CIM export.
    pub fn off_value(&self) -> f64 {
        self.off_value
    }

    /// Pascal `TCapControlObj.PFOnValue` (property `CapControl.pas:197` =
    /// `ControlVars.PFON_Value`). Read-only accessor for the CIM export.
    pub fn pf_on_value(&self) -> f64 {
        self.pfon_value
    }

    /// Pascal `TCapControlObj.PFOffValue` (property `CapControl.pas:198` =
    /// `ControlVars.PFOFF_Value`). Read-only accessor for the CIM export.
    pub fn pf_off_value(&self) -> f64 {
        self.pfoff_value
    }

    /// Pascal `TCapControlObj.OnDelayVal` (property `CapControl.pas:201` =
    /// `ControlVars.OnDelay`). Read-only accessor for the CIM export
    /// (`ShuntCompensator.aVRDelay`).
    pub fn on_delay_val(&self) -> f64 {
        self.on_delay
    }

    /// Pascal `TCapControlObj.Reset` (the `Reset` action property). The
    /// `ControlledElement.Closed[0] := InitialState` restore needs the
    /// controlled capacitor, which the property setter cannot reach; it is
    /// applied by the control-loop reset path ([`Self::reset_with`]) — here we
    /// restore the control's own switching state.
    fn reset(&mut self) {
        self.set_pending_change(ControlAction::None);
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
            ControlAction::Open => Some(false),
            ControlAction::Close => Some(true),
            _ => None,
        };
        if let Some(want) = want_closed {
            cap.set_closed(want);
        }
        self.reset();
        want_closed.is_some()
    }

    /// Pascal `Set_PendingChange` (also mirrors to `DblTraceParameter`, which
    /// stores the raw `EControlAction` ordinal as a Double).
    fn set_pending_change(&mut self, value: ControlAction) {
        self.pending_change = value;
        self.ccd.dbl_trace_param = value.ordinal() as f64;
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

        // Every control type except FOLLOWCONTROL requires a monitored element
        // and uses it as `effElement` (dss_capi `b9bc87b8`: TIMECONTROL now
        // requires + uses the monitored element too — the `<> TIMECONTROL`
        // guard was dropped, `CapControl.pas:581`). b9bc87b8 ports only the
        // guard drop; the message keeps the base 0.14.5 form `%s: Element is
        // not set` (`.inputs/dss_capi` CapControl.pas:601). 0.15.x quotes the
        // object-type name (`%s: "Element" is not set`, capi015 line :584) as
        // part of a separate cross-class error-quoting change we do not adopt
        // here — no deck gates the exact string (substring-checked, default
        // oracle stays 0.14.5).
        let eff = if self.control_type != CapControlType::Follow {
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
            self.ccd.cd.obj.push_error(crate::diag::DssDiagnostic::msg(
                format!(
                "CapControl.{}: Terminal number {} does not exist in \"{}\". Re-specify terminal number. (Error 362)",
                self.ccd.cd.obj.name(),
                self.ccd.element_terminal,
                eff.full_name
                ),
                Some(362),
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
