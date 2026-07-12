//! The `CktElement` and `DssObject` trait impls for `InvControl` (step 2a): the
//! typed property accessors, the curve `set_object_ref` resolution +
//! `ValidateXYCurve`, `PropertySideEffects`, and `MakeLike`. The DER-fleet build
//! and the control behavior (`Sample`/`DoPendingAction`/`RecalcElementData`)
//! land in step 2b.

use num_complex::Complex64;

use crate::elements::general::xy_curve::XyCurveObj;
use crate::elements::pos_seq::{PosSeqCtx, PosSeqPlan};
use crate::elements::traits::{CktElement, ElemRef, SysCtx};
use crate::obj::base::{DssObjData, DssObject};

use super::{InvControl, VOLTWATT, WATTPF, WATTVAR, prop};

/// Pascal `ValidateXYCurve(curve, mode)`: VOLTWATT requires the per-unit Y
/// values in `[0, 1]`; WATTPF/WATTVAR require `[-1, 1]`. A violating curve is
/// dropped (error 381). VOLTVAR is unchecked. Returns `false` (and the caller
/// nils the curve + name) when the curve is invalid.
// The explicit `y < lo || y > hi` reads clearer than clippy's negated
// `!(lo..=hi).contains(&y)` (a double negative), and matches the Pascal
// `(y > 1.0) or (y < -1.0)` bound test literally.
#[allow(clippy::manual_range_contains)]
fn validate_xy_curve(curve: &XyCurveObj, mode: i32) -> Result<(), ()> {
    let ys = curve.y_values();
    match mode {
        VOLTWATT => {
            if ys.iter().any(|&y| y < 0.0 || y > 1.0) {
                return Err(());
            }
        }
        WATTPF | WATTVAR => {
            if ys.iter().any(|&y| y < -1.0 || y > 1.0) {
                return Err(());
            }
        }
        _ => {}
    }
    Ok(())
}

impl InvControl {
    /// The parse-time tail of Pascal `RecalcElementData`: with the first DER's bus
    /// resolved (by the executive at edit-completion), set the control's phase count
    /// and attach its single terminal to that bus — Pascal `Setbus(1,
    /// MonitoredElement.Firstbus)`. Without a resolved DER (no fleet member found at
    /// parse time) the terminal stays unset, exactly like a control whose monitored
    /// element is missing.
    ///
    /// `mon_nphases` is the resolved DER's phase count (Pascal sets `FNphases :=
    /// ControlledElement[i].NPhases` over the recalc loop — the *last* DER's; for a
    /// single-DER / homogeneous fleet, the same as the first DER's bus phases).
    pub(crate) fn recalc(&mut self) {
        if !self.mon_resolved {
            return;
        }
        self.ccd.cd.nphases = self.mon_nphases;
        self.ccd.cd.set_nconds(self.mon_nphases);
        let bus = self.mon_bus.clone();
        self.ccd.cd.set_bus(1, &bus);
    }

    /// Pascal `FDERPointerList.Clear` — drop the resolved fleet so the next
    /// `Sample` rebuilds it (an empty fleet re-triggers the build; a DERList /
    /// PVSystemList edit changes the fleet).
    fn invalidate_fleet(&mut self) {
        self.fleet.clear();
        self.ctrl_vars.clear();
    }

    /// Run `ValidateXYCurve` against one resolved curve, nilling it (curve +
    /// name) and pushing error 381 when invalid. The message text matches the
    /// Pascal per mode.
    fn validate_curve(&mut self, idx: usize) {
        use prop::*;
        let (mode, what) = match idx {
            VOLTWATT_CURVE | VOLTWATTCH_CURVE => (
                VOLTWATT,
                "active power value(s) greater than 1.0 per-unit or less than -1.0 per-unit.  \
                 Not allowed for VOLTWATT control mode for PVSystem/Storages",
            ),
            WATTPF_CURVE => (
                WATTPF,
                "power factor value(s) greater than 1.0 or less than -1.0.  \
                 Not allowed for WATTPF control mode for PVSystem/Storages",
            ),
            WATTVAR_CURVE => (
                WATTVAR,
                "reactive power value(s) greater than 1.0 per-unit or less than -1.0 per-unit.  \
                 Not allowed for WATTVAR control mode for PVSystem/Storages",
            ),
            _ => return,
        };
        let (curve_name, ok) = match self.curve_obj(idx) {
            Some(c) => (
                c.data().name().to_string(),
                validate_xy_curve(c, mode).is_ok(),
            ),
            None => return,
        };
        if !ok {
            self.set_curve(idx, String::new(), None);
            self.ccd
                .cd
                .obj
                .push_error(format!("XY Curve object: \"{curve_name}\" has {what}"));
        }
    }

    fn curve_obj(&self, idx: usize) -> Option<&XyCurveObj> {
        use prop::*;
        match idx {
            VVC_CURVE1 => self.vvc_curve.as_ref(),
            VOLTWATT_CURVE => self.voltwatt_curve.as_ref(),
            VOLTWATTCH_CURVE => self.voltwattch_curve.as_ref(),
            WATTPF_CURVE => self.wattpf_curve.as_ref(),
            WATTVAR_CURVE => self.wattvar_curve.as_ref(),
            _ => None,
        }
    }

    fn set_curve(&mut self, idx: usize, name: String, obj: Option<XyCurveObj>) {
        use prop::*;
        match idx {
            VVC_CURVE1 => {
                self.vvc_curve_name = name;
                self.vvc_curve = obj;
            }
            VOLTWATT_CURVE => {
                self.voltwatt_curve_name = name;
                self.voltwatt_curve = obj;
            }
            VOLTWATTCH_CURVE => {
                self.voltwattch_curve_name = name;
                self.voltwattch_curve = obj;
            }
            WATTPF_CURVE => {
                self.wattpf_curve_name = name;
                self.wattpf_curve = obj;
            }
            WATTVAR_CURVE => {
                self.wattvar_curve_name = name;
                self.wattvar_curve = obj;
            }
            _ => unreachable!("InvControl has no curve property {idx}"),
        }
    }
}

impl CktElement for InvControl {
    fn cd(&self) -> &crate::elements::ckt::CktElementData {
        &self.ccd.cd
    }
    fn cd_mut(&mut self) -> &mut crate::elements::ckt::CktElementData {
        &mut self.ccd.cd
    }

    /// Pascal `TControlElem.FControlledElement` - the element this control
    /// acts on (`None` when it drives a list rather than a single element).
    fn controlled_element(&self) -> Option<crate::elements::traits::ElemRef> {
        self.ccd.controlled_element
    }

    /// Pascal `TInvControlObj.RecalcElementData` (the parse-time subset): attach
    /// the control's terminal to the first DER's bus. The fleet *dispatch* build
    /// (`MakeDERList` + `UpdateDERParameters`) needs store access, so it is deferred
    /// to the first `Sample`; the bus is resolved at edit-completion instead (the
    /// executive calls [`set_resolved_monitored`](InvControl::set_resolved_monitored)
    /// before `end_edit` → `recalc`).
    fn recalc_element_data(&mut self, _sys: &SysCtx) {
        self.recalc();
    }

    /// Pascal `TControlElem.CalcYPrim`: leave YPrim NIL.
    fn calc_yprim(&mut self, _sys: &SysCtx) {}

    /// Pascal `TControlElem.GetCurrents`: always zero.
    fn get_currents(&mut self, _sys: &SysCtx, _node_v: &[Complex64], curr: &mut [Complex64]) {
        curr.fill(Complex64::ZERO);
    }

    /// Pascal `TInvControlObj.MakePosSequence` (`Controls/InvControl.pas:943`).
    /// **NIL-deref hazard** (Access violation #303, probes `1`/`S1`,
    /// `docs/wpg21_makeposseq_probes.md`): with an empty DER list Pascal's
    /// `Setbus(1, MonitoredElement.GetBus(..))` dereferences the NIL
    /// `MonitoredElement`; in dss-python 0.15.7 even the *populated* case faults
    /// (at object creation). Per CLAUDE.md UB is never reproduced. We transcribe
    /// the *defined* scalar resync (`FNphases := 3; Nconds := 3`) and safe-skip
    /// the NIL-deref `Setbus` when no monitored element is resolved; when a
    /// monitored DER *is* resolved (the populated path), adopt its `Firstbus` /
    /// phase count. The empty-list `RecalcElementData` (DER-list rebuild) needs
    /// solve context and sits on the crash path, so it is not reproduced here.
    fn make_pos_sequence(&mut self, ctx: &PosSeqCtx) -> PosSeqPlan {
        // FNphases := 3; Nconds := 3 (defined; independent of the DER list).
        self.ccd.cd.nphases = 3;
        self.ccd.cd.set_nconds(3);
        if let Some(m) = &ctx.monitored {
            // Populated path: MonitoredElement := 1st DER; Setbus(1,
            // MonitoredElement.Firstbus); FNphases := MonitoredElement.NPhases;
            // Nconds := Nphases.
            let bus = m.bus_names.first().cloned().unwrap_or_default();
            self.ccd.cd.set_bus(1, &bus);
            self.ccd.cd.nphases = m.nphases;
            self.ccd.cd.set_nconds(m.nphases);
        }
        // else: MonitoredElement is NIL — Pascal's Setbus derefs it and faults
        // (#303). Safe-skip the Setbus (the 3/3 resync above still applies).
        // inherited MakePosSequence -> base bus rename.
        PosSeqPlan::base()
    }

    /// Pascal `TControlElem.MonitoredElement` — resolved so the exec applier can
    /// build [`PosSeqCtx::monitored`] before calling [`Self::make_pos_sequence`].
    fn monitored_element_ref(&self) -> Option<ElemRef> {
        self.ccd.monitored_element
    }
}

impl DssObject for InvControl {
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

    fn get_f64(&self, idx: usize) -> f64 {
        use prop::*;
        match idx {
            HYSTERESIS_OFFSET => self.vvc_curve_offset,
            DBV_MIN => self.dbv_min,
            DBV_MAX => self.dbv_max,
            AR_GRA_LOW_V => self.ar_gra_low_v,
            AR_GRA_HI_V => self.ar_gra_hi_v,
            DELTA_Q_FACTOR => self.delta_q_factor,
            VOLTAGE_CHANGE_TOLERANCE => self.voltage_change_tolerance,
            VAR_CHANGE_TOLERANCE => self.var_change_tolerance,
            LPF_TAU => self.lpf_tau,
            RISE_FALL_LIMIT => self.rise_fall_limit,
            DELTA_P_FACTOR => self.delta_p_factor,
            ACTIVE_P_CHANGE_TOLERANCE => self.active_p_change_tolerance,
            VSETPOINT => self.v_setpoint,
            BASE_FREQ => self.ccd.cd.base_frequency,
            _ => unreachable!("InvControl has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        use prop::*;
        match idx {
            HYSTERESIS_OFFSET => self.vvc_curve_offset = value,
            DBV_MIN => self.dbv_min = value,
            DBV_MAX => self.dbv_max = value,
            AR_GRA_LOW_V => self.ar_gra_low_v = value,
            AR_GRA_HI_V => self.ar_gra_hi_v = value,
            DELTA_Q_FACTOR => self.delta_q_factor = value,
            VOLTAGE_CHANGE_TOLERANCE => self.voltage_change_tolerance = value,
            VAR_CHANGE_TOLERANCE => self.var_change_tolerance = value,
            LPF_TAU => self.lpf_tau = value,
            RISE_FALL_LIMIT => self.rise_fall_limit = value,
            DELTA_P_FACTOR => self.delta_p_factor = value,
            ACTIVE_P_CHANGE_TOLERANCE => self.active_p_change_tolerance = value,
            VSETPOINT => self.v_setpoint = value,
            BASE_FREQ => self.ccd.cd.base_frequency = value,
            _ => unreachable!("InvControl has no double property {idx}"),
        }
    }

    fn get_i32(&self, idx: usize) -> i32 {
        use prop::*;
        match idx {
            MODE => self.control_mode,
            COMBI_MODE => self.combi_mode,
            VOLTAGE_CURVEX_REF => self.voltage_curvex_ref,
            AVG_WINDOW_LEN => self.roll_avg_window_length,
            DYN_REAC_AVG_WINDOW_LEN => self.drc_roll_avg_window_length,
            VOLTWATT_YAXIS => self.voltwatt_yaxis,
            RATE_OF_CHANGE_MODE => self.rate_of_change_mode,
            REF_REACTIVE_POWER => self.reac_power_ref,
            MON_VOLTAGE_CALC => self.mon_buses_phase,
            CONTROL_MODEL => self.ctrl_model,
            _ => unreachable!("InvControl has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        use prop::*;
        match idx {
            MODE => self.control_mode = value,
            COMBI_MODE => self.combi_mode = value,
            VOLTAGE_CURVEX_REF => self.voltage_curvex_ref = value,
            AVG_WINDOW_LEN => self.roll_avg_window_length = value,
            DYN_REAC_AVG_WINDOW_LEN => self.drc_roll_avg_window_length = value,
            VOLTWATT_YAXIS => self.voltwatt_yaxis = value,
            RATE_OF_CHANGE_MODE => self.rate_of_change_mode = value,
            REF_REACTIVE_POWER => self.reac_power_ref = value,
            MON_VOLTAGE_CALC => self.mon_buses_phase = value,
            CONTROL_MODEL => self.ctrl_model = value,
            _ => unreachable!("InvControl has no integer property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        match idx {
            prop::EVENT_LOG => self.ccd.show_event_log,
            prop::ENABLED => self.ccd.cd.enabled,
            _ => unreachable!("InvControl has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        match idx {
            prop::EVENT_LOG => self.ccd.show_event_log = value,
            prop::ENABLED => self.ccd.cd.enabled = value,
            _ => unreachable!("InvControl has no boolean property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        use prop::*;
        match idx {
            VVC_CURVE1 => self.vvc_curve_name.clone(),
            VOLTWATT_CURVE => self.voltwatt_curve_name.clone(),
            VOLTWATTCH_CURVE => self.voltwattch_curve_name.clone(),
            WATTPF_CURVE => self.wattpf_curve_name.clone(),
            WATTVAR_CURVE => self.wattvar_curve_name.clone(),
            // Pascal DeprecatedAndRemoved getter renders ''.
            VV_REF_REACTIVE_POWER => String::new(),
            _ => unreachable!("InvControl has no string property {idx}"),
        }
    }
    fn set_string(&mut self, idx: usize, _value: String) {
        match idx {
            // DeprecatedAndRemoved — writes do nothing.
            prop::VV_REF_REACTIVE_POWER => {}
            _ => unreachable!("InvControl has no writable string property {idx}"),
        }
    }

    fn get_string_list(&self, idx: usize) -> Vec<String> {
        match idx {
            // PVSystemList shares the DERNameList backing (PropertyOffset).
            prop::DER_LIST | prop::PVSYSTEM_LIST => self.der_name_list.clone(),
            prop::MON_BUS => self.mon_buses_name_list.clone(),
            _ => unreachable!("InvControl has no string-list property {idx}"),
        }
    }
    fn set_string_list(&mut self, idx: usize, value: Vec<String>) {
        match idx {
            prop::DER_LIST | prop::PVSYSTEM_LIST => self.der_name_list = value,
            prop::MON_BUS => self.mon_buses_name_list = value,
            _ => unreachable!("InvControl has no string-list property {idx}"),
        }
    }

    fn get_f64_array(&self, idx: usize) -> Option<&[f64]> {
        match idx {
            // Pascal FMonBusesVbase is NIL until MonBus allocates it; a NIL array
            // dumps as '' (not '[]'), so report empty as absent.
            prop::MON_BUSES_VBASE => {
                (!self.mon_buses_vbase.is_empty()).then_some(self.mon_buses_vbase.as_slice())
            }
            _ => unreachable!("InvControl has no double-array property {idx}"),
        }
    }
    fn set_f64_array(&mut self, idx: usize, value: Vec<f64>) {
        match idx {
            prop::MON_BUSES_VBASE => self.mon_buses_vbase = value,
            _ => unreachable!("InvControl has no double-array property {idx}"),
        }
    }
    /// Pascal `MonBusesVBase` SizeIsFunction: the element count is the
    /// monitored-bus-name-list length.
    fn array_size(&self, idx: usize) -> usize {
        match idx {
            prop::MON_BUSES_VBASE => self.mon_buses_name_list.len(),
            _ => unreachable!("InvControl has no function-sized array {idx}"),
        }
    }

    /// The five control curves resolve against XYcurve (snapshot-clone). The
    /// per-mode range check runs in `side_effects` (`ValidateXYCurve`).
    fn set_object_ref(
        &mut self,
        idx: usize,
        name: String,
        resolved: Option<(ElemRef, &dyn DssObject)>,
    ) {
        let obj = resolved.and_then(|(_, o)| o.as_any().downcast_ref::<XyCurveObj>().cloned());
        match idx {
            prop::VVC_CURVE1
            | prop::VOLTWATT_CURVE
            | prop::VOLTWATTCH_CURVE
            | prop::WATTPF_CURVE
            | prop::WATTVAR_CURVE => self.set_curve(idx, name, obj),
            _ => unreachable!("InvControl has no object-ref property {idx}"),
        }
    }

    fn set_bus_name(&mut self, terminal: usize, value: &str) {
        self.ccd.cd.set_bus(terminal, value);
    }
    fn get_bus_name(&self, terminal: usize) -> String {
        self.ccd.cd.get_bus(terminal).to_string()
    }

    /// Pascal `TInvControlObj.PropertySideEffects`.
    fn side_effects(&mut self, idx: usize, _prev_int: i32) {
        use prop::*;
        match idx {
            DER_LIST => {
                // Pascal: `FDERPointerList.Clear; FListSize := DERNameList.count`.
                // Mark the fleet stale so the next `Sample` rebuilds it.
                self.invalidate_fleet();
                self.f_list_size = self.der_name_list.len() as i32;
            }
            MODE => self.combi_mode = super::NONE_COMBMODE,
            DBV_MIN => {
                if self.dbv_max > 0.0 && self.dbv_min > self.dbv_max {
                    let full = format!("InvControl.{}", self.ccd.cd.obj.name());
                    self.ccd.cd.obj.push_error(format!(
                        "Minimum dead-band voltage value should be less than the maximum \
                         dead-band voltage value.  Value set to 0.0 \"DbVMin\" for object \"{full}\""
                    ));
                    self.dbv_min = 0.0;
                }
            }
            DBV_MAX => {
                if self.dbv_min > 0.0 && self.dbv_max < self.dbv_min {
                    let full = format!("InvControl.{}", self.ccd.cd.obj.name());
                    self.ccd.cd.obj.push_error(format!(
                        "Maximum dead-band voltage value should be greater than the minimum \
                         dead-band voltage value.  Value set to 0.0 \"DbVMax\" for Object \"{full}\""
                    ));
                    self.dbv_max = 0.0;
                }
            }
            LPF_TAU => {
                if self.lpf_tau <= 0.0 {
                    self.rate_of_change_mode = super::ROC_INACTIVE;
                }
            }
            RISE_FALL_LIMIT => {
                if self.rise_fall_limit <= 0.0 {
                    self.rate_of_change_mode = super::ROC_INACTIVE;
                }
            }
            MON_BUS => {
                // Pascal `PropertySideEffects(monBus)`: split each `MonBus=` entry
                // into its bus name (`FMonBuses`) and node numbers (`FMonBusesNodes`)
                // via `DSS.AuxParser.ParseAsBusName`. Consumed by Sample's /
                // UpdateInvControl's `GetMonVoltage` explicit-MonBus path. A fresh
                // parser stands in for the standalone AuxParser (the entries are
                // simple `bus[.node...]` tokens — no vars / auto-increment).
                let mut parser = dss_parser::Parser::new();
                let vars = dss_parser::ParserVars::new();
                let entries = self.mon_buses_name_list.clone();
                let full = format!("InvControl.{}", self.ccd.cd.obj.name());
                self.mon_buses.clear();
                self.mon_buses_nodes.clear();
                for entry in &entries {
                    let (bus, nodes) = match parser.parse_as_bus_name(entry, &vars) {
                        Ok(v) => v,
                        Err(_) => (entry.clone(), Vec::new()),
                    };
                    self.mon_buses.push(bus);
                    // dss_capi 0.15.x C8 (`e6607efd`, `InvControl.pas` l.694-698):
                    // a `MonBus` entry with NO node numbers is a hard error #2024111
                    // that aborts the side-effect (Pascal `Exit`) — the monitored
                    // per-phase voltage is undefined without an explicit node list.
                    if nodes.is_empty() {
                        self.ccd.cd.obj.push_error(format!(
                            "MonBus.{full}: Bus nodes are missing in \"{entry}\"."
                        ));
                        self.mon_buses_nodes.push(nodes);
                        break;
                    }
                    self.mon_buses_nodes.push(nodes);
                }
            }
            PVSYSTEM_LIST => {
                // Legacy list: assume bare PVSystem names; prepend the class.
                for n in &mut self.der_name_list {
                    *n = format!("PVSystem.{n}");
                }
                self.side_effects(DER_LIST, _prev_int);
            }
            VVC_CURVE1 | VOLTWATT_CURVE | VOLTWATTCH_CURVE | WATTPF_CURVE | WATTVAR_CURVE => {
                self.validate_curve(idx);
            }
            _ => {}
        }
    }

    /// Pascal `TCktElementClass.EndEdit` → `RecalcElementData`: attach the
    /// terminal to the resolved first-DER bus (the fleet *dispatch* build is
    /// deferred to the first `Sample`).
    fn end_edit(&mut self) {
        self.recalc();
    }

    /// Pascal `TInvControlObj.MakeLike` — copies the parse-time control settings
    /// (incl. the parsed `FMonBuses`/`FMonBusesNodes` arrays). The per-DER fleet
    /// state (`ControlledElement`/`CtrlVars`) is step-2b runtime state; Pascal
    /// notably does **not** copy `DERNameList`, `MonBusesNameList`,
    /// `FReacPower_ref`, `Fv_setpoint`, `CtrlModel`, or `ShowEventLog`, so those
    /// keep the derived object's ctor defaults.
    fn make_like(&mut self, other: &dyn DssObject) {
        let Some(other) = other.as_any().downcast_ref::<InvControl>() else {
            return;
        };
        self.ccd.cd.make_like_base(&other.ccd.cd);
        self.ccd.cd.nphases = other.ccd.cd.nphases;
        let nc = other.ccd.cd.nconds;
        self.ccd.cd.set_nconds(nc); // Force Reallocation of terminal stuff

        self.control_mode = other.control_mode;
        self.combi_mode = other.combi_mode;
        self.f_list_size = other.f_list_size;
        self.vvc_curve_name = other.vvc_curve_name.clone();
        self.vvc_curve = other.vvc_curve.clone();
        self.vvc_curve_offset = other.vvc_curve_offset;
        self.voltage_curvex_ref = other.voltage_curvex_ref;
        self.voltwatt_curve_name = other.voltwatt_curve_name.clone();
        self.voltwatt_curve = other.voltwatt_curve.clone();
        self.voltwattch_curve_name = other.voltwattch_curve_name.clone();
        self.voltwattch_curve = other.voltwattch_curve.clone();
        self.wattpf_curve_name = other.wattpf_curve_name.clone();
        self.wattpf_curve = other.wattpf_curve.clone();
        self.wattvar_curve_name = other.wattvar_curve_name.clone();
        self.wattvar_curve = other.wattvar_curve.clone();
        self.dbv_min = other.dbv_min;
        self.pf_wp_nominal = other.pf_wp_nominal;
        self.dbv_max = other.dbv_max;
        self.ar_gra_low_v = other.ar_gra_low_v;
        self.ar_gra_hi_v = other.ar_gra_hi_v;
        self.roll_avg_window_length = other.roll_avg_window_length;
        self.drc_roll_avg_window_length = other.drc_roll_avg_window_length;
        self.active_p_change_tolerance = other.active_p_change_tolerance;
        self.delta_q_factor = other.delta_q_factor;
        self.delta_p_factor = other.delta_p_factor;
        self.voltage_change_tolerance = other.voltage_change_tolerance;
        self.var_change_tolerance = other.var_change_tolerance;
        self.voltwatt_yaxis = other.voltwatt_yaxis;
        self.rate_of_change_mode = other.rate_of_change_mode;
        self.lpf_tau = other.lpf_tau;
        self.rise_fall_limit = other.rise_fall_limit;
        self.mon_buses_phase = other.mon_buses_phase;
        self.mon_buses = other.mon_buses.clone();
        self.mon_buses_nodes = other.mon_buses_nodes.clone();

        // Pascal copies FMonBusesVbase up to *this* object's MonBusesNameList
        // count (not the source's) — an upstream quirk; the derived object's
        // name list is unset (MakeLike does not copy it), so this copies 0
        // unless MonBus was set on the derived object first.
        let n = self.mon_buses_name_list.len();
        self.mon_buses_vbase = other.mon_buses_vbase.iter().take(n).copied().collect();

        self.ccd.time_delay = other.ccd.time_delay;
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}
