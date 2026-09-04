//! Raw `extern "C"` bindings to the official EPRI `OpenDSSDirect.dll` (r4133).
//!
//! The DDLL surface is verified against the vendored Pascal source
//! (`.inputs/electricdss-code-r4133-trunk/Version8/Source/DDLL/`,
//! `UNIFIED_GATE_PLAN.md` §7). Every exported entry point is one of five plain
//! `cdecl` shapes — no OLE Variants, no COM, no callbacks:
//!
//! - `XxxI(mode: longint, arg: longint): longint`      → [`FnI`]
//! - `XxxF(mode: longint, arg: double): double`        → [`FnF`]
//!   (two families declare a second double instead — `CircuitF`/`CmathLibF`,
//!   → [`FnF2`])
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
/// `XxxF(mode, arg1, arg2): double` — the **two-double** `F` variant.
///
/// The `F` shape is not uniform across the DDLL: exactly two of the 42 families
/// declare a second double — `CircuitF(mode: longint; arg1, arg2: double)`
/// (`DCircuit.pas:27`, impl `:193`; mode 0 `Circuit.Capacity` takes
/// `CapacityStart` + `CapacityIncrement`) and `CmathLibF(mode: longint;
/// arg1, arg2: double)` (`DCmathLib.pas:5`, impl `:12`; mode 0 `Cabs`, mode 1
/// `Cdang`, both over `cmplx(arg1, arg2)`). Every other `XxxF` — and every
/// `XxxI`/`XxxS`/`XxxV` — is uniform (exhaustive sweep of the vendored DDLL
/// `interface` sections, G1.0).
///
/// # SAFETY
/// Binding these two with [`FnF`] is an ABI mismatch, not a nuisance: the second
/// double is never placed in `XMM2`, so the callee reads whatever the register
/// happened to hold. Measured against the vendored DLL before the fix
/// (2026-09-04): `CmathLibF(0, 3.0, ?)` returned `3.0` instead of
/// `Cabs(3+4j) = 5.0`. [`crate::families::TWO_DOUBLE_F`] names the two families
/// and [`crate::families::FamilyTable::load`] binds them here; the pin is
/// `crates/dss-epri/tests/protocol.rs::cmath_lib_f_takes_two_doubles`.
pub type FnF2 = unsafe extern "C" fn(i32, f64, f64) -> f64;
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

    /// `DSSElementV` — mode 0 returns the active DSS object's
    /// `ParentClass.AllPropertyNames` (`DDSSElement.pas` line 63, type tag 4,
    /// `\0`-separated). Backs the all-properties enumeration (§2.2 — report-tooling
    /// parity when written, gating since R4133_PROPS RP4.1): `? name.Like`
    /// activates the object, then this reads its property
    /// list — the WPG.1-safe path that covers terminal-less `DSS_OBJECT`s too.
    pub dss_element_v: FnV,

    pub solution_i: FnI,
    pub solution_f: FnF,
    /// `SolutionV` — mode 0 returns `Solution.EventLog` (`DSolution.pas`): the raw
    /// `EventStrings[ActiveActor]` lines (`Hour=…, Sec=…, ControlIter=…, Element=…,
    /// Action=…`), each followed by a `\0`; an empty log writes the bare `None`
    /// placeholder **without** a terminator. Backs the golden-regen parity surface
    /// (the retired Oddie `sol.EventLog` read the same entry point).
    pub solution_v: FnV,

    /// `BUSF` — mode 0 returns `Bus.kVBase` for the active bus (`DBus.pas`; select
    /// the bus first via `CircuitS(4)` SetActiveBus). Regen-driver parity surface.
    pub bus_f: FnF,

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

/// The standalone Y-matrix / injection helper exports that are **not** part of the
/// uniform `XxxI/F/S/V` family shape (`DYMatrix.pas`). Bound for capability
/// completeness (dss-python's `YMatrix.*`); reached via the worker `ymatrix`
/// command. `InitAndGetYparams` / `GetCompressedYMatrix` / `getIpointer` stay in
/// [`DllFns`] (the gate's CSC + injection capture path). `Copy`/`Send`: bare fn
/// pointers.
#[derive(Clone, Copy)]
pub struct YMatrixFns {
    /// `ZeroInjCurr` — zero the solution injection-current vector.
    pub zero_inj_curr: unsafe extern "C" fn(),
    /// `GetSourceInjCurrents` — stamp source injection currents into the vector.
    pub get_source_inj_currents: unsafe extern "C" fn(),
    /// `GetPCInjCurr` — stamp PC-element injection currents into the vector.
    pub get_pc_inj_curr: unsafe extern "C" fn(),
    /// `SystemYChanged(mode, arg): longint` — read (mode 0) / write (mode 1) the
    /// "Y needs rebuild" flag. Same ABI as [`FnI`].
    pub system_y_changed: FnI,
    /// `BuildYMatrixD(BuildOps, AllocateVI)` — rebuild the system Y matrix.
    pub build_y_matrix_d: unsafe extern "C" fn(i32, i32),
    /// `UseAuxCurrents(mode, arg): longint` — read/write the aux-currents flag.
    pub use_aux_currents: FnI,
    /// `AddInAuxCurrents(SType)` — fold auxiliary currents into the injection.
    pub add_in_aux_currents: unsafe extern "C" fn(i32),
    /// `getVpointer(var VvectorPtr)` — borrowed pointer to the node-voltage vector
    /// (`Solution.NodeV`, length `2*(NumNodes+1)` doubles, slot 0 = ground).
    pub get_v_pointer: unsafe extern "C" fn(*mut *mut f64),
    /// `SolveSystem(var NodeV): integer` — back-substitute `Y·V = I` into a
    /// caller-provided `NodeV` buffer (pointer-to-pointer), returning KLU status.
    pub solve_system: unsafe extern "C" fn(*mut *mut f64) -> i32,
}

// SAFETY: only bare function pointers; sound to send for the same reason as
// `DllFns` (the DLL stays loaded for the session).
unsafe impl Send for YMatrixFns {}

/// The loaded r4133 DLL: the entry points ([`DllFns`]) plus the owning
/// [`Library`]. Dropping this `FreeLibrary`s the DLL — which deadlocks in the
/// r4133 finalization (see [`crate::dss::Engine`]); use [`Dll::leak`] to keep it
/// loaded for the session instead.
pub struct Dll {
    pub fns: DllFns,
    /// The standalone Y-matrix helper exports (capability channel).
    pub ymatrix: YMatrixFns,
    /// The full uniform-family registry (generic `ffi` capability channel).
    pub families: crate::families::FamilyTable,
    lib: Library,
}

impl Dll {
    /// Leak the loaded library (never `FreeLibrary`'d — unloading deadlocks in
    /// r4133 finalization, see [`crate::dss::Engine`]) and hand back its bound
    /// entry points. The library stays mapped for the rest of the process, so all
    /// returned pointers stay valid.
    pub fn leak_into_parts(self) -> (DllFns, YMatrixFns, crate::families::FamilyTable) {
        let Dll {
            fns,
            ymatrix,
            families,
            lib,
        } = self;
        std::mem::forget(lib);
        (fns, ymatrix, families)
    }
}

/// Copy a symbol's function pointer out of the library.
///
/// # Safety
/// The named symbol must have exactly the ABI/signature of `T` (verified against
/// the Pascal `interface`), and the returned value is only valid while `lib`
/// stays loaded — which [`Dll`] guarantees by owning `lib`.
pub(crate) unsafe fn sym<T: Copy>(lib: &Library, name: &[u8]) -> Result<T, String> {
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
                dss_element_v: sym(&lib, b"DSSElementV\0")?,
                solution_i: sym(&lib, b"SolutionI\0")?,
                solution_f: sym(&lib, b"SolutionF\0")?,
                solution_v: sym(&lib, b"SolutionV\0")?,
                bus_f: sym(&lib, b"BUSF\0")?,
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

        // SAFETY: each Y-helper signature is transcribed 1:1 from `DYMatrix.pas`.
        let ymatrix = unsafe {
            YMatrixFns {
                zero_inj_curr: sym(&lib, b"ZeroInjCurr\0")?,
                get_source_inj_currents: sym(&lib, b"GetSourceInjCurrents\0")?,
                get_pc_inj_curr: sym(&lib, b"GetPCInjCurr\0")?,
                system_y_changed: sym(&lib, b"SystemYChanged\0")?,
                build_y_matrix_d: sym(&lib, b"BuildYMatrixD\0")?,
                use_aux_currents: sym(&lib, b"UseAuxCurrents\0")?,
                add_in_aux_currents: sym(&lib, b"AddInAuxCurrents\0")?,
                get_v_pointer: sym(&lib, b"getVpointer\0")?,
                solve_system: sym(&lib, b"SolveSystem\0")?,
            }
        };

        // SAFETY: the family symbols are the r4133 export table (verified by the
        // export dump), each an `XxxI/F/S/V(mode, arg)` cdecl.
        let families = unsafe { crate::families::FamilyTable::load(&lib)? };

        Ok(Dll {
            fns,
            ymatrix,
            families,
            lib,
        })
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
