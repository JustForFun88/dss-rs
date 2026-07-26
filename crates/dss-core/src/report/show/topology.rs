//! `Show Topology` (Pascal `ShowResults.pas` `ShowTopology`): the circuit's
//! connected-branch tree (for switch-control algorithms) + a summary count of
//! levels / loops / parallels / isolated PD components / controlled switches. Writes
//! two files — `<root>TopoTree.txt` (the indented branch/shunt tree, the
//! `@lastshowfile`-less companion) and `<root>TopoSumm.txt` (the counts, which the
//! dispatcher `@lastshowfile`s).
//!
//! Builds the circuit-wide topology tree via [`get_topology`](crate::solution::topology)
//! (mutating `CHECKED`/`IS_ISOLATED`/`bus_checked` as Pascal does), then walks it.

use crate::circuit::Circuit;
use crate::elements::ckt::ElemFlags;
use crate::elements::control::swt_control::SwtControl;
use crate::elements::traits::ElemId;
use crate::exec::registry::{ClassStore, DssClass};
use crate::solution::topology::get_topology;

/// Pascal `TDSSCktElement.FullName` = `ParentClass.Name + '.' + Name`.
fn full_name(classes: &[DssClass], r: ElemId) -> String {
    format!(
        "{}.{}",
        classes[r.class_ord()].props.class_name(),
        classes[r.class_ord()].arena[r.index()].data().name()
    )
}

/// Pascal `TopoLevelTabs`: `min(nLevel, 30)` `TABCHAR`s, then `(* nLevel *)` when
/// the level exceeds 30.
fn topo_level_tabs(s: &mut String, level: i32) {
    let n_tabs = level.clamp(0, 30);
    for _ in 0..n_tabs {
        s.push('\t');
    }
    if level > 30 {
        s.push_str(&format!("(* {level} *)"));
    }
}

/// The controls acting on element `r` (Pascal `r.ControlElementList`), derived from
/// `ckt.controls` in creation order (see `CktElement::controlled_element`).
fn controls_of(classes: &[DssClass], ckt: &Circuit, r: ElemId) -> Vec<ElemId> {
    ckt.controls
        .iter()
        .copied()
        .filter(|&cr| {
            classes[cr.class_ord()]
                .arena
                .try_ckt_elem(cr.index())
                .and_then(|ce| ce.controlled_element())
                == Some(r)
        })
        .collect()
}

/// Whether the control at `cr` is a SwtControl (Pascal `(DSSObjType and CLASSMASK) =
/// SWT_CONTROL`), counted into `nSwitches`.
fn is_swt_control(classes: &[DssClass], cr: ElemId) -> bool {
    classes[cr.class_ord()]
        .arena
        .get::<SwtControl>(cr.index())
        .is_some()
}

/// The `(Sensor: …)` / `(Control: …)` / `(Meter: …)` annotations shared by the
/// branch and shunt writers; increments `n_switches` per SwtControl. Returns the
/// annotation string.
fn annotations(classes: &[DssClass], ckt: &Circuit, r: ElemId, n_switches: &mut i32) -> String {
    let cd = classes[r.class_ord()]
        .arena
        .try_ckt_elem(r.index())
        .map(|e| e.cd());
    let mut s = String::new();
    if let Some(cd) = cd {
        if cd.flags.contains(ElemFlags::HAS_SENSOR_OBJ)
            && let Some(so) = cd.sensor_obj
        {
            s.push_str(&format!(" (Sensor: {}) ", full_name(classes, so)));
        }
        // Pascal `Flg.HasControl` — derived from the control refs (the flag is not
        // materialised in the port; see `CktElement::controlled_element`).
        for cr in controls_of(classes, ckt, r) {
            s.push_str(&format!(" (Control: {}) ", full_name(classes, cr)));
            if is_swt_control(classes, cr) {
                *n_switches += 1;
            }
        }
        if cd.flags.contains(ElemFlags::HAS_ENERGY_METER)
            && let Some(mo) = cd.meter_obj
        {
            s.push_str(&format!(
                " (Meter: {}) ",
                classes[mo.class_ord()].arena[mo.index()].data().name()
            ));
        }
    }
    s
}

/// Build the two `Show Topology` files (Pascal `ShowTopology`): returns
/// `(summary, tree)`.
pub(crate) fn show_topology(classes: &mut [DssClass], ckt: &mut Circuit) -> (String, String) {
    // Build the circuit-wide topology tree (mutates element/bus flags), then read
    // names/flags from `classes` once the mutable store borrow ends.
    let mut tree = {
        let mut store = ClassStore { classes };
        get_topology(ckt, &mut store)
    };

    let mut summ = String::new();
    let mut ftree = String::new();
    summ.push_str("Topology analysis for switch control algorithms\n");
    summ.push('\n');
    ftree.push_str(&format!("Branches and Loads in Circuit {}\n", ckt.name));
    ftree.push('\n');

    let (mut n_loops, mut n_parallel, mut n_levels, mut n_isolated, mut n_switches) =
        (0i32, 0i32, 0i32, 0i32, 0i32);

    // Walk the tree: each PD branch, then its shunt (Load) objects.
    let mut pd = tree.first();
    while let Some(pd_ref) = pd {
        let level = tree.level();
        if level > n_levels {
            n_levels = level;
        }
        topo_level_tabs(&mut ftree, level);
        ftree.push_str(&format!(
            "{}.{}",
            classes[pd_ref.class_ord()].props.class_name(),
            classes[pd_ref.class_ord()].arena[pd_ref.index()]
                .data()
                .name()
        ));
        // PresentBranch loop/parallel flags.
        let (is_parallel, is_looped, loop_elem) = {
            let node = tree.present_node();
            (node.is_parallel, node.is_looped, node.loop_elem)
        };
        if is_parallel {
            n_parallel += 1;
            if let Some(le) = loop_elem {
                ftree.push_str(&format!("(PARALLEL:{})", full_name(classes, le)));
            }
        }
        if is_looped {
            n_loops += 1;
            if let Some(le) = loop_elem {
                ftree.push_str(&format!("(LOOP:{})", full_name(classes, le)));
            }
        }
        ftree.push_str(&annotations(classes, ckt, pd_ref, &mut n_switches));
        ftree.push('\n');

        // Shunt (Load) objects on this branch.
        let shunts = tree.present_node().shunts.clone();
        for load_ref in shunts {
            topo_level_tabs(&mut ftree, level + 1);
            ftree.push_str(&format!(
                "{}.{}",
                classes[load_ref.class_ord()].props.class_name(),
                classes[load_ref.class_ord()].arena[load_ref.index()]
                    .data()
                    .name()
            ));
            ftree.push_str(&annotations(classes, ckt, load_ref, &mut n_switches));
            ftree.push('\n');
        }

        pd = tree.go_forward();
    }

    // Isolated PD elements (Pascal walks `PDElements`, `Flg.IsIsolated`).
    for &pd_ref in &ckt.pd_elements {
        let isolated = classes[pd_ref.class_ord()]
            .arena
            .try_ckt_elem(pd_ref.index())
            .is_some_and(|e| e.cd().flags.contains(ElemFlags::IS_ISOLATED));
        if isolated {
            ftree.push_str(&format!("Isolated: {}", full_name(classes, pd_ref)));
            ftree.push_str(&annotations(classes, ckt, pd_ref, &mut n_switches));
            ftree.push('\n');
            n_isolated += 1;
        }
    }

    n_loops /= 2; // Pascal `nLoops div 2` (each loop counted from both ends).
    summ.push_str(&format!("{n_levels} Levels Deep\n"));
    summ.push_str(&format!("{n_loops} Loops\n"));
    summ.push_str(&format!("{n_parallel} Parallel PD elements\n"));
    summ.push_str(&format!("{n_isolated} Isolated PD components\n"));
    summ.push_str(&format!("{n_switches} Controlled Switches\n"));

    (summ, ftree)
}
