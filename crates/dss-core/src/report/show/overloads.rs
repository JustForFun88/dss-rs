//! `Show Overloads` (Pascal `ShowResults.pas` `ShowOverloads` (:2514)): the
//! symmetrical-component overload report — one row per **enabled** PDElement
//! (excluding capacitors) whose terminal-1 max phase current exceeds its normal
//! or emergency rating.
//!
//! Shares the seq-current math with [`super::show_currents`]/`export_overloads`,
//! but its **layout** is the `Show` form: no `kVAOver` column (that is
//! `Export`-only), fixed-width fields (`%3d%8.1f`, then `%8.2f`/`%8.1f`), and the
//! degenerate `NormAmps <= 0` / `EmergAmps <= 0` branches emit the literal
//! `     0.0` (8 chars) exactly as Pascal.

use num_complex::Complex64;

use crate::circuit::Circuit;
use crate::elements::traits::SysCtx;
use crate::exec::registry::DssClass;
use crate::report::export::for_each_enabled_elem;
use crate::report::format;
use crate::support::mathutil::SymComp;

/// Build the `Show Overloads` text (Pascal `ShowOverloads`). Walks the PDElements
/// (terminal 1 only) via the mutating `GetCurrents` getter (PHASE8_PLAN §2.1);
/// capacitors are skipped (`(CLASSMASK and DSSObjType) <> CAP_ELEMENT`).
pub(crate) fn show_overloads(
    classes: &mut [DssClass],
    ckt: &Circuit,
    sys: &SysCtx,
    node_v: &[Complex64],
) -> String {
    // `SetMaxDeviceNameLength(DSS)` — the pinned backend returns 0, so the name
    // column is unpadded (`Pad(EncloseQuotes(FullName), 0 + 2)` = the bare quoted
    // name); reproduced 1:1 via [`super::max_device_name_length`].
    let mdnl = super::max_device_name_length(classes, ckt);

    let mut s = String::new();
    s.push('\n');
    s.push_str("Power Delivery Element Overload Report\n");
    s.push('\n');
    s.push_str("SYMMETRICAL COMPONENT CURRENTS BY CIRCUIT ELEMENT \n");
    s.push('\n');
    s.push_str(
        "Element                             Term    I1    IOver %Normal  %Emerg     I2    %I2/I1    I0    %I0/I1\n",
    );
    s.push('\n');

    let sc = SymComp::default();
    for_each_enabled_elem(classes, &ckt.pd_elements, |name, elem| {
        // Ignore capacitors (`(CLASSMASK and PDElem.DSSObjType) <> CAP_ELEMENT`).
        if name
            .split('.')
            .next()
            .is_some_and(|c| c.eq_ignore_ascii_case("capacitor"))
        {
            return;
        }
        elem.compute_iterminal(sys, node_v);
        let norm_amps = elem.norm_amps();
        let emerg_amps = elem.emerg_amps();
        let nphases = elem.cd().nphases;

        // Terminal 1 only (Pascal `for j := 1 to 1`): `Cmax` = max phase magnitude
        // over the first `min(Nphases, 3)` conductors, then symmetrical components;
        // the <3-phase fallback sets `I0 = I2 = 0`, `I1 = |Iph[1]|`, `Cmax := I1`.
        let (i0, i1, i2, cmax) = if nphases >= 3 {
            let iph = [
                elem.cd().iterminal[0],
                elem.cd().iterminal[1],
                elem.cd().iterminal[2],
            ];
            let cmax = iph.iter().map(|c| c.norm()).fold(0.0, f64::max);
            let mut i012 = [Complex64::ZERO; 3];
            sc.phase_to_sym(&iph, &mut i012);
            (i012[0].norm(), i012[1].norm(), i012[2].norm(), cmax)
        } else {
            let i1 = elem.cd().iterminal[0].norm();
            (0.0, i1, 0.0, i1)
        };

        // Only overloaded branches (a positive rating AND `Cmax` over one of them).
        if !(norm_amps > 0.0 || emerg_amps > 0.0) {
            return;
        }
        if !(cmax > norm_amps || cmax > emerg_amps) {
            return;
        }

        // `Pad(EncloseQuotes(FullName), MaxDeviceNameLength+2)` then `%3d%8.1f`
        // [term=1, I1] (the `%3d` and `%8.1f` are glued in Pascal — the tokenizer
        // still splits on the whitespace within `%8.1f`).
        s.push_str(&format::pad(&format::enclose_quotes(name), mdnl + 2));
        s.push_str(&format::fixed_w_int(1, 3));
        s.push_str(&format::fixed_w(i1, 8, 1));
        // IOver (`Cmax - NormAmps`) / %Normal, or the literal `     0.0` twice when
        // `NormAmps <= 0`.
        if norm_amps > 0.0 {
            s.push_str(&format::fixed_w(cmax - norm_amps, 8, 2));
            s.push_str(&format::fixed_w(cmax / norm_amps * 100.0, 8, 1));
        } else {
            s.push_str("     0.0");
            s.push_str("     0.0");
        }
        // %Emergency, or the literal `     0.0`.
        if emerg_amps > 0.0 {
            s.push_str(&format::fixed_w(cmax / emerg_amps * 100.0, 8, 1));
        } else {
            s.push_str("     0.0");
        }
        // I2, then %I2/I1 (`     0.0` when `I1 == 0`), then I0, then %I0/I1.
        s.push_str(&format::fixed_w(i2, 8, 1));
        if i1 > 0.0 {
            s.push_str(&format::fixed_w(100.0 * i2 / i1, 8, 1));
        } else {
            s.push_str("     0.0");
        }
        s.push_str(&format::fixed_w(i0, 8, 1));
        if i1 > 0.0 {
            s.push_str(&format::fixed_w(100.0 * i0 / i1, 8, 1));
        } else {
            s.push_str("     0.0");
        }
        s.push('\n');
    });
    s
}
