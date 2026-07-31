use super::*;
use crate::obj::base::DssObject;
use crate::obj::props::PropEngine;
use dss_parser::{Parser, ParserVars};

/// Build a LineCode, apply edits, return (class, obj) for querying.
fn edited(edits: &[(&str, &str)]) -> (ClassProps, LineCodeObj, crate::diag::ErrorLog) {
    let enums = EnumRegistry::new();
    let cls = class_props(&enums);
    let mut obj = LineCodeObj::new("lc");
    let mut parser = Parser::new();
    let vars = ParserVars::new();
    let mut errors = crate::diag::ErrorLog::new();
    for (name, value) in edits {
        let idx = cls.property_index(name).expect("known property");
        let mut eng = PropEngine {
            parser: &mut parser,
            vars: &vars,
            enums: &enums,
            errors: &mut errors,
            foreign: None,
        };
        cls.edit_property(&mut obj, idx, value, &mut eng).unwrap();
    }
    obj.end_edit(&crate::elements::traits::SysCtx::parse_default());
    errors.extend(obj.data_mut().take_errors());
    (cls, obj, errors)
}

fn get(cls: &ClassProps, obj: &LineCodeObj, name: &str) -> String {
    let enums = EnumRegistry::new();
    let idx = cls.property_index(name).unwrap();
    cls.get_value(obj, idx, &enums)
}

/// Parse a sym-matrix dump `[a |b c |...]`, assert the lower-triangle row
/// structure matches `rows` and the numbers are close (the exact float
/// rendering differs from the oracle but the props gate is tolerance-based;
/// the bracket/space format is locked separately in `matrix_model_*`).
fn assert_matrix(actual: &str, rows: &[&[f64]]) {
    assert!(
        actual.starts_with('[') && actual.ends_with(']'),
        "bad brackets: {actual}"
    );
    let inner = &actual[1..actual.len() - 1];
    let parsed: Vec<Vec<f64>> = inner
        .split('|')
        .map(|row| {
            row.split_whitespace()
                .map(|t| t.parse::<f64>().unwrap())
                .collect()
        })
        .collect();
    assert_eq!(parsed.len(), rows.len(), "row count: {actual}");
    for (p, e) in parsed.iter().zip(rows) {
        assert_eq!(p.len(), e.len(), "row width: {actual}");
        for (a, b) in p.iter().zip(e.iter()) {
            assert!(
                (a - b).abs() <= 1e-9 + 1e-9 * b.abs(),
                "{a} vs {b} in {actual}"
            );
        }
    }
}

#[test]
fn default_sym_matrices_match_oracle() {
    // Oracle (dss-python 0.15.7) for a fresh `new linecode.x`.
    let (cls, obj, errs) = edited(&[]);
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(get(&cls, &obj, "NPhases"), "3");
    assert_eq!(get(&cls, &obj, "Units"), "none");
    assert_eq!(get(&cls, &obj, "Neutral"), "3");
    assert_eq!(get(&cls, &obj, "Repair"), "0");
    assert_eq!(get(&cls, &obj, "Kron"), "No");
    // RMatrix: Zs.re = (2*0.058 + 0.1784)/3, Zm.re = (0.1784-0.058)/3.
    let zs = (2.0 * 0.058 + 0.1784) / 3.0;
    let zm = (0.1784 - 0.058) / 3.0;
    assert_matrix(
        &get(&cls, &obj, "RMatrix"),
        &[&[zs], &[zm, zs], &[zm, zm, zs]],
    );
    // CMatrix in nF: Ys = (2*3.4 + 1.6)/3 = 2.8, Ym = (1.6-3.4)/3 = -0.6.
    assert_matrix(
        &get(&cls, &obj, "CMatrix"),
        &[&[2.8], &[-0.6, 2.8], &[-0.6, -0.6, 2.8]],
    );
}

#[test]
fn incomplete_sym_matrix_rejected_keeps_default() {
    // WP-U1.1 item 2 (DIVERGENCES.md §ParseAsSymMatrix): EPRI r4133 rejects a
    // symmetric matrix supplying fewer rows than NPhases — the property keeps
    // its prior (default) value and a DoSimpleMsg-and-continue error is logged.
    // The FPC line (0.14.5/0.15.x) silently zero-fills the missing row instead.
    let (cls, obj, errs) = edited(&[
        ("nphases", "3"),
        ("rmatrix", "1 | 2 3"), // only 2 of 3 rows
    ]);
    // Error logged, but the object survives and rmatrix reverted to the default
    // symmetric-component matrix (NOT the zero-filled [1 |2 3 |0 0 0]).
    assert!(
        errs.iter()
            .any(|e| e.contains("does not match with the expected order")),
        "expected reject message, got {errs:?}"
    );
    let zs = (2.0 * 0.058 + 0.1784) / 3.0;
    let zm = (0.1784 - 0.058) / 3.0;
    assert_matrix(
        &get(&cls, &obj, "RMatrix"),
        &[&[zs], &[zm, zs], &[zm, zm, zs]],
    );
}

#[test]
fn complete_sym_matrix_still_accepted() {
    // Guard the item-2 change against over-rejection: a full lower triangle
    // (OrderFound == NPhases) parses and stores exactly.
    let (cls, obj, errs) = edited(&[("nphases", "3"), ("rmatrix", "1 | 2 3 | 4 5 6")]);
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(get(&cls, &obj, "RMatrix"), "[1 |2 3 |4 5 6 ]");
}

#[test]
fn one_phase_has_no_positive_seq_special_case() {
    // Oracle: `r1=0.1 x1=0.2 units=mi` on a 1-phase code dumps
    // rmatrix=[0.126133333333333 ] because R0/X0 keep their defaults.
    let (cls, obj, errs) = edited(&[
        ("nphases", "1"),
        ("r1", "0.1"),
        ("x1", "0.2"),
        ("units", "mi"),
    ]);
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(get(&cls, &obj, "units"), "mi");
    assert_eq!(get(&cls, &obj, "r1"), "0.1"); // no units scaling at code level
    assert_matrix(
        &get(&cls, &obj, "rmatrix"),
        &[&[(2.0 * 0.1 + 0.1784) / 3.0]],
    );
}

#[test]
fn matrix_model_hides_sym_scalars() {
    let (cls, obj, errs) = edited(&[
        ("nphases", "2"),
        ("rmatrix", "0.1 | 0.05 0.1"),
        ("xmatrix", "0.2 | 0.07 0.2"),
        ("cmatrix", "3 | -1 3"),
    ]);
    assert!(errs.is_empty(), "{errs:?}");
    for p in ["R1", "X1", "R0", "X0", "C1", "C0", "B1", "B0"] {
        assert_eq!(get(&cls, &obj, p), "----", "property {p}");
    }
    // RMatrix/XMatrix have scale 1.0, so the dump is exact — this locks the
    // `[v |v v |...]` bracket/space format against the oracle.
    assert_eq!(get(&cls, &obj, "RMatrix"), "[0.1 |0.05 0.1 ]");
    assert_eq!(get(&cls, &obj, "XMatrix"), "[0.2 |0.07 0.2 ]");
    assert_matrix(&get(&cls, &obj, "CMatrix"), &[&[3.0], &[-1.0, 3.0]]);
    assert_eq!(get(&cls, &obj, "Neutral"), "2");
}

#[test]
fn kron_reduction_eliminates_neutral() {
    // Oracle: 4-phase symmetric matrices, kron=y -> 3-phase result with
    // diag 0.0842/0.1596 and off-diag 0.0242/0.0496; neutral -> 0.
    let (cls, obj, errs) = edited(&[
        ("nphases", "4"),
        (
            "rmatrix",
            "0.1 | 0.04 0.1 | 0.04 0.04 0.1 | 0.04 0.04 0.04 0.1",
        ),
        (
            "xmatrix",
            "0.2 | 0.09 0.2 | 0.09 0.09 0.2 | 0.09 0.09 0.09 0.2",
        ),
        (
            "cmatrix",
            "2.8 | -0.6 2.8 | -0.6 -0.6 2.8 | -0.6 -0.6 -0.6 2.8",
        ),
        ("kron", "y"),
    ]);
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(get(&cls, &obj, "NPhases"), "3");
    assert_eq!(get(&cls, &obj, "Neutral"), "0");
    assert_eq!(get(&cls, &obj, "Kron"), "No");
    assert_matrix(
        &get(&cls, &obj, "RMatrix"),
        &[&[0.0842], &[0.0242, 0.0842], &[0.0242, 0.0242, 0.0842]],
    );
    assert_matrix(
        &get(&cls, &obj, "XMatrix"),
        &[&[0.1596], &[0.0496, 0.1596], &[0.0496, 0.0496, 0.1596]],
    );
}

#[test]
fn kron_on_one_phase_errors_and_is_noop() {
    let (cls, obj, errs) = edited(&[
        ("nphases", "1"),
        ("rmatrix", "0.1"),
        ("xmatrix", "0.2"),
        ("cmatrix", "3"),
        ("kron", "y"),
    ]);
    assert_eq!(errs.len(), 1);
    assert!(errs[0].contains("1-phase LineCode"), "{:?}", errs);
    assert_eq!(get(&cls, &obj, "NPhases"), "1"); // unchanged
}

#[test]
fn make_like_copies_matrices() {
    let enums = EnumRegistry::new();
    let cls = class_props(&enums);
    let (_, base, _) = edited(&[("nphases", "2"), ("r1", "0.2"), ("x1", "0.4")]);
    let mut obj = LineCodeObj::new("derived");
    obj.make_like(&base);
    assert_eq!(get(&cls, &obj, "NPhases"), "2");
    assert_eq!(get(&cls, &obj, "R1"), "0.2");
    assert_eq!(get(&cls, &obj, "X1"), "0.4");
}

#[test]
fn line_type_pins_enum_ordinals() {
    use super::LineType;
    assert_eq!(LineType::Oh.ordinal(), 1);
    assert_eq!(LineType::Busbar.ordinal(), 12);
    for ord in 1..=12 {
        assert_eq!(LineType::from_ordinal(ord).unwrap().ordinal(), ord);
    }
    assert_eq!(LineType::from_ordinal(0), None);
    assert_eq!(LineType::from_ordinal(13), None);
}

/// The Stage F [`LINECODE_SYM_CLEAR_OMITS_C0`] row, pinned by expected value in
/// both lanes.
///
/// `TLineCodeObj.PropertySideEffects` lists every sequence quantity *except*
/// `C0` as one that sets `SymComponentsModel := TRUE` and clears the matrix
/// property tracking — a slip the Pascal source itself flags with a
/// `-- Missing?` comment. So upstream a LineCode left on the matrix model stays
/// there when only `C0=` is edited, and the value is inert. **Parity lane**:
/// the omission, what both gating oracles reproduce. **Default lane**: `C0`
/// behaves like its seven siblings.
///
/// [`LINECODE_SYM_CLEAR_OMITS_C0`]: crate::compat::LINECODE_SYM_CLEAR_OMITS_C0
#[test]
fn c0_model_selection_is_the_lane_kernel() {
    // Start on the *matrix* model (an rmatrix edit clears the sym flag), then
    // edit only `C0` — the one property whose side-effect case is missing.
    let (cls, obj, errors) = edited(&[
        ("nphases", "2"),
        ("rmatrix", "0.1 | 0.02 0.1"),
        ("xmatrix", "0.2 | 0.05 0.2"),
        ("cmatrix", "3 | -1 3"),
        ("c0", "1.5"),
    ]);
    assert!(errors.is_empty(), "{errors:?}");

    // Derived from the *lane*, never from the row's own alias — see
    // `isource::tests` for why (F-settle W4).
    let parity = crate::compat::ORACLE_PARITY;
    assert_eq!(
        obj.sym_components_model, !parity,
        "parity reproduces the missing `C0` side-effect case (the source's own \
         `-- Missing?`); the default lane selects the sym model like `C1` does"
    );
    // The consequence at the property surface: `C0` is only *displayed* on the
    // sym model (`is_visible`), so upstream's `c0=` is not merely inert — the
    // value the user just wrote does not even read back.
    assert_eq!(
        get(&cls, &obj, "c0"),
        if parity { "----" } else { "1.5" },
        "parity keeps the object on the matrix model, where `C0` renders as \
         `----`; the default lane switched models, so the written value shows"
    );

    // Control: `C1` — the sibling the Pascal *does* list — switches the model
    // in both lanes, which is what makes `C0`'s omission a slip and not a rule.
    let (_, c1_obj, errors) = edited(&[
        ("nphases", "2"),
        ("rmatrix", "0.1 | 0.02 0.1"),
        ("xmatrix", "0.2 | 0.05 0.2"),
        ("cmatrix", "3 | -1 3"),
        ("c1", "1.5"),
    ]);
    assert!(errors.is_empty(), "{errors:?}");
    assert!(c1_obj.sym_components_model);
}
