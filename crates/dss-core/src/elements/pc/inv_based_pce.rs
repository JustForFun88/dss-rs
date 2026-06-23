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
//! The grid-forming-mode (GFM) / dynamics machinery — the `TInvDynamicVars`
//! arrays + `SolveDynamicStep`/`SolveModulation`/`CalcGFM*`/`InitDynArrays`, the
//! `PICtrl` PI-controller array, `CheckAmpsLimit`, the GFM `GetCurrents`
//! override, and the `DynEqPCE` dynamics memory (`DynamicEqVals`/`DynamicEqPair`/
//! `UserDynInit`) — is WP7.7 (dynamics).
//!
//! [`Generator`]: crate::elements::pc::generator::Generator

#[cfg(test)]
mod tests;

use num_complex::Complex64;

use crate::elements::general::dynamic_exp::DynamicExpObj;
use crate::elements::general::load_shape::LoadShapeObj;
use crate::elements::general::xy_curve::XyCurveObj;
use crate::elements::traits::ElemRef;

/// Pascal `Connection: Integer` on `TInvBasedPCE` (0 = line-neutral/wye,
/// 1 = delta). Mirrors `generator::Connection`; kept local so the inverter base
/// does not depend on the Generator module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Connection {
    Wye = 0,
    Delta = 1,
}

/// Pascal `TInvDynamicVars` (`Shared/InvDynamics.pas`) — the per-inverter
/// dynamics/GFM state record. Only the **scalar** fields are ported here (they
/// back catalog properties and the base `Create` defaults); the per-phase array
/// fields (`Vgrid`/`it`/`dit`/`itHistory`/`VDelta`/`ISPDelta`/`AngDelta`/`m`/
/// `SfModePhase`) and every method (`SolveDynamicStep`/`SolveModulation`/
/// `FixPhaseAngle`/`CalcGFMYprim`/`CalcGFMVoltage`/`InitDynArrays`/
/// `Get_InvDynValue`/`Set_InvDynValue`/`Get_InvDynName`) are dynamics-only —
/// WP7.7.
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
}

impl InvDynamicVars {
    /// Pascal `TInvBasedPCE.Create`'s `with dynVars` block: `ILimit := -1`
    /// (no amps limit), `IComp := 0`, `VError := 0.8`. Every other scalar starts
    /// at 0/false and is set by the concrete subclass `Create`/`RecalcElementData`.
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
    /// `dynVars` — the dynamics/GFM scalar state (arrays/methods: WP7.7).
    pub dyn_vars: InvDynamicVars,
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

    /// `UserModelNameStr` — user-DLL model name (NOT_PORTED in safe Rust; stored
    /// only for the dump).
    pub user_model_name: String,
    /// `UserModelEditStr` — user-DLL model edit string (NOT_PORTED).
    pub user_model_edit: String,

    /// `FirstSampleAfterReset`.
    pub first_sample_after_reset: bool,

    // DynEqPCE-inherited fields.
    /// `DynamicEqObj` — the linked `DynamicExp` object. Name kept for the dump;
    /// the object is a snapshot clone (WP4.2/WP5.3 pattern). The dynamics memory
    /// it integrates (`DynamicEqVals`/`DynamicEqPair`/`UserDynInit`) is WP7.7.
    pub dynamic_eq: String,
    pub dynamic_eq_obj: Option<DynamicExpObj>,
    pub dynamic_eq_ref: Option<ElemRef>,
    /// `DynOut` — output-variable selection (the `DynOut=` string as written;
    /// its resolution to `DynamicExp` output indices and dynamics effect is
    /// WP7.7).
    pub dyn_out: String,
}

impl InvBasedPceData {
    /// Pascal `TInvBasedPCE.Create`: `GFM_Mode := FALSE`; the `dynVars` defaults
    /// (`ILimit/IComp/VError`); shape and inverter-curve objects nil. Every other
    /// field starts at 0/false/empty and is set by the concrete subclass.
    pub fn new() -> Self {
        Self {
            dyn_vars: InvDynamicVars::new(),
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
            dynamic_eq: String::new(),
            dynamic_eq_obj: None,
            dynamic_eq_ref: None,
            dyn_out: String::new(),
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
