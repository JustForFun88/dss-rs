//! Port of `Meters/Monitor.pas` — `TMonitorObj`, the passive recorder that
//! samples a metered element's terminal each time `TakeSample` is invoked and
//! stores the results in an in-memory **float32** buffer that dss-python
//! decodes via `Monitors.Channel(i)`.
//!
//! A monitor has no Yprim (`CalcYPrim` is empty); it joins the device list (so
//! `ProcessBusDefs` allocates its `NodeRef` from `SetBus(1, <metered bus>)`)
//! and the circuit `monitors` list, but never PD/PC. The f32 quantization is
//! compat-critical: every value is computed in f64 and pushed `as f32` exactly
//! where Pascal narrows (`AddDblToBuffer`).
//!
//! Modes ported (PHASE6_PLAN §2.3): 0 (V&I), 1 (powers), 2 (transformer tap),
//! 5 (solution variables), 6 (capacitor steps), 9 (losses), 11 (all terminal
//! V&I) — plus the ±16/±32/±64 modifiers, residual, VIpolar/Ppolar. Modes 3
//! (PCElement state vars — needs the dynamics surface), 4 (flicker/Pstcalc),
//! 7 (Storage), 8/10 (transformer winding currents/voltages) and 12 (LL
//! voltages) build their **header** but defer the sample body (Phase 6+/7); the
//! gate exercises 0/1/2/5. File save/`TranslateToCSV` is Phase 8.

use num_complex::Complex64;

use crate::elements::ckt::CktElementData;
use crate::elements::meter::meter_element::{MeterElementData, MeteredKind, MeteredSnapshot};
use crate::elements::pd::capacitor::Capacitor;
use crate::elements::pd::transformer::Transformer;
use crate::elements::traits::{CktElement, ElemRef, SysCtx};
use crate::obj::base::{DssObjData, DssObject};
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef};
use crate::support::complexutil::cdang;
use crate::support::mathutil::SymComp;

const MODEMASK: i32 = 15;
const SEQUENCEMASK: i32 = 16;
const MAGNITUDEMASK: i32 = 32;
const POSSEQONLYMASK: i32 = 64;
const NUM_SOLUTION_VARS: usize = 12;

/// 1-based property ordinals (`TMonitorProp` + the `TCktElementClass` tail).
pub mod prop {
    pub const ELEMENT: usize = 1;
    pub const TERMINAL: usize = 2;
    pub const MODE: usize = 3;
    pub const ACTION: usize = 4;
    pub const RESIDUAL: usize = 5;
    pub const VIPOLAR: usize = 6;
    pub const PPOLAR: usize = 7;
    // CktElementClass tail:
    pub const BASE_FREQ: usize = 8;
    pub const ENABLED: usize = 9;
    pub const NUM_PROPS: usize = 10; // incl. Like
}

/// `TDSSMonitor.DefineProperties`.
pub fn class_props(enums: &EnumRegistry) -> ClassProps {
    let defs = vec![
        // Pascal `PropertyOffset2 = 0` + `DynamicDefault`: any circuit element
        // by full name (defaults to the first circuit element = the source).
        PropDef::object_ref_any("element"),
        PropDef::integer("terminal"),
        PropDef::integer("mode"),
        PropDef::action("action", enums.monitor_action),
        PropDef::boolean("residual"),
        PropDef::boolean("VIPolar"),
        PropDef::boolean("PPolar"),
        // CktElementClass tail:
        PropDef::double("basefreq").flags(
            crate::obj::props::PropFlags::NON_NEGATIVE | crate::obj::props::PropFlags::NON_ZERO,
        ),
        PropDef::enabled("enabled"),
    ];
    debug_assert_eq!(defs.len(), prop::NUM_PROPS - 1);
    ClassProps::new("Monitor", defs, true)
}

/// `TMonitorObj`.
#[derive(Debug, Clone)]
pub struct Monitor {
    pub med: MeterElementData,
    pub mode: i32,
    include_residual: bool,
    vi_polar: bool,
    pp_polar: bool,

    /// FullName of the metered element (`Class.name`) for the dump.
    element_full_name: String,

    /// In-memory sample buffer (Pascal `MonBuffer` / `MonitorStream` merged: we
    /// never spill to disk, so one growing `Vec<f32>` holds all samples). Each
    /// record is `[hour, sec, ch1..ch_record_size]`.
    mon_buffer: Vec<f32>,
    record_size: usize,
    sample_count: i32,
    header: Vec<String>,
    valid_monitor: bool,
    hour: i32,
    sec: f64,
}

/// Solution scalars the monitor reads at sample time (the `ActiveCircuit.
/// Solution.*` reach, snapshotted). Mode 5 records all of them.
pub struct MonitorSampleCtx {
    pub int_hour: i32,
    pub t: f64,
    pub is_harmonic: bool,
    pub frequency: f64,
    pub harmonic: f64,
    pub iteration: i32,
    pub control_iteration: i32,
    pub max_iterations: i32,
    pub max_control_iterations: i32,
    pub converged: bool,
    pub interval_hrs: f64,
    pub solution_count: i32,
    pub mode_ordinal: i32,
    pub year: i32,
    /// `Solve_Time_Elapsed` / `Step_Time_Elapsed` (µs) — wall-clock timings, not
    /// reproducible, so the port records 0 (the gate skips channels 11/12).
    pub solve_time_us: f64,
    pub step_time_us: f64,
}

impl Monitor {
    /// Pascal `TMonitorObj.Create`.
    pub fn new(name: &str) -> Self {
        let mut med = MeterElementData::new(name, prop::NUM_PROPS);
        med.cd.nphases = 3;
        med.cd.nconds = 3;
        med.cd.set_nterms(1);
        med.metered_terminal = 1;
        Self {
            med,
            mode: 0,
            include_residual: false,
            vi_polar: true,
            pp_polar: true,
            element_full_name: String::new(),
            mon_buffer: Vec::new(),
            record_size: 0,
            sample_count: 0,
            header: Vec::new(),
            valid_monitor: false,
            hour: 0,
            sec: 0.0,
        }
    }

    /// Read-only accessors used by the test/golden harness (dss-python
    /// `Monitors.Header` / `SampleCount` / `Channel(i)` / `dblHour`).
    pub fn header(&self) -> &[String] {
        &self.header
    }
    pub fn sample_count(&self) -> i32 {
        self.sample_count
    }
    pub fn num_channels(&self) -> usize {
        self.record_size
    }
    /// `Channel(i)` (1-based): the i-th recorded value across all samples.
    pub fn channel(&self, i: usize) -> Vec<f32> {
        if i < 1 || i > self.record_size {
            return Vec::new();
        }
        let stride = self.record_size + 2;
        (0..self.sample_count as usize)
            .map(|s| self.mon_buffer[s * stride + 2 + (i - 1)])
            .collect()
    }
    /// `dblHour`: the per-sample hour values (record slot 0).
    pub fn dbl_hour(&self) -> Vec<f64> {
        let stride = self.record_size + 2;
        (0..self.sample_count as usize)
            .map(|s| self.mon_buffer[s * stride] as f64)
            .collect()
    }

    fn add_dbl(&mut self, v: f64) {
        self.mon_buffer.push(v as f32);
    }
    fn add_dbls(&mut self, vs: &[f64]) {
        for &v in vs {
            self.add_dbl(v);
        }
    }

    /// Pascal `ResetIt`: clear the buffer and rebuild the header.
    pub fn reset_it(&mut self) {
        self.mon_buffer.clear();
        self.clear_monitor_stream();
    }

    /// Pascal `RecalcElementData`: validate the metered element against the
    /// mode, copy its phase/conductor counts, set the monitor's bus, and build
    /// the header (`ClearMonitorStream`).
    pub fn recalc(&mut self, errors: &mut Vec<String>) {
        self.valid_monitor = false;
        let Some(snap) = self.med.metered_snap.clone() else {
            errors.push(format!(
                "Monitor: \"{}\": Circuit Element is not set. Element must be defined previously.",
                self.med.cd.obj.name()
            ));
            return;
        };

        // Mode-specific element-class validation (Monitor.pas l.539).
        let class_error = match self.mode & MODEMASK {
            2 | 8 | 10 if snap.kind != MeteredKind::Transformer => {
                Some(format!("{} is not a transformer!", snap.full_name))
            }
            3 if snap.kind != MeteredKind::PcElement => Some(format!(
                "{} must be a power conversion element (Load or Generator)!",
                snap.full_name
            )),
            6 if snap.kind != MeteredKind::Capacitor => {
                Some(format!("{} is not a capacitor!", snap.full_name))
            }
            7 if snap.kind != MeteredKind::Storage => {
                Some(format!("{} is not a storage device!", snap.full_name))
            }
            _ => None,
        };
        if let Some(msg) = class_error {
            errors.push(msg);
            return;
        }

        if self.med.metered_terminal as usize > snap.nterms {
            errors.push(format!(
                "Monitor: \"{}\" Terminal no. \"{}\" does not exist. Respecify terminal no.",
                self.med.cd.obj.name(),
                self.med.metered_terminal
            ));
            return;
        }

        // Monitor adopts the metered element's phase/conductor counts and bus.
        self.med.cd.nphases = snap.nphases;
        self.med.cd.set_nconds(snap.nconds);
        let bus = snap
            .buses
            .get(self.med.metered_terminal as usize - 1)
            .cloned()
            .unwrap_or_default();
        self.med.cd.set_bus(1, &bus);

        self.clear_monitor_stream();
        self.valid_monitor = true;
    }

    /// Pascal `ClearMonitorStream`: reset the buffer header and compute
    /// `RecordSize` + the per-mode header strings (non-harmonic; the harmonic
    /// header is Phase 7).
    fn clear_monitor_stream(&mut self) {
        self.header.clear();
        self.sample_count = 0;
        self.header.push("hour".into());
        self.header.push("t(sec)".into());

        let nphases = self.med.cd.nphases;
        let nconds = self.med.cd.nconds;
        let snap = self.med.metered_snap.clone().unwrap_or_default();
        let mode_mask = self.mode & MODEMASK;

        match mode_mask {
            2 => {
                self.record_size = 1;
                self.header.push("Tap (pu)".into());
            }
            3 => {
                self.record_size = snap.num_variables; // 0 in Phase 6
            }
            4 => {
                self.record_size = 2 * nphases;
                for i in 1..=nphases {
                    self.header.push(format!("Flk{i}"));
                    self.header.push(format!("Pst{i}"));
                }
            }
            5 => {
                self.record_size = NUM_SOLUTION_VARS;
                for s in [
                    "TotalIterations",
                    "ControlIteration",
                    "MaxIterations",
                    "MaxControlIterations",
                    "Converged",
                    "IntervalHrs",
                    "SolutionCount",
                    "Mode",
                    "Frequency",
                    "Year",
                    "SolveSnap_uSecs",
                    "TimeStep_uSecs",
                ] {
                    self.header.push(s.into());
                }
            }
            6 => {
                self.record_size = snap.num_steps;
                for i in 1..=self.record_size {
                    self.header.push(format!("Step_{i}"));
                }
            }
            7 => {
                self.record_size = 5;
                for s in [
                    "kW output",
                    "kvar output",
                    "kW Stored",
                    "%kW Stored",
                    "State",
                ] {
                    self.header.push(s.into());
                }
            }
            8 | 10 => {
                let nw = snap.num_windings;
                self.record_size = 2 * nw * nphases;
                for i in 1..=nphases {
                    for j in 1..=nw {
                        self.header.push(format!("P{i}W{j}"));
                        self.header.push("Deg".into());
                    }
                }
            }
            9 => {
                self.record_size = 2;
                self.header.push("watts".into());
                self.header.push("vars".into());
            }
            11 => {
                let yorder = snap.yorder;
                self.record_size = 2 * 2 * yorder;
                for j in 1..=snap.nterms {
                    for i in 1..=snap.nconds {
                        self.header.push(format!("V{i}T{j}"));
                        self.header.push("Deg".into());
                    }
                }
                for j in 1..=snap.nterms {
                    for i in 1..=snap.nconds {
                        self.header.push(format!("I{i}T{j}"));
                        self.header.push("Deg".into());
                    }
                }
            }
            12 => {
                let np = snap.nphases;
                self.record_size = 2 * ((np * snap.nterms) + snap.yorder);
                // Phase-pair map (LL): 1->2, 2->3, ..., np->1.
                for j in 1..=snap.nterms {
                    for i in 1..=np {
                        let a = i;
                        let b = if i == np { 1 } else { i + 1 };
                        self.header.push(format!("V{a}-{b}T{j}"));
                        self.header.push("Deg".into());
                    }
                }
                for j in 1..=snap.nterms {
                    for i in 1..=snap.nconds {
                        self.header.push(format!("I{i}T{j}"));
                        self.header.push("Deg".into());
                    }
                }
            }
            _ => self.clear_general_header(nphases, nconds),
        }
    }

    /// The general V/I header (modes 0/1) with the ±16/±32/±64 modifiers.
    fn clear_general_header(&mut self, nphases: usize, nconds: usize) {
        let is_pos_seq = (self.mode & SEQUENCEMASK) > 0 && nphases == 3;
        let num_vi = if is_pos_seq { 3 } else { nconds };
        let is_power = (self.mode & MODEMASK) == 1;

        match self.mode & (MAGNITUDEMASK + POSSEQONLYMASK) {
            32 => {
                self.record_size = num_vi;
                if !is_power {
                    self.record_size += num_vi;
                    if self.include_residual {
                        self.record_size += 2;
                    }
                    for i in 1..=num_vi {
                        self.header.push(format!("|V|{i} (volts)"));
                    }
                    if self.include_residual {
                        self.header.push("|VN| (volts)".into());
                    }
                    for i in 1..=num_vi {
                        self.header.push(format!("|I|{i} (amps)"));
                    }
                    if self.include_residual {
                        self.header.push("|IN| (amps)".into());
                    }
                } else {
                    for i in 1..=num_vi {
                        if self.pp_polar {
                            self.header.push(format!("S{i} (kVA)"));
                        } else {
                            self.header.push(format!("P{i} (kW)"));
                        }
                    }
                }
            }
            64 => {
                self.record_size = 2;
                if !is_power {
                    self.record_size += 2;
                    if self.vi_polar {
                        for s in ["V1", "V1ang", "I1", "I1ang"] {
                            self.header.push(s.into());
                        }
                    } else {
                        for s in ["V1.re", "V1.im", "I1.re", "I1.im"] {
                            self.header.push(s.into());
                        }
                    }
                } else if self.pp_polar {
                    self.header.push("S1 (kVA)".into());
                    self.header.push("Ang".into());
                } else {
                    self.header.push("P1 (kW)".into());
                    self.header.push("Q1 (kvar)".into());
                }
            }
            96 => {
                self.record_size = 1;
                if !is_power {
                    self.record_size += 1;
                    self.header.push("V".into());
                    self.header.push("I".into());
                } else if self.pp_polar {
                    self.header.push("S1 (kVA)".into());
                } else {
                    self.header.push("P1 (kW)".into());
                }
            }
            _ => {
                self.record_size = num_vi * 2;
                let (i_min, i_max) = if is_pos_seq {
                    (0, num_vi - 1)
                } else {
                    (1, num_vi)
                };
                if !is_power {
                    self.record_size += num_vi * 2;
                    if self.include_residual {
                        self.record_size += 4;
                    }
                    for i in i_min..=i_max {
                        if self.vi_polar {
                            self.header.push(format!("V{i}"));
                            self.header.push(format!("VAngle{i}"));
                        } else {
                            self.header.push(format!("V{i}.re"));
                            self.header.push(format!("V{i}.im"));
                        }
                    }
                    if self.include_residual {
                        if self.vi_polar {
                            self.header.push("VN".into());
                            self.header.push("VNAngle".into());
                        } else {
                            self.header.push("VN.re".into());
                            self.header.push("VN.im".into());
                        }
                    }
                    for i in i_min..=i_max {
                        if self.vi_polar {
                            self.header.push(format!("I{i}"));
                            self.header.push(format!("IAngle{i}"));
                        } else {
                            self.header.push(format!("I{i}.re"));
                            self.header.push(format!("I{i}.im"));
                        }
                    }
                    if self.include_residual {
                        if self.vi_polar {
                            self.header.push("IN".into());
                            self.header.push("INAngle".into());
                        } else {
                            self.header.push("IN.re".into());
                            self.header.push("IN.im".into());
                        }
                    }
                } else {
                    for i in i_min..=i_max {
                        if self.pp_polar {
                            self.header.push(format!("S{i} (kVA)"));
                            self.header.push(format!("Ang{i}"));
                        } else {
                            self.header.push(format!("P{i} (kW)"));
                            self.header.push(format!("Q{i} (kvar)"));
                        }
                    }
                }
            }
        }
    }

    /// Pascal `TMonitorObj.TakeSample`: append one record for the present
    /// solution. `metered` is the monitored circuit element (the source for a
    /// mode-5 monitor, which does not read it).
    pub fn take_sample(
        &mut self,
        metered: &mut dyn DssObject,
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
            let e = metered
                .as_ckt_element()
                .expect("metered element is a circuit element");
            (e.cd().nconds, e.cd().yorder)
        };
        let offset = (self.med.metered_terminal as usize - 1) * m_nconds;
        let fnphases = self.med.cd.nphases;
        let fnconds = self.med.cd.nconds;

        // Time stamp (frequency/harmonic in harmonic mode — Phase 7 path).
        if sol.is_harmonic {
            self.add_dbl(sol.frequency);
            self.add_dbl(sol.harmonic);
        } else {
            self.add_dbl(self.hour as f64);
            self.add_dbl(self.sec);
        }

        let mode_mask = self.mode & MODEMASK;

        // Scratch buffers (Pascal keeps them as fields for reuse).
        let mut current_buffer = vec![Complex64::ZERO; m_yorder.max(fnconds + 1)];
        let mut voltage_buffer = vec![Complex64::ZERO; m_yorder.max(fnconds + 1)];

        match mode_mask {
            0 | 1 => {
                let e = metered.as_ckt_element_mut().expect("ckt element");
                e.compute_iterminal(sys, node_v);
                let cd = e.cd();
                current_buffer[..m_yorder].copy_from_slice(&cd.iterminal[..m_yorder]);
                for (i, v) in voltage_buffer.iter_mut().enumerate().take(fnconds) {
                    *v = node_v[self.med.cd.node_ref[i]];
                }
            }
            2 => {
                let tap = metered
                    .as_any()
                    .downcast_ref::<Transformer>()
                    .map(|t| t.present_tap(self.med.metered_terminal as usize))
                    .unwrap_or(0.0);
                self.add_dbl(tap);
                return;
            }
            5 => {
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
            6 => {
                if let Some(cap) = metered.as_any().downcast_ref::<Capacitor>() {
                    let states: Vec<f64> = cap.states().iter().map(|&s| s as f64).collect();
                    self.add_dbls(&states);
                }
                return;
            }
            9 => {
                let e = metered.as_ckt_element_mut().expect("ckt element");
                let losses = e.losses(sys, node_v);
                self.add_dbl(losses.re);
                self.add_dbl(losses.im);
                return;
            }
            11 => {
                let e = metered.as_ckt_element_mut().expect("ckt element");
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
            // Modes 3 (state vars), 4 (flicker/Pstcalc), 7 (Storage), 8/10
            // (transformer winding currents/voltages), 12 (LL) build their
            // header but defer the sample body to Phase 6+/7 (no gate uses
            // them; the metered surface they need is not yet exposed).
            _ => return,
        }

        // --- Common tail for modes 0 and 1 --------------------------------
        let is_sequence = (self.mode & SEQUENCEMASK) > 0 && fnphases == 3;
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
        let is_power = mode_mask == 1;
        if mode_mask == 0 {
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
        match self.mode & (MAGNITUDEMASK + POSSEQONLYMASK) {
            32 => {
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
            64 => {
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
            96 => {
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
            _ => {
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

impl CktElement for Monitor {
    fn cd(&self) -> &CktElementData {
        &self.med.cd
    }
    fn cd_mut(&mut self) -> &mut CktElementData {
        &mut self.med.cd
    }

    fn recalc_element_data(&mut self, _sys: &SysCtx) {
        let mut errors = Vec::new();
        self.recalc(&mut errors);
        for e in errors {
            self.med.cd.obj.push_error(e);
        }
    }

    /// `TMonitorObj.CalcYPrim` is empty — a monitor never stamps admittance.
    fn calc_yprim(&mut self, _sys: &SysCtx) {}

    /// `TMonitorObj.GetCurrents` returns zeros (12-7-99 fix: a monitor is a
    /// zero-current source so it does not perturb Newton iteration).
    fn get_currents(&mut self, _sys: &SysCtx, _node_v: &[Complex64], curr: &mut [Complex64]) {
        curr.fill(Complex64::ZERO);
    }
}

impl DssObject for Monitor {
    fn data(&self) -> &DssObjData {
        &self.med.cd.obj
    }
    fn data_mut(&mut self) -> &mut DssObjData {
        &mut self.med.cd.obj
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
            TERMINAL => self.med.metered_terminal,
            MODE => self.mode,
            _ => unreachable!("Monitor has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        use prop::*;
        match idx {
            TERMINAL => self.med.metered_terminal = value,
            MODE => self.mode = value,
            _ => unreachable!("Monitor has no integer property {idx}"),
        }
    }

    fn get_f64(&self, idx: usize) -> f64 {
        match idx {
            prop::BASE_FREQ => self.med.cd.base_frequency,
            _ => unreachable!("Monitor has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        match idx {
            prop::BASE_FREQ => self.med.cd.base_frequency = value,
            _ => unreachable!("Monitor has no double property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        use prop::*;
        match idx {
            RESIDUAL => self.include_residual,
            VIPOLAR => self.vi_polar,
            PPOLAR => self.pp_polar,
            ENABLED => self.med.cd.enabled,
            _ => unreachable!("Monitor has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        use prop::*;
        match idx {
            RESIDUAL => self.include_residual = value,
            VIPOLAR => self.vi_polar = value,
            PPOLAR => self.pp_polar = value,
            ENABLED => self.med.cd.set_enabled(value),
            _ => unreachable!("Monitor has no boolean property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        match idx {
            prop::ELEMENT => self.element_full_name.clone(),
            _ => unreachable!("Monitor has no string property {idx}"),
        }
    }
    fn set_string(&mut self, idx: usize, value: String) {
        match idx {
            prop::ELEMENT => self.element_full_name = value,
            _ => unreachable!("Monitor has no string property {idx}"),
        }
    }

    fn set_bus_name(&mut self, terminal: usize, value: &str) {
        self.med.cd.set_bus(terminal, value);
    }
    fn get_bus_name(&self, terminal: usize) -> String {
        self.med.cd.get_bus(terminal).to_string()
    }

    /// Resolve `element=` (any circuit class by full name): capture a snapshot
    /// of the metered element for `RecalcElementData`/`ClearMonitorStream`.
    fn set_object_ref(
        &mut self,
        idx: usize,
        name: String,
        resolved: Option<(ElemRef, &dyn DssObject)>,
    ) {
        match idx {
            prop::ELEMENT => {
                self.element_full_name = name.clone();
                match resolved {
                    Some((r, obj)) => {
                        self.med.metered_element = Some(r);
                        self.med.metered_element_changed = true;
                        self.med.metered_snap = Some(capture_metered(name, obj));
                    }
                    None => {
                        self.med.metered_element = None;
                        self.med.metered_snap = None;
                    }
                }
            }
            _ => unreachable!("Monitor has no object-ref property {idx}"),
        }
    }

    /// Pascal `DoAction`: Clear/Reset → `ResetIt`; Save/TakeSample/Process are
    /// meaningful only via the `Sample` command / solution loop (they need the
    /// live metered element + node voltages), so they no-op here.
    fn do_action(&mut self, ordinal: i32, _errors: &mut Vec<String>) {
        if ordinal == 0 {
            self.reset_it();
        }
    }

    fn end_edit(&mut self) {
        let mut errors = Vec::new();
        self.recalc(&mut errors);
        for e in errors {
            self.med.cd.obj.push_error(e);
        }
    }

    fn make_like(&mut self, other: &dyn DssObject) {
        let Some(o) = other.as_any().downcast_ref::<Monitor>() else {
            return;
        };
        self.med.cd.make_like_base(&o.med.cd);
        self.med.cd.nphases = o.med.cd.nphases;
        self.med.cd.set_nconds(o.med.cd.nconds);
        self.med.metered_element = o.med.metered_element;
        self.med.metered_terminal = o.med.metered_terminal;
        self.med.metered_snap = o.med.metered_snap.clone();
        self.element_full_name = o.element_full_name.clone();
        self.mode = o.mode;
        self.include_residual = o.include_residual;
        self.vi_polar = o.vi_polar;
        self.pp_polar = o.pp_polar;
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}

/// Capture the parse-relevant shape + header dimensions of the metered element.
fn capture_metered(full_name: String, obj: &dyn DssObject) -> MeteredSnapshot {
    let elem = obj
        .as_ckt_element()
        .expect("element= resolves to a ckt elem");
    let cd = elem.cd();
    let (kind, num_windings, num_steps) =
        if let Some(t) = obj.as_any().downcast_ref::<Transformer>() {
            (
                MeteredKind::Transformer,
                t.num_windings().max(0) as usize,
                0,
            )
        } else if let Some(c) = obj.as_any().downcast_ref::<Capacitor>() {
            (MeteredKind::Capacitor, 0, c.states().len())
        } else if obj
            .as_any()
            .downcast_ref::<crate::elements::pc::load::Load>()
            .is_some()
            || obj
                .as_any()
                .downcast_ref::<crate::elements::pc::generator::Generator>()
                .is_some()
        {
            (MeteredKind::PcElement, 0, 0)
        } else {
            (MeteredKind::Other, 0, 0)
        };
    MeteredSnapshot {
        full_name,
        kind,
        nphases: cd.nphases,
        nconds: cd.nconds,
        nterms: cd.nterms,
        yorder: cd.yorder,
        buses: (1..=cd.nterms).map(|i| cd.get_bus(i).to_string()).collect(),
        num_windings,
        num_steps,
        num_variables: 0,
    }
}
