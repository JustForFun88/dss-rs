//! **GOLDEN_REBASE G1.10c — the expected-value pins behind the demand-interval
//! tree surface** (`compare_di`: the CONTENTS of every CSV a run creates under
//! `<OutputDirectory><CaseName>/DI_yr_<year>/`, compared as a file set, header
//! verbatim, row and field counts exact, and every cell at the calibrated tier
//! of the physical quantity its column carries — coordinator decision
//! **D42(1)**, the class table in `crates/dss-core/tests/harness/di.rs`).
//!
//! Four facts about that surface are settled by something other than "the two
//! sides matched", and CLAUDE.md wants each of them pinned by an expected-value
//! test that names BOTH numbers:
//!
//! * the ckt7 hourly **`kvarh`** column — the near-cancellation where no engine
//!   computes an independent value at all, carried by two
//!   `tests/corpus/ledger.json` rows
//!   ([`the_ckt7_hourly_kvarh_is_a_cross_engine_indeterminate`] and its drive
//!   [`the_ckt7_kvarh_scope_masks_that_column_and_nothing_else`], coordinator
//!   decision **D44(1)**);
//! * the ckt7 **`Min LV Bus`** column — an argmin over a three-way tie, where
//!   each of the three names is a correct answer
//!   ([`the_ckt7_min_lv_bus_is_an_argmin_over_a_tie`] and
//!   [`the_ckt7_min_lv_bus_scope_masks_that_column_and_nothing_else`],
//!   **D42(2)(b)**, data confirmed by **D44(4)**);
//! * the **capture point** — the gate reads the tree where all three producers
//!   hold the LAST CLOSED demand-interval cycle, never the in-flight one
//!   ([`the_di_capture_reads_the_last_closed_cycle`], **D42(6)**);
//! * the class table's **exhaustiveness over the live population**
//!   ([`the_di_class_table_covers_every_column_of_every_live_di_file`],
//!   **D42(1)**): the five decks' 36 files, 1 454 classified columns and
//!   4 922 812 classified cells, re-derived from the port's own trees.
//!
//! Each pin drives the **gate-side** comparator (`harness::di::compare_di` and
//! `harness::di::classify_member`, the very code `corpus_gate::runner` calls)
//! and takes the port's side from the port itself: the five vendored corpus
//! decks are run, each in a fresh scratch copy (`harness::scratch`, RETRO_FIXES
//! RF-I00-01), through the gate's own `harness::run_files::RunFileProbe`, and
//! each tree is read exactly where the runner reads it — after the engine is
//! dropped, before the probe sweeps. Never `.inputs/` at runtime (CLAUDE.md).
//!
//! The ORACLE side of the two ckt7 pins is a literal, because the tree is
//! run-created: each oracle transport's own `CorpusGuard` sweeps it away at the
//! end of that run and there is nothing left here to read. Every oracle number
//! below was measured at the gate's own capture slot by the G1.10c R part
//! (`tmp/g110c/{capi,r4133}_files/ckt7/…`, extracted at full precision into
//! `tmp/g110c/f4_kvarh.json` and by `tmp/g110c/f4_extract2.py`) and re-driven
//! live by F3's full 526-case gate in both lanes; every PORT number is read live
//! on each run of this binary, so a port-side move reds here instead of ageing
//! into a comment.
//!
//! Upstream provenance of the surface: the fastdss harness this replaces walks
//! the same CSVs but SKIPS a file the other side does not have
//! (`.inputs/DSS-Python`, `origin/fastdss:tests/compare_outputs.py:412-421`) and
//! merely PRINTS a cell mismatch with its `raise` commented out (`:517-527`), so
//! nothing there fails a run — which is why the gate's version compares the file
//! set first and fails loudly on every difference.
//!
//! **Windows-only**, like the surface: its `r4133` channel is a Win64 DLL, so
//! `harness::di` and `harness::run_files` are declared under `#[cfg(windows)]`.
#![cfg(windows)]

mod harness;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::OnceLock;

use dss_core::exec::Dss;
use harness::di::{DiClass, DiTree, classify_member, compare_di};
use harness::run_files::RunFileProbe;
use harness::scratch::{self, ScratchCopy};
use harness::tol_for;

// ---------------------------------------------------------------------------
// The five live demand-interval cases, run once for the whole binary.
// ---------------------------------------------------------------------------

/// One live `compare_di` case: the manifest row's own deck, `post` block and
/// step count (`tests/corpus/manifests/solvable_now.json`, the five rows that
/// carry `"compare_di": true`), plus the demand-interval members the port writes
/// for it — the file SET this surface compares, pinned by name.
struct DiCase {
    label: &'static str,
    /// Path under `tests/corpus/`, i.e. `electricdss-tst/` + the manifest path.
    rel: &'static str,
    post: &'static [&'static str],
    n_steps: usize,
    files: &'static [&'static str],
}

/// The population coordinator decision **D42(3)** fixed at **5** cases
/// (`Run_Demo1.dss` issues `CloseDI` without ever enabling `DemandInterval` and
/// writes no tree; it is pinned DI-free by
/// `corpus_gate::scheduler::DI_FREE_CLOSEDI_CASE`, not here).
///
/// The member names are the normalized created-file names
/// (`dss_epri::guard::normalize_created_name`: `/`-joined, ASCII-case-folded,
/// case-directory-relative), so they carry the run-created `<CaseName>`
/// directory and the year the run was in — `di_yr_1` on 123Bus, `di_yr_0`
/// everywhere else.
const DI_CASES: [DiCase; 5] = [
    DiCase {
        label: "123bus_yearly",
        rel: "electricdss-tst/Version8/Distrib/IEEETestCases/123Bus/Run_YearlySim.dss",
        post: &[],
        n_steps: 1,
        files: &[
            "16nov2011/di_yr_1/di_overloads_1.csv",
            "16nov2011/di_yr_1/di_systemmeter_1.csv",
            "16nov2011/di_yr_1/di_totals_1.csv",
            "16nov2011/di_yr_1/di_voltexceptions_1.csv",
            "16nov2011/di_yr_1/energymetertotals_1.csv",
            "16nov2011/di_yr_1/feeder_1.csv",
            "16nov2011/di_yr_1/systemmeter_1.csv",
            "16nov2011/di_yr_1/totals_1.csv",
        ],
    },
    DiCase {
        label: "ckt5",
        rel: "electricdss-tst/Version8/Distrib/EPRITestCircuits/ckt5/Run_ckt5.dss",
        post: &["set mode=snapshot"],
        n_steps: 1,
        files: &[
            "ckt5-simplemeter/di_yr_0/di_systemmeter_1.csv",
            "ckt5-simplemeter/di_yr_0/di_totals_1.csv",
            "ckt5-simplemeter/di_yr_0/di_voltexceptions_1.csv",
            "ckt5-simplemeter/di_yr_0/energymetertotals_1.csv",
            "ckt5-simplemeter/di_yr_0/sub_1.csv",
            "ckt5-simplemeter/di_yr_0/systemmeter_1.csv",
            "ckt5-simplemeter/di_yr_0/totals_1.csv",
        ],
    },
    DiCase {
        label: "ckt7",
        rel: "electricdss-tst/Version8/Distrib/EPRITestCircuits/ckt7/RunDSS_ckt7.dss",
        post: &["set mode=snapshot"],
        n_steps: 1,
        files: &[
            "example_ckt7/di_yr_0/25607_1.csv",
            "example_ckt7/di_yr_0/di_systemmeter_1.csv",
            "example_ckt7/di_yr_0/di_totals_1.csv",
            "example_ckt7/di_yr_0/di_voltexceptions_1.csv",
            "example_ckt7/di_yr_0/energymetertotals_1.csv",
            "example_ckt7/di_yr_0/systemmeter_1.csv",
            "example_ckt7/di_yr_0/totals_1.csv",
        ],
    },
    DiCase {
        label: "stoctrl_peakshave",
        rel: "electricdss-tst/Version8/Distrib/Examples/StoCtrl_Current_PeakShave/master.dss",
        post: &[],
        n_steps: 1,
        files: &[
            "ckt7/di_yr_0/25607_1.csv",
            "ckt7/di_yr_0/di_overloads_1.csv",
            "ckt7/di_yr_0/di_systemmeter_1.csv",
            "ckt7/di_yr_0/di_totals_1.csv",
            "ckt7/di_yr_0/di_voltexceptions_1.csv",
            "ckt7/di_yr_0/energymetertotals_1.csv",
            "ckt7/di_yr_0/systemmeter_1.csv",
            "ckt7/di_yr_0/totals_1.csv",
        ],
    },
    DiCase {
        label: "stoctrl_seasontarget",
        rel: "electricdss-tst/Version8/Distrib/Examples/StoCtrl_SeasonTarget/Run_example.dss",
        post: &[],
        n_steps: 1,
        files: &[
            "ieee13nodecktmod/di_yr_0/di_overloads_1.csv",
            "ieee13nodecktmod/di_yr_0/di_systemmeter_1.csv",
            "ieee13nodecktmod/di_yr_0/di_totals_1.csv",
            "ieee13nodecktmod/di_yr_0/energymetertotals_1.csv",
            "ieee13nodecktmod/di_yr_0/systemmeter_1.csv",
            "ieee13nodecktmod/di_yr_0/totals_1.csv",
        ],
    },
];

/// ckt7's three tied service buses (the `Min LV Bus` argmin, D42(2)(b)).
const TIE_BUSES: [&str; 3] = ["s1x_1001577", "s2x_1001577", "s3x_1001577"];

/// What one pass over [`DI_CASES`] produced: each case's demand-interval tree,
/// read at the gate's own capture slot, plus the per-node per-unit magnitudes of
/// ckt7's three tied buses at the port's own final state.
struct PortRun {
    trees: BTreeMap<&'static str, DiTree>,
    ckt7_tie: Vec<(String, Vec<f64>)>,
    /// The same three buses at the operating point the capi probe used for
    /// [`TIE_MAGNITUDES_HEX`] — `compile Master_ckt7.dss` + one `solve`
    /// (`tmp/g110c/probe_tie.py`), so the two sides of that comparison are the
    /// same feeder at the same point (audit findings AC-6 / AT-5).
    ckt7_snapshot_tie: Vec<(String, Vec<f64>)>,
}

static PORT: OnceLock<PortRun> = OnceLock::new();

/// The five decks, run ONCE per binary — three of them are yearly runs over
/// EPRI feeders, so every test in this file shares the one pass.
fn port() -> &'static PortRun {
    PORT.get_or_init(run_di_cases)
}

/// The vendored corpus path of a deck, exactly as the corpus gate addresses it
/// (`corpus_gate::scheduler`'s `abs`): the deck FILE, whose parent is the case
/// directory the probe snapshots.
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

/// Run the five decks the way `corpus_gate::runner` does — `clear` → `compile` →
/// the manifest's `post` block → one `solve` per checkpoint — inside the gate's
/// own run-file probe, and read each tree where the runner reads it: after the
/// engine is DROPPED (so every demand-interval stream the run closed is on disk;
/// `crates/dss-core/src/solution/meters/demand_interval.rs:515-570`
/// `close_all_di_files`) and before `RunFileProbe::finish_and_clean`, which
/// sweeps the tree away. Each deck runs in its own fresh scratch copy
/// (`harness::scratch`, RETRO_FIXES RF-I00-01), removed after the sweep; the
/// vendored corpus is never written.
fn run_di_cases() -> PortRun {
    let mut trees = BTreeMap::new();
    let mut ckt7_tie = Vec::new();
    for case in &DI_CASES {
        let copy = ScratchCopy::new(&corpus_deck(case.rel).to_string_lossy(), scratch::PORT);
        let deck = PathBuf::from(copy.deck());
        let ctx = format!("di_pins:{}", case.label);
        let probe = RunFileProbe::start(&deck.to_string_lossy());
        let mut dss = Dss::new();
        dss.command("clear");
        dss.command(&format!("compile \"{}\"", deck.display()));
        for cmd in case.post {
            dss.command(cmd);
        }
        assert!(
            dss.circuit().is_some(),
            "{ctx}: `compile` left the port with no active circuit ({:?})",
            dss.errors()
        );
        for _ in 0..case.n_steps {
            dss.command("solve");
        }
        if case.label == "ckt7" {
            for bus in TIE_BUSES {
                let v = dss
                    .bus_voltages(bus)
                    .unwrap_or_else(|| panic!("{ctx}: bus {bus} is not in the circuit"));
                ckt7_tie.push((
                    bus.to_string(),
                    v.pu_vmag_angle.iter().map(|(mag, _)| *mag).collect(),
                ));
            }
        }
        // The engine goes first, exactly as in `corpus_gate::runner`: a stream it
        // still held would not be on disk for the read below.
        drop(dss);
        let tree = probe.di_tree(&ctx);
        probe.finish_and_clean(&ctx, false);
        copy.finish();
        trees.insert(case.label, tree);
    }
    PortRun {
        trees,
        ckt7_tie,
        ckt7_snapshot_tie: ckt7_snapshot_tie(),
    }
}

/// Repeat, on the port, the probe that measured [`TIE_MAGNITUDES_HEX`] on
/// `capi_v0145`: `clear` → `compile Master_ckt7.dss` → one `solve` → the three
/// tied buses' per-node per-unit magnitudes (`tmp/g110c/probe_tie.py`).
///
/// The yearly run above ends at hour 8 760 of the loadshape; the oracle triple
/// is a snapshot of the same feeder at its own point, and the two are NOT the
/// same state. Reading the port at the probe's point is what makes
/// [`the_ckt7_min_lv_bus_is_an_argmin_over_a_tie`]'s oracle comparison a
/// measurement at the case's calibrated voltage class instead of a hand-chosen
/// cross-snapshot band (audit findings AC-6 / AT-5).
///
/// Its own scratch copy and run-file bracket: the compile writes the same
/// droppings every other run of this deck does, and they are swept here
/// exactly as the gate sweeps them.
fn ckt7_snapshot_tie() -> Vec<(String, Vec<f64>)> {
    let copy = ScratchCopy::new(
        &corpus_deck("electricdss-tst/Version8/Distrib/EPRITestCircuits/ckt7/Master_ckt7.dss")
            .to_string_lossy(),
        scratch::PORT,
    );
    let deck = PathBuf::from(copy.deck());
    let ctx = "di_pins:ckt7-snapshot";
    let probe = RunFileProbe::start(&deck.to_string_lossy());
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!("compile \"{}\"", deck.display()));
    dss.command("solve");
    assert!(
        dss.circuit().is_some(),
        "{ctx}: `compile` left the port with no active circuit ({:?})",
        dss.errors()
    );
    let tie = TIE_BUSES
        .iter()
        .map(|bus| {
            let v = dss
                .bus_voltages(bus)
                .unwrap_or_else(|| panic!("{ctx}: bus {bus} is not in the circuit"));
            (
                (*bus).to_string(),
                v.pu_vmag_angle.iter().map(|(mag, _)| *mag).collect(),
            )
        })
        .collect();
    drop(dss);
    probe.finish_and_clean(ctx, false);
    copy.finish();
    tie
}

/// One case's tree, or a panic naming the case.
fn tree_of(label: &str) -> &'static DiTree {
    port()
        .trees
        .get(label)
        .unwrap_or_else(|| panic!("{label} is not one of the five `compare_di` cases"))
}

/// The one-file tree a drive runs over: the comparator compares the file SET
/// first, so both sides carry the same single member and every red below can
/// only come from the column under test.
fn one_file(tree: &DiTree, name: &str) -> DiTree {
    let text = tree
        .get(name)
        .unwrap_or_else(|| panic!("{name} is not in this case's tree: {:?}", tree.keys()));
    [(name.to_string(), text.clone())].into_iter().collect()
}

// ---------------------------------------------------------------------------
// Small CSV helpers — the same shape the comparator reads (fields trimmed and
// dequoted, blank lines dropped). They never choose a BAND: every band in this
// file comes from `harness::tol_for`.
// ---------------------------------------------------------------------------

fn lines_of(text: &str) -> Vec<&str> {
    text.lines().filter(|l| !l.trim().is_empty()).collect()
}

fn fields(line: &str) -> Vec<String> {
    line.split(',')
        .map(|c| c.trim().trim_matches('"').trim().to_string())
        .collect()
}

/// The 0-based data-row index of the row whose index column says `time`.
///
/// The ledger and this file spell a row by the file's own `Time` / `Hour` value;
/// the comparator spells the same row `row N-1` (0-based over the data rows).
/// Both conventions appear in the panic texts, so this states the identity
/// instead of leaving two numbering schemes to drift apart.
fn row_of_time(rows: &[&str], time: usize, ctx: &str) -> usize {
    let idx = time - 1;
    let row = rows
        .get(idx + 1)
        .unwrap_or_else(|| panic!("{ctx}: no data row {idx} (Time {time})"));
    let spelled = fields(row)[0].clone();
    assert_eq!(
        spelled,
        time.to_string(),
        "{ctx}: data row {idx} spells its index column {spelled:?}, not Time {time} \
         — the ledger's `row N`, this file's `Time N` and the comparator's 0-based \
         `row N-1` are the same row, and that identity must hold"
    );
    idx
}

fn column_of(rows: &[&str], name: &str, ctx: &str) -> usize {
    let header = fields(rows[0]);
    header
        .iter()
        .position(|c| c.eq_ignore_ascii_case(name))
        .unwrap_or_else(|| panic!("{ctx}: no column {name:?} in {header:?}"))
}

fn cell(rows: &[&str], row: usize, col: usize) -> String {
    fields(rows[row + 1])[col].clone()
}

fn number(rows: &[&str], row: usize, col: usize, ctx: &str) -> f64 {
    let raw = cell(rows, row, col);
    raw.parse()
        .unwrap_or_else(|e| panic!("{ctx}: row {row} column {col} is {raw:?}, not a number: {e}"))
}

/// Rewrite one column in a single pass: `edit(data row index)` returns the new
/// cell, or `None` to keep the port's own. This is how the oracle side of every
/// drive below is built — from the port's own file, one column moved.
fn with_column(text: &str, col: usize, edit: impl Fn(usize) -> Option<String>) -> String {
    let mut out: Vec<String> = Vec::new();
    for (i, line) in lines_of(text).iter().enumerate() {
        match i.checked_sub(1).and_then(&edit) {
            Some(value) => {
                let mut f = fields(line);
                f[col] = value;
                out.push(f.join(", "));
            }
            None => out.push((*line).to_string()),
        }
    }
    out.join("\n") + "\n"
}

/// One cell replaced (the neighbour-column perturbations).
fn with_cell(text: &str, row: usize, col: usize, value: &str) -> String {
    with_column(text, col, |r| (r == row).then(|| value.to_string()))
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

/// A re-measured extreme must be the number this file records. NOT a comparison
/// band: `1e-9` relative is orders below every band in play and exists only so
/// that a last-bit difference in an f64 division cannot red a pin whose subject
/// is the value's magnitude.
#[track_caller]
fn assert_records(measured: f64, recorded: f64, what: &str) {
    assert!(
        (measured - recorded).abs() <= 1e-9 * recorded.abs(),
        "{what}: measured {measured:e}, but this pin records {recorded:e}. The \
         literals are the G1.10c capture (`tmp/g110c/f4_kvarh.json`, re-driven by \
         F3's live gate); if the port moved, triage the move — never re-spell the \
         literal to match it."
    );
}

// ---------------------------------------------------------------------------
// The ledger scopes the drives run under.
// ---------------------------------------------------------------------------

/// One `tests/corpus/ledger.json` entry, checked to be the G1.10c `di` exclusion
/// it claims to be, with its own `name_re` compiled into the predicate the
/// drives use.
///
/// The drives take the scope from the LEDGER rather than restating it: a
/// narrowed, widened or re-spelled scope reds here — in the pin that says the
/// exclusion is the right one — and not only in the live gate.
fn ledger_scope(id: &str, channel: &str) -> regex::Regex {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "corpus",
        "ledger.json",
    ]
    .iter()
    .collect();
    let text = std::fs::read_to_string(&path).expect("the ledger is readable");
    let doc: serde_json::Value = serde_json::from_str(&text).expect("the ledger is JSON");
    let entry = doc["entries"]
        .as_array()
        .expect("ledger.entries is an array")
        .iter()
        .find(|e| e["id"] == id)
        .unwrap_or_else(|| panic!("ledger.json carries no entry `{id}`"));
    assert_eq!(
        entry["case"], "solvable_now:Version8/Distrib/EPRITestCircuits/ckt7/RunDSS_ckt7.dss",
        "`{id}` must scope the ckt7 case"
    );
    assert_eq!(entry["channel"], channel, "`{id}` channel");
    assert_eq!(entry["kind"], "exclusion", "`{id}` kind");
    let m = &entry["match"].as_array().expect("`match` is an array")[0];
    assert_eq!(m["field"], "di", "`{id}` field");
    let re = m["name_re"].as_str().expect("`name_re` is a string");
    regex::Regex::new(re).unwrap_or_else(|e| panic!("`{id}` name_re {re:?}: {e}"))
}

fn nothing_excluded(_: &str) -> bool {
    false
}

/// **The pins measure the state the GATE measures** — [`DI_CASES`]' run recipe
/// and the tier the pins band at are read back out of
/// `tests/corpus/manifests/solvable_now.json` instead of being trusted
/// (G1.10c audit settlement, findings AT2-6 and AC2-11).
///
/// Two ways this could rot silently without it: a manifest edit to `post` or
/// `n_steps` would leave the pins running the case at a DIFFERENT operating
/// point than the gate while both stay green; and the two ckt7 exclusion pins
/// band at `tol_for("large")` by hand, which is only the gate's own band
/// (`runner.rs`, `tol_for(&c.kind)`) while that row stays `kind=large`.
#[test]
fn the_di_pin_cases_are_the_manifest_rows() {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "corpus",
        "manifests",
        "solvable_now.json",
    ]
    .iter()
    .collect();
    let text = std::fs::read_to_string(&path).expect("the manifest is readable");
    let doc: serde_json::Value = serde_json::from_str(&text).expect("the manifest is JSON");
    let rows = doc["cases"].as_array().expect("cases is an array");
    for case in DI_CASES {
        let want_path = case
            .rel
            .strip_prefix("electricdss-tst/")
            .expect("a corpus-relative deck path");
        let row = rows
            .iter()
            .find(|r| r["path"] == want_path)
            .unwrap_or_else(|| panic!("{}: no manifest row for {want_path}", case.label));
        assert_eq!(
            row["compare_di"], true,
            "{}: the pin runs a case the gate does not compare",
            case.label
        );
        let post: Vec<&str> = row["post"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|c| c.as_str().expect("a post command is a string"))
                    .collect()
            })
            .unwrap_or_default();
        assert_eq!(
            post, case.post,
            "{}: the manifest's `post` moved — the pin would measure another state \
             than the gate",
            case.label
        );
        assert_eq!(
            row["n_steps"].as_u64().unwrap_or(1) as usize,
            case.n_steps,
            "{}: the manifest's `n_steps` moved",
            case.label
        );
        if case.label == "ckt7" {
            assert_eq!(
                row["kind"], "large",
                "the two ckt7 exclusion pins band at `tol_for(\"large\")`; the gate \
                 bands the same cells at `tol_for(&c.kind)`, so the two agree only \
                 while this row is `large`"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// (1) the ckt7 hourly `kvarh` column — D44(1).
// ---------------------------------------------------------------------------

/// The two ckt7 files that carry the meter's registers. One meter, so the totals
/// file mirrors the meter's own file — asserted below, not assumed.
const KVARH_FILES: [&str; 2] = [
    "example_ckt7/di_yr_0/di_totals_1.csv",
    "example_ckt7/di_yr_0/25607_1.csv",
];

/// Every `kvarh` cell of `Example_ckt7/DI_yr_0/{DI_Totals,25607}_1.csv` that
/// falls outside the energy class on at least one channel: `(Time, capi_v0145,
/// r4133)`, spelled exactly as the two oracle transports wrote them at the
/// gate's own capture slot (`tmp/g110c/f4_kvarh.json`, extracted from
/// `tmp/g110c/{capi,r4133}_files/ckt7/`).
///
/// **20** of these fail on `capi_v0145` and **17** on `r4133` (13 on both), out
/// of 8 760 rows × 68 columns per file. The port's own value at each row is read
/// LIVE by every test below.
const CKT7_KVARH: [(usize, &str, &str); 24] = [
    (352, "7.2086109009058", "7.20876314957885"),
    (358, "-25.872855023612", "-25.8736491312523"),
    (359, "1.01464560883437", "1.01383583996829"),
    (382, "17.4724395818817", "17.4717169597416"),
    (475, "-16.928618184772", "-16.9294236051611"),
    (476, "-30.3616981892595", "-30.3627444443441"),
    (477, "0.41082760865509", "0.410388521398359"),
    (478, "-76.7673488074516", "-76.7666439852964"),
    (663, "-19.0732986463458", "-19.0739946810349"),
    (827, "-7.43359304228411", "-7.43237935088319"),
    (850, "15.6128546579935", "15.614912445168"),
    (852, "-3.84347342402491", "-3.84344898661543"),
    (1852, "5.48611914286416", "5.48632835526919"),
    (2595, "28.7157902799799", "28.716559059612"),
    (2596, "15.4565891914264", "15.4591024168413"),
    (2618, "20.6968844151901", "20.6959413421749"),
    (2644, "39.1241579717438", "39.1238084469115"),
    (2645, "9.32999723038053", "9.33066168030625"),
    (2668, "22.358460401834", "22.3564319664532"),
    (2692, "37.7754914698442", "37.7764365829655"),
    (7034, "42.7347493282075", "42.736078337089"),
    (7036, "32.2038548849959", "32.2041704055116"),
    (7395, "47.1492128889898", "47.1475009707407"),
    (8517, "-4.31851182407062", "-4.31769587381059"),
];

/// The three rows the four ledger entries quote, `(Time, port, capi_v0145,
/// r4133)`. The port half is asserted against the LIVE file.
const CKT7_KVARH_TRIPLE: [(usize, &str, &str, &str); 3] = [
    (
        352,
        "7.21266169258719",
        "7.2086109009058",
        "7.20876314957885",
    ),
    (
        477,
        "0.412764282391174",
        "0.41082760865509",
        "0.410388521398359",
    ),
    (
        478,
        "-76.7593383203749",
        "-76.7673488074516",
        "-76.7666439852964",
    ),
];

/// (a) the net reactive energy of an excluded cell, over all three producers:
/// `|kvarh| ≤ 76.7593383203749` on the port (Time 478) and `≤ 76.7673488074516`
/// including the two oracles.
const KVARH_ABS_MAX: f64 = 76.7674;
/// (a) the real-energy register of the SAME row — the uncancelled twin of the
/// same hour's integration. Measured minimum `2106.09962738186` (Time 1852).
const KWH_MIN: f64 = 2106.0996;
/// (a) `kWh / |kvarh|` over the 24 cells: the summands of the hourly reactive
/// integration are this many times the net they cancel to.
///
/// D44(1)(a) writes the range as "57x … 11 000x": **57.24** is the minimum over
/// the 20 cells that fail on `capi_v0145` (Time 478); over ALL 24 excluded cells
/// — what this pin asserts — it is **46.376** (Time 7395, an `r4133`-only cell).
/// The wider, measured range is the one recorded.
const SUMMAND_RATIO_MIN: f64 = 46.37619368540396;
const SUMMAND_RATIO_MAX: f64 = 11031.672192265261;
/// (b) the port's own absolute gap against either oracle. Measured maximum
/// `8.010487076703043e-3` kvarh (Time 478 vs `capi_v0145`).
const PORT_GAP_MAX: f64 = 8.010487076703043e-3;
/// (b) that gap as a fraction of the energy band the SAME row's `kWh` earns
/// (`energy_abs + energy_rel·|kWh|` — the accumulator's own class on its
/// uncancelled twin). Measured maximum `0.02301158377516905`, over bands never
/// narrower than `0.210709962738186`.
const GAP_OVER_KWH_BAND_MAX: f64 = 0.02301158377516905;
const KWH_BAND_MIN: f64 = 0.210709962738186;
/// (b) the same gap as a fraction of the row's own `Max kVA` — the `i_rel` scale
/// of the powers the hour integrates. Measured `5.492658891436715e-8` …
/// `1.6014062985305886e-6`.
const GAP_OVER_MAXKVA_MIN: f64 = 5.492658891436715e-8;
const GAP_OVER_MAXKVA_MAX: f64 = 1.6014062985305886e-6;
/// (b) …so `i_rel(large)·Max kVA` bounds the gap on 22 of the 24 cells and is
/// exceeded on two, by at most `1.6014062985305888×` (Time 478 vs `capi_v0145`;
/// the `r4133` side of that row is `1.4605026770812282×` and Time 475's `r4133`
/// side `1.0457771659642237×`). D42(2) asserted that bound universally; D44(1)
/// withdrew it as measured false and records the excess instead.
const IREL_EXCESS_TIMES: [usize; 2] = [475, 478];
const IREL_EXCESS_WORST: f64 = 1.6014062985305888;
/// (c) the two KLU oracles' disagreement with EACH OTHER on the same column:
/// `2.4437409479993732e-5` … `2.513225414901399e-3` kvarh, i.e.
/// `2.711183780275688 %` … `88.80252369020646 %` of the port's own gap on the
/// same cell.
const ORACLE_GAP_MIN: f64 = 2.4437409479993732e-5;
const ORACLE_GAP_MAX: f64 = 2.513225414901399e-3;
const ORACLE_FRACTION_MIN: f64 = 0.02711183780275688;
const ORACLE_FRACTION_MAX: f64 = 0.8880252369020646;
/// (c) …and on **6** of the 24 cells that disagreement exceeds the energy class
/// outright, worst `4.021027186072549×` at Time 359 (`capi_v0145`
/// `1.01464560883437` vs `r4133` `1.01383583996829`). D42(2) asserted this of
/// EVERY excluded cell; D44(1) withdrew that too — the measured six are named.
const ORACLE_CLASS_FAIL_TIMES: [usize; 6] = [359, 477, 827, 850, 2596, 8517];
const ORACLE_WORST_RATIO: f64 = 4.021027186072549;
/// The per-channel cell counts the two ledger entries carry (`measured.cells`).
const KVARH_CLASS_FAILS: (usize, usize) = (20, 17);

/// The WHOLE masked column on the port's own side, measured 2026-09-12 from the
/// live tree (audit finding AT-1 — see
/// [`the_masked_kvarh_column_is_pinned_whole`]): 8 760 rows, their annual net
/// and absolute sums (left-to-right `f64`, the file's own row order), the two
/// extremes with the Time that carries them, and two censuses — the rows inside
/// the cancellation regime the exclusion rests on ([`KVARH_ABS_MAX`]) and the
/// rows whose hour nets out capacitive.
const KVARH_COLUMN_ROWS: usize = 8760;
const KVARH_COLUMN_SUM: f64 = 4432951.126747186;
const KVARH_COLUMN_SUM_ABS: f64 = 4861124.562446087;
const KVARH_COLUMN_MAX: (usize, f64) = (4190, 1508.08783575459);
const KVARH_COLUMN_MIN: (usize, f64) = (8665, -825.561843850456);
const KVARH_CANCELLING_ROWS: usize = 118;
const KVARH_NEGATIVE_ROWS: usize = 581;

/// **`Example_ckt7/DI_yr_0/{DI_Totals,25607}_1.csv:kvarh` — no engine computes
/// an independent value there** (`tests/corpus/ledger.json`
/// `di-ckt7-hourly-kvarh-cancels-{capi,r4133}`, cause
/// `di-hourly-reactive-energy-cancels`; coordinator decisions **D42(2)(a)** and
/// **D44(1)**).
///
/// The EnergyMeter integrates every zone element's kvar over the hour (r4133
/// `Version8/Source/Meters/EnergyMeter.pas:1267` `TakeSample` → `:1324`
/// `Integrate(Reg_kvarh, S_Local.im, Delta_Hrs)`, `Integrate` itself at
/// `:1247`, the zone's own kvar summed in `Accumulate_Load` `:2247-2318`
/// (`:2264`); written out by `WriteDemandIntervalData` `:2990`, the hourly
/// register row at `:3001-3003`; the port
/// `crates/dss-core/src/solution/meters/demand_interval.rs`). Citation
/// corrected by the G1.10c audit settlement, finding AT2-1: the range that
/// stood here was `Accumulate_Load`'s body under `TakeSample`'s name, and its
/// `:2943-2949` was the **dss_capi 0.14.5** `WriteDemandIntervalData` (capi
/// `src/Meters/EnergyMeter.pas:2944`) attributed to r4133. On ckt7 the
/// capacitive and inductive halves of the feeder very nearly cancel, and the
/// three claims below are the decomposition CLAUDE.md asks for — measured, not
/// argued, over ALL 24 excluded cells:
///
/// * **(a) the cancellation.** `|kvarh| ≤ 76.7594` on the port's side of every
///   one of them (`≤ 76.7674` including the oracles) while the same row's
///   real-energy register is `kWh ≥ 2106.0996`: the summands are `46.376×` …
///   `11031.672×` the net they add up to.
/// * **(b) the residual is inside the accumulator's own class on its uncancelled
///   twin.** The port's gap is at most `8.010487076703043e-3` kvarh — at most
///   `0.02301158` of the energy band the same row's `kWh` earns (a band never
///   narrower than `0.2107`), equivalently `5.4927e-8` … `1.6014e-6` of the
///   row's own `Max kVA`, the `i_rel(large) = 1e-6` scale of the powers being
///   integrated, exceeded by at most `1.6014×` on two of the 24 cells.
/// * **(c) no engine-independent value exists at that scale.** The two KLU
///   oracles disagree with EACH OTHER by `2.44e-5` … `2.51e-3` kvarh —
///   `2.7 %` … `88.8 %` of the port's own gap — and exceed the energy class
///   between themselves on 6 of the 24 cells, worst `4.021×` at Time 359. The
///   port's faer solution only scales an indeterminacy the two oracles already
///   have.
///
/// Both numbers, at the three rows the ledger entries quote
/// ([`CKT7_KVARH_TRIPLE`]): Time 352 port `7.21266169258719` vs capi
/// `7.2086109009058` / r4133 `7.20876314957885`; Time 477 port
/// `0.412764282391174` vs `0.41082760865509` / `0.410388521398359`; Time 478
/// port `-76.7593383203749` vs `-76.7673488074516` / `-76.7666439852964`.
///
/// The exclusion reaches that column of those two files and nothing else —
/// [`the_ckt7_kvarh_scope_masks_that_column_and_nothing_else`] drives the real
/// comparator under the real ledger scope and shows it.
#[test]
fn the_ckt7_hourly_kvarh_is_a_cross_engine_indeterminate() {
    let tree = tree_of("ckt7");
    let tol = tol_for("large");
    let ctx = "ckt7 kvarh";

    // One meter: the totals file mirrors the meter's own file on this column.
    let meter = lines_of(&tree[KVARH_FILES[1]]);
    let totals = lines_of(&tree[KVARH_FILES[0]]);
    let jk = column_of(&meter, "kvarh", ctx);
    let jk_t = column_of(&totals, "kvarh", ctx);
    for (time, _, _) in CKT7_KVARH {
        let r = row_of_time(&meter, time, ctx);
        assert_eq!(
            cell(&meter, r, jk),
            cell(&totals, r, jk_t),
            "{ctx}: Time {time} — the meter file and the totals file must carry the \
             same `kvarh` (ckt7 has ONE EnergyMeter, so `DI_Totals` is its mirror)"
        );
    }

    let jw = column_of(&meter, "kWh", ctx);
    let jmk = column_of(&meter, "Max kVA", ctx);

    // The three rows the ledger quotes, port side read live.
    for (time, port, capi, r4133) in CKT7_KVARH_TRIPLE {
        let r = row_of_time(&meter, time, ctx);
        let live = cell(&meter, r, jk);
        assert_eq!(
            live.parse::<f64>().expect("a number"),
            port.parse::<f64>().expect("a number"),
            "{ctx}: Time {time} — the port now writes {live}, this pin records \
             {port} (oracles: capi {capi}, r4133 {r4133})"
        );
    }

    let (mut kvarh_abs, mut kwh_min, mut band_min) = (0.0_f64, f64::MAX, f64::MAX);
    let (mut ratio_min, mut ratio_max) = (f64::MAX, 0.0_f64);
    let (mut gap_max, mut gap_band_max) = (0.0_f64, 0.0_f64);
    let (mut gap_kva_min, mut gap_kva_max) = (f64::MAX, 0.0_f64);
    let (mut o_gap_min, mut o_gap_max) = (f64::MAX, 0.0_f64);
    let (mut o_frac_min, mut o_frac_max) = (f64::MAX, 0.0_f64);
    let (mut o_worst, mut o_worst_time) = (0.0_f64, 0usize);
    let (mut irel_excess, mut irel_worst) = (Vec::new(), 0.0_f64);
    let mut oracle_class_fails = Vec::new();
    let mut fails = (0usize, 0usize);

    for (time, capi_s, r4133_s) in CKT7_KVARH {
        let r = row_of_time(&meter, time, ctx);
        let port = number(&meter, r, jk, ctx);
        let kwh = number(&meter, r, jw, ctx);
        let max_kva = number(&meter, r, jmk, ctx);
        let capi: f64 = capi_s.parse().expect("a number");
        let r4133: f64 = r4133_s.parse().expect("a number");

        // (a) the cancellation.
        kvarh_abs = kvarh_abs.max(port.abs()).max(capi.abs()).max(r4133.abs());
        kwh_min = kwh_min.min(kwh);
        let ratio = kwh.abs() / port.abs();
        ratio_min = ratio_min.min(ratio);
        ratio_max = ratio_max.max(ratio);

        // (b) the residual against the SAME row's uncancelled twin.
        let band = tol.energy_abs + tol.energy_rel * kwh.abs();
        band_min = band_min.min(band);
        let mut excess_here = false;
        for (oracle, capi_side) in [(capi, true), (r4133, false)] {
            let gap = (port - oracle).abs();
            gap_max = gap_max.max(gap);
            gap_band_max = gap_band_max.max(gap / band);
            gap_kva_min = gap_kva_min.min(gap / max_kva);
            gap_kva_max = gap_kva_max.max(gap / max_kva);
            let excess = gap / (tol.i_rel * max_kva);
            if excess > 1.0 {
                excess_here = true;
                irel_worst = irel_worst.max(excess);
            }
            if gap > tol.energy_abs + tol.energy_rel * oracle.abs() {
                if capi_side {
                    fails.0 += 1;
                } else {
                    fails.1 += 1;
                }
            }
        }
        if excess_here {
            irel_excess.push(time);
        }

        // (c) the two oracles against each other.
        let o_gap = (capi - r4133).abs();
        let port_gap = (port - capi).abs().max((port - r4133).abs());
        o_gap_min = o_gap_min.min(o_gap);
        o_gap_max = o_gap_max.max(o_gap);
        o_frac_min = o_frac_min.min(o_gap / port_gap);
        o_frac_max = o_frac_max.max(o_gap / port_gap);
        let o_ratio = o_gap / (tol.energy_abs + tol.energy_rel * r4133.abs());
        if o_ratio > 1.0 {
            oracle_class_fails.push(time);
            if o_ratio > o_worst {
                o_worst = o_ratio;
                o_worst_time = time;
            }
        }
    }

    // (a)
    assert!(
        kvarh_abs <= KVARH_ABS_MAX,
        "{ctx}: (a) an excluded cell's |kvarh| is {kvarh_abs}, over the recorded \
         {KVARH_ABS_MAX} — the cancellation this exclusion rests on has changed"
    );
    assert!(
        kwh_min >= KWH_MIN,
        "{ctx}: (a) an excluded row's kWh is {kwh_min}, under the recorded {KWH_MIN}"
    );
    assert_records(ratio_min, SUMMAND_RATIO_MIN, "(a) min kWh/|kvarh|");
    assert_records(ratio_max, SUMMAND_RATIO_MAX, "(a) max kWh/|kvarh|");

    // (b)
    assert!(
        gap_max <= PORT_GAP_MAX * (1.0 + 1e-9),
        "{ctx}: (b) the port's worst gap is {gap_max:e}, over the recorded \
         {PORT_GAP_MAX:e}"
    );
    assert_records(gap_max, PORT_GAP_MAX, "(b) worst port gap");
    assert_records(
        gap_band_max,
        GAP_OVER_KWH_BAND_MAX,
        "(b) worst gap / kWh band",
    );
    assert_records(band_min, KWH_BAND_MIN, "(b) narrowest kWh band");
    assert_records(gap_kva_min, GAP_OVER_MAXKVA_MIN, "(b) min gap / Max kVA");
    assert_records(gap_kva_max, GAP_OVER_MAXKVA_MAX, "(b) max gap / Max kVA");
    assert_eq!(
        irel_excess, IREL_EXCESS_TIMES,
        "{ctx}: (b) `i_rel(large)·Max kVA` is exceeded at Times {irel_excess:?}, \
         recorded {IREL_EXCESS_TIMES:?} — D44(1) withdrew D42(2)'s universal \
         precisely because these two exist; a move either way is a measurement, \
         never a re-spelling"
    );
    assert_records(irel_worst, IREL_EXCESS_WORST, "(b) worst i_rel excess");

    // (c)
    assert_records(o_gap_min, ORACLE_GAP_MIN, "(c) min |capi - r4133|");
    assert_records(o_gap_max, ORACLE_GAP_MAX, "(c) max |capi - r4133|");
    assert_records(
        o_frac_min,
        ORACLE_FRACTION_MIN,
        "(c) min oracle gap / port gap",
    );
    assert_records(
        o_frac_max,
        ORACLE_FRACTION_MAX,
        "(c) max oracle gap / port gap",
    );
    assert_eq!(
        oracle_class_fails, ORACLE_CLASS_FAIL_TIMES,
        "{ctx}: (c) the two oracles exceed the energy class between themselves at \
         Times {oracle_class_fails:?}, recorded {ORACLE_CLASS_FAIL_TIMES:?}"
    );
    assert_eq!(
        o_worst_time, 359,
        "{ctx}: (c) the worst oracle-vs-oracle cell"
    );
    assert_records(
        o_worst,
        ORACLE_WORST_RATIO,
        "(c) worst oracle-vs-oracle ratio",
    );
    assert_eq!(
        fails, KVARH_CLASS_FAILS,
        "{ctx}: the excluded cells fail the energy class on (capi_v0145, r4133) = \
         {fails:?}, recorded {KVARH_CLASS_FAILS:?} — the two ledger entries' \
         `measured.cells`"
    );
}

/// **(d), the comparator drive behind
/// [`the_ckt7_hourly_kvarh_is_a_cross_engine_indeterminate`]** (D44(1)(d)):
/// under the real ledger scope the 24 cells pass; without it they red naming
/// BOTH numbers; and the `kWh` / `Max kVA` columns of the SAME rows stay
/// compared either way.
///
/// The oracle side is the port's own file with the 24 `kvarh` cells replaced by
/// the captured oracle spellings, so every other cell is equal by construction
/// and each red below can only come from the column under test.
#[test]
fn the_ckt7_kvarh_scope_masks_that_column_and_nothing_else() {
    let tree = tree_of("ckt7");
    let tol = tol_for("large");
    let ctx = "ckt7 kvarh drive";
    let file = KVARH_FILES[1];
    let port = one_file(tree, file);

    for (channel, id, capi_side) in [
        ("capi_v0145", "di-ckt7-hourly-kvarh-cancels-capi", true),
        ("r4133", "di-ckt7-hourly-kvarh-cancels-r4133", false),
    ] {
        let re = ledger_scope(id, channel);
        let excluded = |key: &str| re.is_match(key);

        let rows = lines_of(&port[file]);
        let j = column_of(&rows, "kvarh", ctx);
        let moved: BTreeMap<usize, String> = CKT7_KVARH
            .iter()
            .map(|(time, capi, r4133)| {
                (
                    row_of_time(&rows, *time, ctx),
                    (if capi_side { capi } else { r4133 }).to_string(),
                )
            })
            .collect();
        let mut oracle: DiTree = port.clone();
        oracle.insert(
            file.to_string(),
            with_column(&port[file], j, |r| moved.get(&r).cloned()),
        );

        // With the scope: green.
        compare_di(
            channel,
            Some(&oracle),
            &port,
            &tol,
            &excluded,
            "di_pins:ckt7 kvarh scoped",
        );

        // Without it: red, naming both numbers.
        let msg = panic_message(|| {
            compare_di(
                channel,
                Some(&oracle),
                &port,
                &tol,
                &nothing_excluded,
                "di_pins:ckt7 kvarh unscoped",
            )
        });
        let (time, port_value, capi, r4133) = CKT7_KVARH_TRIPLE[0];
        let oracle_value = if capi_side { capi } else { r4133 };
        assert!(
            msg.contains(port_value) && msg.contains(oracle_value),
            "{ctx} [{channel}]: the unscoped comparison must name BOTH numbers of \
             Time {time} (port {port_value}, oracle {oracle_value}); it said:\n{msg}"
        );

        // The neighbours of the very same row are still compared: `kWh` at 2x its
        // energy band and `Max kVA` at 2x its current band each red WITH the
        // scope on.
        for (column, rel, abs) in [
            ("kWh", tol.energy_rel, tol.energy_abs),
            ("Max kVA", tol.i_rel, tol.i_abs),
        ] {
            let text = &oracle[file];
            let orows = lines_of(text);
            let jn = column_of(&orows, column, ctx);
            let r = row_of_time(&orows, time, ctx);
            let base = number(&orows, r, jn, ctx);
            let value = base + 2.0 * (abs + rel * base.abs());
            let mut poisoned: DiTree = oracle.clone();
            poisoned.insert(
                file.to_string(),
                with_cell(text, r, jn, &format!("{value}")),
            );
            let msg = panic_message(|| {
                compare_di(
                    channel,
                    Some(&poisoned),
                    &port,
                    &tol,
                    &excluded,
                    "di_pins:ckt7 kvarh neighbour",
                )
            });
            assert!(
                msg.contains(&format!("{value}")),
                "{ctx} [{channel}]: the `kvarh` scope must leave `{column}` of the \
                 same row compared — a two-band move at Time {time} passed:\n{msg}"
            );
        }
    }
}

/// **The masked column, end to end** — the exclusion above hides
/// `2 × 8 760` cells to admit 24, so the other 8 736 rows are pinned here
/// (GOLDEN_REBASE G1.10c audit finding **AT-1**).
///
/// **Why the ledger scope is not narrowed to the 24 cells instead.** Measured
/// 2026-09-12 over the R part's captures: the port-vs-oracle ratio across this
/// column is a CONTINUUM, not a set of 24 outliers — over the energy class on
/// `20 / 17` cells (capi / r4133), over half of it on `60 / 54`, over a quarter
/// on `193 / 192`, over a tenth on `968 / 963`, median `0.037`, and the largest
/// PASSING cell sits at `0.956 / 0.939` of the class. A `<file>:<column>:<row>`
/// key naming today's 20 and 17 would therefore red on the next last-bit move
/// in EITHER direction — a `0.956` cell crossing, or a listed row healing into
/// a dead selector — with nothing having regressed. The exclusion stays
/// column-scoped and the column is pinned instead.
///
/// What the pin asserts, live, off the port's own tree: the two files carry the
/// column identically on all 8 760 rows; its annual net and absolute sums, both
/// extremes with their Times, and the two censuses are the measured literals
/// ([`KVARH_COLUMN_SUM`] …); and every one of the 24 excluded cells lies inside
/// the cancellation regime the exclusion rests on. A regression on ANY row of
/// the column moves at least one of those numbers.
///
/// What still gates the same physics against the oracles on all 8 760 rows: the
/// SAME hourly reactive-energy integration at system level —
/// `Example_ckt7/DI_yr_0/DI_SystemMeter_1.csv:kvarh`, which is not excluded on
/// either channel and whose worst measured ratio is `0.0048` — plus the `kWh`,
/// `Zone kvarh` and `Max kVA` columns of these very files.
#[test]
fn the_masked_kvarh_column_is_pinned_whole() {
    let tree = tree_of("ckt7");
    let ctx = "ckt7 kvarh column";
    let meter = lines_of(&tree[KVARH_FILES[1]]);
    let totals = lines_of(&tree[KVARH_FILES[0]]);
    let jk = column_of(&meter, "kvarh", ctx);
    let jk_t = column_of(&totals, "kvarh", ctx);

    assert_eq!(
        (meter.len() - 1, totals.len() - 1),
        (KVARH_COLUMN_ROWS, KVARH_COLUMN_ROWS),
        "{ctx}: the two files hold {} and {} data rows, recorded {KVARH_COLUMN_ROWS} \
         each",
        meter.len() - 1,
        totals.len() - 1
    );

    let mut vals = Vec::with_capacity(KVARH_COLUMN_ROWS);
    for r in 0..KVARH_COLUMN_ROWS {
        // …so `vals[Time - 1]` below is the row the ledger and this file spell
        // `Time`, the same identity `row_of_time` states for the 24.
        assert_eq!(
            cell(&meter, r, 0),
            (r + 1).to_string(),
            "{ctx}: data row {r} spells its Time column {:?}",
            cell(&meter, r, 0)
        );
        assert_eq!(
            cell(&meter, r, jk),
            cell(&totals, r, jk_t),
            "{ctx}: row {r} — ckt7 has ONE EnergyMeter, so `DI_Totals` must mirror \
             the meter's own file on every row, not only on the 24 the exclusion \
             names"
        );
        vals.push(number(&meter, r, jk, ctx));
    }

    // Left-to-right over the file's own row order: expected VALUES of a
    // deterministic reduction, recorded through the file's own `assert_records`
    // identity (1e-9 relative — the epsilon every literal in this file is
    // recorded at, not a comparison band). Measured 2026-09-12 in the
    // settlement's own gate: the `oracle-parity` lane writes this column's last
    // printed digit differently on a handful of hours, so its annual net sum is
    // 4432951.126747187 against the default lane's 4432951.126747186 — ONE ulp
    // at 4.4e6 (2.1e-16 relative), the precision-compat kernels' own footprint.
    // The identity leaves 4.4e-3 kvarh of slack on this sum against the per-cell
    // energy band the gate compares this very column at
    // (`energy_abs + energy_rel·|e|`): 34.0x tighter at the largest hour
    // (0.1509 kvarh at |kvarh| = 1508.1), 12.3x at the column's median
    // (0.0545 at |kvarh| = 544.4) — but on 54 of the 8 760 cells, those with
    // |kvarh| < 43.3, the gate's own band is TIGHTER than this sum's slack
    // (round-2 audit finding SA-3; measured over the captured column). So the
    // sum reds an off-row regression on the overwhelming majority of the
    // column; on the smallest cancellation rows — the ones the exclusion is
    // about — the watchers are the two censuses below (a cell drifting across
    // the cancellation ceiling or changing sign moves a count) and the two
    // extremes with their Times, not this sum.
    let sum: f64 = vals.iter().sum();
    let sum_abs: f64 = vals.iter().map(|v| v.abs()).sum();
    assert_records(
        sum,
        KVARH_COLUMN_SUM,
        &format!(
            "{ctx}: the annual net reactive energy of the masked column — the \
             exclusion masks every cell of this column, so this sum (with the one \
             below) is what stands between an off-row regression and the gate"
        ),
    );
    assert_records(
        sum_abs,
        KVARH_COLUMN_SUM_ABS,
        &format!(
            "{ctx}: the annual SUM OF |kvarh| — the net sum alone cannot see two \
             rows moving in opposite directions"
        ),
    );

    let arg_max = (1..=KVARH_COLUMN_ROWS)
        .max_by(|a, b| vals[a - 1].abs().total_cmp(&vals[b - 1].abs()))
        .expect("a non-empty column");
    assert_eq!(
        arg_max,
        KVARH_COLUMN_MAX.0,
        "{ctx}: the column's largest |kvarh| is now the hour {arg_max} ({}), \
         recorded {:?}",
        vals[arg_max - 1],
        KVARH_COLUMN_MAX
    );
    assert_records(
        vals[arg_max - 1],
        KVARH_COLUMN_MAX.1,
        &format!("{ctx}: the column's largest |kvarh|, at Time {arg_max}"),
    );
    let arg_min = (1..=KVARH_COLUMN_ROWS)
        .min_by(|a, b| vals[a - 1].total_cmp(&vals[b - 1]))
        .expect("a non-empty column");
    assert_eq!(
        arg_min,
        KVARH_COLUMN_MIN.0,
        "{ctx}: the column's most negative kvarh is now the hour {arg_min} ({}), \
         recorded {:?}",
        vals[arg_min - 1],
        KVARH_COLUMN_MIN
    );
    assert_records(
        vals[arg_min - 1],
        KVARH_COLUMN_MIN.1,
        &format!("{ctx}: the column's most negative kvarh, at Time {arg_min}"),
    );

    let cancelling: BTreeSet<usize> = (1..=KVARH_COLUMN_ROWS)
        .filter(|t| vals[t - 1].abs() <= KVARH_ABS_MAX)
        .collect();
    assert_eq!(
        cancelling.len(),
        KVARH_CANCELLING_ROWS,
        "{ctx}: {} rows of the column are inside the cancellation regime \
         (|kvarh| ≤ {KVARH_ABS_MAX}), recorded {KVARH_CANCELLING_ROWS}",
        cancelling.len()
    );
    assert_eq!(
        vals.iter().filter(|v| **v < 0.0).count(),
        KVARH_NEGATIVE_ROWS,
        "{ctx}: the column nets out capacitive on {} hours, recorded \
         {KVARH_NEGATIVE_ROWS}",
        vals.iter().filter(|v| **v < 0.0).count()
    );

    let outside: Vec<usize> = CKT7_KVARH
        .iter()
        .map(|(time, _, _)| *time)
        .filter(|t| !cancelling.contains(t))
        .collect();
    assert!(
        outside.is_empty(),
        "{ctx}: the excluded cells at Times {outside:?} are no longer inside the \
         cancellation regime the exclusion rests on — the 24 masked cells must be \
         a subset of the {KVARH_CANCELLING_ROWS} rows where the hour's summands \
         cancel"
    );
}

// ---------------------------------------------------------------------------
// (2) the ckt7 `Min LV Bus` column — D42(2)(b), data confirmed by D44(4).
// ---------------------------------------------------------------------------

const VOLTEX_FILE: &str = "example_ckt7/di_yr_0/di_voltexceptions_1.csv";

/// The port's own argmin over the tie, measured: `s2x_1001577` on 6 941 rows,
/// `s3x_1001577` on 375, and `s1x_1001577` — the oracles' answer on every row —
/// on the remaining 1 444 of 8 760.
const MIN_LV_BUS_COUNTS: [(&str, usize); 3] = [
    ("s1x_1001577", 1444),
    ("s2x_1001577", 6941),
    ("s3x_1001577", 375),
];
/// 6 941 + 375: the rows where the port and both oracles name a different bus —
/// the number both ledger entries carry.
const MIN_LV_BUS_DIFFERING_ROWS: usize = 7316;
/// The tie itself on `capi_v0145`, recorded BUS BY BUS: each tied bus's three
/// per-unit node magnitudes, bit for bit, as `tmp/g110c/probe_tie.py` read them
/// (`Bus.puVmagAngle[0::2]` after `clear` → `compile Master_ckt7.dss` → one
/// `solve`; re-read 2026-09-12, `tmp/g110c/probe_tie_run2.txt`, identical).
///
/// The three rows are equal to each other — that equality IS the tie this
/// exclusion rests on, so it is recorded as three measurements and asserted
/// below, never summarized into one vector (audit finding AT2-5, settlement
/// round 2 SA-5).
const TIE_MAGNITUDES_HEX: [(&str, [&str; 3]); 3] = [
    (
        "s1x_1001577",
        [
            "0x1.c26d89b2b06a6p-1",
            "0x1.c5736ad8e99ddp-1",
            "0x1.c69388d539fa3p-1",
        ],
    ),
    (
        "s2x_1001577",
        [
            "0x1.c26d89b2b06a6p-1",
            "0x1.c5736ad8e99ddp-1",
            "0x1.c69388d539fa3p-1",
        ],
    ),
    (
        "s3x_1001577",
        [
            "0x1.c26d89b2b06a6p-1",
            "0x1.c5736ad8e99ddp-1",
            "0x1.c69388d539fa3p-1",
        ],
    ),
];

/// **`Example_ckt7/DI_yr_0/DI_VoltExceptions_1.csv:Min LV Bus` — an argmin over
/// a tie, where all three names are correct** (`tests/corpus/ledger.json`
/// `di-ckt7-min-lv-bus-argmin-tie-{capi,r4133}`, cause
/// `di-voltexception-argmin-tie`; coordinator decision **D42(2)(b)**, data
/// confirmed by **D44(4)**).
///
/// `TEnergyMeter.WriteVoltageReport` scans the buses in list order and keeps the
/// FIRST STRICT minimizer (r4133 `Version8/Source/Meters/EnergyMeter.pas:3620` …
/// `:3754`; the LV arm opens at `:3707` and its `If Vmagpu < underVmin` is at
/// `:3717`, the primary arm's at `:3658`), and the port's scan is faithful to it
/// line for line (`crates/dss-core/src/solution/meters/demand_interval.rs:866-906`
/// — same order, same strict `<`, same `> 0.1` neutral guard, same `kVBase`
/// split).
///
/// On ckt7 the three service buses `s1x_1001577` / `s2x_1001577` /
/// `s3x_1001577` carry the SAME node magnitudes, so the minimum is attained at
/// three buses at once and the last bit of the solve decides which one a strict
/// `<` scan reports. Both numbers: the port names `s2x_1001577` on **6 941**
/// rows and `s3x_1001577` on **375** where both oracles name `s1x_1001577` on
/// all **8 760** — 7 316 differing rows. The oracle side of the tie is
/// [`TIE_MAGNITUDES_HEX`], which records ALL THREE buses' magnitudes as capi
/// read them, so their bit-identity is asserted from that measurement here
/// instead of being claimed in prose (audit finding AT2-5 / round-2 SA-5); the
/// port's own three magnitudes are read LIVE here and agree
/// across the three buses inside the case's own voltage class, which is what
/// makes each of the three a correct answer — and the port is read a second
/// time at the probe's OWN operating point ([`ckt7_snapshot_tie`]) so that the
/// comparison against the oracle triple is state-for-state at that same class,
/// not the cross-snapshot 1 % band this pin used to close with (audit findings
/// AC-6 / AT-5).
///
/// The value column carries everything the name column loses:
/// [`the_ckt7_min_lv_bus_scope_masks_that_column_and_nothing_else`] shows `Min
/// LV Voltage` and `Max LV Bus` still compared under the scope (measured: 0
/// `Min LV Voltage` class failures on either channel, worst ratio `0.0191476`,
/// and `Min Bus` / `Max Bus` / `Max LV Bus` equal on every row).
#[test]
fn the_ckt7_min_lv_bus_is_an_argmin_over_a_tie() {
    let run = port();
    let tree = tree_of("ckt7");
    let ctx = "ckt7 min lv bus";
    let tol = tol_for("large");
    let rows = lines_of(&tree[VOLTEX_FILE]);

    // The port's own argmin, counted live.
    let j = column_of(&rows, "Min LV Bus", ctx);
    assert_eq!(
        rows.len() - 1,
        8760,
        "{ctx}: one row per hour of the yearly run"
    );
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for row in &rows[1..] {
        *counts
            .entry(fields(row)[j].to_ascii_lowercase())
            .or_default() += 1;
    }
    let measured: Vec<(&str, usize)> = counts.iter().map(|(k, v)| (k.as_str(), *v)).collect();
    assert_eq!(
        measured,
        MIN_LV_BUS_COUNTS.to_vec(),
        "{ctx}: the port's argmin over the tie moved. Both oracles report \
         s1x_1001577 on all 8760 rows, so {MIN_LV_BUS_DIFFERING_ROWS} rows differ \
         — the number the two ledger entries carry."
    );
    assert_eq!(
        MIN_LV_BUS_COUNTS[1].1 + MIN_LV_BUS_COUNTS[2].1,
        MIN_LV_BUS_DIFFERING_ROWS,
        "{ctx}: the differing-row count is the two non-oracle names' rows"
    );

    // The tie itself, read live from the port's own state.
    assert_eq!(run.ckt7_tie.len(), 3, "{ctx}: three service buses");
    let (first_name, first) = &run.ckt7_tie[0];
    for (name, mags) in &run.ckt7_tie {
        assert_eq!(
            mags.len(),
            first.len(),
            "{ctx}: {name} has {} nodes, {first_name} has {}",
            mags.len(),
            first.len()
        );
        for (k, (a, e)) in mags.iter().zip(first).enumerate() {
            assert!(
                (a - e).abs() <= tol.v_abs + tol.v_rel * e.abs(),
                "{ctx}: node {} of {name} is {a:e} pu and of {first_name} {e:e} pu — \
                 this exclusion rests on the three buses sharing ONE minimum, so a \
                 real difference here makes the argmin meaningful and the exclusion \
                 wrong",
                k + 1
            );
        }
    }
    // …and the oracle's own tie, at the same three buses AND at the same
    // operating point: `tmp/g110c/probe_tie.py` read capi after
    // `compile Master_ckt7.dss` + one `solve`, so the port is read there too
    // ([`ckt7_snapshot_tie`]) instead of holding the yearly run's last state
    // against a snapshot. Both sides are then the same state and the comparison
    // is the case's own calibrated voltage class — the 1 % cross-snapshot band
    // that stood here is gone (audit findings AC-6 / AT-5). The oracle side is
    // recorded per BUS, so its own tie is asserted from that measurement rather
    // than resting on prose (audit finding AT2-5, round-2 SA-5): the three rows
    // must name the three scanned buses and carry the SAME hex magnitudes.
    assert_eq!(
        TIE_MAGNITUDES_HEX.map(|(bus, _)| bus),
        TIE_BUSES,
        "{ctx}: the oracle table must record every tied bus, in the scan's order"
    );
    let (first_oracle_bus, oracle_first) = TIE_MAGNITUDES_HEX[0];
    for (bus, mags) in TIE_MAGNITUDES_HEX {
        assert_eq!(
            mags, oracle_first,
            "{ctx}: capi_v0145 read {bus} as {mags:?} and {first_oracle_bus} as \
             {oracle_first:?} — this exclusion rests on the oracle's three buses \
             carrying ONE bit-identical magnitude vector; were they ever to \
             differ, the oracle's argmin would be meaningful and the exclusion \
             wrong"
        );
    }
    assert_eq!(
        first.len(),
        oracle_first.len(),
        "{ctx}: capi_v0145 measured {} magnitudes per bus, the port has {}",
        oracle_first.len(),
        first.len()
    );
    assert_eq!(
        run.ckt7_snapshot_tie.len(),
        TIE_MAGNITUDES_HEX.len(),
        "{ctx}: the snapshot probe must read all three tied buses"
    );
    for ((name, mags), (oracle_bus, oracle_hex)) in
        run.ckt7_snapshot_tie.iter().zip(TIE_MAGNITUDES_HEX)
    {
        assert_eq!(
            name, oracle_bus,
            "{ctx}: the snapshot probe read {name} where capi_v0145 recorded \
             {oracle_bus} — the two sides are compared bus for bus"
        );
        let expect: Vec<f64> = oracle_hex.iter().map(|h| hex_f64(h)).collect();
        assert_eq!(
            mags.len(),
            expect.len(),
            "{ctx}: at the probe's point {name} has {} nodes, capi_v0145 measured \
             {}",
            mags.len(),
            expect.len()
        );
        for (k, (a, e)) in mags.iter().zip(&expect).enumerate() {
            assert!(
                (a - e).abs() <= tol.v_abs + tol.v_rel * e.abs(),
                "{ctx}: at the probe's point (compile + one solve) node {} of {name} \
                 is {a:e} pu in the port and {e:e} pu on capi_v0145 \
                 ({oracle_hex:?}) — the oracle's three buses carry ONE \
                 bit-identical magnitude vector, and the port must meet it at the \
                 voltage class for the tie, and therefore this exclusion, to be real",
                k + 1
            );
        }
    }
}

/// The drive behind [`the_ckt7_min_lv_bus_is_an_argmin_over_a_tie`]: with the
/// ledger scope the oracle's `s1x_1001577` passes on all 8 760 rows; without it
/// the comparison reds naming both names; and `Min LV Voltage` (the value
/// column) and `Max LV Bus` (the file's other LV name column) stay compared
/// under the scope.
#[test]
fn the_ckt7_min_lv_bus_scope_masks_that_column_and_nothing_else() {
    let tree = tree_of("ckt7");
    let tol = tol_for("large");
    let ctx = "ckt7 min lv bus drive";
    let port = one_file(tree, VOLTEX_FILE);
    let rows = lines_of(&port[VOLTEX_FILE]);
    let j = column_of(&rows, "Min LV Bus", ctx);
    let jv = column_of(&rows, "Min LV Voltage", ctx);
    let jb = column_of(&rows, "Max LV Bus", ctx);

    let mut oracle: DiTree = port.clone();
    oracle.insert(
        VOLTEX_FILE.to_string(),
        with_column(&port[VOLTEX_FILE], j, |_| {
            Some(MIN_LV_BUS_COUNTS[0].0.to_string())
        }),
    );

    for (channel, id) in [
        ("capi_v0145", "di-ckt7-min-lv-bus-argmin-tie-capi"),
        ("r4133", "di-ckt7-min-lv-bus-argmin-tie-r4133"),
    ] {
        let re = ledger_scope(id, channel);
        let excluded = |key: &str| re.is_match(key);

        compare_di(
            channel,
            Some(&oracle),
            &port,
            &tol,
            &excluded,
            "di_pins:ckt7 min lv bus scoped",
        );

        let msg = panic_message(|| {
            compare_di(
                channel,
                Some(&oracle),
                &port,
                &tol,
                &nothing_excluded,
                "di_pins:ckt7 min lv bus unscoped",
            )
        });
        assert!(
            msg.contains(MIN_LV_BUS_COUNTS[1].0) && msg.contains(MIN_LV_BUS_COUNTS[0].0),
            "{ctx} [{channel}]: the unscoped comparison must name both buses ({} vs \
             {}); it said:\n{msg}",
            MIN_LV_BUS_COUNTS[1].0,
            MIN_LV_BUS_COUNTS[0].0
        );

        // The value column of the same file, under the scope.
        let orows = lines_of(&oracle[VOLTEX_FILE]);
        let base = number(&orows, 0, jv, ctx);
        let value = base + 2.0 * (tol.v_abs + tol.v_rel * base.abs());
        let mut poisoned: DiTree = oracle.clone();
        poisoned.insert(
            VOLTEX_FILE.to_string(),
            with_cell(&oracle[VOLTEX_FILE], 0, jv, &format!("{value}")),
        );
        let msg = panic_message(|| {
            compare_di(
                channel,
                Some(&poisoned),
                &port,
                &tol,
                &excluded,
                "di_pins:ckt7 min lv voltage",
            )
        });
        assert!(
            msg.contains(&format!("{value}")),
            "{ctx} [{channel}]: `Min LV Voltage` must stay compared under the `Min \
             LV Bus` scope:\n{msg}"
        );

        // …and the file's other LV NAME column.
        let mut poisoned: DiTree = oracle.clone();
        poisoned.insert(
            VOLTEX_FILE.to_string(),
            with_cell(&oracle[VOLTEX_FILE], 0, jb, "not_a_bus"),
        );
        let msg = panic_message(|| {
            compare_di(
                channel,
                Some(&poisoned),
                &port,
                &tol,
                &excluded,
                "di_pins:ckt7 max lv bus",
            )
        });
        assert!(
            msg.contains("not_a_bus"),
            "{ctx} [{channel}]: `Max LV Bus` must stay compared under the `Min LV \
             Bus` scope:\n{msg}"
        );
    }
}

/// `0x1.…p-1` — the spelling Python's `float.hex()` produced for the oracle
/// magnitudes — as an f64.
fn hex_f64(s: &str) -> f64 {
    let (mantissa, exp) = s.split_once('p').expect("a hex float has an exponent");
    let exp: i32 = exp.parse().expect("the exponent is an integer");
    let body = mantissa
        .strip_prefix("0x")
        .expect("a hex float starts with 0x");
    let (int, frac) = body.split_once('.').unwrap_or((body, ""));
    let mut v = i64::from_str_radix(int, 16).expect("the integer part") as f64;
    for (k, c) in frac.chars().enumerate() {
        let d = f64::from(c.to_digit(16).expect("a hex digit"));
        v += d * 16f64.powi(-(k as i32 + 1));
    }
    v * 2f64.powi(exp)
}

// ---------------------------------------------------------------------------
// (3) the capture point — D42(6).
// ---------------------------------------------------------------------------

fn scratch_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("dss_di_pins_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap_or_else(|e| panic!("mkdir {}: {e}", d.display()));
    d
}

/// Run one `yearly(3) → closedi → yearly(5)` fixture through the gate's own
/// port-side producer and return its demand-interval tree, optionally closing
/// the second cycle before the capture.
fn di_cycle_tree(tag: &str, close_second_cycle: bool) -> DiTree {
    // The master compiles from its own scratch copy (RF-I00-01), like every
    // vendored deck a test runs.
    let master_copy = ScratchCopy::new(
        &corpus_deck("electricdss-tst/Version8/Distrib/IEEETestCases/13Bus/IEEE13Nodeckt.dss")
            .to_string_lossy(),
        scratch::PORT,
    );
    let master = PathBuf::from(master_copy.deck());
    let scratch = scratch_dir(tag);
    // The probe's "case directory" is the deck's parent, so the fixture deck must
    // exist BEFORE the snapshot — otherwise it is itself run-created.
    let deck = scratch.join("di_cycle.dss");
    std::fs::write(&deck, "! GOLDEN_REBASE G1.10c capture-point fixture\n")
        .unwrap_or_else(|e| panic!("write {}: {e}", deck.display()));
    let ctx = format!("di_pins:di_cycle_{tag}");

    let probe = RunFileProbe::start(&deck.to_string_lossy());
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!("compile \"{}\"", master.display()));
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    dss.command("new energymeter.em1 element=Line.650632 terminal=1");
    dss.command("solve");
    dss.command("set demandinterval=yes");
    dss.command("set diverbose=yes");
    dss.command("set mode=yearly number=3 stepsize=1h");
    dss.command("solve");
    dss.command("closedi");
    dss.command("set mode=yearly number=5 stepsize=1h");
    dss.command("solve");
    if close_second_cycle {
        dss.command("closedi");
    }
    assert!(dss.errors().is_empty(), "{ctx}: {:?}", dss.errors());
    drop(dss);
    let tree = probe.di_tree(&ctx);
    probe.finish_and_clean(&ctx, false);
    let _ = std::fs::remove_dir_all(&scratch);
    master_copy.finish();
    tree
}

/// **The demand-interval capture reads the LAST CLOSED cycle** (coordinator
/// decision **D42(6)**: the read sits after `autoadd_log`, before `created()`
/// and before the `capi_v0145` teardown `clear`).
///
/// The demand-interval streams stay open across a yearly run and reach disk only
/// on `CloseDI` / `Set year=` / a mode change. The ENGINE half of that is
/// already pinned by
/// `golden_reports::demand_interval_yearly_stays_open_until_closedi` — this pin
/// cites it and never duplicates it (the port's
/// `crates/dss-core/src/solution/meters/demand_interval.rs:515-570`
/// `close_all_di_files`, r4133's `TEnergyMeter.CloseAllDIFiles`).
///
/// What this adds is the CAPTURE's half, over the very producer the corpus gate
/// uses (`harness::run_files::RunFileProbe::di_tree`, read after the engine is
/// dropped): a fixture that closes a 3-step cycle and leaves a 5-step cycle in
/// flight is captured with the first cycle's **3** rows — not the in-flight
/// **5**, which the same fixture yields when it closes them. Both numbers are
/// named, and this is exactly why the read must precede the capi teardown
/// `clear`: that `clear` flushes the in-flight cycle (measured on 3 of the 5
/// live cases; r4133's does not), so a read after it would compare a different
/// cycle on the two channels.
#[test]
fn the_di_capture_reads_the_last_closed_cycle() {
    let member = "ieee13nodeckt/di_yr_0/em1_1.csv";
    let open = di_cycle_tree("open", false);
    let closed = di_cycle_tree("closed", true);

    let rows = |tree: &DiTree, what: &str| -> usize {
        let text = tree
            .get(member)
            .unwrap_or_else(|| panic!("{what}: no {member} in {:?}", tree.keys()));
        lines_of(text).len() - 1
    };
    assert_eq!(
        rows(&open, "in flight"),
        3,
        "the capture must hold the LAST CLOSED cycle's 3 rows while a 5-step cycle \
         is still open in memory"
    );
    assert_eq!(
        rows(&closed, "closed"),
        5,
        "closing the second cycle before the capture replaces those rows with its \
         own 5"
    );
    // The header is written when the file is opened, so both captures carry it.
    for (tree, what) in [(&open, "in flight"), (&closed, "closed")] {
        let text = &tree[member];
        assert!(
            lines_of(text)[0].starts_with("\"Hour\""),
            "{what}: {member} must carry its header ({:?})",
            lines_of(text)[0]
        );
    }
}

// ---------------------------------------------------------------------------
// (4) the class table over the live population — D42(1).
// ---------------------------------------------------------------------------

/// Every `(file stem, columns classified, the header carries the column names)`
/// the five live cases produce — the inventory `harness::di`'s class table must
/// cover, measured over the port's own trees.
///
/// The three `header carries names = false` shapes are `StoCtrl_SeasonTarget`'s
/// totals files, whose header is its index column alone (`Time` / `Year` /
/// `Name`) because r4133 writes the register names only `if Assigned(mtr)`
/// (`Version8/Source/Meters/EnergyMeter.pas:3528-3530`, `:3603-3606`); their
/// rows still carry all 68 fields, which is why the class vector is positional
/// over `TEnergyMeterObj.RegisterNames` and never a lookup by header text.
const DI_SHAPES_LIVE: [(&str, usize, bool); 13] = [
    ("25607", 68, true),
    ("di_overloads", 10, true),
    ("di_systemmeter", 8, true),
    ("di_totals", 68, false),
    ("di_totals", 68, true),
    ("di_voltexceptions", 13, true),
    ("energymetertotals", 68, false),
    ("energymetertotals", 68, true),
    ("feeder", 68, true),
    ("sub", 68, true),
    ("systemmeter", 8, true),
    ("totals", 68, false),
    ("totals", 68, true),
];

/// The live population's size, re-derived here from the port's own trees: 36
/// files, 1 454 classified columns, 4 922 812 classified cells. The gate's own
/// census (`corpus_gate::scheduler::DI_TREE_CENSUS`) counts the same cells on
/// BOTH channels, minus the three excluded columns:
/// `2 × 4 922 812 − 52 560 = 9 793 064`.
const DI_LIVE_FILES: usize = 36;
const DI_LIVE_COLUMNS: usize = 1454;
const DI_LIVE_CELLS: usize = 4_922_812;

/// **The class table covers every column of every live demand-interval file**
/// (coordinator decision **D42(1)**: each column is compared at the calibrated
/// tier of the quantity it carries, and a column no rule classifies FAILS the
/// case instead of inheriting a default band).
///
/// That refusal is implemented once, in `harness::di::column_specs`, and this
/// pin drives it over the whole live population instead of letting the table
/// quote itself: the five decks' **36** files are classified through the very
/// entry point the comparator uses (`harness::di::classify_member`), all **1 454**
/// of their columns come back with a class, every data row is exactly as wide as
/// its file's class vector (**4 922 812** cells), and the set of SHAPES met is
/// [`DI_SHAPES_LIVE`] — fail-on-stale in both directions, so a new demand-interval
/// file kind (or a lost one) reds here and gets a class before it can be compared.
#[test]
fn the_di_class_table_covers_every_column_of_every_live_di_file() {
    let run = port();
    let mut shapes: BTreeSet<(String, usize, bool)> = BTreeSet::new();
    let (mut files, mut columns, mut cells) = (0usize, 0usize, 0usize);

    for case in &DI_CASES {
        let tree = tree_of(case.label);
        let names: Vec<&str> = tree.keys().map(String::as_str).collect();
        assert_eq!(
            names, case.files,
            "{}: the demand-interval FILE SET the port writes moved",
            case.label
        );
        for (name, text) in tree {
            let ctx = format!("di_pins:{} {name}", case.label);
            let lines = lines_of(text);
            let (shape, specs) = classify_member(name, lines[0], &ctx);
            assert!(
                specs.iter().all(|s| !s.name.is_empty()),
                "{ctx}: every classified column must be named"
            );
            assert_eq!(
                specs.first().map(|s| s.class),
                Some(DiClass::Index),
                "{ctx}: a demand-interval file's first column is its index"
            );
            for (i, row) in lines[1..].iter().enumerate() {
                assert_eq!(
                    row.split(',').count(),
                    specs.len(),
                    "{ctx}: data row {i} is not as wide as its class vector"
                );
            }
            shapes.insert(shape);
            files += 1;
            columns += specs.len();
            cells += specs.len() * (lines.len() - 1);
        }
    }

    assert_eq!(
        shapes.into_iter().collect::<Vec<_>>(),
        DI_SHAPES_LIVE
            .iter()
            .map(|(s, c, n)| ((*s).to_string(), *c, *n))
            .collect::<Vec<_>>(),
        "the live demand-interval SHAPES moved"
    );
    assert_eq!(files, DI_LIVE_FILES, "live DI files");
    assert_eq!(columns, DI_LIVE_COLUMNS, "live DI columns classified");
    assert_eq!(cells, DI_LIVE_CELLS, "live DI cells classified");
    assert_eq!(run.trees.len(), DI_CASES.len(), "the five DI cases");
}
