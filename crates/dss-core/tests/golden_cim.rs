//! Golden gate for the CIM100 XML export (GAPS_PLAN WPG.18): thin driver over
//! `tools/golden/gen_cim.py`'s recipe — replay the same deck (`tools/golden/
//! cim_decks/<circuit>.dss`), preload every UUID via `uuids file=<fixture>`
//! (the same `<circuit>_fixture.csv` the generator produced), issue `export
//! cim100 fil=<tmp>`, and byte-compare the produced file against `tests/
//! golden/cim/<circuit>.xml` — **exact bytes** (CRLF-normalized only), zero
//! tolerance (decision 2: with the fixture preloaded, the oracle and the Rust
//! port must produce bit-identical text, since every UUID and every `%.8g`/
//! `%d`/enum rendering is now fully determined).
//!
//! Add a Stage B-F case by dropping `<name>.dss` + `<name>_fixture.csv` in
//! `tools/golden/cim_decks/`, `<name>.xml` in `tests/golden/cim/` (regenerate
//! both via `python tools/golden/gen_cim.py`), and a `run_case("<name>")` call
//! below.

use std::path::{Path, PathBuf};

use dss_core::exec::Dss;

fn repo_root() -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "..", ".."].iter().collect()
}

fn decks_dir() -> PathBuf {
    repo_root().join("tools").join("golden").join("cim_decks")
}

fn golden_dir() -> PathBuf {
    repo_root().join("tests").join("golden").join("cim")
}

/// Byte-exact compare (CRLF-normalized only — decision 2's zero-tolerance
/// gate), with a per-line diff on mismatch so a divergence points straight at
/// the offending XML element instead of a single opaque `assert_eq!`.
fn assert_cim_bytes_eq(oracle: &str, rust: &str, ctx: &str) {
    let o = oracle.replace("\r\n", "\n");
    let r = rust.replace("\r\n", "\n");
    if o == r {
        return;
    }
    let ol: Vec<&str> = o.split('\n').collect();
    let rl: Vec<&str> = r.split('\n').collect();
    for (i, (a, b)) in ol.iter().zip(rl.iter()).enumerate() {
        assert_eq!(
            a,
            b,
            "{ctx}: line {} differs\n  oracle: {a:?}\n  rust:   {b:?}",
            i + 1
        );
    }
    assert_eq!(
        ol.len(),
        rl.len(),
        "{ctx}: line count differs (oracle {}, rust {})",
        ol.len(),
        rl.len()
    );
}

/// Locate the one file matching `<circuit>_CIM100x.xml` in `scratch`.
fn locate_cim100(scratch: &Path, circuit: &str) -> String {
    let want = format!("{circuit}_CIM100x.xml");
    let matches: Vec<PathBuf> = std::fs::read_dir(scratch)
        .unwrap_or_else(|e| panic!("read_dir {}: {e}", scratch.display()))
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.file_name().map(|n| n == want.as_str()).unwrap_or(false))
        .collect();
    assert_eq!(
        matches.len(),
        1,
        "{circuit}: expected exactly one {want} in {}, found {matches:?}",
        scratch.display()
    );
    std::fs::read_to_string(&matches[0])
        .unwrap_or_else(|e| panic!("read {}: {e}", matches[0].display()))
}

/// Replay `<circuit>.dss` + `<circuit>_fixture.csv`, run `Export CIM100`, and
/// byte-compare against `tests/golden/cim/<circuit>.xml`.
fn run_case(circuit: &str) {
    let deck = decks_dir().join(format!("{circuit}.dss"));
    let fixture = decks_dir().join(format!("{circuit}_fixture.csv"));
    assert!(deck.is_file(), "missing deck: {}", deck.display());
    assert!(fixture.is_file(), "missing fixture: {}", fixture.display());
    let oracle_path = golden_dir().join(format!("{circuit}.xml"));
    let oracle = std::fs::read_to_string(&oracle_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", oracle_path.display()));

    let scratch = std::env::temp_dir().join(format!("dss_golden_cim_{circuit}"));
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).unwrap();

    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!(
        "compile \"{}\"",
        deck.to_string_lossy().replace('\\', "/")
    ));
    dss.command(&format!(
        "set datapath=\"{}\"",
        scratch.to_string_lossy().replace('\\', "/")
    ));
    dss.command(&format!(
        "uuids file=\"{}\"",
        fixture.to_string_lossy().replace('\\', "/")
    ));
    dss.command("export cim100");
    assert!(
        dss.errors().is_empty(),
        "{circuit}: unexpected errors: {:?}",
        dss.errors()
    );

    let rust = locate_cim100(&scratch, circuit);
    assert_cim_bytes_eq(&oracle, &rust, circuit);
    std::fs::remove_dir_all(&scratch).ok();
}

/// Stage A: writer core + skeleton + EnergySource (Vsource + buscoords only).
#[test]
fn cim_src() {
    run_case("cim_src");
}

/// Stage B: EnergyConsumer sweep + AttachLoadPhases/AttachSecondaryPhases +
/// EnergyConnectionProfile (wye/delta/secondary loads + a daily-shape ECP).
#[test]
fn cim_load() {
    run_case("cim_load");
}

/// Stage C: ACLineSegment/LoadBreakSwitch sweep (coded sym + coded matrix +
/// sym-inline + matrix-inline PUZ + geometry + spacing + CN/TS cable lines +
/// a Fuse switch), AttachLinePhases/AttachSwitchPhases, and the LineCode /
/// WireData / TSData / CNData / LineGeometry / LineSpacing catalog.
#[test]
fn cim_lines() {
    run_case("cim_lines");
}

/// Stage D: LinearShuntCompensator sweep (wye + delta + 1-phase caps, the
/// `AttachCapPhases` per-phase breakdown, the SSH `sections`/`aVRDelay`),
/// CapControl → RegulatingControl (a voltage-mode + a current-mode control,
/// `MonitoredPhaseNode`/`RegulatingControlEnum`/target value/deadband), and the
/// series-reactor → SeriesCompensator sweep.
#[test]
fn cim_shunt() {
    run_case("cim_shunt");
}
