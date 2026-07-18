//! The trait plumbing for `TFuseObj`: the [`CktElement`] hooks (a control
//! element builds no Yprim and carries zero current) and the [`DssObject`]
//! property accessors (`get_*`/`set_*`, `set_object_ref`, `PropertySideEffects`,
//! `EndEdit`, `MakeLike`, `take_ref_actions`), plus the per-phase enum-array
//! (`Normal`/`State`) hooks and the executive-driven `FuseCurve` resolution.

use num_complex::Complex64;

use crate::elements::general::tcc_curve::TccCurveObj;
use crate::elements::traits::{CktElement, ElemRef, SysCtx};
use crate::obj::base::{DssObjData, DssObject, RefAction};

use super::{FUSEMAXDIM, Fuse};

impl Fuse {
    /// Executive hook: the `FuseCurve` name to resolve against the TCC_Curve
    /// registry (Pascal constructor `Find('tlink')` / `fusecurve=` parse). Empty
    /// when a failed resolution left the reference NIL.
    pub fn fuse_curve_name(&self) -> &str {
        &self.fuse_curve_name
    }

    /// Executive hook: install the resolved `FuseCurve` clone (or `None` when the
    /// name does not resolve — a NIL fuse never trips).
    pub fn set_fuse_curve_obj(&mut self, curve: Option<TccCurveObj>) {
        self.fuse_curve_obj = curve;
    }
}

impl CktElement for Fuse {
    fn cd(&self) -> &crate::elements::ckt::CktElementData {
        &self.ccd.cd
    }
    fn cd_mut(&mut self) -> &mut crate::elements::ckt::CktElementData {
        &mut self.ccd.cd
    }

    /// Pascal `TControlElem.FControlledElement` — the line/element this fuse
    /// switches (`Fuse` is a `TControlElem`; `SwitchedObj` binds `FControlledElement`).
    fn controlled_element(&self) -> Option<crate::elements::traits::ElemRef> {
        self.ccd.controlled_element
    }

    fn recalc_element_data(&mut self, _sys: &SysCtx) {
        self.recalc();
    }

    /// Pascal `TFuseObj.CalcYPrim`: leave YPrim as NIL (always zero for a fuse).
    fn calc_yprim(&mut self, _sys: &SysCtx) {}

    /// Pascal `TFuseObj.GetCurrents`: always zero.
    fn get_currents(&mut self, _sys: &SysCtx, _node_v: &[Complex64], curr: &mut [Complex64]) {
        curr.fill(Complex64::ZERO);
    }
}

impl DssObject for Fuse {
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
            RATED_CURRENT => self.rated_current,
            CURVE_MULTIPLIER => self.curve_multiplier,
            INTERRUPTING_RATING => self.interrupting_rating,
            DELAY => self.delay_time,
            BASE_FREQ => self.ccd.cd.base_frequency,
            _ => unreachable!("Fuse has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        use super::prop::*;
        match idx {
            RATED_CURRENT => self.rated_current = value,
            CURVE_MULTIPLIER => self.curve_multiplier = value,
            INTERRUPTING_RATING => self.interrupting_rating = value,
            DELAY => self.delay_time = value,
            BASE_FREQ => self.ccd.cd.base_frequency = value,
            _ => unreachable!("Fuse has no double property {idx}"),
        }
    }

    fn get_i32(&self, idx: usize) -> i32 {
        use super::prop::*;
        match idx {
            MONITORED_TERM => self.monitored_element_terminal,
            SWITCHED_TERM => self.ccd.element_terminal,
            _ => unreachable!("Fuse has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        use super::prop::*;
        match idx {
            MONITORED_TERM => self.monitored_element_terminal = value,
            SWITCHED_TERM => self.ccd.element_terminal = value,
            _ => unreachable!("Fuse has no integer property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        use super::prop::*;
        match idx {
            ENABLED => self.ccd.cd.enabled,
            _ => unreachable!("Fuse has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        use super::prop::*;
        match idx {
            // Pascal control elements have no `Set_Enabled` side effect.
            ENABLED => self.ccd.cd.enabled = value,
            _ => unreachable!("Fuse has no boolean property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        use super::prop::*;
        match idx {
            MONITORED_OBJ => self.monitored_full_name.clone(),
            SWITCHED_OBJ => self.switched_full_name.clone(),
            // r4133 `GetPropertyValue` prop 5: `if FuseCurve <> nil then
            // FuseCurve.Name else 'none'` — render the resolved curve's name, or
            // the literal `none` when NIL (default, `fusecurve=none`, or a missed
            // resolve). The executive resolves `fuse_curve_obj` after every edit.
            FUSE_CURVE => self
                .fuse_curve_obj
                .as_ref()
                .map(|c| c.data().name().to_string())
                .unwrap_or_else(|| "none".to_string()),
            _ => unreachable!("Fuse has no string property {idx}"),
        }
    }
    fn set_string(&mut self, idx: usize, _value: String) {
        unreachable!("Fuse has no settable string property {idx}");
    }

    /// `Action`'s `StringEnumActionProperty` (Pascal `DoAction`): set all phases
    /// then run the `State` side effect.
    fn do_action(&mut self, ordinal: i32, _errors: &mut crate::diag::ErrorLog) {
        self.do_fuse_action(ordinal);
    }

    /// The per-phase enum-array count (`GetFuseStateSize`).
    fn array_size(&self, idx: usize) -> usize {
        use super::prop::*;
        match idx {
            NORMAL | STATE => self.fuse_state_size(),
            _ => unreachable!("Fuse has no function-sized array property {idx}"),
        }
    }
    fn get_enum_array(&self, idx: usize) -> Vec<i32> {
        use super::prop::*;
        match idx {
            NORMAL => self.normal_state.to_vec(),
            STATE => self.present_state.to_vec(),
            _ => unreachable!("Fuse has no enum-array property {idx}"),
        }
    }
    fn set_enum_array(&mut self, idx: usize, values: &[i32]) {
        use super::prop::*;
        // A short list sets only the leading phases (the rest keep their value).
        let n = values.len().min(FUSEMAXDIM);
        match idx {
            NORMAL => self.normal_state[..n].copy_from_slice(&values[..n]),
            STATE => self.present_state[..n].copy_from_slice(&values[..n]),
            _ => unreachable!("Fuse has no enum-array property {idx}"),
        }
    }

    /// `monitoredobj=`/`switchedobj=` (any element) + `fusecurve=` (TCC_Curve):
    /// store the name + snapshot. The `FuseCurve` clone is resolved by the
    /// executive from `fuse_curve_name` after the edit.
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
            // `fusecurve=`: just record the name; the executive clones the curve.
            FUSE_CURVE => self.fuse_curve_name = name,
            _ => unreachable!("Fuse has no object-ref property {idx}"),
        }
    }

    fn set_bus_name(&mut self, terminal: usize, value: &str) {
        self.ccd.cd.set_bus(terminal, value);
    }
    fn get_bus_name(&self, terminal: usize) -> String {
        self.ccd.cd.get_bus(terminal).to_string()
    }

    /// Pascal `TFuseObj.PropertySideEffects`.
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
            STATE => self.state_side_effect(),
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

    /// Pascal `TFuseObj.MakeLike`.
    fn make_like(&mut self, other: &dyn DssObject) {
        let Some(other) = other.as_any().downcast_ref::<Fuse>() else {
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

        self.fuse_curve_name = other.fuse_curve_name.clone();
        self.fuse_curve_obj = other.fuse_curve_obj.clone();
        self.rated_current = other.rated_current;
        // r4133 (WP-U2.1): MakeLike copies the new divisor + interrupting rating.
        self.curve_multiplier = other.curve_multiplier;
        self.interrupting_rating = other.interrupting_rating;

        // Pascal copies the first `min(FUSEMAXDIM, ControlledElement.NPhases)`
        // per-phase states; with no controlled element it copies none.
        let n = other.controlled_nphases();
        self.present_state[..n].copy_from_slice(&other.present_state[..n]);
        self.normal_state[..n].copy_from_slice(&other.normal_state[..n]);
        // Pascal `MakeLike` does not copy `NormalStateSet` (stays at the Create
        // default), so neither do we.
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}

impl crate::elements::control::control_elem::ControlElem for Fuse {
    fn ccd(&self) -> &crate::elements::control::control_elem::ControlElemData {
        &self.ccd
    }
    fn ccd_mut(&mut self) -> &mut crate::elements::control::control_elem::ControlElemData {
        &mut self.ccd
    }
    fn control_kind(&self) -> crate::elements::control::control_elem::ControlClass {
        crate::elements::control::control_elem::ControlClass::Fuse
    }
}
