//! The [`CktElement`] and [`DssObject`] trait impls for `TRegControlObj`: the
//! no-Yprim / zero-current circuit-element surface, the typed property
//! getters/setters, `transformer=` reference resolution + shape snapshot,
//! property side-effects, and `MakeLike`.

use num_complex::Complex64;

use crate::elements::control::control_elem::RefSnapshot;
use crate::elements::pd::auto_trans::AutoTrans;
use crate::elements::pd::transformer::Transformer;
use crate::elements::traits::{CktElement, ElemRef, SysCtx};
use crate::obj::base::{DssObjData, DssObject, RefAction};

use super::{RegControl, prop};

impl CktElement for RegControl {
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

    /// Pascal `TControlElem.CalcYPrim`: leave YPrim as NIL — `BuildYMatrix`
    /// skips elements with no primitive matrix.
    fn calc_yprim(&mut self, _sys: &SysCtx) {}

    /// Pascal `TControlElem.GetCurrents`: always zero.
    fn get_currents(&mut self, _sys: &SysCtx, _node_v: &[Complex64], curr: &mut [Complex64]) {
        curr.fill(Complex64::ZERO);
    }
}

impl DssObject for RegControl {
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
            VREG => self.vreg,
            BAND => self.bandwidth,
            PTRATIO => self.pt_ratio,
            CTPRIM => self.ct_rating,
            R => self.r,
            X => self.x,
            DELAY => self.ccd.time_delay,
            REVVREG => self.rev_vreg,
            REVBAND => self.rev_bandwidth,
            REVR => self.rev_r,
            REVX => self.rev_x,
            TAPDELAY => self.tap_delay,
            VLIMIT => self.vlimit,
            REVTHRESHOLD => self.kw_rev_power_threshold,
            REVDELAY => self.rev_delay,
            REMOTEPTRATIO => self.remote_pt_ratio,
            LDC_Z => self.ldc_z,
            REV_Z => self.rev_ldc_z,
            BASE_FREQ => self.ccd.cd.base_frequency,
            _ => unreachable!("RegControl has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        use prop::*;
        match idx {
            VREG => self.vreg = value,
            BAND => self.bandwidth = value,
            PTRATIO => self.pt_ratio = value,
            CTPRIM => self.ct_rating = value,
            R => self.r = value,
            X => self.x = value,
            DELAY => self.ccd.time_delay = value,
            REVVREG => self.rev_vreg = value,
            REVBAND => self.rev_bandwidth = value,
            REVR => self.rev_r = value,
            REVX => self.rev_x = value,
            TAPDELAY => self.tap_delay = value,
            VLIMIT => self.vlimit = value,
            REVTHRESHOLD => self.kw_rev_power_threshold = value,
            REVDELAY => self.rev_delay = value,
            REMOTEPTRATIO => self.remote_pt_ratio = value,
            LDC_Z => self.ldc_z = value,
            REV_Z => self.rev_ldc_z = value,
            BASE_FREQ => self.ccd.cd.base_frequency = value,
            _ => unreachable!("RegControl has no double property {idx}"),
        }
    }

    fn get_i32(&self, idx: usize) -> i32 {
        use prop::*;
        match idx {
            WINDING => self.ccd.element_terminal,
            MAXTAPCHANGE => self.tap_limit_per_change,
            TAPWINDING => self.tap_winding,
            PTPHASE => self.fpt_phase,
            TAPNUM => self.get_tap_num(),
            _ => unreachable!("RegControl has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        use prop::*;
        match idx {
            WINDING => self.ccd.element_terminal = value,
            MAXTAPCHANGE => self.tap_limit_per_change = value,
            TAPWINDING => self.tap_winding = value,
            PTPHASE => self.fpt_phase = value,
            TAPNUM => self.set_tap_num(value),
            _ => unreachable!("RegControl has no integer property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        use prop::*;
        match idx {
            REVERSIBLE => self.is_reversible,
            DEBUGTRACE => self.debug_trace,
            INVERSETIME => self.inverse_time,
            REVNEUTRAL => self.reverse_neutral,
            EVENTLOG => self.ccd.show_event_log,
            COGEN => self.cogen_enabled,
            RESET => false, // Pascal BooleanActionProperty getter: always 0
            ENABLED => self.ccd.cd.enabled,
            _ => unreachable!("RegControl has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        use prop::*;
        match idx {
            REVERSIBLE => self.is_reversible = value,
            DEBUGTRACE => self.debug_trace = value,
            INVERSETIME => self.inverse_time = value,
            REVNEUTRAL => self.reverse_neutral = value,
            EVENTLOG => self.ccd.show_event_log = value,
            COGEN => self.cogen_enabled = value,
            RESET => {
                // Pascal BooleanActionProperty: the action fires on TRUE only.
                if value {
                    self.reset();
                }
            }
            // Pascal `TRegControlObj.Set_Enabled` override: only toggle the
            // flag — no BusNameRedefined side effect.
            ENABLED => self.ccd.cd.enabled = value,
            _ => unreachable!("RegControl has no boolean property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        use prop::*;
        match idx {
            TRANSFORMER => self.controlled_name.clone(),
            BUS => self.regulated_bus.clone(),
            _ => unreachable!("RegControl has no string property {idx}"),
        }
    }
    fn set_string(&mut self, idx: usize, value: String) {
        match idx {
            prop::BUS => self.regulated_bus = value,
            _ => unreachable!("RegControl has no string property {idx}"),
        }
    }

    /// `transformer=` resolution: keep the `ElemRef` and snapshot the
    /// transformer's shape + per-winding tap data for `RecalcElementData` and
    /// `TapNum` (which run after the foreign view is gone).
    fn set_object_ref(
        &mut self,
        idx: usize,
        name: String,
        resolved: Option<(ElemRef, &dyn DssObject)>,
    ) {
        debug_assert_eq!(idx, prop::TRANSFORMER);
        self.controlled_name = name;
        match resolved {
            Some((r, obj)) => {
                self.ccd.controlled_element = Some(r);
                let elem = obj
                    .as_ckt_element()
                    .expect("controlled element is a circuit element");
                // `transformer=` resolves against either class (Pascal
                // `Transf_Or_AutoTrans_ProxyClass`).
                let name = obj.data().name();
                let (full_name, tap_snap) =
                    if let Some(xf) = obj.as_any().downcast_ref::<Transformer>() {
                        (
                            format!("Transformer.{name}"),
                            (1..=xf.num_windings().max(0) as usize)
                                .map(|i| xf.winding_tap_data(i))
                                .collect(),
                        )
                    } else if let Some(at) = obj.as_any().downcast_ref::<AutoTrans>() {
                        (
                            format!("AutoTrans.{name}"),
                            (1..=at.num_windings().max(0) as usize)
                                .map(|i| at.winding_tap_data(i))
                                .collect(),
                        )
                    } else {
                        (format!("Transformer.{name}"), Vec::new())
                    };
                self.snapshot = Some(RefSnapshot::capture(full_name, elem));
                self.tap_snap = tap_snap;
            }
            None => {
                self.ccd.controlled_element = None;
                self.snapshot = None;
                self.tap_snap.clear();
            }
        }
    }

    fn set_bus_name(&mut self, terminal: usize, value: &str) {
        self.ccd.cd.set_bus(terminal, value);
    }
    fn get_bus_name(&self, terminal: usize) -> String {
        self.ccd.cd.get_bus(terminal).to_string()
    }

    /// Pascal `TRegControlObj.PropertySideEffects`.
    fn side_effects(&mut self, idx: usize, _prev_int: i32) {
        use prop::*;
        match idx {
            TRANSFORMER => {
                // MonitoredElement := ControlledElement (same for this
                // controller). Pascal also sets PrpSequence[transformer] :=
                // -10 so Save writes it first; Save is not ported (Phase 6+),
                // so the mark is omitted.
                self.ccd.monitored_element = self.ccd.controlled_element;
            }
            WINDING => {
                // Resets if property re-assigned.
                self.tap_winding = self.ccd.element_terminal;
            }
            PTRATIO => {
                // re-initialise RemotePTRatio whenever PTRatio is set
                self.remote_pt_ratio = self.pt_ratio;
            }
            DEBUGTRACE => {
                // Pascal opens/closes the REG_<name>.csv trace file here; the
                // tap-changing machinery that writes it is Phase 5, so the
                // flag is stored without the file.
            }
            MAXTAPCHANGE => {
                self.tap_limit_per_change = self.tap_limit_per_change.max(0);
            }
            REVTHRESHOLD => {
                self.rev_power_threshold = self.kw_rev_power_threshold * 1000.0;
            }
            _ => {}
        }
    }

    /// Pascal `TCktElementClass.EndEdit` default → `RecalcElementData`.
    fn end_edit(&mut self) {
        self.recalc();
    }

    fn take_ref_actions(&mut self) -> Vec<RefAction> {
        std::mem::take(&mut self.pending_actions)
    }

    /// Pascal `TRegControlObj.MakeLike`.
    fn make_like(&mut self, other: &dyn DssObject) {
        let Some(other) = other.as_any().downcast_ref::<RegControl>() else {
            return;
        };
        self.ccd.cd.make_like_base(&other.ccd.cd);
        self.ccd.cd.nphases = other.ccd.cd.nphases;
        let nc = other.ccd.cd.nconds;
        self.ccd.cd.set_nconds(nc); // Force reallocation of terminal stuff

        // ControlledElement := Other.ControlledElement (pointer copy; the
        // Pascal HasControl/ControlElementList bookkeeping only matters for
        // element deletion, which is not supported).
        self.ccd.controlled_element = other.ccd.controlled_element;
        self.ccd.monitored_element = other.ccd.monitored_element;
        self.controlled_name = other.controlled_name.clone();
        self.snapshot = other.snapshot.clone();
        self.tap_snap = other.tap_snap.clone();

        self.ccd.element_terminal = other.ccd.element_terminal;
        self.vreg = other.vreg;
        self.bandwidth = other.bandwidth;
        self.pt_ratio = other.pt_ratio;
        self.remote_pt_ratio = other.remote_pt_ratio;
        self.ct_rating = other.ct_rating;
        self.r = other.r;
        self.x = other.x;
        self.regulated_bus = other.regulated_bus.clone();
        self.ccd.time_delay = other.ccd.time_delay;
        self.is_reversible = other.is_reversible;
        self.rev_vreg = other.rev_vreg;
        self.rev_bandwidth = other.rev_bandwidth;
        self.rev_r = other.rev_r;
        self.rev_x = other.rev_x;
        self.tap_delay = other.tap_delay;
        self.tap_winding = other.tap_winding;
        self.inverse_time = other.inverse_time;
        self.tap_limit_per_change = other.tap_limit_per_change;
        self.kw_rev_power_threshold = other.kw_rev_power_threshold;
        self.rev_power_threshold = other.rev_power_threshold;
        self.rev_delay = other.rev_delay;
        self.reverse_neutral = other.reverse_neutral;
        self.ccd.show_event_log = other.ccd.show_event_log;
        // DebugTrace := Other.DebugTrace;  Always default to NO
        self.fpt_phase = other.fpt_phase;
        // TapNum := Other.TapNum — runs the property setter, repositioning the
        // (copied) controlled transformer's tap; with untouched taps this is a
        // no-op write of the mid-tap.
        self.set_tap_num(other.get_tap_num());
        self.cogen_enabled = other.cogen_enabled;
        self.ldc_z = other.ldc_z;
        self.rev_ldc_z = other.rev_ldc_z;
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}
