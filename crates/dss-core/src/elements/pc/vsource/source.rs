//! The source-voltage / injection path: `GetVterminalForSource`, the loadshape
//! multipliers (`CalcDaily/Duty/YearlyMult`), and `GetInjCurrents`.

use num_complex::Complex64;

use super::{VSource, get_vmag};
use crate::elements::traits::SysCtx;
use crate::solution::{SolveMode, USEDAILY, USEDUTY, USEYEARLY};
use crate::support::complexutil::{pdeg_to_complex, rotate_phasor_deg};
use crate::util::EPSILON2;

impl VSource {
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

    /// Pascal `GetVterminalForSource` (snapshot/non-harmonic path; the
    /// loadshape-driven time-series magnitude is applied in daily/yearly/duty
    /// modes and — via `Set LoadShapeClass=` — in dynamics).
    pub(super) fn get_vterminal_for_source(&mut self, sys: &SysCtx) {
        let nphases = self.cd.nphases;
        self.shape_is_actual = false;

        // Modify magnitude based on a LOADSHAPE if assigned (loadshape modes;
        // Pascal VSource.pas l.1023-1026 includes DYNAMICMODE).
        let loadshape_mode = matches!(
            sys.mode,
            SolveMode::Daily | SolveMode::Yearly | SolveMode::DutyCycle | SolveMode::Dynamic
        );
        match sys.mode {
            SolveMode::Daily => self.calc_daily_mult(sys.dbl_hour),
            SolveMode::Yearly => self.calc_yearly_mult(sys.dbl_hour),
            SolveMode::DutyCycle => self.calc_duty_mult(sys.dbl_hour),
            // Pascal VSource.pas l.1006-1020 (`DYNAMICMODE`): dynamics honors
            // `Set LoadShapeClass=` — the selected class drives `ShapeFactor`;
            // the default `USENONE` leaves `ShapeFactor := PerUnit`, which
            // reproduces the normal-case magnitude below.
            SolveMode::Dynamic => match sys.active_load_shape_class {
                USEDAILY => self.calc_daily_mult(sys.dbl_hour),
                USEYEARLY => self.calc_yearly_mult(sys.dbl_hour),
                USEDUTY => self.calc_duty_mult(sys.dbl_hour),
                _ => self.shape_factor = Complex64::new(self.per_unit, 0.0),
            },
            _ => {}
        }

        self.vmag = if loadshape_mode {
            if self.shape_is_actual {
                1000.0 * self.shape_factor.re // actual L-N voltage across source
            } else {
                get_vmag(self.kv_base, self.shape_factor.re, nphases)
            }
        } else {
            get_vmag(self.kv_base, self.per_unit, nphases)
        };

        if sys.is_harmonic_model {
            // Harmonic mode: the source voltage at this harmonic is the spectrum
            // multiplier applied to the fundamental magnitude, rotated by the
            // per-phase shift (`ScanType` controls positive/zero/normal rotation).
            let src_harmonic = sys.frequency / self.src_frequency;
            let mult = self
                .spectrum_obj
                .as_ref()
                .map(|s| s.get_mult(src_harmonic))
                .unwrap_or(Complex64::ZERO);
            let mut vharm = mult * self.vmag;
            vharm = rotate_phasor_deg(vharm, src_harmonic, self.angle); // phase 1 shift
            for i in 0..nphases {
                self.cd.vterminal[i] = vharm;
                self.cd.vterminal[i + nphases] = Complex64::ZERO;
                if i < nphases - 1 {
                    vharm = match self.scan_type {
                        1 => rotate_phasor_deg(vharm, 1.0, -360.0 / nphases as f64), // pos seq
                        0 => vharm,                                                  // zero seq
                        _ => rotate_phasor_deg(vharm, src_harmonic, -360.0 / nphases as f64),
                    };
                }
            }
            return;
        }

        if (sys.frequency - self.src_frequency).abs() > EPSILON2 {
            self.vmag = 0.0; // Solution Frequency and Source Frequency don't match!
        }
        for i in 0..nphases {
            let deg = match self.sequence_type {
                -1 => 360.0 + self.angle + (i as f64) * 360.0 / nphases as f64, // neg seq
                0 => 360.0 + self.angle, // all the same for zero sequence
                _ => 360.0 + self.angle - (i as f64) * 360.0 / nphases as f64,
            };
            self.cd.vterminal[i] = pdeg_to_complex(self.vmag, deg);
            self.cd.vterminal[i + nphases] = Complex64::ZERO;
        }
    }

    /// Pascal `GetInjCurrents`: `[Iinj1; Iinj2] = [Yprim]·[Vsource; 0]` — the
    /// solve path: fill `self.cd.inj_current` via [`Self::compute_inj_currents`].
    pub(super) fn get_inj_currents(&mut self, sys: &SysCtx) {
        self.cd.inj_current = self.compute_inj_currents(sys);
    }

    /// Pascal `GetInjCurrents`, **returning** the injection; `self.cd.inj_current`
    /// is left untouched so the reporting path stays side-effect-free (Pascal
    /// `TVsourceObj.GetCurrents` writes into the scratch `ComplexBuffer`, never
    /// `InjCurrent`).
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
