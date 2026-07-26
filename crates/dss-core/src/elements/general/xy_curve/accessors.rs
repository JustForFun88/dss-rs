//! The `DssObject` trait impl for `XyCurveObj`: typed property accessors,
//! `Points` get/set, `PropertySideEffects` and `MakeLike`. Split out of
//! `xy_curve/mod.rs` (no behavioral change).

use crate::obj::base::{DssObjData, DssObject, FileLoad};

use super::prop::{
    CSVFILE, DBLFILE, NPTS, SNGFILE, X, XARRAY, XSCALE, XSHIFT, Y, YARRAY, YSCALE, YSHIFT,
};
use super::{XyCurveObj, prop};

impl XyCurveObj {
    /// Pascal `TXYcurveObj.MakeLike`.
    pub(crate) fn make_like(&mut self, other: &Self) {
        self.data.copy_prp_sequence_from(other.data());
        let o = other;
        self.npts = o.npts;
        self.x_values = o.x_values.clone();
        self.y_values = o.y_values.clone();
        self.fx_shift = o.fx_shift;
        self.fy_shift = o.fy_shift;
        self.fx_scale = o.fx_scale;
        self.fy_scale = o.fy_scale;
    }
}

impl DssObject for XyCurveObj {
    fn data(&self) -> &DssObjData {
        &self.data
    }
    fn data_mut(&mut self) -> &mut DssObjData {
        &mut self.data
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
            // Pascal `DoCSVFile`/`DoSngFile`/`DoDblFile` run here
            // (`XYcurve.pas:337-342`), but the hook can't reach the filesystem:
            // queue the read for the executive. The arrays' first-point sync
            // (`X:=XValues[1]; Y:=YValues[1]`) happens in `apply_*_file_load`
            // (via `sync_first_point`), once the file is resolved.
            CSVFILE => self
                .pending_file_loads
                .push(FileLoad::text(CSVFILE, self.csvfile.clone())),
            SNGFILE => self
                .pending_file_loads
                .push(FileLoad::binary(SNGFILE, self.sngfile.clone())),
            DBLFILE => self
                .pending_file_loads
                .push(FileLoad::binary(DBLFILE, self.dblfile.clone())),
            _ => {}
        }
        // Pascal `case Idx of 2..7: LastValueAccessed := 1;`
        if (prop::POINTS..=prop::DBLFILE).contains(&idx) {
            self.last_value_accessed = 0;
        }
    }

    fn take_file_loads(&mut self) -> Vec<FileLoad> {
        std::mem::take(&mut self.pending_file_loads)
    }

    /// Apply a resolved `CSVFile` (Pascal `DoCSVFile`), then sync the first
    /// point (Pascal `XYcurve.pas:357-361`).
    fn apply_file_load(
        &mut self,
        load: &FileLoad,
        content: &str,
        _errors: &mut crate::diag::ErrorLog,
    ) {
        if load.prop == CSVFILE {
            self.read_csv_file(content);
            self.sync_first_point();
        }
    }

    /// Apply a resolved `SngFile`/`DblFile` (Pascal `DoSngFile`/`DoDblFile`),
    /// then sync the first point (Pascal `XYcurve.pas:357-361`).
    fn apply_binary_file_load(
        &mut self,
        load: &FileLoad,
        content: &[u8],
        _errors: &mut crate::diag::ErrorLog,
    ) {
        match load.prop {
            SNGFILE => self.read_sng_file(content),
            DBLFILE => self.read_dbl_file(content),
            _ => return,
        }
        self.sync_first_point();
    }
}
