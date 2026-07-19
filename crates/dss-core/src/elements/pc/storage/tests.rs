//! Spec-pinned unit tests for the Storage element (`TStorageObj`). The numeric
//! oracle pinning lives in the integration goldens (`der_controls/storage*`) and the
//! live corpus gate; these pin the ported Pascal bodies that the oracle does not
//! expose directly (Create defaults, the state machine, `ComputePresentkW`, the
//! inverter clamp, the `%stored` read/write).

use crate::elements::general::load_shape::{self, LoadShapeObj};
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

fn ctx() -> crate::elements::traits::SysCtx {
    crate::elements::pc::generator::default_recalc_ctx()
}

/// Edit a fresh `Storage` through its real property engine (string-edit path, so
/// the `PropFlags::REPLACE_ZERO` clamp in `set_obj_double` runs).
fn edit_storage(edits: &[(&str, &str)]) -> Storage {
    let enums = EnumRegistry::new();
    let cls = super::class_props(&enums);
    let mut st = Storage::new("sz");
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
        };
        cls.edit_property(&mut st, idx, value, &mut eng).unwrap();
    }
    assert!(errors.is_empty(), "{errors:?}");
    st
}

/// UPGRADE_PLAN ledger L2 (WP-U1.1): a Storage `kW`/`kVA` parsed as 0 clamps to
/// `1e-8` (EPRI r4133 `DblValueNZ`). `kVA` is the direct rating field; `kW` runs
/// through `Set_kW`, where the clamped `+1e-8` (>0) resolves the state to
/// DISCHARGING — a literal 0 would resolve to IDLING, so the state is the
/// discrete witness that the clamp fired.
#[test]
fn zero_kw_kva_clamp_dblvaluenz() {
    let st = edit_storage(&[("kVA", "0")]);
    assert_eq!(
        st.f_kva_rating.to_bits(),
        1e-8f64.to_bits(),
        "kVA {}",
        st.f_kva_rating
    );
    // kW=0 clamps to +1e-8 before `Set_kW`: state DISCHARGING, not IDLING.
    let st = edit_storage(&[("kW", "0")]);
    assert_eq!(st.f_state, STORE_DISCHARGING, "state {}", st.f_state);
    // Out-of-band kVA is untouched.
    assert_eq!(edit_storage(&[("kVA", "25")]).f_kva_rating, 25.0);
}

/// Build a populated `LoadShapeObj` through its real property engine.
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
        };
        cls.edit_property(&mut obj, idx, value, &mut eng).unwrap();
    }
    obj.end_edit();
    assert!(errors.is_empty(), "{errors:?}");
    obj
}

fn time_class_ctx(class: i32, dbl_hour: f64) -> SysCtx {
    SysCtx {
        mode: SolveMode::Time,
        active_load_shape_class: class,
        dbl_hour,
        ..ctx()
    }
}

/// Pascal `SetNominalStorage` GENERALTIME arm (Storage.pas:1318): under the
/// DEFAULT dispatch, `ActiveLoadShapeClass` (`Set LoadShapeClass=`) picks WHICH
/// of the three distinct curves sets `ShapeFactor` (at hr 2: daily→0.6,
/// yearly→0.7, duty→0.5); default `USENONE` leaves it 1+j1. Gates an arm swap.
#[test]
fn time_loadshapeclass_selects_matching_curve() {
    let mut st = Storage::new("s1");
    assert_eq!(st.dispatch_mode, StorageDispatchMode::Default); // the `_ =>` mode-dispatch path
    st.base.daily_shape_obj = Some(build_shape("0.2 0.6 1.0 0.5"));
    st.base.yearly_shape_obj = Some(build_shape("0.3 0.7 0.9 0.4"));
    st.base.duty_shape_obj = Some(build_shape("0.1 0.5 0.8 0.6"));

    st.set_nominal_der_output(&time_class_ctx(USEYEARLY, 2.0));
    assert!(
        (st.base.shape_factor.re - 0.7).abs() < 1e-9,
        "yearly: {}",
        st.base.shape_factor.re
    );
    st.set_nominal_der_output(&time_class_ctx(USEDUTY, 2.0));
    assert!(
        (st.base.shape_factor.re - 0.5).abs() < 1e-9,
        "duty: {}",
        st.base.shape_factor.re
    );
    st.set_nominal_der_output(&time_class_ctx(USENONE, 2.0));
    assert!(
        (st.base.shape_factor.re - 1.0).abs() < 1e-9,
        "none: {}",
        st.base.shape_factor.re
    );
}

/// The harmonic-mode YPrim is the Thevenin admittance behind `%R`/`%X`
/// (`Yeq := 1/(Rthev + j·Xthev)`, then `Y.im /= h`) that `InitHarmonics` sets —
/// NOT the state-dependent power-flow admittance. Pins the harmonic
/// `CalcYPrimMatrix` branch entry-by-entry and discriminates it from the
/// power-flow stamping. Oracle-independent backstop for the
/// `harmonics/harmonics_storage_h5` golden.
#[test]
fn harmonic_yprim_is_thevenin_admittance_not_powerflow() {
    let mut st = Storage::new("s1");
    // A representative power-flow discharge admittance (the state-dependent
    // YeqDischarge the power-flow branch would stamp).
    st.yeq_discharge = Complex64::new(0.006, -0.0004);
    let pf_yeq = st.yeq_discharge;
    // `InitHarmonics`: Yeq := 1/(Rthev + j·Xthev) (representative ohms).
    let z_thev = Complex64::new(25.0, 25.0);
    st.r_thev = z_thev.re;
    st.x_thev = z_thev.im;
    st.base.yeq = z_thev.inv();

    let h = 5.0_f64;
    let sys = SysCtx {
        frequency: 60.0 * h,
        fundamental: 60.0,
        is_harmonic_model: true,
        ..ctx()
    };
    let mut ym = CMatrix::new(st.cd.yorder);
    st.calc_yprim_matrix(&mut ym, &sys);
    let actual = ym.get(0, 0); // wye phase-A diagonal

    let mut expected = z_thev.inv();
    expected.im /= h;
    assert!(
        (actual - expected).norm() < 1e-12,
        "harmonic YPrim {actual} != Thevenin admittance {expected}"
    );
    // Discriminator: the discharging power-flow stamping (−YeqDischarge, freq-scaled).
    let mut naive = -pf_yeq;
    naive.im /= h;
    assert!(
        (actual - naive).norm() > 0.1 * naive.norm(),
        "harmonic YPrim {actual} indistinguishable from the power-flow path {naive}"
    );
}

/// Pascal `TStorageObj.Create` defaults.
#[test]
fn create_defaults() {
    let st = Storage::new("s1");
    assert_eq!(st.cd.nphases, 3);
    assert_eq!(st.cd.nconds, 4); // wye
    assert_eq!(st.base.connection, Connection::Wye);
    assert_eq!(st.base.voltage_model, 1);
    assert_eq!(st.kv_storage_base, 12.47);
    assert_eq!(st.kw_rating, 25.0);
    assert_eq!(st.f_kva_rating, 25.0);
    assert_eq!(st.kwh_rating, 50.0);
    assert_eq!(st.kwh_stored, 50.0);
    assert_eq!(st.pct_reserve, 20.0);
    assert_eq!(st.kwh_reserve, 10.0); // kWhRating·pctReserve/100
    assert_eq!(st.f_state, STORE_IDLING);
    assert_eq!(st.dispatch_mode, StorageDispatchMode::Default);
    assert_eq!(st.pct_kw_out, 100.0);
    assert_eq!(st.pct_kw_in, 100.0);
    assert_eq!(st.pct_idle_kw, 1.0);
    assert_eq!(st.pct_charge_eff, 90.0);
    assert_eq!(st.pct_discharge_eff, 90.0);
    assert_eq!(st.charge_time, 2.0);
    assert_eq!(st.base.pct_r, 0.0);
    assert_eq!(st.base.pct_x, 50.0);
    assert_eq!(st.base.vminpu, 0.90);
    assert_eq!(st.base.vmaxpu, 1.10);
    assert_eq!(st.base.var_mode, VARMODE_PF);
    assert!(st.base.inverter_on);
    assert_eq!(st.base.pf_nominal, 1.0);
    assert_eq!(st.f_kvar_limit, 25.0); // = FkVArating
    assert_eq!(st.f_kvar_limit_neg, 25.0);
    // dynVars Create overrides.
    assert_eq!(st.base.dyn_vars.rated_vdc, 8000.0);
    assert_eq!(st.base.dyn_vars.sm_threshold, 80.0);
    assert_eq!(st.base.dyn_vars.kp, 0.00001);
    assert_eq!(st.base.dyn_vars.i_limit, -1.0);
    assert_eq!(st.base.dyn_vars.v_error, 0.8);
    // The inverter virtual hook.
    assert!(st.is_storage());
    // PIdling = %IdlingkW·kWrating/100 = 0.25; ideal inverter → kWOutIdling = 0.25.
    assert_eq!(st.p_idling, 0.25);
    assert_eq!(st.kw_out_idling, 0.25);
}

/// A default (idling, fully-charged) Storage draws only its idling losses:
/// `kW_out = −kWOutIdling = −0.25`, matching the oracle `? kW` = −0.25.
#[test]
fn idle_draws_only_idling_losses() {
    let st = Storage::new("s1");
    assert_eq!(st.f_state, STORE_IDLING);
    assert!((st.base.kw_out + 0.25).abs() < 1e-12);
    assert!((st.present_kw() + 0.25).abs() < 1e-9);
    assert_eq!(st.present_kvar(), 0.0);
}

/// Setting `State=Discharging` delivers `kWrating·%Discharge = 25` at the
/// terminal (default `%Discharge = 100`, ideal inverter, no cut-in/out).
#[test]
fn discharging_delivers_rated_kw() {
    let mut st = Storage::new("s1");
    st.set_i32(prop::STATE, STORE_DISCHARGING);
    st.recalc(&ctx());
    assert_eq!(st.f_state, STORE_DISCHARGING);
    assert!(
        (st.base.kw_out - 25.0).abs() < 1e-9,
        "kw_out = {}",
        st.base.kw_out
    );
    assert!((st.present_kw() - 25.0).abs() < 1e-9);
}

/// `Set_kW(value)` sets the state + dispatch %: a positive kW discharges,
/// `%Discharge = kW/kWrating·100`. kW=10 → 40% → terminal kW = 10.
#[test]
fn set_kw_positive_discharges() {
    let mut st = Storage::new("s1");
    st.set_f64(prop::KW, 10.0); // Pascal SetkW
    assert_eq!(st.f_state, STORE_DISCHARGING);
    assert_eq!(st.pct_kw_out, 40.0);
    st.recalc(&ctx());
    assert!((st.present_kw() - 10.0).abs() < 1e-9);
}

/// A negative kW charges (absorbs) when the battery is not full: with `%Stored`
/// dropped to 50 (kWhStored=25 < 50) and kW=−5, the terminal kW is −5.
#[test]
fn set_kw_negative_charges_when_not_full() {
    let mut st = Storage::new("s1");
    st.set_f64(prop::PCT_STORED, 50.0); // kWhStored = 25
    assert_eq!(st.kwh_stored, 25.0);
    st.set_f64(prop::KW, -5.0);
    assert_eq!(st.f_state, STORE_CHARGING);
    assert_eq!(st.pct_kw_in, 20.0);
    st.recalc(&ctx());
    assert!(
        (st.present_kw() + 5.0).abs() < 1e-9,
        "present_kw = {}",
        st.present_kw()
    );
}

/// A charge command on a *full* battery (default kWhStored = kWhRating) falls
/// straight back to idling in `ComputePresentkW`.
#[test]
fn charging_full_battery_falls_to_idling() {
    let mut st = Storage::new("s1");
    st.set_f64(prop::KW, -5.0);
    assert_eq!(st.f_state, STORE_CHARGING);
    st.recalc(&ctx()); // kWhStored = kWhRating → state flips to idling
    assert_eq!(st.f_state, STORE_IDLING);
    assert!((st.base.kw_out + 0.25).abs() < 1e-9);
}

/// `%Stored` round-trips through `kWhStored`: write `%Stored=40` →
/// `kWhStored = 0.40·kWhRating = 20`; read back 40.
#[test]
fn pct_stored_round_trip() {
    let mut st = Storage::new("s1");
    st.set_f64(prop::PCT_STORED, 40.0);
    assert_eq!(st.kwh_stored, 20.0);
    assert!((st.get_f64(prop::PCT_STORED) - 40.0).abs() < 1e-9);
}

/// `kWhRated` side effect: setting the energy rating refills the battery and
/// recomputes the reserve (`kWhStored := kWhRating`, `kWhReserve := kWhRating·
/// %Reserve/100`).
#[test]
fn kwh_rated_side_effect_recharges() {
    let mut st = Storage::new("s1");
    st.set_f64(prop::KWH_RATED, 100.0);
    st.side_effects(prop::KWH_RATED, 0);
    assert_eq!(st.kwh_stored, 100.0);
    assert_eq!(st.kwh_before_update, 100.0);
    assert_eq!(st.kwh_reserve, 20.0); // 100·20/100
}

/// The discharging kVA clamp: with PF=0.8 requested (varMode=PF) on a kVA=25
/// inverter discharging at 25 kW, the desired kvar = 25·√(1/0.8²−1) = 18.75
/// gives kVA √(25²+18.75²)=31.25 > 25, so the default Q-priority path backs off
/// **kW** to √(25²−18.75²) = 16.5359… while **kvar stays 18.75**. Both legs are
/// pinned exactly (not just the apparent power) so a regression that backs off
/// the wrong leg — e.g. a PF-priority kW=20/kvar=15, which also lands on the
/// kVA=25 circle with both legs positive — cannot pass. Oracle: `? kW`
/// = 16.5359456941537, `? kvar` = 18.75.
#[test]
fn kva_clamp_backs_off_kw_on_pf() {
    let mut st = Storage::new("s1");
    st.set_i32(prop::STATE, STORE_DISCHARGING);
    st.set_f64(prop::PF, 0.8);
    st.side_effects(prop::PF, 0);
    st.recalc(&ctx());
    assert!(
        (st.base.kw_out - 16.535_945_694_153_7).abs() < 1e-9,
        "kw_out = {}",
        st.base.kw_out
    );
    assert!(
        (st.base.kvar_out - 18.75).abs() < 1e-9,
        "kvar_out = {}",
        st.base.kvar_out
    );
}

/// `ControlMode=GFM` round-trips but its solve behavior is WP7.7; `gfm_mode` is
/// set and the per-element solve guard rejects it. (The solve-time error is
/// exercised by the exec integration test.)
#[test]
fn control_mode_gfm_sets_flag() {
    let mut st = Storage::new("s1");
    st.set_i32(prop::CONTROL_MODE, 1);
    st.side_effects(prop::CONTROL_MODE, 0);
    assert!(st.base.gfm_mode);
}

/// Pascal `TInvBasedPCE.GetCurrents` GFM override (InvBasedPCE.pas l.211-219): in
/// **grid-forming** mode `GetCurrents` never calls `inherited`, so the base
/// `TPCElement.GetCurrents` `LastSolutionWasDirect` shortcut (PCElement.pas l.137)
/// MUST NOT fire — a GFM unit in DIRECT mode still reports `YPrim·V − InjCurrent`,
/// not the frozen `YPrim·V`. The non-GFM arm (the port's `!gfm_mode &&
/// pc_direct_shortcut()` guard) DOES take the shortcut. Guards the class-specific
/// `!self.base.gfm_mode` condition — the only new branch with no live-deck
/// coverage (every vendored `mode=direct` deck has its direct Solve commented out).
#[test]
fn direct_shortcut_excluded_in_gfm_mode() {
    use crate::elements::traits::CktElement;

    let node_v = vec![
        Complex64::ZERO, // ground slot
        Complex64::new(7000.0, 0.0),
        Complex64::new(-3500.0, -6062.0),
        Complex64::new(-3500.0, 6062.0),
    ];
    let inj = Complex64::new(12.0, -5.0);

    // A known diagonal YPrim + nonzero injection so the shortcut (YPrim·V) and
    // the model current (YPrim·V − InjCurrent) differ by whole amps.
    let build = |gfm: bool| -> Storage {
        let mut st = Storage::new("s1");
        st.base.gfm_mode = gfm;
        CktElement::calc_yprim(&mut st, &ctx()); // sizes yorder + buffers
        let n = st.cd.yorder;
        let mut yp = CMatrix::new(n);
        for i in 0..n {
            yp.set(i, i, Complex64::new(0.01, -0.02));
        }
        st.cd.yprim = Some(yp);
        st.cd.set_node_ref(1, &[1, 2, 3, 0]);
        st.cd.inj_current = vec![inj; n];
        st.cd.iterminal_solution_count = 0; // == solution_count → skip model recompute
        st
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
        ..ctx()
    };

    // GFM: shortcut EXCLUDED → YPrim·V − InjCurrent.
    let mut st_gfm = build(true);
    let mut i_gfm = vec![Complex64::ZERO; n];
    st_gfm.get_currents(&sys_direct, &node_v, &mut i_gfm);
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
    let mut st_pf = build(false);
    let mut i_pf = vec![Complex64::ZERO; n];
    st_pf.get_currents(&sys_direct, &node_v, &mut i_pf);
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

// --- MakePosSequence (WPG.21) --------------------------------------------

use crate::elements::pos_seq::{PosSeqAction, PosSeqCtx};
use crate::elements::traits::CktElement;

/// 3-phase Storage: kWrated ÷ phases, PF set. The Pascal body has NO leading
/// `BeginEdit` (each `Set*` auto-brackets) and a dangling trailing `EndEdit`,
/// so the plan starts with a bare `Set*` and ends with a lone `EndEdit`.
#[test]
fn makeposseq_storage_three_phase_no_begin_edit() {
    let mut st = Storage::new("s");
    st.base.connection = Connection::Wye;
    st.cd.nphases = 3;
    st.kv_storage_base = 12.47;
    st.kw_rating = 100.0;
    st.base.pf_nominal = 1.0;

    let plan = st.make_pos_sequence(&PosSeqCtx::default());
    assert!(plan.run_base);
    // No BeginEdit anywhere; exactly one trailing EndEdit.
    assert!(!plan.actions.contains(&PosSeqAction::BeginEdit));
    assert_eq!(plan.actions.last(), Some(&PosSeqAction::EndEdit));
    let v = 12.47 / 3.0_f64.sqrt();
    assert_eq!(
        plan.actions,
        vec![
            PosSeqAction::SetI32(prop::PHASES, 1),
            PosSeqAction::SetI32(prop::CONN, 0),
            PosSeqAction::SetF64(prop::KV, v),
            PosSeqAction::SetF64(prop::KW_RATED, 100.0 / 3.0),
            PosSeqAction::SetF64(prop::PF, 1.0),
            PosSeqAction::EndEdit,
        ]
    );
}

/// 1-phase Storage: base kV kept, no kW/PF split, still the dangling EndEdit.
#[test]
fn makeposseq_storage_single_phase() {
    let mut st = Storage::new("s");
    st.base.connection = Connection::Wye;
    st.cd.nphases = 1;
    st.kv_storage_base = 7.2;
    st.kw_rating = 100.0;

    let plan = st.make_pos_sequence(&PosSeqCtx::default());
    assert_eq!(
        plan.actions,
        vec![
            PosSeqAction::SetI32(prop::PHASES, 1),
            PosSeqAction::SetI32(prop::CONN, 0),
            PosSeqAction::SetF64(prop::KV, 7.2),
            PosSeqAction::EndEdit,
        ]
    );
}

// --- WASM_USERMODELS WM.4: DynaDLL/DynaData/UserModel/UserData property surface ---

/// Edit one property through the real property engine, returning any messages
/// the side effect queued on the object.
fn edit_storage_prop(st: &mut Storage, name: &str, value: &str) -> crate::diag::ErrorLog {
    let enums = EnumRegistry::new();
    let cls = super::class_props(&enums);
    let idx = cls.property_index(name).expect("known Storage property");
    let mut parser = Parser::new();
    let vars = ParserVars::new();
    let mut errors = crate::diag::ErrorLog::new();
    let mut eng = PropEngine {
        parser: &mut parser,
        vars: &vars,
        enums: &enums,
        errors: &mut errors,
        foreign: None,
    };
    cls.edit_property(st, idx, value, &mut eng).unwrap();
    let mut msgs = errors;
    msgs.extend(st.cd.obj.take_errors());
    msgs
}

/// `DynaDLL=<dll>` parses (no longer a hard NOT_PORTED error) and stores the
/// name for the dump; the load is DEFERRED to the executive (like a FileLoad),
/// so the property set does NOT warn. Resolving the queued load for a native-DLL
/// name (no `.wasm` → `wasm = None`) is the path that warns "Not Loaded" (Pascal
/// 1570) and falls back to the built-in dynamics model (WASM_USERMODELS WM.4).
#[test]
fn dyna_dll_stores_and_warns_not_loaded() {
    use crate::obj::base::DssObject;
    let mut st = Storage::new("s1");
    let msgs = edit_storage_prop(&mut st, "DynaDLL", "Dess1.DLL");
    assert_eq!(st.dyna_model_name, "Dess1.DLL");
    assert_eq!(st.get_string(prop::DYNA_DLL), "Dess1.DLL"); // dump parity
    assert!(
        msgs.is_empty(),
        "property set must not warn (load is deferred): {msgs:?}"
    );

    let loads = st.take_user_model_loads();
    assert_eq!(loads.len(), 1, "one deferred DynaDLL load: {loads:?}");
    let mut errors = crate::diag::ErrorLog::new();
    st.apply_user_model_load(&loads[0], None, &mut errors);
    assert_eq!(errors.len(), 1, "exactly one warning: {errors:?}");
    assert!(errors[0].contains("Not Loaded"));
    assert!(errors[0].contains("Dess1.DLL"));
    assert!(errors[0].contains("built-in model"));
}

/// `UserModel=<dll>` (15-fn `TStoreUserModel`) — same deferred warn+fallback for
/// a native-DLL name (Pascal Storage.pas:851-854, 1570).
#[test]
fn storage_user_model_stores_and_warns_not_loaded() {
    use crate::obj::base::DssObject;
    let mut st = Storage::new("s1");
    let msgs = edit_storage_prop(&mut st, "UserModel", "Dess1.DLL");
    assert_eq!(st.base.user_model_name, "Dess1.DLL");
    assert!(msgs.is_empty(), "property set must not warn: {msgs:?}");

    let loads = st.take_user_model_loads();
    assert_eq!(loads.len(), 1, "one deferred UserModel load: {loads:?}");
    let mut errors = crate::diag::ErrorLog::new();
    st.apply_user_model_load(&loads[0], None, &mut errors);
    assert_eq!(errors.len(), 1, "exactly one warning: {errors:?}");
    assert!(errors[0].contains("Not Loaded"));
    assert!(errors[0].contains("built-in model"));
}

/// `DynaData` stores (for the dump) and — no dynamics model exists — is a
/// silent no-op (Pascal `if DynaModel.Exists then Edit`).
#[test]
fn dyna_data_stores_without_warning() {
    let mut st = Storage::new("s1");
    let msgs = edit_storage_prop(&mut st, "DynaData", "(file=DESSModel_Test.TxT)");
    assert_eq!(st.dyna_model_edit, "(file=DESSModel_Test.TxT)");
    assert_eq!(st.get_string(prop::DYNA_DATA), "(file=DESSModel_Test.TxT)");
    assert!(msgs.is_empty(), "DynaData must not warn: {msgs:?}");
}

/// Pascal `Set_Name` bails on a blank / `none` name — no warning.
#[test]
fn dyna_dll_none_does_not_warn() {
    let mut st = Storage::new("s1");
    let msgs = edit_storage_prop(&mut st, "DynaDLL", "none");
    assert!(msgs.is_empty(), "none must not warn: {msgs:?}");
}

#[test]
fn storage_dispatch_mode_pins_enum_ordinals() {
    assert_eq!(StorageDispatchMode::Default.ordinal(), 0);
    assert_eq!(StorageDispatchMode::LoadMode.ordinal(), 1);
    assert_eq!(StorageDispatchMode::PriceMode.ordinal(), 2);
    assert_eq!(StorageDispatchMode::ExternalMode.ordinal(), 3);
    assert_eq!(StorageDispatchMode::Follow.ordinal(), 4);
    for (ord, m) in [
        (0, StorageDispatchMode::Default),
        (1, StorageDispatchMode::LoadMode),
        (2, StorageDispatchMode::PriceMode),
        (3, StorageDispatchMode::ExternalMode),
        (4, StorageDispatchMode::Follow),
    ] {
        assert_eq!(StorageDispatchMode::from_ordinal(ord), Some(m));
    }
    assert_eq!(StorageDispatchMode::from_ordinal(5), None);
    assert_eq!(StorageDispatchMode::from_ordinal(-1), None);
}
