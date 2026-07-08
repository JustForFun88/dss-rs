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
}
