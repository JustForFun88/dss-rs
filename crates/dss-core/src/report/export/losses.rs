//! `Export Losses` (Pascal `ExportResults.pas` `ExportLosses`): per-PD-element
//! total / load (I²R, I²X) / no-load losses in W and var.

use num_complex::Complex64;

use crate::circuit::Circuit;
use crate::elements::traits::SysCtx;
use crate::exec::registry::DssClass;
use crate::report::export::for_each_enabled_elem;
use crate::report::format;

/// Build the `Export Losses` body (Pascal `ExportLosses`): walk the PDElements,
/// calling `GetLosses` (total / load / no-load) and printing each at `%.7g`.
pub(crate) fn export_losses(
    classes: &mut [DssClass],
    ckt: &Circuit,
    sys: &SysCtx,
    node_v: &[Complex64],
) -> String {
    let mut s = String::from(
        "Element,  Total(W), Total(var),  I2R(W), I2X(var), No-load(W), No-load(var)\n",
    );
    for_each_enabled_elem(classes, &ckt.pd_elements, |name, elem| {
        let (total, load, noload) = elem.get_losses_split(sys, node_v);
        s.push_str(&format!(
            "{}, {}, {}, {}, {}, {}, {}\n",
            format::upper_elem_name(name),
            format::g(total.re, 7),
            format::g(total.im, 7),
            format::g(load.re, 7),
            format::g(load.im, 7),
            format::g(noload.re, 7),
            format::g(noload.im, 7),
        ));
    });
    s
}
