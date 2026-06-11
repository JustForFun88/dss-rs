//! Port of `PCElements/Vsource.pas` — `TVsourceObj`, the Thevenin-equivalent
//! voltage source. A **2-terminal** device: terminal 2 defaults to the zero
//! (ground) nodes of bus 1, and the primitive Y is the 2N×2N block matrix
//! `[Zinv, −Zinv; −Zinv, Zinv]` built from the full sequence-impedance
//! matrix `Z` (`Zs`/`Zm` from Z1/Z2/Z0).

use num_complex::Complex64;

use crate::elements::ckt::CktElementData;
use crate::elements::traits::{CktElement, InjCtx, SysCtx};
use crate::obj::base::{DssObjData, DssObject};
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};
use crate::support::cmatrix::CMatrix;
use crate::support::complexutil::pdeg_to_complex;
use crate::util::{CALPHA, EPSILON, EPSILON2, quad_solver, sqrt3};

/// 1-based property ordinals (Pascal `TVsourceProp` + the class tails).
pub mod prop {
    pub const BUS1: usize = 1;
    pub const BASEKV: usize = 2;
    pub const PU: usize = 3;
    pub const ANGLE: usize = 4;
    pub const FREQUENCY: usize = 5;
    pub const PHASES: usize = 6;
    pub const MVASC3: usize = 7;
    pub const MVASC1: usize = 8;
    pub const X1R1: usize = 9;
    pub const X0R0: usize = 10;
    pub const ISC3: usize = 11;
    pub const ISC1: usize = 12;
    pub const R1: usize = 13;
    pub const X1: usize = 14;
    pub const R0: usize = 15;
    pub const X0: usize = 16;
    pub const SCAN_TYPE: usize = 17;
    pub const SEQUENCE: usize = 18;
    pub const BUS2: usize = 19;
    pub const Z1: usize = 20;
    pub const Z0: usize = 21;
    pub const Z2: usize = 22;
    pub const PUZ1: usize = 23;
    pub const PUZ0: usize = 24;
    pub const PUZ2: usize = 25;
    pub const BASE_MVA: usize = 26;
    pub const YEARLY: usize = 27;
    pub const DAILY: usize = 28;
    pub const DUTY: usize = 29;
    pub const MODEL: usize = 30;
    pub const PUZ_IDEAL: usize = 31;
    // TPCClass / TCktElementClass tails:
    pub const SPECTRUM: usize = 32;
    pub const BASE_FREQ: usize = 33;
    pub const ENABLED: usize = 34;
    pub const NUM_PROPS: usize = 35; // incl. the auto-appended Like
}

/// Build the `Vsource` property table (`TVSource.DefineProperties`).
pub fn class_props(enums: &EnumRegistry) -> ClassProps {
    use prop::*;
    let mut defs = vec![
        PropDef::bus("bus1", 1).flags(PropFlags::NONE),
        PropDef::double("basekv"),
        PropDef::double("pu"),
        PropDef::double("angle"),
        PropDef::double("frequency").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::integer("phases").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::double("MVAsc3"),
        PropDef::double("MVAsc1"),
        PropDef::double("x1r1"),
        PropDef::double("x0r0"),
        PropDef::double("Isc3"),
        PropDef::double("Isc1"),
        PropDef::double("R1").flags(PropFlags::REDUNDANT),
        PropDef::double("X1").flags(PropFlags::REDUNDANT),
        PropDef::double("R0").flags(PropFlags::REDUNDANT),
        PropDef::double("X0").flags(PropFlags::REDUNDANT),
        PropDef::mapped_string_enum("ScanType", enums.scan_type),
        PropDef::mapped_string_enum("Sequence", enums.sequence),
        PropDef::bus("bus2", 2),
        PropDef::complex("Z1"),
        PropDef::complex("Z0"),
        PropDef::complex("Z2"),
        PropDef::complex("puZ1"),
        PropDef::complex("puZ0"),
        PropDef::complex("puZ2"),
        PropDef::double("baseMVA"),
        PropDef::object_ref("Yearly"),
        PropDef::object_ref("Daily"),
        PropDef::object_ref("Duty"),
        PropDef::mapped_string_enum("Model", enums.vsource_model),
        PropDef::complex("puZideal"),
        // PCClass tail:
        PropDef::object_ref("spectrum"),
        // CktElementClass tail:
        PropDef::double("basefreq").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::enabled("enabled"),
    ];
    debug_assert_eq!(defs.len(), NUM_PROPS - 1);
    defs.shrink_to_fit();
    ClassProps::new("Vsource", defs, true)
}

/// `TVsourceObj`.
#[derive(Debug, Clone)]
pub struct VSource {
    pub cd: CktElementData,

    pub mva_sc3: f64,
    pub mva_sc1: f64,
    pub isc3: f64,
    pub isc1: f64,
    /// 1 = MVAsc, 2 = Isc, 3 = Z specified.
    pub z_spec_type: i32,
    pub r1: f64,
    pub x1: f64,
    pub r2: f64,
    pub x2: f64,
    pub r0: f64,
    pub x0: f64,
    pub x1r1: f64,
    pub x0r0: f64,
    pub base_mva: f64,
    pub pu_z1: Complex64,
    pub pu_z0: Complex64,
    pub pu_z2: Complex64,
    pub pu_z_ideal: Complex64,
    pub z_base: f64,
    pub bus2_defined: bool,
    pub z1_specified: bool,
    pub pu_z1_specified: bool,
    pub pu_z0_specified: bool,
    pub pu_z2_specified: bool,
    pub z2_specified: bool,
    pub z0_specified: bool,
    pub is_quasi_ideal: bool,
    pub scan_type: i32,
    pub sequence_type: i32,
    /// Base-frequency series Z matrix (order = nphases).
    pub z: Option<CMatrix>,
    pub zinv: Option<CMatrix>,
    pub vmag: f64,
    pub kv_base: f64,
    pub per_unit: f64,
    pub angle: f64,
    pub src_frequency: f64,
    /// Loadshape references by name (consumed in Phase 5).
    pub yearly_shape: String,
    pub daily_shape: String,
    pub duty_shape: String,
    pub spectrum: String,
}

impl VSource {
    /// Pascal `TVsourceObj.Create`.
    pub fn new(name: &str) -> Self {
        let mut cd = CktElementData::new(name, prop::NUM_PROPS);
        cd.nphases = 3;
        cd.nconds = 3;
        cd.set_nterms(2); // Now a 2-terminal device

        let kv_base = 115.0;
        let base_mva = 100.0;
        let mut vs = Self {
            cd,
            mva_sc3: 2000.0,
            mva_sc1: 2100.0,
            isc3: 10000.0,
            isc1: 10540.0,
            z_spec_type: 1, // default to MVAsc
            r1: 1.65,
            x1: 6.6,
            r2: 1.65,
            x2: 6.6,
            r0: 1.9,
            x0: 5.7,
            x1r1: 4.0,
            x0r0: 3.0,
            base_mva,
            pu_z1: Complex64::ZERO,
            pu_z0: Complex64::ZERO,
            pu_z2: Complex64::ZERO,
            pu_z_ideal: Complex64::new(1.0e-6, 0.001),
            z_base: kv_base * kv_base / base_mva,
            bus2_defined: false,
            z1_specified: false,
            pu_z1_specified: false,
            pu_z0_specified: false,
            pu_z2_specified: false,
            z2_specified: false,
            z0_specified: false,
            is_quasi_ideal: false,
            scan_type: 1,
            sequence_type: 1,
            z: None,
            zinv: None,
            vmag: 0.0,
            kv_base,
            per_unit: 1.0,
            angle: 0.0,
            src_frequency: 60.0, // BaseFrequency
            yearly_shape: String::new(),
            daily_shape: String::new(),
            duty_shape: String::new(),
            spectrum: "defaultvsource".to_string(),
        };
        // Property tracking defaults (NoPropertyTracking is off by default).
        vs.cd.obj.set_as_next_seq(prop::MVASC3);
        vs.cd.obj.set_as_next_seq(prop::MVASC1);
        vs.cd.obj.set_as_next_seq(prop::BASEKV);
        vs.recalc();
        vs
    }

    /// Pascal `TVsourceObj.RecalcElementData`.
    pub fn recalc(&mut self) {
        let nphases = self.cd.nphases;
        let mut z = CMatrix::new(nphases);

        let factor = if nphases == 1 { 1.0 } else { sqrt3() };

        // Pascal initializes Rs=0, Rm=0, Xs=0.1, Xm=0 before the case; every
        // branch overwrites all four before they reach the Z matrix, so the
        // bindings start uninitialized (`rs` is set twice on the Z-spec path,
        // once for Isc1 and once for the matrix, hence `mut`).
        let mut rs: f64;
        let rm: f64;
        let xs: f64;
        let xm: f64;

        // Calculate the short circuit impedance and make all other spec
        // types agree.
        match self.z_spec_type {
            1 | 2 => {
                if self.z_spec_type == 1 {
                    // MVAsc
                    self.x1 = self.kv_base.powi(2)
                        / self.mva_sc3
                        / (1.0 + 1.0 / self.x1r1.powi(2)).sqrt();
                    self.r1 = self.x1 / self.x1r1;
                    self.r2 = self.r1; // default Z2 = Z1
                    self.x2 = self.x1;
                    self.isc3 = self.mva_sc3 * 1000.0 / (sqrt3() * self.kv_base);
                    self.isc1 = self.mva_sc1 * 1000.0 / (factor * self.kv_base);
                } else {
                    // Isc
                    self.mva_sc3 = sqrt3() * self.kv_base * self.isc3 / 1000.0;
                    self.mva_sc1 = factor * self.kv_base * self.isc1 / 1000.0;
                    self.x1 = self.kv_base.powi(2)
                        / self.mva_sc3
                        / (1.0 + 1.0 / self.x1r1.powi(2)).sqrt();
                    self.r1 = self.x1 / self.x1r1;
                    self.r2 = self.r1;
                    self.x2 = self.x1;
                }

                // Compute R0, X0
                self.r0 = quad_solver(
                    1.0 + self.x0r0.powi(2),
                    4.0 * (self.r1 + self.x1 * self.x0r0),
                    4.0 * (self.r1 * self.r1 + self.x1 * self.x1)
                        - (3.0 * self.kv_base * 1000.0 / factor / self.isc1).powi(2),
                );
                // Pascal raises on NaN R0; the executive records the message.
                self.x0 = self.r0 * self.x0r0;

                // for Z matrix
                xs = (2.0 * self.x1 + self.x0) / 3.0;
                rs = (2.0 * self.r1 + self.r0) / 3.0;
                rm = (self.r0 - self.r1) / 3.0;
                xm = (self.x0 - self.x1) / 3.0;
            }
            _ => {
                // 3: Z1, Z2, Z0 specified.
                // Compute Z1, Z2, Z0 in ohms if Z1 is specified in pu.
                if self.pu_z1_specified {
                    self.r1 = self.pu_z1.re * self.z_base;
                    self.x1 = self.pu_z1.im * self.z_base;
                    self.r2 = self.pu_z2.re * self.z_base;
                    self.x2 = self.pu_z2.im * self.z_base;
                    self.r0 = self.pu_z0.re * self.z_base;
                    self.x0 = self.pu_z0.im * self.z_base;
                }
                // (R1 = X1 = 0 raises error 7340 in Pascal; executive checks.)

                // Compute equivalent Isc3, Isc1, MVAsc3, MVAsc1.
                self.isc3 =
                    self.kv_base * 1000.0 / sqrt3() / Complex64::new(self.r1, self.x1).norm();

                if nphases == 1 {
                    // Force Z0 and Z2 to be Z1 so Zs is same as Z1.
                    self.r0 = self.r1;
                    self.x0 = self.x1;
                    self.r2 = self.r1;
                    self.x2 = self.x1;
                }
                rs = (2.0 * self.r1 + self.r0) / 3.0;
                xs = (2.0 * self.x1 + self.x0) / 3.0;

                self.isc1 = self.kv_base * 1000.0 / factor / Complex64::new(rs, xs).norm();
                self.mva_sc3 = sqrt3() * self.kv_base * self.isc3 / 1000.0;
                self.mva_sc1 = factor * self.kv_base * self.isc1 / 1000.0;
                xm = xs - self.x1;

                rs = (2.0 * self.r1 + self.r0) / 3.0;
                rm = (self.r0 - self.r1) / 3.0;
            }
        }

        if (self.r1 == self.r2) && (self.x1 == self.x2) {
            // Symmetric matrix case.
            let zs = Complex64::new(rs, xs);
            let zm = Complex64::new(rm, xm);
            for i in 0..nphases {
                z.set(i, i, zs);
                for j in 0..i {
                    z.set(i, j, zm);
                    z.set(j, i, zm);
                }
            }
        } else {
            // Asymmetric matrix case where Z2 <> Z1.
            let z1 = Complex64::new(self.r1, self.x1);
            let z2 = Complex64::new(self.r2, self.x2);
            let z0 = Complex64::new(self.r0, self.x0);

            let value = (z2 + z1 + z0) / 3.0;
            for i in 0..nphases {
                z.set(i, i, value);
            }

            if nphases == 3 {
                // Calpha is 1∠−120; conjugate to agree with textbooks.
                let calpha1 = CALPHA.conj();
                let calpha2 = calpha1 * calpha1;
                let value2 = (calpha2 * z2 + calpha1 * z1 + z0) / 3.0;
                let value1 = (calpha2 * z1 + calpha1 * z2 + z0) / 3.0;
                // (0-based indices; Pascal sets the 1-based lower/upper triangle)
                z.set(1, 0, value1);
                z.set(2, 0, value2);
                z.set(2, 1, value1);
                z.set(0, 1, value2);
                z.set(0, 2, value1);
                z.set(1, 2, value2);
            }
        }

        // If not specified, compute a value for puZ1 for display.
        if !(self.pu_z1_specified || self.pu_z0_specified || self.pu_z2_specified)
            && self.z_base > 0.0
        {
            self.pu_z1 = Complex64::new(self.r1 / self.z_base, self.x1 / self.z_base);
            self.pu_z2 = Complex64::new(self.r2 / self.z_base, self.x2 / self.z_base);
            self.pu_z0 = Complex64::new(self.r0 / self.z_base, self.x0 / self.z_base);
        }

        self.vmag = get_vmag(self.kv_base, self.per_unit, nphases);

        self.z = Some(z);
        self.zinv = Some(CMatrix::new(nphases));
        self.cd.inj_current = vec![Complex64::ZERO; self.cd.yorder];
    }

    /// Pascal `GetVterminalForSource` (snapshot/non-harmonic path; loadshape
    /// modes arrive in Phase 5).
    fn get_vterminal_for_source(&mut self, sys: &SysCtx) {
        let nphases = self.cd.nphases;
        self.vmag = get_vmag(self.kv_base, self.per_unit, nphases);

        if (sys.frequency - self.src_frequency).abs() > EPSILON2 {
            self.vmag = 0.0; // Solution Frequency and Source Frequency don't match!
        }
        for i in 0..nphases {
            let deg = match self.sequence_type {
                -1 => 360.0 + self.angle + (i as f64) * 360.0 / nphases as f64, // neg seq
                0 => 360.0 + self.angle, // all the same for zero sequence
                _ => 360.0 + self.angle - (i as f64) * 360.0 / nphases as f64,
            };
            self.cd.vterminal[i] = pdeg_to_complex(self.vmag, deg);
            self.cd.vterminal[i + nphases] = Complex64::ZERO;
        }
    }

    /// Pascal `GetInjCurrents`: `[Iinj1; Iinj2] = [Yprim]·[Vsource; 0]`.
    fn get_inj_currents(&mut self, sys: &SysCtx) {
        self.get_vterminal_for_source(sys);
        if let Some(yprim) = &self.cd.yprim {
            yprim.mv_mult(&mut self.cd.inj_current, &self.cd.vterminal);
        }
        self.cd.iterminal_updated = false;
    }
}

/// Pascal `Vmag` computation (`RecalcElementData`/`GetVterminalForSource`):
/// 1-phase uses kV directly; polyphase divides by `2·sin(π/n)` (= √3 for 3
/// phases).
fn get_vmag(kv_base: f64, per_unit: f64, nphases: usize) -> f64 {
    if nphases == 1 {
        kv_base * per_unit * 1000.0
    } else {
        kv_base * per_unit * 1000.0
            / 2.0
            / ((180.0 / nphases as f64) * std::f64::consts::PI / 180.0).sin()
    }
}

impl CktElement for VSource {
    fn cd(&self) -> &CktElementData {
        &self.cd
    }
    fn cd_mut(&mut self) -> &mut CktElementData {
        &mut self.cd
    }

    fn recalc_element_data(&mut self, _sys: &SysCtx) {
        self.recalc();
    }

    /// Pascal `TVsourceObj.CalcYPrim`: build only YPrim_Series.
    fn calc_yprim(&mut self, sys: &SysCtx) {
        let nphases = self.cd.nphases;
        let yorder = self.cd.yorder;

        self.cd.yprim_freq = sys.frequency;
        let freq_multiplier = self.cd.yprim_freq / self.cd.base_frequency;

        let z = self.z.as_ref().expect("recalc ran in the constructor");
        let mut zinv = CMatrix::new(nphases);

        if ((freq_multiplier - 1.0) < EPSILON) && self.is_quasi_ideal && !sys.is_harmonic_model {
            // Ideal source approximation: diagonal impedance matrix only.
            let value = self.pu_z_ideal * self.z_base; // convert to ohms
            for i in 0..nphases {
                zinv.set(i, i, value);
            }
        } else {
            // Normal Thevenin source: series RL adjusted for frequency.
            for i in 0..nphases {
                for j in 0..nphases {
                    let mut value = z.get(i, j);
                    value.im *= freq_multiplier;
                    zinv.set(i, j, value);
                }
            }
        }

        if zinv.invert().is_err() {
            // Pascal error 325: put in large series conductance.
            zinv.clear();
            for i in 0..nphases {
                zinv.set(i, i, Complex64::new(1.0 / EPSILON, 0.0));
            }
        }

        let mut yp_series = CMatrix::new(yorder);
        for i in 0..nphases {
            for j in 0..nphases {
                let value = zinv.get(i, j);
                yp_series.set(i, j, value);
                yp_series.set(i + nphases, j + nphases, value);
                yp_series.set(i, j + nphases, -value);
                yp_series.set(i + nphases, j, -value);
            }
        }

        let mut yprim = CMatrix::new(yorder);
        yprim.copy_from(&yp_series);
        self.cd.yprim_series = Some(yp_series);
        self.cd.yprim_shunt = None;
        self.cd.yprim = Some(yprim);
        self.zinv = Some(zinv);

        // Account for open conductors.
        self.cd.apply_yprim_open_conductor_calcs();
        self.cd.yprim_invalid = false;
    }

    /// Pascal `TVsourceObj.InjCurrents` + `TPCElement.InjCurrents`.
    fn inj_currents(&mut self, sys: &SysCtx, ctx: &mut InjCtx) {
        self.get_inj_currents(sys);
        for i in 0..self.cd.yorder {
            ctx.currents[self.cd.node_ref[i]] += self.cd.inj_current[i];
        }
    }

    /// Pascal `TVsourceObj.GetCurrents`: `Yprim·V(node) − InjCurrent`.
    #[allow(clippy::needless_range_loop)] // loop-for-loop Pascal port
    fn get_currents(&mut self, sys: &SysCtx, node_v: &[Complex64], curr: &mut [Complex64]) {
        let yorder = self.cd.yorder;
        for i in 0..yorder {
            self.cd.vterminal[i] = node_v[self.cd.node_ref[i]];
        }
        if let Some(yprim) = &self.cd.yprim {
            yprim.mv_mult(curr, &self.cd.vterminal);
        }
        self.get_inj_currents(sys); // overwrites Vterminal, like the original
        for i in 0..yorder {
            curr[i] -= self.cd.inj_current[i];
        }
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
    fn as_ckt_element(&self) -> Option<&dyn CktElement> {
        Some(self)
    }
    fn as_ckt_element_mut(&mut self) -> Option<&mut dyn CktElement> {
        Some(self)
    }

    fn get_f64(&self, idx: usize) -> f64 {
        use prop::*;
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
        use prop::*;
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
        use prop::*;
        match idx {
            PHASES => self.cd.nphases as i32,
            SCAN_TYPE => self.scan_type,
            SEQUENCE => self.sequence_type,
            MODEL => self.is_quasi_ideal as i32,
            _ => unreachable!("Vsource has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        use prop::*;
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
            prop::ENABLED => self.cd.enabled,
            _ => unreachable!("Vsource has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        match idx {
            prop::ENABLED => self.cd.set_enabled(value),
            _ => unreachable!("Vsource has no boolean property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        use prop::*;
        match idx {
            YEARLY => self.yearly_shape.clone(),
            DAILY => self.daily_shape.clone(),
            DUTY => self.duty_shape.clone(),
            SPECTRUM => self.spectrum.clone(),
            _ => unreachable!("Vsource has no string property {idx}"),
        }
    }
    fn set_string(&mut self, idx: usize, value: String) {
        use prop::*;
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

    fn get_complex(&self, idx: usize) -> (f64, f64) {
        use prop::*;
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
        use prop::*;
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
        use prop::*;
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
            DAILY if self.yearly_shape.is_empty() => {
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
    fn end_edit(&mut self) {
        self.recalc();
        self.cd.yprim_invalid = true;
    }

    /// Pascal `TVsourceObj.MakeLike`.
    fn make_like(&mut self, other: &dyn DssObject) {
        let Some(other) = other.as_any().downcast_ref::<VSource>() else {
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
        self.cd.inj_current = vec![Complex64::ZERO; self.cd.yorder];
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}
