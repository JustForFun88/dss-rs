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
//! class, the case count; for every `solvable_now` case its path **and a per-case
//! rigor fingerprint** (kind/tolerance-tier, oracle target, `n_steps`, and every
//! compare-depth flag — `selected_elements`/`check_meters_monitors`/`probes`/
//! `compare_variables`/eventlog/ctrlqueue/all-properties/global-result/autoadd-log/
//! pending/solve-abort); and the three synthetic families' case counts **and path
//! lists**. The unconditional test below rebuilds that fingerprint from the current
//! manifests and asserts it equals the snapshot. Any drift — a path leaving
//! `solvable_now`, a retained deck *weakened in place* (kind flipped to a looser
//! band, steps/probes/meters cut), a family deck swapped, or any count change —
//! fails with a precise diff and the one-command regeneration path, so the shrink
//! lands as a reviewable diff in the lock file rather than passing unnoticed.
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

/// One property-probe spec (mirrors `corpus_live::ProbeSpec`); we only count them.
#[derive(Debug, Deserialize)]
struct ProbeSpec {
    #[serde(default)]
    #[allow(dead_code)]
    element: String,
}

/// A manifest case. Mirrors the rigor-bearing subset of `corpus_live::SolvableCase`
/// so the lock can fingerprint each `solvable_now` deck's actual compare depth, not
/// just its membership. Only `path` is needed for the non-`solvable_now` manifests.
#[derive(Debug, Deserialize)]
struct Case {
    #[serde(default)]
    path: String,
    #[serde(default = "default_kind")]
    kind: String,
    #[serde(default = "default_steps")]
    n_steps: usize,
    #[serde(default)]
    selected_elements: Vec<String>,
    #[serde(default)]
    check_meters_monitors: bool,
    #[serde(default)]
    probes: Vec<ProbeSpec>,
    #[serde(default)]
    compare_variables: Vec<String>,
    #[serde(default)]
    compare_eventlog: bool,
    #[serde(default)]
    compare_ctrlqueue: bool,
    #[serde(default)]
    compare_all_properties: bool,
    #[serde(default)]
    compare_global_result: bool,
    #[serde(default)]
    compare_autoadd_log: bool,
    #[serde(default)]
    pending: bool,
    #[serde(default)]
    expect_solve_abort: Option<String>,
    #[serde(default)]
    oracle: Option<String>,
}

fn default_kind() -> String {
    "feeder".to_string()
}
fn default_steps() -> usize {
    1
}

impl Case {
    /// A compact, deterministic one-line fingerprint of everything that sets this
    /// case's compare rigor. If any of these weakens on a **retained** deck (path
    /// unchanged, so the membership guard stays green), this string changes and the
    /// lock trips. Kept human-diffable so the reviewer sees *which* knob moved.
    fn rigor(&self) -> String {
        format!(
            "kind={} steps={} sel={} mm={} probes={} vars={} evlog={} ctrlq={} \
             props={} gresult={} aalog={} pending={} abort={} oracle={}",
            self.kind,
            self.n_steps,
            self.selected_elements.len(),
            self.check_meters_monitors as u8,
            self.probes.len(),
            self.compare_variables.len(),
            self.compare_eventlog as u8,
            self.compare_ctrlqueue as u8,
            self.compare_all_properties as u8,
            self.compare_global_result as u8,
            self.compare_autoadd_log as u8,
            self.pending as u8,
            self.expect_solve_abort.is_some() as u8,
            self.oracle.as_deref().unwrap_or("-"),
        )
    }
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
    /// `<family name> -> sorted case path list`. Catches a family deck being
    /// swapped for another at equal count (a count-only guard would miss it).
    family_paths: BTreeMap<String, Vec<String>>,
    /// Every `solvable_now.json` case: `path -> per-case rigor fingerprint`
    /// ([`Case::rigor`]). The key set is the membership guard (a deck leaving
    /// `solvable_now` drops a key); the value is the in-place-weakening guard (a
    /// retained deck's kind/steps/depth flags changing flips its fingerprint).
    solvable_now: BTreeMap<String, String>,
}

// Compare only the data fields; the free-text `comment` is documentation.
impl PopulationLock {
    fn data_eq(&self, other: &Self) -> bool {
        self.manifest_counts == other.manifest_counts
            && self.family_counts == other.family_counts
            && self.family_paths == other.family_paths
            && self.solvable_now == other.solvable_now
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
    let mut solvable_now: BTreeMap<String, String> = BTreeMap::new();
    for p in &files {
        let fname = p.file_name().unwrap().to_string_lossy().to_string();
        let m = read_manifest(p);
        manifest_counts.insert(fname.clone(), m.cases.len());
        if fname == "solvable_now.json" {
            for c in &m.cases {
                let path = c.path.replace('\\', "/");
                if let Some(prev) = solvable_now.insert(path.clone(), c.rigor()) {
                    panic!("duplicate solvable_now path {path} (prev rigor {prev})");
                }
            }
        }
    }
    assert!(
        manifest_counts.contains_key("solvable_now.json"),
        "solvable_now.json not found in {}",
        mdir.display()
    );

    // Family manifest case counts + sorted path lists.
    let mut family_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut family_paths: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for fam in FAMILIES {
        let p = corpus_dir().join(fam).join("manifest.json");
        let m = read_manifest(&p);
        family_counts.insert(fam.to_string(), m.cases.len());
        let mut paths: Vec<String> = m.cases.iter().map(|c| c.path.replace('\\', "/")).collect();
        paths.sort();
        family_paths.insert(fam.to_string(), paths);
    }

    PopulationLock {
        comment: String::new(),
        manifest_counts,
        family_counts,
        family_paths,
        solvable_now,
    }
}

const COMMENT: &str = "Anti-shrink guard (FA fix 3; FA settle: per-case rigor + family path lists): \
committed fingerprint of the gated corpus population. Trips on any drift — a path leaving \
solvable_now, a retained deck weakened in place (kind/tolerance-tier flip, steps/probes/meters cut \
— see solvable_now value fingerprints), a family deck swapped, or any manifest/family case-count \
change. Regenerate DELIBERATELY with `DSS_UPDATE_POPULATION_LOCK=1 cargo test -p dss-core --test \
population_lock` and commit the diff alongside the manifest change. See TESTING.md \u{00a7}Anti-shrink \
population lock.";

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
        family_paths: lock.family_paths.clone(),
        solvable_now: lock.solvable_now.clone(),
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
            "population lock regenerated: {} ({} manifests, {} solvable_now cases)",
            path.display(),
            current.manifest_counts.len(),
            current.solvable_now.len()
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
    // Family path-list changes (a deck swapped at equal count).
    for k in current.family_paths.keys() {
        let now = current.family_paths.get(k);
        let was = stored.family_paths.get(k);
        if now != was {
            let empty = Vec::new();
            let now_set: std::collections::BTreeSet<&String> =
                now.unwrap_or(&empty).iter().collect();
            let was_set: std::collections::BTreeSet<&String> =
                was.unwrap_or(&empty).iter().collect();
            for removed in was_set.difference(&now_set) {
                diff.push_str(&format!("  family {k} REMOVED: {removed}\n"));
            }
            for added in now_set.difference(&was_set) {
                diff.push_str(&format!("  family {k} ADDED:   {added}\n"));
            }
        }
    }
    // solvable_now membership changes (the shrink guard's core) …
    let now_set: std::collections::BTreeSet<&String> = current.solvable_now.keys().collect();
    let was_set: std::collections::BTreeSet<&String> = stored.solvable_now.keys().collect();
    for removed in was_set.difference(&now_set) {
        diff.push_str(&format!("  solvable_now REMOVED: {removed}\n"));
    }
    for added in now_set.difference(&was_set) {
        diff.push_str(&format!("  solvable_now ADDED:   {added}\n"));
    }
    // … and per-case rigor changes on retained decks (the in-place-weakening guard).
    for path in now_set.intersection(&was_set) {
        let now = current.solvable_now.get(*path);
        let was = stored.solvable_now.get(*path);
        if now != was {
            diff.push_str(&format!(
                "  solvable_now RIGOR {path}:\n    was: {}\n    now: {}\n",
                was.map(String::as_str).unwrap_or("<none>"),
                now.map(String::as_str).unwrap_or("<none>"),
            ));
        }
    }

    diff.push_str(
        "\nIf this change is intended (e.g. decks migrated in/out of solvable_now), \
         regenerate the lock DELIBERATELY:\n  \
         DSS_UPDATE_POPULATION_LOCK=1 cargo test -p dss-core --test population_lock\n\
         then commit the population.lock.json diff alongside the manifest change.\n",
    );
    panic!("{diff}");
}
