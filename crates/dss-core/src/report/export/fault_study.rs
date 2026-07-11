//! Pascal `ExportFaultStudy` (`Common/ExportResults.pas:1526`).

use num_complex::Complex64;

use crate::circuit::Circuit;
use crate::report::format;
use crate::support::cmatrix::CMatrix;

/// Pascal `ExportFaultStudy`: per-bus prospective fault currents — the 3-phase
/// bolted fault (max `|BusCurrent|`), the worst single-phase-to-ground fault, and
/// the worst node-to-node (L-L) fault.
///
/// Reads the **precomputed** bus `Ysc`/`BusCurrent` a prior `Solve mode=faultstudy`
/// (WP7.9) populated — "Isc has been previously computed" (PHASE8_PLAN §2.1). Each
/// single-/L-L-phase current is a **local** per-bus scratch inversion: copy `Ysc`,
/// stamp a large fault conductance `GFault = 10000` at the faulted node(s), invert
/// that `NumNodesThisBus × NumNodesThisBus` matrix, and read the voltage the fault
/// draws (`VFault = YFault⁻¹ · BusCurrent`); the current is `|VFault · GFault|`.
/// **No global re-solve, no hidden mutation** — a read-only formatter. On a
/// snapshot (no faultstudy run) `Ysc` is `None` and `BusCurrent` is zero, so the
/// rows are all-`0.00`, the same degenerate output the oracle produces.
pub(crate) fn export_fault_study(ckt: &Circuit) -> String {
    // Pascal header (note the double spaces): `'Bus,  3-Phase,  1-Phase,  L-L'`.
    let mut s = String::from("Bus,  3-Phase,  1-Phase,  L-L\n");
    let gfault = Complex64::new(10000.0, 0.0);

    for bus in &ckt.buses {
        let n = bus.num_nodes_this_bus();

        // 3-phase bolted fault: max |BusCurrent| over this bus's nodes.
        let mut max_3ph = 0.0f64;
        for i in 0..n {
            let mag = bus.bus_current[i].norm();
            if max_3ph < mag {
                max_3ph = mag;
            }
        }

        // 1-phase and node-node faults both need the precomputed Ysc; without a
        // faultstudy solve it is `None` and the extra currents stay 0 (matching
        // the oracle's degenerate snapshot output).
        let mut max_1ph = 0.0f64;
        let mut max_ll = 0.0f64;
        if let Some(ysc) = &bus.ysc {
            // Single-phase-to-ground: stamp GFault at node `iphs`, invert, read
            // the fault voltage, current = |VFault[iphs] * GFault|.
            for iphs in 0..n {
                let mut yfault = CMatrix::new(n);
                yfault.copy_from(ysc);
                yfault.add(iphs, iphs, gfault);
                let _ = yfault.invert();
                let mut vfault = vec![Complex64::ZERO; n];
                yfault.mv_mult(&mut vfault, &bus.bus_current);
                let curr_mag = (vfault[iphs] * gfault).norm();
                if curr_mag > max_1ph {
                    max_1ph = curr_mag;
                }
            }

            // Node-node (L-L): stamp GFault at `iphs`/`iphs2` and `-GFault` on the
            // symmetric off-diagonal; current = |(VFault[iphs] - VFault[iphs2]) *
            // GFault|. `iphs2` wraps the last node back to the first.
            for iphs in 0..n {
                let iphs2 = if iphs == n - 1 { 0 } else { iphs + 1 };
                let mut yfault = CMatrix::new(n);
                yfault.copy_from(ysc);
                yfault.add(iphs, iphs, gfault);
                yfault.add(iphs2, iphs2, gfault);
                yfault.add_sym(iphs, iphs2, -gfault);
                let _ = yfault.invert();
                let mut vfault = vec![Complex64::ZERO; n];
                yfault.mv_mult(&mut vfault, &bus.bus_current);
                let curr_mag = ((vfault[iphs] - vfault[iphs2]) * gfault).norm();
                if curr_mag > max_ll {
                    max_ll = curr_mag;
                }
            }
        }

        // `Pad(UPPER(name), 12)` + `, %10f` per column (`%f` = 2 decimals in FPC;
        // the width pad is cosmetic — the comparator trims). The bus name is stored
        // lowercase, so `UPPER` matches the oracle's `AnsiUpperCase(NameOfIndex)`.
        s.push_str(&format!(
            "{:<12}, {:>10}, {:>10}, {:>10}\n",
            bus.name.to_uppercase(),
            format::fixed(max_3ph, 2),
            format::fixed(max_1ph, 2),
            format::fixed(max_ll, 2),
        ));
    }
    s
}
