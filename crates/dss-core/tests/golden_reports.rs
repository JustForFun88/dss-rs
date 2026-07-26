//! Phase 8 report goldens (PHASE8_PLAN §2.3): pin the **report file the oracle
//! writes**. `tools/golden/gen_reports.py` runs a fixture on the pinned engine,
//! issues the `Export`/`Show`/... command, and captures the produced file's
//! bytes into `tests/golden/reports/<report>.txt` (+ a `<report>.meta.json` with
//! the exact deck, so the Rust and oracle fixtures can never drift). This driver
//! replays the same deck, writes its own report into a temp dir, and diffs the
//! two **after parsing numbers out** via `harness::compare_export` — never a raw
//! float-string diff.
//!
//! WP8.1 self-test: `Export Counts` — a text dump of every DSS class and its
//! instance count. The Rust class registry is a *proper subset* of the oracle's
//! (only a subset of classes is ported), so the comparison is `RustSubsetByKey`:
//! every ported class's count is pinned against the oracle; the classes we don't
//! yet register are ignored (see `tests/TOLERANCE_NOTES.md`). This is a real
//! gate (it shakes out the whole output path: registry walk → format → file IO →
//! the `compare_export` harness), not a self-comparison.
//!
//! Regenerate only manually: `python tools/golden/gen_reports.py`.

mod harness;

use std::path::{Path, PathBuf};

use dss_core::exec::Dss;
use harness::{
    ColSel, ColTol, ExportPolicy, GateSpec, RowPolicy, assert_value_matches_tol, compare_export,
};
use serde::Deserialize;

fn reports_dir() -> PathBuf {
    [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "golden",
        "reports",
    ]
    .iter()
    .collect()
}

#[derive(Debug, Deserialize)]
struct ReportMeta {
    report: String,
    fixture: String,
    deck: Vec<String>,
}

/// Meta for a **deck-based** export golden (a self-contained `New`-circuit deck
/// with no master, like `ReportMeta` but carrying the oracle default-filename
/// suffix so the produced path is pinned — the reliability/capacity fixtures).
#[derive(Debug, Deserialize)]
struct DeckMeta {
    report: String,
    fixture: String,
    suffix: String,
    deck: Vec<String>,
}

/// Drive one deck-based export: replay the deck (no compile), route the report
/// into a scratch dir, export, and diff the produced file against the captured
/// oracle file via `compare_export`. The deck-fixture twin of `run_feeder_export`.
fn run_deck_export(stem: &str, policy: &ExportPolicy) {
    let dir = reports_dir();
    let meta: DeckMeta = {
        let p = dir.join(format!("{stem}.meta.json"));
        let text =
            std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", p.display()))
    };
    let oracle = {
        let p = dir.join(format!("{stem}.txt"));
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
    };

    let scratch = scratch_dir(stem);
    let mut dss = Dss::new();
    dss.command("clear");
    for c in &meta.deck {
        dss.command(c);
    }
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    dss.command(&format!("export {}", meta.report));
    assert!(dss.errors().is_empty(), "{stem}: {:?}", dss.errors());

    let produced = dss.last_result_file();
    let want = format!("{}_{}", meta.fixture, meta.suffix).to_lowercase();
    assert!(
        produced.to_lowercase().ends_with(&want),
        "{stem}: unexpected produced path {produced:?} (want …{want})"
    );
    let rust = std::fs::read_to_string(produced)
        .unwrap_or_else(|e| panic!("read produced {produced}: {e}"));

    compare_export(&oracle, &rust, policy, stem);

    std::fs::remove_dir_all(&scratch).ok();
}

/// A unique scratch dir for this test process (no `tempfile` dep; cleaned up at
/// the end). `Set DataPath=` points the engine's report output here.
fn scratch_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("dss_reports_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap_or_else(|e| panic!("mkdir {}: {e}", d.display()));
    d
}

/// `Export Counts` (Pascal `ExportCounts`): every class + its instance count.
/// The Rust file must be a `RustSubsetByKey` subset of the oracle's (the Rust
/// class registry is a proper subset during the port); shared classes' counts
/// are pinned exactly (integers — zero tolerance).
#[test]
fn export_counts_matches_oracle() {
    let dir = reports_dir();
    let meta: ReportMeta = {
        let p = dir.join("export_counts.meta.json");
        let text =
            std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", p.display()))
    };
    assert_eq!(meta.report, "Counts");
    let oracle = {
        let p = dir.join("export_counts.txt");
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
    };

    let scratch = scratch_dir("counts");
    let mut dss = Dss::new();
    dss.command("clear");
    for c in &meta.deck {
        dss.command(c);
    }
    // Route the report into the scratch dir (the same way the oracle fixture
    // did), then export. Quote the path in case of spaces.
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    dss.command("export counts");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    let produced = dss.last_result_file();
    assert!(
        produced
            .to_lowercase()
            .ends_with(&format!("{}_exp_counts.csv", meta.fixture)),
        "unexpected produced path: {produced:?}"
    );
    let rust = std::fs::read_to_string(produced)
        .unwrap_or_else(|e| panic!("read produced {produced}: {e}"));

    let policy = ExportPolicy {
        sep: '=',
        header_lines: 1, // "Format: DSS Class Name = Instance Count"
        rows: RowPolicy::RustSubsetByKey {
            key: 0,
            // Must-emit classes: the deck-created (Line/Load/Vsource) + the
            // default DSS items (TCC_Curve/Spectrum/LoadShape/GrowthShape). Guards
            // against a registry-walk regression that drops a class or empties the
            // body (the subset compare alone can't see a missing row).
            require: [
                "line",
                "load",
                "vsource",
                "tcc_curve",
                "spectrum",
                "loadshape",
                "growthshape",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
            // WindGen (WP-U1.8) is a 0.15.x class absent from the pinned 0.14.5
            // oracle; skip its count row (gated against capi015 live instead).
            allow_extra: vec!["windgen".to_string()],
        },
        rel: 0.0,
        abs: 0.0, // integer counts — exact
        col_tol: vec![],
    };
    compare_export(&oracle, &rust, &policy, "export_counts");

    std::fs::remove_dir_all(&scratch).ok();
}

/// Meta for a solution-export golden (a feeder compiled + solved on both
/// engines, then one report captured): the master to compile (relative to
/// `tests/corpus/electricdss-tst`), the post commands, the circuit/CaseName, and
/// the oracle's default-filename suffix.
#[derive(Debug, Deserialize)]
struct FeederMeta {
    report: String,
    master: String,
    post: Vec<String>,
    fixture: String,
    suffix: String,
}

/// Drive one solution export: compile the same master the oracle used, replay
/// the post commands, route the report into a scratch dir, export, and diff the
/// produced file against the captured oracle file via `compare_export`.
fn run_feeder_export(stem: &str, policy: &ExportPolicy) {
    let dir = reports_dir();
    let meta: FeederMeta = {
        let p = dir.join(format!("{stem}.meta.json"));
        let text =
            std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", p.display()))
    };
    let oracle = {
        let p = dir.join(format!("{stem}.txt"));
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
    };

    let master: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "corpus",
        "electricdss-tst",
    ]
    .iter()
    .collect::<PathBuf>()
    .join(&meta.master);
    assert!(master.is_file(), "master missing: {}", master.display());

    let scratch = scratch_dir(stem);
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!(
        "compile \"{}\"",
        master.to_string_lossy().replace('\\', "/")
    ));
    for c in &meta.post {
        dss.command(c);
    }
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    dss.command(&format!("export {}", meta.report));
    assert!(dss.errors().is_empty(), "{stem}: {:?}", dss.errors());

    let produced = dss.last_result_file();
    let want = format!("{}_{}", meta.fixture, meta.suffix).to_lowercase();
    assert!(
        produced.to_lowercase().ends_with(&want),
        "{stem}: unexpected produced path {produced:?} (want …{want})"
    );
    let rust = std::fs::read_to_string(produced)
        .unwrap_or_else(|e| panic!("read produced {produced}: {e}"));

    compare_export(&oracle, &rust, policy, stem);

    std::fs::remove_dir_all(&scratch).ok();
}

/// Locate the single `*_<suffix>` report a `Show` command wrote into `scratch`
/// (the same fixed-name suffix glob the oracle generator uses) and read it back.
/// Shared by the feeder/deck show runners; asserts exactly one match — a wrong
/// filename (the `Result.txt`→`.csv` class of bug) leaves the glob empty and fails.
fn locate_show_report(scratch: &Path, suffix: &str, stem: &str) -> String {
    let want = format!("_{}", suffix).to_lowercase();
    let mut matches: Vec<PathBuf> = std::fs::read_dir(scratch)
        .unwrap_or_else(|e| panic!("{stem}: read_dir {}: {e}", scratch.display()))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().to_lowercase().ends_with(&want))
                .unwrap_or(false)
        })
        .collect();
    assert_eq!(
        matches.len(),
        1,
        "{stem}: expected exactly one *_{suffix} in {}, found {matches:?}",
        scratch.display()
    );
    let produced = matches.pop().unwrap();
    std::fs::read_to_string(&produced)
        .unwrap_or_else(|e| panic!("read produced {}: {e}", produced.display()))
}

/// Byte-exact line comparison (no tokenization) for pure-text `Show` reports whose
/// layout has **no** backend width quirk — the zone-tree reports (`Show Loops`/
/// `Show Zone`) indent with deterministic `TABCHAR`s and print no numbers, so the
/// oracle bytes are reproducible in full. Stronger than `compare_export`'s token
/// diff: it also pins the leading indentation and trailing spaces (a formatter-side
/// off-by-one in the tab depth, invisible to the whitespace tokenizer, fails here).
/// Only CRLF→LF is normalized (the oracle golden is stored LF; the port writes LF).
fn assert_show_bytes_eq(oracle: &str, rust: &str, stem: &str) {
    let o = oracle.replace("\r\n", "\n");
    let r = rust.replace("\r\n", "\n");
    if o != r {
        let ol: Vec<&str> = o.split('\n').collect();
        let rl: Vec<&str> = r.split('\n').collect();
        for (i, (a, b)) in ol.iter().zip(rl.iter()).enumerate() {
            assert_eq!(
                a,
                b,
                "{stem}: line {} differs\n  oracle: {a:?}\n  rust:   {b:?}",
                i + 1
            );
        }
        assert_eq!(
            ol.len(),
            rl.len(),
            "{stem}: line count differs (oracle {}, rust {})",
            ol.len(),
            rl.len()
        );
    }
}

/// Drive one `Show` report (PHASE8_PLAN §WP8.4): compile the same master the
/// oracle used, replay the post commands, route output into a scratch dir, issue
/// `Show <keyword>`, and diff the Rust-written fixed-width text file against the
/// oracle's via `compare_export`. The `Show` twin of `run_feeder_export` — it
/// reads `dss.last_show_file()` (Pascal `Show` sets `@lastshowfile`, not
/// `GlobalResult`) and the golden policy tokenizes on whitespace+commas
/// (`sep: ' '`) since `Show` emits space-padded tables, not CSV.
fn run_feeder_show(stem: &str, policy: &ExportPolicy) {
    let (oracle, rust, scratch) = produce_feeder_show(stem);
    compare_export(&oracle, &rust, policy, stem);
    std::fs::remove_dir_all(&scratch).ok();
}

/// Replay a feeder-based `Show` fixture and return `(oracle golden, Rust output,
/// scratch dir)`. Shared by [`run_feeder_show`] (token diff) and
/// [`run_feeder_show_exact`] (byte-exact diff); the caller removes the scratch dir.
fn produce_feeder_show(stem: &str) -> (String, String, PathBuf) {
    let dir = reports_dir();
    let meta: FeederMeta = {
        let p = dir.join(format!("{stem}.meta.json"));
        let text =
            std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", p.display()))
    };
    let oracle = {
        let p = dir.join(format!("{stem}.txt"));
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
    };

    let master: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "corpus",
        "electricdss-tst",
    ]
    .iter()
    .collect::<PathBuf>()
    .join(&meta.master);
    assert!(master.is_file(), "master missing: {}", master.display());

    let scratch = scratch_dir(stem);
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!(
        "compile \"{}\"",
        master.to_string_lossy().replace('\\', "/")
    ));
    for c in &meta.post {
        dss.command(c);
    }
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    dss.command(&format!("show {}", meta.report));
    assert!(dss.errors().is_empty(), "{stem}: {:?}", dss.errors());

    // Locate the produced report by its fixed `<CaseName_><suffix>` name in the
    // datapath — the same suffix glob the oracle generator uses. Robust to whether
    // the report sets `@lastshowfile` (arms 4/27 — Convergence/ControlQueue — do
    // not, matching Pascal's inline `FireOffEditor`-only dispatch), and still pins
    // the filename.
    let rust = locate_show_report(&scratch, &meta.suffix, stem);
    (oracle, rust, scratch)
}

/// Byte-exact twin of [`run_feeder_show`] for the pure-text zone-tree reports
/// (`Show Loops`/`Show Zone`): identical replay + file-locate, but asserts the
/// produced bytes equal the oracle golden in full (indentation + trailing spaces),
/// not just token-for-token.
fn run_feeder_show_exact(stem: &str) {
    let (oracle, rust, scratch) = produce_feeder_show(stem);
    assert_show_bytes_eq(&oracle, &rust, stem);
    std::fs::remove_dir_all(&scratch).ok();
}

/// Drive one **deck-based** `Show` report (PHASE8_PLAN §WP8.4): the deck twin of
/// [`run_feeder_show`] (a self-contained `New`-circuit deck, no master compile) —
/// analogous to [`run_deck_export`], but reading the produced file by its fixed
/// `<CaseName_><suffix>` name in the datapath (`Show` sets no `GlobalResult`).
fn run_deck_show(stem: &str, policy: &ExportPolicy) {
    let (oracle, rust, scratch) = produce_deck_show(stem);
    compare_export(&oracle, &rust, policy, stem);
    std::fs::remove_dir_all(&scratch).ok();
}

/// Byte-exact twin of [`run_deck_show`] (pure-text zone-tree reports). See
/// [`run_feeder_show_exact`].
fn run_deck_show_exact(stem: &str) {
    let (oracle, rust, scratch) = produce_deck_show(stem);
    assert_show_bytes_eq(&oracle, &rust, stem);
    std::fs::remove_dir_all(&scratch).ok();
}

/// Replay a deck-based `Show` fixture and return `(oracle golden, Rust output,
/// scratch dir)`. Shared by [`run_deck_show`] and [`run_deck_show_exact`]; the
/// caller removes the scratch dir.
fn produce_deck_show(stem: &str) -> (String, String, PathBuf) {
    let dir = reports_dir();
    let meta: DeckMeta = {
        let p = dir.join(format!("{stem}.meta.json"));
        let text =
            std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", p.display()))
    };
    let oracle = {
        let p = dir.join(format!("{stem}.txt"));
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
    };

    let scratch = scratch_dir(stem);
    let mut dss = Dss::new();
    dss.command("clear");
    for c in &meta.deck {
        dss.command(c);
    }
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    dss.command(&format!("show {}", meta.report));
    assert!(dss.errors().is_empty(), "{stem}: {:?}", dss.errors());

    let rust = locate_show_report(&scratch, &meta.suffix, stem);
    (oracle, rust, scratch)
}

/// Drive one deck-based `Dump` report (PHASE8_PLAN §WP8.5): replay the deck, route
/// output into a scratch dir, issue `Dump <report>`, and byte-compare the produced
/// `<case>_PropertyDump.txt` against the oracle golden. Unlike `Show`, `Dump` sets
/// `GlobalResult` to the produced path, so the file is read via
/// `dss.last_result_file()` (no suffix glob). Byte-exact: the dump is pure DSS
/// script text (no padded columns), so the oracle bytes are reproducible in full.
fn run_deck_dump_exact(stem: &str) {
    let dir = reports_dir();
    let meta: DeckMeta = {
        let p = dir.join(format!("{stem}.meta.json"));
        let text =
            std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", p.display()))
    };
    let oracle = {
        let p = dir.join(format!("{stem}.txt"));
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
    };

    let scratch = scratch_dir(stem);
    let mut dss = Dss::new();
    dss.command("clear");
    for c in &meta.deck {
        dss.command(c);
    }
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    dss.command(&format!("dump {}", meta.report));
    assert!(dss.errors().is_empty(), "{stem}: {:?}", dss.errors());

    let produced = dss.last_result_file();
    let rust = std::fs::read_to_string(produced)
        .unwrap_or_else(|e| panic!("{stem}: read produced {produced}: {e}"));
    assert_show_bytes_eq(&oracle, &rust, stem);
    std::fs::remove_dir_all(&scratch).ok();
}

/// `run_deck_dump_exact` twin for the Capacitor decks: the oracle golden was
/// captured with the `~ CMatrix=(`/`~ FaultRate=`/`~ pctPerm=` lines already
/// dropped (probe-proven ASLR-garbage in this pinned build, `tools/golden/
/// report_decks/README.md`); this drops the same line prefixes from the Rust
/// output (which renders the correct, non-garbage values — a genuinely
/// different line, not comparable) before the byte-exact compare.
fn run_deck_dump_exact_masked(stem: &str, mask_prefixes: &[&str]) {
    let dir = reports_dir();
    let meta: DeckMeta = {
        let p = dir.join(format!("{stem}.meta.json"));
        let text =
            std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", p.display()))
    };
    let oracle = {
        let p = dir.join(format!("{stem}.txt"));
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
    };

    let scratch = scratch_dir(stem);
    let mut dss = Dss::new();
    dss.command("clear");
    for c in &meta.deck {
        dss.command(c);
    }
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    dss.command(&format!("dump {}", meta.report));
    assert!(dss.errors().is_empty(), "{stem}: {:?}", dss.errors());

    let produced = dss.last_result_file();
    let rust = std::fs::read_to_string(produced)
        .unwrap_or_else(|e| panic!("{stem}: read produced {produced}: {e}"));
    let masked: String = rust
        .lines()
        .filter(|ln| !mask_prefixes.iter().any(|p| ln.starts_with(p)))
        .map(|ln| format!("{ln}\n"))
        .collect();
    assert_show_bytes_eq(&oracle, &masked, stem);
    std::fs::remove_dir_all(&scratch).ok();
}

/// Like [`run_deck_dump_exact`], but drops whole `[Header]…` blocks from the Rust
/// dump before the byte compare — a `Dump commands` section is `[Class]` then its
/// numbered property lines (indistinguishable by prefix from any other class), so
/// a class the pinned 0.14.5 oracle lacks (WP-U1.8 `[WindGen]`, a 0.15.x class)
/// is removed as a contiguous `[WindGen]`→next-`[` block. WindGen's command
/// surface is gated against `capi015` live + the props round-trip, not this
/// 0.14.5 golden.
fn run_deck_dump_exact_block_masked(stem: &str, block_headers: &[&str]) {
    let dir = reports_dir();
    let meta: DeckMeta = {
        let p = dir.join(format!("{stem}.meta.json"));
        let text =
            std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", p.display()))
    };
    let oracle = {
        let p = dir.join(format!("{stem}.txt"));
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
    };

    let scratch = scratch_dir(stem);
    let mut dss = Dss::new();
    dss.command("clear");
    for c in &meta.deck {
        dss.command(c);
    }
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    dss.command(&format!("dump {}", meta.report));
    assert!(dss.errors().is_empty(), "{stem}: {:?}", dss.errors());

    let produced = dss.last_result_file();
    let rust = std::fs::read_to_string(produced)
        .unwrap_or_else(|e| panic!("{stem}: read produced {produced}: {e}"));

    // WP-U1.9: the 0.15.x ExecOptions the pinned 0.14.5 oracle lacks (the PCE
    // force hooks + the NCIM options they share an enum tail with). They dump as
    // single `<ord>, "<Name>", "<help>"` lines after the last 0.14.5 option
    // (NUMANodes, 128) and before the first class block; drop them before the
    // exact compare, same as the `[WindGen]` class block (gated live vs capi015
    // + `exec/tests/force_hooks.rs` instead).
    const NEW_015X_OPTIONS: &[&str] = &[
        "IgnoreGenQLimits",
        "NCIMQGain",
        "StateVar",
        "PyPath",
        "IterNumber",
        "CtrlIterNumber",
        "InjCurrent",
        "ITerminal",
        "YPrim",
        "IntegrationFlag",
        "AllowForms",
        "AllowProgressBar",
    ];
    let is_new_option_line = |ln: &str| {
        // `<digits>, "<Name>", …` where Name is a 0.15.x-only option.
        ln.split_once(", \"")
            .and_then(|(ord, rest)| {
                ord.trim().parse::<u32>().ok()?;
                rest.split_once('"').map(|(name, _)| name)
            })
            .is_some_and(|name| NEW_015X_OPTIONS.contains(&name))
    };

    let mut out = String::new();
    let mut dropping = false;
    for ln in rust.lines() {
        if ln.starts_with('[') {
            dropping = block_headers.contains(&ln.trim());
        }
        if !dropping && !is_new_option_line(ln) {
            out.push_str(ln);
            out.push('\n');
        }
    }
    assert_show_bytes_eq(&oracle, &out, stem);
    std::fs::remove_dir_all(&scratch).ok();
}

/// One binary-save golden: the produced filename in the datapath and the raw
/// bytes captured under `tests/golden/reports/<golden>`.
#[derive(Debug, Deserialize)]
struct BinSaveFile {
    produced: String,
    golden: String,
}

/// Meta for the WPG.17 binary-save golden: the definition deck, the
/// `Action=SngSave/DblSave` commands, and the produced→golden file map.
#[derive(Debug, Deserialize)]
struct BinSaveMeta {
    deck: Vec<String>,
    actions: Vec<String>,
    files: Vec<BinSaveFile>,
}

/// WPG.17: LoadShape/TShape/PriceShape `Action=SngSave/DblSave` binary writers
/// (Pascal `SaveToDblFile`/`SaveToSngFile`). The goldens are the **raw bytes**
/// the oracle wrote — pure little-endian IEEE-754 with zero formatting freedom —
/// so this is a byte-exact `Vec<u8>` compare (no tolerance, unlike the
/// number-parsing `compare_export` used for text reports). Both engines parse
/// the same decimal literals to the same nearest-f64 and narrow to the same
/// nearest-f32, so bit-identity is guaranteed for a literal-mult deck. A
/// Rust-only write→re-read round-trip could not catch an endian/stride/`_P`-
/// suffix/element-count error that happens to round-trip; this can.
#[test]
fn binsave_matches_oracle() {
    let dir = reports_dir();
    let meta: BinSaveMeta = {
        let p = dir.join("loadshape_binsave.meta.json");
        let text =
            std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", p.display()))
    };

    let scratch = scratch_dir("binsave");
    let mut dss = Dss::new();
    dss.command("clear");
    for c in &meta.deck {
        dss.command(c);
    }
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    // Per-action `GlobalResult` (audit settlement): `<tag>=[<ftag>=<path>]`,
    // with the LoadShape Q clause joined by `AppendGlobalResult`'s `', '` plus
    // the clause's own leading space — `],  Qmult=[` (comma + TWO spaces),
    // oracle-probed (`DSSGlobals.pas:452-459`).
    let p = |n: &str| scratch.join(n).display().to_string();
    let expected_results = [
        format!(
            "mult=[sngfile={}],  Qmult=[sngfile={}]",
            p("bs_P.sng"),
            p("bs_Q.sng")
        ),
        format!(
            "mult=[dblfile={}],  Qmult=[dblfile={}]",
            p("bs_P.dbl"),
            p("bs_Q.dbl")
        ),
        format!("Temp=[sngfile={}]", p("ts.sng")),
        format!("Temp=[dblfile={}]", p("ts.dbl")),
        format!("Price=[sngfile={}]", p("ps.sng")),
        format!("Price=[dblfile={}]", p("ps.dbl")),
    ];
    assert_eq!(meta.actions.len(), expected_results.len());
    for (c, exp) in meta.actions.iter().zip(&expected_results) {
        dss.command(c);
        assert_eq!(&dss.result(), exp, "GlobalResult after {c:?}");
    }
    assert!(dss.errors().is_empty(), "binsave: {:?}", dss.errors());

    for f in &meta.files {
        let golden = std::fs::read(dir.join(&f.golden))
            .unwrap_or_else(|e| panic!("read golden {}: {e}", f.golden));
        let produced = std::fs::read(scratch.join(&f.produced))
            .unwrap_or_else(|e| panic!("read produced {}: {e}", f.produced));
        assert_eq!(
            produced, golden,
            "binsave: bytes differ for {} (golden {})",
            f.produced, f.golden
        );
    }

    std::fs::remove_dir_all(&scratch).ok();
}

/// Meta for the WPG.20 MMF-backed binary-save golden. Adds `absent` (files that
/// must NOT be emitted — the `Assigned(dQ)=false` case) and the per-action
/// `GlobalResult` decomposition (`result_files`/`result_tags`) so the Rust
/// runner rebuilds each expected result against its own scratch path.
#[derive(Debug, Deserialize)]
struct BinSaveMmfMeta {
    deck: Vec<String>,
    actions: Vec<String>,
    files: Vec<BinSaveFile>,
    absent: Vec<String>,
    result_files: Vec<Vec<String>>,
    result_tags: Vec<Vec<String>>,
}

/// WPG.20: MMF-backed (`MemoryMapping=Yes`) `Action=SngSave/DblSave`. Under MMF
/// the multipliers live in a memory-mapped file; the oracle re-reads each value
/// through `InterpretDblArrayMMF` at save time (Pascal `SaveToDblFile`/
/// `SaveToSngFile`, LoadShape.pas:1898-1905/1956-1963 P, :1921-1927/:1982-1988
/// Q). The port eagerly read the file into `p_mult`/`q_mult` at directive time
/// (`read_mmf_raw`), so the same non-MMF snapshot emits the identical bytes.
/// Byte-exact `Vec<u8>` compare of every `_P`/`_Q` file, PLUS: (a) the `_Q`-gating
/// — `md` has no `qmult` so `md_Q.*` must be ABSENT (Pascal `Assigned(dQ)` false);
/// (b) the exact per-action `GlobalResult` (`mult=[…],  Qmult=[…]`, the Q clause
/// joined with comma+two-spaces, probe-proven). The MMF source files resolve via
/// the `@FIXTURES@` token on both engines.
#[test]
fn binsave_mmf_matches_oracle() {
    let dir = reports_dir();
    let meta: BinSaveMmfMeta = {
        let p = dir.join("loadshape_binsave_mmf.meta.json");
        let text =
            std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", p.display()))
    };

    // Forward slashes; no canonicalize (its `\\?\` prefix breaks the parser).
    let fixtures = fixtures_dir().to_string_lossy().replace('\\', "/");
    let scratch = scratch_dir("binsave_mmf");
    let mut dss = Dss::new();
    dss.command("clear");
    for c in &meta.deck {
        dss.command(&c.replace("@FIXTURES@", &fixtures));
    }
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    assert!(
        dss.errors().is_empty(),
        "binsave_mmf setup: {:?}",
        dss.errors()
    );

    // Rebuild the expected per-action `GlobalResult` against this scratch dir:
    // `tag=[ftag=path]`, the Q clause joined by `AppendGlobalResult`'s `', '` plus
    // the clause's own leading space -> `],  Qmult=[` (comma + TWO spaces).
    let p = |n: &str| scratch.join(n).display().to_string();
    assert_eq!(meta.actions.len(), meta.result_files.len());
    assert_eq!(meta.actions.len(), meta.result_tags.len());
    for (i, c) in meta.actions.iter().enumerate() {
        let files = &meta.result_files[i];
        let tags = &meta.result_tags[i];
        assert_eq!(tags.len(), 2 * files.len());
        let mut exp = String::new();
        for (k, f) in files.iter().enumerate() {
            let obj_tag = &tags[2 * k]; // mult / Qmult
            let ftag = &tags[2 * k + 1]; // sngfile / dblfile
            if k == 0 {
                exp.push_str(&format!("{obj_tag}=[{ftag}={}]", p(f)));
            } else {
                // AppendGlobalResult joiner ', ' + clause leading space.
                exp.push_str(&format!(",  {obj_tag}=[{ftag}={}]", p(f)));
            }
        }
        dss.command(c);
        assert_eq!(&dss.result(), &exp, "GlobalResult after {c:?}");
    }
    assert!(dss.errors().is_empty(), "binsave_mmf: {:?}", dss.errors());

    // The `Assigned(dQ)=false` case: no `_Q` file for the qmult-less shape.
    for a in &meta.absent {
        assert!(
            !scratch.join(a).exists(),
            "binsave_mmf: {a} must NOT be written (Assigned(dQ) false)"
        );
    }

    for f in &meta.files {
        let golden = std::fs::read(dir.join(&f.golden))
            .unwrap_or_else(|e| panic!("read golden {}: {e}", f.golden));
        let produced = std::fs::read(scratch.join(&f.produced))
            .unwrap_or_else(|e| panic!("read produced {}: {e}", f.produced));
        assert_eq!(
            produced, golden,
            "binsave_mmf: bytes differ for {} (golden {})",
            f.produced, f.golden
        );
    }

    std::fs::remove_dir_all(&scratch).ok();
}

/// Compile a **heavy** master once and diff several reports against the oracle,
/// avoiding a per-report recompile (IEEE 8500 is ~6100 devices / 8531 nodes).
/// Each `(stem, policy)` reads its own `<stem>.meta.json`; all must agree on the
/// master/post/fixture (asserted — they are the same solved circuit), single-
/// sourced exactly like `run_feeder_export`.
fn run_shared_exports(reports: &[(&str, ExportPolicy)]) {
    let dir = reports_dir();
    let metas: Vec<FeederMeta> = reports
        .iter()
        .map(|(stem, _)| {
            let p = dir.join(format!("{stem}.meta.json"));
            let text =
                std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", p.display()))
        })
        .collect();
    let m0 = &metas[0];
    for m in &metas[1..] {
        assert_eq!(m.master, m0.master, "shared exports disagree on master");
        assert_eq!(m.post, m0.post, "shared exports disagree on post");
        assert_eq!(m.fixture, m0.fixture, "shared exports disagree on fixture");
    }

    let master: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "corpus",
        "electricdss-tst",
    ]
    .iter()
    .collect::<PathBuf>()
    .join(&m0.master);
    assert!(master.is_file(), "master missing: {}", master.display());

    let scratch = scratch_dir(reports[0].0);
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!(
        "compile \"{}\"",
        master.to_string_lossy().replace('\\', "/")
    ));
    for c in &m0.post {
        dss.command(c);
    }
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));

    for ((stem, policy), m) in reports.iter().zip(&metas) {
        dss.command(&format!("export {}", m.report));
        assert!(dss.errors().is_empty(), "{stem}: {:?}", dss.errors());
        let produced = dss.last_result_file();
        let want = format!("{}_{}", m.fixture, m.suffix).to_lowercase();
        assert!(
            produced.to_lowercase().ends_with(&want),
            "{stem}: unexpected produced path {produced:?} (want …{want})"
        );
        let rust = std::fs::read_to_string(produced)
            .unwrap_or_else(|e| panic!("read produced {produced}: {e}"));
        let oracle = {
            let p = dir.join(format!("{stem}.txt"));
            std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
        };
        compare_export(&oracle, &rust, policy, stem);
    }

    std::fs::remove_dir_all(&scratch).ok();
}

// Tolerance discipline (WP8 exactness audit, 2026-07-04): every golden here is
// pinned at **exact equality** (`rel = 0`, `abs = 0`) — the produced report
// parses value-identical to the oracle capture — unless a divergence is
// *observed on this golden* and traced to a proven class: a last-printed-digit
// rounding straddle (`%.Nf`/`%g` render floor), a near-zero faer-vs-KLU
// cancellation residual (gated or under a tiny `abs`), the DI accumulation
// floor, or a genuinely unpinnable cell (`Mask`). Preemptive "printing floor"
// tolerances on byte-identical columns are not kept — if a straddle ever fires,
// prove it by decomposition first (both engines' f64 bracketing the print
// boundary), then add the floor. See `tests/TOLERANCE_NOTES.md`.

/// `Export Voltages` (Pascal `ExportVoltages`) on solved IEEE13: per-bus node
/// magnitude/angle/pu, zero-filled to the max node count. Byte-identical
/// Rust↔oracle (incl. the `%6.1f` angle columns) → exact equality.
#[test]
fn export_voltages_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_export("export_voltages", &policy);
}

/// `Export BusCoords` (Pascal `ExportBusCoords`): X/Y of every coord-defined bus.
/// No header row; coordinates are `%-13.11g` (11 sig) loaded from the same file —
/// byte-identical, exact equality.
#[test]
fn export_buscoords_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_export("export_buscoords", &policy);
}

/// `Interpolate` (Pascal `DoInterpolateCmd` → `InterpolateCoordinates` +
/// `CalcBusCoordinates`, WP8.6), gated via `Export BusCoords`: the fixture
/// deck (`tools/golden/report_decks/interp.dss`, replayed from the meta) gives
/// only the anchors src/b1/b5/c2 coordinates via `SetBusXY`; `interpolate`
/// must fill b2/b3/b4/c1 by walking each zone end to the two nearest
/// coordinate-defined anchors and spacing evenly (`Xinc=(X1-X2)/LineCount`).
/// The values pin the ZONE-END ORDER too (the oracle interpolates the c2 end
/// first, so b2/b3 land on the c2→b1 segment). Pure f64 anchor arithmetic —
/// byte-identical, exact equality.
#[test]
fn export_buscoords_interp_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_deck_export("export_buscoords_interp", &policy);
}

/// `Export NodeNames` (Pascal `ExportNodeNames`): `BusName.NodeNum` per line,
/// pure text (compared case-insensitively).
#[test]
fn export_nodenames_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_export("export_nodenames", &policy);
}

/// `Export YNodeList` (Pascal `ExportYNodeList`): node names in Y-matrix order,
/// quoted, pure text.
#[test]
fn export_ynodelist_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_export("export_ynodelist", &policy);
}

// --- WP8.2 sub-step 2a: the real-power element exports ----------------------
// These walk the Sources/PDElements/Faults/PCElements lists calling the mutating
// terminal getters (`Power`/`GetLosses`/`ComputeVterminal`/`ComputeIterminal`).
// All columns are real (kW/kvar/W) — no angle/sequence columns — so the floors
// are the plain fixed-decimal / `%g` printing floors, NOT a physics relaxation:
// the engine's V/I/P physics is pinned to 1e-8 by `corpus_live.rs`; here we gate
// the report layout (header, column set, element order, scaling).
// (tests/TOLERANCE_NOTES.md)

/// `Export Powers` (Pascal `ExportPowers`): per-terminal kW/kvar of every PD then
/// PC element, plus each PD's terminal-1 normal/emergency excess kVA. Every value
/// column is `%11.1f` (one decimal); the underlying kW agree to ~1e-9 rel, far
/// below the print step, and no 0.05-boundary straddle occurs on this golden —
/// byte-identical, exact equality.
#[test]
fn export_powers_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_export("export_powers", &policy);
}

/// `Export Losses` (Pascal `ExportLosses`): per-PD-element total / load / no-load
/// losses in W and var, `%.7g` (7 sig). The proven floor is the **7-sig printing
/// floor**: an observed one-ULP straddle on REG2's 65.34585↔65.34586 W (1e-5 abs
/// = 1.53e-7 rel; a mantissa near 1 could reach ~1.05e-6 rel) — so `rel = 1e-6`
/// is the report's 7-sig render resolution, kept. `abs = 1e-7` absorbs the
/// near-zero no-load/var cancellation cells, where `rel` is meaningless — pure
/// faer-vs-KLU noise-vs-noise (observed up to `1.72e-9` vs `1.275e-8` W). The
/// golden's noise cells top out at 1.28e-8 W while the smallest **real** loss is
/// 9.05e-3 W — a 6-order gap, so `1e-7` (8× over the largest noise cell) can
/// never mask a real loss. (tests/TOLERANCE_NOTES.md)
const LOSSES_REL: f64 = 1e-6;
#[test]
fn export_losses_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: LOSSES_REL,
        abs: 1e-7,
        col_tol: vec![],
    };
    run_feeder_export("export_losses", &policy);
}

/// `Export P_byphase` (Pascal `ExportPbyphase`): per-conductor kW/kvar over the
/// full Yorder. Values are `%10.3f` (three decimals) — a purely **additive** 1-ulp
/// floor with an **observed, decomposition-proven straddle** on the largest
/// conductor (`Transformer.SUB` term-2 phase-3 kW): the live f64s are oracle
/// `−1342.2124999155637` (8.4e-8 kW *above* the −1342.2125 rounding boundary) vs
/// Rust `−1342.2125039696780` (4.0e-6 kW *below* it) — the engines agree to
/// 4.05e-6 kW = 3.0e-9 rel (within the corpus_live 1e-8 power pin), but the value
/// sits on the `%.3f` half-ulp boundary, so the render splits `−1342.212` vs
/// `−1342.213`. So like the `Powers` `%11.1f` floor the whole policy is `rel = 0`
/// / `abs = 0.0011`: no multiplicative band (a per-conductor scale drift ≥ ~0.01%
/// is caught, not masked — mutation-confirmed), the integer NumTerminals/
/// NumConductors/NumPhases columns exact within abs. (tests/TOLERANCE_NOTES.md)
#[test]
fn export_p_byphase_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0011,
        col_tol: vec![],
    };
    run_feeder_export("export_p_byphase", &policy);
}

// --- WP8.4 Show reports -----------------------------------------------------
// `Show` emits Pascal fixed-width text tables (space-padded columns, the odd
// glued trailing comma), not CSV — so these policies use `sep: ' '` (the
// harness whitespace+comma tokenizer) and `header_lines: 0`: every non-blank
// line (prose banner, column header, and data row alike) is tokenized and
// compared token-for-token (text case-insensitively, numbers by tolerance).
// That pins the report **structure** (line count, per-line token count/order)
// on top of the numeric layout. The electrical physics itself is pinned to
// 1e-8 by `corpus_live.rs`; these are report-layout / printing-floor checks.
// (tests/TOLERANCE_NOTES.md)

/// `Show Buses` (Pascal `ShowBuses`): every bus's base kV / `(x,y)` / keep / node
/// list. All columns are **input** data (base kV = `kVBase·√3` `%7.3f`, coords
/// `%-13.11g`, integer node counts/numbers) — identical on both engines, so the
/// floor is just the `%7.3f` printing resolution (`rel = 0`, `abs = 1e-3`).
#[test]
fn show_buses_matches_oracle() {
    let policy = ExportPolicy {
        sep: ' ',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_show("show_buses", &policy);
}

/// `Show Taps` (Pascal `ShowRegulatorTaps`): per-RegControl tap/min/max/step
/// (`%8.5f`), integer position/winding, direction/cogen text. The tap fractions
/// are exact discrete decisions (the timeseries_controls/checkpoint gates pin `tap_number`
/// exactly), so they match to the last `%8.5f` digit — exact equality.
#[test]
fn show_taps_matches_oracle() {
    let policy = ExportPolicy {
        sep: ' ',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_show("show_taps", &policy);
}

/// `Show Losses` (Pascal `ShowLosses`): per-PD kW (`%10.5f`) / `% of Power`
/// (`%8.2f`) / kvar (`%.6g`) plus the line/transformer/total aggregates. Every cell
/// is byte-identical Rust↔oracle → **exact equality** (`rel = 0`, `abs = 0`), except
/// the near-zero **kvar residual** (col 3) of a (near-)lossless-reactive PD element,
/// where the loss collapses to a ~1e-14 kvar faer-vs-KLU cancellation (`-1.45519e-14`
/// vs `-4.36557E-14`, printed at 6 sig). That cell is self-gated on `|kvar| < 1e-9`
/// (well below any real reactive loss). The loss physics is pinned to 1e-8 by
/// `corpus_live.rs`.
#[test]
fn show_losses_matches_oracle() {
    let policy = ExportPolicy {
        sep: ' ',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![ColTol {
            sel: ColSel::Index(3), // kvar
            rel: 0.0,
            abs: 0.0,
            gate: Some(GateSpec::Col(3, 1e-9)), // skip a near-zero kvar cancellation
        }],
    };
    run_feeder_show("show_losses", &policy);
}

/// `Show Voltages` (Pascal `ShowVoltages` case 0 + `WriteSeqVoltages`): the
/// symmetrical-component voltages by bus — `V1 (kV)` / p.u. / `V2 (kV)` / `%V2/V1`
/// / `V0 (kV)` / `%V0/V1`, all `%9.4g`. Every **significant** cell is byte-identical
/// Rust↔oracle → **exact equality** (`rel = 0`, `abs = 0`). The only exceptions are
/// the near-zero **symmetrical-component residuals** of a balanced bus — `V2`
/// (col 3) and `V0` (col 5), e.g. `sourcebus`'s `V0 = 4.319e-9` vs `4.32E-9` — a
/// faer-vs-KLU last-digit cancellation of a ~2.4 kV difference collapsing to ~1e-9 kV
/// (and its knock-on in the `%V2/V1`/`%V0/V1` ratios). Those cells are genuinely not
/// comparable, so each is gated on **its own** near-zero magnitude (`< 1e-6 kV`,
/// provably between the ~1e-9 residual and the smallest real seq cell, `V2 = 8e-5`
/// on bus 650, which stays exact-pinned).
#[test]
fn show_voltages_matches_oracle() {
    // Self-gate the near-zero seq residuals: V2 (col 3) / %V2V1 (col 4) on |V2|;
    // V0 (col 5) / %V0V1 (col 6) on |V0|.
    let near_zero = |col: usize, on: usize| ColTol {
        sel: ColSel::Index(col),
        rel: 0.0,
        abs: 0.0,
        gate: Some(GateSpec::Col(on, 1e-6)),
    };
    let policy = ExportPolicy {
        sep: ' ',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![
            near_zero(3, 3),
            near_zero(4, 3),
            near_zero(5, 5),
            near_zero(6, 5),
        ],
    };
    run_feeder_show("show_voltages", &policy);
}

/// `Show Currents` (Pascal `ShowCurrents` case 0 + `WriteSeqCurrents`/`GetI0I1I2`):
/// per-element sequence currents — `I1`/`I2`/`I0` (`%10.5g`, 5 sig) + `%I2/I1`/
/// `%I0/I1`/`%Normal`/`%Emergency` (`%8.2f`, columns 4/6/7/8). Every ungated cell
/// is **exact** (`rel = 0`, `abs = 0`); the only skipped cells are the near-zero
/// cancellation residuals (self-gated magnitudes) and the ratio cells over a
/// residual denominator: `%I2/I1` = `100·I2/I1` of a switch's floating terminal
/// (`I1 ≈ 1.8e-12 A`) is faer-vs-KLU noise, where the two engines print `53.55`
/// vs `147.66`. The `1e-6 A` threshold is **provably** between that noise floor
/// and the smallest *real* current in the report (`Line.671680`, `I1 = 5.8e-4 A`,
/// whose `%I2/I1 = 1.76` IS checked): an 8-order gap with nothing in between, so
/// `1e-6` can never gate a physical current (a coarser `1e-3` would wrongly skip
/// the real `5.8e-4` row). Current physics is pinned to 1e-8 by `corpus_live.rs`.
/// (tests/TOLERANCE_NOTES.md)
#[test]
fn show_currents_matches_oracle() {
    let pctcol = |i: usize, gate: Option<GateSpec>| ColTol {
        sel: ColSel::Index(i),
        rel: 0.0,
        abs: 0.0,
        gate,
    };
    let policy = ExportPolicy {
        sep: ' ',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![
            // Near-zero seq-current **magnitudes** self-gated (incl. exact zero via
            // `MinCols(c,c,…)`): a balanced element's I2/I0 collapse to a ~2e-11 A
            // faer-vs-KLU cancellation (printed at 5 sig), and a floating switch
            // terminal's I1 to ~1e-12 A — while the ratio columns print `0.00`
            // (`%8.2f`) → identical. `< 1e-6 A` sits between the residuals and the
            // smallest real seq current.
            pctcol(2, Some(GateSpec::MinCols(2, 2, 1e-6))), // I1
            pctcol(3, Some(GateSpec::MinCols(3, 3, 1e-6))), // I2
            pctcol(5, Some(GateSpec::MinCols(5, 5, 1e-6))), // I0
            pctcol(4, Some(GateSpec::Col(2, 1e-6))),        // %I2/I1, gated on near-zero I1
            pctcol(6, Some(GateSpec::Col(2, 1e-6))),        // %I0/I1, gated on near-zero I1
        ],
    };
    run_feeder_show("show_currents", &policy);
}

/// `Show Powers` (Pascal `ShowPowers` case 0): per-element sequence powers —
/// `P1`/`Q1`/`P2`/`Q2` (`%11.1f`) + `P0`/`Q0` (`%8.1f`) + the PD terminal-1 excess
/// power, plus the `Total Circuit Losses` footer. Every value is fixed 1-decimal;
/// the engines agree to ~1e-7 rel, far below the print step — byte-identical,
/// exact equality (same as `Export Powers`). A missing `×0.003` scale or `×3`
/// positive-seq factor would shift values far past any print step and fail loudly.
#[test]
fn show_powers_matches_oracle() {
    let policy = ExportPolicy {
        sep: ' ',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_show("show_powers", &policy);
}

/// `Show Voltages LN Node` (Pascal `ShowVoltages` case 1 + `WriteBusVoltages`):
/// line-ground **and** line-line voltages by bus & node. Per node row the tokens
/// are `BUS Node VLN /_ Angle pu BaseKV [NodeNodeLL VLL /_ Angle pu]` — every
/// numeric column (`%10.5g`/`%9.5g` magnitudes/pu, `%9.3f` input base kV, `%6.1f`
/// angles) is byte-identical → exact equality (the `PadDots` name column's pure
/// dot-runs are dropped by the comparator). Voltage physics is pinned to 1e-8 by
/// `corpus_live.rs`.
#[test]
fn show_voltages_node_matches_oracle() {
    let policy = ExportPolicy {
        sep: ' ',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_show("show_voltages_node", &policy);
}

/// `Show Voltages LN Elem` (Pascal `ShowVoltages` case 2 + `WriteElementVoltages`):
/// node-ground voltages by circuit element. Each conductor row is `BUS (nref)
/// nodenum VLN (pu) /_ Angle`; the `nref` and `pu` are **parenthesised** (and split
/// on the interior padding into `(` + `n)` tokens) so the tokenizer text-compares
/// them — both are bit-pinned (`nref` is the exact node order, `pu` is 4-sig of a
/// 1e-8-pinned voltage). The bare `VLN` (`%13.5g`) and the `%6.1f` angle are
/// byte-identical → exact equality.
#[test]
fn show_voltages_elem_matches_oracle() {
    let policy = ExportPolicy {
        sep: ' ',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_show("show_voltages_elem", &policy);
}

/// `Show Currents Y Elem` (Pascal `ShowCurrents` case 1 + `WriteTerminalCurrents`,
/// `ShowResidual = TRUE`): per-terminal, per-conductor branch currents + the PD
/// residual row. Each row is `BUS nodenum |I| /_ Angle = Re +j Im`; numeric tokens
/// are `|I|` (idx 2, `%13.5g`), `Angle` (`%6.1f`), `Re`/`Im` (idx 6/8, `%9.5g`).
/// Every ungated cell is **exact**; only the near-zero cancellation-residual rows
/// (`|I| < 1e-4 A`) are gated out. The `1e-4 A` threshold (higher than
/// `show_currents`' `1e-6`) brackets **this** report's noise: the residual rows
/// push the near-zero floor up to ~1.08e-5 A (the 633/634 near-balanced-
/// transformer residuals, `|I| = 1.12e-6`/`1.08e-5 A`, whose phase is arbitrary
/// faer-vs-KLU cancellation noise), while the smallest **real** current is
/// `Line.671680`'s `5.72e-4 A` (whose angle IS checked) — a ~53× gap with nothing
/// between, so `1e-4` skips only noise. Current physics is pinned to 1e-8 by
/// `corpus_live.rs`.
#[test]
fn show_currents_elem_matches_oracle() {
    // Everything is byte-identical (exact) EXCEPT the near-zero cancellation
    // residual rows (`|I| < 1e-4 A`): the magnitude (col 2), the real/imag parts
    // (cols 6/8) and the angle (after `/_`) all collapse to faer-vs-KLU noise there
    // and are gated on `|I|` (col 2). `1e-4 A` brackets this report's noise floor
    // (~1.08e-5 A) vs the smallest real current (5.72e-4 A).
    // Gate on `|I|` (col 2) `< 1e-4` **including exact zero** (`MinCols(2,2,…)`):
    // the oracle prints an open/grounded conductor's current as exactly `0` while
    // Rust carries a tiny cancellation residual, so a zero-excluding gate would miss
    // it.
    let on_i = |sel: ColSel| ColTol {
        sel,
        rel: 0.0,
        abs: 0.0,
        gate: Some(GateSpec::MinCols(2, 2, 1e-4)),
    };
    let policy = ExportPolicy {
        sep: ' ',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![
            on_i(ColSel::Index(2)),                // |I|
            on_i(ColSel::Index(6)),                // Re
            on_i(ColSel::Index(8)),                // Im
            on_i(ColSel::AfterToken("/_".into())), // angle
        ],
    };
    run_feeder_show("show_currents_elem", &policy);
}

/// `Show Elements` (Pascal `ShowElements` + `WriteElementRecord`, default PD/PC
/// form): the element ↔ bus-connection listing. Every column is a **name** — the
/// quoted `"Class.Name"` and the terminal bus names — so all tokens are text
/// (identifiers, compared case-insensitively) except numeric-looking bus names
/// (`650`, `633`) which compare as exact integers. Pure structural / input data,
/// identical on both engines: `rel = 0`, `abs = 0`. (`run_feeder_show` reads the
/// main `Elements.txt`; the `_Disabled` companion — header-only here, all elements
/// enabled — is written but not compared.)
#[test]
fn show_elements_matches_oracle() {
    let policy = ExportPolicy {
        sep: ' ',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_show("show_elements", &policy);
}

/// `Show Elements Line` (Pascal `ShowElements` **class-filter** form): the enabled
/// elements of class `Line`, uppercased names, one per row. Exercises the
/// `SetObjectClass` + per-object enabled/disabled routing path (distinct from the
/// default PD/PC form). All-name tokens, exact (`rel = 0` / `abs = 0`).
#[test]
fn show_elements_class_matches_oracle() {
    let policy = ExportPolicy {
        sep: ' ',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_show("show_elements_class", &policy);
}

/// `Show Voltages LL Node` (Pascal `ShowVoltages` case 1 + `WriteBusVoltages`,
/// `LL = TRUE`): the **line-line** node form — the `ll` branch with its distinct
/// header and `/√3` pu scaling (and the `if kk > 0` line-line row emission). Same
/// tolerance/selector shape as the L-N twin (`AfterToken("/_")` angle floor).
#[test]
fn show_voltages_ll_node_matches_oracle() {
    let policy = ExportPolicy {
        sep: ' ',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_show("show_voltages_ll_node", &policy);
}

/// `Show Powers e` (Pascal `ShowPowers` case 1): per-terminal, per-conductor branch
/// power flow `BUS node kW +j kvar kVA PF`, with the `... TERMINAL TOTAL` per
/// terminal (incl. the 1-phase/2-terminal PD floating special case). The kW/kvar/kVA
/// Every column (kW/kvar/kVA `%8.1f`, PF `%8.4f`) is checked for **exact equality**
/// (`rel = 0`, `abs = 0`): the golden is the oracle's captured bytes and the Rust
/// engine is deterministic, so the powers — pinned to ~1e-9 Rust↔oracle, far below
/// any print step — round to *byte-identical* strings; there is nothing to tolerate,
/// they are equal or it is a regression (verified: the whole report passes at
/// `abs = 0`). The **only** exception is the PF of a near-purely-reactive / -real /
/// `S ≈ 0` conductor (`min(|kW|, |kvar|)` ≈ 0), where the powers *themselves* differ
/// — the oracle's `S.re`/`S.im` is exactly 0 → PF 1.0, while a faer-vs-KLU
/// cancellation residual is tiny-nonzero → near-zero/sign-flipped PF; those cells are
/// genuinely not comparable and are gated out. Power physics is pinned to 1e-8 by
/// `corpus_live.rs`.
#[test]
fn show_powers_elem_matches_oracle() {
    let policy = ExportPolicy {
        sep: ' ',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![ColTol {
            // The PF column carries only the degenerate-power gate; its tolerance is
            // the exact default.
            sel: ColSel::Index(6),
            rel: 0.0,
            abs: 0.0,
            gate: Some(GateSpec::MinCols(2, 4, 1e-3)),
        }],
    };
    run_feeder_show("show_powers_elem", &policy);
}

/// `Show Ratings` (Pascal `ShowRatings`): each PD element's `"FullName",
/// normamps=<n>,  <e>  !Amps`. The `normamps=<n>` token is text (glued to the
/// prefix, `%-.4g`) and `<e>` is numeric — both are **input** ratings, identical on
/// both engines, so `rel = 0` / `abs = 0` (the `%-.4g` render must match exactly).
#[test]
fn show_ratings_matches_oracle() {
    let policy = ExportPolicy {
        sep: ' ',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_show("show_ratings", &policy);
}

/// `Show EventLog` (Pascal `ShowEventLog` = `EventStrings.SaveToFile`): the full
/// `Set Log=yes` LogThisEvent marker stream + the regulators' `AppendToEventLog`
/// tap-change lines, over the same event-logging daily-3 IEEE13 fixture as
/// `export_eventlog` (a swinging load moves all three regulators). Pins that the
/// `Show` path emits the identical log the `Export` path does — line-for-line vs the
/// oracle, the non-integer tap values (`CHANGED n TAPS TO <pu>`) exercising the
/// numeric-token compare. Exact equality — the tap decisions are exact (pinned
/// by the timeseries_controls `daily_ieee13` gate).
#[test]
fn show_eventlog_matches_oracle() {
    let policy = ExportPolicy {
        sep: ' ',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_show("show_eventlog", &policy);
}

/// `Show monitor m_vi` (Pascal `ShowOptions.pas` case 10 → `TranslateToCSV`): the
/// named monitor's in-memory sample buffer written to its CSV via the `Show`
/// dispatcher — the same content the `Export Monitors` path produces
/// ([`export_monitors_match_oracle`]). Pins the `Show monitor` dispatch + file
/// naming against the oracle on the daily-solved monitor fixture (CSV, `sep=','`,
/// header verbatim; the f32 `%-.6g` values byte-identical — exact equality).
#[test]
fn show_monitor_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_show("show_monitor", &policy);
}

/// `Show Mismatch` (Pascal `ShowNodeCurrentSum`): the per-node KCL current-sum
/// mismatch. A **value** golden that pins the substantive column — `Max Current`
/// (the largest single terminal current at each node, a 1e-8-pinned magnitude,
/// [`ColSel::FromEnd`]`(0)`) — plus the node number and bus name / row order
/// (`ExactOrdered` also pins the exact `num_nodes + 1` data-row count). The two
/// residual columns are **gated out** ([`GateSpec::Mask`]): `Current Sum`
/// (`FromEnd(2)`) and `%error` (`FromEnd(1)`) are the per-node KCL residual
/// (Σ terminal currents ≈ 0), an inherent faer-vs-KLU cancellation floor that
/// differs between the two engines by construction and is not cross-engine
/// comparable. The `FromEnd` selectors are robust to `"System Ground"` splitting
/// into two tokens (which shifts the leading columns by one vs a bus-name row).
/// `Max Current` is held to `abs = 1.1e-5` — not exact — because at `%10.5f` its
/// last digit genuinely straddles a rounding boundary on this feeder (a `473.76972`
/// vs `473.76971` cell): the current agrees Rust↔oracle to ~5e-6 (1e-8 rel of a
/// ~473 A value), comparable to the `1e-5` render step, so the two solves round to
/// opposite sides — the `1.1 × 10⁻⁵` `%.5f` render floor (like the `0.11`/`0.011`
/// columns), not a slack tolerance.
#[test]
fn show_mismatch_matches_oracle() {
    let residual = |n: usize| ColTol {
        sel: ColSel::FromEnd(n),
        rel: 0.0,
        abs: 0.0,
        gate: Some(GateSpec::Mask),
    };
    let policy = ExportPolicy {
        sep: ' ',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 1.1e-5, // Max Current: the %10.5f render floor (straddle); ints/text exact
        col_tol: vec![residual(2), residual(1)],
    };
    run_feeder_show("show_mismatch", &policy);
}

/// The AutoTrans special cases of the element-form `Show` reports (WP8.8 exit
/// sweep; Pascal `ShowResults.pas` — `WriteTerminalCurrents:604`, `ShowPowers`
/// case 1 `:1190`, `ShowNodeCurrentSum:3636`): `Ntimes = Nphases` rows per
/// terminal instead of `NConds`, the per-terminal `Inc(k, Ntimes)` block-skip in
/// currents/mismatch, and the DEAD post-loop `Inc` in `ShowPowers` (terminal 2
/// re-reads the first conductor block — reproduced). A well-conditioned
/// physical-source snapshot deck (the WPG.15 `autotrans_reg.dss` model minus the
/// control arm), so the digits pin exactly; policies mirror the feeder twins.
#[test]
fn show_powers_elem_autotrans_matches_oracle() {
    let policy = ExportPolicy {
        sep: ' ',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![ColTol {
            sel: ColSel::Index(6),
            rel: 0.0,
            abs: 0.0,
            gate: Some(GateSpec::MinCols(2, 4, 1e-3)),
        }],
    };
    let (oracle, rust, scratch) = produce_deck_show("show_powers_elem_autotrans");
    compare_export(&oracle, &rust, &policy, "show_powers_elem_autotrans");
    // The per-family whitespace layouts (WP8.8: Sources `%s %4d` one-space
    // rows, PC width-6 rows + `kW   +j  kvar` header + `'  TERMINAL TOTAL '`
    // label — `ShowResults.pas:1128/1162/1240/1264/1297/1302`), pinned
    // BYTE-EXACT against the oracle golden (audit-tests follow-up): the
    // tokenizing comparator above deliberately collapses whitespace, so the
    // layout-bearing lines are compared verbatim here. (Same production as
    // the numeric compare — a second `produce_deck_show` in a sibling test
    // would race on the shared per-stem scratch dir.)
    // A row whose |S| rounds to 0.0 kVA is a pure faer-vs-KLU cancellation
    // residual — its re/im can render `0.0` vs `-0.0` (arbitrary sign), so
    // those rows are numerically pinned by the tokenizing twin (0.0 == -0.0)
    // and excluded from the byte compare here (the layout is amply pinned by
    // the non-degenerate rows).
    let degenerate = |l: &str| l.split_whitespace().rev().nth(1) == Some("0.0");
    let select = move |s: &str, pred: fn(&str) -> bool| -> Vec<String> {
        s.lines()
            .filter(|l| pred(l) && !degenerate(l))
            .map(str::to_string)
            .collect()
    };
    type LinePred = fn(&str) -> bool;
    let preds: [(&str, LinePred); 3] = [
        ("column headers", |l| l.contains(" Phase ")),
        ("terminal totals", |l| l.contains("TERMINAL TOTAL")),
        // Every per-conductor row (Sources 1-space, PD 2-space, PC width-6):
        // starts with an uppercased bus name of this deck.
        ("bus rows", |l| {
            ["SRC", "HIGH", "LOW", "FDR"]
                .iter()
                .any(|b| l.starts_with(b))
        }),
    ];
    for (what, pred) in preds {
        let o = select(&oracle, pred);
        let r = select(&rust, pred);
        assert!(!o.is_empty(), "{what}: golden must contain such lines");
        assert_eq!(o, r, "{what}: byte-exact layout");
    }
    std::fs::remove_dir_all(&scratch).ok();
}

/// See [`show_powers_elem_autotrans_matches_oracle`]; the currents twin
/// (`WriteTerminalCurrents` with residual rows). Same near-zero gating as
/// [`show_currents_elem_matches_oracle`].
#[test]
fn show_currents_elem_autotrans_matches_oracle() {
    let on_i = |sel: ColSel| ColTol {
        sel,
        rel: 0.0,
        abs: 0.0,
        gate: Some(GateSpec::MinCols(2, 2, 1e-4)),
    };
    let policy = ExportPolicy {
        sep: ' ',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![
            on_i(ColSel::Index(2)),                // |I|
            on_i(ColSel::Index(6)),                // Re
            on_i(ColSel::Index(8)),                // Im
            on_i(ColSel::AfterToken("/_".into())), // angle
        ],
    };
    run_deck_show("show_currents_elem_autotrans", &policy);
}

/// See [`show_powers_elem_autotrans_matches_oracle`]; the node-current-sum twin
/// (`ShowNodeCurrentSum` — the AutoTrans arm sums only `Nphases` conductors per
/// terminal into the node totals). Same residual gating as
/// [`show_mismatch_matches_oracle`].
#[test]
fn show_mismatch_autotrans_matches_oracle() {
    let residual = |n: usize| ColTol {
        sel: ColSel::FromEnd(n),
        rel: 0.0,
        abs: 0.0,
        gate: Some(GateSpec::Mask),
    };
    let policy = ExportPolicy {
        sep: ' ',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 1.1e-5, // Max Current: the %10.5f render floor (see the feeder twin)
        col_tol: vec![residual(2), residual(1)],
    };
    run_deck_show("show_mismatch_autotrans", &policy);
}

/// `Show Variables` (Pascal `ShowVariables`): every PC element's present dynamic
/// state variables. Exercised on IEEE13 + a Generator (6 variables:
/// `Frequency`/`Theta`/`Vd`/`PShaft`/`dSpeed`/`dTheta`) so the `ELEMENT:` /
/// `No. of variables:` header and the per-variable `  name = %-.6g` value path all
/// run (plain IEEE13 has no PC element with variables). The variable **names** are
/// text (pinned exactly — a rename regression fails); the values (`Frequency = 60`,
/// the rest 0 in a snapshot) parse numeric at the tight default.
#[test]
fn show_variables_matches_oracle() {
    let policy = ExportPolicy {
        sep: ' ',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_show("show_variables", &policy);
}

/// `Show Result` (Pascal `ShowResult`): the `@result` parser var (always `null` in
/// the pinned PM-build oracle). Pins the produced-file **name** (`<case>_Result.csv`,
/// not `.txt` — a filename regression the `run_feeder_show` `ends_with` check catches)
/// and the one-line content via the `Show` dispatch.
#[test]
fn show_result_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_show("show_result", &policy);
}

/// `Show Convergence` (Pascal `Solution.WriteConvergenceReport`): the per-node
/// saved error / `|V|` / `Vbase` snapshot + the `Max Error` footer — **exact
/// equality on every column** (`rel = 0`, `abs = 0`). `|V|` (`VmagSaved`,
/// `Str(v:14)` — 7 significant figures) is exact too: the faer-vs-KLU node-voltage
/// gap is orders of magnitude below the 1e-7 print-rounding step, and the produced
/// file is byte-identical to the oracle golden. Unlike `show_mismatch`'s
/// `Max Current` (f64 gap comparable to its render step, straddle observed), no
/// printing floor is warranted here; if a 7th-digit straddle ever fires, prove it
/// by decomposition (both engines' f64 `vmag_saved` bracketing the print-rounding
/// boundary) before adding a `col_tol`.
#[test]
fn show_convergence_matches_oracle() {
    let policy = ExportPolicy {
        sep: ' ',
        header_lines: 4,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_show("show_convergence", &policy);
}

/// `Show Y` (Pascal `ShowY`): the assembled system Y, lower triangle by columns,
/// `[row,col] = G + jB` (`%13.10g`). The assembled Y is stamped from the
/// FPC-faithful element YPrims (bit-exact on the LineCode-based IEEE13, pinned
/// entry-by-entry by the checkpoint/live gates), so at 10 sig figs G and B are
/// byte-identical Rust↔oracle → **exact equality** (`rel = 0`, `abs = 0`). The
/// row/col indices are the exact node order (column-major, matching KLU's
/// `GetTripletMatrix`).
#[test]
fn show_y_matches_oracle() {
    let policy = ExportPolicy {
        sep: ' ',
        header_lines: 2,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_show("show_y", &policy);
}

/// `Show controlqueue` (Pascal `ControlQueue.WriteQueue`): the pending
/// control-action queue. After a converged snapshot solve the queue is drained,
/// so the report is the header row alone — pinned exactly (a structural / dispatch
/// + `.csv`-filename check; a mid-sequence solve would add per-action rows).
#[test]
fn show_controlqueue_matches_oracle() {
    let policy = ExportPolicy {
        sep: ' ',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_show("show_controlqueue", &policy);
}

/// `Show kvbasemismatch` (Pascal `ShowkVBaseMismatch`) on plain IEEE13: every load
/// is within 10% of its bus base, so the report is the `!!!  LOAD VOLTAGE BASE
/// MISMATCHES` family header alone (no generators on IEEE13) — pins the
/// header-emission logic. The value/generator branches are pinned by
/// [`show_kvbasemismatch_vals_matches_oracle`].
#[test]
fn show_kvbasemismatch_matches_oracle() {
    let policy = ExportPolicy {
        sep: ' ',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_show("show_kvbasemismatch", &policy);
}

/// `Show kvbasemismatch` on IEEE13 + four synthesized kV-base-mismatched elements
/// (`KVBASE_POST`): exercises the mismatch-line formatting (`!!!!! Voltage Base
/// Mismatch …` + the `!setkvbase …` / `!<elem>.kV=…` follow-ups) across both the
/// line-line (`kVBase·√3`) and 1-phase line-neutral forms, for a load **and** a
/// generator (the GENERATOR family header). The printed kV values (`%.6g`) are
/// deterministic → **exact equality** (`rel = 0`, `abs = 0`).
#[test]
fn show_kvbasemismatch_vals_matches_oracle() {
    let policy = ExportPolicy {
        sep: ' ',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_show("show_kvbasemismatch_vals", &policy);
}

/// `Show Meters` (Pascal `ShowMeters`): the EnergyMeter register table on the
/// daily-solved metered IEEE13 (`REGISTER_A_POST` fixture — the same meter path
/// `corpus_live.rs` + `export_meters` pin). The register legend + the `Reg i`
/// column header are fixed text; the per-register values print `%10.0f` (integer),
/// so — matching the oracle to ~1e-8 rel — every rounded cell is identical →
/// **exact equality** (`rel = 0`, `abs = 0`).
#[test]
fn show_meters_matches_oracle() {
    let policy = ExportPolicy {
        sep: ' ',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_show("show_meters", &policy);
}

/// `Show Generators` (Pascal `ShowGenMeters`): the Generator register table on the
/// generator fixture (`REGISTER_B_POST` — g1/g2 enabled, g3 disabled so the
/// enabled-filter is exercised: g3 must NOT appear). Same `%10.0f` integer
/// registers as `export_generators` → **exact equality** (`rel = 0`, `abs = 0`).
///
/// Note (WP8.4 step-8 audit F1): g1/g2's `$` register is **exactly 7.5**, sitting on
/// the `%10.0f` rounding half-boundary → rendered `8`. This is stable, not a
/// knife's-edge: the `$` register derives from the **stiff, clean** `kWh = 300`
/// (a `model=1` generator holds P = 100 kW, so ∫P dt = 300 exactly, bit-identical on
/// both engines — no faer-vs-KLU residual), and `7.5 → 8` under *both* round-half-to-
/// even and round-half-away, so the exact compare cannot straddle. (If the Generator
/// register core ever integrated *measured* terminal power instead, `$` could drift
/// to `7.4999…`; that would be a real regression this pin would correctly catch.)
#[test]
fn show_generators_matches_oracle() {
    let policy = ExportPolicy {
        sep: ' ',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_show("show_generators", &policy);
}

/// `Show Overloads` (Pascal `ShowOverloads`): the PD-element symmetrical-component
/// overload report on the synthesized `ovl` deck (a small-`normamps` line under a
/// heavy load). Columns are `Element Term I1 IOver %Normal %Emerg I2 %I2/I1 I0
/// %I0/I1` (`%3d`/`%8.1f`/`%8.2f`); the seq currents are pinned to 1e-8 by
/// `corpus_live.rs`, and on this clean deck every printed cell is byte-identical
/// Rust↔oracle → **exact equality** (`rel = 0`, `abs = 0`). Layout differs from
/// `Export Overloads` (no `kVAOver` column).
#[test]
fn show_overloads_matches_oracle() {
    let policy = ExportPolicy {
        sep: ' ',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_deck_show("show_overloads", &policy);
}

/// `Show Overloads`, unbalanced (`ovl2` deck): single-phase loads on 3-phase lines
/// drive nonzero I2/I0 (pins the `phase_to_sym` decomposition, not just the balanced
/// all-zero columns), and a `normamps=0` line forces the degenerate branch (IOver /
/// %Normal print the literal `0.0` while %Emerg is computed). Exact equality.
#[test]
fn show_overloads_unbal_matches_oracle() {
    let policy = ExportPolicy {
        sep: ' ',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_deck_show("show_overloads_unbal", &policy);
}

/// `Show Unserved` (Pascal `ShowUnserved`, normal criterion): the `uns` deck sags a
/// load below `NormalMinVolts`, latching a nonzero `EEN_Factor`. Columns are `name
/// bus kW EEN UE` (`%8.0f`/`%9.3f`); the `ExceedsNormal` factors are the same path
/// `export_unserved` pins → **exact equality** (`rel = 0`, `abs = 0`).
#[test]
fn show_unserved_matches_oracle() {
    let policy = ExportPolicy {
        sep: ' ',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_deck_show("show_unserved", &policy);
}

/// `Show Unserved ue` (emergency criterion): the `uns2` deck deep-sags a load below
/// `EmergMinVolts` (a nonzero `UE_Factor` via the `Unserved` path) while a healthy
/// load is **excluded** — pinning both the `ue_only` branch (a nonempty trailing
/// param → `UE_Only = TRUE`, `ShowOptions.pas:322`) and the criterion filter. Exact
/// equality.
#[test]
fn show_unserved_ue_matches_oracle() {
    let policy = ExportPolicy {
        sep: ' ',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_deck_show("show_unserved_ue", &policy);
}

/// `Show Overloads` niche-branch coverage (audit-tests step-9 follow-up): a
/// dedicated deck with a **1-phase** overloaded line (`normamps=5`, `emergamps=0`)
/// and a small overloaded shunt **capacitor**. Exercises the three `show_overloads`
/// branches `ovl`/`ovl2` miss: the `Nphases < 3` symmetrical-component fallback
/// (`I0 = I2 = 0`, the 1-phase line's `%I2/I1`/`%I0/I1` print `0.0`), the
/// `EmergAmps <= 0` degenerate `%Emerg` literal (`     0.0`), and the capacitor-skip
/// (`c1` carries ≈27.8 A > its `normamps=1` yet must NOT appear — `(CLASSMASK and
/// DSSObjType) <> CAP_ELEMENT`). Exact equality (`rel = 0`, `abs = 0`).
#[test]
fn show_overloads_1ph_matches_oracle() {
    let policy = ExportPolicy {
        sep: ' ',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_deck_show("show_overloads_1ph", &policy);
}

/// `Show Unserved` **normal** criterion, exclusion coverage (audit-tests step-9
/// follow-up): run on the `uns2` deck — the deep-sag `ld1` is over its normal
/// criterion (`ExceedsNormal`, a nonzero `EEN_Factor`) while the healthy `ld2` is
/// **excluded**. `show_unserved` (deck `uns`) has a single load, so only the UE path
/// pinned an exclusion; this pins the **normal** path's exclusion (a distinct
/// `exceeds_normal` filter from the UE `unserved`). Exact equality.
#[test]
fn show_unserved_normal_matches_oracle() {
    let policy = ExportPolicy {
        sep: ' ',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_deck_show("show_unserved_normal", &policy);
}

/// `Show Faults` (Pascal `ShowFaultStudy`): the three-section FaultStudy report on
/// the `solve mode=faultstudy`-solved IEEE13 feeder. Section 1 (all-node bolted
/// currents + X/R), section 2 (SLG fault current + pu node voltages), section 3
/// (L-L fault via a `GFault` scratch inversion) — **exact equality** (`rel = 0`,
/// `abs = 0`).
///
/// Provenance of the numbers (audit-tests WP8.4 step 10): the *aggregate* fault
/// currents / sequence Z are independently pinned by `export_faultstudy`
/// (3-Phase/1-Phase/L-L per bus, `%.2f`, exact) and `export_seqz`, over this same
/// `solve mode=faultstudy` fixture. The **per-node** amps, the `X/R` column and the
/// full section-2/3 pu-voltage matrices are pinned by *this* golden alone, at the
/// report's print precision (`%15.0f`/`%12.0f` amps, `%5.1f` X/R, `%10.3f` pu). The
/// underlying short-circuit state carries the ~1e-8 faer-vs-KLU floor (`Zsc` comes
/// from unit-injection re-solves of the factored Y — not a bit-exact assembly), so
/// exact equality holds because on this feeder no printed cell lands within that
/// floor of a `%.Nf` rounding boundary (a feeder property, not a guarantee); a
/// future straddle is an investigation, never a mask (CLAUDE.md). `corpus_live.rs`
/// does **not** cover this — it snapshot-solves and never enters faultstudy mode.
/// The degenerate cold-solve path (unallocated `Zsc`, which access-violates the
/// oracle) is pinned safe by the `fault_study::tests::fault_study_cold_solve_is_safe`
/// unit test (no golden — the oracle crashes there).
#[test]
fn show_faultstudy_matches_oracle() {
    let policy = ExportPolicy {
        sep: ' ',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_show("show_faultstudy", &policy);
}

/// `Show Faults` on an **unbased** circuit (WP8.4 step-10 audit-tests follow-up):
/// a deck with no `Set Voltagebases` / `CalcVoltageBases`, so every bus keeps
/// `kVBase = 0` and sections 2 & 3 render the `%10.1f` "L-N Volts if no base"
/// branch (raw volts) instead of the `%10.3f` per-unit form the based IEEE13 golden
/// exercises. A prior `solve mode=snap` lets FaultStudy run (a **cold**
/// `solve mode=faultstudy` access-violates the oracle — that degenerate path is the
/// no-golden `fault_study::tests::fault_study_cold_solve_is_safe` unit test). The
/// raw-volt cells (`%10.1f`, thousands of volts) and the section-1 amps/X-R are
/// byte-identical Rust↔oracle → **exact equality** (`rel = 0`, `abs = 0`); the
/// single-phase `B2` lateral also pins a 1-node bus row.
#[test]
fn show_faultstudy_unbased_matches_oracle() {
    let policy = ExportPolicy {
        sep: ' ',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_deck_show("show_faultstudy_unbased", &policy);
}

/// `Show Yprim` (Pascal `ShowYPrim`): the **active** circuit element's primitive Y
/// (lower-triangle `G` then `jB`, `%13.10g`). `Select line.650632` makes the line
/// active; the report writes `Line_650632_Yprim.txt` (NO `CircuitName_` prefix, so
/// the `run_feeder_show` `*_Yprim.txt` glob also pins that filename convention). The
/// IEEE13 lines are LineCode-based → the primitive Y is bit-exact Rust↔oracle, so
/// every `G`/`jB` cell is byte-identical → **exact equality** (`rel = 0`, `abs = 0`).
/// Also exercises the newly-ported `Select` command (the active-element surface).
#[test]
fn show_yprim_matches_oracle() {
    let policy = ExportPolicy {
        sep: ' ',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_show("show_yprim", &policy);
}

/// `Show Meters` with **two** EnergyMeters (audit F2 coverage): em1 on the feeder
/// head + em2 on the 632-645 lateral. The zones **partition** (em1 stops at em2), so
/// the two data rows carry distinct per-zone registers — pinning that the legend is
/// emitted **once** from the FIRST meter (`meters[0]`) while each row uses its own
/// registers. A regression that printed the legend per-meter, dropped a row, or
/// reused `meters[0]`'s registers for both rows fails `ExactOrdered`. Integer
/// `%10.0f` registers → **exact equality**.
#[test]
fn show_meters_multi_matches_oracle() {
    let policy = ExportPolicy {
        sep: ' ',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_show("show_meters_multi", &policy);
}

/// `Show Meters` / `Show Generators` on a solved IEEE13 with **no** meters /
/// generators (audit F2 coverage): pins the empty-list banner branches —
/// `ShowMeters` emits the `No Energymeter Elements Defined.` line, `ShowGenMeters`
/// emits its two-line banner only. Fixed text (no numbers), diffed exact against the
/// oracle bytes.
#[test]
fn show_meters_none_matches_oracle() {
    let policy = ExportPolicy {
        sep: ' ',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_show("show_meters_none", &policy);
}

/// Companion to [`show_meters_none_matches_oracle`]: the empty-Generators banner.
#[test]
fn show_generators_none_matches_oracle() {
    let policy = ExportPolicy {
        sep: ' ',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_show("show_generators_none", &policy);
}

/// `Show Loops` (Pascal `ShowLoops`) on the **radial** metered IEEE13 — no loops or
/// parallels, so the report is the two header lines only. Pins the header text + the
/// radial (all-`sequence_list`-branches-non-looped) no-op path. Byte-exact: this
/// pure-text report has no `MaxBusNameLength`/`PadDots` backend quirk, so the whole
/// file is reproducible (indentation + trailing spaces), a stronger check than the
/// whitespace-token diff the padded Show tables must use.
#[test]
fn show_loops_matches_oracle() {
    run_feeder_show_exact("show_loops");
}

/// `Show Zone <meter>` (Pascal `ShowMeterZone`) on the radial metered IEEE13 (em1 on
/// Line.650632): the full zone as an indented branch/shunt tree — every PD branch,
/// its shunt loads/caps/transformers, and the meter-as-`SensorObj` annotation
/// (`(Sensor: EnergyMeter.em1)`, set by the WP6.4 zone build). A 32-line tree,
/// compared **byte-exact**: pins the `First`/`GoForward` walk order, the shunt-object
/// attachment per branch, the per-level `TABCHAR` indentation, and the `_<meter>.txt`
/// filename (glob `*_ZoneOut_em1.txt`).
#[test]
fn show_zone_matches_oracle() {
    run_feeder_show_exact("show_zone");
}

/// `Show Loops` on the synthesized **meshed** deck (a 3-line loop b1-b2-b3-b1 + a line
/// parallel to `la`): exercises the PARALLEL/LOOP branches `show_loops` misses on any
/// radial feeder — one `PARALLEL WITH`/`LOOPED TO` line per parallel/looped branch,
/// naming the branch (`Class.UPPERCASE(Name)`) and its `LoopLineObj.FullName` partner.
/// Both engines port the same zone-build loop/parallel detection, so the marked
/// branches + walk order match exactly (byte-exact).
#[test]
fn show_loops_mesh_matches_oracle() {
    run_deck_show_exact("show_loops_mesh");
}

/// `Show Zone` on the meshed deck: the zone tree with the inline `(PARALLEL:Name)` /
/// `(LOOP:FullName)` branch annotations `show_zone` on a radial feeder never emits
/// (note the Pascal inconsistency the port reproduces: PARALLEL uses the bare
/// `LoopLineObj.Name`, LOOP the `FullName`). Byte-exact.
#[test]
fn show_zone_mesh_matches_oracle() {
    run_deck_show_exact("show_zone_mesh");
}

/// `Show Loops` on a **two-meter** deck (audit-tests step-12 follow-up): zone A (m1)
/// loops, zone B (m2) has a parallel line, so **both** meters emit rows. Exercises the
/// multi-meter outer `for &mr in &ckt.energy_meters` loop the single-meter goldens
/// don't — a regression that broke after the first meter, duplicated the header, or
/// mis-attributed the `(mtr)` prefix would fail here. Byte-exact.
#[test]
fn show_loops_multi_matches_oracle() {
    run_deck_show_exact("show_loops_multi");
}

/// `Show Controlled` (Pascal `ShowControlledElements`) on solved IEEE13: the three
/// voltage-regulator `RegControl`s each control a `Transformer`, so the report is
/// three `Transformer.regN, RegControl.regN ` lines — a PD element followed by the
/// control(s) acting on it (via the derived `ckt.controls` scan). Pure text (names
/// only, no numbers, no `MaxBusNameLength`/`PadDots` quirk) → compared **byte-exact**
/// (indentation + trailing spaces), pinning the `, %s ` control-list format, the
/// creation-order control listing, and the `*_ControlledElements.csv` filename.
#[test]
fn show_controlled_matches_oracle() {
    run_feeder_show_exact("show_controlled");
}

/// `Show Controlled` **multi-control** coverage (audit follow-up, step 13): a
/// synthesized deck where `Line.l1` carries a Recloser **then** a Relay, `Line.l2`
/// two SwtControls, `Line.l3` a Fuse, and `Capacitor.cap1` a CapControl. Pins the
/// repeated-control `, %s , %s ` loop AND its creation-order ordering (`ckt.controls`
/// order) — which the single-control-per-PD feeder golden never exercises — plus all
/// FIVE PD-targeting `controlled_element()` overrides the feeder golden misses
/// (`swt_control`/`recloser`/`relay`/`fuse`/`cap_control`; only `reg_control` is hit
/// there). The **fuse** line is the audit-code Major regression guard: Fuse was the
/// one `TControlElem` subclass whose override was initially omitted (it lives under
/// `elements/pd/fuse/`, not `elements/control/`), so a fuse-switched line silently
/// vanished from the report. Byte-exact.
#[test]
fn show_controlled_multi_matches_oracle() {
    run_deck_show_exact("show_controlled_multi");
}

/// Drive one `Show LineConstants` case: replay `<stem>`'s deck, issue its `show
/// lineconstants <args>`, and verify **both** produced files byte-exact — the report
/// `<case>_LineConstants.txt` (via the deck harness) and the `LineConstantsCode.dss`
/// LineCode script (no `<case>_` prefix, read directly from the scratch dir, golden
/// `<stem>_code.txt`).
fn run_lineconstants_show(stem: &str) {
    let (oracle_main, rust_main, scratch) = produce_deck_show(stem);
    assert_show_bytes_eq(&oracle_main, &rust_main, stem);
    let code_path = scratch.join("LineConstantsCode.dss");
    let rust_code = std::fs::read_to_string(&code_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", code_path.display()));
    let oracle_code = std::fs::read_to_string(reports_dir().join(format!("{stem}_code.txt")))
        .unwrap_or_else(|e| panic!("read {stem}_code golden: {e}"));
    assert_show_bytes_eq(&oracle_code, &rust_code, &format!("{stem}_code"));
    std::fs::remove_dir_all(&scratch).ok();
}

/// `Show busflow 675` (Pascal `ShowBusPowers` code 0): the seq voltages / currents /
/// powers around bus 675 (a fully-energised leaf: Line.692675 + Capacitor.Cap1 + a
/// 3-phase load) on solved IEEE13. Same value paths as `Show Voltages`/`Currents`/
/// `Powers` (bit-exact LineCode-based feeder), filtered to the bus's elements →
/// **exact equality** (`rel = 0`, `abs = 0`): every cell is byte-identical, no
/// near-zero residual (675 has no de-energised branch — chosen for a fully-exact
/// golden; a richer junction like 671 lands a `%10.5g` kvar cell on a 5-sig rounding
/// boundary, a print straddle avoided here).
/// The `Show busflow` **seq** policy: full exact equality (no near-zero residual on a
/// fully-energised bus).
fn busflow_seq_policy() -> ExportPolicy {
    ExportPolicy {
        sep: ' ',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    }
}

/// The `Show busflow` **elem** policy: exact except the capacitor's near-zero kW
/// (field 2) + PF (field 6), gated on the row's kW `< 1e-4` (see
/// [`show_busflow_elem_matches_oracle`]).
fn busflow_elem_policy() -> ExportPolicy {
    let on = |sel: ColSel| ColTol {
        sel,
        rel: 0.0,
        abs: 0.0,
        gate: Some(GateSpec::MinCols(2, 2, 1e-4)),
    };
    ExportPolicy {
        sep: ' ',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![on(ColSel::Index(2)), on(ColSel::Index(6))],
    }
}

#[test]
fn show_busflow_matches_oracle() {
    run_feeder_show("show_busflow", &busflow_seq_policy());
}

/// `Show busflow 675 m` (MVA form, audit-tests step-15 follow-up): the `×0.001`
/// scaling + `MW/Mvar/MVA` headers — the only MVA coverage in the whole `Show` power
/// path. Same seq policy (fully exact); the MVA kW is ~3.6e-18 but the seq form has no
/// near-zero cells here.
#[test]
fn show_busflow_mva_matches_oracle() {
    run_feeder_show("show_busflow_mva", &busflow_seq_policy());
}

/// `Show busflow 675 m e` (MVA element form): the `write_terminal_power` `×0.001`
/// scaling + `MW/Mvar/MVA` headers. Same elem policy (capacitor near-zero kW/PF gated).
#[test]
fn show_busflow_mva_elem_matches_oracle() {
    run_feeder_show("show_busflow_mva_elem", &busflow_elem_policy());
}

/// `Show busflow 611` (1-phase bus, audit-tests follow-up): the `<3`-phase seq path —
/// `WriteSeqVoltages` (<3 nodes → `V2/V0 = 0`), `GetI0I1I2`/`WriteTerminalPowerSeq`
/// (`Nphases < 3` / `S1`). Bus 611 has a 1-phase load + `Capacitor.Cap2`; the
/// `<3`-phase cells are exact zeros → fully exact.
#[test]
fn show_busflow_1ph_matches_oracle() {
    run_feeder_show("show_busflow_1ph", &busflow_seq_policy());
}

/// `Show busflow 611 e` (1-phase element form): the per-terminal branch currents/powers
/// on a 1-phase bus. Same elem policy (Cap2's near-zero kW/PF gated).
#[test]
fn show_busflow_1ph_elem_matches_oracle() {
    run_feeder_show("show_busflow_1ph_elem", &busflow_elem_policy());
}

/// `Show busflow <unknown bus>` (Pascal `#219 'Bus "%s" not found.'`, audit-tests
/// step-15 follow-up): the negative path is golden-uncoverable (it pushes an error and
/// writes no file), so pin it by unit test — a solved circuit + a nonexistent bus
/// yields exactly the uppercase-name #219 message and no report file.
#[test]
fn show_busflow_unknown_bus_errors() {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.t basekv=12.47 bus1=src phases=3");
    dss.command("new load.l bus1=src phases=3 kv=12.47 kw=100");
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "setup: {:?}", dss.errors());
    dss.command("show busflow nosuchbus");
    assert!(
        dss.errors()
            .iter()
            .any(|e| e.code == Some(219) && e.text().contains("not found")),
        "expected #219, got {:?}",
        dss.errors()
    );
}

/// The WP8.4-finalize dispatch tail: an **unknown** `Show` keyword now errors
/// (`#24700`, `ShowOptions.pas:119-124`), while the deferred keywords `autoadded`
/// (arm 1), `QueryLog` (arm 32) and `deltaV` (arm 31) stay **silent** headless no-ops
/// (Pascal's `FireOffEditor` / the deferred `ShowDeltaV`). Pins that porting every
/// real keyword did not turn the deferrals into errors, and that a genuine typo does.
#[test]
fn show_unknown_and_deferred_keywords() {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.t basekv=12.47 bus1=src phases=3");
    dss.command("new load.l bus1=src phases=3 kv=12.47 kw=100");
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "setup: {:?}", dss.errors());

    // Unknown keyword → #24700.
    dss.command("show nosuchreport");
    assert!(
        dss.errors()
            .iter()
            .any(|e| e.contains("Unknown Show Command") && e.contains("nosuchreport")),
        "expected #24700, got {:?}",
        dss.errors()
    );

    // The deferred keywords must NOT error (silent headless no-ops).
    let n = dss.errors().len();
    dss.command("show autoadded");
    dss.command("show querylog");
    assert_eq!(
        dss.errors().len(),
        n,
        "deferred keywords must stay silent no-ops: {:?}",
        &dss.errors()[n..]
    );
}

/// `Show DeltaV` (Pascal `ShowDeltaV` + `WriteElementDeltaVoltages`): the voltage across
/// each enabled 2-terminal element (Sources/PD/PC), per conductor `NodeV[term1] −
/// NodeV[term2]`. On solved IEEE13 the delta-primary `Transformer.SUB` writes 3 rows
/// (the port's node_ref layout now resolves both terminals' buses — the step-4
/// deferral is resolved). Values are the same solved `node_v` the voltage goldens
/// pin, so exact equality.
#[test]
fn show_deltav_matches_oracle() {
    let policy = ExportPolicy {
        sep: ' ',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_show("show_deltav", &policy);
}

/// `Show busflow 675 e` (Pascal `ShowBusPowers` code 1): the element form — node
/// voltages + per-terminal branch currents (PD residual) + branch power flow around
/// bus 675. Reuses the `WriteBusVoltages`/`WriteTerminalCurrents`/`WriteTerminalPower`
/// value paths → **exact equality** except the **capacitor's real-power** cell:
/// `Capacitor.Cap1` consumes ~0 kW, so its power rows carry two faer-vs-KLU
/// artefacts vs the oracle: field 2 (kW) is a ~1e-15 residual (the oracle prints an
/// exact `0` on two phases), and consequently the **power factor** (field 6) diverges
/// — the oracle's exact-0 kW gives `PF = 1.0000` (`P = 0` → `PowerFactor := 1`),
/// while Rust's ~1e-15 kW gives `≈ 0`. Both are the same physical capacitor (its
/// real kvar/kVA are checked exact). Gate the near-zero kW (field 2) **and** the PF
/// (field 6) on the row's kW `< 1e-4` (incl. exact zero via `MinCols`). Field 2 is
/// `|I|` in the current rows (all real ≥-amp at 675) so the gate fires only on the
/// capacitor power rows.
#[test]
fn show_busflow_elem_matches_oracle() {
    run_feeder_show("show_busflow_elem", &busflow_elem_policy());
}

/// `Show Isolated` (Pascal `ShowIsolated`) on the metered IEEE13 — fully connected, so
/// the isolated sections are empty and the report is the connected element tree
/// (`(Level) FullName` + `[SHUNT], FullName`, built from the source via
/// `get_isolated_sub_area`). Pure text (names + tree levels, no floats) → **byte-exact**
/// (`assert_show_bytes_eq`): pins the source-tree walk order, per-level numbering, and
/// the shunt attachment, plus the four empty-section headers.
#[test]
fn show_isolated_matches_oracle() {
    run_feeder_show_exact("show_isolated");
}

/// `Show Topology` (Pascal `ShowTopology`) on the metered IEEE13: verifies **both**
/// files byte-exact — the counts summary `<case>_TopoSumm.txt` (levels/loops/parallel/
/// isolated/switches; IEEE13 = `7 Levels Deep, 1 Loops`, the regulator loop) and the
/// TABCHAR-indented `<case>_TopoTree.txt` with the inline `(LOOP:…)` / `(Control: …)`
/// (via the derived control scan) / `(Meter: …)` annotations. Pins the `get_topology`
/// tree walk + the loop/parallel/sensor/meter/control annotation logic.
#[test]
fn show_topology_matches_oracle() {
    let (oracle_summ, rust_summ, scratch) = produce_feeder_show("show_topology");
    assert_show_bytes_eq(&oracle_summ, &rust_summ, "show_topology");
    let rust_tree = locate_show_report(&scratch, "TopoTree.txt", "show_topology_tree");
    let oracle_tree = std::fs::read_to_string(reports_dir().join("show_topology_tree.txt"))
        .expect("read show_topology_tree golden");
    assert_show_bytes_eq(&oracle_tree, &rust_tree, "show_topology_tree");
    std::fs::remove_dir_all(&scratch).ok();
}

/// `Show Isolated` on the synthesized coverage deck (a PARALLEL line pair, a switched
/// line, and a fully ISOLATED island unreachable from the source) — exercises the
/// non-empty isolated branches the IEEE13 golden misses: the isolated-bus list
/// (`"isoa"`/`"isob"`), the `*** START SUBAREA ***` sub-network walk, and the parallel
/// connected tree. Byte-exact (deck not solved — the island is singular; the report
/// needs only connectivity).
#[test]
fn show_isolated_iso_matches_oracle() {
    run_deck_show_exact("show_isolated_iso");
}

/// `Show Isolated` orphan coverage (audit-tests step-16 follow-up): an orphan Load +
/// Generator on PD-less, source-disconnected buses, so the two `ShowIsolated` sections
/// the IEEE13 + mesh goldens leave empty are non-empty here — the "ENABLED ELEMENTS
/// ARE ISOLATED" list (`"Load.orphan"  Buses:  "orphanbus"`, pinning the `  Buses:`
/// format + the **1-based** `get_bus(j)` walk) and the "BUSES NOT CONNECTED TO ANY
/// POWER DELIVERY ELEMENT" list (`"orphanbus"`/`"genbus"`). Byte-exact.
#[test]
fn show_isolated_orphan_matches_oracle() {
    run_deck_show_exact("show_isolated_orphan");
}

/// `Show Isolated` with a **disabled** PD element (audit-code step-16 follow-up): a
/// `line.dead enabled=no` must NOT emit a `*** START SUBAREA ***` block — the sub-area
/// selection is gated on `Enabled` (a disabled PD element is in `ckt_elements` but not
/// the adjacency lists). Without the guard the port would print a spurious subarea.
/// Byte-exact.
#[test]
fn show_isolated_disabled_matches_oracle() {
    run_deck_show_exact("show_isolated_disabled");
}

/// `Show Isolated` on a compiled-but-**unsolved** circuit (audit-code step-16
/// follow-up): no `calcvoltagebases`/`solve`, so terminal `bus_ref`s are unresolved
/// (`None`) until `ShowIsolated`'s own `ReprocessBusDefs` runs. Pins that the
/// reprocess makes the connected tree correct (Vsource → Line.la → Load.ld) — without
/// it the port would give a degenerate one-source tree. Byte-exact.
#[test]
fn show_isolated_unsolved_matches_oracle() {
    run_deck_show_exact("show_isolated_unsolved");
}

/// `Show Topology` on the same coverage deck: pins the non-zero counts (`2 Parallel PD
/// elements`, `1 Isolated PD components`, `1 Controlled Switches`) + the tree's
/// `(PARALLEL:…)` / `(Control: …)` / `Isolated: …` annotations, none of which the
/// radial IEEE13 golden produces. Both files byte-exact.
#[test]
fn show_topology_mesh_matches_oracle() {
    let (oracle_summ, rust_summ, scratch) = produce_deck_show("show_topology_mesh");
    assert_show_bytes_eq(&oracle_summ, &rust_summ, "show_topology_mesh");
    let rust_tree = locate_show_report(&scratch, "TopoTree.txt", "show_topology_mesh_tree");
    let oracle_tree = std::fs::read_to_string(reports_dir().join("show_topology_mesh_tree.txt"))
        .expect("read show_topology_mesh_tree golden");
    assert_show_bytes_eq(&oracle_tree, &rust_tree, "show_topology_mesh_tree");
    std::fs::remove_dir_all(&scratch).ok();
}

/// `Show LineConstants` (Pascal `ShowLineConstants`) on a synthesized geometry deck
/// with **default** args (freq=60/kft/rho=100): `g3` (3-conductor overhead, order 3)
/// exercises the R/jX/susceptance/L/C matrices AND the equivalent symmetrical-
/// component summary (Z1/Z0, C1/C0, surge impedance, propagation velocity); `g1`
/// (1-conductor, order 1) the non-order-3 branch (no seq block). The Carson recompute
/// reuses the WP7.1 `LineGeometryObj` engine (bit-exact to the oracle bar a proven
/// transcendental libm floor), and every `%.6g` cell lands byte-identical (no 6-sig
/// straddle on this geometry). Both files byte-exact.
#[test]
fn show_lineconstants_matches_oracle() {
    run_lineconstants_show("show_lineconstants");
}

/// `Show LineConstants 60 mi 250` (audit-recommended non-default coverage): pins the
/// `freq`/`units`/`rho` arg-parse arms AND that `rho=250` (earth-return) + `units=mi`
/// propagate into the Carson recompute — the matrix values, the `ohms per mi` labels,
/// and the `To_per_Meter(mi)` velocity all differ from the default case. Guards the
/// `set_rho_earth`-then-recompute path (a fresh geometry's `fline_data` is built at
/// parse, so the rho takes effect). Both files byte-exact.
#[test]
fn show_lineconstants_mi250_matches_oracle() {
    run_lineconstants_show("show_lineconstants_mi250");
}

/// `Show LineConstants 50` (audit-code follow-up): a **non-default frequency**
/// (freq=50 ≠ DefaultBaseFreq=60), units/rho defaulting. Pins that the freq value
/// propagates into the Carson recompute (the freq-parse arm itself is also hit by
/// `mi250`'s non-empty `60`, but its value there equals the default). Both files
/// byte-exact.
#[test]
fn show_lineconstants_f50_matches_oracle() {
    run_lineconstants_show("show_lineconstants_f50");
}

/// `Show LineConstants` on a **geometry-less** deck (circuit + WireData, no
/// `LineGeometry`): both files are header-only (audit-tests step-14 follow-up). Pins
/// the empty-geometry-list path — the `LineConstants.txt` `LINE CONSTANTS`/Frequency/
/// Earth-Model header + trailing blank line, and the `LineConstantsCode.dss` three
/// `!---` header lines — with no per-geometry body. Byte-exact on both files.
#[test]
fn show_lineconstants_empty_matches_oracle() {
    run_lineconstants_show("show_lineconstants_empty");
}

/// The `@lastshowfile` split (Pascal `DoShowCmd`): `ShowY`/`ShowkVBaseMismatch`
/// end in `ParserVars.Add('@lastshowfile', …)`, but the reports dispatched inline
/// with only a `FireOffEditor` — `Show Convergence` (arm 4) and `Show controlqueue`
/// (arm 27) — do **not** touch it. Pins that faithful asymmetry: after a
/// Convergence/ControlQueue the last-show-file must be unchanged, while `Show Y`
/// and `Show kvbasemismatch` update it.
#[test]
fn show_lastshowfile_semantics() {
    let master: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "corpus",
        "electricdss-tst",
        "Version8",
        "Distrib",
        "IEEETestCases",
        "13Bus",
        "IEEE13Nodeckt.dss",
    ]
    .iter()
    .collect();
    assert!(master.is_file(), "master missing: {}", master.display());
    let scratch = scratch_dir("lastshowfile");
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!(
        "compile \"{}\"",
        master.to_string_lossy().replace('\\', "/")
    ));
    dss.command("solve");
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));

    dss.command("show y");
    assert!(
        dss.last_show_file().to_lowercase().ends_with("systemy.txt"),
        "Show Y must set @lastshowfile (got {:?})",
        dss.last_show_file()
    );
    // Convergence must NOT change @lastshowfile (still the Y file).
    dss.command("show convergence");
    assert!(
        dss.last_show_file().to_lowercase().ends_with("systemy.txt"),
        "Show Convergence must not set @lastshowfile (got {:?})",
        dss.last_show_file()
    );
    // ControlQueue must NOT change it either.
    dss.command("show controlqueue");
    assert!(
        dss.last_show_file().to_lowercase().ends_with("systemy.txt"),
        "Show controlqueue must not set @lastshowfile (got {:?})",
        dss.last_show_file()
    );
    // kvbasemismatch DOES set it.
    dss.command("show kvbasemismatch");
    assert!(
        dss.last_show_file()
            .to_lowercase()
            .ends_with("kvbasemismatch.txt"),
        "Show kvbasemismatch must set @lastshowfile (got {:?})",
        dss.last_show_file()
    );
    assert!(dss.errors().is_empty(), "errors: {:?}", dss.errors());
    std::fs::remove_dir_all(&scratch).ok();
}

/// `Export Powers mva` (`opt=1`): the MVA option — the `m…` `Parm2` flag selects
/// MW/Mvar headers + the extra `×0.001` scaling. Backstops the `opt=1` branch
/// (scale + header) wired this WP, which the kVA golden can't reach. Byte-identical
/// like the kVA twin → exact equality; a missing `×0.001` would print kW values
/// ~1000× larger and fail loudly.
#[test]
fn export_powers_mva_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_export("export_powers_mva", &policy);
}

/// `Export P_byphase mva` (`opt=1`): the MVA option for P_byphase — MW/Mvar header
/// plus the single extra `×0.001`. Unlike the kVA twin (whose `%10.3f` straddle is
/// observed), the MW-scaled render is byte-identical → exact equality.
#[test]
fn export_p_byphase_mva_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_export("export_p_byphase_mva", &policy);
}

// --- WP8.2 sub-step 2b: the symmetrical-component family ---------------------
// `Phase2SymComp` + `PctNemaUnbalance` + PD ratings. Magnitude columns are `%g`
// (V1/V2/V0/Vresidual, I1/I2/I0/Iresidual = 6 sig); the ratio/unbalance columns
// (`%V2/V1`, `%V0/V1`, `%NEMA`, `%Normal`, `%Emergency`, `%I2/I1`, `%I0/I1`) are
// `%8.4g` = 4 sig. Every substantial cell is byte-identical Rust↔oracle → exact.
// The sequence quantities V0/V2/I0/I2 of a *balanced* element are near-zero
// cancellation residuals of three ~1e-8-pinned phasors: their absolute error is
// ~phase_scale·1e-8 while their relative error is unbounded — hence a tiny `abs`
// floor on the magnitude columns and a denominator gate on the ratio cells whose
// I1 is itself residual noise. The physics is gated by `corpus_live` (node V /
// Iterminal to 1e-8). (tests/TOLERANCE_NOTES.md)

/// `Export SeqVoltages` (Pascal `ExportSeqVoltages`): per-bus V1/pu/baseKV/V2/
/// %V2V1/V0/%V0V1/Vresidual/%NEMA. Every column byte-identical except the
/// near-zero V0/V2/Vresidual cancellation residuals of a balanced bus, where the
/// 6-sig render straddles (**observed**: the source bus's `V0 = 4.31950e-6` vs
/// `4.31951e-6` V, 1e-11 abs) — `abs = 1e-9` is that measured residual floor with
/// 100× margin, still 5 orders below the smallest real seq voltage. `rel = 0`:
/// every kV-scale magnitude and every `%8.4g` ratio cell is pinned exactly.
#[test]
fn export_seqvoltages_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 1e-9,
        col_tol: vec![],
    };
    run_feeder_export("export_seqvoltages", &policy);
}

/// `Export SeqCurrents` (Pascal `ExportSeqCurrents`): per-terminal I1/%Normal/
/// %Emergency/I2/%I2I1/I0/%I0I1/Iresidual/%NEMA over Sources→PD→PC→Faults. Every
/// substantial cell is byte-identical → `rel = 0`. `abs = 1e-8` absorbs the
/// amp-scale I0/I2/Iresidual cancellation residuals of a balanced terminal
/// (**observed**: a 6-sig render straddle `3.47331e-4` vs `3.47330e-4` A = 1e-9
/// abs, and residual-vs-residual pairs like `5.7e-13` vs `2.7e-12` A) — still
/// below the smallest real printed magnitude, `I1 = 5.8e-4 A`, so real cells stay
/// pinned. The `%I…`/`%NEMA` ratio cells are **gated on I1 (col 2)**: at the one
/// open-terminal row (`Line.671680.2`) I1 is ~1e-12 noise, so the ratios are a
/// faer-vs-KLU noise/noise form (observed `53.55` vs `147.7`) — skipped where
/// `0 < |I1| < 1e-6 A` (the band-limit keeps the 32 exactly-zero rows' `0 == 0`
/// checks). Ungated ratio cells carry `abs = 1e-9` for the residual-numerator/
/// healthy-denominator form (`%I0/I1` of a residual I0 over a loaded I1,
/// **observed** `7.63e-11` vs `1.04e-10`; historically measured ≤ 1.2e-10) —
/// far below any real printed ratio. `%Normal`/`%Emergency` divide by NormAmps
/// (never near-zero) and stay at the default. (tests/TOLERANCE_NOTES.md)
#[test]
fn export_seqcurrents_matches_oracle() {
    let i1_gated = |prefix: &str| ColTol {
        sel: ColSel::Prefix(prefix.to_string()),
        rel: 0.0,
        abs: 1e-9,
        gate: Some(GateSpec::Col(2, 1e-6)),
    };
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 1e-8,
        // The I1 gate covers only the columns that actually divide by I1
        // (`%I2/I1`, `%I0/I1`) or are a same-noise form of the phase currents
        // (`%NEMA`); `%Normal`/`%Emergency` divide by NormAmps (never near-zero)
        // and fall through to the exact default, so a regression there stays
        // checked even on the one gated noise row (audit follow-up).
        col_tol: vec![i1_gated("%i"), i1_gated("%nema")],
    };
    run_feeder_export("export_seqcurrents", &policy);
}

/// `Export SeqPowers` (Pascal `ExportSeqPowers`): per-terminal sequence powers
/// P1/Q1/P2/Q2/P0/Q0 (+ PD excess columns on terminal 1). All value columns are
/// `…:1` (one decimal) — byte-identical like `Powers` → exact equality. PD rows
/// carry 12 fields (the excess columns), PC rows 8; the comparator pins each
/// row's field count.
#[test]
fn export_seqpowers_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_export("export_seqpowers", &policy);
}

// --- WP8.2 sub-step 2c: the per-terminal/per-conductor element exports -------
// `Currents`/`ElemCurrents`/`ElemVoltages` are magnitude (`%10.6g`, 6 sig) +
// angle (`%8.2f`, 2 decimals) reports over Sources→PD→Faults→PC; `ElemPowers`
// prints per-conductor kW/kvar (`%10.6g`); `NodeOrder` is integer node numbers;
// `Taps` is the RegControl tap table. Every real cell is byte-identical → exact;
// the only non-exact cells are the near-zero cancellation residuals (a
// per-terminal residual current / an open-terminal conductor), covered by a tiny
// measured `abs` floor, and their **angles** — pure faer-vs-KLU noise — gated on
// the paired magnitude (`GateSpec::PrevCol`): skipped only where that magnitude
// is a nonzero sub-threshold residual, keeping every exactly-zero row's
// `0.00 == 0.00` check and every real-magnitude row's angle. A proven
// cancellation floor, not a relaxation. (tests/TOLERANCE_NOTES.md)

/// The noise gate for the angle columns of a paired magnitude/angle report: the
/// angle of a real magnitude is byte-identical (exact, `rel = abs = 0`); only
/// the angle of a **near-zero residual** magnitude (a per-terminal residual or
/// an open-terminal conductor — faer-vs-KLU cancellation noise, observed e.g.
/// `-33.69` vs `56.31`°) is skipped, gated on the paired magnitude (the
/// immediately-preceding column, `PrevCol`); `thresh` (1e-6) sits far above that
/// noise (≤ ~1e-8) and below the smallest real printed magnitude. Selected by
/// index parity (`start`, then every other column is an angle) because these
/// reports have **truncated headers** (`…, I_1, Ang_1, ...`) that name only the
/// first pair.
fn ang_tol(start: usize) -> ColTol {
    ColTol {
        sel: ColSel::Parity { start, parity: 1 },
        rel: 0.0,
        abs: 0.0,
        gate: Some(GateSpec::PrevCol(1e-6)),
    }
}

/// `Export Currents` (Pascal `ExportCurrents` + `CalcAndWriteCurrents`):
/// per-terminal, per-conductor `|I|`/angle over the widest element, plus a
/// per-terminal residual. Every real magnitude and angle is byte-identical →
/// `rel = 0`. `abs = 1e-8` absorbs only the near-zero `Iresid` cancellation
/// residuals (**observed**: `8.13e-9` vs `5.09e-9` A, and `3.6e-12` vs an exact
/// oracle `0`) — 5 orders below the smallest real current. The angle of a
/// residual/fill magnitude is noise, gated via [`ang_tol`].
#[test]
fn export_currents_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 1e-8,
        col_tol: vec![ang_tol(1)],
    };
    run_feeder_export("export_currents", &policy);
}

/// `Export NodeOrder` (Pascal `ExportNodeOrder` + `WriteNodeList`): `"Element",
/// Nterms, Nconds, Node-1, …` — all integers (node numbers, 0 = ground), exact.
#[test]
fn export_nodeorder_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_export("export_nodeorder", &policy);
}

/// `Export ElemCurrents` (Pascal `WriteElemCurrents`): per-conductor `|I|`/angle
/// over `NConds·Nterms`. Same structure as `Currents`: everything byte-identical
/// except the ~1e-12 A open-terminal residual magnitudes (**observed** max diff
/// 6.6e-12 A → `abs = 1e-10`, 4 orders below any real current) and their noise
/// angles (gated).
#[test]
fn export_elemcurrents_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 1e-10,
        col_tol: vec![ang_tol(3)],
    };
    run_feeder_export("export_elemcurrents", &policy);
}

/// `Export ElemVoltages` (Pascal `WriteElemVoltages`): per-conductor `|V|`/angle.
/// Byte-identical throughout (the grounded-neutral conductors are an exact `0` on
/// both engines, their angles an exact `0.00` — no residual class in this report)
/// → exact equality, no gates.
#[test]
fn export_elemvoltages_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_export("export_elemvoltages", &policy);
}

/// `Export ElemPowers` (Pascal `WriteElemPowers`): per-conductor `S =
/// Vterminal·conj(Iterminal)`, printed kW/kvar (`%10.6g`, 6 sig). Formed straight
/// from `ComputeVterminal`/`ComputeIterminal` (like `P_byphase`, no `×3`); on the
/// solved feeder equals the canonical terminal power (the `Vsource` row matches
/// `CktElement.Powers`, oracle-probed — the isolated-source 2a divergence never
/// appears here). Byte-identical except the ~1e-11 kW neutral-conductor
/// cancellation residuals (**observed**: `-2.2e-12` vs `-1.78e-11`, and
/// `1.46e-14` vs an exact `0`) → `rel = 0`, `abs = 1e-10`.
#[test]
fn export_elempowers_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 1e-10,
        col_tol: vec![],
    };
    run_feeder_export("export_elempowers", &policy);
}

/// `Export Taps` (Pascal `ExportTaps`): one row per RegControl — the controlled
/// transformer's present/min/max tap + increment (`%8.5f`), integer tap position
/// and winding, and the Forward/Reverse + True/False mode text. The tap value is
/// a discrete `mid + position·increment`, so both engines land the identical
/// value once they converge to the same integer tap position (already pinned by
/// `corpus_live` and the feeder-controls gate) — byte-identical, exact equality.
#[test]
fn export_taps_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_export("export_taps", &policy);
}

// --- WP8.2 sub-step 3: the matrix/summary exports ----------------------------
// `Y`/`Yprims` serialize the assembled/per-element admittance matrices (already
// pinned entry-by-entry by the checkpoint + live full-Y gates); `SeqZ` reads the
// per-bus short-circuit impedances; `Summary` is a one-row status line; `Result`
// dumps the `@result` parser var. These goldens pin the report *layout*.

/// `Export Result` (Pascal `ExportResult`): the `@result` parser var, one line.
/// In the pinned PM-build oracle `@result` is always `null` (the update is
/// compiled out), which our engine reproduces (`ParserVars::new` seeds `null`,
/// never rewritten). Pure text, no header.
#[test]
fn export_result_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_export("export_result", &policy);
}

/// `Export Summary` (Pascal `ExportSummary`): one status row (solve mode, counts,
/// iterations, pu-voltage extremes, total MW/Mvar, losses). Column 0 is the
/// wall-clock `DateTimeToStr(Now)` — non-deterministic, **masked**
/// (`GateSpec::Mask`). Every other column is deterministic and byte-identical
/// (text case-insensitively; the counts and `%g` scalars exactly) → exact
/// equality outside the mask.
#[test]
fn export_summary_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![ColTol {
            sel: ColSel::Index(0), // DateTime — masked (non-deterministic clock)
            rel: 0.0,
            abs: 0.0,
            gate: Some(GateSpec::Mask),
        }],
    };
    run_feeder_export("export_summary", &policy);
}

/// `Export SeqZ` (Pascal `ExportSeqZ`): per-bus symmetrical-component short-circuit
/// impedances after a **FaultStudy** solve (a plain snapshot leaves `Zsc` zero).
/// `R1/X1/R0/X0/Z1/Z0` (`%10.6g`) and the `X1/R1`/`X0/R0` ratios (`%8.4g`) are
/// byte-identical — the same `Zsc1`/`Zsc0` the `fault_study.rs` gate pins to
/// 1e-9·mag — exact equality.
#[test]
fn export_seqz_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_export("export_seqz", &policy);
}

/// `Export Faultstudy` (Pascal `ExportFaultStudy`) on the FaultStudy-solved
/// IEEE13 feeder: per-bus 3-phase / 1-phase / L-L prospective fault currents.
/// The 3-phase column reads the precomputed `BusCurrent`; the 1-phase/L-L columns
/// are local per-bus `YFault` scratch inversions over the same precomputed `Ysc`
/// (PHASE8_PLAN §2.1). All three are `%10f` (2 decimals); the faultstudy
/// `Zsc`/`Ysc` are pinned to 1e-9·mag by `exec/tests/fault_study.rs` and the
/// `YFault` inversions run the same bit-faithful `CMatrix::invert` on both
/// engines, so the rendered currents are byte-identical (no 0.005-boundary
/// straddle on this golden) — exact equality.
#[test]
fn export_faultstudy_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_export("export_faultstudy", &policy);
}

// The assembled/primitive admittance matrices are exact deterministic stamps
// (no faer solve enters them) from FPC-faithful element YPrims — byte-identical
// at 10 sig on both engines → exact equality.

/// `Export Y triplet` (Pascal `ExportY` `TripletOpt`): the assembled system Y as
/// `Row,Col,G,B` for the lower triangle (`row >= col`), column-major. Integer
/// Row/Col and the `%.10g` G/B all exact.
#[test]
fn export_y_triplet_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_export("export_y_triplet", &policy);
}

/// `Export Yprims` (Pascal `ExportYprim`): every enabled PD/PC element's
/// primitive Y, in device order — a `Class.NAME` header line then `Yorder` rows
/// of `re, im,` pairs (`%.10g`). No fixed header (the first line is an element
/// name), so `header_lines = 0`; the name lines are single text fields (matched
/// case-insensitively), the matrix cells exact. The element set/order is the
/// report's contract (`ExactOrdered`).
#[test]
fn export_yprims_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_export("export_yprims", &policy);
}

// --- WP8.2 follow-up: the remaining solution-family exports ------------------
// `VoltagesElements` (per-element terminal voltages — the by-element companion to
// `Voltages`) and the raw Y-ordered node vectors `YVoltages` (NodeV) / `YCurrents`
// (Solution.Currents). Read-only; the engine physics (node V / node currents) is
// pinned to 1e-8 by `corpus_live`, so these pin report layout.

/// `Export VoltagesElements` (Pascal `ExportVoltagesElements`): per-element,
/// per-terminal, per-conductor `Node`(index)/`Magnitude`(kV,`%10.6g`)/`Angle`
/// (`%6.3f`)/`pu`(`%9.5g`) + the per-terminal `Bus`/`BasekV`(`%6.3f`). Every
/// column byte-identical (every conductor is either energized or exactly-ground
/// `0` — no cancellation residual) → exact equality. Rows are ragged (an element
/// writes only its own `NTerms` terminal blocks, not padded to
/// `MaxNumTerminals`); `ExactOrdered` pins each row's fields.
#[test]
fn export_voltageselements_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_export("export_voltageselements", &policy);
}

/// `Export YVoltages` (Pascal `ExportYVoltages`): the node voltage vector `NodeV`
/// for nodes `1..NumNodes`, one `re, im` pair per line, no header. `%10.6g` (6
/// sig), byte-identical → exact equality.
#[test]
fn export_yvoltages_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_export("export_yvoltages", &policy);
}

/// `Export YCurrents` (Pascal `ExportYCurrents`): the node injection-current
/// vector `Solution.Currents` for nodes `1..NumNodes`, one `re, im` pair per line,
/// no header. Byte-identical except one passive-node cell where the oracle prints
/// an exact `0` and Rust a `1.42e-14` A cancellation residual (**observed**) —
/// `abs = 1e-13` covers that f64 KCL-residual print, 17 orders below the ~1e4 A
/// source injections, which stay pinned exactly (`rel = 0`).
#[test]
fn export_ycurrents_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 1e-13,
        col_tol: vec![],
    };
    run_feeder_export("export_ycurrents", &policy);
}

// --- WP8.2 completion gate: the bus/summary exports at scale (IEEE 8500) ------
// PHASE8_PLAN §WP8.2 step 4: `Voltages`/`Summary`/`Counts` on the solved IEEE
// 8500-Node feeder (8531 nodes, 6103 devices), completing the export-diff over
// 13/34/37/123/8500. The per-element/matrix dumps are omitted as enormous (the
// established 8500-golden discipline — `golden_ieee8500.rs`). All three share the
// one heavy compile+solve via `run_shared_exports`. The reports pin layout at
// scale (header/column set/order/scaling + the full 4876-bus row set); the engine
// physics is pinned by the always-on `corpus_live.rs` 8500 model compare.

/// `Voltages`/`Summary`/`Counts` on the solved IEEE 8500-Node feeder, from a
/// single compile. Voltages: byte-identical across all 8531 node rows (incl. the
/// `%6.1f` angles) → exact equality. Summary: the masked non-deterministic
/// `DateTime` column + the deterministic status row, exact outside the mask.
/// Counts: the `RustSubsetByKey` subset compare (`=`-separated), pinning every
/// ported class's instance count at scale (Line=3703/Transformer=1190/etc).
#[test]
fn export8500_reports_match_oracle() {
    let voltages = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    let summary = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![ColTol {
            sel: ColSel::Index(0), // DateTime — masked (non-deterministic clock)
            rel: 0.0,
            abs: 0.0,
            gate: Some(GateSpec::Mask),
        }],
    };
    let counts = ExportPolicy {
        sep: '=',
        header_lines: 1, // "Format: DSS Class Name = Instance Count"
        rows: RowPolicy::RustSubsetByKey {
            key: 0,
            // The core Phase 4-6 classes the 8500 feeder instantiates in bulk —
            // guards against a registry-walk regression dropping a class or
            // emptying the body (the subset compare alone can't see a missing row).
            require: [
                "line",
                "load",
                "transformer",
                "capacitor",
                "regcontrol",
                "capcontrol",
                "vsource",
                "reactor",
                "linecode",
                "xfmrcode",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
            // WindGen (WP-U1.8): a 0.15.x class absent from the pinned 0.14.5
            // oracle; skip its (zero-instance) count row here.
            allow_extra: vec!["windgen".to_string()],
        },
        rel: 0.0,
        abs: 0.0, // integer counts — exact
        col_tol: vec![],
    };
    run_shared_exports(&[
        ("export8500_voltages", voltages),
        ("export8500_summary", summary),
        ("export8500_counts", counts),
    ]);
}

// --- PORTING_PLAN §6: export-diff on IEEE 34/37/123 --------------------------
// §6's literal acceptance requires the export-diff suite green on IEEE
// 13/34/37/123/8500. IEEE13 is pinned by the full export family above and 8500 by
// `export8500_reports_match_oracle`; these three complete the letter with the
// core solution exports — Voltages / Currents / Powers — mirroring the IEEE13
// policies exactly (`export_voltages`/`export_currents`/`export_powers`). Each
// feeder shares one compile+solve via `run_shared_exports`. The V/I/P physics is
// already pinned to 1e-8 by the always-on `corpus_live.rs` model compare on these
// same feeders; here we gate the report layout (header, column set, element
// order, scaling) at the printing floor on each canonical feeder.

/// The near-cancellation current floor for the `Currents` export, mirroring the
/// always-on `corpus_live.rs` **"feeder"-tier** terminal-current tolerance
/// (`tol_for("feeder")`: `i_rel = 1e-7`, `i_abs = 1e-5` A). A lightly-loaded /
/// unloaded phase carries only a tiny mutual-coupling current (e.g. IEEE123
/// `Line.L49` phase 2 = 3.45 mA while phases 1/3 carry 9–18 A; `Line.SW6`
/// phase 3 ≈ 14 µA): that current is the near-cancellation of large mutual terms,
/// so the faer-vs-KLU ~1e-8-rel node-voltage roundoff surfaces as a much larger
/// *relative* current error (observed ≤3e-2 rel / ≤4.4e-7 A abs across 34/37/123)
/// — exactly the floor `corpus_live` absorbs with `i_abs` (see the
/// `assert_power_close` / `tol_for` "feeder" doc). The physics itself is pinned to
/// 1e-8 by that live gate; this export gate reuses its proven current floor rather
/// than a tighter one that would false-fail on the print of these residuals. The
/// loaded phases (amps) stay byte-identical (`rel = 1e-7` is slack there).
const CURRENTS_REL: f64 = 1e-7;
const CURRENTS_ABS: f64 = 1e-5;

/// The current magnitude below which a `Currents` **angle** cell is unverifiable
/// at the report's `%.2f` (0.01°) resolution — decomposition-derived, not swept.
/// The current phasor is pinned only to the `corpus_live` feeder floor
/// [`CURRENTS_ABS`] (1e-5 A abs), so a magnitude-`|I|` current's angle is
/// determined only to `±arcsin(CURRENTS_ABS / |I|)`. That uncertainty exceeds a
/// half-ULP of the print (0.005°) once `|I| ≤ CURRENTS_ABS / sin(0.005°) ≈
/// 0.115 A`, so below that the last printed angle digit can legitimately differ
/// between faer and KLU. (Empirically the straddles are far smaller — realized
/// `δI ≈ 1e-6` A, largest observed straddle at |I| = 1.84 mA — so `0.12 A` clears
/// the worst observed by ~65× while staying the conservative, proven bound.)
/// Above it, the angle is pinned **exactly**.
const CURRENTS_ANGLE_GATE_A: f64 = 0.12;

/// Per-angle-column noise gate for the `Currents` export (cols 2,4,6,… — the
/// `Ang*_*` columns of the truncated-header magnitude/angle layout `Element,
/// I1_1, Ang1_1, …`). Each angle column `c` is gated on its own magnitude column
/// `c-1` via `MinCols(c-1, c-1, CURRENTS_ANGLE_GATE_A)`: the angle is skipped
/// whenever the (oracle) magnitude is below the pinnable-angle threshold — a
/// residual / open-terminal / lightly-loaded-phase current whose printed angle is
/// not determined to `%.2f` by the proven current floor (see
/// [`CURRENTS_ANGLE_GATE_A`]). Unlike the IEEE13 `ang_tol` (`PrevCol`, which skips
/// only a *strictly nonzero* sub-threshold magnitude), `MinCols`'s `< thresh`
/// also covers the **exactly-zero** oracle magnitude — a terminal where KLU
/// produces a structural 0 A while faer leaves a ~1e-12 A residual with a defined
/// (noise) angle (e.g. IEEE37 `Transformer.XFM1` I2_3). Every loaded (≥0.12 A)
/// conductor keeps its angle pinned exactly (`rel = 0`, `abs = 0`).
fn currents_angle_gates() -> Vec<ColTol> {
    // 2 terminals × (up to 4 conductors + 1 residual) = 20 value columns; the
    // angle columns are the even indices 2..=20. Extra even indices past a
    // narrower element's width simply never match a shorter row / no-op the gate.
    (1..=20)
        .filter(|c| c % 2 == 0)
        .map(|c| ColTol {
            sel: ColSel::Index(c),
            rel: 0.0,
            abs: 0.0,
            gate: Some(GateSpec::MinCols(c - 1, c - 1, CURRENTS_ANGLE_GATE_A)),
        })
        .collect()
}

/// The three core solution-export policies, mirroring the IEEE13 gates:
/// Voltages/Powers are byte-identical (exact equality); Currents pins the
/// magnitude columns to the `corpus_live` feeder current floor
/// ([`CURRENTS_REL`]/[`CURRENTS_ABS`]) and gates each near-zero angle
/// ([`currents_angle_gates`]).
fn core_export_policies() -> (ExportPolicy, ExportPolicy, ExportPolicy) {
    let voltages = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    let currents = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: CURRENTS_REL,
        abs: CURRENTS_ABS,
        col_tol: currents_angle_gates(),
    };
    let powers = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    (voltages, currents, powers)
}

/// Voltages/Currents/Powers on the solved IEEE 34-bus feeder (`ieee34Mod1.dss`).
#[test]
fn export_ieee34_reports_match_oracle() {
    let (v, i, p) = core_export_policies();
    run_shared_exports(&[
        ("export_ieee34_voltages", v),
        ("export_ieee34_currents", i),
        ("export_ieee34_powers", p),
    ]);
}

/// Voltages/Currents/Powers on the solved IEEE 37-bus feeder (`ieee37.dss`).
#[test]
fn export_ieee37_reports_match_oracle() {
    let (v, i, p) = core_export_policies();
    run_shared_exports(&[
        ("export_ieee37_voltages", v),
        ("export_ieee37_currents", i),
        ("export_ieee37_powers", p),
    ]);
}

/// Voltages/Currents/Powers on the solved IEEE 123-bus feeder (`IEEE123Master.dss`).
#[test]
fn export_ieee123_reports_match_oracle() {
    let (v, i, p) = core_export_policies();
    run_shared_exports(&[
        ("export_ieee123_voltages", v),
        ("export_ieee123_currents", i),
        ("export_ieee123_powers", p),
    ]);
}

// --- WP8.3 step 1: Export Monitors (Monitor.TranslateToCSV) -------------------
// `Export Monitors <name>` writes each monitor's in-memory f32 sample buffer to
// its own CSV (`<case>_Mon_<name>_1.csv`; the `_1` is the PM-build primary-context
// `DSS._Name`). Three monitors cover the distinct CSV shapes on one daily-solved
// IEEE13 compile: mode 0 (general V/I — paired magnitude/angle, unquoted header),
// mode 1 (power — the `S (kVA)`/`Ang` header whose spaces force `CommaText`
// quoting), and mode 2 (the single quoted `Tap (pu)` channel). The header line is
// compared **verbatim** (the `Header.CommaText` contract — quoting included); the
// data rows (incl. the `hour`/`t(sec)` time columns the live gate's channel
// compare skips) parse numbers out. Values are f32 (the monitor stream is
// single-precision on both engines), printed `%-.6g` (6 sig): at f32 quantization
// the printed values are byte-identical Rust↔oracle → exact equality. The monitor
// *sampling code path* is 1e-8-gated by the always-on `corpus_live.rs` on
// equivalent `line.650632` daily monitors; this golden pins these exact monitors'
// values and the CSV **layout** (header CommaText, column order, row stride, the
// `_1` filename). All three share the one compile+daily-solve via
// `run_shared_exports`.
#[test]
fn export_monitors_match_oracle() {
    let mon = || ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_shared_exports(&[
        ("export_mon_vi", mon()),
        ("export_mon_pow", mon()),
        ("export_mon_tap", mon()),
    ]);
}

// --- WP8.3 step 2: the register/load dumps -----------------------------------
// `Export Meters`/`Generators`/`PVSystem_Meters`/`Storage_Meters` dump each enabled
// element's registers (`Year, LDCurve, Hour, <Name>` + `%10.0f` per register) under
// a `"quoted"` register-name header; `Export Loads` dumps the present allocation
// view. No corpus deck uses these keywords, so the fixtures are synthesized
// (PHASE8_PLAN §1). The header line is compared **verbatim** (the register-name
// set/quoting is the report's contract); the register values are `%10.0f`
// integers, byte-identical on these fixtures (no last-integer rounding straddle)
// → exact equality. Two fixtures keep every value tightly oracle-pinned (no
// masking):
//   A) plain IEEE13 + an EnergyMeter, daily → Meters + Loads. The meter registers
//      match to ~1e-8 (the same daily meter path `corpus_live.rs` pins), so `%10.0f`
//      is identical.
//   B) IEEE13 + Generator + PVSystem + Storage (off the metered zone), daily → the
//      DER register dumps. Their own round kWh/kW registers match cleanly; keeping
//      the DER out of the metered, regulated zone avoids the ~3e-5 metered-element
//      coupling that would straddle the meter's Max kW rounding boundary.

/// Register rows: `%10.0f` integers, byte-identical (no last-digit rounding
/// straddle on these fixtures) — exact equality.
fn register_policy() -> ExportPolicy {
    ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    }
}

/// `Export Meters` (Pascal `ExportMeters`/`WriteSingleMeterFile`) + `Export Loads`
/// (Pascal `ExportLoads`) on the daily-solved plain IEEE13 + EnergyMeter fixture.
#[test]
fn export_meters_loads_match_oracle() {
    // Loads columns: Load(0), ConnectedKVA(1,`%8.1f`), AllocFactor(2,`%5.3f`),
    // Phases(3), kW(4,`%8.1f`), kvar(5,`%8.1f`), PF(6,`%5.3f`), Model(7) — all
    // byte-identical (static input echoes + 1-decimal renders of ~1e-8-agreeing
    // solves) → exact equality.
    let loads = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_shared_exports(&[
        ("export_meters", register_policy()),
        ("export_loads", loads),
    ]);
}

/// `Export Generators`/`PVSystem_Meters`/`Storage_Meters` (Pascal `ExportGenMeters`/
/// `ExportPVSystemMeters`/`ExportStorageMeters`) on the daily-solved IEEE13 + DER
/// fixture — each DER's kWh/kvarh/Max kW/Max kVA/Hours/Price registers.
#[test]
fn export_der_registers_match_oracle() {
    run_shared_exports(&[
        ("export_generators", register_policy()),
        ("export_pvsystem_meters", register_policy()),
        ("export_storage_meters", register_policy()),
    ]);
}

/// The `/m` multi-file switch (Pascal `WriteMultipleMeterFiles`): `Export Meters
/// /m` writes one `EXP_MTR_<NAME>.csv` per enabled meter (no `CaseName_` prefix)
/// and leaves `@lastexportfile` = the literal `/m` (Pascal's `DoExportCmd` tail
/// quirk). Self-consistency gate (no separate oracle capture): the per-meter file's
/// data row must carry the **same** register values as the single-file
/// `export_meters` golden already pins against the oracle — so a `/m` path bug
/// (wrong file name, dropped header, wrong element) fails loudly, while the values
/// stay oracle-anchored transitively.
#[test]
fn export_meters_multifile_switch() {
    let dir = reports_dir();
    let meta: FeederMeta = {
        let p = dir.join("export_meters.meta.json");
        let text =
            std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", p.display()))
    };
    let single = {
        let p = dir.join("export_meters.txt");
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
    };

    let master: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "corpus",
        "electricdss-tst",
    ]
    .iter()
    .collect::<PathBuf>()
    .join(&meta.master);
    assert!(master.is_file(), "master missing: {}", master.display());

    let scratch = scratch_dir("meters_multifile");
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!(
        "compile \"{}\"",
        master.to_string_lossy().replace('\\', "/")
    ));
    for c in &meta.post {
        dss.command(c);
    }
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    dss.command("export meters /m");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    // The `/m` tail sets @lastexportfile / GlobalResult to the literal switch.
    assert_eq!(dss.last_result_file(), "/m");

    // The one enabled meter (EM1) got its own file, header + data row, values
    // identical to the single-file golden's EM1 row (transitively oracle-pinned).
    let em1 = scratch.join("EXP_MTR_EM1.csv");
    let multi =
        std::fs::read_to_string(&em1).unwrap_or_else(|e| panic!("read {}: {e}", em1.display()));
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    compare_export(&single, &multi, &policy, "export_meters_multifile");

    std::fs::remove_dir_all(&scratch).ok();
}

/// Compile the master + replay the post commands from `<stem>.meta.json`, route
/// reports into a fresh scratch dir, and hand the driven `Dss` + scratch path to
/// `body`. Shared by the append / Storage-`/m` self-consistency tests below.
fn with_register_fixture(stem: &str, tag: &str, body: impl FnOnce(&mut Dss, &std::path::Path)) {
    let dir = reports_dir();
    let meta: FeederMeta = {
        let p = dir.join(format!("{stem}.meta.json"));
        let text =
            std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", p.display()))
    };
    let master: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "corpus",
        "electricdss-tst",
    ]
    .iter()
    .collect::<PathBuf>()
    .join(&meta.master);
    assert!(master.is_file(), "master missing: {}", master.display());

    let scratch = scratch_dir(tag);
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!(
        "compile \"{}\"",
        master.to_string_lossy().replace('\\', "/")
    ));
    for c in &meta.post {
        dss.command(c);
    }
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    body(&mut dss, &scratch);
    std::fs::remove_dir_all(&scratch).ok();
}

/// The single-file **append** path (Pascal `WriteSingleMeterFile`: append to a
/// file whose first line begins `Year`, no re-emitted header). Two `Export
/// Meters` into the same datapath must leave one header + two data rows — pinning
/// `register_need_rewrite`'s "append, don't rewrite" branch (the fresh-dir goldens
/// only ever exercise the create branch).
#[test]
fn export_meters_append_accumulates() {
    with_register_fixture("export_meters", "meters_append", |dss, _scratch| {
        dss.command("export meters");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        let path = dss.last_result_file().to_string();
        dss.command("export meters"); // second export → append, not rewrite
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        assert_eq!(
            dss.last_result_file(),
            path.as_str(),
            "append reused the same file"
        );

        let content = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"));
        let lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();
        assert_eq!(
            lines.len(),
            3,
            "append: expected 1 header + 2 rows, got {lines:?}"
        );
        assert!(lines[0].starts_with("Year"), "header line: {:?}", lines[0]);
        // Both rows are the same EM1 sample (a re-emitted header on append, or a
        // truncation, would break this).
        assert!(lines[1].contains("\"EM1\""), "row 1: {:?}", lines[1]);
        assert_eq!(lines[1], lines[2], "the two appended rows differ");
    });
}

/// The Storage `/m` path reproduces an **upstream copy-paste bug** — its per-file
/// prefix is `EXP_PV_`, not `EXP_STORAGE_` (`ExportResults.pas:2240`,
/// `TODO(compat)`). Pin that exact filename (and the transitively-oracle-anchored
/// row) so the deliberately-faithful quirk cannot silently drift to `EXP_STORAGE_`.
#[test]
fn export_storage_multifile_uses_pv_prefix() {
    let single = {
        let p = reports_dir().join("export_storage_meters.txt");
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
    };
    with_register_fixture(
        "export_storage_meters",
        "storage_multifile",
        |dss, scratch| {
            dss.command("export storage_meters /m");
            assert!(dss.errors().is_empty(), "{:?}", dss.errors());
            assert_eq!(dss.last_result_file(), "/m");

            // The copy-paste-bug prefix: EXP_PV_<NAME>.csv, NOT EXP_STORAGE_.
            let pv = scratch.join("EXP_PV_ST1.csv");
            assert!(
                pv.is_file(),
                "storage /m must write EXP_PV_ST1.csv (the EXP_PV_ prefix bug)"
            );
            assert!(
                !scratch.join("EXP_STORAGE_ST1.csv").exists(),
                "storage /m must NOT use an EXP_STORAGE_ prefix"
            );
            let multi = std::fs::read_to_string(&pv)
                .unwrap_or_else(|e| panic!("read {}: {e}", pv.display()));
            compare_export(
                &single,
                &multi,
                &register_policy(),
                "export_storage_multifile",
            );
        },
    );
}

// --- WP8.3 step 3a: the event/error-log dumps ------------------------------
// `Export EventLog` / `Export ErrorLog` (Pascal `ExportEventLog`/`ExportErrorLog`)
// are `TStringList.SaveToFile` dumps of `DSS.EventStrings` / `DSS.ErrorStrings`.
// The event-log lines are `Hour=…, Sec=…, Iteration=…, …` records: the stamps and
// the in-action tap values are *numbers*, so — like the timeseries_controls event-log gate —
// they are compared line-for-line with numbers parsed out (`numeric_skeleton`),
// never as raw float-strings.

/// Compile the fixture, replay its post commands, export the log report into a
/// scratch dir, and compare the produced file to the oracle's line-for-line with
/// numbers parsed out at 1e-6 rel (the timeseries_controls event-log policy). A count mismatch
/// (a missing/extra LogThisEvent marker) fails loudly before the per-line compare.
fn run_log_export(stem: &str) {
    let dir = reports_dir();
    let meta: FeederMeta = {
        let p = dir.join(format!("{stem}.meta.json"));
        let text =
            std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", p.display()))
    };
    let oracle = {
        let p = dir.join(format!("{stem}.txt"));
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
    };

    let master: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "corpus",
        "electricdss-tst",
    ]
    .iter()
    .collect::<PathBuf>()
    .join(&meta.master);
    assert!(master.is_file(), "master missing: {}", master.display());

    let scratch = scratch_dir(stem);
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!(
        "compile \"{}\"",
        master.to_string_lossy().replace('\\', "/")
    ));
    for c in &meta.post {
        dss.command(c);
    }
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    dss.command(&format!("export {}", meta.report));
    assert!(dss.errors().is_empty(), "{stem}: {:?}", dss.errors());

    let produced = dss.last_result_file();
    let want = format!("{}_{}", meta.fixture, meta.suffix).to_lowercase();
    assert!(
        produced.to_lowercase().ends_with(&want),
        "{stem}: unexpected produced path {produced:?} (want …{want})"
    );
    let rust = std::fs::read_to_string(produced)
        .unwrap_or_else(|e| panic!("read produced {produced}: {e}"));

    let ol: Vec<&str> = oracle.lines().filter(|l| !l.trim().is_empty()).collect();
    let rl: Vec<&str> = rust.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(
        rl.len(),
        ol.len(),
        "{stem}: log line count differs (rust {} vs oracle {})\n  rust:\n    {}\n  oracle:\n    {}",
        rl.len(),
        ol.len(),
        rl.join("\n    "),
        ol.join("\n    ")
    );
    for (i, (a, e)) in rl.iter().zip(&ol).enumerate() {
        assert_value_matches_tol(a, e, 1e-6, 1e-9, &format!("{stem} line {i}"));
    }

    std::fs::remove_dir_all(&scratch).ok();
}

/// `Export EventLog` (Pascal `ExportEventLog`): the full `Set Log=yes`
/// LogThisEvent marker stream (bus-def reprocess → meter-zone reset → Yprim
/// recalc → Y build → per-iteration/control markers → Solution Done) **plus**
/// the regulators' `AppendToEventLog` tap-change lines, over a daily-3 IEEE13
/// solve driven by a swinging load so all three regulators actually move taps
/// (18 `CHANGED n TAPS TO <pu>` lines — the non-integer tap values exercise the
/// numeric-tolerance compare, not just the integer stamps). Pins the export
/// **and** that our engine emits every LogThisEvent call site + every tap-change
/// action line-for-line vs the oracle (the Circuit-build markers landed in WP8.3
/// step 3a; the tap logic is the same the timeseries_controls `daily_ieee13` gate pins).
#[test]
fn export_eventlog_matches_oracle() {
    run_log_export("export_eventlog");
}

/// `Export ErrorLog` (Pascal `ExportErrorLog`): a clean IEEE13 solve logs no
/// `DoSimpleMsg`, so the dump is empty — pins the plumbing + `EXP_ErrorLog.txt`
/// naming. The non-empty content path is gated by `export_errorlog_captures_errors`.
#[test]
fn export_errorlog_matches_oracle() {
    run_log_export("export_errorlog");
}

/// The `Export ErrorLog` **content** path (no oracle golden — cross-engine error
/// *message text* is not a Phase-8 axis): trigger a recoverable `DoSimpleMsg`
/// (an unknown property on an existing element), export the log, and assert the
/// message reaches the file. Guards against the dump silently dropping the
/// accumulated `Dss::errors` (the empty-golden alone can't catch that).
#[test]
fn export_errorlog_captures_errors() {
    let scratch = scratch_dir("errorlog_content");
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.elog basekv=12.47 bus1=src");
    dss.command("new line.l1 bus1=src bus2=b");
    // A recoverable error: an unknown property on an existing element
    // (`DoSimpleMsg` records + continues, growing `ErrorStrings`).
    dss.command("edit line.l1 bogusproperty=42");
    assert!(
        !dss.errors().is_empty(),
        "the bogus edit should have logged an error"
    );
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    dss.command("export errorlog");

    let produced = dss.last_result_file();
    assert!(
        produced.to_lowercase().ends_with("elog_exp_errorlog.txt"),
        "unexpected produced path: {produced:?}"
    );
    let content = std::fs::read_to_string(produced)
        .unwrap_or_else(|e| panic!("read produced {produced}: {e}"));
    // The recorded `DoSimpleMsg` names the offending property — assert that exact
    // token reaches the file (a dropped/empty dump would fail this).
    assert!(
        content.to_lowercase().contains("bogusproperty"),
        "the exported ErrorLog must contain the recorded error; got:\n{content}"
    );
    std::fs::remove_dir_all(&scratch).ok();
}

// --- WP8.3 step 3c: the reliability + capacity exports -----------------------
// `Export BusReliability`/`BranchReliability` (Pascal `ExportBusReliability`/
// `ExportBranchReliability`) dump the per-bus / per-branch reliability indices a
// prior `RelCalc` populated; `Export Capacity` (`ExportCapacity` +
// `CalcAndWriteMaxCurrents`) dumps each PDElement's max phase current vs its
// rating + branch customer counts. No corpus deck exports these, so the fixture is
// synthesized (PHASE8_PLAN §1) as a self-contained deck: a two-section radial
// feeder + per-line fault data + a recloser (the OCP device `RelCalc` needs) +
// loads with `numcust` + an EnergyMeter, `solve`d then `relcalc`'d. High recloser
// pickups keep the snapshot from tripping so `Capacity` sees real currents. This
// is the `relcalc_head_recloser_matches_oracle` feeder — the reliability unit test
// proves both engines' `RelCalc` arithmetic agrees.

/// `Export BusReliability` (Pascal `ExportBusReliability`): per-bus Lambda/
/// Num-Interruptions/Num-Customers/Cust-Interruptions/Duration/Total-Miles. All
/// value columns are `%-.11g` (11 sig) computed by the `RelCalc` sweep — pure
/// arithmetic from identical line fault-data on both engines, byte-identical →
/// exact equality.
#[test]
fn export_busreliability_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_deck_export("export_busreliability", &policy);
}

/// `Export BusReliability` on a **two-meter** feeder — pins the port's faithful
/// reproduction of the upstream multi-meter `Bus_Int_Duration` cross-zone
/// contamination (the second meter's duration loop walks the first meter's zone
/// buses and overwrites their `Duration` from *its own* `FeederSections`; see
/// `investigations/reliability_bus_int_duration_oob_bug_report.md`). Both meters
/// have two sections, so every cross-zone read is **in range** — a deterministic
/// overwrite both engines agree on bus-for-bus (no out-of-bounds read; the OOB
/// regime is proven-nondeterministic UB and deliberately not gated). Same
/// byte-identical arithmetic as the single-meter case — exact equality.
#[test]
fn export_busreliability_multimeter_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_deck_export("export_busreliability_multimeter", &policy);
}

/// `Export BranchReliability` (Pascal `ExportBranchReliability`): per-branch
/// Lambda/Accumulated-Lambda/customers/interrupts/durations/miles/Cust-Miles/SAIFI
/// (`%-.11g` + integer customer counts). Byte-identical like BusReliability —
/// exact equality.
#[test]
fn export_branchreliability_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_deck_export("export_branchreliability", &policy);
}

/// `Export Capacity` (Pascal `ExportCapacity` + `CalcAndWriteMaxCurrents`):
/// per-PDElement `Imax`/`%normal`/`%emergency`/`kW`/`kvar`/customers/`NumPhases`/
/// `kVBase`. Every column (`%10.6g` solve-derived, `%8.2f` percentages, `%-.3g`
/// kVBase, integers) is byte-identical on this tiny circuit — exact equality; a
/// real scale/mapping regression (wrong ×0.001, a dropped phase in the `Imax`
/// max, a swapped kW/kvar) fails loudly.
#[test]
fn export_capacity_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_deck_export("export_capacity", &policy);
}

/// `Export GICMvars` (Pascal `ExportGICMvar` + `TGICTransformerObj.
/// WriteVarOutputRecord`): one `Bus, Mvar, GIC Amps per phase` row per
/// GICTransformer. The GIC-study deck (`Set frequency=0.1`) exercises all three
/// types — GSU (tg1)/YY (tg2) on the K-factor Mvar path, Auto (tg3) on the
/// VarCurve path (`varcurve=vgic` → `GetYValue`). Both the Mvar and the per-phase
/// GIC magnitude are solve-derived winding currents; the quasi-DC GIC solve is
/// pinned to ~1e-8 by `corpus_live.rs` (the same `asymmetric/gic` deck), so the
/// `%.8g`-rendered cells agree at the faer-vs-KLU floor — a small rel/abs band. A
/// real regression (wrong K/VarCurve scaling, a swapped Mvar/GIC column, a dropped
/// phase in the `Curr` sum) shifts values far past the band and fails loudly.
#[test]
fn export_gicmvars_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 1e-7,
        abs: 1e-8,
        col_tol: vec![],
    };
    run_deck_export("export_gicmvars", &policy);
}

/// Split every non-empty CSV data line (after the header) into trimmed fields.
fn csv_rows(content: &str) -> Vec<Vec<String>> {
    content
        .lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.split(',').map(|f| f.trim().to_string()).collect())
        .collect()
}

/// The **degenerate no-`RelCalc`** path (audit-tests follow-up): `Export
/// BusReliability`/`BranchReliability` without a prior `RelCalc` must emit the
/// same all-zero reliability columns the oracle produces (the `Bus*`/`Branch*`
/// fields are their zero defaults). A Rust-only structural guard (no oracle — the
/// zeros are the default-branch by construction, like the step-3b `Ysc=None`
/// guard): the golden always runs `relcalc` first, so it never exercises this.
/// Guards against a regression that emitted garbage (or populated fields before
/// `RelCalc`) on the unrun path.
#[test]
fn export_reliability_without_relcalc_is_zeroed() {
    let scratch = scratch_dir("rel_norelcalc");
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.t basekv=12.47 bus1=src phases=3");
    dss.command("new line.l1 bus1=src bus2=b1 length=1 units=mi r1=0.1 x1=0.1 faultrate=0.2");
    dss.command("new line.l2 bus1=b1 bus2=b2 length=1 units=mi r1=0.1 x1=0.1 faultrate=0.3");
    dss.command("new load.ld1 bus1=b1 phases=3 kv=12.47 kw=100 numcust=10");
    dss.command("new energymeter.m1 element=line.l1 terminal=1");
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command("solve mode=snap"); // NB: no `relcalc`
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));

    // BusReliability: every value column (Lambda, Num-Interruptions,
    // Cust-Interruptions, Duration, Total-Miles — cols 1,2,4,5,6) is 0; the
    // integer Num-Customers (col 3) is also 0 without RelCalc's customer roll-up.
    dss.command("export busreliability");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let bus = std::fs::read_to_string(dss.last_result_file()).unwrap();
    let bus_rows = csv_rows(&bus);
    assert!(
        bus_rows.len() >= 3,
        "expected >=3 bus rows, got {bus_rows:?}"
    );
    for row in &bus_rows {
        for (c, field) in row.iter().enumerate().skip(1) {
            assert_eq!(
                field.parse::<f64>().unwrap_or(f64::NAN),
                0.0,
                "BusReliability col {c} must be 0 without RelCalc, row {row:?}"
            );
        }
    }

    // BranchReliability: the RelCalc-computed columns — Lambda(1),
    // Accumulated-Lambda(2), Num-Interrupts(5), Cust-Interruptions(6),
    // Cust-Durations(7), Total-Miles(8), Cust-Miles(9), SAIFI(10) — are all 0
    // without RelCalc. The Num-/Total-Customers columns (3,4) are *not* checked:
    // the branch customer counts are populated by the meter-zone build at solve
    // time (existing Phase-6 behavior, pinned by the with-RelCalc golden), not by
    // RelCalc — so a nonzero count there is faithful, not garbage.
    dss.command("export branchreliability");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let br = std::fs::read_to_string(dss.last_result_file()).unwrap();
    let br_rows = csv_rows(&br);
    assert_eq!(br_rows.len(), 2, "expected 2 branch rows, got {br_rows:?}");
    for row in &br_rows {
        for (c, field) in row.iter().enumerate().skip(1) {
            if c == 3 || c == 4 {
                continue; // zone-build customer counts, not RelCalc-populated
            }
            assert_eq!(
                field.parse::<f64>().unwrap_or(f64::NAN),
                0.0,
                "BranchReliability col {c} must be 0 without RelCalc, row {row:?}"
            );
        }
    }

    std::fs::remove_dir_all(&scratch).ok();
}

/// The **enabled-only** PDElement filter (audit-tests follow-up): a disabled PD
/// element must not appear in `Export Capacity` / `BranchReliability` (Pascal's
/// `if pElem.Enabled` guard — `for_each_enabled_elem` for Capacity, the explicit
/// `cd.enabled` skip in `export_branch_reliability`). The shared oracle fixture
/// has no disabled element, so a regression dropping the filter would produce no
/// diff there; this Rust-only structural test pins it (no oracle needed — the
/// element's presence/absence is structural).
#[test]
fn export_capacity_reliability_skip_disabled_pd() {
    let scratch = scratch_dir("rel_disabled");
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.t basekv=12.47 bus1=src phases=3");
    dss.command("new line.l1 bus1=src bus2=b1 length=1 units=mi r1=0.1 x1=0.1");
    // l2 disabled; b2 carries no load, so the solve stays well-posed.
    dss.command("new line.l2 bus1=b1 bus2=b2 length=1 units=mi r1=0.1 x1=0.1 enabled=no");
    dss.command("new load.ld1 bus1=b1 phases=3 kv=12.47 kw=100 numcust=10");
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command("solve mode=snap");
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));

    for keyword in ["capacity", "branchreliability"] {
        dss.command(&format!("export {keyword}"));
        assert!(dss.errors().is_empty(), "{keyword}: {:?}", dss.errors());
        let content = std::fs::read_to_string(dss.last_result_file()).unwrap();
        let names: Vec<String> = csv_rows(&content)
            .iter()
            .map(|r| r[0].to_lowercase())
            .collect();
        assert!(
            names.iter().any(|n| n.contains("l1")),
            "{keyword}: enabled Line.l1 must appear, got {names:?}"
        );
        assert!(
            names.iter().all(|n| !n.contains("l2")),
            "{keyword}: disabled Line.l2 must be filtered out, got {names:?}"
        );
    }

    std::fs::remove_dir_all(&scratch).ok();
}

// --- WP8.3 step 3c part 2: Overloads / Unserved / AllocationFactors ----------
// No corpus deck exports these, so each is a synthesized deck fixture
// (PHASE8_PLAN §1) tailored to make the report non-empty: an under-rated
// overloaded line (Overloads), a voltage-sagged feeder (Unserved), and
// allocation-spec loads (AllocationFactors). `gen_reports.py` captures the oracle
// output; the Rust golden replays the same deck.

/// The shared `Export Overloads` tolerance policy: every fixed-point column
/// (I1/AmpsOver/kVAOver `%…2f`, the %-loading/sequence columns `%…1f`) is
/// byte-identical on these tiny ~1e-8-agreeing circuits — exact equality.
fn overloads_policy() -> ExportPolicy {
    ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    }
}

/// `Export Overloads` (Pascal `ExportOverloads`), balanced happy path: a single
/// under-rated 3-phase line — I1/AmpsOver/kVAOver/%Normal/%Emergency with the
/// symmetrical-component columns at zero (balanced). Pins the full 11-column row
/// layout + the rating math; the nonzero-I2/I0 sym-comp path is pinned by
/// `export_overloads_unbal_matches_oracle`.
#[test]
fn export_overloads_matches_oracle() {
    run_deck_export("export_overloads", &overloads_policy());
}

/// `Export Overloads`, unbalanced + degenerate-rating: a single-phase load on a
/// 3-phase line drives **nonzero** I2/I0 (pins the `phase_to_sym` decomposition —
/// a swapped I2↔I0, dropped transform, or mis-scaled `%…/I1` would slip past the
/// all-zero balanced deck), and a second `normamps=0` line forces the degenerate
/// `NormAmps<=0` **column-shift** row (I1's trailing separator doubles with the
/// branch separator → an empty AmpsOver field). Pins both the sym-comp values
/// and the byte-exact upstream column shift against the oracle.
#[test]
fn export_overloads_unbal_matches_oracle() {
    run_deck_export("export_overloads_unbal", &overloads_policy());
}

/// `Export Estimation` (Pascal `ExportResults.pas:1652` `ExportEstimation`,
/// export verb 5; ORPHANED_GAPS §1.10). The two-section report layout: the
/// `"Energy Meters"` block (header lines 1-2, compared verbatim) then — as data
/// rows, since blank lines are dropped — the `"Sensors"` label, its own column
/// header, and one row per enabled Sensor. `sep: ','`, `rel/abs = 0.0`: every
/// number is a `%.6g` render of a quantity the allocation gate already pins to
/// ~1e-8, so the six significant digits are byte-identical.
fn estimation_policy() -> ExportPolicy {
    ExportPolicy {
        sep: ',',
        header_lines: 2, // `"Energy Meters"` + the meter column header
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    }
}

/// `Export Estimation` on the allocated fixture (`est8`): two EnergyMeters at
/// two feeder heads — a 3-phase one with unequal `peakcurrent=` targets and a
/// **1-phase** one whose `TempX[2..3]` slots must print `0` — plus three
/// Sensors covering all three spec forms (current, P/Q, voltage+current) and a
/// disabled fourth that must not appear. `allocateloads` fills
/// `CalculatedCurrent`, so the `I… Calc` and `%Err` triplets are live values
/// (17-57 % errors, no cancellation floor). Feature-sensitive: dropping the
/// `Enabled` filter adds an `S4` row; zeroing the wrong `TempX` slots or
/// re-zeroing before the percent-error pass changes M2/S3's trailing columns;
/// the sensor `%Err` columns pin the `Max(0.001, target)` denominator clamp.
#[test]
fn export_estimation_matches_oracle() {
    run_deck_export("export_estimation", &estimation_policy());
}

/// `Export Estimation` **without** `allocateloads` (`estns`): nonzero targets
/// against an all-zero `CalculatedCurrent`/`CalculatedVoltage`, so every `%Err`
/// takes the `(1 - 0/target)*100 = 100` form and the WLS residuals collapse to
/// the pure `-Weight * sum(target^2)` term (`-155.52` / `-1460`). Pins the
/// report as a *read* of stored sensor state — a version that recomputed the
/// currents itself would report nonzero `Calc` columns here.
///
/// Also the **meter-side** `Enabled` filter: the deck carries a disabled
/// EnergyMeter `mdis` on its own feeder head, which the oracle lists in
/// `Meters.AllNames` but omits from the report (probed on the pinned 0.14.5
/// engine). Dropping the port's `if !em.med.cd.enabled { continue }` adds an
/// `"Energymeter.MDIS"` row → row-count failure. (est8 cannot carry one: the
/// 0.14.5 `allocateloads` access-violates on a disabled meter, DIVERGENCES §D9.)
#[test]
fn export_estimation_noalloc_matches_oracle() {
    run_deck_export("export_estimation_noalloc", &estimation_policy());
}

/// `compare_export` runs on `report_lines()`, which **drops blank lines**, so the
/// three goldens above pin every number, row and field but not the `FSWriteln(F)`
/// separator Pascal writes between the "Energy Meters" and "Sensors" blocks
/// (`ExportResults.pas:1711`, r4133 `:1652`). This test closes that hole: replay
/// each estimation deck and require the produced file's blank-line positions and
/// total line count to match the captured oracle file exactly (audit-code D).
#[test]
fn export_estimation_blank_line_layout_matches_oracle() {
    for stem in [
        "export_estimation",
        "export_estimation_noalloc",
        "export_estimation_empty",
    ] {
        let dir = reports_dir();
        let meta: DeckMeta = {
            let p = dir.join(format!("{stem}.meta.json"));
            let text =
                std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", p.display()))
        };
        let oracle = {
            let p = dir.join(format!("{stem}.txt"));
            std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
        };

        let scratch = scratch_dir(&format!("{stem}_layout"));
        let mut dss = Dss::new();
        dss.command("clear");
        for c in &meta.deck {
            dss.command(c);
        }
        dss.command(&format!("set datapath=\"{}\"", scratch.display()));
        dss.command(&format!("export {}", meta.report));
        assert!(dss.errors().is_empty(), "{stem}: {:?}", dss.errors());
        let rust = std::fs::read_to_string(dss.last_result_file())
            .unwrap_or_else(|e| panic!("read produced {}: {e}", dss.last_result_file()));

        // Structure only: which lines are blank, and how many lines there are.
        let shape = |s: &str| -> Vec<bool> {
            s.replace("\r\n", "\n")
                .trim_end_matches('\n')
                .split('\n')
                .map(|l| l.trim().is_empty())
                .collect()
        };
        assert_eq!(
            shape(&rust),
            shape(&oracle),
            "{stem}: blank-line layout differs from the oracle"
        );
        std::fs::remove_dir_all(&scratch).ok();
    }
}

/// `Export Estimation` with no EnergyMeters and no Sensors (`estem`): both
/// section headers, both bodies empty. Pins the unconditional header emission
/// (Pascal writes them before either list walk).
#[test]
fn export_estimation_empty_matches_oracle() {
    run_deck_export("export_estimation_empty", &estimation_policy());
}

/// WP-U1.5 E2 (dss_capi 0.15.x `55400a29`): seasonal ratings, gated on
/// **capi015** (`.meta.json` `"oracle": "capi015"`). Three overloaded PDElements
/// — an overhead Line, a Transformer, and a CN cable Line — each with
/// `Seasons=4 Ratings=[...]`; `SeasonRating=yes`/`SeasonSignal=season`/`hour=13`
/// -> `SeasonalRatingIdx = trunc(GetYValue(13)) = 2`, so `Export Overloads`
/// applies `AmpRatings[2]` (via `TPDElement.GetRatings`) to **every** PDElement,
/// not just lines (0.14.5's `DI_Overloads` restriction). Revision-SENSITIVE: the
/// default 0.14.5 oracle reports base ratings (`%Normal=134.5` for L1), capi015
/// reports the seasonal ones (`%Normal=336.3`); a broken/missing seasonal
/// override diverges by >200 percentage points. The golden was regenerated with
/// `DSS_ORACLE_ENGINE=capi015`; capi015 == oddie:r4133 bit-identical (§1.7).
/// Exact `0.0/0.0` policy: the Rust render matches the capi015 golden
/// byte-for-byte (the CN-cable sparse solve agrees to render precision), so the
/// sibling non-seasonal `overloads_policy` exactness applies here too.
#[test]
fn export_overloads_seasonal_matches_capi015() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_deck_export("export_overloads_seasonal", &policy);
}

/// WP-U1.5 E2: the `Export Capacity` twin of the seasonal overload golden — the
/// same fixture, exercising `export_capacity`'s `GetRatings` wiring (the
/// `%normal`/`%emergency` columns use the seasonal `AmpRatings[2]`). Gated on
/// capi015 (`.meta.json` `"oracle": "capi015"`).
#[test]
fn export_capacity_seasonal_matches_capi015() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_deck_export("export_capacity_seasonal", &policy);
}

/// WP-U1.5 E2 audit (finding 1): `Set Hour`/`SeasonRating`/`SeasonSignal` each
/// re-sync the global `seasonal_rating_idx` (Pascal `55400a29`
/// `SyncSeasonalRatingIdx` at ExecOptions params 3/114/115 + CAPI Set_Hour), so a
/// `solve; set hour=X; export overloads` reads the NEW index, not the stale
/// solve-time one. Verified on capi015: `solve@hour0; set hour=18; export` reports
/// `AmpRatings[3]`, not `AmpRatings[0]`. Feature-sensitive — without the
/// set-command sync the index would latch at its solve-time (or `-1`) value.
#[test]
fn set_commands_resync_seasonal_rating_idx() {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.ss basekv=12.47 pu=1.0 phases=3 bus1=sourcebus");
    dss.command("new xycurve.season npts=4 xarray=[0 6 12 18] yarray=[0 1 2 3]");
    dss.command(
        "new linecode.lc nphases=3 r1=0.1 x1=0.2 r0=0.3 x0=0.6 c1=0 c0=0 normamps=100 emergamps=120",
    );
    dss.command(
        "new line.l1 bus1=sourcebus bus2=b1 linecode=lc length=1 seasons=4 ratings=[100 50 40 30]",
    );
    dss.command("new load.ld bus1=b1 phases=3 kv=12.47 kw=3000 pf=0.95 model=1");
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");

    // Feature OFF: even with a signal + hour set, the index stays -1 (inactive).
    dss.command("set hour=18");
    assert_eq!(dss.circuit().unwrap().seasonal_rating_idx, -1);
    dss.command("set seasonsignal=season");
    assert_eq!(dss.circuit().unwrap().seasonal_rating_idx, -1);

    // Enabling SeasonRating re-syncs immediately at the current hour=18 →
    // trunc(GetYValue(18)) = 3.
    dss.command("set seasonrating=yes");
    assert_eq!(dss.circuit().unwrap().seasonal_rating_idx, 3);

    // The finding-1 case: solve at hour 0, then move the hour WITHOUT re-solving —
    // the Set Hour sync advances the index (0 → 2 for hour=12).
    dss.command("set mode=snap");
    dss.command("set hour=0");
    assert_eq!(dss.circuit().unwrap().seasonal_rating_idx, 0);
    dss.command("solve");
    assert!(dss.errors().is_empty(), "solve errors: {:?}", dss.errors());
    dss.command("set hour=12");
    assert_eq!(dss.circuit().unwrap().seasonal_rating_idx, 2);
}

/// WP-U1.5 E2 audit (finding 4): the `DI_Overloads` demand-interval path
/// (`write_overload_report`) applies the seasonal rating too — Pascal `55400a29`
/// `EnergyMeter.pas::WriteOverloadReport` keeps the BASE `NormAmps`/`EmergAmps`
/// entry gate but uses `AmpRatings[SeasonalRatingIdx]` (guard `0 <= idx <
/// NumAmpRatings`) for the overload test AND the reported `Normal Amps`/`Emerg
/// Amps` columns. Feature-sensitive & solve-independent: `Normal Amps` is a deck
/// constant (`AmpRatings[2]`), so it pins the seasonal wiring, not the numeric
/// solve. A constant `SeasonSignal` (`yarray=[2 2 2 2]`) fixes the index at 2 for
/// every daily step, so `Line.L1` reports `AmpRatings[2] = 40` (NOT the base
/// linecode `NormAmps = 100`); a regression to base ratings flips it to 100.
#[test]
fn di_overloads_applies_seasonal_rating() {
    let scratch = scratch_dir("di_overloads_seasonal");
    let mut dss = Dss::new();
    dss.command("clear");
    for c in [
        "new circuit.ss basekv=12.47 pu=1.0 phases=3 bus1=sourcebus",
        "new xycurve.season npts=4 xarray=[0 6 12 18] yarray=[2 2 2 2]",
        "new linecode.lc nphases=3 r1=0.1 x1=0.2 r0=0.3 x0=0.6 c1=0 c0=0 normamps=100 emergamps=120",
        "new line.l1 bus1=sourcebus bus2=b1 linecode=lc length=1 seasons=4 ratings=[100 50 40 30]",
        "new load.ld bus1=b1 phases=3 kv=12.47 kw=3000 pf=0.95 model=1",
        "new energymeter.em1 element=Line.l1 terminal=1",
        "set voltagebases=[12.47]",
        "calcvoltagebases",
        "set seasonrating=yes",
        "set seasonsignal=season",
    ] {
        dss.command(c);
    }
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    dss.command("solve");
    dss.command("set demandinterval=yes");
    dss.command("set overloadreport=yes");
    dss.command("set mode=daily number=2 stepsize=1h");
    dss.command("solve");
    dss.command("closedi");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    // Locate the produced DI_Overloads file (scratch/<case>/DI_yr_0/…).
    let di = find_file(&scratch, "DI_Overloads")
        .unwrap_or_else(|| panic!("no DI_Overloads file under {}", scratch.display()));
    let text = std::fs::read_to_string(&di).unwrap();
    let l1_norm = text
        .lines()
        .skip(1)
        .find_map(|line| {
            let f: Vec<&str> = line
                .split(',')
                .map(|s| s.trim().trim_matches('"'))
                .collect();
            (f.len() > 3 && f[1].eq_ignore_ascii_case("Line.L1"))
                .then(|| f[2].parse::<f64>().ok())
                .flatten()
        })
        .unwrap_or_else(|| panic!("no Line.L1 row in DI_Overloads:\n{text}"));
    // Seasonal AmpRatings[2] = 40, NOT the base linecode NormAmps = 100.
    assert!(
        (l1_norm - 40.0).abs() < 1e-6,
        "DI_Overloads Line.L1 Normal Amps = {l1_norm}, expected seasonal AmpRatings[2] = 40 \
         (base NormAmps = 100 would mean the seasonal override is not wired)"
    );
    std::fs::remove_dir_all(&scratch).ok();
}

/// Recursively find the first file whose name contains `needle`.
fn find_file(dir: &Path, needle: &str) -> Option<PathBuf> {
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let p = entry.path();
        if p.is_dir() {
            if let Some(hit) = find_file(&p, needle) {
                return Some(hit);
            }
        } else if p.file_name()?.to_string_lossy().contains(needle) {
            return Some(p);
        }
    }
    None
}

/// The shared `Export Unserved` tolerance policy: `kW` is a deck constant
/// (`%8.0f`) and `EEN_Factor`/`UE_Factor` are `%9.3f` renders of ~1e-8-agreeing
/// solves — byte-identical, exact equality; a real regression (wrong criterion,
/// a missing/extra load, a swapped EEN/UE) fails loudly.
fn unserved_policy() -> ExportPolicy {
    ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    }
}

/// `Export Unserved` (Pascal `ExportUnserved`), normal criterion: one load below
/// `NormalMinVolts` → a nonzero `EEN_Factor` via `ExceedsNormal`. Pins the normal
/// (`ue_only = false`) path + the row layout.
#[test]
fn export_unserved_matches_oracle() {
    run_deck_export("export_unserved", &unserved_policy());
}

/// `Export Unserved u…` (Pascal `ExportUnserved` with `UE_Only`), emergency
/// criterion: a deep-sag load below `EmergMinVolts` → a nonzero `UE_Factor` via
/// the `Unserved` path, plus a healthy load that must be **excluded**. Pins the
/// `ue_only = true` branch (the `u` pre-parse) and the criterion filter — the
/// normal-criterion golden alone can't catch a broken UE criterion.
#[test]
fn export_unserved_ue_matches_oracle() {
    run_deck_export("export_unserved_ue", &unserved_policy());
}

/// `Export AllocationFactors` (Pascal `DumpAllocationFactors`): one
/// `Load.<name>.AllocationFactor=<f>` / `.CFactor=<f>` line per allocation-spec
/// load — no header, `=`-separated. The factors are deck constants (`%-.5g`, no
/// solve dependency), byte-identical — exact equality. The fixture
/// covers the spec-type dispatch (a ConnectedkVA `la` + a kWh `lb` emit; a plain
/// `kw=` load `lc` emits **nothing** — the `ExactOrdered` row count guards the
/// no-emit branch) **and** the no-`Enabled`-filter walk (a *disabled*
/// ConnectedkVA load `ld` still emits, matching Pascal `for pLoad in Loads`).
#[test]
fn export_allocationfactors_matches_oracle() {
    let policy = ExportPolicy {
        sep: '=',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_deck_export("export_allocationfactors", &policy);
}

// --- WP8.3 step 3c part 3: Sections / Profile --------------------------------
// `Export Sections` (Pascal `ExportSections`) dumps the per-feeder-section
// reliability data a prior `RelCalc` persisted on each meter (`SectionCount` +
// `FeederSections` — the persistence this step added); `Export Profile` (Pascal
// `ExportProfile` + `WriteNewLine`) is the per-meter branch-list voltage
// profile over the zone-build `DistFromMeter`. No corpus deck exports either,
// so both fixtures are synthesized (PHASE8_PLAN §1).

/// The `Export Sections` tolerance policy: the aggregate columns
/// (AvgRepairHrs/SectFaultRate/Sum*) are `%-.6g` over `RelCalc` arithmetic that
/// is identical on both engines — byte-identical, exact equality (integer
/// id/count columns and the Meter/DeviceType/HeadBranch text included).
fn sections_policy() -> ExportPolicy {
    ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    }
}

/// `Export Sections` (all meters): m1's two recloser-headed sections + m2's
/// fuse-headed section (the FUSE arm of `getOCPDeviceTypeString`), each row's
/// SeqIndex/customer/branch counts + fault-rate/repair aggregates + the quoted
/// head-branch FullName pinned against the oracle.
#[test]
fn export_sections_matches_oracle() {
    run_deck_export("export_sections", &sections_policy());
}

/// `Export Sections meter=m2` (the named-meter pre-parse, Pascal
/// `CompareTextShortest(ParamName, 'meter')` + `EnergyMeterClass.Find`): only
/// m2's section may appear — the all-meters golden alone can't catch a dropped
/// meter filter.
#[test]
fn export_sections_meter_matches_oracle() {
    run_deck_export("export_sections_meter", &sections_policy());
}

/// `Export Sections` structural edge paths (no oracle capture needed — both are
/// row-set–structural, and the row *values* are already oracle-pinned by the
/// goldens above): (1) **without a prior `RelCalc`** the report is header-only
/// (every meter's `SectionCount` is its zero default — Pascal `FeederSections =
/// NIL`); (2) an **unknown `meter=` name** silently falls back to all meters
/// (Pascal `Find` returns NIL → the all-meters branch, no error).
#[test]
fn export_sections_edge_paths() {
    let dir = reports_dir();
    let meta: DeckMeta = {
        let p = dir.join("export_sections.meta.json");
        let text =
            std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", p.display()))
    };

    // (1) The same deck WITHOUT the trailing `relcalc` → header-only.
    let scratch = scratch_dir("sections_norelcalc");
    let mut dss = Dss::new();
    dss.command("clear");
    for c in meta
        .deck
        .iter()
        .filter(|c| !c.eq_ignore_ascii_case("relcalc"))
    {
        dss.command(c);
    }
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    dss.command("export sections");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let content = std::fs::read_to_string(dss.last_result_file()).unwrap();
    assert_eq!(
        content.lines().filter(|l| !l.trim().is_empty()).count(),
        1,
        "no RelCalc → header-only Sections report; got:\n{content}"
    );

    // (2) Full deck, unknown meter name → the all-meters row set (3 sections).
    dss.command("relcalc");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss.command("export sections meter=nosuchmeter");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let content = std::fs::read_to_string(dss.last_result_file()).unwrap();
    let rows = csv_rows(&content);
    assert_eq!(
        rows.len(),
        3,
        "unknown meter= must fall back to all meters, got {rows:?}"
    );

    std::fs::remove_dir_all(&scratch).ok();
}

/// The shared `Export Profile` tolerance policy: the `%.6g` `puV` columns, the
/// zone-build `Distance` constants and the integer Color/Thickness/Linetype/
/// marker columns are all byte-identical — exact equality. The header line
/// (incl. the appended `Title=…` tail) is compared verbatim.
fn profile_policy() -> ExportPolicy {
    ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    }
}

/// `Export Profile` (Pascal `ExportProfile` + `WriteNewLine`) on the metered,
/// solved IEEE13 feeder — all seven `PhasesToPlot` selector variants from one
/// compile: the default (3-phase primary, the unguarded per-phase write), `all`
/// and `primary` (per-present-phase L-N — the single-phase 684652/684611
/// branches pin the phase-presence filter), the three L-L forms (`ll3ph`/
/// `llall`/`llprimary` — the `|V1−V2|/kV/1732` pairs + their guards + the
/// `Title=L-L …` header tail), and an explicit phase `2` (the single-character
/// `Parser.IntValue` re-read; only phase-2-carrying branches may appear).
#[test]
fn export_profile_variants_match_oracle() {
    run_shared_exports(&[
        ("export_profile", profile_policy()),
        ("export_profile_all", profile_policy()),
        ("export_profile_primary", profile_policy()),
        ("export_profile_ll3ph", profile_policy()),
        ("export_profile_llall", profile_policy()),
        ("export_profile_llprimary", profile_policy()),
        ("export_profile_ph2", profile_policy()),
    ]);
}

// --- WP8.3 step 4: TSystemMeter + the demand-interval (DI_) files ------------
// The DI files are written DURING the time-series solve (opened by SolveDaily,
// one row per SampleAll, closed by its finally) — not by an Export command — so
// the runner sets the datapath BEFORE replaying the post commands and reads the
// produced files from `<datapath>/<case>/DI_yr_0/`. One daily-3 IEEE13 run with
// a PhaseVoltageReport meter and all four DI switches produces all nine files:
// the per-meter DI + phase-voltage report, the system-meter DI + cumulative
// registers, the DI/grand totals, the meter-totals dump, and the overload /
// voltage-exception reports. The fixture pre-solves a snapshot so the meter
// zone (VBaseList + vbase register names) exists when the files open — without
// it the oracle renders *uninitialized heap garbage* as PHV vbase labels
// (unpinnable; see gen_reports.py).

/// Meta for a demand-interval golden: the produced file's path relative to the
/// datapath (`<case>/DI_yr_0/<file>`) instead of an export suffix.
#[derive(Debug, Deserialize)]
struct DiMeta {
    master: String,
    post: Vec<String>,
    fixture: String,
    relpath: String,
}

/// All DI values are `%-g` (15 sig) doubles computed by the same meter/solve
/// arithmetic `corpus_live.rs` pins to ~1e-8 on this exact IEEE13 daily path —
/// the **only** WP8 golden family with a genuine (non-print) Rust↔oracle
/// residual: the per-step faer-vs-KLU voltage difference integrates across the
/// 24 daily solves into the registers (**measured** max 1.5e-8 rel, on the
/// `di_totals` Load EEN register). `rel = 5e-8` = ~3× that measured accumulation
/// floor (the `corpus_live` class, not a print floor). `abs = 0`: idle registers
/// print an identical exact `0` on both engines. Headers (incl. the
/// `4.16kV_Phs_…` PHV labels and the register-name columns) compare verbatim;
/// bus names case-insensitively.
fn di_policy() -> ExportPolicy {
    ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 5e-8,
        abs: 0.0,
        col_tol: vec![],
    }
}

/// The nine demand-interval files from one daily IEEE13 run, each diffed
/// against the oracle's capture.
#[test]
fn demand_interval_files_match_oracle() {
    let stems = [
        "di_em1",
        "di_em1_phv",
        "di_systemmeter",
        "di_totals",
        "di_overloads",
        "di_voltexceptions",
        "di_energymetertotals",
        "di_grand_totals",
        "di_systemmeter_registers",
    ];
    let dir = reports_dir();
    let metas: Vec<DiMeta> = stems
        .iter()
        .map(|stem| {
            let p = dir.join(format!("{stem}.meta.json"));
            let text =
                std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", p.display()))
        })
        .collect();
    let m0 = &metas[0];
    for m in &metas[1..] {
        assert_eq!(m.master, m0.master, "DI goldens disagree on master");
        assert_eq!(m.post, m0.post, "DI goldens disagree on post");
        assert_eq!(m.fixture, m0.fixture, "DI goldens disagree on fixture");
    }

    let master: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "corpus",
        "electricdss-tst",
    ]
    .iter()
    .collect::<PathBuf>()
    .join(&m0.master);
    assert!(master.is_file(), "master missing: {}", master.display());

    let scratch = scratch_dir("demand_interval");
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!(
        "compile \"{}\"",
        master.to_string_lossy().replace('\\', "/")
    ));
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    for c in &m0.post {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    for (stem, m) in stems.iter().zip(&metas) {
        let produced = scratch.join(&m.relpath);
        let rust = std::fs::read_to_string(&produced)
            .unwrap_or_else(|e| panic!("{stem}: read produced {}: {e}", produced.display()));
        let oracle = {
            let p = dir.join(format!("{stem}.txt"));
            std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
        };
        compare_export(&oracle, &rust, &di_policy(), stem);
    }

    std::fs::remove_dir_all(&scratch).ok();
}

/// The yearly-mode DI lifecycle (Pascal `SolveYearly` has **no**
/// `CloseAllDIFiles` in its finally — the files stay open, accumulating across
/// runs, until a `CloseDI`/`Reset`/`Set year=` closes them). Rust-only
/// structural gate: after a yearly solve the per-meter DI file must NOT exist
/// yet (its stream is still in memory) and `DIFilesAreOpen` holds; `closedi`
/// then writes it with one row per solved step.
#[test]
fn demand_interval_yearly_stays_open_until_closedi() {
    let dir = reports_dir();
    let meta: DiMeta = {
        let p = dir.join("di_em1.meta.json");
        let text =
            std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", p.display()))
    };
    let master: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "corpus",
        "electricdss-tst",
    ]
    .iter()
    .collect::<PathBuf>()
    .join(&meta.master);

    let scratch = scratch_dir("di_yearly");
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!(
        "compile \"{}\"",
        master.to_string_lossy().replace('\\', "/")
    ));
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    dss.command("new energymeter.em1 element=Line.650632 terminal=1");
    dss.command("solve");
    dss.command("set demandinterval=yes");
    dss.command("set diverbose=yes");
    dss.command("set mode=yearly number=3 stepsize=1h");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    let di_file = scratch
        .join(&meta.fixture)
        .join("DI_yr_0")
        .join("em1_1.csv");
    assert!(
        !di_file.exists(),
        "yearly must leave the DI files open (in memory), not written"
    );

    dss.command("closedi");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let content = std::fs::read_to_string(&di_file)
        .unwrap_or_else(|e| panic!("closedi must write {}: {e}", di_file.display()));
    let lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(
        lines.len(),
        4,
        "expected header + 3 yearly rows, got:\n{content}"
    );
    assert!(lines[0].starts_with("\"Hour\""), "header: {:?}", lines[0]);

    std::fs::remove_dir_all(&scratch).ok();
}

/// The `Set year=` demand-interval side effects (Pascal `TSolutionObj.Set_Year`,
/// Solution.pas:2266; wired into the YEAR handler in step 4): close any open DI
/// files, restart the clock (`intHour=0`/`t=0`), then `ResetAll` (which rebuilds
/// the `DI_yr_<year>` directory). This is the twin of
/// `demand_interval_yearly_stays_open_until_closedi` — it closes the open files
/// via `set year=` instead of `closedi`. A yearly run leaves year-0's files open;
/// `set year=1` flushes `DI_yr_0/em1_1.csv` (header + 3 rows) and rolls the clock;
/// a fresh yearly run then writes into a new `DI_yr_1/`.
#[test]
fn set_year_closes_di_and_rolls_the_year_directory() {
    let dir = reports_dir();
    let meta: DiMeta = {
        let p = dir.join("di_em1.meta.json");
        let text =
            std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", p.display()))
    };
    let master: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "corpus",
        "electricdss-tst",
    ]
    .iter()
    .collect::<PathBuf>()
    .join(&meta.master);

    let scratch = scratch_dir("di_set_year");
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!(
        "compile \"{}\"",
        master.to_string_lossy().replace('\\', "/")
    ));
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    dss.command("new energymeter.em1 element=Line.650632 terminal=1");
    dss.command("solve");
    dss.command("set demandinterval=yes");
    dss.command("set diverbose=yes");
    dss.command("set mode=yearly number=3 stepsize=1h");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    let di0 = scratch
        .join(&meta.fixture)
        .join("DI_yr_0")
        .join("em1_1.csv");
    assert!(
        !di0.exists(),
        "yearly leaves DI files open (in memory), not written"
    );

    // `set year=1` closes year-0's files (writing them) and resets the clock.
    dss.command("set year=1");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let c0 = std::fs::read_to_string(&di0)
        .unwrap_or_else(|e| panic!("set year= must flush {}: {e}", di0.display()));
    let n0 = c0.lines().filter(|l| !l.trim().is_empty()).count();
    assert_eq!(n0, 4, "year-0 DI: header + 3 rows, got:\n{c0}");

    // A fresh yearly run now writes into the rebuilt DI_yr_1 directory.
    dss.command("set mode=yearly number=2 stepsize=1h");
    dss.command("solve");
    dss.command("closedi");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let di1 = scratch
        .join(&meta.fixture)
        .join("DI_yr_1")
        .join("em1_1.csv");
    let c1 = std::fs::read_to_string(&di1)
        .unwrap_or_else(|e| panic!("year-1 DI must be written to DI_yr_1: {e}"));
    let n1 = c1.lines().filter(|l| !l.trim().is_empty()).count();
    assert_eq!(n1, 3, "year-1 DI: header + 2 rows, got:\n{c1}");

    std::fs::remove_dir_all(&scratch).ok();
}

/// The demand-interval `Set`/`Get` option plumbing (step 4): the five report
/// switches echo their value via `Get` (Pascal `ExecOptions.pas:950-968`) and the
/// `Markercode`/`Nodewidth` plot-marker state (Circuit.pas 16/1 defaults). The
/// echoes are **oracle-pinned** (probe: defaults No/No/No/No/No + 16/1; after the
/// sets Yes/Yes/Yes/Yes/No + 7/3). Covers the `Set SampleEnergyMeters=` handler
/// and the five getters the step added.
#[test]
fn di_set_get_option_echoes_match_oracle() {
    // `set demandinterval=yes` runs `ResetAll`, which creates `<case>/DI_yr_0`
    // under the output directory — point it at scratch so nothing lands in-tree.
    let scratch = scratch_dir("di_options");
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.c");
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));

    let get = |dss: &mut Dss, opt: &str| {
        dss.command(&format!("get {opt}"));
        dss.result().to_string()
    };
    let opts = [
        "demandinterval",
        "diverbose",
        "overloadreport",
        "voltexceptionreport",
        "sampleenergymeters",
    ];
    // Defaults.
    for o in opts {
        assert_eq!(get(&mut dss, o), "No", "default {o}");
    }
    assert_eq!(get(&mut dss, "markercode"), "16");
    assert_eq!(get(&mut dss, "nodewidth"), "1");

    for c in [
        "set demandinterval=yes",
        "set diverbose=yes",
        "set overloadreport=yes",
        "set voltexceptionreport=yes",
        "set sampleenergymeters=no",
        "set markercode=7",
        "set nodewidth=3",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(get(&mut dss, "demandinterval"), "Yes");
    assert_eq!(get(&mut dss, "diverbose"), "Yes");
    assert_eq!(get(&mut dss, "overloadreport"), "Yes");
    assert_eq!(get(&mut dss, "voltexceptionreport"), "Yes");
    assert_eq!(get(&mut dss, "sampleenergymeters"), "No"); // set to no
    assert_eq!(get(&mut dss, "markercode"), "7");
    assert_eq!(get(&mut dss, "nodewidth"), "3");

    std::fs::remove_dir_all(&scratch).ok();
}

/// `New Spectrum … CSVFile=…` through the executive (step 5): the deferred
/// `FileLoad` path (`take_file_loads` → the executive reads the file →
/// `apply_file_load` → `read_csv_file` → `end_edit`) loads the harmonics from a
/// real on-disk file, and the byte-position EOF guard (`(F.Position+1) < F.Size`)
/// drops a lone final ≤1-byte line. Both are oracle-confirmed (`? Spectrum.s.…`:
/// 3 rows for the well-formed file, 2 for the stray-final-byte file).
#[test]
fn spectrum_csvfile_loads_through_executive() {
    let scratch = scratch_dir("spectrum_csv");
    let path = |name: &str| {
        scratch
            .join(name)
            .to_string_lossy()
            .replace('\\', "/")
            .to_string()
    };
    std::fs::write(scratch.join("full.csv"), "1, 100, 0\n5, 20, 0\n7, 10, 0\n").unwrap();
    std::fs::write(scratch.join("stray.csv"), "1, 100, 0\n5, 20, 0\n7").unwrap();

    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.c");
    dss.command(&format!(
        "new spectrum.s NumHarm=3 CSVFile=({})",
        path("full.csv")
    ));
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss.command("? spectrum.s.NumHarm");
    assert_eq!(
        dss.result(),
        "3",
        "3 well-formed rows load through the executive"
    );
    dss.command("? spectrum.s.Harmonic");
    assert_eq!(dss.result(), "[ 1 5 7]");

    dss.command(&format!(
        "new spectrum.s2 NumHarm=3 CSVFile=({})",
        path("stray.csv")
    ));
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss.command("? spectrum.s2.NumHarm");
    assert_eq!(
        dss.result(),
        "2",
        "byte-position EOF guard drops the lone final byte"
    );

    std::fs::remove_dir_all(&scratch).ok();
}

/// Meta for a **deck-based** demand-interval golden (a `New`-circuit deck + the
/// post-datapath solve commands — like `DiMeta`, but with an inline `deck`
/// instead of a `master` compile).
#[derive(Debug, Deserialize)]
struct DeckDiMeta {
    deck: Vec<String>,
    post: Vec<String>,
    #[allow(dead_code)]
    fixture: String,
    relpath: String,
}

/// `DI_Overloads` for a synthesized single-phase (phase-2) overloaded lateral —
/// pins `WriteOverloadReport`'s `NPhases < 3` phase-mapping branch: the lateral's
/// terminal-1 current must land in the **I2** column (via `MapNodeToBus.node_num`,
/// the replacement for Pascal's `FirstBus` string-parse), with I1 and I3 zero. The
/// 3-phase `di_overloads` golden can't catch a wrong column mapping (all its rows
/// are 3-phase); this is the discriminating case.
#[test]
fn di_overloads_single_phase_mapping_matches_oracle() {
    let dir = reports_dir();
    let meta: DeckDiMeta = {
        let p = dir.join("di_overloads_1ph.meta.json");
        let text =
            std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", p.display()))
    };
    let oracle = {
        let p = dir.join("di_overloads_1ph.txt");
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
    };

    let scratch = scratch_dir("di_overloads_1ph");
    let mut dss = Dss::new();
    dss.command("clear");
    for c in &meta.deck {
        dss.command(c);
    }
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    for c in &meta.post {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    let produced = scratch.join(&meta.relpath);
    let rust = std::fs::read_to_string(&produced)
        .unwrap_or_else(|e| panic!("read produced {}: {e}", produced.display()));

    compare_export(&oracle, &rust, &di_policy(), "di_overloads_1ph");

    std::fs::remove_dir_all(&scratch).ok();
}

// --- WP8.5 step 1: Dump (single-object forms) ---

/// `Dump reactor.*` (base) — the `TReactorObj.DumpProperties` override: the
/// `New "…"` header, `! ENABLED`, the property loop with the reactor's custom
/// Z/LmH formats, the matrix skip (r1) and the un-`~` `RMatrix=`/`XMatrix=` lines
/// (rz). Byte-exact.
#[test]
fn dump_reactor_matches_oracle() {
    run_deck_dump_exact("dump_reactor");
}

/// `Dump reactor.* debug` — adds the `TDSSCktElement.DumpProperties` Complete
/// block: NPhases/Nconds/Nterms/Yorder, the NodeRef list, terminal open/closed
/// status + bus refs, and the `%13.10g` primitive-Y G/B full matrices. Byte-exact
/// (the assembled Y is bit-exact on this LineCode-free reactor deck).
#[test]
fn dump_reactor_debug_matches_oracle() {
    run_deck_dump_exact("dump_reactor_debug");
}

/// `Dump reactor.rk debug` — a **symmetrical-components** (`SpecType=4`) reactor
/// (the corpus `KerstingMotor` shape): pins the NON-zero `Z1/Z2/Z0` `%-.8g`
/// complex path (audit-tests #1) that the r1/rz fixtures never exercised, and the
/// **single-object** (non-glob) dispatch form (audit-tests #5).
#[test]
fn dump_reactor_symcomp_matches_oracle() {
    run_deck_dump_exact("dump_reactor_symcomp");
}

/// `Dump reactor.* debug` with a **disabled** reactor: pins `! DISABLED`,
/// `NodeRef = "nil"`, and `Terminal Bus Ref: [-1 …]` — the Pascal `BusRef = -1`
/// "not set" value for an unresolved terminal (audit-code Finding 1: the port
/// previously printed `0`; audit-tests #4).
#[test]
fn dump_reactor_disabled_matches_oracle() {
    run_deck_dump_exact("dump_reactor_disabled");
}

/// `Dump loadshape.ls1` — the **generic base** plain-`TDSSObject` path (no
/// override, no `! ENABLED`): header + `~ prop=value` loop. LoadShape's port
/// property names already carry the oracle display case, so this pins the
/// generic plain-object dump for free (audit-tests #2).
#[test]
fn dump_loadshape_matches_oracle() {
    run_deck_dump_exact("dump_loadshape");
}

// --- WP8.5 step 2: per-winding / matrix DumpProperties overrides ---

/// `Dump transformer.t1 debug` (2-winding) — the `TTransfObj.DumpProperties`
/// per-winding block + `XHL…X23`/`Xscmatrix`/thermal scalars + the generic tail,
/// then the Complete `ZB`/`ZB (inverted)`/`Y_OneVolt`/`Y_Terminal`/`TermRef`
/// dumps (the last exercises the `%g` scientific / `%.4f` complex forms).
#[test]
fn dump_transformer_matches_oracle() {
    run_deck_dump_exact("dump_transformer");
}

/// `Dump transformer.t3 debug` (3-winding) — the `Xsc` off-diagonal, `X13`/`X23`,
/// and the order-2 `ZB` / order-3 `Y_OneVolt` / order-6 `Y_Terminal` matrices.
#[test]
fn dump_transformer3_matches_oracle() {
    run_deck_dump_exact("dump_transformer3");
}

/// `Dump transformer.t1` on a **disabled** (post-solve `enabled=no`) transformer:
/// NodeRef stays populated, so `WdgCurrents` reaches `0` only via the `not
/// Enabled` guard (Pascal `Transformer.pas:1530`; audit-code follow-up).
#[test]
fn dump_transformer_disabled_matches_oracle() {
    run_deck_dump_exact("dump_transformer_disabled");
}

/// `Dump autotrans.t1` (WPG.15 Stage A) — the AutoTrans `DumpProperties`
/// per-winding block (`conn=Series`, the winding scalars as `%.7g`),
/// `XHX`/`XHT`/`XXT` (no `X12`/`X13`/`X23`), the flat `Xscmatrix` and the
/// generic tail. Bare (non-`debug`) + on buses isolated from the source, so the
/// dump is solve-independent (the auto YPrim/solve path is Stage B).
#[test]
fn dump_autotrans_matches_oracle() {
    run_deck_dump_exact("dump_autotrans");
}

/// `Dump autotrans.t3` (3-winding, delta tertiary) — the 3-winding Xscmatrix
/// (three off-diagonals) and the wye/delta/Series `conn` render arms.
#[test]
fn dump_autotrans3_matches_oracle() {
    run_deck_dump_exact("dump_autotrans3");
}

/// `Dump line.l1` — the `TLineObj.DumpProperties` sym-components path: the
/// `%-.7g` sequence params + the `RMatrix`/`XMatrix`/`CMatrix` folded out of
/// `Z`/`Yc`.
#[test]
fn dump_line_sym_matches_oracle() {
    run_deck_dump_exact("dump_line_sym");
}

/// `Dump line.lg` — a **geometry**-driven line: `length` is embedded in `Z`/`Yc`,
/// so the matrix cells divide by `LengthMult = Len` (≠1) to recover per-unit
/// length (the branch the sym/linecode decks never hit; audit-tests follow-up #1).
#[test]
fn dump_line_geo_matches_oracle() {
    run_deck_dump_exact("dump_line_geo");
}

/// `Dump line.sw` — a **switch** line pins `Switch=Yes` (audit-tests follow-up #2).
#[test]
fn dump_line_switch_matches_oracle() {
    run_deck_dump_exact("dump_line_switch");
}

/// `Dump line.l2` — a LineCode-driven line: the matrix model, so the sequence
/// params render the literal `----` and the matrices come from the code.
#[test]
fn dump_line_lc_matches_oracle() {
    run_deck_dump_exact("dump_line_lc");
}

/// `Dump linecode.lc1` — `TLineCodeObj.DumpProperties` sym model (`%.5f`
/// scalars + the `%.8f` matrices derived from R1/X1/R0/X0/C1/C0).
#[test]
fn dump_linecode_sym_matches_oracle() {
    run_deck_dump_exact("dump_linecode_sym");
}

/// `Dump linecode.lc2` — the matrix-model LineCode (`RMatrix`/`XMatrix`/`CMatrix`
/// dumped `%.8f`).
#[test]
fn dump_linecode_matrix_matches_oracle() {
    run_deck_dump_exact("dump_linecode_matrix");
}

/// `Dump linegeometry.geo1` — `TLineGeometryObj.DumpProperties`: the `! WARNING`
/// banner + the per-conductor `Cond`/`Wire`/`X`/`H`/`Units` block (the
/// `ActiveCond` walk).
#[test]
fn dump_linegeometry_matches_oracle() {
    run_deck_dump_exact("dump_linegeometry");
}

/// `Dump xfmrcode.xc1` — `TXfmrCodeObj.DumpProperties`: the transformer
/// per-winding block without buses / Complete matrix block.
#[test]
fn dump_xfmrcode_matches_oracle() {
    run_deck_dump_exact("dump_xfmrcode");
}

// --- WP8.5 step 3a: the 8 remaining leaf `DumpProperties` overrides ---

/// `Dump vsource.source debug` — `TVsourceObj.DumpProperties`: generic props,
/// then Complete's `BaseFrequency`/`VMag`/base-frequency series `Z Matrix`
/// lower triangle (`%.8g +j %.8g `, no `~` prefix).
#[test]
fn dump_vsource_matches_oracle() {
    run_deck_dump_exact("dump_vsource");
}

/// `Dump upfc.u1 debug` — `TUPFCObj.DumpProperties`: same shape as VSource but
/// with no `VMag` line; `Z Matrix` is the diagonal `(0, Xs)` series reactance.
#[test]
fn dump_upfc_matches_oracle() {
    run_deck_dump_exact("dump_upfc");
}

/// `Dump regcontrol.rc1 debug` — `TRegControlObj.DumpProperties`: generic
/// props then Complete's `! Bus =<GetBus(1)>` + blank line.
#[test]
fn dump_regcontrol_matches_oracle() {
    run_deck_dump_exact("dump_regcontrol");
}

/// `Dump monitor.mon1 debug` — `TMonitorObj.DumpProperties`: generic props
/// then Complete's `// BufferSize`/`// Hour`/`// Sec`/`// BaseFrequency`/
/// `// Bufptr`/`// Buffer` comment block (an unsampled monitor: empty buffer).
#[test]
fn dump_monitor_matches_oracle() {
    run_deck_dump_exact("dump_monitor");
}

/// `Dump monitor.m1 debug` after a `mode=Time` (SolveGeneralTime) run — the
/// one solve mode that never calls `MonitorClass.SaveAll`, so the dump shows a
/// **non-empty** `// Bufptr=56`/`// Buffer=` block: four buffered records
/// (`Set LoadShapeClass=Daily` makes the load follow `d4`, so each record's
/// V/I differ). Byte-pins the port's Complete-dump pending-render path itself,
/// which `dump_monitor_matches_oracle` never reaches (that fixture flushes, so
/// its buffer is empty). Concretely it catches: a premature flush (the block
/// would be empty), a wrong record stride/layout (record count `Bufptr/stride`
/// or the per-row wrap of `2 + Nconds*4` would shift), the `%.1f` value format,
/// and the `// Bufptr=`/`// Hour=`/`// Sec=` header. (This case runs with
/// `flushed_records = 0`, so it does NOT exercise a non-zero cursor offset —
/// that arithmetic is pinned by the `channel_reflects_flush_state` unit test.)
#[test]
fn dump_monitor_montime_matches_oracle() {
    run_deck_dump_exact("dump_monitor_montime");
}

/// `Dump energymeter.em1 debug` — `TEnergyMeterObj.DumpProperties`: generic
/// props then Complete's `Registers` block + the `Branch List:` zone-tree walk
/// (`Circuit Element =`/`   Shunt Element =`).
#[test]
fn dump_energymeter_matches_oracle() {
    run_deck_dump_exact("dump_energymeter");
}

/// `Dump spectrum.sp5 debug` — `TSpectrumObj.DumpProperties`: a plain
/// `TDSSObject` (no `! ENABLED`); Complete adds the `Multiplier Array:` table.
#[test]
fn dump_spectrum_matches_oracle() {
    run_deck_dump_exact("dump_spectrum");
}

/// `Dump fault.f1 debug` — `TFaultObj.DumpProperties` (`SpecType=1`, single
/// `r`): the custom Bus1/Bus2/Phases/R/pctStdDev/OnTime/Temporary/MinAmps
/// lines, then the tail from `MinAmps` — pinning the upstream double-print
/// quirk (`MinAmps` appears twice: the custom `%.1f` line, then generically).
#[test]
fn dump_fault_matches_oracle() {
    run_deck_dump_exact("dump_fault");
}

/// `Dump fault.fg debug` — `TFaultObj.DumpProperties` (`SpecType=2`, a
/// `Gmatrix`): pins the custom `~ GMatrix= (…)` lower-triangle render.
#[test]
fn dump_fault_gmatrix_matches_oracle() {
    run_deck_dump_exact("dump_fault_gmatrix");
}

/// `Dump capacitor.cm1 debug` (a `CMatrix`-spec bank) — `TCapacitorObj.
/// DumpProperties`: generic props then the bare `SpecType=<int>` line (no
/// `~`/`//` prefix). The three ASLR-garbage lines (`~ CMatrix=(`/
/// `~ FaultRate=`/`~ pctPerm=`) are masked on both sides — see
/// `run_deck_dump_exact_masked` and `investigations/`.
#[test]
fn dump_capacitor_cmatrix_matches_oracle() {
    run_deck_dump_exact_masked(
        "dump_capacitor_cmatrix",
        &["~ CMatrix=(", "~ FaultRate=", "~ pctPerm="],
    );
}

/// `Dump capacitor.cs1 debug` (a switchable-step `kvar`/`kV`-spec bank,
/// `SpecType=1`) — the same override, different spec/step-array shape.
#[test]
fn dump_capacitor_steps_matches_oracle() {
    run_deck_dump_exact_masked(
        "dump_capacitor_steps",
        &["~ CMatrix=(", "~ FaultRate=", "~ pctPerm="],
    );
}

// --- WPG.14: Isource ---

/// `Dump isource.*` — Isource has no Pascal `DumpProperties` override, so it
/// falls through to the ancestor `TPCElement.DumpProperties` ordering; pins
/// the two-unit glob (a full-spec 3-phase unit + a disabled 1-phase unit —
/// the `! DISABLED` / default-Bus2-uses-nphases-at-Bus1-parse-time render).
#[test]
fn dump_isource_matches_oracle() {
    run_deck_dump_exact("dump_isource");
}

/// `Dump isource.i1 debug` — the CktElement Y-block (all-zero, ideal current
/// source) + `! VARIABLES` (empty, `NumVariables=0`) + generic props.
#[test]
fn dump_isource_debug_matches_oracle() {
    run_deck_dump_exact("dump_isource_debug");
}

// --- WPG.16: the GIC family DumpProperties ---

/// `Dump gicline.gl2` — GICLine's Pascal `DumpProperties` override on the
/// geodesy unit (the generic `~ name=value` property loop; the VE/VN/Z-Matrix
/// block is Complete-only, so absent here).
#[test]
fn dump_gicline_matches_oracle() {
    run_deck_dump_exact("dump_gicline");
}

/// `Dump gicline.gl2 debug` — the Complete-only tail: `BaseFrequency`/`Volts`/
/// `VMag`/`VE`/`VN` (%g) + the base-frequency series `Z Matrix` lower triangle.
#[test]
fn dump_gicline_debug_matches_oracle() {
    run_deck_dump_exact("dump_gicline_debug");
}

/// `Dump gictransformer.tg2` — the YY 4-terminal unit; no Pascal override, so
/// the generic PD-element dump (pins the `%R1`/`%R2` spelling + the promoted
/// BusX/BusNX terminals).
#[test]
fn dump_gictransformer_matches_oracle() {
    run_deck_dump_exact("dump_gictransformer");
}

/// `Dump gictransformer.tg2 debug` — the generic PD Complete form (the
/// CktElement Y-block over the pure-conductance shunt YPrim).
#[test]
fn dump_gictransformer_debug_matches_oracle() {
    run_deck_dump_exact("dump_gictransformer_debug");
}

/// `Dump gicsource.seg` — GICsource has no Pascal override; NON_PCPD like
/// Isource, so it takes the `TPCElement` dump ordering (pins the spliced buses).
#[test]
fn dump_gicsource_matches_oracle() {
    run_deck_dump_exact("dump_gicsource");
}

/// `Dump gicsource.seg debug` — the `TPCElement` Complete form (Y-block +
/// `! VARIABLES` + generic props + two trailing blank lines).
#[test]
fn dump_gicsource_debug_matches_oracle() {
    run_deck_dump_exact("dump_gicsource_debug");
}

// --- WP8.5 step 4: the `Save` forms (Pascal `DoSaveCmd`) ---

/// Replay the `save_forms` deck **fresh** (Pascal `Flg.HasBeenSaved` persists
/// across `save` commands within a session — a second `save load` writes 0
/// records and deletes the file, probe-proven 2026-07-07, so each golden is a
/// first-save capture), issue the meta's full `save …` command, and return
/// `(oracle golden, Rust produced text, engine, scratch dir)`. Unlike `Dump`,
/// the produced file is read by its probe-proven fixed name (`meta.suffix`):
/// the meters branch sets `GlobalResult` to the RELATIVE `MTR_<name>.csv` (not
/// an openable path), `save voltages` sets only `GlobalResult` (never
/// `LastResultFile`), and `save <class>` the final `SaveFile` — each pinned by
/// its own test.
fn produce_deck_save(stem: &str) -> (String, String, Dss, PathBuf) {
    let dir = reports_dir();
    let meta: DeckMeta = {
        let p = dir.join(format!("{stem}.meta.json"));
        let text =
            std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", p.display()))
    };
    let oracle = {
        let p = dir.join(format!("{stem}.txt"));
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
    };

    let scratch = scratch_dir(stem);
    let mut dss = Dss::new();
    dss.command("clear");
    for c in &meta.deck {
        dss.command(c);
    }
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    dss.command(&meta.report); // the full `save …` command
    assert!(dss.errors().is_empty(), "{stem}: {:?}", dss.errors());

    let produced = scratch.join(&meta.suffix);
    let rust = std::fs::read_to_string(&produced)
        .unwrap_or_else(|e| panic!("{stem}: read produced {}: {e}", produced.display()));
    (oracle, rust, dss, scratch)
}

/// `save` (the empty/`meters` default branch): every Monitor `Save` (an
/// in-memory stream flush — NO file, probe-proven; the fixture's mon1 must
/// produce nothing) + every EnergyMeter `SaveRegisters` → `MTR_em1.csv` with
/// the `Year, 0,` header and all 67 `"<RegName>",<value :0:0>` rounded-integer
/// register lines. Byte-exact; `GlobalResult`/`LastResultFile` = the RELATIVE
/// CSV name (`EnergyMeter.pas:1249-1251`, probe-proven).
#[test]
fn save_meters_mtr_matches_oracle_exact() {
    let (oracle, rust, dss, scratch) = produce_deck_save("save_mtr");
    assert_eq!(dss.last_result_file(), "MTR_em1.csv");
    assert_eq!(dss.result(), "MTR_em1.csv");
    // Pascal `TMonitorObj.Save` writes no file: the meter CSV must be the
    // scratch dir's only save product for the bare `save`.
    let mon_files: Vec<_> = std::fs::read_dir(&scratch)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.to_lowercase().contains("mon"))
        .collect();
    assert!(mon_files.is_empty(), "monitor Save wrote {mon_files:?}");
    assert_show_bytes_eq(&oracle, &rust, "save_mtr");
    std::fs::remove_dir_all(&scratch).ok();
}

/// `save voltages` (`Solution.SaveVoltages`, `Solution.pas:2277-2315`):
/// `<case>_SavedVoltages.txt`, per node `<bus>, <nodenum>, <|V| %-.7g>,
/// <angle %-.7g>`; `GlobalResult` = the full path. Exact-equality
/// `compare_export` (`rel = 0`): the 7-sig magnitudes/angles parse
/// value-identical to the oracle capture on this 3-bus deck.
#[test]
fn save_voltages_matches_oracle() {
    let (oracle, rust, dss, scratch) = produce_deck_save("save_voltages");
    assert!(
        dss.result().ends_with("svf_SavedVoltages.txt"),
        "GlobalResult must be the produced path, got {:?}",
        dss.result()
    );
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    compare_export(&oracle, &rust, &policy, "save_voltages");
    std::fs::remove_dir_all(&scratch).ok();
}

/// `save load` (`WriteClassFile`, `Utilities.pas:1134-1210`): a file literally
/// named `load` (the bare class name, NO `.dss` extension — probe-proven), one
/// `New "Load.<name>" <Prop>=<value> …` line per load carrying ONLY the
/// explicitly-set properties in set order (`SaveWrite`). Exact token compare
/// (`compare_export` whitespace tokenization, `rel = 0` — every `Name=value`
/// token must match verbatim). Also pins the `Flg.HasBeenSaved` persistence:
/// a second `save load` in the same session writes 0 records and DELETES the
/// file (`Utilities.pas:1191-1198`, probe-proven 2026-07-07).
#[test]
fn save_class_load_matches_oracle() {
    let (oracle, rust, mut dss, scratch) = produce_deck_save("save_class_load");
    assert!(
        dss.last_result_file().ends_with("load"),
        "GlobalResult must be the final SaveFile, got {:?}",
        dss.last_result_file()
    );
    let policy = ExportPolicy {
        sep: ' ',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    compare_export(&oracle, &rust, &policy, "save_class_load");

    // Second `save load`: every Load is already `HasBeenSaved` → 0 records →
    // the file is deleted and nothing is listed.
    let produced = scratch.join("load");
    assert!(produced.is_file(), "first save must leave the file");
    dss.command("save load");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert!(
        !produced.exists(),
        "second `save load` must delete the 0-record file (HasBeenSaved)"
    );
    std::fs::remove_dir_all(&scratch).ok();
}

/// A **disabled** load saves as `… Enabled=No ENABLED=NO` — BOTH forms: the
/// `Enabled` property is explicitly set (so `SaveWrite` prints it in set order)
/// AND `WriteDSSObject` appends its own ` ENABLED=NO` for any disabled
/// CktElement (`Utilities.pas:1229-1231`). Also pins `PF=0.88`: setting `kW`
/// marks `PF` as set via the load-spec side effects, so the never-typed default
/// PF is serialized too. The pinned string is the oracle's exact bytes
/// (probe 2026-07-07, `save load` over these four loads).
#[test]
fn save_class_disabled_load_writes_enabled_no() {
    let deck = [
        "clear",
        "Set DefaultBaseFrequency=60",
        "new circuit.svfd basekv=12.47 pu=1.0 phases=3 bus1=src",
        "new load.ld1 bus1=b1 phases=3 conn=wye model=1 kv=12.47 kw=400 pf=0.92",
        "new load.ld3 bus1=b2 phases=3 kv=12.47 kw=50 enabled=no",
        "new load.ld4 bus1=b2 phases=3 kv=12.47 kw=60",
        "load.ld4.enabled=no",
    ];
    let mut dss = Dss::new();
    for c in deck {
        dss.command(c);
    }
    let scratch = scratch_dir("save_disabled");
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    dss.command("save load");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let produced = scratch.join("load");
    let rust = std::fs::read_to_string(&produced)
        .unwrap_or_else(|e| panic!("read {}: {e}", produced.display()));
    assert_eq!(
        rust,
        "New \"Load.ld1\" Bus1=b1 Phases=3 Conn=wye Model=1 kV=12.47 kW=400 PF=0.92\n\
         New \"Load.ld3\" Bus1=b2 Phases=3 kV=12.47 kW=50 PF=0.88 Enabled=No ENABLED=NO\n\
         New \"Load.ld4\" Bus1=b2 Phases=3 kV=12.47 kW=60 PF=0.88 Enabled=No ENABLED=NO\n"
    );
    std::fs::remove_dir_all(&scratch).ok();
}

/// The `TODO(compat)` GlobalResult delimiter parity of `do_save_cmd`: Pascal
/// composes `SaveFile := SaveDir + PathDelim + SaveFile` as raw STRINGS
/// (`ExecHelper.pas:835-841`), so with the default `SaveDir = OutputDirectory`
/// (already ending in a delimiter) the observable `GlobalResult` carries a
/// DOUBLED one (`…\\load`), while an explicit `dir=` is the raw parameter
/// (single — `sub1\load`) and the file lands under the mkdir'd subdir. An
/// unknown class silently writes nothing but still runs the tail:
/// `GlobalResult`/`LastResultFile` = the raw `file=` value or empty. All four
/// forms oracle-probed 2026-07-07.
#[test]
fn save_class_global_result_pascal_delimiters() {
    let sep = std::path::MAIN_SEPARATOR;
    let deck = [
        "clear",
        "new circuit.svgr basekv=12.47 pu=1.0 phases=3 bus1=src",
        "new load.ld1 bus1=b1 phases=3 kv=12.47 kw=100 pf=0.92",
    ];

    // Bare form: doubled delimiter in GlobalResult; normalized file on disk.
    let scratch = scratch_dir("save_delims_bare");
    let mut dss = Dss::new();
    for c in deck {
        dss.command(c);
    }
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    dss.command("save load");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(
        dss.result(),
        format!("{}{sep}{sep}load", scratch.display()),
        "bare `save load` must carry the Pascal doubled delimiter"
    );
    assert_eq!(dss.last_result_file(), dss.result());
    assert!(
        scratch.join("load").is_file(),
        "the normalized path must hold the file"
    );

    // Unknown class: silent, clears the result to '' (the Pascal tail runs).
    dss.command("save nosuchclass");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(dss.result(), "");
    assert_eq!(dss.last_result_file(), "");
    // Unknown class + file=: the raw file value; nothing written.
    dss.command("save nosuchclass file=xx.dss");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(dss.result(), "xx.dss");
    assert_eq!(dss.last_result_file(), "xx.dss");
    assert!(
        !scratch.join("xx.dss").exists(),
        "an unknown class must write nothing"
    );
    std::fs::remove_dir_all(&scratch).ok();

    // dir= form: the raw relative dir, single delimiter; the file is created
    // under the subdir (covers the mkDir path). Fresh engine —
    // `Flg.HasBeenSaved` would otherwise 0-record a second save.
    let scratch = scratch_dir("save_delims_dir");
    let mut dss = Dss::new();
    for c in deck {
        dss.command(c);
    }
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    dss.command("save load dir=sub1");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(dss.result(), format!("sub1{sep}load"));
    assert_eq!(dss.last_result_file(), dss.result());
    assert!(
        scratch.join("sub1").join("load").is_file(),
        "the file must land under the mkdir'd subdir"
    );
    std::fs::remove_dir_all(&scratch).ok();
}

/// `save voltages` sets only `GlobalResult` — `LastResultFile` stays UNCHANGED
/// (Pascal `SaveVoltages`'s `finally` sets `GlobalResult`, never
/// `SetLastResultFile`); and a bare `save` on a METER-LESS circuit is a
/// complete no-op — no error, no `MTR_*.csv`, `GlobalResult`/`LastResultFile`
/// both untouched (the meters loop has nothing to write).
#[test]
fn save_voltages_and_meterless_save_leave_last_result_file() {
    let scratch = scratch_dir("save_noop");
    let mut dss = Dss::new();
    for c in [
        "clear",
        "new circuit.svnp basekv=12.47 pu=1.0 phases=3 bus1=src",
        "new load.ld1 bus1=src phases=3 kv=12.47 kw=100 pf=0.92",
        "set voltagebases=[12.47]",
        "calcv",
        "solve",
    ] {
        dss.command(c);
    }
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    // Seed LastResultFile/GlobalResult with a real export.
    dss.command("export voltages");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let seeded = dss.last_result_file().to_string();
    assert!(!seeded.is_empty(), "the export must seed LastResultFile");

    dss.command("save voltages");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert!(
        dss.result().ends_with("svnp_SavedVoltages.txt"),
        "GlobalResult must be the produced path, got {:?}",
        dss.result()
    );
    assert_eq!(
        dss.last_result_file(),
        seeded,
        "save voltages must not touch LastResultFile"
    );

    dss.command("save");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    // GlobalResult is cleared by the dispatcher for EVERY command
    // (`DSS.GlobalResult := ''`); the meter-less save adds nothing to it.
    assert_eq!(
        dss.result(),
        "",
        "a meter-less bare `save` must set no GlobalResult"
    );
    assert_eq!(
        dss.last_result_file(),
        seeded,
        "a meter-less bare `save` must not touch LastResultFile"
    );
    let mtr: Vec<_> = std::fs::read_dir(&scratch)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.to_uppercase().starts_with("MTR_"))
        .collect();
    assert!(mtr.is_empty(), "meter-less save wrote {mtr:?}");
    std::fs::remove_dir_all(&scratch).ok();
}

/// Bare `Dump` on the `dump3.dss` fixture (WP8.5 step 3b) — the whole-circuit
/// form: every CktElement in creation order, then every general DSSObj
/// (defaults + user objects, `DSS.DSSObjs` order), then
/// `Solution.DumpProperties(Leaf=TRUE)`. Byte-exact; pins the property display
/// names of every class in the deck plus the default LoadShape/GrowthShape/
/// Spectrum/TCC_Curve library objects.
#[test]
fn dump3_bare_matches_oracle() {
    run_deck_dump_exact("dump3_bare");
}

/// `Dump debug` — the whole-circuit form with Complete=TRUE: the
/// `Circuit.DebugDump` bus/device/node-map header, per-element Y/terminal/
/// variables blocks, and the Solution options followed by the factored
/// system-Y compressed-column dump (`[%4d,%4d] = %12.5g + j%12.5g`).
#[test]
fn dump3_debug_matches_oracle() {
    run_deck_dump_exact("dump3_debug");
}

/// `Dump solution` — `Solution.DumpProperties(F, Complete=FALSE, Leaf=TRUE)`
/// alone: the full `Set …` option listing including the Leaf-gated lines
/// (Mode/hour/sec/year/circuit/editor/allowduplicates/voltagebases).
#[test]
fn dump3_solution_matches_oracle() {
    run_deck_dump_exact("dump3_solution");
}

/// `Dump buslist` — `BusList.DumpToFile`. `BusList` is a Pascal
/// `TAltHashList`, whose dump is the `LINEAR LISTING...` section alone (the
/// structural reason it has no bucket sections — a different class from the
/// device list's `THashList`).
#[test]
fn dump3_buslist_matches_oracle() {
    run_deck_dump_exact("dump3_buslist");
}

/// `Dump devicelist` — `THashList.DumpToFile`: byte-pins the `MakeHash`
/// rotate-left-5 bucket layout (`Create(900)` → 30 lists), the per-bucket
/// distribution + members, and the linear listing.
#[test]
fn dump3_devicelist_matches_oracle() {
    run_deck_dump_exact("dump3_devicelist");
}

/// `Dump commands` — `DumpAllDSSCommands`: `[execcommands]`/`[execoptions]`
/// over the executive name tables and one `[<Class>]` section per registered
/// class in `DSSClassList` order, each line's help from the gettext catalog
/// (`report/help_catalog.rs`, generated from the pinned wheel). Since
/// WPG.14/15/16 every registered class has its section in the golden
/// (`gen_reports.py::DUMP_COMMANDS_UNPORTED_SECTIONS` is empty); the whole
/// report is byte-exact, pinning command/option/property names and help text.
#[test]
fn dump3_commands_matches_oracle() {
    // `[WindGen]` (WP-U1.8) is a 0.15.x class the pinned 0.14.5 oracle lacks; drop
    // its block before the exact compare (gated live vs capi015 + props instead).
    //
    // The four protection classes (`[Relay]`/`[Recloser]`/`[Fuse]`/`[SwtControl]`,
    // WP-U2.1..U2.5) are on their **r4133** property surfaces, so their blocks in
    // this golden are r4133-shaped and pinned **self-referentially** against our
    // own render — the r4133 help text lives in `help_catalog.rs` (from the
    // `r4133_help.py` supplement, sourced verbatim from the oddie:r4133 engine's
    // own `Dump commands`), and no byte-exact-vs-Delphi help gate exists
    // (UPGRADE_PLAN.md §1.3-2). The property NAMES/VALUES are gated live against
    // oddie:r4133 by the `oracle:"r4133"` controls decks + the `props/*.json`
    // round-trips. WP-U2.3 masked `[Relay]` here pending this regeneration; U2.5
    // unmasks it (the other three never needed masking — Recloser regenerated in
    // U2.2, Fuse/SwtControl kept 0.14.5-shaped names with the new props hidden
    // until U2.5 dropped the HIDE flags for the full r4133 surface).
    run_deck_dump_exact_block_masked("dump3_commands", &["[WindGen]"]);
}

/// `Dump alloc` — `DumpAllocationFactors`: `ConnectedkVA`-spec loads render
/// `AllocationFactor=`, `kwh`-spec render `CFactor=`, every other spec type
/// prints nothing (the deck's kW/PF loads must be absent).
#[test]
fn dump3_alloc_matches_oracle() {
    run_deck_dump_exact("dump3_alloc");
}

/// `? transformer.t1.wdgcurrents` after a solve must equal the oracle's live
/// recompute — pins the `do_query_cmd` Vterminal refresh (audit-code follow-up).
/// Pascal `GetAllWindingCurrents` reloads Vterminal from the solution internally;
/// the Rust `&self` getter reads the cached buffer, so the query path (like Dump/
/// Export) must refresh it first. Before the fix this returned all-zeros. The
/// pinned string is the oracle's exact bytes (same deck/values the byte-exact
/// `dump_transformer` golden already validates against the oracle).
#[test]
fn query_wdgcurrents_refreshes_vterminal() {
    let deck = [
        "clear",
        "new circuit.p basekv=115 bus1=src",
        "new transformer.t1 phases=3 windings=2 xhl=6 buses=[src mid] \
         conns=[delta wye] kvs=[115 4.16] kvas=[5000 5000]",
        "new load.ld1 bus1=mid kv=4.16 kw=2000 pf=0.9 conn=wye",
        "set voltagebases=[115 4.16]",
        "calcv",
        "solve",
    ];
    let mut dss = Dss::new();
    for c in deck {
        dss.command(c);
    }
    dss.command("? transformer.t1.wdgcurrents");
    assert_eq!(
        dss.result(),
        "6.535435, (-57.242), 312.9242, (122.76), 6.535435, (-177.24), \
         312.9242, (2.7582), 6.535435, (62.758), 312.9242, (-117.24), "
    );
}

// --- WP8.6 step 5: the Distribute command ---

/// Meta for a `Distribute` golden: the exact command line, the produced bare
/// filename (`GlobalResult`), and the fixture deck.
#[derive(Debug, Deserialize)]
struct DistribMeta {
    command: String,
    file: String,
    deck: Vec<String>,
}

/// Drive one `Distribute` golden (PHASE8_PLAN §WP8.6 step 5): replay the deck,
/// route output into a scratch dir (`Distribute` writes its script relative to
/// the engine cwd), issue the command, and compare the produced DSS script
/// against the oracle capture with the **numeric-token** `compare_export`
/// policy: `=` is turned into a separator on both sides so each `kW=…` value
/// parses as a number (exact equality — pure f64 arithmetic on identical
/// inputs rendered by the same `%g` rules) while the `generator.DG_%d` /
/// `bus1=` name tokens stay text-compared. Also pins `GlobalResult` +
/// `@lastfile` = the bare produced filename.
fn run_deck_distribute(stem: &str) {
    let dir = reports_dir();
    let meta: DistribMeta = {
        let p = dir.join(format!("{stem}.meta.json"));
        let text =
            std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", p.display()))
    };
    let oracle = {
        let p = dir.join(format!("{stem}.txt"));
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
    };

    let scratch = scratch_dir(stem);
    let mut dss = Dss::new();
    dss.command("clear");
    for c in &meta.deck {
        dss.command(c);
    }
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    dss.command(&meta.command);
    assert!(dss.errors().is_empty(), "{stem}: {:?}", dss.errors());
    // `DSS.GlobalResult := Fname` + the finally's `SetLastResultFile(Fname)` —
    // both the BARE filename, not a full path (`Utilities.pas`
    // makeDistributedGenerators; oracle-probed).
    assert_eq!(dss.result(), meta.file, "{stem}: GlobalResult");
    assert_eq!(dss.last_result_file(), meta.file, "{stem}: @lastfile");
    let produced = scratch.join(&meta.file);
    let rust = std::fs::read_to_string(&produced)
        .unwrap_or_else(|e| panic!("{stem}: read produced {}: {e}", produced.display()));

    // Numeric-token compare: `name=value` cells split into (text, number).
    let policy = ExportPolicy {
        sep: ' ',
        header_lines: 2, // the two `! …` comment lines (the blank line is dropped)
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    compare_export(
        &oracle.replace('=', " "),
        &rust.replace('=', " "),
        &policy,
        stem,
    );

    std::fs::remove_dir_all(&scratch).ok();
}

/// `Distribute kw=1500 pf=0.95` (the default `How=Proportional`): one
/// `generator.DG_%d` per enabled load, `kW·kWBase/ΣkWBase` — the three
/// enabled loads have three different kW-base specs (kW/PF, xfkva·allocation
/// factor, kwh·cfactor), so the weights are all distinct; the disabled load
/// must NOT appear.
#[test]
fn distrib_proportional_matches_oracle() {
    run_deck_distribute("distrib_proportional");
}

/// `Distribute kw=1200 how=Uniform pf=0.9`: `kW / Count` each, where `Count`
/// is the FULL Load element count — the disabled load counts in the divisor
/// (1200/4 = 300, probe-proven `Utilities.pas:1349`) but emits no row.
#[test]
fn distrib_uniform_matches_oracle() {
    run_deck_distribute("distrib_uniform");
}

/// `Distribute kw=900 how=Skip skip=1 pf=0.85` (`WriteEveryOtherGenerators`):
/// every 2nd enabled load — only `DG_2` here — with `kW·kWBase/ΣkWBase` over
/// the selected set (= all 900 kW on the one selected load). The Skip writer's
/// trailing-space `kW=%-g ` cell (`Utilities.pas:1473`) is reproduced in the
/// writer (`exec/distribute.rs::write_every_other`) and present in the golden
/// bytes, but NOT pinned by this whitespace-tokenizing compare (the trailing
/// space collapses during tokenization).
#[test]
fn distrib_skip_matches_oracle() {
    run_deck_distribute("distrib_skip");
}

/// `Distribute kw=750 what=Load file=Explicit.dss`: `what=L…` → `load.DL_%d`
/// rows AND the output is unconditionally renamed to `DistLoads.dss` — the
/// explicit `file=` is overridden (probe-proven, `ExecHelper.pas:3814`).
#[test]
fn distrib_load_matches_oracle() {
    run_deck_distribute("distrib_load");
}

/// `Distribute mw=1.5 pf=0.95`: the `MW=` parameter is `kW := value*1000`
/// (`DoDistributeCmd` ordinal 6), so the output equals the kw=1500
/// proportional variant — kW=1500 distributed over the same weights.
#[test]
fn distrib_mw_matches_oracle() {
    run_deck_distribute("distrib_mw");
}

// --- WP8.6 step 6: Uuids + `Export Uuids` ---

/// The absolute `tools/golden/report_decks` dir — the `@FIXTURES@` token in a
/// golden meta deck resolves to it on both engines (`gen_reports.py` resolves
/// the same way before replaying on the oracle).
fn fixtures_dir() -> PathBuf {
    [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tools",
        "golden",
        "report_decks",
    ]
    .iter()
    .collect()
}

/// `Export Uuids` (Pascal `ExportUuids`) after a `Uuids file=` preload:
/// byte-exact. The fixture CSV preloads a UUID for the circuit, every bus,
/// every ckt element, the library linecode AND the three hashed keys
/// `DefaultCircuitUUIDs` auto-creates (`Station=Station=1` etc.) — any object
/// left out would get a random v4 (`NamedObject.pas` `CreateUUID4`), which can
/// never be oracle-pinned. Also pins the probe-proven quirk: `GlobalResult`
/// stays EMPTY after `export uuids` (unlike every other export; the oracle
/// capture asserted the same on its side).
#[test]
fn export_uuids_matches_oracle() {
    let dir = reports_dir();
    let meta: DeckMeta = {
        let p = dir.join("export_uuids.meta.json");
        let text =
            std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", p.display()))
    };
    assert_eq!(meta.report, "uuids");
    let oracle = {
        let p = dir.join("export_uuids.txt");
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
    };
    // Forward slashes; no canonicalize (its `\\?\` prefix breaks the parser).
    let fixtures = fixtures_dir().to_string_lossy().replace('\\', "/");

    let scratch = scratch_dir("export_uuids");
    let mut dss = Dss::new();
    dss.command("clear");
    for c in &meta.deck {
        dss.command(&c.replace("@FIXTURES@", &fixtures));
    }
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    dss.command("export uuids");
    assert!(dss.errors().is_empty(), "export_uuids: {:?}", dss.errors());
    // The probe-proven quirk: `ExportUuids` never sets `GlobalResult`.
    assert_eq!(
        dss.result(),
        "",
        "export uuids must leave GlobalResult empty"
    );

    let produced = dss.last_result_file();
    let want = format!("{}_{}", meta.fixture, meta.suffix).to_lowercase();
    assert!(
        produced.to_lowercase().ends_with(&want),
        "export_uuids: unexpected produced path {produced:?} (want …{want})"
    );
    let rust = std::fs::read_to_string(produced)
        .unwrap_or_else(|e| panic!("read produced {produced}: {e}"));
    assert_show_bytes_eq(&oracle, &rust, "export_uuids");
    std::fs::remove_dir_all(&scratch).ok();
}

/// `? indmach012.m1.pf` is `""` even AFTER a solve — pins the empirical oracle
/// probe (2026-07-05). Pascal registers `pf` as `[SilentReadOnly, ReadByFunction]`
/// but never assigns its `PropertyOffset` (stays `-1`), so `GetObjPropertyValue`'s
/// outer guard (`DSSObjectHelper.pas` l.2221, `PropertyOffset[Index] <> -1`)
/// short-circuits to `''` before `PowerFactorProperty` ever runs — solved or not.
/// The Rust render must stay `""` on a live solution (SILENT_READ_ONLY), and the
/// query must not refresh anything for this unmarked property.
#[test]
fn query_indmach012_pf_empty_after_solve() {
    let deck = [
        "Set DefaultBaseFrequency=60",
        "New Circuit.indtest basekv=12.47 pu=1.0 phases=3 bus1=src \
         mvasc3=20000 mvasc1=21000",
        "New Transformer.tg phases=3 windings=2 buses=(src, mbus) \
         conns=(delta,wye) kvs=(12.47,0.48) kvas=(1500,1500) xhl=5",
        "New Capacitor.cg conn=wye bus1=mbus phases=3 kvar=600 kv=0.48",
        "New IndMach012.m1 bus1=mbus kV=0.48 kW=1200 conn=delta kVA=1500 H=6 \
         puRs=0.048 puXs=0.075 puRr=0.018 puXr=0.12 puXm=3.8 slip=0.02 \
         SlipOption=variableslip",
        "set voltagebases=[12.47, 0.48]",
        "calcv",
        "solve",
    ];
    let mut dss = Dss::new();
    for c in deck {
        dss.command(c);
    }
    assert!(dss.errors().is_empty());
    dss.command("? indmach012.m1.pf");
    assert_eq!(dss.result(), "");
}
