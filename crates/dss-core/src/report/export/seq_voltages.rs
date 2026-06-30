//! `Export SeqVoltages` (Pascal `ExportResults.pas` `ExportSeqVoltages`): per-bus
//! symmetrical-component voltages — `V1`, p.u., base kV (line-to-line), `V2`,
//! `%V2/V1`, `V0`, `%V0/V1`, residual voltage, and `%NEMA` unbalance. Read-only
//! over the solved circuit (PHASE8_PLAN §2.1).

use num_complex::Complex64;

use crate::circuit::Circuit;
use crate::report::format;
use crate::support::mathutil::{SymComp, pct_nema_unbalance};
use crate::util::sqrt3;

/// Build the `Export SeqVoltages` body (Pascal `ExportSeqVoltages`). Walks buses
/// in order; a `< 3`-node bus reports only `V1` (from node 1 when the circuit is
/// `PositiveSequence`), a `>= 3`-node bus runs `Phase2SymComp` over its named
/// nodes 1/2/3 and the NEMA unbalance over the line-to-line voltages.
pub fn export_seq_voltages(ckt: &Circuit) -> String {
    let mut s = String::new();
    s.push_str("Bus,  V1,  p.u.,Base kV, V2, %V2/V1, V0, %V0/V1, Vresidual, %NEMA\n");

    let node_v = &ckt.solution.node_v;
    let sc = SymComp::default();
    for i in 0..ckt.buses.len() {
        let bus = &ckt.buses[i];
        let nn = bus.num_nodes_this_bus();

        let (v0, v1, v2, v_nema) = if nn < 3 {
            // Single-/two-node bus: only the positive sequence, and only from
            // node 1 when `PositiveSequence` (else all zero).
            let v1 = if nn == 1 && ckt.positive_sequence {
                node_v[bus.get_ref(0)].norm()
            } else {
                0.0
            };
            (0.0, v1, 0.0, 0.0)
        } else {
            // Named nodes 1/2/3 (`NodeV^[GetRef(FindIdx(j))]` = `Bus.find(j)`).
            let vph = [
                node_v[bus.find(1)],
                node_v[bus.find(2)],
                node_v[bus.find(3)],
            ];
            let vph_ll = [vph[0] - vph[1], vph[1] - vph[2], vph[2] - vph[0]];
            let mut v012 = [Complex64::ZERO; 3];
            sc.phase_to_sym(&vph, &mut v012);
            (
                v012[0].norm(),
                v012[1].norm(),
                v012[2].norm(),
                pct_nema_unbalance(&vph_ll),
            )
        };

        let vpu = if bus.kv_base != 0.0 {
            0.001 * v1 / bus.kv_base
        } else {
            0.0
        };
        let (v2v1, v0v1) = if v1 > 0.0 {
            (100.0 * v2 / v1, 100.0 * v0 / v1)
        } else {
            (0.0, 0.0)
        };

        let mut vresidual = Complex64::ZERO;
        for j in 0..nn {
            vresidual += node_v[bus.get_ref(j)];
        }

        // `'"%s", %10.6g, %9.5g, %8.2f, %10.6g, %8.4g, %10.6g, %8.4g, %10.6g, %8.4g'`
        let bus_name = ckt.bus_list.name(i).unwrap_or("").to_uppercase();
        s.push_str(&format!(
            "\"{}\", {}, {}, {}, {}, {}, {}, {}, {}, {}\n",
            bus_name,
            format::g(v1, 6),
            format::g(vpu, 5),
            format::fixed(bus.kv_base * sqrt3(), 2),
            format::g(v2, 6),
            format::g(v2v1, 4),
            format::g(v0, 6),
            format::g(v0v1, 4),
            format::g(vresidual.norm(), 6),
            format::g(v_nema, 4),
        ));
    }
    s
}
