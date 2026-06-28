//! Dynamics-mode machinery: the z-domain ring-buffer filter of the inverter HW
//! model. `InitStateVars`/`InitPhasorStates` seed the history terms from the
//! present operating point; `IntegrateStates`/`IntegratePhasorStates` advance the
//! filter one (half-)step — the waveform path pushes the instantaneous terminal
//! voltage through `bp1` → the z-filter → `bp2` and updates a brute-force RMS
//! window, while the RMS/phasor path runs a PLL on the positive-sequence voltage.
//! `ShutoffInjections` clears the buffers when the terminal opens. The 6 state
//! variables `s1..s6` are what Monitor mode 3 consumes.
//!
//! The ring buffers (`z`/`whist`/`zlast`/`wlast`/`y2`) are kept 1-based (index 0
//! unused) so the intricate `MapIdx`/`OffsetIdx` modular wraparound ports
//! verbatim. Per faithful reproduction the curves (`bp1`/`bp2`/`filter`) are
//! `mem::take`-n out of `self` for the duration of a step (their `GetYValue`
//! hunt-cache mutates) and restored at the end (PORTING_PLAN.md §2.1 fallback).

use num_complex::Complex64;

use crate::elements::traits::{CktElement, SysCtx};
use crate::support::complexutil::cang;
use crate::support::dynamics::IterationFlag;

use super::{Vccs, map_idx, offset_idx};

// Pascal RTL `Pi` (full f64 precision); `cang` carries the truncated-pi atan2
// from `DSSUcomplex` (matching the oracle's polar conversion).
const TWO_PI: f64 = std::f64::consts::TAU;

const NUM_VARIABLES: usize = 6;

impl Vccs {
    /// Pascal `TVCCSObj.InitStateVars` — seed the waveform (or, in `RmsMode`, the
    /// phasor) history terms from the present terminal voltage/current.
    pub(super) fn init_state_vars_impl(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        if self.frms_mode {
            self.init_phasor_states(sys, node_v);
            return;
        }
        self.compute_iterminal(sys, node_v);
        let iang = cang(self.cd.iterminal[0]);
        let vang = cang(self.cd.vterminal[0]);
        self.s1 = self.cd.vterminal[0].norm() / self.base_volt;
        self.s3 = self.cd.iterminal[0].norm() / self.base_curr;
        self.s2 = self.s3;
        self.s4 = self.s3;
        self.s5 = 0.0;
        self.s6 = 0.0;
        self.s_v1 = Complex64::new(1.0, 0.0);
        self.vlast = self.cd.vterminal[0] / self.base_volt;

        // Initialize the history terms for the HW model (source convention).
        let d = 1.0 / self.fsample_freq;
        let wd = TWO_PI * sys.frequency * d;
        let ffiltlen = self.ffiltlen;
        let fwinlen = self.fwinlen;
        let s1 = self.s1;
        let s3 = self.s3;

        let mut fbp1 = self.fbp1.take().expect("filter set ⇒ bp1 set in dynamics");
        let fbp2 = self.fbp2.take().expect("filter set ⇒ bp2 set in dynamics");
        for i in 1..=ffiltlen {
            let wt = vang - wd * (ffiltlen - i) as f64;
            self.whist[i] = fbp1.get_y_value(s1 * wt.cos());
            self.wlast[i] = self.whist[i];
        }
        for i in 1..=fwinlen {
            let wt = iang - wd * (fwinlen - i) as f64;
            let val = s3 * wt.cos(); // current by the passive sign convention
            self.y2[i] = val * val;
            let k = i as i64 - fwinlen as i64 + ffiltlen as i64;
            if k > 0 {
                let k = k as usize;
                self.z[k] = -fbp2.get_x_value(val); // HW history, generator convention
                self.zlast[k] = self.z[k];
            }
        }
        self.fbp1 = Some(fbp1);
        self.fbp2 = Some(fbp2);

        self.s_idx_u = 0;
        self.s_idx_y = 0;
    }

    /// Pascal `TVCCSObj.InitPhasorStates`.
    fn init_phasor_states(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.compute_iterminal(sys, node_v);
        self.s1 = self.cd.vterminal[0].norm() / self.base_volt;
        self.s4 = self.cd.iterminal[0].norm() / self.base_curr;
        self.s2 = self.s4;
        self.s3 = self.s4;
        self.s5 = 0.0;
        self.s6 = 0.0;
        self.s_v1 = Complex64::new(1.0, 0.0);
        self.vlast = self.cd.vterminal[0] / self.base_volt;

        let s1 = self.s1;
        let s4 = self.s4;
        for i in 1..=self.ffiltlen {
            self.whist[i] = s1;
            self.wlast[i] = s1;
        }
        for i in 1..=self.fwinlen {
            let k = i as i64 - self.fwinlen as i64 + self.ffiltlen as i64;
            if k > 0 {
                let k = k as usize;
                self.z[k] = s4; // HW history with load convention
                self.zlast[k] = self.z[k];
            }
        }
        self.s_idx_u = 0;
        self.s_idx_y = 0;
    }

    /// Pascal `TVCCSObj.ShutoffInjections` — stop injecting if the terminal opens.
    fn shutoff_injections(&mut self) {
        for i in 1..=self.ffiltlen {
            self.whist[i] = 0.0;
            self.wlast[i] = 0.0;
            self.z[i] = 0.0;
            self.zlast[i] = 0.0;
        }
        for i in 1..=self.fwinlen {
            self.y2[i] = 0.0;
        }
        self.s1 = 0.0;
        self.s2 = 0.0;
        self.s3 = 0.0;
        self.s4 = 0.0;
        self.s5 = 0.0;
        self.s6 = 0.0;
    }

    /// Pascal `TVCCSObj.IntegrateStates` — called twice per dynamic time step
    /// (predictor then corrector). The corrector step (`IterationFlag = 1`) commits
    /// the ring-buffer history (`zlast`/`wlast`/`sIdxU`/`sIdxY`).
    pub(super) fn integrate_states_impl(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        if !self.cd.conductor_closed(1, 1) {
            self.shutoff_injections();
            return;
        }
        if self.frms_mode {
            self.integrate_phasor_states(sys, node_v);
            return;
        }

        self.compute_iterminal(sys, node_v);

        let t = sys.dyna_t;
        let h = sys.dyna_h;
        let f = sys.frequency;
        let corrector = sys.iteration_flag == IterationFlag::SameTimeStep;
        let d = 1.0 / self.fsample_freq;
        let nstep = (1e-6 + h / d).trunc() as i64;
        let w = TWO_PI * f;

        let vnow = self.cd.vterminal[0] / self.base_volt;
        let mut vin = 0.0;
        let mut y = 0.0;
        let mut iu = self.s_idx_u;
        let mut iy = self.s_idx_y;
        let ffiltlen = self.ffiltlen;
        let fwinlen = self.fwinlen;
        let fl = ffiltlen as i64;
        let wl = fwinlen as i64;
        for k in 1..=ffiltlen {
            self.z[k] = self.zlast[k];
            self.whist[k] = self.wlast[k];
        }

        let mut fbp1 = self.fbp1.take().expect("filter set ⇒ bp1 set in dynamics");
        let mut fbp2 = self.fbp2.take().expect("filter set ⇒ bp2 set in dynamics");
        let filter = self.ffilter.take().expect("filter set in dynamics");
        let filter_y = filter.y_values();
        let filter_x = filter.x_values();

        for i in 1..=nstep {
            iu = offset_idx(iu, 1, fl) as i64;
            // Push the input voltage waveform through the first PWL block.
            let scale = 1.0 * i as f64 / nstep as f64;
            let vre = self.vlast.re + (vnow.re - self.vlast.re) * scale;
            let vim = self.vlast.im + (vnow.im - self.vlast.im) * scale;
            let wt = w * (t - h + i as f64 * d);
            vin = vre * wt.cos() + vim * wt.sin();
            let iu_u = iu as usize;
            self.whist[iu_u] = fbp1.get_y_value(vin);
            // Apply the filter and second PWL block.
            let mut z_iu = 0.0;
            for k in 1..=ffiltlen {
                z_iu += filter_y[k - 1] * self.whist[map_idx(iu - k as i64 + 1, fl)];
            }
            for k in 2..=ffiltlen {
                z_iu -= filter_x[k - 1] * self.z[map_idx(iu - k as i64 + 1, fl)];
            }
            self.z[iu_u] = z_iu;
            y = fbp2.get_y_value(z_iu);
            // Update outputs.
            if corrector && y.abs() > self.s4 {
                self.s4 = y.abs(); // catching the fastest peaks
            }
            // Update the RMS (brute-force window sum).
            iy = offset_idx(iy, 1, wl) as i64;
            self.y2[iy as usize] = y * y;
            if i == nstep {
                self.y2sum = 0.0;
                for k in 1..=fwinlen {
                    self.y2sum += self.y2[k];
                }
                self.s3 = (2.0 * self.y2sum / fwinlen as f64).sqrt();
            }
        }
        self.fbp1 = Some(fbp1);
        self.fbp2 = Some(fbp2);
        self.ffilter = Some(filter);

        if corrector {
            self.s_idx_u = iu;
            self.s_idx_y = iy;
            self.vlast = vnow;
            self.s1 = vin;
            self.s5 = self.whist[iu as usize];
            self.s6 = self.z[iu as usize];
            self.s2 = y;
            for k in 1..=ffiltlen {
                self.zlast[k] = self.z[k];
                self.wlast[k] = self.whist[k];
            }
        }
    }

    /// Pascal `TVCCSObj.IntegratePhasorStates` — the RMS/phasor (PLL) path.
    fn integrate_phasor_states(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.compute_iterminal(sys, node_v);
        self.update_sequence_voltage();
        let vpu = self.s_v1.norm() / self.base_volt;
        if vpu <= 0.0 {
            return;
        }
        let h = sys.dyna_h;
        let corrector = sys.iteration_flag == IterationFlag::SameTimeStep;
        let nstep = (1e-6 + h * self.fsample_freq).trunc() as i64;

        // Vrms from the LPF.
        let mut dd = vpu - self.s1;
        self.s1 += dd * (1.0 - (-h / self.fvrms_tau).exp());
        // RMS current to maintain power.
        let mut ipwr = self.base_curr / self.s1;
        let imax = self.fmax_ipu * self.irated;
        if ipwr > imax {
            ipwr = imax;
        }
        self.s2 = ipwr / self.base_curr;

        let mut iu = self.s_idx_u;
        let ffiltlen = self.ffiltlen;
        let fl = ffiltlen as i64;
        for k in 1..=ffiltlen {
            self.z[k] = self.zlast[k];
            self.whist[k] = self.wlast[k];
        }

        let filter = self.ffilter.take().expect("filter set in dynamics");
        let filter_y = filter.y_values();
        let filter_x = filter.x_values();
        let s2 = self.s2;
        for _i in 1..=nstep {
            iu = offset_idx(iu, 1, fl) as i64;
            let iu_u = iu as usize;
            self.whist[iu_u] = s2;
            // Apply the filter and second PWL block.
            let mut z_iu = 0.0;
            for k in 1..=ffiltlen {
                z_iu += filter_y[k - 1] * self.whist[map_idx(iu - k as i64 + 1, fl)];
            }
            for k in 2..=ffiltlen {
                z_iu -= filter_x[k - 1] * self.z[map_idx(iu - k as i64 + 1, fl)];
            }
            self.z[iu_u] = z_iu;
            self.s3 = z_iu;
        }
        self.ffilter = Some(filter);

        // Irms through the LPF.
        dd = self.s3 - self.s4;
        self.s4 += dd * (1.0 - (-h / self.firms_tau).exp());
        if corrector {
            self.s_idx_u = iu;
            for k in 1..=ffiltlen {
                self.zlast[k] = self.z[k];
                self.wlast[k] = self.whist[k];
            }
        }
    }

    /// Pascal `TVCCSObj.NumVariables`.
    pub(super) fn num_variables_impl(&self) -> usize {
        NUM_VARIABLES
    }

    /// Pascal `TVCCSObj.VariableName(i)` (1-based). Out-of-range → "" (the Pascal
    /// `Result := ''` default).
    pub(super) fn variable_name_impl(&self, i: usize) -> String {
        let name = if self.frms_mode {
            match i {
                1 => "Vrms",
                2 => "Ipwr",
                3 => "Hout",
                4 => "Irms",
                5 => "NA",
                6 => "NA",
                _ => "",
            }
        } else {
            match i {
                1 => "Vwave",
                2 => "Iwave",
                3 => "Irms",
                4 => "Ipeak",
                5 => "BP1out",
                6 => "Hout",
                _ => "",
            }
        };
        name.to_string()
    }

    /// Pascal `TVCCSObj.GetAllVariables` (`States[i-1] := Get_Variable(i)` for
    /// `i := 1..6`, i.e. the raw `s1..s6`).
    pub(super) fn get_all_variables_impl(&self, states: &mut [f64]) {
        states[0] = self.s1;
        states[1] = self.s2;
        states[2] = self.s3;
        states[3] = self.s4;
        states[4] = self.s5;
        states[5] = self.s6;
    }

    /// Pascal `TVCCSObj.Set_Variable(i, value)` (1-based).
    pub(super) fn set_variable_impl(&mut self, i: usize, value: f64) {
        match i {
            1 => self.s1 = value,
            2 => self.s2 = value,
            3 => self.s3 = value,
            4 => self.s4 = value,
            5 => self.s5 = value,
            6 => self.s6 = value,
            _ => {}
        }
    }
}
