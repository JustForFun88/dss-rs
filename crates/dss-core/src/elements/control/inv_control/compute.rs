//! The InvControl control algorithm (WP7.5 step 2b — VOLTVAR; step 2c — VOLTWATT +
//! the VV_VW combi mode): the DER fleet build (`MakeDERList`), `RecalcElementData`'s
//! deferred tail, `UpdateDERParameters` / `GetMonVoltage`, the per-mode `Sample`
//! trigger logic, the `DoPendingAction` dispatch (volt-var / volt-watt / VV_VW),
//! `UpdateInvControl` (the rolling-average feed), and the curve/clamp/convergence
//! math (`CalcQVVcurve_desiredpu`/`Check_Qlimits`/`Calc_QHeadRoom`/`CalcVoltVar_vars`
//! for VV; `CalcPVWcurve_limitpu`/`Check_Plimits`/`Calc_PBase`/`CalcVoltWatt_watts`
//! for VW).
//!
//! Like [`StorageController`], the fleet and the controlled DERs are reached
//! through the executive via the [`InvDispatchEnv`] abstraction
//! (`solution/controls/dispatch.rs`), resolved against the class registry; the
//! fleet resolves lazily on the first `Sample`. The control's terminal bus
//! (Pascal `Setbus(1, MonitoredElement.Firstbus)`) is set at parse-time
//! edit-completion instead (see [`InvControl::recalc`]).
//!
//! Step 2d adds the **DRC** (dynamic reactive current) single mode and the
//! **VV_DRC** combi mode: the `CalcQDRC_desiredpu` dynamic-reactive-current law over
//! the DRC rolling-average window, `CalcDRC_vars`/`CalcVVDRC_vars` (the delta-Q
//! convergence), and the joint volt-var + DRC `DoPendingAction` for VV_DRC.
//!
//! Step 2e-i adds the **WATTPF** (watt-pf) and **WATTVAR** (watt-var) single modes:
//! the `CalcQWPcurve_desiredpu`/`CalcWATTPF_vars` (the curve-derived power factor →
//! kvar) and `CalcQWVcurve_desiredpu`/`Check_Qlimits_WV`/`Calc_PQ_WV`/
//! `CalcWATTVAR_vars` (the watt-var curve Q with the kVA-circle quadratic) plus the
//! `Sample` triggers and `DoPendingAction` branches.
//!
//! Step 2e-ii adds the **AVR** (active voltage regulation) single mode: the 3-stage
//! DQDV regulator (`CalcQAVR_desiredpu`/`CalcAVR_vars` + the `DoPendingAction`
//! control-iteration state machine — seed `QHeadRoom/2`, estimate `DQDV`, then
//! regulate toward `Vsetpoint`) plus the `Sample` trigger.
//!
//! Step 2e-iii adds the **`MonBus=` explicit monitored-bus voltage path**
//! (`GetMonVoltage`'s `FUsingMonBuses` branch — per-bus single-node / line-to-line
//! voltages scaled to `FMonBusesVbase`, reduced AVG/MAX/MIN/phase) and the **LPF /
//! Rise-Fall rate-of-change limiting** (`CalcLPF`/`CalcRF` smoothing
//! `QDesireOptionpu`/`PLimitOptionpu` against the prior step's value before the
//! `Check_Qlimits`/`Check_Plimits` clamp, wired into the VOLTVAR / DRC / VV_DRC /
//! VOLTWATT / VV_VW `DoPendingAction` branches).
//!
//! **NOT_PORTED / deferred (each an explicit error, never a silent skip):** GFM →
//! WP7.7, **Storage** in VOLTWATT/VV_VW (the YPrim-state-flip propagation gap;
//! PVSystem volt-watt is ported), and the Exponential `ControlModel` (the `TPICtrl`
//! PI controller → WP7.7).
//!
//! [`StorageController`]: crate::elements::control::storage_controller
//! [`InvDispatchEnv`]: InvDispatchEnv

use num_complex::Complex64;

use crate::elements::traits::ElemRef;
use crate::util::fmt_g;

use super::{
    AVR, CHANGE_NONE, CHANGEDRCVVARLEVEL, CHANGEVARLEVEL, CHANGEWATTLEVEL, CHANGEWATTVARLEVEL,
    DELTAPDEFAULT, DRC, FLAGDELTAP, FLAGDELTAQ, InvControl, MAXPHASE, MINPHASE, MODEL_LINEAR,
    NONE_COMBMODE, NONE_MODE, REAC_POWER_VARMAX, ROC_LPF, ROC_RISEFALL, VOLTVAR, VOLTWATT, VV_DRC,
    VV_VW, WATTPF, WATTVAR,
};

/// Reduce the explicit-`MonBus` complex voltage buffer to a scalar by
/// `MonVoltageCalc` (Pascal `GetMonVoltage`'s `FUsingMonBuses` `case`):
/// AVG/MAX/MIN over `|cBuffer[j]|`, else a specific phase.
fn reduce_mon_phase(cbuffer: &[Complex64], mon_phase: i32) -> f64 {
    match mon_phase {
        super::AVGPHASES => {
            if cbuffer.is_empty() {
                0.0
            } else {
                cbuffer.iter().map(|c| c.norm()).sum::<f64>() / cbuffer.len() as f64
            }
        }
        MAXPHASE => cbuffer.iter().map(|c| c.norm()).fold(0.0, f64::max),
        MINPHASE => cbuffer.iter().map(|c| c.norm()).fold(1.0e50, f64::min),
        // A specific phase. Pascal reads `Cabs(cBuffer[FMonBusesPhase])` — and in
        // the MonBus branch `cBuffer` is filled 0-based (`cBuffer[0..len-1]`), so
        // the phase number indexes it directly (an upstream quirk; the corpus
        // MonBus cases all use AVG/MAX, so this arm is unexercised).
        p => cbuffer.get(p as usize).map_or(0.0, |c| c.norm()),
    }
}

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
    /// `DERElem.GetPFPriority()` — the inverter PF-priority flag (distinct from
    /// `P_Priority`); read by `CalcQWPcurve_desiredpu` (WATTPF).
    pub pf_priority: bool,
    // --- volt-watt fields (VOLTWATT / VV_VW; UpdateDERParameters + Calc_PBase).
    // PVSystem-only: the Storage VOLTWATT/VV_VW dispatch is deferred (an explicit
    // error in `sample_voltwatt`/`sample_vv_vw`), so the Storage-specific reads
    // (`TStorageObj.DCkW`/`StorageState`/`FVWStateRequested`) are not plumbed here. ---
    /// `FDCkW` — PVSystem `PanelkW`.
    pub dckw: f64,
    /// `FDCkWRated` — PVSystem `Pmpp`.
    pub dckw_rated: f64,
    /// `FpctDCkWRated` — PVSystem `puPmpp`.
    pub pct_dckw_rated: f64,
    /// `FEffFactor` — the inverter efficiency factor.
    pub eff_factor: f64,
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
    /// `DERElem.IsPVSystem()` — true for a PVSystem, false for a Storage (the
    /// WATTVAR PVSystem-only kW push in `DoPendingAction`).
    fn der_is_pvsystem(&self, r: ElemRef) -> bool;
    /// Pascal `DERElem.ComputeVTerminal` then `Cabs(Vterminal[1..NPhases])` — the
    /// per-phase terminal voltage magnitudes (used by `GetMonVoltage`).
    fn der_vterminal_mags(&mut self, r: ElemRef) -> Vec<f64>;
    /// `ActiveCircuit.Buses[DERElem.terminals[0].busRef].kVBase * 1000` — the L-N
    /// base volts for the `FVpuSolution` per-unit (UpdateInvControl).
    fn der_bus_vbase(&self, r: ElemRef) -> f64;
    /// Pascal `GetMonVoltage`'s explicit-`MonBus` node read:
    /// `ActiveCircuit.Solution.NodeV[Buses[BusList.Find(FMonBuses[j])].GetRef(node)]`
    /// — the complex voltage at the `j`-th monitored bus's `node` (1-based node
    /// number, used as `TDSSBus.GetRef`'s 1-based index, returning the ground node
    /// `NodeV[0]=0` out of range). `j` indexes the control's parsed `mon_buses`.
    fn mon_bus_node_v(&self, j: usize, node: i32) -> Complex64;
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
    /// Set only `DERElem.VWmode` (the VOLTWATT `Sample`/`DoPendingAction` path:
    /// Pascal sets just `VWmode`, leaving `Varmode`/`VVmode` untouched).
    fn der_set_vw_mode(&mut self, r: ElemRef, value: bool);
    /// Set only `DERElem.DRCmode` (the DRC / VV_DRC `Sample`/`DoPendingAction` path).
    fn der_set_drc_mode(&mut self, r: ElemRef, value: bool);
    /// Set only `DERElem.WPmode` (the WATTPF `Sample`/`DoPendingAction` path).
    fn der_set_wp_mode(&mut self, r: ElemRef, value: bool);
    /// Set only `DERElem.WVmode` (the WATTVAR `Sample`/`DoPendingAction` path).
    fn der_set_wv_mode(&mut self, r: ElemRef, value: bool);
    /// Set only `DERElem.AVRmode` (the AVR `Sample`/`DoPendingAction` path).
    fn der_set_avr_mode(&mut self, r: ElemRef, value: bool);
    /// Pascal `DERElem.Varmode := value` for both DER types (the explicit
    /// `Varmode := VARMODEKVAR` the WATTPF/WATTVAR/AVR `DoPendingAction` sets — so
    /// `SetNominalDEROutput` applies `kvarRequested`. For PVSystem this is redundant
    /// with `der_set_kvar_requested` modeling `Set_Presentkvar`, but a Storage's
    /// `kvarRequested` write has no such side effect, so without it a Storage stays
    /// `VARMODE_PF` and the requested kvar is silently dropped).
    fn der_set_var_mode(&mut self, r: ElemRef, mode: i32);
    /// `TStorageObj.kvarRequested` / `TPVSystemObj.kvarRequested` — the *requested*
    /// kvar (not the achieved `Get_Presentkvar`). AVR's iter-2 `DQDV` reads this for a
    /// Storage (Pascal l.1081), where PVSystem reads the achieved `Presentkvar`.
    fn der_requested_kvar(&self, r: ElemRef) -> f64;
    /// `TPVSystemObj.pf_wp_nominal := value` (WATTPF; PVSystem only — Storage
    /// instead takes the `kvarRequested := QDesiredWP` branch handled via
    /// `der_set_kvar_requested`).
    fn der_set_pf_wp_nominal(&mut self, r: ElemRef, value: f64);
    /// `TPVSystemObj.Presentkvar := q` / `TStorageObj.kvarRequested := q`.
    fn der_set_kvar_requested(&mut self, r: ElemRef, q: f64);
    /// `TPVSystemObj.PresentkW := p` / `TStorageObj.kWRequested := p` — both write
    /// the DER's `kWRequested` field (the volt-watt kW set-point).
    fn der_set_kw_requested(&mut self, r: ElemRef, p: f64);
    /// `DERElem.Get_PresentkW` (read back after `SetNominalDEROutput`, for the
    /// volt-watt event-log + the `FVWOperation` reset check).
    fn der_present_kw(&self, r: ElemRef) -> f64;
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
    /// `ActiveCircuit.Solution.DynaVars.t` (seconds within the step) — the DRC
    /// `Dynavars.t = 1` guard in `CalcQDRC_desiredpu`.
    fn dyna_t(&self) -> f64;
    /// Pascal `ActiveCircuit.Solution.LoadsNeedUpdating := TRUE` (`DoPendingAction`
    /// l.1605) — force the next solve to re-run `SetNominalDEROutput` over every PC
    /// element. Load-bearing for the AVR modes, whose iteration-1/2 dispatch sets
    /// `kvarRequested` *without* calling `SetNominalDEROutput` directly (so the
    /// re-solve must pick the request up); harmless/idempotent for the modes that do
    /// call `der_set_nominal` (the recompute from the same request is a no-op).
    fn set_loads_need_updating(&mut self);
}

/// Which mode-3 monitor state variable a `der_set_monitor_var` write targets.
#[derive(Debug, Clone, Copy)]
pub(crate) enum MonitorVar {
    /// Pascal `Set_Variable(5/14, FVreg)`.
    Vreg,
    /// Pascal `Set_Variable(7/16, FVVOperation)`.
    VvOperation,
    /// Pascal `Set_Variable(8/17, FVWOperation)` (VOLTWATT / VV_VW).
    VwOperation,
    /// Pascal `Set_Variable(6/15, FDRCRollAvgWindow.AvgVal/(basekV*1000))` — the DRC
    /// rolling-average voltage saved to the monitor (DRC / VV_DRC).
    DrcAvg,
    /// Pascal `Set_Variable(9/18, FDRCOperation)` (DRC).
    DrcOperation,
    /// Pascal `Set_Variable(10/19, FVVDRCOperation)` (VV_DRC).
    VvDrcOperation,
    /// Pascal `Set_Variable(11/16, FWPOperation)` (WATTPF).
    WpOperation,
    /// Pascal `Set_Variable(12/16, FWVOperation)` (WATTVAR).
    WvOperation,
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
            if self.control_mode != VOLTWATT && self.control_mode != WATTPF {
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
        cv.f_pf_priority = snap.pf_priority;
        // volt-watt DER parameters (Calc_PBase / Check_Plimits / CalcPVWcurve_limitpu).
        cv.f_dckw = snap.dckw;
        cv.f_dckw_rated = snap.dckw_rated;
        cv.f_pct_dckw_rated = snap.pct_dckw_rated;
        cv.f_eff_factor = snap.eff_factor;
    }

    /// Pascal `TInvControlObj.GetMonVoltage(Vpresent, i, BasekV)`.
    ///
    /// With `MonBus=` named explicit monitored buses (`FUsingMonBuses`), each
    /// monitored bus contributes a complex voltage `cBuffer[j]`: a single-node
    /// magnitude or a 2-node line-to-line difference, scaled by `BasekV·1000 /
    /// FMonBusesVbase[j]`; the buffer is then reduced by `MonVoltageCalc`
    /// (AVG/MAX/MIN/phase). Without `MonBus`, the per-DER self-monitoring path
    /// reduces the DER's own terminal-voltage magnitudes the same way.
    fn get_mon_voltage(&self, i: usize, basekv: f64, env: &mut dyn InvDispatchEnv) -> f64 {
        if self.f_using_mon_buses {
            // The complex per-bus monitored voltages (Pascal `cBuffer[0..len-1]`).
            let cbuffer: Vec<Complex64> = (0..self.mon_buses.len())
                .map(|j| {
                    let nodes = &self.mon_buses_nodes[j];
                    // FMonBusesVbase[j+1] (Pascal 1-based) = mon_buses_vbase[j].
                    let vbase = self.mon_buses_vbase.get(j).copied().unwrap_or(0.0);
                    let scale = if vbase != 0.0 {
                        basekv * 1000.0 / vbase
                    } else {
                        0.0
                    };
                    if nodes.len() == 2 {
                        let vi = env.mon_bus_node_v(j, nodes[0]);
                        let vj = env.mon_bus_node_v(j, nodes[1]);
                        (vi - vj) * scale
                    } else {
                        let node = nodes.first().copied().unwrap_or(0);
                        env.mon_bus_node_v(j, node) * scale
                    }
                })
                .collect();
            return reduce_mon_phase(&cbuffer, self.mon_buses_phase);
        }

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

    /// Pascal `TInvControlObj.Sample`. Step 2b/2c: the VOLTVAR + VOLTWATT single
    /// modes and the VV_VW combi mode; every other mode/combi records an explicit
    /// NOT_PORTED error (never a silent skip).
    pub(crate) fn sample(&mut self, env: &mut dyn InvDispatchEnv) -> Result<(), String> {
        self.ensure_fleet(env); // lazy build (Pascal `if FDERPointerList.Count = 0`)
        if self.f_list_size <= 0 {
            return Ok(());
        }

        // Mode / combi-mode gating: ports VOLTVAR + VOLTWATT + DRC + VV_VW + VV_DRC.
        if self.combi_mode != NONE_COMBMODE {
            if self.combi_mode != VV_VW && self.combi_mode != VV_DRC {
                return Err(self.not_ported_mode());
            }
        } else {
            match self.control_mode {
                NONE_MODE | VOLTVAR | VOLTWATT | DRC | WATTPF | WATTVAR | AVR => {}
                _ => return Err(self.not_ported_mode()), // GFM → WP7.7
            }
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
            let vpresent = self.get_mon_voltage(i, basekv, env);

            // ControlIteration 1: seed the prior averages (for the event log).
            if control_iter == 1 {
                self.ctrl_vars[i].f_avgp_vpu_prior = self.ctrl_vars[i].f_present_vpu;
                self.ctrl_vars[i].f_avgp_drc_vpu_prior = self.ctrl_vars[i].f_present_drc_vpu;
            }

            // `kW_out_desired := FpresentkW` each control iteration (Pascal l.1796 —
            // refreshed for VW/VV_VW before the per-mode dispatch).
            self.ctrl_vars[i].kw_out_desired = self.ctrl_vars[i].f_present_kw;

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

            // Dispatch by mode/combi (Pascal's `if CombiMode <> NONE_COMBMODE` /
            // `case ControlMode`).
            if self.combi_mode == VV_VW {
                self.sample_vv_vw(i, env, snap, control_iter)?;
            } else if self.combi_mode == VV_DRC {
                self.sample_vv_drc(i, env, snap, control_iter)?;
            } else {
                match self.control_mode {
                    VOLTVAR => self.sample_voltvar(i, env, snap, control_iter)?,
                    VOLTWATT => self.sample_voltwatt(i, env, snap, control_iter)?,
                    DRC => self.sample_drc(i, env, snap, control_iter),
                    WATTPF => self.sample_wattpf(i, env, snap, control_iter)?,
                    WATTVAR => self.sample_wattvar(i, env, snap, control_iter)?,
                    AVR => self.sample_avr(i, env, snap, control_iter)?,
                    _ => {} // NONE_MODE: do nothing
                }
            }
        }
        Ok(())
    }

    /// Pascal `Sample`'s `VOLTVAR` arm (the shared prologue ran in [`Self::sample`]).
    /// Returns `Ok(())` to model Pascal's `continue` (inverter off + following).
    fn sample_voltvar(
        &mut self,
        i: usize,
        env: &mut dyn InvDispatchEnv,
        snap: DerSnap,
        control_iter: i32,
    ) -> Result<(), String> {
        let r = self.fleet[i];

        // Write the mode-3 monitor state (unobservable until WP7.7).
        let vreg = self.f_vreg;
        let vv_op = self.ctrl_vars[i].f_vv_operation;
        env.der_set_monitor_var(r, MonitorVar::Vreg, vreg);
        env.der_set_monitor_var(r, MonitorVar::VvOperation, vv_op);

        // If the inverter is off and following it, skip this DER.
        if !snap.inverter_on && snap.var_follow_inverter {
            return Ok(());
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
        Ok(())
    }

    /// Pascal `Sample`'s `VOLTWATT` arm. Note the inverter check is `FInverterON =
    /// FALSE` alone (no `VarFollowInverter`, unlike the var modes).
    fn sample_voltwatt(
        &mut self,
        i: usize,
        env: &mut dyn InvDispatchEnv,
        snap: DerSnap,
        control_iter: i32,
    ) -> Result<(), String> {
        let r = self.fleet[i];
        self.guard_storage_vw(snap)?;

        // Set_Variable(5, FVreg); Set_Variable(8, FVWOperation).
        let vreg = self.f_vreg;
        let vw_op = self.ctrl_vars[i].f_vw_operation;
        env.der_set_monitor_var(r, MonitorVar::Vreg, vreg);
        env.der_set_monitor_var(r, MonitorVar::VwOperation, vw_op);

        if !snap.inverter_on {
            return Ok(());
        }
        self.check_voltwatt_curve(snap)?;
        env.der_set_vw_mode(r, true); // DERElem.VWmode := TRUE

        let cv = &self.ctrl_vars[i];
        let trigger = (cv.f_present_vpu - cv.f_avgp_vpu_prior).abs()
            > self.voltage_change_tolerance
            || (cv.p_limit_endpu - cv.p_old_vw_pu).abs() > self.active_p_change_tolerance
            || control_iter == 1;
        if trigger {
            self.ctrl_vars[i].f_vw_operation = 0.0;
            self.ctrl_vars[i].f_pending_change = CHANGEWATTLEVEL;
            env.push_change(self.ccd.time_delay, CHANGEWATTLEVEL);
            if self.ccd.show_event_log {
                let der = env.der_full_name(r);
                let msg = format!(
                    "**Ready to limit watt output due to VOLTWATT mode**, Vavgpu= {}, VPriorpu={}",
                    fmt_g(self.ctrl_vars[i].f_present_vpu, 5),
                    fmt_g(self.ctrl_vars[i].f_avgp_vpu_prior, 5)
                );
                env.append_event(&der, &msg);
            }
        }
        Ok(())
    }

    /// Pascal `Sample`'s `VV_VW` combi arm: a volt-watt trigger *and* a volt-var
    /// trigger, both queuing `CHANGEWATTVARLEVEL` (so both can push in one Sample).
    fn sample_vv_vw(
        &mut self,
        i: usize,
        env: &mut dyn InvDispatchEnv,
        snap: DerSnap,
        control_iter: i32,
    ) -> Result<(), String> {
        let r = self.fleet[i];
        self.guard_storage_vw(snap)?;

        // Set_Variable(5, FVreg); (7, FVVOperation); (8, FVWOperation).
        let vreg = self.f_vreg;
        let vv_op = self.ctrl_vars[i].f_vv_operation;
        let vw_op = self.ctrl_vars[i].f_vw_operation;
        env.der_set_monitor_var(r, MonitorVar::Vreg, vreg);
        env.der_set_monitor_var(r, MonitorVar::VvOperation, vv_op);
        env.der_set_monitor_var(r, MonitorVar::VwOperation, vw_op);

        if !snap.inverter_on && snap.var_follow_inverter {
            return Ok(());
        }
        self.check_voltwatt_curve(snap)?;
        if self.vvc_curve.is_none() {
            return Err(
                "XY Curve object representing vvc1_curve does not exist or is not tied to InvControl."
                    .to_string(),
            );
        }
        env.der_set_vv_mode(r, true);
        env.der_set_vw_mode(r, true);

        // Trigger from volt-watt mode.
        let cv = &self.ctrl_vars[i];
        let vw_trigger = (cv.f_present_vpu - cv.f_avgp_vpu_prior).abs()
            > self.voltage_change_tolerance
            || (cv.p_limit_endpu - cv.p_old_vw_pu).abs() > self.active_p_change_tolerance
            || control_iter == 1;
        if vw_trigger {
            self.ctrl_vars[i].f_vw_operation = 0.0;
            self.ctrl_vars[i].f_pending_change = CHANGEWATTVARLEVEL;
            env.push_change(self.ccd.time_delay, CHANGEWATTVARLEVEL);
            if self.ccd.show_event_log {
                let der = env.der_full_name(r);
                let msg = format!(
                    "**Ready to change VV_VW output due to volt-watt trigger**, Vavgpu= {}, VPriorpu={}",
                    fmt_g(self.ctrl_vars[i].f_present_vpu, 5),
                    fmt_g(self.ctrl_vars[i].f_avgp_vpu_prior, 5)
                );
                env.append_event(&der, &msg);
            }
        }

        // Trigger from volt-var mode (uses `Qoutputpu`, not `QoutputVVpu`).
        let cv = &self.ctrl_vars[i];
        let vv_trigger = (cv.f_present_vpu - cv.f_avgp_vpu_prior).abs()
            > self.voltage_change_tolerance
            || (cv.qoutputpu.abs() - cv.q_desire_endpu.abs()).abs() > self.var_change_tolerance
            || control_iter == 1;
        if vv_trigger {
            self.ctrl_vars[i].f_vv_operation = 0.0;
            self.ctrl_vars[i].f_pending_change = CHANGEWATTVARLEVEL;
            env.push_change(self.ccd.time_delay, CHANGEWATTVARLEVEL);
            if self.ccd.show_event_log {
                let der = env.der_full_name(r);
                let msg = format!(
                    "**Ready to change VV_VW output due to volt-var trigger**, Vavgpu= {}, VPriorpu={}",
                    fmt_g(self.ctrl_vars[i].f_present_vpu, 5),
                    fmt_g(self.ctrl_vars[i].f_avgp_vpu_prior, 5)
                );
                env.append_event(&der, &msg);
            }
        }
        Ok(())
    }

    /// Pascal `Sample`'s `DRC` (dynamic reactive current) arm. DRC needs no curve;
    /// it responds to the per-step voltage change the DRC rolling-average window
    /// tracks (so it is a no-op in a pure snapshot — the window is fed only in the
    /// time-series `EndOfTimeStepCleanup`).
    fn sample_drc(
        &mut self,
        i: usize,
        env: &mut dyn InvDispatchEnv,
        snap: DerSnap,
        control_iter: i32,
    ) {
        let r = self.fleet[i];

        // Set_Variable(5, FVreg); (6, DRC rolling-avg pu); (9, FDRCOperation).
        let vreg = self.f_vreg;
        let drc_avg = self.drc_avg_pu(i);
        let drc_op = self.ctrl_vars[i].f_drc_operation;
        env.der_set_monitor_var(r, MonitorVar::Vreg, vreg);
        env.der_set_monitor_var(r, MonitorVar::DrcAvg, drc_avg);
        env.der_set_monitor_var(r, MonitorVar::DrcOperation, drc_op);

        // If the inverter is off and following it, skip this DER.
        if !snap.inverter_on && snap.var_follow_inverter {
            return;
        }

        // DRC trigger from the freshly-seeded window (Pascal `priorDRCRollAvgWindow
        // = 0.0`): resets FDRCOperation, queues CHANGEVARLEVEL.
        let cv = &self.ctrl_vars[i];
        if cv.prior_drc_roll_avg_window == 0.0
            && (cv.f_present_drc_vpu - cv.f_avgp_drc_vpu_prior).abs()
                > self.voltage_change_tolerance
        {
            self.ctrl_vars[i].f_drc_operation = 0.0;
            self.ctrl_vars[i].f_pending_change = CHANGEVARLEVEL;
            env.push_change(self.ccd.time_delay, CHANGEVARLEVEL);
            if self.ccd.show_event_log {
                let der = env.der_full_name(r);
                let msg = format!(
                    "**Ready to change var output due to DRC trigger in DRC mode**, Vavgpu= {}, VPriorpu={}",
                    fmt_g(self.ctrl_vars[i].f_present_drc_vpu, 5),
                    fmt_g(self.ctrl_vars[i].f_avgp_drc_vpu_prior, 5)
                );
                env.append_event(&der, &msg);
            }
        }

        env.der_set_drc_mode(r, true); // DERElem.DRCmode := TRUE (Pascal l.2260)

        // Main DRC trigger.
        let cv = &self.ctrl_vars[i];
        let trigger = (cv.f_present_drc_vpu - cv.f_avgp_drc_vpu_prior).abs()
            > self.voltage_change_tolerance
            || (cv.qoutput_drcpu.abs() - cv.q_desire_endpu.abs()).abs() > self.var_change_tolerance
            || control_iter == 1;
        if trigger {
            self.ctrl_vars[i].f_pending_change = CHANGEVARLEVEL;
            env.push_change(self.ccd.time_delay, CHANGEVARLEVEL);
            if self.ccd.show_event_log {
                let der = env.der_full_name(r);
                let msg = format!(
                    "**Ready to change var output due to DRC trigger in DRC mode**, Vavgpu= {}, VPriorpu={}, QoutPU={}, QDesiredEndpu={}",
                    fmt_g(self.ctrl_vars[i].f_present_drc_vpu, 5),
                    fmt_g(self.ctrl_vars[i].f_avgp_drc_vpu_prior, 5),
                    fmt_g(self.ctrl_vars[i].qoutput_drcpu, 3),
                    fmt_g(self.ctrl_vars[i].q_desire_endpu, 3)
                );
                env.append_event(&der, &msg);
            }
        }
    }

    /// Pascal `Sample`'s `WATTPF` arm: the watt-pf control. The trigger uses
    /// `QoutputVVpu` (shared with the var modes); the inverter-off check is the var
    /// form (`FInverterON=FALSE AND VarFollowInverter`).
    fn sample_wattpf(
        &mut self,
        i: usize,
        env: &mut dyn InvDispatchEnv,
        snap: DerSnap,
        control_iter: i32,
    ) -> Result<(), String> {
        let r = self.fleet[i];

        // Set_Variable(5, FVreg); Set_Variable(11, FWPOperation).
        let vreg = self.f_vreg;
        let wp_op = self.ctrl_vars[i].f_wp_operation;
        env.der_set_monitor_var(r, MonitorVar::Vreg, vreg);
        env.der_set_monitor_var(r, MonitorVar::WpOperation, wp_op);

        if !snap.inverter_on && snap.var_follow_inverter {
            return Ok(());
        }
        if self.wattpf_curve.is_none() {
            return Err(
                "XY Curve object representing wattpf_curve does not exist or is not tied to InvControl."
                    .to_string(),
            );
        }
        env.der_set_wp_mode(r, true); // DERElem.WPmode := TRUE

        let cv = &self.ctrl_vars[i];
        let trigger = (cv.f_present_vpu - cv.f_avgp_vpu_prior).abs()
            > self.voltage_change_tolerance
            || (cv.qoutput_vvpu.abs() - cv.q_desire_endpu.abs()).abs() > self.var_change_tolerance
            || control_iter == 1;
        if trigger {
            self.ctrl_vars[i].f_wp_operation = 0.0;
            self.ctrl_vars[i].f_pending_change = CHANGEVARLEVEL;
            env.push_change(self.ccd.time_delay, CHANGEVARLEVEL);
            if self.ccd.show_event_log {
                let der = env.der_full_name(r);
                let msg = format!(
                    "**Ready to change var output due to watt-pf trigger in watt-pf mode**, Vavgpu= {}, VPriorpu={}",
                    fmt_g(self.ctrl_vars[i].f_present_vpu, 5),
                    fmt_g(self.ctrl_vars[i].f_avgp_vpu_prior, 5)
                );
                env.append_event(&der, &msg);
            }
        }
        Ok(())
    }

    /// Pascal `Sample`'s `WATTVAR` arm: the watt-var control. Same trigger shape as
    /// WATTPF (`QoutputVVpu`); records `FWVOperation` to monitor var 12.
    fn sample_wattvar(
        &mut self,
        i: usize,
        env: &mut dyn InvDispatchEnv,
        snap: DerSnap,
        control_iter: i32,
    ) -> Result<(), String> {
        let r = self.fleet[i];

        // Set_Variable(5, FVreg); Set_Variable(12, FWVOperation).
        let vreg = self.f_vreg;
        let wv_op = self.ctrl_vars[i].f_wv_operation;
        env.der_set_monitor_var(r, MonitorVar::Vreg, vreg);
        env.der_set_monitor_var(r, MonitorVar::WvOperation, wv_op);

        if !snap.inverter_on && snap.var_follow_inverter {
            return Ok(());
        }
        if self.wattvar_curve.is_none() {
            return Err(
                "XY Curve object representing wattvar_curve does not exist or is not tied to InvControl."
                    .to_string(),
            );
        }
        env.der_set_wv_mode(r, true); // DERElem.WVmode := TRUE

        let cv = &self.ctrl_vars[i];
        let trigger = (cv.f_present_vpu - cv.f_avgp_vpu_prior).abs()
            > self.voltage_change_tolerance
            || (cv.qoutput_vvpu.abs() - cv.q_desire_endpu.abs()).abs() > self.var_change_tolerance
            || control_iter == 1;
        if trigger {
            self.ctrl_vars[i].f_wv_operation = 0.0;
            self.ctrl_vars[i].f_pending_change = CHANGEVARLEVEL;
            env.push_change(self.ccd.time_delay, CHANGEVARLEVEL);
            if self.ccd.show_event_log {
                let der = env.der_full_name(r);
                let msg = format!(
                    "**Ready to change var output due to watt-var trigger in watt-var mode**, Vavgpu= {}, VPriorpu={}",
                    fmt_g(self.ctrl_vars[i].f_present_vpu, 5),
                    fmt_g(self.ctrl_vars[i].f_avgp_vpu_prior, 5)
                );
                env.append_event(&der, &msg);
            }
        }
        Ok(())
    }

    /// Pascal `Sample`'s `AVR` (active voltage regulation) arm. AVR needs **no
    /// curve** — it regulates the monitored voltage toward `Vsetpoint` via the
    /// 3-stage DQDV process in `DoPendingAction`. The trigger compares the present
    /// voltage against both the prior average and the (kvar-limited) setpoint. Unlike
    /// the other var modes the AVR arm writes no mode-3 monitor `Set_Variable` (Pascal
    /// l.2040-2076), and for a PVSystem it sets `AVRmode := TRUE` (a Storage sets
    /// `VVmode := TRUE` instead — verbatim Pascal l.2051-2054).
    fn sample_avr(
        &mut self,
        i: usize,
        env: &mut dyn InvDispatchEnv,
        snap: DerSnap,
        control_iter: i32,
    ) -> Result<(), String> {
        let r = self.fleet[i];

        // If the inverter is off and following it, skip this DER.
        if !snap.inverter_on && snap.var_follow_inverter {
            return Ok(());
        }
        // PVSystem sets AVRmode; a Storage sets VVmode (verbatim Pascal l.2051-2054).
        if snap.is_pvsystem {
            env.der_set_avr_mode(r, true);
        } else {
            env.der_set_vv_mode(r, true);
        }

        // Trigger from AVR mode: the voltage moved vs the prior average OR vs the
        // (limited) setpoint, the achieved Q drifted from the target, or iter 1.
        let v_setpoint = self.v_setpoint;
        let cv = &self.ctrl_vars[i];
        let trigger = (cv.f_present_vpu - cv.f_avgp_vpu_prior).abs()
            > self.voltage_change_tolerance
            || (cv.qoutput_avrpu.abs() - cv.q_desire_endpu.abs()).abs() > self.var_change_tolerance
            || (cv.f_present_vpu - cv.f_v_setpoint_limited).abs() > self.voltage_change_tolerance
            || control_iter == 1;
        if trigger {
            self.ctrl_vars[i].f_avr_operation = 0.0;
            self.ctrl_vars[i].f_pending_change = CHANGEVARLEVEL;
            env.push_change(self.ccd.time_delay, CHANGEVARLEVEL);
            if self.ccd.show_event_log {
                let der = env.der_full_name(r);
                let cv = &self.ctrl_vars[i];
                let msg = format!(
                    "**Ready to change var output due to AVR trigger in AVR mode**, Vavgpu= {}, VPriorpu={}, Vsetpoint={}, VsetpointLimited={}",
                    fmt_g(cv.f_present_vpu, 5),
                    fmt_g(cv.f_avgp_vpu_prior, 5),
                    fmt_g(v_setpoint, 5),
                    fmt_g(cv.f_v_setpoint_limited, 5)
                );
                env.append_event(&der, &msg);
            }
        }
        Ok(())
    }

    /// Pascal `Sample`'s `VV_DRC` combi arm: a volt-var trigger AND a DRC trigger,
    /// both queuing `CHANGEDRCVVARLEVEL` (so both can push in one Sample). Needs the
    /// volt-var curve (Pascal error 382).
    fn sample_vv_drc(
        &mut self,
        i: usize,
        env: &mut dyn InvDispatchEnv,
        snap: DerSnap,
        control_iter: i32,
    ) -> Result<(), String> {
        let r = self.fleet[i];

        // Set_Variable(5, FVreg); (6, DRC rolling-avg pu); (10, FVVDRCOperation).
        let vreg = self.f_vreg;
        let drc_avg = self.drc_avg_pu(i);
        let vvdrc_op = self.ctrl_vars[i].f_vvdrc_operation;
        env.der_set_monitor_var(r, MonitorVar::Vreg, vreg);
        env.der_set_monitor_var(r, MonitorVar::DrcAvg, drc_avg);
        env.der_set_monitor_var(r, MonitorVar::VvDrcOperation, vvdrc_op);

        if !snap.inverter_on && snap.var_follow_inverter {
            return Ok(());
        }
        if self.vvc_curve.is_none() {
            return Err(
                "XY Curve object representing vvc1_curve does not exist or is not tied to InvControl."
                    .to_string(),
            );
        }
        env.der_set_vv_mode(r, true);
        env.der_set_drc_mode(r, true);

        // DRC trigger from the freshly-seeded window (Pascal `priorDRCRollAvgWindow
        // = 0.0`): the volt-var OR DRC voltage moved.
        let cv = &self.ctrl_vars[i];
        if cv.prior_drc_roll_avg_window == 0.0
            && ((cv.f_present_drc_vpu - cv.f_avgp_drc_vpu_prior).abs()
                > self.voltage_change_tolerance
                || (cv.f_present_vpu - cv.f_avgp_vpu_prior).abs() > self.voltage_change_tolerance)
        {
            self.ctrl_vars[i].f_vvdrc_operation = 0.0;
            self.ctrl_vars[i].f_pending_change = CHANGEDRCVVARLEVEL;
            env.push_change(self.ccd.time_delay, CHANGEDRCVVARLEVEL);
            if self.ccd.show_event_log {
                let der = env.der_full_name(r);
                let msg = format!(
                    "**Ready to change var output due to DRC trigger in VV_DRC mode**, Vavgpu= {}, VPriorpu={}",
                    fmt_g(self.ctrl_vars[i].f_present_drc_vpu, 5),
                    fmt_g(self.ctrl_vars[i].f_avgp_drc_vpu_prior, 5)
                );
                env.append_event(&der, &msg);
            }
        }

        // Trigger from volt-var mode (uses `QoutputVVDRCpu`).
        let cv = &self.ctrl_vars[i];
        let vv_trigger = (cv.f_present_vpu - cv.f_avgp_vpu_prior).abs()
            > self.voltage_change_tolerance
            || (cv.f_present_drc_vpu - cv.f_avgp_drc_vpu_prior).abs()
                > self.voltage_change_tolerance
            || (cv.qoutput_vvdrcpu.abs() - cv.q_desire_endpu.abs()).abs()
                > self.var_change_tolerance
            || control_iter == 1;
        if vv_trigger {
            self.ctrl_vars[i].f_vvdrc_operation = 0.0;
            self.ctrl_vars[i].f_pending_change = CHANGEDRCVVARLEVEL;
            env.push_change(self.ccd.time_delay, CHANGEDRCVVARLEVEL);
            if self.ccd.show_event_log {
                let der = env.der_full_name(r);
                let msg = format!(
                    "**Ready to change VV_DRC output due to volt-var trigger in VV_DRC mode**, Vavgpu= {}, VPriorpu={}",
                    fmt_g(self.ctrl_vars[i].f_present_vpu, 5),
                    fmt_g(self.ctrl_vars[i].f_avgp_vpu_prior, 5)
                );
                env.append_event(&der, &msg);
            }
        }
        Ok(())
    }

    /// The DRC rolling-average voltage in per-unit: `FDRCRollAvgWindow.AvgVal /
    /// (basekV*1000)` = `… / FVBase` (the mode-3 monitor `Set_Variable(6/15)` value).
    fn drc_avg_pu(&self, i: usize) -> f64 {
        let cv = &self.ctrl_vars[i];
        if cv.f_vbase == 0.0 {
            0.0
        } else {
            cv.f_drc_roll_avg_window.avg_val() / cv.f_vbase
        }
    }

    /// The VOLTWATT/VV_VW volt-watt-curve existence check (Pascal error 381): a
    /// PVSystem needs `voltwatt_curve`; a Storage needs `voltwatt_curve` *or*
    /// `voltwattCH_curve`.
    fn check_voltwatt_curve(&self, snap: DerSnap) -> Result<(), String> {
        let ok = if snap.is_pvsystem {
            self.voltwatt_curve.is_some()
        } else {
            self.voltwatt_curve.is_some() || self.voltwattch_curve.is_some()
        };
        if ok {
            Ok(())
        } else {
            Err(
                "XY Curve object representing voltwatt_curve does not exist or is not tied to InvControl."
                    .to_string(),
            )
        }
    }

    /// The Storage VOLTWATT / VV_VW dispatch is **deferred** (not a silent skip):
    /// the Storage-specific volt-watt machinery (`TStorageObj.DCkW`/`StorageState`/
    /// `FVWStateRequested` curve selection) is unverified by any gate, and a Storage
    /// state flip during InvControl dispatch would not propagate `system_y_changed`
    /// through the per-element env (the WP7.4 YPrim-rebuild bug class). PVSystem
    /// volt-watt is fully ported + gated. A Storage in VOLTWATT/VV_VW errors.
    fn guard_storage_vw(&self, snap: DerSnap) -> Result<(), String> {
        if snap.is_pvsystem {
            Ok(())
        } else {
            Err(format!(
                "InvControl.{}: Storage VOLTWATT/VV_VW dispatch is not yet ported (WP7.5; PVSystem volt-watt is ported)",
                self.ccd.cd.obj.name()
            ))
        }
    }

    /// Pascal `TInvControlObj.DoPendingAction` — dispatches per DER by the per-DER
    /// `FPendingChange` (the `Code` is ignored, matching Pascal). The header runs
    /// `Calc_QHeadRoom` for every DER; `Calc_PBase` + `kW_out_desiredpu` (which only
    /// VW/VV_VW consume) run inside those branches (PVSystem-only, so the storage
    /// `Calc_PBase` path is not reached). Ports VOLTVAR / VOLTWATT / VV_VW.
    pub(crate) fn do_pending_action(&mut self, env: &mut dyn InvDispatchEnv) {
        for k in 0..self.fleet.len() {
            // Calc_QHeadRoom (header; consumed by the var modes + VV_VW's VV part).
            self.calc_qheadroom(k);

            let pending = self.ctrl_vars[k].f_pending_change;
            if self.combi_mode == VV_VW {
                if pending == CHANGEWATTVARLEVEL {
                    self.do_pending_vv_vw(k, env);
                }
            } else if self.combi_mode == VV_DRC {
                if pending == CHANGEDRCVVARLEVEL {
                    self.do_pending_vv_drc(k, env);
                }
            } else if self.control_mode == VOLTVAR
                && self.combi_mode == NONE_COMBMODE
                && pending == CHANGEVARLEVEL
            {
                self.do_pending_voltvar(k, env);
            } else if self.control_mode == VOLTWATT
                && self.combi_mode == NONE_COMBMODE
                && pending == CHANGEWATTLEVEL
            {
                self.do_pending_voltwatt(k, env);
            } else if self.control_mode == DRC
                && self.combi_mode == NONE_COMBMODE
                && pending == CHANGEVARLEVEL
            {
                self.do_pending_drc(k, env);
            } else if self.control_mode == WATTPF
                && self.combi_mode == NONE_COMBMODE
                && pending == CHANGEVARLEVEL
            {
                self.do_pending_wattpf(k, env);
            } else if self.control_mode == WATTVAR
                && self.combi_mode == NONE_COMBMODE
                && pending == CHANGEVARLEVEL
            {
                self.do_pending_wattvar(k, env);
            } else if self.control_mode == AVR
                && self.combi_mode == NONE_COMBMODE
                && pending == CHANGEVARLEVEL
            {
                self.do_pending_avr(k, env);
            }

            // Pascal `DoPendingAction` l.1605-1606 (end of every DER's loop body):
            // force the next solve to re-run SetNominalDEROutput (so a kvar/kW request
            // pushed *without* an explicit `der_set_nominal` — the AVR iter-1/2 path —
            // is applied by the re-solve), then reset FPendingChange so a *second*
            // queued action for the same DER in one control iteration (the VV_VW
            // double-push) is a no-op (the DER is dispatched once per control
            // iteration, not once per queued action).
            env.set_loads_need_updating();
            self.ctrl_vars[k].f_pending_change = CHANGE_NONE;
        }
    }

    /// Pascal `CalcLPF(m, powertype, LPF_desiredpu)` — the first-order low-pass
    /// filter `out(t) = desired·(1−α) + prior·α`, `α = exp(−h/LPFTau)`, against the
    /// prior time step's option value (`FPrior*Optionpu`). `is_vars` selects
    /// `QDesireOptionpu` (`'VARS'`) vs `PLimitOptionpu` (`'WATTS'`).
    fn calc_lpf(&mut self, m: usize, is_vars: bool, lpf_desiredpu: f64, env: &dyn InvDispatchEnv) {
        // Pascal `alpha := exp(-1.0 * DynaVars.h / LPFTau)`.
        let alpha = (-env.dyna_h() / self.lpf_tau).exp();
        let cv = &mut self.ctrl_vars[m];
        if is_vars {
            cv.q_desire_optionpu =
                lpf_desiredpu * (1.0 - alpha) + cv.f_prior_q_desire_optionpu * alpha;
        } else {
            cv.p_limit_optionpu =
                lpf_desiredpu * (1.0 - alpha) + cv.f_prior_p_limit_optionpu * alpha;
        }
    }

    /// Pascal `CalcRF(m, powertype, RF_desiredpu)` — clamp the per-step change to
    /// `±FRiseFallLimit·h` around the prior step's option value, else take the
    /// request unchanged. `is_vars` selects `QDesireOptionpu` vs `PLimitOptionpu`.
    fn calc_rf(&mut self, m: usize, is_vars: bool, rf_desiredpu: f64, env: &dyn InvDispatchEnv) {
        let step = self.rise_fall_limit * env.dyna_h();
        let cv = &mut self.ctrl_vars[m];
        let prior = if is_vars {
            cv.f_prior_q_desire_optionpu
        } else {
            cv.f_prior_p_limit_optionpu
        };
        let out = if (rf_desiredpu - prior) > step {
            prior + step
        } else if (rf_desiredpu - prior) < -step {
            prior - step
        } else {
            rf_desiredpu
        };
        if is_vars {
            cv.q_desire_optionpu = out;
        } else {
            cv.p_limit_optionpu = out;
        }
    }

    /// The `DoPendingAction` reactive-power tail shared by VOLTVAR / DRC / VV_DRC:
    /// apply the LPF / Rise-Fall filter (if active) then the `Check_Qlimits`
    /// kVA/kvar clamp, leaving `QDesireEndpu`. With rate-of-change active the clamp
    /// runs on `QDesireOptionpu` and the sign follows it; INACTIVE clamps the raw
    /// `desired_pu` (Pascal l.1001-1021 / l.1248-1269 / l.1318-1339).
    fn apply_roc_qlimit(&mut self, k: usize, desired_pu: f64, env: &dyn InvDispatchEnv) {
        match self.rate_of_change_mode {
            ROC_LPF | ROC_RISEFALL => {
                if self.rate_of_change_mode == ROC_LPF {
                    self.calc_lpf(k, true, desired_pu, env);
                } else {
                    self.calc_rf(k, true, desired_pu, env);
                }
                let opt = self.ctrl_vars[k].q_desire_optionpu;
                self.check_qlimits(k, opt);
                let limited = self.ctrl_vars[k].q_desire_limitedpu;
                self.ctrl_vars[k].q_desire_endpu = limited.abs().min(opt.abs()) * pas_sign(opt);
            }
            _ => {
                self.check_qlimits(k, desired_pu);
                let limited = self.ctrl_vars[k].q_desire_limitedpu;
                self.ctrl_vars[k].q_desire_endpu =
                    desired_pu.abs().min(limited.abs()) * pas_sign(desired_pu);
            }
        }
    }

    /// The `DoPendingAction` active-power tail shared by VOLTWATT / VV_VW: apply the
    /// LPF / Rise-Fall filter (if active) then the `Check_Plimits` kVA/pctPmpp
    /// clamp, leaving `PLimitEndpu`. With rate-of-change active the clamp runs on
    /// `PLimitOptionpu` and `PLimitEndpu := Min(PLimitLimitedpu, PLimitOptionpu)`
    /// (a **plain** min, Pascal l.1390/1478); INACTIVE uses the abs/sign form
    /// (Pascal l.1404/1501).
    fn apply_roc_plimit(&mut self, k: usize, desired_pu: f64, env: &dyn InvDispatchEnv) {
        match self.rate_of_change_mode {
            ROC_LPF | ROC_RISEFALL => {
                if self.rate_of_change_mode == ROC_LPF {
                    self.calc_lpf(k, false, desired_pu, env);
                } else {
                    self.calc_rf(k, false, desired_pu, env);
                }
                let opt = self.ctrl_vars[k].p_limit_optionpu;
                self.check_plimits(k, opt);
                let limited = self.ctrl_vars[k].p_limit_limitedpu;
                self.ctrl_vars[k].p_limit_endpu = limited.min(opt);
            }
            _ => {
                self.check_plimits(k, desired_pu);
                let limited = self.ctrl_vars[k].p_limit_limitedpu;
                self.ctrl_vars[k].p_limit_endpu =
                    limited.abs().min(desired_pu.abs()) * pas_sign(desired_pu);
            }
        }
    }

    /// Pascal `DoPendingAction`'s `VOLTVAR` branch.
    fn do_pending_voltvar(&mut self, k: usize, env: &mut dyn InvDispatchEnv) {
        let r = self.fleet[k];
        env.der_set_modes(r, false, true, crate::elements::pc::pvsystem::VARMODE_KVAR);

        // Main process: the volt-var curve Q, then the LPF/RF rate-of-change filter
        // (if active) and the kVA/kvar clamp.
        self.calc_qvv_curve_desiredpu(k, env);
        let q_desire_vvpu = self.ctrl_vars[k].q_desire_vvpu;
        self.apply_roc_qlimit(k, q_desire_vvpu, env);

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

    /// Pascal `DoPendingAction`'s `VOLTWATT` branch (PVSystem; storage is guarded
    /// out at `Sample`, so `Calc_PBase`/`CalcPVWcurve_limitpu` take the PVSystem path).
    fn do_pending_voltwatt(&mut self, k: usize, env: &mut dyn InvDispatchEnv) {
        let r = self.fleet[k];
        env.der_set_vw_mode(r, true); // DERElem.VWmode := TRUE

        // Header values VW consumes (Pascal computes these for every DER; here only
        // the PVSystem VW path reaches them).
        self.calc_pbase(k);
        self.ctrl_vars[k].kw_out_desiredpu =
            self.ctrl_vars[k].kw_out_desired / self.ctrl_vars[k].p_base;

        // Main process: the volt-watt curve kW limit, then the LPF/RF filter (if
        // active) and the kVA/pctPmpp clamp.
        self.calc_pvw_curve_limitpu(k);
        let p_limit_vw_pu = self.ctrl_vars[k].p_limit_vw_pu;
        self.apply_roc_plimit(k, p_limit_vw_pu, env);

        // Convergence algorithm → PLimitVW (kW set-point).
        let control_iter = env.control_iteration();
        self.calc_voltwatt_watts(k, control_iter);

        // Push the new kW to the DER and recompute its P/Q.
        let p_limit_vw = self.ctrl_vars[k].p_limit_vw;
        env.der_set_kw_requested(r, p_limit_vw);
        env.der_set_nominal(r);

        let p_base = self.ctrl_vars[k].p_base;
        let cv = &mut self.ctrl_vars[k];
        cv.f_avgp_vpu_prior = cv.f_present_vpu;
        cv.p_old_vw_pu = p_limit_vw / p_base;

        // FVWOperation reset flag + event log (Pascal reads presentkW after nominal):
        // PVSystem guards on `abs(PLimitVW) > 0`.
        let present_kw = env.der_present_kw(r);
        if p_limit_vw.abs() > 0.0 && (present_kw - p_limit_vw).abs() / p_limit_vw > 0.0001 {
            self.ctrl_vars[k].f_vw_operation = 0.0;
        }
        if self.ccd.show_event_log {
            let der = env.der_full_name(r);
            let msg = format!(
                "**VOLTWATT mode set PVSystem kW output limit to **, kW= {}. Actual output is kW= {}.",
                fmt_g(p_limit_vw, 5),
                fmt_g(present_kw, 5)
            );
            env.append_event(&der, &msg);
        }
    }

    /// Pascal `DoPendingAction`'s `VV_VW` combi branch (PVSystem; storage guarded
    /// out at `Sample`). Runs the volt-watt P limit *and* the volt-var Q set-point.
    fn do_pending_vv_vw(&mut self, k: usize, env: &mut dyn InvDispatchEnv) {
        let r = self.fleet[k];
        // DERElem.VWmode := TRUE; Varmode := VARMODEKVAR; VVmode := TRUE;
        env.der_set_modes(r, true, true, crate::elements::pc::pvsystem::VARMODE_KVAR);

        self.calc_pbase(k);
        self.ctrl_vars[k].kw_out_desiredpu =
            self.ctrl_vars[k].kw_out_desired / self.ctrl_vars[k].p_base;

        // Main process: QDesireVVpu + PLimitVWpu, then per-function LPF/RF (if
        // active) and the Q and P clamps (Q first, then P — Pascal l.1463-1502).
        self.calc_pvw_curve_limitpu(k);
        self.calc_qvv_curve_desiredpu(k, env);

        let q_desire_vvpu = self.ctrl_vars[k].q_desire_vvpu;
        self.apply_roc_qlimit(k, q_desire_vvpu, env);

        let p_limit_vw_pu = self.ctrl_vars[k].p_limit_vw_pu;
        self.apply_roc_plimit(k, p_limit_vw_pu, env);

        // Convergence algorithms → PLimitVW + QDesiredVV.
        let control_iter = env.control_iteration();
        self.calc_voltwatt_watts(k, control_iter);
        self.calc_voltvar_vars(k);

        // Push the new kvar + kW to the DER and recompute its P/Q (one nominal).
        let q_desired_vv = self.ctrl_vars[k].q_desired_vv;
        let p_limit_vw = self.ctrl_vars[k].p_limit_vw;
        env.der_set_kvar_requested(r, q_desired_vv);
        env.der_set_kw_requested(r, p_limit_vw);
        env.der_set_nominal(r);

        let present_kvar = env.der_present_kvar(r);
        let p_base = self.ctrl_vars[k].p_base;
        let cv = &mut self.ctrl_vars[k];
        cv.qoutputpu = if q_desired_vv >= 0.0 {
            present_kvar / cv.q_headroom
        } else {
            present_kvar / cv.q_headroom_neg
        };
        cv.qoutput_vvpu = cv.qoutputpu;
        cv.f_avgp_vpu_prior = cv.f_present_vpu;
        cv.p_old_vw_pu = p_limit_vw / p_base;
        cv.q_old = present_kvar;
        cv.q_old_vv = present_kvar;

        if self.ccd.show_event_log {
            let der = env.der_full_name(r);
            let msg = format!(
                "**VV_VW mode requested DER output var level to **, kvar= {}. Actual output set to kvar= {}.",
                fmt_g(q_desired_vv, 5),
                fmt_g(present_kvar, 5)
            );
            env.append_event(&der, &msg);
        }

        // FVWOperation reset flag + the kW event log (PVSystem: no `abs(PLimitVW)>0`
        // guard here, unlike pure VOLTWATT — verbatim Pascal l.1547).
        let present_kw = env.der_present_kw(r);
        if (present_kw - p_limit_vw).abs() / p_limit_vw > 0.0001 {
            self.ctrl_vars[k].f_vw_operation = 0.0;
        }
        if self.ccd.show_event_log {
            let der = env.der_full_name(r);
            let msg = format!(
                "**VV_VW mode set PVSystem kW output limit to **, kW= {}. Actual output is kW= {}.",
                fmt_g(p_limit_vw, 5),
                fmt_g(present_kw, 5)
            );
            env.append_event(&der, &msg);
        }
    }

    /// Pascal `DoPendingAction`'s `DRC` branch — the dynamic-reactive-current kvar
    /// set-point (no curve; over `QOldDRC`).
    fn do_pending_drc(&mut self, k: usize, env: &mut dyn InvDispatchEnv) {
        let r = self.fleet[k];
        // VWmode := FALSE; Varmode := VARMODEKVAR; DRCmode := TRUE. (VVmode stays
        // FALSE for pure DRC — the per-step reset already cleared it.)
        env.der_set_modes(r, false, false, crate::elements::pc::pvsystem::VARMODE_KVAR);
        env.der_set_drc_mode(r, true);

        // Main process: the DRC dynamic-reactive-current Q, then the LPF/RF filter
        // (if active) and the kVA/kvar clamp.
        self.calc_qdrc_desiredpu(k, env);
        let q_desire_drcpu = self.ctrl_vars[k].q_desire_drcpu;
        self.apply_roc_qlimit(k, q_desire_drcpu, env);

        // Convergence algorithm → QDesiredDRC (kvar set-point).
        self.calc_drc_vars(k);

        // Push the new kvar to the DER and recompute its P/Q.
        let q_desired_drc = self.ctrl_vars[k].q_desired_drc;
        env.der_set_kvar_requested(r, q_desired_drc);
        env.der_set_nominal(r);

        let present_kvar = env.der_present_kvar(r);
        let cv = &mut self.ctrl_vars[k];
        cv.qoutputpu = if q_desired_drc >= 0.0 {
            present_kvar / cv.q_headroom
        } else {
            present_kvar / cv.q_headroom_neg
        };
        cv.qoutput_drcpu = cv.qoutputpu;
        cv.f_avgp_drc_vpu_prior = cv.f_present_drc_vpu;
        cv.q_old = present_kvar;
        cv.q_old_drc = present_kvar;

        if self.ccd.show_event_log {
            let der = env.der_full_name(r);
            let msg = format!(
                "DRC mode requested DER output var level to **, kvar = {}. Actual output set to kvar = {}.",
                fmt_g(q_desired_drc, 5),
                fmt_g(present_kvar, 5)
            );
            env.append_event(&der, &msg);
        }
    }

    /// Pascal `DoPendingAction`'s `WATTPF` branch — the watt-pf kvar set-point. The
    /// power factor comes from `wattpf_curve(panel pu)` (`CalcQWPcurve_desiredpu`);
    /// `CalcWATTPF_vars` then turns the clamped pu Q into kvar. A PVSystem also
    /// stores `pf_wp_nominal` (so its nominal applies the pf); a Storage takes the
    /// kvar set-point directly (both via `der_set_kvar_requested`). No deltaQ
    /// convergence — `QDesiredWP = QDesireEndpu·QHeadRoom`.
    fn do_pending_wattpf(&mut self, k: usize, env: &mut dyn InvDispatchEnv) {
        let r = self.fleet[k];
        // Pascal: VWmode := FALSE; Varmode := VARMODEKVAR; WPmode := TRUE (for both
        // DER types — der_set_var_mode so a Storage's kvarRequested is applied).
        env.der_set_vw_mode(r, false);
        env.der_set_var_mode(r, crate::elements::pc::pvsystem::VARMODE_KVAR);
        env.der_set_wp_mode(r, true);

        // Main process.
        self.calc_qwp_curve_desiredpu(k);
        let q_desire_wppu = self.ctrl_vars[k].q_desire_wppu;
        self.check_qlimits(k, q_desire_wppu);
        let limited = self.ctrl_vars[k].q_desire_limitedpu;
        self.ctrl_vars[k].q_desire_endpu =
            q_desire_wppu.abs().min(limited.abs()) * pas_sign(q_desire_wppu);
        self.calc_wattpf_vars(k);

        // Push pf_wp_nominal (PVSystem; no-op for Storage) + the kvar set-point.
        let q_desired_wp = self.ctrl_vars[k].q_desired_wp;
        let pf_wp_nominal = self.pf_wp_nominal;
        env.der_set_pf_wp_nominal(r, pf_wp_nominal);
        env.der_set_kvar_requested(r, q_desired_wp);
        env.der_set_nominal(r);

        let present_kvar = env.der_present_kvar(r);
        let cv = &mut self.ctrl_vars[k];
        cv.qoutputpu = if q_desired_wp >= 0.0 {
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
                "WATTPF mode requested DER output var level to **, kvar = {}. Actual output set to kvar= {}.",
                fmt_g(q_desired_wp, 5),
                fmt_g(present_kvar, 5)
            );
            env.append_event(&der, &msg);
        }
    }

    /// Pascal `DoPendingAction`'s `WATTVAR` branch — the watt-var kvar set-point off
    /// `wattvar_curve(panel pu)` (`CalcQWVcurve_desiredpu`), clamped by
    /// `Check_Qlimits_WV` (no watt-priority arm), then `Calc_PQ_WV` keeps the final
    /// (P, Q) inside the kVA circle (solving the watt-var-line ∩ kVA-circle
    /// quadratic when needed). A PVSystem also sets its kW to
    /// `PLimitEndpu·min(kVArating, DCkWrated)`.
    fn do_pending_wattvar(&mut self, k: usize, env: &mut dyn InvDispatchEnv) {
        let r = self.fleet[k];
        let is_pv = env.der_is_pvsystem(r);
        // Pascal: VWmode := FALSE; Varmode := VARMODEKVAR; WVmode := TRUE (for both
        // DER types — der_set_var_mode so a Storage's kvarRequested is applied).
        env.der_set_vw_mode(r, false);
        env.der_set_var_mode(r, crate::elements::pc::pvsystem::VARMODE_KVAR);
        env.der_set_wv_mode(r, true);

        // Main process.
        self.calc_qwv_curve_desiredpu(k);
        let q_desire_wvpu = self.ctrl_vars[k].q_desire_wvpu;
        self.check_qlimits_wv(k, q_desire_wvpu);
        let limited = self.ctrl_vars[k].q_desire_limitedpu;
        self.ctrl_vars[k].q_desire_endpu =
            q_desire_wvpu.abs().min(limited.abs()) * pas_sign(q_desire_wvpu);
        // Keeps the final P and Q within the watt-var curve / kVA circle; sets
        // QDesiredWV (via CalcWATTVAR_vars) + PLimitEndpu.
        self.calc_pq_wv(k);

        // Push kvar; a PVSystem also pushes kW = PLimitEndpu·min(kVArating, DCkWrated).
        let q_desired_wv = self.ctrl_vars[k].q_desired_wv;
        env.der_set_kvar_requested(r, q_desired_wv);
        if is_pv {
            let cv = &self.ctrl_vars[k];
            let p = cv.p_limit_endpu * cv.f_kva_rating.min(cv.f_dckw_rated);
            env.der_set_kw_requested(r, p);
        }
        env.der_set_nominal(r);

        let present_kvar = env.der_present_kvar(r);
        let cv = &mut self.ctrl_vars[k];
        cv.qoutputpu = if q_desired_wv >= 0.0 {
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
                "WATTVAR mode requested DER output var level to **, kvar = {}. Actual output set to kvar= {}.",
                fmt_g(q_desired_wv, 5),
                fmt_g(present_kvar, 5)
            );
            env.append_event(&der, &msg);
        }
    }

    /// Pascal `DoPendingAction`'s `AVR` branch — the 3-stage active-voltage-regulation
    /// DQDV regulator (Pascal l.1055-1127). Control iteration 1 seeds the baseline
    /// voltages and pushes `QHeadRoom/2`; iteration 2 estimates the dQ/dV sensitivity
    /// `DQDV` from the resulting voltage change; iteration 3+ runs the regulator law
    /// (`CalcQAVR_desiredpu` → `Check_Qlimits` → `CalcAVR_vars`) to a kvar set-point.
    /// No `SetNominalDEROutput`/event log on iterations 1-2 (no kvar read-back yet).
    fn do_pending_avr(&mut self, k: usize, env: &mut dyn InvDispatchEnv) {
        let r = self.fleet[k];
        // Pascal l.1058-1060: VWmode := FALSE; Varmode := VARMODEKVAR; AVRmode := TRUE
        // (for both DER types — `der_set_var_mode` so a Storage's `kvarRequested` is
        // applied by `set_nominal`, not discarded by its `VARMODE_PF` default).
        env.der_set_vw_mode(r, false);
        env.der_set_var_mode(r, crate::elements::pc::pvsystem::VARMODE_KVAR);
        env.der_set_avr_mode(r, true);

        let control_iter = env.control_iteration();
        if control_iter == 1 {
            // Seed the AVR baselines and push the initial half-headroom kvar (Pascal
            // l.1063-1073). No SetNominalDEROutput — the next solve applies it.
            let qheadroom = {
                let cv = &mut self.ctrl_vars[k];
                cv.f_avgp_vpu_prior = cv.f_present_vpu;
                cv.f_avgp_avr_vpu_prior = cv.f_present_vpu;
                cv.q_headroom
            };
            env.der_set_kvar_requested(r, qheadroom / 2.0);
        } else if control_iter == 2 {
            // Estimate dQ/dV from the voltage change the half-headroom kvar produced
            // (Pascal l.1075-1082). PVSystem reads the achieved `Presentkvar`; a
            // Storage reads its `kvarRequested` (Pascal l.1079-1081).
            let kvar_for_dqdv = if env.der_is_pvsystem(r) {
                env.der_present_kvar(r)
            } else {
                env.der_requested_kvar(r)
            };
            let cv = &mut self.ctrl_vars[k];
            cv.dqdv =
                (kvar_for_dqdv / cv.q_headroom / (cv.f_present_vpu - cv.f_avgp_vpu_prior)).abs();
        } else {
            // The regulator (Pascal l.1084-1127).
            self.calc_qavr_desiredpu(k, env);
            let q_desire_avrpu = self.ctrl_vars[k].q_desire_avrpu;
            self.check_qlimits(k, q_desire_avrpu);
            let limited = self.ctrl_vars[k].q_desire_limitedpu;
            self.ctrl_vars[k].q_desire_endpu =
                q_desire_avrpu.abs().min(limited.abs()) * pas_sign(q_desire_avrpu);

            // The setpoint used in the trigger: the present voltage if the kvar limit
            // backed the request off, else the configured setpoint (Pascal l.1092-1095).
            {
                let v_setpoint = self.v_setpoint;
                let cv = &mut self.ctrl_vars[k];
                cv.f_v_setpoint_limited =
                    if (cv.q_desire_endpu - cv.q_desire_limitedpu).abs() < 0.05 {
                        cv.f_present_vpu
                    } else {
                        v_setpoint
                    };
            }

            // Convergence algorithm → QDesiredAVR (kvar set-point).
            self.calc_avr_vars(k);

            // Push the new kvar to the DER and recompute its P/Q.
            let q_desired_avr = self.ctrl_vars[k].q_desired_avr;
            env.der_set_kvar_requested(r, q_desired_avr);
            env.der_set_nominal(r);

            let present_kvar = env.der_present_kvar(r);
            let cv = &mut self.ctrl_vars[k];
            cv.qoutputpu = if q_desired_avr >= 0.0 {
                present_kvar / cv.q_headroom
            } else {
                present_kvar / cv.q_headroom_neg
            };
            cv.qoutput_avrpu = cv.qoutputpu;
            cv.f_avgp_vpu_prior = cv.f_present_vpu;
            cv.q_old = present_kvar;
            cv.q_old_avr = present_kvar;

            if self.ccd.show_event_log {
                let der = env.der_full_name(r);
                // Pascal's event-log string here literally says "VOLTVAR mode …"
                // (a copy-paste in InvControl.pas l.1124-1126); reproduced verbatim.
                let msg = format!(
                    "VOLTVAR mode requested DER output var level to **, kvar = {}. Actual output set to kvar= {}.",
                    fmt_g(q_desired_avr, 5),
                    fmt_g(present_kvar, 5)
                );
                env.append_event(&der, &msg);
            }
        }
    }

    /// Pascal `DoPendingAction`'s `VV_DRC` combi branch — the volt-var curve Q
    /// *summed* with the DRC Q, clamped jointly, then converged over `QOldVVDRC`.
    fn do_pending_vv_drc(&mut self, k: usize, env: &mut dyn InvDispatchEnv) {
        let r = self.fleet[k];
        // VWmode := FALSE; Varmode := VARMODEKVAR; VVmode := TRUE; DRCmode := TRUE.
        env.der_set_modes(r, false, true, crate::elements::pc::pvsystem::VARMODE_KVAR);
        env.der_set_drc_mode(r, true);

        // Main process: QDesireVVpu + QDesireDRCpu, then the LPF/RF filter (on the
        // sum, if active) and the combined Q clamp.
        self.calc_qvv_curve_desiredpu(k, env);
        self.calc_qdrc_desiredpu(k, env);
        let q_sum = self.ctrl_vars[k].q_desire_vvpu + self.ctrl_vars[k].q_desire_drcpu;
        self.apply_roc_qlimit(k, q_sum, env);

        // Convergence algorithm → QDesiredVVDRC (kvar set-point).
        self.calc_vvdrc_vars(k);

        // Push the new kvar to the DER and recompute its P/Q.
        let q_desired_vvdrc = self.ctrl_vars[k].q_desired_vvdrc;
        env.der_set_kvar_requested(r, q_desired_vvdrc);
        env.der_set_nominal(r);

        let present_kvar = env.der_present_kvar(r);
        let cv = &mut self.ctrl_vars[k];
        cv.qoutputpu = if q_desired_vvdrc >= 0.0 {
            present_kvar / cv.q_headroom
        } else {
            present_kvar / cv.q_headroom_neg
        };
        cv.qoutput_vvdrcpu = cv.qoutputpu;
        cv.f_avgp_vpu_prior = cv.f_present_vpu;
        cv.f_avgp_drc_vpu_prior = cv.f_present_drc_vpu;
        cv.q_old = present_kvar;
        cv.q_old_vvdrc = present_kvar;

        if self.ccd.show_event_log {
            let der = env.der_full_name(r);
            let msg = format!(
                "**VV_DRC mode requested DER output var level to **, kvar = {}. Actual output set to kvar = {}.",
                fmt_g(q_desired_vvdrc, 5),
                fmt_g(present_kvar, 5)
            );
            env.append_event(&der, &msg);
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
            // Pascal `UpdateInvControl`'s per-step reset (l.2555-2575, "Reset the
            // operation flags for the new time step"): clear the DER inverter-control
            // modes and the per-DER latch/operation flags before seeding the rolling
            // average. **`f_flag_vw_operates` is load-bearing** — without this reset
            // it latches across time steps, forcing the damped "requesting" volt-watt
            // branch (the slow `Change_deltaP` ramp) on every subsequent step instead
            // of taking the curve point directly, which diverges the multi-step
            // control trajectory from the oracle. `FdeltaPFactor` resets to
            // DELTAPDEFAULT each step, but `FdeltaQFactor` deliberately does NOT
            // (Pascal l.2574 leaves it commented). `DQDV` (Pascal l.2562) is reset so
            // the AVR sensitivity is re-estimated each step. `FPrior*Optionpu` latch
            // this step's `QDesireOptionpu`/`PLimitOptionpu` as the LPF/RF reference
            // for the next step (Pascal l.2551-2552).
            self.ctrl_vars[j].f_prior_p_limit_optionpu = self.ctrl_vars[j].p_limit_optionpu;
            self.ctrl_vars[j].f_prior_q_desire_optionpu = self.ctrl_vars[j].q_desire_optionpu;
            let r = self.fleet[j];
            env.der_set_vw_mode(r, false);
            env.der_set_vv_mode(r, false);
            env.der_set_drc_mode(r, false);
            self.ctrl_vars[j].f_flag_vw_operates = false;
            self.ctrl_vars[j].dqdv = 0.0;
            self.ctrl_vars[j].f_vv_operation = 0.0;
            self.ctrl_vars[j].f_vw_operation = 0.0;
            self.ctrl_vars[j].f_drc_operation = 0.0;
            self.ctrl_vars[j].f_vvdrc_operation = 0.0;
            self.ctrl_vars[j].f_wp_operation = 0.0;
            self.ctrl_vars[j].f_wv_operation = 0.0;
            self.ctrl_vars[j].f_avr_operation = 0.0;
            self.ctrl_vars[j].f_delta_p_factor = DELTAPDEFAULT;

            // Pascal `BasekV := CtrlVars[i].FVBase / 1000.0` — `i` is the InvControl's
            // element-list index (1 for the single-InvControl gated case), so this is
            // CtrlVars[1]'s vbase = `ctrl_vars[0]` (the `//TODO: check (i, j)`
            // upstream quirk: it does NOT use the per-DER `j`; identical for a
            // homogeneous fleet — the only gated shape).
            let basekv = self.ctrl_vars[0].f_vbase / 1000.0;
            self.ctrl_vars[j].prior_roll_avg_window = self.ctrl_vars[j].f_roll_avg_window.avg_val();
            self.ctrl_vars[j].prior_drc_roll_avg_window =
                self.ctrl_vars[j].f_drc_roll_avg_window.avg_val();

            let solnvoltage = self.get_mon_voltage(j, basekv, env);

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
        // Error band (Pascal: VOLTVAR/WATTPF/WATTVAR/AVR/VV_DRC/VV_VW = 0.005,
        // DRC = 0.0005; VOLTVAR/WATTPF/AVR/VV_VW/DRC/VV_DRC reach `Check_Qlimits`.
        // WATTVAR uses the separate `check_qlimits_wv`, so its arm here is unreachable
        // and omitted.)
        let error = if self.control_mode == VOLTVAR
            || self.control_mode == WATTPF
            || self.control_mode == AVR
            || self.combi_mode == VV_VW
            || self.combi_mode == VV_DRC
        {
            0.005
        } else if self.control_mode == DRC {
            0.0005
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

        if self.control_mode == VOLTVAR || self.combi_mode == VV_VW {
            cv.f_vv_operation = f_operation;
        }
        if self.control_mode == WATTPF {
            cv.f_wp_operation = f_operation;
        }
        if self.control_mode == DRC {
            cv.f_drc_operation = f_operation;
        }
        if self.control_mode == AVR {
            cv.f_avr_operation = f_operation;
        }
        if self.combi_mode == VV_DRC {
            cv.f_vvdrc_operation = f_operation;
        }
    }

    /// Pascal `Change_deltaQ_factor(j)` — the adaptive convergence-damping band
    /// logic, shared by the var modes (VOLTVAR / DRC / VV_DRC). Touches only
    /// `CtrlVars[j]`. Only invoked on the `FLAGDELTAQ` (unset) sentinel path; a set
    /// `deltaQ_factor` is used directly (the pinned oracle runs CompatFlags=0).
    fn change_deltaq_factor(&mut self, j: usize) {
        let cv = &mut self.ctrl_vars[j];
        let delta_v = (cv.f_present_vpu - cv.f_avgp_vpu_prior).abs();
        if cv.delta_v_old >= 0.0 {
            if delta_v.abs() > 0.8 * cv.delta_v_old && cv.f_delta_q_factor > 0.2 {
                cv.f_delta_q_factor -= 0.1;
            } else if delta_v.abs() > 0.6 * cv.delta_v_old && cv.f_delta_q_factor > 0.2 {
                cv.f_delta_q_factor -= 0.05;
            } else if delta_v.abs() < 0.2 * cv.delta_v_old && cv.f_delta_q_factor < 0.9 {
                cv.f_delta_q_factor += 0.1;
            } else if delta_v.abs() < 0.4 * cv.delta_v_old && cv.f_delta_q_factor < 0.9 {
                cv.f_delta_q_factor += 0.05;
            }
        }
        cv.delta_v_old = (cv.f_present_vpu - cv.f_avgp_vpu_prior).abs();
    }

    /// Pascal `CalcVoltVar_vars(j)` — the convergence step → `QDesiredVV`. Linear
    /// `ControlModel` only; Exponential (the `TPICtrl` PI controller) is NOT_PORTED.
    fn calc_voltvar_vars(&mut self, j: usize) {
        if self.ctrl_vars[j].flag_change_curve {
            // Stay at the present var output level.
            self.ctrl_vars[j].q_desired_vv = self.ctrl_vars[j].f_present_kvar;
            return;
        }
        let mut delta_q = {
            let cv = &self.ctrl_vars[j];
            if cv.q_desire_endpu >= 0.0 {
                cv.q_desire_endpu * cv.q_headroom
            } else {
                cv.q_desire_endpu * cv.q_headroom_neg
            }
        };
        if self.ctrl_model == MODEL_LINEAR {
            delta_q -= self.ctrl_vars[j].q_old_vv;
            self.update_deltaq_factor(j);
            let cv = &mut self.ctrl_vars[j];
            cv.q_desired_vv = cv.q_old_vv + delta_q * cv.f_delta_q_factor;
        } else {
            // Unreachable: the Exponential ControlModel (TPICtrl PI controller)
            // is rejected in `Sample` (WP7.7); kept for structural parity.
            self.ctrl_vars[j].q_desired_vv = self.ctrl_vars[j].q_old_vv;
        }
    }

    /// The shared `FdeltaQFactor` update used by `CalcVoltVar_vars`/`CalcDRC_vars`/
    /// `CalcVVDRC_vars` (Linear model): the adaptive `Change_deltaQ_factor` on the
    /// `FLAGDELTAQ` sentinel, else the set factor used directly (CompatFlags=0).
    fn update_deltaq_factor(&mut self, j: usize) {
        if self.delta_q_factor == FLAGDELTAQ {
            self.change_deltaq_factor(j);
        } else {
            self.ctrl_vars[j].f_delta_q_factor = self.delta_q_factor;
        }
    }

    /// Pascal `CalcDRC_vars(j)` — the DRC convergence step → `QDesiredDRC` (like
    /// `CalcVoltVar_vars` but no curve-hysteresis branch, and over `QOldDRC`).
    fn calc_drc_vars(&mut self, j: usize) {
        let mut delta_q = {
            let cv = &self.ctrl_vars[j];
            if cv.q_desire_endpu >= 0.0 {
                cv.q_desire_endpu * cv.q_headroom
            } else {
                cv.q_desire_endpu * cv.q_headroom_neg
            }
        };
        if self.ctrl_model == MODEL_LINEAR {
            delta_q -= self.ctrl_vars[j].q_old_drc;
            self.update_deltaq_factor(j);
            let cv = &mut self.ctrl_vars[j];
            cv.q_desired_drc = cv.q_old_drc + delta_q * cv.f_delta_q_factor;
        } else {
            // Unreachable: Exponential PICtrl rejected at Sample (WP7.7).
            self.ctrl_vars[j].q_desired_drc = self.ctrl_vars[j].q_old_drc;
        }
    }

    /// Pascal `CalcVVDRC_vars(j)` — the VV_DRC combi convergence step →
    /// `QDesiredVVDRC` (identical to `CalcDRC_vars` but over `QOldVVDRC`).
    fn calc_vvdrc_vars(&mut self, j: usize) {
        let mut delta_q = {
            let cv = &self.ctrl_vars[j];
            if cv.q_desire_endpu >= 0.0 {
                cv.q_desire_endpu * cv.q_headroom
            } else {
                cv.q_desire_endpu * cv.q_headroom_neg
            }
        };
        if self.ctrl_model == MODEL_LINEAR {
            delta_q -= self.ctrl_vars[j].q_old_vvdrc;
            self.update_deltaq_factor(j);
            let cv = &mut self.ctrl_vars[j];
            cv.q_desired_vvdrc = cv.q_old_vvdrc + delta_q * cv.f_delta_q_factor;
        } else {
            // Unreachable: Exponential PICtrl rejected at Sample (WP7.7).
            self.ctrl_vars[j].q_desired_vvdrc = self.ctrl_vars[j].q_old_vvdrc;
        }
    }

    /// Pascal `CalcQDRC_desiredpu(j)` — the DRC dynamic-reactive-current law: the
    /// per-unit deltaV from the DRC rolling-average voltage drives a slope
    /// (`ArGraLowV`/`ArGraHiV`) when outside the [`DbVMin`, `DbVMax`] deadband. A
    /// no-op while the DRC window is empty (`AvgVal = 0` → deltaV forced 0), which is
    /// the whole pure-snapshot case (the window is fed only in time-series cleanup).
    fn calc_qdrc_desiredpu(&mut self, j: usize, env: &mut dyn InvDispatchEnv) {
        let dbv_min = self.dbv_min;
        let dbv_max = self.dbv_max;
        let ar_gra_low_v = self.ar_gra_low_v;
        let ar_gra_hi_v = self.ar_gra_hi_v;
        let dyna_t = env.dyna_t();

        let cv = &mut self.ctrl_vars[j];
        cv.q_desire_drcpu = 0.0;
        let basekv = cv.f_vbase / 1000.0; // line-to-ground

        // deltaV from the DRC rolling-average voltage (pu). When the window is empty
        // (AvgVal = 0) deltaV is forced to 0 (Pascal's `= 0.0` guard).
        let avg_pu = cv.f_drc_roll_avg_window.avg_val() / (basekv * 1000.0);
        let delta_v_dyn_reac = if avg_pu == 0.0 {
            0.0
        } else {
            cv.f_present_drc_vpu - avg_pu
        };

        if delta_v_dyn_reac != 0.0 && cv.f_present_drc_vpu < dbv_min {
            cv.q_desire_drcpu = -delta_v_dyn_reac * ar_gra_low_v;
        } else if delta_v_dyn_reac != 0.0 && cv.f_present_drc_vpu > dbv_max {
            cv.q_desire_drcpu = -delta_v_dyn_reac * ar_gra_hi_v;
        } else if delta_v_dyn_reac == 0.0 {
            cv.q_desire_drcpu = 0.0;
        }

        if dyna_t == 1.0 {
            cv.q_desire_drcpu = 0.0;
        }
    }

    /// Pascal `CalcQAVR_desiredpu(j)` — the AVR regulator law (control iteration 3+).
    /// The desired Q (pu) drives the monitored voltage toward `Vsetpoint`:
    /// `DQ = FdeltaQFactor·DQDV·(Vsetpoint − v)`, clamped by `DQmax` and added to the
    /// present per-unit Q. On control iteration 3 the baseline `v` is the seeded
    /// `FAvgpAVRVpuPrior` and the present Q / `QOldAVR` are zeroed (the first real
    /// regulator step). The damping-band block before `FdeltaQFactor := 0.2` is
    /// reproduced verbatim though Pascal's unconditional `:= 0.2` (l.3184) overwrites
    /// it (so `FdeltaQFactor` is always 0.2 where it is used at l.3191).
    fn calc_qavr_desiredpu(&mut self, j: usize, env: &mut dyn InvDispatchEnv) {
        let control_iter = env.control_iteration();
        let v_setpoint = self.v_setpoint;
        let cv = &mut self.ctrl_vars[j];

        let dqmax = 0.1 * cv.f_kvar_limit / cv.q_headroom_neg;
        cv.q_desire_avrpu = 0.0;

        let mut q_present_pu = if cv.f_present_kvar >= 0.0 {
            cv.f_present_kvar / cv.q_headroom
        } else {
            cv.f_present_kvar / cv.q_headroom_neg
        };

        let v = if control_iter == 3 {
            q_present_pu = 0.0;
            cv.q_old_avr = 0.0;
            cv.f_avgp_avr_vpu_prior
        } else {
            cv.f_present_vpu
        };

        // The adaptive band (Pascal l.3170-3182). Dead: l.3184 unconditionally
        // overwrites `FdeltaQFactor := 0.2` immediately after — reproduced verbatim.
        let delta_v = (v_setpoint - cv.f_avgp_vpu_prior).abs();
        if delta_v.abs() < 0.005 && cv.f_delta_q_factor > 0.2 {
            cv.f_delta_q_factor += 0.1;
        } else if delta_v.abs() < 0.02 && cv.f_delta_q_factor > 0.2 {
            cv.f_delta_q_factor += 0.05;
        } else if delta_v.abs() > 0.02 && cv.f_delta_q_factor < 0.9 {
            cv.f_delta_q_factor -= 0.05;
        } else if delta_v.abs() < 0.05 && cv.f_delta_q_factor < 0.9 {
            cv.f_delta_q_factor -= 0.1;
        }
        cv.f_delta_q_factor = 0.2; // Pascal l.3184 — unconditional override.

        cv.delta_v_old = (cv.f_present_vpu - cv.f_avgp_vpu_prior).abs();

        let mut dq = if cv.f_present_vpu - cv.f_avgp_vpu_prior == 0.0 {
            0.0
        } else {
            cv.f_delta_q_factor * cv.dqdv * (v_setpoint - v)
        };
        if dq.abs() > dqmax {
            dq = if dq < 0.0 { -dqmax } else { dqmax };
        }
        cv.q_desire_avrpu = q_present_pu + dq;
    }

    /// Pascal `CalcAVR_vars(j)` — the AVR convergence step → `QDesiredAVR`. Linear
    /// `ControlModel` only (the hard-coded 0.2 step, **not** `FdeltaQFactor`);
    /// Exponential (the `TPICtrl` PI controller) is rejected at `Sample` (WP7.7).
    fn calc_avr_vars(&mut self, j: usize) {
        let mut delta_q = {
            let cv = &self.ctrl_vars[j];
            if cv.q_desire_endpu >= 0.0 {
                cv.q_desire_endpu * cv.q_headroom
            } else {
                cv.q_desire_endpu * cv.q_headroom_neg
            }
        };
        if self.ctrl_model == MODEL_LINEAR {
            delta_q -= self.ctrl_vars[j].q_old_avr;
            // Pascal updates FdeltaQFactor here (Change_deltaQ_factor / set factor),
            // but QDesiredAVR uses the literal 0.2, not FdeltaQFactor — so this only
            // refreshes the (unused-by-AVR) factor + DeltaV_old. Kept for fidelity.
            self.update_deltaq_factor(j);
            let cv = &mut self.ctrl_vars[j];
            cv.q_desired_avr = cv.q_old_avr + 0.2 * delta_q;
        } else {
            // Unreachable: Exponential PICtrl rejected at Sample (WP7.7).
            self.ctrl_vars[j].q_desired_avr = self.ctrl_vars[j].q_old_avr;
        }
    }

    /// Pascal `CalcQWPcurve_desiredpu(j)` — the watt-pf desired Q (pu). The power
    /// factor `pf_wp_nominal` (object-level, overwritten per DER) is read off
    /// `wattpf_curve` at the panel pu; the desired kvar follows from
    /// `p·tan(acos(pf))·sign(pf)`. `p` is the panel power when neither P- nor
    /// PF-priority is set, else `kW_out_desired`. Pascal's local `QDesiredWP` here
    /// shadows the `CtrlVars` field (only `QDesireWPpu` + `pf_wp_nominal` are
    /// recorded; `CtrlVars.QDesiredWP` is set later by `CalcWATTPF_vars`).
    fn calc_qwp_curve_desiredpu(&mut self, j: usize) {
        let (panel_pu, p, q_headroom, q_headroom_neg) = {
            let cv = &self.ctrl_vars[j];
            let panel_pu = cv.f_dckw * cv.f_eff_factor * cv.f_pct_dckw_rated / cv.f_dckw_rated;
            // Pascal: `if (FPPriority = FALSE) and (pf_priority = FALSE)`.
            let p = if !cv.f_p_priority && !cv.f_pf_priority {
                cv.f_dckw * cv.f_eff_factor * cv.f_pct_dckw_rated
            } else {
                cv.kw_out_desired
            };
            (panel_pu, p, cv.q_headroom, cv.q_headroom_neg)
        };
        let pf_wp_nominal = self
            .wattpf_curve
            .as_mut()
            .expect("wattpf_curve checked at Sample")
            .get_y_value(panel_pu);
        self.pf_wp_nominal = pf_wp_nominal;
        let q_desired_wp =
            p * (1.0 / (pf_wp_nominal * pf_wp_nominal) - 1.0).sqrt() * pas_sign(pf_wp_nominal);
        let cv = &mut self.ctrl_vars[j];
        cv.q_desire_wppu = if q_desired_wp >= 0.0 {
            q_desired_wp / q_headroom
        } else {
            q_desired_wp / q_headroom_neg
        };
    }

    /// Pascal `CalcWATTPF_vars(j)` — turn the clamped pu Q into the kvar set-point
    /// (`QDesiredWP = QDesireEndpu · QHeadRoom`/`QHeadRoomNeg`). No convergence step.
    fn calc_wattpf_vars(&mut self, j: usize) {
        let cv = &mut self.ctrl_vars[j];
        cv.q_desired_wp = if cv.q_desire_endpu >= 0.0 {
            cv.q_desire_endpu * cv.q_headroom
        } else {
            cv.q_desire_endpu * cv.q_headroom_neg
        };
    }

    /// Pascal `CalcQWVcurve_desiredpu(j)` — the watt-var desired Q (pu) off
    /// `wattvar_curve` at the panel pu (`Pbase = min(kVArating, DCkWrated)`).
    fn calc_qwv_curve_desiredpu(&mut self, j: usize) {
        let x = {
            let cv = &self.ctrl_vars[j];
            let pbase = cv.f_kva_rating.min(cv.f_dckw_rated);
            cv.f_dckw * cv.f_eff_factor * cv.f_pct_dckw_rated / pbase
        };
        let value = self
            .wattvar_curve
            .as_mut()
            .expect("wattvar_curve checked at Sample")
            .get_y_value(x);
        self.ctrl_vars[j].q_desire_wvpu = value;
    }

    /// Pascal `Check_Qlimits_WV(j, Q)` — the WATTVAR kvar-limit clamp. Like
    /// `Check_Qlimits` but with **no** watt-priority `Q_Ppriority` arm; records
    /// `FWVOperation`.
    fn check_qlimits_wv(&mut self, j: usize, q: f64) {
        let cv = &mut self.ctrl_vars[j];
        let error = if self.control_mode == WATTVAR {
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

        if self.control_mode == WATTVAR {
            cv.f_wv_operation = f_operation;
        }
    }

    /// Pascal `Calc_PQ_WV(j)` — keep the final P and Q on the watt-var curve and
    /// inside the kVA circle. If the kvar limit was hit (`|FWVOperation| = 0.2`),
    /// `PLimitEndpu` is the watt for that var (`GetXValue(QDesireEndpu)`), else full
    /// power (1.0). If the resulting (P, Q) exceeds `kVArating`, solve the
    /// watt-var-line ∩ kVA-circle quadratic for `PLimitEndpu` + `QDesireEndpu`.
    /// `CalcWATTVAR_vars` is run after each adjustment (matching Pascal's two calls).
    fn calc_pq_wv(&mut self, j: usize) {
        // Pbase + Qbase are read at the TOP (Pascal l.3348-3359), i.e. `Qbase` uses
        // the **prior** `QDesiredWV` (the value the previous control iteration left,
        // 0 on the first) — *before* the first `CalcWATTVAR_vars` overwrites it. This
        // only differs from reading it post-update when QHeadRoom ≠ QHeadRoomNeg
        // (asymmetric kvar limits) and the prior/new `QDesiredWV` signs differ.
        let (pbase, qbase) = {
            let cv = &self.ctrl_vars[j];
            let pbase = cv.f_kva_rating.min(cv.f_dckw_rated);
            let qbase = if cv.q_desired_wv >= 0.0 {
                cv.q_headroom
            } else {
                cv.q_headroom_neg
            };
            (pbase, qbase)
        };

        // Part 1: PLimitEndpu from the kvar-limit flag, then CalcWATTVAR_vars.
        let (wv_operation, q_desire_endpu) = {
            let cv = &self.ctrl_vars[j];
            (cv.f_wv_operation, cv.q_desire_endpu)
        };
        let p_limit_endpu = if wv_operation.abs() == 0.2 {
            self.wattvar_curve
                .as_ref()
                .expect("wattvar_curve checked at Sample")
                .get_x_value(q_desire_endpu)
        } else {
            1.0
        };
        self.ctrl_vars[j].p_limit_endpu = p_limit_endpu;
        self.calc_wattvar_vars(j);

        // Part 2: if (P, Q) leaves the kVA circle, intersect the watt-var line with
        // the kVA circle (Pascal's quadratic) and re-derive QDesireEndpu. `q_desired_wv`
        // here is the *new* (post-CalcWATTVAR_vars) value, matching Pascal's `s` check.
        let (panel, kva_rating, p_limit_endpu, q_desired_wv) = {
            let cv = &self.ctrl_vars[j];
            let panel = cv.f_dckw * cv.f_eff_factor * cv.f_pct_dckw_rated;
            (panel, cv.f_kva_rating, cv.p_limit_endpu, cv.q_desired_wv)
        };
        let s = ((panel * p_limit_endpu).powi(2) + q_desired_wv.powi(2)).sqrt();
        if s > kva_rating {
            let curve = self
                .wattvar_curve
                .as_mut()
                .expect("wattvar_curve checked at Sample");
            let coeff = curve.get_coefficients(panel / pbase);
            let a_line = coeff.0 * qbase / pbase;
            let b_line = coeff.1 * qbase;
            let aa = 1.0 + a_line * a_line;
            let bb = 2.0 * a_line * b_line;
            let cc = b_line * b_line - kva_rating * kva_rating;
            let new_p = (-bb + (bb * bb - 4.0 * aa * cc).sqrt()) / (2.0 * aa * pbase);
            let new_q = curve.get_y_value(new_p);
            self.ctrl_vars[j].p_limit_endpu = new_p;
            self.ctrl_vars[j].q_desire_endpu = new_q;
        }
        self.calc_wattvar_vars(j);
    }

    /// Pascal `CalcWATTVAR_vars(j)` — turn the clamped pu Q into the kvar set-point
    /// (`QDesiredWV = QDesireEndpu · QHeadRoom`/`QHeadRoomNeg`).
    fn calc_wattvar_vars(&mut self, j: usize) {
        let cv = &mut self.ctrl_vars[j];
        cv.q_desired_wv = if cv.q_desire_endpu >= 0.0 {
            cv.q_desire_endpu * cv.q_headroom
        } else {
            cv.q_desire_endpu * cv.q_headroom_neg
        };
    }

    /// Pascal `Calc_PBase(j)` — the volt-watt power base from `VoltWattYAxis`
    /// (0:=%Available `FDCkW·FEffFactor`, 1:=%Pmpp `FDCkWRated`, 2:=%PctPmpp
    /// `FDCkWRated·FpctDCkWRated`, 3:=%kVArating `FkVARating`). PVSystem path only —
    /// Storage VOLTWATT/VV_VW is guarded out at `Sample` (its `TStorageObj.DCkW`
    /// yaxis-0 branch is not reached).
    fn calc_pbase(&mut self, j: usize) {
        let cv = &mut self.ctrl_vars[j];
        cv.p_base = match self.voltwatt_yaxis {
            0 => cv.f_dckw * cv.f_eff_factor,
            1 => cv.f_dckw_rated,
            2 => cv.f_dckw_rated * cv.f_pct_dckw_rated,
            3 => cv.f_kva_rating,
            _ => cv.p_base, // yaxis is enum-constrained 0..=3; keep prior otherwise
        };
    }

    /// Pascal `CalcPVWcurve_limitpu(j)` — the volt-watt curve lookup (PVSystem path:
    /// `PLimitVWpu := Fvoltwatt_curve.GetYValue(FPresentVpu)`).
    fn calc_pvw_curve_limitpu(&mut self, j: usize) {
        let present_vpu = self.ctrl_vars[j].f_present_vpu;
        let value = self
            .voltwatt_curve
            .as_mut()
            .expect("voltwatt_curve checked at Sample")
            .get_y_value(present_vpu);
        self.ctrl_vars[j].p_limit_vw_pu = value;
    }

    /// Pascal `Check_Plimits(j, P)` — clamp the volt-watt P (pu) to the kVA-available
    /// headroom (var priority) and the `pctPmpp` limit; records `FVWOperation`.
    fn check_plimits(&mut self, j: usize, p: f64) {
        let cv = &mut self.ctrl_vars[j];
        cv.p_limit_limitedpu = 1.0; // not limited

        if p < 1.0 {
            cv.f_vw_operation = 1.0;
        }

        let pct_dckw_rated_limit = cv.f_pct_dckw_rated * cv.f_dckw_rated;

        // Under var priority, PLimitEnd must fit under the kVA-available headroom.
        if !cv.f_p_priority {
            let p_ppriority = (cv.f_kva_rating.powi(2) - cv.f_present_kvar.powi(2)).sqrt();
            if p_ppriority < p.abs() * cv.p_base {
                cv.p_limit_limitedpu = p_ppriority / cv.p_base * pas_sign(p);
                cv.f_vw_operation = 0.0; // kVA exceeded under var priority
            }
        }

        // PLimitEnd must be below the pctPmpp limit.
        if p.abs() * cv.p_base > pct_dckw_rated_limit {
            cv.f_vw_operation = 0.0; // pctPmpp exceeded
            cv.p_limit_limitedpu = pct_dckw_rated_limit / cv.p_base * pas_sign(p);
        }
    }

    /// Pascal `CalcVoltWatt_watts(j)` — the convergence step → `PLimitVW` (kW). In
    /// the volt-watt "requesting" region (`PLimitEndpu < 1` and `<= |kW_out_desiredpu|`,
    /// or once latched) it moves slowly toward the curve point with `FdeltaPFactor`;
    /// otherwise it takes the curve point directly.
    fn calc_voltwatt_watts(&mut self, j: usize, control_iter: i32) {
        let cv = &mut self.ctrl_vars[j];
        if (cv.p_limit_endpu < 1.0 && cv.p_limit_endpu <= cv.kw_out_desiredpu.abs())
            || cv.f_flag_vw_operates
        {
            if control_iter == 1 {
                // abs() because the DER might be charging (Storage); always positive
                // on the 1st control iteration.
                cv.p_old_vw_pu = cv.kw_out_desiredpu.abs();
            }
            cv.f_flag_vw_operates = true;

            let delta_p_pu = cv.p_limit_endpu - cv.p_old_vw_pu;
            if self.delta_p_factor == FLAGDELTAP {
                // Adaptive factor (Change_deltaP_factor); inlined to keep the single
                // &mut borrow of cv. Note the bands differ from Change_deltaQ_factor.
                let delta_v = (cv.f_present_vpu - cv.f_avgp_vpu_prior).abs();
                if cv.delta_v_old >= 0.0 {
                    if delta_v.abs() > 0.9 * cv.delta_v_old && cv.f_delta_p_factor > 0.2 {
                        cv.f_delta_p_factor -= 0.1;
                    } else if delta_v.abs() > 0.8 * cv.delta_v_old && cv.f_delta_p_factor > 0.1 {
                        cv.f_delta_p_factor -= 0.05;
                    } else if delta_v.abs() < 0.2 * cv.delta_v_old && cv.f_delta_p_factor < 0.9 {
                        cv.f_delta_p_factor += 0.05;
                    } else if delta_v.abs() < 0.1 * cv.delta_v_old && cv.f_delta_p_factor < 0.9 {
                        cv.f_delta_p_factor += 0.1;
                    }
                }
                cv.delta_v_old = (cv.f_present_vpu - cv.f_avgp_vpu_prior).abs();
            } else {
                // A set DeltaP_factor is used directly each iteration (no compat-flag
                // gating on the P side, unlike DeltaQ_factor).
                cv.f_delta_p_factor = self.delta_p_factor;
            }
            cv.p_limit_vw = (cv.p_old_vw_pu + delta_p_pu * cv.f_delta_p_factor) * cv.p_base;
        } else {
            cv.p_limit_vw = cv.p_limit_endpu * cv.p_base;
        }
    }

    /// The "mode/combi not yet ported" error (records the active mode for clarity).
    fn not_ported_mode(&self) -> String {
        format!(
            "InvControl.{}: VOLTVAR/VOLTWATT/DRC/WATTPF/WATTVAR/AVR + the VV_VW/VV_DRC combis are ported (WP7.5 step 2b-2e-ii); mode={} combi={} is deferred to WP7.7 (GFM)",
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
