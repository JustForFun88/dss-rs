//! Pascal `TMonitorObj.TakeSample` and the polar/residual conversion helpers.

use num_complex::Complex64;

use super::{MAGNITUDEMASK, MODEMASK, Monitor, MonitorSampleCtx, POSSEQONLYMASK, SEQUENCEMASK};
use crate::elements::pd::capacitor::Capacitor;
use crate::elements::pd::transformer::Transformer;
use crate::elements::traits::SysCtx;
use crate::obj::base::DssObject;
use crate::support::complexutil::cdang;
use crate::support::mathutil::SymComp;

impl Monitor {
    fn add_dbl(&mut self, v: f64) {
        self.mon_buffer.push(v as f32);
    }
    fn add_dbls(&mut self, vs: &[f64]) {
        for &v in vs {
            self.add_dbl(v);
        }
    }

    /// Pascal `TMonitorObj.TakeSample`: append one record for the present
    /// solution. `metered` is the monitored circuit element (the source for a
    /// mode-5 monitor, which does not read it).
    pub fn take_sample(
        &mut self,
        metered: &mut dyn DssObject,
        node_v: &[Complex64],
        sys: &SysCtx,
        sol: &MonitorSampleCtx,
    ) {
        if !(self.valid_monitor && self.med.cd.enabled) {
            return;
        }
        self.sample_count += 1;
        self.hour = sol.int_hour;
        self.sec = sol.t;

        // Metered element dimensions (immutable peek; borrow ends here).
        let (m_nconds, m_yorder) = {
            let e = metered
                .as_ckt_element()
                .expect("metered element is a circuit element");
            (e.cd().nconds, e.cd().yorder)
        };
        let offset = (self.med.metered_terminal as usize - 1) * m_nconds;
        let fnphases = self.med.cd.nphases;
        let fnconds = self.med.cd.nconds;

        // Time stamp (frequency/harmonic in harmonic mode — Phase 7 path).
        if sol.is_harmonic {
            self.add_dbl(sol.frequency);
            self.add_dbl(sol.harmonic);
        } else {
            self.add_dbl(self.hour as f64);
            self.add_dbl(self.sec);
        }

        let mode_mask = self.mode & MODEMASK;

        // Scratch buffers (Pascal keeps them as fields for reuse).
        let mut current_buffer = vec![Complex64::ZERO; m_yorder.max(fnconds + 1)];
        let mut voltage_buffer = vec![Complex64::ZERO; m_yorder.max(fnconds + 1)];

        match mode_mask {
            0 | 1 => {
                let e = metered.as_ckt_element_mut().expect("ckt element");
                e.compute_iterminal(sys, node_v);
                let cd = e.cd();
                current_buffer[..m_yorder].copy_from_slice(&cd.iterminal[..m_yorder]);
                for (i, v) in voltage_buffer.iter_mut().enumerate().take(fnconds) {
                    *v = node_v[self.med.cd.node_ref[i]];
                }
            }
            2 => {
                let tap = metered
                    .as_any()
                    .downcast_ref::<Transformer>()
                    .map(|t| t.present_tap(self.med.metered_terminal as usize))
                    .unwrap_or(0.0);
                self.add_dbl(tap);
                return;
            }
            5 => {
                // Solution variables (no metered access).
                let vars = [
                    sol.iteration as f64,
                    sol.control_iteration as f64,
                    sol.max_iterations as f64,
                    sol.max_control_iterations as f64,
                    if sol.converged { 1.0 } else { 0.0 },
                    sol.interval_hrs,
                    sol.solution_count as f64,
                    sol.mode_ordinal as f64,
                    sol.frequency,
                    sol.year as f64,
                    sol.solve_time_us,
                    sol.step_time_us,
                ];
                self.add_dbls(&vars);
                return;
            }
            6 => {
                if let Some(cap) = metered.as_any().downcast_ref::<Capacitor>() {
                    let states: Vec<f64> = cap.states().iter().map(|&s| s as f64).collect();
                    self.add_dbls(&states);
                }
                return;
            }
            9 => {
                let e = metered.as_ckt_element_mut().expect("ckt element");
                let losses = e.losses(sys, node_v);
                self.add_dbl(losses.re);
                self.add_dbl(losses.im);
                return;
            }
            11 => {
                let e = metered.as_ckt_element_mut().expect("ckt element");
                e.cd_mut().compute_vterminal(node_v);
                e.compute_iterminal(sys, node_v);
                let cd = e.cd();
                voltage_buffer[..m_yorder].copy_from_slice(&cd.vterminal[..m_yorder]);
                current_buffer[..m_yorder].copy_from_slice(&cd.iterminal[..m_yorder]);
                convert_to_polar(&mut voltage_buffer, m_yorder);
                for &c in &voltage_buffer[..m_yorder] {
                    self.add_dbl(c.re);
                    self.add_dbl(c.im);
                }
                convert_to_polar(&mut current_buffer, m_yorder);
                for &c in &current_buffer[..m_yorder] {
                    self.add_dbl(c.re);
                    self.add_dbl(c.im);
                }
                return;
            }
            // Modes 3 (state vars), 4 (flicker/Pstcalc), 7 (Storage), 8/10
            // (transformer winding currents/voltages), 12 (LL) build their
            // header but defer the sample body to Phase 6+/7 (no gate uses
            // them; the metered surface they need is not yet exposed).
            _ => return,
        }

        // --- Common tail for modes 0 and 1 --------------------------------
        let is_sequence = (self.mode & SEQUENCEMASK) > 0 && fnphases == 3;
        let num_vi = if is_sequence {
            let sc = SymComp::default();
            let mut v012 = [Complex64::ZERO; 3];
            let mut i012 = [Complex64::ZERO; 3];
            sc.phase_to_sym(&voltage_buffer[0..3], &mut v012);
            let mut iph = [Complex64::ZERO; 3];
            iph.copy_from_slice(&current_buffer[offset..offset + 3]);
            sc.phase_to_sym(&iph, &mut i012);
            voltage_buffer[0..3].copy_from_slice(&v012);
            current_buffer[offset..offset + 3].copy_from_slice(&i012);
            3
        } else {
            fnconds
        };

        // Residual (mode 0 only) and per-mode conversion.
        let mut residual_volt = Complex64::ZERO;
        let mut residual_curr = Complex64::ZERO;
        let is_power = mode_mask == 1;
        if mode_mask == 0 {
            if self.include_residual {
                if self.vi_polar {
                    residual_volt = residual_polar(&voltage_buffer[0..fnphases]);
                    residual_curr = residual_polar(&current_buffer[offset..offset + fnphases]);
                } else {
                    residual_volt = residual_sum(&voltage_buffer[0..fnphases]);
                    residual_curr = residual_sum(&current_buffer[offset..offset + fnphases]);
                }
            }
            if self.vi_polar {
                convert_to_polar(&mut voltage_buffer, num_vi);
                convert_to_polar_offset(&mut current_buffer, offset, num_vi);
            }
        } else {
            // Mode 1: VoltageBuffer := kW/kvar = V·conj(I)·0.001 (dest aliases V).
            for j in 0..num_vi {
                let p = voltage_buffer[j] * current_buffer[offset + j].conj() * 0.001;
                voltage_buffer[j] = p;
            }
            if is_sequence || sys.positive_sequence {
                for v in voltage_buffer.iter_mut().take(num_vi) {
                    *v *= 3.0;
                }
            }
            if self.pp_polar {
                convert_to_polar(&mut voltage_buffer, num_vi);
            }
        }

        // --- Write to disk (the MAGNITUDE/POSSEQ modifier paths) ----------
        match self.mode & (MAGNITUDEMASK + POSSEQONLYMASK) {
            32 => {
                for &c in &voltage_buffer[..num_vi] {
                    self.add_dbl(c.re);
                }
                if self.include_residual {
                    self.add_dbl(residual_volt.re);
                }
                if !is_power {
                    for &c in &current_buffer[offset..offset + num_vi] {
                        self.add_dbl(c.re);
                    }
                    if self.include_residual {
                        self.add_dbl(residual_curr.re);
                    }
                }
            }
            64 => {
                if is_sequence {
                    self.add_dbl(voltage_buffer[1].re);
                    self.add_dbl(voltage_buffer[1].im);
                    if !is_power {
                        self.add_dbl(current_buffer[offset + 1].re);
                        self.add_dbl(current_buffer[offset + 1].im);
                    }
                } else if is_power {
                    let sum: Complex64 = voltage_buffer[0..fnphases].iter().sum();
                    self.add_dbl(sum.re);
                    self.add_dbl(sum.im);
                } else {
                    let mut sum: Complex64 = voltage_buffer[0..fnphases].iter().sum();
                    sum.re /= fnphases as f64;
                    self.add_dbl(sum.re);
                    self.add_dbl(sum.im);
                    let mut sum2: Complex64 =
                        current_buffer[offset..offset + fnphases].iter().sum();
                    sum2.re /= fnphases as f64;
                    self.add_dbl(sum2.re);
                    self.add_dbl(sum2.im);
                }
            }
            96 => {
                if is_sequence {
                    self.add_dbl(voltage_buffer[1].re);
                    if !is_power {
                        self.add_dbl(current_buffer[offset + 1].re);
                    }
                } else {
                    let mut dsum: f64 = voltage_buffer[0..fnphases].iter().map(|c| c.re).sum();
                    if !is_power {
                        dsum /= fnphases as f64;
                    }
                    self.add_dbl(dsum);
                    if !is_power {
                        let dsum2: f64 = current_buffer[offset..offset + fnphases]
                            .iter()
                            .map(|c| c.re)
                            .sum::<f64>()
                            / fnphases as f64;
                        self.add_dbl(dsum2);
                    }
                }
            }
            _ => {
                // V and I in mag/angle or complex kW/kvar.
                for &c in &voltage_buffer[..num_vi] {
                    self.add_dbl(c.re);
                    self.add_dbl(c.im);
                }
                if !is_power {
                    if self.include_residual {
                        self.add_dbl(residual_volt.re);
                        self.add_dbl(residual_volt.im);
                    }
                    for &c in &current_buffer[offset..offset + num_vi] {
                        self.add_dbl(c.re);
                        self.add_dbl(c.im);
                    }
                    if self.include_residual {
                        self.add_dbl(residual_curr.re);
                        self.add_dbl(residual_curr.im);
                    }
                }
            }
        }
    }
}

/// Pascal `ConvertComplexArrayToPolar`: each element → `(mag, angle_deg)`.
fn convert_to_polar(buf: &mut [Complex64], n: usize) {
    for c in buf.iter_mut().take(n) {
        let m = c.norm();
        let a = cdang(*c);
        *c = Complex64::new(m, a);
    }
}

fn convert_to_polar_offset(buf: &mut [Complex64], offset: usize, n: usize) {
    for c in buf.iter_mut().skip(offset).take(n) {
        let m = c.norm();
        let a = cdang(*c);
        *c = Complex64::new(m, a);
    }
}

/// Pascal `Residual`: sum of the first `nph` phasors.
fn residual_sum(buf: &[Complex64]) -> Complex64 {
    buf.iter().sum()
}

/// Pascal `ResidualPolar`: magnitude + angle (deg) of the residual.
fn residual_polar(buf: &[Complex64]) -> Complex64 {
    let x = residual_sum(buf);
    Complex64::new(x.norm(), cdang(x))
}
