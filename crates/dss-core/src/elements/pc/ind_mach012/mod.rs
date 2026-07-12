//! Port of `PCElements/IndMach012.pas` — `TIndMach012Obj`, the symmetrical-
//! component (positive/negative sequence) induction machine.
//!
//! The machine is a PC element on the Generator template. For power flow it is an
//! equivalent-circuit motor whose slip floats to match the specified shaft power
//! (`Get_PFlowModelCurrent` + the `dSdP` slip-Newton in `CalcPFlow`); for dynamics
//! it is a voltage source behind the transient reactance `Zsp` whose internal
//! voltages `E1`/`E2` and shaft speed/angle are integrated trapezoidally
//! (`InitStateVars`/`IntegrateStates`). It reuses `TGeneratorVars` (here flattened
//! as the `MachineData` fields) for the shaft swing equation.
//!
//! Split into submodules (this file holds the metadata, struct, `Create` and the
//! nominal-power machinery):
//! - [`solve`]: `CalcYPrimMatrix`, the equivalent-circuit power-flow/dynamic
//!   current models and the injection-current assembly.
//! - [`dynamics`]: `InitStateVars`/`IntegrateStates`/`Integrate` and the 22 state
//!   variables the Monitor mode-3 path consumes.
//! - [`accessors`]: the `CktElement` / `DssObject` trait impls.
//!
//! NOT_PORTED: the `DebugTrace` CSV trace file (no file I/O), and the
//! `IndMach012SwitchOpen` Open/Close flag is carried but never set — exactly the
//! latent state shared with Generator's
//! `gen_switch_open` (the protection Open/Close path is not yet wired to PC
//! elements' switch flag).

#[cfg(test)]
mod tests;

use num_complex::Complex64;

use crate::elements::ckt::CktElementData;
use crate::elements::general::load_shape::LoadShapeObj;
use crate::elements::general::spectrum::SpectrumObj;
use crate::elements::pc::generator::{Connection, default_recalc_ctx};
use crate::elements::traits::{ElemRef, SysCtx};
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};
use crate::util::{CDOUBLEONE, inv_sqrt3_x1000};

mod accessors;
mod dynamics;
mod solve;

/// 1-based property ordinals (Pascal `TIndMach012Prop` + class tails).
pub mod prop {
    pub const PHASES: usize = 1;
    pub const BUS1: usize = 2;
    pub const KV: usize = 3;
    pub const KW: usize = 4;
    pub const PF: usize = 5;
    pub const CONN: usize = 6;
    pub const KVA: usize = 7;
    pub const H: usize = 8;
    pub const D: usize = 9;
    pub const PURS: usize = 10;
    pub const PUXS: usize = 11;
    pub const PURR: usize = 12;
    pub const PUXR: usize = 13;
    pub const PUXM: usize = 14;
    pub const SLIP: usize = 15;
    pub const MAXSLIP: usize = 16;
    pub const SLIPOPTION: usize = 17;
    pub const YEARLY: usize = 18;
    pub const DAILY: usize = 19;
    pub const DUTY: usize = 20;
    pub const DEBUGTRACE: usize = 21;
    // PCClass tail:
    pub const SPECTRUM: usize = 22;
    // CktElementClass tail:
    pub const BASE_FREQ: usize = 23;
    pub const ENABLED: usize = 24;
    pub const NUM_PROPS: usize = 25; // incl. Like
}

/// `TIndMach012.DefineProperties`.
pub fn class_props(enums: &EnumRegistry) -> ClassProps {
    let defs = vec![
        PropDef::integer("Phases").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::bus("Bus1", 1),
        PropDef::double("kV").flags(PropFlags::NON_NEGATIVE),
        PropDef::double("kW"),
        // Pascal `pf` is `[SilentReadOnly, ReadByFunction]` → PowerFactor(Power[1]):
        // read-only (writes silently ignored in set_f64), and the text render is ""
        // always — upstream never sets PropertyOffset (stays -1), so the
        // GetObjPropertyValue guard skips the read function even on a solved
        // circuit (probe-proven; see SILENT_READ_ONLY).
        PropDef::double("PF").flags(PropFlags::SILENT_READ_ONLY),
        PropDef::mapped_string_enum("Conn", enums.connection),
        PropDef::double("kVA"),
        PropDef::double("H"),
        PropDef::double("D"),
        PropDef::double("puRs"),
        PropDef::double("puXs"),
        PropDef::double("puRr"),
        PropDef::double("puXr"),
        PropDef::double("puXm"),
        // Pascal `Slip` is `[WriteByFunction]` → set_Localslip (handled in set_f64).
        PropDef::double("Slip"),
        PropDef::double("MaxSlip"),
        PropDef::mapped_string_enum("SlipOption", enums.ind_mach_slip_option),
        PropDef::object_ref_class("LoadShape", "Yearly"),
        PropDef::object_ref_class("LoadShape", "Daily"),
        PropDef::object_ref_class("LoadShape", "Duty"),
        PropDef::boolean("DebugTrace"),
        // PCClass tail:
        PropDef::object_ref("Spectrum"),
        // CktElementClass tail:
        PropDef::double("BaseFreq").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::enabled("Enabled"),
    ];
    debug_assert_eq!(defs.len(), prop::NUM_PROPS - 1);
    ClassProps::new("IndMach012", defs, true)
}

/// `TIndMach012Obj`. `MachineData` (the `TGeneratorVars` shaft record) is
/// flattened into the `*_machine`/shaft fields; `MakeLike` copies them wholesale.
#[derive(Debug, Clone)]
pub struct IndMach012 {
    pub cd: CktElementData,

    pub connection: Connection,
    pub yeq: Complex64, // Y at nominal voltage (L-N)

    pub pu_rs: f64,
    pub pu_xs: f64,
    pub pu_rr: f64,
    pub pu_xr: f64,
    pub pu_xm: f64,
    pub s1: f64, // positive-sequence slip
    pub s2: f64, // negative-sequence slip (= 2 - S1)
    pub max_slip: f64,
    pub dsdp: f64, // slip-Newton step for power flow

    // Dynamics impedance parameters.
    pub xopen: f64,
    pub xp: f64,  // transient reactance
    pub t0p: f64, // rotor time constant

    pub in_dynamics: bool,

    pub zs: Complex64,
    pub zm: Complex64,
    pub zr: Complex64,
    // Last computed sequence currents/voltages (used by the variable getters).
    pub is1: Complex64,
    pub ir1: Complex64,
    pub v1: Complex64,
    pub is2: Complex64,
    pub ir2: Complex64,
    pub v2: Complex64,

    // Dynamics state: voltages behind the transient reactance Zsp.
    pub e1: Complex64,
    pub e1n: Complex64,
    pub de1dt: Complex64,
    pub de1dtn: Complex64,
    pub e2: Complex64,
    pub e2n: Complex64,
    pub de2dt: Complex64,
    pub de2dtn: Complex64,
    pub zsp: Complex64,

    pub first_iteration: bool,
    pub fixed_slip: bool,
    pub ind_mach_switch_open: bool,
    pub debug_trace: bool,

    pub machine_on: bool,
    pub shape_factor: Complex64,
    pub shape_is_actual: bool,

    pub v_base: f64,
    pub kw_base: f64,

    // MachineData (TGeneratorVars) — the shaft swing-equation record.
    pub kv_generator_base: f64,
    pub kva_rating: f64,
    pub h_mass: f64,
    /// `MachineData.D` — set by the `D=` property (default 1.0) but overwritten in
    /// `InitStateVars` by `Dpu·kVA·1000/w0`; see [`Self::dpu`].
    pub d: f64,
    /// `MachineData.Dpu` — **no property maps to it**, so it stays at its record
    /// default (0.0). Pascal `InitStateVars` then sets `D := Dpu·kVA·1000/w0 = 0`,
    /// so the machine is effectively undamped in dynamics regardless of `D=`. A
    /// faithful reproduction of the upstream wiring (not a `TODO(compat)` — the
    /// numbers depend on it).
    pub dpu: f64,
    pub w0: f64,
    pub speed: f64,               // relative to synchronous, rad/sec
    pub dspeed: f64,              //
    pub theta: f64,               // Direct-axis voltage angle (rad)
    pub dtheta: f64,              //
    pub p_shaft: f64,             // shaft power (W)
    pub m_mass: f64,              // 2·H·kVA·1000/w0
    pub speed_history: f64,       // integration history
    pub theta_history: f64,       //
    pub p_nominal_per_phase: f64, // power-flow shaft-power target (W/phase)

    // Harmonic spectrum (PCClass tail). `DoHarmonicMode` injects ~0 (the Pascal
    // spectrum lines are commented out), but the property is stored/dumped.
    pub spectrum: String,
    pub spectrum_obj: Option<SpectrumObj>,

    // Dispatch shapes (snapshot-clone, like the Generator shape refs).
    pub yearly_shape: String,
    pub daily_shape: String,
    pub duty_shape: String,
    pub yearly_shape_obj: Option<LoadShapeObj>,
    pub daily_shape_obj: Option<LoadShapeObj>,
    pub duty_shape_obj: Option<LoadShapeObj>,
    pub yearly_shape_ref: Option<ElemRef>,
    pub daily_shape_ref: Option<ElemRef>,
    pub duty_shape_ref: Option<ElemRef>,
}

/// Pascal `SetNcondsForConnection`. Unlike the Generator, a **wye** induction
/// machine has **no neutral conductor** (`NConds := Fnphases`).
fn nconds_for_connection(connection: Connection, nphases: usize) -> usize {
    match connection {
        Connection::Wye => nphases, // neutral not connected for the induction machine
        Connection::Delta => match nphases {
            1 | 2 => nphases + 1, // L-L and open-delta
            _ => nphases,
        },
    }
}

impl IndMach012 {
    /// Pascal `TIndMach012Obj.Create`.
    pub fn new(name: &str) -> Self {
        let mut cd = CktElementData::new(name, prop::NUM_PROPS);
        cd.nphases = 3;
        cd.nconds = 3; // delta default; no neutral
        cd.set_nterms(1);

        let kw_base = 1000.0;
        let kv_generator_base = 12.47;
        let kva_rating = kw_base * 1.2;
        let base_freq = cd.base_frequency;

        let mut m = Self {
            cd,
            connection: Connection::Delta, // Delta default
            yeq: Complex64::ZERO,
            pu_rs: 0.0053,
            pu_xs: 0.106,
            pu_rr: 0.007,
            pu_xr: 0.12,
            pu_xm: 4.0,
            s1: 0.0,
            s2: 0.0,
            max_slip: 0.1, // 10% slip limit — set before the slip
            dsdp: 0.0,
            xopen: 0.0,
            xp: 0.0,
            t0p: 0.0,
            in_dynamics: false,
            zs: Complex64::ZERO,
            zm: Complex64::ZERO,
            zr: Complex64::ZERO,
            is1: Complex64::ZERO,
            ir1: Complex64::ZERO,
            v1: Complex64::ZERO,
            is2: Complex64::ZERO,
            ir2: Complex64::ZERO,
            v2: Complex64::ZERO,
            e1: Complex64::ZERO,
            e1n: Complex64::ZERO,
            de1dt: Complex64::ZERO,
            de1dtn: Complex64::ZERO,
            e2: Complex64::ZERO,
            e2n: Complex64::ZERO,
            de2dt: Complex64::ZERO,
            de2dtn: Complex64::ZERO,
            zsp: Complex64::ZERO,
            first_iteration: true,
            fixed_slip: false, // allow slip to float to match power
            ind_mach_switch_open: false,
            debug_trace: false,
            machine_on: true,
            shape_factor: CDOUBLEONE,
            shape_is_actual: false,
            v_base: 0.0,
            kw_base,
            kv_generator_base,
            kva_rating,
            h_mass: 1.0,
            d: 1.0,
            dpu: 0.0,
            w0: std::f64::consts::TAU * base_freq,
            speed: 0.0,
            dspeed: 0.0,
            theta: 0.0,
            dtheta: 0.0,
            p_shaft: 0.0,
            m_mass: 0.0,
            speed_history: 0.0,
            theta_history: 0.0,
            p_nominal_per_phase: 0.0,
            spectrum: "default".to_string(), // TPCElement: SpectrumClass.DefaultGeneral
            spectrum_obj: None,
            yearly_shape: String::new(),
            daily_shape: String::new(),
            duty_shape: String::new(),
            yearly_shape_obj: None,
            daily_shape_obj: None,
            duty_shape_obj: None,
            yearly_shape_ref: None,
            daily_shape_ref: None,
            duty_shape_ref: None,
        };
        // Set slip local and make the generator model agree (Pascal Create:
        // set_LocalSlip(0.007) then PropertySideEffects(slip)).
        m.set_local_slip(0.007); // about 1 pu power
        m.speed = m.w0 * (-m.s1); // PropertySideEffects(slip)

        m.cd.inj_current = vec![Complex64::ZERO; m.cd.yorder];
        m.recalc(&default_recalc_ctx());
        m
    }

    /// Pascal `set_Localslip`: clamp the slip to ±MaxSlip outside dynamics and set
    /// the negative-sequence slip `S2 := 2 - S1`.
    pub(super) fn set_local_slip(&mut self, value: f64) {
        self.s1 = value;
        if !self.in_dynamics && self.s1.abs() > self.max_slip {
            self.s1 = if self.s1 < 0.0 {
                -self.max_slip
            } else {
                self.max_slip
            };
        }
        self.s2 = 2.0 - self.s1;
    }

    /// Pascal `PropertySideEffects(kV)`: update `VBase`. Pascal multiplies by the
    /// precomputed `InvSQRT3x1000 = 1000/Sqrt(3)` constant (same grouping as the
    /// Generator port), not `(kV·1000)/Sqrt(3)`.
    pub(super) fn update_vbase(&mut self) {
        self.v_base = match self.cd.nphases {
            2 | 3 => self.kv_generator_base * inv_sqrt3_x1000(),
            _ => self.kv_generator_base * 1000.0,
        };
    }

    /// Pascal `TIndMach012Obj.RecalcElementData`.
    pub(super) fn recalc(&mut self, sys: &SysCtx) {
        let z_base = self.kv_generator_base.powi(2) / self.kva_rating * 1000.0;

        let rs = self.pu_rs * z_base;
        let xs = self.pu_xs * z_base;
        let rr = self.pu_rr * z_base;
        let xr = self.pu_xr * z_base;
        let xm = self.pu_xm * z_base;
        self.zs = Complex64::new(rs, xs);
        self.zm = Complex64::new(0.0, xm);
        self.zr = Complex64::new(rr, xr);

        self.xopen = xs + xm;
        self.xp = xs + (xr * xm) / (xr + xm);
        self.zsp = Complex64::new(rs, self.xp);
        // Yeq is vars-only for power flow (the dynamic Yeq = Cinv(Zsp) is set in
        // InitStateVars).
        self.yeq = Complex64::new(0.0, -1.0 / z_base);
        self.t0p = (xr + xm) / (self.w0 * rr);

        self.dsdp = self.compute_dsdp();

        self.is1 = Complex64::ZERO;
        self.v1 = Complex64::ZERO;
        self.is2 = Complex64::ZERO;
        self.v2 = Complex64::ZERO;
        self.first_iteration = true;

        self.cd.inj_current = vec![Complex64::ZERO; self.cd.yorder];

        self.set_nominal_power(sys);
        // NOT_PORTED: DebugTrace CSV file.
    }

    /// Pascal `TIndMach012Obj.Compute_dSdP` — `dSdP` from the rated slip and rated
    /// voltage. Mutates `V1`/`Is1`/`Ir1` (Pascal does the same; `RecalcElementData`
    /// zeroes them right after).
    fn compute_dsdp(&mut self) -> f64 {
        // dSdP based on rated slip and rated voltage.
        self.v1 = Complex64::new(self.kv_generator_base * 1000.0 / 1.732, 0.0);
        if self.s1 != 0.0 {
            let (is1, ir1) = self.get_pflow_model_current(self.v1, self.s1);
            self.is1 = is1;
            self.ir1 = ir1;
        }
        self.s1 / (self.v1 * self.is1.conj()).re
    }

    /// Pascal `TIndMach012Obj.SetNominalPower` — set the per-phase shaft power
    /// target for the power-flow slip-Newton. `Factor` is always 1.0 (an
    /// induction machine does not apply the generator multiplier).
    pub(super) fn set_nominal_power(&mut self, sys: &SysCtx) {
        use crate::solution::{SolveMode, USEDAILY, USEDUTY, USEYEARLY};

        let machine_on_saved = self.machine_on;
        self.shape_factor = CDOUBLEONE;
        let dbl_hour = sys.dbl_hour;
        // Leave the machine in whatever state it had before dynamics/harmonics.
        if !(sys.is_dynamic_model || sys.is_harmonic_model) {
            self.machine_on = true;
        }

        if !self.machine_on {
            // If the machine is OFF, enter as a tiny resistive load (-0.1 pu) to
            // avoid a divide-by-zero in the matrix.
            self.p_nominal_per_phase = -0.1 * self.kw_base / self.cd.nphases as f64;
        } else {
            let factor = 1.0;
            match sys.mode {
                SolveMode::Daily => self.calc_daily_mult(dbl_hour),
                SolveMode::Yearly => self.calc_yearly_mult(dbl_hour),
                SolveMode::DutyCycle => self.calc_duty_mult(dbl_hour),
                // GENERALTIME / DYNAMICMODE (IndMach012.pas l.1091-1105): the
                // one class `ActiveLoadShapeClass` selects (`Set
                // LoadShapeClass=`) drives `ShapeFactor`; default `USENONE` →
                // 1+j1.
                SolveMode::Time | SolveMode::Dynamic => match sys.active_load_shape_class {
                    USEDAILY => self.calc_daily_mult(dbl_hour),
                    USEYEARLY => self.calc_yearly_mult(dbl_hour),
                    USEDUTY => self.calc_duty_mult(dbl_hour),
                    _ => self.shape_factor = CDOUBLEONE,
                },
                SolveMode::Monte2 | SolveMode::Monte3 | SolveMode::LD1 | SolveMode::LD2 => {
                    self.calc_daily_mult(dbl_hour)
                }
                SolveMode::PeakDay => self.calc_daily_mult(dbl_hour),
                // SNAPSHOT / MONTECARLO1 / MONTEFAULT / FAULTSTUDY / AUTOADD /
                // else: Factor := 1.0, no shape.
                _ => {}
            }

            if !(sys.is_dynamic_model || sys.is_harmonic_model) {
                let nphases = self.cd.nphases as f64;
                if self.shape_is_actual {
                    self.p_nominal_per_phase = 1000.0 * self.shape_factor.re / nphases;
                } else {
                    self.p_nominal_per_phase =
                        1000.0 * self.kw_base * factor * self.shape_factor.re / nphases;
                }
                // Cannot dispatch vars in an induction machine — you get what you get.
            }
        }

        // If the machine state changes, force re-calc of the Y matrix.
        if self.machine_on != machine_on_saved {
            self.cd.yprim_invalid = true;
        }
    }

    /// Pascal `CalcDailyMult`.
    fn calc_daily_mult(&mut self, hr: f64) {
        if let Some(shape) = self.daily_shape_obj.as_mut() {
            self.shape_factor = shape.get_mult_at_hour(hr);
            self.shape_is_actual = shape.use_actual();
        } else {
            self.shape_factor = CDOUBLEONE; // no daily variation
        }
    }

    /// Pascal `CalcDutyMult`.
    fn calc_duty_mult(&mut self, hr: f64) {
        if let Some(shape) = self.duty_shape_obj.as_mut() {
            self.shape_factor = shape.get_mult_at_hour(hr);
            self.shape_is_actual = shape.use_actual();
        } else {
            self.calc_daily_mult(hr); // default to the daily mult
        }
    }

    /// Pascal `CalcYearlyMult`.
    fn calc_yearly_mult(&mut self, hr: f64) {
        if let Some(shape) = self.yearly_shape_obj.as_mut() {
            self.shape_factor = shape.get_mult_at_hour(hr);
            self.shape_is_actual = shape.use_actual();
        } else {
            self.shape_factor = CDOUBLEONE; // no variation
        }
    }
}
