//! `Export Unserved` (Pascal `ExportResults.pas` `ExportUnserved` (:2625)): one
//! row per **enabled** Load that is over its normal (or, with the `UE` flag, its
//! emergency) voltage-drop criterion — the load name, its bus, `kWBase`, and the
//! `EEN_Factor`/`UE_Factor` unserved-energy factors.

use num_complex::Complex64;

use crate::circuit::Circuit;
use crate::elements::pc::load::Load;
use crate::elements::traits::SysCtx;
use crate::exec::registry::DssClass;
use crate::report::format;

/// Build the `Export Unserved` body (Pascal `ExportUnserved`). Walks the Loads
/// calling the mutating `Unserved`/`ExceedsNormal` criteria (they recompute the
/// per-phase voltage from the present solution and latch `EEN_Factor`/`UE_Factor`
/// — PHASE8_PLAN §2.1). `ue_only` selects `Unserved` (emergency criterion) over
/// `ExceedsNormal` (normal criterion). The circuit voltage minimums seed the
/// criteria when a load defines none of its own.
pub(crate) fn export_unserved(
    classes: &mut [DssClass],
    ckt: &Circuit,
    sys: &SysCtx,
    node_v: &[Complex64],
    ue_only: bool,
) -> String {
    let norm_min = ckt.normal_min_volts;
    let emerg_min = ckt.emerg_min_volts;
    let mut s = String::from("Load, Bus, kW, EEN_Factor,  UE_Factor\n");
    for &r in &ckt.loads {
        let obj = &mut classes[r.class_ord()].arena[r.index()];
        // Take the name before the exclusive `&mut Load` borrow.
        let name = obj.data().name().to_string();
        let Some(load) = classes[r.class_ord()].arena.get_mut::<Load>(r.index()) else {
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
        // Pascal `AnsiUpperCase(pLoad.Name)`, `pLoad.GetBus(1)` (bus name, native
        // case), `kWBase:8:0`, `EEN_Factor:9:3`, `UE_Factor:9:3`.
        s.push_str(&format!(
            "{}, {}, {}, {}, {}\n",
            name.to_uppercase(),
            load.cd.get_bus(1),
            format::fixed(load.kw_base, 0),
            format::fixed(load.een_factor, 3),
            format::fixed(load.ue_factor, 3),
        ));
    }
    s
}
