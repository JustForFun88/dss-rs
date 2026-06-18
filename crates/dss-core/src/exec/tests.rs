use super::*;

fn query(dss: &mut Dss, what: &str) -> String {
    dss.command(&format!("? {what}"));
    dss.result().to_string()
}

/// `Edit`/`~`/`?` are circuit-gated in `ProcessCommand` (error 301), so
/// even DSS_OBJECT tests need a circuit.
fn dss_with_circuit() -> Dss {
    let mut dss = Dss::new();
    dss.command("New circuit.testckt");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss
}

#[test]
fn new_and_query_defaults() {
    let mut dss = dss_with_circuit();
    dss.command("New TCC_Curve.test");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(query(&mut dss, "TCC_Curve.test.NPts"), "0");
    assert_eq!(query(&mut dss, "TCC_Curve.test.C_Array"), "");
    assert_eq!(query(&mut dss, "TCC_Curve.test.T_Array"), "");
    assert_eq!(query(&mut dss, "TCC_Curve.test.Like"), "");
}

#[test]
fn new_with_inline_edits() {
    let mut dss = dss_with_circuit();
    dss.command("New TCC_Curve.t npts=3 C_array=(1 2 3) T_array=(0.1 0.2 0.3)");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(query(&mut dss, "tcc_curve.t.npts"), "3");
    assert_eq!(query(&mut dss, "tcc_curve.t.c_array"), "[ 1 2 3]");
    assert_eq!(query(&mut dss, "tcc_curve.t.t_array"), "[ 0.1 0.2 0.3]");
}

#[test]
fn edit_and_more_continue_the_object() {
    let mut dss = dss_with_circuit();
    dss.command("New TCC_Curve.t npts=2");
    dss.command("Edit TCC_Curve.t C_array=(5 6)");
    dss.command("~ T_array=(9 8)");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(query(&mut dss, "tcc_curve.t.c_array"), "[ 5 6]");
    assert_eq!(query(&mut dss, "tcc_curve.t.t_array"), "[ 9 8]");
}

#[test]
fn make_like_copies_state() {
    let mut dss = dss_with_circuit();
    dss.command("New TCC_Curve.a npts=2 C_array=(1 2) T_array=(3 4)");
    dss.command("New TCC_Curve.b like=a");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(query(&mut dss, "tcc_curve.b.npts"), "2");
    assert_eq!(query(&mut dss, "tcc_curve.b.c_array"), "[ 1 2]");
    assert_eq!(query(&mut dss, "tcc_curve.b.t_array"), "[ 3 4]");
}

#[test]
fn make_like_copies_prp_sequence() {
    // Pascal `TDSSObject.MakeLike` copies the source's PrpSequence, then
    // the Edit loop stamps the Like property itself — so a Save-order walk
    // of the target yields NPts, C_Array, T_Array, Like.
    let mut dss = Dss::new();
    dss.command("New TCC_Curve.a npts=2 C_array=(1 2) T_array=(3 4)");
    dss.command("New TCC_Curve.b like=a");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    let cls = &dss.classes[0];
    let oi = cls.name_to_idx["b"];
    let data = cls.objects[oi].data();
    assert_eq!(data.next_property_set(None), Some(1)); // NPts
    assert_eq!(data.next_property_set(Some(1)), Some(2)); // C_Array
    assert_eq!(data.next_property_set(Some(2)), Some(3)); // T_Array
    assert_eq!(data.next_property_set(Some(3)), Some(4)); // Like
    assert_eq!(data.next_property_set(Some(4)), None);
}

#[test]
fn set_and_get_require_a_circuit() {
    let mut dss = Dss::new();
    dss.command("Set mode=snap");
    dss.command("Get mode");
    assert_eq!(dss.errors().len(), 2, "{:?}", dss.errors());
    assert!(
        dss.errors()
            .iter()
            .all(|e| e.contains("You must create a new circuit object first")),
        "{:?}",
        dss.errors()
    );
}

#[test]
fn solve_requires_a_circuit() {
    let mut dss = Dss::new();
    dss.command("Solve");
    assert!(
        dss.errors()
            .iter()
            .any(|e| e.contains("You must create a new circuit object first")),
        "{:?}",
        dss.errors()
    );
}

#[test]
fn duplicate_new_edits_existing() {
    let mut dss = dss_with_circuit();
    dss.command("New TCC_Curve.t npts=2 C_array=(1 2)");
    // A second "New" of the same name becomes an edit (DSS_OBJECT, no dups).
    dss.command("New TCC_Curve.t C_array=(7 8)");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(query(&mut dss, "tcc_curve.t.c_array"), "[ 7 8]");
}

#[test]
fn clear_drops_objects() {
    let mut dss = dss_with_circuit();
    dss.command("New TCC_Curve.t npts=2");
    dss.command("Clear"); // drops the circuit and all objects
    assert!(dss.circuit().is_none());
    // `?` is circuit-gated again after Clear, like the Pascal.
    dss.command("? TCC_Curve.t.npts");
    assert!(
        dss.errors()
            .iter()
            .any(|e| e.contains("You must create a new circuit object first")),
        "{:?}",
        dss.errors()
    );
    // After recreating a circuit, the old object is really gone.
    let mut dss = dss_with_circuit();
    dss.command("New circuit.again"); // second circuit is rejected
    assert!(!dss.errors().is_empty());
    assert_eq!(query(&mut dss, "TCC_Curve.t.npts"), "Property Unknown");
}

#[test]
fn unknown_parameter_is_reported() {
    let mut dss = Dss::new();
    dss.command("New TCC_Curve.t bogus=3");
    assert!(dss.errors().iter().any(|e| e.contains("Unknown parameter")));
}

#[test]
fn parse_obj_name_splits_on_last_class_dot() {
    assert_eq!(
        parse_obj_name("TCC_Curve.test.npts"),
        ("TCC_Curve.test".to_string(), "npts".to_string())
    );
    assert_eq!(
        parse_obj_name("test.npts"),
        ("test".to_string(), "npts".to_string())
    );
    assert_eq!(parse_obj_name("npts"), (String::new(), "npts".to_string()));
}

#[test]
fn new_circuit_creates_default_source() {
    let mut dss = Dss::new();
    dss.command("New circuit.test basekv=115 pu=1.0001");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let ckt = dss.circuit().unwrap();
    assert_eq!(ckt.name, "test");
    assert_eq!(ckt.sources.len(), 1);
    assert_eq!(query(&mut dss, "vsource.source.basekv"), "115");
    assert_eq!(query(&mut dss, "vsource.source.pu"), "1.0001");
    assert_eq!(query(&mut dss, "vsource.source.bus1"), "sourcebus");
}

#[test]
fn two_bus_snapshot_solves() {
    let mut dss = Dss::new();
    dss.command("New circuit.twobus basekv=12.47 pu=1.0 phases=3 mvasc3=2000");
    dss.command("New Line.l1 bus1=sourcebus bus2=loadbus length=1 units=km");
    dss.command("New Load.ld1 bus1=loadbus phases=3 kv=12.47 kw=600 pf=0.95");
    dss.command("Set voltagebases=[12.47]");
    dss.command("CalcVoltageBases");
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let ckt = dss.circuit().unwrap();
    assert!(ckt.is_solved);
    assert_eq!(ckt.num_nodes, 6);
    let vbase = 12.47e3 / crate::util::sqrt3();
    for i in 1..=ckt.num_nodes {
        let vm = ckt.solution.node_v[i].norm();
        assert!(
            (vm / vbase - 1.0).abs() < 0.1,
            "node {i} voltage {vm} not near {vbase}"
        );
    }
    // Iteration count reported like the oracle's Solution.Iterations.
    assert!(ckt.solution.iteration >= 2);
}

/// Generator model 1 (constant PQ) injects negative load: a 100 kW / pf
/// 0.95 generator delivers −33.333 kW, −10.956 kvar per phase (oracle
/// dss-python 0.15.7, stiff source + short line).
#[test]
fn generator_model1_pq_snapshot() {
    let mut dss = Dss::new();
    dss.command(
        "New circuit.t1 basekv=12.47 bus1=sourcebus pu=1.0 \
             r1=0 x1=0.0001 r0=0 x0=0.0001",
    );
    dss.command(
        "New Line.l1 bus1=sourcebus bus2=genbus length=1 \
             r1=0.01 x1=0.01 r0=0.01 x0=0.01 c1=0 c0=0",
    );
    dss.command("New Generator.g1 bus1=genbus kV=12.47 kW=100 PF=0.95 model=1 conn=wye");
    dss.command("Set controlmode=off");
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert!(dss.circuit().unwrap().is_solved);

    let snap = dss.snapshot_elements();
    let g = snap
        .iter()
        .find(|e| e.name.eq_ignore_ascii_case("Generator.g1"))
        .expect("generator snapshot");
    for ph in 0..3 {
        assert!(
            (g.powers[2 * ph] - (-33.333333)).abs() < 1e-3,
            "phase {ph} P {}",
            g.powers[2 * ph]
        );
        assert!(
            (g.powers[2 * ph + 1] - (-10.956137)).abs() < 1e-3,
            "phase {ph} Q {}",
            g.powers[2 * ph + 1]
        );
    }
}

/// Generator model 3 (constant P, |V|) exercises the DQDV var-control
/// machinery (`SetGeneratordQdV`): a 300 kW PV generator holds |V| ≈ 1 pu
/// and absorbs/produces vars to do it, landing at −100.003 kW, −64.728
/// kvar per phase (oracle dss-python 0.15.7).
#[test]
fn generator_model3_pv_snapshot() {
    let mut dss = Dss::new();
    dss.command(
        "New circuit.t1 basekv=12.47 bus1=sourcebus pu=1.0 \
             r1=0 x1=0.0001 r0=0 x0=0.0001",
    );
    dss.command(
        "New Line.l1 bus1=sourcebus bus2=genbus length=1 \
             r1=0.05 x1=0.10 r0=0.05 x0=0.10 c1=0 c0=0",
    );
    dss.command("New Load.ld1 bus1=genbus kV=12.47 kW=500 PF=0.9 conn=wye model=1");
    dss.command(
        "New Generator.g1 bus1=genbus kV=12.47 kW=300 model=3 conn=wye \
             Vpu=1.0 maxkvar=200 minkvar=-200",
    );
    dss.command("Set controlmode=off");
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert!(dss.circuit().unwrap().is_solved);

    let snap = dss.snapshot_elements();
    let g = snap
        .iter()
        .find(|e| e.name.eq_ignore_ascii_case("Generator.g1"))
        .expect("generator snapshot");
    for ph in 0..3 {
        assert!(
            (g.powers[2 * ph] - (-100.00275)).abs() < 1e-2,
            "phase {ph} P {}",
            g.powers[2 * ph]
        );
        assert!(
            (g.powers[2 * ph + 1] - (-64.7282)).abs() < 1e-2,
            "phase {ph} Q {}",
            g.powers[2 * ph + 1]
        );
    }
}

/// Parse the single number a `?` scalar query returns.
fn query_f64(dss: &mut Dss, what: &str) -> f64 {
    query(dss, what).parse().expect("numeric query result")
}

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

#[test]
fn line_fetches_sym_linecode() {
    // Oracle (dss-python 0.15.7): linecode in mi, line length 2000 ft.
    let mut dss = Dss::new();
    dss.command("New circuit.p");
    dss.command(
        "New linecode.mtx601 nphases=3 r1=0.1 x1=0.2 r0=0.3 x0=0.6 c1=3 c0=1 \
             units=mi normamps=500 emergamps=700",
    );
    dss.command("New line.l1 bus1=a bus2=b linecode=mtx601 length=2000 units=ft");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(query(&mut dss, "line.l1.linecode"), "mtx601");
    assert_eq!(query(&mut dss, "line.l1.normamps"), "500");
    assert_eq!(query(&mut dss, "line.l1.emergamps"), "700");
    assert_eq!(query(&mut dss, "line.l1.units"), "ft");
    // r1 getter divides by FUnitsConvert = ConvertLineUnits(mi, ft) = 5280.
    assert!((query_f64(&mut dss, "line.l1.r1") - 0.1 / 5280.0).abs() < 1e-12);
    // Unported scalar/array refs render like the oracle.
    assert_eq!(query(&mut dss, "line.l1.geometry"), "");
    assert_eq!(query(&mut dss, "line.l1.wires"), "[]");
}

#[test]
fn load_and_vsource_resolve_shape_refs() {
    // WP5.3: the shape refs became resolved `object_ref_class` props. The
    // ObjectRef getter renders the resolved object's name, and an unset
    // `yearly` is seeded from `daily` (Pascal `YearlyShapeObj := DailyShapeObj`).
    let mut dss = Dss::new();
    dss.command("New circuit.t basekv=12.47 bus1=src");
    dss.command("New loadshape.d1 npts=2 interval=1 mult=(0.4 0.8)");
    dss.command("New growthshape.g1 npts=2 year=(1 2) mult=(1.02 1.05)");
    dss.command("New load.la bus1=src phases=3 kv=12.47 kw=100 pf=1 daily=d1 growth=g1");
    dss.command("New vsource.v2 bus1=src basekv=12.47 daily=d1");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(query(&mut dss, "load.la.daily"), "d1");
    assert_eq!(query(&mut dss, "load.la.yearly"), "d1"); // seeded from daily
    assert_eq!(query(&mut dss, "load.la.growth"), "g1");
    assert_eq!(query(&mut dss, "vsource.v2.daily"), "d1");
    assert_eq!(query(&mut dss, "vsource.v2.yearly"), "d1");

    // A missing shape is the Pascal 401 ("object not found") and leaves the
    // reference empty — the edit continues.
    dss.command("New load.lb bus1=src daily=nope");
    assert!(
        dss.errors().iter().any(|e| e.contains("not found")),
        "expected a not-found error, got {:?}",
        dss.errors()
    );
}

#[test]
fn line_fetches_matrix_linecode() {
    let mut dss = Dss::new();
    dss.command("New circuit.p");
    dss.command(
        "New linecode.mx nphases=2 rmatrix=[0.1 | 0.05 0.1] \
             xmatrix=[0.2 | 0.07 0.2] cmatrix=[3 | -1 3] units=mi",
    );
    dss.command("New line.l3 bus1=a.1.2 bus2=b.1.2 linecode=mx length=1 units=mi");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(query(&mut dss, "line.l3.phases"), "2");
    assert_eq!(query(&mut dss, "line.l3.rmatrix"), "[0.1 |0.05 0.1 ]");
    // Matrix model hides the sym scalars (CONDITIONAL_VALUE).
    assert_eq!(query(&mut dss, "line.l3.r1"), "----");
}

#[test]
fn line_linecode_then_r1_override_keeps_fetched_matrix() {
    // Oracle: r1=0.5 overrides the scalar, but the dumped rmatrix still
    // reflects the code's Z (recalc is deferred to CalcYPrim).
    let mut dss = Dss::new();
    dss.command("New circuit.p");
    dss.command("New linecode.mtx601 nphases=3 r1=0.1 x1=0.2 r0=0.3 x0=0.6 units=mi");
    dss.command("New line.l4 bus1=a bus2=b linecode=mtx601 r1=0.5 length=1 units=mi");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(query(&mut dss, "line.l4.r1"), "0.5");
    // Zs.re = (2*0.1 + 0.3)/3 = 0.5/3, units_convert reset to 1 by r1.
    let rm = query(&mut dss, "line.l4.rmatrix");
    let first: f64 = rm
        .trim_start_matches('[')
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert!((first - 0.5 / 3.0).abs() < 1e-9, "{rm}");
}

#[test]
fn line_unknown_linecode_errors_and_continues() {
    let mut dss = Dss::new();
    dss.command("New circuit.p");
    dss.command("New line.l5 bus1=a bus2=b linecode=nosuch r1=0.1 length=1");
    assert!(
        dss.errors()
            .iter()
            .any(|e| e == "Line.l5.LineCode: LineCode object \"nosuch\" not found."),
        "{:?}",
        dss.errors()
    );
    // The edit continued: r1=0.1 was applied, phases stayed default.
    assert_eq!(query(&mut dss, "line.l5.r1"), "0.1");
    assert_eq!(query(&mut dss, "line.l5.phases"), "3");
}

#[test]
fn line_geometry_undefined_wire_in_array_aborts() {
    // Pascal `DSSObjectReferenceArrayProperty` Exits on the first unresolved
    // token: the "not found" is logged and the write function (SetWires)
    // never runs, so nothing is stored and no spurious "Unexpected number"
    // count error fires.
    let mut dss = Dss::new();
    dss.command("New circuit.p");
    dss.command("New WireData.acsr Rdc=0.0526 GMRac=0.0244 GMRunits=ft radius=0.0306 radunits=ft normamps=530 Runits=ft");
    dss.command("New LineGeometry.g1 nconds=3 nphases=3 wires=[acsr bad acsr]");
    let errs = dss.errors();
    assert!(
        errs.iter().any(|e| e.contains("object \"bad\" not found")),
        "{errs:?}"
    );
    assert!(
        !errs.iter().any(|e| e.contains("Unexpected number")),
        "{errs:?}"
    );
    // Exit before the write function: no conductors were stored.
    assert_eq!(query(&mut dss, "LineGeometry.g1.wires"), "[, , ]");
}

/// WP7.1 step 3a: a `geometry=`-specified Line resolves the `LineGeometry`
/// class end to end (parse → foreign-class resolve → `FetchGeometryCode`),
/// adopts the geometry's conductor count, and solves through the Carson
/// matrix path — the full pipeline the inline `geometry_tests` bypass.
#[test]
fn line_geometry_specified_resolves_and_solves() {
    let mut dss = Dss::new();
    dss.command("New circuit.geo basekv=12.47 phases=3");
    dss.command(
        "New WireData.w runits=m gmrunits=m radunits=m \
             rac=0.0003 gmrac=0.005 radius=0.01 normamps=400",
    );
    dss.command(
        "New LineGeometry.geo1 nconds=3 nphases=3 \
             cond=1 wire=w x=0 h=10 units=m cond=2 wire=w x=1 h=10 cond=3 wire=w x=2 h=10",
    );
    dss.command("New Line.l1 bus1=sourcebus bus2=b2 phases=3 geometry=geo1 length=1 units=km");
    dss.command("New Load.ld bus1=b2 phases=3 kv=12.47 kw=300 pf=0.95 model=1");
    dss.command("Set voltagebases=[12.47]");
    dss.command("CalcVoltageBases");
    dss.command("Set controlmode=off");
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    // The Line resolved the geometry and took its conductor count + type.
    assert_eq!(query(&mut dss, "Line.l1.geometry"), "geo1");
    assert_eq!(query(&mut dss, "Line.l1.phases"), "3");
    // The sym scalars are hidden (`----`) — a matrix/geometry model is active.
    assert_eq!(query(&mut dss, "Line.l1.r1"), "----");

    let ckt = dss.circuit().unwrap();
    assert!(ckt.solution.converged_flag, "geometry line should converge");
}

/// WP4.7 step 6 (the "silent killer" check): control elements attach to
/// existing buses, so adding a RegControl must not change `YNodeOrder`,
/// and the Y build must skip their `yprim: None` (no stamping, solvable).
#[test]
fn reg_control_does_not_change_node_order() {
    let build = |with_control: bool| -> (Vec<String>, bool, i32) {
        let mut dss = Dss::new();
        dss.command("New circuit.ctl basekv=12.47 pu=1.0 phases=3 mvasc3=2000");
        dss.command(
            "New transformer.t1 phases=3 windings=2 buses=(sourcebus, b2) \
                 conns=(delta wye) kvs=(12.47 4.16) kvas=(5000 5000) xhl=8",
        );
        if with_control {
            dss.command("New regcontrol.r1 transformer=t1 winding=2 vreg=122 band=2 ptratio=20");
        }
        dss.command("New load.l1 bus1=b2 phases=3 kv=4.16 kw=300 pf=0.95");
        dss.command("Set voltagebases=[12.47, 4.16]");
        dss.command("CalcVoltageBases");
        // Controls off: this test isolates the *structural* invariants
        // (node order, no Yprim stamping). With controls active the
        // RegControl legitimately adds control iterations (WP5.7).
        dss.command("Set controlmode=off");
        dss.command("Solve");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        let ckt = dss.circuit().unwrap();
        if with_control {
            assert_eq!(ckt.controls.len(), 1);
            // The control sits on the transformer's winding-2 bus.
            let r = ckt.controls[0];
            let elem = dss.classes[r.cls].objects[r.idx].as_ckt_element().unwrap();
            assert_eq!(elem.cd().get_bus(1), "b2");
            assert!(elem.cd().yprim.is_none());
        }
        let names = (1..=ckt.num_nodes).map(|i| ckt.node_name(i)).collect();
        (names, ckt.is_solved, ckt.solution.iteration)
    };
    let (with, solved_w, iter_w) = build(true);
    let (without, solved_wo, iter_wo) = build(false);
    assert!(solved_w && solved_wo);
    assert_eq!(with, without, "RegControl changed the node order");
    assert_eq!(iter_w, iter_wo, "RegControl changed the iteration count");
}

/// WP6.1: `BuildActiveBusAdjacencyLists` (CktTree.pas l.678) — non-shunt
/// PD branches are listed at *every* terminal's bus; PC elements and
/// shunt capacitors land on the terminal-1 PC list; sources (NON_PCPD)
/// appear in neither.
#[test]
fn bus_adjacency_lists_bucket_elements() {
    use crate::circuit::ckt_tree::build_active_bus_adjacency_lists;

    let mut dss = Dss::new();
    dss.command("New circuit.adj basekv=12.47 pu=1.0 phases=3 mvasc3=2000");
    dss.command(
        "New line.l1 bus1=sourcebus bus2=b2 length=1 units=km \
             r1=0.1 x1=0.2 r0=0.3 x0=0.6 c1=0 c0=0",
    );
    dss.command("New capacitor.cap1 bus1=b2 kv=12.47 kvar=300");
    dss.command("New load.ld1 bus1=b2 phases=3 kv=12.47 kw=100 pf=0.95");
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    let Dss {
        classes, circuit, ..
    } = &mut dss;
    let ckt = circuit.as_ref().unwrap();
    let store = ClassStore { classes };
    let adj = build_active_bus_adjacency_lists(ckt, &store);

    let sb = ckt.bus_list.find("sourcebus").unwrap();
    let b2 = ckt.bus_list.find("b2").unwrap();
    let names = |refs: &[ElemRef]| -> Vec<String> {
        refs.iter()
            .map(|&r| store.ckt_elem(r).cd().obj.name().to_string())
            .collect()
    };

    // The line (non-shunt PD) shows up at both of its terminal buses.
    assert_eq!(names(&adj.pd[sb]), vec!["l1"]);
    assert_eq!(names(&adj.pd[b2]), vec!["l1"]);
    // PC list at b2: the load plus the shunt capacitor (PD element on
    // the PC list, in pc_elements-then-pd_elements build order), and
    // no source anywhere.
    assert_eq!(names(&adj.pc[b2]), vec!["ld1", "cap1"]);
    assert!(adj.pc[sb].is_empty(), "sources are NON_PCPD");
}

#[test]
fn get_returns_set_values() {
    let mut dss = Dss::new();
    dss.command("New circuit.c1");
    dss.command("Set mode=daily tolerance=0.001 maxiterations=25");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss.command("Get mode tolerance maxiterations");
    assert_eq!(dss.result(), "Daily, 0.001, 25");
}

/// AutoAdd option object defaults (`TAutoAdd.Init` + Circuit loss/UE
/// defaults) echoed back through `Get`.
#[test]
fn autoadd_options_defaults_via_get() {
    let mut dss = Dss::new();
    dss.command("New circuit.c1");
    dss.command("Get genkw genpf capkvar addtype ueweight lossweight ueregs lossregs");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(dss.result(), "1000, 1, 600, generator, 1, 1, [10], [13]");
}

/// `Set` the AutoAdd options, then verify both the circuit state and the
/// `Get` echo (AddType maps to the lowercase device word).
#[test]
fn autoadd_options_set_then_get() {
    let mut dss = Dss::new();
    dss.command("New circuit.c1");
    dss.command(
        "Set genkw=500 genpf=0.95 capkvar=1200 addtype=capacitor \
             ueweight=2 lossweight=3 ueregs=[1,2,3] lossregs=[13,14]",
    );
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    {
        let ckt = dss.circuit().unwrap();
        assert_eq!(ckt.auto_add_obj.gen_kw, 500.0);
        assert_eq!(ckt.auto_add_obj.gen_pf, 0.95);
        assert_eq!(ckt.auto_add_obj.cap_kvar, 1200.0);
        assert_eq!(ckt.auto_add_obj.add_type, crate::circuit::CAPADD);
        assert_eq!(ckt.ue_weight, 2.0);
        assert_eq!(ckt.loss_weight, 3.0);
        assert_eq!(ckt.ue_regs, vec![1, 2, 3]);
        assert_eq!(ckt.loss_regs, vec![13, 14]);
    }
    dss.command("Get genkw genpf capkvar addtype ueweight lossweight ueregs lossregs");
    assert_eq!(
        dss.result(),
        "500, 0.95, 1200, capacitor, 2, 3, [1, 2, 3], [13, 14]"
    );
}

/// `Set UEregs=` with a non-numeric token reproduces the Pascal
/// `MakeInteger` *raise*: the parser error is logged and the fill stops at
/// the bad token, leaving the already-sized array zero-filled from there on
/// (`[10, 0, 0]`, not a silent `[10, 0, 13]`). A roundable decimal still
/// rounds (`13.7 -> 14`) via the double fallback.
#[test]
fn ueregs_nonnumeric_token_logs_error_and_truncates() {
    let mut dss = Dss::new();
    dss.command("New circuit.c1");
    dss.command("Set ueregs=(10 abc 13)");
    assert!(
        dss.errors()
            .iter()
            .any(|e| e.contains("Integer number conversion error")),
        "expected a logged conversion error, got {:?}",
        dss.errors()
    );
    assert_eq!(dss.circuit().unwrap().ue_regs, vec![10, 0, 0]);

    // The roundable-decimal path is unaffected (fresh circuit so the
    // error log above doesn't bleed into this assertion).
    let mut dss2 = Dss::new();
    dss2.command("New circuit.c2");
    dss2.command("Set lossregs=(13.7 14)");
    assert!(dss2.errors().is_empty(), "{:?}", dss2.errors());
    assert_eq!(dss2.circuit().unwrap().loss_regs, vec![14, 14]);
}

/// `Set addtype=` with an unrecognized value resolves to the enum default
/// (CAPADD) with **no** error — Pascal `StringToOrdinal` returns the default
/// rather than raising.
#[test]
fn addtype_unknown_falls_back_to_default_no_error() {
    let mut dss = Dss::new();
    dss.command("New circuit.c1");
    dss.command("Set addtype=foo");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(
        dss.circuit().unwrap().auto_add_obj.add_type,
        crate::circuit::CAPADD
    );
    dss.command("Get addtype");
    assert_eq!(dss.result(), "capacitor");
}

/// `Set AutoBusList=` parses an inline bus-name list (`DoAutoAddBusList`),
/// stored insertion-ordered and echoed comma-separated by `Get`.
#[test]
fn autoadd_bus_list_inline_round_trips() {
    let mut dss = Dss::new();
    dss.command("New circuit.c1");
    dss.command("Set autobuslist=[b1, b2, b3]");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(
        dss.circuit().unwrap().auto_add_bus_list,
        vec!["b1".to_string(), "b2".to_string(), "b3".to_string()]
    );
    dss.command("Get autobuslist");
    assert_eq!(dss.result(), "b1, b2, b3");
}

/// The AutoAdd *solve mode* is `NOT_PORTED` (the capacity search needs
/// aux-current injection + meter sampling). `Solve mode=autoadd` therefore
/// still reports the unknown-mode error — the documented deferral.
#[test]
fn autoadd_solve_mode_still_deferred() {
    let mut dss = Dss::new();
    dss.command("New circuit.c1 basekv=12.47 bus1=src phases=3");
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command("solve mode=autoadd");
    assert!(
        dss.errors()
            .iter()
            .any(|e| e.contains("Unknown solution mode")),
        "expected AutoAdd solve to remain deferred, got {:?}",
        dss.errors()
    );
}

/// `Set ReduceOption/Zmag/KeepLoad=` defaults + round-trip through `Get`.
/// (ReduceOption's default string is empty, so `Get` elides it — exactly
/// like Pascal `AppendGlobalResult` on a zero-length string.)
#[test]
fn reduce_options_defaults_and_round_trip() {
    let mut dss = Dss::new();
    dss.command("New circuit.c1");
    dss.command("Get zmag keepload");
    assert_eq!(dss.result(), "0.02, Yes");
    dss.command("Get reduceoption");
    assert_eq!(dss.result(), "");

    dss.command("Set reduceoption=shortlines zmag=0.05 keepload=no");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    {
        let ckt = dss.circuit().unwrap();
        assert_eq!(
            ckt.reduction_strategy,
            crate::circuit::ReductionStrategy::ShortLines
        );
        assert_eq!(ckt.reduction_strategy_string, "shortlines");
        assert_eq!(ckt.reduction_zmag, 0.05);
        assert!(!ckt.reduce_laterals_keep_load);
    }
    dss.command("Get reduceoption zmag keepload");
    assert_eq!(dss.result(), "shortlines, 0.05, No");
}

/// `DoSetReduceStrategy` dispatches on the first character; `S` resolves to
/// Switch via `CompareTextShortest(S,'SWITCH')`, else ShortLines.
#[test]
fn reduce_strategy_first_char_dispatch() {
    use crate::circuit::ReductionStrategy as Rs;
    let mut dss = Dss::new();
    dss.command("New circuit.c1");
    let cases = [
        ("break", Rs::BreakLoop),
        ("default", Rs::Default),
        ("ends", Rs::Dangling),
        ("laterals", Rs::Laterals),
        ("merge", Rs::MergeParallel),
        ("switch", Rs::Switches),
        ("shortlines", Rs::ShortLines),
        ("s", Rs::Switches), // CompareTextShortest("s","SWITCH")=0 -> Switch
    ];
    for (opt, want) in cases {
        dss.command(&format!("Set reduceoption={opt}"));
        assert_eq!(dss.circuit().unwrap().reduction_strategy, want, "opt={opt}");
    }
    // Unknown strategy: error logged, strategy falls back to Default.
    dss.command("Set reduceoption=zzz");
    assert!(
        dss.errors()
            .iter()
            .any(|e| e.contains("Unknown Reduction Strategy")),
        "{:?}",
        dss.errors()
    );
    assert_eq!(dss.circuit().unwrap().reduction_strategy, Rs::Default);
}

/// `Reduce` with no energy meters reproduces Pascal error 1890, including
/// the full documentation URL (pinned so an edit can't silently drift it).
#[test]
fn reduce_command_requires_energy_meter() {
    let mut dss = Dss::new();
    dss.command("New circuit.c1 basekv=12.47 bus1=src phases=3");
    dss.command("reduce");
    assert!(
            dss.errors().iter().any(|e| e
                == "An energy meter is required to use this feature. Please check \
                    https://sourceforge.net/p/electricdss/code/HEAD/tree/trunk/Version8/Doc/Circuit%20Reduction%20for%20Version8.docx \
                    for examples."),
            "{:?}",
            dss.errors()
        );
}

/// `Reduce <name>` with a meter present but no such meter reproduces Pascal
/// error 262 (echoing the *uppercased* name), not the generic deferral.
#[test]
fn reduce_named_meter_not_found_is_262() {
    let mut dss = Dss::new();
    dss.command("New circuit.c1 basekv=12.47 bus1=src phases=3");
    dss.command("New line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6");
    dss.command("New energymeter.m1 element=line.l1 terminal=1");
    dss.command("reduce nope");
    assert!(
        dss.errors()
            .iter()
            .any(|e| e == "EnergyMeter \"NOPE\" not found."),
        "{:?}",
        dss.errors()
    );
    // The deferral must NOT fire for a name that did not resolve.
    assert!(
        !dss.errors().iter().any(|e| e.contains("not ported")),
        "{:?}",
        dss.errors()
    );
}

/// `Reduce` marks enabled shunt cap/reactor buses as keepers *before* the
/// meter check — so the marking happens even on the error-1890 path
/// (Pascal `MarkCapandReactorBuses` runs unconditionally).
#[test]
fn reduce_marks_cap_and_reactor_buses() {
    let mut dss = Dss::new();
    dss.command("New circuit.c1 basekv=12.47 bus1=src phases=3");
    dss.command("New line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6");
    dss.command("New line.l2 bus1=b1 bus2=b2 length=1 r1=0.3 x1=0.6");
    dss.command("New capacitor.c bus1=b1 phases=3 kvar=600 kv=12.47");
    dss.command("New reactor.r bus1=b2 phases=3 kvar=100 kv=12.47");
    dss.command("Set voltagebases=[12.47]");
    dss.command("CalcVoltageBases");
    // Bus refs are materialized at Y-build (solve) time in this port; a
    // real `Reduce` always runs post-solve (it needs metered zones).
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    // No energy meter → error 1890, but the marking still ran first.
    dss.command("reduce");
    let ckt = dss.circuit().unwrap();
    let keep = |name: &str| {
        ckt.buses
            .iter()
            .find(|b| b.name.eq_ignore_ascii_case(name))
            .map(|b| b.keep)
            .unwrap_or(false)
    };
    assert!(keep("b1"), "shunt capacitor bus should be a keeper");
    assert!(keep("b2"), "shunt reactor bus should be a keeper");
}

/// `Reduce` with a meter present passes the precondition but the zone
/// reduction (`Line.MergeWith`) is NOT_PORTED — the documented deferral.
#[test]
fn reduce_command_with_meter_deferred() {
    let mut dss = Dss::new();
    dss.command("New circuit.c1 basekv=12.47 bus1=src phases=3");
    dss.command("New line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6");
    dss.command("New energymeter.m1 element=line.l1 terminal=1");
    dss.command("reduce");
    assert!(
        dss.errors()
            .iter()
            .any(|e| e.contains("reduction is not ported")),
        "{:?}",
        dss.errors()
    );
}

/// Build the 2-bus regulator micro-circuit the WP5.7 oracle probes used.
fn reg_two_bus(dss: &mut Dss, reg_props: &str) {
    dss.command("New circuit.ctl basekv=12.47 pu=1.0 phases=3 mvasc3=2000");
    dss.command(
        "New transformer.t1 phases=3 windings=2 buses=(sourcebus, b2) \
             conns=(delta wye) kvs=(12.47 4.16) kvas=(5000 5000) xhl=8",
    );
    dss.command(&format!("New regcontrol.r1 transformer=t1 {reg_props}"));
    dss.command("New load.l1 bus1=b2 phases=3 kv=4.16 kw=300 pf=0.95");
    dss.command("Set voltagebases=[12.47, 4.16]");
    dss.command("CalcVoltageBases");
}

/// WP5.7: the live control loop drives the regulator to the oracle's tap.
/// Oracle probe (pinned dss-python): iterations=6, winding-2 tap=1.01875.
#[test]
fn control_loop_regulates_two_bus_to_oracle_tap() {
    let mut dss = Dss::new();
    reg_two_bus(&mut dss, "winding=2 vreg=122 band=0.0001 ptratio=20");
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let ckt = dss.circuit().unwrap();
    assert!(ckt.is_solved);
    assert_eq!(ckt.solution.iteration, 6);
    let taps = dss.transformer_taps();
    assert_eq!(taps[0].0, "t1");
    assert!(
        (taps[0].1[1] - 1.01875).abs() < 1e-12,
        "winding-2 tap = {}",
        taps[0].1[1]
    );
}

/// WP5.7 step 5: a control that cannot settle within `maxcontroliter`
/// stops with the 485 warning and aborts the solution. Oracle probe:
/// `maxcontroliter=2` + `maxtapchange=1` → iterations=4, tap=1.00625,
/// error 485; the next *external* command resets the abort flag
/// (CAPI `Text_Set_Command`).
#[test]
fn max_control_iterations_exceeded_warns_and_aborts() {
    let mut dss = Dss::new();
    reg_two_bus(
        &mut dss,
        "winding=2 vreg=122 band=2 ptratio=20 maxtapchange=1",
    );
    dss.command("Set maxcontroliter=2");
    dss.command("Solve");
    assert!(
        dss.errors()
            .iter()
            .any(|e| e.starts_with("Warning Max Control Iterations Exceeded.")),
        "{:?}",
        dss.errors()
    );
    let ckt = dss.circuit().unwrap();
    assert_eq!(ckt.solution.iteration, 4);
    assert!(ckt.solution.solution_abort);
    let taps = dss.transformer_taps();
    assert!(
        (taps[0].1[1] - 1.00625).abs() < 1e-12,
        "winding-2 tap = {}",
        taps[0].1[1]
    );
    // External commands reset the abort (the oracle solves again and
    // exceeds again rather than reporting "Solution aborted.").
    dss.command("Get hour");
    assert!(!dss.circuit().unwrap().solution.solution_abort);
}

/// Build the 2-bus + line + load + two-generator micro-circuit the
/// GenDispatcher oracle probes used.
fn gen_disp_two_bus(dss: &mut Dss, gd_props: &str) {
    dss.command("New circuit.a basekv=12.47 bus1=src phases=3");
    dss.command("New line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6");
    dss.command("New load.ld1 bus1=b1 phases=3 kv=12.47 kw=5000 pf=0.95");
    dss.command("New generator.g1 bus1=b1 phases=3 kv=12.47 kw=1000 pf=1.0 model=1");
    dss.command("New generator.g2 bus1=b1 phases=3 kv=12.47 kw=1000 pf=1.0 model=1");
    dss.command(&format!(
        "New gendispatcher.gd1 element=line.l1 terminal=1 {gd_props}"
    ));
    dss.command("Set voltagebases=[12.47]");
    dss.command("CalcVoltageBases");
}

/// WP6.8: the control loop's GenDispatcher redispatches its generators so
/// the monitored line power approaches `kWLimit`. Oracle probe (pinned
/// dss-python): equal weights → g1 = g2 = 1511.569498763734 kW.
#[test]
fn gendispatcher_redispatches_to_oracle() {
    let mut dss = Dss::new();
    gen_disp_two_bus(
        &mut dss,
        "kwlimit=2000 kwband=100 genlist=[g1,g2] weights=[1,1]",
    );
    dss.command("Solve mode=snap");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    for g in ["g1", "g2"] {
        let (kw, _kvar) = dss.generator_kw_kvar(g).unwrap();
        assert!((kw - 1511.569498763734).abs() < 1e-6, "{g} kW = {kw}");
    }
}

/// Weighted redispatch [3, 1]: g1 = 1767.3542481456006, g2 = 1255.7847493818672.
#[test]
fn gendispatcher_respects_weights_oracle() {
    let mut dss = Dss::new();
    gen_disp_two_bus(
        &mut dss,
        "kwlimit=2000 kwband=100 genlist=[g1,g2] weights=[3,1]",
    );
    dss.command("Solve mode=snap");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let (kw1, _) = dss.generator_kw_kvar("g1").unwrap();
    let (kw2, _) = dss.generator_kw_kvar("g2").unwrap();
    assert!((kw1 - 1767.3542481456006).abs() < 1e-6, "g1 kW = {kw1}");
    assert!((kw2 - 1255.7847493818672).abs() < 1e-6, "g2 kW = {kw2}");
}

/// No GenList → dispatch every enabled generator (uniform weights); same
/// result as the explicit equal-weight list.
#[test]
fn gendispatcher_no_list_dispatches_all_gens() {
    let mut dss = Dss::new();
    gen_disp_two_bus(&mut dss, "kwlimit=2000 kwband=100");
    dss.command("Solve mode=snap");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    for g in ["g1", "g2"] {
        let (kw, _) = dss.generator_kw_kvar(g).unwrap();
        assert!((kw - 1511.569498763734).abs() < 1e-6, "{g} kW = {kw}");
    }
}

/// WP6.8: the QDiff (kvar) redispatch path, exercised end-to-end. The gens
/// run at `pf=0.95` so they carry a dispatchable `kvarBase`, and both
/// `kWLimit` and `kvarLimit` bind. Oracle probe (pinned dss-python): equal
/// weights → g1 = g2 = (1509.8126154343354 kW, 591.2600618943429 kvar).
#[test]
fn gendispatcher_redispatches_kvar_to_oracle() {
    let mut dss = Dss::new();
    dss.command("New circuit.a basekv=12.47 bus1=src phases=3");
    dss.command("New line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6");
    dss.command("New load.ld1 bus1=b1 phases=3 kv=12.47 kw=5000 pf=0.95");
    dss.command("New generator.g1 bus1=b1 phases=3 kv=12.47 kw=1000 pf=0.95 model=1");
    dss.command("New generator.g2 bus1=b1 phases=3 kv=12.47 kw=1000 pf=0.95 model=1");
    dss.command(
        "New gendispatcher.gd1 element=line.l1 terminal=1 \
             kwlimit=2000 kwband=100 kvarlimit=500 genlist=[g1,g2] weights=[1,1]",
    );
    dss.command("Set voltagebases=[12.47]");
    dss.command("CalcVoltageBases");
    dss.command("Solve mode=snap");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    for g in ["g1", "g2"] {
        let (kw, kvar) = dss.generator_kw_kvar(g).unwrap();
        assert!((kw - 1509.8126154343354).abs() < 1e-6, "{g} kW = {kw}");
        assert!((kvar - 591.2600618943429).abs() < 1e-6, "{g} kvar = {kvar}");
    }
}

/// WP6.8: the monitored *terminal* is honored (not hard-wired to 1). A later
/// `terminal=2` overrides the helper's `terminal=1`; terminal 2 of the line
/// sits at the load/gen bus, so the measured power drives `PDiff` strongly
/// negative and both gens floor at `Max(1.0, …)` — a result distinct from
/// terminal 1's 1511.57 kW, which pins that the terminal index is read.
/// Oracle probe (pinned dss-python): g1 = g2 = 1.0 kW.
#[test]
fn gendispatcher_honors_monitored_terminal() {
    let mut dss = Dss::new();
    gen_disp_two_bus(
        &mut dss,
        "kwlimit=2000 kwband=100 terminal=2 genlist=[g1,g2] weights=[1,1]",
    );
    dss.command("Solve mode=snap");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    for g in ["g1", "g2"] {
        let (kw, _) = dss.generator_kw_kvar(g).unwrap();
        assert!((kw - 1.0).abs() < 1e-6, "{g} kW = {kw}");
    }
}

/// WP6.8 StorageController skeleton: a circuit carrying a StorageController
/// (whose fleet is always empty in Phase 6) must still solve — the control
/// sweep treats it as an inert no-op. The only logged error is the faithful
/// 37201 ("No unassigned Storage Elements found") emitted at parse-time
/// RecalcElementData, exactly as the oracle reports on a Storage-less circuit.
#[test]
fn storagecontroller_skeleton_solves_as_noop() {
    let mut dss = Dss::new();
    dss.command("new circuit.a basekv=12.47 bus1=src phases=3");
    dss.command("new line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6");
    dss.command("new load.ld1 bus1=b1 phases=3 kv=12.47 kw=3000 pf=0.95");
    dss.command("new storagecontroller.sc1 element=line.l1 terminal=1");
    // The 37201 is logged during the New command; everything after solves.
    let errs: Vec<String> = dss.errors().to_vec();
    assert_eq!(errs.len(), 1, "{errs:?}");
    assert!(errs[0].contains("No unassigned Storage Elements found"));

    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command("solve mode=snap");
    // No *new* errors from the control loop; the circuit converged.
    assert_eq!(dss.errors().len(), 1, "{:?}", dss.errors());
    assert!(dss.circuit().unwrap().solution.converged_flag);
}

/// WP5.8 step 6: time-option round trips, all values transcribed from the
/// pinned oracle.
#[test]
fn time_options_round_trip_matches_oracle() {
    let query = |dss: &mut Dss, what: &str| -> String {
        dss.command(&format!("get {what}"));
        dss.result().to_string()
    };
    let mut dss = Dss::new();
    dss.command("new circuit.t2");
    dss.command("set stepsize=15m");
    assert_eq!(query(&mut dss, "stepsize"), "900");
    dss.command("set hour=5");
    assert_eq!(query(&mut dss, "hour"), "5");
    dss.command("set sec=120.5");
    assert_eq!(query(&mut dss, "sec"), "120.5");
    dss.command("set time=(2, 1800)");
    assert_eq!(query(&mut dss, "time"), "[ 2, 1800 ] !... 2.5 (hours)");
    assert_eq!(query(&mut dss, "hour"), "2");
    assert_eq!(query(&mut dss, "sec"), "1800");
    dss.command("set mode=daily");
    assert_eq!(query(&mut dss, "number"), "24");
    assert_eq!(query(&mut dss, "stepsize"), "3600");
    // DUTYCYCLE forces TIMEDRIVEN control mode and h = 1 s.
    dss.command("set mode=duty");
    assert_eq!(query(&mut dss, "controlmode"), "Time");
    assert_eq!(query(&mut dss, "stepsize"), "1");
    // Circuit defaults resolve to the built-in `loadshape.default`.
    assert_eq!(query(&mut dss, "defaultdaily"), "default");
    assert_eq!(query(&mut dss, "defaultyearly"), "default");
    assert_eq!(query(&mut dss, "pricesignal"), "25");
    assert_eq!(query(&mut dss, "pricecurve"), "");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
}

/// WP5.8: a daily-mode solve steps the clock through `number` steps.
#[test]
fn daily_mode_advances_the_clock_and_solves() {
    let mut dss = Dss::new();
    dss.command("New circuit.d1 basekv=12.47 pu=1.0 phases=3 mvasc3=2000");
    dss.command("New loadshape.two npts=2 interval=1 mult=(0.5 1.0)");
    dss.command("New line.l1 bus1=sourcebus bus2=b2 r1=0.1 x1=0.2 length=1");
    dss.command("New load.ld bus1=b2 phases=3 kv=12.47 kw=500 pf=0.95 daily=two");
    dss.command("Set voltagebases=[12.47]");
    dss.command("CalcVoltageBases");
    dss.command("Set mode=daily stepsize=1h number=2");
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let ckt = dss.circuit().unwrap();
    assert!(ckt.is_solved);
    // IncrementTime runs before each step: after 2 steps dblHour = 2.0
    // (oracle probe: dblHour 2.0, Hour 2, Seconds 0.0).
    assert_eq!(ckt.solution.int_hour, 2);
    assert_eq!(ckt.solution.dbl_hour, 2.0);
    assert_eq!(ckt.solution.t, 0.0);
    // The built-in default daily shape drove DefaultHourMult (the value
    // itself is the WP5.2 oracle-pinned GetMultAtHour).
    let expected = ckt
        .default_daily_shape_obj
        .clone()
        .expect("default shape resolved")
        .get_mult_at_hour(2.0);
    assert_eq!(ckt.default_hour_mult, expected);
}

/// WP5.8 step 5: `BusCoords` reads `bus, x, y` rows, skipping unknown
/// buses silently; coordinates survive on existing buses.
#[test]
fn bus_coords_sets_coordinates_on_existing_buses() {
    let dir = std::env::temp_dir().join("dss_rs_buscoords_test");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("coords.csv");
    std::fs::write(&file, "sourcebus, 10.5, -3\nnosuchbus, 1, 2\nb2 7 8\n").unwrap();

    let mut dss = Dss::new();
    dss.command("New circuit.bc basekv=12.47 pu=1.0 phases=3");
    dss.command("New line.l1 bus1=sourcebus bus2=b2 r1=0.1 x1=0.2 length=1");
    dss.command("Set voltagebases=[12.47]");
    dss.command("CalcVoltageBases"); // builds the bus list
    dss.command(&format!(
        "BusCoords \"{}\"",
        file.to_string_lossy().replace('\\', "/")
    ));
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let ckt = dss.circuit().unwrap();
    let sb = ckt.bus_list.find("sourcebus").unwrap();
    assert!(ckt.buses[sb].coord_defined);
    assert_eq!((ckt.buses[sb].x, ckt.buses[sb].y), (10.5, -3.0));
    let b2 = ckt.bus_list.find("b2").unwrap();
    assert_eq!((ckt.buses[b2].x, ckt.buses[b2].y), (7.0, 8.0));
    std::fs::remove_file(&file).ok();
}

/// The micro radial of PHASE6_PLAN §1.2 `meter_zone_micro`: one meter on the
/// head line walks the whole feeder. Zone branches/ends/PCE transcribed from
/// the oracle (dss-python 0.15.7 `Meters.AllBranchesInZone` /
/// `AllEndElements` / `ZonePCE`).
fn micro_zone_script() -> Vec<&'static str> {
    vec![
        "New circuit.test basekv=12.47 bus1=src",
        "New line.l1 bus1=src bus2=b2 length=1",
        "New line.l2 bus1=b2 bus2=b3 length=2",
        "New line.l3 bus1=b2 bus2=b4 length=1",
        "New load.ld1 bus1=b3 kV=12.47 kW=100 numcust=3",
        "New load.ld2 bus1=b4 kV=12.47 kW=50 numcust=2",
    ]
}

#[test]
fn energymeter_zone_radial() {
    let mut dss = Dss::new();
    for c in micro_zone_script() {
        dss.command(c);
    }
    dss.command("New energymeter.m1 element=line.l1 terminal=1");
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    let z = dss.meter_zone("m1").expect("m1 zone");
    assert_eq!(
        z.all_branches_in_zone,
        vec!["Line.l1", "Line.l3", "Line.l2"]
    );
    assert_eq!(z.all_end_elements, vec!["Line.l3", "Line.l2"]);
    assert_eq!(z.zone_pce, vec!["Load.ld2", "Load.ld1"]);

    // TotalUpDownstreamCustomers: ld1=3 on l2, ld2=2 on l3; l1 totals 5.
    assert_eq!(branch_customers(&dss, "line.l2"), (3, 3));
    assert_eq!(branch_customers(&dss, "line.l3"), (2, 2));
    assert_eq!(branch_customers(&dss, "line.l1"), (0, 5));
}

#[test]
fn energymeter_submeter_boundary() {
    let mut dss = Dss::new();
    for c in micro_zone_script() {
        dss.command(c);
    }
    dss.command("New energymeter.m1 element=line.l1 terminal=1");
    dss.command("New energymeter.m2 element=line.l2 terminal=1");
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    // m1's zone stops at the sub-meter on l2.
    let z1 = dss.meter_zone("m1").expect("m1 zone");
    assert_eq!(z1.all_branches_in_zone, vec!["Line.l1", "Line.l3"]);
    assert_eq!(z1.all_end_elements, vec!["Line.l3"]);
    assert_eq!(z1.zone_pce, vec!["Load.ld2"]);

    let z2 = dss.meter_zone("m2").expect("m2 zone");
    assert_eq!(z2.all_branches_in_zone, vec!["Line.l2"]);
    assert_eq!(z2.all_end_elements, vec!["Line.l2"]);
    assert_eq!(z2.zone_pce, vec!["Load.ld1"]);
}

/// `element=` must resolve to a PD element; a load triggers the Pascal
/// "is not a Power Delivery (PD) element" error (525).
#[test]
fn energymeter_requires_pd_element() {
    let mut dss = Dss::new();
    dss.command("New circuit.test basekv=12.47 bus1=src");
    dss.command("New line.l1 bus1=src bus2=b2 length=1");
    dss.command("New load.ld1 bus1=b2 kV=12.47 kW=100");
    dss.command("New energymeter.m1 element=load.ld1");
    assert!(
        dss.errors()
            .iter()
            .any(|e| e.contains("not a Power Delivery")),
        "expected PD-element error, got {:?}",
        dss.errors()
    );
}

/// Parallel lines (l2a ∥ l2b, both b2→b3): both still join the zone; the
/// `IsParallel` flag is internal metadata, not an exclusion. Branch/end/PCE
/// order transcribed from the oracle (`Meters.AllBranchesInZone` etc.).
#[test]
fn energymeter_parallel_lines() {
    let mut dss = Dss::new();
    dss.command("New circuit.test basekv=12.47 bus1=src");
    dss.command("New line.l1 bus1=src bus2=b2 length=1");
    dss.command("New line.l2a bus1=b2 bus2=b3 length=2");
    dss.command("New line.l2b bus1=b2 bus2=b3 length=2");
    dss.command("New load.ld1 bus1=b3 kV=12.47 kW=100 numcust=1");
    dss.command("New energymeter.m1 element=line.l1 terminal=1");
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    let z = dss.meter_zone("m1").expect("m1 zone");
    assert_eq!(
        z.all_branches_in_zone,
        vec!["Line.l1", "Line.l2b", "Line.l2a"]
    );
    assert_eq!(z.all_end_elements, vec!["Line.l2b", "Line.l2a"]);
    assert_eq!(z.zone_pce, vec!["Load.ld1"]);
}

/// A meshed zone (l4 closes b4→b2 back to the head): the loop branch is
/// detected and not re-added, so the walk terminates. Order from the oracle.
#[test]
fn energymeter_loop_zone() {
    let mut dss = Dss::new();
    dss.command("New circuit.test basekv=12.47 bus1=src");
    dss.command("New line.l1 bus1=src bus2=b2 length=1");
    dss.command("New line.l2 bus1=b2 bus2=b3 length=1");
    dss.command("New line.l3 bus1=b3 bus2=b4 length=1");
    dss.command("New line.l4 bus1=b4 bus2=b2 length=1");
    dss.command("New energymeter.m1 element=line.l1 terminal=1");
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    let z = dss.meter_zone("m1").expect("m1 zone");
    assert_eq!(
        z.all_branches_in_zone,
        vec!["Line.l1", "Line.l4", "Line.l3", "Line.l2"]
    );
    assert_eq!(z.all_end_elements, vec!["Line.l3", "Line.l2"]);
    assert!(z.zone_pce.is_empty());
}

/// A transformer crossing voltage bases (12.47→0.48 kV) drives
/// `AddToVoltBaseList` to two slots; `AssignVoltBaseRegisterNames` names the
/// per-base loss registers (`%.3g kV …`) and fills the unused slots with
/// `Aux<n>`. Register names + branch order transcribed from the oracle.
#[test]
fn energymeter_multi_vbase_register_names() {
    let mut dss = Dss::new();
    dss.command("New circuit.test basekv=12.47 bus1=src");
    dss.command("New line.l1 bus1=src bus2=b2 length=1");
    dss.command(
        "New transformer.tx phases=3 windings=2 buses=[b2, b3] \
             conns=[wye, wye] kvs=[12.47, 0.48] kvas=[500, 500] xhl=5",
    );
    dss.command("New line.l2 bus1=b3 bus2=b4 length=1");
    dss.command("New load.ld1 bus1=b4 kV=0.48 kW=100");
    dss.command("New energymeter.m1 element=line.l1 terminal=1");
    dss.command("Set voltagebases=[12.47, 0.48]");
    dss.command("CalcVoltageBases");
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    let z = dss.meter_zone("m1").expect("m1 zone");
    assert_eq!(
        z.all_branches_in_zone,
        vec!["Line.l1", "Transformer.tx", "Line.l2"]
    );
    // VBaseStart = 32; the two bases occupy slots 0/1, the rest are Aux.
    assert_eq!(z.register_names[32], "12.5 kV Losses");
    assert_eq!(z.register_names[33], "0.48 kV Losses");
    assert_eq!(z.register_names[34], "Aux1");
    assert_eq!(z.register_names[35], "Aux6");
    assert_eq!(z.register_names[39], "12.5 kV Line Loss");
    assert_eq!(z.register_names[40], "0.48 kV Line Loss");
}

/// Manual `ZoneList` zone build. NOTE: the oracle (dss_capi 0.14.5) raises an
/// **access violation** on a manual zone, so there is no golden — this test
/// locks our deterministic, memory-safe behavior, and guards against the
/// path silently degrading back to a no-op. The listed PD element is chained
/// as a child of the metered branch (no connectivity/feeder-ends).
#[test]
fn energymeter_manual_zonelist() {
    let mut dss = Dss::new();
    dss.command("New circuit.test basekv=12.47 bus1=src");
    dss.command("New line.l1 bus1=src bus2=b2 length=1");
    dss.command("New line.l2 bus1=b2 bus2=b3 length=2");
    dss.command("New line.lx bus1=b2 bus2=b9 length=1"); // not in the zonelist
    dss.command("New load.ld1 bus1=b3 kV=12.47 kW=100 numcust=3");
    dss.command("New energymeter.m1 element=line.l1 terminal=1 zonelist=[line.l2]");
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    let z = dss.meter_zone("m1").expect("m1 zone");
    // l2 is chained from l1; lx (not listed) is excluded. The downstream
    // load at l2's far bus is still collected.
    assert_eq!(z.all_branches_in_zone, vec!["Line.l1", "Line.l2"]);
    assert!(!z.all_branches_in_zone.iter().any(|b| b == "Line.lx"));
    assert_eq!(z.zone_pce, vec!["Load.ld1"]);
    // Manual zones populate no feeder ends (Pascal skips ZoneEndsList).
    assert!(z.all_end_elements.is_empty());
}

/// A terminal number past the metered element's terminal count is the Pascal
/// 524 "Terminal no. ... does not exist" error.
#[test]
fn energymeter_bad_terminal() {
    let mut dss = Dss::new();
    dss.command("New circuit.test basekv=12.47 bus1=src");
    dss.command("New line.l1 bus1=src bus2=b2 length=1");
    dss.command("New energymeter.m1 element=line.l1 terminal=3");
    assert!(
        dss.errors().iter().any(|e| e.contains("does not exist")),
        "expected terminal-does-not-exist error, got {:?}",
        dss.errors()
    );
}

/// A disabled meter builds no zone (Pascal `BranchList := NIL`): the zone
/// lists are empty. (The oracle errs #5501 on `AllBranchesInZone` here; we
/// expose the empty zone instead of erroring.)
#[test]
fn energymeter_disabled_empty_zone() {
    let mut dss = Dss::new();
    dss.command("New circuit.test basekv=12.47 bus1=src");
    dss.command("New line.l1 bus1=src bus2=b2 length=1");
    dss.command("New load.ld1 bus1=b2 kV=12.47 kW=100");
    dss.command("New energymeter.m1 element=line.l1 terminal=1 enabled=no");
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    let z = dss.meter_zone("m1").expect("m1 exists");
    assert!(z.all_branches_in_zone.is_empty());
    assert!(z.zone_pce.is_empty());
}

/// Pascal gates `EndEdit` recalc on `NeedsRecalc`: a meter created without an
/// `element` (or edited on an unrelated property) must NOT raise the
/// "Circuit Element not set" error. Oracle: such a meter is created cleanly.
#[test]
fn energymeter_no_element_no_revalidation() {
    let mut dss = Dss::new();
    dss.command("New circuit.test basekv=12.47 bus1=src");
    dss.command("New line.l1 bus1=src bus2=b2 length=1");
    dss.command("New energymeter.mz"); // no element set
    assert!(
        dss.errors().is_empty(),
        "a bare meter must not error, got {:?}",
        dss.errors()
    );
    // Editing an unrelated property on a valid meter must not re-validate.
    dss.command("New load.ld1 bus1=b2 kV=12.47 kW=100");
    dss.command("New energymeter.m1 element=line.l1 terminal=1");
    dss.command("Edit energymeter.m1 kVANormal=5000");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
}

/// Helper: `(BranchNumCustomers, BranchTotalCustomers)` for a named element.
fn branch_customers(dss: &Dss, full: &str) -> (i32, i32) {
    let (cls, name) = full.split_once('.').unwrap();
    for class in &dss.classes {
        if !class.props.class_name().eq_ignore_ascii_case(cls) {
            continue;
        }
        for obj in &class.objects {
            if obj.data().name().eq_ignore_ascii_case(name)
                && let Some(e) = obj.as_ckt_element()
            {
                return (e.cd().branch_num_customers, e.cd().branch_total_customers);
            }
        }
    }
    panic!("element {full} not found");
}

/// Helper: fetch a meter register value by name.
fn meter_reg(dss: &Dss, meter: &str, reg_name: &str) -> f64 {
    dss.meter_registers(meter)
        .unwrap_or_else(|| panic!("meter {meter} not found"))
        .into_iter()
        .find(|(n, _)| n == reg_name)
        .unwrap_or_else(|| panic!("register {reg_name} not found"))
        .1
}

/// Build the 2-bus daily case shared by the register tests. Loadshape ramps
/// 1→2→3 over 3 one-hour steps; the meter is on the source line.
fn daily_meter_case(trapezoidal: bool) -> Dss {
    let mut dss = Dss::new();
    dss.command("New circuit.t basekv=12.47 bus1=src");
    dss.command("New loadshape.ls npts=3 interval=1 mult=(1.0 2.0 3.0)");
    dss.command("New line.l1 bus1=src bus2=b2 length=1 r1=0.1 x1=0.1");
    dss.command("New load.ld1 bus1=b2 kV=12.47 kW=1000 pf=1 model=1 daily=ls");
    dss.command("New energymeter.m1 element=line.l1 terminal=1");
    dss.command("Set voltagebases=[12.47]");
    dss.command("CalcVoltageBases");
    // `Set mode=` resets the trapezoidal flag, so set it afterwards.
    dss.command("Set mode=daily number=3 stepsize=1h time=(0,0)");
    dss.command(if trapezoidal {
        "Set trapezoidal=yes"
    } else {
        "Set trapezoidal=no"
    });
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss
}

/// Plain Euler integration: `kWh = Σ interval·P`. Oracle (dss-python
/// 0.15.7) register values for the 1→2→3 daily ramp.
#[test]
fn energymeter_daily_registers_euler() {
    let dss = daily_meter_case(false);
    let approx = |got: f64, want: f64| {
        assert!(
            (got - want).abs() <= 1e-6 * want.abs().max(1.0),
            "got {got}, want {want}"
        );
    };
    approx(meter_reg(&dss, "m1", "kWh"), 6009.009963043285);
    approx(meter_reg(&dss, "m1", "kvarh"), 8.436633138535129);
    approx(meter_reg(&dss, "m1", "Zone kWh"), 5999.979170340599);
    approx(meter_reg(&dss, "m1", "Max kW"), 3005.7947873123644);
    approx(meter_reg(&dss, "m1", "Line Losses"), 9.038717950944731);
    approx(
        meter_reg(&dss, "m1", "Zone Max kW Losses"),
        5.814433198962477,
    );
    // The single voltage base bucket carries the same line losses.
    approx(
        meter_reg(&dss, "m1", "12.5 kV Line Loss"),
        9.038717950944731,
    );
}

/// Trapezoidal integration: the first sample after reset is skipped, then
/// `kWh += 0.5·interval·(P + P_prev)`. Same circuit, oracle values.
#[test]
fn energymeter_daily_registers_trapezoidal() {
    let dss = daily_meter_case(true);
    let approx = |got: f64, want: f64| {
        assert!(
            (got - want).abs() <= 1e-6 * want.abs().max(1.0),
            "got {got}, want {want}"
        );
    };
    approx(meter_reg(&dss, "m1", "kWh"), 4005.7905368432225);
    approx(meter_reg(&dss, "m1", "Zone kWh"), 3999.9865469246124);
    // Drag-hand maxima are independent of the integration rule.
    approx(meter_reg(&dss, "m1", "Max kW"), 3005.7947873123644);
    approx(
        meter_reg(&dss, "m1", "Zone Max kW Losses"),
        5.814433198962477,
    );
}

/// `Reset Meters` zeroes the registers and re-primes the drag-hand maxima to
/// the large-negative sentinel.
#[test]
fn energymeter_reset_registers() {
    let mut dss = daily_meter_case(false);
    assert!(meter_reg(&dss, "m1", "kWh") > 1.0, "registers accumulated");
    dss.command("Reset Meters");
    assert_eq!(meter_reg(&dss, "m1", "kWh"), 0.0);
    assert_eq!(meter_reg(&dss, "m1", "Zone kWh"), 0.0);
    // Drag-hand registers reset to -1e50.
    assert_eq!(meter_reg(&dss, "m1", "Max kW"), -1.0e50);
    assert_eq!(meter_reg(&dss, "m1", "Zone Max kW Losses"), -1.0e50);
}

/// Helper used by the new register tests: build a 3-step daily case from a
/// list of `New ...` commands, run it, and return the solved `Dss`. The
/// loadshape `ls` (1→2→3) and the daily-mode/trapezoidal-off boilerplate are
/// shared; callers pass the topology + `voltagebases`.
fn meter_case(decls: &[&str], voltagebases: &str, extra_set: &[&str]) -> Dss {
    let mut dss = Dss::new();
    dss.command("New circuit.t basekv=12.47 bus1=src");
    dss.command("New loadshape.ls npts=3 interval=1 mult=(1.0 2.0 3.0)");
    for d in decls {
        dss.command(d);
    }
    dss.command(&format!("Set voltagebases=[{voltagebases}]"));
    dss.command("CalcVoltageBases");
    for s in extra_set {
        dss.command(s);
    }
    dss.command("Set mode=daily number=3 stepsize=1h time=(0,0)");
    dss.command("Set trapezoidal=no");
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss
}

fn approx_meter(dss: &Dss, reg_name: &str, want: f64) {
    let got = meter_reg(dss, "m1", reg_name);
    assert!(
        (got - want).abs() <= 1e-6 * want.abs().max(1.0),
        "{reg_name}: got {got}, want {want}"
    );
}

/// A generator in the zone accumulates the Gen registers (`Accumulate_Gen`:
/// `−Power[1]·0.001` into the gen totals, *not* the zone-load totals). Oracle
/// values for a 500 kW gen on the same 1→2→3 daily ramp.
#[test]
fn energymeter_generator_registers() {
    let dss = meter_case(
        &[
            "New line.l1 bus1=src bus2=b2 length=1 r1=0.1 x1=0.1",
            "New load.ld1 bus1=b2 kV=12.47 kW=1000 pf=1 model=1 daily=ls",
            "New generator.g1 bus1=b2 kV=12.47 kW=500 pf=1 model=1 daily=ls",
            "New energymeter.m1 element=line.l1 terminal=1",
        ],
        "12.47",
        &[],
    );
    approx_meter(&dss, "Gen kWh", 2999.9974045506274);
    approx_meter(&dss, "Gen kvarh", -0.0010792012877156054);
    approx_meter(&dss, "Gen Max kW", 1499.9981626190265);
    approx_meter(&dss, "Gen Max kVA", 1499.9981626191664);
    // Zone load is unaffected by the generator (gen has its own totals).
    approx_meter(&dss, "Zone kWh", 5999.994809101255);
}

/// 3-phase line sequence-mode loss split (`GetSeqLosses`, 3-phase only):
/// balanced line ⇒ all loss in the positive/line mode, ~0 zero-mode.
#[test]
fn energymeter_sequence_mode_losses() {
    let dss = meter_case(
        &[
            "New line.l1 bus1=src bus2=b2 length=1 r1=0.1 x1=0.1",
            "New load.ld1 bus1=b2 kV=12.47 kW=1000 pf=1 model=1 daily=ls",
            "New energymeter.m1 element=line.l1 terminal=1",
        ],
        "12.47",
        &[],
    );
    approx_meter(&dss, "Line Mode Line Losses", 9.038717950944614);
    approx_meter(&dss, "3-phase Line Losses", 9.038717950944731);
    approx_meter(&dss, "1- and 2-phase Line Losses", 0.0);
    // Balanced ⇒ zero-sequence loss is numerically ~0 (1e-20).
    assert!(
        meter_reg(&dss, "m1", "Zero Mode Line Losses").abs() < 1e-9,
        "zero-mode loss should be ~0 for a balanced line"
    );
}

/// A transformer in the zone exercises the load/no-load loss split
/// (`GetLosses` override) and the second voltage-base bucket (the 4.16 kV
/// secondary, reached via line `l2`). Oracle values.
#[test]
fn energymeter_transformer_loss_split_and_vbase() {
    let dss = meter_case(
        &[
            "New line.l1 bus1=src bus2=b2 length=1 r1=0.1 x1=0.1",
            "New transformer.t1 windings=2 buses=(b2 b3) conns=(wye wye) \
                 kvs=(12.47 4.16) kvas=(2000 2000) xhl=5 %loadloss=1 %noloadloss=0.2",
            "New line.l2 bus1=b3 bus2=b4 length=0.5 r1=0.05 x1=0.05",
            "New load.ld1 bus1=b4 kV=4.16 kW=1000 pf=1 model=1 daily=ls",
            "New energymeter.m1 element=line.l1 terminal=1",
        ],
        "12.47 4.16",
        &[],
    );
    approx_meter(&dss, "Transformer Losses", 85.06511357026721);
    approx_meter(&dss, "Load Losses kWh", 103.96306991988196);
    approx_meter(&dss, "No Load Losses kWh", 11.675484334236636);
    approx_meter(&dss, "Line Losses", 30.57344068385137);
    // First voltage-base bucket (12.5 kV primary side): transformer split.
    approx_meter(&dss, "12.5 kV Load Loss", 73.389629);
    approx_meter(&dss, "12.5 kV No Load Loss", 11.675484);
    // Second voltage-base bucket (4.16 kV secondary): line l2 losses +
    // the load energy bucketed by its parent branch's voltage base.
    approx_meter(&dss, "4.16 kV Line Loss", 21.134364);
    approx_meter(&dss, "4.16 kV Load Energy", 5999.665065);
}

/// An under-rated line drives the overload registers and the *radial*
/// EEN/UE marking (`ExcesskVANorm/Emerg` set `Overload_EEN/UE`, loads marked
/// by the degree of overload). Oracle values.
#[test]
fn energymeter_overload_and_radial_een_ue() {
    let dss = meter_case(
        &[
            "New line.l1 bus1=src bus2=b2 length=1 r1=0.1 x1=0.1 normamps=50 emergamps=70",
            "New load.ld1 bus1=b2 kV=12.47 kW=1000 pf=1 model=1 daily=ls",
            "New energymeter.m1 element=line.l1 terminal=1",
        ],
        "12.47",
        &[],
    );
    approx_meter(&dss, "Overload kWh Normal", 2849.1631405361386);
    approx_meter(&dss, "Overload kWh Emerg", 1985.482037568384);
    approx_meter(&dss, "Load EEN", 7062.605747589624);
    approx_meter(&dss, "Load UE", 3616.152913382251);
}

/// A high-impedance line with ample current rating: no line overload, so the
/// EEN/UE come from the load's *voltage* criterion (`ExceedsNormal`/
/// `Unserved`, `VminNormal`/`VminEmerg` defaults). Oracle values.
#[test]
fn energymeter_voltage_een_ue() {
    let dss = meter_case(
        &[
            "New line.l1 bus1=src bus2=b2 length=1 r1=2 x1=2 normamps=2000 emergamps=3000",
            "New load.ld1 bus1=b2 kV=12.47 kW=4000 pf=1 model=1 daily=ls",
            "New energymeter.m1 element=line.l1 terminal=1",
        ],
        "12.47",
        &["Set normvminpu=0.95 emergvminpu=0.90"],
    );
    // No line overload ⇒ overload-energy registers stay 0.
    approx_meter(&dss, "Overload kWh Normal", 0.0);
    approx_meter(&dss, "Overload kWh Emerg", 0.0);
    // EEN/UE come purely from the voltage criterion.
    approx_meter(&dss, "Load EEN", 28188.694199630165);
    approx_meter(&dss, "Load UE", 11344.628473647135);
}

/// `Reset` (no argument) must reset controls too (Pascal `DoResetControls`):
/// a CapControl that opened its bank during the solve has the bank driven
/// back to its `InitialState` (closed) by the reset.
#[test]
fn reset_command_resets_controls() {
    let mut dss = Dss::new();
    dss.command("New circuit.t basekv=12.47 bus1=src");
    dss.command("New line.l1 bus1=src bus2=b2 length=1 r1=0.5 x1=1.0");
    dss.command("New load.ld1 bus1=b2 kV=12.47 kW=50 pf=0.99 model=1");
    dss.command("New capacitor.c1 bus1=b2 kV=12.47 kvar=600 numsteps=1");
    // kvar control opens the bank when the sensed kvar is below `offsetting`;
    // the tiny load keeps it below, so the solve switches the bank OUT.
    dss.command(
        "New capcontrol.cc1 element=line.l1 terminal=1 capacitor=c1 \
             type=kvar ptratio=1 onsetting=200 offsetting=100",
    );
    dss.command("Set voltagebases=[12.47]");
    dss.command("CalcVoltageBases");
    dss.command("Solve mode=snap");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    // The control opened the bank during the solve.
    assert_eq!(
        dss.capacitor_closed("c1"),
        Some(false),
        "control should have opened the bank"
    );
    // No-arg Reset must run DoResetControls → bank back to InitialState.
    dss.command("Reset");
    assert_eq!(
        dss.capacitor_closed("c1"),
        Some(true),
        "Reset must reset controls (close the bank to InitialState)"
    );
}

// --- WP6.6 reliability ------------------------------------------------

/// Two-section radial feeder (src→b1→b2) with per-line fault data and a
/// load on each section. Solved snapshot, EnergyMeter on the source line.
/// `units=mi` keeps `len=1` (so the fault-rate math is unchanged) while
/// making `MilesThisLine = 1` per line for the miles-accumulator assertions.
fn reliability_feeder() -> Dss {
    let mut dss = Dss::new();
    dss.command("New circuit.t basekv=12.47 bus1=src phases=3");
    dss.command(
        "New line.l1 bus1=src bus2=b1 length=1 units=mi r1=0.1 x1=0.1 \
             faultrate=0.2 pctperm=80 repair=4",
    );
    dss.command(
        "New line.l2 bus1=b1 bus2=b2 length=1 units=mi r1=0.1 x1=0.1 \
             faultrate=0.3 pctperm=90 repair=5",
    );
    dss.command("New load.ld1 bus1=b1 phases=3 kv=12.47 kw=100 numcust=10");
    dss.command("New load.ld2 bus1=b2 phases=3 kv=12.47 kw=200 numcust=25");
    dss.command("New energymeter.m1 element=line.l1 terminal=1");
    dss.command("Set voltagebases=[12.47]");
    dss.command("CalcVoltageBases");
    dss.command("Solve mode=snap");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss
}

/// Branching feeder: src→b1, then two laterals b1→b2 and b1→b3, exercising
/// the parent customer roll-up at the junction bus b1.
fn branching_reliability_feeder() -> Dss {
    let mut dss = Dss::new();
    dss.command("New circuit.t basekv=12.47 bus1=src phases=3");
    dss.command(
        "New line.l1 bus1=src bus2=b1 length=1 units=mi r1=0.1 x1=0.1 \
             faultrate=0.2 pctperm=80",
    );
    dss.command(
        "New line.l2 bus1=b1 bus2=b2 length=1 units=mi r1=0.1 x1=0.1 \
             faultrate=0.3 pctperm=90",
    );
    dss.command(
        "New line.l3 bus1=b1 bus2=b3 length=1 units=mi r1=0.1 x1=0.1 \
             faultrate=0.5 pctperm=100",
    );
    dss.command("New load.ld1 bus1=b1 phases=3 kv=12.47 kw=100 numcust=10");
    dss.command("New load.ld2 bus1=b2 phases=3 kv=12.47 kw=200 numcust=25");
    dss.command("New load.ld3 bus1=b3 phases=3 kv=12.47 kw=150 numcust=7");
    dss.command("New energymeter.m1 element=line.l1 terminal=1");
    dss.command("Set voltagebases=[12.47]");
    dss.command("CalcVoltageBases");
    dss.command("Solve mode=snap");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss
}

fn bus_f64(dss: &Dss, bus: &str, f: impl Fn(&crate::circuit::bus::Bus) -> f64) -> f64 {
    let ckt = dss.circuit.as_ref().unwrap();
    let idx = ckt.bus_list.find(bus).expect("bus not found");
    f(&ckt.buses[idx])
}

fn bus_total_miles(dss: &Dss, bus: &str) -> f64 {
    bus_f64(dss, bus, |b| b.bus_total_miles)
}

fn bus_section_id(dss: &Dss, bus: &str) -> i32 {
    let ckt = dss.circuit.as_ref().unwrap();
    let idx = ckt.bus_list.find(bus).expect("bus not found");
    ckt.buses[idx].bus_section_id
}

fn accum_miles(dss: &Dss, full: &str) -> f64 {
    let (cls, name) = full.split_once('.').unwrap();
    for class in &dss.classes {
        if !class.props.class_name().eq_ignore_ascii_case(cls) {
            continue;
        }
        for obj in &class.objects {
            if obj.data().name().eq_ignore_ascii_case(name)
                && let Some(e) = obj.as_ckt_element()
            {
                return e.cd().accumulated_miles_downstream;
            }
        }
    }
    panic!("element {full} not found");
}

fn branch_section_id(dss: &Dss, full: &str) -> i32 {
    let (cls, name) = full.split_once('.').unwrap();
    for class in &dss.classes {
        if !class.props.class_name().eq_ignore_ascii_case(cls) {
            continue;
        }
        for obj in &class.objects {
            if obj.data().name().eq_ignore_ascii_case(name)
                && let Some(e) = obj.as_ckt_element()
            {
                return e.cd().branch_section_id;
            }
        }
    }
    panic!("element {full} not found");
}

fn meter_assume_restoration(dss: &Dss, name: &str) -> bool {
    for class in &dss.classes {
        for obj in &class.objects {
            if obj.data().name().eq_ignore_ascii_case(name)
                && let Some(em) = obj
                    .as_any()
                    .downcast_ref::<crate::elements::meter::energymeter::EnergyMeter>()
            {
                return em.assume_restoration();
            }
        }
    }
    panic!("meter {name} not found");
}

fn bus_flt_rate(dss: &Dss, bus: &str) -> f64 {
    let ckt = dss.circuit.as_ref().unwrap();
    let idx = ckt.bus_list.find(bus).expect("bus not found");
    ckt.buses[idx].bus_flt_rate
}

fn bus_total_custs(dss: &Dss, bus: &str) -> i32 {
    let ckt = dss.circuit.as_ref().unwrap();
    let idx = ckt.bus_list.find(bus).expect("bus not found");
    ckt.buses[idx].bus_total_num_customers
}

fn accum_flt_rate(dss: &Dss, full: &str) -> f64 {
    let (cls, name) = full.split_once('.').unwrap();
    for class in &dss.classes {
        if !class.props.class_name().eq_ignore_ascii_case(cls) {
            continue;
        }
        for obj in &class.objects {
            if obj.data().name().eq_ignore_ascii_case(name)
                && let Some(e) = obj.as_ckt_element()
            {
                return e.cd().accumulated_br_flt_rate;
            }
        }
    }
    panic!("element {full} not found");
}

/// With no OCP device (Relay/Recloser/Fuse — all Phase 7) the zone has zero
/// sections, so `Relcalc` aborts with error 52902 exactly like the oracle
/// (dss-python raises `DSSException (#52902)` on the same feeder).
#[test]
fn relcalc_no_ocp_device_aborts() {
    let mut dss = reliability_feeder();
    dss.command("Relcalc");
    assert!(
        dss.errors()
            .iter()
            .any(|e| e
                .contains("No Overcurrent Protection device (Relay, Recloser, or Fuse) defined")),
        "Relcalc must abort without OCP devices, got {:?}",
        dss.errors()
    );
}

/// Although `Relcalc` aborts, the backward fault-rate sweep and the
/// up/downstream customer rollup run *before* the section check, so the bus
/// and branch accumulators are populated. Hand-computed from the feeder:
/// `BranchFltRate = FaultRate·pctperm·0.01·Len` (0.16 for l1, 0.27 for l2);
/// `AccumulatedBrFltRate` rolls the downstream bus rate up each branch.
#[test]
fn relcalc_backward_sweep_accumulators() {
    let mut dss = reliability_feeder();
    dss.command("Relcalc");

    // l2: ToBus b2 has 0 fault rate → accumulated = its own 0.27.
    assert!((accum_flt_rate(&dss, "line.l2") - 0.27).abs() < 1e-12);
    // l1: ToBus b1 carries l2's 0.27 → accumulated = 0.27 + 0.16 = 0.43.
    assert!((accum_flt_rate(&dss, "line.l1") - 0.43).abs() < 1e-12);

    // FROM-bus accumulated failure rates (no OCP → roll up to FROM bus).
    assert!((bus_flt_rate(&dss, "src") - 0.43).abs() < 1e-12);
    assert!((bus_flt_rate(&dss, "b1") - 0.27).abs() < 1e-12);
    // b2 is a downstream (TO) bus only → never accumulated.
    assert!(bus_flt_rate(&dss, "b2").abs() < 1e-12);

    // Up/downstream customers: src sees all 35, b1 sees l2's 25.
    assert_eq!(bus_total_custs(&dss, "src"), 35);
    assert_eq!(bus_total_custs(&dss, "b1"), 25);
}

/// `AccumFltRate` also sweeps line miles: `AccumulatedMilesDownStream =
/// ToBus.BusTotalMiles + MilesThisLine`, rolled up into `FromBus.BusTotalMiles`.
/// With `units=mi length=1`, `MilesThisLine = 1` for each line.
#[test]
fn relcalc_miles_accumulators() {
    let mut dss = reliability_feeder();
    dss.command("Relcalc");

    // l2: ToBus b2 has 0 miles → accumulated = its own 1.0.
    assert!((accum_miles(&dss, "line.l2") - 1.0).abs() < 1e-12);
    // l1: ToBus b1 carries l2's 1.0 → accumulated = 1.0 + 1.0 = 2.0.
    assert!((accum_miles(&dss, "line.l1") - 2.0).abs() < 1e-12);

    // FROM-bus total miles roll up the same way.
    assert!((bus_total_miles(&dss, "src") - 2.0).abs() < 1e-12);
    assert!((bus_total_miles(&dss, "b1") - 1.0).abs() < 1e-12);
    // b2 is a TO-only end bus → never accumulated.
    assert!(bus_total_miles(&dss, "b2").abs() < 1e-12);
}

/// Junction roll-up: a single feeder (l1) splitting into two laterals
/// (l2→b2, l3→b3). The junction bus b1 and the metered branch l1 must
/// accumulate the failure rates and customers of *both* laterals.
#[test]
fn relcalc_branching_customer_rollup() {
    let mut dss = branching_reliability_feeder();
    dss.command("Relcalc");

    // Branch fault rates: l1=0.16, l2=0.27, l3=0.50.
    // b1 (junction) FROM-bus rate = l2 + l3 = 0.27 + 0.50 = 0.77.
    assert!((bus_flt_rate(&dss, "b1") - 0.77).abs() < 1e-12);
    // l1 accumulates b1's 0.77 plus its own 0.16 = 0.93; rolled to src.
    assert!((accum_flt_rate(&dss, "line.l1") - 0.93).abs() < 1e-12);
    assert!((bus_flt_rate(&dss, "src") - 0.93).abs() < 1e-12);

    // Customers: b1 totals both laterals' loads (25 + 7) → 32; src all 42.
    assert_eq!(bus_total_custs(&dss, "b1"), 32);
    assert_eq!(bus_total_custs(&dss, "src"), 42);
}

/// With no OCP device every zone bus and branch stays in section 0 (the
/// pre-first-OCP section), and the forward sweep never increments
/// `SectionCount`.
#[test]
fn relcalc_no_sections_without_ocp() {
    let mut dss = reliability_feeder();
    dss.command("Relcalc");

    for bus in ["src", "b1", "b2"] {
        assert_eq!(bus_section_id(&dss, bus), 0, "bus {bus} section");
    }
    for branch in ["line.l1", "line.l2"] {
        assert_eq!(
            branch_section_id(&dss, branch),
            0,
            "branch {branch} section"
        );
    }
}

/// The single positional `RelCalc` parameter is the `AssumeRestoration`
/// yes/no flag (Pascal `pMeter.AssumeRestoration := AssumeRestoration`). It
/// defaults FALSE and is stored on the meter for the customer roll-up.
#[test]
fn relcalc_assume_restoration_parsed() {
    let mut dss = reliability_feeder();
    // Default (no param) → FALSE.
    dss.command("Relcalc");
    assert!(!meter_assume_restoration(&dss, "m1"));

    // `Relcalc yes` → TRUE (still aborts: no OCP devices).
    dss.command("Relcalc yes");
    assert!(meter_assume_restoration(&dss, "m1"));
    assert!(
        dss.errors()
            .iter()
            .any(|e| e
                .contains("No Overcurrent Protection device (Relay, Recloser, or Fuse) defined")),
        "Relcalc yes must still abort without OCP devices, got {:?}",
        dss.errors()
    );
}

// ---- WP6.7: Sensor + load allocation -------------------------------------

/// A radial feeder with an EnergyMeter at the head and two ConnectedkVA-spec
/// loads. The meter's `SensorCurrent` defaults to 400 A, so `allocateloads`
/// scales the zone loads to push the metered current toward that peak.
fn allocation_feeder() -> Dss {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.a basekv=12.47 bus1=src phases=3");
    dss.command("new line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6");
    dss.command("new line.l2 bus1=b1 bus2=b2 length=1 r1=0.3 x1=0.6");
    dss.command("new load.ld1 bus1=b1 phases=3 kv=12.47 xfkva=500 allocationfactor=0.5 pf=0.9");
    dss.command("new load.ld2 bus1=b2 phases=3 kv=12.47 xfkva=800 allocationfactor=0.5 pf=0.9");
    dss.command("new energymeter.m1 element=line.l1 terminal=1");
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command("solve mode=snap");
    assert!(dss.errors().is_empty(), "build errors: {:?}", dss.errors());
    dss
}

fn close_rel(a: f64, e: f64) -> bool {
    (a - e).abs() <= 1e-3 + 1e-4 * e.abs()
}

/// `allocateloads` with the default `MaxAllocationIterations = 2`. Values
/// transcribed from the pinned oracle (`Loads.kW` / `AllocationFactor`).
#[test]
fn allocateloads_meter_drives_zone() {
    let mut dss = allocation_feeder();
    dss.command("allocateloads");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let (kw1, f1) = dss.load_alloc("ld1").unwrap();
    let (kw2, f2) = dss.load_alloc("ld2").unwrap();
    assert!(close_rel(kw1, 2867.625566), "ld1 kW {kw1}");
    assert!(close_rel(kw2, 4588.200906), "ld2 kW {kw2}");
    assert!(close_rel(f1, 6.372501), "ld1 factor {f1}");
    assert!(close_rel(f2, 6.372501), "ld2 factor {f2}");
}

/// `Set NumAllocIterations=4` runs two more allocation passes, converging
/// the loads slightly (oracle-pinned).
#[test]
fn allocateloads_honors_numallociterations() {
    let mut dss = allocation_feeder();
    dss.command("set numallociterations=4");
    dss.command("allocateloads");
    let (kw1, f1) = dss.load_alloc("ld1").unwrap();
    let (kw2, _) = dss.load_alloc("ld2").unwrap();
    assert!(close_rel(kw1, 2863.277886), "ld1 kW {kw1}");
    assert!(close_rel(kw2, 4581.244618), "ld2 kW {kw2}");
    assert!(close_rel(f1, 6.36284), "ld1 factor {f1}");
}

/// `Set AllocationFactors=X` sets every load's kVA allocation factor; for a
/// ConnectedkVA-spec load `kWbase = xfkVA · factor · |pf|`.
#[test]
fn set_allocation_factors_scales_all_loads() {
    let mut dss = allocation_feeder();
    dss.command("set allocationfactors=0.8");
    let (kw1, f1) = dss.load_alloc("ld1").unwrap();
    let (kw2, f2) = dss.load_alloc("ld2").unwrap();
    assert!((kw1 - 360.0).abs() < 1e-9, "ld1 {kw1}"); // 500·0.8·0.9
    assert!((kw2 - 576.0).abs() < 1e-9, "ld2 {kw2}"); // 800·0.8·0.9
    assert!((f1 - 0.8).abs() < 1e-12);
    assert!((f2 - 0.8).abs() < 1e-12);
    // A non-positive factor is rejected (Pascal error 271).
    dss.command("set allocationfactors=0");
    assert!(
        dss.errors()
            .iter()
            .any(|e| e.contains("Allocation Factor must be greater than zero")),
        "{:?}",
        dss.errors()
    );
}

/// A Sensor on the mid-feeder line (measured `currents` set in a *separate*
/// edit so they survive `RecalcElementData`'s `ZeroSensorArrays`) gives its
/// downstream load its own allocation target; the meter still drives the
/// upstream load. Oracle-pinned.
#[test]
fn allocateloads_with_sensor() {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.a basekv=12.47 bus1=src phases=3");
    dss.command("new line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6");
    dss.command("new line.l2 bus1=b1 bus2=b2 length=1 r1=0.3 x1=0.6");
    dss.command("new load.ld1 bus1=b1 phases=3 kv=12.47 xfkva=500 allocationfactor=0.5 pf=0.9");
    dss.command("new load.ld2 bus1=b2 phases=3 kv=12.47 xfkva=800 allocationfactor=0.5 pf=0.9");
    dss.command("new energymeter.m1 element=line.l1 terminal=1");
    dss.command("new sensor.s1 element=line.l2 terminal=1");
    dss.command("edit sensor.s1 currents=[20,20,20]");
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command("solve mode=snap");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss.command("allocateloads");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let (kw1, _) = dss.load_alloc("ld1").unwrap();
    let (kw2, _) = dss.load_alloc("ld2").unwrap();
    assert!(close_rel(kw1, 6780.125059), "ld1 kW {kw1}");
    assert!(close_rel(kw2, 382.584034), "ld2 kW {kw2}");
}

/// A bare Sensor (no `element=`) records the Pascal 666 error; defining the
/// element makes it valid and adopts the line's phase count.
#[test]
fn sensor_requires_element() {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.a basekv=12.47 bus1=src phases=3");
    dss.command("new line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6");
    dss.command("new sensor.s1 terminal=1 kvbase=12.47");
    assert!(
        dss.errors()
            .iter()
            .any(|e| e.contains("Circuit Element is not set")),
        "bare sensor must error, got {:?}",
        dss.errors()
    );
}

// The replay scenarios below (kWh-spec, single-phase per-phase, P/Q-sensor)
// are also covered by the data-driven gate `tests/golden_allocation.rs`;
// they are kept here as well for clearer per-case failure messages.

/// `allocateloads` over **kWh/Cfactor-spec** loads (`LoadSpec::KwhPf`): the
/// allocation factor feeds `Set_AllocationFactor`'s `c_factor` branch, not
/// `kva_allocation_factor`. Oracle-pinned `Loads.kW`/`AllocationFactor`.
#[test]
fn allocateloads_kwh_spec_loads() {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.a basekv=12.47 bus1=src phases=3");
    dss.command("new line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6");
    dss.command("new line.l2 bus1=b1 bus2=b2 length=1 r1=0.3 x1=0.6");
    dss.command("new load.ld1 bus1=b1 phases=3 kv=12.47 kwh=200000 cfactor=0.3 pf=0.9");
    dss.command("new load.ld2 bus1=b2 phases=3 kv=12.47 kwh=350000 cfactor=0.3 pf=0.9");
    dss.command("new energymeter.m1 element=line.l1 terminal=1");
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command("solve mode=snap");
    dss.command("allocateloads");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let (kw1, f1) = dss.load_alloc("ld1").unwrap();
    let (kw2, f2) = dss.load_alloc("ld2").unwrap();
    assert!(close_rel(kw1, 2710.478969), "ld1 kW {kw1}");
    assert!(close_rel(kw2, 4743.338196), "ld2 kW {kw2}");
    assert!(close_rel(f1, 9.757724), "ld1 cfactor {f1}");
    assert!(close_rel(f2, 9.757724), "ld2 cfactor {f2}");
}

/// `allocateloads` over **single-phase** loads on distinct phases: each load
/// is scaled by its connected phase's `PhsAllocationFactor[ConnectedPhase]`
/// (the meter is the sensor). Unbalanced xfkVA → distinct per-phase factors
/// (an off-by-one in the phase index would cross-wire them). Oracle-pinned.
#[test]
fn allocateloads_single_phase_per_phase_factor() {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.a basekv=12.47 bus1=src phases=3");
    dss.command("new line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6");
    dss.command("new load.ld1 bus1=b1.1 phases=1 kv=7.2 xfkva=200 allocationfactor=0.5 pf=0.9");
    dss.command("new load.ld2 bus1=b1.2 phases=1 kv=7.2 xfkva=400 allocationfactor=0.5 pf=0.9");
    dss.command("new load.ld3 bus1=b1.3 phases=1 kv=7.2 xfkva=600 allocationfactor=0.5 pf=0.9");
    dss.command("new energymeter.m1 element=line.l1 terminal=1");
    dss.command("set voltagebases=[12.47,7.2]");
    dss.command("calcvoltagebases");
    dss.command("solve mode=snap");
    dss.command("allocateloads");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let (kw1, f1) = dss.load_alloc("ld1").unwrap();
    let (kw2, f2) = dss.load_alloc("ld2").unwrap();
    let (kw3, f3) = dss.load_alloc("ld3").unwrap();
    // The factors are phase-distinct (13.9 / 6.95 / 4.63) — this is the part
    // that pins the connected-phase indexing.
    assert!(close_rel(f1, 13.902785), "ld1 factor {f1}");
    assert!(close_rel(f2, 6.951545), "ld2 factor {f2}");
    assert!(close_rel(f3, 4.634581), "ld3 factor {f3}");
    assert!(close_rel(kw1, 2502.501239), "ld1 kW {kw1}");
    assert!(close_rel(kw2, 2502.556163), "ld2 kW {kw2}");
    assert!(close_rel(kw3, 2502.673607), "ld3 kW {kw3}");
}

/// `allocateloads` driven by a **P/Q (kWs/kvars) Sensor**: the sensor's
/// `UpdateCurrentVector` converts |S|/Vbase to a per-phase current target
/// that then drives its downstream load, while the meter drives the
/// upstream load. Oracle-pinned.
#[test]
fn allocateloads_pq_sensor() {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.a basekv=12.47 bus1=src phases=3");
    dss.command("new line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6");
    dss.command("new line.l2 bus1=b1 bus2=b2 length=1 r1=0.3 x1=0.6");
    dss.command("new load.ld1 bus1=b1 phases=3 kv=12.47 xfkva=500 allocationfactor=0.5 pf=0.9");
    dss.command("new load.ld2 bus1=b2 phases=3 kv=12.47 xfkva=800 allocationfactor=0.5 pf=0.9");
    dss.command("new energymeter.m1 element=line.l1 terminal=1");
    dss.command("new sensor.s1 element=line.l2 terminal=1 kvbase=12.47");
    // P/Q in a separate edit so they survive RecalcElementData's zeroing.
    dss.command("edit sensor.s1 kWs=[400,400,400] kvars=[200,200,200]");
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command("solve mode=snap");
    dss.command("allocateloads");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let (kw1, _) = dss.load_alloc("ld1").unwrap();
    let (kw2, _) = dss.load_alloc("ld2").unwrap();
    assert!(close_rel(kw1, 5431.462098), "ld1 kW {kw1}");
    assert!(close_rel(kw2, 1179.744649), "ld2 kW {kw2}");
}

/// Feeder shared by the `TakeSample` tests: a Sensor on line `l2` term 1
/// (bus `b1`), one 3-phase load downstream, solved.
fn sample_feeder(conn: &str) -> Dss {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.a basekv=12.47 bus1=src phases=3");
    dss.command("new line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6");
    dss.command("new line.l2 bus1=b1 bus2=b2 length=1 r1=0.3 x1=0.6");
    dss.command("new load.ld2 bus1=b2 phases=3 kv=12.47 kw=1000 pf=0.9");
    dss.command(&format!(
        "new sensor.s1 element=line.l2 terminal=1 kvbase=12.47 conn={conn}"
    ));
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command("solve mode=snap");
    assert!(dss.errors().is_empty(), "build: {:?}", dss.errors());
    dss
}

fn cclose(a: num_complex::Complex64, re: f64, im: f64) -> bool {
    (a.re - re).abs() <= 1e-3 + 1e-4 * re.abs() && (a.im - im).abs() <= 1e-3 + 1e-4 * im.abs()
}

/// `TakeSample` (wye): `CalculatedCurrent` = the metered element's terminal-1
/// currents; `CalculatedVoltage` = the terminal node voltages. Oracle-pinned.
#[test]
fn sensor_take_sample_wye() {
    let mut dss = sample_feeder("wye");
    let (curr, volt) = dss.sensor_sample("s1").unwrap();
    assert!(cclose(curr[0], 46.530078, -22.890880), "I0 {:?}", curr[0]);
    assert!(cclose(curr[1], -43.089122, -28.850790), "I1 {:?}", curr[1]);
    assert!(cclose(curr[2], -3.440956, 51.741669), "I2 {:?}", curr[2]);
    assert!(cclose(volt[0], 7169.263686, -24.130402), "V0 {:?}", volt[0]);
    assert!(
        cclose(volt[1], -3605.529385, -6196.699277),
        "V1 {:?}",
        volt[1]
    );
    assert!(
        cclose(volt[2], -3563.734300, 6220.829680),
        "V2 {:?}",
        volt[2]
    );
}

/// `TakeSample` (delta): `CalculatedVoltage[i] = VTerminal[i] -
/// VTerminal[RotatePhases(i)]` (DeltaDirection +1 → L-L differences).
/// Oracle-pinned (computed from the same node voltages).
#[test]
fn sensor_take_sample_delta() {
    let mut dss = sample_feeder("delta");
    let (_curr, volt) = dss.sensor_sample("s1").unwrap();
    assert!(
        cclose(volt[0], 10774.793071, 6172.568875),
        "V0 {:?}",
        volt[0]
    );
    assert!(
        cclose(volt[1], -41.795084, -12417.528957),
        "V1 {:?}",
        volt[1]
    );
    assert!(
        cclose(volt[2], -10732.997986, 6244.960082),
        "V2 {:?}",
        volt[2]
    );
}
