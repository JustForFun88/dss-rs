//! The Carson engine path: `UpdateLineGeometryData` (fill `FLineData`, run the
//! `Calc`/`Reduce`) and the on-demand `Get_Zmatrix`/`Get_YCmatrix` accessors,
//! plus the `RhoEarth` getter/setter.

use crate::elements::general::conductor_data::{
    CableGeom, CnDataObj, ConductorGeom, TsDataObj, conductor_geom,
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
    pub fn load_spacing_and_wires(
        &mut self,
        spc: &LineSpacingObj,
        wires: &[Option<Box<dyn DssObject>>],
        f: f64,
        earth_model: i32,
    ) -> Result<(), String> {
        // `NConds := Spc.NWires` runs the nconds side effect (full reset/realloc).
        self.fnconds = spc.nwires();
        self.realloc_conductors();
        self.fnphases = spc.nphases();
        self.line_spacing_obj = Some(Box::new(spc.clone()));
        if self.fnconds > self.fnphases {
            self.freduce = true;
        }

        let n = self.fnconds.max(0) as usize;
        // Pick the conductor model: any CN ⇒ ConcentricNeutral, any TS ⇒
        // TapeShield (TS wins if both present, mirroring Pascal's sequential ifs),
        // else Overhead.
        let mut new_choice = ConductorChoice::Overhead;
        for o in wires.iter().take(n).flatten() {
            let any = o.as_any();
            if any.is::<CnDataObj>() {
                new_choice = ConductorChoice::ConcentricNeutral;
            }
            if any.is::<TsDataObj>() {
                new_choice = ConductorChoice::TapeShield;
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

        let units = spc.spacing_units();
        let xs = spc.xcoord();
        let hs = spc.ycoord();
        for i in 0..n {
            self.fwiredata[i] = wires[i].as_ref().map(|o| o.clone_box());
            if !self.equivalent_spacing {
                self.fx[i] = xs[i];
                self.fy[i] = hs[i];
                self.funits[i] = units;
            }
        }
        self.data_changed = true;
        // NormAmps/EmergAmps := Wires[1].* (conductor 1).
        if let Some(o) = wires.first().and_then(|o| o.as_ref()) {
            let (cn, ce) = conductor_norm_emerg(o.as_ref());
            self.norm_amps = cn;
            self.emerg_amps = ce;
        }

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
    use crate::elements::general::conductor_data::WireDataObj;
    let any = o.as_any();
    if let Some(w) = any.downcast_ref::<WireDataObj>() {
        let (n, e, _, _) = w.amps();
        (n, e)
    } else if let Some(c) = any.downcast_ref::<CnDataObj>() {
        let (n, e, _, _) = c.amps();
        (n, e)
    } else if let Some(t) = any.downcast_ref::<TsDataObj>() {
        let (n, e, _, _) = t.amps();
        (n, e)
    } else {
        (0.0, 0.0)
    }
}
