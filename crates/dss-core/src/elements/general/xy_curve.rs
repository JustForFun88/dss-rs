//! `XYcurve` — general (x, y) lookup curve with linear interpolation.
//! Port of Pascal `General/XYcurve.pas` (`TXYcurveObj`). A `DSS_OBJECT` class
//! (no terminals, no YPrim): it stores parallel `XValues`/`YValues` arrays and
//! serves interpolated/extrapolated `Y(x)` (and `X(y)`) lookups. Consumed by
//! Reactor `RCurve`/`LCurve`, Load `CVRcurve`, and several later-phase classes.
//!
//! The file-input props (`CSVFile`/`SngFile`/`DblFile`) are `NOT_PORTED` (Phase
//! 5 corpus never uses them; PHASE5_PLAN §WP5.1). The `Points` *setter* triggers
//! an access violation in the pinned oracle (a dss_capi `DoubleDArrayProperty`
//! bug), so the goldens drive the arrays through `XArray`/`YArray` and validate
//! the `Points` *getter* by reading it back; the setter is ported faithfully and
//! covered by a Rust-only unit test.

use crate::obj::base::{DssObjData, DssObject};
use crate::obj::props::{PropDef, PropFlags, define_properties};

// Pascal `TXYcurveProp` ordinals + the property table.
define_properties! {
    class "XYcurve", abbrev true;
    1  NPTS    => PropDef::integer("NPts").flags(PropFlags::SUPPRESS_JSON);
    2  POINTS  => PropDef::double_points("Points")
        .flags(PropFlags::REDUNDANT | PropFlags::REQUIRED_IN_SPEC_SET);
    3  YARRAY  => PropDef::double_array("YArray", NPTS)
        .flags(PropFlags::REQUIRED_IN_SPEC_SET);
    4  XARRAY  => PropDef::double_array("XArray", NPTS)
        .flags(PropFlags::REQUIRED_IN_SPEC_SET);
    5  CSVFILE => PropDef::string("CSVFile").flags(
        PropFlags::NOT_PORTED | PropFlags::IS_FILENAME
            | PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::GLOBAL_COUNT,
    );
    6  SNGFILE => PropDef::string("SngFile").flags(
        PropFlags::NOT_PORTED | PropFlags::IS_FILENAME
            | PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::GLOBAL_COUNT,
    );
    7  DBLFILE => PropDef::string("DblFile").flags(
        PropFlags::NOT_PORTED | PropFlags::IS_FILENAME
            | PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::GLOBAL_COUNT,
    );
    8  X       => PropDef::double("X").flags(PropFlags::SUPPRESS_JSON);
    9  Y       => PropDef::double("Y").flags(PropFlags::SUPPRESS_JSON);
    10 XSHIFT  => PropDef::double("XShift");
    11 YSHIFT  => PropDef::double("YShift");
    12 XSCALE  => PropDef::double("XScale");
    13 YSCALE  => PropDef::double("YScale");
}

use prop::{NPTS, X, XARRAY, XSCALE, XSHIFT, Y, YARRAY, YSCALE, YSHIFT};

/// A `XYcurve` instance (`TXYcurveObj`).
#[derive(Debug, Clone)]
pub struct XyCurveObj {
    data: DssObjData,
    /// Number of points in the curve (Pascal `FNumPoints`).
    npts: i32,
    /// X values (Pascal `XValues`, 1-based there, 0-based here).
    x_values: Vec<f64>,
    /// Y values (Pascal `YValues`).
    y_values: Vec<f64>,
    /// Interpolation hunt cache (Pascal `LastValueAccessed`, 1-based → 0-based).
    last_value_accessed: usize,
    /// Pascal `FX`/`FY`: the "current point" backing the `X`/`Y` scalar
    /// accessors (stored pre-shift/scale).
    fx: f64,
    fy: f64,
    fx_shift: f64,
    fy_shift: f64,
    fx_scale: f64,
    fy_scale: f64,
    csvfile: String,
    sngfile: String,
    dblfile: String,
}

impl XyCurveObj {
    /// Pascal `TXYcurveObj.Create`.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            data: DssObjData::new(name.into().to_lowercase(), prop::NUM_PROPS),
            npts: 0,
            x_values: Vec::new(),
            y_values: Vec::new(),
            last_value_accessed: 0, // Pascal `LastValueAccessed := 1` (1-based)
            fx: 0.0,
            fy: 0.0,
            fx_shift: 0.0,
            fy_shift: 0.0,
            fx_scale: 1.0,
            fy_scale: 1.0,
            csvfile: String::new(),
            sngfile: String::new(),
            dblfile: String::new(),
        }
    }

    fn n(&self) -> usize {
        self.npts.max(0) as usize
    }

    /// Pascal `TXYcurveObj.InterpolatePoints`: linear interpolation between the
    /// `i`-th and `j`-th points (0-based). When the abscissae coincide the
    /// abscissa-array value is undefined, so the `i`-th ordinate is returned.
    fn interpolate(x_arr: &[f64], y_arr: &[f64], i: usize, j: usize, x: f64) -> f64 {
        let den = x_arr[i] - x_arr[j];
        if den != 0.0 {
            y_arr[j] + (x - x_arr[j]) / den * (y_arr[i] - y_arr[j])
        } else {
            y_arr[i]
        }
    }

    /// Pascal `TXYcurveObj.GetYValue`: interpolated `Y` for the given `X`, with
    /// end-extrapolation. Mutates the `LastValueAccessed` hunt cache. Returns
    /// `0.0` for an empty curve.
    pub fn get_y_value(&mut self, x: f64) -> f64 {
        let n = self.n();
        if n == 0 {
            return 0.0;
        }
        if n == 1 {
            return self.y_values[0];
        }
        let (xv, yv) = (&self.x_values, &self.y_values);
        // Restart the hunt if we are now to the left of the cached point.
        if xv[self.last_value_accessed] > x {
            self.last_value_accessed = 0;
        }
        // Off the left end: extrapolate from the first two points.
        if self.last_value_accessed == 0 && xv[0] > x {
            return Self::interpolate(xv, yv, 0, 1, x);
        }
        // In the middle of the arrays.
        for i in (self.last_value_accessed + 1)..n {
            if (xv[i] - x).abs() < 0.00001 {
                self.last_value_accessed = i;
                return yv[i];
            } else if xv[i] > x {
                self.last_value_accessed = i - 1;
                return Self::interpolate(xv, yv, i, i - 1, x);
            }
        }
        // Fell through: extrapolate from the last two points.
        self.last_value_accessed = n - 2;
        Self::interpolate(xv, yv, n - 1, n - 2, x)
    }

    /// Pascal `TXYcurveObj.GetXValue`: interpolated `X` for the given `Y`, with
    /// end-extrapolation. Does not touch `LastValueAccessed` (matching Pascal).
    pub fn get_x_value(&self, y: f64) -> f64 {
        let n = self.n();
        if n == 0 {
            return 0.0;
        }
        if n == 1 {
            return self.x_values[0];
        }
        let (xv, yv) = (&self.x_values, &self.y_values);
        for i in 1..n {
            let (lo, hi) = (yv[i - 1], yv[i]);
            if (y >= lo && y <= hi) || (y <= lo && y >= hi) {
                // Interpolate X as a function of Y (axes swapped).
                return Self::interpolate(yv, xv, i - 1, i, y);
            }
        }
        // Y out of range: pick the end to extrapolate from.
        if yv[0] <= yv[n - 1] {
            if y <= yv[0] {
                Self::interpolate(yv, xv, 0, 1, y)
            } else {
                Self::interpolate(yv, xv, n - 2, n - 1, y)
            }
        } else if y >= yv[0] {
            Self::interpolate(yv, xv, 0, 1, y)
        } else {
            Self::interpolate(yv, xv, n - 2, n - 1, y)
        }
    }

    /// Pascal `TXYcurveObj.Set_X`: store the pre-shift/scale abscissa and keep
    /// `FY` in sync.
    fn set_x(&mut self, value: f64) {
        self.fx = (value - self.fx_shift) / self.fx_scale;
        self.fy = self.get_y_value(self.fx);
    }

    /// Pascal `TXYcurveObj.Set_Y`.
    fn set_y(&mut self, value: f64) {
        self.fy = (value - self.fy_shift) / self.fy_scale;
        self.fx = self.get_x_value(self.fy);
    }

    /// Pascal `Get_X`: `FX·FXscale + FXshift`.
    fn get_x(&self) -> f64 {
        self.fx * self.fx_scale + self.fx_shift
    }

    /// Pascal `Get_Y`: `FY·FYscale + FYshift`.
    fn get_y(&self) -> f64 {
        self.fy * self.fy_scale + self.fy_shift
    }

    /// Pascal `ReAllocmem` of `XValues`/`YValues` to `npts` (`Npts` side effect).
    fn realloc(&mut self) {
        let n = self.n();
        self.x_values.resize(n, 0.0);
        self.y_values.resize(n, 0.0);
    }
}

impl DssObject for XyCurveObj {
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
            _ => unreachable!("XYcurve has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        match idx {
            NPTS => self.npts = value,
            _ => unreachable!("XYcurve has no integer property {idx}"),
        }
    }

    fn get_f64(&self, idx: usize) -> f64 {
        match idx {
            X => self.get_x(),
            Y => self.get_y(),
            XSHIFT => self.fx_shift,
            YSHIFT => self.fy_shift,
            XSCALE => self.fx_scale,
            YSCALE => self.fy_scale,
            _ => unreachable!("XYcurve has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        match idx {
            X => self.set_x(value),
            Y => self.set_y(value),
            XSHIFT => self.fx_shift = value,
            YSHIFT => self.fy_shift = value,
            XSCALE => self.fx_scale = value,
            YSCALE => self.fy_scale = value,
            _ => unreachable!("XYcurve has no double property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        match idx {
            prop::CSVFILE => self.csvfile.clone(),
            prop::SNGFILE => self.sngfile.clone(),
            prop::DBLFILE => self.dblfile.clone(),
            _ => unreachable!("XYcurve has no string property {idx}"),
        }
    }
    fn set_string(&mut self, idx: usize, value: String) {
        match idx {
            prop::CSVFILE => self.csvfile = value,
            prop::SNGFILE => self.sngfile = value,
            prop::DBLFILE => self.dblfile = value,
            _ => unreachable!("XYcurve has no string property {idx}"),
        }
    }

    fn get_f64_array(&self, idx: usize) -> Option<&[f64]> {
        let arr = match idx {
            XARRAY => &self.x_values,
            YARRAY => &self.y_values,
            _ => unreachable!("XYcurve has no array property {idx}"),
        };
        (!arr.is_empty()).then_some(arr.as_slice())
    }
    fn set_f64_array(&mut self, idx: usize, value: Vec<f64>) {
        match idx {
            XARRAY => self.x_values = value,
            YARRAY => self.y_values = value,
            _ => unreachable!("XYcurve has no array property {idx}"),
        }
    }

    /// Pascal `GetPoints`: interleave the X/Y arrays. When either array is NIL
    /// (never allocated) the Pascal fallback returns a single `(0, 0)` point.
    fn get_points(&self) -> Vec<f64> {
        if self.x_values.is_empty() || self.y_values.is_empty() {
            return vec![0.0, 0.0];
        }
        let n = self.n();
        let mut out = Vec::with_capacity(2 * n);
        for i in 0..n {
            out.push(self.x_values[i]);
            out.push(self.y_values[i]);
        }
        out
    }
    /// Pascal `SetPoints`: split interleaved `(x, y)` pairs into the arrays
    /// (resetting `FNumPoints`), then re-sync the `X`/`Y` accessors.
    fn set_points(&mut self, value: Vec<f64>) {
        let n = value.len() / 2;
        self.npts = n as i32;
        self.x_values = vec![0.0; n];
        self.y_values = vec![0.0; n];
        for i in 0..n {
            self.x_values[i] = value[2 * i];
            self.y_values[i] = value[2 * i + 1];
        }
        if n > 0 {
            let (x0, y0) = (self.x_values[0], self.y_values[0]);
            self.set_x(x0); // Pascal `obj.X := obj.Xvalues[1]`
            self.set_y(y0); // Pascal `obj.Y := obj.Yvalues[1]`
        }
    }

    /// Pascal `TXYcurveObj.PropertySideEffects`.
    fn side_effects(&mut self, idx: usize, _prev_int: i32) {
        match idx {
            NPTS => self.realloc(),
            // Pascal `Y := YValues[1]` / `X := XValues[1]` (the property
            // setters, so the FX/FY accessors track the first point).
            YARRAY if !self.y_values.is_empty() => {
                let v = self.y_values[0];
                self.set_y(v);
            }
            XARRAY if !self.x_values.is_empty() => {
                let v = self.x_values[0];
                self.set_x(v);
            }
            _ => {}
        }
        // Pascal `case Idx of 2..7: LastValueAccessed := 1;`
        if (prop::POINTS..=prop::DBLFILE).contains(&idx) {
            self.last_value_accessed = 0;
        }
    }

    /// Pascal `TXYcurveObj.MakeLike`.
    fn make_like(&mut self, other: &dyn DssObject) {
        self.data.copy_prp_sequence_from(other.data());
        let Some(o) = other.as_any().downcast_ref::<XyCurveObj>() else {
            return;
        };
        self.npts = o.npts;
        self.x_values = o.x_values.clone();
        self.y_values = o.y_values.clone();
        self.fx_shift = o.fx_shift;
        self.fy_shift = o.fy_shift;
        self.fx_scale = o.fx_scale;
        self.fy_scale = o.fy_scale;
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

    fn edited(edits: &[(&str, &str)]) -> (ClassProps, XyCurveObj, Vec<String>) {
        let enums = EnumRegistry::new();
        let cls = class_props(&enums);
        let mut obj = XyCurveObj::new("c1");
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
            cls.edit_property(&mut obj, idx, value, &mut eng).unwrap();
        }
        obj.end_edit();
        errors.extend(obj.data_mut().take_errors());
        (cls, obj, errors)
    }

    fn get(cls: &ClassProps, obj: &XyCurveObj, name: &str) -> String {
        let enums = EnumRegistry::new();
        let idx = cls.property_index(name).unwrap();
        cls.get_value(obj, idx, &enums)
    }

    #[test]
    fn arrays_and_points_round_trip() {
        // Oracle (dss-python 0.15.7): points reads back interleaved.
        let (cls, obj, errs) = edited(&[
            ("npts", "4"),
            ("yarray", "10 20 30 40"),
            ("xarray", "1 2 3 4"),
        ]);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(get(&cls, &obj, "npts"), "4");
        assert_eq!(get(&cls, &obj, "xarray"), "[ 1 2 3 4]");
        assert_eq!(get(&cls, &obj, "yarray"), "[ 10 20 30 40]");
        assert_eq!(get(&cls, &obj, "points"), "[ 1 10 2 20 3 30 4 40]");
        // First point feeds the X/Y accessors.
        assert_eq!(get(&cls, &obj, "x"), "1");
        assert_eq!(get(&cls, &obj, "y"), "10");
    }

    #[test]
    fn get_y_value_interpolates_and_extrapolates() {
        // Values transcribed from the oracle (probe_xy3.py).
        let (_cls, mut obj, errs) = edited(&[
            ("npts", "4"),
            ("xarray", "1 2 3 4"),
            ("yarray", "10 20 30 40"),
        ]);
        assert!(errs.is_empty(), "{errs:?}");
        for (x, y) in [
            (0.5, 5.0),
            (1.0, 10.0),
            (1.5, 15.0),
            (2.5, 25.0),
            (4.0, 40.0),
            (5.0, 50.0),
        ] {
            assert!((obj.get_y_value(x) - y).abs() < 1e-9, "GetY({x})");
        }
    }

    #[test]
    fn x_setter_syncs_y_with_shift_and_scale() {
        // Oracle (probe_xy2.py): scales/shifts applied, then `x=2` → y=20.
        let (cls, mut obj, errs) = edited(&[
            ("npts", "3"),
            ("xarray", "0 1 2"),
            ("yarray", "0 10 20"),
            ("xscale", "2"),
            ("yscale", "3"),
            ("xshift", "1"),
            ("yshift", "5"),
        ]);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(get(&cls, &obj, "x"), "1");
        assert_eq!(get(&cls, &obj, "y"), "5");
        // edit x=2
        let enums = EnumRegistry::new();
        let mut parser = Parser::new();
        let vars = ParserVars::new();
        let mut errors = Vec::new();
        let idx = cls.property_index("x").unwrap();
        let mut eng = PropEngine {
            parser: &mut parser,
            vars: &vars,
            enums: &enums,
            errors: &mut errors,
            foreign: None,
        };
        cls.edit_property(&mut obj, idx, "2", &mut eng).unwrap();
        assert_eq!(get(&cls, &obj, "x"), "2");
        assert_eq!(get(&cls, &obj, "y"), "20");
    }

    #[test]
    fn set_points_splits_pairs() {
        // The oracle crashes on `points=`, so this path is Rust-only.
        let (cls, obj, errs) = edited(&[("points", "1 10 2 20 3 30")]);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(get(&cls, &obj, "npts"), "3");
        assert_eq!(get(&cls, &obj, "xarray"), "[ 1 2 3]");
        assert_eq!(get(&cls, &obj, "yarray"), "[ 10 20 30]");
        assert_eq!(get(&cls, &obj, "points"), "[ 1 10 2 20 3 30]");
    }

    #[test]
    fn file_props_are_not_ported() {
        let enums = EnumRegistry::new();
        let cls = class_props(&enums);
        let mut obj = XyCurveObj::new("c1");
        let mut parser = Parser::new();
        let vars = ParserVars::new();
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
            .edit_property(&mut obj, idx, "curve.csv", &mut eng)
            .unwrap_err();
        assert!(
            err.to_string().to_lowercase().contains("not ported"),
            "{err}"
        );
    }

    #[test]
    fn make_like_copies_curve() {
        let (cls, base, _) = edited(&[("npts", "2"), ("xarray", "0 1"), ("yarray", "5 7")]);
        let mut obj = XyCurveObj::new("derived");
        obj.make_like(&base);
        assert_eq!(get(&cls, &obj, "npts"), "2");
        assert_eq!(get(&cls, &obj, "xarray"), "[ 0 1]");
        assert_eq!(get(&cls, &obj, "yarray"), "[ 5 7]");
    }
}
