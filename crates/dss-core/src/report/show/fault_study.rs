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

    /// Branch pin — **no golden** (kept a unit test per the same step-10 decision,
    /// though this branch *is* oracle-comparable). Sections 2 & 3 have two voltage
    /// formats: `%10.3f` per-unit when the bus has a base kV, and `%10.1f`
    /// "L-N Volts if no base" when `kVBase <= 0`. The `show_faultstudy` golden
    /// (based IEEE13) only exercises the pu form, so this test drives the
    /// **unbased** form: a circuit with a prior `solve mode=snap` (so FaultStudy
    /// runs and populates `Zsc`/`Ysc`) but **no** `Set Voltagebases` /
    /// `CalcVoltageBases`, leaving every `kVBase = 0`. It pins that the `kVBase <= 0`
    /// selector fires — the fault-driven node voltages render as raw volts
    /// (`%10.1f`, `>> 1`), never the pu `%10.3f` form — with no panic.
    #[test]
    fn fault_study_unbased_uses_ln_volts_branch() {
        let mut dss = Dss::new();
        for c in [
            "clear",
            "new circuit.fscov basekv=12.47 bus1=src phases=3",
            "new line.l1 bus1=src bus2=b1 length=1 units=mi r1=0.1 x1=0.1",
            "new load.ld1 bus1=b1 phases=3 kv=12.47 kw=100",
            "solve mode=snap", // a prior converged solve, so FaultStudy runs
            "solve mode=faultstudy", // populates Zsc/Ysc/VBus/BusCurrent
                               // NO `Set Voltagebases` / `CalcVoltageBases`:
                               // every bus keeps kVBase = 0.
        ] {
            dss.command(c);
        }

        let ckt = dss.circuit().expect("circuit built");
        assert!(
            ckt.buses.iter().all(|b| b.kv_base == 0.0),
            "fixture must leave every bus's kVBase unset"
        );
        assert!(
            ckt.buses.iter().any(|b| b.zsc.is_some() && b.ysc.is_some()),
            "faultstudy must have populated Zsc/Ysc"
        );

        let report = super::show_fault_study(ckt);
        // The `kVBase <= 0` branch prints voltages at `%10.1f` (one decimal), so a
        // faulted node reads ` 0.0`, never the pu ` 0.000`. A pu (`%10.3f`) cell
        // anywhere would mean the base-kV branch wrongly fired.
        assert!(
            !report.contains("0.000"),
            "unbased buses must use the %10.1f L-N-Volts branch, not %10.3f pu"
        );
        // And the SLG section carries real fault-driven raw volts (>> 1 pu), i.e. a
        // multi-hundred/thousand-volt cell — confirming the branch produced values.
        let slg = report
            .split("ONE-Node to ground Faults")
            .nth(1)
            .and_then(|s| s.split("Adjacent Node-Node Faults").next())
            .expect("SLG section present");
        let has_raw_volts = slg
            .split_whitespace()
            .any(|t| t.parse::<f64>().is_ok_and(|v| v > 100.0 && t.contains('.')));
        assert!(
            has_raw_volts,
            "SLG section must carry raw L-N-Volts cells (>> 1 pu)"
        );
    }
}
