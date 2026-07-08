//! Winding structural machinery: the active-winding index, winding reallocation,
//! `RotatePhases` and the `TermRef` conductor map. The RegControl-facing tap
//! surface (`present_tap`/`set_present_tap`/…) and the winding readouts land with
//! the RegControl integration in WPG.15 Stage C.

use crate::support::cmatrix::CMatrix;

use super::{AutoTrans, auto_winding_init, xsc_size};

impl AutoTrans {
    /// Active winding as a 0-based index, clamped into range.
    pub(super) fn aw(&self) -> usize {
        (self.active_winding.clamp(1, self.num_windings.max(1)) - 1) as usize
    }

    /// Number of windings (`= NumberOfWindings = Nterms`).
    pub fn num_windings(&self) -> i32 {
        self.num_windings
    }

    /// Pascal `Get_PresentTap` (1-based winding; 0 out of range) — the RegControl
    /// tap driver.
    pub fn present_tap(&self, i: usize) -> f64 {
        if i >= 1 && i <= self.num_windings.max(0) as usize {
            self.windings[i - 1].putap
        } else {
            0.0
        }
    }

    /// `(PresentTap, MaxTap, MinTap, TapIncrement)` for 1-based winding `i`.
    pub fn winding_tap_data(&self, i: usize) -> (f64, f64, f64, f64) {
        if i >= 1 && i <= self.num_windings.max(0) as usize {
            let w = &self.windings[i - 1];
            (w.putap, w.max_tap, w.min_tap, w.tap_increment)
        } else {
            (0.0, 0.0, 0.0, 0.0)
        }
    }

    /// Pascal `TAutoTransObj.Set_PresentTap` (`AutoTrans.pas:1432`): clamp to
    /// Min/MaxTap and, only on a change, invalidate YPrim and recompute. Returns
    /// whether YPrim was invalidated (RegControl raises `SystemYChanged`).
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
    /// (0 = wye, 1 = delta, 2 = series). RegControl's regulated-bus path.
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
    /// (1-based phase index).
    pub fn rotate_phases_1based(&self, iphs: usize) -> usize {
        self.rotate_phases(iphs)
    }

    /// Pascal `TDSSCktElement.Get_Power(idxTerm)` (watts+vars into terminal
    /// `term`) — RegControl's reverse-power direction check. Sums over the
    /// terminal's conductors using its **`TermNodeRef`** (`Get_Power` reads
    /// `ActiveTerminal^.TermNodeRef`, which the auto's `SetNodeRef` magic
    /// rewrites for terminal 2 — distinct from the flat `NodeRef`), ×3 under
    /// positive sequence.
    pub fn power_into(
        &mut self,
        term: usize,
        node_v: &[num_complex::Complex64],
        sys: &crate::elements::traits::SysCtx,
    ) -> num_complex::Complex64 {
        use crate::elements::traits::CktElement;
        if !self.cd.enabled || self.cd.node_ref.is_empty() {
            return num_complex::Complex64::ZERO;
        }
        self.compute_iterminal(sys, node_v);
        let nconds = self.cd.nconds;
        let k = (term - 1) * nconds;
        let tref = &self.cd.terminals[term - 1].term_node_ref;
        let mut result = num_complex::Complex64::ZERO;
        for (i, &n) in tref.iter().enumerate().take(nconds) {
            if n > 0 {
                result += node_v[n] * self.cd.iterminal[k + i].conj();
            }
        }
        if sys.positive_sequence {
            result *= 3.0;
        }
        result
    }

    /// Pascal `TAutoTransObj.SetNumWindings`.
    pub(super) fn set_num_windings(&mut self, n: i32) {
        let prev = self.num_windings;
        self.num_windings = n;
        self.realloc_windings(prev);
    }

    /// Pascal `PropertySideEffects(ord(windings), prev)` (`AutoTrans.pas:616`):
    /// reallocate windings (each `Init(i)`), `puXSC` (new slots → 0.30),
    /// terminals and the impedance matrices. `FNconds := 2·Fnphases`.
    pub(super) fn realloc_windings(&mut self, prev_int: i32) {
        let old_xsc = xsc_size(prev_int);
        self.max_windings = self.num_windings;
        self.cd.nconds = 2 * self.cd.nphases;
        let nw = self.num_windings.max(0) as usize;
        self.windings = (1..=nw).map(auto_winding_init).collect();
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

    /// Pascal `TAutoTransObj.RotatePhases` (delta connections / line-line).
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

    /// Pascal `TAutoTransObj.SetTermRef` (`AutoTrans.pas:1129`): map each
    /// winding's two conductors to the auto's phase/neutral conductors per the
    /// winding connection. The **Series** arm maps conductor 1 → phase `i` and
    /// conductor 2 → phase `i + Fnphases` (the series winding straddles the H and
    /// X terminals).
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
                            self.term_ref[k] = self.term_ref[k - 1] + np;
                        }
                        1 => {
                            // Delta — second conductor connects to the next phase
                            self.term_ref[k] = (j - 1) * nconds + i;
                            k += 1;
                            self.term_ref[k] = (j - 1) * nconds + self.rotate_phases(i);
                        }
                        2 => {
                            // Series winding for the autotransformer
                            self.term_ref[k] = i;
                            k += 1;
                            self.term_ref[k] = i + np;
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    /// Pascal `TAutoTransObj.SetBus` override (`AutoTrans.pas:721`): for winding
    /// 2, default all the second-end conductors to one ground node so the common
    /// winding's neutral ties together — unless the user gives an explicit
    /// non-zero neutral, in which case every neutral conductor is tied to it. All
    /// other windings pass through to the base. The winding buses carry numeric
    /// node lists, so the node extraction is a self-contained parse (mirroring
    /// `AuxParser.ParseAsBusName`); no corpus deck exercises the explicit-neutral
    /// rewrite branch — the default case is identical to the base (the extra
    /// conductors ground in `process_bus_defs`).
    pub(super) fn set_bus_auto(&mut self, iwdg: usize, s: &str) {
        if iwdg != 2 {
            self.cd.set_bus(iwdg, s);
            return;
        }
        let nphases = self.cd.nphases;
        let nconds = self.cd.nconds;
        // NNodes (1-based, slot 0 unused): phases 1..nphases, neutrals 0.
        let mut nnodes = vec![0i32; nconds + 1];
        for (i, slot) in nnodes.iter_mut().enumerate().take(nphases + 1).skip(1) {
            *slot = i as i32;
        }
        // ParseAsBusName: the bus name is `busname.n1.n2.…`; overwrite the
        // defaults with any explicit node integers.
        let mut parts = s.split('.');
        let bus_name = parts.next().unwrap_or("");
        for (idx, tok) in parts.enumerate() {
            if idx < nconds {
                nnodes[idx + 1] = tok.parse().unwrap_or(0);
            }
        }
        if nnodes[nphases + 1] > 0 {
            // Reconstruct: phases as given, every neutral tied to nnodes[np+1].
            let mut new_name = bus_name.to_string();
            for slot in nnodes.iter().take(nphases + 1).skip(1) {
                new_name.push_str(&format!(".{slot}"));
            }
            for _ in (nphases + 1)..=nconds {
                new_name.push_str(&format!(".{}", nnodes[nphases + 1]));
            }
            self.cd.set_bus(iwdg, &new_name);
        } else {
            self.cd.set_bus(iwdg, s);
        }
    }
}
