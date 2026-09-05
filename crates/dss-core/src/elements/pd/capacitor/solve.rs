//! Impedance/admittance numerics (`RecalcElementData`, `MakeYprimWork`,
//! `CalcYPrim`) and the `impl CktElement` for [`Capacitor`].

use num_complex::Complex64;

use super::{CUF_SCALE, Capacitor, CapacitorSpecType};
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
            CapacitorSpecType::Kvar => {
                phase_kv = self.phase_kv();
                // `FC[i] := 1.0 / (w * SQR(PhasekV) * 1000.0 / (FkvarRating[1] /
                // Fnphases))` — `SQR` binds first, so the square is an atom:
                // `w * (kv*kv)`, not `(w*kv) * kv`. The two associations differ by
                // one ULP for many realistic (kV, kvar, f) triples and the result
                // is rendered verbatim as the `Cuf` property (pinned bit-exactly
                // against the oracle in `tests.rs`).
                let fc = 1.0 / (w * phase_kv.powi(2) * 1000.0 / (self.fkvarrating[0] / nphases));
                for v in self.fc.iter_mut() {
                    *v = fc;
                }
                for &k in self.fkvarrating.iter().take(n) {
                    self.ftotalkvar += k;
                }
            }
            CapacitorSpecType::Cuf => {
                phase_kv = self.phase_kv();
                for &c in self.fc.iter().take(n) {
                    // `Ftotalkvar + w * FC[i] * SQR(PhasekV) / 1000.0` — same
                    // `SQR`-binds-first rule; observable through the derived
                    // Norm/Emerg amps below.
                    self.ftotalkvar += w * c * phase_kv.powi(2) / 1000.0;
                }
            }
            // Pascal's `case` has no `3:` arm (`Capacitor.pas:623-660`): with a
            // CMatrix there is nothing to derive, and `PhasekV` keeps its 1.0
            // pre-case seeding — which the Norm/Emerg amps below then divide by.
            CapacitorSpecType::CMatrix => {}
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
            CapacitorSpecType::Kvar | CapacitorSpecType::Cuf => {
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
            CapacitorSpecType::CMatrix => {
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
    ///
    /// # The `Cuf` write of the `CMatrix` arm
    ///
    /// `Cuf` is a *double-array* property (per switched step;
    /// `.inputs/dss_capi/src/PDElements/Capacitor.pas:240`), and neither oracle
    /// revision actually applies the `Cs - Cm` it computes for it:
    ///
    /// * pinned dss_capi 0.14.5 aims the **scalar** setter at it —
    ///   `SetDouble(ord(TProp.Cuf), (Cs - Cm), [])` (`Capacitor.pas:814`) —
    ///   whose trailing `case PropertyType`
    ///   (`src/General/DSSObjectHelper.pas:2812-2834`) enumerates only the three
    ///   scalar double types and has no `else`, so the value is dropped with no
    ///   error while `SetDouble`'s success path (`:3050-3054`) still marks the
    ///   property sequence and runs the `Cuf` side effect `SpecType := 2`
    ///   (`Capacitor.pas:383-386`). The bank is then computed from whatever
    ///   `FC` happened to hold, and the `cmatrix` the user gave is switched out
    ///   of `MakeYprimWork` for good (`:926-978`; the matrix arm needs
    ///   `SpecType = 3`). Oracle-verified: `? Capacitor.…cuf` is unchanged
    ///   across `makeposseq`. The sibling `kvar` arm two branches up writes its
    ///   own array property correctly with `SetDoubles` (`:793`).
    /// * EPRI r4133 predates the typed setters and formats the value into a
    ///   command string instead — `S := S + Format(' Cuf=%-.5g', [(Cs - Cm)])`,
    ///   then one `Edit(ActorID)`
    ///   (`Version8/Source/PDElements/Capacitor.pas:829`, `:834-835`) — so it
    ///   *does* apply it, through `InterpretDblArray`
    ///   (`Version8/Source/Common/Utilities.pas:788-791`, "Fills array with
    ///   zeros if we run out of numbers"). But `Cmatrix` is already in farads
    ///   there (`:255`) while the `cuf` side effect multiplies the parsed array
    ///   by `1.0e-6` again (`:411`), so r4133 lands 4e-12 F where 4e-6 F was
    ///   meant — the same value, six orders of magnitude down.
    ///
    /// What both revisions *intend* is unambiguous — r4133 spells the array
    /// write out — so `GOLDEN_REBASE_PLAN.md` G2.5 performs it in both lanes
    /// (CLAUDE.md 2026-08-02: upstream bugs are never reproduced): the array
    /// write the parser would have made, in the property's own µF units, so the
    /// `SpecType := 2` the side effect sets is backed by the capacitance the
    /// reduction computed. The gated deck that sees it
    /// (`modes/makeposseq/makeposseq_shunt.dss`, `Capacitor.cap_cmat`) then
    /// diverges from the `capi_v0145` oracle across the whole post-`makeposseq`
    /// model; that is ledgered (`tests/corpus/ledger.json`
    /// `makeposseq-cuf-applied-capi`) and the correct value pinned by
    /// `elements::pd::capacitor::tests::make_pos_sequence_cmatrix_applies_the_positive_sequence_cuf`.
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
            CapacitorSpecType::Cuf => {
                // Bare single-set edit (no BeginEdit/EndEdit).
                vec![PosSeqAction::SetI32(PHASES, 1)]
            }
            CapacitorSpecType::CMatrix => {
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
                    // `Cs - Cm` is the positive-sequence capacitance, and it is
                    // written to the **array** property `Cuf` — element 1 the
                    // value, steps 2..N zeroed — in the property's own µF units
                    // (`PropertyScale = 1.0e-6`, `Capacitor.pas:243`, applied by
                    // the setter; `Cmatrix` is stored in farads, `:255`, so the
                    // difference is divided back out here). See the doc comment
                    // above for why this is the reading both oracle revisions
                    // meant and neither performs.
                    let nsteps = self.fnumsteps.max(1) as usize;
                    let mut new_cuf: Vec<Option<f64>> = vec![Some(0.0); nsteps];
                    new_cuf[0] = Some((cs - cm) / CUF_SCALE);
                    vec![
                        PosSeqAction::BeginEdit,
                        PosSeqAction::SetI32(PHASES, 1),
                        PosSeqAction::SetStructF64s(CUF, new_cuf),
                        PosSeqAction::EndEdit,
                    ]
                } else {
                    Vec::new()
                }
            }
        };

        PosSeqPlan::with_actions(actions)
    }
}
