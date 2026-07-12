//! Corpus manifest coverage gate (CORPUS_TEST_PLAN.md §2) — the "no silent
//! omissions" guarantee. Oracle-free and always-on (runs in the normal
//! `cargo test --workspace`).
//!
//! Asserts a **bijection** between the `.dss` files under
//! `tests/corpus/electricdss-tst/` and the entries across the manifests in
//! `tests/corpus/manifests/`: every `.dss` is listed in **exactly one**
//! manifest, and every manifested path exists on disk. So the corpus and the
//! manifests can never silently drift — adding or removing a `.dss` fails this
//! test until it is classified.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Manifest {
    #[serde(default)]
    cases: Vec<Case>,
}

#[derive(Debug, Deserialize)]
struct Case {
    path: String,
}

fn corpus_root() -> PathBuf {
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

fn manifests_dir() -> PathBuf {
    [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "corpus",
        "manifests",
    ]
    .iter()
    .collect()
}

/// Recursively collect every `.dss` file under `dir`, as forward-slashed paths
/// relative to `base`.
fn collect_dss(dir: &Path, base: &Path, out: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display())) {
        let p = entry.expect("dir entry").path();
        if p.is_dir() {
            collect_dss(&p, base, out);
        } else if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("dss")) {
            let rel = p
                .strip_prefix(base)
                .expect("under base")
                .to_string_lossy()
                .replace('\\', "/");
            out.push(rel);
        }
    }
}

#[test]
fn every_dss_is_accounted_for_exactly_once() {
    let root = corpus_root();
    assert!(
        root.is_dir(),
        "vendored corpus missing: {} (run tools/corpus/vendor.py)",
        root.display()
    );

    // Every manifest entry, mapped path -> manifest file; duplicates collected.
    let mdir = manifests_dir();
    let mut files: Vec<PathBuf> = std::fs::read_dir(&mdir)
        .unwrap_or_else(|e| panic!("read {}: {e}", mdir.display()))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        // `ad_sweep.json` (WP-AD.4) is an orthogonal A-Diakoptics disposition
        // OVERLAY on the `solvable_now.json` entry points, not an ownership
        // manifest — its paths are deliberately a subset already owned elsewhere,
        // so it must not participate in the exactly-once ownership bijection. Its
        // own coverage (bijective with solvable_now) is checked by
        // `ad_sweep_covers_solvable_now` in `corpus_live.rs`.
        .filter(|p| p.file_name().is_none_or(|n| n != "ad_sweep.json"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no manifests in {}", mdir.display());

    let mut owner: BTreeMap<String, String> = BTreeMap::new();
    let mut dups: Vec<String> = Vec::new();
    for p in &files {
        let fname = p.file_name().unwrap().to_string_lossy().to_string();
        let text =
            std::fs::read_to_string(p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
        let m: Manifest =
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", p.display()));
        for c in m.cases {
            let norm = c.path.replace('\\', "/");
            if let Some(prev) = owner.insert(norm.clone(), fname.clone()) {
                dups.push(format!("{norm}  (in {prev} and {fname})"));
            }
        }
    }
    assert!(
        dups.is_empty(),
        "{} path(s) listed in more than one manifest:\n  {}",
        dups.len(),
        dups.join("\n  ")
    );

    // On-disk .dss set.
    let mut disk: Vec<String> = Vec::new();
    collect_dss(&root, &root, &mut disk);
    let disk_set: std::collections::BTreeSet<String> = disk.into_iter().collect();
    let manifest_set: std::collections::BTreeSet<String> = owner.keys().cloned().collect();

    // Manifested but absent on disk.
    let ghosts: Vec<&String> = manifest_set.difference(&disk_set).collect();
    assert!(
        ghosts.is_empty(),
        "{} manifested path(s) do not exist under {}:\n  {}",
        ghosts.len(),
        root.display(),
        ghosts
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\n  ")
    );

    // On disk but not in any manifest — the silent-omission failure.
    let unaccounted: Vec<&String> = disk_set.difference(&manifest_set).collect();
    assert!(
        unaccounted.is_empty(),
        "{} .dss file(s) are not in any manifest (add them to tests/corpus/manifests/):\n  {}",
        unaccounted.len(),
        unaccounted
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\n  ")
    );

    assert_eq!(
        disk_set.len(),
        manifest_set.len(),
        "bijection size mismatch (disk {} vs manifests {})",
        disk_set.len(),
        manifest_set.len()
    );
    eprintln!(
        "corpus manifest coverage: {} .dss accounted for across {} manifests",
        disk_set.len(),
        files.len()
    );
}
