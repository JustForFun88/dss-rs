use crate::exec::*;

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
        for obj in class.arena.objs() {
            if obj.data().name().eq_ignore_ascii_case(name)
                && let Some(e) = obj.as_ckt_element()
            {
                return (e.cd().branch_num_customers, e.cd().branch_total_customers);
            }
        }
    }
    panic!("element {full} not found");
}
