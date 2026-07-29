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
use crate::report::table::{Cell, Report, Row};

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

    let mut rep = Report::new();
    rep.blank();
    rep.line("UNSERVED  LOAD  REPORT");
    rep.blank();
    // `'Load Element        Bus        Load kW  EEN Factor  UE Factor'` — the
    // header literal as its five columns (offsets 0/20/31/40/52).
    rep.row(
        Row::new()
            .cell(Cell::left("Load Element", 20))
            .cell(Cell::left("Bus", 11))
            .cell(Cell::left("Load kW", 9))
            .cell(Cell::left("EEN Factor", 12))
            .cell(Cell::plain("UE Factor")),
    );
    rep.row(Row::blank(5));

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
        // Pascal `Pad(Name,20)`, `Pad(GetBus(1),10)`, `%8.0f kWBase`, `%9.3f
        // EEN_Factor`, `%9.3f UE_Factor` (all space-padded; the tokenizer collapses
        // the padding so the widths are not gate-load-bearing).
        rep.row(
            Row::new()
                .cell(Cell::left(name, 20))
                .cell(Cell::left(load.cd.get_bus(1), 10))
                .cell(Cell::right(format::fixed(load.kw_base, 0), 8))
                .cell(Cell::right(format::fixed(load.een_factor, 3), 9))
                .cell(Cell::right(format::fixed(load.ue_factor, 3), 9)),
        );
    }
    rep.finish()
}
