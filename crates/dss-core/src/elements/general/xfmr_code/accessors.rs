//! The `DssObject` trait impl for `XfmrCodeObj`: typed property accessors,
//! the per-winding struct-array getters/setters, `PropertySideEffects`,
//! `EndEdit` and `MakeLike`. Split out of `xfmr_code/mod.rs` (no behavioral
//! change).

use crate::elements::pd::winding::Connection;
use crate::obj::base::{DssObjData, DssObject};

use super::{XfmrCodeObj, prop, xsc_size};

impl XfmrCodeObj {
    /// Pascal `TXfmrCodeObj.MakeLike`.
    pub(crate) fn make_like(&mut self, other: &Self) {
        self.data.copy_prp_sequence_from(other.data());
        let o = other;
        self.fnphases = o.fnphases;
        self.set_num_windings(o.num_windings);
        self.windings.clone_from(&o.windings);
        self.xhl = o.xhl;
        self.xht = o.xht;
        self.xlt = o.xlt;
        let n = xsc_size(self.num_windings);
        for i in 0..n {
            self.xsc[i] = o.xsc[i];
        }
        self.thermal_time_const = o.thermal_time_const;
        self.n_thermal = o.n_thermal;
        self.m_thermal = o.m_thermal;
        self.flrise = o.flrise;
        self.hsrise = o.hsrise;
        self.pct_load_loss = o.pct_load_loss;
        self.pct_no_load_loss = o.pct_no_load_loss;
        self.norm_max_hkva = o.norm_max_hkva;
        self.emerg_max_hkva = o.emerg_max_hkva;
        self.num_kva_ratings = o.num_kva_ratings;
        self.kva_ratings.clone_from(&o.kva_ratings);
    }
}

impl DssObject for XfmrCodeObj {
    fn data(&self) -> &DssObjData {
        &self.data
    }
    fn data_mut(&mut self) -> &mut DssObjData {
        &mut self.data
    }

    fn get_i32(&self, idx: usize) -> i32 {
        use prop::*;
        match idx {
            PHASES => self.fnphases,
            WINDINGS => self.num_windings,
            WDG => self.active_winding,
            CONN => self.windings[self.aw()].connection.ordinal(),
            NUMTAPS => self.windings[self.aw()].num_taps,
            SEASONS => self.num_kva_ratings,
            _ => unreachable!("XfmrCode has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        use prop::*;
        match idx {
            PHASES => self.fnphases = value,
            WINDINGS => self.num_windings = value,
            WDG => self.active_winding = value,
            CONN => {
                let w = self.aw();
                if let Some(c) = Connection::from_ordinal(value) {
                    self.windings[w].connection = c;
                }
            }
            NUMTAPS => {
                let w = self.aw();
                self.windings[w].num_taps = value;
            }
            SEASONS => self.num_kva_ratings = value,
            _ => unreachable!("XfmrCode has no integer property {idx}"),
        }
    }

    fn get_f64(&self, idx: usize) -> f64 {
        use prop::*;
        let w = self.aw();
        match idx {
            KV => self.windings[w].kvll,
            KVA => self.windings[w].kva,
            TAP => self.windings[w].putap,
            PCTR => self.windings[w].rpu,
            RNEUT => self.windings[w].rneut,
            XNEUT => self.windings[w].xneut,
            MAXTAP => self.windings[w].max_tap,
            MINTAP => self.windings[w].min_tap,
            RDCOHMS => self.windings[w].rdcohms,
            XHL | X12 => self.xhl,
            XHT | X13 => self.xht,
            XLT | X23 => self.xlt,
            THERMAL => self.thermal_time_const,
            N => self.n_thermal,
            M => self.m_thermal,
            FLRISE => self.flrise,
            HSRISE => self.hsrise,
            PCTLOADLOSS => self.pct_load_loss,
            PCTNOLOADLOSS => self.pct_no_load_loss,
            NORMHKVA => self.norm_max_hkva,
            EMERGHKVA => self.emerg_max_hkva,
            PCTIMAG => self.pct_imag,
            PPM_ANTIFLOAT => self.ppm_float_factor,
            _ => unreachable!("XfmrCode has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        use prop::*;
        let w = self.aw();
        match idx {
            KV => self.windings[w].kvll = value,
            KVA => self.windings[w].kva = value,
            TAP => self.windings[w].putap = value,
            PCTR => self.windings[w].rpu = value,
            RNEUT => self.windings[w].rneut = value,
            XNEUT => self.windings[w].xneut = value,
            MAXTAP => self.windings[w].max_tap = value,
            MINTAP => self.windings[w].min_tap = value,
            RDCOHMS => self.windings[w].rdcohms = value,
            XHL | X12 => self.xhl = value,
            XHT | X13 => self.xht = value,
            XLT | X23 => self.xlt = value,
            THERMAL => self.thermal_time_const = value,
            N => self.n_thermal = value,
            M => self.m_thermal = value,
            FLRISE => self.flrise = value,
            HSRISE => self.hsrise = value,
            PCTLOADLOSS => self.pct_load_loss = value,
            PCTNOLOADLOSS => self.pct_no_load_loss = value,
            NORMHKVA => self.norm_max_hkva = value,
            EMERGHKVA => self.emerg_max_hkva = value,
            PCTIMAG => self.pct_imag = value,
            PPM_ANTIFLOAT => self.ppm_float_factor = value,
            _ => unreachable!("XfmrCode has no double property {idx}"),
        }
    }

    fn get_f64_array(&self, idx: usize) -> Option<&[f64]> {
        match idx {
            prop::XSCARRAY => Some(&self.xsc),
            prop::RATINGS => Some(&self.kva_ratings),
            _ => unreachable!("XfmrCode has no array property {idx}"),
        }
    }
    fn set_f64_array(&mut self, idx: usize, value: Vec<f64>) {
        match idx {
            prop::XSCARRAY => self.xsc = value,
            prop::RATINGS => self.kva_ratings = value,
            _ => unreachable!("XfmrCode has no array property {idx}"),
        }
    }

    fn array_size(&self, idx: usize) -> usize {
        match idx {
            prop::XSCARRAY => xsc_size(self.num_windings),
            _ => unreachable!("XfmrCode has no function-sized array {idx}"),
        }
    }

    fn get_struct_f64_array(&self, idx: usize) -> Vec<f64> {
        use prop::*;
        self.windings
            .iter()
            .map(|w| match idx {
                KVS => w.kvll,
                KVAS => w.kva,
                TAPS => w.putap,
                PCTRS => w.rpu,
                // Per-winding scalars rendered as a JSON array under `ON_ARRAY`
                // (DoubleOnStructArrayProperty; mirrors Transformer).
                RNEUT => w.rneut,
                XNEUT => w.xneut,
                MAXTAP => w.max_tap,
                MINTAP => w.min_tap,
                RDCOHMS => w.rdcohms,
                _ => unreachable!("XfmrCode has no struct array {idx}"),
            })
            .collect()
    }
    fn set_struct_f64_array(&mut self, idx: usize, values: &[Option<f64>]) {
        use prop::*;
        for (w, v) in self.windings.iter_mut().zip(values) {
            let Some(v) = v else { continue };
            match idx {
                KVS => w.kvll = *v,
                KVAS => w.kva = *v,
                TAPS => w.putap = *v,
                PCTRS => w.rpu = *v,
                // JSON import twin of the per-winding `ON_ARRAY` scalars above.
                RNEUT => w.rneut = *v,
                XNEUT => w.xneut = *v,
                MAXTAP => w.max_tap = *v,
                MINTAP => w.min_tap = *v,
                RDCOHMS => w.rdcohms = *v,
                _ => unreachable!("XfmrCode has no struct array {idx}"),
            }
        }
        // Pascal `positionPtr^ := intVal`: leave the active winding at the last.
        self.active_winding = self.num_windings;
    }

    fn get_struct_i32_array(&self, idx: usize) -> Vec<i32> {
        match idx {
            prop::CONNS => self
                .windings
                .iter()
                .map(|w| w.connection.ordinal())
                .collect(),
            // NumTaps rendered as a JSON per-winding array under `ON_ARRAY`
            // (IntegerOnStructArrayProperty; mirrors Transformer).
            prop::NUMTAPS => self.windings.iter().map(|w| w.num_taps).collect(),
            _ => unreachable!("XfmrCode has no struct enum array {idx}"),
        }
    }
    fn set_struct_i32_array(&mut self, idx: usize, values: &[i32]) {
        match idx {
            prop::CONNS => {
                for (w, v) in self.windings.iter_mut().zip(values) {
                    if let Some(c) = Connection::from_ordinal(*v) {
                        w.connection = c;
                    }
                }
            }
            prop::NUMTAPS => {
                for (w, v) in self.windings.iter_mut().zip(values) {
                    w.num_taps = *v;
                }
            }
            _ => unreachable!("XfmrCode has no struct enum array {idx}"),
        }
        self.active_winding = self.num_windings;
    }

    /// Pascal `TXfmrCodeObj.PropertySideEffects`.
    fn side_effects(&mut self, idx: usize, prev_int: i32) {
        use prop::*;
        match idx {
            WINDINGS => self.realloc_windings(prev_int),
            KVA => {
                if self.active_winding == 1 {
                    let k = self.windings[0].kva;
                    for w in self.windings.iter_mut().skip(1) {
                        w.kva = k;
                    }
                    self.norm_max_hkva = 1.1 * k;
                    self.emerg_max_hkva = 1.5 * k;
                } else if self.num_windings == 2 {
                    self.windings[0].kva = self.windings[1].kva;
                }
            }
            PCTR | PCTRS => {
                if self.windings.len() >= 2 {
                    self.pct_load_loss = (self.windings[0].rpu + self.windings[1].rpu) * 100.0;
                }
            }
            KVAS => {
                let k = self.windings[0].kva;
                self.norm_max_hkva = 1.1 * k;
                self.emerg_max_hkva = 1.5 * k;
            }
            PCTLOADLOSS => {
                if self.windings.len() >= 2 {
                    let r = self.pct_load_loss / 2.0 / 100.0;
                    self.windings[0].rpu = r;
                    self.windings[1].rpu = r;
                }
            }
            X12 | X13 | X23 | XHL | XHT | XLT => self.needs_recalc = true,
            RDCOHMS => {
                let w = self.aw();
                self.windings[w].rdc_specified = true;
            }
            SEASONS => self
                .kva_ratings
                .resize(self.num_kva_ratings.max(0) as usize, 0.0),
            _ => {}
        }
    }

    /// Pascal `TXfmrCode.EndEdit`: copy `XHL/XHT/XLT` into the leading `XSC`
    /// slots when a reactance property was edited (≤ 3 windings).
    fn end_edit(&mut self, _sys: &crate::elements::traits::SysCtx) {
        if !self.needs_recalc {
            return;
        }
        self.needs_recalc = false;
        if self.num_windings <= 3 {
            let vals = [self.xhl, self.xht, self.xlt];
            for (slot, v) in self.xsc.iter_mut().zip(vals) {
                *slot = v;
            }
        }
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}
