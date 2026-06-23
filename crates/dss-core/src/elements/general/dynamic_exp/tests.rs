//! Unit tests for the `DynamicExp` interpreter/evaluator (`InterpretDiffEq` →
//! `cmds`, `SolveEq`). The property dump is pinned against the oracle by the
//! `props_roundtrip` golden (`tests/golden/props/dynamicexp.json`); these tests
//! exercise the compilation + evaluation that the dump cannot reach (the oracle
//! does not expose `cmds`/`SolveEq` outside a dynamics solve — that arrives,
//! oracle-pinned, with WP7.7). Values are hand-derived from the Pascal algorithm.

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

#[test]
fn kundur_expression_evaluates() {
    let o = compile(
        &["speed", "mass", "pshaft", "pterm", "damp", "theta"],
        "Speed dt = -1 Mass / ( Pterm Damp Speed * + Pshaft - ) *; theta dt = Speed",
    );
    // mem rows: [value, derivative]; only value (col 0) is read.
    let mut mem = [
        [0.01, 0.0],   // speed
        [1000.0, 0.0], // mass
        [0.9, 0.0],    // pshaft
        [0.85, 0.0],   // pterm
        [2.0, 0.0],    // damp
        [0.5, 0.0],    // theta (unused on the RHS)
    ];
    o.solve_eq(&mut mem);
    // d(speed) = -1/mass * (pterm + damp*speed - pshaft)
    let d_speed = -1.0 / 1000.0 * (0.85 + 2.0 * 0.01 - 0.9);
    assert!(
        (mem[0][1] - d_speed).abs() < 1e-15,
        "{} vs {}",
        mem[0][1],
        d_speed
    );
    // d(theta) = speed
    assert!((mem[5][1] - 0.01).abs() < 1e-15);
}

#[test]
fn trivial_expression_compiles_and_evaluates() {
    let o = compile(&["w", "th"], "w dt = th");
    assert_eq!(o.cmds, vec![0, -50, 1]);
    assert!(o.var_consts.is_empty());
    let mut mem = [[5.0, 0.0], [7.0, 0.0]];
    o.solve_eq(&mut mem);
    assert_eq!(mem[0][1], 7.0); // d(w) = th
}

#[test]
fn pi_and_constant_operators() {
    // `pi` is the nullary operator (full-precision π via EnterPi), `2` a constant.
    let o = compile(&["w"], "w dt = pi 2 * w *");
    assert_eq!(o.cmds, vec![0, -50, -27, 50000, -4, 0, -4]);
    assert_eq!(o.var_consts, vec![2.0]);
    let mut mem = [[3.0, 0.0]];
    o.solve_eq(&mut mem);
    let expected = std::f64::consts::PI * 2.0 * 3.0;
    assert!(
        (mem[0][1] - expected).abs() < 1e-12,
        "{} vs {}",
        mem[0][1],
        expected
    );
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
