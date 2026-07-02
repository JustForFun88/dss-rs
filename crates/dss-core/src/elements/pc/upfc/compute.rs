//! The UPFC power-flow algorithm: the 5-mode `GetOutputCurr` series-injection
//! dispatcher, the `GetInputCurr` power-balance input current, `CalcUPFCPowers`,
//! `CalcUPFCLosses`, the `GetInjCurrents` terminal assembly, the control-coupling
//! `CheckStatus`/`UploadCurrents`, and the Monitor-mode-3 variable interface.
//! Split out of `upfc/mod.rs` (no behavioral change).

use num_complex::Complex64;

use crate::support::complexutil::{c_to_polar, p_to_complex, to_polar};

use super::{NUM_UPFC_VARIABLES, Upfc};

impl Upfc {
    /// Pascal `TUPFCObj.CalcUPFCLosses(Vpu)` — the active-power loss factor read
    /// off the loss curve at the per-unit input voltage.
    fn calc_upfc_losses(&mut self, vpu: f64) -> f64 {
        match &mut self.loss_curve_obj {
            Some(c) => c.get_y_value(vpu),
            // Pascal dereferences `UPFCLossCurveObj` unconditionally; a deck without
            // `losscurve=` would crash there. No corpus path hits that, so we treat
            // an absent curve as unity loss rather than reproduce the NIL crash.
            None => 1.0,
        }
    }

    /// Pascal `TUPFCObj.CalcUPFCPowers(ModeUP, Cond)` — the dual/StatCOM power used
    /// by the reactive-compensation input-current branches. `cond` is 0-based.
    fn calc_upfc_powers(&mut self, mode_up: i32, cond: usize) -> Complex64 {
        let jxs = Complex64::new(0.0, self.xs);
        match mode_up {
            1 => {
                // Dual mode.
                self.iupfc = (self.vbout - self.vbin) / jxs;
                -self.vbin * (self.iupfc + self.sr1[cond]).conj()
            }
            2 => {
                // StatCOM.
                self.iupfc = (self.vbin - self.vbout) / jxs;
                self.vbin * self.iupfc.conj()
            }
            _ => Complex64::ZERO,
        }
    }

    /// Pascal `TUPFCObj.GetOutputCurr(Cond)` — the 5-mode series-injection current
    /// dispatcher (updates the `Sr0` shift register + `UPFCON`/`SyncFlag`/`SF2`
    /// control flags). `cond` is 0-based.
    fn get_output_curr(&mut self, cond: usize) -> Complex64 {
        let jxs = Complex64::new(0.0, self.xs);
        self.upfcon = true;
        let vin_mag = self.vbin.norm();
        if vin_mag > self.vh_limit || vin_mag < self.vl_limit {
            // Check Limits (Voltage) — out of bounds, UPFC off.
            self.upfcon = false;
            return Complex64::ZERO;
        }

        // Limits OK — dispatch on the control mode.
        match self.mode_upfc {
            0 => Complex64::ZERO, // UPFC off
            1 => {
                // UPFC as a voltage regulator.
                let vpolar = c_to_polar(self.vbout);
                let error = (1.0 - (vpolar.mag / (self.v_ref * 1000.0)).abs()).abs();
                if error > self.tol1 {
                    let curr_out = self.vreg_curr_out(cond, jxs);
                    self.sr0[cond] = curr_out;
                    curr_out
                } else {
                    self.sr0[cond]
                }
            }
            2 => Complex64::ZERO, // UPFC as a phase angle regulator
            3 => {
                // Dual mode (voltage + phase angle regulator).
                let vpolar = c_to_polar(self.vbout);
                let error = (1.0 - (vpolar.mag / (self.v_ref * 1000.0)).abs()).abs();
                if error > self.tol1 {
                    let curr_out = self.vreg_curr_out(cond, jxs);
                    self.sr0[cond] = curr_out;
                    self.sync_flag = false;
                    curr_out
                } else {
                    self.sync_flag = true;
                    self.sr0[cond]
                }
            }
            4 => {
                // Double reference control mode (only voltage control).
                let vpolar = c_to_polar(self.vbin);
                let ref_h = (self.v_ref * 1000.0) + (self.v_ref * 1000.0 * self.tol1);
                let ref_l = (self.v_ref2 * 1000.0) - (self.v_ref2 * 1000.0 * self.tol1);
                if vpolar.mag > ref_h || vpolar.mag < ref_l {
                    if vpolar.mag > ref_h {
                        self.v_ref_d = self.v_ref;
                    } else if vpolar.mag < ref_l {
                        self.v_ref_d = self.v_ref2;
                    }
                    let vpolar = c_to_polar(self.vbout);
                    let error = (1.0 - (vpolar.mag / (self.v_ref_d * 1000.0)).abs()).abs();
                    let curr_out = if error > self.tol1 {
                        let c = self.dyn_ref_curr_out(cond, jxs);
                        self.sr0[cond] = c;
                        c
                    } else {
                        self.sr0[cond]
                    };
                    self.sf2 = true; // Normal control routine
                    curr_out
                } else {
                    let curr_out = Complex64::ZERO; // UPFC off
                    self.sr0[cond] = curr_out;
                    self.sf2 = false; // Says to the other controller to do nothing
                    curr_out
                }
            }
            5 => {
                // Double reference control mode (Dual mode).
                let vpolar = c_to_polar(self.vbin);
                let ref_h = (self.v_ref * 1000.0) + (self.v_ref * 1000.0 * self.tol1);
                let ref_l = (self.v_ref2 * 1000.0) - (self.v_ref2 * 1000.0 * self.tol1);
                if vpolar.mag > ref_h || vpolar.mag < ref_l {
                    if vpolar.mag > ref_h {
                        self.v_ref_d = self.v_ref;
                    } else if vpolar.mag < ref_l {
                        self.v_ref_d = self.v_ref2;
                    }
                    let vpolar = c_to_polar(self.vbout);
                    let error = (1.0 - (vpolar.mag / (self.v_ref_d * 1000.0)).abs()).abs();
                    let curr_out = if error > self.tol1 {
                        let c = self.dyn_ref_curr_out(cond, jxs);
                        self.sr0[cond] = c;
                        self.sync_flag = false;
                        c
                    } else {
                        self.sync_flag = true;
                        self.sr0[cond]
                    };
                    self.sf2 = true; // Normal control routine
                    curr_out
                } else {
                    let curr_out = Complex64::ZERO; // UPFC off
                    self.sr0[cond] = curr_out;
                    self.sf2 = false;
                    self.sync_flag = false;
                    curr_out
                }
            }
            _ => Complex64::ZERO, // Control mode not recognized (DoSimpleMsg)
        }
    }

    /// The shared modes-1/3 voltage-regulator series-injection current using the
    /// **static** `VRef` (`SR0 + Vpq/(j·Xs)` with the `VpqMax`-clamped `Vpq`).
    fn vreg_curr_out(&self, cond: usize, jxs: Complex64) -> Complex64 {
        self.curr_out_for_ref(cond, jxs, self.v_ref)
    }

    /// The shared modes-4/5 series-injection current using the **dynamic** `VRefD`.
    fn dyn_ref_curr_out(&self, cond: usize, jxs: Complex64) -> Complex64 {
        self.curr_out_for_ref(cond, jxs, self.v_ref_d)
    }

    /// Pascal's common voltage-regulator series-injection body (the `if Error >
    /// Tol1` branch shared by modes 1/3/4/5): clamp `TError = VRef·1000 − |Vbin|`
    /// to `±VpqMax`, form `Vpq` at the `Vbin` angle, and return `SR0 + Vpq/(j·Xs)`.
    fn curr_out_for_ref(&self, cond: usize, jxs: Complex64, vref: f64) -> Complex64 {
        let vtemp0 = self.vbout - self.vbin;
        let vpolar = c_to_polar(self.vbin);
        let mut terror = (vref * 1000.0) - vpolar.mag;
        if terror > self.vpqmax {
            terror = self.vpqmax;
        } else if terror < -self.vpqmax {
            terror = -self.vpqmax;
        }
        let vpolar2 = to_polar(terror, vpolar.ang);
        let vpq = p_to_complex(vpolar2) - vtemp0; // Calculates Vpq
        self.sr0[cond] + vpq / jxs
    }

    /// Pascal `TUPFCObj.GetInputCurr(Cond)` — the mode-dependent input current that
    /// balances power (updates `Sr1` / `Losses` / `UPFC_Power` / `QIdeal`). `cond`
    /// is 0-based. Reads `UPFCON` set by the preceding `GetOutputCurr`.
    fn get_input_curr(&mut self, cond: usize) -> Complex64 {
        if !self.upfcon {
            return Complex64::ZERO;
        }
        match self.mode_upfc {
            0 => {
                self.upfc_power = Complex64::ZERO;
                Complex64::ZERO
            }
            1 => {
                // Voltage regulation mode.
                let ctemp = ((self.vbout / self.vbin) * self.sr0[cond].conj()).conj();
                self.losses = self.calc_upfc_losses(self.vbin.norm() / (self.v_ref * 1000.0));
                let curr_in = -Complex64::new(ctemp.re * self.losses, self.sr0[cond].im);
                self.sr1[cond] = curr_in;
                curr_in
            }
            2 => {
                // Reactive compensation mode.
                self.upfc_power = self.calc_upfc_powers(2, 0);
                let s = self.upfc_power.re.abs() / self.pf;
                self.qideal = self.upfc_power.im - (1.0 - self.pf * self.pf).sqrt() * s;
                if self.qideal > self.kvar_lim * 1000.0 {
                    self.qideal = self.kvar_lim * 1000.0;
                }
                (Complex64::new(0.0, self.qideal) / self.vbin).conj()
            }
            3 => {
                // Dual mode.
                let ctemp = ((self.vbout / self.vbin) * self.sr0[cond].conj()).conj();
                self.losses = self.calc_upfc_losses(self.vbin.norm() / (self.v_ref * 1000.0));
                let mut curr_in = -Complex64::new(ctemp.re * self.losses, self.sr0[cond].im);
                self.sr1[cond] = curr_in;
                if self.sync_flag {
                    // Compensate the reactive power.
                    self.upfc_power = self.calc_upfc_powers(1, cond);
                    let s = self.upfc_power.re.abs() / self.pf;
                    self.qideal = self.upfc_power.im - (1.0 - self.pf * self.pf).sqrt() * s;
                    if self.qideal > self.kvar_lim * 1000.0 {
                        self.qideal = self.kvar_lim * 1000.0;
                    }
                    curr_in =
                        (Complex64::new(0.0, self.qideal) / self.vbin).conj() + self.sr1[cond];
                }
                curr_in
            }
            4 => {
                // Two-band reference mode (only voltage control).
                if self.sf2 {
                    let ctemp = ((self.vbout / self.vbin) * self.sr0[cond].conj()).conj();
                    self.losses = self.calc_upfc_losses(self.vbin.norm() / (self.v_ref_d * 1000.0));
                    let curr_in = -Complex64::new(ctemp.re * self.losses, self.sr0[cond].im);
                    self.sr1[cond] = curr_in;
                    curr_in
                } else {
                    // Input voltage is OK — do nothing.
                    self.sr0[cond] = Complex64::ZERO;
                    self.upfc_power = Complex64::ZERO;
                    Complex64::ZERO
                }
            }
            5 => {
                // Two-band reference mode (Dual control mode).
                let mut curr_in = if self.sf2 {
                    let ctemp = ((self.vbout / self.vbin) * self.sr0[cond].conj()).conj();
                    self.losses = self.calc_upfc_losses(self.vbin.norm() / (self.v_ref_d * 1000.0));
                    let c = -Complex64::new(ctemp.re * self.losses, self.sr0[cond].im);
                    self.sr1[cond] = c;
                    c
                } else {
                    self.sr1[cond] = Complex64::ZERO;
                    self.upfc_power = Complex64::ZERO;
                    Complex64::ZERO
                };
                // Always corrects PF.
                if self.sync_flag {
                    self.upfc_power = self.calc_upfc_powers(1, cond);
                    let s = self.upfc_power.re.abs() / self.pf;
                    self.qideal = self.upfc_power.im - (1.0 - self.pf * self.pf).sqrt() * s;
                    if self.qideal > self.kvar_lim * 1000.0 {
                        self.qideal = self.kvar_lim * 1000.0;
                    }
                    curr_in =
                        (Complex64::new(0.0, self.qideal) / self.vbin).conj() + self.sr1[cond];
                }
                curr_in
            }
            _ => Complex64::ZERO,
        }
    }

    /// Pascal `TUPFCObj.GetInjCurrents` — the solve path: fill
    /// `self.cd.inj_current` via [`Self::compute_inj_currents`].
    pub(super) fn get_inj_currents(&mut self, node_v: &[Complex64]) {
        self.cd.inj_current = self.compute_inj_currents(node_v);
    }

    /// Pascal `TUPFCObj.GetInjCurrents` — cache `Vbin`/`Vbout` from the present
    /// terminal node voltages and **return** the input/output injections
    /// (terminal 1 = InCurr, terminal 2 = OutCurr — a pure read of the
    /// control-clocked `UploadCurrents` caches). `self.cd.inj_current` is left
    /// untouched so the reporting path stays side-effect-free (Pascal
    /// `TUPFCObj.GetCurrents` writes into the scratch `ComplexBuffer`, never
    /// `InjCurrent`); the `Vbin`/`Vbout` refresh is Pascal's own side effect,
    /// present in both paths upstream.
    pub(super) fn compute_inj_currents(&mut self, node_v: &[Complex64]) -> Vec<Complex64> {
        let nphases = self.cd.nphases;
        let mut inj = vec![Complex64::ZERO; self.cd.yorder];
        for i in 0..nphases {
            self.vbin = node_v[self.cd.node_ref[i]];
            self.vbout = node_v[self.cd.node_ref[i + nphases]];
            inj[i + nphases] = self.out_curr[i];
            inj[i] = self.in_curr[i];
        }
        inj
    }

    /// Pascal `TUPFCObj.UploadCurrents` — recompute every phase's output then input
    /// current (the order matters: `GetOutputCurr` sets `UPFCON`, which
    /// `GetInputCurr` then reads). Driven by the `UPFCControl` `DoPendingAction`.
    pub fn upload_currents(&mut self) {
        for i in 0..self.cd.nphases {
            self.out_curr[i] = self.get_output_curr(i);
            self.in_curr[i] = self.get_input_curr(i);
        }
    }

    /// Pascal `TUPFCObj.CheckStatus` — returns whether the control needs an update
    /// (drives the `UPFCControl.Sample` OR-accumulation). `mon_power` supplies the
    /// monitored element's `Power[1]` for the PF-compensation modes (None when no
    /// `Element=` is set). Side effects: refreshes `UPFCON` and (modes 4/5)
    /// `VRefD`, exactly as upstream.
    pub fn check_status(&mut self, mon_power: Option<Complex64>) -> bool {
        // Pascal `checkPF`: PF deviation of the monitored element vs target.
        let check_pf = |this: &Upfc| -> bool {
            let Some(mp) = mon_power else {
                return false;
            };
            let mon_pf = mp.re / (mp.re * mp.re + mp.im * mp.im).sqrt();
            ((this.pf - mon_pf) / this.pf).abs() > this.tol1
        };

        self.upfcon = true;
        let vin_mag = self.vbin.norm();
        if vin_mag > self.vh_limit || vin_mag < self.vl_limit {
            // Check Limits (Voltage).
            self.upfcon = false;
            return false;
        }

        // Limits OK.
        match self.mode_upfc {
            0 => false, // UPFC off
            1 => {
                // Voltage regulator.
                let vpolar = c_to_polar(self.vbout);
                let error = (1.0 - (vpolar.mag / (self.v_ref * 1000.0)).abs()).abs();
                error > self.tol1
            }
            2 => check_pf(self),
            3 => {
                // Dual mode.
                let vpolar = c_to_polar(self.vbout);
                let error = (1.0 - (vpolar.mag / (self.v_ref * 1000.0)).abs()).abs();
                if error > self.tol1 {
                    true
                } else {
                    check_pf(self)
                }
            }
            4 => {
                // Double reference control mode (only voltage control).
                let vpolar = c_to_polar(self.vbin);
                let ref_h = (self.v_ref * 1000.0) + (self.v_ref * 1000.0 * self.tol1);
                let ref_l = (self.v_ref2 * 1000.0) - (self.v_ref2 * 1000.0 * self.tol1);
                if vpolar.mag > ref_h || vpolar.mag < ref_l {
                    if vpolar.mag > ref_h {
                        self.v_ref_d = self.v_ref;
                    } else if vpolar.mag < ref_l {
                        self.v_ref_d = self.v_ref2;
                    }
                    let vpolar = c_to_polar(self.vbout);
                    let error = (1.0 - (vpolar.mag / (self.v_ref_d * 1000.0)).abs()).abs();
                    error > self.tol1
                } else {
                    false
                }
            }
            5 => {
                // Double reference control mode (Dual mode).
                let vpolar = c_to_polar(self.vbin);
                let ref_h = (self.v_ref * 1000.0) + (self.v_ref * 1000.0 * self.tol1);
                let ref_l = (self.v_ref2 * 1000.0) - (self.v_ref2 * 1000.0 * self.tol1);
                if vpolar.mag > ref_h || vpolar.mag < ref_l {
                    if vpolar.mag > ref_h {
                        self.v_ref_d = self.v_ref;
                    } else if vpolar.mag < ref_l {
                        self.v_ref_d = self.v_ref2;
                    }
                    let vpolar = c_to_polar(self.vbout);
                    let error = (1.0 - (vpolar.mag / (self.v_ref_d * 1000.0)).abs()).abs();
                    if error > self.tol1 {
                        true
                    } else {
                        check_pf(self)
                    }
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Pascal `TUPFCObj.NumVariables`.
    pub(super) fn num_variables_impl(&self) -> usize {
        NUM_UPFC_VARIABLES
    }

    /// Pascal `TUPFCObj.VariableName(i)` (1-based).
    pub(super) fn variable_name_impl(&self, i: usize) -> String {
        match i {
            1 => "ModeUPFC",
            2 => "IUPFC",
            3 => "Re{Vbin}",
            4 => "Im{Vbin}",
            5 => "Re{Vbout}",
            6 => "Im{Vbout}",
            7 => "Losses",
            8 => "P_UPFC",
            9 => "Q_UPFC",
            10 => "Qideal",
            11 => "Re{Sr0^[1]}",
            12 => "Im{Sr0^[1]}",
            13 => "Re{Sr1^[1]}",
            14 => "Im{Sr1^[1]}",
            _ => "",
        }
        .to_string()
    }

    /// Pascal `TUPFCObj.Get_Variable(i)` (1-based).
    pub(super) fn get_variable_impl(&self, i: usize) -> f64 {
        match i {
            1 => self.mode_upfc as f64,
            2 => self.iupfc.norm(),
            3 => self.vbin.re,
            4 => self.vbin.im,
            5 => self.vbout.re,
            6 => self.vbout.im,
            7 => self.losses,
            8 => self.upfc_power.re,
            9 => self.upfc_power.im,
            10 => self.qideal,
            11 => self.sr0[0].re,
            12 => self.sr0[0].im,
            13 => self.sr1[0].re,
            14 => self.sr1[0].im,
            _ => -1.0,
        }
    }

    /// Pascal `TUPFCObj.GetAllVariables(States)`.
    pub(super) fn get_all_variables_impl(&self, states: &mut [f64]) {
        for (i, s) in states.iter_mut().enumerate().take(NUM_UPFC_VARIABLES) {
            *s = self.get_variable_impl(i + 1);
        }
    }

    /// Pascal `TUPFCObj.Set_Variable(i, value)` (1-based; only Mode + Sr0/Sr1[1]).
    pub(super) fn set_variable_impl(&mut self, i: usize, value: f64) {
        match i {
            1 => self.mode_upfc = value.round() as i32,
            11 => self.sr0[0].re = value,
            12 => self.sr0[0].im = value,
            13 => self.sr1[0].re = value,
            14 => self.sr1[0].im = value,
            _ => {} // read-only / invalid index
        }
    }
}
