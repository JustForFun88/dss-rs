//! The per-sub-type sensing logic for `TRelayObj` (r4133) — one function per
//! `ControlType` that `Sample` dispatches to. Each reads the monitored element's
//! solved state (currents / terminal voltages / terminal power), evaluates its
//! protection characteristic, and arms/queues `OPEN`/`CLOSE`/`RESET` actions.
//!
//! **r4133 (WP-U2.3):** the overcurrent logic is a per-phase state machine with a
//! single-phase (`SinglePhTrip`) and a three-phase (ganged) branch, closed-phase
//! sampling-gate (B4), `MaxOperatingCount` curve selection, single-count inst trip
//! (D3), and descriptive relay targets (E2). The other sub-types are ganged-only:
//! they drive the `IdxMultiPh` slot of the per-phase arrays (B1) but their sensing
//! logic is unchanged from r4088 (verified by the r4088↔r4133 method diff — only
//! the `^[IdxMultiPh]` indexing + property renames moved). `VoltageLogic` gains
//! closed-phase-only OV/UV extrema (B2).

use num_complex::Complex64;

use crate::elements::control::control_elem::{CTRL_CLOSE, CTRL_OPEN, CTRL_RESET, CtrlCtx};
use crate::elements::traits::CktElement;
use crate::support::complexutil::{cdang, pdeg_to_complex};
use crate::support::dynamics::IterationFlag;
use crate::support::mathutil::SymComp;

use super::Relay;

/// Pascal global `EPSILON = 1.0e-12` (DSSGlobals.pas) — the inst-vs-curve
/// discriminator for the relay-target label.
const EPSILON: f64 = 1.0e-12;

impl Relay {
    /// 0-based `(cond_offset, nphases)` for the monitored terminal.
    fn mon_offset(&self, mon: &dyn CktElement) -> (usize, usize) {
        let nconds = mon.cd().nconds;
        let cond_offset = (self.monitored_element_terminal.max(1) as usize - 1) * nconds;
        (cond_offset, mon.cd().nphases)
    }

    /// `Debug Sample: Relay.<name>` trace line, gated on `DebugTrace`.
    fn dbg_sample(&self, ctx: &mut CtrlCtx, msg: &str) {
        let el = format!("Debug Sample: Relay.{}", self.ccd.cd.obj.name());
        self.dbg(ctx, &el, msg);
    }

    /// Build the descriptive overcurrent `RelayTarget` (Pascal `OvercurrentLogic`):
    /// a combination of the ground and phase trip causes, discriminating
    /// instantaneous / definite-time / curve by `Abs(time - 0.01) < EPSILON` and
    /// `time = DefiniteTimeDelay`.
    fn build_oc_target(&self, trip_time: f64, ground_time: f64, phase_time: f64) -> String {
        let mut s = String::new();
        if trip_time == ground_time {
            if (ground_time - 0.01).abs() < EPSILON {
                s = "Gnd Instantaneous".to_string();
            } else if ground_time == self.definite_time_delay {
                s = "Gnd Definite Time".to_string();
            } else {
                s = "Gnd Curve".to_string();
            }
        }
        if trip_time == phase_time {
            if !s.is_empty() {
                s.push_str(" + ");
            }
            if (phase_time - 0.01).abs() < EPSILON {
                s.push_str("Ph Instantaneous");
            } else if phase_time == self.definite_time_delay {
                s.push_str("Ph Definite Time");
            } else {
                s.push_str("Ph Curve");
            }
        }
        s
    }

    /// Pascal `TRelayObj.OvercurrentLogic` (r4133) — phase + ground TCC (50/51),
    /// with a definite-time path and a first-operation instantaneous trip, split
    /// into a per-phase single-phase branch and the ganged three-phase branch.
    pub(super) fn overcurrent_logic(&mut self, mon: &mut dyn CktElement, ctx: &mut CtrlCtx) {
        let (cond_offset, nphases) = self.mon_offset(mon);
        let n = self.state_size();
        let mut cbuffer = vec![Complex64::ZERO; mon.cd().yorder.max(1)];
        mon.get_currents(ctx.sys, ctx.node_v, &mut cbuffer);
        // Pascal `cBuffer` is 1-based (`cBuffer^[i+CondOffset]`); keep 1-based math.
        let cur = |i: usize| cbuffer.get(i - 1).copied().unwrap_or(Complex64::ZERO);

        // B4: continue sampling if ≥1 phase closed, else exit.
        if !(1..=n).any(|i| self.present_state[i] == CTRL_CLOSE) {
            return;
        }

        // Identify number of operations (MaxOperatingCount).
        let g = self.idx_multi_ph;
        let max_op = if self.single_ph_trip {
            let mut m: Option<i32> = None;
            for i in 1..=n {
                if self.locked_out[i] {
                    continue; // skip locked-out phases
                }
                m = Some(match m {
                    None => self.operation_count[i],
                    Some(prev) => prev.max(self.operation_count[i]),
                });
            }
            m.unwrap_or(self.operation_count[g])
        } else {
            self.operation_count[g]
        };

        // ---- Ground trip (shared) ----
        let mut ground_time = -1.0_f64;
        if (self.ground_curve.is_some() || self.definite_time_delay > 0.0) && self.ground_trip > 0.0
        {
            let mut csum = Complex64::ZERO;
            for i in (1 + cond_offset)..=(nphases + cond_offset) {
                csum += cur(i);
            }
            let cmag = csum.norm();
            if self.ground_inst > 0.0 && cmag >= self.ground_inst && max_op == 1 {
                ground_time = 0.01; // D3: bare inst on first operation
                self.dbg_sample(
                    ctx,
                    &format!("Gnd Instantaneous Trip: Mag={cmag:.3}, Time={ground_time:.3}"),
                );
            } else if self.definite_time_delay > 0.0 {
                if cmag >= self.ground_trip {
                    ground_time = self.definite_time_delay;
                    self.dbg_sample(
                        ctx,
                        &format!("Gnd Definite Time Trip: Mag={cmag:.3}, Time={ground_time:.3}"),
                    );
                }
            } else if let Some(gc) = self.ground_curve.as_mut() {
                ground_time = self.td_ground * gc.get_tcc_time(cmag / self.ground_trip);
                if ground_time > 0.0 {
                    let m = cmag / self.ground_trip;
                    self.dbg_sample(
                        ctx,
                        &format!("Gnd Curve Trip: Mag={m:.3}, Time={ground_time:.3}"),
                    );
                }
            }
        }
        if ground_time > 0.0 {
            self.ground_target = true;
        }

        if self.single_ph_trip {
            self.overcurrent_single_phase(ctx, cond_offset, n, ground_time, &cur);
        } else {
            self.overcurrent_three_phase(ctx, cond_offset, nphases, max_op, ground_time, &cur);
        }
    }

    /// The single-phase branch of `OvercurrentLogic` (Pascal `if SinglePhTrip`).
    fn overcurrent_single_phase(
        &mut self,
        ctx: &mut CtrlCtx,
        cond_offset: usize,
        n: usize,
        ground_time: f64,
        cur: &dyn Fn(usize) -> Complex64,
    ) {
        for i in 1..=n {
            if self.present_state[i] != CTRL_CLOSE {
                continue;
            }
            let mut trip_time = if ground_time > 0.0 { ground_time } else { -1.0 };
            let mut phase_time = -1.0_f64;

            if (self.phase_curve.is_some() || self.definite_time_delay > 0.0)
                && self.phase_trip > 0.0
            {
                let cmag = cur(i + cond_offset).norm();
                if self.phase_inst > 0.0 && cmag >= self.phase_inst && self.operation_count[i] == 1
                {
                    phase_time = 0.01; // D3: bare inst
                    self.dbg_sample(
                        ctx,
                        &format!(
                            "Ph Instantaneous (1-Phase) Trip: Phase={i}, Mag={cmag:.3}, Time={phase_time:.3}"
                        ),
                    );
                } else {
                    let time_test = if self.definite_time_delay > 0.0 {
                        if cmag >= self.phase_trip {
                            self.dbg_sample(
                                ctx,
                                &format!(
                                    "Ph Definite Time (1-Phase) Trip: Phase={i}, Mag={cmag:.3}, Time={:.3}",
                                    self.definite_time_delay
                                ),
                            );
                            self.definite_time_delay
                        } else {
                            -1.0
                        }
                    } else if let Some(pc) = self.phase_curve.as_mut() {
                        self.td_phase * pc.get_tcc_time(cmag / self.phase_trip)
                    } else {
                        -1.0
                    };
                    if time_test > 0.0 {
                        phase_time = time_test;
                        if self.definite_time_delay <= 0.0 {
                            let m = cmag / self.phase_trip;
                            self.dbg_sample(
                                ctx,
                                &format!(
                                    "Ph Curve (1-Phase) Trip: Phase={i}, Mag={m:.3}, Time={phase_time:.3}"
                                ),
                            );
                        }
                    }
                }
            }
            if phase_time > 0.0 {
                self.phase_target[i] = true;
                trip_time = if trip_time > 0.0 {
                    trip_time.min(phase_time)
                } else {
                    phase_time
                };
            }

            if trip_time > 0.0 {
                if !self.armed_for_open[i] {
                    self.relay_target[i] = self.build_oc_target(trip_time, ground_time, phase_time);
                    self.last_event_handle = ctx.queue.push_delay(
                        ctx.int_hour,
                        ctx.t,
                        trip_time + self.mechanical_delay,
                        CTRL_OPEN,
                        i as i32,
                        ctx.self_ref,
                    );
                    if self.operation_count[i] <= self.num_reclose {
                        let interval = self
                            .reclose_intervals
                            .get((self.operation_count[i] - 1).max(0) as usize)
                            .copied()
                            .unwrap_or(0.0);
                        ctx.queue.push_delay(
                            ctx.int_hour,
                            ctx.t,
                            trip_time + self.mechanical_delay + interval,
                            CTRL_CLOSE,
                            i as i32,
                            ctx.self_ref,
                        );
                    }
                    self.armed_for_open[i] = true;
                    self.armed_for_close[i] = true;
                }
            } else if self.armed_for_open[i] {
                self.last_event_handle = ctx.queue.push_delay(
                    ctx.int_hour,
                    ctx.t,
                    self.reset_time,
                    CTRL_RESET,
                    i as i32,
                    ctx.self_ref,
                );
                self.armed_for_open[i] = false;
                self.armed_for_close[i] = false;
                self.ground_target = false;
                self.phase_target[i] = false;
            }
        }
    }

    /// The three-phase (ganged) branch of `OvercurrentLogic` (Pascal `else`).
    fn overcurrent_three_phase(
        &mut self,
        ctx: &mut CtrlCtx,
        cond_offset: usize,
        nphases: usize,
        max_op: i32,
        ground_time: f64,
        cur: &dyn Fn(usize) -> Complex64,
    ) {
        let g = self.idx_multi_ph;
        let mut trip_time = if ground_time > 0.0 { ground_time } else { -1.0 };
        let mut phase_time = -1.0_f64;

        if (self.phase_curve.is_some() || self.definite_time_delay > 0.0) && self.phase_trip > 0.0 {
            for i in (1 + cond_offset)..=(nphases + cond_offset) {
                let cmag = cur(i).norm();
                if self.phase_inst > 0.0 && cmag >= self.phase_inst && self.operation_count[g] == 1
                {
                    phase_time = 0.01; // D3: bare inst
                    self.dbg_sample(
                        ctx,
                        &format!(
                            "Ph Instantaneous (3-Phase) Trip: Phase={}, Mag={cmag:.3}, Time={phase_time:.3}",
                            i - cond_offset
                        ),
                    );
                    break; // if inst, no sense checking other phases
                }
                if self.definite_time_delay > 0.0 {
                    if cmag >= self.phase_trip {
                        phase_time = self.definite_time_delay;
                        self.dbg_sample(
                            ctx,
                            &format!(
                                "Ph Definite Time (3-Phase) Trip: Phase={}, Mag={cmag:.3}, Time={phase_time:.3}",
                                i - cond_offset
                            ),
                        );
                        break; // if definite time, no sense checking other phases
                    }
                } else {
                    let time_test = if let Some(pc) = self.phase_curve.as_mut() {
                        self.td_phase * pc.get_tcc_time(cmag / self.phase_trip)
                    } else {
                        -1.0
                    };
                    if time_test > 0.0 {
                        let m = cmag / self.phase_trip;
                        self.dbg_sample(
                            ctx,
                            &format!(
                                "Ph Curve (3-Phase) Trip: Phase={}, Mag={m:.3}, Time={time_test:.3}",
                                i - cond_offset
                            ),
                        );
                        phase_time = if phase_time < 0.0 {
                            time_test
                        } else {
                            phase_time.min(time_test)
                        };
                    }
                }
            }
        }

        if phase_time > 0.0 {
            self.phase_target[g] = true;
            trip_time = if trip_time > 0.0 {
                trip_time.min(phase_time)
            } else {
                phase_time
            };
        }

        if trip_time > 0.0 {
            if !self.armed_for_open[g] {
                self.relay_target[g] = self.build_oc_target(trip_time, ground_time, phase_time);
                self.last_event_handle = ctx.queue.push_delay(
                    ctx.int_hour,
                    ctx.t,
                    trip_time + self.mechanical_delay,
                    CTRL_OPEN,
                    0,
                    ctx.self_ref,
                );
                if max_op <= self.num_reclose {
                    let interval = self
                        .reclose_intervals
                        .get((max_op - 1).max(0) as usize)
                        .copied()
                        .unwrap_or(0.0);
                    ctx.queue.push_delay(
                        ctx.int_hour,
                        ctx.t,
                        trip_time + self.mechanical_delay + interval,
                        CTRL_CLOSE,
                        0,
                        ctx.self_ref,
                    );
                }
                self.armed_for_open[g] = true;
                self.armed_for_close[g] = true;
            }
        } else if self.armed_for_open[g] {
            self.last_event_handle = ctx.queue.push_delay(
                ctx.int_hour,
                ctx.t,
                self.reset_time,
                CTRL_RESET,
                0,
                ctx.self_ref,
            );
            self.armed_for_open[g] = false;
            self.armed_for_close[g] = false;
            self.ground_target = false;
            self.phase_target[g] = false;
        }
    }

    /// Pascal `TRelayObj.VoltageLogic` (r4133) — definite-time over/under voltage
    /// (27/59) with a voltage-recovery reclose. **B2:** the OV/UV trip evaluates the
    /// extrema over *closed* phases only (`Vmax_closed`/`Vmin_closed`); the reclose
    /// voltage check still uses all-phase `Vmax`. Ganged-only (drives `IdxMultiPh`).
    pub(super) fn voltage_logic(&mut self, mon: &mut dyn CktElement, ctx: &mut CtrlCtx) {
        let g = self.idx_multi_ph;
        if self.locked_out[g] {
            return;
        }
        let n = self.state_size();
        let mut vbuffer = vec![Complex64::ZERO; mon.cd().nconds.max(1)];
        mon.get_term_voltages(
            self.monitored_element_terminal.max(1) as usize,
            ctx.node_v,
            &mut vbuffer,
        );

        let mut vmin = 1.0e50_f64;
        let mut vmax = 0.0_f64;
        let mut vmin_closed = 1.0e50_f64;
        let mut vmax_closed = 0.0_f64;
        let mut vmag = -1.0_f64;
        for i in 1..=n {
            vmag = vbuffer
                .get(i - 1)
                .copied()
                .unwrap_or(Complex64::ZERO)
                .norm();
            if self.present_state[i] == CTRL_CLOSE {
                if vmag > vmax_closed {
                    vmax_closed = vmag;
                }
                if vmag < vmin_closed {
                    vmin_closed = vmag;
                }
            }
            if vmag > vmax {
                vmax = vmag;
            }
            if vmag < vmin {
                vmin = vmag;
            }
        }
        // Convert to per-unit.
        vmax /= self.vbase;
        let _ = vmin / self.vbase;
        vmax_closed /= self.vbase;
        vmin_closed /= self.vbase;

        let mut trip_time = -1.0_f64;
        let mut ov_time = -1.0_f64;
        let mut uv_time = -1.0_f64;
        if vmag > 0.0
            && let Some(c) = self.ov_curve.as_ref()
        {
            ov_time = c.get_ov_time(vmax_closed);
        }
        if ov_time > 0.0 {
            trip_time = ov_time;
            self.dbg(
                ctx,
                &self.full_name(),
                &format!("OV (3-Phase) Trip: Mag={vmax_closed:.3}, Time={ov_time:.3}"),
            );
        }
        if vmag > 0.0
            && let Some(c) = self.uv_curve.as_ref()
        {
            uv_time = c.get_uv_time(vmin_closed);
        }
        if uv_time > 0.0 {
            trip_time = if trip_time > 0.0 {
                trip_time.min(uv_time)
            } else {
                uv_time
            };
            self.dbg(
                ctx,
                &self.full_name(),
                &format!("UV (3-Phase) Trip: Mag={vmin_closed:.3}, Time={uv_time:.3}"),
            );
        }

        if trip_time > 0.0 {
            let candidate = ctx.t + trip_time + self.mechanical_delay;
            if self.armed_for_open[g] && candidate < self.next_trip_time {
                ctx.queue.delete(self.last_event_handle);
                self.armed_for_open[g] = false; // force it through the next IF
            }
            if !self.armed_for_open[g] {
                self.relay_target[g] = if trip_time == uv_time {
                    if trip_time == ov_time {
                        "UV + OV"
                    } else {
                        "UV"
                    }
                } else {
                    "OV"
                }
                .to_string();
                self.next_trip_time = ctx.t + trip_time + self.mechanical_delay;
                self.last_event_handle = ctx.queue.push(
                    ctx.int_hour,
                    self.next_trip_time,
                    CTRL_OPEN,
                    0,
                    ctx.self_ref,
                );
                self.armed_for_open[g] = true;
            }
        } else if trip_time < 0.0 && self.armed_for_open[g] {
            ctx.queue.delete(self.last_event_handle);
            self.next_trip_time = -1.0;
            self.last_event_handle = ctx.queue.push_delay(
                ctx.int_hour,
                ctx.t,
                self.reset_time,
                CTRL_RESET,
                0,
                ctx.self_ref,
            );
            self.armed_for_open[g] = false;
        }

        // Reclose check — all phases must be open.
        if (1..=n).any(|i| self.present_state[i] == CTRL_CLOSE) {
            return;
        }
        if self.operation_count[g] <= self.num_reclose {
            if !self.armed_for_close[g] {
                if vmax > 0.9 {
                    let interval = self
                        .reclose_intervals
                        .get((self.operation_count[g] - 1).max(0) as usize)
                        .copied()
                        .unwrap_or(0.0);
                    self.last_event_handle = ctx.queue.push_delay(
                        ctx.int_hour,
                        ctx.t,
                        interval,
                        CTRL_CLOSE,
                        0,
                        ctx.self_ref,
                    );
                    self.armed_for_close[g] = true;
                }
            } else if vmax < 0.9 {
                self.armed_for_close[g] = false;
            }
        }
    }

    /// Pascal `TRelayObj.RevPowerLogic` — one-shot reverse-power lockout (32).
    /// Ganged-only.
    pub(super) fn rev_power_logic(&mut self, mon: &mut dyn CktElement, ctx: &mut CtrlCtx) {
        let g = self.idx_multi_ph;
        let s = mon.terminal_power(
            ctx.sys,
            ctx.node_v,
            self.monitored_element_terminal.max(1) as usize,
        );
        if s.re < 0.0 {
            if s.re.abs() > self.phase_inst * 1000.0 {
                if !self.armed_for_open[g] {
                    self.relay_target[g] = "Rev P".to_string();
                    self.last_event_handle = ctx.queue.push_delay(
                        ctx.int_hour,
                        ctx.t,
                        self.definite_time_delay + self.mechanical_delay,
                        CTRL_OPEN,
                        0,
                        ctx.self_ref,
                    );
                    self.operation_count[g] = self.num_reclose + 1; // force a lockout
                    self.armed_for_open[g] = true;
                }
            } else if self.armed_for_open[g] {
                self.last_event_handle = self.push_reset(ctx);
                self.armed_for_open[g] = false;
            }
        }
    }

    /// Pascal `TRelayObj.NegSeq46Logic` — negative-sequence current, one-shot to
    /// lockout. Ganged-only.
    pub(super) fn neg_seq46_logic(&mut self, mon: &mut dyn CktElement, ctx: &mut CtrlCtx) {
        let g = self.idx_multi_ph;
        let (cond_offset, _) = self.mon_offset(mon);
        let mut cbuffer = vec![Complex64::ZERO; mon.cd().yorder.max(1)];
        mon.get_currents(ctx.sys, ctx.node_v, &mut cbuffer);
        let phases = [
            cbuffer.get(cond_offset).copied().unwrap_or(Complex64::ZERO),
            cbuffer
                .get(cond_offset + 1)
                .copied()
                .unwrap_or(Complex64::ZERO),
            cbuffer
                .get(cond_offset + 2)
                .copied()
                .unwrap_or(Complex64::ZERO),
        ];
        let mut i012 = [Complex64::ZERO; 3];
        SymComp::default().phase_to_sym(&phases, &mut i012);
        let neg_seq_mag = i012[2].norm();

        if neg_seq_mag >= self.pickup_amps46 {
            if !self.armed_for_open[g] {
                self.relay_target[g] = "-Seq Curr".to_string();
                let trip_time = if self.definite_time_delay > 0.0 {
                    self.definite_time_delay
                } else {
                    let ratio = neg_seq_mag / self.base_amps46;
                    self.isqt46 / (ratio * ratio)
                };
                self.last_event_handle = ctx.queue.push_delay(
                    ctx.int_hour,
                    ctx.t,
                    trip_time + self.mechanical_delay,
                    CTRL_OPEN,
                    0,
                    ctx.self_ref,
                );
                self.operation_count[g] = self.num_reclose + 1;
                self.armed_for_open[g] = true;
            }
        } else if self.armed_for_open[g] {
            self.last_event_handle = self.push_reset(ctx);
            self.armed_for_open[g] = false;
        }
    }

    /// Pascal `TRelayObj.NegSeq47Logic` — negative-sequence voltage, one-shot to
    /// lockout. Ganged-only.
    pub(super) fn neg_seq47_logic(&mut self, mon: &mut dyn CktElement, ctx: &mut CtrlCtx) {
        let g = self.idx_multi_ph;
        let mut vbuffer = vec![Complex64::ZERO; mon.cd().nconds.max(3)];
        mon.get_term_voltages(
            self.monitored_element_terminal.max(1) as usize,
            ctx.node_v,
            &mut vbuffer,
        );
        let phases = [vbuffer[0], vbuffer[1], vbuffer[2]];
        let mut v012 = [Complex64::ZERO; 3];
        SymComp::default().phase_to_sym(&phases, &mut v012);
        let neg_seq_mag = v012[2].norm();

        if neg_seq_mag >= self.pickup_volts47 {
            if !self.armed_for_open[g] {
                self.relay_target[g] = "-Seq V".to_string();
                self.last_event_handle = ctx.queue.push_delay(
                    ctx.int_hour,
                    ctx.t,
                    self.definite_time_delay + self.mechanical_delay,
                    CTRL_OPEN,
                    0,
                    ctx.self_ref,
                );
                self.operation_count[g] = self.num_reclose + 1;
                self.armed_for_open[g] = true;
            }
        } else if self.armed_for_open[g] {
            self.last_event_handle = self.push_reset(ctx);
            self.armed_for_open[g] = false;
        }
    }

    /// Pascal `TRelayObj.GenericLogic` — a `Generic` relay trips (one-shot) when a
    /// monitored PC element's state variable leaves `[UnderTrip, OverTrip]`.
    /// Ganged-only.
    pub(super) fn generic_logic(&mut self, mon: &mut dyn CktElement, ctx: &mut CtrlCtx) {
        let g = self.idx_multi_ph;
        let idx = self.monitor_var_index;
        let var_value = if idx >= 1 {
            let n = mon.num_variables();
            let mut states = vec![0.0_f64; n];
            mon.get_all_variables(ctx.sys, ctx.node_v, &mut states);
            states.get((idx - 1) as usize).copied().unwrap_or(-9999.99)
        } else {
            -9999.99
        };

        if var_value > self.over_trip || var_value < self.under_trip {
            if !self.armed_for_open[g] {
                self.relay_target[g] = if idx >= 1 {
                    mon.variable_name(idx as usize)
                } else {
                    String::new()
                };
                self.last_event_handle = ctx.queue.push_delay(
                    ctx.int_hour,
                    ctx.t,
                    self.definite_time_delay + self.mechanical_delay,
                    CTRL_OPEN,
                    0,
                    ctx.self_ref,
                );
                self.operation_count[g] = self.num_reclose + 1;
                self.armed_for_open[g] = true;
            }
        } else if self.armed_for_open[g] {
            self.last_event_handle = self.push_reset(ctx);
            self.armed_for_open[g] = false;
        }
    }

    /// Pascal `TRelayObj.DistanceLogic` — mho-style loop-impedance reach (21).
    /// Ganged-only.
    pub(super) fn distance_logic(&mut self, mon: &mut dyn CktElement, ctx: &mut CtrlCtx) {
        let g = self.idx_multi_ph;
        if self.locked_out[g] {
            return;
        }
        let (cond_offset, nphases) = self.mon_offset(mon);
        let mut cbuffer = vec![Complex64::ZERO; mon.cd().yorder.max(1)];
        mon.get_currents(ctx.sys, ctx.node_v, &mut cbuffer);
        if self.dist_reverse {
            for p in 0..nphases {
                if let Some(c) = cbuffer.get_mut(cond_offset + p) {
                    *c = -*c;
                }
            }
        }
        let cur = |p: usize| {
            cbuffer
                .get(cond_offset + p)
                .copied()
                .unwrap_or(Complex64::ZERO)
        };
        let mut ires = Complex64::ZERO;
        for p in 0..nphases {
            ires += cur(p);
        }
        let k_ires = self.dist_k0 * ires;

        let mut cvbuffer = vec![Complex64::ZERO; mon.cd().nconds.max(1)];
        mon.get_term_voltages(
            self.monitored_element_terminal.max(1) as usize,
            ctx.node_v,
            &mut cvbuffer,
        );
        let cv = |p: usize| cvbuffer.get(p).copied().unwrap_or(Complex64::ZERO);

        let mut min_distance = 1.0e30_f64;
        let mut targets: Vec<String> = Vec::new();
        let mut picked_up = false;
        for i in 0..nphases {
            for j in i..nphases {
                let (vloop, iloop, zreach) = if i == j {
                    (cv(i), cur(i) + k_ires, self.dist_z1 * self.mground)
                } else {
                    (cv(i) - cv(j), cur(i) - cur(j), self.dist_z1 * self.mphase)
                };
                let i2 = iloop.re * iloop.re + iloop.im * iloop.im;
                if i2 > 0.1 {
                    let zloop = vloop / iloop;
                    if zloop.re >= 0.0
                        && zloop.im >= super::MIN_DISTANCE_REACTANCE
                        && zloop.re <= zreach.re
                        && zloop.im <= zreach.im
                    {
                        if i == j {
                            targets.push(format!("G{}", i + 1));
                        } else {
                            targets.push(format!("P{}{}", i + 1, j + 1));
                        }
                        let fault_distance = zloop.norm_sqr() / zreach.norm_sqr();
                        if fault_distance < min_distance {
                            min_distance = fault_distance;
                        }
                        picked_up = true;
                    }
                }
            }
        }

        if picked_up {
            if self.armed_for_reset[g] {
                ctx.queue.delete(self.last_event_handle);
                self.armed_for_reset[g] = false;
            }
            if !self.armed_for_open[g] {
                targets.sort();
                self.relay_target[g] = format!("21 {min_distance:.3} pu dist");
                for tgt in &targets {
                    self.relay_target[g].push(' ');
                    let t = tgt.clone();
                    self.relay_target[g].push_str(&t);
                }
                self.last_event_handle = ctx.queue.push_delay(
                    ctx.int_hour,
                    ctx.t,
                    self.definite_time_delay + self.mechanical_delay,
                    CTRL_OPEN,
                    0,
                    ctx.self_ref,
                );
                self.armed_for_open[g] = true;
                if self.operation_count[g] <= self.num_reclose {
                    let interval = self
                        .reclose_intervals
                        .get((self.operation_count[g] - 1).max(0) as usize)
                        .copied()
                        .unwrap_or(0.0);
                    self.last_event_handle = ctx.queue.push_delay(
                        ctx.int_hour,
                        ctx.t,
                        self.definite_time_delay + self.mechanical_delay + interval,
                        CTRL_CLOSE,
                        0,
                        ctx.self_ref,
                    );
                    self.armed_for_close[g] = true;
                }
            }
        } else {
            if self.operation_count[g] > 1 && !self.armed_for_reset[g] {
                self.armed_for_reset[g] = true;
                self.last_event_handle = self.push_reset(ctx);
            }
            if self.armed_for_open[g] {
                self.armed_for_open[g] = false;
                self.armed_for_close[g] = false;
            }
        }
    }

    /// Pascal `TRelayObj.TD21Logic` — the differential (incremental) time-distance
    /// relay (21). Ganged-only. Returns `true` when the coarse-time-step guard
    /// (error 388) fires.
    pub(super) fn td21_logic(&mut self, mon: &mut dyn CktElement, ctx: &mut CtrlCtx) -> bool {
        let g = self.idx_multi_ph;
        let (cond_offset, nphases) = self.mon_offset(mon);
        let dt = ctx.sys.dyna_h;
        let mut solution_abort = false;
        if dt > 0.0 {
            if dt > 1.0 / ctx.sys.frequency {
                ctx.errors.push(format!(
                    "Relay: \"{}\": Has type TD21 with time step greater than one cycle. \
                     Reduce time step, or change type to Distance. (Error 388)",
                    self.ccd.cd.obj.name()
                ));
                solution_abort = true;
            }
            let i = (1.0 / 60.0 / dt + 0.5).round_ties_even() as i32;
            if i > self.td21_pt {
                self.td21_i = 0;
                self.td21_pt = i;
                self.td21_quiet = self.td21_pt + 1;
                self.td21_stride = 2 * nphases as i32;
                let (pt, stride) = (self.td21_pt as usize, self.td21_stride as usize);
                self.td21_h = vec![Complex64::ZERO; stride * pt];
                self.td21_dv = vec![Complex64::ZERO; nphases];
                self.td21_uref = vec![Complex64::ZERO; nphases];
                self.td21_di = vec![Complex64::ZERO; nphases];
            }
        }

        if self.locked_out[g] {
            return solution_abort;
        }

        let mut cbuffer = vec![Complex64::ZERO; mon.cd().yorder.max(1)];
        mon.get_currents(ctx.sys, ctx.node_v, &mut cbuffer);
        if self.dist_reverse {
            for p in 0..nphases {
                if let Some(c) = cbuffer.get_mut(cond_offset + p) {
                    *c = -*c;
                }
            }
        }
        let cur = |p: usize| {
            cbuffer
                .get(cond_offset + p)
                .copied()
                .unwrap_or(Complex64::ZERO)
        };
        let i2fault = self.phase_trip * self.phase_trip;
        let mut fault_detected = false;
        for p in 0..nphases {
            if cur(p).norm_sqr() > i2fault {
                fault_detected = true;
            }
        }

        let mut cvbuffer = vec![Complex64::ZERO; mon.cd().nconds.max(1)];
        mon.get_term_voltages(
            self.monitored_element_terminal.max(1) as usize,
            ctx.node_v,
            &mut cvbuffer,
        );
        let cv = |p: usize| cvbuffer.get(p).copied().unwrap_or(Complex64::ZERO);

        if self.td21_pt < 1 {
            return solution_abort;
        }
        let stride = self.td21_stride as usize;

        if self.td21_i < 1 {
            for s in 0..(self.td21_pt as usize) {
                let ib = s * stride;
                for p in 0..nphases {
                    self.td21_h[ib + p] = cv(p);
                    self.td21_h[ib + nphases + p] = cur(p);
                }
            }
            self.td21_i = 1;
        }

        self.td21_next = (self.td21_i % self.td21_pt) + 1;

        let ib = (self.td21_next - 1) as usize * stride;
        for p in 0..nphases {
            self.td21_uref[p] = self.td21_h[ib + p];
            self.td21_dv[p] = cv(p) - self.td21_h[ib + p];
            self.td21_di[p] = cur(p) - self.td21_h[ib + nphases + p];
        }

        if ctx.sys.iteration_flag == IterationFlag::NewTimeStep {
            let ib = (self.td21_i - 1) as usize * stride;
            for p in 0..nphases {
                self.td21_h[ib + p] = cv(p);
                self.td21_h[ib + nphases + p] = cur(p);
            }
            self.td21_i = self.td21_next;
            if self.td21_quiet > 0 {
                self.td21_quiet -= 1;
            }
        }

        if self.td21_quiet > 0 {
            return solution_abort;
        }

        let mut picked_up = false;
        let mut min_distance = 1.0e30_f64;
        let mut ires = Complex64::ZERO;
        for p in 0..nphases {
            ires += self.td21_di[p];
        }
        let k_ires = self.dist_k0 * ires;

        let mut targets: Vec<String> = Vec::new();
        for i in 0..nphases {
            for j in i..nphases {
                let (uref, vloop, iloop, zhsd) = if i == j {
                    (
                        self.td21_uref[i],
                        self.td21_dv[i],
                        self.td21_di[i] + k_ires,
                        self.dist_z1 * self.mground,
                    )
                } else {
                    (
                        self.td21_uref[i] - self.td21_uref[j],
                        self.td21_dv[i] - self.td21_dv[j],
                        self.td21_di[i] - self.td21_di[j],
                        self.dist_z1 * self.mphase,
                    )
                };
                let i2 = iloop.norm_sqr();
                let uref2 = uref.norm_sqr();
                if fault_detected && i2 > 0.1 && uref2 > 0.1 {
                    let zdir = -(vloop / iloop);
                    if zdir.re > 0.0 && zdir.im > 0.0 {
                        let uhsd = zhsd * iloop - vloop;
                        let uhsd2 = uhsd.norm_sqr();
                        if uhsd2 / uref2 > 1.0 {
                            if i == j {
                                targets.push(format!("G{}", i + 1));
                            } else {
                                targets.push(format!("P{}{}", i + 1, j + 1));
                            }
                            let fault_distance = 1.0 / (uhsd2 / uref2).sqrt();
                            if fault_distance < min_distance {
                                min_distance = fault_distance;
                            }
                            picked_up = true;
                        }
                    }
                }
            }
        }

        if picked_up {
            if self.armed_for_reset[g] {
                ctx.queue.delete(self.last_event_handle);
                self.armed_for_reset[g] = false;
            }
            if !self.armed_for_open[g] {
                targets.sort();
                self.relay_target[g] = format!("TD21 {min_distance:.3} pu dist");
                for tgt in &targets {
                    self.relay_target[g].push(' ');
                    let t = tgt.clone();
                    self.relay_target[g].push_str(&t);
                }
                self.last_event_handle = ctx.queue.push_delay(
                    ctx.int_hour,
                    ctx.t,
                    self.definite_time_delay + self.mechanical_delay,
                    CTRL_OPEN,
                    0,
                    ctx.self_ref,
                );
                self.armed_for_open[g] = true;
                if self.operation_count[g] <= self.num_reclose {
                    let interval = self
                        .reclose_intervals
                        .get((self.operation_count[g] - 1).max(0) as usize)
                        .copied()
                        .unwrap_or(0.0);
                    self.last_event_handle = ctx.queue.push_delay(
                        ctx.int_hour,
                        ctx.t,
                        self.definite_time_delay + self.mechanical_delay + interval,
                        CTRL_CLOSE,
                        0,
                        ctx.self_ref,
                    );
                    self.armed_for_close[g] = true;
                }
            }
        }

        if !fault_detected {
            if self.operation_count[g] > 1 && !self.armed_for_reset[g] {
                self.armed_for_reset[g] = true;
                self.last_event_handle = self.push_reset(ctx);
            }
            if self.armed_for_open[g] {
                self.td21_quiet = self.td21_pt + 1;
                self.armed_for_open[g] = false;
                self.armed_for_close[g] = false;
            }
        }

        solution_abort
    }

    /// Pascal `TRelayObj.GetControlPower` — net positive-sequence active power
    /// (total terminal power for < 3 phases) for the DOC blocking test.
    fn get_control_power(&self, mon: &mut dyn CktElement, ctx: &mut CtrlCtx) -> Complex64 {
        let nphases = mon.cd().nphases;
        if nphases < 3 {
            return mon.terminal_power(
                ctx.sys,
                ctx.node_v,
                self.monitored_element_terminal.max(1) as usize,
            );
        }
        let nconds = mon.cd().nconds;
        let cond_offset = (self.monitored_element_terminal.max(1) as usize - 1) * nconds;
        let mut cbuffer = vec![Complex64::ZERO; mon.cd().yorder.max(1)];
        mon.get_currents(ctx.sys, ctx.node_v, &mut cbuffer);
        let mut cvbuffer = vec![Complex64::ZERO; nconds.max(3)];
        mon.get_term_voltages(
            self.monitored_element_terminal.max(1) as usize,
            ctx.node_v,
            &mut cvbuffer,
        );
        let iph = [
            cbuffer.get(cond_offset).copied().unwrap_or(Complex64::ZERO),
            cbuffer
                .get(cond_offset + 1)
                .copied()
                .unwrap_or(Complex64::ZERO),
            cbuffer
                .get(cond_offset + 2)
                .copied()
                .unwrap_or(Complex64::ZERO),
        ];
        let vph = [cvbuffer[0], cvbuffer[1], cvbuffer[2]];
        let mut i012 = [Complex64::ZERO; 3];
        let mut v012 = [Complex64::ZERO; 3];
        let sc = SymComp::default();
        sc.phase_to_sym(&iph, &mut i012);
        sc.phase_to_sym(&vph, &mut v012);
        (v012[1] * i012[1].conj()) * 0.003
    }

    /// Pascal `TRelayObj.DirectionalOvercurrentLogic` (DOC). Ganged-only.
    pub(super) fn directional_overcurrent_logic(
        &mut self,
        mon: &mut dyn CktElement,
        ctx: &mut CtrlCtx,
    ) {
        let g = self.idx_multi_ph;
        let (cond_offset, nphases) = self.mon_offset(mon);
        let n = self.state_size();
        // B4: continue sampling if ≥1 phase closed, else exit.
        if !(1..=n).any(|i| self.present_state[i] == CTRL_CLOSE) {
            return;
        }
        if self.doc_p1_blocking {
            let control_power = self.get_control_power(mon, ctx);
            if control_power.re >= 0.0 {
                if self.armed_for_open[g] {
                    self.last_event_handle = self.push_reset(ctx);
                    self.armed_for_open[g] = false;
                    self.armed_for_close[g] = false;
                    self.dbg(
                        ctx,
                        &self.full_name(),
                        &format!(
                            "DOC - Reset on Forward Net Balanced Active Power: {:.2} kW",
                            control_power.re
                        ),
                    );
                } else {
                    self.dbg(
                        ctx,
                        &self.full_name(),
                        &format!(
                            "DOC - Forward Net Balanced Active Power: {:.2} kW. DOC Element blocked.",
                            control_power.re
                        ),
                    );
                }
                return;
            }
        }

        let mut cbuffer = vec![Complex64::ZERO; mon.cd().yorder.max(1)];
        mon.get_currents(ctx.sys, ctx.node_v, &mut cbuffer);
        let mut cvbuffer = vec![Complex64::ZERO; mon.cd().nconds.max(1)];
        mon.get_term_voltages(
            self.monitored_element_terminal.max(1) as usize,
            ctx.node_v,
            &mut cvbuffer,
        );

        let mut cb = vec![Complex64::ZERO; nphases];
        for (p, slot) in cb.iter_mut().enumerate() {
            let icur = cbuffer
                .get(cond_offset + p)
                .copied()
                .unwrap_or(Complex64::ZERO);
            let vph = cvbuffer.get(p).copied().unwrap_or(Complex64::ZERO);
            *slot = pdeg_to_complex(icur.norm(), cdang(icur) - cdang(vph));
        }

        let mut trip_time = -1.0_f64;
        for phasor in cb {
            let time_test = self.doc_phase_time_test(phasor, phasor.norm());
            if time_test >= 0.0 {
                trip_time = if trip_time < 0.0 {
                    time_test
                } else {
                    trip_time.min(time_test)
                };
            }
        }

        if trip_time >= 0.0 {
            if !self.armed_for_open[g] {
                self.relay_target[g] = "DOC".to_string();
                self.last_event_handle = ctx.queue.push_delay(
                    ctx.int_hour,
                    ctx.t,
                    trip_time + self.mechanical_delay,
                    CTRL_OPEN,
                    0,
                    ctx.self_ref,
                );
                if self.operation_count[g] <= self.num_reclose {
                    let interval = self
                        .reclose_intervals
                        .get((self.operation_count[g] - 1).max(0) as usize)
                        .copied()
                        .unwrap_or(0.0);
                    ctx.queue.push_delay(
                        ctx.int_hour,
                        ctx.t,
                        trip_time + self.mechanical_delay + interval,
                        CTRL_CLOSE,
                        0,
                        ctx.self_ref,
                    );
                }
                self.armed_for_open[g] = true;
                self.armed_for_close[g] = true;
            }
        } else if self.armed_for_open[g] {
            self.last_event_handle = self.push_reset(ctx);
            self.armed_for_open[g] = false;
            self.armed_for_close[g] = false;
        }
    }

    // --- DOC characteristic decision tree (Pascal nested if/else, factored) ---

    /// Top-level DOC tilt-low decision. `pub(super)` so `tests` can drive it.
    pub(super) fn doc_phase_time_test(&mut self, cb: Complex64, cmag: f64) -> f64 {
        if self.doc_tilt_angle_low == 90.0 || self.doc_tilt_angle_low == 270.0 {
            if cb.re <= -self.doc_trip_set_low {
                self.doc_circle_decision(cb, cmag)
            } else {
                -1.0
            }
        } else if cb.im
            < self.doc_tilt_angle_low.to_radians().tan() * (cb.re + self.doc_trip_set_low)
        {
            self.doc_circle_decision(cb, cmag)
        } else if self.doc_trip_set_high > 0.0 {
            let left = if self.doc_tilt_angle_high == 90.0 || self.doc_tilt_angle_high == 270.0 {
                cb.re < -self.doc_trip_set_high
            } else {
                cb.im
                    < self.doc_tilt_angle_high.to_radians().tan() * (cb.re + self.doc_trip_set_high)
            };
            if left {
                self.doc_outer_time(cmag)
            } else {
                -1.0
            }
        } else {
            -1.0
        }
    }

    fn doc_circle_decision(&mut self, cb: Complex64, cmag: f64) -> f64 {
        if self.doc_trip_set_mag > 0.0 {
            if cmag <= self.doc_trip_set_mag {
                if self.doc_trip_set_high > 0.0 {
                    self.doc_highline_specified(cb, cmag)
                } else {
                    self.doc_inner_region_time(cmag)
                }
            } else {
                self.doc_outer_time(cmag)
            }
        } else if self.doc_trip_set_high > 0.0 {
            self.doc_highline_specified(cb, cmag)
        } else {
            self.doc_outer_time(cmag)
        }
    }

    fn doc_highline_specified(&mut self, cb: Complex64, cmag: f64) -> f64 {
        let left = if self.doc_tilt_angle_high == 90.0 || self.doc_tilt_angle_high == 270.0 {
            cb.re < -self.doc_trip_set_high
        } else {
            cb.im < self.doc_tilt_angle_high.to_radians().tan() * (cb.re + self.doc_trip_set_high)
        };
        if left {
            self.doc_outer_time(cmag)
        } else {
            self.doc_inner_region_time(cmag)
        }
    }

    fn doc_outer_time(&mut self, cmag: f64) -> f64 {
        if self.definite_time_delay > 0.0 {
            return self.definite_time_delay;
        }
        let (td, trip) = (self.td_phase, self.phase_trip);
        if let Some(pc) = self.phase_curve.as_mut() {
            td * pc.get_tcc_time(cmag / trip)
        } else {
            self.definite_time_delay // == 0.0
        }
    }

    fn doc_inner_region_time(&mut self, cmag: f64) -> f64 {
        if self.doc_delay_inner > 0.0 {
            return self.doc_delay_inner;
        }
        let (td, trip, delay) = (
            self.doc_td_phase_inner,
            self.doc_phase_trip_inner,
            self.definite_time_delay,
        );
        if let Some(c) = self.doc_phase_curve_inner.as_mut() {
            td * c.get_tcc_time(cmag / trip)
        } else if self.doc_delay_inner == 0.0 {
            delay
        } else {
            -1.0
        }
    }

    /// Push a `RESET` action `ResetTime` seconds out (ganged, proxy 0); returns
    /// the handle.
    fn push_reset(&mut self, ctx: &mut CtrlCtx) -> i32 {
        ctx.queue.push_delay(
            ctx.int_hour,
            ctx.t,
            self.reset_time,
            CTRL_RESET,
            0,
            ctx.self_ref,
        )
    }
}
