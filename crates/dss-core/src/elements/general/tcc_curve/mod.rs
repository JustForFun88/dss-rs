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
    /// `LastValueAccessed` — the 1-based sequential-access hunt cache for
    /// `GetTCCTime`. A pure optimization for a monotonically-increasing curve
    /// (the only kind in practice), so the per-owner copy a protection element
    /// holds yields identical results to Pascal's per-curve field.
    last_value_accessed: usize,
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
            last_value_accessed: 1,
        }
    }

    pub fn npts(&self) -> i32 {
        self.npts
    }

    /// Pascal `TTCC_CurveObj.GetTCCtime`: the operating time for a multiple-of-
    /// pickup current value, by **log-log interpolation** between the bracketing
    /// points. Returns `-1.0` ("no operation") when `c_value` is below the first
    /// point. Ported loop-for-loop, including the `LastValueAccessed` hunt cache
    /// (starts the scan from the previous bracket, restarting at point 1 when the
    /// current dropped below it).
    pub fn get_tcc_time(&mut self, c_value: f64) -> f64 {
        let (Some(c), Some(t), Some(lc), Some(lt)) = (
            self.c_values.as_deref(),
            self.t_values.as_deref(),
            self.log_c.as_deref(),
            self.log_t.as_deref(),
        ) else {
            return -1.0;
        };
        let n = self.npts.max(0) as usize;
        if n == 0 {
            return -1.0;
        }

        // If current is less than the first point, no operation.
        if c_value < c[0] {
            return -1.0;
        }
        if n == 1 {
            return t[0];
        }

        // Start from the previously-accessed bracket (1-based, like Pascal).
        if c[self.last_value_accessed - 1] > c_value {
            self.last_value_accessed = 1; // start over from the beginning
        }
        for i in (self.last_value_accessed + 1)..=n {
            // 1-based `i` indexes arrays at `i - 1`.
            if c[i - 1] == c_value {
                self.last_value_accessed = i; // direct hit
                return t[i - 1];
            }
            if c[i - 1] > c_value {
                // Log-log interpolation between point `i-1` and `i`.
                self.last_value_accessed = i - 1;
                let log_test = if c_value > 0.0 {
                    c_value.ln()
                } else {
                    0.001_f64.ln()
                };
                let lo = self.last_value_accessed - 1; // 0-based of `i-1`
                let hi = i - 1; // 0-based of `i`
                return (lt[lo] + (log_test - lc[lo]) / (lc[hi] - lc[lo]) * (lt[hi] - lt[lo]))
                    .exp();
            }
        }

        // Fell through the loop: use the last value.
        self.last_value_accessed = n - 1;
        t[n - 1]
    }

    /// Pascal `TTCC_CurveObj.GetOVTime`: the over-voltage **definite-time**
    /// operating time for a per-unit voltage `v_value`. Returns `-1.0` ("no op")
    /// when `v_value` is at or below the first point. A plain forward scan to the
    /// first point `≥ v_value` (no log-log interpolation, unlike
    /// [`Self::get_tcc_time`]); mutates nothing, so the relay's clone shares the
    /// curve read-only. Used by the over-voltage Relay branch.
    pub fn get_ov_time(&self, v_value: f64) -> f64 {
        let (Some(c), Some(t)) = (self.c_values.as_deref(), self.t_values.as_deref()) else {
            return -1.0;
        };
        let n = self.npts.max(0) as usize;
        if n == 0 {
            return -1.0;
        }
        if v_value > c[0] {
            if n == 1 {
                return t[0];
            }
            // 1-based `i`: advance while `C[i] < v_value`, capped at Npts.
            let mut i = 1usize;
            while c[i - 1] < v_value {
                i += 1;
                if i > n {
                    break;
                }
            }
            // Pascal `T_Values[i - 1]` (1-based) = `t[i - 2]` (0-based); `i` is
            // always ≥ 2 here (the first compare advanced it), so no underflow.
            return t[i - 2];
        }
        -1.0
    }

    /// Pascal `TTCC_CurveObj.GetUVTime`: the under-voltage **definite-time**
    /// operating time for a per-unit voltage `v_value`. Returns `-1.0` ("no op")
    /// when `v_value` is at or above the last point. A plain backward scan to the
    /// first point `≤ v_value`; mutates nothing. Used by the under-voltage Relay
    /// branch.
    pub fn get_uv_time(&self, v_value: f64) -> f64 {
        let (Some(c), Some(t)) = (self.c_values.as_deref(), self.t_values.as_deref()) else {
            return -1.0;
        };
        let n = self.npts.max(0) as usize;
        if n == 0 {
            return -1.0;
        }
        if v_value < c[n - 1] {
            if n == 1 {
                return t[0];
            }
            // 1-based `i`: retreat while `C[i] > v_value`, floored at 0.
            let mut i = n;
            while c[i - 1] > v_value {
                i -= 1;
                if i == 0 {
                    break;
                }
            }
            // Pascal `T_Values[i + 1]` (1-based) = `t[i]` (0-based); `i ≤ n-1`
            // after the first retreat (and `t[0]` when it floored to 0).
            return t[i];
        }
        -1.0
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
