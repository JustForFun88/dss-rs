use crate::elements::ckt::{CktElementData, ElemFlags, OcpDeviceType};
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
            OcpDeviceType::Recloser,
            true,
        ),
        (
            "New relay.r1 type=current monitoredobj=line.l1 monitoredterm=1 \
                 switchedobj=line.l1 switchedterm=1 phasetrip=1 delay=0.1",
            OcpDeviceType::Relay,
            true,
        ),
        (
            "New fuse.f1 monitoredobj=line.l1 monitoredterm=1 \
                 switchedobj=line.l1 switchedterm=1",
            OcpDeviceType::Fuse,
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
    assert_eq!(cd.ocp_device_type, OcpDeviceType::Unset);

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
    assert_eq!(
        cd.ocp_device_type,
        OcpDeviceType::Fuse,
        "fuse defined first wins"
    );
    assert!(cd.flags.contains(ElemFlags::HAS_OCP_DEVICE));
    assert!(
        cd.flags.contains(ElemFlags::HAS_AUTO_OCP_DEVICE),
        "the recloser still contributes HasAutoOCPDevice"
    );

    let rec_first = feeder(rec, fuse);
    assert_eq!(
        elem_cd(&rec_first, "line.l1").ocp_device_type,
        OcpDeviceType::Recloser,
        "recloser defined first wins"
    );
}

/// `AssumeRestoration` changes the downstream auto-OCP interruption count
/// (Pascal forward sweep: `if AssumeRestoration and HasAutoOCPDevice then
/// Bus_Num_Interrupt := AccumulatedBrFltRate` *resets* instead of accumulating)
/// **and** — since `CalcReliabilityIndices` re-runs `TotalUpDownstreamCustomers`
/// under the flag (r4133 `Version8/Source/Meters/EnergyMeter.pas:2467-2468`) —
/// the customer totals every SAIDI term is weighted by. A 3-section feeder with
/// auto-reclosers on the head (l1) and a downstream line (l3) makes No vs Yes
/// diverge in SAIFI/SAIFIkW/CustInterrupts/SAIDI. High pickups keep the snapshot
/// solve from tripping.
///
/// **Both numbers, `RelCalc yes` (measured 2026-09-05,
/// `tmp/g16ii/probe_f7_r4133.py` / `probe_f7_capi.py`).** The port follows r4133
/// — r3723, r4088 and r4133 all carry the call — while dss_capi 0.14.5 dropped
/// it (`src/Meters/EnergyMeter.pas:2411-2432` goes straight from the zone check
/// to the zeroing loop) and therefore weights the sections with the totals the
/// last zone build left behind:
///
/// | quantity | port == r4133 | dss_capi 0.14.5 |
/// |---|---|---|
/// | `SAIDI` | `2.1583333333333337` | `2.4899999999999998` |
/// | `Bus.N_Customers` `src` | `35` | `42` |
/// | `Bus.N_Customers` `b1` | `25` | `32` |
/// | `Bus.Cust_Interrupts` `b1` | `10.750000000000002` | `13.760000000000002` |
/// | `Bus.Cust_Duration` `b1` | `69.64999999999999` | `83.58` |
/// | section 1 `SectTotalCust` | `35` | `42` |
///
/// `RelCalc no` is bit-identical on all three engines (the flag the zone build
/// used and the flag `RelCalc` passes agree, so the re-run is idempotent), which
/// is why the whole corpus population — which never passes the flag — is
/// untouched by the restoration.
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
    assert!((meter_f64(&no, "m1", prop::SAIDI) - 2.489_999_999_999_999_8).abs() < eps);
    assert!((meter_f64(&no, "m1", prop::SAIFI_KW) - 0.596_666_666_666_666_7).abs() < eps);
    assert!((meter_f64(&no, "m1", prop::CUST_INTERRUPTS) - 21.560_000_000_000_002).abs() < eps);
    // The totals the zone build left (`AssumeRestoration` false there too), so
    // the re-run under `no` reproduces them exactly: all three engines agree.
    assert_eq!(bus_total_custs(&no, "src"), 42);
    assert_eq!(bus_total_custs(&no, "b1"), 32);
    assert!((bus_f64(&no, "b1", |b| b.bus_cust_interrupts) - 13.760_000_000_000_002).abs() < eps);
    assert!((bus_f64(&no, "b1", |b| b.bus_cust_durations) - 83.58).abs() < eps);
    assert_eq!(section_total_cust(&no, "m1", 1), 42);

    let mut yes = Dss::new();
    build(&mut yes);
    yes.command("Relcalc yes");
    // Restoration resets the downstream section count → lower SAIFI/CustInt.
    // These three come from the loads\' own `numcust`/`kW` and the bus
    // interruption counts, so they never depended on the customer roll-up: all
    // three engines agree here too.
    assert!((meter_f64(&yes, "m1", prop::SAIFI) - 0.441_666_666_666_666_76).abs() < eps);
    assert!((meter_f64(&yes, "m1", prop::SAIFI_KW) - 0.453_333_333_333_333_4).abs() < eps);
    assert!((meter_f64(&yes, "m1", prop::CUST_INTERRUPTS) - 18.550_000_000_000_004).abs() < eps);
    // The re-run half: r4133 (and r3723/r4088) recompute the totals under the
    // restoration flag, so the head section loses everything below the
    // downstream recloser. dss_capi 0.14.5 reports the stale `42`/`32`/`2.49`
    // of the `no` run above — the divergence this pin names on both sides.
    assert!((meter_f64(&yes, "m1", prop::SAIDI) - 2.158_333_333_333_333_7).abs() < eps);
    assert_eq!(bus_total_custs(&yes, "src"), 35, "capi 0.14.5 reports 42");
    assert_eq!(bus_total_custs(&yes, "b1"), 25, "capi 0.14.5 reports 32");
    assert!((bus_f64(&yes, "b1", |b| b.bus_cust_interrupts) - 10.750_000_000_000_002).abs() < eps);
    assert!((bus_f64(&yes, "b1", |b| b.bus_cust_durations) - 69.649_999_999_999_99).abs() < eps);
    assert_eq!(
        section_total_cust(&yes, "m1", 1),
        35,
        "capi 0.14.5 reports 42"
    );
    // b2/b3 sit below the downstream recloser and keep their own totals on
    // every engine — the restoration only cuts the roll-up *upward*.
    assert_eq!(bus_total_custs(&no, "b2"), 7);
    assert_eq!(bus_total_custs(&yes, "b2"), 7);
    assert_eq!(section_total_cust(&no, "m1", 2), 7);
    assert_eq!(section_total_cust(&yes, "m1", 2), 7);

    // Both reclosers head a section; l3 reports recloser device type.
    assert_eq!(
        elem_cd(&no, "line.l3").ocp_device_type,
        OcpDeviceType::Recloser
    );
}

/// `SectTotalCust` of the 1-based feeder section `idx` of meter `name`.
fn section_total_cust(dss: &Dss, name: &str, idx: i32) -> i32 {
    let m = dss
        .meter_reliability()
        .into_iter()
        .find(|m| m.name.eq_ignore_ascii_case(name))
        .expect("meter not found");
    m.sections
        .iter()
        .find(|s| s.idx == idx)
        .unwrap_or_else(|| panic!("meter {name} has no section {idx}"))
        .sect_total_cust
}

/// The gate\'s own reliability deck, compiled from the vendored corpus tree
/// (never `.inputs/`). It writes no file — no `export`/`show`/`save`, and its
/// trailing `AllocateLoads` only solves — so no directory guard is needed (the
/// `exec::tests::controls::compile_corpus_deck` precedent).
fn compile_midi_relcalc() -> Dss {
    let deck = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/controls/energymeter/midi_relcalc.dss");
    assert!(deck.is_file(), "vendored corpus deck missing: {deck:?}");
    let mut dss = Dss::new();
    dss.command(&format!("compile \"{}\"", deck.display()));
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss
}

/// **The pin that makes the restored `TotalUpDownstreamCustomers` call
/// observable** (r4133 `Version8/Source/Meters/EnergyMeter.pas:2467-2468`, in
/// `CalcReliabilityIndices` between the `SequenceList` check and the zeroing
/// loop; port `solution/meters/reliability.rs::calc_reliability_indices`).
///
/// Deck: `tests/corpus/controls/energymeter/midi_relcalc.dss` — the gate\'s own
/// three-section reliability case, whose three Reclosers all set
/// `HasAutoOCPDevice`, so the restoration flag really cuts the roll-up.
///
/// Two halves, both measured 2026-09-05 against the two gating oracles
/// (`tmp/g16ii/probe_f7_relcalc_deck.py`, raw JSON `tmp/g16ii/f7_relcalc_deck.json`):
///
/// 1. **`RelCalc` with no flag — the corpus path — is bit-identical on capi
///    0.14.5 and r4133**, and the re-run changes nothing, because the zone build
///    already rolled the customers up with `AssumeRestoration = false`. That is
///    the zero-footprint half: every gate case drives `RelCalc` without a flag.
/// 2. **`RelCalc restore=y` separates the engines**: the port and r4133 recompute
///    (`src.N_Customers = 0`, section 1 `SectTotalCust = 0`,
///    `SAIDI = 0.27623590504451034`), dss_capi 0.14.5 keeps the build-time totals
///    (`30`, `30`, `SAIDI = 0.5496409495548961`). Everything else — SAIFI
///    `0.11267537091988129`, SAIFIkW `0.11966863270777478`, CustInterrupts
///    `3.79716` and every other bus column — agrees on all three engines.
#[test]
fn relcalc_recomputes_the_customer_totals_it_depends_on() {
    let eps = 1e-12;

    // (1) The corpus path: no flag, so the re-run is idempotent and all three
    // engines return the same numbers.
    let mut base = compile_midi_relcalc();
    base.command("RelCalc");
    assert!(base.errors().is_empty(), "{:?}", base.errors());
    assert!((meter_f64(&base, "em", prop::SAIFI) - 0.180_925_370_919_881_3).abs() < eps);
    assert!((meter_f64(&base, "em", prop::SAIDI) - 0.549_640_949_554_896_1).abs() < eps);
    assert!((meter_f64(&base, "em", prop::SAIFI_KW) - 0.187_918_632_707_774_8).abs() < eps);
    assert!((meter_f64(&base, "em", prop::CUST_INTERRUPTS) - 6.097_185_000_000_000_5).abs() < eps);
    for (bus, n) in [("src", 30), ("mid", 30), ("la", 0), ("lb", 4), ("lc", 0)] {
        assert_eq!(bus_total_custs(&base, bus), n, "no-flag N_Customers {bus}");
    }
    for (idx, n) in [(1, 30), (2, 13), (3, 17)] {
        assert_eq!(
            section_total_cust(&base, "em", idx),
            n,
            "no-flag section {idx}"
        );
    }

    // (2) `restore=y`: only the recomputed totals and the SAIDI they weight move.
    let mut restored = compile_midi_relcalc();
    restored.command("RelCalc restore=y");
    assert!(restored.errors().is_empty(), "{:?}", restored.errors());
    assert_eq!(
        bus_total_custs(&restored, "src"),
        0,
        "r4133 recomputes to 0; dss_capi 0.14.5 keeps 30"
    );
    assert_eq!(
        section_total_cust(&restored, "em", 1),
        0,
        "r4133 recomputes to 0; dss_capi 0.14.5 keeps 30"
    );
    assert!(
        (meter_f64(&restored, "em", prop::SAIDI) - 0.276_235_905_044_510_34).abs() < eps,
        "r4133 0.27623590504451034; dss_capi 0.14.5 0.5496409495548961, got {}",
        meter_f64(&restored, "em", prop::SAIDI)
    );
    // The quantities the roll-up never fed stay put on all three engines.
    assert!((meter_f64(&restored, "em", prop::SAIFI) - 0.112_675_370_919_881_29).abs() < eps);
    assert!((meter_f64(&restored, "em", prop::SAIFI_KW) - 0.119_668_632_707_774_78).abs() < eps);
    assert!((meter_f64(&restored, "em", prop::CUST_INTERRUPTS) - 3.797_16).abs() < eps);
    for (bus, n) in [("mid", 30), ("la", 0), ("lb", 4), ("lc", 0)] {
        assert_eq!(
            bus_total_custs(&restored, bus),
            n,
            "restore N_Customers {bus}"
        );
    }
    for (idx, n) in [(2, 13), (3, 17)] {
        assert_eq!(
            section_total_cust(&restored, "em", idx),
            n,
            "restore section {idx}"
        );
    }

    // Without the restored call the two runs would be identical: that they are
    // not is what proves the call is live rather than dead code.
    assert_ne!(
        bus_total_custs(&base, "src"),
        bus_total_custs(&restored, "src"),
        "the restored TotalUpDownstreamCustomers must change the totals under restore=y"
    );
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
    assert_eq!(
        elem_cd(&dss, "line.l2").ocp_device_type,
        OcpDeviceType::Recloser
    );
}

// --- GOLDEN_REBASE G1.6(i): the `Meters` reliability read surface -------
//
// `Dss::meter_reliability` / `Dss::meter_totals` (`exec/view.rs`) are pure
// reads of the state `CalcReliabilityIndices` leaves behind. The three pins
// below cover the parts of that surface no oracle capture can reach: the
// `Totals` arithmetic, the run-once protocol the live gate depends on, and the
// unguarded division the comparator's NaN rule is written for.

/// A head meter plus a nested sub-meter: `m1` on the head line `l1`, `m2` on
/// the third line `l3`, so `m1`'s zone stops at `l3` and `b2` is the boundary
/// bus (the TO bus of `m1`'s last branch and the FROM bus of `m2`'s head).
/// This is the `controls:energymeter/midi_energymeter.dss` shape in miniature
/// (`energymeter.em` on `transformer.sub`, `energymeter.em2` on
/// `line.rb_bb11`). `units=mi length=1` makes `MilesThisLine` exactly 1 per
/// line, so the miles arithmetic is readable by hand.
fn nested_meter_feeder() -> Dss {
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
    dss.command(
        "New line.l3 bus1=b2 bus2=b3 length=1 units=mi r1=0.1 x1=0.1 \
             faultrate=0.4 pctperm=70 repair=6",
    );
    dss.command("New load.ld1 bus1=b1 phases=3 kv=12.47 kw=100 numcust=10");
    dss.command("New load.ld2 bus1=b2 phases=3 kv=12.47 kw=200 numcust=25");
    dss.command("New load.ld3 bus1=b3 phases=3 kv=12.47 kw=150 numcust=7");
    dss.command("New energymeter.m1 element=line.l1 terminal=1");
    dss.command("New energymeter.m2 element=line.l3 terminal=1");
    dss.command("Set voltagebases=[12.47]");
    dss.command("CalcVoltageBases");
    dss.command("Solve mode=snap");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss
}

/// [`nested_meter_feeder`] with the meters declared in the OPPOSITE order —
/// the INNER meter `m2` first, so `EnergyMeters` walks it before the outer
/// `m1`. Same circuit, same zones; only the creation order differs, which is
/// the order both oracles' `DoLambdaCalcs` loop follows (r4133
/// `Executive/ExecHelper.pas:4438-4441`).
fn nested_meter_feeder_inner_first() -> Dss {
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
    dss.command(
        "New line.l3 bus1=b2 bus2=b3 length=1 units=mi r1=0.1 x1=0.1 \
             faultrate=0.4 pctperm=70 repair=6",
    );
    dss.command("New load.ld1 bus1=b1 phases=3 kv=12.47 kw=100 numcust=10");
    dss.command("New load.ld2 bus1=b2 phases=3 kv=12.47 kw=200 numcust=25");
    dss.command("New load.ld3 bus1=b3 phases=3 kv=12.47 kw=150 numcust=7");
    dss.command("New energymeter.m2 element=line.l3 terminal=1");
    dss.command("New energymeter.m1 element=line.l1 terminal=1");
    dss.command("Set voltagebases=[12.47]");
    dss.command("CalcVoltageBases");
    dss.command("Solve mode=snap");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss
}

/// The named meter's raw registers and `TotalsMask`.
fn meter_regs_and_mask(dss: &Dss, name: &str) -> (Vec<f64>, Vec<f64>) {
    for class in &dss.classes {
        for em in class
            .arena
            .all::<crate::elements::meter::energymeter::EnergyMeter>()
            .unwrap_or(&[])
        {
            if em.data().name().eq_ignore_ascii_case(name) {
                return (em.registers().to_vec(), em.totals_mask().to_vec());
            }
        }
    }
    panic!("meter {name} not found");
}

/// `Meters.Totals` is `Σ_meters Registers[i] · TotalsMask[i]` over **every**
/// meter of the circuit, in the `EnergyMeters` list's creation order — Pascal
/// `TDSSCircuit.TotalizeMeters` (r4133
/// `Version8/Source/Common/Circuit.pas:2520-2538`), the body behind capi
/// `Meters_Get_Totals` (`CAPI/CAPI_Meters.pas:279-290`) and r4133 `MetersV(3)`
/// (`DDLL/DMeters.pas:558-573`). Length is `NumEMRegisters` = 67, which is what
/// both oracles return (measured on the live decks, `tmp/g16i` probes).
///
/// The fixture gives `m2` a non-unit mask slot 0 (`Mask=[0.25]`, the rest
/// defaulting to 1.0 exactly as Pascal fills them), so the masked slot differs
/// from the plain register sum and a dropped multiply cannot pass.
#[test]
fn meter_totals_is_the_masked_register_sum() {
    let mut dss = nested_meter_feeder();
    // Registers only integrate over a time-series solve; the snapshot the
    // fixture ends on leaves them all zero.
    dss.command("Solve mode=daily stepsize=1h number=2");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss.command("Edit energymeter.m2 mask=[0.25]");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    let (r1, w1) = meter_regs_and_mask(&dss, "m1");
    let (r2, w2) = meter_regs_and_mask(&dss, "m2");
    assert_eq!(w2[0], 0.25, "mask slot 0 parsed");
    assert_eq!(
        w2[1], 1.0,
        "mask slots past the supplied values default to 1"
    );

    let totals = dss.meter_totals();
    assert_eq!(
        totals.len(),
        crate::elements::meter::energymeter::NUM_EM_REGISTERS
    );
    assert_eq!(totals.len(), 67, "NumEMRegisters = 32 + 5*7");

    for i in 0..totals.len() {
        let want = r1[i] * w1[i] + r2[i] * w2[i];
        assert_eq!(totals[i], want, "Totals[{i}]");
    }

    // Non-vacuity: the register the mask scales is really non-zero, and the
    // masked total is really below the unmasked sum.
    assert!(
        r2[0] > 0.0,
        "meter m2 register 0 (kWh) is non-zero: {}",
        r2[0]
    );
    assert!(
        totals[0] < r1[0] + r2[0],
        "masked total {} must be below the unmasked sum {}",
        totals[0],
        r1[0] + r2[0]
    );
}

/// **`RelCalc` is not idempotent across meters, so the live gate runs it
/// exactly once, after the last solve** (GOLDEN_REBASE G1.6(i) decision
/// D-i-1).
///
/// `TPDElement.ZeroReliabilityAccums` (r4133
/// `Version8/Source/PDElements/PDElement.pas:313-328`) zeroes only the FROM
/// bus of each element of the meter's **own** `SequenceList` (r4133
/// `Meters/EnergyMeter.pas:2471-2472`), while `AccumFltRate` (`PDElement.pas:
/// 93-120`, port `solution/meters/reliability.rs:142-152`) reads
/// `ToBus.BusTotalMiles`. On a nested pair of meters the boundary bus is the
/// TO bus of the outer zone and the FROM bus of the inner one, so the inner
/// meter's miles from run *n* are read back as the outer meter's downstream
/// miles in run *n+1*: a second `RelCalc` shifts every bus and PD element of
/// the outer zone up by the inner zone's mileage. Runs beyond the second are a
/// fixpoint — the inner zone re-derives the same mileage each time — so the
/// damage is *one* wrong answer, not a runaway, which is exactly why it can
/// only be caught by fixing the protocol rather than by a magnitude check.
///
/// Measured on the pinned capi oracle over
/// `controls:energymeter/midi_energymeter.dss` (`em` on `transformer.sub`,
/// `em2` on `line.rb_bb11`): a second `RelCalc` moves 34 values, the head bus's
/// `TotalMiles` going `13.825757575757578 → 22.348484848484844` (+8.522727…,
/// exactly `em2`'s zone mileage) and `PDElements.TotalMiles` with it; on the
/// single-meter `modes:makeposseq/makeposseq_ctrl.dss` nothing moves at all.
/// Both halves are asserted here, on the port.
///
/// **The same leak also makes the FIRST run depend on meter creation order,
/// and this test says so** (G1.6(i) audit settlement, finding AT-1). With the
/// inner meter declared first, `DoLambdaCalcs` walks it first, so the outer
/// meter reads the already-written boundary bus on run 1 and lands on the
/// shifted answer immediately. That is not the port's reading of the Pascal:
/// **both** oracles do it, measured through the transports the gate uses
/// (`tmp/g16i/settle/probe_settle*.py`, re-runnable) —
/// `AccumulatedMilesDownStream(line.l1)` on a three-line nested pair is
/// `2.0 → 3.0 → 3.0` over three runs with the OUTER meter first and
/// `3.0 → 3.0 → 3.0` with the INNER meter first, identically on
/// dss_capi 0.14.5 and on the EPRI r4133 DDLL. The port reproduces neither
/// engine by choice: it is loop-for-loop the cited Pascal, and the ordering
/// arm below asserts that all three engines agree.
///
/// **Ruling (coordinator decision D4 chain, recorded in the G1.6(i) audit
/// settlement).** r4133's own comment calls the loop "Zero reliability
/// accumulators", so the sweep is *intended* to start from zeroed
/// accumulators; on a multi-meter circuit it does not, and the answer then
/// depends on run count and declaration order. Intent contradicts behaviour
/// ⇒ upstream defect, reported in
/// `investigations/to_opendss/61-relcalc-cross-zone-accumulator-leak.md`.
/// Fixing it is an ENGINE change in `solution/meters/reliability.rs`, which
/// G1.6(i) is explicitly forbidden to touch (brief R-14(d)) and which needs a
/// semantics decision this sub-step does not own: the physically correct
/// `Bus.TotalMiles` for a nested head bus is arguable (3.0 counts the inner
/// zone as downstream, 2.0 stops at the zone boundary). So it is recorded as
/// an engine finding for its own step; until then this test pins what all
/// three engines do, which is what keeps the one-shot gate protocol honest …
/// a port that quietly diverged here would otherwise go unnoticed.
#[test]
fn relcalc_is_not_idempotent_and_the_gate_runs_it_once() {
    // Single meter: the zone's own zeroing covers every bus it writes.
    let mut single = reliability_feeder();
    single.command("Relcalc");
    let once = (
        bus_total_miles(&single, "src"),
        accum_miles(&single, "line.l1"),
    );
    single.command("Relcalc");
    let twice = (
        bus_total_miles(&single, "src"),
        accum_miles(&single, "line.l1"),
    );
    assert_eq!(once, twice, "a single-meter zone is idempotent");
    assert_eq!(once.0, 2.0, "src accumulates l1 + l2 = 2 miles");

    // Nested meters: `b2` is zeroed by `m2` only, so `m1` reads `m2`'s miles.
    let mut dss = nested_meter_feeder();
    dss.command("Relcalc");
    assert_eq!(bus_total_miles(&dss, "src"), 2.0, "m1's zone is l1 + l2");
    assert_eq!(bus_total_miles(&dss, "b1"), 1.0);
    assert_eq!(bus_total_miles(&dss, "b2"), 1.0, "m2's zone is l3");
    assert_eq!(accum_miles(&dss, "line.l1"), 2.0);

    dss.command("Relcalc");
    assert_eq!(
        bus_total_miles(&dss, "src"),
        3.0,
        "the second run adds m2's zone mileage to m1's chain"
    );
    assert_eq!(bus_total_miles(&dss, "b1"), 2.0);
    assert_eq!(bus_total_miles(&dss, "b2"), 1.0, "m2's own zone stays put");
    assert_eq!(accum_miles(&dss, "line.l1"), 3.0);

    dss.command("Relcalc");
    assert_eq!(
        bus_total_miles(&dss, "src"),
        3.0,
        "and then it is a fixpoint: the inner zone re-derives the same \
         mileage every run, so the outer chain stays at the once-shifted value"
    );
    // Declaration order, same circuit: with the INNER meter first the outer
    // zone reads the already-written boundary bus on the FIRST run. Both
    // oracles measure 3.0 there (capi 0.14.5 and EPRI r4133; the probes are
    // cited in the doc above); the port must agree, run for run.
    let mut inner_first = nested_meter_feeder_inner_first();
    for run in 1..=3 {
        inner_first.command("Relcalc");
        assert_eq!(
            bus_total_miles(&inner_first, "src"),
            3.0,
            "run {run}: with `m2` declared before `m1` the outer chain is \
             shifted from the very first RelCalc: both oracles read \
             AccumulatedMilesDownStream(line.l1) = 3.0 on run 1 here, against \
             2.0 with the outer meter first"
        );
        assert_eq!(
            bus_total_miles(&inner_first, "b2"),
            1.0,
            "run {run}: m2's own zone is unaffected"
        );
    }
    assert_eq!(
        accum_miles(&inner_first, "line.l1"),
        3.0,
        "and the PD element carries the same shifted value"
    );
}

/// `Meters.AvgRepairTime` is `SumFltRatesXRepairHrs / SumBranchFltRates` with
/// **no zero guard** on any of the three engines — r4133
/// `Version8/Source/Meters/EnergyMeter.pas:2563`, dss_capi
/// `src/Meters/EnergyMeter.pas:2518`, port
/// `solution/meters/reliability.rs:293` — so a section whose branches all have
/// `faultrate=0` evaluates to `0.0 / 0.0 = NaN` identically everywhere, and
/// the reliability comparator's NaN rule (`NaN == NaN` is agreement, `NaN`
/// against a finite number is a failure) is what compares it. No case in the
/// gated corpus reaches it — every measured section has
/// `SumBranchFltRates > 0` — so the arm is pinned here instead.
#[test]
fn meter_avg_repair_time_is_nan_when_the_section_has_no_fault_rate() {
    let mut dss = Dss::new();
    dss.command("New circuit.t basekv=12.47 bus1=src phases=3");
    dss.command(
        "New line.l1 bus1=src bus2=b1 length=1 units=mi r1=0.1 x1=0.1 \
             faultrate=0 pctperm=80 repair=4",
    );
    dss.command(
        "New line.l2 bus1=b1 bus2=b2 length=1 units=mi r1=0.1 x1=0.1 \
             faultrate=0 pctperm=90 repair=5",
    );
    dss.command("New load.ld1 bus1=b1 phases=3 kv=12.47 kw=100 numcust=10");
    dss.command("New load.ld2 bus1=b2 phases=3 kv=12.47 kw=200 numcust=25");
    dss.command(
        "New recloser.r1 monitoredobj=line.l1 monitoredterm=1 \
             switchedobj=line.l1 switchedterm=1",
    );
    dss.command("New energymeter.m1 element=line.l1 terminal=1");
    dss.command("Set voltagebases=[12.47]");
    dss.command("CalcVoltageBases");
    dss.command("Solve mode=snap");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    dss.command("Relcalc");
    assert!(
        !dss.errors().iter().any(|e| e.contains("Aborting")),
        "the recloser gives the zone a section, got {:?}",
        dss.errors()
    );

    let view = dss.meter_reliability();
    assert_eq!(view.len(), 1);
    let sections = &view[0].sections;
    assert_eq!(sections.len(), 1, "one OCP device, one section");
    let s = &sections[0];
    assert_eq!(s.idx, 1, "sections are reported 1-based");
    assert_eq!(s.sum_branch_flt_rates, 0.0);
    assert_eq!(s.fault_rate_x_repair_hrs, 0.0);
    assert!(
        s.avg_repair_time.is_nan(),
        "0/0 must stay NaN, got {}",
        s.avg_repair_time
    );

    // The same feeder with a non-zero fault rate is finite — so the NaN above
    // is the division, not a dead section.
    let mut ok = ocp_feeder(
        "New recloser.r1 monitoredobj=line.l1 monitoredterm=1 \
             switchedobj=line.l1 switchedterm=1",
    );
    ok.command("Relcalc");
    let s = &ok.meter_reliability()[0].sections[0];
    assert!(s.sum_branch_flt_rates > 0.0);
    assert!(s.avg_repair_time.is_finite(), "{}", s.avg_repair_time);
}

/// The `Meters` walk contract [`Dss::meter_reliability`] has to match on both
/// oracle channels: the circuit's `EnergyMeters` creation order, **enabled
/// meters only** (r4133 `Version8/Source/DDLL/DMeters.pas:32-71` loops
/// `If pMeter.Enabled` in both `First` and `Next`; capi routes `Meters_Get_First`
/// / `_Next` through `Generic_CktElement_Get_First`/`_Next`,
/// `CAPI/CAPI_Meters.pas:153-167`), `TotalCustomers` read off the zone head's
/// FROM bus rather than off the meter, and the per-phase arrays sized by
/// `NPhases` and still zero until an `AllocateLoads` runs.
#[test]
fn meter_reliability_walks_enabled_meters_in_creation_order() {
    let mut dss = nested_meter_feeder();
    dss.command("Relcalc");

    let view = dss.meter_reliability();
    assert_eq!(
        view.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(),
        ["m1", "m2"],
        "creation order"
    );

    // `TotalCustomers` = `BusTotalNumCustomers` of `SequenceList[1]`'s FROM bus
    // (r4133 `DMeters.pas:232-243`), not a meter field: `m1`'s zone head is
    // `line.l1`, whose FROM bus is `src`; `m2`'s is `line.l3` at `b2`.
    assert_eq!(view[0].total_customers, bus_total_custs(&dss, "src"));
    assert_eq!(view[1].total_customers, bus_total_custs(&dss, "b2"));
    assert!(view[0].total_customers > view[1].total_customers);

    // The zone lists are the very lists `meter_zone` resolves, in order.
    let zone = dss.meter_zone("m1").unwrap();
    assert_eq!(view[0].branches, zone.all_branches_in_zone);
    assert_eq!(view[0].ends, zone.all_end_elements);
    assert_eq!(view[0].pce, zone.zone_pce);
    assert_eq!(view[0].branches, ["Line.l1", "Line.l2"]);

    // No OCP device anywhere -> the sweep aborts at 52902 and no section is
    // reported, on all three engines.
    assert!(
        dss.errors()
            .iter()
            .any(|e| e.contains("Aborting Reliability calc")),
        "{:?}",
        dss.errors()
    );
    for m in &view {
        assert_eq!(m.num_sections, 0);
        assert!(m.sections.is_empty());
        // `AllocateLoads` never ran: the port's arrays are zero-initialised
        // (`elements/meter/meter_element.rs:106,111`) where both oracles
        // return whatever `ReallocMem` left behind
        // (r4133 `Meters/MeterElement.pas:45-52`).
        assert_eq!(m.calc_current, vec![0.0; 3]);
        assert_eq!(m.alloc_factors, vec![0.0; 3]);
    }

    // A disabled meter drops out of the walk, exactly as it drops out of
    // `Meters.First`/`Next`.
    dss.command("Edit energymeter.m2 enabled=no");
    let view = dss.meter_reliability();
    assert_eq!(
        view.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(),
        ["m1"]
    );
}

/// The last-bit fixture for [`reliability_accumulators_are_correctly_rounded_f64_sums`]:
/// a four-line radial whose fault rates and lengths are chosen so that the
/// backward-sweep accumulators land on doubles whose shortest decimal needs 17
/// significant digits. `BranchFltRate = FaultRate × MilesThisLine ×
/// pctperm/100` (r4133 `PDElements/PDElement.pas:89-178`), so with
/// `pctperm=100` and one mile `l1` contributes 0.04 and `l2` 0.2; `l3`/`l4`
/// carry no fault rate and exist only for their length — 3 kft and 2 kft, the
/// `Line.l1a`-plus-downstream geometry of the two corpus decks
/// (`tests/corpus/controls/combo/midi_protection.dss:62`), so
/// `AccumulatedMilesDownStream(l3)` is that same 3 kft + 2 kft sum
/// (`Common/LineUnits.pas:77-113`).
fn last_bit_feeder() -> Dss {
    let mut dss = Dss::new();
    dss.command("New circuit.t basekv=12.47 bus1=src phases=3");
    dss.command(
        "New line.l1 bus1=src bus2=b1 length=1 units=mi r1=0.1 x1=0.1 \
             faultrate=0.04 pctperm=100 repair=4",
    );
    dss.command(
        "New line.l2 bus1=b1 bus2=b2 length=1 units=mi r1=0.1 x1=0.1 \
             faultrate=0.2 pctperm=100 repair=5",
    );
    dss.command(
        "New line.l3 bus1=b2 bus2=b3 length=3 units=kft r1=0.1 x1=0.1 \
             faultrate=0 pctperm=100 repair=6",
    );
    dss.command(
        "New line.l4 bus1=b3 bus2=b4 length=2 units=kft r1=0.1 x1=0.1 \
             faultrate=0 pctperm=100 repair=6",
    );
    dss.command("New load.ld1 bus1=b1 phases=3 kv=12.47 kw=100 numcust=10");
    dss.command("New load.ld2 bus1=b4 phases=3 kv=12.47 kw=200 numcust=25");
    dss.command("New energymeter.m1 element=line.l1 terminal=1");
    dss.command("Set voltagebases=[12.47]");
    dss.command("CalcVoltageBases");
    dss.command("Solve mode=snap");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss
}

/// The accumulation-ORDER fixture: one head line into a bus with THREE
/// laterals, so `AccumFltRate`'s `accumsum(FromBus.BusFltRate, ...)`
/// (r4133 `PDElements/PDElement.pas:105-114`) sums three non-zero terms into
/// the same bus, in the backward-sweep order
/// `For idx := SequenceList.ListSize downto 1`
/// (r4133 `Meters/EnergyMeter.pas:2475-2482`). `0.1`, `0.2` and `0.3` are
/// chosen because IEEE addition is NOT associative on them:
/// `(0.1+0.2)+0.3 = 0.6000000000000001` while `0.1+(0.2+0.3) = 0.6`, so a
/// re-association or a re-ordered zone walk moves the pinned literal. A
/// two-summand chain cannot do that (addition IS commutative), which is why
/// [`last_bit_feeder`] alone could not carry the order claim (G1.6(i) audit
/// settlement, finding AT-3).
fn branch_point_feeder() -> Dss {
    let mut dss = Dss::new();
    dss.command("New circuit.t basekv=12.47 bus1=src phases=3");
    for (name, bus2, rate) in [
        ("l1", "b1", 0.04),
        ("la", "ba", 0.1),
        ("lb", "bb", 0.2),
        ("lc", "bc", 0.3),
    ] {
        let bus1 = if name == "l1" { "src" } else { "b1" };
        dss.command(&format!(
            "New line.{name} bus1={bus1} bus2={bus2} length=1 units=mi r1=0.1 \
             x1=0.1 faultrate={rate} pctperm=100 repair=5"
        ));
    }
    for (n, bus) in [(1, "ba"), (2, "bb"), (3, "bc")] {
        dss.command(&format!(
            "New load.ld{n} bus1={bus} phases=3 kv=12.47 kw=100 numcust=10"
        ));
    }
    dss.command("New energymeter.m1 element=line.l1 terminal=1");
    dss.command("Set voltagebases=[12.47]");
    dss.command("CalcVoltageBases");
    dss.command("Solve mode=snap");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss
}

/// **The reliability accumulators are plain IEEE-754 double sums in the Pascal
/// sweep order, and they are right down to the last bit.**
///
/// GOLDEN_REBASE G1.6(i) micro-part F2s: the live gate reported 15 cells
/// (`accumulated_l`, `total_miles`, `cust_interrupts`, `saifikw`,
/// `sum_branch_flt_rates`) as 1-ULP Rust↔oracle divergences on BOTH channels.
/// They are not divergences at all. The raw JSON number token each oracle puts
/// on the wire round-trips to the port's own f64 bit for bit — 15/15, measured
/// at the wire on the capi (`tools/oracle/oracle_server.py`) and r4133
/// (`epri-worker`) transports — and what loses the last bit is the gate's own
/// decoder: `serde_json` WITHOUT the `float_roundtrip` feature decodes a
/// 17-significant-digit token as `significand as f64` followed by one divide by
/// a power of ten — two roundings — and lands 1 ULP away. Coordinator decision
/// **D11** turns that feature on workspace-wide; with it the five flagged cases
/// are green in both lanes (measured). No floor is owed here and the surface
/// stays `rel = abs = 0` — `tests/TOLERANCE_NOTES.md` §"The `Meters`
/// reliability surface (G1.6(i))".
///
/// This pin freezes the port's side with **both numbers** at each cell, so a
/// future change of precision is caught here and never
/// again confused with a transport artifact:
///
/// * `AccumulatedBrFltRate(l1) = 0.04 + 0.2` is `0.24000000000000002` (bits
///   `0x3fceb851eb851eb9`), **not** the `0.24` (bits `0x3fceb851eb851eb8`) a
///   lossy decode reads back — the same double, to the bit, that both oracles
///   put on the wire for `Line.bb12_13` on `controls:combo/midi_protection.dss`
///   and `controls:energymeter/midi_energymeter.dss`.
/// * `AccumulatedMilesDownStream(l3) = 3 kft + 2 kft` is `0.9469696969696969`
///   (bits `0x3fee4d9364d9364d`), **not** the `0.9469696969696968` (bits
///   `0x3fee4d9364d9364c`) a lossy decode reads back — the `Line.l1a` cell of
///   the same two decks.
#[test]
fn reliability_accumulators_are_correctly_rounded_f64_sums() {
    let mut dss = last_bit_feeder();
    dss.command("Relcalc");

    // --- the fault-rate accumulator -------------------------------------
    assert_eq!(accum_flt_rate(&dss, "line.l4"), 0.0, "l4 carries no lambda");
    assert_eq!(accum_flt_rate(&dss, "line.l3"), 0.0, "l3 carries no lambda");
    assert_eq!(accum_flt_rate(&dss, "line.l2"), 0.2, "l2 = its own lambda");

    let acc = accum_flt_rate(&dss, "line.l1");
    assert_eq!(
        acc.to_bits(),
        0.24000000000000002_f64.to_bits(),
        "AccumulatedBrFltRate(l1): the correctly-rounded double sum \
         0.04 + 0.2 = 0.24000000000000002 (0x{:016x}), got {acc:?} (0x{:016x})",
        0.24000000000000002_f64.to_bits(),
        acc.to_bits(),
    );
    assert_ne!(
        acc, 0.24,
        "0.24 is one ULP BELOW the correctly-rounded sum — it is what a JSON \
         decoder without round-trip guarantees reads back, never what any of \
         the three engines computes"
    );

    // --- the miles accumulator -------------------------------------------
    let own = accum_miles(&dss, "line.l3");
    assert_eq!(
        own.to_bits(),
        0.9469696969696969_f64.to_bits(),
        "AccumulatedMilesDownStream(l3) = 3 kft + 2 kft = 0.9469696969696969 mi \
         (0x{:016x}), got {own:?} (0x{:016x})",
        0.9469696969696969_f64.to_bits(),
        own.to_bits(),
    );
    assert_ne!(
        own, 0.9469696969696968,
        "one ULP BELOW: the lossily-decoded reading, not a computed value"
    );

    // --- the accumulation ORDER ------------------------------------------
    // Three laterals summing into one bus, in the backward-sweep order. The
    // sum is association-sensitive, so this literal moves if the zone walk or
    // the summation order ever changes. Both oracles read exactly this double
    // for `PDElements.AccumulatedL(Line.l1)` on the same deck: dss_capi 0.14.5
    // and EPRI r4133 (DDLL `PDElementsF` mode 5) both return
    // 0.6400000000000001 (probes in `tmp/g16i/settle/`).
    let mut branch = branch_point_feeder();
    branch.command("Relcalc");
    let head = accum_flt_rate(&branch, "line.l1");
    assert_eq!(
        head.to_bits(),
        0.6400000000000001_f64.to_bits(),
        "AccumulatedBrFltRate(l1) = ((0.1 + 0.2) + 0.3) + 0.04 in the sweep \
         order = 0.6400000000000001 (0x{:016x}), got {head:?} (0x{:016x}); \
         both oracles return this exact double",
        0.6400000000000001_f64.to_bits(),
        head.to_bits(),
    );
    // Non-vacuity of the ORDER claim: the other association is a different
    // double, so a re-ordered walk cannot pass the assertion above.
    assert_ne!(
        (0.1 + (0.2 + 0.3)) + 0.04,
        ((0.1 + 0.2) + 0.3) + 0.04,
        "the fixture is only an order pin while the two associations differ"
    );
    assert_ne!(
        head,
        (0.1 + (0.2 + 0.3)) + 0.04,
        "the head must carry the sweep-order sum, not the reverse association"
    );
}
