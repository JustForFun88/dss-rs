//! `Show Powers` (Pascal `ShowResults.pas` `ShowPowers`). Two forms:
//! - `ShowOptionCode = 0` ([`show_powers`]) — the symmetrical-component powers by
//!   circuit element (first 3 phases): per terminal `P1`/`Q1`, `P2`/`Q2`,
//!   `P0`/`Q0` (kW/kvar, or MW/Mvar for `opt = 1`), plus the terminal-1 normal/
//!   emergency excess power of each PD element, and the total-circuit-losses footer;
//! - `ShowOptionCode = 1` ([`show_powers_elements`]) — the per-terminal, per-
//!   conductor branch power flow (`kW +j kvar   kVA   PF`) with per-terminal totals.

use num_complex::Complex64;

use crate::circuit::Circuit;
use crate::elements::traits::{CktElement, SysCtx};
use crate::exec::registry::DssClass;
use crate::report::export::for_each_enabled_elem;
use crate::report::format;
use crate::support::mathutil::{SymComp, power_factor};

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
            for (i, (ci, nref)) in cd.term_phases(j - 1).iter().enumerate() {
                iph[i] = ci;
                vph[i] = node_v[nref];
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
    // Pascal `WriteStr(sout, 'Total Circuit Losses = ', S.re:6:1, ' +j ', S.im:6:1)`
    // — the `:6:1` field width (right-justified, 1 decimal).
    s.push_str(&format!(
        "Total Circuit Losses = {} +j {}\n",
        format::fixed_w(losses.re, 6, 1),
        format::fixed_w(losses.im, 6, 1)
    ));
    s
}

/// Build the `Show Powers` (code 1) text (Pascal `ShowPowers` case 1): the
/// per-terminal, per-conductor branch power flow — `S = NodeV·conj(I)` (×3 for a
/// positive-sequence model, ×0.001 for `opt = 1` MVA), printed `kW +j kvar   kVA
/// PF` per conductor with a per-terminal total, then the total-circuit-losses
/// footer. `opt` = 0 → kW/kvar, 1 → MW/Mvar.
pub(crate) fn show_powers_elements(
    classes: &mut [DssClass],
    ckt: &Circuit,
    sys: &SysCtx,
    node_v: &[Complex64],
    opt: i32,
) -> String {
    let mbnl = super::max_bus_name_length(ckt);
    let pos_seq = sys.positive_sequence;
    let mva = if opt == 1 { 0.001 } else { 1.0 };
    // The `Bus Phase …` column header. The PD-section and PC-section headers use
    // different inter-column whitespace (Pascal `ShowResults.pas:1128/1264`):
    // PD `kW     +j   kvar`, PC `kW   +j  kvar`.
    let hdr = |s: &mut String, mw: bool, pc: bool| {
        s.push_str(&format::pad("  Bus", mbnl));
        s.push_str(match (mw, pc) {
            (true, false) => " Phase     MW     +j   Mvar         MVA         PF\n",
            (false, false) => " Phase     kW     +j   kvar         kVA         PF\n",
            (true, true) => " Phase     MW   +j  Mvar         MVA         PF\n",
            (false, true) => " Phase     kW   +j  kvar         kVA         PF\n",
        });
        s.push('\n');
    };

    let mut s = String::new();
    s.push('\n');
    s.push_str("CIRCUIT ELEMENT POWER FLOW\n");
    s.push('\n');
    s.push_str("(Power Flow into element from indicated Bus)\n");
    s.push('\n');
    s.push_str("Power Delivery Elements\n");
    s.push('\n');
    hdr(&mut s, opt == 1, false);

    for_each_enabled_elem(classes, &ckt.sources, |n, e| {
        write_powers_element(
            &mut s,
            ckt,
            mbnl,
            pos_seq,
            mva,
            n,
            e,
            PowersFamily::Source,
            sys,
            node_v,
        )
    });
    for_each_enabled_elem(classes, &ckt.pd_elements, |n, e| {
        write_powers_element(
            &mut s,
            ckt,
            mbnl,
            pos_seq,
            mva,
            n,
            e,
            PowersFamily::Pd,
            sys,
            node_v,
        )
    });

    s.push_str("= = = = = = = = = = = = = = = = = = =  = = = = = = = = = = =  = =\n");
    s.push('\n');
    s.push_str("Power Conversion Elements\n");
    s.push('\n');
    hdr(&mut s, opt == 1, true);
    for_each_enabled_elem(classes, &ckt.pc_elements, |n, e| {
        write_powers_element(
            &mut s,
            ckt,
            mbnl,
            pos_seq,
            mva,
            n,
            e,
            PowersFamily::Pc,
            sys,
            node_v,
        )
    });

    // Footer: `Total Circuit Losses = re +j im` (Circuit.Losses·0.001, ·0.001 again
    // for MVA). Note code 1's footer uses the raw `%6.1f` `WriteStr :6:1`.
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
        format::fixed_w(losses.re, 6, 1),
        format::fixed_w(losses.im, 6, 1)
    ));
    s
}

/// One symmetrical-component power row for terminal `j` (Pascal
/// `WriteTerminalPowerSeq`, `ShowResults.pas:1357`): `P1 Q1 P2 Q2 P0 Q0` (kW/kvar,
/// or MW/Mvar for `opt = 1`; each `× 0.003`). Used by `Show busflow`'s seq-powers
/// section (called for the single terminal `j` `CheckBusReference` matched). The
/// name label is `Pad(EncloseQuotes(FullName), mdnl+2) + IntToStr(j)` (native case,
/// no space before `j`). Reproduces the Pascal 1-/2-phase `S1` special cases.
#[allow(clippy::too_many_arguments)]
pub(crate) fn write_terminal_power_seq(
    s: &mut String,
    name: &str,
    elem: &mut dyn CktElement,
    sys: &SysCtx,
    node_v: &[Complex64],
    j: usize,
    opt: i32,
    mdnl: usize,
) {
    elem.compute_iterminal(sys, node_v);
    let nphases = elem.cd().nphases;
    let cd = elem.cd();
    let mut vph = [Complex64::ZERO; 3];
    let mut iph = [Complex64::ZERO; 3];
    for (i, (ci, nref)) in cd.term_phases(j - 1).iter().enumerate() {
        vph[i] = node_v[nref];
        iph[i] = ci;
    }
    // Sym-comp for >=3 phases; else only the positive sequence (pos-seq model),
    // zero-seq/neg-seq stay 0 (Pascal V012[1]/V012[3] := CZERO; 0-based [0]/[2]).
    let (mut v012, mut i012) = ([Complex64::ZERO; 3], [Complex64::ZERO; 3]);
    if nphases >= 3 {
        let sc = SymComp::default();
        sc.phase_to_sym(&iph, &mut i012);
        sc.phase_to_sym(&vph, &mut v012);
    } else if sys.positive_sequence {
        v012[1] = vph[0];
        i012[1] = iph[0];
    }
    s.push_str(&format::pad(&format::enclose_quotes(name), mdnl + 2));
    s.push_str(&j.to_string());
    let mva = if opt == 1 { 0.001 } else { 1.0 };
    // P1/Q1: 1-phase → Vph1·conj(Iph1); 2-phase → +Vph2·conj(Iph3); else pos seq.
    let s1 = match nphases {
        1 => vph[0] * iph[0].conj(),
        2 => vph[0] * iph[0].conj() + vph[1] * iph[2].conj(),
        _ => v012[1] * i012[1].conj(),
    } * mva;
    s.push_str(&format::fixed_w(s1.re * 0.003, 11, 1));
    s.push_str(&format::fixed_w(s1.im * 0.003, 11, 1));
    // P2/Q2: neg seq (V012[3]·conj(I012[3]); 0-based [2]).
    let s2 = v012[2] * i012[2].conj() * mva;
    s.push_str(&format::fixed_w(s2.re * 0.003, 11, 1));
    s.push_str(&format::fixed_w(s2.im * 0.003, 11, 1));
    // P0/Q0: zero seq (V012[1]·conj(I012[1]); 0-based [0]).
    let s0 = v012[0] * i012[0].conj() * mva;
    s.push_str(&format::fixed_w(s0.re * 0.003, 8, 1));
    s.push_str(&format::fixed_w(s0.im * 0.003, 8, 1));
    s.push('\n');
}

/// Which `ShowPowers` case-1 walk an element belongs to. Pascal uses three
/// slightly different whitespace layouts (`ShowResults.pas:1162/1240/1297`):
/// Source rows are `Format('%s %4d …')` (ONE space after the bus name), PD/PC
/// rows are `WriteStr(…, '  ', node:4, …)` (two spaces); PC per-conductor
/// powers are `:6:1` (width 6, vs width-8 elsewhere); and the PC terminal
/// label is `'  TERMINAL TOTAL '` (vs `'   TERMINAL TOTAL'`, `:1302`).
#[derive(Clone, Copy, PartialEq)]
enum PowersFamily {
    Source,
    Pd,
    Pc,
}

/// One element's power-flow block for `show_powers_elements` (Pascal `ShowPowers`
/// case 1 inner body). `PowersFamily::Pd` enables the 1-phase/2-terminal floating
/// special case and the AutoTrans `Ntimes = Nphases` arm.
#[allow(clippy::too_many_arguments)]
fn write_powers_element(
    s: &mut String,
    ckt: &Circuit,
    mbnl: usize,
    pos_seq: bool,
    mva: f64,
    name: &str,
    elem: &mut dyn CktElement,
    family: PowersFamily,
    sys: &SysCtx,
    node_v: &[Complex64],
) {
    // One conductor row + the per-terminal `... TERMINAL TOTAL` line, in the
    // family's exact layout (see [`PowersFamily`]).
    let (bus_sep, pw) = match family {
        PowersFamily::Source => (" ", 8),
        PowersFamily::Pd => ("  ", 8),
        PowersFamily::Pc => ("  ", 6),
    };
    let row = |s: &mut String, from_bus: &str, node_num: i32, sp: Complex64| {
        s.push_str(&format!(
            "{}{}{}    {} +j {}   {}     {}\n",
            from_bus,
            bus_sep,
            format::fixed_w_int(node_num as i64, 4),
            format::fixed_w(sp.re / 1000.0, pw, 1),
            format::fixed_w(sp.im / 1000.0, pw, 1),
            format::fixed_w(sp.norm() / 1000.0, 8, 1),
            format::fixed_w(power_factor(sp), 8, 4),
        ));
    };
    let total_label = if family == PowersFamily::Pc {
        "  TERMINAL TOTAL "
    } else {
        "   TERMINAL TOTAL"
    };
    let total = |s: &mut String, saccum: Complex64| {
        s.push_str(&format::pad_dots(total_label, mbnl + 10));
        s.push_str(&format!(
            "{} +j {}   {}     {}\n",
            format::fixed_w(saccum.re / 1000.0, 8, 1),
            format::fixed_w(saccum.im / 1000.0, 8, 1),
            format::fixed_w(saccum.norm() / 1000.0, 8, 1),
            format::fixed_w(power_factor(saccum), 8, 4),
        ));
    };

    elem.compute_iterminal(sys, node_v);
    let (ncond, nterm, nphases) = (elem.cd().nconds, elem.cd().nterms, elem.cd().nphases);
    let cd = elem.cd();
    s.push_str(&format!("ELEMENT = {}\n", format::enclose_quotes(name)));
    // AutoTrans special case (`ShowResults.pas:1190`): only `Nphases` conductor
    // rows per terminal. The Pascal `Inc(k, Ntimes)` at `:1252` sits AFTER the
    // terminal loop where `k` is element-local — it is dead, so terminal 2's rows
    // re-read the first-terminal conductor block; reproduced faithfully.
    let ntimes = if family == PowersFamily::Pd && super::is_autotrans(name) {
        nphases
    } else {
        ncond
    };
    let bus_pad = |t: usize| {
        let b = ckt
            .buses
            .get(cd.terminals[t].bus_ref)
            .map(|x| x.name.as_str())
            .unwrap_or("");
        format::pad(b, mbnl).to_uppercase()
    };
    let power = |k: usize, volts: Complex64| -> Complex64 {
        let mut sp = volts * cd.iterminal[k].conj();
        if pos_seq {
            sp *= 3.0;
        }
        sp * mva
    };
    // PD 1-phase / 2-terminal (possibly floating) special case: one row using the
    // terminal-1 line-line voltage `NodeV[nref1] − NodeV[nref2]` (Pascal
    // `ShowResults.pas:1195`, "Added April 6 2020").
    if family == PowersFamily::Pd && nterm == 2 && ncond == 1 {
        let volts = node_v[cd.node_ref[0]] - node_v[cd.node_ref[1]];
        let sp = power(0, volts);
        row(
            s,
            &bus_pad(0),
            ckt.map_node_to_bus[cd.node_ref[0]].node_num,
            sp,
        );
        total(s, sp);
    } else {
        let mut k = 0usize;
        for j in 0..nterm {
            let from_bus = bus_pad(j);
            let mut saccum = Complex64::ZERO;
            for _ in 0..ntimes {
                let sp = power(k, node_v[cd.node_ref[k]]);
                saccum += sp;
                row(
                    s,
                    &from_bus,
                    ckt.map_node_to_bus[cd.node_ref[k]].node_num,
                    sp,
                );
                k += 1;
            }
            total(s, saccum);
        }
    }
    s.push('\n');
}
