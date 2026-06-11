//! `LineCode` — catalog of line impedances referenced by `Line.linecode`.
//! Port of Pascal `General/LineCode.pas` (`TLineCodeObj`). A `DSS_OBJECT`
//! class (no terminals, no YPrim): it only holds per-unit-length `Z`/`Yc`
//! matrices and the sym-component scalars that produce them, which `TLineObj`
//! copies in `FetchLineCode` (Phase 4 WP4.2).
//!
//! Unlike `TLineObj`, a LineCode does **no** units conversion on its own
//! getters — values are stored in the code's declared `Units`, and the
//! relative conversion happens later when a Line fetches the code. Its
//! `CalcMatricesFromZ1Z0` also has **no** 1-phase / positive-sequence special
//! case (a 1-phase code still mixes in the default R0/X0/C0).

use num_complex::Complex64;

use crate::obj::base::{DssObjData, DssObject};
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};
use crate::support::cmatrix::CMatrix;

/// 1-based property ordinals (Pascal `TLineCodeProp`).
pub mod prop {
    pub const NPHASES: usize = 1;
    pub const R1: usize = 2;
    pub const X1: usize = 3;
    pub const R0: usize = 4;
    pub const X0: usize = 5;
    pub const C1: usize = 6;
    pub const C0: usize = 7;
    pub const UNITS: usize = 8;
    pub const RMATRIX: usize = 9;
    pub const XMATRIX: usize = 10;
    pub const CMATRIX: usize = 11;
    pub const BASE_FREQ: usize = 12;
    pub const NORMAMPS: usize = 13;
    pub const EMERGAMPS: usize = 14;
    pub const FAULTRATE: usize = 15;
    pub const PCTPERM: usize = 16;
    pub const REPAIR: usize = 17;
    pub const KRON: usize = 18;
    pub const RG: usize = 19;
    pub const XG: usize = 20;
    pub const RHO: usize = 21;
    pub const NEUTRAL: usize = 22;
    pub const B1: usize = 23;
    pub const B0: usize = 24;
    pub const SEASONS: usize = 25;
    pub const RATINGS: usize = 26;
    pub const LINE_TYPE: usize = 27;
    pub const NUM_PROPS: usize = 28; // incl. Like
}

/// `TLineCode.DefineProperties`.
pub fn class_props(enums: &EnumRegistry) -> ClassProps {
    use prop::*;
    // The sym-component scalars are only shown while the sym model is active
    // (`PropertyOffset3 = @SymComponentsModel`, `ConditionalValue`).
    let conditional = PropFlags::CONDITIONAL_VALUE;
    let defs = vec![
        PropDef::integer("NPhases").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::double("R1").flags(conditional),
        PropDef::double("X1").flags(conditional),
        PropDef::double("R0").flags(conditional),
        PropDef::double("X0").flags(conditional),
        PropDef::double("C1").scale(1.0e-9).flags(conditional),
        PropDef::double("C0").scale(1.0e-9).flags(conditional),
        PropDef::mapped_string_enum("Units", enums.units),
        PropDef::sym_matrix_real("RMatrix", NPHASES),
        PropDef::sym_matrix_imag("XMatrix", NPHASES),
        // CMatrix stores susceptance; GetYCScale converts to/from nF on dump.
        PropDef::sym_matrix_imag("CMatrix", NPHASES).flags(PropFlags::SCALED_BY_FUNCTION),
        PropDef::double("BaseFreq").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::double("NormAmps"),
        PropDef::double("EmergAmps"),
        PropDef::double("FaultRate"),
        PropDef::double("PctPerm"),
        PropDef::double("Repair"),
        // BooleanActionProperty: setting it `yes` runs DoKronReduction; the
        // getter always reads back `No` (it stores no state).
        PropDef::boolean("Kron"),
        PropDef::double("Rg"),
        PropDef::double("Xg"),
        PropDef::double("rho"),
        PropDef::integer("Neutral"),
        PropDef::double("B1")
            .flags(PropFlags::SCALED_BY_FUNCTION | PropFlags::REDUNDANT | conditional),
        PropDef::double("B0")
            .flags(PropFlags::SCALED_BY_FUNCTION | PropFlags::REDUNDANT | conditional),
        PropDef::integer("Seasons").flags(PropFlags::SUPPRESS_JSON),
        PropDef::double_array("Ratings", SEASONS),
        PropDef::mapped_string_enum("LineType", enums.line_type),
    ];
    debug_assert_eq!(defs.len(), NUM_PROPS - 1);
    ClassProps::new("LineCode", defs, true)
}

/// `TLineCodeObj`.
#[derive(Debug, Clone)]
pub struct LineCodeObj {
    data: DssObjData,
    /// Pascal `FNPhases` (matrix order).
    fnphases: i32,
    /// Pascal `FNeutralConductor` (1-based; 0 = none after Kron).
    fneutral_conductor: i32,
    num_amp_ratings: i32,
    sym_components_model: bool,
    /// Pascal `Flg.NeedsRecalc`: a matrix property was set, so `EndEdit` must
    /// reinvert `Zinv` from `Z`.
    needs_recalc: bool,
    /// Transient: the value last parsed into the `Kron` action property.
    kron_pending: bool,
    /// Base-frequency series impedance (ohms / unit length).
    z: Option<CMatrix>,
    zinv: Option<CMatrix>,
    /// Base-frequency shunt susceptance (S / unit length).
    yc: Option<CMatrix>,
    base_frequency: f64,
    r1: f64,
    x1: f64,
    r0: f64,
    x0: f64,
    c1: f64,
    c0: f64,
    norm_amps: f64,
    emerg_amps: f64,
    fault_rate: f64,
    pct_perm: f64,
    hrs_to_repair: f64,
    rg: f64,
    xg: f64,
    rho: f64,
    amp_ratings: Vec<f64>,
    fline_type: i32,
    units: i32,
}

const TWO_PI: f64 = std::f64::consts::TAU;

impl LineCodeObj {
    /// Pascal `TLineCodeObj.Create`.
    pub fn new(name: &str) -> Self {
        let mut obj = Self {
            data: DssObjData::new(name.to_lowercase(), prop::NUM_PROPS),
            fnphases: 3,
            fneutral_conductor: 3, // last conductor
            num_amp_ratings: 1,
            sym_components_model: true,
            needs_recalc: false,
            kron_pending: false,
            z: None,
            zinv: None,
            yc: None,
            base_frequency: 60.0, // ActiveCircuit.Fundamental
            r1: 0.0580,           // ohms per 1000 ft
            x1: 0.1206,
            r0: 0.1784,
            x0: 0.4047,
            c1: 3.4e-9, // nF per 1000 ft (stored in farads)
            c0: 1.6e-9,
            norm_amps: 400.0,
            emerg_amps: 600.0,
            fault_rate: 0.1,
            pct_perm: 20.0,
            // TODO(compat): Pascal `Create` sets `HrsToRepair := 3`, but the
            // oracle's `? linecode.x.repair` reads back 0 for a default code
            // (Line keeps 3). The field is deprecated/unused since 2014 — never
            // propagated to lines — so we default it to the value the getter
            // reports. The clean fix drops this dead field entirely.
            hrs_to_repair: 0.0,
            rg: 0.01805, // ohms per 1000'
            xg: 0.155081,
            rho: 100.0,
            amp_ratings: vec![400.0],
            fline_type: 1, // OH line
            units: 0,      // UNITS_NONE
        };
        for p in [prop::R1, prop::X1, prop::R0, prop::X0, prop::C1, prop::C0] {
            obj.data.set_as_next_seq(p);
        }
        obj.calc_matrices_from_z1z0();
        obj
    }

    fn full_name(&self) -> String {
        format!("LineCode.{}", self.data.name())
    }

    fn nphases_usize(&self) -> usize {
        self.fnphases.max(0) as usize
    }

    /// Pascal `CalcMatricesFromZ1Z0`. Note: no 1-phase special case — a
    /// 1-phase code still folds in R0/X0/C0.
    fn calc_matrices_from_z1z0(&mut self) {
        let n = self.nphases_usize();
        let mut z = CMatrix::new(n);
        let mut zinv = CMatrix::new(n);
        let mut yc = CMatrix::new(n);

        let one_third = 1.0 / 3.0; // extra precision in the next statements
        let ztemp = Complex64::new(self.r1, self.x1) * 2.0;
        let zs = (ztemp + Complex64::new(self.r0, self.x0)) * one_third;
        let zm = (Complex64::new(self.r0, self.x0) - Complex64::new(self.r1, self.x1)) * one_third;

        let yc1 = TWO_PI * self.base_frequency * self.c1;
        let yc0 = TWO_PI * self.base_frequency * self.c0;
        let ys = (Complex64::new(0.0, yc1) * 2.0 + Complex64::new(0.0, yc0)) * one_third;
        let ym = (Complex64::new(0.0, yc0) - Complex64::new(0.0, yc1)) * one_third;

        for i in 0..n {
            z.set(i, i, zs);
            yc.set(i, i, ys);
            for j in 0..i {
                z.set(i, j, zm);
                z.set(j, i, zm);
                yc.set(i, j, ym);
                yc.set(j, i, ym);
            }
        }
        zinv.copy_from(&z);
        let _ = zinv.invert();
        self.z = Some(z);
        self.zinv = Some(zinv);
        self.yc = Some(yc);
    }

    /// Pascal `Set_NumPhases`: realloc the phase-sensitive matrices when the
    /// order actually changes.
    fn set_num_phases(&mut self, value: i32) {
        if value > 0 && self.fnphases != value {
            self.fnphases = value;
            self.fneutral_conductor = self.fnphases;
            self.calc_matrices_from_z1z0();
        }
    }

    /// Pascal `DoKronReduction`: eliminate the neutral conductor from `Z`/`YC`.
    fn do_kron_reduction(&mut self) {
        if self.sym_components_model {
            return;
        }
        if self.fneutral_conductor == 0 {
            return; // do nothing
        }
        if self.fnphases <= 1 {
            self.data.push_error(format!(
                "Cannot perform Kron Reduction on a 1-phase LineCode: {}",
                self.full_name()
            ));
            return;
        }

        // Pascal `TcMatrix.Kron` takes a 1-based conductor index.
        let elim = (self.fneutral_conductor - 1).max(0) as usize;
        let new_z = self.z.as_ref().and_then(|z| z.kron(elim));
        // Vn = 0, not In: invert YC in place, Kron, invert back.
        if let Some(yc) = self.yc.as_mut() {
            let _ = yc.invert();
        }
        let new_yc = self.yc.as_ref().and_then(|yc| yc.kron(elim));

        let (Some(new_z), Some(mut new_yc)) = (new_z, new_yc) else {
            self.data.push_error(format!(
                "Kron Reduction failed: {}. Attempting to eliminate Neutral Conductor {}.",
                self.full_name(),
                self.fneutral_conductor
            ));
            return;
        };
        let _ = new_yc.invert(); // back to Y

        self.set_num_phases(new_z.order() as i32);

        self.z = Some(new_z);
        self.yc = Some(new_yc);
        self.fneutral_conductor = 0;

        // Reflect the reduction in the property set-order for `save`.
        for p in [prop::NPHASES, prop::RMATRIX, prop::XMATRIX, prop::CMATRIX] {
            self.data.set_as_next_seq(p);
        }
    }
}

impl DssObject for LineCodeObj {
    fn data(&self) -> &DssObjData {
        &self.data
    }
    fn data_mut(&mut self) -> &mut DssObjData {
        &mut self.data
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn get_f64(&self, idx: usize) -> f64 {
        use prop::*;
        match idx {
            R1 => self.r1,
            X1 => self.x1,
            R0 => self.r0,
            X0 => self.x0,
            C1 | B1 => self.c1,
            C0 | B0 => self.c0,
            BASE_FREQ => self.base_frequency,
            NORMAMPS => self.norm_amps,
            EMERGAMPS => self.emerg_amps,
            FAULTRATE => self.fault_rate,
            PCTPERM => self.pct_perm,
            REPAIR => self.hrs_to_repair,
            RG => self.rg,
            XG => self.xg,
            RHO => self.rho,
            _ => unreachable!("LineCode has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        use prop::*;
        match idx {
            R1 => self.r1 = value,
            X1 => self.x1 = value,
            R0 => self.r0 = value,
            X0 => self.x0 = value,
            C1 | B1 => self.c1 = value,
            C0 | B0 => self.c0 = value,
            BASE_FREQ => self.base_frequency = value,
            NORMAMPS => self.norm_amps = value,
            EMERGAMPS => self.emerg_amps = value,
            FAULTRATE => self.fault_rate = value,
            PCTPERM => self.pct_perm = value,
            REPAIR => self.hrs_to_repair = value,
            RG => self.rg = value,
            XG => self.xg = value,
            RHO => self.rho = value,
            _ => unreachable!("LineCode has no double property {idx}"),
        }
    }

    fn get_i32(&self, idx: usize) -> i32 {
        use prop::*;
        match idx {
            NPHASES => self.fnphases,
            UNITS => self.units,
            NEUTRAL => self.fneutral_conductor,
            SEASONS => self.num_amp_ratings,
            LINE_TYPE => self.fline_type,
            _ => unreachable!("LineCode has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        use prop::*;
        match idx {
            NPHASES => self.fnphases = value,
            UNITS => self.units = value,
            NEUTRAL => self.fneutral_conductor = value,
            SEASONS => self.num_amp_ratings = value,
            LINE_TYPE => self.fline_type = value,
            _ => unreachable!("LineCode has no integer property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        match idx {
            // The Kron action property never reports back as set.
            prop::KRON => false,
            _ => unreachable!("LineCode has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        match idx {
            prop::KRON => self.kron_pending = value,
            _ => unreachable!("LineCode has no boolean property {idx}"),
        }
    }

    fn get_f64_array(&self, idx: usize) -> Option<&[f64]> {
        match idx {
            prop::RATINGS => Some(&self.amp_ratings),
            _ => unreachable!("LineCode has no array property {idx}"),
        }
    }
    fn set_f64_array(&mut self, idx: usize, value: Vec<f64>) {
        match idx {
            prop::RATINGS => self.amp_ratings = value,
            _ => unreachable!("LineCode has no array property {idx}"),
        }
    }

    fn set_matrix_part(&mut self, idx: usize, values: &[f64], order: usize, real: bool) {
        use prop::*;
        let target = match idx {
            RMATRIX | XMATRIX => &mut self.z,
            CMATRIX => &mut self.yc,
            _ => unreachable!("LineCode has no matrix property {idx}"),
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
        use prop::*;
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

    /// Pascal `GetYCScale` (cmatrix) and `GetC1C0Scale` (B1/B0). Both ignore
    /// the getter/setter direction.
    fn prop_scale(&self, idx: usize, _getter: bool) -> f64 {
        match idx {
            prop::CMATRIX => TWO_PI * self.base_frequency * 1.0e-9,
            prop::B1 | prop::B0 => 1.0 / (TWO_PI * self.base_frequency) * 1.0e-6,
            _ => 1.0,
        }
    }

    fn prop_conditional(&self, idx: usize) -> bool {
        use prop::*;
        match idx {
            R1 | X1 | R0 | X0 | C1 | C0 | B1 | B0 => self.sym_components_model,
            _ => true,
        }
    }

    /// Pascal `TLineCodeObj.PropertySideEffects`.
    fn side_effects(&mut self, idx: usize, prev_int: i32) {
        use prop::*;

        if idx == NPHASES && self.fnphases != prev_int {
            self.fneutral_conductor = self.fnphases; // init to last conductor
            self.calc_matrices_from_z1z0(); // reallocs matrices
        }

        match idx {
            // TODO(compat): Pascal omits `C0` from this list (the source has a
            // "-- Missing?" comment), so setting only C0 neither forces the sym
            // model nor clears the matrix property tracking. Reproduced as-is.
            NPHASES | R1 | X1 | R0 | X0 | C1 | B1 | B0 => {
                self.sym_components_model = true;
                self.data.clear_seq(RMATRIX);
                self.data.clear_seq(XMATRIX);
                self.data.clear_seq(CMATRIX);
            }
            RMATRIX | XMATRIX | CMATRIX => {
                self.needs_recalc = true;
                self.sym_components_model = false;
                for p in [R1, X1, R0, X0, C1, C0, B1, B0] {
                    self.data.clear_seq(p);
                }
            }
            SEASONS => {
                self.amp_ratings
                    .resize(self.num_amp_ratings.max(0) as usize, 0.0);
            }
            KRON if self.kron_pending => {
                self.kron_pending = false;
                self.do_kron_reduction();
            }
            _ => {}
        }
    }

    /// Pascal `TLineCode.EndEdit`.
    fn end_edit(&mut self) {
        if self.sym_components_model {
            self.calc_matrices_from_z1z0();
        }
        if self.needs_recalc {
            self.needs_recalc = false;
            if let Some(z) = self.z.as_ref() {
                let mut zinv = z.clone();
                let _ = zinv.invert();
                self.zinv = Some(zinv);
            }
        }
    }

    /// Pascal `TLineCodeObj.MakeLike`.
    fn make_like(&mut self, other: &dyn DssObject) {
        self.data.copy_prp_sequence_from(other.data());
        let Some(o) = other.as_any().downcast_ref::<LineCodeObj>() else {
            return;
        };
        self.fnphases = o.fnphases;
        self.z = o.z.clone();
        self.zinv = o.zinv.clone();
        self.yc = o.yc.clone();
        self.base_frequency = o.base_frequency;
        self.r1 = o.r1;
        self.x1 = o.x1;
        self.r0 = o.r0;
        self.x0 = o.x0;
        self.c1 = o.c1;
        self.c0 = o.c0;
        self.rg = o.rg;
        self.xg = o.xg;
        self.rho = o.rho;
        self.fneutral_conductor = o.fneutral_conductor;
        self.norm_amps = o.norm_amps;
        self.emerg_amps = o.emerg_amps;
        self.fault_rate = o.fault_rate;
        self.pct_perm = o.pct_perm;
        self.hrs_to_repair = o.hrs_to_repair;
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::obj::props::PropEngine;
    use dss_parser::{Parser, ParserVars};

    /// Build a LineCode, apply edits, return (class, obj) for querying.
    fn edited(edits: &[(&str, &str)]) -> (ClassProps, LineCodeObj, Vec<String>) {
        let enums = EnumRegistry::new();
        let cls = class_props(&enums);
        let mut obj = LineCodeObj::new("lc");
        let mut parser = Parser::new();
        let vars = ParserVars::new();
        let mut errors = Vec::new();
        for (name, value) in edits {
            let idx = cls.property_index(name).expect("known property");
            let mut eng = PropEngine {
                parser: &mut parser,
                vars: &vars,
                enums: &enums,
                errors: &mut errors,
            };
            cls.edit_property(&mut obj, idx, value, &mut eng).unwrap();
        }
        obj.end_edit();
        errors.extend(obj.data_mut().take_errors());
        (cls, obj, errors)
    }

    fn get(cls: &ClassProps, obj: &LineCodeObj, name: &str) -> String {
        let enums = EnumRegistry::new();
        let idx = cls.property_index(name).unwrap();
        cls.get_value(obj, idx, &enums)
    }

    /// Parse a sym-matrix dump `[a |b c |...]`, assert the lower-triangle row
    /// structure matches `rows` and the numbers are close (the exact float
    /// rendering differs from the oracle but the props gate is tolerance-based;
    /// the bracket/space format is locked separately in `matrix_model_*`).
    fn assert_matrix(actual: &str, rows: &[&[f64]]) {
        assert!(
            actual.starts_with('[') && actual.ends_with(']'),
            "bad brackets: {actual}"
        );
        let inner = &actual[1..actual.len() - 1];
        let parsed: Vec<Vec<f64>> = inner
            .split('|')
            .map(|row| {
                row.split_whitespace()
                    .map(|t| t.parse::<f64>().unwrap())
                    .collect()
            })
            .collect();
        assert_eq!(parsed.len(), rows.len(), "row count: {actual}");
        for (p, e) in parsed.iter().zip(rows) {
            assert_eq!(p.len(), e.len(), "row width: {actual}");
            for (a, b) in p.iter().zip(e.iter()) {
                assert!(
                    (a - b).abs() <= 1e-9 + 1e-9 * b.abs(),
                    "{a} vs {b} in {actual}"
                );
            }
        }
    }

    #[test]
    fn default_sym_matrices_match_oracle() {
        // Oracle (dss-python 0.15.7) for a fresh `new linecode.x`.
        let (cls, obj, errs) = edited(&[]);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(get(&cls, &obj, "NPhases"), "3");
        assert_eq!(get(&cls, &obj, "Units"), "none");
        assert_eq!(get(&cls, &obj, "Neutral"), "3");
        assert_eq!(get(&cls, &obj, "Repair"), "0");
        assert_eq!(get(&cls, &obj, "Kron"), "No");
        // RMatrix: Zs.re = (2*0.058 + 0.1784)/3, Zm.re = (0.1784-0.058)/3.
        let zs = (2.0 * 0.058 + 0.1784) / 3.0;
        let zm = (0.1784 - 0.058) / 3.0;
        assert_matrix(
            &get(&cls, &obj, "RMatrix"),
            &[&[zs], &[zm, zs], &[zm, zm, zs]],
        );
        // CMatrix in nF: Ys = (2*3.4 + 1.6)/3 = 2.8, Ym = (1.6-3.4)/3 = -0.6.
        assert_matrix(
            &get(&cls, &obj, "CMatrix"),
            &[&[2.8], &[-0.6, 2.8], &[-0.6, -0.6, 2.8]],
        );
    }

    #[test]
    fn one_phase_has_no_positive_seq_special_case() {
        // Oracle: `r1=0.1 x1=0.2 units=mi` on a 1-phase code dumps
        // rmatrix=[0.126133333333333 ] because R0/X0 keep their defaults.
        let (cls, obj, errs) = edited(&[
            ("nphases", "1"),
            ("r1", "0.1"),
            ("x1", "0.2"),
            ("units", "mi"),
        ]);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(get(&cls, &obj, "units"), "mi");
        assert_eq!(get(&cls, &obj, "r1"), "0.1"); // no units scaling at code level
        assert_matrix(
            &get(&cls, &obj, "rmatrix"),
            &[&[(2.0 * 0.1 + 0.1784) / 3.0]],
        );
    }

    #[test]
    fn matrix_model_hides_sym_scalars() {
        let (cls, obj, errs) = edited(&[
            ("nphases", "2"),
            ("rmatrix", "0.1 | 0.05 0.1"),
            ("xmatrix", "0.2 | 0.07 0.2"),
            ("cmatrix", "3 | -1 3"),
        ]);
        assert!(errs.is_empty(), "{errs:?}");
        for p in ["R1", "X1", "R0", "X0", "C1", "C0", "B1", "B0"] {
            assert_eq!(get(&cls, &obj, p), "----", "property {p}");
        }
        // RMatrix/XMatrix have scale 1.0, so the dump is exact — this locks the
        // `[v |v v |...]` bracket/space format against the oracle.
        assert_eq!(get(&cls, &obj, "RMatrix"), "[0.1 |0.05 0.1 ]");
        assert_eq!(get(&cls, &obj, "XMatrix"), "[0.2 |0.07 0.2 ]");
        assert_matrix(&get(&cls, &obj, "CMatrix"), &[&[3.0], &[-1.0, 3.0]]);
        assert_eq!(get(&cls, &obj, "Neutral"), "2");
    }

    #[test]
    fn kron_reduction_eliminates_neutral() {
        // Oracle: 4-phase symmetric matrices, kron=y -> 3-phase result with
        // diag 0.0842/0.1596 and off-diag 0.0242/0.0496; neutral -> 0.
        let (cls, obj, errs) = edited(&[
            ("nphases", "4"),
            (
                "rmatrix",
                "0.1 | 0.04 0.1 | 0.04 0.04 0.1 | 0.04 0.04 0.04 0.1",
            ),
            (
                "xmatrix",
                "0.2 | 0.09 0.2 | 0.09 0.09 0.2 | 0.09 0.09 0.09 0.2",
            ),
            (
                "cmatrix",
                "2.8 | -0.6 2.8 | -0.6 -0.6 2.8 | -0.6 -0.6 -0.6 2.8",
            ),
            ("kron", "y"),
        ]);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(get(&cls, &obj, "NPhases"), "3");
        assert_eq!(get(&cls, &obj, "Neutral"), "0");
        assert_eq!(get(&cls, &obj, "Kron"), "No");
        assert_matrix(
            &get(&cls, &obj, "RMatrix"),
            &[&[0.0842], &[0.0242, 0.0842], &[0.0242, 0.0242, 0.0842]],
        );
        assert_matrix(
            &get(&cls, &obj, "XMatrix"),
            &[&[0.1596], &[0.0496, 0.1596], &[0.0496, 0.0496, 0.1596]],
        );
    }

    #[test]
    fn kron_on_one_phase_errors_and_is_noop() {
        let (cls, obj, errs) = edited(&[
            ("nphases", "1"),
            ("rmatrix", "0.1"),
            ("xmatrix", "0.2"),
            ("cmatrix", "3"),
            ("kron", "y"),
        ]);
        assert_eq!(errs.len(), 1);
        assert!(errs[0].contains("1-phase LineCode"), "{:?}", errs);
        assert_eq!(get(&cls, &obj, "NPhases"), "1"); // unchanged
    }

    #[test]
    fn make_like_copies_matrices() {
        let enums = EnumRegistry::new();
        let cls = class_props(&enums);
        let (_, base, _) = edited(&[("nphases", "2"), ("r1", "0.2"), ("x1", "0.4")]);
        let mut obj = LineCodeObj::new("derived");
        obj.make_like(&base);
        assert_eq!(get(&cls, &obj, "NPhases"), "2");
        assert_eq!(get(&cls, &obj, "R1"), "0.2");
        assert_eq!(get(&cls, &obj, "X1"), "0.4");
    }
}
