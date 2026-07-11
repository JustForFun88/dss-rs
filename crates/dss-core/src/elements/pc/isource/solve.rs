//! `CalcYPrim` (all-zero), the loadshape/harmonic magnitude dispatch
//! (`GetBaseCurr`/`CalcDaily/Duty/YearlyMult`), the injection path
//! (`GetInjCurrents`), and the `impl CktElement` for [`Isource`].

use num_complex::Complex64;

use super::Isource;
use crate::elements::ckt::CktElementData;
use crate::elements::general::spectrum::SpectrumObj;
use crate::elements::pos_seq::{PosSeqAction, PosSeqCtx, PosSeqPlan};
use crate::elements::traits::{CktElement, InjCtx, SysCtx};
use crate::solution::SolveMode;
use crate::support::cmatrix::CMatrix;
use crate::support::complexutil::{pdeg_to_complex, rotate_phasor_deg};
use crate::util::EPSILON2;

impl Isource {
    /// Pascal `TIsourceObj.RecalcElementData`: reallocate `InjCurrent`.
    pub(super) fn recalc(&mut self) {
        self.cd.inj_current = vec![Complex64::ZERO; self.cd.yorder];
    }

    /// Pascal `CalcDailyMult`: `ShapeFactor`/`ShapeIsActual` from the daily
    /// shape; the default `(PerUnit, 0)` reproduces the normal magnitude.
    fn calc_daily_mult(&mut self, hr: f64) {
        if let Some(s) = self.daily_shape_obj.as_mut() {
            self.shape_factor = s.get_mult_at_hour(hr);
            self.shape_is_actual = s.use_actual();
        } else {
            self.shape_factor = Complex64::new(self.per_unit, 0.0);
        }
    }

    /// Pascal `CalcDutyMult`: falls back to the daily shape when no duty shape.
    fn calc_duty_mult(&mut self, hr: f64) {
        if let Some(s) = self.duty_shape_obj.as_mut() {
            self.shape_factor = s.get_mult_at_hour(hr);
            self.shape_is_actual = s.use_actual();
        } else {
            self.calc_daily_mult(hr);
        }
    }

    /// Pascal `CalcYearlyMult` (the yearly curve is assumed hourly).
    fn calc_yearly_mult(&mut self, hr: f64) {
        if let Some(s) = self.yearly_shape_obj.as_mut() {
            self.shape_factor = s.get_mult_at_hour(hr);
            self.shape_is_actual = s.use_actual();
        } else {
            self.shape_factor = Complex64::new(self.per_unit, 0.0);
        }
    }

    /// Pascal `TIsourceObj.GetBaseCurr` (`Isource.pas:370`): the per-phase-1
    /// base current, applying the harmonic spectrum or the loadshape-mode
    /// magnitude, gated by the source/solution frequency match. (The Pascal
    /// body is wrapped in a `try/except` that logs error 334 on any exception;
    /// nothing in this arithmetic can raise in Rust, so it is not replicated —
    /// same convention as the rest of this port's numeric paths.)
    fn get_base_curr(&mut self, sys: &SysCtx) -> Complex64 {
        if sys.is_harmonic_model {
            let src_harmonic = sys.frequency / self.src_frequency;
            let mult = self
                .spectrum_obj
                .as_ref()
                .map(|s| s.get_mult(src_harmonic))
                .unwrap_or(Complex64::ZERO);
            let mut result = mult * self.amps; // base current for this harmonic
            result = rotate_phasor_deg(result, src_harmonic, self.angle);
            return result;
        }

        match sys.mode {
            // Uses the same logic as Load.
            SolveMode::Daily => self.calc_daily_mult(sys.dbl_hour),
            SolveMode::Yearly => self.calc_yearly_mult(sys.dbl_hour),
            SolveMode::DutyCycle => self.calc_duty_mult(sys.dbl_hour),
            SolveMode::Dynamic => {
                // Pascal dispatches on `ActiveCircuit.ActiveLoadShapeClass`
                // (USEDAILY/USEYEARLY/USEDUTY); that class isn't tracked yet
                // (always the `else` / USENONE case — same assumption as
                // Load/Generator/Storage/PVSystem), so ShapeFactor is always
                // explicitly reset to 1+j0 here (unlike Load's Dynamic arm,
                // Isource's Pascal `else` branch runs unconditionally).
                self.shape_factor = Complex64::new(1.0, 0.0);
            }
            _ => {}
        }

        let loadshape_mode = matches!(
            sys.mode,
            SolveMode::Daily | SolveMode::Yearly | SolveMode::DutyCycle | SolveMode::Dynamic
        );
        let namps = if loadshape_mode {
            self.amps * self.shape_factor.re
        } else {
            self.amps
        };

        if (sys.frequency - self.src_frequency).abs() < EPSILON2 {
            pdeg_to_complex(namps, self.angle)
        } else {
            Complex64::ZERO
        }
    }

    /// Pascal `TIsourceObj.GetInjCurrents` (`Isource.pas:463`): fill the
    /// per-terminal injection array — `+BaseCurr` on terminal 1, `-BaseCurr`
    /// on terminal 2, each phase after the first rotated in place by the
    /// scan-type (harmonic) or sequence-type (fundamental) rule.
    fn compute_inj_currents(&mut self, sys: &SysCtx) -> Vec<Complex64> {
        let nphases = self.cd.nphases;
        let mut base_curr = self.get_base_curr(sys); // applies spectrum if needed
        let mut curr = vec![Complex64::ZERO; self.cd.yorder];
        for i in 0..nphases {
            curr[i] = base_curr;
            curr[i + nphases] = -base_curr; // 2nd terminal
            if i < nphases - 1 {
                if sys.is_harmonic_model {
                    base_curr = match self.scan_type {
                        1 => rotate_phasor_deg(base_curr, 1.0, -self.phase_shift), // maintain pos seq
                        0 => base_curr, // zero seq: no rotation
                        _ => rotate_phasor_deg(
                            base_curr,
                            sys.frequency / sys.fundamental, // Solution.Harmonic
                            -self.phase_shift,
                        ),
                    };
                } else {
                    base_curr = match self.sequence_type {
                        -1 => rotate_phasor_deg(base_curr, 1.0, self.phase_shift), // neg seq
                        0 => base_curr, // zero seq: no rotation
                        _ => rotate_phasor_deg(base_curr, 1.0, -self.phase_shift), // maintain pos seq
                    };
                }
            }
        }
        curr
    }
}

impl CktElement for Isource {
    fn cd(&self) -> &CktElementData {
        &self.cd
    }
    fn cd_mut(&mut self) -> &mut CktElementData {
        &mut self.cd
    }

    fn recalc_element_data(&mut self, _sys: &SysCtx) {
        self.recalc();
    }

    /// Pascal `TIsourceObj.MakePosSequence` (Isource.pas:500-505): a multi-phase
    /// Isource collapses to `Phases := 1` (a bare single edit), then `inherited`
    /// (the base bus rename).
    fn make_pos_sequence(&mut self, _ctx: &PosSeqCtx) -> PosSeqPlan {
        if self.cd.nphases > 1 {
            PosSeqPlan::with_actions(vec![PosSeqAction::SetI32(super::prop::PHASES, 1)])
        } else {
            PosSeqPlan::base()
        }
    }

    /// Pascal `TIsourceObj.CalcYPrim`: build only YPrim_Series, left all-zero
    /// (ideal current source — no self-impedance), then the standard
    /// open-conductor fold.
    fn calc_yprim(&mut self, _sys: &SysCtx) {
        let yorder = self.cd.yorder;
        let yp_series = CMatrix::new(yorder);
        let mut yprim = CMatrix::new(yorder);
        yprim.copy_from(&yp_series);
        self.cd.yprim_series = Some(yp_series);
        self.cd.yprim_shunt = None;
        self.cd.yprim = Some(yprim);

        self.cd.apply_yprim_open_conductor_calcs();
        self.cd.yprim_invalid = false;
    }

    /// Pascal `TIsourceObj.InjCurrents` + `TPCElement.InjCurrents`.
    fn inj_currents(&mut self, sys: &SysCtx, ctx: &mut InjCtx) {
        self.cd.inj_current = self.compute_inj_currents(sys);
        for i in 0..self.cd.yorder {
            ctx.currents[self.cd.node_ref[i]] += self.cd.inj_current[i];
        }
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

    /// Pascal `GetSourceFrequency` (Isource branch): the source's own
    /// `SrcFrequency`.
    fn source_frequency(&self) -> Option<f64> {
        Some(self.src_frequency)
    }

    /// Pascal `TIsourceObj.GetCurrents`: `Curr[i] = -ComplexBuffer[i]` — the
    /// negated injection into a local scratch (YPrim is zero, so there is no
    /// `Yprim·V` term; `node_v` is unused, matching the Pascal signature that
    /// never reads it either).
    fn get_currents(&mut self, sys: &SysCtx, _node_v: &[Complex64], curr: &mut [Complex64]) {
        let inj = self.compute_inj_currents(sys); // present value of inj currents
        for i in 0..self.cd.yorder {
            curr[i] = -inj[i];
        }
    }
}
