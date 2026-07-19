//! The wasm boundary — the 15 exports of the frozen ABI
//! (`docs/wasm/USERMODEL_ABI.md` §1) plus `dss_alloc`, over the record shuttle
//! of ABI §2. `new(dynarec) -> id` (the Storage/PVSystem form; no `GeneratorVars`
//! pointer). Pointers arriving from the host are addresses inside guest linear
//! memory the host obtained from `dss_alloc`, so every access resolves through
//! the allocation registry — no raw-pointer dereference. The model uses NO host
//! imports (`dss_env` empty) and NO `get_public_data` record.
//!
//! Failure policy (ABI §6): a pointer outside a `dss_alloc` allocation or an OOB
//! range panics; with `panic = "abort"` that is a wasm trap the host surfaces as
//! a hard, attributed engine error.

#![cfg(target_arch = "wasm32")]

use crate::records::{decode_dynamics_rec, read_complex, write_complex, DYNAMICS_REC_SIZE};
use crate::{Cx, Registry, DYNAMICMODE, NPHASES};
use std::collections::BTreeMap;
use std::sync::Mutex;

/// Guest-global state: the `dss_alloc` allocation registry, the model registry,
/// and the retained `TDynamicsRec` buffer pointer (Pascal captures `@DynaData`
/// at `New`; the host refreshes the image in place before each call).
struct Guest {
    allocs: BTreeMap<u32, Box<[u8]>>,
    reg: Registry,
    dynarec_ptr: i32,
}

static GUEST: Mutex<Guest> = Mutex::new(Guest {
    allocs: BTreeMap::new(),
    reg: Registry::new(),
    dynarec_ptr: 0,
});

fn with_guest<R>(f: impl FnOnce(&mut Guest) -> R) -> R {
    let mut g = GUEST.lock().unwrap_or_else(|p| p.into_inner());
    f(&mut g)
}

fn region_mut(allocs: &mut BTreeMap<u32, Box<[u8]>>, ptr: i32, len: usize) -> &mut [u8] {
    let p = ptr as u32;
    let (base, buf) = allocs
        .range_mut(..=p)
        .next_back()
        .unwrap_or_else(|| panic!("wm4model: pointer {ptr:#x} outside guest allocations"));
    let off = (p - *base) as usize;
    assert!(
        off <= buf.len() && len <= buf.len() - off,
        "wm4model: out-of-bounds buffer access at {ptr:#x} len {len}"
    );
    &mut buf[off..off + len]
}

fn region(allocs: &BTreeMap<u32, Box<[u8]>>, ptr: i32, len: usize) -> &[u8] {
    let p = ptr as u32;
    let (base, buf) = allocs
        .range(..=p)
        .next_back()
        .unwrap_or_else(|| panic!("wm4model: pointer {ptr:#x} outside guest allocations"));
    let off = (p - *base) as usize;
    assert!(
        off <= buf.len() && len <= buf.len() - off,
        "wm4model: out-of-bounds buffer access at {ptr:#x} len {len}"
    );
    &buf[off..off + len]
}

fn read_v3(g: &Guest, ptr: i32) -> [Cx; NPHASES] {
    let buf = region(&g.allocs, ptr, NPHASES * 16);
    [
        read_complex(buf, 1),
        read_complex(buf, 2),
        read_complex(buf, 3),
    ]
}

fn write_i3(g: &mut Guest, ptr: i32, arr: &[Cx; NPHASES]) {
    let buf = region_mut(&mut g.allocs, ptr, NPHASES * 16);
    write_complex(buf, 1, arr[0]);
    write_complex(buf, 2, arr[1]);
    write_complex(buf, 3, arr[2]);
}

/// `true` iff the retained `TDynamicsRec` says we are in DYNAMICMODE.
fn in_dynamics(g: &Guest) -> (bool, f64, bool) {
    let dyna = decode_dynamics_rec(region(&g.allocs, g.dynarec_ptr, DYNAMICS_REC_SIZE));
    (
        dyna.solution_mode == DYNAMICMODE,
        dyna.h,
        dyna.iteration_flag == 0,
    )
}

/// ABI common export: guest-owned allocator (0 for a non-positive size).
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

/// ABI `new(dynarec) -> id` (Storage/PVSystem form). Retains the dynarec buffer
/// pointer for the instance lifetime (Pascal `@DynaData` capture).
#[no_mangle]
pub extern "C" fn new(dynarec: i32) -> i32 {
    with_guest(|g| {
        g.dynarec_ptr = dynarec;
        g.reg.new_instance()
    })
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

/// ABI `edit(ptr, len)` — the `UserData=`/`DynaData=` string (NUL-terminated).
#[no_mangle]
pub extern "C" fn edit(ptr: i32, len: i32) {
    with_guest(|g| {
        let raw = region(&g.allocs, ptr, len.max(0) as usize);
        let n = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
        let s = String::from_utf8_lossy(&raw[..n]).into_owned();
        if let Some(m) = g.reg.active_mut() {
            m.edit(&s);
        }
    });
}

/// ABI `init(v, i)` — dynamics seed.
#[no_mangle]
pub extern "C" fn init(v: i32, _i: i32) {
    with_guest(|g| {
        let varr = read_v3(g, v);
        if let Some(m) = g.reg.active_mut() {
            m.init(&varr);
        }
    });
}

/// ABI `calc(v, i)` — write terminal currents (dispatch on SolutionMode).
#[no_mangle]
pub extern "C" fn calc(v: i32, i: i32) {
    with_guest(|g| {
        let varr = read_v3(g, v);
        let (dynamics, _, _) = in_dynamics(g);
        let mut iarr = [Cx::ZERO; NPHASES];
        if let Some(m) = g.reg.active_mut() {
            m.calc(&varr, &mut iarr, dynamics);
        }
        write_i3(g, i, &iarr);
    });
}

/// ABI `integrate()`.
#[no_mangle]
pub extern "C" fn integrate() {
    with_guest(|g| {
        let (_, h, new_step) = in_dynamics(g);
        if let Some(m) = g.reg.active_mut() {
            m.integrate(h, new_step);
        }
    });
}

/// ABI `save()` — the model has no checkpoint state (15-fn interface only).
#[no_mangle]
pub extern "C" fn save() {}

/// ABI `restore()`.
#[no_mangle]
pub extern "C" fn restore() {}

/// ABI `update_model()` — nothing to recompute.
#[no_mangle]
pub extern "C" fn update_model() {}

/// ABI `num_vars() -> i32`.
#[no_mangle]
pub extern "C" fn num_vars() -> i32 {
    crate::Model::NUM_VARS
}

/// ABI `get_all_vars(ptr)` — 4 f64 at 1-based offsets.
#[no_mangle]
pub extern "C" fn get_all_vars(ptr: i32) {
    if ptr == 0 {
        return;
    }
    with_guest(|g| {
        let mut vars = [0.0f64; 4];
        if let Some(m) = g.reg.active_ref() {
            m.get_all_vars(&mut vars);
        } else {
            return;
        }
        let buf = region_mut(&mut g.allocs, ptr, 4 * 8);
        for (k, val) in vars.iter().enumerate() {
            buf[k * 8..k * 8 + 8].copy_from_slice(&val.to_le_bytes());
        }
    });
}

/// ABI `get_variable(i) -> f64`.
#[no_mangle]
pub extern "C" fn get_variable(i: i32) -> f64 {
    with_guest(|g| g.reg.active_ref().map_or(-9999.99, |m| m.get_variable(i)))
}

/// ABI `set_variable(i, v)`.
#[no_mangle]
pub extern "C" fn set_variable(i: i32, value: f64) {
    with_guest(|g| {
        if let Some(m) = g.reg.active_mut() {
            m.set_variable(i, value);
        }
    });
}

/// ABI `get_var_name(i, ptr, maxlen)` — NUL-terminated, `StrLCopy` semantics.
#[no_mangle]
pub extern "C" fn get_var_name(i: i32, ptr: i32, maxlen: i32) {
    with_guest(|g| {
        let Some(name) = crate::Model::var_name(i) else {
            return; // out-of-range: buffer untouched
        };
        let bytes = name.as_bytes();
        let n = bytes.len().min(maxlen.max(0) as usize);
        let buf = region_mut(&mut g.allocs, ptr, n + 1);
        buf[..n].copy_from_slice(&bytes[..n]);
        buf[n] = 0;
    });
}
