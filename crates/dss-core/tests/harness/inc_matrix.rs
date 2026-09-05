//! `GOLDEN_REBASE_PLAN.md` WP-G1 sub-step **G1.8** — the flat branch-to-node
//! incidence surface, compared live on every gating channel of the unified
//! corpus gate.
//!
//! # The surface
//!
//! Four quantities, read after one `CalcIncMatrix` + `CalcLaplacian` pair:
//! `Solution.IncMatrix`, `Solution.Laplacian`, `Solution.IncMatrixRows` and
//! `Solution.IncMatrixCols` — the four `Solution` columns the fastdss harness
//! adds for this surface (`.inputs/DSS-Python`
//! `origin/fastdss:tests/save_outputs.py:187`, fed by the `CalcIncMatrix` /
//! `CalcLaplacian` pair it issues at `:117-118`).
//!
//! | quantity | r4133 `DDLL/DSolution.pas` | capi `CAPI/CAPI_Solution.pas` |
//! |---|---|---|
//! | `IncMatrix` | `SolutionV` mode 1, `:542-568` | `:897-925` |
//! | `Laplacian` | `SolutionV` mode 5, `:640-667` | `:860-888` |
//! | `IncMatrixRows` | `SolutionV` mode 3, `:589-608` | `:953-970` |
//! | `IncMatrixCols` | `SolutionV` mode 4, `:609-639` | `:979-1020` |
//!
//! Two members of the same Pascal family are deliberately absent, and the
//! absence is asserted from both transports' source text by
//! `crates/dss-core/tests/capture_order.rs` rather than merely stated here:
//!
//! * **`Solution.BusLevels`** (`SolutionV` mode 2, `DSolution.pas:569-588`)
//!   sizes its buffer `length(Inc_Mat_Levels) - 1` (`:578`) and then writes
//!   `0..ArrSize` **inclusive** (`:581`) — one element past the end. The r4133
//!   bridge refuses the mode before the call
//!   (`crates/dss-epri/src/modes.rs`'s `DO_NOT_CALL` register), and a quantity
//!   only one channel can answer is not a gate.
//! * **The hierarchical `CalcIncMatrix_O` build**, because it runs `GetTopology`
//!   (r4133 `Common/Solution.pas:3173`) — which builds and memoizes the very
//!   `Branch_List` G1.7's two decline censuses are defined on.
//!
//! Both keep their byte-exact `tests/golden/inc_matrix/` `org_*` coverage
//! (`GOLDEN_REBASE_PLAN.md` §G3.2c, re-scoped in the same commit).
//!
//! # Fully discrete — no tolerance, deliberately
//!
//! Every field is an integer triple stream or an identifier list; nothing here
//! is a float, so this surface introduces **no** band and gets no
//! `tests/TOLERANCE_NOTES.md` floor. The absence is recorded there in one
//! sentence so a later reader does not read it as an omission.
//!
//! # Shape: three transport normalizations, only *asserted* here
//!
//! Both transports normalize before the payload reaches this module
//! (`tools/oracle/oracle_server.py::_inc_ints` / `::_inc_names`,
//! `crates/dss-epri/src/capture.rs::inc_ints` / `::inc_names`), and exactly
//! three shapes are normalized:
//!
//! * **N1, the capi `+1` cell.** capi allocates `ArrSize + 1` integers for both
//!   arrays and leaves the last one unwritten (`CAPI_Solution.pas:910` and
//!   `:873`, both carrying upstream's own `//TODO: remove the +1`), so a capi
//!   reply is always `3·NZero + 1` long. The transport asserts the trailing cell
//!   is `0` — this sub-step's kill criterion — and drops it.
//! * **N2, the r4133 nil/empty sentinel.** `SetLength(myIntArray, 1)` runs
//!   unconditionally and a nil/empty matrix answers the single cell `[0]`
//!   (`DSolution.pas:544-547`, `:642-646`) — decoded to `[]`.
//! * **N3, the empty-name sentinel.** An absent name list is one entry: `''` on
//!   capi (`DefaultResult`, `CAPI_Solution.pas:961`) and `'None'` on r4133
//!   (`DSolution.pas:605`, `:636`) — decoded to `[]` only where the engine can
//!   reach it.
//!
//! After N1–N3 the two channels are byte-identical: 358 both-gated cases, 0
//! disagreements on all four quantities (`tmp/g18/` sweeps, spec §3.1).
//!
//! This module does **not** re-normalize. A repair here would hide a transport
//! regression behind a green gate (§1.1(f): a comparator must fail, never
//! quietly fix up), so [`assert_transport_shape`] requires the arriving capture
//! to be at the transports' fixpoint already and names the transport to fix — the
//! `harness::topology::assert_normalized` precedent.
//!
//! # The port's rows are dense; upstream's cursor is not (settlement S-INC)
//!
//! Both oracles advance the incidence row cursor for **every** reactor:
//! `inc(ActiveIncCell[0])` sits at r4133 `Common/Solution.pas:3039`, *outside*
//! the `if BusdotIdx = 0` guard at `:3015` that decides whether a row was
//! emitted (dss_capi 0.14.5 `Common/Solution.pas:1501` is identical in effect),
//! while the walk's three siblings advance only on an emitted row (`:2885`
//! Lines, `:2938` Transformers, `:2986` series Capacitors). A series reactor
//! that follows a shunt one therefore carries a matrix row index that does not
//! index `Inc_Mat_Rows` — contradicting the surface's own contract, since
//! `IncMatrixRows` *is* "the PD-element name per incidence-matrix row"
//! (exported as `B2N Incidence Matrix Row Names (PDElements)`).
//!
//! CLAUDE.md is binding: no upstream bug is reproduced in any lane. The port
//! emits dense rows (`solution::inc_matrix::add_series_reactors`) and this
//! comparator re-derives upstream's index **positively** — it applies
//! `IncMatrixView::upstream_row_index` to the port's own triples and requires
//! the oracle to equal the result, so a red still means "upstream changed or the
//! port broke", never "we stopped looking". 0 `tests/corpus/ledger.json` rows.
//! The declining population is re-derived on every gate run and pinned in both
//! directions by [`INC_UPSTREAM_ROW_DECLINES`].
//!
//! The Laplacian is compared **unmapped**: `IncMatᵀ·IncMat` indexes buses on both
//! axes, so it is blind to a gap in the row cursor (measured on the oracle — the
//! same circuit declared shunt-reactor-first vs series-reactor-first yields
//! identical `Laplacian` bytes while the `IncMatrix` row index shifts).
//!
//! # Read and compared strictly last in the step
//!
//! Both transports capture this surface after everything else, including G1.7's
//! topology read, and `corpus_gate/runner.rs` calls [`compare_inc_matrix`] last
//! in a step for the same two reasons:
//!
//! * the pair is not `ActiveCktElement`-neutral on the r4133 channel —
//!   `AddSeriesReac2IncMatrix` re-points `LastClassReferenced` / `ActiveDSSClass`
//!   and then calls `ActiveDSSClass.First` (`Common/Solution.pas:3007-3010`), so
//!   building the matrix is a state write, not a read (the pinned capi walks the
//!   same reactors with a typed class iterator and leaves the active element
//!   alone, `.inputs/dss_capi/src/Common/Solution.pas:1464-1473` — the stronger
//!   channel sets the rule for both);
//! * it must FOLLOW the topology read, which is the one that builds and memoizes
//!   `Branch_List`; G1.7's `TOPOLOGY_STALE_DECLINES` /
//!   `LOOPED_PAIR_WINDOW_DECLINES` are defined on a tree nothing else has
//!   touched.

use std::collections::BTreeSet;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrd};

use dss_core::exec::Dss;
use serde::Deserialize;

use super::capture_guard;

/// How many triples/entries a mismatch message previews before eliding — the
/// integer arrays run to 90 918 ints on `ckt24` and a failure must stay
/// readable.
const PREVIEW: usize = 8;

/// The four flat incidence quantities of one checkpoint, in the shape **both**
/// transports emit (`tools/oracle/oracle_server.py::capture_inc_matrix`,
/// `crates/dss-epri/src/capture.rs::capture_inc_matrix` — identical JSON keys,
/// identical normalized shapes).
///
/// [`Self::inc_matrix`] and [`Self::laplacian`] are **flat**: `[row, col, value]`
/// per stored non-zero in the sparse container's insertion order, `3·NZero`
/// long, because both getters copy `IncMat.data[k][0..2]` cell by cell (r4133
/// `DSolution.pas:551-564`, `:649-662`; capi `CAPI_Solution.pas:908-921`,
/// `:871-884`). The capi `+1` cell (N1) and the r4133 `[0]` sentinel (N2) are
/// already gone.
#[derive(Debug, Clone, Deserialize)]
pub struct IncMatrixCap {
    /// `Solution.IncMatrix`, flat triples.
    pub inc_matrix: Vec<i32>,
    /// `Solution.Laplacian`, flat triples.
    pub laplacian: Vec<i32>,
    /// `Solution.IncMatrixRows` — `Inc_Mat_Rows`, one PD-element name per row.
    pub rows: Vec<String>,
    /// `Solution.IncMatrixCols` — after a FLAT build `IncMat_Ordered` is false
    /// (r4133 `Common/Solution.pas:3066`), so both getters answer every bus in
    /// `BusList` order rather than `Inc_Mat_Cols` (r4133 `DSolution.pas:616` +
    /// `:627-631`; capi `CAPI_Solution.pas:991` + `:1008-1018`). Measured equal
    /// to `AllBusNames` on 1 645 / 1 645 live capi steps and 1 756 / 1 756 live
    /// r4133 steps.
    pub cols: Vec<String>,
}

// ---------------------------------------------------------------------------
// Arms 1-2 — what is provable about the ORACLE alone, before the port is
// consulted. A red here is a transport failure, never a Rust divergence.
// ---------------------------------------------------------------------------

/// The transports' normalization rules (N1–N3, module doc) as a **fixpoint
/// assertion** over one arriving capture.
///
/// * `3·NZero`: a capi reply that still carries its `+1` cell arrives
///   `len % 3 == 1`, which is exactly what a transport that stopped applying N1
///   would produce — and a length that is not `3·NZero (+1)` is this sub-step's
///   kill criterion, so it is re-checked at the gate and not only in the
///   capture.
/// * No empty or whitespace-only name: a row label always carries its `Class.`
///   prefix and `BusList.Get` never returns a blank, so an empty entry is a lost
///   name, not a shape.
/// * No un-decoded empty-name sentinel: a one-entry `['']` / `['None']` next to
///   an empty matrix is N3 having stopped firing. (A bus legitimately *named*
///   `None` next to a NON-empty matrix is not this rule's business and passes.)
#[track_caller]
pub fn assert_transport_shape(cap: &IncMatrixCap, ctx: &str) {
    for (what, v) in [
        ("inc_matrix", &cap.inc_matrix),
        ("laplacian", &cap.laplacian),
    ] {
        assert_eq!(
            v.len() % 3,
            0,
            "{ctx}: the oracle's `{what}` has length {} — not 3*NZero. Both \
             transports normalize before sending (capi drops the unwritten `+1` \
             cell it over-allocates, CAPI_Solution.pas:910 / :873; the r4133 \
             bridge decodes the one-cell `[0]` nil sentinel, DSolution.pas:544 / \
             :642), so this is a transport failure, not a divergence — and a \
             non-`3*NZero` length is G1.8's kill criterion",
            v.len()
        );
    }
    // The sentinel check runs BEFORE the empty-entry scan: an un-decoded capi
    // sentinel IS a one-element empty list, and "N3 stopped firing" is the
    // precise diagnosis for it, while "a name was lost" is the precise one for
    // every other empty entry.
    assert!(
        !(cap.inc_matrix.is_empty()
            && cap.rows.len() == 1
            && (cap.rows[0].eq_ignore_ascii_case("none") || cap.rows[0].is_empty())),
        "{ctx}: the oracle's `rows` reached the gate UN-normalized ({:?} next to \
         an EMPTY incidence matrix): that is the empty-name sentinel — `''` on \
         capi (DefaultResult, CAPI_Solution.pas:961), `'None'` on r4133 \
         (DSolution.pas:605) — which rule N3 decodes in the transports. The \
         comparator asserts their fixpoint instead of repeating the repair, so a \
         transport that stops decoding FAILS here rather than comparing a \
         phantom one-element list as if it were empty",
        cap.rows
    );
    for (what, names) in [("rows", &cap.rows), ("cols", &cap.cols)] {
        assert!(
            !names.iter().any(|s| s.trim().is_empty()),
            "{ctx}: the oracle's `{what}` carries an empty entry at index {}: \
             {}. A row label always carries its `Class.` prefix and \
             `BusList.Get` never returns a blank, so an empty entry is a LOST \
             name — the transports refuse it \
             (`oracle_server.py::_inc_names`, `dss-epri::capture::inc_names`) \
             and so does the gate",
            names
                .iter()
                .position(|s| s.trim().is_empty())
                .expect("just asserted one exists"),
            preview_names(names)
        );
    }
}

/// The oracle's own internal consistency — free teeth: neither channel can fail
/// it, so a red is a transport bug rather than a divergence (the
/// `harness::topology::compare_topology` arm-2 precedent).
///
/// `Inc_Mat_Rows` is grown one entry per **emitted** row
/// (`SetLength(Inc_Mat_Rows, temp_counter)`, r4133 `Common/Solution.pas:3018`)
/// and every emitted row uploads at least one non-zero (`Upload2IncMatrix`
/// writes `+1` at the first terminal), so a name list and a triple stream are
/// empty together and non-empty together. Same for the Laplacian:
/// `IncMatᵀ·IncMat` carries `sum(v²) > 0` on every occupied bus column's
/// diagonal. Measured on the whole live population — the empty case is exactly
/// 104 r4133 steps and 99 capi non-`large` steps, and the biconditional held in
/// both directions on all of them.
#[track_caller]
pub fn assert_oracle_self_consistent(cap: &IncMatrixCap, ctx: &str) {
    assert_eq!(
        cap.rows.is_empty(),
        cap.inc_matrix.is_empty(),
        "{ctx}: the oracle contradicts itself: `rows` holds {} name(s) while \
         `inc_matrix` holds {} triple(s). `Inc_Mat_Rows` grows one entry per \
         EMITTED row (r4133 Common/Solution.pas:3018) and every emitted row \
         uploads at least one non-zero, so the two are empty together.\n  rows:  \
         {}\n  inc_matrix: {}",
        cap.rows.len(),
        cap.inc_matrix.len() / 3,
        preview_names(&cap.rows),
        preview_ints(&cap.inc_matrix)
    );
    assert_eq!(
        cap.laplacian.is_empty(),
        cap.inc_matrix.is_empty(),
        "{ctx}: the oracle contradicts itself: `laplacian` holds {} triple(s) \
         while `inc_matrix` holds {}. The Laplacian is IncMat^T * IncMat, whose \
         diagonal carries sum(v^2) > 0 for every occupied bus column, so the two \
         are empty together (measured on every live step of both channels)",
        cap.laplacian.len() / 3,
        cap.inc_matrix.len() / 3
    );
}

// ---------------------------------------------------------------------------
// Settlement S-INC — the port's dense rows mapped onto upstream's cursor
// ---------------------------------------------------------------------------

/// The port's `IncMatrix` in **upstream's** row numbering, flat, ready to be
/// compared against the oracle's array element for element.
///
/// Only the row component is rewritten, through
/// `IncMatrixView::upstream_row_index` — the reconstruction the engine exposes
/// for exactly this (module doc, settlement S-INC; the
/// `TopologyView::looped_pair_candidates` precedent of decision D16). Columns
/// and values are the port's own.
///
/// The two asserts are the port's own consistency arms, the mirror of
/// [`assert_oracle_self_consistent`]: the map covers every row name, and every
/// stored triple's row indexes a row name — the port's DENSE-row contract, which
/// upstream's cursor breaks and the port does not.
///
/// Takes the three pieces of `IncMatrixView` rather than the struct: `exec::view`
/// is a private module (only `ElementSnapshot`, `MeterZoneView`, `MonitorView`
/// and `SystemYCsc` are re-exported from `dss_core::exec`), so the view type is
/// not nameable from a test binary — the same reason
/// `harness::topology::compare_topology` never names `TopologyView` either.
#[track_caller]
fn upstream_view_of(inc_matrix: &[[i32; 3]], map: &[i32], n_rows: usize, ctx: &str) -> Vec<i32> {
    assert_eq!(
        map.len(),
        n_rows,
        "{ctx}: the port contradicts itself: `upstream_row_index` has {} \
         entry(ies) for {n_rows} row name(s) — the map is built one per row \
         (`exec::view::upstream_row_index`)",
        map.len()
    );
    let mut out = Vec::with_capacity(inc_matrix.len() * 3);
    for (k, [row, col, val]) in inc_matrix.iter().copied().enumerate() {
        let idx = usize::try_from(row)
            .unwrap_or_else(|_| panic!("{ctx}: the port's `inc_matrix`[{k}] has a negative row"));
        let mapped = *map.get(idx).unwrap_or_else(|| {
            panic!(
                "{ctx}: the port contradicts itself: `inc_matrix`[{k}] sits at \
                 row {row} but only {n_rows} row name(s) exist. The port's rows \
                 are DENSE — that is the whole point of settlement S-INC \
                 (`solution::inc_matrix::add_series_reactors`); upstream's \
                 cursor is the one that runs past `Inc_Mat_Rows`"
            )
        });
        out.extend_from_slice(&[mapped, col, val]);
    }
    out
}

/// Whether upstream's cursor and the port's dense rows agree on this circuit —
/// i.e. whether the S-INC remap is the identity. False exactly where a shunt
/// reactor precedes a series one.
fn map_is_identity(map: &[i32]) -> bool {
    map.iter()
        .enumerate()
        .all(|(i, &r)| i32::try_from(i).is_ok_and(|i| i == r))
}

// ---------------------------------------------------------------------------
// Ordered, exact compares
// ---------------------------------------------------------------------------

/// First [`PREVIEW`] triples of a flat integer array, elided.
fn preview_ints(v: &[i32]) -> String {
    let shown = v.len().min(PREVIEW * 3);
    let head: Vec<String> = v[..shown]
        .chunks(3)
        .map(|t| match t {
            [r, c, x] => format!("[{r},{c},{x}]"),
            rest => format!("{rest:?}"),
        })
        .collect();
    if shown == v.len() {
        format!("{} triple(s) {}", v.len() / 3, head.join(" "))
    } else {
        format!(
            "{} triple(s) {} ... (+{} more)",
            v.len() / 3,
            head.join(" "),
            (v.len() - shown) / 3
        )
    }
}

/// First [`PREVIEW`] entries of a name list, elided.
fn preview_names(v: &[String]) -> String {
    if v.len() <= PREVIEW {
        format!("{v:?}")
    } else {
        format!("{:?} ... (+{} more)", &v[..PREVIEW], v.len() - PREVIEW)
    }
}

/// One `(port, oracle)` flat integer arm — exact, ordered, zero tolerance.
///
/// The message decodes the first differing element back to its triple and names
/// which of `row` / `col` / `value` moved, because a flat index into a 90 918-int
/// array is unreadable on its own.
#[track_caller]
fn compare_ints(what: &str, actual: &[i32], expected: &[i32], ctx: &str) {
    assert_eq!(
        actual.len(),
        expected.len(),
        "{ctx}: `{what}` length differs: Rust {} vs oracle {} int(s)\n  Rust:   \
         {}\n  oracle: {}",
        actual.len(),
        expected.len(),
        preview_ints(actual),
        preview_ints(expected)
    );
    if let Some(i) = actual.iter().zip(expected).position(|(a, e)| a != e) {
        let t = i - i % 3;
        let field = ["row", "col", "value"][i % 3];
        let end = (t + 3).min(actual.len());
        panic!(
            "{ctx}: `{what}` differs at int {i} (triple {}, the `{field}`): Rust \
             {} vs oracle {}\n  Rust triple:   {:?}\n  oracle triple: {:?}\n  \
             Rust:   {}\n  oracle: {}",
            t / 3,
            actual[i],
            expected[i],
            &actual[t..end],
            &expected[t..end],
            preview_ints(actual),
            preview_ints(expected)
        );
    }
}

/// One `(port, oracle)` ordered name-vector arm, compared **case-insensitively**
/// (the harness convention: DSS identifiers are case-insensitive, and the port
/// lowercases its `BusList` entries — `support::hashlist::HashList` — while
/// upstream keeps the spelling it first saw).
///
/// Order is load-bearing and is NOT relaxed to a set compare: `rows` is the flat
/// build order (Lines -> Transformers -> series Capacitors -> series Reactors)
/// and `cols` is `BusList` order — the walk order is precisely what this gate
/// exists to catch.
#[track_caller]
fn compare_names(what: &str, actual: &[String], expected: &[String], ctx: &str) {
    assert_eq!(
        actual.len(),
        expected.len(),
        "{ctx}: `{what}` length differs: Rust {} vs oracle {}\n  Rust:   {}\n  \
         oracle: {}",
        actual.len(),
        expected.len(),
        preview_names(actual),
        preview_names(expected)
    );
    for (i, (a, e)) in actual.iter().zip(expected).enumerate() {
        assert!(
            a.eq_ignore_ascii_case(e),
            "{ctx}: `{what}`[{i}] differs: Rust `{a}` vs oracle `{e}` (ordered \
             compare — `rows` is the flat build order Lines -> Transformers -> \
             series Capacitors -> series Reactors, `cols` is `BusList` order)"
        );
    }
}

// ---------------------------------------------------------------------------
// The decline census — re-derived on every gate run, pinned both ways
// ---------------------------------------------------------------------------

/// `(case label, step)` pairs where upstream's row cursor and the port's dense
/// rows part company (settlement S-INC). A `both` case that declines on both
/// channels is ONE entry — the pinned population counts cases and case-steps,
/// not channel visits.
static ROW_DECLINES: Mutex<BTreeSet<(String, usize)>> = Mutex::new(BTreeSet::new());
/// Channel visits behind [`ROW_DECLINES`] (the `both` cases counted twice).
static ROW_VISITS: AtomicUsize = AtomicUsize::new(0);
/// `(case, step, channel)` incidence comparisons this process ran at all — the
/// non-vacuity half: a census of `0 / 0` means nothing was compared, not that
/// nothing declined.
static COMPARED: AtomicUsize = AtomicUsize::new(0);

/// What the live gate measured in this process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeclineCensus {
    /// `(case, step, channel)` incidence comparisons run.
    pub compared: usize,
    /// S-INC: `(cases, case-steps)` whose row map is not the identity.
    pub rows: (usize, usize),
    /// S-INC channel visits (a `both` case-step counts twice).
    pub row_visits: usize,
}

fn lock<T>(m: &'static Mutex<T>) -> std::sync::MutexGuard<'static, T> {
    // A poisoned census is not a reason to fail a different case: whoever
    // panicked while holding it has already reported itself.
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn cases_and_steps(set: &BTreeSet<(String, usize)>) -> (usize, usize) {
    let cases: BTreeSet<&str> = set.iter().map(|(c, _)| c.as_str()).collect();
    (cases.len(), set.len())
}

/// The census as it stands, for the gate epilogue
/// ([`assert_declines_are_the_pinned_population`]).
pub fn decline_census() -> DeclineCensus {
    let rows = lock(&ROW_DECLINES);
    DeclineCensus {
        compared: COMPARED.load(AtomicOrd::Relaxed),
        rows: cases_and_steps(&rows),
        row_visits: ROW_VISITS.load(AtomicOrd::Relaxed),
    }
}

/// The declining population, per case, as a report the epilogue prints and a
/// mismatch quotes — so a moved constant is re-derived from the run itself and
/// never from a guess.
pub fn decline_report() -> String {
    let set = lock(&ROW_DECLINES);
    let mut per_case: std::collections::BTreeMap<&str, Vec<usize>> =
        std::collections::BTreeMap::new();
    for (case, step) in set.iter() {
        per_case.entry(case.as_str()).or_default().push(*step);
    }
    let mut out = format!(
        "S-INC row-cursor declines: {} case(s), {} case-step(s)",
        per_case.len(),
        set.len()
    );
    for (case, steps) in per_case {
        out.push_str(&format!("\n    {case}: {} step(s) {steps:?}", steps.len()));
    }
    out
}

fn record_row_decline(case: &str, step: usize) {
    ROW_VISITS.fetch_add(1, AtomicOrd::Relaxed);
    lock(&ROW_DECLINES).insert((case.to_string(), step));
}

/// **Settlement S-INC's declining population**, as `(cases, case-steps)`: where
/// upstream's row cursor runs past `Inc_Mat_Rows` and [`compare_inc_matrix`]
/// therefore compares its remapped answer instead of a bare one.
///
/// Measured 2026-09-05 over the forced incidence population (every live
/// non-`large` case), independently on both channels and identically:
/// `solvable_now:…/NEVTestCase/NEVMASTER.DSS` (step 0),
/// `solvable_now:…/NEVTestCase/Run_NEV.dss` (step 0, the same deck),
/// `asymmetric:reactor/reactor_asym.dss` (step 0) and
/// `asymmetric:reactor/midi_reactor_asym.dss` (steps 0 and 1) — 4 cases, 5
/// case-steps. Every one of them declares a **shunt** reactor (`bus2` carrying a
/// `.0` ground token) ahead of a series one; NEVMASTER decomposes exactly: 102
/// rows, 5 of them reactor rows, so `base = 97`, and its last series reactor
/// sits at reactor index 92 => upstream row 189.
pub const INC_UPSTREAM_ROW_DECLINES: (usize, usize) = (4, 5);

/// **The decline population is re-derived on every full gate run and pinned in
/// BOTH directions** — the fail-on-stale rule the 0-ledger-row settlement S-INC
/// hangs on (the D15/D16 shape G1.7 landed on this lane).
///
/// A population that GREW means a case-step started declining and nobody looked;
/// one that SHRANK means the pin behind it is a statement about nothing — in
/// particular, upstream fixing its cursor, or the port silently starting to
/// reproduce it, would show up here. Either way the gate must fail rather than
/// absorb it.
///
/// Silent in exactly two documented situations:
///
/// * `DSS_GATE_ONLY` is set — a filtered run holds a filtered population, so its
///   census cannot be the pinned one (the mandatory gate never sets it).
/// * No comparison ran at all, which while G1.8's surface flag is unwired is the
///   honest state of the world. Unlike G1.7's twin, this arming predicate is the
///   run's own comparison counter rather than a re-read of the manifests, for
///   two reasons: this module compiles into ~20 test binaries that have no
///   manifest access, and the counter is *tighter* — it observes the surface
///   actually being consumed rather than being declared. It gives up nothing,
///   because the three ways the surface could go quiet are each caught elsewhere
///   and loudly: a masked capture request fails
///   [`capture_guard::require_capture_opt`] at the first flagged case, a dropped
///   manifest declaration moves `tests/corpus/manifests/population.lock.json`,
///   and a changed force rule fails `corpus_gate::scheduler`'s forced-population
///   re-derivation.
pub fn assert_declines_are_the_pinned_population() {
    if std::env::var("DSS_GATE_ONLY").is_ok() {
        return;
    }
    check_census(decline_census(), &decline_report());
}

/// The rule itself, over **injected** values — split out for the reason
/// `capture_guard::missing_capture_msg` is: it is then provable without the
/// process-global census, so its own tests state the contract instead of
/// depending on whether some other test in the same binary happened to run a
/// comparison first.
#[track_caller]
fn check_census(census: DeclineCensus, report: &str) {
    if census.compared == 0 {
        assert_eq!(
            census.rows,
            (0, 0),
            "no (case, step, channel) triple reached `compare_inc_matrix`, yet \
             the S-INC census recorded declines: {census:?}"
        );
        eprintln!(
            "corpus_gate inc_matrix: the surface is not requested by any live \
             case (GOLDEN_REBASE G1.8 F6 wires the flag) — nothing to re-derive"
        );
        return;
    }
    eprintln!(
        "corpus_gate inc_matrix: {} compared (case, step, channel) triple(s); \
         S-INC row-cursor declines {:?} ({} channel visit(s))\n  {report}",
        census.compared, census.rows, census.row_visits,
    );
    assert_eq!(
        census.rows, INC_UPSTREAM_ROW_DECLINES,
        "the S-INC row-cursor decline population moved (measured {:?}, pinned \
         {:?}). It is re-derived on every run and fails in BOTH directions: a \
         bigger population means a case-step started declining with nobody \
         looking, a smaller one means the settlement now covers less than it \
         claims — and a shrink to zero would mean upstream stopped advancing its \
         cursor on shunt reactors (r4133 Common/Solution.pas:3039), which is the \
         defect the whole settlement is built on. Re-measure, move the constant \
         WITH the record, and re-read the pin \
         `inc_matrix_pins::the_incidence_row_cursor_skips_a_shunt_reactor`.\n{report}",
        census.rows, INC_UPSTREAM_ROW_DECLINES,
    );
}

// ---------------------------------------------------------------------------
// The comparator
// ---------------------------------------------------------------------------

/// Compare the port's `Dss::inc_matrix_view` against one channel's capture. Zero
/// tolerance throughout: two exact integer streams and two ordered,
/// case-insensitive identifier lists.
///
/// Arms, in order — the oracle-only ones first, so a transport failure is
/// reported as one instead of as a Rust divergence:
///
/// 1. **Presence.** `cols` is non-empty on every live case (it is the whole bus
///    list, measured `cols.len() == NumBuses >= 1` on every live step of both
///    channels), so [`capture_guard::require_capture`] is a real per-step
///    non-vacuity rail even on the ~104 steps whose incidence matrix is empty.
/// 2. **Transport fixpoint** ([`assert_transport_shape`]) and **the oracle's own
///    consistency** ([`assert_oracle_self_consistent`]).
/// 3. **The port builds.** `Dss::inc_matrix_view` issues `CalcIncMatrix` then
///    `CalcLaplacian` through the real dispatch; the pair must push **no**
///    diagnostic. The port guards `CalcLaplacian` with error 8877 when no
///    incidence matrix exists (`exec::command`'s `do_calc_laplacian`; capi
///    `Executive/ExecCommands.pas:421-433`, while r4133 `:911-917` has no guard
///    at all and would dereference NIL), so a swapped or dropped `CalcIncMatrix`
///    is caught HERE rather than one step later in the runner's own
///    `errors().len() == baseline` check, whose message would blame the solve.
/// 4. **`rows` and `cols`**, ordered and case-insensitive. `cols` doubles as the
///    bus-count contract: it is every bus of the port's `BusList`, because a flat
///    build leaves `IncMat_Ordered` false and both getters take that branch.
/// 5. **`IncMatrix`**: the port's triples in upstream's row numbering
///    ([`upstream_view_of`]) must equal the oracle's array, exactly and in order
///    — a positive assertion of upstream's cursor rule (settlement S-INC, module
///    doc), never an exclusion. On every deck but four the map is the identity
///    and this is literally `port == oracle`.
/// 6. **`Laplacian`**: exact, ordered, **unmapped** — it is blind to the row
///    cursor (module doc).
/// 7. **Census**: a case-step whose map is not the identity is recorded for
///    [`assert_declines_are_the_pinned_population`].
///
/// `case` and `step` are the census keys (the case label as the gate prints it
/// and the checkpoint index); `channel` is the gating channel's tag, for the
/// presence rail's message.
///
/// `&mut Dss` because the port, like upstream, **rebuilds engine state** here
/// (`IncMat`, `Laplacian`, `Inc_Mat_Rows`, `IncMat_Ordered`); the call must
/// therefore be the LAST comparison of a step — see the module doc.
pub fn compare_inc_matrix(
    dss: &mut Dss,
    cap: &IncMatrixCap,
    channel: &str,
    ctx: &str,
    case: &str,
    step: usize,
) {
    // --- 1. presence -------------------------------------------------------
    capture_guard::require_capture("compare_inc_matrix", channel, cap.cols.len(), ctx);

    // --- 2. the oracle alone ----------------------------------------------
    assert_transport_shape(cap, ctx);
    assert_oracle_self_consistent(cap, ctx);

    // --- 3. the port builds ------------------------------------------------
    // Nothing else in this checkpoint may run after this: the pair rewrites
    // solution state and, on r4133, moves the active element (module doc).
    let view = dss.inc_matrix_view();
    assert_eq!(
        view.new_errors, 0,
        "{ctx}: the `CalcIncMatrix` + `CalcLaplacian` pair pushed {} \
         diagnostic(s) on the Rust engine; the contract is 0. Error 8877 here \
         means `CalcLaplacian` ran without a preceding `CalcIncMatrix` \
         (`exec::command::do_calc_laplacian`) — i.e. the pair was issued in the \
         wrong order or the first command was dropped",
        view.new_errors
    );

    // --- 4. rows and cols --------------------------------------------------
    compare_names("rows", &view.rows, &cap.rows, ctx);
    compare_names("cols", &view.cols, &cap.cols, ctx);

    // --- 5. IncMatrix, in upstream's row numbering (S-INC) ------------------
    let remapped = upstream_view_of(
        &view.inc_matrix,
        &view.upstream_row_index,
        view.rows.len(),
        ctx,
    );
    compare_ints("inc_matrix", &remapped, &cap.inc_matrix, ctx);

    // --- 6. Laplacian, unmapped -------------------------------------------
    let laplacian: Vec<i32> = view.laplacian.iter().flatten().copied().collect();
    compare_ints("laplacian", &laplacian, &cap.laplacian, ctx);

    // --- 7. census ---------------------------------------------------------
    if !map_is_identity(&view.upstream_row_index) {
        record_row_decline(case, step);
    }
    COMPARED.fetch_add(1, AtomicOrd::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|x| (*x).to_string()).collect()
    }

    /// Run `f` and return the panic message (the `capture_guard::tests`
    /// extractor; each harness module keeps its own — they compile into separate
    /// test binaries).
    fn panic_message(f: impl FnOnce()) -> String {
        let payload = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f))
            .expect_err("the arm must panic");
        if let Some(m) = payload.downcast_ref::<&str>() {
            (*m).to_string()
        } else if let Some(m) = payload.downcast_ref::<String>() {
            m.clone()
        } else {
            "<non-string panic payload>".to_string()
        }
    }

    /// `asymmetric:reactor/reactor_asym.dss` as both channels report it after
    /// the flat pair (measured: 36 ints / 12 triples, 39 ints / 13 Laplacian
    /// triples, 6 rows, 5 cols) — reduced to the first two rows, which is enough
    /// for every shape arm.
    fn cap() -> IncMatrixCap {
        IncMatrixCap {
            inc_matrix: vec![0, 0, 1, 0, 1, -1, 1, 1, 1, 1, 2, -1],
            laplacian: vec![0, 0, 1, 0, 1, -1, 1, 0, -1, 1, 1, 2],
            rows: s(&["Line.l1", "Line.l2"]),
            cols: s(&["sourcebus", "b1", "b2", "b3", "b4"]),
        }
    }

    /// **N1/N2 (the kill criterion) restated at the gate.** A capi payload that
    /// still carries its unwritten `+1` cell, or an r4133 one whose `[0]`
    /// sentinel was never decoded, arrives `len % 3 != 0` and FAILS instead of
    /// being re-stripped here.
    #[test]
    fn an_un_normalized_integer_array_fails_instead_of_being_repaired() {
        assert_transport_shape(&cap(), "ctx step 0");
        for what in ["inc_matrix", "laplacian"] {
            let mut c = cap();
            if what == "inc_matrix" {
                c.inc_matrix.push(0);
            } else {
                c.laplacian.push(0);
            }
            let msg = panic_message(move || assert_transport_shape(&c, "ctx step 0"));
            assert!(msg.contains(&format!("`{what}` has length")), "{msg}");
            assert!(msg.contains("kill criterion"), "{msg}");
            assert!(msg.contains("transport failure"), "{msg}");
        }
        // The empty matrix is a legal shape on both channels (104 live r4133
        // steps): `[]`, not `[0]` and not `[..., 0]`.
        let empty = IncMatrixCap {
            inc_matrix: vec![],
            laplacian: vec![],
            rows: s(&[]),
            cols: s(&["sourcebus"]),
        };
        assert_transport_shape(&empty, "ctx step 0");
        assert_oracle_self_consistent(&empty, "ctx step 0");
    }

    /// **N3 restated at the gate.** A one-entry `['']` (capi) / `['None']`
    /// (r4133) name list next to an EMPTY matrix is the un-decoded sentinel, and
    /// an empty entry anywhere is a lost name — both fail rather than compare a
    /// phantom.
    #[test]
    fn an_un_decoded_name_sentinel_fails_instead_of_being_repaired() {
        for token in ["", "None", "NONE", "none"] {
            let c = IncMatrixCap {
                inc_matrix: vec![],
                laplacian: vec![],
                rows: s(&[token]),
                cols: s(&["sourcebus"]),
            };
            let msg = panic_message(move || assert_transport_shape(&c, "ctx step 0"));
            assert!(msg.contains("UN-normalized"), "{token:?}: {msg}");
        }
        // An interior / trailing empty entry is a LOST name on both lists.
        for (what, c) in [
            (
                "rows",
                IncMatrixCap {
                    rows: s(&["Line.l1", ""]),
                    ..cap()
                },
            ),
            (
                "cols",
                IncMatrixCap {
                    cols: s(&["sourcebus", "  ", "b2", "b3", "b4"]),
                    ..cap()
                },
            ),
        ] {
            let msg = panic_message(move || assert_transport_shape(&c, "ctx step 0"));
            assert!(
                msg.contains(&format!("`{what}` carries an empty entry")),
                "{msg}"
            );
            assert!(msg.contains("LOST name"), "{msg}");
        }
        // A bus genuinely NAMED `None` beside a non-empty matrix is not the
        // sentinel and must pass — the rule is scoped to the empty state.
        let named = IncMatrixCap {
            cols: s(&["None"]),
            rows: s(&["Line.l1"]),
            inc_matrix: vec![0, 0, 1],
            laplacian: vec![0, 0, 1],
        };
        assert_transport_shape(&named, "ctx step 0");
    }

    /// The oracle's own consistency arm has teeth in **both** directions, on
    /// both biconditionals — a transport that dropped the whole row list, or the
    /// whole Laplacian, would otherwise reach the port compare and be reported
    /// as a Rust divergence.
    #[test]
    fn the_oracle_self_consistency_arm_has_teeth() {
        assert_oracle_self_consistent(&cap(), "ctx step 0");
        let msg = panic_message(|| {
            assert_oracle_self_consistent(
                &IncMatrixCap {
                    rows: s(&[]),
                    ..cap()
                },
                "ctx step 0",
            )
        });
        assert!(msg.contains("the oracle contradicts itself"), "{msg}");
        assert!(msg.contains("`rows` holds 0 name(s)"), "{msg}");
        let msg = panic_message(|| {
            assert_oracle_self_consistent(
                &IncMatrixCap {
                    laplacian: vec![],
                    ..cap()
                },
                "ctx step 0",
            )
        });
        assert!(msg.contains("`laplacian` holds 0 triple(s)"), "{msg}");
        // And the other direction: names with no triples.
        let msg = panic_message(|| {
            assert_oracle_self_consistent(
                &IncMatrixCap {
                    inc_matrix: vec![],
                    laplacian: vec![],
                    ..cap()
                },
                "ctx step 0",
            )
        });
        assert!(msg.contains("the oracle contradicts itself"), "{msg}");
    }

    /// The integer arm is exact and ordered, and its message decodes the flat
    /// index back to `(triple, field)` — the readability contract a 90 918-int
    /// array needs. A sign flip, a dropped triple and a moved row all red.
    #[test]
    fn the_integer_arm_is_exact_and_names_the_triple_that_moved() {
        let good = cap().inc_matrix;
        compare_ints("inc_matrix", &good, &good, "ctx step 0");
        // V1 — one value's sign flipped.
        let mut flipped = good.clone();
        flipped[2] = -1;
        let msg = panic_message(|| compare_ints("inc_matrix", &flipped, &good, "ctx step 0"));
        assert!(
            msg.contains("differs at int 2 (triple 0, the `value`)"),
            "{msg}"
        );
        // V2 — the last triple dropped.
        let short = good[..good.len() - 3].to_vec();
        let msg = panic_message(|| compare_ints("inc_matrix", &short, &good, "ctx step 0"));
        assert!(msg.contains("length differs: Rust 9 vs oracle 12"), "{msg}");
        // A moved ROW index — the S-INC arm's own failure mode.
        let mut moved = good.clone();
        moved[9] = 2;
        let msg = panic_message(|| compare_ints("inc_matrix", &moved, &good, "ctx step 0"));
        assert!(
            msg.contains("differs at int 9 (triple 3, the `row`)"),
            "{msg}"
        );
        assert!(msg.contains("Rust 2 vs oracle 1"), "{msg}");
    }

    /// The name arm is ordered and case-insensitive: a rename, a reordering and
    /// a length change all red, a case change does not.
    #[test]
    fn the_name_arm_is_ordered_and_case_insensitive() {
        let oracle = s(&["Line.l1", "Line.l2"]);
        compare_names("rows", &oracle, &oracle, "ctx step 0");
        compare_names("rows", &s(&["LINE.L1", "line.l2"]), &oracle, "ctx step 0");
        let msg = panic_message(|| {
            compare_names("rows", &s(&["Line.l2", "Line.l1"]), &oracle, "ctx step 0")
        });
        assert!(msg.contains("`rows`[0] differs"), "{msg}");
        let msg = panic_message(|| {
            compare_names("cols", &s(&["sourcebus"]), &s(&["sourcebus", "b1"]), "ctx")
        });
        assert!(
            msg.contains("`cols` length differs: Rust 1 vs oracle 2"),
            "{msg}"
        );
    }

    /// **Settlement S-INC.** The remap rewrites ONLY the row component, and it
    /// is the identity wherever upstream's cursor and the port's dense rows
    /// agree. Numbers are the measured micro deck (two lines, a shunt reactor,
    /// then a series reactor): the port puts `Reactor.r1` at row **2** with
    /// `rows.len() == 3`, upstream at row **3**.
    #[test]
    fn the_remap_rewrites_only_the_row_and_is_the_identity_without_a_gap() {
        let inc = [
            [0, 0, 1],
            [0, 1, -1],
            [1, 2, 1],
            [1, 3, -1],
            [2, 1, 1],
            [2, 2, -1],
        ];
        let gap = [0, 1, 3];
        assert!(!map_is_identity(&gap));
        assert_eq!(
            upstream_view_of(&inc, &gap, 3, "ctx step 0"),
            vec![0, 0, 1, 0, 1, -1, 1, 2, 1, 1, 3, -1, 3, 1, 1, 3, 2, -1],
            "only the row component moves: 2 -> 3 on the last two triples"
        );
        // Without a shunt reactor ahead of it the map is the identity and the
        // remap is a no-op — the state of every corpus deck but four.
        let dense = [0, 1, 2];
        assert!(map_is_identity(&dense));
        assert_eq!(
            upstream_view_of(&inc, &dense, 3, "ctx step 0"),
            inc.iter().flatten().copied().collect::<Vec<i32>>()
        );
        assert!(map_is_identity(&[]), "no rows is a trivial identity");
    }

    /// The port-side consistency arms of [`upstream_view_of`]: a map that does
    /// not cover every row, and a triple whose row does not index a row name,
    /// both FAIL — the port's dense-row contract, which is the half of S-INC
    /// that is ours to keep.
    #[test]
    fn the_port_side_dense_row_contract_has_teeth() {
        // A triple whose row does not index a row name.
        let msg = panic_message(|| {
            upstream_view_of(&[[0, 0, 1], [3, 1, -1]], &[0], 1, "ctx step 0");
        });
        assert!(msg.contains("the port contradicts itself"), "{msg}");
        assert!(msg.contains("only 1 row name(s) exist"), "{msg}");
        // A map that does not cover every row.
        let msg = panic_message(|| {
            upstream_view_of(&[[0, 0, 1]], &[], 1, "ctx step 0");
        });
        assert!(
            msg.contains("`upstream_row_index` has 0 entry(ies)"),
            "{msg}"
        );
    }

    /// The census keys on `(case, step)` and dedups the channel visit, so a
    /// `both` case-step counts ONCE in [`INC_UPSTREAM_ROW_DECLINES`] and twice
    /// in the visit tally.
    #[test]
    fn the_census_counts_cases_and_case_steps_not_channel_visits() {
        let mut set = BTreeSet::new();
        set.insert(("asymmetric:reactor/midi_reactor_asym.dss".to_string(), 0));
        set.insert(("asymmetric:reactor/midi_reactor_asym.dss".to_string(), 0));
        set.insert(("asymmetric:reactor/midi_reactor_asym.dss".to_string(), 1));
        set.insert(("asymmetric:reactor/reactor_asym.dss".to_string(), 0));
        assert_eq!(cases_and_steps(&set), (2, 3));
        assert_eq!(cases_and_steps(&BTreeSet::new()), (0, 0));
    }

    /// The epilogue rule, stated over injected values so it is provable without
    /// the process-global census: it self-silences ONLY on a run that compared
    /// nothing, refuses a census that recorded declines without running, and
    /// fails in BOTH directions against [`INC_UPSTREAM_ROW_DECLINES`].
    #[test]
    fn the_census_epilogue_self_silences_only_on_an_empty_run() {
        let c = |compared, rows| DeclineCensus {
            compared,
            rows,
            row_visits: 0,
        };
        // Nothing compared: silent (the state while F6 has not wired the flag).
        check_census(c(0, (0, 0)), "report");
        // Nothing compared yet declines recorded: impossible, and refused.
        let msg = panic_message(|| check_census(c(0, (1, 1)), "report"));
        assert!(
            msg.contains("yet the S-INC census recorded declines"),
            "{msg}"
        );
        // The pinned population, exactly: silent.
        check_census(c(880, INC_UPSTREAM_ROW_DECLINES), "report");
        // A shrink and a growth both red, and the message names both numbers.
        for measured in [(3, 5), (5, 6), (0, 0)] {
            let msg = panic_message(move || check_census(c(880, measured), "the report"));
            assert!(msg.contains(&format!("measured {measured:?}")), "{msg}");
            assert!(
                msg.contains(&format!("pinned {INC_UPSTREAM_ROW_DECLINES:?}")),
                "{msg}"
            );
            assert!(msg.contains("the report"), "{msg}");
        }
        // The report renders the (here empty) population.
        assert!(decline_report().contains("case-step(s)"));
    }
}
