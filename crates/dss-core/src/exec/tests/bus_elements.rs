//! `Bus.AllPCEatBus` / `Bus.AllPDEatBus` — the port's own at-bus lists
//! ([`Dss::all_bus_elements`], GOLDEN_REBASE G1.4d) and the two class
//! predicates they rest on ([`ElemKind::is_power_delivery`] /
//! [`ElemKind::is_power_conversion`]).
//!
//! Upstream answers one bus per call by walking every PD/PC class and testing
//! each element (r4133 `Version8/Source/Common/Circuit.pas:1493-1535`
//! `getPDEatBus` / `:1540-1581` `getPCEatBus`, reached from
//! `DDLL/DBus.pas:840-865` / `:867-897`; capi `Common/Circuit.pas:1712-1794` /
//! `:1797-1870` from `CAPI/CAPI_Bus.pas:773-805`). The port answers **S4**
//! (coordinator decision D26): any terminal for a PD-class element under
//! r4133's own `bus1 <> bus2` shunt filter, terminal 1 for a PC-class element,
//! disabled elements included. The live corpus gate compares that answer
//! against both oracle channels case by case; what a live comparison cannot
//! state on its own, and what is nailed down here, is:
//!
//! 1. the two class sets are the **upstream** ones — `InheritsFrom(TPDClass)`
//!    and `InheritsFrom(TPCClass)` plus `Capacitor`/`Reactor` by name
//!    (`Circuit.pas:1559`) — and not the port's `pd_elements`/`pc_elements`
//!    membership, which puts `Fault` on neither list and the two shunts on the
//!    PD list only (`circuit/circuit.rs::add_ckt_element`);
//! 2. a 3-winding transformer is on its **third** bus's PD list — r4133's body
//!    tests `GetBus(1)`/`GetBus(2)` only and so contradicts its own header
//!    *"all PDE connected to the bus"* (`Circuit.pas:1490-1492`); the port keeps
//!    the header's promise;
//! 3. the shunt filter drops a `bus1 = bus2` element from the PD list of the
//!    bus it sits on — including a PD-only class, where that leaves **both**
//!    lists empty;
//! 4. neither list looks at `Enabled`, while the raw
//!    [`BusAttachment::by_node_ref`](crate::exec::BusAttachment::by_node_ref) a
//!    consumer replays capi's fast path over is empty for an element that was
//!    never enabled — the port's image of the staleness both engines carry;
//! 5. the lists are in the port's own creation order (both oracles emit class
//!    order instead, which is why the gate compares them as sets).
//!
//! The fixture below was measured against the pinned dss-python oracle
//! (capi 0.14.5) on 2026-09-06: it answers `b3` `AllPDEatBus =
//! ['Transformer.t3', '']` — dropping the never-enabled `Line.dead` the port
//! keeps (capi's fast path reads `TermNodeRef`, which that element never got)
//! — `b7` `['None']` on both lists, and `b2` `AllPCEatBus =
//! ['Load.ld', 'Capacitor.shunt', 'Reactor.ser', '']`, i.e. the same set in
//! class order rather than creation order. Every other bus agrees name for
//! name with the port.

use crate::circuit::ElemKind;
use crate::exec::{BusElementsView, Dss};

/// Every [`ElemKind`] variant. The `match` in [`expected_membership`] carries
/// no `_` arm, so adding a variant fails to compile until it is classified
/// against the upstream class tree here as well as in the engine.
const ALL_KINDS: [ElemKind; 22] = [
    ElemKind::Source,
    ElemKind::Line,
    ElemKind::Load,
    ElemKind::Transformer,
    ElemKind::AutoTrans,
    ElemKind::Capacitor,
    ElemKind::Reactor,
    ElemKind::Fault,
    ElemKind::Control,
    ElemKind::Generator,
    ElemKind::WindGen,
    ElemKind::PVSystem,
    ElemKind::Storage,
    ElemKind::IndMach012,
    ElemKind::VsConverter,
    ElemKind::Vccs,
    ElemKind::Upfc,
    ElemKind::GicLine,
    ElemKind::GicTransformer,
    ElemKind::Meter,
    ElemKind::EnergyMeter,
    ElemKind::Sensor,
];

/// `(is_power_delivery, is_power_conversion)` written out independently of the
/// engine's own two `match`es, one variant at a time.
fn expected_membership(kind: ElemKind) -> (bool, bool) {
    match kind {
        // `class(TPDClass)` under `Version8/Source/PDElements/` — the seven.
        ElemKind::Line => (true, false),
        ElemKind::Transformer => (true, false),
        ElemKind::AutoTrans => (true, false),
        ElemKind::Fault => (true, false),
        ElemKind::GicTransformer => (true, false),
        // PD by inheritance AND PC by name (`Circuit.pas:1559`).
        ElemKind::Capacitor => (true, true),
        ElemKind::Reactor => (true, true),
        // `CLASS(TPCClass)` under `Version8/Source/PCElements/`.
        // `Source` is VSource / ISource / GICSource, all three of them.
        ElemKind::Source => (false, true),
        ElemKind::Load => (false, true),
        ElemKind::Generator => (false, true),
        ElemKind::WindGen => (false, true),
        ElemKind::PVSystem => (false, true),
        ElemKind::Storage => (false, true),
        ElemKind::IndMach012 => (false, true),
        ElemKind::VsConverter => (false, true),
        ElemKind::Vccs => (false, true),
        ElemKind::Upfc => (false, true),
        ElemKind::GicLine => (false, true),
        // `TControlClass` / `TMeterClass` — neither walk ever reaches them.
        ElemKind::Control => (false, false),
        ElemKind::Meter => (false, false),
        ElemKind::EnergyMeter => (false, false),
        ElemKind::Sensor => (false, false),
    }
}

#[test]
fn the_two_class_predicates_are_the_upstream_class_sets() {
    for kind in ALL_KINDS {
        let (pd, pc) = expected_membership(kind);
        assert_eq!(
            kind.is_power_delivery(),
            pd,
            "{kind:?}: InheritsFrom(TPDClass)"
        );
        assert_eq!(
            kind.is_power_conversion(),
            pc,
            "{kind:?}: InheritsFrom(TPCClass) or named Capacitor/Reactor"
        );
    }
    // The two shunts are the only members of both sets, and `Fault` is the one
    // class that is PD upstream while the port's `pd_elements` list rejects it.
    let both: Vec<ElemKind> = ALL_KINDS
        .into_iter()
        .filter(|k| k.is_power_delivery() && k.is_power_conversion())
        .collect();
    assert_eq!(both, vec![ElemKind::Capacitor, ElemKind::Reactor]);
    assert!(ElemKind::Fault.is_power_delivery() && !ElemKind::Fault.is_power_conversion());
    // The classes with no at-bus role are exactly the control/meter families.
    let neither: Vec<ElemKind> = ALL_KINDS
        .into_iter()
        .filter(|k| !k.is_power_delivery() && !k.is_power_conversion())
        .collect();
    assert_eq!(
        neither,
        vec![
            ElemKind::Control,
            ElemKind::Meter,
            ElemKind::EnergyMeter,
            ElemKind::Sensor
        ]
    );
}

/// One deck holding every shape the two lists have to distinguish: a 3-winding
/// transformer (a bus reached only by winding 3), an AutoTrans, a shunt
/// Capacitor and a series Reactor (the two classes on both lists), a series
/// Fault and a shunt Fault (a PD-only class, so the shunt filter empties both
/// lists at its bus), an ISource, a Load, and a never-enabled Line.
///
/// Nothing here writes a file, so no directory guard is needed.
fn fixture() -> Dss {
    let mut dss = Dss::new();
    for c in [
        "clear",
        "new circuit.atbus basekv=12.47 pu=1.0 phases=3 bus1=src",
        "new linecode.lc nphases=3 r1=0.3 x1=0.7 r0=0.9 x0=2.1 c1=3.4 c0=1.6 units=km",
        "new line.feed bus1=src bus2=b1 linecode=lc length=0.5 units=km",
        // Winding 3 lands on `b3`, which nothing else touches.
        "new transformer.t3 windings=3 buses=[b1, b2, b3] conns=[delta, wye, wye] \
         kvs=[12.47, 4.16, 2.4] kvas=[5000, 5000, 5000] xhl=7 xht=7 xlt=7",
        "new autotrans.at windings=2 buses=[b1, b4] conns=[wye, wye] kvs=[12.47, 4.16] \
         kvas=[3000, 3000] xhx=5",
        // bus2 defaults to `b2.0.0.0` => `StripExtension(bus1) = StripExtension(bus2)`.
        "new capacitor.shunt bus1=b2 phases=3 kvar=600 kv=4.16",
        "new reactor.ser bus1=b2 bus2=b5 phases=3 r=0.1 x=1.0",
        "new fault.f1 bus1=b5 bus2=b6 phases=3 r=0.01",
        // Two PD-only elements whose two bus NAMES are the same after
        // `StripExtension`, so r4133's `myBus[0] <> myBus[1]` filter drops both
        // — a line down to the bus's own ground nodes (a real series branch:
        // the filter is by name, not by topology) and a shunt fault. Neither
        // class is a PC class, so `b7` reports two empty lists.
        "new line.grnd bus1=b7.1.2.3 bus2=b7.0.0.0 linecode=lc length=0.1 units=km",
        "new fault.fsh phases=3 bus1=b7 r=100",
        "new isource.inj bus1=b4 amps=10 phases=3",
        "new load.ld bus1=b2 phases=3 conn=wye kv=4.16 kw=500",
        // Never enabled => `SetNodeRef` never runs for it (`reprocess_bus_defs`
        // redoes enabled elements only), so its `node_ref` stays empty.
        "new line.dead bus1=b3 bus2=b9 linecode=lc length=0.1 units=km enabled=no",
        "set voltagebases=[12.47, 4.16, 2.4]",
        "calcvoltagebases",
        "solve",
    ] {
        dss.command(c);
        assert!(dss.errors().is_empty(), "{c}: {:?}", dss.errors());
    }
    dss
}

fn bus<'a>(views: &'a [BusElementsView], name: &str) -> &'a BusElementsView {
    views
        .iter()
        .find(|v| v.name == name)
        .unwrap_or_else(|| panic!("bus {name} is not in the BusList"))
}

#[test]
fn at_bus_lists_follow_the_s4_rule() {
    let dss = fixture();
    let views = dss.all_bus_elements();
    assert_eq!(
        views.iter().map(|v| v.name.as_str()).collect::<Vec<_>>(),
        ["src", "b1", "b2", "b3", "b4", "b5", "b6", "b7"],
        "BusList order"
    );

    // The source bus: the circuit's VSource is a `TPCClass` descendant.
    assert_eq!(bus(&views, "src").pce, ["Vsource.source"]);
    assert_eq!(bus(&views, "src").pde, ["Line.feed"]);

    // Creation order, not class order (both oracles emit class order).
    assert_eq!(
        bus(&views, "b1").pde,
        ["Line.feed", "Transformer.t3", "AutoTrans.at"]
    );
    assert!(bus(&views, "b1").pce.is_empty());

    // The shunt Capacitor is dropped from the PD list and kept on the PC list;
    // the series Reactor is on BOTH (`Circuit.pas:1559` adds it by name).
    assert_eq!(bus(&views, "b2").pde, ["Transformer.t3", "Reactor.ser"]);
    assert_eq!(
        bus(&views, "b2").pce,
        ["Capacitor.shunt", "Reactor.ser", "Load.ld"]
    );

    // Winding 3: r4133's `GetBus(1)`/`GetBus(2)` test misses it; S4 keeps it,
    // and so does the disabled `Line.dead` whose bus1 names this bus.
    assert_eq!(bus(&views, "b3").pde, ["Transformer.t3", "Line.dead"]);
    assert!(bus(&views, "b3").pce.is_empty());

    // An ISource is a PC element; the AutoTrans reaches `b4` on winding 2.
    assert_eq!(bus(&views, "b4").pde, ["AutoTrans.at"]);
    assert_eq!(bus(&views, "b4").pce, ["Isource.inj"]);

    // `Fault` is PD only: it is on the PD list of both its buses and on
    // neither PC list. The Reactor's terminal 1 is at `b2`, so `b5` — its
    // terminal 2 — does NOT carry it on the PC list.
    assert_eq!(bus(&views, "b5").pde, ["Reactor.ser", "Fault.f1"]);
    assert!(bus(&views, "b5").pce.is_empty());
    assert_eq!(bus(&views, "b6").pde, ["Fault.f1"]);
    assert!(bus(&views, "b6").pce.is_empty());

    // The shunt Fault: filtered off the PD list, never a PC class => both
    // lists empty even though the bus has an attachment.
    assert!(bus(&views, "b7").pde.is_empty() && bus(&views, "b7").pce.is_empty());
    assert_eq!(
        bus(&views, "b7")
            .attachments
            .iter()
            .map(|a| (a.name.as_str(), a.is_pd, a.is_pc, a.series))
            .collect::<Vec<_>>(),
        [
            ("Line.grnd", true, false, false),
            ("Fault.fsh", true, false, false)
        ]
    );
}

#[test]
fn attachments_publish_the_raw_terminal_facts_both_upstream_walks_read() {
    let dss = fixture();
    let views = dss.all_bus_elements();

    let at = |b: &str, e: &str| {
        bus(&views, b)
            .attachments
            .iter()
            .find(|a| a.name == e)
            .unwrap_or_else(|| panic!("{e} is not attached to {b}"))
            .clone()
    };

    // Winding 3 is terminal 3 by name and by node reference — the terminal
    // window is the ONLY thing separating the port from r4133 here.
    let t3_b3 = at("b3", "Transformer.t3");
    assert_eq!(t3_b3.by_name, [3]);
    assert_eq!(t3_b3.by_node_ref, [3]);
    assert!(t3_b3.is_pd && !t3_b3.is_pc && t3_b3.series && t3_b3.enabled);

    // A never-enabled element keeps its name attachment and has NO node
    // references at all — capi's fast path therefore drops it while its own
    // fallback (and r4133's name test) would not.
    let dead = at("b3", "Line.dead");
    assert_eq!(dead.by_name, [1]);
    assert!(dead.by_node_ref.is_empty());
    assert!(!dead.enabled);

    // The shunt Capacitor names `b2` on BOTH terminals: that is what makes
    // `series` false, and the bus still sees exactly one attachment.
    let cap = at("b2", "Capacitor.shunt");
    assert_eq!(cap.by_name, [1, 2]);
    assert!(!cap.series && cap.is_pd && cap.is_pc);
    assert_eq!(
        bus(&views, "b2")
            .attachments
            .iter()
            .filter(|a| a.name == "Capacitor.shunt")
            .count(),
        1
    );
    // Its grounded second terminal carries node reference 0 on every conductor,
    // so terminal 2 contributes no `by_node_ref` entry.
    assert_eq!(cap.by_node_ref, [1]);
}

#[test]
fn bus_elements_answers_one_bus_and_agrees_with_the_sweep() {
    let dss = fixture();
    let views = dss.all_bus_elements();
    for v in &views {
        let one = dss
            .bus_elements(&v.name.to_ascii_uppercase())
            .expect("case-insensitive BusList lookup");
        assert_eq!(one.name, v.name);
        assert_eq!(one.pde, v.pde);
        assert_eq!(one.pce, v.pce);
        assert_eq!(one.attachments.len(), v.attachments.len());
    }
    assert!(dss.bus_elements("nosuchbus").is_none());
    // No circuit at all: the sweep is empty and the single-bus read is `None`.
    let empty = Dss::new();
    assert!(empty.all_bus_elements().is_empty());
    assert!(empty.bus_elements("src").is_none());
}
