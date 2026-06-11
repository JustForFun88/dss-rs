//! `TCC_Curve` — time-current (and volt-time) curves. Port of Pascal
//! `General/TCC_Curve.pas`. This is the simplest `DSS_OBJECT` class (three
//! properties, no enums, no file I/O) and serves as the first guinea pig for
//! the [property engine](crate::obj::props).
//!
//! Pascal `TProp`: `NPts=1`, `C_Array=2`, `T_Array=3`; the base class appends
//! `Like=4`. The point arrays are sized by `NPts` (`PropertyOffset2`).

use crate::obj::base::{DssObjData, DssObject};
use crate::obj::props::{ClassProps, PropDef, PropFlags};

/// 1-based property ordinals (Pascal `TTCC_CurveProp`).
pub const NPTS: usize = 1;
pub const C_ARRAY: usize = 2;
pub const T_ARRAY: usize = 3;

/// Build the `TCC_Curve` property table. Abbreviation matching stays on (only
/// `GrowthShape` disables it).
pub fn class_props() -> ClassProps {
    ClassProps::new(
        "TCC_Curve",
        vec![
            // NPts: integer point count; SuppressJSON in the original (inert in
            // the text-dump path).
            PropDef::integer("NPts").flags(PropFlags::SUPPRESS_JSON),
            // C_Array / T_Array: point values, sized by NPts.
            PropDef::double_array("C_Array", NPTS),
            PropDef::double_array("T_Array", NPTS),
        ],
        true,
    )
}

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

/// Number of properties this class exposes (`NPts`, `C_Array`, `T_Array`,
/// `Like`).
const NUM_PROPS: usize = 4;

impl TccCurveObj {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            data: DssObjData::new(name.into().to_lowercase(), NUM_PROPS),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::obj::dss_enum::EnumRegistry;
    use crate::obj::props::PropEngine;
    use dss_parser::{Parser, ParserVars};

    /// Drive a sequence of `name=value` edits through the engine the way the
    /// executive's `Edit` loop will, then return the all-properties dump.
    fn edit_and_dump(edits: &[(&str, &str)]) -> Vec<(String, String)> {
        let cls = class_props();
        let mut obj = TccCurveObj::new("test");
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
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");

        (1..=cls.num_properties())
            .map(|i| {
                (
                    cls.property_name(i).to_string(),
                    cls.get_value(&obj, i, &enums),
                )
            })
            .collect()
    }

    #[test]
    fn defaults_match_oracle() {
        // Oracle: NPts='0', C_Array='', T_Array='', Like=''
        let dump = edit_and_dump(&[]);
        assert_eq!(
            dump,
            vec![
                ("NPts".into(), "0".into()),
                ("C_Array".into(), "".into()),
                ("T_Array".into(), "".into()),
                ("Like".into(), "".into()),
            ]
        );
    }

    #[test]
    fn full_spec_matches_oracle() {
        // Oracle: npts=3 c_array=(1 2 3) t_array=(0.1 0.2 0.3)
        //   => NPts='3', C_Array='[ 1 2 3]', T_Array='[ 0.1 0.2 0.3]'
        let dump = edit_and_dump(&[
            ("npts", "3"),
            ("C_array", "1 2 3"),
            ("T_array", "0.1 0.2 0.3"),
        ]);
        assert_eq!(dump[0], ("NPts".into(), "3".into()));
        assert_eq!(dump[1], ("C_Array".into(), "[ 1 2 3]".into()));
        assert_eq!(dump[2], ("T_Array".into(), "[ 0.1 0.2 0.3]".into()));
    }

    #[test]
    fn short_array_zero_fills() {
        // Oracle "npts then part c": npts=3, C_array=(5 6) => '[ 5 6 0]'
        let dump = edit_and_dump(&[("npts", "3"), ("C_array", "5 6")]);
        assert_eq!(dump[1], ("C_Array".into(), "[ 5 6 0]".into()));
    }

    #[test]
    fn shrinking_npts_truncates() {
        // Oracle "npts=2 after 3": keeps the first two points.
        let dump = edit_and_dump(&[
            ("npts", "3"),
            ("C_array", "1 2 3"),
            ("T_array", "7 8 9"),
            ("npts", "2"),
        ]);
        assert_eq!(dump[0], ("NPts".into(), "2".into()));
        assert_eq!(dump[1], ("C_Array".into(), "[ 1 2]".into()));
        assert_eq!(dump[2], ("T_Array".into(), "[ 7 8]".into()));
    }

    #[test]
    fn abbreviations_and_order_independence() {
        // Set arrays before npts, using abbreviations; the final dump must be
        // identical to the canonical order (the engine resizes on npts and the
        // arrays re-read against the count).
        let canonical = edit_and_dump(&[("npts", "2"), ("C_array", "10 20"), ("T_array", "1 2")]);
        let odd = edit_and_dump(&[("np", "2"), ("c", "10 20"), ("t", "1 2")]);
        assert_eq!(canonical, odd);
    }

    #[test]
    fn log_points_track_c_array() {
        // CalcLogPoints side effect: log_c[i] = ln(c[i]).
        edit_and_dump(&[("npts", "2"), ("C_array", "1 100")]);
        let cls = class_props();
        let mut obj = TccCurveObj::new("t");
        let mut parser = Parser::new();
        let vars = ParserVars::new();
        let enums = EnumRegistry::new();
        let mut errors = Vec::new();
        for (n, v) in [("npts", "2"), ("C_array", "1 100")] {
            let idx = cls.property_index(n).unwrap();
            let mut eng = PropEngine {
                parser: &mut parser,
                vars: &vars,
                enums: &enums,
                errors: &mut errors,
                foreign: None,
            };
            cls.edit_property(&mut obj, idx, v, &mut eng).unwrap();
        }
        let logs = obj.log_c.as_ref().unwrap();
        assert!((logs[0] - 0.0).abs() < 1e-12); // ln(1) = 0
        assert!((logs[1] - 100.0_f64.ln()).abs() < 1e-12);
    }
}
