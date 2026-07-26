//! The trait plumbing for `TRecloserObj` (r4133): the [`CktElement`] hooks (a
//! control element builds no Yprim and carries zero current) and the
//! [`DssObject`] property accessors (`get_*`/`set_*`, `set_object_ref`,
//! `PropertySideEffects`, `EndEdit`, `MakeLike`, `take_ref_actions`), plus the
//! executive-driven resolution of the four TCC curves.

use num_complex::Complex64;

use crate::elements::general::tcc_curve::TccCurveObj;
use crate::elements::pos_seq::{PosSeqCtx, PosSeqPlan};
use crate::elements::traits::{CktElement, ElemId, SysCtx};
use crate::obj::base::{DssObjData, DssObject, RefAction};

use super::{RCMAX, RECLOSE_MAX, Recloser};

impl Recloser {
    /// Executive hook: the four TCC curve names (PhFast / PhSlow / GndFast /
    /// GndSlow) to resolve against the TCC_Curve registry. `none` (the default)
    /// resolves to no curve (Pascal `GetTccCurve('none')` -> NIL silently).
    pub fn curve_names(&self) -> [String; 4] {
        [
            self.ph_fast_name.clone(),
            self.ph_slow_name.clone(),
            self.gnd_fast_name.clone(),
            self.gnd_slow_name.clone(),
        ]
    }

    /// Executive hook: install the four resolved TCC curve clones (in the
    /// [`Self::curve_names`] order). A `None` curve never trips its branch.
    pub fn set_resolved_curves(&mut self, curves: [Option<TccCurveObj>; 4]) {
        let [pf, ps, gf, gs] = curves;
        self.ph_fast = pf;
        self.ph_slow = ps;
        self.gnd_fast = gf;
        self.gnd_slow = gs;
    }
}

impl CktElement for Recloser {
    fn cd(&self) -> &crate::elements::ckt::CktElementData {
        &self.ccd.cd
    }
    fn cd_mut(&mut self) -> &mut crate::elements::ckt::CktElementData {
        &mut self.ccd.cd
    }

    fn controlled_element(&self) -> Option<crate::elements::traits::ElemId> {
        self.ccd.controlled_element
    }

    fn recalc_element_data(&mut self, _sys: &SysCtx) {
        self.recalc();
    }

    fn calc_yprim(&mut self, _sys: &SysCtx) {}

    fn get_currents(&mut self, _sys: &SysCtx, _node_v: &[Complex64], curr: &mut [Complex64]) {
        curr.fill(Complex64::ZERO);
    }

    /// Pascal `TRecloserObj.MakePosSequence`.
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
        PosSeqPlan::base()
    }

    fn monitored_element_ref(&self) -> Option<ElemId> {
        self.ccd.monitored_element
    }
}

impl Recloser {
    /// Pascal `TRecloserObj.MakeLike` (r4133).
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

        self.ph_fast_name = other.ph_fast_name.clone();
        self.ph_slow_name = other.ph_slow_name.clone();
        self.gnd_fast_name = other.gnd_fast_name.clone();
        self.gnd_slow_name = other.gnd_slow_name.clone();
        self.ph_fast = other.ph_fast.clone();
        self.ph_slow = other.ph_slow.clone();
        self.gnd_fast = other.gnd_fast.clone();
        self.gnd_slow = other.gnd_slow.clone();

        self.ph_fast_pickup = other.ph_fast_pickup;
        self.gnd_fast_pickup = other.gnd_fast_pickup;
        self.ph_slow_pickup = other.ph_slow_pickup;
        self.gnd_slow_pickup = other.gnd_slow_pickup;
        self.ph_inst = other.ph_inst;
        self.gnd_inst = other.gnd_inst;
        self.reset_time = other.reset_time;
        self.mechanical_delay = other.mechanical_delay;
        self.num_reclose = other.num_reclose;
        self.num_fast = other.num_fast;
        self.single_ph_trip = other.single_ph_trip;
        self.single_ph_lockout = other.single_ph_lockout;
        self.rated_current = other.rated_current;
        self.interrupting_rating = other.interrupting_rating;
        self.reclose_intervals = other.reclose_intervals;
        self.f_locked = other.f_locked;

        // Per-phase state (Pascal copies FPresentState/FNormalState over the
        // controlled element's phases).
        let n = RCMAX.min(other.ccd.cd.nphases.max(1));
        for i in 1..=n {
            self.present_state[i] = other.present_state[i];
            self.normal_state[i] = other.normal_state[i];
        }
        // Pascal MakeLike does NOT copy the TD* time dials → Create defaults.
    }
}

impl DssObject for Recloser {
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
            PH_FAST_PICKUP => self.ph_fast_pickup,
            GND_FAST_PICKUP => self.gnd_fast_pickup,
            PH_INST | PHASE_INST => self.ph_inst,
            GND_INST | GROUND_INST => self.gnd_inst,
            RESET_TIME => self.reset_time,
            MECHANICAL_DELAY | DELAY => self.mechanical_delay,
            TD_PH_FAST => self.td_ph_fast,
            TD_GND_FAST | TD_GR_FAST => self.td_gnd_fast,
            TD_PH_SLOW | TD_PH_DELAYED => self.td_ph_slow,
            TD_GND_SLOW | TD_GR_DELAYED => self.td_gnd_slow,
            RATED_CURRENT => self.rated_current,
            INTERRUPTING_RATING => self.interrupting_rating,
            // Deprecated PhaseTrip/GroundTrip render the fast pickup (Pascal
            // GetPropertyValue `10, 37` / `11, 38`).
            PHASE_TRIP => self.ph_fast_pickup,
            GROUND_TRIP => self.gnd_fast_pickup,
            PH_SLOW_PICKUP => self.ph_slow_pickup,
            GND_SLOW_PICKUP => self.gnd_slow_pickup,
            BASE_FREQ => self.ccd.cd.base_frequency,
            _ => unreachable!("Recloser has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        use super::prop::*;
        match idx {
            PH_FAST_PICKUP => self.ph_fast_pickup = value,
            GND_FAST_PICKUP => self.gnd_fast_pickup = value,
            PH_INST | PHASE_INST => self.ph_inst = value,
            GND_INST | GROUND_INST => self.gnd_inst = value,
            RESET_TIME => self.reset_time = value,
            MECHANICAL_DELAY | DELAY => self.mechanical_delay = value,
            TD_PH_FAST => self.td_ph_fast = value,
            TD_GND_FAST | TD_GR_FAST => self.td_gnd_fast = value,
            TD_PH_SLOW | TD_PH_DELAYED => self.td_ph_slow = value,
            TD_GND_SLOW | TD_GR_DELAYED => self.td_gnd_slow = value,
            RATED_CURRENT => self.rated_current = value,
            INTERRUPTING_RATING => self.interrupting_rating = value,
            // Legacy PhaseTrip/GroundTrip set both fast & slow pickups.
            PHASE_TRIP => {
                self.ph_fast_pickup = value;
                self.ph_slow_pickup = value;
            }
            GROUND_TRIP => {
                self.gnd_fast_pickup = value;
                self.gnd_slow_pickup = value;
            }
            PH_SLOW_PICKUP => self.ph_slow_pickup = value,
            GND_SLOW_PICKUP => self.gnd_slow_pickup = value,
            BASE_FREQ => self.ccd.cd.base_frequency = value,
            _ => unreachable!("Recloser has no double property {idx}"),
        }
    }

    fn get_i32(&self, idx: usize) -> i32 {
        use super::prop::*;
        match idx {
            MONITORED_TERM => self.monitored_element_terminal,
            SWITCHED_TERM => self.ccd.element_terminal,
            NUM_FAST => self.num_fast,
            SHOTS => self.num_reclose, // dump subtracts the -1 value offset
            _ => unreachable!("Recloser has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        use super::prop::*;
        match idx {
            MONITORED_TERM => self.monitored_element_terminal = value,
            SWITCHED_TERM => self.ccd.element_terminal = value,
            NUM_FAST => self.num_fast = value,
            SHOTS => self.num_reclose = value,
            _ => unreachable!("Recloser has no integer property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        use super::prop::*;
        match idx {
            SINGLE_PH_TRIP => self.single_ph_trip,
            SINGLE_PH_LOCKOUT => self.single_ph_lockout,
            LOCK => self.f_locked,
            RESET_ACTION => false, // Pascal BooleanActionProperty getter: always 0
            EVENT_LOG => self.ccd.show_event_log,
            DEBUG_TRACE => self.debug_trace,
            ENABLED => self.ccd.cd.enabled,
            _ => unreachable!("Recloser has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        use super::prop::*;
        match idx {
            SINGLE_PH_TRIP => self.single_ph_trip = value,
            SINGLE_PH_LOCKOUT => self.single_ph_lockout = value,
            LOCK => self.f_locked = value,
            RESET_ACTION => {
                // Pascal BooleanActionProperty: the action fires on TRUE only.
                if value {
                    self.reset_action();
                }
            }
            EVENT_LOG => self.ccd.show_event_log = value,
            DEBUG_TRACE => self.debug_trace = value,
            ENABLED => self.ccd.cd.enabled = value,
            _ => unreachable!("Recloser has no boolean property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        use super::prop::*;
        match idx {
            MONITORED_OBJ => self.monitored_full_name.clone(),
            SWITCHED_OBJ => self.switched_full_name.clone(),
            PH_FAST_CURVE | PHASE_FAST => self.ph_fast_name.clone(),
            PH_SLOW_CURVE | PHASE_DELAYED => self.ph_slow_name.clone(),
            GND_FAST_CURVE | GROUND_FAST => self.gnd_fast_name.clone(),
            GND_SLOW_CURVE | GROUND_DELAYED => self.gnd_slow_name.clone(),
            _ => unreachable!("Recloser has no string property {idx}"),
        }
    }
    fn set_string(&mut self, idx: usize, _value: String) {
        unreachable!("Recloser has no settable string property {idx}");
    }

    /// `Action`'s `StringEnumActionProperty` (Pascal `DoAction`): ganged set +
    /// the `State` side effect.
    fn do_action(&mut self, ordinal: i32, _errors: &mut crate::diag::ErrorLog) {
        Recloser::do_action(self, ordinal);
    }

    fn array_size(&self, idx: usize) -> usize {
        use super::prop::*;
        match idx {
            RECLOSE_INTERVALS => self.num_reclose.max(0) as usize,
            NORMAL | STATE => self.state_size(),
            _ => unreachable!("Recloser has no function-sized array property {idx}"),
        }
    }
    fn get_f64_array(&self, idx: usize) -> Option<&[f64]> {
        use super::prop::*;
        match idx {
            RECLOSE_INTERVALS => Some(&self.reclose_intervals),
            _ => unreachable!("Recloser has no double-array property {idx}"),
        }
    }
    fn set_f64_array(&mut self, idx: usize, value: Vec<f64>) {
        use super::prop::*;
        match idx {
            RECLOSE_INTERVALS => {
                let n = value.len().min(RECLOSE_MAX);
                self.num_reclose = n as i32;
                self.reclose_intervals[..n].copy_from_slice(&value[..n]);
            }
            _ => unreachable!("Recloser has no double-array property {idx}"),
        }
    }

    /// `Normal`/`State` per-phase enum arrays. Render is over
    /// [`Self::state_size`]; the 1-based Pascal arrays are exposed 0-based here.
    fn get_enum_array(&self, idx: usize) -> Vec<i32> {
        use super::prop::*;
        let n = self.state_size();
        match idx {
            NORMAL => self.normal_state[1..=n].to_vec(),
            STATE => self.present_state[1..=n].to_vec(),
            _ => unreachable!("Recloser has no enum-array property {idx}"),
        }
    }
    /// Pascal `InterpretRecloserState`: a bare unquoted scalar (`state=open`)
    /// parses to a single ordinal and fills **all** phases (ganged); a quoted
    /// list (`state=[open closed open]`) fills phase-by-phase. The engine strips
    /// the brackets before this point, so a single-ordinal input is treated as
    /// ganged (the only quoted single-element form `[open]` is not exercised by
    /// any oracle-probed deck; ganged scalars and full arrays are).
    fn set_enum_array(&mut self, idx: usize, values: &[i32]) {
        use super::prop::*;
        if self.f_locked {
            return; // Pascal: state/normal writes blocked while Locked.
        }
        let n = self.state_size();
        let ganged = values.len() == 1;
        match idx {
            NORMAL => {
                if ganged {
                    self.set_all_normal(values[0]);
                } else {
                    for (k, &v) in values.iter().take(n).enumerate() {
                        self.normal_state[k + 1] = v;
                    }
                }
            }
            STATE => {
                if ganged {
                    self.set_all_present(values[0]);
                } else {
                    for (k, &v) in values.iter().take(n).enumerate() {
                        self.present_state[k + 1] = v;
                    }
                }
            }
            _ => unreachable!("Recloser has no enum-array property {idx}"),
        }
    }

    fn set_object_ref(
        &mut self,
        idx: usize,
        name: String,
        resolved: Option<(ElemId, &dyn DssObject)>,
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
                    self.mon_snap = Some(super::RefSnapshot::capture(name, elem));
                }
                None => {
                    self.monitored_full_name = name;
                    self.ccd.monitored_element = None;
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
            // The four TCC curves + deprecated aliases: record the name; the
            // executive clones them. `none` resolves to no curve silently.
            PH_FAST_CURVE | PHASE_FAST => self.ph_fast_name = curve_name(name),
            PH_SLOW_CURVE | PHASE_DELAYED => self.ph_slow_name = curve_name(name),
            GND_FAST_CURVE | GROUND_FAST => self.gnd_fast_name = curve_name(name),
            GND_SLOW_CURVE | GROUND_DELAYED => self.gnd_slow_name = curve_name(name),
            _ => unreachable!("Recloser has no object-ref property {idx}"),
        }
    }

    fn set_bus_name(&mut self, terminal: usize, value: &str) {
        self.ccd.cd.set_bus(terminal, value);
    }
    fn get_bus_name(&self, terminal: usize) -> String {
        self.ccd.cd.get_bus(terminal).to_string()
    }

    /// Pascal `TRecloserObj.PropertySideEffects`.
    fn side_effects(&mut self, idx: usize, _prev_int: i32) {
        use super::prop::*;
        match idx {
            MONITORED_OBJ => {
                self.ccd.controlled_element = self.ccd.monitored_element;
                self.switched_full_name = self.monitored_full_name.clone();
                self.ctrl_snap = self.mon_snap.clone();
            }
            MONITORED_TERM => self.ccd.element_terminal = self.monitored_element_terminal,
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

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}

/// Map a resolved TCC-curve reference name to the stored dump name: an empty
/// (unresolved / `none`) reference renders `none` (Pascal `GetPropertyValue`
/// `if Curve <> nil then Curve.Name else 'none'`).
fn curve_name(name: String) -> String {
    if name.is_empty() {
        "none".to_string()
    } else {
        name
    }
}

impl crate::elements::control::control_elem::ControlElem for Recloser {
    fn ccd(&self) -> &crate::elements::control::control_elem::ControlElemData {
        &self.ccd
    }
    fn ccd_mut(&mut self) -> &mut crate::elements::control::control_elem::ControlElemData {
        &mut self.ccd
    }
    fn control_kind(&self) -> crate::elements::control::control_elem::ControlClass {
        crate::elements::control::control_elem::ControlClass::Recloser
    }
    fn reset_control_side(&mut self) {
        Recloser::reset_control_side(self);
    }
}
