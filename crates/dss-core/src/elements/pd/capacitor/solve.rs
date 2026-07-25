//! Impedance/admittance numerics (`RecalcElementData`, `MakeYprimWork`,
//! `CalcYPrim`) and the `impl CktElement` for [`Capacitor`].

use num_complex::Complex64;

use super::Capacitor;
use crate::elements::ckt::CktElementData;
use crate::elements::pos_seq::{PosSeqAction, PosSeqCtx, PosSeqPlan};
use crate::elements::traits::{CktElement, ReliabilityData, SysCtx};
use crate::support::cmatrix::{CMatrix, StampBl};
use crate::util::sqrt3;

impl Capacitor {
    /// Pascal `RecalcElementData`: derive `FC`/`FTotalkvar` from the spec, run
    /// the optional harmonic-filter recomputation, and (unless overridden) the
    /// default Norm/Emerg current ratings.
    pub(super) fn recalc(&mut self) {
        let two_pi = 2.0 * std::f64::consts::PI;
        let w = two_pi * self.cd.base_frequency;
        let nphases = self.cd.nphases as f64;
        let n = self.n_steps();
        self.ftotalkvar = 0.0;
        let mut phase_kv = 1.0;

        match self.spec_type {
            1 => {
                // kvar
                phase_kv = self.phase_kv();
                let fc = 1.0 / (w * phase_kv * phase_kv * 1000.0 / (self.fkvarrating[0] / nphases));
                for v in self.fc.iter_mut() {
                    *v = fc;
                }
                for &k in self.fkvarrating.iter().take(n) {
                    self.ftotalkvar += k;
                }
            }
            2 => {
                // Cuf
                phase_kv = self.phase_kv();
                for &c in self.fc.iter().take(n) {
                    self.ftotalkvar += w * c * phase_kv * phase_kv / 1000.0;
                }
            }
            _ => {} // CMatrix: nothing to do
        }

        if self.do_harmonic_recalc {
            for i in 0..n {
                self.fxl[i] = if self.fharm[i] != 0.0 {
                    (1.0 / (w * self.fc[i])) / (self.fharm[i] * self.fharm[i])
                } else {
                    0.0 // 0 harmonic means no filter
                };
                if self.fr[i] == 0.0 {
                    self.fr[i] = self.fxl[i] / 1000.0;
                }
            }
        }

        let kvar_per_phase = self.ftotalkvar / nphases;
        if !self.norm_amps_specified {
            self.norm_amps = kvar_per_phase / phase_kv * 1.35;
        }
        if !self.emerg_amps_specified {
            self.emerg_amps = kvar_per_phase / phase_kv * 1.8;
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

    /// Pascal `MakeYprimWork`: build one energized step's admittance into
    /// `ywork`. The matrix is *reused across steps without clearing* (faithful to
    /// the Pascal: wye/cmatrix overwrite their positions; delta accumulates).
    fn make_yprim_work(&self, ywork: &mut CMatrix, istep: usize, freq: f64) {
        let two_pi = 2.0 * std::f64::consts::PI;
        let freq_multiple = freq / self.cd.base_frequency;
        let w = two_pi * freq;
        let i_step = istep - 1;
        let nphases = self.cd.nphases;
        let nconds = self.cd.nconds;

        let has_zl = (self.fr[i_step] + self.fxl[i_step].abs()) > 0.0;
        let zl = Complex64::new(self.fr[i_step], self.fxl[i_step] * freq_multiple);

        match self.spec_type {
            1 | 2 => {
                let mut value = Complex64::new(0.0, self.fc[i_step] * w);
                if self.connection == 1 {
                    // Delta (line-line); AddElement accumulates.
                    ywork.stamp_delta_series(nphases, nconds, value);
                } else {
                    // Wye; assignment overwrites.
                    if has_zl {
                        value = (zl + value.inv()).inv(); // add in ZL
                    }
                    ywork.stamp_two_terminal_diag(nphases, nphases, value);
                }
            }
            _ => {
                // CMatrix.
                let cm = self.cmatrix.as_ref().expect("SpecType 3 has a CMatrix");
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
            1 | 2 => {
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
            _ => {
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

    fn recalc_element_data(&mut self, _sys: &SysCtx) {
        self.recalc();
    }

    fn norm_amps(&self) -> f64 {
        self.norm_amps
    }
    fn emerg_amps(&self) -> f64 {
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

    /// Pascal `TCapacitorObj.MakePosSequence` (Capacitor.pas:768-819). Collapse
    /// a capacitor bank to its positive-sequence single-phase form (done for
    /// 1-phase too). By `SpecType`:
    /// - 1 (kvar): kV per the connection/phase rule, per-step kvar/3, `Phases:=1`.
    /// - 2 (Cuf): a *bare* `Phases := 1` — a single Set with no surrounding
    ///   `BeginEdit`/`EndEdit` (the applier auto-brackets it).
    /// - 3 (CMatrix, only when multi-phase): average the self/mutual of `CMatrix`
    ///   into `Cuf` (the mutual loop includes the 2..N diagonals, as in Pascal).
    fn make_pos_sequence(&mut self, _ctx: &PosSeqCtx) -> PosSeqPlan {
        use super::prop::*;

        let nphases = self.cd.nphases;

        let actions = match self.spec_type {
            1 => {
                // kvar
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
            2 => {
                // Bare single-set edit (no BeginEdit/EndEdit).
                vec![PosSeqAction::SetI32(PHASES, 1)]
            }
            3 => {
                if nphases > 1 {
                    // C Matrix: average self/mutual → Cuf.
                    let cmat = self.cmatrix.as_deref().expect("SpecType 3 CMatrix");
                    let np = nphases;
                    let npf = np as f64;
                    let mut cs = 0.0; // Avg Self
                    for i in 0..np {
                        cs += cmat[i * np + i];
                    }
                    cs /= npf;
                    let mut cm = 0.0; // Avg mutual (2..N diagonals included)
                    for i0 in 1..np {
                        for j0 in i0..np {
                            cm += cmat[i0 * np + j0];
                        }
                    }
                    cm /= npf * (npf - 1.0) / 2.0;
                    vec![
                        PosSeqAction::BeginEdit,
                        PosSeqAction::SetI32(PHASES, 1),
                        // TODO(compat): Pascal `SetDouble(ord(TProp.Cuf), Cs - Cm)`
                        // aims a scalar `SetObjDouble` at `Cuf`, which is a
                        // `DoubleArrayProperty`. Upstream `SetObjDouble`'s trailing
                        // `case PropertyType` writes only the scalar double types, so
                        // the array `Cuf` is silently NOT written (oracle-verified:
                        // `cuf` unchanged across `makeposseq`); only the seq-mark +
                        // Begin/End side effects run. We emit the action to preserve
                        // that side-effect shape, and the applier's `set_obj_double`
                        // mirrors the fall-through by skipping the write for non-scalar
                        // types (see `obj/props/setters.rs:147-160`). Clean fix once
                        // the 1:1 port is done: drop this discarded write entirely.
                        PosSeqAction::SetF64(CUF, cs - cm),
                        PosSeqAction::EndEdit,
                    ]
                } else {
                    Vec::new()
                }
            }
            _ => Vec::new(),
        };

        PosSeqPlan::with_actions(actions)
    }
}
