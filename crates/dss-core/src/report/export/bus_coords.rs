//! `Export BusCoords` (Pascal `ExportResults.pas` `ExportBusCoords`): the X/Y
//! coordinate of every bus that has one, in bus order. No header row.

use crate::circuit::Circuit;
use crate::report::format;
use crate::util::check_for_blanks;

/// Build the `Export BusCoords` body (Pascal `ExportBusCoords`).
pub fn export_bus_coords(ckt: &Circuit) -> String {
    let mut s = String::new();
    for i in 0..ckt.buses.len() {
        let bus = &ckt.buses[i];
        if bus.coord_defined {
            // Pascal `Format('%s, %-13.11g, %-13.11g',
            //   [CheckForBlanks(AnsiUpperCase(BusList.NameOfIndex(i))), X, Y])`.
            let name = check_for_blanks(&ckt.bus_list.name(i).unwrap_or("").to_uppercase());
            s.push_str(&format!(
                "{}, {}, {}\n",
                name,
                format::g(bus.x, 11),
                format::g(bus.y, 11)
            ));
        }
    }
    s
}
