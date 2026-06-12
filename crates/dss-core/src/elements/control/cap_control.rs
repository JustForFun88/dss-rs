//! Port of `Controls/CapControl.pas` — `TCapControlObj`, the capacitor-bank
//! switching control. **Phase 4 ports the parse-time surface only**
//! (properties, `capacitor=`/`element=` resolution, `RecalcElementData`'s
//! bus/phase setup, `MakeLike`); the `Sample`/`DoPendingAction` switching
//! machinery is Phase 5 (PHASE4_PLAN §WP4.7).
//!
//! Like every `TControlElem`, a CapControl builds **no Yprim** and its terminal
//! currents are zero; its single terminal attaches to the monitored element's
//! terminal bus (or the capacitor's, for Time/Follow control types).

use num_complex::Complex64;

use crate::elements::control::control_elem::{ControlElemData, RefSnapshot};
use crate::elements::traits::{CktElement, ElemRef, SysCtx};
use crate::obj::base::{DssObjData, DssObject};
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};

/// `ECapControlType` ordinals.
mod ctrl_type {
    pub const _CURRENT: i32 = 0;
    pub const _VOLTAGE: i32 = 1;
    pub const _KVAR: i32 = 2;
    pub const TIME: i32 = 3;
    pub const PF: i32 = 4;
    pub const FOLLOW: i32 = 5;
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
        // LoadShape references arrive in Phase 5 (WP5.1); `Follow` mode needs it.
        PropDef::object_ref("ControlSignal").flags(PropFlags::NOT_PORTED),
        // TCktElementClass tail:
        PropDef::double("basefreq").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::enabled("enabled"),
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
    // Phase-5 runtime state (kept so `Reset` is faithful; the full
    // `TCapControlVars` switching state arrives with Sample in Phase 5):
    should_switch: bool,
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
            control_type: ctrl_type::_CURRENT,
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
            should_switch: false,
        }
    }

    /// Pascal `TCapControlObj.Reset` (the `Reset` action property), local
    /// state only: the `ControlledElement.Closed[0] := InitialState` write is
    /// Phase 5 (capacitor terminals never move during the parse-only phase, so
    /// present state == initial state == closed throughout).
    fn reset(&mut self) {
        self.should_switch = false;
        self.last_open_time = -self.dead_time;
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

impl CktElement for CapControl {
    fn cd(&self) -> &crate::elements::ckt::CktElementData {
        &self.ccd.cd
    }
    fn cd_mut(&mut self) -> &mut crate::elements::ckt::CktElementData {
        &mut self.ccd.cd
    }

    fn recalc_element_data(&mut self, _sys: &SysCtx) {
        self.recalc();
    }

    /// Pascal `TControlElem.CalcYPrim`: leave YPrim as NIL — `BuildYMatrix`
    /// skips elements with no primitive matrix.
    fn calc_yprim(&mut self, _sys: &SysCtx) {}

    /// Pascal `TControlElem.GetCurrents`: always zero.
    fn get_currents(&mut self, _sys: &SysCtx, _node_v: &[Complex64], curr: &mut [Complex64]) {
        curr.fill(Complex64::ZERO);
    }
}

impl DssObject for CapControl {
    fn data(&self) -> &DssObjData {
        &self.ccd.cd.obj
    }
    fn data_mut(&mut self) -> &mut DssObjData {
        &mut self.ccd.cd.obj
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_ckt_element(&self) -> Option<&dyn CktElement> {
        Some(self)
    }
    fn as_ckt_element_mut(&mut self) -> Option<&mut dyn CktElement> {
        Some(self)
    }

    fn get_f64(&self, idx: usize) -> f64 {
        use prop::*;
        match idx {
            PTRATIO => self.pt_ratio,
            CTRATIO => self.ct_ratio,
            ONSETTING => self.on_value,
            OFFSETTING => self.off_value,
            DELAY => self.on_delay,
            VMAX => self.vmax,
            VMIN => self.vmin,
            DELAYOFF => self.off_delay,
            DEADTIME => self.dead_time,
            PCTMINKVAR => self.fpct_minkvar,
            BASE_FREQ => self.ccd.cd.base_frequency,
            _ => unreachable!("CapControl has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        use prop::*;
        match idx {
            PTRATIO => self.pt_ratio = value,
            CTRATIO => self.ct_ratio = value,
            ONSETTING => self.on_value = value,
            OFFSETTING => self.off_value = value,
            DELAY => self.on_delay = value,
            VMAX => self.vmax = value,
            VMIN => self.vmin = value,
            DELAYOFF => self.off_delay = value,
            DEADTIME => self.dead_time = value,
            PCTMINKVAR => self.fpct_minkvar = value,
            BASE_FREQ => self.ccd.cd.base_frequency = value,
            _ => unreachable!("CapControl has no double property {idx}"),
        }
    }

    fn get_i32(&self, idx: usize) -> i32 {
        use prop::*;
        match idx {
            TERMINAL => self.ccd.element_terminal,
            TYPE => self.control_type,
            CTPHASE => self.fct_phase,
            PTPHASE => self.fpt_phase,
            _ => unreachable!("CapControl has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        use prop::*;
        match idx {
            TERMINAL => self.ccd.element_terminal = value,
            TYPE => self.control_type = value,
            CTPHASE => self.fct_phase = value,
            PTPHASE => self.fpt_phase = value,
            _ => unreachable!("CapControl has no integer property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        use prop::*;
        match idx {
            VOLTOVERRIDE => self.voverride,
            EVENTLOG => self.ccd.show_event_log,
            RESET => false, // Pascal BooleanActionProperty getter: always 0
            ENABLED => self.ccd.cd.enabled,
            _ => unreachable!("CapControl has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        use prop::*;
        match idx {
            VOLTOVERRIDE => self.voverride = value,
            EVENTLOG => self.ccd.show_event_log = value,
            RESET => {
                // Pascal BooleanActionProperty: the action fires on TRUE only.
                if value {
                    self.reset();
                }
            }
            // Pascal `TCapControlObj.Set_Enabled` override: only toggle the
            // flag — no BusNameRedefined side effect.
            ENABLED => self.ccd.cd.enabled = value,
            _ => unreachable!("CapControl has no boolean property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        use prop::*;
        match idx {
            ELEMENT => self.monitored_full_name.clone(),
            CAPACITOR => self.controlled_name.clone(),
            VBUS => self.voverride_bus_name.clone(),
            // NOT_PORTED user-model / control-signal slots dump as empty (NIL).
            USERMODEL | USERDATA | CONTROLSIGNAL => String::new(),
            _ => unreachable!("CapControl has no string property {idx}"),
        }
    }
    fn set_string(&mut self, idx: usize, value: String) {
        match idx {
            prop::VBUS => self.voverride_bus_name = value,
            _ => unreachable!("CapControl has no string property {idx}"),
        }
    }

    /// `capacitor=` / `element=` resolution: keep the `ElemRef`s plus shape
    /// snapshots for `RecalcElementData` (which runs after the foreign view is
    /// gone).
    fn set_object_ref(
        &mut self,
        idx: usize,
        name: String,
        resolved: Option<(ElemRef, &dyn DssObject)>,
    ) {
        use prop::*;
        match idx {
            CAPACITOR => {
                self.controlled_name = name;
                match resolved {
                    Some((r, obj)) => {
                        self.ccd.controlled_element = Some(r);
                        let elem = obj
                            .as_ckt_element()
                            .expect("Capacitor is a circuit element");
                        self.ctrl_snap = Some(RefSnapshot::capture(
                            format!("Capacitor.{}", obj.data().name()),
                            elem,
                        ));
                    }
                    None => {
                        self.ccd.controlled_element = None;
                        self.ctrl_snap = None;
                    }
                }
            }
            ELEMENT => {
                // `name` is the FullName ("Class.name") for the dump.
                self.monitored_full_name = name.clone();
                match resolved {
                    Some((r, obj)) => {
                        self.ccd.monitored_element = Some(r);
                        let elem = obj
                            .as_ckt_element()
                            .expect("element= resolves against circuit classes");
                        self.mon_snap = Some(RefSnapshot::capture(name, elem));
                    }
                    None => {
                        self.ccd.monitored_element = None;
                        self.mon_snap = None;
                    }
                }
            }
            _ => unreachable!("CapControl has no object-ref property {idx}"),
        }
    }

    fn set_bus_name(&mut self, terminal: usize, value: &str) {
        self.ccd.cd.set_bus(terminal, value);
    }
    fn get_bus_name(&self, terminal: usize) -> String {
        self.ccd.cd.get_bus(terminal).to_string()
    }

    /// Pascal `TCapControlObj.PropertySideEffects`.
    fn side_effects(&mut self, idx: usize, _prev_int: i32) {
        use prop::*;
        // PF Controller changes (the type has already been written when the
        // `typ` side effect runs, so this covers "switched to PF" too).
        if self.control_type == ctrl_type::PF {
            match idx {
                TYPE => {
                    self.pfon_value = 0.95; // defaults
                    self.pfoff_value = 1.05;
                }
                ONSETTING => {
                    if (-1.0..=1.0).contains(&self.on_value) {
                        self.pfon_value = if self.on_value < 0.0 {
                            2.0 + self.on_value
                        } else {
                            self.on_value
                        };
                    } else {
                        self.ccd.cd.obj.push_error(format!(
                            "Invalid PF ON value for \"CapControl.{}\"",
                            self.ccd.cd.obj.name()
                        ));
                    }
                }
                OFFSETTING => {
                    if (-1.0..=1.0).contains(&self.off_value) {
                        self.pfoff_value = if self.off_value < 0.0 {
                            2.0 + self.off_value
                        } else {
                            self.off_value
                        };
                    } else {
                        self.ccd.cd.obj.push_error(format!(
                            "Invalid PF OFF value for \"CapControl.{}\"",
                            self.ccd.cd.obj.name()
                        ));
                    }
                }
                _ => {}
            }
        }

        match idx {
            CTPHASE => {
                if self.fct_phase > self.ccd.cd.nphases as i32 {
                    self.ccd.cd.obj.push_error(format!(
                        "Error: Monitored phase ({}) must be less than or equal to number of phases ({}). ",
                        self.fct_phase, self.ccd.cd.nphases
                    ));
                    self.fct_phase = 1;
                }
            }
            PTPHASE => {
                if self.fpt_phase > self.ccd.cd.nphases as i32 {
                    self.ccd.cd.obj.push_error(format!(
                        "Error: Monitored phase ({}) must be less than or equal to number of phases ({}). ",
                        self.fpt_phase, self.ccd.cd.nphases
                    ));
                    self.fpt_phase = 1;
                }
            }
            // CAPACITOR: Pascal stores ControlVars.CapacitorName :=
            // ControlledElement.FullName for Save; Save is not ported.
            VBUS => {
                self.voverride_bus_name = self.voverride_bus_name.to_lowercase();
                self.voverride_bus_specified = true;
            }
            // USERMODEL/USERDATA are NOT_PORTED (hard parse error upstream of
            // this hook), so the user-model wiring never runs.
            _ => {}
        }
    }

    /// Pascal `TCktElementClass.EndEdit` default → `RecalcElementData`.
    fn end_edit(&mut self) {
        self.recalc();
    }

    /// Pascal `TCapControlObj.MakeLike`.
    fn make_like(&mut self, other: &dyn DssObject) {
        let Some(other) = other.as_any().downcast_ref::<CapControl>() else {
            return;
        };
        self.ccd.cd.make_like_base(&other.ccd.cd);
        self.ccd.cd.nphases = other.ccd.cd.nphases;
        let nc = other.ccd.cd.nconds;
        self.ccd.cd.set_nconds(nc); // Force Reallocation of terminal stuff

        self.ccd.controlled_element = other.ccd.controlled_element;
        self.ccd.monitored_element = other.ccd.monitored_element;
        self.controlled_name = other.controlled_name.clone();
        self.monitored_full_name = other.monitored_full_name.clone();
        self.ctrl_snap = other.ctrl_snap.clone();
        self.mon_snap = other.mon_snap.clone();

        self.ccd.element_terminal = other.ccd.element_terminal;
        self.pt_ratio = other.pt_ratio;
        self.ct_ratio = other.ct_ratio;
        self.control_type = other.control_type;
        self.on_value = other.on_value;
        self.off_value = other.off_value;
        self.pfon_value = other.pfon_value;
        self.pfoff_value = other.pfoff_value;
        self.fct_phase = other.fct_phase;
        self.fpt_phase = other.fpt_phase;
        self.voverride = other.voverride;
        self.voverride_bus_specified = other.voverride_bus_specified;
        self.voverride_bus_name = other.voverride_bus_name.clone();
        self.fpct_minkvar = other.fpct_minkvar;
        self.ccd.show_event_log = other.ccd.show_event_log;
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_shape_is_3ph_1term() {
        let cc = CapControl::new("cc1");
        assert_eq!(cc.ccd.cd.nphases, 3);
        assert_eq!(cc.ccd.cd.nconds, 3);
        assert_eq!(cc.ccd.cd.nterms, 1);
        assert_eq!(cc.control_type, 0); // Current
        assert_eq!(cc.pt_ratio, 60.0);
        assert_eq!(cc.ct_ratio, 60.0);
        assert_eq!(cc.on_value, 300.0);
        assert_eq!(cc.off_value, 200.0);
        assert_eq!(cc.on_delay, 15.0);
        assert_eq!(cc.off_delay, 15.0);
        assert_eq!(cc.dead_time, 300.0);
        assert_eq!(cc.vmax, 126.0);
        assert_eq!(cc.vmin, 115.0);
        assert_eq!(cc.fpct_minkvar, 50.0);
        assert!(cc.ccd.cd.yprim.is_none());
    }

    #[test]
    fn time_control_forces_terminal_1() {
        // Probed: `type=time terminal=2` dumps Terminal = 1.
        let mut cc = CapControl::new("cc1");
        cc.ccd.controlled_element = Some(ElemRef { cls: 0, idx: 0 });
        cc.ctrl_snap = Some(RefSnapshot {
            full_name: "Capacitor.cap1".into(),
            nphases: 3,
            nterms: 2,
            buses: vec!["b2.1.2.3".into(), "b2.0.0.0".into()],
        });
        cc.control_type = ctrl_type::TIME;
        cc.ccd.element_terminal = 2;
        cc.recalc();
        assert_eq!(cc.ccd.element_terminal, 1);
        assert_eq!(cc.ccd.cd.get_bus(1), "b2.1.2.3");
        assert!(cc.ccd.cd.obj.take_errors().is_empty());
    }

    #[test]
    fn pf_mode_translates_on_off_settings() {
        // Probed: type=pf onsetting=0.97 offsetting=-0.99 keeps the raw dump
        // values; internally PFON=0.97, PFOFF=2-0.99=1.01.
        let mut cc = CapControl::new("cc1");
        cc.control_type = ctrl_type::PF;
        cc.side_effects(prop::TYPE, 0);
        assert_eq!(cc.pfon_value, 0.95);
        assert_eq!(cc.pfoff_value, 1.05);
        cc.on_value = 0.97;
        cc.side_effects(prop::ONSETTING, 0);
        assert!((cc.pfon_value - 0.97).abs() < 1e-12);
        cc.off_value = -0.99;
        cc.side_effects(prop::OFFSETTING, 0);
        assert!((cc.pfoff_value - 1.01).abs() < 1e-12);
        assert_eq!(cc.on_value, 0.97);
        assert_eq!(cc.off_value, -0.99);
    }
}
