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

use crate::families::{Family, FamilyTable, VData, decode_v, encode_v_set, vset_len};
use crate::ffi::{Dll, DllFns, FnV, YMatrixFns, cstr_to_string, to_cstring};

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

/// One generic C-API call request for [`Engine::ffi_dispatch`]. `kind` selects
/// the ABI shape (`"i"`/`"f"`/`"s"`/`"v"`); only the matching scalar
/// (`iarg`/`farg`/`sarg`) is used. `vset` (V only) drives an array-SET mode.
#[derive(Debug, Clone, Default)]
pub struct FfiCall<'a> {
    pub family: &'a str,
    pub kind: &'a str,
    pub mode: i32,
    pub iarg: i32,
    pub farg: f64,
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
const USER_MODEL: &[i32] = &[567, 570, 1570];

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
    /// → read Version. All subsequent DLL calls happen on this same thread.
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
        }
        Ok(reply)
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

    pub fn clear(&self) -> Result<(), EngineError> {
        self.command_strict("clear", "clear").map(|_| ())
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

    pub fn element_currents(&self) -> Vec<f64> {
        self.v_f64s(self.dll.ckt_element_v, 3)
    }

    pub fn element_powers(&self) -> Vec<f64> {
        self.v_f64s(self.dll.ckt_element_v, 4)
    }

    /// Total losses `[re, im]` in W/var (`CktElement.Losses`, one complex).
    pub fn element_losses(&self) -> Vec<f64> {
        self.v_f64s(self.dll.ckt_element_v, 5)
    }

    pub fn element_variable_names(&self) -> Vec<String> {
        self.v_strings(self.dll.ckt_element_v, 15)
    }

    /// The active DSS object's `ParentClass.AllPropertyNames`, in property-index
    /// order (`DSSElementV` mode 0 — `DDSSElement.pas`). Read after activating the
    /// object with `? name.Like`; empty class ⇒ `["None"]` (Pascal placeholder).
    /// Backs [`crate::capture`]'s all-properties enumeration (§2.2).
    pub fn element_all_property_names(&self) -> Vec<String> {
        self.v_strings(self.dll.dss_element_v, 0)
    }

    pub fn element_variable_values(&self) -> Vec<f64> {
        self.v_f64s(self.dll.ckt_element_v, 16)
    }

    /// Read Powers then Currents then Losses on the active element, with a single
    /// user-model retry (`oracle_server.capture_all_elements`'s `_read`): a
    /// Generator model=6 fires #567 on the first current recompute after a solve,
    /// which the recompute itself clears — so a retry returns the cached, correct
    /// Yprim-only currents.
    pub fn element_pcl(&self, warn: bool, ctx: &str) -> Result<Pcl, EngineError> {
        for attempt in 0..2 {
            let powers = self.element_powers();
            let currents = self.element_currents();
            let losses = self.element_losses();
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

    /// Dispatch one generic C-API call ([`FfiCall`]). For a `"v"` call,
    /// `vset = None` reads the array getter for `mode`; `vset = Some(_)` drives the
    /// SET mode, handing the array in via `myPointer`. The caller polls
    /// [`Engine::poll_error`] afterwards for the structured errno surface.
    pub fn ffi_dispatch(&self, call: FfiCall) -> Result<FfiOut, EngineError> {
        let FfiCall {
            family,
            kind,
            mode,
            iarg,
            farg,
            sarg,
            vset,
        } = call;
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
                let f = fam.f.ok_or_else(|| missing("F"))?;
                // SAFETY: `f` is a transcribed `XxxF(mode, arg): double` cdecl.
                Ok(FfiOut::F(unsafe { f(mode, farg) }))
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
