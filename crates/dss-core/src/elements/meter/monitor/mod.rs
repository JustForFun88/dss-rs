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
//! 3 (PCElement state vars), 5 (solution variables), 6 (capacitor steps), 9
//! (losses), 11 (all terminal V&I) — plus the ±16/±32/±64 modifiers, residual,
//! VIpolar/Ppolar. Modes 4 (flicker/Pstcalc), 7 (Storage), 8/10 (transformer
//! winding currents/voltages) and 12 (LL voltages) build their **header** but
//! defer the sample body (Phase 6+/7); the gate exercises 0/1/2/5. File
//! save/`TranslateToCSV` is Phase 8.
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
mod dump;
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
        PropDef::object_ref_any("Element"),
        PropDef::integer("Terminal"),
        PropDef::integer("Mode"),
        PropDef::action("Action", enums.monitor_action),
        PropDef::boolean("Residual"),
        PropDef::boolean("VIPolar"),
        PropDef::boolean("PPolar"),
        // CktElementClass tail:
        PropDef::double("BaseFreq").flags(
            crate::obj::props::PropFlags::NON_NEGATIVE | crate::obj::props::PropFlags::NON_ZERO,
        ),
        PropDef::enabled("Enabled"),
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

    /// In-memory sample buffer (Pascal `MonBuffer` / `MonitorStream` merged into
    /// one growing `Vec<f32>`: we never spill to disk, so nothing is ever
    /// physically discarded). Each record is `[hour, sec, ch1..ch_record_size]`.
    /// `TakeSample` always appends here (Pascal `AddDblToBuffer` into
    /// `MonBuffer`); [`Self::flushed_records`] tracks how many leading records
    /// are additionally visible via `Channel`/`dblHour` (Pascal `Save`/
    /// `SaveAll` copying `MonBuffer` into `MonitorStream`, which is the only
    /// thing `Monitors_Get_Channel`/`Monitors_Get_ByteStream` ever reads —
    /// oracle-probed, `SolveGeneralTime` never calls `SaveAll`, so its
    /// `Channel()` output stays stuck at whatever was last flushed).
    mon_buffer: Vec<f32>,
    record_size: usize,
    sample_count: i32,
    /// Pascal `MonitorStream` size in whole records (not modeled as a separate
    /// byte stream: `mon_buffer[..flushed_records*stride]` IS the flushed
    /// portion). Advances to `sample_count` on [`Self::save`]. The Pascal
    /// `BufferSize=1024`-double auto-flush-on-overflow (`AddDblToBuffer`,
    /// `Monitor.pas:1596`) is not modeled — no ported deck samples anywhere
    /// near 1024/`(record_size+2)` records between two `Save`s.
    flushed_records: usize,
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
            flushed_records: 0,
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
    /// `Channel(i)` (1-based): the i-th recorded value across every **flushed**
    /// sample (Pascal `Monitors_Get_Channel`/`Monitors.Channel` read
    /// `MonitorStream`, not the live `SampleCount` — a sample taken since the
    /// last `Save`/`SaveAll` is invisible here; see [`Self::flushed_records`]).
    ///
    /// TODO(compat): when nothing has been flushed yet (`flushed_records ==
    /// 0`), dss-python's `IMonitors.Channel` (not the raw C-API — the Python
    /// wrapper itself, `IMonitors.py`) special-cases an all-header
    /// `MonitorStream` (byte size `== 272`, oracle-probed) into a **one**-
    /// element `[0.0]` placeholder rather than an empty array; reproduced
    /// here for oracle parity, since that is what a live comparison actually
    /// observes. Clean fix (report an honest empty/absent channel) would need
    /// its own gate on the raw C-API instead of dss-python's convenience
    /// wrapper.
    pub fn channel(&self, i: usize) -> Vec<f32> {
        if i < 1 || i > self.record_size {
            return Vec::new();
        }
        if self.flushed_records == 0 {
            return vec![0.0];
        }
        let stride = self.record_size + 2;
        (0..self.flushed_records)
            .map(|s| self.mon_buffer[s * stride + 2 + (i - 1)])
            .collect()
    }
    /// `dblHour`: the per-sample hour values (record slot 0), **flushed**
    /// samples only — see [`Self::channel`].
    pub fn dbl_hour(&self) -> Vec<f64> {
        let stride = self.record_size + 2;
        (0..self.flushed_records)
            .map(|s| self.mon_buffer[s * stride] as f64)
            .collect()
    }

    /// Pascal `TMonitorObj.Save` (`Meters/Monitor.pas:1118`): flush the pending
    /// buffer into `MonitorStream` — here, advance the flush cursor to
    /// `sample_count` so `channel`/`dbl_hour` see every sample taken so far.
    /// Called per-monitor by `TDSSMonitor.SaveAll` (`solution/monitors.rs`
    /// `save_all_monitors`), which every ported ordinary solve mode invokes at
    /// its natural end **except** `SolveGeneralTime` (deliberately, "roll your
    /// own") and `SolveFaultStudy` (which never samples monitors at all).
    pub fn save(&mut self) {
        self.flushed_records = self.sample_count as usize;
    }

    /// Pascal `TMonitorObj.TranslateToCSV` (`Meters/Monitor.pas:1690`): serialize
    /// the in-memory sample buffer to the monitor CSV text. Line 1 is the header
    /// (`Header.CommaText`); each subsequent line is `hr:0:0, s:0:5` followed by
    /// `, %-.6g` per recorded channel (`RecordSize` values). Pascal's own first
    /// line is `Save;` (Monitor.pas:1713) — `TranslateToCSV` always self-flushes
    /// before reading, so (unlike [`Self::channel`]/[`Self::dbl_hour`]) this
    /// iterates every sample taken so far (`sample_count`), never gated by
    /// [`Self::flushed_records`]. The `Show`/`FireOffEditor` leg is the GUI
    /// no-op; `GlobalResult` is set by the caller (`export.rs`).
    pub fn to_csv(&self) -> String {
        let mut out = String::new();
        out.push_str(&crate::util::comma_text(&self.header));
        out.push('\n');
        let stride = self.record_size + 2;
        for s in 0..self.sample_count as usize {
            let base = s * stride;
            // Pascal `WriteStr(sout, hr: 0: 0, ', ', s: 0: 5)` — the two leading
            // time columns (or Freq/Harmonic in harmonics mode) as fixed-point.
            let hr = self.mon_buffer[base] as f64;
            let sec = self.mon_buffer[base + 1] as f64;
            out.push_str(&format!("{hr:.0}, {sec:.5}"));
            for i in 0..self.record_size {
                // Pascal `Format(', %-.6g', [sngBuffer[i]])`.
                out.push_str(", ");
                out.push_str(&crate::util::fmt_g(self.mon_buffer[base + 2 + i] as f64, 6));
            }
            out.push('\n');
        }
        out
    }
}
