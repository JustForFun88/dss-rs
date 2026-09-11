//! **GOLDEN_REBASE G1.10b — the expected-value pins behind the run-file
//! CONTENTS surface** (`harness::run_file_contents::compare_run_file_cells`:
//! every CELL of every run-produced report the gate selects, compared under the
//! D40(1) rule `|a − b| ≤ floor(quantity class, tol_for(&kind)) + ulp(print
//! format)`).
//!
//! Five measured cell classes (C1–C5, `tmp/g110b/STOP.md` §2.2) and three
//! deliberate declines carry this surface, and CLAUDE.md wants each of them
//! pinned by an expected-value test that names BOTH numbers:
//!
//! * **C1** the angle of a cancellation residual, where one engine prints an
//!   exact `0 A ∠ 0.00°` and the other a `1e-11 A` residual with a real
//!   argument — the two-sided magnitude rule;
//! * **C2** an angle one print-ulp apart on a current three orders under the
//!   case's own `i_abs`;
//! * **C3** a `%10.6g` magnitude one unit apart in its last significant place;
//! * **C4** a near-zero cancellation cell bounded by the case floor;
//! * **C5** the two oracles' exponent spellings inside `Export Y`'s `+j` cells;
//! * the Storage `DebugTrace`'s 36 `%-.g` columns, which the two ORACLES
//!   disagree on (D40(3)), and its read-back TAIL, which is the gate's own
//!   reader footprint (D43(1));
//! * the reports that are recorded and never compared (D40(5)).
//!
//! Every pin takes the port's side from the port itself — the very
//! `RunFileProbe` the corpus gate uses, over the vendored corpus deck (never
//! `.inputs/`, CLAUDE.md) — so no port literal can rot: if the engine's own
//! render of a pinned cell moves, the pin reds before the claim in the docs goes
//! stale. The ORACLE side of every pair is the literal the gate's own capture
//! measured on the named channel (`tmp/g110b/f4_cells2.tsv`, the instrumented
//! scoped drive of 2026-09-12, whose cells agree with the R part's independent
//! Python replica, `tmp/g110b/census.txt`); each doc comment says which side is
//! which.
//!
//! **Windows-only**, like the surface: the classification and the sidecar
//! transport live in `dss_epri::guard`, and `dss-epri` is `#[cfg(windows)]`.
#![cfg(windows)]

mod harness;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use dss_core::exec::Dss;
use harness::run_file_contents::{
    CONTENTS_NOT_SELECTED, CellTally, ReportKind, compare_run_file_cells, trace_tail_census,
};
use harness::run_files::{MatchedRunFile, RunFileProbe};
use harness::tol_for;

// ---------------------------------------------------------------------------
// The decks these pins read, and the port-side probe that runs them.
// ---------------------------------------------------------------------------

/// `IEEETestCases/8500-Node/Run_8500Node.dss` — the C1 / C2 / C3 cells.
const DECK_8500: &str = "electricdss-tst/Version8/Distrib/IEEETestCases/8500-Node/Run_8500Node.dss";
/// `IEEETestCases/8500-Node/Run_8500Node_Unbal.dss` — the C4 currents cell and
/// the only live `Export Profile`.
const DECK_8500U: &str =
    "electricdss-tst/Version8/Distrib/IEEETestCases/8500-Node/Run_8500Node_Unbal.dss";
/// `IEEETestCases/NEVTestCase/Run_NEV.dss` — the only live `Export Y` and
/// `Export Yprims`; the C4 admittance cell and the C5 `+j` cells.
const DECK_NEV: &str = "electricdss-tst/Version8/Distrib/IEEETestCases/NEVTestCase/Run_NEV.dss";
/// `Test/PVSystemTest.dss` — the only live register report (`EXP_PV_<NAME>.CSV`).
const DECK_PV: &str = "electricdss-tst/Test/PVSystemTest.dss";
/// `Examples/StorageTechNote/Example_9_3_Price/Storage_price.dss` — the only
/// corpus deck with `debugtrace=yes` (G1.10a F4a ported the writer).
const DECK_STOR: &str =
    "electricdss-tst/Version8/Distrib/Examples/StorageTechNote/Example_9_3_Price/Storage_price.dss";

/// The vendored corpus path of a deck, exactly as the corpus gate addresses it.
fn corpus_deck(rel: &str) -> PathBuf {
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
    deck
}

/// Run one corpus deck the way `corpus_gate::runner::run_rust_capture` does
/// (`clear` → `compile` → one `solve`; every deck pinned here is `n_steps = 1`
/// in `tests/corpus/manifests/solvable_now.json`) inside the gate's own
/// run-file probe, and return the CONTENTS of the members the gate selects.
///
/// Memoized behind a `Mutex`, which serializes the runs as well: two of these
/// decks share one case directory (`8500-Node/`), and the gate's own rule is one
/// producer per case directory at a time (D33(2)/D35(2)). The probe removes
/// everything the run created, so the vendored corpus is left as it was found.
fn port_contents(rel: &str) -> BTreeMap<String, String> {
    static RUNS: OnceLock<Mutex<BTreeMap<String, BTreeMap<String, String>>>> = OnceLock::new();
    let mut runs = RUNS
        .get_or_init(|| Mutex::new(BTreeMap::new()))
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if let Some(hit) = runs.get(rel) {
        return hit.clone();
    }
    let deck = corpus_deck(rel);
    let probe = RunFileProbe::start(&deck.to_string_lossy());
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!("compile \"{}\"", deck.display()));
    assert!(dss.errors().is_empty(), "{rel}: {:?}", dss.errors());
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{rel}: {:?}", dss.errors());
    // The gate's port capture reads every element's terminal quantities once at
    // the converged `NodeV` (`corpus_gate::runner`'s `dss.snapshot_elements()`,
    // GOLDEN_REBASE G2.3), so the probe does too: on a deck with a Storage
    // `debugtrace` that read is itself a writer — it appends the two records
    // that make up the port's half of the read-back tail
    // (`the_storage_trace_tail_is_the_readers_footprint`). Leaving it out would
    // give these pins a port file the gate never sees.
    let _ = dss.snapshot_elements();
    // Drop the engine BEFORE the probe reads, exactly as the runner does.
    drop(dss);
    let report = probe.finish_and_clean(&format!("run_file_contents_pins:{rel}"));
    runs.insert(rel.to_string(), report.contents.clone());
    report.contents
}

/// One selected report the port wrote, by its normalized created-set name.
fn port_file(rel: &str, name: &str) -> String {
    let all = port_contents(rel);
    all.get(name)
        .unwrap_or_else(|| {
            panic!(
                "{rel} no longer writes the selected report {name:?}; the port \
                 wrote {:?}. These pins read the PORT side live, so a report that \
                 stops travelling reds here instead of leaving the doc claim stale.",
                all.keys().collect::<Vec<_>>()
            )
        })
        .clone()
}

// ---------------------------------------------------------------------------
// Report surgery — the oracle side of a pin is the port's own file with the
// measured oracle spelling substituted into the named cell.
// ---------------------------------------------------------------------------

/// The lines of a report as the comparator sees them (`trim_end`, blank lines
/// dropped) — `harness`'s own `report_lines` contract, restated because it is
/// private to that module.
fn lines(text: &str) -> Vec<String> {
    text.lines()
        .map(|l| l.trim_end().to_string())
        .filter(|l| !l.trim().is_empty())
        .collect()
}

/// The header lines plus exactly the named data rows, as one report text.
///
/// Slicing keeps a drive honest and fast at once: the header, the field widths
/// and the values are the engine's own, while the comparison is a handful of
/// rows instead of the 2.6 M cells of `NEV_EXP_Y.csv`.
fn rows_of(kind: ReportKind, text: &str, rows: &[usize]) -> String {
    let (_, header_lines) = kind.structure();
    let all = lines(text);
    let mut out: Vec<String> = all[..header_lines].to_vec();
    for r in rows {
        out.push(
            all.get(header_lines + r)
                .unwrap_or_else(|| {
                    panic!(
                        "the {} report has no data row {r} (it has {})",
                        kind.label(),
                        all.len() - header_lines
                    )
                })
                .clone(),
        );
    }
    out.join("\n") + "\n"
}

/// The text of field `j` of data row `row`.
fn field(kind: ReportKind, text: &str, row: usize, j: usize) -> String {
    let (sep, header_lines) = kind.structure();
    let all = lines(text);
    let line = all
        .get(header_lines + row)
        .unwrap_or_else(|| panic!("the {} report has no data row {row}", kind.label()));
    line.split(sep)
        .map(str::trim)
        .nth(j)
        .unwrap_or_else(|| panic!("the {} report's row {row} has no field {j}", kind.label()))
        .to_string()
}

/// Rewrite field `j` of data row `row`. Fields are trimmed by the comparator
/// (`harness`'s `split_fields`), so re-joining with the separator changes the
/// padding and nothing the comparison can see; the header lines are copied
/// verbatim.
fn set_field(kind: ReportKind, text: &str, row: usize, j: usize, value: &str) -> String {
    let (sep, header_lines) = kind.structure();
    let mut all = lines(text);
    let at = header_lines + row;
    assert!(
        at < all.len(),
        "the {} report has no data row {row}",
        kind.label()
    );
    let mut fields: Vec<String> = all[at].split(sep).map(str::to_string).collect();
    assert!(
        j < fields.len(),
        "the {} report's row {row} has {} field(s), not {}",
        kind.label(),
        fields.len(),
        j + 1
    );
    fields[j] = value.to_string();
    all[at] = fields.join(&sep.to_string());
    all.join("\n") + "\n"
}

/// Swap two fields of one data row (the "swapped column pair" mutation).
fn swap_fields(kind: ReportKind, text: &str, row: usize, a: usize, b: usize) -> String {
    let (va, vb) = (field(kind, text, row, a), field(kind, text, row, b));
    assert_ne!(
        va,
        vb,
        "the {} report's row {row} prints the same text in fields {a} and {b}, \
         so swapping them would mutate nothing — the drive must pick two columns \
         that differ",
        kind.label()
    );
    let once = set_field(kind, text, row, a, &vb);
    set_field(kind, &once, row, b, &va)
}

/// Drop one data row (the "dropped row" mutation).
fn drop_row(kind: ReportKind, text: &str, row: usize) -> String {
    let (_, header_lines) = kind.structure();
    let mut all = lines(text);
    all.remove(header_lines + row);
    all.join("\n") + "\n"
}

/// Swap two whole data rows (the "re-ordered row" mutation).
fn swap_rows(kind: ReportKind, text: &str, a: usize, b: usize) -> String {
    let (_, header_lines) = kind.structure();
    let mut all = lines(text);
    all.swap(header_lines + a, header_lines + b);
    all.join("\n") + "\n"
}

/// One unit in the LAST PRINTED PLACE of a rendered cell, read off the text
/// itself (`683.41` → `0.01`; `1.02898E-11` → `1e-16`; `277.1` → `0.1`).
///
/// This is what "a `%g` last-digit re-spelling" means at the cell level, and
/// deriving it from the render rather than from the column map is what lets one
/// drive run over every kind without a second copy of the format table: for a
/// `%.Ng` render that did not strip trailing zeros it is exactly
/// `PrintFmt::Sig(N)`'s ulp at that magnitude, and for a `:w:d` render exactly
/// `10^-d`.
fn last_place(text: &str) -> f64 {
    let t = text.trim();
    let (mantissa, exponent) = match t.find(['e', 'E']) {
        Some(i) => (
            &t[..i],
            t[i + 1..]
                .parse::<i32>()
                .unwrap_or_else(|e| panic!("{t:?} has no exponent: {e}")),
        ),
        None => (t, 0),
    };
    let decimals = match mantissa.find('.') {
        Some(i) => (mantissa.len() - i - 1) as i32,
        None => 0,
    };
    10f64.powi(exponent - decimals)
}

// ---------------------------------------------------------------------------
// Driving the comparator.
// ---------------------------------------------------------------------------

/// Compare one (oracle, port) pair through the live comparator at the case's own
/// calibrated floor. Every deck pinned here is `kind = "feeder"` in
/// `tests/corpus/manifests/solvable_now.json`, which is where
/// `corpus_gate::runner` reads its `tol_for` from.
fn compare(channel: &str, name: &str, oracle: &str, port: &str) -> CellTally {
    compare_labeled("pin:run_file_contents", channel, name, oracle, port)
}

/// [`compare`] under a caller-chosen case label — the process-wide
/// [`trace_tail_census`] is shared by every comparison in this binary, so an
/// assertion about ONE of them filters on its own label.
fn compare_labeled(label: &str, channel: &str, name: &str, oracle: &str, port: &str) -> CellTally {
    compare_run_file_cells(
        channel,
        &[MatchedRunFile {
            name: name.to_string(),
            oracle: oracle.to_string(),
            port: port.to_string(),
        }],
        &tol_for("feeder"),
        label,
    )
}

/// Run `f` and return its panic message — a pin that claims "this would fail"
/// must show the failure it means, not merely that something failed.
fn panic_message(f: impl FnOnce()) -> String {
    let payload =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).expect_err("the arm must panic");
    if let Some(m) = payload.downcast_ref::<&str>() {
        (*m).to_string()
    } else if let Some(m) = payload.downcast_ref::<String>() {
        m.clone()
    } else {
        "<non-string panic payload>".to_string()
    }
}

/// The comparison must FAIL; returns the message.
fn fails(channel: &str, name: &str, oracle: &str, port: &str) -> String {
    panic_message(|| {
        compare(channel, name, oracle, port);
    })
}

// ---------------------------------------------------------------------------
// C1–C5: the five measured cell classes.
// ---------------------------------------------------------------------------

/// **C1 — the angle of a cancellation residual is free on BOTH sides, and the
/// magnitude beside it is not.** On `Run_8500Node.dss` row 2049
/// (`Line.LN6258530-1`) the port prints `1.02898E-11 A ∠ 45.00°` where dss_capi
/// 0.14.5 prints an exact `0 A ∠ 0.00°`.
///
/// The residual pair is the sum of a terminal's conductor currents (r4133
/// `Common/ExportResults.pas:475`, the `', %10.6g, %8.2f'` residual write inside
/// `CalcAndWriteCurrents` `:458`); at `1e-11 A` on a feeder whose real currents
/// are `1e1…1e4 A` the two engines' underlying phasors agree far inside the
/// case's own `i_abs = 1e-5 A`, so the ARGUMENT of that phasor carries no
/// information at all — `harness::polar_angle_band` returns `None` once
/// `|I| ≤ allowed` and the cell is DECLINED, counted, never compared.
///
/// What makes the rule two-sided (D40(1)) is that the magnitude is
/// `max(|oracle|, |port|)`: the golden's `GateSpec::PrevCol(1e-6)` reads the
/// ORACLE's magnitude only and deliberately never fires on an exact `0`, so on
/// exactly this class it would compare `45.00°` against `0.00°` as two numbers.
///
/// Both numbers: port `1.02898E-11 A ∠ 45.00°` (read live below) vs
/// `capi_v0145` `0 A ∠ 0.00°` (measured, `tmp/g110b/f4_cells2.tsv`). The r4133
/// channel prints its own residual there, which is why the class is gated and
/// not excluded.
#[test]
fn an_angle_of_a_residual_magnitude_is_gated_on_both_sides() {
    let kind = ReportKind::Currents;
    let name = "ieee8500_exp_currents.csv";
    let full = port_file(DECK_8500, name);
    const ROW: usize = 2049;
    assert_eq!(
        field(kind, &full, ROW, 0),
        "Line.LN6258530-1",
        "the pinned row moved; re-measure before re-pointing it"
    );
    // The PORT side, live.
    assert_eq!(field(kind, &full, ROW, 11), "1.02898E-11");
    assert_eq!(field(kind, &full, ROW, 12), "45.00");

    let port = rows_of(kind, &full, &[ROW]);
    // The ORACLE side, measured on the capi_v0145 channel.
    let oracle = set_field(kind, &port, 0, 11, " 0");
    let oracle = set_field(kind, &oracle, 0, 12, "     0.00");
    let tally = compare("capi_v0145", name, &oracle, &port);
    assert_eq!(tally.files, 1);
    assert!(
        tally.declined >= 1 && tally.compared >= 2,
        "the row must carry at least the declined angle and its compared \
         magnitude: {tally:?}"
    );

    // …and the decline is a property of the MAGNITUDE, not of the column: give
    // both sides a real current and the same 45° gap fails.
    let real_port = set_field(kind, &port, 0, 11, " 683.41");
    let real_oracle = set_field(kind, &oracle, 0, 11, " 683.41");
    let msg = fails("capi_v0145", name, &real_oracle, &real_port);
    assert!(
        msg.contains("Ang (deg) of the currents map") && msg.contains("port 45 vs oracle 0"),
        "a real magnitude's angle must still be compared: {msg}"
    );

    // …and the magnitude column itself is never masked by its angle's decline.
    let moved = set_field(kind, &port, 0, 11, " 1.0");
    let msg = fails("capi_v0145", name, &oracle, &moved);
    assert!(
        msg.contains("|I| (A) of the currents map") && msg.contains("port 1 vs oracle 0"),
        "the residual MAGNITUDE stays compared at the case floor: {msg}"
    );
}

/// **C2 — the angle of a 17-microamp current is free within the case floor, and
/// its magnitude still has to agree.** On `Run_8500Node.dss` row 934
/// (`Line.LN5486729-1`) the port prints `0.0000169532 A ∠ -74.04°` (=
/// 1.69532E-5), dss_capi 0.14.5 `0.000016957 A ∠ -74.06°` (= 1.6957E-5).
///
/// The band is the image of the magnitude's own floor:
/// `Δθ ≤ rad2deg·(i_abs + i_rel·|I|)/|I| + ulp(%8.2f)`
/// `= 57.29577951308232 · 1.0000016957e-5 / 1.6957e-5 + 0.01 = 33.7896…°`, and
/// the measured `Δθ` is `0.02°`. The current is three orders of magnitude under
/// the case's own `i_abs = 1e-5 A`, so its argument is nearly unconstrained —
/// but the MAGNITUDES had to meet at `Δ|I| = 3.8e-9 A`, four orders inside the
/// same floor, and the cell is COMPARED, not declined (the negative drive below
/// is what proves that: a declined cell would swallow any angle).
///
/// Both numbers: port `-74.04°` (live) vs `capi_v0145` `-74.06°` (measured).
#[test]
fn the_angle_of_a_16_microamp_current_is_free_within_the_case_floor() {
    let kind = ReportKind::Currents;
    let name = "ieee8500_exp_currents.csv";
    let full = port_file(DECK_8500, name);
    const ROW: usize = 934;
    assert_eq!(
        field(kind, &full, ROW, 0),
        "Line.LN5486729-1",
        "the pinned row moved; re-measure before re-pointing it"
    );
    assert_eq!(field(kind, &full, ROW, 3), "0.0000169532");
    assert_eq!(field(kind, &full, ROW, 4), "-74.04");

    let port = rows_of(kind, &full, &[ROW]);
    let oracle = set_field(kind, &port, 0, 3, " 0.000016957");
    let oracle = set_field(kind, &oracle, 0, 4, "   -74.06");
    compare("capi_v0145", name, &oracle, &port);

    // The cell is compared, not declined: 34° more and the same pair fails.
    let far = set_field(kind, &port, 0, 4, "   -40.00");
    let msg = fails("capi_v0145", name, &oracle, &far);
    assert!(
        msg.contains("Ang (deg) of the currents map") && msg.contains("port -40 vs oracle -74.06"),
        "the angle band is finite here: {msg}"
    );

    // And the paired magnitude is compared at the case floor, not by the angle's
    // slack: a 1e-3 A move on a 1.7e-5 A cell fails.
    let moved = set_field(kind, &port, 0, 3, " 0.001");
    let msg = fails("capi_v0145", name, &oracle, &moved);
    assert!(
        msg.contains("|I| (A) of the currents map"),
        "the magnitude of a below-floor current is still compared: {msg}"
    );
}

/// **C3 — a six-significant-digit magnitude may move by one unit in its last
/// printed place, and by nothing more.** On `Run_8500Node.dss` row 927
/// (`Line.LN5472403-3`) the port prints `0.0374457 A` where the EPRI r4133 DLL
/// prints `0.0374458 A` — a `Δ = 1e-7 A` that is a property of the RENDER
/// (`%10.6g`, r4133 `Common/ExportResults.pas:471`), not of the two solutions:
/// the same current is compared in memory on the same case by the live element
/// comparator and passes at `i_rel = 1e-7`.
///
/// The band is `i_abs 1e-5 + i_rel·3.74e-2 + ulp(Sig(6), 3.74e-2) 1e-7`; the
/// negative drive shows that a `1e-3 A` move — four decades larger and still
/// invisible to a `%10.6g` eyeball — fails.
///
/// Both numbers: port `0.0374457` (live) vs `r4133` `0.0374458` (measured).
#[test]
fn a_six_significant_digit_cell_may_move_by_one_ulp() {
    let kind = ReportKind::Currents;
    let name = "ieee8500_exp_currents.csv";
    let full = port_file(DECK_8500, name);
    const ROW: usize = 927;
    assert_eq!(
        field(kind, &full, ROW, 0),
        "Line.LN5472403-3",
        "the pinned row moved; re-measure before re-pointing it"
    );
    assert_eq!(field(kind, &full, ROW, 3), "0.0374457");
    assert_eq!(
        last_place("0.0374457"),
        1e-7,
        "the last printed place of a six-significant-digit render at this \
         magnitude is 1e-7 A"
    );

    let port = rows_of(kind, &full, &[ROW]);
    let oracle = set_field(kind, &port, 0, 3, " 0.0374458");
    compare("r4133", name, &oracle, &port);

    // One ulp, not two decades.
    let moved = set_field(kind, &port, 0, 3, " 0.0384458");
    let msg = fails("r4133", name, &oracle, &moved);
    assert!(
        msg.contains("port 0.0384458 vs oracle 0.0374458"),
        "a 1e-3 A move on the same cell must fail: {msg}"
    );
}

/// **C4 — a cancellation residual is bounded by the case's own floor, in
/// amperes and in siemens.**
///
/// Two cells, one per quantity class and one per gating channel:
///
/// * `Run_8500Node_Unbal.dss` row 0 (`Vsource.SOURCE`) field 9, a source
///   terminal residual: port `2.88924E-9 A` vs `capi_v0145` `1.56542E-8 A`,
///   `Δ = 1.276496e-8 A` against `i_abs = 1e-5 A`;
/// * `Run_NEV.dss` `NEV_EXP_Y.csv` row 340 (`"CKT4-1-8.1"`) field 747, a
///   conductance stamp that cancels: port `-2.498001805E-16 S` vs `r4133`
///   `-1.110223025E-016 S`, `Δ = 1.38777878e-16 S` against `y_abs = 1e-6 S`.
///
/// Both are a difference of large summands, printed at a precision far coarser
/// than the difference itself; the floors they are held to are the case's own
/// calibrated in-memory bands (`harness::tol_for("feeder")`), not a new one.
/// The negative drives move each cell to just above its class floor and require
/// a failure, which is what keeps "bounded by the floor" from meaning "ignored".
#[test]
fn a_cancellation_residual_cell_is_bounded_by_the_case_floor() {
    // (a) amperes, capi_v0145.
    let kind = ReportKind::Currents;
    let name = "ieee8500u_exp_currents.csv";
    let full = port_file(DECK_8500U, name);
    assert_eq!(field(kind, &full, 0, 0), "Vsource.SOURCE");
    assert_eq!(field(kind, &full, 0, 9), "2.88924E-9");
    let port = rows_of(kind, &full, &[0]);
    let oracle = set_field(kind, &port, 0, 9, " 1.56542E-8");
    compare("capi_v0145", name, &oracle, &port);
    let over = set_field(kind, &port, 0, 9, " 0.0001");
    let msg = fails("capi_v0145", name, &oracle, &over);
    assert!(
        msg.contains("|I| (A) of the currents map") && msg.contains("port 0.0001"),
        "a 1e-4 A cell against a 1.6e-8 A oracle is above i_abs and must fail: {msg}"
    );

    // (b) siemens, r4133.
    let kind = ReportKind::YDense;
    let name = "nev_exp_y.csv";
    let full = port_file(DECK_NEV, name);
    const ROW: usize = 340;
    assert_eq!(field(kind, &full, ROW, 0), "\"CKT4-1-8.1\"");
    assert_eq!(field(kind, &full, ROW, 747), "-2.498001805E-16");
    let port = rows_of(kind, &full, &[ROW]);
    let oracle = set_field(kind, &port, 0, 747, "-1.110223025E-016");
    compare("r4133", name, &oracle, &port);
    let over = set_field(kind, &port, 0, 747, "-0.00001");
    let msg = fails("r4133", name, &oracle, &over);
    assert!(
        msg.contains("G (S) of the y(dense) map") && msg.contains("port -0.00001"),
        "a 1e-5 S cell against a 1.1e-16 S oracle is above y_abs and must fail: {msg}"
    );
}

/// **C5 — the two oracles spell the same exponent differently inside
/// `Export Y`'s `+j` cells, and the tokenization is what makes them meet.**
///
/// `ExportY` writes each imaginary part as `+j <number>`
/// (r4133 `Common/ExportResults.pas:3090`), a field no `f64` parser accepts —
/// so the golden's text fallback would compare the Delphi-built r4133 DLL's
/// three-digit exponent against the port's two-digit one as STRINGS and fail on
/// a spelling. `harness::run_file_contents`'s tokenizer removes the marker on
/// both sides (asserting its presence on both, so a layout change cannot hide)
/// and the numbers are then compared by the same rule as every other cell
/// (coordinator decision **D40(4)**).
///
/// Both numbers: port `+j 2.220446049E-16` (live) vs `r4133`
/// `+j 2.220446049E-016` (measured) — the identical f64.
#[test]
fn the_two_oracle_exponent_spellings_of_a_j_cell_meet_numerically() {
    let kind = ReportKind::YDense;
    let name = "nev_exp_y.csv";
    let full = port_file(DECK_NEV, name);
    const ROW: usize = 340;
    assert_eq!(field(kind, &full, ROW, 748), "+j 2.220446049E-16");
    assert_eq!(
        "2.220446049E-016".parse::<f64>().unwrap(),
        "2.220446049E-16".parse::<f64>().unwrap(),
        "the two spellings are the same number — only the render differs"
    );

    let port = rows_of(kind, &full, &[ROW]);
    let oracle = set_field(kind, &port, 0, 748, "+j 2.220446049E-016");
    compare("r4133", name, &oracle, &port);

    // The marker itself is structural: losing it on one side fails.
    let no_marker = set_field(kind, &port, 0, 748, "2.220446049E-16");
    let msg = fails("r4133", name, &oracle, &no_marker);
    assert!(
        msg.contains("the `+j` marker is on one side only"),
        "a lost `+j` is a layout change, not a spelling: {msg}"
    );

    // …and the tokenizer compares the NUMBER, not the text: a real move fails.
    let moved = set_field(kind, &port, 0, 748, "+j 2.220446049E-05");
    let msg = fails("r4133", name, &oracle, &moved);
    assert!(
        msg.contains("+j B (S) of the y(dense) map"),
        "the `+j` cells are compared as numbers: {msg}"
    );
}

// ---------------------------------------------------------------------------
// The Storage DebugTrace: a declined column SET and an accounted reader tail.
// ---------------------------------------------------------------------------

/// **The 36 `%-.g` trace columns are declined on BOTH channels, and the 16
/// fixed-decimal / integer / text columns beside them are compared.**
///
/// `TStorageObj.WriteTraceRecord` renders `t`, `LoadMultiplier` and every state
/// variable with `Format('%-.g', …)` (r4133 `PCElements/Storage.pas:2401-2429`,
/// the record write at `:2412`/`:2421`). FPC reads that empty precision as **2**
/// significant digits and the Delphi-built r4133 DLL as ~15, so the two GATING
/// ORACLES disagree with each other on those columns — measured on
/// `Storage_price.dss` row 2: `Vref` port/capi `1E4` vs r4133 `9999`;
/// `kWTotalLosses` port `5.4` (default lane; the parity lane's `compat::fmt_g`
/// renders `5.5`) vs capi `5.5` vs r4133 `5.45` — three renders of ONE number,
/// and the one cell of this surface where the port's two lanes part. At two
/// significant digits every numeric band is vacuous (a 2 % straddle), so
/// the column SET is declined on both channels and counted — never a tolerance
/// (coordinator decision **D40(3)**; WP-G4 **G4.1** re-measures the decline once
/// the `fmt_g` kernel dies and the port prints at r4133's precision).
///
/// The decline is a COLUMN SET, not a file: exactly 36 of the row's 52 value
/// columns are declined and the other 16 — `Iteration`, `Mode`, `LoadModel`,
/// `StorageModel`, `Qnominalperphase`, `Pnominalperphase`, `CurrentType` and the
/// three `|Iinj|` / `|Iterm|` / `|Vterm|` phase triples — are compared, which the
/// negative drive proves by moving `|Vterm1|` one volt.
#[test]
fn the_two_sig_trace_columns_are_declined_on_both_channels() {
    let kind = ReportKind::StorageTrace;
    let name = "stor_storage1.csv";
    let full = port_file(DECK_STOR, name);
    const ROW: usize = 2;
    // The PORT side, live.
    assert_eq!(field(kind, &full, ROW, 8), "Injection");
    assert_eq!(field(kind, &full, ROW, 15), "277.1");
    // `kWTotalLosses` is the cell that straddles at two significant digits, and
    // it is also where the port's two LANES part: the parity lane's `%-.g`
    // kernel (`compat::fmt_g`) renders `5.5` like dss_capi, the default lane
    // `5.4`, and the Delphi-built r4133 `5.45`. Three renders of one number,
    // none of them wrong at the precision the column is printed with — which is
    // the whole reason this column set is declined instead of banded.
    let port_losses = if cfg!(feature = "oracle-parity") {
        "5.5"
    } else {
        "5.4"
    };
    assert_eq!(
        field(kind, &full, ROW, 24),
        port_losses,
        "kWTotalLosses, FPC `%-.g` (default lane 5.4, parity lane 5.5)"
    );
    assert_eq!(field(kind, &full, ROW, 31), "1E4", "Vref, FPC `%-.g`");

    let port = rows_of(kind, &full, &[ROW]);
    // capi_v0145 prints the same `%-.g` kernel as the port (both are FPC) and
    // still lands elsewhere on a straddle; r4133 prints ~15 significant digits.
    let capi = set_field(kind, &port, 0, 24, " 5.5");
    let r4133 = set_field(kind, &port, 0, 24, " 5.45");
    let r4133 = set_field(kind, &r4133, 0, 31, " 9999");
    for (channel, oracle) in [("capi_v0145", &capi), ("r4133", &r4133)] {
        let tally = compare(channel, name, oracle, &port);
        assert_eq!(
            (tally.compared, tally.declined),
            (16, 36),
            "{channel}: one trace record carries 16 compared and 36 declined \
             `%-.g` columns (the header's 54 fields are 52 value columns, the \
             `Vthev, Theta` pair the record never writes, and the trailing \
             separator)"
        );
    }

    // A compared column is not covered by the decline: one volt on `|Vterm1|`
    // fails on both channels.
    for (channel, oracle) in [("capi_v0145", &capi), ("r4133", &r4133)] {
        let moved = set_field(kind, &port, 0, 15, "    278.1");
        let msg = fails(channel, name, oracle, &moved);
        assert!(
            msg.contains("|Vterm| (V) of the storage-trace map")
                && msg.contains("port 278.1 vs oracle 277.1"),
            "{channel}: the fixed-decimal columns stay compared: {msg}"
        );
    }
}

/// **The Storage trace's row count is the run's PLUS the reader's: 96 solve
/// records on all three engines, then a tail of 2 records the port appends and 6
/// an oracle transport appends.**
///
/// `WriteTraceRecord` sits at the END of `GetTerminalCurrents`, OUTSIDE the
/// `IterminalSolutionCount <> SolutionCount` cache test (r4133
/// `PCElements/Storage.pas:2874`, the test at `:2868-2871`; dss_capi 0.14.5
/// `:2356`, test at `:2348-2353`), so every terminal-current read appends a
/// record — solving or merely reporting. The two oracle transports read the
/// element's terminal quantities six times after the last solve and therefore
/// append six records; the port's `exec::view::snapshot_elements` recomputes
/// ONCE at the converged `NodeV` (GOLDEN_REBASE **G2.3**) and appends two.
///
/// Both numbers, measured on `Storage_price.dss` (48 hours × 2 iterations = 96
/// `Injection` records, identical on all three engines): oracle **102** rows on
/// BOTH channels, port **98** (read live below), gap **4** — the population
/// `corpus_gate::scheduler::TRACE_READBACK_RECORDS` pins fail-on-stale in both
/// directions. The comparator therefore asserts `port_rows <= oracle_rows`,
/// compares every cell of the common prefix and REPORTS the gap (coordinator
/// decision **D43(1)**, option A: positive accounting of the gate's own
/// footprint — never a declined file, never a data-derived boundary).
#[test]
fn the_storage_trace_tail_is_the_readers_footprint() {
    let kind = ReportKind::StorageTrace;
    let name = "stor_storage1.csv";
    let full = port_file(DECK_STOR, name);
    let rows = lines(&full).len() - 1;
    let solve_records = lines(&full)
        .iter()
        .skip(1)
        .filter(|l| l.contains("Injection"))
        .count();
    let read_back = lines(&full)
        .iter()
        .skip(1)
        .filter(|l| l.contains("TotalCurrent"))
        .count();
    // The PORT side, live.
    assert_eq!(
        (rows, solve_records, read_back),
        (98, 96, 2),
        "the port writes 96 solve records (48 hours × 2 iterations) and the two \
         records its single G2.3 recompute appends"
    );
    // The ORACLE side, measured on both channels (`tmp/g110b/f_F2r.md`).
    const ORACLE_ROWS: usize = 102;
    const ORACLE_READ_BACK: usize = 6;
    assert_eq!(ORACLE_ROWS, solve_records + ORACLE_READ_BACK);
    assert_eq!(
        ORACLE_ROWS - rows,
        4,
        "the read-back tail the gate accounts per (case, channel)"
    );

    // Drive the accounting: an oracle file that is the port's plus four more
    // read-back records compares clean and REPORTS the gap.
    let last = lines(&full).last().expect("a trace record").clone();
    let oracle = format!("{}{}\n", full, [last.as_str(); 4].join("\n"));
    let tally = compare_labeled("pin:trace-tail", "capi_v0145", name, &oracle, &full);
    assert_eq!(
        (tally.files, tally.trace_files, tally.trace_tail),
        (1, 1, 4),
        "the gap is reported, not absorbed: {tally:?}"
    );
    let seen = trace_tail_census()
        .into_iter()
        .find(|t| t.ctx.starts_with("pin:trace-tail"))
        .expect("the observation reaches the census the scheduler epilogue reads");
    assert_eq!((seen.oracle_rows, seen.port_rows, seen.gap()), (102, 98, 4));

    // A port that writes MORE records than the oracle is a finding, not a tail.
    let msg = fails("capi_v0145", name, &drop_row(kind, &full, 0), &full);
    assert!(
        msg.contains("the port wrote MORE storage-trace records than the oracle")
            && msg.contains("PCElements/Storage.pas:2874"),
        "the inequality is one-sided and names both Pascal sites: {msg}"
    );

    // …and the common prefix is compared cell by cell, tail or no tail.
    let corrupted = set_field(kind, &full, 0, 15, "    278.1");
    let msg = fails("capi_v0145", name, &oracle, &corrupted);
    assert!(
        msg.contains("|Vterm| (V) of the storage-trace map"),
        "a corrupted cell inside the common prefix still reds: {msg}"
    );
}

// ---------------------------------------------------------------------------
// What is recorded and never compared.
// ---------------------------------------------------------------------------

/// **A report without a policy is RECORDED, never compared ad hoc** — the seven
/// census rows of `harness::run_file_contents::CONTENTS_NOT_SELECTED`
/// (coordinator decision **D40(5)**).
///
/// The bytes of these files never leave the case directory: the gate ships the
/// selection as data (`dss_epri::guard::RUN_FILE_CONTENTS_PATTERNS`) and neither
/// transport copies anything else, so "not compared" is a property of the
/// transport and not of a comparator that quietly returns early. What this pin
/// holds is that every real name the corpus produces in one of those classes is
/// (a) refused by the selection and (b) claimed by a census row that states its
/// reason — and that the selection is not vacuous, i.e. a report that IS mapped
/// still travels.
#[test]
fn a_report_without_a_policy_is_recorded_not_compared() {
    for (what, why) in CONTENTS_NOT_SELECTED {
        assert!(!why.trim().is_empty(), "{what}: a decline needs its reason");
    }
    assert_eq!(CONTENTS_NOT_SELECTED.len(), 7);

    // Real created-set members, measured on the corpus (`tmp/g110b/census.txt`,
    // `tmp/g110a/oracle_diff.txt`), each with the census row that claims it.
    let declined: [(&str, &str); 7] = [
        ("ieee13_mon_m1_1.csv", "monitor CSV"),
        ("ieee13_eventlog.csv", "eventlog CSV"),
        ("ckt7_di_yr_0.csv", "demand-interval tree (DI_yr_*)"),
        ("testygd_curr_elem.txt", "Show / Dump text reports"),
        (
            "kundur_two_area_jacobian.csv",
            "NCIM Jacobian / deltaF / deltaZ",
        ),
        ("ieee13_cim100.xml", "cim100 XML"),
        (
            "auto1bus_hl_current.txt",
            "deck-named export (`export ... file=<name>`)",
        ),
    ];
    for (name, row) in declined {
        assert!(
            !dss_epri::guard::selects_contents(
                &dss_epri::guard::RUN_FILE_CONTENTS_PATTERNS[..],
                name
            ),
            "{name} must not be selected — it belongs to the census row {row:?}"
        );
        assert_eq!(
            ReportKind::of(name, ""),
            None,
            "{name} has no column map, which is why it is recorded instead"
        );
        assert!(
            CONTENTS_NOT_SELECTED.iter().any(|(what, _)| *what == row),
            "the census row {row:?} that claims {name} is gone"
        );
    }

    // Not vacuous: the mapped reports DO travel, and the two `Export Y` arms
    // share one name.
    for (name, first, kind) in [
        ("ieee8500_exp_currents.csv", "Element", ReportKind::Currents),
        ("nev_exp_y.csv", "1146, ", ReportKind::YDense),
        ("nev_exp_y.csv", "Row,Col,G,B", ReportKind::YTriplet),
        (
            "stor_storage1.csv",
            "t, Iteration",
            ReportKind::StorageTrace,
        ),
    ] {
        assert!(dss_epri::guard::selects_contents(
            &dss_epri::guard::RUN_FILE_CONTENTS_PATTERNS[..],
            name
        ));
        assert_eq!(ReportKind::of(name, first), Some(kind));
    }
}

/// **Every compared kind reads its structure out of the SAME `ExportPolicy` its
/// byte golden uses** — the compile-time half of "the live compare uses the
/// policy the golden uses" (§1.1 mechanic (c), coordinator decision **D40(2)**).
///
/// The seven golden policies were lifted byte-faithfully out of
/// `tests/golden_reports.rs` into `harness::export_policies`
/// (`SPLITTING_RULES.md` protocol; the unchanged golden run and `golden_lock`
/// are the proof that no value moved), and both callers now name the same
/// function — so a re-tuned policy moves the goldens and the live surface
/// together, and cannot move one silently.
///
/// The Storage `DebugTrace` is the one kind with no golden at all: it is not an
/// `Export` report, so its separator and header count are declared with their
/// r4133 citation instead of borrowed, which is recorded here rather than papered
/// over.
#[test]
fn every_compared_kind_uses_the_same_policy_as_its_golden() {
    for k in ReportKind::ALL {
        let (sep, header_lines) = k.structure();
        match k.golden_policy() {
            Some(p) => assert_eq!(
                (sep, header_lines),
                (p.sep, p.header_lines),
                "{}: the live structure must be the golden's",
                k.label()
            ),
            None => assert_eq!(
                k,
                ReportKind::StorageTrace,
                "only the Storage DebugTrace may have no golden policy"
            ),
        }
    }
    assert_eq!(ReportKind::Yprim.structure(), (',', 0));
    assert_eq!(ReportKind::Currents.structure(), (',', 1));
    assert_eq!(ReportKind::StorageTrace.structure(), (',', 1));
}

// ---------------------------------------------------------------------------
// The per-kind mutation gates (§1.1(f), spec §2.6).
// ---------------------------------------------------------------------------

/// Which column a kind's scale / re-spelling drives move.
#[derive(Clone, Copy)]
enum Target {
    /// The column the report's own header names (the kinds that have one).
    Header(&'static str),
    /// The first field at or after this index that carries a real value — for
    /// `Export Y`, `Export Yprims` and the register report, whose rows are an
    /// unnamed repeating group.
    Search(usize),
}

/// What "a re-ordered row" means for a kind with only one data row.
#[derive(Clone, Copy)]
enum Reorder {
    /// Swap the first and last row of the slice.
    SwapFirstLast,
    /// The report has a single row: move its identity field instead, which is
    /// the same claim (the row set is load-bearing) on the only row there is.
    IdentityField(usize),
}

/// `Export Y triplet` is the one mapped kind no corpus deck exports today
/// (`Export Y` defaults to the dense arm), so its mutation drive runs on a
/// fixture in the writer's own layout: `Row,Col,G,B` with integer indices and
/// `%.10g` values (r4133 `Common/ExportResults.pas:3052`, `:3059`). Everything
/// else in this drive table is the engine's own live report.
const Y_TRIPLET_FIXTURE: &str = "Row,Col,G,B\n\
     1, 1, 0.001234567891, -0.002345678912\n\
     2, 1, -0.0004567891234, 0.0005678912345\n\
     3, 2, 0.007654321098, -0.008765432109\n";

/// One kind's drive: where the report comes from and which cell to move.
struct Drive {
    kind: ReportKind,
    /// The vendored deck whose live run produces the report, or `None` for the
    /// one kind that has no live case.
    deck: Option<&'static str>,
    file: &'static str,
    target: Target,
    /// The scale drive needs a cell big enough that 1 % of it is outside the
    /// class floor, and the re-spelling drive a render that did not strip
    /// significant digits; the drive picks the first row that qualifies.
    min_abs: f64,
    reorder: Reorder,
}

fn drives() -> Vec<Drive> {
    vec![
        Drive {
            kind: ReportKind::Currents,
            deck: Some(DECK_8500),
            file: "ieee8500_exp_currents.csv",
            target: Target::Header("I1_1"),
            min_abs: 1.0,
            reorder: Reorder::SwapFirstLast,
        },
        Drive {
            kind: ReportKind::Powers,
            deck: Some(DECK_8500),
            file: "ieee8500_exp_powers.csv",
            target: Target::Header("P(kW)"),
            min_abs: 100.0,
            reorder: Reorder::SwapFirstLast,
        },
        Drive {
            kind: ReportKind::Voltages,
            deck: Some(DECK_8500),
            file: "ieee8500_exp_voltages.csv",
            target: Target::Header("Magnitude1"),
            min_abs: 100.0,
            reorder: Reorder::SwapFirstLast,
        },
        Drive {
            kind: ReportKind::Profile,
            deck: Some(DECK_8500U),
            file: "ieee8500u_exp_profile.csv",
            target: Target::Header("puV1"),
            min_abs: 0.5,
            reorder: Reorder::SwapFirstLast,
        },
        Drive {
            kind: ReportKind::YDense,
            deck: Some(DECK_NEV),
            file: "nev_exp_y.csv",
            target: Target::Search(1),
            min_abs: 1e-2,
            reorder: Reorder::SwapFirstLast,
        },
        Drive {
            kind: ReportKind::YTriplet,
            deck: None,
            file: "fixture_exp_y.csv",
            target: Target::Search(2),
            min_abs: 1e-4,
            reorder: Reorder::SwapFirstLast,
        },
        Drive {
            kind: ReportKind::Yprim,
            deck: Some(DECK_NEV),
            file: "nev_exp_yprim.csv",
            target: Target::Search(0),
            min_abs: 1e-2,
            reorder: Reorder::SwapFirstLast,
        },
        Drive {
            kind: ReportKind::Register,
            deck: Some(DECK_PV),
            file: "exp_pv_pv.csv",
            target: Target::Search(4),
            min_abs: 1e3,
            // The register report is one row per PVSystem per write, and
            // `PVSystemTest.dss` writes one; its identity column is the
            // `"NAME"` field (r4133 `Common/ExportResults.pas:2029`).
            reorder: Reorder::IdentityField(3),
        },
        Drive {
            kind: ReportKind::StorageTrace,
            deck: Some(DECK_STOR),
            file: "stor_storage1.csv",
            target: Target::Header("|Vterm1|"),
            min_abs: 1.0,
            reorder: Reorder::SwapFirstLast,
        },
    ]
}

/// The index the header gives a named column.
fn header_index(kind: ReportKind, text: &str, name: &str) -> usize {
    let (sep, header_lines) = kind.structure();
    assert!(
        header_lines >= 1,
        "{}: a header target needs a header line",
        kind.label()
    );
    let header = &lines(text)[header_lines - 1];
    header
        .split(sep)
        .position(|f| f.trim().eq_ignore_ascii_case(name))
        .unwrap_or_else(|| {
            panic!(
                "the {} report no longer has a column called {name:?}: {header:?}",
                kind.label()
            )
        })
}

/// Is this rendered cell usable by the two value drives — a real number (never
/// a `+j` cell, whose marker is structural) and big enough that 1 % of it leaves
/// every class floor?
fn usable(text: &str, min_abs: f64) -> Option<f64> {
    let t = text.trim();
    if t.starts_with("+j") {
        return None;
    }
    let v: f64 = t.parse().ok()?;
    (v.is_finite() && v.abs() >= min_abs).then_some(v)
}

/// How many rows a drive scans for its cell. The reports are up to 2 300 rows
/// wide and 1 146 rows long; the best-printed cell is always in the first
/// handful of decades of values, and the scan is bounded so a drive costs
/// milliseconds.
const SCAN_ROWS: usize = 200;
/// How many fields a `Target::Search` scans in one row.
const SCAN_FIELDS: usize = 64;

/// The (row, field) the value drives move, and its value: among the cells that
/// pass [`usable`], the one whose render carries the MOST significant digits,
/// i.e. the smallest `last place / |v|`.
///
/// That choice is what makes the re-spelling drive exact rather than
/// approximate. `%g` strips trailing zeros, so a cell printed `69715` under
/// `%10.6g` has five significant digits and its last printed place (`1`) is TEN
/// times the column's own print ulp (`0.1`) — moving it by one last place would
/// be a different number, not a re-spelling. The fullest render in the column is
/// by construction one whose last place IS the format's ulp, for a `%N.sg` and a
/// `:w:d` column alike.
fn pick_cell(kind: ReportKind, text: &str, target: Target, min_abs: f64) -> (usize, usize, f64) {
    let (sep, header_lines) = kind.structure();
    let all = lines(text);
    let rows = (all.len() - header_lines).min(SCAN_ROWS);
    let fixed = match target {
        Target::Header(name) => Some(header_index(kind, text, name)),
        Target::Search(_) => None,
    };
    let mut best: Option<(usize, usize, f64, f64)> = None;
    for r in 0..rows {
        let n = all[header_lines + r].split(sep).count();
        let candidates: Vec<usize> = match (fixed, target) {
            (Some(j), _) => vec![j],
            (None, Target::Search(from)) => (from..n.min(from + SCAN_FIELDS)).collect(),
            (None, Target::Header(_)) => unreachable!("a header target resolves to a fixed index"),
        };
        for j in candidates {
            if j >= n {
                continue;
            }
            let text = field(kind, text, r, j);
            let Some(v) = usable(&text, min_abs) else {
                continue;
            };
            let ratio = last_place(&text) / v.abs();
            if best.is_none_or(|(_, _, _, b)| ratio < b) {
                best = Some((r, j, v, ratio));
            }
        }
    }
    let (r, j, v, _) = best.unwrap_or_else(|| {
        panic!(
            "the {} report carries no cell with |v| >= {min_abs:e} in its first              {SCAN_ROWS} row(s) — the drive cannot move a cell that does not              exist; re-measure the report before weakening the drive",
            kind.label()
        )
    });
    (r, j, v)
}

/// Two fields of `row` that differ, carry numbers and agree about the `+j`
/// marker — the pair the "swapped column pair" drive exchanges.
fn pick_swap_pair(kind: ReportKind, text: &str, row: usize, from: usize) -> (usize, usize) {
    let n = field_count(kind, text, row);
    for a in from..n {
        let fa = field(kind, text, row, a);
        if fa.trim().parse::<f64>().is_err() {
            continue;
        }
        for b in (a + 1)..n {
            let fb = field(kind, text, row, b);
            if fb.trim().parse::<f64>().is_err() || fb.trim() == fa.trim() {
                continue;
            }
            return (a, b);
        }
    }
    panic!(
        "the {} report's row {row} has no two numeric columns that differ, so a \
         swapped pair would mutate nothing",
        kind.label()
    )
}

fn field_count(kind: ReportKind, text: &str, row: usize) -> usize {
    let (sep, header_lines) = kind.structure();
    lines(text)[header_lines + row].split(sep).count()
}

/// **Every compared report kind is mutation-gated on its own report**: a 1 %
/// scale error, a swapped column pair, a dropped row and a re-ordered row each
/// RED, while a last-place re-spelling of the same number PASSES (§1.1(f) and
/// spec §2.6).
///
/// The drives run on the engine's OWN live reports — real headers, real widths,
/// real values, read through the same `RunFileProbe` the gate uses — with the
/// mutation applied to the port side, which is the honest direction: it is the
/// port the gate is there to catch. The only fixture is `Export Y triplet`, the
/// one mapped kind no corpus deck exports today; its absence from the live
/// population is itself recorded (`corpus_gate::scheduler`'s per-kind census).
///
/// The slice is four data rows (one for a single-row report) so a drive costs
/// thousands of cells instead of the 2.6 M of `NEV_EXP_Y.csv`, and the control
/// comparison at the top of each drive is what proves the slice compares at all
/// — a mutation gate over a file the comparator silently ignores would pass
/// every "must RED" clause for the wrong reason.
#[test]
fn every_compared_kind_is_mutation_gated_on_its_own_report() {
    let mut covered: Vec<ReportKind> = Vec::new();
    for d in drives() {
        let kind = d.kind;
        let text = match d.deck {
            Some(deck) => port_file(deck, d.file),
            None => Y_TRIPLET_FIXTURE.to_string(),
        };
        let (row, j, v) = pick_cell(kind, &text, d.target, d.min_abs);
        let (_, header_lines) = kind.structure();
        let available = lines(&text).len() - header_lines;
        // Four data rows around the cell the value drives move — enough for a
        // re-ordering to have something to move, and few enough that a drive
        // costs thousands of cells instead of millions. `tr` is the target row
        // inside the slice.
        let start = row.min(available.saturating_sub(4));
        let take: Vec<usize> = (start..available.min(start + 4)).collect();
        let tr = row - start;
        let slice = rows_of(kind, &text, &take);
        let label = format!("mutation:{}", kind.label());

        // Control: the slice compares, and it compares something.
        let tally = compare_labeled(&label, "capi_v0145", d.file, &slice, &slice);
        assert!(
            tally.compared > 0,
            "{}: the drive slice compares no cell at all — every \"must RED\" \
             clause below would then be vacuous",
            kind.label()
        );

        // (1) a 1 % scale error on a real column must RED.
        let scaled = set_field(kind, &slice, tr, j, &format!("{}", v * 1.01));
        let msg = panic_message(|| {
            compare_labeled(&label, "capi_v0145", d.file, &slice, &scaled);
        });
        assert!(
            msg.contains(&format!("row {tr} field {j}")),
            "{}: a 1 % scale error on field {j} must name the cell: {msg}",
            kind.label()
        );

        // (2) …while one unit in the cell's last printed place must PASS: that
        // is the print resolution the D40(1) rule spends, and nothing more.
        let step = last_place(&field(kind, &slice, tr, j));
        let respelt = set_field(kind, &slice, tr, j, &format!("{}", v + step));
        compare_labeled(&label, "capi_v0145", d.file, &slice, &respelt);

        // (3) a swapped column pair must RED.
        // The pair is searched from the first field, not from the scale
        // drive's target: the two columns only have to differ and to be
        // numbers on both sides.
        let (a, b) = pick_swap_pair(kind, &slice, tr, 0);
        let swapped = swap_fields(kind, &slice, tr, a, b);
        let msg = panic_message(|| {
            compare_labeled(&label, "capi_v0145", d.file, &slice, &swapped);
        });
        assert!(
            msg.contains(&format!("row {tr} field")),
            "{}: swapping fields {a} and {b} must RED: {msg}",
            kind.label()
        );

        // (4) a dropped row must RED. The Storage trace is the one kind whose
        // row count is an inequality (D43(1)), so its drive drops the record
        // from the ORACLE side — the direction that is a finding there.
        let (short_oracle, short_port) = if kind == ReportKind::StorageTrace {
            (drop_row(kind, &slice, take.len() - 1), slice.clone())
        } else {
            (slice.clone(), drop_row(kind, &slice, take.len() - 1))
        };
        let msg = panic_message(|| {
            compare_labeled(&label, "capi_v0145", d.file, &short_oracle, &short_port);
        });
        assert!(
            msg.contains("row count") || msg.contains("MORE"),
            "{}: a dropped row must RED on the row count: {msg}",
            kind.label()
        );

        // (5) a re-ordered row must RED.
        let reordered = match d.reorder {
            Reorder::SwapFirstLast => {
                assert!(
                    take.len() >= 2,
                    "{}: SwapFirstLast needs two rows",
                    kind.label()
                );
                swap_rows(kind, &slice, 0, take.len() - 1)
            }
            Reorder::IdentityField(id) => {
                let was = field(kind, &slice, tr, id);
                set_field(kind, &slice, tr, id, &format!("{was}x"))
            }
        };
        let msg = panic_message(|| {
            compare_labeled(&label, "capi_v0145", d.file, &slice, &reordered);
        });
        assert!(
            msg.contains("row"),
            "{}: a re-ordered row (or a moved row identity) must RED: {msg}",
            kind.label()
        );

        covered.push(kind);
    }
    assert_eq!(
        covered,
        ReportKind::ALL.to_vec(),
        "every mapped report kind is mutation-gated, in the order the census \
         reports them"
    );
}
