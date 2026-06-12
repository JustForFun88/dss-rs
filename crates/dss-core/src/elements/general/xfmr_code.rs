//! `XfmrCode` — a catalog of transformer winding/impedance data that a
//! `Transformer` can pull in via `xfmrcode=`. Port of Pascal
//! `General/XfmrCode.pas` (`TXfmrCodeObj`). A `DSS_OBJECT` class (no terminals,
//! no YPrim): it holds the same `Winding` web as `TTransfObj` minus the buses,
//! bank and circuit wiring.
//!
//! The winding-property machinery (per-winding scalars `kV`/`kVA`/`Tap`/`%R`/…
//! addressed through the `Wdg=` active index, plus the plural array forms
//! `kVs`/`Conns`/… that write every winding at once) is shared with the
//! Transformer (WP4.4); see [`crate::elements::pd::winding::Winding`].

use crate::elements::pd::winding::Winding;
use crate::obj::base::{DssObjData, DssObject};
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};

/// 1-based property ordinals (Pascal `TXfmrCodeProp`).
pub mod prop {
    pub const PHASES: usize = 1;
    pub const WINDINGS: usize = 2;
    pub const WDG: usize = 3;
    pub const CONN: usize = 4;
    pub const KV: usize = 5;
    pub const KVA: usize = 6;
    pub const TAP: usize = 7;
    pub const PCTR: usize = 8;
    pub const RNEUT: usize = 9;
    pub const XNEUT: usize = 10;
    pub const CONNS: usize = 11;
    pub const KVS: usize = 12;
    pub const KVAS: usize = 13;
    pub const TAPS: usize = 14;
    pub const XHL: usize = 15;
    pub const XHT: usize = 16;
    pub const XLT: usize = 17;
    pub const XSCARRAY: usize = 18;
    pub const THERMAL: usize = 19;
    pub const N: usize = 20;
    pub const M: usize = 21;
    pub const FLRISE: usize = 22;
    pub const HSRISE: usize = 23;
    pub const PCTLOADLOSS: usize = 24;
    pub const PCTNOLOADLOSS: usize = 25;
    pub const NORMHKVA: usize = 26;
    pub const EMERGHKVA: usize = 27;
    pub const MAXTAP: usize = 28;
    pub const MINTAP: usize = 29;
    pub const NUMTAPS: usize = 30;
    pub const PCTIMAG: usize = 31;
    pub const PPM_ANTIFLOAT: usize = 32;
    pub const PCTRS: usize = 33;
    pub const X12: usize = 34;
    pub const X13: usize = 35;
    pub const X23: usize = 36;
    pub const RDCOHMS: usize = 37;
    pub const SEASONS: usize = 38;
    pub const RATINGS: usize = 39;
    pub const NUM_PROPS: usize = 40; // incl. Like
}

/// `TXfmrCode.DefineProperties`.
pub fn class_props(enums: &EnumRegistry) -> ClassProps {
    use prop::*;
    let pct = 0.01;
    let defs = vec![
        PropDef::integer("Phases").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::integer("Windings").flags(PropFlags::GREATER_THAN_ONE | PropFlags::SUPPRESS_JSON),
        // Winding definition (active winding selected by `Wdg=`).
        PropDef::integer("Wdg"),
        PropDef::mapped_string_enum("Conn", enums.connection),
        PropDef::double("kV").flags(PropFlags::NON_NEGATIVE),
        PropDef::double("kVA"),
        PropDef::double("Tap"),
        PropDef::double("%R").scale(pct),
        PropDef::double("RNeut"),
        PropDef::double("XNeut"),
        // General data (plural array forms write every winding).
        PropDef::enum_array_on_struct("Conns", enums.connection, WINDINGS),
        PropDef::double_array_on_struct("kVs", WINDINGS),
        PropDef::double_array_on_struct("kVAs", WINDINGS),
        PropDef::double_array_on_struct("Taps", WINDINGS),
        PropDef::double("XHL").scale(pct).flags(PropFlags::NON_ZERO),
        PropDef::double("XHT").scale(pct).flags(PropFlags::NON_ZERO),
        PropDef::double("XLT").scale(pct).flags(PropFlags::NON_ZERO),
        PropDef::double_v_array("XSCArray")
            .scale(pct)
            .flags(PropFlags::NON_ZERO | PropFlags::REQUIRED_IN_SPEC_SET),
        PropDef::double("Thermal"),
        PropDef::double("n"),
        PropDef::double("m"),
        PropDef::double("FLRise"),
        PropDef::double("HSRise"),
        PropDef::double("%LoadLoss"),
        PropDef::double("%NoLoadLoss"),
        PropDef::double("NormHkVA"),
        PropDef::double("EmergHkVA"),
        PropDef::double("MaxTap"),
        PropDef::double("MinTap"),
        PropDef::integer("NumTaps"),
        PropDef::double("%IMag"),
        PropDef::double("ppm_Antifloat").scale(1.0e-6),
        PropDef::double_array_on_struct("%Rs", WINDINGS).scale(pct),
        PropDef::double("X12").scale(pct).flags(PropFlags::NON_ZERO),
        PropDef::double("X13").scale(pct).flags(PropFlags::NON_ZERO),
        PropDef::double("X23").scale(pct).flags(PropFlags::NON_ZERO),
        PropDef::double("RDCOhms"),
        PropDef::integer("Seasons").flags(PropFlags::SUPPRESS_JSON),
        PropDef::double_array("Ratings", SEASONS),
    ];
    debug_assert_eq!(defs.len(), NUM_PROPS - 1);
    ClassProps::new("XfmrCode", defs, true)
}

/// `TXfmrCodeObj`.
#[derive(Debug, Clone)]
pub struct XfmrCodeObj {
    data: DssObjData,
    fnphases: i32,
    /// Pascal `ActiveWinding` (1-based).
    active_winding: i32,
    num_windings: i32,
    max_windings: i32,
    xhl: f64,
    xht: f64,
    xlt: f64,
    /// Pascal `XSC` — per-unit short-circuit reactances (`x12 x13 x23 …`).
    xsc: Vec<f64>,
    vabase: f64,
    norm_max_hkva: f64,
    emerg_max_hkva: f64,
    thermal_time_const: f64,
    n_thermal: f64,
    m_thermal: f64,
    flrise: f64,
    hsrise: f64,
    pct_load_loss: f64,
    pct_no_load_loss: f64,
    ppm_float_factor: f64,
    pct_imag: f64,
    windings: Vec<Winding>,
    num_kva_ratings: i32,
    kva_ratings: Vec<f64>,
    /// Pascal `Flg.NeedsRecalc`: an `XHL/XHT/XLT/X12/X13/X23` was set, so
    /// `EndEdit` copies them into the leading `XSC` slots.
    needs_recalc: bool,
}

/// Pascal `XscSize`: `(NumWindings-1)·NumWindings/2`.
fn xsc_size(num_windings: i32) -> usize {
    let n = num_windings.max(0) as usize;
    if n >= 1 { (n - 1) * n / 2 } else { 0 }
}

impl XfmrCodeObj {
    /// Pascal `TXfmrCodeObj.Create`.
    pub fn new(name: impl Into<String>) -> Self {
        let num_windings = 2;
        let windings = vec![Winding::new(); num_windings as usize];
        let vabase = windings[0].kva * 1000.0;
        let mut obj = Self {
            data: DssObjData::new(name.into().to_lowercase(), prop::NUM_PROPS),
            fnphases: 3,
            active_winding: 1,
            num_windings,
            max_windings: num_windings,
            xhl: 0.07,
            xht: 0.35,
            xlt: 0.30,
            xsc: vec![0.0; xsc_size(num_windings)],
            vabase,
            norm_max_hkva: 1.1 * windings[0].kva,
            emerg_max_hkva: 1.5 * windings[0].kva,
            thermal_time_const: 2.0,
            n_thermal: 0.8,
            m_thermal: 0.8,
            flrise: 65.0,
            hsrise: 15.0,
            pct_load_loss: 2.0 * windings[0].rpu * 100.0,
            pct_no_load_loss: 0.0,
            ppm_float_factor: 0.000001,
            pct_imag: 0.0,
            windings,
            num_kva_ratings: 1,
            kva_ratings: vec![600.0],
            needs_recalc: false,
        };
        // Pascal ctor recomputes each winding's anti-float adder on VABase/phases.
        let vabase_1ph = obj.vabase / obj.fnphases as f64;
        for w in &mut obj.windings {
            w.compute_anti_float_adder(obj.ppm_float_factor, vabase_1ph);
        }
        obj
    }

    /// Read accessors for `TTransfObj.FetchXfmrCode` (the transformer copies
    /// the whole winding web out of a resolved `XfmrCode`).
    pub fn fnphases(&self) -> i32 {
        self.fnphases
    }
    pub fn num_windings(&self) -> i32 {
        self.num_windings
    }
    pub fn windings(&self) -> &[Winding] {
        &self.windings
    }
    pub fn xhl(&self) -> f64 {
        self.xhl
    }
    pub fn xht(&self) -> f64 {
        self.xht
    }
    pub fn xlt(&self) -> f64 {
        self.xlt
    }
    pub fn xsc(&self) -> &[f64] {
        &self.xsc
    }
    pub fn thermal_time_const(&self) -> f64 {
        self.thermal_time_const
    }
    pub fn n_thermal(&self) -> f64 {
        self.n_thermal
    }
    pub fn m_thermal(&self) -> f64 {
        self.m_thermal
    }
    pub fn flrise(&self) -> f64 {
        self.flrise
    }
    pub fn hsrise(&self) -> f64 {
        self.hsrise
    }
    pub fn pct_load_loss(&self) -> f64 {
        self.pct_load_loss
    }
    pub fn pct_no_load_loss(&self) -> f64 {
        self.pct_no_load_loss
    }
    pub fn pct_imag(&self) -> f64 {
        self.pct_imag
    }
    pub fn norm_max_hkva(&self) -> f64 {
        self.norm_max_hkva
    }
    pub fn emerg_max_hkva(&self) -> f64 {
        self.emerg_max_hkva
    }
    pub fn ppm_float_factor(&self) -> f64 {
        self.ppm_float_factor
    }
    pub fn num_kva_ratings(&self) -> i32 {
        self.num_kva_ratings
    }
    pub fn kva_ratings(&self) -> &[f64] {
        &self.kva_ratings
    }

    /// Active winding as a 0-based index, clamped into range.
    fn aw(&self) -> usize {
        (self.active_winding.clamp(1, self.num_windings.max(1)) - 1) as usize
    }

    /// Pascal `TXfmrCodeObj.SetNumWindings` → the `windings` side effect.
    fn set_num_windings(&mut self, n: i32) {
        let prev = self.num_windings;
        self.num_windings = n;
        self.realloc_windings(prev);
    }

    /// Pascal `PropertySideEffects(ord(windings), prev)`: reallocate (and
    /// re-`Init`) the winding array and grow `XSC`, defaulting the new
    /// short-circuit reactances to 0.30.
    fn realloc_windings(&mut self, prev_int: i32) {
        let old_xsc = xsc_size(prev_int);
        self.max_windings = self.num_windings;
        let nw = self.num_windings.max(0) as usize;
        self.windings = vec![Winding::new(); nw];
        let new_xsc = xsc_size(self.num_windings);
        if new_xsc > old_xsc {
            self.xsc.resize(new_xsc, 0.30);
        } else {
            self.xsc.truncate(new_xsc);
        }
    }
}

impl DssObject for XfmrCodeObj {
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
        use prop::*;
        match idx {
            PHASES => self.fnphases,
            WINDINGS => self.num_windings,
            WDG => self.active_winding,
            CONN => self.windings[self.aw()].connection,
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
                self.windings[w].connection = value;
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
                _ => unreachable!("XfmrCode has no struct array {idx}"),
            }
        }
        // Pascal `positionPtr^ := intVal`: leave the active winding at the last.
        self.active_winding = self.num_windings;
    }

    fn get_struct_i32_array(&self, idx: usize) -> Vec<i32> {
        match idx {
            prop::CONNS => self.windings.iter().map(|w| w.connection).collect(),
            _ => unreachable!("XfmrCode has no struct enum array {idx}"),
        }
    }
    fn set_struct_i32_array(&mut self, idx: usize, values: &[i32]) {
        match idx {
            prop::CONNS => {
                for (w, v) in self.windings.iter_mut().zip(values) {
                    w.connection = *v;
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
    fn end_edit(&mut self) {
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

    /// Pascal `TXfmrCodeObj.MakeLike`.
    fn make_like(&mut self, other: &dyn DssObject) {
        self.data.copy_prp_sequence_from(other.data());
        let Some(o) = other.as_any().downcast_ref::<XfmrCodeObj>() else {
            return;
        };
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

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::obj::props::PropEngine;
    use dss_parser::{Parser, ParserVars};

    /// Drive a sequence of `(prop, value)` edits through the property engine,
    /// then run `end_edit`. Mirrors the executive's edit loop (sans foreign
    /// class resolution, which XfmrCode does not use).
    fn edited(edits: &[(&str, &str)]) -> (ClassProps, XfmrCodeObj) {
        let enums = EnumRegistry::new();
        let cls = class_props(&enums);
        let mut obj = XfmrCodeObj::new("xc");
        let mut parser = Parser::new();
        let vars = ParserVars::new();
        let mut errors = Vec::new();
        for (name, value) in edits {
            let idx = cls.property_index(name).expect("known property");
            let mut eng = PropEngine {
                parser: &mut parser,
                vars: &vars,
                enums: &enums,
                errors: &mut errors,
                foreign: None,
            };
            cls.edit_property(&mut obj, idx, value, &mut eng).unwrap();
        }
        obj.end_edit();
        assert!(errors.is_empty(), "{errors:?}");
        (cls, obj)
    }

    fn get(cls: &ClassProps, obj: &XfmrCodeObj, name: &str) -> String {
        let enums = EnumRegistry::new();
        let idx = cls.property_index(name).unwrap();
        cls.get_value(obj, idx, &enums)
    }

    /// Assert a property's dump matches `expected` the way the golden harness
    /// does: the non-numeric skeleton exactly, the numbers within tolerance
    /// (FPC `FloatToStr` ≠ Rust `{}`, so raw strings must not be diffed — see
    /// `util::float_to_str`).
    fn check(cls: &ClassProps, obj: &XfmrCodeObj, name: &str, expected: &str) {
        let actual = get(cls, obj, name);
        let split = |s: &str| -> (String, Vec<f64>) {
            let mut skel = String::new();
            let mut nums = Vec::new();
            let mut tok = String::new();
            let flush = |tok: &mut String, skel: &mut String, nums: &mut Vec<f64>| {
                if let Ok(v) = tok.parse::<f64>() {
                    nums.push(v);
                    skel.push('#');
                } else {
                    skel.push_str(tok);
                }
                tok.clear();
            };
            for c in s.chars() {
                if c.is_ascii_digit() || matches!(c, '.' | '+' | '-' | 'e' | 'E') {
                    tok.push(c);
                } else {
                    flush(&mut tok, &mut skel, &mut nums);
                    skel.push(c);
                }
            }
            flush(&mut tok, &mut skel, &mut nums);
            (skel, nums)
        };
        let (askel, anums) = split(&actual);
        let (eskel, enums) = split(expected);
        assert_eq!(askel, eskel, "{name}: {actual:?} vs {expected:?}");
        assert_eq!(
            anums.len(),
            enums.len(),
            "{name}: {actual:?} vs {expected:?}"
        );
        for (a, e) in anums.iter().zip(&enums) {
            assert!(
                (a - e).abs() <= 1e-9 + 1e-9 * e.abs(),
                "{name}: {a} vs {e} ({actual:?} vs {expected:?})"
            );
        }
    }

    #[test]
    fn defaults_match_oracle() {
        let (cls, obj) = edited(&[]);
        // Oracle (dss-python 0.15.7) `new xfmrcode.x`.
        check(&cls, &obj, "Windings", "2");
        check(&cls, &obj, "kV", "12.47");
        check(&cls, &obj, "%R", "0.2");
        check(&cls, &obj, "%LoadLoss", "0.4");
        check(&cls, &obj, "XHL", "7");
        check(&cls, &obj, "XSCArray", "[ 0]");
        check(&cls, &obj, "Conns", "[wye, wye, ]");
        check(&cls, &obj, "RDCOhms", "0.26435153");
    }

    #[test]
    fn wdg_sequencing_writes_active_winding() {
        // `wdg=2` selects the active winding; scalars then write through to it.
        let (cls, obj) = edited(&[
            ("windings", "2"),
            ("wdg", "1"),
            ("kv", "7.2"),
            ("wdg", "2"),
            ("kv", "0.24"),
        ]);
        check(&cls, &obj, "kVs", "[7.2, 0.24, ]");
        // The active winding is left at 2 (the last `wdg=`).
        check(&cls, &obj, "Wdg", "2");
        check(&cls, &obj, "kV", "0.24");
    }

    #[test]
    fn plural_arrays_leave_active_at_last_winding() {
        let (cls, obj) = edited(&[
            ("windings", "3"),
            ("conns", "delta, wye, wye"),
            ("kvs", "115, 12.47, 4.16"),
            ("kvas", "5000, 5000, 5000"),
            ("xhl", "8"),
            ("xht", "10"),
            ("xlt", "9"),
        ]);
        check(&cls, &obj, "Conns", "[delta, wye, wye, ]");
        check(&cls, &obj, "Wdg", "3");
        // XHL/XHT/XLT flow into the XSC slots in EndEdit (≤ 3 windings).
        check(&cls, &obj, "XSCArray", "[ 8 10 9]");
        // kVAs default the norm/emerg ratings to 1.1×/1.5× winding-1 kVA.
        check(&cls, &obj, "NormHkVA", "5500");
        check(&cls, &obj, "EmergHkVA", "7500");
    }

    #[test]
    fn pct_loadloss_splits_across_windings_one_and_two() {
        // %LoadLoss is split evenly between windings 1 and 2 (→ %R = 1.2 each).
        let (cls, obj) = edited(&[("windings", "2"), ("%loadloss", "2.4")]);
        check(&cls, &obj, "%Rs", "[1.2, 1.2, ]");
    }

    #[test]
    fn xscarray_explicit_overrides_default() {
        let (cls, obj) = edited(&[("windings", "3"), ("xscarray", "8 10 9")]);
        check(&cls, &obj, "XSCArray", "[ 8 10 9]");
        // No XHL/XHT/XLT edit → no EndEdit overwrite; XHL stays at default.
        check(&cls, &obj, "XHL", "7");
    }

    #[test]
    fn seasons_then_ratings_round_trip() {
        let (cls, obj) = edited(&[("seasons", "3"), ("ratings", "600, 700, 800")]);
        check(&cls, &obj, "Seasons", "3");
        check(&cls, &obj, "Ratings", "[ 600 700 800]");
    }

    #[test]
    fn make_like_copies_winding_web() {
        let enums = EnumRegistry::new();
        let cls = class_props(&enums);
        let (_, base) = edited(&[
            ("windings", "2"),
            ("kvs", "115, 4.16"),
            ("kvas", "3000, 3000"),
            ("xhl", "7"),
        ]);
        let mut obj = XfmrCodeObj::new("derived");
        obj.make_like(&base);
        check(&cls, &obj, "kVs", "[115, 4.16, ]");
        check(&cls, &obj, "NormHkVA", "3300");
        check(&cls, &obj, "XSCArray", "[ 7]");
    }
}
