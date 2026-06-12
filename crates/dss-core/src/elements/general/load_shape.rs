//! `LoadShape` — per-unit multiplier curve indexed by hour.
//! Port of Pascal `General/LoadShape.pas` (`TLoadShapeObj`). A `DSS_OBJECT`
//! class (no terminals, no YPrim) consumed by Load/VSource/Storage as the
//! daily/yearly/duty multiplier source (wired in WP5.3) and by the time-series
//! solution modes (WP5.8).
//!
//! Scope (WP5.2a/b): the in-memory core — fixed- and variable-interval data,
//! `GetMultAtHour` (the consumer-facing lookup), `Normalize`, `SetMaxPandQ`,
//! lazy mean/std-dev, `MakeLike`, and `CSVFile` (WP5.2b: the read is deferred
//! to the executive via [`FileLoad`], which resolves the path relative to the
//! script's current directory and hands back the text). The binary file props
//! (`SngFile`/`DblFile`/`PQCSVFile`) stay `NOT_PORTED`. Single-precision arrays
//! (`sP`/`sH`/`sQ`) and memory-mapped files (`MemoryMapping`) are not ported (no
//! corpus case needs them); `MemoryMapping=yes` stores the flag but the lookup
//! never takes the MMF path.

use crate::obj::base::{DssObjData, DssObject, FileLoad};
use crate::obj::props::{PropDef, PropFlags, define_properties};
use crate::support::mathutil::{curve_mean_and_std_dev, mean_and_std_dev};
use dss_parser::{Parser, ParserVars};
use num_complex::Complex64;

// Pascal `TLoadShapeProp` ordinals (1..22) + the property table.
define_properties! {
    class "LoadShape", abbrev true, enums enums;
    1  NPTS      => PropDef::integer("NPts").flags(PropFlags::SUPPRESS_JSON);
    2  INTERVAL  => PropDef::double("Interval")
        .flags(PropFlags::NON_NEGATIVE | PropFlags::REQUIRED_IN_SPEC_SET);
    3  MULT      => PropDef::double_array("Mult", NPTS).flags(PropFlags::REDUNDANT);
    4  HOUR      => PropDef::double_array("Hour", NPTS).flags(PropFlags::REQUIRED_IN_SPEC_SET);
    5  MEAN      => PropDef::double("Mean");
    6  STDDEV    => PropDef::double("StdDev");
    7  CSVFILE   => PropDef::string("CSVFile").flags(
        PropFlags::IS_FILENAME | PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::GLOBAL_COUNT,
    );
    8  SNGFILE   => PropDef::string("SngFile").flags(
        PropFlags::NOT_PORTED | PropFlags::IS_FILENAME
            | PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::GLOBAL_COUNT,
    );
    9  DBLFILE   => PropDef::string("DblFile").flags(
        PropFlags::NOT_PORTED | PropFlags::IS_FILENAME
            | PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::GLOBAL_COUNT,
    );
    10 ACTION    => PropDef::action("Action", enums.load_shape_action);
    11 QMULT     => PropDef::double_array("QMult", NPTS);
    12 USEACTUAL => PropDef::boolean("UseActual");
    13 PMAX      => PropDef::double("PMax");
    14 QMAX      => PropDef::double("QMax");
    15 SINTERVAL => PropDef::double("SInterval")
        .scale(1.0 / 3600.0)
        .flags(PropFlags::REDUNDANT | PropFlags::NON_NEGATIVE);
    16 MINTERVAL => PropDef::double("MInterval")
        .scale(1.0 / 60.0)
        .flags(PropFlags::REDUNDANT | PropFlags::NON_NEGATIVE);
    17 PBASE     => PropDef::double("PBase");
    18 QBASE     => PropDef::double("QBase");
    19 PMULT     => PropDef::double_array("PMult", NPTS).flags(PropFlags::REQUIRED_IN_SPEC_SET);
    20 PQCSVFILE => PropDef::string("PQCSVFile").flags(
        PropFlags::NOT_PORTED | PropFlags::IS_FILENAME
            | PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::GLOBAL_COUNT,
    );
    21 MEMORYMAPPING => PropDef::boolean("MemoryMapping");
    22 INTERPOLATION => PropDef::mapped_string_enum("Interpolation", enums.load_shape_interp);
}

use prop::{
    CSVFILE, DBLFILE, HOUR, INTERPOLATION, INTERVAL, MEAN, MEMORYMAPPING, MINTERVAL, MULT, NPTS,
    PBASE, PMAX, PMULT, PQCSVFILE, QBASE, QMAX, QMULT, SINTERVAL, SNGFILE, STDDEV, USEACTUAL,
};

/// `Avg` interpolation ordinal (`TLoadShapeInterp.Avg`).
const INTERP_AVG: i32 = 0;
/// `Edge` interpolation ordinal (`TLoadShapeInterp.Edge`).
const INTERP_EDGE: i32 = 1;

/// A `LoadShape` instance (`TLoadShapeObj`).
#[derive(Debug, Clone)]
pub struct LoadShapeObj {
    data: DssObjData,
    /// Number of points in the curve (Pascal `NumPoints`).
    num_points: i32,
    /// Fixed interval in hours; `0.0` means variable interval (use `hour`).
    interval: f64,
    /// Active-power multipliers (Pascal `dP`). `None` = NIL pointer.
    p_mult: Option<Vec<f64>>,
    /// Reactive-power multipliers (Pascal `dQ`); `None` falls back to `p_mult`.
    q_mult: Option<Vec<f64>>,
    /// Hour values for variable interval (Pascal `dH`); `None` = even spacing.
    hour: Option<Vec<f64>>,
    /// Mean / std-dev (Pascal `FMean`/`FStdDev`); lazily computed unless set.
    f_mean: f64,
    f_std_dev: f64,
    /// Pascal `FStdDevCalculated`: true when mean/std-dev were set explicitly
    /// (skips the on-demand recompute).
    std_dev_calculated: bool,
    /// Pascal `UseActual`: multipliers are absolute (kW/kvar), not per-unit.
    use_actual: bool,
    /// Peak P / peak-coincident Q (Pascal `MaxP`/`MaxQ`).
    max_p: f64,
    max_q: f64,
    /// Pascal `MaxQSpecified`: `QMax` set explicitly (do not recompute it).
    max_q_specified: bool,
    /// Normalization bases (Pascal `BaseP`/`BaseQ`).
    base_p: f64,
    base_q: f64,
    /// Pascal `interpolation` (`Avg`=0, `Edge`=1).
    interpolation: i32,
    /// Pascal `UseMMF` (memory-mapped files); stored but the lookup never uses
    /// it (MMF not ported).
    use_mmf: bool,
    /// Hunt cache for the variable-interval lookup (Pascal `LastValueAccessed`,
    /// a 0-based index into the 0-based arrays; ctor sets it to 1).
    last_value_accessed: usize,
    csvfile: String,
    sngfile: String,
    dblfile: String,
    pqcsvfile: String,
    /// Deferred file reads queued by `CSVFile` (drained by the executive).
    pending_file_loads: Vec<FileLoad>,
}

impl LoadShapeObj {
    /// Pascal `TLoadShapeObj.Create`.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            data: DssObjData::new(name.into().to_lowercase(), prop::NUM_PROPS),
            num_points: 0,
            interval: 1.0,
            p_mult: None,
            q_mult: None,
            hour: None,
            f_mean: 0.0,
            f_std_dev: 0.0,
            std_dev_calculated: false,
            use_actual: false,
            max_p: 1.0,
            max_q: 0.0,
            max_q_specified: false,
            base_p: 0.0,
            base_q: 0.0,
            interpolation: INTERP_AVG,
            use_mmf: false,
            last_value_accessed: 1,
            csvfile: String::new(),
            sngfile: String::new(),
            dblfile: String::new(),
            pqcsvfile: String::new(),
            pending_file_loads: Vec::new(),
        }
    }

    fn n(&self) -> usize {
        self.num_points.max(0) as usize
    }

    /// `Set_Result_im` from `GetMultAtHour`: the imaginary part when no Q
    /// multipliers are defined (0 in actual mode, else mirror the real part).
    fn result_im(&self, real_part: f64) -> f64 {
        if self.use_actual { 0.0 } else { real_part }
    }

    /// Pascal `TLoadShapeObj.GetMultAtHour`: the (P, Q) multiplier nearest the
    /// requested hour. Returns `(1, 1)` for an empty curve; repeats the curve
    /// past its last point. Ported verbatim for the double-precision,
    /// non-memory-mapped path (the only one the corpus uses); single arrays and
    /// MMF are out of scope.
    pub fn get_mult_at_hour(&mut self, hr: f64) -> Complex64 {
        let npts = self.n();
        if npts == 0 {
            return Complex64::new(1.0, 1.0); // default for an empty curve
        }
        let Some(p) = self.p_mult.as_ref() else {
            return Complex64::new(1.0, 1.0);
        };
        if p.is_empty() {
            return Complex64::new(1.0, 1.0);
        }

        if npts == 1 {
            let re = p[0];
            let im = match self.q_mult.as_ref() {
                Some(q) if !q.is_empty() => q[0],
                _ => self.result_im(re),
            };
            return Complex64::new(re, im);
        }

        // --- Fixed (even) interval ---
        if self.interval > 0.0 {
            // TODO(compat): FPC `Round` is banker's rounding (ties-to-even);
            // these indices are always in i64 range, so `round_ties_even`
            // reproduces it. Wiped with the other compat shims.
            let mut i = if self.interpolation == INTERP_EDGE {
                (hr / self.interval).floor() as i64
            } else {
                (hr / self.interval).round_ties_even() as i64
            };
            let np = npts as i64;
            if i > np {
                i %= np; // wrap around using remainder
            }
            if i == 0 {
                i = np;
            }
            i -= 1;
            let off = i as usize;
            let re = p[off];
            let im = match self.q_mult.as_ref() {
                Some(q) if off < q.len() => q[off],
                _ => self.result_im(re),
            };
            return Complex64::new(re, im);
        }

        // --- Variable interval (hour array) ---
        let mut hr = hr;
        let h = match self.hour.as_ref() {
            Some(h) if h.len() >= npts => h.clone(),
            // No hour array despite Interval==0: degenerate; treat as empty.
            _ => return Complex64::new(1.0, 1.0),
        };

        // Normalize Hr into the first cycle (wraparound).
        let last_h = h[npts - 1];
        if hr > last_h && last_h != 0.0 {
            hr -= (hr / last_h).trunc() * last_h;
        }

        if h[self.last_value_accessed.min(npts - 1)] > hr {
            self.last_value_accessed = 0; // start over from the beginning
        }

        for i in self.last_value_accessed..npts {
            if (h[i] - hr).abs() < 0.00001 {
                // Close to an actual point — use it directly.
                let re = p[i];
                let im = match self.q_mult.as_ref() {
                    Some(q) if i < q.len() => q[i],
                    _ => self.result_im(re),
                };
                self.last_value_accessed = i;
                return Complex64::new(re, im);
            }
            if h[i] > hr {
                if self.interpolation == INTERP_EDGE {
                    // Edge: hold the last point at or before Hr.
                    let mut re = 0.0;
                    let mut im = 0.0;
                    for k in 0..npts {
                        if h[k] <= hr {
                            re = p[k];
                            if let Some(q) = self.q_mult.as_ref()
                                && k < q.len()
                            {
                                im = q[k];
                            }
                        } else {
                            break;
                        }
                    }
                    return Complex64::new(re, im);
                }
                // Avg: linear interpolation between i-1 and i. Pascal reads
                // dP[i-1]; for i == 0 that underflows (reads garbage) — an
                // hour before the first point is undefined input, so we return
                // the first point instead of reproducing the UB.
                if i == 0 {
                    let re = p[0];
                    let im = match self.q_mult.as_ref() {
                        Some(q) if !q.is_empty() => q[0],
                        _ => self.result_im(re),
                    };
                    return Complex64::new(re, im);
                }
                self.last_value_accessed = i - 1;
                let prev = i - 1;
                let frac = (hr - h[prev]) / (h[i] - h[prev]);
                let re = p[prev] + frac * (p[i] - p[prev]);
                let im = match self.q_mult.as_ref() {
                    Some(q) if i < q.len() => q[prev] + frac * (q[i] - q[prev]),
                    _ => self.result_im(re),
                };
                return Complex64::new(re, im);
            }
        }

        // Fell through the loop: use the last interior value.
        self.last_value_accessed = npts - 2;
        let off = self.last_value_accessed;
        let re = p[off];
        let im = match self.q_mult.as_ref() {
            Some(q) if off < q.len() => q[off],
            _ => self.result_im(re),
        };
        Complex64::new(re, im)
    }

    /// Pascal `iMaxAbsArrayValue` − 1: 0-based index of the largest-magnitude
    /// element over the first `npts` entries, or `None` for an empty array.
    fn i_max_abs(a: &[f64], npts: usize) -> Option<usize> {
        let n = npts.min(a.len());
        if n == 0 {
            return None;
        }
        let mut idx = 0;
        let mut maxv = a[0].abs();
        for (i, &v) in a.iter().enumerate().take(n).skip(1) {
            if v.abs() > maxv {
                maxv = v.abs();
                idx = i;
            }
        }
        Some(idx)
    }

    /// Pascal `TLoadShapeObj.SetMaxPandQ`: peak P and the coincident Q.
    fn set_max_p_and_q(&mut self) {
        let Some(p) = self.p_mult.as_ref() else {
            return;
        };
        let Some(i_max) = Self::i_max_abs(p, self.n()) else {
            return;
        };
        self.max_p = p[i_max];
        if !self.max_q_specified {
            self.max_q = match self.q_mult.as_ref() {
                Some(q) if i_max < q.len() => q[i_max],
                _ => 0.0,
            };
        }
    }

    /// Pascal `TLoadShapeObj.Normalize`: scale the multipliers so the peak (or
    /// `BaseP`/`BaseQ` if set) becomes 1.0.
    fn normalize(&mut self, errors: &mut Vec<String>) {
        if !self.has_data(errors) {
            return;
        }
        let npts = self.n();
        let base_p = self.base_p;
        let base_q = self.base_q;
        if let Some(p) = self.p_mult.as_mut() {
            Self::do_normalize(p, npts, base_p);
        }
        if let Some(q) = self.q_mult.as_mut() {
            Self::do_normalize(q, npts, base_q);
        }
        self.use_actual = false;
    }

    /// Inner `DoNormalize`: divide by `max_mult` (or the array's own peak
    /// magnitude when `max_mult <= 0`).
    fn do_normalize(mult: &mut [f64], npts: usize, mut max_mult: f64) {
        let n = npts.min(mult.len());
        if n == 0 {
            return;
        }
        if max_mult <= 0.0 {
            max_mult = mult[0].abs();
            for v in &mult[1..n] {
                max_mult = max_mult.max(v.abs());
            }
        }
        if max_mult == 0.0 {
            max_mult = 1.0; // avoid divide by zero
        }
        for v in &mut mult[..n] {
            *v /= max_mult;
        }
    }

    /// Pascal `TLoadShapeObj.HasData`: true once P multipliers exist; otherwise
    /// records error 61107 and returns false.
    fn has_data(&mut self, errors: &mut Vec<String>) -> bool {
        if self.p_mult.as_ref().is_some_and(|p| !p.is_empty()) {
            return true;
        }
        errors.push(format!(
            "LoadShape.{}: LoadShape has no data to be normalized. \
             Check for previous errors. (61107)",
            self.data.name()
        ));
        false
    }

    /// Pascal `TLoadShapeObj.CalcMeanandStdDev` (double, non-MMF path): the
    /// mean and std-dev of the P multipliers, even-interval (`RCDMeanAndStdDev`)
    /// or trapezoid-integrated over `hour` (`CurveMeanAndStdDev`).
    fn calc_mean_std(&self) -> (f64, f64) {
        let Some(p) = self.p_mult.as_ref() else {
            return (self.f_mean, self.f_std_dev);
        };
        let n = self.n().min(p.len());
        if n == 0 {
            return (self.f_mean, self.f_std_dev);
        }
        if self.interval > 0.0 {
            mean_and_std_dev(&p[..n])
        } else {
            match self.hour.as_ref() {
                Some(h) if h.len() >= n => curve_mean_and_std_dev(&p[..n], &h[..n]),
                _ => mean_and_std_dev(&p[..n]),
            }
        }
    }

    fn mean(&self) -> f64 {
        if self.std_dev_calculated {
            self.f_mean
        } else {
            self.calc_mean_std().0
        }
    }

    fn std_dev(&self) -> f64 {
        if self.std_dev_calculated {
            self.f_std_dev
        } else {
            self.calc_mean_std().1
        }
    }

    /// Pascal `TLoadShapeObj.ReadCSVFile` (double-precision, non-MMF path): one
    /// row per point, parsed with the comma/whitespace aux parser. For a fixed
    /// interval each row is a single P multiplier; for a variable interval
    /// (`Interval = 0`) each row is `hour, mult`. Reads at most `NumPoints` rows
    /// and shrinks `NumPoints` to the count actually read.
    fn read_csv_file(&mut self, content: &str) {
        let npts = self.n();
        let variable = self.interval == 0.0;
        let mut p = vec![0.0; npts];
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
            p[i] = parser.make_double(&vars).unwrap_or(0.0);
            i += 1;
        }

        // Pascal shrinks NumPoints to the count read; the surviving values are
        // exactly the first `i`.
        p.truncate(i);
        self.p_mult = store_array(p);
        if variable {
            h.truncate(i);
            self.hour = store_array(h);
        }
        self.num_points = i as i32;
    }
}

/// Pascal `ReAllocmem` semantics for the data setters: an empty parse result is
/// a freed (NIL) pointer, so it reads back as an empty dump; non-empty becomes
/// an allocated array.
fn store_array(value: Vec<f64>) -> Option<Vec<f64>> {
    if value.is_empty() { None } else { Some(value) }
}

impl DssObject for LoadShapeObj {
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
            NPTS => self.num_points,
            INTERPOLATION => self.interpolation,
            _ => unreachable!("LoadShape has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        match idx {
            NPTS => self.num_points = value,
            INTERPOLATION => self.interpolation = value,
            _ => unreachable!("LoadShape has no integer property {idx}"),
        }
    }

    fn get_f64(&self, idx: usize) -> f64 {
        match idx {
            // Interval and its second/minute aliases share one field; the
            // engine applies the scale on the way out.
            INTERVAL | SINTERVAL | MINTERVAL => self.interval,
            MEAN => self.mean(),
            STDDEV => self.std_dev(),
            PMAX => self.max_p,
            QMAX => self.max_q,
            PBASE => self.base_p,
            QBASE => self.base_q,
            _ => unreachable!("LoadShape has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        match idx {
            INTERVAL | SINTERVAL | MINTERVAL => self.interval = value,
            // Pascal `Set_Mean`/`Set_StdDev`: mark as externally provided.
            MEAN => {
                self.f_mean = value;
                self.std_dev_calculated = true;
            }
            STDDEV => {
                self.f_std_dev = value;
                self.std_dev_calculated = true;
            }
            PMAX => self.max_p = value,
            QMAX => self.max_q = value,
            PBASE => self.base_p = value,
            QBASE => self.base_q = value,
            _ => unreachable!("LoadShape has no double property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        match idx {
            USEACTUAL => self.use_actual,
            MEMORYMAPPING => self.use_mmf,
            _ => unreachable!("LoadShape has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        match idx {
            USEACTUAL => self.use_actual = value,
            MEMORYMAPPING => self.use_mmf = value,
            _ => unreachable!("LoadShape has no boolean property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        match idx {
            CSVFILE => self.csvfile.clone(),
            SNGFILE => self.sngfile.clone(),
            DBLFILE => self.dblfile.clone(),
            PQCSVFILE => self.pqcsvfile.clone(),
            _ => unreachable!("LoadShape has no string property {idx}"),
        }
    }
    fn set_string(&mut self, idx: usize, value: String) {
        match idx {
            CSVFILE => self.csvfile = value,
            SNGFILE => self.sngfile = value,
            DBLFILE => self.dblfile = value,
            PQCSVFILE => self.pqcsvfile = value,
            _ => unreachable!("LoadShape has no string property {idx}"),
        }
    }

    fn get_f64_array(&self, idx: usize) -> Option<&[f64]> {
        match idx {
            // Mult and PMult are the same array (Pascal `dP`).
            MULT | PMULT => self.p_mult.as_deref(),
            HOUR => self.hour.as_deref(),
            QMULT => self.q_mult.as_deref(),
            _ => unreachable!("LoadShape has no array property {idx}"),
        }
    }
    fn set_f64_array(&mut self, idx: usize, value: Vec<f64>) {
        let stored = store_array(value);
        match idx {
            MULT | PMULT => self.p_mult = stored,
            HOUR => self.hour = stored,
            QMULT => self.q_mult = stored,
            _ => unreachable!("LoadShape has no array property {idx}"),
        }
    }

    /// Pascal `StringEnumActionProperty` for `Action`.
    fn do_action(&mut self, ordinal: i32, errors: &mut Vec<String>) {
        match ordinal {
            0 => self.normalize(errors), // Normalize
            // DblSave / SngSave write binary files — not ported.
            _ => errors.push(format!(
                "LoadShape.{}: Action=DblSave/SngSave (binary file output) is not ported.",
                self.data.name()
            )),
        }
    }

    fn take_file_loads(&mut self) -> Vec<FileLoad> {
        std::mem::take(&mut self.pending_file_loads)
    }

    /// Apply a resolved `CSVFile` (Pascal `DoCSVFile`). Other file kinds are
    /// `NOT_PORTED`, so they never reach here.
    fn apply_file_load(&mut self, load: &FileLoad, content: &str, _errors: &mut Vec<String>) {
        if load.prop == CSVFILE {
            self.read_csv_file(content);
        }
    }

    /// Pascal `TLoadShapeObj.PropertySideEffects` (the parts that affect the
    /// in-memory model; `PrpSequence` bookkeeping is display-only and inert).
    fn side_effects(&mut self, idx: usize, _prev_int: i32) {
        match idx {
            // Setting Mult/PMult/QMult invalidates the cached statistics.
            MULT | PMULT | QMULT => {
                self.std_dev_calculated = false;
            }
            // Pascal `DoCSVFile` runs here, but the hook can't reach the
            // filesystem/current dir: queue the read for the executive.
            CSVFILE => {
                self.std_dev_calculated = false;
                self.pending_file_loads.push(FileLoad {
                    prop: CSVFILE,
                    filename: self.csvfile.clone(),
                });
            }
            QMAX => self.max_q_specified = true,
            // Interval and Hour are mutually exclusive specs.
            INTERVAL => self.data.clear_seq(HOUR),
            HOUR => {
                self.interval = 0.0;
                self.data.clear_seq(INTERVAL);
            }
            _ => {}
        }
    }

    /// Pascal `TLoadShape.EndEdit`: recompute peaks once data exists.
    fn end_edit(&mut self) {
        if self.p_mult.is_some() {
            self.set_max_p_and_q();
        }
    }

    /// Pascal `TLoadShapeObj.MakeLike`.
    fn make_like(&mut self, other: &dyn DssObject) {
        self.data.copy_prp_sequence_from(other.data());
        let Some(o) = other.as_any().downcast_ref::<LoadShapeObj>() else {
            return;
        };
        self.num_points = o.num_points;
        self.interval = o.interval;
        self.p_mult = o.p_mult.clone();
        self.q_mult = o.q_mult.clone();
        // With a fixed interval the hour array is dropped (Pascal frees dH).
        self.hour = if self.interval > 0.0 {
            None
        } else {
            o.hour.clone()
        };
        self.use_actual = o.use_actual;
        self.use_mmf = o.use_mmf;
        self.base_p = o.base_p;
        self.base_q = o.base_q;
        self.set_max_p_and_q();
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

    fn edited(edits: &[(&str, &str)]) -> (ClassProps, LoadShapeObj, Vec<String>) {
        let enums = EnumRegistry::new();
        let cls = class_props(&enums);
        let mut obj = LoadShapeObj::new("d");
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

    fn get(cls: &ClassProps, obj: &LoadShapeObj, name: &str) -> String {
        let enums = EnumRegistry::new();
        let idx = cls.property_index(name).unwrap();
        cls.get_value(obj, idx, &enums)
    }

    #[test]
    fn defaults_match_oracle() {
        // Oracle (dss-python 0.15.7) `new loadshape.d`.
        let (cls, obj, errs) = edited(&[]);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(get(&cls, &obj, "NPts"), "0");
        assert_eq!(get(&cls, &obj, "Interval"), "1");
        assert_eq!(get(&cls, &obj, "Mult"), "");
        assert_eq!(get(&cls, &obj, "PMax"), "1");
        assert_eq!(get(&cls, &obj, "QMax"), "0");
        assert_eq!(get(&cls, &obj, "SInterval"), "3600");
        assert_eq!(get(&cls, &obj, "MInterval"), "60");
        assert_eq!(get(&cls, &obj, "Interpolation"), "Avg");
        assert_eq!(get(&cls, &obj, "Action"), "");
    }

    #[test]
    fn fixed_interval_arrays_and_stats() {
        // Oracle: mult=(1 2 4 8) interval=1 → Mean 3.75, StdDev 3.0956…, PMax 8.
        let (cls, obj, errs) = edited(&[("npts", "4"), ("interval", "1"), ("mult", "1 2 4 8")]);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(get(&cls, &obj, "Mult"), "[ 1 2 4 8]");
        assert_eq!(get(&cls, &obj, "PMult"), "[ 1 2 4 8]");
        assert_eq!(get(&cls, &obj, "PMax"), "8");
        assert!((get(&cls, &obj, "Mean").parse::<f64>().unwrap() - 3.75).abs() < 1e-12);
        assert!(
            (get(&cls, &obj, "StdDev").parse::<f64>().unwrap() - 3.09569593683445).abs() < 1e-9
        );
    }

    #[test]
    fn qmult_drives_coincident_qmax() {
        // Oracle: QMax = Q at the index where |P| peaks (index 3 → 0.8).
        let (cls, obj, errs) = edited(&[
            ("npts", "4"),
            ("interval", "1"),
            ("mult", "1 2 4 8"),
            ("qmult", ".5 .6 .7 .8"),
        ]);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(get(&cls, &obj, "QMult"), "[ 0.5 0.6 0.7 0.8]");
        assert_eq!(get(&cls, &obj, "QMax"), "0.8");
    }

    #[test]
    fn second_and_minute_interval_aliases() {
        // sinterval=900 → 0.25 hr; minterval mirrors it.
        let (cls, obj, errs) = edited(&[("npts", "4"), ("sinterval", "900"), ("mult", "1 2 4 8")]);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(get(&cls, &obj, "Interval"), "0.25");
        assert_eq!(get(&cls, &obj, "SInterval"), "900");
        assert_eq!(get(&cls, &obj, "MInterval"), "15");
    }

    #[test]
    fn hour_array_sets_variable_interval() {
        let (cls, obj, errs) = edited(&[
            ("npts", "3"),
            ("interval", "0"),
            ("hour", "1 2 4"),
            ("mult", "1 2 4"),
        ]);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(get(&cls, &obj, "Interval"), "0");
        assert_eq!(get(&cls, &obj, "Hour"), "[ 1 2 4]");
        // Trapezoid mean over the curve.
        assert!((get(&cls, &obj, "Mean").parse::<f64>().unwrap() - 2.5).abs() < 1e-12);
    }

    #[test]
    fn normalize_scales_to_peak() {
        // Oracle: action=normalize divides by peak (8) → max becomes 1.
        let (cls, obj, errs) = edited(&[
            ("npts", "4"),
            ("interval", "1"),
            ("mult", "2 4 6 8"),
            ("action", "normalize"),
        ]);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(get(&cls, &obj, "Mult"), "[ 0.25 0.5 0.75 1]");
        assert_eq!(get(&cls, &obj, "PMax"), "1");
    }

    #[test]
    fn normalize_uses_pbase_when_set() {
        // Oracle: pbase=10 → divide by 10 (not the peak).
        let (cls, obj, errs) = edited(&[
            ("npts", "4"),
            ("interval", "1"),
            ("mult", "2 4 6 8"),
            ("pbase", "10"),
            ("action", "normalize"),
        ]);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(get(&cls, &obj, "Mult"), "[ 0.2 0.4 0.6 0.8]");
    }

    #[test]
    fn explicit_mean_stddev_override_computation() {
        let (cls, obj, errs) = edited(&[
            ("npts", "4"),
            ("interval", "1"),
            ("mult", "2 4 6 8"),
            ("mean", "5"),
            ("stddev", "2"),
        ]);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(get(&cls, &obj, "Mean"), "5");
        assert_eq!(get(&cls, &obj, "StdDev"), "2");
    }

    #[test]
    fn get_mult_at_hour_fixed_interval_wraps() {
        // Hand-traced from the Pascal even-interval branch (Avg, no Q).
        // dP = [10, 20, 30, 40], interval=1. round(hr) wraps: hr 0 → last point.
        let (_cls, mut obj, errs) =
            edited(&[("npts", "4"), ("interval", "1"), ("mult", "10 20 30 40")]);
        assert!(errs.is_empty(), "{errs:?}");
        let cases = [
            (0.0, 40.0), // round(0)=0 → wraps to NumPoints → dP[3]
            (1.0, 10.0), // round(1)=1 → dP[0]
            (2.0, 20.0),
            (3.0, 30.0),
            (4.0, 40.0),
            (4.25, 40.0), // round(4.25)=4 → dP[3]
            (5.0, 10.0),  // 5 mod 4 = 1 → dP[0]
        ];
        for (hr, want) in cases {
            let m = obj.get_mult_at_hour(hr);
            assert!(
                (m.re - want).abs() < 1e-12,
                "P at hr {hr}: {} != {want}",
                m.re
            );
            // No Q array, not actual: im mirrors re.
            assert!((m.im - want).abs() < 1e-12, "Q at hr {hr}");
        }
    }

    #[test]
    fn get_mult_at_hour_variable_interval_interpolates() {
        // Hand-traced from the Pascal random-interval branch (Avg).
        // hour=[1,2,4], mult=[1,2,4].
        let (_cls, mut obj, errs) = edited(&[
            ("npts", "3"),
            ("interval", "0"),
            ("hour", "1 2 4"),
            ("mult", "1 2 4"),
        ]);
        assert!(errs.is_empty(), "{errs:?}");
        for (hr, want) in [(1.0, 1.0), (1.5, 1.5), (2.0, 2.0), (3.0, 3.0), (4.0, 4.0)] {
            let m = obj.get_mult_at_hour(hr);
            assert!((m.re - want).abs() < 1e-12, "hr {hr}: {} != {want}", m.re);
        }
        // hr=5 wraps: 5 - trunc(5/4)*4 = 1 → exact point dP[0]=1.
        assert!((obj.get_mult_at_hour(5.0).re - 1.0).abs() < 1e-12);
    }

    #[test]
    fn get_mult_at_hour_use_actual_zero_q() {
        // UseActual: missing Q multiplier → 0, not a mirror of P.
        let (_cls, mut obj, errs) = edited(&[
            ("npts", "2"),
            ("interval", "1"),
            ("mult", "3 6"),
            ("useactual", "yes"),
        ]);
        assert!(errs.is_empty(), "{errs:?}");
        let m = obj.get_mult_at_hour(1.0);
        assert!((m.re - 3.0).abs() < 1e-12);
        assert_eq!(m.im, 0.0);
    }

    #[test]
    fn make_like_copies_and_recomputes() {
        let (cls, base, errs) = edited(&[
            ("npts", "3"),
            ("interval", "2"),
            ("mult", "1 2 3"),
            ("qmult", "4 5 6"),
            ("pbase", "7"),
            ("useactual", "yes"),
        ]);
        assert!(errs.is_empty(), "{errs:?}");
        let mut obj = LoadShapeObj::new("d");
        obj.make_like(&base);
        obj.end_edit();
        assert_eq!(get(&cls, &obj, "NPts"), "3");
        assert_eq!(get(&cls, &obj, "Interval"), "2");
        assert_eq!(get(&cls, &obj, "Mult"), "[ 1 2 3]");
        assert_eq!(get(&cls, &obj, "QMult"), "[ 4 5 6]");
        assert_eq!(get(&cls, &obj, "PMax"), "3");
        assert_eq!(get(&cls, &obj, "QMax"), "6");
        assert_eq!(get(&cls, &obj, "PBase"), "7");
        assert_eq!(get(&cls, &obj, "UseActual"), "Yes");
    }

    #[test]
    fn binary_file_props_are_not_ported() {
        // CSVFile is now ported; SngFile/DblFile/PQCSVFile stay NOT_PORTED.
        let enums = EnumRegistry::new();
        let cls = class_props(&enums);
        for name in ["sngfile", "dblfile", "pqcsvfile"] {
            let mut obj = LoadShapeObj::new("d");
            let mut parser = Parser::new();
            let vars = ParserVars::new();
            let mut errors = Vec::new();
            let idx = cls.property_index(name).unwrap();
            let mut eng = PropEngine {
                parser: &mut parser,
                vars: &vars,
                enums: &enums,
                errors: &mut errors,
                foreign: None,
            };
            let err = cls
                .edit_property(&mut obj, idx, "shape.bin", &mut eng)
                .unwrap_err();
            assert!(
                err.to_string().to_lowercase().contains("not ported"),
                "{name}: {err}"
            );
        }
    }

    #[test]
    fn csvfile_queues_a_file_load() {
        // Setting CSVFile records a deferred load (the executive does the read).
        let (_cls, mut obj, errs) =
            edited(&[("npts", "6"), ("interval", "1"), ("csvfile", "day.csv")]);
        // No executive here, so the file is never read — but the request is
        // queued and the stored filename reads back.
        let loads = obj.take_file_loads();
        assert_eq!(loads.len(), 1);
        assert_eq!(loads[0].prop, prop::CSVFILE);
        assert_eq!(loads[0].filename, "day.csv");
        assert!(errs.is_empty(), "{errs:?}");
    }

    #[test]
    fn read_csv_file_fixed_and_variable_interval() {
        // Fixed interval: one mult per row; oracle Mult=[0.3 0.5 0.9 1 0.7 0.4].
        let (cls, mut obj, _) = edited(&[("npts", "6"), ("interval", "1")]);
        obj.read_csv_file("0.3\n0.5\n0.9\n1.0\n0.7\n0.4\n");
        obj.end_edit();
        assert_eq!(get(&cls, &obj, "NPts"), "6");
        assert_eq!(get(&cls, &obj, "Mult"), "[ 0.3 0.5 0.9 1 0.7 0.4]");
        assert_eq!(get(&cls, &obj, "PMax"), "1");

        // Variable interval: `hour, mult` per row; oracle Hour=[0 2 5 9].
        let (cls, mut obj, _) = edited(&[("npts", "4"), ("interval", "0")]);
        obj.read_csv_file("0,0.30\n2,0.55\n5,0.95\n9,0.60\n");
        obj.end_edit();
        assert_eq!(get(&cls, &obj, "Hour"), "[ 0 2 5 9]");
        assert_eq!(get(&cls, &obj, "Mult"), "[ 0.3 0.55 0.95 0.6]");
        assert_eq!(get(&cls, &obj, "PMax"), "0.95");
    }

    #[test]
    fn read_csv_file_shrinks_npts_to_lines_read() {
        // Oracle: npts=10 but only 6 rows in the file → NumPoints becomes 6.
        let (cls, mut obj, _) = edited(&[("npts", "10"), ("interval", "1")]);
        obj.read_csv_file("0.3\n0.5\n0.9\n1.0\n0.7\n0.4\n");
        obj.end_edit();
        assert_eq!(get(&cls, &obj, "NPts"), "6");
        assert_eq!(get(&cls, &obj, "Mult"), "[ 0.3 0.5 0.9 1 0.7 0.4]");
    }

    /// Full path through the executive: a temp CSV resolved relative to the
    /// script's current directory, read and parsed (Pascal `DoCSVFile`). Values
    /// are transcribed from the pinned oracle.
    #[test]
    fn csvfile_through_executive_matches_oracle() {
        use crate::exec::Dss;

        let dir = std::env::temp_dir().join(format!(
            "dss_ls_csv_{}_{}",
            std::process::id(),
            // a per-test nonce so parallel runs don't collide
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let day6 = dir.join("day6.csv");
        std::fs::write(&day6, "0.3\n0.5\n0.9\n1.0\n0.7\n0.4\n").unwrap();

        let mut dss = Dss::new();
        dss.command("clear");
        dss.command("new circuit.p");
        dss.command(&format!(
            "New LoadShape.d npts=6 interval=1 csvfile=\"{}\"",
            day6.display()
        ));
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());

        dss.command("? LoadShape.d.npts");
        assert_eq!(dss.result(), "6");
        dss.command("? LoadShape.d.mult");
        assert_eq!(dss.result(), "[ 0.3 0.5 0.9 1 0.7 0.4]");
        dss.command("? LoadShape.d.pmax");
        assert_eq!(dss.result(), "1");
        // Oracle Mean 0.633333…, StdDev 0.280475786239502.
        dss.command("? LoadShape.d.mean");
        assert!((dss.result().parse::<f64>().unwrap() - 0.633_333_333_333_333).abs() < 1e-9);
        dss.command("? LoadShape.d.stddev");
        assert!((dss.result().parse::<f64>().unwrap() - 0.280_475_786_239_502).abs() < 1e-9);

        std::fs::remove_dir_all(&dir).ok();
    }

    /// A missing CSV file is Pascal error 613 (recorded, edit continues).
    #[test]
    fn csvfile_missing_records_error() {
        use crate::exec::Dss;
        let mut dss = Dss::new();
        dss.command("clear");
        dss.command("new circuit.p");
        dss.command("New LoadShape.d npts=6 interval=1 csvfile=does_not_exist_42.csv");
        assert!(
            dss.errors()
                .iter()
                .any(|e| e.contains("Error opening file")),
            "expected a 613-style error, got {:?}",
            dss.errors()
        );
    }
}
