//! The electrical core: `RecalcElementData` (derived per-winding data, the
//! Series-winding `kVSeries`/`VBase`), `CalcY_Terminal` (the `2·NumWindings`
//! terminal admittance with the auto corrections `ZCorrected`/`puXst`),
//! `GICBuildYTerminal` (the `Frequency < 0.51` resistance-only branch), and the
//! winding-current readouts.

use num_complex::Complex64;

use crate::support::cmatrix::CMatrix;
use crate::util::{EPSILON, inv_sqrt3_x1000, sqrt3};

use super::{AutoTrans, xsc_size};

/// Pascal `ZeroTapFix`: a 0 pu tap (which RegControl can force) becomes 0.0001.
fn zero_tap_fix(tap: f64) -> f64 {
    if tap == 0.0 { 0.0001 } else { tap }
}

impl AutoTrans {
    /// Pascal `TAutoTransObj.RecalcElementData` (`AutoTrans.pas:919`): derived
    /// per-winding data (`DeltaDirection`, `TermRef`, `puXSC` from XHX,
    /// per-winding `VBase` including the Series `kVSeries`, `Rdc`, anti-float,
    /// `NormAmps`/`EmergAmps`) then `CalcY_Terminal` at base frequency.
    pub(super) fn recalc(&mut self) {
        // Determine Delta Direction (the Series arm is `Auto` → 1).
        if self.windings[0].connection == self.windings[1].connection {
            self.delta_direction = 1;
        } else if self.windings[0].connection == 2 {
            self.delta_direction = 1; // Auto
        } else {
            let ihv = if self.windings[0].kvll >= self.windings[1].kvll {
                1
            } else {
                2
            };
            match self.windings[ihv - 1].connection {
                0 => self.delta_direction = if self.hv_leads_lv { -1 } else { 1 },
                1 => self.delta_direction = if self.hv_leads_lv { 1 } else { -1 },
                _ => {}
            }
        }

        self.set_term_ref();

        for w in &mut self.windings {
            w.tap_increment = if w.num_taps > 0 {
                (w.max_tap - w.min_tap) / w.num_taps as f64
            } else {
                0.0
            };
        }

        if self.xhx_changed {
            if self.num_windings <= 3 {
                let n = xsc_size(self.num_windings);
                let vals = [self.puxhx, self.puxht, self.puxxt];
                for (i, v) in vals.iter().enumerate().take(n) {
                    self.xsc[i] = *v;
                }
            }
            self.xhx_changed = false;
        }

        // Winding voltage bases (volts). The Series winding derives `kVSeries`.
        let np = self.cd.nphases;
        let w2_kvll = if self.windings.len() >= 2 {
            self.windings[1].kvll
        } else {
            0.0
        };
        let mut kv_series = self.kv_series;
        for w in &mut self.windings {
            match w.connection {
                0 => {
                    // Wye — assume 3-phase for the 2-phase designation.
                    w.vbase = if np == 2 || np == 3 {
                        w.kvll * inv_sqrt3_x1000()
                    } else {
                        w.kvll * 1000.0
                    };
                }
                1 => w.vbase = w.kvll * 1000.0, // Delta
                2 => {
                    // Series winding for the auto (should be winding 1).
                    kv_series = if np == 2 || np == 3 {
                        (w.kvll - w2_kvll) / sqrt3()
                    } else {
                        w.kvll - w2_kvll
                    };
                    if kv_series == 0.0 {
                        kv_series = w.kvll * 0.0001; // series same voltage as common
                    }
                    w.vbase = kv_series * 1000.0;
                }
                _ => {}
            }
        }
        self.kv_series = kv_series;

        // Base rating of winding 1.
        self.vabase = self.windings[0].kva * 1000.0;
        let vabase = self.vabase;

        // Rdc per winding.
        for w in &mut self.windings {
            if w.rdc_specified {
                w.rdcpu = w.rdcohms / (w.vbase * w.vbase / vabase);
            } else {
                w.rdcpu = 0.85 * w.rpu; // 85 % of the ac value (no stray loss)
                w.rdcohms = w.rdcpu * w.vbase * w.vbase / vabase;
            }
        }

        let ppm = self.ppm_float_factor;
        let vabase_1ph = vabase / np as f64;
        for w in &mut self.windings {
            w.compute_anti_float_adder(ppm, vabase_1ph);
        }

        // Normal/Emergency terminal current rating (UE check).
        let w1 = &self.windings[0];
        let vfactor = match w1.connection {
            0 => w1.vbase * 0.001, // wye
            1 => match np {
                1 => w1.vbase * 0.001,
                2 | 3 => w1.vbase * 0.001 / sqrt3(),
                _ => w1.vbase * 0.001 * 0.5 / (std::f64::consts::PI / np as f64).sin(),
            },
            2 => w1.vbase * 0.001, // series
            _ => 1.0,
        };
        self.norm_amps = self.norm_max_hkva / np as f64 / vfactor;
        self.emerg_amps = self.emerg_max_hkva / np as f64 / vfactor;

        self.calc_y_terminal(1.0);
    }

    /// Pascal `TAutoTransObj.CalcY_Terminal` (`AutoTrans.pas:1856`): build the
    /// `2·NumWindings` terminal admittance (`Y_Term`) and its no-load companion
    /// (`Y_Term_NL`). Below `0.51 Hz` (the GIC/dc branch) it delegates to
    /// [`Self::gic_build_y_terminal`]; otherwise it builds `ZB` with the auto
    /// corrections — the series diagonal scaled by `ZCorrected = ZBase·(1 +
    /// Vc/Vs)²` (Dommel 6.45) and the 3-winding `puXst` (Dommel 6.50).
    pub(super) fn calc_y_terminal(&mut self, freq_mult: f64) {
        // Pascal checks `ActiveCircuit.Solution.Frequency < 0.51`; the absolute
        // solution frequency is `FreqMult · BaseFrequency` (CalcYPrim derives
        // `FreqMult := Solution.Frequency / BaseFrequency`).
        if freq_mult * self.cd.base_frequency < 0.51 {
            self.gic_build_y_terminal();
            self.y_terminal_freqmult = freq_mult;
            return;
        }

        let nw = self.num_windings.max(0) as usize;
        let rmult = if self.xrconst { freq_mult } else { 1.0 };

        // ZBMatrix (order NumWindings-1), pu → ohms on a one-volt base.
        let mut zb = CMatrix::new(nw - 1);
        let zbase = 1.0 / (self.vabase / self.cd.nphases as f64);
        // Series correction (Dommel 6.45 for Zsc / puXSC[1]).
        let zcorrected = zbase * (1.0 + self.windings[1].vbase / self.windings[0].vbase).powi(2);
        // 3-winding derived `puXst` (Dommel 6.50 for Zst / puXSC[2]).
        let puxst = if nw > 2 {
            let vc = self.windings[1].vbase;
            let vs = self.windings[0].vbase;
            self.xsc[0] * (vs + vc) * vc / vs / vs + self.xsc[1] * (vs + vc) / vs
                - self.xsc[2] * vc / vs
        } else {
            0.0
        };

        for i in 0..nw - 1 {
            let r = rmult * (self.windings[0].rpu + self.windings[i + 1].rpu);
            let v = if i == 0 {
                Complex64::new(r, freq_mult * self.xsc[0]) * zcorrected
            } else if i == 1 {
                Complex64::new(r, freq_mult * puxst) * zbase
            } else {
                Complex64::new(r, freq_mult * self.xsc[i]) * zbase
            };
            zb.set(i, i, v);
        }
        // Off diagonals (running XSC index `k`, Pascal starts at NumWindings).
        let mut k = nw - 1;
        for i in 0..nw - 1 {
            for j in (i + 1)..(nw - 1) {
                let term = Complex64::new(
                    rmult * (self.windings[i + 1].rpu + self.windings[j + 1].rpu),
                    freq_mult * self.xsc[k],
                ) * zbase;
                let v = (zb.get(i, i) + zb.get(j, j) - term) * 0.5;
                zb.set(i, j, v);
                zb.set(j, i, v);
                k += 1;
            }
        }

        if zb.invert().is_err() {
            // Pascal error 117: replace with a tiny conductance to ground.
            zb.clear();
            for i in 0..zb.order() {
                zb.set(i, i, Complex64::new(EPSILON, 0.0));
            }
        }

        // Y_1Volt = AT · ZB⁻¹ · A (the N-1×N incidence). One phase, wye, 1 V.
        let mut y1 = CMatrix::new(nw);
        let mut y1nl = CMatrix::new(nw);
        let mut at = CMatrix::new(nw);
        for ip in 1..nw {
            at.set(ip, ip - 1, Complex64::new(1.0, 0.0));
            at.set(0, ip - 1, Complex64::new(-1.0, 0.0));
        }
        let mut a = vec![Complex64::ZERO; nw];
        let mut t1 = vec![Complex64::ZERO; nw];
        let mut t2 = vec![Complex64::ZERO; nw];
        for i in 1..=nw {
            for v in a.iter_mut() {
                *v = Complex64::ZERO;
            }
            if i == 1 {
                for v in a.iter_mut().take(nw - 1) {
                    *v = Complex64::new(-1.0, 0.0);
                }
            } else {
                a[i - 2] = Complex64::new(1.0, 0.0);
            }
            zb.mv_mult(&mut t1, &a); // ZB⁻¹ · A (order nw-1)
            t1[nw - 1] = Complex64::ZERO; // Pascal ctemparray1[NumWindings] := 0
            at.mv_mult(&mut t2, &t1); // AT · result (order nw)
            for (j, &val) in t2.iter().enumerate().take(nw) {
                y1.set(j, i - 1, val);
            }
        }

        // Magnetizing branch on winding 2 (closest to the core).
        y1nl.add(
            1,
            1,
            Complex64::new(
                self.pct_no_load_loss / 100.0 / zbase,
                -self.pct_imag / 100.0 / zbase / freq_mult,
            ),
        );

        // Y_Term = AT · Y_1Volt · A, corrected for the actual voltage ratings.
        let n2 = 2 * nw;
        let mut yterm = CMatrix::new(n2);
        let mut yterm_nl = CMatrix::new(n2);
        let mut at2 = CMatrix::new(n2);
        for i in 1..=nw {
            let denom = self.windings[i - 1].vbase * zero_tap_fix(self.windings[i - 1].putap);
            at2.set(2 * i - 2, i - 1, Complex64::new(1.0 / denom, 0.0));
            at2.set(2 * i - 1, i - 1, Complex64::new(-1.0 / denom, 0.0));
        }
        let mut av = vec![Complex64::ZERO; n2];
        let mut s1 = vec![Complex64::ZERO; n2];
        let mut s2 = vec![Complex64::ZERO; n2];
        for i in 1..=n2 {
            for v in av.iter_mut() {
                *v = Complex64::ZERO;
            }
            for kp in 1..=nw {
                let denom = self.windings[kp - 1].vbase * zero_tap_fix(self.windings[kp - 1].putap);
                if i == 2 * kp - 1 {
                    av[kp - 1] = Complex64::new(1.0 / denom, 0.0);
                } else if i == 2 * kp {
                    av[kp - 1] = Complex64::new(-1.0 / denom, 0.0);
                }
            }
            // Main autotransformer part.
            y1.mv_mult(&mut s1, &av); // order nw
            for v in s1.iter_mut().take(n2).skip(nw) {
                *v = Complex64::ZERO;
            }
            at2.mv_mult(&mut s2, &s1); // order n2
            for (j, &val) in s2.iter().enumerate().take(n2) {
                yterm.set(j, i - 1, val);
            }
            // No-load part.
            y1nl.mv_mult(&mut s1, &av);
            for v in s1.iter_mut().take(n2).skip(nw) {
                *v = Complex64::ZERO;
            }
            at2.mv_mult(&mut s2, &s1);
            for (j, &val) in s2.iter().enumerate().take(n2) {
                yterm_nl.set(j, i - 1, val);
            }
        }

        // Anti-float adders: a small admittance on both conductors of each
        // winding so the matrix always inverts even without a voltage ref.
        if self.ppm_float_factor != 0.0 {
            for i in 1..=nw {
                let yadder = Complex64::new(0.0, self.windings[i - 1].y_ppm);
                for j in (2 * i - 1)..=(2 * i) {
                    yterm.add(j - 1, j - 1, yadder);
                }
            }
        }

        self.zb = zb;
        self.y_1volt = y1;
        self.y_1volt_nl = y1nl;
        self.y_term = yterm;
        self.y_term_nl = yterm_nl;
        self.zbase = zbase;
        self.y_terminal_freqmult = freq_mult;
    }

    /// Pascal `TAutoTransObj.GICBuildYTerminal` (`AutoTrans.pas:1823`): the
    /// `Frequency < 0.51 Hz` (dc/GIC) build — a resistance-only `Y_Term` from
    /// `1/RdcOhms` per winding, with **no inter-winding coupling**, and the
    /// anti-float adder added as a real *conductance* (`-Y_PPM`, `G + j0`).
    fn gic_build_y_terminal(&mut self) {
        let nw = self.num_windings.max(0) as usize;
        let n2 = 2 * nw;
        let mut yterm = CMatrix::new(n2);
        let yterm_nl = CMatrix::new(n2);

        for i in 1..=nw {
            let yr = Complex64::new(1.0 / self.windings[i - 1].rdcohms, 0.0); // Siemens
            let idx = 2 * i - 2; // 0-based (Pascal 1-based `2*i - 1`)
            yterm.set(idx, idx, yr);
            yterm.set(idx + 1, idx + 1, yr);
            yterm.set(idx, idx + 1, -yr);
            yterm.set(idx + 1, idx, -yr);
        }

        // Anti-float as a real conductance so the matrix inverts even without a
        // voltage reference on all sides.
        if self.ppm_float_factor != 0.0 {
            for i in 1..=nw {
                let yadder = Complex64::new(-self.windings[i - 1].y_ppm, 0.0); // G + j0
                for j in (2 * i - 1)..=(2 * i) {
                    yterm.add(j - 1, j - 1, yadder);
                }
            }
        }

        self.y_term = yterm;
        self.y_term_nl = yterm_nl;
    }

    /// Pascal `TAutoTransObj.GetAllWindingCurrents` (`AutoTrans.pas:1523`):
    /// `Iterm = Y_Term · Vterm` phase-by-phase, length `2·nphases·NumWindings`.
    /// The Series arm reads the second node from `Vterminal[iphase + Fnphases]`
    /// (the auto's X-terminal phase), the Wye arm from `iphase + …·Fnconds +
    /// Fnphases` (the winding's second conductor). Reads the caller-populated
    /// `cd.vterminal` (see the `WdgCurrents` note in the transformer port).
    fn get_all_winding_currents(&self) -> Vec<Complex64> {
        let nw = self.num_windings.max(0) as usize;
        let np = self.cd.nphases;
        let nconds = self.cd.nconds;
        let mut curr = vec![Complex64::ZERO; 2 * np * nw];
        if !self.cd.enabled || self.cd.node_ref.is_empty() {
            return curr;
        }
        let vterminal = &self.cd.vterminal;
        let mut vterm = vec![Complex64::ZERO; 2 * nw];
        let mut iterm = vec![Complex64::ZERO; 2 * nw];
        let mut iterm_nl = vec![Complex64::ZERO; 2 * nw];
        let mut kk = 0usize;
        for iphase in 1..=np {
            for iwind in 1..=nw {
                let i = 2 * iwind - 1; // 1-based into vterm
                let base = iphase + (iwind - 1) * nconds; // 1-based Vterminal index
                match self.windings[iwind - 1].connection {
                    0 => {
                        // Wye (common winding usually)
                        vterm[i - 1] = vterminal[base - 1];
                        vterm[i] = vterminal[base + np - 1];
                    }
                    1 => {
                        // Delta
                        let jphase = self.rotate_phases(iphase);
                        vterm[i - 1] = vterminal[base - 1];
                        vterm[i] = vterminal[jphase + (iwind - 1) * nconds - 1];
                    }
                    2 => {
                        // Series winding
                        vterm[i - 1] = vterminal[base - 1];
                        vterm[i] = vterminal[iphase + np - 1];
                    }
                    _ => {}
                }
            }
            self.y_term.mv_mult(&mut iterm, &vterm);
            self.y_term_nl.mv_mult(&mut iterm_nl, &vterm);
            for i in 0..2 * nw {
                curr[kk] = iterm[i] + iterm_nl[i];
                kk += 1;
            }
        }
        curr
    }

    /// Pascal `GetWindingCurrentsResult`: the `mag, (angle), ` formatted string
    /// the `WdgCurrents` read-only property returns (one entry per phase ×
    /// winding; the other end of each winding is skipped).
    pub(super) fn winding_currents_result(&self) -> String {
        let nw = self.num_windings.max(0) as usize;
        let np = self.cd.nphases;
        let curr = self.get_all_winding_currents();
        let mut out = String::new();
        let mut k = 0usize;
        for _ in 0..np {
            for _ in 0..nw {
                let c = curr[k];
                k += 1;
                let mag = c.norm();
                let ang = if mag == 0.0 {
                    0.0
                } else {
                    c.arg().to_degrees()
                };
                out.push_str(&crate::report::format::g(mag, 7));
                out.push_str(", (");
                out.push_str(&crate::report::format::g(ang, 5));
                out.push_str("), ");
                k += 1; // skip the other end of the winding
            }
        }
        out
    }
}
