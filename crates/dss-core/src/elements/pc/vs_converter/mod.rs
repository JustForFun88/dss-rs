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
        PropDef::integer("phases").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::bus("bus1", 1),
        PropDef::double("kVac"),
        PropDef::double("kVdc"),
        PropDef::double("kW"),
        PropDef::integer("Ndc"),
        PropDef::double("Rac"),
        PropDef::double("Xac"),
        PropDef::double("m0"),
        PropDef::double("d0"),
        PropDef::double("Mmin"),
        PropDef::double("Mmax"),
        PropDef::double("Iacmax"),
        PropDef::double("Idcmax"),
        PropDef::double("Vacref"),
        PropDef::double("Pacref"),
        PropDef::double("Qacref"),
        PropDef::double("Vdcref"),
        PropDef::mapped_string_enum("VscMode", enums.vsc_mode),
        // PCClass tail:
        PropDef::object_ref("spectrum"),
        // CktElementClass tail:
        PropDef::double("basefreq").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::enabled("enabled"),
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
    pub f_mode: i32,
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
            f_mode: 0, // VSC_FIXED
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
