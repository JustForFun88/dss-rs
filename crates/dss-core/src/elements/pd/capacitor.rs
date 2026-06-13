//! Port of `PDElements/Capacitor.pas` — `TCapacitorObj`, a two-terminal
//! constant-impedance shunt (or series) capacitor bank. The bank may have
//! several switchable steps (`NumSteps`/`States`); each energized step stamps a
//! capacitive admittance (optionally with a series filter `R`+`XL`) into `YPrim`.
//!
//! Capacitance is specified one of three ways (`SpecType`): `kvar`+`kV`,
//! `Cuf` (µF per phase), or a nodal `CMatrix` (µF). Bus2 defaults to the
//! grounded-zero node of Bus1, giving a shunt bank; specifying Bus2 with
//! matching nodes makes a series capacitor.
//!
//! The harmonic-filter recomputation (`Harm`) and `MakePosSequence` are ported
//! for fidelity; the harmonic *solution* itself is Phase 7.

use num_complex::Complex64;

use crate::elements::ckt::CktElementData;
use crate::elements::traits::{CktElement, SysCtx};
use crate::obj::base::{DssObjData, DssObject};
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};
use crate::support::cmatrix::CMatrix;
use crate::util::sqrt3;

/// 1-based property ordinals (Pascal `TCapacitorProp` + class tails).
pub mod prop {
    pub const BUS1: usize = 1;
    pub const BUS2: usize = 2;
    pub const PHASES: usize = 3;
    pub const KVAR: usize = 4;
    pub const KV: usize = 5;
    pub const CONN: usize = 6;
    pub const CMATRIX: usize = 7;
    pub const CUF: usize = 8;
    pub const R: usize = 9;
    pub const XL: usize = 10;
    pub const HARM: usize = 11;
    pub const NUMSTEPS: usize = 12;
    pub const STATES: usize = 13;
    // TPDClass tail:
    pub const NORMAMPS: usize = 14;
    pub const EMERGAMPS: usize = 15;
    pub const FAULTRATE: usize = 16;
    pub const PCTPERM: usize = 17;
    pub const REPAIR: usize = 18;
    // TCktElementClass tail:
    pub const BASE_FREQ: usize = 19;
    pub const ENABLED: usize = 20;
    pub const NUM_PROPS: usize = 21; // incl. Like
}

/// `TCapacitor.DefineProperties`.
pub fn class_props(enums: &EnumRegistry) -> ClassProps {
    use prop::*;
    let defs = vec![
        // Pascal flags bus1 `Required` (inert here — not enforced in Phase 4).
        PropDef::bus("bus1", 1),
        PropDef::bus("bus2", 2),
        PropDef::integer("phases").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::double_array("kvar", NUMSTEPS).flags(PropFlags::REQUIRED_IN_SPEC_SET),
        PropDef::double("kv").flags(PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::NON_NEGATIVE),
        PropDef::mapped_string_enum("conn", enums.connection),
        PropDef::double_sym_matrix("cmatrix", PHASES)
            .scale(1.0e-6)
            .flags(PropFlags::REQUIRED_IN_SPEC_SET),
        PropDef::double_array("cuf", NUMSTEPS)
            .scale(1.0e-6)
            .flags(PropFlags::REQUIRED_IN_SPEC_SET),
        PropDef::double_array("R", NUMSTEPS),
        PropDef::double_array("XL", NUMSTEPS),
        PropDef::double_array("Harm", NUMSTEPS),
        PropDef::integer("NumSteps")
            .flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO | PropFlags::SUPPRESS_JSON),
        PropDef::int_array("states", NUMSTEPS),
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
    ClassProps::new("Capacitor", defs, true)
}

/// `TCapacitorObj`.
#[derive(Debug, Clone)]
pub struct Capacitor {
    pub cd: CktElementData,
    /// Per-step capacitance (farads), filter reactance, kvar rating, filter
    /// resistance, tuning harmonic and on/off state (Pascal `FC`, `FXL`,
    /// `Fkvarrating`, `FR`, `FHarm`, `FStates`).
    fc: Vec<f64>,
    fxl: Vec<f64>,
    fkvarrating: Vec<f64>,
    fr: Vec<f64>,
    fharm: Vec<f64>,
    fstates: Vec<i32>,
    ftotalkvar: f64,
    kvrating: f64,
    fnumsteps: i32,
    flast_step_in_service: i32,
    /// Nodal capacitance matrix (µF stored as farads, row-major `nphases²`);
    /// `None` unless `SpecType = 3`.
    cmatrix: Option<Vec<f64>>,
    do_harmonic_recalc: bool,
    bus2_defined: bool,
    /// 1 = kvar+kV, 2 = Cuf+kV, 3 = CMatrix.
    spec_type: i32,
    num_term: i32,
    is_shunt: bool,
    connection: i32,
    // PD-element common:
    norm_amps: f64,
    emerg_amps: f64,
    norm_amps_specified: bool,
    emerg_amps_specified: bool,
    fault_rate: f64,
    pct_perm: f64,
    hrs_to_repair: f64,
}

impl Capacitor {
    /// Per-step switch states (`States[1..NumSteps]`, 0=open/1=closed). Read by
    /// mode-6 monitors (`Meters/Monitor.pas` TakeSample).
    pub fn states(&self) -> &[i32] {
        &self.fstates
    }

    /// Pascal `TCapacitorObj.Create`.
    pub fn new(name: &str) -> Self {
        let mut cd = CktElementData::new(name, prop::NUM_PROPS);
        cd.nphases = 3;
        cd.nconds = 3;
        cd.set_nterms(2); // forces allocation of terminals/conductors + buses

        // Default Bus2 to the grounded-zero node of Bus1 (`Bus1.0.0.0`).
        let bus1 = cd.get_bus(1).to_string();
        cd.set_bus(2, &format!("{bus1}.0.0.0"));

        let kvrating = 12.47;
        let two_pi = 2.0 * std::f64::consts::PI;
        let base_freq = cd.base_frequency;
        let fkvar = 1200.0;
        // FC default: InitDblArray(1, FC, 1/(TwoPi*BaseFreq*SQR(kv)*1000/kvar)).
        let fc0 = 1.0 / (two_pi * base_freq * kvrating * kvrating * 1000.0 / fkvar);

        let mut c = Self {
            cd,
            fc: vec![fc0],
            fxl: vec![0.0],
            fkvarrating: vec![fkvar],
            fr: vec![0.0],
            fharm: vec![0.0],
            fstates: vec![1],
            ftotalkvar: 0.0,
            kvrating,
            fnumsteps: 1,
            flast_step_in_service: 1,
            cmatrix: None,
            do_harmonic_recalc: false,
            bus2_defined: false,
            spec_type: 1, // kvar
            num_term: 1,
            is_shunt: true,
            connection: 0,                                // wye
            norm_amps: fkvar * sqrt3() / kvrating * 1.35, // 135%
            emerg_amps: 0.0,
            norm_amps_specified: false,
            emerg_amps_specified: false,
            fault_rate: 0.0005,
            pct_perm: 100.0,
            hrs_to_repair: 3.0,
        };
        c.emerg_amps = c.norm_amps * 1.8 / 1.35; // 180%
        c.cd.yorder = c.cd.nterms * c.cd.nconds;
        c.recalc();
        c
    }

    /// Number of steps as `usize`.
    fn n_steps(&self) -> usize {
        self.fnumsteps.max(0) as usize
    }

    /// Pascal `set_NumSteps`: programmatic setter (ctor/MakeLike). Property edits
    /// write `FNumSteps` directly then call the side effect; this guards against
    /// no-op/invalid values, mirroring the Pascal property setter.
    fn set_num_steps(&mut self, value: i32) {
        if value <= 0 || self.fnumsteps == value {
            return;
        }
        let prev = self.fnumsteps;
        self.fnumsteps = value;
        self.side_effect_numsteps(prev);
    }

    /// Pascal `PropertySideEffects(numsteps)`: reallocate the per-step arrays;
    /// when growing a single-step bank into a multi-step one, split the ratings
    /// to keep the same net size and energize every step.
    fn side_effect_numsteps(&mut self, prev_int: i32) {
        let n = self.n_steps();
        let mut rstep = 0.0;
        let mut xlstep = 0.0;
        if prev_int == 1 {
            self.ftotalkvar = self.fkvarrating[0];
            rstep = self.fr[0] * self.fnumsteps as f64;
            xlstep = self.fxl[0] * self.fnumsteps as f64;
        }

        // Reallocate arrays (preserve element 0; new slots zero-filled).
        self.fc.resize(n, 0.0);
        self.fxl.resize(n, 0.0);
        self.fkvarrating.resize(n, 0.0);
        self.fr.resize(n, 0.0);
        self.fharm.resize(n, 0.0);
        self.fstates.resize(n, 0);

        if prev_int == 1 {
            match self.spec_type {
                1 => {
                    let step_size = self.ftotalkvar / self.fnumsteps as f64;
                    for v in self.fkvarrating.iter_mut() {
                        *v = step_size;
                    }
                }
                2 => {
                    let c0 = self.fc[0];
                    for v in self.fc.iter_mut().skip(1) {
                        *v = c0;
                    }
                }
                _ => {}
            }
            match self.spec_type {
                1 => {
                    for v in self.fr.iter_mut() {
                        *v = rstep;
                    }
                    for v in self.fxl.iter_mut() {
                        *v = xlstep;
                    }
                }
                2 | 3 => {
                    let (r0, xl0) = (self.fr[0], self.fxl[0]);
                    for v in self.fr.iter_mut().skip(1) {
                        *v = r0;
                    }
                    for v in self.fxl.iter_mut().skip(1) {
                        *v = xl0;
                    }
                }
                _ => {}
            }
            for v in self.fstates.iter_mut() {
                *v = 1; // turn 'em all ON
            }
            self.set_last_step_in_service(self.fnumsteps);
            let h0 = self.fharm[0];
            for v in self.fharm.iter_mut().skip(1) {
                *v = h0; // tune 'em all the same as the first
            }
        }
    }

    /// Pascal `FindLastStepInService`: the highest energized step.
    fn find_last_step_in_service(&mut self) {
        self.flast_step_in_service = 0;
        for i in (1..=self.n_steps()).rev() {
            if self.fstates[i - 1] == 1 {
                self.flast_step_in_service = i as i32;
                break;
            }
        }
    }

    /// Pascal `set_LastStepInService`: energize steps `1..=value`, open the rest.
    fn set_last_step_in_service(&mut self, value: i32) {
        let n = self.n_steps();
        let v = value.clamp(0, n as i32) as usize;
        for i in 0..v {
            self.fstates[i] = 1;
        }
        for i in v..n {
            self.fstates[i] = 0;
        }
        if value != self.flast_step_in_service {
            self.cd.yprim_invalid = true;
        }
        self.flast_step_in_service = value;
    }

    /// Pascal `set_States(Idx, Value)` (1-based `idx`): set step `idx` on/off,
    /// invalidating `YPrim` only when the state actually changes (the non-
    /// incremental-Y path).
    fn set_states(&mut self, idx: usize, value: i32) {
        if self.fstates[idx - 1] != value {
            self.fstates[idx - 1] = value;
            self.cd.yprim_invalid = true;
        }
    }

    /// Pascal `TCapacitorObj.AddStep`: energize the next step (starting from the
    /// last step in service); `false` if all steps are already in.
    fn add_step(&mut self) -> bool {
        if self.flast_step_in_service == self.fnumsteps {
            false
        } else {
            self.flast_step_in_service += 1;
            self.set_states(self.flast_step_in_service as usize, 1);
            true
        }
    }

    /// Pascal `TCapacitorObj.SubtractStep`: de-energize the highest step; returns
    /// `false` once the bank is fully open (signals "bank OPEN").
    fn subtract_step(&mut self) -> bool {
        if self.flast_step_in_service == 0 {
            false
        } else {
            self.set_states(self.flast_step_in_service as usize, 0);
            self.flast_step_in_service -= 1;
            self.flast_step_in_service != 0
        }
    }

    /// Pascal `TCapacitorObj.AvailableSteps`.
    fn available_steps(&self) -> i32 {
        self.fnumsteps - self.flast_step_in_service
    }

    /// Pascal `Closed[0]` getter on terminal 1 (`TDSSCktElement.Get_ConductorClosed(0)`):
    /// true iff every phase conductor of terminal 1 is closed.
    fn terminal1_closed(&self) -> bool {
        let t = &self.cd.terminals[0];
        (0..self.cd.nphases).all(|i| t.conductors_closed[i])
    }

    /// Pascal `Closed[0] := value` on terminal 1
    /// (`TDSSCktElement.Set_ConductorClosed(0, value)`): open/close all phase
    /// conductors of terminal 1 and invalidate `YPrim`.
    fn set_terminal1_closed(&mut self, value: bool) {
        let nph = self.cd.nphases;
        let t = &mut self.cd.terminals[0];
        for i in 0..nph {
            t.conductors_closed[i] = value;
        }
        self.cd.yprim_invalid = true;
    }

    /// Pascal `RecalcElementData`: derive `FC`/`FTotalkvar` from the spec, run
    /// the optional harmonic-filter recomputation, and (unless overridden) the
    /// default Norm/Emerg current ratings.
    fn recalc(&mut self) {
        let two_pi = 2.0 * std::f64::consts::PI;
        let w = two_pi * self.cd.base_frequency;
        let nphases = self.cd.nphases as f64;
        let n = self.n_steps();
        self.ftotalkvar = 0.0;
        let mut phase_kv = 1.0;

        match self.spec_type {
            1 => {
                // kvar
                phase_kv = self.phase_kv();
                let fc = 1.0 / (w * phase_kv * phase_kv * 1000.0 / (self.fkvarrating[0] / nphases));
                for v in self.fc.iter_mut() {
                    *v = fc;
                }
                for &k in self.fkvarrating.iter().take(n) {
                    self.ftotalkvar += k;
                }
            }
            2 => {
                // Cuf
                phase_kv = self.phase_kv();
                for &c in self.fc.iter().take(n) {
                    self.ftotalkvar += w * c * phase_kv * phase_kv / 1000.0;
                }
            }
            _ => {} // CMatrix: nothing to do
        }

        if self.do_harmonic_recalc {
            for i in 0..n {
                self.fxl[i] = if self.fharm[i] != 0.0 {
                    (1.0 / (w * self.fc[i])) / (self.fharm[i] * self.fharm[i])
                } else {
                    0.0 // 0 harmonic means no filter
                };
                if self.fr[i] == 0.0 {
                    self.fr[i] = self.fxl[i] / 1000.0;
                }
            }
        }

        let kvar_per_phase = self.ftotalkvar / nphases;
        if !self.norm_amps_specified {
            self.norm_amps = kvar_per_phase / phase_kv * 1.35;
        }
        if !self.emerg_amps_specified {
            self.emerg_amps = kvar_per_phase / phase_kv * 1.8;
        }
    }

    /// Pascal per-phase voltage selection (`RecalcElementData`): delta uses the
    /// can rating; wye assumes a three-phase line-line rating for 2/3 phases.
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

    /// Pascal `MakeYprimWork`: build one energized step's admittance into
    /// `ywork`. The matrix is *reused across steps without clearing* (faithful to
    /// the Pascal: wye/cmatrix overwrite their positions; delta accumulates).
    fn make_yprim_work(&self, ywork: &mut CMatrix, istep: usize, freq: f64) {
        let two_pi = 2.0 * std::f64::consts::PI;
        let freq_multiple = freq / self.cd.base_frequency;
        let w = two_pi * freq;
        let i_step = istep - 1;
        let nphases = self.cd.nphases;
        let nconds = self.cd.nconds;

        let has_zl = (self.fr[i_step] + self.fxl[i_step].abs()) > 0.0;
        let zl = Complex64::new(self.fr[i_step], self.fxl[i_step] * freq_multiple);

        match self.spec_type {
            1 | 2 => {
                let mut value = Complex64::new(0.0, self.fc[i_step] * w);
                if self.connection == 1 {
                    // Delta (line-line); AddElement accumulates.
                    let value2 = -value;
                    for i in 1..=nphases {
                        let mut j = i + 1;
                        if j > nconds {
                            j = 1;
                        }
                        ywork.add(i - 1, i - 1, value);
                        ywork.add(j - 1, j - 1, value);
                        ywork.add(i - 1, j - 1, value2);
                        ywork.add(j - 1, i - 1, value2);
                    }
                } else {
                    // Wye; assignment overwrites.
                    if has_zl {
                        value = (zl + value.inv()).inv(); // add in ZL
                    }
                    let value2 = -value;
                    for i in 1..=nphases {
                        let j = i + nphases;
                        ywork.set(i - 1, i - 1, value);
                        ywork.set(j - 1, j - 1, value);
                        ywork.set(i - 1, j - 1, value2);
                        ywork.set(j - 1, i - 1, value2);
                    }
                }
            }
            _ => {
                // CMatrix.
                let cm = self.cmatrix.as_ref().expect("SpecType 3 has a CMatrix");
                for i in 1..=nphases {
                    let ioffset = (i - 1) * nphases;
                    for j in 1..=nphases {
                        let value = Complex64::new(0.0, cm[ioffset + (j - 1)] * w);
                        ywork.set(i - 1, j - 1, value);
                        ywork.set(i - 1 + nphases, j - 1 + nphases, value);
                        let nvalue = -value;
                        ywork.set(i - 1, j - 1 + nphases, nvalue);
                        ywork.set(j - 1 + nphases, i - 1, nvalue);
                    }
                }
            }
        }

        // Add the filter reactance, if any.
        if !has_zl {
            return;
        }
        match self.spec_type {
            1 | 2 => {
                if self.connection == 1 {
                    // Delta: invert, add ZL in series on the diagonal, re-invert.
                    for i in 1..=nphases {
                        let d = ywork.get(i - 1, i - 1) * 1.000001;
                        ywork.set(i - 1, i - 1, d);
                    }
                    let _ = ywork.invert();
                    for i in 1..=nphases {
                        let v = zl + ywork.get(i - 1, i - 1);
                        ywork.set(i - 1, i - 1, v);
                    }
                    let _ = ywork.invert();
                }
                // Wye: ZL already folded into `value` above.
            }
            _ => {
                let _ = ywork.invert();
                for i in 1..=nphases {
                    let v = zl + ywork.get(i - 1, i - 1);
                    ywork.set(i - 1, i - 1, v);
                }
                let _ = ywork.invert();
            }
        }
    }
}

impl CktElement for Capacitor {
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

    /// Pascal `TPDElement.IsShunt` (set by the Bus1/Bus2 side effects).
    fn is_shunt(&self) -> bool {
        self.is_shunt
    }

    /// Pascal `TCapacitorObj.CalcYPrim`: accumulate every energized step into the
    /// shunt (or series) primitive, then mirror tiny diagonals into the other
    /// matrix so `CalcVoltages` never sees an all-zero row.
    fn calc_yprim(&mut self, sys: &SysCtx) {
        let yorder = self.cd.yorder;
        self.cd.yprim_freq = sys.frequency;

        let mut yp_series = CMatrix::new(yorder);
        let mut yp_shunt = CMatrix::new(yorder);
        let mut ywork = CMatrix::new(yorder);

        {
            let temp = if self.is_shunt {
                &mut yp_shunt
            } else {
                &mut yp_series
            };
            for step in 1..=self.n_steps() {
                if self.fstates[step - 1] == 1 {
                    self.make_yprim_work(&mut ywork, step, sys.frequency);
                    temp.add_from(&ywork);
                }
            }
        }

        // Set YPrim_Series from the shunt diagonals so CalcVoltages doesn't fail.
        if self.is_shunt {
            for i in 0..yorder {
                yp_series.set(i, i, yp_shunt.get(i, i) * 1.0e-10);
            }
        }

        let mut yprim = CMatrix::new(yorder);
        yprim.copy_from(if self.is_shunt { &yp_shunt } else { &yp_series });

        self.cd.yprim_series = Some(yp_series);
        self.cd.yprim_shunt = Some(yp_shunt);
        self.cd.yprim = Some(yprim);

        self.cd.apply_yprim_open_conductor_calcs();
        self.cd.yprim_invalid = false;
    }
}

/// The controlled-capacitor surface CapControl's `Sample`/`DoPendingAction`
/// read and mutate (Pascal `TCapacitorObj` switching methods). A trait so the
/// CapControl switching logic is unit-testable against a lightweight mock;
/// [`Capacitor`] is the production implementor. Step indices are 1-based as in
/// Pascal.
pub trait ControlledCapacitor {
    /// `ControlledElement.FullName` for the event log (`Capacitor.<name>`).
    fn full_name(&self) -> String;
    /// `NumSteps`.
    fn num_steps(&self) -> i32;
    /// `AvailableSteps` (`NumSteps − LastStepInService`).
    fn available_steps(&self) -> i32;
    /// `Totalkvar` of the bank.
    fn total_kvar(&self) -> f64;
    /// `Connection` (0 = wye, 1 = delta) — selects the L-L voltage for control.
    fn connection(&self) -> i32;
    /// `Closed[0]`: every phase of terminal 1 closed.
    fn is_closed(&self) -> bool;
    /// `Closed[0] := value`: open/close all phases of terminal 1 (invalidates Y).
    fn set_closed(&mut self, value: bool);
    /// `AddStep`: energize the next step; `false` if all steps already in.
    fn add_step(&mut self) -> bool;
    /// `SubtractStep`: de-energize the highest step; `false` once fully open.
    fn subtract_step(&mut self) -> bool;
}

impl ControlledCapacitor for Capacitor {
    fn full_name(&self) -> String {
        format!("Capacitor.{}", self.cd.obj.name())
    }
    fn num_steps(&self) -> i32 {
        self.fnumsteps
    }
    fn available_steps(&self) -> i32 {
        Capacitor::available_steps(self)
    }
    fn total_kvar(&self) -> f64 {
        self.ftotalkvar
    }
    fn connection(&self) -> i32 {
        self.connection
    }
    fn is_closed(&self) -> bool {
        self.terminal1_closed()
    }
    fn set_closed(&mut self, value: bool) {
        self.set_terminal1_closed(value);
    }
    fn add_step(&mut self) -> bool {
        Capacitor::add_step(self)
    }
    fn subtract_step(&mut self) -> bool {
        Capacitor::subtract_step(self)
    }
}

impl DssObject for Capacitor {
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
            KV => self.kvrating,
            NORMAMPS => self.norm_amps,
            EMERGAMPS => self.emerg_amps,
            FAULTRATE => self.fault_rate,
            PCTPERM => self.pct_perm,
            REPAIR => self.hrs_to_repair,
            BASE_FREQ => self.cd.base_frequency,
            _ => unreachable!("Capacitor has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        use prop::*;
        match idx {
            KV => self.kvrating = value,
            NORMAMPS => self.norm_amps = value,
            EMERGAMPS => self.emerg_amps = value,
            FAULTRATE => self.fault_rate = value,
            PCTPERM => self.pct_perm = value,
            REPAIR => self.hrs_to_repair = value,
            BASE_FREQ => self.cd.base_frequency = value,
            _ => unreachable!("Capacitor has no double property {idx}"),
        }
    }

    fn get_i32(&self, idx: usize) -> i32 {
        use prop::*;
        match idx {
            PHASES => self.cd.nphases as i32,
            CONN => self.connection,
            NUMSTEPS => self.fnumsteps,
            _ => unreachable!("Capacitor has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        use prop::*;
        match idx {
            PHASES => self.cd.nphases = value.max(0) as usize,
            CONN => self.connection = value,
            NUMSTEPS => self.fnumsteps = value,
            _ => unreachable!("Capacitor has no integer property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        match idx {
            prop::ENABLED => self.cd.enabled,
            _ => unreachable!("Capacitor has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        match idx {
            prop::ENABLED => self.cd.set_enabled(value),
            _ => unreachable!("Capacitor has no boolean property {idx}"),
        }
    }

    fn get_f64_array(&self, idx: usize) -> Option<&[f64]> {
        use prop::*;
        match idx {
            KVAR => Some(&self.fkvarrating),
            CUF => Some(&self.fc),
            R => Some(&self.fr),
            XL => Some(&self.fxl),
            HARM => Some(&self.fharm),
            CMATRIX => self.cmatrix.as_deref(),
            _ => unreachable!("Capacitor has no double-array property {idx}"),
        }
    }
    fn set_f64_array(&mut self, idx: usize, value: Vec<f64>) {
        use prop::*;
        match idx {
            KVAR => self.fkvarrating = value,
            CUF => self.fc = value,
            R => self.fr = value,
            XL => self.fxl = value,
            HARM => self.fharm = value,
            CMATRIX => self.cmatrix = Some(value),
            _ => unreachable!("Capacitor has no double-array property {idx}"),
        }
    }

    fn get_i32_array(&self, idx: usize) -> Option<&[i32]> {
        match idx {
            prop::STATES => Some(&self.fstates),
            _ => unreachable!("Capacitor has no integer-array property {idx}"),
        }
    }
    fn set_i32_array(&mut self, idx: usize, value: Vec<i32>) {
        match idx {
            prop::STATES => self.fstates = value,
            _ => unreachable!("Capacitor has no integer-array property {idx}"),
        }
    }

    fn set_bus_name(&mut self, terminal: usize, value: &str) {
        self.cd.set_bus(terminal, value);
    }
    fn get_bus_name(&self, terminal: usize) -> String {
        self.cd.get_bus(terminal).to_string()
    }

    /// Pascal `TCapacitorObj.PropertySideEffects`.
    fn side_effects(&mut self, idx: usize, prev_int: i32) {
        use prop::*;
        match idx {
            BUS1 => {
                // Default Bus2 to the grounded-zero node of Bus1 (wye shunt) if
                // Bus2 has not been explicitly set.
                if !self.bus2_defined && self.cd.nterms == 2 {
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
                    self.cd.obj.clear_seq(BUS2); // reset for the save function
                }
            }
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
            BUS2 => {
                self.num_term = 2;
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
            CMATRIX => self.spec_type = 3,
            CUF => self.spec_type = 2,
            NUMSTEPS => self.side_effect_numsteps(prev_int),
            XL => {
                for i in 0..self.n_steps() {
                    if self.fxl[i] != 0.0 && self.fr[i] == 0.0 {
                        self.fr[i] = self.fxl[i].abs() / 1000.0;
                    }
                }
                self.do_harmonic_recalc = false;
            }
            HARM => self.do_harmonic_recalc = true,
            STATES => self.find_last_step_in_service(),
            NORMAMPS => self.norm_amps_specified = true,
            EMERGAMPS => self.emerg_amps_specified = true,
            _ => {}
        }

        // YPrim invalidation on anything that changes the impedance values.
        if matches!(
            idx,
            PHASES | KVAR | KV | CONN | CMATRIX | CUF | NUMSTEPS | STATES
        ) {
            self.cd.yprim_invalid = true;
        }
    }

    /// Pascal base `EndEdit` → `RecalcElementData` (Capacitor does not override).
    fn end_edit(&mut self) {
        self.recalc();
    }

    /// Pascal `TCapacitorObj.MakeLike`.
    fn make_like(&mut self, other: &dyn DssObject) {
        let Some(other) = other.as_any().downcast_ref::<Capacitor>() else {
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

        self.set_num_steps(other.fnumsteps);
        let n = self.n_steps();
        self.fc[..n].copy_from_slice(&other.fc[..n]);
        self.fkvarrating[..n].copy_from_slice(&other.fkvarrating[..n]);
        self.fr[..n].copy_from_slice(&other.fr[..n]);
        self.fxl[..n].copy_from_slice(&other.fxl[..n]);
        self.fharm[..n].copy_from_slice(&other.fharm[..n]);
        self.fstates[..n].copy_from_slice(&other.fstates[..n]);

        self.kvrating = other.kvrating;
        self.connection = other.connection;
        self.spec_type = other.spec_type;
        self.cmatrix = other.cmatrix.clone();

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
    use crate::elements::traits::SysCtx;
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
        let c = Capacitor::new("c1");
        assert_eq!(c.cd.nphases, 3);
        assert_eq!(c.cd.nconds, 3);
        assert_eq!(c.cd.nterms, 2);
        assert_eq!(c.cd.yorder, 6);
        assert!(c.is_shunt);
        assert_eq!(c.spec_type, 1);
        // Bus2 defaulted to the grounded node of the auto-named Bus1.
        assert_eq!(c.get_bus_name(2), "c1_1.0.0.0");
    }

    /// YPrim of a 3-phase wye 600 kvar @ 4.16 kV bank: every step diagonal is
    /// `j·b` with `b = 0.034670858` (probed in dss-python), wye 2-terminal
    /// stamping. Oracle: `Yprim[i,i] = +jb`, `Yprim[i,i+3] = -jb`.
    #[test]
    fn yprim_3ph_wye_kvar_matches_oracle() {
        let mut c = Capacitor::new("cap");
        c.cd.nphases = 3;
        c.cd.nconds = 3;
        c.cd.set_nterms(2);
        c.cd.yorder = 6;
        c.connection = 0;
        c.is_shunt = true;
        c.spec_type = 1;
        c.kvrating = 4.16;
        c.fkvarrating = vec![600.0];
        c.fc = vec![0.0];
        c.fr = vec![0.0];
        c.fxl = vec![0.0];
        c.fharm = vec![0.0];
        c.fstates = vec![1];
        c.fnumsteps = 1;
        c.recalc();

        c.calc_yprim(&test_sys());
        let yp = c.cd.yprim.as_ref().unwrap();
        let b = 0.034670858_f64;
        for i in 0..3 {
            let d = yp.get(i, i);
            assert!((d.re).abs() < 1e-9, "diag re {}", d.re);
            assert!((d.im - b).abs() < 1e-5, "diag im {} vs {b}", d.im);
            let off = yp.get(i, i + 3);
            assert!((off.im + b).abs() < 1e-5, "off im {} vs {}", off.im, -b);
        }
    }

    /// 1-phase wye 100 kvar @ 2.4 kV: `b = 0.017361111` (probed in dss-python).
    #[test]
    fn yprim_1ph_wye_kvar_matches_oracle() {
        let mut c = Capacitor::new("cap");
        c.cd.nphases = 1;
        c.cd.nconds = 1;
        c.cd.set_nterms(2);
        c.cd.yorder = 2;
        c.connection = 0;
        c.is_shunt = true;
        c.spec_type = 1;
        c.kvrating = 2.4;
        c.fkvarrating = vec![100.0];
        c.fc = vec![0.0];
        c.fr = vec![0.0];
        c.fxl = vec![0.0];
        c.fharm = vec![0.0];
        c.fstates = vec![1];
        c.fnumsteps = 1;
        c.recalc();

        c.calc_yprim(&test_sys());
        let yp = c.cd.yprim.as_ref().unwrap();
        let b = 0.017361111_f64;
        assert!((yp.get(0, 0).im - b).abs() < 1e-6);
        assert!((yp.get(0, 1).im + b).abs() < 1e-6);
        assert!((yp.get(1, 1).im - b).abs() < 1e-6);
    }

    /// YPrim of a 3-phase wye `CMatrix` bank (`SpecType = 3`):
    /// `cmatrix=(1.5 | -0.3 1.5 | -0.3 -0.3 1.5)` µF. Probed in dss-python:
    /// diagonal `j·w·1.5e-6 = j5.6548668e-4`, in-block off-diagonal
    /// `j·w·(-0.3e-6) = -j1.1309734e-4`, cross-block negated.
    #[test]
    fn yprim_3ph_cmatrix_matches_oracle() {
        let mut c = Capacitor::new("cap");
        c.cd.nphases = 3;
        c.cd.nconds = 3;
        c.cd.set_nterms(2);
        c.cd.yorder = 6;
        c.connection = 0;
        c.is_shunt = true;
        c.spec_type = 3;
        // Row-major nphases² in farads (the parse scales µF by 1e-6).
        let m = 1.0e-6;
        c.cmatrix = Some(vec![
            1.5 * m,
            -0.3 * m,
            -0.3 * m,
            -0.3 * m,
            1.5 * m,
            -0.3 * m,
            -0.3 * m,
            -0.3 * m,
            1.5 * m,
        ]);
        c.fr = vec![0.0];
        c.fxl = vec![0.0];
        c.fharm = vec![0.0];
        c.fstates = vec![1];
        c.fnumsteps = 1;
        c.recalc();

        c.calc_yprim(&test_sys());
        let yp = c.cd.yprim.as_ref().unwrap();
        let diag = 5.6548668e-4_f64;
        let off = 1.1309734e-4_f64;
        for i in 0..3 {
            assert!(
                (yp.get(i, i).im - diag).abs() < 1e-9,
                "diag {}",
                yp.get(i, i).im
            );
            assert!((yp.get(i, i + 3).im + diag).abs() < 1e-9);
            for j in 0..3 {
                if i != j {
                    assert!(
                        (yp.get(i, j).im + off).abs() < 1e-9,
                        "off {}",
                        yp.get(i, j).im
                    );
                }
            }
        }
    }

    #[test]
    fn numsteps_splits_kvar() {
        let mut c = Capacitor::new("c1");
        c.spec_type = 1;
        c.fkvarrating = vec![600.0];
        c.fnumsteps = 1;
        c.set_num_steps(3);
        assert_eq!(c.fnumsteps, 3);
        assert_eq!(c.fkvarrating, vec![200.0, 200.0, 200.0]);
        assert_eq!(c.fstates, vec![1, 1, 1]);
        assert_eq!(c.flast_step_in_service, 3);
    }
}
