//! The trait plumbing for `TCapControlObj`: the [`CktElement`] hooks (a control
//! element builds no Yprim and carries zero current) and the [`DssObject`]
//! property accessors (`get_*`/`set_*`, `set_object_ref`, `PropertySideEffects`,
//! `EndEdit`, `MakeLike`).

use num_complex::Complex64;

use crate::elements::general::load_shape::LoadShapeObj;
use crate::elements::pos_seq::{PosSeqCtx, PosSeqPlan};
use crate::elements::traits::{CktElement, ElemId, SysCtx};
use crate::obj::arena::ResolvedObj;
use crate::obj::base::{DssObjData, DssObject};

use super::{CapControl, CapControlType};

impl CktElement for CapControl {
    fn cd(&self) -> &crate::elements::ckt::CktElementData {
        &self.ccd.cd
    }
    fn cd_mut(&mut self) -> &mut crate::elements::ckt::CktElementData {
        &mut self.ccd.cd
    }

    /// Pascal `TControlElem.FControlledElement` - the element this control
    /// acts on (`None` when it drives a list rather than a single element).
    fn controlled_element(&self) -> Option<crate::elements::traits::ElemId> {
        self.ccd.controlled_element
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

    /// Pascal `TCapControlObj.MakePosSequence` (`Controls/CapControl.pas:643`):
    /// adopt the controlled capacitor's enabled state / phase / conductor
    /// counts, then attach terminal 1 to the *effective* element's bus — the
    /// monitored element when set, else the controlled element (forcing
    /// `ElementTerminal := 1`) — and run the base bus rename (`inherited`).
    fn make_pos_sequence(&mut self, ctx: &PosSeqCtx) -> PosSeqPlan {
        if let Some(c) = &ctx.controlled {
            // Enabled := ControlledElement.Enabled (default Set_Enabled).
            self.ccd.cd.set_enabled(c.enabled);
            // FNphases := ControlledElement.NPhases; Nconds := FNphases
            self.ccd.cd.nphases = c.nphases;
            self.ccd.cd.set_nconds(c.nphases);
        }
        // effElement := MonitoredElement if set, else ControlledElement (which
        // forces ElementTerminal := 1).
        let eff = match &ctx.monitored {
            Some(m) => Some(m),
            None => {
                self.ccd.element_terminal = 1;
                ctx.controlled.as_ref()
            }
        };
        if let Some(e) = eff {
            // Setbus(1, effElement.GetBus(ElementTerminal))
            let t = self.ccd.element_terminal as usize;
            let bus = t
                .checked_sub(1)
                .and_then(|k| e.bus_names.get(k))
                .cloned()
                .unwrap_or_default();
            self.ccd.cd.set_bus(1, &bus);
            // ReAllocMem(cBuffer, ..) + ControlVars.CondOffset: no persistent
            // field here — the sampler sizes `cbuffer` and computes `cond_offset`
            // as locals each `Sample` from the live monitored element.
        }
        // inherited MakePosSequence -> base bus rename.
        PosSeqPlan::base()
    }

    /// Pascal `TControlElem.MonitoredElement` — resolved so the exec applier can
    /// build [`PosSeqCtx::monitored`] before calling [`Self::make_pos_sequence`].
    fn monitored_element_ref(&self) -> Option<ElemId> {
        self.ccd.monitored_element
    }
}

impl CapControl {
    /// Pascal `TCapControlObj.MakeLike`.
    pub(crate) fn make_like(&mut self, other: &Self) {
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
        // TODO(compat): Pascal `TCapControlObj.MakeLike` (`CapControl.pas`
        // l.446-490) never copies `ctrlSignalShape`/its name — `Like` on a
        // Follow-type CapControl silently drops the ControlSignal reference on
        // the new object (every other reference/field is copied). Reproduced
        // verbatim: `control_signal_name`/`ctrl_signal_shape` are deliberately
        // left at their `new()` defaults here. Clean fix (post-1:1-port
        // sweep): also copy them like `ctrl_snap`/`mon_snap` above.

        self.ccd.element_terminal = other.ccd.element_terminal;
        self.pt_ratio = other.pt_ratio;
        self.ct_ratio = other.ct_ratio;
        self.control_type = other.control_type;
        self.present_state = other.present_state;
        self.should_switch = other.should_switch;
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
        // WM.5 — Pascal `MakeLike` (`CapControl.pas:481-484`): `UserModel.Name :=
        // Other.UserModel.Name` re-`New`s a fresh instance (the clone drops the
        // live wasmi instance and re-creates it lazily on the next control call).
        self.user_model_name = other.user_model_name.clone();
        self.user_model_edit = other.user_model_edit.clone();
        self.is_user_model = other.is_user_model;
        self.user_model = other.user_model.clone();
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
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn as_ckt_element(&self) -> Option<&dyn CktElement> {
        Some(self)
    }
    fn as_ckt_element_mut(&mut self) -> Option<&mut dyn CktElement> {
        Some(self)
    }
    fn as_control(&self) -> Option<&dyn crate::elements::control::control_elem::ControlElem> {
        Some(self)
    }
    fn as_control_mut(
        &mut self,
    ) -> Option<&mut dyn crate::elements::control::control_elem::ControlElem> {
        Some(self)
    }

    fn get_f64(&self, idx: usize) -> f64 {
        use super::prop::*;
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
        use super::prop::*;
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
        use super::prop::*;
        match idx {
            TERMINAL => self.ccd.element_terminal,
            TYPE => self.control_type.ordinal(),
            CTPHASE => self.fct_phase,
            PTPHASE => self.fpt_phase,
            _ => unreachable!("CapControl has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        use super::prop::*;
        match idx {
            TERMINAL => self.ccd.element_terminal = value,
            TYPE => {
                self.control_type =
                    super::CapControlType::from_ordinal(value).unwrap_or(self.control_type)
            }
            CTPHASE => self.fct_phase = value,
            PTPHASE => self.fpt_phase = value,
            _ => unreachable!("CapControl has no integer property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        use super::prop::*;
        match idx {
            VOLTOVERRIDE => self.voverride,
            EVENTLOG => self.ccd.show_event_log,
            RESET => false, // Pascal BooleanActionProperty getter: always 0
            ENABLED => self.ccd.cd.enabled,
            _ => unreachable!("CapControl has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        use super::prop::*;
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
        use super::prop::*;
        match idx {
            ELEMENT => self.monitored_full_name.clone(),
            CAPACITOR => self.controlled_name.clone(),
            VBUS => self.voverride_bus_name.clone(),
            CONTROLSIGNAL => self.control_signal_name.clone(),
            // WM.5 — the `.wasm` path / `UserData=` string exactly as written.
            USERMODEL => self.user_model_name.clone(),
            USERDATA => self.user_model_edit.clone(),
            _ => unreachable!("CapControl has no string property {idx}"),
        }
    }
    fn set_string(&mut self, idx: usize, value: String) {
        use super::prop::*;
        match idx {
            VBUS => self.voverride_bus_name = value,
            // `UserModel=`/`UserData=` store the string; the deferred load/edit
            // is queued by `side_effects` and resolved by the executive (WM.5
            // §2.4).
            USERMODEL => self.user_model_name = value,
            USERDATA => self.user_model_edit = value,
            _ => unreachable!("CapControl has no string property {idx}"),
        }
    }

    /// `capacitor=` / `element=` resolution: keep the `ElemId`s plus shape
    /// snapshots for `RecalcElementData` (which runs after the foreign view is
    /// gone).
    fn set_object_ref(&mut self, idx: usize, name: String, resolved: Option<ResolvedObj<'_>>) {
        use super::prop::*;
        match idx {
            CAPACITOR => {
                self.controlled_name = name;
                match resolved {
                    Some(o) => {
                        self.ccd.controlled_element = Some(o.id());
                        let elem = o.ckt().expect("Capacitor is a circuit element");
                        self.ctrl_snap = Some(super::RefSnapshot::capture(
                            format!("Capacitor.{}", o.name()),
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
                    Some(o) => {
                        self.ccd.monitored_element = Some(o.id());
                        let elem = o.ckt().expect("element= resolves against circuit classes");
                        self.mon_snap = Some(super::RefSnapshot::capture(name, elem));
                    }
                    None => {
                        self.ccd.monitored_element = None;
                        self.mon_snap = None;
                    }
                }
            }
            // `ctrlSignalShape` (`CapControl.pas` l.289): snapshot-clone the
            // resolved LoadShapeObj (the `StorageController` Yearly/Daily/Duty
            // pattern) — Pascal reads it live through a raw pointer, which
            // `Sample` (running well after `EndEdit`) can no longer borrow.
            CONTROLSIGNAL => {
                self.control_signal_name = name;
                self.ctrl_signal_shape = resolved.and_then(|o| o.cloned::<LoadShapeObj>());
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
        use super::prop::*;
        // PF Controller changes (the type has already been written when the
        // `typ` side effect runs, so this covers "switched to PF" too).
        if self.control_type == CapControlType::Pf {
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
                self.voverride_bus_name = self.voverride_bus_name.to_ascii_lowercase();
                self.voverride_bus_specified = true;
            }
            // WM.5 — the §2.4 uniform activation rule. Pascal
            // `PropertySideEffects` (`CapControl.pas:429-436`): `UserModel.Name`
            // (load) then `UserData` (edit). The filesystem/current-dir are
            // unreachable from the property hook, so each records a deferred
            // request the executive resolves before `EndEdit` (a `.wasm` loads
            // and sets `IsUserModel`; a native-DLL name / missing file warns
            // "Not Loaded" 570 and falls back — `apply_user_model_load_impl`).
            USERMODEL => self.queue_user_model_load(self.user_model_name.clone()),
            USERDATA => self.queue_user_model_edit(self.user_model_edit.clone()),
            _ => {}
        }

        // Pascal `if IsUserModel then ControlType := USERCONTROL` runs at the end
        // of every `PropertySideEffects` (`:439-440`): once a model is loaded,
        // any later property edit re-forces USERCONTROL. `IsUserModel` is set
        // when the deferred load resolves (`apply_user_model_load_impl`); a
        // same-batch `UserModel=` load forces the type there, so this trailing
        // re-force only matters for edits after the model already exists.
        if self.is_user_model {
            self.control_type = CapControlType::UserControl;
        }
    }

    /// Pascal `TCktElementClass.EndEdit` default → `RecalcElementData`.
    fn end_edit(&mut self, _sys: &crate::elements::traits::SysCtx) {
        self.recalc();
    }

    /// Drain the deferred `UserModel=`/`UserData=` requests queued by the
    /// property side effects (WASM_USERMODELS WM.5, §2.4). The executive
    /// resolves each `Load` path and reads the `.wasm` bytes (or `None`), then
    /// calls [`Self::apply_user_model_load`] before `end_edit`.
    fn take_user_model_loads(&mut self) -> Vec<crate::obj::base::UserModelLoad> {
        self.take_user_model_loads()
    }

    /// Apply a resolved user-model load/edit (the data side of a queued
    /// `UserModelLoad`) — WASM_USERMODELS WM.5 §2.4.
    fn apply_user_model_load(
        &mut self,
        load: &crate::obj::base::UserModelLoad,
        wasm: Option<&[u8]>,
        _sys: &crate::elements::traits::SysCtx,
        errors: &mut crate::diag::ErrorLog,
    ) {
        // CapControl's user model is a control model; its recalc reads no live
        // `ActiveCircuit.Solution` globals, so the live snapshot is ignored.
        self.apply_user_model_load_impl(load, wasm, errors);
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}

impl crate::elements::control::control_elem::ControlElem for CapControl {
    fn ccd(&self) -> &crate::elements::control::control_elem::ControlElemData {
        &self.ccd
    }
    fn ccd_mut(&mut self) -> &mut crate::elements::control::control_elem::ControlElemData {
        &mut self.ccd
    }
    fn control_kind(&self) -> crate::elements::control::control_elem::ControlClass {
        crate::elements::control::control_elem::ControlClass::Cap
    }
}
