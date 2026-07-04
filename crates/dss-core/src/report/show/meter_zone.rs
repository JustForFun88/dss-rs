//! `Show Loops` / `Show Zone` (Pascal `ShowResults.pas` `ShowLoops` /
//! `ShowMeterZone`): the two reports that walk an EnergyMeter's built zone tree
//! (`BranchList`). Both read the tree the WP6.4 zone build already populated —
//! its `First`/`GoForward` walk order is the persisted `sequence_list`, and each
//! branch's `PresentBranch` loop/parallel flags + shunt-object list are on the
//! matching tree node (`sequence_nodes[i]`). Read-only, no cursor mutation.
//!
//! - `ShowLoops`: every parallel/looped branch across **all** meter zones, one
//!   line each.
//! - `ShowMeterZone`: the named meter's zone as an indented branch/shunt tree.

use crate::circuit::Circuit;
use crate::elements::meter::EnergyMeter;
use crate::elements::traits::ElemRef;
use crate::exec::registry::DssClass;

/// Downcast a `ckt.energy_meters` ref to its concrete [`EnergyMeter`].
fn as_meter(classes: &[DssClass], r: ElemRef) -> &EnergyMeter {
    classes[r.cls].objects[r.idx]
        .as_any()
        .downcast_ref::<EnergyMeter>()
        .expect("energy_meters holds EnergyMeter")
}

/// Pascal `TDSSCktElement.FullName` = `ParentClass.Name + '.' + Name`.
fn full_name(classes: &[DssClass], r: ElemRef) -> String {
    format!(
        "{}.{}",
        classes[r.cls].props.class_name(),
        classes[r.cls].objects[r.idx].data().name()
    )
}

/// Pascal `TDSSCktElement.SensorObj` → `(Sensor: <FullName>) ` else `(Sensor: NIL)`.
/// Note the width difference the port reproduces 1:1: the assigned form has a
/// leading **and** trailing space (`Format(' (Sensor: %s) ', …)`); the NIL form
/// has only the leading space (`' (Sensor: NIL)'`).
fn sensor_note(classes: &[DssClass], sensor: Option<ElemRef>) -> String {
    match sensor {
        Some(r) => format!(" (Sensor: {}) ", full_name(classes, r)),
        None => " (Sensor: NIL)".to_string(),
    }
}

/// Build the `Show Loops` text (Pascal `ShowLoops`): for every EnergyMeter with a
/// built zone, one line per branch that is `IsParallel` and/or `IsLoopedHere`,
/// naming the meter, the branch (`Class.UPPERCASE(Name)`) and the loop/parallel
/// partner (`LoopLineObj.FullName`). Radial zones emit only the two header lines.
pub(crate) fn show_loops(classes: &[DssClass], ckt: &Circuit) -> String {
    let mut s = String::new();
    s.push_str("Loops and Paralleled Lines in all EnergyMeter Zones\n");
    s.push('\n');

    for &mr in &ckt.energy_meters {
        let m = as_meter(classes, mr);
        let Some(tree) = m.branch_list() else {
            continue;
        };
        let mtr_name = classes[mr.cls].objects[mr.idx].data().name().to_string();
        for (i, &br) in m.sequence_list().iter().enumerate() {
            let node = tree.node(m.sequence_nodes()[i]);
            // Pascal `Format('… %s.%s …', [ParentClass.Name, AnsiUpperCase(Name)])`.
            let cls = classes[br.cls].props.class_name();
            let name_up = classes[br.cls].objects[br.idx].data().name().to_uppercase();
            if node.is_parallel {
                let partner = node
                    .loop_elem
                    .map_or_else(String::new, |r| full_name(classes, r));
                s.push_str(&format!(
                    "({mtr_name}) {cls}.{name_up}: PARALLEL WITH {partner}\n"
                ));
            }
            if node.is_looped {
                let partner = node
                    .loop_elem
                    .map_or_else(String::new, |r| full_name(classes, r));
                s.push_str(&format!(
                    "({mtr_name}) {cls}.{name_up}: LOOPED TO     {partner}\n"
                ));
            }
        }
    }
    s
}

/// Build the `Show Zone` body for one meter (Pascal `ShowMeterZone`, the
/// `BranchList <> NIL` arm). Walks the zone tree in `First`/`GoForward` order:
/// each branch on its own `Level`-indented line (`Class.Name`, then any
/// `(PARALLEL:LoopLineObj.Name)` / `(LOOP:LoopLineObj.FullName)` + Sensor note),
/// then each shunt object attached at that branch on a `Level+1`-indented line.
/// The header + tab indentation match the Pascal layout byte-for-byte.
pub(crate) fn show_meter_zone(classes: &[DssClass], meter: ElemRef) -> String {
    let m = as_meter(classes, meter);
    let mtr_name = classes[meter.cls].objects[meter.idx].data().name();

    let mut s = String::new();
    // Pascal `FSWriteln(F, 'Branches and Load in Zone for EnergyMeter ', Param)`:
    // the trailing space is inside the literal.
    s.push_str(&format!(
        "Branches and Load in Zone for EnergyMeter {mtr_name}\n"
    ));
    s.push('\n');

    let Some(tree) = m.branch_list() else {
        // Pascal writes nothing past the header when `BranchList = NIL` (the arm
        // guards the whole body on `BranchList <> NIL`; a found-but-unbuilt meter
        // yields no body).
        return s;
    };

    for (i, &br) in m.sequence_list().iter().enumerate() {
        let node = tree.node(m.sequence_nodes()[i]);
        let level = node.level();
        for _ in 0..level {
            s.push('\t');
        }
        s.push_str(&format!(
            "{}.{}",
            classes[br.cls].props.class_name(),
            classes[br.cls].objects[br.idx].data().name()
        ));
        // PARALLEL uses `LoopLineObj.Name` (bare name); LOOP uses `.FullName`.
        if node.is_parallel {
            let partner = node.loop_elem.map_or_else(String::new, |r| {
                classes[r.cls].objects[r.idx].data().name().to_string()
            });
            s.push_str(&format!("(PARALLEL:{partner})"));
        }
        if node.is_looped {
            let partner = node
                .loop_elem
                .map_or_else(String::new, |r| full_name(classes, r));
            s.push_str(&format!("(LOOP:{partner})"));
        }
        let branch_sensor = classes[br.cls].objects[br.idx]
            .as_ckt_element()
            .and_then(|e| e.cd().sensor_obj);
        s.push_str(&sensor_note(classes, branch_sensor));
        s.push('\n');

        // The shunt objects attached at this branch (Pascal `FirstObject`/
        // `NextObject` over the present branch), one `Level+1`-indented line each.
        for &shunt in &node.shunts {
            for _ in 0..=level {
                s.push('\t');
            }
            s.push_str(&format!(
                "{}.{}",
                classes[shunt.cls].props.class_name(),
                classes[shunt.cls].objects[shunt.idx].data().name()
            ));
            let shunt_sensor = classes[shunt.cls].objects[shunt.idx]
                .as_ckt_element()
                .and_then(|e| e.cd().sensor_obj);
            s.push_str(&sensor_note(classes, shunt_sensor));
            s.push('\n');
        }
    }
    s
}
