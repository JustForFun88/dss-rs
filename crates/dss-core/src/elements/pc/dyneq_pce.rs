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
//! The module holds the data record, the link of `DynamicEq=`
//! ([`DynEqPceData::link`], which carries the host's values and memory to the
//! new equation by variable name), the parse helpers (`parse_dyn_var`,
//! `set_dyn_output_names`/`get_dyn_output_names`) and the state-variable
//! interface (`num_variables`/`variable_name`/`get_all_variables`). The
//! per-host integration (which calc values feed the equation, which outputs
//! drive the model) lives in each host's `dynamics.rs`. `Save` and the JSON
//! export write the assignments through [`DynEqPceData::written_init`].
//! `SetDynVars` (the JSON init API) has no corpus path and is not ported.

use dss_parser::{Parser, ParserError, ParserVars};

use crate::elements::general::dynamic_exp::{DYN_SLOT_LENGTH, DynamicExpObj, VarRef};
use crate::elements::traits::Idx;

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

/// The output slots `DynOut` fills: speed and angle for a machine, and an
/// inverter reads the first as its current.
pub const DYN_OUT_SLOTS: usize = 2;

/// How a host integrates its linked `DynamicExp` each dynamics step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DynEqHost {
    /// A machine (Generator, WindGen): integrates the speed and angle outputs
    /// `DynOut[0..2]` and loads every calculated value.
    Machine,
    /// An inverter (PVSystem, Storage): integrates the current output
    /// `DynOut[0]` per phase and loads the calculated values that are not
    /// initializations, `imag` excepted (the current is the state itself).
    Inverter,
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
    /// The resolved `DynamicExp`'s stable arena handle (statically that class —
    /// `DynamicEq=` is an `object_ref_class("DynamicExp", …)` property).
    pub dynamic_eq_ref: Option<Idx<DynamicExpObj>>,
    /// `DynamicEqVals` — per-variable `[value, derivative]` memory,
    /// `DynamicExp.NVariables` rows, sized and carried by name at
    /// [`Self::link`].
    pub dynamic_eq_vals: Vec<[f64; DYN_SLOT_LENGTH]>,
    /// The variable each row of `dynamic_eq_vals` belongs to, `None` for a
    /// row past `VarNames`. Set at every link to an equation and kept with the
    /// memory through a link to none.
    pub memory_names: Vec<Option<String>>,
    /// The rows, value and derivative, of the variables the linked equation
    /// lacks or holds past its memory, by name: kept from an earlier
    /// equation or written past the memory, until a link to an equation that
    /// holds the variable moves the row into its memory.
    pub parked_rows: Vec<(String, [f64; DYN_SLOT_LENGTH])>,
    /// `DynOut` — the outputs the last accepted `DynOut=` list named, in slot
    /// order: a machine integrates the first as its speed and the second as
    /// its angle, an inverter the first as its current. Empty until a list
    /// resolves, and emptied by a re-link to an equation without those
    /// outputs ([`Self::link`]).
    pub dyn_out: Vec<usize>,
    /// `DynamicEqPair` — flat (variable row, operand code) pairs of the
    /// calculated values the host loads, built from `user_dyn_init` against
    /// the linked equation.
    pub dynamic_eq_pair: Vec<i32>,
    /// `UserDynInit` — the user's state-variable assignments (variable →
    /// typed value), deduplicated by variable and kept in assignment order (a
    /// re-assigned variable moves to the end). Every link keeps them all, and
    /// `Save` and the JSON `"DynInit"` tail write those of the linked
    /// equation through [`Self::written_init`].
    pub user_dyn_init: Vec<(String, DynInitValue)>,
    /// The constant that the last assignment of a variable in `user_dyn_init`
    /// gives it, evaluated when it was written, by variable name. Every entry
    /// into a dynamics-model mode writes them into the memory again
    /// ([`Self::apply_init_values`]).
    pub init_values: Vec<(String, f64)>,
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

    /// Link the equation `DynamicEq=` named (`eq_obj` is `None` when the name
    /// is empty or did not resolve). What the host holds follows its variable
    /// by name, never an index into the equation it replaces:
    /// - the outputs `DynOut` reads back stay when every one is an output of
    ///   the new equation, and `DynOut` is emptied otherwise
    ///   (`every_host_refuses_a_relink_to_an_equation_without_its_outputs`),
    /// - the memory gets `NVariables` rows, and each named row, value and
    ///   derivative, moves to the row of its variable
    ///   (`every_host_keeps_its_equation_memory_through_a_relink`). A name
    ///   `VarNames` repeats carries the row the name resolves to, its first
    ///   (`a_relink_carries_the_row_a_repeated_variable_name_resolves_to`),
    /// - a row whose variable the new equation lacks or holds past its memory
    ///   is parked by name until a later link holds the variable again
    ///   (`every_host_keeps_what_an_equation_lacks_for_a_later_link`),
    /// - the assignments are all kept, and the calculated values are paired
    ///   with the rows of their variables in the new equation
    ///   (`every_host_carries_its_calculated_values_by_name_through_a_relink`).
    ///   The host lists and writes only the variables of the new equation
    ///   (`a_relink_lists_and_writes_only_the_variables_of_the_new_equation`).
    ///
    /// A link to no equation empties `DynOut` and the pairs and keeps the
    /// memory and the assignments for the next link
    /// (`a_link_to_no_equation_keeps_the_values_for_the_next_link`).
    pub fn link(
        &mut self,
        name: String,
        eq_ref: Option<Idx<DynamicExpObj>>,
        eq_obj: Option<DynamicExpObj>,
    ) {
        let named = self.get_dyn_output_names();
        let outputs = eq_obj.as_ref().and_then(|eq| {
            named
                .iter()
                .map(|n| usize::try_from(eq.get_out_idx(n)).ok())
                .collect::<Option<Vec<_>>>()
        });
        if let Some(eq) = &eq_obj {
            let rows = eq.n_variables().max(0) as usize;
            let mut memory = vec![[0.0; DYN_SLOT_LENGTH]; rows];
            let mut parked = Vec::new();
            for (var, row) in self.rows_by_name() {
                match eq.get_var_idx(&var) {
                    VarRef::State(j) if j < rows => memory[j] = row,
                    _ => parked.push((var, row)),
                }
            }
            self.dynamic_eq_vals = memory;
            self.parked_rows = parked;
            self.memory_names = (0..rows)
                .map(|row| eq.get_var_name(row * DYN_SLOT_LENGTH))
                .collect();
        }
        self.dynamic_eq = name;
        self.dynamic_eq_ref = eq_ref;
        self.dynamic_eq_obj = eq_obj;
        self.dyn_out = outputs.unwrap_or_default();
        self.rebuild_pairs();
    }

    /// Every row the host holds, by variable name: the memory row each name
    /// resolves to (the first of a name `VarNames` repeats), then the parked
    /// rows.
    fn rows_by_name(&self) -> Vec<(String, [f64; DYN_SLOT_LENGTH])> {
        let memory = self
            .memory_names
            .iter()
            .zip(&self.dynamic_eq_vals)
            .filter_map(|(var, row)| Some((var.as_ref()?, row)));
        let parked = self.parked_rows.iter().map(|(var, row)| (var, row));
        let mut rows: Vec<(String, [f64; DYN_SLOT_LENGTH])> = Vec::new();
        for (var, row) in memory.chain(parked) {
            if !rows.iter().any(|(seen, _)| seen == var) {
                rows.push((var.clone(), *row));
            }
        }
        rows
    }

    /// Write `value` into the parked row of `var`, a variable the linked
    /// equation holds past its memory, so a later link that holds the
    /// variable starts it from the value.
    fn park_value(&mut self, var: &str, value: f64) {
        match self.parked_rows.iter_mut().find(|(v, _)| v == var) {
            Some((_, row)) => row[0] = value,
            None => {
                let mut row = [0.0; DYN_SLOT_LENGTH];
                row[0] = value;
                self.parked_rows.push((var.to_string(), row));
            }
        }
    }

    /// Pair each calculated value of `user_dyn_init`, in assignment order,
    /// with the row of its variable in the linked equation. Empty with no
    /// equation.
    fn rebuild_pairs(&mut self) {
        self.dynamic_eq_pair.clear();
        let Some(eq) = &self.dynamic_eq_obj else {
            return;
        };
        for (var, val) in &self.user_dyn_init {
            let DynInitValue::Text(text) = val else {
                continue;
            };
            let code = DynamicExpObj::check_if_calc_value(text);
            if code >= 0
                && let VarRef::State(row) = eq.get_var_idx(var)
            {
                self.dynamic_eq_pair.push(row as i32);
                self.dynamic_eq_pair.push(code);
            }
        }
    }

    /// The assignments `Save` and the JSON `"DynInit"` tail write: those of
    /// the variables of the linked equation, in assignment order, and none
    /// while no equation is linked, because a saved line takes only the
    /// variables of its equation
    /// (`a_relink_lists_and_writes_only_the_variables_of_the_new_equation`,
    /// `a_link_to_no_equation_keeps_the_values_for_the_next_link`).
    pub fn written_init(&self) -> Vec<&(String, DynInitValue)> {
        let Some(eq) = &self.dynamic_eq_obj else {
            return Vec::new();
        };
        self.user_dyn_init
            .iter()
            .filter(|(var, _)| matches!(eq.get_var_idx(var), VarRef::State(_)))
            .collect()
    }

    /// Handle a `name=value` pair whose `name` is not a class property but a
    /// state variable of the linked `DynamicExp`. Returns `Ok(false)` when no
    /// equation is linked or `name` is not one of its state variables, and the
    /// edit loop then reports the unknown parameter.
    /// A constant that does not read as a number is refused with the parser's
    /// error before anything is recorded, so the variable keeps its earlier
    /// assignment (`an_unreadable_state_variable_value_is_refused`).
    ///
    /// `vars` is the executive's `ParserVars` so the value re-evaluates with the
    /// same `Var` substitutions / RPN a normal double property would (the value
    /// `value` is the parser token with its `(...)`/`[...]` delimiters already
    /// stripped, so it is re-wrapped in `(...)` before `make_double`, exactly as
    /// `obj/props/setters.rs` does for a `Double` property).
    ///
    /// The pairs are rebuilt from the assignments, so the last assignment of
    /// a variable is the one the host applies
    /// (`a_constant_after_a_calculated_value_replaces_it`). A constant written
    /// to a variable past the memory is parked by name
    /// (`every_host_keeps_what_an_equation_lacks_for_a_later_link`).
    pub fn parse_dyn_var(
        &mut self,
        variable: &str,
        value: &str,
        vars: &ParserVars,
    ) -> Result<bool, ParserError> {
        let variable = variable.to_ascii_lowercase();
        let Some(eq) = &self.dynamic_eq_obj else {
            return Ok(false);
        };
        // Only a state variable is consumed, not an unknown name or a number.
        let VarRef::State(var_idx) = eq.get_var_idx(&variable) else {
            return Ok(false);
        };

        let stored = if DynamicExpObj::check_if_calc_value(value) >= 0 {
            // A value the host computes from its model (P/Q/Vmag/.../P0/Q0/edp),
            // kept as its operand token and paired with its row below.
            self.init_values.retain(|(k, _)| *k != variable);
            DynInitValue::Text(value.to_string())
        } else {
            // A constant, evaluated with RPN into the memory row of its
            // variable, or into its parked row past the memory. A fresh parser
            // does not advance on its own, so it moves to the token before
            // `make_double_ex` reads it. A plain number is kept as the evaluated
            // number, a value that needs more than one RPN token as its text.
            let mut parser = Parser::new();
            parser.set_cmd_string(&format!("({value})"));
            parser.next_param(vars);
            let (dbl, required_rpn) = parser.make_double_ex(vars)?;
            match self.dynamic_eq_vals.get_mut(var_idx) {
                Some(row) => row[0] = dbl,
                None => self.park_value(&variable, dbl),
            }
            self.init_values.retain(|(k, _)| *k != variable);
            self.init_values.push((variable.clone(), dbl));
            if required_rpn {
                DynInitValue::Text(value.to_string())
            } else {
                DynInitValue::Number(dbl)
            }
        };
        // A new assignment of a variable replaces its earlier one.
        self.user_dyn_init.retain(|(k, _)| *k != variable);
        self.user_dyn_init.push((variable, stored));
        self.rebuild_pairs();
        Ok(true)
    }

    /// Resolve `DynOut=[names]` to the outputs of the linked equation, in slot
    /// order. Errors are returned for the host to push onto its object error
    /// list: 50007 with no linked equation, 50009 for more names than the
    /// [`DYN_OUT_SLOTS`] output slots, 50008 for each name that is not a
    /// defined output. A list with an error is refused whole and `DynOut`
    /// keeps the outputs it had, so a slot only holds an output a list named
    /// (`every_host_keeps_its_dynout_through_a_list_with_an_unresolved_name`).
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
        // A name past the slots has no output to drive, so the list is refused
        // whole and the slots keep their outputs
        // (`every_host_refuses_a_dynout_longer_than_two_outputs`).
        if names.len() > DYN_OUT_SLOTS {
            errors.push(crate::diag::DssDiagnostic::msg(
                format!(
                    "DynOut takes at most {DYN_OUT_SLOTS} output variables, but {} were \
                     given: [{}]. DynOut is unchanged.",
                    names.len(),
                    names.join(", ")
                ),
                Some(50009),
            ));
            return errors;
        }
        let mut outputs = Vec::with_capacity(names.len());
        for name in names {
            match usize::try_from(eq.get_out_idx(name)) {
                Ok(var_idx) => outputs.push(var_idx),
                Err(_) => errors.push(crate::diag::DssDiagnostic::msg(
                    format!(
                        "DynamicExp variable \"{}\" not found or not defined as an output. \
                         DynOut is unchanged.",
                        name.to_lowercase()
                    ),
                    Some(50008),
                )),
            }
        }
        if errors.is_empty() {
            self.dyn_out = outputs;
        }
        errors
    }

    /// The first variable `list` names more than once, compared without case
    /// and lowercased, or `None` when every name is its own.
    pub fn repeated_output(list: &[String]) -> Option<String> {
        let mut seen: Vec<String> = Vec::with_capacity(list.len());
        for name in list {
            let name = name.to_ascii_lowercase();
            if seen.contains(&name) {
                return Some(name);
            }
            seen.push(name);
        }
        None
    }

    /// The refusal of a `DynOut=` list written to the host `element`
    /// (`Class.name`) that names one variable more than once. Asked before
    /// [`Self::set_dyn_output_names`], so it holds with or without a linked
    /// equation and ahead of 50009 and 50008
    /// (`every_host_refuses_a_dynout_that_names_one_variable_twice`).
    pub fn refuse_output_list(element: &str, list: &[String]) -> Option<String> {
        let name = Self::repeated_output(list)?;
        Some(format!(
            "{element}.DynOut: [{}] names the variable \"{name}\" more than once. Each output \
             needs its own variable.",
            list.join(", ")
        ))
    }

    /// The `DynOut=` readback: the names of the outputs the slots hold, one
    /// per name of the accepted list. A slot whose variable has no name is
    /// left out.
    pub fn get_dyn_output_names(&self) -> Vec<String> {
        let Some(eq) = &self.dynamic_eq_obj else {
            return Vec::new();
        };
        self.dyn_out
            .iter()
            .filter_map(|&i| eq.get_var_name(i * DYN_SLOT_LENGTH))
            .collect()
    }

    /// How many variables the host lists from its equation memory, a value
    /// and a derivative slot per row. A written `NVariables` lists all its
    /// rows. An omitted one lists only the rows its `VarNames` name, at most
    /// the 20 the memory holds, so an equation without names lists none
    /// (`an_omitted_nvariables_lists_only_the_named_variables`). 0 with no
    /// linked equation: the host then lists its classic variables.
    pub fn num_variables(&self) -> usize {
        let Some(eq) = &self.dynamic_eq_obj else {
            return 0;
        };
        let rows = eq.n_variables().max(0) as usize;
        let listed = if eq.n_variables_written() {
            rows
        } else {
            rows.min(eq.num_state_vars())
        };
        listed * DYN_SLOT_LENGTH
    }

    /// The name of memory slot `i` (1-based), or `None` with no equation or
    /// an `i` past [`Self::num_variables`], the last valid index (the host
    /// then falls back to its classic name table). When the count was
    /// written, a memory row past `VarNames` is named by the empty string.
    pub fn variable_name(&self, i: usize) -> Option<String> {
        let eq = self.dynamic_eq_obj.as_ref()?;
        if i < 1 || i > self.num_variables() {
            return None;
        }
        Some(eq.get_var_name(i - 1).unwrap_or_default())
    }

    /// Why the linked equation cannot be integrated in a dynamics solve, or
    /// `None` when it can (or none is linked). The memory must hold a row for
    /// every output `host` integrates, every calculated value it loads each
    /// step and every variable the equations refer to, and `DynOut` must name
    /// every output the host integrates: two for a machine, one for an
    /// inverter. An equation with `NVariables=0` holds no state at all, so it
    /// defines no dynamics. One that omits `NVariables` holds the default rows
    /// and is refused only when its equations reach past them
    /// (`an_omitted_nvariables_holds_twenty_rows`). The memory is checked
    /// first.
    pub fn integration_problem(&self, host: DynEqHost) -> Option<String> {
        let eq = self.dynamic_eq_obj.as_ref()?;
        let name = crate::obj::base::DssObject::data(eq).name();
        let rows = self.dynamic_eq_vals.len();
        if rows == 0 {
            return Some(format!(
                "DynamicExp.{name} declares no state variables (NVariables=0). Set \
                 NVariables to the number of its VarNames."
            ));
        }
        let outputs = match host {
            DynEqHost::Machine => 2,
            DynEqHost::Inverter => 1,
        };
        let loads = |code: i32| match host {
            DynEqHost::Machine => true,
            DynEqHost::Inverter => !Self::is_init_val(code) && code != 4,
        };
        let pairs = self
            .dynamic_eq_pair
            .chunks_exact(2)
            .filter(|p| loads(p[1]))
            .map(|p| p[0].max(0) as usize + 1);
        let used = self
            .dyn_out
            .iter()
            .take(outputs)
            .map(|&o| o + 1)
            .chain(pairs)
            .chain(std::iter::once(eq.rows_used()))
            .max()
            .unwrap_or(0);
        if used > rows {
            let holds = if eq.n_variables_written() {
                "declares"
            } else {
                "holds the default"
            };
            return Some(format!(
                "DynamicExp.{name} {holds} NVariables={rows} but its equations use {used} \
                 state variables. Set NVariables to the number of its VarNames."
            ));
        }
        match self.dyn_out.len() {
            0 => Some(format!(
                "DynOut is not set. Name the output variables of DynamicExp.{name} with \
                 DynOut=[...]."
            )),
            // Only a machine integrates two outputs, so one output leaves it
            // without its angle (`a_machine_refuses_a_dynout_with_one_output`).
            n if n < outputs => Some(format!(
                "DynOut names one output, but a machine integrates two, its speed and its \
                 angle. Name both output variables of DynamicExp.{name} with DynOut=[...]."
            )),
            _ => None,
        }
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

    /// Write each constant initial value into its variable's memory row, as the
    /// assignment did, so every run starts from the values the equation's
    /// inputs define and not from those an earlier run left
    /// (`a_dynamicexp_machine_starts_every_run_from_its_initial_values`). A
    /// variable without a memory row is skipped: the dynamics solve refuses
    /// that equation before its first step.
    pub fn apply_init_values(&mut self) {
        let Some(eq) = &self.dynamic_eq_obj else {
            return;
        };
        for (variable, value) in &self.init_values {
            if let VarRef::State(row) = eq.get_var_idx(variable)
                && let Some(slot) = self.dynamic_eq_vals.get_mut(row)
            {
                slot[0] = *value;
            }
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
