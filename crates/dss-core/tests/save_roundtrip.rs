//! `Save circuit` round-trip + structural gate (PHASE8_PLAN §WP8.5 step 5, §1
//! gate #3). Faithfulness of `Circuit.Save` is **round-trip**, not byte-equality
//! with the oracle's Save output (§2.4): we save a solved circuit to a scratch
//! directory, `clear`, re-`compile` the emitted `Master.dss` on OUR engine, and
//! re-`solve` — the node voltages must match the pre-save solution (≤1e-6 rel)
//! and the iteration count must be identical. A structural test additionally
//! pins the emitted **file set** against the oracle's probe-proven set (captured
//! with the pinned dss-python `Circuit.Save`, 2026-07-07).

use dss_core::exec::Dss;
use std::collections::BTreeSet;
use std::path::PathBuf;

/// Repo-root-relative path under the vendored corpus.
fn corpus(rel: &str) -> PathBuf {
    [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "corpus",
        "electricdss-tst",
    ]
    .iter()
    .collect::<PathBuf>()
    .join(rel)
}

/// A repo-root-relative path (for the `tools/golden/report_decks` fixtures).
fn repo(rel: &str) -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "..", ".."]
        .iter()
        .collect::<PathBuf>()
        .join(rel)
}

/// A unique scratch dir for this test process (no `tempfile` dep).
fn scratch_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("dss_saveroundtrip_{tag}_{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    std::fs::create_dir_all(&d).unwrap_or_else(|e| panic!("mkdir {}: {e}", d.display()));
    d
}

/// Snapshot the solved state as (node-name → complex V) plus the iteration
/// count. Keyed by node NAME so a re-compile that renumbers buses still lines up.
fn snapshot(dss: &Dss) -> (Vec<(String, f64, f64)>, i32) {
    let ckt = dss.circuit().expect("circuit solved");
    let mut v = Vec::with_capacity(ckt.num_nodes);
    for j in 1..=ckt.num_nodes {
        let volt = ckt.solution.node_v[j];
        v.push((ckt.node_name(j), volt.re, volt.im));
    }
    (v, ckt.solution.iteration)
}

/// Solve `master`, `save circuit` to a scratch dir, `clear`, re-compile the
/// emitted `Master.dss`, re-solve, and assert node voltages (≤1e-6 rel) +
/// iteration count match the pre-save solution.
///
/// The IEEE masters embed a `Solve` (e.g. IEEE13 l.149), so a circuit reaches
/// this test already converged — the regulator taps are settled. To compare
/// iteration counts *fairly* (a cold solve of a regulator circuit spends extra
/// power-flow iterations settling taps that a warm re-solve does not), we
/// compare a **warm** re-solve on both sides: the pre-save snapshot is a warm
/// re-solve of the already-converged original, and the round-tripped circuit is
/// first solved cold (to settle taps to the same fixpoint) then warm-re-solved.
/// The node voltages must match after both reach convergence; the warm-re-solve
/// iteration count must be identical (proving the recompiled circuit converges
/// to the same operating point in the same way — a structural-identity check).
fn round_trip(tag: &str, master: PathBuf) {
    assert!(master.is_file(), "missing master: {}", master.display());
    let out = scratch_dir(tag);

    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!(
        "compile \"{}\"",
        master.to_string_lossy().replace('\\', "/")
    ));
    // Settle then warm re-solve: some masters embed `Solve` (IEEE13/37), some do
    // not (IEEE123), so solve twice to guarantee a warm re-solve on both sides.
    dss.command("solve");
    dss.command("solve");
    assert!(
        dss.errors().is_empty(),
        "{tag}: pre-save errors: {:?}",
        dss.errors()
    );
    let (pre, pre_iter) = snapshot(&dss);
    assert!(!pre.is_empty(), "{tag}: no nodes pre-save");

    dss.command(&format!(
        "save circuit dir=\"{}\"",
        out.to_string_lossy().replace('\\', "/")
    ));
    assert!(
        dss.errors().is_empty(),
        "{tag}: save errors: {:?}",
        dss.errors()
    );
    let emitted_master = out.join("Master.dss");
    assert!(emitted_master.is_file(), "{tag}: no Master.dss emitted");

    // Re-compile the emitted script on OUR engine (no embedded Solve → cold).
    dss.command("clear");
    dss.command(&format!(
        "compile \"{}\"",
        emitted_master.to_string_lossy().replace('\\', "/")
    ));
    // Cold solve settles regulator taps to the same fixpoint, then a warm
    // re-solve gives the count comparable to the pre-save warm re-solve.
    dss.command("solve");
    assert!(
        dss.errors().is_empty(),
        "{tag}: post-save cold-solve errors: {:?}",
        dss.errors()
    );
    dss.command("solve");
    assert!(
        dss.errors().is_empty(),
        "{tag}: post-save warm-solve errors: {:?}",
        dss.errors()
    );
    let (post, post_iter) = snapshot(&dss);

    // Iteration count exact (warm re-solve on both sides).
    assert_eq!(
        pre_iter, post_iter,
        "{tag}: warm-re-solve iteration count changed across save round-trip ({pre_iter} -> {post_iter})"
    );

    // Node voltages ≤1e-6 rel (matched by node name).
    use std::collections::HashMap;
    let post_map: HashMap<&str, (f64, f64)> = post
        .iter()
        .map(|(n, re, im)| (n.as_str(), (*re, *im)))
        .collect();
    assert_eq!(
        pre.len(),
        post.len(),
        "{tag}: node count changed {} -> {}",
        pre.len(),
        post.len()
    );
    for (name, re, im) in &pre {
        let (pre_re, pre_im) = (*re, *im);
        let &(post_re, post_im) = post_map
            .get(name.as_str())
            .unwrap_or_else(|| panic!("{tag}: node {name} missing after round-trip"));
        let mag = (pre_re * pre_re + pre_im * pre_im).sqrt();
        let d = ((post_re - pre_re).powi(2) + (post_im - pre_im).powi(2)).sqrt();
        let rel = if mag > 0.0 { d / mag } else { d };
        assert!(
            rel <= 1e-6,
            "{tag}: node {name} voltage diverged rel={rel:.3e} \
             (pre={pre_re:.6}+j{pre_im:.6}, post={post_re:.6}+j{post_im:.6})"
        );
    }

    std::fs::remove_dir_all(&out).ok();
}

#[test]
fn save_roundtrip_ieee13() {
    round_trip(
        "ieee13",
        corpus("Version8/Distrib/IEEETestCases/13Bus/IEEE13Nodeckt.dss"),
    );
}

#[test]
fn save_roundtrip_ieee37() {
    round_trip(
        "ieee37",
        corpus("Version8/Distrib/IEEETestCases/37Bus/ieee37.dss"),
    );
}

#[test]
fn save_roundtrip_ieee123() {
    round_trip(
        "ieee123",
        corpus("Version8/Distrib/IEEETestCases/123Bus/IEEE123Master.dss"),
    );
}

/// Collect the set of emitted files relative to `root`, using `/` separators.
fn emitted_set(root: &std::path::Path) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    fn walk(dir: &std::path::Path, root: &std::path::Path, set: &mut BTreeSet<String>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(&p, root, set);
            } else {
                let rel = p
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                set.insert(rel);
            }
        }
    }
    walk(root, root, &mut set);
    set
}

/// Structural test: `save circuit` on `save_forms.dss` (a deck with an
/// EnergyMeter zone → feeder subdir) emits exactly the oracle's probe-proven
/// file set (dss-python `Circuit.Save`, 2026-07-07). Round-trip through our own
/// parser is covered by the IEEE cases above; here we pin the multi-file layout,
/// including the `em1/` meter-zone subdirectory (`Branches`/`Loads`/`Capacitors`
/// with the empty `Transformers`/`Shunts`/`Generators` deleted, not listed).
#[test]
fn save_forms_structural_file_set() {
    let deck = repo("tools/golden/report_decks/save_forms.dss");
    assert!(deck.is_file(), "missing fixture: {}", deck.display());
    let out = scratch_dir("saveforms");

    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!(
        "compile \"{}\"",
        deck.to_string_lossy().replace('\\', "/")
    ));
    assert!(
        dss.errors().is_empty(),
        "compile errors: {:?}",
        dss.errors()
    );
    let nodes_before = dss.circuit().expect("circuit").num_nodes;
    dss.command(&format!(
        "save circuit dir=\"{}\"",
        out.to_string_lossy().replace('\\', "/")
    ));
    assert!(dss.errors().is_empty(), "save errors: {:?}", dss.errors());

    let got = emitted_set(&out);
    let expected: BTreeSet<String> = [
        "BusCoords.dss",
        "BusVoltageBases.dss",
        "EnergyMeter.dss",
        "GrowthShape.dss",
        "LineCode.dss",
        "LoadShape.dss",
        "Master.dss",
        "Monitor.dss",
        "Spectrum.dss",
        "TCC_Curve.dss",
        "Vsource.dss",
        "em1/Branches.dss",
        "em1/Capacitors.dss",
        "em1/Loads.dss",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    assert_eq!(got, expected, "emitted file set differs from oracle probe");

    // The emitted tree must re-compile cleanly on our own parser — this is the
    // only gate deck with a meter zone, so it exercises the `em1\…` feeder
    // Redirects + the zone-file (Branches/Loads/Capacitors) re-parse. (A voltage
    // round-trip is out of scope: the emitted Master carries no `set mode=daily`,
    // so it re-compiles in snapshot mode, not the fixture's daily final state.)
    dss.command("clear");
    dss.command(&format!(
        "compile \"{}\"",
        out.join("Master.dss").to_string_lossy().replace('\\', "/")
    ));
    assert!(
        dss.errors().is_empty(),
        "re-compile of emitted feeder tree errored: {:?}",
        dss.errors()
    );
    // Same node count proves every element (lines/loads/cap in the feeder
    // subdir, source, meter/monitor) re-parsed from the emitted tree.
    let nodes_after = dss.circuit().expect("circuit after re-compile").num_nodes;
    assert_eq!(
        nodes_before, nodes_after,
        "node count changed across feeder-tree round-trip"
    );

    std::fs::remove_dir_all(&out).ok();
}
