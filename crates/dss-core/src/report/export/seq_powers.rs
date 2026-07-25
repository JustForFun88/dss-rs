//! `Export SeqPowers` (Pascal `ExportResults.pas` `ExportSeqPowers`): per-terminal
//! symmetrical-component powers of every PD then PC element. PD rows carry the
//! `P_Normal`/`Q_Normal`/`P_Emergency`/`Q_Emergency` excess columns on terminal 1;
//! PC rows stop after the sequence powers.
//!
//! `opt` = 0 → kW/kvar, 1 → MW/Mvar. `DoExportCmd` only ever reaches `opt = 0`
//! (ptr 10 does **not** pre-parse the MVA flag — only `Powers`(9)/`P_byphase`(19)
//! do, `ExportOptions.pas:191`), so the MVA path is ported but unreached.

use num_complex::Complex64;

use crate::circuit::Circuit;
use crate::elements::traits::{CktElement, SysCtx};
use crate::exec::registry::DssClass;
use crate::report::export::for_each_enabled_elem;
use crate::report::format;
use crate::support::mathutil::SymComp;

/// Build the `Export SeqPowers` body (Pascal `ExportSeqPowers`). Sequence power
/// `S = V012·conj(I012)`; the printed value is `S.re·0.003` (= per-sequence VA →
/// three-phase kW), with the MVA path applying an extra `0.001` to `S` first.
pub(crate) fn export_seq_powers(
    classes: &mut [DssClass],
    ckt: &Circuit,
    sys: &SysCtx,
    node_v: &[Complex64],
    opt: i32,
) -> String {
    let mut s = String::new();
    if opt == 1 {
        s.push_str("Element, Terminal, P1(MW), Q1(Mvar), P2, Q2, P0, Q0, P_Normal, Q_Normal, P_Emergency, Q_Emergency\n");
    } else {
        s.push_str("Element, Terminal, P1(kW), Q1(kvar), P2, Q2, P0, Q0, P_Normal, Q_Normal, P_Emergency, Q_Emergency\n");
    }
    // `if Opt = 1 then S := S * 0.001` before the print scaling.
    let pscale = if opt == 1 { 0.001 } else { 1.0 };

    let sc = SymComp::default();
    let mut calc = |name: &str, elem: &mut dyn CktElement, is_pd: bool| {
        elem.compute_iterminal(sys, node_v);
        let nterm = elem.cd().nterms;
        let nphases = elem.cd().nphases;
        // PD excess kVA (terminal 1) — captured before the immutable `cd` borrow.
        let (exc_norm, exc_emerg) = if is_pd {
            (
                elem.excess_kva_norm(sys, node_v, 1),
                elem.excess_kva_emerg(sys, node_v, 1),
            )
        } else {
            (Complex64::ZERO, Complex64::ZERO)
        };
        let cd = elem.cd();

        for j in 1..=nterm {
            let mut iph = [Complex64::ZERO; 3];
            let mut vph = [Complex64::ZERO; 3];
            for (i, (ci, nref)) in cd.term_phases(j - 1).iter().enumerate() {
                iph[i] = ci;
                vph[i] = node_v[nref];
            }

            let mut v012 = [Complex64::ZERO; 3];
            let mut i012 = [Complex64::ZERO; 3];
            if nphases >= 3 {
                sc.phase_to_sym(&iph, &mut i012);
                sc.phase_to_sym(&vph, &mut v012);
            } else if sys.positive_sequence {
                v012[1] = vph[0];
                i012[1] = iph[0];
            }

            // `S := V012[seq]·conj(I012[seq])` then `· pscale`; print `S.re·0.003`.
            let seq_pow = |seq: usize| (v012[seq] * i012[seq].conj()) * pscale;
            let s1 = seq_pow(1);
            let s2 = seq_pow(2);
            let s0 = seq_pow(0);
            s.push_str(&format!(
                "\"{}\", {:3}, {}, {}, {}, {}, {}, {}",
                format::upper_elem_name(name),
                j,
                format::fixed(s1.re * 0.003, 1),
                format::fixed(s1.im * 0.003, 1),
                format::fixed(s2.re * 0.003, 1),
                format::fixed(s2.im * 0.003, 1),
                format::fixed(s0.re * 0.003, 1),
                format::fixed(s0.im * 0.003, 1),
            ));

            if is_pd && j == 1 {
                // Excess kVA is already kVA — `Abs(S.re)` directly, no `·0.003`;
                // MVA applies the single `pscale` (= 0.001).
                let en = exc_norm * pscale;
                let ee = exc_emerg * pscale;
                s.push_str(&format!(
                    ", {}, {}, {}, {}",
                    format::fixed(en.re.abs(), 1),
                    format::fixed(en.im.abs(), 1),
                    format::fixed(ee.re.abs(), 1),
                    format::fixed(ee.im.abs(), 1),
                ));
            }
            s.push('\n');
        }
    };

    for_each_enabled_elem(classes, &ckt.pd_elements, |n, e| calc(n, e, true));
    for_each_enabled_elem(classes, &ckt.pc_elements, |n, e| calc(n, e, false));
    s
}
