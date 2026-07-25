//! The electrical core: `RecalcElementData` (derived per-winding data),
//! `CalcY_Terminal` (the `2·NumWindings` admittance from `ZB`, the winding-ratio
//! incidence and the magnetizing branch), the phase-expanded `YPrim` stamping
//! and the winding-current readouts.

use num_complex::Complex64;

use crate::elements::pd::winding::{Connection, TermRef, WdgTerms, Winding};
use crate::support::cmatrix::CMatrix;
use crate::util::{EPSILON, inv_sqrt3_x1000, sqrt3};

use super::{Transformer, xsc_size};

/// Pascal `ZeroTapFix`: a 0 pu tap (which RegControl can force) becomes 0.0001.
fn zero_tap_fix(tap: f64) -> f64 {
    if tap == 0.0 { 0.0001 } else { tap }
}

impl Transformer {
    /// Pascal `TTransfObj.RecalcElementData`: derived per-winding data
    /// (`DeltaDirection`, `TermRef`, `XSC` from XHL, `VBase`, `Rdc`, anti-float,
    /// `NormAmps`/`EmergAmps`/`AmpRatings`) then `CalcY_Terminal` at base freq.
    pub(super) fn recalc(&mut self) {
        // Determine Delta Direction. If HV winding is delta it leads wye.
        if self.windings[0].connection == self.windings[1].connection {
            self.delta_direction = 1;
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

        if self.xhl_changed {
            if self.num_windings <= 3 {
                let n = xsc_size(self.num_windings);
                let vals = [self.xhl, self.xht, self.xlt];
                for (i, v) in vals.iter().enumerate().take(n) {
                    self.xsc[i] = *v;
                }
            }
            self.xhl_changed = false;
        }

        // Winding voltage bases (volts).
        let np = self.cd.nphases;
        for w in &mut self.windings {
            match w.connection {
                Connection::Wye => {
                    w.vbase = if np == 2 || np == 3 {
                        w.kvll * inv_sqrt3_x1000()
                    } else {
                        w.kvll * 1000.0
                    };
                }
                Connection::Delta => w.vbase = w.kvll * 1000.0,
                Connection::Series => {}
            }
        }

        self.vabase = self.windings[0].kva * 1000.0;
        let vabase = self.vabase;

        // Rdc per winding.
        for w in &mut self.windings {
            if w.rdc_specified {
                w.rdcpu = w.rdcohms / (w.vbase * w.vbase / vabase);
            } else {
                w.rdcpu = (0.85 * w.rpu).abs();
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
            Connection::Wye => w1.vbase * 0.001,
            Connection::Delta => match np {
                1 => w1.vbase * 0.001,
                2 | 3 => w1.vbase * 0.001 / sqrt3(),
                _ => w1.vbase * 0.001 * 0.5 / (std::f64::consts::PI / np as f64).sin(),
            },
            Connection::Series => 1.0,
        };
        self.norm_amps = self.norm_max_hkva / np as f64 / vfactor;
        self.emerg_amps = self.emerg_max_hkva / np as f64 / vfactor;
        // dss_capi 0.15.x (`Transformer.pas:1058`, commit 4ed59416 / SVN r4033):
        // the spurious `1.1 *` factor was DROPPED from the seasonal AmpRatings —
        // `AmpRatings[i] := kVARatings[i] / Fnphases / Vfactor` (the seasonal
        // ratings now equal the plain per-phase current at each seasonal kVA,
        // NOT 110% of it). `NormMaxHkVA`'s own 1.1 (the 110% default norm rating,
        // above) is a DIFFERENT quantity and is unchanged upstream. UPGRADE_PLAN
        // WP-U1.2 row D6; ledger DIVERGENCES.md §D6.
        self.amp_ratings = self
            .kva_ratings
            .iter()
            .map(|r| r / np as f64 / vfactor)
            .collect();

        self.calc_y_terminal(1.0);
    }

    /// Pascal `TTransfObj.CalcY_Terminal`: build the `2·NumWindings` terminal
    /// admittance (`Y_Term`) and its no-load companion (`Y_Term_NL`) at the
    /// given frequency multiplier. GIC (`frequency < 0.51`) is Phase 7.
    pub(super) fn calc_y_terminal(&mut self, freq_mult: f64) {
        let nw = self.num_windings.max(0) as usize;
        let rmult = if self.xrconst { freq_mult } else { 1.0 };

        // ZBMatrix (order NumWindings-1), pu → ohms on a one-volt base.
        let mut zb = CMatrix::new(nw - 1);
        let zbase = 1.0 / (self.vabase / self.cd.nphases as f64);
        for i in 0..nw - 1 {
            let v = Complex64::new(
                rmult * (self.windings[0].rpu + self.windings[i + 1].rpu),
                freq_mult * self.xsc[i],
            ) * zbase;
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
        for (iwind, w) in self.windings.iter().enumerate() {
            let denom = w.vbase * zero_tap_fix(w.putap);
            let wt = WdgTerms::of(iwind);
            at2.set(wt.plus, iwind, Complex64::new(1.0 / denom, 0.0));
            at2.set(wt.minus, iwind, Complex64::new(-1.0 / denom, 0.0));
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
            // Main transformer part.
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

    /// Pascal `BuildYPrimComponent`: stamp `Y_Terminal` into the phase-expanded
    /// `YPrim` component via `TermRef` (each entry goes in `nphases` times).
    pub(super) fn build_yprim_component(
        yp: &mut CMatrix,
        yt: &CMatrix,
        term_ref: &TermRef,
        nw: usize,
        np: usize,
    ) {
        // Walk the lower triangle of the `2·nw` `Y_Terminal` as (winding, side)
        // pairs — `Y_Terminal` row `2·w+side` is winding `w`'s plus (0) / minus
        // (1) conductor. This yields the exact `(i, j, phase)` `add_sym` order of
        // the flat Pascal `TermRef` stamping (i outer `1..=2nw`, j `1..=i`).
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

    /// Pascal `AddNeutralToY`: neutral grounding branches for wye windings
    /// (`Rneut`/`Xneut`; `Rneut < 0` = open, bumped by the anti-float adder).
    pub(super) fn add_neutral_to_y(
        yps: &mut CMatrix,
        windings: &[Winding],
        nconds: usize,
        ppm: f64,
        freq_mult: f64,
    ) {
        for (i, w) in windings.iter().enumerate() {
            if w.connection != Connection::Wye {
                continue; // wye only (ignore delta and open wye)
            }
            let j = (i + 1) * nconds;
            if w.rneut >= 0.0 {
                let value = if w.rneut == 0.0 && w.xneut == 0.0 {
                    Complex64::new(1_000_000.0, 0.0) // solidly grounded
                } else {
                    Complex64::new(w.rneut, w.xneut * freq_mult).inv()
                };
                yps.add(j - 1, j - 1, value);
            } else if ppm != 0.0 {
                // Open neutral: bump admittance a bit in case it floats.
                yps.add(j - 1, j - 1, Complex64::new(0.0, w.y_ppm));
            }
        }
    }

    /// Pascal `TTransfObj.GetAllWindingCurrents`: `Iterm = Y_Term · Vterm`
    /// phase-by-phase, length `2·nphases·NumWindings`. Returns zeros for a
    /// disabled element or one not yet wired into the solution (Pascal
    /// `Transformer.pas:1530`: `if (not Enabled) or (NodeRef = NIL) or
    /// (Solution.NodeV = NIL) then Exit`). Reads the caller-populated
    /// `cd.vterminal` (Pascal reloads it from `Solution.NodeV` internally; the
    /// `&self` getter can't reach the solution, so the `WdgCurrents` prop def
    /// carries `PropFlags::READS_VTERMINAL` and the `?`/`Dump` surfaces refresh
    /// via `Dss::refresh_vterminal_if_marked`; the solve path for
    /// `Powers`/`Currents` refreshes as before).
    pub(crate) fn get_all_winding_currents(&self) -> Vec<Complex64> {
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
            for (iwind, w) in self.windings.iter().enumerate() {
                let wt = WdgTerms::of(iwind);
                let base = iphase + iwind * nconds - 1; // 0-based phase conductor
                match w.connection {
                    Connection::Wye => {
                        vterm[wt.plus] = vterminal[base];
                        vterm[wt.minus] = vterminal[(iwind + 1) * nconds - 1]; // neutral
                    }
                    Connection::Delta => {
                        let jphase = self.rotate_phases(iphase);
                        vterm[wt.plus] = vterminal[base];
                        vterm[wt.minus] = vterminal[jphase + iwind * nconds - 1];
                    }
                    Connection::Series => {}
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
                // Pascal Cdang: degrees; 0 for a zero phasor.
                let ang = if mag == 0.0 {
                    0.0
                } else {
                    c.arg().to_degrees()
                };
                // Pascal: Format('%.7g, (%.5g), ', [Cabs, Cdang]) — FPC `%g`
                // renders the exponent uppercase (`E-12`), so route through
                // `report::format::g` (not the lowercase-`e` `fmt_g`).
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
