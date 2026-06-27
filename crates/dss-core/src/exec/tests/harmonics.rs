//! Harmonics solve-mode integration tests (WP7.6 step 1): the end-to-end
//! `Solve mode=harmonics` pipeline for the current-source family (VSource +
//! Load), and the loud deferral guard for the DER family.

use crate::exec::*;

/// A weak VSource feeding a nonlinear Load through a line. Solve the
/// fundamental, then sweep only the 5th harmonic: the Load becomes an ideal
/// harmonic current source (its `defaultload` spectrum injects 20% at the 5th),
/// and that current flows through the frequency-scaled network to non-zero 5th
/// harmonic voltages.
fn harmonic_test_dss() -> Dss {
    let mut dss = Dss::new();
    dss.command("New circuit.harm basekv=12.47 pu=1.0 phases=3 mvasc3=20000 mvasc1=21000");
    dss.command("New Line.l1 bus1=sourcebus bus2=loadbus length=1 units=km r1=0.1 x1=0.3 c1=3.4");
    dss.command("New Load.ld1 bus1=loadbus phases=3 kv=12.47 kw=1000 pf=0.9 model=1");
    dss.command("Set voltagebases=[12.47]");
    dss.command("CalcVoltageBases");
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "fundamental: {:?}", dss.errors());
    dss
}

fn load_current_mag(dss: &mut Dss) -> f64 {
    let snap = dss.snapshot_elements();
    let ld = snap
        .iter()
        .find(|e| e.name.eq_ignore_ascii_case("Load.ld1"))
        .expect("load snapshot");
    // Phase-A terminal current (re, im).
    (ld.currents[0].powi(2) + ld.currents[1].powi(2)).sqrt()
}

#[test]
fn fifth_harmonic_injects_load_spectrum_current() {
    let mut dss = harmonic_test_dss();
    let i_fundamental = load_current_mag(&mut dss);
    assert!(
        i_fundamental > 1.0,
        "fundamental load current {i_fundamental}"
    );

    dss.command("Set harmonics=(5)");
    dss.command("Set mode=harmonics");
    assert!(dss.errors().is_empty(), "set mode: {:?}", dss.errors());
    dss.command("Solve");
    assert!(
        dss.errors().is_empty(),
        "harmonic solve: {:?}",
        dss.errors()
    );

    let ckt = dss.circuit().unwrap();
    assert!(ckt.is_solved);
    // The sweep leaves the solution at the 5th harmonic (300 Hz).
    assert!(
        (ckt.solution.harmonic - 5.0).abs() < 1e-9,
        "{}",
        ckt.solution.harmonic
    );
    assert!((ckt.solution.frequency - 300.0).abs() < 1e-9);

    // The load injects 20% of its fundamental current (defaultload %mag at h=5),
    // captured as its terminal current in harmonic mode.
    let i_h5 = load_current_mag(&mut dss);
    let ratio = i_h5 / i_fundamental;
    assert!(
        (ratio - 0.20).abs() < 1e-3,
        "5th-harmonic/fundamental load current ratio {ratio} (expected ~0.20)"
    );

    // The 5th-harmonic load-bus voltage is a small, non-zero distortion.
    let ckt = dss.circuit().unwrap();
    let vmax = (1..=ckt.num_nodes)
        .map(|i| ckt.solution.node_v[i].norm())
        .fold(0.0_f64, f64::max);
    assert!(vmax > 0.0, "all harmonic voltages zero");
    assert!(
        vmax < 12.47e3,
        "harmonic voltage {vmax} not a small distortion"
    );
}

#[test]
fn default_all_harmonics_sweeps_load_spectrum() {
    // No `Set harmonics=`: DoAllHarmonics collects the load's spectrum (1,3,5,
    // 7,9,11,13) and solves each non-fundamental one. The solution ends at the
    // largest (13th → 780 Hz).
    let mut dss = harmonic_test_dss();
    dss.command("Set mode=harmonics");
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let ckt = dss.circuit().unwrap();
    assert!(
        (ckt.solution.frequency - 780.0).abs() < 1e-9,
        "{}",
        ckt.solution.frequency
    );
    assert!((ckt.solution.harmonic - 13.0).abs() < 1e-9);
}

#[test]
fn second_harmonic_solve_restores_saved_voltages() {
    // After a sweep the frequency sits at the last harmonic; a second `solve`
    // must reset to the fundamental and reload the saved voltages
    // (`RetrieveSavedVoltages`) before re-sweeping — and reproduce the result
    // bit-for-bit (the restore is deterministic).
    let mut dss = harmonic_test_dss();
    dss.command("Set harmonics=(5)");
    dss.command("Set mode=harmonics");
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "first: {:?}", dss.errors());
    let v1 = load_current_mag(&mut dss);
    assert!(
        (dss.circuit().unwrap().solution.frequency - 300.0).abs() < 1e-9,
        "sweep should leave frequency at the 5th harmonic"
    );

    // Second solve: frequency != fundamental at entry → restore + re-sweep.
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "second: {:?}", dss.errors());
    let v2 = load_current_mag(&mut dss);
    assert!(
        (v1 - v2).abs() < 1e-9,
        "re-entrant harmonic solve drifted: {v1} vs {v2}"
    );
}

/// The voltage-source-behind-reactance DER family (Generator/PVSystem/Storage)
/// defers its harmonic injection to WP7.6 step 2 and must refuse the solve
/// loudly rather than run its power-flow injection at a harmonic frequency.
fn assert_der_harmonic_deferral(der_new: &str, full_name: &str) {
    let mut dss = Dss::new();
    dss.command("New circuit.d basekv=12.47 pu=1.0 phases=3 mvasc3=20000 mvasc1=21000");
    dss.command("New Line.l1 bus1=sourcebus bus2=db length=1 units=km r1=0.1 x1=0.3");
    dss.command(der_new);
    dss.command("Set voltagebases=[12.47]");
    dss.command("CalcVoltageBases");
    dss.command("Solve");
    assert!(
        dss.errors().is_empty(),
        "{full_name} fundamental: {:?}",
        dss.errors()
    );

    dss.command("Set harmonics=(5)");
    dss.command("Set mode=harmonics");
    dss.command("Solve");
    let errs = dss.errors().join("\n");
    assert!(
        errs.contains(full_name) && errs.contains("WP7.6 step 2"),
        "expected a loud {full_name} harmonic deferral, got: {errs:?}"
    );
}

#[test]
fn generator_in_harmonic_mode_aborts_loudly() {
    assert_der_harmonic_deferral(
        "New Generator.g1 bus1=db kv=12.47 kw=100 pf=0.95 model=1",
        "Generator.g1",
    );
}

#[test]
fn pvsystem_in_harmonic_mode_aborts_loudly() {
    assert_der_harmonic_deferral(
        "New PVSystem.pv1 bus1=db phases=3 kv=12.47 kva=100 pmpp=100 irradiance=1.0",
        "PVSystem.pv1",
    );
}

#[test]
fn storage_in_harmonic_mode_aborts_loudly() {
    assert_der_harmonic_deferral(
        "New Storage.st1 bus1=db phases=3 kv=12.47 kwrated=100 kwhrated=200 state=idling",
        "Storage.st1",
    );
}
