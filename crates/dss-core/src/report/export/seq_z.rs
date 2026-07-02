//! `Export SeqZ` (Pascal `ExportResults.pas` `ExportSeqZ`): the symmetrical-
//! component short-circuit impedances (`Zsc1`/`Zsc0`) at every bus.
//!
//! Reads the precomputed per-bus `Zsc` the FaultStudy solve populated
//! (`TDSSBus.Get_Zsc1`/`Get_Zsc0` = `Zs∓Zm`); zero on a plain snapshot (no
//! FaultStudy has allocated `Zsc`) — the same faithful behaviour as Pascal, which
//! reads whatever `Buses^[i].Zsc1` holds. No re-solve, no mutation.

use num_complex::Complex64;

use crate::circuit::Circuit;
use crate::report::format;
use crate::util::check_for_blanks;

/// Build the `Export SeqZ` body (Pascal `ExportSeqZ`): one row per bus with its
/// node count, `R1/X1/R0/X0`, `|Z1|/|Z0|`, and the `X/R` ratios.
pub fn export_seq_z(ckt: &Circuit) -> String {
    let mut s = String::new();
    // Pascal header (`ExportResults.pas:2837`).
    s.push_str("Bus,  NumNodes, R1, X1, R0, X0, Z1, Z0, \"X1/R1\", \"X0/R0\"\n");
    for i in 0..ckt.buses.len() {
        let bus = &ckt.buses[i];
        let z1 = bus.get_zsc1();
        let z0 = bus.get_zsc0();
        // Pascal: X/R = im/re, or the 1000.0 sentinel when re == 0.
        let x1r1 = if z1.re != 0.0 { z1.im / z1.re } else { 1000.0 };
        let x0r0 = if z0.re != 0.0 { z0.im / z0.re } else { 1000.0 };
        let name = check_for_blanks(&ckt.bus_list.name(i).unwrap_or("").to_uppercase());
        // Pascal `Format('"%s", %d, %10.6g, %10.6g, %10.6g, %10.6g, %10.6g,
        //   %10.6g, %8.4g, %8.4g', …)`.
        s.push_str(&format!(
            "\"{}\", {}, {}, {}, {}, {}, {}, {}, {}, {}\n",
            name,
            bus.num_nodes_this_bus(),
            format::g(z1.re, 6),
            format::g(z1.im, 6),
            format::g(z0.re, 6),
            format::g(z0.im, 6),
            format::g(cabs(z1), 6),
            format::g(cabs(z0), 6),
            format::g(x1r1, 4),
            format::g(x0r0, 4),
        ));
    }
    s
}

/// Pascal `Cabs` (magnitude of a complex).
fn cabs(c: Complex64) -> f64 {
    c.norm()
}
