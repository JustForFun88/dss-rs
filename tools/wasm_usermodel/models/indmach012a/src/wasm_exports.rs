//! The wasm boundary — the 15 exports of the frozen ABI
//! (`docs/wasm/USERMODEL_ABI.md` §1, Generator 15-function form) plus
//! `dss_alloc`, over the record shuttle of ABI §2. This module replaces the
//! Pascal DLL plumbing (`IndMach012a.dpr` exports clause `:33-51`,
//! `MainUnit.pas` Stdcall shims): pointers arriving from the host are
//! addresses inside guest linear memory that the host previously obtained
//! from `dss_alloc`, so every access resolves through the allocation
//! registry — no raw-pointer dereference anywhere.
//!
//! Failure policy (ABI §6): protocol violations (pointer outside a
//! `dss_alloc` allocation, OOB range) and upstream crash-class paths panic;
//! with `panic = "abort"` that is a wasm trap the host surfaces as a hard,
//! attributed engine error.

#![cfg(target_arch = "wasm32")]

use crate::cmath::{Complex, CZERO};
use crate::mainunit::{self, MainState};
use crate::records::{
    decode_dynamics_rec, decode_generator_vars, encode_generator_vars, read_complex, write_complex,
    DynamicsRec, GeneratorVars, DYNAMICS_REC_SIZE, GENERATOR_VARS_SIZE,
};
use std::collections::BTreeMap;
use std::sync::Mutex;

// The single host import the fixture uses (P3 census: `MsgCallBack` only) —
// module `dss_env` per ABI §4 slot 1. `ptr` points at the message bytes in
// guest memory (a Rust string constant's storage).
#[link(wasm_import_module = "dss_env")]
extern "C" {
    fn msg_callback(ptr: i32, len: i32);
}

/// Guest-global state: the allocation registry backing `dss_alloc` plus the
/// `MainUnit` model registry. One instance per wasm instance (each element
/// binding gets its own `Store`/instance per plan §2.1, so per-guest global
/// state matches the native DLL's per-process globals).
struct Guest {
    allocs: BTreeMap<u32, Box<[u8]>>,
    main: MainState,
}

static GUEST: Mutex<Guest> = Mutex::new(Guest {
    allocs: BTreeMap::new(),
    main: MainState::new(),
});

fn with_guest<R>(f: impl FnOnce(&mut Guest) -> R) -> R {
    let mut g = GUEST.lock().unwrap_or_else(|p| p.into_inner());
    f(&mut g)
}

/// Resolve a host-passed pointer to a mutable slice of `len` bytes inside a
/// registered allocation. The host allocates every shuttle buffer via
/// `dss_alloc` (ABI §2), so anything else is a protocol violation → trap.
fn region_mut(allocs: &mut BTreeMap<u32, Box<[u8]>>, ptr: i32, len: usize) -> &mut [u8] {
    let p = ptr as u32;
    let (base, buf) = allocs
        .range_mut(..=p)
        .next_back()
        .unwrap_or_else(|| panic!("indmach012a: pointer {ptr:#x} outside guest allocations"));
    let off = (p - *base) as usize;
    assert!(
        off <= buf.len() && len <= buf.len() - off,
        "indmach012a: out-of-bounds buffer access at {ptr:#x} len {len}"
    );
    &mut buf[off..off + len]
}

fn region(allocs: &BTreeMap<u32, Box<[u8]>>, ptr: i32, len: usize) -> &[u8] {
    let p = ptr as u32;
    let (base, buf) = allocs
        .range(..=p)
        .next_back()
        .unwrap_or_else(|| panic!("indmach012a: pointer {ptr:#x} outside guest allocations"));
    let off = (p - *base) as usize;
    assert!(
        off <= buf.len() && len <= buf.len() - off,
        "indmach012a: out-of-bounds buffer access at {ptr:#x} len {len}"
    );
    &buf[off..off + len]
}

fn read_genvars(g: &Guest, ptr: i32) -> GeneratorVars {
    decode_generator_vars(region(&g.allocs, ptr, GENERATOR_VARS_SIZE))
}

fn write_genvars(g: &mut Guest, ptr: i32, gv: &GeneratorVars) {
    encode_generator_vars(gv, region_mut(&mut g.allocs, ptr, GENERATOR_VARS_SIZE));
}

fn read_dynarec(g: &Guest, ptr: i32) -> DynamicsRec {
    decode_dynamics_rec(region(&g.allocs, ptr, DYNAMICS_REC_SIZE))
}

fn read_v3(g: &Guest, ptr: i32) -> [Complex; 3] {
    let buf = region(&g.allocs, ptr, 48);
    [
        read_complex(buf, 1),
        read_complex(buf, 2),
        read_complex(buf, 3),
    ]
}

fn write_i3(g: &mut Guest, ptr: i32, arr: &[Complex; 3]) {
    let buf = region_mut(&mut g.allocs, ptr, 48);
    write_complex(buf, 1, arr[0]);
    write_complex(buf, 2, arr[1]);
    write_complex(buf, 3, arr[2]);
}

/// The active model's stored record-buffer addresses (Pascal `GenData`/
/// `DynaData` pointers). `None` ⇔ `ActiveModel = Nil`.
fn active_ptrs(g: &Guest) -> Option<(i32, i32)> {
    g.main
        .active
        .and_then(|i| g.main.models[i].as_ref())
        .map(|m| (m.gen_data_ptr, m.dyna_data_ptr))
}

/// ABI common export: guest-owned allocator. Returns 0 for a non-positive
/// size (the host treats 0 as a protocol failure, ABI §6).
#[no_mangle]
pub extern "C" fn dss_alloc(size: i32) -> i32 {
    if size <= 0 {
        return 0;
    }
    with_guest(|g| {
        let buf = vec![0u8; size as usize].into_boxed_slice();
        let addr = buf.as_ptr() as usize as u32;
        g.allocs.insert(addr, buf);
        addr as i32
    })
}

/// Pascal `MainUnit.New` over the ABI `new(genvars, dynarec) -> id`
/// (Generator form). The two pointers are retained for the instance's
/// lifetime, exactly like the Pascal `GenData := @GenVars` capture.
#[no_mangle]
pub extern "C" fn new(genvars: i32, dynarec: i32) -> i32 {
    with_guest(|g| {
        let mut gv = read_genvars(g, genvars);
        let id = mainunit::new_instance(&mut g.main, &mut gv, genvars, dynarec);
        write_genvars(g, genvars, &gv); // Create mutates Speed (Slip := -0.007)
        id
    })
}

/// Pascal `MainUnit.Delete` (ABI `delete(id)`).
#[no_mangle]
pub extern "C" fn delete(id: i32) {
    with_guest(|g| mainunit::delete(&mut g.main, id));
}

/// Pascal `MainUnit.Select` (ABI `select(id) -> i32`).
#[no_mangle]
pub extern "C" fn select(id: i32) -> i32 {
    with_guest(|g| mainunit::select(&mut g.main, id))
}

/// Pascal `MainUnit.Edit` (ABI `edit(ptr, len)`): the host writes the ANSI
/// `UserData=` string (NUL-terminated) into a `dss_alloc` buffer. Upstream
/// converts the pAnsiChar up to the NUL (`ModelParser.CmdString := S`), so
/// the effective string stops at the first NUL within `len` bytes.
#[no_mangle]
pub extern "C" fn edit(ptr: i32, len: i32) {
    with_guest(|g| {
        let Some((gv_ptr, _)) = active_ptrs(g) else {
            return;
        };
        let raw = region(&g.allocs, ptr, len.max(0) as usize);
        let n = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
        let s = raw[..n].to_vec();
        let mut gv = read_genvars(g, gv_ptr);
        let mut msg = |bytes: &[u8]| {
            // SAFETY-free call impossible: host imports are extern; the
            // pointer/len describe bytes in guest linear memory.
            unsafe { msg_callback(bytes.as_ptr() as usize as i32, bytes.len() as i32) }
        };
        mainunit::edit(&mut g.main, &s, &mut gv, &mut msg);
        write_genvars(g, gv_ptr, &gv);
    });
}

/// Pascal `MainUnit.Init` (ABI `init(v, i)`), dynamics-mode entry.
#[no_mangle]
pub extern "C" fn init(v: i32, i: i32) {
    with_guest(|g| {
        let Some((gv_ptr, _)) = active_ptrs(g) else {
            return;
        };
        let varr = read_v3(g, v);
        let iarr = read_v3(g, i);
        let mut gv = read_genvars(g, gv_ptr);
        mainunit::init(&mut g.main, &varr, &iarr, &mut gv);
        write_genvars(g, gv_ptr, &gv);
    });
}

/// Pascal `MainUnit.Calc` (ABI `calc(v, i)`): reads V, dispatches on the
/// refreshed `TDynamicsRec.SolutionMode`, writes I (first 3 entries — the
/// model is three-phase, like the native DLL).
#[no_mangle]
pub extern "C" fn calc(v: i32, i: i32) {
    with_guest(|g| {
        let Some((gv_ptr, dyn_ptr)) = active_ptrs(g) else {
            return;
        };
        let varr = read_v3(g, v);
        let dyna = read_dynarec(g, dyn_ptr);
        let mut gv = read_genvars(g, gv_ptr);
        let mut iarr = [CZERO; 3];
        let wrote = mainunit::calc(&mut g.main, &varr, &mut iarr, &mut gv, &dyna);
        if wrote {
            write_i3(g, i, &iarr);
        }
        write_genvars(g, gv_ptr, &gv);
    });
}

/// Pascal `MainUnit.Integrate` (ABI `integrate()`).
#[no_mangle]
pub extern "C" fn integrate() {
    with_guest(|g| {
        let Some((gv_ptr, dyn_ptr)) = active_ptrs(g) else {
            return;
        };
        let gv = read_genvars(g, gv_ptr);
        let dyna = read_dynarec(g, dyn_ptr);
        mainunit::integrate(&mut g.main, &gv, &dyna);
    });
}

/// Pascal `MainUnit.Save` (ABI `save()`) — empty upstream.
#[no_mangle]
pub extern "C" fn save() {
    with_guest(|g| mainunit::save(&mut g.main));
}

/// Pascal `MainUnit.Restore` (ABI `restore()`) — empty upstream.
#[no_mangle]
pub extern "C" fn restore() {
    with_guest(|g| mainunit::restore(&mut g.main));
}

/// Pascal `MainUnit.UpdateModel` (ABI `update_model()`).
#[no_mangle]
pub extern "C" fn update_model() {
    with_guest(|g| {
        let Some((gv_ptr, _)) = active_ptrs(g) else {
            return;
        };
        let gv = read_genvars(g, gv_ptr);
        mainunit::update_model(&mut g.main, &gv);
    });
}

/// Pascal `MainUnit.NumVars` (ABI `num_vars() -> i32`).
#[no_mangle]
pub extern "C" fn num_vars() -> i32 {
    mainunit::num_vars()
}

/// Pascal `MainUnit.GetAllVars` (ABI `get_all_vars(ptr)`): writes NumVars
/// f64 at 1-based double-array offsets; nil pointer or no active model ⇒
/// buffer untouched (upstream `Vars <> NIL` check + swallowed AV).
#[no_mangle]
pub extern "C" fn get_all_vars(ptr: i32) {
    if ptr == 0 {
        return;
    }
    with_guest(|g| {
        let mut vars = [0.0f64; 14];
        if g.main
            .active
            .and_then(|i| g.main.models[i].as_ref())
            .is_none()
        {
            return;
        }
        mainunit::get_all_vars(&g.main, &mut vars);
        let buf = region_mut(&mut g.allocs, ptr, 14 * 8);
        for (k, v) in vars.iter().enumerate() {
            buf[k * 8..k * 8 + 8].copy_from_slice(&v.to_le_bytes());
        }
    });
}

/// Pascal `MainUnit.GetVariable` (ABI `get_variable(i) -> f64`).
#[no_mangle]
pub extern "C" fn get_variable(i: i32) -> f64 {
    with_guest(|g| mainunit::get_variable(&g.main, i))
}

/// Pascal `MainUnit.SetVariable` (ABI `set_variable(i, v)`).
#[no_mangle]
pub extern "C" fn set_variable(i: i32, value: f64) {
    with_guest(|g| {
        let (gv_ptr, _) = active_ptrs(g)
            .expect("indmach012a: SetVariable with no active model (upstream: access violation)");
        let mut gv = read_genvars(g, gv_ptr);
        mainunit::set_variable(&mut g.main, i, value, &mut gv);
        write_genvars(g, gv_ptr, &gv);
    });
}

/// Pascal `MainUnit.GetVarName` (ABI `get_var_name(i, ptr, maxlen)`):
/// `StrLCopy` semantics — at most `maxlen` name bytes plus a terminating
/// NUL; out-of-range index leaves the buffer untouched.
#[no_mangle]
pub extern "C" fn get_var_name(i: i32, ptr: i32, maxlen: i32) {
    with_guest(|g| {
        if let Some(bytes) = mainunit::get_var_name(i, maxlen.max(0) as u32) {
            let buf = region_mut(&mut g.allocs, ptr, bytes.len());
            buf.copy_from_slice(&bytes);
        }
    });
}
