//! `Export Powers` (Pascal `ExportResults.pas` `ExportPowers`): per-terminal
//! complex power (kW/kvar, or MW/Mvar for `opt = 1`) of every PD then PC element,
//! plus the normal/emergency excess kVA on terminal 1 of each PD element.

use num_complex::Complex64;

use crate::circuit::Circuit;
use crate::elements::traits::SysCtx;
use crate::exec::registry::DssClass;
use crate::report::export::for_each_enabled_elem;
use crate::report::format;

/// Build the `Export Powers` body (Pascal `ExportPowers`, `opt` = 0 → kVA,
/// 1 → MVA). Read/compute over the solved circuit: walks PDElements then
/// PCElements, calling the mutating `Power`/`ExcesskVANorm`/`ExcesskVAEmerg`
/// getters (PHASE8_PLAN §2.1).
pub(crate) fn export_powers(
    classes: &mut [DssClass],
    ckt: &Circuit,
    sys: &SysCtx,
    node_v: &[Complex64],
    opt: i32,
) -> String {
    let mut s = String::new();
    if opt == 1 {
        s.push_str(
            "Element, Terminal, P(MW), Q(Mvar), P_Normal, Q_Normal, P_Emergency, Q_Emergency\n",
        );
    } else {
        s.push_str(
            "Element, Terminal, P(kW), Q(kvar),  P_Normal, Q_Normal, P_Emergency, Q_Emergency\n",
        );
    }

    // `Power[j]` returns W; `:11:1` prints `S.re * 0.001` (= kW). For MVA the
    // power is first scaled to kW (`S := S * 0.001`) then the same `* 0.001`
    // print yields MW. So the printed scale is `0.001 * (opt==1 ? 0.001 : 1)`.
    let pscale = if opt == 1 { 0.001 * 0.001 } else { 0.001 };
    // `ExcesskVANorm`/`ExcesskVAEmerg` already return kVA, printed `Abs(S.re)`
    // directly; MVA multiplies by a single `0.001`.
    let escale = if opt == 1 { 0.001 } else { 1.0 };

    // PDElements first.
    for_each_enabled_elem(classes, &ckt.pd_elements, |name, elem| {
        let nterm = elem.cd().nterms;
        for j in 1..=nterm {
            let power = elem.terminal_power(sys, node_v, j);
            s.push_str(&format!(
                "\"{}\", {:3}, {}, {}",
                name.to_uppercase(),
                j,
                format::fixed(power.re * pscale, 1),
                format::fixed(power.im * pscale, 1),
            ));
            if j == 1 {
                let en = elem.excess_kva_norm(sys, node_v, 1);
                let ee = elem.excess_kva_emerg(sys, node_v, 1);
                s.push_str(&format!(
                    ", {}, {}, {}, {}",
                    format::fixed((en.re * escale).abs(), 1),
                    format::fixed((en.im * escale).abs(), 1),
                    format::fixed((ee.re * escale).abs(), 1),
                    format::fixed((ee.im * escale).abs(), 1),
                ));
            }
            s.push('\n');
        }
    });

    // PCElements next (no excess columns).
    for_each_enabled_elem(classes, &ckt.pc_elements, |name, elem| {
        let nterm = elem.cd().nterms;
        for j in 1..=nterm {
            let power = elem.terminal_power(sys, node_v, j);
            s.push_str(&format!(
                "\"{}\", {:3}, {}, {}\n",
                name.to_uppercase(),
                j,
                format::fixed(power.re * pscale, 1),
                format::fixed(power.im * pscale, 1),
            ));
        }
    });

    s
}
