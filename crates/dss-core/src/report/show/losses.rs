//! `Show Losses` (Pascal `ShowResults.pas` `ShowLosses`): per-PD-element kW/kvar
//! losses + `% of terminal-1 power`, then the line / transformer / total loss
//! aggregates and the served-load-power / percent-losses summary.

use num_complex::Complex64;

use crate::circuit::Circuit;
use crate::elements::traits::SysCtx;
use crate::exec::registry::DssClass;
use crate::report::export::for_each_enabled_elem;
use crate::report::format;

/// Build the `Show Losses` text (Pascal `ShowLosses`). Walks the PDElements
/// (`Losses`/`Power[1]` — the mutating getters) then sums served load power over
/// `Loads`, so it takes the `&mut [DssClass]` element-walk borrow.
pub(crate) fn show_losses(
    classes: &mut [DssClass],
    ckt: &Circuit,
    sys: &SysCtx,
    node_v: &[Complex64],
) -> String {
    let mdnl = super::max_device_name_length(classes, ckt);

    let mut s = String::new();
    s.push('\n');
    s.push_str("LOSSES REPORT\n");
    s.push('\n');
    s.push_str("Power Delivery Element Loss Report\n");
    s.push('\n');
    s.push_str("Element                  kW Losses    % of Power   kvar Losses\n");
    s.push('\n');

    let mut total = Complex64::ZERO;
    let mut line = Complex64::ZERO;
    let mut trans = Complex64::ZERO;

    for_each_enabled_elem(classes, &ckt.pd_elements, |name, elem| {
        let klosses = elem.losses(sys, node_v) * 0.001; // kW losses
        total += klosses;
        let term_power = elem.terminal_power(sys, node_v, 1) * 0.001; // terminal-1 power

        // Aggregate by element class (Pascal's CLASSMASK XFMR/AUTOTRANS/LINE test).
        let class = name.split('.').next().unwrap_or("").to_lowercase();
        if class == "transformer" || class == "autotransformer" || class == "autotrans" {
            trans += klosses;
        } else if class == "line" {
            line += klosses;
        }

        s.push_str(&format::pad(&format::enclose_quotes(name), mdnl + 2));
        // `Format('%10.5f, ', kLosses.re)`.
        s.push_str(&format!("{}, ", format::fixed_w(klosses.re, 10, 5)));
        // `% of Power`: guard `TermPower.re <> 0 and kLosses.re > 0.0009`.
        if term_power.re != 0.0 && klosses.re > 0.0009 {
            s.push_str(&format::fixed_w(
                klosses.re / term_power.re.abs() * 100.0,
                8,
                2,
            ));
        } else {
            s.push_str(&format::fixed_w(0.0, 8, 1));
        }
        // `Format('     %.6g', kLosses.im)`.
        s.push_str(&format!("     {}\n", format::g(klosses.im, 6)));
    });

    s.push('\n');
    s.push_str(&format!(
        "{}{} kW\n",
        format::pad("LINE LOSSES=", 30),
        format::fixed_w(line.re, 10, 1)
    ));
    s.push_str(&format!(
        "{}{} kW\n",
        format::pad("TRANSFORMER LOSSES=", 30),
        format::fixed_w(trans.re, 10, 1)
    ));
    s.push('\n');
    s.push_str(&format!(
        "{}{} kW\n",
        format::pad("TOTAL LOSSES=", 30),
        format::fixed_w(total.re, 10, 1)
    ));

    // Sum served load power over the enabled Loads (`Power[1]`).
    let mut load_power = Complex64::ZERO;
    for_each_enabled_elem(classes, &ckt.loads, |_name, elem| {
        load_power += elem.terminal_power(sys, node_v, 1);
    });
    load_power *= 0.001;

    s.push('\n');
    s.push_str(&format!(
        "{}{} kW\n",
        format::pad("TOTAL LOAD POWER = ", 30),
        format::fixed_w(load_power.re.abs(), 10, 1)
    ));
    // Pascal writes the "Percent Losses" label unconditionally, the value only
    // when `LoadPower.re <> 0`.
    s.push_str(&format::pad("Percent Losses for Circuit = ", 30));
    if load_power.re != 0.0 {
        s.push_str(&format!(
            "{} %\n",
            format::fixed_w((total.re / load_power.re).abs() * 100.0, 8, 2)
        ));
    }
    s
}
