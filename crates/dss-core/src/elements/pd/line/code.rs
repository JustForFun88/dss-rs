//! The impedance-source selection: adopting a `LineCode` or a `LineGeometry`
//! catalog object onto the line, and the `Kill*Specified`/`ResetLengthUnits`
//! helpers the property side effects use to switch back to the sym model.

use crate::elements::general::conductor_data::ConductorKind;
use crate::elements::general::line_code::LineCodeObj;
use crate::elements::general::line_geometry::LineGeometryObj;
use crate::obj::base::{DssObject, ObjectRefArrayItem};
use crate::support::line_units::{LineUnits, convert_line_units};

use super::{ConductorChoice, Line, prop};

impl Line {
    /// Pascal `TLineObj.KillLineCodeSpecified`: drop the LineCode reference and
    /// clear its set-order mark, so a later sym/matrix override stops the dump
    /// from reporting the (now superseded) code.
    pub(super) fn kill_line_code_specified(&mut self) {
        self.line_code_ref = None;
        self.line_code_name = String::new();
        self.cd.obj.clear_seq(prop::LINECODE);
    }

    /// Pascal `ResetLengthUnits`.
    pub(super) fn reset_length_units(&mut self) {
        self.units_convert = 1.0;
        self.length_units = LineUnits::None;
        self.user_length_units = LineUnits::None;
    }

    /// Pascal `TLineObj.FetchLineCode`: copy the resolved LineCode's impedance
    /// data onto this line. Called from the `linecode=` property side effect
    /// (here, directly when the reference resolves). Faithful to the
    /// non-`DSS_EXTENSIONS_COMPAT` path: the copied properties' set-order marks
    /// are cleared so dumps reflect the code, not the line.
    pub(super) fn fetch_line_code(&mut self, code: &LineCodeObj) {
        use prop::*;

        // Frequency compensation takes place in CalcYPrim.
        self.cd.base_frequency = code.base_frequency();

        // Copy impedances, but do not recalc here for the matrix model: the
        // symmetrical-component z's may not match what is in the matrix.
        if code.sym_components_model() {
            self.r1 = code.r1();
            self.x1 = code.x1();
            self.r0 = code.r0();
            self.x0 = code.x0();
            self.c1 = code.c1();
            self.c0 = code.c0();
            self.sym_components_model = true;
        } else {
            self.sym_components_model = false;
        }

        // Earth-return impedances used to compensate for frequency.
        self.rg = code.rg();
        self.xg = code.xg();
        self.rho = code.rho();
        // TODO(compat): 658.5 (not 658.8530451057239) — see the Kxg note in
        // accessors.rs; upstream `Line.pas` keeps 658.5 while `LineConstants`
        // moved to the corrected De (UPGRADE_PLAN WP-U1.2 B2/D1).
        self.kxg = self.xg / (658.5 * (self.rho / self.cd.base_frequency).sqrt()).ln();

        self.line_code_units = LineUnits::from_code(code.units());
        self.units_convert = convert_line_units(self.line_code_units, self.length_units);

        self.norm_amps = code.norm_amps();
        self.emerg_amps = code.emerg_amps();
        self.num_amp_ratings = code.num_amp_ratings();
        self.amp_ratings = code.amp_ratings().to_vec();

        // FaultRate/PctPerm/HrsToRepair deliberately NOT copied (Pascal
        // commented out 2014 — they vary section to section).

        // Zero the set-order marks of the sym-component sources the linecode
        // supersedes (`Save`/`?` then reflect the linecode, not stale scalars).
        // The ratings the code supplies are (re)marked *set* by the `LINECODE`
        // side effect — which runs AFTER the linecode's own set-order mark, so
        // they sort after it (the pinned oracle's Save/JSON order); doing it here
        // (during the parse-time ref resolve, before the edit loop's
        // `SetAsNextSeq(linecode)`) would sort them before it.
        for p in [GEOMETRY, SPACING, R1, X1, R0, X0, C1, C0, B1, B0] {
            self.cd.obj.clear_seq(p);
        }

        if self.cd.nphases as i32 != code.nphases() {
            self.cd.nphases = code.nphases().max(0) as usize;
        }

        if !self.sym_components_model {
            // Copy matrices (Z, Yc) verbatim.
            self.z = code.z().cloned();
            self.yc = code.yc().cloned();
        } else {
            // Compute matrices from the copied sym components. Pascal
            // `FetchLineCode` calls `RecalcElementData` (`Line.pas:572`), which
            // reads the live `ActiveCircuit.PositiveSequence` (`Line.pas:1085`) —
            // in a `CktModel=Positive` circuit it collapses r0/x0/c0 into
            // r1/x1/c1 at fetch time (readback-observable, probe-proven). Use the
            // executive-synced live flag.
            self.recalc(self.positive_sequence);
        }

        // NConds := Fnphases; forces reallocation of terminal info + Yorder.
        let n = self.cd.nphases;
        self.cd.set_nconds(n);

        self.line_type = code.fline_type();

        // Pascal `FetchLineCode` tail (Line.pas:590-591): a `linecode=` supersedes
        // any spacing/geometry source.
        self.kill_spacing_specified();
        self.kill_geometry_specified();
    }

    /// Pascal `TLineObj.KillGeometrySpecified`: drop the geometry reference and
    /// clear its set-order mark; reset `FZFrequency` so a later geometry/spacing
    /// rebuild is forced. A no-op when no geometry is attached.
    pub(super) fn kill_geometry_specified(&mut self) {
        if self.geometry_obj.is_none() {
            return;
        }
        self.geometry_obj = None;
        self.geometry_name = String::new();
        self.cd.obj.clear_seq(prop::GEOMETRY);
        self.fz_frequency = -1.0;
    }

    /// Pascal `TLineObj.FetchGeometryCode`: adopt a resolved `LineGeometry` as
    /// the impedance source. Snapshot-clones it (the WP4.2 `FetchLineCode`
    /// pattern), copies the ratings/line-type, sizes the Line to the geometry's
    /// effective conductor count, and switches off the sym-component model.
    pub(super) fn fetch_geometry_code(&mut self, geom: &LineGeometryObj) {
        use prop::*;

        self.kill_line_code_specified();
        self.kill_spacing_specified();

        self.fz_frequency = -1.0; // Init to signify not computed

        // Own a private copy so the per-Line `rho`/cached matrices don't mutate
        // the shared catalog object (Pascal mutates the shared object — the
        // upstream "weird" TODO at Line.pas:776; cloning is the faithful Rust
        // equivalent under the snapshot-clone borrow model).
        let mut geom = geom.clone();

        // If `rho=` was set on this Line *before* `geometry=`, push it into the
        // geometry now (the symmetric after-case is handled in `side_effects`).
        if self.cd.obj.prp_specified(RHO) {
            geom.set_rho_earth(self.rho);
        }

        self.norm_amps = geom.norm_amps();
        self.emerg_amps = geom.emerg_amps();

        // Zero the set-order marks of everything the geometry now supplies, so
        // `Save`/`?` reflect the geometry (non-NoPropertyTracking branch).
        for p in [
            LINECODE, R1, X1, R0, X0, C1, C0, B1, B0, SEASONS, RATINGS, NORMAMPS, EMERGAMPS,
        ] {
            self.cd.obj.clear_seq(p);
        }

        // FNPhases := geometry.Nconds (reduce-aware); NConds := FNPhases forces
        // reallocation of terminal info + Yorder.
        self.cd.nphases = geom.nconds().max(0) as usize;
        let n = self.cd.nphases;
        self.cd.set_nconds(n);

        self.num_amp_ratings = geom.num_amp_ratings();
        self.amp_ratings = geom.amp_ratings().to_vec();
        self.line_type = geom.line_type();

        self.sym_components_model = false;
        self.sym_components_changed = false;

        self.geometry_obj = Some(geom);
        self.cd.yprim_invalid = true;
    }

    /// Pascal `TLineObj.SpacingSpecified` (Line.pas:2141): a spacing source is in
    /// force once both the spacing object and the (allocated) wire array exist.
    pub(super) fn spacing_specified(&self) -> bool {
        self.line_spacing_obj.is_some() && !self.line_wire_data.is_empty()
    }

    /// Pascal `TLineObj.KillSpacingSpecified` (Line.pas:2042): drop the spacing
    /// reference, free the wire array, reset `FPhaseChoice`/`FZFrequency`, and
    /// clear the spacing/wires/cncables/tscables set-order marks. No-op when no
    /// spacing is attached.
    pub(super) fn kill_spacing_specified(&mut self) {
        if !self.spacing_specified() {
            return;
        }
        self.line_spacing_obj = None;
        self.line_wire_data = Vec::new();
        self.fphase_choice = ConductorChoice::Unknown;
        self.fz_frequency = -1.0;
        for p in [prop::SPACING, prop::WIRES, prop::CNCABLES, prop::TSCABLES] {
            self.cd.obj.clear_seq(p);
        }
    }

    /// Pascal `TLineObj.FetchLineSpacing` (Line.pas:1853): adopt the (already
    /// stored) `LineSpacing` — drop LineCode/geometry, size the Line to the
    /// spacing's phase count, and allocate the empty `LineWireData` array of
    /// `NWires` slots that a following `wires=`/`cncables=`/`tscables=` fills.
    pub(super) fn fetch_line_spacing(&mut self) {
        let Some(spc) = self.line_spacing_obj.as_ref() else {
            return;
        };
        let nphases = spc.nphases();
        let nwires = spc.nwires().max(0) as usize;

        self.kill_line_code_specified();
        self.kill_geometry_specified();

        // need to establish Yorder before FMakeZFromSpacing
        self.cd.nphases = nphases.max(0) as usize;
        let n = self.cd.nphases;
        self.cd.set_nconds(n); // force reallocation of terminal info
        self.cd.yprim_invalid = true; // force rebuild of Y matrix

        self.line_wire_data = (0..nwires).map(|_| None).collect();
    }

    /// Pascal `TLineObj.SetWires` (Line.pas:803), the text path (`AllowAllConductors`
    /// is JSON-only, skipped — the `LineGeometry.set_wires` precedent). The `wires=`
    /// form: overhead conductors when `FPhaseChoice = Unknown`, else bare neutrals
    /// appended after the cable phases (`istart = NPhases + 1`). Validates the count
    /// against the open conductor span and seeds the Line's ratings from the wires.
    pub(super) fn set_wires(&mut self, refs: &[ObjectRefArrayItem<'_>]) {
        let Some(spc) = self.line_spacing_obj.as_ref() else {
            self.cd.obj.push_error(format!(
                "You must assign the LineSpacing before the Wires Property (\"Line.{}\").",
                self.cd.obj.name()
            ));
            return;
        };
        let nwires = spc.nwires().max(0) as usize;
        let nphases = spc.nphases().max(0) as usize;

        let istart = if self.fphase_choice == ConductorChoice::Unknown {
            // it's an overhead line
            self.kill_line_code_specified();
            self.kill_geometry_specified();
            1
        } else {
            // adding bare neutrals to an underground line
            nphases + 1
        };

        // Validate number of elements: (NWires - istart + 1).
        let expected = nwires.saturating_sub(istart) + 1;
        if expected != refs.len() {
            self.cd.obj.push_error(format!(
                "Line.{}: Unexpected number ({}) of wires; expected {expected} objects.",
                self.cd.obj.name(),
                refs.len()
            ));
            return;
        }

        let mut new_num_rat = 1i32;
        let mut new_ratings: Vec<f64> = Vec::new();
        let mut ratings_inc = false;
        for (k, i) in (istart..=nwires).enumerate() {
            // A `none` slot (AllowNoneItem) stays NIL and contributes no ratings.
            let Some((_, res)) = refs[k].as_ref() else {
                self.line_wire_data[i - 1] = None;
                continue;
            };
            let obj = res.obj();
            let (cnorm, cemerg, cnum, crat) = conductor_amps(obj);
            self.line_wire_data[i - 1] = Some(obj.clone_box());
            if cnum > new_num_rat {
                new_num_rat = cnum;
                new_ratings = crat.into_iter().take(new_num_rat.max(0) as usize).collect();
                ratings_inc = true;
            }
            self.norm_amps = cnorm;
            self.emerg_amps = cemerg;
        }
        if ratings_inc {
            self.num_amp_ratings = new_num_rat;
            self.amp_ratings = new_ratings;
        }
        self.cd.obj.set_as_next_seq(prop::RATINGS);
        self.cd.obj.set_as_next_seq(prop::NORMAMPS);
        self.cd.obj.set_as_next_seq(prop::EMERGAMPS);
    }

    /// Pascal generic `DSSObjectReferenceArrayProperty` fill for the r4133
    /// `Conductors=` form (`Line.pas:341-344`, the 3-class list): write each
    /// resolved conductor / NIL straight into `LineWireData` from slot 0. The
    /// conductor *model* (`FPhaseChoice`) and ratings are computed by the side
    /// effect (`Line.pas:750-865`), matching upstream's split. Class-prefixed text
    /// items resolve via the CASE-INSENSITIVE `parse_conductor_proxy` (r4133
    /// `LowerCase(CondClass)`; the capi015 `GetDSSClass` case bug was dropped in
    /// the 0.15.x-adoption sweep), and a `none` slot writes NIL (compacted out at
    /// solve time by `LoadSpacingAndWires`); the real-conductor fill is also gated
    /// by the whitebox equivalence test
    /// `tests::conductors_array_matches_buried_neutral_and_oracle`.
    pub(super) fn set_conductors(&mut self, refs: &[ObjectRefArrayItem<'_>]) {
        for (i, r) in refs.iter().enumerate() {
            if i < self.line_wire_data.len() {
                self.line_wire_data[i] = r.as_ref().map(|(_, o)| o.obj().clone_box());
            }
        }
    }

    /// Pascal `Line.pas:762-782`: the conductor *model* the `Conductors=` list
    /// implies — the **last** valid phase conductor decides (`TCNDataObj` →
    /// ConcentricNeutral, `TTSDataObj` → TapeShield, any wire → Overhead); an
    /// all-`none` phase set leaves `Unknown` (the caller defaults it to Overhead).
    pub(super) fn conductors_phase_choice(&self) -> ConductorChoice {
        let nph = (self.cd.nphases).min(self.line_wire_data.len());
        let mut choice = ConductorChoice::Unknown;
        for slot in self.line_wire_data.iter().take(nph) {
            if let Some(c) = slot.as_ref() {
                choice = conductor_choice_of(c.as_ref());
            }
        }
        choice
    }

    /// Pascal's generic `DSSObjectReferenceArrayProperty` fill for the
    /// `cncables=`/`tscables=` forms (no `WriteByFunction`, unlike `wires=`): write
    /// the resolved cables straight into `LineWireData` from conductor 1, with no
    /// `istart` gymnastics. `FPhaseChoice` is set by the side effect, so a following
    /// `wires=` appends bare neutrals after the cable phases. `prop` is the display
    /// name for the no-spacing error (`CNCables`/`TSCables`).
    ///
    /// A missing/too-short wire array is left partly NIL — the solve-time
    /// `LoadSpacingAndWires` reports the "not correctly initialized" abort exactly
    /// as upstream (probe-confirmed); but with *no* spacing at all (`FWireDataSize <
    /// 1`) the generic fill raises Pascal error 402 up front, so reproduce that.
    pub(super) fn set_cables(&mut self, prop: &str, refs: &[ObjectRefArrayItem<'_>]) {
        if self.line_wire_data.is_empty() {
            self.cd.obj.push_error(format!(
                "Line.{}.{prop}: No objects are expected! \
                 Check if the order of property assignments is correct.",
                self.cd.obj.name()
            ));
            return;
        }
        for (k, r) in refs.iter().enumerate() {
            if k < self.line_wire_data.len() {
                // A `none` slot (AllowNoneItem) stays NIL.
                self.line_wire_data[k] = r.as_ref().map(|(_, o)| o.obj().clone_box());
            }
        }
    }
}

/// Pascal `condObj is TCNDataObj / TTSDataObj` (`Line.pas:772-780`): the
/// conductor model a single catalog object implies.
fn conductor_choice_of(o: &dyn DssObject) -> ConductorChoice {
    match o.as_conductor().map(|c| c.conductor_kind()) {
        Some(ConductorKind::Cn) => ConductorChoice::ConcentricNeutral,
        Some(ConductorKind::Ts) => ConductorChoice::TapeShield,
        _ => ConductorChoice::Overhead,
    }
}

/// `(NormAmps, EmergAmps, NumAmpRatings, AmpRatings)` of a resolved conductor,
/// whichever concrete catalog type it is (Pascal reads `TConductorDataObj` fields).
fn conductor_amps(o: &dyn DssObject) -> (f64, f64, i32, Vec<f64>) {
    match o.as_conductor() {
        Some(c) => {
            let (n, e, k, r) = c.amps();
            (n, e, k, r.to_vec())
        }
        None => (0.0, 0.0, 1, Vec::new()),
    }
}
