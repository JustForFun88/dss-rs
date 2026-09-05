//! Unit tests for the incidence-matrix helpers that are hand-checkable without a
//! full circuit. The end-to-end `Calc_Inc_Matrix`/`Calc_Inc_Matrix_Org` builds
//! and the five exports are pinned byte-exact against the oracle in
//! `crates/dss-core/tests/inc_matrix_reports.rs`.

use super::*;

#[test]
fn strip_bus_cuts_at_first_dot() {
    assert_eq!(strip_bus("bus1.1.2.3"), "bus1");
    assert_eq!(strip_bus("bus1.0"), "bus1");
    assert_eq!(strip_bus("sourcebus"), "sourcebus");
    assert_eq!(strip_bus(""), "");
}

fn incidence_2x3() -> SparseInt {
    // Two branches over three buses: rows 0 and 1, cols 0/1/2.
    let mut m = SparseInt::new();
    m.insert(0, 0, 1);
    m.insert(0, 1, -1);
    m.insert(1, 1, 1);
    m.insert(1, 2, -1);
    m
}

#[test]
fn get_inc_matrix_row_scans_from_index_one() {
    let m = incidence_2x3();
    // col 1 first appears at data[1] (row 0).
    assert_eq!(get_inc_matrix_row(&m, 1), 0);
    // col 0 only appears at data[0], which the scan (starting at index 1) skips.
    assert_eq!(get_inc_matrix_row(&m, 0), -1);
    // col 2 at data[3] (row 1).
    assert_eq!(get_inc_matrix_row(&m, 2), 1);
    // a column that does not exist.
    assert_eq!(get_inc_matrix_row(&m, 9), -1);
}

#[test]
fn get_inc_matrix_col_returns_link_columns() {
    let m = incidence_2x3();
    // row 1 first appears at data[2] (col 1); data[idx+1] = data[3] col 2.
    let (result, active_cols) = get_inc_matrix_col(&m, 1);
    assert_eq!(result, 1);
    assert_eq!(active_cols, [1, 2]);
    // row 0 at data[1] would be scanned from index 1 → col 1, next col from data[2].
    let (r0, ac0) = get_inc_matrix_col(&m, 0);
    assert_eq!(r0, 1);
    assert_eq!(ac0, [1, 1]);
}

// --- The reactor row cursor (GOLDEN_REBASE G1.8, settlement S-INC) -----------
//
// r4133 `Version8/Source/Common/Solution.pas:3039` (dss_capi 0.14.5
// `Common/Solution.pas:1501`) advances `ActiveIncCell[0]` for **every** reactor,
// outside the `if BusdotIdx = 0` guard, so a shunt reactor consumes a matrix row
// index without appending a name to `Inc_Mat_Rows`; the three sibling walks
// advance only on an emitted row (`:2885` / `:2938` / `:2986`). The port emits
// dense rows (CLAUDE.md: no upstream bug is reproduced in any lane); the corpus
// gate re-derives upstream's index positively instead of excluding it.

/// The deck the two order tests share: three buses in a line plus a shunt reactor
/// at the far end. `series_first` only swaps the declaration order of the two
/// reactors, so `BusList` (sourcebus, a, b, c) is identical either way.
fn reactor_order_deck(series_first: bool) -> crate::exec::Dss {
    let mut dss = crate::exec::Dss::new();
    let series = "new reactor.r1 phases=3 bus1=a bus2=b R=1 X=5";
    let shunt = "new reactor.rsh phases=3 bus1=c kvar=300 kv=12.47";
    let mut deck = vec![
        "new circuit.serreacgap basekv=12.47 bus1=sourcebus",
        "new line.l1 phases=3 bus1=sourcebus bus2=a length=1",
        "new line.l2 phases=3 bus1=b bus2=c length=1",
        "new load.ld1 phases=3 bus1=c kv=12.47 kw=500 pf=0.95",
    ];
    if series_first {
        deck.push(series);
        deck.push(shunt);
    } else {
        deck.push(shunt);
        deck.push(series);
    }
    deck.extend([
        "set voltagebases=[12.47]",
        "calcvoltagebases",
        "CalcIncMatrix",
    ]);
    for cmd in deck {
        dss.command(cmd);
    }
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss
}

/// A shunt reactor declared **before** a series one no longer costs the series
/// reactor its row: `Reactor.r1` lands at matrix row **2** with
/// `rows[2] == "Reactor.r1"`, where upstream puts it at row **3** although
/// `Inc_Mat_Rows` only has 3 entries (valid indices 0..=2).
#[test]
fn the_reactor_row_cursor_advances_only_on_an_emitted_row() {
    let dss = reactor_order_deck(false);
    let ckt = dss.circuit().expect("circuit");
    let st = &ckt.solution.inc_matrix;
    assert_eq!(st.rows, ["Line.l1", "Line.l2", "Reactor.r1"]);
    let data = &st.inc_mat.as_ref().expect("CalcIncMatrix ran").data;
    // Columns: sourcebus=0, a=1, b=2, c=3.
    assert_eq!(
        data.as_slice(),
        &[
            [0, 0, 1],
            [0, 1, -1],
            [1, 2, 1],
            [1, 3, -1],
            [2, 1, 1],
            [2, 2, -1],
        ],
        "the series reactor must occupy row 2, not upstream's row 3"
    );
    // The dense-row contract: every stored row index names a PD element.
    let max_row = data.iter().map(|d| d[0]).max().expect("non-empty");
    assert_eq!(max_row, st.rows.len() as i32 - 1);
}

/// The emitted row index no longer depends on how many shunt reactors precede
/// the series one: the same circuit declared series-first and shunt-first yields
/// byte-identical `IncMat` content and row names.
#[test]
fn the_flat_incidence_matrix_is_blind_to_the_reactor_declaration_order() {
    let a = reactor_order_deck(true);
    let b = reactor_order_deck(false);
    let (sa, sb) = (
        &a.circuit().expect("circuit").solution.inc_matrix,
        &b.circuit().expect("circuit").solution.inc_matrix,
    );
    assert_eq!(sa.rows, sb.rows);
    assert_eq!(
        sa.inc_mat.as_ref().expect("built").data,
        sb.inc_mat.as_ref().expect("built").data
    );
}

/// Several shunt reactors interleaved with series ones (and a shunt capacitor,
/// whose walk never had the defect): every stored row index still indexes
/// `Inc_Mat_Rows`, and the reactor rows follow the Line/Xfmr/Cap rows in
/// declaration order.
#[test]
fn every_flat_incidence_row_indexes_a_row_name() {
    let mut dss = crate::exec::Dss::new();
    for cmd in [
        "new circuit.reacgaps basekv=12.47 bus1=sourcebus",
        "new line.l1 phases=3 bus1=sourcebus bus2=a length=1",
        "new line.l2 phases=3 bus1=b bus2=c length=1",
        "new line.l3 phases=3 bus1=d bus2=e length=1",
        "new capacitor.csh phases=3 bus1=e kvar=300 kv=12.47",
        "new reactor.sh1 phases=3 bus1=a kvar=100 kv=12.47",
        "new reactor.ser1 phases=3 bus1=a bus2=b R=1 X=5",
        "new reactor.sh2 phases=3 bus1=c kvar=100 kv=12.47",
        "new reactor.sh3 phases=3 bus1=c kvar=100 kv=12.47",
        "new reactor.ser2 phases=3 bus1=c bus2=d R=1 X=5",
        "new load.ld1 phases=3 bus1=e kv=12.47 kw=500 pf=0.95",
        "set voltagebases=[12.47]",
        "calcvoltagebases",
        "CalcIncMatrix",
    ] {
        dss.command(cmd);
    }
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let ckt = dss.circuit().expect("circuit");
    let st = &ckt.solution.inc_matrix;
    assert_eq!(
        st.rows,
        [
            "Line.l1",
            "Line.l2",
            "Line.l3",
            "Reactor.ser1",
            "Reactor.ser2"
        ]
    );
    let data = &st.inc_mat.as_ref().expect("CalcIncMatrix ran").data;
    let n = st.rows.len() as i32;
    for d in data {
        assert!(
            d[0] >= 0 && d[0] < n,
            "row {} does not index Inc_Mat_Rows (len {n})",
            d[0]
        );
    }
    // Upstream's cursor is `3 + j` for the j-th reactor in declaration order, so
    // it would put `Reactor.ser1` (j = 1) at row 4 and `Reactor.ser2` (j = 4) at
    // row 7 with only five row names; the port emits rows 3 and 4.
    let reactor_rows: Vec<i32> = data.iter().map(|d| d[0]).filter(|&r| r >= 3).collect();
    assert_eq!(reactor_rows, [3, 3, 4, 4]);
}
