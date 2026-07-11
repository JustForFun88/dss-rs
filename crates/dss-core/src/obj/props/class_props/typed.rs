//! The typed property setters — Pascal `TDSSObjectHelper.SetDouble` /
//! `SetInteger` / `SetDoubles` / `SetIntegers` / `SetStrings`
//! (`DSSObjectHelper.pas:3060-3260`). These are the value-typed twins of the
//! string [`ClassProps::edit_property`] path: they drive the SAME `SetObj*`
//! coercion/validation logic (`setters::set_obj_double`/`set_obj_integer` and
//! the object struct-array setters) directly from an `f64`/`i32`/list, never a
//! lossy `f64 → string → f64` round-trip.
//!
//! MakePosSequence's per-class overrides mutate through these. Each Pascal
//! helper auto-wraps `if not EditingActive then BeginEdit(True) … EndEdit(1)`;
//! that editing-active bracketing lives in the exec applier (a bare `Set*` is a
//! single edit the applier wraps, a `BeginEdit`/`EndEdit` action pair brackets
//! a multi-set block), so these methods are the "run `SetObjX`, then — on
//! success — `SetAsNextSeq` + `PropertySideEffects`" core.
//!
//! **The seq-mark / side-effect skip on a flag reject is the one behavioral
//! difference from the string path.** Pascal's `SetDouble` gates the bookkeeping
//! on `Result := (DSS.ErrorNumber = 0)`, so a `SetObjDouble` that logs a
//! range/sign error (a `DoSimpleMsg`) skips BOTH `SetAsNextSeq` and
//! `PropertySideEffects`. The string `ParseObjPropertyValue` instead returns
//! `Result := True` after the same reject, so the `Edit` loop marks the sequence
//! and runs side effects anyway (reproduced in [`ClassProps::edit_property`]).
//! Here we detect the reject by watching `eng.errors` grow across the
//! `set_obj_*` call — an `IgnoreInvalid` reject logs nothing, so it still marks,
//! exactly as Pascal (no `DoSimpleMsg` → `ErrorNumber` unchanged).

use crate::obj::base::DssObject;
use crate::obj::props::PropFlags;
use crate::obj::props::setters::{set_obj_double, set_obj_integer};

use super::ClassProps;

impl ClassProps {
    /// Pascal `TDSSObjectHelper.SetDouble` core: `SetObjDouble` (scale + range/
    /// sign/zero-trap/inverse checks) then, only if no error was logged,
    /// `SetAsNextSeq(idx)` + `PropertySideEffects(idx, 0)`.
    pub fn set_prop_f64(
        &self,
        obj: &mut dyn DssObject,
        idx: usize,
        value: f64,
        eng: &mut crate::obj::props::PropEngine,
    ) {
        let pd = &self.props[idx];
        let full = format!("{}.{}", self.class_name, obj.data().name());
        let scale = if pd.flags.contains(PropFlags::SCALED_BY_FUNCTION) {
            obj.prop_scale(idx, false)
        } else {
            pd.scale
        };
        let before = eng.errors.len();
        set_obj_double(pd, obj, idx, value, scale, eng, &full);
        if eng.errors.len() == before {
            obj.data_mut().set_as_next_seq(idx);
            obj.side_effects(idx, 0);
        }
    }

    /// Pascal `TDSSObjectHelper.SetInteger` core: `SetObjInteger` (range/sign +
    /// `ValueOffset`, returning the previous value) then, only on success,
    /// `SetAsNextSeq(idx)` + `PropertySideEffects(idx, prevInt)`.
    pub fn set_prop_i32(
        &self,
        obj: &mut dyn DssObject,
        idx: usize,
        value: i32,
        eng: &mut crate::obj::props::PropEngine,
    ) {
        let pd = &self.props[idx];
        let full = format!("{}.{}", self.class_name, obj.data().name());
        let before = eng.errors.len();
        let prev = set_obj_integer(pd, obj, idx, value, eng, &full);
        if eng.errors.len() == before {
            obj.data_mut().set_as_next_seq(idx);
            obj.side_effects(idx, prev);
        }
    }

    /// Pascal `TDSSObjectHelper.SetDoubles` onto a `DoubleArrayOnStructArray`
    /// property (per-winding `kVs`/`kVAs`): each `Some` entry is scaled by
    /// `PropertyScale` (mirroring the string `DoubleArrayOnStruct` parse arm),
    /// `None` keeps the prior struct entry; then `SetAsNextSeq` +
    /// `PropertySideEffects(idx, 0)`. The object setter also advances the
    /// struct-array cursor to the count (`ActiveWinding := NumWindings`).
    pub fn set_prop_struct_f64s(
        &self,
        obj: &mut dyn DssObject,
        idx: usize,
        values: &[Option<f64>],
        eng: &mut crate::obj::props::PropEngine,
    ) {
        let pd = &self.props[idx];
        let scale = pd.scale;
        let scaled: Vec<Option<f64>> = values.iter().map(|v| v.map(|x| x * scale)).collect();
        let before = eng.errors.len();
        obj.set_struct_f64_array(idx, &scaled);
        if eng.errors.len() == before {
            obj.data_mut().set_as_next_seq(idx);
            obj.side_effects(idx, 0);
        }
    }

    /// Pascal `TDSSObjectHelper.SetIntegers` onto a
    /// `MappedStringEnumArrayOnStructArray` property (per-winding `conns`):
    /// write the ordinals, then `SetAsNextSeq` + `PropertySideEffects(idx, 0)`.
    pub fn set_prop_struct_i32s(
        &self,
        obj: &mut dyn DssObject,
        idx: usize,
        values: &[i32],
        eng: &mut crate::obj::props::PropEngine,
    ) {
        let before = eng.errors.len();
        obj.set_struct_i32_array(idx, values);
        if eng.errors.len() == before {
            obj.data_mut().set_as_next_seq(idx);
            obj.side_effects(idx, 0);
        }
    }

    /// Pascal `TDSSObjectHelper.SetStrings` onto the `BusesOnStructArray`
    /// property (`SetStrings(ord(TProp.Buses), new_buses, [])` in the
    /// Transformer/AutoTrans overrides): every name is set (`None` never
    /// occurs — the override supplies all windings), the cursor advances to the
    /// count, then `SetAsNextSeq` + `PropertySideEffects`. The `Buses` property
    /// index is resolved by name (both transformer classes expose exactly one
    /// `buses` struct array), so the frozen `SetStructBuses(Vec<String>)` action
    /// needs no index. A class without a `buses` property is a no-op (defensive;
    /// only the transformer family emits this action).
    pub fn set_prop_struct_buses(
        &self,
        obj: &mut dyn DssObject,
        names: &[String],
        eng: &mut crate::obj::props::PropEngine,
    ) {
        let Some(idx) = self.property_index("buses") else {
            return;
        };
        let vals: Vec<Option<String>> = names.iter().map(|s| Some(s.clone())).collect();
        let before = eng.errors.len();
        obj.set_struct_buses(&vals);
        if eng.errors.len() == before {
            obj.data_mut().set_as_next_seq(idx);
            obj.side_effects(idx, 0);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::elements::general::line_code::{LineCodeObj, class_props, prop};
    use crate::obj::base::DssObject;
    use crate::obj::dss_enum::EnumRegistry;
    use crate::obj::props::PropEngine;
    use dss_parser::{Parser, ParserVars};

    /// Run a closure with a fresh `PropEngine` over a scratch parser, returning
    /// the accumulated errors.
    fn with_engine<F: FnOnce(&mut PropEngine)>(enums: &EnumRegistry, f: F) -> Vec<String> {
        let mut parser = Parser::new();
        let vars = ParserVars::new();
        let mut errors = Vec::new();
        let mut eng = PropEngine {
            parser: &mut parser,
            vars: &vars,
            enums,
            errors: &mut errors,
            foreign: None,
        };
        f(&mut eng);
        errors
    }

    /// Parity: the typed `set_prop_f64` and the string `edit_property` must land
    /// IDENTICAL state for a scaled property (LineCode `C1`, nF→F, scale 1e-9),
    /// and advance `prp_sequence` identically. No f64→string→f64 round-trip.
    #[test]
    fn typed_f64_matches_string_path_for_scaled_prop() {
        let enums = EnumRegistry::new();
        let cls = class_props(&enums);
        let idx = cls.property_index("C1").unwrap();

        // String path: `C1=3.4` (interpreted nF → 3.4e-9 F).
        let mut a = LineCodeObj::new("lc_a");
        with_engine(&enums, |eng| {
            cls.edit_property(&mut a, idx, "3.4", eng).unwrap();
        });
        a.end_edit();

        // Typed path: `set_prop_f64(idx, 3.4)`.
        let mut b = LineCodeObj::new("lc_b");
        with_engine(&enums, |eng| {
            cls.set_prop_f64(&mut b, idx, 3.4, eng);
        });
        b.end_edit();

        // Stored raw value: scale applied once, identically.
        assert_eq!(a.get_f64(prop::C1), b.get_f64(prop::C1));
        assert!((a.get_f64(prop::C1) - 3.4e-9).abs() < 1e-24);
        // Sequence advanced identically (both marked C1, same counter value).
        assert!(a.data().prp_specified(prop::C1));
        assert!(b.data().prp_specified(prop::C1));
        assert_eq!(
            a.data().next_property_set(None),
            b.data().next_property_set(None)
        );
    }

    /// A flag-check reject (`NON_ZERO` `BaseFreq = 0`) must skip BOTH the
    /// seq-mark and the side effects on the typed path — Pascal's `SetDouble`
    /// `Result := (ErrorNumber = 0)` gate — unlike the string path, which marks
    /// the sequence regardless (its `ParseObjPropertyValue` returns `True`).
    #[test]
    fn typed_f64_flag_reject_skips_seq_and_side_effects() {
        let enums = EnumRegistry::new();
        let cls = class_props(&enums);
        let idx = cls.property_index("BaseFreq").unwrap();

        // Typed path rejects 0 (NON_ZERO): error logged, field/seq unchanged.
        let mut b = LineCodeObj::new("lc_b");
        let prev = b.get_f64(idx);
        let errs = with_engine(&enums, |eng| {
            cls.set_prop_f64(&mut b, idx, 0.0, eng);
        });
        assert_eq!(errs.len(), 1, "reject logs exactly one error: {errs:?}");
        assert_eq!(b.get_f64(idx), prev, "rejected value must not be written");
        assert!(
            !b.data().prp_specified(idx),
            "typed reject must NOT mark the sequence"
        );

        // String path, same reject: marks the sequence anyway (contrast).
        let mut a = LineCodeObj::new("lc_a");
        with_engine(&enums, |eng| {
            cls.edit_property(&mut a, idx, "0", eng).unwrap();
        });
        assert!(
            a.data().prp_specified(idx),
            "string path marks the sequence even on a flag reject"
        );
    }

    /// The typed integer twin advances the sequence and applies the same
    /// `SetObjInteger` write as the string path (LineCode `NPhases`).
    #[test]
    fn typed_i32_matches_string_path() {
        let enums = EnumRegistry::new();
        let cls = class_props(&enums);
        let idx = cls.property_index("NPhases").unwrap();

        let mut a = LineCodeObj::new("lc_a");
        with_engine(&enums, |eng| {
            cls.edit_property(&mut a, idx, "1", eng).unwrap();
        });
        a.end_edit();

        let mut b = LineCodeObj::new("lc_b");
        with_engine(&enums, |eng| {
            cls.set_prop_i32(&mut b, idx, 1, eng);
        });
        b.end_edit();

        assert_eq!(a.get_i32(idx), b.get_i32(idx));
        assert_eq!(a.get_i32(idx), 1);
        assert_eq!(
            a.data().next_property_set(None),
            b.data().next_property_set(None)
        );
    }
}
