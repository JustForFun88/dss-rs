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
    (ld.currents[0].re.powi(2) + ld.currents[0].im.powi(2)).sqrt()
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

#[test]
fn harmonic_mode_relabels_monitor_time_columns() {
    // Pascal `ClearMonitorStream` (Monitor.pas l.709): the two leading time
    // columns are `hour`/`t(sec)` at fundamental and `Freq`/`Harmonic` in
    // harmonics mode. Entering harmonics resets every monitor (Pascal `Set_Mode`
    // tail), which rebuilds the header from the now-committed `IsHarmonicModel`.
    // (Oracle-independent: the C-API `Monitors.Header` strips these two columns,
    // so only the offline header surface can pin the relabel.)
    let mut dss = Dss::new();
    dss.command("New circuit.harm basekv=12.47 pu=1.0 phases=3 mvasc3=20000 mvasc1=21000");
    dss.command("New Line.l1 bus1=sourcebus bus2=loadbus length=1 units=km r1=0.1 x1=0.3 c1=3.4");
    dss.command("New Load.ld1 bus1=loadbus phases=3 kv=12.47 kw=1000 pf=0.9 model=1");
    dss.command("New Monitor.m element=Load.ld1 terminal=1 mode=0");
    dss.command("Set voltagebases=[12.47]");
    dss.command("CalcVoltageBases");
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "fundamental: {:?}", dss.errors());
    let hdr0 = dss.monitor_view("m").expect("m").header;
    assert_eq!(
        &hdr0[0..2],
        ["hour".to_string(), "t(sec)".to_string()],
        "fundamental-mode time columns"
    );

    dss.command("Set harmonics=(5)");
    dss.command("Set mode=harmonics");
    assert!(dss.errors().is_empty(), "set mode: {:?}", dss.errors());
    let hdr_h = dss.monitor_view("m").expect("m").header;
    assert_eq!(
        &hdr_h[0..2],
        ["Freq".to_string(), "Harmonic".to_string()],
        "harmonics mode must relabel the monitor time columns"
    );
    // The relabel touches only the two time columns — the V/I data channels are
    // unchanged (the golden `harmonics_doall` pins those values against the
    // oracle; here we confirm the relabel does not disturb them).
    assert_eq!(&hdr_h[2..], &hdr0[2..], "data channels must be unchanged");

    // The harmonic sweep then samples the fundamental + the 5th into the
    // relabelled monitor.
    dss.command("Solve");
    assert!(
        dss.errors().is_empty(),
        "harmonic solve: {:?}",
        dss.errors()
    );
    let m = dss.monitor_view("m").expect("m");
    assert_eq!(
        &m.header[0..2],
        ["Freq".to_string(), "Harmonic".to_string()]
    );
    assert_eq!(m.sample_count, 2, "fundamental + 5th harmonic samples");
    // In harmonics mode the two time slots hold the *values* (Pascal `TakeSample`
    // l.1202-1206 stores Frequency/Harmonic, not hour/sec), so slot 0 (`dbl_hour`)
    // is the swept frequency: the fundamental 60 Hz then the 5th 300 Hz. (Pins the
    // harmonic time-slot value offline — the oracle's `Monitors.Header` can't.)
    assert_eq!(
        m.dbl_hour,
        vec![60.0, 300.0],
        "harmonic time slot 0 holds Freq (fundamental, then 5th)"
    );
}

#[test]
fn mode_change_resets_monitor_buffer() {
    // Pascal `Set_Mode` tail (Solution.pas l.2133): `MonitorClass.ResetAll`
    // clears every monitor's buffer on a mode change, so samples accumulated
    // under one mode never carry into the next.
    let mut dss = harmonic_test_dss();
    dss.command("New Monitor.m element=Line.l1 terminal=1 mode=0");
    dss.command("Set mode=daily number=2");
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "daily: {:?}", dss.errors());
    assert_eq!(
        dss.monitor_view("m").expect("m").sample_count,
        2,
        "daily(2) accumulates one sample per step"
    );

    // Switching mode resets the buffer.
    dss.command("Set mode=snapshot");
    assert_eq!(
        dss.monitor_view("m").expect("m").sample_count,
        0,
        "a mode change must clear the monitor buffer"
    );
}

/// The voltage-source-behind-reactance DER family (Generator/PVSystem/Storage)
/// injects harmonic current from its spectrum in harmonics mode (WP7.6 step 2):
/// the solve completes (no loud deferral) and the spectrum-driven injection
/// produces a non-zero harmonic distortion voltage.
fn assert_der_harmonic_injects(der_new: &str, full_name: &str) {
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
    assert!(
        dss.errors().is_empty(),
        "{full_name} harmonic solve: {:?}",
        dss.errors()
    );

    let ckt = dss.circuit().unwrap();
    assert!(ckt.is_solved, "{full_name}: not solved");
    // The sweep leaves the solution at the 5th harmonic (300 Hz).
    assert!((ckt.solution.harmonic - 5.0).abs() < 1e-9, "{full_name}");
    assert!((ckt.solution.frequency - 300.0).abs() < 1e-9, "{full_name}");
    // The DER injected 5th-harmonic current from its spectrum → a genuine small
    // distortion voltage: materially non-zero (>0.1 V — a near-zero broken
    // injection fails), but well under 5% of the L-N nominal (~7200 V) since the
    // spectrum injects only a few % at the 5th and the network attenuates it. The
    // exact magnitude is pinned by `harmonics/harmonics_*_h5` (oracle) and the
    // offline `harmonic_yprim_*` discriminator unit tests; this just brackets the
    // smoke envelope on the real solve.
    let vmax = (1..=ckt.num_nodes)
        .map(|i| ckt.solution.node_v[i].norm())
        .fold(0.0_f64, f64::max);
    assert!(
        (0.1..0.05 * 7200.0).contains(&vmax),
        "{full_name}: harmonic distortion {vmax} V not a genuine small distortion"
    );
}

#[test]
fn unknown_spectrum_name_errors_not_silent() {
    // A typo'd `spectrum=` must error (Pascal #401 `Set_Spectrum`), not silently
    // resolve to NIL and inject zero harmonic current.
    let mut dss = Dss::new();
    dss.command("New circuit.s basekv=12.47 phases=3 bus1=src");
    dss.command("New Load.x bus1=src phases=3 kv=12.47 kw=100 spectrum=doesnotexist");
    let errs = dss.error_texts().join("\n");
    assert!(
        errs.contains("Spectrum object \"doesnotexist\" not found"),
        "expected a loud missing-spectrum error, got: {errs:?}"
    );
}

#[test]
fn generator_in_harmonic_mode_injects() {
    // Generator carries the built-in `defaultgen` spectrum (3% at the 5th).
    assert_der_harmonic_injects(
        "New Generator.g1 bus1=db kv=12.47 kw=100 pf=0.95 model=1",
        "Generator.g1",
    );
}

#[test]
fn pvsystem_in_harmonic_mode_injects() {
    // PVSystem has no default spectrum (Create forces `SpectrumObj := NIL`), so
    // give it one explicitly to exercise the injection path.
    assert_der_harmonic_injects(
        "New PVSystem.pv1 bus1=db phases=3 kv=12.47 kva=100 pmpp=100 irradiance=1.0 spectrum=defaultgen",
        "PVSystem.pv1",
    );
}

#[test]
fn storage_in_harmonic_mode_injects() {
    // Storage discharging (so it carries a fundamental current to capture) +
    // an explicit spectrum.
    assert_der_harmonic_injects(
        "New Storage.st1 bus1=db phases=3 kv=12.47 kwrated=1000 kwhrated=2000 state=discharging %discharge=100 spectrum=defaultgen",
        "Storage.st1",
    );
}
