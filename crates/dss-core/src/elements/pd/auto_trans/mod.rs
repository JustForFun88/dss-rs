//! Port of `PDElements/AutoTrans.pas` — `TAutoTransObj`, the autotransformer.
//! Created upstream (2018) *from* Transformer and sharing its
//! `TControlledTransformerObj` base, so the machinery mirrors
//! [`crate::elements::pd::transformer`] closely; the differences are the
//! auto-connection electrical model:
//!
//! - Windings: **Series** (`conn=s`, code 2), **Common/Wye** (code 0) and an
//!   optional **Delta** tertiary (code 1). `nconds = 2·nphases` (two conductors
//!   per winding — the series winding's second end is aliased onto the common
//!   winding's first node in `SetNodeRef`, "Magic happens here").
//! - Reactances are `XHX`/`XHT`/`XXT` (not `XHL`/`XHT`/`XLT`), and `RNeut`/
//!   `XNeut` are absent (the auto has no brought-out neutral impedance).
//!   `XfmrCode` *is* present (property 39, R4133_PROPS RP1.2) but is read
//!   through the auto's own `FetchXfmrCode`, which forces the first two
//!   windings' connections and remaps `XHL/XHT/XLT` onto `puXHX/puXHT/puXXT`.
//! - `CalcY_Terminal` applies the auto corrections (`ZCorrected`, the 3-winding
//!   `puXst`, `kVSeries`) — Dommel (6.45/6.46/6.50).
//!
//! The electrical core (`SetNodeRef` magic, `CalcY_Terminal`, `GICBuildYTerminal`,
//! `GetCurrents` fold, the Series arms of the winding readouts) lands in
//! WPG.15 Stage B; Stage A is the class skeleton (props, `RecalcElementData`,
//! `CalcY_Terminal`, dump) and defers the `CalcYPrim`/solve path behind a loud
//! error so the pending corpus decks stay red until Stage B.
//!
//! Split into submodules mirroring the transformer layout:
//! - [`windings`]: winding/tap queries, `SetTermRef`, `RotatePhases`, the winding
//!   reallocation and the RegControl-facing surface.
//! - [`yterminal`]: `RecalcElementData`, `CalcY_Terminal`, `GICBuildYTerminal`,
//!   the `YPrim` stamping and the winding-current results.
//! - [`accessors`]: the `CktElement` / `DssObject` trait impls.

#[cfg(test)]
mod tests;

use crate::elements::ckt::CktElementData;
use crate::elements::pd::transformer::CoreType;
use crate::elements::pd::winding::{Connection, TermRef, Winding};
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags, prop_index};
use crate::support::cmatrix::CMatrix;
use crate::util::sqrt3;

mod accessors;
mod dump;
mod save;
mod windings;
mod yterminal;

/// 1-based property ordinals (EPRI r4133 `TAutoTrans.DefineProperties`,
/// `Version8/Source/PDElements/AutoTrans.pas:270-336`, + the class tails).
///
/// Slot 39 is `XfmrCode` (R4133_PROPS RP1.2). dss_capi 0.14.5 deleted the row
/// outright — `//XfmrCode=39, // removed, unused`,
/// `.inputs/dss_capi/src/PDElements/AutoTrans.pas:76,125` — so the port's table
/// is one name longer than the pinned oracle's and the row is allowlisted
/// (`PROPS_015X`) + [`PropFlags::HIDE_R4133`].
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
    pub const RDCOHMS: usize = 10;
    pub const CORE: usize = 11;
    pub const BUSES: usize = 12;
    pub const CONNS: usize = 13;
    pub const KVS: usize = 14;
    pub const KVAS: usize = 15;
    pub const TAPS: usize = 16;
    pub const XHX: usize = 17;
    pub const XHT: usize = 18;
    pub const XXT: usize = 19;
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
    /// EPRI r4133 `PropertyName^[39] := 'XfmrCode'` (`AutoTrans.pas:329`).
    pub const XFMRCODE: usize = 39;
    pub const XRCONST: usize = 40;
    pub const LEADLAG: usize = 41;
    pub const WDGCURRENTS: usize = 42;
    // GICharm BH-curve data props (dss_capi 0.15.x r4064, commit 90962ae8) —
    // `Unused` (parse+store only; never consumed by the port).
    pub const BHPOINTS: usize = 43;
    pub const BHCURRENT: usize = 44;
    pub const BHFLUX: usize = 45;
    // TPDClass tail:
    pub const NORMAMPS: usize = 46;
    pub const EMERGAMPS: usize = 47;
    pub const FAULTRATE: usize = 48;
    pub const PCTPERM: usize = 49;
    pub const REPAIR: usize = 50;
    // TCktElementClass tail:
    pub const BASE_FREQ: usize = 51;
    pub const ENABLED: usize = 52;
    pub const NUM_PROPS: usize = 53; // incl. Like
}

/// `TAutoTrans.DefineProperties` (`AutoTrans.pas:364`).
pub fn class_props(enums: &EnumRegistry) -> ClassProps {
    use prop::*;
    let pct = 0.01;
    let mut defs = vec![
        PropDef::integer("Phases").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::integer("Windings")
            .flags(PropFlags::NON_ZERO | PropFlags::NON_NEGATIVE | PropFlags::SUPPRESS_JSON),
        // Winding definition (active winding selected by `Wdg=`).
        PropDef::integer("Wdg"),
        PropDef::bus_on_struct("Bus").flags(PropFlags::REQUIRED),
        PropDef::mapped_string_enum("Conn", enums.autotrans_connection),
        // `AutoTrans.pas:415` kV = Required + Units_kV + NonNegative.
        PropDef::double("kV")
            .flags(PropFlags::NON_NEGATIVE | PropFlags::REQUIRED | PropFlags::UNITS_KV),
        PropDef::double("kVA"),
        PropDef::double("Tap"),
        PropDef::double("%R").scale(pct),
        PropDef::double("RDCOhms"),
        PropDef::mapped_string_enum("Core", enums.core_type),
        // General data (plural array forms write every winding). The plural forms
        // are `REDUNDANT` (schema/JSON redirect to the singular) — wired below.
        PropDef::buses_on_struct("Buses", WINDINGS)
            .flags(PropFlags::REQUIRED | PropFlags::DYNAMIC_DEFAULT),
        PropDef::enum_array_on_struct("Conns", enums.autotrans_connection, WINDINGS),
        PropDef::double_array_on_struct("kVs", WINDINGS)
            .flags(PropFlags::NON_NEGATIVE | PropFlags::REQUIRED),
        PropDef::double_array_on_struct("kVAs", WINDINGS),
        PropDef::double_array_on_struct("Taps", WINDINGS),
        PropDef::double("XHX")
            .scale(pct)
            .trap_zero(7.0)
            .flags(PropFlags::REQUIRED_IN_SPEC_SET),
        PropDef::double("XHT").scale(pct).trap_zero(35.0),
        PropDef::double("XXT").scale(pct).trap_zero(30.0),
        PropDef::double_v_array("XSCArray")
            .scale(pct)
            .flags(PropFlags::NON_ZERO | PropFlags::REQUIRED_IN_SPEC_SET),
        PropDef::double("Thermal").flags(PropFlags::UNITS_HOUR),
        PropDef::double("n"),
        PropDef::double("m"),
        PropDef::double("FLRise").flags(PropFlags::UNITS_DEGC),
        PropDef::double("HSRise").flags(PropFlags::UNITS_DEGC),
        PropDef::double("%LoadLoss"),
        PropDef::double("%NoLoadLoss"),
        PropDef::double("NormHkVA").flags(PropFlags::DYNAMIC_DEFAULT | PropFlags::UNITS_KVA),
        PropDef::double("EmergHkVA").flags(PropFlags::DYNAMIC_DEFAULT | PropFlags::UNITS_KVA),
        PropDef::boolean("Sub"),
        PropDef::double("MaxTap"),
        PropDef::double("MinTap"),
        PropDef::integer("NumTaps"),
        PropDef::string("SubName"),
        PropDef::double("%IMag"),
        PropDef::double("ppm_Antifloat").scale(1.0e-6),
        PropDef::double_array_on_struct("%Rs", WINDINGS).scale(pct),
        // Property 38. **The port implements it; r4133 does not** — an inherited
        // divergence, recorded here because RP1.2 read this exact pair of Edit
        // arms. r4133 keeps the assignment commented out (`38: {XfmrBank :=
        // Param};`, `AutoTrans.pas:519`) and answers the write with
        // `DoSimpleMsg('Bank Property not used with AutoTrans object.', 100130)`
        // (`:566`), so its `XfmrBank` is always `''` and every auto is its own
        // bank in the CIM export (`ExportCIMXML.pas:3280`). dss_capi 0.14.5
        // restored the property (`PropertyOffset[ord(TProp.Bank)] :=
        // ptruint(@obj.XfmrBank)`, `.inputs/dss_capi/src/PDElements/`
        // `AutoTrans.pas:482`) and the port follows it: the name is stored and
        // is the CIM bank-grouping key (`cim/power_xfmr.rs`), and #100130 is
        // never logged. Unlike the `XfmrCode` arm below, this r4133 pair is
        // self-consistent — a deliberate upstream limitation, not a bug — so it
        // is neither reported upstream nor "fixed" here; changing it would drop
        // a working feature and move CIM output. Latent as of 2026-08-23: no
        // corpus deck writes `bank=` on an AutoTrans (every `bank=` in the
        // corpus is a Transformer), the property echo agrees on both channels,
        // and the CIM goldens are capi-captured. Undecided, unowned — see
        // STATUS §RP1.2.
        PropDef::string("Bank"),
        // EPRI r4133 property 39 (`AutoTrans.pas:329`, help `:414`), read by the
        // auto's OWN `TAutoTransObj.FetchXfmrCode` (`:520` → `:2339-2396`), not
        // by the Transformer's. Absent from BOTH pinned tables (0.14.5 deleted
        // it, capi015 with it), hence `HIDE_R4133`: the row keeps its `?`/props
        // surface and its `AltPropertyOrder` slot but is skipped by the
        // 0.14.5-pinned full-enumeration surfaces (Dump / `Dump commands` /
        // JSON / schema). It carries no `ORDERING_FIRST`: that flag exists to
        // make the JSON reader apply a Transformer's `XfmrCode` before the
        // per-winding props it would otherwise overwrite, and a `HIDE_R4133` row
        // never appears in an exported or imported AltDSS document at all.
        PropDef::object_ref_class("XfmrCode", "XfmrCode")
            .flags(PropFlags::HIDE_R4133)
            .ref_miss_msg(100180, "Xfmr Code:"),
        PropDef::boolean("XRConst"),
        PropDef::mapped_string_enum("LeadLag", enums.lead_lag),
        // Read-only result string (winding currents mag/angle); the render reads
        // the live `cd.vterminal`, so the `?`/`Dump` surfaces refresh it first.
        PropDef::string("WdgCurrents").flags(PropFlags::READS_VTERMINAL),
        // GICharm BH-curve data (r4064, 90962ae8): `Unused` props — parsed and
        // stored, never used in a solve (mirrors the Transformer port). Absent
        // from the pinned oracle (dss_capi 0.14.5), so all three carry
        // `SUPPRESS_JSON` to keep the Full JSON dump byte-identical.
        PropDef::integer("BHPoints").flags(PropFlags::SUPPRESS_JSON | PropFlags::NON_NEGATIVE),
        PropDef::double_array("BHCurrent", BHPOINTS).flags(PropFlags::SUPPRESS_JSON),
        PropDef::double_array("BHFlux", BHPOINTS).flags(PropFlags::SUPPRESS_JSON),
        // TPDClass tail. Unlike Transformer, AutoTrans does NOT flag NormAmps/
        // EmergAmps `SuppressJSON` (`AutoTrans.pas` has no such override), so both
        // are emitted in the schema/JSON exactly as the pinned oracle shows.
        PropDef::double("NormAmps"),
        PropDef::double("EmergAmps"),
        PropDef::double("FaultRate"),
        PropDef::double("pctPerm"),
        PropDef::double("Repair"),
        // TCktElementClass tail:
        PropDef::double("BaseFreq").flags(
            PropFlags::DYNAMIC_DEFAULT
                | PropFlags::NON_NEGATIVE
                | PropFlags::NON_ZERO
                | PropFlags::UNITS_HZ,
        ),
        PropDef::enabled("Enabled"),
    ];
    debug_assert_eq!(defs.len(), NUM_PROPS - 1);

    // JSON metadata (Pascal `AutoTrans.pas:439-557`). Each per-winding singular
    // scalar carries an `array_alternative` to its plural array form (rendered as
    // the full per-winding array in the JSON/schema sweep); the plural forms are
    // `REDUNDANT` and defer back to the singular. Per-winding scalars with no
    // plural (RDCOhms/MaxTap/MinTap/NumTaps) are `ON_ARRAY` (also render as the
    // per-winding array under `preferArray`). `Wdg` is the struct-array index.
    {
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
        for name in ["RDCOhms", "MaxTap", "MinTap", "NumTaps"] {
            let i = prop_index(&defs, name);
            defs[i - 1].flags |= PropFlags::ON_ARRAY;
        }
        let wdg = prop_index(&defs, "Wdg");
        defs[wdg - 1].flags |= PropFlags::INTEGER_STRUCT_INDEX;
    }

    ClassProps::new("AutoTrans", defs, true)
}

/// `TAutoTransObj`.
#[derive(Debug, Clone)]
pub struct AutoTrans {
    pub cd: CktElementData,
    /// Pascal `ActiveWinding` (1-based).
    active_winding: i32,
    num_windings: i32,
    max_windings: i32,
    windings: Vec<Winding>,
    /// Pascal `puXSC` — per-unit short-circuit reactances (`xhx xht xxt …`).
    xsc: Vec<f64>,
    /// Pascal `TermRef`: winding-conductor → terminal-conductor map, one 0-based
    /// `[plus, minus]` pair per phase × winding (phase-major).
    term_ref: TermRef,
    /// Short-circuit / one-volt / terminal admittance matrices (Pascal `ZB`,
    /// `Y_1Volt`, `Y_1Volt_NL`, `Y_Term`, `Y_Term_NL`).
    zb: CMatrix,
    y_1volt: CMatrix,
    y_1volt_nl: CMatrix,
    y_term: CMatrix,
    y_term_nl: CMatrix,
    y_terminal_freqmult: f64,
    /// The Rust stand-in for Pascal's global `ActiveCircuit.Solution.Frequency`,
    /// which `TAutoTransObj.CalcY_Terminal` reads for the `< 0.51 Hz` GIC/dc gate
    /// (`AutoTrans.pas`). Refreshed by `CalcYPrim` at every solve and by the
    /// executive at each New/Edit boundary, so the `RecalcElementData`
    /// (`calc_y_terminal(1.0, ..)`) path reads the live frequency instead of
    /// reconstructing the base frequency. Default `60` (the DSS base frequency).
    live_frequency: f64,
    delta_direction: i32,
    hv_leads_lv: bool,
    xrconst: bool,
    is_substation: bool,
    substation_name: String,
    xfmr_bank: String,
    /// Pascal `XfmrCode: String` (`AutoTrans.pas:164`) — the resolved library
    /// entry's name, lowercased (`:2349`), `''` until one resolves (`:905`).
    /// Only the name is kept: unlike the Transformer's typed `Idx<XfmrCodeObj>`
    /// (which the CIM `PowerTransformer` writer reads back) nothing in the port
    /// re-reads an auto's code after the copy, and r4133 keeps only the string
    /// too.
    xfmr_code: String,
    core_type: CoreType,
    /// Pascal `puXHX`/`puXHT`/`puXXT` — per-unit reactances between winding pairs.
    puxhx: f64,
    puxht: f64,
    puxxt: f64,
    /// Pascal `kVSeries` — rating for the Series winding.
    kv_series: f64,
    /// Pascal `XHXChanged`: an XHX/XHT/XXT was set, so the leading `puXSC` slots
    /// are refilled in `RecalcElementData`.
    xhx_changed: bool,
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
    // GICharm BH-curve data (Unused; r4064). Kept at length `bh_points` by the
    // BHpoints side effect.
    bh_points: i32,
    bh_current: Vec<f64>,
    bh_flux: Vec<f64>,
}

/// Pascal `XscSize`: `(NumWindings-1)·NumWindings/2`.
fn xsc_size(num_windings: i32) -> usize {
    let n = num_windings.max(0) as usize;
    if n >= 1 { (n - 1) * n / 2 } else { 0 }
}

/// Pascal `TAutoWinding.Init(iWinding)`: winding 1 is the **Series** winding
/// (115 kV), all others the **Common/Wye** default (12.47 kV). Reuses the shared
/// [`Winding`] record (its `rneut`/`xneut` fields are unused by the auto — there
/// is no brought-out neutral impedance).
fn auto_winding_init(iwinding: usize) -> Winding {
    let (connection, kvll) = if iwinding == 1 {
        (Connection::Series, 115.0)
    } else {
        (Connection::Wye, 12.47)
    };
    let kva = 1000.0;
    let rpu = 0.002;
    let rdcpu = rpu * 0.85;
    let vbase = kvll / sqrt3() * 1000.0;
    let mut w = Winding {
        connection,
        kvll,
        vbase,
        kva,
        putap: 1.0,
        rpu,
        rdcpu,
        // Pascal: RdcOhms := Sqr(kVLL) / (kVA / 1000) * Rdcpu (placeholder;
        // RecalcElementData recomputes it from the series VBase).
        rdcohms: kvll * kvll / (kva / 1000.0) * rdcpu,
        rdc_specified: false,
        rneut: -1.0, // unused by AutoTrans
        xneut: 0.0,
        y_ppm: 0.0,
        tap_increment: 0.00625,
        min_tap: 0.90,
        max_tap: 1.10,
        num_taps: 32,
    };
    // Pascal Init: ComputeAntiFloatAdder(1.0e-6, kVA / 3 / 1000).
    w.compute_anti_float_adder(1.0e-6, kva / 3.0 / 1000.0);
    w
}

impl AutoTrans {
    /// Pascal `TAutoTransObj.Create` (`AutoTrans.pas:820`).
    pub fn new(name: &str) -> Self {
        let mut cd = CktElementData::new(name, prop::NUM_PROPS);
        cd.nphases = 3;
        cd.nconds = 2 * cd.nphases; // two conductors per phase (auto)

        let mut t = Self {
            cd,
            active_winding: 1,
            num_windings: 0,
            max_windings: 0,
            windings: Vec::new(),
            xsc: Vec::new(),
            term_ref: TermRef::default(),
            zb: CMatrix::new(0),
            y_1volt: CMatrix::new(0),
            y_1volt_nl: CMatrix::new(0),
            y_term: CMatrix::new(0),
            y_term_nl: CMatrix::new(0),
            y_terminal_freqmult: 0.0,
            live_frequency: 60.0,
            delta_direction: 1,
            hv_leads_lv: false,
            xrconst: false,
            is_substation: false,
            substation_name: String::new(),
            xfmr_bank: String::new(),
            xfmr_code: String::new(), // Pascal `XfmrCode := ''` (`AutoTrans.pas:905`)
            core_type: CoreType::Shell,
            puxhx: 0.10,
            puxht: 0.35,
            puxxt: 0.30,
            kv_series: 0.0,
            xhx_changed: true,
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
            bh_points: 0,
            bh_current: Vec::new(),
            bh_flux: Vec::new(),
        };
        t.set_num_windings(2); // allocates windings, puXSC, terminals, matrices
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

        t.recalc();
        t
    }

    /// Sync the cached live `ActiveCircuit.Solution.Frequency` (Pascal reads the
    /// global directly in `CalcY_Terminal` for the `< 0.51 Hz` GIC gate). The
    /// executive calls this at each New/Edit boundary; `CalcYPrim` refreshes it
    /// again per solve.
    pub fn set_live_frequency(&mut self, frequency: f64) {
        self.live_frequency = frequency;
    }
}
