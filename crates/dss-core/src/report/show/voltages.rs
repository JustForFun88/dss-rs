//! `Show Voltages` (Pascal `ShowResults.pas` `ShowVoltages` + `WriteSeqVoltages`),
//! `ShowOptionCode = 0` — the symmetrical-component voltages by bus (for 3-phase
//! buses): `V1 (kV)`, p.u., `V2 (kV)`, `%V2/V1`, `V0 (kV)`, `%V0/V1`.
//!
//! The `LL` flag (`Show Voltages LL`) reports phase-to-phase: the sym-comp runs
//! over the line-to-line voltages and the p.u. divides by √3. The node/element
//! forms (`ShowOptionCode` 1/2, angle-bearing) are a later WP8.4 step.

use num_complex::Complex64;

use crate::circuit::Circuit;
use crate::report::format;
use crate::support::mathutil::SymComp;
use crate::util::sqrt3;

/// Build the `Show Voltages` (code 0) text (Pascal `ShowVoltages` case 0 +
/// `WriteSeqVoltages`). Read-only over the solved node voltages. `ll` = the
/// line-to-line option.
pub(crate) fn show_voltages(ckt: &Circuit, ll: bool) -> String {
    let mbnl = super::max_bus_name_length(ckt);

    let mut s = String::new();
    s.push('\n');
    if ll {
        s.push_str("SYMMETRICAL COMPONENT PHASE-PHASE VOLTAGES BY BUS (for 3-phase buses)\n");
    } else {
        s.push_str("SYMMETRICAL COMPONENT VOLTAGES BY BUS (for 3-phase buses)\n");
    }
    s.push('\n');
    s.push_str(&format::pad("Bus", mbnl));
    s.push_str("  Mag:   V1 (kV)    p.u.     V2 (kV)   %V2/V1    V0 (kV)    %V0/V1\n");
    s.push('\n');

    let node_v = &ckt.solution.node_v;
    let sc = SymComp::default();
    for i in 0..ckt.buses.len() {
        let bus = &ckt.buses[i];
        let nn = bus.num_nodes_this_bus();

        // Pascal `WriteSeqVoltages`: >=3 nodes → sym-comp over named nodes 1/2/3
        // (or their L-L differences when `LL`); <3 nodes → V1 = |V| of the FIRST
        // node (`GetRef(1)`), V0 = V2 = 0 (note: NOT `PositiveSequence`-gated,
        // unlike `ExportSeqVoltages`).
        let (mut v0, mut v1, mut v2) = if nn >= 3 {
            let vph = [
                node_v[bus.find(1)],
                node_v[bus.find(2)],
                node_v[bus.find(3)],
            ];
            let mut v012 = [Complex64::ZERO; 3];
            if ll {
                let vll = [vph[0] - vph[1], vph[1] - vph[2], vph[2] - vph[0]];
                sc.phase_to_sym(&vll, &mut v012);
            } else {
                sc.phase_to_sym(&vph, &mut v012);
            }
            (v012[0].norm(), v012[1].norm(), v012[2].norm())
        } else {
            (0.0, node_v[bus.get_ref(0)].norm(), 0.0)
        };
        // Convert to kV.
        v1 /= 1000.0;
        v2 /= 1000.0;
        v0 /= 1000.0;

        let mut vpu = if bus.kv_base != 0.0 {
            v1 / bus.kv_base
        } else {
            0.0
        };
        if ll {
            vpu /= sqrt3();
        }
        let (v2v1, v0v1) = if v1 > 0.0 {
            (100.0 * v2 / v1, 100.0 * v0 / v1)
        } else {
            (0.0, 0.0)
        };

        // `Format('%s %9.4g  %9.4g  %9.4g  %9.4g %9.4g %9.4g', [Pad(BusName,…), V1,
        // Vpu, V2, V2V1, V0, V0V1])` — bus name space-padded, not uppercased/quoted.
        let bus_name = ckt.bus_list.name(i).unwrap_or("");
        s.push_str(&format::pad(bus_name, mbnl));
        s.push_str(&format!(
            " {}  {}  {}  {} {} {}\n",
            format::g_w(v1, 9, 4),
            format::g_w(vpu, 9, 4),
            format::g_w(v2, 9, 4),
            format::g_w(v2v1, 9, 4),
            format::g_w(v0, 9, 4),
            format::g_w(v0v1, 9, 4),
        ));
    }
    s
}
