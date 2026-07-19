//! The Carson engine path: `UpdateLineGeometryData` (fill `FLineData`, run the
//! `Calc`/`Reduce`) and the on-demand `Get_Zmatrix`/`Get_YCmatrix` accessors,
//! plus the `RhoEarth` getter/setter.

use crate::elements::general::conductor_data::{
    CableGeom, ConductorGeom, ConductorKind, conductor_geom,
};
use crate::elements::general::line_spacing::LineSpacingObj;
use crate::obj::base::DssObject;
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
        wires: &[Option<Box<dyn DssObject>>],
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
        // whose ORIGINAL index is a phase position. (The pre-fix code sized to
        // `Spc.NWires` and copied index-aligned, so a NIL slot reached the Carson
        // calc and aborted "WireData is not correctly initialized".) For a list
        // with no NIL this is byte-identical to the old path (`actualNConds ==
        // NWires`, `j == i`).
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
        self.line_spacing_obj = Some(Box::new(spc.clone()));
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
            match o.as_conductor().map(|c| c.conductor_kind()) {
                Some(ConductorKind::Cn) => new_choice = ConductorChoice::ConcentricNeutral,
                Some(ConductorKind::Ts) => new_choice = ConductorChoice::TapeShield,
                _ => {}
            }
        }
        self.change_line_constants_type(new_choice);

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

        // Skip-NIL copy (r4133 :1220-1242): contiguous conductor `j`, spacing
        // coordinates indexed by the ORIGINAL position `i`. NormAmps/EmergAmps are
        // the running MINIMUM over the PHASE conductors (dss_capi 0.15.x D3 /
        // r4133 :1235-1239, `j <= FNPhases`) — a phase with a lower rating governs
        // the line; conductor 1's rating no longer wins by default.
        let units = spc.spacing_units();
        let xs = spc.xcoord();
        let hs = spc.ycoord();
        let nph = self.fnphases.max(0) as usize;
        self.norm_amps = 0.0;
        self.emerg_amps = 0.0;
        let mut j = 0usize; // 0-based contiguous conductor index (Pascal 1-based)
        for (i, o) in wires.iter().take(nwires).enumerate() {
            let Some(o) = o.as_ref() else { continue };
            self.fwiredata[j] = Some(o.clone_box());
            if !self.equivalent_spacing {
                self.fx[j] = xs[i];
                self.fy[j] = hs[i];
                self.funits[j] = units;
            }
            let (cn, ce) = conductor_norm_emerg(o.as_ref());
            // 0-based `j < nph` == Pascal 1-based `(j+1) <= FNPhases`.
            if (cn < self.norm_amps || self.norm_amps == 0.0) && j < nph {
                self.norm_amps = cn;
                self.emerg_amps = ce;
            }
            j += 1;
        }
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
    /// Returns `Err` for the two Pascal abort paths: a NIL conductor slot
    /// (`raise Exception`, "WireData is not correctly initialized") and a failed
    /// geometry check (`ELineGeometryProblem` + `SolutionAbort`).
    pub fn update_line_geometry_data(&mut self, f: f64, earth_model: i32) -> Result<(), String> {
        let n = self.fnconds.max(0) as usize;

        // Pascal reads each `FWireData[i]`'s fields directly; gather them first
        // (immutable borrows) so the engine fill below can borrow `FLineData`.
        let mut geoms: Vec<ConductorGeom> = Vec::with_capacity(n);
        for i in 0..n {
            let g = self
                .fwiredata
                .get(i)
                .and_then(|o| o.as_ref())
                .and_then(|o| conductor_geom(o.as_ref()))
                .ok_or_else(|| {
                    format!(
                        "LineGeometry.{}: WireData is not correctly initialized. \
                         Check the object definition.",
                        self.data.name()
                    )
                })?;
            geoms.push(g);
        }

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
            // Pascal skips SetX/SetY under equivalent spacing (coordinates unused).
            if !self.equivalent_spacing {
                eng.set_x(i, self.funits[i], self.fx[i]);
                eng.set_y(i, self.funits[i], self.fy[i]);
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

        // Pascal sets `FLineData.Nphases := FNphases` here, unclamped (the
        // `nphases` side effect's `> FNConds` clamp is transient).
        eng.set_nphases(self.fnphases.max(0) as usize);

        // Before the calc, reject bad conductor definitions (Pascal raises
        // `ELineGeometryProblem` and sets `SolutionAbort`). Leave `data_changed`
        // SET on this error path (Pascal clears it at line 968 before the check,
        // but its exception immediately halts the whole solve; the Result-based
        // port returns instead, so we must keep the geometry "dirty" or a later
        // rebuild would read the never-computed matrices and silently succeed).
        if let Some(msg) = eng.conductors_in_same_space() {
            return Err(format!("Error in LineGeometry.{}: {msg}", self.data.name()));
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
        let eng = self
            .fline_data
            .as_mut()
            .ok_or_else(|| format!("LineGeometry.{}: no conductors defined.", self.data.name()))?;
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
            .ok_or_else(|| format!("LineGeometry.{}: no conductors defined.", self.data.name()))?;
        Ok(eng.yc_matrix(length, units))
    }

    /// dss_capi 0.15.x `TLineObj.makeZFromGeometry`/`makeZFromSpacing`
    /// (Line.pas:2049-2051 / 2111-2113): push the consuming Line's medium
    /// permittivity + height offset into the Carson engine, in the exact upstream
    /// call order (`SetEpsRMedium`, then `SetHeightOffset`, then
    /// `SetUserHeightUnit`) — the last re-applies the offset in the new unit
    /// (the `set_user_height_unit` re-conversion quirk). Each setter flags
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

/// `(NormAmps, EmergAmps)` of a conductor (Pascal `Wires[1].NormAmps/EmergAmps`),
/// whichever concrete catalog type it is.
fn conductor_norm_emerg(o: &dyn DssObject) -> (f64, f64) {
    match o.as_conductor() {
        Some(c) => {
            let (n, e, _, _) = c.amps();
            (n, e)
        }
        None => (0.0, 0.0),
    }
}
