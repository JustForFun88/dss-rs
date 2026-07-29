//! `Show Taps` (Pascal `ShowResults.pas` `ShowRegulatorTaps`): one row per
//! RegControl — the controlled transformer's present/min/max tap, tap increment,
//! integer tap position, tapped winding, and the runtime reverse/cogen mode. The
//! same data as `Export Taps`, in Pascal's fixed-width table layout.

use crate::circuit::Circuit;
use crate::elements::control::reg_control::RegControl;
use crate::exec::registry::DssClass;
use crate::report::format;
use crate::report::table::{Cell, Report, Row};

/// Build the `Show Taps` text (Pascal `ShowRegulatorTaps`). Read-only: walks
/// `RegControls` (the RegControl objects in `ckt.controls`, creation order) and
/// reads the controlled transformer's tap data live.
pub(crate) fn show_taps(classes: &[DssClass], ckt: &Circuit) -> String {
    let mut rep = Report::new();
    rep.blank();
    rep.line("CONTROLLED TRANSFORMER TAP SETTINGS");
    rep.blank();
    // `'Name    RegControl        Tap      Min       Max     Step      Position
    //   Winding      Direction       CogenMode'` — the header literal as its ten
    // columns (offsets 0/8/26/35/45/53/63/77/90/106).
    rep.row(
        Row::new()
            .cell(Cell::left("Name", 8))
            .cell(Cell::left("RegControl", 18))
            .cell(Cell::left("Tap", 9))
            .cell(Cell::left("Min", 10))
            .cell(Cell::left("Max", 8))
            .cell(Cell::left("Step", 10))
            .cell(Cell::left("Position", 14))
            .cell(Cell::left("Winding", 13))
            .cell(Cell::left("Direction", 16))
            .cell(Cell::plain("CogenMode")),
    );
    rep.row(Row::blank(10));

    for &r in &ckt.controls {
        let obj = &classes[r.class_ord()].arena[r.index()];
        let Some(rc) = classes[r.class_ord()].arena.get::<RegControl>(r.index()) else {
            continue;
        };
        let Some(tref) = rc.controlled_ref() else {
            continue;
        };
        let tarena = &classes[tref.class_ord()].arena;
        let tobj = tarena.obj(tref.index());
        // Either member of the Transformer/AutoTrans proxy (Pascal walks the
        // shared `TControlledTransformerObj` base).
        let Some(tr) = tarena.try_controlled_transformer(tref.index()) else {
            continue;
        };

        let iwind = rc.tr_winding();
        let (present, max_tap, min_tap, inc) = (
            tr.present_tap(iwind as usize),
            tr.max_tap(iwind as usize),
            tr.min_tap(iwind as usize),
            tr.tap_increment(iwind as usize),
        );
        // Pascal `TapPosition(iWind) = Round((PresentTap - (Max+Min)/2)/Increment)`.
        // Pascal `Round` = ties-to-even (see RegControl `get_tap_num`).
        let position = if inc == 0.0 {
            0
        } else {
            ((present - (max_tap + min_tap) / 2.0) / inc).round_ties_even() as i32
        };
        let direction = if rc.in_reverse_mode() {
            "Reverse"
        } else {
            "Forward"
        };
        // Pascal `BoolToStr(InCogenMode, TRUE)` → "True"/"False".
        let cogen = if rc.in_cogen_mode() { "True" } else { "False" };

        // Pascal `Pad(Name, 12)` (transformer name) then `Pad(pReg.Name, 12)`,
        // each followed by a space, then
        // `Format('%8.5f %8.5f %8.5f %8.5f     %d      %d      %s      %s')`.
        rep.row(
            Row::new()
                .cell(Cell::left(tobj.data().name(), 12).sep(" "))
                .cell(Cell::left(obj.data().name(), 12).sep(" "))
                .cell(Cell::right(format::fixed(present, 5), 8).sep(" "))
                .cell(Cell::right(format::fixed(min_tap, 5), 8).sep(" "))
                .cell(Cell::right(format::fixed(max_tap, 5), 8).sep(" "))
                .cell(Cell::right(format::fixed(inc, 5), 8).sep("     "))
                .cell(Cell::plain(position.to_string()).sep("      "))
                .cell(Cell::plain(iwind.to_string()).sep("      "))
                .cell(Cell::plain(direction).sep("      "))
                .cell(Cell::plain(cogen)),
        );
    }
    rep.finish()
}
