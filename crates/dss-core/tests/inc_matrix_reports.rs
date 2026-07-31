//! Incidence-matrix report goldens (WP-AD.1, DIAKOPTICS_PSTCALC_PLAN §2): pin the
//! CSV files the oracle writes for `CalcIncMatrix`/`CalcIncMatrix_O`/
//! `CalcLaplacian` + exports 53–57. `tools/golden/gen_inc_matrix.py` runs each
//! fixture on the pinned engine, issues the calc + export commands, and captures
//! the produced file bytes into `tests/golden/inc_matrix/<stem>.txt` (+ a
//! `<stem>.meta.json` with the exact deck/master + command sequence, so the Rust
//! and oracle fixtures can never drift).
//!
//! The exports are integer/text CSV (no floats), so this driver compares
//! **byte-exact**, line-for-line (CRLF→LF normalized) — a stronger gate than the
//! token diff the float reports need. A wrong traversal order, a dropped/extra
//! row, a mis-normalized level, or a filename/header typo all fail here.
//!
//! Stage F (`DE_PASCALIZE_PLAN.md` Part IV.2): that byte compare stays in
//! **both** lanes. These writers render no number through the F-FMT seam
//! (`report::format`'s float helpers / `fmt_g`), so F.4 cannot move their bytes
//! and the stronger gate costs the default lane nothing — the scoping rule is
//! documented in `harness::lane`.
//!
//! Regenerate only manually: `python tools/golden/gen_inc_matrix.py`.

use std::path::PathBuf;

use dss_core::exec::Dss;
use serde::Deserialize;

fn golden_dir() -> PathBuf {
    [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "golden",
        "inc_matrix",
    ]
    .iter()
    .collect()
}

fn corpus_dir() -> PathBuf {
    [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "corpus",
        "electricdss-tst",
    ]
    .iter()
    .collect()
}

#[derive(Debug, Deserialize)]
struct IncMeta {
    master: Option<String>,
    deck: Option<Vec<String>>,
    post: Vec<String>,
    calc: Vec<String>,
    report: String,
    #[allow(dead_code)]
    fixture: String,
}

fn scratch_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("dss_incm_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap_or_else(|e| panic!("mkdir {}: {e}", d.display()));
    d
}

/// Replay one golden stem: build the fixture (compile master or replay the inline
/// deck), run the post + calc commands, export the report, and byte-compare the
/// produced file against the captured oracle bytes.
fn run(stem: &str) {
    let dir = golden_dir();
    let meta: IncMeta = {
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
    match (&meta.master, &meta.deck) {
        (Some(master), _) => {
            let path = corpus_dir().join(master);
            assert!(path.is_file(), "{stem}: master missing: {}", path.display());
            dss.command(&format!(
                "compile \"{}\"",
                path.to_string_lossy().replace('\\', "/")
            ));
        }
        (None, Some(deck)) => {
            for c in deck {
                dss.command(c);
            }
        }
        (None, None) => panic!("{stem}: meta has neither master nor deck"),
    }
    for c in &meta.post {
        dss.command(c);
    }
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    for c in &meta.calc {
        dss.command(c);
    }
    dss.command(&format!("export {}", meta.report));
    assert!(dss.errors().is_empty(), "{stem}: errors {:?}", dss.errors());

    let produced = dss.last_result_file();
    // Pin the oracle default filename per export keyword (Pascal
    // `ExportOptions.pas:417-425`), mirroring the `golden_reports.rs` `ends_with`
    // convention — a swapped/wrong default basename (the `Result.txt`→`.csv`
    // regression class) would otherwise pass since content is keyed by the export
    // function, not the filename.
    let want = expected_basename(&meta.report);
    assert!(
        produced.to_lowercase().ends_with(want),
        "{stem}: unexpected produced path {produced:?} (want …{want} for report {})",
        meta.report
    );
    let rust = std::fs::read_to_string(produced)
        .unwrap_or_else(|e| panic!("{stem}: read produced {produced}: {e}"));

    assert_bytes_eq(&oracle, &rust, stem);
    std::fs::remove_dir_all(&scratch).ok();
}

/// The oracle default output filename for each incidence-matrix export keyword
/// (Pascal `ExportOptions.pas:417-425`, verbatim). Lowercased for the
/// case-insensitive `ends_with` check.
fn expected_basename(report: &str) -> &'static str {
    match report.to_lowercase().as_str() {
        "incmatrix" => "inc_matrix.csv",
        "incmatrixrows" => "inc_matrix_rows.csv",
        "incmatrixcols" => "inc_matrix_cols.csv",
        "buslevels" => "bus_levels.csv",
        "laplacian" => "laplacian.csv",
        other => panic!("unknown incidence-matrix report keyword: {other:?}"),
    }
}

/// Byte-exact line comparison (CRLF→LF normalized), with a per-line failure
/// message pointing at the first divergence.
fn assert_bytes_eq(oracle: &str, rust: &str, stem: &str) {
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

macro_rules! fixture_tests {
    ($name:ident, $prefix:literal) => {
        #[test]
        fn $name() {
            for suffix in [
                "flat_incmatrix",
                "flat_rows",
                "org_incmatrix",
                "org_rows",
                "org_cols",
                "org_levels",
                "org_laplacian",
            ] {
                run(&format!("{}_{}", $prefix, suffix));
            }
        }
    };
}

fixture_tests!(inc_matrix_ieee13, "ieee13");
fixture_tests!(inc_matrix_ieee123, "ieee123");
fixture_tests!(inc_matrix_sercap, "sercap");
fixture_tests!(inc_matrix_serreac, "serreac");

/// `CalcLaplacian` before any `CalcIncMatrix`/`CalcIncMatrix_O` raises the
/// upstream NIL guard (error 8877) — the message text is verbatim, including the
/// upstream "Indidence" typo (probe-confirmed against the pinned oracle).
#[test]
fn calc_laplacian_without_inc_matrix_errors_8877() {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.x basekv=12.47 bus1=sourcebus");
    dss.command("new line.l1 bus1=sourcebus bus2=a");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "setup errors: {:?}", dss.errors());
    dss.command("CalcLaplacian");
    let errs = dss.error_texts();
    assert_eq!(
        errs,
        [
            "Indidence matrix is not present. Please run either \"CalcIncMatrix\" or \"CalcIncMatrix_O\" first."
                .to_string()
        ],
        "unexpected errors: {errs:?}"
    );
}
