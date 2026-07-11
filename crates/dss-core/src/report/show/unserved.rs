//! `Show Unserved` (Pascal `ShowResults.pas` `ShowUnserved` (:2629)): one row per
//! **enabled** Load over its normal (or, with the `UE_Only` flag, its emergency)
//! voltage-drop criterion — the load name, its bus, `kWBase`, and the
//! `EEN_Factor`/`UE_Factor` unserved-energy factors.
//!
//! Shares the criterion logic with `export_unserved`; the `Show` form is a
//! space-padded fixed-width table (`Pad(Name,20)`/`Pad(Bus,10)` + `%8.0f`/`%9.3f`).

use num_complex::Complex64;

use crate::circuit::Circuit;
use crate::elements::pc::load::Load;
use crate::elements::traits::SysCtx;
use crate::exec::registry::DssClass;
use crate::report::format;

/// Build the `Show Unserved` text (Pascal `ShowUnserved`). Walks the Loads calling
/// the mutating `Unserved`/`ExceedsNormal` criteria (they recompute the per-phase
/// voltage from the present solution and latch `EEN_Factor`/`UE_Factor` —
/// PHASE8_PLAN §2.1). `ue_only` selects `Unserved` (emergency) over `ExceedsNormal`
/// (normal). The circuit voltage minimums seed the criteria when a load defines
/// none of its own.
pub(crate) fn show_unserved(
    classes: &mut [DssClass],
    ckt: &Circuit,
    sys: &SysCtx,
    node_v: &[Complex64],
    ue_only: bool,
) -> String {
    let norm_min = ckt.normal_min_volts;
    let emerg_min = ckt.emerg_min_volts;

    let mut s = String::new();
    s.push('\n');
    s.push_str("UNSERVED  LOAD  REPORT\n");
    s.push('\n');
    s.push_str("Load Element        Bus        Load kW  EEN Factor  UE Factor\n");
    s.push('\n');

    for &r in &ckt.loads {
        let obj = &mut classes[r.cls].objects[r.idx];
        // Take the name before the exclusive `&mut Load` borrow.
        let name = obj.data().name().to_string();
        let Some(load) = obj.as_any_mut().downcast_mut::<Load>() else {
            continue;
        };
        if !load.cd.enabled {
            continue;
        }
        let do_it = if ue_only {
            load.unserved(sys, node_v, norm_min, emerg_min)
        } else {
            load.exceeds_normal(sys, node_v, norm_min, emerg_min)
        };
        if !do_it {
            continue;
        }
        // Pascal `Pad(Name,20)`, `Pad(GetBus(1),10)`, `%8.0f kWBase`, `%9.3f
        // EEN_Factor`, `%9.3f UE_Factor` (all space-padded; the tokenizer collapses
        // the padding so the widths are not gate-load-bearing).
        s.push_str(&format::pad(&name, 20));
        s.push_str(&format::pad(load.cd.get_bus(1), 10));
        s.push_str(&format::fixed_w(load.kw_base, 8, 0));
        s.push_str(&format::fixed_w(load.een_factor, 9, 3));
        s.push_str(&format::fixed_w(load.ue_factor, 9, 3));
        s.push('\n');
    }
    s
}
