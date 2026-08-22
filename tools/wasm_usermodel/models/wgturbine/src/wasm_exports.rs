//! The wasm boundary — the 15 exports of the frozen ABI
//! (`docs/wasm/USERMODEL_ABI.md` §1) plus `dss_alloc`, over the record shuttle
//! of ABI §2. `new(windgenvars, dynarec) -> id` (the two-pointer form, Pascal
//! `TWindGenUserModel.FNew`, `WindGenUserModel.pas:34`).
//!
//! Pointers arriving from the host are addresses inside guest linear memory the
//! host obtained from `dss_alloc`, so every access resolves through the
//! allocation registry — no raw-pointer dereference. The model uses NO host
//! imports (`dss_env` empty).
//!
//! Failure policy (ABI §6): a pointer outside a `dss_alloc` allocation or an OOB
//! range panics; with `panic = "abort"` that is a wasm trap the host surfaces as
//! a hard, attributed engine error.
//!
//! **Phase-count safety.** `V`/`I` are `Yorder`-long and the record's
//! `NumPhases ≤ NumConductors ≤ Yorder` for every WindGen, so reading
//! `NumPhases` complexes is in bounds by construction — and the allocation
//! bounds-check below is the backstop if that ever stops holding.

#![cfg(target_arch = "wasm32")]

use crate::records::{
    decode_dynamics_rec, decode_windgen_vars, put_f64, read_complex, write_complex, WindGenIn,
    DYNAMICS_REC_SIZE, OFF_CP, OFF_DSPEED, OFF_LAMDA, OFF_PG, OFF_PM, OFF_PR, OFF_PS, OFF_PSHAFT,
    OFF_S, OFF_SPEED, WINDGEN_VARS_SIZE,
};
use crate::{Cx, Registry, DYNAMICMODE, MAX_PHASES, VAR_OUT_OF_RANGE};
use std::collections::BTreeMap;
use std::sync::Mutex;

/// Guest-global state: the `dss_alloc` allocation registry, the model registry,
/// and the retained record buffer pointers (Pascal captures `@WindGenVars` and
/// `@DynaData` at `New`; the host refreshes both images in place before each
/// call).
struct Guest {
    allocs: BTreeMap<u32, Box<[u8]>>,
    reg: Registry,
    windgen_ptr: i32,
    dynarec_ptr: i32,
}

static GUEST: Mutex<Guest> = Mutex::new(Guest {
    allocs: BTreeMap::new(),
    reg: Registry::new(),
    windgen_ptr: 0,
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
        .unwrap_or_else(|| panic!("wgturbine: pointer {ptr:#x} outside guest allocations"));
    let off = (p - *base) as usize;
    assert!(
        off <= buf.len() && len <= buf.len() - off,
        "wgturbine: out-of-bounds buffer access at {ptr:#x} len {len}"
    );
    &mut buf[off..off + len]
}

fn region(allocs: &BTreeMap<u32, Box<[u8]>>, ptr: i32, len: usize) -> &[u8] {
    let p = ptr as u32;
    let (base, buf) = allocs
        .range(..=p)
        .next_back()
        .unwrap_or_else(|| panic!("wgturbine: pointer {ptr:#x} outside guest allocations"));
    let off = (p - *base) as usize;
    assert!(
        off <= buf.len() && len <= buf.len() - off,
        "wgturbine: out-of-bounds buffer access at {ptr:#x} len {len}"
    );
    &buf[off..off + len]
}

/// Decode the retained `TWindGenVars` image (the fields the model reads).
fn windgen_in(g: &Guest) -> WindGenIn {
    decode_windgen_vars(region(&g.allocs, g.windgen_ptr, WINDGEN_VARS_SIZE))
}

/// How many phases to touch, from the record, clamped like `Model::nph`.
fn nph(rec: &WindGenIn) -> usize {
    (rec.num_phases.max(1) as usize).min(MAX_PHASES)
}

fn read_v(g: &Guest, ptr: i32, n: usize) -> [Cx; MAX_PHASES] {
    let buf = region(&g.allocs, ptr, n * 16);
    let mut out = [Cx::ZERO; MAX_PHASES];
    for k in 0..n {
        out[k] = read_complex(buf, k + 1);
    }
    out
}

fn write_i(g: &mut Guest, ptr: i32, arr: &[Cx; MAX_PHASES], n: usize) {
    let buf = region_mut(&mut g.allocs, ptr, n * 16);
    for k in 0..n {
        write_complex(buf, k + 1, arr[k]);
    }
}

/// `(in DYNAMICMODE, h, new step)` from the retained `TDynamicsRec`.
fn dyn_state(g: &Guest) -> (bool, f64, bool) {
    let dyna = decode_dynamics_rec(region(&g.allocs, g.dynarec_ptr, DYNAMICS_REC_SIZE));
    (
        dyna.solution_mode == DYNAMICMODE,
        dyna.h,
        dyna.iteration_flag == 0,
    )
}

/// Write the model's turbine outputs back into the retained `TWindGenVars`
/// image (ABI §2 copy-out: the host reads the whole record back after the call).
/// Called from `calc`, which is where every output acquires a value.
fn store_outputs(g: &mut Guest) {
    let Some(out) = g.reg.active_ref().map(|m| m.out) else {
        return;
    };
    let ptr = g.windgen_ptr;
    let buf = region_mut(&mut g.allocs, ptr, WINDGEN_VARS_SIZE);
    put_f64(buf, OFF_PSHAFT, out.pshaft);
    put_f64(buf, OFF_SPEED, out.speed);
    put_f64(buf, OFF_DSPEED, out.dspeed);
    put_f64(buf, OFF_CP, out.cp);
    put_f64(buf, OFF_LAMDA, out.lamda);
    put_f64(buf, OFF_PM, out.pm);
    put_f64(buf, OFF_PS, out.ps);
    put_f64(buf, OFF_PR, out.pr);
    put_f64(buf, OFF_PG, out.pg);
    put_f64(buf, OFF_S, out.s);
}

/// Write back only the two head speed fields (`Speed`/`dSpeed`) that `init`
/// seeds and `integrate` advances — Pascal user
/// models set the machine speed from their own initial state at `Init`
/// (IndMach012a's `Init` → `Set_Slip` → `GenData^.Speed`), they do not
/// invent turbine *outputs* before the first `Calc`. Leaving the rest of the
/// record alone here is deliberate: a blanket `store_outputs` would zero the
/// engine's Pg/Ps/Pr/Cp on every `InitStateVars`.
fn store_speed(g: &mut Guest) {
    let Some((speed, dspeed)) = g.reg.active_ref().map(|m| (m.speed, m.dspeed)) else {
        return;
    };
    let ptr = g.windgen_ptr;
    let buf = region_mut(&mut g.allocs, ptr, WINDGEN_VARS_SIZE);
    put_f64(buf, OFF_SPEED, speed);
    put_f64(buf, OFF_DSPEED, dspeed);
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

/// ABI `new(windgenvars, dynarec) -> id`. Retains both buffer pointers for the
/// instance lifetime (Pascal `@WindGenVars` / `@DynaData` capture).
#[no_mangle]
pub extern "C" fn new(windgenvars: i32, dynarec: i32) -> i32 {
    with_guest(|g| {
        g.windgen_ptr = windgenvars;
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

/// ABI `edit(ptr, len)` — the `UserData=` string (Pascal
/// `TWindGenUserModel.Set_Edit`, `WindGenUserModel.pas:150-154`).
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

/// ABI `init(v, i)` — dynamics seed (`WindGen.pas:2568`).
#[no_mangle]
pub extern "C" fn init(v: i32, _i: i32) {
    with_guest(|g| {
        let rec = windgen_in(g);
        let n = nph(&rec);
        let varr = read_v(g, v, n);
        if let Some(m) = g.reg.active_mut() {
            m.init(&varr, &rec);
        }
        store_speed(g);
    });
}

/// ABI `calc(v, i)` — write terminal currents and the turbine outputs
/// (`WindGen.pas:1887` power flow / `:1993` dynamics).
#[no_mangle]
pub extern "C" fn calc(v: i32, i: i32) {
    with_guest(|g| {
        let rec = windgen_in(g);
        let n = nph(&rec);
        let varr = read_v(g, v, n);
        let (dynamics, _, _) = dyn_state(g);
        let mut iarr = [Cx::ZERO; MAX_PHASES];
        if let Some(m) = g.reg.active_mut() {
            m.calc(&varr, &mut iarr, &rec, dynamics);
        } else {
            return;
        }
        write_i(g, i, &iarr, n);
        store_outputs(g);
    });
}

/// ABI `integrate()` (`WindGen.pas:2663`).
#[no_mangle]
pub extern "C" fn integrate() {
    with_guest(|g| {
        let (_, h, new_step) = dyn_state(g);
        if let Some(m) = g.reg.active_mut() {
            m.integrate(h, new_step);
        }
        store_speed(g);
    });
}

/// ABI `save()` — the model has no checkpoint state (15-fn interface only).
#[no_mangle]
pub extern "C" fn save() {}

/// ABI `restore()`.
#[no_mangle]
pub extern "C" fn restore() {}

/// ABI `update_model()` — nothing to recompute (Pascal `FUpdateModel`,
/// `WindGen.pas:1418`).
#[no_mangle]
pub extern "C" fn update_model() {}

/// ABI `num_vars() -> i32`.
#[no_mangle]
pub extern "C" fn num_vars() -> i32 {
    crate::Model::NUM_VARS
}

/// ABI `get_all_vars(ptr)` — [`crate::Model::NUM_VARS`] f64 at 1-based offsets.
#[no_mangle]
pub extern "C" fn get_all_vars(ptr: i32) {
    if ptr == 0 {
        return;
    }
    with_guest(|g| {
        let n = crate::Model::NUM_VARS as usize;
        let mut vars = vec![0.0f64; n];
        if let Some(m) = g.reg.active_ref() {
            m.get_all_vars(&mut vars);
        } else {
            return;
        }
        let buf = region_mut(&mut g.allocs, ptr, n * 8);
        for (k, val) in vars.iter().enumerate() {
            buf[k * 8..k * 8 + 8].copy_from_slice(&val.to_le_bytes());
        }
    });
}

/// ABI `get_variable(i) -> f64`.
#[no_mangle]
pub extern "C" fn get_variable(i: i32) -> f64 {
    with_guest(|g| {
        g.reg
            .active_ref()
            .map_or(VAR_OUT_OF_RANGE, |m| m.get_variable(i))
    })
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
