use crate::exec::*;

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
