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
//!   every oracle-anchored row (nothing in this repo produces those bytes). For
//!   a `self` row the [`LANE_INVARIANT`] register decides it: `lane-invariant`
//!   for a listed family, `parity` for every other one. Until WP-G4 the two
//!   lanes render different report/JSON bytes, so §1.2 makes the **parity**
//!   lane the producer of any family with a parity-only byte arm; a family joins
//!   `LANE_INVARIANT` only after a cross-lane regen has *measured* it (never
//!   assumed), which is WP-G4's closing job (G4.6). `harness::snapshot_*`
//!   refuses to write from the non-producing lane, so this field is what keeps
//!   a parity-produced golden out of the default lane's reach.
//!
//! # What this test asserts
//!
//! 1. **Every artifact on disk has a row** — a new golden cannot be smuggled in
//!    unfingerprinted.
//! 2. **Every row has an artifact on disk** — fail-on-stale. Deleting a
//!    `props/` class file therefore reds here, beside the counts
//!    `props_roundtrip.rs` pins itself (`PROPS_CLASS_FILES`, `PROPS_SCENARIOS`,
//!    `PROPS_PROPERTY_CELLS`).
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
//!    if and only if `produced_by` is set, and a `self` row's `produced_by` is
//!    the one [`LANE_INVARIANT`] derives. Two reason-only registers,
//!    [`NON_ENGINE_RESIDUE`] and [`CAPI_V0145_OVERLAYS`], write a truthful
//!    reason into a residue row without moving its anchor. Two corpus-side
//!    rails decide [`CAPI015_ARTIFACTS`] from the artifacts' own evidence: the
//!    stamp rail ([`capi015_stamp_violations`]) keeps it equal, both
//!    directions, to the artifacts whose provenance block declares capi015, and
//!    the sidecar↔payload rail ([`capi015_sidecar_violations`]) keeps every
//!    payload a capi015 generator call wrote next to its `.meta.json` in it —
//!    the residue can no longer swallow a capi015 capture, stamped or not.
//! 5. **Every declared provenance admits the locked anchor.** A JSON artifact
//!    whose own provenance block names an engine (`engine_spec`, `engine`,
//!    `oracle`) must be anchored to that engine ([`declared_provenance_violations`]),
//!    so an artifact no register names cannot take the `capi_v0145` residue
//!    against its own declaration. `self` rows are exempt (their [`DEANCHORED`]
//!    reason says why the declared engine is no longer the value authority), and
//!    [`DECLARED_PROVENANCE_EXCEPTIONS`] holds the reasoned exceptions. A value
//!    naming several engines counts only when the artifact's own `engine_spec`
//!    picks one of them, an `oracle` block without an engine name reds, and
//!    [`DECLARED_PROVENANCE_CENSUS`] pins the declaring population, so a reader
//!    that stops seeing a declaration reds as well.
//!
//! # Regenerate deliberately
//!
//! ```text
//! DSS_UPDATE_GOLDEN_LOCK=1 cargo test -p dss-core --test golden_lock -- --nocapture
//! ```
//!
//! ([`LOCK_REGEN_CMD`]) recomputes every digest from the artifacts currently on
//! disk and rewrites the lock. Provenance is re-derived from the registers via
//! [`seed_metadata`] — never carried over from the stored row — so this knob can
//! move digests but cannot invent, preserve or launder an anchor: a hand-edited
//! one is reset (loudly, `RE-ANCHORED …` on stderr), a hand-edited unregistered
//! `self` is refused outright, and a path no register recognizes is announced
//! (`SEEDED …`) instead of silently acquiring the `capi_v0145` residue.
//! `produced_by` is re-derived from [`LANE_INVARIANT`] the same way (a reset is
//! announced `RE-LANED …`). Every rewritten digest is announced too
//! (`DIGEST MOVED …`, the check arm's label), so "the regen moved no digest" is
//! read off the regen's own output, and every row whose artifact is gone is
//! announced `DROPPED …`, with the old and new row counts in the summary line.
//! `-- --nocapture` is required: the regen run passes, and libtest discards a
//! passing test's stderr. Run it after reviewing *why* the bytes moved, and
//! commit the lock diff together with the change that caused it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The committed lock, repo-root-relative. It lives inside a scanned root and is
/// therefore skipped by the scan (it fingerprints artifacts, it is not one).
const LOCK_PATH: &str = "tests/golden/golden.lock.json";

/// The deliberate regen command every message of this test prints. Its
/// announcements are a passing test's stderr, which libtest only shows under
/// `--nocapture`.
const LOCK_REGEN_CMD: &str =
    "DSS_UPDATE_GOLDEN_LOCK=1 cargo test -p dss-core --test golden_lock -- --nocapture";

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
const EXCLUDED_TREES: &[(&str, &str)] = &[
    (
        "crates/dss-metis/tests/golden",
        "the METIS partitioner fixtures (.graph inputs + .part.N outputs, regenerated manually per \
         tools/golden/gen_metis_reference.md) are captured from the METIS 5.2.1 C original, not \
         from any DSS oracle: they witness a vendored third-party algorithm, no WP of this plan \
         regenerates them, and anchoring them would need a seventh anchor value \
         GOLDEN_REBASE_PLAN.md \u{a7}1.2 does not define. Revisit in G3.6.",
    ),
    (
        "tests/corpus/props_r4133",
        "the vendored G1.1 r4133 property census (docs/plans-archive/R4133_PROPS_PLAN.md RP0.1): \
         frozen *evidence*, not goldens — nothing in tools/golden regenerates them (their only \
         re-measurement path is the RP0.2 DSS_PROPS_CENSUS knob), and none of this lock's anchors \
         describes \"extract of a local-only census\". Their bytes are locked instead by \
         crates/dss-core/tests/props_r4133_evidence_lock.rs (SHA-256 over the five verbatim \
         copies, row counts and cross-file equalities for the derived files), so the tree is \
         guarded without minting an anchor \u{a7}1.2 does not define.",
    ),
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
    /// Measured identical in both lanes by a cross-lane regen (WP-G4 outcome)
    /// and listed in [`LANE_INVARIANT`].
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
         rendering the parity lane produces. Its external half is json/schema_full_oracle.json \
         (capi_v0145, written by tools/golden/gen_schema.py on the pinned oracle); the \
         divergence inventory json/schema_divergences.json between the two is hand-authored and \
         engine-less (NON_ENGINE_RESIDUE). Regen today bypasses the rails via REGEN_SCHEMA_PORT \
         (golden_schema.rs::full_document_matches_port_golden); G3.6 routes it through \
         harness::snapshot_*.",
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

/// The `self` artifacts a cross-lane regen has *measured* identical in both
/// lanes, as `(pattern, the measurement)`: their `produced_by` is
/// `lane-invariant`, which `harness::snapshot_*` lets either lane write. Every
/// other `self` row is produced by the parity lane. Empty until WP-G4 (G4.6)
/// measures a family. Same pattern convention as [`DEANCHORED`]; an entry must
/// lie inside a `DEANCHORED` entry (only a `self` row names a lane) and cover a
/// locked row, so `lane-invariant` is a reviewed register edit, never a lock
/// hand-edit a regen would carry over.
const LANE_INVARIANT: &[(&str, &str)] = &[];

/// The seventeen artifacts captured on the retired `capi015` stack. Eleven
/// declare it in their own provenance block — spelled
/// `"engine_spec": "capi015"` in the seven `props/` + `line_constants/` dumps
/// and `"oracle": "capi015"` in the four `.meta.json` sidecars (measured; a
/// derivation from `engine_spec` alone would drop those four); plan G0.1
/// enumerated those eleven. The other six carry no provenance block at all:
/// they are the payloads the SAME generator call writes next to a capi015
/// sidecar — `ncim/{pq,pv_qlimit}_{Jacobian.csv,PV2PQ.txt}`
/// (`tools/golden/gen_ncim_reports.py:90-124`, behind the `0.15.0b4` backend
/// guard at `:86-87`; the pinned 0.14.5 engine has no NCIM) and
/// `reports/export_{capacity,overloads}_seasonal.txt`
/// (`tools/golden/gen_reports.py::gen_seasonal_overloads`, reachable only under
/// `DSS_ORACLE_ENGINE=capi015`; 0.14.5 reports base, not seasonal, ratings). The
/// G0.1 sweep read declared engine strings and so missed them; RF-D08-06
/// registered them and added the sidecar↔payload rail
/// ([`capi015_sidecar_violations`]) that keeps a payload from falling back to
/// the `capi_v0145` residue again, and its settlement added the stamp rail
/// ([`capi015_stamp_violations`]), which reads both spellings from disk so a
/// stamped capture dropped from this list reds too — the count below is a
/// cross-check, not the only guard. Their generator environment — a dss_capi
/// 0.15.0b4 / DSS-Python 0.16.0b2 beta stack — no longer exists, so they can
/// never be regenerated and `snapshot_*` hard-refuses them.
const CAPI015_ARTIFACTS: &[&str] = &[
    "tests/golden/line_constants/line_geometry_carson.json",
    "tests/golden/ncim/pq.meta.json",
    "tests/golden/ncim/pq_Jacobian.csv",
    "tests/golden/ncim/pq_PV2PQ.txt",
    "tests/golden/ncim/pv_qlimit.meta.json",
    "tests/golden/ncim/pv_qlimit_Jacobian.csv",
    "tests/golden/ncim/pv_qlimit_PV2PQ.txt",
    "tests/golden/props/autotrans_bh.json",
    "tests/golden/props/linemedium.json",
    "tests/golden/props/linespacing_eqspacing.json",
    "tests/golden/props/regcontrol.json",
    "tests/golden/props/swtcontrol.json",
    "tests/golden/props/transformer_bh.json",
    "tests/golden/reports/export_capacity_seasonal.meta.json",
    "tests/golden/reports/export_capacity_seasonal.txt",
    "tests/golden/reports/export_overloads_seasonal.meta.json",
    "tests/golden/reports/export_overloads_seasonal.txt",
];

const CAPI015_REASON: &str = "captured on the retired dss_capi 0.15.0b4 / DSS-Python 0.16.0b2 beta stack (the pinned \
     0.14.5 oracle cannot render these 0.15.x-only surfaces); that environment no longer \
     exists, so the bytes are unreproducible and snapshot_* hard-refuses them.";

/// A capi015 artifact whose committed bytes are no longer purely that capture:
/// a later WP measured some cells on another engine and overlaid them. The
/// overlay does **not** move the anchor — the value authority for every unmoved
/// cell is still the 0.15.0b4 capture, and the overlaying engine is explicitly
/// *not* the authority for the rest — but it has to be stated HERE, because the
/// lock is the provenance register a regenerator reads first and a row saying
/// only "captured on … 0.15.0b4" tells them every byte is that capture's.
/// [`R4133_FAMILIES`] states `props/fuse.json`'s derivation in exactly this way;
/// this register does the same for an artifact that stays `capi015`. The text is
/// appended to [`CAPI015_REASON`]. Added by the RP3.7 audit settlement
/// (2026-09-02).
const CAPI015_OVERLAYS: &[(&str, &str)] = &[(
    "tests/golden/props/swtcontrol.json",
    "Since R4133_PROPS RP3.7 (2026-09-02) the bytes are PARTLY DERIVED: the ten Normal/State \
     cells were overlaid with the official EPRI r4133 DLL's own bytes (the per-phase state \
     arrays of SwtControl.pas:589-599/:600-610, a surface no capi-line engine renders), \
     measured by replaying this artifact's five scenarios through the epri-worker bridge with \
     the props gate's own `clear` + `new circuit.propsprobe` preamble; the artifact's own \
     `oracle.engine` block carries the per-cell citations. The anchor stays capi015 because \
     r4133 is NOT this artifact's value authority - its Action, Reset, Enabled, SwitchedObj \
     and Delay cells still hold capi-side values r4133 diverges from. A regen must repeat the \
     r4133 overlay.",
)];

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
         byte-identically (proven: docs/phase-records/epri-bridge.md \u{a7}\"Parity proof \
         (committed goldens UNTOUCHED — scratch regen + byte-compare)\", the flicker/pst_demo.json \
         row). The pinned 0.14.5 oracle cannot produce this family at all (its \
         DoFlickerCalculations segfaults).",
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
     EPRI r3723 engine by tools/opendss/gen_ad_reference.py, which c060c0b2 retired together \
     with that revision (recoverable from history); the tree is frozen (§1.2).";

/// Residue rows whose bytes **no engine wrote** — script-generated input decks
/// and hand-authored documents. They keep the `capi_v0145` anchor only because
/// GOLDEN_REBASE_PLAN.md §1.2 defines a closed anchor set with no value for
/// engine-less bytes and a real anchor for them is G3.6's call; the register
/// writes the truthful reason into the row instead, so the lock stops reading
/// "the pinned oracle produced this". Two alternatives were weighed and not
/// taken (RF-D08-06): a new anchor value (`derived_input`, wired through
/// [`seed_metadata`] and the G0.2 anchor guard) would open the closed set before
/// G3.2a deletes the controls-off family; and `self` cannot be satisfied
/// honestly, because `anchor == self` holds iff `produced_by` names a lane that
/// may (re)write the bytes — no lane writes these, and a `self` row would make a
/// hand-authored document writable by `snapshot_*`. An entry is a directory
/// prefix (`.../`) or an exact path, same convention as [`DEANCHORED`]; it must
/// not overlap any anchor-deciding register (`provenance_registers_are_well_formed`).
const NON_ENGINE_RESIDUE: &[(&str, &str)] = &[
    (
        "tests/golden/feeders_controlsoff/",
        "input deck written by tools/golden/gen_feeders_controlsoff.py from the vendored corpus \
         master (copied line by line, solve/buscoords dropped, redirects rewritten, Set \
         controlmode=OFF + Solve appended); no oracle bytes — the oracle output is \
         feeders_controlsoff.json. The capi_v0145 anchor is the residue, not a provenance claim: \
         GOLDEN_REBASE_PLAN.md \u{a7}1.2 has no anchor value for a derived input, and G3.2a \
         deletes the family.",
    ),
    (
        "tests/golden/json/schema_divergences.json",
        "hand-authored inventory of the port's documented schema divergences from the pinned \
         0.14.5 oracle (r4133 / 0.15.x adoptions, each with its upstream cause), applied by \
         golden_schema.rs between the port's render and json/schema_full_oracle.json and edited \
         by hand whenever a divergence is adopted; no engine writes it (tools/golden/gen_schema.py \
         writes only the oracle half). The capi_v0145 anchor is the residue, not a provenance \
         claim; it is not `self` because no lane (re)produces it, so snapshot_* must never \
         write it.",
    ),
];

/// A `capi_v0145` artifact whose committed bytes are no longer purely the
/// pinned oracle's capture: later WPs hand-landed blocks the 0.14.5 engine
/// cannot render. The twin of [`CAPI015_OVERLAYS`] for the residue anchor — the
/// anchor does not move (the value authority for every untouched block is still
/// the pinned capture), but a regenerator reading the lock must learn which
/// bytes a pinned-oracle regen would revert. Same pattern convention and
/// disjointness rule as [`NON_ENGINE_RESIDUE`]. The nine rows are the complete
/// set of Dump goldens a pinned-oracle re-capture does not reproduce (the
/// RF-D08-06 audit re-captured all 44 `gen_reports.py::gen_dump_decks` outputs:
/// 35 identical, these 9 differ). A G3 step that de-anchors `reports/dump*`
/// drops them in the same commit (the hygiene test reds on the overlap).
const CAPI_V0145_OVERLAYS: &[(&str, &str)] = &[
    (
        "tests/golden/reports/dump3_bare.txt",
        DUMP_BH_REGCONTROL_OVERLAY,
    ),
    (
        "tests/golden/reports/dump3_commands.txt",
        DUMP3_COMMANDS_OVERLAY,
    ),
    (
        "tests/golden/reports/dump3_debug.txt",
        DUMP_BH_REGCONTROL_OVERLAY,
    ),
    ("tests/golden/reports/dump_autotrans.txt", DUMP_BH_OVERLAY),
    ("tests/golden/reports/dump_autotrans3.txt", DUMP_BH_OVERLAY),
    (
        "tests/golden/reports/dump_regcontrol.txt",
        DUMP_REGCONTROL_OVERLAY,
    ),
    ("tests/golden/reports/dump_transformer.txt", DUMP_BH_OVERLAY),
    (
        "tests/golden/reports/dump_transformer3.txt",
        DUMP_BH_OVERLAY,
    ),
    (
        "tests/golden/reports/dump_transformer_disabled.txt",
        DUMP_BH_OVERLAY,
    ),
];

/// `reports/dump3_commands.txt`: the property-help listing of every class.
const DUMP3_COMMANDS_OVERLAY: &str = "PARTLY HAND-LANDED, not purely the pinned oracle's capture: the [Relay]/[Recloser]/[Fuse]/\
     [SwtControl] blocks are the r4133 property surfaces (Relay 71, Recloser 46, Fuse 12, \
     SwtControl 9 props; help verbatim from the r4133 binary's own Dump commands via \
     tools/golden/r4133_help.py; 845fdd2f, 288afc35), and the 0.15.x property lines of \
     [CNData] (SemiconLayer), [LineSpacing] (Detailed/EqDistPhPh/EqDistPhN/AvgPhaseHeight/\
     AvgNeutralHeight), [Transformer]/[AutoTrans] (BHPoints/BHCurrent/BHFlux) and [RegControl] \
     (Idle/IdleReverse/IdleForward/FwdThreshold) were hand-edited in to match the port's render \
     (2a20b97c, d9ee3f1c, 702adcbd). The pinned oracle renders none of them, so all of them are \
     pinned SELF-REFERENTIALLY against our own render by \
     golden_reports.rs::dump3_commands_matches_oracle. The anchor stays capi_v0145 because \
     every other block ([execcommands], [execoptions] and the remaining class sections) is \
     still the tools/golden/gen_reports.py capture on the pinned oracle, as is the deck \
     sidecar dump3_commands.meta.json (untouched since 34b15a12). A regen from the pinned \
     oracle reverts every hand-landed block; it must re-land them.";

/// The five object dumps whose only hand-landed lines are 702adcbd's BH-curve
/// props (WP-U1.6 C6): `dump_{autotrans,autotrans3}` (one AutoTrans each) and
/// `dump_{transformer,transformer3,transformer_disabled}` (one Transformer each).
const DUMP_BH_OVERLAY: &str = "PARTLY HAND-LANDED, not purely the pinned oracle's capture: 702adcbd (WP-U1.6 C6) \
     hand-edited the 0.15.x default BH-curve lines `~ BHPoints=0`, `~ BHCurrent=`, `~ BHFlux=` into \
     every Transformer/AutoTrans object block to match the port's render. The pinned 0.14.5 \
     engine has no BH property, so golden_reports.rs pins those three lines \
     SELF-REFERENTIALLY against our own render, while every other line is still the \
     tools/golden/gen_reports.py capture on the pinned oracle. A regen from the pinned oracle \
     drops them and must re-land them.";

/// `dump_regcontrol`: 702adcbd's RegControl lines only (WP-U1.6 C5).
const DUMP_REGCONTROL_OVERLAY: &str = "PARTLY HAND-LANDED, not purely the pinned oracle's capture: 702adcbd (WP-U1.6 C5) \
     hand-edited the RegControl object block to match the port's render: `~ RevThreshold=-100` \
     (the signed-threshold default, docs/upgrade/DIVERGENCES.md WP-U1.6 C5) where the pinned \
     0.14.5 engine renders 100 (.inputs/dss_capi/src/Controls/RegControl.pas:525-526), plus the \
     0.15.x lines `~ Idle=No`, `~ IdleReverse=No`, `~ IdleForward=No`, `~ FwdThreshold=100`, \
     props the pinned engine does not have. golden_reports.rs pins them SELF-REFERENTIALLY \
     against our own render, while every other line is still the tools/golden/gen_reports.py \
     capture on the pinned oracle. A regen from the pinned oracle reverts them and must re-land \
     them.";

/// `dump3_bare` / `dump3_debug`: both kinds of 702adcbd lines (two Transformers
/// and one RegControl in the `dump3.dss` fixture).
const DUMP_BH_REGCONTROL_OVERLAY: &str = "PARTLY HAND-LANDED, not purely the pinned oracle's capture: 702adcbd (WP-U1.6 C5/C6) \
     hand-edited two kinds of lines in to match the port's render: the 0.15.x default BH-curve \
     lines `~ BHPoints=0`, `~ BHCurrent=`, `~ BHFlux=` in every Transformer object block, and in \
     the RegControl object block `~ RevThreshold=-100` (the signed-threshold default, \
     docs/upgrade/DIVERGENCES.md WP-U1.6 C5) where the pinned 0.14.5 engine renders 100 \
     (.inputs/dss_capi/src/Controls/RegControl.pas:525-526), plus the 0.15.x lines `~ Idle=No`, \
     `~ IdleReverse=No`, `~ IdleForward=No`, `~ FwdThreshold=100`. The pinned engine renders \
     none of those lines as landed, so golden_reports.rs pins them SELF-REFERENTIALLY against \
     our own render, while every other line is still the tools/golden/gen_reports.py capture on \
     the pinned oracle. A regen from the pinned oracle reverts them and must re-land them.";

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
    /// Human note, written from [`COMMENT`] by every regen and checked equal to
    /// it, so the committed note cannot drift from the const.
    #[serde(default)]
    comment: String,
    /// Sorted by `path`; one row per committed golden artifact.
    artifacts: Vec<Artifact>,
}

const COMMENT: &str = "Provenance lock over the committed golden corpus (GOLDEN_REBASE_PLAN.md \
\u{a7}1.2, G0.1): tests/golden/** plus the registered out-of-tree witness \
crates/dss-core/tests/data/adiakoptics/r3723_ref/. That enumeration is the scope; golden-shaped \
trees left outside it (today: crates/dss-metis/tests/golden, the vendored METIS 5.2.1 fixtures, and \
tests/corpus/props_r4133, the frozen r4133 property-census evidence locked by \
props_r4133_evidence_lock.rs) are \
named with their reason in golden_lock.rs::EXCLUDED_TREES and revisited in G3.6. Each row records \
the artifact's content digest and where the truth in those bytes comes from (anchor), plus - for \
self-anchored rows - a mandatory reason and the lane allowed to (re)produce them. \
crates/dss-core/tests/golden_lock.rs asserts, fail-on-stale in both directions: every artifact has a \
row, every row an artifact, every digest matches, and every row's anchor, reason and produced_by are \
exactly what the test's provenance registers (DEANCHORED / CAPI015_ARTIFACTS / R4133_FAMILIES / \
FPC_ARTIFACT / R3723_TREE, capi_v0145 as the residue; the reason-only CAPI015_OVERLAYS / \
NON_ENGINE_RESIDUE / CAPI_V0145_OVERLAYS; LANE_INVARIANT for produced_by) derive for its path - so \
re-anchoring an artifact, or declaring it lane-invariant, is a reviewed edit to a register, never a \
hand-edit here. A JSON artifact whose own provenance block declares an engine must be anchored to \
that engine (self rows and golden_lock.rs::DECLARED_PROVENANCE_EXCEPTIONS aside). Digests are taken \
over the COMMITTED content: \
CRLF is normalized to LF for text artifacts (core.autocrlf=true here, so the working tree carries \
CRLF while git stores LF), while reports/*.bin streams - `binary` in .gitattributes, cross-checked \
against that file - are hashed raw; an EOL-only working-tree change therefore moves no digest \
because it moves no committed byte (see the golden_lock.rs module docs, assertion 3, for what that \
means for G4.3). Regenerate DELIBERATELY with \
`DSS_UPDATE_GOLDEN_LOCK=1 cargo test -p dss-core --test golden_lock -- --nocapture` (the flag is \
required: the regen run passes, and libtest discards a passing test's stderr); it moves digests only \
- provenance is re-derived from the registers, and every re-anchored, re-laned, newly seeded or \
dropped path and every moved digest is announced on stderr - and the diff is the review artifact. \
The regeneration rules R1-R4 and the operational walkthrough are in TESTING.md: the section \
\"Golden provenance lock (`golden_lock.rs`) and the self-golden write rails\" and, under \
\"Procedures\", \"Regenerate a self-golden (R1–R4)\".";

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

/// The lane that may write the `self` artifact at `path`: `lane-invariant` when
/// a `lane_invariant` entry (the live one is [`LANE_INVARIANT`]) covers it, else
/// the parity lane, which holds the strictest byte contract until WP-G4 measures
/// a family identical in both lanes.
fn producing_lane(path: &str, lane_invariant: &[(&str, &str)]) -> ProducedBy {
    if lane_invariant
        .iter()
        .any(|(pattern, _)| pattern_covers(pattern, path))
    {
        ProducedBy::LaneInvariant
    } else {
        ProducedBy::Parity
    }
}

/// The `produced_by` violation of one locked row, if any: `anchor == self` iff
/// `produced_by` is set, and a `self` row names the lane `lane_invariant`
/// derives ([`producing_lane`]). Parameterized so
/// `produced_by_is_derived_from_the_lane_invariant_register` can drive it.
fn produced_by_violation(row: &Artifact, lane_invariant: &[(&str, &str)]) -> Option<String> {
    let is_self = row.anchor == Anchor::SelfSnapshot;
    if is_self != row.produced_by.is_some() {
        return Some(format!(
            "  PRODUCED_BY MISMATCH: {} is anchored {:?} with produced_by {:?}; exactly the \
             self-anchored rows name a producing lane\n",
            row.path, row.anchor, row.produced_by
        ));
    }
    let want = is_self.then(|| producing_lane(&row.path, lane_invariant));
    (row.produced_by != want).then(|| {
        format!(
            "  PRODUCED_BY OUT OF REGISTER: {} is locked produced_by {:?} but LANE_INVARIANT \
             derives {:?}. A lane-invariant family is a LANE_INVARIANT entry naming the \
             cross-lane regen that measured it, never a lock hand-edit\n",
            row.path, row.produced_by, want
        )
    })
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
        return (
            Anchor::SelfSnapshot,
            (*reason).to_string(),
            Some(producing_lane(path, LANE_INVARIANT)),
        );
    }
    if CAPI015_ARTIFACTS.contains(&path) {
        let mut reason = CAPI015_REASON.to_string();
        if let Some((_, overlay)) = CAPI015_OVERLAYS.iter().find(|(p, _)| *p == path) {
            reason.push(' ');
            reason.push_str(overlay);
        }
        return (Anchor::Capi015, reason, None);
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
    // The residue. The two reason-only registers never move the anchor; the
    // hygiene test keeps them disjoint from every register above, so reaching
    // this point is the only way to pick one up.
    let reason = NON_ENGINE_RESIDUE
        .iter()
        .chain(CAPI_V0145_OVERLAYS)
        .find(|(pattern, _)| pattern_covers(pattern, path))
        .map(|(_, reason)| (*reason).to_string())
        .unwrap_or_default();
    (Anchor::CapiV0145, reason, None)
}

/// Do two register entries (each a `/`-terminated prefix or an exact path)
/// claim a common artifact?
fn patterns_overlap(a: &str, b: &str) -> bool {
    pattern_covers(a, b) || pattern_covers(b, a)
}

/// The [`DEANCHORED`] × [`CAPI015_ARTIFACTS`] disjointness arm, as
/// `(deanchored pattern, capi015 path)` pairs — must be empty.
/// [`seed_metadata`] consults `DEANCHORED` first, so an overlapping pattern
/// would silently re-anchor an unreproducible capi015 capture to `self`, and
/// with it make those bytes writable by the G0.2 `snapshot_*` rails, while the
/// capi015 closed-set count and its stale sweep stayed green (the entry is
/// still "hit" — by a row the register no longer decides). Parameterized so
/// `deanchored_never_shadows_a_capi015_artifact` can drive it red.
fn deanchored_capi015_overlaps<'a>(
    deanchored: &[(&'a str, &'a str)],
    capi015: &[&'a str],
) -> Vec<(&'a str, &'a str)> {
    deanchored
        .iter()
        .flat_map(|&(pattern, _)| {
            capi015
                .iter()
                .filter(move |path| patterns_overlap(pattern, path))
                .map(move |&path| (pattern, path))
        })
        .collect()
}

/// Is `path` a payload of the generator call that wrote `sidecar` (a
/// `<stem>.meta.json`)? The two capi015 generators write their payloads beside
/// the sidecar as `<stem>.<ext>` (`gen_reports.py::gen_seasonal_overloads`) or
/// `<stem>_<Name>.<ext>` (`gen_ncim_reports.py`), in the same directory.
fn is_sidecar_payload(sidecar: &str, path: &str) -> bool {
    let Some(stem) = sidecar.strip_suffix(".meta.json") else {
        return false;
    };
    path != sidecar
        && path.strip_prefix(stem).is_some_and(|rest| {
            (rest.starts_with('.') || rest.starts_with('_')) && !rest.contains('/')
        })
}

/// The capi015 sidecar↔payload rail (RF-D08-06), over the artifact `paths`
/// currently on disk. A capi015 sidecar is a `.meta.json` either registered in
/// `capi015` or stamping `"oracle": "capi015"` itself (`stamped`, read from the
/// file by the caller). Every such sidecar must be registered, must still sit
/// beside at least one payload, and every payload of it must be registered too:
/// the payloads carry no provenance block of their own, so this rail — not an
/// engine-string sweep over JSON — is what keeps them out of the `capi_v0145`
/// residue. One message per violation; parameterized so
/// `capi015_sidecar_rail_reds_on_an_unregistered_payload` can drive it red.
fn capi015_sidecar_violations(paths: &[&str], stamped: &[&str], capi015: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    let mut sidecars: Vec<&str> = capi015
        .iter()
        .copied()
        .filter(|p| p.ends_with(".meta.json"))
        .collect();
    for &s in stamped {
        if !capi015.contains(&s) {
            out.push(format!(
                "  UNREGISTERED CAPI015 SIDECAR: {s} stamps \"oracle\": \"capi015\" but \
                 CAPI015_ARTIFACTS does not list it\n"
            ));
        }
        if !sidecars.contains(&s) {
            sidecars.push(s);
        }
    }
    for s in sidecars {
        let payloads: Vec<&str> = paths
            .iter()
            .copied()
            .filter(|p| is_sidecar_payload(s, p))
            .collect();
        if payloads.is_empty() {
            out.push(format!(
                "  CAPI015 SIDECAR WITHOUT PAYLOAD: {s} describes a generator call whose \
                 payload is gone — delete the sidecar with it, or restore the payload\n"
            ));
        }
        for p in payloads {
            if !capi015.contains(&p) {
                out.push(format!(
                    "  CAPI015 PAYLOAD OFF REGISTER: {p} was written by the same generator call \
                     as the capi015 sidecar {s} but CAPI015_ARTIFACTS does not list it, so it \
                     falls to the capi_v0145 residue — an engine that cannot produce it\n"
                ));
            }
        }
    }
    out
}

/// The capi015 stamp rail (RF-D08-06 settlement), both directions, over the
/// artifacts whose own provenance block declares capi015 (`stamped`, read from
/// disk by [`capi015_stamped_artifacts`]). Forward: a stamped capture that is
/// not a `.meta.json` must be registered — the sidecar half of that direction
/// is [`capi015_sidecar_violations`]'s, which also follows the payloads.
/// Converse: every registered artifact either declares capi015 itself or is a
/// payload of a registered capi015 sidecar — payloads carry no block of their
/// own. The converse is also the reader's non-vacuity guard: a reader that
/// finds nothing reds every stamped registration instead of silently emptying
/// the forward arm. With both, the register is decided by the corpus and the
/// closed-set count is a cross-check. One message per violation, parameterized
/// so `capi015_stamp_rail_reds_on_a_dropped_or_unread_capture` can drive it red.
fn capi015_stamp_violations(stamped: &[&str], capi015: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    for &s in stamped {
        if !s.ends_with(".meta.json") && !capi015.contains(&s) {
            out.push(format!(
                "  UNREGISTERED CAPI015 CAPTURE: {s} declares oracle.engine_spec \"capi015\" but \
                 CAPI015_ARTIFACTS does not list it, so it falls to the capi_v0145 residue — an \
                 engine that cannot produce it\n"
            ));
        }
    }
    for &p in capi015 {
        let is_payload = capi015
            .iter()
            .any(|s| s.ends_with(".meta.json") && is_sidecar_payload(s, p));
        if !is_payload && !stamped.contains(&p) {
            out.push(format!(
                "  CAPI015 ARTIFACT WITHOUT STAMP: {p} is registered capi015 but neither declares \
                 it in its own provenance block (\"oracle\": \"capi015\" or oracle.engine_spec) \
                 nor is a payload of a registered capi015 sidecar\n"
            ));
        }
    }
    out
}

/// Does a JSON artifact's own provenance block declare the retired capi015
/// stack? Two spellings exist on disk (measured): the `.meta.json` sidecars
/// carry a top-level `"oracle": "capi015"`, the `props/` + `line_constants/`
/// dumps an `"oracle": {"engine_spec": "capi015", …}` block.
fn declares_capi015(doc: &serde_json::Value) -> bool {
    match doc.get("oracle") {
        Some(serde_json::Value::String(engine)) => engine == "capi015",
        Some(block) => {
            block.get("engine_spec").and_then(serde_json::Value::as_str) == Some("capi015")
        }
        None => false,
    }
}

/// The JSON artifacts among `disk` whose own provenance block declares capi015
/// ([`declares_capi015`]) — the evidence side of both capi015 rails. Only a file
/// holding the literal `"capi015"` can carry either spelling, so only those are
/// parsed (a parse failure among them panics: fail loud, never skip).
fn capi015_stamped_artifacts(root: &Path, disk: &BTreeMap<String, String>) -> Vec<String> {
    disk.keys()
        .filter(|p| p.ends_with(".json"))
        .filter(|p| {
            let abs = root.join(p);
            let text = std::fs::read_to_string(&abs)
                .unwrap_or_else(|e| panic!("read {}: {e}", abs.display()));
            text.contains("\"capi015\"") && {
                let doc: serde_json::Value = serde_json::from_str(&text)
                    .unwrap_or_else(|e| panic!("parse {}: {e}", abs.display()));
                declares_capi015(&doc)
            }
        })
        .cloned()
        .collect()
}

/// Locked JSON artifacts whose own provenance block declares an engine their
/// anchor is not, as `(path, the anchor the lock keeps, why)`. Fail-on-stale:
/// an entry must excuse a live contradiction
/// ([`declared_provenance_violations`]).
const DECLARED_PROVENANCE_EXCEPTIONS: &[(&str, Anchor, &str)] = &[(
    "tests/golden/flicker/pst_demo.json",
    Anchor::R4133,
    "declares the official-EPRI r3723 Oddie capture its committed bytes are; anchored r4133 \
     by R4133_FAMILIES because the r4133 epri-worker regen reproduces every payload key \
     byte-identically (docs/phase-records/epri-bridge.md \u{a7}\"Parity proof (committed goldens \
     UNTOUCHED — scratch regen + byte-compare)\", the flicker/pst_demo.json row), and r4133 is \
     the only surviving regeneration path.",
)];

/// The declaring population of the locked JSON artifacts, as
/// [`declared_provenance_census`] measures it: the declaring rows and the
/// declarations per key. A reader that stops seeing a declaration leaves it
/// unchecked without a red, so the census is pinned. Re-measure it (the red
/// prints the measured census) in the commit that moves the population.
const DECLARED_PROVENANCE_CENSUS: (usize, &[(&str, usize)]) = (
    173,
    &[
        ("engine", 9),
        ("oracle", 11),
        ("oracle.engine", 162),
        ("oracle.engine_spec", 10),
    ],
);

/// The r4133 binary's own version banner (`tools/opendss/revisions.json`, its
/// `expect_version`), which some r4133 captures record as their engine.
const R4133_VERSION_BANNER: &str = "Version 11.0.0.1 (64-bit build) - Charlottesville";

/// One provenance declaration of a JSON artifact: the key and its value, `None`
/// when the key carries no engine name as a string (a non-string value, or an
/// `oracle` block without `engine_spec` / `engine`).
type Declaration = (&'static str, Option<String>);

/// A locked JSON artifact that declares its provenance.
struct Declaring {
    path: String,
    anchor: Anchor,
    declarations: Vec<Declaration>,
}

/// The provenance declarations of a JSON document: a top-level `engine_spec` or
/// `engine`, and an `oracle` that is either a string or a block carrying
/// `engine_spec` / `engine`. A block carrying neither is one declaration
/// without an engine name.
fn provenance_declarations(doc: &serde_json::Value) -> Vec<Declaration> {
    let text = |v: &serde_json::Value| v.as_str().map(str::to_string);
    let mut out: Vec<Declaration> = ["engine_spec", "engine"]
        .into_iter()
        .filter_map(|key| doc.get(key).map(|v| (key, text(v))))
        .collect();
    match doc.get("oracle") {
        None => {}
        Some(serde_json::Value::Object(block)) => {
            let before = out.len();
            for (key, name) in [
                ("engine_spec", "oracle.engine_spec"),
                ("engine", "oracle.engine"),
            ] {
                if let Some(v) = block.get(key) {
                    out.push((name, text(v)));
                }
            }
            if out.len() == before {
                out.push(("oracle", None));
            }
        }
        Some(v) => out.push(("oracle", text(v))),
    }
    out
}

/// Every anchor a declared engine string names, empty for an engine this test
/// does not know. Every spelling on disk is covered (measured over the locked
/// JSON artifacts): the 0.15.x beta stack, the pinned 0.14.5 oracle, the
/// retired r3723 and the r4133 engine.
fn declared_anchors(value: &str) -> Vec<Anchor> {
    let v = value.to_ascii_lowercase();
    [
        (
            Anchor::Capi015,
            v.contains("capi015") || v.contains("0.15.0b"),
        ),
        (
            Anchor::CapiV0145,
            v.contains("capi_v0145") || v.contains("0.14.5"),
        ),
        (Anchor::R3723, v.contains("r3723")),
        (
            Anchor::R4133,
            v.contains("r4133") || value.starts_with(R4133_VERSION_BANNER),
        ),
    ]
    .into_iter()
    .filter_map(|(anchor, named)| named.then_some(anchor))
    .collect()
}

/// The locked JSON artifacts on disk that declare their provenance, read from
/// disk (a parse failure panics: fail loud, never skip).
fn declared_provenance_rows(
    root: &Path,
    rows: &[Artifact],
    disk: &BTreeMap<String, String>,
) -> Vec<Declaring> {
    rows.iter()
        .filter(|row| row.path.ends_with(".json") && disk.contains_key(&row.path))
        .filter_map(|row| {
            let abs = root.join(&row.path);
            let text = std::fs::read_to_string(&abs)
                .unwrap_or_else(|e| panic!("read {}: {e}", abs.display()));
            if !text.contains("\"engine") && !text.contains("\"oracle\"") {
                return None;
            }
            let doc: serde_json::Value = serde_json::from_str(&text)
                .unwrap_or_else(|e| panic!("parse {}: {e}", abs.display()));
            let declarations = provenance_declarations(&doc);
            (!declarations.is_empty()).then(|| Declaring {
                path: row.path.clone(),
                anchor: row.anchor,
                declarations,
            })
        })
        .collect()
}

/// The declared-provenance rail: every declaration names one known engine, and
/// the locked anchor of a declaring artifact is among the engines it declares,
/// unless the row is `self` (its [`DEANCHORED`] reason says why the declared
/// engine stopped being the value authority) or an `exceptions` entry excuses
/// exactly that path and anchor. A value naming several engines declares none
/// of them: it passes only when one of its engines is the one a single-engine
/// `engine_spec` of the same artifact names. An entry that excuses nothing is
/// stale. One message per violation; parameterized so
/// `declared_provenance_must_admit_the_locked_anchor` can drive it red.
fn declared_provenance_violations(
    declaring: &[Declaring],
    exceptions: &[(&str, Anchor, &str)],
) -> Vec<String> {
    let mut out = Vec::new();
    let mut hits = vec![0usize; exceptions.len()];
    for d in declaring {
        let specified: Vec<Anchor> = d
            .declarations
            .iter()
            .filter(|(key, _)| key.ends_with("engine_spec"))
            .filter_map(|(_, value)| match declared_anchors(value.as_deref()?)[..] {
                [a] => Some(a),
                _ => None,
            })
            .collect();
        let mut declared = Vec::new();
        for (key, value) in &d.declarations {
            let Some(v) = value.as_deref() else {
                out.push(format!(
                    "  UNREADABLE PROVENANCE DECLARATION: {} declares `{key}` without a string \
                     engine name\n",
                    d.path
                ));
                continue;
            };
            match declared_anchors(v)[..] {
                [] => out.push(format!(
                    "  UNCLASSIFIED PROVENANCE DECLARATION: {} declares `{key}` = {v:?}, an \
                     engine declared_anchors() does not know — teach it the engine rather \
                     than leave the declaration unchecked\n",
                    d.path
                )),
                [a] => declared.push(a),
                ref several => {
                    if !several.iter().any(|a| specified.contains(a)) {
                        out.push(format!(
                            "  AMBIGUOUS PROVENANCE DECLARATION: {} declares `{key}` = {v:?}, \
                             which names {several:?} while no single-engine `engine_spec` of \
                             the artifact picks one of them — name the one engine the bytes \
                             came from\n",
                            d.path
                        ));
                    }
                }
            }
        }
        if d.anchor == Anchor::SelfSnapshot || declared.is_empty() || declared.contains(&d.anchor) {
            continue;
        }
        match exceptions
            .iter()
            .position(|(p, a, _)| *p == d.path && *a == d.anchor)
        {
            Some(i) => hits[i] += 1,
            None => out.push(format!(
                "  DECLARED PROVENANCE CONTRADICTS ANCHOR: {} is locked {:?} but its own \
                 provenance block declares {declared:?} ({:?}) — register it where its bytes \
                 came from, or add a reasoned DECLARED_PROVENANCE_EXCEPTIONS entry\n",
                d.path, d.anchor, d.declarations
            )),
        }
    }
    for (i, (path, anchor, _)) in exceptions.iter().enumerate() {
        if hits[i] == 0 {
            out.push(format!(
                "  STALE DECLARED_PROVENANCE_EXCEPTIONS ENTRY: {path:?} ({anchor:?}) excuses no \
                 locked artifact whose declaration contradicts that anchor\n"
            ));
        }
    }
    out
}

/// The census of a declaring population: its row count and its declarations
/// per key, keys sorted.
fn declared_provenance_census(declaring: &[Declaring]) -> (usize, Vec<(&'static str, usize)>) {
    let mut per_key: BTreeMap<&'static str, usize> = BTreeMap::new();
    for d in declaring {
        for (key, _) in &d.declarations {
            *per_key.entry(*key).or_default() += 1;
        }
    }
    (declaring.len(), per_key.into_iter().collect())
}

/// A red when the measured census of `declaring` is not `pinned`
/// ([`DECLARED_PROVENANCE_CENSUS`]); parameterized so
/// `declared_provenance_must_admit_the_locked_anchor` can drive it red.
fn declared_provenance_census_violation(
    declaring: &[Declaring],
    pinned: (usize, &[(&str, usize)]),
) -> Option<String> {
    let measured = declared_provenance_census(declaring);
    (measured.0 != pinned.0 || measured.1 != pinned.1).then(|| {
        format!(
            "  DECLARED PROVENANCE CENSUS MOVED: the rail reads {measured:?} (declaring rows, \
             declarations per key), DECLARED_PROVENANCE_CENSUS pins {pinned:?} — a reader that \
             stops seeing a declaration leaves it unchecked, so re-measure and update the const \
             in the commit that moves the population\n"
        )
    })
}

/// `reports/*.bin` are raw little-endian IEEE-754 streams, declared `binary` in
/// `.gitattributes` precisely so git never EOL-munges them.
///
/// This predicate **emulates** that declaration, so the two must not drift: a
/// `.bin` added outside `reports/` would be EOL-normalized by git but hashed raw
/// here, and a binary family with another extension would be hashed
/// EOL-normalized although git stores it raw — either way the digest silently
/// stops being over the committed content. [`assert_binary_classification_matches_gitattributes`]
/// binds the two over every scanned path, both directions, against the
/// repo-root `.gitattributes`, and refuses every attribute declaration in the
/// tree that could decide a locked path's bytes but that this reader does not
/// see or does not emulate ([`gitattributes_binary_patterns`]). Pathspecs and
/// attribute file names are compared in any ASCII case, as git compares them
/// under `core.ignorecase` (on in this checkout), and a pathspec that reaches a
/// locked path only that way reds ([`binary_classification_violations`]).
fn is_binary_artifact(rel: &str) -> bool {
    rel.ends_with(".bin")
}

/// Attributes that change the bytes git stores or checks out for a path, so a
/// declaration carrying one decides what a locked path's digest is over.
const CONTENT_ATTRIBUTES: &[&str] = &[
    "binary",
    "text",
    "eol",
    "crlf",
    "filter",
    "working-tree-encoding",
    "ident",
];

/// The attribute a `.gitattributes` token sets, unsets or unspecifies
/// (`-text` and `!text` name `text`, `eol=lf` names `eol`).
fn attribute_name(token: &str) -> &str {
    let name = token.trim_start_matches(['-', '!']);
    name.split_once('=').map_or(name, |(name, _)| name)
}

/// Is `spec` (a repo-root pathspec without its leading `/`) a locked root or a
/// path below one, in any ASCII case?
fn is_rooted(spec: &str) -> bool {
    ROOTS.iter().any(|r| {
        spec.get(..r.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(r))
            && matches!(spec.as_bytes().get(r.len()), None | Some(b'/'))
    })
}

/// The pathspecs of the repo-root `.gitattributes` (`text`) that mark a locked
/// path `binary` or `-text` — the declarations [`is_binary_artifact`] emulates —
/// and one refusal per declaration that could decide a locked path's bytes but
/// would otherwise go unread: every attribute file in `nested` (inside or above
/// a locked root, found by [`nested_gitattributes`]); a content attribute
/// ([`CONTENT_ATTRIBUTES`]) on a pathspec that does not name a locked root yet
/// can reach one (a bare name matches at every depth, a glob before the end of
/// the root's literal prefix matches into it); a content attribute other than
/// `binary` / `-text` on a locked path; a macro or a quoted pathspec carrying
/// one. Roots are compared in any ASCII case. A pathspec that cannot reach a
/// locked root (the vendored corpus) is dropped. Parameterized so
/// `gitattributes_reader_refuses_what_it_cannot_see` can drive every refusal.
fn gitattributes_binary_patterns(text: &str, nested: &[String]) -> (Vec<String>, Vec<String>) {
    let mut patterns = Vec::new();
    let mut refused: Vec<String> = nested
        .iter()
        .map(|p| {
            format!(
                "  NESTED .gitattributes: {p} can set attributes on locked paths, and only the \
                 repo-root .gitattributes is read here — move its declarations there, spelled \
                 from the repo root\n"
            )
        })
        .collect();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut fields = line.split_whitespace();
        let Some(pattern) = fields.next() else {
            continue;
        };
        let content: Vec<&str> = fields
            .filter(|a| CONTENT_ATTRIBUTES.contains(&attribute_name(a)))
            .collect();
        if content.is_empty() {
            continue;
        }
        if pattern.starts_with("[attr]") || pattern.starts_with('"') {
            refused.push(format!(
                "  UNREAD DECLARATION: `{line}` is a macro or a quoted pathspec carrying a \
                 content attribute; this reader neither expands macros nor unquotes, so spell \
                 the attributes on a plain pathspec\n"
            ));
            continue;
        }
        let bare = !pattern.trim_end_matches('/').contains('/');
        let spec = pattern.strip_prefix('/').unwrap_or(pattern);
        if !bare && is_rooted(spec) {
            if content.iter().all(|a| *a == "binary" || *a == "-text") {
                patterns.push(spec.to_string());
            } else {
                refused.push(format!(
                    "  UNEMULATED ATTRIBUTE ON A LOCKED PATH: `{line}` — is_binary_artifact() \
                     emulates only `binary` / `-text`, so the digest would not be over the bytes \
                     git stores\n"
                ));
            }
            continue;
        }
        let reaches_a_root = bare
            || spec.find(['*', '?', '[', '\\']).is_some_and(|k| {
                ROOTS.iter().any(|r| {
                    r.get(..k)
                        .is_some_and(|head| head.eq_ignore_ascii_case(&spec[..k]))
                })
            });
        if reaches_a_root {
            refused.push(format!(
                "  UNROOTED PATHSPEC REACHES A LOCKED PATH: `{line}` carries a content attribute \
                 on a pathspec that does not name a locked root {ROOTS:?} but can match inside \
                 one (a bare name matches at every depth, a leading glob anywhere); spell it \
                 from a locked root or a directory outside them\n"
            ));
        }
    }
    (patterns, refused)
}

/// Every `.gitattributes` other than the repo-root one that can reach a locked
/// path: one among the scanned artifacts (inside a root, its name in any ASCII
/// case), or one `exists` reports in a directory between the repo root and a
/// root. Parameterized so `gitattributes_reader_refuses_what_it_cannot_see` can
/// drive the directories it probes.
fn nested_gitattributes(
    paths: &BTreeMap<String, String>,
    exists: impl Fn(&str) -> bool,
) -> Vec<String> {
    let mut out: std::collections::BTreeSet<String> = paths
        .keys()
        .filter(|p| {
            p.rsplit('/')
                .next()
                .is_some_and(|name| name.eq_ignore_ascii_case(".gitattributes"))
        })
        .cloned()
        .collect();
    for r in ROOTS {
        for (k, _) in r.match_indices('/') {
            let candidate = format!("{}/.gitattributes", &r[..k]);
            if exists(&candidate) {
                out.insert(candidate);
            }
        }
    }
    out.into_iter().collect()
}

/// Match one `.gitattributes` pathspec against a repo-relative path,
/// case-sensitively. Only the
/// two shapes the file actually uses are supported — a `dir/**` subtree and a
/// `dir/<glob>` leaf with at most one `*` — and
/// [`check_binary_classification`] rejects anything else
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
/// both directions, over every scanned artifact, after refusing every
/// declaration [`gitattributes_binary_patterns`] could not read: the repo-root
/// file and the tree under `root` through [`check_binary_classification`].
fn assert_binary_classification_matches_gitattributes(
    root: &Path,
    paths: &BTreeMap<String, String>,
) {
    let path = root.join(".gitattributes");
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    if let Err(e) = check_binary_classification(&text, |p| root.join(p).is_file(), paths) {
        panic!("{e}");
    }
}

/// The checks [`assert_binary_classification_matches_gitattributes`] runs, the
/// first failure being the error: `text` is the repo-root `.gitattributes` and
/// `exists` probes the directories above each locked root
/// ([`nested_gitattributes`]). In order, it reds a declaration the reader
/// refuses, the absence of any `binary` / `-text` pathspec, a pathspec shape
/// [`attr_pattern_matches`] does not implement, and a case-variant or drifted
/// classification of `paths` ([`binary_classification_violations`]).
/// Parameterized so `gitattributes_reader_refuses_what_it_cannot_see` can drive
/// each one red.
fn check_binary_classification(
    text: &str,
    exists: impl Fn(&str) -> bool,
    paths: &BTreeMap<String, String>,
) -> Result<(), String> {
    let (patterns, refused) =
        gitattributes_binary_patterns(text, &nested_gitattributes(paths, exists));
    if !refused.is_empty() {
        return Err(format!(
            "attribute declarations that can decide the committed bytes of a locked path are \
             not all readable here:\n{}",
            refused.concat()
        ));
    }
    if patterns.is_empty() {
        return Err(format!(
            ".gitattributes declares no `binary`/`-text` pathspec inside {ROOTS:?}, but \
             is_binary_artifact() classifies `.bin` artifacts as binary — the two have drifted"
        ));
    }
    if let Some(p) = patterns.iter().find(|p| {
        let body = p.strip_suffix("/**").unwrap_or(p.as_str());
        body.matches('*').count() > 1 || body.contains('?') || body.contains('[')
    }) {
        return Err(format!(
            ".gitattributes pathspec {p:?} uses a glob shape attr_pattern_matches() does not \
             implement; teach it that shape rather than letting the match silently fail"
        ));
    }
    let drift = binary_classification_violations(&patterns, paths.keys().map(String::as_str));
    if !drift.is_empty() {
        return Err(format!(
            "the `.gitattributes` declarations do not decide the locked paths' bytes the way \
             is_binary_artifact() says:\n{}",
            drift.concat()
        ));
    }
    Ok(())
}

/// The locked paths whose bytes the declared `binary` / `-text` pathspecs
/// (`patterns`) do not decide the way [`is_binary_artifact`] says: a path a
/// pathspec reaches only in another ASCII case (git applies it under
/// `core.ignorecase`, not on a case-sensitive checkout, so the stored bytes
/// would depend on the platform), and a path the predicate classifies
/// otherwise than the declaration. Parameterized so
/// `gitattributes_reader_refuses_what_it_cannot_see` can drive both red.
fn binary_classification_violations<'a>(
    patterns: &[String],
    paths: impl IntoIterator<Item = &'a str>,
) -> Vec<String> {
    let mut out = Vec::new();
    for rel in paths {
        let declared = patterns.iter().any(|p| attr_pattern_matches(p, rel));
        let folded = patterns
            .iter()
            .any(|p| attr_pattern_matches(&p.to_ascii_lowercase(), &rel.to_ascii_lowercase()));
        if folded != declared {
            out.push(format!(
                "  CASE-VARIANT PATHSPEC: {patterns:?} reaches {rel} only in another ASCII case, \
                 which git applies under core.ignorecase (on in this checkout) but not on a \
                 case-sensitive checkout — spell the pathspec in the case of the paths it names\n"
            ));
        } else if is_binary_artifact(rel) != declared {
            out.push(format!(
                "  BINARY CLASSIFICATION DRIFT: golden artifact {rel}: is_binary_artifact() says \
                 {}, .gitattributes says {declared} ({patterns:?}). The digest would be taken \
                 over bytes git does not store; keep the predicate and the attribute in \
                 lockstep.\n",
                is_binary_artifact(rel)
            ));
        }
    }
    out
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

/// What a regen did to a row, so the knob can announce it instead of writing a
/// provenance claim or a digest nobody reviewed.
enum Seeded {
    /// The path is new to the lock: the registers classified it, and the
    /// `capi_v0145` residue in particular is a claim that wants confirming.
    New(Anchor),
    /// The stored row disagreed with the registers and was reset to them.
    ReAnchored(Anchor, Anchor),
    /// The stored digest (carried here) differs from the bytes on disk and was
    /// rewritten. Announced under the check arm's `DIGEST MOVED` label, so a
    /// regen that moved no golden byte is provable from its own output (the
    /// RF-D08-06 audit found that claim unfalsifiable while regens were silent).
    DigestMoved(String),
    /// The stored `produced_by` (first) disagreed with [`LANE_INVARIANT`] and
    /// was reset to it (second); the anchor did not move.
    ReLaned(Option<ProducedBy>, Option<ProducedBy>),
    /// The stored row's artifact (stored anchor carried here) is gone from
    /// disk, so the row and the coverage it stood for leave the lock.
    Dropped(Anchor),
}

/// The stderr lines of a regen: the summary with the old and new row counts,
/// then one line per [`Seeded`] entry, so no provenance movement, digest move or
/// dropped row is silent.
fn regen_announcements(
    lock: &str,
    stored_rows: usize,
    written_rows: usize,
    report: &[(String, Seeded)],
) -> Vec<String> {
    let delta = written_rows as i64 - stored_rows as i64;
    let mut out = vec![format!(
        "golden lock regenerated: {lock} ({stored_rows} -> {written_rows} artifacts, {delta:+})"
    )];
    // A new path inherits the `capi_v0145` residue only because no register
    // claimed it, which is a claim about where its bytes came from and must be
    // confirmed by a human.
    out.extend(report.iter().map(|(p, what)| match what {
        Seeded::New(a) => format!(
            "  SEEDED {p} as {a:?} — confirm this artifact really came from that source; \
             if not, add it to the matching register in golden_lock.rs and regenerate"
        ),
        Seeded::ReAnchored(from, to) => {
            format!("  RE-ANCHORED {p}: {from:?} -> {to:?} (the registers are the truth)")
        }
        Seeded::DigestMoved(locked) => format!(
            "  DIGEST MOVED {p}: locked {locked} rewritten from disk — commit it only \
             together with the change that moved the bytes"
        ),
        Seeded::ReLaned(from, to) => format!(
            "  RE-LANED {p}: produced_by {from:?} -> {to:?} (the LANE_INVARIANT register is \
             the truth)"
        ),
        Seeded::Dropped(a) => format!(
            "  DROPPED {p} (was {a:?}): the artifact is gone from disk, so its row and the \
             coverage it stood for leave the lock — confirm the deletion was intended"
        ),
    }));
    out
}

/// Rebuild the row set from disk. Provenance always comes from
/// [`seed_metadata`] — the registers are the invariant, so a regen can neither
/// invent an anchor or a producing lane nor preserve a hand-edited one. Refuses
/// outright to write an unregistered `self` anchor: de-anchoring is a
/// [`DEANCHORED`] edit, never a regen side effect. Every stored row without an
/// artifact on disk is reported [`Seeded::Dropped`].
fn regenerate(
    disk: &BTreeMap<String, String>,
    stored: &BTreeMap<String, Artifact>,
    report: &mut Vec<(String, Seeded)>,
) -> Vec<Artifact> {
    let rows: Vec<Artifact> = disk
        .iter()
        .map(|(path, sha256)| {
            let registered = deanchored_entries(path);
            assert!(
                registered.len() <= 1,
                "{path} is covered by {} DEANCHORED entries; the register must assign exactly one \
                 reason per artifact",
                registered.len()
            );
            let (anchor, reason, produced_by) = seed_metadata(path);
            if let Some(prev) = stored.get(path) {
                assert!(
                    prev.anchor != Anchor::SelfSnapshot || anchor == Anchor::SelfSnapshot,
                    "{path} is anchored `self` in the lock but no DEANCHORED entry covers it. \
                     De-anchoring is a reviewed register edit, not a regen side effect — add the \
                     family (with its reason) to DEANCHORED in golden_lock.rs first."
                );
                if prev.anchor != anchor {
                    report.push((path.clone(), Seeded::ReAnchored(prev.anchor, anchor)));
                } else if prev.produced_by != produced_by {
                    report.push((path.clone(), Seeded::ReLaned(prev.produced_by, produced_by)));
                }
                if prev.sha256 != *sha256 {
                    report.push((path.clone(), Seeded::DigestMoved(prev.sha256.clone())));
                }
            } else {
                report.push((path.clone(), Seeded::New(anchor)));
            }
            Artifact {
                path: path.clone(),
                sha256: sha256.clone(),
                anchor,
                reason,
                produced_by,
            }
        })
        .collect();
    for (path, prev) in stored {
        if !disk.contains_key(path) {
            report.push((path.clone(), Seeded::Dropped(prev.anchor)));
        }
    }
    rows
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
        let lock = path.display().to_string();
        for line in regen_announcements(&lock, stored.len(), artifacts.len(), &report) {
            eprintln!("{line}");
        }
        return;
    }

    assert!(
        path.is_file(),
        "golden provenance lock missing: {} — bootstrap it with `{LOCK_REGEN_CMD}`",
        path.display()
    );
    let lock = read_lock(&path);
    let rows = by_path(&lock.artifacts);
    assert!(
        lock.artifacts.windows(2).all(|w| w[0].path < w[1].path),
        "{LOCK_PATH} is not sorted by path; regenerate it with `{LOCK_REGEN_CMD}`"
    );

    let mut diff = String::new();
    if lock.comment != COMMENT {
        diff.push_str(&format!(
            "  COMMENT OUT OF SYNC: the lock's comment is not golden_lock.rs::COMMENT (regenerate \
             with `{LOCK_REGEN_CMD}` after editing the const, never hand-edit the lock)\n"
        ));
    }

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
    let mut capi015_overlay_hits = vec![0usize; CAPI015_OVERLAYS.len()];
    let mut r4133_hits = vec![0usize; R4133_FAMILIES.len()];
    let mut fpc_hits = 0usize;
    let mut r3723_hits = 0usize;
    let mut non_engine_hits = vec![0usize; NON_ENGINE_RESIDUE.len()];
    let mut capi_v0145_overlay_hits = vec![0usize; CAPI_V0145_OVERLAYS.len()];
    let mut lane_invariant_hits = vec![0usize; LANE_INVARIANT.len()];
    for row in &lock.artifacts {
        for (i, (pattern, _)) in NON_ENGINE_RESIDUE.iter().enumerate() {
            if pattern_covers(pattern, &row.path) {
                non_engine_hits[i] += 1;
            }
        }
        for (i, (pattern, _)) in CAPI_V0145_OVERLAYS.iter().enumerate() {
            if pattern_covers(pattern, &row.path) {
                capi_v0145_overlay_hits[i] += 1;
            }
        }
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
                 `{LOCK_REGEN_CMD}` after editing the register)\n",
                row.path
            ));
        }
        for (i, (pattern, _)) in LANE_INVARIANT.iter().enumerate() {
            if pattern_covers(pattern, &row.path) {
                lane_invariant_hits[i] += 1;
            }
        }
        if CAPI015_ARTIFACTS.contains(&row.path.as_str()) {
            let i = CAPI015_ARTIFACTS
                .iter()
                .position(|p| *p == row.path)
                .expect("checked by contains");
            capi015_hits[i] += 1;
        }
        if let Some(i) = CAPI015_OVERLAYS.iter().position(|(p, _)| *p == row.path) {
            capi015_overlay_hits[i] += 1;
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
        // `anchor == self` <=> the artifact is (re)producible in-repo, by the
        // lane LANE_INVARIANT derives.
        if let Some(v) = produced_by_violation(row, LANE_INVARIANT) {
            diff.push_str(&v);
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
    for (i, (p, _)) in CAPI015_OVERLAYS.iter().enumerate() {
        if capi015_overlay_hits[i] == 0 {
            diff.push_str(&format!(
                "  STALE CAPI015_OVERLAYS ENTRY: {p:?} matches no locked artifact\n"
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
    for (i, (pattern, _)) in NON_ENGINE_RESIDUE.iter().enumerate() {
        if non_engine_hits[i] == 0 {
            diff.push_str(&format!(
                "  STALE NON_ENGINE_RESIDUE ENTRY: {pattern:?} covers no locked artifact\n"
            ));
        }
    }
    for (i, (pattern, _)) in CAPI_V0145_OVERLAYS.iter().enumerate() {
        if capi_v0145_overlay_hits[i] == 0 {
            diff.push_str(&format!(
                "  STALE CAPI_V0145_OVERLAYS ENTRY: {pattern:?} covers no locked artifact\n"
            ));
        }
    }
    for (i, (pattern, _)) in LANE_INVARIANT.iter().enumerate() {
        if lane_invariant_hits[i] == 0 {
            diff.push_str(&format!(
                "  STALE LANE_INVARIANT ENTRY: {pattern:?} covers no locked artifact\n"
            ));
        }
    }

    // (5) every JSON artifact's own declared provenance admits its anchor, over
    // the pinned declaring population.
    let declaring = declared_provenance_rows(&root, &lock.artifacts, &disk);
    for v in declared_provenance_violations(&declaring, DECLARED_PROVENANCE_EXCEPTIONS) {
        diff.push_str(&v);
    }
    if let Some(v) = declared_provenance_census_violation(&declaring, DECLARED_PROVENANCE_CENSUS) {
        diff.push_str(&v);
    }

    // The two capi015 rails, over what is on disk. The stamp rail binds the
    // register to the artifacts that declare capi015 themselves (both
    // directions — the converse also proves the reader found them). The
    // sidecar↔payload rail follows each capi015 `.meta.json` to the payloads
    // its generator call wrote, which carry no provenance block of their own.
    let stamped = capi015_stamped_artifacts(&root, &disk);
    let stamped: Vec<&str> = stamped.iter().map(String::as_str).collect();
    let stamped_sidecars: Vec<&str> = stamped
        .iter()
        .copied()
        .filter(|p| p.ends_with(".meta.json"))
        .collect();
    let on_disk: Vec<&str> = disk.keys().map(String::as_str).collect();
    for v in capi015_stamp_violations(&stamped, CAPI015_ARTIFACTS) {
        diff.push_str(&v);
    }
    for v in capi015_sidecar_violations(&on_disk, &stamped_sidecars, CAPI015_ARTIFACTS) {
        diff.push_str(&v);
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
         deliberately with:\n  {LOCK_REGEN_CMD}\n\
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
    // Every overlay names a member of the capi015 set (an overlay on anything
    // else would be a provenance claim no anchor backs), once.
    for (i, (path, reason)) in CAPI015_OVERLAYS.iter().enumerate() {
        assert!(
            CAPI015_ARTIFACTS.contains(path),
            "CAPI015_OVERLAYS entry {path:?} is not a capi015 artifact"
        );
        assert!(
            !reason.trim().is_empty(),
            "CAPI015_OVERLAYS entry {path:?} has no reason"
        );
        for (other, _) in CAPI015_OVERLAYS.iter().skip(i + 1) {
            assert_ne!(path, other, "CAPI015_OVERLAYS has duplicate entries");
        }
    }

    assert_eq!(
        CAPI015_ARTIFACTS.len(),
        17,
        "the capi015 set is closed at seventeen — the eleven provenance-stamped artifacts \
         GOLDEN_REBASE_PLAN.md G0.1 enumerated plus the six unstamped payloads their sidecars' \
         generator calls wrote (registered by RF-D08-06) — and never grows: that beta \
         environment is gone. A deletion is caught corpus-side by the STALE CAPI015_ARTIFACTS \
         sweep, a stamped capture dropped from the list (or a reader that finds no stamp) by \
         the capi015 stamp rail, and a payload/sidecar split by the capi015 sidecar↔payload \
         rail — they are what keeps this count from guarding a constant against itself. \
         Shrink them together, deliberately."
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
    // The DEANCHORED × CAPI015_ARTIFACTS pair (RF-D08-06): `seed_metadata`
    // consults DEANCHORED first, so without this arm one register edit could
    // re-anchor an unreproducible capi015 capture to a writable `self`.
    let shadowed = deanchored_capi015_overlaps(DEANCHORED, CAPI015_ARTIFACTS);
    assert!(
        shadowed.is_empty(),
        "DEANCHORED entries shadow capi015 artifacts {shadowed:?}: those bytes come from a dead \
         beta stack, so no family pattern may de-anchor them to `self`"
    );
    assert!(R3723_TREE.ends_with('/'));

    // LANE_INVARIANT names `self` families only, once each: an entry outside
    // DEANCHORED would give an oracle-anchored row a producing lane.
    for (i, (pattern, measurement)) in LANE_INVARIANT.iter().enumerate() {
        assert!(
            DEANCHORED.iter().any(|(d, _)| pattern_covers(d, pattern)),
            "LANE_INVARIANT entry {pattern:?} lies outside every DEANCHORED entry"
        );
        assert!(
            !measurement.trim().is_empty(),
            "LANE_INVARIANT entry {pattern:?} names no cross-lane measurement"
        );
        for (other, _) in LANE_INVARIANT.iter().skip(i + 1) {
            assert!(
                !patterns_overlap(pattern, other),
                "LANE_INVARIANT entries {pattern:?} and {other:?} overlap"
            );
        }
    }

    // A declared-provenance exception names one locked JSON artifact, keeps the
    // anchor its register derives, and says why.
    for (i, (path, anchor, reason)) in DECLARED_PROVENANCE_EXCEPTIONS.iter().enumerate() {
        assert!(
            path.ends_with(".json") && ROOTS.iter().any(|r| path.starts_with(r)),
            "DECLARED_PROVENANCE_EXCEPTIONS entry {path:?} is not a JSON artifact inside {ROOTS:?}"
        );
        assert_eq!(
            seed_metadata(path).0,
            *anchor,
            "DECLARED_PROVENANCE_EXCEPTIONS entry {path:?} keeps an anchor its register does not derive"
        );
        assert!(
            !reason.trim().is_empty(),
            "DECLARED_PROVENANCE_EXCEPTIONS entry {path:?} has no reason"
        );
        for (other, _, _) in DECLARED_PROVENANCE_EXCEPTIONS.iter().skip(i + 1) {
            assert_ne!(
                path, other,
                "DECLARED_PROVENANCE_EXCEPTIONS has duplicate entries"
            );
        }
    }

    // The two reason-only residue registers: well-formed, one reason per
    // artifact, and disjoint from every anchor-deciding register — an overlap
    // would be unreachable in `seed_metadata` (the anchor register wins), so its
    // reason would be a claim no row ever carries.
    let residue: Vec<(&str, &str, &str)> = NON_ENGINE_RESIDUE
        .iter()
        .map(|&(p, r)| ("NON_ENGINE_RESIDUE", p, r))
        .chain(
            CAPI_V0145_OVERLAYS
                .iter()
                .map(|&(p, r)| ("CAPI_V0145_OVERLAYS", p, r)),
        )
        .collect();
    for (i, &(register, pattern, reason)) in residue.iter().enumerate() {
        assert!(
            !pattern.starts_with('/') && !pattern.contains('\\'),
            "{register} pattern {pattern:?} must be repo-root-relative with forward slashes"
        );
        assert!(
            ROOTS.iter().any(|r| pattern.starts_with(r)),
            "{register} pattern {pattern:?} is outside the locked roots {ROOTS:?}"
        );
        assert!(
            !reason.trim().is_empty(),
            "{register} entry {pattern:?} has no reason"
        );
        for &(other_register, other, _) in residue.iter().skip(i + 1) {
            assert!(
                !patterns_overlap(pattern, other),
                "{register} entry {pattern:?} and {other_register} entry {other:?} overlap"
            );
        }
        let anchor_register = if DEANCHORED.iter().any(|(d, _)| patterns_overlap(d, pattern)) {
            Some("DEANCHORED")
        } else if CAPI015_ARTIFACTS
            .iter()
            .any(|c| patterns_overlap(c, pattern))
        {
            Some("CAPI015_ARTIFACTS")
        } else if R4133_FAMILIES
            .iter()
            .any(|(f, _)| patterns_overlap(f, pattern))
        {
            Some("R4133_FAMILIES")
        } else if patterns_overlap(FPC_ARTIFACT, pattern) {
            Some("FPC_ARTIFACT")
        } else if patterns_overlap(R3723_TREE, pattern) {
            Some("R3723_TREE")
        } else {
            None
        };
        assert_eq!(
            anchor_register, None,
            "{register} entry {pattern:?} overlaps an anchor-deciding register: a residue reason \
             can only describe a capi_v0145 row"
        );
    }

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

/// The DEANCHORED × CAPI015_ARTIFACTS arm of
/// [`provenance_registers_are_well_formed`], driven red: a family prefix or an
/// exact path that reaches a capi015 capture is caught and named, and the live
/// registers are clean.
#[test]
fn deanchored_never_shadows_a_capi015_artifact() {
    assert_eq!(
        deanchored_capi015_overlaps(DEANCHORED, CAPI015_ARTIFACTS),
        Vec::<(&str, &str)>::new()
    );

    // A family prefix swallowing ncim/ would re-anchor all six capi015 files
    // there (two sidecars + four payloads) to `self`.
    let family = [("tests/golden/ncim/", "would-be snapshot family")];
    assert_eq!(
        deanchored_capi015_overlaps(&family, CAPI015_ARTIFACTS),
        vec![
            ("tests/golden/ncim/", "tests/golden/ncim/pq.meta.json"),
            ("tests/golden/ncim/", "tests/golden/ncim/pq_Jacobian.csv"),
            ("tests/golden/ncim/", "tests/golden/ncim/pq_PV2PQ.txt"),
            (
                "tests/golden/ncim/",
                "tests/golden/ncim/pv_qlimit.meta.json"
            ),
            (
                "tests/golden/ncim/",
                "tests/golden/ncim/pv_qlimit_Jacobian.csv"
            ),
            (
                "tests/golden/ncim/",
                "tests/golden/ncim/pv_qlimit_PV2PQ.txt"
            ),
        ]
    );
    // An exact path is caught too.
    let exact = [("tests/golden/reports/export_capacity_seasonal.txt", "x")];
    assert_eq!(
        deanchored_capi015_overlaps(&exact, CAPI015_ARTIFACTS),
        vec![(
            "tests/golden/reports/export_capacity_seasonal.txt",
            "tests/golden/reports/export_capacity_seasonal.txt"
        )]
    );
    // A sibling family that stops short of the capi015 files is not.
    let sibling = [("tests/golden/reports/export_capacity.txt", "x")];
    assert!(deanchored_capi015_overlaps(&sibling, CAPI015_ARTIFACTS).is_empty());
}

/// The capi015 sidecar↔payload rail, driven red over a synthetic tree: an
/// unregistered payload, an unregistered stamped sidecar and a payload-less
/// sidecar each produce their named violation, near-miss names never count as
/// payloads, and a fully registered generator call is clean.
#[test]
fn capi015_sidecar_rail_reds_on_an_unregistered_payload() {
    let paths = [
        "tests/golden/ncim/pq.meta.json",
        "tests/golden/ncim/pq_Jacobian.csv",
        "tests/golden/ncim/pq_PV2PQ.txt",
        // near misses: no `.`/`_` boundary after the stem, or a subdirectory
        "tests/golden/ncim/pqx.txt",
        "tests/golden/ncim/pq_dir/x.txt",
        "tests/golden/ncim/other.txt",
    ];
    let full = [
        "tests/golden/ncim/pq.meta.json",
        "tests/golden/ncim/pq_Jacobian.csv",
        "tests/golden/ncim/pq_PV2PQ.txt",
    ];
    let stamped = ["tests/golden/ncim/pq.meta.json"];
    assert!(capi015_sidecar_violations(&paths, &stamped, &full).is_empty());

    // The G0.1 shape: only the sidecar registered — both payloads go red.
    let v = capi015_sidecar_violations(&paths, &stamped, &full[..1]);
    assert_eq!(v.len(), 2, "{v:?}");
    assert!(v[0].contains("CAPI015 PAYLOAD OFF REGISTER: tests/golden/ncim/pq_Jacobian.csv"));
    assert!(v[1].contains("CAPI015 PAYLOAD OFF REGISTER: tests/golden/ncim/pq_PV2PQ.txt"));

    // A stamped sidecar the register never heard of: itself plus its payloads.
    let v = capi015_sidecar_violations(&paths, &stamped, &[]);
    assert_eq!(v.len(), 3, "{v:?}");
    assert!(v[0].contains("UNREGISTERED CAPI015 SIDECAR: tests/golden/ncim/pq.meta.json"));

    // A registered sidecar whose payloads are gone.
    let v = capi015_sidecar_violations(&paths[..1], &stamped, &full[..1]);
    assert_eq!(v.len(), 1, "{v:?}");
    assert!(v[0].contains("CAPI015 SIDECAR WITHOUT PAYLOAD: tests/golden/ncim/pq.meta.json"));

    // The `<stem>.<ext>` shape of gen_reports.py.
    assert!(is_sidecar_payload(
        "tests/golden/reports/export_capacity_seasonal.meta.json",
        "tests/golden/reports/export_capacity_seasonal.txt"
    ));
    assert!(!is_sidecar_payload(
        "tests/golden/reports/export_capacity_seasonal.meta.json",
        "tests/golden/reports/export_capacity_seasonal.meta.json"
    ));
}

/// The capi015 stamp rail and its reader predicate, driven red: both on-disk
/// spellings are recognized (fixtures shaped like the real stamps of
/// `ncim/pq.meta.json` and `props/transformer_bh.json`) and the r4133 /
/// pinned-oracle spellings are not. A stamped dump dropped from the register
/// and a reader that finds nothing (the two holes the RF-D08-06 audit
/// mutation-proved) each red with their named violation, while a registered
/// sidecar's payloads need no stamp of their own.
#[test]
fn capi015_stamp_rail_reds_on_a_dropped_or_unread_capture() {
    let parse = |s: &str| -> serde_json::Value { serde_json::from_str(s).expect("fixture") };
    assert!(declares_capi015(&parse(
        r#"{"deck": ["clear"], "oracle": "capi015"}"#
    )));
    assert!(declares_capi015(&parse(
        r#"{"schema": 1, "oracle": {"engine_spec": "capi015", "engine": "DSS C-API 0.15.0b4"}}"#
    )));
    for other in [
        r#"{"deck": ["clear"], "oracle": "capi_v0145"}"#,
        r#"{"schema": 1, "oracle": {"engine_spec": "r4133"}}"#,
        r#"{"schema": 1, "oracle": {"engine_spec": "oddie:r4133", "rev": "r4133"}}"#,
        r#"{"schema": 1, "engine_spec": "capi015"}"#,
        r#"["capi015"]"#,
    ] {
        assert!(!declares_capi015(&parse(other)), "{other}");
    }

    let registered = [
        "tests/golden/ncim/pq.meta.json",
        "tests/golden/ncim/pq_Jacobian.csv",
        "tests/golden/props/transformer_bh.json",
    ];
    let stamped = [
        "tests/golden/ncim/pq.meta.json",
        "tests/golden/props/transformer_bh.json",
    ];
    assert!(capi015_stamp_violations(&stamped, &registered).is_empty());

    // A stamped dump dropped from the register (count and lock edited to match).
    let v = capi015_stamp_violations(&stamped, &registered[..2]);
    assert_eq!(v.len(), 1, "{v:?}");
    assert!(v[0].contains("UNREGISTERED CAPI015 CAPTURE: tests/golden/props/transformer_bh.json"));

    // A reader that finds nothing: every stamped registration reds, the
    // sidecar's payload does not.
    let v = capi015_stamp_violations(&[], &registered);
    assert_eq!(v.len(), 2, "{v:?}");
    assert!(v[0].contains("CAPI015 ARTIFACT WITHOUT STAMP: tests/golden/ncim/pq.meta.json"));
    assert!(
        v[1].contains("CAPI015 ARTIFACT WITHOUT STAMP: tests/golden/props/transformer_bh.json")
    );

    // A payload whose sidecar left the register is no longer excused.
    let v = capi015_stamp_violations(&stamped[1..], &registered[1..]);
    assert_eq!(v.len(), 1, "{v:?}");
    assert!(v[0].contains("CAPI015 ARTIFACT WITHOUT STAMP: tests/golden/ncim/pq_Jacobian.csv"));

    // An unregistered stamped sidecar is the sidecar rail's to report, together
    // with its payloads — not repeated here.
    assert!(capi015_stamp_violations(&stamped[..1], &[]).is_empty());
}

/// Expected-value pins of the classification this step corrected: the six
/// capi015 payloads carry their sidecars' anchor and reason, the two
/// reason-only registers put a reason on a row without moving it off the
/// `capi_v0145` residue (the nine hand-landed Dump goldens included), and their
/// unregistered neighbours keep the plain residue.
#[test]
fn residue_reasons_and_capi015_payloads_classify_as_registered() {
    let payloads: Vec<&str> = CAPI015_ARTIFACTS
        .iter()
        .copied()
        .filter(|p| CAPI015_ARTIFACTS.iter().any(|s| is_sidecar_payload(s, p)))
        .collect();
    assert_eq!(
        payloads,
        [
            "tests/golden/ncim/pq_Jacobian.csv",
            "tests/golden/ncim/pq_PV2PQ.txt",
            "tests/golden/ncim/pv_qlimit_Jacobian.csv",
            "tests/golden/ncim/pv_qlimit_PV2PQ.txt",
            "tests/golden/reports/export_capacity_seasonal.txt",
            "tests/golden/reports/export_overloads_seasonal.txt",
        ]
    );
    for payload in payloads {
        let sidecar = CAPI015_ARTIFACTS
            .iter()
            .find(|s| is_sidecar_payload(s, payload))
            .expect("every capi015 payload has its registered sidecar");
        assert_eq!(seed_metadata(payload), seed_metadata(sidecar));
        assert_eq!(seed_metadata(payload).0, Anchor::Capi015);
    }

    let (anchor, reason, produced_by) =
        seed_metadata("tests/golden/feeders_controlsoff/ieee13_controlsoff.dss");
    assert_eq!((anchor, produced_by), (Anchor::CapiV0145, None));
    assert!(reason.starts_with("input deck written by tools/golden/gen_feeders_controlsoff.py"));
    // The oracle output of that family stays the plain residue.
    assert_eq!(
        seed_metadata("tests/golden/feeders_controlsoff.json"),
        (Anchor::CapiV0145, String::new(), None)
    );

    let (anchor, reason, produced_by) = seed_metadata("tests/golden/json/schema_divergences.json");
    assert_eq!((anchor, produced_by), (Anchor::CapiV0145, None));
    assert!(reason.starts_with("hand-authored inventory"));

    let (anchor, reason, produced_by) = seed_metadata("tests/golden/reports/dump3_commands.txt");
    assert_eq!((anchor, produced_by), (Anchor::CapiV0145, None));
    assert!(reason.starts_with("PARTLY HAND-LANDED"));
    // Its deck sidecar is the untouched pinned-oracle capture.
    assert_eq!(
        seed_metadata("tests/golden/reports/dump3_commands.meta.json"),
        (Anchor::CapiV0145, String::new(), None)
    );
    // The eight other Dump goldens 702adcbd hand-landed lines into (the
    // settlement's AC-1), each naming what a pinned-oracle regen would revert;
    // an untouched Dump golden of the same generator keeps the plain residue.
    for (path, landed) in [
        ("tests/golden/reports/dump3_bare.txt", "BHPoints"),
        ("tests/golden/reports/dump3_debug.txt", "IdleReverse"),
        ("tests/golden/reports/dump_autotrans.txt", "BHFlux"),
        ("tests/golden/reports/dump_autotrans3.txt", "BHFlux"),
        (
            "tests/golden/reports/dump_regcontrol.txt",
            "RevThreshold=-100",
        ),
        ("tests/golden/reports/dump_transformer.txt", "BHCurrent"),
        ("tests/golden/reports/dump_transformer3.txt", "BHCurrent"),
        (
            "tests/golden/reports/dump_transformer_disabled.txt",
            "BHPoints",
        ),
    ] {
        let (anchor, reason, produced_by) = seed_metadata(path);
        assert_eq!((anchor, produced_by), (Anchor::CapiV0145, None), "{path}");
        assert!(
            reason.starts_with("PARTLY HAND-LANDED")
                && reason.contains("702adcbd")
                && reason.contains(landed),
            "{path}: {reason}"
        );
    }
    assert_eq!(
        seed_metadata("tests/golden/reports/dump_loadshape.txt"),
        (Anchor::CapiV0145, String::new(), None)
    );

    // The born-self schema reason no longer calls the divergence inventory a
    // frozen capi_v0145 capture.
    let (_, schema_port_reason, _) = seed_metadata("tests/golden/json/schema_full_port.json");
    assert!(!schema_port_reason.contains("frozen"));
}

/// A regen that loses a row says so: every stored row whose artifact is gone is
/// announced `DROPPED` with its stored anchor, and the summary line carries the
/// old and new row counts, so the direction that loses coverage is as loud as
/// the `SEEDED` direction.
#[test]
fn regen_announces_every_dropped_row_and_the_row_count_delta() {
    let row = |path: &str| Artifact {
        path: path.to_string(),
        sha256: "aa".to_string(),
        anchor: Anchor::CapiV0145,
        reason: String::new(),
        produced_by: None,
    };
    let kept = "tests/golden/reports/dump_loadshape.txt";
    let gone = "tests/golden/reports/export_vanished.txt";
    let stored = by_path(&[row(kept), row(gone)]);
    let disk: BTreeMap<String, String> = [(kept.to_string(), "aa".to_string())].into();

    let mut report = Vec::new();
    let rows = regenerate(&disk, &stored, &mut report);
    assert_eq!(rows, [row(kept)]);
    let lines = regen_announcements("lock", stored.len(), rows.len(), &report);
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert_eq!(
        lines[0],
        "golden lock regenerated: lock (2 -> 1 artifacts, -1)"
    );
    assert!(
        lines[1].starts_with(
            "  DROPPED tests/golden/reports/export_vanished.txt (was CapiV0145): the artifact is \
             gone from disk"
        ),
        "{lines:?}"
    );

    // A regen that changes nothing announces nothing but the summary.
    let mut report = Vec::new();
    let rows = regenerate(&disk, &by_path(&[row(kept)]), &mut report);
    assert_eq!(
        regen_announcements("lock", 1, rows.len(), &report),
        ["golden lock regenerated: lock (1 -> 1 artifacts, +0)"]
    );

    // The new-path direction, for contrast: SEEDED and a positive delta.
    let mut report = Vec::new();
    let rows = regenerate(&disk, &BTreeMap::new(), &mut report);
    let lines = regen_announcements("lock", 0, rows.len(), &report);
    assert_eq!(
        lines[0],
        "golden lock regenerated: lock (0 -> 1 artifacts, +1)"
    );
    assert!(lines[1].starts_with("  SEEDED tests/golden/reports/dump_loadshape.txt as CapiV0145"));
}

/// `produced_by` is a register decision like the anchor: a `self` row names the
/// lane [`LANE_INVARIANT`] derives, so a hand-flipped `lane-invariant` reds, a
/// regen resets it and announces `RE-LANED`, and only a register entry makes a
/// family lane-invariant.
#[test]
fn produced_by_is_derived_from_the_lane_invariant_register() {
    let recloser = "tests/golden/props/recloser.json";
    let self_row = |produced_by| Artifact {
        path: recloser.to_string(),
        sha256: "aa".to_string(),
        anchor: Anchor::SelfSnapshot,
        reason: String::new(),
        produced_by,
    };
    let parity = self_row(Some(ProducedBy::Parity));
    let flipped = self_row(Some(ProducedBy::LaneInvariant));

    // The live register lists no family: every self row is parity-produced.
    assert_eq!(seed_metadata(recloser).2, Some(ProducedBy::Parity));
    assert_eq!(produced_by_violation(&parity, LANE_INVARIANT), None);

    // A hand-flipped `lane-invariant` reds...
    let v = produced_by_violation(&flipped, LANE_INVARIANT).expect("a hand-flipped row reds");
    assert!(
        v.contains("PRODUCED_BY OUT OF REGISTER: tests/golden/props/recloser.json"),
        "{v}"
    );
    // ...and a regen resets it to the register's lane, loudly.
    let disk: BTreeMap<String, String> = [(recloser.to_string(), "aa".to_string())].into();
    let mut report = Vec::new();
    let rows = regenerate(&disk, &by_path(std::slice::from_ref(&flipped)), &mut report);
    assert_eq!(rows[0].produced_by, Some(ProducedBy::Parity));
    let lines = regen_announcements("lock", 1, rows.len(), &report);
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(
        lines[1].starts_with(
            "  RE-LANED tests/golden/props/recloser.json: produced_by Some(LaneInvariant) -> \
             Some(Parity)"
        ),
        "{lines:?}"
    );

    // A register entry is what makes a family lane-invariant; then the parity
    // spelling is the one out of register.
    let measured = [(recloser, "a cross-lane regen measured both lanes identical")];
    assert_eq!(
        producing_lane(recloser, &measured),
        ProducedBy::LaneInvariant
    );
    assert_eq!(
        producing_lane("tests/golden/props/relay.json", &measured),
        ProducedBy::Parity
    );
    assert_eq!(produced_by_violation(&flipped, &measured), None);
    assert!(
        produced_by_violation(&parity, &measured)
            .is_some_and(|v| v.contains("PRODUCED_BY OUT OF REGISTER"))
    );

    // The iff rule keeps its own message, in both directions.
    assert!(
        produced_by_violation(&self_row(None), LANE_INVARIANT)
            .is_some_and(|v| v.contains("PRODUCED_BY MISMATCH"))
    );
    let oracle_row = Artifact {
        anchor: Anchor::CapiV0145,
        ..parity.clone()
    };
    assert!(
        produced_by_violation(&oracle_row, LANE_INVARIANT)
            .is_some_and(|v| v.contains("PRODUCED_BY MISMATCH"))
    );
}

/// The regen command const [`LOCK_REGEN_CMD`] and its one spelling in
/// [`COMMENT`], which the lock carries verbatim, carry `-- --nocapture`: the
/// regen run passes, and libtest discards a passing test's stderr,
/// announcements included. The messages are not read here: each formats the
/// const instead of spelling the command.
#[test]
fn the_regen_command_const_and_comment_carry_nocapture() {
    assert!(
        LOCK_REGEN_CMD
            .starts_with("DSS_UPDATE_GOLDEN_LOCK=1 cargo test -p dss-core --test golden_lock")
            && LOCK_REGEN_CMD.ends_with(" -- --nocapture"),
        "{LOCK_REGEN_CMD}"
    );
    let spelled = COMMENT.matches("--test golden_lock").count();
    assert_eq!(
        spelled, 1,
        "COMMENT spells the regen command {spelled} times"
    );
    assert_eq!(
        COMMENT.matches(LOCK_REGEN_CMD).count(),
        spelled,
        "COMMENT spells a regen command without `-- --nocapture`"
    );
}

/// The `.gitattributes` reader refuses every declaration in the tree that could
/// decide a locked path's bytes but that it would not read: a nested attribute
/// file (inside a root, or probed in each directory above one), an unrooted
/// pathspec that can reach a locked root, an attribute the classifier does not
/// emulate on a locked path, and a macro or quoted pathspec, in any ASCII case.
/// A pathspec that reaches a locked path only in another case reds, and so does
/// a classification drift. The check the scan runs reds each of these in its
/// order. The live shapes stay readable.
#[test]
fn gitattributes_reader_refuses_what_it_cannot_see() {
    let read = |text: &str| gitattributes_binary_patterns(text, &[]);
    let live = "# comment\n\ntests/corpus/electricdss-tst/** -text\ntests/corpus/SHA256SUMS -text\n\
                tests/golden/reports/*.bin binary\n";
    assert_eq!(
        read(live),
        (vec!["tests/golden/reports/*.bin".to_string()], Vec::new())
    );
    // A leading `/` anchors at the repo root, like no slash at all.
    assert_eq!(
        read("/tests/golden/reports/*.txt -text").0,
        ["tests/golden/reports/*.txt"]
    );
    // Attributes that leave the stored bytes alone, and a root-name near miss.
    for quiet in [
        "*.py diff=python",
        "tests/golden/reports/*.txt -diff linguist-generated",
        "tests/goldenX/*.bin binary",
    ] {
        assert_eq!(read(quiet), (Vec::new(), Vec::new()), "{quiet}");
    }

    for (line, label) in [
        ("*.txt -text", "UNROOTED PATHSPEC REACHES A LOCKED PATH"),
        (
            "SHA256SUMS -text",
            "UNROOTED PATHSPEC REACHES A LOCKED PATH",
        ),
        ("*.csv eol=crlf", "UNROOTED PATHSPEC REACHES A LOCKED PATH"),
        (
            "**/reports/*.txt -text",
            "UNROOTED PATHSPEC REACHES A LOCKED PATH",
        ),
        (
            "*/golden/reports/x.txt binary",
            "UNROOTED PATHSPEC REACHES A LOCKED PATH",
        ),
        (
            "tests/*/reports/*.txt -text",
            "UNROOTED PATHSPEC REACHES A LOCKED PATH",
        ),
        (
            "tests/golden*/x.txt !text",
            "UNROOTED PATHSPEC REACHES A LOCKED PATH",
        ),
        (
            "crates/dss-core/tests/data/*/r3723_ref/x.txt filter=lfs",
            "UNROOTED PATHSPEC REACHES A LOCKED PATH",
        ),
        (
            "Tests/*/reports/*.txt -text",
            "UNROOTED PATHSPEC REACHES A LOCKED PATH",
        ),
        (
            "TESTS/GOLDEN/reports/*.txt eol=crlf",
            "UNEMULATED ATTRIBUTE ON A LOCKED PATH",
        ),
        (
            "tests/golden/json/*.json eol=crlf",
            "UNEMULATED ATTRIBUTE ON A LOCKED PATH",
        ),
        (
            "tests/golden/reports/*.txt text",
            "UNEMULATED ATTRIBUTE ON A LOCKED PATH",
        ),
        (
            "tests/golden/reports/*.bin binary filter=lfs",
            "UNEMULATED ATTRIBUTE ON A LOCKED PATH",
        ),
        ("[attr]raw -text -diff", "UNREAD DECLARATION"),
        ("\"tests/golden/a b.txt\" -text", "UNREAD DECLARATION"),
    ] {
        let (patterns, refused) = read(line);
        assert!(patterns.is_empty(), "{line}: {patterns:?}");
        assert_eq!(refused.len(), 1, "{line}: {refused:?}");
        assert!(
            refused[0].contains(label) && refused[0].contains(line),
            "{line}: {refused:?}"
        );
    }

    // A case-variant pathspec is read as rooted, and the classifier reds every
    // locked path it reaches only in another case; the live one stays clean,
    // and a `.bin` outside it or a declared non-`.bin` drifts.
    for (line, rel) in [
        (
            "Tests/Golden/reports/*.txt -text",
            "tests/golden/reports/x.txt",
        ),
        (
            "tests/golden/reports/*.TXT -text",
            "tests/golden/reports/x.txt",
        ),
        (
            "Crates/dss-core/tests/data/adiakoptics/r3723_ref/ieee13/*.csv -text",
            "crates/dss-core/tests/data/adiakoptics/r3723_ref/ieee13/zll.csv",
        ),
        (
            "tests/golden/reports/*.bin binary",
            "tests/golden/reports/X.BIN",
        ),
    ] {
        let (patterns, refused) = read(line);
        assert!(refused.is_empty(), "{line}: {refused:?}");
        assert_eq!(patterns.len(), 1, "{line}: {patterns:?}");
        let v = binary_classification_violations(&patterns, [rel]);
        assert_eq!(v.len(), 1, "{line}: {v:?}");
        assert!(
            v[0].contains("CASE-VARIANT PATHSPEC") && v[0].contains(rel),
            "{line}: {v:?}"
        );
    }
    let bin = read(live).0;
    assert_eq!(
        binary_classification_violations(
            &bin,
            ["tests/golden/reports/x.bin", "tests/golden/reports/x.txt"]
        ),
        Vec::<String>::new()
    );
    for (patterns, rel) in [
        (bin, "tests/golden/json/x.bin"),
        (
            read("tests/golden/reports/* -text").0,
            "tests/golden/reports/x.txt",
        ),
    ] {
        let v = binary_classification_violations(&patterns, [rel]);
        assert_eq!(v.len(), 1, "{rel}: {v:?}");
        assert!(
            v[0].contains("BINARY CLASSIFICATION DRIFT: golden artifact") && v[0].contains(rel),
            "{rel}: {v:?}"
        );
    }

    // The check the scan runs passes the live shape and reds each failing
    // input with its own message.
    let tree = |paths: &[&str]| -> BTreeMap<String, String> {
        paths
            .iter()
            .map(|p| (p.to_string(), "d".to_string()))
            .collect()
    };
    let clean = tree(&["tests/golden/reports/x.bin", "tests/golden/reports/x.txt"]);
    let case_variant = tree(&["tests/golden/reports/X.BIN"]);
    let drifted = tree(&["tests/golden/json/x.bin"]);
    assert_eq!(check_binary_classification(live, |_| false, &clean), Ok(()));
    for (text, ancestors, paths, label) in [
        (
            live,
            true,
            &clean,
            "NESTED .gitattributes: crates/.gitattributes",
        ),
        (
            "*.py diff=python\n",
            false,
            &clean,
            "declares no `binary`/`-text` pathspec",
        ),
        (
            "tests/golden/reports/*.b*n binary\n",
            false,
            &clean,
            "uses a glob shape attr_pattern_matches() does not implement",
        ),
        (
            live,
            false,
            &case_variant,
            "CASE-VARIANT PATHSPEC: [\"tests/golden/reports/*.bin\"] reaches \
             tests/golden/reports/X.BIN",
        ),
        (
            live,
            false,
            &drifted,
            "BINARY CLASSIFICATION DRIFT: golden artifact tests/golden/json/x.bin",
        ),
    ] {
        let e = check_binary_classification(text, |_| ancestors, paths).expect_err(label);
        assert!(e.contains(label), "{label}: {e}");
    }

    // A nested attribute file is refused whatever the root file says.
    let nested = ["tests/golden/reports/.gitattributes".to_string()];
    let (patterns, refused) = gitattributes_binary_patterns(live, &nested);
    assert_eq!(patterns, ["tests/golden/reports/*.bin"]);
    assert_eq!(refused.len(), 1, "{refused:?}");
    assert!(refused[0].contains("NESTED .gitattributes: tests/golden/reports/.gitattributes"));
    // The finder reports the scanned ones in any case, and probes exactly the
    // directories between the repo root and each locked root.
    let scanned: BTreeMap<String, String> = [
        ("tests/golden/x/.gitattributes", "d"),
        ("tests/golden/y/.GitAttributes", "d"),
        ("tests/golden/x/a.gitattributes.txt", "d"),
    ]
    .map(|(p, d)| (p.to_string(), d.to_string()))
    .into();
    assert_eq!(
        nested_gitattributes(&scanned, |_| false),
        [
            "tests/golden/x/.gitattributes",
            "tests/golden/y/.GitAttributes"
        ]
    );
    assert_eq!(
        nested_gitattributes(&BTreeMap::new(), |_| true),
        [
            "crates/.gitattributes",
            "crates/dss-core/.gitattributes",
            "crates/dss-core/tests/.gitattributes",
            "crates/dss-core/tests/data/.gitattributes",
            "crates/dss-core/tests/data/adiakoptics/.gitattributes",
            "tests/.gitattributes",
        ]
    );
    let present = [
        "tests/.gitattributes",
        "crates/dss-core/tests/data/.gitattributes",
        "tests/corpus/.gitattributes",
    ];
    assert_eq!(
        nested_gitattributes(&BTreeMap::new(), |p| present.contains(&p)),
        [
            "crates/dss-core/tests/data/.gitattributes",
            "tests/.gitattributes"
        ]
    );
    // The real directories above each locked root hold none.
    assert_eq!(
        nested_gitattributes(&BTreeMap::new(), |p| repo_root().join(p).is_file()),
        Vec::<String>::new()
    );
}

/// The declared-provenance rail: every engine spelling on disk classifies, an
/// artifact whose own block names another engine than its anchor reds (the
/// residue swallowing an r4133 capture included), a `self` row is exempt, and
/// the one exception excuses exactly `flicker/pst_demo.json` and goes stale
/// when it excuses nothing. A value naming several engines reds unless a
/// single-engine `engine_spec` of the artifact picks one, an `oracle` block
/// without an engine name reds, and the census reds on any move.
#[test]
fn declared_provenance_must_admit_the_locked_anchor() {
    for (value, anchor) in [
        ("capi015", Anchor::Capi015),
        (
            "DSS C-API Library version 0.15.0b4 revision e936d2101d745e0f0ea6872d7a",
            Anchor::Capi015,
        ),
        (
            "dss_capi 0.15.0b4 (OpenDSS SVN r4103); regenerated for WP-U1.6 D12",
            Anchor::Capi015,
        ),
        (
            "DSS C-API Library version 0.14.5 revision 87d85c2622c8281b92255335bc7c09b11191b21d \
             based on OpenDSS SVN 3723 [FPC 3.2.2]",
            Anchor::CapiV0145,
        ),
        ("official-EPRI-r3723 (Oddie)", Anchor::R3723),
        ("oddie:r4133", Anchor::R4133),
        ("r4133", Anchor::R4133),
        (
            "Version 11.0.0.1 (64-bit build) - Charlottesville; License Status: Open",
            Anchor::R4133,
        ),
        (
            "r4133 bridge (dss-epri) + wm4model.dll native twin",
            Anchor::R4133,
        ),
        (
            "EPRI OpenDSS r4133 (v11.0.0.1 Charlottesville) via AltDSS Oddie",
            Anchor::R4133,
        ),
    ] {
        assert_eq!(declared_anchors(value), [anchor], "{value}");
    }
    assert!(declared_anchors("OpenDSS 9.4 (Oddie)").is_empty());
    // A value that names several engines names them all.
    assert_eq!(
        declared_anchors(
            "EPRI OpenDSS r4133 via AltDSS Oddie; the pinned 0.14.5 oracle cannot render this \
             surface"
        ),
        [Anchor::CapiV0145, Anchor::R4133]
    );
    assert_eq!(
        declared_anchors("official-EPRI-r3723 (Oddie); payload byte-identical under r4133"),
        [Anchor::R3723, Anchor::R4133]
    );

    let parse = |s: &str| -> serde_json::Value { serde_json::from_str(s).expect("fixture") };
    let some = |s: &str| Some(s.to_string());
    assert_eq!(
        provenance_declarations(&parse(
            r#"{"oracle": "capi015", "engine": "DSS C-API 0.15.0b4"}"#
        )),
        [
            ("engine", some("DSS C-API 0.15.0b4")),
            ("oracle", some("capi015"))
        ]
    );
    assert_eq!(
        provenance_declarations(&parse(
            r#"{"oracle": {"engine_spec": "r4133", "engine": "Version 11.0.0.1", "rev": "r4133"}}"#
        )),
        [
            ("oracle.engine_spec", some("r4133")),
            ("oracle.engine", some("Version 11.0.0.1"))
        ]
    );
    assert_eq!(
        provenance_declarations(&parse(r#"{"engine": 3, "oracle": null}"#)),
        [("engine", None), ("oracle", None)]
    );
    // An `oracle` block without an engine name is a declaration without one.
    assert_eq!(
        provenance_declarations(&parse(
            r#"{"oracle": {"tool": "x", "rev": "r4133"}, "data": 1}"#
        )),
        [("oracle", None)]
    );
    assert!(provenance_declarations(&parse(r#"{"data": {"engine": "r4133"}}"#)).is_empty());
    assert!(provenance_declarations(&parse(r#"["engine", "oracle"]"#)).is_empty());

    let declaring = |path: &str, anchor, value: &str| Declaring {
        path: path.to_string(),
        anchor,
        declarations: vec![("oracle.engine", some(value))],
    };
    let pst = "tests/golden/flicker/pst_demo.json";
    let exceptions = [(pst, Anchor::R4133, "byte-proven regen")];
    let clean = [
        declaring(
            "tests/golden/checkpoints/ieee13_daily.json",
            Anchor::CapiV0145,
            "DSS C-API Library version 0.14.5",
        ),
        declaring(
            "tests/golden/props/relay.json",
            Anchor::SelfSnapshot,
            "EPRI OpenDSS r4133 via AltDSS Oddie",
        ),
        declaring(pst, Anchor::R4133, "official-EPRI-r3723 (Oddie)"),
    ];
    assert_eq!(
        declared_provenance_violations(&clean, &exceptions),
        Vec::<String>::new()
    );

    // An r4133 capture no register names falls to the capi_v0145 residue: red.
    let v = declared_provenance_violations(
        &[declaring(
            "tests/golden/protection/unregistered.json",
            Anchor::CapiV0145,
            "oddie:r4133",
        )],
        &[],
    );
    assert_eq!(v.len(), 1, "{v:?}");
    assert!(v[0].contains(
        "DECLARED PROVENANCE CONTRADICTS ANCHOR: tests/golden/protection/unregistered.json is \
         locked CapiV0145"
    ));

    // Without its exception pst_demo reds; an exception for another anchor
    // excuses nothing and goes stale; so does one whose artifact needs none.
    let v = declared_provenance_violations(&clean, &[]);
    assert_eq!(v.len(), 1, "{v:?}");
    assert!(
        v[0].contains("DECLARED PROVENANCE CONTRADICTS ANCHOR: tests/golden/flicker/pst_demo.json")
    );
    let v = declared_provenance_violations(&clean, &[(pst, Anchor::Capi015, "x")]);
    assert_eq!(v.len(), 2, "{v:?}");
    assert!(v[0].contains("CONTRADICTS ANCHOR: tests/golden/flicker/pst_demo.json"));
    assert!(v[1].contains(
        "STALE DECLARED_PROVENANCE_EXCEPTIONS ENTRY: \"tests/golden/flicker/pst_demo.json\""
    ));
    let v = declared_provenance_violations(&clean[..2], &exceptions);
    assert_eq!(v.len(), 1, "{v:?}");
    assert!(v[0].contains("STALE DECLARED_PROVENANCE_EXCEPTIONS ENTRY"));

    // An unknown engine and a declaration without a string engine name red,
    // self row or not.
    let odd = Declaring {
        path: "tests/golden/props/relay.json".to_string(),
        anchor: Anchor::SelfSnapshot,
        declarations: vec![("engine", some("OpenDSS 9.4 (Oddie)")), ("oracle", None)],
    };
    let v = declared_provenance_violations(&[odd], &[]);
    assert_eq!(v.len(), 2, "{v:?}");
    assert!(v[0].contains(
        "UNCLASSIFIED PROVENANCE DECLARATION: tests/golden/props/relay.json declares `engine`"
    ));
    assert!(v[1].contains(
        "UNREADABLE PROVENANCE DECLARATION: tests/golden/props/relay.json declares `oracle` \
         without a string engine name"
    ));

    // A value naming several engines declares none of them: an r4133 capture
    // whose prose also names the pinned oracle cannot pass as the residue, and
    // only a single-engine `engine_spec` among its engines settles it.
    let several = "EPRI OpenDSS r4133 via AltDSS Oddie; the pinned 0.14.5 oracle cannot \
                   render this surface";
    let v = declared_provenance_violations(
        &[declaring(
            "tests/golden/protection/unregistered.json",
            Anchor::CapiV0145,
            several,
        )],
        &[],
    );
    assert_eq!(v.len(), 1, "{v:?}");
    assert!(v[0].contains(
        "AMBIGUOUS PROVENANCE DECLARATION: tests/golden/protection/unregistered.json declares \
         `oracle.engine`"
    ));
    let specified = |spec: &str, anchor| Declaring {
        path: "tests/golden/props/swtcontrol.json".to_string(),
        anchor,
        declarations: vec![
            ("oracle.engine_spec", some(spec)),
            ("oracle.engine", some(several)),
        ],
    };
    assert_eq!(
        declared_provenance_violations(&[specified("r4133", Anchor::R4133)], &[]),
        Vec::<String>::new()
    );
    let v = declared_provenance_violations(&[specified("r3723", Anchor::R3723)], &[]);
    assert_eq!(v.len(), 1, "{v:?}");
    assert!(v[0].contains("AMBIGUOUS PROVENANCE DECLARATION"));
    let v = declared_provenance_violations(&[specified("r4133", Anchor::CapiV0145)], &[]);
    assert_eq!(v.len(), 1, "{v:?}");
    assert!(v[0].contains("DECLARED PROVENANCE CONTRADICTS ANCHOR"));
    // Only a single-engine value under an `engine_spec` key picks: a
    // multi-engine `engine_spec` does not settle itself, and a single-engine
    // `engine` beside the multi-engine value does not settle it.
    for declarations in [
        vec![("oracle.engine_spec", some(several))],
        vec![
            ("engine", some("DSS C-API Library version 0.14.5")),
            ("oracle.engine", some(several)),
        ],
    ] {
        let key = declarations.last().map(|(key, _)| *key).expect("fixture");
        let d = Declaring {
            path: "tests/golden/protection/unregistered.json".to_string(),
            anchor: Anchor::CapiV0145,
            declarations,
        };
        let v = declared_provenance_violations(&[d], &[]);
        assert_eq!(v.len(), 1, "{key}: {v:?}");
        assert!(
            v[0].contains(&format!(
                "AMBIGUOUS PROVENANCE DECLARATION: tests/golden/protection/unregistered.json \
                 declares `{key}`"
            )),
            "{key}: {v:?}"
        );
    }

    // The census counts declaring rows and declarations per key, and reds on
    // any move.
    assert_eq!(
        declared_provenance_census(&clean),
        (3, vec![("oracle.engine", 3)])
    );
    assert_eq!(
        declared_provenance_census_violation(&clean, (3, &[("oracle.engine", 3)])),
        None
    );
    for pinned in [
        (2, &[("oracle.engine", 3)][..]),
        (3, &[("oracle.engine", 2), ("oracle.engine_spec", 1)][..]),
        (3, &[("engine", 3)][..]),
    ] {
        let v = declared_provenance_census_violation(&clean, pinned);
        assert!(
            v.as_deref()
                .is_some_and(|v| v.contains("DECLARED PROVENANCE CENSUS MOVED")),
            "{pinned:?}: {v:?}"
        );
    }

    // The live register holds exactly the one reasoned exception.
    let live: Vec<(&str, Anchor)> = DECLARED_PROVENANCE_EXCEPTIONS
        .iter()
        .map(|(p, a, _)| (*p, *a))
        .collect();
    assert_eq!(live, [(pst, Anchor::R4133)]);
}
