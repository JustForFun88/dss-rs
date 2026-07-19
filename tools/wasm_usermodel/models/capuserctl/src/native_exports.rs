//! The **native twin** boundary — built ONLY to empirically confirm the WM.5
//! round-2 finding: a native `TCapUserControl` DLL loaded by the r4133 engine
//! **cannot drive the control queue**, because it has no pointer to the owning
//! `TCapControlObj` to pass as `ControlQueue.Push`'s `Owner`
//! (`ControlQueue.pas:145` — the `Owner` becomes the `ControlElement` whose
//! `DoPendingAction` runs on pop). The 7-fn `New(var CallBacks)` hands the model
//! ONLY the callback vtable (`CapUserControl.pas:36`); neither
//! `SampleControlDevices` (r4133 `Solution.pas:3611-3621`) nor `CapControl.Sample`
//! (`:885`/USERCONTROL arm `:1054-1069`) sets `ActiveCktElement` to the
//! CapControl, so `GetActiveElementPtr` returns the WRONG element.
//!
//! This shim compiles ONLY for the host target
//! (`#[cfg(not(target_arch = "wasm32"))]`) into a native cdylib the r4133 engine
//! loads as a CapControl `UserModel=` DLL. It exports the exact PascalCase
//! Stdcall names and `var`-parameter signatures the loader binds
//! (`CapUserControl.pas:36-50,176-182`): `New`, `Delete`, `Select`, `Edit`,
//! `UpdateModel`, `Sample`, `DoPending`. Its `Sample` reads the monitored node
//! voltage via the `GetPtrToSystemVarray` callback (offset 128) and the time via
//! `GetDynamicsStruct` (184), runs the SAME deadband decision as the wasm guest,
//! and attempts `ControlQueuePush` (240) with `Owner :=
//! GetActiveElementPtr()` (232) — the only element pointer it can obtain. The
//! r4133 golden generator observes that the resulting event log contains NO
//! CapControl switch, empirically confirming the channel is un-gatable.
//!
//! On Win64 there is a single calling convention, so `extern "system"` matches
//! Delphi's `stdcall`; `var` parameters are pointers. All callback offsets are
//! the frozen ABI vtable image (`docs/wasm/probes/p2_offsets_r3723.txt`:
//! `TDSSCallBacks` 256 B — `GetPtrToSystemVarray`@128, `GetDynamicsStruct`@184,
//! `GetActiveElementPtr`@232, `ControlQueuePush`@240), byte-identical r3723→r4133.

#![cfg(not(target_arch = "wasm32"))]
#![allow(clippy::missing_safety_doc)]

use crate::records::{decode_dynamics_rec, read_complex, DYNAMICS_REC_SIZE};
use crate::Registry;
use std::sync::Mutex;

// Callback signatures (Stdcall = extern "system" on win64; `var` params are
// pointers). Only the four slots the twin uses are typed.
type GetPtrToSystemVarrayFn = unsafe extern "system" fn(v: *mut *const u8, n: *mut i32);
type GetDynamicsStructFn = unsafe extern "system" fn(p: *mut *const u8);
type GetActiveElementPtrFn = unsafe extern "system" fn() -> *const u8;
type ControlQueuePushFn =
    unsafe extern "system" fn(hour: i32, sec: f64, code: i32, proxy: i32, owner: *const u8) -> i32;

// Vtable byte offsets (p2_offsets_r3723.txt).
const OFF_GET_PTR_TO_SYSTEM_VARRAY: usize = 128;
const OFF_GET_DYNAMICS_STRUCT: usize = 184;
const OFF_GET_ACTIVE_ELEMENT_PTR: usize = 232;
const OFF_CONTROL_QUEUE_PUSH: usize = 240;

/// Process-global twin state (the native DLL is loaded once per r4133 process).
struct NativeState {
    reg: Registry,
    /// The `TDSSCallBacks` vtable pointer captured at `New`.
    callbacks: usize,
}

static STATE: Mutex<NativeState> = Mutex::new(NativeState {
    reg: Registry::new(),
    callbacks: 0,
});

fn with_state<R>(f: impl FnOnce(&mut NativeState) -> R) -> R {
    let mut s = STATE.lock().unwrap_or_else(|p| p.into_inner());
    f(&mut s)
}

/// Read the function pointer at vtable byte `offset`.
///
/// # Safety
/// `callbacks` must be a valid `TDSSCallBacks` pointer captured at `New`.
unsafe fn slot(callbacks: usize, offset: usize) -> usize {
    // SAFETY: caller contract — `callbacks` points at the ≥256-byte vtable.
    unsafe { *((callbacks + offset) as *const usize) }
}

/// Pascal `New(var CallBacks: TDSSCallBacks): Integer` — capture the vtable.
///
/// # Safety
/// `callbacks` points to the engine's `TDSSCallBacks` record.
#[export_name = "New"]
pub unsafe extern "system" fn new_(callbacks: *mut u8) -> i32 {
    with_state(|s| {
        s.callbacks = callbacks as usize;
        s.reg.new_instance()
    })
}

/// Pascal `Delete(var x: Integer)`.
#[export_name = "Delete"]
pub unsafe extern "system" fn delete_(x: *mut i32) {
    // SAFETY: caller passes @Id.
    let id = unsafe { *x };
    with_state(|s| s.reg.delete(id));
}

/// Pascal `Select(var x: Integer): Integer`.
#[export_name = "Select"]
pub unsafe extern "system" fn select_(x: *mut i32) -> i32 {
    let id = unsafe { *x };
    with_state(|s| s.reg.select(id))
}

/// Pascal `Edit(s: pAnsiChar; Maxlen: Cardinal)`.
#[export_name = "Edit"]
pub unsafe extern "system" fn edit_(s: *const u8, maxlen: u32) {
    // SAFETY: caller passes a pAnsiChar of length `maxlen`.
    let raw = unsafe { std::slice::from_raw_parts(s, maxlen as usize) };
    let n = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
    let text = String::from_utf8_lossy(&raw[..n]).into_owned();
    with_state(|st| {
        if let Some(m) = st.reg.active_mut() {
            m.edit(&text);
        }
    });
}

/// Pascal `UpdateModel`.
#[export_name = "UpdateModel"]
pub extern "system" fn update_model_() {}

/// Pascal `Sample` — read the monitored node voltage + time via the callbacks,
/// run the deadband decision, and attempt `ControlQueuePush(Owner :=
/// GetActiveElementPtr())`. The owner is NOT the CapControl (the asymmetry), so
/// the r4133 engine cannot execute the switch — the empirical confirmation.
#[export_name = "Sample"]
pub extern "system" fn sample_() {
    with_state(|s| {
        let cb = s.callbacks;
        if cb == 0 {
            return;
        }
        let Some(node) = s.reg.active_ref().map(|m| m.node) else {
            return;
        };

        // Time (GetDynamicsStruct → 52-byte TDynamicsRec).
        // SAFETY: `cb` is the vtable; the slot is the engine's GetDynamicsStruct.
        let get_dyn: GetDynamicsStructFn = unsafe { std::mem::transmute(slot(cb, OFF_GET_DYNAMICS_STRUCT)) };
        let mut dyn_ptr: *const u8 = std::ptr::null();
        unsafe { get_dyn(&mut dyn_ptr) };
        let (int_hour, t) = if dyn_ptr.is_null() {
            (0, 0.0)
        } else {
            // SAFETY: `dyn_ptr` is the engine's Solution.DynaVars (≥52 B).
            let buf = unsafe { std::slice::from_raw_parts(dyn_ptr, DYNAMICS_REC_SIZE) };
            let dr = decode_dynamics_rec(buf);
            (dr.int_hour, dr.t)
        };

        // Node voltage (GetPtrToSystemVarray → &NodeV[0], ground at index 0).
        // SAFETY: the slot is the engine's GetPtrToSystemVarray.
        let get_v: GetPtrToSystemVarrayFn = unsafe { std::mem::transmute(slot(cb, OFF_GET_PTR_TO_SYSTEM_VARRAY)) };
        let mut v_ptr: *const u8 = std::ptr::null();
        let mut n_nodes: i32 = 0;
        unsafe { get_v(&mut v_ptr, &mut n_nodes) };
        let vmag = if v_ptr.is_null() || node < 1 || node > n_nodes as usize {
            0.0
        } else {
            // NodeV[0]=ground, node k at index k → read_complex 1-based (node+1).
            // SAFETY: `v_ptr` addresses NodeV[0..NumNodes] (≥ (n+1) complex).
            let buf = unsafe { std::slice::from_raw_parts(v_ptr, (n_nodes as usize + 1) * 16) };
            read_complex(buf, node + 1).abs()
        };

        let decision = s.reg.active_mut().and_then(|m| m.decide(vmag));
        if let Some(code) = decision {
            // The only element pointer the model can obtain — NOT the CapControl.
            // SAFETY: the slot is the engine's GetActiveElementPtr.
            let get_elem: GetActiveElementPtrFn = unsafe { std::mem::transmute(slot(cb, OFF_GET_ACTIVE_ELEMENT_PTR)) };
            let owner = unsafe { get_elem() };
            // SAFETY: the slot is the engine's ControlQueuePush.
            let push: ControlQueuePushFn = unsafe { std::mem::transmute(slot(cb, OFF_CONTROL_QUEUE_PUSH)) };
            unsafe {
                push(int_hour, t, code, 0, owner);
            }
        }
    });
}

/// Pascal `DoPending(var Code, ProxyHdl: Integer)` — no-op (the switch block
/// acts on `PendingChange`; this model keeps no per-action state).
#[export_name = "DoPending"]
pub unsafe extern "system" fn do_pending_(_code: *mut i32, _proxy: *mut i32) {}
