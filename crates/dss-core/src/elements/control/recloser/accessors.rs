//! The trait plumbing for `TRecloserObj`: the [`CktElement`] hooks (a control
//! element builds no Yprim and carries zero current) and the [`DssObject`]
//! property accessors (`get_*`/`set_*`, `set_object_ref`, `PropertySideEffects`,
//! `EndEdit`, `MakeLike`, `take_ref_actions`), plus the executive-driven
//! resolution of the four TCC curves.

use num_complex::Complex64;

use crate::elements::general::tcc_curve::TccCurveObj;
use crate::elements::pos_seq::{PosSeqCtx, PosSeqPlan};
use crate::elements::traits::{CktElement, ElemRef, SysCtx};
use crate::obj::base::{DssObjData, DssObject, RefAction};

use super::{RECLOSE_MAX, Recloser};

impl Recloser {
    /// Executive hook: the four TCC curve names (PhaseFast / PhaseDelayed /
    /// GroundFast / GroundDelayed) to resolve against the TCC_Curve registry
    /// (the constructor defaults `a`/`d` + any `phasefast=`/… parse). Empty
    /// where the reference is NIL.
    pub fn curve_names(&self) -> [String; 4] {
        [
            self.phase_fast_name.clone(),
            self.phase_delayed_name.clone(),
            self.ground_fast_name.clone(),
            self.ground_delayed_name.clone(),
        ]
    }

    /// Executive hook: install the four resolved TCC curve clones (in the
    /// [`Self::curve_names`] order). A `None` curve never trips its branch.
    pub fn set_resolved_curves(&mut self, curves: [Option<TccCurveObj>; 4]) {
        let [pf, pd, gf, gd] = curves;
        self.phase_fast = pf;
        self.phase_delayed = pd;
        self.ground_fast = gf;
        self.ground_delayed = gd;
    }
}

impl CktElement for Recloser {
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

    /// Pascal `TRecloserObj.MakePosSequence` (`Controls/Recloser.pas:460`):
    /// adopt the monitored element's phase / conductor counts and attach
    /// terminal 1 to its bus, then run the base bus rename (`inherited`). Pascal
    /// NIL-guards `MonitoredElement`; `ctx.monitored` is `None` in the same case.
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
            // ReAllocMem(cBuffer, ..) + CondOffset: no persistent field — the
            // sampler sizes `cbuffer` and computes `cond_offset` as locals each
            // `Sample` from the live monitored element.
        }
        // inherited MakePosSequence -> base bus rename.
        PosSeqPlan::base()
    }

    /// Pascal `TControlElem.MonitoredElement` — resolved so the exec applier can
    /// build [`PosSeqCtx::monitored`] before calling [`Self::make_pos_sequence`].
    fn monitored_element_ref(&self) -> Option<ElemRef> {
        self.ccd.monitored_element
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

    fn get_f64(&self, idx: usize) -> f64 {
        use super::prop::*;
        match idx {
            PHASE_TRIP => self.phase_trip,
            GROUND_TRIP => self.ground_trip,
            PHASE_INST => self.phase_inst,
            GROUND_INST => self.ground_inst,
            RESET => self.reset_time,
            DELAY => self.delay_time,
            TD_PH_FAST => self.td_ph_fast,
            TD_GR_FAST => self.td_gr_fast,
            TD_PH_DELAYED => self.td_ph_delayed,
            TD_GR_DELAYED => self.td_gr_delayed,
            BASE_FREQ => self.ccd.cd.base_frequency,
            _ => unreachable!("Recloser has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        use super::prop::*;
        match idx {
            PHASE_TRIP => self.phase_trip = value,
            GROUND_TRIP => self.ground_trip = value,
            PHASE_INST => self.phase_inst = value,
            GROUND_INST => self.ground_inst = value,
            RESET => self.reset_time = value,
            DELAY => self.delay_time = value,
            TD_PH_FAST => self.td_ph_fast = value,
            TD_GR_FAST => self.td_gr_fast = value,
            TD_PH_DELAYED => self.td_ph_delayed = value,
            TD_GR_DELAYED => self.td_gr_delayed = value,
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
            // Shots aliases NumReclose; the dump subtracts the -1 value offset.
            SHOTS => self.num_reclose,
            // Action/State read FPresentState; Normal reads NormalState. The
            // text dump renders these via the property's own enum (the
            // `get_PresentState` getter is unused by `?` — Pascal comment).
            ACTION | STATE => self.present_state,
            NORMAL => self.normal_state,
            _ => unreachable!("Recloser has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        use super::prop::*;
        match idx {
            MONITORED_TERM => self.monitored_element_terminal = value,
            SWITCHED_TERM => self.ccd.element_terminal = value,
            NUM_FAST => self.num_fast = value,
            // The engine has already applied the -1 value offset, so `value` is
            // NumReclose directly.
            SHOTS => self.num_reclose = value,
            ACTION | STATE => self.present_state = value,
            NORMAL => self.normal_state = value,
            _ => unreachable!("Recloser has no integer property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        use super::prop::*;
        match idx {
            ENABLED => self.ccd.cd.enabled,
            _ => unreachable!("Recloser has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        use super::prop::*;
        match idx {
            // Pascal control elements have no `Set_Enabled` side effect.
            ENABLED => self.ccd.cd.enabled = value,
            _ => unreachable!("Recloser has no boolean property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        use super::prop::*;
        match idx {
            MONITORED_OBJ => self.monitored_full_name.clone(),
            SWITCHED_OBJ => self.switched_full_name.clone(),
            PHASE_FAST => self.phase_fast_name.clone(),
            PHASE_DELAYED => self.phase_delayed_name.clone(),
            GROUND_FAST => self.ground_fast_name.clone(),
            GROUND_DELAYED => self.ground_delayed_name.clone(),
            _ => unreachable!("Recloser has no string property {idx}"),
        }
    }
    fn set_string(&mut self, idx: usize, _value: String) {
        unreachable!("Recloser has no settable string property {idx}");
    }

    /// `RecloseIntervals` is the only double-array property; it is sized by
    /// `NumReclose` (the dump renders that many of the fixed-`RECLOSE_MAX`
    /// buffer). Always non-nil, so an empty count dumps `[]` (not `''`).
    fn array_size(&self, idx: usize) -> usize {
        use super::prop::*;
        match idx {
            RECLOSE_INTERVALS => self.num_reclose.max(0) as usize,
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
                // The parse hands us the supplied values (already clamped to
                // `RECLOSE_MAX`); NumReclose tracks the count, the rest of the
                // fixed buffer keeps its prior (dead) value.
                let n = value.len().min(RECLOSE_MAX);
                self.num_reclose = n as i32;
                self.reclose_intervals[..n].copy_from_slice(&value[..n]);
            }
            _ => unreachable!("Recloser has no double-array property {idx}"),
        }
    }

    /// `monitoredobj=`/`switchedobj=` (any element) + the four `…=` TCC curves:
    /// store the name + snapshot. The curve clones are resolved by the executive
    /// from the curve names after the edit (the Fuse `fusecurve` pattern).
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
            // The four TCC curves: record the name; the executive clones them.
            PHASE_FAST => self.phase_fast_name = name,
            PHASE_DELAYED => self.phase_delayed_name = name,
            GROUND_FAST => self.ground_fast_name = name,
            GROUND_DELAYED => self.ground_delayed_name = name,
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
            // Default the controlled element to the monitored element.
            MONITORED_OBJ => {
                self.ccd.controlled_element = self.ccd.monitored_element;
                self.switched_full_name = self.monitored_full_name.clone();
                self.ctrl_snap = self.mon_snap.clone();
            }
            MONITORED_TERM => self.ccd.element_terminal = self.monitored_element_terminal,
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

    /// Pascal `TRecloserObj.MakeLike`.
    fn make_like(&mut self, other: &dyn DssObject) {
        let Some(other) = other.as_any().downcast_ref::<Recloser>() else {
            return;
        };
        self.ccd.cd.make_like_base(&other.ccd.cd);
        self.ccd.cd.nphases = other.ccd.cd.nphases;
        let nc = other.ccd.cd.nconds;
        self.ccd.cd.set_nconds(nc); // Force Reallocation of terminal stuff

        self.ccd.element_terminal = other.ccd.element_terminal;
        self.ccd.controlled_element = other.ccd.controlled_element;
        self.ccd.monitored_element = other.ccd.monitored_element;
        self.monitored_element_terminal = other.monitored_element_terminal;
        self.monitored_full_name = other.monitored_full_name.clone();
        self.switched_full_name = other.switched_full_name.clone();
        self.mon_snap = other.mon_snap.clone();
        self.ctrl_snap = other.ctrl_snap.clone();

        // Pascal copies the four curve pointers; we copy name + resolved clone.
        self.phase_fast_name = other.phase_fast_name.clone();
        self.phase_delayed_name = other.phase_delayed_name.clone();
        self.ground_fast_name = other.ground_fast_name.clone();
        self.ground_delayed_name = other.ground_delayed_name.clone();
        self.phase_fast = other.phase_fast.clone();
        self.phase_delayed = other.phase_delayed.clone();
        self.ground_fast = other.ground_fast.clone();
        self.ground_delayed = other.ground_delayed.clone();

        self.phase_trip = other.phase_trip;
        self.ground_trip = other.ground_trip;
        self.phase_inst = other.phase_inst;
        self.ground_inst = other.ground_inst;
        self.reset_time = other.reset_time;
        self.num_reclose = other.num_reclose;
        self.num_fast = other.num_fast;
        // RecloseIntervals[1..NumReclose] (the tail beyond NumReclose is dead).
        self.reclose_intervals = other.reclose_intervals;

        self.locked_out = other.locked_out;
        self.present_state = other.present_state;
        self.normal_state = other.normal_state;
        self.normal_state_set = other.normal_state_set;
        // Pascal MakeLike does NOT copy DelayTime or the TD* time dials; they
        // keep this object's Create defaults.
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}
