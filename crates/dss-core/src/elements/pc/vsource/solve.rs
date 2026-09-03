//! The sequence-impedance numerics (`RecalcElementData`), `CalcYPrim`, and the
//! `impl CktElement` for [`VSource`].

use num_complex::Complex64;

use super::{VSource, VsourceZSpec, get_vmag};
use crate::elements::ckt::CktElementData;
use crate::elements::general::spectrum::SpectrumObj;
use crate::elements::pos_seq::{PosSeqAction, PosSeqCtx, PosSeqPlan};
use crate::elements::traits::{CktElement, InjComputeCtx, SysCtx};
use crate::support::cmatrix::CMatrix;
use crate::util::{CALPHA, EPSILON, quad_solver, sqrt3};

impl VSource {
    /// Pascal `TVsourceObj.RecalcElementData`.
    pub(super) fn recalc(&mut self) {
        let nphases = self.cd.nphases;
        let mut z = CMatrix::new(nphases);

        let factor = if nphases == 1 { 1.0 } else { sqrt3() };

        // Pascal initializes Rs=0, Rm=0, Xs=0.1, Xm=0 before the case; every
        // branch overwrites all four before they reach the Z matrix, so the
        // bindings start uninitialized (`rs` is set twice on the Z-spec path,
        // once for Isc1 and once for the matrix, hence `mut`).
        let mut rs: f64;
        let rm: f64;
        let xs: f64;
        let xm: f64;

        // Calculate the short circuit impedance and make all other spec
        // types agree.
        match self.z_spec_type {
            VsourceZSpec::MvaSc | VsourceZSpec::Isc => {
                if self.z_spec_type == VsourceZSpec::MvaSc {
                    // MVAsc
                    self.x1 = self.kv_base.powi(2)
                        / self.mva_sc3
                        / (1.0 + 1.0 / self.x1r1.powi(2)).sqrt();
                    self.r1 = self.x1 / self.x1r1;
                    self.r2 = self.r1; // default Z2 = Z1
                    self.x2 = self.x1;
                    self.isc3 = self.mva_sc3 * 1000.0 / (sqrt3() * self.kv_base);
                    self.isc1 = self.mva_sc1 * 1000.0 / (factor * self.kv_base);
                } else {
                    // Isc
                    self.mva_sc3 = sqrt3() * self.kv_base * self.isc3 / 1000.0;
                    self.mva_sc1 = factor * self.kv_base * self.isc1 / 1000.0;
                    self.x1 = self.kv_base.powi(2)
                        / self.mva_sc3
                        / (1.0 + 1.0 / self.x1r1.powi(2)).sqrt();
                    self.r1 = self.x1 / self.x1r1;
                    self.r2 = self.r1;
                    self.x2 = self.x1;
                }

                // Compute R0, X0
                self.r0 = quad_solver(
                    1.0 + self.x0r0.powi(2),
                    4.0 * (self.r1 + self.x1 * self.x0r0),
                    4.0 * (self.r1 * self.r1 + self.x1 * self.x1)
                        - (3.0 * self.kv_base * 1000.0 / factor / self.isc1).powi(2),
                );
                // Pascal raises on NaN R0; the executive records the message.
                self.x0 = self.r0 * self.x0r0;

                // for Z matrix
                xs = (2.0 * self.x1 + self.x0) / 3.0;
                rs = (2.0 * self.r1 + self.r0) / 3.0;
                rm = (self.r0 - self.r1) / 3.0;
                xm = (self.x0 - self.x1) / 3.0;
            }
            VsourceZSpec::Ohms => {
                // Z1, Z2, Z0 specified.
                // Compute Z1, Z2, Z0 in ohms if Z1 is specified in pu.
                if self.pu_z1_specified {
                    self.r1 = self.pu_z1.re * self.z_base;
                    self.x1 = self.pu_z1.im * self.z_base;
                    self.r2 = self.pu_z2.re * self.z_base;
                    self.x2 = self.pu_z2.im * self.z_base;
                    self.r0 = self.pu_z0.re * self.z_base;
                    self.x0 = self.pu_z0.im * self.z_base;
                }
                // (R1 = X1 = 0 raises error 7340 in Pascal; executive checks.)

                // Compute equivalent Isc3, Isc1, MVAsc3, MVAsc1.
                self.isc3 =
                    self.kv_base * 1000.0 / sqrt3() / Complex64::new(self.r1, self.x1).norm();

                if nphases == 1 {
                    // Force Z0 and Z2 to be Z1 so Zs is same as Z1.
                    self.r0 = self.r1;
                    self.x0 = self.x1;
                    self.r2 = self.r1;
                    self.x2 = self.x1;
                }
                rs = (2.0 * self.r1 + self.r0) / 3.0;
                xs = (2.0 * self.x1 + self.x0) / 3.0;

                self.isc1 = self.kv_base * 1000.0 / factor / Complex64::new(rs, xs).norm();
                self.mva_sc3 = sqrt3() * self.kv_base * self.isc3 / 1000.0;
                self.mva_sc1 = factor * self.kv_base * self.isc1 / 1000.0;
                xm = xs - self.x1;

                rs = (2.0 * self.r1 + self.r0) / 3.0;
                rm = (self.r0 - self.r1) / 3.0;
            }
        }

        if (self.r1 == self.r2) && (self.x1 == self.x2) {
            // Symmetric matrix case.
            let zs = Complex64::new(rs, xs);
            let zm = Complex64::new(rm, xm);
            for i in 0..nphases {
                z.set(i, i, zs);
                for j in 0..i {
                    z.set(i, j, zm);
                    z.set(j, i, zm);
                }
            }
        } else {
            // Asymmetric matrix case where Z2 <> Z1.
            let z1 = Complex64::new(self.r1, self.x1);
            let z2 = Complex64::new(self.r2, self.x2);
            let z0 = Complex64::new(self.r0, self.x0);

            let value = (z2 + z1 + z0) / 3.0;
            for i in 0..nphases {
                z.set(i, i, value);
            }

            if nphases == 3 {
                // Calpha is 1∠−120; conjugate to agree with textbooks.
                let calpha1 = CALPHA.conj();
                let calpha2 = calpha1 * calpha1;
                let value2 = (calpha2 * z2 + calpha1 * z1 + z0) / 3.0;
                let value1 = (calpha2 * z1 + calpha1 * z2 + z0) / 3.0;
                // (0-based indices; Pascal sets the 1-based lower/upper triangle)
                z.set(1, 0, value1);
                z.set(2, 0, value2);
                z.set(2, 1, value1);
                z.set(0, 1, value2);
                z.set(0, 2, value1);
                z.set(1, 2, value2);
            }
        }

        // If not specified, compute a value for puZ1 for display.
        if !(self.pu_z1_specified || self.pu_z0_specified || self.pu_z2_specified)
            && self.z_base > 0.0
        {
            self.pu_z1 = Complex64::new(self.r1 / self.z_base, self.x1 / self.z_base);
            self.pu_z2 = Complex64::new(self.r2 / self.z_base, self.x2 / self.z_base);
            self.pu_z0 = Complex64::new(self.r0 / self.z_base, self.x0 / self.z_base);
        }

        self.vmag = get_vmag(self.kv_base, self.per_unit, nphases);

        self.z = Some(z);
        self.zinv = Some(CMatrix::new(nphases));
        self.cd.inj_current = vec![Complex64::ZERO; self.cd.yorder];
    }
}

impl CktElement for VSource {
    fn cd(&self) -> &CktElementData {
        &self.cd
    }
    fn cd_mut(&mut self) -> &mut CktElementData {
        &mut self.cd
    }

    /// Pascal `TVsourceObj.MakePosSequence` (`VSource.pas:1201`). Single phase,
    /// line-neutral base kV (`kVBase / SQRT3`), keeping the R1/X1 sequence
    /// impedance.
    fn make_pos_sequence(&mut self, _ctx: &PosSeqCtx) -> PosSeqPlan {
        use super::prop;

        let kv_new = self.kv_base / sqrt3();
        PosSeqPlan::with_actions(vec![
            PosSeqAction::BeginEdit,
            PosSeqAction::SetI32(prop::PHASES, 1),
            PosSeqAction::SetF64(prop::BASEKV, kv_new),
            PosSeqAction::SetF64(prop::R1, self.r1),
            PosSeqAction::SetF64(prop::X1, self.x1),
            PosSeqAction::EndEdit,
        ])
    }

    /// Pascal `TVsourceObj.CalcYPrim`: build only YPrim_Series.
    fn calc_yprim(&mut self, sys: &SysCtx) {
        let nphases = self.cd.nphases;
        let yorder = self.cd.yorder;

        self.cd.yprim_freq = sys.frequency;
        let freq_multiplier = self.cd.yprim_freq / self.cd.base_frequency;

        let z = self.z.as_ref().expect("recalc ran in the constructor");
        let mut zinv = CMatrix::new(nphases);

        if ((freq_multiplier - 1.0) < EPSILON) && self.is_quasi_ideal && !sys.is_harmonic_model {
            // Ideal source approximation: diagonal impedance matrix only.
            let value = self.pu_z_ideal * self.z_base; // convert to ohms
            for i in 0..nphases {
                zinv.set(i, i, value);
            }
        } else {
            // Normal Thevenin source: series RL adjusted for frequency.
            for i in 0..nphases {
                for j in 0..nphases {
                    let mut value = z.get(i, j);
                    value.im *= freq_multiplier;
                    zinv.set(i, j, value);
                }
            }
        }

        if zinv.invert().is_err() {
            // Pascal error 325: put in large series conductance.
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
                yp_series.set(i, j + nphases, -value);
                yp_series.set(i + nphases, j, -value);
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

    /// Pascal `TVsourceObj.InjCurrents` + `TPCElement.InjCurrents` (M3b compute
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

    /// Pascal `GetSourceFrequency` (Vsource branch): the source's `srcFrequency`.
    fn source_frequency(&self) -> Option<f64> {
        Some(self.src_frequency)
    }

    /// Pascal `TVsourceObj.GetCurrents`: `Yprim·V(node)` minus a freshly
    /// recomputed injection (into a local, mirroring Pascal's `ComplexBuffer`
    /// scratch — the solver's `InjCurrent` stays untouched).
    ///
    /// **NCIM swing arm** (r4133 `PCElements/VSource.pas` l.1194:
    /// `if ((Algorithm = NCIMSOLVE) and (NodeRef[1] = 1)) then
    /// CalcInjCurrAtBus(Curr, ActorID)`). NCIM holds the swing bus at the ideal
    /// EMF, so the `Yprim·V - Iinj` branch below cancels to ~0 there — measured
    /// on the corpus decks before RP3.13, `Export Currents`' `Vsource.SOURCE`
    /// `|I1|` read `3.24074e-05 A` (`modes/ncim/ncim_pq` and `ncim_pv_pq`) and
    /// `4.42577e-05 A` (`ncim_midi`) where live r4133 reports `90.0718`,
    /// `64.2127` and `124.964 A`. The physical current is the Kirchhoff sum at
    /// the source bus, which needs every element there and so cannot be built
    /// from inside one element: the solver stamps it into `Iterminal` once at
    /// the converged `NodeV`
    /// (`solution::solution::ncim::ncim_stamp_swing_source_currents`, the port of
    /// `CalcInjCurrAtBus`) and this arm echoes that stamp, so the element path
    /// and `Dss::snapshot_elements` (the corpus gate's reader) are one live
    /// state. The Pascal test is on the **first node reference** being the global
    /// slack node 1 — `NodeRef^[1] = 1` is 1-based indexing of `NodeRef`, i.e.
    /// `node_ref.first() == Some(&1)` here, not "node 1 of some terminal".
    ///
    /// One condition r4133 does not have is added: the stamp must be **this**
    /// element's and current for this `SolutionCount`
    /// ([`VSource::ncim_swing_stamped_at`]). r4133 needs no marker because it
    /// re-runs `CalcInjCurrAtBus` on every read — and pays for it: with two
    /// sources on the slack node each one's sum calls the other's
    /// `GetCurrents` (`VSource.pas` l.1158 excludes only the element itself,
    /// l.1149), so the pair recurses until the stack dies (measured
    /// 2026-09-03, RP3.13: the r4133 DLL overflows its stack on
    /// `export currents`). Here the solver stamps the one swing source; any
    /// other slack-node source, and any read with no stamp behind it (a solve
    /// that aborted before the stamp, or `Set algorithm=NCIM` typed after a
    /// normal `Solve` with no re-solve), falls through to the ordinary branch
    /// and reports its own `YPrim·V - Iinj` terminal current.
    #[allow(clippy::needless_range_loop)] // loop-for-loop Pascal port
    fn get_currents(&mut self, sys: &SysCtx, node_v: &[Complex64], curr: &mut [Complex64]) {
        if sys.ncim
            && self.cd.node_ref.first() == Some(&1)
            && self.ncim_swing_stamped_at == Some(sys.solution_count)
        {
            // r4133 zeroes `Curr[1..Yorder]` and writes conductors `1..NPhases`
            // (VSource.pas l.1110-1111, l.1135/l.1169); the stamp carries exactly
            // that vector, zeros in the tail included, so it is copied whole.
            curr.fill(Complex64::ZERO);
            let n = curr.len().min(self.cd.iterminal.len());
            curr[..n].copy_from_slice(&self.cd.iterminal[..n]);
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
