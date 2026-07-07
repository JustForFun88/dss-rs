//! The `LoadShape` numeric core: the `GetMultAtHour` lookup, `Normalize`,
//! `SetMaxPandQ`, the lazy mean/std-dev, and the `ReadCSVFile` parser.

use crate::support::mathutil::{curve_mean_and_std_dev, mean_and_std_dev};
use dss_parser::{Parser, ParserVars};
use num_complex::Complex64;

use super::{INTERP_EDGE, LoadShapeObj, store_array};

impl LoadShapeObj {
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
    pub(super) fn set_max_p_and_q(&mut self) {
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
    pub(super) fn normalize(&mut self, errors: &mut Vec<String>) {
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

    /// Pascal `TLoadShapeObj.Get_Mean` (lazy `CalcMeanandStdDev`); crate-wide
    /// for the Solution dump's `Set %mean=` line (`Solution.pas:1814`).
    pub(crate) fn mean(&self) -> f64 {
        if self.std_dev_calculated {
            self.f_mean
        } else {
            self.calc_mean_std().0
        }
    }

    /// Pascal `TLoadShapeObj.Get_StdDev`; crate-wide for the Solution dump's
    /// `Set %stddev=` line (`Solution.pas:1815`).
    pub(crate) fn std_dev(&self) -> f64 {
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
    pub(super) fn read_csv_file(&mut self, content: &str) {
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

    /// Pascal `TLoadShapeObj.Read2ColCSVFile` (`PQCSVFile`, double, non-MMF
    /// path): each row is `P, Q` (or `hour, P, Q` when `Interval = 0`). Reads at
    /// most `NumPoints` rows and shrinks `NumPoints` to the count actually read.
    pub(super) fn read_pq_csv_file(&mut self, content: &str) {
        let npts = self.n();
        let variable = self.interval == 0.0;
        let mut p = vec![0.0; npts];
        let mut q = vec![0.0; npts];
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
            parser.next_param(&vars);
            q[i] = parser.make_double(&vars).unwrap_or(0.0);
            i += 1;
        }

        p.truncate(i);
        q.truncate(i);
        self.p_mult = store_array(p);
        self.q_mult = store_array(q);
        if variable {
            h.truncate(i);
            self.hour = store_array(h);
        }
        self.num_points = i as i32;
    }

    /// Pascal `TLoadShapeObj.ReadSngFile` (little-endian `f32` stream). For a
    /// variable interval (`Interval = 0`) each point is an `(hour, mult)` pair;
    /// otherwise a bare `mult` stream. Reads at most `NumPoints` points and
    /// shrinks `NumPoints` to the count actually read.
    ///
    /// Pascal keeps two code paths here — a "float32" path taken when `QMult`
    /// is not yet set (stores into `sP`/`sH`, single precision throughout) and
    /// a "float64" path taken once `QMult` is set (stores into `dP`/`dH`
    /// directly, widening each `Single` read on assignment). Both read the
    /// identical bytes; they differ only in a rare edge case (a *truncated*
    /// variable-interval file on the float32 path skips the `NumPoints` shrink
    /// Pascal applies everywhere else) that no corpus deck exercises. This port
    /// always widens straight into the f64 `p_mult`/`hour` arrays and always
    /// shrinks `NumPoints` to the count read (the float64 path's behavior),
    /// bit-identical to the float32 path for every non-truncated read (the
    /// widen is a lossless `f32 -> f64` cast either way, and no corpus curve
    /// interpolates a single-precision-only shape — the one place float32 vs.
    /// float64 *arithmetic* could diverge).
    pub(super) fn read_sng_file(&mut self, content: &[u8]) {
        let npts = self.n();
        if self.interval == 0.0 {
            let mut h = Vec::with_capacity(npts);
            let mut p = Vec::with_capacity(npts);
            let mut off = 0usize;
            while h.len() < npts && off + 8 <= content.len() {
                let hr = f32::from_le_bytes(content[off..off + 4].try_into().unwrap());
                let m = f32::from_le_bytes(content[off + 4..off + 8].try_into().unwrap());
                h.push(hr as f64);
                p.push(m as f64);
                off += 8;
            }
            self.num_points = h.len() as i32;
            self.hour = store_array(h);
            self.p_mult = store_array(p);
        } else {
            let n = (content.len() / 4).min(npts);
            let p: Vec<f64> = content[..n * 4]
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes(c.try_into().unwrap()) as f64)
                .collect();
            self.num_points = n as i32;
            self.p_mult = store_array(p);
        }
    }

    /// Pascal `TLoadShapeObj.ReadDblFile` (little-endian `f64` stream); same
    /// row layout as [`Self::read_sng_file`] but always double precision (no
    /// float32/float64 branch in Pascal here).
    pub(super) fn read_dbl_file(&mut self, content: &[u8]) {
        let npts = self.n();
        if self.interval == 0.0 {
            let mut h = Vec::with_capacity(npts);
            let mut p = Vec::with_capacity(npts);
            let mut off = 0usize;
            while h.len() < npts && off + 16 <= content.len() {
                let hr = f64::from_le_bytes(content[off..off + 8].try_into().unwrap());
                let m = f64::from_le_bytes(content[off + 8..off + 16].try_into().unwrap());
                h.push(hr);
                p.push(m);
                off += 16;
            }
            self.num_points = h.len() as i32;
            self.hour = store_array(h);
            self.p_mult = store_array(p);
        } else {
            let n = (content.len() / 8).min(npts);
            let p: Vec<f64> = content[..n * 8]
                .chunks_exact(8)
                .map(|c| f64::from_le_bytes(c.try_into().unwrap()))
                .collect();
            self.num_points = n as i32;
            self.p_mult = store_array(p);
        }
    }
}
