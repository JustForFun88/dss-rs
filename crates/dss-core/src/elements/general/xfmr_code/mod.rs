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
//!
//! Split into submodules (no behavioral change): the struct, its constructor,
//! the read-only accessors, the winding-realloc helpers and the property table
//! live here; the `DssObject` trait impl (typed accessors, the per-winding
//! struct arrays, `PropertySideEffects`, `EndEdit`, `MakeLike`) is in
//! [`accessors`].

#[cfg(test)]
mod tests;

mod accessors;
mod dump;

use crate::elements::pd::winding::Winding;
use crate::obj::base::DssObjData;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags, prop_index};

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
        PropDef::double("kV")
            .flags(PropFlags::NON_NEGATIVE | PropFlags::REQUIRED | PropFlags::UNITS_KV),
        PropDef::double("kVA"),
        PropDef::double("Tap"),
        PropDef::double("%R").scale(pct),
        PropDef::double("RNeut").flags(PropFlags::UNITS_OHM),
        PropDef::double("XNeut").flags(PropFlags::UNITS_OHM),
        // General data (plural array forms write every winding).
        PropDef::enum_array_on_struct("Conns", enums.connection, WINDINGS),
        PropDef::double_array_on_struct("kVs", WINDINGS),
        PropDef::double_array_on_struct("kVAs", WINDINGS),
        PropDef::double_array_on_struct("Taps", WINDINGS),
        PropDef::double("XHL").scale(pct).flags(PropFlags::NON_ZERO),
        PropDef::double("XHT").scale(pct).flags(PropFlags::NON_ZERO),
        PropDef::double("XLT").scale(pct).flags(PropFlags::NON_ZERO),
        PropDef::double_v_array("XSCArray").scale(pct).flags(
            PropFlags::NON_ZERO | PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::DYNAMIC_DEFAULT,
        ),
        PropDef::double("Thermal").flags(PropFlags::UNITS_HOUR),
        PropDef::double("n"),
        PropDef::double("m"),
        PropDef::double("FLRise").flags(PropFlags::UNITS_DEGC),
        PropDef::double("HSRise").flags(PropFlags::UNITS_DEGC),
        PropDef::double("%LoadLoss"),
        PropDef::double("%NoLoadLoss"),
        PropDef::double("NormHkVA").flags(PropFlags::DYNAMIC_DEFAULT | PropFlags::UNITS_KVA),
        PropDef::double("EmergHkVA").flags(PropFlags::DYNAMIC_DEFAULT | PropFlags::UNITS_KVA),
        PropDef::double("MaxTap"),
        PropDef::double("MinTap"),
        PropDef::integer("NumTaps"),
        PropDef::double("%IMag"),
        PropDef::double("ppm_Antifloat").scale(1.0e-6),
        PropDef::double_array_on_struct("%Rs", WINDINGS).scale(pct),
        PropDef::double("X12")
            .scale(pct)
            .flags(PropFlags::NON_ZERO | PropFlags::REQUIRED_IN_SPEC_SET),
        PropDef::double("X13").scale(pct).flags(PropFlags::NON_ZERO),
        PropDef::double("X23").scale(pct).flags(PropFlags::NON_ZERO),
        PropDef::double("RDCOhms"),
        PropDef::integer("Seasons").flags(PropFlags::SUPPRESS_JSON),
        PropDef::double_array("Ratings", SEASONS),
    ];
    debug_assert_eq!(defs.len(), NUM_PROPS - 1);

    // JSON/schema metadata (Pascal `XfmrCode.pas:225-384`), mirroring Transformer:
    // the singular per-winding scalars carry an `array_alternative` to their plural
    // array form (rendered as the full per-winding array; the plural is `REDUNDANT`
    // and defers back), the scalars with no plural (RNeut/XNeut/Max/MinTap/RdcOhms/
    // NumTaps) are `ON_ARRAY`, and XHL/XHT/XLT are `REDUNDANT` aliases of X12/X13/X23.
    let mut defs = defs;
    {
        // (singular, plural) — bidirectional array_alternative / redundant_with.
        for (single, plural) in [
            ("kV", "kVs"),
            ("kVA", "kVAs"),
            ("Tap", "Taps"),
            ("%R", "%Rs"),
            ("Conn", "Conns"),
        ] {
            let si = prop_index(&defs, single);
            let pi = prop_index(&defs, plural);
            defs[si - 1].array_alternative = pi;
            defs[pi - 1].flags |= PropFlags::REDUNDANT;
            defs[pi - 1].redundant_with = si;
        }
        // kVs is additionally `Required` (Pascal `XfmrCode.pas:368`).
        let kvs = prop_index(&defs, "kVs");
        defs[kvs - 1].flags |= PropFlags::REQUIRED;
        // XHL/XHT/XLT are redundant aliases of X12/X13/X23 (`:321-326`).
        for (alias, canon) in [("XHL", "X12"), ("XHT", "X13"), ("XLT", "X23")] {
            let ai = prop_index(&defs, alias);
            let ci = prop_index(&defs, canon);
            defs[ai - 1].flags |= PropFlags::REDUNDANT;
            defs[ai - 1].redundant_with = ci;
        }
        // Per-winding scalars with no array alternative → ON_ARRAY.
        for name in ["RNeut", "XNeut", "MaxTap", "MinTap", "RDCOhms", "NumTaps"] {
            let i = prop_index(&defs, name);
            defs[i - 1].flags |= PropFlags::ON_ARRAY;
        }
        // The active-winding selector is a struct index → skipped by the sweep.
        let wdg = prop_index(&defs, "Wdg");
        defs[wdg - 1].flags |= PropFlags::INTEGER_STRUCT_INDEX;
    }
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
            data: DssObjData::new(name.into().to_ascii_lowercase(), prop::NUM_PROPS),
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
