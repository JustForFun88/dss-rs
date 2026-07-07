//! `Distribute` (WP8.6 step 5) and `Uuids` (step 6) behavior tests — the
//! error paths and RNG/persistence semantics the byte-exact goldens
//! (`golden_reports.rs::distrib_*`/`export_uuids`) cannot pin.

use crate::exec::*;

fn scratch(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("dss_exec_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&d).expect("mkdir scratch");
    d
}

fn distrib_fixture() -> Dss {
    let mut dss = Dss::new();
    for c in [
        "new circuit.dst basekv=12.47 pu=1.0 phases=3 bus1=src",
        "new line.l1 bus1=src bus2=b1 length=1 units=km r1=0.3 x1=0.7 r0=0.9 x0=2.0",
        "new load.ld1 bus1=b1 phases=3 kv=12.47 kw=500 pf=0.95",
        "new load.ld2 bus1=b1.1 phases=1 kv=7.2 kw=200 pf=0.9",
        "set voltagebases=[12.47]",
        "calcvoltagebases",
        "solve",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss
}

/// Pascal error 721: `Distribute` refuses to overwrite an existing file (the
/// second run in the same directory) — and neither `GlobalResult` nor
/// `@lastfile` is updated on the refusal.
#[test]
fn distribute_refuses_to_overwrite() {
    let dir = scratch("distrib_721");
    let mut dss = distrib_fixture();
    dss.command(&format!("set datapath=\"{}\"", dir.display()));
    dss.command("distribute kw=100");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(dss.result(), "DistGenerators.dss");
    dss.command("distribute kw=100");
    assert_eq!(
        dss.errors(),
        &["File \"DistGenerators.dss\" was about to be overwritten. \
             Rename/remove the existing file and try again."
            .to_string()]
    );
    assert_eq!(dss.result(), "", "refusal must not set GlobalResult");
    std::fs::remove_dir_all(&dir).ok();
}

/// `How=Random` is RNG-carried upstream (FPC `randomize` + `random`,
/// `Utilities.pas:1400` — time-seeded, probe-proven class), so it is NEVER
/// golden-gated (the GAPS_PLAN RNG rule); this smoke test pins the
/// deterministic frame around the random values: header, row count/shape, and
/// every `kW=` value within `[0, 2·kW/count)`.
#[test]
fn distribute_random_shape_and_bounds() {
    let dir = scratch("distrib_rand");
    let mut dss = distrib_fixture();
    dss.command(&format!("set datapath=\"{}\"", dir.display()));
    dss.command("distribute kw=700 how=Random pf=0.9");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let text = std::fs::read_to_string(dir.join("DistGenerators.dss")).expect("produced file");
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(
        lines[1],
        "! Distribute kW=700 PF=0.9 How=Random Skip=1  file=DistGenerators.dss  what=Generators"
    );
    let rows: Vec<&str> = lines[3..].to_vec();
    assert_eq!(rows.len(), 2, "one row per enabled load: {text}");
    for (i, row) in rows.iter().enumerate() {
        assert!(row.starts_with(&format!("new generator.DG_{}  bus1=", i + 1)));
        let kw: f64 = row
            .split_whitespace()
            .find_map(|t| t.strip_prefix("kW="))
            .expect("kW= token")
            .parse()
            .expect("numeric kW");
        // kWeach = 700/2 = 350; value = kWeach * random * 2 ∈ [0, 700).
        assert!((0.0..700.0).contains(&kw), "kW out of range: {row}");
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// Pascal error 242: `Uuids file=<missing>` — and the command still RESETS the
/// hashed list first (`StartUuidList` runs before the file check,
/// `ExecHelper.pas:4472`).
#[test]
fn uuids_missing_file_errors_242() {
    let mut dss = distrib_fixture();
    dss.command("uuids file=no_such_uuids.csv");
    assert_eq!(
        dss.errors(),
        &["UUIDs file: no_such_uuids.csv does not exist".to_string()]
    );
}

/// The hashed-key list persists across commands, `Uuids` resets it, and
/// `Export Uuids` frees it: after `uuids file=` preloads `Station=Station=1`,
/// the FIRST `export uuids` reports the preloaded value, and a SECOND one
/// (list freed by the first's `finally`) re-creates the key as a random v4 —
/// different bytes (probe-proven upstream, 2026-07-07). Object UUIDs
/// (circuit/bus/element) survive both exports.
#[test]
fn uuids_hashed_list_lifecycle() {
    let dir = scratch("uuids_cycle");
    let csv = dir.join("pre.csv");
    std::fs::write(
        &csv,
        "circuit.dst, {00000000-0000-4000-8000-0000000000A0}\n\
         Station=Station=1, {00000000-0000-4000-8000-0000000000D1}\n",
    )
    .expect("write csv");
    let mut dss = distrib_fixture();
    dss.command(&format!("set datapath=\"{}\"", dir.display()));
    dss.command(&format!(
        "uuids file={}",
        csv.to_string_lossy().replace('\\', "/")
    ));
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    dss.command("export uuids first.csv");
    let first = std::fs::read_to_string(dir.join("first.csv")).expect("first export");
    assert!(
        first.contains("Station=Station=1 {00000000-0000-4000-8000-0000000000D1}"),
        "preloaded hashed key must survive to the first export:\n{first}"
    );
    assert!(first.starts_with("Circuit.dst {00000000-0000-4000-8000-0000000000A0}"));

    dss.command("export uuids second.csv");
    let second = std::fs::read_to_string(dir.join("second.csv")).expect("second export");
    // The first export freed the list; the key is re-created random.
    assert!(
        !second.contains("{00000000-0000-4000-8000-0000000000D1}"),
        "hashed key must be re-created after FreeUuidList:\n{second}"
    );
    assert!(second.contains("Station=Station=1 {"));
    // Object UUIDs are NOT freed — the circuit row is stable across exports.
    assert!(second.starts_with("Circuit.dst {00000000-0000-4000-8000-0000000000A0}"));
    std::fs::remove_dir_all(&dir).ok();
}
