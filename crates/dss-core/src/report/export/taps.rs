//! `Export Taps` (Pascal `ExportResults.pas` `ExportTaps`): one row per
//! RegControl — the controlled transformer's present/min/max tap, tap increment,
//! integer tap position, tapped winding, and the runtime reverse/cogen mode.

use crate::circuit::Circuit;
use crate::elements::control::reg_control::RegControl;
use crate::elements::pd::transformer::as_controlled_transformer;
use crate::exec::registry::DssClass;
use crate::report::format;

/// Build the `Export Taps` body (Pascal `ExportTaps`). Read-only: walks
/// `RegControls` (the RegControl objects in `ckt.controls`, creation order) and
/// reads the controlled transformer's tap data live.
pub(crate) fn export_taps(classes: &[DssClass], ckt: &Circuit) -> String {
    let mut s = String::from(
        "Name, RegControl, Tap, Min, Max, Step, Position, Winding, Direction, CogenMode\n",
    );
    for &r in &ckt.controls {
        let obj = &classes[r.cls].objects[r.idx];
        let Some(rc) = obj.as_any().downcast_ref::<RegControl>() else {
            continue;
        };
        let Some(tref) = rc.controlled_ref() else {
            continue;
        };
        let tobj = &classes[tref.cls].objects[tref.idx];
        // Either member of the Transformer/AutoTrans proxy (Pascal walks the
        // shared `TControlledTransformerObj` base).
        let Some(tr) = as_controlled_transformer(&**tobj) else {
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
        // TODO(compat): FPC `Round` is ties-to-even (see RegControl `get_tap_num`).
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

        // Pascal writes the *Transformer* name first (unquoted, `with pReg.Transformer`),
        // then `Format(', %s , %8.5f, …')` with the RegControl name (note the space
        // padding, trimmed by the comparator).
        s.push_str(&format!(
            "{}, {} , {}, {}, {}, {}, {}, {}, {}, {}\n",
            tobj.data().name(),
            obj.data().name(),
            format::fixed(present, 5),
            format::fixed(min_tap, 5),
            format::fixed(max_tap, 5),
            format::fixed(inc, 5),
            position,
            iwind,
            direction,
            cogen,
        ));
    }
    s
}
