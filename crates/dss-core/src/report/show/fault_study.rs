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
use crate::compat;
use crate::report::format;
use crate::report::table::{Cell, Report, Row};
use crate::support::cmatrix::CMatrix;
use crate::support::mathutil::get_xr;

/// Build the `Show Faults` text (Pascal `ShowFaultStudy`). Reads the precomputed
/// bus `Zsc`/`Ysc`/`VBus`/`BusCurrent`; a snapshot (no faultstudy run) leaves them
/// zero/unallocated → the degenerate output the oracle produces.
pub(crate) fn show_fault_study(ckt: &Circuit) -> String {
    let mbnl = super::max_bus_name_length(ckt);
    let gfault = Complex64::new(10000.0, 0.0);
    let mut rep = Report::new();

    // ── Section 1: All-Node Fault Currents ─────────────────────────────────
    rep.line("FAULT STUDY REPORT");
    rep.blank();
    rep.line("ALL-Node Fault Currents");
    rep.blank();
    // Header: `Pad('Bus',mbnl)` + 3×(`Node` + 12sp + `Amps` + 3sp + `X/R `), the
    // last group ending `X/R ...` — one cell per column the rows draw (each bus
    // repeats the `node / amps / X-over-R` group per node).
    let mut hdr = Row::new().cell(Cell::left("Bus", mbnl));
    for k in 0..3 {
        hdr = hdr
            .cell(Cell::plain("Node").sep("            "))
            .cell(Cell::plain("Amps").sep("   "));
        hdr = if k == 2 {
            hdr.cell(Cell::plain("X/R").sep(" "))
                .cell(Cell::plain("..."))
        } else {
            hdr.cell(Cell::plain("X/R").sep(" "))
        };
    }
    rep.row(hdr);
    rep.row(Row::blank(10));

    for bus in &ckt.buses {
        let n = bus.num_nodes_this_bus();
        // `Pad(EncloseQuotes(UPPER(name)) + ' ', mbnl + 2)` — the trailing space
        // is the column gutter, so the quoted name fills `mbnl + 1` (`Pad` only
        // appends; the bytes are the same).
        let name = bus.name.to_uppercase();
        let mut row = Row::new().cell(Cell::left(format::enclose_quotes(&name), mbnl + 1).sep(" "));
        for i in 0..n {
            let curr_mag = bus.bus_current[i].norm();
            // `GetNum(i)` (no field width) then `CurrMag:15:0`.
            row = row.cell(Cell::plain(bus.get_num(i).to_string()));
            let amps = Cell::right(format::fixed(curr_mag, 0), 15);
            // The next node's group is four spaces along; the last carries none.
            let tail_sep = if i + 1 < n { "    " } else { "" };
            if curr_mag > 0.0 {
                let zbus = compat::cdiv(bus.vbus[i], bus.bus_current[i]);
                row = row
                    .cell(amps.sep(" "))
                    .cell(Cell::right(format::fixed(get_xr(zbus), 1), 5).sep(tail_sep));
            } else {
                // `'   N/A'` — the same six-wide field, right-justified.
                row = row.cell(amps).cell(Cell::right("N/A", 6).sep(tail_sep));
            }
        }
        rep.row(row);
    }
    rep.blank();

    // The pu-voltage cell of both fault sections: `%10.3f` of the pu magnitude, or
    // the raw volts at `%10.1f` when the bus has no kV base.
    let pu_cell = |kv_base: f64, vphs: f64| {
        if kv_base > 0.0 {
            Cell::right(format::fixed(0.001 * vphs / kv_base, 3), 10)
        } else {
            Cell::right(format::fixed(vphs, 1), 10)
        }
    };

    // ── Section 2: One-Node to ground Faults ───────────────────────────────
    rep.blank();
    rep.line("ONE-Node to ground Faults");
    rep.blank();
    rep.line("                                      pu Node Voltages (L-N Volts if no base)");
    rep.row(
        Row::new()
            .cell(Cell::left("Bus", mbnl).sep("   "))
            .cell(Cell::plain("Node").sep("  "))
            .cell(Cell::plain("Amps").sep("         "))
            .cell(Cell::plain("Node 1").sep("     "))
            .cell(Cell::plain("Node 2").sep("     "))
            .cell(Cell::plain("Node 3").sep("    "))
            .cell(Cell::plain("...")),
    );
    rep.row(Row::blank(7));

    for bus in &ckt.buses {
        let n = bus.num_nodes_this_bus();
        let Some(zsc) = &bus.zsc else { continue };
        let quoted = format::enclose_quotes(&bus.name.to_uppercase());
        for iphs in 0..n {
            // `IFault := VBus[iphs] / Zsc[iphs,iphs]` (FPC `ucomplex` `/`).
            let ifault = compat::cdiv(bus.vbus[iphs], zsc.get(iphs, iphs));
            // `Format('%s %4u %12.0f ', …)` then `'   '`.
            let mut row = Row::new()
                .cell(Cell::left(quoted.clone(), mbnl + 2).sep(" "))
                .cell(Cell::right(bus.get_num(iphs).to_string(), 4).sep(" "))
                // the `'%12.0f '` trailing space + the `'   '` literal, then the
                // one space each pu column is preceded by
                .cell(Cell::right(format::fixed(ifault.norm(), 0), 12).sep("     "));
            for i in 0..n {
                let vphs = (bus.vbus[i] - zsc.get(i, iphs) * ifault).norm();
                row = row.cell(pu_cell(bus.kv_base, vphs).sep(if i + 1 < n { " " } else { "" }));
            }
            rep.row(row);
        }
    }

    // ── Section 3: Adjacent Node-Node Faults ───────────────────────────────
    rep.blank();
    rep.line("Adjacent Node-Node Faults");
    rep.blank();
    rep.line("                                        pu Node Voltages (L-N Volts if no base)");
    rep.row(
        Row::new()
            .cell(Cell::left("Bus", 13))
            .cell(Cell::plain("Node-Node").sep("      "))
            .cell(Cell::plain("Amps").sep("        "))
            .cell(Cell::plain("Node 1").sep("     "))
            .cell(Cell::plain("Node 2").sep("     "))
            .cell(Cell::plain("Node 3").sep("    "))
            .cell(Cell::plain("...")),
    );
    rep.row(Row::blank(7));

    for bus in &ckt.buses {
        let n = bus.num_nodes_this_bus();
        let Some(ysc) = &bus.ysc else { continue };
        let quoted = format::enclose_quotes(&bus.name.to_uppercase());
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
                let mut row = Row::new()
                    .cell(Cell::left(quoted.clone(), mbnl + 2))
                    .cell(Cell::right(bus.get_num(iphs).to_string(), 4))
                    .cell(Cell::right(bus.get_num(iphs2).to_string(), 4))
                    .cell(Cell::right(format::fixed(iamps, 0), 12).sep("    "));
                for (i, vf) in vfault.iter().enumerate() {
                    row = row.cell(pu_cell(bus.kv_base, vf.norm()).sep(if i + 1 < n {
                        " "
                    } else {
                        ""
                    }));
                }
                rep.row(row);
            }
        }
    }

    rep.finish()
}

#[cfg(test)]
mod tests {
    use crate::exec::Dss;

    /// Robustness pin — **no golden** (the oracle cannot produce a reference here:
    /// it crashes).
    ///
    /// The pinned oracle (`dss_capi 0.14.5`) **access-violates** (#303 "Access
    /// violation") on `Show Faults` after a **cold** `solve mode=faultstudy` — one
    /// with no prior converged non-dynamic solve. There the per-bus short-circuit
    /// matrices are never allocated, and `ShowFaultStudy` still reads them
    /// (`ZFault.CopyFrom(nil)`, `ShowResults.pas:1927`) → **UB** (empirically
    /// reproduced against the pinned engine: the exact deck below, minus the guard
    /// our engine has, faults the oracle process). Per CLAUDE.md an *undefined-
    /// behavior* upstream bug is **not reproduced** — it is documented and gated
    /// around, so there is nothing to compare against and no golden.
    ///
    /// The safe-Rust engine (`#![forbid(unsafe_code)]`) instead handles the same
    /// sequence cleanly: `solve mode=faultstudy` requires a prior non-dynamic solve
    /// (it logs a clean error and does NOT allocate the short-circuit state), and
    /// `show_fault_study`'s `None`-`Zsc`/`Ysc` guard skips the crashing read — so the
    /// report is well-defined (zero fault current → the section-1 `N/A` X/R branch;
    /// the SLG / L-L sections carry only their headers) with no panic, where the
    /// oracle corrupts memory. This pins that we have **no UB** on the oracle's
    /// crash path.
    ///
    /// (The `kVBase > 0` full report is pinned exactly against the oracle by the
    /// `show_faultstudy` golden on the FaultStudy-solved IEEE13 feeder; this test is
    /// only the degenerate cold-solve safety net.)
    #[test]
    fn fault_study_cold_solve_is_safe() {
        let mut dss = Dss::new();
        for c in [
            "clear",
            "new circuit.fscov basekv=12.47 bus1=src phases=3",
            "new line.l1 bus1=src bus2=b1 length=1 units=mi r1=0.1 x1=0.1",
            "new load.ld1 bus1=b1 phases=3 kv=12.47 kw=100",
            // COLD: `solve mode=faultstudy` with NO prior `solve` — the exact
            // sequence the oracle access-violates on at `show faults`.
            "solve mode=faultstudy",
        ] {
            dss.command(c);
        }

        let ckt = dss.circuit().expect("circuit built");
        // The cold faultstudy is refused, so the per-bus short-circuit matrices are
        // never allocated — the `None` state whose read UB-crashes the oracle.
        assert!(
            ckt.buses.iter().all(|b| b.zsc.is_none() && b.ysc.is_none()),
            "cold faultstudy must leave Zsc/Ysc unallocated"
        );

        // The formatter runs to completion (a panic here would fail the test) and
        // emits the full report — the safe-Rust behavior on the input the oracle
        // access-violates on.
        let report = super::show_fault_study(ckt);
        assert!(report.contains("FAULT STUDY REPORT"));
        assert!(report.contains("ALL-Node Fault Currents"));
        assert!(report.contains("Adjacent Node-Node Faults"));
        // Zero prospective fault current (no faultstudy ran) → section 1 takes the
        // `"   N/A"` X/R branch on every node.
        assert!(
            report.contains("N/A"),
            "zero fault current must take the section-1 N/A branch"
        );
    }
}
