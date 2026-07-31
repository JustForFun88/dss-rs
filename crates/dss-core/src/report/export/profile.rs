//! `Export Profile` (Pascal `ExportResults.pas` `ExportProfile` (:3103) +
//! `WriteNewLine` (:3088)): the per-meter branch-list voltage profile — one row
//! per plotted phase of every **Line** branch in every EnergyMeter's zone, with
//! the pu voltage at both terminal buses vs their `DistFromMeter`, plus the GUI
//! plot-styling columns (color/thickness/linetype/markers) the headless report
//! simply echoes.
//!
//! Read-only over the solved circuit: the branch list (`SequenceList` — the
//! `BranchList.First`/`GoForward` order), the zone-build `DistFromMeter`, and
//! `NodeV`. A meter whose zone was never built contributes no rows (Pascal
//! `BranchList.First` on an empty tree).

use crate::circuit::{Bus, Circuit};
use crate::compat;
use crate::elements::meter::EnergyMeter;
use crate::elements::pd::line::Line;
use crate::exec::registry::DssClass;
use crate::report::format;

/// Pascal `TPlotPhases` ordinals (`DSSClass.pas:326`) — the `PhasesToPlot`
/// selector values; a **positive** value is a single explicit phase number.
pub(crate) mod plot_phases {
    pub const THREE_PHASE: i32 = -1;
    pub const ALL: i32 = -2;
    pub const PRIMARY: i32 = -3;
    pub const LL_3PH: i32 = -4;
    pub const LL_ALL: i32 = -5;
    pub const LL_PRIMARY: i32 = -6;
}

/// Pascal `WriteNewLine`: `%s, %.6g, %.6g, %.6g, %.6g,` + `%d, %d, %d, ` +
/// `%d, ` + `%d, %d, %d` + newline (note: **no** space between the `puV2`
/// comma and the color code — the four `FSWrite` calls concatenate verbatim).
#[allow(clippy::too_many_arguments)]
fn write_new_line(
    s: &mut String,
    name: &str,
    dist1: f64,
    pu_v1: f64,
    dist2: f64,
    pu_v2: f64,
    color: i32,
    thickness: i32,
    linetype: i32,
    mark_center: i32,
    center_code: i32,
    node_code: i32,
    node_width: i32,
) {
    s.push_str(&format!(
        "{}, {}, {}, {}, {},{}, {}, {}, {}, {}, {}, {}\n",
        name.to_uppercase(),
        format::g(dist1, 6),
        format::g(pu_v1, 6),
        format::g(dist2, 6),
        format::g(pu_v2, 6),
        color,
        thickness,
        linetype,
        mark_center,
        center_code,
        node_code,
        node_width,
    ));
}

/// Build the `Export Profile` body (Pascal `ExportProfile`): the header line
/// (column names + the appended `Title=…` tail on the same line), then the
/// per-line rows for each meter's zone, phase-selected by `phases_to_plot`.
pub(crate) fn export_profile(classes: &[DssClass], ckt: &Circuit, phases_to_plot: i32) -> String {
    use plot_phases::*;

    let mut s = String::from(
        "Name, Distance1, puV1, Distance2, puV2, Color, Thickness, Linetype, Markcenter, \
         Centercode, NodeCode, NodeWidth,",
    );
    let title = match phases_to_plot {
        LL_3PH | LL_ALL | LL_PRIMARY => "L-L Voltage Profile",
        _ => "L-N Voltage Profile",
    };
    s.push_str(&format!("Title={title}, Distance in km\n"));

    let node_v = &ckt.solution.node_v;
    let nmc = ckt.node_marker_code;
    let nmw = ckt.node_marker_width;
    // Pascal `Bus.GetRef(Bus.FindIdx(iphs))`: a missing phase falls through to
    // the ground reference 0 (`NodeV[0]` = 0 V) in the unguarded ThreePhase arm.
    let vref = |bus: &Bus, ph: i32| bus.find_idx(ph).map_or(0, |i| bus.get_ref(i));
    // L-N pu voltage of phase `ph` at `bus`: `CABS(NodeV[ref]) / kVBase / 1000`.
    let pu_ln = |bus: &Bus, ph: i32| node_v[vref(bus, ph)].norm() / bus.kv_base / 1000.0;
    // L-L pu voltage between phases `ph`/`ph2`: `CABS(V1 − V2) / kVBase / 1732`
    // upstream. `kVBase` is line-to-neutral, so the correct divisor is the
    // `1000·√3` the L-N arm above spells `1000.0`; the parity lane keeps the
    // truncated literal (`compat::profile_ll_pu_divisor`, F.3z).
    let ll_divisor = compat::profile_ll_pu_divisor();
    let pu_ll = |bus: &Bus, ph: i32, ph2: i32| {
        (node_v[vref(bus, ph)] - node_v[vref(bus, ph2)]).norm() / bus.kv_base / ll_divisor
    };

    for &m in &ckt.energy_meters {
        let em = classes[m.class_ord()]
            .arena
            .get::<EnergyMeter>(m.index())
            .expect("energy_meters holds EnergyMeter objects");
        // `BranchList.First`/`GoForward` == the persisted `SequenceList` order.
        for &r in em.sequence_list() {
            let obj = &classes[r.class_ord()].arena[r.index()];
            // Pascal `IslineElement(PresentCktElement)`.
            let Some(line) = classes[r.class_ord()].arena.get::<Line>(r.index()) else {
                continue;
            };
            let cd = &line.cd;
            let bus1 = &ckt.buses[cd.terminals[0].bus_idx()];
            let bus2 = &ckt.buses[cd.terminals[1].bus_idx()];
            if !(bus1.kv_base > 0.0 && bus2.kv_base > 0.0) {
                continue;
            }
            let name = obj.data().name();
            // `Linetype` = 2 (dashed) on a secondary (< 1 kV base) segment.
            let linetype = if bus1.kv_base < 1.0 { 2 } else { 0 };
            let has_ph = |ph: i32| bus1.find_idx(ph).is_some() && bus2.find_idx(ph).is_some();

            match phases_to_plot {
                THREE_PHASE => {
                    // 3-phase primary lines only; phases 1..3 written unguarded
                    // (a missing phase reads ground → puV 0), Linetype fixed 0.
                    if cd.nphases >= 3 && bus1.kv_base > 1.0 {
                        for iphs in 1..=3 {
                            write_new_line(
                                &mut s,
                                name,
                                bus1.dist_from_meter,
                                pu_ln(bus1, iphs),
                                bus2.dist_from_meter,
                                pu_ln(bus2, iphs),
                                iphs,
                                2,
                                0,
                                0,
                                0,
                                nmc,
                                nmw,
                            );
                        }
                    }
                }
                ALL | PRIMARY => {
                    // Every phase present at both ends (PRIMARY: > 1 kV only).
                    if phases_to_plot == ALL || bus1.kv_base > 1.0 {
                        for iphs in 1..=3 {
                            if has_ph(iphs) {
                                write_new_line(
                                    &mut s,
                                    name,
                                    bus1.dist_from_meter,
                                    pu_ln(bus1, iphs),
                                    bus2.dist_from_meter,
                                    pu_ln(bus2, iphs),
                                    iphs,
                                    2,
                                    linetype,
                                    0,
                                    0,
                                    nmc,
                                    nmw,
                                );
                            }
                        }
                    }
                }
                LL_3PH | LL_ALL | LL_PRIMARY => {
                    // Line-to-line: phase pairs 1-2, 2-3, 3-1 present at both
                    // ends. LL3Ph adds the `NPhases >= 3` guard; LLPrimary the
                    // `> 1 kV` guard; LLAll neither.
                    let selected = match phases_to_plot {
                        LL_3PH => cd.nphases >= 3,
                        LL_PRIMARY => bus1.kv_base > 1.0,
                        _ => true,
                    };
                    if selected {
                        for iphs in 1..=3 {
                            let iphs2 = if iphs == 3 { 1 } else { iphs + 1 };
                            if has_ph(iphs) && has_ph(iphs2) {
                                write_new_line(
                                    &mut s,
                                    name,
                                    bus1.dist_from_meter,
                                    pu_ll(bus1, iphs, iphs2),
                                    bus2.dist_from_meter,
                                    pu_ll(bus2, iphs, iphs2),
                                    iphs,
                                    2,
                                    linetype,
                                    0,
                                    0,
                                    nmc,
                                    nmw,
                                );
                            }
                        }
                    }
                }
                iphs => {
                    // A single explicit phase number (`export profile 2`).
                    if has_ph(iphs) {
                        write_new_line(
                            &mut s,
                            name,
                            bus1.dist_from_meter,
                            pu_ln(bus1, iphs),
                            bus2.dist_from_meter,
                            pu_ln(bus2, iphs),
                            iphs,
                            2,
                            linetype,
                            0,
                            0,
                            nmc,
                            nmw,
                        );
                    }
                }
            }
        }
    }
    s
}
