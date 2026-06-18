//! `TCC_Curve` — time-current (and volt-time) curves. Port of Pascal
//! `General/TCC_Curve.pas`. This is the simplest `DSS_OBJECT` class (three
//! properties, no enums, no file I/O) and serves as the first guinea pig for
//! the [property engine](crate::obj::props).
//!
//! Pascal `TProp`: `NPts=1`, `C_Array=2`, `T_Array=3`; the base class appends
//! `Like=4`. The point arrays are sized by `NPts` (`PropertyOffset2`).

#[cfg(test)]
mod tests;

use crate::obj::base::{DssObjData, DssObject};
use crate::obj::props::{PropDef, PropFlags, define_properties};

// Pascal `TTCC_CurveProp` ordinals + the property table. Abbreviation matching
// stays on (only `GrowthShape` disables it). `NPts` is SuppressJSON in the
// original (inert in the text-dump path); the point arrays are sized by it.
define_properties! {
    class "TCC_Curve", abbrev true;
    1 NPTS    => PropDef::integer("NPts").flags(PropFlags::SUPPRESS_JSON);
    2 C_ARRAY => PropDef::double_array("C_Array", NPTS);
    3 T_ARRAY => PropDef::double_array("T_Array", NPTS);
}

use prop::{C_ARRAY, NPTS, T_ARRAY};

/// A `TCC_Curve` instance (`TTCC_CurveObj`).
#[derive(Debug, Clone)]
pub struct TccCurveObj {
    data: DssObjData,
    /// Number of points (`Npts`).
    npts: i32,
    /// Multiplier (current) values; `None` mirrors a NIL Pascal pointer.
    c_values: Option<Vec<f64>>,
    /// Time values (seconds).
    t_values: Option<Vec<f64>>,
    /// Natural logs of the C/T values, kept in step with the arrays
    /// (`CalcLogPoints`). Not dumped; used by the protection elements later.
    log_c: Option<Vec<f64>>,
    log_t: Option<Vec<f64>>,
}

impl TccCurveObj {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            data: DssObjData::new(name.into().to_lowercase(), prop::NUM_PROPS),
            npts: 0,
            c_values: None,
            t_values: None,
            log_c: None,
            log_t: None,
        }
    }

    pub fn npts(&self) -> i32 {
        self.npts
    }
}

/// Pascal `ReAllocmem(arr, Sizeof(Double) * Npts)`: grow/shrink keeping the
/// surviving values; `Npts = 0` frees the array (NIL). New slots are zeroed
/// here — the original leaves them uninitialized, but that memory is always
/// overwritten before it is observed, so this is a determinism choice, not a
/// reproduced quirk.
fn realloc(arr: &mut Option<Vec<f64>>, n: usize) {
    if n == 0 {
        *arr = None;
    } else {
        arr.get_or_insert_with(Vec::new).resize(n, 0.0);
    }
}

/// Pascal `CalcLogPoints`: `ln(x)`, or `ln(0.001)` for non-positive `x`.
fn calc_log_points(x: Option<&[f64]>, n: usize) -> Option<Vec<f64>> {
    let x = x?;
    Some(
        (0..n)
            .map(|i| {
                let xi = x.get(i).copied().unwrap_or(0.0);
                if xi > 0.0 { xi.ln() } else { 0.001_f64.ln() }
            })
            .collect(),
    )
}

impl DssObject for TccCurveObj {
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
            NPTS => self.npts,
            _ => unreachable!("TCC_Curve has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        match idx {
            NPTS => self.npts = value,
            _ => unreachable!("TCC_Curve has no integer property {idx}"),
        }
    }

    fn get_f64_array(&self, idx: usize) -> Option<&[f64]> {
        match idx {
            C_ARRAY => self.c_values.as_deref(),
            T_ARRAY => self.t_values.as_deref(),
            _ => unreachable!("TCC_Curve has no array property {idx}"),
        }
    }
    fn set_f64_array(&mut self, idx: usize, value: Vec<f64>) {
        match idx {
            C_ARRAY => self.c_values = Some(value),
            T_ARRAY => self.t_values = Some(value),
            _ => unreachable!("TCC_Curve has no array property {idx}"),
        }
    }

    fn side_effects(&mut self, idx: usize, _prev_int: i32) {
        match idx {
            NPTS => {
                let n = self.npts.max(0) as usize;
                realloc(&mut self.c_values, n);
                realloc(&mut self.log_c, n);
                realloc(&mut self.t_values, n);
                realloc(&mut self.log_t, n);
            }
            C_ARRAY => {
                self.log_c = calc_log_points(self.c_values.as_deref(), self.npts.max(0) as usize);
            }
            T_ARRAY => {
                self.log_t = calc_log_points(self.t_values.as_deref(), self.npts.max(0) as usize);
            }
            _ => {}
        }
    }

    fn make_like(&mut self, other: &dyn DssObject) {
        // Pascal `TTCC_CurveObj.MakeLike`: `inherited MakeLike` (copy the
        // PrpSequence), then copy Npts and the point arrays and rebuild the
        // log tables. Read through the typed accessors so we don't need a
        // concrete downcast.
        self.data.copy_prp_sequence_from(other.data());
        self.npts = other.get_i32(NPTS);
        let n = self.npts.max(0) as usize;
        self.c_values = other.get_f64_array(C_ARRAY).map(<[f64]>::to_vec);
        self.t_values = other.get_f64_array(T_ARRAY).map(<[f64]>::to_vec);
        self.log_c = calc_log_points(self.c_values.as_deref(), n);
        self.log_t = calc_log_points(self.t_values.as_deref(), n);
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}
