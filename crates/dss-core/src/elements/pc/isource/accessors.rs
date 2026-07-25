//! The `impl DssObject` property surface for [`Isource`] — scalar/string
//! getters & setters, bus names, object refs, `PropertySideEffects`,
//! `EndEdit`, `MakeLike`.

use num_complex::Complex64;

use super::Isource;
use crate::elements::general::load_shape::LoadShapeObj;
use crate::elements::traits::{CktElement, ElemRef};
use crate::obj::base::{DssObjData, DssObject};

impl Isource {
    /// Pascal `TIsourceObj.MakeLike`. Note: like the Pascal source, this does
    /// **not** recompute `phase_shift` from the (possibly just-copied)
    /// `nphases`, and does not copy `per_unit` — neither is touched by
    /// `Isource.pas:275-303` either.
    pub(crate) fn make_like(&mut self, other: &Self) {
        self.cd.make_like_base(&other.cd);
        if self.cd.nphases != other.cd.nphases {
            self.cd.nphases = other.cd.nphases;
            let n = other.cd.nphases;
            self.cd.set_nconds(n);
            self.cd.yprim_invalid = true;
        }
        self.amps = other.amps;
        self.angle = other.angle;
        self.src_frequency = other.src_frequency;
        self.scan_type = other.scan_type;
        self.sequence_type = other.sequence_type;
        self.shape_is_actual = other.shape_is_actual;
        self.daily_shape = other.daily_shape.clone();
        self.duty_shape = other.duty_shape.clone();
        self.yearly_shape = other.yearly_shape.clone();
        self.daily_shape_obj = other.daily_shape_obj.clone();
        self.duty_shape_obj = other.duty_shape_obj.clone();
        self.yearly_shape_obj = other.yearly_shape_obj.clone();
        self.daily_shape_ref = other.daily_shape_ref;
        self.duty_shape_ref = other.duty_shape_ref;
        self.yearly_shape_ref = other.yearly_shape_ref;
        self.bus2_defined = other.bus2_defined;
        // Pascal `MakeLike` (inherited) also copies spectrum via the generic
        // path; mirrored here for the same reason VSource/Load do.
        self.spectrum = other.spectrum.clone();
        self.spectrum_obj = other.spectrum_obj.clone();
        self.cd.inj_current = vec![Complex64::ZERO; self.cd.yorder];
    }
}

impl DssObject for Isource {
    fn data(&self) -> &DssObjData {
        &self.cd.obj
    }
    fn data_mut(&mut self) -> &mut DssObjData {
        &mut self.cd.obj
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
            AMPS => self.amps,
            ANGLE => self.angle,
            FREQUENCY => self.src_frequency,
            BASE_FREQ => self.cd.base_frequency,
            _ => unreachable!("Isource has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        use super::prop::*;
        match idx {
            AMPS => self.amps = value,
            ANGLE => self.angle = value,
            FREQUENCY => self.src_frequency = value,
            BASE_FREQ => self.cd.base_frequency = value,
            _ => unreachable!("Isource has no double property {idx}"),
        }
    }

    fn get_i32(&self, idx: usize) -> i32 {
        use super::prop::*;
        match idx {
            PHASES => self.cd.nphases as i32,
            SCAN_TYPE => self.scan_type,
            SEQUENCE => self.sequence_type,
            _ => unreachable!("Isource has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        use super::prop::*;
        match idx {
            PHASES => self.cd.nphases = value.max(0) as usize,
            SCAN_TYPE => self.scan_type = value,
            SEQUENCE => self.sequence_type = value,
            _ => unreachable!("Isource has no integer property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        match idx {
            super::prop::ENABLED => self.cd.enabled,
            _ => unreachable!("Isource has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        match idx {
            super::prop::ENABLED => self.cd.set_enabled(value),
            _ => unreachable!("Isource has no boolean property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        use super::prop::*;
        match idx {
            YEARLY => self.yearly_shape.clone(),
            DAILY => self.daily_shape.clone(),
            DUTY => self.duty_shape.clone(),
            SPECTRUM => self.spectrum.clone(),
            _ => unreachable!("Isource has no string property {idx}"),
        }
    }
    fn set_string(&mut self, idx: usize, value: String) {
        use super::prop::*;
        match idx {
            YEARLY => self.yearly_shape = value,
            DAILY => self.daily_shape = value,
            DUTY => self.duty_shape = value,
            SPECTRUM => self.spectrum = value,
            _ => unreachable!("Isource has no string property {idx}"),
        }
    }

    fn set_bus_name(&mut self, terminal: usize, value: &str) {
        self.cd.set_bus(terminal, value);
    }
    fn get_bus_name(&self, terminal: usize) -> String {
        self.cd.get_bus(terminal).to_string()
    }

    /// Resolve a shape reference (`yearly`/`daily`/`duty` → `LoadShape`): store
    /// the name (for the dump), the `ElemRef`, and a snapshot clone the
    /// time-series `GetBaseCurr` drives — same pattern as
    /// [`super::super::vsource::VSource`]/[`super::super::load::Load`].
    fn set_object_ref(
        &mut self,
        idx: usize,
        name: String,
        resolved: Option<(ElemRef, &dyn DssObject)>,
    ) {
        use super::prop::*;
        let elem_ref = resolved.map(|(r, _)| r);
        let load_shape =
            || resolved.and_then(|(_, o)| o.as_any().downcast_ref::<LoadShapeObj>().cloned());
        match idx {
            YEARLY => {
                self.yearly_shape = name;
                self.yearly_shape_ref = elem_ref;
                self.yearly_shape_obj = load_shape();
            }
            DAILY => {
                self.daily_shape = name;
                self.daily_shape_ref = elem_ref;
                self.daily_shape_obj = load_shape();
            }
            DUTY => {
                self.duty_shape = name;
                self.duty_shape_ref = elem_ref;
                self.duty_shape_obj = load_shape();
            }
            _ => unreachable!("Isource has no resolved object-ref property {idx}"),
        }
    }

    /// Pascal `TIsourceObj.PropertySideEffects` (`Isource.pas:221`).
    fn side_effects(&mut self, idx: usize, _prev_int: i32) {
        use super::prop::*;
        match idx {
            PHASES => {
                let n = self.cd.nphases;
                self.phase_shift = match n {
                    1 => 0.0,
                    2 | 3 => 120.0,
                    _ => 360.0 / n as f64,
                };
                self.cd.set_nconds(n); // Force reallocation of terminal info
            }
            BUS1 => {
                // Default Bus2 to the zero node of Bus1 (grounded-Y), unless
                // Bus2Defined — which Isource, unlike VSource, never sets true
                // (see the TODO(compat) note on `Isource::bus2_defined` in
                // `mod.rs`): this branch always fires when Bus1 is (re)set.
                if !self.bus2_defined {
                    let s = self.cd.get_bus(1).to_string();
                    let mut s2 = match s.find('.') {
                        Some(dot) => s[..dot].to_string(),
                        None => s,
                    };
                    for _ in 0..self.cd.nphases {
                        s2.push_str(".0");
                    }
                    self.cd.set_bus(2, &s2);
                }
            }
            // Index 9 = Daily: if the yearly shape is not yet defined, make it
            // the daily one (Pascal `YearlyShapeObj := DailyShapeObj`).
            DAILY if self.yearly_shape_obj.is_none() => {
                self.yearly_shape_obj = self.daily_shape_obj.clone();
                self.yearly_shape_ref = self.daily_shape_ref;
                self.yearly_shape = self.daily_shape.clone();
            }
            _ => {}
        }
    }

    /// Pascal `TIsource.EndEdit`.
    fn end_edit(&mut self, _sys: &crate::elements::traits::SysCtx) {
        self.recalc();
        self.cd.yprim_invalid = true;
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}
