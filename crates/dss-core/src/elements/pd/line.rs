//! Port of `PDElements/Line.pas` — `TLineObj`. Impedance sources: symmetrical
//! components (R1/X1/R0/X0/C1/C0/B1/B0), the direct matrix specification
//! (rmatrix/xmatrix/cmatrix), the LineCode catalog (Phase 4), and the
//! `LineGeometry` Carson path (`geometry=`, WP7.1 step 3a). The `spacing=`/
//! `wires=`/`cncables=`/`tscables=` forms remain `NOT_PORTED` (step 3b).
//!
//! For the sym/matrix/linecode sources `Z`/`Yc` hold ohms (resp. susceptance)
//! **per unit length** at base frequency and `CalcYPrim` applies length, units
//! and frequency corrections. For the geometry source `Z`/`Yc` are already the
//! **total** matrices (the geometry's `Zmatrix[f, len, units]` folds length and
//! units in), so `CalcYPrim` inverts/embeds them directly.

use num_complex::Complex64;

use crate::elements::ckt::CktElementData;
use crate::elements::general::line_code::LineCodeObj;
use crate::elements::general::line_geometry::LineGeometryObj;
use crate::elements::traits::{CktElement, ElemRef, ReliabilityData, SysCtx};
use crate::obj::base::{DssObjData, DssObject};
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};
use crate::support::cmatrix::CMatrix;
use crate::support::line_units::{LineUnits, convert_line_units};
use crate::support::mathutil::SymComp;
use crate::util::EPSILON;

/// Pascal `CAP_EPSILON` (Line.pas): "5 kvar of capacitive reactance at
/// 345 kV to avoid open line problem", added to the series Yprim diagonal.
const CAP_EPSILON: Complex64 = Complex64::new(0.0, 4.2e-8);

/// 1-based property ordinals (Pascal `TLineProp` + class tails).
pub mod prop {
    pub const BUS1: usize = 1;
    pub const BUS2: usize = 2;
    pub const LINECODE: usize = 3;
    pub const LENGTH: usize = 4;
    pub const PHASES: usize = 5;
    pub const R1: usize = 6;
    pub const X1: usize = 7;
    pub const R0: usize = 8;
    pub const X0: usize = 9;
    pub const C1: usize = 10;
    pub const C0: usize = 11;
    pub const RMATRIX: usize = 12;
    pub const XMATRIX: usize = 13;
    pub const CMATRIX: usize = 14;
    pub const SWITCH: usize = 15;
    pub const RG: usize = 16;
    pub const XG: usize = 17;
    pub const RHO: usize = 18;
    pub const GEOMETRY: usize = 19;
    pub const UNITS: usize = 20;
    pub const SPACING: usize = 21;
    pub const WIRES: usize = 22;
    pub const EARTH_MODEL: usize = 23;
    pub const CNCABLES: usize = 24;
    pub const TSCABLES: usize = 25;
    pub const B1: usize = 26;
    pub const B0: usize = 27;
    pub const SEASONS: usize = 28;
    pub const RATINGS: usize = 29;
    pub const LINE_TYPE: usize = 30;
    // TPDClass tail:
    pub const NORMAMPS: usize = 31;
    pub const EMERGAMPS: usize = 32;
    pub const FAULTRATE: usize = 33;
    pub const PCTPERM: usize = 34;
    pub const REPAIR: usize = 35;
    // TCktElementClass tail:
    pub const BASE_FREQ: usize = 36;
    pub const ENABLED: usize = 37;
    pub const NUM_PROPS: usize = 38; // incl. Like
}

/// `TLine.DefineProperties`.
pub fn class_props(enums: &EnumRegistry) -> ClassProps {
    use prop::*;
    let defs = vec![
        PropDef::bus("bus1", 1),
        PropDef::bus("bus2", 2),
        PropDef::object_ref_class("LineCode", "LineCode"),
        PropDef::double("length"),
        PropDef::integer("phases").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        // The sym-component scalars are shown only while the sym model is
        // active (`PropertyOffset3 = @SymComponentsModel`, ConditionalValue).
        PropDef::double("r1").flags(
            PropFlags::SCALED_BY_FUNCTION
                | PropFlags::CONDITIONAL_VALUE
                | PropFlags::UNITS_OHM_PER_LENGTH,
        ),
        PropDef::double("x1").flags(
            PropFlags::SCALED_BY_FUNCTION
                | PropFlags::CONDITIONAL_VALUE
                | PropFlags::UNITS_OHM_PER_LENGTH,
        ),
        PropDef::double("r0").flags(
            PropFlags::SCALED_BY_FUNCTION
                | PropFlags::CONDITIONAL_VALUE
                | PropFlags::UNITS_OHM_PER_LENGTH,
        ),
        PropDef::double("x0").flags(
            PropFlags::SCALED_BY_FUNCTION
                | PropFlags::CONDITIONAL_VALUE
                | PropFlags::UNITS_OHM_PER_LENGTH,
        ),
        PropDef::double("C1").flags(PropFlags::SCALED_BY_FUNCTION | PropFlags::CONDITIONAL_VALUE),
        PropDef::double("C0").flags(PropFlags::SCALED_BY_FUNCTION | PropFlags::CONDITIONAL_VALUE),
        PropDef::sym_matrix_real("rmatrix", PHASES)
            .flags(PropFlags::SCALED_BY_FUNCTION | PropFlags::UNITS_OHM_PER_LENGTH),
        PropDef::sym_matrix_imag("xmatrix", PHASES)
            .flags(PropFlags::SCALED_BY_FUNCTION | PropFlags::UNITS_OHM_PER_LENGTH),
        PropDef::sym_matrix_imag("cmatrix", PHASES).flags(PropFlags::SCALED_BY_FUNCTION),
        PropDef::boolean("Switch"),
        PropDef::double("Rg").flags(PropFlags::UNITS_OHM_PER_LENGTH),
        PropDef::double("Xg").flags(PropFlags::UNITS_OHM_PER_LENGTH),
        PropDef::double("rho"),
        PropDef::object_ref_class("LineGeometry", "geometry"),
        PropDef::mapped_string_enum("units", enums.units),
        PropDef::object_ref("spacing").flags(PropFlags::NOT_PORTED),
        PropDef::object_ref("wires").flags(PropFlags::NOT_PORTED),
        PropDef::mapped_string_enum("EarthModel", enums.earth_model),
        PropDef::object_ref("cncables").flags(PropFlags::NOT_PORTED),
        PropDef::object_ref("tscables").flags(PropFlags::NOT_PORTED),
        PropDef::double("B1").flags(
            PropFlags::SCALED_BY_FUNCTION | PropFlags::REDUNDANT | PropFlags::CONDITIONAL_VALUE,
        ),
        PropDef::double("B0").flags(
            PropFlags::SCALED_BY_FUNCTION | PropFlags::REDUNDANT | PropFlags::CONDITIONAL_VALUE,
        ),
        PropDef::integer("Seasons"),
        PropDef::double_array("Ratings", SEASONS),
        PropDef::mapped_string_enum("LineType", enums.line_type),
        // TPDClass tail:
        PropDef::double("normamps"),
        PropDef::double("emergamps"),
        PropDef::double("faultrate"),
        PropDef::double("pctperm"),
        PropDef::double("repair"),
        // TCktElementClass tail:
        PropDef::double("basefreq").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::enabled("enabled"),
    ];
    debug_assert_eq!(defs.len(), NUM_PROPS - 1);
    ClassProps::new("Line", defs, true)
}

/// `TLineObj`.
#[derive(Debug, Clone)]
pub struct Line {
    pub cd: CktElementData,

    /// Sequence parameters, ohms / farads per unit length.
    pub r1: f64,
    pub x1: f64,
    pub r0: f64,
    pub x0: f64,
    pub c1: f64,
    pub c0: f64,
    pub len: f64,
    pub length_units: LineUnits,
    pub user_length_units: LineUnits,
    /// `FLineCodeUnits`: the units the active LineCode declared, captured at
    /// `FetchLineCode`; drives the `units=` relative reconversion.
    pub line_code_units: LineUnits,
    /// `FUnitsConvert`.
    pub units_convert: f64,
    /// `LineCodeObj` reference (the resolved code's stable [`ElemRef`]) and its
    /// name for dumps; `None`/empty before any `linecode=`.
    pub line_code_ref: Option<ElemRef>,
    pub line_code_name: String,
    pub is_switch: bool,
    pub sym_components_model: bool,
    pub sym_components_changed: bool,
    pub cap_specified: bool,
    pub rg: f64,
    pub xg: f64,
    pub kxg: f64,
    pub rho: f64,
    pub earth_model: i32,
    pub line_type: i32,
    /// Pascal `LineGeometryObj` — the snapshot-cloned geometry a `geometry=`
    /// reference attaches (the WP4.2 `FetchLineCode` pattern). `Some` activates
    /// the Carson matrix path in [`Line::calc_yprim`], driving `Z`/`Yc` from the
    /// geometry instead of the sym/linecode data.
    pub geometry_obj: Option<LineGeometryObj>,
    /// The resolved geometry object's name (the `geometry=` dump value).
    pub geometry_name: String,
    /// Pascal `FZFrequency`: the frequency the geometry `Z`/`Yc` were last built
    /// for (`-1` = not yet computed), so [`Line::make_z_from_geometry`] rebuilds
    /// only on a frequency change (the geometry matrices fold in length + units).
    pub fz_frequency: f64,
    /// Per-unit-length series impedance at base frequency.
    pub z: Option<CMatrix>,
    /// Per-unit-length shunt susceptance at base frequency.
    pub yc: Option<CMatrix>,
    // PD-element common:
    pub norm_amps: f64,
    pub emerg_amps: f64,
    pub fault_rate: f64,
    pub pct_perm: f64,
    pub hrs_to_repair: f64,
    pub miles_this_line: f64,
    pub num_amp_ratings: i32,
    pub amp_ratings: Vec<f64>,
}

impl Line {
    /// Pascal `TLineObj.Create`.
    pub fn new(name: &str) -> Self {
        let mut cd = CktElementData::new(name, prop::NUM_PROPS);
        cd.nphases = 3;
        cd.nconds = 3;
        cd.set_nterms(2);

        let rho: f64 = 100.0;
        let xg: f64 = 0.155081;
        let base_freq = 60.0;
        let mut line = Self {
            cd,
            r1: 0.0580, // ohms per 1000 ft
            x1: 0.1206,
            r0: 0.1784,
            x0: 0.4047,
            c1: 3.4e-9, // nF per 1000 ft (stored in farads)
            c0: 1.6e-9,
            len: 1.0, // 1 kFt
            length_units: LineUnits::None,
            user_length_units: LineUnits::None,
            line_code_units: LineUnits::None,
            units_convert: 1.0,
            line_code_ref: None,
            line_code_name: String::new(),
            is_switch: false,
            sym_components_model: true,
            sym_components_changed: false,
            cap_specified: false,
            rg: 0.01805, // ohms per 1000 ft
            xg,
            kxg: xg / (658.5 * (rho / base_freq).sqrt()).ln(),
            rho,
            earth_model: 3, // DSS.DefaultEarthModel = DERI
            line_type: 1,   // OH line
            geometry_obj: None,
            geometry_name: String::new(),
            fz_frequency: -1.0,
            z: None,
            yc: None,
            norm_amps: 400.0,
            emerg_amps: 600.0,
            fault_rate: 0.1,
            pct_perm: 20.0,
            hrs_to_repair: 3.0,
            miles_this_line: 0.0,
            num_amp_ratings: 1,
            amp_ratings: vec![400.0],
        };
        for p in [prop::R1, prop::X1, prop::R0, prop::X0, prop::C1, prop::C0] {
            line.cd.obj.set_as_next_seq(p);
        }
        line.recalc(false);
        line
    }

    /// Pascal `TLineObj.RecalcElementData`: compute the per-unit-length
    /// `Z`/`Yc` from the symmetrical components. Only called when those have
    /// changed.
    pub fn recalc(&mut self, positive_sequence: bool) {
        let nphases = self.cd.nphases;
        let mut z = CMatrix::new(nphases);
        let mut yc = CMatrix::new(nphases);

        let ztemp = Complex64::new(self.r1, self.x1) * 2.0;
        // Handle special case for 1-phase line and/or pos-seq model:
        // zero sequence the same as positive sequence.
        if nphases == 1 || positive_sequence {
            self.r0 = self.r1;
            self.x0 = self.x1;
            self.c0 = self.c1;
        }

        let zs = (ztemp + Complex64::new(self.r0, self.x0)) / 3.0;
        let zm = (Complex64::new(self.r0, self.x0) - Complex64::new(self.r1, self.x1)) / 3.0;

        let two_pi = 2.0 * std::f64::consts::PI;
        let yc1 = two_pi * self.cd.base_frequency * self.c1;
        let yc0 = two_pi * self.cd.base_frequency * self.c0;

        let ys = (Complex64::new(0.0, yc1) * 2.0 + Complex64::new(0.0, yc0)) / 3.0;
        let ym = (Complex64::new(0.0, yc0) - Complex64::new(0.0, yc1)) / 3.0;

        for i in 0..nphases {
            z.set(i, i, zs);
            yc.set(i, i, ys);
            for j in 0..i {
                z.set(i, j, zm);
                z.set(j, i, zm);
                yc.set(i, j, ym);
                yc.set(j, i, ym);
            }
        }

        self.z = Some(z);
        self.yc = Some(yc);
        self.sym_components_changed = false;
        // values in ohms per unit length
    }

    /// Pascal `TLineObj.KillLineCodeSpecified`: drop the LineCode reference and
    /// clear its set-order mark, so a later sym/matrix override stops the dump
    /// from reporting the (now superseded) code.
    fn kill_line_code_specified(&mut self) {
        self.line_code_ref = None;
        self.line_code_name = String::new();
        self.cd.obj.clear_seq(prop::LINECODE);
    }

    /// Pascal `ResetLengthUnits`.
    fn reset_length_units(&mut self) {
        self.units_convert = 1.0;
        self.length_units = LineUnits::None;
        self.user_length_units = LineUnits::None;
    }

    /// Pascal `TLineObj.FetchLineCode`: copy the resolved LineCode's impedance
    /// data onto this line. Called from the `linecode=` property side effect
    /// (here, directly when the reference resolves). Faithful to the
    /// non-`DSS_EXTENSIONS_COMPAT` path: the copied properties' set-order marks
    /// are cleared so dumps reflect the code, not the line.
    fn fetch_line_code(&mut self, code: &LineCodeObj) {
        use prop::*;

        // Frequency compensation takes place in CalcYPrim.
        self.cd.base_frequency = code.base_frequency();

        // Copy impedances, but do not recalc here for the matrix model: the
        // symmetrical-component z's may not match what is in the matrix.
        if code.sym_components_model() {
            self.r1 = code.r1();
            self.x1 = code.x1();
            self.r0 = code.r0();
            self.x0 = code.x0();
            self.c1 = code.c1();
            self.c0 = code.c0();
            self.sym_components_model = true;
        } else {
            self.sym_components_model = false;
        }

        // Earth-return impedances used to compensate for frequency.
        self.rg = code.rg();
        self.xg = code.xg();
        self.rho = code.rho();
        self.kxg = self.xg / (658.5 * (self.rho / self.cd.base_frequency).sqrt()).ln();

        self.line_code_units = LineUnits::from_code(code.units());
        self.units_convert = convert_line_units(self.line_code_units, self.length_units);

        self.norm_amps = code.norm_amps();
        self.emerg_amps = code.emerg_amps();
        self.num_amp_ratings = code.num_amp_ratings();
        self.amp_ratings = code.amp_ratings().to_vec();

        // FaultRate/PctPerm/HrsToRepair deliberately NOT copied (Pascal
        // commented out 2014 — they vary section to section).

        // Zero the set-order marks of everything the code now supplies, so
        // `Save`/`?` reflect the linecode (non-NoPropertyTracking branch).
        for p in [
            GEOMETRY, SPACING, R1, X1, R0, X0, C1, C0, B1, B0, SEASONS, RATINGS, NORMAMPS,
            EMERGAMPS,
        ] {
            self.cd.obj.clear_seq(p);
        }

        if self.cd.nphases as i32 != code.nphases() {
            self.cd.nphases = code.nphases().max(0) as usize;
        }

        if !self.sym_components_model {
            // Copy matrices (Z, Yc) verbatim.
            self.z = code.z().cloned();
            self.yc = code.yc().cloned();
        } else {
            // Compute matrices from the copied sym components. Pascal reads
            // ActiveCircuit.PositiveSequence; at parse time we use the
            // multiphase default, exactly as the `phases=` side effect does.
            self.recalc(false);
        }

        // NConds := Fnphases; forces reallocation of terminal info + Yorder.
        let n = self.cd.nphases;
        self.cd.set_nconds(n);

        self.line_type = code.fline_type();
    }

    /// Pascal `TLineObj.KillGeometrySpecified`: drop the geometry reference and
    /// clear its set-order mark; reset `FZFrequency` so a later geometry/spacing
    /// rebuild is forced. A no-op when no geometry is attached.
    fn kill_geometry_specified(&mut self) {
        if self.geometry_obj.is_none() {
            return;
        }
        self.geometry_obj = None;
        self.geometry_name = String::new();
        self.cd.obj.clear_seq(prop::GEOMETRY);
        self.fz_frequency = -1.0;
    }

    /// Pascal `TLineObj.FetchGeometryCode`: adopt a resolved `LineGeometry` as
    /// the impedance source. Snapshot-clones it (the WP4.2 `FetchLineCode`
    /// pattern), copies the ratings/line-type, sizes the Line to the geometry's
    /// effective conductor count, and switches off the sym-component model.
    fn fetch_geometry_code(&mut self, geom: &LineGeometryObj) {
        use prop::*;

        self.kill_line_code_specified();
        // KillSpacingSpecified is the WP7.1 step-3b spacing path; no spacing can
        // be attached yet (the props are still NOT_PORTED).

        self.fz_frequency = -1.0; // Init to signify not computed

        // Own a private copy so the per-Line `rho`/cached matrices don't mutate
        // the shared catalog object (Pascal mutates the shared object — the
        // upstream "weird" TODO at Line.pas:776; cloning is the faithful Rust
        // equivalent under the snapshot-clone borrow model).
        let mut geom = geom.clone();

        // If `rho=` was set on this Line *before* `geometry=`, push it into the
        // geometry now (the symmetric after-case is handled in `side_effects`).
        if self.cd.obj.prp_specified(RHO) {
            geom.set_rho_earth(self.rho);
        }

        self.norm_amps = geom.norm_amps();
        self.emerg_amps = geom.emerg_amps();

        // Zero the set-order marks of everything the geometry now supplies, so
        // `Save`/`?` reflect the geometry (non-NoPropertyTracking branch).
        for p in [
            LINECODE, R1, X1, R0, X0, C1, C0, B1, B0, SEASONS, RATINGS, NORMAMPS, EMERGAMPS,
        ] {
            self.cd.obj.clear_seq(p);
        }

        // FNPhases := geometry.Nconds (reduce-aware); NConds := FNPhases forces
        // reallocation of terminal info + Yorder.
        self.cd.nphases = geom.nconds().max(0) as usize;
        let n = self.cd.nphases;
        self.cd.set_nconds(n);

        self.num_amp_ratings = geom.num_amp_ratings();
        self.amp_ratings = geom.amp_ratings().to_vec();
        self.line_type = geom.line_type();

        self.sym_components_model = false;
        self.sym_components_changed = false;

        self.geometry_obj = Some(geom);
        self.cd.yprim_invalid = true;
    }

    /// Pascal `TLineObj.FMakeZFromGeometry` (Line.pas:1929): build the total
    /// `Z`/`Yc` (length + units already folded in) from the attached geometry at
    /// frequency `f`, recomputing only when `f` differs from the last build. The
    /// geometry's `ActiveEarthModel` is this Line's `FEarthModel`. Returns the
    /// geometry's error (Pascal raises `ELineGeometryProblem` + `SolutionAbort`).
    fn make_z_from_geometry(&mut self, f: f64) -> Result<(), String> {
        if f == self.fz_frequency {
            return Ok(()); // already done for this frequency
        }
        let len = self.len;
        let units = self.length_units.code();
        let earth_model = self.earth_model;
        let geom = self
            .geometry_obj
            .as_mut()
            .expect("make_z_from_geometry called without a geometry");
        let z = geom.z_matrix(f, len, units, earth_model)?;
        let yc = geom.yc_matrix(f, len, units, earth_model)?;
        self.z = Some(z);
        self.yc = Some(yc);
        self.fz_frequency = f;
        Ok(())
    }
}

impl CktElement for Line {
    fn cd(&self) -> &CktElementData {
        &self.cd
    }
    fn cd_mut(&mut self) -> &mut CktElementData {
        &mut self.cd
    }

    fn recalc_element_data(&mut self, sys: &SysCtx) {
        self.recalc(sys.positive_sequence);
    }

    /// Pascal `TLineObj.CalcFltRate` (l.1129): the base rate scaled by line
    /// length (`Faultrate · pctperm · 0.01 · Len`, faultrate in per-unit-length
    /// terms). `MilesThisLine` is maintained by the length/units side effects.
    fn reliability_data(&self) -> ReliabilityData {
        ReliabilityData {
            branch_flt_rate: self.fault_rate * self.pct_perm * 0.01 * self.len,
            hrs_to_repair: self.hrs_to_repair,
            miles_this_line: self.miles_this_line,
        }
    }

    fn norm_amps(&self) -> f64 {
        self.norm_amps
    }
    fn emerg_amps(&self) -> f64 {
        self.emerg_amps
    }

    /// Pascal `TLineObj.GetSeqLosses` (Line.pas l.1495): pos/neg/zero-mode
    /// losses summed over both terminals — 3-phase branches only.
    fn get_seq_losses(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
    ) -> (Complex64, Complex64, Complex64) {
        let mut pos = Complex64::ZERO;
        let mut neg = Complex64::ZERO;
        let mut zero = Complex64::ZERO;
        if self.cd.nphases == 3 {
            self.compute_iterminal(sys, node_v);
            let cd = &self.cd;
            let np = cd.nphases;
            let sc = SymComp::default();
            for i in 0..2 {
                let k = i * np;
                let vph: [Complex64; 3] = [
                    node_v[cd.node_ref[k]],
                    node_v[cd.node_ref[k + 1]],
                    node_v[cd.node_ref[k + 2]],
                ];
                let mut v012 = [Complex64::ZERO; 3];
                let mut i012 = [Complex64::ZERO; 3];
                sc.phase_to_sym(&vph, &mut v012);
                sc.phase_to_sym(&cd.iterminal[k..k + 3], &mut i012);
                pos += v012[1] * i012[1].conj();
                neg += v012[2] * i012[2].conj();
                zero += v012[0] * i012[0].conj();
            }
            pos *= 3.0;
            neg *= 3.0;
            zero *= 3.0;
        }
        (pos, neg, zero)
    }

    /// Pascal `TLineObj.CalcYPrim` (sym-component, matrix and geometry paths;
    /// the long-line correction and the <0.51 Hz GIC conversion are Phase 7+).
    fn calc_yprim(&mut self, sys: &SysCtx) {
        let nphases = self.cd.nphases;
        let yorder = self.cd.yorder;

        // Build Z, Yc and the to-be-inverted Zinv. Two paths:
        //  - geometry: `FMakeZFromGeometry` makes the *total* Z/Yc (length and
        //    units already folded into the geometry's `Zmatrix[f, len, units]`),
        //    so Zinv = Z directly and the shunt needs no length/freq scaling.
        //  - sym/linecode: Z/Yc are per-unit-length at base frequency; scale by
        //    length/frequency and the earth-return Rg/Xg before inverting.
        let geometry_path = self.geometry_obj.is_some();
        let mut length_multiplier = 1.0;
        let mut freq_multiplier = 1.0;

        let mut zinv = if geometry_path {
            // Pascal `FMakeZFromGeometry(Solution.Frequency)`.
            if let Err(msg) = self.make_z_from_geometry(sys.frequency) {
                // Pascal: the geometry getter raised `ELineGeometryProblem` and
                // set `SolutionAbort`, so `CalcYPrim` exits without building
                // YPrim. `CalcYPrim` has no solve-time abort channel here, so
                // record the message and leave YPrim unbuilt — the solve cannot
                // converge with the resulting isolated bus. TODO: thread
                // `SolutionAbort` once a solve-time error sink exists.
                self.cd.obj.push_error(msg);
                return;
            }
            self.cd.yprim_freq = sys.frequency;
            self.z.as_ref().expect("make_z_from_geometry set Z").clone()
        } else {
            if self.sym_components_changed {
                // Catch inadvertent user error when C1/C0 were never specified:
                // adjust the kft-based defaults for the new length units.
                if !self.cap_specified {
                    self.c1 /= convert_line_units(LineUnits::Kft, self.length_units);
                    self.c0 /= convert_line_units(LineUnits::Kft, self.length_units);
                    self.cap_specified = true;
                }
                self.recalc(sys.positive_sequence);
            }

            // Z is from line data (per unit length, base frequency).
            length_multiplier = self.len / self.units_convert;
            self.cd.yprim_freq = sys.frequency;
            freq_multiplier = self.cd.yprim_freq / self.cd.base_frequency;

            // Put in series RL, corrected for length and frequency: Rg increases
            // with frequency, Xg is modified by ln of sqrt(1/f).
            let xgmod = if self.xg != 0.0 {
                0.5 * self.kxg * freq_multiplier.ln()
            } else {
                0.0
            };

            let z = self.z.as_ref().expect("recalc ran in the constructor");
            let mut zinv = CMatrix::new(nphases);
            for i in 0..nphases {
                for j in 0..nphases {
                    let zv = z.get(i, j);
                    zinv.set(
                        i,
                        j,
                        Complex64::new(
                            (zv.re + self.rg * (freq_multiplier - 1.0)) * length_multiplier,
                            (zv.im - xgmod) * length_multiplier * freq_multiplier,
                        ),
                    );
                }
            }
            zinv
        };

        // ClearYPrim
        let mut yp_series = CMatrix::new(yorder);
        let mut yp_shunt = CMatrix::new(yorder);
        let mut yprim = CMatrix::new(yorder);

        if zinv.invert().is_err() {
            // Pascal error 183: put in tiny series conductance.
            zinv.clear();
            for i in 0..nphases {
                zinv.set(i, i, Complex64::new(EPSILON, 0.0));
            }
            for i in 0..nphases {
                for j in 0..nphases {
                    let value = zinv.get(i, j);
                    yp_series.set(i, j, value);
                    yp_series.set(i + nphases, j + nphases, value);
                    yp_series.set(i, j + nphases, -value);
                    yp_series.set(j + nphases, i, -value);
                }
            }
        } else {
            for i in 0..nphases {
                for j in 0..nphases {
                    let value = zinv.get(i, j);
                    yp_series.set(i, j, value);
                    yp_series.set(i + nphases, j + nphases, value);
                    yp_series.set(i, j + nphases, -value);
                    yp_series.set(j + nphases, i, -value);
                }
            }
        }

        yprim.copy_from(&yp_series); // initialize YPrim for series impedances

        // Increase diagonals of the series Yprim to avoid isolated buses.
        for i in 0..yorder {
            yp_series.add(i, i, CAP_EPSILON);
        }

        // Shunt: half the capacitive admittance at each end (skip for GIC).
        if sys.frequency > 0.51 {
            let yc = self.yc.as_ref().expect("Z/Yc built above");
            for j in 0..nphases {
                for i in 0..nphases {
                    let ycv = yc.get(i, j);
                    let value = if geometry_path {
                        // Already total (length + frequency folded in); halve it.
                        Complex64::new(ycv.re / 2.0, ycv.im / 2.0)
                    } else {
                        Complex64::new(0.0, ycv.im * length_multiplier * freq_multiplier / 2.0)
                    };
                    yp_shunt.add(i, j, value);
                    yp_shunt.add(i + nphases, j + nphases, value);
                }
            }
        }

        yprim.add_from(&yp_shunt);

        self.cd.yprim_series = Some(yp_series);
        self.cd.yprim_shunt = Some(yp_shunt);
        self.cd.yprim = Some(yprim);

        // Account for open conductors.
        self.cd.apply_yprim_open_conductor_calcs();
        self.cd.yprim_invalid = false;
    }
}

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
        use prop::*;
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
        use prop::*;
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
        use prop::*;
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
        use prop::*;
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
        use prop::*;
        match idx {
            SWITCH => self.is_switch,
            ENABLED => self.cd.enabled,
            _ => unreachable!("Line has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        use prop::*;
        match idx {
            SWITCH => self.is_switch = value,
            ENABLED => self.cd.set_enabled(value),
            _ => unreachable!("Line has no boolean property {idx}"),
        }
    }

    fn get_f64_array(&self, idx: usize) -> Option<&[f64]> {
        match idx {
            prop::RATINGS => Some(&self.amp_ratings),
            _ => unreachable!("Line has no array property {idx}"),
        }
    }
    fn set_f64_array(&mut self, idx: usize, value: Vec<f64>) {
        match idx {
            prop::RATINGS => self.amp_ratings = value,
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
            prop::LINECODE => {
                self.line_code_name = name;
                self.line_code_ref = resolved.map(|(r, _)| r);
                if let Some((_, obj)) = resolved
                    && let Some(code) = obj.as_any().downcast_ref::<LineCodeObj>()
                {
                    self.fetch_line_code(code);
                }
            }
            prop::GEOMETRY => {
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
        use prop::*;
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
        use prop::*;
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

    /// `GetZSeqScale`/`GetCSeqScale`/`GetZmatScale`/`GetYCScale`/`GetB1B0Scale`.
    fn prop_scale(&self, idx: usize, getter: bool) -> f64 {
        use prop::*;
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
                    self.units_convert // no geometry/spacing in Phase 3
                } else {
                    1.0
                }
            }
            CMATRIX => {
                let base = two_pi * self.cd.base_frequency * 1.0e-9;
                if getter {
                    base * self.units_convert
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
        use prop::*;
        match idx {
            R1 | X1 | R0 | X0 | C1 | C0 | B1 | B0 => self.sym_components_model,
            _ => true,
        }
    }

    /// Pascal `TLineObj.PropertySideEffects`.
    fn side_effects(&mut self, idx: usize, prev_int: i32) {
        use prop::*;
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
        // earth resistivity into the geometry (the YPrim invalidation below forces
        // the rebuild). `FZFrequency` is deliberately *not* reset — matching the
        // upstream side effect, which leaves it to the next frequency change.
        if idx == RHO {
            let rho = self.rho;
            if let Some(g) = self.geometry_obj.as_mut() {
                g.set_rho_earth(rho);
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
                | RHO
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

#[cfg(test)]
mod geometry_tests {
    //! WP7.1 step 3a — the `geometry=` Carson path. These drive a `Line` through
    //! the property engine, attach a `LineGeometry`, and assert the resulting
    //! `Z`/`Yc`/`YPrim` against the **same dss-python oracle reference**
    //! (`deri_full_3cond`) the line-constants and LineGeometry matrix unit tests
    //! pin — proving `FetchGeometryCode` + `FMakeZFromGeometry` forward
    //! `f`/`len`/`units`/`earth_model` and embed the total matrices correctly.
    use super::*;
    use crate::elements::general::conductor_data::{WireDataObj, wire_data};
    use crate::elements::general::line_geometry::{self, LineGeometryObj};
    use crate::obj::dss_enum::EnumRegistry;
    use crate::obj::props::{ClassProps, PropEngine};
    use crate::solution::SolveMode;
    use dss_parser::{Parser, ParserVars};

    const M_UNIT: i32 = 4; // LineUnits::Meter code
    const DERI: i32 = 3; // EarthModel::Deri code

    fn assert_close(got: f64, want: f64, what: &str) {
        let tol = 1e-8 * want.abs().max(1e-12);
        assert!(
            (got - want).abs() <= tol,
            "{what}: got {got:.12e}, want {want:.12e}"
        );
    }

    /// Apply a scalar property edit through the property engine (object refs go
    /// through [`set_ref`]). Asserts no deferred error was raised.
    fn scalar(cls: &ClassProps, obj: &mut dyn DssObject, name: &str, value: &str) {
        let enums = EnumRegistry::new();
        let mut parser = Parser::new();
        let vars = ParserVars::new();
        let mut errors = Vec::new();
        let idx = cls.property_index(name).expect("known property");
        let mut eng = PropEngine {
            parser: &mut parser,
            vars: &vars,
            enums: &enums,
            errors: &mut errors,
            foreign: None,
        };
        cls.edit_property(obj, idx, value, &mut eng).unwrap();
        errors.extend(obj.data_mut().take_errors());
        assert!(errors.is_empty(), "edit {name}={value}: {errors:?}");
    }

    /// Mirror the executive's `edit_property` for a single (resolved) object
    /// reference: set the reference, record the set order, run side effects.
    fn set_ref(cls: &ClassProps, obj: &mut dyn DssObject, name: &str, target: &dyn DssObject) {
        let idx = cls.property_index(name).expect("known property");
        let r = ElemRef { cls: 0, idx: 0 };
        obj.set_object_ref(idx, target.data().name().to_string(), Some((r, target)));
        obj.data_mut().set_as_next_seq(idx);
        obj.side_effects(idx, 0);
    }

    fn test_sys() -> SysCtx {
        SysCtx {
            frequency: 60.0,
            fundamental: 60.0,
            is_harmonic_model: false,
            is_dynamic_model: false,
            load_model: 1,
            mode: SolveMode::Snapshot,
            load_multiplier: 1.0,
            gen_multiplier: 1.0,
            generator_dispatch_reference: 0.0,
            price_signal: 25.0,
            default_growth_factor: 1.0,
            year: 0,
            dbl_hour: 0.0,
            solution_count: 0,
            loads_need_updating: false,
            neglect_load_y: false,
            long_line_correction: false,
            positive_sequence: false,
        }
    }

    /// The canonical WP7.1 3-phase overhead geometry (the `deri_full_3cond`
    /// reference): `build_si_wire` at x = 0/1/2 m, h = 10 m, driven through the
    /// LineGeometry editing path.
    fn build_overhead_geometry() -> LineGeometryObj {
        let enums = EnumRegistry::new();
        let wcls = wire_data::class_props(&enums);
        let mut w = WireDataObj::new("w");
        for (n, v) in &[
            ("runits", "m"),
            ("gmrunits", "m"),
            ("radunits", "m"),
            ("rac", "0.0003"),
            ("gmrac", "0.005"),
            ("radius", "0.01"),
        ] {
            scalar(&wcls, &mut w, n, v);
        }
        let gcls = line_geometry::class_props(&enums);
        let mut g = LineGeometryObj::new("geo1");
        scalar(&gcls, &mut g, "nconds", "3");
        scalar(&gcls, &mut g, "nphases", "3");
        for (k, x) in ["0", "1", "2"].iter().enumerate() {
            scalar(&gcls, &mut g, "cond", &(k + 1).to_string());
            set_ref(&gcls, &mut g, "wire", &w);
            scalar(&gcls, &mut g, "x", x);
            scalar(&gcls, &mut g, "h", "10");
            scalar(&gcls, &mut g, "units", "m");
        }
        g
    }

    #[test]
    fn geometry_path_builds_oracle_z_and_yc() {
        let enums = EnumRegistry::new();
        let lcls = class_props(&enums);

        let mut geom = build_overhead_geometry();
        // Oracle reference (the `deri_full_3cond` total matrices at 1 m): the
        // identical numbers the line_constants + LineGeometry unit tests pin.
        let z_ref = geom.z_matrix(60.0, 1.0, M_UNIT, DERI).expect("z_ref");
        let yc_ref = geom.yc_matrix(60.0, 1.0, M_UNIT, DERI).expect("yc_ref");

        let mut line = Line::new("l1");
        scalar(&lcls, &mut line, "length", "1");
        scalar(&lcls, &mut line, "units", "m");
        set_ref(&lcls, &mut line, "geometry", &geom);

        // FetchGeometryCode adopted the geometry.
        assert_eq!(line.geometry_name, "geo1");
        assert_eq!(line.cd.nphases, 3);
        assert_eq!(line.cd.nconds, 3);
        assert!(!line.sym_components_model);
        assert_eq!(line.line_type, geom.line_type());
        assert_eq!(line.norm_amps, geom.norm_amps());
        assert_eq!(line.emerg_amps, geom.emerg_amps());

        line.calc_yprim(&test_sys());

        // The geometry's TOTAL Z/Yc flowed into the Line (entry by entry) — the
        // sym branch would have produced a completely different matrix.
        let z = line.z.as_ref().expect("z");
        let yc = line.yc.as_ref().expect("yc");
        for i in 0..3 {
            for j in 0..3 {
                assert_close(z.get(i, j).re, z_ref.get(i, j).re, "Z.re");
                assert_close(z.get(i, j).im, z_ref.get(i, j).im, "Z.im");
                assert_close(yc.get(i, j).re, yc_ref.get(i, j).re, "Yc.re");
                assert_close(yc.get(i, j).im, yc_ref.get(i, j).im, "Yc.im");
            }
        }
        // Oracle anchor: Z[0][0] is the `deri_full_3cond` diagonal (length 1 m).
        assert_close(z.get(0, 0).re, 3.525947626277e-04, "Z00.re");
        assert_close(z.get(0, 0).im, 9.150978496084e-04, "Z00.im");

        // YPrim series embeds Zinv = Z^-1 in the 2-terminal Kron pattern: the
        // off-diagonal block [i][j+n] = -Zinv[i][j] (no CAP_EPSILON there).
        let mut zinv = z.clone();
        zinv.invert().expect("Z invertible");
        let yps = line.cd.yprim_series.as_ref().expect("yprim_series");
        for i in 0..3 {
            for j in 0..3 {
                assert_close(yps.get(i, j + 3).re, -zinv.get(i, j).re, "Yps.re");
                assert_close(yps.get(i, j + 3).im, -zinv.get(i, j).im, "Yps.im");
            }
        }
        // YPrim shunt = half the total Yc at the near-end block (already total —
        // not rescaled by length/frequency, unlike the sym path).
        let ypsh = line.cd.yprim_shunt.as_ref().expect("yprim_shunt");
        for i in 0..3 {
            for j in 0..3 {
                assert_close(ypsh.get(i, j).im, yc_ref.get(i, j).im / 2.0, "Ypsh.im");
            }
        }
    }

    #[test]
    fn geometry_length_units_scale_the_total_z() {
        // `length`/`units` feed `Zmatrix[f, len, units]`, so a 2 km line is the
        // 1 m total × 1000 × 2 (guards the object→engine forward of len/units —
        // a hardcoded 1.0/meters or swapped arg would fail here).
        let enums = EnumRegistry::new();
        let lcls = class_props(&enums);
        let mut geom = build_overhead_geometry();
        let z_ref = geom.z_matrix(60.0, 1.0, M_UNIT, DERI).expect("z_ref");
        let factor = 1000.0 * 2.0; // from_per_meter(km) * length

        let mut line = Line::new("l1");
        scalar(&lcls, &mut line, "length", "2");
        scalar(&lcls, &mut line, "units", "km");
        set_ref(&lcls, &mut line, "geometry", &geom);
        line.calc_yprim(&test_sys());

        let z = line.z.as_ref().expect("z");
        for i in 0..3 {
            for j in 0..3 {
                assert_close(z.get(i, j).re, z_ref.get(i, j).re * factor, "Zkm.re");
                assert_close(z.get(i, j).im, z_ref.get(i, j).im * factor, "Zkm.im");
            }
        }
    }

    #[test]
    fn sym_scalar_detaches_geometry() {
        // A sym-component edit after `geometry=` runs Pascal KillGeometrySpecified:
        // the geometry is dropped and the sym model takes over.
        let enums = EnumRegistry::new();
        let lcls = class_props(&enums);
        let geom = build_overhead_geometry();
        let mut line = Line::new("l1");
        set_ref(&lcls, &mut line, "geometry", &geom);
        assert!(line.geometry_obj.is_some());

        scalar(&lcls, &mut line, "r1", "0.1");
        assert!(line.geometry_obj.is_none());
        assert_eq!(line.geometry_name, "");
        assert!(line.sym_components_model);
        assert!(line.sym_components_changed);
        assert_eq!(line.fz_frequency, -1.0);
    }
}
