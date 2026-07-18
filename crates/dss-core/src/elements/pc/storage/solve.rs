//! Solve-time electrical machinery: `CalcYPrimMatrix` (state-dependent
//! `YeqDischarge`), the `DoConstantPQ` / `DoConstantZ` model currents and the
//! injection/terminal-current assembly. The model-current signs are the reverse
//! of a load's (the inverter pushes power into the node when discharging),
//! routed by [`InvBasedPceData::stick_curr_in_terminal_array`].
//!
//! [`InvBasedPceData::stick_curr_in_terminal_array`]:
//!     crate::elements::pc::inv_based_pce::InvBasedPceData::stick_curr_in_terminal_array

use num_complex::Complex64;

use crate::elements::pc::inv_based_pce::Connection;
use crate::elements::traits::{CktElement, SysCtx};
use crate::support::cmatrix::CMatrix;
use crate::support::complexutil::{cang, rotate_phasor_deg, rotate_phasor_rad};
use crate::support::mathutil::SymComp;
use crate::util::sqrt3;

use super::{STORE_CHARGING, STORE_DISCHARGING, Storage};

impl Storage {
    /// Pascal `CalcYPrimMatrix` (power-flow path). `Y` depends on the state:
    /// charging stamps `+YeqDischarge`, idling stamps 0, discharging stamps
    /// `−YeqDischarge`. The harmonic-model branch uses the `%R`/`%X` `Yeq`; a
    /// discharging **grid-forming** unit stamps the `CalcGFMYprim` short-circuit
    /// admittance instead (WPG.13).
    pub(super) fn calc_yprim_matrix(&mut self, ymatrix: &mut CMatrix, sys: &SysCtx) {
        self.cd.yprim_freq = sys.frequency;
        let freq_multiplier = self.cd.yprim_freq / self.cd.base_frequency;

        let nphases = self.cd.nphases;
        let nconds = self.cd.nconds;

        if sys.is_harmonic_model {
            // Yeq is computed from %R and %X — inverse of Rthev + j Xthev.
            let mut y = self.base.yeq;
            if self.base.connection == Connection::Delta {
                y /= 3.0; // convert to delta impedance
            }
            y.im /= freq_multiplier;
            let yij = -y;
            match self.base.connection {
                Connection::Wye => {
                    for i in 0..nphases {
                        ymatrix.set(i, i, y);
                        ymatrix.add(nconds - 1, nconds - 1, y);
                        ymatrix.set(i, nconds - 1, yij);
                        ymatrix.set(nconds - 1, i, yij);
                    }
                }
                Connection::Delta => {
                    for i in 0..nphases {
                        ymatrix.set(i, i, y);
                        ymatrix.add(i, i, y); // put it in again
                        for j in 0..i {
                            ymatrix.set(i, j, yij);
                            ymatrix.set(j, i, yij);
                        }
                    }
                }
            }
            return;
        }

        // Regular power-flow Storage model. Yeq is the L-N equivalent admittance.
        // A discharging **grid-forming** unit stamps the CalcGFMYprim short-circuit
        // admittance directly and exits (Pascal `if GFM_mode then Exit`).
        let mut y = match self.f_state {
            STORE_CHARGING => self.yeq_discharge,
            STORE_DISCHARGING if !self.base.gfm_mode => -self.yeq_discharge,
            STORE_DISCHARGING => {
                // GFM discharging: `with dynVars, StorageVars` seeds the GFM
                // impedance inputs, then `CalcGFMYprim` fills YMatrix in place.
                self.base.dyn_vars.rated_kv_ll = self.kv_storage_base; // PresentkV
                self.base.dyn_vars.discharging = self.f_state == STORE_DISCHARGING;
                self.base.dyn_vars.m_kva_rating = self.f_kva_rating;
                let order = ymatrix.order();
                let gfm = self.base.dyn_vars.calc_gfm_yprim(nphases, order);
                ymatrix.copy_from(&gfm);
                Complex64::ZERO // Y is unused on the GFM path (exits below).
            }
            _ => Complex64::ZERO, // idling
        };
        // Modify the base admittance for harmonics.
        y.im /= freq_multiplier;

        // Grid-forming mode built its whole YMatrix above (CalcGFMYprim), or (for a
        // non-discharging GFM unit) stamps nothing — either way exit here.
        if self.base.gfm_mode {
            return;
        }

        match self.base.connection {
            Connection::Wye => {
                let yij = -y;
                for i in 0..nphases {
                    ymatrix.set(i, i, y);
                    ymatrix.add(nconds - 1, nconds - 1, y);
                    ymatrix.set(i, nconds - 1, yij);
                    ymatrix.set(nconds - 1, i, yij);
                }
            }
            Connection::Delta => {
                let y = y / 3.0; // convert to delta impedance
                let yij = -y;
                for i in 0..nphases {
                    let j = if i + 1 >= nconds { 0 } else { i + 1 };
                    ymatrix.add(i, i, y);
                    ymatrix.add(j, j, y);
                    ymatrix.add_sym(i, j, yij);
                }
            }
        }
    }

    /// Pascal `StickCurrInTerminalArray` routed into `ITerminal` or `InjCurrent`,
    /// reusing the shared inverter base routine.
    fn stick_curr(&mut self, into_iterminal: bool, curr: Complex64, i: usize) {
        let nconds = self.cd.nconds;
        if into_iterminal {
            self.base
                .stick_curr_in_terminal_array(&mut self.cd.iterminal, nconds, curr, i);
        } else {
            self.base
                .stick_curr_in_terminal_array(&mut self.cd.inj_current, nconds, curr, i);
        }
    }

    /// Pascal `CalcVTerminalPhase`.
    fn calc_vterminal_phase(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        let nphases = self.cd.nphases;
        let nconds = self.cd.nconds;
        match self.base.connection {
            Connection::Wye => {
                for i in 0..nphases {
                    self.cd.vterminal[i] =
                        node_v[self.cd.node_ref[i]] - node_v[self.cd.node_ref[nconds - 1]];
                }
            }
            Connection::Delta => {
                for i in 0..nphases {
                    let j = if i + 1 >= nconds { 0 } else { i + 1 };
                    self.cd.vterminal[i] =
                        node_v[self.cd.node_ref[i]] - node_v[self.cd.node_ref[j]];
                }
            }
        }
        self.storage_solution_count = sys.solution_count;
    }

    /// Pascal `CalcYPrimContribution`: `InjCurrent = Yprim · V(node)`.
    pub(super) fn calc_yprim_contribution(&mut self, node_v: &[Complex64]) {
        self.cd.compute_vterminal(node_v);
        let cd = &mut self.cd;
        if let Some(yprim) = &cd.yprim {
            yprim.mv_mult(&mut cd.inj_current, &cd.vterminal);
        }
    }

    /// The shared tail of each `DoXxx`: terminal/injection bookkeeping.
    fn put_curr(&mut self, sys: &SysCtx, curr: Complex64, i: usize) {
        self.stick_curr(true, -curr, i); // into ITerminal
        self.cd.iterminal_updated = true;
        self.cd.iterminal_solution_count = sys.solution_count;
        self.stick_curr(false, curr, i); // into InjCurrent
    }

    /// Pascal `DoConstantPQStorageObj` (model 1): constant-PQ between Vminpu and
    /// Vmaxpu, an impedance model outside that band, or the current-limited model.
    fn do_constant_pq(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.calc_yprim_contribution(node_v); // init InjCurrent
        self.cd.zero_iterminal();
        self.calc_vterminal_phase(sys, node_v);

        if self.base.force_balanced && self.cd.nphases == 3 {
            self.force_pos_seq();
        }

        let s = Complex64::new(self.base.p_nominal_per_phase, self.base.q_nominal_per_phase);
        for i in 0..self.cd.nphases {
            let curr = match self.base.connection {
                Connection::Wye => {
                    let vln = self.cd.vterminal[i];
                    let vmag_ln = vln.norm();
                    let mut curr = if vmag_ln <= self.base.v_base_min {
                        self.base.yeq_min * vln // below Vminpu → impedance model
                    } else if vmag_ln > self.base.v_base_max {
                        self.base.yeq_max * vln // above Vmaxpu → impedance model
                    } else {
                        (s / vln).conj() // constant PQ
                    };
                    if self.base.current_limited && curr.norm() > self.max_dyn_phase_current {
                        curr = (self.base.phase_current_limit / (vln / vmag_ln)).conj();
                    }
                    curr
                }
                Connection::Delta => {
                    let vll = self.cd.vterminal[i];
                    let vmag_ll = vll.norm();
                    let vmag_ln = if self.cd.nphases > 1 {
                        vmag_ll / sqrt3()
                    } else {
                        vmag_ll
                    };
                    let mut curr = if vmag_ln <= self.base.v_base_min {
                        (self.base.yeq_min / 3.0) * vll
                    } else if vmag_ln > self.base.v_base_max {
                        (self.base.yeq_max / 3.0) * vll
                    } else {
                        (s / vll).conj()
                    };
                    if self.base.current_limited
                        && curr.norm() * sqrt3() > self.max_dyn_phase_current
                    {
                        curr = (self.base.phase_current_limit / (vll / vmag_ln)).conj();
                    }
                    curr
                }
            };
            self.put_curr(sys, curr, i);
        }
    }

    /// Pascal `DoConstantZStorageObj` (model 2).
    fn do_constant_z(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.calc_yprim_contribution(node_v); // init InjCurrent
        self.calc_vterminal_phase(sys, node_v);

        if self.base.force_balanced && self.cd.nphases == 3 {
            self.force_pos_seq();
        }

        self.cd.zero_iterminal();
        let yeq2 = match self.base.connection {
            Connection::Wye => self.base.yeq, // YEQ always line-neutral
            Connection::Delta => self.base.yeq / 3.0,
        };
        for i in 0..self.cd.nphases {
            let curr = yeq2 * self.cd.vterminal[i];
            self.put_curr(sys, curr, i);
        }
    }

    /// Pascal `ForceBalanced` block: keep only the positive-sequence Vterminal.
    fn force_pos_seq(&mut self) {
        let sc = SymComp::default();
        let mut v012 = [Complex64::ZERO; 3];
        sc.phase_to_sym(&self.cd.vterminal[..3], &mut v012);
        v012[0] = Complex64::ZERO; // zero-sequence → 0
        v012[2] = Complex64::ZERO; // negative-sequence → 0
        let mut vph = [Complex64::ZERO; 3];
        sc.sym_to_phase(&v012, &mut vph);
        self.cd.vterminal[..3].copy_from_slice(&vph);
    }

    /// Pascal `CalcStorageModelContribution`: dispatch the power-flow model.
    /// The dynamics (`DoDynamicMode`) path is reached only in a dynamics solve;
    /// harmonics (`DoHarmonicMode`) above the fundamental. The grid-forming
    /// (`DoGFM_Mode`) contribution (WPG.13) injects the internal voltage-source
    /// current through `YPrim` when `ControlMode=GFM`.
    pub(super) fn calc_storage_model_contribution(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
        errors: &mut crate::diag::ErrorLog,
    ) {
        // Dynamics mode: delegate entirely to DoDynamicMode (Pascal dispatch order).
        if sys.is_dynamic_model {
            self.do_dynamic_mode(sys, node_v, errors);
            return;
        }
        self.cd.iterminal_updated = false;
        // Harmonics (above the fundamental) inject the spectrum-scaled Thevenin
        // source — checked before GFM, matching Pascal's dispatch order.
        if sys.is_harmonic_model && sys.frequency != sys.fundamental {
            self.do_harmonic_mode(sys, node_v);
            return;
        }
        if self.base.gfm_mode {
            // Pascal `if GFM_Mode then DoGFM_Mode(); Exit;`.
            self.do_gfm_mode(node_v);
            return;
        }
        match self.base.voltage_model {
            1 => self.do_constant_pq(sys, node_v),
            2 => self.do_constant_z(sys, node_v),
            3 => {
                // User-written DLL model — never ported. Pascal inits InjCurrent
                // then records error 567.
                self.calc_yprim_contribution(node_v);
                errors.push(format!(
                    "Storage.{} model designated to use user-written model, but \
                     user-written model is not defined.",
                    self.cd.obj.name()
                ));
            }
            _ => self.do_constant_pq(sys, node_v),
        }
    }

    /// Pascal `TStorageObj.CheckOLInverter` (Storage.pas): true if any inverter
    /// phase current exceeds the per-phase VA rating divided by `VBase`
    /// (grid-forming overload check; `GFM_Mode = FALSE` short-circuits to false).
    pub fn check_ol_inverter(&mut self, sys: &SysCtx, node_v: &[Complex64]) -> bool {
        if !self.base.gfm_mode {
            return false;
        }
        let nphases = self.cd.nphases;
        let max_amps = ((self.f_kva_rating * 1000.0) / nphases as f64) / self.base.v_base;
        // Pascal `GetCurrents(Iterminal)`: fresh currents written into `Iterminal`
        // (so a later cache-aware `Get_Powers` reuses them — do not leave the
        // `Iterminal` cache flagged fresh but unwritten).
        self.refresh_iterminal(sys, node_v);
        (0..nphases).any(|i| self.cd.iterminal[i].norm() > max_amps)
    }

    /// Pascal `TInvBasedPCE.CheckAmpsLimit` (InvBasedPCE.pas l.182): the GFM
    /// amps limiter — set `dynVars.IComp` to the largest per-phase apparent
    /// power (`|I|·|V_node|`) exceeding the `ILimit·VBase` threshold, and return
    /// whether any phase exceeded it (drives the `DoGFM_Mode` `IComp > 0` BaseV
    /// shrink on the next solve).
    pub fn check_amps_limit(&mut self, sys: &SysCtx, node_v: &[Complex64]) -> bool {
        let nom_p = self.base.dyn_vars.i_limit * self.base.v_base;
        if !self.base.gfm_mode {
            return false;
        }
        // Pascal `GetCurrents(Iterminal)` — fresh currents into `Iterminal`.
        self.refresh_iterminal(sys, node_v);
        let nphases = self.cd.nphases;
        self.base.dyn_vars.i_comp = 0.0;
        let mut result = false;
        for i in 0..nphases {
            let phase_amps = self.cd.iterminal[i].norm();
            let volts = node_v[self.cd.node_ref[i]].norm();
            let phase_p = phase_amps * volts;
            if phase_p > nom_p {
                if phase_p > self.base.dyn_vars.i_comp {
                    self.base.dyn_vars.i_comp = phase_p;
                }
                result = true;
            }
        }
        result
    }

    /// Pascal `TStorageObj.DoGFM_Mode` (Storage.pas l.2185): the grid-forming
    /// inverter as an internal balanced voltage source (`CalcGFMVoltage` at
    /// `BaseV`) behind the `CalcGFMYprim` short-circuit impedance already stamped
    /// into `YPrim`. `InjCurrent = YPrim · Vterminal(internal)`; `ITerminal` is
    /// left *not* updated so a later `GetCurrents` recomputes it against the node
    /// voltage (`TInvBasedPCE.GetCurrents`). The `IComp > 0` branch shrinks
    /// `BaseV` when the amps limiter (InvControl GFM mode) is saturating.
    pub(super) fn do_gfm_mode(&mut self, node_v: &[Complex64]) {
        let _ = node_v; // internal source voltage — independent of node voltages
        // dynVars.BaseV := VBase; Discharging := (StorageState = STORE_DISCHARGING).
        self.base.dyn_vars.base_v = self.base.v_base;
        self.base.dyn_vars.discharging = self.f_state == STORE_DISCHARGING;
        if self.base.dyn_vars.i_comp > 0.0 {
            let z_sys =
                2.0 * (self.base.v_base * self.base.dyn_vars.i_limit) - self.base.dyn_vars.i_comp;
            self.base.dyn_vars.base_v =
                (z_sys / self.base.dyn_vars.i_limit) * self.base.dyn_vars.v_error;
        }
        let nphases = self.cd.nphases;
        self.base
            .dyn_vars
            .calc_gfm_voltage(nphases, &mut self.cd.vterminal);
        // InjCurrent = YPrim · Vterminal (overwrites, like the source elements).
        let cd = &mut self.cd;
        if let Some(yprim) = &cd.yprim {
            yprim.mv_mult(&mut cd.inj_current, &cd.vterminal);
        }
        // set_ITerminalUpdated(FALSE): force GetCurrents to recompute Iterminal.
        self.cd.iterminal_updated = false;
    }

    /// Pascal `CalcInjCurrentArray`.
    pub(super) fn calc_inj_current_array(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
        errors: &mut crate::diag::ErrorLog,
    ) {
        if self.storage_obj_switch_open {
            self.cd.inj_current.fill(Complex64::ZERO);
        } else {
            self.calc_storage_model_contribution(sys, node_v, errors);
        }
    }

    /// Pascal `TStorageObj.InitHarmonics`: a Thevenin equivalent behind the
    /// `%R`/`%X` reactance — capture its source magnitude/angle from the present
    /// fundamental terminal current. `Yeq` becomes the L-N harmonic admittance the
    /// harmonic `CalcYPrimMatrix` branch consumes.
    pub(super) fn init_harmonics_impl(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.cd.yprim_invalid = true; // force YPrim rebuild
        self.storage_fundamental = sys.frequency; // frequency on entry
        let z_thev = Complex64::new(self.r_thev, self.x_thev);
        self.base.yeq = z_thev.inv(); // L-N, used for current calcs

        self.compute_iterminal(sys, node_v); // present value of current
        let nconds = self.cd.nconds;
        let va = match self.base.connection {
            // wye — neutral is explicit
            Connection::Wye => node_v[self.cd.node_ref[0]] - node_v[self.cd.node_ref[nconds - 1]],
            // delta — assume neutral is at zero
            Connection::Delta => node_v[self.cd.node_ref[0]],
        };
        let e = va - self.cd.iterminal[0] * z_thev;
        self.v_thev_harm = e.norm(); // base mag
        self.theta_harm = cang(e); // base angle (radians)
    }

    /// Pascal `TStorageObj.DoHarmonicMode`: the Storage element as a voltage
    /// source behind `%R`/`%X` — the spectrum-scaled, phase-rotated Thevenin
    /// voltage pushed through YPrim to the injection current.
    fn do_harmonic_mode(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.cd.compute_vterminal(node_v);
        let storage_harmonic = sys.frequency / self.storage_fundamental;
        let mult = self
            .spectrum_obj
            .as_ref()
            .map(|s| s.get_mult(storage_harmonic))
            .unwrap_or(Complex64::ZERO);
        let mut e = mult * self.v_thev_harm; // base harmonic magnitude
        e = rotate_phasor_rad(e, storage_harmonic, self.theta_harm); // fundamental phase shift

        let nphases = self.cd.nphases;
        let nconds = self.cd.nconds;
        let mut buffer = vec![Complex64::ZERO; nconds];
        for (i, slot) in buffer.iter_mut().enumerate().take(nphases) {
            *slot = e;
            if i < nphases - 1 {
                e = rotate_phasor_deg(e, storage_harmonic, -120.0); // assume 3-phase
            }
        }
        // Handle wye connection: assume no neutral injection voltage.
        if self.base.connection == Connection::Wye {
            buffer[nconds - 1] = self.cd.vterminal[nconds - 1];
        }

        // InjCurrent = YPrim · buffer.
        let cd = &mut self.cd;
        if let Some(yprim) = &cd.yprim {
            yprim.mv_mult(&mut cd.inj_current, &buffer);
        }
    }
}
