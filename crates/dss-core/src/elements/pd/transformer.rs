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

use num_complex::Complex64;

use crate::elements::ckt::CktElementData;
use crate::elements::general::xfmr_code::XfmrCodeObj;
use crate::elements::pd::winding::Winding;
use crate::elements::traits::{CktElement, ElemRef, SysCtx};
use crate::obj::base::{DssObjData, DssObject};
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};
use crate::support::cmatrix::CMatrix;
use crate::util::{EPSILON, inv_sqrt3_x1000, sqrt3};

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
    // TPDClass tail:
    pub const NORMAMPS: usize = 50;
    pub const EMERGAMPS: usize = 51;
    pub const FAULTRATE: usize = 52;
    pub const PCTPERM: usize = 53;
    pub const REPAIR: usize = 54;
    // TCktElementClass tail:
    pub const BASE_FREQ: usize = 55;
    pub const ENABLED: usize = 56;
    pub const NUM_PROPS: usize = 57; // incl. Like
}

/// `TTransf.DefineProperties`.
pub fn class_props(enums: &EnumRegistry) -> ClassProps {
    use prop::*;
    let pct = 0.01;
    let defs = vec![
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
        // Read-only result string (winding currents mag/angle).
        PropDef::string("WdgCurrents"),
        PropDef::mapped_string_enum("Core", enums.core_type),
        PropDef::double("RDCOhms"),
        PropDef::integer("Seasons").flags(PropFlags::SUPPRESS_JSON),
        PropDef::double_array("Ratings", SEASONS),
        // TPDClass tail:
        PropDef::double("normamps").flags(PropFlags::SUPPRESS_JSON),
        PropDef::double("emergamps").flags(PropFlags::SUPPRESS_JSON),
        PropDef::double("faultrate"),
        PropDef::double("pctperm"),
        PropDef::double("repair"),
        // TCktElementClass tail:
        PropDef::double("basefreq").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::enabled("enabled"),
    ];
    debug_assert_eq!(defs.len(), NUM_PROPS - 1);
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
}

/// Pascal `XscSize`: `(NumWindings-1)·NumWindings/2`.
fn xsc_size(num_windings: i32) -> usize {
    let n = num_windings.max(0) as usize;
    if n >= 1 { (n - 1) * n / 2 } else { 0 }
}

/// Pascal `ZeroTapFix`: a 0 pu tap (which RegControl can force) becomes 0.0001.
fn zero_tap_fix(tap: f64) -> f64 {
    if tap == 0.0 { 0.0001 } else { tap }
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

    /// Active winding as a 0-based index, clamped into range.
    fn aw(&self) -> usize {
        (self.active_winding.clamp(1, self.num_windings.max(1)) - 1) as usize
    }

    /// Pascal `Get_PresentTap` (1-based winding; 0 out of range). Used by the
    /// Phase-5 RegControl tap driver.
    pub fn present_tap(&self, i: usize) -> f64 {
        if i >= 1 && i <= self.num_windings.max(0) as usize {
            self.windings[i - 1].putap
        } else {
            0.0
        }
    }

    /// Number of windings (`= NumberOfWindings = Nterms`). Used by RegControl.
    pub fn num_windings(&self) -> i32 {
        self.num_windings
    }

    /// `(PresentTap, MaxTap, MinTap, TapIncrement)` for 1-based winding `i`
    /// (zeros out of range). RegControl's `TapNum` get/set work off this.
    pub fn winding_tap_data(&self, i: usize) -> (f64, f64, f64, f64) {
        if i >= 1 && i <= self.num_windings.max(0) as usize {
            let w = &self.windings[i - 1];
            (w.putap, w.max_tap, w.min_tap, w.tap_increment)
        } else {
            (0.0, 0.0, 0.0, 0.0)
        }
    }

    /// Pascal `Set_PresentTap` (1-based winding): clamp to the winding's
    /// Min/MaxTap and, only on a change, invalidate YPrim and recompute.
    /// Returns whether YPrim was invalidated (Pascal `Set_YprimInvalid`'s
    /// `SystemYChanged := True` trigger fires on the same condition, gated by
    /// `Enabled`) — the RegControl driver uses this to raise `system_y_changed`.
    pub fn set_present_tap(&mut self, i: usize, value: f64) -> bool {
        if i < 1 || i > self.num_windings.max(0) as usize {
            return false;
        }
        let w = &self.windings[i - 1];
        let v = value.clamp(w.min_tap, w.max_tap);
        if v != w.putap {
            self.windings[i - 1].putap = v;
            self.cd.yprim_invalid = true;
            self.recalc();
            self.cd.enabled
        } else {
            false
        }
    }

    /// Pascal `Get_WdgConnection(i)`: the 1-based winding's connection code
    /// (0 = wye, 1 = delta). Used by RegControl's regulated-bus path.
    pub fn wdg_connection(&self, i: usize) -> i32 {
        if i >= 1 && i <= self.num_windings.max(0) as usize {
            self.windings[i - 1].connection
        } else {
            0
        }
    }

    /// Pascal `Get_BaseVoltage(i)`: the 1-based winding's `VBase`, falling back
    /// to winding 1 when out of range.
    pub fn base_voltage(&self, i: usize) -> f64 {
        if i >= 1 && i <= self.num_windings.max(0) as usize {
            self.windings[i - 1].vbase
        } else {
            self.windings[0].vbase
        }
    }

    /// Pascal `RotatePhases` exposed for the RegControl delta/regulated-bus path
    /// (returns a 1-based phase index).
    pub fn rotate_phases_1based(&self, iphs: usize) -> usize {
        self.rotate_phases(iphs)
    }

    /// Pascal `TTransfObj.GetWindingVoltages(iWind, VBuffer)` — the voltages
    /// across the `iWind` winding's phases. `vbuffer` is 0-based, length
    /// `nphases`; `node_v` is the global voltage vector. Ported from the
    /// 1-based Pascal (`VBuffer[i]`, `Vterminal[i + k]`) to 0-based indices.
    pub fn get_winding_voltages(
        &mut self,
        iwind: usize,
        node_v: &[Complex64],
        vbuffer: &mut [Complex64],
    ) {
        let nphases = self.cd.nphases;
        if !self.cd.enabled || self.cd.node_ref.is_empty() || node_v.is_empty() {
            return;
        }
        if iwind < 1 || iwind > self.num_windings.max(0) as usize {
            for v in vbuffer.iter_mut().take(self.cd.nconds) {
                *v = Complex64::ZERO;
            }
            return;
        }
        self.cd.compute_vterminal(node_v);
        let vt = &self.cd.vterminal;
        let nconds = self.cd.nconds;
        let k = (iwind - 1) * nconds; // offset for winding (0-based)
        let neut = nphases + k; // Pascal NeutTerm = Fnphases + k + 1 (1-based)
        let conn = self.windings[iwind - 1].connection;
        for i in 0..nphases {
            match conn {
                0 => vbuffer[i] = vt[i + k] - vt[neut], // Wye
                1 => {
                    // Delta: next phase in sequence (rotate_phases is 1-based).
                    let ii = self.rotate_phases(i + 1) - 1;
                    vbuffer[i] = vt[i + k] - vt[ii + k];
                }
                _ => {}
            }
        }
    }

    /// Pascal `Power[idxTerm].re` (watts) into terminal `term` — used by
    /// RegControl's reverse-power direction check. Sums `NodeV · conj(Iterminal)`
    /// over the terminal's conductors (zero refs skipped), ×3 under positive
    /// sequence.
    pub fn power_into(&mut self, term: usize, node_v: &[Complex64], sys: &SysCtx) -> Complex64 {
        if !self.cd.enabled || self.cd.node_ref.is_empty() {
            return Complex64::ZERO;
        }
        self.compute_iterminal(sys, node_v);
        let nconds = self.cd.nconds;
        let k = (term - 1) * nconds;
        let mut result = Complex64::ZERO;
        for i in 0..nconds {
            let n = self.cd.node_ref[k + i];
            if n > 0 {
                result += node_v[n] * self.cd.iterminal[k + i].conj();
            }
        }
        if sys.positive_sequence {
            result *= 3.0;
        }
        result
    }

    /// Pascal `TTransfObj.SetNumWindings`.
    fn set_num_windings(&mut self, n: i32) {
        let prev = self.num_windings;
        self.num_windings = n;
        self.realloc_windings(prev);
    }

    /// Pascal `PropertySideEffects(ord(windings), prev)`: reallocate windings,
    /// `XSC` (new slots → 0.30), terminals and the impedance matrices.
    fn realloc_windings(&mut self, prev_int: i32) {
        let old_xsc = xsc_size(prev_int);
        self.max_windings = self.num_windings;
        self.cd.nconds = self.cd.nphases + 1;
        let nw = self.num_windings.max(0) as usize;
        self.windings = vec![Winding::new(); nw];
        let new_xsc = xsc_size(self.num_windings);
        if new_xsc > old_xsc {
            self.xsc.resize(new_xsc, 0.30);
        } else {
            self.xsc.truncate(new_xsc);
        }
        // Nterms := NumWindings (reallocates bus names / terminals / buffers).
        self.cd.set_nterms(nw);
        self.zb = CMatrix::new(nw.saturating_sub(1));
        self.y_1volt = CMatrix::new(nw);
        self.y_1volt_nl = CMatrix::new(nw);
        self.y_term = CMatrix::new(2 * nw);
        self.y_term_nl = CMatrix::new(2 * nw);
    }

    /// Pascal `RotatePhases` (delta connections / line-line).
    fn rotate_phases(&self, iphs: usize) -> usize {
        let np = self.cd.nphases as i32;
        let mut result = iphs as i32 + self.delta_direction;
        if np > 2 {
            if result > np {
                result = 1;
            }
            if result < 1 {
                result = np;
            }
        } else if result < 1 {
            result = 3; // 2-phase delta: next phase is the 3rd phase
        }
        result as usize
    }

    /// Pascal `TTransfObj.SetTermRef`: map each winding's two conductors to the
    /// transformer's phase/neutral conductors per the winding connection.
    fn set_term_ref(&mut self) {
        let nw = self.num_windings.max(0) as usize;
        let np = self.cd.nphases;
        let nconds = self.cd.nconds;
        self.term_ref = vec![0; 2 * nw * np + 1];
        let mut k = 0usize;
        if np == 1 {
            for j in 1..=nw {
                k += 1;
                self.term_ref[k] = (j - 1) * nconds + 1;
                k += 1;
                self.term_ref[k] = j * nconds;
            }
        } else {
            for i in 1..=np {
                for j in 1..=nw {
                    k += 1;
                    match self.windings[j - 1].connection {
                        0 => {
                            // Wye
                            self.term_ref[k] = (j - 1) * nconds + i;
                            k += 1;
                            self.term_ref[k] = j * nconds;
                        }
                        1 => {
                            // Delta — second conductor connects to the next phase
                            self.term_ref[k] = (j - 1) * nconds + i;
                            k += 1;
                            self.term_ref[k] = (j - 1) * nconds + self.rotate_phases(i);
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    /// Pascal `TTransfObj.RecalcElementData`: derived per-winding data
    /// (`DeltaDirection`, `TermRef`, `XSC` from XHL, `VBase`, `Rdc`, anti-float,
    /// `NormAmps`/`EmergAmps`/`AmpRatings`) then `CalcY_Terminal` at base freq.
    fn recalc(&mut self) {
        // Determine Delta Direction. If HV winding is delta it leads wye.
        if self.windings[0].connection == self.windings[1].connection {
            self.delta_direction = 1;
        } else {
            let ihv = if self.windings[0].kvll >= self.windings[1].kvll {
                1
            } else {
                2
            };
            match self.windings[ihv - 1].connection {
                0 => self.delta_direction = if self.hv_leads_lv { -1 } else { 1 },
                1 => self.delta_direction = if self.hv_leads_lv { 1 } else { -1 },
                _ => {}
            }
        }

        self.set_term_ref();

        for w in &mut self.windings {
            w.tap_increment = if w.num_taps > 0 {
                (w.max_tap - w.min_tap) / w.num_taps as f64
            } else {
                0.0
            };
        }

        if self.xhl_changed {
            if self.num_windings <= 3 {
                let n = xsc_size(self.num_windings);
                let vals = [self.xhl, self.xht, self.xlt];
                for (i, v) in vals.iter().enumerate().take(n) {
                    self.xsc[i] = *v;
                }
            }
            self.xhl_changed = false;
        }

        // Winding voltage bases (volts).
        let np = self.cd.nphases;
        for w in &mut self.windings {
            match w.connection {
                0 => {
                    w.vbase = if np == 2 || np == 3 {
                        w.kvll * inv_sqrt3_x1000()
                    } else {
                        w.kvll * 1000.0
                    };
                }
                1 => w.vbase = w.kvll * 1000.0,
                _ => {}
            }
        }

        self.vabase = self.windings[0].kva * 1000.0;
        let vabase = self.vabase;

        // Rdc per winding.
        for w in &mut self.windings {
            if w.rdc_specified {
                w.rdcpu = w.rdcohms / (w.vbase * w.vbase / vabase);
            } else {
                w.rdcpu = (0.85 * w.rpu).abs();
                w.rdcohms = w.rdcpu * w.vbase * w.vbase / vabase;
            }
        }

        let ppm = self.ppm_float_factor;
        let vabase_1ph = vabase / np as f64;
        for w in &mut self.windings {
            w.compute_anti_float_adder(ppm, vabase_1ph);
        }

        // Normal/Emergency terminal current rating (UE check).
        let w1 = &self.windings[0];
        let vfactor = match w1.connection {
            0 => w1.vbase * 0.001, // wye
            1 => match np {
                1 => w1.vbase * 0.001,
                2 | 3 => w1.vbase * 0.001 / sqrt3(),
                _ => w1.vbase * 0.001 * 0.5 / (std::f64::consts::PI / np as f64).sin(),
            },
            _ => 1.0,
        };
        self.norm_amps = self.norm_max_hkva / np as f64 / vfactor;
        self.emerg_amps = self.emerg_max_hkva / np as f64 / vfactor;
        self.amp_ratings = self
            .kva_ratings
            .iter()
            .map(|r| 1.1 * r / np as f64 / vfactor)
            .collect();

        self.calc_y_terminal(1.0);
    }

    /// Pascal `TTransfObj.CalcY_Terminal`: build the `2·NumWindings` terminal
    /// admittance (`Y_Term`) and its no-load companion (`Y_Term_NL`) at the
    /// given frequency multiplier. GIC (`frequency < 0.51`) is Phase 7.
    fn calc_y_terminal(&mut self, freq_mult: f64) {
        let nw = self.num_windings.max(0) as usize;
        let rmult = if self.xrconst { freq_mult } else { 1.0 };

        // ZBMatrix (order NumWindings-1), pu → ohms on a one-volt base.
        let mut zb = CMatrix::new(nw - 1);
        let zbase = 1.0 / (self.vabase / self.cd.nphases as f64);
        for i in 0..nw - 1 {
            let v = Complex64::new(
                rmult * (self.windings[0].rpu + self.windings[i + 1].rpu),
                freq_mult * self.xsc[i],
            ) * zbase;
            zb.set(i, i, v);
        }
        // Off diagonals (running XSC index `k`, Pascal starts at NumWindings).
        let mut k = nw - 1;
        for i in 0..nw - 1 {
            for j in (i + 1)..(nw - 1) {
                let term = Complex64::new(
                    rmult * (self.windings[i + 1].rpu + self.windings[j + 1].rpu),
                    freq_mult * self.xsc[k],
                ) * zbase;
                let v = (zb.get(i, i) + zb.get(j, j) - term) * 0.5;
                zb.set(i, j, v);
                zb.set(j, i, v);
                k += 1;
            }
        }

        if zb.invert().is_err() {
            // Pascal error 117: replace with a tiny conductance to ground.
            zb.clear();
            for i in 0..zb.order() {
                zb.set(i, i, Complex64::new(EPSILON, 0.0));
            }
        }

        // Y_1Volt = AT · ZB⁻¹ · A (the N-1×N incidence). One phase, wye, 1 V.
        let mut y1 = CMatrix::new(nw);
        let mut y1nl = CMatrix::new(nw);
        let mut at = CMatrix::new(nw);
        for ip in 1..nw {
            at.set(ip, ip - 1, Complex64::new(1.0, 0.0));
            at.set(0, ip - 1, Complex64::new(-1.0, 0.0));
        }
        let mut a = vec![Complex64::ZERO; nw];
        let mut t1 = vec![Complex64::ZERO; nw];
        let mut t2 = vec![Complex64::ZERO; nw];
        for i in 1..=nw {
            for v in a.iter_mut() {
                *v = Complex64::ZERO;
            }
            if i == 1 {
                for v in a.iter_mut().take(nw - 1) {
                    *v = Complex64::new(-1.0, 0.0);
                }
            } else {
                a[i - 2] = Complex64::new(1.0, 0.0);
            }
            zb.mv_mult(&mut t1, &a); // ZB⁻¹ · A (order nw-1)
            t1[nw - 1] = Complex64::ZERO; // Pascal ctemparray1[NumWindings] := 0
            at.mv_mult(&mut t2, &t1); // AT · result (order nw)
            for (j, &val) in t2.iter().enumerate().take(nw) {
                y1.set(j, i - 1, val);
            }
        }

        // Magnetizing branch on winding 2 (closest to the core).
        y1nl.add(
            1,
            1,
            Complex64::new(
                self.pct_no_load_loss / 100.0 / zbase,
                -self.pct_imag / 100.0 / zbase / freq_mult,
            ),
        );

        // Y_Term = AT · Y_1Volt · A, corrected for the actual voltage ratings.
        let n2 = 2 * nw;
        let mut yterm = CMatrix::new(n2);
        let mut yterm_nl = CMatrix::new(n2);
        let mut at2 = CMatrix::new(n2);
        for i in 1..=nw {
            let denom = self.windings[i - 1].vbase * zero_tap_fix(self.windings[i - 1].putap);
            at2.set(2 * i - 2, i - 1, Complex64::new(1.0 / denom, 0.0));
            at2.set(2 * i - 1, i - 1, Complex64::new(-1.0 / denom, 0.0));
        }
        let mut av = vec![Complex64::ZERO; n2];
        let mut s1 = vec![Complex64::ZERO; n2];
        let mut s2 = vec![Complex64::ZERO; n2];
        for i in 1..=n2 {
            for v in av.iter_mut() {
                *v = Complex64::ZERO;
            }
            for kp in 1..=nw {
                let denom = self.windings[kp - 1].vbase * zero_tap_fix(self.windings[kp - 1].putap);
                if i == 2 * kp - 1 {
                    av[kp - 1] = Complex64::new(1.0 / denom, 0.0);
                } else if i == 2 * kp {
                    av[kp - 1] = Complex64::new(-1.0 / denom, 0.0);
                }
            }
            // Main transformer part.
            y1.mv_mult(&mut s1, &av); // order nw
            for v in s1.iter_mut().take(n2).skip(nw) {
                *v = Complex64::ZERO;
            }
            at2.mv_mult(&mut s2, &s1); // order n2
            for (j, &val) in s2.iter().enumerate().take(n2) {
                yterm.set(j, i - 1, val);
            }
            // No-load part.
            y1nl.mv_mult(&mut s1, &av);
            for v in s1.iter_mut().take(n2).skip(nw) {
                *v = Complex64::ZERO;
            }
            at2.mv_mult(&mut s2, &s1);
            for (j, &val) in s2.iter().enumerate().take(n2) {
                yterm_nl.set(j, i - 1, val);
            }
        }

        // Anti-float adders: a small admittance on both conductors of each
        // winding so the matrix always inverts even without a voltage ref.
        if self.ppm_float_factor != 0.0 {
            for i in 1..=nw {
                let yadder = Complex64::new(0.0, self.windings[i - 1].y_ppm);
                for j in (2 * i - 1)..=(2 * i) {
                    yterm.add(j - 1, j - 1, yadder);
                }
            }
        }

        self.zb = zb;
        self.y_1volt = y1;
        self.y_1volt_nl = y1nl;
        self.y_term = yterm;
        self.y_term_nl = yterm_nl;
        self.zbase = zbase;
        self.y_terminal_freqmult = freq_mult;
    }

    /// Pascal `BuildYPrimComponent`: stamp `Y_Terminal` into the phase-expanded
    /// `YPrim` component via `TermRef` (each entry goes in `nphases` times).
    fn build_yprim_component(
        yp: &mut CMatrix,
        yt: &CMatrix,
        term_ref: &[usize],
        nw: usize,
        np: usize,
    ) {
        let nw2 = 2 * nw;
        for i in 1..=nw2 {
            for j in 1..=i {
                let value = yt.get(i - 1, j - 1);
                for kk in 0..np {
                    let r = term_ref[i + kk * nw2];
                    let c = term_ref[j + kk * nw2];
                    yp.add_sym(r - 1, c - 1, value);
                }
            }
        }
    }

    /// Pascal `AddNeutralToY`: neutral grounding branches for wye windings
    /// (`Rneut`/`Xneut`; `Rneut < 0` = open, bumped by the anti-float adder).
    fn add_neutral_to_y(
        yps: &mut CMatrix,
        windings: &[Winding],
        nconds: usize,
        ppm: f64,
        freq_mult: f64,
    ) {
        for (i, w) in windings.iter().enumerate() {
            if w.connection != 0 {
                continue; // wye only (ignore delta and open wye)
            }
            let j = (i + 1) * nconds;
            if w.rneut >= 0.0 {
                let value = if w.rneut == 0.0 && w.xneut == 0.0 {
                    Complex64::new(1_000_000.0, 0.0) // solidly grounded
                } else {
                    Complex64::new(w.rneut, w.xneut * freq_mult).inv()
                };
                yps.add(j - 1, j - 1, value);
            } else if ppm != 0.0 {
                // Open neutral: bump admittance a bit in case it floats.
                yps.add(j - 1, j - 1, Complex64::new(0.0, w.y_ppm));
            }
        }
    }

    /// Pascal `TTransfObj.GetAllWindingCurrents`: `Iterm = Y_Term · Vterm`
    /// phase-by-phase, length `2·nphases·NumWindings`. Returns zeros when the
    /// element is not yet wired into the solution (Pascal `NodeRef = NIL`).
    fn get_all_winding_currents(&self) -> Vec<Complex64> {
        let nw = self.num_windings.max(0) as usize;
        let np = self.cd.nphases;
        let nconds = self.cd.nconds;
        let mut curr = vec![Complex64::ZERO; 2 * np * nw];
        if self.cd.node_ref.is_empty() {
            return curr;
        }
        let vterminal = &self.cd.vterminal;
        let mut vterm = vec![Complex64::ZERO; 2 * nw];
        let mut iterm = vec![Complex64::ZERO; 2 * nw];
        let mut iterm_nl = vec![Complex64::ZERO; 2 * nw];
        let mut kk = 0usize;
        for iphase in 1..=np {
            for iwind in 1..=nw {
                let neut_term = iwind * nconds;
                let i = 2 * iwind - 1; // 1-based into vterm
                match self.windings[iwind - 1].connection {
                    0 => {
                        vterm[i - 1] = vterminal[iphase + (iwind - 1) * nconds - 1];
                        vterm[i] = vterminal[neut_term - 1];
                    }
                    1 => {
                        let jphase = self.rotate_phases(iphase);
                        vterm[i - 1] = vterminal[iphase + (iwind - 1) * nconds - 1];
                        vterm[i] = vterminal[jphase + (iwind - 1) * nconds - 1];
                    }
                    _ => {}
                }
            }
            self.y_term.mv_mult(&mut iterm, &vterm);
            self.y_term_nl.mv_mult(&mut iterm_nl, &vterm);
            for i in 0..2 * nw {
                curr[kk] = iterm[i] + iterm_nl[i];
                kk += 1;
            }
        }
        curr
    }

    /// Pascal `GetWindingCurrentsResult`: the `mag, (angle), ` formatted string
    /// the `WdgCurrents` read-only property returns (one entry per phase ×
    /// winding; the other end of each winding is skipped).
    fn winding_currents_result(&self) -> String {
        let nw = self.num_windings.max(0) as usize;
        let np = self.cd.nphases;
        let curr = self.get_all_winding_currents();
        let mut out = String::new();
        let mut k = 0usize;
        for _ in 0..np {
            for _ in 0..nw {
                let c = curr[k];
                k += 1;
                let mag = c.norm();
                // Pascal Cdang: degrees; 0 for a zero phasor.
                let ang = if mag == 0.0 {
                    0.0
                } else {
                    c.arg().to_degrees()
                };
                // Pascal: Format('%.7g, (%.5g), ', [Cabs, Cdang]).
                out.push_str(&crate::util::fmt_g(mag, 7));
                out.push_str(", (");
                out.push_str(&crate::util::fmt_g(ang, 5));
                out.push_str("), ");
                k += 1; // skip the other end of the winding
            }
        }
        out
    }

    /// Pascal `TTransfObj.FetchXfmrCode`: copy the resolved `XfmrCode`'s whole
    /// winding web onto this transformer, then recompute.
    fn fetch_xfmr_code(&mut self, code: &XfmrCodeObj) {
        self.cd.nphases = code.fnphases().max(0) as usize;
        self.set_num_windings(code.num_windings());
        let nc = self.cd.nphases + 1;
        self.cd.set_nconds(nc);
        self.windings = code.windings().to_vec();
        self.set_term_ref();

        self.xhl = code.xhl();
        self.xht = code.xht();
        self.xlt = code.xlt();
        let n = xsc_size(self.num_windings);
        for i in 0..n {
            self.xsc[i] = code.xsc()[i];
        }
        for p in [
            prop::XHL,
            prop::XHT,
            prop::XLT,
            prop::X12,
            prop::X13,
            prop::X23,
            prop::XSCARRAY,
        ] {
            self.cd.obj.clear_seq(p);
        }

        self.thermal_time_const = code.thermal_time_const();
        self.n_thermal = code.n_thermal();
        self.m_thermal = code.m_thermal();
        self.flrise = code.flrise();
        self.hsrise = code.hsrise();
        self.pct_load_loss = code.pct_load_loss();
        self.pct_no_load_loss = code.pct_no_load_loss();
        self.pct_imag = code.pct_imag();
        self.norm_max_hkva = code.norm_max_hkva();
        self.emerg_max_hkva = code.emerg_max_hkva();
        self.ppm_float_factor = code.ppm_float_factor();
        self.cd.yprim_invalid = true;
        self.y_terminal_freqmult = 0.0;

        self.num_amp_ratings = code.num_kva_ratings();
        self.kva_ratings = code.kva_ratings().to_vec();

        self.recalc();
    }
}

impl CktElement for Transformer {
    fn cd(&self) -> &CktElementData {
        &self.cd
    }
    fn cd_mut(&mut self) -> &mut CktElementData {
        &mut self.cd
    }

    fn recalc_element_data(&mut self, _sys: &SysCtx) {
        self.recalc();
    }

    fn norm_amps(&self) -> f64 {
        self.norm_amps
    }
    fn emerg_amps(&self) -> f64 {
        self.emerg_amps
    }

    /// Pascal `TTransfObj.GetLosses` (Transformer.pas l.1635): no-load losses
    /// are the power into `Yprim_Shunt` from each terminal; load losses are the
    /// remainder of the total.
    fn get_losses_split(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
    ) -> (Complex64, Complex64, Complex64) {
        if !self.cd.enabled || self.cd.node_ref.is_empty() {
            return (Complex64::ZERO, Complex64::ZERO, Complex64::ZERO);
        }
        let total = self.losses(sys, node_v); // side effect: computes Iterminal
        let yorder = self.cd.yorder;
        self.cd.compute_vterminal(node_v);
        let mut no_load = Complex64::ZERO;
        if let Some(yshunt) = &self.cd.yprim_shunt {
            let mut temp = vec![Complex64::ZERO; yorder];
            yshunt.mv_mult(&mut temp, &self.cd.vterminal);
            for (v, t) in self.cd.vterminal.iter().zip(temp.iter()).take(yorder) {
                no_load += v * t.conj();
            }
        }
        let load = total - no_load;
        (total, load, no_load)
    }

    /// Pascal `TTransfObj.CalcYPrim`: stamp `Y_Term`/`Y_Term_NL` into the
    /// series/shunt YPrim via `TermRef`, add neutral branches, then apply the
    /// open-conductor corrections.
    fn calc_yprim(&mut self, sys: &SysCtx) {
        let yorder = self.cd.yorder;
        let nw = self.num_windings.max(0) as usize;
        let np = self.cd.nphases;

        let mut yp_series = CMatrix::new(yorder);
        let mut yp_shunt = CMatrix::new(yorder);
        let mut yprim = CMatrix::new(yorder);

        self.cd.yprim_freq = sys.frequency;
        let freq_mult = sys.frequency / self.cd.base_frequency;
        if freq_mult != self.y_terminal_freqmult {
            self.calc_y_terminal(freq_mult);
        }

        Self::build_yprim_component(&mut yp_series, &self.y_term, &self.term_ref, nw, np);
        Self::build_yprim_component(&mut yp_shunt, &self.y_term_nl, &self.term_ref, nw, np);
        Self::add_neutral_to_y(
            &mut yp_series,
            &self.windings,
            self.cd.nconds,
            self.ppm_float_factor,
            freq_mult,
        );

        yprim.copy_from(&yp_series);
        yprim.add_from(&yp_shunt);

        self.cd.yprim_series = Some(yp_series);
        self.cd.yprim_shunt = Some(yp_shunt);
        self.cd.yprim = Some(yprim);

        self.cd.apply_yprim_open_conductor_calcs();
        self.cd.yprim_invalid = false;
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

impl ControlledTransformer for Transformer {
    fn name(&self) -> &str {
        self.cd.obj.name()
    }
    fn n_phases(&self) -> usize {
        self.cd.nphases
    }
    fn n_conds(&self) -> usize {
        self.cd.nconds
    }
    fn y_order(&self) -> usize {
        self.cd.yorder
    }
    fn wdg_connection(&self, term: usize) -> i32 {
        Transformer::wdg_connection(self, term)
    }
    fn rotate_phases(&self, iphs: usize) -> usize {
        self.rotate_phases_1based(iphs)
    }
    fn base_voltage(&self, term: usize) -> f64 {
        Transformer::base_voltage(self, term)
    }
    fn present_tap(&self, w: usize) -> f64 {
        Transformer::present_tap(self, w)
    }
    fn min_tap(&self, w: usize) -> f64 {
        self.winding_tap_data(w).2
    }
    fn max_tap(&self, w: usize) -> f64 {
        self.winding_tap_data(w).1
    }
    fn tap_increment(&self, w: usize) -> f64 {
        self.winding_tap_data(w).3
    }
    fn set_present_tap(&mut self, w: usize, value: f64) -> bool {
        Transformer::set_present_tap(self, w, value)
    }
    fn power_into_re(&mut self, term: usize, node_v: &[Complex64], sys: &SysCtx) -> f64 {
        self.power_into(term, node_v, sys).re
    }
    fn winding_voltages(&mut self, term: usize, node_v: &[Complex64], vbuffer: &mut [Complex64]) {
        self.get_winding_voltages(term, node_v, vbuffer);
    }
    fn terminal_currents(&mut self, node_v: &[Complex64], sys: &SysCtx, cbuffer: &mut [Complex64]) {
        self.get_currents(sys, node_v, cbuffer);
    }
}

impl DssObject for Transformer {
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

    fn get_i32(&self, idx: usize) -> i32 {
        use prop::*;
        match idx {
            PHASES => self.cd.nphases as i32,
            WINDINGS => self.num_windings,
            WDG => self.active_winding,
            CONN => self.windings[self.aw()].connection,
            NUMTAPS => self.windings[self.aw()].num_taps,
            LEADLAG => self.hv_leads_lv as i32,
            CORE => self.core_type,
            SEASONS => self.num_amp_ratings,
            _ => unreachable!("Transformer has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        use prop::*;
        match idx {
            PHASES => self.cd.nphases = value.max(0) as usize,
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
            LEADLAG => self.hv_leads_lv = value != 0,
            CORE => self.core_type = value,
            SEASONS => self.num_amp_ratings = value,
            _ => unreachable!("Transformer has no integer property {idx}"),
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
            NORMAMPS => self.norm_amps,
            EMERGAMPS => self.emerg_amps,
            FAULTRATE => self.fault_rate,
            PCTPERM => self.pct_perm,
            REPAIR => self.hrs_to_repair,
            BASE_FREQ => self.cd.base_frequency,
            _ => unreachable!("Transformer has no double property {idx}"),
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
            NORMAMPS => self.norm_amps = value,
            EMERGAMPS => self.emerg_amps = value,
            FAULTRATE => self.fault_rate = value,
            PCTPERM => self.pct_perm = value,
            REPAIR => self.hrs_to_repair = value,
            BASE_FREQ => self.cd.base_frequency = value,
            _ => unreachable!("Transformer has no double property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        use prop::*;
        match idx {
            SUB => self.is_substation,
            XRCONST => self.xrconst,
            ENABLED => self.cd.enabled,
            _ => unreachable!("Transformer has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        use prop::*;
        match idx {
            SUB => self.is_substation = value,
            XRCONST => self.xrconst = value,
            ENABLED => self.cd.set_enabled(value),
            _ => unreachable!("Transformer has no boolean property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        use prop::*;
        match idx {
            SUBNAME => self.substation_name.clone(),
            BANK => self.xfmr_bank.clone(),
            XFMRCODE => self.xfmr_code_name.clone(),
            WDGCURRENTS => self.winding_currents_result(),
            _ => unreachable!("Transformer has no string property {idx}"),
        }
    }
    fn set_string(&mut self, idx: usize, value: String) {
        use prop::*;
        match idx {
            SUBNAME => self.substation_name = value,
            BANK => self.xfmr_bank = value,
            // WdgCurrents is a read-only result property (silent ignore).
            WDGCURRENTS => {}
            _ => unreachable!("Transformer has no string property {idx}"),
        }
    }

    fn get_f64_array(&self, idx: usize) -> Option<&[f64]> {
        match idx {
            prop::XSCARRAY => Some(&self.xsc),
            prop::RATINGS => Some(&self.kva_ratings),
            _ => unreachable!("Transformer has no array property {idx}"),
        }
    }
    fn set_f64_array(&mut self, idx: usize, value: Vec<f64>) {
        match idx {
            prop::XSCARRAY => self.xsc = value,
            prop::RATINGS => self.kva_ratings = value,
            _ => unreachable!("Transformer has no array property {idx}"),
        }
    }

    fn array_size(&self, idx: usize) -> usize {
        match idx {
            prop::XSCARRAY => xsc_size(self.num_windings),
            _ => unreachable!("Transformer has no function-sized array {idx}"),
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
                _ => unreachable!("Transformer has no struct array {idx}"),
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
                _ => unreachable!("Transformer has no struct array {idx}"),
            }
        }
        self.active_winding = self.num_windings;
    }

    fn get_struct_i32_array(&self, idx: usize) -> Vec<i32> {
        match idx {
            prop::CONNS => self.windings.iter().map(|w| w.connection).collect(),
            _ => unreachable!("Transformer has no struct enum array {idx}"),
        }
    }
    fn set_struct_i32_array(&mut self, idx: usize, values: &[i32]) {
        match idx {
            prop::CONNS => {
                for (w, v) in self.windings.iter_mut().zip(values) {
                    w.connection = *v;
                }
            }
            _ => unreachable!("Transformer has no struct enum array {idx}"),
        }
        self.active_winding = self.num_windings;
    }

    fn set_active_struct_bus(&mut self, value: &str) {
        let t = self.aw() + 1;
        self.cd.set_bus(t, value);
    }
    fn get_active_struct_bus(&self) -> String {
        self.cd.get_bus(self.aw() + 1).to_string()
    }
    fn set_struct_buses(&mut self, values: &[Option<String>]) {
        for (i, v) in values.iter().enumerate() {
            if let Some(v) = v {
                self.cd.set_bus(i + 1, v);
            }
        }
        self.active_winding = self.num_windings;
    }
    fn get_struct_buses(&self) -> Vec<String> {
        (1..=self.num_windings.max(0) as usize)
            .map(|t| self.cd.get_bus(t).to_string())
            .collect()
    }

    /// `xfmrcode=`: store the resolved code's name + ElemRef and copy its data
    /// immediately (Pascal `FetchXfmrCode`).
    fn set_object_ref(
        &mut self,
        idx: usize,
        name: String,
        resolved: Option<(ElemRef, &dyn DssObject)>,
    ) {
        match idx {
            prop::XFMRCODE => {
                self.xfmr_code_name = name;
                self.xfmr_code_ref = resolved.map(|(r, _)| r);
                if let Some((_, obj)) = resolved
                    && let Some(code) = obj.as_any().downcast_ref::<XfmrCodeObj>()
                {
                    self.fetch_xfmr_code(code);
                }
            }
            _ => unreachable!("Transformer has no resolved object-ref property {idx}"),
        }
    }

    /// Pascal `TTransfObj.PropertySideEffects`.
    fn side_effects(&mut self, idx: usize, prev_int: i32) {
        use prop::*;
        match idx {
            PHASES => {
                if self.cd.nphases as i32 != prev_int {
                    let nc = self.cd.nphases + 1;
                    self.cd.set_nconds(nc);
                }
            }
            CONN => {
                self.cd.yorder = self.cd.nconds * self.cd.nterms;
                self.cd.yprim_invalid = true;
            }
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
            XHL | XHT | XLT | X12 | X13 | X23 => {
                self.cd.obj.clear_seq(XSCARRAY);
                self.cd.obj.clear_seq(XFMRCODE);
                self.xhl_changed = true;
            }
            PCTLOADLOSS => {
                if self.windings.len() >= 2 {
                    let r = self.pct_load_loss / 2.0 / 100.0;
                    self.windings[0].rpu = r;
                    self.windings[1].rpu = r;
                }
            }
            RDCOHMS => {
                let w = self.aw();
                self.windings[w].rdc_specified = true;
            }
            SEASONS => self
                .kva_ratings
                .resize(self.num_amp_ratings.max(0) as usize, 0.0),
            XSCARRAY => {
                for p in [XHL, XHT, XLT, X12, X13, X23, XFMRCODE] {
                    self.cd.obj.clear_seq(p);
                }
            }
            _ => {}
        }

        // YPrim invalidation on anything that changes impedance values.
        if matches!(
            idx,
            TAP | TAPS
                | KV
                | KVA
                | PCTR
                | RNEUT
                | XNEUT
                | BUSES
                | CONNS
                | KVS
                | KVAS
                | PCTLOADLOSS
                | PCTNOLOADLOSS
                | PCTIMAG
                | PPM_ANTIFLOAT
                | PCTRS
                | XHL
                | XHT
                | XLT
                | X12
                | X13
                | X23
                | XSCARRAY
        ) {
            self.cd.yprim_invalid = true;
        }
    }

    /// Pascal base `EndEdit` → `RecalcElementData` (Transformer does not
    /// override `EndEdit`, unlike Line).
    fn end_edit(&mut self) {
        self.recalc();
    }

    /// Pascal `TTransfObj.MakeLike`.
    fn make_like(&mut self, other: &dyn DssObject) {
        let Some(o) = other.as_any().downcast_ref::<Transformer>() else {
            return;
        };
        self.cd.make_like_base(&o.cd);
        self.cd.nphases = o.cd.nphases;
        self.set_num_windings(o.num_windings);
        let nc = self.cd.nphases + 1;
        self.cd.set_nconds(nc); // forces terminal/conductor reallocation
        self.cd.yprim_invalid = true;

        self.windings.clone_from(&o.windings);
        self.set_term_ref();

        self.xhl = o.xhl;
        self.xht = o.xht;
        self.xlt = o.xlt;
        let n = xsc_size(self.num_windings);
        for i in 0..n {
            self.xsc[i] = o.xsc[i];
        }
        self.zb = o.zb.clone();
        self.y_1volt = o.y_1volt.clone();
        self.y_term = o.y_term.clone();
        self.y_1volt_nl = o.y_1volt_nl.clone();
        self.y_term_nl = o.y_term_nl.clone();

        self.thermal_time_const = o.thermal_time_const;
        self.n_thermal = o.n_thermal;
        self.m_thermal = o.m_thermal;
        self.flrise = o.flrise;
        self.hsrise = o.hsrise;
        self.pct_load_loss = o.pct_load_loss;
        self.pct_no_load_loss = o.pct_no_load_loss;
        self.norm_max_hkva = o.norm_max_hkva;
        self.emerg_max_hkva = o.emerg_max_hkva;
        self.xrconst = o.xrconst;

        self.xfmr_bank = o.xfmr_bank.clone();
        self.xfmr_code_name = o.xfmr_code_name.clone();
        self.xfmr_code_ref = o.xfmr_code_ref;

        self.num_amp_ratings = o.num_amp_ratings;
        self.kva_ratings.clone_from(&o.kva_ratings);
    }

    /// Target side of RegControl's deferred `TapNum` write (Pascal
    /// `Set_TapNum` pokes `tr.PresentTap[w]` directly).
    fn apply_ref_action(&mut self, action: &crate::obj::base::RefAction) {
        match action {
            crate::obj::base::RefAction::SetTransformerTap { winding, tap, .. } => {
                self.set_present_tap(*winding, *tap);
            }
        }
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elements::traits::SysCtx;
    use crate::obj::props::PropEngine;
    use crate::solution::SolveMode;
    use dss_parser::{Parser, ParserVars};

    /// Drive `(prop, value)` edits through the property engine, then `end_edit`
    /// (which recomputes). Mirrors the executive's edit loop without foreign
    /// class resolution (none of these tests reference an XfmrCode).
    fn edited(edits: &[(&str, &str)]) -> Transformer {
        let enums = EnumRegistry::new();
        let cls = class_props(&enums);
        let mut obj = Transformer::new("t");
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
        obj
    }

    fn test_sys() -> SysCtx {
        SysCtx {
            frequency: 60.0,
            fundamental: 60.0,
            is_harmonic_model: false,
            is_dynamic_model: false,
            load_model: 1,
            mode: SolveMode::Snapshot,
            load_multiplier: 1.0,
            gen_multiplier: 1.0,
            generator_dispatch_reference: 0.0,
            price_signal: 25.0,
            default_growth_factor: 1.0,
            year: 0,
            dbl_hour: 0.0,
            solution_count: 0,
            loads_need_updating: false,
            neglect_load_y: false,
            long_line_correction: false,
            positive_sequence: false,
        }
    }

    #[test]
    fn set_term_ref_3ph_wye_wye() {
        // 3-phase 2-winding wye-wye: nconds = 4. Each phase i maps winding j's
        // phase conductor `(j-1)*4 + i` and its neutral `j*4`.
        let t = edited(&[("phases", "3"), ("windings", "2"), ("conns", "wye, wye")]);
        // TermRef is 1-based with slot 0 unused. Layout: per phase i (1..3),
        // per winding j (1..2): [phaseCond, neutCond].
        // phase 1: w1 -> (1, 4), w2 -> (5, 8)
        assert_eq!(&t.term_ref[1..=4], &[1, 4, 5, 8]);
        // phase 2: w1 -> (2, 4), w2 -> (6, 8)
        assert_eq!(&t.term_ref[5..=8], &[2, 4, 6, 8]);
        // phase 3: w1 -> (3, 4), w2 -> (7, 8)
        assert_eq!(&t.term_ref[9..=12], &[3, 4, 7, 8]);
    }

    #[test]
    fn set_term_ref_3ph_wye_delta() {
        // Winding 2 delta: its second conductor connects to the next phase in
        // sequence (DeltaDirection from RotatePhases), not the neutral.
        let t = edited(&[
            ("phases", "3"),
            ("windings", "2"),
            ("kvs", "115, 4.16"),
            ("conns", "wye, delta"),
        ]);
        // HV (winding 1) is wye → DeltaDirection = +1, so phase i delta maps to
        // phase i+1 (wrapping 3→1).
        // phase 1: w1 wye -> (1, 4), w2 delta -> (5, 6)   [(2-1)*4 + rot(1)=2]
        assert_eq!(&t.term_ref[1..=4], &[1, 4, 5, 6]);
        // phase 2: w1 -> (2, 4), w2 delta -> (6, 7)
        assert_eq!(&t.term_ref[5..=8], &[2, 4, 6, 7]);
        // phase 3: w1 -> (3, 4), w2 delta -> (7, 5)  [rot(3)=1 → (2-1)*4+1=5]
        assert_eq!(&t.term_ref[9..=12], &[3, 4, 7, 5]);
    }

    #[test]
    fn yprim_1ph_wye_wye_matches_oracle() {
        // Oracle (dss-python 0.15.7) Yprim of
        //   Transformer.t1 phases=1 windings=2 buses=(a.1.0, b.1.0)
        //     conns=(wye,wye) kvs=(7.2,0.24) kvas=(25,25) xhl=2 %r=0.5
        // captured via ActiveCktElement.Yprim (order 4).
        let mut t = edited(&[
            ("phases", "1"),
            ("windings", "2"),
            ("buses", "a.1.0, b.1.0"),
            ("conns", "wye, wye"),
            ("kvs", "7.2, 0.24"),
            ("kvas", "25, 25"),
            ("xhl", "2"),
            ("%r", "0.5"),
        ]);
        let sys = test_sys();
        t.calc_yprim(&sys);
        let yprim = t.cd.yprim.as_ref().unwrap();
        assert_eq!(yprim.order(), 4);

        let expected = [
            [
                (0.007518, -0.021481),
                (-0.007518, 0.021481),
                (-0.225553, 0.644436),
                (0.225553, -0.644436),
            ],
            [
                (-0.007518, 0.021481),
                (0.007518, -0.021481),
                (0.225553, -0.644436),
                (-0.225553, 0.644436),
            ],
            [
                (-0.225553, 0.644436),
                (0.225553, -0.644436),
                (6.766580, -19.333086),
                (-6.766580, 19.333086),
            ],
            [
                (0.225553, -0.644436),
                (-0.225553, 0.644436),
                (-6.766580, 19.333086),
                (6.766580, -19.333086),
            ],
        ];
        for (i, row) in expected.iter().enumerate() {
            for (j, &(re, im)) in row.iter().enumerate() {
                let v = yprim.get(i, j);
                assert!(
                    (v.re - re).abs() < 1e-4 && (v.im - im).abs() < 1e-4,
                    "Yprim[{i},{j}] = {v} vs ({re}, {im})"
                );
            }
        }
    }

    #[test]
    fn set_present_tap_clamps_to_winding_limits() {
        let mut t = edited(&[("windings", "2"), ("kvs", "7.2, 0.24")]);
        // MaxTap defaults to 1.10; asking for 1.5 clamps there.
        t.set_present_tap(1, 1.5);
        assert!((t.present_tap(1) - 1.10).abs() < 1e-12);
        // MinTap defaults to 0.90.
        t.set_present_tap(1, 0.5);
        assert!((t.present_tap(1) - 0.90).abs() < 1e-12);
        // In range: applied verbatim.
        t.set_present_tap(2, 1.025);
        assert!((t.present_tap(2) - 1.025).abs() < 1e-12);
        // Out-of-range winding index is a no-op.
        t.set_present_tap(9, 1.0);
    }
}
