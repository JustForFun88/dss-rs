//! The Carson engine path: `UpdateLineGeometryData` (fill `FLineData`, run the
//! `Calc`/`Reduce`) and the on-demand `Get_Zmatrix`/`Get_YCmatrix` accessors,
//! plus the `RhoEarth` getter/setter.

use crate::elements::general::conductor_data::{
    CableGeom, ConductorData, ConductorGeom, ConductorKind, ConductorObj,
};
use crate::elements::general::line_spacing::LineSpacingObj;
use crate::obj::Rating;
use crate::support::cmatrix::CMatrix;
use crate::support::line_constants::ConductorType;

use super::{ConductorChoice, LineGeometryObj};

impl LineGeometryObj {
    /// Pascal `TLineGeometryObj.LoadSpacingAndWires` (LineGeometry.pas): build a
    /// throwaway geometry from a `LineSpacing` plus the resolved conductor list —
    /// the path `TLineObj.FMakeZFromSpacing` drives. Sizes the geometry to the
    /// spacing's wire count, copies the coordinates/units, picks the conductor
    /// model from the wire kinds, then runs the Carson `Calc` at `f`/`earth_model`.
    ///
    /// `earth_model` is the consuming Line's `FEarthModel` (Pascal sets
    /// `DSS.ActiveEarthModel := FEarthModel` around the matrix read; computing here
    /// under that model makes the later `z_matrix`/`yc_matrix` scale-only reads
    /// reproduce it). `wires[i]` is conductor `i+1` (the Line's `LineWireData`).
    #[allow(clippy::too_many_arguments)]
    pub fn load_spacing_and_wires(
        &mut self,
        spc: &LineSpacingObj,
        wires: &[Option<ConductorObj>],
        f: f64,
        earth_model: i32,
        eps_r_medium: f64,
        height_offset: f64,
        height_units: i32,
    ) -> Result<(), String> {
        // r4133 `LoadSpacingAndWires` (LineGeometry.pas:1190-1262): before
        // allocating, recount the conductors actually present. A Line-level
        // `wires=(w none)` (a documented r4088+/r4133 pattern, accepted by BOTH
        // gating oracles) leaves NIL slots; the geometry is sized to the compacted
        // count (`actualNConds`), and only the non-NIL wires are copied — into
        // CONTIGUOUS positions but with their ORIGINAL spacing coordinates
        // (`FX^[j] := Spc.Xcoord[i]`). `actualNPhases` counts the non-NIL wires
        // whose ORIGINAL index is a phase position, so no NIL slot reaches the
        // Carson calc. For a list with no NIL, `actualNConds == NWires` and
        // `j == i`.
        let nwires = spc.nwires().max(0) as usize;
        let spc_nphases = spc.nphases();
        let mut actual_nconds = 0i32;
        let mut actual_nphases = 0i32;
        for (i, o) in wires.iter().take(nwires).enumerate() {
            if o.is_some() {
                actual_nconds += 1;
                if (i as i32) < spc_nphases {
                    actual_nphases += 1;
                }
            }
        }
        self.fnconds = actual_nconds;
        self.realloc_conductors();
        self.fnphases = actual_nphases;
        self.line_spacing_obj = Some(spc.clone());
        if self.fnconds > self.fnphases {
            self.freduce = true;
        }

        // Pick the conductor model: any CN ⇒ ConcentricNeutral, any TS ⇒
        // TapeShield (TS wins if both present, mirroring Pascal's sequential ifs),
        // else Overhead. Over the non-NIL wires.
        let mut new_choice = ConductorChoice::Overhead;
        for o in wires.iter().take(nwires).flatten() {
            // Sequential ifs in Pascal: TS wins if both a CN and a TS are
            // present. Preserve that by not resetting on a plain Wire.
            match o.conductor_kind() {
                ConductorKind::Cn => new_choice = ConductorChoice::ConcentricNeutral,
                ConductorKind::Ts => new_choice = ConductorChoice::TapeShield,
                ConductorKind::Wire => {}
            }
        }
        self.change_line_constants_type_at(Some(0), new_choice);

        // dss_capi 0.15.x: adopt the spacing's equivalent-spacing model. When
        // equivalent, the per-conductor coordinates are not read.
        self.equivalent_spacing = spc.equivalent_spacing();
        if self.equivalent_spacing {
            self.eq_dist_ph_ph = spc.eq_dist_ph_ph();
            self.eq_dist_ph_n = spc.eq_dist_ph_n();
            self.avg_phase_height = spc.avg_phase_height();
            self.avg_neutral_height = spc.avg_neutral_height();
            self.flast_unit = spc.spacing_units();
        }

        // Skip-NIL copy: contiguous conductor `j`, spacing coordinates indexed by
        // the ORIGINAL position `i`. NormAmps and EmergAmps are each the lowest
        // among the PHASE conductors' ratings of that kind, since the line
        // current flows through every phase conductor, and not set when any
        // phase conductor has none. The result does not depend on conductor
        // order (`spacing_minimum_does_not_depend_on_conductor_order`,
        // `spacing_ratings_are_each_the_weakest_phase_wires_in_any_order`).
        let units = spc.spacing_units();
        let xs = spc.xcoord();
        let hs = spc.ycoord();
        let nph = self.fnphases.max(0) as usize;
        let (mut norm, mut emerg): (Option<Rating>, Option<Rating>) = (None, None);
        let mut j = 0usize; // 0-based contiguous conductor index (Pascal 1-based)
        for (i, o) in wires.iter().take(nwires).enumerate() {
            let Some(o) = o.as_ref() else { continue };
            self.fwiredata[j] = Some(o.clone());
            if !self.equivalent_spacing {
                self.fx[j] = xs[i];
                self.fy[j] = hs[i];
                self.funits[j] = units;
            }
            let (cn, ce) = conductor_norm_emerg(o);
            // 0-based `j < nph` == Pascal 1-based `(j+1) <= FNPhases`.
            if j < nph {
                norm = Some(weaker_rating(norm, cn));
                emerg = Some(weaker_rating(emerg, ce));
            }
            j += 1;
        }
        self.norm_amps = norm.unwrap_or(Rating::NotSet);
        self.emerg_amps = emerg.unwrap_or(Rating::NotSet);
        self.data_changed = true;

        // dss_capi 0.15.x (Line.pas:2111-2113): apply the consuming Line's
        // EpsRMedium/HeightOffset/HeightUnit to the engine *before* the Carson
        // calc, so the equivalent-spacing height offset folds into the average
        // heights (`update_line_geometry_data`) and eps/height flag `rhoChanged`.
        self.set_line_constants_medium(eps_r_medium, height_offset, height_units);

        self.update_line_geometry_data(f, earth_model)
    }

    /// Pascal `UpdateLineGeometryData(f)` (LineGeometry.pas:918-982): push every
    /// conductor's geometry into the `FLineData` engine, set `Nphases`, then run
    /// the Carson `Calc` (and a Kron `Reduce` when `FReduce`). `earth_model` is
    /// Pascal's `DSS.ActiveEarthModel`, supplied by the solution.
    ///
    /// Returns `Err` for the two abort paths: conductors without a conductor
    /// object (one message naming every one of them) and a failed geometry
    /// check (conductors in the same place).
    pub fn update_line_geometry_data(&mut self, f: f64, earth_model: i32) -> Result<(), String> {
        let n = self.fnconds.max(0) as usize;

        // Gather every conductor's data first (immutable borrows) so the engine
        // fill below can borrow `FLineData`.
        let geoms: Vec<Option<ConductorGeom>> = (0..n)
            .map(|i| {
                self.fwiredata
                    .get(i)
                    .and_then(|o| o.as_ref())
                    .map(|o| o.geom())
            })
            .collect();
        let missing: Vec<usize> = (0..n).filter(|&i| geoms[i].is_none()).collect();
        if !missing.is_empty() {
            return Err(missing_conductors_message(&self.error_subject(), &missing));
        }
        let geoms: Vec<ConductorGeom> = geoms.into_iter().flatten().collect();
        let units = self.conductor_units();
        let phases = self.phase_positions();

        // `FNConds = 0` ⇒ no engine (Pascal's loop never runs, `FLineData` NIL).
        let Some(eng) = self.fline_data.as_mut() else {
            return Ok(());
        };

        // dss_capi 0.15.x `UpdateLineGeometryData`: push the equivalent-spacing
        // state first. The distances are converted from `flast_unit` to meters;
        // the avg heights also carry the (0-default) height offset.
        eng.set_equivalent_spacing(self.equivalent_spacing);
        if self.equivalent_spacing {
            let to_m =
                crate::support::line_units::LineUnits::from_code(self.flast_unit).to_meters();
            let h_off = eng.height_offset_meters();
            eng.set_equivalent_distances(
                self.eq_dist_ph_ph * to_m,
                self.eq_dist_ph_n * to_m,
                self.avg_phase_height * to_m + h_off,
                self.avg_neutral_height * to_m + h_off,
            );
        }

        for (i, g) in geoms.iter().enumerate() {
            // Equivalent spacing reads no coordinates.
            if !self.equivalent_spacing {
                eng.set_x(i, units[i], self.fx[i]);
                eng.set_y(i, units[i], self.fy[i]);
            }
            eng.set_radius(i, g.radius_units, g.radius);
            eng.set_capradius(i, g.radius_units, g.cap_radius);
            eng.set_gmr(i, g.gmr_units, g.gmr);
            eng.set_rdc(i, g.res_units, g.rdc);
            eng.set_rac(i, g.res_units, g.rac);
            // dss_capi 0.15.x `UpdateLineGeometryData`: for cable conductors,
            // assign the per-conductor CN/TS type (`SetCondType`) into the
            // merged engine before pushing the cable data. A plain wire
            // conductor stays `INVALID` (no cable branch).
            match &g.cable {
                Some(CableGeom::Cn {
                    eps_r,
                    ins_layer,
                    dia_ins,
                    dia_cable,
                    k_strand,
                    dia_strand,
                    gmr_strand,
                    r_strand,
                    semicon_layer,
                }) => {
                    eng.set_cond_type(i, ConductorType::Cn);
                    eng.set_eps_r(i, *eps_r);
                    eng.set_ins_layer(i, g.radius_units, *ins_layer);
                    eng.set_dia_ins(i, g.radius_units, *dia_ins);
                    eng.set_dia_cable(i, g.radius_units, *dia_cable);
                    eng.set_k_strand(i, *k_strand);
                    eng.set_dia_strand(i, g.radius_units, *dia_strand);
                    eng.set_gmr_strand(i, g.gmr_units, *gmr_strand);
                    eng.set_r_strand(i, g.res_units, *r_strand);
                    eng.set_semicon_layer(i, *semicon_layer);
                }
                Some(CableGeom::Ts {
                    eps_r,
                    ins_layer,
                    dia_ins,
                    dia_cable,
                    dia_shield,
                    tape_layer,
                    tape_lap,
                }) => {
                    eng.set_cond_type(i, ConductorType::Ts);
                    eng.set_eps_r(i, *eps_r);
                    eng.set_ins_layer(i, g.radius_units, *ins_layer);
                    eng.set_dia_ins(i, g.radius_units, *dia_ins);
                    eng.set_dia_cable(i, g.radius_units, *dia_cable);
                    eng.set_dia_shield(i, g.radius_units, *dia_shield);
                    eng.set_tape_layer(i, g.radius_units, *tape_layer);
                    eng.set_tape_lap(i, *tape_lap);
                }
                None => {}
            }
        }

        // The engine's phase count is the number of phase positions, so a
        // geometry with fewer conductors than phases computes them all as
        // phases (pinned by `exec::tests::line_geometry_rules::nphases_defaults_to_three`).
        eng.set_nphases(phases);

        // Before the calc, reject bad conductor definitions (Pascal raises
        // `ELineGeometryProblem` and sets `SolutionAbort`). Leave `data_changed`
        // SET on this error path (Pascal clears it at line 968 before the check,
        // but its exception immediately halts the whole solve; the Result-based
        // port returns instead, so we must keep the geometry "dirty" or a later
        // rebuild would read the never-computed matrices and silently succeed).
        if let Some(msg) = eng.conductors_in_same_space() {
            return Err(format!("Error in {}: {msg}", self.error_subject()));
        }
        eng.calc(f, earth_model);
        if self.freduce {
            eng.reduce();
        }
        // Cleared only on a successful build (see the error path above).
        self.data_changed = false;
        Ok(())
    }

    /// Pascal `Get_Zmatrix[f, Lngth, Units]` (LineGeometry.pas:782-790):
    /// recompute when stale, then the engine's length/units-scaled series Z.
    pub fn z_matrix(
        &mut self,
        f: f64,
        length: f64,
        units: i32,
        earth_model: i32,
    ) -> Result<CMatrix, String> {
        if self.data_changed {
            self.update_line_geometry_data(f, earth_model)?;
        }
        let subject = self.error_subject();
        let eng = self
            .fline_data
            .as_mut()
            .ok_or_else(|| format!("{subject}: no conductors defined."))?;
        Ok(eng.z_matrix(f, length, units, earth_model))
    }

    /// Pascal `Get_YCmatrix[f, Lngth, Units]` (LineGeometry.pas:772-780):
    /// recompute when stale, then the engine's length/units-scaled shunt Yc.
    pub fn yc_matrix(
        &mut self,
        f: f64,
        length: f64,
        units: i32,
        earth_model: i32,
    ) -> Result<CMatrix, String> {
        if self.data_changed {
            self.update_line_geometry_data(f, earth_model)?;
        }
        let eng = self
            .fline_data
            .as_ref()
            .ok_or_else(|| format!("{}: no conductors defined.", self.error_subject()))?;
        Ok(eng.yc_matrix(length, units))
    }

    /// dss_capi 0.15.x `TLineObj.makeZFromGeometry`/`makeZFromSpacing`
    /// (Line.pas:2049-2051 / 2111-2113): push the consuming Line's medium
    /// permittivity + height offset into the Carson engine, in the exact upstream
    /// call order (`SetEpsRMedium`, then `SetHeightOffset`, then
    /// `SetUserHeightUnit`) — the last re-applies the offset in the new unit,
    /// which is what makes `HeightUnit=` mean anything here: the offset is
    /// stored under whatever unit the engine currently carries (the constructed
    /// `UNITS_M` on the first push into a fresh engine), and the unit set then
    /// re-reads the typed number under the declared one. Which number that is
    /// depends on the *outgoing* unit — see
    /// `LineConstants::set_user_height_unit`, whose doc works the two cases (a
    /// fresh engine at metres, and the persistent geometry engine re-entered on a
    /// later build) out in full. Each setter flags
    /// `rhoChanged`, so the next `z_matrix`/`yc_matrix` (or a still-pending
    /// `update_line_geometry_data`) recomputes. A no-op when no engine exists yet
    /// (Pascal's `lineConstants` is always allocated for a real geometry).
    pub fn set_line_constants_medium(
        &mut self,
        eps_r_medium: f64,
        height_offset: f64,
        height_units: i32,
    ) {
        if let Some(eng) = self.fline_data.as_mut() {
            eng.set_eps_r_medium(eps_r_medium);
            eng.set_height_offset(height_offset);
            eng.set_user_height_unit(height_units);
        }
    }

    /// Pascal `Get_RhoEarth` (`FLineData.rhoearth`; the engine default 100 when
    /// there is no engine yet).
    pub fn rho_earth(&self) -> f64 {
        self.fline_data.as_ref().map_or(100.0, |ld| ld.rho_earth())
    }

    /// Pascal `Set_RhoEarth` (`FLineData.RhoEarth := Value`).
    pub fn set_rho_earth(&mut self, value: f64) {
        if let Some(ld) = self.fline_data.as_mut() {
            ld.set_rho_earth(value);
        }
    }
}

/// The one error for conductors without a conductor object: `missing` holds
/// their 0-based slots, the message names them 1-based after `subject`.
fn missing_conductors_message(subject: &str, missing: &[usize]) -> String {
    let numbers: Vec<String> = missing.iter().map(|i| (i + 1).to_string()).collect();
    let (noun, verb, list) = match numbers.split_last() {
        Some((last, [])) => ("conductor", "has", last.clone()),
        Some((last, rest)) => (
            "conductors",
            "have",
            format!("{} and {last}", rest.join(", ")),
        ),
        None => ("conductor", "has", String::new()),
    };
    format!("{subject}: {noun} {list} {verb} no wire, cncable or tscable.")
}

/// `(NormAmps, EmergAmps)` of a conductor (Pascal `Wires[1].NormAmps/EmergAmps`),
/// whichever concrete catalog type it is.
fn conductor_norm_emerg(o: &ConductorObj) -> (Rating, Rating) {
    let (n, e, _, _) = o.amps();
    (n, e)
}

/// The weaker of the rating folded so far (`None` before the first phase
/// conductor) and the next phase conductor's: a rating that is not set wins,
/// else the lower number.
fn weaker_rating(seen: Option<Rating>, next: Rating) -> Rating {
    match (seen, next) {
        (None, r) => r,
        (Some(Rating::NotSet), _) | (_, Rating::NotSet) => Rating::NotSet,
        (Some(Rating::Set(a)), Rating::Set(b)) => Rating::Set(if b < a { b } else { a }),
    }
}
