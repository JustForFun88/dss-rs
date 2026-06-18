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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elements::general::conductor_data::wire_data;
    use crate::obj::dss_enum::EnumRegistry;
    use crate::obj::props::{ClassProps, PropEngine};
    use dss_parser::{Parser, ParserVars};

    /// Apply a scalar property edit through the property engine (no foreign view
    /// needed — object references are driven directly by [`set_ref`]/
    /// [`set_ref_array`] below). The real parse + object-reference *resolution*
    /// path is covered end-to-end against the oracle by the `props_roundtrip`
    /// golden for the scalar `wire`/`cncable`/`tscable`, the `wires` array, and
    /// `spacing`; the array-resolution abort is covered by the exec test
    /// `line_geometry_undefined_wire_in_array_aborts`. (The plural `cncables=`/
    /// `tscables=` forms are not yet golden-pinned — see STATUS: the oracle's
    /// post-plural active-conductor value diverges and is under investigation.)
    fn scalar(cls: &ClassProps, obj: &mut dyn DssObject, name: &str, value: &str) -> Vec<String> {
        let enums = EnumRegistry::new();
        let mut parser = Parser::new();
        let vars = ParserVars::new();
        let mut errors = Vec::new();
        let idx = cls.property_index(name).expect("known property");
        let mut eng = PropEngine {
            parser: &mut parser,
            vars: &vars,
            enums: &enums,
            errors: &mut errors,
            foreign: None,
        };
        cls.edit_property(obj, idx, value, &mut eng).unwrap();
        errors.extend(obj.data_mut().take_errors());
        errors
    }

    /// Mirror the executive's `edit_property` for a single object reference: set
    /// the (already-resolved) reference, record the set order, run side effects.
    fn set_ref(cls: &ClassProps, obj: &mut dyn DssObject, name: &str, target: &dyn DssObject) {
        let idx = cls.property_index(name).expect("known property");
        let r = ElemRef { cls: 0, idx: 0 };
        obj.set_object_ref(idx, target.data().name().to_string(), Some((r, target)));
        obj.data_mut().set_as_next_seq(idx);
        obj.side_effects(idx, 0);
    }

    /// Mirror `edit_property` for an object-reference array (`wires=`/...).
    fn set_ref_array(
        cls: &ClassProps,
        obj: &mut dyn DssObject,
        name: &str,
        targets: &[&dyn DssObject],
    ) {
        let idx = cls.property_index(name).expect("known property");
        let r = ElemRef { cls: 0, idx: 0 };
        let refs: Vec<(String, ElemRef, &dyn DssObject)> = targets
            .iter()
            .map(|t| (t.data().name().to_string(), r, *t))
            .collect();
        obj.set_object_ref_array(idx, &refs);
        obj.data_mut().set_as_next_seq(idx);
        obj.side_effects(idx, 0);
    }

    fn build_wire(name: &str, edits: &[(&str, &str)]) -> WireDataObj {
        let enums = EnumRegistry::new();
        let cls = wire_data::class_props(&enums);
        let mut obj = WireDataObj::new(name);
        for (n, v) in edits {
            scalar(&cls, &mut obj, n, v);
        }
        obj
    }

    fn build_spacing(edits: &[(&str, &str)]) -> LineSpacingObj {
        let enums = EnumRegistry::new();
        let cls = crate::elements::general::line_spacing::class_props(&enums);
        let mut obj = LineSpacingObj::new("sp");
        for (n, v) in edits {
            scalar(&cls, &mut obj, n, v);
        }
        obj
    }

    fn get(cls: &ClassProps, obj: &dyn DssObject, name: &str) -> String {
        let enums = EnumRegistry::new();
        let idx = cls.property_index(name).unwrap();
        cls.get_value(obj, idx, &enums)
    }

    #[test]
    fn defaults() {
        // Pascal Create: nconds=0, nphases=0, cond=1, reduce=No, linetype=oh,
        // empty object-ref arrays render `[]`, ratings `[ 0]`.
        let enums = EnumRegistry::new();
        let cls = class_props(&enums);
        let obj = LineGeometryObj::new("g1");
        assert_eq!(get(&cls, &obj, "nconds"), "0");
        assert_eq!(get(&cls, &obj, "nphases"), "0");
        assert_eq!(get(&cls, &obj, "cond"), "1");
        assert_eq!(get(&cls, &obj, "reduce"), "No");
        assert_eq!(get(&cls, &obj, "wires"), "[]");
        assert_eq!(get(&cls, &obj, "cncables"), "[]");
        assert_eq!(get(&cls, &obj, "normamps"), "0");
        assert_eq!(get(&cls, &obj, "ratings"), "[ 0]");
        assert_eq!(get(&cls, &obj, "linetype"), "oh");
    }

    #[test]
    fn cond_wire_state_machine() {
        // The classic per-conductor edit: `cond=N wire=.. x=.. h=.. units=..`.
        // Only the active conductor's scalars are visible via `?`; the full
        // assignment shows through the `wires` array. NormAmps/EmergAmps default
        // from the first conductor.
        let enums = EnumRegistry::new();
        let cls = class_props(&enums);
        let acsr = build_wire("acsr", &[("normamps", "530"), ("radius", "0.0306")]);
        let mut g = LineGeometryObj::new("g1");
        scalar(&cls, &mut g, "nconds", "3");
        scalar(&cls, &mut g, "nphases", "3");
        scalar(&cls, &mut g, "cond", "1");
        set_ref(&cls, &mut g, "wire", &acsr);
        scalar(&cls, &mut g, "x", "-1.2909");
        scalar(&cls, &mut g, "h", "13.716");
        scalar(&cls, &mut g, "units", "m");
        scalar(&cls, &mut g, "cond", "2");
        set_ref(&cls, &mut g, "wire", &acsr);
        scalar(&cls, &mut g, "x", "0");
        scalar(&cls, &mut g, "h", "13.716");
        scalar(&cls, &mut g, "cond", "3");
        set_ref(&cls, &mut g, "wire", &acsr);
        scalar(&cls, &mut g, "x", "1.2909");
        scalar(&cls, &mut g, "h", "13.716");

        assert_eq!(get(&cls, &g, "cond"), "3"); // last active
        assert_eq!(get(&cls, &g, "wire"), "acsr");
        assert_eq!(get(&cls, &g, "x"), "1.2909");
        assert_eq!(get(&cls, &g, "h"), "13.716");
        assert_eq!(get(&cls, &g, "units"), "m"); // sticky from cond 1
        assert_eq!(get(&cls, &g, "normamps"), "530"); // defaulted from acsr
        assert_eq!(get(&cls, &g, "emergamps"), "795"); // 1.5 × 530 on the wire
        assert_eq!(get(&cls, &g, "wires"), "[acsr, acsr, acsr]");
        assert_eq!(get(&cls, &g, "cncables"), "[acsr, acsr, acsr]");
    }

    #[test]
    fn wires_array_sets_active_to_last() {
        // The plural `wires=` form fills every slot and leaves ActiveCond at the
        // last conductor (Pascal `SetWires`).
        let enums = EnumRegistry::new();
        let cls = class_props(&enums);
        let acsr = build_wire("acsr", &[("normamps", "530")]);
        let mut g = LineGeometryObj::new("g1");
        scalar(&cls, &mut g, "nconds", "3");
        scalar(&cls, &mut g, "nphases", "3");
        set_ref_array(&cls, &mut g, "wires", &[&acsr, &acsr, &acsr]);
        assert_eq!(get(&cls, &g, "cond"), "3");
        assert_eq!(get(&cls, &g, "wires"), "[acsr, acsr, acsr]");
        assert_eq!(get(&cls, &g, "normamps"), "530");
    }

    #[test]
    fn wires_wrong_count_errors() {
        // A count mismatch logs the "Unexpected number" error and fills nothing.
        let enums = EnumRegistry::new();
        let cls = class_props(&enums);
        let acsr = build_wire("acsr", &[]);
        let mut g = LineGeometryObj::new("g1");
        scalar(&cls, &mut g, "nconds", "3");
        scalar(&cls, &mut g, "nphases", "3");
        set_ref_array(&cls, &mut g, "wires", &[&acsr, &acsr]);
        let errs = g.data_mut().take_errors();
        assert!(
            errs.iter().any(|e| e.contains("Unexpected number (2)")),
            "{errs:?}"
        );
        assert_eq!(get(&cls, &g, "wires"), "[, , ]"); // nothing filled
    }

    #[test]
    fn spacing_copies_coordinates() {
        // `spacing=` copies the LineSpacing's coordinates/units into every
        // conductor.
        let enums = EnumRegistry::new();
        let cls = class_props(&enums);
        let acsr = build_wire("acsr", &[("normamps", "530")]);
        let sp = build_spacing(&[
            ("nconds", "3"),
            ("nphases", "3"),
            ("x", "-1.2909 0 1.2909"),
            ("h", "28.6 28.6 28.6"),
            ("units", "ft"),
        ]);
        let mut g = LineGeometryObj::new("g1");
        scalar(&cls, &mut g, "nconds", "3");
        scalar(&cls, &mut g, "nphases", "3");
        set_ref(&cls, &mut g, "spacing", &sp);
        set_ref_array(&cls, &mut g, "wires", &[&acsr, &acsr, &acsr]);
        assert!(g.data_mut().take_errors().is_empty());
        assert_eq!(get(&cls, &g, "spacing"), "sp");
        assert_eq!(get(&cls, &g, "cond"), "3");
        assert_eq!(get(&cls, &g, "x"), "1.2909"); // cond 3 from spacing
        assert_eq!(get(&cls, &g, "h"), "28.6");
        assert_eq!(get(&cls, &g, "units"), "ft");
    }

    #[test]
    fn spacing_wrong_wire_count_errors() {
        // A spacing with a different wire count logs error 10103.
        let enums = EnumRegistry::new();
        let cls = class_props(&enums);
        let sp = build_spacing(&[
            ("nconds", "2"),
            ("nphases", "2"),
            ("x", "0 1"),
            ("h", "10 10"),
        ]);
        let mut g = LineGeometryObj::new("g1");
        scalar(&cls, &mut g, "nconds", "3");
        scalar(&cls, &mut g, "nphases", "3");
        set_ref(&cls, &mut g, "spacing", &sp);
        let errs = g.data_mut().take_errors();
        assert!(
            errs.iter().any(|e| e.contains("wrong number of wires")),
            "{errs:?}"
        );
    }

    #[test]
    fn make_like_copies_geometry_and_resets_active() {
        let enums = EnumRegistry::new();
        let cls = class_props(&enums);
        let acsr = build_wire("acsr", &[("normamps", "530")]);
        let mut src = LineGeometryObj::new("base");
        scalar(&cls, &mut src, "nconds", "3");
        scalar(&cls, &mut src, "nphases", "3");
        scalar(&cls, &mut src, "cond", "1");
        set_ref(&cls, &mut src, "wire", &acsr);
        scalar(&cls, &mut src, "x", "-1.29");
        scalar(&cls, &mut src, "h", "13.7");
        scalar(&cls, &mut src, "units", "m");
        scalar(&cls, &mut src, "cond", "2");
        set_ref(&cls, &mut src, "wire", &acsr);
        scalar(&cls, &mut src, "x", "0");
        scalar(&cls, &mut src, "h", "13.7");
        scalar(&cls, &mut src, "cond", "3");
        set_ref(&cls, &mut src, "wire", &acsr);
        scalar(&cls, &mut src, "x", "1.29");
        scalar(&cls, &mut src, "h", "13.7");
        scalar(&cls, &mut src, "reduce", "y");

        let mut dst = LineGeometryObj::new("g1");
        dst.make_like(&src);
        assert_eq!(get(&cls, &dst, "nconds"), "3");
        assert_eq!(get(&cls, &dst, "cond"), "1"); // reset by the nconds side effect
        assert_eq!(get(&cls, &dst, "x"), "-1.29"); // cond 1
        assert_eq!(get(&cls, &dst, "units"), "m");
        assert_eq!(get(&cls, &dst, "reduce"), "Yes");
        assert_eq!(get(&cls, &dst, "wires"), "[acsr, acsr, acsr]");
        assert_eq!(get(&cls, &dst, "normamps"), "530");
    }

    #[test]
    fn wire_undefined_pushes_not_defined_error() {
        // An unresolved `wire=` leaves the active conductor NIL; the side effect
        // logs the Pascal 10103 "object was not defined" (the generic ObjectRef
        // parse logs its own 401 "not found" separately, upstream).
        let enums = EnumRegistry::new();
        let cls = class_props(&enums);
        let mut g = LineGeometryObj::new("g1");
        scalar(&cls, &mut g, "nconds", "3");
        scalar(&cls, &mut g, "nphases", "3");
        scalar(&cls, &mut g, "cond", "1");
        // Mirror the executive's edit for an unresolved scalar ObjectRef: NIL.
        let idx = cls.property_index("wire").unwrap();
        g.set_object_ref(idx, "doesnotexist".to_string(), None);
        g.data_mut().set_as_next_seq(idx);
        g.side_effects(idx, 0);
        let errs = g.data_mut().take_errors();
        assert!(
            errs.iter()
                .any(|e| e.contains("was not defined. Must be previously defined")),
            "{errs:?}"
        );
        assert_eq!(get(&cls, &g, "wire"), ""); // still empty
    }

    #[test]
    fn wires_array_defaults_multi_season_ratings() {
        // The `wires=` array branch defaults the geometry's Seasons/Ratings from
        // the first conductor when its own are unset (the NumAmpRatings>1 /
        // AmpRatings-copy branches of `default_amps_from`).
        let enums = EnumRegistry::new();
        let cls = class_props(&enums);
        let w4 = build_wire(
            "w4",
            &[
                ("normamps", "530"),
                ("Seasons", "4"),
                ("Ratings", "400 450 500 550"),
            ],
        );
        let mut g = LineGeometryObj::new("g1");
        scalar(&cls, &mut g, "nconds", "3");
        scalar(&cls, &mut g, "nphases", "3");
        set_ref_array(&cls, &mut g, "wires", &[&w4, &w4, &w4]);
        assert_eq!(get(&cls, &g, "seasons"), "4");
        assert_eq!(get(&cls, &g, "ratings"), "[ 400 450 500 550]");
        assert_eq!(get(&cls, &g, "normamps"), "530");
        assert_eq!(get(&cls, &g, "emergamps"), "795");
    }

    // ----- matrix wiring (UpdateLineGeometryData / CalcMatrices) -------------
    //
    // These drive the full LineGeometry object path (nconds/cond/wire/x/h/units)
    // and assert the resulting Z/Yc against the *same* dss-python oracle
    // references the Carson-engine unit tests pin (support/line_constants/tests),
    // proving the object→engine wiring (units, radius/GMR/Rdc/Rac, cable extras,
    // Nphases, Reduce) is correct end to end.

    use crate::elements::general::conductor_data::{CnDataObj, TsDataObj, cn_data, ts_data};
    use crate::support::cmatrix::CMatrix;
    use crate::support::line_constants::DERI;

    const M_UNIT: i32 = 4; // LineUnits::Meter code
    /// Truncated `Twopi * 60` — the engine's `Fw` at 60 Hz (matches the oracle).
    #[allow(clippy::approx_constant)] // truncated upstream `Twopi`, mirrors the engine
    const W60: f64 = 6.283185307 * 60.0;

    fn assert_close(got: f64, want: f64, what: &str) {
        let tol = 1e-8 * want.abs().max(1e-12);
        assert!(
            (got - want).abs() <= tol,
            "{what}: got {got:.12e}, want {want:.12e}"
        );
    }

    fn assert_z(z: &CMatrix, z_ref: &[(f64, f64)], n: usize) {
        for i in 0..n {
            for j in 0..n {
                let (re, im) = z_ref[i * n + j];
                let g = z.get(i, j);
                assert_close(g.re, re, &format!("Z[{i}][{j}].re"));
                assert_close(g.im, im, &format!("Z[{i}][{j}].im"));
            }
        }
    }

    /// The shared 3-wire overhead wire (SI): rac = 3e-4 ohm/m, gmr = 0.005 m,
    /// radius = 0.01 m. Rdc defaults from Rac (/1.02); capradius from radius.
    fn build_si_wire() -> WireDataObj {
        build_wire(
            "w",
            &[
                ("runits", "m"),
                ("gmrunits", "m"),
                ("radunits", "m"),
                ("rac", "0.0003"),
                ("gmrac", "0.005"),
                ("radius", "0.01"),
            ],
        )
    }

    fn build_si_cn() -> CnDataObj {
        let enums = EnumRegistry::new();
        let cls = cn_data::class_props(&enums);
        let mut obj = CnDataObj::new("cn");
        // Same core conductor + insulation/strand data as the engine `build_cn`.
        for (n, v) in &[
            ("runits", "m"),
            ("gmrunits", "m"),
            ("radunits", "m"),
            ("rdc", "0.0001"),
            ("rac", "0.000105"),
            ("radius", "0.005"),
            ("gmrac", "0.004"),
            ("epsr", "2.3"),
            ("inslayer", "0.004"),
            ("diains", "0.022"),
            ("diacable", "0.030"),
            ("k", "16"),
            ("diastrand", "0.001"),
            ("gmrstrand", "0.0004"),
            ("rstrand", "0.002"),
        ] {
            scalar(&cls, &mut obj, n, v);
        }
        obj
    }

    fn build_si_ts() -> TsDataObj {
        let enums = EnumRegistry::new();
        let cls = ts_data::class_props(&enums);
        let mut obj = TsDataObj::new("ts");
        // Same core conductor + insulation/shield data as the engine `build_ts`.
        for (n, v) in &[
            ("runits", "m"),
            ("gmrunits", "m"),
            ("radunits", "m"),
            ("rdc", "0.0001"),
            ("rac", "0.000105"),
            ("radius", "0.005"),
            ("gmrac", "0.004"),
            ("epsr", "2.3"),
            ("inslayer", "0.004"),
            ("diains", "0.022"),
            ("diacable", "0.030"),
            ("diashield", "0.025"),
            ("tapelayer", "0.0002"),
            ("tapelap", "20"),
        ] {
            scalar(&cls, &mut obj, n, v);
        }
        obj
    }

    #[test]
    fn matrices_overhead_match_oracle() {
        // 3-phase overhead at x = 0/1/2 m, h = 10 m, DERI earth model — matches
        // the engine `deri_full_3cond` reference (Z) and `C3_NF` (capacitance).
        let enums = EnumRegistry::new();
        let cls = class_props(&enums);
        let w = build_si_wire();
        let mut g = LineGeometryObj::new("g1");
        scalar(&cls, &mut g, "nconds", "3");
        scalar(&cls, &mut g, "nphases", "3");
        for (k, x) in ["0", "1", "2"].iter().enumerate() {
            scalar(&cls, &mut g, "cond", &(k + 1).to_string());
            set_ref(&cls, &mut g, "wire", &w);
            scalar(&cls, &mut g, "x", x);
            scalar(&cls, &mut g, "h", "10");
            scalar(&cls, &mut g, "units", "m");
        }

        let z = g.z_matrix(60.0, 1.0, M_UNIT, DERI).expect("z");
        let z_ref = [
            (3.525947626277e-04, 9.150978496084e-04),
            (5.807483299046e-05, 5.156141524879e-04),
            (5.807470316267e-05, 4.633520928126e-04),
            (5.807483299046e-05, 5.156141524879e-04),
            (3.525947626277e-04, 9.150978496084e-04),
            (5.807483299046e-05, 5.156141524879e-04),
            (5.807470316267e-05, 4.633520928126e-04),
            (5.807483299046e-05, 5.156141524879e-04),
            (3.525947626277e-04, 9.150978496084e-04),
        ];
        assert_z(&z, &z_ref, 3);

        let yc = g.yc_matrix(60.0, 1.0, M_UNIT, DERI).expect("yc");
        let c_ref_nf = [
            8.941431489720e-03,
            -2.907193315935e-03,
            -1.568246629107e-03,
            -2.907193315935e-03,
            9.611612264845e-03,
            -2.907193315935e-03,
            -1.568246629107e-03,
            -2.907193315935e-03,
            8.941431489720e-03,
        ];
        for i in 0..3 {
            for j in 0..3 {
                assert_close(
                    yc.get(i, j).im / W60,
                    c_ref_nf[i * 3 + j] * 1e-9,
                    &format!("C[{i}][{j}]"),
                );
            }
        }
    }

    #[test]
    fn matrices_reduce_neutral_to_phases() {
        // 3 phases + 1 neutral at (1, 12); `reduce=y` Krons the neutral out, so
        // z_matrix returns the reduced 3×3 (engine `deri_reduce_4cond_to_3`).
        let enums = EnumRegistry::new();
        let cls = class_props(&enums);
        let w = build_si_wire();
        let mut g = LineGeometryObj::new("g1");
        scalar(&cls, &mut g, "nconds", "4");
        scalar(&cls, &mut g, "nphases", "3");
        scalar(&cls, &mut g, "reduce", "y");
        let coords = [("0", "10"), ("1", "10"), ("2", "10"), ("1", "12")];
        for (k, (x, h)) in coords.iter().enumerate() {
            scalar(&cls, &mut g, "cond", &(k + 1).to_string());
            set_ref(&cls, &mut g, "wire", &w);
            scalar(&cls, &mut g, "x", x);
            scalar(&cls, &mut g, "h", h);
            scalar(&cls, &mut g, "units", "m");
        }

        let z = g.z_matrix(60.0, 1.0, M_UNIT, DERI).expect("z");
        assert_eq!(z.order(), 3);
        let z_ref = [
            (3.770208623296e-04, 7.019407348531e-04),
            (8.343915794981e-05, 2.986360481570e-04),
            (8.250080286461e-05, 2.501949780572e-04),
            (8.343915794981e-05, 2.986360481570e-04),
            (3.789232335261e-04, 6.942314211641e-04),
            (8.343915794981e-05, 2.986360481570e-04),
            (8.250080286461e-05, 2.501949780572e-04),
            (8.343915794981e-05, 2.986360481570e-04),
            (3.770208623296e-04, 7.019407348531e-04),
        ];
        assert_z(&z, &z_ref, 3);
    }

    #[test]
    fn matrices_cn_cable_match_oracle() {
        // 3 buried concentric-neutral cables (h = -1.2 m, x = 0/0.1/0.2 m), DERI
        // — exercises the CN param transfer (k/DiaStrand/GMRStrand/RStrand,
        // EpsR/InsLayer/DiaIns/DiaCable). Engine `cn_cable_deri_3cond`.
        let enums = EnumRegistry::new();
        let cls = class_props(&enums);
        let cn = build_si_cn();
        let mut g = LineGeometryObj::new("g1");
        scalar(&cls, &mut g, "nconds", "3");
        scalar(&cls, &mut g, "nphases", "3");
        for (k, x) in ["0", "0.1", "0.2"].iter().enumerate() {
            scalar(&cls, &mut g, "cond", &(k + 1).to_string());
            set_ref(&cls, &mut g, "cncable", &cn);
            scalar(&cls, &mut g, "x", x);
            scalar(&cls, &mut g, "h", "-1.2");
            scalar(&cls, &mut g, "units", "m");
        }

        let z = g.z_matrix(60.0, 1.0, M_UNIT, DERI).expect("z");
        let z_ref = [
            (1.957766526264e-04, 1.402105660670e-04),
            (2.071515058850e-05, -1.736794561581e-05),
            (7.705339488750e-06, -1.452216497679e-05),
            (2.071515058850e-05, -1.736794561581e-05),
            (1.844408315520e-04, 1.418811316531e-04),
            (2.071515058850e-05, -1.736794561581e-05),
            (7.705339488750e-06, -1.452216497679e-05),
            (2.071515058850e-05, -1.736794561581e-05),
            (1.957766526264e-04, 1.402105660670e-04),
        ];
        assert_z(&z, &z_ref, 3);

        // Coaxial insulation capacitance: diagonal only, off-diagonals zero.
        let yc = g.yc_matrix(60.0, 1.0, M_UNIT, DERI).expect("yc");
        assert_close(yc.get(0, 0).im / W60, 2.830890564838e-01 * 1e-9, "C[0][0]");
        assert_close(yc.get(0, 1).im, 0.0, "C[0][1]");
    }

    #[test]
    fn matrices_ts_cable_match_oracle() {
        // 3 buried tape-shield cables (h = -1.2 m, x = 0/0.1/0.2 m), DERI —
        // exercises the TS param transfer (DiaShield/TapeLayer/TapeLap,
        // EpsR/InsLayer/DiaIns/DiaCable). Engine `ts_cable_deri_3cond`.
        let enums = EnumRegistry::new();
        let cls = class_props(&enums);
        let ts = build_si_ts();
        let mut g = LineGeometryObj::new("g1");
        scalar(&cls, &mut g, "nconds", "3");
        scalar(&cls, &mut g, "nphases", "3");
        for (k, x) in ["0", "0.1", "0.2"].iter().enumerate() {
            scalar(&cls, &mut g, "cond", &(k + 1).to_string());
            set_ref(&cls, &mut g, "tscable", &ts);
            scalar(&cls, &mut g, "x", x);
            scalar(&cls, &mut g, "h", "-1.2");
            scalar(&cls, &mut g, "units", "m");
        }

        let z = g.z_matrix(60.0, 1.0, M_UNIT, DERI).expect("z");
        let z_ref = [
            (4.675825330004e-04, 4.983304721750e-04),
            (3.583518060982e-04, 2.468927740652e-04),
            (3.436688380811e-04, 2.058672035803e-04),
            (3.583518060982e-04, 2.468927740652e-04),
            (4.783159246306e-04, 4.782357442920e-04),
            (3.583518060982e-04, 2.468927740652e-04),
            (3.436688380811e-04, 2.058672035803e-04),
            (3.583518060982e-04, 2.468927740652e-04),
            (4.675825330004e-04, 4.983304721750e-04),
        ];
        assert_z(&z, &z_ref, 3);

        // Coaxial insulation capacitance: diagonal only, off-diagonals zero.
        let yc = g.yc_matrix(60.0, 1.0, M_UNIT, DERI).expect("yc");
        assert_close(yc.get(0, 0).im / W60, 2.830890564838e-01 * 1e-9, "C[0][0]");
        assert_close(yc.get(0, 1).im, 0.0, "C[0][1]");
    }

    #[test]
    fn update_uninitialized_conductor_errors() {
        // A conductor slot left NIL is the Pascal "WireData is not correctly
        // initialized" hard error.
        let enums = EnumRegistry::new();
        let cls = class_props(&enums);
        let w = build_si_wire();
        let mut g = LineGeometryObj::new("g1");
        scalar(&cls, &mut g, "nconds", "3");
        scalar(&cls, &mut g, "nphases", "3");
        scalar(&cls, &mut g, "cond", "1");
        set_ref(&cls, &mut g, "wire", &w); // only conductor 1 set
        let err = g.update_line_geometry_data(60.0, DERI).unwrap_err();
        assert!(err.contains("not correctly initialized"), "{err}");
    }

    #[test]
    fn update_conductors_in_same_space_errors() {
        // Two fat conductors (radius 0.5 m) only 0.2 m apart overlap — the Pascal
        // ELineGeometryProblem / SolutionAbort path.
        let enums = EnumRegistry::new();
        let cls = class_props(&enums);
        let w = build_wire(
            "fat",
            &[
                ("runits", "m"),
                ("gmrunits", "m"),
                ("radunits", "m"),
                ("radius", "0.5"),
            ],
        );
        let mut g = LineGeometryObj::new("g1");
        scalar(&cls, &mut g, "nconds", "2");
        scalar(&cls, &mut g, "nphases", "2");
        for (k, x) in ["0", "0.2"].iter().enumerate() {
            scalar(&cls, &mut g, "cond", &(k + 1).to_string());
            set_ref(&cls, &mut g, "wire", &w);
            scalar(&cls, &mut g, "x", x);
            scalar(&cls, &mut g, "h", "10");
            scalar(&cls, &mut g, "units", "m");
        }
        let err = g.update_line_geometry_data(60.0, DERI).unwrap_err();
        assert!(err.contains("occupy the same space"), "{err}");
    }

    #[test]
    fn make_like_cn_cable_recomputes() {
        // Pascal `MakeLike` rebuilds an *overhead* engine then runs
        // UpdateLineGeometryData, which `EInvalidCast`s for a cable source. We
        // clone the source engine instead, so `like=` a CN geometry does not
        // crash and reproduces the source's matrix (the documented divergence).
        let enums = EnumRegistry::new();
        let cls = class_props(&enums);
        let cn = build_si_cn();
        let mut src = LineGeometryObj::new("src");
        scalar(&cls, &mut src, "nconds", "3");
        scalar(&cls, &mut src, "nphases", "3");
        for (k, x) in ["0", "0.1", "0.2"].iter().enumerate() {
            scalar(&cls, &mut src, "cond", &(k + 1).to_string());
            set_ref(&cls, &mut src, "cncable", &cn);
            scalar(&cls, &mut src, "x", x);
            scalar(&cls, &mut src, "h", "-1.2");
            scalar(&cls, &mut src, "units", "m");
        }
        let z_src = src.z_matrix(60.0, 1.0, M_UNIT, DERI).expect("src z");

        let mut dst = LineGeometryObj::new("dst");
        dst.make_like(&src);
        // No crash (the Pascal bug averted), and the cloned CN engine reproduces
        // the matrix on first use (`data_changed` forces the recompute).
        let z_dst = dst.z_matrix(60.0, 1.0, M_UNIT, DERI).expect("dst z");
        for i in 0..3 {
            for j in 0..3 {
                assert_close(z_dst.get(i, j).re, z_src.get(i, j).re, "Z.re");
                assert_close(z_dst.get(i, j).im, z_src.get(i, j).im, "Z.im");
            }
        }
    }

    #[test]
    fn z_matrix_recomputes_on_frequency_change() {
        // Guard against a hardcoded 60 Hz: the engine recalcs when `f` changes,
        // so Z at 120 Hz must differ from Z at 60 Hz, and returning to 60 Hz must
        // reproduce the original (the recompute keys off frequency).
        let enums = EnumRegistry::new();
        let cls = class_props(&enums);
        let w = build_si_wire();
        let mut g = LineGeometryObj::new("g1");
        scalar(&cls, &mut g, "nconds", "3");
        scalar(&cls, &mut g, "nphases", "3");
        for (k, x) in ["0", "1", "2"].iter().enumerate() {
            scalar(&cls, &mut g, "cond", &(k + 1).to_string());
            set_ref(&cls, &mut g, "wire", &w);
            scalar(&cls, &mut g, "x", x);
            scalar(&cls, &mut g, "h", "10");
            scalar(&cls, &mut g, "units", "m");
        }
        let z60 = g.z_matrix(60.0, 1.0, M_UNIT, DERI).expect("z60");
        let z120 = g.z_matrix(120.0, 1.0, M_UNIT, DERI).expect("z120");
        // Series reactance rises with frequency (≈ ×2 here) — proves `f` reached
        // the engine and triggered a recompute rather than returning a cached Z.
        assert!(
            z120.get(0, 0).im > z60.get(0, 0).im * 1.5,
            "reactance must rise with frequency: {:.6e} vs {:.6e}",
            z120.get(0, 0).im,
            z60.get(0, 0).im
        );
        let z60b = g.z_matrix(60.0, 1.0, M_UNIT, DERI).expect("z60b");
        assert_close(z60b.get(0, 0).im, z60.get(0, 0).im, "Z60 reproduced");
        assert_close(z60b.get(0, 0).re, z60.get(0, 0).re, "Z60 reproduced (re)");
    }

    #[test]
    fn cond_out_of_range_is_ignored() {
        // Pascal `set_ActiveCond` ignores values outside `1..=NConds`; the value
        // stays at the last valid conductor (the generic struct-index "Invalid
        // value" diagnostic is not reproduced — transformer wdg precedent).
        let enums = EnumRegistry::new();
        let cls = class_props(&enums);
        let mut g = LineGeometryObj::new("g1");
        scalar(&cls, &mut g, "nconds", "3");
        scalar(&cls, &mut g, "nphases", "3");
        scalar(&cls, &mut g, "cond", "2");
        scalar(&cls, &mut g, "cond", "99"); // above NConds -> ignored
        assert_eq!(get(&cls, &g, "cond"), "2");
        scalar(&cls, &mut g, "cond", "0"); // below 1 -> ignored
        assert_eq!(get(&cls, &g, "cond"), "2");
    }
}
