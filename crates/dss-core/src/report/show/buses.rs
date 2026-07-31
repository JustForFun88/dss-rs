//! `Show Buses` (Pascal `ShowResults.pas` `ShowBuses`): every bus with its
//! line-to-line base kV, `(x, y)` coordinate, keep flag, node count, and the list
//! of node numbers in use.

use crate::circuit::Circuit;
use crate::report::format;
use crate::report::table::{Cell, Report, Row};
use crate::util::sqrt3;

/// Build the `Show Buses` text (Pascal `ShowBuses`). Read-only over the circuit's
/// bus list.
///
/// F-FMT step 2: the bus lines are [`Row`]s, so the lane renders the columns
/// ([`crate::compat::render_rows`]). The coordinate parentheses are cells of
/// their own — a `)` written flush against a coordinate that happens to fill its
/// 13-char field is the one shape in which the two kernels' tokens could differ,
/// and a cell keeps it a token in both. The two header lines stay free text:
/// Pascal spans them across the data columns (`Coord` sits over the `(`/`x`/`y`/
/// `)` group, `Number of` over one column), so no cell-per-column decomposition
/// of them exists; only a v2 re-layout (plan §F-FMT step 4) could move them.
pub(crate) fn show_buses(ckt: &Circuit) -> String {
    // Pascal `SetMaxBusNameLength; Inc(MaxBusNameLength, 2)`.
    let mbnl = super::max_bus_name_length(ckt) + 2;

    let mut rep = Report::new();
    rep.blank();
    rep.line(&format!("BUSES AND NODES IN ACTIVE CIRCUIT: {}", ckt.name));
    rep.blank();
    rep.line(&format!(
        "{}{}",
        format::pad("     ", mbnl),
        "                         Coord                                 Number of     Nodes"
    ));
    rep.line(&format!(
        "{}{}",
        format::pad("  Bus", mbnl),
        "    Base kV             (x, y)                      Keep?       Nodes        connected ..."
    ));
    rep.blank();

    for i in 0..ckt.buses.len() {
        let bus = &ckt.buses[i];
        let name = ckt.bus_list.name(i).unwrap_or("");
        let mut row = Row::new().cell(Cell::left(format::enclose_quotes(name), mbnl).sep(" "));
        // `Format('%7.3f')` when the base is set, else the literal `'   NA '` —
        // one space narrower, which is why each arm carries its own separator.
        row = if bus.kv_base > 0.0 {
            row.cell(Cell::right(format::fixed(bus.kv_base * sqrt3(), 3), 7).sep("          "))
        } else {
            row.cell(Cell::right("NA", 5).sep("           "))
        };
        row = if bus.coord_defined {
            row.cell(Cell::plain("(").sep(" "))
                .cell(Cell::left(format::g(bus.x, 11), 13).sep(", "))
                .cell(Cell::left(format::g(bus.y, 11), 13))
                .cell(Cell::plain(")").sep("     "))
        } else {
            // `'           NA,            NA )'`.
            row.cell(Cell::plain("(").sep("           "))
                .cell(Cell::plain("NA").sep(",            "))
                .cell(Cell::plain("NA").sep(" "))
                .cell(Cell::plain(")").sep("     "))
        };
        row = row
            .cell(Cell::plain(if bus.keep { "Yes" } else { "No" }).sep("       "))
            .cell(Cell::right(bus.num_nodes_this_bus().to_string(), 5).sep("       "));
        for j in 0..bus.num_nodes_this_bus() {
            row = row.cell(Cell::right(bus.get_num(j).to_string(), 4).sep(" "));
        }
        rep.row(row);
    }
    rep.finish()
}
