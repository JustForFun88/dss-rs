//! Port of `PDElements/Transformer.pas` — `TTransfObj`, the multi-winding
//! transformer. Each winding becomes a terminal (`nterms = NumWindings`,
//! `nconds = nphases + 1`, the extra conductor the brought-out neutral). The
//! electrical core is `CalcY_Terminal` (a `2·NumWindings` admittance built from
//! the short-circuit reactance matrix `ZB`, the winding-ratio incidence and the
//! magnetizing branch), stamped phase-by-phase into `YPrim` through `TermRef`.
//!
//! GIC (`frequency < 0.51`) and harmonics interplay are deferred (Phase 7); the
//! 60 Hz power-flow path is complete. Per-winding data lives in the shared
//! [`Winding`] record (`Transformer.pas` `TWinding`).
//!
//! Split into submodules (this file holds the metadata, struct, `Create` and the
//! [`ControlledTransformer`] trait):
//! - [`windings`]: winding/tap queries and the structural reallocation +
//!   `TermRef` / `FetchXfmrCode` machinery (the RegControl-facing surface).
//! - [`yterminal`]: the electrical core — `RecalcElementData`, `CalcY_Terminal`,
//!   the `YPrim` stamping and the winding-current results.
//! - [`accessors`]: the `CktElement` / `ControlledTransformer` / `DssObject`
//!   trait impls.

#[cfg(test)]
mod tests;

use num_complex::Complex64;

use crate::elements::ckt::CktElementData;
use crate::elements::pd::winding::Winding;
use crate::elements::traits::{ElemRef, SysCtx};
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags, prop_index};
use crate::support::cmatrix::CMatrix;

mod accessors;
mod dump;
mod save;
mod windings;
mod yterminal;

/// 1-based property ordinals (Pascal `TTransfProp` + class tails).
pub mod prop {
    pub const PHASES: usize = 1;
    pub const WINDINGS: usize = 2;
    pub const WDG: usize = 3;
    pub const BUS: usize = 4;
    pub const CONN: usize = 5;
    pub const KV: usize = 6;
    pub const KVA: usize = 7;
    pub const TAP: usize = 8;
    pub const PCTR: usize = 9;
    pub const RNEUT: usize = 10;
    pub const XNEUT: usize = 11;
    pub const BUSES: usize = 12;
    pub const CONNS: usize = 13;
    pub const KVS: usize = 14;
    pub const KVAS: usize = 15;
    pub const TAPS: usize = 16;
    pub const XHL: usize = 17;
    pub const XHT: usize = 18;
    pub const XLT: usize = 19;
    pub const XSCARRAY: usize = 20;
    pub const THERMAL: usize = 21;
    pub const N: usize = 22;
    pub const M: usize = 23;
    pub const FLRISE: usize = 24;
    pub const HSRISE: usize = 25;
    pub const PCTLOADLOSS: usize = 26;
    pub const PCTNOLOADLOSS: usize = 27;
    pub const NORMHKVA: usize = 28;
    pub const EMERGHKVA: usize = 29;
    pub const SUB: usize = 30;
    pub const MAXTAP: usize = 31;
    pub const MINTAP: usize = 32;
    pub const NUMTAPS: usize = 33;
    pub const SUBNAME: usize = 34;
    pub const PCTIMAG: usize = 35;
    pub const PPM_ANTIFLOAT: usize = 36;
    pub const PCTRS: usize = 37;
    pub const BANK: usize = 38;
    pub const XFMRCODE: usize = 39;
    pub const XRCONST: usize = 40;
    pub const X12: usize = 41;
    pub const X13: usize = 42;
    pub const X23: usize = 43;
    pub const LEADLAG: usize = 44;
    pub const WDGCURRENTS: usize = 45;
    pub const CORE: usize = 46;
    pub const RDCOHMS: usize = 47;
    pub const SEASONS: usize = 48;
    pub const RATINGS: usize = 49;
    // GICharm BH-curve data props (dss_capi 0.15.x r4064, commit 90962ae8) —
    // `Unused` (parse+store only; never consumed by the port).
    pub const BHPOINTS: usize = 50;
    pub const BHCURRENT: usize = 51;
    pub const BHFLUX: usize = 52;
    // TPDClass tail:
    pub const NORMAMPS: usize = 53;
    pub const EMERGAMPS: usize = 54;
    pub const FAULTRATE: usize = 55;
    pub const PCTPERM: usize = 56;
    pub const REPAIR: usize = 57;
    // TCktElementClass tail:
    pub const BASE_FREQ: usize = 58;
    pub const ENABLED: usize = 59;
    pub const NUM_PROPS: usize = 60; // incl. Like
}

/// `TTransf.DefineProperties`.
pub fn class_props(enums: &EnumRegistry) -> ClassProps {
    use prop::*;
    let pct = 0.01;
    let mut defs = vec![
        PropDef::integer("Phases").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::integer("Windings").flags(PropFlags::GREATER_THAN_ONE | PropFlags::SUPPRESS_JSON),
        // Winding definition (active winding selected by `Wdg=`).
        PropDef::integer("Wdg"),
        PropDef::bus_on_struct("Bus"),
        PropDef::mapped_string_enum("Conn", enums.connection),
        PropDef::double("kV").flags(PropFlags::NON_NEGATIVE),
        PropDef::double("kVA"),
        PropDef::double("Tap"),
        PropDef::double("%R").scale(pct),
        PropDef::double("RNeut"),
        PropDef::double("XNeut"),
        // General data (plural array forms write every winding).
        PropDef::buses_on_struct("Buses", WINDINGS),
        PropDef::enum_array_on_struct("Conns", enums.connection, WINDINGS),
        PropDef::double_array_on_struct("kVs", WINDINGS).flags(PropFlags::NON_NEGATIVE),
        PropDef::double_array_on_struct("kVAs", WINDINGS),
        PropDef::double_array_on_struct("Taps", WINDINGS),
        PropDef::double("XHL").scale(pct).trap_zero(7.0),
        PropDef::double("XHT").scale(pct).trap_zero(35.0),
        PropDef::double("XLT").scale(pct).trap_zero(30.0),
        PropDef::double_v_array("XSCArray")
            .scale(pct)
            .flags(PropFlags::NON_ZERO),
        PropDef::double("Thermal"),
        PropDef::double("n"),
        PropDef::double("m"),
        PropDef::double("FLRise"),
        PropDef::double("HSRise"),
        PropDef::double("%LoadLoss"),
        PropDef::double("%NoLoadLoss"),
        PropDef::double("NormHkVA"),
        PropDef::double("EmergHkVA"),
        PropDef::boolean("Sub"),
        PropDef::double("MaxTap"),
        PropDef::double("MinTap"),
        PropDef::integer("NumTaps"),
        PropDef::string("SubName"),
        PropDef::double("%IMag"),
        PropDef::double("ppm_Antifloat").scale(1.0e-6),
        PropDef::double_array_on_struct("%Rs", WINDINGS).scale(pct),
        PropDef::string("Bank"),
        PropDef::object_ref_class("XfmrCode", "XfmrCode"),
        PropDef::boolean("XRConst"),
        PropDef::double("X12").scale(pct).trap_zero(7.0),
        PropDef::double("X13").scale(pct).trap_zero(35.0),
        PropDef::double("X23").scale(pct).trap_zero(30.0),
        PropDef::mapped_string_enum("LeadLag", enums.lead_lag),
        // Read-only result string (winding currents mag/angle); the render reads
        // the live `cd.vterminal`, so the `?`/`Dump` surfaces refresh it first.
        PropDef::string("WdgCurrents").flags(PropFlags::READS_VTERMINAL),
        PropDef::mapped_string_enum("Core", enums.core_type),
        PropDef::double("RDCOhms"),
        PropDef::integer("Seasons").flags(PropFlags::SUPPRESS_JSON),
        PropDef::double_array("Ratings", SEASONS),
        // GICharm BH-curve data (r4064, 90962ae8): `Unused` props — parsed and
        // stored, never used in a solve. `BHpoints` reallocates the two arrays
        // (side effect below); `BHcurrent`/`BHflux` are `BHpoints`-sized.
        PropDef::integer("BHPoints").flags(PropFlags::SUPPRESS_JSON | PropFlags::NON_NEGATIVE),
        PropDef::double_array("BHCurrent", BHPOINTS),
        PropDef::double_array("BHFlux", BHPOINTS),
        // TPDClass tail:
        PropDef::double("NormAmps").flags(PropFlags::SUPPRESS_JSON),
        PropDef::double("EmergAmps").flags(PropFlags::SUPPRESS_JSON),
        PropDef::double("FaultRate"),
        PropDef::double("pctPerm"),
        PropDef::double("Repair"),
        // TCktElementClass tail:
        PropDef::double("BaseFreq").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::enabled("Enabled"),
    ];
    debug_assert_eq!(defs.len(), NUM_PROPS - 1);

    // JSON metadata (Pascal `Transformer.pas:452,491-538,593-598`). The
    // singular per-winding scalars carry an `array_alternative` to their plural
    // array form, so the JSON default sweep renders them as the full per-winding
    // array; the plural forms are `REDUNDANT` and defer back to the singular.
    // The remaining per-winding scalars with no plural form (Rneut/Xneut/
    // Max/MinTap/RdcOhms/NumTaps) are `ON_ARRAY`: under `preferArray` they too
    // render the full per-winding array (`DSSObjectHelper.pas:1014/1054`).
    {
        // (singular, plural) — bidirectional array_alternative / redundant_with.
        for (single, plural) in [
            ("kV", "kVs"),
            ("kVA", "kVAs"),
            ("Tap", "Taps"),
            ("%R", "%Rs"),
            ("Bus", "Buses"),
            ("Conn", "Conns"),
        ] {
            let si = prop_index(&defs, single);
            let pi = prop_index(&defs, plural);
            defs[si - 1].array_alternative = pi;
            defs[pi - 1].flags |= PropFlags::REDUNDANT;
            defs[pi - 1].redundant_with = si;
        }
        // XHL/XHT/XLT are redundant aliases of X12/X13/X23 (REDUNDANT already set
        // via the flag mutation here).
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
    ClassProps::new("Transformer", defs, true)
}

/// `TTransfObj`.
#[derive(Debug, Clone)]
pub struct Transformer {
    pub cd: CktElementData,
    /// Pascal `ActiveWinding` (1-based).
    active_winding: i32,
    num_windings: i32,
    max_windings: i32,
    windings: Vec<Winding>,
    /// Pascal `XSC` — per-unit short-circuit reactances (`x12 x13 x23 …`).
    xsc: Vec<f64>,
    /// Pascal `TermRef`: winding-conductor → terminal-conductor map, 1-based
    /// with slot 0 unused; values are 1-based conductor indices into `YPrim`.
    term_ref: Vec<usize>,
    /// Short-circuit / one-volt / terminal admittance matrices (Pascal `ZB`,
    /// `Y_1Volt`, `Y_1Volt_NL`, `Y_Term`, `Y_Term_NL`).
    zb: CMatrix,
    y_1volt: CMatrix,
    y_1volt_nl: CMatrix,
    y_term: CMatrix,
    y_term_nl: CMatrix,
    y_terminal_freqmult: f64,
    delta_direction: i32,
    hv_leads_lv: bool,
    xrconst: bool,
    is_substation: bool,
    substation_name: String,
    xfmr_bank: String,
    xfmr_code_name: String,
    xfmr_code_ref: Option<ElemRef>,
    core_type: i32,
    xhl: f64,
    xht: f64,
    xlt: f64,
    /// Pascal `XHLChanged`: an XHL/XHT/XLT/X12/X13/X23 was set, so the leading
    /// `XSC` slots are refilled in `RecalcElementData`.
    xhl_changed: bool,
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
    vabase: f64,
    zbase: f64,
    // PD-element common:
    norm_amps: f64,
    emerg_amps: f64,
    fault_rate: f64,
    pct_perm: f64,
    hrs_to_repair: f64,
    num_amp_ratings: i32,
    kva_ratings: Vec<f64>,
    amp_ratings: Vec<f64>,
    // GICharm BH-curve data (Unused; r4064). `bh_current`/`bh_flux` are kept at
    // length `bh_points` by the BHpoints side effect.
    bh_points: i32,
    bh_current: Vec<f64>,
    bh_flux: Vec<f64>,
}

/// Pascal `XscSize`: `(NumWindings-1)·NumWindings/2`.
fn xsc_size(num_windings: i32) -> usize {
    let n = num_windings.max(0) as usize;
    if n >= 1 { (n - 1) * n / 2 } else { 0 }
}

impl Transformer {
    /// Pascal `TTransfObj.Create`.
    pub fn new(name: &str) -> Self {
        let mut cd = CktElementData::new(name, prop::NUM_PROPS);
        cd.nphases = 3;
        cd.nconds = 4;

        let mut t = Self {
            cd,
            active_winding: 1,
            num_windings: 0,
            max_windings: 0,
            windings: Vec::new(),
            xsc: Vec::new(),
            term_ref: vec![0],
            zb: CMatrix::new(0),
            y_1volt: CMatrix::new(0),
            y_1volt_nl: CMatrix::new(0),
            y_term: CMatrix::new(0),
            y_term_nl: CMatrix::new(0),
            y_terminal_freqmult: 0.0,
            delta_direction: 1,
            hv_leads_lv: false,
            xrconst: false,
            is_substation: false,
            substation_name: String::new(),
            xfmr_bank: String::new(),
            xfmr_code_name: String::new(),
            xfmr_code_ref: None,
            core_type: 0,
            xhl: 0.07,
            xht: 0.35,
            xlt: 0.30,
            xhl_changed: true,
            norm_max_hkva: 0.0,
            emerg_max_hkva: 0.0,
            thermal_time_const: 2.0,
            n_thermal: 0.8,
            m_thermal: 0.8,
            flrise: 65.0,
            hsrise: 15.0,
            pct_load_loss: 0.0,
            pct_no_load_loss: 0.0,
            ppm_float_factor: 0.000001,
            pct_imag: 0.0,
            vabase: 0.0,
            zbase: 0.0,
            norm_amps: 0.0,
            emerg_amps: 0.0,
            fault_rate: 0.007,
            pct_perm: 0.0,
            hrs_to_repair: 0.0,
            num_amp_ratings: 1,
            kva_ratings: vec![0.0],
            amp_ratings: vec![0.0],
            bh_points: 0,
            bh_current: Vec::new(),
            bh_flux: Vec::new(),
        };
        t.set_num_windings(2); // allocates windings, XSC, terminals, matrices
        t.active_winding = 1;

        let kva1 = t.windings[0].kva;
        t.vabase = kva1 * 1000.0;
        t.norm_max_hkva = 1.1 * kva1;
        t.emerg_max_hkva = 1.5 * kva1;
        t.pct_load_loss = 2.0 * t.windings[0].rpu * 100.0; // assume two windings

        let ppm = t.ppm_float_factor;
        let vabase_1ph = t.vabase / t.cd.nphases as f64;
        for w in &mut t.windings {
            w.compute_anti_float_adder(ppm, vabase_1ph);
        }

        t.num_amp_ratings = 1;
        t.kva_ratings = vec![t.norm_max_hkva];

        t.recalc();
        t
    }
}

/// The controlled-transformer surface RegControl's `Sample`/`DoPendingAction`
/// read and mutate (Pascal `TControlledTransformerObj` methods). It is a trait
/// so the regulator decision logic can be unit-tested against a lightweight mock
/// without a fully node-wired transformer; [`Transformer`] is the production
/// implementor. All winding/terminal indices are 1-based (as in Pascal); the
/// voltage/current buffers are 0-based, length `nphases`/`yorder`.
pub trait ControlledTransformer {
    fn name(&self) -> &str;
    /// Pascal `FullName` (`Class.name`) — RegControl's Series-connection guard
    /// message reports the controlled element's full name, so it names the
    /// concrete class (`Transformer.x` or `AutoTrans.x`).
    fn full_name(&self) -> String;
    fn n_phases(&self) -> usize;
    fn n_conds(&self) -> usize;
    fn y_order(&self) -> usize;
    fn wdg_connection(&self, term: usize) -> i32;
    /// `RotatePhases` (1-based in, 1-based out).
    fn rotate_phases(&self, iphs: usize) -> usize;
    fn base_voltage(&self, term: usize) -> f64;
    fn present_tap(&self, w: usize) -> f64;
    fn min_tap(&self, w: usize) -> f64;
    fn max_tap(&self, w: usize) -> f64;
    fn tap_increment(&self, w: usize) -> f64;
    /// Apply a tap; returns whether Y must be rebuilt (Pascal `SystemYChanged`).
    fn set_present_tap(&mut self, w: usize, value: f64) -> bool;
    /// `Power[term].re` in watts.
    fn power_into_re(&mut self, term: usize, node_v: &[Complex64], sys: &SysCtx) -> f64;
    /// `GetWindingVoltages(term, VBuffer)`.
    fn winding_voltages(&mut self, term: usize, node_v: &[Complex64], vbuffer: &mut [Complex64]);
    /// `ControlledElement.GetCurrents(CBuffer)`.
    fn terminal_currents(&mut self, node_v: &[Complex64], sys: &SysCtx, cbuffer: &mut [Complex64]);
}

/// View a [`DssObject`](crate::obj::base::DssObject) as a
/// [`ControlledTransformer`] — the Pascal `TControlledTransformerObj` base,
/// implemented by both `Transformer` and `AutoTrans` (the two members of
/// RegControl's `Transf_Or_AutoTrans_ProxyClass`, `RegControl.pas:264`). Used
/// by every surface that reaches the controlled transformer through a
/// RegControl reference (`Export`/`Show Taps`, the live `TapNum` reads).
pub fn as_controlled_transformer(
    obj: &dyn crate::obj::base::DssObject,
) -> Option<&dyn ControlledTransformer> {
    let any = obj.as_any();
    if let Some(t) = any.downcast_ref::<Transformer>() {
        return Some(t);
    }
    if let Some(t) = any.downcast_ref::<crate::elements::pd::auto_trans::AutoTrans>() {
        return Some(t);
    }
    None
}
