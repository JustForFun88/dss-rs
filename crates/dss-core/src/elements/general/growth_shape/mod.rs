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

#[cfg(test)]
mod tests;

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
