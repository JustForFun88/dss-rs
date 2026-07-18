//! `RecalcElementData` (Zbase / %R↔G), `CalcYPrim` (pure-conductance stamping),
//! and the `impl CktElement` for [`GicTransformer`].

use num_complex::Complex64;

use super::{GicTransformer, SPEC_AUTO, SPEC_GSU, SPEC_YY};
use crate::elements::ckt::CktElementData;
use crate::elements::pos_seq::{PosSeqAction, PosSeqCtx, PosSeqPlan};
use crate::elements::traits::{CktElement, ReliabilityData, SysCtx};
use crate::support::cmatrix::CMatrix;

impl GicTransformer {
    /// Pascal `TGICTransformerObj.RecalcElementData` (GICTransformer.pas:433).
    pub(super) fn recalc(&mut self) {
        self.z_base1 = self.kv1 * self.kv1 / self.mva_rating;
        self.z_base2 = self.kv2 * self.kv2 / self.mva_rating;

        if self.pct_r_specified {
            self.g1 = 100.0 / (self.z_base1 * self.pct_r1);
            // TODO(compat): Pascal uses `FPctR1` here, NOT `FPctR2`
            // (GICTransformer.pas:441 — `G2 := 100.0 / (FZBase2 * FPctR1)`), so
            // both conductances scale off %R1. Reproduced 1:1; the clean fix
            // would read FPctR2 for G2.
            self.g2 = 100.0 / (self.z_base2 * self.pct_r1);
        } else {
            self.pct_r1 = 100.0 / (self.z_base1 * self.g1);
            self.pct_r2 = 100.0 / (self.z_base2 * self.g2);
        }
    }

    /// Pascal `TGICTransformerObj.WriteVarOutputRecord` (GICTransformer.pas:450):
    /// the per-GICTransformer record for `Export GICMvars` — the reactive (Mvar)
    /// demand the winding GIC drives, plus the per-phase GIC magnitude. Mutating:
    /// recomputes `Iterminal` (and, on the VarCurve path, hunts the curve cache).
    /// Returns `(GetBus(1), MVarMag, GICperPhase)`; the caller formats the row.
    pub(crate) fn var_output_record(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
    ) -> (String, f64, f64) {
        self.compute_iterminal(sys, node_v);
        let nphases = self.cd.nphases;
        // Curr := 0; for i := 1 to Fnphases do Curr += Iterminal[i].
        let mut curr = Complex64::ZERO;
        for i in 0..nphases {
            curr += self.cd.iterminal[i];
        }
        let gic_per_phase = curr.norm() / nphases as f64;
        let mvar_mag = if self.k_specified {
            self.k_factor * self.kv1 * gic_per_phase / 1000.0
        } else if let Some(vc) = self.var_curve.as_mut() {
            // MVA = sqrt(3) * kVLL * I/1000; pu A per phase (avg). Pascal
            // divides `GICperPhase` by `FMVArating*1000/FkV1/Sqrt3`, then reads
            // the VarCurve and scales by `FMVARating/Sqrt2`.
            let pu_curr_mag =
                gic_per_phase / (self.mva_rating * 1000.0 / self.kv1 / crate::util::sqrt3());
            vc.get_y_value(pu_curr_mag) * self.mva_rating / 2.0_f64.sqrt()
        } else {
            0.0
        };
        (self.cd.get_bus(1).to_string(), mvar_mag, gic_per_phase)
    }

    /// Stamp one 2-terminal conductance block `G` between the phases of
    /// consecutive terminals starting at 0-based conductor offset `base`
    /// (Pascal's `for i := base+1 to base+Fnphases` diagonal loop).
    fn stamp_block(yp: &mut CMatrix, base: usize, nphases: usize, g: f64) {
        let value = Complex64::new(g, 0.0);
        let value2 = -value;
        for i in base..base + nphases {
            let j = i + nphases;
            yp.set(i, i, value); // Elements are only on the diagonals
            yp.set(j, j, value);
            yp.set(i, j, value2);
            yp.set(j, i, value2);
        }
    }
}

impl CktElement for GicTransformer {
    fn cd(&self) -> &CktElementData {
        &self.cd
    }
    fn cd_mut(&mut self) -> &mut CktElementData {
        &mut self.cd
    }

    fn recalc_element_data(&mut self, _sys: &SysCtx) {
        self.recalc();
    }

    /// Pascal `TGICTransformerObj.MakePosSequence` (GICTransformer.pas:586-591):
    /// a multi-phase GICTransformer collapses to `Phases := 1` (a bare single
    /// edit), then `inherited` (the base bus rename).
    fn make_pos_sequence(&mut self, _ctx: &PosSeqCtx) -> PosSeqPlan {
        if self.cd.nphases > 1 {
            PosSeqPlan::with_actions(vec![PosSeqAction::SetI32(super::prop::PHASES, 1)])
        } else {
            PosSeqPlan::base()
        }
    }

    /// Pascal `TPDElement.CalcFltRate` (base): `Faultrate · pctperm · 0.01`.
    fn reliability_data(&self) -> ReliabilityData {
        ReliabilityData {
            branch_flt_rate: self.fault_rate * self.pct_perm * 0.01,
            hrs_to_repair: self.hrs_to_repair,
            miles_this_line: 0.0,
        }
    }

    fn norm_amps(&self) -> f64 {
        self.norm_amps
    }
    fn emerg_amps(&self) -> f64 {
        self.emerg_amps
    }

    /// Pascal `TPDElement.IsShunt`: this element is always shunt-connected.
    fn is_shunt(&self) -> bool {
        self.is_shunt
    }

    /// Pascal `TGICTransformerObj.CalcYPrim` (GICTransformer.pas:483): pure
    /// conductance blocks stamped into `YPrim_Shunt`; no frequency dependence.
    fn calc_yprim(&mut self, _sys: &SysCtx) {
        let yorder = self.cd.yorder;
        let nphases = self.cd.nphases;

        // Elements go into YPrim_Shunt (IsShunt is always true).
        let mut yp_temp = CMatrix::new(yorder);

        match self.spec_type {
            SPEC_GSU => {
                // One G1 block on terminals 1-2 (conductors 0..2N).
                Self::stamp_block(&mut yp_temp, 0, nphases, self.g1);
            }
            SPEC_AUTO | SPEC_YY => {
                // Terminals 1 and 2 (G1) + terminals 3 and 4 (G2).
                Self::stamp_block(&mut yp_temp, 0, nphases, self.g1);
                Self::stamp_block(&mut yp_temp, 2 * nphases, nphases, self.g2);
            }
            _ => {}
        }

        let mut yprim = CMatrix::new(yorder);
        yprim.copy_from(&yp_temp);

        // Mirror a tiny (1e-10) shunt diagonal into YPrim_Series so the
        // series-only zero-load snapshot (CalcVoltageBases) never sees an
        // all-zero row for a bus reachable ONLY through this shunt element
        // (e.g. a GICTransformer's X-side bus). This is the same guard the
        // Reactor applies (Reactor.pas:254) — but it is NOT in the Pascal
        // GICTransformer, which instead relies on KLU tolerating the singular
        // series Y; faer rejects it, so this reproduces KLU's tolerance while
        // staying invisible to the full-YPrim compare (the reported `yprim` is
        // the exact shunt matrix; the 1e-10 lives only in `yprim_series`,
        // which the oracle does not expose).
        let mut yp_series = CMatrix::new(yorder);
        for i in 0..yorder {
            yp_series.set(i, i, yp_temp.get(i, i) * 1.0e-10);
        }

        self.cd.yprim_shunt = Some(yp_temp);
        self.cd.yprim_series = Some(yp_series);
        self.cd.yprim = Some(yprim);

        // Account for open conductors.
        self.cd.apply_yprim_open_conductor_calcs();
        self.cd.yprim_invalid = false;
    }
}
