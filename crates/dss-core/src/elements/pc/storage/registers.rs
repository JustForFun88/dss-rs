//! State-of-charge integration (`ComputeDCkW` / `UpdateStorage` + the loss
//! split), the energy-meter registers (`TakeSample` / `Integrate`), and the
//! present-output accessors. The SOC update runs in the time-step cleanup hook
//! (Pascal `StorageClass.UpdateAll` → `UpdateStorage`).

use num_complex::Complex64;

use crate::elements::general::xy_curve::XyCurveObj;
use crate::elements::traits::{CktElement, SysCtx};

use super::{
    NUM_STORAGE_REGISTERS, REG_HOURS, REG_KVARH, REG_KWH, REG_MAXKVA, REG_MAXKW, REG_PRICE,
    Storage, StorageState,
};

impl Storage {
    /// Pascal `ResetRegisters`.
    pub fn reset_registers(&mut self) {
        self.registers = [0.0; NUM_STORAGE_REGISTERS];
        self.derivatives = [0.0; NUM_STORAGE_REGISTERS];
        self.base.first_sample_after_reset = true; // for trapezoidal integration
    }

    /// Pascal `Integrate`.
    fn integrate(&mut self, reg: usize, deriv: f64, interval: f64, trapezoidal: bool) {
        if trapezoidal {
            if !self.base.first_sample_after_reset {
                self.registers[reg] += 0.5 * interval * (deriv + self.derivatives[reg]);
            }
        } else {
            self.registers[reg] += interval * deriv;
        }
        self.derivatives[reg] = deriv;
    }

    /// Pascal `SetDragHandRegister`.
    fn set_drag_hand_register(&mut self, reg: usize, value: f64) {
        if value > self.registers[reg] {
            self.registers[reg] = value;
        }
    }

    /// Pascal `Get_PresentkW`.
    pub fn present_kw(&self) -> f64 {
        self.base.p_nominal_per_phase * 0.001 * self.cd.nphases as f64
    }

    /// Pascal `Get_Presentkvar` (on the inverter base).
    pub fn present_kvar(&self) -> f64 {
        self.base.get_present_kvar(self.cd.nphases)
    }

    /// Pascal `Get_PresentkV` — the rated kV (`kVStorageBase`). Read by the
    /// StorageController's current-mode (I-PeakShave) kW conversion.
    pub fn present_kv(&self) -> f64 {
        self.kv_storage_base
    }

    /// The DC-side kW that updates the state of charge: the signed AC terminal
    /// kW of an ideal inverter (no efficiency curve), else the DC power
    /// [`dc_power_on_curve`] solves from the AC power, false with the DC power
    /// kept where no segment of the curve holds the solution. Only the time
    /// step refuses the element for it ([`Self::update_storage`]), so a reader
    /// of the DC power changes nothing else (pinned by
    /// `exec::tests::rating_refusals::the_dc_power_of_an_efficiency_curve_is_solved_on_the_segment_that_holds_it`).
    fn compute_dckw(&mut self, sys: &SysCtx, node_v: &[Complex64]) -> bool {
        let p1 = self.terminal_power(sys, node_v, 1).re * 0.001; // Power[1] in kW
        let dc = match &self.base.inverter_curve_obj {
            None => Some(p1),
            Some(curve) => dc_power_on_curve(
                curve,
                p1.abs(),
                self.f_kva_rating,
                self.f_state == StorageState::Discharging,
            ),
        };
        let Some(dc) = dc else {
            return false;
        };
        // Make sure the sign is correct.
        self.f_dckw = if self.f_state == StorageState::Idling {
            -dc.abs()
        } else {
            dc.abs() * self.f_state.ordinal() as f64
        };
        true
    }

    /// The DC power the time step updates the state of charge with, `None`
    /// while the element is refused. Where no segment of the efficiency curve
    /// holds it, the element is refused from the next Y build on, naming the
    /// curve.
    fn step_dckw(&mut self, sys: &SysCtx, node_v: &[Complex64]) -> Option<f64> {
        let solved = self.compute_dckw(sys, node_v);
        if !solved && self.cd.recalc_refusal.is_none() {
            let input = format!("EffCurve={}", self.base.inverter_curve);
            let msg = self.cd.refusal_message("Storage", &input, "dc power");
            self.cd.recalc_refusal = Some(msg);
            self.cd.yprim_invalid = true;
        }
        (solved && self.cd.recalc_refusal.is_none()).then_some(self.f_dckw)
    }

    /// Pascal `Get_kWIdlingLosses`.
    pub(super) fn kw_idling_losses(&mut self, sys: &SysCtx, node_v: &[Complex64]) -> f64 {
        if self.f_state == StorageState::Idling {
            self.dckw(sys, node_v).abs() // consistency with voltage variations
        } else {
            self.p_idling
        }
    }

    /// The `DCkW` variable, recomputed from the present AC power, the DC power
    /// kept where the efficiency curve holds none. Used by the InvControl
    /// Storage volt-watt `Calc_PBase` `%Available` base
    /// (`InvDispEnv::der_storage_dckw`), hence `pub(crate)`.
    pub(crate) fn dckw(&mut self, sys: &SysCtx, node_v: &[Complex64]) -> f64 {
        self.compute_dckw(sys, node_v);
        self.f_dckw
    }

    /// Pascal `UpdateStorage`: advance the state of charge by one interval, based
    /// on the present DC power and the idling/charge/discharge efficiencies.
    /// Runs in the time-step cleanup hook with a solved circuit.
    pub fn update_storage(&mut self, sys: &SysCtx, node_v: &[Complex64], interval_hrs: f64) {
        self.kwh_before_update = self.kwh_stored; // keep for the "kWh Chng" variable

        // Pascal: `if IsDynamicModel and IsUserModel then Exit` (Storage.pas:2502).
        // `IsUserModel` = `UserModel.Exists or DynaModel.Exists` (`:854`/`:862`);
        // in dynamics mode with a loaded user model the model owns the state, so
        // the built-in SOC update is skipped (WASM_USERMODELS WM.4).
        if sys.is_dynamic_model && (self.user_model_exists() || self.dyna_model_exists()) {
            return;
        }

        match self.f_state {
            StorageState::Discharging => {
                // Pascal Storage.pas l.2500-2523: `UpdateSt := TRUE; if GFM_Mode
                // then UpdateSt := CheckIfDelivering()`. A GFM inverter that is
                // actually *absorbing* (not delivering) recharges instead of
                // discharging (the `else` branch, capped at `kWhRating`) — the
                // dynamics-mode GFM black-start (WPG.17) can pass through this.
                let update_st = if self.base.gfm_mode {
                    self.check_if_delivering(sys, node_v)
                } else {
                    true
                };
                let Some(dckw) = self.step_dckw(sys, node_v) else {
                    return;
                };
                let idle = self.kw_idling_losses(sys, node_v);
                if update_st {
                    self.kwh_stored -= (dckw + idle) / self.discharge_eff * interval_hrs;
                } else {
                    // Absorbing power → recharge, capped at the rating.
                    self.kwh_stored += (dckw + idle) / self.discharge_eff * interval_hrs;
                    if self.kwh_stored > self.kwh_rating {
                        self.kwh_stored = self.kwh_rating;
                    }
                }
                // Check we still have enough energy to deliver.
                if self.kwh_stored < self.kwh_reserve {
                    self.kwh_stored = self.kwh_reserve;
                    self.f_state = StorageState::Idling; // empty — turn it off
                    self.state_changed = true;
                    self.base.gfm_mode = false;
                }
            }
            StorageState::Charging => {
                let Some(dckw) = self.step_dckw(sys, node_v) else {
                    return;
                };
                let idle = self.kw_idling_losses(sys, node_v);
                if (dckw.abs() - idle) >= 0.0 {
                    // 99.9% of cases.
                    self.kwh_stored += (dckw.abs() - idle) * self.charge_eff * interval_hrs;
                    if self.kwh_stored > self.kwh_rating {
                        self.kwh_stored = self.kwh_rating;
                        self.f_state = StorageState::Idling; // full — turn it off
                        self.state_changed = true;
                        self.base.gfm_mode = false;
                    }
                } else {
                    // Idling losses exceed DCkW → the ideal storage discharges.
                    self.kwh_stored += (dckw.abs() - idle) / self.discharge_eff * interval_hrs;
                    if self.kwh_stored < self.kwh_reserve {
                        self.kwh_stored = self.kwh_reserve;
                        self.f_state = StorageState::Idling;
                        self.state_changed = true;
                    }
                }
            }
            _ => {} // idling: SOC unchanged
        }

        // The update is at the end of a time step, so force a Yprim re-calc for
        // the next step (else the state-dependent Y would stay stale).
        if self.state_changed {
            self.cd.yprim_invalid = true;
        }
    }

    /// Pascal `TakeSample`: accumulate the Storage energy registers. Only
    /// discharge hours are tabulated.
    pub fn take_sample(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
        interval_hrs: f64,
        trapezoidal: bool,
    ) {
        if !self.cd.enabled {
            return;
        }
        // Only tabulate discharge hours.
        let (mut s, mut smag, hour_value) = if self.f_state == StorageState::Discharging {
            let _ = (sys, node_v);
            let s = Complex64::new(self.present_kw(), self.present_kvar());
            (s, s.norm(), 1.0)
        } else {
            (Complex64::ZERO, 0.0, 0.0)
        };

        if self.f_state == StorageState::Discharging || trapezoidal {
            if sys.positive_sequence {
                s *= 3.0;
                smag *= 3.0;
            }
            self.integrate(REG_KWH, s.re, interval_hrs, trapezoidal);
            self.integrate(REG_KVARH, s.im, interval_hrs, trapezoidal);
            self.set_drag_hand_register(REG_MAXKW, s.re.abs());
            self.set_drag_hand_register(REG_MAXKVA, smag);
            self.integrate(REG_HOURS, hour_value, interval_hrs, trapezoidal);
            self.integrate(
                REG_PRICE,
                s.re * sys.price_signal * 0.001,
                interval_hrs,
                trapezoidal,
            );
            self.base.first_sample_after_reset = false;
        }
    }
}

/// The DC power an inverter rated `kva` exchanges for the AC power `p >= 0`
/// on the efficiency curve, `None` where no segment of the curve holds its
/// own solution. Each segment gives the efficiency `η = a x + b` at the per
/// unit DC power `x = P / kva`, discharging delivers `p = η P` and charging
/// (or idling) stores `P = η p`, so each segment is solved in closed form and
/// the first one, in curve order, whose solution lies on it gives the power.
/// A solution that rounding puts just past a knot of a segment across which
/// the relation changes sign is taken at that knot. A curve of one point is
/// its efficiency at every power, as every reader of a curve takes it.
pub(super) fn dc_power_on_curve(
    curve: &XyCurveObj,
    p: f64,
    kva: f64,
    discharging: bool,
) -> Option<f64> {
    if p == 0.0 {
        return Some(0.0);
    }
    if curve.num_points() == 1 {
        let eta = curve.y_values()[0];
        let dc = if discharging { p / eta } else { p * eta };
        return (dc.is_finite() && dc >= 0.0).then_some(dc);
    }
    let segments = curve.num_points().checked_sub(1).filter(|&k| k > 0)?;
    let (xs, ys) = (curve.x_values(), curve.y_values());
    // The relation at a per-unit DC power `x` read at efficiency `eta`, zero
    // at the solution and rising through it.
    let residual = |x: f64, eta: f64| {
        if discharging {
            kva * x * eta - p
        } else {
            kva * x - eta * p
        }
    };
    for k in 0..segments {
        let (a, b) = curve.segment_line(k);
        let lo = if k == 0 { 0.0 } else { xs[k].max(0.0) };
        let hi = if k + 1 == segments {
            f64::INFINITY
        } else {
            xs[k + 1]
        };
        if hi < lo {
            continue;
        }
        let roots: Vec<f64> = if discharging {
            let qa = a / kva;
            if qa == 0.0 {
                if b != 0.0 { vec![p / b] } else { Vec::new() }
            } else {
                let mid = (b * b - 4.0 * qa * -p).sqrt();
                let (r1, r2) = ((-b + mid) / (2.0 * qa), (-b - mid) / (2.0 * qa));
                vec![r1.min(r2), r1.max(r2)]
            }
        } else {
            let den = 1.0 - a * p / kva;
            if den != 0.0 {
                vec![p * b / den]
            } else {
                Vec::new()
            }
        };
        let roots: Vec<f64> = roots
            .into_iter()
            .filter(|r| r.is_finite() && *r >= 0.0)
            .collect();
        if let Some(&r) = roots.iter().find(|&&r| (lo..=hi).contains(&(r / kva))) {
            return Some(r);
        }
        let r_lo = if k > 0 && xs[k] >= 0.0 {
            residual(xs[k], ys[k])
        } else {
            residual(0.0, b)
        };
        let r_hi = if hi.is_finite() {
            residual(xs[k + 1], ys[k + 1])
        } else if discharging {
            [a, b, -p].into_iter().find(|&c| c != 0.0).unwrap_or(0.0)
        } else {
            [kva - a * p, -b * p]
                .into_iter()
                .find(|&c| c != 0.0)
                .unwrap_or(0.0)
        };
        if r_lo <= 0.0 && r_hi >= 0.0 {
            let distance = |r: f64| {
                let x = r / kva;
                (lo - x).max(x - hi).max(0.0)
            };
            if let Some(r) = roots
                .into_iter()
                .min_by(|r, s| distance(*r).total_cmp(&distance(*s)))
            {
                return Some((r / kva).clamp(lo, hi) * kva);
            }
        }
    }
    None
}
