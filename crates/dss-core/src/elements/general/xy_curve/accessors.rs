//! The `DssObject` trait impl for `XyCurveObj`: typed property accessors,
//! `Points` get/set, `PropertySideEffects` and `MakeLike`. Split out of
//! `xy_curve/mod.rs` (no behavioral change).

use crate::obj::base::{DssObjData, DssObject};

use super::prop::{NPTS, X, XARRAY, XSCALE, XSHIFT, Y, YARRAY, YSCALE, YSHIFT};
use super::{XyCurveObj, prop};

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
