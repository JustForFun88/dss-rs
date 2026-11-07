//! The RegControl `Save`-serializer override: `transformer=` is written first,
//! so the emitted deck stays re-compilable.
//!
//! Every other RegControl property is resolved against the controlled
//! transformer as it is parsed — `winding=`, `tapnum=`, `vreg=`, `ptratio=` — so
//! a line that re-emits them before `transformer=` edits a control with no
//! transformer yet. In the parse chain `transformer` is wherever the deck typed
//! it, so the generic body would write `New regcontrol.rc1 winding=2 vreg=122
//! band=3 ptratio=20 transformer=t1` with the transformer last.
//!
//! The `winding=` edit stamps no `tapwinding` into the sequence, unlike r4133.
//! The saved line is round-trip-safe: re-parsing `Winding=2` re-fires the same
//! `TapWinding := winding` side effect.
//!
//! Co-located with the element and dispatched from
//! [`crate::report::save::save::write_dss_object`] via a typed arena read.

use crate::obj::base::DssObject;
use crate::report::save::save::{SaveCtx, save_write_token};

use super::{RegControl, prop::TRANSFORMER};

impl RegControl {
    /// Pascal `TRegcontrolObj.SaveWrite` body (the caller already emitted
    /// `New "RegControl.name"`). Appends ` name=value` tokens to `out`.
    pub(crate) fn save_write_body(&self, out: &mut String, cx: &SaveCtx) {
        // Pascal `iProp := 1; If Length(PropertyValue[iProp])>0 Then Write(…)`
        // — outside the chain, and skipped when the value renders empty (the
        // `Length>0` guard `save_write_token` carries). Pascal's own guard is
        // only that length test: `save_write_token` also `trim`s and skips the
        // `----` sentinel, because it is the *generic* `SaveWrite` body. Both
        // additions can only suppress a token that would not re-parse, and no
        // reachable RegControl property renders blanks or the sentinel, so the
        // deviation is unobservable.
        save_write_token(out, cx, self, TRANSFORMER);
        // Pascal `While iProp > 0 … If iProp <> 1 Then` — every other set
        // property in chain order, the transformer never repeated.
        let mut iprop = self.data().next_property_set(None);
        while let Some(i) = iprop {
            if i != TRANSFORMER {
                save_write_token(out, cx, self, i);
            }
            iprop = self.data().next_property_set(Some(i));
        }
    }
}
