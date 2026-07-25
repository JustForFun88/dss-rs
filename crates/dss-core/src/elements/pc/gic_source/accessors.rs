//! The `impl DssObject` property surface, `PropertySideEffects`, `EndEdit`,
//! `take_ref_actions`, and `MakeLike` for [`GicSource`].

use num_complex::Complex64;

use super::GicSource;
use crate::elements::traits::CktElement;
use crate::obj::base::{DssObjData, DssObject, RefAction};

impl DssObject for GicSource {
    fn data(&self) -> &DssObjData {
        &self.cd.obj
    }
    fn data_mut(&mut self) -> &mut DssObjData {
        &mut self.cd.obj
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn as_ckt_element(&self) -> Option<&dyn CktElement> {
        Some(self)
    }
    fn as_ckt_element_mut(&mut self) -> Option<&mut dyn CktElement> {
        Some(self)
    }

    fn get_f64(&self, idx: usize) -> f64 {
        use super::prop::*;
        match idx {
            VOLTS => self.volts,
            ANGLE => self.angle,
            FREQUENCY => self.src_frequency,
            EN => self.e_north,
            EE => self.e_east,
            LAT1 => self.lat1,
            LON1 => self.lon1,
            LAT2 => self.lat2,
            LON2 => self.lon2,
            BASE_FREQ => self.cd.base_frequency,
            _ => unreachable!("GICsource has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        use super::prop::*;
        match idx {
            VOLTS => self.volts = value,
            ANGLE => self.angle = value,
            FREQUENCY => self.src_frequency = value,
            EN => self.e_north = value,
            EE => self.e_east = value,
            LAT1 => self.lat1 = value,
            LON1 => self.lon1 = value,
            LAT2 => self.lat2 = value,
            LON2 => self.lon2 = value,
            BASE_FREQ => self.cd.base_frequency = value,
            _ => unreachable!("GICsource has no double property {idx}"),
        }
    }

    fn get_i32(&self, idx: usize) -> i32 {
        match idx {
            super::prop::PHASES => self.cd.nphases as i32,
            _ => unreachable!("GICsource has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        match idx {
            super::prop::PHASES => self.cd.nphases = value.max(0) as usize,
            _ => unreachable!("GICsource has no integer property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        match idx {
            super::prop::ENABLED => self.cd.enabled,
            _ => unreachable!("GICsource has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        match idx {
            super::prop::ENABLED => self.cd.set_enabled(value),
            _ => unreachable!("GICsource has no boolean property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        match idx {
            super::prop::SPECTRUM => self.spectrum.clone(),
            _ => unreachable!("GICsource has no string property {idx}"),
        }
    }
    fn set_string(&mut self, idx: usize, value: String) {
        match idx {
            // Spectrum is forbidden (forced NIL) — the value is stored for the
            // dump round-trip but never resolves to a live spectrum.
            super::prop::SPECTRUM => self.spectrum = value,
            _ => unreachable!("GICsource has no string property {idx}"),
        }
    }

    fn set_bus_name(&mut self, terminal: usize, value: &str) {
        self.cd.set_bus(terminal, value);
    }
    fn get_bus_name(&self, terminal: usize) -> String {
        self.cd.get_bus(terminal).to_string()
    }

    /// Pascal `TGICsourceObj.PropertySideEffects` (GICsource.pas:210).
    fn side_effects(&mut self, idx: usize, _prev_int: i32) {
        use super::prop::*;
        match idx {
            VOLTS | ANGLE => self.volts_specified = true,
            PHASES => {
                // FphaseShift := 0 (always zero sequence); reallocate terminals.
                let n = self.cd.nphases;
                self.cd.set_nconds(n);
            }
            EN | EE | LAT1 | LON1 | LAT2 | LON2 => self.volts_specified = false,
            _ => {}
        }
    }

    /// Pascal `TGICsource.EndEdit` (GICsource.pas:232): `RecalcElementData`
    /// (updates Volts + splices the Line) then `YPrimInvalid := TRUE`.
    fn end_edit(&mut self, _sys: &crate::elements::traits::SysCtx) {
        self.recalc();
        self.cd.yprim_invalid = true;
    }

    fn take_ref_actions(&mut self) -> Vec<RefAction> {
        std::mem::take(&mut self.pending_actions)
    }

    /// Pascal `TGICsourceObj.MakeLike` (GICsource.pas:243).
    fn make_like(&mut self, other: &dyn DssObject) {
        let Some(other) = other.as_any().downcast_ref::<GicSource>() else {
            return;
        };
        self.cd.make_like_base(&other.cd);
        if self.cd.nphases != other.cd.nphases {
            self.cd.nphases = other.cd.nphases;
            let n = other.cd.nphases;
            self.cd.set_nconds(n); // Forces reallocation of terminal stuff
            self.cd.yorder = self.cd.nconds * self.cd.nterms;
            self.cd.yprim_invalid = true;
        }
        self.volts = other.volts;
        self.angle = other.angle;
        self.src_frequency = other.src_frequency;
        // Pascal copies pLineElem (the base's Line pointer); the executive
        // re-resolves it by the derived name before end_edit, so it is
        // effectively transient.
        self.line_ref = other.line_ref;
        self.line_bus2 = other.line_bus2.clone();
        self.e_north = other.e_north;
        self.e_east = other.e_east;
        self.lat1 = other.lat1;
        self.lon1 = other.lon1;
        self.lat2 = other.lat2;
        self.lon2 = other.lon2;
        self.bus2_defined = other.bus2_defined;
        // Spectrum not allowed (forced NIL).
        self.spectrum = String::new();
        self.spectrum_obj = None;
        self.cd.inj_current = vec![Complex64::ZERO; self.cd.yorder];
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}
