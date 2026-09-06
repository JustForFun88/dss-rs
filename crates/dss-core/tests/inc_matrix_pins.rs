//! **GOLDEN_REBASE G1.8 — the expected-value pins behind the flat incidence
//! surface** (`Solution.IncMatrix` / `Laplacian` / `IncMatrixRows` /
//! `IncMatrixCols`), lane `lane-s`, coordinator decisions D3 / D4 / D7.
//!
//! The surface adds **zero** `tests/corpus/ledger.json` rows. Its one
//! port-vs-oracle divergence — both oracles advance the incidence row cursor for
//! every reactor, the port emits dense rows — is settled as settlement
//! **S-INC**: a *positive assertion* of upstream's cursor rule inside the live
//! comparator (`crates/dss-core/tests/harness/inc_matrix.rs`), the shape D15 and
//! D16 gave G1.7. CLAUDE.md's discipline applies unchanged — every divergence
//! from an oracle channel is pinned by an expected-value test naming BOTH
//! numbers — and the pins here are those tests.
//!
//! They live in their own binary rather than in the engine's `#[cfg(test)]` tree
//! because each one drives the **gate-side** code: `compare_inc_matrix` and
//! `assert_transport_shape` against captures built from the *measured* oracle
//! replies. The engine-side twins (`Dss::inc_matrix_view`, the row map, the
//! `IncMat_Ordered` branch, the reproduced Q2/Q3 defects) are pinned in
//! `dss_core::exec::tests::inc_matrix`.
//!
//! **Every oracle array below was measured on this tree, on both channels**, by
//! `tmp/g18/probe_f7.py` / `probe_f7b.py` / `probe_f7c.py`: the pinned
//! dss-python 0.15.7 over dss_capi 0.14.5 (`tools/golden/PIN.txt`) and the
//! vendored EPRI r4133 DLL through `crates/dss-epri`. After the transports'
//! normalizations — **N1**, drop the cell capi over-allocates
//! (`CAPI_Solution.pas:910` / `:873`); **N2**, decode r4133's one-cell `[0]` nil
//! sentinel (`DSolution.pas:544` / `:642`); **N3**, decode the empty-name
//! sentinel, `''` on capi (`CAPI_Solution.pas:961`) vs `'None'` on r4133
//! (`DSolution.pas:605`) — the two channels were **byte-identical on every array
//! in this file**, so each pin carries one literal and names a channel only
//! where they genuinely differ (the raw lengths).

mod harness;

use std::panic::AssertUnwindSafe;
use std::path::PathBuf;

use dss_core::exec::Dss;
use harness::inc_matrix::{IncMatrixCap, assert_transport_shape, compare_inc_matrix};

// ---------------------------------------------------------------------------
// fixtures
// ---------------------------------------------------------------------------

/// Compile a vendored corpus deck (never `.inputs/`, CLAUDE.md) exactly as the
/// corpus gate does (`corpus_gate/runner.rs::run_rust_capture`: `clear`, then
/// `compile`, then a `solve` per checkpoint).
///
/// Only decks with no `Show`/`Export`/`Plot`/`Save` verb are compiled here, so
/// nothing writes into the vendored corpus. That is why
/// `Test/CableParameters.dss` — G1.8's declared empty-`IncMat` manifest row — is
/// *cited* rather than compiled by
/// [`an_empty_incidence_matrix_reads_back_as_no_rows`]: its `:50` runs
/// `show lineconstants`.
fn compile_corpus_deck(rel: &str) -> Dss {
    let deck: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "corpus",
        rel,
    ]
    .iter()
    .collect();
    assert!(deck.is_file(), "vendored corpus deck missing: {deck:?}");
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!("compile \"{}\"", deck.display()));
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{rel}: {:?}", dss.errors());
    dss
}

/// A circuit built from literal commands — no file is read and none is written.
fn build(cmds: &[&str]) -> Dss {
    let mut dss = Dss::new();
    dss.command("clear");
    for c in cmds {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss
}

/// The micro deck the row-cursor pins run on, in both declaration orders.
///
/// Two lines that do NOT touch (`sourcebus→a`, `b→c`), one **shunt** reactor and
/// one **series** reactor `a→b`; `BusList` is `sourcebus, a, b, c`. With the
/// shunt reactor declared first the oracles' cursor consumes a row on it and the
/// series reactor lands one row past the end of `Inc_Mat_Rows`; declared the
/// other way round there is no gap — and the circuit is otherwise the same, same
/// buses, same edges, same Laplacian.
fn micro_deck(shunt_first: bool) -> Dss {
    let shunt = "new reactor.rsh phases=3 bus1=a kvar=300 kv=12.47";
    let series = "new reactor.r1 phases=3 bus1=a bus2=b R=1 X=5";
    let (first, second) = if shunt_first {
        (shunt, series)
    } else {
        (series, shunt)
    };
    build(&[
        "new circuit.rq basekv=12.47 bus1=sourcebus",
        "new line.l1 phases=3 bus1=sourcebus bus2=a length=1",
        "new line.l3 phases=3 bus1=b bus2=c length=1",
        first,
        second,
        "new load.ld1 phases=3 bus1=b kv=12.47 kw=500 pf=0.95",
        "set voltagebases=[12.47]",
        "calcvoltagebases",
        "solve",
    ])
}

fn names(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_string()).collect()
}

fn cap(inc: &[i32], lap: &[i32], rows: &[&str], cols: &[&str]) -> IncMatrixCap {
    IncMatrixCap {
        inc_matrix: inc.to_vec(),
        laplacian: lap.to_vec(),
        rows: names(rows),
        cols: names(cols),
    }
}

/// The port's `IncMatrix` as the getters flatten it — **without** settlement
/// S-INC's row remap, i.e. the dense answer the engine actually holds.
fn dense_flat(dss: &mut Dss) -> Vec<i32> {
    dss.inc_matrix_view()
        .inc_matrix
        .iter()
        .flatten()
        .copied()
        .collect()
}

/// Run `f` and return its panic message — the negative drives below require a
/// specific failure, not merely "something failed".
fn panic_message(f: impl FnOnce()) -> String {
    let payload = std::panic::catch_unwind(AssertUnwindSafe(f)).expect_err("the arm must panic");
    if let Some(m) = payload.downcast_ref::<&str>() {
        (*m).to_string()
    } else if let Some(m) = payload.downcast_ref::<String>() {
        m.clone()
    } else {
        "<non-string panic payload>".to_string()
    }
}

// ---------------------------------------------------------------------------
// the measured oracle replies (both channels, after N1/N2/N3)
// ---------------------------------------------------------------------------

/// `micro_deck(true)` — the shunt reactor is declared first, so the oracles put
/// `Reactor.r1` at row **3** while `Inc_Mat_Rows` holds **3** entries (indices
/// 0..2).
const MICRO_GAP_INC: [i32; 18] = [0, 0, 1, 0, 1, -1, 1, 2, 1, 1, 3, -1, 3, 1, 1, 3, 2, -1];
/// `micro_deck(false)` — no shunt reactor precedes the series one, so the same
/// edge sits at row **2**. Everything else is byte-identical to
/// [`MICRO_GAP_INC`].
const MICRO_DENSE_INC: [i32; 18] = [0, 0, 1, 0, 1, -1, 1, 2, 1, 1, 3, -1, 2, 1, 1, 2, 2, -1];
/// The Laplacian of **both** micro decks — one constant, because the two
/// declaration orders return bit-identical bytes in bit-identical order. That
/// measurement is what [`the_laplacian_is_blind_to_the_row_cursor`] pins and what
/// lets the gate compare this arm unmapped.
const MICRO_LAP: [i32; 30] = [
    0, 0, 1, 0, 1, -1, 1, 0, -1, 1, 1, 2, 1, 2, -1, 2, 1, -1, 2, 2, 2, 2, 3, -1, 3, 2, -1, 3, 3, 1,
];
const MICRO_ROWS: [&str; 3] = ["Line.l1", "Line.l3", "Reactor.r1"];
const MICRO_COLS: [&str; 4] = ["sourcebus", "a", "b", "c"];

/// `asymmetric:reactor/reactor_asym.dss`, step 0 — one of the four corpus decks
/// behind `harness::inc_matrix::INC_UPSTREAM_ROW_DECLINES` and one of G1.8's six
/// `INC_MATRIX_DECLARED_IN_MANIFEST` rows. `Reactor.rsh` (shunt) is skipped
/// between two emitted rows, so `Reactor.rp` lands at row **6** while
/// `rows.len()` is **6**. Row 5 is the Q2 phantom branch — `Reactor.rdel`, a
/// 1-terminal delta shunt misclassified as series, drawing `b3 → b4`, the last
/// bus in `BusList` — reproduced in both lanes and pinned engine-side by
/// `exec::tests::inc_matrix::a_one_terminal_reactor_becomes_a_phantom_branch_to_the_last_bus`.
const ASYM_INC: [i32; 36] = [
    0, 2, 1, 0, 3, -1, 1, 0, 1, 1, 1, -1, 2, 1, 1, 2, 2, -1, 3, 2, 1, 3, 3, -1, 5, 3, 1, 5, 4, -1,
    6, 3, 1, 6, 4, -1,
];
const ASYM_LAP: [i32; 39] = [
    0, 0, 1, 0, 1, -1, 1, 0, -1, 1, 1, 2, 1, 2, -1, 2, 1, -1, 2, 2, 3, 2, 3, -2, 3, 2, -2, 3, 3, 4,
    3, 4, -2, 4, 3, -2, 4, 4, 2,
];
const ASYM_ROWS: [&str; 6] = [
    "Line.j2",
    "Reactor.rz12",
    "Reactor.rmat",
    "Reactor.rrx",
    "Reactor.rdel",
    "Reactor.rp",
];
const ASYM_COLS: [&str; 5] = ["src", "b1", "b2", "b3", "b4"];

/// `IEEE13Nodeckt.dss`, step 0 — the oracles' `Solution.IncMatrix`, 34 triples,
/// measured identically on capi (after N1 drops its 103rd cell) and on r4133.
const IEEE13_INC: [i32; 102] = [
    0, 2, 1, 0, 13, -1, 1, 13, 1, 1, 12, -1, 2, 12, 1, 2, 5, -1, 3, 5, 1, 3, 14, -1, 4, 13, 1, 4,
    3, -1, 5, 13, 1, 5, 6, -1, 6, 6, 1, 6, 7, -1, 7, 8, 1, 7, 9, -1, 8, 5, 1, 8, 15, -1, 9, 15, 1,
    9, 10, -1, 10, 15, 1, 10, 11, -1, 11, 5, 1, 11, 8, -1, 12, 0, 1, 12, 1, -1, 13, 1, 1, 13, 2,
    -1, 14, 1, 1, 14, 2, -1, 15, 1, 1, 15, 2, -1, 16, 3, 1, 16, 4, -1,
];
/// The same step's `Solution.Laplacian`, 46 triples — likewise identical on both
/// channels after N1/N2.
const IEEE13_LAP: [i32; 138] = [
    0, 0, 1, 0, 1, -1, 1, 0, -1, 1, 1, 4, 1, 2, -3, 2, 1, -3, 2, 2, 4, 2, 13, -1, 3, 3, 2, 3, 4,
    -1, 3, 13, -1, 4, 3, -1, 4, 4, 1, 5, 5, 4, 5, 8, -1, 5, 12, -1, 5, 14, -1, 5, 15, -1, 6, 6, 2,
    6, 7, -1, 6, 13, -1, 7, 6, -1, 7, 7, 1, 8, 5, -1, 8, 8, 2, 8, 9, -1, 9, 8, -1, 9, 9, 1, 10, 10,
    1, 10, 15, -1, 11, 11, 1, 11, 15, -1, 12, 5, -1, 12, 12, 2, 12, 13, -1, 13, 2, -1, 13, 3, -1,
    13, 6, -1, 13, 12, -1, 13, 13, 4, 14, 5, -1, 14, 14, 1, 15, 5, -1, 15, 10, -1, 15, 11, -1, 15,
    15, 3,
];
/// `Solution.IncMatrixCols` after a **flat** `CalcIncMatrix`: every bus in
/// `BusList` order, equal to `Circuit.AllBusNames` (measured on both channels
/// here, and on 1733/1733 live capi steps in this sub-step's sweep).
const IEEE13_COLS_FLAT: [&str; 16] = [
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
    "684",
];
/// The same sixteen buses after `CalcIncMatrix_O`, in the hierarchical
/// `Inc_Mat_Cols` order — a genuinely different list, which is why the getter's
/// `IncMat_Ordered` branch cannot be skipped. **The G1.8 capture never issues
/// `CalcIncMatrix_O`** (it calls `GetTopology`, r4133
/// `Common/Solution.pas:3173`, which would memoize the `Branch_List` G1.7's
/// census is defined on); this list exists only to give the branch teeth.
/// Measured identically on both channels.
const IEEE13_COLS_ORDERED: [&str; 16] = [
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
    "680",
];
const IEEE13_ROWS: [&str; 17] = [
    "Line.650632",
    "Line.632670",
    "Line.670671",
    "Line.671680",
    "Line.632633",
    "Line.632645",
    "Line.645646",
    "Line.692675",
    "Line.671684",
    "Line.684611",
    "Line.684652",
    "Line.671692",
    "Transformer.sub",
    "Transformer.reg1",
    "Transformer.reg2",
    "Transformer.reg3",
    "Transformer.xfm1",
];

const IEEE13: &str = "electricdss-tst/Version8/Distrib/IEEETestCases/13Bus/IEEE13Nodeckt.dss";
const REACTOR_ASYM: &str = "asymmetric/reactor/reactor_asym.dss";

// ---------------------------------------------------------------------------
// settlement S-INC — the row cursor
// ---------------------------------------------------------------------------

/// **S-INC, both numbers: the port puts the series reactor at matrix row 2, both
/// oracles put it at row 3, and `Inc_Mat_Rows` has 3 entries.**
///
/// `AddSeriesReac2IncMatrix` advances the incidence row cursor for **every**
/// reactor — `inc(ActiveIncCell[0])` sits at r4133
/// `Version8/Source/Common/Solution.pas:3039`, *outside* the `if BusdotIdx = 0`
/// emit guard at `:3013-3015`; dss_capi 0.14.5 `Common/Solution.pas:1501` is
/// identical — while the walk's three siblings advance only on an emitted row
/// (`:2885` Lines, `:2938` Transformers, `:2986` series Capacitors). A series
/// reactor that follows a shunt one therefore carries a row index that does not
/// index `Inc_Mat_Rows`, contradicting the surface's own contract
/// (`Inc_Mat_Rows` = the PD-element name per incidence row, exported as
/// `B2N Incidence Matrix Row Names (PDElements)`). D4's chain — stated intent
/// plus three siblings against one outlier — makes it an upstream defect, so the
/// port emits dense rows (`solution::inc_matrix::add_series_reactors`) and the
/// gate re-derives upstream's numbering positively instead of excluding the
/// field.
///
/// The drive below is the whole settlement in one place: the measured oracle
/// array passes through the real `compare_inc_matrix`, and the port's own dense
/// array — fed to the same comparator as if an oracle had sent it — reds at the
/// exact integer that moved.
#[test]
fn the_incidence_row_cursor_skips_a_shunt_reactor() {
    let mut dss = micro_deck(true);

    // Both numbers, from the port and from the oracle.
    let view = dss.inc_matrix_view();
    assert_eq!(view.rows, MICRO_ROWS, "the port's row names");
    assert_eq!(view.rows.len(), 3);
    assert_eq!(
        view.inc_matrix[4][0], 2,
        "port: `Reactor.r1` is at matrix row 2, which indexes `rows`"
    );
    assert_eq!(
        MICRO_GAP_INC[12], 3,
        "oracle (capi and r4133, measured): the same edge is at matrix row 3, \
         one past the last of the 3 row names"
    );
    assert_eq!(
        view.upstream_row_index,
        [0, 1, 3],
        "the map settlement S-INC re-derives"
    );

    // The remap is load-bearing: without it the two arrays are NOT equal.
    assert_ne!(
        dense_flat(&mut dss),
        MICRO_GAP_INC.to_vec(),
        "if these were equal the remap would be untested"
    );

    // …and with it the real comparator passes on the measured reply, under both
    // channel tags (the two transports send byte-identical arrays here).
    for channel in ["capi_v0145", "r4133"] {
        compare_inc_matrix(
            &mut dss,
            &cap(&MICRO_GAP_INC, &MICRO_LAP, &MICRO_ROWS, &MICRO_COLS),
            channel,
            &format!("pin:micro_shunt_first {channel}"),
            "pin:micro_shunt_first",
            0,
        );
    }

    // Teeth: an oracle that reported the port's DENSE numbering instead would be
    // a real divergence, and reds naming the integer that moved.
    let msg = panic_message(|| {
        compare_inc_matrix(
            &mut micro_deck(true),
            &cap(&MICRO_DENSE_INC, &MICRO_LAP, &MICRO_ROWS, &MICRO_COLS),
            "capi_v0145",
            "pin:micro_shunt_first teeth",
            "pin:micro_shunt_first",
            0,
        );
    });
    assert!(msg.contains("`inc_matrix` differs at int 12"), "{msg}");
    assert!(msg.contains("Rust 3 vs oracle 2"), "{msg}");
}

/// The same settlement on the corpus deck that carries it into the live gate:
/// `asymmetric:reactor/reactor_asym.dss`, one of the four decks of
/// `harness::inc_matrix::INC_UPSTREAM_ROW_DECLINES = (4, 5)`.
///
/// Both numbers: `Reactor.rp` is the port's row **5** and the oracles' row
/// **6**, with **6** row names — measured on capi and on r4133 alike.
#[test]
fn the_row_cursor_settlement_holds_on_the_corpus_witness() {
    let mut dss = compile_corpus_deck(REACTOR_ASYM);
    let view = dss.inc_matrix_view();
    assert_eq!(view.rows, ASYM_ROWS);
    assert_eq!(view.rows.len(), 6);
    assert_eq!(view.inc_matrix[10][0], 5, "port: `Reactor.rp` at row 5");
    assert_eq!(ASYM_INC[30], 6, "oracle: the same edge at row 6");
    assert_eq!(view.upstream_row_index, [0, 1, 2, 3, 5, 6]);

    assert_ne!(dense_flat(&mut dss), ASYM_INC.to_vec());
    for channel in ["capi_v0145", "r4133"] {
        compare_inc_matrix(
            &mut dss,
            &cap(&ASYM_INC, &ASYM_LAP, &ASYM_ROWS, &ASYM_COLS),
            channel,
            &format!("pin:{REACTOR_ASYM} {channel}"),
            REACTOR_ASYM,
            0,
        );
    }
}

/// **The Laplacian is bit-identical under a row gap — which is why the gate
/// compares it UNMAPPED.**
///
/// `Laplacian = IncMatᵀ · IncMat` (r4133
/// `Version8/Source/Executive/ExecCommands.pas:911-917`), so both of its indices
/// are **bus** columns and an empty matrix row contributes nothing to the
/// product. Measured on the oracle: the same circuit declared shunt-reactor-first
/// and series-reactor-first returns the SAME 30 integers in the SAME order, while
/// `IncMatrix` moves its last two triples from row 3 to row 2.
#[test]
fn the_laplacian_is_blind_to_the_row_cursor() {
    // The oracle's two answers: the incidence arrays differ …
    assert_ne!(MICRO_GAP_INC, MICRO_DENSE_INC);
    assert_eq!((MICRO_GAP_INC[12], MICRO_DENSE_INC[12]), (3, 2));
    // … while there is exactly ONE Laplacian constant for both decks, i.e. the
    // measured bytes were equal.

    let mut gap = micro_deck(true);
    let mut dense = micro_deck(false);
    assert_eq!(
        gap.inc_matrix_view().laplacian,
        dense.inc_matrix_view().laplacian,
        "the port's Laplacian must be order-blind too"
    );

    for (label, dss, inc) in [
        ("shunt_first", &mut gap, MICRO_GAP_INC),
        ("series_first", &mut dense, MICRO_DENSE_INC),
    ] {
        compare_inc_matrix(
            dss,
            &cap(&inc, &MICRO_LAP, &MICRO_ROWS, &MICRO_COLS),
            "capi_v0145",
            &format!("pin:laplacian_invariance {label}"),
            "pin:laplacian_invariance",
            0,
        );
    }

    // Teeth: the Laplacian arm is a real compare, not a formality.
    let mut perturbed = MICRO_LAP;
    perturbed[29] = 7;
    let msg = panic_message(|| {
        compare_inc_matrix(
            &mut micro_deck(false),
            &cap(&MICRO_DENSE_INC, &perturbed, &MICRO_ROWS, &MICRO_COLS),
            "capi_v0145",
            "pin:laplacian_invariance teeth",
            "pin:laplacian_invariance",
            0,
        );
    });
    assert!(msg.contains("`laplacian` differs at int 29"), "{msg}");
    assert!(msg.contains("Rust 1 vs oracle 7"), "{msg}");
}

// ---------------------------------------------------------------------------
// the `IncMatrixCols` branch
// ---------------------------------------------------------------------------

/// **After a flat `CalcIncMatrix` both oracles answer `IncMatrixCols` with every
/// bus in `BusList` order — not `Inc_Mat_Cols`.**
///
/// The getter branches on `IncMat_Ordered` (r4133 `DDLL/DSolution.pas:616`, the
/// unordered arm `:627-631`; capi `CAPI/CAPI_Solution.pas:991`, arm
/// `:1008-1018`), which `Calc_Inc_Matrix` clears and only `Calc_Inc_Matrix_Org`
/// sets. The `Export IncMatrixCols` CSV writer deliberately does NOT take that
/// branch — which is why the getter cannot stand in for the writer, and why
/// G3.2c keeps the twenty `org_*` goldens.
///
/// Both numbers, measured on IEEE13 on both channels: after the flat build the
/// sixteen names are `sourcebus, 650, rg60, 633, 634, 671, …`; after
/// `CalcIncMatrix_O` the same sixteen buses come back as
/// `sourcebus, 650, rg60, 632, 645, 646, …` — the same set, a different order
/// from position 3 on. A naive `cols` implementation returns `Inc_Mat_Cols`
/// unconditionally, i.e. an EMPTY list against 16 names, on every flat-built
/// case.
///
/// The incidence and Laplacian arrays driven here are the measured oracle ones
/// ([`IEEE13_INC`], [`IEEE13_LAP`]), not the port's own, so this pin is a
/// two-sided comparison of all four quantities on the deck.
#[test]
fn inc_matrix_cols_are_the_bus_list_when_unordered() {
    let mut sorted_flat = IEEE13_COLS_FLAT;
    sorted_flat.sort_unstable();
    let mut sorted_ordered = IEEE13_COLS_ORDERED;
    sorted_ordered.sort_unstable();
    assert_eq!(
        sorted_flat, sorted_ordered,
        "the same sixteen buses in both branches"
    );
    assert_ne!(
        IEEE13_COLS_FLAT, IEEE13_COLS_ORDERED,
        "…in a different order, so the branch is observable"
    );
    assert_eq!(
        (IEEE13_COLS_FLAT[3], IEEE13_COLS_ORDERED[3]),
        ("633", "632")
    );

    let mut dss = compile_corpus_deck(IEEE13);
    let view = dss.inc_matrix_view();
    assert_eq!(
        view.cols, IEEE13_COLS_FLAT,
        "the port takes the same branch"
    );
    assert_eq!(view.rows, IEEE13_ROWS);

    let oracle = cap(&IEEE13_INC, &IEEE13_LAP, &IEEE13_ROWS, &IEEE13_COLS_FLAT);
    assert_eq!(
        (oracle.inc_matrix.len(), oracle.laplacian.len()),
        (102, 138),
        "IEEE13 after N1/N2 — the lengths both channels send"
    );
    for channel in ["capi_v0145", "r4133"] {
        compare_inc_matrix(
            &mut dss,
            &oracle,
            channel,
            &format!("pin:{IEEE13} {channel}"),
            IEEE13,
            0,
        );
    }

    // Teeth: the hierarchical list is rejected, at the first name that moved.
    let ordered = IncMatrixCap {
        cols: names(&IEEE13_COLS_ORDERED),
        ..oracle.clone()
    };
    let msg = panic_message(|| {
        compare_inc_matrix(
            &mut compile_corpus_deck(IEEE13),
            &ordered,
            "capi_v0145",
            "pin:ieee13 cols teeth",
            IEEE13,
            0,
        );
    });
    assert!(msg.contains("`cols`[3] differs"), "{msg}");
    assert!(msg.contains("Rust `633` vs oracle `632`"), "{msg}");
}

// ---------------------------------------------------------------------------
// the two channel shapes (N1 / N2)
// ---------------------------------------------------------------------------

/// **capi allocates one integer too many for each of the two matrices and leaves
/// it zero; the transport drops it (rule N1) after asserting it is 0.**
///
/// `Solution_Get_IncMatrix` / `_Get_Laplacian` size their result `NZero * 3 + 1`
/// (`.inputs/dss_capi/src/CAPI/CAPI_Solution.pas:910` and `:873`, both carrying
/// the upstream `//TODO: remove the +1`) and never write the last cell. Measured
/// on IEEE13: `IncMatrix` raw **103** ints of which the last is **0** → **102**;
/// `Laplacian` raw **139**, last **0** → **138**. r4133 has no such cell
/// (`DDLL/DSolution.pas:545` sizes exactly `NZero * 3`).
///
/// A non-zero trailing cell, or a length that is not `3·NZero (+1)`, is G1.8's
/// **kill criterion**: measured 0 / 1733 live capi steps and 0 / 1756 live r4133
/// steps (`tmp/g18/capi_inc.json`, `tmp/g18/r4133_inc.json`).
///
/// The pin exists so a later "cleanup" cannot quietly drop N1. The raw payload
/// does not survive the transport, so the gate restates the rule as a fixpoint
/// and reds on anything that is not `3·NZero` — driven below.
#[test]
fn capi_incmatrix_carries_one_trailing_zero() {
    // The measured raw capi lengths, and the normalized ones.
    assert_eq!((103 - 1, 139 - 1), (102, 138));
    assert_eq!(103 % 3, 1, "3*NZero + 1 — never 3*NZero");
    assert_eq!(139 % 3, 1);
    assert_eq!(IEEE13_INC.len(), 102, "what the gate actually compares");
    assert_eq!(IEEE13_LAP.len(), 138);

    // A payload that still carries the cell fails, with the kill-criterion
    // message rather than as a divergence.
    let mut raw = IEEE13_INC.to_vec();
    raw.push(0);
    let un_normalized = IncMatrixCap {
        inc_matrix: raw,
        laplacian: IEEE13_LAP.to_vec(),
        rows: names(&IEEE13_ROWS),
        cols: names(&IEEE13_COLS_FLAT),
    };
    let msg = panic_message(|| assert_transport_shape(&un_normalized, "pin:n1"));
    assert!(msg.contains("length 103"), "{msg}");
    assert!(msg.contains("kill criterion"), "{msg}");

    // The empty state is where that cell IS the whole array on capi (`[0]`), and
    // where r4133 sends its own one-cell nil sentinel (`DSolution.pas:544` +
    // `:546`). Both decode to `[]`, so the gate sees a single shape.
    assert_transport_shape(&cap(&[], &[], &[], &["sourcebus"]), "pin:n1 empty");
}

/// **The two channels' RAW integer arrays differ by exactly one cell, and after
/// N1/N2 they are byte-identical.** Measured on four decks, both channels
/// (`tmp/g18/f7_capi.json`, `tmp/g18/f7_r4133.json`):
///
/// | deck | `IncMatrix` capi → r4133 | `Laplacian` capi → r4133 |
/// |---|---|---|
/// | `IEEE13Nodeckt.dss` | 103 → 102 | 139 → 138 |
/// | [`micro_deck`] (either order) | 19 → 18 | 31 → 30 |
/// | `asymmetric:reactor/reactor_asym.dss` | 37 → 36 | 40 → 39 |
/// | a bare source-and-load circuit | 1 → 1 | 1 → 1 |
///
/// The last row is the one place the difference vanishes, for two different
/// reasons: capi's array IS its unwritten `+1` cell, r4133's is the deliberate
/// one-cell `[0]` nil sentinel (`DSolution.pas:544-547`). Both decode to `[]`,
/// which is why the comparator can demand `len % 3 == 0` from both channels.
#[test]
fn capi_and_r4133_incmatrix_lengths_differ_by_one() {
    for (capi, r4133) in [
        (103, 102),
        (139, 138),
        (19, 18),
        (31, 30),
        (37, 36),
        (40, 39),
    ] {
        assert_eq!(capi - r4133, 1);
        assert_eq!(capi % 3, 1, "capi always sends 3*NZero + 1");
        assert_eq!(r4133 % 3, 0, "r4133 always sends 3*NZero");
    }

    // The normalized arrays are the same bytes on both channels, which is what
    // lets ONE literal per deck serve both throughout this file. The port lands
    // on the same lengths — the values themselves are compared above.
    let mut dss = micro_deck(true);
    let view = dss.inc_matrix_view();
    assert_eq!(
        (view.inc_matrix.len() * 3, view.laplacian.len() * 3),
        (18, 30)
    );
    assert_eq!((MICRO_GAP_INC.len(), MICRO_LAP.len()), (18, 30));
    assert_eq!((ASYM_INC.len(), ASYM_LAP.len()), (36, 39));
    assert_eq!((IEEE13_INC.len(), IEEE13_LAP.len()), (102, 138));
}

// ---------------------------------------------------------------------------
// the empty-matrix sentinel (N3)
// ---------------------------------------------------------------------------

/// **An empty incidence matrix reads back as a one-element NAME sentinel — `['']`
/// on capi, `['None']` on r4133 — and both transports decode it to `[]`.**
///
/// `Solution_Get_IncMatrixRows` exits through `DefaultResult(…, '')` when
/// `Inc_Mat_Rows` is nil (capi `CAPI_Solution.pas:961` via
/// `CAPI_Utils.pas:234-243`); r4133 writes the word `None` instead
/// (`DDLL/DSolution.pas:605`). Measured on a bare source-and-load circuit and,
/// byte for byte identically, on `Test/CableParameters.dss` — G1.8's declared
/// empty-`IncMat` manifest row, cited rather than compiled here because its
/// `:50` runs `show lineconstants` and would write into the vendored corpus:
///
/// * capi `IncMatrixRows = ['']`, r4133 `= ['None']`, port `= []`;
/// * both integer arrays `[0]` on both channels → `[]`;
/// * `IncMatrixCols = ['sourcebus']` on both channels and in the port — the
///   column list is NOT empty here, which is what makes
///   `capture_guard::require_capture` a real per-step rail on the 104 live steps
///   that take this path.
#[test]
fn an_empty_incidence_matrix_reads_back_as_no_rows() {
    let mut bare = build(&[
        "new circuit.bare basekv=12.47 bus1=sourcebus",
        "new load.ld phases=3 bus1=sourcebus kv=12.47 kw=500 pf=0.95",
        "set voltagebases=[12.47]",
        "calcvoltagebases",
        "solve",
    ]);
    let view = bare.inc_matrix_view();
    assert!(
        view.rows.is_empty(),
        "the port answers no rows, not a token"
    );
    assert!(view.inc_matrix.is_empty() && view.laplacian.is_empty());
    assert_eq!(view.cols, ["sourcebus"]);

    let decoded = cap(&[], &[], &[], &["sourcebus"]);
    for channel in ["capi_v0145", "r4133"] {
        compare_inc_matrix(
            &mut bare,
            &decoded,
            channel,
            &format!("pin:bare {channel}"),
            "pin:bare",
            0,
        );
    }

    // Teeth: an UN-decoded sentinel — either spelling — is refused, and the
    // message diagnoses N3 rather than "a name was lost".
    for token in ["", "None"] {
        let raw = IncMatrixCap {
            rows: names(&[token]),
            ..decoded.clone()
        };
        let msg = panic_message(|| assert_transport_shape(&raw, "pin:sentinel"));
        assert!(msg.contains("UN-normalized"), "{token:?}: {msg}");
    }

    // …while a bus legitimately NAMED `None` beside a NON-empty matrix is not
    // this rule's business: the sentinel decode is scoped to the empty state.
    assert_transport_shape(
        &cap(&[0, 0, 1], &[0, 0, 1], &["Line.l"], &["None"]),
        "pin:sentinel scope",
    );
}

// ---------------------------------------------------------------------------
// the pair's order and the 8877 guard
// ---------------------------------------------------------------------------

/// **`CalcLaplacian` before any `CalcIncMatrix` raises error 8877 — in the port
/// and in capi, with the same message text (upstream's `Indidence` typo
/// included) — while r4133 has no guard at all.**
///
/// capi guards the command
/// (`.inputs/dss_capi/src/Executive/ExecCommands.pas:421-433`) and so does the
/// port (`exec::command::do_calc_laplacian`); r4133
/// `Version8/Source/Executive/ExecCommands.pas:911-917` is a bare
/// `Laplacian := IncMat.Transpose()` / `.multiply(IncMat)` with no
/// `Assigned(IncMat)` test, so issuing the pair out of order there dereferences
/// NIL inside the DLL and kills the worker. That asymmetry is exactly why the
/// **order of the pair is a contract**, asserted from the sources by
/// `capture_order.rs::the_incidence_capture_issues_calcincmatrix_then_calclaplacian`
/// rather than driven live on r4133.
///
/// Both numbers, measured: the port pushes exactly one diagnostic, code **8877**,
/// text `Indidence matrix is not present. Please run either "CalcIncMatrix" or
/// "CalcIncMatrix_O" first.`; the pinned dss-python raises `DSSException` with
/// that identical string prefixed `(#8877)` (`tmp/g18/probe_f7b.py`).
///
/// The gate-side half: `Dss::inc_matrix_view` counts diagnostics **relative** to
/// the engine's existing log, so a pre-existing 8877 is not charged to the pair
/// and `compare_inc_matrix`'s `new_errors == 0` arm stays a signal about the
/// pair's own order rather than a tripwire on deck history.
#[test]
fn calclaplacian_without_calcincmatrix_raises_8877() {
    const MSG: &str = "Indidence matrix is not present. Please run either \"CalcIncMatrix\" \
                       or \"CalcIncMatrix_O\" first.";
    let mut dss = micro_deck(false);
    dss.command("CalcLaplacian");
    let raised: Vec<_> = dss.errors().iter().filter_map(|e| e.code).collect();
    assert_eq!(raised, [8877], "the port's guard");
    assert!(
        dss.errors()[0].message.contains(MSG),
        "the port's text must be capi's, verbatim: {:?}",
        dss.errors()[0].message
    );

    // The pair, issued in order by the view, pushes nothing…
    let view = dss.inc_matrix_view();
    assert_eq!(view.new_errors, 0);
    assert_eq!(
        dss.errors().len(),
        1,
        "the pre-existing 8877 is still there"
    );

    // …so the live comparator still passes on the measured reply: the arm
    // reports the PAIR's diagnostics, not the case's log.
    compare_inc_matrix(
        &mut dss,
        &cap(&MICRO_DENSE_INC, &MICRO_LAP, &MICRO_ROWS, &MICRO_COLS),
        "capi_v0145",
        "pin:8877",
        "pin:8877",
        0,
    );
}
