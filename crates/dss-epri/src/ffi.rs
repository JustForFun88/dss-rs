//! Raw `extern "C"` bindings to the official EPRI `OpenDSSDirect.dll` (r4133).
//!
//! The DDLL surface is verified against the vendored Pascal source
//! (`.inputs/electricdss-code-r4133-trunk/Version8/Source/DDLL/`,
//! `UNIFIED_GATE_PLAN.md` §7). Every exported entry point is one of five plain
//! `cdecl` shapes — no OLE Variants, no COM, no callbacks:
//!
//! - `XxxI(mode: longint, arg: longint): longint`      → [`FnI`]
//! - `XxxF(mode: longint, arg: double): double`        → [`FnF`]
//! - `XxxS(mode: longint, arg: PAnsiChar): PAnsiChar`  → [`FnS`]
//! - `XxxV(mode: longint; var ptr; var type, size)`    → [`FnV`] (the "V-protocol":
//!   the DLL fills a global dynamic array and hands back a borrowed pointer + a
//!   type tag + a byte size; the caller copies immediately and never frees).
//! - plus the flat Y-matrix / injection helpers.
//!
//! # SAFETY (module-wide invariant)
//! Every function pointer in [`DllFns`] is obtained from a `libloading`
//! [`Library`] that [`crate::dss::Engine`] deliberately **leaks** ([`Dll::leak`]),
//! so the DLL stays mapped for the whole process and the pointers never dangle
//! (unloading it deadlocks — see the `Engine` docs). The DLL is single-threaded
//! per process; the `epri-worker` binary drives exactly one in-flight request at
//! a time. V-protocol pointers borrow DLL-owned heap that stays valid until the
//! next call into the same family, so the safe wrapper in [`crate::dss`] copies
//! the bytes out before issuing any further call. The signatures below are
//! transcribed 1:1 from the Pascal `interface` declarations, so a call with the
//! documented arguments cannot read or write out of bounds on the Rust side.

use std::ffi::{CString, OsStr, c_char, c_void};
use std::path::Path;

use libloading::os::windows::{Library, Symbol};

/// `XxxI(mode, arg): longint` — integer scalar in/out.
pub type FnI = unsafe extern "C" fn(i32, i32) -> i32;
/// `XxxF(mode, arg): double` — float scalar in/out.
pub type FnF = unsafe extern "C" fn(i32, f64) -> f64;
/// `XxxS(mode, arg): PAnsiChar` — string in/out (borrowed `\0`-terminated bytes).
pub type FnS = unsafe extern "C" fn(i32, *const c_char) -> *const c_char;
/// `XxxV(mode; var ptr; var type; var size)` — the V-protocol array getter.
pub type FnV = unsafe extern "C" fn(i32, *mut *mut c_void, *mut i32, *mut i32);

/// `LoadLibraryExW` flag: resolve the DLL's own dependencies (`KLUSolve.dll`)
/// from its directory rather than the process search path.
const LOAD_WITH_ALTERED_SEARCH_PATH: u32 = 0x0000_0008;

/// The typed entry points, copied out of the loaded library. `Copy`/`Send`: the
/// fn pointers borrow nothing, so they can cross threads (the bridge loads the
/// DLL on a dedicated "main" thread but calls it from another — see
/// [`crate::dss::Engine`]). Their validity is upheld by keeping the owning
/// [`Dll`]'s `Library` loaded for the whole session (module SAFETY invariant).
#[derive(Clone, Copy)]
pub struct DllFns {
    pub dss_put_command: unsafe extern "C" fn(*const c_char) -> *const c_char,
    pub error_code: unsafe extern "C" fn() -> i32,
    pub error_desc: unsafe extern "C" fn() -> *const c_char,

    pub dss_i: FnI,
    pub dss_s: FnS,

    pub circuit_i: FnI,
    pub circuit_s: FnS,
    pub circuit_v: FnV,

    pub ckt_element_i: FnI,
    pub ckt_element_s: FnS,
    pub ckt_element_v: FnV,

    pub solution_i: FnI,
    pub solution_f: FnF,

    pub monitors_i: FnI,
    pub monitors_s: FnS,
    pub monitors_v: FnV,

    pub meters_i: FnI,
    pub meters_s: FnS,
    pub meters_v: FnV,

    pub transformers_i: FnI,
    pub transformers_f: FnF,
    pub transformers_s: FnS,

    pub reg_controls_i: FnI,
    pub reg_controls_s: FnS,

    pub capacitors_i: FnI,
    pub capacitors_s: FnS,
    pub capacitors_v: FnV,

    pub ctrl_queue_v: FnV,

    /// `ParallelV` — mode 1 returns `ActorStatus[]` (0 = actor busy solving,
    /// 1 = done). The DDLL `solve` dispatches asynchronously to the actor thread
    /// and returns immediately (Solution.pas `TSolutionObj.Solve`), so results
    /// must not be read until the actor finishes.
    pub parallel_v: FnV,

    /// `InitAndGetYparams(var hY; var nBus, nNZ): Longword` — factors Y (always)
    /// and returns the CSC dimensions. `hY` is `NativeUInt` (`u64` on Win64).
    pub init_and_get_yparams: unsafe extern "C" fn(*mut u64, *mut u32, *mut u32) -> u32,
    /// `GetCompressedYMatrix(hY; nBus, nNZ; var ColPtr, RowIdx; var cVals)` — the
    /// out-pointers are set to DLL-owned heap (`ColPtr[nBus+1]`, `RowIdx[nNZ]`,
    /// `cVals[nNZ]` complex).
    pub get_compressed_y_matrix:
        unsafe extern "C" fn(u64, u32, u32, *mut *mut i32, *mut *mut i32, *mut *mut f64),
    /// `getIpointer(var IvectorPtr)` — borrowed pointer to the internal injection
    /// current vector (length `2*(NumNodes+1)` doubles, slot 0 = ground).
    pub get_i_pointer: unsafe extern "C" fn(*mut *mut f64),
}

// SAFETY: `DllFns` is only bare function pointers (no interior state); sending it
// to the calling thread is sound because the DLL stays loaded for the session.
unsafe impl Send for DllFns {}

/// The loaded r4133 DLL: the entry points ([`DllFns`]) plus the owning
/// [`Library`]. Dropping this `FreeLibrary`s the DLL — which deadlocks in the
/// r4133 finalization (see [`crate::dss::Engine`]); use [`Dll::leak`] to keep it
/// loaded for the session instead.
pub struct Dll {
    pub fns: DllFns,
    lib: Library,
}

impl Dll {
    /// Leak the loaded library so it is never `FreeLibrary`'d (the entry points in
    /// [`Dll::fns`] stay valid for the rest of the process — see the [`Dll`] and
    /// [`crate::dss::Engine`] docs for why unloading must be avoided).
    pub fn leak(self) {
        std::mem::forget(self.lib);
    }
}

/// Copy a symbol's function pointer out of the library.
///
/// # Safety
/// The named symbol must have exactly the ABI/signature of `T` (verified against
/// the Pascal `interface`), and the returned value is only valid while `lib`
/// stays loaded — which [`Dll`] guarantees by owning `lib`.
unsafe fn sym<T: Copy>(lib: &Library, name: &[u8]) -> Result<T, String> {
    // SAFETY: `name` is a NUL-terminated symbol name; `T` matches the exported
    // signature (documented per field in `Dll`). `*s` copies the bare fn pointer
    // out of the borrowing `Symbol` wrapper.
    let s: Symbol<T> = unsafe { lib.get(name) }
        .map_err(|e| format!("symbol {}: {e}", String::from_utf8_lossy(name)))?;
    Ok(*s)
}

impl Dll {
    /// Load `OpenDSSDirect.dll` from `dll_path` with an altered search path so its
    /// sibling `KLUSolve.dll` (in the same directory) resolves, and bind every
    /// entry point the bridge needs.
    pub fn load(dll_path: &Path) -> Result<Dll, String> {
        // `LOAD_WITH_ALTERED_SEARCH_PATH` requires a fully-qualified path with no
        // `..`; and the loader's dependency search (for `KLUSolve.dll`) does not
        // like the `\\?\` verbatim prefix `canonicalize` yields — strip it so the
        // native form matches what the working ctypes probe used.
        let canon = std::fs::canonicalize(dll_path)
            .map_err(|e| format!("resolve {}: {e}", dll_path.display()))?;
        let native = canon.to_string_lossy();
        let native = native.strip_prefix(r"\\?\").unwrap_or(&native).to_string();
        // SAFETY: loading an arbitrary DLL runs its init code; the vendored EPRI
        // binary is trusted (git-tracked, SHA-pinned). The altered search path
        // makes the loader resolve `KLUSolve.dll` from the DLL's own folder.
        let lib = unsafe {
            Library::load_with_flags::<&OsStr>(OsStr::new(&native), LOAD_WITH_ALTERED_SEARCH_PATH)
        }
        .map_err(|e| format!("LoadLibraryEx {native}: {e}"))?;

        // SAFETY: each signature is transcribed from the Pascal `interface`.
        let fns = unsafe {
            DllFns {
                dss_put_command: sym(&lib, b"DSSPut_Command\0")?,
                error_code: sym(&lib, b"ErrorCode\0")?,
                error_desc: sym(&lib, b"ErrorDesc\0")?,
                dss_i: sym(&lib, b"DSSI\0")?,
                dss_s: sym(&lib, b"DSSS\0")?,
                circuit_i: sym(&lib, b"CircuitI\0")?,
                circuit_s: sym(&lib, b"CircuitS\0")?,
                circuit_v: sym(&lib, b"CircuitV\0")?,
                ckt_element_i: sym(&lib, b"CktElementI\0")?,
                ckt_element_s: sym(&lib, b"CktElementS\0")?,
                ckt_element_v: sym(&lib, b"CktElementV\0")?,
                solution_i: sym(&lib, b"SolutionI\0")?,
                solution_f: sym(&lib, b"SolutionF\0")?,
                monitors_i: sym(&lib, b"MonitorsI\0")?,
                monitors_s: sym(&lib, b"MonitorsS\0")?,
                monitors_v: sym(&lib, b"MonitorsV\0")?,
                meters_i: sym(&lib, b"MetersI\0")?,
                meters_s: sym(&lib, b"MetersS\0")?,
                meters_v: sym(&lib, b"MetersV\0")?,
                transformers_i: sym(&lib, b"TransformersI\0")?,
                transformers_f: sym(&lib, b"TransformersF\0")?,
                transformers_s: sym(&lib, b"TransformersS\0")?,
                reg_controls_i: sym(&lib, b"RegControlsI\0")?,
                reg_controls_s: sym(&lib, b"RegControlsS\0")?,
                capacitors_i: sym(&lib, b"CapacitorsI\0")?,
                capacitors_s: sym(&lib, b"CapacitorsS\0")?,
                capacitors_v: sym(&lib, b"CapacitorsV\0")?,
                ctrl_queue_v: sym(&lib, b"CtrlQueueV\0")?,
                parallel_v: sym(&lib, b"ParallelV\0")?,
                init_and_get_yparams: sym(&lib, b"InitAndGetYparams\0")?,
                get_compressed_y_matrix: sym(&lib, b"GetCompressedYMatrix\0")?,
                get_i_pointer: sym(&lib, b"getIpointer\0")?,
            }
        };
        Ok(Dll { fns, lib })
    }
}

/// Read a borrowed `\0`-terminated `PAnsiChar` the DLL returned into an owned
/// `String` (lossy UTF-8, matching dss-python's `codec='UTF8'` decode of the
/// same bytes for ASCII corpus content). A null pointer yields `""`.
///
/// # Safety
/// `p` must be null or point to a `\0`-terminated byte run owned by the DLL that
/// stays valid for the duration of this call.
pub unsafe fn cstr_to_string(p: *const c_char) -> String {
    if p.is_null() {
        return String::new();
    }
    // SAFETY: `p` is a live, DLL-owned NUL-terminated buffer (caller invariant).
    let bytes = unsafe { std::ffi::CStr::from_ptr(p) }.to_bytes();
    String::from_utf8_lossy(bytes).into_owned()
}

/// Build a NUL-terminated `CString` for passing into an `S`/command entry point.
/// Interior NULs (never present in DSS commands) are rejected.
pub fn to_cstring(s: &str) -> CString {
    CString::new(s).unwrap_or_else(|_| CString::new(s.replace('\0', "")).unwrap())
}
