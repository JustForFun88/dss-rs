//! Pascal `TEnergyMeterObj.InterpolateCoordinates` +
//! `CalcBusCoordinates` (`Meters/EnergyMeter.pas:2298-2409`): fill in missing
//! bus coordinates along each meter zone by walking from every zone end
//! toward the meter, spacing the in-between buses EVENLY between the two
//! nearest coordinate-defined anchor buses. Free function over the element
//! store (the `solution::meters` pattern): the meter's branch tree is taken
//! out for the walk and put back at the end.

use crate::circuit::Circuit;
use crate::circuit::ckt_tree::CktTree;
use crate::elements::ckt::ElemFlags;
use crate::elements::traits::{ElemRef, ElemStore};

use super::downcast_meter;

/// Safe read of `buses[i].CoordDefined`. Pascal indexes `buses[BusRef]`
/// unguarded; a node with no from-bus (`NO_BUS`, Pascal `0`) would read
/// `buses[0]` — out of the 1-based array, an undefined heap read. UB is never
/// reproduced (CLAUDE.md known-bug rule): treat it as "no coordinate".
fn coord_defined(ckt: &Circuit, bus: usize) -> bool {
    ckt.buses.get(bus).is_some_and(|b| b.coord_defined)
}

/// Pascal `TEnergyMeterObj.InterpolateCoordinates` (`EnergyMeter.pas:2298`):
/// start at the ends of the zone and work toward the start, interpolating
/// between known coordinates.
pub(crate) fn interpolate_coordinates(
    meter_ref: ElemRef,
    ckt: &mut Circuit,
    store: &mut dyn ElemStore,
    errors: &mut Vec<String>,
) {
    // Pascal `CheckBranchList(529)`.
    let Some(tree) = downcast_meter(store, meter_ref).take_branch_list() else {
        errors
            .push("Meter Zone Lists need to be built. Do Solve or Makebuslist first!".to_string());
        return;
    };

    for i in 0..tree.zone_ends.num_ends() {
        // `Busref := Branchlist.ZoneEndsList.Get(i, PresentNode)`.
        let (end_node, bus_ref) = tree.zone_ends.ends[i];
        let mut present: Option<usize> = Some(end_node);

        let mut first_coord_ref = bus_ref;
        let mut second_coord_ref = first_coord_ref; // Pascal: "so compiler won't issue stupid warning"

        // Find a bus with a coordinate.
        if !coord_defined(ckt, bus_ref) {
            while let Some(p) = present {
                if coord_defined(ckt, tree.node(p).from_bus) {
                    break;
                }
                present = tree.node(p).parent();
            }
            if let Some(p) = present {
                first_coord_ref = tree.node(p).from_bus;
            }
        }

        while let Some(start_node) = present {
            // Back up until we find another Coord defined.
            let mut line_count = 0usize; // number of line segments in this segment
            let mut ckt_elem = tree.node(start_node).elem;
            if first_coord_ref != tree.node(start_node).from_bus {
                // Handle special case for end branch.
                if coord_defined(ckt, tree.node(start_node).from_bus) {
                    first_coord_ref = tree.node(start_node).from_bus;
                } else {
                    line_count += 1;
                }
            }

            let mut p = start_node;
            loop {
                store
                    .ckt_elem_mut(ckt_elem)
                    .cd_mut()
                    .flags
                    .include(ElemFlags::CHECKED);
                present = tree.node(p).parent();
                let Some(np) = present else { break };
                p = np;
                ckt_elem = tree.node(p).elem;
                second_coord_ref = tree.node(p).from_bus;
                line_count += 1;
                let checked = store
                    .ckt_elem(ckt_elem)
                    .cd()
                    .flags
                    .contains(ElemFlags::CHECKED);
                if coord_defined(ckt, second_coord_ref) || checked {
                    break;
                }
            }

            if present.is_some() && line_count > 1 {
                if coord_defined(ckt, second_coord_ref) {
                    calc_bus_coordinates(
                        &tree,
                        start_node,
                        first_coord_ref,
                        second_coord_ref,
                        line_count,
                        ckt,
                    );
                } else {
                    break; // while — went as far as we could go this way
                }
            }

            first_coord_ref = second_coord_ref;
        }
    }

    downcast_meter(store, meter_ref).put_branch_list(tree);
}

/// Pascal `TEnergyMeterObj.CalcBusCoordinates` (`EnergyMeter.pas:2371`): space
/// the buses between the two anchors evenly (`Xinc = (X1-X2)/LineCount`),
/// walking up the parent chain from `start_branch`.
fn calc_bus_coordinates(
    tree: &CktTree,
    start_branch: usize,
    first_coord_ref: usize,
    second_coord_ref: usize,
    mut line_count: usize,
    ckt: &mut Circuit,
) {
    if line_count == 1 {
        return; // Nothing to do!
    }

    let (x1, y1) = (ckt.buses[first_coord_ref].x, ckt.buses[first_coord_ref].y);
    let (x2, y2) = (ckt.buses[second_coord_ref].x, ckt.buses[second_coord_ref].y);
    let xinc = (x1 - x2) / line_count as f64;
    let yinc = (y1 - y2) / line_count as f64;

    let mut x = x1;
    let mut y = y1;
    let mut branch = start_branch;

    // Either start with the "to" end of StartNode or the "from" end.
    if first_coord_ref != tree.node(branch).from_bus {
        // Start with "to" end.
        x -= xinc;
        y -= yinc;
        if let Some(bus) = ckt.buses.get_mut(tree.node(branch).from_bus) {
            bus.x = x;
            bus.y = y;
            bus.coord_defined = true;
        }
        line_count -= 1;
    }

    while line_count > 1 {
        x -= xinc;
        y -= yinc;
        // Back up the tree (the interpolation scan visited every parent on
        // this segment, so the chain cannot end here — Pascal dereferences it
        // unguarded).
        branch = tree
            .node(branch)
            .parent()
            .expect("segment parent visited by the interpolation scan");
        if let Some(bus) = ckt.buses.get_mut(tree.node(branch).from_bus) {
            bus.x = x;
            bus.y = y;
            bus.coord_defined = true;
        }
        line_count -= 1;
    }
}
