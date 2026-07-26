//! Port of `PCElements/UPFC.pas` — `TUPFCObj`, a Unified Power Flow Controller.
//! It is a **two-terminal** PC element (Bus1 = input, Bus2 = output) with a series
//! reactance `Xs` between the terminals (the `YPrim_series` block) and two current
//! injections — the output current `OutCurr` at terminal 2 and the input current
//! `InCurr` at terminal 1. The injections are *not* recomputed every solve: they
//! are persistent control state updated only when the companion `UPFCControl`
//! commands an `UploadCurrents` (the WP5.7 control sweep), driving the output-bus
//! voltage `Vbout` to `RefkV` within the `Tol1` dead-band.
//!
//! `GetOutputCurr` is a 5-mode dispatcher (0=Off, 1=VReg, 2=PAReg, 3=Dual,
//! 4=DoubleRef_V, 5=DoubleRef_Dual) with `VpqMax` clamping and a loss curve; the
//! `Sr0`/`Sr1` shift registers accumulate across control iterations. It is
//! **power-flow only** (the shift registers are control state, not differential
//! dynamics — there is no `InitStateVars`/`IntegrateStates`), but it exposes the
//! 14 `NumVariables` for Monitor mode 3.
//!
//! NOT_PORTED:
//! - The `TUPFCObj.Create` block that, on creating a *second* UPFC, casts the
//!   first UPFC object to a `TUPFCControlObj` and clears `.UPFCList`/`.ListSize`
//!   (UPFC.pas l.396). That cast reaches the wrong class (a `TUPFCObj` has no
//!   such fields) — undefined behavior that only ever runs with ≥2 UPFCs; the
//!   single-UPFC corpus deck never triggers it. The control rebuilds its list
//!   lazily anyway (`UPFCList.Count = 0` ⇒ `MakeUPFCList`), so the intent is
//!   already covered. Not reproduced (it cannot be expressed in safe Rust).
//! - `ZBase`/`CLimit`/`ERR0` fields: declared upstream but never read in any
//!   compute path (`ZBase` is only copied by `MakeLike`, `CLimit` is a parsed-only
//!   current limit, `ERR0` is a dead dual-mode error array). `CLimit` is kept as a
//!   property (round-trips); `ZBase`/`ERR0` are dropped as dead state.

#[cfg(test)]
mod tests;

mod accessors;
mod compute;
mod dump;

use num_complex::Complex64;

use crate::elements::ckt::CktElementData;
use crate::elements::general::spectrum::SpectrumObj;
use crate::elements::general::xy_curve::XyCurveObj;
use crate::elements::traits::ElemId;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};

/// Number of Monitor-mode-3 state variables (`NumUPFCVariables`).
pub const NUM_UPFC_VARIABLES: usize = 14;

/// 1-based property ordinals (Pascal `TUPFCProp` + the PC/CktElement tails).
pub mod prop {
    pub const BUS1: usize = 1;
    pub const BUS2: usize = 2;
    pub const REFKV: usize = 3;
    pub const PF: usize = 4;
    pub const FREQUENCY: usize = 5;
    pub const PHASES: usize = 6;
    pub const XS: usize = 7;
    pub const TOL1: usize = 8;
    pub const MODE: usize = 9;
    pub const VPQMAX: usize = 10;
    pub const LOSSCURVE: usize = 11;
    pub const VHLIMIT: usize = 12;
    pub const VLLIMIT: usize = 13;
    pub const CLIMIT: usize = 14;
    pub const REFKV2: usize = 15;
    pub const KVARLIMIT: usize = 16;
    pub const ELEMENT: usize = 17;
    // PCClass tail:
    pub const SPECTRUM: usize = 18;
    // CktElementClass tail:
    pub const BASE_FREQ: usize = 19;
    pub const ENABLED: usize = 20;
    pub const NUM_PROPS: usize = 21; // incl. Like
}

/// `TUPFC.DefineProperties`.
pub fn class_props(enums: &EnumRegistry) -> ClassProps {
    // Pascal `UPFC.pas:242-257`: PF is `PowerFactorLimits`, Frequency is
    // `Units_Hz`, kvarLimit is `Units_kvar` — schema-only flags (inert for the
    // text parse/dump path) now carried so the JSON schema walk renders them.
    let defs = vec![
        PropDef::bus("Bus1", 1).flags(PropFlags::REQUIRED),
        PropDef::bus("Bus2", 2).flags(PropFlags::REQUIRED),
        PropDef::double("RefkV"),
        PropDef::double("PF").flags(PropFlags::POWER_FACTOR_LIMITS),
        PropDef::double("Frequency").flags(
            PropFlags::DYNAMIC_DEFAULT
                | PropFlags::NON_NEGATIVE
                | PropFlags::NON_ZERO
                | PropFlags::UNITS_HZ,
        ),
        PropDef::integer("Phases").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::double("Xs"),
        PropDef::double("Tol1"),
        PropDef::mapped_int_enum("Mode", enums.upfc_mode),
        PropDef::double("VpqMax"),
        PropDef::object_ref_class("XYcurve", "LossCurve"),
        PropDef::double("VHLimit"),
        PropDef::double("VLLimit"),
        PropDef::double("CLimit"),
        PropDef::double("refkV2"),
        PropDef::double("kvarLimit").flags(PropFlags::UNITS_KVAR),
        // Pascal `PropertyOffset2 = 0` + PDElement flag: a monitored element by
        // full name (used only by the PF compensation modes).
        PropDef::object_ref_any("Element"),
        // PCClass tail:
        PropDef::object_ref_deferred("Spectrum", "Spectrum"),
        // CktElementClass tail:
        PropDef::double("BaseFreq").flags(
            PropFlags::DYNAMIC_DEFAULT
                | PropFlags::NON_NEGATIVE
                | PropFlags::NON_ZERO
                | PropFlags::UNITS_HZ,
        ),
        PropDef::enabled("Enabled"),
    ];
    debug_assert_eq!(defs.len(), prop::NUM_PROPS - 1);
    ClassProps::new("UPFC", defs, true)
}

/// `TUPFCObj`.
#[derive(Debug, Clone)]
pub struct Upfc {
    pub cd: CktElementData,

    /// `VRef` — expected output voltage magnitude (kV); property `refkV`.
    pub v_ref: f64,
    /// `pf` — expected power factor.
    pub pf: f64,
    /// `Xs` — series transformer reactance (ohms).
    pub xs: f64,
    /// `Tol1` — dead-band tolerance for controller 1.
    pub tol1: f64,
    /// `Freq` — operating frequency (Hz).
    pub freq: f64,
    /// `ModeUPFC` — control mode (0..5).
    pub mode_upfc: i32,
    /// `VpqMax` — maximum series-injection voltage magnitude (V).
    pub vpqmax: f64,
    /// `VHLimit`/`VLLimit` — input-voltage operating window (V).
    pub vh_limit: f64,
    pub vl_limit: f64,
    /// `CLimit` — maximum current (A); parsed only, never read in compute.
    pub c_limit: f64,
    /// `VRef2` — second voltage reference (kV) for the double-reference modes.
    pub v_ref2: f64,
    /// `kvarLim` — kvar absorption limit; property `kvarLimit`.
    pub kvar_lim: f64,

    /// `VRefD` — dynamic voltage reference (modes 4/5).
    pub v_ref_d: f64,
    /// `UPFCON` — true while the device operates within its voltage window.
    pub upfcon: bool,
    /// `SyncFlag` — dual-mode controller synchronization flag.
    pub sync_flag: bool,
    /// `SF2` — double-reference (modes 4/5) "do something" flag.
    pub sf2: bool,

    /// `Sr0` — shift register for controller 1 (one per phase, 0-based).
    pub sr0: Vec<Complex64>,
    /// `Sr1` — shift register for controller 2 (one per phase, 0-based).
    pub sr1: Vec<Complex64>,
    /// `InCurr`/`OutCurr` — the persistent input/output injection currents
    /// (one per phase, 0-based; Pascal stored 1-based with a dead slot 0).
    pub in_curr: Vec<Complex64>,
    pub out_curr: Vec<Complex64>,

    /// `Vbin`/`Vbout` — input/output terminal voltages cached by the most recent
    /// `GetInjCurrents` (the last phase's values for a multiphase model).
    pub vbin: Complex64,
    pub vbout: Complex64,

    // Reporting state (Monitor mode-3 variables 7..10 + 2).
    pub losses: f64,
    pub iupfc: Complex64,
    pub upfc_power: Complex64,
    pub qideal: f64,

    /// `UPFCLossCurveObj` — losses-vs-Vpu XYcurve reference (snapshot clone).
    pub loss_curve_name: String,
    pub loss_curve_obj: Option<XyCurveObj>,

    /// `MonElm` — monitored element for the PF-compensation modes.
    pub mon_elm: Option<ElemId>,
    pub mon_elm_name: String,

    pub spectrum: String,
    pub spectrum_obj: Option<SpectrumObj>,
}

impl Upfc {
    /// Pascal `TUPFCObj.Create`.
    pub fn new(name: &str) -> Self {
        let mut cd = CktElementData::new(name, prop::NUM_PROPS);
        cd.nphases = 1;
        cd.nconds = 1;
        cd.set_nterms(2); // a 2-terminal device

        let mut o = Self {
            cd,
            v_ref: 0.24,
            pf: 1.0,
            xs: 0.7540, // Xfmr series inductance 2e-3 H
            tol1: 0.02,
            freq: 60.0, // Round(ActiveCircuit.Fundamental); set at parse via basefreq default
            mode_upfc: 1,
            vpqmax: 24.0,
            vh_limit: 300.0,
            vl_limit: 125.0,
            c_limit: 265.0,
            v_ref2: 0.0,
            kvar_lim: 5.0,
            v_ref_d: 0.0,
            upfcon: true,
            sync_flag: false,
            sf2: false,
            sr0: vec![Complex64::ZERO; 1],
            sr1: vec![Complex64::ZERO; 1],
            in_curr: vec![Complex64::ZERO; 1],
            out_curr: vec![Complex64::ZERO; 1],
            vbin: Complex64::ZERO,
            vbout: Complex64::ZERO,
            losses: 0.0,
            iupfc: Complex64::ZERO,
            upfc_power: Complex64::ZERO,
            qideal: 0.0,
            loss_curve_name: String::new(),
            loss_curve_obj: None,
            mon_elm: None,
            mon_elm_name: String::new(),
            spectrum: "default".to_string(),
            spectrum_obj: None,
        };
        o.cd.set_nconds(1); // sync yorder = nconds * nterms
        o.recalc();
        o
    }

    /// Pascal `TUPFCObj.RecalcElementData`: (re)size the shift registers / current
    /// vectors / injection array. The series `Z = j·Xs` is rebuilt lazily in
    /// `calc_yprim` (it is a single diagonal value, so no matrix is stored).
    pub(super) fn recalc(&mut self) {
        let n = self.cd.nphases;
        self.qideal = 0.0;
        self.sr0.resize(n, Complex64::ZERO);
        self.sr1.resize(n, Complex64::ZERO);
        // Pascal `PropertySideEffects(phases)` resizes In/OutCurr; mirror here so a
        // phase change always leaves consistent vectors even without that hook.
        self.in_curr.resize(n, Complex64::ZERO);
        self.out_curr.resize(n, Complex64::ZERO);
        self.cd.inj_current = vec![Complex64::ZERO; self.cd.yorder];
    }
}
