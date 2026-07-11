//! Shared core for the scalar shape classes `TShape` (TempShape) and
//! `PriceShape` — ports of `General/TempShape.pas` and `General/PriceShape.pas`.
//!
//! Both are `DSS_OBJECT` classes (no terminals, no YPrim) that hold a single
//! scalar curve indexed by hour: TempShape's `Temp` (consumed by temperature-
//! dependent loads, Phase 7) and PriceShape's `Price` (feeds `ckt.PriceSignal`
//! in `SolveDaily`, wired in WP5.7). They are the simpler, *legacy* siblings of
//! LoadShape: one value array (no Q), no peak/normalize, no UseActual, no
//! interpolation mode, and an `Action` that only does the unported binary
//! saves. Crucially their `GetTemperature`/`GetPrice` lookups are the **legacy
//! 1-based** Pascal (init `LastValueAccessed := 1`, loop
//! `for i := LastValueAccessed + 1 to FNumPoints`, fall-through returns the
//! *last* point) — distinct from LoadShape's modernized 0-based `GetMultAtHour`
//! (which falls through to the *second-to-last* point). So this is ported from
//! TempShape/PriceShape directly, not derived from [`super::load_shape`].
//!
//! The data storage and the algorithms that are byte-identical between the two
//! classes — the hour lookup, the lazy mean/std-dev, and the `CSVFile`/
//! `SngFile`/`DblFile` readers (Pascal's shared `Common/Utilities.pas`
//! `DoCSVFile`/`DoSngFile`/`DoDblFile`, called with `OnlyLoadB = Interval <> 0`
//! — i.e. a fixed interval loads only the value column, `Interval = 0` loads
//! `(hour, value)` pairs from both) — live here on [`ScalarShapeCore`]; the
//! per-class property tables and their differing `PropertySideEffects` live in
//! [`super::temp_shape`] and [`super::price_shape`]. All three file props are
//! read via the deferred [`FileLoad`] path, exactly like LoadShape (WP5.2b /
//! WPG.1 for the binary pair).

use crate::obj::base::{DssObjData, FileLoad, ShapeSave};
use crate::support::mathutil::{curve_mean_and_std_dev, mean_and_std_dev};
use dss_parser::{Parser, ParserVars};

/// The state shared by `TShapeObj` and `PriceShapeObj` — the `TTShapeObj` /
/// `TPriceShapeObj` private/public fields that carry behavior.
#[derive(Debug, Clone)]
pub struct ScalarShapeCore {
    pub data: DssObjData,
    /// Pascal `FNumPoints`: number of points in the curve.
    pub num_points: i32,
    /// Fixed interval in hours; `0.0` means variable interval (use `hours`).
    pub interval: f64,
    /// The scalar values (Pascal `TValues`/`PriceValues`). `None` = NIL pointer.
    pub values: Option<Vec<f64>>,
    /// Hour values for a variable interval (Pascal `Hours`); `None` = even
    /// spacing.
    pub hours: Option<Vec<f64>>,
    /// Pascal `FMean`/`FStdDev`; lazily computed unless set explicitly.
    pub f_mean: f64,
    pub f_std_dev: f64,
    /// Pascal `FStdDevCalculated`: true when mean/std-dev were set explicitly
    /// (skips the on-demand recompute).
    pub std_dev_calculated: bool,
    /// Hunt cache for the variable-interval lookup (Pascal `LastValueAccessed`,
    /// kept as the original **1-based** index; ctor sets it to 1).
    pub last_value_accessed: usize,
    pub csvfile: String,
    pub sngfile: String,
    pub dblfile: String,
    /// Deferred file reads queued by `CSVFile` (drained by the executive).
    pub pending_file_loads: Vec<FileLoad>,
    /// Deferred binary saves queued by `Action=SngSave/DblSave`.
    pub pending_shape_saves: Vec<ShapeSave>,
}

impl ScalarShapeCore {
    /// Pascal `TTShapeObj.Create` / `TPriceShapeObj.Create`.
    pub fn new(name: impl Into<String>, num_props: usize) -> Self {
        Self {
            data: DssObjData::new(name.into().to_lowercase(), num_props),
            num_points: 0,
            interval: 1.0, // hr
            values: None,
            hours: None,
            f_mean: 0.0,
            f_std_dev: 0.0,
            std_dev_calculated: false,
            last_value_accessed: 1,
            csvfile: String::new(),
            sngfile: String::new(),
            dblfile: String::new(),
            pending_file_loads: Vec::new(),
            pending_shape_saves: Vec::new(),
        }
    }

    fn n(&self) -> usize {
        self.num_points.max(0) as usize
    }

    /// Queue a `SngSave`/`DblSave` binary write (Pascal `TTShapeObj`/
    /// `TPriceShapeObj.SaveToDblFile`/`SaveToSngFile`, `TempShape.pas:528/548`,
    /// `PriceShape.pas:547/568`). Single value series, bare `<name>` filename (no
    /// `_P`/`_Q` split), `GlobalResult` tag `result_tag` (`Temp`/`Price`). The
    /// caller passes `full_name` and `noun` for the not-defined guard
    /// (`if not Assigned(TValues/PriceValues)` → `DoSimpleMsg` 57622/57623 or
    /// 58622/58623).
    pub fn queue_shape_save(
        &mut self,
        sng: bool,
        result_tag: &'static str,
        full_name: &str,
        noun: &str,
        errors: &mut Vec<String>,
    ) {
        let n = self.n();
        let Some(v) = self.values.as_ref() else {
            errors.push(format!("{full_name} {noun} not defined."));
            return;
        };
        let values: Vec<f64> = v.iter().take(n).copied().collect();
        self.pending_shape_saves.push(ShapeSave {
            name: self.data.name().to_string(),
            sng,
            values,
            q_values: None,
            p_suffix: false,
            result_tag,
        });
    }

    /// Drain the queued binary saves for the executive.
    pub fn take_shape_saves(&mut self) -> Vec<ShapeSave> {
        std::mem::take(&mut self.pending_shape_saves)
    }

    /// Pascal `GetTemperature` / `GetPrice`: the scalar value nearest the
    /// requested hour. `0.0` for an empty curve; repeats the curve past its last
    /// point. Ported verbatim from the **legacy 1-based** Pascal — note the
    /// fall-through returns the last point (`TValues[FNumPoints]`), not the
    /// second-to-last as LoadShape's modernized lookup does.
    pub fn get_value_at_hour(&mut self, hr: f64) -> f64 {
        let npts = self.n();
        if npts == 0 {
            return 0.0; // default for an empty curve
        }
        let Some(t) = self.values.as_ref() else {
            return 0.0;
        };
        if t.is_empty() {
            return 0.0;
        }
        if npts == 1 {
            return t[0]; // TValues[1]
        }

        // --- Fixed (even) interval ---
        if self.interval > 0.0 {
            // TODO(compat): FPC `Round` is banker's rounding (ties-to-even);
            // these indices are always in i64 range, so `round_ties_even`
            // reproduces it. Wiped with the other compat shims.
            let mut index = (hr / self.interval).round_ties_even() as i64;
            let np = npts as i64;
            if index > np {
                index %= np; // wrap around using remainder
            }
            if index == 0 {
                index = np;
            }
            return t[(index - 1) as usize]; // TValues[Index]
        }

        // --- Variable interval (hour array) ---
        let h = match self.hours.as_ref() {
            Some(h) if h.len() >= npts => h.clone(),
            // No hour array despite Interval==0: degenerate; treat as empty.
            _ => return 0.0,
        };

        // Normalize Hr into the first cycle (wraparound). Pascal divides by
        // Hours[FNumPoints] unguarded, so a degenerate curve whose LAST hour is
        // 0 poisons Hr to NaN (masked-FPU Inf·0) — defined-garbage on a
        // pathological input, not reproduced (CLAUDE.md known-bug policy): the
        // port skips the wraparound and reads the curve as-is.
        let mut hr = hr;
        let last = h[npts - 1]; // Hours[FNumPoints]
        if hr > last && last != 0.0 {
            hr -= (hr / last).trunc() * last;
        }

        // LastValueAccessed is 1-based; Hours[LastValueAccessed] -> h[lva-1].
        let probe = self.last_value_accessed.min(npts);
        if h[probe - 1] > hr {
            self.last_value_accessed = 1; // start over from the beginning
        }

        // for i := LastValueAccessed + 1 to FNumPoints   (i is 1-based)
        for i in (self.last_value_accessed + 1)..=npts {
            if (h[i - 1] - hr).abs() < 0.00001 {
                // Close to an actual point — use it directly.
                self.last_value_accessed = i;
                return t[i - 1];
            }
            if h[i - 1] > hr {
                // Interpolate between LastValueAccessed (= i-1) and i.
                self.last_value_accessed = i - 1;
                let p = self.last_value_accessed; // 1-based anchor
                return t[p - 1] + (hr - h[p - 1]) / (h[i - 1] - h[p - 1]) * (t[i - 1] - t[p - 1]);
            }
        }

        // Fell through the loop: use the last value (legacy = TValues[FNumPoints]).
        self.last_value_accessed = npts - 1;
        t[npts - 1]
    }

    /// Pascal `TPriceShapeObj.Price(i)` (`PriceShape.pas:517`): the scalar
    /// value at 1-based index `i`, updating `LastValueAccessed` (already
    /// 1-based here, matching Pascal's own 1-based `PriceValues`/
    /// `LastValueAccessed` convention — no `dec(i)` in the source) — used by
    /// `SolveLD1`/`SolveLD2` to walk the price curve alongside the
    /// load-duration curve (`ckt.PriceCurveObj.Price(N)`).
    pub fn value_at(&mut self, i: i32) -> f64 {
        if i <= 0 || i > self.num_points {
            return 0.0;
        }
        let idx = (i - 1) as usize;
        let v = self
            .values
            .as_ref()
            .and_then(|t| t.get(idx))
            .copied()
            .unwrap_or(0.0);
        self.last_value_accessed = i as usize;
        v
    }

    /// Pascal `CalcMeanandStdDev`: even-interval (`RCDMeanAndStdDev`) or
    /// trapezoid-integrated over `Hours` (`CurveMeanAndStdDev`). An empty curve
    /// keeps the stored (default 0) values.
    fn calc_mean_std(&self) -> (f64, f64) {
        let Some(t) = self.values.as_ref() else {
            return (self.f_mean, self.f_std_dev);
        };
        let n = self.n().min(t.len());
        if n == 0 {
            return (self.f_mean, self.f_std_dev);
        }
        if self.interval > 0.0 {
            mean_and_std_dev(&t[..n])
        } else {
            match self.hours.as_ref() {
                Some(h) if h.len() >= n => curve_mean_and_std_dev(&t[..n], &h[..n]),
                _ => mean_and_std_dev(&t[..n]),
            }
        }
    }

    /// Pascal `Get_Mean` (recomputes on demand unless set explicitly).
    pub fn mean(&self) -> f64 {
        if self.std_dev_calculated {
            self.f_mean
        } else {
            self.calc_mean_std().0
        }
    }

    /// Pascal `Get_StdDev`.
    pub fn std_dev(&self) -> f64 {
        if self.std_dev_calculated {
            self.f_std_dev
        } else {
            self.calc_mean_std().1
        }
    }

    /// Pascal `DoCSVFile` (double-precision, non-MMF path): one row per point.
    /// For a fixed interval each row is a single value; for a variable interval
    /// (`Interval = 0`) each row is `hour, value`. Reads at most `NumPoints`
    /// rows and shrinks `NumPoints` to the count actually read.
    pub fn read_csv_file(&mut self, content: &str) {
        let npts = self.n();
        let variable = self.interval == 0.0;
        let mut v = vec![0.0; npts];
        let mut h = if variable {
            vec![0.0; npts]
        } else {
            Vec::new()
        };

        let mut parser = Parser::new();
        parser.set_auto_increment(false);
        let vars = ParserVars::new();

        let mut i = 0usize;
        for line in content.lines() {
            if i >= npts {
                break;
            }
            parser.set_cmd_string(line);
            if variable {
                parser.next_param(&vars);
                h[i] = parser.make_double(&vars).unwrap_or(0.0);
            }
            parser.next_param(&vars);
            v[i] = parser.make_double(&vars).unwrap_or(0.0);
            i += 1;
        }

        // Pascal shrinks NumPoints to the count read; the surviving values are
        // exactly the first `i`.
        v.truncate(i);
        self.values = store_array(v);
        if variable {
            h.truncate(i);
            self.hours = store_array(h);
        }
        self.num_points = i as i32;
    }

    /// Pascal `Common/Utilities.pas` `DoSngFile` (little-endian `f32` stream),
    /// called with `OnlyLoadB = Interval <> 0`: a fixed interval reads a bare
    /// value stream (`Hours` untouched); `Interval = 0` reads `(hour, value)`
    /// pairs into both arrays. Reads at most `NumPoints` points and shrinks
    /// `NumPoints` to the count actually read.
    pub fn read_sng_file(&mut self, content: &[u8]) {
        let npts = self.n();
        if self.interval == 0.0 {
            let mut h = Vec::with_capacity(npts);
            let mut v = Vec::with_capacity(npts);
            let mut off = 0usize;
            while v.len() < npts && off + 8 <= content.len() {
                let hr = f32::from_le_bytes(content[off..off + 4].try_into().unwrap());
                let val = f32::from_le_bytes(content[off + 4..off + 8].try_into().unwrap());
                h.push(hr as f64);
                v.push(val as f64);
                off += 8;
            }
            self.num_points = v.len() as i32;
            self.hours = store_array(h);
            self.values = store_array(v);
        } else {
            let n = (content.len() / 4).min(npts);
            let v: Vec<f64> = content[..n * 4]
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes(c.try_into().unwrap()) as f64)
                .collect();
            self.num_points = n as i32;
            self.values = store_array(v);
        }
    }

    /// Pascal `Common/Utilities.pas` `DoDblFile` (little-endian `f64` stream);
    /// same row layout as [`Self::read_sng_file`] but double precision.
    pub fn read_dbl_file(&mut self, content: &[u8]) {
        let npts = self.n();
        if self.interval == 0.0 {
            let mut h = Vec::with_capacity(npts);
            let mut v = Vec::with_capacity(npts);
            let mut off = 0usize;
            while v.len() < npts && off + 16 <= content.len() {
                let hr = f64::from_le_bytes(content[off..off + 8].try_into().unwrap());
                let val = f64::from_le_bytes(content[off + 8..off + 16].try_into().unwrap());
                h.push(hr);
                v.push(val);
                off += 16;
            }
            self.num_points = v.len() as i32;
            self.hours = store_array(h);
            self.values = store_array(v);
        } else {
            let n = (content.len() / 8).min(npts);
            let v: Vec<f64> = content[..n * 8]
                .chunks_exact(8)
                .map(|c| f64::from_le_bytes(c.try_into().unwrap()))
                .collect();
            self.num_points = n as i32;
            self.values = store_array(v);
        }
    }

    /// Pascal `MakeLike` (shared body): copy points and interval; the hour array
    /// is dropped for a fixed interval (`ReallocMem(Hours, 0)`).
    pub fn make_like_from(&mut self, other: &ScalarShapeCore) {
        self.data.copy_prp_sequence_from(&other.data);
        self.num_points = other.num_points;
        self.interval = other.interval;
        self.values = other.values.clone();
        self.hours = if self.interval > 0.0 {
            None
        } else {
            other.hours.clone()
        };
    }
}

/// Pascal `ReAllocmem` semantics: an empty parse result is a freed (NIL)
/// pointer, so it reads back as an empty dump; non-empty becomes an allocated
/// array.
pub fn store_array(value: Vec<f64>) -> Option<Vec<f64>> {
    if value.is_empty() { None } else { Some(value) }
}
