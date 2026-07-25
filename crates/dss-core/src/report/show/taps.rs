//! `Show Taps` (Pascal `ShowResults.pas` `ShowRegulatorTaps`): one row per
//! RegControl — the controlled transformer's present/min/max tap, tap increment,
//! integer tap position, tapped winding, and the runtime reverse/cogen mode. The
//! same data as `Export Taps`, in Pascal's fixed-width table layout.

use crate::circuit::Circuit;
use crate::elements::control::reg_control::RegControl;
use crate::elements::pd::transformer::as_controlled_transformer;
use crate::exec::registry::DssClass;
use crate::report::format;

/// Build the `Show Taps` text (Pascal `ShowRegulatorTaps`). Read-only: walks
/// `RegControls` (the RegControl objects in `ckt.controls`, creation order) and
/// reads the controlled transformer's tap data live.
pub(crate) fn show_taps(classes: &[DssClass], ckt: &Circuit) -> String {
    let mut s = String::new();
    s.push('\n');
    s.push_str("CONTROLLED TRANSFORMER TAP SETTINGS\n");
    s.push('\n');
    s.push_str(
        "Name    RegControl        Tap      Min       Max     Step      Position      Winding      Direction       CogenMode\n",
    );
    s.push('\n');

    for &r in &ckt.controls {
        let obj = &classes[r.cls].arena[r.idx];
        let Some(rc) = obj.as_any().downcast_ref::<RegControl>() else {
            continue;
        };
        let Some(tref) = rc.controlled_ref() else {
            continue;
        };
        let tobj = &classes[tref.cls].arena[tref.idx];
        // Either member of the Transformer/AutoTrans proxy (Pascal walks the
        // shared `TControlledTransformerObj` base).
        let Some(tr) = as_controlled_transformer(tobj) else {
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

        // Pascal `Pad(Name, 12)` (transformer name) then `Pad(pReg.Name, 12)`,
        // each followed by a space, then the fixed-format numeric row.
        s.push_str(&format::pad(tobj.data().name(), 12));
        s.push(' ');
        s.push_str(&format::pad(obj.data().name(), 12));
        s.push(' ');
        // `Format('%8.5f %8.5f %8.5f %8.5f     %d      %d      %s      %s')`.
        s.push_str(&format!(
            "{} {} {} {}     {}      {}      {}      {}\n",
            format::fixed_w(present, 8, 5),
            format::fixed_w(min_tap, 8, 5),
            format::fixed_w(max_tap, 8, 5),
            format::fixed_w(inc, 8, 5),
            position,
            iwind,
            direction,
            cogen,
        ));
    }
    s
}
