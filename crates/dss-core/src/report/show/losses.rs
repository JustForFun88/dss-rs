//! `Show Losses` (Pascal `ShowResults.pas` `ShowLosses`): per-PD-element kW/kvar
//! losses + `% of terminal-1 power`, then the line / transformer / total loss
//! aggregates and the served-load-power / percent-losses summary.

use num_complex::Complex64;

use crate::circuit::Circuit;
use crate::elements::traits::SysCtx;
use crate::exec::registry::DssClass;
use crate::report::export::for_each_enabled_elem;
use crate::report::format;
use crate::report::table::{Cell, Report, Row};

/// Build the `Show Losses` text (Pascal `ShowLosses`). Walks the PDElements
/// (`Losses`/`Power[1]` — the mutating getters) then sums served load power over
/// `Loads`, so it takes the `&mut [DssClass]` element-walk borrow.
pub(crate) fn show_losses(
    classes: &mut [DssClass],
    ckt: &Circuit,
    sys: &SysCtx,
    node_v: &[Complex64],
) -> String {
    let mdnl = super::device_name_width(classes, ckt);

    let mut rep = Report::new();
    rep.blank();
    rep.line("LOSSES REPORT");
    rep.blank();
    rep.line("Power Delivery Element Loss Report");
    rep.blank();
    // `'Element                  kW Losses    % of Power   kvar Losses'` — the
    // header literal, as the four columns it draws (offsets 0/25/38/51).
    rep.row(
        Row::new()
            .cell(Cell::left("Element", 25))
            .cell(Cell::left("kW Losses", 13))
            .cell(Cell::left("% of Power", 13))
            .cell(Cell::plain("kvar Losses")),
    );
    rep.row(Row::blank(4));

    let mut total = Complex64::ZERO;
    let mut line = Complex64::ZERO;
    let mut trans = Complex64::ZERO;

    for_each_enabled_elem(classes, &ckt.pd_elements, |name, elem| {
        let klosses = elem.losses(sys, node_v) * 0.001; // kW losses
        total += klosses;
        let term_power = elem.terminal_power(sys, node_v, 1) * 0.001; // terminal-1 power

        // Aggregate by element class (Pascal's CLASSMASK XFMR/AUTOTRANS/LINE test).
        let class = name.split('.').next().unwrap_or("").to_ascii_lowercase();
        if class == "transformer" || class == "autotransformer" || class == "autotrans" {
            trans += klosses;
        } else if class == "line" {
            line += klosses;
        }

        // `% of Power`: guard `TermPower.re <> 0 and kLosses.re > 0.0009`.
        let pct = if term_power.re != 0.0 && klosses.re > 0.0009 {
            format::fixed(klosses.re / term_power.re.abs() * 100.0, 2)
        } else {
            format::fixed(0.0, 1)
        };
        rep.row(
            Row::new()
                .cell(Cell::left(format::enclose_quotes(name), mdnl + 2))
                // `Format('%10.5f, ', kLosses.re)`.
                .cell(Cell::right(format::fixed(klosses.re, 5), 10).sep(", "))
                .cell(Cell::right(pct, 8).sep("     "))
                // `Format('     %.6g', kLosses.im)`.
                .cell(Cell::plain(format::g(klosses.im, 6))),
        );
    });

    rep.blank();

    // Sum served load power over the enabled Loads (`Power[1]`).
    let mut load_power = Complex64::ZERO;
    for_each_enabled_elem(classes, &ckt.loads, |_name, elem| {
        load_power += elem.terminal_power(sys, node_v, 1);
    });
    load_power *= 0.001;

    // The aggregate block: `Pad(label, 30)` + `Format('%10.1f')` + `' kW'`. The
    // unit is a cell of its own — a separator carries no token.
    let agg = |label: &str, v: f64, unit: &str| {
        Row::new()
            .cell(Cell::left(label, 30))
            .cell(Cell::right(format::fixed(v, 1), 10).sep(" "))
            .cell(Cell::plain(unit))
    };
    rep.row(agg("LINE LOSSES=", line.re, "kW"));
    rep.row(agg("TRANSFORMER LOSSES=", trans.re, "kW"));
    rep.row(Row::blank(3));
    rep.row(agg("TOTAL LOSSES=", total.re, "kW"));
    rep.row(Row::blank(3));
    rep.row(agg("TOTAL LOAD POWER = ", load_power.re.abs(), "kW"));
    // Pascal writes the "Percent Losses" label unconditionally, the value only
    // when `LoadPower.re <> 0` — and then *without* a trailing newline, which is
    // why the empty case leaves the row model and is written as raw text.
    if load_power.re != 0.0 {
        rep.row(
            Row::new()
                .cell(Cell::left("Percent Losses for Circuit = ", 30))
                .cell(
                    Cell::right(
                        format::fixed((total.re / load_power.re).abs() * 100.0, 2),
                        8,
                    )
                    .sep(" "),
                )
                .cell(Cell::plain("%")),
        );
    } else {
        rep.text(&format::pad("Percent Losses for Circuit = ", 30));
    }
    rep.finish()
}
