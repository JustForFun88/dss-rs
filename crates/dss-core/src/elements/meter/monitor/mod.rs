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
//! 3 (PCElement state vars), 4 (flicker/Pst — WP-PF.2), 5 (solution variables),
//! 6 (capacitor steps), 7 (Storage), 8 (transformer winding currents), 9
//! (losses), 10 (transformer winding voltages), 11 (all terminal V&I), 12
//! (line-to-line terminal voltages + currents) — plus the ±16/±32/±64
//! modifiers, residual, VIpolar/Ppolar. `TranslateToCSV` is ported as `to_csv`;
//! only the raw binary `.mon` Save (Monitor.pas:1118) is deferred. The mode-4
//! flicker post-process (`DoFlickerCalculations`) is in `post.rs`.
//!
//! Split into submodules mirroring `load/`, `generator/`, `vsource/`:
//! - this `mod.rs` — property ordinals, `class_props`, the `Monitor` struct,
//!   `MonitorSampleCtx`, `new`, and the read-only harness accessors.
//! - `header.rs` — `RecalcElementData`, `ResetIt`, and the per-mode header
//!   construction (`ClearMonitorStream` / the general V/I header).
//! - `sample.rs` — `TakeSample` and the polar/residual conversion helpers.
//! - `accessors.rs` — the `impl CktElement` / `impl DssObject` surface and the
//!   metered-element snapshot capture.

use num_complex::Complex64;

use crate::elements::meter::meter_element::MeterElementData;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef};

mod accessors;
mod dump;
mod header;
mod mode;
mod post;
mod sample;

use mode::{MonitorBaseMode, MonitorModeView};

const NUM_SOLUTION_VARS: usize = 12;
/// Pascal `BufferSize` (`Meters/Monitor.pas:478`): a **fixed** 1024-single
/// (4 KiB) scratch buffer. `AddDblToBuffer` flushes once `BufPtr` reaches it.
pub(super) const BUFFER_SIZE: usize = 1024;

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
            crate::obj::props::PropFlags::DYNAMIC_DEFAULT
                | crate::obj::props::PropFlags::NON_NEGATIVE
                | crate::obj::props::PropFlags::NON_ZERO
                | crate::obj::props::PropFlags::UNITS_HZ,
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
    /// Typed decode of the raw `mode=` bitfield (`mode` submodule). The raw
    /// `i32` survives only at the property boundary ([`Self::get_i32`]/
    /// [`Self::set_i32`] via `to_raw`/`from_raw`).
    mode: MonitorModeView,
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
    /// portion). Advances to `sample_count` on [`Self::save`]. Distinct from
    /// [`Self::bufptr`]: `flushed_records == 0` uniquely means "no explicit
    /// `Save`/`SaveAll` yet" (gating the dss-python `[0.0]` Channel placeholder),
    /// whereas `bufptr == 0` is also true right after a 1024-single auto-flush.
    flushed_records: usize,
    /// Pascal `BufPtr` (`Meters/Monitor.pas:143`): the live count of singles in
    /// the `MonBuffer` scratch since the last flush. In the merged
    /// MonBuffer+MonitorStream model a flush moves no data (every single already
    /// lives in `mon_buffer`) — this cursor just resets, so the last `bufptr`
    /// singles of `mon_buffer` are the unflushed remainder `DumpProperties`
    /// renders as `// Buffer=`. Advanced per single with the 1024 wrap by
    /// `AddDblToBuffer`, zeroed by [`Self::save`] and `ResetIt`.
    bufptr: usize,
    header: Vec<String>,
    valid_monitor: bool,
    hour: i32,
    sec: f64,
    /// Pascal `IsProcessed` (`Monitor.pas:174`): the mode-4 post-process latch.
    /// `PostProcess` runs `DoFlickerCalculations` once per fresh sample set (when
    /// the stream is first closed/read), then latches `true`; `ResetIt`/
    /// `ClearMonitorStream` clear it so a re-solved monitor reprocesses.
    is_processed: bool,
    /// Pascal `VoltageBuffer` / `CurrentBuffer` (`Monitor.pas:146-147`): the
    /// per-sample scratch arrays. Upstream keeps them as object fields (sized in
    /// `RecalcElementData`); the port sizes them per sample inside `TakeSample`
    /// but reuses the allocation across samples — every entry is re-zeroed on
    /// entry, so the contents are exactly a freshly allocated buffer's.
    voltage_buffer: Vec<Complex64>,
    current_buffer: Vec<Complex64>,
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
            mode: MonitorModeView::default(),
            include_residual: false,
            vi_polar: true,
            pp_polar: true,
            // Pascal `TMonitorObj.Create` (`Monitor.pas:482`):
            // `MeteredElement := ActiveCircuit.CktElements.Get(1)` — a fresh
            // monitor defaults to the first circuit element, which is always the
            // auto-created `Vsource.source`. Every real deck overrides this via
            // the (positional or `element=`) Element property before solve; it is
            // observable only as the all-default sample's schema default.
            element_full_name: "Vsource.source".to_string(),
            mon_buffer: Vec::new(),
            record_size: 0,
            sample_count: 0,
            flushed_records: 0,
            bufptr: 0,
            header: Vec::new(),
            valid_monitor: false,
            hour: 0,
            sec: 0.0,
            is_processed: false,
            voltage_buffer: Vec::new(),
            current_buffer: Vec::new(),
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
    /// The raw `mode=` ordinal (re-packed from the typed view) — the property
    /// report boundary. `solution::monitors` uses it for the Pascal
    /// `SampleAllMode5` `Mode = 5` full-ordinal split.
    pub fn mode_raw(&self) -> i32 {
        self.mode.to_raw()
    }
    pub fn num_channels(&self) -> usize {
        self.record_size
    }
    /// How many records `Save`/`SaveAll` has made visible to
    /// [`Self::channel`]/[`Self::dbl_hour`] (Pascal `MonitorStream` length in
    /// records). `0` means nothing was ever flushed — the state in which
    /// `Channel` has nothing to report (see [`Self::channel`]).
    pub fn flushed_records(&self) -> usize {
        self.flushed_records
    }
    /// `Channel(i)` (1-based): the i-th recorded value across every **flushed**
    /// sample (Pascal `Monitors_Get_Channel`/`Monitors.Channel` read
    /// `MonitorStream`, not the live `SampleCount` — a sample taken since the
    /// last `Save`/`SaveAll` is invisible here; see [`Self::flushed_records`]).
    ///
    /// Nothing to report — an index outside `1..=RecordSize`, or a stream
    /// nothing has been flushed into yet — is the **empty** channel, in both
    /// lanes: `Monitors_Get_Channel` returns `DefaultResult`, an empty array,
    /// for either (`CAPI_Monitors.pas:295-331`). It is also what the
    /// neighbouring [`Self::dbl_hour`] reads off the same stream.
    ///
    /// The one-element `[0.0]` an unflushed stream is *read back* as by the
    /// oracle clients is not an engine value and never was: dss-python's
    /// `IMonitors.Channel` (`dss/IMonitors.py:28-55`) bypasses that function,
    /// pulls the raw `ByteStream` and short-circuits `if cnt == 272` (the
    /// header-only stream size) to `np.zeros((1,))`; our r4133 bridge copies
    /// that decoder (`crates/dss-epri/src/dss.rs:625-634`) and the native r4133
    /// reader pads the same way (`DMonitors.pas:509-516` — `myDBLArray := [0]`,
    /// overwritten only `If pMon.SampleCount > 0`). It is a client artifact of
    /// every reader, so since `GOLDEN_REBASE_PLAN.md` G2.4 it is normalized out
    /// of the *capture* (`harness::lane::expected_monitor_channel`) instead of
    /// being reproduced here under a lane split.
    pub fn channel(&self, i: usize) -> Vec<f32> {
        if i < 1 || i > self.record_size || self.flushed_records == 0 {
            return Vec::new();
        }
        let stride = self.record_size + 2;
        (0..self.flushed_records)
            .map(|s| self.mon_buffer[s * stride + 2 + (i - 1)])
            .collect()
    }
    /// The metered terminal's bus name (bare, node qualifiers stripped) — the
    /// caller resolves its `kVBase` for the mode-4 flicker `Vbase`
    /// (`DoFlickerCalculations`, `Monitor.pas:1655-1656`). `None` if unset or not
    /// a mode-4 monitor.
    pub fn metered_bus_name(&self) -> Option<String> {
        if self.mode.base != MonitorBaseMode::Flicker {
            return None;
        }
        let snap = self.med.metered_snap.as_ref()?;
        let full = snap.buses.get(self.med.metered_terminal as usize - 1)?;
        Some(full.split('.').next().unwrap_or(full).to_string())
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
        self.bufptr = 0; // Pascal `Save` (Monitor.pas:1127): `BufPtr := 0`.
    }

    /// Pascal `TMonitorObj.TranslateToCSV` (`Meters/Monitor.pas:1690`): serialize
    /// the in-memory sample buffer to the monitor CSV text. Line 1 is the header
    /// (`Header.CommaText`); each subsequent line is `hr:0:0, s:0:5` followed by
    /// `, %-.6g` per recorded channel (`RecordSize` values).
    ///
    /// Pascal's own first statement is `Save;` (Monitor.pas:1713) — so
    /// `Export`/`Show Monitor` **flushes** the pending buffer as a side effect,
    /// after which a subsequent `Monitors.Channel`/`dblHour` read returns the
    /// full data. Reproduced via `self.save()` here (`&mut self`). It matters
    /// only in `mode=Time`, whose loop never calls `SaveAll`
    /// (`flushed_records` can be 0 at export time); every ordinary solve mode
    /// already flushed, so the call is idempotent there. The `Show`/
    /// `FireOffEditor` leg is the GUI no-op; `GlobalResult` is set by the
    /// caller (`export.rs`). The body itself iterates every sample taken so
    /// far (`sample_count`), independent of the flush cursor.
    pub fn to_csv(&mut self, kv_base: f64) -> String {
        self.save(); // Pascal `Save;` — flush pending, so Channel() reads full data next
        // Pascal `TranslateToCSV` calls `CloseMonitorStream` (→ `PostProcess` →
        // `DoFlickerCalculations` for mode 4) before serializing (Monitor.pas:1714).
        self.post_process(kv_base);
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

#[cfg(test)]
mod tests {
    use super::Monitor;

    /// Build a monitor with `n` synthetic 1-channel records staged in
    /// `mon_buffer` but NOT flushed (`flushed_records = 0`) — the `mode=Time`
    /// state: `TakeSample` appended to `MonBuffer`, but `SolveGeneralTime` never
    /// called `MonitorClass.SaveAll`.
    fn staged_monitor(n: usize) -> Monitor {
        let mut m = Monitor::new("m");
        m.record_size = 1; // one data channel -> stride 3 (hour, sec, ch1)
        m.sample_count = n as i32;
        for s in 0..n {
            m.mon_buffer.push(s as f32); // hour
            m.mon_buffer.push(0.0); // sec
            m.mon_buffer.push((10 + s) as f32); // ch1 value
        }
        m.header = vec!["hour".into(), "t(sec)".into(), "V1".into()];
        m
    }

    // EXPECTED-VALUE-PIN(MONITOR_CHANNEL_PADS_THE_UNFLUSHED_STREAM): the value
    // the engine reports for a stream nothing has been flushed into — the
    // **empty** channel `Monitors_Get_Channel` returns
    // (`CAPI_Monitors.pas:295-331`), asserted outright, in both lanes. The
    // one-element `[0.0]` is fabricated by the oracles' client-side stream
    // decoders (`dss/IMonitors.py:28-55`, `crates/dss-epri/src/dss.rs:625-634`,
    // r4133 `DMonitors.pas:509-516`), so no engine value is asserted away here;
    // the capture side normalizes it out instead
    // (`harness::lane::expected_monitor_channel`).
    /// Sharpened against [`channel_reflects_flush_state`], which walks the whole
    /// flush lifecycle: this one holds the *unflushed* answer still while the
    /// samples are demonstrably there — four records staged in `MonBuffer`,
    /// every channel of the monitor empty, and `dbl_hour` (the same stream read
    /// through a surface no client special-cases) empty beside it. So "empty"
    /// here means "nothing flushed", never "nothing sampled".
    #[test]
    fn monitor_channel_of_an_unflushed_stream_is_empty() {
        let m = staged_monitor(4);
        assert_eq!(m.sample_count(), 4, "four samples were taken");
        assert_eq!(m.flushed_records(), 0, "and none of them flushed");
        // 0-based loop, `+ 1` at the 1-based `Channel(i)` boundary — the port's
        // indexing convention (`DE_PASCALIZE` P14).
        for ch in 0..m.num_channels() {
            let i = ch + 1;
            assert_eq!(
                m.channel(i),
                Vec::<f32>::new(),
                "channel {i} of an unflushed stream is empty, not [0.0]"
            );
        }
        assert_eq!(
            m.dbl_hour(),
            Vec::<f64>::new(),
            "and it agrees with the neighbouring read of the same stream"
        );
    }

    /// Pascal `Monitors_Get_Channel` reads `MonitorStream` (the flushed data),
    /// not the live `SampleCount`: before any `Save`/`SaveAll`, `Channel` sees
    /// nothing — the empty channel pinned above, which is also what `dblHour`
    /// reports. After `save()` the full history is visible in both.
    #[test]
    fn channel_reflects_flush_state() {
        let mut m = staged_monitor(4);
        assert_eq!(m.flushed_records(), 0);
        assert_eq!(m.channel(1), Vec::<f32>::new());
        assert_eq!(m.dbl_hour(), Vec::<f64>::new());
        m.save();
        assert_eq!(m.flushed_records(), 4);
        assert_eq!(m.channel(1), vec![10.0, 11.0, 12.0, 13.0]);
        assert_eq!(m.dbl_hour(), vec![0.0, 1.0, 2.0, 3.0]);
    }

    /// An out-of-range index is the empty channel whatever the flush state —
    /// before `save()` (where the whole monitor is empty anyway) and after it,
    /// where the in-range channel carries data.
    #[test]
    fn channel_index_outside_the_record_is_empty() {
        let mut m = staged_monitor(2);
        assert_eq!(m.channel(0), Vec::<f32>::new());
        assert_eq!(m.channel(2), Vec::<f32>::new()); // record_size == 1
        m.save();
        assert_eq!(m.channel(1), vec![10.0, 11.0]);
        assert_eq!(m.channel(2), Vec::<f32>::new());
    }

    /// Pascal `TranslateToCSV` runs `Save;` first (Monitor.pas:1713), so
    /// `Export`/`Show Monitor` **flushes** as a side effect: a subsequent
    /// `Channel` read returns the full data even in `mode=Time` (MINOR 1). The
    /// CSV body itself always covers every sample regardless of the cursor.
    #[test]
    fn to_csv_flushes_like_pascal_save() {
        let mut m = staged_monitor(3);
        assert_eq!(m.channel(1), Vec::<f32>::new()); // unflushed before export
        let csv = m.to_csv(0.0); // not mode 4 -> kv_base unused
        // Body carries all three samples (values 10/11/12).
        assert_eq!(csv.lines().count(), 4); // header + 3 rows
        assert!(csv.contains("10") && csv.contains("11") && csv.contains("12"));
        // Side effect: now flushed, so Channel sees the whole history.
        assert_eq!(m.channel(1), vec![10.0, 11.0, 12.0]);
    }
}
