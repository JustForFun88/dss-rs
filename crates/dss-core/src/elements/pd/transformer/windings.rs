//! Winding/tap queries and the structural machinery: winding reallocation,
//! the `TermRef` conductor map, `RotatePhases`, the winding-voltage/power
//! readouts RegControl drives, and `FetchXfmrCode`.

use num_complex::Complex64;

use crate::elements::general::xfmr_code::XfmrCodeObj;
use crate::elements::pd::winding::Winding;
use crate::elements::traits::{CktElement, SysCtx};
use crate::support::cmatrix::CMatrix;

use super::{Transformer, prop, xsc_size};

impl Transformer {
    /// Active winding as a 0-based index, clamped into range.
    pub(super) fn aw(&self) -> usize {
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

    /// Pascal `Get_BasekVLL(i)` = `Winding[i].kVLL` (`Transformer.pas:1819`) —
    /// also the raw `Winding[i].kvll` the CIM `PowerTransformerEnd.ratedU`
    /// reads. 1-based winding; 0 out of range. (CIM export, WPG.18 Stage E.)
    pub fn winding_kvll(&self, i: usize) -> f64 {
        if i >= 1 && i <= self.num_windings.max(0) as usize {
            self.windings[i - 1].kvll
        } else {
            0.0
        }
    }

    /// Pascal `Get_WdgkVA(i)` = `Winding[i].kVA` (`Transformer.pas:1438`).
    pub fn wdg_kva(&self, i: usize) -> f64 {
        if i >= 1 && i <= self.num_windings.max(0) as usize {
            self.windings[i - 1].kva
        } else {
            0.0
        }
    }

    /// Pascal `Get_WdgResistance(i)` = `Winding[i].Rpu` (`Transformer.pas:1430`).
    pub fn wdg_resistance(&self, i: usize) -> f64 {
        if i >= 1 && i <= self.num_windings.max(0) as usize {
            self.windings[i - 1].rpu
        } else {
            0.0
        }
    }

    /// Pascal `Get_WdgRneutral(i)` = `Winding[i].Rneut` (`Transformer.pas:1446`).
    pub fn winding_rneut(&self, i: usize) -> f64 {
        if i >= 1 && i <= self.num_windings.max(0) as usize {
            self.windings[i - 1].rneut
        } else {
            0.0
        }
    }

    /// Pascal `Get_WdgXneutral(i)` = `Winding[i].Xneut` (`Transformer.pas:1454`).
    pub fn winding_xneut(&self, i: usize) -> f64 {
        if i >= 1 && i <= self.num_windings.max(0) as usize {
            self.windings[i - 1].xneut
        } else {
            0.0
        }
    }

    /// `Winding[i].NumTaps` — the tap-changer step count (CIM
    /// `ShortCircuitTest.energisedEndStep`, WPG.18 Stage E). 1-based; 0 out of
    /// range.
    pub fn winding_num_taps(&self, i: usize) -> i32 {
        if i >= 1 && i <= self.num_windings.max(0) as usize {
            self.windings[i - 1].num_taps
        } else {
            0
        }
    }

    /// Pascal `Get_Xsc(i)` = `XSC[i]` for 1-based `i` in
    /// `1..=(NumWindings-1)·NumWindings/2` (`Transformer.pas:1462`); 0 otherwise.
    pub fn xsc_val(&self, seq: usize) -> f64 {
        let imax = xsc_size(self.num_windings);
        if seq >= 1 && seq <= imax {
            self.xsc[seq - 1]
        } else {
            0.0
        }
    }

    /// The whole `Winding` array (`TTransfObj.Winding`) — the CIM `WriteXfmrCode`
    /// case-3 synthesis reads a transformer's winding web directly (Pascal
    /// `PullFromTransformer`, `XfmrCode.pas:582`).
    pub fn windings(&self) -> &[Winding] {
        &self.windings
    }

    /// The whole `XSC` array (`TTransfObj.XSC`) — CIM case-3 synthesis.
    pub fn xsc(&self) -> &[f64] {
        &self.xsc
    }

    /// Pascal `pctNoLoadLoss` (`%NoLoadLoss`) — CIM `TransformerCoreAdmittance.g`.
    pub fn pct_no_load_loss(&self) -> f64 {
        self.pct_no_load_loss
    }

    /// Pascal `pctImag` (`%IMag`) — CIM `TransformerCoreAdmittance.b`.
    pub fn pct_imag(&self) -> f64 {
        self.pct_imag
    }

    /// Pascal `NormMaxHKVA` — CIM `NoLoadTest`/`TransformerEndInfo` ratings.
    pub fn norm_max_hkva(&self) -> f64 {
        self.norm_max_hkva
    }

    /// Pascal `EmergMaxHKVA`.
    pub fn emerg_max_hkva(&self) -> f64 {
        self.emerg_max_hkva
    }

    /// Pascal `XfmrBank` — the bank name the CIM `PowerTransformer` groups by
    /// (`ExportCIMXML.pas:3970`).
    pub fn xfmr_bank(&self) -> &str {
        &self.xfmr_bank
    }

    /// Pascal `XfmrCodeObj` — `Some` iff an `xfmrcode=` resolved (the CIM arm's
    /// case-2 vs case-1/3 discriminant, `ExportCIMXML.pas:3945/3998`).
    pub fn xfmr_code_ref(&self) -> Option<crate::elements::traits::ElemRef> {
        self.xfmr_code_ref
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
    pub(super) fn set_num_windings(&mut self, n: i32) {
        let prev = self.num_windings;
        self.num_windings = n;
        self.realloc_windings(prev);
    }

    /// Pascal `PropertySideEffects(ord(windings), prev)`: reallocate windings,
    /// `XSC` (new slots → 0.30), terminals and the impedance matrices.
    pub(super) fn realloc_windings(&mut self, prev_int: i32) {
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
    pub(super) fn rotate_phases(&self, iphs: usize) -> usize {
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
    pub(super) fn set_term_ref(&mut self) {
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

    /// Pascal `TTransfObj.FetchXfmrCode`: copy the resolved `XfmrCode`'s whole
    /// winding web onto this transformer, then recompute.
    pub(super) fn fetch_xfmr_code(&mut self, code: &XfmrCodeObj) {
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
