//! The source-voltage / injection path for [`GicLine`]:
//! `GetVterminalForSource` (zero-sequence default) and `GetInjCurrents`.

use num_complex::Complex64;

use super::GicLine;
use crate::elements::pc::source_seq::{ScanType, SequenceType};
use crate::elements::traits::SysCtx;
use crate::support::complexutil::{pdeg_to_complex, rotate_phasor_deg};
use crate::util::EPSILON2;

impl GicLine {
    /// Pascal `TGICLineObj.GetVterminalForSource` (GICLine.pas:524): fill
    /// `Vterminal` with the source phasors — zero sequence by default (all
    /// phases the same), harmonic branch when a spectrum is assigned.
    pub(super) fn get_vterminal_for_source(&mut self, sys: &SysCtx) {
        let nphases = self.cd.nphases;
        self.vmag = self.volts;

        if sys.is_harmonic_model && self.spectrum_obj.is_some() {
            let src_harmonic = sys.frequency / self.src_frequency;
            let mult = self
                .spectrum_obj
                .as_ref()
                .map(|s| s.get_mult(src_harmonic))
                .unwrap_or(Complex64::ZERO);
            let mut vharm = mult * self.vmag; // Base voltage for this harmonic
            vharm = rotate_phasor_deg(vharm, src_harmonic, self.angle); // phase 1 shift
            for i in 0..nphases {
                self.cd.vterminal[i] = vharm;
                self.cd.vterminal[i + nphases] = Complex64::ZERO;
                if i < nphases - 1 {
                    vharm = match self.scan_type {
                        // maintain pos seq
                        ScanType::Positive => {
                            rotate_phasor_deg(vharm, 1.0, -360.0 / nphases as f64)
                        }
                        // Do nothing for Zero Sequence; All the same
                        ScanType::Zero => vharm,
                        // normal rotation (Pascal's `else` arm)
                        ScanType::None => {
                            rotate_phasor_deg(vharm, src_harmonic, -360.0 / nphases as f64)
                        }
                    };
                }
            }
            return;
        }

        // Non-harmonic modes or no spectrum.
        if (sys.frequency - self.src_frequency).abs() > EPSILON2 {
            self.vmag = 0.0; // Solution Frequency and Source Frequency don't match!
        }
        for i in 0..nphases {
            // Pascal notes "Always 0 for GIC" (`GICLine.pas:563`): GICLine has no
            // `Sequence=` property, so only the zero-sequence arm is reachable.
            let deg = match self.sequence_type {
                SequenceType::Negative => 360.0 + self.angle + (i as f64) * 360.0 / nphases as f64,
                // all the same for zero sequence
                SequenceType::Zero => 360.0 + self.angle,
                // Pascal's `else` arm
                SequenceType::Positive => 360.0 + self.angle - (i as f64) * 360.0 / nphases as f64,
            };
            self.cd.vterminal[i] = pdeg_to_complex(self.vmag, deg);
            self.cd.vterminal[i + nphases] = Complex64::ZERO;
        }
    }

    /// Pascal `TGICLineObj.GetInjCurrents` (GICLine.pas:611): `[Yprim]·[Vsource;
    /// 0]` into `self.cd.inj_current` (the solve path).
    pub(super) fn get_inj_currents(&mut self, sys: &SysCtx) {
        self.cd.inj_current = self.compute_inj_currents(sys);
    }

    /// Pascal `GetInjCurrents`, **returning** the injection; `self.cd.inj_current`
    /// stays untouched so the reporting path (`GetCurrents`) is side-effect-free
    /// (Pascal writes into the `ComplexBuffer` scratch there, never `InjCurrent`).
    pub(super) fn compute_inj_currents(&mut self, sys: &SysCtx) -> Vec<Complex64> {
        self.get_vterminal_for_source(sys);
        let mut inj = vec![Complex64::ZERO; self.cd.yorder];
        if let Some(yprim) = &self.cd.yprim {
            yprim.mv_mult(&mut inj, &self.cd.vterminal);
        }
        self.cd.iterminal_updated = false;
        inj
    }
}
