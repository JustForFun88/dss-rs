//! `Show Faults` (Pascal `ShowResults.pas` `ShowFaultStudy` (:1848)): the
//! FaultStudy report — three per-bus sections over the **precomputed** short-circuit
//! state (`Zsc`/`Ysc`/`VBus`/`BusCurrent`) a prior `Solve mode=faultstudy` (WP7.9)
//! populated. Read-only (PHASE8_PLAN §2.1): the adjacent-node-node section does
//! local per-bus `YFault` scratch inversions (the same math as
//! `export_fault_study`), never a global re-solve or hidden mutation.
//!
//! The three sections:
//! 1. **All-Node Fault Currents** — per node the prospective bolted current
//!    (`|BusCurrent[i]|`, `%15.0f`) and `X/R` of the driving-point impedance
//!    (`Zbus = VBus[i] / BusCurrent[i]`, `%5.1f`, `N/A` when the current is 0).
//! 2. **One-Node to ground Faults** — per node the SLG fault current
//!    (`|VBus[iphs] / Zsc[iphs,iphs]|`) and the pu node voltages the fault leaves
//!    (`|VBus[i] − Zsc[i,iphs]·IFault|`, the faulted node ≈ 0).
//! 3. **Adjacent Node-Node Faults** — per node pair `(iphs < iphs2)` the L-L fault
//!    current and pu node voltages via a `GFault = 10000 S` scratch inversion of
//!    `Ysc`.

use num_complex::Complex64;

use crate::circuit::Circuit;
use crate::report::format;
use crate::support::cmatrix::{CMatrix, cdiv_fpc};
use crate::support::mathutil::get_xr;

/// Build the `Show Faults` text (Pascal `ShowFaultStudy`). Reads the precomputed
/// bus `Zsc`/`Ysc`/`VBus`/`BusCurrent`; a snapshot (no faultstudy run) leaves them
/// zero/unallocated → the degenerate output the oracle produces.
pub(crate) fn show_fault_study(ckt: &Circuit) -> String {
    let mbnl = super::max_bus_name_length(ckt);
    let gfault = Complex64::new(10000.0, 0.0);
    let mut s = String::new();

    // ── Section 1: All-Node Fault Currents ─────────────────────────────────
    s.push_str("FAULT STUDY REPORT\n");
    s.push('\n');
    s.push_str("ALL-Node Fault Currents\n");
    s.push('\n');
    // Header: `Pad('Bus',mbnl)` + 3×(`Node` + 12sp + `Amps` + 3sp + `X/R `), the
    // last group ending `X/R ...`.
    s.push_str(&format::pad("Bus", mbnl));
    for k in 0..3 {
        s.push_str("Node");
        s.push_str(&" ".repeat(12));
        s.push_str("Amps");
        s.push_str(&" ".repeat(3));
        s.push_str(if k == 2 { "X/R ..." } else { "X/R " });
    }
    s.push('\n');
    s.push('\n');

    for bus in &ckt.buses {
        let n = bus.num_nodes_this_bus();
        // `Pad(EncloseQuotes(UPPER(name)) + ' ', mbnl + 2)`.
        let name = bus.name.to_uppercase();
        s.push_str(&format::pad(
            &format!("{} ", format::enclose_quotes(&name)),
            mbnl + 2,
        ));
        for i in 0..n {
            if i > 0 {
                s.push_str("    ");
            }
            let curr_mag = bus.bus_current[i].norm();
            // `GetNum(i)` (no field width) then `CurrMag:15:0`.
            s.push_str(&bus.get_num(i).to_string());
            s.push_str(&format::fixed_w(curr_mag, 15, 0));
            if curr_mag > 0.0 {
                let zbus = cdiv_fpc(bus.vbus[i], bus.bus_current[i]);
                s.push(' ');
                s.push_str(&format::fixed_w(get_xr(zbus), 5, 1));
            } else {
                s.push_str("   N/A");
            }
        }
        s.push('\n');
    }
    s.push('\n');

    // ── Section 2: One-Node to ground Faults ───────────────────────────────
    s.push('\n');
    s.push_str("ONE-Node to ground Faults\n");
    s.push('\n');
    s.push_str("                                      pu Node Voltages (L-N Volts if no base)\n");
    s.push_str(&format::pad("Bus", mbnl));
    s.push_str("   Node  Amps         Node 1     Node 2     Node 3    ...\n");
    s.push('\n');

    for bus in &ckt.buses {
        let n = bus.num_nodes_this_bus();
        let Some(zsc) = &bus.zsc else { continue };
        let padded = format::pad(&format::enclose_quotes(&bus.name.to_uppercase()), mbnl + 2);
        for iphs in 0..n {
            // `IFault := VBus[iphs] / Zsc[iphs,iphs]` (FPC `ucomplex` `/`).
            let ifault = cdiv_fpc(bus.vbus[iphs], zsc.get(iphs, iphs));
            // `Format('%s %4u %12.0f ', …)` then `'   '`.
            s.push_str(&padded);
            s.push(' ');
            s.push_str(&format::fixed_w_int(bus.get_num(iphs) as i64, 4));
            s.push(' ');
            s.push_str(&format::fixed_w(ifault.norm(), 12, 0));
            s.push_str("    "); // the '%12.0f ' trailing space + the '   ' literal
            for i in 0..n {
                let vphs = (bus.vbus[i] - zsc.get(i, iphs) * ifault).norm();
                s.push(' ');
                if bus.kv_base > 0.0 {
                    s.push_str(&format::fixed_w(0.001 * vphs / bus.kv_base, 10, 3));
                } else {
                    s.push_str(&format::fixed_w(vphs, 10, 1));
                }
            }
            s.push('\n');
        }
    }

    // ── Section 3: Adjacent Node-Node Faults ───────────────────────────────
    s.push('\n');
    s.push_str("Adjacent Node-Node Faults\n");
    s.push('\n');
    s.push_str("                                        pu Node Voltages (L-N Volts if no base)\n");
    s.push_str("Bus          Node-Node      Amps        Node 1     Node 2     Node 3    ...\n");
    s.push('\n');

    for bus in &ckt.buses {
        let n = bus.num_nodes_this_bus();
        let Some(ysc) = &bus.ysc else { continue };
        let padded = format::pad(&format::enclose_quotes(&bus.name.to_uppercase()), mbnl + 2);
        for iphs in 0..n {
            for iphs2 in 0..n {
                if iphs >= iphs2 {
                    continue;
                }
                // Stamp `GFault` at the two faulted nodes, invert, read the fault
                // voltage `VFault = YFault⁻¹ · BusCurrent`.
                let mut yfault = CMatrix::new(n);
                yfault.copy_from(ysc);
                yfault.add(iphs, iphs, gfault);
                yfault.add(iphs2, iphs2, gfault);
                yfault.add_sym(iphs, iphs2, -gfault);
                let _ = yfault.invert();
                let mut vfault = vec![Complex64::ZERO; n];
                yfault.mv_mult(&mut vfault, &bus.bus_current);
                let iamps = ((vfault[iphs] - vfault[iphs2]) * gfault).norm();
                // `Pad(name,mbnl+2), GetNum(iphs):4, GetNum(iphs2):4, Cabs():12:0, '   '`.
                s.push_str(&padded);
                s.push_str(&format::fixed_w_int(bus.get_num(iphs) as i64, 4));
                s.push_str(&format::fixed_w_int(bus.get_num(iphs2) as i64, 4));
                s.push_str(&format::fixed_w(iamps, 12, 0));
                s.push_str("   ");
                for vf in &vfault {
                    let vphs = vf.norm();
                    s.push(' ');
                    if bus.kv_base > 0.0 {
                        s.push_str(&format::fixed_w(0.001 * vphs / bus.kv_base, 10, 3));
                    } else {
                        s.push_str(&format::fixed_w(vphs, 10, 1));
                    }
                }
                s.push('\n');
            }
        }
    }

    s
}
