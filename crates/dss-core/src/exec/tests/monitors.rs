use crate::exec::*;

/// A mode-0 (V&I) and mode-1 (powers) monitor on a 2-bus line sampled by a
/// single daily step. Channel values transcribed from the oracle
/// (dss-python 0.15.7): at hour 1 the flat default shape gives mult=1, so
/// the sample equals the snapshot solution.
#[test]
fn monitor_mode0_mode1_daily() {
    let mut dss = Dss::new();
    dss.command("New circuit.test basekv=12.47 pu=1.0");
    dss.command("New line.l1 bus1=sourcebus bus2=b2 r1=0.1 x1=0.1 length=1");
    dss.command("New load.ld1 bus1=b2 phases=3 kv=12.47 kw=100 pf=0.95");
    dss.command("New monitor.m0 element=line.l1 terminal=1 mode=0");
    dss.command("New monitor.m1 element=line.l1 terminal=1 mode=1");
    dss.command("Set mode=daily number=1 stepsize=1h");
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    let m0 = dss.monitor_view("m0").expect("m0");
    assert_eq!(m0.sample_count, 1);
    assert_eq!(
        m0.header,
        vec![
            "hour", "t(sec)", "V1", "VAngle1", "V2", "VAngle2", "V3", "VAngle3", "I1", "IAngle1",
            "I2", "IAngle2", "I3", "IAngle3"
        ]
    );
    assert_eq!(m0.dbl_hour, vec![1.0]);
    // V1, VAngle1, I1, IAngle1 (channels 1,2,7,8 — 0-based 0,1,6,7).
    assert!(
        (m0.channels[0][0] - 7199.3564).abs() < 1e-2,
        "V1 {}",
        m0.channels[0][0]
    );
    assert!(
        (m0.channels[1][0] - (-0.0025524646)).abs() < 1e-4,
        "VAng1 {}",
        m0.channels[1][0]
    );
    assert!(
        (m0.channels[6][0] - 4.8712726).abs() < 1e-4,
        "I1 {}",
        m0.channels[6][0]
    );
    assert!(
        (m0.channels[7][0] - (-18.096796)).abs() < 1e-3,
        "IAng1 {}",
        m0.channels[7][0]
    );

    let m1 = dss.monitor_view("m1").expect("m1");
    assert_eq!(
        m1.header,
        vec![
            "hour", "t(sec)", "S1 (kVA)", "Ang1", "S2 (kVA)", "Ang2", "S3 (kVA)", "Ang3"
        ]
    );
    assert!(
        (m1.channels[0][0] - 35.070026).abs() < 1e-3,
        "S1 {}",
        m1.channels[0][0]
    );
    assert!(
        (m1.channels[1][0] - 18.094244).abs() < 1e-3,
        "Ang1 {}",
        m1.channels[1][0]
    );
}

/// A mode-5 (solution variables) monitor records the per-step solution
/// state. Channels 11/12 are wall-clock timings (non-reproducible), so only
/// the deterministic 1..10 are checked (oracle dss-python 0.15.7).
#[test]
fn monitor_mode5_solution_vars() {
    let mut dss = Dss::new();
    dss.command("New circuit.test basekv=12.47 pu=1.0");
    dss.command("New line.l1 bus1=sourcebus bus2=b2 r1=0.1 x1=0.1 length=1");
    dss.command("New load.ld1 bus1=b2 phases=3 kv=12.47 kw=100 pf=0.95");
    dss.command("New monitor.m5 element=line.l1 terminal=1 mode=5");
    dss.command("Set mode=daily number=1 stepsize=1h");
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    let m5 = dss.monitor_view("m5").expect("m5");
    assert_eq!(m5.sample_count, 1);
    let v = |i: usize| m5.channels[i][0];
    assert_eq!(v(2), 15.0); // MaxIterations
    assert_eq!(v(3), 10.0); // MaxControlIterations
    assert_eq!(v(4), 1.0); // Converged
    assert_eq!(v(5), 1.0); // IntervalHrs
    assert_eq!(v(6), 1.0); // SolutionCount
    assert_eq!(v(7), 1.0); // Mode = daily (ordinal 1)
    assert_eq!(v(8), 60.0); // Frequency
    assert_eq!(v(9), 0.0); // Year
}

/// The header-string modifier paths (±16 sequence / ±32 magnitude / ±64
/// pos-seq, residual, VIpolar/Ppolar) match the oracle (dss-python 0.15.7).
#[test]
fn monitor_header_modifiers() {
    let mut dss = Dss::new();
    dss.command("New circuit.test basekv=12.47");
    dss.command("New line.l1 bus1=sourcebus bus2=b2 r1=0.1 x1=0.1");
    let hdr = |dss: &mut Dss, decl: &str| -> Vec<String> {
        dss.command(decl);
        dss.monitor_view("m").expect("m").header
    };
    assert_eq!(
        hdr(
            &mut dss,
            "New monitor.m element=line.l1 mode=0 residual=yes"
        ),
        vec![
            "hour", "t(sec)", "V1", "VAngle1", "V2", "VAngle2", "V3", "VAngle3", "VN", "VNAngle",
            "I1", "IAngle1", "I2", "IAngle2", "I3", "IAngle3", "IN", "INAngle"
        ]
    );
    assert_eq!(
        hdr(&mut dss, "Edit monitor.m mode=0 residual=no VIPolar=no"),
        vec![
            "hour", "t(sec)", "V1.re", "V1.im", "V2.re", "V2.im", "V3.re", "V3.im", "I1.re",
            "I1.im", "I2.re", "I2.im", "I3.re", "I3.im"
        ]
    );
    assert_eq!(
        hdr(&mut dss, "Edit monitor.m mode=16 VIPolar=yes"),
        vec![
            "hour", "t(sec)", "V0", "VAngle0", "V1", "VAngle1", "V2", "VAngle2", "I0", "IAngle0",
            "I1", "IAngle1", "I2", "IAngle2"
        ]
    );
    assert_eq!(
        hdr(&mut dss, "Edit monitor.m mode=32"),
        vec![
            "hour",
            "t(sec)",
            "|V|1 (volts)",
            "|V|2 (volts)",
            "|V|3 (volts)",
            "|I|1 (amps)",
            "|I|2 (amps)",
            "|I|3 (amps)"
        ]
    );
    assert_eq!(
        hdr(&mut dss, "Edit monitor.m mode=64"),
        vec!["hour", "t(sec)", "V1", "V1ang", "I1", "I1ang"]
    );
    assert_eq!(
        hdr(&mut dss, "Edit monitor.m mode=96"),
        vec!["hour", "t(sec)", "V", "I"]
    );
    assert_eq!(
        hdr(&mut dss, "Edit monitor.m mode=1 PPolar=no"),
        vec![
            "hour",
            "t(sec)",
            "P1 (kW)",
            "Q1 (kvar)",
            "P2 (kW)",
            "Q2 (kvar)",
            "P3 (kW)",
            "Q3 (kvar)"
        ]
    );
}

/// A mode-2 monitor records a transformer tap; a wrong element class errors.
#[test]
fn monitor_mode2_tap_and_class_check() {
    let mut dss = Dss::new();
    dss.command("New circuit.test basekv=12.47 pu=1.0");
    dss.command("New line.l1 bus1=sourcebus bus2=b2 r1=0.1 x1=0.1 length=1");
    dss.command(
        "New transformer.t1 phases=3 windings=2 buses=[b2 b3] conns=[wye wye] \
             kvs=[12.47 4.16] kvas=[1000 1000] xhl=5 tap=1.05",
    );
    dss.command("New load.ld1 bus1=b3 phases=3 kv=4.16 kw=100 pf=0.95");
    dss.command("New monitor.mt element=transformer.t1 terminal=2 mode=2");
    dss.command("Set mode=daily number=1 stepsize=1h");
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let mt = dss.monitor_view("mt").expect("mt");
    assert_eq!(mt.header, vec!["hour", "t(sec)", "Tap (pu)"]);
    assert!(
        (mt.channels[0][0] - 1.05).abs() < 1e-5,
        "tap {}",
        mt.channels[0][0]
    );

    // Mode 2 on a line is rejected (Pascal 663).
    let mut bad = Dss::new();
    bad.command("New circuit.t basekv=12.47");
    bad.command("New line.l1 bus1=sourcebus bus2=b2 r1=0.1 x1=0.1");
    bad.command("New monitor.bad element=line.l1 mode=2");
    assert!(
        bad.errors()
            .iter()
            .any(|e| e.contains("is not a transformer")),
        "{:?}",
        bad.errors()
    );
}
