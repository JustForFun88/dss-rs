//! Spec-pinned unit tests for the PVSystem element (`TPVsystemObj`). The
//! numeric oracle pinning lives in the integration goldens (`der_controls/pvsystem*`)
//! and the live corpus gate; these pin the ported Pascal bodies that the oracle
//! does not expose directly (Create defaults, the inverter clamp branches,
//! `ComputePanelPower`, the YEQ derivation).

use crate::elements::general::load_shape::{self, LoadShapeObj};
use crate::elements::general::temp_shape::{self, TShapeObj};
use crate::elements::pc::generator::default_recalc_ctx;
use crate::elements::pc::inv_based_pce::{Connection, InvBasedPce};
use crate::elements::traits::SysCtx;
use crate::obj::base::DssObject;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::PropEngine;
use crate::solution::{SolveMode, USEDUTY, USENONE, USEYEARLY};
use crate::support::cmatrix::CMatrix;
use dss_parser::{Parser, ParserVars};
use num_complex::Complex64;

use super::*;

/// Build a `LoadShapeObj` (mult curve) through its real property engine.
fn build_shape(mult: &str) -> LoadShapeObj {
    let enums = EnumRegistry::new();
    let cls = load_shape::class_props(&enums);
    let mut obj = LoadShapeObj::new("s");
    let mut parser = Parser::new();
    let vars = ParserVars::new();
    let mut errors = crate::diag::ErrorLog::new();
    for (name, value) in [("npts", "4"), ("interval", "1"), ("mult", mult)] {
        let idx = cls.property_index(name).expect("known property");
        let mut eng = PropEngine {
            parser: &mut parser,
            vars: &vars,
            enums: &enums,
            errors: &mut errors,
            foreign: None,

            was_quoted: false,
        };
        cls.edit_property(&mut obj, idx, value, &mut eng).unwrap();
    }
    obj.end_edit(&crate::elements::traits::SysCtx::parse_default());
    assert!(errors.is_empty(), "{errors:?}");
    obj
}

/// Build a `TShapeObj` (temperature curve) through its real property engine.
fn build_tshape(temp: &str) -> TShapeObj {
    let enums = EnumRegistry::new();
    let cls = temp_shape::class_props(&enums);
    let mut obj = TShapeObj::new("t");
    let mut parser = Parser::new();
    let vars = ParserVars::new();
    let mut errors = crate::diag::ErrorLog::new();
    for (name, value) in [("npts", "4"), ("interval", "1"), ("temp", temp)] {
        let idx = cls.property_index(name).expect("known property");
        let mut eng = PropEngine {
            parser: &mut parser,
            vars: &vars,
            enums: &enums,
            errors: &mut errors,
            foreign: None,

            was_quoted: false,
        };
        cls.edit_property(&mut obj, idx, value, &mut eng).unwrap();
    }
    obj.end_edit(&crate::elements::traits::SysCtx::parse_default());
    obj
}

/// Edit a fresh `PVSystem` through its real property engine (string-edit path,
/// so the `PropFlags::REPLACE_ZERO` clamp in `set_obj_double` runs).
fn edit_pvsystem(edits: &[(&str, &str)]) -> PVSystem {
    let enums = EnumRegistry::new();
    let cls = super::class_props(&enums);
    let mut pv = PVSystem::new("pvz");
    let mut parser = Parser::new();
    let vars = ParserVars::new();
    let mut errors = crate::diag::ErrorLog::new();
    for (name, value) in edits {
        let idx = cls.property_index(name).expect("known property");
        let mut eng = PropEngine {
            parser: &mut parser,
            vars: &vars,
            enums: &enums,
            errors: &mut errors,
            foreign: None,

            was_quoted: false,
        };
        cls.edit_property(&mut pv, idx, value, &mut eng).unwrap();
    }
    assert!(errors.is_empty(), "{errors:?}");
    pv
}

/// UPGRADE_PLAN ledger L2 (WP-U1.1): a PVSystem `kVA` parsed as 0 clamps to
/// `1e-8` (EPRI r4133 `DblValueNZ`). PVSystem sizes on `Pmpp`/`kVA` (no `kW`
/// prop), so `kVA` is the only clamped rating; pins that `REPLACE_ZERO` stays on
/// it.
#[test]
fn zero_kva_clamp_dblvaluenz() {
    let pv = edit_pvsystem(&[("kVA", "0")]);
    assert_eq!(
        pv.f_kva_rating.to_bits(),
        1e-8f64.to_bits(),
        "kVA {}",
        pv.f_kva_rating
    );
    // Tiny in-band (incl. negative) also clamps to +1e-8.
    assert_eq!(
        edit_pvsystem(&[("kVA", "-2e-9")]).f_kva_rating.to_bits(),
        1e-8f64.to_bits()
    );
    // Out-of-band values are untouched.
    assert_eq!(edit_pvsystem(&[("kVA", "5000")]).f_kva_rating, 5000.0);
}

fn time_class_ctx(class: i32, dbl_hour: f64) -> SysCtx {
    SysCtx {
        mode: SolveMode::Time,
        active_load_shape_class: class,
        dbl_hour,
        ..default_recalc_ctx()
    }
}

/// Pascal `SetNominalPVSystem` GENERALTIME arm (PVsystem.pas:1174): under
/// `ActiveLoadShapeClass` (`Set LoadShapeClass=`) the class picks BOTH the mult
/// curve (`ShapeFactor`) AND the temperature curve (`TShapeValue`) — the two
/// travel together, so a swap that mixed them (e.g. yearly mult + daily temp)
/// is caught. Three distinct mult curves (hr 2: daily→0.6, yearly→0.7,
/// duty→0.5) and three distinct temp curves (yearly = daily+1, duty = daily+2);
/// default `USENONE` leaves ShapeFactor 1+j1 and the temperature at the fixed
/// `f_temperature` (25).
#[test]
fn time_loadshapeclass_selects_matching_mult_and_temperature() {
    let mut pv = PVSystem::new("pv1");
    pv.base.daily_shape_obj = Some(build_shape("0.2 0.6 1.0 0.5"));
    pv.base.yearly_shape_obj = Some(build_shape("0.3 0.7 0.9 0.4"));
    pv.base.duty_shape_obj = Some(build_shape("0.1 0.5 0.8 0.6"));
    pv.daily_t_shape_obj = Some(build_tshape("10 20 30 40"));
    pv.yearly_t_shape_obj = Some(build_tshape("11 21 31 41"));
    pv.duty_t_shape_obj = Some(build_tshape("12 22 32 42"));

    // The daily temp at hr 2 (whatever the wrap index): the yearly/duty curves
    // are that +1 / +2, so we assert relative to it to stay index-agnostic.
    pv.set_nominal_der_output(&time_class_ctx(crate::solution::USEDAILY, 2.0));
    let daily_temp = pv.t_shape_value;
    assert!((pv.base.shape_factor.re - 0.6).abs() < 1e-9);

    pv.set_nominal_der_output(&time_class_ctx(USEYEARLY, 2.0));
    assert!(
        (pv.base.shape_factor.re - 0.7).abs() < 1e-9,
        "yearly mult: {}",
        pv.base.shape_factor.re
    );
    assert!(
        (pv.t_shape_value - (daily_temp + 1.0)).abs() < 1e-9,
        "yearly temp {} != daily+1 {}",
        pv.t_shape_value,
        daily_temp + 1.0
    );

    pv.set_nominal_der_output(&time_class_ctx(USEDUTY, 2.0));
    assert!(
        (pv.base.shape_factor.re - 0.5).abs() < 1e-9,
        "duty mult: {}",
        pv.base.shape_factor.re
    );
    assert!(
        (pv.t_shape_value - (daily_temp + 2.0)).abs() < 1e-9,
        "duty temp {} != daily+2 {}",
        pv.t_shape_value,
        daily_temp + 2.0
    );

    pv.set_nominal_der_output(&time_class_ctx(USENONE, 2.0));
    assert!(
        (pv.base.shape_factor.re - 1.0).abs() < 1e-9,
        "none mult: {}",
        pv.base.shape_factor.re
    );
    assert!(
        (pv.t_shape_value - pv.f_temperature).abs() < 1e-9,
        "none temp {} != f_temperature {}",
        pv.t_shape_value,
        pv.f_temperature
    );
}

/// Pascal `InitStateVars`/`IntegrateStates` load-shape dispatch (PVsystem.pas
/// l.2192-2210 / l.2281-2299): **dynamics mode** honors `Set LoadShapeClass=` —
/// the class samples the mult AND temperature curves at the dynamics hour, so a
/// deck doing `set loadshapeclass=daily; set time=(10,0); set mode=dynamics`
/// starts at that hour's irradiance, not full sun. Regression for the CF2-G
/// GFL/GFM daily-deck bug (the port hardcoded `ShapeFactor := 1+j1`, injecting
/// PanelkW 800 instead of the oracle's 594.78). Also pins the `USENONE` arm:
/// only `ShapeFactor` resets — `TShapeValue` keeps its prior value (Pascal's
/// `else` touches nothing else).
#[test]
fn dynamics_loadshapeclass_selects_mult_and_temperature() {
    let mut pv = PVSystem::new("pv1");
    pv.base.daily_shape_obj = Some(build_shape("0.2 0.6 1.0 0.5"));
    pv.base.yearly_shape_obj = Some(build_shape("0.3 0.7 0.9 0.4"));
    pv.base.duty_shape_obj = Some(build_shape("0.1 0.5 0.8 0.6"));
    pv.daily_t_shape_obj = Some(build_tshape("10 20 30 40"));
    pv.yearly_t_shape_obj = Some(build_tshape("11 21 31 41"));
    pv.duty_t_shape_obj = Some(build_tshape("12 22 32 42"));

    let dyn_ctx = |class: i32| SysCtx {
        mode: SolveMode::Dynamic,
        is_dynamic_model: true,
        active_load_shape_class: class,
        dbl_hour: 2.0,
        ..default_recalc_ctx()
    };

    pv.apply_dynamics_load_shape(&dyn_ctx(crate::solution::USEDAILY));
    let daily_temp = pv.t_shape_value;
    assert!(
        (pv.base.shape_factor.re - 0.6).abs() < 1e-9,
        "daily mult: {}",
        pv.base.shape_factor.re
    );

    pv.apply_dynamics_load_shape(&dyn_ctx(USEYEARLY));
    assert!(
        (pv.base.shape_factor.re - 0.7).abs() < 1e-9,
        "yearly mult: {}",
        pv.base.shape_factor.re
    );
    assert!(
        (pv.t_shape_value - (daily_temp + 1.0)).abs() < 1e-9,
        "yearly temp {} != daily+1",
        pv.t_shape_value
    );

    pv.apply_dynamics_load_shape(&dyn_ctx(USEDUTY));
    assert!(
        (pv.base.shape_factor.re - 0.5).abs() < 1e-9,
        "duty mult: {}",
        pv.base.shape_factor.re
    );
    assert!(
        (pv.t_shape_value - (daily_temp + 2.0)).abs() < 1e-9,
        "duty temp {} != daily+2",
        pv.t_shape_value
    );

    // USENONE: ShapeFactor resets to 1+j1; TShapeValue keeps the prior (duty)
    // value — Pascal's `else` branch does NOT touch it.
    pv.apply_dynamics_load_shape(&dyn_ctx(USENONE));
    assert_eq!(pv.base.shape_factor, crate::util::CDOUBLEONE);
    assert!(
        (pv.t_shape_value - (daily_temp + 2.0)).abs() < 1e-9,
        "USENONE must not touch TShapeValue (got {})",
        pv.t_shape_value
    );
}

/// The harmonic-mode YPrim is the Thevenin admittance behind `%R`/`%X`
/// (`Yeq := 1/(Rthev + j·Xthev)`, then `Y.im /= h`) that `InitHarmonics` sets —
/// NOT the (negated) power-flow admittance. Pins the harmonic `CalcYPrimMatrix`
/// branch entry-by-entry and, as a discriminator, asserts it is far from the
/// power-flow stamping. Oracle-independent backstop for the
/// `harmonics/harmonics_pvsystem_h5` golden.
#[test]
fn harmonic_yprim_is_thevenin_admittance_not_powerflow() {
    let mut pv = PVSystem::new("pv1");
    // A representative power-flow Yeq (what SetNominalDEROutput leaves before harmonics).
    pv.base.yeq = Complex64::new(0.006, -0.0004);
    let pf_yeq = pv.base.yeq;
    // `InitHarmonics`: Yeq := 1/(Rthev + j·Xthev) (representative ohms).
    let z_thev = Complex64::new(25.0, 25.0);
    pv.r_thev = z_thev.re;
    pv.x_thev = z_thev.im;
    pv.base.yeq = z_thev.inv();

    let h = 5.0_f64;
    let sys = SysCtx {
        frequency: 60.0 * h,
        fundamental: 60.0,
        is_harmonic_model: true,
        ..default_recalc_ctx()
    };
    let mut ym = CMatrix::new(pv.cd.yorder);
    pv.calc_yprim_matrix(&mut ym, &sys);
    let actual = ym.get(0, 0); // wye phase-A diagonal

    let mut expected = z_thev.inv();
    expected.im /= h;
    assert!(
        (actual - expected).norm() < 1e-12,
        "harmonic YPrim {actual} != Thevenin admittance {expected}"
    );
    let mut naive = -pf_yeq;
    naive.im /= h;
    assert!(
        (actual - naive).norm() > 0.1 * naive.norm(),
        "harmonic YPrim {actual} indistinguishable from the power-flow path {naive}"
    );
}

/// Pascal `TPVsystemObj.Create` defaults.
#[test]
fn create_defaults() {
    let pv = PVSystem::new("pv1");
    assert_eq!(pv.cd.nphases, 3);
    assert_eq!(pv.cd.nconds, 4); // wye
    assert_eq!(pv.base.connection, Connection::Wye);
    assert_eq!(pv.base.voltage_model, 1);
    assert_eq!(pv.kv_pvsystem_base, 12.47);
    assert_eq!(pv.f_kva_rating, 500.0);
    assert_eq!(pv.f_pmpp, 500.0);
    assert_eq!(pv.f_pu_pmpp, 1.0);
    assert_eq!(pv.f_irradiance, 1.0);
    assert_eq!(pv.f_temperature, 25.0);
    assert_eq!(pv.base.vminpu, 0.90);
    assert_eq!(pv.base.vmaxpu, 1.10);
    assert_eq!(pv.base.var_mode, VarMode::Pf);
    assert!(pv.base.inverter_on);
    assert_eq!(pv.base.pf_nominal, 1.0);
    assert_eq!(pv.base.pct_r, 50.0);
    assert_eq!(pv.base.pct_x, 0.0);
    assert_eq!(pv.base.fpct_cut_in, 20.0);
    assert_eq!(pv.base.fpct_cut_out, 20.0);
    // dynVars Create overrides.
    assert_eq!(pv.base.dyn_vars.rated_vdc, 8000.0);
    assert_eq!(pv.base.dyn_vars.sm_threshold, 80.0);
    assert_eq!(pv.base.dyn_vars.kp, 0.00001);
    // InvBasedPCE base dynVars defaults.
    assert_eq!(pv.base.dyn_vars.i_limit, -1.0);
    assert_eq!(pv.base.dyn_vars.v_error, 0.8);
    // The inverter virtual hook.
    assert!(pv.is_pvsystem());
}

/// After `RecalcElementData`, the default PV (PF = 1, irradiance = 1, Pmpp =
/// 500, ideal inverter) delivers its full panel power at unity PF: kW_out = 500,
/// kvar_out = 0, and YEQ = (P − jQ)/Vbase² with Q = 0.
#[test]
fn default_recalc_yields_full_kw_unity_pf() {
    // `new` no longer recalcs (no live ctx at construction); recalc explicitly
    // with the parse-time default, exactly as the executive does at create.
    let mut pv = PVSystem::new("pv1");
    pv.recalc(&SysCtx::parse_default());
    // Panel kW = irradiance(1) · shape(1) · Pmpp(500) · tempfactor(1) = 500.
    assert_eq!(pv.panel_kw, 500.0);
    assert_eq!(pv.base.kw_out, 500.0);
    assert_eq!(pv.base.kvar_out, 0.0);
    assert!(pv.base.inverter_on);
    let nphases = pv.cd.nphases as f64;
    assert_eq!(pv.base.p_nominal_per_phase, 1000.0 * 500.0 / nphases);
    assert_eq!(pv.base.q_nominal_per_phase, 0.0);
    // VBase L-N = 12.47 kV / sqrt(3) · 1000.
    let vbase = pv.base.v_base;
    let expect_yeq = num_complex::Complex64::new(pv.base.p_nominal_per_phase, 0.0) / vbase.powi(2);
    assert!((pv.base.yeq.re - expect_yeq.re).abs() < 1e-9);
    assert_eq!(pv.base.yeq.im, 0.0);
}

/// `Get_PresentkW` = Pnominalperphase · 0.001 · nphases (= the nominal kW).
#[test]
fn present_kw_kvar_round_trip() {
    let mut pv = PVSystem::new("pv1");
    pv.recalc(&SysCtx::parse_default());
    assert!((pv.present_kw() - 500.0).abs() < 1e-9);
    assert_eq!(pv.present_kvar(), 0.0);
}

/// The inverter cut-out: when the panel power drops below CutOutkW
/// (= %Cutout·kVA/100 = 20%·500/100 = 100 kW), the inverter turns OFF and
/// kW_out collapses to 0. Driven by setting a tiny irradiance and recomputing.
#[test]
fn inverter_cuts_out_below_threshold() {
    let mut pv = PVSystem::new("pv1");
    assert!(pv.base.inverter_on);
    // CutOutkW = 100; drive panel power well below it.
    pv.set_f64(prop::IRRADIANCE, 0.1); // 0.1·500 = 50 kW < 100
    pv.recalc(&crate::elements::pc::generator::default_recalc_ctx());
    assert!(!pv.base.inverter_on);
    assert_eq!(pv.base.kw_out, 0.0);
}

/// The non-priority kVA clamp: with kvar=400 requested (varMode=KVAR) on a
/// kVA=500 inverter and full panel power (kw would be 500), apparent power
/// 640 > 500 forces the no-priority back-off `kW_out := sqrt(kVA²−kvar²)` → kW =
/// sqrt(500²−400²) = **300**, kvar **stays 400**. (Pinned exactly — not just an
/// upper bound — so a regression that zeroed the output or backed off the wrong
/// leg can't pass; the oracle pins the same state in golden `der_controls/pvsystem_clamps`
/// element `pva`.)
#[test]
fn kva_clamp_backs_off_kw() {
    let mut pv = PVSystem::new("pv1");
    pv.set_i32(prop::CONN, 0);
    // Request kvar; switch to kvar mode.
    pv.set_f64(prop::KVAR, 400.0);
    pv.side_effects(prop::KVAR, 0);
    pv.recalc(&crate::elements::pc::generator::default_recalc_ctx());
    assert!(
        (pv.base.kw_out - 300.0).abs() < 1e-9,
        "kw_out = {}",
        pv.base.kw_out
    );
    assert!(
        (pv.base.kvar_out - 400.0).abs() < 1e-9,
        "kvar_out = {}",
        pv.base.kvar_out
    );
}

/// The negative-kvar absorption clamp + back-off: kvar=−400 requested with
/// `kvarMaxAbs`=300 clamps to kvar_out=−300 (absorption limit), then the kVA
/// back-off sets kW = sqrt(500²−300²) = **400**. Pins the absorption direction
/// (the `kvarNEG` corpus sibling that would cover it is deferred at ~4e-6;
/// the oracle pins this state in golden `der_controls/pvsystem_clamps` element `pvc`).
#[test]
fn kvar_absorption_clamp_then_backoff() {
    let mut pv = PVSystem::new("pv1");
    pv.set_i32(prop::CONN, 0);
    pv.set_f64(prop::KVAR_MAX_ABS, 300.0);
    pv.side_effects(prop::KVAR_MAX_ABS, 0);
    pv.set_f64(prop::KVAR, -400.0);
    pv.side_effects(prop::KVAR, 0);
    pv.recalc(&crate::elements::pc::generator::default_recalc_ctx());
    assert!(
        (pv.base.kvar_out + 300.0).abs() < 1e-9,
        "kvar_out = {}",
        pv.base.kvar_out
    );
    assert!(
        (pv.base.kw_out - 400.0).abs() < 1e-9,
        "kw_out = {}",
        pv.base.kw_out
    );
}

// --- MakePosSequence (WPG.21) --------------------------------------------

use crate::elements::pos_seq::{PosSeqAction, PosSeqCtx};
use crate::elements::traits::CktElement;

/// 3-phase PVSystem: V line-neutral, kVA ÷ phases, PF set to nominal.
#[test]
fn makeposseq_pv_three_phase() {
    let mut pv = PVSystem::new("pv");
    pv.base.connection = Connection::Wye;
    pv.cd.nphases = 3;
    pv.kv_pvsystem_base = 12.47;
    pv.f_kva_rating = 150.0;
    pv.base.pf_nominal = 1.0;

    let plan = pv.make_pos_sequence(&PosSeqCtx::default());
    assert!(plan.run_base);
    let v = 12.47 / 3.0_f64.sqrt();
    assert_eq!(
        plan.actions,
        vec![
            PosSeqAction::BeginEdit,
            PosSeqAction::SetI32(prop::PHASES, 1),
            PosSeqAction::SetI32(prop::CONN, 0),
            PosSeqAction::SetF64(prop::KV, v),
            PosSeqAction::SetF64(prop::KVA, 150.0 / 3.0),
            PosSeqAction::SetF64(prop::PF, 1.0),
            PosSeqAction::EndEdit,
        ]
    );
}

/// 1-phase PVSystem: base kV kept, no kVA/PF split.
#[test]
fn makeposseq_pv_single_phase() {
    let mut pv = PVSystem::new("pv");
    pv.base.connection = Connection::Wye;
    pv.cd.nphases = 1;
    pv.kv_pvsystem_base = 7.2;
    pv.f_kva_rating = 150.0;

    let plan = pv.make_pos_sequence(&PosSeqCtx::default());
    assert_eq!(
        plan.actions,
        vec![
            PosSeqAction::BeginEdit,
            PosSeqAction::SetI32(prop::PHASES, 1),
            PosSeqAction::SetI32(prop::CONN, 0),
            PosSeqAction::SetF64(prop::KV, 7.2),
            PosSeqAction::EndEdit,
        ]
    );
}

/// Pascal `TInvBasedPCE.GetCurrents` GFM override (InvBasedPCE.pas l.211-219): in
/// **grid-forming** mode `GetCurrents` never calls `inherited`, so the base
/// `TPCElement.GetCurrents` `LastSolutionWasDirect` shortcut (PCElement.pas l.137)
/// MUST NOT fire — a GFM inverter in DIRECT mode still reports `YPrim·V −
/// InjCurrent`, not the frozen `YPrim·V`. The non-GFM arm (the port's `!gfm_mode
/// && pc_direct_shortcut()` guard) DOES take the shortcut. Guards the
/// class-specific `!self.base.gfm_mode` condition — the only new branch with no
/// live-deck coverage (every vendored `mode=direct` deck has the direct Solve
/// commented out).
#[test]
fn direct_shortcut_excluded_in_gfm_mode() {
    use crate::elements::traits::CktElement;

    let node_v = vec![
        Complex64::ZERO, // ground slot
        Complex64::new(7000.0, 0.0),
        Complex64::new(-3500.0, -6062.0),
        Complex64::new(-3500.0, 6062.0),
    ];
    let inj = Complex64::new(9.0, -4.0);

    let build = |gfm: bool| -> PVSystem {
        let mut pv = PVSystem::new("pv1");
        pv.base.gfm_mode = gfm;
        CktElement::calc_yprim(&mut pv, &default_recalc_ctx()); // sizes yorder + buffers
        let n = pv.cd.yorder;
        let mut yp = CMatrix::new(n);
        for i in 0..n {
            yp.set(i, i, Complex64::new(0.01, -0.02));
        }
        pv.cd.yprim = Some(yp);
        pv.cd.set_node_ref(1, &[1, 2, 3, 0]);
        pv.cd.inj_current = vec![inj; n];
        pv.cd.iterminal_solution_count = Some(0); // == solution_count → skip model recompute
        pv
    };

    // The shortcut result YPrim·Vterminal (computed independently).
    let n = 4usize;
    let mut yp = CMatrix::new(n);
    for i in 0..n {
        yp.set(i, i, Complex64::new(0.01, -0.02));
    }
    let vterm: Vec<Complex64> = [1usize, 2, 3, 0].iter().map(|&r| node_v[r]).collect();
    let mut yprim_v = vec![Complex64::ZERO; n];
    yp.mv_mult(&mut yprim_v, &vterm);

    let sys_direct = SysCtx {
        last_solution_was_direct: true,
        solution_count: 0, // == default iterminal_solution_count → model skipped
        ..default_recalc_ctx()
    };

    // GFM: shortcut EXCLUDED → YPrim·V − InjCurrent.
    let mut pv_gfm = build(true);
    let mut i_gfm = vec![Complex64::ZERO; n];
    pv_gfm.get_currents(&sys_direct, &node_v, &mut i_gfm);
    for k in 0..n {
        let expect = yprim_v[k] - inj;
        assert!(
            (i_gfm[k] - expect).norm() < 1e-9,
            "GFM direct read [{k}] {} != YPrim·V−Inj {}",
            i_gfm[k],
            expect
        );
    }

    // Non-GFM: shortcut TAKEN → YPrim·V (no InjCurrent).
    let mut pv_pf = build(false);
    let mut i_pf = vec![Complex64::ZERO; n];
    pv_pf.get_currents(&sys_direct, &node_v, &mut i_pf);
    for k in 0..n {
        assert!(
            (i_pf[k] - yprim_v[k]).norm() < 1e-9,
            "non-GFM direct read [{k}] {} != YPrim·V {}",
            i_pf[k],
            yprim_v[k]
        );
    }

    // The exclusion is observable: the two differ by exactly InjCurrent.
    assert!(
        (i_pf[0] - i_gfm[0]).norm() > 1.0,
        "GFM exclusion not observable: non-GFM {} vs GFM {}",
        i_pf[0],
        i_gfm[0]
    );
}
