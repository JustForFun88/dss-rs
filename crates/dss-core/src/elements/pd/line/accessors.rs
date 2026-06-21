//! The `impl DssObject for Line` property surface: typed getters/setters, the
//! matrix get/set, property scaling/conditional flags, `set_object_ref`
//! (LineCode/LineGeometry resolution), `PropertySideEffects`, and `MakeLike`.

use crate::elements::general::line_code::LineCodeObj;
use crate::elements::general::line_geometry::LineGeometryObj;
use crate::elements::traits::{CktElement, ElemRef};
use crate::obj::base::{DssObjData, DssObject};
use crate::support::cmatrix::CMatrix;
use crate::support::line_units::{LineUnits, convert_line_units};

use super::Line;

impl DssObject for Line {
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
            LENGTH => self.len,
            R1 => self.r1,
            X1 => self.x1,
            R0 => self.r0,
            X0 => self.x0,
            C1 | B1 => self.c1,
            C0 | B0 => self.c0,
            RG => self.rg,
            XG => self.xg,
            RHO => self.rho,
            NORMAMPS => self.norm_amps,
            EMERGAMPS => self.emerg_amps,
            FAULTRATE => self.fault_rate,
            PCTPERM => self.pct_perm,
            REPAIR => self.hrs_to_repair,
            BASE_FREQ => self.cd.base_frequency,
            _ => unreachable!("Line has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        use super::prop::*;
        match idx {
            LENGTH => self.len = value,
            R1 => self.r1 = value,
            X1 => self.x1 = value,
            R0 => self.r0 = value,
            X0 => self.x0 = value,
            C1 | B1 => self.c1 = value,
            C0 | B0 => self.c0 = value,
            RG => self.rg = value,
            XG => self.xg = value,
            RHO => self.rho = value,
            NORMAMPS => self.norm_amps = value,
            EMERGAMPS => self.emerg_amps = value,
            FAULTRATE => self.fault_rate = value,
            PCTPERM => self.pct_perm = value,
            REPAIR => self.hrs_to_repair = value,
            BASE_FREQ => self.cd.base_frequency = value,
            _ => unreachable!("Line has no double property {idx}"),
        }
    }

    fn get_i32(&self, idx: usize) -> i32 {
        use super::prop::*;
        match idx {
            PHASES => self.cd.nphases as i32,
            UNITS => self.length_units.code(),
            EARTH_MODEL => self.earth_model,
            SEASONS => self.num_amp_ratings,
            LINE_TYPE => self.line_type,
            _ => unreachable!("Line has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        use super::prop::*;
        match idx {
            PHASES => self.cd.nphases = value.max(0) as usize,
            UNITS => self.length_units = LineUnits::from_code(value),
            EARTH_MODEL => self.earth_model = value,
            SEASONS => self.num_amp_ratings = value,
            LINE_TYPE => self.line_type = value,
            _ => unreachable!("Line has no integer property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        use super::prop::*;
        match idx {
            SWITCH => self.is_switch,
            ENABLED => self.cd.enabled,
            _ => unreachable!("Line has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        use super::prop::*;
        match idx {
            SWITCH => self.is_switch = value,
            ENABLED => self.cd.set_enabled(value),
            _ => unreachable!("Line has no boolean property {idx}"),
        }
    }

    fn get_f64_array(&self, idx: usize) -> Option<&[f64]> {
        match idx {
            super::prop::RATINGS => Some(&self.amp_ratings),
            _ => unreachable!("Line has no array property {idx}"),
        }
    }
    fn set_f64_array(&mut self, idx: usize, value: Vec<f64>) {
        match idx {
            super::prop::RATINGS => self.amp_ratings = value,
            _ => unreachable!("Line has no array property {idx}"),
        }
    }

    fn set_bus_name(&mut self, terminal: usize, value: &str) {
        self.cd.set_bus(terminal, value);
    }
    fn get_bus_name(&self, terminal: usize) -> String {
        self.cd.get_bus(terminal).to_string()
    }

    /// `linecode=`: store the resolved code's name + ElemRef and run
    /// `FetchLineCode` immediately (Pascal stores the pointer then
    /// `PropertySideEffects` calls `FetchLineCode`; here the resolved view is
    /// only available at parse time, so we fetch here).
    fn set_object_ref(
        &mut self,
        idx: usize,
        name: String,
        resolved: Option<(ElemRef, &dyn DssObject)>,
    ) {
        match idx {
            super::prop::LINECODE => {
                self.line_code_name = name;
                self.line_code_ref = resolved.map(|(r, _)| r);
                if let Some((_, obj)) = resolved
                    && let Some(code) = obj.as_any().downcast_ref::<LineCodeObj>()
                {
                    self.fetch_line_code(code);
                }
            }
            super::prop::GEOMETRY => {
                // Pascal stores the pointer then `PropertySideEffects` calls
                // `FetchGeometryCode`; the resolved view is only available here
                // (parse time), so fetch immediately (the `linecode` pattern).
                self.geometry_name = name;
                if let Some((_, obj)) = resolved
                    && let Some(geom) = obj.as_any().downcast_ref::<LineGeometryObj>()
                {
                    self.fetch_geometry_code(geom);
                }
            }
            _ => unreachable!("Line has no resolved object-ref property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        use super::prop::*;
        match idx {
            LINECODE => self.line_code_name.clone(),
            GEOMETRY => self.geometry_name.clone(),
            // Unported scalar ref renders as the oracle's empty value.
            SPACING => String::new(),
            // wires/cncables/tscables are array refs in Pascal; their empty
            // dump is `[]` (NOT_PORTED — they only need to round-trip empty).
            WIRES | CNCABLES | TSCABLES => "[]".to_string(),
            _ => unreachable!("Line has no string property {idx}"),
        }
    }

    fn set_matrix_part(&mut self, idx: usize, values: &[f64], order: usize, real: bool) {
        use super::prop::*;
        let target = match idx {
            RMATRIX | XMATRIX => &mut self.z,
            CMATRIX => &mut self.yc,
            _ => unreachable!("Line has no matrix property {idx}"),
        };
        let m = match target {
            Some(m) if m.order() == order => m,
            _ => {
                *target = Some(CMatrix::new(order));
                target.as_mut().unwrap()
            }
        };
        for j in 0..order {
            for i in 0..order {
                let mut v = m.get(i, j);
                if real {
                    v.re = values[j * order + i];
                } else {
                    v.im = values[j * order + i];
                }
                m.set(i, j, v);
            }
        }
    }
    fn get_matrix_part(&self, idx: usize, real: bool) -> Option<(Vec<f64>, usize)> {
        use super::prop::*;
        let m = match idx {
            RMATRIX | XMATRIX => self.z.as_ref()?,
            CMATRIX => self.yc.as_ref()?,
            _ => return None,
        };
        let order = m.order();
        let mut out = Vec::with_capacity(order * order);
        for j in 0..order {
            for i in 0..order {
                out.push(if real { m.get(i, j).re } else { m.get(i, j).im });
            }
        }
        Some((out, order))
    }

    /// `GetZSeqScale`/`GetCSeqScale`/`GetZmatScale`/`GetYCScale`/`GetB1B0Scale`.
    fn prop_scale(&self, idx: usize, getter: bool) -> f64 {
        use super::prop::*;
        let two_pi = 2.0 * std::f64::consts::PI;
        match idx {
            R1 | X1 | R0 | X0 => {
                if getter {
                    self.units_convert
                } else {
                    1.0
                }
            }
            C1 | C0 => {
                if getter {
                    self.units_convert * 1.0e-9
                } else {
                    1.0e-9
                }
            }
            RMATRIX | XMATRIX => {
                if getter {
                    // Pascal `GetZmatScale`: a geometry (later: spacing) line
                    // stores the *total* `Z` (length folded in), so the
                    // per-unit-length getter divides by `Len`; the sym/matrix line
                    // stores per-unit-length and divides by `units_convert`.
                    // (Spacing joins this branch in WP7.1 step 3b.)
                    if self.geometry_obj.is_some() {
                        self.len
                    } else {
                        self.units_convert
                    }
                } else {
                    1.0
                }
            }
            CMATRIX => {
                let base = two_pi * self.cd.base_frequency * 1.0e-9;
                if getter {
                    // Pascal `GetYCScale`: total `Yc` on a geometry line, so the
                    // getter divides the base scale by `Len`; else `units_convert`.
                    let unit = if self.geometry_obj.is_some() {
                        self.len
                    } else {
                        self.units_convert
                    };
                    base * unit
                } else {
                    base
                }
            }
            B1 | B0 => {
                let base = 1.0 / (two_pi * self.cd.base_frequency) * 1.0e-6;
                if getter {
                    base * self.units_convert
                } else {
                    base
                }
            }
            _ => 1.0,
        }
    }

    /// Pascal `ConditionalValue` (`@SymComponentsModel`): the sym scalars are
    /// hidden (`----`) once a matrix model is in force.
    fn prop_conditional(&self, idx: usize) -> bool {
        use super::prop::*;
        match idx {
            R1 | X1 | R0 | X0 | C1 | C0 | B1 | B0 => self.sym_components_model,
            _ => true,
        }
    }

    /// Pascal `TLineObj.PropertySideEffects`.
    fn side_effects(&mut self, idx: usize, prev_int: i32) {
        use super::prop::*;
        match idx {
            C1 | C0 | CMATRIX | B1 | B0 => self.cap_specified = true,
            UNITS => {
                // Update the units conversion factor. With a LineCode in play
                // the factor is recomputed relative to the code's units;
                // otherwise it is adjusted relative to the previous units.
                if self.line_code_ref.is_some() {
                    self.units_convert =
                        convert_line_units(self.line_code_units, self.length_units);
                } else {
                    self.units_convert *=
                        convert_line_units(LineUnits::from_code(prev_int), self.length_units);
                }
                self.user_length_units = self.length_units;
                self.cd.yprim_invalid = true;
            }
            _ => {}
        }

        match idx {
            LENGTH | UNITS => {
                self.miles_this_line =
                    self.len * convert_line_units(self.length_units, LineUnits::Miles);
            }
            PHASES => {
                if self.cd.nphases as i32 != prev_int {
                    if self.geometry_obj.is_none() && self.sym_components_model {
                        let n = self.cd.nphases;
                        self.cd.set_nconds(n); // force reallocation of terminal info
                        // Note: Pascal reads ActiveCircuit.PositiveSequence
                        // here; at parse time we use the multiphase default
                        // (positive-sequence circuits revisit in CalcYPrim).
                        self.recalc(false);
                    } else {
                        // Ignore change of nphases if a matrix or geometry model
                        // is in force (Pascal also logs 18101; the message is a
                        // pre-existing matrix-path gap, deferred uniformly here).
                        self.cd.nphases = prev_int.max(0) as usize;
                    }
                }
            }
            R1 | X1 | R0 | X0 | C1 | C0 | B1 | B0 => {
                self.kill_line_code_specified();
                self.kill_geometry_specified();
                self.reset_length_units();
                self.sym_components_changed = true;
                self.sym_components_model = true;
            }
            RMATRIX | XMATRIX | CMATRIX => {
                self.kill_line_code_specified();
                self.sym_components_model = false;
                for p in [R1, X1, R0, X0, C1, C0, B1, B0] {
                    self.cd.obj.clear_seq(p);
                }
                self.reset_length_units();
                self.kill_geometry_specified();
            }
            SWITCH => {
                if self.is_switch {
                    self.sym_components_changed = true;
                    self.cd.yprim_invalid = true;
                    self.kill_line_code_specified();
                    self.kill_geometry_specified();
                    self.r1 = 1.0;
                    self.x1 = 1.0;
                    self.r0 = 1.0;
                    self.x0 = 1.0;
                    self.c1 = 1.1 * 1.0e-9;
                    self.c0 = 1.0e-9;
                    self.len = 0.001;
                    self.reset_length_units();
                    for p in [R1, X1, R0, X0, C1, C0] {
                        self.cd.obj.set_as_next_seq(p);
                    }
                    self.cd.obj.clear_seq(B1);
                    self.cd.obj.clear_seq(B0);
                    self.cd.obj.set_as_next_seq(LENGTH);
                    self.cd.obj.set_as_next_seq(UNITS);
                }
            }
            XG | RHO => {
                self.kxg = self.xg / (658.5 * (self.rho / self.cd.base_frequency).sqrt()).ln();
            }
            SEASONS => {
                self.amp_ratings
                    .resize(self.num_amp_ratings.max(0) as usize, 0.0);
            }
            _ => {}
        }

        // Pascal (Line.pas:772): a `rho=` while a geometry is attached pushes the
        // earth resistivity into the geometry and invalidates YPrim to force the
        // rebuild. *Without* a geometry, `rho=` only updates `Kxg` (above) — Pascal
        // does NOT invalidate YPrim there, so neither do we (hence RHO is absent
        // from the unconditional list below). `FZFrequency` is deliberately *not*
        // reset — left to the next frequency change.
        if idx == RHO {
            let rho = self.rho;
            if let Some(g) = self.geometry_obj.as_mut() {
                g.set_rho_earth(rho);
                self.cd.yprim_invalid = true;
            }
        }

        // Yprim invalidation on anything that changes impedance values.
        if matches!(
            idx,
            LINECODE
                | LENGTH
                | PHASES
                | R1
                | X1
                | R0
                | X0
                | C1
                | C0
                | RMATRIX
                | XMATRIX
                | CMATRIX
                | RG
                | XG
        ) {
            self.cd.yprim_invalid = true;
        }
    }

    /// Pascal `TLine.EndEdit`: Line does *not* call RecalcElementData here.
    fn end_edit(&mut self) {}

    /// Pascal `TLineObj.MakeLike`.
    fn make_like(&mut self, other: &dyn DssObject) {
        let Some(other) = other.as_any().downcast_ref::<Line>() else {
            return;
        };
        self.cd.make_like_base(&other.cd);
        if self.cd.nphases != other.cd.nphases {
            self.cd.nphases = other.cd.nphases;
            let n = other.cd.nphases;
            self.cd.set_nconds(n);
            self.cd.yprim_invalid = true;
        }
        self.z = other.z.clone();
        self.yc = other.yc.clone();
        self.r1 = other.r1;
        self.x1 = other.x1;
        self.r0 = other.r0;
        self.x0 = other.x0;
        self.c1 = other.c1;
        self.c0 = other.c0;
        self.len = other.len;
        self.length_units = other.length_units;
        self.user_length_units = other.user_length_units;
        self.line_code_units = other.line_code_units;
        self.units_convert = other.units_convert;
        self.line_code_ref = other.line_code_ref;
        self.line_code_name = other.line_code_name.clone();
        self.is_switch = other.is_switch;
        self.sym_components_model = other.sym_components_model;
        self.sym_components_changed = other.sym_components_changed;
        self.cap_specified = other.cap_specified;
        self.rg = other.rg;
        self.xg = other.xg;
        self.kxg = other.kxg;
        self.rho = other.rho;
        self.earth_model = other.earth_model;
        self.line_type = other.line_type;
        self.geometry_obj = other.geometry_obj.clone();
        self.geometry_name = other.geometry_name.clone();
        self.fz_frequency = other.fz_frequency;
        self.norm_amps = other.norm_amps;
        self.emerg_amps = other.emerg_amps;
        self.fault_rate = other.fault_rate;
        self.pct_perm = other.pct_perm;
        self.hrs_to_repair = other.hrs_to_repair;
        self.num_amp_ratings = other.num_amp_ratings;
        self.amp_ratings = other.amp_ratings.clone();
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}
