//! The wasm boundary — the 7 CapControl exports of the frozen ABI
//! (`docs/wasm/USERMODEL_ABI.md` §1, CapControl 7-function form) plus
//! `dss_alloc`. The model reads its context through three `dss_env` host imports
//! (ABI §4) — `get_node_voltages` (row 17), `get_dynamics_rec` (row 24) and
//! `control_queue_push` (row 31) — and holds NO boundary record (`sample()`
//! takes no arguments, ABI §2.5).
//!
//! Pointers passed to the host imports are guest-owned scratch buffers (Rust
//! heap inside linear memory); the host writes the read results there through
//! wasmi's `Memory`, then the guest decodes them. Failure policy (ABI §6): a
//! pointer/length fault inside the guest panics; with `panic = "abort"` that is
//! a wasm trap the host surfaces as a hard, attributed engine error.

#![cfg(target_arch = "wasm32")]

use crate::records::{decode_dynamics_rec, read_complex, COMPLEX_SIZE, DYNAMICS_REC_SIZE};
use crate::Registry;
use std::sync::Mutex;

// The host imports the model uses (module `dss_env`, ABI §4). `get_node_voltages`
// writes up to `max` `Complex` node voltages (ground-excluded, node k at
// `(k-1)*16`) into `dest` and returns the count; `get_dynamics_rec` writes the
// 52-byte `TDynamicsRec`; `control_queue_push` schedules an open/close.
#[link(wasm_import_module = "dss_env")]
extern "C" {
    fn get_node_voltages(dest: i32, max: i32) -> i32;
    fn get_dynamics_rec(dest: i32);
    fn control_queue_push(hour: i32, sec: f64, code: i32, proxy_hdl: i32) -> i32;
}

/// Guest-global state: the model registry (one model per wasm instance).
struct Guest {
    reg: Registry,
}

static GUEST: Mutex<Guest> = Mutex::new(Guest {
    reg: Registry::new(),
});

fn with_guest<R>(f: impl FnOnce(&mut Guest) -> R) -> R {
    let mut g = GUEST.lock().unwrap_or_else(|p| p.into_inner());
    f(&mut g)
}

/// ABI common export: guest-owned allocator (the host allocates its per-instance
/// buffers, but this model needs none — it uses local scratch; kept for the ABI
/// contract / export-set validation).
#[no_mangle]
pub extern "C" fn dss_alloc(size: i32) -> i32 {
    if size <= 0 {
        return 0;
    }
    // Leak a stable region so the returned pointer is valid for the instance
    // lifetime (matches the native malloc contract).
    let buf = vec![0u8; size as usize].into_boxed_slice();
    Box::leak(buf).as_ptr() as usize as i32
}

/// ABI `new() -> id` (CapControl form: the callback vtable has no wasm
/// counterpart, ABI §1).
#[no_mangle]
pub extern "C" fn new() -> i32 {
    with_guest(|g| g.reg.new_instance())
}

/// ABI `delete(id)`.
#[no_mangle]
pub extern "C" fn delete(id: i32) {
    with_guest(|g| g.reg.delete(id));
}

/// ABI `select(id) -> i32`.
#[no_mangle]
pub extern "C" fn select(id: i32) -> i32 {
    with_guest(|g| g.reg.select(id))
}

/// ABI `edit(ptr, len)` — the `UserData=` string (NUL-terminated).
#[no_mangle]
pub extern "C" fn edit(ptr: i32, len: i32) {
    let n = len.max(0) as usize;
    // SAFETY: the host wrote `len` bytes at `ptr` in guest memory before the call.
    let raw = unsafe { std::slice::from_raw_parts(ptr as usize as *const u8, n) };
    let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
    let s = String::from_utf8_lossy(&raw[..end]).into_owned();
    with_guest(|g| {
        if let Some(m) = g.reg.active_mut() {
            m.edit(&s);
        }
    });
}

/// ABI `update_model()` — nothing to recompute.
#[no_mangle]
pub extern "C" fn update_model() {}

/// ABI `sample()` — read the monitored node voltage + schedule time, run the
/// deadband decision, and schedule an open/close on the control queue.
#[no_mangle]
pub extern "C" fn sample() {
    with_guest(|g| {
        let Some(node) = g.reg.active_ref().map(|m| m.node) else {
            return;
        };

        // Read the schedule time from the dynamics record.
        let mut dyn_buf = [0u8; DYNAMICS_REC_SIZE];
        // SAFETY: `dyn_buf` is a valid 52-byte guest region the host writes into.
        unsafe { get_dynamics_rec(dyn_buf.as_mut_ptr() as usize as i32) };
        let dr = decode_dynamics_rec(&dyn_buf);

        // Read up to `node` node voltages (ground-excluded); node k at (k-1)*16.
        let mut v_buf = vec![0u8; node.max(1) * COMPLEX_SIZE];
        // SAFETY: `v_buf` holds `node` complex slots the host writes into.
        let count = unsafe { get_node_voltages(v_buf.as_mut_ptr() as usize as i32, node as i32) };
        let vmag = if node >= 1 && (count as usize) >= node {
            read_complex(&v_buf, node).abs()
        } else {
            0.0
        };

        // Decide + schedule (a plain queue push at the current time, ABI §2.5).
        let decision = g.reg.active_mut().and_then(|m| m.decide(vmag));
        if let Some(code) = decision {
            // SAFETY: no memory crosses; a plain queue push.
            unsafe {
                control_queue_push(dr.int_hour, dr.t, code, 0);
            }
        }
    });
}

/// ABI `do_pending(code, proxy_hdl)` — the host has set `PendingChange := code`
/// and the shared switch block acts on it (ABI §2.5); this model keeps no
/// per-action state, so the hook is a no-op.
#[no_mangle]
pub extern "C" fn do_pending(_code: i32, _proxy_hdl: i32) {}
