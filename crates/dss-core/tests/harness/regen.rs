//! Self-golden regeneration rails — the `DSS_UPDATE_GOLDENS` plumbing and the
//! two write guards of `GOLDEN_REBASE_PLAN.md` §1.2 (sub-step G0.2).
//!
//! # Why a guarded writer at all
//!
//! WP-G3 converts most golden families into **self-snapshots** of our own
//! engine, and WP-G4 regenerates them again when the FPC print-emulation
//! kernels die. That gives the project a capability the old "never regenerate"
//! rule made impossible — rewriting a golden — together with the failure mode
//! it made impossible: a red gate quietly silenced by re-baselining the very
//! bytes that were supposed to catch it.
//!
//! The rails make that impossible to do *by accident*. Every write goes through
//! [`snapshot_text`] / [`snapshot_bytes`], which:
//!
//! 1. do **nothing at all** unless `DSS_UPDATE_GOLDENS` is set — during the
//!    mandatory gate they are inert, so a snapshot call site can never
//!    self-approve a comparison it is about to make;
//! 2. **refuse any artifact whose lock anchor is not `self`** — an
//!    oracle-anchored artifact (`capi_v0145` / `r4133` / `r3723` / `capi015` /
//!    `fpc_3.2.2`) is somebody else's capture, and de-anchoring it is a reviewed
//!    edit to the `DEANCHORED` register in `golden_lock.rs`, never a side effect
//!    of pressing a regen button;
//! 3. **refuse a write from the non-producing lane** — §1.2: until WP-G4 the two
//!    lanes render different report/JSON bytes, so each self-anchored family
//!    records in the lock (`produced_by`) which lane may (re)produce it. A
//!    parity-produced family regenerated from the default lane would silently
//!    re-baseline the parity byte contract onto default-lane rendering.
//!
//! The refusals are *skips*, not panics: a family regen legitimately sweeps
//! artifacts it must not touch (G3.3a snapshots 144 of `reports/export*`'s 146
//! files — the two `capi015` `.meta.json` sidecars are refused; G3.5 refuses the
//! six `capi015` `props/` scenarios). Every refusal is announced on stderr with
//! its remedy, and the artifact is left byte-identical.
//!
//! # The announcements need `-- --nocapture`
//!
//! libtest captures everything a test writes through `print!`/`eprintln!` and
//! **discards it when the test passes** — and a regen run passes by design (it
//! writes goldens instead of comparing them). Every documented regen command
//! therefore ends in `-- --nocapture`; without it the SNAPSHOT/REFUSED lines and
//! the stale-lock reminder below are dropped, and a sweep that silently refused
//! N artifacts is indistinguishable from one that wrote them. The refusal
//! *outcome* is still observable without the flag — [`snapshot_text`] /
//! [`snapshot_bytes`] return a `#[must_use]` [`Outcome`], so a driver can assert
//! against its family's expected refusal list rather than trusting stderr.
//!
//! # Relationship to the lock
//!
//! The rails **read** `tests/golden/golden.lock.json`; they never write it. The
//! lock is owned by `crates/dss-core/tests/golden_lock.rs`, which re-derives
//! every anchor from its registers — so a writer that also rewrote lock rows
//! could launder a provenance claim, and several test binaries regenerating
//! concurrently would race over one file. Instead, a regen run leaves the lock
//! **stale on purpose**: `golden_lock.rs` goes red with `DIGEST MOVED` until the
//! operator reviews the diff and runs
//!
//! ```text
//! DSS_UPDATE_GOLDEN_LOCK=1 cargo test -p dss-core --test golden_lock -- --nocapture
//! ```
//!
//! which is the reviewed event the whole design exists to force. The rails print
//! that command once per regen run.
//!
//! # Regeneration rules (TESTING.md §Procedures, R1–R4)
//!
//! R1 clean tree only; R2 **never** to fix a red gate (fix → pin the intended
//! new value with an expected-value test → regen → the diff moves ONLY the
//! predicted cells); R3 per-family only; R4 the lock digest diff is the review
//! artifact and the commit body names every moved artifact and its cause.
//!
//! No golden driver calls these helpers yet — wiring them is WP-G3.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use serde::Deserialize;

/// The knob that turns a comparison run into a regeneration run.
pub const UPDATE_ENV: &str = "DSS_UPDATE_GOLDENS";

/// The provenance lock the guards read, repo-root-relative.
pub const LOCK_PATH: &str = "tests/golden/golden.lock.json";

/// What the operator must run after a regen so the lock stops being stale.
/// `--nocapture` because that run *passes* (it rewrites the lock and returns),
/// and libtest discards a passing test's stderr — including the `SEEDED` /
/// `RE-ANCHORED` provenance announcements the operator is supposed to review.
pub const LOCK_REGEN_CMD: &str =
    "DSS_UPDATE_GOLDEN_LOCK=1 cargo test -p dss-core --test golden_lock -- --nocapture";

/// Where the truth in an artifact's bytes comes from — the mirror of
/// `golden_lock.rs::Anchor`. Only `self` is writable.
///
/// Deliberately duplicated rather than shared: `golden_lock.rs` is a separate
/// test binary, so there is nothing to import. The unit test
/// `the_committed_lock_parses_into_this_mirror` binds the two — a variant
/// renamed on the lock side fails to deserialize here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Anchor {
    /// Our own engine: a pure anti-regression snapshot of report *form*.
    #[serde(rename = "self")]
    SelfSnapshot,
    /// The pinned dss-python numeric oracle (`tools/golden/PIN.txt`).
    #[serde(rename = "capi_v0145")]
    CapiV0145,
    /// The official EPRI OpenDSS r4133 engine.
    #[serde(rename = "r4133")]
    R4133,
    /// The retired EPRI r3723 revision (the A-Diakoptics witness).
    #[serde(rename = "r3723")]
    R3723,
    /// A dss_capi 0.15.x beta probe environment that no longer exists.
    #[serde(rename = "capi015")]
    Capi015,
    /// A Free Pascal 3.2.2 RTL print capture.
    #[serde(rename = "fpc_3.2.2")]
    Fpc322,
}

impl Anchor {
    /// The spelling used in the lock (and in the refusal messages).
    pub fn as_str(self) -> &'static str {
        match self {
            Anchor::SelfSnapshot => "self",
            Anchor::CapiV0145 => "capi_v0145",
            Anchor::R4133 => "r4133",
            Anchor::R3723 => "r3723",
            Anchor::Capi015 => "capi015",
            Anchor::Fpc322 => "fpc_3.2.2",
        }
    }
}

/// Which lane may *write* a `self` artifact — the mirror of
/// `golden_lock.rs::ProducedBy`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum ProducedBy {
    /// Rendered by the `oracle-parity` lane (the strictest byte contract while
    /// the print-emulation kernels live).
    #[serde(rename = "parity")]
    Parity,
    /// Measured identical in both lanes by a cross-lane regen (WP-G4 outcome).
    #[serde(rename = "lane-invariant")]
    LaneInvariant,
}

impl ProducedBy {
    pub fn as_str(self) -> &'static str {
        match self {
            ProducedBy::Parity => "parity",
            ProducedBy::LaneInvariant => "lane-invariant",
        }
    }

    /// Does a build running in `lane` hold this family's rendering contract?
    fn admits(self, lane: Lane) -> bool {
        match self {
            ProducedBy::Parity => lane == Lane::Parity,
            // Only ever set after a cross-lane regen *measured* the invariance
            // (§1.2 — never assumed), so either lane reproduces the bytes.
            ProducedBy::LaneInvariant => true,
        }
    }
}

/// The lane of the running build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lane {
    Parity,
    Default,
}

impl Lane {
    /// The lane this test binary was compiled into. Read from the engine's own
    /// switch, exactly like the harness-wide `lane::PARITY`; the two are
    /// asserted equal by `lane_current_tracks_the_build`.
    pub fn current() -> Lane {
        if dss_core::compat::ORACLE_PARITY {
            Lane::Parity
        } else {
            Lane::Default
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Lane::Parity => "parity",
            Lane::Default => "default",
        }
    }

    /// The cargo invocation that produces this lane.
    fn build_flag(self) -> &'static str {
        match self {
            Lane::Parity => "--features dss-core/oracle-parity",
            Lane::Default => "(no features)",
        }
    }
}

/// Why a write was refused. Each variant is a *guard outcome*, never an
/// operator inconvenience to be worked around: the remedy is always a reviewed
/// edit somewhere else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// No row in `golden.lock.json`. Writing here would smuggle an
    /// unfingerprinted artifact into the corpus — the exact hole the lock
    /// exists to close.
    Unlocked,
    /// The artifact's truth comes from somewhere else (§1.2 guard 1).
    ExternallyAnchored(Anchor),
    /// The running build is not the family's producing lane (§1.2 guard 2).
    WrongLane { producer: ProducedBy, running: Lane },
    /// `anchor: "self"` with no `produced_by`. `golden_lock.rs` asserts the
    /// equivalence, so this can only be a hand-edited lock — refuse rather than
    /// guess a producing lane.
    NoProducingLane,
}

impl Refusal {
    /// The one-line explanation printed with every refusal.
    fn explain(&self, rel: &str) -> String {
        match self {
            Refusal::Unlocked => format!(
                "{rel} has no row in {LOCK_PATH}. A snapshot may only rewrite an artifact the \
                 provenance lock already knows: create it, register its family in \
                 golden_lock.rs::DEANCHORED, run `{LOCK_REGEN_CMD}`, then regenerate."
            ),
            Refusal::ExternallyAnchored(a) => format!(
                "{rel} is anchored `{}`, not `self`: those bytes are that engine's capture, not \
                 ours to re-render. De-anchoring is a reviewed edit to golden_lock.rs::DEANCHORED \
                 (with a reason) plus `{LOCK_REGEN_CMD}` — never a regen side effect.",
                a.as_str()
            ),
            Refusal::WrongLane { producer, running } => format!(
                "{rel} is produced by the `{}` lane, but this build is `{}`. Until WP-G4 the lanes \
                 render different bytes, so regenerate it with `{}` — writing it from here would \
                 re-baseline the strict lane's byte contract onto the other lane's rendering.",
                producer.as_str(),
                running.as_str(),
                match producer {
                    ProducedBy::Parity => Lane::Parity.build_flag(),
                    ProducedBy::LaneInvariant => Lane::Default.build_flag(),
                }
            ),
            Refusal::NoProducingLane => format!(
                "{rel} is anchored `self` but names no producing lane. golden_lock.rs asserts \
                 `anchor == self` iff `produced_by` is set, so the lock has been hand-edited; fix \
                 it with `{LOCK_REGEN_CMD}` rather than letting a regen pick a lane."
            ),
        }
    }
}

/// The result of a [`snapshot_text`] / [`snapshot_bytes`] call.
///
/// `must_use`: a dropped `Refused` is a refusal nobody saw — libtest hides the
/// stderr announcement of a passing test unless the run carries `--nocapture`,
/// so the returned outcome is the only refusal signal a driver can assert on.
#[must_use = "a snapshot outcome carries the guard's refusal; assert it against the family's \
              expected outcome instead of dropping it"]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// `DSS_UPDATE_GOLDENS` is not set — this is a comparison run and nothing
    /// was written. The overwhelmingly common case: the mandatory gate.
    NotRequested,
    /// The artifact was rewritten; the lock is now stale on purpose.
    Wrote,
    /// A guard refused; the artifact is byte-identical to what it was.
    Refused(Refusal),
}

impl Outcome {
    pub fn wrote(&self) -> bool {
        *self == Outcome::Wrote
    }
}

/// One row of `golden.lock.json`, as the rails need to see it.
///
/// `deny_unknown_fields` is the drift binder against `golden_lock.rs::Artifact`:
/// a field added there (or a typo here) fails to parse loudly instead of
/// silently defaulting — a guard reading a defaulted anchor would be a guard
/// that stopped guarding.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct LockRow {
    path: String,
    /// Never read here — the digest is `golden_lock.rs`'s business — but
    /// declared so `deny_unknown_fields` binds the whole schema.
    sha256: String,
    anchor: Anchor,
    /// Omitted from the lock when empty (`skip_serializing_if`).
    #[serde(default)]
    reason: String,
    produced_by: Option<ProducedBy>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LockDoc {
    #[serde(default)]
    comment: String,
    artifacts: Vec<LockRow>,
}

/// `reports/*.bin` are raw little-endian IEEE-754 streams, declared `binary` in
/// `.gitattributes`. The same predicate `golden_lock.rs::is_binary_artifact`
/// uses to decide how a digest is taken — it must stay identical so
/// [`snapshot_text`]'s payload guard and the lock's digest rule cannot disagree
/// about what a text artifact is.
///
/// The duplication is *bound*, not merely intended: `golden_lock.rs` is a
/// separate test binary, so both copies are tied to the same third source of
/// truth — the `.gitattributes` declaration — its own copy by
/// `assert_binary_classification_matches_gitattributes`, this one by
/// [`the_binary_classifier_matches_gitattributes`]. A future binary family with
/// another extension therefore reds *both* files instead of drifting past this
/// one.
fn is_binary_artifact(rel: &str) -> bool {
    rel.ends_with(".bin")
}

/// The `.gitattributes` pathspecs that declare a locked artifact `binary`,
/// pinned here so a *new* one cannot appear without this module being revisited
/// ([`the_binary_classifier_matches_gitattributes`] asserts the set both ways).
/// Kept to the one shape the file actually uses, `<dir>/*<suffix>`.
#[cfg(test)]
const GITATTRIBUTES_BINARY_PATHSPECS: &[&str] = &["tests/golden/reports/*.bin"];

/// The §1.2 lock scope, as path prefixes: a `.gitattributes` declaration
/// outside it cannot classify an artifact these rails may write.
#[cfg(test)]
const LOCK_SCOPE_PREFIXES: &[&str] = &[
    "tests/golden/",
    "crates/dss-core/tests/data/adiakoptics/r3723_ref/",
];

fn repo_root() -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "..", ".."].iter().collect()
}

/// The guarded writer: a lock snapshot plus the lane it may write from.
pub struct Rails {
    root: PathBuf,
    rows: BTreeMap<String, LockRow>,
    lane: Lane,
    /// Paths already announced, so a driver looping over a family does not
    /// print the same refusal once per scenario.
    announced: Mutex<BTreeSet<String>>,
    /// Has the "your lock is now stale" reminder been printed?
    reminded: AtomicBool,
}

impl Rails {
    /// Read the committed lock under `root` and arm the rails for `lane`.
    pub fn load(root: PathBuf, lane: Lane) -> Rails {
        let path = root.join(LOCK_PATH);
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            panic!(
                "cannot read the golden provenance lock {}: {e} — {UPDATE_ENV} cannot be honored \
                 without it (bootstrap with `{LOCK_REGEN_CMD}`)",
                path.display()
            )
        });
        Rails::from_lock_text(root, lane, &text)
    }

    /// Same, from lock text already in hand (the scratch-fixture entry point).
    pub fn from_lock_text(root: PathBuf, lane: Lane, text: &str) -> Rails {
        let doc: LockDoc = serde_json::from_str(text)
            .unwrap_or_else(|e| panic!("cannot parse the golden provenance lock: {e}"));
        let mut rows = BTreeMap::new();
        for row in doc.artifacts {
            let path = row.path.clone();
            if let Some(prev) = rows.insert(path.clone(), row) {
                panic!(
                    "duplicate row for {path} in the golden provenance lock (previous digest {})",
                    prev.sha256
                );
            }
        }
        assert!(
            !rows.is_empty(),
            "the golden provenance lock holds no rows; a vacuous lock would make every \
             snapshot guard accept nothing at all"
        );
        Rails {
            root,
            rows,
            lane,
            announced: Mutex::new(BTreeSet::new()),
            reminded: AtomicBool::new(false),
        }
    }

    pub fn lane(&self) -> Lane {
        self.lane
    }

    /// The whole guard, as a pure decision over the lock row — the acceptance
    /// surface of §1.2 (`Ok` = writable; `Err` = one of the refusals).
    pub fn decide(&self, rel: &str) -> Result<(), Refusal> {
        let Some(row) = self.rows.get(rel) else {
            return Err(Refusal::Unlocked);
        };
        if row.anchor != Anchor::SelfSnapshot {
            return Err(Refusal::ExternallyAnchored(row.anchor));
        }
        match row.produced_by {
            None => Err(Refusal::NoProducingLane),
            Some(producer) if !producer.admits(self.lane) => Err(Refusal::WrongLane {
                producer,
                running: self.lane,
            }),
            Some(_) => Ok(()),
        }
    }

    /// Rewrite `rel` with `bytes` if both guards allow it.
    pub fn write_bytes(&self, rel: &str, bytes: &[u8]) -> Outcome {
        assert_path_shape(rel);
        if !is_binary_artifact(rel) {
            assert!(
                !bytes.contains(&0),
                "snapshot of {rel} carries a NUL byte but the path is text-classified: the lock \
                 would EOL-normalize those bytes before digesting them. Give the artifact a \
                 `binary` .gitattributes declaration and a `.bin` name, or fix the payload."
            );
        }
        match self.decide(rel) {
            Err(refusal) => {
                self.announce(rel, &format!("REFUSED  {}", refusal.explain(rel)));
                Outcome::Refused(refusal)
            }
            Ok(()) => {
                let abs = self.root.join(rel);
                if let Some(parent) = abs.parent() {
                    std::fs::create_dir_all(parent)
                        .unwrap_or_else(|e| panic!("create {}: {e}", parent.display()));
                }
                std::fs::write(&abs, bytes)
                    .unwrap_or_else(|e| panic!("write snapshot {}: {e}", abs.display()));
                self.announce(rel, &format!("SNAPSHOT {rel} ({} bytes)", bytes.len()));
                if !self.reminded.swap(true, Ordering::Relaxed) {
                    eprintln!(
                        "  the provenance lock is now stale ON PURPOSE — review the moved bytes, \
                         then run `{LOCK_REGEN_CMD}` and commit both diffs together"
                    );
                }
                Outcome::Wrote
            }
        }
    }

    /// Rewrite a **text** artifact. Rejects a binary-classified path outright:
    /// that is a call-site mistake (the payload would be EOL-normalized by git
    /// on check-in), not an operator condition.
    pub fn write_text(&self, rel: &str, text: &str) -> Outcome {
        assert!(
            !is_binary_artifact(rel),
            "{rel} is a binary-classified artifact ({LOCK_PATH} digests it raw); write it with \
             snapshot_bytes, not snapshot_text"
        );
        self.write_bytes(rel, text.as_bytes())
    }

    fn announce(&self, rel: &str, line: &str) {
        let mut seen = self.announced.lock().expect("regen announce mutex");
        if seen.insert(rel.to_string()) {
            eprintln!("{UPDATE_ENV}: {line}");
        }
    }
}

/// Repo-relative, forward slashes, no escape hatches — the rails join this onto
/// the repo root, so a `..` or an absolute path would write outside the corpus.
fn assert_path_shape(rel: &str) {
    assert!(
        !rel.is_empty()
            && !rel.starts_with('/')
            && !rel.contains('\\')
            && !rel.contains(':')
            && !rel.split('/').any(|c| c == ".." || c.is_empty()),
        "snapshot path {rel:?} must be repo-root-relative with forward slashes and no `..`"
    );
}

static RAILS: OnceLock<Option<Rails>> = OnceLock::new();

/// The process-wide rails: `Some` iff `DSS_UPDATE_GOLDENS` is set.
///
/// `None` is the normal state — during the mandatory gate every snapshot call
/// site is inert, which is what keeps a driver from blessing its own output.
pub fn regen() -> Option<&'static Rails> {
    RAILS
        .get_or_init(|| {
            // `?`: no knob, no rails — the mandatory-gate state.
            std::env::var_os(UPDATE_ENV)?;
            let rails = Rails::load(repo_root(), Lane::current());
            eprintln!(
                "{UPDATE_ENV} is set: golden self-snapshots may be rewritten from the `{}` lane \
                 ({} locked artifacts). Anchors other than `self` and families produced by the \
                 other lane are refused.",
                rails.lane.as_str(),
                rails.rows.len()
            );
            Some(rails)
        })
        .as_ref()
}

/// Snapshot a text artifact (`tests/golden/...`, repo-relative). Inert unless
/// `DSS_UPDATE_GOLDENS` is set; guarded by the lock's anchor and producing lane.
pub fn snapshot_text(rel: &str, text: &str) -> Outcome {
    snapshot_text_with(regen(), rel, text)
}

/// Snapshot a binary artifact (the `reports/*.bin` streams). Same guards.
pub fn snapshot_bytes(rel: &str, bytes: &[u8]) -> Outcome {
    snapshot_bytes_with(regen(), rel, bytes)
}

/// The body of [`snapshot_text`], with the process-global rails passed in.
///
/// Split out purely so the **armed** public path is testable: the `OnceLock` in
/// [`regen`] is process-global and reads the real repo, so a test driving
/// `snapshot_text` directly could only ever exercise the disarmed branch — and a
/// delegation that skipped `write_text`'s binary-path assert would pass unseen.
fn snapshot_text_with(rails: Option<&Rails>, rel: &str, text: &str) -> Outcome {
    match rails {
        None => Outcome::NotRequested,
        Some(rails) => rails.write_text(rel, text),
    }
}

/// The body of [`snapshot_bytes`]; see [`snapshot_text_with`].
fn snapshot_bytes_with(rails: Option<&Rails>, rel: &str, bytes: &[u8]) -> Outcome {
    match rails {
        None => Outcome::NotRequested,
        Some(rails) => rails.write_bytes(rel, bytes),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A hermetic repo-shaped scratch root: the guards are proven against a
    /// fixture, never against the committed corpus.
    fn scratch(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "golden_regen_{tag}_{}_{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("create scratch root");
        d
    }

    /// Build a mini lock: `(path, anchor, produced_by)`.
    fn mini_lock(rows: &[(&str, &str, Option<&str>)]) -> String {
        let artifacts: Vec<serde_json::Value> = rows
            .iter()
            .map(|(path, anchor, produced_by)| {
                serde_json::json!({
                    "path": path,
                    "sha256": "0".repeat(64),
                    "anchor": anchor,
                    "produced_by": produced_by,
                })
            })
            .collect();
        serde_json::json!({ "comment": "scratch fixture", "artifacts": artifacts }).to_string()
    }

    const ORIGINAL: &[u8] = b"the committed bytes\n";

    /// Seed `rel` under `root` with [`ORIGINAL`] so a refusal can be proven to
    /// leave it untouched.
    fn seed(root: &Path, rel: &str) -> PathBuf {
        let abs = root.join(rel);
        std::fs::create_dir_all(abs.parent().expect("artifact parent")).expect("create dirs");
        std::fs::write(&abs, ORIGINAL).expect("seed artifact");
        abs
    }

    fn read(abs: &Path) -> Vec<u8> {
        std::fs::read(abs).expect("read artifact")
    }

    /// Guard 1: an artifact anchored anywhere but `self` is refused — for
    /// **every** external anchor, so the frozen `capi015` / `fpc_3.2.2` /
    /// `r3723` sets are covered, not just the pinned oracle — and the bytes on
    /// disk do not move.
    #[test]
    fn a_foreign_anchor_is_refused() {
        for (i, anchor) in ["capi_v0145", "r4133", "r3723", "capi015", "fpc_3.2.2"]
            .into_iter()
            .enumerate()
        {
            let root = scratch(&format!("foreign{i}"));
            let rel = "tests/golden/reports/export_x.txt";
            let abs = seed(&root, rel);
            // `produced_by` is null on every oracle-anchored row.
            let rails = Rails::from_lock_text(
                root.clone(),
                Lane::current(),
                &mini_lock(&[(rel, anchor, None)]),
            );

            let outcome = rails.write_text(rel, "freshly rendered bytes\n");

            assert_eq!(
                outcome,
                Outcome::Refused(Refusal::ExternallyAnchored(match anchor {
                    "capi_v0145" => Anchor::CapiV0145,
                    "r4133" => Anchor::R4133,
                    "r3723" => Anchor::R3723,
                    "capi015" => Anchor::Capi015,
                    _ => Anchor::Fpc322,
                })),
                "anchor {anchor} must refuse the write"
            );
            assert_eq!(
                read(&abs),
                ORIGINAL,
                "a refused snapshot moved bytes anyway ({anchor})"
            );
            let _ = std::fs::remove_dir_all(&root);
        }
    }

    /// Guard 2: a `parity`-produced family is refused from the default lane and
    /// accepted from the parity lane. Both lanes are exercised in **both**
    /// builds — the guard is a property of the lock row plus the lane the rails
    /// were armed with, so neither arm can go untested in either gate run.
    #[test]
    fn the_non_producing_lane_is_refused_and_the_producing_lane_writes() {
        let root = scratch("lane");
        let rel = "tests/golden/reports/show_x.txt";
        let abs = seed(&root, rel);
        let lock = mini_lock(&[(rel, "self", Some("parity"))]);

        let default_lane = Rails::from_lock_text(root.clone(), Lane::Default, &lock);
        assert_eq!(
            default_lane.write_text(rel, "default-lane rendering\n"),
            Outcome::Refused(Refusal::WrongLane {
                producer: ProducedBy::Parity,
                running: Lane::Default,
            })
        );
        assert_eq!(
            read(&abs),
            ORIGINAL,
            "the non-producing lane wrote the artifact anyway"
        );

        let parity_lane = Rails::from_lock_text(root.clone(), Lane::Parity, &lock);
        assert_eq!(
            parity_lane.write_text(rel, "parity-lane rendering\n"),
            Outcome::Wrote
        );
        assert_eq!(
            read(&abs),
            b"parity-lane rendering\n",
            "the producing lane must actually rewrite the artifact"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    /// A `lane-invariant` family — the WP-G4 end state, only ever set after a
    /// cross-lane regen measured it — is writable from either lane.
    #[test]
    fn a_lane_invariant_family_accepts_both_lanes() {
        for (tag, lane) in [("li_parity", Lane::Parity), ("li_default", Lane::Default)] {
            let root = scratch(tag);
            let rel = "tests/golden/json/x.json";
            let abs = seed(&root, rel);
            let rails = Rails::from_lock_text(
                root.clone(),
                lane,
                &mini_lock(&[(rel, "self", Some("lane-invariant"))]),
            );
            assert_eq!(rails.write_text(rel, "{}\n"), Outcome::Wrote);
            assert_eq!(read(&abs), b"{}\n");
            let _ = std::fs::remove_dir_all(&root);
        }
    }

    /// An artifact the lock has never seen is refused: writing it would smuggle
    /// an unfingerprinted golden into the corpus.
    #[test]
    fn an_unlocked_path_is_refused() {
        let root = scratch("unlocked");
        let known = "tests/golden/reports/known.txt";
        let unknown = "tests/golden/reports/brand_new.txt";
        seed(&root, known);
        let abs = seed(&root, unknown);
        let rails = Rails::from_lock_text(
            root.clone(),
            Lane::current(),
            &mini_lock(&[(known, "self", Some("parity"))]),
        );

        assert_eq!(
            rails.write_text(unknown, "invented\n"),
            Outcome::Refused(Refusal::Unlocked)
        );
        assert_eq!(read(&abs), ORIGINAL);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A hand-edited `self` row with no producing lane is refused rather than
    /// silently defaulting to one.
    #[test]
    fn a_self_row_without_a_producing_lane_is_refused() {
        let root = scratch("noproducer");
        let rel = "tests/golden/reports/handedited.txt";
        let abs = seed(&root, rel);
        let rails = Rails::from_lock_text(
            root.clone(),
            Lane::current(),
            &mini_lock(&[(rel, "self", None)]),
        );
        assert_eq!(
            rails.write_text(rel, "whatever\n"),
            Outcome::Refused(Refusal::NoProducingLane)
        );
        assert_eq!(read(&abs), ORIGINAL);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The text/binary split matches the lock's digest classification: a `.bin`
    /// stream must go through `snapshot_bytes`, and a text artifact may not
    /// carry NUL bytes (the lock would EOL-normalize them).
    #[test]
    fn the_text_binary_split_matches_the_lock_classification() {
        let root = scratch("binary");
        let bin = "tests/golden/reports/dump3_x.bin";
        let txt = "tests/golden/reports/dump3_x.txt";
        let bin_abs = seed(&root, bin);
        seed(&root, txt);
        let rails = Rails::from_lock_text(
            root.clone(),
            Lane::Parity,
            &mini_lock(&[(bin, "self", Some("parity")), (txt, "self", Some("parity"))]),
        );

        // Raw stream through the byte helper: written verbatim, NUL included.
        assert_eq!(rails.write_bytes(bin, &[0x00, 0x01, 0xff]), Outcome::Wrote);
        assert_eq!(read(&bin_abs), vec![0x00, 0x01, 0xff]);

        // The same path through the text helper is a call-site error.
        let wrong_helper =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| rails.write_text(bin, "x")));
        assert!(
            wrong_helper.is_err(),
            "a binary-classified path must not be writable as text"
        );

        // A text path may not carry a NUL payload.
        let nul = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            rails.write_bytes(txt, &[b'a', 0x00])
        }));
        assert!(
            nul.is_err(),
            "a NUL byte in a text-classified artifact must fail loudly"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    /// Path shapes that would escape the corpus are rejected before any guard
    /// runs.
    #[test]
    fn escaping_paths_are_rejected() {
        for bad in [
            "",
            "/tests/golden/x.txt",
            "tests\\golden\\x.txt",
            "tests/golden/../../x.txt",
            "C:/tests/golden/x.txt",
        ] {
            assert!(
                std::panic::catch_unwind(|| assert_path_shape(bad)).is_err(),
                "path {bad:?} must be rejected"
            );
        }
        assert_path_shape("tests/golden/reports/export_x.txt");
    }

    /// The committed lock parses into this module's mirror of the schema, and
    /// the guards agree with it on real rows. This is the drift binder against
    /// `golden_lock.rs`: a renamed anchor value, a new row field or a changed
    /// `produced_by` spelling fails here.
    #[test]
    fn the_committed_lock_parses_into_this_mirror() {
        let rails = Rails::load(repo_root(), Lane::Parity);
        assert!(
            rails.rows.len() > 700,
            "the committed lock holds only {} rows — this binder went vacuous",
            rails.rows.len()
        );

        // A born-self row is writable from its producing lane...
        let born_self = "tests/golden/json/schema_full_port.json";
        assert_eq!(
            rails.rows.get(born_self).map(|r| r.anchor),
            Some(Anchor::SelfSnapshot),
            "{born_self} is expected to be the born-self schema document"
        );
        assert_eq!(rails.decide(born_self), Ok(()));
        // ...and refused from the other one.
        let default_rails = Rails::from_lock_text(
            repo_root(),
            Lane::Default,
            &std::fs::read_to_string(repo_root().join(LOCK_PATH)).expect("read lock"),
        );
        assert_eq!(
            default_rails.decide(born_self),
            Err(Refusal::WrongLane {
                producer: ProducedBy::Parity,
                running: Lane::Default,
            })
        );

        // Every non-self row in the real corpus is refused, in either lane —
        // and there are many of them (non-vacuity of guard 1 over the corpus).
        let mut external = 0usize;
        for (path, row) in &rails.rows {
            if row.anchor == Anchor::SelfSnapshot {
                assert!(
                    row.produced_by.is_some(),
                    "{path}: a self row must name a producing lane"
                );
                continue;
            }
            external += 1;
            assert_eq!(
                rails.decide(path),
                Err(Refusal::ExternallyAnchored(row.anchor)),
                "{path} is anchored {} and must be unwritable",
                row.anchor.as_str()
            );
        }
        assert!(
            external > 700,
            "only {external} externally anchored rows — the corpus or this binder moved"
        );
    }

    /// The rails read the same lane switch as the rest of the harness.
    #[test]
    fn lane_current_tracks_the_build() {
        assert_eq!(
            Lane::current(),
            if super::super::lane::PARITY {
                Lane::Parity
            } else {
                Lane::Default
            },
            "the regen rails and the lane policy disagree about the build"
        );
    }

    /// The **armed** public entry points, driven over a scratch fixture: the
    /// same two guard outcomes plus `write_text`'s binary-path rejection, this
    /// time through `snapshot_text`/`snapshot_bytes`' own delegation rather than
    /// through `Rails` directly. Without this, a future edit routing
    /// `snapshot_text` straight at `write_bytes` (skipping that assert) would
    /// pass the whole suite.
    #[test]
    fn the_armed_public_helpers_delegate_through_the_guards() {
        let root = scratch("armed");
        let writable = "tests/golden/reports/show_armed.txt";
        let refused = "tests/golden/reports/export_armed.txt";
        let stream = "tests/golden/reports/dump3_armed.bin";
        let writable_abs = seed(&root, writable);
        let refused_abs = seed(&root, refused);
        let stream_abs = seed(&root, stream);
        let rails = Rails::from_lock_text(
            root.clone(),
            Lane::Parity,
            &mini_lock(&[
                (writable, "self", Some("parity")),
                (refused, "capi_v0145", None),
                (stream, "self", Some("parity")),
            ]),
        );
        let armed = Some(&rails);

        assert_eq!(
            snapshot_text_with(armed, writable, "armed rendering\n"),
            Outcome::Wrote
        );
        assert_eq!(read(&writable_abs), b"armed rendering\n");

        assert_eq!(
            snapshot_text_with(armed, refused, "armed rendering\n"),
            Outcome::Refused(Refusal::ExternallyAnchored(Anchor::CapiV0145))
        );
        assert_eq!(read(&refused_abs), ORIGINAL);

        assert_eq!(
            snapshot_bytes_with(armed, stream, &[0x00, 0x7f]),
            Outcome::Wrote
        );
        assert_eq!(read(&stream_abs), vec![0x00, 0x7f]);

        // The text helper still rejects a binary-classified path after the
        // delegation — the assert lives in `write_text`, which it must go
        // through.
        let wrong_helper = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            snapshot_text_with(armed, stream, "x")
        }));
        assert!(
            wrong_helper.is_err(),
            "snapshot_text must not reach a binary-classified path"
        );

        // Disarmed, the same calls touch nothing.
        assert_eq!(
            snapshot_text_with(None, writable, "ignored\n"),
            Outcome::NotRequested
        );
        assert_eq!(read(&writable_abs), b"armed rendering\n");

        let _ = std::fs::remove_dir_all(&root);
    }

    /// The binder for this module's copy of `is_binary_artifact`: it must agree
    /// with the `.gitattributes` declaration `golden_lock.rs::is_binary_artifact`
    /// is bound to, over every path the committed lock knows — so the two copies
    /// cannot drift apart through their shared source of truth.
    #[test]
    fn the_binary_classifier_matches_gitattributes() {
        let root = repo_root();
        let attrs =
            std::fs::read_to_string(root.join(".gitattributes")).expect("read .gitattributes");
        let declared: Vec<String> = attrs
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .filter_map(|l| {
                let mut f = l.split_whitespace();
                let pattern = f.next()?;
                let binary = f.any(|a| a == "binary" || a == "-text");
                // Only declarations that can classify a locked artifact: the
                // §1.2 lock scope (tests/golden/** plus the registered
                // out-of-tree r3723 witness). The vendored corpus's `-text`
                // pathspecs classify nothing this module can write.
                let in_scope = LOCK_SCOPE_PREFIXES.iter().any(|r| pattern.starts_with(r));
                (binary && in_scope).then(|| pattern.to_string())
            })
            .collect();
        assert_eq!(
            declared, GITATTRIBUTES_BINARY_PATHSPECS,
            "the `binary` declarations over the locked roots moved. is_binary_artifact() here and \
             in golden_lock.rs both emulate them, so teach BOTH copies the new family (and this \
             register) rather than letting one keep hashing/guarding it as text."
        );

        // Both directions over the real corpus: every locked path is classified
        // binary by this module exactly when .gitattributes declares it so.
        let rails = Rails::load(root, Lane::Parity);
        let mut binary_rows = 0usize;
        for rel in rails.rows.keys() {
            let attr_binary = GITATTRIBUTES_BINARY_PATHSPECS.iter().any(|p| {
                let (dir, base) = p.rsplit_once('/').expect("pathspec has a directory");
                let suffix = base
                    .strip_prefix('*')
                    .expect("pathspec shape <dir>/*<suffix>");
                rel.rsplit_once('/')
                    .is_some_and(|(d, b)| d == dir && b.ends_with(suffix))
            });
            assert_eq!(
                is_binary_artifact(rel),
                attr_binary,
                "{rel}: the rails classify it {}, .gitattributes says {attr_binary}",
                is_binary_artifact(rel)
            );
            binary_rows += usize::from(attr_binary);
        }
        assert!(
            binary_rows > 0,
            "no locked artifact is binary-classified — this binder went vacuous"
        );
    }

    /// Without the knob the helpers are inert — the property that keeps a
    /// driver from blessing its own output during the mandatory gate.
    #[test]
    fn the_helpers_are_inert_without_the_knob() {
        let armed = std::env::var_os(UPDATE_ENV).is_some();
        assert_eq!(
            regen().is_some(),
            armed,
            "the rails must be armed exactly when {UPDATE_ENV} is set"
        );
        if !armed {
            assert_eq!(
                snapshot_text("tests/golden/reports/nonexistent.txt", "x"),
                Outcome::NotRequested
            );
            assert_eq!(
                snapshot_bytes("tests/golden/reports/nonexistent.bin", b"x"),
                Outcome::NotRequested
            );
        }
    }
}
