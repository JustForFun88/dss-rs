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

/// WP-PF.2: the full Monitor mode-4 flicker pipeline end-to-end on a live solve
/// — `TakeSample` records the raw (|V|, angle) buffer each duty step, and
/// `Export Monitor` runs `post_process` (Pascal `DoFlickerCalculations`), which
/// rewrites the stream in place with (flicker level, Pst). The bit-exact filter
/// math is pinned separately by `golden_flicker`; this test proves the wiring:
/// the raw sample, the export trigger, and the in-place rewrite with the correct
/// Pst window stepping. (The pinned dss_capi oracle cannot run this — its
/// `DoFlickerCalculations` segfaults on the Terminals OOB — so it is Rust-only.)
#[test]
fn monitor_mode4_flicker_end_to_end() {
    let mut dss = Dss::new();
    // Route Export output to a scratch dir (the CSV is a side effect we discard).
    let scratch = std::env::temp_dir().join(format!("dss_flicker_{}", std::process::id()));
    std::fs::create_dir_all(&scratch).ok();
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    dss.command("Set DefaultBaseFrequency=60");
    dss.command("New circuit.f basekv=12.47 pu=1.0 bus1=src phases=3 mvasc3=20000 mvasc1=21000");
    // A duty shape that ripples the load (hence the monitored RMS voltage).
    dss.command(
        "New LoadShape.flk npts=13 interval=(100 3600 /) \
         mult=[1.0 1.08 1.08 1.0 0.92 0.92 1.0 1.08 1.08 1.0 0.92 0.92 1.0]",
    );
    dss.command("New Line.ln bus1=src bus2=b1 phases=3 r1=0.1 x1=0.3 length=1 units=km");
    dss.command(
        "New Load.l bus1=b1 phases=3 kV=12.47 kW=2000 pf=0.95 model=2 duty=flk vminpu=0.85",
    );
    dss.command("New Monitor.pst element=Load.l terminal=1 mode=4");
    dss.command("Set voltagebases=[12.47]");
    dss.command("Calcvoltagebases");
    // stepsize=100 s -> a 600 s Pst window spans 6 samples; 13 steps cross two
    // window boundaries (t=600 at sample 6, t=1200 at sample 12).
    dss.command("Set mode=duty stepsize=100 number=1");
    for _ in 0..13 {
        dss.command("Solve");
    }
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    // Raw mode-4 buffer BEFORE post-processing: channels are (|V|, angle) pairs.
    let raw = dss.monitor_view("pst").expect("pst monitor");
    assert_eq!(raw.sample_count, 13);
    assert_eq!(raw.header.len(), 8); // hour,t(sec)+ Flk1,Pst1,Flk2,Pst2,Flk3,Pst3
    let raw_mag1 = raw.channels[0].clone(); // |V1|
    let raw_ang1 = raw.channels[1].clone(); // angle 1 (pre-process)
    assert!(
        raw_mag1.iter().all(|&m| (6000.0..8000.0).contains(&m)),
        "raw |V1| not ~7.2 kV LN: {raw_mag1:?}"
    );
    assert!(
        raw_ang1.iter().all(|&a| a.abs() < 30.0),
        "raw phase-1 angle not ~0 deg: {raw_ang1:?}"
    );

    // `Export Monitor` triggers `post_process` -> `DoFlickerCalculations`.
    dss.command("Export monitor pst");
    assert!(dss.errors().is_empty(), "export: {:?}", dss.errors());

    let post = dss.monitor_view("pst").expect("pst monitor");
    let flk1 = &post.channels[0]; // now the instantaneous flicker level
    let pst1 = &post.channels[1]; // now the short-term severity
    // Post-processing rewrote the magnitude channel into flicker levels.
    assert!(
        flk1 != &raw_mag1,
        "flicker channel not rewritten (post_process didn't run)"
    );
    // Flicker of a mildly rippling near-nominal voltage is small (|flk| << |V|).
    assert!(
        flk1.iter().all(|&f| f.abs() < 1.0),
        "flicker levels implausibly large: {flk1:?}"
    );
    // Pst stepping: 0 until the first 600 s window completes (sample index 5,
    // t=600), then a single constant severity value across that window.
    for (i, &p) in pst1.iter().enumerate().take(5) {
        assert_eq!(p, 0.0, "Pst must be 0 before the first window (sample {i})");
    }
    assert!(
        pst1[5] > 0.0,
        "Pst must be set once the first window completes"
    );
    // Within a window the Pst is held constant (samples 5..=10 are window 1).
    for i in 6..=10 {
        assert_eq!(pst1[i], pst1[5], "Pst not held constant within window 1");
    }
    // A second export is idempotent (the `is_processed` latch): channels unchanged.
    dss.command("Export monitor pst");
    let post2 = dss.monitor_view("pst").expect("pst monitor");
    assert_eq!(&post2.channels[0], flk1, "re-export must not reprocess");
    assert_eq!(&post2.channels[1], pst1, "re-export must not reprocess");
}
