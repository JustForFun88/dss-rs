//! GOLDEN_REBASE G1.6b — expected-value pins for the `PDElements` walk
//! ([`Dss::pd_elements`] / [`crate::exec::view::PdElementView`]).
//!
//! The live gate compares the walk against both oracle channels; these pins fix
//! the port-side facts that comparison rests on — membership and order, the
//! 1-based `FromTerminal` encoding, the parent linkage, and the fact that the
//! four `RelCalc`-fed fields are still zero because no live deck runs the
//! reliability sweep.

use crate::exec::*;

/// The vendored IEEE 123-bus test case, metered and solved.
///
/// `IEEE123Master.dss` (plus the four files it redirects) is pure model
/// definition — no `export`/`show`/`save`/`plot`/`buscoords` — so it writes
/// nothing and needs no directory guard; the meter and `MaxControlIter` line
/// are the two commands `Run_IEEE123Bus.DSS`'s first script adds around it.
/// Never `.inputs/`: the vendored corpus tree is the one the live gate reads
/// (CLAUDE.md).
fn ieee123() -> Dss {
    let deck = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../tests/corpus/electricdss-tst/Version8/Distrib/IEEETestCases/123Bus/IEEE123Master.dss",
    );
    assert!(deck.is_file(), "vendored corpus deck missing: {deck:?}");
    let mut dss = Dss::new();
    dss.command(&format!("compile \"{}\"", deck.display()));
    dss.command("New EnergyMeter.Feeder Line.L115 1");
    dss.command("Set MaxControlIter=30");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss
}

/// A two-branch metered feeder carrying every membership edge case: a `Fault`
/// object (`NON_PCPD_ELEM` — never a `PDElements` member), a disabled line
/// (both oracles' `First`/`Next` skip `not Enabled`), a shunt capacitor and a
/// series reactor. `line.l2` is declared **reversed** (`bus1=b2 bus2=b1`), so
/// the zone build reaches it through its *second* terminal.
fn membership_feeder() -> Dss {
    let mut dss = Dss::new();
    dss.command("New circuit.t basekv=12.47 bus1=src phases=3");
    dss.command("New line.l1 bus1=src bus2=b1 length=1 units=mi r1=0.1 x1=0.1");
    dss.command("New line.l2 bus1=b2 bus2=b1 length=1 units=mi r1=0.1 x1=0.1");
    dss.command("New line.loff bus1=b2 bus2=b9 length=1 units=mi r1=0.1 x1=0.1 enabled=no");
    dss.command("New capacitor.c1 bus1=b2 phases=3 kv=12.47 kvar=300");
    dss.command("New reactor.r1 bus1=b1 bus2=b3 phases=3 r=0.1 x=1.0");
    dss.command("New load.ld1 bus1=b2 phases=3 kv=12.47 kw=100 numcust=4");
    dss.command("New fault.f1 bus1=b2 phases=3 r=1000");
    dss.command("New energymeter.m1 element=line.l1 terminal=1");
    dss.command("Set voltagebases=[12.47]");
    dss.command("CalcVoltageBases");
    dss.command("Solve mode=snap");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss
}

/// **Pin (G1.6b-3).** The walk is the circuit's `PDElements` pointer list in
/// `AddCktElement` creation order, enabled members only.
///
/// Membership is `DSSObjType and BaseClassMask = PD_ELEMENT`
/// (`Common/Circuit.pas:2242-2248`): Line / Transformer / AutoTrans /
/// Capacitor / Reactor / GICTransformer. `Fault` is
/// `FAULTOBJECT + NON_PCPD_ELEM` (`PDElements/Fault.pas:114`) and is therefore
/// **not** a member, however PD-like it looks; both oracles' iterators skip
/// disabled elements (capi `Generic_CktElement_Get_First/Next`,
/// `CAPI/CAPI_Utils.pas:718-759`; r4133 `DDLL/DPDELements.pas:27-59`).
///
/// Measured on IEEE 123: **138** rows, opening
/// `Transformer.reg1a, Line.l115, Line.l1, Line.l2, Line.l3` — the regulator
/// transformer first because `IEEE123Regulators.DSS` is redirected before the
/// line definitions.
#[test]
fn pd_elements_walk_is_the_enabled_pd_list_in_creation_order() {
    let dss = ieee123();
    let walk = dss.pd_elements();
    assert_eq!(walk.len(), 138, "IEEE123 PDElements count");
    assert_eq!(
        walk.iter()
            .take(5)
            .map(|v| v.name.as_str())
            .collect::<Vec<_>>(),
        [
            "Transformer.reg1a",
            "Line.l115",
            "Line.l1",
            "Line.l2",
            "Line.l3"
        ]
    );
    for v in &walk {
        let class = v.name.split('.').next().unwrap();
        assert!(
            matches!(
                class,
                "Line" | "Transformer" | "AutoTrans" | "Capacitor" | "Reactor" | "GICTransformer"
            ),
            "unexpected PDElements member class in {}",
            v.name
        );
    }

    // The membership edge cases, on a deck that has all of them.
    let dss = membership_feeder();
    let walk = dss.pd_elements();
    let names: Vec<&str> = walk.iter().map(|v| v.name.as_str()).collect();
    assert_eq!(
        names,
        ["Line.l1", "Line.l2", "Capacitor.c1", "Reactor.r1"],
        "Fault.f1 is NON_PCPD_ELEM and line.loff is disabled — neither is a member"
    );
    // `is_shunt` follows the Bus2 side effect, not the class: the capacitor is
    // shunt-connected, the two-terminal reactor is not.
    let shunt: Vec<(&str, bool)> = walk.iter().map(|v| (v.name.as_str(), v.is_shunt)).collect();
    assert_eq!(
        shunt,
        [
            ("Line.l1", false),
            ("Line.l2", false),
            ("Capacitor.c1", true),
            ("Reactor.r1", false)
        ]
    );
    // The reported reliability inputs are the class defaults, verbatim:
    // Line 0.1 / 20.0 / 3.0 (`PDElements/Line.pas:839-841`), Capacitor and
    // Reactor 0.0005 / 100.0 / 3.0 (`Capacitor.pas:555-557`,
    // `Reactor.pas:601-603`).
    let cap = walk.iter().find(|v| v.name == "Capacitor.c1").unwrap();
    assert_eq!(
        (cap.fault_rate, cap.pct_permanent, cap.repair_time),
        (0.0005, 100.0, 3.0)
    );
    let line = walk.iter().find(|v| v.name == "Line.l1").unwrap();
    assert_eq!(
        (line.fault_rate, line.pct_permanent, line.repair_time),
        (0.1, 20.0, 3.0)
    );
    // A transformer sets only `FaultRate := 0.007` (`Transformer.pas:959`) and
    // leaves `PctPerm`/`HrsToRepair` at Pascal's zero-initialized 0.0 — the
    // walk reports that, it does not substitute a "sensible" default.
    let reg = ieee123()
        .pd_elements()
        .into_iter()
        .find(|v| v.name == "Transformer.reg1a")
        .unwrap();
    assert_eq!(
        (reg.fault_rate, reg.pct_permanent, reg.repair_time),
        (0.007, 0.0, 0.0)
    );
}

/// **Pin (G1.6b-4).** `from_terminal` is reported **1-based**, the way both
/// oracles read `TPDElement.FromTerminal` (capi `CAPI_PDElements.pas:284-292`,
/// r4133 `PDElementsI:7`), while the port stores it 0-based
/// (`elements/ckt.rs:148-149`). Pascal's constructor default is
/// `FromTerminal := 1` (`PDElements/PDElement.pas:194`) = the port's
/// `Some(0)` (`elements/ckt.rs:285-286`), so an unmetered element reports 1,
/// and a branch the zone build entered through terminal 2 reports 2
/// (`solution/meters/zones/build.rs:338` stores `Some(j - 1)`).
/// The `None` arm — no Pascal counterpart, the port's "never set" state —
/// reports 0.
#[test]
fn pd_elements_from_terminal_is_one_based() {
    // Every IEEE123 branch is entered through terminal 1.
    let by_terminal: std::collections::BTreeSet<i32> = ieee123()
        .pd_elements()
        .iter()
        .map(|v| v.from_terminal)
        .collect();
    assert_eq!(by_terminal, [1].into_iter().collect());

    // `line.l2` is declared bus1=b2 bus2=b1, so the zone build walks into it
    // from b1 — its *second* terminal.
    let mut dss = membership_feeder();
    let walk = dss.pd_elements();
    let ft = |n: &str| walk.iter().find(|v| v.name == n).unwrap().from_terminal;
    assert_eq!(ft("Line.l1"), 1);
    assert_eq!(ft("Line.l2"), 2, "reversed branch is entered at terminal 2");

    // The unset arm: clear the stored `Option` on `line.l1` and re-read.
    let l1 = dss
        .circuit
        .as_ref()
        .expect("solved circuit")
        .pd_elements
        .iter()
        .copied()
        .find(|r| {
            dss.classes[r.class_ord()].arena[r.index()]
                .data()
                .name()
                .eq_ignore_ascii_case("l1")
        })
        .expect("line.l1 is a PD element");
    dss.classes[l1.class_ord()]
        .arena
        .try_ckt_elem_mut(l1.index())
        .expect("a Line is a circuit element")
        .cd_mut()
        .from_terminal = None;
    assert_eq!(
        dss.pd_elements()
            .iter()
            .find(|v| v.name == "Line.l1")
            .unwrap()
            .from_terminal,
        0,
        "an unset FromTerminal reports 0, never a spurious 1"
    );
}

/// **Pin (G1.6b-5).** `parent_class_index` is the upline branch's Pascal
/// `ClassIndex` — the **1-based, per-class** creation index that
/// `AddObjectToList` stamps (`General/DSSObject.pas:43`) — and
/// `parent_name` is that same element's FullName. A branch with no upline
/// parent reports `0` / `""`.
///
/// Measured on IEEE 123 (`New EnergyMeter.Feeder Line.L115 1`): `Line.l1`'s
/// parent is `Line.l115` at class index **1** (`L115` is the first `Line`
/// created), and the seven parentless rows are the regulator transformer, the
/// meter's own head branch `Line.l115`, the tie switch `Line.sw1` and the four
/// shunt capacitors. `parent_class_index` alone is ambiguous — 85 distinct
/// values against `parent_name`'s 88, because the index is per class — which is
/// why both are compared against the oracles.
#[test]
fn pd_elements_parent_is_the_upline_branch_by_name_and_class_index() {
    let dss = ieee123();
    let walk = dss.pd_elements();

    let l1 = walk.iter().find(|v| v.name == "Line.l1").unwrap();
    assert_eq!(
        (l1.parent_class_index, l1.parent_name.as_str()),
        (1, "Line.l115")
    );

    let roots: Vec<&str> = walk
        .iter()
        .filter(|v| v.parent_class_index == 0)
        .map(|v| v.name.as_str())
        .collect();
    assert_eq!(
        roots,
        [
            "Transformer.reg1a",
            "Line.l115",
            "Line.sw1",
            "Capacitor.c83",
            "Capacitor.c88a",
            "Capacitor.c90b",
            "Capacitor.c92c"
        ]
    );
    for v in &walk {
        assert_eq!(
            v.parent_class_index == 0,
            v.parent_name.is_empty(),
            "index and name must agree about {} having a parent",
            v.name
        );
    }

    // The index really is per-class and 1-based: for every parented row, the
    // parent's index equals its 1-based position among the walk's rows of the
    // parent's own class. (Every IEEE123 parent is itself a PD element, so the
    // walk is a complete per-class ordering for this check.)
    for v in walk.iter().filter(|v| v.parent_class_index != 0) {
        let class = v.parent_name.split('.').next().unwrap();
        let pos = walk
            .iter()
            .filter(|w| w.name.starts_with(class))
            .position(|w| w.name == v.parent_name)
            .expect("the parent is itself a walked PD element");
        assert_eq!(
            v.parent_class_index as usize,
            pos + 1,
            "{}: ClassIndex of {}",
            v.name,
            v.parent_name
        );
    }
    assert_eq!(
        walk.iter()
            .map(|v| v.parent_class_index)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        85
    );
    assert_eq!(
        walk.iter()
            .map(|v| v.parent_name.as_str())
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        88
    );
}

/// **Pin (G1.6b-6).** `section_id`, `total_miles`, `lambda` and `accumulated_l`
/// are the four fields the EnergyMeter *reliability sweep* writes
/// (`solution/meters/reliability.rs:113-115,180`; Pascal `CalcFltRate`'s only
/// caller is `EnergyMeter.pas:2479` inside `CalcReliabilityIndices`, and
/// `BranchSectionID`/`AccumulatedMilesDownStream` come from
/// `PDElements/PDElement.pas:106-110,183`). No live corpus deck runs
/// `RelCalc`, so on a merely metered-and-solved circuit all four are **0** on
/// the port exactly as on both oracles.
///
/// The reading is deliberately the *stored* `TPDElement` field, not
/// `CalcFltRate`'s product: `Line.l115` has `FaultRate` 0.1 and `PctPerm` 20.0
/// with `Len` 0.4, so `ReliabilityData::branch_flt_rate` is 0.008 — reporting
/// **that** as `Lambda` would manufacture a divergence on every metered deck.
///
/// **G1.6(i) owns these four fields' non-vacuity demo**: it drives `RelCalc`
/// and turns them into accumulated sums. Until then they are wired and
/// compared — which asserts the port does not populate them prematurely — but
/// they are not multi-valued coverage.
#[test]
fn pd_elements_relcalc_fields_are_zero_without_relcalc() {
    let dss = ieee123();
    for v in dss.pd_elements() {
        assert_eq!(
            (v.section_id, v.total_miles, v.lambda, v.accumulated_l),
            (0, 0.0, 0.0, 0.0),
            "{} carries a RelCalc value without RelCalc",
            v.name
        );
    }

    // `Lambda` is the stored `BranchFltRate`, not what `CalcFltRate` would
    // produce from the same element's inputs.
    let Dss {
        classes, circuit, ..
    } = &dss;
    let ckt = circuit.as_ref().expect("solved circuit");
    let l115 = ckt
        .pd_elements
        .iter()
        .copied()
        .find(|r| {
            classes[r.class_ord()].arena[r.index()]
                .data()
                .name()
                .eq_ignore_ascii_case("l115")
        })
        .expect("line.l115");
    let computed = classes[l115.class_ord()]
        .arena
        .try_ckt_elem(l115.index())
        .expect("a Line is a circuit element")
        .reliability_data()
        .branch_flt_rate;
    assert_eq!(computed, 0.008, "CalcFltRate = 0.1 * 20.0 * 0.01 * 0.4");
    assert_eq!(
        dss.pd_elements()
            .iter()
            .find(|v| v.name == "Line.l115")
            .unwrap()
            .lambda,
        0.0,
        "the walk reports the stored BranchFltRate, which RelCalc never wrote"
    );
}
