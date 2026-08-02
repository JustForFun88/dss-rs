//! Provenance lock over every committed golden artifact
//! (`GOLDEN_REBASE_PLAN.md` §1.2, sub-step G0.1) — the golden-side twin of
//! [`population_lock`](../population_lock.rs).
//!
//! # Why
//!
//! The golden corpus is about to stop being a third-party artifact. WP-G3
//! migrates most families to **self-snapshots** of our own engine and WP-G4
//! regenerates them again when the FPC print-emulation kernels die. That means
//! the project acquires, for the first time, the ability to *rewrite* a golden —
//! and with it the failure mode the old "never regenerate" rule made impossible:
//! a red gate quietly silenced by re-baselining the very bytes that were
//! supposed to catch it.
//!
//! This lock makes every byte movement a loud, reviewed event. Each artifact
//! carries a row recording:
//!
//! - `path` — repo-root-relative, forward slashes;
//! - `sha256` — content digest (see [`digest_of`] for the EOL rule);
//! - `anchor` — **where the truth in those bytes comes from**: `capi_v0145`
//!   (the pinned dss-python oracle, `tools/golden/PIN.txt`), `r4133` (the
//!   official EPRI engine), `r3723` (the retired EPRI revision that produced
//!   the A-Diakoptics witness), `capi015` (a dead 0.15.x probe environment that
//!   no longer exists), `fpc_3.2.2` (an FPC RTL print capture), or `self` (our
//!   own engine — a pure anti-regression snapshot);
//! - `reason` — mandatory for every `anchor: "self"` row (G3.6 extends the
//!   requirement to the rest);
//! - `produced_by` — which lane is allowed to *write* the artifact. `null` for
//!   every oracle-anchored row (nothing in this repo produces those bytes);
//!   `parity` / `lane-invariant` for `self` rows. Until WP-G4 the two lanes
//!   render different report/JSON bytes, so §1.2 makes the **parity** lane the
//!   producer of any family with a parity-only byte arm; `lane-invariant` is
//!   only ever set after a cross-lane regen has *measured* it (never assumed),
//!   which is WP-G4's closing job (G4.6). The field is bookkeeping today and
//!   becomes load-bearing in G0.2, where `harness::snapshot_*` refuses to write
//!   from the non-producing lane.
//!
//! # What this test asserts
//!
//! 1. **Every artifact on disk has a row** — a new golden cannot be smuggled in
//!    unfingerprinted.
//! 2. **Every row has an artifact on disk** — fail-on-stale. (This also closes
//!    the `props_roundtrip.rs` hole: that test asserts only that the scenario
//!    list is non-empty, so deleting a `props/` class file removed coverage
//!    silently.)
//! 3. **Every digest matches** — a golden byte cannot move without the lock diff
//!    moving with it, in the same reviewed commit.
//! 4. **`anchor: "self"` is a registered decision** — every self row carries a
//!    non-empty reason equal to its [`DEANCHORED`] family entry, the `self` set
//!    equals the set matched by that register (both directions, exactly like
//!    `oracle_parity_cfg_gate.rs::ESCAPE_REGISTER`), and `anchor == self` holds
//!    if and only if `produced_by` is set. De-anchoring a family is therefore a
//!    reviewable *code* edit in [`DEANCHORED`], never a side effect of a regen
//!    run.
//!
//! # Regenerate deliberately
//!
//! ```text
//! DSS_UPDATE_GOLDEN_LOCK=1 cargo test -p dss-core --test golden_lock
//! ```
//!
//! recomputes every digest from the artifacts currently on disk and rewrites the
//! lock. It **preserves** the curated metadata (`anchor`/`reason`/`produced_by`)
//! of rows that already exist, seeds new paths from [`seed_metadata`], and
//! refuses to write an unregistered `self` anchor — so this knob can move
//! digests, but never provenance. Run it only after reviewing *why* the bytes
//! moved, and commit the lock diff together with the change that caused it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The committed lock, repo-root-relative. It lives inside a scanned root and is
/// therefore skipped by the scan (it fingerprints artifacts, it is not one).
const LOCK_PATH: &str = "tests/golden/golden.lock.json";

/// Every tree the lock covers (§1.2 "Lock scope"): the golden corpus plus the
/// registered out-of-tree witness — the r3723 A-Diakoptics reference fixtures,
/// which are golden artifacts in everything but their location.
const ROOTS: &[&str] = &[
    "tests/golden",
    "crates/dss-core/tests/data/adiakoptics/r3723_ref",
];

/// Where the truth in an artifact's bytes comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
enum Anchor {
    /// Our own engine: a pure anti-regression snapshot of report *form*.
    #[serde(rename = "self")]
    SelfSnapshot,
    /// The pinned dss-python numeric oracle (`tools/golden/PIN.txt`, backend
    /// dss_capi 0.14.5 = the vendored Pascal spec).
    #[serde(rename = "capi_v0145")]
    CapiV0145,
    /// The official EPRI OpenDSS r4133 engine (the behavioral authority).
    #[serde(rename = "r4133")]
    R4133,
    /// The retired EPRI r3723 revision (the A-Diakoptics witness harvest).
    #[serde(rename = "r3723")]
    R3723,
    /// A dss_capi 0.15.x beta probe environment that no longer exists.
    #[serde(rename = "capi015")]
    Capi015,
    /// A Free Pascal 3.2.2 RTL print capture.
    #[serde(rename = "fpc_3.2.2")]
    Fpc322,
}

/// Which lane may *write* a `self` artifact (§1.2 "Which lane writes a
/// self-golden"). `None` on every oracle-anchored row: nothing in this repo
/// produces those bytes at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum ProducedBy {
    /// Rendered by the `oracle-parity` lane (the strictest byte contract while
    /// the print-emulation kernels live).
    #[serde(rename = "parity")]
    Parity,
    /// Measured identical in both lanes by a cross-lane regen (WP-G4 outcome).
    #[serde(rename = "lane-invariant")]
    LaneInvariant,
}

/// The `anchor: "self"` register, kept per **family/glob** with one reason per
/// family — a per-file list would just be a second copy of the lock. A pattern
/// ending in `/` is a directory prefix; anything else is an exact path.
///
/// Both entries are **born-`self`**: they were never oracle-anchored, so nothing
/// was de-anchored to create them. WP-G3 adds the migrated families here, one
/// reviewed row at a time.
const DEANCHORED: &[(&str, &str)] = &[
    (
        "tests/golden/adiakoptics/",
        "born-self: the emitted Torn_Circuit tree is our own D2 partitioner + Format_SubCircuits \
         output; no oracle emits a comparable tree (the external A-Diakoptics witness is the \
         separate r3723 numeric fixture set). Regen today bypasses the rails via \
         DSS_REGEN_AD_GOLDEN (adiakoptics.rs::torn_tree_matches_golden); G3.6 routes it through \
         harness::snapshot_*.",
    ),
    (
        "tests/golden/json/schema_full_port.json",
        "born-self: the port's own DSS_ExtractSchema document, spelled in the fpjson/CRLF \
         rendering the parity lane produces. Its external half (json/schema_full_oracle.json + \
         json/schema_divergences.json) stays capi_v0145 and frozen. Regen today bypasses the \
         rails via REGEN_SCHEMA_PORT (golden_schema.rs::full_document_matches_port_golden); G3.6 \
         routes it through harness::snapshot_*.",
    ),
];

/// The eleven artifacts whose provenance block declares `engine_spec:
/// "capi015"` (plan G0.1). Their generator environment — a dss_capi 0.15.0b4 /
/// DSS-Python 0.16.0b2 beta stack — no longer exists, so they can never be
/// regenerated and `snapshot_*` hard-refuses them.
const CAPI015_ARTIFACTS: &[&str] = &[
    "tests/golden/line_constants/line_geometry_carson.json",
    "tests/golden/ncim/pq.meta.json",
    "tests/golden/ncim/pv_qlimit.meta.json",
    "tests/golden/props/autotrans_bh.json",
    "tests/golden/props/linemedium.json",
    "tests/golden/props/linespacing_eqspacing.json",
    "tests/golden/props/regcontrol.json",
    "tests/golden/props/swtcontrol.json",
    "tests/golden/props/transformer_bh.json",
    "tests/golden/reports/export_capacity_seasonal.meta.json",
    "tests/golden/reports/export_overloads_seasonal.meta.json",
];

const CAPI015_REASON: &str = "captured on the retired dss_capi 0.15.0b4 / DSS-Python 0.16.0b2 beta stack (the pinned \
     0.14.5 oracle cannot render these 0.15.x-only surfaces); that environment no longer \
     exists, so the bytes are unreproducible and snapshot_* hard-refuses them.";

/// Families captured on the official EPRI r4133 engine rather than the pinned
/// capi oracle. They stay externally anchored through WP-G3 (§1.2: they are
/// *not* frozen-`capi_v0145`; their disposition is recorded here and re-confirmed
/// in G3.6).
const R4133_FAMILIES: &[(&str, &str)] = &[
    (
        "tests/golden/flicker/",
        "IEC 61000-4-15 Pst captured on the official EPRI r4133 engine; r4133 is the behavioral \
         authority for the flicker meter and this family is the only Pst oracle gate.",
    ),
    (
        "tests/golden/protection/",
        "fuse/SwtControl scenarios captured on r4133 (tools/golden/gen_protection.py \
         EPRI_SCENARIOS): the WP-U2.1 fuse overhaul and the WP-U2.4 SwtControl Action fix exist \
         only in r4133, so the pinned 0.14.5 oracle is not the authority here.",
    ),
    (
        "tests/golden/wasm_usermodels/",
        "user-model trajectories captured on r4133 via crates/dss-epri \
         (gen_wasm_usermodels*.rs); the wasm host must reproduce the authority engine's \
         DLL-model behavior, so these stay externally anchored (§1.2 frozen set).",
    ),
];

/// The single FPC-RTL print capture.
const FPC_ARTIFACT: &str = "tests/golden/fmt_battery.csv";

const FPC_REASON: &str = "a Free Pascal 3.2.2 RTL print capture (the %g/%f spelling battery behind the compat print \
     kernels), not an engine capture. Retires with those kernels in WP-G4 (G4.6, measure-first).";

/// The out-of-tree A-Diakoptics witness tree.
const R3723_TREE: &str = "crates/dss-core/tests/data/adiakoptics/r3723_ref/";

const R3723_REASON: &str = "the only external A-Diakoptics witness (zll/zcc/y4/voltages), harvested from the official \
     EPRI r3723 engine by tools/opendss/gen_ad_reference.py; that revision is retired, so the \
     tree is frozen (§1.2).";

/// One fingerprinted artifact.
///
/// `deny_unknown_fields`: a mistyped key in the lock is a lost assertion (the
/// row would silently fall back to its default), so it fails loudly instead.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Artifact {
    /// Repo-root-relative, forward slashes.
    path: String,
    /// Lowercase hex SHA-256 over the artifact's committed content
    /// ([`digest_of`]).
    sha256: String,
    anchor: Anchor,
    /// Mandatory for `anchor: "self"`; G3.6 extends the requirement to the rest.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    reason: String,
    /// `null` for oracle-anchored rows (see the module docs).
    produced_by: Option<ProducedBy>,
}

/// The committed lock document.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GoldenLock {
    /// Human note; never compared (documentation may be edited freely).
    #[serde(default)]
    comment: String,
    /// Sorted by `path`; one row per committed golden artifact.
    artifacts: Vec<Artifact>,
}

const COMMENT: &str = "Provenance lock over every committed golden artifact (GOLDEN_REBASE_PLAN.md \
\u{a7}1.2, G0.1): tests/golden/** plus the registered out-of-tree witness \
crates/dss-core/tests/data/adiakoptics/r3723_ref/. Each row records the artifact's content digest \
and where the truth in those bytes comes from (anchor), plus - for self-anchored rows - a mandatory \
reason and the lane allowed to (re)produce them. crates/dss-core/tests/golden_lock.rs asserts, \
fail-on-stale in both directions: every artifact has a row, every row an artifact, every digest \
matches, and every anchor=self row is registered in the test's DEANCHORED family register. Digests \
are taken over the COMMITTED content: CRLF is normalized to LF for text artifacts (core.autocrlf=true \
here, so the working tree carries CRLF while git stores LF), while reports/*.bin streams - `binary` in \
.gitattributes - are hashed raw. Regenerate DELIBERATELY with \
`DSS_UPDATE_GOLDEN_LOCK=1 cargo test -p dss-core --test golden_lock`; it moves digests only, never \
provenance, and the diff is the review artifact. The regeneration rules R1-R4 and the operational \
walkthrough land in TESTING.md with the snapshot helpers (plan sub-step G0.2); until then the \
authority is the golden_lock.rs module documentation.";

fn repo_root() -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "..", ".."].iter().collect()
}

/// Does `pattern` (a `/`-terminated directory prefix, or an exact path) cover
/// `path`?
fn pattern_covers(pattern: &str, path: &str) -> bool {
    if pattern.ends_with('/') {
        path.starts_with(pattern)
    } else {
        path == pattern
    }
}

/// The [`DEANCHORED`] entries covering `path` (a well-formed register yields at
/// most one; the test asserts that).
fn deanchored_entries(path: &str) -> Vec<&'static (&'static str, &'static str)> {
    DEANCHORED
        .iter()
        .filter(|(pattern, _)| pattern_covers(pattern, path))
        .collect()
}

/// The initial anchor classification of the plan's G0.1 section, applied to a
/// path the lock does not know yet. Never invents provenance for a family it
/// does not recognize: the fallback is the pinned oracle, which is what every
/// `tools/golden/gen_*.py` generator captured.
fn seed_metadata(path: &str) -> (Anchor, String, Option<ProducedBy>) {
    let registered = deanchored_entries(path);
    if let Some((_, reason)) = registered.first() {
        // §1.2: until WP-G4 proves lane-invariance by a cross-lane regen, the
        // producing lane is the parity lane (the strictest byte contract).
        return (
            Anchor::SelfSnapshot,
            (*reason).to_string(),
            Some(ProducedBy::Parity),
        );
    }
    if CAPI015_ARTIFACTS.contains(&path) {
        return (Anchor::Capi015, CAPI015_REASON.to_string(), None);
    }
    if let Some((_, reason)) = R4133_FAMILIES
        .iter()
        .find(|(prefix, _)| pattern_covers(prefix, path))
    {
        return (Anchor::R4133, (*reason).to_string(), None);
    }
    if path == FPC_ARTIFACT {
        return (Anchor::Fpc322, FPC_REASON.to_string(), None);
    }
    if path.starts_with(R3723_TREE) {
        return (Anchor::R3723, R3723_REASON.to_string(), None);
    }
    (Anchor::CapiV0145, String::new(), None)
}

/// `reports/*.bin` are raw little-endian IEEE-754 streams, declared `binary` in
/// `.gitattributes` precisely so git never EOL-munges them.
fn is_binary_artifact(rel: &str) -> bool {
    rel.ends_with(".bin")
}

/// Strip the `\r` of every `\r\n` pair. Reproduces git's `core.autocrlf`
/// check-in filter, so the digest is over the artifact's **committed** content
/// and does not depend on the checkout's EOL configuration.
fn normalize_eol(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == b'\r' && raw.get(i + 1) == Some(&b'\n') {
            i += 1;
            continue;
        }
        out.push(raw[i]);
        i += 1;
    }
    out
}

/// SHA-256 over the artifact's committed content — see [`normalize_eol`].
///
/// A text-classified artifact holding a NUL byte would mean a binary stream
/// escaped [`is_binary_artifact`] and is being EOL-normalized; that is a hard
/// failure, not a silent digest over munged bytes.
fn digest_of(abs: &Path, rel: &str) -> String {
    let raw = std::fs::read(abs)
        .unwrap_or_else(|e| panic!("read golden artifact {}: {e}", abs.display()));
    let bytes = if is_binary_artifact(rel) {
        raw
    } else {
        assert!(
            !raw.contains(&0),
            "golden artifact {rel} is classified as text but contains a NUL byte: it is a binary \
             stream whose digest would be taken over EOL-normalized bytes. Mark it `binary` in \
             .gitattributes and teach is_binary_artifact() about it."
        );
        normalize_eol(&raw)
    };
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    format!("{:x}", hasher.finalize())
}

fn rel_path(root: &Path, abs: &Path) -> String {
    abs.strip_prefix(root)
        .unwrap_or_else(|_| panic!("{} is not under {}", abs.display(), root.display()))
        .to_string_lossy()
        .replace('\\', "/")
}

fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, String>) {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("read dir {}: {e}", dir.display()))
        .map(|e| e.unwrap_or_else(|e| panic!("read dir entry under {}: {e}", dir.display())))
        .map(|e| e.path())
        .collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            walk(root, &p, out);
            continue;
        }
        let rel = rel_path(root, &p);
        if rel == LOCK_PATH {
            continue;
        }
        let digest = digest_of(&p, &rel);
        if let Some(prev) = out.insert(rel.clone(), digest) {
            panic!("duplicate artifact path {rel} (previous digest {prev})");
        }
    }
}

/// `path -> digest` for every artifact currently on disk under [`ROOTS`].
fn scan_disk(root: &Path) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for r in ROOTS {
        let dir = root.join(r);
        assert!(
            dir.is_dir(),
            "golden root {} is missing — the lock cannot fingerprint a tree that is not there",
            dir.display()
        );
        let before = out.len();
        walk(root, &dir, &mut out);
        assert!(
            out.len() > before,
            "golden root {r} contributed no artifacts; a vacuous scan would make this lock \
             assert nothing"
        );
    }
    out
}

fn lock_path(root: &Path) -> PathBuf {
    root.join(LOCK_PATH)
}

fn read_lock(path: &Path) -> GoldenLock {
    let text =
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
}

/// The on-disk form: stable pretty JSON + trailing newline, so a regen
/// reproduces byte-for-byte what the assertion compares against.
fn serialize_lock(artifacts: &[Artifact]) -> String {
    let doc = GoldenLock {
        comment: COMMENT.to_string(),
        artifacts: artifacts.to_vec(),
    };
    let mut s = serde_json::to_string_pretty(&doc).expect("serialize golden lock");
    s.push('\n');
    s
}

/// Rebuild the row set from disk, preserving the curated metadata of rows that
/// already exist and seeding the rest. Refuses to write an unregistered `self`
/// anchor: de-anchoring is a [`DEANCHORED`] edit, never a regen side effect.
fn regenerate(
    disk: &BTreeMap<String, String>,
    stored: &BTreeMap<String, Artifact>,
) -> Vec<Artifact> {
    disk.iter()
        .map(|(path, sha256)| {
            let registered = deanchored_entries(path);
            assert!(
                registered.len() <= 1,
                "{path} is covered by {} DEANCHORED entries; the register must assign exactly one \
                 reason per artifact",
                registered.len()
            );
            let prev = stored.get(path);
            let (anchor, reason, produced_by) = match (registered.first(), prev) {
                // Registered `self`: the register owns anchor + reason, so
                // editing a family's reason takes effect on regen. Only the
                // measured `produced_by` survives from the stored row.
                (Some((_, reason)), prev) => (
                    Anchor::SelfSnapshot,
                    (*reason).to_string(),
                    Some(
                        prev.and_then(|a| a.produced_by)
                            .unwrap_or(ProducedBy::Parity),
                    ),
                ),
                // Unregistered: keep whatever provenance was curated for it.
                (None, Some(prev)) => {
                    assert_ne!(
                        prev.anchor,
                        Anchor::SelfSnapshot,
                        "{path} is anchored `self` but no DEANCHORED entry covers it. \
                         De-anchoring is a reviewed register edit, not a regen side effect — add \
                         the family (with its reason) to DEANCHORED in golden_lock.rs first."
                    );
                    (prev.anchor, prev.reason.clone(), prev.produced_by)
                }
                (None, None) => seed_metadata(path),
            };
            Artifact {
                path: path.clone(),
                sha256: sha256.clone(),
                anchor,
                reason,
                produced_by,
            }
        })
        .collect()
}

fn by_path(artifacts: &[Artifact]) -> BTreeMap<String, Artifact> {
    let mut out: BTreeMap<String, Artifact> = BTreeMap::new();
    for a in artifacts {
        if let Some(prev) = out.insert(a.path.clone(), a.clone()) {
            panic!(
                "duplicate row for {} in {LOCK_PATH} (previous digest {})",
                a.path, prev.sha256
            );
        }
    }
    out
}

#[test]
fn golden_lock_matches_the_committed_artifacts() {
    let root = repo_root();
    let path = lock_path(&root);
    let disk = scan_disk(&root);

    // Deliberate-regeneration arm.
    if std::env::var_os("DSS_UPDATE_GOLDEN_LOCK").is_some() {
        let stored = if path.is_file() {
            by_path(&read_lock(&path).artifacts)
        } else {
            BTreeMap::new()
        };
        let artifacts = regenerate(&disk, &stored);
        std::fs::write(&path, serialize_lock(&artifacts))
            .unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
        eprintln!(
            "golden lock regenerated: {} ({} artifacts)",
            path.display(),
            artifacts.len()
        );
        return;
    }

    assert!(
        path.is_file(),
        "golden provenance lock missing: {} — bootstrap it with \
         `DSS_UPDATE_GOLDEN_LOCK=1 cargo test -p dss-core --test golden_lock`",
        path.display()
    );
    let lock = read_lock(&path);
    let rows = by_path(&lock.artifacts);
    assert!(
        lock.artifacts.windows(2).all(|w| w[0].path < w[1].path),
        "{LOCK_PATH} is not sorted by path; regenerate it with DSS_UPDATE_GOLDEN_LOCK=1"
    );

    let mut diff = String::new();

    // (1) every artifact on disk has a row; (3) every digest matches.
    for (p, sha) in &disk {
        match rows.get(p) {
            None => diff.push_str(&format!("  UNFINGERPRINTED (no lock row): {p}\n")),
            Some(row) if row.sha256 != *sha => diff.push_str(&format!(
                "  DIGEST MOVED {p}:\n    locked: {}\n    ondisk: {sha}\n",
                row.sha256
            )),
            Some(_) => {}
        }
    }
    // (2) every row has an artifact on disk.
    for p in rows.keys() {
        if !disk.contains_key(p) {
            diff.push_str(&format!("  STALE ROW (artifact deleted): {p}\n"));
        }
    }

    // (4) `anchor: "self"` is a registered decision, both directions.
    let mut register_hits = vec![0usize; DEANCHORED.len()];
    for row in &lock.artifacts {
        let covering: Vec<usize> = DEANCHORED
            .iter()
            .enumerate()
            .filter(|(_, (pattern, _))| pattern_covers(pattern, &row.path))
            .map(|(i, _)| i)
            .collect();
        for &i in &covering {
            register_hits[i] += 1;
        }
        let is_self = row.anchor == Anchor::SelfSnapshot;
        if is_self && covering.is_empty() {
            diff.push_str(&format!(
                "  UNREGISTERED SELF ANCHOR: {} — add its family (with a reason) to the \
                 DEANCHORED register in golden_lock.rs\n",
                row.path
            ));
        }
        if !is_self && !covering.is_empty() {
            diff.push_str(&format!(
                "  REGISTERED FAMILY, NON-SELF ANCHOR: {} is covered by DEANCHORED entry {:?} \
                 but is anchored {:?} — the register would be claiming a de-anchoring that did \
                 not happen\n",
                row.path, DEANCHORED[covering[0]].0, row.anchor
            ));
        }
        if covering.len() > 1 {
            diff.push_str(&format!(
                "  AMBIGUOUS REGISTER: {} is covered by {} DEANCHORED entries; exactly one reason \
                 per artifact\n",
                row.path,
                covering.len()
            ));
        }
        if is_self {
            if row.reason.trim().is_empty() {
                diff.push_str(&format!(
                    "  SELF ROW WITHOUT REASON: {} — every self-anchored artifact states why\n",
                    row.path
                ));
            } else if covering
                .first()
                .is_some_and(|&i| row.reason != DEANCHORED[i].1)
            {
                diff.push_str(&format!(
                    "  REASON OUT OF SYNC: {} does not carry its DEANCHORED reason (regenerate \
                     with DSS_UPDATE_GOLDEN_LOCK=1 after editing the register)\n",
                    row.path
                ));
            }
        }
        // `anchor == self` <=> the artifact is (re)producible in-repo.
        if is_self != row.produced_by.is_some() {
            diff.push_str(&format!(
                "  PRODUCED_BY MISMATCH: {} is anchored {:?} with produced_by {:?}; exactly the \
                 self-anchored rows name a producing lane\n",
                row.path, row.anchor, row.produced_by
            ));
        }
    }
    for (i, (pattern, _)) in DEANCHORED.iter().enumerate() {
        if register_hits[i] == 0 {
            diff.push_str(&format!(
                "  STALE DEANCHORED ENTRY: {pattern:?} covers no locked artifact\n"
            ));
        }
    }

    if diff.is_empty() {
        return;
    }
    panic!(
        "the committed golden corpus no longer matches {LOCK_PATH} \
         ({} artifacts on disk, {} rows locked):\n{diff}\n\
         A golden byte NEVER moves to make a red gate green (GOLDEN_REBASE_PLAN.md \u{a7}1.2 R2): \
         fix the engine, pin the intended new value with an expected-value test, and only then \
         regenerate — the diff must move exactly the predicted artifacts. Regenerate \
         deliberately with:\n  \
         DSS_UPDATE_GOLDEN_LOCK=1 cargo test -p dss-core --test golden_lock\n\
         and commit golden.lock.json together with the change that moved the bytes.",
        disk.len(),
        rows.len(),
    );
}

/// The digest pipeline is the whole value of this lock — pin it against NIST
/// vectors and the git check-in filter it emulates, so a hashing or
/// normalization mistake fails here rather than silently blessing every row.
#[test]
fn digest_pipeline_is_pinned() {
    let sha = |b: &[u8]| {
        let mut h = Sha256::new();
        h.update(b);
        format!("{:x}", h.finalize())
    };
    assert_eq!(
        sha(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        sha(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );

    assert_eq!(normalize_eol(b"a\r\nb\r\n"), b"a\nb\n");
    assert_eq!(normalize_eol(b"a\nb\n"), b"a\nb\n");
    // A lone CR is content, not a line ending — git's filter leaves it alone.
    assert_eq!(normalize_eol(b"a\rb"), b"a\rb");
    assert_eq!(normalize_eol(b"\r\n\r\n"), b"\n\n");

    assert!(is_binary_artifact("tests/golden/reports/dump3_ieee13.bin"));
    assert!(!is_binary_artifact("tests/golden/reports/di_em1_phv.txt"));
}

/// Register hygiene, independent of the corpus: the patterns must be
/// well-formed and mutually exclusive, and every constant family list must name
/// distinct paths. A duplicated or overlapping entry would make the "exactly one
/// reason per artifact" rule unenforceable.
#[test]
fn provenance_registers_are_well_formed() {
    for (pattern, reason) in DEANCHORED {
        assert!(
            !pattern.starts_with('/') && !pattern.contains('\\'),
            "DEANCHORED pattern {pattern:?} must be repo-root-relative with forward slashes"
        );
        assert!(
            ROOTS.iter().any(|r| pattern.starts_with(r)),
            "DEANCHORED pattern {pattern:?} is outside the locked roots {ROOTS:?}"
        );
        assert!(
            !reason.trim().is_empty(),
            "DEANCHORED entry {pattern:?} has no reason"
        );
    }
    for (i, (a, _)) in DEANCHORED.iter().enumerate() {
        for (b, _) in DEANCHORED.iter().skip(i + 1) {
            assert!(
                !pattern_covers(a, b) && !pattern_covers(b, a),
                "DEANCHORED entries {a:?} and {b:?} overlap"
            );
        }
    }

    let mut capi015 = CAPI015_ARTIFACTS.to_vec();
    capi015.sort_unstable();
    capi015.dedup();
    assert_eq!(
        capi015.len(),
        CAPI015_ARTIFACTS.len(),
        "CAPI015_ARTIFACTS has duplicate entries"
    );
    assert_eq!(
        CAPI015_ARTIFACTS.len(),
        11,
        "the capi015 set is closed (GOLDEN_REBASE_PLAN.md G0.1 enumerates all eleven); it shrinks \
         only when an artifact is deleted, and never grows — that beta environment is gone"
    );

    // The seeding classification must be unambiguous: no artifact may fall into
    // two special families at once.
    for (prefix, _) in R4133_FAMILIES {
        assert!(
            prefix.ends_with('/'),
            "R4133_FAMILIES entry {prefix:?} must be a directory prefix"
        );
        assert!(
            !CAPI015_ARTIFACTS.iter().any(|p| p.starts_with(prefix)),
            "r4133 family {prefix:?} overlaps the capi015 set"
        );
        assert!(
            !DEANCHORED
                .iter()
                .any(|(d, _)| pattern_covers(d, prefix) || pattern_covers(prefix, d)),
            "r4133 family {prefix:?} overlaps the DEANCHORED register"
        );
    }
    assert!(!CAPI015_ARTIFACTS.contains(&FPC_ARTIFACT));
    assert!(R3723_TREE.ends_with('/'));
}
