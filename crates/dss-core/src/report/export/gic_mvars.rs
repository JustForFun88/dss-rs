//! `Export GICMvars` (Pascal `ExportResults.pas` `ExportGICMvar` +
//! `TGICTransformerObj.WriteVarOutputRecord`): one row per GICTransformer — the
//! bus, the reactive (Mvar) demand the winding GIC drives, and the per-phase GIC
//! magnitude. A GMD-study report of transformer var loading under quasi-DC GIC.

use num_complex::Complex64;

use crate::circuit::Circuit;
use crate::elements::pd::GicTransformer;
use crate::elements::traits::SysCtx;
use crate::exec::registry::DssClass;
use crate::report::format;

/// Build the `Export GICMvars` body (Pascal `ExportGICMvar`). Walks the
/// **GICTransformer class** ElementList in creation order (Pascal
/// `GICClass.ElementList.First`/`.Next`, no `Enabled` filter), invoking each
/// element's `var_output_record` (mutating: recomputes `Iterminal`).
pub(crate) fn export_gic_mvars(
    classes: &mut [DssClass],
    _ckt: &Circuit,
    sys: &SysCtx,
    node_v: &[Complex64],
) -> String {
    let mut s = String::from("Bus, Mvar, GIC Amps per phase\n");
    let Some(ci) = classes
        .iter()
        .position(|c| c.props.class_name().eq_ignore_ascii_case("GICTransformer"))
    else {
        return s;
    };
    for idx in 0..classes[ci].arena.len() {
        let Some(gt) = classes[ci].arena.get_mut::<GicTransformer>(idx) else {
            continue;
        };
        let (bus, mvar, gic) = gt.var_output_record(sys, node_v);
        // Pascal `Format('%s, %.8g, %.8g', [GetBus(1), MVarMag, GICperPhase])`.
        s.push_str(&format!(
            "{}, {}, {}\n",
            bus,
            format::g(mvar, 8),
            format::g(gic, 8)
        ));
    }
    s
}
