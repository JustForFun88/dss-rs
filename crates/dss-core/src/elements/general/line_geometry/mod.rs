//! `LineGeometry` (`TLineGeometryObj`) — the multi-conductor overhead/cable
//! geometry a `Line` references to build its impedance via the Carson engine.
//! Port of Pascal `General/LineGeometry.pas`.
//!
//! A `DSS_OBJECT` catalog class (no terminals, no YPrim). It holds per-conductor
//! coordinate (`X`/`H`) and unit arrays plus a reference to each conductor's
//! catalog object (`WireData`/`CNData`/`TSData`), sized by `NConds`. Editing is a
//! small **state machine** keyed by the active conductor `Cond=`: `wire=`/
//! `cncable=`/`tscable=`/`x=`/`h=`/`units=` all write the slot selected by the
//! last `cond=`. The plural `wires=`/`cncables=`/`tscables=` forms write every
//! slot at once. `spacing=` copies an entire `LineSpacing`'s coordinates in.
//!
//! Referenced conductor/spacing objects are resolved and **snapshot-cloned** at
//! edit time (the WP4.2 `FetchLineCode` pattern), so the geometry owns the data
//! it needs. The held [`LineConstants`] engine (`FLineData`) is allocated/swapped
//! by [`LineGeometryObj::change_line_constants_type`] to track the active
//! conductor model; [`LineGeometryObj::update_line_geometry_data`] pushes every
//! conductor's geometry into it and runs the Carson `Calc` (plus a Kron `Reduce`
//! when `FReduce`), caching `Zmatrix`/`YCmatrix`. The [`LineGeometryObj::z_matrix`]
//! / [`LineGeometryObj::yc_matrix`] accessors recompute on demand when the
//! geometry is stale (`data_changed`); a `Line` consumes them in WP7.1 step 3.

#[cfg(test)]
mod tests;

use crate::elements::general::conductor_data::{
    CableGeom, CnDataObj, ConductorGeom, TsDataObj, WireDataObj, conductor_geom,
};
use crate::elements::general::line_spacing::LineSpacingObj;
use crate::elements::traits::ElemRef;
use crate::obj::base::{DssObjData, DssObject};
use crate::obj::props::{PropDef, PropFlags, define_properties};
use crate::support::cmatrix::CMatrix;
use crate::support::line_constants::LineConstants;

/// Pascal `LineUnits.UNITS_FT` — the `ft` ordinal; the value `FLastUnit` resets
/// to and the default coordinate unit.
const UNITS_FT: i32 = 5;

/// Pascal default `FLineType` (`oh`, the 1-based `LineTypeEnum` ordinal 1).
const LINETYPE_OH: i32 = 1;

define_properties! {
    class "LineGeometry", abbrev true, enums enums;
    1  NCONDS    => PropDef::integer("nconds")
        .flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO);
    2  NPHASES   => PropDef::integer("nphases").flags(PropFlags::NON_NEGATIVE);
    3  COND      => PropDef::integer("cond");
    4  WIRE      => PropDef::object_ref_class("WireData", "wire");
    5  X         => PropDef::double("x");
    6  H         => PropDef::double("h");
    7  UNITS     => PropDef::mapped_string_enum("units", enums.units);
    8  NORMAMPS  => PropDef::double("normamps");
    9  EMERGAMPS => PropDef::double("emergamps");
    10 REDUCE    => PropDef::boolean("Reduce");
    11 SPACING   => PropDef::object_ref_class("LineSpacing", "spacing");
    12 WIRES     => PropDef::object_ref_array("WireData", "wires");
    13 CNCABLE   => PropDef::object_ref_class("CNData", "cncable");
    14 TSCABLE   => PropDef::object_ref_class("TSData", "tscable");
    15 CNCABLES  => PropDef::object_ref_array("CNData", "cncables");
    16 TSCABLES  => PropDef::object_ref_array("TSData", "tscables");
    17 SEASONS   => PropDef::integer("Seasons").flags(PropFlags::SUPPRESS_JSON);
    18 RATINGS   => PropDef::double_array("Ratings", SEASONS);
    19 LINETYPE  => PropDef::mapped_string_enum("LineType", enums.line_type);
}

/// Pascal `ConductorChoice`: the per-conductor model a conductor uses. Selects
/// the [`LineConstants`] engine kind allocated by
/// [`LineGeometryObj::change_line_constants_type`] and routes the
/// `wires=`/`wire=` side effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConductorChoice {
    Unknown,
    Overhead,
    ConcentricNeutral,
    TapeShield,
}

/// `TLineGeometryObj`. Pascal stores the per-conductor data as 1-based
/// `pXxxArray`s of length `FNConds`; here they are plain 0-based `Vec`s (slot
/// `i` is conductor `i+1`). `FActiveCond` stays 1-based (the property value).
pub struct LineGeometryObj {
    data: DssObjData,
    fnconds: i32,
    fnphases: i32,
    factive_cond: i32,
    fphase_choice: Vec<ConductorChoice>,
    /// Snapshot-cloned conductor objects (`WireData`/`CNData`/`TSData`), one per
    /// conductor (`None` = not yet set, Pascal NIL).
    fwiredata: Vec<Option<Box<dyn DssObject>>>,
    fx: Vec<f64>,
    fy: Vec<f64>,
    funits: Vec<i32>,
    flast_unit: i32,
    freduce: bool,
    /// Pascal `FLineData`: the Carson engine, allocated/swapped by
    /// [`Self::change_line_constants_type`] to match the active conductor model
    /// and filled by [`Self::update_line_geometry_data`]. `None` until the first
    /// conductor exists (Pascal NIL when `FNConds = 0`).
    fline_data: Option<LineConstants>,
    /// Pascal `DataChanged`: set by any geometry-affecting edit so the matrix
    /// calc knows to recompute. `LineSpacing` has no such flag, so the geometry
    /// tracks its own staleness.
    data_changed: bool,
    norm_amps: f64,
    emerg_amps: f64,
    num_amp_ratings: i32,
    amp_ratings: Vec<f64>,
    fline_type: i32,
    /// Snapshot-cloned `LineSpacing` (`spacing=`), or `None`.
    line_spacing_obj: Option<Box<dyn DssObject>>,
}

impl Clone for LineGeometryObj {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            fnconds: self.fnconds,
            fnphases: self.fnphases,
            factive_cond: self.factive_cond,
            fphase_choice: self.fphase_choice.clone(),
            fwiredata: self
                .fwiredata
                .iter()
                .map(|o| o.as_ref().map(|b| b.clone_box()))
                .collect(),
            fx: self.fx.clone(),
            fy: self.fy.clone(),
            funits: self.funits.clone(),
            flast_unit: self.flast_unit,
            freduce: self.freduce,
            fline_data: self.fline_data.clone(),
            data_changed: self.data_changed,
            norm_amps: self.norm_amps,
            emerg_amps: self.emerg_amps,
            num_amp_ratings: self.num_amp_ratings,
            amp_ratings: self.amp_ratings.clone(),
            fline_type: self.fline_type,
            line_spacing_obj: self.line_spacing_obj.as_ref().map(|b| b.clone_box()),
        }
    }
}

impl std::fmt::Debug for LineGeometryObj {
    // The conductor/spacing slots are `Box<dyn DssObject>` (not `Debug`), so the
    // derived impl is unavailable; print the scalar geometry state instead.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LineGeometryObj")
            .field("name", &self.data.name())
            .field("nconds", &self.fnconds)
            .field("nphases", &self.fnphases)
            .field("active_cond", &self.factive_cond)
            .field("reduce", &self.freduce)
            .field("line_type", &self.fline_type)
            .finish_non_exhaustive()
    }
}

impl LineGeometryObj {
    pub fn new(name: impl Into<String>) -> Self {
        // Pascal `TLineGeometryObj.Create`: zero conductors/phases (no
        // allocation), ActiveCond=1, LastUnit=ft, LineType=oh, 1 amp rating
        // equal to NormAmps (0).
        Self {
            data: DssObjData::new(name.into().to_lowercase(), prop::NUM_PROPS),
            fnconds: 0,
            fnphases: 0,
            factive_cond: 1,
            fphase_choice: Vec::new(),
            fwiredata: Vec::new(),
            fx: Vec::new(),
            fy: Vec::new(),
            funits: Vec::new(),
            flast_unit: UNITS_FT,
            freduce: false,
            fline_data: None,
            data_changed: true,
            norm_amps: 0.0,
            emerg_amps: 0.0,
            num_amp_ratings: 1,
            amp_ratings: vec![0.0],
            fline_type: LINETYPE_OH,
            line_spacing_obj: None,
        }
    }

    /// 0-based index of the active conductor, or `None` when `FActiveCond` is out
    /// of `1..=FNConds` (e.g. `NConds=0`).
    fn active_index(&self) -> Option<usize> {
        if self.factive_cond >= 1 && self.factive_cond <= self.fnconds {
            Some((self.factive_cond - 1) as usize)
        } else {
            None
        }
    }

    /// Pascal `nconds` side effect: free the old data and re-init every
    /// per-conductor array to its default. (Pascal reallocates and then runs
    /// loops that unconditionally reset `FX=0`, `FY=0`, `FUnits=-1`,
    /// `FPhaseChoice=Overhead`, `FWireData=NIL` for all conductors — so the net
    /// effect is a full reset regardless of whether the count actually changed;
    /// `FActiveCond` resets to 1 and `FLastUnit` to ft.)
    fn realloc_conductors(&mut self) {
        let n = self.fnconds.max(0) as usize;
        self.fphase_choice = vec![ConductorChoice::Overhead; n];
        self.fwiredata = (0..n).map(|_| None).collect();
        self.fx = vec![0.0; n];
        self.fy = vec![0.0; n];
        self.funits = vec![-1; n];
        self.flast_unit = UNITS_FT;
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
    fn change_line_constants_type(&mut self, new_choice: ConductorChoice) {
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
    fn apply_spacing(&mut self) {
        let Some(spc) = self
            .line_spacing_obj
            .as_ref()
            .and_then(|b| b.as_any().downcast_ref::<LineSpacingObj>())
        else {
            return;
        };
        if self.fnconds == spc.nwires() {
            let units = spc.spacing_units();
            let xs = spc.xcoord().to_vec();
            let hs = spc.ycoord().to_vec();
            self.flast_unit = units;
            let n = self.fnconds.max(0) as usize;
            for i in 0..n {
                self.fx[i] = xs[i];
                self.fy[i] = hs[i];
                self.funits[i] = units;
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
    fn set_wires(&mut self, refs: &[(String, ElemRef, &dyn DssObject)]) {
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
            self.fwiredata[i - 1] = Some(refs[k].2.clone_box());
        }
        self.factive_cond = istop as i32;
    }

    /// Pascal `wire`/`cncable`/`tscable` side effect (active conductor) and the
    /// `wires`/`cncables`/`tscables` "traditional" branch (first conductor):
    /// default this geometry's ratings from the conductor's once, when unset.
    fn default_amps_from(&mut self, cond_index: usize) {
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

    /// Pascal `LineGeometryObj.Get_Nconds` (LineGeometry.pas:754): the
    /// *effective* conductor count a consuming `Line` adopts as its phase count
    /// — `FNPhases` when the geometry is Kron-reduced (the reduced matrices are
    /// `FNPhases × FNPhases`), otherwise the full `FNConds`.
    pub fn nconds(&self) -> i32 {
        if self.freduce {
            self.fnphases
        } else {
            self.fnconds
        }
    }

    /// Pascal `LineGeometryObj.NormAmps` (seeded from the first conductor unless
    /// set explicitly) — `TLineObj.FetchGeometryCode` copies it onto the Line.
    pub fn norm_amps(&self) -> f64 {
        self.norm_amps
    }

    /// Pascal `LineGeometryObj.EmergAmps`.
    pub fn emerg_amps(&self) -> f64 {
        self.emerg_amps
    }

    /// Pascal `LineGeometryObj.NumAmpRatings`.
    pub fn num_amp_ratings(&self) -> i32 {
        self.num_amp_ratings
    }

    /// Pascal `LineGeometryObj.AmpRatings`.
    pub fn amp_ratings(&self) -> &[f64] {
        &self.amp_ratings
    }

    /// Pascal `LineGeometryObj.FLineType`.
    pub fn line_type(&self) -> i32 {
        self.fline_type
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

impl DssObject for LineGeometryObj {
    fn data(&self) -> &DssObjData {
        &self.data
    }
    fn data_mut(&mut self) -> &mut DssObjData {
        &mut self.data
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn get_i32(&self, idx: usize) -> i32 {
        match idx {
            prop::NCONDS => self.fnconds,
            prop::NPHASES => self.fnphases,
            prop::COND => self.factive_cond,
            // Per-conductor unit (active conductor); falls back to FLastUnit when
            // there is no active conductor (NConds=0 — the oracle raises here, so
            // this value is never compared).
            prop::UNITS => self
                .active_index()
                .map_or(self.flast_unit, |a| self.funits[a]),
            prop::SEASONS => self.num_amp_ratings,
            prop::LINETYPE => self.fline_type,
            _ => unreachable!("LineGeometry has no integer at {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        match idx {
            prop::NCONDS => self.fnconds = value,
            prop::NPHASES => self.fnphases = value,
            // Pascal `set_ActiveCond`: only move within `1..=FNConds`
            // (out-of-range is silently ignored). The sticky-unit default is in
            // the `cond` side effect.
            prop::COND => {
                if value > 0 && value <= self.fnconds {
                    self.factive_cond = value;
                }
            }
            prop::UNITS => {
                if let Some(a) = self.active_index() {
                    self.funits[a] = value;
                }
            }
            prop::SEASONS => self.num_amp_ratings = value,
            prop::LINETYPE => self.fline_type = value,
            _ => unreachable!("LineGeometry has no integer at {idx}"),
        }
    }

    fn get_f64(&self, idx: usize) -> f64 {
        match idx {
            prop::X => self.active_index().map_or(0.0, |a| self.fx[a]),
            prop::H => self.active_index().map_or(0.0, |a| self.fy[a]),
            prop::NORMAMPS => self.norm_amps,
            prop::EMERGAMPS => self.emerg_amps,
            _ => unreachable!("LineGeometry has no double at {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        match idx {
            prop::X => {
                if let Some(a) = self.active_index() {
                    self.fx[a] = value;
                }
            }
            prop::H => {
                if let Some(a) = self.active_index() {
                    self.fy[a] = value;
                }
            }
            prop::NORMAMPS => self.norm_amps = value,
            prop::EMERGAMPS => self.emerg_amps = value,
            _ => unreachable!("LineGeometry has no double at {idx}"),
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
                .map(|o| o.data().name().to_string())
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

    fn set_object_ref(
        &mut self,
        idx: usize,
        _name: String,
        resolved: Option<(ElemRef, &dyn DssObject)>,
    ) {
        let cloned = resolved.map(|(_, o)| o.clone_box());
        match idx {
            prop::WIRE | prop::CNCABLE | prop::TSCABLE => {
                if let Some(a) = self.active_index() {
                    self.fwiredata[a] = cloned;
                }
            }
            prop::SPACING => self.line_spacing_obj = cloned,
            _ => unreachable!("LineGeometry has no object reference at {idx}"),
        }
    }

    fn set_object_ref_array(&mut self, idx: usize, refs: &[(String, ElemRef, &dyn DssObject)]) {
        debug_assert!(matches!(idx, prop::WIRES | prop::CNCABLES | prop::TSCABLES));
        self.set_wires(refs);
    }
    fn get_object_ref_names(&self, idx: usize) -> Vec<String> {
        debug_assert!(matches!(idx, prop::WIRES | prop::CNCABLES | prop::TSCABLES));
        self.fwiredata
            .iter()
            .map(|o| {
                o.as_ref()
                    .map(|o| o.data().name().to_string())
                    .unwrap_or_default()
            })
            .collect()
    }

    fn get_f64_array(&self, idx: usize) -> Option<&[f64]> {
        debug_assert_eq!(idx, prop::RATINGS);
        (!self.amp_ratings.is_empty()).then_some(self.amp_ratings.as_slice())
    }
    fn set_f64_array(&mut self, idx: usize, value: Vec<f64>) {
        debug_assert_eq!(idx, prop::RATINGS);
        self.amp_ratings = value;
    }

    fn side_effects(&mut self, idx: usize, _prev_int: i32) {
        // Pascal `TLineGeometryObj.PropertySideEffects`, in the same three case
        // blocks.
        match idx {
            prop::NPHASES => {
                // Mirror `FLineData.Nphases := FNphases`, clamped to `FNConds`.
                // (UpdateLineGeometryData later re-sets it unclamped before Calc.)
                if let Some(ld) = self.fline_data.as_mut() {
                    let np = self.fnphases.min(self.fnconds).max(0);
                    ld.set_nphases(np as usize);
                }
            }
            prop::COND => {
                // sticky unit: a fresh conductor inherits the last-used unit
                if let Some(a) = self.active_index()
                    && self.funits[a] == -1
                {
                    self.funits[a] = self.flast_unit;
                }
            }
            prop::WIRE => {
                if self
                    .active_index()
                    .is_some_and(|a| self.fphase_choice[a] == ConductorChoice::Unknown)
                {
                    self.change_line_constants_type(ConductorChoice::Overhead);
                }
            }
            prop::UNITS => {
                if let Some(a) = self.active_index() {
                    self.flast_unit = self.funits[a];
                }
            }
            prop::CNCABLE | prop::CNCABLES => {
                self.change_line_constants_type(ConductorChoice::ConcentricNeutral);
            }
            prop::TSCABLE | prop::TSCABLES => {
                self.change_line_constants_type(ConductorChoice::TapeShield);
            }
            prop::NCONDS => self.realloc_conductors(),
            prop::SPACING => self.apply_spacing(),
            _ => {}
        }

        // Second block: default this geometry's ratings from its conductors.
        match idx {
            prop::WIRES | prop::CNCABLES | prop::TSCABLES => {
                // "Traditional" branch reads the first conductor (overhead
                // istart=1); the buried-neutral istart shift only changes which
                // slots were filled, not the conductor that seeds the ratings.
                self.default_amps_from(0);
            }
            prop::WIRE | prop::CNCABLE | prop::TSCABLE => {
                // Pascal: `conductorObj := FWireData[ActiveCond]`; if assigned,
                // the first conductor defaults the geometry ratings; if NIL (the
                // name did not resolve — the generic ObjectRef parse already
                // logged its 401), log the conductor-not-defined 10103.
                if let Some(a) = self.active_index() {
                    if self.fwiredata[a].is_some() {
                        if self.factive_cond == 1 {
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
                self.amp_ratings.resize(n, 0.0);
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

    fn make_like(&mut self, other: &dyn DssObject) {
        self.data.copy_prp_sequence_from(other.data());
        if let Some(o) = other.as_any().downcast_ref::<LineGeometryObj>() {
            // Pascal `MakeLike`: `NConds := Other.NWires` runs the nconds side
            // effect (full reset), then every per-conductor array is copied.
            self.fnconds = o.fnconds;
            self.realloc_conductors();
            self.fnphases = o.fnphases;
            self.line_spacing_obj = o.line_spacing_obj.as_ref().map(|b| b.clone_box());
            self.fline_type = o.fline_type;
            self.fphase_choice.clone_from(&o.fphase_choice);
            self.fwiredata = o
                .fwiredata
                .iter()
                .map(|c| c.as_ref().map(|b| b.clone_box()))
                .collect();
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

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}
