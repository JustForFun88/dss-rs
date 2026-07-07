//! The `LoadShape` numeric core: the `GetMultAtHour` lookup, `Normalize`,
//! `SetMaxPandQ`, the lazy mean/std-dev, and the `ReadCSVFile` parser.

use crate::support::mathutil::{
    curve_mean_and_std_dev, curve_mean_and_std_dev_single, mean_and_std_dev,
    mean_and_std_dev_single,
};
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

    /// Pascal `TLoadShapeObj.UseFloat32` (`LoadShape.pas:2158`): convert the
    /// f64 arrays to single-precision storage. Only call site (as in Pascal's
    /// script-reachable surface) is `read_sng_file` with `QMult` unset, so
    /// `q_mult` is `None` here by construction. The f64 fields keep the
    /// widened *view* of the quantized singles (Pascal frees `dH`/`dP` and
    /// widens `sH`/`sP` on every dP-shaped read; identical values).
    fn use_float32(&mut self) {
        debug_assert!(self.q_mult.is_none(), "float32 path requires dQ = NIL");
        if let Some(h) = self.hour.take() {
            let sh: Vec<f32> = h.iter().map(|&v| v as f32).collect();
            self.hour = Some(sh.iter().map(|&v| f64::from(v)).collect());
            self.s_h = Some(sh);
        }
        if let Some(p) = self.p_mult.take() {
            let sp: Vec<f32> = p.iter().map(|&v| v as f32).collect();
            self.p_mult = Some(sp.iter().map(|&v| f64::from(v)).collect());
            self.s_p = Some(sp);
        }
    }

    /// Pascal `TLoadShapeObj.UseFloat64` (`LoadShape.pas:2200`): widen single
    /// storage back to f64 and free the singles. The f64 views already hold
    /// exactly the widened values, so this just drops the f32 authority.
    pub(super) fn use_float64(&mut self) {
        self.s_p = None;
        self.s_h = None;
    }

    /// Pascal `TLoadShapeObj.GetMultAtHour`: the (P, Q) multiplier nearest the
    /// requested hour. Returns `(1, 1)` for an empty curve; repeats the curve
    /// past its last point. Ported verbatim for the double-precision,
    /// non-memory-mapped path; single-precision storage dispatches to
    /// [`Self::get_mult_at_hour_single`] first, exactly like the Pascal
    /// `if Assigned(sP)` head. MMF is out of scope.
    pub fn get_mult_at_hour(&mut self, hr: f64) -> Complex64 {
        if self.s_p.is_some() {
            return self.get_mult_at_hour_single(hr);
        }
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

    /// Pascal `TLoadShapeObj.GetMultAtHourSingle` (`LoadShape.pas:2248-2360`),
    /// the single-precision twin of `GetMultAtHour` (`Stride` = 1: external
    /// memory is API-only, not ported). Reads are f32 widened on use; the
    /// variable-interval interpolation reproduces FPC's mixed precision — the
    /// same-type `Single` differences round to f32 before the f64 divide /
    /// multiply (verified bit-exact by `tools/fpc/single_prec_probe.pas`).
    /// `sQ` is script-unreachable, so the Q side follows `Set_Result_im`
    /// everywhere except the Edge walk, where Pascal starts from `Result := 0`
    /// and only overwrites `im` when `dQ <> NIL` (never here) — `im` stays 0.
    fn get_mult_at_hour_single(&mut self, hr: f64) -> Complex64 {
        let npts = self.n();
        if npts == 0 {
            return Complex64::new(1.0, 1.0); // default for an empty curve
        }
        let Some(p) = self.s_p.as_ref() else {
            return Complex64::new(1.0, 1.0);
        };
        if p.is_empty() {
            return Complex64::new(1.0, 1.0);
        }

        if npts == 1 {
            let re = f64::from(p[0]);
            return Complex64::new(re, self.result_im(re));
        }

        // --- Fixed (even) interval ---
        if self.interval > 0.0 {
            // TODO(compat): FPC `Round` = ties-to-even (see the f64 twin).
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
            let re = f64::from(p[i as usize]);
            return Complex64::new(re, self.result_im(re));
        }

        // --- Variable interval (single-precision hour array) ---
        let mut hr = hr;
        let h = match self.s_h.as_ref() {
            Some(h) if h.len() >= npts => h.clone(),
            _ => return Complex64::new(1.0, 1.0), // degenerate; see f64 twin
        };
        let p = p.clone();

        // Normalize Hr into the first cycle (mixed f64 over widened singles).
        let last_h = f64::from(h[npts - 1]);
        if hr > last_h && last_h != 0.0 {
            hr -= (hr / last_h).trunc() * last_h;
        }

        if f64::from(h[self.last_value_accessed.min(npts - 1)]) > hr {
            self.last_value_accessed = 0; // start over from the beginning
        }

        for i in self.last_value_accessed..npts {
            if (f64::from(h[i]) - hr).abs() < 0.00001 {
                let re = f64::from(p[i]);
                self.last_value_accessed = i;
                return Complex64::new(re, self.result_im(re));
            }
            if f64::from(h[i]) > hr {
                if self.interpolation == INTERP_EDGE {
                    // Edge: `Result := 0`, hold the last point at or before
                    // Hr; `im` is only written when `dQ <> NIL` (never in
                    // single mode) so it stays 0.
                    let mut re = 0.0;
                    for k in 0..npts {
                        if f64::from(h[k]) <= hr {
                            re = f64::from(p[k]);
                        } else {
                            break;
                        }
                    }
                    return Complex64::new(re, 0.0);
                }
                // Avg: linear interpolation; i == 0 underflow is the same
                // upstream UB the f64 twin guards (first point returned).
                if i == 0 {
                    let re = f64::from(p[0]);
                    return Complex64::new(re, self.result_im(re));
                }
                self.last_value_accessed = i - 1;
                let prev = i - 1;
                // FPC mixed precision: the same-type Single differences round
                // to f32; the divide/multiply then run in f64 (probe-proven).
                let dh = f64::from(h[i] - h[prev]);
                let dp = f64::from(p[i] - p[prev]);
                let re = f64::from(p[prev]) + (hr - f64::from(h[prev])) / dh * dp;
                return Complex64::new(re, self.result_im(re));
            }
        }

        // Fell through the loop: use the last interior value.
        self.last_value_accessed = npts - 2;
        let re = f64::from(p[self.last_value_accessed]);
        Complex64::new(re, self.result_im(re))
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
        if let Some(sp) = self.s_p.as_mut() {
            // Pascal `DoNormalizeSingle`: `MaxMult` is a Double built from
            // widened singles; each `Multipliers[i] / MaxMult` runs in f64 and
            // rounds back to f32 on store. (`sQ` is script-unreachable.)
            Self::do_normalize_single(sp, npts, base_p);
            self.p_mult = Some(sp.iter().map(|&v| f64::from(v)).collect());
        } else {
            if let Some(p) = self.p_mult.as_mut() {
                Self::do_normalize(p, npts, base_p);
            }
            if let Some(q) = self.q_mult.as_mut() {
                Self::do_normalize(q, npts, base_q);
            }
        }
        self.use_actual = false;
    }

    /// Inner `DoNormalizeSingle` (see [`Self::normalize`]).
    fn do_normalize_single(mult: &mut [f32], npts: usize, mut max_mult: f64) {
        let n = npts.min(mult.len());
        if n == 0 {
            return;
        }
        if max_mult <= 0.0 {
            max_mult = f64::from(mult[0]).abs();
            for &v in &mult[1..n] {
                max_mult = max_mult.max(f64::from(v).abs());
            }
        }
        if max_mult == 0.0 {
            max_mult = 1.0; // avoid divide by zero
        }
        for v in &mut mult[..n] {
            *v = (f64::from(*v) / max_mult) as f32;
        }
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
        // Pascal's `else` (dP = NIL) branch: single-precision statistics over
        // `sP`/`sH` (`LoadShape.pas:1711-1716`).
        if let Some(sp) = self.s_p.as_ref() {
            let n = self.n().min(sp.len());
            if n == 0 {
                return (self.f_mean, self.f_std_dev);
            }
            return if self.interval > 0.0 {
                mean_and_std_dev_single(&sp[..n])
            } else {
                match self.s_h.as_ref() {
                    Some(sh) if sh.len() >= n => curve_mean_and_std_dev_single(&sp[..n], &sh[..n]),
                    _ => mean_and_std_dev_single(&sp[..n]),
                }
            };
        }
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
        // Pascal `ReadCSVFile` runs `UseFloat64` first (`LoadShape.pas:1044`):
        // a CSV read ends any live single-precision storage before it
        // overwrites `dP`/`dH` (audit follow-up — without this a stale `sP`
        // from an earlier `sngfile=` would keep winning the lookup).
        self.use_float64();
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
        // Pascal `Read2ColCSVFile` runs `UseFloat64` first (`LoadShape.pas:970`).
        self.use_float64();
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

    /// Pascal `TLoadShapeObj.ReadSngFile` (little-endian `f32` stream,
    /// `LoadShape.pas:1082-1180`). For a variable interval (`Interval = 0`)
    /// each point is an `(hour, mult)` pair; otherwise a bare `mult` stream.
    ///
    /// Two Pascal paths, both ported: with `QMult` unset (`dQ = NIL`) the
    /// **float32 path** runs `UseFloat32` and stores into `sP`/`sH` — single
    /// precision stays authoritative for the lookup/statistics/normalize; once
    /// `QMult` is set the **float64 path** stores into `dP`/`dH`, widening each
    /// `Single` on assignment, and shrinks `NumPoints` to the count read.
    /// One deliberate divergence — greppable:
    /// NOT_PORTED(LoadShape float32 truncated-pair shrink): Pascal's float32
    /// `Interval=0` loop (`:1129-1134`) breaks on a short read **without**
    /// shrinking `NumPoints`, leaving the tail of `sH`/`sP` as uninitialized
    /// heap (`ReallocMem` does not zero) — nondeterministic upstream UB, NOT
    /// reproduced per the CLAUDE.md rule; the port shrinks like the float64
    /// path, so a truncated file yields the defined prefix instead of garbage.
    pub(super) fn read_sng_file(&mut self, content: &[u8]) {
        let npts = self.n();
        if self.q_mult.is_none() {
            // Float32 path: "Take the opportunity to use float32 data".
            self.use_float32();
            if self.interval == 0.0 {
                let mut sh = Vec::with_capacity(npts);
                let mut sp = Vec::with_capacity(npts);
                let mut off = 0usize;
                while sh.len() < npts && off + 8 <= content.len() {
                    sh.push(f32::from_le_bytes(
                        content[off..off + 4].try_into().unwrap(),
                    ));
                    sp.push(f32::from_le_bytes(
                        content[off + 4..off + 8].try_into().unwrap(),
                    ));
                    off += 8;
                }
                self.num_points = sh.len() as i32;
                self.hour = store_array(sh.iter().map(|&v| f64::from(v)).collect());
                self.p_mult = store_array(sp.iter().map(|&v| f64::from(v)).collect());
                self.s_h = if sh.is_empty() { None } else { Some(sh) };
                self.s_p = if sp.is_empty() { None } else { Some(sp) };
            } else {
                let n = (content.len() / 4).min(npts);
                let sp: Vec<f32> = content[..n * 4]
                    .chunks_exact(4)
                    .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
                    .collect();
                self.num_points = n as i32;
                self.p_mult = store_array(sp.iter().map(|&v| f64::from(v)).collect());
                self.s_p = if sp.is_empty() { None } else { Some(sp) };
            }
            return;
        }

        // Float64 path (QMult already set): widen each Single on assignment.
        self.use_float64();
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
        // Pascal `ReadDblFile` runs `UseFloat64` first (`LoadShape.pas:1220`).
        self.use_float64();
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
