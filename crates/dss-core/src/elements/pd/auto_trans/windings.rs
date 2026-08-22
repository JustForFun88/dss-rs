//! Winding structural machinery: the active-winding index, winding reallocation,
//! `RotatePhases` and the `TermRef` conductor map. The RegControl-facing tap
//! surface (`present_tap`/`set_present_tap`/…) and the winding readouts land with
//! the RegControl integration in WPG.15 Stage C.

use crate::elements::general::xfmr_code::XfmrCodeObj;
use crate::elements::pd::winding::{Connection, TermRef};
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
            self.windings[i - 1].connection.ordinal()
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

    /// Pascal `Get_BasekVLL(i)` = `Winding[i].kVLL` (`AutoTrans.pas:1817`) — the
    /// CIM `PowerTransformerEnd.ratedU`/`zbase` source (WPG.18 Stage E). 1-based;
    /// 0 out of range.
    pub fn winding_kvll(&self, i: usize) -> f64 {
        if i >= 1 && i <= self.num_windings.max(0) as usize {
            self.windings[i - 1].kvll
        } else {
            0.0
        }
    }

    /// Pascal `Get_WdgkVA(i)` = `Winding[i].kVA` (`AutoTrans.pas:1464`).
    pub fn wdg_kva(&self, i: usize) -> f64 {
        if i >= 1 && i <= self.num_windings.max(0) as usize {
            self.windings[i - 1].kva
        } else {
            0.0
        }
    }

    /// Pascal `Get_WdgResistance(i)` = `Winding[i].Rpu` (`AutoTrans.pas:1456`).
    pub fn wdg_resistance(&self, i: usize) -> f64 {
        if i >= 1 && i <= self.num_windings.max(0) as usize {
            self.windings[i - 1].rpu
        } else {
            0.0
        }
    }

    /// Pascal `Get_Xsc(i)` = `XSC[i]` for 1-based `i` in
    /// `1..=(NumWindings-1)·NumWindings/2` (`AutoTrans.pas:1472`); 0 otherwise.
    pub fn xsc_val(&self, seq: usize) -> f64 {
        let imax = xsc_size(self.num_windings);
        if seq >= 1 && seq <= imax {
            self.xsc[seq - 1]
        } else {
            0.0
        }
    }

    /// Pascal `pctNoLoadLoss` — CIM `TransformerCoreAdmittance.g`.
    pub fn pct_no_load_loss(&self) -> f64 {
        self.pct_no_load_loss
    }

    /// Pascal `pctImag` — CIM `TransformerCoreAdmittance.b`.
    pub fn pct_imag(&self) -> f64 {
        self.pct_imag
    }

    /// Pascal `XfmrBank` — the CIM bank name (`ExportCIMXML.pas:3813`).
    pub fn xfmr_bank(&self) -> &str {
        &self.xfmr_bank
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
        // Auto's `Get_Power` reads the terminal's own `TermNodeRef` (the auto's
        // `SetNodeRef` magic rewrites terminal 2's, distinct from the flat
        // `NodeRef`) against the terminal's `Iterminal` conductor slice.
        let tref = &self.cd.terminals[term - 1].term_node_ref;
        let mut result = num_complex::Complex64::ZERO;
        for (&n, &ci) in tref.iter().zip(self.cd.term_i(term - 1)) {
            if n > 0 {
                result += node_v[n] * ci.conj();
            }
        }
        if sys.positive_sequence {
            result *= 3.0;
        }
        result
    }

    /// Pascal `TAutoTransObj.FetchXfmrCode` (`AutoTrans.pas:2339-2396`) — copy a
    /// resolved `XfmrCode`'s electrical model onto this autotransformer, *"Uses
    /// standard Xfmrcode, but forces connection of first two windings"* (`:2340`).
    ///
    /// Deliberately **not** the Transformer's
    /// [`fetch_xfmr_code`](crate::elements::pd::transformer) — that one assigns
    /// the winding vector wholesale (`windings.rs:362`) and copies the 0.15.x
    /// kVA-ratings (`:398-399`), and neither is r4133's auto form. Here every
    /// field is copied one at a time, in the Pascal's order, with the auto's
    /// three deviations from the Transformer routine:
    ///
    /// 1. **the connection override** (`:2356-2361`) — winding 1 is forced
    ///    `SERIES` and winding 2 `WYE` whatever the code says ("No Choice for 1st
    ///    two"); only a tertiary and beyond keeps the code's own connection;
    /// 2. **the reactance rename** (`:2374-2376`) — the code's `XHL/XHT/XLT`
    ///    become the auto's `puXHX/puXHT/puXXT`;
    /// 3. **`RdcSpecified := TRUE`** on every winding (`:2367`), so
    ///    [`recalc`](AutoTrans::recalc) derives `Rdcpu` from the copied `RdcOhms`
    ///    instead of the 85 %-of-ac default.
    ///
    /// **The one r4133 statement not reproduced** is `NConds := Fnphases + 1`
    /// (`:2353`) — the *Transformer's* conductor rule (one per phase plus a
    /// neutral), copy-pasted into the auto, whose every other path sets
    /// `2 * Fnphases` (`:538` the `phases=` side effect, `:798` `MakeLike`,
    /// `:891` `Create`) because an autotransformer carries two conductors per
    /// phase — `SetNodeRef` aliases the series winding's second end onto the
    /// common winding's node inside that block. Leaving it at `Fnphases + 1`
    /// truncates `Yorder` (`4·2` instead of `6·2` on a 3-phase, 2-winding unit)
    /// and scrambles the alias, so r4133 solves a different circuit: measured
    /// 2026-08-22 on the epri-worker, one 115/69 kV 3-phase auto reports
    /// `NumConductors = 4` and `MID.1 = 92 236 V` on a 69 kV winding, while the
    /// same deck with a trailing `phases=3` (which re-runs the correct side
    /// effect) reports 6 and 38 472 V. Upstream bugs are never reproduced
    /// (CLAUDE.md), so the port keeps `2 * Fnphases` — via
    /// [`CktElementData::set_nconds`](crate::elements::ckt::CktElementData::set_nconds),
    /// which is what the Pascal statement does structurally (reallocate the
    /// terminals, flag `BusNameRedefined`), just with the auto's count. Report:
    /// `investigations/to_opendss/37-autotrans-fetchxfmrcode-nconds.md`; the
    /// corpus deck `asymmetric:autotrans/autotrans_xfmrcode.dss` is single-phase
    /// precisely because `1 + 1 == 2 · 1` makes the two rules agree there, so the
    /// r4133 channel can gate the rest of this copy.
    pub(super) fn fetch_xfmr_code(&mut self, code: &XfmrCodeObj) {
        // `Nphases := Obj.Fnphases; SetNumWindings(Obj.NumWindings);` (`:2351-2352`)
        // — `SetNumWindings` re-`Init`s the windings, resizes `puXSC`, sets
        // `Nterms := NumWindings` and the impedance matrices.
        self.cd.nphases = code.fnphases().max(0) as usize;
        self.set_num_windings(code.num_windings());
        // `:2353`, with the auto's conductor count (see the doc comment).
        self.cd.set_nconds(2 * self.cd.nphases);

        let nw = self.num_windings.max(0) as usize;
        for (i, w) in self.windings.iter_mut().enumerate().take(nw) {
            let src = &code.windings()[i];
            // `:2356-2361` — "No Choice for 1st two".
            w.connection = match i {
                0 => Connection::Series,
                1 => Connection::Wye,
                _ => src.connection,
            };
            w.kvll = src.kvll;
            w.vbase = src.vbase;
            w.kva = src.kva;
            w.putap = src.putap;
            w.rpu = src.rpu;
            // `:2367` — `RdcOhms := …; RdcSpecified := TRUE;` "This will force
            // calc of Rdcpu" in `RecalcElementData`.
            w.rdcohms = src.rdcohms;
            w.rdc_specified = true;
            w.tap_increment = src.tap_increment;
            w.min_tap = src.min_tap;
            w.max_tap = src.max_tap;
            w.num_taps = src.num_taps;
        }
        self.set_term_ref(); // `:2373`

        // `:2374-2377` — the XHL/XHT/XLT → puXHX/puXHT/puXXT rename, then the
        // whole short-circuit array.
        self.puxhx = code.xhl();
        self.puxht = code.xht();
        self.puxxt = code.xlt();
        let n = xsc_size(self.num_windings);
        self.xsc[..n].copy_from_slice(&code.xsc()[..n]);

        // `:2378-2388` — thermal / loss / rating / anti-float scalars.
        self.thermal_time_const = code.thermal_time_const();
        self.n_thermal = code.n_thermal();
        self.m_thermal = code.m_thermal();
        self.flrise = code.flrise();
        self.hsrise = code.hsrise();
        self.pct_load_loss = code.pct_load_loss();
        self.pct_no_load_loss = code.pct_no_load_loss();
        self.pct_imag = code.pct_imag(); // "Omission corrected 12-14-18"
        self.norm_max_hkva = code.norm_max_hkva();
        self.emerg_max_hkva = code.emerg_max_hkva();
        self.ppm_float_factor = code.ppm_float_factor();

        // `:2389-2393` — Yorder, the YPrim invalidation, the frequency-multiplier
        // reset that forces `CalcY_Terminal` to rebuild, and the recalc.
        self.cd.yorder = self.cd.nconds * self.cd.nterms;
        self.cd.yprim_invalid = true;
        self.y_terminal_freqmult = 0.0;
        self.recalc();
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
        // One 0-based [plus, minus] conductor pair per (phase, winding),
        // phase-major — the same visiting order the flat Pascal array fills.
        let mut pairs: Vec<[usize; 2]> = Vec::with_capacity(np.max(1) * nw);
        if np == 1 {
            for j in 0..nw {
                // Pascal: c1 = (j-1)*nconds+1, c2 = j*nconds (1-based).
                pairs.push([j * nconds, (j + 1) * nconds - 1]);
            }
        } else {
            for i in 0..np {
                for j in 0..nw {
                    let base = j * nconds; // winding j's 0-based conductor base
                    let plus = base + i; // phase conductor i (0-based)
                    let pair = match self.windings[j].connection {
                        // Wye — second conductor is the winding neutral (`plus + np`).
                        Connection::Wye => [plus, plus + np],
                        // Delta — second conductor is the next phase in sequence
                        // (`rotate_phases` speaks the 1-based phase language).
                        Connection::Delta => [plus, base + self.rotate_phases(i + 1) - 1],
                        // Series straddles the H/X terminals: c1 → phase i, c2 →
                        // phase i + Fnphases (both in the shared terminal block).
                        Connection::Series => [i, i + np],
                    };
                    pairs.push(pair);
                }
            }
        }
        self.term_ref = TermRef(pairs);
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
