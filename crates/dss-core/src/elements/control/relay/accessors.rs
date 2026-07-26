//! Trait plumbing for `TRelayObj` (r4133): the [`CktElement`] hooks (a control
//! element builds no Yprim and carries zero current) and the [`DssObject`]
//! property accessors, plus the executive-driven resolution of the five TCC
//! curves (PhCurve / OC_GndCurve / Voltage_OVCurve / Voltage_UVCurve /
//! DOC_PhaseCurveInner). The deprecated-alias props (57-71) share the canonical
//! fields via extra match arms.

use num_complex::Complex64;

use crate::elements::control::control_elem::ControlAction;
use crate::elements::general::tcc_curve::TccCurveObj;
use crate::elements::pos_seq::{PosSeqCtx, PosSeqPlan};
use crate::elements::traits::{CktElement, ElemId, SysCtx};
use crate::obj::arena::ResolvedObj;
use crate::obj::base::{DssObjData, DssObject, RefAction};

use super::Relay;

impl Relay {
    /// Executive hook: the five TCC curve names to resolve against the TCC_Curve
    /// registry, in the [`Self::set_resolved_curves`] order. `none` (the default)
    /// resolves to no curve (Pascal `GetTccCurve('none')` -> NIL silently).
    pub fn curve_names(&self) -> [String; 5] {
        [
            self.phase_curve_name.clone(),
            self.ground_curve_name.clone(),
            self.ov_curve_name.clone(),
            self.uv_curve_name.clone(),
            self.doc_phase_curve_inner_name.clone(),
        ]
    }

    /// Executive hook: install the five resolved TCC curve clones.
    pub fn set_resolved_curves(&mut self, curves: [Option<TccCurveObj>; 5]) {
        let [pc, gc, ov, uv, doc] = curves;
        self.phase_curve = pc;
        self.ground_curve = gc;
        self.ov_curve = ov;
        self.uv_curve = uv;
        self.doc_phase_curve_inner = doc;
    }
}

impl CktElement for Relay {
    fn cd(&self) -> &crate::elements::ckt::CktElementData {
        &self.ccd.cd
    }
    fn cd_mut(&mut self) -> &mut crate::elements::ckt::CktElementData {
        &mut self.ccd.cd
    }

    fn controlled_element(&self) -> Option<crate::elements::traits::ElemId> {
        self.ccd.controlled_element
    }

    fn calc_yprim(&mut self, _sys: &SysCtx) {}

    fn get_currents(&mut self, _sys: &SysCtx, _node_v: &[Complex64], curr: &mut [Complex64]) {
        curr.fill(Complex64::ZERO);
    }

    /// Pascal `TRelayObj.MakePosSequence`.
    fn make_pos_sequence(&mut self, ctx: &PosSeqCtx) -> PosSeqPlan {
        if let Some(m) = &ctx.monitored {
            self.ccd.cd.nphases = m.nphases;
            self.ccd.cd.set_nconds(m.nphases);
            let t = self.monitored_element_terminal as usize;
            let bus = t
                .checked_sub(1)
                .and_then(|k| m.bus_names.get(k))
                .cloned()
                .unwrap_or_default();
            self.ccd.cd.set_bus(1, &bus);
        }
        // Vbase / PickupVolts47 recompute sits OUTSIDE the NIL guard.
        self.vbase = if self.ccd.cd.nphases == 1 {
            self.kv_base * 1000.0
        } else {
            self.kv_base / crate::util::sqrt3() * 1000.0
        };
        self.pickup_volts47 = self.vbase * self.pct_pickup47 * 0.01;
        PosSeqPlan::base()
    }

    fn monitored_element_ref(&self) -> Option<ElemId> {
        self.ccd.monitored_element
    }
}

impl Relay {
    /// Pascal `TRelayObj.MakeLike` (r4133).
    pub(crate) fn make_like(&mut self, other: &Self) {
        self.ccd.cd.make_like_base(&other.ccd.cd);
        self.ccd.cd.nphases = other.ccd.cd.nphases;
        let nc = other.ccd.cd.nconds;
        self.ccd.cd.set_nconds(nc);
        self.ccd.show_event_log = other.ccd.show_event_log; // but leave DebugTrace off

        self.ccd.element_terminal = other.ccd.element_terminal;
        self.ccd.controlled_element = other.ccd.controlled_element;
        self.ccd.monitored_element = other.ccd.monitored_element;
        self.monitored_element_terminal = other.monitored_element_terminal;
        self.monitored_full_name = other.monitored_full_name.clone();
        self.switched_full_name = other.switched_full_name.clone();
        self.mon_snap = other.mon_snap.clone();
        self.ctrl_snap = other.ctrl_snap.clone();

        self.phase_curve_name = other.phase_curve_name.clone();
        self.ground_curve_name = other.ground_curve_name.clone();
        self.ov_curve_name = other.ov_curve_name.clone();
        self.uv_curve_name = other.uv_curve_name.clone();
        self.doc_phase_curve_inner_name = other.doc_phase_curve_inner_name.clone();
        self.phase_curve = other.phase_curve.clone();
        self.ground_curve = other.ground_curve.clone();
        self.ov_curve = other.ov_curve.clone();
        self.uv_curve = other.uv_curve.clone();
        self.doc_phase_curve_inner = other.doc_phase_curve_inner.clone();

        self.phase_trip = other.phase_trip;
        self.ground_trip = other.ground_trip;
        self.td_phase = other.td_phase;
        self.td_ground = other.td_ground;
        self.phase_inst = other.phase_inst;
        self.ground_inst = other.ground_inst;
        self.reset_time = other.reset_time;
        self.num_reclose = other.num_reclose;
        self.definite_time_delay = other.definite_time_delay;
        self.mechanical_delay = other.mechanical_delay;
        self.single_ph_trip = other.single_ph_trip;
        self.single_ph_lockout = other.single_ph_lockout;
        self.rated_current = other.rated_current;
        self.interrupting_rating = other.interrupting_rating;
        self.reclose_intervals = other.reclose_intervals;

        self.kv_base = other.kv_base;
        self.f_locked = other.f_locked;
        self.locked_out = other.locked_out;

        // Per-phase state (Pascal MakeLike Relay.pas:683 copies
        // FPresentState/FNormalState over `Min(RELAYCONTROLMAXDIM,
        // ControlledElement.Nphases)`, NOT the relay's own FNPhases). `ctrl_snap`
        // was copied from `other` above, so `state_size()` yields the same
        // controlled-element phase count Pascal loops here.
        let n = self.state_size();
        for i in 1..=n {
            self.present_state[i] = other.present_state[i];
            self.normal_state[i] = other.normal_state[i];
        }
        self.normal_state_set = other.normal_state_set;

        self.control_type = other.control_type;

        // 46 / 47.
        self.pickup_amps46 = other.pickup_amps46;
        self.pct_pickup46 = other.pct_pickup46;
        self.base_amps46 = other.base_amps46;
        self.isqt46 = other.isqt46;
        self.pickup_volts47 = other.pickup_volts47;
        self.pct_pickup47 = other.pct_pickup47;

        // Generic.
        self.monitor_variable = other.monitor_variable.clone();
        self.monitor_var_index = other.monitor_var_index;
        self.monitor_var_names = other.monitor_var_names.clone();
        self.over_trip = other.over_trip;
        self.under_trip = other.under_trip;

        // Distance.
        self.z1mag = other.z1mag;
        self.z1ang = other.z1ang;
        self.z0mag = other.z0mag;
        self.z0ang = other.z0ang;
        self.mphase = other.mphase;
        self.mground = other.mground;
        self.dist_reverse = other.dist_reverse;

        // Directional overcurrent.
        self.doc_tilt_angle_low = other.doc_tilt_angle_low;
        self.doc_tilt_angle_high = other.doc_tilt_angle_high;
        self.doc_trip_set_low = other.doc_trip_set_low;
        self.doc_trip_set_high = other.doc_trip_set_high;
        self.doc_trip_set_mag = other.doc_trip_set_mag;
        self.doc_delay_inner = other.doc_delay_inner;
        self.doc_phase_trip_inner = other.doc_phase_trip_inner;
        self.doc_td_phase_inner = other.doc_td_phase_inner;
        self.doc_p1_blocking = other.doc_p1_blocking;
    }
}

impl DssObject for Relay {
    fn data(&self) -> &DssObjData {
        &self.ccd.cd.obj
    }
    fn data_mut(&mut self) -> &mut DssObjData {
        &mut self.ccd.cd.obj
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
            PH_PICKUP | PHASE_TRIP => self.phase_trip,
            OC_GND_PICKUP | GROUND_TRIP => self.ground_trip,
            TD_PH | TD_PHASE => self.td_phase,
            OC_TD_GND | TD_GROUND => self.td_ground,
            PH_INST | PHASE_INST => self.phase_inst,
            OC_GND_INST | GROUND_INST => self.ground_inst,
            RESET_TIME => self.reset_time,
            DEFINITE_TIME_DELAY | DELAY => self.definite_time_delay,
            MECHANICAL_DELAY | BREAKER_TIME => self.mechanical_delay,
            KV_BASE => self.kv_base,
            PCT_PICKUP47 => self.pct_pickup47,
            BASE_AMPS46 => self.base_amps46,
            PCT_PICKUP46 => self.pct_pickup46,
            ISQT46 => self.isqt46,
            GENERIC_OVER_TRIP | OVERTRIP => self.over_trip,
            GENERIC_UNDER_TRIP | UNDERTRIP => self.under_trip,
            RATED_CURRENT => self.rated_current,
            INTERRUPTING_RATING => self.interrupting_rating,
            Z1MAG => self.z1mag,
            Z1ANG => self.z1ang,
            Z0MAG => self.z0mag,
            Z0ANG => self.z0ang,
            MPHASE => self.mphase,
            MGROUND => self.mground,
            DOC_TILT_ANGLE_LOW => self.doc_tilt_angle_low,
            DOC_TILT_ANGLE_HIGH => self.doc_tilt_angle_high,
            DOC_TRIP_SETTING_LOW => self.doc_trip_set_low,
            DOC_TRIP_SETTING_HIGH => self.doc_trip_set_high,
            DOC_TRIP_SETTING_MAG => self.doc_trip_set_mag,
            DOC_DELAY_INNER => self.doc_delay_inner,
            DOC_PHASE_TRIP_INNER => self.doc_phase_trip_inner,
            DOC_TD_PHASE_INNER => self.doc_td_phase_inner,
            BASE_FREQ => self.ccd.cd.base_frequency,
            _ => unreachable!("Relay has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        use super::prop::*;
        match idx {
            PH_PICKUP | PHASE_TRIP => self.phase_trip = value,
            OC_GND_PICKUP | GROUND_TRIP => self.ground_trip = value,
            TD_PH | TD_PHASE => self.td_phase = value,
            OC_TD_GND | TD_GROUND => self.td_ground = value,
            PH_INST | PHASE_INST => self.phase_inst = value,
            OC_GND_INST | GROUND_INST => self.ground_inst = value,
            RESET_TIME => self.reset_time = value,
            DEFINITE_TIME_DELAY | DELAY => self.definite_time_delay = value,
            MECHANICAL_DELAY | BREAKER_TIME => self.mechanical_delay = value,
            KV_BASE => self.kv_base = value,
            PCT_PICKUP47 => self.pct_pickup47 = value,
            BASE_AMPS46 => self.base_amps46 = value,
            PCT_PICKUP46 => self.pct_pickup46 = value,
            ISQT46 => self.isqt46 = value,
            GENERIC_OVER_TRIP | OVERTRIP => self.over_trip = value,
            GENERIC_UNDER_TRIP | UNDERTRIP => self.under_trip = value,
            RATED_CURRENT => self.rated_current = value,
            INTERRUPTING_RATING => self.interrupting_rating = value,
            Z1MAG => self.z1mag = value,
            Z1ANG => self.z1ang = value,
            Z0MAG => self.z0mag = value,
            Z0ANG => self.z0ang = value,
            MPHASE => self.mphase = value,
            MGROUND => self.mground = value,
            DOC_TILT_ANGLE_LOW => self.doc_tilt_angle_low = value,
            DOC_TILT_ANGLE_HIGH => self.doc_tilt_angle_high = value,
            DOC_TRIP_SETTING_LOW => self.doc_trip_set_low = value,
            DOC_TRIP_SETTING_HIGH => self.doc_trip_set_high = value,
            DOC_TRIP_SETTING_MAG => self.doc_trip_set_mag = value,
            DOC_DELAY_INNER => self.doc_delay_inner = value,
            DOC_PHASE_TRIP_INNER => self.doc_phase_trip_inner = value,
            DOC_TD_PHASE_INNER => self.doc_td_phase_inner = value,
            BASE_FREQ => self.ccd.cd.base_frequency = value,
            _ => unreachable!("Relay has no double property {idx}"),
        }
    }

    fn get_i32(&self, idx: usize) -> i32 {
        use super::prop::*;
        match idx {
            MONITORED_TERM => self.monitored_element_terminal,
            SWITCHED_TERM => self.ccd.element_terminal,
            TYP => self.control_type.ordinal(),
            SHOTS => self.num_reclose, // dump subtracts the -1 value offset
            _ => unreachable!("Relay has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        use super::prop::*;
        match idx {
            MONITORED_TERM => self.monitored_element_terminal = value,
            SWITCHED_TERM => self.ccd.element_terminal = value,
            TYP => {
                self.control_type =
                    super::RelayControlType::from_ordinal(value).unwrap_or(self.control_type)
            }
            SHOTS => self.num_reclose = value,
            _ => unreachable!("Relay has no integer property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        use super::prop::*;
        match idx {
            EVENT_LOG => self.ccd.show_event_log,
            DEBUG_TRACE => self.debug_trace,
            DIST_REVERSE => self.dist_reverse,
            DOC_P1_BLOCKING => self.doc_p1_blocking,
            SINGLE_PH_TRIP => self.single_ph_trip,
            SINGLE_PH_LOCKOUT => self.single_ph_lockout,
            LOCK => self.f_locked,
            RESET_ACTION => false, // Pascal BooleanActionProperty getter: always No
            ENABLED => self.ccd.cd.enabled,
            _ => unreachable!("Relay has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        use super::prop::*;
        match idx {
            EVENT_LOG => self.ccd.show_event_log = value,
            DEBUG_TRACE => self.debug_trace = value,
            DIST_REVERSE => self.dist_reverse = value,
            DOC_P1_BLOCKING => self.doc_p1_blocking = value,
            SINGLE_PH_TRIP => self.single_ph_trip = value,
            SINGLE_PH_LOCKOUT => self.single_ph_lockout = value,
            LOCK => self.f_locked = value,
            RESET_ACTION => {
                if value {
                    self.reset_action();
                }
            }
            ENABLED => self.ccd.cd.enabled = value,
            _ => unreachable!("Relay has no boolean property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        use super::prop::*;
        match idx {
            MONITORED_OBJ => self.monitored_full_name.clone(),
            SWITCHED_OBJ => self.switched_full_name.clone(),
            PH_CURVE | PHASE_CURVE => self.phase_curve_name.clone(),
            OC_GND_CURVE | GROUND_CURVE => self.ground_curve_name.clone(),
            VOLTAGE_OV_CURVE | OVERVOLT_CURVE => self.ov_curve_name.clone(),
            VOLTAGE_UV_CURVE | UNDERVOLT_CURVE => self.uv_curve_name.clone(),
            DOC_PHASE_CURVE_INNER => self.doc_phase_curve_inner_name.clone(),
            GENERIC_VARIABLE | VARIABLE => self.monitor_variable.clone(),
            _ => unreachable!("Relay has no string property {idx}"),
        }
    }
    fn set_string(&mut self, idx: usize, value: String) {
        use super::prop::*;
        match idx {
            GENERIC_VARIABLE | VARIABLE => self.monitor_variable = value,
            _ => unreachable!("Relay has no settable string property {idx}"),
        }
    }

    /// `Action`'s `StringEnumActionProperty` (Pascal ganged deprecated): set every
    /// phase's present state + the `State` side effect. Blocked while `Locked`.
    fn do_action(&mut self, ordinal: i32, _errors: &mut crate::diag::ErrorLog) {
        Relay::do_action(self, ordinal);
    }

    fn array_size(&self, idx: usize) -> usize {
        use super::prop::*;
        match idx {
            RECLOSE_INTERVALS => self.num_reclose.max(0) as usize,
            NORMAL | STATE => self.state_size(),
            _ => unreachable!("Relay has no function-sized array property {idx}"),
        }
    }
    fn get_f64_array(&self, idx: usize) -> Option<&[f64]> {
        use super::prop::*;
        match idx {
            RECLOSE_INTERVALS => Some(&self.reclose_intervals),
            _ => unreachable!("Relay has no double-array property {idx}"),
        }
    }
    fn set_f64_array(&mut self, idx: usize, value: Vec<f64>) {
        use super::{RECLOSE_MAX, prop::*};
        match idx {
            RECLOSE_INTERVALS => {
                let n = value.len().min(RECLOSE_MAX);
                self.num_reclose = n as i32;
                self.reclose_intervals[..n].copy_from_slice(&value[..n]);
            }
            _ => unreachable!("Relay has no double-array property {idx}"),
        }
    }

    /// `Normal`/`State` per-phase enum arrays (1-based Pascal, exposed 0-based).
    fn get_enum_array(&self, idx: usize) -> Vec<i32> {
        use super::prop::*;
        let n = self.state_size();
        match idx {
            NORMAL => self.normal_state[1..=n]
                .iter()
                .map(|s| s.ordinal())
                .collect(),
            STATE => self.present_state[1..=n]
                .iter()
                .map(|s| s.ordinal())
                .collect(),
            _ => unreachable!("Relay has no enum-array property {idx}"),
        }
    }
    /// Pascal `InterpretRelayState`: a bare unquoted scalar fills **all** phases
    /// (ganged); a quoted list fills phase-by-phase. `State` writes are blocked
    /// while `Locked`; `Normal` writes are NOT (Pascal `property_name[1] in
    /// {'a','s'}` guard — Normal starts with 'n'). A [`ControlAction::Keep`] ordinal
    /// (a token whose first char is neither `o` nor `c`) leaves that phase's slot
    /// unchanged — for a ganged scalar that means *every* phase is left as-is
    /// (Pascal's `case` with no matching arm).
    fn set_enum_array(&mut self, idx: usize, values: &[i32]) {
        use super::prop::*;
        let n = self.state_size();
        let ganged = values.len() == 1;
        match idx {
            NORMAL => {
                if ganged {
                    let state = ControlAction::from_ordinal(values[0]);
                    if state != ControlAction::Keep {
                        self.set_all_normal(state);
                    }
                } else {
                    for (k, &v) in values.iter().take(n).enumerate() {
                        let state = ControlAction::from_ordinal(v);
                        if state != ControlAction::Keep {
                            self.normal_state[k + 1] = state;
                        }
                    }
                }
            }
            STATE => {
                if self.f_locked {
                    return; // Pascal: state writes blocked while Locked.
                }
                if ganged {
                    let state = ControlAction::from_ordinal(values[0]);
                    if state != ControlAction::Keep {
                        self.set_all_present(state);
                    }
                } else {
                    for (k, &v) in values.iter().take(n).enumerate() {
                        let state = ControlAction::from_ordinal(v);
                        if state != ControlAction::Keep {
                            self.present_state[k + 1] = state;
                        }
                    }
                }
            }
            _ => unreachable!("Relay has no enum-array property {idx}"),
        }
    }

    fn set_object_ref(&mut self, idx: usize, name: String, resolved: Option<ResolvedObj<'_>>) {
        use super::prop::*;
        match idx {
            MONITORED_OBJ => match resolved {
                Some(o) => {
                    self.monitored_full_name = name.clone();
                    self.ccd.monitored_element = Some(o.id());
                    let elem = o
                        .ckt()
                        .expect("monitoredobj resolves against circuit classes");
                    self.monitor_var_names = (1..=elem.num_variables())
                        .map(|i| elem.variable_name(i))
                        .collect();
                    self.mon_snap = Some(super::RefSnapshot::capture(name, elem));
                }
                None => {
                    self.monitored_full_name = name;
                    self.ccd.monitored_element = None;
                    self.monitor_var_names = Vec::new();
                    self.mon_snap = None;
                }
            },
            SWITCHED_OBJ => match resolved {
                Some(o) => {
                    self.switched_full_name = name.clone();
                    self.ccd.controlled_element = Some(o.id());
                    let elem = o
                        .ckt()
                        .expect("switchedobj resolves against circuit classes");
                    self.ctrl_snap = Some(super::RefSnapshot::capture(name, elem));
                }
                None => {
                    self.switched_full_name = name;
                    self.ccd.controlled_element = None;
                    self.ctrl_snap = None;
                }
            },
            // The five TCC curves + aliases: record the name (`none` for unresolved
            // / literal `none`); the executive clones them.
            PH_CURVE | PHASE_CURVE => self.phase_curve_name = curve_name(name),
            OC_GND_CURVE | GROUND_CURVE => self.ground_curve_name = curve_name(name),
            VOLTAGE_OV_CURVE | OVERVOLT_CURVE => self.ov_curve_name = curve_name(name),
            VOLTAGE_UV_CURVE | UNDERVOLT_CURVE => self.uv_curve_name = curve_name(name),
            DOC_PHASE_CURVE_INNER => self.doc_phase_curve_inner_name = curve_name(name),
            _ => unreachable!("Relay has no object-ref property {idx}"),
        }
    }

    fn set_bus_name(&mut self, terminal: usize, value: &str) {
        self.ccd.cd.set_bus(terminal, value);
    }
    fn get_bus_name(&self, terminal: usize) -> String {
        self.ccd.cd.get_bus(terminal).to_string()
    }

    /// Pascal `TRelayObj.PropertySideEffects`.
    fn side_effects(&mut self, idx: usize, _prev_int: i32) {
        use super::prop::*;
        match idx {
            MONITORED_OBJ => {
                self.ccd.controlled_element = self.ccd.monitored_element;
                self.switched_full_name = self.monitored_full_name.clone();
                self.ctrl_snap = self.mon_snap.clone();
            }
            MONITORED_TERM => self.ccd.element_terminal = self.monitored_element_terminal,
            GENERIC_VARIABLE | VARIABLE => {
                self.monitor_variable = self.monitor_variable.to_ascii_lowercase()
            }
            TYP => self.type_side_effect(),
            NORMAL => self.normal_state_set = true,
            STATE => self.state_side_effect(),
            _ => {}
        }
    }

    /// Pascal `TCktElementClass.EndEdit` default → `RecalcElementData`.
    fn end_edit(&mut self, _sys: &crate::elements::traits::SysCtx) {
        self.recalc();
    }

    fn take_ref_actions(&mut self) -> Vec<RefAction> {
        std::mem::take(&mut self.pending_ref_actions)
    }
}

/// Map a resolved TCC-curve reference name to the stored dump name: an empty
/// (unresolved / `none`) reference renders `none`.
fn curve_name(name: String) -> String {
    if name.is_empty() {
        "none".to_string()
    } else {
        name
    }
}

impl crate::elements::control::control_elem::ControlElem for Relay {
    fn ccd(&self) -> &crate::elements::control::control_elem::ControlElemData {
        &self.ccd
    }
    fn ccd_mut(&mut self) -> &mut crate::elements::control::control_elem::ControlElemData {
        &mut self.ccd
    }
    fn control_kind(&self) -> crate::elements::control::control_elem::ControlClass {
        crate::elements::control::control_elem::ControlClass::Relay
    }
    fn reset_control_side(&mut self) {
        Relay::reset_control_side(self);
    }
}
