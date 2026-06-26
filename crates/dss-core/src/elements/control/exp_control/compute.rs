//! The ExpControl control algorithm (PHASE7_PLAN WP7.5 step 3): the PVSystem
//! fleet build (`MakePVSystemList`), the `Sample` trigger logic, the
//! `DoPendingAction` adaptive-`Vreg` volt-var dispatch, and `UpdateExpControl`
//! (the per-step `Vreg` slew). Ported loop-for-loop from `Controls/ExpControl.pas`.
//!
//! Like [`InvControl`](super::super::inv_control), the fleet and the controlled
//! PVSystems are reached through the executive via the [`ExpDispatchEnv`]
//! abstraction (`solution/controls/dispatch.rs`), resolved against the class
//! registry; the fleet resolves lazily on the first `Sample`.

use crate::elements::pc::pvsystem::VARMODE_KVAR;
use crate::elements::traits::ElemRef;
use crate::solution::CTRLSTATIC;
use crate::util::fmt_g;

use super::{CHANGEVARLEVEL, ExpControl, ExpVars, NONE};

/// Pascal `Math.Sign` — returns -1.0 / 0.0 / 1.0.
fn pas_sign(x: f64) -> f64 {
    if x < 0.0 {
        -1.0
    } else if x > 0.0 {
        1.0
    } else {
        0.0
    }
}

/// The outcome of resolving one named PVSystem (Pascal `PVSysClass.Find` + the
/// `Enabled` check in `MakePVSystemList`). A missing or disabled member is
/// silently skipped (no error, unlike `InvControl`'s named-list 14403).
#[derive(Debug, Clone, Copy)]
pub(crate) enum PvFind {
    NotFound,
    Disabled,
    Found(ElemRef),
}

/// A read-only snapshot of one controlled PVSystem's *stable* state (the fields
/// that do not change across a single control iteration). `Presentkvar` /
/// `PresentkW` change after `SetNominalDEROutput`, so they are read live through
/// dedicated env methods instead.
#[derive(Debug, Clone)]
pub(crate) struct PvSnap {
    /// `PVSys.Name` (bare object name, for the event log).
    pub name: String,
    pub nphases: usize,
    pub inverter_on: bool,
    pub var_follow_inverter: bool,
    pub kva_rating: f64,
    pub kvar_limit: f64,
    /// `PVSys.Pmpp`.
    pub pmpp: f64,
    /// `ActiveCircuit.Buses[PVSys.terminals[0].busRef].kVBase` (kV, not volts).
    pub bus_kvbase: f64,
}

/// The executive surface `Sample`/`DoPendingAction`/`UpdateExpControl` need to
/// reach the controlled PVSystem fleet (the Rust stand-in for Pascal's live
/// `TPVSystemObj` pointers + `ActiveCircuit.Solution`/`ControlQueue`/
/// `EventStrings`). ExpControl controls **only PVSystem**, so the surface is
/// PVSystem-typed (no DER-type dispatch).
pub(crate) trait ExpDispatchEnv {
    // --- fleet resolution ---
    /// Pascal `PVSysClass.Find(name)` (distinguish missing / disabled / found).
    fn find_pvsystem(&self, name: &str) -> PvFind;
    /// Pascal's "scan the whole circuit for every PVSystem" (creation order);
    /// returns `(Name, ref, enabled)` — the name is added to `FPVSystemNameList`
    /// regardless of `enabled`, the ref to the fleet only when enabled.
    fn all_pvsystems(&self) -> Vec<(String, ElemRef, bool)>;

    // --- per-PVSystem read (stable + live) ---
    fn pv_snap(&self, r: ElemRef) -> PvSnap;
    /// Pascal `PVSys.ComputeVTerminal` then `Cabs(Vterminal[1..NPhases])` — the
    /// per-phase terminal-voltage magnitudes (the first `NPhases`).
    fn pv_vterminal_mags(&mut self, r: ElemRef) -> Vec<f64>;
    /// `PVSys.Get_Presentkvar` (achieved; live).
    fn pv_present_kvar(&self, r: ElemRef) -> f64;
    /// `PVSys.Get_PresentkW` (achieved; live, read after `SetNominalDEROutput`).
    fn pv_present_kw(&self, r: ElemRef) -> f64;

    // --- per-PVSystem write ---
    /// Pascal `PVSys.AVRmode := TRUE` (set on every fleet member in `MakePVSystemList`).
    fn pv_set_avr_mode(&mut self, r: ElemRef, value: bool);
    /// Pascal `PVSys.VWmode := value`.
    fn pv_set_vw_mode(&mut self, r: ElemRef, value: bool);
    /// Pascal `PVSys.Varmode := value` (VARMODEKVAR).
    fn pv_set_var_mode(&mut self, r: ElemRef, mode: i32);
    /// Pascal `PVSys.SetNominalDEROutput`.
    fn pv_set_nominal(&mut self, r: ElemRef);
    /// Pascal `PVSys.PresentkW := value` (writes `kWRequested`).
    fn pv_set_present_kw(&mut self, r: ElemRef, value: f64);
    /// Pascal `PVSys.puPmpp := value`.
    fn pv_set_pu_pmpp(&mut self, r: ElemRef, value: f64);
    /// Pascal `PVSys.Presentkvar := value` (`Set_Presentkvar`: writes
    /// `kvarRequested` + `Varmode := VARMODEKVAR`).
    fn pv_set_present_kvar(&mut self, r: ElemRef, value: f64);
    /// Pascal `PVSys.Set_Variable(5, value)` — the dynamic state variable `Vreg`.
    fn pv_set_vreg_var(&mut self, r: ElemRef, value: f64);

    // --- control queue / event log / scalars ---
    /// Pascal `ActiveCircuit.ControlQueue.Push(TimeDelay, CHANGEVARLEVEL, 0, Self)`.
    fn push_change(&mut self, delay: f64, code: i32);
    /// Pascal `AppendToEventLog(sender, msg)` — `sender` is the fully composed
    /// `Self.FullName + sep + PVSys.Name` (the separator differs: a space in
    /// `Sample`, a comma in `DoPendingAction` / `UpdateExpControl`).
    fn append_event(&mut self, sender: &str, msg: &str);
    /// `ActiveCircuit.Solution.ControlMode` (the `CTRLSTATIC` checks).
    fn control_mode(&self) -> i32;
    /// `ActiveCircuit.Solution.ControlIteration` (the `= 1` Sample trigger).
    fn control_iteration(&self) -> i32;
    /// `ActiveCircuit.Solution.DynaVars.h` (seconds).
    fn dyna_h(&self) -> f64;
    /// Pascal `ActiveCircuit.Solution.LoadsNeedUpdating := TRUE`.
    fn set_loads_need_updating(&mut self);
}

impl ExpControl {
    /// `Self.FullName` (`ExpControl.<name>`) for the event log.
    fn full_name(&self) -> String {
        format!("ExpControl.{}", self.ccd.cd.obj.name())
    }

    /// Pascal `Set_PendingChange(Value, DevIndex)` — `FPendingChange[DevIndex] :=
    /// Value; DblTraceParameter := Value`.
    fn set_pending_change(&mut self, i: usize, value: i32) {
        self.ctrl_vars[i].f_pending_change = value;
        self.ccd.dbl_trace_param = value as f64;
    }

    /// Pascal `TExpControlObj.MakePVSystemList(doRecalc=FALSE)` — resolve the
    /// PVSystem fleet (a named list keeps each found+enabled member, silently
    /// skipping missing/disabled; an empty list scans every PVSystem), set
    /// `AVRmode := TRUE` on each fleet member, and (re)initialize the per-DER
    /// `CtrlVars`.
    fn make_pvsystem_list(&mut self, env: &mut dyn ExpDispatchEnv) {
        self.fleet.clear();

        if self.f_list_size > 0 {
            // Named list — use it (clone to release the immutable borrow of self
            // before the &mut env calls).
            for name in self.pvsystem_name_list.clone() {
                if let PvFind::Found(r) = env.find_pvsystem(&name) {
                    self.fleet.push(r);
                    env.pv_set_avr_mode(r, true);
                }
            }
        } else {
            // Scan every PVSystem in the circuit.
            self.pvsystem_name_list.clear();
            for (name, r, enabled) in env.all_pvsystems() {
                if enabled {
                    self.fleet.push(r);
                    env.pv_set_avr_mode(r, true);
                }
                self.pvsystem_name_list.push(name);
            }
            self.f_list_size = self.fleet.len() as i32;
        }

        let vreg_init = self.f_vreg_init;
        self.ctrl_vars = (0..self.fleet.len())
            .map(|_| ExpVars::new(vreg_init))
            .collect();
    }

    /// Build the fleet lazily on the first `Sample` (Pascal's `RecalcElementData`
    /// runs `MakePVSystemList` whenever `FPVSystemPointerList.Count = 0`; `FOpenTau`
    /// + the terminal bus are already set at parse time).
    fn ensure_fleet(&mut self, env: &mut dyn ExpDispatchEnv) {
        if self.fleet.is_empty() {
            self.make_pvsystem_list(env);
        }
    }

    /// Pascal `TExpControlObj.Sample`: per controlled PVSystem, compute the present
    /// per-unit terminal voltage, find `Vreg` in static-init mode, and queue a
    /// `CHANGEVARLEVEL` action when the voltage / kvar moved past tolerance (or on
    /// the first control iteration).
    pub(crate) fn sample(&mut self, env: &mut dyn ExpDispatchEnv) {
        self.ensure_fleet(env);

        if self.f_list_size <= 0 {
            return;
        }

        for i in 0..self.fleet.len() {
            let r = self.fleet[i];
            let snap = env.pv_snap(r);

            // Present average voltage magnitude (per-unit on the L-N base).
            let mags = env.pv_vterminal_mags(r);
            let vpresent: f64 = mags.iter().take(snap.nphases).sum();
            self.ctrl_vars[i].f_present_vpu =
                (vpresent / snap.nphases as f64) / (snap.bus_kvbase * 1000.0);

            // If initializing with Vreg=0 in static mode, FIND Vreg.
            if env.control_mode() == CTRLSTATIC && self.f_vreg_init <= 0.0 {
                self.ctrl_vars[i].f_vregs = self.ctrl_vars[i].f_present_vpu;
                if self.ctrl_vars[i].f_vregs < self.vreg_min {
                    self.ctrl_vars[i].f_vregs = self.vreg_min;
                    self.f_vreg_init = 0.01; // don't let it outside the band
                }
                if self.ctrl_vars[i].f_vregs > self.vreg_max {
                    self.ctrl_vars[i].f_vregs = self.vreg_max;
                    self.f_vreg_init = 0.01; // don't let it outside the band
                }
            }

            // Both errors are in per-unit.
            let verr = (self.ctrl_vars[i].f_present_vpu - self.ctrl_vars[i].f_prior_vpu).abs();
            let qerr =
                (env.pv_present_kvar(r) - self.ctrl_vars[i].f_target_q).abs() / snap.kva_rating;

            // Not injecting: track Vreg toward the grid voltage but take no action.
            if !snap.inverter_on && snap.var_follow_inverter {
                if self.vreg_tau > 0.0 && self.ctrl_vars[i].f_vregs <= 0.0 {
                    self.ctrl_vars[i].f_vregs = self.ctrl_vars[i].f_present_vpu;
                }
                continue;
            }

            env.pv_set_vw_mode(r, false);
            if verr > self.f_voltage_change_tolerance
                || qerr > self.f_var_change_tolerance
                || env.control_iteration() == 1
            {
                self.ctrl_vars[i].f_within_tol = false;
                self.set_pending_change(i, CHANGEVARLEVEL);
                env.push_change(self.ccd.time_delay, CHANGEVARLEVEL);
                if self.ccd.show_event_log {
                    let sender = format!("{} {}", self.full_name(), snap.name);
                    env.append_event(
                        &sender,
                        &format!(
                            " outside Hit Tolerance, Verr= {}, Qerr={}",
                            fmt_g(verr, 5),
                            fmt_g(qerr, 5)
                        ),
                    );
                }
            } else {
                self.ctrl_vars[i].f_within_tol = true;
                if self.ccd.show_event_log {
                    let sender = format!("{} {}", self.full_name(), snap.name);
                    env.append_event(
                        &sender,
                        &format!(
                            " within Hit Tolerance, Verr= {}, Qerr={}",
                            fmt_g(verr, 5),
                            fmt_g(qerr, 5)
                        ),
                    );
                }
            }
        }
    }

    /// Pascal `TExpControlObj.DoPendingAction`: per pending PVSystem, set the kvar
    /// from the slope crossing at `Vreg` (+ `Qbias`), clamp to the inverter
    /// headroom / `QmaxLead` / `QmaxLag`, optionally curtail kW (`PreferQ`),
    /// low-pass filter the target (`FOpenTau`), and move it by `DeltaQ_Factor`.
    pub(crate) fn do_pending_action(&mut self, env: &mut dyn ExpDispatchEnv) {
        for i in 0..self.fleet.len() {
            if self.ctrl_vars[i].f_pending_change != CHANGEVARLEVEL {
                continue;
            }
            let r = self.fleet[i];
            let snap = env.pv_snap(r);

            env.pv_set_vw_mode(r, false);
            env.pv_set_var_mode(r, VARMODE_KVAR);
            self.ctrl_vars[i].f_target_q = 0.0;
            let qbase = snap.kva_rating;
            let qinvmaxpu = snap.kvar_limit / qbase;
            let mut qpu = env.pv_present_kvar(r) / qbase; // no change for now

            if !self.ctrl_vars[i].f_within_tol {
                // Look up Qpu from the slope crossing at Vreg, and add the bias.
                qpu = -self.q_v_slope
                    * (self.ctrl_vars[i].f_present_vpu - self.ctrl_vars[i].f_vregs)
                    + self.f_qbias;
                if self.ccd.show_event_log {
                    let sender = format!("{},{}", self.full_name(), snap.name);
                    env.append_event(
                        &sender,
                        &format!(
                            " Setting Qpu= {} at FVreg= {}, Vpu= {}",
                            fmt_g(qpu, 5),
                            fmt_g(self.ctrl_vars[i].f_vregs, 5),
                            fmt_g(self.ctrl_vars[i].f_present_vpu, 5)
                        ),
                    );
                }
            }

            // Apply limits on Qpu, then define the target in kVAR.
            env.pv_set_nominal(r); // as does InvControl
            let present_kw = env.pv_present_kw(r);
            let mut qmaxpu = if self.f_prefer_q {
                1.0
            } else {
                (1.0 - (present_kw / qbase).powi(2)).sqrt() // dynamic headroom
            };
            if qmaxpu > qinvmaxpu {
                qmaxpu = qinvmaxpu;
            }
            if qpu.abs() > qmaxpu {
                qpu = qmaxpu * pas_sign(qpu);
            }
            if qpu < -self.qmax_lead {
                qpu = -self.qmax_lead;
            }
            if qpu > self.qmax_lag {
                qpu = self.qmax_lag;
            }
            self.ctrl_vars[i].f_target_q = qbase * qpu;

            if self.f_prefer_q {
                let plimit = qbase * (1.0 - qpu * qpu).sqrt();
                if plimit < present_kw {
                    if self.ccd.show_event_log {
                        let sender = format!("{},{}", self.full_name(), snap.name);
                        env.append_event(
                            &sender,
                            &format!(" curtailing {present_kw:.3} to {plimit:.3} kW"),
                        );
                    }
                    env.pv_set_present_kw(r, plimit);
                    env.pv_set_pu_pmpp(r, plimit / snap.pmpp);
                }
            }

            // Put FTargetQ through the low-pass open-loop filter.
            if self.f_open_tau > 0.0 && env.control_mode() != CTRLSTATIC {
                let dt = env.dyna_h();
                self.ctrl_vars[i].f_target_q = self.ctrl_vars[i].f_last_step_q
                    + (self.ctrl_vars[i].f_target_q - self.ctrl_vars[i].f_last_step_q)
                        * (1.0 - (-dt / self.f_open_tau).exp());
            }

            // Only move the non-bias component by deltaQ_factor in this iteration.
            let delta_q = self.ctrl_vars[i].f_target_q - self.ctrl_vars[i].f_last_iter_q;
            let qset = self.ctrl_vars[i].f_last_iter_q + delta_q * self.f_delta_q_factor;
            if env.pv_present_kvar(r) != qset {
                env.pv_set_present_kvar(r, qset);
            }
            if self.ccd.show_event_log {
                let sender = format!("{},{}", self.full_name(), snap.name);
                env.append_event(
                    &sender,
                    &format!(
                        " Setting PVSystem output kvar= {}",
                        fmt_g(env.pv_present_kvar(r), 5)
                    ),
                );
            }
            self.ctrl_vars[i].f_last_iter_q = qset;
            self.ctrl_vars[i].f_prior_vpu = self.ctrl_vars[i].f_present_vpu;
            env.set_loads_need_updating(); // force recalc of power parms
            self.set_pending_change(i, NONE);
        }
    }

    /// Pascal `TExpControlObj.UpdateExpControl(i)` (driven by `UpdateAll` at the end
    /// of the power-flow loop): per PVSystem, snapshot the last-step kvar, slew
    /// `Vreg` toward the present voltage by `VregTau`, clamp it to
    /// `[VregMin, VregMax]`, and write it back as the PVSystem's `Vreg` state var.
    pub(crate) fn update_exp_control(&mut self, env: &mut dyn ExpDispatchEnv) {
        for j in 0..self.fleet.len() {
            let r = self.fleet[j];
            let snap = env.pv_snap(r);
            self.ctrl_vars[j].f_last_step_q = env.pv_present_kvar(r);
            let verr = if self.vreg_tau > 0.0 {
                let dt = env.dyna_h();
                let verr = self.ctrl_vars[j].f_present_vpu - self.ctrl_vars[j].f_vregs;
                self.ctrl_vars[j].f_vregs += verr * (1.0 - (-dt / self.vreg_tau).exp());
                verr
            } else {
                0.0
            };
            if self.ctrl_vars[j].f_vregs < self.vreg_min {
                self.ctrl_vars[j].f_vregs = self.vreg_min;
            }
            if self.ctrl_vars[j].f_vregs > self.vreg_max {
                self.ctrl_vars[j].f_vregs = self.vreg_max;
            }
            env.pv_set_vreg_var(r, self.ctrl_vars[j].f_vregs);
            if self.ccd.show_event_log {
                let sender = format!("{},{}", self.full_name(), snap.name);
                env.append_event(
                    &sender,
                    &format!(
                        " Setting new Vreg= {} Vpu={} Verr={}",
                        fmt_g(self.ctrl_vars[j].f_vregs, 5),
                        fmt_g(self.ctrl_vars[j].f_present_vpu, 5),
                        fmt_g(verr, 5)
                    ),
                );
            }
        }
    }

    /// Pascal `TExpControlObj.Reset` — a no-op (the body is commented out upstream).
    pub(crate) fn reset(&mut self) {}
}
