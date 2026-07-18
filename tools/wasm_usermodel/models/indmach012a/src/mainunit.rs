//! DLL interface layer — loop-for-loop port of r3723
//! `Version8/Source/IndMach012a/MainUnit.pas`: the model registry
//! (`ModelList`/`ActiveModel`), the sym-component marshalling around
//! `Init`/`Calc`, and the monitoring-variable surface. The wasm export shims
//! (`wasm_exports.rs`) call straight into these functions; host-target tests
//! (`tests/twin_parity.rs`) drive them directly to pin the port against the
//! native FPC twin.

use crate::cmath::{Complex, CZERO};
use crate::model::{IndMach012Model, NUM_VARIABLES, VAR_NAMES};
use crate::parser::Parser;
use crate::records::{DynamicsRec, GeneratorVars, DYNAMICMODE};
use crate::symcomp::{phase2symcomp, symcomp2phase};

/// Pascal `MainUnit` unit globals: `ModelList: TList` + `ActiveModel`
/// (`IndMach012Model.pas:97`, `MainUnit.pas:66-67`). `active` indexes into
/// `models`; a `None` entry is a deleted slot (TList keeps the hole).
pub struct MainState {
    pub models: Vec<Option<IndMach012Model>>,
    pub active: Option<usize>,
}

impl MainState {
    pub const fn new() -> Self {
        MainState {
            models: Vec::new(),
            active: None,
        }
    }

    fn active_model(&self) -> Option<&IndMach012Model> {
        self.active.and_then(|i| self.models[i].as_ref())
    }

    fn active_model_mut(&mut self) -> Option<&mut IndMach012Model> {
        self.active.and_then(|i| self.models[i].as_mut())
    }
}

impl Default for MainState {
    fn default() -> Self {
        Self::new()
    }
}

/// Bounds discipline for `Delete`/`Select`: Pascal only guards
/// `ID <= ModelList.Count`; `ID < 1` reaches `TList.Items[ID-1]`, which
/// raises `EListError` through the Stdcall boundary (crash class) — the port
/// panics (deterministic trap).
fn check_lower_bound(id: i32, what: &str) {
    assert!(
        id >= 1,
        "indmach012a: {what} with id {id} < 1 (upstream: EListError)"
    );
}

/// Pascal `MainUnit.New` (`MainUnit.pas:77-81`): create, append, activate;
/// returns the 1-based instance id.
pub fn new_instance(
    state: &mut MainState,
    genvars: &mut GeneratorVars,
    gen_data_ptr: i32,
    dyna_data_ptr: i32,
) -> i32 {
    let model = IndMach012Model::create(genvars, gen_data_ptr, dyna_data_ptr);
    state.models.push(Some(model));
    state.active = Some(state.models.len() - 1);
    state.models.len() as i32
}

/// Pascal `MainUnit.Delete` (`MainUnit.pas:83-91`). Faithful quirk: any
/// in-range delete leaves `ActiveModel = Nil`, even when the deleted slot was
/// not the active one.
pub fn delete(state: &mut MainState, id: i32) {
    if (id as i64) <= state.models.len() as i64 {
        check_lower_bound(id, "Delete");
        state.active = None;
        state.models[id as usize - 1] = None;
    }
}

/// Pascal `MainUnit.Select` (`MainUnit.pas:104-111`): activates the slot
/// (possibly `Nil`), returns the id when a live model was selected, else 0.
pub fn select(state: &mut MainState, id: i32) -> i32 {
    let mut result = 0;
    if (id as i64) <= state.models.len() as i64 {
        check_lower_bound(id, "Select");
        let idx = id as usize - 1;
        if state.models[idx].is_some() {
            state.active = Some(idx);
            result = id;
        } else {
            state.active = None;
        }
    }
    result
}

/// Pascal `MainUnit.Init` (`MainUnit.pas:114-127`): phase → sym components
/// for both V and I, then the model's dynamics init. Writes nothing back to
/// the V/I buffers.
pub fn init(
    state: &mut MainState,
    v: &[Complex; 3],
    i: &[Complex; 3],
    genvars: &mut GeneratorVars,
) {
    let Some(idx) = state.active else { return };
    let Some(model) = state.models[idx].as_mut() else {
        return;
    };
    let v012 = phase2symcomp(v);
    let i012 = phase2symcomp(i);
    model.init(&v012, &i012, genvars);
}

/// Pascal `MainUnit.Calc` (`MainUnit.pas:129-169`): phase → sym, dispatch on
/// `DynaData^.SolutionMode` (DYNAMICMODE → `CalcDynamic`, every other mode →
/// `CalcPflow`), sym → phase into the I buffer. Returns whether I was
/// written (false ⇔ no active model, matching the Pascal nil guard).
pub fn calc(
    state: &mut MainState,
    v: &[Complex; 3],
    i_out: &mut [Complex; 3],
    genvars: &mut GeneratorVars,
    dyna: &DynamicsRec,
) -> bool {
    let Some(idx) = state.active else {
        return false;
    };
    let Some(model) = state.models[idx].as_mut() else {
        return false;
    };
    let v012 = phase2symcomp(v);
    let mut i012 = [CZERO; 3];
    if dyna.solution_mode == DYNAMICMODE {
        model.calc_dynamic(&v012, &mut i012, genvars);
    } else {
        // all other modes are power-flow modes
        model.calc_pflow(&v012, &mut i012, genvars);
    }
    *i_out = symcomp2phase(&i012); // convert back to I abc
    true
}

/// Pascal `MainUnit.Integrate` (`MainUnit.pas:172-177`).
pub fn integrate(state: &mut MainState, genvars: &GeneratorVars, dyna: &DynamicsRec) {
    if let Some(model) = state.active_model_mut() {
        model.integrate(genvars, dyna);
    }
}

/// Pascal `MainUnit.Save` (`MainUnit.pas:179-190`) — the guarded body is
/// empty upstream (the example never implemented save/restore).
pub fn save(_state: &mut MainState) {}

/// Pascal `MainUnit.Restore` (`MainUnit.pas:192-202`) — empty body upstream.
pub fn restore(_state: &mut MainState) {}

/// Pascal `MainUnit.Edit` (`MainUnit.pas:232-242`): load the parser with the
/// received ANSI string, then interpret. `msg` is the MsgCallBack sink.
pub fn edit(
    state: &mut MainState,
    s: &[u8],
    genvars: &mut GeneratorVars,
    msg: &mut dyn FnMut(&[u8]),
) {
    let Some(idx) = state.active else { return };
    let Some(model) = state.models[idx].as_mut() else {
        return;
    };
    let mut parser = Parser::new();
    parser.set_cmd_string(s);
    model.edit(&mut parser, genvars, msg);
}

/// Pascal `MainUnit.UpdateModel` (`MainUnit.pas:244-248`).
pub fn update_model(state: &mut MainState, genvars: &GeneratorVars) {
    if let Some(model) = state.active_model_mut() {
        model.recalc_element_data(genvars);
    }
}

/// Pascal `MainUnit.NumVars` (`MainUnit.pas:251-255`) — the constant 14,
/// active model or not.
pub fn num_vars() -> i32 {
    NUM_VARIABLES
}

/// Pascal `MainUnit.GetAllVars` (`MainUnit.pas:257-280`). Upstream wraps the
/// loop in `Try/Except`: with `ActiveModel = Nil` the first `GetVariable`
/// access-violates before any write and the handler swallows it — so the
/// contract is "no active model ⇒ buffer untouched", reproduced here.
pub fn get_all_vars(state: &MainState, vars: &mut [f64; 14]) {
    if let Some(model) = state.active_model() {
        for (i, slot) in vars.iter_mut().enumerate() {
            *slot = model.get_variable((i + 1) as i32);
        }
    }
}

/// Pascal `MainUnit.GetVariable` (`MainUnit.pas:282-286`) — NO nil guard
/// upstream (access violation on a missing active model); the port panics.
pub fn get_variable(state: &MainState, i: i32) -> f64 {
    state
        .active_model()
        .expect("indmach012a: GetVariable with no active model (upstream: access violation)")
        .get_variable(i)
}

/// Pascal `MainUnit.SetVariable` (`MainUnit.pas:288-291`) — same no-guard
/// contract as `GetVariable`.
pub fn set_variable(state: &mut MainState, i: i32, value: f64, genvars: &mut GeneratorVars) {
    state
        .active_model_mut()
        .expect("indmach012a: SetVariable with no active model (upstream: access violation)")
        .set_variable(i, value, genvars);
}

/// Pascal `MainUnit.GetVarName` (`MainUnit.pas:294-317`): `StrLCopy`
/// semantics — copy at most `maxlen` bytes then a terminating NUL (the
/// destination must hold `maxlen+1`); an out-of-range VarNum leaves the
/// buffer untouched (the CASE has an empty ELSE). Returns the bytes to write
/// (name truncated to maxlen, plus NUL), or `None` for out-of-range.
pub fn get_var_name(var_num: i32, maxlen: u32) -> Option<Vec<u8>> {
    if !(1..=NUM_VARIABLES).contains(&var_num) {
        return None;
    }
    let name = VAR_NAMES[var_num as usize - 1].as_bytes();
    let n = name.len().min(maxlen as usize);
    let mut out = name[..n].to_vec();
    out.push(0);
    Some(out)
}
