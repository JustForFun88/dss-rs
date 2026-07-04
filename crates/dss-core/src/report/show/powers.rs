//! `Show Powers` (Pascal `ShowResults.pas` `ShowPowers`), `ShowOptionCode = 0` —
//! the symmetrical-component powers by circuit element (first 3 phases): per
//! terminal `P1`/`Q1`, `P2`/`Q2`, `P0`/`Q0` (kW/kvar, or MW/Mvar for `opt = 1`),
//! plus the terminal-1 normal/emergency excess power of each PD element, and the
//! total circuit losses footer.

use num_complex::Complex64;

use crate::circuit::Circuit;
use crate::elements::traits::{CktElement, SysCtx};
use crate::exec::registry::DssClass;
use crate::report::export::for_each_enabled_elem;
use crate::report::format;
use crate::support::mathutil::SymComp;

/// Build the `Show Powers` (code 0) text (Pascal `ShowPowers` case 0). Walks
/// Sources → PDElements → PCElements calling the mutating `GetCurrents`
/// (`compute_iterminal`) + `ExcesskVANorm`/`ExcesskVAEmerg`; `opt` = 0 → kW/kvar,
/// 1 → MW/Mvar.
pub(crate) fn show_powers(
    classes: &mut [DssClass],
    ckt: &Circuit,
    sys: &SysCtx,
    node_v: &[Complex64],
    opt: i32,
) -> String {
    let mdnl = super::max_device_name_length(classes, ckt);

    let mut s = String::new();
    s.push('\n');
    s.push_str("SYMMETRICAL COMPONENT POWERS BY CIRCUIT ELEMENT (first 3 phases)                                     Excess Power\n");
    s.push('\n');
    if opt == 1 {
        s.push_str(&format::pad("Element", mdnl + 2));
        s.push_str(" Term    P1(MW)   Q1(Mvar)       P2         Q2      P0      Q0       P_Norm      Q_Norm     P_Emerg    Q_Emerg\n");
    } else {
        s.push_str(&format::pad("Element", mdnl + 2));
        s.push_str(" Term    P1(kW)   Q1(kvar)       P2         Q2      P0      Q0       P_Norm      Q_Norm     P_Emerg    Q_Emerg\n");
    }
    s.push('\n');

    let sc = SymComp::default();
    let pos_seq = sys.positive_sequence;

    // The seq-power body shared by Sources/PD/PC. `do_excess` writes the PD-only
    // terminal-1 excess-power columns.
    let mut body = |name: &str, elem: &mut dyn CktElement, do_excess: bool| {
        elem.compute_iterminal(sys, node_v);
        let nterm = elem.cd().nterms;
        let ncond = elem.cd().nconds;
        let nphases = elem.cd().nphases;
        // Terminal-1 excess power (PD only), captured before releasing `&mut`.
        let (exc_norm, exc_emerg) = if do_excess {
            (
                elem.excess_kva_norm(sys, node_v, 1),
                elem.excess_kva_emerg(sys, node_v, 1),
            )
        } else {
            (Complex64::ZERO, Complex64::ZERO)
        };
        let cd = elem.cd();

        for j in 1..=nterm {
            // Named-phase Iph/Vph over the first min(3, nphases) conductors.
            let mut iph = [Complex64::ZERO; 3];
            let mut vph = [Complex64::ZERO; 3];
            for i in 0..nphases.min(3) {
                let k = (j - 1) * ncond + i;
                iph[i] = cd.iterminal[k];
                vph[i] = node_v[cd.node_ref[k]];
            }
            let (mut i012, mut v012) = ([Complex64::ZERO; 3], [Complex64::ZERO; 3]);
            if nphases >= 3 {
                sc.phase_to_sym(&iph, &mut i012);
                sc.phase_to_sym(&vph, &mut v012);
            } else if pos_seq {
                // Single-phase / pos-seq model: only the positive sequence.
                v012[1] = vph[0];
                i012[1] = iph[0];
            }

            // Each `S := V012[k] * conj(I012[k])`; `if opt=1 then S := S*0.001`;
            // printed `S.re*0.003` / `S.im*0.003` (the `×3` for 3-phase total,
            // `×0.001` for kW; a second `×0.001` already folded in for MW).
            let mva = if opt == 1 { 0.001 } else { 1.0 };
            let seq = |k: usize| -> Complex64 { v012[k] * i012[k].conj() * mva };
            let (s1, s2, s0) = (seq(1), seq(2), seq(0));
            s.push_str(&format::pad(&format::enclose_quotes(name), mdnl + 2));
            s.push_str(&format::fixed_w_int(j as i64, 3));
            s.push_str(&format!(
                "{}{}",
                format::fixed_w(s1.re * 0.003, 11, 1),
                format::fixed_w(s1.im * 0.003, 11, 1)
            ));
            s.push_str(&format!(
                "{}{}",
                format::fixed_w(s2.re * 0.003, 11, 1),
                format::fixed_w(s2.im * 0.003, 11, 1)
            ));
            s.push_str(&format!(
                "{}{}",
                format::fixed_w(s0.re * 0.003, 8, 1),
                format::fixed_w(s0.im * 0.003, 8, 1)
            ));
            // PD terminal-1 excess power (Pascal only writes it for `j = 1`).
            if do_excess && j == 1 {
                let en = exc_norm * mva;
                let ee = exc_emerg * mva;
                s.push_str(&format!(
                    "{}{}{}{}",
                    format::fixed_w(en.re, 11, 1),
                    format::fixed_w(en.im, 11, 1),
                    format::fixed_w(ee.re, 11, 1),
                    format::fixed_w(ee.im, 11, 1)
                ));
            }
            s.push('\n');
        }
    };

    for_each_enabled_elem(classes, &ckt.sources, |n, e| body(n, e, false));
    for_each_enabled_elem(classes, &ckt.pd_elements, |n, e| body(n, e, true));
    for_each_enabled_elem(classes, &ckt.pc_elements, |n, e| body(n, e, false));

    // Footer: `Total Circuit Losses = re +j im` (Pascal `DSS.ActiveCircuit.Losses`
    // = Σ enabled non-shunt PD losses, `Circuit.Get_Losses`), ×0.001 → kW (×0.001
    // again for MW when opt=1).
    let mut losses = Complex64::ZERO;
    for_each_enabled_elem(classes, &ckt.pd_elements, |_n, e| {
        if !e.is_shunt() {
            losses += e.losses(sys, node_v);
        }
    });
    losses *= 0.001;
    if opt == 1 {
        losses *= 0.001;
    }
    s.push('\n');
    s.push_str(&format!(
        "Total Circuit Losses = {} +j {}\n",
        format::fixed(losses.re, 1),
        format::fixed(losses.im, 1)
    ));
    s
}
