//! The impedance-source selection: adopting a `LineCode` or a `LineGeometry`
//! catalog object onto the line, and the `Kill*Specified`/`ResetLengthUnits`
//! helpers the property side effects use to switch back to the sym model.

use crate::elements::general::line_code::LineCodeObj;
use crate::elements::general::line_geometry::LineGeometryObj;
use crate::support::line_units::{LineUnits, convert_line_units};

use super::{Line, prop};

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
        self.kxg = self.xg / (658.5 * (self.rho / self.cd.base_frequency).sqrt()).ln();

        self.line_code_units = LineUnits::from_code(code.units());
        self.units_convert = convert_line_units(self.line_code_units, self.length_units);

        self.norm_amps = code.norm_amps();
        self.emerg_amps = code.emerg_amps();
        self.num_amp_ratings = code.num_amp_ratings();
        self.amp_ratings = code.amp_ratings().to_vec();

        // FaultRate/PctPerm/HrsToRepair deliberately NOT copied (Pascal
        // commented out 2014 — they vary section to section).

        // Zero the set-order marks of everything the code now supplies, so
        // `Save`/`?` reflect the linecode (non-NoPropertyTracking branch).
        for p in [
            GEOMETRY, SPACING, R1, X1, R0, X0, C1, C0, B1, B0, SEASONS, RATINGS, NORMAMPS,
            EMERGAMPS,
        ] {
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
            // Compute matrices from the copied sym components. Pascal reads
            // ActiveCircuit.PositiveSequence; at parse time we use the
            // multiphase default, exactly as the `phases=` side effect does.
            self.recalc(false);
        }

        // NConds := Fnphases; forces reallocation of terminal info + Yorder.
        let n = self.cd.nphases;
        self.cd.set_nconds(n);

        self.line_type = code.fline_type();
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
        // KillSpacingSpecified is the WP7.1 step-3b spacing path; no spacing can
        // be attached yet (the props are still NOT_PORTED).

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
}
