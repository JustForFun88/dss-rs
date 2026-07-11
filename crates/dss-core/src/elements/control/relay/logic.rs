//! The per-sub-type sensing logic for `TRelayObj` — one function per
//! `ControlType` that `Sample` dispatches to. Each reads the monitored
//! element's solved state (currents / terminal voltages / terminal power),
//! evaluates its protection characteristic, and arms/queues `OPEN`/`CLOSE`/
//! `RESET` actions exactly like the Pascal `*Logic` procedures.
//!
//! Ported here: `OvercurrentLogic`, `VoltageLogic`, `RevPowerLogic`,
//! `NegSeq46Logic`, `NegSeq47Logic`, `GenericLogic`, `DistanceLogic`,
//! `TD21Logic`, `DirectionalOvercurrentLogic` + `GetControlPower`. `GenericLogic`
//! reads a monitored PC element's state variable; `TD21Logic` runs the per-cycle
//! differential-distance ring buffer off the dynamics clock
//! (`DynaVars.h`/`Frequency`/`IterationFlag`).

use num_complex::Complex64;

use crate::elements::control::control_elem::{CTRL_CLOSE, CTRL_OPEN, CTRL_RESET, CtrlCtx};
use crate::elements::traits::CktElement;
use crate::support::complexutil::{cdang, pdeg_to_complex};
use crate::support::dynamics::IterationFlag;
use crate::support::mathutil::SymComp;

use super::Relay;

impl Relay {
    /// 0-based `(cond_offset, nphases)` for the monitored terminal.
    fn mon_offset(&self, mon: &dyn CktElement) -> (usize, usize) {
        let nconds = mon.cd().nconds;
        let cond_offset = (self.monitored_element_terminal.max(1) as usize - 1) * nconds;
        (cond_offset, mon.cd().nphases)
    }

    /// Pascal `TRelayObj.OvercurrentLogic` — phase + ground TCC (50/51), with a
    /// definite-time path (`Delay_Time`) and a first-operation instantaneous trip.
    pub(super) fn overcurrent_logic(&mut self, mon: &mut dyn CktElement, ctx: &mut CtrlCtx) {
        if self.present_state != CTRL_CLOSE {
            return;
        }
        let (cond_offset, nphases) = self.mon_offset(mon);
        let mut cbuffer = vec![Complex64::ZERO; mon.cd().yorder.max(1)];
        mon.get_currents(ctx.sys, ctx.node_v, &mut cbuffer);
        let cur = |p: usize| {
            cbuffer
                .get(cond_offset + p)
                .copied()
                .unwrap_or(Complex64::ZERO)
        };

        // Hoist scalars so the curve `&mut` borrows below don't conflict.
        let (ground_inst, ground_trip, td_ground) =
            (self.ground_inst, self.ground_trip, self.td_ground);
        let (phase_inst, phase_trip, td_phase) = (self.phase_inst, self.phase_trip, self.td_phase);
        let (delay_time, breaker_time, operation_count) =
            (self.delay_time, self.breaker_time, self.operation_count);

        let mut trip_time = -1.0_f64;
        let mut ground_time = -1.0_f64;

        // Ground trip: magnitude of the sum of the monitored phase currents.
        if (self.ground_curve.is_some() || delay_time > 0.0) && ground_trip > 0.0 {
            let mut csum = Complex64::ZERO;
            for p in 0..nphases {
                csum += cur(p);
            }
            let cmag = csum.norm();
            ground_time = if ground_inst > 0.0 && cmag >= ground_inst && operation_count == 1 {
                0.01 + breaker_time // inst trip on the first operation
            } else if delay_time > 0.0 {
                if cmag >= ground_trip {
                    delay_time
                } else {
                    -1.0
                }
            } else if let Some(gc) = self.ground_curve.as_mut() {
                td_ground * gc.get_tcc_time(cmag / ground_trip)
            } else {
                -1.0
            };
        }
        if ground_time > 0.0 {
            trip_time = ground_time;
            self.ground_target = true;
        }

        // Phase trip: the smallest per-phase TCC time (or an inst trip).
        let mut phase_time = -1.0_f64;
        if (self.phase_curve.is_some() || delay_time > 0.0) && phase_trip > 0.0 {
            for p in 0..nphases {
                let cmag = cur(p).norm();
                if phase_inst > 0.0 && cmag >= phase_inst && operation_count == 1 {
                    phase_time = 0.01 + breaker_time;
                    break; // no sense checking the other phases
                }
                let time_test = if delay_time > 0.0 {
                    if cmag >= phase_trip { delay_time } else { -1.0 }
                } else if let Some(pc) = self.phase_curve.as_mut() {
                    td_phase * pc.get_tcc_time(cmag / phase_trip)
                } else {
                    -1.0
                };
                if time_test > 0.0 {
                    phase_time = if phase_time < 0.0 {
                        time_test
                    } else {
                        phase_time.min(time_test)
                    };
                }
            }
        }
        if phase_time > 0.0 {
            self.phase_target = true;
            trip_time = if trip_time > 0.0 {
                trip_time.min(phase_time)
            } else {
                phase_time
            };
        }

        if trip_time > 0.0 {
            if !self.armed_for_open {
                self.relay_target = String::new();
                if phase_time > 0.0 {
                    self.relay_target.push_str("Ph");
                }
                if ground_time > 0.0 {
                    self.relay_target.push_str(" Gnd");
                }
                self.queue_trip_and_reclose(trip_time + breaker_time, ctx);
                self.armed_for_open = true;
                self.armed_for_close = true;
            }
        } else if self.armed_for_open {
            // Current dropped below pickup before tripping: disarm and reset.
            self.push_reset(ctx);
            self.armed_for_open = false;
            self.armed_for_close = false;
            self.phase_target = false;
            self.ground_target = false;
        }
    }

    /// Pascal `TRelayObj.VoltageLogic` — definite-time over/under voltage (27/59)
    /// with a voltage-recovery reclose. Uses the over/under-voltage curve scans.
    pub(super) fn voltage_logic(&mut self, mon: &mut dyn CktElement, ctx: &mut CtrlCtx) {
        if self.locked_out {
            return;
        }
        let nphases = mon.cd().nphases;
        let mut vbuffer = vec![Complex64::ZERO; mon.cd().nconds.max(1)];
        mon.get_term_voltages(
            self.monitored_element_terminal.max(1) as usize,
            ctx.node_v,
            &mut vbuffer,
        );

        let mut vmin = 1.0e50_f64;
        let mut vmax = 0.0_f64;
        for p in 0..nphases {
            let vmag = vbuffer.get(p).copied().unwrap_or(Complex64::ZERO).norm();
            if vmag > vmax {
                vmax = vmag;
            }
            if vmag < vmin {
                vmin = vmag;
            }
        }
        // Convert to per-unit.
        vmax /= self.vbase;
        vmin /= self.vbase;

        if self.present_state == CTRL_CLOSE {
            let mut trip_time = -1.0_f64;
            let ov_time = self.ov_curve.as_ref().map_or(-1.0, |c| c.get_ov_time(vmax));
            if ov_time > 0.0 {
                trip_time = ov_time;
            }
            let uv_time = self.uv_curve.as_ref().map_or(-1.0, |c| c.get_uv_time(vmin));
            if uv_time > 0.0 {
                trip_time = if trip_time > 0.0 {
                    trip_time.min(uv_time)
                } else {
                    uv_time
                };
            }

            if trip_time > 0.0 {
                let candidate = ctx.t + trip_time + self.breaker_time;
                if self.armed_for_open && candidate < self.next_trip_time {
                    ctx.queue.delete(self.last_event_handle);
                    self.armed_for_open = false; // force it through the next IF
                }
                if !self.armed_for_open {
                    self.relay_target = if trip_time == uv_time {
                        if trip_time == ov_time {
                            "UV + OV"
                        } else {
                            "UV"
                        }
                    } else {
                        "OV"
                    }
                    .to_string();
                    self.next_trip_time = ctx.t + trip_time + self.breaker_time;
                    self.last_event_handle = ctx.queue.push(
                        ctx.int_hour,
                        self.next_trip_time,
                        CTRL_OPEN,
                        0,
                        ctx.self_ref,
                    );
                    self.armed_for_open = true;
                }
            } else if self.armed_for_open {
                // Voltage recovered before tripping: drop the trip, set for reset.
                ctx.queue.delete(self.last_event_handle);
                self.next_trip_time = -1.0;
                self.last_event_handle = self.push_reset(ctx);
                self.armed_for_open = false;
            }
        } else {
            // Present state OPEN: reclose a set time after voltage recovers.
            if self.operation_count <= self.num_reclose {
                if !self.armed_for_close {
                    if vmax > 0.9 {
                        let interval = self.reclose_interval();
                        self.last_event_handle = ctx.queue.push_delay(
                            ctx.int_hour,
                            ctx.t,
                            interval,
                            CTRL_CLOSE,
                            0,
                            ctx.self_ref,
                        );
                        self.armed_for_close = true;
                    }
                } else if vmax < 0.9 {
                    // Armed but voltage dropped before reclosing: cancel.
                    self.armed_for_close = false;
                }
            }
        }
    }

    /// Pascal `TRelayObj.RevPowerLogic` — one-shot reverse-power lockout (32).
    pub(super) fn rev_power_logic(&mut self, mon: &mut dyn CktElement, ctx: &mut CtrlCtx) {
        let s = mon.terminal_power(
            ctx.sys,
            ctx.node_v,
            self.monitored_element_terminal.max(1) as usize,
        );
        if s.re < 0.0 {
            if s.re.abs() > self.phase_inst * 1000.0 {
                if !self.armed_for_open {
                    self.relay_target = "Rev P".to_string();
                    ctx.queue.push_delay(
                        ctx.int_hour,
                        ctx.t,
                        self.delay_time + self.breaker_time,
                        CTRL_OPEN,
                        0,
                        ctx.self_ref,
                    );
                    self.operation_count = self.num_reclose + 1; // force a lockout
                    self.armed_for_open = true;
                }
            } else if self.armed_for_open {
                self.push_reset(ctx);
                self.armed_for_open = false;
            }
        }
    }

    /// Pascal `TRelayObj.NegSeq46Logic` — negative-sequence current (Basler-style),
    /// one-shot to lockout.
    pub(super) fn neg_seq46_logic(&mut self, mon: &mut dyn CktElement, ctx: &mut CtrlCtx) {
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
        let neg_seq_mag = i012[2].norm(); // I012[3] (1-based) = neg sequence

        if neg_seq_mag >= self.pickup_amps46 {
            if !self.armed_for_open {
                self.relay_target = "-Seq Curr".to_string();
                let trip_time = if self.delay_time > 0.0 {
                    self.delay_time
                } else {
                    // Isqt46 / (NegSeqMag / BaseAmps46)^2 (constant-current estimate).
                    let ratio = neg_seq_mag / self.base_amps46;
                    self.isqt46 / (ratio * ratio)
                };
                ctx.queue.push_delay(
                    ctx.int_hour,
                    ctx.t,
                    trip_time + self.breaker_time,
                    CTRL_OPEN,
                    0,
                    ctx.self_ref,
                );
                self.operation_count = self.num_reclose + 1; // force a lockout
                self.armed_for_open = true;
            }
        } else if self.armed_for_open {
            self.push_reset(ctx);
            self.armed_for_open = false;
        }
    }

    /// Pascal `TRelayObj.NegSeq47Logic` — negative-sequence voltage, one-shot to
    /// lockout.
    pub(super) fn neg_seq47_logic(&mut self, mon: &mut dyn CktElement, ctx: &mut CtrlCtx) {
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
            if !self.armed_for_open {
                self.relay_target = "-Seq V".to_string();
                ctx.queue.push_delay(
                    ctx.int_hour,
                    ctx.t,
                    self.delay_time + self.breaker_time,
                    CTRL_OPEN,
                    0,
                    ctx.self_ref,
                );
                self.operation_count = self.num_reclose + 1; // force a lockout
                self.armed_for_open = true;
            }
        } else if self.armed_for_open {
            self.push_reset(ctx);
            self.armed_for_open = false;
        }
    }

    /// Pascal `TRelayObj.GenericLogic` — a `Generic` relay trips (one-shot to
    /// lockout) when a monitored PC element's state variable leaves the
    /// `[UnderTrip, OverTrip]` band. `MonitorVarIndex` was resolved in `recalc`
    /// (`LookupVariable`); its value is read fresh from the live element
    /// (`Variable[MonitorVarIndex]`).
    pub(super) fn generic_logic(&mut self, mon: &mut dyn CktElement, ctx: &mut CtrlCtx) {
        // VarValue := TPCElement(MonitoredElement).Variable[MonitorVarIndex].
        // Pascal `GetAllVariables[i-1] = Get_Variable(i)` (generator.pas l.2639),
        // so read the whole state vector and index it — a control element carries
        // no `Variable[]` of its own, and the Rust trait exposes the vector form.
        let idx = self.monitor_var_index;
        let var_value = if idx >= 1 {
            let n = mon.num_variables();
            let mut states = vec![0.0_f64; n];
            mon.get_all_variables(ctx.sys, ctx.node_v, &mut states);
            states.get((idx - 1) as usize).copied().unwrap_or(-9999.99)
        } else {
            -9999.99 // Pascal base `Get_Variable` error return
        };

        if var_value > self.over_trip || var_value < self.under_trip {
            if !self.armed_for_open {
                self.relay_target = if idx >= 1 {
                    mon.variable_name(idx as usize)
                } else {
                    String::new()
                };
                ctx.queue.push_delay(
                    ctx.int_hour,
                    ctx.t,
                    self.delay_time + self.breaker_time,
                    CTRL_OPEN,
                    0,
                    ctx.self_ref,
                );
                self.operation_count = self.num_reclose + 1; // force a lockout
                self.armed_for_open = true;
            }
        } else if self.armed_for_open {
            // Back within bounds: reset and disarm.
            self.push_reset(ctx);
            self.armed_for_open = false;
        }
    }

    /// Pascal `TRelayObj.DistanceLogic` — mho-style loop-impedance reach (21).
    /// Rectangular characteristic on each phase/phase-phase loop; the closest
    /// in-reach loop sets the target and a definite-time trip.
    pub(super) fn distance_logic(&mut self, mon: &mut dyn CktElement, ctx: &mut CtrlCtx) {
        if self.locked_out {
            return;
        }
        let (cond_offset, nphases) = self.mon_offset(mon);
        let mut cbuffer = vec![Complex64::ZERO; mon.cd().yorder.max(1)];
        mon.get_currents(ctx.sys, ctx.node_v, &mut cbuffer);
        // Dist_Reverse negates the monitored phase currents.
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
                    (
                        cv(i),
                        cur(i) + k_ires,
                        self.dist_z1 * self.mground, // Z0 folded into K0
                    )
                } else {
                    (cv(i) - cv(j), cur(i) - cur(j), self.dist_z1 * self.mphase)
                };
                let i2 = iloop.re * iloop.re + iloop.im * iloop.im;
                if i2 > 0.1 {
                    let zloop = vloop / iloop;
                    // Simple rectangular characteristic.
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
            if self.armed_for_reset {
                ctx.queue.delete(self.last_event_handle);
                self.armed_for_reset = false;
            }
            if !self.armed_for_open {
                targets.sort();
                self.relay_target = format!("21 {min_distance:.3} pu dist");
                for tgt in &targets {
                    self.relay_target.push(' ');
                    self.relay_target.push_str(tgt);
                }
                self.last_event_handle = ctx.queue.push_delay(
                    ctx.int_hour,
                    ctx.t,
                    self.delay_time + self.breaker_time,
                    CTRL_OPEN,
                    0,
                    ctx.self_ref,
                );
                self.armed_for_open = true;
                if self.operation_count <= self.num_reclose {
                    let interval = self.reclose_interval();
                    self.last_event_handle = ctx.queue.push_delay(
                        ctx.int_hour,
                        ctx.t,
                        self.delay_time + self.breaker_time + interval,
                        CTRL_CLOSE,
                        0,
                        ctx.self_ref,
                    );
                    self.armed_for_close = true;
                }
            }
        } else {
            // Not picked up: reset if necessary.
            if self.operation_count > 1 && !self.armed_for_reset {
                self.armed_for_reset = true;
                self.last_event_handle = self.push_reset(ctx);
            }
            if self.armed_for_open {
                self.armed_for_open = false;
                self.armed_for_close = false;
            }
        }
    }

    /// Pascal `TRelayObj.TD21Logic` — the differential (incremental) time-distance
    /// relay (21). On the first dynamics step a per-cycle ring buffer of terminal
    /// V/I is sized from `DynaVars.h`/`Frequency` (`round(1/60/dt + 0.5)` samples);
    /// each `Sample` forms the pre-fault-referenced increments `dV`/`dI` (present
    /// minus one-cycle-old), and — once a full cycle has elapsed since start or the
    /// last operation (`td21_quiet <= 0`) — checks every phase / phase-phase loop
    /// against a half-reach directional characteristic, arming a definite-time trip
    /// on the closest in-reach loop. Runs only under dynamics (`DynaVars.h > 0`).
    ///
    /// Returns `true` when the coarse-time-step guard (error 388) fires: Pascal
    /// `Relay.pas:1460` reports it via `DoErrorMsg`, which sets
    /// `DSS.SolutionAbort := True` (`DSSGlobals.pas:265`) — the dispatch layer
    /// lifts this into `Solution.SolutionAbort` so the run halts exactly where
    /// the oracle does (the `Sample`-time analogue of CapControl's abort return).
    pub(super) fn td21_logic(&mut self, mon: &mut dyn CktElement, ctx: &mut CtrlCtx) -> bool {
        let (cond_offset, nphases) = self.mon_offset(mon);
        let dt = ctx.sys.dyna_h;
        // Pascal `DoErrorMsg(...,388)` sets `SolutionAbort := True` but does NOT
        // `Exit` — the logic falls through. We mirror: record the request, keep
        // running, and return it so the dispatch layer aborts the solution.
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
            // Pascal `round(1/60/dt + 0.5)` (FPC banker's) — samples per ~one
            // 60 Hz cycle. The `1/60` is a hard-coded literal upstream (not
            // `1/Frequency`), reproduced verbatim.
            let i = (1.0 / 60.0 / dt + 0.5).round_ties_even() as i32;
            if i > self.td21_pt {
                self.td21_i = 0; // ring index, incremented before first use
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

        if self.locked_out {
            return solution_abort;
        }

        // Fault detection: any monitored phase current above `PhaseTrip`.
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

        // Defensive: the buffer is unallocated only when `dt <= 0` (Pascal would
        // then `mod 0`-crash in the ring math below — a crash we do not reproduce;
        // no corpus deck sets `stepsize <= 0`). `dt` defaults to 0.001, so this is
        // never hit in practice.
        if self.td21_pt < 1 {
            return solution_abort;
        }
        let stride = self.td21_stride as usize;

        // Prime the whole history with the present sample on the first pass.
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

        // Oldest sample = next write slot (1-based ring counter, Pascal semantics).
        self.td21_next = (self.td21_i % self.td21_pt) + 1;

        // Differential V/I vs the one-cycle-old reference sample.
        let ib = (self.td21_next - 1) as usize * stride;
        for p in 0..nphases {
            self.td21_uref[p] = self.td21_h[ib + p];
            self.td21_dv[p] = cv(p) - self.td21_h[ib + p];
            self.td21_di[p] = cur(p) - self.td21_h[ib + nphases + p];
        }

        // Advance the ring + decrement quiet only on a predictor (new-time-step)
        // iteration (`IterationFlag < 1`), not the corrector.
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
            return solution_abort; // within the post-start / post-operation quiet window
        }

        // One cycle elapsed: run the differential distance sense.
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
                        self.dist_z1 * self.mground, // Z0 folded into K0
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
                            // this loop trips
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
            if self.armed_for_reset {
                ctx.queue.delete(self.last_event_handle);
                self.armed_for_reset = false;
            }
            if !self.armed_for_open {
                targets.sort();
                self.relay_target = format!("TD21 {min_distance:.3} pu dist");
                for tgt in &targets {
                    self.relay_target.push(' ');
                    self.relay_target.push_str(tgt);
                }
                self.last_event_handle = ctx.queue.push_delay(
                    ctx.int_hour,
                    ctx.t,
                    self.delay_time + self.breaker_time,
                    CTRL_OPEN,
                    0,
                    ctx.self_ref,
                );
                self.armed_for_open = true;
                if self.operation_count <= self.num_reclose {
                    let interval = self.reclose_interval();
                    self.last_event_handle = ctx.queue.push_delay(
                        ctx.int_hour,
                        ctx.t,
                        self.delay_time + self.breaker_time + interval,
                        CTRL_CLOSE,
                        0,
                        ctx.self_ref,
                    );
                    self.armed_for_close = true;
                }
            }
        }

        if !fault_detected {
            // Not picked up: reset if past the first operation, drop out if armed.
            if self.operation_count > 1 && !self.armed_for_reset {
                self.armed_for_reset = true;
                self.last_event_handle = self.push_reset(ctx);
            }
            if self.armed_for_open {
                self.td21_quiet = self.td21_pt + 1;
                self.armed_for_open = false;
                self.armed_for_close = false;
            }
        }

        solution_abort
    }

    /// Pascal `TRelayObj.GetControlPower` — the net positive-sequence active
    /// power used by the DOC blocking test (total terminal power for < 3 phases).
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
        // Positive-sequence power (V012[2]·conj(I012[2])), ×3 / 1000 (kilo).
        (v012[1] * i012[1].conj()) * 0.003
    }

    /// Pascal `TRelayObj.DirectionalOvercurrentLogic` (DOC). Blocks on forward
    /// net-balanced active power (`DOC_P1Blocking`), then evaluates a
    /// per-phase directional/circle/inner-zone characteristic on the
    /// voltage-referenced current phasors.
    pub(super) fn directional_overcurrent_logic(
        &mut self,
        mon: &mut dyn CktElement,
        ctx: &mut CtrlCtx,
    ) {
        if self.present_state != CTRL_CLOSE {
            return;
        }
        // Identify net balanced power flow.
        if self.doc_p1_blocking {
            let control_power = self.get_control_power(mon, ctx);
            if control_power.re >= 0.0 {
                // Forward power: disarm any pending trip and block.
                if self.armed_for_open {
                    self.push_reset(ctx);
                    self.armed_for_open = false;
                    self.armed_for_close = false;
                }
                return;
            }
        }

        let (cond_offset, nphases) = self.mon_offset(mon);
        let mut cbuffer = vec![Complex64::ZERO; mon.cd().yorder.max(1)];
        mon.get_currents(ctx.sys, ctx.node_v, &mut cbuffer);
        let mut cvbuffer = vec![Complex64::ZERO; mon.cd().nconds.max(1)];
        mon.get_term_voltages(
            self.monitored_element_terminal.max(1) as usize,
            ctx.node_v,
            &mut cvbuffer,
        );

        // Shift each current phasor to be relative to its phase voltage.
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
            if !self.armed_for_open {
                self.relay_target = "DOC".to_string();
                self.queue_trip_and_reclose(trip_time + self.breaker_time, ctx);
                self.armed_for_open = true;
                self.armed_for_close = true;
            }
        } else if self.armed_for_open {
            self.push_reset(ctx);
            self.armed_for_open = false;
            self.armed_for_close = false;
        }
    }

    // --- DOC characteristic decision tree (Pascal nested if/else, factored) ---

    /// Top-level DOC tilt-low decision: pick the inner circle/high-line zone, or
    /// the `else` straight-line-high-precedence branch. `pub(super)` so the
    /// `tests` module can drive the characteristic directly.
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
        } else {
            // Straight-line-high takes precedence; only the left side trips.
            if self.doc_trip_set_high > 0.0 {
                let left = if self.doc_tilt_angle_high == 90.0 || self.doc_tilt_angle_high == 270.0
                {
                    cb.re < -self.doc_trip_set_high
                } else {
                    cb.im
                        < self.doc_tilt_angle_high.to_radians().tan()
                            * (cb.re + self.doc_trip_set_high)
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
    }

    /// The circle (`DOC_TripSettingMag`) decision: within → high-line/inner zone,
    /// out → simple outer time; no circle → high-line-or-outer.
    fn doc_circle_decision(&mut self, cb: Complex64, cmag: f64) -> f64 {
        if self.doc_trip_set_mag > 0.0 {
            if cmag <= self.doc_trip_set_mag {
                // Within the circle.
                if self.doc_trip_set_high > 0.0 {
                    self.doc_highline_specified(cb, cmag)
                } else {
                    self.doc_inner_region_time(cmag)
                }
            } else {
                // Outside the circle.
                self.doc_outer_time(cmag)
            }
        } else {
            // Circle not specified.
            if self.doc_trip_set_high > 0.0 {
                self.doc_highline_specified(cb, cmag)
            } else {
                self.doc_outer_time(cmag)
            }
        }
    }

    /// High straight-line specified: left side → outer time, right side → inner
    /// region time.
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

    /// Outer-zone time: `Delay_Time` (if > 0), else the main `PhaseCurve` TCC,
    /// else `Delay_Time` (= 0).
    fn doc_outer_time(&mut self, cmag: f64) -> f64 {
        if self.delay_time > 0.0 {
            return self.delay_time;
        }
        let (td, trip) = (self.td_phase, self.phase_trip);
        if let Some(pc) = self.phase_curve.as_mut() {
            td * pc.get_tcc_time(cmag / trip)
        } else {
            self.delay_time // Delay_Time == 0.0
        }
    }

    /// Inner-zone time: `DOC_DelayInner` (if > 0), else the `DOC_PhaseCurveInner`
    /// TCC, else `Delay_Time` (when `DOC_DelayInner == 0`); `-1` when neither is
    /// set (the default `DOC_DelayInner = -1`).
    fn doc_inner_region_time(&mut self, cmag: f64) -> f64 {
        if self.doc_delay_inner > 0.0 {
            return self.doc_delay_inner;
        }
        let (td, trip, delay) = (
            self.doc_td_phase_inner,
            self.doc_phase_trip_inner,
            self.delay_time,
        );
        if let Some(c) = self.doc_phase_curve_inner.as_mut() {
            td * c.get_tcc_time(cmag / trip)
        } else if self.doc_delay_inner == 0.0 {
            delay
        } else {
            -1.0
        }
    }

    // --- shared queue helpers (Pascal `Push` patterns) ---

    /// `RecloseIntervals[OperationCount]` (Pascal 1-based) with the safe clamp the
    /// Recloser uses (the dead 4th slot is `0.0`).
    fn reclose_interval(&self) -> f64 {
        self.reclose_intervals
            .get((self.operation_count - 1).max(0) as usize)
            .copied()
            .unwrap_or(0.0)
    }

    /// Push the `OPEN` trip and, if shots remain, the follow-on `CLOSE` reclose
    /// (the overcurrent / DOC pattern). `at` is `TripTime + Breaker_time`.
    fn queue_trip_and_reclose(&mut self, at: f64, ctx: &mut CtrlCtx) {
        self.last_event_handle =
            ctx.queue
                .push_delay(ctx.int_hour, ctx.t, at, CTRL_OPEN, 0, ctx.self_ref);
        if self.operation_count <= self.num_reclose {
            let interval = self.reclose_interval();
            self.last_event_handle = ctx.queue.push_delay(
                ctx.int_hour,
                ctx.t,
                at + interval,
                CTRL_CLOSE,
                0,
                ctx.self_ref,
            );
        }
    }

    /// Push a `RESET` action `ResetTime` seconds out; returns the handle.
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
