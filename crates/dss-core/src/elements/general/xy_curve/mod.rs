//! `XYcurve` — general (x, y) lookup curve with linear interpolation.
//! Port of Pascal `General/XYcurve.pas` (`TXYcurveObj`). A `DSS_OBJECT` class
//! (no terminals, no YPrim): it stores parallel `XValues`/`YValues` arrays and
//! serves interpolated/extrapolated `Y(x)` (and `X(y)`) lookups. Consumed by
//! Reactor `RCurve`/`LCurve`, Load `CVRcurve`, and several later-phase classes.
//!
//! The file-input props (`CSVFile`/`SngFile`/`DblFile`, `XYcurve.pas:337-342`)
//! load the curve from a CSV / little-endian-`f32` / little-endian-`f64` file
//! via the deferred `FileLoad` path (WPG.17). The `Points` *setter* triggers
//! an access violation in the pinned oracle (a dss_capi `DoubleDArrayProperty`
//! bug), so the goldens drive the arrays through `XArray`/`YArray` and validate
//! the `Points` *getter* by reading it back; the setter is ported faithfully and
//! covered by a Rust-only unit test.

#[cfg(test)]
mod tests;

mod accessors;
mod save;

use crate::obj::base::{DssObjData, FileLoad};
use crate::obj::props::{PropDef, PropFlags, define_properties};
use dss_parser::{Parser, ParserVars};

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
    5  CSVFILE => PropDef::string("CSVFile").size_prop(NPTS).flags(
        PropFlags::IS_FILENAME | PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::GLOBAL_COUNT,
    );
    6  SNGFILE => PropDef::string("SngFile").size_prop(NPTS).flags(
        PropFlags::IS_FILENAME | PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::GLOBAL_COUNT,
    );
    7  DBLFILE => PropDef::string("DblFile").size_prop(NPTS).flags(
        PropFlags::IS_FILENAME | PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::GLOBAL_COUNT,
    );
    8  X       => PropDef::double("X").flags(PropFlags::SUPPRESS_JSON);
    9  Y       => PropDef::double("Y").flags(PropFlags::SUPPRESS_JSON);
    10 XSHIFT  => PropDef::double("XShift");
    11 YSHIFT  => PropDef::double("YShift");
    12 XSCALE  => PropDef::double("XScale");
    13 YSCALE  => PropDef::double("YScale");
}

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
    /// Deferred file reads (`CSVFile`/`SngFile`/`DblFile`) queued for the
    /// executive — the property hook can't reach the filesystem (WPG.17,
    /// mirrors `spectrum`/`load_shape`).
    pending_file_loads: Vec<FileLoad>,
}

impl XyCurveObj {
    /// Pascal `TXYcurveObj.Create`.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            data: DssObjData::new(name.into().to_ascii_lowercase(), prop::NUM_PROPS),
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
            pending_file_loads: Vec::new(),
        }
    }

    fn n(&self) -> usize {
        self.npts.max(0) as usize
    }

    /// Test/helper constructor: a curve from explicit `(x, y)` arrays (the same
    /// state a `npts`/`xarray`/`yarray` parse would leave). Used by control unit
    /// tests that need a ready volt-var / volt-watt curve without the prop engine.
    #[cfg(test)]
    pub(crate) fn from_points(name: &str, xs: &[f64], ys: &[f64]) -> Self {
        assert_eq!(xs.len(), ys.len(), "from_points: x/y length mismatch");
        let mut c = Self::new(name);
        c.npts = xs.len() as i32;
        c.x_values = xs.to_vec();
        c.y_values = ys.to_vec();
        c
    }

    /// The curve's `Y` values (Pascal `YValues`, indexed `1..NumPoints`). Used by
    /// InvControl's `ValidateXYCurve` to range-check a control curve.
    pub fn y_values(&self) -> &[f64] {
        &self.y_values
    }

    /// The curve's `X` values (Pascal `XValues`, indexed `1..NumPoints`). Used by
    /// VCCS's z-domain filter (`Ffilter.Xvalue_pt[k]` denominator taps).
    pub fn x_values(&self) -> &[f64] {
        &self.x_values
    }

    /// The number of points in the curve (Pascal `FNumPoints` / `NumPoints`).
    pub fn num_points(&self) -> usize {
        self.n()
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

    /// Pascal `TXYcurveObj.GetCoefficients`: the `(a, b)` of the interpolated
    /// line `a·X + b` for the given `X`. `(0, 0)` for an empty/single-point
    /// curve; off either end the coefficients come from the nearest two points
    /// (the same line `GetYValue` extrapolates along). Unlike `get_y_value`,
    /// there is no exact-match shortcut (faithful to the Pascal). Mutates the
    /// `LastValueAccessed` hunt cache.
    pub fn get_coefficients(&mut self, x: f64) -> (f64, f64) {
        let n = self.n();
        if n <= 1 {
            return (0.0, 0.0);
        }
        let (xv, yv) = (&self.x_values, &self.y_values);
        // Restart the hunt if we are now to the left of the cached point.
        if xv[self.last_value_accessed] > x {
            self.last_value_accessed = 0;
        }
        // Off the left end: coefficients from the first two points.
        if self.last_value_accessed == 0 && xv[0] > x {
            let a = (yv[1] - yv[0]) / (xv[1] - xv[0]);
            let b = yv[1] - a * xv[1];
            return (a, b);
        }
        // In the middle of the arrays.
        for i in (self.last_value_accessed + 1)..n {
            if xv[i] > x {
                self.last_value_accessed = i - 1;
                let a = (yv[i] - yv[i - 1]) / (xv[i] - xv[i - 1]);
                let b = yv[i] - a * xv[i];
                return (a, b);
            }
        }
        // Fell through: coefficients from the last two points.
        let a = (yv[n - 1] - yv[n - 2]) / (xv[n - 1] - xv[n - 2]);
        let b = yv[n - 1] - a * xv[n - 1];
        (a, b)
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

    /// Pascal `DoCSVFile` (`Common/Utilities.pas:2258`) with `pA=XValues`,
    /// `pB=YValues`, `OnlyLoadB=False`: parse up to `NumPoints` rows of `x, y`
    /// (AuxParser — comma or whitespace separated), then set `NumPoints := i`
    /// **unconditionally** (`:2303`). The read loop reproduces Pascal's
    /// `while ((F.Position + 1) < F.Size) and (i < NumPoints)` guard (`:2277`):
    /// the pre-read guard requires ≥2 bytes to remain, so a final ≤1-byte line
    /// with no trailing newline (or a trailing blank line) is not read — the
    /// same subtlety `spectrum::read_csv_file` reproduces. Malformed-token
    /// divergence (audit-settled 2026-07-09): Pascal `MakeDouble`
    /// (`ParserDel.pas:718-747`) returns `0.0` only for an *empty* token; a
    /// non-empty non-numeric token **raises**, and `DoCSVFile`'s handler prints
    /// error 58614 and exits WITHOUT `NumPoints := i` (partially-filled
    /// realloc'd arrays — semi-garbage). The port deliberately follows the
    /// established `spectrum::read_csv_file` convention instead: substitute
    /// `0.0` and keep reading. Well-formed files are identical on both engines.
    fn read_csv_file(&mut self, content: &str) {
        let n = self.n();
        let mut xs = vec![0.0; n];
        let mut ys = vec![0.0; n];

        let mut parser = Parser::new();
        parser.set_auto_increment(false);
        let vars = ParserVars::new();

        let bytes = content.as_bytes();
        let size = bytes.len();
        let mut pos = 0usize;
        let mut i = 0usize;
        while pos + 1 < size && i < n {
            // `FSReadln`: read up to (and consume) the next LF; strip a trailing
            // CR for CRLF files, exactly like `str::lines`.
            let end = content[pos..].find('\n').map_or(size, |rel| pos + rel);
            let line_end = if end > pos && bytes[end - 1] == b'\r' {
                end - 1
            } else {
                end
            };
            let line = &content[pos..line_end];
            pos = if end < size { end + 1 } else { end };

            parser.set_cmd_string(line);
            parser.next_param(&vars);
            xs[i] = parser.make_double(&vars).unwrap_or(0.0);
            parser.next_param(&vars);
            ys[i] = parser.make_double(&vars).unwrap_or(0.0);
            i += 1;
        }

        xs.truncate(i);
        ys.truncate(i);
        self.x_values = xs;
        self.y_values = ys;
        self.npts = i as i32; // Pascal `NumPoints := i` (unconditional, :2303)
    }

    /// Pascal `DoSngFile` (`Common/Utilities.pas:2138`) with `pA=XValues`,
    /// `pB=YValues`, `OnlyLoadB=False`, `RoundA=False`: read little-endian
    /// `(x:f32, y:f32)` pairs, widening each to `f64` on store. Pascal shrinks
    /// `NumPoints` **only when short** (`if i <> NumPoints`, `:2188`); the
    /// `i < NumPoints` guard caps it at `npts` so the observable count is the
    /// number of full pairs read.
    ///
    /// The full-row gate (`off + 8 <= len`) matches the shipped LoadShape
    /// reader (`compute.rs:645`) rather than Pascal's literal `pos+1 < size`:
    /// for a malformed trailing 2..7-byte fragment Pascal `Inc(i)` fires before
    /// the partial `F.Read` fails, shrinking `NumPoints` to include a point
    /// whose value is uninitialized realloc'd heap — upstream UB, NOT
    /// reproduced (CLAUDE.md known-bugs policy). For all well-formed files the
    /// two gates are identical. `RoundA=False`, so X is never rounded (unlike
    /// GrowthShape, `XYcurve.pas:340`).
    fn read_sng_file(&mut self, content: &[u8]) {
        let npts = self.n();
        let mut xs = Vec::with_capacity(npts);
        let mut ys = Vec::with_capacity(npts);
        let mut off = 0usize;
        while xs.len() < npts && off + 8 <= content.len() {
            xs.push(f32::from_le_bytes(content[off..off + 4].try_into().unwrap()) as f64);
            ys.push(f32::from_le_bytes(content[off + 4..off + 8].try_into().unwrap()) as f64);
            off += 8;
        }
        if xs.len() != npts {
            self.npts = xs.len() as i32; // Pascal `if i <> NumPoints then NumPoints := i`
        }
        self.x_values = xs;
        self.y_values = ys;
    }

    /// Pascal `DoDblFile` (`Common/Utilities.pas:2202`): identical to
    /// [`Self::read_sng_file`] but 16-byte little-endian `(x:f64, y:f64)` rows.
    /// Same shrink-only-when-short (`:2244`), same full-row UB note.
    fn read_dbl_file(&mut self, content: &[u8]) {
        let npts = self.n();
        let mut xs = Vec::with_capacity(npts);
        let mut ys = Vec::with_capacity(npts);
        let mut off = 0usize;
        while xs.len() < npts && off + 16 <= content.len() {
            xs.push(f64::from_le_bytes(
                content[off..off + 8].try_into().unwrap(),
            ));
            ys.push(f64::from_le_bytes(
                content[off + 8..off + 16].try_into().unwrap(),
            ));
            off += 16;
        }
        if xs.len() != npts {
            self.npts = xs.len() as i32; // Pascal `if i <> NumPoints then NumPoints := i`
        }
        self.x_values = xs;
        self.y_values = ys;
    }

    /// Pascal `XYcurve.pas:357-369` (the `csvfile|sngfile|dblfile` arm of
    /// `PropertySideEffects`): after the reader populates the arrays, set
    /// `X := XValues[1]; Y := YValues[1];` (the shift/scale-honoring accessors)
    /// and `LastValueAccessed := 1`. Run here — from `apply_*_file_load` —
    /// rather than in `side_effects`, because the arrays aren't populated until
    /// the executive resolves the file.
    fn sync_first_point(&mut self) {
        if !self.x_values.is_empty() {
            let v = self.x_values[0];
            self.set_x(v);
        }
        if !self.y_values.is_empty() {
            let v = self.y_values[0];
            self.set_y(v);
        }
        self.last_value_accessed = 0;
    }

    /// Pascal `ReAllocmem` of `XValues`/`YValues` to `npts` (`Npts` side effect).
    fn realloc(&mut self) {
        let n = self.n();
        self.x_values.resize(n, 0.0);
        self.y_values.resize(n, 0.0);
    }
}
