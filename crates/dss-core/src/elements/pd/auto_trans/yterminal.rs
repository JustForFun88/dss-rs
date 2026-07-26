//! The electrical core: `RecalcElementData` (derived per-winding data, the
//! Series-winding `kVSeries`/`VBase`), `CalcY_Terminal` (the `2·NumWindings`
//! terminal admittance with the auto corrections `ZCorrected`/`puXst`),
//! `GICBuildYTerminal` (the `Frequency < 0.51` resistance-only branch), and the
//! winding-current readouts.

use num_complex::Complex64;

use crate::elements::pd::winding::{Connection, TermRef, WdgTerms};
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
        } else if self.windings[0].connection == Connection::Series {
            self.delta_direction = 1; // Auto
        } else {
            let ihv = if self.windings[0].kvll >= self.windings[1].kvll {
                1
            } else {
                2
            };
            match self.windings[ihv - 1].connection {
                Connection::Wye => self.delta_direction = if self.hv_leads_lv { -1 } else { 1 },
                Connection::Delta => self.delta_direction = if self.hv_leads_lv { 1 } else { -1 },
                Connection::Series => {}
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
                Connection::Wye => {
                    // Wye — assume 3-phase for the 2-phase designation.
                    w.vbase = if np == 2 || np == 3 {
                        w.kvll * inv_sqrt3_x1000()
                    } else {
                        w.kvll * 1000.0
                    };
                }
                Connection::Delta => w.vbase = w.kvll * 1000.0,
                Connection::Series => {
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
                // Pascal `Rdcpu * SQR(VBase) / VABase` (AutoTrans.pas:1021):
                // `SQR` binds first, so the square is formed BEFORE the multiply.
                // Left-to-right `rdcpu * vbase * vbase` reassociates the product
                // and lands one ULP off the oracle on the rendered RDCOhms.
                w.rdcohms = w.rdcpu * (w.vbase * w.vbase) / vabase;
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
            Connection::Wye => w1.vbase * 0.001,
            Connection::Delta => match np {
                1 => w1.vbase * 0.001,
                2 | 3 => w1.vbase * 0.001 / sqrt3(),
                _ => w1.vbase * 0.001 * 0.5 / (std::f64::consts::PI / np as f64).sin(),
            },
            Connection::Series => w1.vbase * 0.001,
        };
        self.norm_amps = self.norm_max_hkva / np as f64 / vfactor;
        self.emerg_amps = self.emerg_max_hkva / np as f64 / vfactor;

        self.calc_y_terminal(1.0, self.live_frequency);
    }

    /// Pascal `TAutoTransObj.CalcY_Terminal` (`AutoTrans.pas:1856`): build the
    /// `2·NumWindings` terminal admittance (`Y_Term`) and its no-load companion
    /// (`Y_Term_NL`). Below `0.51 Hz` (the GIC/dc branch) it delegates to
    /// [`Self::gic_build_y_terminal`]; otherwise it builds `ZB` with the auto
    /// corrections — the series diagonal scaled by `ZCorrected = ZBase·(1 +
    /// Vc/Vs)²` (Dommel 6.45) and the 3-winding `puXst` (Dommel 6.50).
    /// `frequency` is the live `ActiveCircuit.Solution.Frequency` Pascal reads
    /// for the `< 0.51 Hz` GIC gate.
    pub(super) fn calc_y_terminal(&mut self, freq_mult: f64, frequency: f64) {
        // Pascal `AutoTrans.pas`: below 0.51 Hz build the GIC/dc branch and
        // return. `frequency` is threaded from the caller (CalcYPrim passes the
        // live `Solution.Frequency`; RecalcElementData passes the synced copy),
        // matching Pascal's global read 1:1 on both paths.
        if frequency < 0.51 {
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
        // Off diagonals: the upper triangle of the `nw-1` block, one XSC entry
        // per (i, j) pair in row-major order. Pascal's running index starts at
        // `XSC[NumWindings]` → 0-based `xsc[nw-1]`, so pair `t` reads
        // `xsc[nw-1+t]` (same (i, j, k) sequence as the old running `k`).
        let off_pairs = (0..nw - 1).flat_map(|i| ((i + 1)..(nw - 1)).map(move |j| (i, j)));
        for (t, (i, j)) in off_pairs.enumerate() {
            let term = Complex64::new(
                rmult * (self.windings[i + 1].rpu + self.windings[j + 1].rpu),
                freq_mult * self.xsc[nw - 1 + t],
            ) * zbase;
            let v = (zb.get(i, i) + zb.get(j, j) - term) * 0.5;
            zb.set(i, j, v);
            zb.set(j, i, v);
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
        for i in 0..nw {
            for v in a.iter_mut() {
                *v = Complex64::ZERO;
            }
            if i == 0 {
                for v in a.iter_mut().take(nw - 1) {
                    *v = Complex64::new(-1.0, 0.0);
                }
            } else {
                a[i - 1] = Complex64::new(1.0, 0.0);
            }
            zb.mv_mult(&mut t1, &a); // ZB⁻¹ · A (order nw-1)
            t1[nw - 1] = Complex64::ZERO; // Pascal ctemparray1[NumWindings] := 0
            at.mv_mult(&mut t2, &t1); // AT · result (order nw)
            for (j, &val) in t2.iter().enumerate().take(nw) {
                y1.set(j, i, val);
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
        for (iwind, w) in self.windings.iter().enumerate() {
            let denom = w.vbase * zero_tap_fix(w.putap);
            let wt = WdgTerms::of(iwind);
            at2.set(wt.plus, iwind, Complex64::new(1.0 / denom, 0.0));
            at2.set(wt.minus, iwind, Complex64::new(-1.0 / denom, 0.0));
        }
        let mut av = vec![Complex64::ZERO; n2];
        let mut s1 = vec![Complex64::ZERO; n2];
        let mut s2 = vec![Complex64::ZERO; n2];
        for i in 0..n2 {
            for v in av.iter_mut() {
                *v = Complex64::ZERO;
            }
            for (kp, (av_kp, w)) in av.iter_mut().zip(self.windings.iter()).enumerate() {
                let denom = w.vbase * zero_tap_fix(w.putap);
                if i == 2 * kp {
                    *av_kp = Complex64::new(1.0 / denom, 0.0);
                } else if i == 2 * kp + 1 {
                    *av_kp = Complex64::new(-1.0 / denom, 0.0);
                }
            }
            // Main autotransformer part.
            y1.mv_mult(&mut s1, &av); // order nw
            for v in s1.iter_mut().take(n2).skip(nw) {
                *v = Complex64::ZERO;
            }
            at2.mv_mult(&mut s2, &s1); // order n2
            for (j, &val) in s2.iter().enumerate().take(n2) {
                yterm.set(j, i, val);
            }
            // No-load part.
            y1nl.mv_mult(&mut s1, &av);
            for v in s1.iter_mut().take(n2).skip(nw) {
                *v = Complex64::ZERO;
            }
            at2.mv_mult(&mut s2, &s1);
            for (j, &val) in s2.iter().enumerate().take(n2) {
                yterm_nl.set(j, i, val);
            }
        }

        // Anti-float adders: a small admittance on both conductors of each
        // winding so the matrix always inverts even without a voltage ref.
        if self.ppm_float_factor != 0.0 {
            for (iwind, w) in self.windings.iter().enumerate() {
                let yadder = Complex64::new(0.0, w.y_ppm);
                let wt = WdgTerms::of(iwind);
                yterm.add(wt.plus, wt.plus, yadder);
                yterm.add(wt.minus, wt.minus, yadder);
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

        for (iwind, w) in self.windings.iter().enumerate() {
            let yr = Complex64::new(1.0 / w.rdcohms, 0.0); // Siemens
            let wt = WdgTerms::of(iwind);
            yterm.set(wt.plus, wt.plus, yr);
            yterm.set(wt.minus, wt.minus, yr);
            yterm.set(wt.plus, wt.minus, -yr);
            yterm.set(wt.minus, wt.plus, -yr);
        }

        // Anti-float as a real conductance so the matrix inverts even without a
        // voltage reference on all sides.
        if self.ppm_float_factor != 0.0 {
            for (iwind, w) in self.windings.iter().enumerate() {
                let yadder = Complex64::new(-w.y_ppm, 0.0); // G + j0
                let wt = WdgTerms::of(iwind);
                yterm.add(wt.plus, wt.plus, yadder);
                yterm.add(wt.minus, wt.minus, yadder);
            }
        }

        self.y_term = yterm;
        self.y_term_nl = yterm_nl;
    }

    /// Pascal `TAutoTransObj.BuildYPrimComponent` (`AutoTrans.pas:1794`): stamp
    /// `Y_Terminal` into the phase-expanded `YPrim` component via `TermRef` (each
    /// entry goes in `nphases` times). Identical to the Transformer's.
    pub(super) fn build_yprim_component(
        yp: &mut CMatrix,
        yt: &CMatrix,
        term_ref: &TermRef,
        nw: usize,
        np: usize,
    ) {
        // Walk the lower triangle of the `2·nw` `Y_Terminal` as (winding, side)
        // pairs — identical to the Transformer's, in the same `(i, j, phase)`
        // `add_sym` order as the flat Pascal `TermRef` stamping.
        for wi in 0..nw {
            for side_i in 0..2 {
                let i = 2 * wi + side_i;
                for wj in 0..=wi {
                    let side_j_max = if wj == wi { side_i } else { 1 };
                    for side_j in 0..=side_j_max {
                        let j = 2 * wj + side_j;
                        let value = yt.get(i, j);
                        for kk in 0..np {
                            let r = term_ref.pair(kk, wi, nw)[side_i];
                            let c = term_ref.pair(kk, wj, nw)[side_j];
                            yp.add_sym(r, c, value);
                        }
                    }
                }
            }
        }
    }

    /// Pascal `TAutoTransObj.GetWindingVoltages` (`AutoTrans.pas:1604`): the
    /// voltages across the `iWind` winding's phases into `vbuffer` (0-based,
    /// length `nphases`). The Series arm subtracts `Vterminal[i + Fnconds]` (the
    /// series winding straddles the H/X terminals). RegControl's control voltage.
    pub(super) fn get_winding_voltages(
        &mut self,
        iwind: usize,
        node_v: &[Complex64],
        vbuffer: &mut [Complex64],
    ) {
        let nphases = self.cd.nphases;
        if !self.cd.enabled || self.cd.node_ref.is_empty() || node_v.is_empty() {
            return;
        }
        if iwind < 1 || iwind > self.num_windings.max(0) as usize {
            for v in vbuffer.iter_mut().take(self.cd.nconds) {
                *v = Complex64::ZERO;
            }
            return;
        }
        self.cd.compute_vterminal(node_v);
        // Winding `iwind` is terminal `iwind-1`; `vt` is that terminal's
        // conductor slice (phase `i` = `vt[i]`, neutral = `vt[nphases]`). The
        // Series arm subtracts terminal 1's phase `i` (the auto's X terminal).
        let vt = self.cd.term_v(iwind - 1);
        let conn = self.windings[iwind - 1].connection;
        for i in 0..nphases {
            match conn {
                Connection::Wye => vbuffer[i] = vt[i] - vt[nphases],
                Connection::Delta => {
                    // Delta: next phase in sequence (rotate_phases is 1-based).
                    let ii = self.rotate_phases(i + 1) - 1;
                    vbuffer[i] = vt[i] - vt[ii];
                }
                Connection::Series => vbuffer[i] = vt[i] - self.cd.term_v(1)[i], // winding 1
            }
        }
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
        for iphase in 0..np {
            for (iwind, w) in self.windings.iter().enumerate() {
                let wt = WdgTerms::of(iwind);
                let base = iphase + iwind * nconds; // 0-based Vterminal phase conductor
                match w.connection {
                    Connection::Wye => {
                        // Wye (common winding usually)
                        vterm[wt.plus] = vterminal[base];
                        vterm[wt.minus] = vterminal[base + np];
                    }
                    Connection::Delta => {
                        // `rotate_phases` speaks the 1-based phase language.
                        let jphase = self.rotate_phases(iphase + 1);
                        vterm[wt.plus] = vterminal[base];
                        vterm[wt.minus] = vterminal[jphase + iwind * nconds - 1];
                    }
                    Connection::Series => {
                        // Series winding straddles the H/X terminals.
                        vterm[wt.plus] = vterminal[base];
                        vterm[wt.minus] = vterminal[iphase + np];
                    }
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
