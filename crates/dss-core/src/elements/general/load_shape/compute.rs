//! The `LoadShape` numeric core: the `GetMultAtHour` lookup, `Normalize`,
//! `SetMaxPandQ`, the lazy mean/std-dev, and the `ReadCSVFile` parser.

use crate::support::mathutil::{
    curve_mean_and_std_dev, curve_mean_and_std_dev_single, mean_and_std_dev,
    mean_and_std_dev_single,
};
use dss_parser::{Parser, ParserVars};
use num_complex::Complex64;

use crate::obj::base::{InterpLoad, InterpTarget, MmfKind};

use super::{LoadShapeInterp, LoadShapeObj, store_array};

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
            // Pascal `Round` = ties-to-even (see RegControl `get_tap_num`).
            // For every index this can produce from a well-formed shape the two
            // agree bit-for-bit; a degenerate `interval` small enough to push
            // `hr/interval` out of Int64 range is upstream UB either way (FPC's
            // indefinite sentinel then indexes the array out of bounds), so
            // there is nothing defined to reproduce — the port saturates and
            // the bounds check below is real.
            let mut i = if self.interpolation == LoadShapeInterp::Edge {
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
                if self.interpolation == LoadShapeInterp::Edge {
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
            // Pascal `Round` = ties-to-even (see the f64 twin above).
            let mut i = if self.interpolation == LoadShapeInterp::Edge {
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
                if self.interpolation == LoadShapeInterp::Edge {
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

    /// Pascal `TLoadShapeObj.Mult(i)` (`LoadShape.pas:1756`): the P multiplier
    /// at 1-based index `i`, updating `LastValueAccessed` (this struct's hunt
    /// cache is already 0-based, matching Pascal's `dec(i)` before indexing) —
    /// used by `SolveLD1`/`SolveLD2` to walk the load-duration curve. MMF is
    /// out of scope (module doc); the f64/f32 storage split needs no branch
    /// here because `p_mult` already holds the widened f32 view whenever
    /// single-precision storage is authoritative, covering both of Pascal's
    /// `dP <> nil` / `else sP` arms with one read.
    pub fn mult(&mut self, i: i32) -> f64 {
        let idx = i - 1;
        let npts = self.n();
        if idx < 0 || (idx as usize) >= npts {
            return 0.0;
        }
        let off = idx as usize;
        let re = self
            .p_mult
            .as_ref()
            .and_then(|p| p.get(off))
            .copied()
            .unwrap_or(0.0);
        self.last_value_accessed = off;
        re
    }

    /// Pascal `TLoadShapeObj.PresentInterval` / `Get_Interval`
    /// (`LoadShape.pas:1724`): the fixed `Interval` if set, else the gap
    /// between the last two `Mult`-walked variable-interval hour points
    /// (`0.0` before at least two points have been walked). Consumed by
    /// `SolveLD1`/`SolveLD2` (`ckt.LoadDurCurveObj.PresentInterval`).
    pub fn present_interval(&self) -> f64 {
        if self.interval > 0.0 {
            return self.interval;
        }
        if self.last_value_accessed <= 1 {
            return 0.0;
        }
        let Some(h) = self.hour.as_ref() else {
            return 0.0;
        };
        match (
            h.get(self.last_value_accessed),
            h.get(self.last_value_accessed - 1),
        ) {
            (Some(&hi), Some(&hprev)) => hi - hprev,
            _ => 0.0,
        }
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

    /// Pascal `TLoadShapeObj.SetMaxPandQ` (`LoadShape.pas:2048`): peak P and
    /// the coincident Q. Under MMF (or external memory, not modeled) Pascal
    /// exits FIRST, leaving `MaxP`/`MaxQ` at the constructor defaults `1.0`/
    /// `0.0` — oracle-confirmed `pmax=1, qmax=0` for every MMF shape (audit
    /// settlement 2026-07-09: the dropped guard mis-scaled `useactual` loads).
    pub(super) fn set_max_p_and_q(&mut self) {
        if self.use_mmf {
            return;
        }
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

    /// Queue a `SngSave`/`DblSave` binary write (Pascal `TLoadShapeObj.
    /// SaveToDblFile`/`SaveToSngFile`, `LoadShape.pas:1880/1939`). Mirrors the
    /// Pascal head: `UseFloat64` first, then the `if not Assigned(dP)` guard
    /// (`DoSimpleMsg` 622/623), then snapshot `dP` — and `dQ` when `Assigned(dQ)`
    /// — into a [`ShapeSave`] for the executive (which owns `OutputDirectory` /
    /// `GlobalResult`). LoadShape uses the `_P`/`_Q` filename split.
    pub(super) fn queue_shape_save(&mut self, sng: bool, errors: &mut crate::diag::ErrorLog) {
        // Pascal `UseFloat64` (LoadShape.pas:1888/1946): ensure the f64 arrays.
        self.use_float64();
        // MMF (`MemoryMapping=Yes`): Pascal re-reads each value at save time via
        // `InterpretDblArrayMMF` (P `:1898-1905`/`:1956-1963`, Q `:1921-1927`/
        // `:1982-1988`). This port already eagerly read the whole MMF file into
        // `p_mult` (and `q_mult` when a `qmult=` MMF directive was given) at
        // directive time (`read_mmf_raw`/`finish_mmf`, `:952-963`) using the
        // identical record semantics, so the non-MMF snapshot below emits the
        // same bytes and `q_mult.is_some()` matches Pascal `Assigned(dQ)`
        // (`CustomSetRaw` `:791-802` allocates a 2-elem `dQ` sentinel iff a
        // `qmult=` MMF directive was given). Oracle-probed 2026-07-11
        // (dss-python 0.15.7): Case A (P sng-src + `qmult` sng-src) → `_P`+`_Q`
        // bytes = the f32-narrowed values; Case B (no `qmult`) → only `_P`, no
        // `_Q` file, `GlobalResult` has no `Qmult=` clause. No separate MMF path
        // needed — the guard is gone.
        let n = self.n();
        // Pascal `if not Assigned(dP)` → `DoSimpleMsg('%s P multipliers not
        // defined.', [FullName], 622/623)` then `Exit`.
        let Some(p) = self.p_mult.as_ref() else {
            errors.push(crate::diag::DssDiagnostic::msg(
                format!("LoadShape.{} P multipliers not defined.", self.data.name()),
                // SaveToSngFile → 623, SaveToDblFile → 622 (LoadShape.pas:1891/1949).
                Some(if sng { 623 } else { 622 }),
            ));
            return;
        };
        let values: Vec<f64> = p.iter().take(n).copied().collect();
        // Q file only `if Assigned(dQ)`.
        let q_values = self
            .q_mult
            .as_ref()
            .map(|q| q.iter().take(n).copied().collect::<Vec<f64>>());
        self.pending_shape_saves.push(crate::obj::base::ShapeSave {
            name: self.data.name().to_string(),
            sng,
            values,
            q_values,
            p_suffix: true,
            result_tag: "mult",
        });
    }

    /// Pascal `TLoadShapeObj.Normalize`: scale the multipliers so the peak (or
    /// `BaseP`/`BaseQ` if set) becomes 1.0.
    pub(super) fn normalize(&mut self, errors: &mut crate::diag::ErrorLog) {
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
    fn has_data(&mut self, errors: &mut crate::diag::ErrorLog) -> bool {
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

    /// Pascal `TLoadShapeObj.Set_Mean` (`LoadShape.pas:2082`): override the
    /// stored mean and mark the statistics as computed (`Set %mean=` on the
    /// default daily shape).
    pub(crate) fn set_mean(&mut self, value: f64) {
        self.std_dev_calculated = true;
        self.f_mean = value;
    }

    /// Pascal `TLoadShapeObj.Set_StdDev` (`LoadShape.pas:2088`): override the
    /// stored standard deviation (`Set %stddev=`).
    pub(crate) fn set_std_dev(&mut self, value: f64) {
        self.std_dev_calculated = true;
        self.f_std_dev = value;
    }

    /// Pascal `TLoadShapeObj.ReadCSVFile` (double-precision, non-MMF path): one
    /// row per point, parsed with the comma/whitespace aux parser. For a fixed
    /// interval each row is a single P multiplier; for a variable interval
    /// (`Interval = 0`) each row is `hour, mult`. Reads at most `NumPoints` rows
    /// and shrinks `NumPoints` to the count actually read.
    pub(super) fn read_csv_file(&mut self, content: &str) {
        // Pascal `ReadCSVFile` MMF branch (`LoadShape.pas:1031-1041`): map the
        // whole file (`file=`+name, column 1) and eager-read via the text
        // accept-set. Single-column text under MMF divides by zero in the
        // pinned 0.14.5 oracle's lazy byte reader (#482) — the c4590d16 fix
        // (D13) restored the missing `not` on the CreateMMF guard; the eager
        // reader already loads the P data the fixed engine loads. Gated capi015
        // (`modes/upgrade/mmf_singlecol`) + unit
        // `mmf_single_column_csvfile_loads_like_capi015`.
        if self.use_mmf {
            let npts = self.n();
            let vals = self.mmf_read_text(content, 1, npts);
            self.finish_mmf(vals, false);
            return;
        }
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
        // Pascal `Read2ColCSVFile` MMF branch (`LoadShape.pas:950-966`): P from
        // column 1, Q from column 2 of the same mapped view. Read both before
        // finishing so a short-file shrink does not perturb the Q pass.
        if self.use_mmf {
            let npts = self.n();
            let p = self.mmf_read_text(content, 1, npts);
            let q = self.mmf_read_text(content, 2, npts);
            self.finish_mmf(p, false);
            self.finish_mmf(q, true);
            return;
        }
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
        // Pascal `ReadSngFile` MMF branch (`LoadShape.pas:1103-1113`): map the
        // whole file (`sngfile=`+name) and read `npts` little-endian f32,
        // widened into `dP` (f64) — NOT the `sP`/`GetMultAtHourSingle` path.
        if self.use_mmf {
            let npts = self.n();
            let vals = self.mmf_read_f32(content, npts);
            self.finish_mmf(vals, false);
            return;
        }
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
        // Pascal `ReadDblFile` MMF branch (`LoadShape.pas:1207-1217`): map the
        // whole file (`dblfile=`+name) and read `npts` little-endian f64.
        if self.use_mmf {
            let npts = self.n();
            let vals = self.mmf_read_f64(content, npts);
            self.finish_mmf(vals, false);
            return;
        }
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

    // ------------------------------------------------------------------
    // Memory-mapped-file (`MemoryMapping=Yes`) eager readers (WPG.17).
    // Pascal maps the file and reads records lazily through the map in
    // `GetMultAtHour`; this port reads the whole file up front into the f64
    // `dP`/`dQ` arrays with the identical record semantics
    // (`InterpretDblArrayMMF`, `LoadShape.pas:1343-1418`), so the existing f64
    // lookup is then correct with `sP = NIL`. Fixed-interval only (every corpus
    // deck is `interval=1`); MMF + variable interval never populates the hour
    // array upstream (a degenerate path) and is not exercised — see mod.rs.
    // ------------------------------------------------------------------

    /// Read `min(npts, available)` little-endian f32 records, widened to f64.
    fn mmf_read_f32(&self, bytes: &[u8], npts: usize) -> Vec<f64> {
        let k = (bytes.len() / 4).min(npts);
        bytes[..k * 4]
            .chunks_exact(4)
            .map(|c| f64::from(f32::from_le_bytes(c.try_into().unwrap())))
            .collect()
    }

    /// Read `min(npts, available)` little-endian f64 records.
    fn mmf_read_f64(&self, bytes: &[u8], npts: usize) -> Vec<f64> {
        let k = (bytes.len() / 8).min(npts);
        bytes[..k * 8]
            .chunks_exact(8)
            .map(|c| f64::from_le_bytes(c.try_into().unwrap()))
            .collect()
    }

    /// Read up to `npts` text records at the given 1-based comma column via the
    /// Pascal MMF accept-set (`InterpretDblArrayMMF` PlainText). One line per
    /// record (equivalent to Pascal's fixed-width byte indexing for the uniform
    /// files it requires; non-uniform widths are upstream UB, not reproduced).
    fn mmf_read_text(&mut self, content: &str, column: i32, npts: usize) -> Vec<f64> {
        let mut out = Vec::with_capacity(npts);
        let mut lines = content.lines();
        // The same aux parser the non-mapped twin uses, built once for the whole
        // read (Pascal's `DSS.AuxParser` is a long-lived singleton too).
        let mut parser = Parser::new();
        parser.set_auto_increment(false);
        let vars = ParserVars::new();
        for _ in 0..npts {
            let Some(line) = lines.next() else { break };
            out.push(self.mmf_text_value(&mut parser, &vars, line, column));
        }
        out
    }

    /// Pascal `InterpretDblArrayMMF` PlainText record (`LoadShape.pas:1361-
    /// 1400`): take one comma-delimited column of the record and parse it.
    ///
    /// The column is taken **verbatim** and handed to the ordinary aux float
    /// parser — the same `Parser`/`make_double` pair the non-mapped twin uses
    /// ([`Self::read_csv_file`], Pascal `ReadCSVFile`'s `:1044` branch, which
    /// feeds each row to `DSS.AuxParser`). Both oracle revisions instead filter
    /// the column through an accept-set of bytes in `[46, 58)` —
    /// `.` (46), `/` (47) and the digits `0`–`9` (48–57) — pinned dss_capi
    /// 0.14.5 `src/General/LoadShape.pas:1374` and EPRI r4133
    /// `Version8/Source/Common/Utilities.pas:834`, the same line. That drops the
    /// sign (45), `+` (43), the exponent letter `e`/`E` (101/69) and whitespace
    /// while keeping `/` inside the number, so `-0.5` reads as `0.5`, `1.5e-3`
    /// as `1.53`, and `1/2` reaches `strtofloat` as a malformed token.
    ///
    /// It is a slip in the mapped reader, not a second file format, and the
    /// witness is the class itself: `TLoadShapeObj` owns two readers for the
    /// *same* file, and only the mapped one deletes those characters.
    /// `MemoryMapping=Yes` selects how a shape is stored, not what its bytes
    /// mean, so the two must agree byte-for-byte on the same input —
    /// `tests::mmf_text_reader_agrees_with_its_non_mapped_twin` pins exactly
    /// that, over content carrying a sign, an exponent and an explicit `+`.
    ///
    /// Fixed in both lanes (CLAUDE.md 2026-08-02: upstream bugs are never
    /// reproduced) by `GOLDEN_REBASE_PLAN.md` G2.5. Two consequences worth
    /// naming: `tests/corpus/modes/inputformat/shape_mmf/shape_mmf.dss` exists
    /// *to observe* the quirk (its `mmpq8.csv` P column is written in exponent
    /// notation on purpose), so it now diverges from the `capi_v0145` oracle
    /// across the whole run — ledgered as `mmf-accept-set-honoured-capi`, with
    /// the deck's unrelated sng/dbl/`mult=(sngfile=)` MMF-reader coverage moved
    /// to the sibling deck `shape_mmf_io.dss`; and the vendored MMF text corpus
    /// (`Examples/MemoryMappingLoadShapes/ckt24`) cannot see the change at all,
    /// its files holding nothing but digits, `.`, `,` and newlines, which
    /// `tests::mmf_accept_set_fix_is_gated_by_exactly_one_deck` keeps measuring.
    ///
    /// The empty-column default of `1.0` (`:1389-1390`, r4133
    /// `Utilities.pas:845`) is a deliberate default return value, not part of
    /// the accept-set slip, and is kept.
    fn mmf_text_value(
        &mut self,
        parser: &mut Parser,
        vars: &ParserVars,
        line: &str,
        column: i32,
    ) -> f64 {
        let mut content = String::new();
        let mut j = 0i32;
        for &b in line.as_bytes() {
            if b == 0x0A {
                break; // lines() already strips this; kept for byte-faithfulness
            }
            if b == 44 {
                // a comma: advance the column counter, stop at the target column
                j += 1;
                if j == column {
                    break;
                }
                content.clear();
                continue;
            }
            content.push(b as char);
        }
        let token = content.trim();
        if token.is_empty() {
            return 1.0;
        }
        parser.set_cmd_string(token);
        parser.next_param(vars);
        match parser.make_double(vars) {
            Ok(v) => v,
            Err(_) => {
                // NOT_PORTED(InterpretDblArrayMMF error-785 byte-offset return):
                // Pascal returns `i - 1` (a heap byte index) on a `strtofloat`
                // failure (`:1396`) — a defined-but-nonsensical value, still
                // reachable here on a genuinely malformed token. Not reproduced
                // (UB-adjacent, unreachable from the corpus); surface a real
                // error and fall back to the `1.0` default instead.
                self.data.push_error(format!(
                    "LoadShape.{}: invalid numeric token \"{token}\" in a \
                     memory-mapped text file.",
                    self.data.name()
                ));
                1.0
            }
        }
    }

    /// Store an eager MMF read into `dP` (P side) or `dQ` (Q side), ending any
    /// single-precision storage (`sP = NIL`, so the f64 lookup wins) and
    /// leaving `NumPoints` unchanged for a complete file (Pascal never shrinks
    /// under MMF, `:761`). A short file is upstream UB (Pascal reads past the
    /// map); the port clamps to the records present with a loud diagnostic —
    /// the P side then shrinks `NumPoints` to keep the array/`npts` invariant
    /// the lookup relies on (the Q lookup is length-guarded, so Q does not).
    fn finish_mmf(&mut self, values: Vec<f64>, qside: bool) {
        let npts = self.n();
        let short = values.len() < npts;
        if short {
            self.data.push_error(format!(
                "LoadShape.{}: memory-mapped file has fewer records ({}) than \
                 npts ({npts}); using the {} present (upstream reads past the map).",
                self.data.name(),
                values.len(),
                values.len()
            ));
        }
        if qside {
            self.q_mult = store_array(values);
        } else {
            self.s_p = None;
            self.s_h = None;
            if short {
                self.num_points = values.len() as i32;
            }
            self.p_mult = store_array(values);
        }
    }

    /// Apply a raw MMF array directive (`mult=(sngfile=…)` / `qmult=(file=…)`,
    /// Pascal `CustomSetRaw` MMF branches, `LoadShape.pas:756-800`). The kind /
    /// column / P-vs-Q side were parsed from the directive at set time.
    pub(super) fn read_mmf_raw(&mut self, bytes: &[u8], kind: MmfKind, column: i32, qside: bool) {
        let npts = self.n();
        let values = match kind {
            MmfKind::Float32 => self.mmf_read_f32(bytes, npts),
            MmfKind::Float64 => self.mmf_read_f64(bytes, npts),
            MmfKind::Text => {
                let content = String::from_utf8_lossy(bytes);
                self.mmf_read_text(&content, column, npts)
            }
        };
        self.finish_mmf(values, qside);
    }

    /// Apply a non-memory-mapped `InterpretDblArray` LoadShape directive
    /// (`mult=(file=…)` / `qmult=(sngfile=…)` / `hour=(dblfile=…)`, Pascal
    /// `CustomSetRaw`, `LoadShape.pas:749-810`, WPG.19). Runs `UseFloat64` first
    /// (Pascal `:767/779/804`), reads with the `Utilities.pas` file grammar
    /// capped at the current `NumPoints`, then stores per the Pascal shrink rule:
    /// `mult`/`Pmult` shrink `NumPoints := result` (`:770`); `qmult`/`hour` leave
    /// it unchanged (the return is ignored, `:781/806`). A short `qmult`/`hour`
    /// file leaves an uninitialized tail upstream (`ReAllocmem` UB, not
    /// reproduced per CLAUDE.md): we store only the prefix read.
    pub(super) fn apply_interp_file(&mut self, il: &InterpLoad, bytes: &[u8]) {
        self.use_float64();
        let max = self.n();
        let values = match il.kind {
            MmfKind::Text => {
                let content = String::from_utf8_lossy(bytes);
                let (vals, err_row) =
                    crate::util::read_dbl_array_text(&content, il.column, il.header, max);
                if let Some(row) = err_row {
                    // Pascal `DoSimpleMsg(#705)` then stop-and-shrink
                    // (`Utilities.pas:515-521`); `vals` already holds only `i-1`.
                    self.data.push_error(crate::diag::DssDiagnostic::msg(
                        format!(
                            "LoadShape.{}: (#705) Error reading {row}-th numeric array \
                             value from file.",
                            self.data.name()
                        ),
                        Some(705),
                    ));
                }
                vals
            }
            MmfKind::Float32 => crate::util::read_le_f32_array(bytes, max),
            MmfKind::Float64 => crate::util::read_le_f64_array(bytes, max),
        };
        let count = values.len();
        match il.target {
            InterpTarget::PMult => {
                self.p_mult = store_array(values);
                self.num_points = count as i32;
            }
            InterpTarget::QMult => {
                self.q_mult = store_array(values);
            }
            InterpTarget::Hour => {
                self.hour = store_array(values);
            }
        }
    }
}
