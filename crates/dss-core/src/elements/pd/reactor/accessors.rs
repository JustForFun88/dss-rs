//! The `impl DssObject` property surface, side effects, and `MakeLike`.

use num_complex::Complex64;

use super::Reactor;
use crate::elements::general::xy_curve::XyCurveObj;
use crate::elements::traits::CktElement;
use crate::obj::arena::ResolvedObj;
use crate::obj::base::{DssObjData, DssObject};

impl Reactor {
    /// Pascal `TReactorObj.Z` (the series impedance complex). Read-only accessor
    /// for the CIM export (`SeriesCompensator.r`/`.x`/`.r0`/`.x0`, GAPS_PLAN
    /// WPG.18 Stage D).
    pub fn z(&self) -> Complex64 {
        self.z
    }

    /// Pascal `TDSSCktElement.NormAmps`. Read-only accessor for the CIM export
    /// (`WriteTerminals` operational limits).
    pub fn norm_amps(&self) -> f64 {
        self.norm_amps
    }

    /// Pascal `TDSSCktElement.EmergAmps`. Read-only accessor for the CIM export.
    pub fn emerg_amps(&self) -> f64 {
        self.emerg_amps
    }
}

impl Reactor {
    /// Pascal `TReactorObj.MakeLike`.
    pub(crate) fn make_like(&mut self, other: &Self) {
        self.cd.make_like_base(&other.cd);
        if self.cd.nphases != other.cd.nphases {
            self.cd.nphases = other.cd.nphases;
            let n = other.cd.nphases;
            self.cd.set_nconds(n); // force reallocation of terminals/conductors
            self.cd.yorder = self.cd.nconds * self.cd.nterms;
            self.cd.yprim_invalid = true;
        }

        self.rp = other.rp;
        self.rp_specified = other.rp_specified;
        self.is_parallel = other.is_parallel;
        self.kvarrating = other.kvarrating;
        self.kvrating = other.kvrating;
        self.connection = other.connection;
        self.spec_type = other.spec_type;
        self.z = other.z;
        self.z1 = other.z1;
        self.z2 = other.z2;
        self.z0 = other.z0;
        self.z2_specified = other.z2_specified;
        self.z0_specified = other.z0_specified;
        self.rmatrix = other.rmatrix.clone();
        self.xmatrix = other.xmatrix.clone();
        self.r_curve_name = other.r_curve_name.clone();
        self.r_curve = other.r_curve.clone();
        self.l_curve_name = other.l_curve_name.clone();
        self.l_curve = other.l_curve.clone();

        // TPDElement.MakeLike copies the rating fields.
        self.norm_amps = other.norm_amps;
        self.emerg_amps = other.emerg_amps;
        self.fault_rate = other.fault_rate;
        self.pct_perm = other.pct_perm;
        self.hrs_to_repair = other.hrs_to_repair;
    }
}

impl DssObject for Reactor {
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
            KVAR => self.kvarrating,
            KV => self.kvrating,
            R => self.z.re,
            X => self.z.im,
            RP => self.rp,
            LMH => self.l,
            NORMAMPS => self.norm_amps,
            EMERGAMPS => self.emerg_amps,
            FAULTRATE => self.fault_rate,
            PCTPERM => self.pct_perm,
            REPAIR => self.hrs_to_repair,
            BASE_FREQ => self.cd.base_frequency,
            _ => unreachable!("Reactor has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        use super::prop::*;
        match idx {
            KVAR => self.kvarrating = value,
            KV => self.kvrating = value,
            R => self.z.re = value,
            X => self.z.im = value,
            RP => self.rp = value,
            LMH => self.l = value,
            NORMAMPS => self.norm_amps = value,
            EMERGAMPS => self.emerg_amps = value,
            FAULTRATE => self.fault_rate = value,
            PCTPERM => self.pct_perm = value,
            REPAIR => self.hrs_to_repair = value,
            BASE_FREQ => self.cd.base_frequency = value,
            _ => unreachable!("Reactor has no double property {idx}"),
        }
    }

    fn get_i32(&self, idx: usize) -> i32 {
        use super::prop::*;
        match idx {
            PHASES => self.cd.nphases as i32,
            CONN => self.connection,
            _ => unreachable!("Reactor has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        use super::prop::*;
        match idx {
            PHASES => self.cd.nphases = value.max(0) as usize,
            CONN => self.connection = value,
            _ => unreachable!("Reactor has no integer property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        use super::prop::*;
        match idx {
            PARALLEL => self.is_parallel,
            ENABLED => self.cd.enabled,
            _ => unreachable!("Reactor has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        use super::prop::*;
        match idx {
            PARALLEL => self.is_parallel = value,
            ENABLED => self.cd.set_enabled(value),
            _ => unreachable!("Reactor has no boolean property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        use super::prop::*;
        match idx {
            RCURVE => self.r_curve_name.clone(),
            LCURVE => self.l_curve_name.clone(),
            _ => unreachable!("Reactor has no string property {idx}"),
        }
    }

    fn get_complex(&self, idx: usize) -> (f64, f64) {
        use super::prop::*;
        let c = match idx {
            Z1 => self.z1,
            Z2 => self.z2,
            Z0 => self.z0,
            Z => self.z,
            _ => unreachable!("Reactor has no complex property {idx}"),
        };
        (c.re, c.im)
    }
    fn set_complex(&mut self, idx: usize, re: f64, im: f64) {
        use super::prop::*;
        let c = Complex64::new(re, im);
        match idx {
            Z1 => self.z1 = c,
            Z2 => self.z2 = c,
            Z0 => self.z0 = c,
            Z => self.z = c,
            _ => unreachable!("Reactor has no complex property {idx}"),
        }
    }

    fn get_f64_array(&self, idx: usize) -> Option<&[f64]> {
        use super::prop::*;
        match idx {
            RMATRIX => self.rmatrix.as_deref(),
            XMATRIX => self.xmatrix.as_deref(),
            _ => unreachable!("Reactor has no double-array property {idx}"),
        }
    }
    fn set_f64_array(&mut self, idx: usize, value: Vec<f64>) {
        use super::prop::*;
        match idx {
            RMATRIX => self.rmatrix = Some(value),
            XMATRIX => self.xmatrix = Some(value),
            _ => unreachable!("Reactor has no double-array property {idx}"),
        }
    }

    fn set_bus_name(&mut self, terminal: usize, value: &str) {
        self.cd.set_bus(terminal, value);
    }
    fn get_bus_name(&self, terminal: usize) -> String {
        self.cd.get_bus(terminal).to_string()
    }

    /// Resolve the `RCurve`/`LCurve` XYcurve references (snapshot-clone, like
    /// the PVSystem/VCCS curve refs).
    fn set_object_ref(&mut self, idx: usize, name: String, resolved: Option<ResolvedObj<'_>>) {
        use super::prop::*;
        let xy_curve = || resolved.and_then(|o| o.cloned::<XyCurveObj>());
        match idx {
            RCURVE => {
                self.r_curve_name = name;
                self.r_curve = xy_curve();
            }
            LCURVE => {
                self.l_curve_name = name;
                self.l_curve = xy_curve();
            }
            _ => unreachable!("Reactor has no resolved object-ref property {idx}"),
        }
    }

    /// Pascal `TReactorObj.PropertySideEffects`.
    fn side_effects(&mut self, idx: usize, prev_int: i32) {
        use super::prop::*;
        match idx {
            BUS1 => {
                // Default Bus2 to the grounded-zero node of Bus1 (wye grounded) if
                // Bus2 has not been explicitly defined.
                if !self.bus2_defined && self.cd.nterms > 1 {
                    let s = self.cd.get_bus(1).to_string();
                    let base = match s.find('.') {
                        Some(p) => s[..p].to_string(),
                        None => s,
                    };
                    let mut s2 = base;
                    for _ in 0..self.cd.nphases {
                        s2.push_str(".0");
                    }
                    self.cd.set_bus(2, &s2);
                    self.is_shunt = true;
                }
                self.cd.obj.clear_seq(BUS2); // reset for the save function
            }
            BUS2 => {
                if !strip_extension(self.cd.get_bus(1))
                    .eq_ignore_ascii_case(&strip_extension(self.cd.get_bus(2)))
                {
                    self.is_shunt = false;
                    self.bus2_defined = true;
                }
            }
            PHASES => {
                if self.cd.nphases as i32 != prev_int {
                    let nc =
                        if self.connection == 1 && (self.cd.nphases == 1 || self.cd.nphases == 2) {
                            self.cd.nphases + 1
                        } else {
                            self.cd.nphases
                        };
                    self.cd.set_nconds(nc);
                    self.cd.yorder = self.cd.nterms * self.cd.nconds;
                } else if self.connection == 1 && self.cd.nconds != self.cd.nphases + 1 {
                    self.cd.set_nconds(self.cd.nphases + 1);
                    self.cd.yorder = self.cd.nterms * self.cd.nconds;
                }
            }
            KVAR => self.spec_type = 1,
            CONN => match self.connection {
                1 => {
                    // Delta: force one terminal.
                    self.cd.set_nterms(1);
                    let nc = if self.cd.nphases == 1 || self.cd.nphases == 2 {
                        self.cd.nphases + 1
                    } else {
                        self.cd.nphases
                    };
                    self.cd.set_nconds(nc);
                }
                _ => {
                    // Wye.
                    if self.cd.nterms != 2 {
                        self.cd.set_nterms(2);
                    }
                    let np = self.cd.nphases;
                    self.cd.set_nconds(np);
                }
            },
            RMATRIX | XMATRIX => self.spec_type = 3,
            X => self.spec_type = 2,
            RP => self.rp_specified = true,
            Z1 => {
                self.spec_type = 4; // have to set Z1 to get this mode
                if !self.z2_specified {
                    self.z2 = self.z1;
                }
                if !self.z0_specified {
                    self.z0 = self.z1;
                }
            }
            Z2 => self.z2_specified = true,
            Z0 => self.z0_specified = true,
            Z => self.spec_type = 2,
            LMH => {
                self.spec_type = 2;
                self.z.im = self.l * (2.0 * std::f64::consts::PI) * self.cd.base_frequency;
            }
            NORMAMPS => self.norm_amps_specified = true,
            EMERGAMPS => self.emerg_amps_specified = true,
            _ => {}
        }

        // YPrim invalidation on anything that changes the impedance values.
        if matches!(
            idx,
            PHASES
                | KVAR
                | KV
                | CONN
                | RMATRIX
                | XMATRIX
                | PARALLEL
                | R
                | X
                | RP
                | Z1
                | Z2
                | Z0
                | Z
                | LMH
        ) {
            self.cd.yprim_invalid = true;
        }
    }

    /// Pascal base `EndEdit` → `RecalcElementData` (Reactor does not override).
    fn end_edit(&mut self, _sys: &crate::elements::traits::SysCtx) {
        self.recalc();
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}

/// Pascal `StripExtension`: the bus name with its `.node.node…` suffix removed.
fn strip_extension(s: &str) -> String {
    match s.find('.') {
        Some(p) => s[..p].to_string(),
        None => s.to_string(),
    }
}
