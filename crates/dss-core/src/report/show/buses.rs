//! `Show Buses` (Pascal `ShowResults.pas` `ShowBuses`): every bus with its
//! line-to-line base kV, `(x, y)` coordinate, keep flag, node count, and the list
//! of node numbers in use.

use crate::circuit::Circuit;
use crate::report::format;
use crate::util::sqrt3;

/// Build the `Show Buses` text (Pascal `ShowBuses`). Read-only over the circuit's
/// bus list.
pub(crate) fn show_buses(ckt: &Circuit) -> String {
    // Pascal `SetMaxBusNameLength; Inc(MaxBusNameLength, 2)`.
    let mbnl = super::max_bus_name_length(ckt) + 2;

    let mut s = String::new();
    s.push('\n');
    s.push_str(&format!(
        "BUSES AND NODES IN ACTIVE CIRCUIT: {}\n",
        ckt.name
    ));
    s.push('\n');
    s.push_str(&format::pad("     ", mbnl));
    s.push_str(
        "                         Coord                                 Number of     Nodes\n",
    );
    s.push_str(&format::pad("  Bus", mbnl));
    s.push_str("    Base kV             (x, y)                      Keep?       Nodes        connected ...\n");
    s.push('\n');

    for i in 0..ckt.buses.len() {
        let bus = &ckt.buses[i];
        let name = ckt.bus_list.name(i).unwrap_or("");
        s.push_str(&format::pad(&format::enclose_quotes(name), mbnl));
        s.push(' ');
        if bus.kv_base > 0.0 {
            s.push_str(&format::fixed_w(bus.kv_base * sqrt3(), 7, 3));
        } else {
            s.push_str("   NA ");
        }
        s.push_str("          (");
        if bus.coord_defined {
            s.push_str(&format!(
                " {}, {})",
                format::g_left_w(bus.x, 13, 11),
                format::g_left_w(bus.y, 13, 11)
            ));
        } else {
            s.push_str("           NA,            NA )");
        }
        if bus.keep {
            s.push_str("     Yes  ");
        } else {
            s.push_str("     No  ");
        }
        s.push_str("     ");
        s.push_str(&format::fixed_w_int(bus.num_nodes_this_bus() as i64, 5));
        s.push_str("       ");
        for j in 0..bus.num_nodes_this_bus() {
            s.push_str(&format!(
                "{} ",
                format::fixed_w_int(bus.get_num(j) as i64, 4)
            ));
        }
        s.push('\n');
    }
    s
}
