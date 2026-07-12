use super::*;

use crate::elements::general::load_shape::{self, LoadShapeObj};
use crate::elements::traits::SysCtx;
use crate::obj::base::DssObject;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::PropEngine;
use crate::solution::{SolveMode, USEDUTY, USENONE, USEYEARLY};
use dss_parser::{Parser, ParserVars};
use num_complex::Complex64;

fn snap_ctx() -> SysCtx {
    default_recalc_ctx()
}

/// Build a populated `LoadShapeObj` through its real property engine.
fn build_shape(mult: &str) -> LoadShapeObj {
    let enums = EnumRegistry::new();
    let cls = load_shape::class_props(&enums);
    let mut obj = LoadShapeObj::new("s");
    let mut parser = Parser::new();
    let vars = ParserVars::new();
    let mut errors = Vec::new();
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
        ..default_recalc_ctx()
    }
}

/// Edit a fresh `Generator` through its real property engine (string-edit path,
/// so the `PropFlags::REPLACE_ZERO` clamp in `set_obj_double` runs).
fn edit_generator(edits: &[(&str, &str)]) -> Generator {
    let enums = EnumRegistry::new();
    let cls = super::class_props(&enums);
    let mut g = Generator::new("gz");
    let mut parser = Parser::new();
    let vars = ParserVars::new();
    let mut errors = Vec::new();
    for (name, value) in edits {
        let idx = cls.property_index(name).expect("known property");
        let mut eng = PropEngine {
            parser: &mut parser,
            vars: &vars,
            enums: &enums,
            errors: &mut errors,
            foreign: None,
        };
        cls.edit_property(&mut g, idx, value, &mut eng).unwrap();
    }
    assert!(errors.is_empty(), "{errors:?}");
    g
}

/// UPGRADE_PLAN ledger L2 (WP-U1.1): a Generator `kW`/`kVA` parsed as 0 clamps
/// to `1e-8` (EPRI r4133 `DblValueNZ`) — but `MVA` (prop 27) does NOT (r4133
/// `generator.pas:664` uses plain `DblValue*1000`). Pins both the adopted clamp
/// and the deliberate MVA asymmetry, so removing `REPLACE_ZERO` from kW/kVA — or
/// re-adding it to MVA (the crashed-draft regression) — fails here.
#[test]
fn zero_kw_kva_clamp_dblvaluenz() {
    let g = edit_generator(&[("kW", "0"), ("kVA", "0")]);
    assert_eq!(g.kw_base.to_bits(), 1e-8f64.to_bits(), "kW {}", g.kw_base);
    assert_eq!(
        g.kva_rating.to_bits(),
        1e-8f64.to_bits(),
        "kVA {}",
        g.kva_rating
    );
    // Tiny in-band (incl. negative) also clamps to +1e-8.
    assert_eq!(
        edit_generator(&[("kW", "4e-9")]).kw_base.to_bits(),
        1e-8f64.to_bits()
    );
    // MVA (prop 27) is NOT clamped: `MVA=0` -> kVA rating a literal 0 (had it
    // been clamped, the pre-scale 1e-8 * 1000 would leave 1e-5, not 0).
    assert_eq!(edit_generator(&[("MVA", "0")]).kva_rating, 0.0);
    // Out-of-band values are untouched (MVA carries its *1000 scale).
    assert_eq!(edit_generator(&[("kW", "250")]).kw_base, 250.0);
    assert_eq!(edit_generator(&[("MVA", "2")]).kva_rating, 2000.0);
}

/// Pascal `SetNominalGeneration` GENERALTIME arm (Generator.pas:1129): the
/// `ActiveLoadShapeClass` (`Set LoadShapeClass=`) picks WHICH of the three
/// distinct curves drives `ShapeFactor` (at hr 2: daily→0.6, yearly→0.7,
/// duty→0.5); default `USENONE` leaves it 1+j1. Gates a copy-paste arm swap.
#[test]
fn time_loadshapeclass_selects_matching_curve() {
    let mut g = Generator::new("g1");
    g.daily_shape_obj = Some(build_shape("0.2 0.6 1.0 0.5"));
    g.yearly_shape_obj = Some(build_shape("0.3 0.7 0.9 0.4"));
    g.duty_shape_obj = Some(build_shape("0.1 0.5 0.8 0.6"));

    g.set_nominal_generation(&time_class_ctx(USEYEARLY, 2.0));
    assert!(
        (g.shape_factor.re - 0.7).abs() < 1e-9,
        "yearly: {}",
        g.shape_factor.re
    );
    g.set_nominal_generation(&time_class_ctx(USEDUTY, 2.0));
    assert!(
        (g.shape_factor.re - 0.5).abs() < 1e-9,
        "duty: {}",
        g.shape_factor.re
    );
    g.set_nominal_generation(&time_class_ctx(USENONE, 2.0));
    assert!(
        (g.shape_factor.re - 1.0).abs() < 1e-9,
        "none: {}",
        g.shape_factor.re
    );
}

/// A default generator's nominal quantities (oracle dss-python 0.15.7):
/// kW=1000, kvar=60, 3-phase, Vbase=7200 L-N.
#[test]
fn default_nominal_generation() {
    let mut g = Generator::new("g1");
    g.set_nominal_generation(&snap_ctx());
    // Pnom = 1000*1000*1*1/3, Qnom = 1000*60*1*1/3
    assert!((g.p_nominal_per_phase - 333333.3333).abs() < 1e-3);
    assert!((g.q_nominal_per_phase - 20000.0).abs() < 1e-6);
    // Yeq = (Pnom - jQnom)/Vbase^2
    let yeq = Complex64::new(333333.3333, -20000.0) / 7200.0_f64.powi(2);
    assert!((g.yeq.re - yeq.re).abs() < 1e-9);
    assert!((g.yeq.im - yeq.im).abs() < 1e-9);
}

/// The harmonic-mode YPrim is the **subtransient** admittance behind Xd"
/// (`Yeq := 1/(j·Xd")`, then `Y.im /= h`) that `InitHarmonics` sets — NOT the
/// frequency-scaled power-flow admittance. Pins it entry-by-entry and, as a
/// discriminator, asserts it is far from the power-flow path: a regression that
/// forgot to overwrite `Yeq` in `init_harmonics_impl` (reusing the power-flow
/// `Yeq`) fails here. Oracle-independent — the offline backstop the
/// `harmonics/harmonics_generator_h5` golden complements (mirrors the step-1 Load
/// `harmonic_yprim_uses_series_rl_split_not_naive_yeq` discriminator).
#[test]
fn harmonic_yprim_is_subtransient_admittance_not_powerflow() {
    let mut g = Generator::new("g1");
    // Establish the power-flow Yeq, like a snapshot solve does before harmonics.
    g.set_nominal_generation(&snap_ctx());
    let pf_yeq = g.yeq;
    assert!(pf_yeq.norm() > 0.0, "power-flow Yeq not established");

    // `InitHarmonics` overwrites Yeq with the L-N subtransient admittance and
    // leaves the generator on.
    g.gen_on = true;
    g.yeq = Complex64::new(0.0, g.xdpp).inv();

    let h = 5.0_f64;
    let sys = SysCtx {
        frequency: 60.0 * h,
        fundamental: 60.0,
        is_harmonic_model: true,
        ..default_recalc_ctx()
    };
    let mut ym = CMatrix::new(g.cd.yorder);
    g.calc_yprim_matrix(&mut ym, &sys);
    let actual = ym.get(0, 0); // wye phase-A diagonal

    // Expected: Y := 1/(j·Xd"), reactive part scaled by the harmonic.
    let mut expected = Complex64::new(0.0, g.xdpp).inv();
    expected.im /= h;
    assert!(
        (actual - expected).norm() < 1e-9,
        "harmonic YPrim {actual} != subtransient admittance {expected}"
    );

    // Discriminator: the (negated, freq-scaled) power-flow admittance is far off.
    let mut naive = -pf_yeq;
    naive.im /= h;
    assert!(
        (actual - naive).norm() > 0.1 * naive.norm(),
        "harmonic YPrim {actual} indistinguishable from the power-flow path {naive}"
    );
}

/// kW/PF web: setting kW=100, PF=0.95 derives kvar via SyncUpPowerQuantities.
#[test]
fn kw_pf_sets_kvar() {
    let mut g = Generator::new("g1");
    g.kw_base = 100.0;
    g.pf_nominal = 0.95;
    g.sync_up_power_quantities();
    // kvar = kW*sqrt(1/pf^2 - 1) = 100*0.328684
    assert!((g.kvar_base - 32.8684).abs() < 1e-3, "kvar {}", g.kvar_base);
    // kVA not set → tracks kW*1.2
    assert!((g.kva_rating - 120.0).abs() < 1e-9);
}

/// kva explicitly set freezes kVArating (kVANotSet=false).
#[test]
fn kva_set_freezes_rating() {
    let mut g = Generator::new("g1");
    g.kva_rating = 500.0;
    g.kva_not_set = false;
    g.kw_base = 100.0;
    g.pf_nominal = 0.95;
    g.sync_up_power_quantities();
    assert!((g.kva_rating - 500.0).abs() < 1e-9);
}

/// Off-state (LOADMODE dispatch below the reference) → tiny resistive load.
#[test]
fn off_state_is_tiny_resistive_load() {
    let mut g = Generator::new("g1");
    g.dispatch_mode = LOADMODE;
    g.dispatch_value = 2.0;
    let mut sys = snap_ctx();
    sys.generator_dispatch_reference = 1.0; // below dispatch_value → OFF
    g.set_nominal_generation(&sys);
    assert!(!g.gen_on);
    assert!((g.p_nominal_per_phase - (-0.1 * 1000.0 / 3.0)).abs() < 1e-9);
    assert_eq!(g.q_nominal_per_phase, 0.0);
}

/// A delta generator carries `nphases` conductors (3-phase).
#[test]
fn delta_connection_nconds() {
    let mut g = Generator::new("g1");
    g.set_i32(prop::CONN, 1); // delta
    g.side_effects(prop::CONN, 0);
    assert_eq!(g.cd.nconds, 3);
}

/// Status=Fixed forces factor=1 regardless of gen_multiplier.
#[test]
fn fixed_status_ignores_gen_multiplier() {
    let mut g = Generator::new("g1");
    g.is_fixed = true;
    let mut sys = snap_ctx();
    sys.gen_multiplier = 0.5;
    g.set_nominal_generation(&sys);
    // factor=1 → Pnom = 1000*1000/3 (gen_multiplier ignored)
    assert!((g.p_nominal_per_phase - 333333.3333).abs() < 1e-3);
}

/// TakeSample integrates kWh/kvarh over an interval (plain Euler).
#[test]
fn take_sample_accumulates_energy() {
    let mut g = Generator::new("g1");
    g.set_nominal_generation(&snap_ctx());
    g.gen_on = true;
    g.reset_registers();
    // present kW = Pnom*0.001*3 = 1000; present kvar = 60
    g.take_sample(1.0, false, false, 25.0);
    assert!((g.registers[REG_KWH] - 1000.0).abs() < 1e-6);
    assert!((g.registers[REG_KVARH] - 60.0).abs() < 1e-6);
    assert!((g.registers[REG_MAXKW] - 1000.0).abs() < 1e-6);
    assert!((g.registers[REG_HOURS] - 1.0).abs() < 1e-9);
}

// --- MakePosSequence (WPG.21) --------------------------------------------

use crate::elements::pos_seq::{PosSeqAction, PosSeqCtx};
use crate::elements::traits::CktElement;

/// 3-phase, kw=200 pf=0.95 kva=250, maxkvar=120 minkvar=-60; no props marked.
fn gen_3ph() -> Generator {
    let mut g = Generator::new("g");
    g.connection = Connection::Wye;
    g.cd.nphases = 3;
    g.kv_generator_base = 12.47;
    g.kw_base = 200.0;
    g.pf_nominal = 0.95;
    g.kva_rating = 250.0;
    g.kvar_max = 120.0;
    g.kvar_min = -60.0;
    g
}

fn common_head(v: f64) -> Vec<PosSeqAction> {
    vec![
        PosSeqAction::BeginEdit,
        PosSeqAction::SetI32(prop::PHASES, 1),
        PosSeqAction::SetI32(prop::CONN, 0),
        PosSeqAction::SetF64(prop::KV, v),
        PosSeqAction::SetF64(prop::KW, 200.0 / 3.0),
        PosSeqAction::SetF64(prop::PF, 0.95),
    ]
}

/// Plain (kW+pf only, nothing else marked): kW/pf ÷ phases, no kvar/kVA/MVA.
#[test]
fn makeposseq_generator_plain() {
    let mut g = gen_3ph();
    let plan = g.make_pos_sequence(&PosSeqCtx::default());
    assert!(plan.run_base);
    let v = 12.47 / 3.0_f64.sqrt();
    let mut expect = common_head(v);
    expect.push(PosSeqAction::EndEdit);
    assert_eq!(plan.actions, expect);
}

/// `kVA=` set (PrpSequence slot 23). The upstream `had_kVA` guard reads slot 26
/// (`Xdp`), so this does NOT trigger a kVA divide — kVA stays as-is (oracle:
/// `g_kva` keeps kVA=250). Pins the `generator.pas:2744` wrong-index quirk.
#[test]
fn makeposseq_generator_kva_set_is_ignored_wrong_index() {
    let mut g = gen_3ph();
    g.cd.obj.set_as_next_seq(prop::KVA); // slot 23
    let plan = g.make_pos_sequence(&PosSeqCtx::default());
    let v = 12.47 / 3.0_f64.sqrt();
    let mut expect = common_head(v);
    expect.push(PosSeqAction::EndEdit);
    assert_eq!(plan.actions, expect, "kVA= must not divide (reads slot 26)");
    assert!(!plan.actions.iter().any(|a| matches!(
        a,
        PosSeqAction::SetF64(i, _) if *i == prop::KVA
    )));
}

/// `MVA=` set (slot 24). `had_MVA` reads slot 27 (`Xdpp`) → no MVA action.
#[test]
fn makeposseq_generator_mva_set_is_ignored_wrong_index() {
    let mut g = gen_3ph();
    g.cd.obj.set_as_next_seq(prop::MVA); // slot 24
    let plan = g.make_pos_sequence(&PosSeqCtx::default());
    let v = 12.47 / 3.0_f64.sqrt();
    let mut expect = common_head(v);
    expect.push(PosSeqAction::EndEdit);
    assert_eq!(plan.actions, expect);
}

/// `maxkvar=`/`minkvar=` set (slots 19/20 — the CORRECT indices): `had_kvars`
/// fires, emitting minkvar then maxkvar ÷ phases (120→40, -60→-20).
#[test]
fn makeposseq_generator_kvars_divided() {
    let mut g = gen_3ph();
    g.cd.obj.set_as_next_seq(prop::MAXKVAR); // slot 19
    g.cd.obj.set_as_next_seq(prop::MINKVAR); // slot 20
    let plan = g.make_pos_sequence(&PosSeqCtx::default());
    let v = 12.47 / 3.0_f64.sqrt();
    let mut expect = common_head(v);
    expect.push(PosSeqAction::SetF64(prop::MINKVAR, -60.0 / 3.0)); // -20
    expect.push(PosSeqAction::SetF64(prop::MAXKVAR, 120.0 / 3.0)); // 40
    expect.push(PosSeqAction::EndEdit);
    assert_eq!(plan.actions, expect);
}

/// Setting `Xdp=` (slot 26) is what actually trips `had_kVA` — the wrong-index
/// quirk in reverse: kVA IS divided even though the user never touched kVA.
#[test]
fn makeposseq_generator_xdp_trips_kva_divide() {
    let mut g = gen_3ph();
    g.cd.obj.set_as_next_seq(prop::XDP); // slot 26 == the buggy had_kVA index
    let plan = g.make_pos_sequence(&PosSeqCtx::default());
    assert!(plan.actions.iter().any(|a| matches!(
        a,
        PosSeqAction::SetF64(i, val) if *i == prop::KVA && (*val - 250.0 / 3.0).abs() < 1e-9
    )));
}

/// 1-phase generator: V stays base kV, and NO power split (oldPhases==1).
#[test]
fn makeposseq_generator_single_phase() {
    let mut g = gen_3ph();
    g.cd.nphases = 1;
    let plan = g.make_pos_sequence(&PosSeqCtx::default());
    assert_eq!(
        plan.actions,
        vec![
            PosSeqAction::BeginEdit,
            PosSeqAction::SetI32(prop::PHASES, 1),
            PosSeqAction::SetI32(prop::CONN, 0),
            PosSeqAction::SetF64(prop::KV, 12.47), // NOT /√3
            PosSeqAction::EndEdit,
        ]
    );
}
