//! The conductor-selection edit helpers: conductor (re)allocation, the
//! line-constants engine swap, `spacing=` copy-in, the plural `wires=`,
//! `cncables=` and `tscables=` fills,
//! and the per-conductor ampacity defaulting.

use std::ops::Range;

use crate::elements::general::conductor_data::{ConductorData, ConductorKind, ConductorObj};
use crate::obj::Rating;
use crate::obj::base::{DssObject, ObjectRefArrayItem};
use crate::support::line_constants::LineConstants;

use super::accessors::LENGTH_UNITS;
use super::{ConductorChoice, LineGeometryObj, UNIT_UNSET, UNITS_FT, prop};
use crate::support::line_units::LineUnits;

impl LineGeometryObj {
    /// The `nconds` side effect: reset every per-conductor array to its
    /// default (no conductor object, coordinates 0, no unit of its own,
    /// overhead) whether or not the count changed, drop the equivalent spacing
    /// a `spacing=` copied, select no conductor and build a fresh overhead
    /// engine sized `NConds` (none without conductors). The geometry then
    /// computes only from data written after it.
    /// The first allocation keeps the default unit, so a `units=` written
    /// before the first `nconds=` still applies. A re-allocation of a geometry
    /// that has conductors starts the default from feet again.
    /// Pinned by
    /// `exec::tests::line_geometry_rules::a_redefinition_drops_the_equivalent_spacing`.
    pub(super) fn realloc_conductors(&mut self) {
        if !self.funits.is_empty() {
            self.flast_unit = UNITS_FT;
        }
        self.equivalent_spacing = false;
        self.eq_dist_ph_ph = 0.0;
        self.eq_dist_ph_n = 0.0;
        self.avg_phase_height = 0.0;
        self.avg_neutral_height = 0.0;
        let n = self.fnconds.max(0) as usize;
        self.fphase_choice = vec![ConductorChoice::Overhead; n];
        self.fwiredata = (0..n).map(|_| None).collect();
        self.fx = vec![0.0; n];
        self.fy = vec![0.0; n];
        self.funits = vec![UNIT_UNSET; n];
        self.selected = None;
        self.last_filled = None;
        self.fline_data = (n >= 1).then(|| LineConstants::new(n));
    }

    /// Set the conductor model of conductor `slot` (0-based, `None` for no
    /// conductor) and (re)allocate the `FLineData` engine when the kind or the
    /// conductor count requires it, keeping `Nphases`/`RhoEarth` across the
    /// swap.
    pub(super) fn change_line_constants_type_at(
        &mut self,
        slot: Option<usize>,
        new_choice: ConductorChoice,
    ) {
        let n = self.fnconds.max(0) as usize;
        let slot = slot.filter(|&a| a < self.fphase_choice.len());
        // A new engine when the conductor's model changes, or when the engine
        // is missing or sized for another conductor count.
        let need_new = slot.is_some_and(|a| new_choice != self.fphase_choice[a])
            || self
                .fline_data
                .as_ref()
                .is_none_or(|ld| ld.num_conductors() != n);
        if need_new {
            // Only the three concrete kinds allocate. CN and TS both build the
            // merged cable engine. The per-conductor CN/TS type is assigned in
            // `update_line_geometry_data`.
            let fresh = match new_choice {
                ConductorChoice::Overhead => Some(LineConstants::new(n)),
                ConductorChoice::ConcentricNeutral | ConductorChoice::TapeShield => {
                    Some(LineConstants::new_cable(n))
                }
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
        if let Some(a) = slot {
            self.fphase_choice[a] = new_choice;
        }
    }

    /// The `spacing=` side effect: when the spacing's wire count matches,
    /// copy its coordinates/units into every conductor and clear the `X`/`H`
    /// "set" marks; otherwise log the wrong-count error. A spacing with no
    /// length unit (code 0, or a code outside 1 to 8) is refused and not kept.
    pub(super) fn apply_spacing(&mut self) {
        let Some(spc) = self.line_spacing_obj.as_ref() else {
            return;
        };
        if LineUnits::from_code(spc.spacing_units()) == LineUnits::None {
            let msg = format!(
                "LineGeometry.{}.Spacing: LineSpacing.{} has no length unit, so nothing is \
                 copied. Give the spacing one of {LENGTH_UNITS}.",
                self.data.name(),
                spc.data().name()
            );
            self.line_spacing_obj = None;
            self.data.push_error(msg);
            return;
        }
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

    /// The plural `wires=` fill (text path). The list is the buried neutrals
    /// after the phase conductors when the selected conductor is a cable or,
    /// with none selected, when a phase conductor holds a cable, and every
    /// conductor otherwise. The selection is left alone.
    pub(super) fn set_wires(&mut self, refs: &[ObjectRefArrayItem<'_>]) {
        let n = self.fnconds.max(0) as usize;
        let mut start = 0usize;
        match self.active_index() {
            Some(a) => {
                if self.fphase_choice[a] == ConductorChoice::Unknown {
                    self.change_line_constants_type_at(Some(a), ConductorChoice::Overhead);
                } else if self.fphase_choice[a] != ConductorChoice::Overhead {
                    start = self.phase_positions();
                }
            }
            None if self.phases_hold_a_cable() => start = self.phase_positions(),
            None => {}
        }
        self.fill(
            start..n,
            refs,
            "the phases hold cables and no conductor is a neutral",
        );
    }

    /// The plural `cncables=`/`tscables=` fill (text path): one cable per
    /// phase, written into the phase positions whichever conductor is
    /// selected. The selection is left alone.
    pub(super) fn set_cables(&mut self, refs: &[ObjectRefArrayItem<'_>]) {
        let phases = self.phase_positions();
        self.fill(0..phases, refs, "the geometry has no phases");
    }

    /// Fill `slots` from `refs`, one object per slot, or refuse a list of
    /// another length and fill nothing. `empty` says why a list has no slot to
    /// fill. The last slot written is kept in `last_filled` for the plural side
    /// effects (`None` when the list is refused).
    fn fill(&mut self, slots: Range<usize>, refs: &[ObjectRefArrayItem<'_>], empty: &str) {
        self.last_filled = None;
        let expected = slots.len();
        if expected != refs.len() {
            let why = match (expected, self.fnconds) {
                (0, ..=0) => ": the geometry has no conductors. Set NConds first".to_string(),
                (0, _) => format!(": {empty}"),
                _ => String::new(),
            };
            self.data.push_error(format!(
                "LineGeometry.{}: Unexpected number ({}) of objects; expected {expected} objects{why}.",
                self.data.name(),
                refs.len()
            ));
            return;
        }
        for (i, r) in slots.clone().zip(refs) {
            // A `none` slot (AllowNoneItem) stays unset.
            self.fwiredata[i] = r
                .as_ref()
                .and_then(|(_, o)| ConductorObj::from_resolved(*o));
        }
        self.last_filled = (expected > 0).then(|| slots.end - 1);
    }

    /// The number of phase positions: `NPhases`, at most `NConds`. A geometry
    /// with fewer conductors than phases holds phases only.
    pub(super) fn phase_positions(&self) -> usize {
        self.fnphases.clamp(0, self.fnconds.max(0)) as usize
    }

    /// Whether a phase conductor holds a concentric-neutral or tape-shield
    /// cable.
    fn phases_hold_a_cable(&self) -> bool {
        self.fwiredata
            .iter()
            .take(self.phase_positions())
            .flatten()
            .any(|o| o.conductor_kind() != ConductorKind::Wire)
    }

    /// The `Conductors=` side effect: set each valid conductor's engine kind
    /// (the selection is left alone) and default the geometry ratings from the
    /// first valid conductor. An all-NIL list is rejected with #10103. Returns
    /// `true` when at least one conductor was valid (the caller flags
    /// `data_changed`; Pascal `Exit`s before the `dataChanged` block on rejection).
    ///
    /// Since the 0.15.x-adoption sweep, class-prefixed text items resolve
    /// case-insensitively (`parse_conductor_proxy`, r4133 parity), so this
    /// per-conductor dispatch is now the live text path AND the JSON-import path;
    /// the all-`none` reject path stays live too (r4133 #303-AVs on that input —
    /// UB, not reproduced). Both are gated by the whitebox equivalence tests
    /// `tests::conductors_array_matches_mixed_capi015` /
    /// `conductors_array_defaults_ratings_from_first_valid`, which drive the
    /// resolved-ref entry point directly against the capi015 references.
    pub(super) fn apply_conductors(&mut self) -> bool {
        let n = self.fnconds.max(0) as usize;
        let mut first_valid: Option<usize> = None;
        for i in 0..n {
            let choice = match self.fwiredata.get(i).and_then(|o| o.as_ref()) {
                Some(c) => conductor_choice_of(c),
                None => continue, // an unset slot is skipped
            };
            if first_valid.is_none() {
                first_valid = Some(i);
            }
            self.change_line_constants_type_at(Some(i), choice);
        }
        match first_valid {
            Some(i) => {
                self.default_amps_from(i);
                true
            }
            None => {
                let name = self.data.name().to_string();
                self.data.push_error(format!(
                    "LineGeometry.{name}.Conductors: At least one valid conductor must be provided."
                ));
                false
            }
        }
    }

    /// Pascal `wire`/`cncable`/`tscable` side effect (active conductor) and the
    /// `wires`/`cncables`/`tscables` "traditional" branch (first conductor):
    /// default this geometry's ratings from the conductor's once, when unset.
    ///
    /// A positive conductor rating fills a geometry rating that was never
    /// typed, or a typed zero. A typed `none` or `-1` is kept
    /// (`a_geometry_rating_typed_not_set_is_kept_against_a_later_conductor`).
    pub(super) fn default_amps_from(&mut self, cond_index: usize) {
        let Some((cnorm, cemerg, cnum, crat)) = self
            .fwiredata
            .get(cond_index)
            .and_then(|o| o.as_ref())
            .map(|o| o.amps_owned())
        else {
            return;
        };
        let fills = |conductor: Rating, geometry: Rating, typed: bool| {
            matches!(conductor, Rating::Set(c) if c > 0.0)
                && match geometry {
                    Rating::NotSet => !typed,
                    Rating::Set(g) => g == 0.0,
                }
        };
        if fills(
            cnorm,
            self.norm_amps,
            self.data.prp_specified(prop::NORMAMPS),
        ) {
            self.norm_amps = cnorm;
        }
        if fills(
            cemerg,
            self.emerg_amps,
            self.data.prp_specified(prop::EMERGAMPS),
        ) {
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

/// Pascal `conductors[i] is TCNDataObj / TTSDataObj` (LineGeometry.pas:522-533):
/// the conductor model a single catalog object implies.
fn conductor_choice_of(o: &ConductorObj) -> ConductorChoice {
    match o.conductor_kind() {
        ConductorKind::Cn => ConductorChoice::ConcentricNeutral,
        ConductorKind::Ts => ConductorChoice::TapeShield,
        ConductorKind::Wire => ConductorChoice::Overhead,
    }
}
