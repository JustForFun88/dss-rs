use crate::exec::*;

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
