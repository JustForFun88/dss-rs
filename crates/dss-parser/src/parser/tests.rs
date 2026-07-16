use super::Parser;
use crate::vars::ParserVars;

fn parser_with(cmd: &str) -> (Parser, ParserVars) {
    let mut p = Parser::new();
    p.set_cmd_string(cmd);
    (p, ParserVars::new())
}

#[test]
fn next_param_name_value_pairs() {
    let (mut p, vars) = parser_with("param1=value1 param2=value2");
    assert_eq!(p.next_param(&vars), "param1");
    assert_eq!(p.token(), "value1");
    assert_eq!(p.next_param(&vars), "param2");
    assert_eq!(p.token(), "value2");
    assert_eq!(p.next_param(&vars), "");
    assert_eq!(p.token(), "");
}

#[test]
fn equals_with_surrounding_spaces() {
    let (mut p, vars) = parser_with("kv = 12.47");
    assert_eq!(p.next_param(&vars), "kv");
    assert_eq!(p.token(), "12.47");
}

#[test]
fn positional_tokens_have_empty_param_names() {
    let (mut p, vars) = parser_with("New Line.L1 Bus1 Bus2");
    assert_eq!(p.next_param(&vars), "");
    assert_eq!(p.token(), "New");
    assert_eq!(p.next_param(&vars), "");
    assert_eq!(p.token(), "Line.L1");
    p.next_param(&vars);
    assert_eq!(p.token(), "Bus1");
}

#[test]
fn commas_are_delimiters() {
    let (mut p, vars) = parser_with("a,b, c");
    p.next_param(&vars);
    assert_eq!(p.token(), "a");
    p.next_param(&vars);
    assert_eq!(p.token(), "b");
    p.next_param(&vars);
    assert_eq!(p.token(), "c");
}

#[test]
fn all_quote_pairs() {
    for (open, close) in [('(', ')'), ('"', '"'), ('\'', '\''), ('[', ']'), ('{', '}')] {
        let (mut p, vars) = parser_with(&format!("x={open}alpha beta{close} next"));
        assert_eq!(p.next_param(&vars), "x");
        assert_eq!(p.token(), "alpha beta", "quote pair {open}{close}");
        p.next_param(&vars);
        assert_eq!(p.token(), "next");
    }
}

#[test]
fn bang_comment_stops_the_line() {
    let (mut p, vars) = parser_with("kw=5 ! the rest is comment kv=12");
    assert_eq!(p.next_param(&vars), "kw");
    assert_eq!(p.token(), "5");
    assert_eq!(p.next_param(&vars), "");
    assert_eq!(p.token(), "");
}

#[test]
fn double_slash_comment_stops_the_line() {
    let (mut p, vars) = parser_with("conn=delta // trailing comment");
    assert_eq!(p.next_param(&vars), "conn");
    assert_eq!(p.token(), "delta");
    p.next_param(&vars);
    assert_eq!(p.token(), "");
}

#[test]
fn slash_inside_token_is_not_a_comment() {
    let (mut p, vars) = parser_with("file=dir/sub/name.dss");
    assert_eq!(p.next_param(&vars), "file");
    assert_eq!(p.token(), "dir/sub/name.dss");
}

#[test]
fn make_complex_matches_capi015_token_forms() {
    // Reproduces the capi015 probe (0.15.0b4): single tokens give real-only or
    // pure-imaginary; `5+3i` is 0 because `5+3` is not two whitespace-separated
    // reals for FPC `ReadStr(spart, re, im)`.
    for (tok, want) in [
        ("5", (5.0, 0.0)),
        ("-2.5", (-2.5, 0.0)),
        ("3i", (0.0, 3.0)),
        ("3j", (0.0, 3.0)),
        ("5+3i", (0.0, 0.0)),
        ("-2.5-1.5i", (0.0, 0.0)),
        ("abc", (0.0, 0.0)),
    ] {
        let (mut p, vars) = parser_with(tok);
        p.next_param(&vars);
        assert_eq!(p.token(), tok);
        assert_eq!(p.make_complex(), want, "token {tok:?}");
    }
}

#[test]
fn parse_as_complex_vector_fills_expected_size_only() {
    let (mut p, vars) = parser_with("x=[5 6 7 8]");
    assert_eq!(p.next_param(&vars), "x");
    // ExpectedSize 2: first two kept, extras counted; backing tail preserved.
    let mut out = [(1.0, 1.0); 3];
    let found = p.parse_as_complex_vector(&vars, &mut out[..2]);
    assert_eq!(found, 4);
    assert_eq!(out[0], (5.0, 0.0));
    assert_eq!(out[1], (6.0, 0.0));
    assert_eq!(out[2], (1.0, 1.0), "slot past ExpectedSize is untouched");
}

#[test]
fn parse_as_complex_matrix_column_major() {
    let (mut p, vars) = parser_with("x=[1 2 | 3 4]");
    assert_eq!(p.next_param(&vars), "x");
    let mut out = [(0.0, 0.0); 4];
    assert_eq!(p.parse_as_complex_matrix(&vars, &mut out, 2), 2);
    // column-major: out[j*order + i] = row i col j
    assert_eq!(out, [(1.0, 0.0), (3.0, 0.0), (2.0, 0.0), (4.0, 0.0)]);
}

#[test]
fn make_double_accepts_fpc_val_forms() {
    // Forms verified against the reference parser (probe_val.py).
    for (s, expected) in [
        (".5", 0.5),
        ("5.", 5.0),
        ("1e3", 1000.0),
        ("1E3", 1000.0),
        ("+5", 5.0),
        ("-0.25", -0.25),
        ("1.5e-3", 0.0015),
        ("007", 7.0),
        ("-.5", -0.5),
        ("1e+3", 1000.0),
        ("1.e3", 1000.0),
    ] {
        let (mut p, vars) = parser_with(&format!("x={s}"));
        p.next_param(&vars);
        assert_eq!(p.make_double(&vars).unwrap(), expected, "input {s:?}");
    }
    // inf/nan are accepted by FPC Val
    let (mut p, vars) = parser_with("x=inf");
    p.next_param(&vars);
    assert_eq!(p.make_double(&vars).unwrap(), f64::INFINITY);
    let (mut p, vars) = parser_with("x=nan");
    p.next_param(&vars);
    assert!(p.make_double(&vars).unwrap().is_nan());
}

#[test]
fn make_double_rejects_what_fpc_rejects() {
    for s in ["$FF", "0xFF", "1_000", "5.5.5", "1e", "abc", "infinity"] {
        let (mut p, vars) = parser_with(&format!("x={s}"));
        p.next_param(&vars);
        assert!(p.make_double(&vars).is_err(), "input {s:?}");
        assert!(p.convert_error(), "input {s:?}");
    }
}

#[test]
fn make_double_empty_token_is_zero() {
    let (mut p, vars) = parser_with("");
    p.next_param(&vars);
    assert_eq!(p.make_double(&vars).unwrap(), 0.0);
    assert!(!p.convert_error());
}

#[test]
fn make_integer_radix_prefixes() {
    // Verified against the reference parser (probe_val.py).
    for (s, expected) in [
        ("$FF", 255),
        ("$ff", 255),
        ("0xFF", 255),
        ("%101", 5),
        ("&777", 511),
        ("007", 7),
        ("+5", 5),
        ("1e3", 1000),
    ] {
        let (mut p, vars) = parser_with(&format!("n={s}"));
        p.next_param(&vars);
        assert_eq!(p.make_integer(&vars).unwrap(), expected, "input {s:?}");
    }
}

#[test]
fn make_integer_rounds_ties_to_even() {
    // FPC Round is banker's rounding: 1.5 → 2, 2.5 → 2, .5 → 0.
    for (s, expected) in [("1.5", 2), ("2.5", 2), (".5", 0), ("-.5", 0), ("-0.25", 0)] {
        let (mut p, vars) = parser_with(&format!("n={s}"));
        p.next_param(&vars);
        assert_eq!(p.make_integer(&vars).unwrap(), expected, "input {s:?}");
    }
    // non-finite values collapse to 0 through the integer-indefinite path
    for s in ["inf", "nan"] {
        let (mut p, vars) = parser_with(&format!("n={s}"));
        p.next_param(&vars);
        assert_eq!(p.make_integer(&vars).unwrap(), 0, "input {s:?}");
    }
}

#[test]
fn make_integer_errors_match_fpc() {
    for s in ["1_000", "5.5.5", "1e", "abc"] {
        let (mut p, vars) = parser_with(&format!("n={s}"));
        p.next_param(&vars);
        assert!(p.make_integer(&vars).is_err(), "input {s:?}");
    }
}

#[test]
fn quoted_token_evaluates_as_rpn() {
    let (mut p, vars) = parser_with("x=(1 2 +)");
    p.next_param(&vars);
    let (v, required) = p.make_double_ex(&vars).unwrap();
    assert_eq!(v, 3.0);
    assert!(required);
}

#[test]
fn quoted_single_number_is_not_required_rpn() {
    let (mut p, vars) = parser_with("x=(42.5)");
    p.next_param(&vars);
    let (v, required) = p.make_double_ex(&vars).unwrap();
    assert_eq!(v, 42.5);
    assert!(!required);
}

#[test]
fn rpn_operations_and_functions() {
    for (expr, expected) in [
        ("(2 3 *)", 6.0),
        ("(10 4 -)", 6.0),
        ("(3 4 /)", 0.75),
        ("(2 10 ^)", 1024.0),
        ("(9 sqrt)", 3.0),
        ("(3 sqr)", 9.0),
        ("(0.5 inv)", 2.0),
        ("(10 exp ln)", 10.0),
        ("(1000 log10)", 3.0),
    ] {
        let (mut p, vars) = parser_with(&format!("x={expr}"));
        p.next_param(&vars);
        let v = p.make_double(&vars).unwrap();
        assert!(
            (v - expected).abs() < 1e-12,
            "{expr} → {v}, want {expected}"
        );
    }
    // trig in degrees
    let (mut p, vars) = parser_with("x=(30 sin)");
    p.next_param(&vars);
    assert!((p.make_double(&vars).unwrap() - 0.5).abs() < 1e-12);
}

#[test]
fn rpn_invalid_entry_errors() {
    let (mut p, vars) = parser_with("x=(1 2 bogus)");
    p.next_param(&vars);
    let err = p.make_double(&vars).unwrap_err();
    assert!(err.message().contains("Invalid inline math entry"));
}

#[test]
fn rpn_x_register_persists_between_calls() {
    // The Pascal calculator instance lives on the parser; its stack is
    // never reset between commands.
    let (mut p, vars) = parser_with("a=(10) b=(2 +)");
    p.next_param(&vars);
    assert_eq!(p.make_double(&vars).unwrap(), 10.0);
    p.next_param(&vars);
    assert_eq!(p.make_double(&vars).unwrap(), 12.0);
}

#[test]
fn variable_substitution_plain() {
    let (mut p, mut vars) = parser_with("kv=@v");
    vars.add("@v", "12.47");
    p.next_param(&vars);
    assert_eq!(p.token(), "12.47");
    assert_eq!(p.make_double(&vars).unwrap(), 12.47);
}

#[test]
fn variable_substitution_keeps_dot_suffix() {
    let (mut p, mut vars) = parser_with("bus1=@mybus.1.2.3");
    vars.add("@mybus", "alpha");
    p.next_param(&vars);
    assert_eq!(p.token(), "alpha.1.2.3");
    let (name, nodes) = p.parse_as_bus_name("alpha.1.2.3", &vars).unwrap();
    assert_eq!(name, "alpha");
    assert_eq!(nodes, vec![1, 2, 3]);
}

#[test]
fn variable_defined_with_variables_forces_rpn() {
    let (mut p, mut vars) = parser_with("x=@b");
    vars.add("@a", "2");
    vars.add("@b", "@a 3 *"); // stored as {@a 3 *}
    p.next_param(&vars);
    assert_eq!(p.make_double(&vars).unwrap(), 6.0);
}

#[test]
fn unknown_variable_is_left_alone() {
    let (mut p, vars) = parser_with("x=@nosuchvar");
    p.next_param(&vars);
    assert_eq!(p.token(), "@nosuchvar");
}

#[test]
fn bus_name_parsing() {
    let (mut p, vars) = parser_with("");
    let (name, nodes) = p.parse_as_bus_name("Bus1.1.2.3", &vars).unwrap();
    assert_eq!(name, "Bus1");
    assert_eq!(nodes, vec![1, 2, 3]);

    let (name, nodes) = p.parse_as_bus_name("SourceBus", &vars).unwrap();
    assert_eq!(name, "SourceBus");
    assert!(nodes.is_empty());

    // explicit ground connections
    let (name, nodes) = p.parse_as_bus_name("b2.1.0.0", &vars).unwrap();
    assert_eq!(name, "b2");
    assert_eq!(nodes, vec![1, 0, 0]);
}

#[test]
fn bus_name_restores_parser_state() {
    let (mut p, vars) = parser_with("next=42");
    p.parse_as_bus_name("Bus1.1.2", &vars).unwrap();
    // delimiters were restored: normal parsing continues to work
    assert_eq!(p.next_param(&vars), "next");
    assert_eq!(p.make_integer(&vars).unwrap(), 42);
}

#[test]
fn vector_parsing_from_bracket_token() {
    let (mut p, vars) = parser_with("kvs=[1.0 2.5 3.5]");
    p.next_param(&vars);
    let mut out = [0.0; 3];
    let n = p.parse_as_vector(&vars, &mut out, false).unwrap();
    assert_eq!(n, 3);
    assert_eq!(out, [1.0, 2.5, 3.5]);
}

#[test]
fn vector_counts_extras_but_drops_them() {
    let (mut p, vars) = parser_with("v=[1 2 3 4 5]");
    p.next_param(&vars);
    let mut out = [0.0; 3];
    let n = p.parse_as_vector(&vars, &mut out, false).unwrap();
    assert_eq!(n, 5); // found 5, stored 3 — Pascal reports the count found
    assert_eq!(out, [1.0, 2.0, 3.0]);
}

#[test]
fn vector_with_commas_and_rounding() {
    let (mut p, vars) = parser_with("v=(1.4, 2.5, 3.6)");
    p.next_param(&vars);
    let mut out = [0.0; 3];
    let n = p.parse_as_vector(&vars, &mut out, true).unwrap();
    assert_eq!(n, 3);
    assert_eq!(out, [1.0, 2.0, 4.0]); // 2.5 rounds to even
}

#[test]
fn matrix_rows_are_stored_column_major() {
    let (mut p, vars) = parser_with("m=[1 2 | 3 4]");
    p.next_param(&vars);
    let mut out = [0.0; 4];
    let n = p.parse_as_matrix(&vars, &mut out, 2).unwrap();
    assert_eq!(n, 2);
    // column-major: [a11 a21 a12 a22]
    assert_eq!(out, [1.0, 3.0, 2.0, 4.0]);
}

#[test]
fn sym_matrix_lower_triangle_fills_both_sides() {
    let (mut p, vars) = parser_with("z=[2 | -1 3]");
    p.next_param(&vars);
    let mut out = [0.0; 4];
    let n = p.parse_as_sym_matrix(&vars, &mut out, 2, 1, 1.0).unwrap();
    assert_eq!(n, 2);
    assert_eq!(out, [2.0, -1.0, -1.0, 3.0]);
}

#[test]
fn sym_matrix_with_stride_and_scale() {
    // stride 2 leaves gaps (the engine interleaves re/im parts)
    let (mut p, vars) = parser_with("z=[1 | 2 4]");
    p.next_param(&vars);
    let mut out = [9.0; 8];
    p.parse_as_sym_matrix(&vars, &mut out, 2, 2, 10.0).unwrap();
    assert_eq!(out, [10.0, 9.0, 20.0, 9.0, 20.0, 9.0, 40.0, 9.0]);
}

#[test]
fn is_quoted_reflects_the_last_token_quote_state() {
    // WP-U1.1 item 3 "WasQuoted plumbing": the exposed accessor mirrors the
    // parser's internal quote tracking (a quoted composite vs a bare token).
    let (mut p, vars) = parser_with("a=(1 2 3) b=bare");
    p.next_param(&vars);
    let _ = p.token();
    assert!(p.is_quoted(), "(1 2 3) is a quoted composite");
    p.next_param(&vars);
    let _ = p.token();
    assert!(!p.is_quoted(), "bare token is not quoted");
}

#[test]
fn sym_matrix_returns_order_found_for_incomplete_input() {
    // WP-U1.1 item 2: a 3x3 sym matrix given only 2 rows returns OrderFound=2
    // (< 3) so the caller can reject it (EPRI r4133); the FPC line returned 3.
    let (mut p, vars) = parser_with("z=[1 | 2 3]");
    p.next_param(&vars);
    let mut out = [0.0; 9];
    let n = p.parse_as_sym_matrix(&vars, &mut out, 3, 1, 1.0).unwrap();
    assert_eq!(n, 2, "two rows supplied -> OrderFound 2");
    // A complete matrix returns OrderFound == order.
    let (mut p2, vars2) = parser_with("z=[1 | 2 3 | 4 5 6]");
    p2.next_param(&vars2);
    let mut out2 = [0.0; 9];
    let n2 = p2
        .parse_as_sym_matrix(&vars2, &mut out2, 3, 1, 1.0)
        .unwrap();
    assert_eq!(n2, 3);
}

#[test]
fn matrix_full_rows_no_terminator() {
    // 3x3 matrix as three rows
    let (mut p, vars) = parser_with("m=[1 2 3 | 4 5 6 | 7 8 9]");
    p.next_param(&vars);
    let mut out = [0.0; 9];
    p.parse_as_matrix(&vars, &mut out, 3).unwrap();
    // column-major check of a couple of entries: a21 = 4 at index 1,
    // a13 = 3 at index 6
    assert_eq!(out[1], 4.0);
    assert_eq!(out[6], 3.0);
    assert_eq!(out[8], 9.0);
}

#[test]
fn remainder_and_position_roundtrip() {
    let (mut p, vars) = parser_with("a=1 b=2 c=3");
    p.next_param(&vars);
    let saved = p.position();
    let rem_before = p.remainder().to_string();
    p.next_param(&vars);
    assert_ne!(p.remainder(), rem_before);
    p.set_position(saved);
    assert_eq!(p.remainder(), rem_before);
    assert_eq!(p.next_param(&vars), "b");
}

#[test]
fn reset_delims_restores_defaults() {
    let mut p = Parser::new();
    p.set_delimiters(";");
    p.set_whitespace(" ");
    p.set_begin_quote_chars("<");
    p.set_end_quote_chars(">");
    p.reset_delims();
    assert_eq!(p.delimiters(), ",=");
    assert_eq!(p.whitespace(), " \t");
    assert_eq!(p.begin_quote_chars(), "(\"'[{");
    assert_eq!(p.end_quote_chars(), ")\"']}");
}

#[test]
fn custom_delimiters() {
    let mut p = Parser::new();
    let vars = ParserVars::new();
    p.set_delimiters(",=;");
    p.set_cmd_string("a;b;c");
    p.next_param(&vars);
    assert_eq!(p.token(), "a");
    p.next_param(&vars);
    assert_eq!(p.token(), "b");
    p.next_param(&vars);
    assert_eq!(p.token(), "c");
}

#[test]
fn auto_increment_advances_in_make_string() {
    let (mut p, vars) = parser_with("one two three");
    p.set_auto_increment(true);
    assert_eq!(p.make_string(&vars), "one");
    assert_eq!(p.make_string(&vars), "two");
    assert_eq!(p.make_string(&vars), "three");
}

#[test]
fn tab_is_whitespace() {
    let (mut p, vars) = parser_with("a\tb");
    p.next_param(&vars);
    assert_eq!(p.token(), "a");
    p.next_param(&vars);
    assert_eq!(p.token(), "b");
}

#[test]
fn utf8_content_passes_through_tokens() {
    let (mut p, vars) = parser_with("file=\"путь/к файлу.dss\" kv=12");
    assert_eq!(p.next_param(&vars), "file");
    assert_eq!(p.token(), "путь/к файлу.dss");
    assert_eq!(p.next_param(&vars), "kv");
    assert_eq!(p.make_double(&vars).unwrap(), 12.0);
}

#[test]
fn unterminated_quote_runs_to_end_of_line() {
    let (mut p, vars) = parser_with("x=(1 2");
    p.next_param(&vars);
    // Pascal scans to the end (the appended space is the last char)
    assert_eq!(p.token(), "1 2");
}
