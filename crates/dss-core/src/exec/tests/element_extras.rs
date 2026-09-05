//! The per-element **discrete extras** of `GOLDEN_REBASE_PLAN.md` G1.3d — part
//! (i)'s index/name scalars (`CktElement.NumTerminals` / `NumConductors` /
//! `NumPhases`, `CktElement.NodeOrder`, `CktElement.EnergyMeter`) and part
//! (ii)'s control-derived five (`NumControls`, `OCPDevIndex`, `OCPDevType`,
//! `HasVoltControl`, `HasSwitchControl`) plus the one numeric surface
//! `CktElement.PhaseLosses` — as they come out of
//! [`Dss::snapshot_elements`](crate::exec::Dss::snapshot_elements).
//!
//! These are the two sub-steps' expected-value pins. The live corpus gate
//! compares every one of them against both oracle channels, case by case —
//! exactly for the ten discrete fields, on the powers tier for `PhaseLosses`
//! (the only quantity in this file carrying a tolerance at all); what a live
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
//!    including the cases where they differ from each other;
//! 6. `PhaseLosses` is in **W/var** (the ×0.001 to the oracles' kW/kvar is a
//!    capture-boundary encoding), it is `Powers` bucketed by phase, it keeps its
//!    `NPhases` length on a disabled element and it carries the ×3
//!    positive-sequence scaling;
//! 7. the five control-derived scalars read a per-element `ControlElementList`
//!    the port does not store: it is derived from the attach order
//!    ([`crate::circuit::controls`]), no `Enabled` filter anywhere, a re-edited
//!    control moves to the end of its element's list, and only six of the twelve
//!    registered control classes ever enter one.
//!
//! Pascal: r4133 `Version8/Source/DDLL/DCktElement.pas:139`/`:144`/`:149`
//! (`CktElementI` modes `0`/`1`/`2`), `:207-262` (modes `7`-`11`, over
//! `Common/Utilities.pas:3165-3184` `GetOCPDeviceType`), `:442`
//! (`CktElementS` mode `4`), `:637-659` (`CktElementV` mode `6`, over
//! `Common/CktElement.pas:1078-1120` `GetPhaseLosses`), `:1032`
//! (`CktElementV` mode `17`, over `Common/Utilities.pas:1718` `GetNodeNum`);
//! capi `CAPI/CAPI_CktElement.pas:202`/`:182`/`:192`/`:672`/`:689-988`/`:885`
//! and `CAPI/CAPI_Alt.pas:449-467`. All eleven are fastdss
//! `ICktElement._columns` surfaces
//! (`git -C .inputs/DSS-Python show origin/fastdss:dss/ICktElement.py:29-70`);
//! the sibling flag `HasOCPDevice` is listed there too but fastdss's own
//! comparison subtracts it unconditionally
//! (`origin/fastdss:tests/save_outputs.py:169`), so it is not part of the
//! parity target and this port's `HAS_OCP_DEVICE` flag stays an internal
//! reliability input.

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
// `CAPI/CAPI_CktElement.pas:910-917`), i.e. the *bus-local* node number the element's
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
// (`CAPI/CAPI_CktElement.pas:900-906`) while r4133, which has no such guard,
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

// ---------------------------------------------------------------------------
// G1.3d(ii): `PhaseLosses` + the five control-derived scalars
// ---------------------------------------------------------------------------

/// The G1.3d(ii) control fixture. `Line.l1` carries three **heterogeneous**
/// controls in a known attach order; `Line.l2` carries a **disabled** OCP
/// control ahead of an enabled one; a CapControl acts on a capacitor, a
/// RegControl on a transformer, a GenDispatcher on a line it never joins the
/// list of, and `Line.free` carries nothing at all.
fn control_fixture() -> Dss {
    let mut dss = Dss::new();
    for c in [
        "clear",
        "new circuit.ctrlextras basekv=12.47 pu=1.0 phases=3 bus1=src",
        "new line.l1 bus1=src bus2=b2 length=1 r1=0.1 x1=0.3 c1=0",
        "new line.l2 bus1=b2 bus2=b3 length=1 r1=0.1 x1=0.3 c1=0",
        "new line.free bus1=b3 bus2=b4 length=1 r1=0.1 x1=0.3 c1=0",
        "new load.ld bus1=b4 kv=12.47 kw=900 phases=3",
        "new capacitor.c1 bus1=b3 phases=3 kvar=600 kv=12.47",
        "new transformer.t1 phases=3 windings=2 buses=(b2, tb) conns=(wye,wye) \
         kvs=(12.47,4.16) kvas=(1000,1000) xhl=6",
        "new load.tl bus1=tb kv=4.16 kw=200 phases=3",
        // `Line.l1`'s list, in attach order: Relay, Fuse, SwtControl.
        "new relay.r monitoredobj=line.l1 monitoredterm=1",
        "new fuse.f monitoredobj=line.l1 monitoredterm=1",
        "new swtcontrol.s switchedobj=line.l1 switchedterm=1",
        // `Line.l2`: a disabled Fuse ahead of an enabled Relay. The CapControl
        // monitors `Line.l2` but *controls* `Capacitor.c1`, so it joins the
        // capacitor's list, not the line's.
        "new fuse.fd monitoredobj=line.l2 monitoredterm=1 enabled=no",
        "new relay.rd monitoredobj=line.l2 monitoredterm=1",
        "new capcontrol.cc element=line.l2 terminal=1 capacitor=c1 type=voltage \
         ptratio=60 onsetting=115 offsetting=126",
        "new regcontrol.rc transformer=t1 winding=2 vreg=120 band=3 ptratio=60",
        // A control class that never joins any `ControlElementList`: its
        // `ControlledElement` is nil upstream (r4133
        // `Controls/GenDispatcher.pas:290`).
        "new generator.g bus1=b4 kv=12.47 kw=100 phases=3",
        "new gendispatcher.gd element=line.free terminal=1 kwlimit=100 kwband=10 \
         genlist=[g]",
        "set voltagebases=[12.47, 4.16]",
        "calcvoltagebases",
        "solve",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss
}

// Expected-value pin — the engine reports `GetPhaseLosses` in **W/var**, like
// `Get_Losses` and unlike `Powers` (kW/kvar): both oracle surfaces apply the
// ×0.001 at the API boundary (r4133 `DDLL/DCktElement.pas:637-659`, capi
// `CAPI/CAPI_Alt.pas:449-467`), so that scaling is a capture-boundary encoding
// and belongs in the harness comparator, not in the engine. The identities
// below are a real cross-check: `Get_Losses` walks the flat conductor list
// (`Common/CktElement.pas:707-770`) while `GetPhaseLosses` walks phase-major
// with the `k = (j-1)·NConds + i` offset (`:1078-1120`), and `Powers` is a third
// walk again — they must still agree on an element whose `NConds == NPhases`
// (no neutral conductor to drop).
/// `PhaseLosses` is in watts, sums to `Get_Losses` and equals `Powers` bucketed
/// by phase (×1000).
#[test]
fn phase_losses_are_watts_and_sum_to_get_losses() {
    let mut dss = fixture();
    let snaps = dss.snapshot_elements();

    // `Line.l1`: 2 terminals × 3 conductors, `NConds == NPhases`, so every
    // conductor belongs to a phase and the two walks cover the same set.
    let l1 = elem(&snaps, "Line.l1");
    assert_eq!((l1.n_terms, l1.n_conds, l1.n_phases), (2, 3, 3));
    assert_eq!(l1.phase_losses.len(), 3);
    let sum: num_complex::Complex64 = l1.phase_losses.iter().sum();
    let loss = num_complex::Complex64::new(l1.loss_w.0, l1.loss_w.1);
    // Not bit-exact by construction, and the band is DERIVED, not swept. The
    // three walks form the identical per-conductor products `V·conj(I)` and
    // differ only in summation order, so the gap is pure reassociation, bounded
    // by `n·eps·Σ|terms|` ≈ `6 · 2.2e-16 · scale` ≈ `1.3e-15 · scale`. The band
    // is `1e-13 · scale` — ~75× that bound and ~12 orders below the smallest
    // real disagreement (dropping one conductor costs ~`scale/6`). `scale`, not
    // `|loss|`, is the right denominator: a line's per-conductor products are
    // ~±3e5 W and its loss ~30 W, i.e. a 1e-4 cancellation, so the absolute
    // floor is set by the summands, not by the answer. Measured here:
    // |Δ| = 7.275958e-12 W against a permitted ~1.8e-7 W.
    let scale: f64 = l1.powers.iter().map(|p| p.norm() * 1000.0).sum();
    assert!(
        (sum - loss).norm() <= 1e-13 * scale,
        "Line.l1: sum of PhaseLosses {sum} must be Get_Losses {loss}",
    );
    // Watts, not kilowatts: `Powers` is the same product ×0.001.
    let p_kw: num_complex::Complex64 = l1.powers.iter().sum();
    assert!(
        (loss - p_kw * 1000.0).norm() <= 1e-13 * scale,
        "Line.l1: Get_Losses {loss} W must be the summed Powers {p_kw} kW × 1000",
    );
    // …and the per-phase bucketing itself: `PhaseLosses[i]` is the sum over
    // terminals of the same phase's `Powers` slot (the identity measured to hold
    // on the capi channel for every element, `tmp/g13d2/probe_pl_both.py`).
    for (i, pl) in l1.phase_losses.iter().enumerate() {
        let terms = || (0..l1.n_terms).map(move |j| l1.powers[j * l1.n_conds + i] * 1000.0);
        let from_powers: num_complex::Complex64 = terms().sum();
        // Same derivation, scaled to this phase's own two summands.
        let phase_scale: f64 = terms().map(|p| p.norm()).sum();
        assert!(
            (pl - from_powers).norm() <= 1e-13 * phase_scale,
            "Line.l1 phase {i}: PhaseLosses {pl} W vs Powers-bucketed {from_powers} W",
        );
    }

    // On a wye load `NConds = NPhases + 1`: the neutral conductor is ignored by
    // `GetPhaseLosses` ("neutral conductors are ignored by this routine",
    // `Common/CktElement.pas:1080`) but summed by `Get_Losses`. Here the neutral
    // is solidly grounded (`NodeRef = 0`), so both skip it — which is why the
    // length, not the value, is the observable difference.
    let wye = elem(&snaps, "Load.wye");
    assert_eq!((wye.n_conds, wye.n_phases), (4, 3));
    assert_eq!(wye.phase_losses.len(), 3);
}

// Expected-value pin — Pascal sets `Num_Phases := Fnphases` *before* the
// `If FEnabled` test and zero-fills in the `else`
// (`Common/CktElement.pas:1088`, `:1118-1119`), so the reported length never
// depends on the solve state; the port gives the same answer for the
// nil-`NodeRef` state Pascal would dereference.
/// A disabled element reports `NPhases` zeros, not an empty vector.
#[test]
fn phase_losses_of_a_disabled_element_are_zeros_not_empty() {
    let mut dss = fixture();
    let snaps = dss.snapshot_elements();
    let tie = elem(&snaps, "Line.tie");
    assert!(!tie.enabled);
    assert_eq!(tie.n_phases, 3);
    assert_eq!(
        tie.phase_losses,
        vec![num_complex::Complex64::ZERO; 3],
        "a disabled element zero-fills NPhases slots",
    );
    // Its `NodeRef` never ran, so this is also the nil-`NodeRef` shape.
    assert!(tie.node_order.is_empty());
}

// Expected-value pin — a 0-phase element reports an empty array on both
// channels: r4133's `CktElementV(6)` answers `n = 0` (measured — the DLL
// survives it, so no do-not-call rule is owed) and capi allocates
// `2·NPhases = 0` doubles (`CAPI/CAPI_Alt.pas:455`).
/// `UPFCControl` — 0 terminals, 0 conductors, 0 phases (r4133
/// `Controls/UPFCControl.pas:230-246` never assigns `Nterms`) — reports no
/// phase losses at all.
#[test]
fn phase_losses_of_a_zero_phase_element_are_empty() {
    let mut dss = fixture();
    let snaps = dss.snapshot_elements();
    let uc = elem(&snaps, "UPFCControl.uc");
    assert_eq!((uc.n_terms, uc.n_conds, uc.n_phases), (0, 0, 0));
    assert!(
        uc.phase_losses.is_empty(),
        "a 0-phase element has no phase losses: {:?}",
        uc.phase_losses,
    );
}

// Expected-value pin — `PositiveSequence` multiplies every accumulated term by
// 3 (`Common/CktElement.pas:1102-1104`), the balanced-three-phase scaling
// `Get_Losses`/`Get_Powers` also apply. Proven against the element's OWN
// reported voltage and current rather than against a second solve: the raw
// per-phase `V·conj(I)` sum is exactly a third of what `PhaseLosses` reports.
/// Under `CktModel=Positive` the phase losses are 3 × the raw `V·conj(I)`.
#[test]
fn phase_losses_scale_by_three_under_positive_sequence() {
    let mut dss = Dss::new();
    for c in [
        "clear",
        "new circuit.pos basekv=12.47 pu=1.0 phases=3 bus1=src",
        "set cktmodel=positive",
        "new line.l1 bus1=src bus2=b2 length=1 r1=0.1 x1=0.3 c1=0",
        "new load.ld bus1=b2 kv=12.47 kw=900 phases=3",
        "set voltagebases=[12.47]",
        "calcvoltagebases",
        "solve",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let snaps = dss.snapshot_elements();
    let l1 = elem(&snaps, "Line.l1");
    assert!(!l1.phase_losses.is_empty());
    for (i, pl) in l1.phase_losses.iter().enumerate() {
        // Rebuild V from the element's own `VoltagesMagAng` (the polar
        // round-trip is what the 1e-9 band pays for) and pair it with the same
        // conductor's current.
        let raw: num_complex::Complex64 = (0..l1.n_terms)
            .map(|j| {
                let k = j * l1.n_conds + i;
                let v = l1.voltages_mag_ang[k];
                num_complex::Complex64::from_polar(v.mag, v.ang.to_radians())
            })
            .zip((0..l1.n_terms).map(|j| l1.currents[j * l1.n_conds + i]))
            .map(|(v, c)| v * c.conj())
            .sum();
        assert!(
            (pl - raw * 3.0).norm() <= 1e-9 * pl.norm().max(1.0),
            "phase {i}: PhaseLosses {pl} must be 3 × the raw sum {raw}",
        );
    }
}

// Expected-value pin — neither channel filters `ControlElementList` by
// `Enabled`: `NumControls` is the bare `ListSize` (r4133
// `DDLL/DCktElement.pas:237-241`, capi `CAPI/CAPI_CktElement.pas:939-948`).
// Measured on both oracles: `Line.l2` reports 2 with one of its two OCP
// controls disabled.
/// A disabled control still occupies its slot in the list.
#[test]
fn num_controls_counts_disabled_controls_too() {
    let mut dss = control_fixture();
    let snaps = dss.snapshot_elements();
    assert!(!elem(&snaps, "Fuse.fd").enabled, "Fuse.fd is disabled");
    assert_eq!(elem(&snaps, "Line.l2").num_controls, 2);
    assert_eq!(elem(&snaps, "Line.l1").num_controls, 3);
    // A control acting on nothing, and an element under no control.
    assert_eq!(elem(&snaps, "Line.free").num_controls, 0);
    assert_eq!(elem(&snaps, "Relay.r").num_controls, 0);
}

// Expected-value pin — `GetOCPDeviceType`'s scan (r4133
// `Common/Utilities.pas:3165-3184`) stops at the first Fuse/Recloser/Relay in
// list order and has NO `Enabled` test, so a disabled fuse ahead of an enabled
// relay wins. Measured identically on both channels (`Line.l2` → 1). The port's
// registration-time latch `CktElementData::ocp_device_type` — which the
// reliability sweep reads — answers 3 there, because its emitter is
// `Enabled`-guarded; this surface must therefore recompute, never read it.
/// A disabled OCP control still wins `OCPDevType`: both oracles say `1` (Fuse)
/// where the latch says `3` (Relay).
#[test]
fn a_disabled_ocp_control_still_wins_the_ocp_scan() {
    let mut dss = control_fixture();
    let snaps = dss.snapshot_elements();
    let l2 = elem(&snaps, "Line.l2");
    assert_eq!(
        (l2.ocp_dev_index, l2.ocp_dev_type),
        (1, 1),
        "the disabled Fuse.fd is list member 1 and is a FUSE_CONTROL",
    );
    // The latch the reliability sweep reads disagrees — deliberately: it is
    // written once, by the first *enabled* OCP control to register (the
    // `SetOcpDevice` ref-action, `exec/command.rs`), which here is `Relay.rd`.
    let l2_id = *dss
        .circuit()
        .expect("solved circuit")
        .pd_elements
        .iter()
        .find(|&&r| {
            dss.classes[r.class_ord()].arena[r.index()]
                .data()
                .name()
                .eq_ignore_ascii_case("l2")
        })
        .expect("Line.l2 is a PD element");
    let latched = dss.classes[l2_id.class_ord()]
        .arena
        .try_ckt_elem(l2_id.index())
        .expect("Line.l2 is a circuit element")
        .cd()
        .ocp_device_type;
    assert_eq!(
        latched,
        crate::elements::ckt::OcpDeviceType::Relay,
        "the enabled-only latch answers Relay(3) where the live scan answers Fuse(1)",
    );
}

// Expected-value pin — `TControlElem.Set_ControlledElement` is remove-then-add
// (r4133 `Controls/ControlElem.pas:113-131`) and r4133 re-runs it inside
// `RecalcElementData`, i.e. on EVERY edit (`Controls/Relay.pas:955` from
// `:626`), so re-editing a control moves it to the END of its element's list.
// capi 0.14.5 makes `ControlledElement` a property-write target
// (`Controls/Relay.pas:439-441`) and leaves the order alone. r4133 is the
// behavioral authority (`DIVERGENCES.md`); measured on the two live channels as
// `OCPDevType` 3 → 1 on r4133 and a flat 3 on capi.
/// Re-editing `Relay.r` moves it behind `Fuse.f`, so `OCPDevType` goes 3 → 1
/// (r4133's answer; capi 0.14.5 would still say 3).
#[test]
fn ocp_dev_type_follows_the_last_attach_order() {
    let mut dss = control_fixture();
    let before = dss.snapshot_elements();
    let l1 = elem(&before, "Line.l1");
    assert_eq!(
        (l1.num_controls, l1.ocp_dev_index, l1.ocp_dev_type),
        (3, 1, 3),
        "attach order Relay, Fuse, SwtControl",
    );

    // A re-edit that does not touch the element reference at all. No re-solve:
    // the list order is rebuilt by the control's `RecalcElementData`, i.e. by
    // the edit itself, and the five scalars are structural.
    dss.command("edit relay.r delay=0.05");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let after = dss.snapshot_elements();
    let l1 = elem(&after, "Line.l1");
    assert_eq!(
        (l1.num_controls, l1.ocp_dev_index, l1.ocp_dev_type),
        (3, 1, 1),
        "the relay re-appended at the end leaves Fuse.f first: r4133 says 1, \
         capi 0.14.5 says 3",
    );
}

// Expected-value pin (G1.3d(ii) audit settlement, 2026-09-05) — the converse of
// the pin above: `MakePosSequence` is NOT an edit. r4133's `DoMakePosSeq` calls
// only `CktElem.MakePosSequence` (`Executive/ExecHelper.pas:3069-3086`), and
// every control override mutates its own fields and ends with `inherited` — the
// base bus rename, `Common/CktElement.pas:1352` (`Controls/Relay.pas:1008`,
// `Recloser.pas:738`, `SwtControl.pas:367`, `CapControl.pas:656`,
// `RegControl.pas:1491`; `Fuse` has no override). None reaches
// `RecalcElementData`, so no `Set_ControlledElement` runs and no
// `ControlElementList` is reordered. The port keeps the re-attach outside the
// shared post-edit tail for exactly that reason
// (`exec/command.rs::reattach_edited_control`).
/// `MakePosSeq` leaves the control lists alone: after the re-edit above,
/// `Line.l1` still answers `OCPDevType` 1 (Fuse first), not the 3 a re-attach in
/// creation order would give back.
#[test]
fn makeposseq_does_not_reattach_controls() {
    let mut dss = control_fixture();
    dss.command("edit relay.r delay=0.05");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let snaps = dss.snapshot_elements();
    let l1 = elem(&snaps, "Line.l1");
    assert_eq!(
        (l1.num_controls, l1.ocp_dev_index, l1.ocp_dev_type),
        (3, 1, 1),
        "precondition: the re-edited relay sits at the end of the list, so the \
         Fuse is the first OCP member",
    );
    let l1_id = *dss
        .circuit()
        .expect("circuit")
        .pd_elements
        .iter()
        .find(|&&r| {
            dss.classes[r.class_ord()].arena[r.index()]
                .data()
                .name()
                .eq_ignore_ascii_case("l1")
        })
        .expect("Line.l1 is a PD element");
    let attach_before = dss.circuit().expect("circuit").control_attach_order.clone();
    let list_before = crate::circuit::controls::derive_control_lists(
        &dss.classes,
        dss.circuit().expect("circuit"),
    )
    .remove(&l1_id)
    .expect("Line.l1 carries three controls");

    dss.command("makeposseq");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    let ckt = dss.circuit().expect("circuit");
    assert_eq!(
        ckt.control_attach_order, attach_before,
        "MakePosSequence must not re-attach: a post-edit tail that ran the \
         control's `Set_ControlledElement` for every element would rewrite the \
         attach order into `CktElements` (creation) order",
    );
    let list_after = crate::circuit::controls::derive_control_lists(&dss.classes, ckt)
        .remove(&l1_id)
        .expect("Line.l1 still carries three controls");
    assert_eq!(
        list_after, list_before,
        "Line.l1's ControlElementList order survives MakePosSeq — the Fuse stays \
         first, so `OCPDevType` stays 1 (r4133's answer); a re-attach would put \
         the Relay back in front and answer 3",
    );
}

// Expected-value pin — `OCPDevIndex` is the **1-based** position of the first
// OCP member and `0` when there is none, from the identical
// `repeat … until (i > listSize) or (Result > 0)` on both channels (r4133
// `DDLL/DCktElement.pas:242-258`, capi `CAPI/CAPI_CktElement.pas:951-976`); the
// two OCP scalars are one scan, so they are zero together.
/// `OCPDevIndex` counts from 1 and is 0 exactly when `OCPDevType` is 0.
#[test]
fn ocp_dev_index_is_one_based_and_zero_when_there_is_none() {
    let mut dss = control_fixture();
    let snaps = dss.snapshot_elements();
    // `Capacitor.c1` carries exactly one control, a CapControl: no OCP device.
    let c1 = elem(&snaps, "Capacitor.c1");
    assert_eq!(
        (c1.num_controls, c1.ocp_dev_index, c1.ocp_dev_type),
        (1, 0, 0)
    );
    // `Line.l1`'s first member is the Relay → index 1, type 3.
    let l1 = elem(&snaps, "Line.l1");
    assert_eq!((l1.ocp_dev_index, l1.ocp_dev_type), (1, 3));
    // The two are zero together on every element, and the index is inside the
    // list — the mechanism, asserted over the whole snapshot.
    for e in &snaps {
        assert_eq!(
            e.ocp_dev_index == 0,
            e.ocp_dev_type == 0,
            "{}: OCPDevIndex and OCPDevType come from one scan",
            e.name,
        );
        assert!(
            e.ocp_dev_index <= e.num_controls,
            "{}: OCPDevIndex {} outside a list of {}",
            e.name,
            e.ocp_dev_index,
            e.num_controls,
        );
    }
}

// Expected-value pin — `HasVoltControl` is "any member is a `CAP_CONTROL` or a
// `REG_CONTROL`" and `HasSwitchControl` "any member is a `SWT_CONTROL`" (r4133
// `DDLL/DCktElement.pas:207-236`, capi `CAPI/CAPI_CktElement.pas:689-734`),
// over the CLASSMASK ordinals of `Common/DSSClassDefs.pas:43-48`, `:55`. The
// classes are disjoint, so an element can raise one, the other or neither —
// all three appear below.
/// The two `Has*Control` flags answer for their own three classes only.
#[test]
fn has_volt_control_is_capcontrol_or_regcontrol_and_has_switch_control_is_swtcontrol() {
    let mut dss = control_fixture();
    let snaps = dss.snapshot_elements();
    // A SwtControl among OCP devices: switch yes, volt no.
    let l1 = elem(&snaps, "Line.l1");
    assert_eq!((l1.has_volt_control, l1.has_switch_control), (false, true));
    // CapControl → volt only. RegControl → volt only.
    let c1 = elem(&snaps, "Capacitor.c1");
    assert_eq!((c1.has_volt_control, c1.has_switch_control), (true, false));
    let t1 = elem(&snaps, "Transformer.t1");
    assert_eq!((t1.num_controls, t1.has_volt_control), (1, true));
    // Two OCP controls and nothing else: neither flag.
    let l2 = elem(&snaps, "Line.l2");
    assert_eq!((l2.has_volt_control, l2.has_switch_control), (false, false));
    // No controls at all.
    let free = elem(&snaps, "Line.free");
    assert_eq!(
        (free.has_volt_control, free.has_switch_control),
        (false, false)
    );
}

// Expected-value pin (totality guard) — only six of the twelve registered
// control classes ever join a `ControlElementList`; the other six keep their
// controlled element in a private field, a list or an array and never call
// `Set_ControlledElement` (r4133 `Controls/GenDispatcher.pas:290`,
// `StorageController.pas:828`, `ESPVLControl.pas:362`, `InvControl.pas:74`,
// `ExpControl.pas:33`, and `UPFCControl.pas`, which never assigns it). A
// thirteenth control class would fall silently into `ControlCategory::Other`;
// this is what makes that a decision instead of an accident.
/// The registered control classes are the known twelve, and exactly six of them
/// map to a `CLASSMASK` category the five scalars answer for.
#[test]
fn only_six_control_classes_join_an_elements_control_list() {
    use crate::circuit::ElemKind;
    use crate::circuit::controls::{ControlCategory, control_category};

    let dss = Dss::new();
    let mut registered: Vec<&str> = dss
        .classes
        .iter()
        .filter(|c| c.kind == Some(ElemKind::Control))
        .map(|c| c.props.class_name())
        .collect();
    registered.sort_unstable();
    assert_eq!(
        registered,
        [
            "CapControl",
            "ESPVLControl",
            "ExpControl",
            "Fuse",
            "GenDispatcher",
            "InvControl",
            "Recloser",
            "RegControl",
            "Relay",
            "StorageController",
            "SwtControl",
            "UPFCControl",
        ],
        "a new control class must be classified in `circuit::controls`",
    );

    // …and the six that carry a CLASSMASK the scalars ask about, in
    // registration order.
    let listed: Vec<&str> = dss
        .classes
        .iter()
        .enumerate()
        .filter(|(ord, c)| {
            c.kind == Some(ElemKind::Control)
                && control_category(dss.classes[*ord].arena.id(0)) != ControlCategory::Other
        })
        .map(|(_, c)| c.props.class_name())
        .collect();
    assert_eq!(
        listed,
        vec![
            "RegControl",
            "CapControl",
            "Relay",
            "Recloser",
            "Fuse",
            "SwtControl"
        ],
        "exactly the CAP/REG/RELAY/RECLOSER/FUSE/SWT control classes",
    );

    // The behavioral half, on the fixture: the GenDispatcher targets
    // `Line.free` and still leaves it with an empty list.
    let mut dss = control_fixture();
    let snaps = dss.snapshot_elements();
    assert_eq!(elem(&snaps, "Line.free").num_controls, 0);
}

// Expected-value pin — `Set_ControlledElement` maintains the per-element
// `ControlElementList` only; `ActiveCircuit.Controls`, the list
// `Sample_DoControlActions` walks, is appended to once at creation and never
// reordered. Reordering it would change the control-action order and therefore
// the event log, so the port keeps the two lists separate:
// `Circuit::controls` (sampling) vs `Circuit::control_attach_order` (list order).
/// A re-edit moves the control in the attach order and leaves the sampling
/// order alone.
#[test]
fn the_control_sampling_order_is_not_reordered_by_a_re_edit() {
    let mut dss = control_fixture();
    let sampling: Vec<crate::elements::traits::ElemId> =
        dss.circuit().expect("circuit").controls.clone();
    let attach_before: Vec<crate::elements::traits::ElemId> =
        dss.circuit().expect("circuit").control_attach_order.clone();

    dss.command("edit relay.r delay=0.05");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    let ckt = dss.circuit().expect("circuit");
    assert_eq!(
        ckt.controls, sampling,
        "the sampling order must survive a control edit untouched",
    );
    assert_ne!(
        ckt.control_attach_order, attach_before,
        "the re-edited relay must move to the end of the attach order",
    );
    // Precisely: the relay left its slot and re-appended; everything else held.
    let relay = attach_before[0];
    let mut expected: Vec<_> = attach_before
        .iter()
        .copied()
        .filter(|&c| c != relay)
        .collect();
    expected.push(relay);
    assert_eq!(ckt.control_attach_order, expected);
    // The GenDispatcher never entered the attach order at all (no controlled
    // element), so the two lists differ in length as well as in order.
    assert_eq!(sampling.len(), ckt.control_attach_order.len() + 1);
}
