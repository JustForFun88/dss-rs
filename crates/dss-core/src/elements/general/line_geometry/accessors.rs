//! The `DssObject` property trait for `LineGeometryObj`: the typed get/set
//! accessors, object-reference plumbing and the `PropertySideEffects` state
//! machine.

use crate::elements::traits::ElemRef;
use crate::obj::base::{DssObjData, DssObject, ObjectRefArrayItem};

use super::{ConductorChoice, LineGeometryObj, prop};

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

    fn set_object_ref_array(&mut self, idx: usize, refs: &[ObjectRefArrayItem<'_>]) {
        match idx {
            prop::WIRES | prop::CNCABLES | prop::TSCABLES => self.set_wires(refs),
            // Pascal generic fill (no WriteByFunction): write each resolved
            // conductor / NIL straight into `conductors` from slot 0; the side
            // effect sets the engine kind + ratings (`Conductors` handling).
            prop::CONDUCTORS => {
                for (i, r) in refs.iter().enumerate() {
                    if i < self.fwiredata.len() {
                        self.fwiredata[i] = r.as_ref().map(|(_, _, o)| o.clone_box());
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
            .map(|o| {
                o.as_ref()
                    .map(|o| o.data().name().to_string())
                    .unwrap_or_default()
            })
            .collect()
    }

    /// Pascal `PropertyStructArrayCountOffset := @obj.FNConds`
    /// (LineGeometry.pas:244): the conductor-array length the generic
    /// `Conductors=` fill validates against (#402 when `< 1`).
    fn array_size(&self, idx: usize) -> usize {
        debug_assert_eq!(idx, prop::CONDUCTORS);
        self.fnconds.max(0) as usize
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

    fn make_like(&mut self, other: &dyn DssObject) {
        self.data.copy_prp_sequence_from(other.data());
        if let Some(o) = other.as_any().downcast_ref::<LineGeometryObj>() {
            // Pascal `MakeLike`: `NConds := Other.NWires` runs the nconds side
            // effect (full reset), then every per-conductor array is copied.
            self.fnconds = o.fnconds;
            self.realloc_conductors();
            self.fnphases = o.fnphases;
            self.line_spacing_obj = o.line_spacing_obj.as_ref().map(|b| b.clone_box());
            // dss_capi 0.15.x `MakeLike` copies the equivalent-spacing fields.
            self.eq_dist_ph_ph = o.eq_dist_ph_ph;
            self.eq_dist_ph_n = o.eq_dist_ph_n;
            self.avg_phase_height = o.avg_phase_height;
            self.avg_neutral_height = o.avg_neutral_height;
            self.equivalent_spacing = o.equivalent_spacing;
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
