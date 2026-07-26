//! `Export SeqCurrents` (Pascal `ExportResults.pas` `ExportSeqCurrents` +
//! `CalcAndWriteSeqCurrents`): per-terminal symmetrical-component currents of
//! every Source, then PD, then PC, then Fault element. The PD pass alone writes
//! the `%Normal`/`%Emergency` rating columns (terminal 1 only).

use num_complex::Complex64;

use crate::circuit::Circuit;
use crate::elements::traits::{CktElement, SysCtx};
use crate::exec::registry::DssClass;
use crate::report::export::for_each_enabled_elem;
use crate::report::format;
use crate::support::mathutil::{SymComp, pct_nema_unbalance};

/// Build the `Export SeqCurrents` body (Pascal `ExportSeqCurrents`). Walks the
/// four element lists in Pascal order calling the mutating `GetCurrents`
/// (`compute_iterminal`); `do_ratings` (PD only) gates the `%Normal`/`%Emergency`
/// columns to terminal 1.
pub(crate) fn export_seq_currents(
    classes: &mut [DssClass],
    ckt: &Circuit,
    sys: &SysCtx,
    node_v: &[Complex64],
) -> String {
    let mut s = String::new();
    s.push_str(
        "Element, Terminal,  I1, %Normal, %Emergency, I2, %I2/I1, I0, %I0/I1, Iresidual, %NEMA\n",
    );

    let sc = SymComp::default();
    let mut calc = |name: &str, elem: &mut dyn CktElement, do_ratings: bool| {
        elem.compute_iterminal(sys, node_v);
        let nterm = elem.cd().nterms;
        let ncond = elem.cd().nconds;
        let nphases = elem.cd().nphases;
        let norm_amps = elem.norm_amps();
        let emerg_amps = elem.emerg_amps();
        let cd = elem.cd();

        for j in 1..=nterm {
            let it = cd.term_i(j - 1);
            let (i0, i1, i2, i_nema) = if nphases >= 3 {
                let iph = [it[0], it[1], it[2]];
                let mut i012 = [Complex64::ZERO; 3];
                sc.phase_to_sym(&iph, &mut i012);
                (
                    i012[0].norm(),
                    i012[1].norm(),
                    i012[2].norm(),
                    pct_nema_unbalance(&iph),
                )
            } else {
                // `PositiveSequence` uses phase 1 only; else all zero.
                let i1 = if sys.positive_sequence {
                    it[0].norm()
                } else {
                    0.0
                };
                (0.0, i1, 0.0, 0.0)
            };

            let (i2i1, i0i1) = if i1 > 0.0 {
                (100.0 * i2 / i1, 100.0 * i0 / i1)
            } else {
                (0.0, 0.0)
            };

            // TODO(compat): a non-positive rating is printed **raw** in the
            // `%Normal`/`%Emergency` columns — Pascal seeds `iNormal := NormAmps`
            // and only *overwrites* it with the percentage when the rating is
            // `> 0` (`ExportResults.pas:409-414`), so e.g. `normamps=-1` prints
            // `-1` as a "percent". Reproduced verbatim (unpinnable on IEEE13 —
            // all ratings positive); the clean fix is printing 0 for an
            // undefined rating.
            let (i_normal, i_emerg) = if do_ratings && j == 1 {
                let n = if norm_amps > 0.0 {
                    i1 / norm_amps * 100.0
                } else {
                    norm_amps
                };
                let e = if emerg_amps > 0.0 {
                    i1 / emerg_amps * 100.0
                } else {
                    emerg_amps
                };
                (n, e)
            } else {
                (0.0, 0.0)
            };

            // The residual (neutral/ground) current of this row's terminal.
            //
            // Upstream sums the *terminal-1* conductors (`cBuffer^[i]`,
            // i = 1..Ncond) for **every** terminal row — `CalcAndWriteSeqCurrents`
            // indexes `cBuffer^[i]`, not `cBuffer^[(j-1)*Ncond+i]` — so every row
            // repeats terminal 1's residual (oracle-proven on IEEE13
            // `Line.671680`: true terminal-2 residual 9.8e-12 A, printed 2.83e-5
            // = terminal 1's; upstream bug report
            // `tmp/seqcurrents_iresidual_bug_report.md`). Stage F's
            // `IRESIDUAL_FROM_TERMINAL_1` row: parity keeps the bug (goldens pin
            // it), the default lane sums the row's own terminal.
            let base = if crate::compat::IRESIDUAL_FROM_TERMINAL_1 {
                0
            } else {
                (j - 1) * ncond
            };
            let mut iresidual = Complex64::ZERO;
            for i in 0..ncond {
                iresidual += cd.iterminal[base + i];
            }

            // `'"%s", %3d, %10.6g, %8.4g, %8.4g, %10.6g, %8.4g, %10.6g, %8.4g, %10.6g, %8.4g'`
            s.push_str(&format!(
                "\"{}\", {:3}, {}, {}, {}, {}, {}, {}, {}, {}, {}\n",
                format::upper_elem_name(name),
                j,
                format::g(i1, 6),
                format::g(i_normal, 4),
                format::g(i_emerg, 4),
                format::g(i2, 6),
                format::g(i2i1, 4),
                format::g(i0, 6),
                format::g(i0i1, 4),
                format::g(iresidual.norm(), 6),
                format::g(i_nema, 4),
            ));
        }
    };

    // Sources, then PDElements (with ratings), then PCElements, then Faults.
    for_each_enabled_elem(classes, &ckt.sources, |n, e| calc(n, e, false));
    for_each_enabled_elem(classes, &ckt.pd_elements, |n, e| calc(n, e, true));
    for_each_enabled_elem(classes, &ckt.pc_elements, |n, e| calc(n, e, false));
    for_each_enabled_elem(classes, &ckt.faults, |n, e| calc(n, e, false));
    s
}
