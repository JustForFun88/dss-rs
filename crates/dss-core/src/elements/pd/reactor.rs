//! Port of `PDElements/Reactor.pas` — `TReactorObj`, a two-terminal
//! constant-impedance shunt (or series) reactor. Like the capacitor it follows
//! the Capacitor/Fault connection rules: Bus2 defaults to the grounded-zero node
//! of Bus1 (a shunt reactor); specifying Bus2 with matching nodes makes a series
//! reactor. `Parallel=Yes` treats the `R` and `X` components as parallel.
//!
//! Reactance is specified one of four ways (`SpecType`):
//!   1. `kvar`+`kV` ratings at base frequency.
//!   2. series `R`+`X` ohms (or `R`+`LmH`, or the `Z` complex array).
//!   3. `RMatrix`/`XMatrix` ohms (optionally in parallel).
//!   4. symmetrical components `Z1`, `Z2`, `Z0` (`Z2`/`Z0` default to `Z1`).
//!
//! `RCurve`/`LCurve` reference an `XYcurve` (ported in PHASE5_PLAN WP5.1) but
//! stay flagged `NOT_PORTED`: their only consumer is the frequency-dependent
//! `R(f)`/`L(f)` scaling in the *harmonic* `CalcYPrim`, which is Phase 7. Until
//! then `CalcYPrim` always uses the unity-curve path, so resolving the
//! reference would be dead state with no observable behavior — wire it together
//! with the harmonic scaling.

use num_complex::Complex64;

use crate::elements::ckt::CktElementData;
use crate::elements::traits::{CktElement, SysCtx};
use crate::obj::base::{DssObjData, DssObject};
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};
use crate::support::cmatrix::CMatrix;
use crate::support::mathutil::etk_invert;
use crate::util::{EPSILON, sqrt3};

/// 1-based property ordinals (Pascal `TReactorProp` + class tails).
pub mod prop {
    pub const BUS1: usize = 1;
    pub const BUS2: usize = 2;
    pub const PHASES: usize = 3;
    pub const KVAR: usize = 4;
    pub const KV: usize = 5;
    pub const CONN: usize = 6;
    pub const RMATRIX: usize = 7;
    pub const XMATRIX: usize = 8;
    pub const PARALLEL: usize = 9;
    pub const R: usize = 10;
    pub const X: usize = 11;
    pub const RP: usize = 12;
    pub const Z1: usize = 13;
    pub const Z2: usize = 14;
    pub const Z0: usize = 15;
    pub const Z: usize = 16;
    pub const RCURVE: usize = 17;
    pub const LCURVE: usize = 18;
    pub const LMH: usize = 19;
    // TPDClass tail:
    pub const NORMAMPS: usize = 20;
    pub const EMERGAMPS: usize = 21;
    pub const FAULTRATE: usize = 22;
    pub const PCTPERM: usize = 23;
    pub const REPAIR: usize = 24;
    // TCktElementClass tail:
    pub const BASE_FREQ: usize = 25;
    pub const ENABLED: usize = 26;
    pub const NUM_PROPS: usize = 27; // incl. Like
}

/// `TReactor.DefineProperties`.
pub fn class_props(enums: &EnumRegistry) -> ClassProps {
    use prop::*;
    let defs = vec![
        // Pascal flags bus1 `Required` (inert here — not enforced in Phase 4).
        PropDef::bus("bus1", 1),
        PropDef::bus("bus2", 2),
        PropDef::integer("phases").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::double("kvar").flags(PropFlags::REQUIRED_IN_SPEC_SET),
        PropDef::double("kv").flags(PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::NON_NEGATIVE),
        PropDef::mapped_string_enum("conn", enums.connection),
        PropDef::double_sym_matrix("RMatrix", PHASES),
        PropDef::double_sym_matrix("XMatrix", PHASES).flags(PropFlags::REQUIRED_IN_SPEC_SET),
        PropDef::boolean("Parallel"),
        PropDef::double("R").flags(PropFlags::REDUNDANT),
        PropDef::double("X").flags(PropFlags::REDUNDANT | PropFlags::REQUIRED_IN_SPEC_SET),
        PropDef::double("Rp"),
        PropDef::complex("Z1"),
        PropDef::complex("Z2"),
        PropDef::complex("Z0"),
        PropDef::complex("Z").flags(PropFlags::REQUIRED_IN_SPEC_SET),
        // RCurve/LCurve reference XYcurve (ported WP5.1) but are consumed only
        // by the harmonic CalcYPrim (Phase 7); see the module note.
        PropDef::object_ref("RCurve").flags(PropFlags::NOT_PORTED),
        PropDef::object_ref("LCurve").flags(PropFlags::NOT_PORTED),
        PropDef::double("LmH")
            .scale(1.0e-3)
            .flags(PropFlags::REDUNDANT | PropFlags::REQUIRED_IN_SPEC_SET),
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
    ClassProps::new("Reactor", defs, true)
}

/// `TReactorObj`.
#[derive(Debug, Clone)]
pub struct Reactor {
    pub cd: CktElementData,
    /// Parallel resistance and its conductance (`Rp`, `Gp`).
    rp: f64,
    gp: f64,
    /// Inductance in henries (Pascal `L`; the `LmH` property scales by 1e-3).
    l: f64,
    kvarrating: f64,
    kvrating: f64,
    /// Series impedance (`Z`); the `R`/`X` redundant properties alias `Z.re`/im.
    z: Complex64,
    /// Symmetrical-component impedances (`SpecType = 4`).
    z1: Complex64,
    z2: Complex64,
    z0: Complex64,
    /// `RMatrix`/`XMatrix` ohms (row-major `nphases²`); `None` unless
    /// `SpecType = 3`. `gmatrix`/`bmatrix` are the inverted parallel forms.
    rmatrix: Option<Vec<f64>>,
    xmatrix: Option<Vec<f64>>,
    gmatrix: Option<Vec<f64>>,
    bmatrix: Option<Vec<f64>>,
    /// 0 = wye (default), 1 = delta.
    connection: i32,
    /// 1 = kvar, 2 = R+jX, 3 = R/X matrices, 4 = symmetrical components.
    spec_type: i32,
    is_parallel: bool,
    rp_specified: bool,
    bus2_defined: bool,
    z2_specified: bool,
    z0_specified: bool,
    is_shunt: bool,
    // PD-element common:
    norm_amps: f64,
    emerg_amps: f64,
    norm_amps_specified: bool,
    emerg_amps_specified: bool,
    fault_rate: f64,
    pct_perm: f64,
    hrs_to_repair: f64,
}

impl Reactor {
    /// Pascal `TReactorObj.Create`.
    pub fn new(name: &str) -> Self {
        let mut cd = CktElementData::new(name, prop::NUM_PROPS);
        cd.nphases = 3;
        cd.nconds = 3;
        cd.set_nterms(2); // forces allocation of terminals/conductors + buses

        // Default Bus2 to the grounded-zero node of Bus1 (`Bus1.0.0.0`).
        let bus1 = cd.get_bus(1).to_string();
        cd.set_bus(2, &format!("{bus1}.0.0.0"));

        let kvarrating = 100.0;
        let kvrating = 12.47;
        let z = Complex64::new(0.0, kvrating * kvrating * 1000.0 / kvarrating);

        let mut r = Self {
            cd,
            rp: 0.0,
            gp: 0.0,
            l: 0.0,
            kvarrating,
            kvrating,
            z,
            z1: Complex64::new(0.0, 0.0),
            z2: Complex64::new(0.0, 0.0),
            z0: Complex64::new(0.0, 0.0),
            rmatrix: None,
            xmatrix: None,
            gmatrix: None,
            bmatrix: None,
            connection: 0, // wye
            spec_type: 1,  // kvar
            is_parallel: false,
            rp_specified: false,
            bus2_defined: false,
            z2_specified: false,
            z0_specified: false,
            is_shunt: true,
            norm_amps: kvarrating * sqrt3() / kvrating,
            emerg_amps: 0.0,
            norm_amps_specified: false,
            emerg_amps_specified: false,
            fault_rate: 0.0005,
            pct_perm: 100.0,
            hrs_to_repair: 3.0,
        };
        r.emerg_amps = r.norm_amps * 1.35;
        r.cd.yorder = r.cd.nterms * r.cd.nconds;
        r.recalc();
        r
    }

    /// Pascal per-phase voltage selection (`RecalcElementData`): delta uses the
    /// coil rating; wye assumes a three-phase line-line rating for 2/3 phases.
    fn phase_kv(&self) -> f64 {
        if self.connection == 1 {
            self.kvrating // delta: line-line
        } else {
            match self.cd.nphases {
                2 | 3 => self.kvrating / sqrt3(),
                _ => self.kvrating,
            }
        }
    }

    /// Pascal `RecalcElementData`: derive `Z.im`/`L` from the spec, set `Gp` from
    /// `Rp`, build the inverted parallel `Gmatrix`/`Bmatrix` when needed, and
    /// (unless overridden) the default Norm/Emerg current ratings.
    fn recalc(&mut self) {
        let two_pi = 2.0 * std::f64::consts::PI;
        let w = two_pi * self.cd.base_frequency;

        match self.spec_type {
            1 => {
                // kvar
                let kvar_per_phase = self.kvarrating / self.cd.nphases as f64;
                let phase_kv = self.phase_kv();
                self.z.im = phase_kv * phase_kv * 1000.0 / kvar_per_phase;
                self.l = self.z.im / w;
                // Leave R as specified.
                if !self.norm_amps_specified {
                    self.norm_amps = kvar_per_phase / phase_kv;
                }
                if !self.emerg_amps_specified {
                    self.emerg_amps = kvar_per_phase / phase_kv * 1.35;
                }
            }
            2 => {
                // R + jX: nothing much to do.
                self.l = self.z.im / w;
            }
            _ => {} // matrices / sym components: handled in CalcYPrim
        }

        if self.rp_specified && self.rp != 0.0 {
            self.gp = 1.0 / self.rp;
        } else {
            self.gp = 0.0; // default to 0 if Rp = 0
        }

        if self.is_parallel && self.spec_type == 3 {
            let nphases = self.cd.nphases;
            let n2 = nphases * nphases;
            // Copy Rmatrix to Gmatrix and invert (Pascal comment notes the source
            // bug where Rmatrix was inverted in place; the ported code inverts the
            // copy, matching the shipped binary).
            let mut g = self.rmatrix.clone().unwrap_or_else(|| vec![0.0; n2]);
            if etk_invert(&mut g, nphases).is_err() {
                self.cd.obj.push_error(format!(
                    "Error inverting R Matrix for \"{}\" - G is zeroed.",
                    self.cd.obj.name()
                ));
                g.iter_mut().for_each(|v| *v = 0.0);
            }
            self.gmatrix = Some(g);

            // Copy -Xmatrix to Bmatrix and invert.
            let mut b: Vec<f64> = self
                .xmatrix
                .as_deref()
                .unwrap_or(&vec![0.0; n2])
                .iter()
                .map(|v| -v)
                .collect();
            if etk_invert(&mut b, nphases).is_err() {
                self.cd.obj.push_error(format!(
                    "Error inverting X Matrix for \"{}\" - B is zeroed.",
                    self.cd.obj.name()
                ));
                b.iter_mut().for_each(|v| *v = 0.0);
            }
            self.bmatrix = Some(b);
        }
    }

    /// Build the series-impedance `ZMatrix` (already inverted to a Y matrix) for
    /// `SpecType = 3` series and `SpecType = 4`, then stamp it into the four
    /// quadrants of the two-terminal `work` matrix. `zmat[i][j]` is `nphases²`
    /// row-major. Mirrors the shared Pascal stamping loop.
    fn stamp_series(work: &mut CMatrix, zmat: &mut CMatrix, nphases: usize) {
        if zmat.invert().is_err() {
            // Inversion error: tiny series conductance on the diagonal.
            zmat.clear();
            for i in 0..nphases {
                zmat.set(i, i, Complex64::new(EPSILON, 0.0));
            }
        }
        for i in 0..nphases {
            for j in 0..nphases {
                let value = zmat.get(i, j);
                work.set(i, j, value);
                work.set(i + nphases, j + nphases, value);
                work.set(i, j + nphases, -value);
                work.set(j + nphases, i, -value);
            }
        }
    }
}

impl CktElement for Reactor {
    fn cd(&self) -> &CktElementData {
        &self.cd
    }
    fn cd_mut(&mut self) -> &mut CktElementData {
        &mut self.cd
    }

    fn recalc_element_data(&mut self, _sys: &SysCtx) {
        self.recalc();
    }

    fn norm_amps(&self) -> f64 {
        self.norm_amps
    }
    fn emerg_amps(&self) -> f64 {
        self.emerg_amps
    }

    /// Pascal `TReactorObj.GetLosses` (Reactor.pas l.1017): no-load losses are
    /// `V²/Rp` across the shunt — only when `Rp` is specified on a shunt
    /// reactor; otherwise the default element behavior.
    fn get_losses_split(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
    ) -> (Complex64, Complex64, Complex64) {
        if !self.cd.enabled || self.cd.node_ref.is_empty() {
            return (Complex64::ZERO, Complex64::ZERO, Complex64::ZERO);
        }
        if self.rp_specified && self.is_shunt && self.rp != 0.0 {
            let total = self.losses(sys, node_v);
            let mut no_load = 0.0_f64;
            let cd = &self.cd;
            for i in 0..cd.nphases {
                let v = node_v[cd.node_ref[i]];
                no_load += (v.re * v.re + v.im * v.im) / self.rp;
            }
            if sys.positive_sequence {
                no_load *= 3.0;
            }
            let no_load = Complex64::new(no_load, 0.0);
            (total, total - no_load, no_load)
        } else {
            let total = self.losses(sys, node_v);
            (total, total, Complex64::ZERO)
        }
    }

    /// Pascal `TPDElement.IsShunt` (set by the Bus1/Bus2 side effects).
    fn is_shunt(&self) -> bool {
        self.is_shunt
    }

    /// Pascal `TReactorObj.CalcYPrim`: stamp the reactor admittance by spec type
    /// into the shunt (or series) primitive, then mirror tiny diagonals into the
    /// other matrix so `CalcVoltages` never sees an all-zero row.
    fn calc_yprim(&mut self, sys: &SysCtx) {
        let two_pi = 2.0 * std::f64::consts::PI;
        let yorder = self.cd.yorder;
        let nphases = self.cd.nphases;
        let nconds = self.cd.nconds;

        let mut yprim_freq = sys.frequency;
        let mut freq_multiplier = yprim_freq / self.cd.base_frequency;
        let mut z = self.z; // local copy (the GIC path may adjust Z.re)

        // If GIC simulation (< 0.5 Hz), resistance only.
        if sys.frequency < 0.51 {
            if z.im > 0.0 && z.re <= 0.0 {
                z.re = z.im / 50.0; // assume X/R = 50
            }
            yprim_freq = 0.0;
            freq_multiplier = 0.0;
        }
        self.cd.yprim_freq = yprim_freq;

        let mut work = CMatrix::new(yorder);

        match self.spec_type {
            1 | 2 => {
                // Some form of R and X specified. RCurve/LCurve are NOT_PORTED, so
                // R(f)/L(f) always use the stored values (unity curve).
                let r_value = z.re;
                let l_value = self.l;
                let mut value = Complex64::new(r_value, l_value * two_pi * yprim_freq).inv();
                if self.rp_specified {
                    value += self.gp;
                }
                let value2 = -value;

                if self.connection == 1 {
                    // Delta (line-line); AddElement accumulates.
                    for i in 1..=nphases {
                        let mut j = i + 1;
                        if j > nconds {
                            j = 1;
                        }
                        work.add(i - 1, i - 1, value);
                        work.add(j - 1, j - 1, value);
                        work.add(i - 1, j - 1, value2);
                        work.add(j - 1, i - 1, value2);
                    }
                } else {
                    // Wye: elements only on the diagonals.
                    for i in 1..=nphases {
                        let j = i + nphases;
                        work.set(i - 1, i - 1, value);
                        work.set(j - 1, j - 1, value);
                        work.set(i - 1, j - 1, value2);
                        work.set(j - 1, i - 1, value2);
                    }
                }
            }
            3 => {
                // R/X matrices.
                if self.is_parallel {
                    let g = self
                        .gmatrix
                        .as_ref()
                        .expect("parallel SpecType 3 has Gmatrix");
                    let b = self
                        .bmatrix
                        .as_ref()
                        .expect("parallel SpecType 3 has Bmatrix");
                    for i in 1..=nphases {
                        for j in 1..=nphases {
                            let idx = (j - 1) * nphases + (i - 1);
                            let value = if freq_multiplier > 0.0 {
                                Complex64::new(g[idx], b[idx] / freq_multiplier)
                            } else {
                                Complex64::new(g[idx], 0.0)
                            };
                            work.set(i - 1, j - 1, value);
                            work.set(i - 1 + nphases, j - 1 + nphases, value);
                            work.set(i - 1, j - 1 + nphases, -value);
                            work.set(j - 1 + nphases, i - 1, -value);
                        }
                    }
                } else {
                    // Series R and X: build Z, invert, stamp.
                    let rm = self.rmatrix.as_ref().expect("SpecType 3 has Rmatrix");
                    let xm = self.xmatrix.as_ref().expect("SpecType 3 has Xmatrix");
                    let mut zmat = CMatrix::new(nphases);
                    for i in 0..nphases {
                        for j in 0..nphases {
                            let k = i * nphases + j;
                            zmat.set(i, j, Complex64::new(rm[k], xm[k] * freq_multiplier));
                        }
                    }
                    Self::stamp_series(&mut work, &mut zmat, nphases);
                }
            }
            _ => {
                // Symmetrical-component Z's specified (SpecType 4).
                let mut zmat = CMatrix::new(nphases);
                // Diagonal — all the same.
                let mut value = if nphases == 1 {
                    self.z1
                } else {
                    self.z2 + self.z1 + self.z0
                };
                value.im *= freq_multiplier;
                value /= 3.0;
                for i in 0..nphases {
                    zmat.set(i, i, value);
                }

                if nphases == 3 {
                    // TODO(compat): Pascal `CALPHA` is the truncated literal
                    // (-0.5, -0.866025) (DSSGlobals.pas:74), not the exact 1∠-120°.
                    // `Calpha1 := cong(Calpha)` then flips it to 1∠+120° "to agree
                    // with textbooks". The clean fix uses an exact 120° rotation.
                    let calpha = Complex64::new(-0.5, -0.866025);
                    let calpha1 = calpha.conj();
                    let calpha2 = calpha1 * calpha1;
                    let mut value2 = calpha2 * self.z2 + calpha1 * self.z1 + self.z0;
                    let mut value1 = calpha2 * self.z1 + calpha1 * self.z2 + self.z0;
                    value1.im *= freq_multiplier;
                    value2.im *= freq_multiplier;
                    value1 /= 3.0;
                    value2 /= 3.0;
                    // Lower triangle.
                    zmat.set(1, 0, value1);
                    zmat.set(2, 0, value2);
                    zmat.set(2, 1, value1);
                    // Upper triangle.
                    zmat.set(0, 1, value2);
                    zmat.set(0, 2, value1);
                    zmat.set(1, 2, value2);
                }

                Self::stamp_series(&mut work, &mut zmat, nphases);
            }
        }

        // Distribute the work matrix into shunt/series and mirror diagonals so
        // CalcVoltages doesn't fail.
        let mut yp_series = CMatrix::new(yorder);
        let mut yp_shunt = CMatrix::new(yorder);
        if self.is_shunt {
            yp_shunt.copy_from(&work);
            // 1-phase non-positive-sequence: assume a neutral/grounding reactor,
            // leave the full diagonal in the circuit; otherwise scale it down.
            let factor = if nphases == 1 && !sys.positive_sequence {
                1.0
            } else {
                1.0e-10
            };
            for i in 0..yorder {
                yp_series.set(i, i, work.get(i, i) * factor);
            }
        } else {
            yp_series.copy_from(&work);
        }

        let mut yprim = CMatrix::new(yorder);
        yprim.copy_from(&work);

        self.cd.yprim_series = Some(yp_series);
        self.cd.yprim_shunt = Some(yp_shunt);
        self.cd.yprim = Some(yprim);

        self.cd.apply_yprim_open_conductor_calcs();
        self.cd.yprim_invalid = false;
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
        use prop::*;
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
        use prop::*;
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
        use prop::*;
        match idx {
            PHASES => self.cd.nphases as i32,
            CONN => self.connection,
            _ => unreachable!("Reactor has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        use prop::*;
        match idx {
            PHASES => self.cd.nphases = value.max(0) as usize,
            CONN => self.connection = value,
            _ => unreachable!("Reactor has no integer property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        use prop::*;
        match idx {
            PARALLEL => self.is_parallel,
            ENABLED => self.cd.enabled,
            _ => unreachable!("Reactor has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        use prop::*;
        match idx {
            PARALLEL => self.is_parallel = value,
            ENABLED => self.cd.set_enabled(value),
            _ => unreachable!("Reactor has no boolean property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        use prop::*;
        match idx {
            // RCurve/LCurve are NOT_PORTED (no XYcurve until Phase 5) — never set,
            // so the reference always renders empty, matching the oracle.
            RCURVE | LCURVE => String::new(),
            _ => unreachable!("Reactor has no string property {idx}"),
        }
    }

    fn get_complex(&self, idx: usize) -> (f64, f64) {
        use prop::*;
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
        use prop::*;
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
        use prop::*;
        match idx {
            RMATRIX => self.rmatrix.as_deref(),
            XMATRIX => self.xmatrix.as_deref(),
            _ => unreachable!("Reactor has no double-array property {idx}"),
        }
    }
    fn set_f64_array(&mut self, idx: usize, value: Vec<f64>) {
        use prop::*;
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

    /// Pascal `TReactorObj.PropertySideEffects`.
    fn side_effects(&mut self, idx: usize, prev_int: i32) {
        use prop::*;
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
    fn end_edit(&mut self) {
        self.recalc();
    }

    /// Pascal `TReactorObj.MakeLike`.
    fn make_like(&mut self, other: &dyn DssObject) {
        let Some(other) = other.as_any().downcast_ref::<Reactor>() else {
            return;
        };
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

        // TPDElement.MakeLike copies the rating fields.
        self.norm_amps = other.norm_amps;
        self.emerg_amps = other.emerg_amps;
        self.fault_rate = other.fault_rate;
        self.pct_perm = other.pct_perm;
        self.hrs_to_repair = other.hrs_to_repair;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solution::SolveMode;

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

    #[test]
    fn default_is_3ph_wye_shunt() {
        let r = Reactor::new("r1");
        assert_eq!(r.cd.nphases, 3);
        assert_eq!(r.cd.nconds, 3);
        assert_eq!(r.cd.nterms, 2);
        assert_eq!(r.cd.yorder, 6);
        assert!(r.is_shunt);
        assert_eq!(r.spec_type, 1);
        assert_eq!(r.get_bus_name(2), "r1_1.0.0.0");
    }

    /// kvar-spec 3φ wye, `kvar=500 kV=12.47`: every diagonal is `-jb`, the
    /// `[i,i+3]` off-diagonal `+jb`, with `b = 0.00321542` (probed in dss-python).
    #[test]
    fn yprim_3ph_wye_kvar_matches_oracle() {
        let mut r = Reactor::new("r");
        r.kvarrating = 500.0;
        r.kvrating = 12.47;
        r.recalc();
        r.calc_yprim(&test_sys());

        let yp = r.cd.yprim.as_ref().unwrap();
        let b = 0.00321542_f64;
        for i in 0..3 {
            let d = yp.get(i, i);
            assert!(d.re.abs() < 1e-9, "diag re {}", d.re);
            assert!((d.im + b).abs() < 1e-7, "diag im {} vs {}", d.im, -b);
            let off = yp.get(i, i + 3);
            assert!((off.im - b).abs() < 1e-7, "off im {} vs {b}", off.im);
        }
    }

    /// Symmetrical components `Z1=Z2=(1,5) Z0=(2,8)`, 3φ. Probed in dss-python:
    /// diagonal `0.0354449 - 0.167421j`, in-block off `-0.0030166 + 0.0248869j`.
    #[test]
    fn yprim_3ph_z1z2z0_matches_oracle() {
        let mut r = Reactor::new("r");
        r.spec_type = 4;
        r.z1 = Complex64::new(1.0, 5.0);
        r.z2 = Complex64::new(1.0, 5.0);
        r.z0 = Complex64::new(2.0, 8.0);
        r.recalc();
        r.calc_yprim(&test_sys());

        let yp = r.cd.yprim.as_ref().unwrap();
        let diag = Complex64::new(0.0354449, -0.167421);
        let off = Complex64::new(-0.0030166, 0.0248869);
        for i in 0..3 {
            assert!((yp.get(i, i) - diag).norm() < 1e-5, "diag {}", yp.get(i, i));
            for j in 0..3 {
                if i != j {
                    assert!((yp.get(i, j) - off).norm() < 1e-5, "off {}", yp.get(i, j));
                }
            }
            // Cross-block is negated.
            assert!((yp.get(i, i + 3) + diag).norm() < 1e-5);
        }
    }

    /// RMatrix/XMatrix series spec (`bus2` set), 3φ. Probed in dss-python:
    /// diagonal `0.0412088 - 0.206044j`, in-block off `-0.00686813 + 0.0343407j`.
    #[test]
    fn yprim_3ph_rxmatrix_matches_oracle() {
        let mut r = Reactor::new("r");
        r.spec_type = 3;
        r.is_shunt = false; // bus2 set → series
        // Row-major nphases² (symmetric).
        r.rmatrix = Some(vec![1.0, 0.2, 0.2, 0.2, 1.0, 0.2, 0.2, 0.2, 1.0]);
        r.xmatrix = Some(vec![5.0, 1.0, 1.0, 1.0, 5.0, 1.0, 1.0, 1.0, 5.0]);
        r.recalc();
        r.calc_yprim(&test_sys());

        let yp = r.cd.yprim.as_ref().unwrap();
        let diag = Complex64::new(0.0412088, -0.206044);
        let off = Complex64::new(-0.00686813, 0.0343407);
        for i in 0..3 {
            assert!((yp.get(i, i) - diag).norm() < 1e-5, "diag {}", yp.get(i, i));
            for j in 0..3 {
                if i != j {
                    assert!((yp.get(i, j) - off).norm() < 1e-5, "off {}", yp.get(i, j));
                }
            }
            assert!((yp.get(i, i + 3) + diag).norm() < 1e-5);
        }
    }
}
