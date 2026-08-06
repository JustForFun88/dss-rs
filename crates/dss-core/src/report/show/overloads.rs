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
use crate::report::table::{Cell, Report, Row};
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
    // `SetMaxDeviceNameLength(DSS)`, sized from its own content in both lanes:
    // the pinned dss_capi backend answers 0 here because its loop fills a
    // shadowing circuit field instead of the unit variable the writers read
    // (see [`super::device_name_width`]), and that defect is reproduced in no
    // lane. Only padding moves; the tokenizing golden comparator drops it.
    let mdnl = super::device_name_width(classes, ckt);

    let mut rep = Report::new();
    rep.blank();
    rep.line("Power Delivery Element Overload Report");
    rep.blank();
    rep.line("SYMMETRICAL COMPONENT CURRENTS BY CIRCUIT ELEMENT ");
    rep.blank();
    // `'Element                             Term    I1    IOver %Normal  %Emerg
    //      I2    %I2/I1    I0    %I0/I1'` — the header literal as its ten columns
    // (offsets 0/36/44/50/56/65/76/82/92/98).
    rep.row(
        Row::new()
            .cell(Cell::left("Element", 36))
            .cell(Cell::left("Term", 8))
            .cell(Cell::left("I1", 6))
            .cell(Cell::left("IOver", 6))
            .cell(Cell::left("%Normal", 9))
            .cell(Cell::left("%Emerg", 11))
            .cell(Cell::left("I2", 6))
            .cell(Cell::left("%I2/I1", 10))
            .cell(Cell::left("I0", 6))
            .cell(Cell::plain("%I0/I1")),
    );
    rep.row(Row::blank(10));

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

        // The degenerate branches emit the literal `0.0` at one decimal, exactly
        // as Pascal's `'     0.0'` (8 chars) does.
        let zero = || format::fixed(0.0, 1);
        // IOver (`Cmax - NormAmps`) / %Normal, or `0.0` twice when `NormAmps <= 0`.
        let (iover, pct_norm) = if norm_amps > 0.0 {
            (
                format::fixed(cmax - norm_amps, 2),
                format::fixed(cmax / norm_amps * 100.0, 1),
            )
        } else {
            (zero(), zero())
        };
        // %Emergency, then %I2/I1 and %I0/I1 (`0.0` when `I1 == 0`).
        let pct_emerg = if emerg_amps > 0.0 {
            format::fixed(cmax / emerg_amps * 100.0, 1)
        } else {
            zero()
        };
        let pct_of_i1 = |v: f64| {
            if i1 > 0.0 {
                format::fixed(100.0 * v / i1, 1)
            } else {
                zero()
            }
        };
        // `Pad(EncloseQuotes(FullName), MaxDeviceNameLength+2)` then `%3d%8.1f`
        // [term=1, I1] and seven more `%8.*f` fields, all butted together in
        // Pascal — each is its own column here.
        rep.row(
            Row::new()
                .cell(Cell::left(format::enclose_quotes(name), mdnl + 2))
                .cell(Cell::right("1", 3))
                .cell(Cell::right(format::fixed(i1, 1), 8))
                .cell(Cell::right(iover, 8))
                .cell(Cell::right(pct_norm, 8))
                .cell(Cell::right(pct_emerg, 8))
                .cell(Cell::right(format::fixed(i2, 1), 8))
                .cell(Cell::right(pct_of_i1(i2), 8))
                .cell(Cell::right(format::fixed(i0, 1), 8))
                .cell(Cell::right(pct_of_i1(i0), 8)),
        );
    });
    rep.finish()
}
