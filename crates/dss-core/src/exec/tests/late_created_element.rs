//! Reporting on a circuit element declared **after** the last solve
//! (GOLDEN_REBASE G1.10b micro-part F2b, from the F0 side observation;
//! coordinator decision D43(4)).
//!
//! `NodeRef` is the element's map from its own conductors to the solution's node
//! vector. r4133 leaves it `nil` in the constructor
//! (`Common/CktElement.pas:186`) and allocates it only in `SetNodeRef`
//! (`:547-558`), which `ReProcessBusDefs` re-runs — for **enabled** elements —
//! while the Y matrix is built; the port models that with an empty `node_ref`
//! (`elements/ckt.rs::set_node_ref` resizes it to `yorder`). So a `New` typed
//! after a `Solve` leaves the element unmapped until the next solve, and every
//! report that reads terminal currents reaches it in that state
//! (`Export Currents`, `Show Currents`, …).
//!
//! r4133 walks into the nil pointer there: `TStorageObj.GetTerminalCurrents`
//! (`PCElements/Storage.pas:2861-2871`) recomputes the model, which indexes
//! `NodeV^[NodeRef^[i]]` (`CalcVTerminalPhase`), and the resulting access
//! violation is caught by `TPCElement.GetCurrents`' `TRY … EXCEPT`
//! (`PCElements/PCElement.pas:277`, `:304-306`), which reports DSS error 641
//! ("Inadequate storage allotted for circuit element") instead of a current.
//! The port has no access violation to catch, and `#![forbid(unsafe_code)]`
//! turns the same read into a panic — which is what nine `get_currents`
//! overrides did before F2b (`index out of bounds: the len is 0`, e.g.
//! `elements/pc/storage/solve.rs`' `CalcVTerminalPhase`). They now answer with
//! the zero vector the base trait's `get_currents` default
//! (`elements/traits.rs`) and Pascal's own `not Enabled` arm
//! (`PCElement.pas:298-300`) return: an element with no node references carries
//! no terminal current.

use crate::exec::Dss;

/// A scratch output directory for one export, removed by the caller.
fn scratch(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("dss_late_elem_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

/// A solved 3-phase feeder: `source — Line.l1 — bus b` with a 100 kW load.
/// Everything below declares its element on `b` *after* this solve.
fn solved_feeder(dir: &std::path::Path) -> Dss {
    let mut dss = Dss::new();
    dss.command(&format!("set datapath=\"{}\"", dir.display()));
    dss.command("New circuit.t basekv=12.47 phases=3 bus1=a pu=1");
    dss.command("New Line.l1 bus1=a bus2=b phases=3 r1=0.1 x1=0.2 length=1");
    dss.command("New Load.ld1 bus1=b kv=12.47 phases=3 kw=100 pf=1");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss
}

/// The numeric fields of `<element>`'s row in a fresh `Export Currents`.
fn exported_currents(dss: &mut Dss, dir: &std::path::Path, elem: &str) -> Vec<String> {
    let file = dir.join("t_EXP_CURRENTS.csv");
    let _ = std::fs::remove_file(&file);
    dss.command("export currents");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let text = std::fs::read_to_string(&file).expect("Export Currents wrote its file");
    let row = text
        .lines()
        .find(|l| {
            l.split(',')
                .next()
                .is_some_and(|n| n.trim().eq_ignore_ascii_case(elem))
        })
        .unwrap_or_else(|| panic!("{elem} has a row in\n{text}"));
    row.split(',')
        .skip(1)
        .map(|f| f.trim().to_string())
        .collect()
}

/// The nine PC classes whose `get_currents` override indexes `node_ref` without
/// the base trait's guard — each measured to panic before F2b — with the
/// declaration that puts one on the solved feeder's bus `b`.
const LATE_ELEMENTS: [(&str, &str); 9] = [
    (
        "Storage.s1",
        "New Storage.s1 bus1=b kv=12.47 phases=3 kWrated=50 kWhrated=500",
    ),
    (
        "Load.ld2",
        "New Load.ld2 bus1=b kv=12.47 phases=3 kw=10 pf=1",
    ),
    (
        "Generator.g1",
        "New Generator.g1 bus1=b kv=12.47 phases=3 kw=10 pf=1",
    ),
    (
        "PVSystem.pv1",
        "New PVSystem.pv1 bus1=b kv=12.47 phases=3 Pmpp=10 irradiance=1 kVA=10",
    ),
    (
        "WindGen.w1",
        "New WindGen.w1 bus1=b kv=12.47 phases=3 kw=10",
    ),
    ("GICLine.gl1", "New GICLine.gl1 bus1=b bus2=c volts=100"),
    (
        "GICsource.gs1",
        "New Line.gs1 bus1=b bus2=c phases=3 r1=0.1 x1=0.2 length=1 | New GICsource.gs1 volts=100 angle=0",
    ),
    ("Vsource.v2", "New Vsource.v2 bus1=b basekv=12.47 phases=3"),
    (
        "VSConverter.vsc1",
        "New VSConverter.vsc1 bus1=b phases=3 kvac=12.47 kvdc=12.47 kw=10",
    ),
];

/// **Pin (D43(4)).** A report that reaches an element declared after the last
/// solve reads the base trait's result — every terminal current `0`, on all
/// nine classes whose override used to index the empty `node_ref` and panic
/// (`index out of bounds: the len is 0`). r4133 answers the same read with DSS
/// error 641 out of its access-violation handler (`PCElement.pas:304-306`).
#[test]
fn a_report_on_an_element_created_after_the_last_solve_reads_as_ground() {
    for (elem, decl) in LATE_ELEMENTS {
        let dir = scratch(&elem.replace('.', "_"));
        let mut dss = solved_feeder(&dir);
        for cmd in decl.split('|') {
            dss.command(cmd.trim());
        }
        assert!(dss.errors().is_empty(), "{elem}: {:?}", dss.errors());
        let fields = exported_currents(&mut dss, &dir, elem);
        assert!(
            !fields.is_empty(),
            "{elem}: the export row carries the terminal columns"
        );
        assert!(
            fields.iter().all(|f| f.parse::<f64>() == Ok(0.0)),
            "{elem}: every terminal current of an unmapped element is zero, got {fields:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// **Pin (D43(4), the both-numbers half).** The zero above is the *unmapped*
/// state, not a silenced element: the very same `Storage.s1`, exported again
/// after the `Solve` that maps it, reports `2.31504 A` at `179.99°` on phase 1
/// (50 kW discharging at 12.47 kV, `pf = 1`) where the report taken before that
/// solve read `0 A` at `0.00°`.
#[test]
fn the_late_element_reports_its_current_once_the_next_solve_maps_it() {
    let dir = scratch("resolved");
    let mut dss = solved_feeder(&dir);
    dss.command(
        "New Storage.s1 bus1=b kv=12.47 phases=3 kWrated=50 kWhrated=500 \
         %stored=100 state=discharging %discharge=100",
    );
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    let before = exported_currents(&mut dss, &dir, "Storage.s1");
    assert_eq!(
        (before[0].as_str(), before[1].as_str()),
        ("0", "0.00"),
        "unmapped: |I1_1|, Ang1_1"
    );

    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let after = exported_currents(&mut dss, &dir, "Storage.s1");
    assert_eq!(
        (after[0].as_str(), after[1].as_str()),
        ("2.31504", "179.99"),
        "mapped by the solve: |I1_1|, Ang1_1"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
