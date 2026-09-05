//! GOLDEN_REBASE G1.8 — in-engine pins for [`Dss::inc_matrix_view`], the four
//! flat branch-to-node incidence quantities the live corpus gate compares
//! (`Solution.IncMatrix` / `.Laplacian` / `.IncMatrixRows` / `.IncMatrixCols`;
//! r4133 `Version8/Source/DDLL/DSolution.pas` `SolutionV` modes 1/5/3/4, capi
//! 0.14.5 `CAPI/CAPI_Solution.pas:897-925` / `:860-888` / `:953-970` /
//! `:979-1020`).
//!
//! Everything here is discrete, so every expectation is an exact integer, an
//! exact name or an exact vector — there is no tolerance on this surface. The
//! builder itself (`solution::inc_matrix`) is pinned byte-exact against the
//! oracle by `tests/golden/inc_matrix/`; what these tests own is the **API
//! getter semantics** the goldens do not exercise: the `IncMatrixCols` branch,
//! the reconstruction of upstream's row cursor
//! ([`view::IncMatrixView::upstream_row_index`]) and the no-diagnostics contract
//! of the `CalcIncMatrix` + `CalcLaplacian` pair.

use crate::exec::*;
use std::collections::BTreeMap;

/// Compile a corpus deck by absolute path and return the live engine.
///
/// The four decks read below write no files, so no directory guard is needed.
/// Never `.inputs/` — the vendored corpus tree is the one the live gate reads
/// (CLAUDE.md).
fn compile_corpus_deck(rel: &str) -> Dss {
    let deck = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus")
        .join(rel);
    assert!(deck.is_file(), "corpus deck missing: {deck:?}");
    let mut dss = Dss::new();
    dss.command(&format!("compile \"{}\"", deck.display()));
    assert!(dss.errors().is_empty(), "{rel}: {:?}", dss.errors());
    dss
}

/// Run a synthetic deck line by line and assert it compiled clean.
fn build(cmds: &[&str]) -> Dss {
    let mut dss = Dss::new();
    for c in cmds {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss
}

/// The deck the row-cursor pins share: two lines with a gap between them, a
/// **shunt** reactor and then a **series** reactor bridging the gap. Only the
/// reactors' declaration order matters; `BusList` is `sourcebus, a, b, c` either
/// way.
fn reactor_gap_deck() -> Dss {
    build(&[
        "new circuit.serreacgap basekv=12.47 bus1=sourcebus",
        "new line.l1 phases=3 bus1=sourcebus bus2=a length=1",
        "new line.l2 phases=3 bus1=b bus2=c length=1",
        "new load.ld1 phases=3 bus1=c kv=12.47 kw=500 pf=0.95",
        "new reactor.rsh phases=3 bus1=c kvar=300 kv=12.47",
        "new reactor.r1 phases=3 bus1=a bus2=b R=1 X=5",
        "set voltagebases=[12.47]",
        "calcvoltagebases",
    ])
}

/// `IncMatᵀ · IncMat` recomputed from the incidence triples, as a
/// `(row, col) -> value` map with the structural zeros dropped. Independent of
/// `support::sparse_math`'s `transpose`/`multiply` (whose storage order this
/// deliberately does not model — only the values).
fn gram(inc: &[[i32; 3]]) -> BTreeMap<(i32, i32), i32> {
    let mut by_row: BTreeMap<i32, Vec<(i32, i32)>> = BTreeMap::new();
    for t in inc {
        by_row.entry(t[0]).or_default().push((t[1], t[2]));
    }
    let mut g: BTreeMap<(i32, i32), i32> = BTreeMap::new();
    for cells in by_row.values() {
        for &(ci, vi) in cells {
            for &(cj, vj) in cells {
                *g.entry((ci, cj)).or_insert(0) += vi * vj;
            }
        }
    }
    g.retain(|_, v| *v != 0);
    g
}

/// The Laplacian triples as a `(row, col) -> value` map.
fn as_map(triples: &[[i32; 3]]) -> BTreeMap<(i32, i32), i32> {
    triples.iter().map(|t| ((t[0], t[1]), t[2])).collect()
}

/// **Pin 1 — the flat builder's row order, and only series branches get rows.**
///
/// `Calc_Inc_Matrix` walks Lines, then Transformers, then series Capacitors,
/// then series Reactors (r4133 `Common/Solution.pas` `AddLines2IncMatrix` →
/// `AddXfmr2IncMatrix` → `AddSeriesCap2IncMatrix` → `AddSeriesReac2IncMatrix`),
/// independently of the order the elements were declared in — the deck below
/// declares line, capacitor, reactor, transformer. A shunt capacitor
/// (`NumTerminals = 1`) contributes nothing.
#[test]
fn the_flat_build_orders_rows_lines_xfmrs_series_caps_series_reactors() {
    let mut dss = build(&[
        "new circuit.four basekv=12.47 bus1=sourcebus",
        "new line.l1 phases=3 bus1=sourcebus bus2=a length=1",
        "new capacitor.cser phases=3 bus1=b bus2=c kvar=600 kv=12.47",
        "new reactor.rser phases=3 bus1=a bus2=b R=0.1 X=1",
        "new transformer.t1 phases=3 windings=2 buses=[c, d] conns=[wye,wye] \
         kvs=[12.47,4.16] kvas=[5000,5000] xhl=6",
        "new capacitor.csh phases=3 bus1=d kvar=300 kv=4.16",
        "new load.ld phases=3 bus1=d kv=4.16 kw=500 pf=0.95",
        "set voltagebases=[12.47,4.16]",
        "calcvoltagebases",
    ]);
    let v = dss.inc_matrix_view();
    assert_eq!(
        v.rows,
        [
            "Line.l1",
            "Transformer.t1",
            "Capacitor.cser",
            "Reactor.rser"
        ],
        "build order, not declaration order; the shunt capacitor is not a row"
    );
    assert_eq!(v.cols, ["sourcebus", "a", "b", "c", "d"]);
    // `+1` at terminal 1's bus, `-1` at the other (Pascal `Upload2IncMatrix`).
    assert_eq!(
        v.inc_matrix,
        [
            [0, 0, 1],
            [0, 1, -1], // Line.l1      sourcebus -> a
            [1, 3, 1],
            [1, 4, -1], // Transformer.t1      c -> d
            [2, 2, 1],
            [2, 3, -1], // Capacitor.cser      b -> c
            [3, 1, 1],
            [3, 2, -1], // Reactor.rser        a -> b
        ]
    );
    assert_eq!(
        v.upstream_row_index,
        [0, 1, 2, 3],
        "no shunt reactor, no gap"
    );
    assert_eq!(v.new_errors, 0);
}

/// **Pin 2 — `IncMatrixCols` after a flat build is every bus, not `Inc_Mat_Cols`.**
///
/// `Calc_Inc_Matrix` populates only `IncMat`/`Inc_Mat_Rows` and clears
/// `IncMat_Ordered`, so both getters take their *unordered* branch and answer
/// `BusList` in index order (r4133 `DSolution.pas:627-631`, capi
/// `CAPI_Solution.pas:1014-1017`) — while `Inc_Mat_Cols` itself is still empty.
/// A getter that returned `Inc_Mat_Cols` unconditionally (which is what the
/// `Export IncMatrixCols` CSV writer does, faithfully to `ExportResults.pas`)
/// would answer **0** names where the oracles answer **16** on IEEE13.
#[test]
fn inc_matrix_cols_are_every_bus_after_a_flat_build() {
    let mut dss = compile_corpus_deck(
        "electricdss-tst/Version8/Distrib/IEEETestCases/13Bus/IEEE13Nodeckt.dss",
    );
    let v = dss.inc_matrix_view();
    assert_eq!(
        v.cols,
        [
            "sourcebus",
            "650",
            "rg60",
            "633",
            "634",
            "671",
            "645",
            "646",
            "692",
            "675",
            "611",
            "652",
            "670",
            "632",
            "680",
            "684"
        ],
        "`BusList` in index order"
    );
    assert_eq!(v.rows.len(), 17, "12 lines + 5 transformers");
    let st = &dss.circuit().expect("circuit").solution.inc_matrix;
    assert!(!st.ordered, "the flat build clears `IncMat_Ordered`");
    assert!(
        st.cols.is_empty(),
        "the flat build leaves `Inc_Mat_Cols` empty — reading it here would \
         answer 0 names instead of 16"
    );
}

/// **Pin 3 — after `CalcIncMatrix_O` the same getter answers `Inc_Mat_Cols`.**
///
/// The hierarchical build sets `IncMat_Ordered` and fills `Inc_Mat_Cols` in
/// topology-discovery order, which is a genuinely different sequence from
/// `BusList` (r4133 `DSolution.pas:616-623`, capi `CAPI_Solution.pas:991-1004`).
/// Both lists hold the same 16 IEEE13 buses; they differ from position 4 on.
#[test]
fn inc_matrix_cols_are_the_hierarchical_columns_after_calcincmatrix_o() {
    let mut dss = compile_corpus_deck(
        "electricdss-tst/Version8/Distrib/IEEETestCases/13Bus/IEEE13Nodeckt.dss",
    );
    let flat = dss.inc_matrix_view().cols;
    dss.command("CalcIncMatrix_O");
    let ckt = dss.circuit().expect("circuit");
    assert!(ckt.solution.inc_matrix.ordered);
    let ordered = view::inc_matrix_cols(ckt);
    assert_eq!(ordered, ckt.solution.inc_matrix.cols);
    assert_eq!(
        ordered,
        [
            "sourcebus",
            "650",
            "rg60",
            "632",
            "645",
            "646",
            "633",
            "634",
            "670",
            "671",
            "692",
            "675",
            "684",
            "652",
            "611",
            "680"
        ],
        "topology-discovery order"
    );
    assert_eq!(ordered.len(), flat.len(), "the same 16 buses");
    assert_ne!(ordered, flat, "in a different order");
}

/// **Pin 4 — a shunt reactor costs the ORACLES a matrix row; the port stays dense.**
///
/// `AddSeriesReac2IncMatrix` advances the row cursor for every reactor —
/// `inc(ActiveIncCell[0])` at r4133 `Common/Solution.pas:3039`, outside the
/// `if BusdotIdx = 0` guard at `:3015` (dss_capi 0.14.5 `Common/Solution.pas:1501`
/// identical) — so on this deck both oracles put `Reactor.r1` at matrix row **3**
/// while `Inc_Mat_Rows` holds only **3** names (valid indices 0..=2). The port
/// emits dense rows (CLAUDE.md: no upstream bug is reproduced in any lane) and
/// puts it at row **2**; `upstream_row_index` re-derives the oracle index so the
/// live gate can assert the difference positively instead of excluding the field
/// (GOLDEN_REBASE G1.8 settlement S-INC).
#[test]
fn a_shunt_reactor_costs_the_oracle_cursor_a_row() {
    let mut dss = reactor_gap_deck();
    let v = dss.inc_matrix_view();
    assert_eq!(v.rows, ["Line.l1", "Line.l2", "Reactor.r1"]);
    assert_eq!(
        v.inc_matrix,
        [
            [0, 0, 1],
            [0, 1, -1], // Line.l1   sourcebus -> a
            [1, 2, 1],
            [1, 3, -1], // Line.l2           b -> c
            [2, 1, 1],
            [2, 2, -1], // Reactor.r1        a -> b, at row 2
        ]
    );
    assert_eq!(
        v.upstream_row_index,
        [0, 1, 3],
        "the oracles' cursor skipped a row on `Reactor.rsh`"
    );
    // Both numbers of the divergence: port row 2 (in range), oracle row 3 (not).
    assert_eq!(v.rows.len(), 3);
    assert_eq!(v.upstream_row_index[2], 3);
    assert!(
        v.upstream_row_index[2] >= v.rows.len() as i32,
        "upstream's index does not index `Inc_Mat_Rows` — the defect this map \
         reconstructs"
    );
}

/// **Pin 5 — the same reconstruction on the corpus deck that exercises it hardest.**
///
/// `NEVMASTER.DSS` declares **94** reactors, 89 of them grounding (`bus2` = `….0`)
/// and 5 series, after 97 line/transformer rows. The port emits 102 dense rows;
/// upstream's cursor, advancing once per reactor, puts the five series reactors at
/// 97, 183, 185, 187 and **189** — 88 rows past the end of its own `Inc_Mat_Rows`.
/// This is one of the four corpus decks in the gate's `INC_UPSTREAM_ROW_DECLINES`
/// census.
#[test]
fn the_upstream_row_map_reconstructs_the_nev_test_case_cursor() {
    let mut dss = compile_corpus_deck(
        "electricdss-tst/Version8/Distrib/IEEETestCases/NEVTestCase/NEVMASTER.DSS",
    );
    let reactors = dss.circuit().expect("circuit").reactors.len();
    let v = dss.inc_matrix_view();
    assert_eq!(reactors, 94);
    assert_eq!(v.rows.len(), 102);
    assert_eq!(
        v.rows[97..],
        [
            "Reactor.neutral",
            "Reactor.jumper1",
            "Reactor.jumper2",
            "Reactor.jumper3",
            "Reactor.jumper4"
        ]
    );
    // Identity over the 97 line/transformer rows, then `base + j` per reactor.
    assert!(v.upstream_row_index[..97].iter().copied().eq(0..97));
    assert_eq!(v.upstream_row_index[97..], [97, 183, 185, 187, 189]);
    assert!(
        v.inc_matrix.iter().all(|t| t[0] < v.rows.len() as i32),
        "every port row indexes a row name"
    );
    assert_eq!(v.new_errors, 0);
}

/// **Pin 6 — the Laplacian is `IncMatᵀ · IncMat`, and it is blind to the row gaps.**
///
/// `CalcLaplacian` is literally `Laplacian := IncMat.Transpose().multiply(IncMat)`
/// (r4133 `Executive/ExecCommands.pas:911-917`, capi `:421-432`). Both of its
/// indices are **bus columns**, so a skipped row shifts nothing — which is why
/// the live gate compares the Laplacian arm without the
/// [`view::IncMatrixView::upstream_row_index`] remap. Checked as a value map
/// against an independent recomputation on a synthetic deck with a row gap and on
/// IEEE13 (34 incidence triples → 46 Laplacian triples).
#[test]
fn the_laplacian_is_the_incidence_gram_matrix() {
    let mut gap = reactor_gap_deck();
    let g = gap.inc_matrix_view();
    assert_eq!(as_map(&g.laplacian), gram(&g.inc_matrix));
    assert_eq!(g.laplacian.len(), 10);

    let mut ieee13 = compile_corpus_deck(
        "electricdss-tst/Version8/Distrib/IEEETestCases/13Bus/IEEE13Nodeckt.dss",
    );
    let v = ieee13.inc_matrix_view();
    assert_eq!(v.inc_matrix.len(), 34);
    assert_eq!(v.laplacian.len(), 46);
    assert_eq!(as_map(&v.laplacian), gram(&v.inc_matrix));
}

/// **Pin 7 — the empty-matrix path still reports every bus column.**
///
/// A circuit with no series branch at all builds an empty `IncMat` (not NIL):
/// rows, incidence and Laplacian are empty while `IncMatrixCols` still answers
/// every bus. The oracles answer their own sentinels for the three empty
/// quantities — capi `DefaultResult` (`CAPI_Solution.pas:887`, `:958`), r4133 the
/// one-element `[0]` / `'None'` (`DSolution.pas:545-546`, `:604-605`) — which the
/// capture normalizes to empty, so `cols` is the only per-step non-vacuity rail
/// left on such a step.
///
/// With no circuit at all the view answers [`view::IncMatrixView::default`] and
/// issues nothing: a command with no circuit would push "You must create a new
/// circuit object first".
#[test]
fn an_empty_incidence_matrix_still_reports_every_bus_column() {
    let mut bare = build(&[
        "new circuit.bare basekv=12.47 bus1=sourcebus",
        "new load.ld phases=3 bus1=sourcebus kv=12.47 kw=500 pf=0.95",
        "set voltagebases=[12.47]",
        "calcvoltagebases",
    ]);
    let v = bare.inc_matrix_view();
    assert!(v.rows.is_empty());
    assert!(v.inc_matrix.is_empty());
    assert!(v.laplacian.is_empty());
    assert!(v.upstream_row_index.is_empty());
    assert_eq!(v.cols, ["sourcebus"]);
    assert_eq!(v.new_errors, 0);
    assert!(
        bare.circuit()
            .expect("circuit")
            .solution
            .inc_matrix
            .inc_mat
            .is_some(),
        "empty, not NIL — the 8877 guard must not fire"
    );

    let mut fresh = Dss::new();
    assert_eq!(fresh.inc_matrix_view(), view::IncMatrixView::default());
    assert!(fresh.errors().is_empty(), "{:?}", fresh.errors());
}

/// **Pin 8 — the pair is issued in order, so error 8877 never fires; the guard is
/// live all the same.**
///
/// `CalcLaplacian` before any `CalcIncMatrix` raises 8877 in this port and in capi
/// (`Executive/ExecCommands.pas:423-427`); r4133 `:911-917` has no guard and would
/// dereference NIL. [`Dss::inc_matrix_view`] therefore issues `CalcIncMatrix`
/// first and reports the diagnostics the pair pushed **relative to the engine's
/// existing log**, so the live gate sees a swapped or dropped build in the same
/// step rather than through the runner's error-count check one step later.
#[test]
fn the_incidence_pair_pushes_no_diagnostics_and_the_guard_is_live() {
    let mut dss = build(&[
        "new circuit.guard basekv=12.47 bus1=sourcebus",
        "new line.l1 phases=3 bus1=sourcebus bus2=a length=1",
        "new load.ld phases=3 bus1=a kv=12.47 kw=500 pf=0.95",
        "set voltagebases=[12.47]",
        "calcvoltagebases",
    ]);
    // The guard fires when the order is wrong…
    dss.command("CalcLaplacian");
    assert_eq!(
        dss.errors()
            .iter()
            .filter_map(|e| e.code)
            .collect::<Vec<_>>(),
        [8877]
    );
    // …and the view's count is relative, so that pre-existing error is not
    // charged to the pair.
    let v = dss.inc_matrix_view();
    assert_eq!(v.new_errors, 0);
    assert_eq!(dss.errors().len(), 1, "the pair pushed nothing");
    assert_eq!(v.rows, ["Line.l1"]);
}

/// **Pin 9 — Q2, reproduced: a 1-terminal reactor becomes a phantom branch to the
/// last bus.**
///
/// `AddSeriesReac2IncMatrix` classifies by `ansipos('.0', GetBus(2))`
/// (r4133 `Common/Solution.pas:3013-3015`); on a 1-terminal reactor `GetBus(2)` is
/// `''`, `ansipos` is 0, and the reactor is treated as series. The bus search then
/// finds nothing and the cursor runs off the end, so the `-1` lands on column
/// `NumBuses - 1` (`:3030-3035`). On `reactor_asym.dss` the delta shunt
/// `Reactor.rdel` (bus `b3`, column 3) therefore draws an edge to `b4`, the **last**
/// bus in `BusList` — and it does move the Laplacian. Both oracles emit that same
/// edge at row **5** (`inc_matrix_pins.rs`'s measured oracle array `ASYM_INC`),
/// because this deck also trips the row cursor settlement S-INC settles; the port
/// numbers it **4** in its dense rows, so only the column pair is
/// channel-independent. This is a *which element is a row* defect, registered for
/// the WP-G2 teardown rather than fixed in a comparison sub-step, so the port
/// reproduces it and the pin states the current behaviour. The teardown's exit
/// value is **no row at all** for a 1-terminal reactor — it is a shunt, not a
/// branch — which costs a comparator-side reconstruction of the phantom row and
/// of its `NumBuses - 1` fallback column, i.e. a settlement of its own.
#[test]
fn a_one_terminal_reactor_becomes_a_phantom_branch_to_the_last_bus() {
    let mut dss = compile_corpus_deck("asymmetric/reactor/reactor_asym.dss");
    let v = dss.inc_matrix_view();
    assert_eq!(
        v.rows,
        [
            "Line.j2",
            "Reactor.rz12",
            "Reactor.rmat",
            "Reactor.rrx",
            "Reactor.rdel",
            "Reactor.rp"
        ]
    );
    assert_eq!(v.cols, ["src", "b1", "b2", "b3", "b4"]);
    // `Reactor.rdel` is row 4: `+1` on b3 (its only bus), `-1` on b4 (the last).
    let rdel: Vec<[i32; 3]> = v.inc_matrix.iter().copied().filter(|t| t[0] == 4).collect();
    assert_eq!(rdel, [[4, 3, 1], [4, 4, -1]]);
    assert_eq!(v.cols[4], "b4", "the fallback column is `NumBuses - 1`");
    // The shunt `Reactor.rsh` (bus2 = `b3.2.0`) is the one skipped, so upstream's
    // cursor runs one ahead from `Reactor.rdel` on: 4 -> 5, 5 -> 6.
    assert_eq!(v.upstream_row_index, [0, 1, 2, 3, 5, 6]);
}

/// **Pin 10 — Q3, reproduced: the reactor walk has no `Enabled` test.**
///
/// `AddLines2IncMatrix` (`:2862`), `AddXfmr2IncMatrix` (`:2916`) and
/// `AddSeriesCap2IncMatrix` (`:2964`) all skip disabled elements;
/// `AddSeriesReac2IncMatrix` (r4133 `Common/Solution.pas:3011-3040`) tests nothing
/// but `bus2`. A disabled series reactor is therefore a matrix row while a
/// disabled line and a disabled series capacitor on the same buses are not — **3**
/// rows on this deck, where the reactor walk carrying its siblings' `Enabled`
/// test would give **2** (the teardown's exit value) and no walk having one would
/// give **5** (both lines, the capacitor and the reactor). Registered for the
/// WP-G2 teardown; both oracles agree, so the port reproduces it for now.
#[test]
fn a_disabled_series_reactor_is_still_a_row() {
    let mut dss = build(&[
        "new circuit.dis basekv=12.47 bus1=sourcebus",
        "new line.l1 phases=3 bus1=sourcebus bus2=a length=1",
        "new line.l2 phases=3 bus1=b bus2=c length=1",
        "new line.loff phases=3 bus1=a bus2=b length=1 enabled=no",
        "new capacitor.coff phases=3 bus1=a bus2=b kvar=600 kv=12.47 enabled=no",
        "new reactor.roff phases=3 bus1=a bus2=b R=0.1 X=1 enabled=no",
        "new load.ld phases=3 bus1=c kv=12.47 kw=500 pf=0.95",
        "set voltagebases=[12.47]",
        "calcvoltagebases",
    ]);
    let v = dss.inc_matrix_view();
    assert_eq!(
        v.rows,
        ["Line.l1", "Line.l2", "Reactor.roff"],
        "the disabled line and the disabled series capacitor are skipped; the \
         disabled reactor is not"
    );
    assert_eq!(v.cols, ["sourcebus", "a", "b", "c"]);
    let roff: Vec<[i32; 3]> = v.inc_matrix.iter().copied().filter(|t| t[0] == 2).collect();
    assert_eq!(
        roff,
        [[2, 1, 1], [2, 2, -1]],
        "a -> b, from a disabled element"
    );
    assert_eq!(v.upstream_row_index, [0, 1, 2]);
}
