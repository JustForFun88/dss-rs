//! Base per-object data and the object trait, the Rust replacement for
//! Pascal `TDSSObject` (DSSObject.pas). Pascal reaches into object fields by
//! raw pointer offset; we can't, so the generic property engine in
//! [`crate::obj::props`] drives a small typed accessor trait
//! ([`DssObject`]) that each class implements with `match idx` arms — the
//! 1:1 stand-in for `SetObjDouble`/`GetObjInteger`/... pointer pokes.

/// Shared object state every DSS object carries (`TDSSObject` fields that
/// matter to the port so far): its name and the property set-order tracker.
///
/// Properties are addressed 1-based, exactly as in Pascal, so `prp_sequence`
/// has `num_props + 1` slots and slot 0 is the monotonic counter
/// (`PrpSequence[0]`).
#[derive(Debug, Clone)]
pub struct DssObjData {
    /// Lowercased local name (`TNamedObject.LocalName`).
    name: String,
    /// Pascal `PrpSequence`: `[0]` is the counter, `[i]` is the order in
    /// which property `i` was last set (0 = never set).
    prp_sequence: Vec<u32>,
    /// Deferred `DoSimpleMsg`/`DoErrorMsg` messages emitted by
    /// `side_effects`/`end_edit` (which run without direct access to the
    /// engine error sink). The executive drains these right after the edit
    /// loop, so the message ordering within a command is preserved.
    deferred_errors: Vec<String>,
}

impl DssObjData {
    pub fn new(name: impl Into<String>, num_props: usize) -> Self {
        Self {
            name: name.into(),
            prp_sequence: vec![0; num_props + 1],
            deferred_errors: Vec::new(),
        }
    }

    /// Queue a `DoSimpleMsg`-style message from inside a property hook; the
    /// executive collects it after the active edit finishes.
    pub fn push_error(&mut self, msg: impl Into<String>) {
        self.deferred_errors.push(msg.into());
    }

    /// Drain the queued messages (Pascal would have already logged them).
    pub fn take_errors(&mut self) -> Vec<String> {
        std::mem::take(&mut self.deferred_errors)
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// Pascal `Set_Name`. Names are stored lowercased by the object
    /// constructors (`Name := AnsiLowerCase(...)`); callers pass the
    /// already-normalized form.
    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
    }

    /// Pascal `SetAsNextSeq`: record that property `index` was just set, so
    /// `Save` can later replay edits in the order they happened.
    pub fn set_as_next_seq(&mut self, index: usize) {
        self.prp_sequence[0] += 1;
        self.prp_sequence[index] = self.prp_sequence[0];
    }

    /// Whether property `index` was explicitly set (Pascal `PrpSpecified`).
    pub fn prp_specified(&self, index: usize) -> bool {
        self.prp_sequence.get(index).copied().unwrap_or(0) != 0
    }

    /// Pascal `PrpSequence[index] := 0`: spec-set side effects clear the
    /// "explicitly set" marks of competing properties.
    pub fn clear_seq(&mut self, index: usize) {
        if index < self.prp_sequence.len() {
            self.prp_sequence[index] = 0;
        }
    }

    /// Pascal `TDSSObject.MakeLike`: the base-class part of `like=` copies the
    /// source's whole `PrpSequence` (counter slot included) onto the target,
    /// so `Save` later writes the copied properties as explicitly set. Class
    /// `make_like` impls call this first, mirroring `inherited MakeLike`.
    pub fn copy_prp_sequence_from(&mut self, other: &DssObjData) {
        self.prp_sequence.clone_from(&other.prp_sequence);
    }

    /// Pascal `GetNextPropertySet`: the property index whose set-order is the
    /// smallest one still greater than that of `after` (pass `None` to start).
    /// Returns `None` when there are no more — drives `SaveWrite` ordering.
    pub fn next_property_set(&self, after: Option<usize>) -> Option<usize> {
        let threshold = match after {
            Some(i) => self.prp_sequence.get(i).copied().unwrap_or(0),
            None => 0,
        };
        let mut smallest = u32::MAX;
        let mut result = None;
        for (i, &seq) in self.prp_sequence.iter().enumerate().skip(1) {
            if seq != 0 && seq > threshold && seq < smallest {
                smallest = seq;
                result = Some(i);
            }
        }
        result
    }
}

/// The typed field accessors the property engine calls, keyed by the 1-based
/// property index. Each concrete class implements only the kinds it actually
/// uses; the defaults panic so a wrong dispatch surfaces as an obvious bug
/// rather than silent data corruption (this mirrors the Pascal base
/// `CustomSetRaw` "base ... reached" guard).
#[allow(unused_variables)]
pub trait DssObject {
    fn data(&self) -> &DssObjData;
    fn data_mut(&mut self) -> &mut DssObjData;

    /// `&dyn Any` view for the rare flows that need a concrete downcast
    /// (`MakeLike` between circuit elements copies matrices that the typed
    /// accessors cannot express).
    fn as_any(&self) -> &dyn std::any::Any;

    /// Circuit-element view (Pascal `obj is TDSSCktElement`). `None` for
    /// `DSS_OBJECT` classes like TCC_Curve and Spectrum.
    fn as_ckt_element(&self) -> Option<&dyn crate::elements::traits::CktElement> {
        None
    }
    fn as_ckt_element_mut(&mut self) -> Option<&mut dyn crate::elements::traits::CktElement> {
        None
    }

    fn get_f64(&self, idx: usize) -> f64 {
        unreachable!("get_f64 not implemented for property {idx}")
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        unreachable!("set_f64 not implemented for property {idx}")
    }
    fn get_i32(&self, idx: usize) -> i32 {
        unreachable!("get_i32 not implemented for property {idx}")
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        unreachable!("set_i32 not implemented for property {idx}")
    }
    fn get_bool(&self, idx: usize) -> bool {
        unreachable!("get_bool not implemented for property {idx}")
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        unreachable!("set_bool not implemented for property {idx}")
    }
    fn get_string(&self, idx: usize) -> String {
        unreachable!("get_string not implemented for property {idx}")
    }
    fn set_string(&mut self, idx: usize, value: String) {
        unreachable!("set_string not implemented for property {idx}")
    }
    /// `None` mirrors a NIL Pascal array pointer (dumps as an empty string).
    fn get_f64_array(&self, idx: usize) -> Option<&[f64]> {
        unreachable!("get_f64_array not implemented for property {idx}")
    }
    fn set_f64_array(&mut self, idx: usize, value: Vec<f64>) {
        unreachable!("set_f64_array not implemented for property {idx}")
    }

    /// `BusProperty` write: `terminal` is 1-based (`PropertyOffset`); the
    /// element lowercases and flags `BusNameRedefined` (Pascal `SetBus`).
    fn set_bus_name(&mut self, terminal: usize, value: &str) {
        unreachable!("set_bus_name not implemented (terminal {terminal})")
    }
    fn get_bus_name(&self, terminal: usize) -> String {
        unreachable!("get_bus_name not implemented (terminal {terminal})")
    }

    /// `ComplexProperty` / `ComplexPartsProperty`: both parse a 2-vector
    /// `(re, im)`; the class stores it as one `Complex` field or two doubles.
    fn get_complex(&self, idx: usize) -> (f64, f64) {
        unreachable!("get_complex not implemented for property {idx}")
    }
    fn set_complex(&mut self, idx: usize, re: f64, im: f64) {
        unreachable!("set_complex not implemented for property {idx}")
    }

    /// `ComplexPartSymMatrixProperty` write: `values` is the full `order²`
    /// column-major matrix already scaled; `real` selects the re/im part
    /// (Pascal writes one part with stride 2, preserving the other).
    fn set_matrix_part(&mut self, idx: usize, values: &[f64], order: usize, real: bool) {
        unreachable!("set_matrix_part not implemented for property {idx}")
    }
    /// Read one part of the matrix back, column-major, unscaled; `None` when
    /// the matrix is not allocated.
    fn get_matrix_part(&self, idx: usize, real: bool) -> Option<(Vec<f64>, usize)> {
        unreachable!("get_matrix_part not implemented for property {idx}")
    }

    /// Pascal `ScaledByFunction` (`PropertyOffset2` holding a function
    /// pointer): the per-class scale for property `idx`. The engine multiplies
    /// parsed values by `prop_scale(idx, false)` and divides dumps by
    /// `prop_scale(idx, true)`.
    fn prop_scale(&self, idx: usize, getter: bool) -> f64 {
        1.0
    }

    /// Pascal `TPropertyFlag.ConditionalValue` (`PropertyOffset3` holding a
    /// `LongBool`): whether property `idx`'s value should be displayed. When a
    /// `CONDITIONAL_VALUE` property returns `false` here, the getter renders
    /// the Pascal placeholder `----` instead of the stored value (e.g. a
    /// LineCode's `R1` once a matrix model has replaced the sym-component one).
    fn prop_conditional(&self, idx: usize) -> bool {
        let _ = idx;
        true
    }

    /// Pascal `PropertySideEffects`: run after property `idx` is written.
    /// `prev_int` is the integer value the property held beforehand (only
    /// meaningful for integer/boolean/enum properties; 0 otherwise).
    fn side_effects(&mut self, idx: usize, prev_int: i32) {
        let _ = (idx, prev_int);
    }

    /// Pascal per-class `EndEdit`: recompute derived state once an edit block
    /// finishes (e.g. `ReCalcYearMult`, `SetMultArray`). No-op by default.
    fn end_edit(&mut self) {}

    /// Pascal `TDSSObject.MakeLike`: copy `other`'s field state onto `self`
    /// (the name is *not* copied). Default is a no-op; classes override it.
    /// `other` is the same concrete class as `self`, so the implementation can
    /// read it through the typed accessors.
    fn make_like(&mut self, other: &dyn DssObject) {
        let _ = other;
    }

    /// Clone this object behind the trait object, so the executive can copy a
    /// `MakeLike` source out of its arena without aliasing the target.
    fn clone_box(&self) -> Box<dyn DssObject>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_as_next_seq_tracks_order() {
        let mut d = DssObjData::new("t", 4);
        assert!(!d.prp_specified(2));
        d.set_as_next_seq(3);
        d.set_as_next_seq(1);
        d.set_as_next_seq(3); // re-setting bumps it to the latest order
        assert!(d.prp_specified(3));
        assert!(d.prp_specified(1));
        assert!(!d.prp_specified(2));
        // SaveWrite walk: order is 1 (seq 2) then 3 (seq 3); 3's first set
        // (seq 1) is superseded.
        assert_eq!(d.next_property_set(None), Some(1));
        assert_eq!(d.next_property_set(Some(1)), Some(3));
        assert_eq!(d.next_property_set(Some(3)), None);
    }
}
