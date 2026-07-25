//! The `impl DssObject` property surface, `PropertySideEffects`, the `SetBusX`
//! write function, and `MakeLike` for [`GicTransformer`].

use super::{GicTransformer, SPEC_AUTO};
use crate::elements::general::xy_curve::XyCurveObj;
use crate::elements::traits::{CktElement, ElemRef};
use crate::obj::base::{DssObjData, DssObject};

impl GicTransformer {
    /// Pascal `TGICTransformerObj.MakeLike` (GICTransformer.pas:352).
    pub(crate) fn make_like(&mut self, other: &Self) {
        self.cd.make_like_base(&other.cd);
        if self.cd.nphases != other.cd.nphases {
            self.cd.nphases = other.cd.nphases;
            self.cd.nterms = other.cd.nterms;
            let n = other.cd.nphases;
            self.cd.set_nconds(n); // force reallocation of terminals and conductors
            self.cd.yorder = self.cd.nconds * self.cd.nterms;
            self.cd.yprim_invalid = true;
        }
        self.cd.base_frequency = other.cd.base_frequency;
        self.g1 = other.g1;
        self.g2 = other.g2;
        self.spec_type = other.spec_type;
        self.mva_rating = other.mva_rating;
        self.var_curve_name = other.var_curve_name.clone();
        self.var_curve = other.var_curve.clone();
        self.kv1 = other.kv1;
        self.kv2 = other.kv2;
        self.pct_r1 = other.pct_r1;
        self.pct_r2 = other.pct_r2;
        self.pct_r_specified = other.pct_r_specified;
        self.z_base1 = other.z_base1;
        self.z_base2 = other.z_base2;
        self.k_factor = other.k_factor;
        self.k_specified = other.k_specified;
        // TPDElement.MakeLike copies the rating fields.
        self.norm_amps = other.norm_amps;
        self.emerg_amps = other.emerg_amps;
        self.fault_rate = other.fault_rate;
        self.pct_perm = other.pct_perm;
        self.hrs_to_repair = other.hrs_to_repair;
    }
}

impl DssObject for GicTransformer {
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
            // R1/R2 alias the conductances through the InverseValue flag (the
            // engine returns 1/G on the way out).
            R1 => self.g1,
            R2 => self.g2,
            KVLL1 => self.kv1,
            KVLL2 => self.kv2,
            MVA => self.mva_rating,
            PCT_R1 => self.pct_r1,
            PCT_R2 => self.pct_r2,
            K => self.k_factor,
            NORMAMPS => self.norm_amps,
            EMERGAMPS => self.emerg_amps,
            FAULTRATE => self.fault_rate,
            PCTPERM => self.pct_perm,
            REPAIR => self.hrs_to_repair,
            BASE_FREQ => self.cd.base_frequency,
            _ => unreachable!("GICTransformer has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        use super::prop::*;
        match idx {
            R1 => self.g1 = value, // engine already inverted (G = 1/R)
            R2 => self.g2 = value,
            KVLL1 => self.kv1 = value,
            KVLL2 => self.kv2 = value,
            MVA => self.mva_rating = value,
            PCT_R1 => self.pct_r1 = value,
            PCT_R2 => self.pct_r2 = value,
            K => self.k_factor = value,
            NORMAMPS => self.norm_amps = value,
            EMERGAMPS => self.emerg_amps = value,
            FAULTRATE => self.fault_rate = value,
            PCTPERM => self.pct_perm = value,
            REPAIR => self.hrs_to_repair = value,
            BASE_FREQ => self.cd.base_frequency = value,
            _ => unreachable!("GICTransformer has no double property {idx}"),
        }
    }

    fn get_i32(&self, idx: usize) -> i32 {
        use super::prop::*;
        match idx {
            PHASES => self.cd.nphases as i32,
            TYP => self.spec_type,
            _ => unreachable!("GICTransformer has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        use super::prop::*;
        match idx {
            PHASES => self.cd.nphases = value.max(0) as usize,
            TYP => self.spec_type = value,
            _ => unreachable!("GICTransformer has no integer property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        match idx {
            super::prop::ENABLED => self.cd.enabled,
            _ => unreachable!("GICTransformer has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        match idx {
            super::prop::ENABLED => self.cd.set_enabled(value),
            _ => unreachable!("GICTransformer has no boolean property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        match idx {
            super::prop::VARCURVE => self.var_curve_name.clone(),
            _ => unreachable!("GICTransformer has no string property {idx}"),
        }
    }

    /// Resolve the `VarCurve` XYcurve reference (snapshot-clone, like the
    /// Reactor RCurve/LCurve refs).
    fn set_object_ref(
        &mut self,
        idx: usize,
        name: String,
        resolved: Option<(ElemRef, &dyn DssObject)>,
    ) {
        match idx {
            super::prop::VARCURVE => {
                self.var_curve_name = name;
                self.var_curve =
                    resolved.and_then(|(_, o)| o.as_any().downcast_ref::<XyCurveObj>().cloned());
            }
            _ => unreachable!("GICTransformer has no resolved object-ref property {idx}"),
        }
    }

    /// Bus write. `BusX` (terminal 3) is Pascal `WriteByFunction` `SetBusX`
    /// (GICTransformer.pas:150): promote to 4 terminals before setting bus 3.
    fn set_bus_name(&mut self, terminal: usize, value: &str) {
        if terminal == super::prop::BUS_X && self.cd.nterms != 4 {
            // Must have 4 terminals to set this property.
            self.cd.set_nterms(4);
            let n = self.cd.nphases;
            self.cd.set_nconds(n); // force reallocation of terminals and conductors
        }
        self.cd.set_bus(terminal, value);
    }
    fn get_bus_name(&self, terminal: usize) -> String {
        self.cd.get_bus(terminal).to_string()
    }

    /// Pascal `TGICTransformerObj.PropertySideEffects` (GICTransformer.pas:251).
    fn side_effects(&mut self, idx: usize, prev_int: i32) {
        use super::prop::*;
        match idx {
            BUS_H => {
                // Default Bus2 to the zero node of Bus1 (Wye Grounded).
                let base = strip_extension(self.cd.get_bus(1));
                self.cd.set_bus(2, &format!("{base}.0.0.0"));
                self.is_shunt = true;
            }
            BUS_X => {
                // Default Bus4 to the zero node of Bus3 (Wye Grounded).
                let base = strip_extension(self.cd.get_bus(3));
                self.cd.set_bus(4, &format!("{base}.0.0.0"));
                self.is_shunt = true;
                if self.spec_type == SPEC_AUTO {
                    // Automatically make up the series-to-common connection.
                    let b3 = self.cd.get_bus(3).to_string();
                    self.cd.set_bus(2, &b3);
                }
            }
            PHASES => {
                if self.cd.nphases as i32 != prev_int {
                    let n = self.cd.nphases;
                    self.cd.set_nconds(n); // Force reallocation of terminal info
                    self.cd.signal_bus_name_redefined = true;
                }
            }
            TYP => {
                if self.spec_type == SPEC_AUTO {
                    if self.cd.nterms == 2 {
                        self.cd.set_nterms(4);
                        let n = self.cd.nphases;
                        self.cd.set_nconds(n);
                    }
                    let b3 = self.cd.get_bus(3).to_string();
                    self.cd.set_bus(2, &b3);
                }
            }
            R1 => {
                if self.g1 == 0.0 {
                    self.g1 = 10000.0; // Default to a low resistance
                }
                self.pct_r_specified = false;
            }
            R2 => {
                if self.g2 == 0.0 {
                    self.g2 = 10000.0;
                }
                self.pct_r_specified = false;
            }
            VARCURVE => {
                if self.var_curve.is_some() {
                    self.k_specified = false;
                }
            }
            PCT_R1 | PCT_R2 => self.pct_r_specified = true,
            K => self.k_specified = true,
            _ => {}
        }

        // YPrim invalidation on anything that changes impedance / terminal count.
        if matches!(idx, BUS_X | BUS_NX | PHASES | TYP | R1 | R2) {
            self.cd.yprim_invalid = true;
        }
    }

    /// Pascal base `TCktElementClass.EndEdit` → `RecalcElementData`
    /// (GICTransformer does not override EndEdit).
    fn end_edit(&mut self, _sys: &crate::elements::traits::SysCtx) {
        self.recalc();
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}

/// Pascal `Copy(S, 1, Pos('.', S) - 1)`: the bus name with any `.node…` suffix
/// removed.
fn strip_extension(s: &str) -> String {
    match s.find('.') {
        Some(p) => s[..p].to_string(),
        None => s.to_string(),
    }
}
