//! The per-element **index/name scalars** of `GOLDEN_REBASE_PLAN.md` G1.3d(i) —
//! `CktElement.NumTerminals` / `NumConductors` / `NumPhases`,
//! `CktElement.NodeOrder` and `CktElement.EnergyMeter` — as they come out of
//! [`Dss::snapshot_elements`](crate::exec::Dss::snapshot_elements).
//!
//! These are the sub-step's expected-value pins. The live corpus gate compares
//! all five against both oracle channels, case by case and **exactly** (they are
//! discrete: no tolerance is involved anywhere in this file); what a live
//! comparison cannot state on its own, and what is nailed down here, is:
//!
//! 1. `NodeOrder` is the *bus-local* node number of each conductor slot in the
//!    element's own terminal-major order — a rotated bus spec rotates it — and
//!    a grounded conductor reads `0`;
//! 2. the accessor and the frozen `Export NodeOrder` renderer agree row for row,
//!    so the two code paths cannot drift (WP-G1 moves no golden byte, so the
//!    renderer could not be refactored into the accessor);
//! 3. `EnergyMeter` names the meter that *meters* the element — its
//!    `MeteredElement` — and not every element in that meter's zone;
//! 4. the shapes with no `NodeRef` to report (never-enabled, 0-terminal) are
//!    empty, which is what makes the enabled-only capture predicate honest,
//!    while an element **disabled after a solve** keeps the mapping it was
//!    given (the port answers from state, not from `Enabled`);
//! 5. the three counts are the element's own `NTerms`/`NConds`/`NPhases`,
//!    including the cases where they differ from each other.
//!
//! Pascal: r4133 `Version8/Source/DDLL/DCktElement.pas:139`/`:144`/`:149`
//! (`CktElementI` modes `0`/`1`/`2`), `:442` (`CktElementS` mode `4`), `:1032`
//! (`CktElementV` mode `17`, over `Common/Utilities.pas:1718` `GetNodeNum`);
//! capi `CAPI/CAPI_CktElement.pas:202`/`:182`/`:192`/`:672` and
//! `CAPI/CAPI_Alt.pas:953`. All five are fastdss `ICktElement._columns`
//! surfaces (`git -C .inputs/DSS-Python show origin/fastdss:dss/ICktElement.py`).

use crate::exec::{Dss, ElementSnapshot};

/// One deck exercising every shape the accessor has to distinguish: a rotated
/// bus spec, a grounded neutral, a single-phase element, an element that is
/// `enabled=no` from birth, a 0-terminal control and a metered branch.
///
/// Nothing here writes a file (the `Export` pin below sets its own datapath
/// first), so no directory guard is needed.
fn fixture() -> Dss {
    let mut dss = Dss::new();
    for c in [
        "clear",
        "new circuit.extras basekv=12.47 pu=1.0 phases=3 bus1=src",
        "new line.l1 bus1=src.1.2.3 bus2=b1.1.2.3 phases=3 r1=0.3 x1=0.7 length=1",
        // Terminal 1 is wired 3-1-2: the conductor order is the ELEMENT's, so
        // this is what separates `NodeOrder` from "1..nconds per terminal".
        "new line.rot bus1=b1.3.1.2 bus2=b2.1.2.3 phases=3 r1=0.3 x1=0.7 length=1",
        // A wye load has a 4th (neutral) conductor; `b2.1.2.3` leaves it at the
        // default node 0, i.e. solidly grounded — the `GetNodeNum(0) = 0` slot.
        "new load.wye bus1=b2.1.2.3 phases=3 conn=wye kv=12.47 kw=100",
        "new load.a bus1=b2.1 phases=1 kv=7.2 kw=10",
        // Never enabled => `SetNodeRef` never runs for it (`reprocess_bus_defs`
        // walks enabled elements only, `circuit/circuit.rs`).
        "new line.tie bus1=b2 bus2=b3 phases=3 r1=0.1 x1=0.1 length=1 enabled=no",
        // `TUPFCControlObj.Create` never assigns `Nterms` (r4133
        // `Version8/Source/Controls/UPFCControl.pas:230-246`): a legitimately
        // 0-terminal, 0-conductor circuit element.
        "new upfccontrol.uc",
        // Mixed-case spelling on purpose — both engines and the port store the
        // name lowercased in the constructor.
        "new energymeter.Feeder element=line.l1 terminal=1",
        "set voltagebases=[12.47]",
        "calcvoltagebases",
        "solve",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss
}

fn elem<'a>(snaps: &'a [ElementSnapshot], name: &str) -> &'a ElementSnapshot {
    snaps
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case(name))
        .unwrap_or_else(|| panic!("{name} not in the snapshot"))
}

fn scratch(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("dss_exec_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&d).expect("mkdir scratch");
    d
}

// Expected-value pin — `NodeOrder` is `GetNodeNum(NodeRef^[j])` per conductor
// slot, terminal-major (r4133 `DDLL/DCktElement.pas:1043-1050`, capi
// `CAPI/CAPI_Alt.pas:970-977`), i.e. the *bus-local* node number the element's
// own bus spec assigned to that conductor — never the global node index and
// never a fixed `1..nconds` run. `GetNodeNum(0) = 0` is ground
// (`Common/Utilities.pas:1718-1722`).
/// The node order follows the element's bus spec conductor by conductor: a
/// `3.1.2` terminal reports `[3, 1, 2]`, and a grounded conductor reports `0`.
#[test]
fn node_order_is_the_bus_local_node_number_per_conductor() {
    let mut dss = fixture();
    let snaps = dss.snapshot_elements();

    // `bus1=b1.3.1.2 bus2=b2.1.2.3`: terminal 1 is rotated, terminal 2 is not.
    // If the accessor read anything but the element's own `NodeRef`, the two
    // terminals would be identical here.
    let rot = elem(&snaps, "Line.rot");
    assert_eq!(rot.node_order, vec![3, 1, 2, 1, 2, 3]);
    // ... and the unrotated sibling on the same two buses is the flat run, so
    // the rotation above is a property of the deck and not of the reader.
    assert_eq!(elem(&snaps, "Line.l1").node_order, vec![1, 2, 3, 1, 2, 3]);

    // The wye load's 4th conductor is the neutral, left at node 0 = ground.
    let wye = elem(&snaps, "Load.wye");
    assert_eq!((wye.n_terms, wye.n_conds), (1, 4));
    assert_eq!(wye.node_order, vec![1, 2, 3, 0]);
    // A 1-phase wye load: one live conductor, one grounded.
    assert_eq!(elem(&snaps, "Load.a").node_order, vec![1, 0]);
    // The source's terminal 2 is the grounded reference — three ground slots.
    assert_eq!(
        elem(&snaps, "Vsource.source").node_order,
        vec![1, 2, 3, 0, 0, 0]
    );

    // Length is `n_terms · n_conds` for every element that has a mapping — the
    // array size both oracles allocate (r4133 `:1043`, capi `:968`).
    for e in &snaps {
        if !e.node_order.is_empty() {
            assert_eq!(
                e.node_order.len(),
                e.n_terms * e.n_conds,
                "{}: node order length must be nterms · nconds",
                e.name
            );
        }
    }
}

// Expected-value pin — the accessor and the `Export NodeOrder` report path read
// the same mapping. WP-G1 may move no committed golden byte, so the report
// renderer (`report/export/node_order.rs`, Pascal `ExportNodeOrder` +
// `WriteNodeList`) could not be refactored into the accessor and the two walks
// exist side by side; this pin is what keeps them from drifting.
/// Every `Export NodeOrder` row equals the snapshot's counts and node order for
/// the same element.
#[test]
fn node_order_matches_the_export_nodeorder_row() {
    let dir = scratch("nodeorder_row");
    let mut dss = fixture();
    let snaps = dss.snapshot_elements();
    dss.command(&format!("set datapath=\"{}\"", dir.display()));
    dss.command("export nodeorder");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    let file = std::fs::read_dir(&dir)
        .expect("read scratch")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .find(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.eq_ignore_ascii_case("extras_EXP_NodeOrder.csv"))
        })
        .expect("Export NodeOrder wrote no file");
    let text = std::fs::read_to_string(&file).expect("read export");

    let mut rows = 0usize;
    for line in text.lines().skip(1).filter(|l| !l.trim().is_empty()) {
        // `"Element", Nterminals, Nconductors, Node-1, …`
        let mut it = line.split(',').map(str::trim);
        let name = it.next().expect("row name").trim_matches('"').to_string();
        let nums: Vec<i32> = it.map(|f| f.parse().expect("integer field")).collect();
        let snap = elem(&snaps, &name);
        assert_eq!(
            (nums[0], nums[1]),
            (snap.n_terms as i32, snap.n_conds as i32),
            "{name}: the export's counts and the snapshot's must agree",
        );
        assert_eq!(
            nums[2..],
            snap.node_order[..],
            "{name}: the export's node list and the accessor's must agree",
        );
        rows += 1;
    }
    // The export walks Sources → PD → Faults → PC (enabled only), so it is a
    // strict subset of the snapshot; assert it is not an *empty* subset, or the
    // loop above would pass vacuously.
    assert_eq!(
        rows, 5,
        "expected the source, two lines and two loads: {text}"
    );
}

// Expected-value pin — `EnergyMeter` answers for the meter's **MeteredElement**
// only. `SetHasMeterFlag` clears `HasEnergyMeter` on every PD element and sets
// it on each enabled meter's `MeteredElement` (r4133
// `Meters/EnergyMeter.pas:1712-1719`), and that element is where
// `MakeMeterZoneLists` assigns `MeterObj := Self` (`:1777`/`:1782`); both
// engines gate the read on exactly that flag (r4133 `DCktElement.pas:444`, capi
// `CAPI_CktElement.pas:682`). So the rest of the zone — which the meter very
// much does measure — reports no meter on this surface. The name is the bare
// object name, lowercased by the constructor in the port
// (`elements/ckt.rs`), in r4133 (`Meters/EnergyMeter.pas:921` `LowerCase`) and
// in capi (`src/Meters/EnergyMeter.pas:952` `AnsiLowerCase`), so the channel is
// compared with no case folding.
/// `EnergyMeter` is the metered element's meter, spelled lowercase — not the
/// zone's, and not the deck's `Feeder` capitalization.
#[test]
fn energy_meter_is_the_bare_lowercased_meter_name() {
    let mut dss = fixture();
    let snaps = dss.snapshot_elements();

    // `new energymeter.Feeder element=line.l1 terminal=1` — deck spelling
    // `Feeder`, stored and reported spelling `feeder`.
    assert_eq!(
        elem(&snaps, "Line.l1").energy_meter.as_deref(),
        Some("feeder")
    );
    assert!(
        snaps.iter().any(|e| e.name == "EnergyMeter.feeder"),
        "the meter object itself is stored lowercased: {:?}",
        snaps.iter().map(|e| &e.name).collect::<Vec<_>>()
    );

    // `Line.rot` and both loads are downstream of `Line.l1`, i.e. inside the
    // meter's zone, and still report no meter: the flag marks the metered
    // element, not the zone.
    for name in ["Line.rot", "Load.wye", "Load.a"] {
        assert_eq!(
            elem(&snaps, name).energy_meter,
            None,
            "{name} is in the zone but is not the metered element",
        );
    }
    // Exactly one element carries a meter name here — one meter, one metered
    // element.
    assert_eq!(
        snaps.iter().filter(|e| e.energy_meter.is_some()).count(),
        1,
        "{:?}",
        snaps
            .iter()
            .filter(|e| e.energy_meter.is_some())
            .map(|e| (&e.name, &e.energy_meter))
            .collect::<Vec<_>>()
    );
}

// Expected-value pin (G1.3d(i) capture predicate) — the two shapes with nothing
// to report. A never-enabled element never gets `SetNodeRef`, so its `NodeRef`
// is still nil upstream: capi warns 15013 and returns its `DefaultResult`
// (`CAPI/CAPI_Alt.pas:960-966`) while r4133, which has no such guard,
// dereferences the nil pointer (`DDLL/DCktElement.pas:1048`). A 0-terminal
// element makes r4133 allocate a zero-length array (`:1043`) while capi still
// takes its nil-`NodeRef` branch. The capture therefore issues the read only
// for enabled elements with terminals, and these two pins are what make that
// skip honest: the port has nothing to report in either state either.
/// A never-enabled element has no node order at all.
#[test]
fn a_never_enabled_element_has_no_node_order() {
    let mut dss = fixture();
    let snaps = dss.snapshot_elements();
    let tie = elem(&snaps, "Line.tie");
    assert!(!tie.enabled, "Line.tie is `enabled=no` in the deck");
    // The counts are still the element's own — only the mapping is missing.
    assert_eq!((tie.n_terms, tie.n_conds, tie.n_phases), (2, 3, 3));
    assert!(
        tie.node_order.is_empty(),
        "no NodeRef means no node order; got {:?}",
        tie.node_order
    );
}

/// A 0-terminal element (`UPFCControl`) has no node order and no conductors.
#[test]
fn a_zero_terminal_element_has_no_node_order() {
    let mut dss = fixture();
    let snaps = dss.snapshot_elements();
    let uc = elem(&snaps, "UPFCControl.uc");
    assert!(
        uc.enabled,
        "the control is enabled — only its terminals are 0"
    );
    assert_eq!((uc.n_terms, uc.n_conds, uc.n_phases), (0, 0, 0));
    assert!(uc.node_order.is_empty(), "{:?}", uc.node_order);
}

// Expected-value pin — the accessor answers from the element's state, not from
// `Enabled`: an element disabled *after* a solve keeps the `NodeRef` it was
// given (nothing clears it — `set_enabled` only raises
// `signal_bus_name_redefined`, `elements/ckt.rs`), and so do both oracles,
// whose mode-17 reads carry no `Enabled` guard at all. The capture skips
// disabled elements for the never-enabled crash above, so the live comparator
// must assert the ORACLE side is empty there and never require the port to be
// (the shape `compare_element_derived` already uses for its disabled branch).
/// An element disabled after a solve keeps the node order it was given.
#[test]
fn a_disabled_element_keeps_the_node_order_it_was_given() {
    let mut dss = Dss::new();
    for cmd in [
        "clear",
        "new circuit.late basekv=12.47 phases=3 bus1=src",
        "new line.a bus1=src.1.2.3 bus2=b.1.2.3 phases=3 r1=0.1 x1=0.1 length=1",
        "new load.l bus1=b.1.2.3 phases=3 kv=12.47 kw=10",
        "solve",
        "edit line.a enabled=no",
        "solve",
    ] {
        dss.command(cmd);
    }
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let snaps = dss.snapshot_elements();
    let a = elem(&snaps, "Line.a");
    assert!(!a.enabled);
    assert_eq!(a.node_order, vec![1, 2, 3, 1, 2, 3]);
}

// Regression pin (the G1.3a audit settlement class) — a `NodeRef` SHORTER than
// `Yorder` must not panic here. `set_nterms`/`set_nconds` grow `yorder` and
// reallocate the terminal buffers but leave `node_ref` alone; only
// `set_node_ref` resizes it (`elements/ckt.rs`), and `reprocess_bus_defs`
// re-runs it for enabled elements only — so a disabled element that grows
// phases keeps a short `node_ref`, and a `node_order` written as
// `node_ref[..yorder]` would panic in the public `snapshot_elements`.
/// A stale, too-short `NodeRef` reports a full-length node order whose missing
/// slots read as ground.
#[test]
fn a_stale_node_ref_reads_the_missing_slots_as_ground() {
    let mut dss = Dss::new();
    for cmd in [
        "clear",
        "new circuit.stale basekv=12.47 phases=3 bus1=src",
        "new line.a bus1=src.1 bus2=b.1 phases=1 r1=0.1 x1=0.1 length=1",
        "new load.l bus1=b.1 phases=1 kv=7.2 kw=10",
        "solve",
        // Disabled first, so `ReProcessBusDefs` no longer re-runs `SetNodeRef`
        // on it; then grown, so `Yorder` (2 -> 6) outruns `node_ref` (2).
        "edit line.a enabled=no",
        "edit line.a phases=3",
        "edit line.a bus1=src.1.2.3 bus2=b.1.2.3",
        "solve",
    ] {
        dss.command(cmd);
    }
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let snaps = dss.snapshot_elements();
    let a = elem(&snaps, "Line.a");
    assert!(!a.enabled, "Line.a stays disabled");
    // The two surviving slots keep the mapping they had (both buses' node 1);
    // the four with no `NodeRef` entry read ground, like a `NodeRef` of 0.
    assert_eq!(a.node_order, vec![1, 1, 0, 0, 0, 0]);
}

// Expected-value pin — the three counts are the element's own `NTerms`,
// `NConds` and `NPhases` (r4133 `DDLL/DCktElement.pas:141`/`:146`/`:151`, capi
// `CAPI/CAPI_CktElement.pas:211`/`:189`/`:199`). `NConds` is NOT derivable from
// `NPhases` (a wye load adds a neutral), nor `NTerms` from either — the deck
// below separates all three.
/// `NumTerminals` / `NumConductors` / `NumPhases` are three independent counts.
#[test]
fn num_phases_terminals_conductors_follow_the_element_data() {
    let mut dss = fixture();
    let snaps = dss.snapshot_elements();
    // Two terminals, 3 conductors each, 3 phases (no neutral conductor).
    let l1 = elem(&snaps, "Line.l1");
    assert_eq!((l1.n_terms, l1.n_conds, l1.n_phases), (2, 3, 3));
    // One terminal, 4 conductors, 3 phases: nconds = nphases + neutral.
    let wye = elem(&snaps, "Load.wye");
    assert_eq!((wye.n_terms, wye.n_conds, wye.n_phases), (1, 4, 3));
    // One terminal, 2 conductors, 1 phase.
    let a = elem(&snaps, "Load.a");
    assert_eq!((a.n_terms, a.n_conds, a.n_phases), (1, 2, 1));
    // Zero of everything (see the 0-terminal pin above).
    let uc = elem(&snaps, "UPFCControl.uc");
    assert_eq!((uc.n_terms, uc.n_conds, uc.n_phases), (0, 0, 0));

    // The counts are the shape of every other per-element channel, so they must
    // agree with what the snapshot already exposes: one bus name per terminal,
    // and `nterms · nconds` conductor slots of current.
    for e in &snaps {
        assert_eq!(
            e.bus_names.len(),
            e.n_terms,
            "{}: one bus name per terminal",
            e.name
        );
        assert_eq!(
            e.currents.len(),
            e.n_terms * e.n_conds,
            "{}: yorder conductor slots",
            e.name
        );
    }
}
