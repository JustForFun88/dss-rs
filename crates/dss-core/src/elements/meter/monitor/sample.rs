//! Pascal `TMonitorObj.TakeSample` and the polar/residual conversion helpers.

use num_complex::Complex64;

use super::{Monitor, MonitorBaseMode, MonitorSampleCtx};
use crate::elements::traits::{MeteredElem, SysCtx};
use crate::support::complexutil::cdang;
use crate::support::mathutil::SymComp;

impl Monitor {
    /// Pascal `TMonitorObj.AddDblToBuffer` (`Meters/Monitor.pas:1591`): narrow
    /// the value to single precision and append it, tracking the live scratch
    /// cursor `BufPtr`. The 1024-single flush check is per single, so it can (and
    /// does) fire mid-record. In the merged MonBuffer+MonitorStream model the
    /// flush moves no data — the single already lives in `mon_buffer` — so only
    /// `bufptr` resets (Pascal `Save` sets `BufPtr := 0`, Monitor.pas:1596-1599).
    /// Accepted divergence (audit Question, 2026-07-09): Pascal's mid-solve
    /// flush calls the full `Save`, so `MonitorStream` becomes non-empty at
    /// that instant; here `flushed_records` stays 0 until an explicit `save()`.
    /// Observable only by reading a monitor mid-solve after ≥1024 singles and
    /// before the loop-end `SaveAll` — unreachable from script (every
    /// multi-step solve loop ends with `SaveAll`); all post-solve outputs are
    /// identical.
    pub(super) fn add_dbl(&mut self, v: f64) {
        if self.bufptr == super::BUFFER_SIZE {
            self.bufptr = 0;
        }
        self.bufptr += 1;
        self.mon_buffer.push(v as f32);
    }
    fn add_dbls(&mut self, vs: &[f64]) {
        for &v in vs {
            self.add_dbl(v);
        }
    }

    /// Pascal `TMonitorObj.TakeSample`: append one record for the present
    /// solution. `metered` is the monitored circuit element (the source for a
    /// mode-5 monitor, which does not read it).
    pub fn take_sample(
        &mut self,
        metered: &mut MeteredElem<'_>,
        node_v: &[Complex64],
        sys: &SysCtx,
        sol: &MonitorSampleCtx,
    ) {
        if !(self.valid_monitor && self.med.cd.enabled) {
            return;
        }
        self.sample_count += 1;
        self.hour = sol.int_hour;
        self.sec = sol.t;

        // Metered element dimensions (immutable peek; borrow ends here).
        let (m_nconds, m_yorder) = {
            let e = metered.ckt();
            (e.cd().nconds, e.cd().yorder)
        };
        let offset = (self.med.metered_terminal as usize - 1) * m_nconds;
        let fnphases = self.med.cd.nphases;
        let fnconds = self.med.cd.nconds;

        // Time stamp (frequency/harmonic in harmonic mode).
        if sol.is_harmonic {
            self.add_dbl(sol.frequency);
            self.add_dbl(sol.harmonic);
        } else {
            self.add_dbl(self.hour as f64);
            self.add_dbl(self.sec);
        }

        let base = self.mode.base;

        // Scratch buffers (Pascal keeps them as fields for reuse).
        let mut current_buffer = vec![Complex64::ZERO; m_yorder.max(fnconds + 1)];
        let mut voltage_buffer = vec![Complex64::ZERO; m_yorder.max(fnconds + 1)];

        match base {
            MonitorBaseMode::VoltageAndCurrent | MonitorBaseMode::Power => {
                let e = metered.ckt_mut();
                e.compute_iterminal(sys, node_v);
                let cd = e.cd();
                current_buffer[..m_yorder].copy_from_slice(&cd.iterminal[..m_yorder]);
                for (i, v) in voltage_buffer.iter_mut().enumerate().take(fnconds) {
                    *v = node_v[self.med.cd.node_ref[i]];
                }
            }
            MonitorBaseMode::Tap => {
                let w = self.med.metered_terminal as usize;
                let tap = metered.ckt().present_tap(w).unwrap_or(0.0);
                self.add_dbl(tap);
                return;
            }
            MonitorBaseMode::StateVars => {
                // Pascal `TakeSample` mode 3 (Monitor.pas l.1245-1250): pick up
                // the metered PC element's state variables —
                // GetAllVariables(StateBuffer) then AddDblsToBuffer(StateBuffer,
                // Length(StateBuffer)). `record_size` was set to NumVariables at
                // header build (`ClearMonitorStream`).
                let n = self.record_size;
                let mut states = vec![0.0_f64; n];
                let e = metered.ckt_mut();
                e.get_all_variables(sys, node_v, &mut states);
                self.add_dbls(&states);
                return;
            }
            MonitorBaseMode::SolutionVars => {
                // Solution variables (no metered access).
                let vars = [
                    sol.iteration as f64,
                    sol.control_iteration as f64,
                    sol.max_iterations as f64,
                    sol.max_control_iterations as f64,
                    if sol.converged { 1.0 } else { 0.0 },
                    sol.interval_hrs,
                    sol.solution_count as f64,
                    sol.mode_ordinal as f64,
                    sol.frequency,
                    sol.year as f64,
                    sol.solve_time_us,
                    sol.step_time_us,
                ];
                self.add_dbls(&vars);
                return;
            }
            MonitorBaseMode::CapacitorSteps => {
                if let Some(cap) = metered.capacitor() {
                    let states: Vec<f64> = cap.states().iter().map(|&s| s as f64).collect();
                    self.add_dbls(&states);
                }
                return;
            }
            MonitorBaseMode::Losses => {
                let e = metered.ckt_mut();
                let losses = e.losses(sys, node_v);
                self.add_dbl(losses.re);
                self.add_dbl(losses.im);
                return;
            }
            MonitorBaseMode::AllTerminalVI => {
                let e = metered.ckt_mut();
                e.cd_mut().compute_vterminal(node_v);
                e.compute_iterminal(sys, node_v);
                let cd = e.cd();
                voltage_buffer[..m_yorder].copy_from_slice(&cd.vterminal[..m_yorder]);
                current_buffer[..m_yorder].copy_from_slice(&cd.iterminal[..m_yorder]);
                convert_to_polar(&mut voltage_buffer, m_yorder);
                for &c in &voltage_buffer[..m_yorder] {
                    self.add_dbl(c.re);
                    self.add_dbl(c.im);
                }
                convert_to_polar(&mut current_buffer, m_yorder);
                for &c in &current_buffer[..m_yorder] {
                    self.add_dbl(c.re);
                    self.add_dbl(c.im);
                }
                return;
            }
            MonitorBaseMode::Storage => {
                // Pascal `TakeSample` mode 7 (Monitor.pas l.1298): Storage device
                // state — PresentkW, Presentkvar, kWhStored, %stored, StorageState,
                // guarded on the element class exactly like Pascal (a non-Storage
                // element records the time stamp only). The header promised
                // `record_size = 5` (header.rs), so before this arm existed a
                // yearly run panicked in `to_csv` (buffer rows of 2 vs stride 7 —
                // the StoCtrl_Current_PeakShave corpus deck).
                if let Some(st) = metered.storage() {
                    self.add_dbl(st.present_kw());
                    self.add_dbl(st.present_kvar());
                    self.add_dbl(st.kwh_stored);
                    self.add_dbl(st.kwh_stored / st.kwh_rating * 100.0);
                    self.add_dbl(st.f_state.ordinal() as f64);
                }
                return;
            }
            MonitorBaseMode::Flicker => {
                // Pascal `TakeSample` mode 4 (Monitor.pas l.1252-1263, 1479,
                // 1560-1562): RMS phase voltages for flicker. Fill
                // `FlickerBuffer[i] := NodeV[NodeRef[i]]` for the metered phases,
                // convert to polar (mag, angle_deg) exactly as
                // `ConvertComplexArrayToPolar`, and store `2·Fnphases` doubles
                // (mag, ang) — narrowed to f32 by `add_dbl` like every mode. The
                // flicker/Pst post-processing happens later in `post_process`
                // (Pascal `DoFlickerCalculations`), not here.
                let mut flicker_buffer = vec![Complex64::ZERO; fnphases];
                for (i, v) in flicker_buffer.iter_mut().enumerate() {
                    // Same unguarded NodeRef read as mode 0 above (the port
                    // resolves NodeRef at bus-def time; Pascal's l.1261 "NodeRef
                    // is invalid / solve a snapshot first" except-guard is the
                    // not-yet-solved safety net, handled upstream in the port).
                    *v = node_v[self.med.cd.node_ref[i]];
                }
                convert_to_polar(&mut flicker_buffer, fnphases);
                for &c in &flicker_buffer {
                    self.add_dbl(c.re); // magnitude
                    self.add_dbl(c.im); // angle (deg)
                }
                return;
            }
            MonitorBaseMode::TransformerWindingCurrents => {
                // Pascal `TakeSample` mode 8 (Monitor.pas l.1311-1327): all
                // winding currents of a transformer. `GetAllWindingCurrents`
                // returns `2·Nphases·NumWindings` complex currents (both ends of
                // each winding); Pascal reloads `Vterminal` from `NodeV` inside
                // that routine, but the `&self` Rust getter reads the
                // caller-populated `cd.vterminal`, so refresh it first (as mode 11
                // does). Convert the whole buffer to polar, then store every other
                // entry's (mag, angle) — the magnitude is identical at each end of
                // a winding (`k := 1; k += 2`).
                {
                    let e = metered.ckt_mut();
                    e.cd_mut().compute_vterminal(node_v);
                }
                let tr = metered
                    .transformer()
                    .expect("mode 8 monitor validated as a transformer in recalc");
                let mut wdg = tr.get_all_winding_currents();
                let n = wdg.len();
                convert_to_polar(&mut wdg, n);
                let mut k = 0usize;
                while k < n {
                    self.add_dbl(wdg[k].re); // magnitude
                    self.add_dbl(wdg[k].im); // angle (deg)
                    k += 2;
                }
                return;
            }
            MonitorBaseMode::TransformerWindingVoltages => {
                // Pascal `TakeSample` mode 10 (Monitor.pas l.1337-1351): all
                // winding voltages. For each winding `i`, `GetWindingVoltages`
                // fills `PhsVoltagesBuffer[1..Nphases]`; they are scattered into
                // `WdgVoltagesBuffer[i + (j-1)·NumWindings]` (winding-minor within
                // each phase — matching the `P{phase}W{winding}` header order).
                // Convert `NumWindingVoltages = NumWindings·Nphases` entries to
                // polar and store all as (mag, angle) pairs.
                let np = fnphases;
                let tr = metered
                    .transformer_mut()
                    .expect("mode 10 monitor validated as a transformer in recalc");
                let nw = tr.num_windings().max(0) as usize;
                let num_wdg_volts = nw * np;
                let mut wdg_v = vec![Complex64::ZERO; num_wdg_volts];
                let mut phs = vec![Complex64::ZERO; np];
                for i in 1..=nw {
                    tr.get_winding_voltages(i, node_v, &mut phs);
                    for (j, &pv) in phs.iter().enumerate() {
                        // Pascal 1-based `WdgVoltagesBuffer[i + (j-1)*NumWindings]`.
                        wdg_v[(i - 1) + j * nw] = pv;
                    }
                }
                convert_to_polar(&mut wdg_v, num_wdg_volts);
                for &c in &wdg_v {
                    self.add_dbl(c.re); // magnitude
                    self.add_dbl(c.im); // angle (deg)
                }
                return;
            }
            MonitorBaseMode::LineToLineVoltages => {
                // Pascal `TakeSample` mode 12 (Monitor.pas l.1374-1416): all
                // terminal line-to-line voltages and terminal currents of any
                // device. Per terminal `k`, extract that terminal's phase voltages
                // into `VoltageBuffer[1..NPhases]`, plant `VoltageBuffer[1]` at the
                // "reference" slot (`NPhases+1` when `NPhases = NConds`, else
                // `NConds`) so the last phase wraps, form the LL differences
                // `V[i] -= V[i+1]`, convert to polar and store `2·NPhases` doubles.
                // Then the full terminal currents (`2·MeteredElement.Yorder`,
                // like mode 11).
                //
                // NOT reproduced — upstream UB (see
                // `investigations/monitor_mode12_terminal_currents_ub.md`): Pascal
                // fills the current buffer with `for i := 1 to Yorder` using the
                // *monitor's own* `Yorder` (= `Nconds`, since the monitor always
                // has `NTerms = 1`, `Monitor.pas:458`) but then
                // `ConvertComplexArrayToPolar`/`AddDblsToBuffer` run over
                // `MeteredElement.Yorder` (`Monitor.pas:1407,1410,1413`). For any
                // element with `NTerms > 1` the terminal-2..N currents are read
                // from uninitialized `CurrentBuffer` heap — proven
                // nondeterministic (byte-identical solve, terminal-2 currents
                // change with unrelated prior monitor allocations). Per CLAUDE.md
                // (UB is documented and gated around, never reproduced) the port
                // fills all `MeteredElement.Yorder` currents correctly. Mode 12 on
                // a single-terminal element (Load/Generator/Capacitor) has no UB
                // region (`monitor Yorder == MeteredElement.Yorder`) and matches
                // the oracle exactly — that is what the goldens pin.
                let e = metered.ckt_mut();
                e.cd_mut().compute_vterminal(node_v);
                let (np, nc, yorder, nterms) = {
                    let cd = e.cd();
                    (cd.nphases, cd.nconds, cd.yorder, cd.nterms)
                };
                let vterminal = e.cd().vterminal.clone();
                // Pascal 1-based `myRefIdx`; 0-based here.
                let my_ref0 = if np == nc { np } else { nc - 1 };
                for k in 0..nterms {
                    let buff0 = np * k; // 0-based start of terminal k's phases
                    voltage_buffer[..np].copy_from_slice(&vterminal[buff0..buff0 + np]);
                    // Bring the first phase to the reference slot for the wrap.
                    voltage_buffer[my_ref0] = voltage_buffer[0];
                    for p in 0..np {
                        let next = voltage_buffer[p + 1];
                        voltage_buffer[p] -= next;
                    }
                    convert_to_polar(&mut voltage_buffer, yorder);
                    for &c in &voltage_buffer[..np] {
                        self.add_dbl(c.re); // magnitude
                        self.add_dbl(c.im); // angle (deg)
                    }
                }
                e.compute_iterminal(sys, node_v);
                current_buffer[..yorder].copy_from_slice(&e.cd().iterminal[..yorder]);
                convert_to_polar(&mut current_buffer, yorder);
                for &c in &current_buffer[..yorder] {
                    self.add_dbl(c.re); // magnitude
                    self.add_dbl(c.im); // angle (deg)
                }
                return;
            }
            // `Undefined` (base 13/14/15) lands here: Pascal's `TakeSample`
            // `else Exit` writes a timestamp-only row for those ordinals.
            MonitorBaseMode::Undefined => return,
        }

        // --- Common tail for modes 0 and 1 --------------------------------
        let is_sequence = self.mode.sequence && fnphases == 3;
        let num_vi = if is_sequence {
            let sc = SymComp::default();
            let mut v012 = [Complex64::ZERO; 3];
            let mut i012 = [Complex64::ZERO; 3];
            sc.phase_to_sym(&voltage_buffer[0..3], &mut v012);
            let mut iph = [Complex64::ZERO; 3];
            iph.copy_from_slice(&current_buffer[offset..offset + 3]);
            sc.phase_to_sym(&iph, &mut i012);
            voltage_buffer[0..3].copy_from_slice(&v012);
            current_buffer[offset..offset + 3].copy_from_slice(&i012);
            3
        } else {
            fnconds
        };

        // Residual (mode 0 only) and per-mode conversion.
        let mut residual_volt = Complex64::ZERO;
        let mut residual_curr = Complex64::ZERO;
        let is_power = base == MonitorBaseMode::Power;
        if base == MonitorBaseMode::VoltageAndCurrent {
            if self.include_residual {
                if self.vi_polar {
                    residual_volt = residual_polar(&voltage_buffer[0..fnphases]);
                    residual_curr = residual_polar(&current_buffer[offset..offset + fnphases]);
                } else {
                    residual_volt = residual_sum(&voltage_buffer[0..fnphases]);
                    residual_curr = residual_sum(&current_buffer[offset..offset + fnphases]);
                }
            }
            if self.vi_polar {
                convert_to_polar(&mut voltage_buffer, num_vi);
                convert_to_polar_offset(&mut current_buffer, offset, num_vi);
            }
        } else {
            // Mode 1: VoltageBuffer := kW/kvar = V·conj(I)·0.001 (dest aliases V).
            for j in 0..num_vi {
                let p = voltage_buffer[j] * current_buffer[offset + j].conj() * 0.001;
                voltage_buffer[j] = p;
            }
            if is_sequence || sys.positive_sequence {
                for v in voltage_buffer.iter_mut().take(num_vi) {
                    *v *= 3.0;
                }
            }
            if self.pp_polar {
                convert_to_polar(&mut voltage_buffer, num_vi);
            }
        }

        // --- Write to disk (the MAGNITUDE/POSSEQ modifier paths) ----------
        match (self.mode.magnitude, self.mode.posseq_only) {
            (true, false) => {
                for &c in &voltage_buffer[..num_vi] {
                    self.add_dbl(c.re);
                }
                if self.include_residual {
                    self.add_dbl(residual_volt.re);
                }
                if !is_power {
                    for &c in &current_buffer[offset..offset + num_vi] {
                        self.add_dbl(c.re);
                    }
                    if self.include_residual {
                        self.add_dbl(residual_curr.re);
                    }
                }
            }
            (false, true) => {
                if is_sequence {
                    self.add_dbl(voltage_buffer[1].re);
                    self.add_dbl(voltage_buffer[1].im);
                    if !is_power {
                        self.add_dbl(current_buffer[offset + 1].re);
                        self.add_dbl(current_buffer[offset + 1].im);
                    }
                } else if is_power {
                    let sum: Complex64 = voltage_buffer[0..fnphases].iter().sum();
                    self.add_dbl(sum.re);
                    self.add_dbl(sum.im);
                } else {
                    let mut sum: Complex64 = voltage_buffer[0..fnphases].iter().sum();
                    sum.re /= fnphases as f64;
                    self.add_dbl(sum.re);
                    self.add_dbl(sum.im);
                    let mut sum2: Complex64 =
                        current_buffer[offset..offset + fnphases].iter().sum();
                    sum2.re /= fnphases as f64;
                    self.add_dbl(sum2.re);
                    self.add_dbl(sum2.im);
                }
            }
            (true, true) => {
                if is_sequence {
                    self.add_dbl(voltage_buffer[1].re);
                    if !is_power {
                        self.add_dbl(current_buffer[offset + 1].re);
                    }
                } else {
                    let mut dsum: f64 = voltage_buffer[0..fnphases].iter().map(|c| c.re).sum();
                    if !is_power {
                        dsum /= fnphases as f64;
                    }
                    self.add_dbl(dsum);
                    if !is_power {
                        let dsum2: f64 = current_buffer[offset..offset + fnphases]
                            .iter()
                            .map(|c| c.re)
                            .sum::<f64>()
                            / fnphases as f64;
                        self.add_dbl(dsum2);
                    }
                }
            }
            (false, false) => {
                // V and I in mag/angle or complex kW/kvar.
                for &c in &voltage_buffer[..num_vi] {
                    self.add_dbl(c.re);
                    self.add_dbl(c.im);
                }
                if !is_power {
                    if self.include_residual {
                        self.add_dbl(residual_volt.re);
                        self.add_dbl(residual_volt.im);
                    }
                    for &c in &current_buffer[offset..offset + num_vi] {
                        self.add_dbl(c.re);
                        self.add_dbl(c.im);
                    }
                    if self.include_residual {
                        self.add_dbl(residual_curr.re);
                        self.add_dbl(residual_curr.im);
                    }
                }
            }
        }
    }
}

/// Pascal `ConvertComplexArrayToPolar`: each element → `(mag, angle_deg)`.
fn convert_to_polar(buf: &mut [Complex64], n: usize) {
    for c in buf.iter_mut().take(n) {
        let m = c.norm();
        let a = cdang(*c);
        *c = Complex64::new(m, a);
    }
}

fn convert_to_polar_offset(buf: &mut [Complex64], offset: usize, n: usize) {
    for c in buf.iter_mut().skip(offset).take(n) {
        let m = c.norm();
        let a = cdang(*c);
        *c = Complex64::new(m, a);
    }
}

/// Pascal `Residual`: sum of the first `nph` phasors.
fn residual_sum(buf: &[Complex64]) -> Complex64 {
    buf.iter().sum()
}

/// Pascal `ResidualPolar`: magnitude + angle (deg) of the residual.
fn residual_polar(buf: &[Complex64]) -> Complex64 {
    let x = residual_sum(buf);
    Complex64::new(x.norm(), cdang(x))
}
