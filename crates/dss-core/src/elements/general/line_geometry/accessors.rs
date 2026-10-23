//! The `DssObject` property trait for `LineGeometryObj`: the typed get/set
//! accessors, object-reference plumbing and the property side effects.

use crate::diag::DssDiagnostic;
use crate::obj::Rating;
use crate::obj::arena::ResolvedObj;
use crate::obj::base::{DssObjData, DssObject, ObjectRefArrayItem};

use super::{ConductorChoice, LineGeometryObj, LineType, prop};
use crate::elements::general::conductor_data::ConductorObj;
use crate::elements::general::line_spacing::LineSpacingObj;
use crate::support::line_units::{LineUnits, POSITION_WORD_LIST};
use dss_parser::ParserError;

/// The number of the out-of-range `cond=` diagnostic.
const COND_OUT_OF_RANGE: u32 = 10102;

/// The `like=` row the class table appends after the class's own properties.
const LIKE: usize = prop::NUM_PROPS;

impl LineGeometryObj {
    /// Refuse conductor data written with no conductor selected (`prop_name` as
    /// the class table spells it). Returns the selected conductor otherwise.
    fn selected_for(&mut self, prop_name: &str) -> Option<usize> {
        let a = self.active_index();
        if a.is_none() {
            let name = self.data.name().to_string();
            self.data.push_error(format!(
                "LineGeometry.{name}.{prop_name}: conductor data without cond=. No conductor \
                 is selected, so the value is not applied. Select one with cond=N first."
            ));
        }
        a
    }

    /// Select conductor `value` (1-based), or refuse an out-of-range value and
    /// select none. A written `cond=` is range-checked by `refuse_integer`
    /// first, so this refusal serves a value set directly. Pinned by
    /// `elements::general::line_geometry::tests::cond_set_directly_out_of_range_selects_nothing`.
    fn select_conductor(&mut self, value: i32) {
        let n = self.fnconds.max(0);
        if value >= 1 && value <= n {
            self.selected = Some((value - 1) as usize);
            return;
        }
        self.selected = None;
        let msg = self.cond_refusal(&value.to_string(), "is out of range");
        self.data
            .push_error(DssDiagnostic::msg(msg, Some(COND_OUT_OF_RANGE)));
    }

    /// The message refusing `cond=<text>`: `what` the value is, the
    /// geometry's conductor count and the selection it leaves.
    fn cond_refusal(&self, text: &str, what: &str) -> String {
        let has = match self.fnconds.max(0) {
            1 => "has 1 conductor".to_string(),
            n => format!("has {n} conductors"),
        };
        format!(
            "LineGeometry.{}.Cond: cond={text} {what}, the geometry {has}. No conductor is \
             selected.",
            self.data.name()
        )
    }

    pub(crate) fn make_like(&mut self, other: &Self) {
        self.data.copy_prp_sequence_from(other.data());
        let o = other;
        {
            // Take the source's count and select no conductor, then copy the
            // source's default unit, every per-conductor array and its engine.
            self.fnconds = o.fnconds;
            self.selected = None;
            self.last_filled = None;
            self.flast_unit = o.flast_unit;
            self.fnphases = o.fnphases;
            self.line_spacing_obj = o.line_spacing_obj.clone();
            // dss_capi 0.15.x `MakeLike` copies the equivalent-spacing fields.
            self.eq_dist_ph_ph = o.eq_dist_ph_ph;
            self.eq_dist_ph_n = o.eq_dist_ph_n;
            self.avg_phase_height = o.avg_phase_height;
            self.avg_neutral_height = o.avg_neutral_height;
            self.eq_unit = o.eq_unit;
            self.equivalent_spacing = o.equivalent_spacing;
            self.fline_type = o.fline_type;
            self.fphase_choice.clone_from(&o.fphase_choice);
            self.fwiredata.clone_from(&o.fwiredata);
            self.fx.clone_from(&o.fx);
            self.fy.clone_from(&o.fy);
            self.funits.clone_from(&o.funits);
            self.data_changed = true;
            self.norm_amps = o.norm_amps;
            self.emerg_amps = o.emerg_amps;
            self.freduce = o.freduce;
            // Pascal's `NConds := Other.NWires` rebuilds an *overhead* engine via
            // the nconds side effect and then runs `UpdateLineGeometryData`; for a
            // cable source that trailing update raises `EInvalidCast` (FLineData is
            // overhead but the conductors are CN/TS). We instead clone the source
            // engine so the kind matches the copied conductors and defer the
            // recompute (`data_changed = true` keeps it stale until first use).
            self.fline_data = o.fline_data.clone();
        }
    }
}

impl DssObject for LineGeometryObj {
    fn data(&self) -> &DssObjData {
        &self.data
    }
    fn data_mut(&mut self) -> &mut DssObjData {
        &mut self.data
    }

    fn get_i32(&self, idx: usize) -> i32 {
        match idx {
            prop::NCONDS => self.fnconds,
            prop::NPHASES => self.fnphases,
            // 1-based, 0 when no conductor is selected.
            prop::COND => self.active_index().map_or(0, |a| a as i32 + 1),
            // The selected conductor's unit, or the default with none selected.
            prop::UNITS => self
                .active_index()
                .map_or(self.flast_unit, |a| self.conductor_unit(a)),
            prop::SEASONS => self.num_amp_ratings,
            prop::LINETYPE => self.fline_type.ordinal(),
            _ => unreachable!("LineGeometry has no integer at {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        match idx {
            prop::NCONDS => {
                self.fnconds = value;
                self.nconds_stored = true;
            }
            prop::NPHASES => self.fnphases = value,
            // The unit default of a newly selected conductor is in the `cond`
            // side effect.
            prop::COND => self.select_conductor(value),
            // A unit belongs to the selected conductor (if any) and becomes the
            // default. Code 0 and a code outside 1 to 8 name no length unit and
            // are refused (the text path refuses any word outside the list
            // before it gets here).
            prop::UNITS => {
                if LineUnits::from_code(value) == LineUnits::None {
                    let name = self.data.name().to_string();
                    self.data.push_error(format!(
                        "LineGeometry.{name}.Units: unit code {value} is not one of the length \
                         units {POSITION_WORD_LIST}."
                    ));
                    return;
                }
                if let Some(a) = self.active_index() {
                    self.funits[a] = value;
                }
                self.flast_unit = value;
            }
            prop::SEASONS => self.num_amp_ratings = value,
            prop::LINETYPE => {
                self.fline_type = LineType::from_ordinal(value).unwrap_or(self.fline_type)
            }
            _ => unreachable!("LineGeometry has no integer at {idx}"),
        }
    }

    /// An `x` or `h` that the selected conductor does not hold, or with none
    /// selected, reads `NaN`: every reader asks [`DssObject::holds_no_value`]
    /// first and shows it empty.
    fn get_f64(&self, idx: usize) -> f64 {
        match idx {
            prop::X => self
                .active_index()
                .and_then(|a| self.fx[a])
                .unwrap_or(f64::NAN),
            prop::H => self
                .active_index()
                .and_then(|a| self.fy[a])
                .unwrap_or(f64::NAN),
            _ => unreachable!("LineGeometry has no double at {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        match idx {
            prop::X => {
                if let Some(a) = self.selected_for("X") {
                    self.fx[a] = Some(value);
                }
            }
            prop::H => {
                if let Some(a) = self.selected_for("H") {
                    self.fy[a] = Some(value);
                }
            }
            _ => unreachable!("LineGeometry has no double at {idx}"),
        }
    }

    fn get_rating(&self, idx: usize) -> Rating {
        match idx {
            prop::NORMAMPS => self.norm_amps,
            prop::EMERGAMPS => self.emerg_amps,
            _ => unreachable!("LineGeometry has no rating at {idx}"),
        }
    }
    fn set_rating(&mut self, idx: usize, value: Rating) {
        match idx {
            prop::NORMAMPS => self.norm_amps = value,
            prop::EMERGAMPS => self.emerg_amps = value,
            _ => unreachable!("LineGeometry has no rating at {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        debug_assert_eq!(idx, prop::REDUCE);
        self.freduce
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        debug_assert_eq!(idx, prop::REDUCE);
        self.freduce = value;
    }

    fn get_string(&self, idx: usize) -> String {
        let name_of = |slot: Option<usize>| {
            slot.and_then(|a| self.fwiredata.get(a))
                .and_then(|o| o.as_ref())
                .map(|o| o.name().to_string())
                .unwrap_or_default()
        };
        match idx {
            prop::WIRE | prop::CNCABLE | prop::TSCABLE => name_of(self.active_index()),
            prop::SPACING => self
                .line_spacing_obj
                .as_ref()
                .map(|o| o.data().name().to_string())
                .unwrap_or_default(),
            _ => unreachable!("LineGeometry has no string at {idx}"),
        }
    }

    fn set_object_ref(&mut self, idx: usize, _name: String, resolved: Option<ResolvedObj<'_>>) {
        match idx {
            prop::WIRE | prop::CNCABLE | prop::TSCABLE => {
                let prop_name = match idx {
                    prop::WIRE => "Wire",
                    prop::CNCABLE => "CNCable",
                    _ => "TSCable",
                };
                if let Some(a) = self.selected_for(prop_name) {
                    self.fwiredata[a] = resolved.and_then(ConductorObj::from_resolved);
                }
            }
            prop::SPACING => {
                self.line_spacing_obj = resolved.and_then(|o| o.cloned::<LineSpacingObj>())
            }
            _ => unreachable!("LineGeometry has no object reference at {idx}"),
        }
    }

    fn set_object_ref_array(&mut self, idx: usize, refs: &[ObjectRefArrayItem<'_>]) {
        match idx {
            prop::WIRES => self.set_wires(refs),
            prop::CNCABLES | prop::TSCABLES => self.set_cables(refs),
            // Pascal generic fill (no WriteByFunction): write each resolved
            // conductor / NIL straight into `conductors` from slot 0; the side
            // effect sets the engine kind + ratings (`Conductors` handling).
            prop::CONDUCTORS => {
                for (i, r) in refs.iter().enumerate() {
                    if i < self.fwiredata.len() {
                        self.fwiredata[i] = r
                            .as_ref()
                            .and_then(|(_, o)| ConductorObj::from_resolved(*o));
                    }
                }
            }
            _ => unreachable!("LineGeometry has no object-ref-array property {idx}"),
        }
    }
    fn get_object_ref_names(&self, idx: usize) -> Vec<String> {
        debug_assert!(matches!(
            idx,
            prop::WIRES | prop::CNCABLES | prop::TSCABLES | prop::CONDUCTORS
        ));
        self.fwiredata
            .iter()
            .map(|o| o.as_ref().map(|o| o.name().to_string()).unwrap_or_default())
            .collect()
    }

    /// Pascal `PropertyStructArrayCountOffset := @obj.FNConds`
    /// (LineGeometry.pas:244): the conductor-array length the generic
    /// `Conductors=` fill validates against (#402 when `< 1`).
    fn array_size(&self, idx: usize) -> usize {
        debug_assert_eq!(idx, prop::CONDUCTORS);
        self.fnconds.max(0) as usize
    }

    fn get_rating_array(&self, idx: usize) -> Option<&[Rating]> {
        debug_assert_eq!(idx, prop::RATINGS);
        (!self.amp_ratings.is_empty()).then_some(self.amp_ratings.as_slice())
    }
    fn set_rating_array(&mut self, idx: usize, value: Vec<Rating>) {
        debug_assert_eq!(idx, prop::RATINGS);
        self.amp_ratings = value;
    }

    /// `cond=` takes a conductor number from 1 to `nconds`: a fraction is
    /// refused, and so is a whole number outside that range, each naming the
    /// value as written, the out-of-range one with code 10102 in and past the
    /// integer range alike. Pinned by
    /// `exec::tests::line_geometry_rules::cond_that_is_not_a_whole_number_selects_nothing`
    /// and `::cond_out_of_range_is_refused_and_selects_nothing`.
    /// A `nconds=` past the integer range is refused naming the value as
    /// written: a count that large needs more memory than is available, and a
    /// negative one is no count. Pinned by
    /// `exec::tests::line_geometry_rules::a_count_beyond_the_memory_is_refused`.
    fn refuse_integer(&self, idx: usize, text: &str, value: f64) -> Option<ParserError> {
        if idx == prop::NCONDS {
            let msg = if value > f64::from(i32::MAX) {
                self.nconds_memory_refusal(text)
            } else if value < f64::from(i32::MIN) {
                format!(
                    "LineGeometry.{}.NConds: Value ({text}) cannot be negative.",
                    self.data.name()
                )
            } else {
                return None;
            };
            return Some(ParserError::new(msg));
        }
        if idx != prop::COND {
            return None;
        }
        if !value.is_finite() || value.fract() != 0.0 {
            return Some(ParserError::new(
                self.cond_refusal(text, "is not a conductor number"),
            ));
        }
        if value >= 1.0 && value <= f64::from(self.fnconds.max(0)) {
            return None;
        }
        Some(
            ParserError::new(self.cond_refusal(text, "is out of range"))
                .with_code(COND_OUT_OF_RANGE),
        )
    }

    /// A `cond=` whose value is not a number, or one `refuse_integer` refuses,
    /// selects no conductor, like an out-of-range one, so the conductor data
    /// after it is refused.
    fn value_unreadable(&mut self, idx: usize) {
        if idx == prop::COND {
            self.selected = None;
        }
    }

    /// With no conductor selected, the JSON export leaves out the selected
    /// conductor's values (`Cond` 0, no conductor object): the import refuses
    /// each of them by the selection rules. A geometry without conductors
    /// writes its name only, since the import refuses every property before
    /// `NConds` and its 0. Pinned by
    /// `exec::tests::line_geometry_rules::json_export_of_a_geometry_without_conductors_writes_its_name_only`.
    fn json_omits(&self, idx: usize) -> bool {
        self.fnconds <= 0
            || (self.selected.is_none()
                && matches!(idx, prop::COND | prop::WIRE | prop::CNCABLE | prop::TSCABLE))
    }

    /// `x` and `h` hold no value with no conductor selected, or when the
    /// selected conductor's was never given. Pinned by
    /// `exec::tests::line_geometry_rules::x_and_h_read_empty_when_never_given`.
    fn holds_no_value(&self, idx: usize) -> bool {
        match idx {
            prop::X => self.active_index().and_then(|a| self.fx[a]).is_none(),
            prop::H => self.active_index().and_then(|a| self.fy[a]).is_none(),
            _ => false,
        }
    }

    /// Before the first `nconds=` the geometry has no conductors, and every
    /// property but `nconds=` and `like=` is refused, naming it. A `like=` of
    /// a source with conductors builds the table as `nconds=` does. Pinned by
    /// `exec::tests::line_geometry_rules::a_property_before_the_first_nconds_is_refused`.
    fn refuse_property(&self, idx: usize, prop_name: &str) -> Option<String> {
        (self.fnconds <= 0 && idx != prop::NCONDS && idx != LIKE).then(|| {
            format!(
                "LineGeometry.{}.{prop_name}: the geometry has no conductors yet, so the value \
                 is not applied. Give nconds= first.",
                self.data.name()
            )
        })
    }

    fn side_effects(&mut self, idx: usize, _prev_int: i32) {
        // Three blocks: the engine and selection state, the ratings default,
        // then the staleness flag.
        let selected = self.active_index();
        match idx {
            prop::NPHASES => {
                // The engine's phase count, the number of phase positions.
                let np = self.phase_positions();
                if let Some(ld) = self.fline_data.as_mut() {
                    ld.set_nphases(np);
                }
            }
            prop::COND => {
                // A conductor without a unit of its own takes the default.
                if let Some(a) = selected
                    && self.funits[a] == super::UNIT_UNSET
                {
                    self.funits[a] = self.flast_unit;
                }
            }
            // The singular conductor arms act on the selected conductor only:
            // with none selected the setter already refused the value.
            prop::WIRE => {
                if let Some(a) = selected
                    && self.fphase_choice[a] == ConductorChoice::Unknown
                {
                    self.change_line_constants_type_at(Some(a), ConductorChoice::Overhead);
                }
            }
            prop::CNCABLE if selected.is_some() => {
                self.change_line_constants_type_at(selected, ConductorChoice::ConcentricNeutral);
            }
            prop::TSCABLE if selected.is_some() => {
                self.change_line_constants_type_at(selected, ConductorChoice::TapeShield);
            }
            // The plural cable arms set the model of the last slot the fill
            // wrote (nothing when the fill was refused).
            prop::CNCABLES => {
                if let Some(last) = self.last_filled.take() {
                    self.change_line_constants_type_at(
                        Some(last),
                        ConductorChoice::ConcentricNeutral,
                    );
                }
            }
            prop::TSCABLES => {
                if let Some(last) = self.last_filled.take() {
                    self.change_line_constants_type_at(Some(last), ConductorChoice::TapeShield);
                }
            }
            // A refused `nconds=` (0 or negative) leaves the conductors as
            // they were.
            prop::NCONDS => {
                if std::mem::take(&mut self.nconds_stored) {
                    let _ = self.realloc_conductors();
                }
            }
            prop::SPACING => self.apply_spacing(),
            // `like=` selects no conductor, whether its source was found or not.
            LIKE => self.selected = None,
            _ => {}
        }
        if idx == prop::WIRES {
            self.last_filled = None;
        }

        // Second block: default this geometry's ratings from its conductors.
        match idx {
            prop::WIRES | prop::CNCABLES | prop::TSCABLES => {
                // The plural forms seed the ratings from the first conductor,
                // whichever slots the fill wrote.
                self.default_amps_from(0);
            }
            prop::WIRE | prop::CNCABLE | prop::TSCABLE => {
                // A conductor object on conductor 1 seeds the geometry ratings.
                // A name that did not resolve (the parse already logged its
                // "not found") leaves the slot unset and logs that the object
                // was not defined. Nothing happens with no conductor selected.
                if let Some(a) = selected {
                    if self.fwiredata[a].is_some() {
                        if a == 0 {
                            self.default_amps_from(0);
                        }
                    } else {
                        self.data.push_error(
                            "WireData/CNData/TSData object was not defined. \
                             Must be previously defined."
                                .to_string(),
                        );
                    }
                }
            }
            prop::SEASONS => {
                let n = self.num_amp_ratings.max(0) as usize;
                self.amp_ratings.resize(n, Rating::NotSet);
            }
            // Pascal LineGeometry.pas:497-539: the 0.15.x `Conductors=` list. On an
            // all-NIL list `apply_conductors` logs #10103 and Pascal `Exit`s before
            // the `dataChanged` block, so flag stale only when a conductor was
            // valid.
            prop::CONDUCTORS => {
                if self.apply_conductors() {
                    self.data_changed = true;
                }
            }
            _ => {}
        }

        // Third block: flag the geometry stale (step 2c-ii consumes this).
        if matches!(
            idx,
            prop::NCONDS
                | prop::WIRE
                | prop::X
                | prop::H
                | prop::UNITS
                | prop::SPACING
                | prop::WIRES
                | prop::CNCABLE
                | prop::TSCABLE
                | prop::CNCABLES
                | prop::TSCABLES
        ) {
            self.data_changed = true;
        }
    }
}
