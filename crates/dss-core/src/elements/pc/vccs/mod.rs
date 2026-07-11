//! Port of `PCElements/vccs.pas` — `TVCCSObj`, a voltage-controlled current
//! source modelling a hardware (HW) inverter. It is a **one-terminal** ideal
//! current-source PC element (`YPrim = 0`): for power flow it injects a fixed
//! `BaseCurr` aligned with the terminal voltage (`Ppct` of rated, unity power
//! factor); for dynamics it runs a z-domain ring-buffer filter of the inverter
//! hardware model (`bp1` → `filter` → `bp2`), either in the time-domain waveform
//! mode or, when `RMSMode`, as a phasor-domain PLL.
//!
//! Split into submodules (this file holds the metadata, struct, `Create`,
//! `RecalcElementData`, the ring-buffer index helpers, and the injection-current
//! assembly `GetInjCurrents`):
//! - [`dynamics`]: `InitStateVars`/`InitPhasorStates`, `IntegrateStates`/
//!   `IntegratePhasorStates`, `ShutoffInjections`, and the 6 state variables the
//!   Monitor mode-3 path consumes.
//! - [`accessors`]: the `CktElement` / `DssObject` trait impls.
//!
//! `Fkv`/`Fki` are recomputed in `RecalcElementData` faithfully but are dead in
//! the upstream source too (no proc reads them).

#[cfg(test)]
mod tests;

mod accessors;
mod dynamics;

use num_complex::Complex64;

use crate::elements::ckt::CktElementData;
use crate::elements::general::spectrum::SpectrumObj;
use crate::elements::general::xy_curve::XyCurveObj;
use crate::elements::traits::SysCtx;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};
use crate::support::complexutil::{cdang, pdeg_to_complex};

/// 1-based property ordinals (Pascal `TVCCSProp` + the PC/CktElement tails).
pub mod prop {
    pub const BUS1: usize = 1;
    pub const PHASES: usize = 2;
    pub const PRATED: usize = 3;
    pub const VRATED: usize = 4;
    pub const PPCT: usize = 5;
    pub const BP1: usize = 6;
    pub const BP2: usize = 7;
    pub const FILTER: usize = 8;
    pub const FSAMPLE: usize = 9;
    pub const RMSMODE: usize = 10;
    pub const IMAXPU: usize = 11;
    pub const VRMSTAU: usize = 12;
    pub const IRMSTAU: usize = 13;
    // PCClass tail:
    pub const SPECTRUM: usize = 14;
    // CktElementClass tail:
    pub const BASE_FREQ: usize = 15;
    pub const ENABLED: usize = 16;
    pub const NUM_PROPS: usize = 17; // incl. Like
}

/// `TVCCS.DefineProperties`. (The enum registry is unused — `RMSMode` is a plain
/// `Boolean` property, not a mapped enum — but the signature mirrors the other PC
/// classes for uniform registration.)
pub fn class_props(_enums: &EnumRegistry) -> ClassProps {
    let defs = vec![
        PropDef::bus("Bus1", 1),
        PropDef::integer("Phases").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::double("PRated"),
        PropDef::double("VRated").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::double("Ppct"),
        PropDef::object_ref_class("XYcurve", "BP1"),
        PropDef::object_ref_class("XYcurve", "BP2"),
        PropDef::object_ref_class("XYcurve", "Filter"),
        PropDef::double("FSample").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::boolean("RMSMode"),
        PropDef::double("IMaxpu"),
        PropDef::double("VRMSTau"),
        PropDef::double("IRMSTau"),
        // PCClass tail:
        PropDef::object_ref("Spectrum"),
        // CktElementClass tail:
        PropDef::double("BaseFreq").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::enabled("Enabled"),
    ];
    debug_assert_eq!(defs.len(), prop::NUM_PROPS - 1);
    ClassProps::new("VCCS", defs, true)
}

/// `TVCCSObj`.
#[derive(Debug, Clone)]
pub struct Vccs {
    pub cd: CktElementData,

    // Properties (Pascal PUBLIC + PRIVATE fields).
    pub prated: f64,
    pub vrated: f64,
    pub ppct: f64,

    /// Object-reference curves (`bp1`/`bp2`/`filter`), stored by name plus a
    /// snapshot clone (the established Generator/PVSystem object-ref pattern).
    pub bp1_name: String,
    pub bp2_name: String,
    pub filter_name: String,
    pub fbp1: Option<XyCurveObj>,
    pub fbp2: Option<XyCurveObj>,
    pub ffilter: Option<XyCurveObj>,

    pub base_curr: f64,    // line current at Ppct
    pub base_volt: f64,    // line-to-neutral voltage at Vrated
    pub fsample_freq: f64, // discretization frequency for the Z filter
    pub fwinlen: usize,
    pub ffiltlen: usize,
    pub irated: f64, // line current at full output
    pub fkv: f64,    // scale voltage to HW pu input
    pub fki: f64,    // scale HW pu output to current

    pub frms_mode: bool, // phasor-domain PLL simulation
    pub fmax_ipu: f64,   // maximum RMS current in pu of rated
    pub fvrms_tau: f64,  // LPF time constant sensing Vrms
    pub firms_tau: f64,  // LPF time constant producing Irms

    // State variables for Dynamics Mode (PU of Vrated and BaseCurr).
    pub s1: f64,         // Vwave(t), or Vrms in phasor mode
    pub s2: f64,         // Iwave(t), or Ipwr in phasor mode
    pub s3: f64,         // Irms,     or Hout in phasor mode
    pub s4: f64,         // Ipeak,    or Irms in phasor mode
    pub s5: f64,         // BP1out,   or NA in phasor mode
    pub s6: f64,         // Hout,     or NA in phasor mode
    pub s_v1: Complex64, // positive-sequence voltage; use to inject I1 only

    pub vlast: Complex64,
    // Ring buffers, kept 1-based (index 0 unused) to mirror the Pascal
    // `pDoubleArray` 1..len indexing exactly.
    pub y2: Vec<f64>,
    pub z: Vec<f64>, // current digital-filter history terms
    pub whist: Vec<f64>,
    pub zlast: Vec<f64>, // update only after the corrector step
    pub wlast: Vec<f64>,
    pub s_idx_u: i64, // ring-buffer index for z and whist
    pub s_idx_y: i64, // ring-buffer index for y2 (rms current)
    pub y2sum: f64,

    // Harmonic spectrum (PCClass tail). VCCS injects no harmonic spectrum (the
    // model is a fixed current source), but the property is stored/dumped like
    // every other PC element.
    pub spectrum: String,
    pub spectrum_obj: Option<SpectrumObj>,
}

/// Pascal helper `MapIdx(idx, len)` — wrap a 1-based ring-buffer index into the
/// range `1..=len` (`while idx <= 0 do idx += len; idx mod (len+1)`, mapping a 0
/// result back to 1). Ported with `i64` arithmetic; the inputs can go negative.
fn map_idx(mut idx: i64, len: i64) -> usize {
    while idx <= 0 {
        idx += len;
    }
    let mut r = idx % (len + 1);
    if r == 0 {
        r = 1;
    }
    r as usize
}

/// Pascal helper `OffsetIdx(idx, offset, len)`.
fn offset_idx(idx: i64, offset: i64, len: i64) -> usize {
    map_idx(idx + offset, len)
}

impl Vccs {
    /// Pascal `TVCCSObj.Create`.
    pub fn new(name: &str) -> Self {
        let mut cd = CktElementData::new(name, prop::NUM_PROPS);
        cd.nphases = 1;
        cd.nconds = 1;
        cd.set_nterms(1);

        let mut o = Self {
            cd,
            prated: 250.0,
            vrated: 208.0,
            ppct: 100.0,
            bp1_name: String::new(),
            bp2_name: String::new(),
            filter_name: String::new(),
            fbp1: None,
            fbp2: None,
            ffilter: None,
            base_curr: 0.0,
            base_volt: 0.0,
            fsample_freq: 5000.0,
            fwinlen: 0,
            ffiltlen: 0,
            irated: 0.0,
            fkv: 1.0,
            fki: 1.0,
            frms_mode: false,
            fmax_ipu: 1.1,
            fvrms_tau: 0.0015,
            firms_tau: 0.0015,
            s1: 0.0,
            s2: 0.0,
            s3: 0.0,
            s4: 0.0,
            s5: 0.0,
            s6: 0.0,
            s_v1: Complex64::ZERO,
            vlast: Complex64::ZERO,
            y2: Vec::new(),
            z: Vec::new(),
            whist: Vec::new(),
            zlast: Vec::new(),
            wlast: Vec::new(),
            s_idx_u: 0,
            s_idx_y: 0,
            y2sum: 0.0,
            spectrum: "default".to_string(),
            spectrum_obj: None,
        };
        o.cd.set_nconds(1); // sync yorder = nconds * nterms
        o.recalc();
        o
    }

    /// Pascal `TVCCSObj.RecalcElementData`. Recomputes the rated quantities + the
    /// pu↔physical scale factors, and (only when a filter is set) (re)allocates the
    /// ring buffers from the filter length and the one-cycle window length.
    pub(super) fn recalc(&mut self) {
        self.cd.inj_current = vec![Complex64::ZERO; self.cd.yorder];

        let nphases = self.cd.nphases as f64;
        self.irated = self.prated / self.vrated / nphases;
        self.base_volt = self.vrated;
        if self.cd.nphases == 3 {
            let sqrt3 = 3.0_f64.sqrt();
            self.irated *= sqrt3;
            self.base_volt /= sqrt3;
        }
        self.base_curr = 0.01 * self.ppct * self.irated;
        self.fkv = 1.0 / self.base_volt / 2.0_f64.sqrt();
        self.fki = self.base_curr * 2.0_f64.sqrt();

        if let Some(filter) = self.ffilter.as_ref() {
            self.ffiltlen = filter.num_points();
            // Trunc toward zero (Pascal `Trunc`); both operands are positive here.
            self.fwinlen = (self.fsample_freq / self.cd.base_frequency).trunc() as usize;
            self.y2 = vec![0.0; self.fwinlen + 1];
            self.z = vec![0.0; self.ffiltlen + 1];
            self.whist = vec![0.0; self.ffiltlen + 1];
            self.wlast = vec![0.0; self.ffiltlen + 1];
            self.zlast = vec![0.0; self.ffiltlen + 1];
        }
    }

    /// Pascal `TVCCSObj.UpdateSequenceVoltage` — the positive-sequence terminal
    /// voltage `sV1` (used for the injected `I1` in phasor/RMS mode).
    pub(super) fn update_sequence_voltage(&mut self) {
        if self.cd.nphases == 3 {
            let v = &self.cd.vterminal;
            self.s_v1 = (v[0] + (alpha1() * v[1] + alpha2() * v[2])) / 3.0;
        } else {
            self.s_v1 = self.cd.vterminal[0];
        }
    }

    /// Pascal `TVCCSObj.GetInjCurrents` (l.441) — the solve path: fill
    /// `self.cd.inj_current` via [`Self::compute_inj_currents`].
    pub(super) fn get_inj_currents(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.cd.inj_current = self.compute_inj_currents(sys, node_v);
    }

    /// Pascal `TVCCSObj.GetInjCurrents` (l.441) — compute the current-source
    /// injection, **returning** the vector; `self.cd.inj_current` is left
    /// untouched so the reporting path stays side-effect-free (Pascal
    /// `TVCCSObj.GetCurrents` writes into the scratch `ComplexBuffer`, never
    /// `InjCurrent`). Three regimes: snapshot power flow (fixed `BaseCurr` at the
    /// terminal-voltage angle), waveform dynamics (the filtered RMS current `s3`),
    /// and RMS/phasor dynamics (`s4`, distributed as a balanced positive-sequence
    /// set off `sV1`). An open terminal injects nothing.
    pub(super) fn compute_inj_currents(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
    ) -> Vec<Complex64> {
        let nphases = self.cd.nphases;
        let mut inj = vec![Complex64::ZERO; self.cd.yorder];
        // Pascal `if not Closed[1]` (the active terminal's first conductor).
        if !self.cd.conductor_closed(1, 1) {
            return inj;
        }

        self.cd.compute_vterminal(node_v);
        self.update_sequence_voltage();

        if sys.is_dynamic_model {
            if self.frms_mode {
                let i1 = pdeg_to_complex(self.s4 * self.base_curr, cdang(self.s_v1));
                match nphases {
                    1 => inj[0] = i1,
                    3 => {
                        inj[0] = i1;
                        inj[1] = i1 * alpha2();
                        inj[2] = i1 * alpha1();
                    }
                    _ => {
                        for (i, slot) in inj.iter_mut().enumerate().take(nphases) {
                            *slot = pdeg_to_complex(
                                self.s4 * self.base_curr,
                                cdang(self.cd.vterminal[i]),
                            );
                        }
                    }
                }
            } else {
                for (i, slot) in inj.iter_mut().enumerate().take(nphases) {
                    *slot = pdeg_to_complex(self.s3 * self.base_curr, cdang(self.cd.vterminal[i]));
                }
            }
        } else {
            for (i, slot) in inj.iter_mut().enumerate().take(nphases) {
                *slot = pdeg_to_complex(self.base_curr, cdang(self.cd.vterminal[i]));
            }
        }
        inj
    }
}

/// Pascal `ALPHA1 := cmplx(-0.5, 0.5 * sqrt(3.0))` — `1 ∠ 120°`.
fn alpha1() -> Complex64 {
    Complex64::new(-0.5, 0.5 * 3.0_f64.sqrt())
}

/// Pascal `ALPHA2 := cmplx(-0.5, -ALPHA1.im)` — `1 ∠ 240°`.
fn alpha2() -> Complex64 {
    Complex64::new(-0.5, -0.5 * 3.0_f64.sqrt())
}
