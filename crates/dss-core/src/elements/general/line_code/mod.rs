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

#[cfg(test)]
mod tests;

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
        PropDef::double("R1").flags(conditional | PropFlags::UNITS_OHM_PER_LENGTH),
        PropDef::double("X1").flags(conditional | PropFlags::UNITS_OHM_PER_LENGTH),
        PropDef::double("R0").flags(conditional | PropFlags::UNITS_OHM_PER_LENGTH),
        PropDef::double("X0").flags(conditional | PropFlags::UNITS_OHM_PER_LENGTH),
        PropDef::double("C1").scale(1.0e-9).flags(conditional),
        PropDef::double("C0").scale(1.0e-9).flags(conditional),
        PropDef::mapped_string_enum("Units", enums.units),
        PropDef::sym_matrix_real("RMatrix", NPHASES).flags(PropFlags::UNITS_OHM_PER_LENGTH),
        PropDef::sym_matrix_imag("XMatrix", NPHASES).flags(PropFlags::UNITS_OHM_PER_LENGTH),
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
        PropDef::double("Rg").flags(PropFlags::UNITS_OHM_PER_LENGTH),
        PropDef::double("Xg").flags(PropFlags::UNITS_OHM_PER_LENGTH),
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

    // Read accessors consumed by `TLineObj.FetchLineCode` (Phase 4 WP4.2).
    pub fn base_frequency(&self) -> f64 {
        self.base_frequency
    }
    pub fn sym_components_model(&self) -> bool {
        self.sym_components_model
    }
    pub fn r1(&self) -> f64 {
        self.r1
    }
    pub fn x1(&self) -> f64 {
        self.x1
    }
    pub fn r0(&self) -> f64 {
        self.r0
    }
    pub fn x0(&self) -> f64 {
        self.x0
    }
    pub fn c1(&self) -> f64 {
        self.c1
    }
    pub fn c0(&self) -> f64 {
        self.c0
    }
    pub fn rg(&self) -> f64 {
        self.rg
    }
    pub fn xg(&self) -> f64 {
        self.xg
    }
    pub fn rho(&self) -> f64 {
        self.rho
    }
    pub fn units(&self) -> i32 {
        self.units
    }
    pub fn norm_amps(&self) -> f64 {
        self.norm_amps
    }
    pub fn emerg_amps(&self) -> f64 {
        self.emerg_amps
    }
    pub fn num_amp_ratings(&self) -> i32 {
        self.num_amp_ratings
    }
    pub fn amp_ratings(&self) -> &[f64] {
        &self.amp_ratings
    }
    pub fn nphases(&self) -> i32 {
        self.fnphases
    }
    pub fn fline_type(&self) -> i32 {
        self.fline_type
    }
    pub fn z(&self) -> Option<&CMatrix> {
        self.z.as_ref()
    }
    pub fn yc(&self) -> Option<&CMatrix> {
        self.yc.as_ref()
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
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
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
