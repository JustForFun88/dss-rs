//! `LineSpacing` (`TLineSpacingObj`) — overhead-line conductor spacing geometry.
//! Port of Pascal `General/LineSpacing.pas`.
//!
//! A `DSS_OBJECT` catalog class (no terminals, no YPrim): a `LineGeometry`
//! references it by name and reads the per-conductor horizontal/vertical
//! coordinate arrays (`X`/`H`, in `Units`) plus `NConds`/`NPhases` to build the
//! Carson `LineConstants` matrices. The two coordinate arrays are
//! `DoubleVArrayProperty` with the element count taken from `FNConds`
//! (`PropertyOffset2 = @FNConds`), so they are sized — and re-sized — by the
//! `nconds` property's side effect.

use crate::obj::base::{DssObjData, DssObject};
use crate::obj::props::{PropDef, PropFlags, define_properties};

/// Pascal `LineUnits.UNITS_FT` — the `ft` ordinal of `DSS.UnitsEnum`; the
/// default unit and the value the `nconds` side effect resets `Units` to.
const UNITS_FT: i32 = 5;

define_properties! {
    class "LineSpacing", abbrev true, enums enums;
    1 NCONDS  => PropDef::integer("nconds").flags(PropFlags::SUPPRESS_JSON);
    2 NPHASES => PropDef::integer("nphases");
    3 X       => PropDef::double_v_array("x");
    4 H       => PropDef::double_v_array("h");
    5 UNITS   => PropDef::mapped_string_enum("units", enums.units);
}

/// `TLineSpacingObj`. Pascal stores `FX`/`FY` as 1-based `pDoubleArray`s of
/// length `FNConds`; here they are plain `Vec<f64>` kept at `fnconds` elements.
#[derive(Debug, Clone)]
pub struct LineSpacingObj {
    data: DssObjData,
    fx: Vec<f64>,
    fy: Vec<f64>,
    fnconds: i32,
    nphases: i32,
    units: i32,
}

impl LineSpacingObj {
    pub fn new(name: impl Into<String>) -> Self {
        // Pascal `TLineSpacingObj.Create`: FNConds := 3, then the `nconds`
        // side effect sizes FX/FY to 3 (and sets Units := ft); the arrays are
        // then zeroed and NPhases := 3.
        let mut obj = Self {
            data: DssObjData::new(name.into().to_lowercase(), prop::NUM_PROPS),
            fx: Vec::new(),
            fy: Vec::new(),
            fnconds: 3,
            nphases: 3,
            units: UNITS_FT,
        };
        obj.realloc_conductors();
        obj
    }

    /// Pascal `NWires` (= `FNConds`): the conductor count a `LineGeometry`
    /// matches against when it reads this spacing's coordinates.
    pub fn nwires(&self) -> i32 {
        self.fnconds
    }
    /// Pascal `Xcoord`/`Ycoord` arrays (length `FNConds`) and `Units` — read by
    /// `TLineGeometryObj`'s `spacing=` side effect.
    pub fn xcoord(&self) -> &[f64] {
        &self.fx
    }
    pub fn ycoord(&self) -> &[f64] {
        &self.fy
    }
    pub fn spacing_units(&self) -> i32 {
        self.units
    }

    /// Pascal `nconds` `PropertySideEffects`: `ReAllocmem(FX/FY, FNConds)`.
    /// Pascal leaves grown entries uninitialized; we zero-fill the tail (the
    /// preserved leading entries match, and undefined upstream memory is not a
    /// behaviour the goldens pin). Shrinking truncates, as Pascal does.
    fn realloc_conductors(&mut self) {
        let n = self.fnconds.max(0) as usize;
        self.fx.resize(n, 0.0);
        self.fy.resize(n, 0.0);
    }
}

impl DssObject for LineSpacingObj {
    fn data(&self) -> &DssObjData {
        &self.data
    }
    fn data_mut(&mut self) -> &mut DssObjData {
        &mut self.data
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn get_i32(&self, idx: usize) -> i32 {
        match idx {
            prop::NCONDS => self.fnconds,
            prop::NPHASES => self.nphases,
            prop::UNITS => self.units,
            _ => unreachable!("LineSpacing has no integer at {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        match idx {
            prop::NCONDS => self.fnconds = value,
            prop::NPHASES => self.nphases = value,
            prop::UNITS => self.units = value,
            _ => unreachable!("LineSpacing has no integer at {idx}"),
        }
    }

    fn get_f64_array(&self, idx: usize) -> Option<&[f64]> {
        let arr = match idx {
            prop::X => &self.fx,
            prop::H => &self.fy,
            _ => unreachable!("LineSpacing has no double array at {idx}"),
        };
        // Pascal `ReAllocmem(FX, 0)` (the `nconds=0` side effect) frees the
        // buffer and leaves the pointer nil, so `GetDSSArray` returns the empty
        // string `''` — not `'[]'`. Mirror that: an empty coordinate array reads
        // as nil. (For `nconds < 0` the Pascal realloc raises an exception; we
        // clamp the size to 0, so that degenerate path likewise reads `''` and
        // is not oracle-pinnable.)
        if arr.is_empty() {
            None
        } else {
            Some(arr.as_slice())
        }
    }
    fn set_f64_array(&mut self, idx: usize, value: Vec<f64>) {
        match idx {
            prop::X => self.fx = value,
            prop::H => self.fy = value,
            _ => unreachable!("LineSpacing has no double array at {idx}"),
        }
    }

    /// Both `X` and `H` are sized by `FNConds` (`PropertyOffset2 = @FNConds`).
    fn array_size(&self, idx: usize) -> usize {
        match idx {
            prop::X | prop::H => self.fnconds.max(0) as usize,
            _ => unreachable!("LineSpacing has no function-sized array at {idx}"),
        }
    }

    fn side_effects(&mut self, idx: usize, _prev_int: i32) {
        // Pascal `TLineSpacingObj.PropertySideEffects`: `nconds` resizes the
        // coordinate arrays and resets the unit to feet; the rest only flag
        // `DataChanged`, which we do not track.
        if idx == prop::NCONDS {
            self.realloc_conductors();
            self.units = UNITS_FT;
        }
    }

    fn make_like(&mut self, other: &dyn DssObject) {
        self.data.copy_prp_sequence_from(other.data());
        if let Some(o) = other.as_any().downcast_ref::<LineSpacingObj>() {
            // Pascal `MakeLike`: copy FNConds, run the `nconds` side effect
            // (resize + Units := ft), copy NPhases, then the X/Y arrays, and
            // finally `Units := Other.Units` (overriding the side effect).
            self.fnconds = o.fnconds;
            self.realloc_conductors();
            self.nphases = o.nphases;
            let n = self.fnconds.max(0) as usize;
            self.fx[..n].copy_from_slice(&o.fx[..n]);
            self.fy[..n].copy_from_slice(&o.fy[..n]);
            self.units = o.units;
        }
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::obj::dss_enum::EnumRegistry;
    use crate::obj::props::{ClassProps, PropEngine};
    use dss_parser::{Parser, ParserVars};

    fn apply(cls: &ClassProps, obj: &mut dyn DssObject, edits: &[(&str, &str)]) -> Vec<String> {
        let enums = EnumRegistry::new();
        let mut parser = Parser::new();
        let vars = ParserVars::new();
        let mut errors = Vec::new();
        for (name, value) in edits {
            let idx = cls.property_index(name).expect("known property");
            let mut eng = PropEngine {
                parser: &mut parser,
                vars: &vars,
                enums: &enums,
                errors: &mut errors,
                foreign: None,
            };
            cls.edit_property(obj, idx, value, &mut eng).unwrap();
        }
        obj.end_edit();
        errors.extend(obj.data_mut().take_errors());
        errors
    }

    fn get(cls: &ClassProps, obj: &dyn DssObject, name: &str) -> String {
        let enums = EnumRegistry::new();
        let idx = cls.property_index(name).unwrap();
        cls.get_value(obj, idx, &enums)
    }

    #[test]
    fn defaults() {
        // Pascal Create: nconds=3, nphases=3, units=ft, x/h all zero.
        let enums = EnumRegistry::new();
        let cls = class_props(&enums);
        let obj = LineSpacingObj::new("ls");
        assert_eq!(get(&cls, &obj, "nconds"), "3");
        assert_eq!(get(&cls, &obj, "nphases"), "3");
        assert_eq!(get(&cls, &obj, "units"), "ft");
        assert_eq!(get(&cls, &obj, "x"), "[ 0 0 0]");
        assert_eq!(get(&cls, &obj, "h"), "[ 0 0 0]");
    }

    #[test]
    fn nconds_resizes_arrays_and_resets_units() {
        // Setting nconds reallocates X/H to the new length and resets Units to
        // ft; X/H read with the new count (zero-filled here).
        let enums = EnumRegistry::new();
        let cls = class_props(&enums);
        let mut obj = LineSpacingObj::new("ls");
        let errs = apply(&cls, &mut obj, &[("units", "m"), ("nconds", "2")]);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(get(&cls, &obj, "nconds"), "2");
        assert_eq!(get(&cls, &obj, "units"), "ft"); // reset by the nconds side effect
        assert_eq!(get(&cls, &obj, "x"), "[ 0 0]");
    }

    #[test]
    fn nconds_grow_preserves_leading_and_zero_fills_tail() {
        // Growing `nconds` re-runs the realloc side effect: the leading entries
        // are preserved and the grown tail reads as zero. Pascal's `ReAllocmem`
        // leaves that tail uninitialized (nondeterministic heap), so this is a
        // Rust-only invariant — not oracle-pinnable — locking the zero-fill
        // choice documented on `realloc_conductors`. The grow also resets units.
        let enums = EnumRegistry::new();
        let cls = class_props(&enums);
        let mut obj = LineSpacingObj::new("ls");
        let errs = apply(
            &cls,
            &mut obj,
            &[
                ("nconds", "3"),
                ("x", "1 2 3"),
                ("h", "10 11 12"),
                ("units", "m"),
            ],
        );
        assert!(errs.is_empty(), "{errs:?}");
        let errs = apply(&cls, &mut obj, &[("nconds", "5")]);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(get(&cls, &obj, "nconds"), "5");
        assert_eq!(get(&cls, &obj, "x"), "[ 1 2 3 0 0]"); // leading kept, tail zeroed
        assert_eq!(get(&cls, &obj, "h"), "[ 10 11 12 0 0]");
        assert_eq!(get(&cls, &obj, "units"), "ft"); // realloc side effect resets units
    }

    #[test]
    fn nconds_zero_reads_empty_string() {
        // `nconds=0` frees the coordinate buffers (Pascal `ReAllocmem(FX, 0)`
        // nils the pointer), so X/H read as the empty string `''`, not `'[]'`.
        // Oracle-confirmed and pinned by the `linespacing_zero_nconds` golden;
        // kept here as a unit-level guard on the empty-array → nil mapping.
        let enums = EnumRegistry::new();
        let cls = class_props(&enums);
        let mut obj = LineSpacingObj::new("ls");
        let errs = apply(&cls, &mut obj, &[("nconds", "0")]);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(get(&cls, &obj, "nconds"), "0");
        assert_eq!(get(&cls, &obj, "x"), "");
        assert_eq!(get(&cls, &obj, "h"), "");
    }

    #[test]
    fn nconds_negative_clamps_to_empty() {
        // Pascal's `ReAllocmem(FX, FNConds)` *raises an exception* for a negative
        // `nconds` (the oracle reports DSSException 303), so the degenerate path
        // cannot be oracle-pinned. We clamp the allocation size to zero rather
        // than panic: NConds reads back the raw negative value, the coordinate
        // buffers are empty, and X/H therefore read `''`. Rust-only invariant
        // locking that graceful clamp.
        let enums = EnumRegistry::new();
        let cls = class_props(&enums);
        let mut obj = LineSpacingObj::new("ls");
        let errs = apply(&cls, &mut obj, &[("nconds", "-1")]);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(get(&cls, &obj, "nconds"), "-1");
        assert_eq!(get(&cls, &obj, "x"), "");
        assert_eq!(get(&cls, &obj, "h"), "");
    }

    #[test]
    fn x_h_arrays_sized_by_nconds() {
        // X/H are DoubleVArray sized by FNConds: extra tokens are dropped and
        // missing tokens zero-fill (Pascal `InterpretDblArray`).
        let enums = EnumRegistry::new();
        let cls = class_props(&enums);
        let mut obj = LineSpacingObj::new("ls");
        let errs = apply(
            &cls,
            &mut obj,
            &[
                ("nconds", "3"),
                ("nphases", "3"),
                ("x", "-1.2 0 1.2 9.9"), // 4 tokens, only 3 kept
                ("h", "28"),             // 1 token, rest zero-filled
                ("units", "ft"),
            ],
        );
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(get(&cls, &obj, "x"), "[ -1.2 0 1.2]");
        assert_eq!(get(&cls, &obj, "h"), "[ 28 0 0]");
    }

    #[test]
    fn make_like_copies_geometry() {
        let enums = EnumRegistry::new();
        let cls = class_props(&enums);
        let mut src = LineSpacingObj::new("s1");
        apply(
            &cls,
            &mut src,
            &[
                ("nconds", "4"),
                ("nphases", "3"),
                ("x", "-1.2 0 1.2 0"),
                ("h", "28 28 28 24"),
                ("units", "m"),
            ],
        );
        let mut dst = LineSpacingObj::new("s2");
        dst.make_like(&src);
        assert_eq!(get(&cls, &dst, "nconds"), "4");
        assert_eq!(get(&cls, &dst, "nphases"), "3");
        assert_eq!(get(&cls, &dst, "units"), "m"); // Other.Units overrides the ft reset
        assert_eq!(get(&cls, &dst, "x"), "[ -1.2 0 1.2 0]");
        assert_eq!(get(&cls, &dst, "h"), "[ 28 28 28 24]");
    }
}
