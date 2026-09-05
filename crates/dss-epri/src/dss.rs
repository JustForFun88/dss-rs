//! Safe-ish wrapper over the raw [`crate::ffi`] DLL: command execution, error
//! polling, the V-protocol decode, and the individual accessors used to build a
//! `CaseResult`. Everything here mirrors what dss-python's `IOddieDSS` returns
//! for the *same* engine memory, so the captures are byte-for-byte comparable
//! (verified by `tools/opendss/xcheck_bridge.py` before its Phase E retirement).
//!
//! # SAFETY
//! All FFI calls go through `self.dll`'s function pointers, whose validity is
//! upheld by [`crate::ffi::Dll`] owning the loaded library. The DLL is
//! single-threaded and driven with one in-flight request; every V-protocol
//! buffer is copied out immediately (before any further call), so no borrowed
//! DLL heap is ever aliased or outlived.

use std::ffi::{c_char, c_void};
use std::path::Path;

use crate::families::{
    Family, FamilyTable, VData, decode_string_raw, decode_v, encode_v_set, vset_len,
};
use crate::ffi::{Dll, DllFns, FnV, YMatrixFns, cstr_to_string, to_cstring};
use crate::modes::{self, ModeKind, ModeSpec, ModeStatus};

/// An untolerated engine error (mirrors dss-python raising on `Error.Number != 0`).
#[derive(Debug, Clone)]
pub enum EngineError {
    /// A DSS `DoSimpleMsg` error the caller's context does not tolerate.
    Dss {
        errno: i32,
        desc: String,
        ctx: String,
    },
    /// Any other bridge-level failure (bad DLL path, version mismatch, ...).
    Other(String),
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EngineError::Dss { errno, desc, ctx } => {
                write!(f, "DSS error #{errno} ({ctx}): {desc}")
            }
            EngineError::Other(s) => write!(f, "{s}"),
        }
    }
}

impl std::error::Error for EngineError {}

/// Powers, Currents, Losses (each flat `[re, im, ...]`) for one element.
pub type Pcl = (Vec<f64>, Vec<f64>, Vec<f64>);

/// What the executive `RelCalc` did (GOLDEN_REBASE G1.6(i)) — an *observable*
/// of the reliability surface, not a bridge detail: a zone with no overcurrent
/// device aborts the calculation on every engine, and the gate compares the
/// abort (and its text) across the three of them.
#[derive(Debug, Clone)]
pub struct RelCalcResult {
    /// The command hit the tolerated `52902` abort
    /// (`Meters/EnergyMeter.pas:2502`).
    pub aborted: bool,
    /// The abort's message, verbatim from `ErrorDesc`; empty when it did not
    /// abort.
    pub message: String,
}

/// `CurrentsMagAng`, `Residuals`, `VoltagesMagAng` (each the flat
/// `[mag, ang, ...]` array the DDLL writes, angles in degrees on the
/// `(-180, 180]` branch cut) for one **enabled** element — the GOLDEN_REBASE
/// G1.3a derived capture ([`Engine::element_polar`]).
pub type Polar3 = (Vec<f64>, Vec<f64>, Vec<f64>);

/// `SeqPowers` (flat complex `[re, im, …]`, kW + j·kvar), `SeqCurrents` and
/// `SeqVoltages` (magnitudes only, A and V) for one **enabled** element — the
/// GOLDEN_REBASE G1.3b sequence capture ([`Engine::element_seq`]), in that
/// read order. All three are `3 * NTerms` values long (the powers therefore
/// `6 * NTerms` doubles on the wire), or `0` for a 0-terminal element.
pub type Seq3 = (Vec<f64>, Vec<f64>, Vec<f64>);

/// The nine unconditional discrete per-element scalars of the GOLDEN_REBASE
/// G1.3d capture ([`Engine::element_extras`]): the four shape/name reads of
/// part (i) and the five control-derived reads of part (ii). A named struct
/// rather than a nine-tuple — at that width a positional read is a bug waiting
/// for the next field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extras {
    /// `CktElementI(0)` `NTerms` — `DDLL/DCktElement.pas:139`.
    pub n_terms: i32,
    /// `CktElementI(1)` `NConds` — `:144`.
    pub n_conds: i32,
    /// `CktElementI(2)` `NPhases` — `:149`.
    pub n_phases: i32,
    /// `CktElementS(4)` `EnergyMeter`, the **raw** DDLL string: `"0"` when the
    /// element has no meter (`CktElementS`'s pre-`case` default,
    /// `DDLL/DCktElement.pas:421`); normalizing that against the capi channel's
    /// `""` is the comparator's job, not the capture's.
    pub energy_meter: String,
    /// `CktElementI(9)` `ControlElementList.listSize` — `:237`. No `Enabled`
    /// filter: a disabled control still occupies its slot.
    pub num_controls: i32,
    /// `CktElementI(10)` — the **1-based** position of the first
    /// Fuse/Recloser/Relay in that list, `0` when it has none (`:242-258`).
    pub ocp_dev_index: i32,
    /// `CktElementI(11)` `GetOCPDeviceType` — `:259`,
    /// `Common/Utilities.pas:3165` (1 = Fuse, 2 = Recloser, 3 = Relay, 0 = none).
    pub ocp_dev_type: i32,
    /// `CktElementI(8)` — any `CapControl`/`RegControl` in the list (`:222`).
    pub has_volt_control: bool,
    /// `CktElementI(7)` — any `SwtControl` in the list (`:207`).
    pub has_switch_control: bool,
}

/// One generic C-API call request for [`Engine::ffi_dispatch`]. `kind` selects
/// the ABI shape (`"i"`/`"f"`/`"s"`/`"v"`); only the matching scalar
/// (`iarg`/`farg`[`/farg2`](FfiCall::farg2)/`sarg`) is used. `vset` (V only)
/// drives an array-SET mode.
#[derive(Debug, Clone, Default)]
pub struct FfiCall<'a> {
    pub family: &'a str,
    pub kind: &'a str,
    pub mode: i32,
    pub iarg: i32,
    pub farg: f64,
    /// The **second** double of a [`crate::families::TWO_DOUBLE_F`] family's `F`
    /// entry point (`CircuitF`/`CmathLibF` — `DCircuit.pas:27`,
    /// `DCmathLib.pas:5`). Ignored by every other family; `Default` = `0.0`.
    pub farg2: f64,
    pub sarg: &'a str,
    pub vset: Option<VData>,
}

/// The result of a generic [`Engine::ffi_dispatch`] call, by ABI shape.
#[derive(Debug, Clone)]
pub enum FfiOut {
    /// `XxxI` — integer scalar.
    I(i32),
    /// `XxxF` — float scalar.
    F(f64),
    /// `XxxS` — string.
    S(String),
    /// `XxxV` getter — a decoded, type-tagged array.
    V(VData),
    /// `XxxV` setter — the element count the DLL accepted (`mySize` out).
    VSet(i32),
}

/// Compile-time tolerated non-fatal errno the official engine solves through
/// (`oracle_server._TOLERATED_COMPILE_ERRNOS`): a `? Export monitor <undef>`.
const TOLERATED_COMPILE: &[i32] = &[250];
/// User-written-model errnos (`oracle_server._USER_MODEL_ERRNOS`), tolerated at
/// compile + every solve/read only when the case opts in via `warn_and_continue`.
///
/// The **one** Rust-side definition: [`crate::capture`] carried a hand-synced
/// twin from G1.9 until the 2026-09-05 lane merge folded it away (that
/// handoff's "dedup at merge"), so a change here now reaches both readers.
pub(crate) const USER_MODEL: &[i32] = &[567, 570, 1570];
/// The **only** errno the executive `RelCalc` may raise without failing the case
/// (`oracle_server._RELCALC_TOLERATED_ERRNOS`, the same narrow scope on the capi
/// transport): `52902` "No Overcurrent Protection device (Relay, Recloser, or
/// Fuse) defined. Aborting Reliability calc." — `Meters/EnergyMeter.pas:2502`,
/// raised per meter whose zone holds no OCP device. Deliberately *not* a member
/// of [`TOLERATED_COMPILE`]: the tolerance exists for one command only.
const RELCALC_TOLERATED: &[i32] = &[52902];

/// The engine handle: the DLL entry points plus its self-reported version.
///
/// Loaded and driven on a single thread. The DLL is **never `FreeLibrary`'d**:
/// the r4133 DLL's unit finalization tears down its solver actor thread through a
/// Delphi message-pump `TThread.WaitFor` that deadlocks in a headless process (no
/// VCL `Application` / `WakeMainThread`), so unloading it hangs (diagnosed by
/// minidump — the process parked in `NtUserMsgWaitForMultipleObjectsEx` inside
/// `OpenDSSDirect.dll` at exit; the Python host dodged it because interpreter
/// shutdown does not `FreeLibrary` the Oddie DLL either). Leaking the library is
/// correct for a test-only worker: it stays loaded until the process exits, which
/// the OS reclaims cleanly.
pub struct Engine {
    dll: DllFns,
    ymatrix: YMatrixFns,
    families: FamilyTable,
    version: String,
    dll_path: String,
}

impl Engine {
    /// Load the r4133 DLL (leaking its `Library` handle — see [`Engine`]) and run
    /// the init sequence (`UNIFIED_GATE_PLAN.md` §2.2): `DSSI(8,0)` disable forms
    /// → read Version → `Set RegistryUpdate=No` →
    /// `Set DefaultBaseFrequency=60`. All subsequent DLL calls happen
    /// on this same thread.
    ///
    /// # `Set RegistryUpdate=No` — the process-global registry channel (D13)
    ///
    /// r4133 keeps `DefaultBaseFreq` in `HKCU\Software\OpenDSS`, section
    /// `MainSect`, value `BaseFrequency`: the key is
    /// `TIniRegSave.Create('\Software\' + ProgramName)` with
    /// `ProgramName := 'OpenDSS'` (`Common/DSSGlobals.pas:2093`, `:2078`;
    /// `Shared/IniRegSave.pas:63-71`). `ReadDSS_Registry` loads it
    /// (`Common/DSSGlobals.pas:1005`) from `TExecutive.Create`
    /// (`Executive/Executive.pas:124`) — i.e. once, at DLL load, before this
    /// bridge can issue anything — and `WriteDSS_Registry` writes it back
    /// (`Common/DSSGlobals.pas:1022`) from `TExecutive.Destroy`
    /// (`Executive/Executive.pas:141`), reached through the unit `Finalization`
    /// (`Common/DSSGlobals.pas:2159` → `LocalFinalization`) at process exit. The
    /// write is guarded by `UpdateRegistry` (`Common/DSSGlobals.pas:1015`), which
    /// defaults to `TRUE` (`:2131`) and is the `RegistryUpdate` option 102
    /// (`Executive/ExecOptions.pas:146`), settable with no circuit active
    /// (`:574`, inside `DoSetCmd_NoCircuit` `:545`, dispatched by
    /// `Executive/ExecCommands.pas:641-644`).
    ///
    /// So without this command every worker process leaks its last
    /// `DefaultBaseFreq` into the machine-wide registry and the next worker
    /// process — in any worktree — reads it back as its startup default. Measured
    /// against this DLL (2026-09-04): a worker that runs
    /// `Set DefaultBaseFrequency=37` and exits leaves `BaseFrequency = 37` in the
    /// key; with this command issued first the key does not move.
    ///
    /// # `Set DefaultBaseFrequency=60` — the init reset (D13)
    ///
    /// `Set RegistryUpdate=No` stops this process from *writing* the key; it
    /// cannot undo the *read*, which has already happened. `ReadDSS_Registry`
    /// assigns `DefaultBaseFreq := StrToInt(DSS_Registry.ReadString(
    /// 'BaseFrequency', '60'))` (`Common/DSSGlobals.pas:1005`) from
    /// `TExecutive.Create` (`Executive/Executive.pas:124`), i.e. at DLL load,
    /// before this bridge can issue anything — so a session that starts while
    /// the key holds `50` (left there by any pre-fix worker on this machine)
    /// begins at 50 Hz. [`Engine::clear`] covers every gate path (`run_case`
    /// clears before it compiles), but a bare probe session that only issues
    /// `exec` never clears and would inherit the registry value, so the reset
    /// is issued once here as well. Both land the bridge on the port's own
    /// starting point: `Dss::new()` sets `default_base_freq: 60.0`
    /// (`crates/dss-core/src/exec/construct.rs:173`).
    ///
    /// `Get RegistryUpdate` cannot be used to observe the flag: r4133's
    /// `DoGetCmd` case 102 assigns `UpdateRegistry := InterpretYesNo(Param)`
    /// instead of appending a result (`Executive/ExecOptions.pas:1314`), so it
    /// returns the empty string *and* clobbers the flag. The pin is the registry
    /// value itself (`tests/protocol.rs`).
    ///
    /// The capi channel needs no counterpart: dss_capi keeps `DefaultBaseFreq`
    /// per `TDSSContext` with no registry at all
    /// (`.inputs/dss_capi/src/Common/DSSClass.pas:1278`) and rejects option 102
    /// outright (`.inputs/dss_capi/src/Executive/ExecOptions.pas:259-260`,
    /// `DoSimpleMsg(... 302)`).
    pub fn new(dll_path: &Path) -> Result<Engine, EngineError> {
        let dll = Dll::load(dll_path).map_err(EngineError::Other)?;
        // Never `FreeLibrary`: dropping the DLL deadlocks in its finalization
        // (see the `Engine` doc). Leak the loaded `Library` for the session and
        // take its bound entry points (gate path + capability channels).
        let (fns, ymatrix, families) = dll.leak_into_parts();
        // SAFETY: `DSSI(8, 0)` sets NoFormsAllowed := TRUE (DDSS.pas mode 8).
        unsafe { (fns.dss_i)(8, 0) };
        // SAFETY: `DSSS(1, "")` returns the version string (borrowed, copied now).
        let vc = to_cstring("");
        let version = unsafe { cstr_to_string((fns.dss_s)(1, vc.as_ptr())) };
        let eng = Engine {
            dll: fns,
            ymatrix,
            families,
            version,
            dll_path: dll_path.display().to_string(),
        };
        // Clear any stray error from the setup commands.
        let _ = eng.poll_error();
        // D13: stop this process from persisting `DefaultBaseFreq` (and
        // `LastFile`/`DataPath`) into `HKCU\Software\OpenDSS` on exit — see the
        // fn doc. Strict: a DLL that does not accept the option must fail loudly,
        // never leave the registry channel open.
        eng.command_strict("Set RegistryUpdate=No", "init")?;
        // D13: the registry read already happened at DLL load
        // (`Common/DSSGlobals.pas:1005`), so start this session from the port's
        // own default (`crates/dss-core/src/exec/construct.rs:173`, 60 Hz)
        // instead of whatever the key held — a bare probe session that never
        // calls `clear` would otherwise inherit it. Strict for the same reason.
        eng.command_strict("Set DefaultBaseFrequency=60", "init")?;
        Ok(eng)
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn dll_path(&self) -> &str {
        &self.dll_path
    }

    // ---- command + error polling -----------------------------------------

    /// Run one executive command line and return the accumulated `GlobalResult`
    /// reply (exactly what `DSSPut_Command` returns == dss-python's
    /// `Text.Result`). Does not poll the error code — the caller decides
    /// tolerance.
    pub fn raw_command(&self, cmd: &str) -> String {
        let c = to_cstring(cmd);
        // SAFETY: `c` is a live NUL-terminated buffer for the duration of the
        // call; the returned pointer is DLL-owned and copied out immediately.
        let p = unsafe { (self.dll.dss_put_command)(c.as_ptr()) };
        unsafe { cstr_to_string(p) }
    }

    /// Read (and clear) the current error number + description. `ErrorCode()`
    /// resets `ErrorNumber` to 0 and `ErrorDesc()` resets the message, exactly
    /// like dss-python's per-command `_check_for_error`.
    pub fn poll_error(&self) -> (i32, String) {
        // SAFETY: both are zero-argument getters that read-and-reset globals.
        let errno = unsafe { (self.dll.error_code)() };
        let desc = if errno != 0 {
            unsafe { cstr_to_string((self.dll.error_desc)()) }
        } else {
            // Still clear the message buffer to match dss-python's behavior.
            let _ = unsafe { (self.dll.error_desc)() };
            String::new()
        };
        (errno, desc)
    }

    /// Run a command and escalate any non-zero error (no tolerance) — used for
    /// `clear` / `post` commands, which `oracle_server` does not wrap in a
    /// try/except (so any error raises).
    fn command_strict(&self, cmd: &str, ctx: &str) -> Result<String, EngineError> {
        let reply = self.raw_command(cmd);
        let (errno, desc) = self.poll_error();
        if errno != 0 {
            return Err(EngineError::Dss {
                errno,
                desc,
                ctx: format!("{ctx}: {cmd}"),
            });
        }
        Ok(reply)
    }

    /// Run a command tolerating `tolerated` errnos (cleared + logged), escalating
    /// anything else. Returns the reply.
    fn command_tolerating(
        &self,
        cmd: &str,
        tolerated: &[i32],
        ctx: &str,
    ) -> Result<String, EngineError> {
        self.command_tolerating_report(cmd, tolerated, ctx)
            .map(|(reply, _)| reply)
    }

    /// [`Engine::command_tolerating`], keeping the tolerated `(errno, desc)`
    /// instead of only logging it — for the caller whose *tolerated* error is
    /// itself a captured observable ([`Engine::relcalc`]).
    fn command_tolerating_report(
        &self,
        cmd: &str,
        tolerated: &[i32],
        ctx: &str,
    ) -> Result<(String, Option<(i32, String)>), EngineError> {
        let reply = self.raw_command(cmd);
        let (errno, desc) = self.poll_error();
        if errno != 0 && !tolerated.contains(&errno) {
            return Err(EngineError::Dss {
                errno,
                desc,
                ctx: format!("{ctx}: {cmd}"),
            });
        }
        if errno != 0 {
            eprintln!("epri-worker: tolerated non-fatal #{errno} on `{cmd}` ({ctx})");
            return Ok((reply, Some((errno, desc))));
        }
        Ok((reply, None))
    }

    /// Escalate a lingering error after a capture read (no tolerance). A clean
    /// case never trips this; a real read failure surfaces instead of silently
    /// producing garbage (mirrors dss-python raising on any read error).
    fn check_read(&self, ctx: &str) -> Result<(), EngineError> {
        let (errno, desc) = self.poll_error();
        if errno != 0 {
            return Err(EngineError::Dss {
                errno,
                desc,
                ctx: ctx.to_string(),
            });
        }
        Ok(())
    }

    // ---- compile / solve --------------------------------------------------

    /// `clear` + the per-case `DefaultBaseFreq` reset (D13).
    ///
    /// r4133's `clear` (`Executive/ExecHelper.pas:987-995` → `TExecutive.Clear`,
    /// `Executive/Executive.pas:234-275`) resets `DefaultEarthModel`,
    /// `LogQueries` and `MaxAllocationIterations` but **not** `DefaultBaseFreq`:
    /// the only assignments to it are `Set DefaultBaseFrequency`
    /// (`Executive/ExecOptions.pas:573` with no circuit, `:829` with one), the
    /// registry read at DLL load (see [`Engine::new`]) and the unit
    /// initialization's `DefaultBaseFreq := 60.0`
    /// (`Common/DSSGlobals.pas:2055`), which runs once per DLL load. So a deck
    /// that sets 50 Hz leaks it into every later deck this process compiles —
    /// `TDSSCircuit.Create` takes `Fundamental := DefaultBaseFreq`
    /// (`Common/Circuit.pas:416`) and the default `Vsource` follows.
    ///
    /// Measured against this DLL (2026-09-04): `Set DefaultBaseFrequency=50` →
    /// `clear` → `new circuit.…` reports `Get DefaultBaseFrequency` = 50 and
    /// `? Vsource.source.frequency` = 50.
    ///
    /// The port starts every case from a fresh `Dss::new()` whose
    /// `default_base_freq` is `60.0`
    /// (`crates/dss-core/src/exec/construct.rs:173`), so the bridge restores that
    /// same starting point after every `clear`. A deck that wants 50 Hz still
    /// gets it: the reset precedes the `Compile`.
    pub fn clear(&self) -> Result<(), EngineError> {
        self.command_strict("clear", "clear")?;
        self.command_strict("Set DefaultBaseFrequency=60", "clear")?;
        Ok(())
    }

    /// Compile a deck; tolerate `{250}` (+ user-model errnos when `warn`).
    pub fn compile(&self, case_path: &str, warn: bool) -> Result<(), EngineError> {
        let mut tol: Vec<i32> = TOLERATED_COMPILE.to_vec();
        if warn {
            tol.extend_from_slice(USER_MODEL);
        }
        self.command_tolerating(&format!("Compile \"{case_path}\""), &tol, "compile")
            .map(|_| ())
    }

    pub fn post(&self, cmd: &str) -> Result<(), EngineError> {
        self.command_strict(cmd, "post").map(|_| ())
    }

    /// Solve one step; tolerate user-model errnos when `warn`. Returns the solve
    /// reply (the `GlobalResult`, used for `global_result` capture).
    ///
    /// The DDLL `solve` dispatches the work to the actor thread and returns
    /// *before* it finishes (`TSolutionObj.Solve` = `Send_Message(SIMULATE)`;
    /// Solution.pas). Reading results — especially `y_csc`'s KLU re-factorization
    /// — while the actor is still solving deadlocks on the shared Y matrix. So
    /// block on the actor status until it reports done, exactly as dss-python's
    /// synchronous `Solution_Solve` does (this is *the* fix for the r4133-from-Rust
    /// hang; the Python host never hit it only because per-call overhead let the
    /// actor finish first).
    pub fn solve(&self, warn: bool) -> Result<String, EngineError> {
        let tol: &[i32] = if warn { USER_MODEL } else { &[] };
        let reply = self.command_tolerating("solve", tol, "solve")?;
        self.wait_for_actor()?;
        Ok(reply)
    }

    /// Run one executive command exactly like the retired Oddie bridge's
    /// `Text.Command` setter (the golden-regen / probe parity surface): escalate
    /// any non-zero errno (dss-python raises per command), then block until the
    /// solver actor is idle — a command may dispatch an asynchronous solve
    /// (`solve`, a `Compile` of a deck with an inline solve tail), and any
    /// following read must not race it (see [`Engine::solve`]).
    pub fn exec_wait(&self, cmd: &str) -> Result<String, EngineError> {
        let reply = self.command_strict(cmd, "exec")?;
        self.wait_for_actor()?;
        Ok(reply)
    }

    /// Run the executive `RelCalc` — the reliability half of the model
    /// (GOLDEN_REBASE G1.6(i); the capi transport issues the same command at the
    /// same point of the step, `oracle_server.run_case`).
    ///
    /// `RelCalc` is `ExecCommand[100]` -> `DoLambdaCalcs`
    /// (`Executive/ExecCommands.pas:154`, `:869`; `Executive/ExecHelper.pas:4404`):
    /// it zeroes every bus's `BusFltRate`/`Bus_Num_Interrupt` and then runs
    /// `TEnergyMeterObj.CalcReliabilityIndices` over `EnergyMeters.First`/`Next`
    /// (`ExecHelper.pas:4417`, `:4439-4441`) — so, like `Meters.Totals`, it leaves
    /// meter pointer-list cursor past the last meter and any following walk must
    /// start from `Meters.First`.
    ///
    /// It is **not idempotent** (`Bus.TotalMiles` accumulates), which is why the
    /// gate runs it exactly once per case, on the last step.
    ///
    /// Tolerates errno [`RELCALC_TOLERATED`] **only**, and *reports* it: the
    /// abort is compared across engines. Everything else still fails the case —
    /// notably `28724` "No EnergyMeter Objects Defined"
    /// (`ExecHelper.pas:4418-4421`), which is exactly what a
    /// `compare_reliability` flag on a meterless deck would produce and must
    /// never pass silently.
    ///
    /// Blocks on the solver actor afterwards for the same reason
    /// [`Engine::exec_wait`] does: the command dispatches no solve of its own,
    /// but no capture may race a busy actor.
    pub fn relcalc(&self) -> Result<RelCalcResult, EngineError> {
        let (_reply, tolerated) =
            self.command_tolerating_report("RelCalc", RELCALC_TOLERATED, "relcalc")?;
        self.wait_for_actor()?;
        Ok(match tolerated {
            Some((_errno, desc)) => RelCalcResult {
                aborted: true,
                message: desc,
            },
            None => RelCalcResult {
                aborted: false,
                message: String::new(),
            },
        })
    }

    /// `ParallelV(1)` = `ActorStatus[]` (0 = busy, 1 = done).
    fn actor_status(&self) -> Vec<i32> {
        self.v_i32s(self.dll.parallel_v, 1)
    }

    /// Block until every actor reports done (status != 0). Bounded by
    /// `DSS_EPRI_ACTOR_TIMEOUT_SECS` (default 300) so a wedged solve fails the
    /// case instead of hanging the worker forever.
    fn wait_for_actor(&self) -> Result<(), EngineError> {
        let deadline = std::time::Instant::now()
            + std::time::Duration::from_secs(
                std::env::var("DSS_EPRI_ACTOR_TIMEOUT_SECS")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(300),
            );
        loop {
            let status = self.actor_status();
            // All actors done (non-empty and no zero/busy entry).
            if !status.is_empty() && status.iter().all(|&s| s != 0) {
                return Ok(());
            }
            if std::time::Instant::now() >= deadline {
                return Err(EngineError::Other(
                    "actor did not finish solving within the deadline".to_string(),
                ));
            }
            std::thread::sleep(std::time::Duration::from_micros(200));
        }
    }

    // ---- V-protocol decode ------------------------------------------------

    /// Call a V getter for `mode` and copy the returned byte buffer out (the DLL
    /// hands back a borrowed pointer + type tag + byte size). Alignment-safe: the
    /// bytes are copied through a `u8` view (align 1), then reinterpreted.
    fn call_v(&self, f: FnV, mode: i32) -> (i32, Vec<u8>) {
        let mut ptr: *mut c_void = std::ptr::null_mut();
        let mut ty: i32 = 0;
        let mut size: i32 = 0;
        // SAFETY: out-params are valid locals; the getter sets `ptr` to DLL-owned
        // heap and `size` to its byte length (Pascal always sets `mySize` in
        // bytes). We copy exactly `size` bytes before any further DLL call.
        unsafe { f(mode, &mut ptr, &mut ty, &mut size) };
        if ptr.is_null() || size <= 0 {
            return (ty, Vec::new());
        }
        let len = size as usize;
        // SAFETY: `ptr` points to `len` valid bytes of DLL heap; `u8` has align 1,
        // so the slice is well-formed regardless of the element alignment.
        let bytes = unsafe { std::slice::from_raw_parts(ptr as *const u8, len) }.to_vec();
        (ty, bytes)
    }

    fn v_f64s(&self, f: FnV, mode: i32) -> Vec<f64> {
        let (_ty, bytes) = self.call_v(f, mode);
        bytes
            .chunks_exact(8)
            .map(|c| f64::from_le_bytes(c.try_into().unwrap()))
            .collect()
    }

    fn v_i32s(&self, f: FnV, mode: i32) -> Vec<i32> {
        let (_ty, bytes) = self.call_v(f, mode);
        bytes
            .chunks_exact(4)
            .map(|c| i32::from_le_bytes(c.try_into().unwrap()))
            .collect()
    }

    fn v_strings(&self, f: FnV, mode: i32) -> Vec<String> {
        let (_ty, bytes) = self.call_v(f, mode);
        decode_string_array(&bytes)
    }

    fn call_s(&self, f: crate::ffi::FnS, mode: i32) -> String {
        let empty = to_cstring("");
        // SAFETY: `empty` is a live NUL-terminated buffer; result copied now.
        let p = unsafe { f(mode, empty.as_ptr()) };
        unsafe { cstr_to_string(p) }
    }

    fn set_s(&self, f: crate::ffi::FnS, mode: i32, arg: &str) {
        let a = to_cstring(arg);
        // SAFETY: `a` is a live NUL-terminated buffer for the duration of call.
        let _ = unsafe { f(mode, a.as_ptr()) };
    }

    fn call_s_arg(&self, f: crate::ffi::FnS, mode: i32, arg: &str) -> String {
        let a = to_cstring(arg);
        // SAFETY: `a` is a live NUL-terminated buffer; the returned pointer is
        // DLL-owned and copied out immediately.
        let p = unsafe { f(mode, a.as_ptr()) };
        unsafe { cstr_to_string(p) }
    }

    fn call_i(&self, f: crate::ffi::FnI, mode: i32, arg: i32) -> i32 {
        // SAFETY: plain integer scalar entry point.
        unsafe { f(mode, arg) }
    }

    fn call_f(&self, f: crate::ffi::FnF, mode: i32, arg: f64) -> f64 {
        // SAFETY: plain float scalar entry point.
        unsafe { f(mode, arg) }
    }

    // ---- circuit-level accessors -----------------------------------------

    pub fn num_nodes(&self) -> i32 {
        self.call_i(self.dll.circuit_i, 2, 0)
    }

    pub fn ynode_order(&self) -> Vec<String> {
        self.v_strings(self.dll.circuit_v, 19)
    }

    /// Flat `[re, im, ...]` node voltages, nodes 1..NumNodes (ground excluded —
    /// `CircuitV(18)` points at `NodeV[1]`, length `NumNodes` complex).
    pub fn ynode_varray(&self) -> Vec<f64> {
        self.v_f64s(self.dll.circuit_v, 18)
    }

    pub fn all_element_names(&self) -> Vec<String> {
        self.v_strings(self.dll.circuit_v, 6)
    }

    pub fn set_active_element(&self, name: &str) {
        self.set_s(self.dll.circuit_s, 3, name);
    }

    /// Active circuit name (`CircuitS(0)`).
    pub fn circuit_name(&self) -> String {
        self.call_s(self.dll.circuit_s, 0)
    }

    /// `Circuit.SetActiveBus` (`CircuitS(4)`, `DCircuit.pas`): activate a bus by
    /// name and return its 0-based index, or `-1` if not found / no circuit.
    pub fn set_active_bus(&self, name: &str) -> i32 {
        self.call_s_arg(self.dll.circuit_s, 4, name)
            .trim()
            .parse()
            .unwrap_or(-1)
    }

    /// `Bus.kVBase` (`BUSF(0)`, `DBus.pas`) of the active bus (select it first
    /// via [`Engine::set_active_bus`]).
    pub fn bus_kvbase(&self) -> f64 {
        self.call_f(self.dll.bus_f, 0, 0.0)
    }

    // ---- element accessors (active element) ------------------------------

    /// Flat YPrim `[re, im, ...]` (column-major, `2*yorder^2` floats) or a short
    /// stub for a non-YPrim element (control/meter) — the caller filters by
    /// `2*n*n == len`.
    pub fn element_yprim(&self) -> Vec<f64> {
        self.v_f64s(self.dll.ckt_element_v, 12)
    }

    /// Terminal currents, flat `[re, im, ...]`. See
    /// [`modes::CKT_ELEMENT_CURRENTS`] — D3 capture group **B**.
    pub fn element_currents(&self) -> Vec<f64> {
        self.v_f64s(self.dll.ckt_element_v, modes::CKT_ELEMENT_CURRENTS.mode)
    }

    /// Per-conductor powers, flat `[kW, kvar, ...]`. See
    /// [`modes::CKT_ELEMENT_POWERS`] — D3 capture group **A**.
    pub fn element_powers(&self) -> Vec<f64> {
        self.v_f64s(self.dll.ckt_element_v, modes::CKT_ELEMENT_POWERS.mode)
    }

    /// Total losses `[re, im]` in W/var (`CktElement.Losses`, one complex). See
    /// [`modes::CKT_ELEMENT_LOSSES`] — D3 capture group **A**.
    pub fn element_losses(&self) -> Vec<f64> {
        self.v_f64s(self.dll.ckt_element_v, modes::CKT_ELEMENT_LOSSES.mode)
    }

    pub fn element_variable_names(&self) -> Vec<String> {
        self.v_strings(self.dll.ckt_element_v, 15)
    }

    /// The active DSS object's `ParentClass.AllPropertyNames`, in property-index
    /// order (`DSSElementV` mode 0 — `DDSSElement.pas`). Read after activating the
    /// object with `? name.Like`; empty class ⇒ `["None"]` (Pascal placeholder).
    /// Backs [`crate::capture`]'s all-properties enumeration (§2.2), which gates
    /// the r4133 property compare since R4133_PROPS RP4.1 (2026-09-03).
    pub fn element_all_property_names(&self) -> Vec<String> {
        self.v_strings(self.dll.dss_element_v, 0)
    }

    pub fn element_variable_values(&self) -> Vec<f64> {
        self.v_f64s(self.dll.ckt_element_v, 16)
    }

    /// Read `PhaseLosses` on the active element — the **first** read of the
    /// GOLDEN_REBASE G1.3d(ii) element capture, ahead of
    /// [`Engine::element_pcl`]'s own group-A pair.
    ///
    /// `CktElementV(6)` (`DDLL/DCktElement.pas:637-658`) is
    /// `TDSSCktElement.GetPhaseLosses` (`Common/CktElement.pas:1075-1116`),
    /// whose first act on an enabled element is `ComputeIterminal` (`:1090`):
    /// a cache-aware **group-A** read ([`modes::CKT_ELEMENT_PHASE_LOSSES`]), so
    /// the §1.1(a)/D3 order puts it before every read that fills a scratch
    /// buffer through `GetCurrents`.
    ///
    /// Returned flat `[re, im, …]`, `NPhases` complex, in **kW/kvar**: the arm
    /// scales every value by `0.001` at `:651` (capi does the same at
    /// `CAPI/CAPI_Alt.pas:464`), unlike `Losses`, which is W/var. A disabled
    /// element yields `NPhases` zeros (`Common/CktElement.pas:1118-1119`) and a
    /// 0-phase element (`UPFCControl`, which never assigns `Nterms` —
    /// `Controls/UPFCControl.pas:230-246`) an empty array, neither touching
    /// `NodeRef` — so, unlike `NodeOrder`, this read needs no enabled/terminal
    /// predicate and the two channels' shapes already agree (measured).
    ///
    /// Carries the same single user-model retry as [`Engine::element_pcl`]:
    /// with `element_extras` on, this is the read that fires — and clears — a
    /// Generator model=6's #567 priming warning.
    pub fn element_phase_losses(&self, warn: bool, ctx: &str) -> Result<Vec<f64>, EngineError> {
        for attempt in 0..2 {
            let pl = self.ckt_element_phase_losses()?; // capture-order: PhaseLosses (A)
            let (errno, desc) = self.poll_error();
            if errno == 0 {
                return Ok(pl);
            }
            if warn && USER_MODEL.contains(&errno) && attempt == 0 {
                continue; // priming read fired + cleared the warning; retry once
            }
            return Err(EngineError::Dss {
                errno,
                desc,
                ctx: ctx.to_string(),
            });
        }
        unreachable!()
    }

    /// Read **Losses, then Powers, then Currents** on the active element — the
    /// §1.1(a)/D3 capture order (`GOLDEN_REBASE_PLAN.md`, coordinator decision
    /// D3): both cache-aware group-A reads run before the group-B `Currents`,
    /// which fills a *scratch* buffer while still stamping the element's
    /// `IterminalSolutionCount` (`PCElements/PCElement.pas:247-266`) and can
    /// therefore starve a cache-aware read that follows it. The name and the
    /// returned tuple keep their `(powers, currents, losses)` shape — only the
    /// read order moved (GOLDEN_REBASE G1.3a; both reordered reads are group A,
    /// so no captured value can change, proven byte-for-byte on IEEE13, two
    /// harmonics decks and the two user-model decks).
    ///
    /// One user-model retry (`oracle_server.capture_all_elements`'s `_read`): a
    /// Generator model=6 fires #567 on the first current recompute after a
    /// solve — under this order that is the `Losses` read rather than `Powers`
    /// — which the recompute itself clears, so a retry returns the cached,
    /// correct Yprim-only values.
    pub fn element_pcl(&self, warn: bool, ctx: &str) -> Result<Pcl, EngineError> {
        for attempt in 0..2 {
            let losses = self.element_losses(); // capture-order: Losses (A)
            let powers = self.element_powers(); // capture-order: Powers (A)
            let currents = self.element_currents(); // capture-order: Currents (B)
            let (errno, desc) = self.poll_error();
            if errno == 0 {
                return Ok((powers, currents, losses));
            }
            if warn && USER_MODEL.contains(&errno) && attempt == 0 {
                continue; // priming read fired + cleared the warning; retry once
            }
            return Err(EngineError::Dss {
                errno,
                desc,
                ctx: ctx.to_string(),
            });
        }
        unreachable!()
    }

    /// Read `CurrentsMagAng`, `Residuals` and `VoltagesMagAng` on the active
    /// element — the GOLDEN_REBASE G1.3a derived capture, in D3 order (the two
    /// group-B reads first; `VoltagesMagAng` is group C, reading
    /// `NodeV[NodeRef[i]]` only) and with the same single user-model retry as
    /// [`Engine::element_pcl`], so an errno is attributed to its own element.
    ///
    /// **Only ever called on an `Enabled` element**
    /// ([`Engine::ckt_element_enabled`], checked at the one call site
    /// `crate::capture::capture_all_elements`): `CktElementV(19)` dereferences
    /// `NodeRef^[i]` with no nil guard (`DCktElement.pas:1099`, where capi has
    /// one at `CAPI/CAPI_Alt.pas:1081`) and **kills the process** on a
    /// never-enabled element — measured on `controls/fuse/midi_fuse.dss`'s
    /// `Line.tie`. Capturing enabled elements only removes that crash class and
    /// makes the two oracle channels' shapes identical, so no sentinel
    /// normalization is owed.
    pub fn element_polar(&self, warn: bool, ctx: &str) -> Result<Polar3, EngineError> {
        for attempt in 0..2 {
            // capture-order: CurrentsMagAng (B)
            let cma = self.ckt_element_currents_mag_ang()?;
            let res = self.ckt_element_residuals()?; // capture-order: Residuals (B)
            // capture-order: VoltagesMagAng (C)
            let vma = self.ckt_element_voltages_mag_ang()?;
            let (errno, desc) = self.poll_error();
            if errno == 0 {
                return Ok((cma, res, vma));
            }
            if warn && USER_MODEL.contains(&errno) && attempt == 0 {
                continue; // priming read fired + cleared the warning; retry once
            }
            return Err(EngineError::Dss {
                errno,
                desc,
                ctx: ctx.to_string(),
            });
        }
        unreachable!()
    }

    /// Read `SeqPowers`, `SeqCurrents` and `SeqVoltages` on the active element
    /// — the GOLDEN_REBASE G1.3b sequence capture, in D3 order (`SeqPowers`
    /// and `SeqCurrents` both call `GetCurrents` into a scratch buffer and are
    /// group **B** — [`modes::CKT_ELEMENT_SEQ_POWERS`] `:739`,
    /// [`modes::CKT_ELEMENT_SEQ_CURRENTS`] `:700`; `SeqVoltages` reads
    /// `Solution.NodeV` only and is group **C**,
    /// [`modes::CKT_ELEMENT_SEQ_VOLTAGES`] `:660`) and with the same single
    /// user-model retry as [`Engine::element_polar`], so an errno is
    /// attributed to its own element.
    ///
    /// The two magnitude reads return `Cabs` of the 012 components
    /// (`DCktElement.pas:719` / `:680`, capi `CAPI/CAPI_Alt.pas:490` / `:620`);
    /// `SeqPowers` is complex `[re, im, …]` in **kW/kvar**, both engines
    /// scaling by `0.003` inside the arm (`:767` and `:788`, capi `:561` and
    /// `:588-589`) — a fixed 3-phase kVA conversion, not the
    /// `PositiveSequence` ×3 that `Powers` applies.
    ///
    /// **Only ever called on an `Enabled` element**
    /// ([`Engine::ckt_element_enabled`], checked at the one call site
    /// `crate::capture::capture_all_elements`), and here the rule is
    /// load-bearing rather than merely shape-normalizing: `CktElementV(9)` has
    /// neither an `Enabled` nor a `NodeRef` guard (`DCktElement.pas:739-797`)
    /// and dereferences `NodeRef^[k+1]` at `:765` on a never-enabled element,
    /// where the two magnitude modes do guard (`If Enabled` at `:711` / `:671`).
    /// capi is no safer on that read: `Alt_CE_Get_SeqPowers` skips the
    /// `Enabled` test (`CAPI/CAPI_Alt.pas:604`, commented out) and resizes the
    /// result to `3 * NTerms` complex slots at `:608` before its helper exits
    /// on `(not Enabled) or (NodeRef = NIL)` at `:544`, returning
    /// uninitialized memory.
    ///
    /// A 0-terminal element (`UPFCControl` never assigns `Nterms` —
    /// `Controls/UPFCControl.pas:230-246`) is legitimate and answers `[]` on
    /// all three modes here, where capi returns its 1-element `DefaultResult`
    /// for `SeqVoltages` and `SeqPowers`; that shape difference is the
    /// comparator's business, not the capture's.
    pub fn element_seq(&self, warn: bool, ctx: &str) -> Result<Seq3, EngineError> {
        for attempt in 0..2 {
            let seq_p = self.ckt_element_seq_powers()?; // capture-order: SeqPowers (B)
            // capture-order: SeqCurrents (B)
            let seq_i = self.ckt_element_seq_currents()?;
            // capture-order: SeqVoltages (C)
            let seq_v = self.ckt_element_seq_voltages()?;
            let (errno, desc) = self.poll_error();
            if errno == 0 {
                return Ok((seq_p, seq_i, seq_v));
            }
            if warn && USER_MODEL.contains(&errno) && attempt == 0 {
                continue; // priming read fired + cleared the warning; retry once
            }
            return Err(EngineError::Dss {
                errno,
                desc,
                ctx: ctx.to_string(),
            });
        }
        unreachable!()
    }

    /// Read the nine unconditional discrete scalars of the GOLDEN_REBASE
    /// G1.3d capture on the active element: `NumTerminals`, `NumConductors`,
    /// `NumPhases` and `EnergyMeter` (part (i) — `CktElementI(0)`/`(1)`/`(2)`
    /// at `DDLL/DCktElement.pas:139`/`:144`/`:149`, `CktElementS(4)` at `:442`;
    /// capi `CAPI/CAPI_CktElement.pas:182-211` and `:672-687`), then
    /// `NumControls`, `OCPDevIndex`, `OCPDevType`, `HasVoltControl` and
    /// `HasSwitchControl` (part (ii) — `CktElementI(9)`/`(10)`/`(11)`/`(8)`/`(7)`
    /// at `:237`/`:242`/`:259`/`:222`/`:207`; capi
    /// `CAPI/CAPI_CktElement.pas:939`/`:951`/`:978`/`:689`/`:713`).
    ///
    /// The five part-(ii) reads all answer from the element's
    /// `ControlElementList` (`Common/CktElement.pas:100`), which
    /// `TControlElem.Set_ControlledElement` maintains
    /// (`Controls/ControlElem.pas:113-131`), and none of them touches
    /// `Iterminal`. Neither `NumControls` nor the OCP scan filters on
    /// `Enabled`: a disabled control keeps its slot and still wins
    /// `GetOCPDeviceType` (`Common/Utilities.pas:3165-3184` has no `Enabled`
    /// test) — measured on both channels, and the reason the port recomputes
    /// these live rather than reading its registration-time latch.
    ///
    /// All nine modes are group **C**: seven are `ModeEffect::Pure`, and the two
    /// `Has*` are `ModeEffect::Impure` only because their `First`/`Next` walk
    /// leaves that list's cursor moved (`:209-218`, `:224-233`) — unobservable,
    /// since every consumer restarts with `First`/`Get(i)`. So this helper may
    /// sit anywhere in the capture order and — unlike [`Engine::element_pcl`] /
    /// [`Engine::element_polar`] / [`Engine::element_phase_losses`] — takes no
    /// `warn` flag: none of these reads can fire the user-model priming warning
    /// those absorb. The error slot is still drained here
    /// ([`Engine::assert_clean`]) so an errno is attributed to its own element
    /// rather than to the next one's `element_pcl`.
    ///
    /// `NodeOrder` is deliberately **not** part of this helper: it is read
    /// conditionally (`Enabled` and `NumTerminals > 0`, see
    /// `crate::capture::capture_all_elements`), and a conditional read inside an
    /// unconditional helper would make the helper's declared capture-order
    /// sequence a lie.
    pub fn element_extras(&self, ctx: &str) -> Result<Extras, EngineError> {
        let n_terms = self.ckt_element_num_terminals()?; // capture-order: NumTerminals (C)
        let n_conds = self.ckt_element_num_conductors()?; // capture-order: NumConductors (C)
        let n_phases = self.ckt_element_num_phases()?; // capture-order: NumPhases (C)
        let energy_meter = self.ckt_element_energy_meter()?; // capture-order: EnergyMeter (C)
        let num_controls = self.ckt_element_num_controls()?; // capture-order: NumControls (C)
        let ocp_dev_index = self.ckt_element_ocp_dev_index()?; // capture-order: OCPDevIndex (C)
        let ocp_dev_type = self.ckt_element_ocp_dev_type()?; // capture-order: OCPDevType (C)
        // capture-order: HasVoltControl (C)
        let has_volt_control = self.ckt_element_has_volt_control()?;
        // capture-order: HasSwitchControl (C)
        let has_switch_control = self.ckt_element_has_switch_control()?;
        self.assert_clean(ctx)?;
        Ok(Extras {
            n_terms,
            n_conds,
            n_phases,
            energy_meter,
            num_controls,
            ocp_dev_index,
            ocp_dev_type,
            has_volt_control,
            has_switch_control,
        })
    }

    // ---- solution scalars -------------------------------------------------

    pub fn iterations(&self) -> i32 {
        self.call_i(self.dll.solution_i, 7, 0)
    }

    pub fn converged(&self) -> bool {
        self.call_i(self.dll.solution_i, 38, 0) != 0
    }

    pub fn dbl_hour(&self) -> f64 {
        self.call_f(self.dll.solution_f, 20, 0.0)
    }

    /// `Solution.EventLog` (`SolutionV(0)`, `DSolution.pas`): the raw
    /// `EventStrings` lines (`Hour=…, Sec=…, ControlIter=…, Element=…, Action=…`).
    ///
    /// Decode parity with the retired Oddie `sol.EventLog` (the committed
    /// protection goldens are the empirical anchor): only **NUL-terminated**
    /// segments count as strings. The Pascal writes each real event line followed
    /// by a `Char(0)`, but the empty-log placeholder is a bare `None` with **no**
    /// terminator (mode 0), which Oddie decoded to zero complete strings — `[]`,
    /// exactly what the `swt_manual` golden pins. (Deliberately NOT
    /// [`decode_string_array`], whose trailing-segment handling serves the gate's
    /// V-protocol arrays and must stay untouched.)
    pub fn eventlog(&self) -> Vec<String> {
        let (_ty, bytes) = self.call_v(self.dll.solution_v, 0);
        let mut out = Vec::new();
        let mut start = 0usize;
        for (i, &b) in bytes.iter().enumerate() {
            if b == 0 {
                out.push(String::from_utf8_lossy(&bytes[start..i]).into_owned());
                start = i + 1;
            }
        }
        out
    }

    // ---- system Y (CSC) + injection --------------------------------------

    /// Assembled, factored system Y as CSC: `(col_ptr[n+1], row_idx[nnz],
    /// vals[nnz] as (re, im))`. Mirrors dss-python's
    /// `YMatrix.GetCompressedYMatrix` (InitAndGetYparams factors first; proven
    /// solution-neutral by the smoke check).
    pub fn y_csc(&self) -> Result<Ycsc, EngineError> {
        let mut hy: u64 = 0;
        let mut n_bus: u32 = 0;
        let mut n_nz: u32 = 0;
        // SAFETY: out-params valid; the call factors Y and reports dimensions.
        let ok = unsafe { (self.dll.init_and_get_yparams)(&mut hy, &mut n_bus, &mut n_nz) };
        if ok == 0 || n_bus == 0 || n_nz == 0 {
            let (errno, desc) = self.poll_error();
            return Err(EngineError::Dss {
                errno,
                desc: if desc.is_empty() {
                    "InitAndGetYparams: Y not built".to_string()
                } else {
                    desc
                },
                ctx: "y_csc".to_string(),
            });
        }
        let mut col_ptr_p: *mut i32 = std::ptr::null_mut();
        let mut row_idx_p: *mut i32 = std::ptr::null_mut();
        let mut vals_p: *mut f64 = std::ptr::null_mut();
        // SAFETY: the getter sets the three out-pointers to DLL-owned heap of the
        // dimensions just returned (`col_ptr[n_bus+1]`, `row_idx[n_nz]`,
        // `vals[n_nz]` complex). Copied out immediately below.
        unsafe {
            (self.dll.get_compressed_y_matrix)(
                hy,
                n_bus,
                n_nz,
                &mut col_ptr_p,
                &mut row_idx_p,
                &mut vals_p,
            )
        };
        if col_ptr_p.is_null() || row_idx_p.is_null() || vals_p.is_null() {
            return Err(EngineError::Other(
                "GetCompressedYMatrix returned null".into(),
            ));
        }
        let n = n_bus as usize;
        let nnz = n_nz as usize;
        // SAFETY: pointers/lengths come straight from InitAndGetYparams; align 1
        // byte-copy avoids any alignment assumption.
        let col_ptr = copy_i32(col_ptr_p, n + 1);
        let row_idx = copy_i32(row_idx_p, nnz);
        let vals = copy_f64(vals_p, nnz * 2); // interleaved re/im
        Ok(Ycsc {
            n,
            col_ptr,
            row_idx,
            vals,
        })
    }

    /// Injection current vector (`getIpointer`), length `2*(NumNodes+1)` flat
    /// `[re, im, ...]`, slot 0 = ground.
    pub fn injection_raw(&self, num_nodes: i32) -> Vec<f64> {
        let mut p: *mut f64 = std::ptr::null_mut();
        // SAFETY: sets `p` to the internal current vector; length is a function of
        // NumNodes (getIpointer just hands back Solution.Currents).
        unsafe { (self.dll.get_i_pointer)(&mut p) };
        if p.is_null() {
            return Vec::new();
        }
        let len = 2 * (num_nodes as usize + 1);
        copy_f64(p, len)
    }

    // ---- monitors ---------------------------------------------------------

    pub fn monitors_first(&self) -> bool {
        self.call_i(self.dll.monitors_i, 0, 0) != 0
    }
    pub fn monitors_next(&self) -> bool {
        self.call_i(self.dll.monitors_i, 1, 0) != 0
    }
    pub fn monitor_name(&self) -> String {
        self.call_s(self.dll.monitors_s, 1)
    }
    /// `Monitors.Name` write (`MonitorsS(2)`): activate a monitor by name (an
    /// unknown name raises a DSS error — poll after).
    pub fn monitor_select(&self, name: &str) {
        self.set_s(self.dll.monitors_s, 2, name);
    }
    pub fn monitor_num_channels(&self) -> i32 {
        self.call_i(self.dll.monitors_i, 17, 0)
    }
    pub fn monitor_sample_count(&self) -> i32 {
        self.call_i(self.dll.monitors_i, 9, 0)
    }
    pub fn monitor_header(&self) -> Vec<String> {
        self.v_strings(self.dll.monitors_v, 2)
    }

    /// Decode monitor channel `index` (1-based) from the raw ByteStream, exactly
    /// like dss-python's `IMonitors.Channel`: a 272-byte header (`record_size =
    /// int32@offset8 + 2` = NumChannels + hour + sec), then f32 records; an empty
    /// stream (272 bytes) yields a single `0.0`.
    pub fn monitor_channel(&self, index: i32) -> Vec<f64> {
        let (_ty, bytes) = self.call_v(self.dll.monitors_v, 1);
        let cnt = bytes.len();
        if cnt == 272 {
            return vec![0.0];
        }
        if cnt < 272 + 4 {
            return Vec::new();
        }
        let record_size = i32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize + 2; // + hour + sec
        if record_size == 0 {
            return Vec::new();
        }
        let floats: Vec<f32> = bytes[272..]
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
            .collect();
        // data[(index + 1)::record_size]
        let start = (index + 1) as usize;
        floats
            .iter()
            .skip(start)
            .step_by(record_size)
            .map(|&x| x as f64)
            .collect()
    }

    // ---- meters -----------------------------------------------------------

    pub fn meters_first(&self) -> bool {
        self.call_i(self.dll.meters_i, 0, 0) != 0
    }
    pub fn meters_next(&self) -> bool {
        self.call_i(self.dll.meters_i, 1, 0) != 0
    }
    pub fn meter_name(&self) -> String {
        self.call_s(self.dll.meters_s, 0)
    }
    pub fn meter_register_names(&self) -> Vec<String> {
        self.v_strings(self.dll.meters_v, 1)
    }
    pub fn meter_register_values(&self) -> Vec<f64> {
        self.v_f64s(self.dll.meters_v, 2)
    }
    pub fn meter_all_end_elements(&self) -> Vec<String> {
        self.v_strings(self.dll.meters_v, 10)
    }
    pub fn meter_all_branches_in_zone(&self) -> Vec<String> {
        self.v_strings(self.dll.meters_v, 11)
    }
    pub fn meter_zone_pce(&self) -> Vec<String> {
        self.v_strings(self.dll.meters_v, 12)
    }

    // ---- discrete state (transformers / regcontrols / capacitors) --------

    pub fn transformers_first(&self) -> bool {
        self.call_i(self.dll.transformers_i, 8, 0) != 0
    }
    pub fn transformers_next(&self) -> bool {
        self.call_i(self.dll.transformers_i, 9, 0) != 0
    }
    pub fn transformer_name(&self) -> String {
        self.call_s(self.dll.transformers_s, 2)
    }
    pub fn transformer_num_windings(&self) -> i32 {
        self.call_i(self.dll.transformers_i, 0, 0)
    }
    pub fn transformer_set_wdg(&self, w: i32) {
        self.call_i(self.dll.transformers_i, 3, w);
    }
    pub fn transformer_tap(&self) -> f64 {
        self.call_f(self.dll.transformers_f, 2, 0.0)
    }

    pub fn regcontrols_first(&self) -> bool {
        self.call_i(self.dll.reg_controls_i, 0, 0) != 0
    }
    pub fn regcontrols_next(&self) -> bool {
        self.call_i(self.dll.reg_controls_i, 1, 0) != 0
    }
    pub fn regcontrol_name(&self) -> String {
        self.call_s(self.dll.reg_controls_s, 0)
    }
    pub fn regcontrol_tap_number(&self) -> i32 {
        self.call_i(self.dll.reg_controls_i, 13, 0)
    }

    pub fn capacitors_first(&self) -> bool {
        self.call_i(self.dll.capacitors_i, 4, 0) != 0
    }
    pub fn capacitors_next(&self) -> bool {
        self.call_i(self.dll.capacitors_i, 5, 0) != 0
    }
    pub fn capacitor_name(&self) -> String {
        self.call_s(self.dll.capacitors_s, 0)
    }
    pub fn capacitor_states(&self) -> Vec<i32> {
        self.v_i32s(self.dll.capacitors_v, 1)
    }

    // ---- ctrlqueue --------------------------------------------------------

    pub fn ctrl_queue(&self) -> Vec<String> {
        self.v_strings(self.dll.ctrl_queue_v, 0)
    }

    // ---- generic FFI capability channel (EPRI Round 2) --------------------
    //
    // Every DDLL family is a uniform `XxxI/F/S/V(mode, arg)` quartet; dispatching
    // `(family, kind, mode, arg)` reaches every mode of every family (see
    // `crate::families`). All FFI happens here (behind the module SAFETY doc);
    // `crate::script::handle_ffi` only parses/serializes JSON around it.

    /// Look up a family's entry points by (case-insensitive) name.
    pub fn family(&self, name: &str) -> Option<&Family> {
        self.families.get(name)
    }

    /// `(family, present-kinds)` pairs for the `caps` handshake.
    pub fn family_manifest(&self) -> Vec<(&'static str, String)> {
        self.families.manifest()
    }

    /// Number of registered families.
    pub fn family_count(&self) -> usize {
        self.families.family_count()
    }

    /// Total bound family entry points.
    pub fn family_entry_points(&self) -> usize {
        self.families.entry_point_count()
    }

    /// Ask whether this DLL revision serves `(family, kind, mode)`.
    ///
    /// The grouped DDLL API has no `GetProcAddress` miss to detect: an absent
    /// property falls through its family's `case` into an `else` branch that
    /// returns a **sentinel** ([`crate::modes`]). This drives the mode with a
    /// neutral argument (`0` / `0.0` / `""`, the array *getter* for `V`) and
    /// classifies the reply, so a capture never records `-1` or `"Error, …"` as
    /// data. It never panics.
    ///
    /// Refusals, all *before* any FFI:
    /// * a `(family, kind, mode)` on [`modes::DO_NOT_CALL`] returns
    ///   [`ModeStatus::DoNotCall`] — the DLL is not touched;
    /// * an `S` probe on a family with no measured sentinel
    ///   ([`modes::s_sentinel`]), or a `V` probe on one whose `else` branch
    ///   writes no sentinel at all ([`modes::V_WITHOUT_SENTINEL`]), is an
    ///   `Err` — reporting `Served` there would be a silent mask.
    ///
    /// **Point this at getter modes only.** A `mode` that is a *setter* in the
    /// DDLL `case` would be executed with the neutral argument, writing engine
    /// state; the classification cannot tell the two apart.
    ///
    /// The errno is drained afterwards (as [`crate::script::handle_ffi`] does)
    /// so a probe cannot poison the next [`Engine::assert_clean`]; it is not part
    /// of the verdict (measured 2026-09-04: every unknown-mode reply left
    /// errno 0).
    pub fn probe_mode(
        &self,
        family: &str,
        kind: ModeKind,
        mode: i32,
    ) -> Result<ModeStatus, EngineError> {
        if let Some(refusal) = modes::check_callable(family, kind, mode) {
            return Ok(refusal);
        }
        match kind {
            ModeKind::S if modes::s_sentinel(family).is_none() => {
                return Err(EngineError::Other(format!(
                    "probe_mode: no measured S unknown-mode sentinel for family {family:?} — \
                     add its D*.pas else-branch literal to modes::S_SENTINELS before probing S"
                )));
            }
            ModeKind::V if modes::v_sentinel_undetectable(family) => {
                return Err(EngineError::Other(format!(
                    "probe_mode: family {family:?} writes no V unknown-mode sentinel \
                     (modes::V_WITHOUT_SENTINEL) — a miss cannot be detected from its reply"
                )));
            }
            _ => {}
        }
        let status = match kind {
            ModeKind::V => {
                // The raw `myType` tag matters here (`ActiveClassV` reports the
                // miss as `myType = -1`), and `decode_v` folds every unknown tag
                // into `Bytes`, so read the V buffer directly instead.
                let fam = self
                    .families
                    .get(family)
                    .ok_or_else(|| EngineError::Other(format!("unknown FFI family {family:?}")))?;
                let f = fam.v.ok_or_else(|| {
                    EngineError::Other(format!("family {family} has no V entry point"))
                })?;
                let (ty, bytes) = self.call_v(f, mode);
                let strings = if ty == modes::SENTINEL_V_TYPE_TAG {
                    decode_string_raw(&bytes)
                } else {
                    Vec::new()
                };
                modes::classify_v(ty, &strings)
            }
            _ => {
                let out = self.ffi_dispatch(FfiCall {
                    family,
                    kind: kind.as_str(),
                    mode,
                    ..Default::default()
                })?;
                match out {
                    FfiOut::I(v) => modes::classify_i(v),
                    FfiOut::F(v) => modes::classify_f(v),
                    FfiOut::S(s) => modes::classify_s(family, &s),
                    FfiOut::V(_) | FfiOut::VSet(_) => {
                        return Err(EngineError::Other(format!(
                            "probe_mode: {kind} dispatch returned a V reply for {family}:{mode}"
                        )));
                    }
                }
            }
        };
        let _ = self.poll_error();
        Ok(status)
    }

    /// Dispatch one generic C-API call ([`FfiCall`]). For a `"v"` call,
    /// `vset = None` reads the array getter for `mode`; `vset = Some(_)` drives the
    /// SET mode, handing the array in via `myPointer`. The caller polls
    /// [`Engine::poll_error`] afterwards for the structured errno surface.
    ///
    /// This is the crate's **single chokepoint**: [`Engine::probe_mode`],
    /// [`Engine::read_mode`], every typed accessor and the worker's raw
    /// `{"cmd":"ffi"}` command all funnel through it, so the
    /// [`modes::DO_NOT_CALL`] register is enforced here — a triple on it is an
    /// `Err` and the DLL is never touched, whatever the caller. (`probe_mode`
    /// still consults the register itself, because its contract is to report the
    /// refusal as a typed [`ModeStatus`] rather than as an error.)
    pub fn ffi_dispatch(&self, call: FfiCall) -> Result<FfiOut, EngineError> {
        let FfiCall {
            family,
            kind,
            mode,
            iarg,
            farg,
            farg2,
            sarg,
            vset,
        } = call;
        if let Some(k) = ModeKind::from_tag(kind)
            && let Some(refusal) = modes::check_callable(family, k, mode)
        {
            return Err(EngineError::Other(format!(
                "{family}{}:{mode} is on the do-not-call register: {refusal}",
                k.as_str().to_ascii_uppercase()
            )));
        }
        let fam = self
            .families
            .get(family)
            .ok_or_else(|| EngineError::Other(format!("unknown FFI family {family:?}")))?;
        let missing =
            |k: &str| EngineError::Other(format!("family {family} has no {k} entry point"));
        match kind {
            "i" => {
                let f = fam.i.ok_or_else(|| missing("I"))?;
                // SAFETY: `f` is a transcribed `XxxI(mode, arg): longint` cdecl.
                Ok(FfiOut::I(unsafe { f(mode, iarg) }))
            }
            "f" => {
                // Two ABIs share this shape: the uniform one-double `XxxF` and
                // the two-double `CircuitF`/`CmathLibF`
                // (`crate::families::TWO_DOUBLE_F`). A family with neither keeps
                // the "no F entry point" error.
                match (fam.f, fam.f2) {
                    (Some(f), _) => {
                        // SAFETY: `f` is a transcribed `XxxF(mode, arg): double` cdecl.
                        Ok(FfiOut::F(unsafe { f(mode, farg) }))
                    }
                    (None, Some(f2)) => {
                        // SAFETY: `f2` is a transcribed
                        // `XxxF(mode; arg1, arg2: double): double` cdecl
                        // (`DCircuit.pas:27`, `DCmathLib.pas:5`).
                        Ok(FfiOut::F(unsafe { f2(mode, farg, farg2) }))
                    }
                    (None, None) => Err(missing("F")),
                }
            }
            "s" => {
                let f = fam.s.ok_or_else(|| missing("S"))?;
                let a = to_cstring(sarg);
                // SAFETY: `a` is a live NUL-terminated buffer for the call; the
                // returned pointer is DLL-owned and copied out immediately.
                let p = unsafe { f(mode, a.as_ptr()) };
                Ok(FfiOut::S(unsafe { cstr_to_string(p) }))
            }
            "v" => {
                let f = fam.v.ok_or_else(|| missing("V"))?;
                match vset {
                    None => {
                        let (tag, bytes) = self.call_v(f, mode);
                        Ok(FfiOut::V(decode_v(tag, &bytes)))
                    }
                    Some(data) => Ok(FfiOut::VSet(self.call_v_set(f, mode, &data))),
                }
            }
            other => Err(EngineError::Other(format!("unknown FFI kind {other:?}"))),
        }
    }

    // ---- typed WP-G1 mode accessors (G1.0 rails) --------------------------
    //
    // One thin `pub fn` per [`modes::WP_G1_MODES`] row: the mode *number* lives
    // only in that table, and each accessor takes its row by reference, so a
    // number can never drift between the table and its reader. Every accessor
    // funnels through [`Engine::read_mode`], which refuses a do-not-call mode
    // before any FFI and checks the reply's shape against the row.
    //
    // These are **rails**, not capture: no gate path calls them yet. Each WP-G1
    // surface sub-step wires the rows it needs into `crate::capture` and
    // compares them against the capi channel on a gated `both` case
    // (`GOLDEN_REBASE_PLAN.md` WP-G1, coordinator decision D2).

    /// Read one WP-G1 mode ([`modes::ModeSpec`]) generically — the path every
    /// typed accessor below shares, and the one a table-driven test walks.
    ///
    /// Refuses **before any FFI** a mode on [`modes::DO_NOT_CALL`], and rejects
    /// a reply whose shape is not the row's: for a `V` row the observed `myType`
    /// must equal the tag the `case` arm assigns ([`modes::ModeSpec::v_type`]),
    /// so a shape change in a future DLL revision fails loudly instead of being
    /// decoded as garbage.
    ///
    /// **Sentinel classification.** The `myType` check alone does not separate a
    /// served string array from the `V` unknown-mode reply, which also carries
    /// tag 4 — the two `Solution.IncMatrix{Rows,Cols}` rows would decode
    /// `"Error, paratemer not recognized"` as data — so a tag-4 reply is run
    /// through [`modes::classify_v`] and an [`modes::ModeStatus::UnknownMode`]
    /// verdict is an `Err`. On `I`/`F` no such check is possible: the sentinel is
    /// the plain value `-1` / `-1.0`, which several served modes may return
    /// legally (module doc of [`crate::modes`]), so classifying here would turn
    /// legitimate data into an error. The guarantee for those shapes is
    /// `r4133_mode_capability_is_complete_for_wp_g1`, which classifies **all**
    /// table rows through [`Engine::probe_mode`] against the git-tracked DLL and
    /// therefore trips the moment a re-vendored revision drops a mode.
    ///
    /// Drives the mode with the neutral argument (`0` / `0.0` / `""`, the array
    /// *getter* for `V`), which is sound because every table row is a getter
    /// (`modes::EXCLUDED_WRITE_MODES` records the two arms that are not, and a
    /// unit test keeps them out of the table).
    pub fn read_mode(&self, spec: &ModeSpec) -> Result<FfiOut, EngineError> {
        if let Some(refusal) = modes::check_callable(spec.family, spec.kind, spec.mode) {
            return Err(EngineError::Other(format!("{spec}: {refusal}")));
        }
        let out = self.ffi_dispatch(FfiCall {
            family: spec.family,
            kind: spec.kind.as_str(),
            mode: spec.mode,
            ..Default::default()
        })?;
        match (&out, spec.kind) {
            (FfiOut::I(_), ModeKind::I)
            | (FfiOut::F(_), ModeKind::F)
            | (FfiOut::S(_), ModeKind::S) => Ok(out),
            (FfiOut::V(v), ModeKind::V) => {
                let want = spec.v_type.ok_or_else(|| {
                    EngineError::Other(format!("{spec}: a V row must declare its myType tag"))
                })?;
                if v.type_tag() == want {
                    if let crate::families::VData::Strings(ss) = v
                        && let ModeStatus::UnknownMode { sentinel } =
                            modes::classify_v(v.type_tag(), ss)
                    {
                        return Err(EngineError::Other(format!(
                            "{spec}: the DLL replied with the V unknown-mode sentinel \
                             {sentinel:?} — this revision does not serve the mode"
                        )));
                    }
                    Ok(out)
                } else {
                    Err(EngineError::Other(format!(
                        "{spec}: the DLL replied with myType {} but the case arm assigns {want} \
                         — the V shape of this mode changed",
                        v.type_tag()
                    )))
                }
            }
            (other, kind) => Err(EngineError::Other(format!(
                "{spec}: a {kind} mode replied {other:?}"
            ))),
        }
    }

    /// The scalar `i32` of an `I` row.
    fn read_mode_i(&self, spec: &ModeSpec) -> Result<i32, EngineError> {
        match self.read_mode(spec)? {
            FfiOut::I(v) => Ok(v),
            other => Err(EngineError::Other(format!(
                "{spec}: expected I, got {other:?}"
            ))),
        }
    }

    /// Drive an `I` row that takes a **selector argument** — today exactly
    /// [`modes::METERS_SET_ACTIVE_SECTION`] (see the "one declared selector"
    /// paragraph on [`ModeSpec`]).
    ///
    /// Same rails as [`Engine::read_mode`]: the [`modes::DO_NOT_CALL`] refusal
    /// before any FFI and a shape check on the reply — only the neutral `0` is
    /// replaced by the caller's index, which is the whole point (the neutral
    /// drive would deselect instead of select).
    fn select_mode_i(&self, spec: &ModeSpec, arg: i32) -> Result<i32, EngineError> {
        if let Some(refusal) = modes::check_callable(spec.family, spec.kind, spec.mode) {
            return Err(EngineError::Other(format!("{spec}: {refusal}")));
        }
        if spec.kind != ModeKind::I {
            return Err(EngineError::Other(format!(
                "{spec}: a selector must be an I row"
            )));
        }
        match self.ffi_dispatch(FfiCall {
            family: spec.family,
            kind: spec.kind.as_str(),
            mode: spec.mode,
            iarg: arg,
            ..Default::default()
        })? {
            FfiOut::I(v) => Ok(v),
            other => Err(EngineError::Other(format!(
                "{spec}: expected I, got {other:?}"
            ))),
        }
    }

    /// The scalar `f64` of an `F` row.
    fn read_mode_f(&self, spec: &ModeSpec) -> Result<f64, EngineError> {
        match self.read_mode(spec)? {
            FfiOut::F(v) => Ok(v),
            other => Err(EngineError::Other(format!(
                "{spec}: expected F, got {other:?}"
            ))),
        }
    }

    /// The string of an `S` row.
    fn read_mode_s(&self, spec: &ModeSpec) -> Result<String, EngineError> {
        match self.read_mode(spec)? {
            FfiOut::S(v) => Ok(v),
            other => Err(EngineError::Other(format!(
                "{spec}: expected S, got {other:?}"
            ))),
        }
    }

    /// An `I` row whose Pascal codomain is exactly `{0, 1}`, decoded as a
    /// `bool` — never `!= 0`.
    ///
    /// Every `CktElementI` arm starts from the family default `Result := 0`
    /// (`DDLL/DCktElement.pas:137`) and a boolean arm only ever raises it to
    /// `1`, so any other reply is the family's unknown-mode sentinel `-1`
    /// (`:308`) — this DLL revision does not serve the mode — and must be an
    /// error rather than a truthy answer. `codomain` names the Pascal lines the
    /// message cites.
    fn read_bool01(&self, spec: &ModeSpec, codomain: &str) -> Result<bool, EngineError> {
        match self.read_mode_i(spec)? {
            0 => Ok(false),
            1 => Ok(true),
            other => Err(EngineError::Other(format!(
                "{spec}: replied {other}, but the case arm's codomain is {{0, 1}} \
                 (DCktElement.pas{codomain})"
            ))),
        }
    }

    /// An `I` row that reports a count, a 1-based position or a small tag —
    /// every one of them non-negative by construction (the family default is
    /// `Result := 0`, `DDLL/DCktElement.pas:137`). A negative reply is the
    /// unknown-mode sentinel `-1` (`:308`), never data, so it is an error
    /// instead of a silently-cast count.
    fn read_count(&self, spec: &ModeSpec) -> Result<i32, EngineError> {
        match self.read_mode_i(spec)? {
            v if v >= 0 => Ok(v),
            other => Err(EngineError::Other(format!(
                "{spec}: replied {other}, but this arm's codomain is the non-negative integers \
                 (DCktElement.pas:137); -1 is the CktElementI unknown-mode sentinel (:308)"
            ))),
        }
    }

    /// The `i32` array of a `myType = 1` `V` row.
    fn read_mode_ints(&self, spec: &ModeSpec) -> Result<Vec<i32>, EngineError> {
        match self.read_mode(spec)? {
            FfiOut::V(VData::Ints(v)) => Ok(v),
            other => Err(EngineError::Other(format!(
                "{spec}: expected an int array, got {other:?}"
            ))),
        }
    }

    /// The `f64` array of a `myType = 2` (real) or `myType = 3` (complex,
    /// interleaved `[re, im, …]`) `V` row — the flat layout the DLL writes and
    /// the existing typed accessors already return.
    fn read_mode_doubles(&self, spec: &ModeSpec) -> Result<Vec<f64>, EngineError> {
        match self.read_mode(spec)? {
            FfiOut::V(VData::Doubles(v) | VData::Complex(v)) => Ok(v),
            other => Err(EngineError::Other(format!(
                "{spec}: expected a double array, got {other:?}"
            ))),
        }
    }

    /// The string array of a `myType = 4` `V` row (raw — no monitor-header
    /// strip; [`crate::families::decode_string_raw`]).
    fn read_mode_strings(&self, spec: &ModeSpec) -> Result<Vec<String>, EngineError> {
        match self.read_mode(spec)? {
            FfiOut::V(VData::Strings(v)) => Ok(v),
            other => Err(EngineError::Other(format!(
                "{spec}: expected a string array, got {other:?}"
            ))),
        }
    }

    // -- CktElement --------------------------------------------------------------
    /// `CktElementI(0)` `CktElement.NumTerminals` — `DCktElement.pas:139`. See [`modes::CKT_ELEMENT_NUM_TERMINALS`].
    pub fn ckt_element_num_terminals(&self) -> Result<i32, EngineError> {
        self.read_mode_i(&modes::CKT_ELEMENT_NUM_TERMINALS)
    }

    /// `CktElementI(1)` `CktElement.NumConductors` — `DCktElement.pas:144`. See [`modes::CKT_ELEMENT_NUM_CONDUCTORS`].
    pub fn ckt_element_num_conductors(&self) -> Result<i32, EngineError> {
        self.read_mode_i(&modes::CKT_ELEMENT_NUM_CONDUCTORS)
    }

    /// `CktElementI(2)` `CktElement.NumPhases` — `DCktElement.pas:149`. See [`modes::CKT_ELEMENT_NUM_PHASES`].
    pub fn ckt_element_num_phases(&self) -> Result<i32, EngineError> {
        self.read_mode_i(&modes::CKT_ELEMENT_NUM_PHASES)
    }

    /// `CktElementI(7)` `CktElement.HasSwitchControl` — `DCktElement.pas:207`. See [`modes::CKT_ELEMENT_HAS_SWITCH_CONTROL`].
    ///
    /// Decoded **strictly**, for the reason spelled out at
    /// [`Engine::ckt_element_enabled`]: the arm's codomain is exactly `{0, 1}`
    /// (the `CktElementI` default `Result := 0` at `DCktElement.pas:137`, set to
    /// `1` and `Exit`ed on the first `SWT_CONTROL` at `:210-215`), so under a
    /// `!= 0` decode the family's unknown-mode sentinel `-1` (`:308`) would read
    /// as "a switch control is attached".
    pub fn ckt_element_has_switch_control(&self) -> Result<bool, EngineError> {
        self.read_bool01(&modes::CKT_ELEMENT_HAS_SWITCH_CONTROL, ":137, :207-221")
    }

    /// `CktElementI(8)` `CktElement.HasVoltControl` — `DCktElement.pas:222`. See [`modes::CKT_ELEMENT_HAS_VOLT_CONTROL`].
    ///
    /// Decoded strictly, exactly as
    /// [`Engine::ckt_element_has_switch_control`]: codomain `{0, 1}`
    /// (`DCktElement.pas:137`, the `CAP_CONTROL, REG_CONTROL: Result := 1` +
    /// `Exit` at `:225-231`).
    pub fn ckt_element_has_volt_control(&self) -> Result<bool, EngineError> {
        self.read_bool01(&modes::CKT_ELEMENT_HAS_VOLT_CONTROL, ":137, :222-236")
    }

    /// `CktElementI(9)` `CktElement.NumControls` — `DCktElement.pas:237`. See [`modes::CKT_ELEMENT_NUM_CONTROLS`].
    /// A list size: [`Engine::read_count`] rejects a negative reply.
    pub fn ckt_element_num_controls(&self) -> Result<i32, EngineError> {
        self.read_count(&modes::CKT_ELEMENT_NUM_CONTROLS)
    }

    /// `CktElementI(10)` `CktElement.OCPDevIndex` — `DCktElement.pas:242`. See [`modes::CKT_ELEMENT_OCP_DEV_INDEX`].
    /// A 1-based list position or `0`: [`Engine::read_count`] rejects a negative
    /// reply.
    pub fn ckt_element_ocp_dev_index(&self) -> Result<i32, EngineError> {
        self.read_count(&modes::CKT_ELEMENT_OCP_DEV_INDEX)
    }

    /// `CktElementI(11)` `CktElement.OCPDevType` — `DCktElement.pas:259`. See [`modes::CKT_ELEMENT_OCP_DEV_TYPE`].
    /// `GetOCPDeviceType`'s codomain is `0..3` (`Common/Utilities.pas:3165-3184`):
    /// [`Engine::read_count`] rejects a negative reply.
    pub fn ckt_element_ocp_dev_type(&self) -> Result<i32, EngineError> {
        self.read_count(&modes::CKT_ELEMENT_OCP_DEV_TYPE)
    }

    /// `CktElementI(12)` `CktElement.Enabled` — `DCktElement.pas:263`. See [`modes::CKT_ELEMENT_ENABLED`].
    ///
    /// Decodes **strictly**. The arm's codomain is exactly `{0, 1}` (the
    /// `CktElementI` default `Result := 0` at `DCktElement.pas:137`, raised to 1
    /// only when `Enabled`), so anything else is an error rather than a truthy
    /// "enabled": under a `!= 0` decode the family's unknown-mode sentinel `-1`
    /// (`DCktElement.pas:308`) would read as *enabled* and route the derived
    /// capture into `CktElementV(19)`'s unguarded `NodeRef^[i]` dereference
    /// (`:1099`), which kills the process.
    pub fn ckt_element_enabled(&self) -> Result<bool, EngineError> {
        match self.read_mode_i(&modes::CKT_ELEMENT_ENABLED)? {
            0 => Ok(false),
            1 => Ok(true),
            other => Err(EngineError::Other(format!(
                "{}: replied {other}, but the case arm's codomain is {{0, 1}} \
                 (DCktElement.pas:137, :263)",
                modes::CKT_ELEMENT_ENABLED
            ))),
        }
    }

    /// `CktElementI(15)` `CktElement.HasOCPDevice` — `DCktElement.pas:300`. See [`modes::CKT_ELEMENT_HAS_OCP_DEVICE`].
    pub fn ckt_element_has_ocp_device(&self) -> Result<i32, EngineError> {
        self.read_mode_i(&modes::CKT_ELEMENT_HAS_OCP_DEVICE)
    }

    /// `CktElementS(4)` `CktElement.EnergyMeter` — `DCktElement.pas:442`. See [`modes::CKT_ELEMENT_ENERGY_METER`].
    pub fn ckt_element_energy_meter(&self) -> Result<String, EngineError> {
        self.read_mode_s(&modes::CKT_ELEMENT_ENERGY_METER)
    }

    /// `CktElementV(6)` `CktElement.PhaseLosses` — `DCktElement.pas:637`. See [`modes::CKT_ELEMENT_PHASE_LOSSES`].
    pub fn ckt_element_phase_losses(&self) -> Result<Vec<f64>, EngineError> {
        self.read_mode_doubles(&modes::CKT_ELEMENT_PHASE_LOSSES)
    }

    /// `CktElementV(7)` `CktElement.SeqVoltages` — `DCktElement.pas:660`. See [`modes::CKT_ELEMENT_SEQ_VOLTAGES`].
    pub fn ckt_element_seq_voltages(&self) -> Result<Vec<f64>, EngineError> {
        self.read_mode_doubles(&modes::CKT_ELEMENT_SEQ_VOLTAGES)
    }

    /// `CktElementV(8)` `CktElement.SeqCurrents` — `DCktElement.pas:700`. See [`modes::CKT_ELEMENT_SEQ_CURRENTS`].
    pub fn ckt_element_seq_currents(&self) -> Result<Vec<f64>, EngineError> {
        self.read_mode_doubles(&modes::CKT_ELEMENT_SEQ_CURRENTS)
    }

    /// `CktElementV(9)` `CktElement.SeqPowers` — `DCktElement.pas:739`. See [`modes::CKT_ELEMENT_SEQ_POWERS`].
    pub fn ckt_element_seq_powers(&self) -> Result<Vec<f64>, EngineError> {
        self.read_mode_doubles(&modes::CKT_ELEMENT_SEQ_POWERS)
    }

    /// `CktElementV(11)` `CktElement.Residuals` — `DCktElement.pas:827`. See [`modes::CKT_ELEMENT_RESIDUALS`].
    pub fn ckt_element_residuals(&self) -> Result<Vec<f64>, EngineError> {
        self.read_mode_doubles(&modes::CKT_ELEMENT_RESIDUALS)
    }

    /// `CktElementV(13)` `CktElement.CplxSeqVoltages` — `DCktElement.pas:885`. See [`modes::CKT_ELEMENT_CPLX_SEQ_VOLTAGES`].
    pub fn ckt_element_cplx_seq_voltages(&self) -> Result<Vec<f64>, EngineError> {
        self.read_mode_doubles(&modes::CKT_ELEMENT_CPLX_SEQ_VOLTAGES)
    }

    /// `CktElementV(14)` `CktElement.CplxSeqCurrents` — `DCktElement.pas:931`. See [`modes::CKT_ELEMENT_CPLX_SEQ_CURRENTS`].
    pub fn ckt_element_cplx_seq_currents(&self) -> Result<Vec<f64>, EngineError> {
        self.read_mode_doubles(&modes::CKT_ELEMENT_CPLX_SEQ_CURRENTS)
    }

    /// `CktElementV(17)` `CktElement.NodeOrder` — `DCktElement.pas:1032`. See [`modes::CKT_ELEMENT_NODE_ORDER`].
    pub fn ckt_element_node_order(&self) -> Result<Vec<i32>, EngineError> {
        self.read_mode_ints(&modes::CKT_ELEMENT_NODE_ORDER)
    }

    /// `CktElementV(18)` `CktElement.CurrentsMagAng` — `DCktElement.pas:1058`. See [`modes::CKT_ELEMENT_CURRENTS_MAG_ANG`].
    pub fn ckt_element_currents_mag_ang(&self) -> Result<Vec<f64>, EngineError> {
        self.read_mode_doubles(&modes::CKT_ELEMENT_CURRENTS_MAG_ANG)
    }

    /// `CktElementV(19)` `CktElement.VoltagesMagAng` — `DCktElement.pas:1082`. See [`modes::CKT_ELEMENT_VOLTAGES_MAG_ANG`].
    pub fn ckt_element_voltages_mag_ang(&self) -> Result<Vec<f64>, EngineError> {
        self.read_mode_doubles(&modes::CKT_ELEMENT_VOLTAGES_MAG_ANG)
    }

    /// `CktElementV(20)` `CktElement.TotalPowers` — `DCktElement.pas:1109`. See [`modes::CKT_ELEMENT_TOTAL_POWERS`].
    pub fn ckt_element_total_powers(&self) -> Result<Vec<f64>, EngineError> {
        self.read_mode_doubles(&modes::CKT_ELEMENT_TOTAL_POWERS)
    }

    // -- Bus --------------------------------------------------------------
    /// `BUSF(5)` `Bus.Distance` — `DBus.pas:122`. See [`modes::BUS_DISTANCE`].
    pub fn bus_distance(&self) -> Result<f64, EngineError> {
        self.read_mode_f(&modes::BUS_DISTANCE)
    }

    /// `BUSV(1)` `Bus.SeqVoltages` — `DBus.pas:285`. See [`modes::BUS_SEQ_VOLTAGES`].
    pub fn bus_seq_voltages(&self) -> Result<Vec<f64>, EngineError> {
        self.read_mode_doubles(&modes::BUS_SEQ_VOLTAGES)
    }

    /// `BUSV(2)` `Bus.Nodes` — `DBus.pas:319`. See [`modes::BUS_NODES`].
    ///
    /// The active bus's node **numbers** in ascending order (not the bus's
    /// internal insertion order), the same order every per-node array of this
    /// family uses — see [`modes::BUS_NODES`] for the shared `FindIdx` walk and
    /// its capi twin.
    pub fn bus_nodes(&self) -> Result<Vec<i32>, EngineError> {
        self.read_mode_ints(&modes::BUS_NODES)
    }

    /// `BUSV(3)` `Bus.Voc` — `DBus.pas:351`. See [`modes::BUS_VOC`].
    pub fn bus_voc(&self) -> Result<Vec<f64>, EngineError> {
        self.read_mode_doubles(&modes::BUS_VOC)
    }

    /// `BUSV(4)` `Bus.Isc` — `DBus.pas:374`. See [`modes::BUS_ISC`].
    pub fn bus_isc(&self) -> Result<Vec<f64>, EngineError> {
        self.read_mode_doubles(&modes::BUS_ISC)
    }

    /// `BUSV(5)` `Bus.PuVoltages` — `DBus.pas:399`. See [`modes::BUS_PU_VOLTAGES`].
    pub fn bus_pu_voltages(&self) -> Result<Vec<f64>, EngineError> {
        self.read_mode_doubles(&modes::BUS_PU_VOLTAGES)
    }

    /// `BUSV(6)` `Bus.ZscMatrix` — `DBus.pas:431`. See [`modes::BUS_ZSC_MATRIX`].
    pub fn bus_zsc_matrix(&self) -> Result<Vec<f64>, EngineError> {
        self.read_mode_doubles(&modes::BUS_ZSC_MATRIX)
    }

    /// `BUSV(7)` `Bus.Zsc1` — `DBus.pas:461`. See [`modes::BUS_ZSC1`].
    pub fn bus_zsc1(&self) -> Result<Vec<f64>, EngineError> {
        self.read_mode_doubles(&modes::BUS_ZSC1)
    }

    /// `BUSV(8)` `Bus.Zsc0` — `DBus.pas:476`. See [`modes::BUS_ZSC0`].
    pub fn bus_zsc0(&self) -> Result<Vec<f64>, EngineError> {
        self.read_mode_doubles(&modes::BUS_ZSC0)
    }

    /// `BUSV(9)` `Bus.YscMatrix` — `DBus.pas:491`. See [`modes::BUS_YSC_MATRIX`].
    pub fn bus_ysc_matrix(&self) -> Result<Vec<f64>, EngineError> {
        self.read_mode_doubles(&modes::BUS_YSC_MATRIX)
    }

    /// `BUSV(10)` `Bus.CplxSeqVoltages` — `DBus.pas:520`. See [`modes::BUS_CPLX_SEQ_VOLTAGES`].
    pub fn bus_cplx_seq_voltages(&self) -> Result<Vec<f64>, EngineError> {
        self.read_mode_doubles(&modes::BUS_CPLX_SEQ_VOLTAGES)
    }

    /// `BUSV(11)` `Bus.VLL` — `DBus.pas:549`. See [`modes::BUS_VLL`].
    pub fn bus_vll(&self) -> Result<Vec<f64>, EngineError> {
        self.read_mode_doubles(&modes::BUS_VLL)
    }

    /// `BUSV(12)` `Bus.PuVLL` — `DBus.pas:603`. See [`modes::BUS_PU_VLL`].
    pub fn bus_pu_vll(&self) -> Result<Vec<f64>, EngineError> {
        self.read_mode_doubles(&modes::BUS_PU_VLL)
    }

    /// `BUSV(13)` `Bus.VMagAngle` — `DBus.pas:659`. See [`modes::BUS_VMAG_ANGLE`].
    pub fn bus_vmag_angle(&self) -> Result<Vec<f64>, EngineError> {
        self.read_mode_doubles(&modes::BUS_VMAG_ANGLE)
    }

    /// `BUSV(14)` `Bus.PuVMagAngle` — `DBus.pas:690`. See [`modes::BUS_PU_VMAG_ANGLE`].
    pub fn bus_pu_vmag_angle(&self) -> Result<Vec<f64>, EngineError> {
        self.read_mode_doubles(&modes::BUS_PU_VMAG_ANGLE)
    }

    /// `BUSV(18)` `Bus.AllPCEatBus` — `DBus.pas:840`. See [`modes::BUS_ALL_PCE_AT_BUS`].
    pub fn bus_all_pce_at_bus(&self) -> Result<Vec<String>, EngineError> {
        self.read_mode_strings(&modes::BUS_ALL_PCE_AT_BUS)
    }

    /// `BUSV(19)` `Bus.AllPDEatBus` — `DBus.pas:867`. See [`modes::BUS_ALL_PDE_AT_BUS`].
    pub fn bus_all_pde_at_bus(&self) -> Result<Vec<String>, EngineError> {
        self.read_mode_strings(&modes::BUS_ALL_PDE_AT_BUS)
    }

    // -- Bus reliability columns (GOLDEN_REBASE G1.6(ii)) ---------------------
    // The eight per-bus columns of fastdss' `IBus._columns`
    // (`DSS-Python@origin/fastdss` `dss/IBus.py:19-53`) that the EnergyMeter
    // `RelCalc` sweep writes. Every read answers for the **active bus**, so a
    // caller selects with [`Engine::set_active_bus`] first; see the group's
    // block comment in [`modes`] for the shared `ActiveBusIndex > 0` guard.

    /// `BUSF(6)` `Bus.Lambda` — `DBus.pas:129`. See [`modes::BUS_LAMBDA`].
    pub fn bus_lambda(&self) -> Result<f64, EngineError> {
        self.read_mode_f(&modes::BUS_LAMBDA)
    }

    /// `BUSF(7)` `Bus.N_interrupts` — `DBus.pas:136`. See [`modes::BUS_N_INTERRUPTS`].
    pub fn bus_n_interrupts(&self) -> Result<f64, EngineError> {
        self.read_mode_f(&modes::BUS_N_INTERRUPTS)
    }

    /// `BUSF(8)` `Bus.Int_Duration` — `DBus.pas:143`. See [`modes::BUS_INT_DURATION`].
    pub fn bus_int_duration(&self) -> Result<f64, EngineError> {
        self.read_mode_f(&modes::BUS_INT_DURATION)
    }

    /// `BUSF(9)` `Bus.Cust_Interrupts` — `DBus.pas:150`. See [`modes::BUS_CUST_INTERRUPTS`].
    pub fn bus_cust_interrupts(&self) -> Result<f64, EngineError> {
        self.read_mode_f(&modes::BUS_CUST_INTERRUPTS)
    }

    /// `BUSF(10)` `Bus.Cust_Duration` — `DBus.pas:157`. See [`modes::BUS_CUST_DURATION`].
    pub fn bus_cust_duration(&self) -> Result<f64, EngineError> {
        self.read_mode_f(&modes::BUS_CUST_DURATION)
    }

    /// `BUSF(11)` `Bus.TotalMiles` — `DBus.pas:164`. See [`modes::BUS_TOTAL_MILES`].
    pub fn bus_total_miles(&self) -> Result<f64, EngineError> {
        self.read_mode_f(&modes::BUS_TOTAL_MILES)
    }

    /// `BUSI(4)` `Bus.N_Customers` — `DBus.pas:60`. See [`modes::BUS_N_CUSTOMERS`].
    ///
    /// Goes through the family's **integer** entry point (`BUSI`), not `BUSF`:
    /// `BusTotalNumCustomers` is a `longint`, and mode 4 on the `F` shape is
    /// `Bus.Y - Write` (`DBus.pas:113-121`) — it would store the generic
    /// reader's neutral `0.0` into the bus coordinate and set `Coorddefined`.
    /// The collision is a row of [`modes::EXCLUDED_WRITE_MODES`], not a comment.
    pub fn bus_n_customers(&self) -> Result<i32, EngineError> {
        self.read_mode_i(&modes::BUS_N_CUSTOMERS)
    }

    /// `BUSI(5)` `Bus.SectionID` — `DBus.pas:67`. See [`modes::BUS_SECTION_ID`],
    /// which records why a `-1` here is data and not necessarily the `I`
    /// unknown-mode sentinel.
    pub fn bus_section_id(&self) -> Result<i32, EngineError> {
        self.read_mode_i(&modes::BUS_SECTION_ID)
    }

    // -- Circuit --------------------------------------------------------------
    /// `CircuitV(0)` `Circuit.Losses` — `DCircuit.pas:294`. See [`modes::CIRCUIT_LOSSES`].
    pub fn circuit_losses(&self) -> Result<Vec<f64>, EngineError> {
        self.read_mode_doubles(&modes::CIRCUIT_LOSSES)
    }

    /// `CircuitV(1)` `Circuit.LineLosses` — `DCircuit.pas:305`. See [`modes::CIRCUIT_LINE_LOSSES`].
    pub fn circuit_line_losses(&self) -> Result<Vec<f64>, EngineError> {
        self.read_mode_doubles(&modes::CIRCUIT_LINE_LOSSES)
    }

    /// `CircuitV(2)` `Circuit.SubstationLosses` — `DCircuit.pas:327`. See [`modes::CIRCUIT_SUBSTATION_LOSSES`].
    pub fn circuit_substation_losses(&self) -> Result<Vec<f64>, EngineError> {
        self.read_mode_doubles(&modes::CIRCUIT_SUBSTATION_LOSSES)
    }

    /// `CircuitV(3)` `Circuit.TotalPower` — `DCircuit.pas:349`. See [`modes::CIRCUIT_TOTAL_POWER`].
    pub fn circuit_total_power(&self) -> Result<Vec<f64>, EngineError> {
        self.read_mode_doubles(&modes::CIRCUIT_TOTAL_POWER)
    }

    /// `CircuitV(7)` `Circuit.AllBusNames` — `DCircuit.pas:439`. See
    /// [`modes::CIRCUIT_ALL_BUS_NAMES`].
    ///
    /// Every bus name in `BusList` order — the walk order of the per-bus
    /// capture, which re-asserts it bus by bus through
    /// [`Engine::set_active_bus`]'s returned index.
    pub fn circuit_all_bus_names(&self) -> Result<Vec<String>, EngineError> {
        self.read_mode_strings(&modes::CIRCUIT_ALL_BUS_NAMES)
    }

    /// `CircuitV(8)` `Circuit.AllElementLosses` — `DCircuit.pas:458`. See [`modes::CIRCUIT_ALL_ELEMENT_LOSSES`].
    pub fn circuit_all_element_losses(&self) -> Result<Vec<f64>, EngineError> {
        self.read_mode_doubles(&modes::CIRCUIT_ALL_ELEMENT_LOSSES)
    }

    /// `CircuitV(9)` `Circuit.AllBusMagPu` — `DCircuit.pas:481`. See [`modes::CIRCUIT_ALL_BUS_MAG_PU`].
    pub fn circuit_all_bus_mag_pu(&self) -> Result<Vec<f64>, EngineError> {
        self.read_mode_doubles(&modes::CIRCUIT_ALL_BUS_MAG_PU)
    }

    /// `CircuitV(12)` `Circuit.AllBusDistances` — `DCircuit.pas:566`. See [`modes::CIRCUIT_ALL_BUS_DISTANCES`].
    pub fn circuit_all_bus_distances(&self) -> Result<Vec<f64>, EngineError> {
        self.read_mode_doubles(&modes::CIRCUIT_ALL_BUS_DISTANCES)
    }

    /// `CircuitV(13)` `Circuit.AllNodeDistances` — `DCircuit.pas:582`. See [`modes::CIRCUIT_ALL_NODE_DISTANCES`].
    pub fn circuit_all_node_distances(&self) -> Result<Vec<f64>, EngineError> {
        self.read_mode_doubles(&modes::CIRCUIT_ALL_NODE_DISTANCES)
    }

    // -- Meters --------------------------------------------------------------
    /// `MetersI(20)` `Meters.TotalCustomers` — `DMeters.pas:232`. See [`modes::METERS_TOTAL_CUSTOMERS`].
    pub fn meters_total_customers(&self) -> Result<i32, EngineError> {
        self.read_mode_i(&modes::METERS_TOTAL_CUSTOMERS)
    }

    /// `MetersI(21)` `Meters.NumSections` — `DMeters.pas:244`. See [`modes::METERS_NUM_SECTIONS`].
    pub fn meters_num_sections(&self) -> Result<i32, EngineError> {
        self.read_mode_i(&modes::METERS_NUM_SECTIONS)
    }

    /// `MetersI(22)` `Meters.SetActiveSection` — `DMeters.pas:254`. See
    /// [`modes::METERS_SET_ACTIVE_SECTION`].
    ///
    /// Selects the 1-based feeder section every `MetersI(23..27)` /
    /// `MetersF(4..6)` read then answers for, on the **active meter**; `0` (or
    /// any index outside `1..=NumSections`) deselects, after which those eight
    /// reads return `0` (`DMeters.pas:254-264`). The selection is a per-meter
    /// field the `Meters.First`/`Next` walk never resets, so every section block
    /// of a capture must select first.
    ///
    /// The arm assigns no `Result`, so the family default `0` (`DMeters.pas:30`)
    /// is the only legal reply: a `-1` is the family's unknown-mode sentinel
    /// ([`modes::classify_i`]) — a DLL that does not serve the selector at all,
    /// where every section read would silently answer for section 0 — and is
    /// escalated instead of being decoded as success.
    pub fn meters_set_active_section(&self, section: i32) -> Result<(), EngineError> {
        let reply = self.select_mode_i(&modes::METERS_SET_ACTIVE_SECTION, section)?;
        match modes::classify_i(reply) {
            ModeStatus::Served => Ok(()),
            other => Err(EngineError::Other(format!(
                "{}: {other}",
                modes::METERS_SET_ACTIVE_SECTION
            ))),
        }
    }

    /// `MetersI(23)` `Meters.OCPDeviceType` — `DMeters.pas:265`. See [`modes::METERS_OCP_DEVICE_TYPE`].
    pub fn meters_ocp_device_type(&self) -> Result<i32, EngineError> {
        self.read_mode_i(&modes::METERS_OCP_DEVICE_TYPE)
    }

    /// `MetersI(24)` `Meters.NumSectionCustomers` — `DMeters.pas:275`. See [`modes::METERS_NUM_SECTION_CUSTOMERS`].
    pub fn meters_num_section_customers(&self) -> Result<i32, EngineError> {
        self.read_mode_i(&modes::METERS_NUM_SECTION_CUSTOMERS)
    }

    /// `MetersI(25)` `Meters.NumSectionBranches` — `DMeters.pas:285`. See [`modes::METERS_NUM_SECTION_BRANCHES`].
    pub fn meters_num_section_branches(&self) -> Result<i32, EngineError> {
        self.read_mode_i(&modes::METERS_NUM_SECTION_BRANCHES)
    }

    /// `MetersI(26)` `Meters.SectSeqIdx` — `DMeters.pas:295`. See [`modes::METERS_SECT_SEQ_IDX`].
    pub fn meters_sect_seq_idx(&self) -> Result<i32, EngineError> {
        self.read_mode_i(&modes::METERS_SECT_SEQ_IDX)
    }

    /// `MetersI(27)` `Meters.SectTotalCust` — `DMeters.pas:305`. See [`modes::METERS_SECT_TOTAL_CUST`].
    pub fn meters_sect_total_cust(&self) -> Result<i32, EngineError> {
        self.read_mode_i(&modes::METERS_SECT_TOTAL_CUST)
    }

    /// `MetersF(0)` `Meters.SAIFI` — `DMeters.pas:329`. See [`modes::METERS_SAIFI`].
    pub fn meters_saifi(&self) -> Result<f64, EngineError> {
        self.read_mode_f(&modes::METERS_SAIFI)
    }

    /// `MetersF(1)` `Meters.SAIFIkW` — `DMeters.pas:340`. See [`modes::METERS_SAIFI_KW`].
    pub fn meters_saifi_kw(&self) -> Result<f64, EngineError> {
        self.read_mode_f(&modes::METERS_SAIFI_KW)
    }

    /// `MetersF(2)` `Meters.SAIDI` — `DMeters.pas:351`. See [`modes::METERS_SAIDI`].
    pub fn meters_saidi(&self) -> Result<f64, EngineError> {
        self.read_mode_f(&modes::METERS_SAIDI)
    }

    /// `MetersF(3)` `Meters.CustInterrupts` — `DMeters.pas:360`. See [`modes::METERS_CUST_INTERRUPTS`].
    pub fn meters_cust_interrupts(&self) -> Result<f64, EngineError> {
        self.read_mode_f(&modes::METERS_CUST_INTERRUPTS)
    }

    /// `MetersF(4)` `Meters.AvgRepairTime` — `DMeters.pas:369`. See [`modes::METERS_AVG_REPAIR_TIME`].
    pub fn meters_avg_repair_time(&self) -> Result<f64, EngineError> {
        self.read_mode_f(&modes::METERS_AVG_REPAIR_TIME)
    }

    /// `MetersF(5)` `Meters.FaultRateXRepairHrs` — `DMeters.pas:378`. See [`modes::METERS_FAULT_RATE_X_REPAIR_HRS`].
    pub fn meters_fault_rate_x_repair_hrs(&self) -> Result<f64, EngineError> {
        self.read_mode_f(&modes::METERS_FAULT_RATE_X_REPAIR_HRS)
    }

    /// `MetersF(6)` `Meters.SumBranchFltRates` — `DMeters.pas:387`. See [`modes::METERS_SUM_BRANCH_FLT_RATES`].
    pub fn meters_sum_branch_flt_rates(&self) -> Result<f64, EngineError> {
        self.read_mode_f(&modes::METERS_SUM_BRANCH_FLT_RATES)
    }

    /// `MetersV(3)` `Meters.Totals` — `DMeters.pas:558`. See [`modes::METERS_TOTALS`].
    pub fn meters_totals(&self) -> Result<Vec<f64>, EngineError> {
        self.read_mode_doubles(&modes::METERS_TOTALS)
    }

    /// `MetersV(6)` `Meters.CalcCurrent` — `DMeters.pas:609`. See [`modes::METERS_CALC_CURRENT`].
    pub fn meters_calc_current(&self) -> Result<Vec<f64>, EngineError> {
        self.read_mode_doubles(&modes::METERS_CALC_CURRENT)
    }

    /// `MetersV(8)` `Meters.AllocFactors` — `DMeters.pas:645`. See [`modes::METERS_ALLOC_FACTORS`].
    pub fn meters_alloc_factors(&self) -> Result<Vec<f64>, EngineError> {
        self.read_mode_doubles(&modes::METERS_ALLOC_FACTORS)
    }

    // -- Topology --------------------------------------------------------------
    /// `TopologyI(0)` `Topology.NumLoops` — `DTopology.pas:67`. See [`modes::TOPOLOGY_NUM_LOOPS`].
    pub fn topology_num_loops(&self) -> Result<i32, EngineError> {
        self.read_mode_i(&modes::TOPOLOGY_NUM_LOOPS)
    }

    /// `TopologyI(1)` `Topology.NumIsolatedBranches` — `DTopology.pas:79`. See [`modes::TOPOLOGY_NUM_ISOLATED_BRANCHES`].
    pub fn topology_num_isolated_branches(&self) -> Result<i32, EngineError> {
        self.read_mode_i(&modes::TOPOLOGY_NUM_ISOLATED_BRANCHES)
    }

    /// `TopologyI(2)` `Topology.NumIsolatedLoads` — `DTopology.pas:89`. See [`modes::TOPOLOGY_NUM_ISOLATED_LOADS`].
    pub fn topology_num_isolated_loads(&self) -> Result<i32, EngineError> {
        self.read_mode_i(&modes::TOPOLOGY_NUM_ISOLATED_LOADS)
    }

    /// `TopologyV(0)` `Topology.AllLoopedPairs` — `DTopology.pas:271`. See [`modes::TOPOLOGY_ALL_LOOPED_PAIRS`].
    pub fn topology_all_looped_pairs(&self) -> Result<Vec<String>, EngineError> {
        self.read_mode_strings(&modes::TOPOLOGY_ALL_LOOPED_PAIRS)
    }

    /// `TopologyV(1)` `Topology.AllIsolatedBranches` — `DTopology.pas:322`. See [`modes::TOPOLOGY_ALL_ISOLATED_BRANCHES`].
    pub fn topology_all_isolated_branches(&self) -> Result<Vec<String>, EngineError> {
        self.read_mode_strings(&modes::TOPOLOGY_ALL_ISOLATED_BRANCHES)
    }

    /// `TopologyV(2)` `Topology.AllIsolatedLoads` — `DTopology.pas:357`. See [`modes::TOPOLOGY_ALL_ISOLATED_LOADS`].
    pub fn topology_all_isolated_loads(&self) -> Result<Vec<String>, EngineError> {
        self.read_mode_strings(&modes::TOPOLOGY_ALL_ISOLATED_LOADS)
    }

    // -- Solution --------------------------------------------------------------
    /// `SolutionI(1)` `Solution.Mode` — `DSolution.pas:29`. See [`modes::SOLUTION_MODE`].
    pub fn solution_mode(&self) -> Result<i32, EngineError> {
        self.read_mode_i(&modes::SOLUTION_MODE)
    }

    /// `SolutionI(3)` `Solution.Hour` — `DSolution.pas:37`. See [`modes::SOLUTION_HOUR`].
    pub fn solution_hour(&self) -> Result<i32, EngineError> {
        self.read_mode_i(&modes::SOLUTION_HOUR)
    }

    /// `SolutionI(5)` `Solution.Year` — `DSolution.pas:47`. See [`modes::SOLUTION_YEAR`].
    pub fn solution_year(&self) -> Result<i32, EngineError> {
        self.read_mode_i(&modes::SOLUTION_YEAR)
    }

    /// `SolutionI(7)` `Solution.Iterations` — `DSolution.pas:54`. See [`modes::SOLUTION_ITERATIONS`].
    pub fn solution_iterations(&self) -> Result<i32, EngineError> {
        self.read_mode_i(&modes::SOLUTION_ITERATIONS)
    }

    /// `SolutionI(22)` `Solution.ControlIterations` — `DSolution.pas:113`. See [`modes::SOLUTION_CONTROL_ITERATIONS`].
    pub fn solution_control_iterations(&self) -> Result<i32, EngineError> {
        self.read_mode_i(&modes::SOLUTION_CONTROL_ITERATIONS)
    }

    /// `SolutionI(37)` `Solution.SystemYChanged` — `DSolution.pas:192`. See [`modes::SOLUTION_SYSTEM_Y_CHANGED`].
    pub fn solution_system_y_changed(&self) -> Result<i32, EngineError> {
        self.read_mode_i(&modes::SOLUTION_SYSTEM_Y_CHANGED)
    }

    /// `SolutionI(40)` `Solution.TotalIterations` — `DSolution.pas:218`. See [`modes::SOLUTION_TOTAL_ITERATIONS`].
    pub fn solution_total_iterations(&self) -> Result<i32, EngineError> {
        self.read_mode_i(&modes::SOLUTION_TOTAL_ITERATIONS)
    }

    /// `SolutionI(41)` `Solution.MostIterationsDone` — `DSolution.pas:222`. See [`modes::SOLUTION_MOST_ITERATIONS_DONE`].
    pub fn solution_most_iterations_done(&self) -> Result<i32, EngineError> {
        self.read_mode_i(&modes::SOLUTION_MOST_ITERATIONS_DONE)
    }

    /// `SolutionI(42)` `Solution.ControlActionsDone` — `DSolution.pas:226`. See [`modes::SOLUTION_CONTROL_ACTIONS_DONE`].
    pub fn solution_control_actions_done(&self) -> Result<i32, EngineError> {
        self.read_mode_i(&modes::SOLUTION_CONTROL_ACTIONS_DONE)
    }

    /// `SolutionF(2)` `Solution.Seconds` — `DSolution.pas:312`. See [`modes::SOLUTION_SECONDS`].
    pub fn solution_seconds(&self) -> Result<f64, EngineError> {
        self.read_mode_f(&modes::SOLUTION_SECONDS)
    }

    /// `SolutionF(6)` `Solution.LoadMult` — `DSolution.pas:336`. See [`modes::SOLUTION_LOAD_MULT`].
    pub fn solution_load_mult(&self) -> Result<f64, EngineError> {
        self.read_mode_f(&modes::SOLUTION_LOAD_MULT)
    }

    /// `SolutionF(20)` `Solution.dblHour` — `DSolution.pas:400`. See [`modes::SOLUTION_DBL_HOUR`].
    pub fn solution_dbl_hour(&self) -> Result<f64, EngineError> {
        self.read_mode_f(&modes::SOLUTION_DBL_HOUR)
    }

    /// `SolutionV(1)` `Solution.IncMatrix` — `DSolution.pas:542`. See [`modes::SOLUTION_INC_MATRIX`].
    pub fn solution_inc_matrix(&self) -> Result<Vec<i32>, EngineError> {
        self.read_mode_ints(&modes::SOLUTION_INC_MATRIX)
    }

    /// `SolutionV(3)` `Solution.IncMatrixRows` — `DSolution.pas:589`. See [`modes::SOLUTION_INC_MATRIX_ROWS`].
    pub fn solution_inc_matrix_rows(&self) -> Result<Vec<String>, EngineError> {
        self.read_mode_strings(&modes::SOLUTION_INC_MATRIX_ROWS)
    }

    /// `SolutionV(4)` `Solution.IncMatrixCols` — `DSolution.pas:609`. See [`modes::SOLUTION_INC_MATRIX_COLS`].
    pub fn solution_inc_matrix_cols(&self) -> Result<Vec<String>, EngineError> {
        self.read_mode_strings(&modes::SOLUTION_INC_MATRIX_COLS)
    }

    /// `SolutionV(5)` `Solution.Laplacian` — `DSolution.pas:640`. See [`modes::SOLUTION_LAPLACIAN`].
    pub fn solution_laplacian(&self) -> Result<Vec<i32>, EngineError> {
        self.read_mode_ints(&modes::SOLUTION_LAPLACIAN)
    }

    // -- PDElements --------------------------------------------------------------
    /// `PDElementsI(1)` `PDElements.First` — `DPDELements.pas:27`. See [`modes::PD_ELEMENTS_FIRST`].
    ///
    /// `true` when the walk found an enabled PD element (and `ActiveCktElement`
    /// now points at it); `false` when the circuit holds none.
    pub fn pd_elements_first(&self) -> Result<bool, EngineError> {
        Ok(self.read_mode_i(&modes::PD_ELEMENTS_FIRST)? != 0)
    }

    /// `PDElementsI(2)` `PDElements.Next` — `DPDELements.pas:44`. See [`modes::PD_ELEMENTS_NEXT`].
    ///
    /// `false` ends the walk; the DDLL then leaves `ActiveCktElement` wherever
    /// the previous arm put it (`DPDELements.pas:44-59`), so a caller that needs
    /// a defined active element must re-select one.
    pub fn pd_elements_next(&self) -> Result<bool, EngineError> {
        Ok(self.read_mode_i(&modes::PD_ELEMENTS_NEXT)? != 0)
    }

    /// `PDElementsS(0)` `PDElements.Name` — `DPDELements.pas:226`. See [`modes::PD_ELEMENTS_NAME`].
    ///
    /// The **active** element's full `Class.Name`, not the walk cursor's: after
    /// [`Engine::pd_elements_parent_pd_element`] has hijacked `ActiveCktElement`
    /// this reads the *parent's* name (which is exactly how
    /// [`crate::capture::capture_pd_elements`] obtains it).
    pub fn pd_elements_name(&self) -> Result<String, EngineError> {
        self.read_mode_s(&modes::PD_ELEMENTS_NAME)
    }

    /// `PDElementsI(3)` `PDElements.IsShunt` — `DPDELements.pas:61`. See [`modes::PD_ELEMENTS_IS_SHUNT`].
    pub fn pd_elements_is_shunt(&self) -> Result<i32, EngineError> {
        self.read_mode_i(&modes::PD_ELEMENTS_IS_SHUNT)
    }

    /// `PDElementsI(4)` `PDElements.NumCustomers` — `DPDELements.pas:70`. See [`modes::PD_ELEMENTS_NUM_CUSTOMERS`].
    pub fn pd_elements_num_customers(&self) -> Result<i32, EngineError> {
        self.read_mode_i(&modes::PD_ELEMENTS_NUM_CUSTOMERS)
    }

    /// `PDElementsI(5)` `PDElements.TotalCustomers` — `DPDELements.pas:79`. See [`modes::PD_ELEMENTS_TOTAL_CUSTOMERS`].
    pub fn pd_elements_total_customers(&self) -> Result<i32, EngineError> {
        self.read_mode_i(&modes::PD_ELEMENTS_TOTAL_CUSTOMERS)
    }

    /// `PDElementsI(6)` `PDElements.ParentPDElement` — `DPDELements.pas:88`. See [`modes::PD_ELEMENTS_PARENT_PD_ELEMENT`].
    pub fn pd_elements_parent_pd_element(&self) -> Result<i32, EngineError> {
        self.read_mode_i(&modes::PD_ELEMENTS_PARENT_PD_ELEMENT)
    }

    /// `PDElementsI(7)` `PDElements.FromTerminal` — `DPDELements.pas:101`. See [`modes::PD_ELEMENTS_FROM_TERMINAL`].
    pub fn pd_elements_from_terminal(&self) -> Result<i32, EngineError> {
        self.read_mode_i(&modes::PD_ELEMENTS_FROM_TERMINAL)
    }

    /// `PDElementsI(8)` `PDElements.SectionID` — `DPDELements.pas:110`. See [`modes::PD_ELEMENTS_SECTION_ID`].
    pub fn pd_elements_section_id(&self) -> Result<i32, EngineError> {
        self.read_mode_i(&modes::PD_ELEMENTS_SECTION_ID)
    }

    /// `PDElementsF(0)` `PDElements.FaultRate` — `DPDELements.pas:133`. See [`modes::PD_ELEMENTS_FAULT_RATE`].
    pub fn pd_elements_fault_rate(&self) -> Result<f64, EngineError> {
        self.read_mode_f(&modes::PD_ELEMENTS_FAULT_RATE)
    }

    /// `PDElementsF(2)` `PDElements.PctPermanent` — `DPDELements.pas:152`. See [`modes::PD_ELEMENTS_PCT_PERMANENT`].
    pub fn pd_elements_pct_permanent(&self) -> Result<f64, EngineError> {
        self.read_mode_f(&modes::PD_ELEMENTS_PCT_PERMANENT)
    }

    /// `PDElementsF(4)` `PDElements.Lambda` — `DPDELements.pas:171`. See [`modes::PD_ELEMENTS_LAMBDA`].
    pub fn pd_elements_lambda(&self) -> Result<f64, EngineError> {
        self.read_mode_f(&modes::PD_ELEMENTS_LAMBDA)
    }

    /// `PDElementsF(5)` `PDElements.AccumulatedL` — `DPDELements.pas:181`. See [`modes::PD_ELEMENTS_ACCUMULATED_L`].
    pub fn pd_elements_accumulated_l(&self) -> Result<f64, EngineError> {
        self.read_mode_f(&modes::PD_ELEMENTS_ACCUMULATED_L)
    }

    /// `PDElementsF(6)` `PDElements.RepairTime` — `DPDELements.pas:191`. See [`modes::PD_ELEMENTS_REPAIR_TIME`].
    pub fn pd_elements_repair_time(&self) -> Result<f64, EngineError> {
        self.read_mode_f(&modes::PD_ELEMENTS_REPAIR_TIME)
    }

    /// `PDElementsF(7)` `PDElements.TotalMiles` — `DPDELements.pas:201`. See [`modes::PD_ELEMENTS_TOTAL_MILES`].
    pub fn pd_elements_total_miles(&self) -> Result<f64, EngineError> {
        self.read_mode_f(&modes::PD_ELEMENTS_TOTAL_MILES)
    }

    /// Drive a V-protocol **SET** mode: hand the caller's array in via
    /// `myPointer` + `mySize` and return the element count the DLL accepted
    /// (`mySize` out, per `DLoadShape.pas` `mySize := k - 1`).
    ///
    /// `mySize` is an **element (point) count**, not a byte count: the SET path
    /// clamps `LoopLimit := min(mySize, NumPoints)` and steps `myPointer` one
    /// element per iteration ([`vset_len`]).
    fn call_v_set(&self, f: FnV, mode: i32, data: &VData) -> i32 {
        let elems = vset_len(data);
        let (tag, mut bytes) = encode_v_set(data);
        let mut ptr: *mut c_void = bytes.as_mut_ptr() as *mut c_void;
        let mut ty = tag;
        let mut size = elems;
        // SAFETY: `bytes` holds `elems` elements and outlives the call. The SET
        // path reads at most `min(size, NumPoints)` elements; with `size ==
        // elems` it never steps past `bytes`. A few setters (e.g. `DXYCurves`
        // XArray) ignore `mySize` and read the object's full `NumPoints`, so the
        // caller must supply an array of at least the target's point count (the
        // same contract the Python/Oddie bridge required). `ptr` may be
        // repointed by the DLL and is not read afterwards; the accepted element
        // count is written back into `size`.
        unsafe { f(mode, &mut ptr, &mut ty, &mut size) };
        size
    }

    // ---- Y-matrix / injection helpers (capability channel) ----------------

    /// `SystemYChanged(mode, arg)` — read (mode 0) / write (mode 1) the flag.
    pub fn ym_system_y_changed(&self, mode: i32, arg: i32) -> i32 {
        // SAFETY: `SystemYChanged` is a plain `(mode, arg): longint` cdecl.
        unsafe { (self.ymatrix.system_y_changed)(mode, arg) }
    }

    /// `UseAuxCurrents(mode, arg)` — read/write the aux-currents flag.
    pub fn ym_use_aux_currents(&self, mode: i32, arg: i32) -> i32 {
        // SAFETY: plain `(mode, arg): longint` cdecl.
        unsafe { (self.ymatrix.use_aux_currents)(mode, arg) }
    }

    /// `BuildYMatrixD(BuildOps, AllocateVI)` — rebuild the system Y matrix.
    pub fn ym_build_y(&self, build_ops: i32, allocate_vi: i32) {
        // SAFETY: plain `(longint, longint)` cdecl; no out-params.
        unsafe { (self.ymatrix.build_y_matrix_d)(build_ops, allocate_vi) }
    }

    /// `ZeroInjCurr` — zero the injection-current vector.
    pub fn ym_zero_inj(&self) {
        // SAFETY: parameterless cdecl.
        unsafe { (self.ymatrix.zero_inj_curr)() }
    }

    /// `GetSourceInjCurrents` — stamp source injection currents.
    pub fn ym_get_source_inj(&self) {
        // SAFETY: parameterless cdecl.
        unsafe { (self.ymatrix.get_source_inj_currents)() }
    }

    /// `GetPCInjCurr` — stamp PC-element injection currents.
    pub fn ym_get_pc_inj(&self) {
        // SAFETY: parameterless cdecl.
        unsafe { (self.ymatrix.get_pc_inj_curr)() }
    }

    /// `AddInAuxCurrents(SType)` — fold auxiliary currents into the injection.
    pub fn ym_add_aux(&self, stype: i32) {
        // SAFETY: plain `(integer)` cdecl.
        unsafe { (self.ymatrix.add_in_aux_currents)(stype) }
    }

    /// `getVpointer` — node-voltage vector (`Solution.NodeV`), flat `[re, im, ...]`
    /// of length `2*(NumNodes+1)`, slot 0 = ground.
    pub fn v_pointer(&self, num_nodes: i32) -> Vec<f64> {
        let mut p: *mut f64 = std::ptr::null_mut();
        // SAFETY: `getVpointer` sets `p` to `Solution.NodeV`; the length is a
        // function of NumNodes. Copied out immediately (byte-view, align 1).
        unsafe { (self.ymatrix.get_v_pointer)(&mut p) };
        if p.is_null() {
            return Vec::new();
        }
        copy_f64(p, 2 * (num_nodes as usize + 1))
    }

    /// `InitAndGetYparams` — factor Y and report `(nBus, nNZ)`; `None` if Y is
    /// not built (no circuit).
    pub fn y_dims(&self) -> Option<(u32, u32)> {
        let mut hy: u64 = 0;
        let mut n_bus: u32 = 0;
        let mut n_nz: u32 = 0;
        // SAFETY: out-params are valid locals; the call factors Y and reports dims.
        let ok = unsafe { (self.dll.init_and_get_yparams)(&mut hy, &mut n_bus, &mut n_nz) };
        if ok == 0 || n_bus == 0 {
            None
        } else {
            Some((n_bus, n_nz))
        }
    }

    /// `SolveSystem` — back-substitute `Y·V = I` (present injection currents) into
    /// a fresh caller-owned `NodeV` buffer of `2*(NumNodes+1)` doubles; returns the
    /// KLU status. Non-destructive to `Solution.NodeV` (writes only the caller
    /// buffer, which is discarded — this proves the external-solve entry is live).
    pub fn solve_system(&self, num_nodes: i32) -> i32 {
        let mut buf = vec![0.0f64; 2 * (num_nodes as usize + 1)];
        let mut p: *mut f64 = buf.as_mut_ptr();
        // SAFETY: `solve_system` takes `var NodeV` (**double); it back-substitutes
        // into the array `p` points at (our `buf`, sized `2*(NumNodes+1)` doubles).
        // `buf` outlives the call; we ignore any repointed `p`.
        unsafe { (self.ymatrix.solve_system)(&mut p) }
    }

    /// Escalate any read error accumulated during a capture block.
    pub fn assert_clean(&self, ctx: &str) -> Result<(), EngineError> {
        self.check_read(ctx)
    }
}

/// Assembled system-Y CSC copied out of the DLL.
pub struct Ycsc {
    pub n: usize,
    pub col_ptr: Vec<i32>,
    pub row_idx: Vec<i32>,
    /// Interleaved `[re, im, ...]`, `2*nnz` long.
    pub vals: Vec<f64>,
}

/// Decode a V-protocol string buffer (type tag 4) into the token list
/// dss-python's Oddie backend produces for the *same* bytes.
///
/// The Pascal writes `name\0` per element (so the buffer ends with `\0`), and the
/// header path writes one extra `\0`. Oddie reads the buffer as NUL-terminated
/// C-strings until it is consumed (token count == number of `\0`). Empirically
/// (ground truth = the since-retired `tools/opendss/xcheck_bridge.py`): drop exactly one trailing
/// `\0`, split on `\0`, and lstrip a single leading space from element 0 (the
/// monitor-header first-column artifact — `[' V1', ...] → ['V1', ...]`).
///
/// Applying the strip to element 0 of EVERY string array is safe, not by
/// coincidence of the current universe but by DSS grammar: the other V-protocol
/// string arrays (node order, element/register/variable names, zone lists) are
/// whitespace-delimited DSS identifiers, which can never begin with a space, so
/// the strip is a guaranteed no-op on them. The monitor CSV header — the one
/// array whose first token carries a leading space — is exactly the intended
/// target. Cross-checked bit-for-bit against Oddie over the full r4133 universe
/// (`xcheck_bridge.py`).
pub fn decode_string_array(bytes: &[u8]) -> Vec<String> {
    if bytes.is_empty() {
        return Vec::new();
    }
    let b = if *bytes.last().unwrap() == 0 {
        &bytes[..bytes.len() - 1]
    } else {
        bytes
    };
    let mut out: Vec<String> = b
        .split(|&c| c == 0)
        .map(|seg| String::from_utf8_lossy(seg).into_owned())
        .collect();
    if let Some(first) = out.first_mut()
        && let Some(stripped) = first.strip_prefix(' ')
    {
        *first = stripped.to_string();
    }
    out
}

fn copy_i32(p: *const i32, len: usize) -> Vec<i32> {
    if p.is_null() || len == 0 {
        return Vec::new();
    }
    // SAFETY: caller guarantees `p` addresses `len` valid i32; copy via a byte
    // view (align 1) then reinterpret to avoid an alignment assumption.
    let bytes = unsafe { std::slice::from_raw_parts(p as *const u8, len * 4) };
    bytes
        .chunks_exact(4)
        .map(|c| i32::from_le_bytes(c.try_into().unwrap()))
        .collect()
}

fn copy_f64(p: *const f64, len: usize) -> Vec<f64> {
    if p.is_null() || len == 0 {
        return Vec::new();
    }
    // SAFETY: caller guarantees `p` addresses `len` valid f64; byte-view copy.
    let bytes = unsafe { std::slice::from_raw_parts(p as *const u8, len * 8) };
    bytes
        .chunks_exact(8)
        .map(|c| f64::from_le_bytes(c.try_into().unwrap()))
        .collect()
}

// Keep `c_char` used even if the accessor set changes.
const _: fn() = || {
    let _p: *const c_char = std::ptr::null();
};
