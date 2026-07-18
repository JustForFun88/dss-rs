//! The solve-time control loop of `TCapControlObj`: `Sample` (sense the
//! monitored quantity and decide whether the bank should switch, arming or
//! disarming the control queue) and `DoPendingAction` (execute a queued
//! open/close/step when its time arrives), plus the sensing helpers
//! (`GetControlCurrent`/`GetControlVoltage`/`TimeOfDay`).

use num_complex::Complex64;

use crate::elements::control::control_elem::{CTRL_CLOSE, CTRL_NONE, CTRL_OPEN, CtrlCtx};
use crate::elements::pd::capacitor::ControlledCapacitor;
use crate::elements::traits::CktElement;

use super::{AVGPHASES, CapControl, MAXPHASE, MINPHASE, ctrl_type, pf_1to2};

impl CapControl {
    /// Pascal `TSolutionObj.TimeOfDay(useEpsilon = true)`: normalize the
    /// simulation time to a 0:00⁺…24:00 time-of-day, wrapping past 24 h.
    fn time_of_day_eps(int_hour: i32, t: f64) -> f64 {
        let h = int_hour;
        let hour_of_day = if h > 24 { h - ((h - 1) / 24) * 24 } else { h };
        let result = hour_of_day as f64 + t / 3600.0;
        if result - 24.0 > crate::util::EPSILON {
            result - 24.0
        } else {
            result
        }
    }

    /// Pascal `GetControlCurrent`: the control current from `cbuffer` (the
    /// monitored element's terminal currents) per `FCTphase`, divided by the CT
    /// ratio. `cond_offset` is the 0-based start of the monitored terminal's
    /// conductors.
    fn get_control_current(&self, cbuffer: &[Complex64], cond_offset: usize) -> f64 {
        let nph = self.ccd.cd.nphases; // Fnphases
        match self.fct_phase {
            AVGPHASES => {
                let mut c = 0.0;
                for i in 0..nph {
                    c += cbuffer[cond_offset + i].norm();
                }
                c / nph as f64 / self.ct_ratio
            }
            MAXPHASE => {
                let mut c = 0.0_f64;
                for i in 0..nph {
                    c = c.max(cbuffer[cond_offset + i].norm());
                }
                c / self.ct_ratio
            }
            MINPHASE => {
                let mut c = 1.0e50_f64;
                for i in 0..nph {
                    c = c.min(cbuffer[cond_offset + i].norm());
                }
                c / self.ct_ratio
            }
            // Just one phase (the monitored phase) — note: Pascal uses no
            // CondOffset on this default branch.
            _ => cbuffer[(self.fct_phase as usize) - 1].norm() / self.ct_ratio,
        }
    }

    /// Pascal `GetControlVoltage`: the control voltage from `cbuffer` (the
    /// monitored element's terminal voltages) per `FPTphase`, divided by the PT
    /// ratio. The specific-phase branch uses the controlled capacitor's
    /// connection (delta ⇒ line-line difference).
    fn get_control_voltage(&self, cbuffer: &[Complex64], mon_nphases: usize, cap_conn: i32) -> f64 {
        match self.fpt_phase {
            AVGPHASES => {
                let mut v = 0.0;
                for vb in cbuffer.iter().take(mon_nphases) {
                    v += vb.norm();
                }
                v / mon_nphases as f64 / self.pt_ratio
            }
            MAXPHASE => {
                let mut v = 0.0_f64;
                for vb in cbuffer.iter().take(mon_nphases) {
                    v = v.max(vb.norm());
                }
                v / self.pt_ratio
            }
            MINPHASE => {
                let mut v = 1.0e50_f64;
                for vb in cbuffer.iter().take(mon_nphases) {
                    v = v.min(vb.norm());
                }
                v / self.pt_ratio
            }
            // Just one phase; L-L if the capacitor is delta-connected.
            _ => {
                let p = self.fpt_phase as usize; // 1-based
                if cap_conn == 1 {
                    // NextDeltaPhase uses the control's own Fnphases.
                    let mut next = p + 1;
                    if next > self.ccd.cd.nphases {
                        next = 1;
                    }
                    (cbuffer[p - 1] - cbuffer[next - 1]).norm() / self.pt_ratio
                } else {
                    cbuffer[p - 1].norm() / self.pt_ratio
                }
            }
        }
    }

    /// Pascal `TCapControlObj.Sample` — sense the monitored quantity for the
    /// control type, decide whether the bank should switch, and arm/disarm the
    /// control queue. `cap` is the controlled capacitor; `mon` the monitored
    /// element (they differ except for Time/Follow control, where `mon` is the
    /// capacitor and is not read). Ported top-to-bottom.
    ///
    /// Returns `true` when the sample raised the equivalent of Pascal's
    /// `DSS.SolutionAbort := True` (the FOLLOWCONTROL-with-no-ControlSignal
    /// path). `CtrlCtx` has no direct abort channel — like `reset_with`
    /// returning "raise SystemYChanged", the dispatcher lifts this to
    /// `ckt.solution.solution_abort` after the borrow of the control ends.
    #[must_use]
    pub(crate) fn sample(
        &mut self,
        cap: &mut dyn ControlledCapacitor,
        mon: &mut dyn CktElement,
        ctx: &mut CtrlCtx,
    ) -> bool {
        // ControlledElement.ActiveTerminalIdx := 1 (terminal 1 is implicit).
        self.present_state = if cap.is_closed() {
            CTRL_CLOSE
        } else {
            CTRL_OPEN
        };
        self.should_switch = false;

        let now = ctx.t + ctx.int_hour as f64 * 3600.0;
        let element_terminal = self.ccd.element_terminal as usize;
        let mon_nconds = mon.cd().nconds;
        let mon_nphases = mon.cd().nphases;
        let cond_offset = (element_terminal - 1) * mon_nconds;
        let mut cbuffer = vec![Complex64::ZERO; mon.cd().yorder.max(1)];

        // First, voltage override (skipped for VOLTAGECONTROL).
        if self.voverride && self.control_type != ctrl_type::VOLTAGE {
            // VoverrideBusSpecified is always reverted at parse (the bus list
            // does not yet exist — PHASE4 §WP4.7), so the GetBusVoltages variant
            // is unreachable here; sense the monitored terminal instead.
            mon.get_term_voltages(element_terminal, ctx.node_v, &mut cbuffer);
            let vtest = self.get_control_voltage(&cbuffer, mon_nphases, cap.connection());

            // Faithful to Pascal's `case PresentState of CTRL_x: if … then`;
            // a match guard would obscure the ported `case`/`if` structure.
            #[allow(clippy::collapsible_match)]
            match self.present_state {
                CTRL_OPEN => {
                    if vtest < self.vmin {
                        self.set_pending_change(CTRL_CLOSE);
                        self.should_switch = true;
                        self.voverride_event = true;
                        if self.ccd.show_event_log {
                            ctx.events.append(
                                &cap.full_name(),
                                &format!(
                                    "Low Voltage Override: {} V",
                                    crate::util::fmt_g(vtest, 8)
                                ),
                                ctx.int_hour,
                                ctx.t,
                                ctx.control_iter,
                            );
                        }
                    }
                }
                CTRL_CLOSE => {
                    if vtest > self.vmax {
                        self.set_pending_change(CTRL_OPEN);
                        self.should_switch = true;
                        self.voverride_event = true;
                        if self.ccd.show_event_log {
                            ctx.events.append(
                                &cap.full_name(),
                                &format!(
                                    "High Voltage Override: {} V",
                                    crate::util::fmt_g(vtest, 8)
                                ),
                                ctx.int_hour,
                                ctx.t,
                                ctx.control_iter,
                            );
                        }
                    }
                }
                _ => {}
            }
        }

        if !self.should_switch {
            match self.control_type {
                ctrl_type::CURRENT => {
                    mon.get_currents(ctx.sys, ctx.node_v, &mut cbuffer);
                    let curr_test = self.get_control_current(&cbuffer, cond_offset);
                    match self.present_state {
                        CTRL_OPEN => {
                            if curr_test > self.on_value {
                                self.set_pending_change(CTRL_CLOSE);
                                self.should_switch = true;
                            } else {
                                self.set_pending_change(CTRL_NONE);
                            }
                        }
                        CTRL_CLOSE => {
                            if curr_test < self.off_value {
                                self.set_pending_change(CTRL_OPEN);
                                self.should_switch = true;
                            } else if cap.available_steps() > 0 {
                                if curr_test > self.on_value {
                                    self.set_pending_change(CTRL_CLOSE);
                                    self.should_switch = true;
                                }
                            } else {
                                self.set_pending_change(CTRL_NONE);
                            }
                        }
                        _ => {}
                    }
                }
                ctrl_type::VOLTAGE => {
                    mon.get_term_voltages(element_terminal, ctx.node_v, &mut cbuffer);
                    let vtest = self.get_control_voltage(&cbuffer, mon_nphases, cap.connection());
                    match self.present_state {
                        CTRL_OPEN => {
                            if vtest < self.on_value {
                                self.set_pending_change(CTRL_CLOSE);
                                self.should_switch = true;
                            } else {
                                self.set_pending_change(CTRL_NONE);
                            }
                        }
                        CTRL_CLOSE => {
                            self.set_pending_change(CTRL_NONE);
                            if vtest > self.off_value {
                                self.set_pending_change(CTRL_OPEN);
                                self.should_switch = true;
                            } else if cap.available_steps() > 0 && vtest < self.on_value {
                                self.set_pending_change(CTRL_CLOSE);
                                self.should_switch = true;
                            }
                        }
                        _ => {}
                    }
                }
                ctrl_type::KVAR => {
                    let s = mon.terminal_power(ctx.sys, ctx.node_v, element_terminal);
                    let q = s.im * 0.001; // kvar
                    match self.present_state {
                        CTRL_OPEN => {
                            if q > self.on_value {
                                self.set_pending_change(CTRL_CLOSE);
                                self.should_switch = true;
                            } else {
                                self.set_pending_change(CTRL_NONE);
                            }
                        }
                        CTRL_CLOSE => {
                            if q < self.off_value {
                                self.set_pending_change(CTRL_OPEN);
                                self.should_switch = true;
                            } else if cap.available_steps() > 0 {
                                if q > self.on_value {
                                    self.set_pending_change(CTRL_CLOSE);
                                    self.should_switch = true;
                                }
                            } else {
                                self.set_pending_change(CTRL_NONE);
                            }
                        }
                        _ => {}
                    }
                }
                ctrl_type::TIME => {
                    let normalized_time = Self::time_of_day_eps(ctx.int_hour, ctx.t);
                    self.sample_time_control(normalized_time, cap.available_steps());
                }
                ctrl_type::PF => {
                    let s = mon.terminal_power(ctx.sys, ctx.node_v, element_terminal);
                    let pf = pf_1to2(s);
                    match self.present_state {
                        CTRL_OPEN => {
                            // Make sure we don't go too far leading.
                            if pf < self.pfon_value
                                && s.im * 0.001 > cap.total_kvar() * self.fpct_minkvar * 0.01
                            {
                                self.set_pending_change(CTRL_CLOSE);
                                self.should_switch = true;
                            } else {
                                self.set_pending_change(CTRL_NONE);
                            }
                        }
                        CTRL_CLOSE => {
                            if pf > self.pfoff_value {
                                self.set_pending_change(CTRL_OPEN);
                                self.should_switch = true;
                            } else if cap.available_steps() > 0 {
                                if pf < self.pfon_value
                                    && s.im * 0.001
                                        > cap.total_kvar() / cap.num_steps() as f64 * 0.5
                                {
                                    self.set_pending_change(CTRL_CLOSE);
                                    self.should_switch = true;
                                }
                            } else {
                                self.set_pending_change(CTRL_NONE);
                            }
                        }
                        _ => {}
                    }
                }
                ctrl_type::FOLLOW => {
                    // Pascal `Sample`'s FOLLOWCONTROL arm (`CapControl.pas`
                    // l.1151-1169). `ctrlSignalShape = NIL` does
                    // `DoSimpleMsg(...,10362)`, **`DSS.SolutionAbort := TRUE`**,
                    // then `Exit`. We reproduce all three: queue the message,
                    // `return true` so the dispatcher sets `solution_abort` (the
                    // solve then freezes — the daily/duty/yearly loops skip every
                    // remaining step on `solution_abort`), and the early return
                    // skips this control's arm/disarm block, exactly like `Exit`.
                    let Some(shape) = self.ctrl_signal_shape.as_mut() else {
                        ctx.errors.push(crate::diag::DssDiagnostic::msg(
                            format!(
                                "CapControl.{}: Type is set to \"Follow\", but not \"ControlSignal\" was provided. Aborting solution.",
                                self.ccd.cd.obj.name()
                            ),
                            Some(10362),
                        ));
                        return true;
                    };

                    // `nextState := ctrlSignalShape.GetMultAtHour(dblHour).re`:
                    // nonzero means the signal wants the bank CLOSED, zero
                    // wants it OPEN.
                    let next_state = shape.get_mult_at_hour(ctx.dbl_hour).re;
                    // `if not ((nextState <> 0) xor (PresentState = CTRL_OPEN))`
                    // — an XNOR: switch exactly when the bank's present state
                    // mismatches the signal's desired state.
                    if (next_state != 0.0) == (self.present_state == CTRL_OPEN) {
                        if self.present_state == CTRL_OPEN {
                            self.set_pending_change(CTRL_CLOSE);
                        } else {
                            self.set_pending_change(CTRL_OPEN);
                        }
                        self.should_switch = true;
                    }
                    // No `else` in Pascal: unlike every other control type here,
                    // a non-switching sample leaves `PendingChange` untouched
                    // (not reset to `CTRL_NONE`) — faithfully mirrored by
                    // simply not touching `pending_change` in that case.
                }
                _ => {}
            }
        }

        // Arm / disarm the control queue.
        if self.should_switch && !self.armed {
            let time_delay = if self.pending_change == CTRL_CLOSE {
                if (now - self.last_open_time) < self.dead_time {
                    // Delay the close until the dead time has elapsed.
                    self.on_delay
                        .max((self.dead_time + self.on_delay) - (now - self.last_open_time))
                } else {
                    self.on_delay
                }
            } else {
                self.off_delay
            };
            self.ccd.time_delay = time_delay;
            self.control_action_handle = ctx.queue.push_delay(
                ctx.int_hour,
                ctx.t,
                time_delay,
                self.pending_change,
                0,
                ctx.self_ref,
            );
            self.armed = true;
            if self.ccd.show_event_log {
                ctx.events.append(
                    &cap.full_name(),
                    &format!(
                        "**Armed**, Delay= {} sec",
                        crate::util::fmt_g(time_delay, 5)
                    ),
                    ctx.int_hour,
                    ctx.t,
                    ctx.control_iter,
                );
            }
        }

        if self.armed && self.pending_change == CTRL_NONE {
            ctx.queue.delete(self.control_action_handle);
            self.armed = false;
            if self.ccd.show_event_log {
                ctx.events.append(
                    &cap.full_name(),
                    "**Reset**",
                    ctx.int_hour,
                    ctx.t,
                    ctx.control_iter,
                );
            }
        }

        // No abort raised (only the FOLLOW-without-ControlSignal path aborts).
        false
    }

    /// Pascal `Sample`'s `TIMECONTROL` branch (factored out for readability):
    /// compare the time-of-day against the on/off window.
    fn sample_time_control(&mut self, normalized_time: f64, available_steps: i32) {
        match self.present_state {
            CTRL_OPEN => {
                let close = if self.off_value > self.on_value {
                    normalized_time >= self.on_value && normalized_time < self.off_value
                } else {
                    // OFF time is next day.
                    normalized_time >= self.on_value && normalized_time < 24.0
                };
                if close {
                    self.set_pending_change(CTRL_CLOSE);
                    self.should_switch = true;
                } else {
                    self.set_pending_change(CTRL_NONE);
                }
            }
            CTRL_CLOSE => {
                if self.off_value > self.on_value {
                    if normalized_time >= self.off_value || normalized_time < self.on_value {
                        self.set_pending_change(CTRL_OPEN);
                        self.should_switch = true;
                    } else if available_steps > 0
                        && normalized_time >= self.on_value
                        && normalized_time < self.off_value
                    {
                        self.set_pending_change(CTRL_CLOSE);
                        self.should_switch = true;
                    }
                    // (no else-reset branch in the OFF>ON close path)
                } else {
                    // OFF time is next day.
                    if normalized_time >= self.off_value && normalized_time < self.on_value {
                        self.set_pending_change(CTRL_OPEN);
                        self.should_switch = true;
                    } else if available_steps > 0
                        && normalized_time >= self.on_value
                        && normalized_time < 24.0
                    {
                        self.set_pending_change(CTRL_CLOSE);
                        self.should_switch = true;
                    } else {
                        self.set_pending_change(CTRL_NONE);
                    }
                }
            }
            _ => {}
        }
    }

    /// Pascal `TCapControlObj.DoPendingAction` — switch the controlled bank when
    /// the queued action's time arrives (open/close or step up/down), then
    /// disarm. Marks `system_y_changed` whenever the bank's admittance changes.
    pub(crate) fn do_pending_action(
        &mut self,
        cap: &mut dyn ControlledCapacitor,
        ctx: &mut CtrlCtx,
    ) {
        // ControlledElement.ActiveTerminalIdx := 1 (terminal 1 is implicit).
        match self.pending_change {
            CTRL_OPEN => {
                if cap.num_steps() == 1 {
                    if self.present_state == CTRL_CLOSE {
                        cap.set_closed(false); // open all phases
                        cap.subtract_step();
                        *ctx.system_y_changed = true;
                        if self.ccd.show_event_log {
                            ctx.events.append(
                                &cap.full_name(),
                                "**Opened**",
                                ctx.int_hour,
                                ctx.t,
                                ctx.control_iter,
                            );
                        }
                        self.present_state = CTRL_OPEN;
                        self.last_open_time = ctx.t + 3600.0 * ctx.int_hour as f64;
                    }
                } else if self.present_state == CTRL_CLOSE {
                    // Multi-step: step down (do this only if at least one closed).
                    if !cap.subtract_step() {
                        self.present_state = CTRL_OPEN;
                        cap.set_closed(false); // open all phases
                        if self.ccd.show_event_log {
                            ctx.events.append(
                                &cap.full_name(),
                                "**Opened**",
                                ctx.int_hour,
                                ctx.t,
                                ctx.control_iter,
                            );
                        }
                    } else if self.ccd.show_event_log {
                        ctx.events.append(
                            &cap.full_name(),
                            "**Step Down**",
                            ctx.int_hour,
                            ctx.t,
                            ctx.control_iter,
                        );
                    }
                    *ctx.system_y_changed = true;
                }
            }
            CTRL_CLOSE => {
                if self.present_state == CTRL_OPEN {
                    cap.set_closed(true); // close all phases
                    if self.ccd.show_event_log {
                        ctx.events.append(
                            &cap.full_name(),
                            "**Closed**",
                            ctx.int_hour,
                            ctx.t,
                            ctx.control_iter,
                        );
                    }
                    self.present_state = CTRL_CLOSE;
                    cap.add_step();
                    *ctx.system_y_changed = true;
                } else if cap.add_step() {
                    *ctx.system_y_changed = true;
                    if self.ccd.show_event_log {
                        ctx.events.append(
                            &cap.full_name(),
                            "**Step Up**",
                            ctx.int_hour,
                            ctx.t,
                            ctx.control_iter,
                        );
                    }
                }
            }
            _ => {}
        }

        self.voverride_event = false;
        self.should_switch = false;
        self.armed = false;
    }
}
