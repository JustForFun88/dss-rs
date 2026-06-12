//! `GrowthShape` — yearly load-growth multiplier curve.
//! Port of Pascal `General/GrowthShape.pas` (`TGrowthShapeObj`). A `DSS_OBJECT`
//! class (no terminals, no YPrim): it stores `(year, mult)` pairs and expands
//! them into a per-year cumulative multiplier table consumed by the yearly
//! solution mode (Phase 5).
//!
//! Growth multipliers are entered relative to the previous year's load (a 2.5%
//! growth is `1.025`); only the years where the rate changes need to be listed.
//! The file-input props (`CSVFile`/`SngFile`/`DblFile`) are `NOT_PORTED` — the
//! gate feeders never use them (PHASE4_PLAN §5).

use crate::obj::base::{DssObjData, DssObject};
use crate::obj::props::{ClassProps, PropDef, PropFlags};

/// 1-based property ordinals (Pascal `TGrowthShapeProp`).
pub mod prop {
    pub const NPTS: usize = 1;
    pub const YEAR: usize = 2;
    pub const MULT: usize = 3;
    pub const CSVFILE: usize = 4;
    pub const SNGFILE: usize = 5;
    pub const DBLFILE: usize = 6;
    pub const NUM_PROPS: usize = 7; // incl. Like
}

/// `TGrowthShape.DefineProperties`.
pub fn class_props() -> ClassProps {
    use prop::*;
    let file_flags =
        PropFlags::NOT_PORTED | PropFlags::IS_FILENAME | PropFlags::REQUIRED_IN_SPEC_SET;
    let defs = vec![
        PropDef::integer("NPts").flags(PropFlags::SUPPRESS_JSON),
        // Years are stored as (rounded) doubles; `ApplyRound` matches the Pascal
        // `TPropertyFlag.ApplyRound` on the year vector.
        PropDef::double_array("Year", NPTS)
            .flags(PropFlags::APPLY_ROUND | PropFlags::REQUIRED_IN_SPEC_SET),
        PropDef::double_array("Mult", NPTS).flags(PropFlags::REQUIRED_IN_SPEC_SET),
        PropDef::string("CSVFile").flags(file_flags),
        PropDef::string("SngFile").flags(file_flags),
        PropDef::string("DblFile").flags(file_flags),
    ];
    debug_assert_eq!(defs.len(), NUM_PROPS - 1);
    ClassProps::new("GrowthShape", defs, true)
}

/// `TGrowthShapeObj`.
#[derive(Debug, Clone)]
pub struct GrowthShapeObj {
    data: DssObjData,
    /// Number of `(year, mult)` points (Pascal `Npts`).
    npts: i32,
    /// Years presently allocated in the look-up table (Pascal `NYears`).
    nyears: i32,
    /// Year values (Pascal `Year`, 1-based there, 0-based here).
    year: Option<Vec<f64>>,
    /// Per-point growth multipliers (Pascal `Multiplier`).
    multiplier: Option<Vec<f64>>,
    /// Cumulative multiplier from the base year (Pascal `YearMult`).
    year_mult: Vec<f64>,
    csvfile: String,
    sngfile: String,
    dblfile: String,
}

const DEFAULT_NYEARS: i32 = 30;

impl GrowthShapeObj {
    /// Pascal `TGrowthShapeObj.Create`.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            data: DssObjData::new(name.into().to_lowercase(), prop::NUM_PROPS),
            npts: 0,
            nyears: DEFAULT_NYEARS,
            year: None,
            multiplier: None,
            year_mult: vec![0.0; DEFAULT_NYEARS as usize],
            csvfile: String::new(),
            sngfile: String::new(),
            dblfile: String::new(),
        }
    }

    /// Pascal `TGrowthShapeObj.GetMult`: cumulative growth multiplier for `yr`
    /// relative to the base (first) year. Returns 1.0 for the base year or any
    /// earlier year, or when the curve is empty. Consumed by the yearly solution
    /// mode (Phase 5).
    pub fn get_mult(&mut self, yr: i32) -> f64 {
        if self.npts <= 0 {
            return 1.0;
        }
        // TODO(compat): FPC `Round` is ties-to-even; the base year is always in
        // Int64 range so `round_ties_even` reproduces it. Wiped with the other
        // compat shims (CLAUDE.md §TODO(compat)).
        let base = match self.year.as_ref().and_then(|y| y.first()) {
            Some(&y0) => y0.round_ties_even() as i32,
            None => return 1.0,
        };
        let index = yr - base;
        if index <= 0 {
            return 1.0; // base year or any previous year
        }
        if index > self.nyears {
            self.nyears = index + 10; // make some more space
            self.recalc_year_mult();
        }
        self.year_mult[(index - 1) as usize]
    }

    /// Pascal `TGrowthShapeObj.ReCalcYearMult`: fill `YearMult` with the running
    /// product of the per-point multipliers, year by year from the base year.
    fn recalc_year_mult(&mut self) {
        let nyears = self.nyears.max(0) as usize;
        if nyears == 0 {
            return;
        }
        let npts = self.npts.max(0) as usize;
        let (year, mult) = match (self.year.as_ref(), self.multiplier.as_ref()) {
            (Some(y), Some(m)) if npts > 0 && !m.is_empty() && !y.is_empty() => (y, m),
            _ => return,
        };

        let mut out = vec![0.0_f64; nyears];
        let mut cur = mult[0];
        let mut mult_inc = cur;
        out[0] = cur;
        let mut data_ptr = 0usize;
        // TODO(compat): FPC `Round` (see `get_mult`); years are integral.
        let mut cur_year = year[0].round_ties_even() as i32;
        for slot in out.iter_mut().skip(1) {
            cur_year += 1;
            if data_ptr + 1 < npts && year[data_ptr + 1] == cur_year as f64 {
                data_ptr += 1;
                mult_inc = mult[data_ptr];
            }
            cur *= mult_inc;
            *slot = cur;
        }
        self.year_mult = out;
    }
}

impl DssObject for GrowthShapeObj {
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
            prop::NPTS => self.npts,
            _ => unreachable!("GrowthShape has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        match idx {
            prop::NPTS => self.npts = value,
            _ => unreachable!("GrowthShape has no integer property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        match idx {
            prop::CSVFILE => self.csvfile.clone(),
            prop::SNGFILE => self.sngfile.clone(),
            prop::DBLFILE => self.dblfile.clone(),
            _ => unreachable!("GrowthShape has no string property {idx}"),
        }
    }
    fn set_string(&mut self, idx: usize, value: String) {
        match idx {
            prop::CSVFILE => self.csvfile = value,
            prop::SNGFILE => self.sngfile = value,
            prop::DBLFILE => self.dblfile = value,
            _ => unreachable!("GrowthShape has no string property {idx}"),
        }
    }

    fn get_f64_array(&self, idx: usize) -> Option<&[f64]> {
        match idx {
            prop::YEAR => self.year.as_deref(),
            prop::MULT => self.multiplier.as_deref(),
            _ => unreachable!("GrowthShape has no array property {idx}"),
        }
    }
    fn set_f64_array(&mut self, idx: usize, value: Vec<f64>) {
        match idx {
            prop::YEAR => self.year = Some(value),
            prop::MULT => self.multiplier = Some(value),
            _ => unreachable!("GrowthShape has no array property {idx}"),
        }
    }

    /// Pascal `TGrowthShapeObj.PropertySideEffects`. `Npts` reallocates the
    /// `Year`/`Multiplier` arrays; the file props are `NOT_PORTED` (the parse
    /// errors before reaching here).
    fn side_effects(&mut self, idx: usize, _prev_int: i32) {
        if idx == prop::NPTS {
            let n = self.npts.max(0) as usize;
            realloc(&mut self.year, n);
            realloc(&mut self.multiplier, n);
        }
    }

    /// Pascal `TGrowthShape.EndEdit` → `ReCalcYearMult`.
    fn end_edit(&mut self) {
        self.recalc_year_mult();
    }

    /// Pascal `TGrowthShapeObj.MakeLike`.
    fn make_like(&mut self, other: &dyn DssObject) {
        self.data.copy_prp_sequence_from(other.data());
        let Some(o) = other.as_any().downcast_ref::<GrowthShapeObj>() else {
            return;
        };
        self.npts = o.npts;
        self.multiplier = o.multiplier.clone();
        self.year = o.year.clone();
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}

/// Pascal `ReAllocmem`: grow/shrink keeping surviving values; 0 frees (NIL).
fn realloc(arr: &mut Option<Vec<f64>>, n: usize) {
    if n == 0 {
        *arr = None;
    } else {
        arr.get_or_insert_with(Vec::new).resize(n, 0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::obj::dss_enum::EnumRegistry;
    use crate::obj::props::PropEngine;
    use dss_parser::{Parser, ParserVars};

    fn edited(edits: &[(&str, &str)]) -> (ClassProps, GrowthShapeObj, Vec<String>) {
        let cls = class_props();
        let mut obj = GrowthShapeObj::new("gs");
        let mut parser = Parser::new();
        let vars = ParserVars::new();
        let enums = EnumRegistry::new();
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
            cls.edit_property(&mut obj, idx, value, &mut eng).unwrap();
        }
        obj.end_edit();
        errors.extend(obj.data_mut().take_errors());
        (cls, obj, errors)
    }

    fn get(cls: &ClassProps, obj: &GrowthShapeObj, name: &str) -> String {
        let enums = EnumRegistry::new();
        let idx = cls.property_index(name).unwrap();
        cls.get_value(obj, idx, &enums)
    }

    #[test]
    fn defaults_match_oracle() {
        // Oracle (dss-python 0.15.7) `new growthshape.x`: NPts=0, empty arrays.
        let (cls, obj, errs) = edited(&[]);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(get(&cls, &obj, "NPts"), "0");
        assert_eq!(get(&cls, &obj, "Year"), "");
        assert_eq!(get(&cls, &obj, "Mult"), "");
    }

    #[test]
    fn year_is_rounded_and_arrays_round_trip() {
        let (cls, obj, errs) = edited(&[
            ("npts", "5"),
            ("year", "1999 2000 2001 2005 2010"),
            ("mult", "1.10 1.07 1.05 1.025 1.01"),
        ]);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(get(&cls, &obj, "NPts"), "5");
        assert_eq!(get(&cls, &obj, "Year"), "[ 1999 2000 2001 2005 2010]");
        assert_eq!(get(&cls, &obj, "Mult"), "[ 1.1 1.07 1.05 1.025 1.01]");
    }

    #[test]
    fn file_props_are_not_ported() {
        // NOT_PORTED returns a hard parse error (not a deferred-error push).
        let cls = class_props();
        let mut obj = GrowthShapeObj::new("gs");
        let mut parser = Parser::new();
        let vars = ParserVars::new();
        let enums = EnumRegistry::new();
        let mut errors = Vec::new();
        let idx = cls.property_index("csvfile").unwrap();
        let mut eng = PropEngine {
            parser: &mut parser,
            vars: &vars,
            enums: &enums,
            errors: &mut errors,
            foreign: None,
        };
        let err = cls
            .edit_property(&mut obj, idx, "growth.csv", &mut eng)
            .unwrap_err();
        assert!(
            err.to_string().to_lowercase().contains("not ported"),
            "{err}"
        );
    }

    #[test]
    fn get_mult_compounds_from_base_year() {
        // The example from the Pascal header: fast start tapering to 1%.
        let (_cls, mut obj, errs) = edited(&[
            ("npts", "5"),
            ("year", "1999 2000 2001 2005 2010"),
            ("mult", "1.10 1.07 1.05 1.025 1.01"),
        ]);
        assert!(errs.is_empty(), "{errs:?}");
        // The base year (and any earlier year) returns 1.0 — the multipliers
        // apply to the *following* years (Pascal `if Index > 0`).
        assert_eq!(obj.get_mult(1998), 1.0);
        assert_eq!(obj.get_mult(1999), 1.0);
        // 2000 = first multiplier; 2001 compounds the second; 2002 the third.
        assert!((obj.get_mult(2000) - 1.10).abs() < 1e-12);
        assert!((obj.get_mult(2001) - 1.10 * 1.07).abs() < 1e-12);
        assert!((obj.get_mult(2002) - 1.10 * 1.07 * 1.05).abs() < 1e-12);
        // Between listed years the last rate persists: 2003 keeps ×1.05.
        let m2002 = obj.get_mult(2002);
        assert!((obj.get_mult(2003) - m2002 * 1.05).abs() < 1e-12);
    }

    #[test]
    fn get_mult_grows_table_past_initial_nyears() {
        // Default NYears=30; query well beyond it to exercise the regrow path.
        let (_cls, mut obj, errs) =
            edited(&[("npts", "2"), ("year", "2000 2001"), ("mult", "1.0 1.02")]);
        assert!(errs.is_empty(), "{errs:?}");
        // get_mult(2000+n) == YearMult[n-1]; YearMult[0]=1.0 (base mult) and
        // each later year compounds ×1.02, so get_mult(2050) == 1.02^49.
        let expected = 1.02_f64.powi(49);
        assert!((obj.get_mult(2050) - expected).abs() < 1e-9);
    }

    #[test]
    fn make_like_copies_curve() {
        let (cls, base, _) = edited(&[("npts", "2"), ("year", "2000 2010"), ("mult", "1.05 1.02")]);
        let mut obj = GrowthShapeObj::new("derived");
        obj.make_like(&base);
        assert_eq!(get(&cls, &obj, "NPts"), "2");
        assert_eq!(get(&cls, &obj, "Year"), "[ 2000 2010]");
        assert_eq!(get(&cls, &obj, "Mult"), "[ 1.05 1.02]");
    }
}
