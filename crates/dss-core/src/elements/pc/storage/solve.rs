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
use crate::elements::traits::SysCtx;
use crate::support::cmatrix::CMatrix;
use crate::support::mathutil::SymComp;
use crate::util::sqrt3;

use super::{STORE_CHARGING, STORE_DISCHARGING, Storage};

impl Storage {
    /// Pascal `CalcYPrimMatrix` (power-flow path). `Y` depends on the state:
    /// charging stamps `+YeqDischarge`, idling stamps 0, discharging stamps
    /// `−YeqDischarge`. The harmonic-model branch (uses the `%R`/`%X` `Yeq`) and
    /// the grid-forming (`CalcGFMYprim`) branch are WP7.6/7.7.
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
        let mut y = match self.f_state {
            STORE_CHARGING => self.yeq_discharge,
            STORE_DISCHARGING => -self.yeq_discharge,
            _ => Complex64::ZERO, // idling
        };
        // Modify the base admittance for harmonics.
        y.im /= freq_multiplier;

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
    fn calc_yprim_contribution(&mut self, node_v: &[Complex64]) {
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
    /// The dynamics (`DoDynamicMode`) and harmonic (`DoHarmonicMode`)
    /// contributions are genuinely unreachable from a power-flow solve (those
    /// modes still error before any element runs) and land in WP7.6/7.7. The
    /// grid-forming (`DoGFM_Mode`) contribution is **also** WP7.7, but
    /// `ControlMode=GFM` is a settable per-element property, so it *is* reachable
    /// — guarded with an explicit "not ported" error rather than silently running
    /// the regular PQ model (which would give plausible-but-wrong numbers).
    pub(super) fn calc_storage_model_contribution(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
        errors: &mut Vec<String>,
    ) {
        self.cd.iterminal_updated = false;
        if self.base.gfm_mode {
            // Pascal `if GFM_Mode then DoGFM_Mode(); Exit;` — DoGFM_Mode /
            // CalcGFMYprim are WP7.7 (dynamics). Init InjCurrent from Yprim like
            // the user-model path, then surface a clear unported error.
            self.calc_yprim_contribution(node_v);
            errors.push(format!(
                "Storage.{}: grid-forming inverter mode (ControlMode=GFM) is not \
                 ported yet (Phase 7 WP7.7).",
                self.cd.obj.name()
            ));
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

    /// Pascal `CalcInjCurrentArray`.
    pub(super) fn calc_inj_current_array(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
        errors: &mut Vec<String>,
    ) {
        if self.storage_obj_switch_open {
            self.cd.inj_current.fill(Complex64::ZERO);
        } else {
            self.calc_storage_model_contribution(sys, node_v, errors);
        }
    }
}
