//! The `CktElement` and `DssObject` trait impls for `ExpControl`: the typed
//! property accessors, `PropertySideEffects` (the PVSystemList ↔ DERList sync),
//! `RecalcElementData` (the parse-time terminal attach + `FOpenTau` derivation),
//! `MakeLike`, and the dump-time getters.

use num_complex::Complex64;

use crate::elements::traits::{CktElement, SysCtx};
use crate::obj::base::{DssObjData, DssObject};

use super::{ExpControl, prop};

// TODO(compat): Pascal `FOpenTau := Tresponse / 2.3026` divides by the *truncated*
// ln(10) literal (the full value is 2.302585092994046…); reproducing the exact bits
// keeps the open-loop low-pass filter bit-identical to the oracle. The clean fix
// (`std::f64::consts::LN_10`) lands in the post-port compat sweep (PORTING_PLAN §6).
#[allow(clippy::approx_constant)]
const LN10_TRUNCATED: f64 = 2.3026;

/// Pascal `StripClassName` — everything past the first '.' (the whole string when
/// there is no '.').
fn strip_class_name(s: &str) -> String {
    s.split_once('.')
        .map_or_else(|| s.to_string(), |(_, rest)| rest.to_string())
}

impl ExpControl {
    /// The parse-time tail of Pascal `RecalcElementData`: derive `FOpenTau`, then —
    /// with the first PVSystem's bus resolved (by the executive at edit-completion)
    /// — set the control's phase count and attach its single terminal to that bus
    /// (Pascal `Setbus(1, MonitoredElement.Firstbus)`). Without a resolved DER (no
    /// fleet member found at parse time) the terminal stays unset.
    ///
    /// `mon_nphases` is the resolved DER's phase count (Pascal sets `FNphases :=
    /// ControlledElement[i].NPhases` over the recalc loop — the *last* DER's; for a
    /// single-DER / homogeneous fleet, the same as the first DER's bus phases).
    pub(crate) fn recalc(&mut self) {
        self.f_open_tau = self.tresponse / LN10_TRUNCATED;
        if !self.mon_resolved {
            return;
        }
        self.ccd.cd.nphases = self.mon_nphases;
        self.ccd.cd.set_nconds(self.mon_nphases);
        let bus = self.mon_bus.clone();
        self.ccd.cd.set_bus(1, &bus);
    }

    /// Pascal `FPVSystemPointerList.Clear` — drop the resolved fleet so the next
    /// `Sample` rebuilds it (an empty fleet re-triggers the build; a list edit
    /// changes the fleet).
    fn invalidate_fleet(&mut self) {
        self.fleet.clear();
        self.ctrl_vars.clear();
    }
}

impl CktElement for ExpControl {
    fn cd(&self) -> &crate::elements::ckt::CktElementData {
        &self.ccd.cd
    }
    fn cd_mut(&mut self) -> &mut crate::elements::ckt::CktElementData {
        &mut self.ccd.cd
    }

    /// Pascal `TExpControlObj.RecalcElementData` (the parse-time subset): derive
    /// `FOpenTau` and attach the control's terminal to the first DER's bus. The
    /// fleet *dispatch* build (`MakePVSystemList`) needs store access, so it is
    /// deferred to the first `Sample`; the bus is resolved at edit-completion (the
    /// executive calls [`set_resolved_monitored`](ExpControl::set_resolved_monitored)
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
}

impl DssObject for ExpControl {
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
            VREG => self.f_vreg_init,
            SLOPE => self.q_v_slope,
            VREG_TAU => self.vreg_tau,
            QBIAS => self.f_qbias,
            VREG_MIN => self.vreg_min,
            VREG_MAX => self.vreg_max,
            QMAX_LEAD => self.qmax_lead,
            QMAX_LAG => self.qmax_lag,
            DELTA_Q_FACTOR => self.f_delta_q_factor,
            TRESPONSE => self.tresponse,
            BASE_FREQ => self.ccd.cd.base_frequency,
            _ => unreachable!("ExpControl has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        use prop::*;
        match idx {
            VREG => self.f_vreg_init = value,
            SLOPE => self.q_v_slope = value,
            VREG_TAU => self.vreg_tau = value,
            QBIAS => self.f_qbias = value,
            VREG_MIN => self.vreg_min = value,
            VREG_MAX => self.vreg_max = value,
            QMAX_LEAD => self.qmax_lead = value,
            QMAX_LAG => self.qmax_lag = value,
            DELTA_Q_FACTOR => self.f_delta_q_factor = value,
            TRESPONSE => self.tresponse = value,
            BASE_FREQ => self.ccd.cd.base_frequency = value,
            _ => unreachable!("ExpControl has no double property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        match idx {
            prop::EVENT_LOG => self.ccd.show_event_log,
            prop::PREFER_Q => self.f_prefer_q,
            prop::ENABLED => self.ccd.cd.enabled,
            _ => unreachable!("ExpControl has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        match idx {
            prop::EVENT_LOG => self.ccd.show_event_log = value,
            prop::PREFER_Q => self.f_prefer_q = value,
            prop::ENABLED => self.ccd.cd.enabled = value,
            _ => unreachable!("ExpControl has no boolean property {idx}"),
        }
    }

    fn get_string_list(&self, idx: usize) -> Vec<String> {
        match idx {
            prop::PVSYSTEM_LIST => self.pvsystem_name_list.clone(),
            prop::DER_LIST => self.der_name_list.clone(),
            _ => unreachable!("ExpControl has no string-list property {idx}"),
        }
    }
    fn set_string_list(&mut self, idx: usize, value: Vec<String>) {
        match idx {
            prop::PVSYSTEM_LIST => self.pvsystem_name_list = value,
            prop::DER_LIST => self.der_name_list = value,
            _ => unreachable!("ExpControl has no string-list property {idx}"),
        }
    }

    fn set_bus_name(&mut self, terminal: usize, value: &str) {
        self.ccd.cd.set_bus(terminal, value);
    }
    fn get_bus_name(&self, terminal: usize) -> String {
        self.ccd.cd.get_bus(terminal).to_string()
    }

    /// Pascal `TExpControlObj.PropertySideEffects` — keep the bare PVSystemList and
    /// the class-prefixed DERList in sync; either write clears the fleet.
    fn side_effects(&mut self, idx: usize, _prev_int: i32) {
        use prop::*;
        match idx {
            PVSYSTEM_LIST => {
                self.invalidate_fleet();
                self.f_list_size = self.pvsystem_name_list.len() as i32;
                self.der_name_list = self
                    .pvsystem_name_list
                    .iter()
                    .map(|n| format!("PVSystem.{n}"))
                    .collect();
            }
            DER_LIST => {
                self.invalidate_fleet();
                self.f_list_size = self.der_name_list.len() as i32;
                self.pvsystem_name_list = self
                    .der_name_list
                    .iter()
                    .map(|n| strip_class_name(n))
                    .collect();
            }
            _ => {}
        }
    }

    /// Pascal `TCktElementClass.EndEdit` → `RecalcElementData`: derive `FOpenTau`
    /// and attach the terminal to the resolved first-DER bus (the fleet *dispatch*
    /// build is deferred to the first `Sample`).
    fn end_edit(&mut self) {
        self.recalc();
    }

    /// Pascal `TExpControlObj.MakeLike` — copies the parse-time control settings
    /// and the phase count. Pascal notably does **not** copy `Tresponse`/`FOpenTau`,
    /// `ShowEventLog`, `TimeDelay`, or the name lists (`FPVSystemNameList`/
    /// `DERNameList`), so those keep the derived object's ctor defaults; the per-DER
    /// fleet state (`ControlledElement`/`CtrlVars`) is step-3 runtime state.
    fn make_like(&mut self, other: &dyn DssObject) {
        let Some(other) = other.as_any().downcast_ref::<ExpControl>() else {
            return;
        };
        self.ccd.cd.make_like_base(&other.ccd.cd);
        self.ccd.cd.nphases = other.ccd.cd.nphases;
        let nc = other.ccd.cd.nconds;
        self.ccd.cd.set_nconds(nc); // Force Reallocation of terminal stuff

        self.f_list_size = other.f_list_size;
        self.f_voltage_change_tolerance = other.f_voltage_change_tolerance;
        self.f_var_change_tolerance = other.f_var_change_tolerance;
        self.f_vreg_init = other.f_vreg_init;
        self.q_v_slope = other.q_v_slope;
        self.vreg_tau = other.vreg_tau;
        self.f_qbias = other.f_qbias;
        self.vreg_min = other.vreg_min;
        self.vreg_max = other.vreg_max;
        self.qmax_lead = other.qmax_lead;
        self.qmax_lag = other.qmax_lag;
        self.f_delta_q_factor = other.f_delta_q_factor;
        self.f_prefer_q = other.f_prefer_q;
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}
