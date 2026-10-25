//! Elements across a bus rebuild: the node map a disabled element keeps or
//! drops, `like=`, the fault MonteFault picks, and the step power flow after a
//! bus change.
//!
//! A bus rebuild (`Solve`, `Calcvoltagebases`, `MakeBusList`, `Show Isolated`,
//! a dynamics step) maps every enabled element on the bus list it builds. A
//! disabled element keeps its node map when that list gives its buses the nodes
//! the map names, and drops it when it is enabled again otherwise, so until the
//! next rebuild it reads as one created after the last: it carries no current,
//! and each reader and sampler reads it at node 0 or leaves it out as a disabled
//! one. So does an element re-phased or re-connected since its map was built. A
//! kept map is no membership of the last power flow
//! (`a_machine_enabled_again_on_its_nodes_is_outside_the_run`).
//!
//! Deck and conventions as in the parent module. The cross-check literals were
//! measured on 2026-10-23 on the r4133 DLL through `epri-worker` and on the
//! pinned dss-python with the same command lists, each case in a fresh process.

use num_complex::Complex64;

use super::joining::{
    assert_refused, dropped_bus, join_msg, node_magnitude, outside_msg, renumbered_msg,
};
use super::*;
use crate::elements::ckt::{CktElementData, DynamicsSeed};
use crate::elements::traits::CktElement;

/// A meter on `Line.l1`, so that `Export BranchReliability` has a zone.
const METER: &str = "New EnergyMeter.m1 element=Line.l1 terminal=1";
/// A second line from the source bus to `wbus`, which adds no bus.
const L2_PAR: &str = "New Line.l2 bus1=srcbus bus2=wbus phases=3 r1=0.005 x1=0.02 length=1";
/// A line from `wbus` to `gbus`, a bus nothing else holds.
const L2_GBUS: &str = "New Line.l2 bus1=wbus bus2=gbus phases=3 r1=0.005 x1=0.02 length=1";
const G2: &str = "New Generator.g2 bus1=wbus phases=3 kv=0.69 kW=100 kva=120 model=1";
const FLT: &str = "New Fault.f1 bus1=wbus phases=1 r=0.01";

/// The readers that read an element through its node map or its terminal buses.
const MAP_READERS: [&str; 10] = [
    "Export SeqPowers",
    "Export VoltagesElements",
    "Export NodeOrder",
    "Export Capacity",
    "Export CIM100",
    "Export BranchReliability",
    "Show Powers",
    "Show Mismatch",
    "Show deltaV",
    "Show kVBaseMismatch",
];

/// The mode entries a reader runs after: snap, then the three dynamics-model
/// modes.
const ENTRIES: [&str; 4] = [
    "",
    "Set mode=dynamic stepsize=0.001 number=5",
    "Set mode=faultstudy",
    "Set mode=mf number=2",
];

/// `before` and then `entry` (none for snap).
fn with_entry(before: &[String], entry: &str) -> Vec<String> {
    let mut all = before.to_vec();
    all.extend((!entry.is_empty()).then(|| entry.to_string()));
    all
}

/// `before` and then the lines of `tail`.
fn then(before: &[String], tail: &[&str]) -> Vec<String> {
    let mut all = before.to_vec();
    all.extend(tail.iter().map(|s| s.to_string()));
    all
}

/// The report `cmd` writes (an `Export` names its file in `last_result_file`, a
/// `Show` in `@lastshowfile`), with the CIM ids (drawn at random) read as `_`
/// and its lines sorted, so two runs compare by content.
fn report(dss: &mut Dss, cmd: &str) -> String {
    dss.last_result_file.clear();
    dss.vars.add("@lastshowfile", "");
    run(dss, &[cmd.to_string()]);
    let path = if cmd.starts_with("Show") {
        dss.vars
            .get("@lastshowfile")
            .unwrap_or_default()
            .to_string()
    } else {
        dss.last_result_file().to_string()
    };
    assert!(!path.is_empty(), "{cmd}: no report file");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{cmd}: {path}: {e}"));
    let ids = regex::Regex::new(
        r"[0-9A-Fa-f]{8}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{12}",
    )
    .expect("id pattern");
    let text = ids.replace_all(&text, "_");
    let mut rows: Vec<&str> = text.lines().collect();
    rows.sort_unstable();
    rows.join("\n")
}

/// The circuit-element data of `name` (`Class.name`, any case).
pub(super) fn elem_data<'a>(dss: &'a Dss, name: &str) -> &'a CktElementData {
    ckt_elem(dss, name).cd()
}

/// The circuit element `name` (`Class.name`, any case).
fn ckt_elem<'a>(dss: &'a Dss, name: &str) -> &'a dyn CktElement {
    for class in &dss.classes {
        let cn = class.props.class_name();
        for oi in 0..class.arena.len() {
            if let Some(e) = class.arena.try_ckt_elem(oi)
                && format!("{cn}.{}", e.cd().obj.name()).eq_ignore_ascii_case(name)
            {
                return e;
            }
        }
    }
    panic!("{name} not found")
}

/// The lines of `report` that start with the quoted or bare `name`, any case.
fn rows_of<'a>(report: &'a str, name: &str) -> Vec<&'a str> {
    let name = name.to_ascii_uppercase();
    report
        .lines()
        .filter(|l| {
            let up = l.trim_start().to_ascii_uppercase();
            up.starts_with(&format!("\"{name}\"")) || up.starts_with(&format!("{name},"))
        })
        .collect()
}

/// The numbers of `row` after its first `skip` fields, split at commas and
/// blanks.
fn numbers(row: &str, skip: usize) -> Vec<f64> {
    row.split(|c: char| c == ',' || c.is_whitespace())
        .filter(|f| !f.is_empty())
        .skip(skip)
        .map(|f| f.parse().unwrap_or_else(|_| panic!("{f} in {row}")))
        .collect()
}

/// The `Export VoltagesElements` row of an element without a node map: no bus
/// and no base on any terminal, and each conductor at 0 kV, 0 degrees and 0 pu,
/// zero-filled to the `max_nodes` groups of the header.
fn unmapped_voltages_row(name: &str, cd: &CktElementData, max_nodes: usize) -> String {
    let mut row = format!("{name},{}", cd.nterms);
    let mut k = 0;
    for t in 1..=cd.nterms {
        row.push_str(&format!(",{t},{},{},,", cd.nconds, cd.nphases));
        for _ in 0..cd.nconds {
            k += 1;
            row.push_str(&format!(", {k}, 0, 0.000, 0"));
        }
        for _ in cd.nconds..max_nodes {
            row.push_str(", 0, 0, 0, 0");
        }
    }
    row
}

/// The numbers of the last data row of `Export Monitors {mon}`, after its two
/// time columns.
fn last_monitor_row(dss: &mut Dss, mon: &str) -> Vec<f64> {
    dss.last_result_file.clear();
    run(dss, &[format!("Export Monitors {mon}")]);
    let path = dss.last_result_file().to_string();
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let last = text.lines().last().expect("a monitor row");
    numbers(last, 2)
}

/// The `Export Powers` rows of `name` read zero power on every terminal.
fn assert_zero_powers(dss: &mut Dss, case: &str, name: &str) {
    let text = report(dss, "Export Powers");
    let rows = rows_of(&text, name);
    assert!(
        !rows.is_empty(),
        "{case}: no Export Powers row of {name}\n{text}"
    );
    for row in rows {
        assert!(numbers(row, 2).iter().all(|&x| x == 0.0), "{case}: {row}");
    }
}

/// A one-phase Generator, the machine the re-shaped forms re-phase.
const GEN1: &str = "New Generator.e1 bus1=wbus.1 phases=1 kv=0.398 kW=500 kva=600 model=1";
/// A two-phase delta Generator, whose three conductors a three-phase delta
/// machine also has.
const G2D: &str = "New Generator.e1 bus1=wbus phases=2 conn=delta kv=0.69 kW=1000 kva=1200 model=1";
/// A two-phase wye Generator, three conductors as well.
const G2W: &str = "New Generator.e1 bus1=wbus phases=2 conn=wye kv=0.69 kW=1000 kva=1200 model=1";

/// An element re-phased or reconnected through `phases=`, `conn=` or `like=` after the rebuild
/// that built or kept its map loses the map at the edit or at its `Enable`, whether its conductor
/// count changed or not. Until the next rebuild every reader and `Sample` read it as zero.
#[test]
fn readers_of_an_element_reshaped_since_its_map() {
    let dir = scratch("reshaped_readers");
    let mon = |elem: &str| format!("New Monitor.mg element={elem} terminal=1 mode=0");
    let gen_mon = mon("Generator.e1");
    let load_mon = mon("Load.l2");
    let line_mon = mon("Line.l2");
    let cap_mon = mon("Capacitor.c2");
    let storage_mon = mon("Storage.s2");
    let windgen_mon = mon("WindGen.w2");
    let rephase = "Edit Generator.e1 phases=3 bus1=wbus kv=0.69";
    let forms: [(&str, &str, Vec<String>); 15] = [
        (
            "Generator re-phased while disabled",
            "Generator.e1",
            lines(&[
                &H,
                &[GEN1, gen_mon.as_str()],
                &PF,
                &[
                    "Disable Generator.e1",
                    "Solve",
                    rephase,
                    "Enable Generator.e1",
                ],
            ]),
        ),
        (
            "Generator re-phased while disabled, enabled with its class",
            "Generator.e1",
            lines(&[
                &H,
                &[GEN1, gen_mon.as_str()],
                &PF,
                &[
                    "Disable Generator.e1",
                    "Solve",
                    rephase,
                    "Enable Generator.*",
                ],
            ]),
        ),
        (
            "Load re-connected while disabled",
            "Load.l2",
            lines(&[
                &H,
                &[
                    "New Load.l2 bus1=wbus phases=3 kv=0.69 kW=50 kvar=10 conn=delta",
                    load_mon.as_str(),
                ],
                &PF,
                &[
                    "Disable Load.l2",
                    "Solve",
                    "Edit Load.l2 conn=wye",
                    "Enable Load.l2",
                ],
            ]),
        ),
        (
            "Line re-phased while disabled",
            "Line.l2",
            lines(&[
                &H,
                &[
                    "New Line.l2 bus1=srcbus.1 bus2=wbus.1 phases=1 r1=0.005 x1=0.02 length=1",
                    line_mon.as_str(),
                ],
                &PF,
                &[
                    "Disable Line.l2",
                    "Solve",
                    "Edit Line.l2 phases=3 bus1=srcbus bus2=wbus",
                    "Enable Line.l2",
                ],
            ]),
        ),
        (
            "like= of three phases onto a disabled one-phase Generator",
            "Generator.e1",
            lines(&[
                &H,
                &[GEN1, G2, gen_mon.as_str()],
                &PF,
                &["Disable Generator.e1", "Solve", "Edit Generator.e1 like=g2"],
            ]),
        ),
        (
            "like= of three phases onto an enabled one-phase Generator",
            "Generator.e1",
            lines(&[
                &H,
                &[GEN1, G2, gen_mon.as_str()],
                &PF,
                &["Edit Generator.e1 like=g2"],
            ]),
        ),
        (
            "phases=3 on an enabled one-phase Generator",
            "Generator.e1",
            lines(&[&H, &[GEN1, gen_mon.as_str()], &PF, &[rephase]]),
        ),
        (
            "phases=3 on an enabled two-phase delta Generator",
            "Generator.e1",
            lines(&[
                &H,
                &[G2D, gen_mon.as_str()],
                &PF,
                &["Edit Generator.e1 phases=3"],
            ]),
        ),
        (
            "phases=3 on a disabled two-phase delta Generator, enabled",
            "Generator.e1",
            lines(&[
                &H,
                &[G2D, gen_mon.as_str()],
                &PF,
                &[
                    "Disable Generator.e1",
                    "Solve",
                    "Edit Generator.e1 phases=3",
                    "Enable Generator.e1",
                ],
            ]),
        ),
        (
            "conn=delta on an enabled one-phase Generator",
            "Generator.e1",
            lines(&[
                &H,
                &[GEN1, gen_mon.as_str()],
                &PF,
                &["Edit Generator.e1 conn=delta"],
            ]),
        ),
        (
            "like= of a three-phase wye Generator onto an enabled two-phase wye one",
            "Generator.e1",
            lines(&[
                &H,
                &[G2W, G2, gen_mon.as_str()],
                &PF,
                &["Edit Generator.e1 like=g2"],
            ]),
        ),
        (
            "conn=delta on an enabled wye Capacitor",
            "Capacitor.c2",
            lines(&[
                &H,
                &[
                    "New Capacitor.c2 bus1=wbus phases=3 kvar=100 kv=0.69",
                    cap_mon.as_str(),
                ],
                &PF,
                &["Edit Capacitor.c2 conn=delta"],
            ]),
        ),
        (
            "conn=delta on an enabled two-phase wye Load",
            "Load.l2",
            lines(&[
                &H,
                &[
                    "New Load.l2 bus1=wbus phases=2 kv=0.69 kW=50 kvar=10",
                    load_mon.as_str(),
                ],
                &PF,
                &["Edit Load.l2 conn=delta"],
            ]),
        ),
        (
            "conn=delta on an enabled two-phase wye Storage",
            "Storage.s2",
            lines(&[
                &H,
                &[
                    "New Storage.s2 bus1=wbus phases=2 kv=0.69 kWrated=50 kWhrated=200 kva=50 \
                     %stored=50 %discharge=60 state=discharging",
                    storage_mon.as_str(),
                ],
                &PF,
                &["Edit Storage.s2 conn=delta"],
            ]),
        ),
        (
            "phases=3 on an enabled two-phase delta WindGen",
            "WindGen.w2",
            lines(&[
                &H,
                &[
                    "New WindGen.w2 bus1=wbus phases=2 conn=delta kv=0.69 kW=10 kva=12 model=1 \
                     vss=1 pss=1 qss=0 vwind=12",
                    windgen_mon.as_str(),
                ],
                &PF,
                &["Edit WindGen.w2 phases=3"],
            ]),
        ),
    ];
    for (form, elem, before) in &forms {
        for entry in ["", ENTRIES[1]] {
            let case = format!("{form} [{entry}]");
            let mut dss = deck(&dir, &with_entry(before, entry));
            let cd = elem_data(&dss, elem);
            assert!(cd.enabled, "{case}: enabled");
            assert!(
                cd.node_ref.is_empty() && !cd.is_mapped(),
                "{case}: the map of the old shape is dropped"
            );
            assert_zero_powers(&mut dss, &case, elem);
            report(&mut dss, "Show Currents Elements");
            run(&mut dss, &["Sample".to_string()]);
            let row = last_monitor_row(&mut dss, "mg");
            assert!(!row.is_empty(), "{case}: an empty monitor row");
            assert!(row.iter().all(|&x| x == 0.0), "{case}: monitor row {row:?}");
        }
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// A monitor in a mode that reads terminal voltages, on a disabled element
/// re-phased after the rebuild that kept its map, records a zero row at
/// `Sample`. The element keeps the map of its old shape until it is enabled, so
/// the terminal-voltage reader meets a map that does not fit.
#[test]
fn a_monitor_reads_a_disabled_element_reshaped_since_its_map_as_zero() {
    let dir = scratch("disabled_reshaped_monitor");
    for mode in [0, 1, 11, 12] {
        let case = format!("mode {mode}");
        let monitor = format!("New Monitor.mg element=Generator.e1 terminal=1 mode={mode}");
        let mut dss = deck(
            &dir,
            &lines(&[
                &H,
                &[GEN1, monitor.as_str()],
                &PF,
                &[
                    "Disable Generator.e1",
                    "Solve",
                    "Edit Generator.e1 phases=3 bus1=wbus kv=0.69",
                ],
            ]),
        );
        let cd = elem_data(&dss, "Generator.e1");
        assert!(
            !cd.enabled && !cd.node_ref.is_empty() && !cd.is_mapped(),
            "{case}: the disabled element keeps a map of its old shape"
        );
        run(&mut dss, &["Sample".to_string()]);
        let row = last_monitor_row(&mut dss, "mg");
        assert!(!row.is_empty(), "{case}: an empty monitor row");
        assert!(row.iter().all(|&x| x == 0.0), "{case}: monitor row {row:?}");
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// A bus rebuild without a power flow that gives a machine of the last power
/// flow other node numbers sets it outside the run, so after `MakeBusList` or
/// `Show Isolated` the first solve of each dynamics-model mode and a
/// step-solution command refuse it as renumbered, where the same entry before
/// the rebuild, or after a rebuild that keeps its map, seeds it. A `Solve` after
/// the rebuild takes it into the power flow again, and its run then ends where
/// the run of the rebuilt circuit from the start ends.
#[test]
fn a_rebuild_that_renumbers_a_machine_sets_it_outside_the_run() {
    let dir = scratch("renumbered");
    // `Line.l0` alone holds `abus`. Disabling it and enabling `Line.lz` gives
    // `wbus` the nodes `abus` had and `zbus` those of `wbus`, and keeps the node
    // count.
    let l0 = "New Line.l0 bus1=srcbus bus2=abus phases=3 r1=0.005 x1=0.02 length=1";
    let lz = "New Line.lz bus1=wbus bus2=zbus phases=3 r1=0.005 x1=0.02 length=1";
    let (l0_off, lz_off) = (format!("{l0} enabled=no"), format!("{lz} enabled=no"));
    let swapped = |extra: &[&str]| {
        lines(&[
            &[H[0], H[1], l0, H[2], H[3], GEN, lz_off.as_str()],
            extra,
            &PF,
            &["Disable Line.l0", "Enable Line.lz"],
        ])
    };
    let entry5 = dyn_entry(5);
    for rebuild in ["MakeBusList", "Show Isolated"] {
        for (mode, extra, entry, cmd) in [
            ("dynamic", &[][..], entry5.as_str(), "Solve"),
            ("faultstudy", &[][..], "Set mode=faultstudy", "Solve"),
            ("mf", &[FLT][..], "Set mode=mf number=2", "Solve"),
            ("dynamic", &[][..], entry5.as_str(), "_SolvePFlow"),
        ] {
            let case = format!("{rebuild}, {cmd} after Set mode={mode}");
            let mut dss = deck(&dir, &then(&swapped(extra), &[rebuild]));
            let before = elem_data(&dss, "Generator.e1").node_ref.clone();
            assert!(
                !elem_data(&dss, "Generator.e1").in_power_flow,
                "{case}: outside the last power flow"
            );
            run(&mut dss, &[entry.to_string()]);
            assert_eq!(
                elem_data(&dss, "Generator.e1").node_ref,
                before,
                "{case}: the entry keeps the rebuilt map"
            );
            assert_refused(
                &mut dss,
                &case,
                cmd,
                &renumbered_msg("Generator", mode),
                &[],
            );
        }
        // A rebuild after another element left and joined again gives the
        // machine the map it had, so it stays in the power flow.
        let case = format!("{rebuild} that keeps the map");
        let mut dss = deck(
            &dir,
            &lines(&[
                &H,
                &[GEN],
                &PF,
                &["Disable Load.ld", "Enable Load.ld", rebuild],
            ]),
        );
        assert!(
            elem_data(&dss, "Generator.e1").in_power_flow,
            "{case}: in the last power flow"
        );
        run(&mut dss, &[entry5.clone(), "Solve".to_string()]);
        assert_eq!(
            elem_data(&dss, "Generator.e1").dynamics_seed,
            DynamicsSeed::Seeded(3),
            "{case}: seeded"
        );
        assert!(
            dss.circuit().expect("circuit").solution.converged_flag,
            "{case}: converged"
        );
    }

    // The entry before any rebuild reads the node voltages through the map the
    // power flow used.
    let mut dss = deck(&dir, &then(&swapped(&[]), &[&entry5, "Solve"]));
    assert!(
        dss.circuit().expect("circuit").solution.converged_flag,
        "no rebuild before the entry"
    );
    let unrebuilt = wbus1(&dss);
    // The rebuilt circuit from the start, and the swapped one solved again.
    let fresh = deck(
        &dir,
        &lines(&[
            &[H[0], H[1], l0_off.as_str(), H[2], H[3], GEN, lz],
            &PF,
            &[&entry5, "Solve"],
        ]),
    );
    assert_eq!(
        format!("{:.6}", wbus1(&fresh)),
        "414.450889",
        "the rebuilt circuit from the start"
    );
    dss = deck(
        &dir,
        &then(&swapped(&[]), &["MakeBusList", "Solve", &entry5, "Solve"]),
    );
    assert_eq!(
        format!("{:.6}", wbus1(&dss)),
        "414.450900",
        "solved after the rebuild"
    );
    // Two power flows that each end within the tolerance of one fixpoint.
    let tol = dss
        .circuit()
        .expect("circuit")
        .solution
        .convergence_tolerance;
    for (what, v) in [
        ("solved after the rebuild", wbus1(&dss)),
        ("no rebuild", unrebuilt),
    ] {
        assert!(
            (v - wbus1(&fresh)).abs() <= 2.0 * tol * wbus1(&fresh),
            "{what}: {v} against {} from the start",
            wbus1(&fresh)
        );
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// A re-shaping edit: the form, the elements before it, the edit, the elements
/// written in the new shape, and |V(WBUS.n)| of that circuit and of the edited
/// one, one reading for every phase or one per phase (empty where it is not
/// pinned).
type Reshape<'a> = (
    &'a str,
    Vec<&'a str>,
    &'a str,
    Vec<&'a str>,
    &'a str,
    &'a str,
);

/// The reading of `readings` at phase `n`, one for every phase or one per phase.
fn reading_at(readings: &str, n: usize) -> Option<&str> {
    let all: Vec<&str> = readings.split_whitespace().collect();
    match all.len() {
        0 => None,
        1 => Some(all[0]),
        _ => Some(all[n - 1]),
    }
}

/// The power flow after an edit that re-shapes an element, re-phased or
/// reconnected or given through `like=` the shape of another Generator, Load,
/// PVSystem, Storage, WindGen, Capacitor or Reactor of the same or another phase
/// count, solves the element as it now is. Each element has the terminals and
/// conductors of the circuit written in that shape from the start, and the power
/// flow ends within the power-flow tolerance of it.
#[test]
fn the_power_flow_after_a_reshape_solves_the_new_shape() {
    let dir = scratch("reshaped_power_flow");
    let g3d = "New Generator.e1 bus1=wbus phases=3 conn=delta kv=0.69 kW=1000 kva=1200 model=1";
    let g3 = "New Generator.g3 bus1=wbus phases=3 kv=0.69 kW=10 kva=12 model=1";
    let e3 = "New Generator.e1 bus1=wbus phases=3 kv=0.69 kW=10 kva=12 model=1";
    let gd = "New Generator.gd bus1=wbus phases=3 conn=delta kv=0.69 kW=10 kva=12 model=1";
    let ed = "New Generator.e1 bus1=wbus phases=3 conn=delta kv=0.69 kW=10 kva=12 model=1";
    let cap = "New Capacitor.c1 bus1=wbus phases=3 kvar=100 kv=0.69";
    let rx = "New Reactor.r1 bus1=wbus phases=3 kvar=100 kv=0.69";
    let (cap_d, rx_d) = (format!("{cap} conn=delta"), format!("{rx} conn=delta"));
    let (cw, c1w) = (
        "New Capacitor.cw bus1=wbus phases=3 kvar=50 kv=0.69",
        "New Capacitor.c1 bus1=wbus phases=3 kvar=50 kv=0.69",
    );
    let (rw, r1w) = (
        "New Reactor.rw bus1=wbus phases=3 kvar=50 kv=0.69",
        "New Reactor.r1 bus1=wbus phases=3 kvar=50 kv=0.69",
    );
    let (cw1, c1d1, c1w1) = (
        "New Capacitor.cw bus1=wbus.1 phases=1 kvar=50 kv=0.398",
        "New Capacitor.c1 bus1=wbus.1.2 phases=1 kvar=100 kv=0.69 conn=delta",
        "New Capacitor.c1 bus1=wbus.1.2 phases=1 kvar=50 kv=0.398",
    );
    let (cd2, ce3, ed2) = (
        "New Capacitor.d1 bus1=wbus phases=2 conn=delta kvar=60 kv=0.69",
        "New Capacitor.e1 bus1=wbus phases=3 kvar=100 kv=0.69",
        "New Capacitor.e1 bus1=wbus phases=2 conn=delta kvar=60 kv=0.69",
    );
    let (rd2, re3, red2) = (
        "New Reactor.d1 bus1=wbus phases=2 conn=delta kvar=60 kv=0.69",
        "New Reactor.e1 bus1=wbus phases=3 kvar=100 kv=0.69",
        "New Reactor.e1 bus1=wbus phases=2 conn=delta kvar=60 kv=0.69",
    );
    let (l3d, e3d, l2w) = (
        "New Load.l3 bus1=wbus phases=3 conn=delta kv=0.69 kW=100 kvar=30",
        "New Load.e1 bus1=wbus phases=3 conn=delta kv=0.69 kW=100 kvar=30",
        "New Load.e1 bus1=wbus phases=2 kv=0.69 kW=50 kvar=10",
    );
    let (pv2, pv3, pve) = (
        "New PVSystem.e1 bus1=wbus phases=2 conn=wye kv=0.69 kVA=1200 Pmpp=1000 irradiance=1",
        "New PVSystem.p3 bus1=wbus phases=3 kv=0.69 kVA=12 Pmpp=10 irradiance=1",
        "New PVSystem.e1 bus1=wbus phases=3 kv=0.69 kVA=12 Pmpp=10 irradiance=1",
    );
    let (st2, st3, ste) = (
        "New Storage.e1 bus1=wbus phases=2 conn=wye kv=0.69 kWrated=500 kWhrated=2000 kva=500 \
         %stored=50 %discharge=60 state=discharging",
        "New Storage.s3 bus1=wbus phases=3 kv=0.69 kWrated=10 kWhrated=40 kva=10 %stored=50 \
         %discharge=60 state=discharging",
        "New Storage.e1 bus1=wbus phases=3 kv=0.69 kWrated=10 kWhrated=40 kva=10 %stored=50 \
         %discharge=60 state=discharging",
    );
    let (wg2, wg3, wge) = (
        "New WindGen.e1 bus1=wbus phases=2 conn=wye kv=0.69 kW=1500 kva=1800 model=1 vss=1 \
         pss=1 qss=0 vwind=12",
        "New WindGen.w3 bus1=wbus phases=3 kv=0.69 kW=10 kva=12 conn=wye model=1 vss=1 pss=1 \
         qss=0 vwind=12",
        "New WindGen.e1 bus1=wbus phases=3 kv=0.69 kW=10 kva=12 conn=wye model=1 vss=1 pss=1 \
         qss=0 vwind=12",
    );
    // The WindGen written out reads differently on each engine, a difference of
    // its power flow and not of `like=`, so only the copy against it is pinned.
    let forms: [Reshape; 18] = [
        (
            "phases=3 on a two-phase delta Generator",
            vec![G2D],
            "Edit Generator.e1 phases=3",
            vec![g3d],
            "408.404574",
            "",
        ),
        (
            "conn=delta phases=3 on a two-phase wye Generator",
            vec![G2W],
            "Edit Generator.e1 conn=delta phases=3",
            vec![g3d],
            "408.404574",
            "",
        ),
        (
            "like= of a three-phase wye Generator onto a two-phase one",
            vec![G2W, g3],
            "Edit Generator.e1 like=g3",
            vec![g3, e3],
            "395.633802",
            "",
        ),
        (
            "conn=delta on a wye Capacitor",
            vec![cap],
            "Edit Capacitor.c1 conn=delta",
            vec![cap_d.as_str()],
            "397.054114",
            "397.054109",
        ),
        (
            "conn=delta on a wye Reactor",
            vec![rx],
            "Edit Reactor.r1 conn=delta",
            vec![rx_d.as_str()],
            "393.677998",
            "393.677981",
        ),
        (
            "like= of a wye Capacitor onto a delta one",
            vec![cw, cap_d.as_str()],
            "Edit Capacitor.c1 like=cw",
            vec![cw, c1w],
            "397.054114",
            "",
        ),
        (
            "a delta Capacitor created as a copy of a wye one",
            vec![cw],
            "New Capacitor.c1 bus1=wbus phases=3 kvar=100 kv=0.69 conn=delta like=cw",
            vec![cw, c1w],
            "397.054114",
            "",
        ),
        (
            "like= of a wye Reactor onto a delta one",
            vec![rw, rx_d.as_str()],
            "Edit Reactor.r1 like=rw",
            vec![rw, r1w],
            "393.677998",
            "",
        ),
        (
            "a delta Reactor created as a copy of a wye one",
            vec![rw],
            "New Reactor.r1 bus1=wbus phases=3 kvar=100 kv=0.69 conn=delta like=rw",
            vec![rw, r1w],
            "393.677998",
            "",
        ),
        (
            "like= of a one-phase wye Capacitor onto a one-phase delta one",
            vec![cw1, c1d1],
            "Edit Capacitor.c1 like=cw",
            vec![cw1, c1w1],
            "431.332019 398.315103 364.370517",
            "",
        ),
        (
            "like= of a two-phase delta Capacitor onto a three-phase wye one",
            vec![cd2, ce3],
            "Edit Capacitor.e1 like=d1",
            vec![cd2, ed2],
            "391.470298 404.153229 394.643550",
            "",
        ),
        (
            "like= of a two-phase delta Reactor onto a three-phase wye one",
            vec![rd2, re3],
            "Edit Reactor.e1 like=d1",
            vec![rd2, red2],
            "398.907440 386.881440 396.365042",
            "",
        ),
        (
            "a Generator created as a copy of a three-phase delta one",
            vec![gd],
            "New Generator.e1 bus1=wbus like=gd",
            vec![gd, ed],
            "395.633802",
            "",
        ),
        (
            "a Load created as a delta copy of the same phase count",
            vec![l3d],
            "New Load.e1 bus1=wbus like=l3",
            vec![l3d, e3d],
            "393.408372",
            "",
        ),
        (
            "like= of a three-phase delta Load onto a two-phase wye one",
            vec![l2w, l3d],
            "Edit Load.e1 like=l3",
            vec![l3d, e3d],
            "393.408372",
            "",
        ),
        (
            "like= of a three-phase wye PVSystem onto a two-phase one",
            vec![pv2, pv3],
            "Edit PVSystem.e1 like=p3",
            vec![pv3, pve],
            "395.448931",
            "",
        ),
        (
            "like= of a three-phase wye Storage onto a two-phase one",
            vec![st2, st3],
            "Edit Storage.e1 like=s3",
            vec![st3, ste],
            "395.413051",
            "",
        ),
        (
            "like= of a three-phase wye WindGen onto a two-phase one",
            vec![wg2, wg3],
            "Edit WindGen.e1 like=w3",
            vec![wg3, wge],
            "",
            "",
        ),
    ];
    for (form, elements, edit, reshaped, v_start, v_edited) in forms {
        let fresh = deck(&dir, &lines(&[&H, &reshaped, &PF]));
        let dss = deck(&dir, &lines(&[&H, &elements, &PF, &[edit, "Solve"]]));
        assert_solves_as(form, &dss, &fresh, &reshaped, v_start, v_edited);
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// `dss` converged and solves as `fresh`, the circuit written out: within twice
/// the power-flow tolerance at WBUS.1..3, on the same nodes, and with every
/// element of `written` on the terminals and conductors it has there, a shunt or
/// a series element as there. `v_start` and `v_edited` are the readings of
/// `fresh` and `dss` as in [`Reshape`].
fn assert_solves_as(
    form: &str,
    dss: &Dss,
    fresh: &Dss,
    written: &[&str],
    v_start: &str,
    v_edited: &str,
) {
    let sol = &dss.circuit().expect("circuit").solution;
    assert!(sol.converged_flag, "{form}: converged");
    let tol = sol.convergence_tolerance;
    for n in 1..=3 {
        let node = format!("WBUS.{n}");
        let (got, want) = (node_magnitude(dss, &node), node_magnitude(fresh, &node));
        if let Some(v) = reading_at(v_start, n) {
            assert_eq!(format!("{want:.6}"), v, "{form}: {node} from the start");
        }
        if let Some(v) = reading_at(v_edited, n) {
            assert_eq!(format!("{got:.6}"), v, "{form}: {node} edited");
        }
        assert!(
            (got - want).abs() <= 2.0 * tol * want,
            "{form}: {node} = {got} against {want} from the start"
        );
    }
    assert_same_node_names(form, dss, fresh);
    for line in written {
        let name = line.split_whitespace().nth(1).expect("an element");
        let (got, want) = (elem_data(dss, name), elem_data(fresh, name));
        assert_eq!(
            (got.nterms, got.nconds),
            (want.nterms, want.nconds),
            "{form}: the terminals and conductors of {name}"
        );
        assert_eq!(
            ckt_elem(dss, name).is_shunt(),
            ckt_elem(fresh, name).is_shunt(),
            "{form}: whether {name} is a shunt"
        );
    }
}

/// A circuit rebuilt by edits after its power flow: the form, the header, the
/// elements, the edits (each followed by `Solve`), the circuit written in the
/// final shape, and the readings as in [`Reshape`].
type Rebuilt<'a> = (
    String,
    &'a [&'a str],
    Vec<String>,
    Vec<String>,
    Vec<String>,
    &'a str,
    &'a str,
);

/// Each of `forms` solves as its circuit written out ([`assert_solves_as`]).
fn assert_rebuilt_solve_as_written(tag: &str, forms: &[Rebuilt]) {
    fn strs(v: &[String]) -> Vec<&str> {
        v.iter().map(String::as_str).collect()
    }
    let dir = scratch(tag);
    for (form, head, elements, edits, written, v_start, v_edited) in forms {
        let tail: Vec<&str> = edits.iter().flat_map(|e| [e.as_str(), "Solve"]).collect();
        let (elements, written) = (strs(elements), strs(written));
        let fresh = deck(&dir, &lines(&[head, &written, &PF]));
        let dss = deck(&dir, &lines(&[head, &elements, &PF, &tail]));
        assert_solves_as(form, &dss, &fresh, &written, v_start, v_edited);
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// A wye Capacitor or Reactor has its second terminal on the `bus2` the user
/// gave or else on the ground nodes of `bus1`, whatever order of `New`, `conn=`,
/// `like=`, `phases=`, `bus1=` and `bus2=` built its shape in one command or
/// across edits, and a delta one keeps that `bus2` for a later wye. Each circuit
/// solves as the circuit written out, on its nodes and with its terminals, and
/// Save writes a `bus2` given before `bus1`, so the saved circuit reloads as it
/// was.
#[test]
fn a_wye_capacitor_or_reactor_has_its_second_terminal_where_a_new_one_has_it() {
    const SRC: [&str; 2] = ["Clear", "New Circuit.dp basekv=0.69 phases=3 bus1=srcbus"];
    let dir = scratch("bus2_kept_while_delta");
    let unbalance = "New Load.u1 bus1=wbus.1 phases=1 kv=0.398 kW=100 kvar=30";
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<String>>();
    let mut forms: Vec<Rebuilt> = Vec::new();
    for (class, x, w, d, v) in [
        (
            "Capacitor",
            "c1",
            "cw",
            "cd",
            [
                "399.623050",
                "399.623055",
                "397.054114",
                "396.197686",
                "397.903030",
                "367.577237 430.027711 392.933657",
                "397.906882",
            ],
        ),
        (
            "Reactor",
            "r1",
            "rw",
            "rd",
            [
                "391.182036",
                "391.182009",
                "393.677998",
                "394.527224",
                "392.856212",
                "365.099319 426.306382 389.101293",
                "392.842604",
            ],
        ),
    ] {
        let [
            wye100,
            edited100,
            wye50,
            series50,
            series_d,
            ungrounded,
            delta100,
        ] = v;
        let w50 = format!("New {class}.{w} bus1=wbus phases=3 kvar=50 kv=0.69");
        let d100 = format!("New {class}.{d} bus1=wbus phases=3 kvar=100 kv=0.69 conn=delta");
        let x100 = format!("New {class}.{x} bus1=wbus phases=3 kvar=100 kv=0.69");
        let x50 = format!("New {class}.{x} bus1=wbus phases=3 kvar=50 kv=0.69");
        let xs50 = format!("New {class}.{x} bus1=wbus bus2=srcbus phases=3 kvar=50 kv=0.69");
        let xu = format!("New {class}.{x} bus1=wbus bus2=wbus.4.4.4 phases=3 kvar=100 kv=0.69");
        let one = format!("New {class}.{x} phases=1 bus1=wbus kvar=20 kv=0.398");
        let delta_copy = |bus2: &str| {
            format!(
                "New {class}.{x} bus1=wbus phases=3 kvar=100 kv=0.69 conn=delta {bus2} like={w}"
            )
        };
        let edit = |p: &str| format!("Edit {class}.{x} {p}");
        let delta_bus2 =
            format!("New {class}.{x} bus1=wbus phases=3 kvar=100 kv=0.69 conn=delta bus2=srcbus");
        let mut delta = deck(&dir, &lines(&[&H, &[delta_bus2.as_str()]]));
        let bus2 = query(&mut delta, &format!("{class}.{x}.bus2"));
        assert_eq!(bus2, "srcbus", "{class}: the bus2 a delta element keeps");
        forms.extend([
            (
                format!("bus2 on another bus given to a delta {class}"),
                &H[..],
                s(&[&w50]),
                vec![delta_bus2.clone()],
                s(&[
                    &w50,
                    &format!("New {class}.{x} bus1=wbus phases=3 kvar=100 kv=0.69 conn=delta"),
                ]),
                delta100,
                "",
            ),
            (
                format!("bus2= before bus1= on an ungrounded wye {class}"),
                &H[..],
                s(&[unbalance]),
                vec![format!(
                    "New {class}.{x} bus2=wbus.4.4.4 bus1=wbus phases=3 kvar=100 kv=0.69"
                )],
                s(&[unbalance, &xu]),
                ungrounded,
                "",
            ),
            (
                format!("a wye copy of a delta {class}"),
                &H[..],
                s(&[&w50, &d100]),
                vec![format!("New {class}.{x} bus1=wbus like={d} conn=wye")],
                s(&[&w50, &d100, &x100]),
                wye100,
                "",
            ),
            (
                format!("conn=delta and then conn=wye on a wye {class}"),
                &H[..],
                s(&[&w50, &d100, &x100]),
                vec![edit("conn=delta"), edit("conn=wye")],
                s(&[&w50, &d100, &x100]),
                wye100,
                edited100,
            ),
            (
                format!("conn=delta conn=wye in the command that creates a {class}"),
                &H[..],
                s(&[&w50, &d100]),
                vec![format!("{x100} conn=delta conn=wye")],
                s(&[&w50, &d100, &x100]),
                wye100,
                "",
            ),
            (
                format!("bus2 at the ground nodes given to a delta {class} copying a wye one"),
                &H[..],
                s(&[&w50]),
                vec![delta_copy("bus2=wbus.0.0.0")],
                s(&[&w50, &x50]),
                wye50,
                "",
            ),
            (
                format!("bus2 on another bus given to a delta {class} copying a wye one"),
                &H[..],
                s(&[&w50]),
                vec![delta_copy("bus2=srcbus")],
                s(&[&w50, &xs50]),
                series50,
                "",
            ),
            (
                format!("a series {class} copied to delta and back to wye"),
                &H[..],
                s(&[&w50, &d100, &xs50]),
                vec![edit(&format!("like={d}")), edit(&format!("like={w}"))],
                s(&[&w50, &d100, &xs50]),
                series_d,
                "",
            ),
            (
                format!("conn=delta and then conn=wye on an ungrounded wye {class}"),
                &H[..],
                s(&[unbalance, &xu]),
                vec![edit("conn=delta"), edit("conn=wye")],
                s(&[unbalance, &xu]),
                ungrounded,
                "",
            ),
            (
                format!("bus1= again on an ungrounded wye {class}"),
                &H[..],
                s(&[unbalance, &xu]),
                vec![edit("bus1=wbus")],
                s(&[unbalance, &xu]),
                ungrounded,
                "",
            ),
            (
                format!("phases=3 on a one-phase wye {class}"),
                &H[..],
                s(&[&w50, &one]),
                vec![edit("phases=3 kvar=50 kv=0.69")],
                s(&[&w50, &x50]),
                wye50,
                "",
            ),
        ]);
    }
    let load = "New Load.ld bus1=wbus phases=3 kv=0.69 kW=300 kvar=100";
    let donors = [
        "New Reactor.rw bus1=dbus phases=3 R=0.006 X=0.025",
        "New Reactor.rd bus1=dbus phases=3 R=0.006 X=0.025 conn=delta",
        "Disable Reactor.rw",
        "Disable Reactor.rd",
    ];
    let series = |r: &str| {
        let rs = format!("New Reactor.rs bus1=srcbus bus2=wbus phases=3 {r}");
        let mut v = s(&[&rs, load]);
        v.extend(s(&donors));
        v
    };
    forms.push((
        "a series Reactor that alone feeds the load copied to delta and back to wye".into(),
        &SRC[..],
        series("R=0.005 X=0.02"),
        vec![
            "Edit Reactor.rs like=rd".into(),
            "Edit Reactor.rs like=rw".into(),
        ],
        series("R=0.006 X=0.025"),
        "394.662362",
        "",
    ));
    assert_rebuilt_solve_as_written("bus2_where_a_new_one_has_it", &forms);

    let series = [
        "New Reactor.rs bus2=wbus2 bus1=wbus phases=3 R=0.01 X=0.05",
        "New Load.l2 bus1=wbus2 phases=3 kv=0.69 kW=50 kvar=10",
    ];
    let mut dss = deck(&dir, &lines(&[&H, &series, &PF]));
    assert_wbus_readings(&dss, "394.956458", "before the Save");
    let at_wbus2 = |dss: &Dss| format!("{:.6}", node_magnitude(dss, "WBUS2.1"));
    assert_eq!(at_wbus2(&dss), "394.105451", "WBUS2.1 before the Save");
    let saved = dir.join("saved");
    let saved_path = saved.to_string_lossy().replace('\\', "/");
    run(&mut dss, &[format!("Save circuit dir=\"{saved_path}\"")]);
    let text = std::fs::read_to_string(saved.join("Reactor.dss")).expect("Reactor.dss");
    let line = text
        .lines()
        .find(|l| l.starts_with("New \"Reactor."))
        .unwrap_or_else(|| panic!("no Reactor in {text}"));
    assert!(line.contains("Bus2=wbus2"), "the saved Reactor: {line}");
    let mut back = Dss::new();
    run(
        &mut back,
        &[
            format!("Compile \"{saved_path}/Master.dss\""),
            "Solve".into(),
        ],
    );
    assert_eq!(at_wbus2(&back), "394.105451", "WBUS2.1 after the reload");
    std::fs::remove_dir_all(&dir).ok();
}

/// A Capacitor or Reactor that `bus1=` moves to another bus keeps the `bus2`
/// the user gave: one given on the ground nodes is a shunt at its new bus in the
/// solve and in the meter zones, as the bank written there, and one given on a
/// floating neutral is a branch to that neutral on the old bus.
#[test]
fn a_bank_moved_by_bus1_keeps_the_bus2_the_user_gave() {
    const TWO_FEEDERS: [&str; 6] = [
        "Clear",
        "New Circuit.dp basekv=0.69 phases=3 bus1=srcbus",
        "New Line.l1 bus1=srcbus bus2=wbus phases=3 r1=0.005 x1=0.02 length=1",
        "New Line.l2 bus1=srcbus bus2=xbus phases=3 r1=0.005 x1=0.02 length=1",
        "New Load.ld bus1=wbus phases=3 kv=0.69 kW=300 kvar=100",
        "New Load.lx bus1=xbus.1 phases=1 kv=0.398 kW=100 kvar=30",
    ];
    const METERS: [&str; 2] = [
        "New EnergyMeter.m2 element=Line.l2 terminal=1",
        "New EnergyMeter.m1 element=Line.l1 terminal=1",
    ];
    let dir = scratch("bank_moved_by_bus1");
    let moved = |class: &str, bus2: &str| {
        let bank = format!("New {class}.c bus1=wbus bus2={bus2} phases=3 kvar=100 kv=0.69");
        let edit = format!("Edit {class}.c bus1=xbus");
        deck(
            &dir,
            &lines(&[
                &TWO_FEEDERS,
                &[bank.as_str()],
                &METERS,
                &PF,
                &[edit.as_str(), "Solve"],
            ]),
        )
    };
    for (class, at_xbus, at_wbus) in [
        ("Capacitor", "367.327870", "395.333795"),
        ("Reactor", "370.614108", "395.297935"),
    ] {
        let name = format!("{class}.c");
        let mut dss = moved(class, "wbus.0.0.0");
        let written = format!("New {class}.c bus1=xbus phases=3 kvar=100 kv=0.69");
        let mut fresh = deck(
            &dir,
            &lines(&[&TWO_FEEDERS, &[written.as_str()], &METERS, &PF]),
        );
        assert!(ckt_elem(&dss, &name).is_shunt(), "{class}: a shunt");
        assert!(
            ckt_elem(&fresh, &name).is_shunt(),
            "{class}: a shunt written"
        );
        assert_eq!(
            format!("{:.6}", node_magnitude(&dss, "XBUS.1")),
            at_xbus,
            "{class}: XBUS.1"
        );
        assert_eq!(
            format!("{:.6}", node_magnitude(&dss, "WBUS.1")),
            at_wbus,
            "{class}: WBUS.1"
        );
        assert_solves_on_every_node_as(&name, &dss, &fresh);
        for (meter, zone) in [
            ("m1", vec!["Line.l1", "Load.ld"]),
            ("m2", vec!["Line.l2", "Load.lx", name.as_str()]),
        ] {
            let mut zone: Vec<String> = zone.into_iter().map(String::from).collect();
            zone.sort();
            assert_eq!(
                zone_of(&mut dss, meter),
                zone,
                "{class}: the zone of {meter}"
            );
            assert_eq!(
                zone_of(&mut fresh, meter),
                zone,
                "{class}: the zone of {meter} written"
            );
        }
    }
    for (class, neutral) in [("Capacitor", "39.857883"), ("Reactor", "39.487946")] {
        let dss = moved(class, "wbus.4.4.4");
        let written = format!("New {class}.c bus1=xbus bus2=wbus.4.4.4 phases=3 kvar=100 kv=0.69");
        let fresh = deck(
            &dir,
            &lines(&[&TWO_FEEDERS, &[written.as_str()], &METERS, &PF]),
        );
        let case = format!("the ungrounded {class}");
        let name = format!("{class}.c");
        assert!(!ckt_elem(&dss, &name).is_shunt(), "{case}: a branch");
        assert_eq!(
            format!("{:.6}", node_magnitude(&dss, "WBUS.4")),
            neutral,
            "{case}: its neutral on WBUS.4"
        );
        assert_solves_on_every_node_as(&case, &dss, &fresh);
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// `dss` converged on the nodes of `fresh`, every node within twice the
/// power-flow tolerance of it.
fn assert_solves_on_every_node_as(case: &str, dss: &Dss, fresh: &Dss) {
    let sol = &dss.circuit().expect("circuit").solution;
    assert!(sol.converged_flag, "{case}: converged");
    let tol = sol.convergence_tolerance;
    assert_same_node_names(case, dss, fresh);
    for ((node, got), (_, want)) in node_voltages(dss).iter().zip(node_voltages(fresh)) {
        let (got, want) = (got.norm(), want.norm());
        assert!(
            (got - want).abs() <= 2.0 * tol * want,
            "{case}: {node} = {got} against {want} written out"
        );
    }
}

/// The elements `Show Zone {meter}` lists, sorted, each as written in the
/// report (a loop it reports stays part of the name).
fn zone_of(dss: &mut Dss, meter: &str) -> Vec<String> {
    let text = report(dss, &format!("Show Zone {meter}"));
    let mut zone: Vec<String> = text
        .lines()
        .filter_map(|l| l.split_once(" (Sensor:").map(|(e, _)| e.trim().to_string()))
        .collect();
    zone.sort();
    zone
}

/// A Capacitor, Reactor or Fault copy takes the admittance of the element it
/// copies whatever shape it had, and a wye copy that gains phases grounds its
/// second terminal for the new phase count. Each circuit solves as the circuit
/// written out.
#[test]
fn a_copy_solves_as_the_circuit_written_out() {
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<String>>();
    let mut forms: Vec<Rebuilt> = Vec::new();
    for (class, x, w, wye50) in [
        ("Capacitor", "c1", "cw", "397.054114"),
        ("Reactor", "r1", "rw", "393.677998"),
    ] {
        let w50 = format!("New {class}.{w} bus1=wbus phases=3 kvar=50 kv=0.69");
        let x50 = format!("New {class}.{x} bus1=wbus phases=3 kvar=50 kv=0.69");
        let like = vec![format!("Edit {class}.{x} like={w}")];
        forms.extend([
            (
                format!("like= of a wye {class} onto a wye one of the same shape"),
                &H[..],
                s(&[
                    &w50,
                    &format!("New {class}.{x} bus1=wbus phases=3 kvar=100 kv=0.69"),
                ]),
                like.clone(),
                s(&[&w50, &x50]),
                wye50,
                "",
            ),
            (
                format!("like= of a three-phase wye {class} onto a one-phase one"),
                &H[..],
                s(&[
                    &w50,
                    &format!("New {class}.{x} phases=1 bus1=wbus kvar=20 kv=0.398"),
                ]),
                like,
                s(&[&w50, &x50]),
                wye50,
                "",
            ),
        ]);
    }
    let f1 = "New Fault.f1 bus1=wbus.1 phases=1 r=10";
    forms.push((
        "like= of a Fault onto one of the same phase count".into(),
        &H[..],
        s(&[f1, "New Fault.f2 bus1=wbus.2 phases=1 r=2"]),
        vec!["Edit Fault.f2 like=f1".into()],
        s(&[f1, "New Fault.f2 bus1=wbus.2 phases=1 r=10"]),
        "389.579563 398.190868 397.941390",
        "",
    ));
    assert_rebuilt_solve_as_written("copy_solves_as_written", &forms);
}

/// An IndMach012 takes the conductors its final phase count and connection give,
/// whatever the order of `phases=` and `conn=`: one per phase in wye, whose
/// neutral the machine does not connect, and one more below three phases in
/// delta, on the same node map either way. A one-phase wye or delta machine
/// enters a dynamics run from its present seed, a fault study and a harmonic
/// solve in either order as the circuit of its final connection.
#[test]
fn an_indmach012_takes_its_conductors_from_its_final_connection() {
    let dir = scratch("indmach_conductors");
    let entry = dyn_entry(5);
    for (conn, bus, kv, nconds, [dynamics, fault, harmonic]) in [
        (
            "wye",
            "wbus.1",
            "0.398",
            1,
            ["381.127714", "0.104020", "12.957990 5.007929 11.179873"],
        ),
        (
            "delta",
            "wbus.1.2",
            "0.69",
            2,
            ["397.617071", "0.110921", "8.626708 0.762934 9.495963"],
        ),
    ] {
        let mut maps = Vec::new();
        for order in [
            format!("phases=1 conn={conn}"),
            format!("conn={conn} phases=1"),
        ] {
            let line = format!("New IndMach012.m1 bus1={bus} {order} {IM_DATA} kv={kv}");
            let dss = deck(&dir, &lines(&[&H, &[line.as_str(), "MakeBusList"]]));
            let cd = elem_data(&dss, "IndMach012.m1");
            assert_eq!(cd.nconds, nconds, "{order}: conductors");
            maps.push(cd.node_ref.clone());
            for (study, tail, readings) in [
                ("dynamics", vec![entry.as_str(), "Solve"], dynamics),
                ("fault study", vec!["Set mode=faultstudy", "Solve"], fault),
                ("harmonic solve", HARMONIC_5.to_vec(), harmonic),
            ] {
                let solved = deck(&dir, &lines(&[&H, &[line.as_str()], &PF, &tail]));
                let sol = &solved.circuit().expect("circuit").solution;
                assert!(sol.converged_flag, "{conn} {order}: {study} converged");
                assert_wbus_readings(&solved, readings, &format!("{conn} {order}: {study}"));
            }
        }
        assert_eq!(maps[0], maps[1], "{conn}: the node map of either order");
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// The machine data of the IndMach012 pins.
const IM_DATA: &str =
    "kW=20 kVA=25 H=6 purs=0.048 puxs=0.075 purr=0.018 puxr=0.12 puxm=3.8 slip=0.02";

/// A harmonic solve at the fifth harmonic after the power flow.
const HARMONIC_5: [&str; 3] = ["Set harmonics=[5]", "Set mode=harmonic", "Solve"];

/// |V(WBUS.1..)| of `dss` reads `readings`, one per phase from phase 1.
fn assert_wbus_readings(dss: &Dss, readings: &str, case: &str) {
    for (n, want) in readings.split_whitespace().enumerate() {
        let node = format!("WBUS.{}", n + 1);
        assert_eq!(
            format!("{:.6}", node_magnitude(dss, &node)),
            want,
            "{case}: {node}"
        );
    }
}

/// A three-phase wye IndMach012 in a harmonic solve is a passive admittance on
/// each phase, so on the balanced deck it reads the same harmonic voltage on
/// every phase in either order of `phases=` and `conn=`.
#[test]
fn a_balanced_wye_indmach012_reads_balanced_harmonic_voltages() {
    let dir = scratch("indmach_balanced_harmonic");
    for order in ["phases=3 conn=wye", "conn=wye phases=3"] {
        let line = format!("New IndMach012.m1 bus1=wbus {order} {IM_DATA} kv=0.69");
        let dss = deck(&dir, &lines(&[&H, &[line.as_str()], &PF, &HARMONIC_5]));
        assert_wbus_readings(&dss, "5.289504 5.289504 5.289504", order);
        // Without a machine the deck already spreads its phases inside this band.
        let v: Vec<f64> = (1..=3)
            .map(|n| node_magnitude(&dss, &format!("WBUS.{n}")))
            .collect();
        for (n, vn) in v.iter().enumerate() {
            assert!(
                (vn - v[0]).abs() <= 1e-8 * v[0],
                "{order}: WBUS.{} = {vn} against WBUS.1 = {}",
                n + 1,
                v[0]
            );
        }
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// Both circuits hold the same nodes.
fn assert_same_node_names(case: &str, got: &Dss, reference: &Dss) {
    let names =
        |dss: &Dss| -> Vec<String> { node_voltages(dss).into_iter().map(|x| x.0).collect() };
    assert_eq!(names(got), names(reference), "{case}: node lists");
}

/// A Monitor or an EnergyMeter on an element without a node map records zero voltage, current and
/// registers at `Sample`, in snap mode and after each dynamics-model entry. The dropped form is a
/// Line's far bus, because a monitor on a machine keeps the machine's bus in the bus list.
#[test]
fn samplers_read_an_element_without_a_node_map_as_connected_to_no_node() {
    let dir = scratch("unmapped_samplers");
    let l3 = "New Line.l3 bus1=srcbus bus2=wbus phases=3 r1=0.005 x1=0.02 length=1";
    let monitored: [(&str, Vec<String>); 3] = [
        (
            "Monitor on a Line created after the last rebuild",
            lines(&[
                &H,
                &PF,
                &[l3, "New Monitor.mg element=Line.l3 terminal=1 mode=0"],
            ]),
        ),
        (
            "Monitor on a Generator created after the last rebuild",
            lines(&[
                &H,
                &PF,
                &[GEN, "New Monitor.mg element=Generator.e1 terminal=1 mode=1"],
            ]),
        ),
        (
            "Monitor on a Line enabled again on a dropped bus",
            lines(&[
                &H,
                &[L2_GBUS, "New Monitor.mg element=Line.l2 terminal=1 mode=0"],
                &PF,
                &["Disable Line.l2", "Solve", "Enable Line.l2"],
            ]),
        ),
    ];
    let meter = lines(&[
        &H,
        &[L2_GBUS, "New EnergyMeter.m3 element=Line.l2 terminal=1"],
        &PF,
        &["Disable Line.l2", "Solve", "Enable Line.l2"],
    ]);
    for entry in ENTRIES {
        for (form, before) in &monitored {
            let case = format!("{form} [{entry}]");
            let mut dss = deck(&dir, &with_entry(before, entry));
            run(&mut dss, &["Sample".to_string()]);
            let row = last_monitor_row(&mut dss, "mg");
            assert!(!row.is_empty(), "{case}: an empty monitor row");
            assert!(row.iter().all(|&x| x == 0.0), "{case}: monitor row {row:?}");
        }
        let case = format!("EnergyMeter on a Line enabled again on a dropped bus [{entry}]");
        let mut dss = deck(&dir, &with_entry(&meter, entry));
        assert!(
            !elem_data(&dss, "Line.l2").is_mapped(),
            "{case}: no node map"
        );
        run(&mut dss, &["Sample".to_string()]);
        // A row is `year, load-duration curve, hour, meter, registers`, and the
        // file keeps the rows of the earlier cases, each of them zero too.
        let text = report(&mut dss, "Export Meters");
        let rows: Vec<&str> = text
            .lines()
            .filter(|l| l.to_ascii_uppercase().contains("\"M3\""))
            .collect();
        assert!(!rows.is_empty(), "{case}: no row\n{text}");
        for row in rows {
            let registers: Vec<f64> = row
                .split(',')
                .skip(4)
                .map(|f| f.trim().parse().unwrap_or_else(|_| panic!("{f} in {row}")))
                .collect();
            assert!(
                !registers.is_empty() && registers.iter().all(|&x| x == 0.0),
                "{case}: {row}"
            );
        }
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// An element enabled again with a node map a bus rebuild left right reads in
/// every reader exactly as after a further `MakeBusList`, in snap mode and
/// after each dynamics-model entry. One enabled again on a dropped bus, or
/// created after the last rebuild, reads at node 0 with zero power, volts and
/// current or is left out as the same element disabled, and `Export CIM100`
/// leaves out a Load, a Storage, a Capacitor and a Transformer created so too.
#[test]
fn readers_of_an_element_enabled_after_a_bus_rebuild() {
    let dir = scratch("rebuild_readers");
    let gen_row = "\"Generator.e1\", 1, 4, 1, 2, 3, 0";
    let kept_through = |rebuild: &str| {
        lines(&[
            &H,
            &[METER, GEN],
            &PF,
            &["Disable Generator.e1", rebuild, "Enable Generator.e1"],
        ])
    };
    let kept: [(&str, &str, Vec<String>, &str); 6] = [
        (
            "Line kept across Solve",
            "Line.l2",
            lines(&[
                &H,
                &[METER, L2_PAR],
                &PF,
                &["Disable Line.l2", "Solve", "Enable Line.l2"],
            ]),
            "\"Line.l2\", 2, 3, 1, 2, 3, 1, 2, 3",
        ),
        (
            "PVSystem kept across Solve",
            "PVSystem.e1",
            lines(&[
                &H,
                &[METER, PV],
                &PF,
                &["Disable PVSystem.e1", "Solve", "Enable PVSystem.e1"],
            ]),
            "\"PVSystem.e1\", 1, 4, 1, 2, 3, 0",
        ),
        (
            "Generator kept across Solve",
            "Generator.e1",
            kept_through("Solve"),
            gen_row,
        ),
        (
            "Generator kept across Calcvoltagebases",
            "Generator.e1",
            kept_through("Calcvoltagebases"),
            gen_row,
        ),
        (
            "Generator kept across MakeBusList",
            "Generator.e1",
            kept_through("MakeBusList"),
            gen_row,
        ),
        (
            "Generator kept across Show Isolated",
            "Generator.e1",
            kept_through("Show Isolated"),
            gen_row,
        ),
    ];
    let mut kept_cases: Vec<(String, &str, Vec<String>, &str)> = Vec::new();
    for (case, elem, before, row) in &kept {
        for entry in ENTRIES {
            kept_cases.push((
                format!("{case} [{entry}]"),
                elem,
                with_entry(before, entry),
                row,
            ));
        }
    }
    kept_cases.push((
        "Line kept across a dynamics step".to_string(),
        "Line.l2",
        lines(&[
            &H,
            &[METER, L2_PAR],
            &PF,
            &[
                dyn_entry(1).as_str(),
                "Solve",
                "Disable Line.l2",
                "Solve",
                "Enable Line.l2",
            ],
        ]),
        "\"Line.l2\", 2, 3, 1, 2, 3, 1, 2, 3",
    ));
    // The first solve drops its bus and marks the map stale, the second brings
    // the bus back on the same nodes while the Generator is still off.
    let gen_at_gbus = GEN.replace("bus1=wbus", "bus1=gbus");
    kept_cases.push((
        "Generator kept across a solve that brought its bus back".to_string(),
        "Generator.e1",
        lines(&[
            &H,
            &[METER, L2_GBUS, gen_at_gbus.as_str()],
            &PF,
            &[
                "Disable Line.l2",
                "Disable Generator.e1",
                "Solve",
                "Enable Line.l2",
                "Solve",
                "Enable Generator.e1",
            ],
        ]),
        gen_row,
    ));
    for (case, elem, all, row) in &kept_cases {
        let mut dss = deck(&dir, all);
        assert!(elem_data(&dss, elem).is_mapped(), "{case}: no node map");
        // The same state with the element mapped afresh: `MakeBusList` before
        // the entry, which only rebuilds the bus list.
        let (before, entry) = match all.last().map(String::as_str) {
            Some(e) if e.starts_with("Set mode=") => (&all[..all.len() - 1], Some(e)),
            _ => (&all[..], None),
        };
        let mut fresh_lines = before.to_vec();
        fresh_lines.push("MakeBusList".to_string());
        fresh_lines.extend(entry.map(String::from));
        let mut fresh = deck(&dir, &fresh_lines);
        for reader in MAP_READERS {
            assert_eq!(
                report(&mut dss, reader),
                report(&mut fresh, reader),
                "{case}: {reader}"
            );
        }
        let order = report(&mut dss, "Export NodeOrder");
        assert_eq!(rows_of(&order, elem), [*row], "{case}: {order}");
    }

    // (case, element, the deck, the same deck with the element disabled)
    let mut unmapped: Vec<(&str, &str, Vec<String>, Vec<String>)> = Vec::new();
    for (class, line) in [("Generator", GEN), ("PVSystem", PV)] {
        let enabled = dropped_bus(class, &[METER], line);
        let disabled = enabled[..enabled.len() - 1].to_vec();
        let case = if class == "Generator" {
            "Generator enabled again on a bus the last solve dropped"
        } else {
            "PVSystem enabled again on a bus the last solve dropped"
        };
        unmapped.push((
            case,
            if class == "Generator" {
                "Generator.e1"
            } else {
                "PVSystem.e1"
            },
            enabled,
            disabled,
        ));
    }
    let line_dropped = |enable: bool| {
        let mut l = lines(&[&H, &[METER, L2_GBUS], &PF, &["Disable Line.l2", "Solve"]]);
        if enable {
            l.push("Enable Line.l2".to_string());
        }
        l
    };
    unmapped.push((
        "Line enabled again on a bus the last solve dropped",
        "Line.l2",
        line_dropped(true),
        line_dropped(false),
    ));
    let gen_created = GEN.replace("bus1=wbus", "bus1=gbus");
    let gen_created_off = format!("{gen_created} enabled=no");
    unmapped.push((
        "Generator created after the last rebuild",
        "Generator.e1",
        lines(&[&H, &[METER], &PF, &[L2_GBUS, gen_created.as_str()]]),
        lines(&[&H, &[METER], &PF, &[L2_GBUS, gen_created_off.as_str()]]),
    ));
    let l3 = "New Line.l3 bus1=srcbus bus2=wbus phases=3 r1=0.005 x1=0.02 length=1";
    let l3_off = format!("{l3} enabled=no");
    unmapped.push((
        "Line created after the last rebuild",
        "Line.l3",
        lines(&[&H, &[METER], &PF, &[l3]]),
        lines(&[&H, &[METER], &PF, &[l3_off.as_str()]]),
    ));
    for (form, elem, enabled, disabled) in &unmapped {
        for entry in ENTRIES {
            let case = format!("{form} [{entry}]");
            let mut dss = deck(&dir, &with_entry(enabled, entry));
            let mut off = deck(&dir, &with_entry(disabled, entry));
            let cd = elem_data(&dss, elem).clone();
            assert!(!cd.is_mapped(), "{case}: a node map");
            assert!(cd.node_ref.is_empty(), "{case}: {:?}", cd.node_ref);
            for c in element_currents(&mut dss, elem) {
                assert_eq!(c, Complex64::ZERO, "{case}: current");
            }
            let is_pd = elem.starts_with("Line");

            let seq = report(&mut dss, "Export SeqPowers");
            let seq_rows = rows_of(&seq, elem);
            assert_eq!(seq_rows.len(), cd.nterms, "{case}: {seq}");
            for r in &seq_rows {
                assert!(numbers(r, 2).iter().all(|&v| v == 0.0), "{case}: {r}");
            }

            let volts = report(&mut dss, "Export VoltagesElements");
            let max_nodes = volts
                .lines()
                .find(|l| l.starts_with("Element,"))
                .expect("header")
                .matches("Node1_")
                .count();
            assert_eq!(
                rows_of(&volts, elem),
                [unmapped_voltages_row(elem, &cd, max_nodes).as_str()],
                "{case}"
            );

            let order = report(&mut dss, "Export NodeOrder");
            let zero_row = format!(
                "\"{elem}\", {}, {}{}",
                cd.nterms,
                cd.nconds,
                ", 0".repeat(cd.nterms * cd.nconds)
            );
            assert_eq!(rows_of(&order, elem), [zero_row.as_str()], "{case}");

            if is_pd {
                let capacity = report(&mut dss, "Export Capacity");
                let rows = rows_of(&capacity, elem);
                assert_eq!(rows.len(), 1, "{case}: {capacity}");
                let v = numbers(rows[0], 1);
                assert_eq!(v, [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 3.0, 0.0], "{case}");
            }

            let powers = report(&mut dss, "Show Powers");
            let rows = rows_of(&powers, elem);
            assert_eq!(rows.len(), cd.nterms, "{case}: {powers}");
            for r in &rows {
                assert!(numbers(r, 2).iter().all(|&v| v == 0.0), "{case}: {r}");
            }

            let delta = report(&mut dss, "Show deltaV");
            let upper = crate::report::format::upper_elem_name(elem);
            assert!(!delta.contains(&upper), "{case}: {delta}");

            for reader in [
                "Export CIM100",
                "Export BranchReliability",
                "Show Mismatch",
                "Show kVBaseMismatch",
            ] {
                assert_eq!(
                    report(&mut dss, reader),
                    report(&mut off, reader),
                    "{case}: {reader} against the element disabled"
                );
            }
        }
    }

    // The CIM sweeps of the other classes an element created after the last
    // rebuild can be in.
    let created_cim = [
        ("Load.l7", "New Load.l7 bus1=wbus phases=3 kv=0.69 kW=30"),
        ("Storage.e1", STI),
        (
            "Capacitor.c1",
            "New Capacitor.c1 bus1=wbus phases=3 kvar=100 kv=0.69",
        ),
        (
            "Transformer.t1",
            "New Transformer.t1 phases=3 windings=2 buses=[wbus tbus] kvs=[0.69 0.48] \
             kvas=[500 500]",
        ),
    ];
    for (elem, line) in created_cim {
        let off_line = format!("{line} enabled=no");
        let created = lines(&[&H, &[METER], &PF, &[line]]);
        let created_off = lines(&[&H, &[METER], &PF, &[off_line.as_str()]]);
        for entry in ENTRIES {
            let case = format!("{elem} created after the last rebuild [{entry}]");
            let mut dss = deck(&dir, &with_entry(&created, entry));
            let mut off = deck(&dir, &with_entry(&created_off, entry));
            assert!(!elem_data(&dss, elem).is_mapped(), "{case}: a node map");
            assert_eq!(
                report(&mut dss, "Export CIM100"),
                report(&mut off, "Export CIM100"),
                "{case}: Export CIM100 against the element disabled"
            );
        }
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// `like=` onto a disabled Generator turns it on as `Enable` does: with its
/// line back, with its line off and on a renumbered bus list it gives what
/// `Enable` and the same edit give for `Export Powers`, `Solve` and
/// `_SolvePFlow`, and after a dynamics-model entry it is outside the run. On
/// the renumbered list it injects at its own bus, an island whose one solution
/// is zero volts with no current, and the solve converges there.
#[test]
fn like_enables_a_disabled_machine_as_enable_does() {
    let dir = scratch("rebuild_like");
    let at_gbus = GEN.replace("bus1=wbus", "bus1=gbus");
    let head = lines(&[&H, &[L2_GBUS, at_gbus.as_str(), G2], &PF]);
    let renumbered = lines(&[
        &[
            "Clear",
            "New Circuit.dp basekv=0.69 phases=3 bus1=srcbus",
            "New Line.l0 bus1=srcbus bus2=abus phases=3 r1=0.005 x1=0.02 length=1",
            "New Line.l1 bus1=srcbus bus2=wbus phases=3 r1=0.005 x1=0.02 length=1",
            "New Load.ld bus1=wbus phases=3 kv=0.69 kW=300 kvar=100",
            "New Load.la bus1=abus phases=3 kv=0.69 kW=50 kvar=10",
            "New Generator.e1 bus1=abus phases=3 kv=0.69 kW=150 kva=180 model=1",
            G2,
        ],
        &PF,
        &[
            "Disable Line.l0",
            "Disable Load.la",
            "Disable Generator.e1",
            "Solve",
        ],
    ]);
    let forms: [(&str, Vec<String>); 3] = [
        (
            "its line back",
            then(
                &head,
                &[
                    "Disable Line.l2",
                    "Disable Generator.e1",
                    "Solve",
                    "Enable Line.l2",
                ],
            ),
        ),
        (
            "its line off",
            then(&head, &["Disable Line.l2", "Disable Generator.e1", "Solve"]),
        ),
        ("a renumbered bus list", renumbered),
    ];
    let like = "Edit Generator.e1 like=g2";
    for (form, before) in &forms {
        let routes = |tail: &[&str]| {
            let by_like = then(&then(before, &[like]), tail);
            let by_enable = then(&then(before, &["Enable Generator.e1", like]), tail);
            (deck(&dir, &by_like), deck(&dir, &by_enable))
        };
        let (mut a, mut b) = routes(&[]);
        assert_eq!(
            report(&mut a, "Export Powers"),
            report(&mut b, "Export Powers"),
            "{form}: Export Powers"
        );
        let (solved, solved_enable) = routes(&["Solve"]);
        assert_same_voltages(form, &solved, &solved_enable, "by Enable");
        assert!(elem_data(&solved, "Generator.e1").is_mapped(), "{form}");
        let (stepped, stepped_enable) = routes(&["_SolvePFlow"]);
        assert_same_voltages(form, &stepped, &stepped_enable, "by Enable");
        assert_same_voltages(form, &stepped, &solved, "after Solve");
        if *form == "a renumbered bus list" {
            assert_eq!(format!("{:.6}", wbus1(&solved)), "396.726539", "{form}");
            let sol = &solved.circuit().expect("circuit").solution;
            assert!(sol.converged_flag, "{form}: converged");
            for phase in 1..=3 {
                let node = format!("ABUS.{phase}");
                assert_eq!(node_magnitude(&solved, &node), 0.0, "{form}: {node}");
            }
            let mut solved = solved;
            let rows = rows_of(&report(&mut solved, "Export Powers"), "Generator.e1")
                .into_iter()
                .map(String::from)
                .collect::<Vec<_>>();
            assert_eq!(rows.len(), 1, "{form}: {rows:?}");
            assert!(
                numbers(&rows[0], 2).iter().all(|&v| v == 0.0),
                "{form}: {rows:?}"
            );
        }

        let mut dss = deck(&dir, &then(before, &[like, dyn_entry(5).as_str()]));
        let rows = rows_of(&report(&mut dss, "Export Powers"), "Generator.e1")
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>();
        assert_eq!(rows.len(), 1, "{form}: {rows:?}");
        assert!(
            numbers(&rows[0], 2).iter().all(|&v| v == 0.0),
            "{form}: {rows:?}"
        );
        let outside = outside_msg("Generator", "dynamic");
        assert_refused(&mut dss, form, "Solve", &outside, &[]);
        dss.command("Set mode=dynamic stepsize=0.001 number=5");
        assert_refused(&mut dss, form, "_SolvePFlow", &outside, &[]);

        let mut dss = deck(&dir, &then(before, &[dyn_entry(5).as_str(), like]));
        assert_refused(
            &mut dss,
            form,
            "Solve",
            &join_msg("Generator", "dynamic"),
            &[],
        );
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// MonteFault turns the fault it picks on as `Enable` does, so a fault whose
/// map a rebuild made stale is mapped on the present bus list before it is
/// stamped. A fault disabled at a solve that kept its node, faults taken out by
/// a fault study before a rebuild, a fault on a bus a rebuild renumbered, which
/// lands on its own phase, and a fault behind a Line dropped with it each solve
/// on the present bus list.
#[test]
fn monte_fault_maps_a_fault_a_rebuild_left_out() {
    let dir = scratch("rebuild_mf");
    let mf = ["Set mode=mf number=2", "Solve"];
    let dss = deck(
        &dir,
        &lines(&[&H, &[FLT], &PF, &["Disable Fault.f1", "Solve"], &mf]),
    );
    assert_eq!(format!("{:.6}", wbus1(&dss)), "27.850048", "kept");

    let dss = deck(
        &dir,
        &lines(&[
            &H,
            &[FLT],
            &PF,
            &[
                "Set mode=faultstudy",
                "Solve",
                "Set mode=snap",
                "New Load.l9 bus1=srcbus phases=3 kv=0.69 kW=10",
                "Solve",
            ],
            &mf,
        ]),
    );
    assert_eq!(format!("{:.6}", wbus1(&dss)), "27.849942", "fault study");

    let dss = deck(
        &dir,
        &lines(&[
            &[
                "Clear",
                "New Circuit.dp basekv=0.69 phases=3 bus1=srcbus",
                "New Fault.f1 bus1=wbus.2 phases=1 r=0.01",
                H[2],
                H[3],
            ],
            &PF,
            &["Disable Fault.f1", "Solve"],
            &mf,
        ]),
    );
    let fault = elem_data(&dss, "Fault.f1");
    assert!(fault.is_mapped(), "renumbered: no node map");
    let ckt = dss.circuit().expect("circuit");
    assert_eq!(ckt.node_name(fault.node_ref[0]), "WBUS.2", "renumbered");
    assert!(node_magnitude(&dss, "WBUS.2") < 30.0, "renumbered");
    assert!(node_magnitude(&dss, "WBUS.1") > 600.0, "renumbered");

    let dss = deck(
        &dir,
        &lines(&[
            &H,
            &[L2_GBUS, "New Fault.f1 bus1=gbus phases=1 r=0.01"],
            &PF,
            &["Disable Line.l2", "Disable Fault.f1", "Solve"],
            &mf,
        ]),
    );
    assert_eq!(format!("{:.6}", wbus1(&dss)), "395.359086", "dropped");
    std::fs::remove_dir_all(&dir).ok();
}

/// `_SolvePFlow` solves the circuit as it stands: while a bus redefinition is
/// pending or the node vectors do not hold the bus list, it rebuilds the bus
/// list, sizes the vectors and builds the system Y first, so it gives the node
/// voltages, the convergence and the iteration count of `Solve` on the same
/// circuit bit for bit, after edits that add, disable or re-phase elements, add
/// buses or leave the bus list ahead of the vectors. A three-phase delta
/// `cmatrix` bank, which has no primitive admittance, makes it report the error
/// of `Solve` and leave the circuit not solved, as `Solve` does.
#[test]
fn the_step_power_flow_needs_the_system_y_of_the_present_circuit() {
    let dir = scratch("step_pflow_y");
    let load = "New Load.l3 bus1=wbus phases=3 kv=0.69 kW=30 kvar=10";
    let source = "New Vsource.v2 bus1=wbus basekv=0.69 phases=3 pu=1.0";
    let new_bus = "New Line.l2 bus1=wbus bus2=nbus phases=3 r1=0.005 x1=0.02";
    let decks: [(&str, Vec<String>); 12] = [
        ("a Load disabled", lines(&[&H, &PF, &["Disable Load.ld"]])),
        (
            "a Capacitor added",
            lines(&[
                &H,
                &PF,
                &["New Capacitor.c1 bus1=wbus phases=3 kvar=100 kv=0.69"],
            ]),
        ),
        (
            "a Line added between existing buses",
            lines(&[
                &H,
                &PF,
                &["New Line.l9 bus1=srcbus bus2=wbus phases=3 r1=0.5 x1=0.2 length=1"],
            ]),
        ),
        (
            "a Generator re-phased to one phase",
            lines(&[&H, &[GEN], &PF, &["Edit Generator.e1 phases=1"]]),
        ),
        ("never built", lines(&[&H])),
        ("after _InitSnap", lines(&[&H, &["_InitSnap"]])),
        ("a Load added", lines(&[&H, &["Solve", load]])),
        ("a Vsource added", lines(&[&H, &["Solve", source]])),
        (
            "a Line to a new bus added",
            lines(&[&H, &["Solve", new_bus]]),
        ),
        (
            "a Line to a new bus, then MakeBusList",
            lines(&[&H, &["Solve", new_bus, "MakeBusList"]]),
        ),
        (
            "a Line to a new bus, then Show Isolated",
            lines(&[&H, &["Solve", new_bus, "Show Isolated"]]),
        ),
        (
            "a Generator enabled again on a bus the last solve dropped",
            dropped_bus("Generator", &[], GEN),
        ),
    ];
    for (case, before) in &decks {
        let stepped = deck(&dir, &then(before, &["_SolvePFlow"]));
        let solved = deck(&dir, &then(before, &["Solve"]));
        assert_same_voltages(case, &stepped, &solved, "after Solve");
        let (s, r) = (
            stepped.circuit().expect("circuit"),
            solved.circuit().expect("circuit"),
        );
        assert!(
            s.is_solved && s.solution.converged_flag,
            "{case}: not solved"
        );
        assert_eq!(
            (s.solution.converged_flag, s.solution.iteration),
            (r.solution.converged_flag, r.solution.iteration),
            "{case}"
        );
        assert_eq!(s.solution.node_v.len(), s.num_nodes + 1, "{case}: vectors");
    }
    assert_eq!(
        format!(
            "{:.6}",
            wbus1(&deck(
                &dir,
                &lines(&[&H, &PF, &["Disable Load.ld", "_SolvePFlow"]])
            ))
        ),
        "398.371691"
    );

    let bank = "New Capacitor.cd bus1=wbus phases=3 conn=delta cmatrix=[10 | -2 10 | -2 -2 10]";
    let refused = |cmd: &str| {
        let mut dss = deck(&dir, &lines(&[&H, &PF, &[bank]]));
        dss.command(cmd);
        let errors: Vec<(String, Option<u32>)> = dss
            .errors()
            .iter()
            .map(|e| (e.message.clone(), e.code))
            .collect();
        let ckt = dss.circuit().expect("circuit");
        (errors, ckt.is_solved, ckt.solution.solution_abort)
    };
    let want = refused("Solve");
    assert_eq!(
        want.0.last(),
        Some(&(
            "Error Encountered in Solve: an element has no defined primitive admittance"
                .to_string(),
            Some(482)
        )),
        "{want:?}"
    );
    assert_eq!((want.1, want.2), (false, true), "Solve on the bank");
    assert_eq!(refused("_SolvePFlow"), want, "_SolvePFlow on the bank");

    let dss = deck(&dir, &lines(&[&H, &PF[..2], &["_SolvePFlow"]]));
    assert_eq!(format!("{:.4}", wbus1(&dss)), "395.3591");
    std::fs::remove_dir_all(&dir).ok();
}
