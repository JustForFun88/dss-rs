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
//!
//! Split into submodules mirroring `load/`, `generator/`, `vsource/`:
//! - this `mod.rs` — property ordinals, `class_props`, the `Monitor` struct,
//!   `MonitorSampleCtx`, `new`, and the read-only harness accessors.
//! - `header.rs` — `RecalcElementData`, `ResetIt`, and the per-mode header
//!   construction (`ClearMonitorStream` / the general V/I header).
//! - `sample.rs` — `TakeSample` and the polar/residual conversion helpers.
//! - `accessors.rs` — the `impl CktElement` / `impl DssObject` surface and the
//!   metered-element snapshot capture.

use crate::elements::meter::meter_element::MeterElementData;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef};

mod accessors;
mod header;
mod sample;

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
}
