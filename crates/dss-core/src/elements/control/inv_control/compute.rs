//! The InvControl control algorithm (WP7.5 step 2b — the VOLTVAR mode): the DER
//! fleet build (`MakeDERList`), `RecalcElementData`'s deferred tail,
//! `UpdateDERParameters` / `GetMonVoltage`, the `Sample` trigger logic, the
//! `DoPendingAction` volt-var dispatch, `UpdateInvControl` (the rolling-average
//! feed), and the volt-var math (`CalcQVVcurve_desiredpu`, `Check_Qlimits`,
//! `Calc_QHeadRoom`, `CalcVoltVar_vars`, `Change_deltaQ_factor`).
//!
//! Like [`StorageController`], the fleet and the controlled DERs are reached
//! through the executive via the [`InvDispatchEnv`] abstraction
//! (`solution/controls/dispatch.rs`), resolved against the class registry; the
//! fleet resolves lazily on the first `Sample`. The control's terminal bus
//! (Pascal `Setbus(1, MonitoredElement.Firstbus)`) is set at parse-time
//! edit-completion instead (see [`InvControl::recalc`]).
//!
//! **NOT_PORTED / deferred to sub-steps 2c–2e:** every non-VOLTVAR control mode
//! (VOLTWATT / DRC / WATTPF / WATTVAR / AVR + the VV_VW / VV_DRC combi modes), the
//! `MonBus=` explicit monitored-bus voltage path (`FUsingMonBuses`), the LPF /
//! Rise-Fall rate-of-change limiting, and the Exponential `ControlModel` (the
//! `TPICtrl` PI controller). Each records an explicit error rather than silently
//! degrading.
//!
//! [`StorageController`]: crate::elements::control::storage_controller
//! [`InvDispatchEnv`]: InvDispatchEnv

use crate::elements::traits::ElemRef;
use crate::util::fmt_g;

use super::{
    CHANGEVARLEVEL, FLAGDELTAQ, InvControl, MAXPHASE, MINPHASE, MODEL_LINEAR, NONE_COMBMODE,
    NONE_MODE, REAC_POWER_VARMAX, ROC_INACTIVE, VOLTVAR, WATTPF,
};

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

/// The outcome of resolving one named fleet member (Pascal `Class.Find` + the
/// `Enabled` check in `MakeDERList`).
#[derive(Debug, Clone, Copy)]
pub(crate) enum FleetFind {
    /// No DER by that name (Pascal `DoSimpleMsg(14403)` + `Exit`).
    NotFound,
    /// Found but disabled (Pascal silently skips it — not added to the fleet).
    Disabled,
    /// Found and enabled.
    Found(ElemRef),
}

/// A read-only snapshot of one controlled DER's state — the inputs
/// `UpdateDERParameters` copies into `CtrlVars` each `Sample` (Pascal reads these
/// live off the `TPVSystemObj`/`TStorageObj` pointer).
#[derive(Debug, Clone, Copy)]
pub(crate) struct DerSnap {
    pub is_pvsystem: bool,
    pub nphases: usize,
    pub nterms: usize,
    pub nconds: usize,
    // --- UpdateDERParameters fields ---
    pub vbase: f64,
    pub var_follow_inverter: bool,
    pub inverter_on: bool,
    pub present_kw: f64,
    pub kva_rating: f64,
    pub present_kvar: f64,
    pub kvar_limit: f64,
    pub kvar_limit_neg: f64,
    pub current_kvar_limit: f64,
    pub current_kvar_limit_neg: f64,
    pub p_priority: bool,
}

/// The executive surface `Sample`/`DoPendingAction`/`UpdateInvControl` need to
/// reach the controlled DER fleet (the Rust stand-in for Pascal's live object
/// pointers + `ActiveCircuit.Solution`/`ControlQueue`/`EventStrings`). Mirrors the
/// [`StorageDispatchEnv`](crate::elements::control::storage_controller) shape.
pub(crate) trait InvDispatchEnv {
    // --- fleet resolution ---
    /// Pascal `PVSysClass.Find(name)` (distinguish missing / disabled / found).
    fn find_pvsystem(&self, name: &str) -> FleetFind;
    /// Pascal `StorageClass.Find(name)`.
    fn find_storage(&self, name: &str) -> FleetFind;
    /// Pascal's "scan the whole circuit for every PVSystem" (creation order);
    /// returns `(FullName, ref, enabled)` — the name is added to `DERNameList`
    /// regardless of `enabled`, the ref to the fleet only when enabled.
    fn all_pvsystems(&self) -> Vec<(String, ElemRef, bool)>;
    /// Pascal's "scan the whole circuit for every Storage".
    fn all_storages(&self) -> Vec<(String, ElemRef, bool)>;
    /// Pascal `DoSimpleMsg` sink (the 14403 named-missing error).
    fn push_error(&mut self, msg: String);

    // --- per-DER read ---
    fn der_snap(&self, r: ElemRef) -> DerSnap;
    /// Pascal `DERElem.ComputeVTerminal` then `Cabs(Vterminal[1..NPhases])` — the
    /// per-phase terminal voltage magnitudes (used by `GetMonVoltage`).
    fn der_vterminal_mags(&mut self, r: ElemRef) -> Vec<f64>;
    /// `ActiveCircuit.Buses[DERElem.terminals[0].busRef].kVBase * 1000` — the L-N
    /// base volts for the `FVpuSolution` per-unit (UpdateInvControl).
    fn der_bus_vbase(&self, r: ElemRef) -> f64;
    /// `obj.FullName` (`PVSystem.<n>` / `Storage.<n>`) for the event log.
    fn der_full_name(&self, r: ElemRef) -> String;

    // --- per-DER write ---
    /// Pascal `DERElem.SetPFPriority(value)`.
    fn der_set_pf_priority(&mut self, r: ElemRef, value: bool);
    /// Set the DER's inverter-control mode flags (`VWmode`/`VVmode`) + `Varmode`
    /// (the `DoPendingAction` path).
    fn der_set_modes(&mut self, r: ElemRef, vw_mode: bool, vv_mode: bool, var_mode: i32);
    /// Set only `DERElem.VVmode` (the `Sample` path: Pascal sets just `VVmode`,
    /// leaving `VWmode`/`Varmode` until `DoPendingAction`).
    fn der_set_vv_mode(&mut self, r: ElemRef, value: bool);
    /// `TPVSystemObj.Presentkvar := q` / `TStorageObj.kvarRequested := q`.
    fn der_set_kvar_requested(&mut self, r: ElemRef, q: f64);
    /// `DERElem.SetNominalDEROutput()`.
    fn der_set_nominal(&mut self, r: ElemRef);
    /// `DERElem.Get_Presentkvar`.
    fn der_present_kvar(&self, r: ElemRef) -> f64;
    /// `PVSys/Storage.Set_Variable(idx, value)` — the mode-3 monitor state vars
    /// (`5`=FVreg, `7`/`16`=FVVOperation). Unobservable until the mode-3 monitor
    /// body lands (WP7.7); ported for fidelity.
    fn der_set_monitor_var(&mut self, r: ElemRef, kind: MonitorVar, value: f64);

    // --- control queue / event log / scalars ---
    /// Pascal `ControlQueue.Push(TimeDelay, CHANGEVARLEVEL, 0, Self)`.
    fn push_change(&mut self, delay: f64, code: i32);
    /// Pascal `AppendToEventLog(Self.FullName + ', ' + DERElem.FullName, msg)`.
    fn append_event(&mut self, der_full_name: &str, msg: &str);
    /// `ActiveCircuit.Solution.ControlIteration`.
    fn control_iteration(&self) -> i32;
    /// `ActiveCircuit.Solution.DynaVars.h` (seconds).
    fn dyna_h(&self) -> f64;
    /// `ActiveCircuit.Solution.DynaVars.dblHour`.
    fn dbl_hour(&self) -> f64;
}

/// Which mode-3 monitor state variable a `der_set_monitor_var` write targets.
#[derive(Debug, Clone, Copy)]
pub(crate) enum MonitorVar {
    /// Pascal `Set_Variable(5/14, FVreg)`.
    Vreg,
    /// Pascal `Set_Variable(7/16, FVVOperation)`.
    VvOperation,
}

impl InvControl {
    /// Pascal `TInvControlObj.MakeDERList`. A named `DERList` resolves each entry
    /// against the PVSystem/Storage class (keeping only enabled ones; a *missing*
    /// name errors 14403 and aborts the build); an empty list scans every PVSystem
    /// then every Storage (adding the enabled ones to the fleet and *every* name to
    /// `DERNameList`). Returns whether the fleet ended up non-empty.
    pub(super) fn make_der_list(&mut self, env: &mut dyn InvDispatchEnv) -> bool {
        self.fleet.clear();

        if self.f_list_size > 0 {
            // Named list — use it.
            for i in 0..self.der_name_list.len() {
                let entry = self.der_name_list[i].clone();
                let (class, bare) = split_class_name(&entry);
                match class.to_ascii_lowercase().as_str() {
                    "pvsystem" => match env.find_pvsystem(&bare) {
                        FleetFind::Found(r) => self.fleet.push(r),
                        FleetFind::Disabled => {}
                        FleetFind::NotFound => {
                            env.push_error(format!(
                                "Error: PVSystem Element \"{entry}\" not found."
                            ));
                            return false;
                        }
                    },
                    "storage" => match env.find_storage(&bare) {
                        FleetFind::Found(r) => self.fleet.push(r),
                        FleetFind::Disabled => {}
                        FleetFind::NotFound => {
                            env.push_error(format!(
                                "Error: Storage Element \"{entry}\" not found."
                            ));
                            return false;
                        }
                    },
                    // Pascal silently ignores a name with neither class prefix.
                    _ => {}
                }
            }
        } else {
            // Scan the whole circuit for PVSystem then Storage.
            self.der_name_list.clear();
            for (name, r, enabled) in env.all_pvsystems() {
                if enabled {
                    self.fleet.push(r);
                }
                self.der_name_list.push(name);
            }
            for (name, r, enabled) in env.all_storages() {
                if enabled {
                    self.fleet.push(r);
                }
                self.der_name_list.push(name);
            }
            self.f_list_size = self.fleet.len() as i32;
        }

        !self.fleet.is_empty()
    }

    /// The deferred tail of Pascal `RecalcElementData`: build the fleet, then run
    /// the per-DER setup (`SetPFPriority(FALSE)` off VOLTWATT/WATTPF,
    /// `UpdateDERParameters`, the rolling-window lengths). Pascal re-runs the build
    /// whenever `FDERPointerList.Count = 0`, so gate on an **empty** fleet: a
    /// successful (non-empty) build runs the setup once; a *partial* named list
    /// that hit a missing member keeps its valid prefix (Count > 0) and is **not**
    /// rebuilt (errors once); an all-missing / no-DER list stays empty and re-runs
    /// each `Sample` (the named-missing 14403 every Sample, matching Pascal). A
    /// DERList edit clears the fleet (`invalidate_fleet`), forcing a rebuild.
    fn ensure_fleet(&mut self, env: &mut dyn InvDispatchEnv) {
        if !self.fleet.is_empty() {
            return;
        }
        self.make_der_list(env);
        // One CtrlVars per built fleet member (Pascal `SetLength(CtrlVars, …)`) —
        // sized to the *actual* fleet, so a partial named list (a missing member
        // aborted the build after a valid prefix) still has its prefix allocated.
        self.ctrl_vars = (0..self.fleet.len())
            .map(|_| super::InvVars::new())
            .collect();

        self.f_using_mon_buses = !self.mon_buses_name_list.is_empty();

        for i in 0..self.fleet.len() {
            let snap = env.der_snap(self.fleet[i]);
            self.ctrl_vars[i].nphases_der = snap.nphases;
            self.ctrl_vars[i].nconds_der = snap.nconds;
            self.ctrl_vars[i]
                .f_roll_avg_window
                .set_length(self.roll_avg_window_length);
            self.ctrl_vars[i]
                .f_drc_roll_avg_window
                .set_length(self.drc_roll_avg_window_length);

            // For all modes other than VW and WATTPF, PF priority is not allowed.
            if self.control_mode != super::VOLTWATT && self.control_mode != WATTPF {
                env.der_set_pf_priority(self.fleet[i], false);
            }
            self.update_der_parameters(i, env);
        }
    }

    /// Pascal `TInvControlObj.UpdateDERParameters(i)` — refresh `CtrlVars[i]` from
    /// the controlled DER.
    fn update_der_parameters(&mut self, i: usize, env: &mut dyn InvDispatchEnv) {
        let snap = env.der_snap(self.fleet[i]);
        let cv = &mut self.ctrl_vars[i];
        cv.cond_offset = (snap.nterms - 1) * cv.nconds_der; // PVSystem path; storage = 0
        if !snap.is_pvsystem {
            cv.cond_offset = 0; // Pascal does not set CondOffset for Storage
        }
        cv.f_vbase = snap.vbase;
        cv.f_var_follow_inverter = snap.var_follow_inverter;
        cv.f_inverter_on = snap.inverter_on;
        cv.f_present_kw = snap.present_kw;
        cv.f_kva_rating = snap.kva_rating;
        cv.f_present_kvar = snap.present_kvar;
        cv.f_kvar_limit = snap.kvar_limit;
        cv.f_kvar_limit_neg = snap.kvar_limit_neg;
        cv.f_current_kvar_limit = snap.current_kvar_limit;
        cv.f_current_kvar_limit_neg = snap.current_kvar_limit_neg;
        cv.f_p_priority = snap.p_priority;
    }

    /// Pascal `TInvControlObj.GetMonVoltage(Vpresent, i, BasekV)` — the no-`MonBus`
    /// path (per-DER self-monitoring). The explicit-`MonBus` path is NOT_PORTED
    /// (step 2e); the caller guards on `FUsingMonBuses` first.
    fn get_mon_voltage(&self, i: usize, env: &mut dyn InvDispatchEnv) -> f64 {
        let mags = env.der_vterminal_mags(self.fleet[i]);
        let n = self.ctrl_vars[i].nphases_der.min(mags.len());
        match self.mon_buses_phase {
            super::AVGPHASES => {
                if n == 0 {
                    0.0
                } else {
                    mags[..n].iter().sum::<f64>() / n as f64
                }
            }
            MAXPHASE => mags[..n].iter().copied().fold(0.0, f64::max),
            MINPHASE => mags[..n].iter().copied().fold(1.0e50, f64::min),
            // A specific (1-based) phase.
            p => {
                let idx = (p - 1) as usize;
                mags.get(idx).copied().unwrap_or(0.0)
            }
        }
    }

    /// Pascal `TInvControlObj.Sample`. Step 2b: the VOLTVAR trigger only; every
    /// other mode records an explicit NOT_PORTED error (never a silent skip).
    pub(crate) fn sample(&mut self, env: &mut dyn InvDispatchEnv) -> Result<(), String> {
        self.ensure_fleet(env); // lazy build (Pascal `if FDERPointerList.Count = 0`)
        if self.f_list_size <= 0 {
            return Ok(());
        }

        // Mode / combi-mode gating: step 2b ports VOLTVAR only.
        if self.combi_mode != NONE_COMBMODE {
            return Err(self.not_ported_mode());
        }
        match self.control_mode {
            NONE_MODE => return Ok(()), // do nothing
            VOLTVAR => {}
            _ => return Err(self.not_ported_mode()),
        }
        if self.f_using_mon_buses {
            return Err(format!(
                "InvControl.{}: explicit MonBus voltage monitoring is not yet ported (WP7.5 step 2e)",
                self.ccd.cd.obj.name()
            ));
        }
        if self.rate_of_change_mode != ROC_INACTIVE {
            return Err(format!(
                "InvControl.{}: LPF/RiseFall rate-of-change limiting is not yet ported (WP7.5 step 2e)",
                self.ccd.cd.obj.name()
            ));
        }
        // Exponential ControlModel runs the `TPICtrl` PI controller in
        // `CalcVoltVar_vars` (WP7.7); reject it rather than silently freeze the
        // var output (the deferral-is-never-a-silent-skip convention).
        if self.ctrl_model != MODEL_LINEAR {
            return Err(format!(
                "InvControl.{}: Exponential ControlModel (the PICtrl PI controller) is not yet ported (WP7.7)",
                self.ccd.cd.obj.name()
            ));
        }

        let control_iter = env.control_iteration();

        for i in 0..self.fleet.len() {
            self.update_der_parameters(i, env);
            let r = self.fleet[i];
            let snap = env.der_snap(r);

            let basekv = self.ctrl_vars[i].f_vbase / 1000.0; // L-N voltage
            let vpresent = self.get_mon_voltage(i, env);

            // ControlIteration 1: seed the prior averages (for the event log).
            if control_iter == 1 {
                self.ctrl_vars[i].f_avgp_vpu_prior = self.ctrl_vars[i].f_present_vpu;
                self.ctrl_vars[i].f_avgp_drc_vpu_prior = self.ctrl_vars[i].f_present_drc_vpu;
            }

            // Convert to per-unit on the curve X reference.
            let avg_val = self.ctrl_vars[i].f_roll_avg_window.avg_val();
            let present_vpu = if self.voltage_curvex_ref == 1 && avg_val != 0.0 {
                vpresent / avg_val
            } else if self.voltage_curvex_ref == 2 && avg_val != 0.0 {
                avg_val / (basekv * 1000.0)
            } else {
                vpresent / (basekv * 1000.0)
            };
            self.ctrl_vars[i].f_present_vpu = present_vpu;
            self.ctrl_vars[i].f_present_drc_vpu = vpresent / (basekv * 1000.0);
            self.f_vreg = present_vpu;

            // VOLTVAR: write the mode-3 monitor state (unobservable until WP7.7).
            let vreg = self.f_vreg;
            let vv_op = self.ctrl_vars[i].f_vv_operation;
            env.der_set_monitor_var(r, MonitorVar::Vreg, vreg);
            env.der_set_monitor_var(r, MonitorVar::VvOperation, vv_op);

            // If the inverter is off and following it, skip this DER.
            if !snap.inverter_on && snap.var_follow_inverter {
                continue;
            }
            // The volt-var curve must exist (Pascal error 382).
            if self.vvc_curve.is_none() {
                return Err(
                    "XY Curve object representing vvc1_curve does not exist or is not tied to InvControl."
                        .to_string(),
                );
            }
            // Pascal `Sample` sets only `DERElem.VVmode := TRUE` here; `VWmode :=
            // FALSE` / `Varmode := VARMODEKVAR` happen in `DoPendingAction`.
            env.der_set_vv_mode(r, true);

            // Trigger from the volt-var mode.
            let cv = &self.ctrl_vars[i];
            let v_trigger =
                (cv.f_present_vpu - cv.f_avgp_vpu_prior).abs() > self.voltage_change_tolerance;
            let q_trigger =
                (cv.qoutput_vvpu.abs() - cv.q_desire_endpu.abs()).abs() > self.var_change_tolerance;
            if v_trigger || q_trigger || control_iter == 1 {
                self.ctrl_vars[i].f_vv_operation = 0.0;
                self.ctrl_vars[i].f_pending_change = CHANGEVARLEVEL;
                env.push_change(self.ccd.time_delay, CHANGEVARLEVEL);
                if self.ccd.show_event_log {
                    let der = env.der_full_name(r);
                    let msg = format!(
                        "**Ready to change var output due to volt-var trigger in volt-var mode**, Vavgpu= {}, VPriorpu={}",
                        fmt_g(self.ctrl_vars[i].f_present_vpu, 5),
                        fmt_g(self.ctrl_vars[i].f_avgp_vpu_prior, 5)
                    );
                    env.append_event(&der, &msg);
                }
            }
        }
        Ok(())
    }

    /// Pascal `TInvControlObj.DoPendingAction` — the VOLTVAR + `CHANGEVARLEVEL`
    /// branch (the only path step 2b handles; the `Code` is ignored, matching
    /// Pascal — the per-DER `FPendingChange` drives the dispatch).
    pub(crate) fn do_pending_action(&mut self, env: &mut dyn InvDispatchEnv) {
        for k in 0..self.fleet.len() {
            let r = self.fleet[k];

            // Pascal's DoPendingAction header computes QHeadRoom, PBase and the
            // FPrior*pu's. VOLTVAR reads only QHeadRoom/QHeadRoomNeg
            // (Calc_QHeadRoom); PBase + the kW_out_desiredpu / FPrior*pu's drive VW
            // and the other modes (sub-steps 2c–2e) and have no side effect, so they
            // are deferred there.
            self.calc_qheadroom(k);

            if self.control_mode == VOLTVAR
                && self.combi_mode == NONE_COMBMODE
                && self.ctrl_vars[k].f_pending_change == CHANGEVARLEVEL
            {
                env.der_set_modes(r, false, true, crate::elements::pc::pvsystem::VARMODE_KVAR);

                // Main process (RateOfChangeMode INACTIVE — LPF/RF are step 2e).
                self.calc_qvv_curve_desiredpu(k, env);
                let q_desire_vvpu = self.ctrl_vars[k].q_desire_vvpu;
                self.check_qlimits(k, q_desire_vvpu);
                let limited = self.ctrl_vars[k].q_desire_limitedpu;
                self.ctrl_vars[k].q_desire_endpu =
                    q_desire_vvpu.abs().min(limited.abs()) * pas_sign(q_desire_vvpu);

                // Convergence algorithm → QDesiredVV (kvar set-point).
                self.calc_voltvar_vars(k);

                // Push the new kvar to the DER and recompute its P/Q.
                let q_desired_vv = self.ctrl_vars[k].q_desired_vv;
                env.der_set_kvar_requested(r, q_desired_vv);
                env.der_set_nominal(r);

                let present_kvar = env.der_present_kvar(r);
                let cv = &mut self.ctrl_vars[k];
                cv.qoutputpu = if q_desired_vv >= 0.0 {
                    present_kvar / cv.q_headroom
                } else {
                    present_kvar / cv.q_headroom_neg
                };
                cv.qoutput_vvpu = cv.qoutputpu;
                cv.f_avgp_vpu_prior = cv.f_present_vpu;
                cv.q_old = present_kvar;
                cv.q_old_vv = present_kvar;

                if self.ccd.show_event_log {
                    let der = env.der_full_name(r);
                    let msg = format!(
                        "VOLTVAR mode requested DER output var level to **, kvar = {}. Actual output set to kvar= {}.",
                        fmt_g(q_desired_vv, 5),
                        fmt_g(present_kvar, 5)
                    );
                    env.append_event(&der, &msg);
                }
            }
        }
    }

    /// Pascal `TInvControlObj.Reset` — `// inherited` (a no-op).
    pub(crate) fn reset(&mut self) {}

    /// Pascal `TInvControlObj.UpdateInvControl(i)` — feed each DER's solution
    /// voltage into the rolling-average windows and record the last two per-unit
    /// solution voltages (the hysteresis history). Called at the end of the power
    /// flow loop (`UpdateAll`).
    pub(crate) fn update_inv_control(&mut self, env: &mut dyn InvDispatchEnv) {
        self.ensure_fleet(env); // lazy build (Pascal `if FDERPointerList.Count = 0`)
        // Update the solution index once (Pascal gates on j=1, i=1; for a single
        // InvControl — every gated case — `i=1`, so the bump is unconditional here.
        // The multi-InvControl `i=1`-only quirk needs the element-list index the
        // per-element env doesn't carry; unobservable, only the hysteresis path).
        if self.f_vpu_solution_idx == 2 {
            self.f_vpu_solution_idx = 1;
        } else {
            self.f_vpu_solution_idx += 1;
        }
        let idx = self.f_vpu_solution_idx as usize;
        let dyna_h = env.dyna_h();

        for j in 0..self.fleet.len() {
            let basekv = self.ctrl_vars[j].f_vbase / 1000.0;
            self.ctrl_vars[j].prior_roll_avg_window = self.ctrl_vars[j].f_roll_avg_window.avg_val();
            self.ctrl_vars[j].prior_drc_roll_avg_window =
                self.ctrl_vars[j].f_drc_roll_avg_window.avg_val();

            let solnvoltage = if self.f_using_mon_buses {
                0.0 // MonBus path NOT_PORTED (step 2e); guarded at Sample
            } else {
                self.get_mon_voltage(j, env)
            };

            let roll_len = self.roll_avg_window_length as f64;
            let drc_len = self.drc_roll_avg_window_length as f64;
            self.ctrl_vars[j]
                .f_roll_avg_window
                .add(solnvoltage, dyna_h, roll_len);
            self.ctrl_vars[j]
                .f_drc_roll_avg_window
                .add(solnvoltage, dyna_h, drc_len);

            let bus_vbase = env.der_bus_vbase(self.fleet[j]);
            if idx < 3 {
                self.ctrl_vars[j].f_vpu_solution[idx] = if bus_vbase != 0.0 {
                    solnvoltage / bus_vbase
                } else {
                    0.0
                };
            }
            let _ = basekv;
        }
    }

    // --- the volt-var math (Pascal Calc* helpers) ---

    /// Pascal `Calc_QHeadRoom(j)`.
    fn calc_qheadroom(&mut self, j: usize) {
        let cv = &mut self.ctrl_vars[j];
        if self.reac_power_ref == super::REAC_POWER_VARAVAL {
            if cv.f_present_kw.abs() < cv.f_kva_rating {
                cv.q_headroom = (cv.f_kva_rating.powi(2) - cv.f_present_kw.powi(2)).sqrt();
            } else {
                cv.q_headroom = 0.0;
            }
            cv.q_headroom_neg = cv.q_headroom;
        }
        if self.reac_power_ref == REAC_POWER_VARMAX || self.control_mode == WATTPF {
            cv.q_headroom = cv.f_kvar_limit;
            cv.q_headroom_neg = cv.f_kvar_limit_neg;
        }
        if cv.q_headroom == 0.0 {
            cv.q_headroom = cv.f_kvar_limit;
        }
        if cv.q_headroom_neg == 0.0 {
            cv.q_headroom_neg = cv.f_kvar_limit_neg;
        }
    }

    /// Pascal `CalcQVVcurve_desiredpu(j)` — the volt-var curve lookup (with the
    /// hysteresis state machine for a non-zero `Hysteresis_Offset`).
    fn calc_qvv_curve_desiredpu(&mut self, j: usize, env: &mut dyn InvDispatchEnv) {
        let present_vpu = self.ctrl_vars[j].f_present_vpu;
        let f_present_kvar = self.ctrl_vars[j].f_present_kvar;
        let q_headroom = self.ctrl_vars[j].q_headroom;
        let q_headroom_neg = self.ctrl_vars[j].q_headroom_neg;
        let offset = self.vvc_curve_offset;

        self.ctrl_vars[j].q_desire_vvpu = 0.0;
        let q_present_pu = if f_present_kvar >= 0.0 {
            f_present_kvar / q_headroom
        } else {
            f_present_kvar / q_headroom_neg
        };

        // For the first two seconds, keep voltagechangesolution zero (no time-series
        // history yet).
        let mut voltage_change_solution = 0.0;
        if (env.dbl_hour() * 3600.0 / env.dyna_h()) >= 3.0 {
            let fv = &self.ctrl_vars[j].f_vpu_solution;
            if self.f_vpu_solution_idx == 1 {
                voltage_change_solution = fv[1] - fv[2];
            } else if self.f_vpu_solution_idx == 2 {
                voltage_change_solution = fv[2] - fv[1];
            }
        }

        let curve = self
            .vvc_curve
            .as_mut()
            .expect("vvc_curve checked at Sample");
        let active = self.ctrl_vars[j].f_active_vv_curve;
        let flag = self.ctrl_vars[j].flag_change_curve;

        let (q_desire, new_active, new_flag) = if offset == 0.0 {
            // No hysteresis: just look up the curve.
            (curve.get_y_value(present_vpu), active, flag)
        } else if voltage_change_solution > 0.0 && active == 1 {
            if flag {
                let vpu_from_curve = curve.get_x_value(q_present_pu);
                if (present_vpu - vpu_from_curve).abs() < self.voltage_change_tolerance / 2.0 {
                    (curve.get_y_value(present_vpu), active, false)
                } else {
                    (q_present_pu, active, false)
                }
            } else {
                (curve.get_y_value(present_vpu), active, flag)
            }
        } else if voltage_change_solution > 0.0 && active == 2 {
            (q_present_pu, 1, true)
        } else if voltage_change_solution < 0.0 && active == 2 {
            if flag {
                let vpu_from_curve = curve.get_x_value(q_present_pu) - offset;
                if (present_vpu - vpu_from_curve).abs() < self.voltage_change_tolerance / 2.0 {
                    (curve.get_y_value(present_vpu - offset), active, false)
                } else {
                    (q_present_pu, active, false)
                }
            } else {
                (curve.get_y_value(present_vpu - offset), active, flag)
            }
        } else if voltage_change_solution < 0.0 && active == 1 {
            (q_present_pu, 2, true)
        } else if voltage_change_solution == 0.0 && active == 1 && !flag {
            (curve.get_y_value(present_vpu), active, flag)
        } else if voltage_change_solution == 0.0 && flag {
            (q_present_pu, active, flag)
        } else if voltage_change_solution == 0.0 && active == 2 && !flag {
            (curve.get_y_value(present_vpu - offset), active, flag)
        } else {
            (0.0, active, flag)
        };

        let cv = &mut self.ctrl_vars[j];
        cv.q_desire_vvpu = q_desire;
        cv.f_active_vv_curve = new_active;
        cv.flag_change_curve = new_flag;
    }

    /// Pascal `Check_Qlimits(j, Q)` — clamp Q (pu) to the current kvar limit and,
    /// under watt priority, the kVA-available headroom; records `FVVOperation`.
    fn check_qlimits(&mut self, j: usize, q: f64) {
        let cv = &mut self.ctrl_vars[j];
        // The VOLTVAR error band.
        let error = if self.control_mode == VOLTVAR {
            0.005
        } else {
            0.0
        };

        let mut f_operation = if q < -error {
            -1.0
        } else if q > error {
            1.0
        } else {
            0.0
        };

        cv.q_desire_limitedpu = 1.0; // not limited

        let mut current_kvar_limit_pu = cv.f_current_kvar_limit / cv.q_headroom;
        let mut current_kvar_limit_neg_pu = cv.f_current_kvar_limit_neg / cv.q_headroom_neg;
        if current_kvar_limit_pu > cv.q_desire_limitedpu {
            current_kvar_limit_pu = cv.q_desire_limitedpu;
        }
        if current_kvar_limit_neg_pu > cv.q_desire_limitedpu {
            current_kvar_limit_neg_pu = cv.q_desire_limitedpu;
        }

        if q > 0.0 && q.abs() >= current_kvar_limit_pu.abs() {
            f_operation = 0.2 * pas_sign(q);
            cv.q_desire_limitedpu = current_kvar_limit_pu * pas_sign(q);
        } else if q < 0.0 && q.abs() >= current_kvar_limit_neg_pu.abs() {
            f_operation = 0.2 * pas_sign(q);
            cv.q_desire_limitedpu = current_kvar_limit_neg_pu * pas_sign(q);
        }

        // Watt-priority kVA-available clamp (VARMAX or WATTPF).
        if cv.f_p_priority
            && (self.reac_power_ref == REAC_POWER_VARMAX || self.control_mode == WATTPF)
        {
            let q_ppriority = if q >= 0.0 {
                (cv.f_kva_rating.powi(2) - cv.f_present_kw.powi(2)).sqrt() / cv.q_headroom
            } else {
                (cv.f_kva_rating.powi(2) - cv.f_present_kw.powi(2)).sqrt() / cv.q_headroom_neg
            };
            if q_ppriority.abs() < cv.q_desire_limitedpu.abs() && q_ppriority.abs() < q.abs() {
                f_operation = 0.6 * pas_sign(q);
                if q.abs() < (0.01 / 100.0) || q_ppriority.abs() < f64::EPSILON {
                    f_operation = 0.0;
                }
                cv.q_desire_limitedpu = q_ppriority * pas_sign(q);
            }
        }

        if self.control_mode == VOLTVAR {
            cv.f_vv_operation = f_operation;
        }
    }

    /// Pascal `CalcVoltVar_vars(j)` — the convergence step → `QDesiredVV`. Linear
    /// `ControlModel` only; Exponential (the `TPICtrl` PI controller) is NOT_PORTED.
    fn calc_voltvar_vars(&mut self, j: usize) {
        let cv = &mut self.ctrl_vars[j];
        if !cv.flag_change_curve {
            let mut delta_q = if cv.q_desire_endpu >= 0.0 {
                cv.q_desire_endpu * cv.q_headroom
            } else {
                cv.q_desire_endpu * cv.q_headroom_neg
            };
            if self.ctrl_model == MODEL_LINEAR {
                delta_q -= cv.q_old_vv;
                if self.delta_q_factor == FLAGDELTAQ {
                    // Adaptive factor (Change_deltaQ_factor); inlined to keep the
                    // single &mut borrow of cv.
                    let delta_v = (cv.f_present_vpu - cv.f_avgp_vpu_prior).abs();
                    if cv.delta_v_old >= 0.0 {
                        if delta_v.abs() > 0.8 * cv.delta_v_old && cv.f_delta_q_factor > 0.2 {
                            cv.f_delta_q_factor -= 0.1;
                        } else if delta_v.abs() > 0.6 * cv.delta_v_old && cv.f_delta_q_factor > 0.2
                        {
                            cv.f_delta_q_factor -= 0.05;
                        } else if delta_v.abs() < 0.2 * cv.delta_v_old && cv.f_delta_q_factor < 0.9
                        {
                            cv.f_delta_q_factor += 0.1;
                        } else if delta_v.abs() < 0.4 * cv.delta_v_old && cv.f_delta_q_factor < 0.9
                        {
                            cv.f_delta_q_factor += 0.05;
                        }
                    }
                    cv.delta_v_old = (cv.f_present_vpu - cv.f_avgp_vpu_prior).abs();
                } else {
                    // The pinned oracle runs with CompatFlags=0, so a set
                    // deltaQ_factor is used directly each iteration.
                    cv.f_delta_q_factor = self.delta_q_factor;
                }
                cv.q_desired_vv = cv.q_old_vv + delta_q * cv.f_delta_q_factor;
            } else {
                // Unreachable: the Exponential ControlModel (TPICtrl PI controller)
                // is rejected in `Sample` (WP7.7); kept for structural parity.
                cv.q_desired_vv = cv.q_old_vv;
            }
        } else {
            // Stay at the present var output level.
            cv.q_desired_vv = cv.f_present_kvar;
        }
    }

    /// The "mode/combi not yet ported" error (records the active mode for clarity).
    fn not_ported_mode(&self) -> String {
        format!(
            "InvControl.{}: only VOLTVAR is ported (WP7.5 step 2b); mode={} combi={} is deferred to 2c-2e",
            self.ccd.cd.obj.name(),
            self.control_mode,
            self.combi_mode
        )
    }
}

/// Split a `DERList` entry `"PVSystem.pv1"` into `("PVSystem", "pv1")` (Pascal
/// `StripExtension` / `StripClassName`). A bare name keeps an empty class.
fn split_class_name(full: &str) -> (String, String) {
    match full.split_once('.') {
        Some((class, name)) => (class.to_string(), name.to_string()),
        None => (String::new(), full.to_string()),
    }
}
