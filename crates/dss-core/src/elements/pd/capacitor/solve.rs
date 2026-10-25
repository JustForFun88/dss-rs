//! Impedance/admittance numerics (`RecalcElementData`, `MakeYprimWork`,
//! `CalcYPrim`) and the `impl CktElement` for [`Capacitor`].

use num_complex::Complex64;

use super::{CUF_SCALE, Capacitor, CapacitorSpecType};
use crate::elements::ckt::CktElementData;
use crate::elements::pd::matrix_order_refusal;
use crate::elements::pos_seq::{
    PosSeqAction, PosSeqCtx, PosSeqLeg, PosSeqLegKind, PosSeqPlan, legs, pos_seq_self_term,
};
use crate::elements::traits::{CktElement, ReliabilityData, SysCtx};
use crate::support::cmatrix::{CMatrix, StampBl};
use crate::util::sqrt3;

/// What a refused recalc derived, read to name the input of an overflow.
struct Trial<'a> {
    /// The capacitance of each step.
    fc: &'a [f64],
    /// The angular frequency.
    w: f64,
    phase_kv: f64,
}

impl Capacitor {
    /// Derive the capacitance of each step, the total kvar, the filter
    /// impedance of each tuned step and (unless typed) the normal and
    /// emergency current ratings. When an input makes one of them not a finite
    /// number, in that order, nothing is committed: the bank keeps what its last good recalc
    /// derived and refuses the build ([`CktElementData::refuse_recalc`]).
    pub(super) fn recalc(&mut self) {
        use crate::obj::Rating;
        let two_pi = 2.0 * std::f64::consts::PI;
        let w = two_pi * self.cd.base_frequency;
        let nphases = self.cd.nphases as f64;
        let n = self.n_steps();
        let mut fc = self.fc.clone();
        let mut ftotalkvar = 0.0;
        let mut phase_kv = 1.0;

        match self.spec_type {
            CapacitorSpecType::Kvar => {
                phase_kv = self.phase_kv();
                // Each step's capacitance comes from its own rating (pinned by
                // `derived_cuf_multistep_takes_each_steps_own_rating` and
                // `a_kvar_bank_of_unequal_steps_draws_its_rating`). The square
                // is one factor, `w * (kv*kv)`, which `Cuf` renders to the bit
                // (`derived_cuf_kvar_wye_matches_oracle_bit_exactly`).
                let kv_sq = phase_kv.powi(2);
                for (c, &kvar) in fc.iter_mut().zip(&self.fkvarrating) {
                    *c = 1.0 / (w * kv_sq * 1000.0 / (kvar / nphases));
                }
                for &k in self.fkvarrating.iter().take(n) {
                    ftotalkvar += k;
                }
            }
            CapacitorSpecType::Cuf => {
                phase_kv = self.phase_kv();
                for &c in fc.iter().take(n) {
                    // The square binds first, as the derived normal and
                    // emergency ratings below show.
                    ftotalkvar += w * c * phase_kv.powi(2) / 1000.0;
                }
            }
            // A `cmatrix` bank derives nothing, and its ratings divide by a
            // phase voltage of 1.
            CapacitorSpecType::CMatrix => {}
        }

        let mut fxl = self.fxl.clone();
        let mut fr = self.fr.clone();
        if self.do_harmonic_recalc {
            for i in 0..n {
                // A step of no size is an open branch with no filter
                // (`a_zero_kvar_reactor_is_an_open_device`), and harmonic 0
                // means no filter.
                fxl[i] = if self.fharm[i] != 0.0 && fc[i] != 0.0 {
                    (1.0 / (w * fc[i])) / (self.fharm[i] * self.fharm[i])
                } else {
                    0.0
                };
                if fr[i] == 0.0 {
                    fr[i] = fxl[i] / 1000.0;
                }
            }
        }

        let kvar_per_phase = ftotalkvar / nphases;
        let derive = |specified: bool, held: Rating, factor: f64| {
            if specified {
                Ok(held)
            } else {
                Rating::derived(kvar_per_phase / phase_kv * factor)
            }
        };
        let norm = derive(self.norm_amps_specified, self.norm_amps, 1.35);
        let emerg = derive(self.emerg_amps_specified, self.emerg_amps, 1.8);

        let finite = |v: &[f64]| v.iter().take(n).all(|x| x.is_finite());
        let stamp_finite = match self.spec_type {
            CapacitorSpecType::CMatrix => self
                .cmatrix
                .as_ref()
                .is_none_or(|m| m.iter().all(|x| x.is_finite())),
            _ => finite(&fc),
        };
        let quantity = if !stamp_finite {
            "capacitance"
        } else if !(finite(&fxl) && finite(&fr)) {
            "filter impedance"
        } else if !ftotalkvar.is_finite() {
            "total kvar"
        } else if norm.is_err() {
            "normal current rating"
        } else if emerg.is_err() {
            "emergency current rating"
        } else {
            ""
        };
        match (norm, emerg) {
            (Ok(norm), Ok(emerg)) if quantity.is_empty() => {
                self.cd.recalc_refusal = None;
                self.fc = fc;
                self.ftotalkvar = ftotalkvar;
                self.fxl = fxl;
                self.fr = fr;
                self.norm_amps = norm;
                self.emerg_amps = emerg;
            }
            _ => {
                let trial = Trial {
                    fc: &fc,
                    w,
                    phase_kv,
                };
                let input = self.refused_input(quantity, &trial);
                self.cd.refuse_recalc("Capacitor", &input, quantity);
            }
        }
    }

    /// The input a refused recalc names: the first input the bank's form
    /// reads that is not a finite number, else a zero `kV`, else the cause of
    /// the overflow. A filter reactance `1/(w C)` that overflows names the
    /// input that sizes the step (`Cuf`, or `kvar`), a finite one divided by
    /// a harmonic that overflows names `Harm`. A total kvar that overflows
    /// names the steps' `kvar` or `Cuf`, or `kV` when its square overflows.
    /// Any other overflow names `kV`, the divisor.
    fn refused_input(&self, quantity: &str, trial: &Trial) -> String {
        use crate::elements::pd::{array_input_text, input_text, sym_matrix_input_text};
        let n = self.n_steps();
        let steps = |v: &[f64]| v[..n.min(v.len())].to_vec();
        let not_finite = |v: &[f64]| v.iter().take(n).any(|x| !x.is_finite());
        let uses_kv = self.spec_type != CapacitorSpecType::CMatrix;
        if self.spec_type == CapacitorSpecType::Kvar && not_finite(&self.fkvarrating) {
            return array_input_text("kvar", &steps(&self.fkvarrating), 1.0);
        }
        if uses_kv && !self.kvrating.is_finite() {
            return input_text("kV", self.kvrating);
        }
        if let Some(m) = self
            .cmatrix
            .as_ref()
            .filter(|m| !uses_kv && m.iter().any(|x| !x.is_finite()))
        {
            return sym_matrix_input_text("CMatrix", m, self.cd.nphases, CUF_SCALE);
        }
        if self.spec_type == CapacitorSpecType::Cuf && not_finite(&self.fc) {
            return array_input_text("Cuf", &steps(&self.fc), CUF_SCALE);
        }
        for (name, v) in [("R", &self.fr), ("XL", &self.fxl), ("Harm", &self.fharm)] {
            if not_finite(v) {
                return array_input_text(name, &steps(v), 1.0);
            }
        }
        let kv = input_text("kV", self.kvrating);
        if uses_kv && self.kvrating == 0.0 {
            return kv;
        }
        let step_size = || match self.spec_type {
            CapacitorSpecType::Cuf => array_input_text("Cuf", &steps(&self.fc), CUF_SCALE),
            _ => array_input_text("kvar", &steps(&self.fkvarrating), 1.0),
        };
        match quantity {
            "filter impedance" => {
                let reactance_overflows = trial
                    .fc
                    .iter()
                    .zip(&self.fharm)
                    .take(n)
                    .any(|(&c, &h)| h != 0.0 && c != 0.0 && !(1.0 / (trial.w * c)).is_finite());
                if reactance_overflows {
                    step_size()
                } else {
                    array_input_text("Harm", &steps(&self.fharm), 1.0)
                }
            }
            "total kvar" => {
                // A `Cuf` step's kvar is `w C kV²`: the voltage overflows it
                // when its square alone is not finite.
                if self.spec_type == CapacitorSpecType::Cuf && !trial.phase_kv.powi(2).is_finite() {
                    kv
                } else {
                    step_size()
                }
            }
            _ => kv,
        }
    }

    /// Pascal per-phase voltage selection (`RecalcElementData`): delta uses the
    /// can rating; wye assumes a three-phase line-line rating for 2/3 phases.
    fn phase_kv(&self) -> f64 {
        if self.connection == 1 {
            self.kvrating // delta: line-line
        } else {
            match self.cd.nphases {
                2 | 3 => self.kvrating / sqrt3(),
                _ => self.kvrating,
            }
        }
    }

    /// Why the solve refuses this bank, if it does: the input its last recalc
    /// refused, and for a `cmatrix` bank a delta of two or more phases (a
    /// nodal matrix has no defined delta stamp) or a matrix of another order
    /// than `nphases × nphases`.
    fn refusal(&self) -> Option<String> {
        if let Some(msg) = &self.cd.recalc_refusal {
            return Some(msg.clone());
        }
        if self.spec_type != CapacitorSpecType::CMatrix {
            return None;
        }
        let full = format!("Capacitor.{}", self.cd.obj.name());
        let nphases = self.cd.nphases;
        if self.connection == 1 && nphases >= 2 {
            return Some(format!(
                "{full}: a cmatrix bank of {nphases} phases cannot be connected in delta. \
                 Specify it with conn=wye. Aborting solution."
            ));
        }
        let len = self.cmatrix.as_ref().map_or(0, Vec::len);
        matrix_order_refusal(&full, nphases, "cmatrix", len, "cmatrix")
    }

    /// A closed three-phase delta: three legs, one conductor per phase.
    fn is_closed_delta(&self) -> bool {
        self.connection == 1 && self.cd.nphases == 3 && self.cd.nconds == 3
    }

    /// The series filter of each step, `R` and `XL`, as `?` reads them.
    fn filter_input_text(&self) -> String {
        use crate::elements::pd::array_input_text;
        let n = self.n_steps();
        let steps = |v: &[f64]| v[..n.min(v.len())].to_vec();
        format!(
            "{} {}",
            array_input_text("R", &steps(&self.fr), 1.0),
            array_input_text("XL", &steps(&self.fxl), 1.0)
        )
    }

    /// Build one energized step's admittance into `ywork`, cleared first, so
    /// the steps of a bank add as capacitors in parallel in every connection
    /// (pinned by `a_multistep_delta_bank_draws_the_sum_of_its_steps`).
    fn make_yprim_work(&self, ywork: &mut CMatrix, istep: usize, freq: f64) {
        ywork.clear();
        let two_pi = 2.0 * std::f64::consts::PI;
        let freq_multiple = freq / self.cd.base_frequency;
        let w = two_pi * freq;
        let i_step = istep - 1;
        let nphases = self.cd.nphases;
        let nconds = self.cd.nconds;

        // A step of no size is an open branch: it stamps nothing.
        if self.spec_type != CapacitorSpecType::CMatrix && self.fc[i_step] == 0.0 {
            return;
        }
        let has_zl = (self.fr[i_step] + self.fxl[i_step].abs()) > 0.0;
        let zl = Complex64::new(self.fr[i_step], self.fxl[i_step] * freq_multiple);

        match self.spec_type {
            CapacitorSpecType::Kvar | CapacitorSpecType::Cuf => {
                let mut value = Complex64::new(0.0, self.fc[i_step] * w);
                if self.connection == 1 {
                    // Delta (line-line): each leg adds to the two nodes it joins.
                    ywork.stamp_delta_series(nphases, nconds, value);
                } else {
                    // Wye; assignment overwrites.
                    if has_zl {
                        value = (zl + value.inv()).inv(); // add in ZL
                    }
                    ywork.stamp_two_terminal_diag(nphases, nphases, value);
                }
            }
            CapacitorSpecType::CMatrix => {
                // `calc_yprim` refuses the delta form of two or more phases and
                // a matrix of another order before it reaches here, so the
                // matrix holds `nphases²` entries.
                let cm = self.cmatrix.as_deref().unwrap_or_default();
                ywork.stamp_two_terminal_block(nphases, StampBl::Transposed, |i, j| {
                    Complex64::new(0.0, cm[i * nphases + j] * w)
                });
            }
        }

        // Add the filter reactance, if any.
        if !has_zl {
            return;
        }
        match self.spec_type {
            CapacitorSpecType::Kvar | CapacitorSpecType::Cuf => {
                if self.connection == 1 {
                    // Delta: invert, add ZL in series on the diagonal, re-invert.
                    for i in 1..=nphases {
                        let d = ywork.get(i - 1, i - 1) * 1.000001;
                        ywork.set(i - 1, i - 1, d);
                    }
                    let _ = ywork.invert();
                    for i in 1..=nphases {
                        let v = zl + ywork.get(i - 1, i - 1);
                        ywork.set(i - 1, i - 1, v);
                    }
                    let _ = ywork.invert();
                }
                // Wye: ZL already folded into `value` above.
            }
            CapacitorSpecType::CMatrix => {
                // dss_capi 0.15.x (`Capacitor.pas` `MakeYprimWork`, SpecType=3):
                // "Add a little bit to each phase so it will invert" — the same
                // ×1.000001 diagonal perturbation the Delta 1|2 branch already
                // used, added to the Cmatrix branch so a singular C matrix still
                // inverts. Only reached when the Cmatrix capacitor has a series
                // filter reactance (`has_zl`). UPGRADE_PLAN WP-U1.2 row B1; ledger
                // DIVERGENCES.md §B1.
                for i in 1..=nphases {
                    let d = ywork.get(i - 1, i - 1) * 1.000001;
                    ywork.set(i - 1, i - 1, d);
                }
                let _ = ywork.invert();
                for i in 1..=nphases {
                    let v = zl + ywork.get(i - 1, i - 1);
                    ywork.set(i - 1, i - 1, v);
                }
                let _ = ywork.invert();
            }
        }
    }
}

impl CktElement for Capacitor {
    fn cd(&self) -> &CktElementData {
        &self.cd
    }
    fn cd_mut(&mut self) -> &mut CktElementData {
        &mut self.cd
    }

    /// Wye as branches from terminal 1 to terminal 2, delta as delta legs,
    /// rated at `kV`. A `kvar` bank draws `-j 1000 Σkvar / N` VA a leg over
    /// all its steps, one given by capacitance nothing.
    fn pos_seq_legs(&self) -> Option<Vec<PosSeqLeg>> {
        let cd = &self.cd;
        let n = cd.nphases;
        let delta = self.connection == 1;
        let v = legs::v_leg_of(self.kvrating, n, delta);
        let s = match self.spec_type {
            CapacitorSpecType::Kvar => {
                let kvar: f64 = self.fkvarrating.iter().take(self.n_steps()).sum();
                Complex64::new(0.0, -1000.0 * kvar / n as f64)
            }
            _ => Complex64::ZERO,
        };
        Some(if delta {
            legs::delta(n, PosSeqLegKind::Lumped, v, s)
        } else {
            legs::branch(n, 0, cd.nconds, PosSeqLegKind::Lumped, v, s)
        })
    }

    fn pos_seq_refusal(&self) -> Option<String> {
        self.refusal()
    }

    fn norm_amps(&self) -> crate::obj::Rating {
        self.norm_amps
    }
    fn emerg_amps(&self) -> crate::obj::Rating {
        self.emerg_amps
    }

    /// Pascal `TPDElement.IsShunt` (set by the Bus1/Bus2 side effects).
    fn is_shunt(&self) -> bool {
        self.is_shunt
    }

    /// Pascal `TPDElement.CalcFltRate` (base): `Faultrate · pctperm · 0.01`.
    fn reliability_data(&self) -> ReliabilityData {
        ReliabilityData {
            branch_flt_rate: self.fault_rate * self.pct_perm * 0.01,
            hrs_to_repair: self.hrs_to_repair,
            miles_this_line: 0.0,
            fault_rate: self.fault_rate,
            pct_perm: self.pct_perm,
        }
    }

    /// Pascal `TCapacitorObj.CalcYPrim`: accumulate every energized step into the
    /// shunt (or series) primitive, then mirror tiny diagonals into the other
    /// matrix so `CalcVoltages` never sees an all-zero row.
    fn calc_yprim(&mut self, sys: &SysCtx) {
        let yorder = self.cd.yorder;
        self.cd.yprim_freq = sys.frequency;

        let mut yp_series = CMatrix::new(yorder);
        let mut yp_shunt = CMatrix::new(yorder);
        let mut ywork = CMatrix::new(yorder);

        if let Some(msg) = self.refusal() {
            // An enabled bank refuses the solve (pinned by
            // `a_delta_cmatrix_bank_refuses_the_solve` and
            // `a_cmatrix_of_another_phase_count_refuses_the_solve`). A disabled
            // one is out of the model and keeps a zero stamp (pinned by
            // `a_disabled_delta_cmatrix_bank_leaves_the_solve_to_the_rest`).
            if self.cd.enabled {
                self.cd.obj.push_error(msg);
                self.cd.yprim_refused = true;
            }
            self.cd.yprim_series = Some(yp_series);
            self.cd.yprim_shunt = Some(yp_shunt);
            self.cd.yprim = Some(ywork);
            self.cd.yprim_invalid = false;
            return;
        }

        {
            let temp = if self.is_shunt {
                &mut yp_shunt
            } else {
                &mut yp_series
            };
            for step in 1..=self.n_steps() {
                if self.fstates[step - 1] == 1 {
                    self.make_yprim_work(&mut ywork, step, sys.frequency);
                    temp.add_from(&ywork);
                }
            }
        }

        // A filter in series resonance at the solve frequency with no
        // resistance is a short the stamp cannot carry (pinned by
        // `exec::tests::rating_refusals::a_series_filter_at_its_resonance_refuses_its_capacitor`).
        let stamp = if self.is_shunt { &yp_shunt } else { &yp_series };
        if !stamp
            .values()
            .iter()
            .all(|z| z.re.is_finite() && z.im.is_finite())
        {
            let input = self.filter_input_text();
            let msg = self.cd.refusal_message("Capacitor", &input, "admittance");
            self.cd.refuse_yprim(msg);
            return;
        }

        // Set YPrim_Series from the shunt diagonals so CalcVoltages doesn't fail.
        if self.is_shunt {
            for i in 0..yorder {
                yp_series.set(i, i, yp_shunt.get(i, i) * 1.0e-10);
            }
        }

        let mut yprim = CMatrix::new(yorder);
        yprim.copy_from(if self.is_shunt { &yp_shunt } else { &yp_series });

        self.cd.yprim_series = Some(yp_series);
        self.cd.yprim_shunt = Some(yp_shunt);
        self.cd.yprim = Some(yprim);

        self.cd.apply_yprim_open_conductor_calcs();
        self.cd.yprim_invalid = false;
    }

    /// Collapse a capacitor bank to its positive-sequence single-phase form,
    /// keeping the bank's power under a balanced voltage (the positive-sequence
    /// circuit reports three times the one phase). By `SpecType`:
    /// - 1 (kvar): kV per the connection/phase rule, per-step kvar/3, `Phases:=1`.
    /// - 2 (Cuf): a *bare* `Phases := 1` (the applier brackets it), which keeps
    ///   the per-phase capacitance. A closed three-phase delta bank also writes
    ///   its wye equivalent, three times the leg value, on every step (pinned by
    ///   `elements::pd::capacitor::tests::make_pos_sequence_delta_cuf_bank_keeps_its_power`).
    /// - 3 (CMatrix, two or more phases): the positive-sequence self term
    ///   [`pos_seq_self_term`] of the nodal matrix goes to the array property
    ///   `Cuf` in its own µF units, the same value on every step, since every
    ///   energized step stamps the whole matrix (pinned by
    ///   `elements::pd::capacitor::tests::make_pos_sequence_cmatrix_applies_the_positive_sequence_cuf`
    ///   and `make_pos_sequence_cmatrix_multistep_bank_keeps_every_step`). A
    ///   bank the solve refuses, a delta bank of two or more phases or a matrix
    ///   of another order than its phases, is left as it is.
    ///
    /// Values are written at full `f64` precision.
    fn make_pos_sequence(&mut self, _ctx: &PosSeqCtx) -> PosSeqPlan {
        use super::prop::*;

        let nphases = self.cd.nphases;

        let actions = match self.spec_type {
            CapacitorSpecType::Kvar => {
                let phase_kv = if nphases > 1 || self.connection != 0 {
                    self.kvrating / sqrt3()
                } else {
                    self.kvrating
                };
                // do caps like a load: divide the total kvar equally among
                // 3 phases, per step.
                let nsteps = self.fnumsteps.max(0) as usize;
                let new_kvars: Vec<Option<f64>> = (0..nsteps)
                    .map(|i| Some(self.fkvarrating[i] / 3.0))
                    .collect();
                vec![
                    PosSeqAction::BeginEdit,
                    PosSeqAction::SetI32(PHASES, 1),
                    PosSeqAction::SetF64(KV, phase_kv),
                    PosSeqAction::SetStructF64s(KVAR, new_kvars),
                    PosSeqAction::EndEdit,
                ]
            }
            CapacitorSpecType::Cuf if self.is_closed_delta() => {
                // Each leg's capacitance is a third of its wye equivalent.
                let new_cuf: Vec<Option<f64>> = self
                    .fc
                    .iter()
                    .take(self.n_steps())
                    .map(|&c| Some(3.0 * c / CUF_SCALE))
                    .collect();
                vec![
                    PosSeqAction::BeginEdit,
                    PosSeqAction::SetI32(PHASES, 1),
                    PosSeqAction::SetStructF64s(CUF, new_cuf),
                    PosSeqAction::EndEdit,
                ]
            }
            CapacitorSpecType::Cuf => {
                // Bare single-set edit (no BeginEdit/EndEdit).
                vec![PosSeqAction::SetI32(PHASES, 1)]
            }
            CapacitorSpecType::CMatrix if nphases > 1 && self.refusal().is_none() => {
                // `Cmatrix` is stored in farads, `Cuf` is written in µF. With
                // no refusal it holds `nphases²` entries.
                let cmat = self.cmatrix.as_deref().unwrap_or_default();
                let c1 = pos_seq_self_term(cmat, nphases) / CUF_SCALE;
                let nsteps = self.fnumsteps.max(1) as usize;
                vec![
                    PosSeqAction::BeginEdit,
                    PosSeqAction::SetI32(PHASES, 1),
                    PosSeqAction::SetStructF64s(CUF, vec![Some(c1); nsteps]),
                    PosSeqAction::EndEdit,
                ]
            }
            CapacitorSpecType::CMatrix => Vec::new(),
        };

        PosSeqPlan::with_actions(actions)
    }
}
