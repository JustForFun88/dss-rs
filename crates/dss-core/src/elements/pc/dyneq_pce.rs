//! Shared `DynEqPCE` data + machinery. Port of Pascal `PCElements/DynEqPCE.pas`
//! (`TDynEqPCE`) — the abstract PC-element base that links a user-defined
//! [`DynamicExpObj`] (the `DynamicEq=` reference) and integrates it during a
//! dynamics solve in place of the element's built-in shaft/inverter model.
//!
//! Upstream comment: "DynEqPCE is an abstract class for grouping data used in
//! components that use DynamicExp. In the upstream OpenDSS, this is included in
//! the base PCElement". The concrete hosts — Generator, PVSystem, Storage — embed
//! [`DynEqPceData`] and drive `solve_eq` over `dynamic_eq_vals` each step.
//!
//! Scope (WP7.7 step 3b): the data record + the parse helpers (`parse_dyn_var`,
//! `set_dyn_output_names`/`get_dyn_output_names`), the `DynamicEq=` sizing side
//! effect, and the state-variable interface (`num_variables`/`variable_name`/
//! `get_all_variables`). The per-host integration (which calc values feed the
//! equation, which outputs drive the model) lives in each host's `dynamics.rs`.
//! `SaveWrite` (the `UserDynInit` dump) is Phase 8; `SetDynVars` (the JSON
//! init API) has no corpus path and is not ported.

use dss_parser::{Parser, ParserVars};

use crate::elements::general::dynamic_exp::{DYN_SLOT_LENGTH, DynamicExpObj};
use crate::elements::traits::ElemId;

/// One `UserDynInit` entry value. Pascal stores each assignment in a
/// `TJSONObject` as either a `TJSONNumber` (a constant with no RPN) or a
/// `TJSONString` (a calc-value operand, or a constant that required RPN
/// evaluation) — see `TDynEqPCE.ParseDynVar` (`DynEqPCE.pas:159-179`). The
/// distinction drives both the `SaveWrite` dump (`FloatToStr` vs
/// `CheckForBlanks`) and the AltDSS JSON `"DynInit"` tail (a bare number vs a
/// quoted string, `CAPI_Obj.pas:759`).
#[derive(Debug, Clone, PartialEq)]
pub enum DynInitValue {
    /// `TJSONNumber` — a plain constant (`Damp = 0` → `0.0`), the evaluated
    /// double. Serialized as a JSON number.
    Number(f64),
    /// `TJSONString` — a calc-value operand (`PShaft = P0` → `"P0"`) or an
    /// RPN constant (`Mass = (3.5 2 * ...)`), stored as the raw user token.
    /// Serialized as a JSON string (case preserved).
    Text(String),
}

/// `TDynEqPCE` shared data. The host PC element embeds this and exposes it via the
/// [`DynEqPce`] trait so the edit loop and the dynamics step loop reach it
/// polymorphically.
#[derive(Debug, Clone, Default)]
pub struct DynEqPceData {
    /// `DynamicEqObj` name (kept for the `DynamicEq=` dump / resolution bookkeeping).
    pub dynamic_eq: String,
    /// `DynamicEqObj` — the linked `DynamicExp` (snapshot clone, WP4.2/WP5.3 pattern).
    pub dynamic_eq_obj: Option<DynamicExpObj>,
    pub dynamic_eq_ref: Option<ElemId>,
    /// `DynamicEqVals` — per-variable `[value, derivative]` memory, sized by the
    /// `DynamicEq=` side effect to `DynamicExp.NVariables` rows.
    pub dynamic_eq_vals: Vec<[f64; DYN_SLOT_LENGTH]>,
    /// `DynOut` — the resolved output-variable indices (`SetDynOutputNames`); the
    /// host integration reads `DynOut[0]`/`DynOut[1]` as its primary states.
    pub dyn_out: Vec<usize>,
    /// `DynamicEqPair` — flat (var-index, operand-code) pairs for the
    /// calculated/initialization values (`ParseDynVar` / `Check_If_CalcValue`).
    pub dynamic_eq_pair: Vec<i32>,
    /// `UserDynInit` — the user dynamic-init assignments (var → typed value),
    /// deduped by variable and kept in insertion order (the last write of a
    /// variable moves to the end, mirroring Pascal `Delete` + `Add`). Consumed
    /// by the AltDSS JSON `"DynInit"` tail (`CAPI_Obj.pas:752-759`) and, once
    /// ported, `SaveWrite`.
    pub user_dyn_init: Vec<(String, DynInitValue)>,
}

impl DynEqPceData {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether a `DynamicEq` is linked (the `DynamicEqObj <> NIL` guard the host
    /// branches on for the dynamics integration).
    pub fn has_dynamic_eq(&self) -> bool {
        self.dynamic_eq_obj.is_some()
    }

    /// Pascal `TProp.DynamicEq` side effect (generator.pas l.805,
    /// PVsystem/Storage equivalents): `SetLength(DynamicEqVals,
    /// DynamicEqObj.NVariables)`. Called from the host `side_effects` after the
    /// object ref resolves. A nil ref leaves the memory empty (Pascal guards on
    /// `DynamicEqObj <> NIL`).
    pub fn on_dynamic_eq_set(&mut self) {
        if let Some(eq) = &self.dynamic_eq_obj {
            let n = eq.n_variables().max(0) as usize;
            self.dynamic_eq_vals = vec![[0.0; DYN_SLOT_LENGTH]; n];
        }
    }

    /// Pascal `TDynEqPCE.ParseDynVar` (was `CheckIfDynVar`): handle a `name=value`
    /// pair whose `name` is not a class property but *is* a state variable of the
    /// linked `DynamicExp`. Returns `false` when there is no dynamic equation or
    /// `name` is not one of its variables — the edit loop then reports the
    /// "Unknown parameter" error, exactly like Pascal's `DSSClass.Edit` fallback.
    ///
    /// `vars` is the executive's `ParserVars` so the value re-evaluates with the
    /// same `Var` substitutions / RPN a normal double property would (the value
    /// `value` is the parser token with its `(...)`/`[...]` delimiters already
    /// stripped, so it is re-wrapped in `(...)` before `make_double`, exactly as
    /// `obj/props/setters.rs` does for a `Double` property).
    pub fn parse_dyn_var(&mut self, variable: &str, value: &str, vars: &ParserVars) -> bool {
        let variable = variable.to_ascii_lowercase();
        let Some(eq) = &self.dynamic_eq_obj else {
            return false;
        };
        let var_idx = eq.get_var_idx(&variable);
        // Pascal: `(varIdx < 0) or (varIdx >= 50000)` — not a state variable
        // (negative) or a numeric-constant sentinel (50001).
        if !(0..50000).contains(&var_idx) {
            return false;
        }
        let var_idx = var_idx as usize;

        // Dedup the UserDynInit entry (Pascal `UserDynInit.Delete(variable)`).
        self.user_dyn_init.retain(|(k, _)| *k != variable);

        let op = DynamicExpObj::check_if_calc_value(value);
        if op >= 0 {
            // A value the host computes from its model (P/Q/Vmag/.../P0/Q0/edp):
            // record the (var index, operand code) pair for the step loop.
            // Pascal stores the raw operand token as a `TJSONString`.
            self.dynamic_eq_pair.push(var_idx as i32);
            self.dynamic_eq_pair.push(op);
            self.user_dyn_init
                .push((variable, DynInitValue::Text(value.to_string())));
        } else {
            // A constant: evaluate (with RPN) into the values array. A fresh
            // parser defaults `auto_increment = false`, so advance to the token
            // explicitly before `make_double` reads it. `make_double_ex` also
            // reports Pascal's `requiredRPN` (more than one RPN token): a plain
            // number is stored as a `TJSONNumber` (the evaluated double), a value
            // that required RPN is stored as the raw `TJSONString`
            // (`DynEqPCE.pas:173-178`).
            let mut parser = Parser::new();
            parser.set_cmd_string(&format!("({value})"));
            parser.next_param(vars);
            let (dbl, required_rpn) = parser.make_double_ex(vars).unwrap_or((0.0, false));
            if var_idx < self.dynamic_eq_vals.len() {
                self.dynamic_eq_vals[var_idx][0] = dbl;
            }
            let stored = if required_rpn {
                DynInitValue::Text(value.to_string())
            } else {
                DynInitValue::Number(dbl)
            };
            self.user_dyn_init.push((variable, stored));
        }
        true
    }

    /// Pascal `TDynEqPCE.SetDynOutputNames`: resolve `DynOut=[names]` to the output
    /// variable indices via `DynamicExp.Get_Out_Idx`. Errors (no linked equation,
    /// or a name that is not a defined output) are returned for the host to push
    /// onto its object error list (Pascal `DoSimpleMsg` 50007/50008).
    pub fn set_dyn_output_names(&mut self, names: &[String]) -> Vec<crate::diag::DssDiagnostic> {
        let mut errors = Vec::new();
        let Some(eq) = &self.dynamic_eq_obj else {
            // Pascal builds the list with a trailing comma per element.
            let list: String = names.iter().map(|n| format!("{n},")).collect();
            errors.push(crate::diag::DssDiagnostic::msg(
                format!(
                    "A DynamicExp object needs to be assigned to this element before \
                     this declaration: DynOut = [{list}]"
                ),
                Some(50007),
            ));
            return errors;
        };
        // Pascal `SetLength(DynOut, 2)` — always two output slots (speed + angle).
        // A 1-element `DynOut=[Speed]` leaves slot 1 = 0 (the integration reads
        // `DynOut[1]` as the angle var); >2 names overrun (Pascal range error).
        self.dyn_out = vec![0; 2];
        for (idx, name) in names.iter().enumerate() {
            let var_idx = eq.get_out_idx(name);
            if var_idx < 0 {
                errors.push(crate::diag::DssDiagnostic::msg(
                    format!(
                        "DynamicExp variable \"{}\" not found or not defined as an output.",
                        name.to_lowercase()
                    ),
                    Some(50008),
                ));
            } else {
                self.dyn_out[idx] = var_idx as usize;
            }
        }
        errors
    }

    /// Pascal `TDynEqPCE.GetDynOutputNames`: the `DynOut=` dump — reconstruct the
    /// output names from the resolved indices (`Get_VarName(DynOut[idx] * 2)`).
    pub fn get_dyn_output_names(&self) -> Vec<String> {
        let Some(eq) = &self.dynamic_eq_obj else {
            return Vec::new();
        };
        self.dyn_out
            .iter()
            .map(|&i| eq.get_var_name(i * DYN_SLOT_LENGTH))
            .collect()
    }

    /// Pascal `TDynEqPCE.NumVariables`: `DynamicEqObj.NVariables *
    /// Length(DynamicEqVals[0])` (= `NVariables * DYN_SLOT_LENGTH`), or 0 with no
    /// linked equation (the host then falls back to its classic variable count).
    pub fn num_variables(&self) -> usize {
        match &self.dynamic_eq_obj {
            Some(eq) => (eq.n_variables().max(0) as usize) * DYN_SLOT_LENGTH,
            None => 0,
        }
    }

    /// Pascal `TDynEqPCE.VariableName(i)` (1-based): the memory-slot name, or
    /// `None` when there is no equation / `i` is out of range (the host then falls
    /// back to its classic name table). Pascal guards `i > NVariables*Len` (the
    /// last valid index is exactly `NVariables*Len`).
    pub fn variable_name(&self, i: usize) -> Option<String> {
        let eq = self.dynamic_eq_obj.as_ref()?;
        if i < 1 || i > self.num_variables() {
            return None;
        }
        Some(eq.get_var_name(i - 1))
    }

    /// Pascal `DynamicEqObj.Get_DynamicEqVal(i, DynamicEqVals)` — read memory slot
    /// `i` (0-based; row = variable, column = value/derivative). Used by the host
    /// `Get_Variable`/`GetAllVariables` DynamicEq branch.
    pub fn get_dynamic_eq_val(&self, i: usize) -> f64 {
        DynamicExpObj::get_dynamic_eq_val(i, &self.dynamic_eq_vals)
    }

    /// Pascal `DynamicEqObj.SolveEq(DynamicEqVals)` — evaluate the linked
    /// equation's RHS and write the state-variable derivatives back into the
    /// memory space, so the host's trapezoidal integrator advances the state
    /// (both gating oracles integrate; see [`DynamicExpObj::solve_eq`] and
    /// DIVERGENCES.md §D14).
    /// (Disjoint-field borrow: the read-only equation and the mutable memory are
    /// separate fields, so no per-step clone of the expression is needed.)
    pub fn solve_eq(&mut self) {
        let Self {
            dynamic_eq_obj,
            dynamic_eq_vals,
            ..
        } = self;
        if let Some(eq) = dynamic_eq_obj {
            eq.solve_eq(dynamic_eq_vals);
        }
    }

    /// Pascal `DynamicEqObj.IsInitVal(code)` — codes 7/8/9 (p0/q0/edp) are
    /// initialization values (applied in `InitStateVars`, skipped in
    /// `IntegrateStates`).
    pub fn is_init_val(code: i32) -> bool {
        DynamicExpObj::is_init_val(code)
    }
}

/// The host hook: a Generator/PVSystem/Storage exposes its embedded
/// [`DynEqPceData`] so the edit loop's `parse_dyn_var` fallback and the dynamics
/// step loop can drive it without downcasting to the concrete type.
pub trait DynEqPce {
    fn dyneq(&self) -> &DynEqPceData;
    fn dyneq_mut(&mut self) -> &mut DynEqPceData;
}
