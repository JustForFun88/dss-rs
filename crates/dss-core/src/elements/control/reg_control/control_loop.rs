//! The solve-time control loop of `TRegControlObj`: `Sample` (sense the
//! regulated voltage, optionally flip reverse/cogen mode, and arm a tap change
//! on the control queue) and `DoPendingAction` (apply the queued tap change or
//! mode toggle when its time arrives), plus the decision helpers
//! (`ComputeTimeDelay`/`AtLeastOneTap`/`OneInDirectionOf`/`GetControlVoltage`/
//! `VLimitActive`).

use num_complex::Complex64;

use crate::elements::control::control_elem::CtrlCtx;
use crate::elements::pd::transformer::ControlledTransformer;
use crate::solution::{CTRLSTATIC, EVENTDRIVEN, MULTIRATE, TIMEDRIVEN};
use crate::util::EPSILON;

use super::{ACTION_REVERSE, ACTION_TAPCHANGE, MAXPHASE, MINPHASE, RegControl};

impl RegControl {
    /// Pascal `set_PendingTapChange`: store the pending change and mirror it to
    /// the debug-trace scratch.
    pub(super) fn set_pending_tap_change(&mut self, value: f64) {
        self.pending_tap_change = value;
        self.ccd.dbl_trace_param = value;
    }

    /// Pascal `VLimitActive`.
    fn vlimit_active(&self) -> bool {
        self.vlimit > 0.0
    }

    /// Pascal `ComputeTimeDelay`: fixed `Delay`, or the inverse-time form
    /// (`Delay / min(10, 2·|Vreg − Vavg|/Band)`).
    pub(super) fn compute_time_delay(&self, vavg: f64) -> f64 {
        if self.inverse_time {
            self.ccd.time_delay / 10.0_f64.min(2.0 * (self.vreg - vavg).abs() / self.bandwidth)
        } else {
            self.ccd.time_delay
        }
    }

    /// Pascal `AtLeastOneTap` (STATIC mode): change 70 % of the way but at least
    /// one tap, capped at `TapLimitPerChange`; records `LastChange`.
    fn at_least_one_tap(&mut self, proposed_change: f64, increment: f64) -> f64 {
        let mut num_taps = (0.7 * proposed_change.abs() / increment).trunc() as i32;
        if num_taps == 0 {
            num_taps = 1;
        }
        if num_taps > self.tap_limit_per_change {
            num_taps = self.tap_limit_per_change;
        }
        self.last_change = num_taps;
        if proposed_change > 0.0 {
            num_taps as f64 * increment
        } else {
            self.last_change = -num_taps;
            -(num_taps as f64) * increment
        }
    }

    /// Pascal `OneInDirectionOf`: one tap toward the pending change, decrementing
    /// `FPendingTapChange` directly (no debug-trace mirror, as in Pascal) and
    /// zeroing it once within 0.9 increments.
    fn one_in_direction_of(&mut self, increment: f64) -> f64 {
        self.last_change = 0;
        let result = if self.pending_tap_change > 0.0 {
            self.last_change = 1;
            self.pending_tap_change -= increment;
            increment
        } else {
            self.last_change = -1;
            self.pending_tap_change += increment;
            -increment
        };
        if self.pending_tap_change.abs() < 0.9 * increment {
            self.pending_tap_change = 0.0;
        }
        result
    }

    /// Pascal `GetControlVoltage`: pick the regulated phase per `PTphase`
    /// (specific / `max` / `min`) and divide by the PT ratio. `vbuffer` and the
    /// stored `controlled_phase` are 0-based (Pascal is 1-based).
    fn get_control_voltage(
        &mut self,
        vbuffer: &[Complex64],
        nphs: usize,
        pt_ratio: f64,
    ) -> Complex64 {
        match self.fpt_phase {
            MAXPHASE => {
                let mut cp = 0;
                let mut v = vbuffer[0].norm();
                for (i, val) in vbuffer.iter().enumerate().take(nphs).skip(1) {
                    if val.norm() > v {
                        v = val.norm();
                        cp = i;
                    }
                }
                self.controlled_phase = cp;
                vbuffer[cp] / pt_ratio
            }
            MINPHASE => {
                let mut cp = 0;
                let mut v = vbuffer[0].norm();
                for (i, val) in vbuffer.iter().enumerate().take(nphs).skip(1) {
                    if val.norm() < v {
                        v = val.norm();
                        cp = i;
                    }
                }
                self.controlled_phase = cp;
                vbuffer[cp] / pt_ratio
            }
            // Specific phase (most controls): FPTphase is a 1-based phase.
            _ => {
                let cp = (self.fpt_phase - 1).max(0) as usize;
                self.controlled_phase = cp;
                vbuffer[cp] / pt_ratio
            }
        }
    }

    /// Pascal `TRegControlObj.Sample` — sense the regulated voltage, optionally
    /// flip reverse/cogen mode, and (if out of band) compute `PendingTapChange`
    /// and arm an `ACTION_TAPCHANGE` on the control queue. Ported top-to-bottom.
    pub(crate) fn sample(&mut self, tr: &mut dyn ControlledTransformer, ctx: &mut CtrlCtx) {
        if self.tap_limit_per_change == 0 {
            self.set_pending_tap_change(0.0);
            return;
        }

        // Always looking forward in cogen mode.
        let looking_forward = (!self.in_reverse_mode) || self.in_cogen_mode;
        let element_terminal = self.ccd.element_terminal as usize;
        let tap_winding = self.tap_winding as usize;
        let nphases = self.ccd.cd.nphases;

        // 1) Reverse / cogen power-direction handling (not for regulated bus).
        if !self.using_regulated_bus && (self.is_reversible || self.cogen_enabled) {
            if looking_forward && !self.in_cogen_mode {
                let fwd_power = -tr.power_into_re(element_terminal, ctx.node_v, ctx.sys);
                if !self.reverse_pending && fwd_power < -self.rev_power_threshold {
                    self.reverse_pending = true;
                    self.rev_handle = ctx.queue.push_delay(
                        ctx.int_hour,
                        ctx.t,
                        self.rev_delay,
                        ACTION_REVERSE,
                        0,
                        ctx.self_ref,
                    );
                }
                if self.reverse_pending && fwd_power >= -self.rev_power_threshold {
                    self.reverse_pending = false; // Reset it if power goes back
                    if self.rev_handle > 0 {
                        ctx.queue.delete(self.rev_handle);
                        self.rev_handle = 0;
                    }
                }
            } else {
                // Looking the reverse direction or in cogen mode.
                let fwd_power = -tr.power_into_re(element_terminal, ctx.node_v, ctx.sys);
                if !self.reverse_pending && fwd_power > self.rev_power_threshold {
                    self.reverse_pending = true;
                    self.rev_back_handle = ctx.queue.push_delay(
                        ctx.int_hour,
                        ctx.t,
                        self.rev_delay,
                        ACTION_REVERSE,
                        0,
                        ctx.self_ref,
                    );
                }
                if self.reverse_pending && fwd_power <= self.rev_power_threshold {
                    self.reverse_pending = false;
                    if self.rev_back_handle > 0 {
                        ctx.queue.delete(self.rev_back_handle);
                        self.rev_back_handle = 0;
                    }
                }
                // Reverse-neutral special case: drive the tap to neutral (1.0).
                if self.reverse_neutral {
                    if !self.armed {
                        self.set_pending_tap_change(0.0);
                        let present = tr.present_tap(tap_winding);
                        if (present - 1.0).abs() > EPSILON {
                            let increment = tr.tap_increment(tap_winding);
                            // TODO(compat): FPC banker's `Round`.
                            let ptc = ((1.0 - present) / increment).round_ties_even() * increment;
                            self.set_pending_tap_change(ptc);
                            if self.pending_tap_change != 0.0 && !self.armed {
                                ctx.queue.push_delay(
                                    ctx.int_hour,
                                    ctx.t,
                                    self.tap_delay,
                                    ACTION_TAPCHANGE,
                                    0,
                                    ctx.self_ref,
                                );
                                self.armed = true;
                            }
                        }
                    }
                    return; // Done in any case if reverse-neutral specified.
                }
            }
        }

        // 2) Control voltage.
        let mut vbuffer = vec![Complex64::ZERO; nphases];
        let mut vcontrol = if self.using_regulated_bus {
            let conn = tr.wdg_connection(element_terminal);
            self.ccd.cd.compute_vterminal(ctx.node_v); // voltage at the regulated bus
            for (i, vb) in vbuffer.iter_mut().enumerate().take(nphases) {
                match conn {
                    0 => *vb = self.ccd.cd.vterminal[i], // Wye
                    1 => {
                        // Delta: next phase in sequence.
                        let ii = tr.rotate_phases(i + 1) - 1;
                        *vb = self.ccd.cd.vterminal[i] - self.ccd.cd.vterminal[ii];
                    }
                    _ => ctx.errors.push(format!(
                        "{}: Series connection used in \"Transformer.{}\" has not been implemented or tested!",
                        self.ccd.cd.obj.name(),
                        tr.name()
                    )),
                }
            }
            self.get_control_voltage(&vbuffer, nphases, self.remote_pt_ratio)
        } else {
            tr.winding_voltages(element_terminal, ctx.node_v, &mut vbuffer);
            self.get_control_voltage(&vbuffer, nphases, self.pt_ratio)
        };

        // 3) Vlimit local-bus voltage.
        let vlimit_active = self.vlimit_active();
        let vlocalbus = if vlimit_active {
            if self.using_regulated_bus {
                tr.winding_voltages(element_terminal, ctx.node_v, &mut vbuffer);
                (vbuffer[0] / self.pt_ratio).norm()
            } else {
                vcontrol.norm()
            }
        } else {
            0.0
        };

        // 4) Line-drop compensation.
        if !self.using_regulated_bus && self.ldc_active {
            let nconds = tr.n_conds();
            let mut cbuffer = vec![Complex64::ZERO; tr.y_order()];
            tr.terminal_currents(ctx.node_v, ctx.sys, &mut cbuffer);
            let ildc =
                cbuffer[nconds * (element_terminal - 1) + self.controlled_phase] / self.ct_rating;
            if self.ldc_z == 0.0 {
                // Standard R + jX LDC; ILDC is INTO the terminal → Vterm − (R+jX)·I.
                let vldc = if self.in_reverse_mode || self.in_cogen_mode {
                    Complex64::new(self.rev_r, self.rev_x) * ildc
                } else {
                    Complex64::new(self.r, self.x) * ildc
                };
                vcontrol += vldc;
            } else {
                // Beckwith LDC_Z mode: magnitudes only.
                let z = if self.in_reverse_mode || self.in_cogen_mode {
                    self.rev_ldc_z
                } else {
                    self.ldc_z
                };
                vcontrol = Complex64::new(vcontrol.norm() - ildc.norm() * z, 0.0);
            }
        }

        let mut vactual = vcontrol.norm(); // assumes looking forward; adjusted below

        // 5) Out-of-band test.
        let (vreg_test, band_test) = if self.in_reverse_mode {
            vactual /= tr.present_tap(tap_winding);
            (self.rev_vreg, self.rev_bandwidth)
        } else if self.in_cogen_mode {
            (self.rev_vreg, self.rev_bandwidth)
        } else {
            (self.vreg, self.bandwidth)
        };
        let mut tap_change_needed = (vreg_test - vactual).abs() > band_test / 2.0;
        if vlimit_active && vlocalbus > self.vlimit {
            tap_change_needed = true;
        }

        if tap_change_needed {
            let mut vboost = vreg_test - vactual;
            if vlimit_active && vlocalbus > self.vlimit {
                vboost = self.vlimit - vlocalbus;
            }
            // Per-unit winding boost needed.
            let boost_needed = vboost * self.pt_ratio / tr.base_voltage(element_terminal);
            let increment = tr.tap_increment(tap_winding);
            // TODO(compat): FPC banker's `Round` — this single line decides
            // tap-position equality; `round_ties_even` reproduces it.
            let mut ptc = (boost_needed / increment).round_ties_even() * increment;
            // A tap on another winding or in REVERSE moves the opposite way.
            if (self.tap_winding != self.ccd.element_terminal) || self.in_reverse_mode {
                ptc = -ptc;
            }
            self.set_pending_tap_change(ptc);

            if self.pending_tap_change != 0.0 && !self.armed {
                let present = tr.present_tap(tap_winding);
                // Only arm if a tap change is actually possible in that direction.
                let possible = if self.pending_tap_change > 0.0 {
                    present < tr.max_tap(tap_winding)
                } else {
                    present > tr.min_tap(tap_winding)
                };
                if possible {
                    let delay = self.compute_time_delay(vactual);
                    self.control_action_handle = ctx.queue.push_delay(
                        ctx.int_hour,
                        ctx.t,
                        delay,
                        ACTION_TAPCHANGE,
                        0,
                        ctx.self_ref,
                    );
                    self.armed = true; // Armed to change taps
                }
            }
        } else {
            // Back in band: reset.
            self.set_pending_tap_change(0.0);
            if self.armed {
                ctx.queue.delete(self.control_action_handle);
                self.armed = false;
                self.control_action_handle = 0;
            }
        }
    }

    /// Pascal `TRegControlObj.DoPendingAction` — apply the armed action when its
    /// queue time arrives. `ACTION_TAPCHANGE` applies the pending tap (per
    /// control mode); `ACTION_REVERSE` toggles reverse/cogen mode.
    pub(crate) fn do_pending_action(
        &mut self,
        code: i32,
        tr: &mut dyn ControlledTransformer,
        ctx: &mut CtrlCtx,
    ) {
        match code {
            ACTION_TAPCHANGE => {
                if self.pending_tap_change == 0.0 {
                    // Control has reset since the action was queued.
                    self.armed = false;
                    return;
                }
                let tap_winding = self.tap_winding as usize;
                let increment = tr.tap_increment(tap_winding);
                if ctx.control_mode == CTRLSTATIC {
                    let change = self.at_least_one_tap(self.pending_tap_change, increment);
                    let new_tap = tr.present_tap(tap_winding) + change;
                    if tr.set_present_tap(tap_winding, new_tap) {
                        *ctx.system_y_changed = true;
                    }
                    self.sync_tap_snap(tap_winding, tr.present_tap(tap_winding));
                    if self.ccd.show_event_log {
                        ctx.events.append(
                            &format!("Regulator.{}", tr.name()),
                            &format!(
                                " Changed {} taps to {}.",
                                self.last_change,
                                crate::util::fmt_g(tr.present_tap(tap_winding), 6)
                            ),
                            ctx.int_hour,
                            ctx.t,
                            ctx.control_iter,
                        );
                    }
                    self.set_pending_tap_change(0.0); // program re-determines need
                    self.armed = false;
                } else if matches!(ctx.control_mode, EVENTDRIVEN | TIMEDRIVEN | MULTIRATE) {
                    let change = self.one_in_direction_of(increment);
                    let new_tap = tr.present_tap(tap_winding) + change;
                    if tr.set_present_tap(tap_winding, new_tap) {
                        *ctx.system_y_changed = true;
                    }
                    self.sync_tap_snap(tap_winding, tr.present_tap(tap_winding));
                    if self.ccd.show_event_log {
                        ctx.events.append(
                            &format!("Regulator.{}", tr.name()),
                            &format!(
                                " Changed {} tap to {}.",
                                self.last_change,
                                crate::util::fmt_g(tr.present_tap(tap_winding), 6)
                            ),
                            ctx.int_hour,
                            ctx.t,
                            ctx.control_iter,
                        );
                    }
                    if self.pending_tap_change != 0.0 {
                        ctx.queue.push_delay(
                            ctx.int_hour,
                            ctx.t,
                            self.tap_delay,
                            ACTION_TAPCHANGE,
                            0,
                            ctx.self_ref,
                        );
                    } else {
                        self.armed = false;
                    }
                }
            }
            // Toggle reverse mode or cogen mode flag (only if still pending).
            ACTION_REVERSE if self.reverse_pending => {
                if self.cogen_enabled {
                    // Cogen mode takes precedence if present.
                    self.in_cogen_mode = !self.in_cogen_mode;
                } else {
                    self.in_reverse_mode = !self.in_reverse_mode;
                }
                self.reverse_pending = false;
            }
            _ => {}
        }
    }
}
