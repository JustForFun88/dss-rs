//! Anti-shrink guard for the gated corpus population (FA fix 3).
//!
//! The mandatory gate defines its own population: `solvable_now.json` lists the
//! decks `corpus_live.rs` compiles + live-compares against the oracle, and the
//! other manifests bucket the rest. Because that classification is *self-defined*,
//! a port regression could be silently "neutralized" by moving a deck out of
//! `solvable_now` into a skip bucket — `cargo test` stays green while real
//! coverage shrinks, invisibly, in a one-line manifest edit.
//!
//! This test makes any such change a **loud, reviewed event**. A committed
//! snapshot (`tests/corpus/manifests/population.lock.json`) records, per manifest
//! class, the case count; the full sorted `solvable_now` path list; and the three
//! synthetic family-manifest case counts. The unconditional test below rebuilds
//! that fingerprint from the current manifests and asserts it equals the snapshot.
//! Any drift — a path leaving `solvable_now`, any count change — fails with a diff
//! and the one-command regeneration path, so the shrink lands as a reviewable diff
//! in the lock file rather than passing unnoticed.
//!
//! **Regenerate deliberately** (never to silence a failure you have not reviewed):
//!
//! ```text
//! DSS_UPDATE_POPULATION_LOCK=1 cargo test -p dss-core --test population_lock
//! ```
//!
//! writes the lock from the current manifests and passes. Commit the resulting
//! `population.lock.json` diff together with the manifest change that caused it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Name of the committed snapshot file (lives in the manifests dir; skipped by
/// both this test's manifest scan and `corpus_manifest.rs` — it carries no
/// `cases`).
const LOCK_FILE: &str = "population.lock.json";

/// The three synthetic family manifests (`tests/corpus/<name>/manifest.json`).
const FAMILIES: [&str; 3] = ["asymmetric", "controls", "modes"];

#[derive(Debug, Deserialize)]
struct Manifest {
    #[serde(default)]
    cases: Vec<Case>,
}

#[derive(Debug, Deserialize)]
struct Case {
    #[serde(default)]
    path: String,
}

/// The committed population fingerprint. Field order + `BTreeMap`/sorted `Vec`
/// give a deterministic, review-friendly serialization.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
struct PopulationLock {
    /// Human note; not compared (see `PartialEq` below — we compare the data
    /// fields explicitly, not via derive, so an edited comment never fails).
    #[serde(default)]
    comment: String,
    /// `<manifest filename> -> case count` for every classifier manifest in
    /// `tests/corpus/manifests/` (excluding this lock file).
    manifest_counts: BTreeMap<String, usize>,
    /// `<family name> -> case count` for the three synthetic families.
    family_counts: BTreeMap<String, usize>,
    /// Every `solvable_now.json` case path, `/`-normalized and sorted.
    solvable_now_paths: Vec<String>,
}

// Compare only the data fields; the free-text `comment` is documentation.
impl PopulationLock {
    fn data_eq(&self, other: &Self) -> bool {
        self.manifest_counts == other.manifest_counts
            && self.family_counts == other.family_counts
            && self.solvable_now_paths == other.solvable_now_paths
    }
}

fn corpus_dir() -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "..", "..", "tests", "corpus"]
        .iter()
        .collect()
}

fn manifests_dir() -> PathBuf {
    corpus_dir().join("manifests")
}

fn read_manifest(p: &Path) -> Manifest {
    let text = std::fs::read_to_string(p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", p.display()))
}

/// Build the population fingerprint from the manifests currently on disk.
fn current_lock() -> PopulationLock {
    // Per-manifest case counts (every *.json in manifests/ except the lock).
    let mdir = manifests_dir();
    let mut files: Vec<PathBuf> = std::fs::read_dir(&mdir)
        .unwrap_or_else(|e| panic!("read {}: {e}", mdir.display()))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .filter(|p| p.file_name().and_then(|n| n.to_str()) != Some(LOCK_FILE))
        .collect();
    files.sort();

    let mut manifest_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut solvable_now_paths: Vec<String> = Vec::new();
    for p in &files {
        let fname = p.file_name().unwrap().to_string_lossy().to_string();
        let m = read_manifest(p);
        manifest_counts.insert(fname.clone(), m.cases.len());
        if fname == "solvable_now.json" {
            solvable_now_paths = m.cases.iter().map(|c| c.path.replace('\\', "/")).collect();
            solvable_now_paths.sort();
        }
    }
    assert!(
        manifest_counts.contains_key("solvable_now.json"),
        "solvable_now.json not found in {}",
        mdir.display()
    );

    // Family manifest case counts.
    let mut family_counts: BTreeMap<String, usize> = BTreeMap::new();
    for fam in FAMILIES {
        let p = corpus_dir().join(fam).join("manifest.json");
        let m = read_manifest(&p);
        family_counts.insert(fam.to_string(), m.cases.len());
    }

    PopulationLock {
        comment: String::new(),
        manifest_counts,
        family_counts,
        solvable_now_paths,
    }
}

const COMMENT: &str = "Anti-shrink guard (FA fix 3): committed fingerprint of the gated corpus \
population. Any drift (a path leaving solvable_now, any manifest/family case-count change) fails \
population_lock.rs. Regenerate DELIBERATELY with `DSS_UPDATE_POPULATION_LOCK=1 cargo test -p \
dss-core --test population_lock` and commit the diff alongside the manifest change. See TESTING.md \
\u{00a7}Anti-shrink population lock.";

fn lock_path() -> PathBuf {
    manifests_dir().join(LOCK_FILE)
}

/// Serialize a lock to the committed on-disk form (stable pretty JSON + trailing
/// newline). Shared by the writer arm and the diff message so a regenerate always
/// reproduces byte-for-byte what the assertion compares against.
fn serialize_lock(lock: &PopulationLock) -> String {
    let with_comment = PopulationLock {
        comment: COMMENT.to_string(),
        manifest_counts: lock.manifest_counts.clone(),
        family_counts: lock.family_counts.clone(),
        solvable_now_paths: lock.solvable_now_paths.clone(),
    };
    let mut s = serde_json::to_string_pretty(&with_comment).expect("serialize lock");
    s.push('\n');
    s
}

#[test]
fn population_lock_matches_manifests() {
    let current = current_lock();
    let path = lock_path();

    // Deliberate-regeneration arm: rewrite the snapshot from current manifests.
    if std::env::var_os("DSS_UPDATE_POPULATION_LOCK").is_some() {
        std::fs::write(&path, serialize_lock(&current))
            .unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
        eprintln!(
            "population lock regenerated: {} ({} manifests, {} solvable_now paths)",
            path.display(),
            current.manifest_counts.len(),
            current.solvable_now_paths.len()
        );
        return;
    }

    assert!(
        path.is_file(),
        "population lock missing: {} — bootstrap it with \
         `DSS_UPDATE_POPULATION_LOCK=1 cargo test -p dss-core --test population_lock`",
        path.display()
    );
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let stored: PopulationLock =
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()));

    if current.data_eq(&stored) {
        return;
    }

    // Build a precise, actionable diff.
    let mut diff = String::new();
    diff.push_str("corpus population changed vs the committed lock.\n");

    // Manifest count changes.
    let mut keys: Vec<&String> = current
        .manifest_counts
        .keys()
        .chain(stored.manifest_counts.keys())
        .collect();
    keys.sort();
    keys.dedup();
    for k in keys {
        let now = current.manifest_counts.get(k).copied();
        let was = stored.manifest_counts.get(k).copied();
        if now != was {
            diff.push_str(&format!("  manifest {k}: {was:?} -> {now:?}\n"));
        }
    }
    // Family count changes.
    for k in current.family_counts.keys() {
        let now = current.family_counts.get(k).copied();
        let was = stored.family_counts.get(k).copied();
        if now != was {
            diff.push_str(&format!("  family {k}: {was:?} -> {now:?}\n"));
        }
    }
    // solvable_now path set changes (the shrink guard's core).
    let now_set: std::collections::BTreeSet<&String> = current.solvable_now_paths.iter().collect();
    let was_set: std::collections::BTreeSet<&String> = stored.solvable_now_paths.iter().collect();
    for removed in was_set.difference(&now_set) {
        diff.push_str(&format!("  solvable_now REMOVED: {removed}\n"));
    }
    for added in now_set.difference(&was_set) {
        diff.push_str(&format!("  solvable_now ADDED:   {added}\n"));
    }

    diff.push_str(
        "\nIf this change is intended (e.g. decks migrated in/out of solvable_now), \
         regenerate the lock DELIBERATELY:\n  \
         DSS_UPDATE_POPULATION_LOCK=1 cargo test -p dss-core --test population_lock\n\
         then commit the population.lock.json diff alongside the manifest change.\n",
    );
    panic!("{diff}");
}
