//! The `Cond=`-keyed edit state machine: conductor (re)allocation, the
//! `ChangeLineConstantsType` engine swap, `spacing=` copy-in, `SetWires`, and
//! the per-conductor ampacity defaulting.

use crate::elements::general::conductor_data::{CnDataObj, TsDataObj, WireDataObj};
use crate::elements::general::line_spacing::LineSpacingObj;
use crate::obj::base::{DssObject, ObjectRefArrayItem};
use crate::support::line_constants::LineConstants;

use super::{ConductorChoice, LineGeometryObj, prop};

impl LineGeometryObj {
    /// Pascal `nconds` side effect: free the old data and re-init every
    /// per-conductor array to its default. (Pascal reallocates and then runs
    /// loops that unconditionally reset `FX=0`, `FY=0`, `FUnits=-1`,
    /// `FPhaseChoice=Overhead`, `FWireData=NIL` for all conductors — so the net
    /// effect is a full reset regardless of whether the count actually changed;
    /// `FActiveCond` resets to 1 and `FLastUnit` to ft.)
    pub(super) fn realloc_conductors(&mut self) {
        let n = self.fnconds.max(0) as usize;
        self.fphase_choice = vec![ConductorChoice::Overhead; n];
        self.fwiredata = (0..n).map(|_| None).collect();
        self.fx = vec![0.0; n];
        self.fy = vec![0.0; n];
        self.funits = vec![-1; n];
        self.flast_unit = super::UNITS_FT;
        self.factive_cond = 1;
        // Pascal frees the old `FLineData` and rebuilds it via the per-conductor
        // `ChangeLineConstantsType(Overhead)` loop — net effect a fresh overhead
        // engine sized `FNConds` (and NIL when there are no conductors).
        self.fline_data = (n >= 1).then(|| LineConstants::new(n));
    }

    /// Pascal `ChangeLineConstantsType`: select the conductor model for the
    /// active conductor and (re)allocate the `FLineData` engine when the kind or
    /// conductor count requires it, preserving `Nphases`/`RhoEarth` across the
    /// swap.
    pub(super) fn change_line_constants_type(&mut self, new_choice: ConductorChoice) {
        let n = self.fnconds.max(0) as usize;
        // Pascal `needNew`: TRUE when the active conductor's choice changed, OR
        // the engine is NIL / sized for a different conductor count. Pascal's
        // `if … else if …` (both arms set TRUE) is a boolean OR — the second
        // clause is *not* skipped when the active conductor's choice is unchanged,
        // so the engine self-heals if it ever falls out of sync with `FNConds`.
        let need_new = self
            .active_index()
            .is_some_and(|a| new_choice != self.fphase_choice[a])
            || self
                .fline_data
                .as_ref()
                .is_none_or(|ld| ld.num_conductors() != n);
        if need_new {
            // Pascal's `case` allocates only for the three concrete kinds; an
            // `Unknown` request leaves `FLineData` untouched (never reached in
            // practice — the callers always pass a concrete choice).
            let fresh = match new_choice {
                ConductorChoice::Overhead => Some(LineConstants::new(n)),
                ConductorChoice::ConcentricNeutral => Some(LineConstants::new_cn(n)),
                ConductorChoice::TapeShield => Some(LineConstants::new_ts(n)),
                ConductorChoice::Unknown => None,
            };
            if let Some(mut ld) = fresh {
                if let Some(old) = &self.fline_data {
                    ld.set_nphases(old.nphases());
                    ld.set_rho_earth(old.rho_earth());
                }
                self.fline_data = Some(ld);
            }
        }
        if let Some(a) = self.active_index() {
            self.fphase_choice[a] = new_choice;
        }
    }

    /// Pascal `spacing=` side effect: when the spacing's wire count matches,
    /// copy its coordinates/units into every conductor and clear the `X`/`H`
    /// "set" marks; otherwise log error 10103.
    pub(super) fn apply_spacing(&mut self) {
        let Some(spc) = self
            .line_spacing_obj
            .as_ref()
            .and_then(|b| b.as_any().downcast_ref::<LineSpacingObj>())
        else {
            return;
        };
        if self.fnconds == spc.nwires() {
            let units = spc.spacing_units();
            self.flast_unit = units;
            // dss_capi 0.15.x: an equivalent-spacing spacing copies its four
            // distances (not per-conductor coordinates); a detailed spacing
            // copies the coordinates as before.
            self.equivalent_spacing = spc.equivalent_spacing();
            if self.equivalent_spacing {
                self.eq_dist_ph_ph = spc.eq_dist_ph_ph();
                self.eq_dist_ph_n = spc.eq_dist_ph_n();
                self.avg_phase_height = spc.avg_phase_height();
                self.avg_neutral_height = spc.avg_neutral_height();
            } else {
                let xs = spc.xcoord().to_vec();
                let hs = spc.ycoord().to_vec();
                let n = self.fnconds.max(0) as usize;
                for i in 0..n {
                    self.fx[i] = xs[i];
                    self.fy[i] = hs[i];
                    self.funits[i] = units;
                }
            }
            // Pascal clears PrpSequence[X]/[H] so SaveWrite emits the spacing,
            // not the (now spacing-derived) coordinates (NoPropertyTracking off).
            self.data.clear_seq(prop::X);
            self.data.clear_seq(prop::H);
        } else {
            let name = spc.data().name().to_string();
            self.data.push_error(format!(
                "LineSpacing object {name} has the wrong number of wires."
            ));
        }
    }

    /// Pascal `SetWires` (the text path — `AllowAllConductors` is JSON-only):
    /// validate the count against the conductor span and fill `FWireData`.
    pub(super) fn set_wires(&mut self, refs: &[ObjectRefArrayItem<'_>]) {
        let mut istart = 1usize;
        let istop = self.fnconds.max(0) as usize;
        if let Some(a) = self.active_index() {
            if self.fphase_choice[a] == ConductorChoice::Unknown {
                self.change_line_constants_type(ConductorChoice::Overhead);
            } else if self.fphase_choice[a] != ConductorChoice::Overhead {
                // buried neutral wires (phase conductors are cables)
                istart = (self.fnphases + 1).max(1) as usize;
            }
        }
        let expected = istop.saturating_sub(istart) + 1;
        if expected != refs.len() {
            let full = format!("LineGeometry.{}", self.data.name());
            self.data.push_error(format!(
                "{full}: Unexpected number ({}) of objects; expected {expected} objects.",
                refs.len()
            ));
            return;
        }
        for (k, i) in (istart..=istop).enumerate() {
            // A `none` slot (AllowNoneItem) stays NIL.
            self.fwiredata[i - 1] = refs[k].as_ref().map(|(_, _, o)| o.clone_box());
        }
        self.factive_cond = istop as i32;
    }

    /// Pascal `wire`/`cncable`/`tscable` side effect (active conductor) and the
    /// `wires`/`cncables`/`tscables` "traditional" branch (first conductor):
    /// default this geometry's ratings from the conductor's once, when unset.
    pub(super) fn default_amps_from(&mut self, cond_index: usize) {
        let Some((cnorm, cemerg, cnum, crat)) = self
            .fwiredata
            .get(cond_index)
            .and_then(|o| o.as_ref())
            .map(|o| conductor_amps(o.as_ref()))
        else {
            return;
        };
        if cnorm > 0.0 && self.norm_amps == 0.0 {
            self.norm_amps = cnorm;
        }
        if cemerg > 0.0 && self.emerg_amps == 0.0 {
            self.emerg_amps = cemerg;
        }
        if cnum > 1 && self.num_amp_ratings == 1 {
            self.num_amp_ratings = cnum;
        }
        if crat.len() > 1 && self.amp_ratings.len() == 1 {
            let n = self.num_amp_ratings.max(0) as usize;
            self.amp_ratings = crat.into_iter().take(n).collect();
        }
    }
}

/// `(NormAmps, EmergAmps, NumAmpRatings, AmpRatings)` of a snapshot-cloned
/// conductor, whichever concrete catalog type it is.
fn conductor_amps(o: &dyn DssObject) -> (f64, f64, i32, Vec<f64>) {
    let any = o.as_any();
    if let Some(w) = any.downcast_ref::<WireDataObj>() {
        let (n, e, c, r) = w.amps();
        (n, e, c, r.to_vec())
    } else if let Some(c) = any.downcast_ref::<CnDataObj>() {
        let (n, e, k, r) = c.amps();
        (n, e, k, r.to_vec())
    } else if let Some(t) = any.downcast_ref::<TsDataObj>() {
        let (n, e, k, r) = t.amps();
        (n, e, k, r.to_vec())
    } else {
        (0.0, 0.0, 1, Vec::new())
    }
}
