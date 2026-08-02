//! Provenance lock over the committed golden corpus — `tests/golden/**` plus the
//! one registered out-of-tree witness ([`ROOTS`]), the scope
//! `GOLDEN_REBASE_PLAN.md` §1.2 enumerates; golden-shaped trees deliberately
//! left outside it are named, with their reason, in [`EXCLUDED_TREES`]. The
//! golden-side twin of [`population_lock`](../population_lock.rs)
//! (sub-step G0.1).
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
//! 3. **Every digest matches** — a *committed* golden byte cannot move without
//!    the lock diff moving with it, in the same reviewed commit. "Committed" is
//!    load-bearing: digests are taken over the git blob (CRLF→LF for text, see
//!    [`digest_of`]), so a change that flips only a text artifact's line endings
//!    in the working tree moves no digest — and moves no committed byte either,
//!    because git's check-in filter normalizes it away. Recording the working
//!    tree's EOL shape instead would make the lock fail on an LF checkout, which
//!    is the exact failure the normalization exists to prevent. Consequence for
//!    WP-G4: the `JSON_LINE_BREAK` teardown (G4.3) cannot use the lock diff as
//!    its only review artifact — an EOL-only rendering change is invisible here
//!    and must be reviewed against the renderer and its unit pins.
//! 4. **Every anchor is a registered decision, both directions.** The provenance
//!    registers below — [`DEANCHORED`], [`CAPI015_ARTIFACTS`], [`R4133_FAMILIES`],
//!    [`FPC_ARTIFACT`], [`R3723_TREE`], with `capi_v0145` as the residue — are
//!    the invariant: every row's `anchor` and `reason` must equal what
//!    [`seed_metadata`] derives for its path, and every register entry must cover
//!    at least one locked row (fail-on-stale in both directions, exactly like
//!    `oracle_parity_cfg_gate.rs::ESCAPE_REGISTER`). Re-anchoring an artifact is
//!    therefore a reviewable *code* edit in a register, never a lock hand-edit
//!    and never a side effect of a regen run. Additionally `anchor == self` holds
//!    if and only if `produced_by` is set.
//!
//! # Regenerate deliberately
//!
//! ```text
//! DSS_UPDATE_GOLDEN_LOCK=1 cargo test -p dss-core --test golden_lock
//! ```
//!
//! recomputes every digest from the artifacts currently on disk and rewrites the
//! lock. Provenance is re-derived from the registers via [`seed_metadata`] —
//! never carried over from the stored row — so this knob can move digests but
//! cannot invent, preserve or launder an anchor: a hand-edited one is reset
//! (loudly, `RE-ANCHORED …` on stderr), a hand-edited unregistered `self` is
//! refused outright, and a path no register recognizes is announced (`SEEDED …`)
//! instead of silently acquiring the `capi_v0145` residue. Only the *measured*
//! `produced_by` of a `self` row survives a regen. Run it after reviewing *why*
//! the bytes moved, and commit the lock diff together with the change that
//! caused it.

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

/// Committed golden-shaped trees deliberately **outside** [`ROOTS`]. §1.2 fixes
/// the lock's scope by enumeration, so anything golden-shaped that the
/// enumeration leaves out is recorded here with its reason — a reviewed
/// exclusion rather than an oversight, which G3.6 re-confirms when it extends
/// the reason requirement to every row. Fail-on-stale: the path must still
/// exist and must still be outside [`ROOTS`].
const EXCLUDED_TREES: &[(&str, &str)] = &[(
    "crates/dss-metis/tests/golden",
    "the METIS partitioner fixtures (.graph inputs + .part.N outputs, regenerated manually per \
     tools/golden/gen_metis_reference.md) are captured from the METIS 5.2.1 C original, not from \
     any DSS oracle: they witness a vendored third-party algorithm, no WP of this plan \
     regenerates them, and anchoring them would need a seventh anchor value GOLDEN_REBASE_PLAN.md \
     \u{a7}1.2 does not define. Revisit in G3.6.",
)];

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
/// The first two entries are **born-`self`**: they were never oracle-anchored,
/// so nothing was de-anchored to create them. The two `props/` entries were
/// de-anchored by the WP-U2 r4133 control rewrites, years before this lock
/// existed — they are recorded here because their own provenance blocks say the
/// committed values are the port's renders, and an anchor that contradicts the
/// artifact is the one failure this lock exists to prevent. WP-G3 adds the
/// migrated families here, one reviewed row at a time.
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
    (
        "tests/golden/props/recloser.json",
        "de-anchored by the WP-U2.2 r4133 Recloser rewrite (24 -> 46 props), long before this \
         lock: no capi-line engine renders that surface, so the committed values are the PORT's \
         own renders and props_roundtrip compares Rust output back to them (the file's own note \
         says so) — a self-consistency regression pin. SwitchedObj/RecloseIntervals/Enabled/\
         EventLog were cross-validated MANUALLY (non-CI) against oddie:r4133; a regen must repeat \
         that cross-validation. Independent r4133 coverage lives in the live controls-family \
         decks.",
    ),
    (
        "tests/golden/props/relay.json",
        "de-anchored by the WP-U2.3 r4133 Relay per-phase rewrite (50 -> 71 props), long before \
         this lock: no capi-line engine renders that surface, so the committed values are the \
         PORT's own renders — the file's own note calls it a REGRESSION PIN (self-consistency), \
         explicitly NOT an independent oracle gate. The load-bearing discrete state \
         (Normal/State) plus SwitchedObj/RecloseIntervals/Enabled were cross-validated MANUALLY \
         (non-CI) against oddie:r4133; a regen must repeat that cross-validation. Independent \
         r4133 coverage lives in the live controls-family relay decks.",
    ),
];

/// The eleven artifacts whose provenance block declares `capi015` — spelled
/// `"engine_spec": "capi015"` in the seven `props/` + `line_constants/` dumps
/// and `"oracle": "capi015"` in the four `.meta.json` sidecars (measured; a
/// derivation from `engine_spec` alone would drop those four). Plan G0.1
/// enumerates the same eleven. Their generator environment — a dss_capi 0.15.0b4
/// / DSS-Python 0.16.0b2 beta stack — no longer exists, so they can never be
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

/// Artifacts whose value authority is the official EPRI r4133 engine rather than
/// the pinned capi oracle. An entry is a directory prefix (`.../`) or an exact
/// path, same convention as [`DEANCHORED`]. They stay externally anchored through
/// WP-G3 (§1.2: they are *not* frozen-`capi_v0145`; their disposition is recorded
/// here and re-confirmed in G3.6).
const R4133_FAMILIES: &[(&str, &str)] = &[
    (
        "tests/golden/flicker/",
        "the monitor mode-4 IEC 61000-4-15 Pst gate (the Pstcalc command results are a separate \
         family, pstcalc/). The COMMITTED bytes are the frozen capture from the official EPRI \
         r3723 binary via the retired Oddie bridge, which the artifact's own oracle block records; \
         the anchor is r4133 because that is the only surviving regeneration path \
         (tools/golden/gen_flicker.py, epri-worker) and its payload reproduces the r3723 capture \
         byte-identically (proven, STATUS 'EPRI bridge parity round'). The pinned 0.14.5 oracle \
         cannot produce this family at all (its DoFlickerCalculations segfaults).",
    ),
    (
        "tests/golden/props/fuse.json",
        "the WP-U2.1 r4133 Fuse surface (RatedCurrent -> informational, new CurveMultiplier / \
         InterruptingRating, default FuseCurve tlink -> none, props 10 -> 12), which NO capi-line \
         engine has (tools/golden/gen_fuse_r4133.py:11) — the artifact declares engine_spec \
         \"r4133\" itself. The bytes are DERIVED: the retired 0.14.5 dump supplies the rendering \
         the port reproduces bit-for-bit, and every r4133 value delta overlaid on it was verified \
         on the official EPRI r4133 engine (Oddie). The value authority, hence the anchor, is \
         r4133.",
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

const COMMENT: &str = "Provenance lock over the committed golden corpus (GOLDEN_REBASE_PLAN.md \
\u{a7}1.2, G0.1): tests/golden/** plus the registered out-of-tree witness \
crates/dss-core/tests/data/adiakoptics/r3723_ref/. That enumeration is the scope; golden-shaped \
trees left outside it (today: crates/dss-metis/tests/golden, the vendored METIS 5.2.1 fixtures) are \
named with their reason in golden_lock.rs::EXCLUDED_TREES and revisited in G3.6. Each row records \
the artifact's content digest and where the truth in those bytes comes from (anchor), plus - for \
self-anchored rows - a mandatory reason and the lane allowed to (re)produce them. \
crates/dss-core/tests/golden_lock.rs asserts, fail-on-stale in both directions: every artifact has a \
row, every row an artifact, every digest matches, and every row's anchor+reason are exactly what the \
test's provenance registers (DEANCHORED / CAPI015_ARTIFACTS / R4133_FAMILIES / FPC_ARTIFACT / \
R3723_TREE, capi_v0145 as the residue) derive for its path - so re-anchoring an artifact is a \
reviewed edit to a register, never a hand-edit here. Digests are taken over the COMMITTED content: \
CRLF is normalized to LF for text artifacts (core.autocrlf=true here, so the working tree carries \
CRLF while git stores LF), while reports/*.bin streams - `binary` in .gitattributes, cross-checked \
against that file - are hashed raw; an EOL-only working-tree change therefore moves no digest \
because it moves no committed byte (see the golden_lock.rs module docs, assertion 3, for what that \
means for G4.3). Regenerate DELIBERATELY with \
`DSS_UPDATE_GOLDEN_LOCK=1 cargo test -p dss-core --test golden_lock`; it moves digests only - \
provenance is re-derived from the registers, and every re-anchored or newly seeded path is announced \
on stderr - and the diff is the review artifact. The regeneration rules R1-R4 and the operational \
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

/// **The** anchor classification: the provenance registers applied to a path.
/// This is not a seeding heuristic — it is the invariant every locked row is
/// checked against and the only thing a regen writes, so re-anchoring an
/// artifact means editing a register above. The residue is the pinned oracle,
/// which is what all but the enumerated `tools/golden/gen_*.py` generators
/// captured.
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
///
/// This predicate **emulates** that declaration, so the two must not drift: a
/// `.bin` added outside `reports/` would be EOL-normalized by git but hashed raw
/// here, and a binary family with another extension would be hashed
/// EOL-normalized although git stores it raw — either way the digest silently
/// stops being over the committed content. [`assert_binary_classification_matches_gitattributes`]
/// binds the two, both directions, over every scanned path.
fn is_binary_artifact(rel: &str) -> bool {
    rel.ends_with(".bin")
}

/// The pathspecs `.gitattributes` marks `binary` (or `-text`) **inside**
/// [`ROOTS`] — the declarations [`is_binary_artifact`] emulates. Patterns
/// outside the locked roots (the vendored corpus) cannot classify a golden and
/// are dropped.
fn gitattributes_binary_patterns(root: &Path) -> Vec<String> {
    let path = root.join(".gitattributes");
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut fields = line.split_whitespace();
        let Some(pattern) = fields.next() else {
            continue;
        };
        if !fields.any(|a| a == "binary" || a == "-text") {
            continue;
        }
        if ROOTS.iter().any(|r| pattern.starts_with(r)) {
            out.push(pattern.to_string());
        }
    }
    out
}

/// Match one `.gitattributes` pathspec against a repo-relative path. Only the
/// two shapes the file actually uses are supported — a `dir/**` subtree and a
/// `dir/<glob>` leaf with at most one `*` — and
/// [`assert_binary_classification_matches_gitattributes`] rejects anything else
/// rather than silently under-matching it.
fn attr_pattern_matches(pattern: &str, path: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix("/**") {
        return path.starts_with(prefix) && path[prefix.len()..].starts_with('/');
    }
    let (Some((dir, base)), Some((pdir, pbase))) =
        (pattern.rsplit_once('/'), path.rsplit_once('/'))
    else {
        return false;
    };
    if dir != pdir {
        return false;
    }
    match base.split_once('*') {
        Some((head, tail)) => {
            pbase.len() >= head.len() + tail.len()
                && pbase.starts_with(head)
                && pbase.ends_with(tail)
        }
        None => base == pbase,
    }
}

/// Bind [`is_binary_artifact`] to the `.gitattributes` declaration it emulates,
/// both directions, over every scanned artifact.
fn assert_binary_classification_matches_gitattributes(
    root: &Path,
    paths: &BTreeMap<String, String>,
) {
    let patterns = gitattributes_binary_patterns(root);
    assert!(
        !patterns.is_empty(),
        ".gitattributes declares no `binary`/`-text` pathspec inside {ROOTS:?}, but \
         is_binary_artifact() classifies `.bin` artifacts as binary — the two have drifted"
    );
    for p in &patterns {
        let body = p.strip_suffix("/**").unwrap_or(p);
        assert!(
            body.matches('*').count() <= 1 && !body.contains('?') && !body.contains('['),
            ".gitattributes pathspec {p:?} uses a glob shape attr_pattern_matches() does not \
             implement; teach it that shape rather than letting the match silently fail"
        );
    }
    for rel in paths.keys() {
        let declared = patterns.iter().any(|p| attr_pattern_matches(p, rel));
        assert_eq!(
            is_binary_artifact(rel),
            declared,
            "golden artifact {rel}: is_binary_artifact() says {}, .gitattributes says {} \
             ({patterns:?}). The digest would be taken over bytes git does not store; keep the \
             predicate and the attribute in lockstep.",
            is_binary_artifact(rel),
            declared
        );
    }
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
    assert_binary_classification_matches_gitattributes(root, &out);
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

/// What a regen did to a row's provenance, so the knob can announce it instead
/// of writing a provenance claim nobody reviewed.
enum Seeded {
    /// The path is new to the lock: the registers classified it, and the
    /// `capi_v0145` residue in particular is a claim that wants confirming.
    New(Anchor),
    /// The stored row disagreed with the registers and was reset to them.
    ReAnchored(Anchor, Anchor),
}

/// Rebuild the row set from disk. Provenance always comes from
/// [`seed_metadata`] — the registers are the invariant, so a regen can neither
/// invent an anchor nor preserve a hand-edited one; only the *measured*
/// `produced_by` of a `self` row survives. Refuses outright to write an
/// unregistered `self` anchor: de-anchoring is a [`DEANCHORED`] edit, never a
/// regen side effect.
fn regenerate(
    disk: &BTreeMap<String, String>,
    stored: &BTreeMap<String, Artifact>,
    report: &mut Vec<(String, Seeded)>,
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
            let (anchor, reason, seed_produced_by) = seed_metadata(path);
            let prev = stored.get(path);
            if let Some(prev) = prev {
                assert!(
                    prev.anchor != Anchor::SelfSnapshot || anchor == Anchor::SelfSnapshot,
                    "{path} is anchored `self` in the lock but no DEANCHORED entry covers it. \
                     De-anchoring is a reviewed register edit, not a regen side effect — add the \
                     family (with its reason) to DEANCHORED in golden_lock.rs first."
                );
                if prev.anchor != anchor {
                    report.push((path.clone(), Seeded::ReAnchored(prev.anchor, anchor)));
                }
            } else {
                report.push((path.clone(), Seeded::New(anchor)));
            }
            // Only the measured lane of a self row survives; everything else is
            // the register's.
            let produced_by = match anchor {
                Anchor::SelfSnapshot => Some(
                    prev.and_then(|a| a.produced_by)
                        .unwrap_or(ProducedBy::Parity),
                ),
                _ => seed_produced_by,
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
        let mut report = Vec::new();
        let artifacts = regenerate(&disk, &stored, &mut report);
        std::fs::write(&path, serialize_lock(&artifacts))
            .unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
        eprintln!(
            "golden lock regenerated: {} ({} artifacts)",
            path.display(),
            artifacts.len()
        );
        // Provenance movements are never silent: a new path inherits the
        // `capi_v0145` residue only because no register claimed it, which is a
        // claim about where its bytes came from and must be confirmed by a human.
        for (p, what) in &report {
            match what {
                Seeded::New(a) => eprintln!(
                    "  SEEDED {p} as {a:?} — confirm this artifact really came from that source; \
                     if not, add it to the matching register in golden_lock.rs and regenerate"
                ),
                Seeded::ReAnchored(from, to) => {
                    eprintln!("  RE-ANCHORED {p}: {from:?} -> {to:?} (the registers are the truth)")
                }
            }
        }
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

    // (4) every anchor is a registered decision, both directions.
    let mut register_hits = vec![0usize; DEANCHORED.len()];
    let mut capi015_hits = vec![0usize; CAPI015_ARTIFACTS.len()];
    let mut r4133_hits = vec![0usize; R4133_FAMILIES.len()];
    let mut fpc_hits = 0usize;
    let mut r3723_hits = 0usize;
    for row in &lock.artifacts {
        // The registers own `anchor` and `reason` — a lock hand-edit cannot
        // claim a provenance no register derives, and a register entry that
        // stopped matching the corpus (a renamed or deleted artifact) is caught
        // by the stale sweep below.
        let (want_anchor, want_reason, _) = seed_metadata(&row.path);
        if row.anchor != want_anchor {
            diff.push_str(&format!(
                "  WRONG ANCHOR: {} is locked {:?} but the provenance registers in \
                 golden_lock.rs classify it {:?}. Re-anchoring an artifact is a reviewed register \
                 edit (DEANCHORED / CAPI015_ARTIFACTS / R4133_FAMILIES / FPC_ARTIFACT / \
                 R3723_TREE), never a lock hand-edit — and the register entry must state where \
                 those bytes really came from.\n",
                row.path, row.anchor, want_anchor
            ));
        } else if row.reason != want_reason {
            diff.push_str(&format!(
                "  REASON OUT OF SYNC: {} does not carry its register's reason (regenerate with \
                 DSS_UPDATE_GOLDEN_LOCK=1 after editing the register)\n",
                row.path
            ));
        }
        if CAPI015_ARTIFACTS.contains(&row.path.as_str()) {
            let i = CAPI015_ARTIFACTS
                .iter()
                .position(|p| *p == row.path)
                .expect("checked by contains");
            capi015_hits[i] += 1;
        }
        for (i, (pattern, _)) in R4133_FAMILIES.iter().enumerate() {
            if pattern_covers(pattern, &row.path) {
                r4133_hits[i] += 1;
            }
        }
        if row.path == FPC_ARTIFACT {
            fpc_hits += 1;
        }
        if row.path.starts_with(R3723_TREE) {
            r3723_hits += 1;
        }

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
        // "every self row carries a non-empty reason" needs no separate check:
        // the reason check above pins it to its DEANCHORED entry, and
        // `provenance_registers_are_well_formed` rejects an empty entry.
        //
        // `anchor == self` <=> the artifact is (re)producible in-repo.
        if is_self != row.produced_by.is_some() {
            diff.push_str(&format!(
                "  PRODUCED_BY MISMATCH: {} is anchored {:?} with produced_by {:?}; exactly the \
                 self-anchored rows name a producing lane\n",
                row.path, row.anchor, row.produced_by
            ));
        }
    }
    // Fail-on-stale for every register, not just DEANCHORED: an entry that
    // covers no locked row is a claim about an artifact that no longer exists
    // (renamed, deleted by a later WP, or mistyped), and it must go loud rather
    // than shrink a closed set into a lie.
    for (i, (pattern, _)) in DEANCHORED.iter().enumerate() {
        if register_hits[i] == 0 {
            diff.push_str(&format!(
                "  STALE DEANCHORED ENTRY: {pattern:?} covers no locked artifact\n"
            ));
        }
    }
    for (i, p) in CAPI015_ARTIFACTS.iter().enumerate() {
        if capi015_hits[i] == 0 {
            diff.push_str(&format!(
                "  STALE CAPI015_ARTIFACTS ENTRY: {p:?} matches no locked artifact\n"
            ));
        }
    }
    for (i, (pattern, _)) in R4133_FAMILIES.iter().enumerate() {
        if r4133_hits[i] == 0 {
            diff.push_str(&format!(
                "  STALE R4133_FAMILIES ENTRY: {pattern:?} covers no locked artifact\n"
            ));
        }
    }
    if fpc_hits == 0 {
        diff.push_str(&format!(
            "  STALE FPC_ARTIFACT: {FPC_ARTIFACT:?} matches no locked artifact\n"
        ));
    }
    if r3723_hits == 0 {
        diff.push_str(&format!(
            "  STALE R3723_TREE: {R3723_TREE:?} covers no locked artifact\n"
        ));
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
/// reason per artifact" rule unenforceable — and, since
/// [`golden_lock_matches_the_committed_artifacts`] now checks every row against
/// [`seed_metadata`], would let the register *order* rather than the evidence
/// decide an anchor. The corpus-facing direction (every entry covers a locked
/// row) lives in that test's stale sweep.
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
        "the capi015 set is closed (GOLDEN_REBASE_PLAN.md G0.1 enumerates all eleven) and never \
         grows — that beta environment is gone. A deletion is caught corpus-side by the STALE \
         CAPI015_ARTIFACTS sweep, which is what keeps this count from guarding a constant against \
         itself; shrink both together, deliberately."
    );

    // The classification must be unambiguous: no artifact may fall into two
    // registers at once, or `seed_metadata`'s order — not the evidence — would
    // decide its anchor.
    for (i, (pattern, reason)) in R4133_FAMILIES.iter().enumerate() {
        assert!(
            !pattern.starts_with('/') && !pattern.contains('\\'),
            "R4133_FAMILIES pattern {pattern:?} must be repo-root-relative with forward slashes"
        );
        assert!(
            ROOTS.iter().any(|r| pattern.starts_with(r)),
            "R4133_FAMILIES pattern {pattern:?} is outside the locked roots {ROOTS:?}"
        );
        assert!(
            !reason.trim().is_empty(),
            "R4133_FAMILIES entry {pattern:?} has no reason"
        );
        for (other, _) in R4133_FAMILIES.iter().skip(i + 1) {
            assert!(
                !pattern_covers(pattern, other) && !pattern_covers(other, pattern),
                "R4133_FAMILIES entries {pattern:?} and {other:?} overlap"
            );
        }
        assert!(
            !CAPI015_ARTIFACTS.iter().any(|p| pattern_covers(pattern, p)),
            "r4133 entry {pattern:?} overlaps the capi015 set"
        );
        assert!(
            !DEANCHORED
                .iter()
                .any(|(d, _)| pattern_covers(d, pattern) || pattern_covers(pattern, d)),
            "r4133 entry {pattern:?} overlaps the DEANCHORED register"
        );
        assert!(
            !pattern_covers(pattern, FPC_ARTIFACT) && !pattern.starts_with(R3723_TREE),
            "r4133 entry {pattern:?} overlaps another register"
        );
    }
    assert!(!CAPI015_ARTIFACTS.contains(&FPC_ARTIFACT));
    assert!(!CAPI015_ARTIFACTS.iter().any(|p| p.starts_with(R3723_TREE)));
    assert!(
        !DEANCHORED
            .iter()
            .any(|(d, _)| pattern_covers(d, FPC_ARTIFACT) || d.starts_with(R3723_TREE))
    );
    assert!(R3723_TREE.ends_with('/'));

    // The named out-of-scope trees must still be there and still be out of
    // scope; a stale exclusion is a scope claim nobody re-read.
    for (tree, reason) in EXCLUDED_TREES {
        assert!(
            !reason.trim().is_empty(),
            "EXCLUDED_TREES entry {tree:?} has no reason"
        );
        assert!(
            !ROOTS
                .iter()
                .any(|r| tree.starts_with(r) || r.starts_with(tree)),
            "EXCLUDED_TREES entry {tree:?} is inside the locked roots {ROOTS:?} — it is covered, \
             not excluded"
        );
        assert!(
            repo_root().join(tree).is_dir(),
            "EXCLUDED_TREES entry {tree:?} no longer exists; drop the exclusion (or fix the path) \
             rather than leaving a scope note about nothing"
        );
    }
}

/// The `.gitattributes` matcher is the bridge between [`is_binary_artifact`] and
/// git's own classification, so its two supported glob shapes are pinned here —
/// a matcher that silently fails to match would hand every artifact the "text"
/// digest.
#[test]
fn gitattributes_matcher_handles_the_declared_shapes() {
    let bin = "tests/golden/reports/*.bin";
    assert!(attr_pattern_matches(bin, "tests/golden/reports/x.bin"));
    assert!(!attr_pattern_matches(bin, "tests/golden/reports/x.txt"));
    // Subdirectories are NOT matched by a leaf glob — git's `*` stops at `/`.
    assert!(!attr_pattern_matches(bin, "tests/golden/reports/sub/x.bin"));
    assert!(!attr_pattern_matches(bin, "tests/golden/json/x.bin"));

    let tree = "tests/corpus/electricdss-tst/**";
    assert!(attr_pattern_matches(
        tree,
        "tests/corpus/electricdss-tst/a/b.dss"
    ));
    assert!(!attr_pattern_matches(tree, "tests/corpus/electricdss-tst"));
    assert!(!attr_pattern_matches(tree, "tests/corpus/other/b.dss"));

    let exact = "tests/golden/SHA256SUMS";
    assert!(attr_pattern_matches(exact, "tests/golden/SHA256SUMS"));
    assert!(!attr_pattern_matches(exact, "tests/golden/SHA256SUMS.bak"));
}
