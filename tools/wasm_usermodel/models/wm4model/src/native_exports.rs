//! The **native twin** boundary — the r4133-oracle side of the WM.4 gate
//! (plan §2.5 channel 1 / §2.6 plan B). This module compiles ONLY for the host
//! target (`#[cfg(not(target_arch = "wasm32"))]`) into a native cdylib the
//! official EPRI OpenDSS r4133 engine loads as a Storage `DynaDLL=` / PVSystem
//! `UserModel=` DLL. It exports the exact PascalCase Stdcall names and the exact
//! `var`-parameter (by-reference) signatures the loader binds
//! (`StoreUserModel.pas:214-228` / `PVSystemUserModel.pas:160-174`,
//! `GenUserModel`-shaped): `New`, `Delete`, `Select`, `Init`, `Calc`,
//! `Integrate`, `Save`, `Restore`, `Edit`, `UpdateModel`, `NumVars`,
//! `GetAllVars`, `GetVariable`, `SetVariable`, `GetVarName`.
//!
//! It shares the SAME [`crate::Model`] math as the wasm guest, so the r4133
//! engine (native twin) and the dss-rs engine (wasm fixture) run bit-identical
//! model code — the WM.4 gate is a real cross-engine comparison, never "Rust
//! agrees with itself". This shim is tooling (built on demand for manual golden
//! generation), lives outside the product's `forbid(unsafe_code)` scope, and
//! never joins the workspace (plan §2.6).
//!
//! On Win64 there is a single calling convention, so `extern "system"` matches
//! Delphi's `stdcall`; `var` parameters are pointers. All record/array layouts
//! are the frozen ABI images (`records.rs`), identical to the wasm side.

#![cfg(not(target_arch = "wasm32"))]
#![allow(clippy::missing_safety_doc)]

use crate::records::{decode_dynamics_rec, read_complex, write_complex, DYNAMICS_REC_SIZE};
use crate::{Cx, Registry, DYNAMICMODE, NPHASES};
use std::sync::Mutex;

/// Process-global twin state (the native DLL is loaded once per r4133 process).
struct NativeState {
    reg: Registry,
    /// Retained `@DynaData` pointer captured at `New` (the engine's single live
    /// `Solution.Dynavars`; updated in place by the engine each step).
    dynarec_ptr: usize,
}

static STATE: Mutex<NativeState> = Mutex::new(NativeState {
    reg: Registry::new(),
    dynarec_ptr: 0,
});

fn with_state<R>(f: impl FnOnce(&mut NativeState) -> R) -> R {
    let mut s = STATE.lock().unwrap_or_else(|p| p.into_inner());
    f(&mut s)
}

/// Read the 3-phase V array from a `pComplexArray` (element k 1-based at
/// `(k-1)*16`).
///
/// # Safety
/// `v` must point to at least `NPHASES` `Complex` (48 bytes) the caller owns.
unsafe fn read_v3(v: *const u8) -> [Cx; NPHASES] {
    // SAFETY: caller contract — `v` is the engine's Vterminal buffer, ≥ Yorder
    // complex (Yorder ≥ NPHASES for the gate decks).
    let buf = unsafe { std::slice::from_raw_parts(v, NPHASES * 16) };
    [
        read_complex(buf, 1),
        read_complex(buf, 2),
        read_complex(buf, 3),
    ]
}

/// # Safety
/// `i` must point to at least `NPHASES` writable `Complex`.
unsafe fn write_i3(i: *mut u8, arr: &[Cx; NPHASES]) {
    // SAFETY: caller contract — `i` is the engine's Iterminal / DESSCurr buffer.
    let buf = unsafe { std::slice::from_raw_parts_mut(i, NPHASES * 16) };
    write_complex(buf, 1, arr[0]);
    write_complex(buf, 2, arr[1]);
    write_complex(buf, 3, arr[2]);
}

/// `(dynamics?, h, new_step?)` from the retained DynaData record.
fn dyn_state(s: &NativeState) -> (bool, f64, bool) {
    if s.dynarec_ptr == 0 {
        return (false, 0.0, true);
    }
    // SAFETY: `dynarec_ptr` was captured from the engine at `New`; the engine
    // keeps `Solution.Dynavars` alive for the run.
    let buf = unsafe { std::slice::from_raw_parts(s.dynarec_ptr as *const u8, DYNAMICS_REC_SIZE) };
    let d = decode_dynamics_rec(buf);
    (d.solution_mode == DYNAMICMODE, d.h, d.iteration_flag == 0)
}

/// Pascal `New(var DynaData; var CallBacks): Integer` — CallBacks unused.
///
/// # Safety
/// `dynadata` points to the engine's `TDynamicsRec`.
#[export_name = "New"]
pub unsafe extern "system" fn new_(dynadata: *mut u8, _callbacks: *mut u8) -> i32 {
    with_state(|s| {
        s.dynarec_ptr = dynadata as usize;
        s.reg.new_instance()
    })
}

/// Pascal `Delete(var x: Integer)`.
///
/// # Safety
/// `x` points to a valid `Integer`.
#[export_name = "Delete"]
pub unsafe extern "system" fn delete_(x: *mut i32) {
    // SAFETY: caller passes @Id.
    let id = unsafe { *x };
    with_state(|s| s.reg.delete(id));
}

/// Pascal `Select(var x: Integer): Integer`.
///
/// # Safety
/// `x` points to a valid `Integer`.
#[export_name = "Select"]
pub unsafe extern "system" fn select_(x: *mut i32) -> i32 {
    let id = unsafe { *x };
    with_state(|s| s.reg.select(id))
}

/// Pascal `Edit(s: pAnsiChar; Maxlen: Cardinal)`.
///
/// # Safety
/// `s` points to `maxlen` ANSI bytes (NUL-terminated within).
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

/// Pascal `Init(V, I: pComplexArray)`.
///
/// # Safety
/// `v`/`i` point to ≥ `NPHASES` `Complex`.
#[export_name = "Init"]
pub unsafe extern "system" fn init_(v: *const u8, _i: *mut u8) {
    let varr = unsafe { read_v3(v) };
    with_state(|s| {
        if let Some(m) = s.reg.active_mut() {
            m.init(&varr);
        }
    });
}

/// Pascal `Calc(V, I: pComplexArray)`.
///
/// # Safety
/// `v`/`i` point to ≥ `NPHASES` `Complex`.
#[export_name = "Calc"]
pub unsafe extern "system" fn calc_(v: *const u8, i: *mut u8) {
    let varr = unsafe { read_v3(v) };
    let mut iarr = [Cx::ZERO; NPHASES];
    with_state(|s| {
        let (dynamics, _, _) = dyn_state(s);
        if let Some(m) = s.reg.active_mut() {
            m.calc(&varr, &mut iarr, dynamics);
        }
    });
    unsafe { write_i3(i, &iarr) };
}

/// Pascal `Integrate`.
#[export_name = "Integrate"]
pub extern "system" fn integrate_() {
    with_state(|s| {
        let (_, h, new_step) = dyn_state(s);
        if let Some(m) = s.reg.active_mut() {
            m.integrate(h, new_step);
        }
    });
}

/// Pascal `Save` — no checkpoint state.
#[export_name = "Save"]
pub extern "system" fn save_() {}

/// Pascal `Restore`.
#[export_name = "Restore"]
pub extern "system" fn restore_() {}

/// Pascal `UpdateModel`.
#[export_name = "UpdateModel"]
pub extern "system" fn update_model_() {}

/// Pascal `NumVars: Integer`.
#[export_name = "NumVars"]
pub extern "system" fn num_vars_() -> i32 {
    crate::Model::NUM_VARS
}

/// Pascal `GetAllVars(Vars: pDoubleArray)`.
///
/// # Safety
/// `vars` points to ≥ `NumVars` writable `Double`.
#[export_name = "GetAllVars"]
pub unsafe extern "system" fn get_all_vars_(vars: *mut f64) {
    if vars.is_null() {
        return;
    }
    with_state(|s| {
        let mut out = [0.0f64; 4];
        if let Some(m) = s.reg.active_ref() {
            m.get_all_vars(&mut out);
        } else {
            return;
        }
        // SAFETY: caller passes a pDoubleArray of ≥ NumVars.
        let dst = unsafe { std::slice::from_raw_parts_mut(vars, 4) };
        dst.copy_from_slice(&out);
    });
}

/// Pascal `GetVariable(var I: Integer): Double`.
///
/// # Safety
/// `i` points to a valid `Integer`.
#[export_name = "GetVariable"]
pub unsafe extern "system" fn get_variable_(i: *mut i32) -> f64 {
    let idx = unsafe { *i };
    with_state(|s| s.reg.active_ref().map_or(-9999.99, |m| m.get_variable(idx)))
}

/// Pascal `SetVariable(var i: Integer; var value: Double)`.
///
/// # Safety
/// `i`/`value` point to valid `Integer`/`Double`.
#[export_name = "SetVariable"]
pub unsafe extern "system" fn set_variable_(i: *mut i32, value: *mut f64) {
    let idx = unsafe { *i };
    let val = unsafe { *value };
    with_state(|s| {
        if let Some(m) = s.reg.active_mut() {
            m.set_variable(idx, val);
        }
    });
}

/// Pascal `GetVarName(var VarNum: Integer; VarName: pAnsiChar; maxlen: Cardinal)`.
///
/// # Safety
/// `varnum` points to a valid `Integer`; `varname` to ≥ `maxlen`+1 bytes.
#[export_name = "GetVarName"]
pub unsafe extern "system" fn get_var_name_(varnum: *mut i32, varname: *mut u8, maxlen: u32) {
    let idx = unsafe { *varnum };
    let Some(name) = crate::Model::var_name(idx) else {
        return;
    };
    let bytes = name.as_bytes();
    let n = bytes.len().min(maxlen as usize);
    // SAFETY: caller passes a buffer of ≥ maxlen+1 bytes (StrLCopy convention).
    let dst = unsafe { std::slice::from_raw_parts_mut(varname, n + 1) };
    dst[..n].copy_from_slice(&bytes[..n]);
    dst[n] = 0;
}
