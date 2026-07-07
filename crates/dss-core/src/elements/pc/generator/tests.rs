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
