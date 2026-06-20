//! The Carson engine path: `UpdateLineGeometryData` (fill `FLineData`, run the
//! `Calc`/`Reduce`) and the on-demand `Get_Zmatrix`/`Get_YCmatrix` accessors,
//! plus the `RhoEarth` getter/setter.

use crate::elements::general::conductor_data::{CableGeom, ConductorGeom, conductor_geom};
use crate::support::cmatrix::CMatrix;

use super::LineGeometryObj;

impl LineGeometryObj {
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

        for (i, g) in geoms.iter().enumerate() {
            eng.set_x(i, self.funits[i], self.fx[i]);
            eng.set_y(i, self.funits[i], self.fy[i]);
            eng.set_radius(i, g.radius_units, g.radius);
            eng.set_capradius(i, g.radius_units, g.cap_radius);
            eng.set_gmr(i, g.gmr_units, g.gmr);
            eng.set_rdc(i, g.res_units, g.rdc);
            eng.set_rac(i, g.res_units, g.rac);
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
                }) => {
                    eng.set_eps_r(i, *eps_r);
                    eng.set_ins_layer(i, g.radius_units, *ins_layer);
                    eng.set_dia_ins(i, g.radius_units, *dia_ins);
                    eng.set_dia_cable(i, g.radius_units, *dia_cable);
                    eng.set_k_strand(i, *k_strand);
                    eng.set_dia_strand(i, g.radius_units, *dia_strand);
                    eng.set_gmr_strand(i, g.gmr_units, *gmr_strand);
                    eng.set_r_strand(i, g.res_units, *r_strand);
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
        self.data_changed = false;

        // Before the calc, reject bad conductor definitions (Pascal raises
        // `ELineGeometryProblem` and sets `SolutionAbort`).
        if let Some(msg) = eng.conductors_in_same_space() {
            return Err(format!("Error in LineGeometry.{}: {msg}", self.data.name()));
        }
        eng.calc(f, earth_model);
        if self.freduce {
            eng.reduce();
        }
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
