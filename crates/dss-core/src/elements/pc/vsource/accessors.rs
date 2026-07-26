//! The `impl DssObject` property surface for [`VSource`] — scalar/string/complex
//! getters & setters, bus names, object refs, `PropertySideEffects`, `EndEdit`,
//! `MakeLike`.

use num_complex::Complex64;

use super::VSource;
use crate::elements::general::load_shape::LoadShapeObj;
use crate::elements::traits::CktElement;
use crate::obj::arena::ResolvedObj;
use crate::obj::base::{DssObjData, DssObject};

impl VSource {
    /// Pascal `TVsourceObj.MakeLike`.
    pub(crate) fn make_like(&mut self, other: &Self) {
        self.cd.make_like_base(&other.cd);
        if self.cd.nphases != other.cd.nphases {
            self.cd.nphases = other.cd.nphases;
            let n = other.cd.nphases;
            self.cd.set_nconds(n);
            self.cd.yprim_invalid = true;
        }
        self.z = other.z.clone();
        self.vmag = other.vmag;
        self.kv_base = other.kv_base;
        self.base_mva = other.base_mva;
        self.per_unit = other.per_unit;
        self.angle = other.angle;
        self.mva_sc3 = other.mva_sc3;
        self.mva_sc1 = other.mva_sc1;
        self.scan_type = other.scan_type;
        self.sequence_type = other.sequence_type;
        self.src_frequency = other.src_frequency;
        self.z_spec_type = other.z_spec_type;
        self.r1 = other.r1;
        self.x1 = other.x1;
        self.r2 = other.r2;
        self.x2 = other.x2;
        self.r0 = other.r0;
        self.x0 = other.x0;
        self.x1r1 = other.x1r1;
        self.x0r0 = other.x0r0;
        self.pu_z1 = other.pu_z1;
        self.pu_z0 = other.pu_z0;
        self.pu_z2 = other.pu_z2;
        self.z_base = other.z_base;
        self.bus2_defined = other.bus2_defined;
        self.z1_specified = other.z1_specified;
        self.z2_specified = other.z2_specified;
        self.z0_specified = other.z0_specified;
        self.pu_z0_specified = other.pu_z0_specified;
        self.pu_z1_specified = other.pu_z1_specified;
        self.pu_z2_specified = other.pu_z2_specified;
        self.is_quasi_ideal = other.is_quasi_ideal;
        self.pu_z_ideal = other.pu_z_ideal;
        self.yearly_shape = other.yearly_shape.clone();
        self.daily_shape = other.daily_shape.clone();
        self.duty_shape = other.duty_shape.clone();
        self.spectrum = other.spectrum.clone();
        // Pascal copies the resolved shape/spectrum pointers (generic MakeLike).
        self.spectrum_obj = other.spectrum_obj.clone();
        self.yearly_shape_obj = other.yearly_shape_obj.clone();
        self.daily_shape_obj = other.daily_shape_obj.clone();
        self.duty_shape_obj = other.duty_shape_obj.clone();
        self.yearly_shape_ref = other.yearly_shape_ref;
        self.daily_shape_ref = other.daily_shape_ref;
        self.duty_shape_ref = other.duty_shape_ref;
        self.shape_is_actual = other.shape_is_actual;
        self.cd.inj_current = vec![Complex64::ZERO; self.cd.yorder];
    }
}

impl DssObject for VSource {
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
            BASEKV => self.kv_base,
            PU => self.per_unit,
            ANGLE => self.angle,
            FREQUENCY => self.src_frequency,
            MVASC3 => self.mva_sc3,
            MVASC1 => self.mva_sc1,
            X1R1 => self.x1r1,
            X0R0 => self.x0r0,
            ISC3 => self.isc3,
            ISC1 => self.isc1,
            R1 => self.r1,
            X1 => self.x1,
            R0 => self.r0,
            X0 => self.x0,
            BASE_MVA => self.base_mva,
            BASE_FREQ => self.cd.base_frequency,
            _ => unreachable!("Vsource has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        use super::prop::*;
        match idx {
            BASEKV => self.kv_base = value,
            PU => self.per_unit = value,
            ANGLE => self.angle = value,
            FREQUENCY => self.src_frequency = value,
            MVASC3 => self.mva_sc3 = value,
            MVASC1 => self.mva_sc1 = value,
            X1R1 => self.x1r1 = value,
            X0R0 => self.x0r0 = value,
            ISC3 => self.isc3 = value,
            ISC1 => self.isc1 = value,
            R1 => self.r1 = value,
            X1 => self.x1 = value,
            R0 => self.r0 = value,
            X0 => self.x0 = value,
            BASE_MVA => self.base_mva = value,
            BASE_FREQ => self.cd.base_frequency = value,
            _ => unreachable!("Vsource has no double property {idx}"),
        }
    }

    fn get_i32(&self, idx: usize) -> i32 {
        use super::prop::*;
        match idx {
            PHASES => self.cd.nphases as i32,
            SCAN_TYPE => self.scan_type,
            SEQUENCE => self.sequence_type,
            MODEL => self.is_quasi_ideal as i32,
            _ => unreachable!("Vsource has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        use super::prop::*;
        match idx {
            PHASES => self.cd.nphases = value.max(0) as usize,
            SCAN_TYPE => self.scan_type = value,
            SEQUENCE => self.sequence_type = value,
            MODEL => self.is_quasi_ideal = value != 0,
            _ => unreachable!("Vsource has no integer property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        match idx {
            super::prop::ENABLED => self.cd.enabled,
            _ => unreachable!("Vsource has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        match idx {
            super::prop::ENABLED => self.cd.set_enabled(value),
            _ => unreachable!("Vsource has no boolean property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        use super::prop::*;
        match idx {
            YEARLY => self.yearly_shape.clone(),
            DAILY => self.daily_shape.clone(),
            DUTY => self.duty_shape.clone(),
            SPECTRUM => self.spectrum.clone(),
            _ => unreachable!("Vsource has no string property {idx}"),
        }
    }
    fn set_string(&mut self, idx: usize, value: String) {
        use super::prop::*;
        match idx {
            YEARLY => self.yearly_shape = value,
            DAILY => self.daily_shape = value,
            DUTY => self.duty_shape = value,
            SPECTRUM => self.spectrum = value,
            _ => unreachable!("Vsource has no string property {idx}"),
        }
    }

    fn set_bus_name(&mut self, terminal: usize, value: &str) {
        self.cd.set_bus(terminal, value);
    }
    fn get_bus_name(&self, terminal: usize) -> String {
        self.cd.get_bus(terminal).to_string()
    }

    /// Resolve a shape reference (`yearly`/`daily`/`duty` → `LoadShape`): store
    /// the name (for the dump), the `ElemId`, and a snapshot clone the
    /// time-series `GetVterminalForSource` drives — same pattern as
    /// [`super::super::load::Load`].
    fn set_object_ref(&mut self, idx: usize, name: String, resolved: Option<ResolvedObj<'_>>) {
        use super::prop::*;
        let elem_ref = resolved.map(|o| o.id());
        let load_shape = || resolved.and_then(|o| o.cloned::<LoadShapeObj>());
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
            _ => unreachable!("Vsource has no resolved object-ref property {idx}"),
        }
    }

    fn get_complex(&self, idx: usize) -> (f64, f64) {
        use super::prop::*;
        match idx {
            Z1 => (self.r1, self.x1),
            Z0 => (self.r0, self.x0),
            Z2 => (self.r2, self.x2),
            PUZ1 => (self.pu_z1.re, self.pu_z1.im),
            PUZ0 => (self.pu_z0.re, self.pu_z0.im),
            PUZ2 => (self.pu_z2.re, self.pu_z2.im),
            PUZ_IDEAL => (self.pu_z_ideal.re, self.pu_z_ideal.im),
            _ => unreachable!("Vsource has no complex property {idx}"),
        }
    }
    fn set_complex(&mut self, idx: usize, re: f64, im: f64) {
        use super::prop::*;
        match idx {
            Z1 => {
                self.r1 = re;
                self.x1 = im;
            }
            Z0 => {
                self.r0 = re;
                self.x0 = im;
            }
            Z2 => {
                self.r2 = re;
                self.x2 = im;
            }
            PUZ1 => self.pu_z1 = Complex64::new(re, im),
            PUZ0 => self.pu_z0 = Complex64::new(re, im),
            PUZ2 => self.pu_z2 = Complex64::new(re, im),
            PUZ_IDEAL => self.pu_z_ideal = Complex64::new(re, im),
            _ => unreachable!("Vsource has no complex property {idx}"),
        }
    }

    /// Pascal `TVsourceObj.PropertySideEffects`.
    fn side_effects(&mut self, idx: usize, _prev_int: i32) {
        use super::prop::*;
        match idx {
            BUS1 => {
                // Default Bus2 to the zero node of Bus1 (grounded-Y).
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
            PHASES => {
                let n = self.cd.nphases;
                self.cd.set_nconds(n); // Force reallocation of terminal info
            }
            R1 => self.r2 = self.r1,
            X1 => self.x2 = self.x1,
            Z1 => {
                self.z1_specified = true;
                if !self.z2_specified {
                    self.r2 = self.r1;
                    self.x2 = self.x1;
                }
                if !self.z0_specified {
                    self.r0 = self.r1;
                    self.x0 = self.x1;
                }
            }
            Z0 => self.z0_specified = true,
            Z2 => self.z2_specified = true,
            PUZ1 => {
                self.pu_z1_specified = true;
                if !self.pu_z2_specified {
                    self.pu_z2 = self.pu_z1;
                }
                if !self.pu_z0_specified {
                    self.pu_z0 = self.pu_z1;
                }
            }
            PUZ0 => self.pu_z0_specified = true,
            PUZ2 => self.pu_z2_specified = true,
            // If the yearly shape is not yet defined, make it the daily one
            // (Pascal `YearlyShapeObj := DailyShapeObj`).
            DAILY if self.yearly_shape_obj.is_none() => {
                self.yearly_shape_obj = self.daily_shape_obj.clone();
                self.yearly_shape_ref = self.daily_shape_ref;
                self.yearly_shape = self.daily_shape.clone();
            }
            _ => {}
        }

        // Z spec-type switch + property-tracking resets.
        match idx {
            MVASC3 | MVASC1 => {
                self.z_spec_type = 1;
                for p in [ISC3, ISC1, R1, X1, R0, X0, Z1, Z0, Z2, PUZ1, PUZ0, PUZ2] {
                    self.cd.obj.clear_seq(p);
                }
            }
            ISC3 | ISC1 => {
                self.z_spec_type = 2;
                for p in [MVASC3, MVASC1, R1, X1, R0, X0, Z1, Z0, Z2, PUZ1, PUZ0, PUZ2] {
                    self.cd.obj.clear_seq(p);
                }
            }
            R1 | X1 | R0 | X0 => {
                self.z_spec_type = 3; // specified in ohms
                for p in [ISC3, ISC1, MVASC3, MVASC1] {
                    self.cd.obj.clear_seq(p);
                }
            }
            BUS2 => self.bus2_defined = true,
            Z1 | Z0 | Z2 | PUZ1 | PUZ0 | PUZ2 => {
                self.z_spec_type = 3;
                for p in [ISC3, ISC1, MVASC3, MVASC1] {
                    self.cd.obj.clear_seq(p);
                }
            }
            _ => {}
        }

        match idx {
            BASEKV | BASE_MVA => self.z_base = self.kv_base.powi(2) / self.base_mva,
            PUZ1 => {
                self.z1_specified = true;
                self.pu_z1_specified = true;
            }
            PUZ0 => self.pu_z0_specified = true,
            PUZ2 => self.pu_z2_specified = true,
            _ => {}
        }
    }

    /// Pascal `TVsource.EndEdit`.
    fn end_edit(&mut self, _sys: &crate::elements::traits::SysCtx) {
        self.recalc();
        self.cd.yprim_invalid = true;
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}
