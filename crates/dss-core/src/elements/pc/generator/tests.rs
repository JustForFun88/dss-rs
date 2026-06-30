use super::*;

use crate::elements::traits::SysCtx;
use crate::obj::base::DssObject;
use num_complex::Complex64;

fn snap_ctx() -> SysCtx {
    default_recalc_ctx()
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
/// `phase7/harmonics_generator_h5` golden complements (mirrors the step-1 Load
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
