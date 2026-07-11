//! Trait plumbing for `TRelayObj`: the [`CktElement`] hooks (a control element
//! builds no Yprim and carries zero current) and the [`DssObject`] property
//! accessors, plus the executive-driven resolution of the five TCC curves
//! (PhaseCurve / GroundCurve / OvervoltCurve / UndervoltCurve /
//! DOC_PhaseCurveInner).

use num_complex::Complex64;

use crate::elements::general::tcc_curve::TccCurveObj;
use crate::elements::pos_seq::{PosSeqCtx, PosSeqPlan};
use crate::elements::traits::{CktElement, ElemRef, SysCtx};
use crate::obj::base::{DssObjData, DssObject, RefAction};

use super::Relay;

impl Relay {
    /// Executive hook: the five TCC curve names to resolve against the
    /// TCC_Curve registry, in the [`Self::set_resolved_curves`] order. Empty
    /// where the reference is NIL.
    pub fn curve_names(&self) -> [String; 5] {
        [
            self.phase_curve_name.clone(),
            self.ground_curve_name.clone(),
            self.ov_curve_name.clone(),
            self.uv_curve_name.clone(),
            self.doc_phase_curve_inner_name.clone(),
        ]
    }

    /// Executive hook: install the five resolved TCC curve clones (PhaseCurve /
    /// GroundCurve / OvervoltCurve / UndervoltCurve / DOC_PhaseCurveInner).
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

    /// Pascal `TControlElem.FControlledElement` - the element this control
    /// acts on (`None` when it drives a list rather than a single element).
    fn controlled_element(&self) -> Option<crate::elements::traits::ElemRef> {
        self.ccd.controlled_element
    }

    fn recalc_element_data(&mut self, _sys: &SysCtx) {
        self.recalc();
    }

    /// Pascal `TControlElem.CalcYPrim`: leave YPrim as NIL (always zero).
    fn calc_yprim(&mut self, _sys: &SysCtx) {}

    /// Pascal `TControlElem.GetCurrents`: always zero.
    fn get_currents(&mut self, _sys: &SysCtx, _node_v: &[Complex64], curr: &mut [Complex64]) {
        curr.fill(Complex64::ZERO);
    }

    /// Pascal `TRelayObj.MakePosSequence` (`Controls/Relay.pas:915`): like
    /// Recloser (monitored-element phase/conductor/bus resync, plus a `cvBuffer`
    /// realloc for the Distance/TD21/DOC types), then — **outside** the NIL
    /// guard — recompute `Vbase`/`PickupVolts47` from the new `Fnphases`, and run
    /// the base bus rename (`inherited`). The out-of-guard placement is
    /// reproduced exactly.
    fn make_pos_sequence(&mut self, ctx: &PosSeqCtx) -> PosSeqPlan {
        if let Some(m) = &ctx.monitored {
            // FNphases := MonitoredElement.NPhases; Nconds := FNphases
            self.ccd.cd.nphases = m.nphases;
            self.ccd.cd.set_nconds(m.nphases);
            // Setbus(1, MonitoredElement.GetBus(ElementTerminal))
            let t = self.monitored_element_terminal as usize;
            let bus = t
                .checked_sub(1)
                .and_then(|k| m.bus_names.get(k))
                .cloned()
                .unwrap_or_default();
            self.ccd.cd.set_bus(1, &bus);
            // ReAllocMem(cBuffer, ..) [all types] + ReAllocMem(cvBuffer, ..)
            // [Distance/TD21/DOC] + CondOffset: no persistent field — the sampler
            // sizes `cbuffer`/`cvbuffer` and computes `cond_offset` as locals each
            // `Sample` from the live monitored element.
        }
        // Vbase / PickupVolts47 recompute sits OUTSIDE the NIL guard
        // (Relay.pas:931-937), reproduced verbatim.
        self.vbase = if self.ccd.cd.nphases == 1 {
            self.kv_base * 1000.0
        } else {
            self.kv_base / crate::util::sqrt3() * 1000.0
        };
        self.pickup_volts47 = self.vbase * self.pct_pickup47 * 0.01;
        // inherited MakePosSequence -> base bus rename.
        PosSeqPlan::base()
    }

    /// Pascal `TControlElem.MonitoredElement` — resolved so the exec applier can
    /// build [`PosSeqCtx::monitored`] before calling [`Self::make_pos_sequence`].
    fn monitored_element_ref(&self) -> Option<ElemRef> {
        self.ccd.monitored_element
    }
}

impl DssObject for Relay {
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
        use super::prop::*;
        match idx {
            PHASE_TRIP => self.phase_trip,
            GROUND_TRIP => self.ground_trip,
            TD_PHASE => self.td_phase,
            TD_GROUND => self.td_ground,
            PHASE_INST => self.phase_inst,
            GROUND_INST => self.ground_inst,
            RESET => self.reset_time,
            DELAY => self.delay_time,
            KV_BASE => self.kv_base,
            PCT_PICKUP47 => self.pct_pickup47,
            BASE_AMPS46 => self.base_amps46,
            PCT_PICKUP46 => self.pct_pickup46,
            ISQT46 => self.isqt46,
            OVERTRIP => self.over_trip,
            UNDERTRIP => self.under_trip,
            BREAKER_TIME => self.breaker_time,
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
            PHASE_TRIP => self.phase_trip = value,
            GROUND_TRIP => self.ground_trip = value,
            TD_PHASE => self.td_phase = value,
            TD_GROUND => self.td_ground = value,
            PHASE_INST => self.phase_inst = value,
            GROUND_INST => self.ground_inst = value,
            RESET => self.reset_time = value,
            DELAY => self.delay_time = value,
            KV_BASE => self.kv_base = value,
            PCT_PICKUP47 => self.pct_pickup47 = value,
            BASE_AMPS46 => self.base_amps46 = value,
            PCT_PICKUP46 => self.pct_pickup46 = value,
            ISQT46 => self.isqt46 = value,
            OVERTRIP => self.over_trip = value,
            UNDERTRIP => self.under_trip = value,
            BREAKER_TIME => self.breaker_time = value,
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
            TYP => self.control_type,
            // Shots aliases NumReclose; the dump subtracts the -1 value offset.
            SHOTS => self.num_reclose,
            // Action/State read FPresentState; Normal reads NormalState.
            ACTION | STATE => self.present_state,
            NORMAL => self.normal_state,
            _ => unreachable!("Relay has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        use super::prop::*;
        match idx {
            MONITORED_TERM => self.monitored_element_terminal = value,
            SWITCHED_TERM => self.ccd.element_terminal = value,
            TYP => self.control_type = value,
            // The engine has already applied the -1 value offset.
            SHOTS => self.num_reclose = value,
            ACTION | STATE => self.present_state = value,
            NORMAL => self.normal_state = value,
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
            // Pascal control elements have no `Set_Enabled` side effect.
            ENABLED => self.ccd.cd.enabled = value,
            _ => unreachable!("Relay has no boolean property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        use super::prop::*;
        match idx {
            MONITORED_OBJ => self.monitored_full_name.clone(),
            SWITCHED_OBJ => self.switched_full_name.clone(),
            PHASE_CURVE => self.phase_curve_name.clone(),
            GROUND_CURVE => self.ground_curve_name.clone(),
            OVERVOLT_CURVE => self.ov_curve_name.clone(),
            UNDERVOLT_CURVE => self.uv_curve_name.clone(),
            DOC_PHASE_CURVE_INNER => self.doc_phase_curve_inner_name.clone(),
            VARIABLE => self.monitor_variable.clone(),
            _ => unreachable!("Relay has no string property {idx}"),
        }
    }
    fn set_string(&mut self, idx: usize, value: String) {
        use super::prop::*;
        match idx {
            VARIABLE => self.monitor_variable = value,
            _ => unreachable!("Relay has no settable string property {idx}"),
        }
    }

    /// `RecloseIntervals` is the only double-array property; sized by `NumReclose`.
    fn array_size(&self, idx: usize) -> usize {
        use super::prop::*;
        match idx {
            RECLOSE_INTERVALS => self.num_reclose.max(0) as usize,
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

    /// `monitoredobj=`/`switchedobj=` (any element) + the five `…=` TCC curves:
    /// store the name + snapshot. The curve clones are resolved by the executive
    /// from the curve names after the edit (the Recloser pattern).
    fn set_object_ref(
        &mut self,
        idx: usize,
        name: String,
        resolved: Option<(ElemRef, &dyn DssObject)>,
    ) {
        use super::prop::*;
        match idx {
            MONITORED_OBJ => match resolved {
                Some((r, obj)) => {
                    self.monitored_full_name = name.clone();
                    self.ccd.monitored_element = Some(r);
                    let elem = obj
                        .as_ckt_element()
                        .expect("monitoredobj resolves against circuit classes");
                    // Capture the monitored element's state-variable names for the
                    // Generic relay's `LookupVariable` (recalc has no live element).
                    // Non-PC elements expose none (`num_variables() == 0`).
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
                Some((r, obj)) => {
                    self.switched_full_name = name.clone();
                    self.ccd.controlled_element = Some(r);
                    let elem = obj
                        .as_ckt_element()
                        .expect("switchedobj resolves against circuit classes");
                    self.ctrl_snap = Some(super::RefSnapshot::capture(name, elem));
                }
                None => {
                    self.switched_full_name = name;
                    self.ccd.controlled_element = None;
                    self.ctrl_snap = None;
                }
            },
            // The five TCC curves: record the name; the executive clones them.
            PHASE_CURVE => self.phase_curve_name = name,
            GROUND_CURVE => self.ground_curve_name = name,
            OVERVOLT_CURVE => self.ov_curve_name = name,
            UNDERVOLT_CURVE => self.uv_curve_name = name,
            DOC_PHASE_CURVE_INNER => self.doc_phase_curve_inner_name = name,
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
            // Default the controlled element to the monitored element.
            MONITORED_OBJ => {
                self.ccd.controlled_element = self.ccd.monitored_element;
                self.switched_full_name = self.monitored_full_name.clone();
                self.ctrl_snap = self.mon_snap.clone();
            }
            MONITORED_TERM => self.ccd.element_terminal = self.monitored_element_terminal,
            VARIABLE => self.monitor_variable = self.monitor_variable.to_lowercase(),
            TYP => self.type_side_effect(),
            NORMAL => self.normal_state_set = true,
            ACTION | STATE => self.state_side_effect(),
            _ => {}
        }
    }

    /// Pascal `TCktElementClass.EndEdit` default → `RecalcElementData`.
    fn end_edit(&mut self) {
        self.recalc();
    }

    fn take_ref_actions(&mut self) -> Vec<RefAction> {
        std::mem::take(&mut self.pending_ref_actions)
    }

    /// Pascal `TRelayObj.MakeLike`.
    fn make_like(&mut self, other: &dyn DssObject) {
        let Some(other) = other.as_any().downcast_ref::<Relay>() else {
            return;
        };
        self.ccd.cd.make_like_base(&other.ccd.cd);
        self.ccd.cd.nphases = other.ccd.cd.nphases;
        let nc = other.ccd.cd.nconds;
        self.ccd.cd.set_nconds(nc); // Force Reallocation of terminal stuff
        self.ccd.show_event_log = other.ccd.show_event_log; // but leave DebugTrace off

        self.ccd.element_terminal = other.ccd.element_terminal;
        self.ccd.controlled_element = other.ccd.controlled_element;
        self.ccd.monitored_element = other.ccd.monitored_element;
        self.monitored_element_terminal = other.monitored_element_terminal;
        self.monitored_full_name = other.monitored_full_name.clone();
        self.switched_full_name = other.switched_full_name.clone();
        self.mon_snap = other.mon_snap.clone();
        self.ctrl_snap = other.ctrl_snap.clone();

        // Curves: Pascal copies the pointers; we copy name + resolved clone.
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
        self.delay_time = other.delay_time;
        self.breaker_time = other.breaker_time;
        self.reclose_intervals = other.reclose_intervals;

        self.kv_base = other.kv_base;
        self.locked_out = other.locked_out;

        self.present_state = other.present_state;
        self.normal_state = other.normal_state;
        self.normal_state_set = other.normal_state_set;

        self.control_type = other.control_type;

        // 46 / 47.
        self.pickup_amps46 = other.pickup_amps46;
        self.pct_pickup46 = other.pct_pickup46;
        self.base_amps46 = other.base_amps46;
        self.isqt46 = other.isqt46;
        self.pickup_volts47 = other.pickup_volts47;
        self.pct_pickup47 = other.pct_pickup47;

        // Generic. (Pascal copies `MonitorVariable`; `MonitorVarIndex` is
        // re-resolved in the new object's `recalc` — carry the name cache + index
        // so a `like=` clone that does not re-specify `monitoredobj` still resolves.)
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

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}
