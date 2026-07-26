//! The `impl DssObject` property surface, `PropertySideEffects`, `EndEdit`, and
//! `MakeLike` for [`GicLine`].

use num_complex::Complex64;

use super::GicLine;
use crate::obj::base::{DssObjData, DssObject};

impl GicLine {
    /// Pascal `TGICLineObj.MakeLike` (GICLine.pas:312). Copies ONLY Z/R/X/C/
    /// Volts/Angle/SrcFrequency/Scan/Sequence (plus the inherited spectrum);
    /// the geodesy (`EN/EE/Lat/Lon`) and `VoltsSpecified` are NOT copied, so the
    /// derived object keeps its Create defaults and its `EndEdit`
    /// `RecalcElementData` recomputes `Volts` from the default geodesy (proven
    /// against the oracle 2026-07: derived Volts = default 113.32, geodesy =
    /// defaults, even when the base sets custom values).
    pub(crate) fn make_like(&mut self, other: &Self) {
        self.cd.make_like_base(&other.cd);
        if self.cd.nphases != other.cd.nphases {
            self.cd.nphases = other.cd.nphases;
            let n = other.cd.nphases;
            self.cd.set_nconds(n); // forces reallocation of terminal stuff
            self.cd.yorder = self.cd.nconds * self.cd.nterms;
            self.cd.yprim_invalid = true;
        }
        self.z = other.z.clone(); // Z.CopyFrom(Other.Z)
        self.r = other.r;
        self.x = other.x;
        self.c = other.c;
        self.volts = other.volts;
        self.angle = other.angle;
        self.src_frequency = other.src_frequency;
        self.scan_type = other.scan_type;
        self.sequence_type = other.sequence_type;
        // Inherited TPCElement.MakeLike carries the spectrum reference.
        self.spectrum = other.spectrum.clone();
        self.spectrum_obj = other.spectrum_obj.clone();
        self.cd.inj_current = vec![Complex64::ZERO; self.cd.yorder];
    }
}

impl DssObject for GicLine {
    fn data(&self) -> &DssObjData {
        &self.cd.obj
    }
    fn data_mut(&mut self) -> &mut DssObjData {
        &mut self.cd.obj
    }

    fn get_f64(&self, idx: usize) -> f64 {
        use super::prop::*;
        match idx {
            VOLTS => self.volts,
            ANGLE => self.angle,
            FREQUENCY => self.src_frequency,
            R => self.r,
            X => self.x,
            C => self.c,
            EN => self.e_north,
            EE => self.e_east,
            LAT1 => self.lat1,
            LON1 => self.lon1,
            LAT2 => self.lat2,
            LON2 => self.lon2,
            BASE_FREQ => self.cd.base_frequency,
            _ => unreachable!("GICLine has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        use super::prop::*;
        match idx {
            VOLTS => self.volts = value,
            ANGLE => self.angle = value,
            FREQUENCY => self.src_frequency = value,
            R => self.r = value,
            X => self.x = value,
            C => self.c = value,
            EN => self.e_north = value,
            EE => self.e_east = value,
            LAT1 => self.lat1 = value,
            LON1 => self.lon1 = value,
            LAT2 => self.lat2 = value,
            LON2 => self.lon2 = value,
            BASE_FREQ => self.cd.base_frequency = value,
            _ => unreachable!("GICLine has no double property {idx}"),
        }
    }

    fn get_i32(&self, idx: usize) -> i32 {
        match idx {
            super::prop::PHASES => self.cd.nphases as i32,
            _ => unreachable!("GICLine has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        match idx {
            super::prop::PHASES => self.cd.nphases = value.max(0) as usize,
            _ => unreachable!("GICLine has no integer property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        match idx {
            super::prop::ENABLED => self.cd.enabled,
            _ => unreachable!("GICLine has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        match idx {
            super::prop::ENABLED => self.cd.set_enabled(value),
            _ => unreachable!("GICLine has no boolean property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        match idx {
            super::prop::SPECTRUM => self.spectrum.clone(),
            _ => unreachable!("GICLine has no string property {idx}"),
        }
    }
    fn set_string(&mut self, idx: usize, value: String) {
        match idx {
            super::prop::SPECTRUM => self.spectrum = value,
            _ => unreachable!("GICLine has no string property {idx}"),
        }
    }

    fn set_bus_name(&mut self, terminal: usize, value: &str) {
        self.cd.set_bus(terminal, value);
    }
    fn get_bus_name(&self, terminal: usize) -> String {
        self.cd.get_bus(terminal).to_string()
    }

    /// Pascal `TGICLineObj.PropertySideEffects` (GICLine.pas:264).
    fn side_effects(&mut self, idx: usize, _prev_int: i32) {
        use super::prop::*;
        match idx {
            BUS1 => {
                // Default Bus2 = Bus1 with node designations stripped.
                let s = self.cd.get_bus(1).to_string();
                let s2 = match s.find('.') {
                    Some(dot) => s[..dot].to_string(),
                    None => s,
                };
                self.cd.set_bus(2, &s2);
            }
            PHASES => {
                let n = self.cd.nphases;
                self.cd.set_nconds(n); // Force reallocation of terminal info
            }
            VOLTS | ANGLE => self.volts_specified = true,
            EN | EE | LAT1 | LON1 | LAT2 | LON2 => self.volts_specified = false,
            _ => {}
        }
    }

    /// Pascal `TGICLine.EndEdit` (GICLine.pas:301): `RecalcElementData` then
    /// `YPrimInvalid := TRUE`.
    fn end_edit(&mut self, _sys: &crate::elements::traits::SysCtx) {
        self.recalc();
        self.cd.yprim_invalid = true;
    }
}
