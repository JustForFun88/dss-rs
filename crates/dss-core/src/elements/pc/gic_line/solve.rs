//! `RecalcElementData` (the series `Z` matrix), `CalcYPrim` (series RL adjusted
//! for frequency + the optional blocking capacitor), and the `impl CktElement`
//! for [`GicLine`].

use num_complex::Complex64;

use super::GicLine;
use crate::elements::ckt::CktElementData;
use crate::elements::general::spectrum::SpectrumObj;
use crate::elements::pos_seq::{PosSeqAction, PosSeqCtx, PosSeqPlan};
use crate::elements::traits::{CktElement, InjComputeCtx, SysCtx};
use crate::support::cmatrix::CMatrix;
use crate::util::EPSILON;

impl GicLine {
    /// Pascal `TGICLineObj.RecalcElementData` (GICLine.pas:407): the diagonal
    /// series `Z` (`Zs = R + jX`, off-diagonals zero), then the induced `Volts`
    /// (geodesy) unless explicitly specified.
    pub(super) fn recalc(&mut self) {
        let nphases = self.cd.nphases;
        let mut z = CMatrix::new(nphases);

        let zs = Complex64::new(self.r, self.x);
        let zm = Complex64::ZERO;
        for i in 0..nphases {
            z.set(i, i, zs);
            for j in 0..i {
                z.set(i, j, zm);
                z.set(j, i, zm);
            }
        }

        if !self.volts_specified {
            self.volts = self.compute_vline();
        }
        self.vmag = self.volts;

        self.z = Some(z);
        self.zinv = Some(CMatrix::new(nphases));
        self.cd.inj_current = vec![Complex64::ZERO; self.cd.yorder];
    }
}

impl CktElement for GicLine {
    fn cd(&self) -> &CktElementData {
        &self.cd
    }
    fn cd_mut(&mut self) -> &mut CktElementData {
        &mut self.cd
    }

    /// Pascal `TGICLineObj.MakePosSequence` (`GICLine.pas:660`). Single phase,
    /// keeping the `Volts`/`Angle`/`R`/`X` unchanged.
    fn make_pos_sequence(&mut self, _ctx: &PosSeqCtx) -> PosSeqPlan {
        use super::prop;

        PosSeqPlan::with_actions(vec![
            PosSeqAction::BeginEdit,
            PosSeqAction::SetI32(prop::PHASES, 1),
            PosSeqAction::SetF64(prop::VOLTS, self.volts),
            PosSeqAction::SetF64(prop::ANGLE, self.angle),
            PosSeqAction::SetF64(prop::R, self.r),
            PosSeqAction::SetF64(prop::X, self.x),
            PosSeqAction::EndEdit,
        ])
    }

    /// Pascal `TGICLineObj.CalcYPrim` (GICLine.pas:445): build only YPrim_Series
    /// — series `R + jX·FreqMultiplier`, plus the blocking capacitor `Xc =
    /// −1/(2π·f·C·1e-6)` on the diagonal when `C > 0`; invert and stamp the
    /// 2-terminal series block.
    fn calc_yprim(&mut self, sys: &SysCtx) {
        let nphases = self.cd.nphases;
        let yorder = self.cd.yorder;

        self.cd.yprim_freq = sys.frequency;
        let freq_multiplier = self.cd.yprim_freq / self.cd.base_frequency;

        let z = self.z.as_ref().expect("recalc ran in the constructor");
        let mut zinv = CMatrix::new(nphases);

        // Put in Series RL adjusted for frequency (im scaled by FreqMultiplier).
        for i in 0..nphases {
            for j in 0..nphases {
                let mut value = z.get(i, j);
                value.im *= freq_multiplier;
                zinv.set(i, j, value);
            }
        }

        // Add the blocking capacitor 1/(ωC) into the diagonals of Zinv.
        if self.c > 0.0 {
            let xc = -1.0 / (2.0 * std::f64::consts::PI * self.cd.yprim_freq * self.c * 1.0e-6);
            for i in 0..nphases {
                zinv.add(i, i, Complex64::new(0.0, xc));
            }
        }

        if zinv.invert().is_err() {
            // Pascal error 325: put in a large series conductance.
            zinv.clear();
            for i in 0..nphases {
                zinv.set(i, i, Complex64::new(1.0 / EPSILON, 0.0));
            }
        }

        let mut yp_series = CMatrix::new(yorder);
        for i in 0..nphases {
            for j in 0..nphases {
                let value = zinv.get(i, j);
                yp_series.set(i, j, value);
                yp_series.set(i + nphases, j + nphases, value);
                yp_series.set(i + nphases, j, -value);
                yp_series.set(j, i + nphases, -value);
            }
        }

        let mut yprim = CMatrix::new(yorder);
        yprim.copy_from(&yp_series);
        self.cd.yprim_series = Some(yp_series);
        self.cd.yprim_shunt = None;
        self.cd.yprim = Some(yprim);
        self.zinv = Some(zinv);

        // Account for open conductors.
        self.cd.apply_yprim_open_conductor_calcs();
        self.cd.yprim_invalid = false;
    }

    /// Pascal `TGICLineObj.InjCurrents` + `TPCElement.InjCurrents` (M3b compute
    /// half; the caller scatters `cd.inj_current`).
    fn compute_inj_currents(
        &mut self,
        sys: &SysCtx,
        _node_v: &[Complex64],
        _ctx: &mut InjComputeCtx,
    ) -> bool {
        self.get_inj_currents(sys);
        false
    }

    fn harmonic_spectrum(&self) -> Option<&SpectrumObj> {
        self.spectrum_obj.as_ref()
    }
    fn harmonic_spectrum_name(&self) -> Option<&str> {
        Some(&self.spectrum)
    }
    fn set_harmonic_spectrum(&mut self, spectrum: Option<SpectrumObj>) {
        self.spectrum_obj = spectrum;
    }

    /// Pascal `GetSourceFrequency` (GICLine branch): the source's `SrcFrequency`.
    fn source_frequency(&self) -> Option<f64> {
        Some(self.src_frequency)
    }

    /// Pascal `TGICLineObj.GetCurrents` (GICLine.pas:590): `Yprim·V(node)` minus
    /// a freshly recomputed injection (into a local scratch, mirroring Pascal's
    /// `ComplexBuffer` — `InjCurrent` stays untouched).
    #[allow(clippy::needless_range_loop)] // loop-for-loop Pascal port
    fn get_currents(&mut self, sys: &SysCtx, node_v: &[Complex64], curr: &mut [Complex64]) {
        // No node references yet: an element declared after the last
        // `SetNodeRef` sweep has an empty `node_ref` (the port's model of
        // r4133's `NodeRef := nil`, `Common/CktElement.pas:186`, allocated only
        // by `SetNodeRef` `:547-558`, which `ReProcessBusDefs` re-runs for
        // enabled elements at Y build). r4133 indexes the nil pointer right
        // below and lets the access violation surface out of this procedure's
        // own `TRY ... EXCEPT` as DSS error 327 -- `TGICLineObj.GetCurrents`,
        // `PCElements/GICLine.pas:635-663`; the port answers
        // with the zero vector the base trait's `get_currents` default returns
        // for an unmapped element. The `Enabled` test the other PC classes
        // inherit from `TPCElement.GetCurrents` is deliberately NOT added here:
        // this override carries none, so a disabled element goes on reporting
        // `YPrim*Vterminal - Iinj` from its last mapping, as upstream does.
        if self.cd.node_ref.is_empty() {
            curr.fill(Complex64::ZERO);
            return;
        }
        let yorder = self.cd.yorder;
        for i in 0..yorder {
            self.cd.vterminal[i] = node_v[self.cd.node_ref[i]];
        }
        if let Some(yprim) = &self.cd.yprim {
            yprim.mv_mult(curr, &self.cd.vterminal);
        }
        let inj = self.compute_inj_currents(sys); // overwrites Vterminal, like the original
        for i in 0..yorder {
            curr[i] -= inj[i];
        }
    }
}

#[cfg(test)]
mod pos_seq_tests {
    use super::*;
    use crate::elements::pc::gic_line::prop;
    use crate::elements::pos_seq::PosSeqCtx;

    /// GICLine → 1 phase, keeping Volts/Angle/R/X unchanged (WPG.21).
    #[test]
    fn makeposseq_gicline_keeps_all() {
        let mut g = GicLine::new("g");
        g.volts = 100.0;
        g.angle = 30.0;
        g.r = 2.5;
        g.x = 0.4;

        let plan = g.make_pos_sequence(&PosSeqCtx::default());
        assert!(plan.run_base);
        assert_eq!(
            plan.actions,
            vec![
                PosSeqAction::BeginEdit,
                PosSeqAction::SetI32(prop::PHASES, 1),
                PosSeqAction::SetF64(prop::VOLTS, 100.0),
                PosSeqAction::SetF64(prop::ANGLE, 30.0),
                PosSeqAction::SetF64(prop::R, 2.5),
                PosSeqAction::SetF64(prop::X, 0.4),
                PosSeqAction::EndEdit,
            ]
        );
    }
}
