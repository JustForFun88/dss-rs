//! Pascal `TRegcontrolObj.SaveWrite` (r4133
//! `Version8/Source/Controls/RegControl.pas:1399-1421`) — the `Save`-serializer
//! override. Adopted by RP3.11 P2-P4 (§1.1: the port's override set is the
//! *union* of both upstreams', because each one exists to keep the emitted deck
//! re-compilable); dss_capi 0.14.5 dropped it, so the port never had it.
//!
//! Pascal's own reason, verbatim: *"Write Transformer name out first so that it
//! is set for later operations"*. Every other RegControl property is resolved
//! against the controlled transformer as it is parsed — `winding=`, `tapnum=`,
//! `vreg=`, `ptratio=` — so a line that re-emits them before `transformer=`
//! edits a control with no transformer yet. In the parse chain `transformer` is
//! wherever the deck typed it: for `New regcontrol.rc1 winding=2 vreg=122
//! band=3 ptratio=20 transformer=t1` this port wrote
//! `New "RegControl.rc1" Winding=2 VReg=122 Band=3 PTRatio=20 Transformer=t1`
//! (measured before this override, RP3.11 I1) where r4133 writes
//! `New "RegControl.rc1" transformer=t1 winding=2 tapwinding=2 vreg=122 band=3
//! ptratio=20`.
//!
//! (The `tapwinding` in r4133's line is a separate, already-decided *sequence*
//! divergence — r4133 stamps it as a side effect of `winding=`
//! (`RegControl.pas:480-483`), 0.14.5 deliberately dropped that stamp
//! (`CAPI:Controls/RegControl.pas:417`, *"not really required"*) and this port
//! followed. It is round-trip-safe: re-parsing `Winding=2` re-fires the same
//! `TapWinding := winding` side effect. RP3.11 §3.3.)
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
        // deviation is unobservable — written down rather than left implicit
        // (RP3.11 audit finding AC-4).
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
