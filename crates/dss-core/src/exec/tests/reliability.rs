use crate::elements::ckt::{CktElementData, ElemFlags};
use crate::elements::meter::energymeter::prop;
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

/// The two-section radial feeder of [`reliability_feeder`] with one OCP device
/// inserted (`ocp` = a full `New recloser/relay/fuse …` command). The control
/// monitors+switches its line, so the metered zone gets a real section.
fn ocp_feeder(ocp: &str) -> Dss {
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
    dss.command(ocp);
    dss.command("New energymeter.m1 element=line.l1 terminal=1");
    dss.command("Set voltagebases=[12.47]");
    dss.command("CalcVoltageBases");
    dss.command("Solve mode=snap");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss
}

/// Read an EnergyMeter reliability index by its 1-based property ordinal.
fn meter_f64(dss: &Dss, name: &str, prop: usize) -> f64 {
    for class in &dss.classes {
        if !class.props.class_name().eq_ignore_ascii_case("energymeter") {
            continue;
        }
        for obj in class.arena.objs() {
            if obj.data().name().eq_ignore_ascii_case(name) {
                return obj.get_f64(prop);
            }
        }
    }
    panic!("meter {name} not found");
}

/// The shared circuit-element data of `full` (`class.name`), for flag asserts.
fn elem_cd<'a>(dss: &'a Dss, full: &str) -> &'a CktElementData {
    let (cls, name) = full.split_once('.').unwrap();
    for class in &dss.classes {
        if !class.props.class_name().eq_ignore_ascii_case(cls) {
            continue;
        }
        for i in 0..class.arena.len() {
            if class.arena.obj(i).data().name().eq_ignore_ascii_case(name)
                && let Some(e) = class.arena.try_ckt_elem(i)
            {
                return e.cd();
            }
        }
    }
    panic!("element {full} not found");
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
        for i in 0..class.arena.len() {
            if class.arena.obj(i).data().name().eq_ignore_ascii_case(name)
                && let Some(e) = class.arena.try_ckt_elem(i)
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
        for i in 0..class.arena.len() {
            if class.arena.obj(i).data().name().eq_ignore_ascii_case(name)
                && let Some(e) = class.arena.try_ckt_elem(i)
            {
                return e.cd().branch_section_id;
            }
        }
    }
    panic!("element {full} not found");
}

fn meter_assume_restoration(dss: &Dss, name: &str) -> bool {
    for class in &dss.classes {
        for em in class
            .arena
            .all::<crate::elements::meter::energymeter::EnergyMeter>()
            .unwrap_or(&[])
        {
            if em.data().name().eq_ignore_ascii_case(name) {
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
        for i in 0..class.arena.len() {
            if class.arena.obj(i).data().name().eq_ignore_ascii_case(name)
                && let Some(e) = class.arena.try_ckt_elem(i)
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

// --- WP7.2 step 3: OCP-device reliability activation ------------------

/// A Relay/Recloser/Fuse marks its controlled element with `Flg.HasOCPDevice`
/// (Pascal `RecalcElementData`), and the `GetOCPDeviceType` ordinal lands on
/// the element (1=Fuse, 2=Recloser, 3=Relay). The auto-reclosing Relay/Recloser
/// also set `HasAutoOCPDevice`; the Fuse never does.
#[test]
fn ocp_device_flags_and_type_per_class() {
    for (ocp, dev_type, auto) in [
        (
            "New recloser.r1 monitoredobj=line.l1 monitoredterm=1 \
                 switchedobj=line.l1 switchedterm=1",
            2,
            true,
        ),
        (
            "New relay.r1 type=current monitoredobj=line.l1 monitoredterm=1 \
                 switchedobj=line.l1 switchedterm=1 phasetrip=1 delay=0.1",
            3,
            true,
        ),
        (
            "New fuse.f1 monitoredobj=line.l1 monitoredterm=1 \
                 switchedobj=line.l1 switchedterm=1",
            1,
            false,
        ),
    ] {
        let dss = ocp_feeder(ocp);
        let cd = elem_cd(&dss, "line.l1");
        assert!(
            cd.flags.contains(ElemFlags::HAS_OCP_DEVICE),
            "HasOCPDevice for {ocp}"
        );
        assert_eq!(
            cd.flags.contains(ElemFlags::HAS_AUTO_OCP_DEVICE),
            auto,
            "HasAutoOCPDevice for {ocp}"
        );
        assert_eq!(cd.ocp_device_type, dev_type, "GetOCPDeviceType for {ocp}");
    }
}

/// A disabled OCP control sets no flag (Pascal `if Enabled then Include(...)`),
/// so the zone stays section-less and `RelCalc` aborts exactly as with no
/// device at all.
#[test]
fn disabled_ocp_device_sets_no_flag() {
    let mut dss = ocp_feeder(
        "New recloser.r1 monitoredobj=line.l1 monitoredterm=1 \
             switchedobj=line.l1 switchedterm=1 enabled=no",
    );
    let cd = elem_cd(&dss, "line.l1");
    assert!(!cd.flags.contains(ElemFlags::HAS_OCP_DEVICE));
    assert_eq!(cd.ocp_device_type, 0);

    // The promised consequence: no section ⇒ RelCalc aborts (#52902).
    dss.command("Relcalc");
    assert!(
        dss.errors().iter().any(|e| e.contains("Aborting")),
        "a disabled OCP device must still abort RelCalc, got {:?}",
        dss.errors()
    );
}

/// **Documented WP7.2-step-3 deferral.** The port ports Pascal's
/// `if Enabled then Include(Flg.HasOCPDevice)` but **not** the unconditional
/// `Exclude(PreviousControlledElement.Flags, …)` that precedes it. So disabling
/// a control after it was enabled leaves the controlled element's flag *stale*
/// (Pascal would clear it) — the same limitation the existing
/// `SetSwitchClosed`/`SetConductorsClosed` forces carry (they never un-force a
/// prior target). Pinned here so the gap is explicit, not silent; a future
/// faithful fix flips this assertion deliberately.
#[test]
fn enable_then_disable_leaves_ocp_flag_stale() {
    let mut dss = ocp_feeder(
        "New recloser.r1 monitoredobj=line.l1 monitoredterm=1 \
             switchedobj=line.l1 switchedterm=1",
    );
    assert!(
        elem_cd(&dss, "line.l1")
            .flags
            .contains(ElemFlags::HAS_OCP_DEVICE)
    );
    dss.command("edit recloser.r1 enabled=no");
    // Stale — Pascal clears it; the port does not (documented deferral).
    assert!(
        elem_cd(&dss, "line.l1")
            .flags
            .contains(ElemFlags::HAS_OCP_DEVICE)
    );
}

/// Two OCP controls switching the same line: `GetOCPDeviceType` reports the
/// **first** one defined (Pascal scans `ControlElementList` and stops at the
/// first Fuse/Recloser/Relay). Oracle `Meters.OCPDeviceType`: fuse-first ⇒ 1,
/// recloser-first ⇒ 2 (`tools/golden/probe_reliability.py`). The recloser
/// always contributes `HasAutoOCPDevice` regardless of order.
#[test]
fn ocp_device_type_first_registered_wins() {
    let feeder = |first: &str, second: &str| -> Dss {
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
        dss.command(first);
        dss.command(second);
        dss.command("New energymeter.m1 element=line.l1 terminal=1");
        dss.command("Set voltagebases=[12.47]");
        dss.command("CalcVoltageBases");
        dss.command("Solve mode=snap");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        dss
    };
    // High recloser pickups so the snapshot solve never trips the device.
    let fuse = "New fuse.f1 monitoredobj=line.l1 monitoredterm=1 \
                switchedobj=line.l1 switchedterm=1";
    let rec = "New recloser.r1 monitoredobj=line.l1 monitoredterm=1 \
               switchedobj=line.l1 switchedterm=1 phasetrip=100000 groundtrip=100000";

    let fuse_first = feeder(fuse, rec);
    let cd = elem_cd(&fuse_first, "line.l1");
    assert_eq!(cd.ocp_device_type, 1, "fuse defined first wins");
    assert!(cd.flags.contains(ElemFlags::HAS_OCP_DEVICE));
    assert!(
        cd.flags.contains(ElemFlags::HAS_AUTO_OCP_DEVICE),
        "the recloser still contributes HasAutoOCPDevice"
    );

    let rec_first = feeder(rec, fuse);
    assert_eq!(
        elem_cd(&rec_first, "line.l1").ocp_device_type,
        2,
        "recloser defined first wins"
    );
}

/// `AssumeRestoration` changes the downstream auto-OCP interruption count
/// (Pascal forward sweep: `if AssumeRestoration and HasAutoOCPDevice then
/// Bus_Num_Interrupt := AccumulatedBrFltRate` *resets* instead of accumulating).
/// A 3-section feeder with auto-reclosers on the head (l1) and a downstream line
/// (l3) makes No vs Yes diverge in SAIFI/SAIFIkW/CustInterrupts; SAIDI is
/// restoration-independent (section fault-rate × repair × customers). High
/// pickups keep the snapshot solve from tripping. Oracle: dss-python 0.15.7
/// (`tools/golden/probe_reliability.py`).
#[test]
fn relcalc_assume_restoration_changes_auto_ocp_interruptions() {
    let build = |dss: &mut Dss| {
        dss.command("New circuit.t basekv=12.47 bus1=src phases=3");
        dss.command(
            "New line.l1 bus1=src bus2=b1 length=1 units=mi r1=0.1 x1=0.1 \
                 faultrate=0.2 pctperm=80 repair=4",
        );
        dss.command(
            "New line.l2 bus1=b1 bus2=b2 length=1 units=mi r1=0.1 x1=0.1 \
                 faultrate=0.3 pctperm=90 repair=5",
        );
        dss.command(
            "New line.l3 bus1=b2 bus2=b3 length=1 units=mi r1=0.1 x1=0.1 \
                 faultrate=0.5 pctperm=100 repair=6",
        );
        dss.command("New load.ld1 bus1=b1 phases=3 kv=12.47 kw=100 numcust=10");
        dss.command("New load.ld2 bus1=b2 phases=3 kv=12.47 kw=200 numcust=25");
        dss.command("New load.ld3 bus1=b3 phases=3 kv=12.47 kw=150 numcust=7");
        dss.command(
            "New recloser.r1 monitoredobj=line.l1 monitoredterm=1 \
                 switchedobj=line.l1 switchedterm=1 phasetrip=100000 groundtrip=100000",
        );
        dss.command(
            "New recloser.r2 monitoredobj=line.l3 monitoredterm=1 \
                 switchedobj=line.l3 switchedterm=1 phasetrip=100000 groundtrip=100000",
        );
        dss.command("New energymeter.m1 element=line.l1 terminal=1");
        dss.command("Set voltagebases=[12.47]");
        dss.command("CalcVoltageBases");
        dss.command("Solve mode=snap");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    };
    let eps = 1e-9;

    let mut no = Dss::new();
    build(&mut no);
    no.command("Relcalc no");
    assert!((meter_f64(&no, "m1", prop::SAIFI) - 0.513_333_333_333_333_4).abs() < eps);
    assert!((meter_f64(&no, "m1", prop::SAIDI) - 2.49).abs() < eps);
    assert!((meter_f64(&no, "m1", prop::SAIFI_KW) - 0.596_666_666_666_666_7).abs() < eps);
    assert!((meter_f64(&no, "m1", prop::CUST_INTERRUPTS) - 21.56).abs() < eps);

    let mut yes = Dss::new();
    build(&mut yes);
    yes.command("Relcalc yes");
    // Restoration resets the downstream section count → lower SAIFI/CustInt.
    assert!((meter_f64(&yes, "m1", prop::SAIFI) - 0.441_666_666_666_666_76).abs() < eps);
    assert!((meter_f64(&yes, "m1", prop::SAIDI) - 2.49).abs() < eps);
    assert!((meter_f64(&yes, "m1", prop::SAIFI_KW) - 0.453_333_333_333_333_4).abs() < eps);
    assert!((meter_f64(&yes, "m1", prop::CUST_INTERRUPTS) - 18.55).abs() < eps);

    // Both reclosers head a section; l3 reports recloser device type.
    assert_eq!(elem_cd(&no, "line.l3").ocp_device_type, 2);
}

/// With an OCP device at the metered head line, `RelCalc` no longer aborts and
/// produces SAIFI/SAIDI/SAIFIkW/CustInterrupts matching the pinned oracle
/// (dss-python 0.15.7; see `tools/golden/probe_reliability.py`). The single
/// section covers both branches → 35 customers; CAIDI = SAIDI / SAIFI.
#[test]
fn relcalc_head_recloser_matches_oracle() {
    let mut dss = ocp_feeder(
        "New recloser.r1 monitoredobj=line.l1 monitoredterm=1 \
             switchedobj=line.l1 switchedterm=1",
    );
    dss.command("Relcalc");
    assert!(
        !dss.errors().iter().any(|e| e.contains("Aborting")),
        "RelCalc must not abort with an OCP device, got {:?}",
        dss.errors()
    );

    let eps = 1e-9;
    assert!((meter_f64(&dss, "m1", prop::SAIFI) - 0.43).abs() < eps);
    assert!((meter_f64(&dss, "m1", prop::SAIDI) - 1.99).abs() < eps);
    assert!((meter_f64(&dss, "m1", prop::SAIFI_KW) - 0.43).abs() < eps);
    assert!((meter_f64(&dss, "m1", prop::CUST_INTERRUPTS) - 15.05).abs() < eps);
    assert!((meter_f64(&dss, "m1", prop::CAIDI) - 1.99 / 0.43).abs() < eps);

    // The whole metered zone is one section, headed by l1.
    assert_eq!(branch_section_id(&dss, "line.l1"), 1);
    assert_eq!(branch_section_id(&dss, "line.l2"), 1);
}

/// An OCP device on the *downstream* line `l2` covers only its 25 customers,
/// so the indices drop (oracle: SAIFI 0.19285714…, SAIDI 0.96428571…,
/// SAIFIkW 0.18, CustInterrupts 6.75; CAIDI = 5.0).
#[test]
fn relcalc_downstream_recloser_matches_oracle() {
    let mut dss = ocp_feeder(
        "New recloser.r1 monitoredobj=line.l2 monitoredterm=1 \
             switchedobj=line.l2 switchedterm=1",
    );
    dss.command("Relcalc");
    assert!(
        !dss.errors().iter().any(|e| e.contains("Aborting")),
        "RelCalc must not abort, got {:?}",
        dss.errors()
    );

    let eps = 1e-9;
    assert!((meter_f64(&dss, "m1", prop::SAIFI) - 0.192_857_142_857_142_87).abs() < eps);
    assert!((meter_f64(&dss, "m1", prop::SAIDI) - 0.964_285_714_285_714_3).abs() < eps);
    assert!((meter_f64(&dss, "m1", prop::SAIFI_KW) - 0.18).abs() < eps);
    assert!((meter_f64(&dss, "m1", prop::CUST_INTERRUPTS) - 6.75).abs() < eps);
    assert!((meter_f64(&dss, "m1", prop::CAIDI) - 5.0).abs() < eps);

    // Only l2 heads a section; l1 (upstream of the OCP) stays in section 0.
    assert_eq!(branch_section_id(&dss, "line.l1"), 0);
    assert_eq!(branch_section_id(&dss, "line.l2"), 1);
    assert_eq!(elem_cd(&dss, "line.l2").ocp_device_type, 2);
}
