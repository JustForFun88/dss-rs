//! Port of `PCElements/VSConverter.pas` — `TVSConverterObj`, a voltage-source
//! converter (an AC/DC bridge). It is a **two-terminal** PC element: the first
//! `phases - Ndc` conductors are AC, the last `Ndc` are DC. Power flow models the
//! AC side as a voltage source behind `Rac + jXac` (the `YPrim_series` admittance
//! block) and the DC side as a power-balance current source (`Idc = Pac/|Vdc|`,
//! clamped to `±IDCMax·kW/kVDC`).
//!
//! Scope: the `VSCMode = Fixed` injection (Pascal `GetInjCurrents` only ever uses
//! the fixed modulation `M0`/`d0` — the PacVac/PacQac/VdcVac/VdcQac control modes
//! parse + dump but Pascal never branches on them, so there is no extra behavior).
//! No dynamics state (power-flow only); the spectrum property rides on the base.

use num_complex::Complex64;

use crate::elements::ckt::CktElementData;
use crate::elements::general::spectrum::SpectrumObj;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};
use crate::support::complexutil::rotate_phasor_deg;
use crate::util::EPSILON;

/// Pascal `TVSConverterObj.Fmode` (`VSConverter.pas:95`, `Integer`) — the
/// `VSCMode=` control mode.
///
/// Backed by the `'VSConverter: Control Mode'` `DssEnum`
/// (`VSConverter.pas:152-155`, names `['Fixed', 'PacVac', 'PacQac', 'VdcVac',
/// 'VdcQac']`, values `[0, 1, 2, 3, 4]`, `DefaultValue = VSC_FIXED`), whose
/// ordinals are the `VSC_*` constants at `:136-140`. `Create` seeds
/// [`Self::Fixed`] (`:312`).
///
/// The field is **store-only in both engines**: upstream never reads `Fmode`
/// (its only mentions are the declaration, the property offset, `MakeLike` and
/// the `Create` seed), so the mode round-trips through `?`/dump and nothing
/// else — see this module's header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VscMode {
    /// `VSC_FIXED = 0` — fixed modulation `M0`/`d0` (the only modeled mode).
    Fixed = 0,
    /// `VSC_PACVAC = 1`.
    PacVac = 1,
    /// `VSC_PACQAC = 2`.
    PacQac = 2,
    /// `VSC_VDCVAC = 3`.
    VdcVac = 3,
    /// `VSC_VDCQAC = 4`.
    VdcQac = 4,
}

impl VscMode {
    /// The `VSConverter: Control Mode` `DssEnum` ordinal (the `VSCMode=` value).
    pub fn ordinal(self) -> i32 {
        self as i32
    }

    /// From a property/registry ordinal; `None` outside 0..=4.
    pub fn from_ordinal(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::Fixed),
            1 => Some(Self::PacVac),
            2 => Some(Self::PacQac),
            3 => Some(Self::VdcVac),
            4 => Some(Self::VdcQac),
            _ => None,
        }
    }
}

/// 1-based property ordinals (Pascal `TVSConverterProp` + the PC/CktElement tails).
pub mod prop {
    pub const PHASES: usize = 1;
    pub const BUS1: usize = 2;
    pub const KVAC: usize = 3;
    pub const KVDC: usize = 4;
    pub const KW: usize = 5;
    pub const NDC: usize = 6;
    pub const RAC: usize = 7;
    pub const XAC: usize = 8;
    pub const M0: usize = 9;
    pub const D0: usize = 10;
    pub const MMIN: usize = 11;
    pub const MMAX: usize = 12;
    pub const IACMAX: usize = 13;
    pub const IDCMAX: usize = 14;
    pub const VACREF: usize = 15;
    pub const PACREF: usize = 16;
    pub const QACREF: usize = 17;
    pub const VDCREF: usize = 18;
    pub const VSCMODE: usize = 19;
    // PCClass tail:
    pub const SPECTRUM: usize = 20;
    // CktElementClass tail:
    pub const BASE_FREQ: usize = 21;
    pub const ENABLED: usize = 22;
    pub const NUM_PROPS: usize = 23; // incl. Like
}

/// `TVSConverter.DefineProperties`.
pub fn class_props(enums: &EnumRegistry) -> ClassProps {
    let defs = vec![
        PropDef::integer("Phases").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::bus("Bus1", 1).flags(PropFlags::REQUIRED),
        PropDef::double("kVAC"),
        PropDef::double("kVDC"),
        PropDef::double("kW"),
        PropDef::integer("NDC"),
        // Rac/Xac: Units_ohm (`VSConverter.pas:197,200`).
        PropDef::double("RAC").flags(PropFlags::UNITS_OHM),
        PropDef::double("XAC").flags(PropFlags::UNITS_OHM),
        PropDef::double("M0"),
        PropDef::double("d0"),
        PropDef::double("MMin"),
        PropDef::double("MMax"),
        PropDef::double("IACMax"),
        PropDef::double("IDCMax"),
        PropDef::double("VACRef"),
        PropDef::double("PACRef"),
        PropDef::double("QACRef"),
        PropDef::double("VDCRef"),
        PropDef::mapped_string_enum("VSCMode", enums.vsc_mode),
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
    ClassProps::new("VSConverter", defs, true)
}

/// `TVSConverterObj`.
#[derive(Debug, Clone)]
pub struct VsConverter {
    pub cd: CktElementData,

    pub f_kvac: f64,
    pub f_kvdc: f64,
    pub f_kw: f64,
    pub fm: f64,
    pub fd: f64,
    pub f_rac: f64,
    pub f_xac: f64,
    pub f_ref_vac: f64,
    pub f_ref_vdc: f64,
    pub f_ref_pac: f64,
    pub f_ref_qac: f64,
    pub f_min_m: f64,
    pub f_max_m: f64,
    pub f_max_iac: f64,
    pub f_max_idc: f64,
    pub f_mode: VscMode,
    pub ndc: usize,

    /// `LastCurrents` — the terminal currents saved by `GetCurrents` (Pascal keeps
    /// this as state memory; the `Get_Variable`/reporting paths read it).
    pub last_currents: Vec<Complex64>,

    pub spectrum: String,
    pub spectrum_obj: Option<SpectrumObj>,
}

impl VsConverter {
    /// Pascal `TVSConverterObj.Create`.
    pub fn new(name: &str) -> Self {
        let mut cd = CktElementData::new(name, prop::NUM_PROPS);
        // Typically the first 3 "phases" are AC and the last one is DC.
        cd.nphases = 4;
        cd.nconds = 4;
        cd.set_nterms(2); // two-terminal device, like the voltage source

        let mut o = Self {
            cd,
            f_kvac: 1.0,
            f_kvdc: 1.0,
            f_kw: 1.0,
            fm: 0.5,
            fd: 0.0,
            f_rac: 0.0,
            f_xac: 0.0,
            f_ref_vac: 0.0,
            f_ref_vdc: 0.0,
            f_ref_pac: 0.0,
            f_ref_qac: 0.0,
            f_min_m: 0.1,
            f_max_m: 0.9,
            f_max_iac: 2.0,
            f_max_idc: 2.0,
            f_mode: VscMode::Fixed, // VSC_FIXED
            ndc: 1,
            last_currents: Vec::new(),
            spectrum: "default".to_string(),
            spectrum_obj: None,
        };
        o.cd.set_nconds(4); // sync yorder = nconds * nterms
        o.recalc();
        o
    }

    /// Pascal `TVSConverterObj.RecalcElementData`.
    pub(super) fn recalc(&mut self) {
        if self.f_rac == 0.0 && self.f_xac == 0.0 {
            self.f_rac = EPSILON;
        }
        self.cd.inj_current = vec![Complex64::ZERO; self.cd.yorder];
        self.last_currents = vec![Complex64::ZERO; self.cd.yorder];
    }

    /// Pascal `TVSConverterObj.GetInjCurrents` (l.418) — the solve path: fill
    /// `self.cd.inj_current` via [`Self::compute_inj_currents`].
    pub(super) fn get_inj_currents(&mut self, node_v: &[Complex64]) {
        self.cd.inj_current = self.compute_inj_currents(node_v);
    }

    /// Pascal `TVSConverterObj.GetInjCurrents` (l.418) — compute the AC
    /// voltage-source injection (`YPrim·[Vsource; 0]`) plus the DC power-balance
    /// current source, **returning** the vector; `self.cd.inj_current` is left
    /// untouched so the reporting path stays side-effect-free (Pascal's
    /// `GetCurrents` writes into the scratch `ComplexBuffer`, never `InjCurrent`).
    /// `node_v` supplies the terminal voltages.
    ///
    /// Order matters (and sets the converged iteration count): Pascal computes the
    /// terminal current `ITerminal = YPrim·VTerminal − InjCurrent` via
    /// `TPCElement.GetTerminalCurrents` using the **previous** solve's
    /// `InjCurrent`, **before** the new injection replaces it — a one-iteration
    /// lag the `Pac` power estimate carries. Replicated here.
    #[allow(clippy::needless_range_loop)] // loop-for-loop Pascal port
    pub(super) fn compute_inj_currents(&mut self, node_v: &[Complex64]) -> Vec<Complex64> {
        let nphases = self.cd.nphases;
        let yorder = self.cd.yorder;
        let nac = nphases - self.ndc;
        let idclim = self.f_max_idc * self.f_kw / self.f_kvdc;

        // ComputeVTerminal.
        for i in 0..yorder {
            self.cd.vterminal[i] = node_v[self.cd.node_ref[i]];
        }

        // Vdc = Vterminal[FNphases] (the DC conductor of terminal 1).
        let mut vdc = self.cd.vterminal[nphases - 1];
        if vdc.re == 0.0 && vdc.im == 0.0 {
            vdc.re = 1000.0 * self.f_kvdc;
        }

        // The AC voltage-source phasors → complex buffer (terminal 2 stays 0).
        let mut cbuf = vec![Complex64::ZERO; yorder];
        let mut vmag = vdc * (0.353553 * self.fm);
        vmag = rotate_phasor_deg(vmag, 1.0, self.fd);
        cbuf[0] = vmag;
        let deg = -360.0 / nac as f64;
        for slot in cbuf.iter_mut().take(nac).skip(1) {
            vmag = rotate_phasor_deg(vmag, 1.0, deg);
            *slot = vmag;
        }
        cbuf[nphases - 1] = Complex64::ZERO; // ComplexBuffer[FNPhases] := 0

        let yprim = self
            .cd
            .yprim
            .as_ref()
            .expect("calc_yprim runs before inj_currents");
        // ITerminal = YPrim·VTerminal − InjCurrent, using the *previous* InjCurrent
        // (Pascal `GetTerminalCurrents`, ITerminalUpdated := FALSE branch).
        let mut iterm = vec![Complex64::ZERO; yorder];
        yprim.mv_mult(&mut iterm, &self.cd.vterminal);
        for i in 0..yorder {
            iterm[i] -= self.cd.inj_current[i];
        }
        // Curr = YPrim · ComplexBuffer (the new AC source injection).
        let mut inj = vec![Complex64::ZERO; yorder];
        yprim.mv_mult(&mut inj, &cbuf);

        // Pac = Σ ComplexBuffer[i] · conj(ITerminal[i]) over the AC conductors.
        let mut pac = 0.0;
        for i in 0..nac {
            pac += (cbuf[i] * iterm[i].conj()).re;
        }
        if pac == 0.0 {
            pac = 1000.0 * self.f_kw;
        }

        // DC current-source injection (power balance, clamped).
        let mut idc = pac / vdc.norm();
        if idc > idclim {
            idc = idclim;
        }
        if idc < -idclim {
            idc = -idclim;
        }
        inj[nphases - 1] = Complex64::new(idc, 0.0);
        inj[2 * nphases - 1] = Complex64::new(-idc, 0.0);
        inj
    }
}

mod accessors;

#[cfg(test)]
mod tests {
    use super::{VsConverter, VscMode};
    use crate::obj::dss_enum::EnumRegistry;

    /// `VSC_FIXED = 0` … `VSC_VDCQAC = 4` (`VSConverter.pas:136-140`), the
    /// `'VSConverter: Control Mode'` value list (`:152-154`), its
    /// `DefaultValue = VSC_FIXED` (`:155`) and the `Create` seed (`:312`).
    #[test]
    fn vsc_mode_pins_pascal_ordinals() {
        for (m, ord) in [
            (VscMode::Fixed, 0),
            (VscMode::PacVac, 1),
            (VscMode::PacQac, 2),
            (VscMode::VdcVac, 3),
            (VscMode::VdcQac, 4),
        ] {
            assert_eq!(m.ordinal(), ord);
            assert_eq!(VscMode::from_ordinal(ord), Some(m));
        }
        for v in [i32::MIN, -1, 5, 100, i32::MAX] {
            assert_eq!(VscMode::from_ordinal(v), None, "ordinal {v}");
        }
        assert_eq!(VsConverter::new("v").f_mode, VscMode::Fixed);

        // The live registry's default is the same variant (an unmatched token
        // lands there instead of raising, unlike Scan/Sequence Type).
        let reg = EnumRegistry::new();
        assert_eq!(
            VscMode::from_ordinal(reg.get(reg.vsc_mode).default_value),
            Some(VscMode::Fixed)
        );
    }
}
