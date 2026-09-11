//! Port of `TStorageObj`'s `DebugTrace` file — the per-element CSV trace a
//! `debugtrace=yes` opens in the output directory and appends one line to at
//! every injection / terminal-current computation.
//!
//! r4133 `Version8/Source/PCElements/Storage.pas`: the edit-time header
//! (`:1073-1085`, `AssignFile(TraceFile, GetOutputDirectory + 'STOR_'+Name+'.CSV')`
//! → `ReWrite` → header → `CloseFile`), `WriteTraceRecord` (`:2401-2429`) at its
//! two call sites — `GetTerminalCurrents` `:2874` (`'TotalCurrent'`) and
//! `InjCurrents` `:2888` (`'Injection'`) — and the dynamics record at the tail of
//! `IntegrateStates` (`:3676-3683`). dss_capi 0.14.5 `src/PCElements/Storage.pas`
//! carries the same three sites with the same format strings (`:868-885`,
//! `:1946-1990`, the two calls at `:2356`/`:2367`); the only differences are the
//! file *extension* (`.csv` vs r4133's `.CSV` — one member after the corpus
//! gate's ASCII fold) and capi's never-closed handle (see below).
//!
//! Three deliberate port decisions, each measured or cited:
//!
//! * **Every write is open-append-close.** r4133 opens at edit time, closes
//!   immediately (`:1085`), and each record does `Append` → write → `CloseFile`
//!   (`:2410`/`:2429`). capi instead keeps a `TBufferedFileStream` open for the
//!   life of the object (`:872`, freed only at `:871`/`:1199`), which makes its
//!   own corpus guard's `os.remove` fail and leaks the file into the next
//!   producer's pre-run snapshot (measured, GOLDEN_REBASE G1.10a F4). The port
//!   follows r4133: no handle is ever held open, so the file can always be
//!   deleted by the run-file guards.
//! * **The file is created by the executive**, not by the property hook: the hook
//!   cannot reach `OutputDirectory` (the `ShapeSave` precedent,
//!   `exec/command.rs`), so [`Storage::queue_debug_trace`] builds the *header*
//!   with the phase/variable counts as they stand at that instant (r4133 builds
//!   it inside the `CASE` arm) and the executive drains it through
//!   [`crate::obj::base::DssObject::open_debug_traces`] before `end_edit`.
//! * **`InShowResults` is honoured** (GOLDEN_REBASE G1.10b F0): r4133 skips the
//!   record while a report command is assembling its file (`:2408`, inside
//!   `WriteTraceRecord`'s `Try`), so a `Show`/`Export` that reads element
//!   currents does not grow the trace. The flag is the DSS-instance global
//!   `Common/DSSGlobals.pas:242`, raised and lowered around `DoShowCmd`
//!   (`Executive/ShowOptions.pas:204`/`:393`) and `DoExportCmd`
//!   (`Executive/ExportOptions.pas:328`/`:512`) and raised — never lowered — by
//!   `DoSaveCmd` (`Executive/ExecHelper.pas:935`; that latch is an upstream
//!   defect the port does not reproduce, see [`crate::exec`]'s save bracket).
//!   The port keeps it on the circuit and reads it through
//!   [`SysCtx::in_show_results`](crate::elements::traits::SysCtx). Only
//!   `WriteTraceRecord` is gated: the dynamics record at the tail of
//!   `IntegrateStates` (`:3676-3683`) tests `DebugTrace` alone in r4133, and so
//!   does [`Storage::write_dynamics_trace_record`].
//!
//! Number formatting follows the two Pascal specs literally:
//! `%-.g` ([`TRACE_G_SIG`]) and the `:8:2` / `:8:1` fixed fields
//! ([`crate::report::format::fixed_w`]). Line endings are the port's usual `\n`
//! (the oracles' Pascal text mode emits CRLF on Windows; every port report
//! writer emits `\n` and the text comparators normalize).

use std::path::{Path, PathBuf};

use crate::diag::ErrorLog;
use crate::elements::traits::{CktElement, SysCtx};
use crate::report::format;
use crate::solution::{LoadSolutionModel, SolveMode};
use crate::support::dynamics::IterationFlag;

use super::Storage;

/// The significant-digit request behind FPC `Format('%-.g', …)` — the precision
/// after the dot is **empty**, which FPC reads as `0` and
/// [`crate::util::fpc_general_digits`] clamps to Grisu1's floor of **2**
/// significant digits (`%-g`, with no dot at all, is the 15-digit default the
/// demand-interval writer uses — the two specs are not the same).
///
/// Measured against the pinned oracle's own `STOR_storage1.csv` (dss-python
/// 0.15.7 / dss_capi 0.14.5, `Storage_price.dss` on a scratch copy): `250` prints
/// `2.5E2`, `8000` prints `8E3`, `-250` prints `-2.5E2`, `0.5` prints `0.5` and
/// `50` prints `50` — exactly the 2-significant-digit `ffGeneral` window, not the
/// 6- or 15-digit readings the unobservable `Show ControlQueue` row had to guess
/// at ([`crate::compat::CONTROL_QUEUE_SEC_DIGITS`]).
///
/// The **two oracles disagree here**, and no port spelling can satisfy both: the
/// r4133 DLL is a Delphi build whose `Format` reads the empty precision as
/// "default" and prints ~15 significant digits — the same file, same deck,
/// measured through `epri-worker` on a clean scratch copy, renders `250` (not
/// `2.5E2`), `9999` (not `1E4`), `8000` (not `8E3`) and `5.45` (not `5.5`). The
/// port keeps the FPC reading, which is the one every other port writer's number
/// goes through ([`crate::util::fmt_g`], the goldens' contract); the trace file's
/// *contents* are not oracle-compared by the G1.10a run-file surface (names only),
/// and the divergence is recorded for the G1.10b contents surface, which has to
/// normalize or exclude it whichever spelling the port picks.
const TRACE_G_SIG: usize = 0;

/// The `DebugTrace` file of one Storage element.
#[derive(Debug, Clone, Default)]
pub(super) struct DebugTraceFile {
    /// `(file name, header line)` queued by the `DebugTrace=yes` property hook,
    /// drained by the executive once `OutputDirectory` is reachable.
    pending: Option<(String, String)>,
    /// Where the executive created the file. `None` until then (and after a
    /// failed create — a record is then simply not written).
    path: Option<PathBuf>,
}

impl Storage {
    /// r4133 `Storage.pas:1073-1085` (`propDEBUGTRACE`, `IF DebugTrace THEN`):
    /// build the trace file's name and header with the phase count and state
    /// variables as they stand at *this* property assignment, and queue the
    /// create for the executive (see the module doc).
    ///
    /// Pascal `ReWrite`s the file, so re-issuing `debugtrace=yes` truncates it and
    /// writes a fresh header; queuing again reproduces that.
    pub(super) fn queue_debug_trace(&mut self) {
        // Pascal writes the header only when the property turns the trace ON;
        // `debugtrace=no` writes nothing (capi additionally frees its handle,
        // `:886-889` — the port's writes are gated on `debug_trace` instead).
        if !self.base.debug_trace {
            return;
        }
        let file_name = format!("STOR_{}.CSV", self.cd.obj.name());
        let nphases = self.cd.nphases;
        let mut header = String::from(
            "t, Iteration, LoadMultiplier, Mode, LoadModel, StorageModel,  \
             Qnominalperphase, Pnominalperphase, CurrentType",
        );
        // The four header loops run 0-based like the rest of the engine; the
        // `i + 1` written into each label — and into the `variable_name`
        // argument, whose contract is 1-based
        // ([`crate::elements::traits::CktElement::variable_name`]) — is the
        // 1-based *user-API boundary*, the only place a 1-based index is
        // allowed (DE_PASCALIZE P14).
        for i in 0..nphases {
            header.push_str(&format!(", |Iinj{}|", i + 1));
        }
        for i in 0..nphases {
            header.push_str(&format!(", |Iterm{}|", i + 1));
        }
        for i in 0..nphases {
            header.push_str(&format!(", |Vterm{}|", i + 1));
        }
        for i in 0..self.num_variables() {
            header.push_str(&format!(", {}", self.variable_name(i + 1)));
        }
        // Pascal `Write(TraceFile, ',Vthev, Theta')` — two columns the record
        // writer never fills (its own write is commented out at `:2426`).
        header.push_str(",Vthev, Theta\n");
        self.trace.pending = Some((file_name, header));
    }

    /// Executive drain (see the module doc): create/truncate the queued trace
    /// file in `output_directory`, write its header, close it, and remember the
    /// path for the per-record appends. A failed create is a loud diagnostic and
    /// leaves the element trace-less rather than half-open.
    pub(super) fn open_queued_debug_trace(
        &mut self,
        output_directory: &Path,
        errors: &mut ErrorLog,
    ) {
        let Some((file_name, header)) = self.trace.pending.take() else {
            return;
        };
        let path = output_directory.join(&file_name);
        match std::fs::write(&path, header.as_bytes()) {
            Ok(()) => self.trace.path = Some(path),
            Err(e) => {
                self.trace.path = None;
                errors.push(format!("Error writing file: \"{}\" ({e})", path.display()));
            }
        }
    }

    /// Append one line to the trace file, opening and closing it around the
    /// write (r4133 `Append` … `CloseFile`, `:2410`/`:2429`). r4133 swallows any
    /// I/O failure here (`Except On E:Exception Do Begin End;`, `:2431-2433`) and
    /// so does the port — a debug trace must never break a solve — but the
    /// element stops tracing instead of retrying a dead path every record.
    fn append_trace_line(&mut self, line: &str) {
        use std::io::Write;
        let Some(path) = self.trace.path.clone() else {
            return;
        };
        let opened = std::fs::OpenOptions::new().append(true).open(&path);
        match opened.and_then(|mut f| f.write_all(line.as_bytes())) {
            Ok(()) => {}
            Err(_) => self.trace.path = None,
        }
    }

    /// r4133 `TStorageObj.WriteTraceRecord` (`Storage.pas:2401-2429`) — one line
    /// of solution scalars, the nominal per-phase P/Q, the caller's tag `s`, the
    /// per-phase |Iinj| / |Iterm| / |Vterm| magnitudes and every state variable.
    ///
    /// The two tags are Pascal's own: `'TotalCurrent'` from `GetTerminalCurrents`
    /// (`:2874`) and `'Injection'` from `InjCurrents` (`:2888`).
    pub(super) fn write_trace_record(
        &mut self,
        tag: &str,
        sys: &SysCtx,
        node_v: &[num_complex::Complex64],
    ) {
        // `If (Not InshowResults)` (`:2408`): a record is written only outside a
        // report command — the `Show`/`Export` that reads this element's currents
        // must not appear in its own debug trace. `DebugTrace` itself is Pascal's
        // call-site test (`:2874`, `:2888`), kept here so both call sites share it.
        if !self.base.debug_trace || sys.in_show_results || self.trace.path.is_none() {
            return;
        }
        // Pascal `Variable[i]` (`Get_Variable`) per column; read before the line
        // is assembled because the getter needs `&mut self`.
        let mut states = vec![0.0_f64; self.num_variables()];
        self.get_all_variables(sys, node_v, &mut states);

        let nphases = self.cd.nphases;
        let mut line = format!(
            "{}, {}, {}, {}, {}, {}, {}, {}, {}, ",
            format::g(sys.dbl_hour, TRACE_G_SIG),
            sys.iteration,
            format::g(sys.load_multiplier, TRACE_G_SIG),
            solution_mode_id(sys.mode),
            load_model_id(sys.load_model),
            self.base.voltage_model,
            format::fixed_w(self.base.q_nominal_per_phase * 3.0 / 1.0e6, 8, 2),
            format::fixed_w(self.base.p_nominal_per_phase * 3.0 / 1.0e6, 8, 2),
            tag,
        );
        for i in 0..nphases {
            line.push_str(&format::fixed_w(self.cd.inj_current[i].norm(), 8, 1));
            line.push_str(", ");
        }
        for i in 0..nphases {
            line.push_str(&format::fixed_w(self.cd.iterminal[i].norm(), 8, 1));
            line.push_str(", ");
        }
        for i in 0..nphases {
            line.push_str(&format::fixed_w(self.cd.vterminal[i].norm(), 8, 1));
            line.push_str(", ");
        }
        for v in &states {
            line.push_str(&format::g(*v, TRACE_G_SIG));
            line.push_str(", ");
        }
        line.push('\n');
        self.append_trace_line(&line);
    }

    /// r4133 `TStorageObj.IntegrateStates`' trailing trace block
    /// (`Storage.pas:3676-3683`): `Format('t=%-.5g ', [DynaVars.t])` +
    /// `Format(' Flag=%d ', [DynaVars.IterationFlag])` + newline, appended once
    /// per integration call on the built-in (non-`DynaModel`) path.
    pub(super) fn write_dynamics_trace_record(&mut self, sys: &SysCtx) {
        if !self.base.debug_trace || self.trace.path.is_none() {
            return;
        }
        let line = format!(
            "t={}  Flag={} \n",
            format::g(sys.dyna_t, 5),
            // Pascal's `DynaVars.IterationFlag` is a bare 0/1 Integer (the same
            // mapping the user-model ABI packs, `storage/user_model.rs`).
            match sys.iteration_flag {
                IterationFlag::NewTimeStep => 0,
                IterationFlag::SameTimeStep => 1,
            }
        );
        self.append_trace_line(&line);
    }
}

/// r4133 `GetSolutionModeIDName` (`Common/Utilities.pas:1147-1173`, reached from
/// `GetSolutionModeID` `:1175-1181`) — the authority's own spelling, which is
/// *not* the capi enum's (`DSS.SolveModeEnum.OrdinalToString`, whose names are
/// `PeakDay`/`DutyCycle`/`Direct`/`Dynamic`/`FaultStudy`/`AutoAdd`); the two
/// agree on every other mode, `Daily` and `Snap` included.
fn solution_mode_id(mode: SolveMode) -> &'static str {
    match mode {
        SolveMode::Snapshot => "Snap",
        SolveMode::Daily => "Daily",
        SolveMode::Yearly => "Yearly",
        SolveMode::Monte1 => "M1",
        SolveMode::Monte2 => "M2",
        SolveMode::Monte3 => "M3",
        SolveMode::LD1 => "LD1",
        SolveMode::LD2 => "LD2",
        SolveMode::PeakDay => "Peakday",
        SolveMode::DutyCycle => "DUtycycle",
        SolveMode::Direct => "DIrect",
        SolveMode::Dynamic => "DYnamic",
        SolveMode::MonteFault => "MF",
        SolveMode::FaultStudy => "Faultstudy",
        SolveMode::AutoAdd => "Autoadd",
        SolveMode::Harmonic => "Harmonic",
        SolveMode::HarmonicT => "HarmonicT",
        SolveMode::Time => "Time",
    }
}

/// r4133 `GetLoadModel` (`Common/Utilities.pas:1218-1228`) — `Admittance` for
/// `ADMITTANCE`, `PowerFlow` for everything else (capi's
/// `DefaultLoadModelEnum` spells both the same way).
fn load_model_id(load_model: LoadSolutionModel) -> &'static str {
    match load_model {
        LoadSolutionModel::Admittance => "Admittance",
        LoadSolutionModel::PowerFlow => "PowerFlow",
    }
}
