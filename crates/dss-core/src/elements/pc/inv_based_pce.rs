//! Shared inverter-based PC-element base. Port of Pascal
//! `PCElements/InvBasedPCE.pas` (`TInvBasedPCE`) plus the `DynamicEq`/`DynOut`
//! fields its parent `PCElements/DynEqPCE.pas` (`TDynEqPCE`) contributes.
//!
//! Upstream comment: "InvBasedPCE is an abstract class for grouping inverter
//! based functions" — the shared data + behavior behind PVSystem and Storage.
//! It is not a New-able class (`CreateDSSClasses` never registers it); the
//! concrete subclasses embed [`InvBasedPceData`] the way [`Generator`] flattens
//! `GenVars`, and dispatch the virtual hooks through the [`InvBasedPce`] trait.
//!
//! Scope (WP7.3 step 1): the base **data** record, the [`InvDynamicVars`]
//! scalar sub-record (the scalars back PVSystem/Storage properties — `kVDC`,
//! `kP`, `PITol`, `SafeVoltage`, `AmpLimit`, `AmpLimitGain`, `SafeMode` — so they
//! must exist before those classes land), and the power-flow-relevant shared
//! methods (`StickCurrInTerminalArray`, `Get_Presentkvar`, `UsingCIMDynamics`).
//! The GFL dynamics machinery — the `TInvDynamicVars` arrays +
//! `SolveDynamicStep`/`SolveModulation`/`InitDynArrays`, the `PICtrl`
//! PI-controller array, and (WP7.7 step 3b) the embedded [`DynEqPceData`] memory
//! that integrates a user `DynamicExp` — landed in WP7.7. The grid-forming-mode
//! (GFM) power-flow model — `CalcGFMYprim`/`CalcGFMVoltage`, the `GetCurrents`
//! override, `CheckAmpsLimit` — landed in WPG.13 (snapshot/daily/direct). The
//! **dynamics-mode** GFM branch (`DoDynamicMode`/`IntegrateStates` GFM,
//! `FixPhaseAngle`/`VDelta`/`ISPDelta` black-start droop) landed in WPG.17.
//!
//! [`Generator`]: crate::elements::pc::generator::Generator

#[cfg(test)]
mod tests;

use num_complex::Complex64;

use crate::elements::general::load_shape::LoadShapeObj;
use crate::elements::general::xy_curve::XyCurveObj;
use crate::elements::pc::dyneq_pce::DynEqPceData;
use crate::elements::traits::ElemRef;
use crate::support::cmatrix::CMatrix;
use crate::support::complexutil::{Polar, pdeg_to_complex};
use crate::support::dynamics::IterationFlag;
use crate::support::mathutil::PiCtrl;
use crate::util::{quad_solver, sqrt3};

/// Pascal `NumInvDynVars = 9` (InvDynamics.pas l.61).
pub const NUM_INV_DYN_VARS: usize = 9;

/// Pascal `Connection: Integer` on `TInvBasedPCE` (0 = line-neutral/wye,
/// 1 = delta). Mirrors `generator::Connection`; kept local so the inverter base
/// does not depend on the Generator module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Connection {
    Wye = 0,
    Delta = 1,
}

/// Pascal `TInvDynamicVars` (`Shared/InvDynamics.pas`) — the per-inverter
/// dynamics/GFM state record. Scalars back catalog properties; per-phase arrays
/// and methods are the GFL dynamics machinery (WP7.7 step 2b).
#[derive(Debug, Clone)]
pub struct InvDynamicVars {
    /// `iMaxPPhase` — max amps per phase (dynamics target).
    pub i_max_p_phase: f64,
    /// `kP` — PI controller gain (property `kP`).
    pub kp: f64,
    /// `CtrlTol` — control-loop tolerance (property `PITol`).
    pub ctrl_tol: f64,
    /// `SMThreshold` — safe-mode voltage threshold (property `SafeVoltage`).
    pub sm_threshold: f64,
    /// `RatedVDC` — rated DC input voltage (property `kVDC`).
    pub rated_vdc: f64,
    /// `LS` — series inductance (cannot be 0 in dyn mode).
    pub ls: f64,
    /// `RS` — series (filter) resistance.
    pub rs: f64,
    /// `BasekV` — base kV by phase count.
    pub base_kv: f64,
    /// `BaseV` — GFM base voltage.
    pub base_v: f64,
    /// `MaxVS` — max terminal voltage for safe operation.
    pub max_vs: f64,
    /// `MinVS` — min terminal voltage for safe operation.
    pub min_vs: f64,
    /// `MinAmps` — min amps required to export energy.
    pub min_amps: f64,
    /// `mKVARating` — GFM impedance kVA rating.
    pub m_kva_rating: f64,
    /// `RatedkVLL` — declared rated L-L kV (GFM impedance calc).
    pub rated_kv_ll: f64,
    /// `ILimit` — output-current limit (property `AmpLimit`; base default -1 =
    /// no limit).
    pub i_limit: f64,
    /// `IComp` — compensation value when the amps limiter is active.
    pub i_comp: f64,
    /// `VError` — current-limiting systemic error factor (property
    /// `AmpLimitGain`; base default 0.8).
    pub v_error: f64,
    /// `ISP` — current setpoint from the present DER kW.
    pub isp: f64,
    /// `Discharging` — storage discharging flag.
    pub discharging: bool,
    /// `ResetIBR` — force-the-IBR-OFF flag.
    pub reset_ibr: bool,
    /// `SafeMode` — inverter entered safe mode (property `SafeMode`).
    pub safe_mode: bool,

    // --- per-phase array fields (WP7.7 step 2b; GFL dynamics) ---
    // `VDelta` (GFM-only) is intentionally absent.
    /// `Vgrid` — grid voltage at the point of connection per phase (polar).
    pub vgrid: Vec<Polar>,
    /// `dit` — current first derivative per phase.
    pub dit: Vec<f64>,
    /// `it` — current integration per phase.
    pub it: Vec<f64>,
    /// `itHistory` — shift register for `it`.
    pub it_history: Vec<f64>,
    /// `m` — average duty cycle per phase.
    pub m: Vec<f64>,
    /// `VDelta` — GFM black-start voltage delta per phase (droop control).
    pub v_delta: Vec<f64>,
    /// `ISPDelta` — GFM current-target delta per phase (black-start / droop;
    /// ramped by the dynamics-mode GFM `IntegrateStates`, WPG.17).
    pub isp_delta: Vec<f64>,
    /// `AngDelta` — phase-angle correction (GFM `FixPhaseAngle`).
    pub ang_delta: Vec<f64>,
    /// `SfModePhase` — per-phase safe-mode flag.
    pub sf_mode_phase: Vec<bool>,
}

impl InvDynamicVars {
    /// Pascal `TInvBasedPCE.Create`'s `with dynVars` block: `ILimit := -1`
    /// (no amps limit), `IComp := 0`, `VError := 0.8`. Every other scalar starts
    /// at 0/false; per-phase arrays are empty until `init_dyn_arrays` sizes them.
    pub fn new() -> Self {
        Self {
            i_max_p_phase: 0.0,
            kp: 0.0,
            ctrl_tol: 0.0,
            sm_threshold: 0.0,
            rated_vdc: 0.0,
            ls: 0.0,
            rs: 0.0,
            base_kv: 0.0,
            base_v: 0.0,
            max_vs: 0.0,
            min_vs: 0.0,
            min_amps: 0.0,
            m_kva_rating: 0.0,
            rated_kv_ll: 0.0,
            i_limit: -1.0,
            i_comp: 0.0,
            v_error: 0.8,
            isp: 0.0,
            discharging: false,
            reset_ibr: false,
            safe_mode: false,
            vgrid: Vec::new(),
            dit: Vec::new(),
            it: Vec::new(),
            it_history: Vec::new(),
            m: Vec::new(),
            v_delta: Vec::new(),
            isp_delta: Vec::new(),
            ang_delta: Vec::new(),
            sf_mode_phase: Vec::new(),
        }
    }

    /// Pascal `TInvDynamicVars.InitDynArrays` (l.304): resize and zero all
    /// per-phase arrays. Called by `InitStateVars` of both PVSystem and Storage.
    pub fn init_dyn_arrays(&mut self, nphases: usize) {
        self.dit = vec![0.0; nphases];
        self.it = vec![0.0; nphases];
        self.it_history = vec![0.0; nphases];
        self.vgrid = vec![Polar { mag: 0.0, ang: 0.0 }; nphases];
        self.m = vec![0.0; nphases];
        self.v_delta = vec![0.0; nphases];
        self.isp_delta = vec![0.0; nphases];
        self.ang_delta = vec![0.0; nphases];
        self.sf_mode_phase = vec![false; nphases];
        self.safe_mode = false;
    }

    /// Pascal `TInvDynamicVars.CalcGFMYprim` (InvDynamics.pas l.229) — the
    /// equivalent short-circuit admittance for an inverter operating in
    /// grid-forming mode. Similar to the VSource sequence-impedance build, with
    /// the `R0`/`X0` defaults (1.9/5.7) and `R1 = X1/4` baked in. Fills the
    /// `nphases×nphases` symmetric block of an `order×order` matrix, inverts it,
    /// and returns the resulting admittance (`YMatrix.CopyFrom(Z⁻¹)`).
    pub fn calc_gfm_yprim(&self, nphases: usize, order: usize) -> CMatrix {
        let mut z = CMatrix::new(order);

        // X1 = (RatedkVLL² / mKVARating) / √(1 + 0.0625).
        let x1 = (self.rated_kv_ll.powi(2) / self.m_kva_rating) / (1.0 + 0.0625_f64).sqrt();
        let r1 = x1 / 4.0; // uses defaults
        // R0 := 1.9; X0 := 5.7; X0R0 := X0/R0 (before QuadSolver re-solves R0).
        let x0r0 = 5.7 / 1.9;
        let isc1 = (self.m_kva_rating / (sqrt3() * self.rated_kv_ll)) / nphases as f64;
        // Compute R0, X0. Pascal hardcodes `a := 10` (= 1 + X0R0² for the 5.7/1.9
        // defaults) — reproduced as the literal so QuadSolver matches bit-for-bit.
        let a = 10.0;
        let b = 4.0 * (r1 + (x1 * x0r0));
        let c = 4.0 * (r1 * r1 + x1 * x1) - ((sqrt3() * self.rated_kv_ll * 1000.0) / isc1).powi(2);
        let r0 = quad_solver(a, b, c);
        let x0 = r0 * x0r0;
        // for Z matrix
        let xs = (2.0 * x1 + x0) / 3.0;
        let rs = (2.0 * r1 + r0) / 3.0;
        let rm = (r0 - r1) / 3.0;
        let xm = (x0 - x1) / 3.0;
        let zs = num_complex::Complex64::new(rs, xs);
        let zm = num_complex::Complex64::new(rm, xm);

        for i in 0..nphases {
            z.set(i, i, zs);
            for j in 0..i {
                z.set(i, j, zm);
                z.set(j, i, zm);
            }
        }
        // Pascal ignores the invert return (a delta GFM inverter is non-singular;
        // the corpus GFM decks are all delta-connected).
        let _ = z.invert();
        z
    }

    /// Pascal `TInvDynamicVars.CalcGFMVoltage` (InvDynamics.pas l.293) —
    /// balanced internal source phasors at magnitude `BaseV`, angles
    /// `360 − (k·360)/NPhases` (degrees). Written into `x[0..nphases-1]`
    /// (Pascal writes the 1-based `x[1..NPhases]`, leaving the ground slot).
    pub fn calc_gfm_voltage(&self, nphases: usize, x: &mut [num_complex::Complex64]) {
        let ref_angle = 0.0;
        for (k, slot) in x.iter_mut().enumerate().take(nphases) {
            *slot = pdeg_to_complex(
                self.base_v,
                360.0 + ref_angle - (k as f64 * 360.0) / nphases as f64,
            );
        }
    }

    /// Pascal `TInvDynamicVars.FixPhaseAngle` (InvDynamics.pas l.220) — corrects
    /// the current phasor angle for phase `idx` (dynamics GFM, black-start).
    pub fn fix_phase_angle(&mut self, idx: usize) {
        use std::f64::consts::TAU;
        self.ang_delta[idx] += ((idx as f64 * TAU) / -3.0) - self.vgrid[idx].ang;
        self.vgrid[idx].ang = self.ang_delta[idx];
    }

    /// Pascal `TInvDynamicVars.SolveModulation` (l.178) — update the duty cycle
    /// for phase `i` using the PI controller `pi`. Runs only on the corrector
    /// pass: Pascal `if IterationFlag=0 then Exit` (0 == `NewTimeStep`), so we
    /// return early on the predictor.
    pub fn solve_modulation(&mut self, iteration_flag: IterationFlag, i: usize, pi: &mut PiCtrl) {
        if iteration_flag == IterationFlag::NewTimeStep {
            return;
        }

        let i_error = self.isp - self.it[i];
        // Pascal `iErrorPct := iError / ISP` (InvDynamics.pas l.190) — an
        // unconditional IEEE division. `ISP` is forced to >= 0.01 (the inverter
        // "off" value) before this call in every reachable path, so the divisor is
        // never 0; the bare divide reproduces FPC's inf/NaN on ISP==0 verbatim (a
        // defensive zero-guard would change the control path the oracle takes).
        let i_error_pct = i_error / self.isp;

        if i_error_pct.abs() > self.ctrl_tol {
            let i_delta = pi.solve_pi(i_error);
            let d_cycle = self.m[i] + i_delta;

            if self.vgrid[i].mag > self.min_vs || self.min_vs == 0.0 {
                if self.safe_mode || self.sf_mode_phase[i] {
                    // Coming back from safe operation — boost duty cycle.
                    self.m[i] = ((self.rs * self.it[i]) + self.vgrid[i].mag) / self.rated_vdc;
                    self.safe_mode = false;
                    self.sf_mode_phase[i] = false;
                } else if d_cycle <= 1.0 && d_cycle > 0.0 {
                    self.m[i] = d_cycle;
                }
            } else {
                self.m[i] = 0.0;
                self.it[i] = 0.0;
                self.it_history[i] = 0.0;
                self.safe_mode = true;
                self.sf_mode_phase[i] = true;
            }
        }
    }

    /// Pascal `TInvDynamicVars.SolveDynamicStep` (l.168): call
    /// `solve_modulation` then compute `dit[i]`.
    pub fn solve_dynamic_step(&mut self, iteration_flag: IterationFlag, i: usize, pi: &mut PiCtrl) {
        self.solve_modulation(iteration_flag, i, pi);
        if self.safe_mode {
            self.dit[i] = 0.0;
        } else {
            self.dit[i] =
                ((self.m[i] * self.rated_vdc) - (self.rs * self.it[i]) - self.vgrid[i].mag)
                    / self.ls;
        }
    }

    /// Pascal `TInvDynamicVars.Get_InvDynValue` (l.69) — return the state
    /// variable at 0-based index `var_idx`. Reports the **last** phase
    /// (`num_phases - 1`) for the per-phase arrays, exactly as Pascal does.
    pub fn get_inv_dyn_value(&self, var_idx: usize, num_phases: usize) -> f64 {
        let last = num_phases.saturating_sub(1);
        match var_idx {
            0 => self.vgrid.get(last).map_or(0.0, |p| p.mag),
            1 => *self.dit.get(last).unwrap_or(&0.0),
            2 => *self.it.get(last).unwrap_or(&0.0),
            3 => *self.it_history.get(last).unwrap_or(&0.0),
            4 => self.rated_vdc,
            5 => *self.m.first().unwrap_or(&0.0),
            6 => self.isp,
            7 => self.ls,
            8 => self.i_max_p_phase,
            _ => 0.0,
        }
    }

    /// Pascal `TInvDynamicVars.Get_InvDynName` (l.140).
    pub fn get_inv_dyn_name(var_idx: usize) -> &'static str {
        match var_idx {
            0 => "Grid voltage",
            1 => "di/dt",
            2 => "it",
            3 => "it History",
            4 => "Rated VDC",
            5 => "Avg duty cycle",
            6 => "Target (Amps)",
            7 => "Series L",
            8 => "Max. Amps (phase)",
            _ => "Unknown variable",
        }
    }

    /// Pascal `TInvDynamicVars.Set_InvDynValue` (l.112) — set the state
    /// variable at 0-based index `var_idx`. Indices 0 and 6 are read-only in
    /// Pascal (bare `;`); mirrored here. Reached via the element `set_variable`
    /// trait method (the InvDyn tail of `Set_Variable`).
    pub fn set_inv_dyn_value(&mut self, var_idx: usize, value: f64) {
        match var_idx {
            0 => {} // read-only (Vgrid.mag)
            1 => {
                if let Some(v) = self.dit.first_mut() {
                    *v = value;
                }
            }
            2 => {
                if let Some(v) = self.it.first_mut() {
                    *v = value;
                }
            }
            3 => {
                if let Some(v) = self.it_history.first_mut() {
                    *v = value;
                }
            }
            4 => {
                self.rated_vdc = value;
            }
            5 => {
                if let Some(v) = self.m.first_mut() {
                    *v = value;
                }
            }
            6 => {} // read-only (ISP)
            7 => {
                self.ls = value;
            }
            8 => {
                self.i_max_p_phase = value;
            }
            _ => {} // no-op (Pascal TODO: error out)
        }
    }
}

impl Default for InvDynamicVars {
    fn default() -> Self {
        Self::new()
    }
}

/// `TInvBasedPCE` shared base data (+ the `DynEqPCE` `DynamicEq`/`DynOut`
/// fields). The concrete inverter PC elements (PVSystem, Storage) embed this and
/// set its fields in their own `Create`/`RecalcElementData`.
#[derive(Debug, Clone)]
pub struct InvBasedPceData {
    /// `dynVars` — the dynamics/GFM state record (scalars + per-phase arrays).
    pub dyn_vars: InvDynamicVars,
    /// `PICtrl` array — one PI controller per phase, sized by `InitStateVars`.
    /// Kept separate from `dyn_vars` so both can be borrowed independently in
    /// the per-phase dynamics loop.
    pub pi_ctrl: Vec<PiCtrl>,
    /// `GFM_Mode` — grid-forming-inverter mode flag.
    pub gfm_mode: bool,
    /// `InverterON` — inverter currently energized.
    pub inverter_on: bool,
    /// `varMode` — 0 = constant PF, 1 = kvar specified.
    pub var_mode: i32,

    // InvControl/ExpControl mode flags (set by the controls, WP7.5).
    /// `VWMode` — under volt-watt control (InvControl).
    pub vw_mode: bool,
    /// `VVMode` — under volt-var control (InvControl).
    pub vv_mode: bool,
    /// `WVMode` — under watt-var control (InvControl).
    pub wv_mode: bool,
    /// `WPMode` — under watt-pf control (InvControl).
    pub wp_mode: bool,
    /// `DRCMode` — under dynamic-reactive-current control.
    pub drc_mode: bool,
    /// `AVRMode` — under AVR control (ExpControl).
    pub avr_mode: bool,

    /// `VBase` — base volts for computing currents.
    pub v_base: f64,
    /// `Vmaxpu`.
    pub vmaxpu: f64,
    /// `Vminpu`.
    pub vminpu: f64,
    /// `kvar_out` — present reactive output.
    pub kvar_out: f64,
    /// `kW_out` — present active output.
    pub kw_out: f64,
    /// `pctR` — % resistance.
    pub pct_r: f64,
    /// `pctX` — % reactance.
    pub pct_x: f64,
    /// `Pnominalperphase`.
    pub p_nominal_per_phase: f64,
    /// `Qnominalperphase`.
    pub q_nominal_per_phase: f64,
    /// `ShapeFactor` — present shape multiplier (complex).
    pub shape_factor: Complex64,
    /// `Connection` — wye/delta.
    pub connection: Connection,

    // Shape references (snapshot-clone + ElemRef, the WP4.2/WP5.3 pattern).
    /// `YearlyShapeObj` name.
    pub yearly_shape: String,
    /// `DailyShapeObj` name.
    pub daily_shape: String,
    /// `DutyShapeObj` name.
    pub duty_shape: String,
    pub yearly_shape_obj: Option<LoadShapeObj>,
    pub daily_shape_obj: Option<LoadShapeObj>,
    pub duty_shape_obj: Option<LoadShapeObj>,
    pub yearly_shape_ref: Option<ElemRef>,
    pub daily_shape_ref: Option<ElemRef>,
    pub duty_shape_ref: Option<ElemRef>,

    /// `InverterCurveObj` — inverter efficiency curve (XYcurve) name.
    pub inverter_curve: String,
    pub inverter_curve_obj: Option<XyCurveObj>,
    pub inverter_curve_ref: Option<ElemRef>,

    // Inverter functionality variables.
    /// `FpctCutIn` — % cut-in.
    pub fpct_cut_in: f64,
    /// `FpctCutOut` — % cut-out.
    pub fpct_cut_out: f64,
    /// `CutInkW` — computed cut-in kW.
    pub cut_in_kw: f64,
    /// `CutOutkW` — computed cut-out kW.
    pub cut_out_kw: f64,
    /// `CurrentkvarLimit`.
    pub current_kvar_limit: f64,
    /// `CurrentkvarLimitNeg`.
    pub current_kvar_limit_neg: f64,
    /// `VoltageModel` — variation with voltage.
    pub voltage_model: i32,
    /// `PFNominal`.
    pub pf_nominal: f64,
    /// `VarFollowInverter`.
    pub var_follow_inverter: bool,

    /// `YEQ` — admittance at nominal.
    pub yeq: Complex64,
    /// `YEQ_Min` — admittance at Vmin.
    pub yeq_min: Complex64,
    /// `YEQ_Max` — admittance at Vmax.
    pub yeq_max: Complex64,
    /// `VBaseMax`.
    pub v_base_max: f64,
    /// `VBaseMin`.
    pub v_base_min: f64,

    /// `kvarLimitSet`.
    pub kvar_limit_set: bool,
    /// `kvarLimitNegSet`.
    pub kvar_limit_neg_set: bool,
    /// `PhaseCurrentLimit`.
    pub phase_current_limit: Complex64,
    /// `DebugTrace` — (the trace *file* is not ported, like Generator).
    pub debug_trace: bool,

    /// `FpctPminNoVars`.
    pub fpct_pmin_no_vars: f64,
    /// `FpctPminkvarLimit`.
    pub fpct_pmin_kvar_limit: f64,
    /// `PminNoVars`.
    pub pmin_no_vars: f64,
    /// `PminkvarLimit`.
    pub pmin_kvar_limit: f64,
    /// `pf_wp_nominal`.
    pub pf_wp_nominal: f64,

    /// `ForceBalanced`.
    pub force_balanced: bool,
    /// `CurrentLimited`.
    pub current_limited: bool,

    /// `UserModelNameStr` — user model name; a `.wasm` value loads through the
    /// sandboxed WASM ABI (`WASM_USERMODELS_PLAN.md` §WP-WM.4, §2.4), a native-DLL
    /// name warns-and-falls-back. Stored for the dump + deferred load.
    pub user_model_name: String,
    /// `UserModelEditStr` — user model edit string, forwarded to the loaded WASM
    /// model's `edit` (WASM_USERMODELS §WP-WM.4).
    pub user_model_edit: String,

    /// `FirstSampleAfterReset`.
    pub first_sample_after_reset: bool,

    /// `TDynEqPCE` data the parent class contributes — the linked `DynamicExp`
    /// (`DynamicEq=`), its `DynamicEqVals`/`DynamicEqPair` integration memory, and
    /// the resolved `DynOut` output indices. Embedded here (rather than as the
    /// former bare `DynamicEq`/`DynOut` strings) so PVSystem/Storage share the same
    /// machinery as [`Generator`], driving `solve_eq` during a dynamics solve.
    ///
    /// [`Generator`]: crate::elements::pc::generator::Generator
    pub dyneq: DynEqPceData,
}

impl InvBasedPceData {
    /// Pascal `TInvBasedPCE.Create`: `GFM_Mode := FALSE`; the `dynVars` defaults
    /// (`ILimit/IComp/VError`); shape and inverter-curve objects nil. Every other
    /// field starts at 0/false/empty and is set by the concrete subclass.
    pub fn new() -> Self {
        Self {
            dyn_vars: InvDynamicVars::new(),
            pi_ctrl: Vec::new(),
            gfm_mode: false,
            inverter_on: false,
            var_mode: 0,
            vw_mode: false,
            vv_mode: false,
            wv_mode: false,
            wp_mode: false,
            drc_mode: false,
            avr_mode: false,
            v_base: 0.0,
            vmaxpu: 0.0,
            vminpu: 0.0,
            kvar_out: 0.0,
            kw_out: 0.0,
            pct_r: 0.0,
            pct_x: 0.0,
            p_nominal_per_phase: 0.0,
            q_nominal_per_phase: 0.0,
            shape_factor: Complex64::ZERO,
            connection: Connection::Wye,
            yearly_shape: String::new(),
            daily_shape: String::new(),
            duty_shape: String::new(),
            yearly_shape_obj: None,
            daily_shape_obj: None,
            duty_shape_obj: None,
            yearly_shape_ref: None,
            daily_shape_ref: None,
            duty_shape_ref: None,
            inverter_curve: String::new(),
            inverter_curve_obj: None,
            inverter_curve_ref: None,
            fpct_cut_in: 0.0,
            fpct_cut_out: 0.0,
            cut_in_kw: 0.0,
            cut_out_kw: 0.0,
            current_kvar_limit: 0.0,
            current_kvar_limit_neg: 0.0,
            voltage_model: 0,
            pf_nominal: 0.0,
            var_follow_inverter: false,
            yeq: Complex64::ZERO,
            yeq_min: Complex64::ZERO,
            yeq_max: Complex64::ZERO,
            v_base_max: 0.0,
            v_base_min: 0.0,
            kvar_limit_set: false,
            kvar_limit_neg_set: false,
            phase_current_limit: Complex64::ZERO,
            debug_trace: false,
            fpct_pmin_no_vars: 0.0,
            fpct_pmin_kvar_limit: 0.0,
            pmin_no_vars: 0.0,
            pmin_kvar_limit: 0.0,
            pf_wp_nominal: 0.0,
            force_balanced: false,
            current_limited: false,
            user_model_name: String::new(),
            user_model_edit: String::new(),
            first_sample_after_reset: true,
            dyneq: DynEqPceData::new(),
        }
    }

    /// Pascal `TInvBasedPCE.Get_Presentkvar`: `Qnominalperphase * 0.001 *
    /// Fnphases` — the present total reactive output in kvar.
    pub fn get_present_kvar(&self, nphases: usize) -> f64 {
        self.q_nominal_per_phase * 0.001 * nphases as f64
    }

    /// Pascal `TInvBasedPCE.UsingCIMDynamics`: `VWMode or VVMode or WVMode or
    /// AVRMode or DRCMode` (WPMode is deliberately excluded — "not in CIM
    /// Dynamics").
    pub fn using_cim_dynamics(&self) -> bool {
        self.vw_mode || self.vv_mode || self.wv_mode || self.avr_mode || self.drc_mode
    }

    /// Pascal `TInvBasedPCE.StickCurrInTerminalArray`: add `curr` into terminal
    /// array `arr` at conductor `i` (0-based), routed by connection — "Reverse of
    /// similar routine in load (Cnegates are switched)". Identical sign
    /// convention to [`Generator::stick_curr`](crate::elements::pc::generator).
    pub fn stick_curr_in_terminal_array(
        &self,
        arr: &mut [Complex64],
        nconds: usize,
        curr: Complex64,
        i: usize,
    ) {
        match self.connection {
            Connection::Wye => {
                arr[i] += curr;
                arr[nconds - 1] -= curr; // neutral
            }
            Connection::Delta => {
                arr[i] += curr;
                let j = if i + 1 >= nconds { 0 } else { i + 1 };
                arr[j] -= curr;
            }
        }
    }
}

impl Default for InvBasedPceData {
    fn default() -> Self {
        Self::new()
    }
}

/// The `TInvBasedPCE` virtual hooks the concrete inverter PC elements override.
/// Implemented by PVSystem (WP7.3 step 2) and Storage (WP7.4); the base
/// `Is*`/`GetPFPriority` defaults mirror the Pascal base bodies (all `False`).
/// The abstract behavioral hooks (`SetPFPriority`/`CheckOLInverter`/
/// `SetNominalDEROutput`) are added with the concrete classes that implement
/// them.
pub trait InvBasedPce {
    /// The embedded shared base data.
    fn inv_based(&self) -> &InvBasedPceData;
    /// The embedded shared base data, mutably.
    fn inv_based_mut(&mut self) -> &mut InvBasedPceData;

    /// Pascal `IsPVSystem` (base: `False`).
    fn is_pvsystem(&self) -> bool {
        false
    }
    /// Pascal `IsStorage` (base: `False`).
    fn is_storage(&self) -> bool {
        false
    }
    /// Pascal `GetPFPriority` (base: `False`).
    fn get_pf_priority(&self) -> bool {
        false
    }
}
