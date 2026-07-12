//! Unit tests for the `DynamicExp` interpreter/evaluator (`InterpretDiffEq` →
//! `cmds`, `SolveEq`). The property dump is pinned against the oracle by the
//! `props_roundtrip` golden (`tests/golden/props/dynamicexp.json`); these tests
//! exercise the compilation the dump cannot reach (the oracle does not expose
//! `cmds` outside a dynamics solve). Values are hand-derived from the Pascal
//! algorithm. Since the D14 upgrade (upstream `2a8bdb78`) `SolveEq` is a no-op
//! evaluator — it exits before touching any derivative slot — so the SolveEq
//! tests pin that no-op (leaving the caller's derivative untouched), witnessed
//! live against the capi015 oracle; see `dynamic_exp.rs::solve_eq`.

use super::*;

/// Build an object, set its state-variable names + expression, and compile.
fn compile(vars: &[&str], expr: &str) -> DynamicExpObj {
    let mut o = DynamicExpObj::new("d");
    o.set_string_list(VARNAMES, vars.iter().map(|s| s.to_string()).collect());
    o.set_string(EXPRESSION, expr.to_string());
    o.side_effects(EXPRESSION, 0);
    o
}

/// Whether the object queued any `DoSimpleMsg`-style error during its last edit.
fn has_errors(o: &mut DynamicExpObj) -> bool {
    !o.data_mut().take_errors().is_empty()
}

#[test]
fn kundur_expression_compiles_to_expected_cmds() {
    // The vendored corpus expression (Dynamic_KundurDynExp.dss):
    //   Speed dt = -1 Mass / ( Pterm Damp Speed * + Pshaft - ) *; theta dt = Speed
    // vars: speed=0 mass=1 pshaft=2 pterm=3 damp=4 theta=5.
    let o = compile(
        &["speed", "mass", "pshaft", "pterm", "damp", "theta"],
        "Speed dt = -1 Mass / ( Pterm Damp Speed * + Pshaft - ) *; theta dt = Speed",
    );
    assert_eq!(
        o.cmds,
        vec![
            0, -50, // speed dt =
            50000, 1, -5, // -1 mass /
            3, 4, 0, -4, // pterm damp speed *
            -2, // +
            2, -3, // pshaft -
            -4, // *
            5, -50, // theta dt =
            0,   // speed
        ],
    );
    assert_eq!(o.var_consts, vec![-1.0]);
    let mut o = o;
    assert!(!has_errors(&mut o));
    // The original input text is kept verbatim (Pascal leaves it unbracketed).
    assert_eq!(
        o.get_string(EXPRESSION),
        "Speed dt = -1 Mass / ( Pterm Damp Speed * + Pshaft - ) *; theta dt = Speed"
    );
}

// --- D14 (upstream `2a8bdb78`): SolveEq is a no-op evaluator ------------------
//
// The 0.15.x `Exit` returns from SolveEq as soon as the first equation's output
// index is latched — idx 0 of every well-formed `[out, EQ_MARK, ...]` stream —
// so no RHS is ever evaluated and every derivative slot is left exactly as the
// caller set it. The `cmds` -> RPN op-dispatch is therefore dynamically dead
// (retained 1:1 with the Pascal `case`, but unreachable for InterpretDiffEq
// output). The tests below still pin the InterpretDiffEq `cmds` layout
// (compilation is unchanged from 0.14.5), and assert the no-op leaves the
// derivative column untouched. `SENTINEL` in a derivative slot would be
// overwritten by any stray evaluation, so it catches a regression to the old
// behavior. Each expression's pre-D14 computed value is kept in a comment as the
// 0.14.5 contrast.

const SENTINEL: f64 = 1.2345e9;

#[test]
fn solve_eq_d14_freezes_derivative_index_bug_witness() {
    // The vendored Kundur two-equation expression. The 0.14.5 evaluator wrote
    // d(speed) = -1/mass*(pterm+damp*speed-pshaft) and d(theta) = speed; the
    // 0.15.x form (D14) exits at the first marker and touches nothing, so both
    // derivative slots stay at the caller's SENTINEL. Pinned against the capi015
    // oracle: a Generator with this DynamicEq reports dspeed = 0 / speed frozen,
    // where the 0.14.5 engine integrated dspeed = -1.6e-6 (DIVERGENCES.md §D14).
    let o = compile(
        &["speed", "mass", "pshaft", "pterm", "damp", "theta"],
        "Speed dt = -1 Mass / ( Pterm Damp Speed * + Pshaft - ) *; theta dt = Speed",
    );
    let mut mem = [
        [0.01, SENTINEL],   // speed  (pre-D14 -> d(speed) = -3e-5)
        [1000.0, SENTINEL], // mass
        [0.9, SENTINEL],    // pshaft
        [0.85, SENTINEL],   // pterm
        [2.0, SENTINEL],    // damp
        [0.5, SENTINEL],    // theta  (pre-D14 -> d(theta) = 0.01)
    ];
    o.solve_eq(&mut mem);
    // No-op: every derivative slot is exactly as passed in.
    for (row, m) in mem.iter().enumerate() {
        assert_eq!(m[1], SENTINEL, "row {row} derivative was mutated");
    }
}

#[test]
fn solve_eq_d14_noop_single_output_witness() {
    // Simple single-output expression `w dt = th`. Compilation is unchanged; the
    // 0.14.5 evaluator wrote d(w) = th = 7.0. Under D14 SolveEq is a no-op, so the
    // derivative slot keeps whatever the caller set.
    let o = compile(&["w", "th"], "w dt = th");
    assert_eq!(o.cmds, vec![0, -50, 1]);
    assert!(o.var_consts.is_empty());
    let mut mem = [[5.0, SENTINEL], [7.0, 0.0]];
    o.solve_eq(&mut mem);
    assert_eq!(mem[0][1], SENTINEL); // untouched (pre-D14: 7.0)
}

#[test]
fn interpret_diff_eq_compiles_operators_to_cmds() {
    // InterpretDiffEq (compilation) is unchanged by D14 — only SolveEq changed.
    // Pin the opcode -> cmds mapping for a spread of operators; the pre-D14
    // SolveEq value each would have produced is noted for reference. `pi` is the
    // nullary EnterPi operator, numeric literals are harvested into var_consts.
    let o = compile(&["w"], "w dt = pi 2 * w *"); // pre-D14: π·2·w
    assert_eq!(o.cmds, vec![0, -50, -27, 50000, -4, 0, -4]);
    assert_eq!(o.var_consts, vec![2.0]);
    let o = compile(&["w"], "w dt = w sqr"); // pre-D14: w²
    assert_eq!(o.cmds, vec![0, -50, 0, -11]);
    let o = compile(&["w"], "w dt = w inv"); // pre-D14: 1/w
    assert_eq!(o.cmds, vec![0, -50, 0, -13]);
    let o = compile(&["w"], "w dt = w ln"); // pre-D14: ln(w)
    assert_eq!(o.cmds, vec![0, -50, 0, -14]);
    let o = compile(&["w"], "w dt = w exp"); // pre-D14: e^w
    assert_eq!(o.cmds, vec![0, -50, 0, -15]);
    let o = compile(&["w"], "w dt = w 3 ^"); // pre-D14: w³ (3 = const)
    assert_eq!(o.cmds, vec![0, -50, 0, 50000, -28]);
    assert_eq!(o.var_consts, vec![3.0]);
    let o = compile(&["w", "a", "b"], "w dt = a b swap /"); // pre-D14: b/a
    assert_eq!(o.cmds, vec![0, -50, 1, 2, -26, -5]);
}

#[test]
fn substring_tiebreak_makes_sqrt_and_atan2_unreachable() {
    // Pascal `Get_Closer_Op` keeps the smallest position and, on a tie, the
    // earlier opcode index — so `sqr` (idx 11) always shadows `sqrt` (12) and
    // `atan` (22) shadows `atan2` (23) at the same position. The longer forms are
    // therefore dead opcodes the compiler can never emit; we port the quirk
    // verbatim. `w sqrt` compiles to `sqr` (-11), the trailing `t` swallowed by
    // the multi-char advance.
    let o = compile(&["w"], "w dt = w sqrt");
    assert_eq!(o.cmds, vec![0, -50, 0, -11]); // sqr, never -12 (sqrt)
    // `a atan2` → `atan` (-22), never -23.
    let o = compile(&["w", "a"], "w dt = a atan2");
    assert_eq!(o.cmds, vec![0, -50, 1, -22]); // atan, never -23 (atan2)
}

#[test]
fn unknown_variable_clears_expression() {
    let mut o = compile(&["a", "b"], "a dt = zzz");
    assert_eq!(o.get_string(EXPRESSION), ""); // cleared on error
    assert!(has_errors(&mut o)); // logged 50005 + 50003
}

#[test]
fn empty_operand_before_dt_is_recoverable_not_a_panic() {
    // Pascal `vars[0]` on an empty preceding sub-expression raises a *catchable*
    // EStringListError; the expression is left as written (the unwind skips the
    // clear-on-error path). The Rust port must reproduce a recoverable error, not
    // a panic (the oracle is pinned by props/dynamicexp.json::dynamicexp_empty_dt_operand).
    let mut o = compile(&["a", "b"], " dt = b");
    assert_eq!(o.get_string(EXPRESSION), " dt = b"); // not cleared
    assert!(has_errors(&mut o)); // "List index (0) out of bounds" logged
}

#[test]
fn var_active_resolves_index_and_missing_clears() {
    let mut o = DynamicExpObj::new("d");
    o.set_string_list(VARNAMES, vec!["a".into(), "b".into(), "c".into()]);
    // Existing variable → VarIdx is its position.
    o.set_string(VR, "b".into());
    o.side_effects(VR, 0);
    assert_eq!(o.get_i32(VARIDX), 1);
    assert_eq!(o.get_string(VR), "b");
    // Missing variable → VarIdx -1 and the active var is cleared (logs 50001).
    o.set_string(VR, "zzz".into());
    o.side_effects(VR, 0);
    assert_eq!(o.get_i32(VARIDX), -1);
    assert_eq!(o.get_string(VR), "");
}

#[test]
fn varidx_is_silent_read_only() {
    let mut o = DynamicExpObj::new("d");
    o.set_i32(VARIDX, 42); // Pascal SilentReadOnly: ignored
    assert_eq!(o.get_i32(VARIDX), -1);
}

#[test]
fn get_out_idx_flags_only_outputs() {
    let o = compile(
        &["speed", "mass", "pshaft", "pterm", "damp", "theta"],
        "Speed dt = -1 Mass / ( Pterm Damp Speed * + Pshaft - ) *; theta dt = Speed",
    );
    assert_eq!(o.get_out_idx("speed"), 0); // an output (followed by -50)
    assert_eq!(o.get_out_idx("theta"), 5); // an output
    assert_eq!(o.get_out_idx("mass"), -1); // input only
    assert_eq!(o.get_out_idx("nope"), -1); // not a variable
}

#[test]
fn get_var_name_prefixes_derivative_columns() {
    let o = compile(&["speed", "mass"], "speed dt = mass");
    assert_eq!(o.get_var_name(0), "speed"); // row 0, col 0 (value)
    assert_eq!(o.get_var_name(1), "dspeed"); // row 0, col 1 (1st derivative)
    assert_eq!(o.get_var_name(2), "mass"); // row 1, col 0
    assert_eq!(o.get_var_name(3), "dmass"); // row 1, col 1
}

#[test]
fn is_init_val_and_check_if_calc_value() {
    assert!(DynamicExpObj::is_init_val(7)); // p0
    assert!(DynamicExpObj::is_init_val(8)); // q0
    assert!(DynamicExpObj::is_init_val(9)); // edp
    assert!(!DynamicExpObj::is_init_val(0));
    assert!(!DynamicExpObj::is_init_val(6));

    assert_eq!(DynamicExpObj::check_if_calc_value("p"), 0);
    assert_eq!(DynamicExpObj::check_if_calc_value("edp"), 9);
    assert_eq!(DynamicExpObj::check_if_calc_value("mod"), 11);
    assert_eq!(DynamicExpObj::check_if_calc_value("nope"), -1);
}
