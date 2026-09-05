//! CI grep gate for the Stage F lane split (`DE_PASCALIZE_PLAN.md` Part IV.2,
//! "Mechanism" + §Verification): the `oracle-parity` cfg string must appear
//! **only** inside the per-crate `compat` modules — plus test code, where the
//! harness is allowed to branch per lane.
//!
//! Why a test and not a habit: the whole value of the split is that the engine
//! stays one readable code base with the lane difference concentrated in three
//! small files. One `#[cfg(feature = "oracle-parity")]` sprinkled into an
//! element or a report writer starts the cfg spaghetti the plan forbids
//! (forbidden move 4), and it would silently split behavior in a place no
//! per-kernel test covers.
//!
//! Failure means: move the branch into the crate's `compat` module and let the
//! call site use an unconditional `compat::` alias.
//!
//! The same file also polices the **compat tag** (`CLAUDE.md`: "it must stay
//! greppable"): the tag is an *index* of the places that deliberately reproduce
//! an upstream inexactness, and Stage F's exit criterion is a count of it.
//! Prose mentions — cross-references, "this is NOT one of those" disclaimers,
//! continuation lines — inflate that count and make the grep unusable as an
//! index, so they are rejected here. (This file spells the tag only at runtime,
//! so it does not trip its own gate.)

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// The cfg string this gate polices, assembled at runtime so the gate file
/// itself does not contain a literal occurrence to whitelist.
fn needle() -> String {
    format!("feature = {:?}", "oracle-parity")
}

fn repo_root() -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "..", ".."].iter().collect()
}

/// Directory names the source walk never descends into.
///
/// `target` and `node_modules` are build output. `.git`, `.inputs` and `.venv`
/// are VCS or vendored trees (the latter two are Windows **junctions** into the
/// main checkout — see `CLAUDE.md`, "Git worktrees"). `.claude` holds the
/// parallel-agent worktrees, which are *complete copies of this repository*:
/// descending there would report every marker in the tree two or three times
/// and fail this gate on an otherwise clean checkout.
///
/// These six are skipped at **any** depth; the scratch root is
/// [`SKIP_DIRS_AT_ROOT`] instead.
const SKIP_DIRS: &[&str] = &[
    ".git",
    ".claude",
    ".inputs",
    ".venv",
    "target",
    "node_modules",
];

/// Directory names the walk skips **only directly under the repository root**.
///
/// `tmp` is the gitignored scratch root (`.gitignore`'s `/tmp`) where sub-steps
/// park probe crates, renamed fixtures and half-applied patches. Rust left
/// there is not product code and makes no claim about this tree, but the walk
/// below is the WHOLE repository by design, so before RP3.8 added this entry a
/// scratch `.rs` under `tmp/` was scanned for teardown markers, compat tags and
/// lane `cfg`s — i.e. a throwaway file could red the mandatory gate, and one did
/// (RP3.8 P2b, worked around by renaming the file to `.rs.txt`).
///
/// Anchored at the root on purpose (RP3.8 audit settlement): the name-matched
/// form skipped any directory called `tmp` at any depth, so a future
/// `crates/dss-core/src/tmp/` would have left the whole-repository walk
/// silently. `.gitignore` anchors the same way (`/tmp`), so the skip now covers
/// exactly the path that is ignored — no premise about the rest of the tree, and
/// nothing for a later reader to re-verify by hand.
const SKIP_DIRS_AT_ROOT: &[&str] = &["tmp"];

/// Every `.rs` file in the repository.
///
/// Deliberately the **whole tree**, not just `crates/*/{src,tests,benches,
/// examples}`. The compat tag is an index of the ported Rust that deliberately
/// reproduces an upstream inexactness, and `CLAUDE.md` says all of them are
/// absorbed in one pass — not "all of them under `crates/`". F.3ac found three
/// markers living outside that prefix: `tools/wasm_usermodel/models/
/// indmach012a` is a real hand-ported Rust crate (the WM.2 reference user
/// model) that is **workspace-excluded by design** — it carries its own empty
/// `[workspace]` table so the fixture toolchain stays pinned separately from
/// the product gate (`WASM_USERMODELS_PLAN.md` §2.6). A `crates/`-shaped walk
/// can never see it, so its markers named Stage F as their owner while being
/// invisible to every Stage F gate, including the exit count.
fn rust_sources(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut dirs = vec![root.to_path_buf()];

    while let Some(dir) = dirs.pop() {
        for entry in fs::read_dir(&dir).expect("directory is readable").flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = entry.file_name().to_string_lossy().to_lowercase();
                let at_root = dir == root;
                let skipped = SKIP_DIRS.contains(&name.as_str())
                    || (at_root && SKIP_DIRS_AT_ROOT.contains(&name.as_str()));
                if !skipped {
                    dirs.push(path);
                }
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
    out.sort();
    assert!(
        out.len() > 100,
        "source scan found only {} files — the walk is broken",
        out.len()
    );
    // Non-vacuity of the *widening*: both halves of the tree must be reached,
    // so a future "tidy the walk" cannot silently shrink it back to `crates/`
    // and drop the fixture crates' markers out of the register again.
    for expected in [
        "crates/dss-core/src/compat.rs",
        "tools/wasm_usermodel/models/indmach012a/src/model.rs",
    ] {
        let want = root.join(expected);
        assert!(
            out.contains(&want),
            "the source walk no longer reaches {expected} — every compat marker \
             in that subtree just left the Stage F register unnoticed"
        );
    }
    out
}

/// A file may carry the cfg string when it belongs to one of the **three
/// registered** compat modules ([`COMPAT_MODULES`] or that module's own
/// `compat/` submodule directory) or when it is test code (`crates/*/tests/**`,
/// or a `tests.rs` unit-test module).
///
/// The registration is deliberate. An earlier version whitelisted *any* path
/// with a component named `compat`, which meant a fourth `…/src/<anything>/
/// compat.rs` could carry lane cfgs with nothing objecting — against IV.2's
/// "one `compat` module per affected crate", and invisible to the alias-pin
/// gate too, since that walks [`COMPAT_MODULES`] and would never see the new
/// module's rows. Adding a crate to the split now means adding it here, which
/// is the point.
fn is_sanctioned(path: &Path, root: &Path) -> bool {
    let rel = path.strip_prefix(root).unwrap_or(path);
    let parts: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
        .collect();
    let file = parts.last().expect("a file name");

    // `crates/<crate>/src/compat.rs` and its sibling `crates/<crate>/src/
    // compat/**` directory, for exactly the registered crates.
    let in_compat = COMPAT_MODULES.iter().any(|m| {
        let m_parts: Vec<String> = Path::new(m)
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
            .collect();
        let stem = &m_parts[..m_parts.len() - 1]; // crates/<crate>/src
        if !parts.starts_with(stem) {
            return false;
        }
        let tail = &parts[stem.len()..];
        matches!(
            tail.first().map(String::as_str),
            Some("compat.rs") | Some("compat")
        )
    });
    let in_tests = parts.iter().any(|p| p == "tests") || file == "tests.rs";
    in_compat || in_tests
}

#[test]
fn oracle_parity_cfg_appears_only_in_compat_modules_and_tests() {
    let root = repo_root();
    let needle = needle();

    let mut offenders = Vec::new();
    let mut sanctioned_hits = Vec::new();
    for path in rust_sources(&root) {
        let text = match fs::read_to_string(&path) {
            Ok(t) => t,
            Err(_) => continue, // non-UTF-8 fixtures are not Rust sources
        };
        if !text.contains(&needle) {
            continue;
        }
        let rel = path.strip_prefix(&root).unwrap_or(&path).to_path_buf();
        if is_sanctioned(&path, &root) {
            sanctioned_hits.push(rel);
        } else {
            let lines: Vec<String> = text
                .lines()
                .enumerate()
                .filter(|(_, l)| l.contains(&needle))
                .map(|(i, l)| format!("    {}:{}: {}", rel.display(), i + 1, l.trim()))
                .collect();
            offenders.push(lines.join("\n"));
        }
    }

    assert!(
        offenders.is_empty(),
        "`{needle}` outside the compat modules (DE_PASCALIZE IV.2 mechanism / \
         forbidden move 4) — move the branch into the crate's `compat` module \
         and call an unconditional `compat::` alias:\n{}",
        offenders.join("\n")
    );

    // Non-vacuity: the three compat modules that carry the split must each be
    // present and actually cfg-selecting, or this gate would pass on a tree
    // where the seam was accidentally deleted.
    for expected in COMPAT_MODULES {
        let path = root.join(expected);
        let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{expected}: {e}"));
        assert!(
            text.contains(&needle),
            "{expected} carries no `{needle}` — the Stage F seam is gone"
        );
    }
    assert_eq!(
        sanctioned_hits.len(),
        sanctioned_hits.len().max(COMPAT_MODULES.len()),
        "fewer cfg-carrying files than there are registered compat modules"
    );
}

/// The **second** lane-branch channel, policed the same way as the cfg string.
///
/// `compat::ORACLE_PARITY` is a plain `bool` const, so product code could write
/// `if compat::ORACLE_PARITY { .. } else { .. }` and get exactly the lane
/// spaghetti forbidden move 4 bans — while carrying none of the cfg text the
/// gate above searches for. Reproduced (F-settle W4): a one-line
/// `pub fn probe(x: f64) -> f64 { if compat::ORACLE_PARITY { x * 2.0 } else
/// { x * 3.0 } }` dropped into `dss-core/src` passed every test in this file.
///
/// The rule is the same as for the cfg: the branch belongs in a `compat`
/// module behind an unconditional alias. Reads inside test code are the whole
/// point of the constant and stay allowed, and so is `crates/*/examples/**` —
/// build-time instruments that never link into the shipped library. There is
/// exactly one such reader, and it is the reason the allowance exists:
/// `lane_dump` stamps the lane it was built in into its dump header, which is
/// how the differential job knows it compared two *different* lanes.
#[test]
fn the_lane_constant_is_read_only_by_compat_modules_and_tests() {
    let root = repo_root();

    let mut offenders = Vec::new();
    let mut sanctioned = 0usize;
    for path in rust_sources(&root) {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        if !names_token(&text, "ORACLE_PARITY") {
            continue;
        }
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let in_examples = path
            .strip_prefix(&root)
            .unwrap_or(&path)
            .components()
            .any(|c| c.as_os_str().eq_ignore_ascii_case("examples"));
        if is_sanctioned(&path, &root) || in_examples {
            sanctioned += 1;
            continue;
        }
        // A plain source file may read it only from inside its test region.
        let head = match test_region(&path, &root, &text) {
            Some((start, _)) => &text[..start],
            None => text.as_str(),
        };
        if names_token(head, "ORACLE_PARITY") {
            let lines: Vec<String> = head
                .lines()
                .enumerate()
                .filter(|(_, l)| {
                    !l.trim_start().starts_with("//") && names_token(l, "ORACLE_PARITY")
                })
                .map(|(i, l)| format!("    {rel}:{}: {}", i + 1, l.trim()))
                .collect();
            if !lines.is_empty() {
                offenders.push(lines.join("\n"));
            }
        } else {
            sanctioned += 1;
        }
    }

    assert!(
        offenders.is_empty(),
        "`compat::ORACLE_PARITY` read outside the compat modules and test code \
         — that is a lane branch in product code (DE_PASCALIZE IV.2 forbidden \
         move 4). Move it into the crate's `compat` module and call an \
         unconditional `compat::` alias:\n{}",
        offenders.join("\n")
    );
    // Non-vacuity: the constant is read by the compat modules and by the pins.
    assert!(
        sanctioned >= COMPAT_MODULES.len(),
        "the walk stopped reaching the lane constant's legitimate readers"
    );
}

/// The compat tag, assembled at runtime so this gate file carries no literal
/// occurrence of its own needle.
fn compat_tag() -> String {
    format!("TODO{}compat{}", "(", ")")
}

/// The compat tag is an index, not prose: every occurrence must open a marker.
///
/// Concretely — the tag must sit inside a comment and be followed immediately
/// by `": "`, i.e. `<tag>: <why>`. That is what makes a grep for it count
/// *reproduction sites* and nothing else, which is the form `CLAUDE.md`
/// mandates and the number `DE_PASCALIZE_PLAN.md` Part IV.2 drives to zero. To
/// point at a site from elsewhere, write "compat-tagged at …" / "see the compat
/// marker at …" instead of repeating the tag.
#[test]
fn compat_tag_is_only_ever_a_marker_never_prose() {
    let root = repo_root();
    let tag = compat_tag();

    let mut offenders = Vec::new();
    let mut markers = 0usize;
    for path in rust_sources(&root) {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        if !text.contains(&tag) {
            continue;
        }
        let rel = path.strip_prefix(&root).unwrap_or(&path).to_path_buf();
        for (i, line) in text.lines().enumerate() {
            for (col, _) in line.match_indices(&tag) {
                let after = &line[col + tag.len()..];
                let before = &line[..col];
                let in_comment = before.contains("//");
                if in_comment && after.starts_with(": ") {
                    markers += 1;
                } else {
                    offenders.push(format!("    {}:{}: {}", rel.display(), i + 1, line.trim()));
                }
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "the compat tag must always open a marker (`{tag}: <why>`) inside a \
         comment — a prose mention breaks the greppable index CLAUDE.md \
         requires and inflates the Stage F exit count. Reword it as \
         \"compat-tagged at …\":\n{}",
        offenders.join("\n")
    );

    // Non-vacuity: the walk must actually be seeing the markers. Stage F drives
    // this population to zero; when it gets there, delete this floor with the
    // last marker (the gate above still enforces the spelling on any new one).
    assert!(
        markers > 0,
        "no compat markers found at all — the source walk is broken (or Stage F \
         finished, in which case drop this assertion)"
    );
}

// ---------------------------------------------------------------------------
// The Stage F.3 escape register, executable
// ---------------------------------------------------------------------------

/// Who owns a compat marker that survived Stage F.3, i.e. what has to happen
/// before it can be resolved.
///
/// F.3 flipped every marker whose clean fix fits the IV.2 drift model — a
/// field-scoped default-lane exclusion plus an expected-value pin — and
/// *measured* the cost of every one that does not. These variants are the
/// reasons a measurement came back "no": the fix is another step's job, the fix
/// needs an oracle re-baseline, the fix costs a whole gated case its
/// default-lane oracle comparison, or the marker sits in a build the lane
/// mechanism cannot reach at all.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Escape {
    /// **An UPGRADE rung.** A truncated physical constant whose corrected value
    /// moves gated *oracle* artifacts — live corpus cases and/or byte goldens —
    /// past their calibrated floors. Both gating oracles carry the literal, so
    /// this is a re-baseline against a different engine, not a lane flip. The
    /// per-row bill is at the site.
    UpgradeRung,
    /// **Unsanctioned exclusion.** A single-site upstream quirk whose clean fix
    /// moves the node voltages (or the whole dynamics surface), so the default
    /// lane would have to drop a gated case *entirely* rather than a field. The
    /// drift-model row sanctions excluding a deliberate divergence "at those
    /// fields"; a whole-case skip is a coverage trade the plan does not grant
    /// the executor.
    WholeCase,
    /// **`WASM_USERMODELS_PLAN`.** The marker lives in a wasm reference user
    /// model (`tools/wasm_usermodel/models/…`), where the Stage F mechanism
    /// does not exist: the model is a **workspace-excluded** crate with its own
    /// `[workspace]` table (plan §2.6), so `dss-core/oracle-parity` cannot
    /// reach it, and the gated artifact is ONE committed binary
    /// (`tests/fixtures/wasm/*.wasm`, regenerated manually with the toolchain
    /// pinned in `tools/wasm_usermodel/PIN.txt`). A lane split here is not a
    /// `cfg` alias but a second `.wasm` fixture, and the guest's numbers are
    /// pinned bit-exactly against the **native FPC twin**
    /// (`tests/twin_expected.rs`, "GENERATED … DO NOT EDIT", probed into
    /// `docs/wasm/probes/p6_twin_expected.txt`) as well as by the
    /// `wasm_usermodels` oracle goldens the parity lane may never re-baseline.
    /// Each row's flip was run against that twin; the number is at the site.
    WasmGuest,
}

/// Every compat marker still standing at the Stage F.3 exit, keyed by the file
/// it lives in and a distinctive slice of its own text.
///
/// This is `STATUS.md`'s escape register with the prose removed. It is checked
/// **both ways** — an unregistered marker fails, and a registered marker that no
/// longer exists fails — the same fail-on-stale discipline
/// `tests/corpus/ledger.json` uses, and for the same reason: a list of known
/// divergences that nothing re-reads decays into folklore. Concretely it makes
/// two plan rules executable that were previously habits:
///
/// * "the dual-kernel inventory is **closed** — do not invent new compat items"
///   (`DE_PASCALIZE_PLAN.md` IV.2): a new marker cannot appear without an edit
///   here, which is where its owner has to be named.
/// * Stage F's exit criterion is a *count* of this tag: closing a row is now a
///   deliberate act (delete the row, drop the bucket total) rather than a
///   silent decrement.
const ESCAPE_REGISTER: &[(&str, &str, Escape)] = &[
    // ---- truncated physical constants → an UPGRADE-rung re-baseline (11) ----
    // `mu0` (4.889e-8 low, 35/520 corpus cases) and `Twopi` (2.858e-11 low,
    // 1/520 at 1.031x its floor). The physically meaningful group is `mu0/twopi`,
    // so a correct fix flips both and pays the 35-case bill.
    (
        "crates/dss-core/src/support/line_constants/mod.rs",
        "these reproduce the upstream truncated literals exactly",
        Escape::UpgradeRung,
    ),
    // `0.3183` as `1/pi`, 3.10e-5 low, multiplies the tape-shield resistance:
    // 8/520 corpus cases (every tape-shield deck) + 8 unit tests.
    (
        "crates/dss-core/src/support/line_constants/cable.rs",
        "upstream truncated `1/pi` literal in the tape-shield resistance",
        Escape::UpgradeRung,
    ),
    // `CALPHA`'s imaginary part is 4.37e-7 short of −sin 120°: 33/520 corpus
    // cases + the `dump_reactor_symcomp` byte golden. Two sites, one constant.
    (
        "crates/dss-core/src/util.rs",
        "Pascal `CALPHA = (-0.5, -0.866025)`",
        Escape::UpgradeRung,
    ),
    (
        "crates/dss-core/src/elements/pd/reactor/solve.rs",
        "Pascal `CALPHA` is the truncated literal",
        Escape::UpgradeRung,
    ),
    // The truncated pi / rad-to-deg pair: upstream names the fix itself, and the
    // distances are tiny — but `cdang` and `pdeg_to_complex` are inverses the
    // engine round-trips through, so the truncations cancel exactly. Flipping
    // both (the only coherent form) is strictly *more* accurate and costs 16/520
    // corpus cases, 5 `wasm_*` r4133 goldens and 7 byte goldens.
    (
        "crates/dss-core/src/support/complexutil/mod.rs",
        "truncated constants reproduced from DSSUcomplex.pas",
        Escape::UpgradeRung,
    ),
    (
        "crates/dss-core/src/support/complexutil/mod.rs",
        "replace with `f64::atan2`",
        Escape::UpgradeRung,
    ),
    // Carson's earth-return depth. The pinned 0.14.5 oracle is *self-consistent*
    // at 658.5 — `LineConstants.pas:424` and `Line.pas:520/704/959` both — so the
    // port's "one engine, two values" state is not an upstream inconsistency we
    // inherited: it is WP-U1.2 B2/D1 adopting r4133's corrected
    // 658.8530451057239 for `LineConstants` alone. Finishing that job in `Line`
    // is therefore the *next step of that rung*, not a lane flip, which is what
    // the measurement says too: the flip fails `harmonics_doall` on
    // `Line.l1 Yprim[0,0]` at 1.732e-6 against an allowed 1.002e-6 — an oracle
    // golden gated in *both* lanes.
    (
        "crates/dss-core/src/elements/pd/line/mod.rs",
        "658.5 (not 658.8530451057239)",
        Escape::UpgradeRung,
    ),
    (
        "crates/dss-core/src/elements/pd/line/code.rs",
        "658.5 (not 658.8530451057239)",
        Escape::UpgradeRung,
    ),
    (
        "crates/dss-core/src/elements/pd/line/accessors.rs",
        "658.5, NOT the 658.8530451057239",
        Escape::UpgradeRung,
    ),
    // `2.3026` as ln(10). Not a slip at all: r4133 states it in the user-facing
    // property help ("tau = Tresponse / 2.3026", `ExpControl.pas:184`), so it is
    // a documented model constant and replacing it is a specified-behaviour
    // change (6.47e-6 on `FOpenTau`, plus the byte-exact `cim_der{,_DYN}.xml`).
    (
        "crates/dss-core/src/elements/control/exp_control/accessors.rs",
        "Pascal `FOpenTau := Tresponse / 2.3026`",
        Escape::UpgradeRung,
    ),
    (
        "crates/dss-core/src/cim/ieee1547.rs",
        "Pascal's `LPFTau * 2.3026`",
        Escape::UpgradeRung,
    ),
    // ---- the rendering seam → F.4 (`F-FMT`): all 7 resolved, none left ----
    // Six of them are still lane rows (they die at WP-G4); the seventh is gone
    // altogether — `GOLDEN_REBASE_PLAN.md` G2.6 tore down the `Show`
    // device-name column width, which was never a rendering convention but a
    // dss_capi-only defect (a shadowing `TDSSCircuit` field, absent in r4133).
    // See its `TORN_DOWN_ROWS` entry.
    //
    // F.4a took the six *number-rendering* rows through the `compat` seam:
    // `util.rs`'s `%g` two-stage rounding (`compat::fmt_g`),
    // `report/format.rs`'s script fixed-point (`compat::fixed_w_script`),
    // `show/diagnostics.rs`'s unobservable control-queue precision
    // (`compat::CONTROL_QUEUE_SEC_DIGITS`), `export/json/mod.rs`'s fpjson float
    // literal (`compat::json_float`) and platform line break
    // (`compat::JSON_LINE_BREAK`), and `export/json/circuit.rs`'s PostCommands
    // umbrella — whose two spellings are the `%g` and fixed rows above and whose
    // command set is IV.1 contract, not compat. F.4b took the seventh, the
    // `Show` device-name column width — the one G2.6 has since removed from the
    // seam entirely (the paragraph above).
    // ---- whole-case default-lane exclusions → not the executor's to grant (1) ----
    // The GICTransformer `%R2`, Capacitor `Cuf` and LoadShape MMF accept-set
    // rows that used to sit here are gone: `GOLDEN_REBASE_PLAN.md` G2.5 fixed
    // all three engines in both lanes and paid each one's price where this
    // bucket said it had to be paid — a `tests/corpus/ledger.json` entry per
    // (case, channel) plus an expected-value pin. See [`TORN_DOWN_ROWS`].
    //
    // Seeding `E1` from a fresh `Vterminal` moves the whole dynamics run of the
    // only deck exercising a Model=6 user model: `wasm_gen_dyn`'s `dSpeed` by
    // 3.449e-2 relative, on an r4133 golden the parity lane may never re-baseline.
    (
        "crates/dss-core/src/elements/pc/generator/user_model.rs",
        "this deliberately reproduces an upstream inconsistency",
        Escape::WholeCase,
    ),
    // ---- the wasm reference user model → WASM_USERMODELS_PLAN (3) ----
    // Each flip was run in isolation against the crate's own native-FPC-twin
    // pins (`cargo test` in the excluded crate, 2026-07-29); every one of them
    // fails a *bit-exact* generated pin, which is the whole contract of that
    // fixture. Same truncated-constant family as the engine-side rows next
    // door, but with no lane to flip into.
    //
    // Truncated `sqrt(3)/2` in the symmetrical-component `A` matrix →
    // `pf_i_1_0` -1436.051530295505 vs the twin's -1436.0515303730524
    // (5.40e-11 relative).
    (
        "tools/wasm_usermodel/models/indmach012a/src/symcomp.rs",
        "0.866025403",
        Escape::WasmGuest,
    ),
    // Truncated `sqrt(3)` in `Compute_dSdP`'s rated voltage → `vars_initial_10`
    // (`Ir1`) 1723.6616943288936 vs the twin's 1723.7122572967085 (2.93e-5
    // relative) — the largest of the three by four orders of magnitude.
    (
        "tools/wasm_usermodel/models/indmach012a/src/model.rs",
        "`1.732` reproduces upstream",
        Escape::WasmGuest,
    ),
    // FPC's SINGLE-precision constant folding of `3.0/746.0` → `vars_initial_14`
    // (`HPshaft`) -1848.1305807400754 vs the twin's -1848.1305331200504
    // (2.58e-8 relative, confirming the 2.6e-8 the marker already claimed).
    (
        "tools/wasm_usermodel/models/indmach012a/src/model.rs",
        "FPC folds the all-constant",
        Escape::WasmGuest,
    ),
];

/// The Stage F.3 exit population, per handoff owner.
///
/// Update these numbers in the same commit that closes a row — that is the
/// point of stating them: the count is the plan's exit criterion, so it should
/// move only on purpose.
const EXIT_POPULATION: [(Escape, usize); 3] = [
    (Escape::UpgradeRung, 11),
    // 4 → 1 at `GOLDEN_REBASE_PLAN.md` G2.5; the survivor is the Generator
    // Model=6 user-model row, deferred with WASM_USERMODELS_PLAN.
    (Escape::WholeCase, 1),
    (Escape::WasmGuest, 3),
];

/// Collect `(repo-relative path with `/` separators, marker text)` for every
/// compat marker in the crate sources.
fn markers_in_tree(root: &Path) -> Vec<(String, String)> {
    let tag = format!("{}: ", compat_tag());
    let mut out = Vec::new();
    for path in rust_sources(root) {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        if !text.contains(&tag) {
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        for line in text.lines() {
            if let Some(col) = line.find(&tag) {
                out.push((rel.clone(), line[col + tag.len()..].trim().to_string()));
            }
        }
    }
    out
}

/// The surviving markers are exactly the recorded escape register — no more, no
/// fewer, each owned by a named successor.
#[test]
fn surviving_compat_markers_are_exactly_the_recorded_escape_register() {
    let root = repo_root();
    let found = markers_in_tree(&root);

    assert!(
        !found.is_empty(),
        "no compat markers found at all — the source walk is broken"
    );

    // Every marker in the tree is registered, and unambiguously so.
    let mut hits = vec![0usize; ESCAPE_REGISTER.len()];
    let mut unregistered = Vec::new();
    for (rel, text) in &found {
        let matched: Vec<usize> = ESCAPE_REGISTER
            .iter()
            .enumerate()
            .filter(|(_, (path, key, _))| path == rel && text.contains(key))
            .map(|(i, _)| i)
            .collect();
        match matched.len() {
            1 => hits[matched[0]] += 1,
            0 => unregistered.push(format!("    {rel}: {text}")),
            n => panic!("{rel}: {text}\n  matches {n} register rows — sharpen their keys"),
        }
    }
    assert!(
        unregistered.is_empty(),
        "compat marker(s) with no row in the Stage F escape register. A new \
         marker means a new compat item, and DE_PASCALIZE IV.2's inventory is \
         closed: either resolve it in this stage, or add a row naming which \
         successor owns it and what the flip was measured to cost:\n{}",
        unregistered.join("\n")
    );

    // ...and every registered marker still exists (fail-on-stale).
    let stale: Vec<String> = ESCAPE_REGISTER
        .iter()
        .enumerate()
        .filter(|(i, _)| hits[*i] != 1)
        .map(|(i, (path, key, owner))| {
            format!("    {path}: {key:?} ({owner:?}) — {} matches", hits[i])
        })
        .collect();
    assert!(
        stale.is_empty(),
        "escape-register row(s) that no longer match exactly one marker. If the \
         site was resolved, delete its row and decrement its bucket in \
         `EXIT_POPULATION`; if it was only reworded, re-key the row:\n{}",
        stale.join("\n")
    );

    // The exit count, per owner.
    for (owner, expected) in EXIT_POPULATION {
        let actual = ESCAPE_REGISTER
            .iter()
            .filter(|(_, _, o)| *o == owner)
            .count();
        assert_eq!(
            actual, expected,
            "{owner:?} carries {actual} markers, the recorded Stage F.3 exit \
             population is {expected}"
        );
    }
    let total: usize = EXIT_POPULATION.iter().map(|(_, n)| n).sum();
    assert_eq!(found.len(), total, "total marker population");
}

// ---------------------------------------------------------------------------
// The one non-marker escape: its artifact population, by surface
// ---------------------------------------------------------------------------

/// The `Dump` half of the 0.15.x hide-flag escape's blast radius, relative to
/// `tests/golden/reports/`.
///
/// `+4` rows per Line-bearing dump and `+1` per LineGeometry one — the carrier
/// count per class, which is what makes the radius closed.
const HIDE_FLAG_DUMP_GOLDENS: [&str; 8] = [
    "dump3_bare.txt",
    "dump3_commands.txt",
    "dump3_debug.txt",
    "dump_line_geo.txt",
    "dump_line_lc.txt",
    "dump_line_sym.txt",
    "dump_line_switch.txt",
    "dump_linegeometry.txt",
];

/// The JSON half, relative to `tests/golden/json/`: two AltDSS captures and the
/// three schema walks.
const HIDE_FLAG_JSON_GOLDENS: [&str; 5] = [
    "circuit_micro.json",
    "line_micro.json",
    "schema_divergences.json",
    "schema_full_oracle.json",
    "schema_full_port.json",
];

/// Pins the population the **0.15.x hide-flag** escape was measured over — the
/// half the carrier-set pin
/// (`dss_core::exec::tests::compat_quirks::hide_015x_carrier_set_is_the_measured_escape`,
/// which pins the *five carriers*) does not cover: the 13 gated artifacts the
/// flip moves.
///
/// # Why the surface, and not just the names
///
/// F.3aa handed this row forward to **F.4** because "F-FMT re-layouts the same
/// Dump/Show surface", i.e. F.4 would be regenerating these goldens anyway.
/// F.3ag found that false at the step it cites: `DE_PASCALIZE_PLAN.md` §F-FMT
/// re-layouts **`Show`-style reports only** (step 2), step 3 keeps row/column
/// structure so the default lane compares the *same* committed goldens through
/// the parsed-numeric tokenizer, and free re-layout — the thing that does force
/// self-goldens — is the optional **v2**. The disproof is therefore a claim
/// about *which surfaces these 13 sit on*, so that is what this test pins:
/// eight `Dump` texts and five JSON documents, and **not one `Show` table**. A
/// later WP that renames one of them, drops one, or re-points the measurement at
/// a `Show` report fails here instead of quietly invalidating the record.
#[test]
fn the_hide_flag_escape_population_is_pinned_by_surface() {
    let root = repo_root();
    let mut missing = Vec::new();

    for (dir, names) in [
        ("tests/golden/reports", &HIDE_FLAG_DUMP_GOLDENS[..]),
        ("tests/golden/json", &HIDE_FLAG_JSON_GOLDENS[..]),
    ] {
        for name in names {
            let path = root.join(dir).join(name);
            if !path.is_file() {
                missing.push(format!("    {dir}/{name}"));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "gated artifact(s) the 0.15.x hide-flag escape was measured over no \
         longer exist. The recorded blast radius (13 artifacts, no corpus \
         movement) stops describing the tree: re-measure the flip and update \
         the flag's doc in `obj/props/prop_flags.rs` before editing this \
         list:\n{}",
        missing.join("\n")
    );

    // The surface classification *is* the disproof of the F.4 hand-off, so it
    // is asserted rather than narrated: every text artifact is a `Dump`, and a
    // `Show` report may never enter this population without re-opening that
    // question.
    for name in HIDE_FLAG_DUMP_GOLDENS {
        assert!(
            name.starts_with("dump"),
            "{name} is not a `Dump` golden — F-FMT's re-layout step covers \
             `Show`-style reports, so a non-`Dump` text artifact here reopens \
             whether F.4 hosts this row"
        );
    }

    assert_eq!(
        HIDE_FLAG_DUMP_GOLDENS.len() + HIDE_FLAG_JSON_GOLDENS.len(),
        13,
        "the escape record says 13 artifacts move; this list must say the same"
    );
}

// ---------------------------------------------------------------------------
// The flipped half of the register: every deliberate divergence is pinned
// ---------------------------------------------------------------------------

/// The per-crate `compat` modules that carry the lane split.
const COMPAT_MODULES: [&str; 3] = [
    "crates/dss-core/src/compat.rs",
    "crates/dss-parser/src/compat.rs",
    "crates/dss-sparse/src/compat.rs",
];

/// Aliases whose two cfg arms select the **same** impl — declared, not wired.
///
/// `dss-sparse`'s two solver-execution knobs are the whole set (IV.2 row
/// "solver execution"): `MULTITHREADING_PLAN` M3c and `RESONANCE_PLAN` WP-R1 own
/// the flip, and until then no call site reads them, so there is no observable
/// to pin. Listed by name rather than skipped by count, because the moment their
/// owner makes the arms differ they become split rows and the pin rule below
/// starts applying to them — which is exactly when someone must write the
/// expected-value test.
const DECLARED_NOT_WIRED: [&str; 2] = ["ITERATIVE_REFINEMENT", "PARALLEL_FACTORIZATION"];

/// How many `compat::` aliases genuinely resolve to different code in the two
/// lanes today. Every one of them is a **deliberate divergence** from the
/// gating oracles, so every one owes an expected-value pin.
///
/// Move this number only in the commit that flips (or un-flips) a row.
///
/// 37 at the close of F.5; 38 after the W4 settlement completed the round row
/// with its **array** kernel (`round_f64` — Pascal's `ApplyRound`, which writes
/// `Round`'s Int64 back into a Double); **31** after the R4133-alignment pass of
/// 2026-08-02 dismantled seven bug kernels whose defect r4133 does not share
/// (LineSpacing `MakeLike`, LineCode `C0`, the `DoubleSymMatrix` text getter,
/// `Save <class>`'s reported path, Storage `MakePosSequence` bracketing, the
/// `CktModel` ordinal and the Generator rating guards) — both lanes now take the
/// authority's behaviour and each row keeps an unconditional expected-value pin.
/// **30** after `GOLDEN_REBASE_PLAN.md` G2.1a tore down `stddev_single_point`,
/// the first of WP-G2's rows (a defect r4133 *does* share — see
/// [`TORN_DOWN_ROWS`] for what each teardown leaves behind); **29** after G2.1b
/// did the same to `CAPCONTROL_MAKELIKE_DROPS_CONTROL_SIGNAL`; **28** after
/// G2.1c did the same to `SEQ_CURRENTS_PRINTS_RAW_NONPOSITIVE_RATING`; **27**
/// after G2.1d did the same to `REDUCE_SCANS_ONLY_THE_FIRST_PARENT_SHUNT`;
/// **26** after G2.1e did the same to
/// `STORAGE_CONTROLLER_IDLE_TEST_COMPLEMENTS_THE_ORDINAL`; **25** after G2.1f
/// did the same to `STORAGE_MULTIFILE_USES_THE_PV_PREFIX`; **24** after G2.1g
/// did the same to `CIM_WYE_GROUNDED_IS_HARDCODED_TRUE`; **23** after G2.1h did
/// the same to `HEIGHT_UNIT_CHANGE_REREADS_THE_METRES_FIELD`, the last of the
/// G2.1 zero-footprint rows; **21** after G2.2a tore down the first two rows
/// whose teardown a golden compare observes — `IRESIDUAL_FROM_TERMINAL_1` and
/// `BUS_INT_DURATION_WALKS_ALL_BUSES`, whose harness exclusions became
/// unconditional instead of moving a golden byte; **19** after G2.2b did the
/// same for the two *property* exclusions, `monitor_base_frequency` and
/// `ISOURCE_BUS2_NEVER_LATCHES`; **16** after G2.2c did the same for the three
/// *text-transform* rows — `FAULT_DUMP_TAIL_REPRINTS_MINAMPS` and the two CIM
/// attribute names, `CIM_DELTA_SHUNT_GROUNDED_USES_LINEAR_PREFIX` and
/// `CIM_ACLINESEGMENT_G0CH_WRITTEN_AS_B0CH`, whose oracle-text rewrites became
/// unconditional instead of moving a golden byte; **14** after G2.2d did the
/// same for the two *event-log* rows, `RELAY_SAMPLE_TRACE_IGNORES_DEBUGTRACE`
/// and `RELAY_RESET_EVENT_IS_LABELLED_RECLOSER`, whose two rewrites moved above
/// `expected_eventlog`'s lane guard while the `compat::fmt_g` re-round fold in
/// the same function — a precision row, alive until G4.1 — stayed behind it.
/// **13** after G2.3 tore down `POWERS_REUSE_STALE_NEWTON_ITERMINAL`, the last
/// of the six CLAUDE.md upstream bugs still reproduced anywhere: both lanes
/// recompute `Iterminal` at the converged `NodeV`, and the two `modes/newton/`
/// decks' powers/losses — which no oracle channel reports that way — became an
/// unconditional harness exclusion instead of a lane split. **12** after G2.4
/// *reclassified* `MONITOR_CHANNEL_PADS_THE_UNFLUSHED_STREAM`: the `[0.0]` an
/// unflushed monitor stream reads back as is fabricated by the client-side
/// ByteStream decoders of **both** gating channels, not by any engine, so both
/// lanes report the empty channel and the harness normalizes the placeholder out
/// of every capture. (The engines' own answer in that state — `SampleCount`
/// zeros conjured from unwritten stream bytes — is a separate upstream defect
/// the port declines; no client reaches it. See the row's `TORN_DOWN_ROWS`
/// entry.) **11** after G2.6 tore down `max_device_name_length`, the last WP-G2
/// row and the only one of the seven F-FMT *rendering* rows that was not a
/// rendering convention at all: dss_capi's `SetMaxDeviceNameLength` fills a
/// shadowing `TDSSCircuit` field while its writers read the unit variable it
/// zeroed, so the `Show` device-name column collapses to 0 there and does not in
/// r4133, which has no such field. Both lanes now size the column from its own
/// content and the three `show_busflow*` goldens' oracle text is de-glued
/// unconditionally. The eleven survivors are WP-G2's fixed point: the five
/// numeric precision rows (`PI`, `round_f64`, `round_i32`,
/// `kv_base_search_scale`, `profile_ll_pu_divisor`) and the six *rendering* ones
/// (`fmt_g`, `fixed_w_script`, `CONTROL_QUEUE_SEC_DIGITS`, `json_float`,
/// `JSON_LINE_BREAK`, `render_rows`), which WP-G4 takes down to five.
const SPLIT_ALIAS_POPULATION: usize = 11;

/// The slice of `text` that is **test code**, or `None` if the file has none.
///
/// Three shapes, all present in the tree: an integration test under
/// `crates/*/tests/`, an extracted sibling `tests.rs`, or an inline
/// `#[cfg(test)] mod tests` in a plain source file — for which only the part
/// *after* the attribute counts. That last distinction is the point: matching
/// the whole file let a production call site satisfy the pin gate on its own,
/// so gutting `dispatch.rs`'s test module while leaving `#[cfg(test)] mod tests
/// {}` behind kept the gate green with `kv_base_search_scale` unpinned
/// (reproduced, F-settle W4). A region must also contain a `#[test]`, or there
/// is nothing in it that can assert anything.
fn test_region(path: &Path, root: &Path, text: &str) -> Option<(usize, usize)> {
    let rel = path.strip_prefix(root).unwrap_or(path);
    let parts: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
        .collect();
    let file = parts.last().expect("a file name");

    let start = if parts.iter().any(|p| p == "tests") || file == "tests.rs" {
        0
    } else {
        text.find("#[cfg(test)]")?
    };
    let region = &text[start..];
    if !region.contains("#[test]") {
        return None;
    }
    Some((start, text.len()))
}

/// Files that may never count as a pin: the compat modules themselves (their
/// own `tests` submodules assert the *kernels* against each other, which is a
/// different obligation — the pin has to be at an observable), and this gate,
/// which names rows for bookkeeping.
fn is_pin_candidate(path: &Path, root: &Path, text: &str) -> Option<(usize, usize)> {
    let rel = path.strip_prefix(root).unwrap_or(path);
    let parts: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
        .collect();
    let file = parts.last().expect("a file name");

    if file == "compat.rs" || parts.iter().any(|p| p == "compat") {
        return None;
    }
    if file == "oracle_parity_cfg_gate.rs" {
        return None;
    }
    test_region(path, root, text)
}

/// `token` occurs in `text` as a whole identifier, not inside a longer one.
fn names_token(text: &str, token: &str) -> bool {
    let ident = |c: char| c.is_alphanumeric() || c == '_';
    text.match_indices(token).any(|(i, _)| {
        let before = text[..i].chars().next_back();
        let after = text[i + token.len()..].chars().next();
        !before.is_some_and(ident) && !after.is_some_and(ident)
    })
}

/// A pin has to *branch on the lane*, or it pins one lane's value in both and
/// the other lane's behavior is unasserted.
///
/// The accepted forms are a read of the lane constant `ORACLE_PARITY` or of the
/// harness alias `lane::PARITY` — legitimate because `harness/lane.rs:78`
/// defines `PARITY` as `cfg!(feature = "oracle-parity")` and asserts it equals
/// `dss_core::compat::ORACLE_PARITY` (`lane.rs:795-796`), so reading it *is*
/// reading the lane. Deriving the expectation from the row's **own** alias was
/// accepted until F-settle W4 and is now rejected: engine and test then read
/// the same constant, so the pin asserts "the engine agrees with the
/// declaration" — true by construction whichever way the alias points. Flipping
/// five such rows' `*_DEFAULT_IMPL` back to the parity value (a silently
/// reverted fix) passed the entire workspace suite, in both lanes. No oracle
/// gate can catch that class either: reverting the fix restores exactly what
/// the oracles return.
///
/// The `lane::PARITY` spelling was added in G2.1f, which exposed the gap by
/// deleting the last `ORACLE_PARITY` token in `tests/golden_reports.rs`: the
/// walk matches per **file**, so three still-split rows pinned there through
/// `lane::PARITY` alone (`IRESIDUAL_FROM_TERMINAL_1`,
/// `BUS_INT_DURATION_WALKS_ALL_BUSES`, `FAULT_DUMP_TAIL_REPRINTS_MINAMPS`) had
/// been credited by a *neighbouring* test's constant. Their pins do branch on
/// the lane; only this predicate could not see how. (All three are torn down
/// now — the first two by G2.2a, the third by G2.2c — and their pins are
/// unconditional. The arm was still load-bearing while
/// `max_device_name_length` lived, because `golden_reports.rs` pinned it and
/// reads the lane in this spelling only. G2.6 tore that row down too, and the
/// measurement now is: **no** surviving row's pin depends on this arm —
/// `golden_reports.rs` names no split alias at all any more, and
/// `harness/mod.rs`'s single `lane::PARITY` read belongs to no row. The arm
/// stays because the spelling is still how the harness reads the lane (×4 in
/// `golden_reports.rs`, ×2 in `harness/regen.rs`, ×1 in `harness/mod.rs`), so
/// the next pin written there must be recognised; it is no longer what keeps
/// any row pinned.)
///
/// The second arm is the **qualified** path only, not the bare word
/// [`reads_the_lane`] settles for. (No line number: the two in-file citations
/// this sentence used to carry both went stale under the teardowns' own line
/// shifts — the rustdoc link resolves without one, and the sibling
/// cross-file numbers below are re-measured with each edit.) The two are not
/// symmetric: there a loose match makes the caller *reject* more (a pin that
/// still reads the lane fails the teardown check — fail-safe), here it makes
/// the caller *accept* more, so an English `PARITY` in a comment would satisfy
/// the rail. Two such comments exist in-tree (`tests/golden_reports.rs:1628`,
/// `tests/corpus_gate/scheduler.rs:358`) while every real read is written
/// `lane::PARITY` (`golden_reports.rs` ×4 — it was ×15 until G2.2a tore down
/// two rows pinned there, ×9 until G2.2c tore down the Fault dump row and ×6
/// until G2.6 tore down the device-name column width, whose two arms it also
/// held — plus `harness/mod.rs:4738`, the kV-value compare and
/// that file's only remaining read; `skip_prop`'s, which was the *first* of its
/// two, went unconditional in G2.2b);
/// `harness/lane.rs`, which uses the bare name because it declares it, names
/// `ORACLE_PARITY` in that same assert and is credited by the first arm.
fn branches_on_lane(text: &str, _alias: &str) -> bool {
    names_token(text, "ORACLE_PARITY") || names_token(text, "lane::PARITY")
}

/// `(alias, the impl each cfg arm selects)` for every lane-selected alias in the
/// three compat modules.
fn lane_aliases(root: &Path) -> Vec<(String, Vec<String>)> {
    let needle = needle();
    let mut out: Vec<(String, Vec<String>)> = Vec::new();

    for module in COMPAT_MODULES {
        let text =
            fs::read_to_string(root.join(module)).unwrap_or_else(|e| panic!("{module}: {e}"));
        let lines: Vec<&str> = text.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if !trimmed.starts_with("#[cfg(") || !trimmed.contains(&needle) {
                continue;
            }
            let Some(next) = lines.get(i + 1) else {
                continue;
            };
            let Some(rest) = next
                .trim()
                .strip_prefix("pub use ")
                .and_then(|r| r.strip_suffix(';'))
            else {
                continue;
            };
            let Some((selected, alias)) = rest.split_once(" as ") else {
                continue;
            };
            let (selected, alias) = (selected.trim().to_owned(), alias.trim().to_owned());
            match out.iter_mut().find(|(a, _)| *a == alias) {
                Some((_, impls)) => impls.push(selected),
                None => out.push((alias, vec![selected])),
            }
        }
    }
    out
}

/// Every lane cfg inside a `compat` module selects an **alias**, never a
/// definition — so [`lane_aliases`] can see the whole split.
///
/// `lane_aliases` recognises exactly one shape: a `#[cfg(…)]` line immediately
/// followed by `pub use X as Y;`. A row written instead as two cfg-selected
/// *definitions* of the same name —
///
/// ```ignore
/// #[cfg(feature = "oracle-parity")]
/// pub const ROW: f64 = 1.0;
/// #[cfg(not(feature = "oracle-parity"))]
/// pub const ROW: f64 = 2.0;
/// ```
///
/// — is a genuine lane split that the parser never sees, so it would not count
/// toward [`SPLIT_ALIAS_POPULATION`] and would never be asked for a pin.
/// Reproduced (F-settle W4): appending such a pair to a *sanctioned* compat
/// module passed every test in this file.
///
/// The rule is therefore mechanical rather than stylistic: inside a compat
/// module, a needle-bearing cfg attribute must be an alias arm, or be one of
/// the [`LANE_CONST_ARMS`] the model itself needs. A comment between the
/// attribute and its item breaks the parser the same way and is rejected here
/// too.
#[test]
fn every_lane_cfg_in_a_compat_module_selects_an_alias() {
    let root = repo_root();
    let needle = needle();
    let mut offenders = Vec::new();
    let mut alias_arms = 0usize;
    let mut const_arms = 0usize;

    for module in COMPAT_MODULES {
        let text =
            fs::read_to_string(root.join(module)).unwrap_or_else(|e| panic!("{module}: {e}"));
        let lines: Vec<&str> = text.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if !trimmed.starts_with("#[cfg(") || !trimmed.contains(&needle) {
                continue;
            }
            let next = lines.get(i + 1).map(|l| l.trim()).unwrap_or("");
            if next.starts_with("pub use ") && next.contains(" as ") {
                alias_arms += 1;
            } else if LANE_CONST_ARMS.iter().any(|c| next.starts_with(c)) {
                const_arms += 1;
            } else {
                offenders.push(format!("    {module}:{}: {next}", i + 2));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "a lane cfg in a compat module must be followed immediately by          `pub use <impl> as <alias>;` — anything else is a split the alias          parser cannot see, so it escapes SPLIT_ALIAS_POPULATION and the pin          requirement. Write both kernels as plain sibling `_impl` items and let          the cfg select only the alias:\n{}",
        offenders.join("\n")
    );
    // Non-vacuity: the walk must have found the real arms.
    assert_eq!(
        alias_arms,
        (SPLIT_ALIAS_POPULATION + DECLARED_NOT_WIRED.len()) * 2,
        "expected two cfg arms per alias"
    );
    assert_eq!(
        const_arms,
        COMPAT_MODULES.len() * 2,
        "each compat module declares `ORACLE_PARITY` in two arms"
    );
}

/// The only non-alias items a lane cfg may guard inside a compat module: the
/// per-crate lane constant, which is what lets a test state a per-lane
/// expectation without repeating the cfg (and what proves the feature reached
/// the crate at all).
const LANE_CONST_ARMS: [&str; 1] = ["pub const ORACLE_PARITY:"];

/// Every lane-split alias is pinned by an expected-value test at an observable.
///
/// The escape register above governs the markers Stage F did *not* resolve. This
/// is its other half: the rows it *did* flip. Each one makes the default lane
/// deliberately answer something the gating oracles do not, so the oracle gates
/// cannot cover it — `DE_PASCALIZE_PLAN.md` IV.2's drift model says such a row is
/// "excluded from oracle comparison at those fields; pinned by their own
/// **expected-value tests**". That obligation was met row by row through F.3 and
/// then recorded in prose, which is precisely the state F.3ab found unsatisfactory
/// for the escapes: a claim nothing re-reads decays into folklore. A deleted pin,
/// a renamed alias, or a new row flipped without a pin now fails here.
///
/// It deliberately does **not** check what a pin asserts — that is the reviewer's
/// job and cannot be mechanized. It checks the two things that can rot silently:
/// that a test names the row, and that it reads the lane rather than hard-coding
/// one side.
#[test]
fn every_lane_split_alias_is_pinned_by_an_expected_value_test() {
    let root = repo_root();
    let aliases = lane_aliases(&root);

    let mut split = Vec::new();
    let mut declared = Vec::new();
    for (alias, impls) in &aliases {
        assert_eq!(
            impls.len(),
            2,
            "{alias}: a lane alias has exactly two cfg arms, found {impls:?}"
        );
        if impls[0] == impls[1] {
            declared.push(alias.clone());
        } else {
            split.push(alias.clone());
        }
    }
    declared.sort();
    assert_eq!(
        declared, DECLARED_NOT_WIRED,
        "the set of aliases whose cfg arms select the same impl moved. Wiring \
         one is a flip: it becomes a deliberate divergence and needs an \
         expected-value pin (and a row in `SPLIT_ALIAS_POPULATION`)"
    );
    assert_eq!(
        split.len(),
        SPLIT_ALIAS_POPULATION,
        "lane-split alias population moved: {split:?}"
    );

    let mut pins: Vec<(String, Vec<String>)> = split.iter().map(|a| (a.clone(), vec![])).collect();
    for path in rust_sources(&root) {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let Some((start, end)) = is_pin_candidate(&path, &root, &text) else {
            continue;
        };
        // Only the test region counts — a production call site above the
        // `#[cfg(test)]` boundary must not be able to pin its own row.
        let region = &text[start..end];
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        for (alias, found) in pins.iter_mut() {
            if names_token(region, alias) && branches_on_lane(region, alias) {
                found.push(rel.clone());
            }
        }
    }

    let unpinned: Vec<&str> = pins
        .iter()
        .filter(|(_, found)| found.is_empty())
        .map(|(alias, _)| alias.as_str())
        .collect();
    assert!(
        unpinned.is_empty(),
        "lane-split alias(es) with no expected-value pin: {unpinned:?}\n  A pin \
         is a test that (a) names the alias — in an assertion or in the doc \
         comment that says which row it pins — and (b) branches on the lane, \
         either through `compat::ORACLE_PARITY` or through the harness alias \
         `lane::PARITY` (qualified — the bare word does not count). Deriving \
         the expectation from the row's own `compat::<alias>` does NOT count \
         either (F-settle W4): engine and test then read the same constant, so \
         the pin holds whichever way the alias points. Kernel-vs-kernel tests \
         inside the compat module do not count: they assert the two impls \
         against each other, not the observable a deck sees."
    );

    // Non-vacuity of the *walk*, in both shapes a pin is allowed to take — a
    // narrowing of `is_pin_candidate` that dropped either would otherwise show
    // up as "everything still passes". The integration-test shape was anchored
    // on `IRESIDUAL_FROM_TERMINAL_1` until G2.2a tore that row down; it now
    // names `PI`, one of the five **numeric** precision-compat survivors
    // (TESTING.md §"Precision-compat rows still split by lane"), which outlive
    // WP-G2 and WP-G4 both — so this anchor does not have to move again with
    // the next teardown.
    for (alias, expected) in [
        (
            "kv_base_search_scale",
            "crates/dss-core/src/solution/solution/dispatch.rs",
        ),
        ("PI", "crates/dss-parser/tests/parser_golden.rs"),
    ] {
        let found = pins
            .iter()
            .find(|(a, _)| a == alias)
            .map(|(_, f)| f.as_slice())
            .unwrap_or_default();
        assert!(
            found.iter().any(|f| f == expected),
            "{alias} is no longer pinned by {expected} (found {found:?}) — if the \
             pin moved, re-anchor it here; if the walk stopped reaching that \
             shape of test file, fix the walk"
        );
        // The walk credits a **bare** token, which is the right rule for a pin
        // (a test may name its row in prose) but too weak for an anchor whose
        // job is to fail loudly: `PI` also occurs in `parser_golden.rs` as the
        // incidental `f64::consts::PI`, so deleting the deliberate `compat::PI`
        // citation would leave this assert green on a std-library homonym —
        // exactly the "everything still passes" outcome the anchor exists to
        // prevent. So re-check the **qualified** spelling in the same region
        // the walk credited. `IRESIDUAL_FROM_TERMINAL_1`, the anchor before
        // G2.2a, had no homonym and needed no such guard; the numeric survivors
        // this WP must anchor on are short names, so the guard travels with them.
        let path = root.join(expected);
        let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{expected}: {e}"));
        let (start, end) = is_pin_candidate(&path, &root, &text)
            .unwrap_or_else(|| panic!("{expected} no longer counts as a pin candidate"));
        assert!(
            names_token(&text[start..end], &format!("compat::{alias}")),
            "{expected} no longer cites `compat::{alias}` by its qualified name \
             — the anchor above was being satisfied by a bare-token homonym"
        );
    }
}

// ---------------------------------------------------------------------------
// The teardown register: the rows that LEFT the two censuses (GOLDEN_REBASE WP-G2)
// ---------------------------------------------------------------------------

/// Which census a torn-down row was counted by *before* it was torn down.
///
/// The two censuses above are decrements-only: a row leaves
/// [`SPLIT_ALIAS_POPULATION`] when its alias stops selecting two different
/// impls, and it leaves [`EXIT_POPULATION`]'s [`Escape::WholeCase`] bucket when
/// its marker is resolved. Neither decrement says *where the fix went*, which is
/// what [`TORN_DOWN_ROWS`] adds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    /// A `compat::` alias whose two cfg arms selected different impls.
    SplitAlias,
    /// A surviving compat marker owned by [`Escape::WholeCase`] — the F.3
    /// measurement priced its clean fix at a whole gated case's default-lane
    /// oracle comparison, which WP-G2 pays with a ledger entry plus a pin.
    WholeCase,
}

/// What, in the tree, still shows that a teardown actually landed.
///
/// A register of past events is only worth reading if it rots when the tree
/// moves under it — the same reason [`ESCAPE_REGISTER`] and
/// `tests/corpus/ledger.json` are checked fail-on-stale. The pin (below) proves
/// the *behaviour*; this proves the *mechanism* the teardown left behind.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
// Only the variants WP-G2 has reached so far are constructed (`Site` since
// G2.1a, `Exclusion` since G2.2a, `Ledger` since G2.5); the last — `None`
// (WP-G4) — is dead code until its first row, and this list is amended by
// the sub-step that lands it. `expect` rather than
// `allow` on purpose: the day the last variant gets its first row, this
// attribute becomes unfulfilled and has to be deleted, instead of quietly
// covering a variant that later goes unused for real.
#[expect(dead_code)]
enum Evidence {
    /// `(file, distinctive slices)` — keyed exactly like [`ESCAPE_REGISTER`]:
    /// the file must still contain **every** slice. The WP-G2 shape for a row
    /// whose teardown made an **engine kernel** unconditional.
    ///
    /// The list is one entry per *site*, not per taste: a single row can carry
    /// two writers (G2.1g's capacitor and load `grounded` kernels), and a row
    /// anchored on only one of them lets the other be reverted with the
    /// register still green — the row→tree direction
    /// `GOLDEN_REBASE_PLAN.md` §G2.0(b) asks to mechanize would then cover half
    /// the teardown. An empty list is refused for the same reason
    /// [`Evidence::None`] is.
    ///
    /// The slice must be the unconditional **code**, not the comment that
    /// explains it. [`ESCAPE_REGISTER`]'s slices are prose because what they
    /// key is a *comment marker*; here the evidence is a kernel, and a
    /// re-introduced alias would re-route the call while leaving any nearby
    /// sentence — however carefully worded — matching.
    ///
    /// It must also **discriminate the torn-down state**, which is not
    /// automatic. Where the fix replaced the call (G2.1a's
    /// `return (data[0], 0.0);` did not exist while the alias did), the
    /// statement is discriminating on its own. Where the fixed form is an
    /// ordinary statement the *split* form also contained — the split merely
    /// wrapped it in a lane branch, which is the common shape — the bare
    /// statement matches both states and the check degrades to "the mechanism
    /// was not deleted outright". Anchor those on the line break plus the
    /// statement's top-level indentation (`"\n        self.x = …"`): re-wrapping
    /// the copy in an `if` re-indents it, so the revert stops matching. The
    /// leading `\n` matches an LF and a CRLF checkout alike (`\r\n` contains
    /// `\n`), and `fs::read_to_string` does no normalization. If even that is
    /// impossible for a row, say so in the row's comment and name the checks
    /// that carry the discrimination instead (the census tie, the ghost check,
    /// and the row's now-unconditional pin run in **both** lanes).
    Site(&'static str, &'static [&'static str]),
    /// `(file, distinctive slices)` for a **harness exclusion** made
    /// unconditional — the same key and the same slice check as
    /// [`Evidence::Site`], plus the obligation that the file carries the
    /// `LANE-EXCLUSION` marker naming the row.
    ///
    /// The two are separate variants only because that obligation cannot be
    /// guessed: `GOLDEN_REBASE_PLAN.md` §G2.0(b) asks for the marker convention
    /// checked "both ways", and only the register knows which of its evidence
    /// sites is an exclusion. A blanket requirement would be wrong — a row
    /// whose fix touched no harness has no exclusion to mark — so the row says
    /// which it is, and [`teardown_markers_and_the_register_agree`] holds it to
    /// it.
    Exclusion(&'static str, &'static [&'static str]),
    /// The `id` of the `tests/corpus/ledger.json` entry that pins the
    /// divergence the fix opened against an oracle channel — the shape G2.5's
    /// engine fixes take, where the observable is a gated corpus case rather
    /// than a source site.
    Ledger(&'static str),
    /// Nothing beyond the pin. Reserved for rows whose teardown leaves no
    /// distinctive site and moves no oracle-compared number — WP-G4's
    /// rendering rows (`GOLDEN_REBASE_PLAN.md` §WP-G4 preamble).
    ///
    /// **Rejected until then**, exactly like a missing pin: every WP-G2
    /// teardown does leave a mechanism, so the first legitimate `None` row
    /// lands by editing the assert that refuses it rather than by slipping
    /// through a hole in it.
    None,
}

/// Every row removed from a census above, with the proof the removal landed.
///
/// `(former row name, which census it left, evidence in the tree, the pin)`.
///
/// # Why a register and not just a smaller number
///
/// Both censuses are single integers, so "row X was torn down" and "row X was
/// quietly stopped being counted" are the same edit. `GOLDEN_REBASE_PLAN.md`
/// WP-G2 removes twenty split aliases and three `WholeCase` markers on the
/// promise that each one's expected-value pin becomes *unconditional* — i.e.
/// that the coverage moved rather than evaporated. That promise is exactly what
/// nothing re-reads unless it is written down executably, so it is written here:
/// the arithmetic ties below make a census decrement impossible without a row,
/// and the checks make a row impossible without a pin that still exists.
///
/// Created **empty** in G2.0, before any deletion, so the first teardown commit
/// has somewhere to land and the rails cannot be retro-fitted around whatever
/// happened to be convenient.
///
/// The pin slot is an `Option` because WP-G4's rendering rows will carry none
/// (plan §WP-G4: "the pin slot is mandatory only for WP-G2 bug rows"), but every
/// row **this** WP adds must carry one, and the check below demands it of every
/// row. Narrowing it is then a deliberate edit in the commit that lands the
/// first pinless row — which is the register's whole point.
const TORN_DOWN_ROWS: &[TornDownRow] = &[
    // G2.1a. Upstream's single-point branch assigns the mean and repeats the
    // same right-hand side into `StdDev` (r4133
    // `Version8/Source/Shared/mathutil.pas:405`/`:429`, identical in the pinned
    // dss_capi 0.14.5) — a `{3.5}` sample reported as 100 % spread. Both gating
    // oracles carry it; the four `mathutil` entry points now return `0.0` in
    // both lanes. Zero-footprint: no golden and no gated corpus case reads a
    // one-point shape's std-dev, so nothing moved but the two pins.
    (
        "stddev_single_point",
        Kind::SplitAlias,
        // The slice is the unconditional *kernel*, not the comment that
        // explains it: a revert re-routes this `return` through the alias while
        // an explanatory sentence next to it can survive untouched. That is the
        // anchor convention for every `Evidence::Site` row — point at code that
        // exists only in the torn-down state. Here that is free (the split form
        // called the alias on this very line); where it is not, see the
        // indentation anchor documented on [`Evidence::Site`].
        Evidence::Site(
            "crates/dss-core/src/support/mathutil/mod.rs",
            &["return (data[0], 0.0);"],
        ),
        Some((
            "crates/dss-core/src/support/mathutil/tests.rs",
            "single_point_std_dev_is_zero",
        )),
    ),
    // G2.1b. `TCapControlObj.MakeLike` copies every reference a CapControl
    // holds — controlled and monitored element, the user model, both snapshots
    // — except the `ControlSignal` shape (`.inputs/dss_capi/src/Controls/
    // CapControl.pas:445-489`, field `:169`; r4133 `Version8/Source/Controls/
    // CapControl.pas:410-465`, field `myShapeObj` `:76`), so a clone of a
    // `type=Follow` controller has nothing to follow and aborts the solve with
    // message 10362. Both lanes now copy it. Zero-footprint: no golden and no
    // gated corpus deck clones a Follow CapControl, so nothing moved but the
    // pins.
    (
        "CAPCONTROL_MAKELIKE_DROPS_CONTROL_SIGNAL",
        Kind::SplitAlias,
        // The copy itself. Unlike G2.1a's row the *fixed* form is an ordinary
        // statement the split form also contained — the split only wrapped it in
        // `if !compat::…` — so the bare statement would match a revert just as
        // well. Hence the leading line break and the eight spaces of `make_like`'s
        // own body: any re-wrapping in a lane branch indents the copy past this
        // needle. Deleting it outright fails the same check, and a revert through
        // a re-introduced alias additionally fails the census tie and the ghost
        // check below.
        Evidence::Site(
            "crates/dss-core/src/elements/control/cap_control/accessors.rs",
            &["\n        self.ctrl_signal_shape = other.ctrl_signal_shape.clone();"],
        ),
        Some((
            "crates/dss-core/src/elements/control/cap_control/tests.rs",
            "make_like_copies_the_control_signal",
        )),
    ),
    // G2.1c. `CalcAndWriteSeqCurrents` seeds `iNormal := NormAmps` and
    // overwrites that seed with `I1/NormAmps*100` only when the rating is `> 0`
    // (`.inputs/dss_capi/src/Common/ExportResults.pas:409-414`; r4133
    // `Version8/Source/Common/ExportResults.pas:355-358` is the same four
    // lines), so a column headed "percent" reports `normamps=-1` as a loading
    // of −1 %. Both gating oracles carry it; both lanes now print the `0` the
    // report's own `else` arm already writes for every unrated row.
    // Zero-footprint: no golden and no gated corpus deck rates an element
    // negatively (at `normamps=0` the two readings coincide), so nothing moved
    // but the pin.
    (
        "SEQ_CURRENTS_PRINTS_RAW_NONPOSITIVE_RATING",
        Kind::SplitAlias,
        // Both columns going through **one** kernel is the torn-down state: the
        // split computed each of them inline with its own lane-branching
        // fallback, so this call pair never existed while the alias did. The
        // leading line break plus the sixteen spaces of the `do_ratings && j
        // == 1` arm hold the indentation half of the anchor convention on
        // [`Evidence::Site`]: re-wrapping the *pair* in a lane branch indents it
        // past this needle.
        //
        // It does **not** discriminate every revert, and per that doc's last
        // paragraph this row says so rather than over-claiming: a re-split
        // written *inside* the closure (`} else if compat::… { rating }`) leaves
        // this line byte-identical, and the whole-closure needle that would
        // catch it is not available — the check is a plain `contains` over the
        // file as checked out, and this repo checks Rust sources out with CRLF
        // (`git ls-files --eol`), which only a **single**-line needle with a
        // leading `\n` survives. For that shape the check degrades to "the
        // kernel was not deleted outright" and the discrimination is carried by:
        // the row's now-unconditional pin, which asserts `(0.0, 0.0)` for
        // `Line.bad` in *both* lanes, so any lane branch that restores the
        // upstream reading fails it wherever it is written; the ghost check
        // below, which fails if this alias ever selects two impls again; and the
        // census tie, which fails if the row leaves the register.
        Evidence::Site(
            "crates/dss-core/src/report/export/seq_currents.rs",
            &["\n                (pct_of_rating(norm_amps), pct_of_rating(emerg_amps))"],
        ),
        Some((
            "crates/dss-core/tests/golden_reports.rs",
            "export_seqcurrents_prints_zero_for_an_undefined_rating",
        )),
    ),
    // G2.1d. `DoReduceShortLines`' merge-with-parent branch opens its
    // capacitor/reactor scan on `ParentNode.FirstShuntObject()` and advances it
    // with `PresentBranch.NextShuntObject()`
    // (`.inputs/dss_capi/src/Meters/ReduceAlgs.pas:200`/`:209`; r4133
    // `Version8/Source/Meters/ReduceAlgs.pas:199`/`:206` is the same pair) — a
    // cross-node cursor mix whose second list is already exhausted
    // (`DSSPointerList.pas:88` leaves `ActiveItem` at the last `Add`), so the
    // scan ends after ONE element and a capacitor at position ≥ 2 is merged
    // onto another bus instead of blocking. Both gating oracles carry it; both
    // lanes now scan the whole list, exactly as the merge-with-child branch of
    // the same procedure (`:246-258`) always did. Zero-footprint: no golden and
    // no gated corpus deck reduces a parent branch whose first shunt is not a
    // capacitor while a later one is.
    (
        "REDUCE_SCANS_ONLY_THE_FIRST_PARENT_SHUNT",
        Kind::SplitAlias,
        // The `any` was the split's *default* arm, so the bare call would match
        // a revert; the needle therefore carries the line break, the twelve
        // spaces of the merge-with-parent block, and the `if` that consumes the
        // scan directly — the split bound it to `parent_blocked` from inside a
        // 16-space `else`, so no lane-branching form of this code can match.
        Evidence::Site(
            "crates/dss-core/src/exec/reduce.rs",
            &["\n            if parent_shunts.iter().any(|&s| self.red_is_cap_or_reactor(s)) {"],
        ),
        Some((
            "crates/dss-core/src/exec/tests/reduce.rs",
            "short_line_merge_scans_every_parent_shunt",
        )),
    ),
    // G2.1e. Both of `TStorageControllerObj`'s terminal branches guard
    // `SetFleetToIdle` with `if not FleetState = STORE_IDLING`
    // (`.inputs/dss_capi/src/Controls/StorageController.pas:1350` "Ran out of
    // OOMPH", `:1619` "Fully charged"; r4133
    // `Version8/Source/Controls/StorageController.pas:1771`/`:2042` is the same
    // unparenthesised pair). Object Pascal binds `not` tighter than `=` over the
    // `Integer` field, so the guard is `(not FleetState) = 0` — true only for
    // `STORE_CHARGING = -1`, i.e. never in the discharging state that reaches
    // "Ran out of OOMPH": the fleet is left discharging at its old kW with no
    // re-solve queued while the event log announces the idling anyway. Both
    // gating oracles carry it; both lanes now ask `FleetState <> STORE_IDLING`,
    // which is how the same unit spells the same test seven other times
    // (`:872`, `:969`, `:994`, `:1162`, `:1252`, `:1450`, `:1515`).
    // Zero-footprint: no golden and no gated corpus deck runs a fleet out of
    // energy while discharging.
    (
        "STORAGE_CONTROLLER_IDLE_TEST_COMPLEMENTS_THE_ORDINAL",
        Kind::SplitAlias,
        // The comparison was the split's *default* arm, so the bare expression
        // would match a revert; the needle therefore carries the line break and
        // the eight spaces of the function body, which is the whole of it, so
        // the shape the alias actually had — the same expression at twelve
        // spaces inside an `else` — stops matching.
        //
        // It does **not** discriminate every revert, and per the last paragraph
        // of [`Evidence::Site`] this row says so rather than over-claiming: an
        // early-return re-split (`if compat::… { return (!…) == …; }`, or the
        // same with a `#[cfg]`-guarded `return`) leaves this statement at
        // exactly these eight spaces while the lane branch is fully restored.
        // The multi-line needle that would catch it is not available: the check
        // is a plain `contains` over the file as checked out, and with
        // `core.autocrlf=true` and no `*.rs` rule in `.gitattributes` a fresh
        // checkout is CRLF, which only a single-line needle with a leading `\n`
        // survives. For that shape the check degrades to "the kernel was not
        // deleted outright" and the discrimination is carried by: the row's
        // now-unconditional pin
        // `fleet_idle_guard_fires_unless_the_fleet_is_already_idling`, which
        // asserts the `Discharging` answer in *both* lanes, so a restored
        // complement fails it wherever it is written; the ghost check below,
        // which fails if this alias ever selects two impls again; and the
        // census tie, which fails if the row leaves the register.
        Evidence::Site(
            "crates/dss-core/src/elements/control/storage_controller/compute.rs",
            &["\n        self.fleet_state != StorageState::Idling"],
        ),
        Some((
            "crates/dss-core/src/elements/control/storage_controller/tests.rs",
            "fleet_idle_guard_fires_unless_the_fleet_is_already_idling",
        )),
    ),
    // G2.1f. `WriteMultipleStorageMeterFiles` (`.inputs/dss_capi/src/Common/
    // ExportResults.pas:2240`; r4133 `Version8/Source/Common/
    // ExportResults.pas:2280` — the live line; the `Storage2` twin repeats it
    // at `:2335` but is inert, sitting inside the `(*` … `*)` block that spans
    // `:2314-:2368`) was cloned from `WriteMultiplePVSystemMeterFiles`
    // (`:2095`) and kept its `'EXP_PV_'`
    // literal, so `Export Storage_Meters /m` drops a Storage fleet's registers
    // into the PVSystem export's own files — and because that writer emits a
    // header only for a file that does not yet exist and otherwise appends
    // (`:2242`), a same-named PVSystem and Storage interleave their rows under
    // whichever class's header landed first. The same command's single-file
    // mode already writes `EXP_STORAGEMeters.csv` (`ExportOptions.pas:411`)
    // next to `EXP_PVMeters.csv` (`:409`), and `EXP_MTR_`/`EXP_GEN_` (`:1792`,
    // `:1948`) show the one-prefix-per-class rule, so the fixed spelling is not
    // a choice. Both gating oracles carry it; both lanes now write
    // `EXP_STORAGE_`. Zero-footprint: only a *file name* moves — the rows are
    // byte-identical (the pin still compares them against the oracle-anchored
    // single-file golden), the single-file path is untouched, and no golden or
    // gated corpus deck runs `Export Storage_Meters /m` at all.
    (
        "STORAGE_MULTIFILE_USES_THE_PV_PREFIX",
        Kind::SplitAlias,
        // The literal was the split's *default* arm, so the bare string would
        // match a revert. The needle therefore carries the line break and the
        // twenty spaces of the tuple element, plus the trailing comma: in the
        // split form the same literal sat at the same depth as the tail
        // expression of an `else` block — no comma — and the tuple's third
        // element was the `prefix` binding. Re-introducing a lane branch cannot
        // leave the literal as a comma-terminated tuple element. Single-line by
        // necessity: this repo checks Rust sources out with CRLF, which no
        // multi-line needle survives.
        Evidence::Site(
            "crates/dss-core/src/exec/report.rs",
            &["\n                    \"EXP_STORAGE_\","],
        ),
        Some((
            "crates/dss-core/tests/golden_reports.rs",
            "export_storage_multifile_uses_the_storage_prefix",
        )),
    ),
    // G2.1g. The CIM writer answers "is this wye point earthed?" with a
    // hard-coded `TRUE` for a capacitor (`.inputs/dss_capi/src/Common/
    // ExportCIMXML.pas:3700`; r4133 `Version8/Source/Common/
    // ExportCIMXML.pas:3183`) and for a load (`:4478`; r4133 `:3854`), each
    // under upstream's own `// TODO - check bus 2` — so a wye with an isolated
    // or impedance-earthed neutral is handed to the receiving tool as solidly
    // grounded, which changes its earth-fault and zero-sequence answers. The
    // fix is the **same unit's** transformer writer's test
    // (`XfmrTankPhasesAndGround` `:1531-1570`: `NodeRef[j2] = 0`, "last
    // conductor is grounded solidly") applied where each shunt class keeps its
    // neutral — the capacitor's second terminal, the load's `Nphases+1`-th
    // conductor. Both gating oracles carry the quirk; both lanes now read the
    // model. Zero-footprint, measured: no CIM golden deck and no gated corpus
    // deck has a wye capacitor with an explicit `bus2=` or a wye load with a
    // non-ground neutral, so all 15 CIM goldens stay byte-identical (the
    // `golden_cim::expected_cim` transform — `lane_expected_cim` until G2.2c
    // made it lane-independent — gained no third entry) and nothing moved but
    // the pin.
    (
        "CIM_WYE_GROUNDED_IS_HARDCODED_TRUE",
        Kind::SplitAlias,
        // Both halves of this two-site row — the capacitor's terminal-2 reading
        // and the load's neutral reading — get their own needle: the pin
        // asserts both, but a register anchored on one of them would stay green
        // while the other was reverted, and the row→tree direction is exactly
        // what this register exists to mechanize.
        //
        // Each reading existed in the split form too, as the right operand of
        // `alias ||`. The capacitor's was pushed onto its own line four spaces
        // deeper by rustfmt's binary-operator wrap; the load's shared the line
        // with `crate::compat::…` at this very indentation. So both needles
        // carry the line break and the sixteen spaces of `boolean_node`'s own
        // argument list plus the trailing comma: neither can match a line that
        // begins with a lane branch, and deleting a reading outright fails the
        // same check. Single-line by necessity (CRLF checkout).
        Evidence::Site(
            "crates/dss-core/src/cim/export.rs",
            &[
                "\n                snap.term2_nodes.iter().all(|&n| n == 0),",
                "\n                snap.neutral_node == 0,",
            ],
        ),
        Some((
            "crates/dss-core/tests/golden_cim.rs",
            "cim_wye_grounded_reads_the_neutral",
        )),
    ),
    // G2.1h. A height-unit change on the Carson engine re-reads the offset
    // under the new unit — and upstream re-reads the wrong number:
    // `Set_FuserHeightUnit` moves the unit field and then calls
    // `Set_FheightOffset(FheightOffset)` (r4133 `Version8/Source/General/
    // LineConstants.pas:689-696`), passing a field declared "The height is
    // always saved in meters here" (`:71`, `:97`) into a setter whose argument
    // is a *user-unit* number it multiplies by `To_Meters` (`:676-687`), so the
    // same number is converted twice (5 ft → 1.524 m → 1.524 in). The fix is
    // the getter the class already has, called one statement earlier
    // (`Get_FheightOffset`, `:396-399`). The height-offset surface does not
    // exist in the pinned 0.14.5 backend at all (no `FheightOffset` in its 186
    // `.pas`), so this row is cited against r4133 only and gates on the `r4133`
    // channel. Zero-footprint, measured: the sole consumer
    // (`line_geometry::matrix::set_line_constants_medium`) pushes into a
    // freshly built engine at its constructed `UNITS_M` on the first build,
    // where both readings coincide, and re-enters with the unit unchanged on
    // every later build, where the setter returns early. The readings part only
    // after an `Edit Line.<n> HeightUnit=` plus a rebuild, which no golden and
    // no gated deck performs — so the gated deck
    // `modes/upgrade/upgrade_linecs_heightoffset.dss` and every golden are
    // byte-identical either way and nothing moved but the pin.
    (
        "HEIGHT_UNIT_CHANGE_REREADS_THE_METRES_FIELD",
        Kind::SplitAlias,
        // The `self.height_offset()` read existed in the split form too — as
        // the `else` arm of the lane branch, indented four spaces deeper and
        // without the binding. The needle therefore carries the line break, the
        // method body's own eight spaces and the `let typed =` binding the
        // fixed form introduced: re-wrapping the read in a lane branch re-indents
        // it and drops the binding, and deleting it outright fails the same
        // check. Single-line by necessity (CRLF checkout).
        Evidence::Site(
            "crates/dss-core/src/support/line_constants/mod.rs",
            &["\n        let typed = self.height_offset();"],
        ),
        Some((
            "crates/dss-core/src/support/line_constants/tests.rs",
            "height_unit_change_rereads_the_typed_number",
        )),
    ),
    // G2.2a, row 1. `CalcAndWriteSeqCurrents` is called once per terminal `j`
    // over a buffer that holds **all** terminals, and applies the offset that
    // fact requires — `k := (j-1)*Ncond + i` — to its symmetric components
    // (r4133 `Version8/Source/Common/ExportResults.pas:323`; dss_capi
    // `ExportResults.pas:367`) but not to the residual sum three dozen lines
    // later, which accumulates `cBuffer^[i]`, i = 1..Ncond (r4133 `:365-366`;
    // dss_capi `:422-424`). Every terminal row of an element therefore prints
    // terminal 1's residual beside its own I1/I2/I0/%NEMA — oracle-proven on
    // IEEE13 `Line.671680`, true terminal-2 residual 9.8e-12 A against the
    // printed 2.83e-5 A. Both gating oracles carry it; both lanes now sum the
    // row's own terminal, which is the slice the element's `Iterminal` already
    // holds (no solved quantity moves). The first WP-G2 row a golden compare
    // observes: the `Terminal >= 2` cells of `export_seqcurrents` were excluded
    // in the default lane only and are now excluded unconditionally — an
    // exclusion, not a regenerated golden, so no golden byte moved.
    (
        "IRESIDUAL_FROM_TERMINAL_1",
        Kind::SplitAlias,
        // The first row whose teardown left a mechanism in *two* files — the
        // engine kernel (`seq_currents.rs`: `let base = (j - 1) * ncond;`) and
        // the harness exclusion — while `Evidence` keys one file per row. The
        // exclusion is what is recorded, because it is the half that carries
        // the [`Evidence::Exclusion`] marker obligation; the needle is the
        // unconditional `push`, which under the split sat four spaces deeper
        // inside `if !lane::PARITY { … }`, so no lane-branching form matches it.
        // Single-line by necessity (this repo checks Rust sources out with
        // CRLF, which no multi-line needle survives).
        //
        // Per the last paragraph of [`Evidence::Site`], what carries the engine
        // half instead is named rather than left implied: the row's pin asserts
        // the own-terminal reading in **both** lanes, so a re-split engine fails
        // it whichever way the branch is written; and re-conditioning the
        // exclusion alone fails `export_seqcurrents_matches_oracle` in the
        // parity lane, where the golden still carries the upstream residual.
        Evidence::Exclusion(
            "crates/dss-core/tests/golden_reports.rs",
            &["\n    col_tol.push(ColTol {"],
        ),
        Some((
            "crates/dss-core/tests/golden_reports.rs",
            "export_seqcurrents_iresidual_sums_the_rows_own_terminal",
        )),
    ),
    // G2.2a, row 2. `CalcReliabilityIndices` sizes `FeederSections` to **this**
    // meter's `SectionCount` (r4133 `Version8/Source/Meters/EnergyMeter.pas:2507`;
    // dss_capi `EnergyMeter.pas:2461`) and keeps its per-section loop inside it
    // (r4133 `:2561-2563`), then writes the bus durations while walking **every
    // circuit bus** (r4133 `:2567-2574`; dss_capi `:2521-2526`). A foreign
    // `BusSectionID` survives to be read there because the zeroing that clears
    // it is itself per-zone (r4133 `:2472` walks this meter's `SequenceList`),
    // so with two meters the later one overwrites the earlier one's bus
    // durations from its own unrelated sections and the reported column depends
    // on meter order. Both gating oracles carry that (in-range) regime; both
    // lanes now walk only the buses this meter's own forward sweep numbered.
    // The out-of-range regime — an OOB heap read, proven nondeterministic — was
    // never reproduced in either lane and has no defined value to pin. The
    // `Duration` column of `export_busreliability_multimeter` was excluded in
    // the default lane only and is now excluded unconditionally; no golden byte
    // moved, and no live gate reads `Bus.Int_Duration` yet (WP-G1's G1.6 adds
    // it, which is why the plan orders this row first).
    (
        "BUS_INT_DURATION_WALKS_ALL_BUSES",
        Kind::SplitAlias,
        // Same two-file shape as the row above, recorded the same way: the
        // needle is the unconditional `let col_tol = vec![ColTol {`, which under
        // the split read `let col_tol = if lane::PARITY { vec![] } else { … }`.
        // The engine half (`reliability.rs`: the duration loop at the function
        // body's own four spaces, four spaces shallower than the `else` arm the
        // split gave it) is carried by the row's pin, which asserts the
        // zone-scoped durations in **both** lanes, and by
        // `export_busreliability_multimeter_matches_oracle`, which fails in the
        // parity lane if the exclusion alone is re-conditioned.
        Evidence::Exclusion(
            "crates/dss-core/tests/golden_reports.rs",
            &["\n    let col_tol = vec![ColTol {"],
        ),
        Some((
            "crates/dss-core/tests/golden_reports.rs",
            "export_busreliability_multimeter_duration_stays_in_the_meters_zone",
        )),
    ),
    // G2.2b, row 1. `TMonitorObj.Create` re-assigns `Basefrequency := 60.0`
    // after the inherited `TDSSCktElement.Create` already set `BaseFrequency :=
    // ActiveCircuit.Fundamental` (`.inputs/dss_capi/src/Meters/Monitor.pas:472`
    // == r4133 `Version8/Source/Meters/Monitor.pas:552`; the base-class
    // statement is `Common/CktElement.pas:203`), so the Monitor is the one
    // element that does not know the circuit's frequency. That it is left-over
    // and not meant is the same file family's own testimony: `Line.pas:974` and
    // `GICLine.pas:373` carry the identical assignment commented out with "set
    // in base class", and the sibling measurement classes (EnergyMeter, Sensor)
    // never write the field. Its one physical consumer is mode-4 flicker —
    // `Monitor.pas:1657` hands the field to `FlickerMeter` as `fBase`
    // (`Pstcalc.pas:594`), where `fBase = 50.0` selects the IEC 61000-4-15
    // 230 V/50 Hz lamp weighting over the 120 V/60 Hz set (`:609-626`) — so a
    // 50 Hz feeder's Pst comes out on the wrong curve. Both gating oracles
    // report the 60.0; both lanes now inherit. The only oracle-compared
    // observable is `Monitor.BaseFreq` on the single 50 Hz gated deck
    // (`LVTestCase`), whose property exclusion was default-lane-only and is now
    // unconditional; no golden byte moves (every golden deck is 60 Hz, where
    // the two readings coincide) and no Pst number moves (every mode-4 deck in
    // the corpus is 60 Hz).
    (
        "monitor_base_frequency",
        Kind::SplitAlias,
        // The harness exclusion, which is the half carrying the
        // [`Evidence::Exclusion`] marker obligation. The needle is the
        // unconditional binding: under the split this line read
        // `let lane_skipped = !lane::PARITY` with the list on the next one, so
        // no lane-branching form of *this statement* matches, and deleting the
        // exclusion outright fails the same check.
        //
        // Per the last paragraph of [`Evidence::Site`], what it does not
        // discriminate is named rather than left implied: a re-split written as
        // an early `if lane::PARITY { return skip_prop_ub(class, prop); }`
        // above this line would leave the needle intact. That shape is carried
        // by the row's now-unconditional pin — which asserts the inherited 50 in
        // *both* lanes, so any restored 60.0 kernel fails it however the branch
        // is written — plus the ghost check and the census tie below. The engine
        // half (`exec/command.rs`: `.base_frequency = fundamental;` with no
        // `is_monitor` arm at all) is carried by the same pin.
        Evidence::Exclusion(
            "crates/dss-core/tests/harness/mod.rs",
            &["\n    let lane_skipped = LANE_SKIP_PROPS"],
        ),
        Some((
            "crates/dss-core/src/exec/tests/base_frequency.rs",
            "monitor_basefreq_inherits_the_fundamental",
        )),
    ),
    // G2.2b, row 2. `TIsourceObj.PropertySideEffects`
    // (`.inputs/dss_capi/src/PCElements/Isource.pas:221-262`) has three cases —
    // `Phases`, `bus1`, `Daily` — and no `bus2`, so the `Bus2Defined` flag the
    // class declares (`:76`), copies in `MakeLike` (`:302`) and clears in the
    // constructor (`:323`) is never set, and the `bus1` case's
    // `if not Bus2Defined then SetBus(2, S2)` (`:242-255`) re-derives the
    // grounded-Y default over an explicit `Bus2=` parsed earlier in the same
    // edit. r4133 shares the hole exactly (`Version8/Source/PCElements/
    // Isource.pas`: declaration `:61`, copy `:335`, constructor `:398`, the
    // `If Not Bus2Defined Then` guard inside `IsourceSetBus1` `:354-366`, no
    // assignment anywhere). The sibling classes set the flag on that very
    // property — `Vsource.pas:497-498`, r4133 `:468`; `Capacitor.pas:346-350` —
    // so the omission is a hole, not a design. Both gating oracles carry it;
    // both lanes now latch. The one observable is the props scenario
    // `isource_bus2_clobbered_by_bus1`'s `Bus2` cell, whose exclusion was
    // default-lane-only and is now unconditional; the golden
    // `tests/golden/props/isource.json` keeps its captured `b1.0.0.0` byte for
    // byte, and no solved deck writes `bus2=` before `bus1=` on one Isource
    // edit.
    (
        "ISOURCE_BUS2_NEVER_LATCHES",
        Kind::SplitAlias,
        // Same shape as the row above: the recorded half is the harness
        // exclusion, and the needle is the unconditional `if`, which under the
        // split read `if !dss_core::compat::ORACLE_PARITY` with the list on the
        // following line. A lane-branching form of this statement cannot match
        // it, and deleting the exclusion fails the same check; an early-return
        // re-split placed above it would not, which the row's pin
        // (`b2` survives the second `Bus1=`, asserted in both lanes), the ghost
        // check and the census tie cover instead. The engine half is
        // `elements/pc/isource/accessors.rs`'s bare `BUS2 =>` arm.
        Evidence::Exclusion(
            "crates/dss-core/tests/props_roundtrip.rs",
            &["\n            if LANE_SKIP_SCENARIO_PROPS"],
        ),
        Some((
            "crates/dss-core/src/elements/pc/isource/tests.rs",
            "bus2_latches_like_the_sibling_class",
        )),
    ),
    // G2.2c, row 1. `TFaultObj.DumpProperties` writes its own
    // `~ MinAmps=%.1f` line and then runs the generic tail from
    // `NumPropsthisClass`, which this class defines as `Ord(High(TProp))` = 9 =
    // `MinAmps` itself (`.inputs/dss_capi/src/PDElements/Fault.pas:533` with
    // `:134`; r4133 `Version8/Source/PDElements/Fault.pas:594` with
    // `Const NumPropsthisclass = 9` `:107`), so the loop's first iteration
    // re-emits the property just written — in the generic spelling, giving the
    // pair `~ MinAmps=3.0` / `~ MinAmps=3`. The `+ 1` every sibling class with
    // that loop writes (`Transformer.pas:1276`, `AutoTrans.pas:1307`,
    // `XfmrCode.pas:663`) is the fix, and no class prints a property twice on
    // purpose. Both gating oracles carry the reprint; both lanes now start the
    // tail at `NormAmps`. The four dump goldens that carry the pair keep every
    // byte: `golden_reports::fault_dump_expected` drops the second line of each
    // pair from the oracle text in both lanes instead.
    (
        "FAULT_DUMP_TAIL_REPRINTS_MINAMPS",
        Kind::SplitAlias,
        // The engine kernel, which is the half a needle can discriminate: the
        // split wrote `let tail_start = if compat::… { prop::MINAMPS } else {
        // prop::NORMAMPS };` and passed `tail_start`, so a re-introduced branch
        // cannot leave `prop::NORMAMPS` as this call's argument. Single-line by
        // necessity (CRLF checkout).
        //
        // Per the last paragraph of [`Evidence::Site`], the other half is named
        // rather than left implied: the harness drop (`golden_reports.rs`, which
        // carries this row's `LANE-EXCLUSION` marker) is held by the row's pin —
        // it asserts, in both lanes, that the expectation keeps exactly one
        // `~ MinAmps=` per Fault and that the survivor is the custom `%.1f`
        // render — and re-conditioning the drop alone fails
        // `dump_fault_matches_oracle` and its three siblings, which compare the
        // engine against that expectation in both lanes.
        Evidence::Site(
            "crates/dss-core/src/elements/pd/fault/dump.rs",
            &["\n        dump::generic_props_from(out, cx, self, prop::NORMAMPS);"],
        ),
        Some((
            "crates/dss-core/tests/golden_reports.rs",
            "fault_dump_goldens_carry_the_double_print",
        )),
    ),
    // G2.2c, row 2. One `if` in the CIM shunt-compensator writer emits the same
    // attribute under two class prefixes: `BooleanNode(FunPrf,
    // 'ShuntCompensator.grounded', TRUE)` for a wye bank
    // (`.inputs/dss_capi/src/Common/ExportCIMXML.pas:3700`; r4133
    // `Version8/Source/Common/ExportCIMXML.pas:3183`) and `BooleanNode(FunPrf,
    // 'LinearShuntCompensator.grounded', FALSE)` for a delta one six lines below
    // (`:3706`; r4133 `:3187`). CIM100 declares `grounded` on
    // `ShuntCompensator`, so the delta spelling resolves against no property —
    // a strict consumer rejects it, a lenient one drops the flag. Both gating
    // oracles carry it; both lanes now write the sibling arm's name. Only the
    // element name moves (the value is `false` in both), and no golden byte
    // moves: `golden_cim::expected_cim` applies the rename to the oracle text in
    // both lanes.
    (
        "CIM_DELTA_SHUNT_GROUNDED_USES_LINEAR_PREFIX",
        Kind::SplitAlias,
        // This is the row whose engine half a needle genuinely cannot
        // discriminate, and [`Evidence::Site`]'s last paragraph says to record
        // that rather than pretend: the fixed delta arm writes
        // `"ShuntCompensator.grounded"` as `boolean_node`'s third argument at
        // sixteen spaces — byte-identical to the line the *wye* arm of the same
        // `if` already had, split form included, because that arm's own value
        // argument keeps the call broken across lines. Any needle over
        // `cim/export.rs` therefore matches the reverted tree too.
        //
        // So the recorded evidence is the harness half, which does
        // discriminate: `expected_cim` was `lane_expected_cim` — a name that
        // began with a lane early-return — until this teardown made the rewrite
        // unconditional and renamed it. What holds the engine half is the row's
        // pin, which asserts in **both** lanes that the expectation carries the
        // corrected name and none of the upstream one, plus the CIM byte
        // compares that hold the writer to that expectation in both lanes: a
        // re-split engine fails them whichever way its branch is written, and so
        // does re-conditioning the rewrite alone.
        Evidence::Exclusion(
            "crates/dss-core/tests/golden_cim.rs",
            &["\nfn expected_cim(oracle: &str) -> (String, [usize; 2]) {"],
        ),
        Some((
            "crates/dss-core/tests/golden_cim.rs",
            "cim_writer_divergences_are_pinned",
        )),
    ),
    // G2.2c, row 3. The symmetrical-components branch of the CIM line writer
    // closes its `bch`/`gch`/`b0ch`/`g0ch` quartet with `DoubleNode(EpPrf,
    // 'ACLineSegment.b0ch', 0.0)` immediately after the real
    // `DoubleNode(EpPrf, 'ACLineSegment.b0ch', Len * C0 * val)`
    // (`.inputs/dss_capi/src/Common/ExportCIMXML.pas:4367` after `:4366`; r4133
    // `Version8/Source/Common/ExportCIMXML.pas:3756` after `:3755`) — a copied
    // line whose value was replaced and whose name was not. The segment is
    // exported with no `g0ch` and two contradictory `b0ch` nodes, so a consumer
    // taking the last one reads the zero-sequence susceptance as 0. The
    // `PerLengthSequenceImpedance` sibling of the same procedure (`:4521-4522`;
    // r4133 `:3895-3896`) writes the quartet correctly, which is where the fix
    // comes from. Both gating oracles carry the duplicate; both lanes now name
    // it `g0ch`. Only the element name moves (the value stays `0.0`), and no
    // golden byte moves — `golden_cim::expected_cim` renames the second node in
    // both lanes.
    (
        "CIM_ACLINESEGMENT_G0CH_WRITTEN_AS_B0CH",
        Kind::SplitAlias,
        // Inside the keyed file `cim/export.rs` the literal
        // `"ACLineSegment.g0ch"` is unique — the sibling writer earlier in the
        // same file spells `"PerLengthSequenceImpedance.g0ch"` — so the needle
        // cannot be satisfied by some other writer in the same file. (It is
        // *not* unique tree-wide: `golden_cim.rs`'s rewrite names it too, and so
        // does this register's own needle. Both are outside the keyed file,
        // which the check never reads for this row, and neither could keep it
        // green — the slice is looked up in `cim/export.rs` alone.) What
        // discriminates the torn-down state is the needle's *shape*: under the
        // split the call sat inside an `else` arm eight spaces deeper, which
        // rustfmt had to break across six lines, so only the unbranched form can
        // be this whole one-line call at `double_node`'s own sixteen spaces.
        // Single-line by necessity (CRLF checkout).
        //
        // The harness half is carried the same way as row 2's: by the shared pin
        // (which asserts, in both lanes, that no two consecutive `b0ch` nodes
        // survive in the expectation) and by the CIM byte compares.
        Evidence::Site(
            "crates/dss-core/src/cim/export.rs",
            &[
                "\n                writer::double_node(&mut buf, ProfileChoice::Ep, \"ACLineSegment.g0ch\", 0.0);",
            ],
        ),
        Some((
            "crates/dss-core/tests/golden_cim.rs",
            "cim_writer_divergences_are_pinned",
        )),
    ),
    // G2.2d, row 1. `TRelayObj.Sample` closes its `FPresentState` resync with a
    // bare `AppendtoEventLog('Debug Sample: Relay.' + Name, 'FPresentState: …')`
    // (r4133 `Version8/Source/Controls/Relay.pas:1325`) — no `if DebugTrace`,
    // and not gated on `ShowEventLog` either, so every relay writes one debug
    // line per control sample straight into the user-facing event log. Exactly
    // one line lost that guard: the Recloser's byte-identical line carries it
    // (`Recloser.pas:1044`), so do the class's own sibling traces (`:1822`,
    // `:1845`), and r4088 had no such line in `Sample` at all — it arrived with
    // the r4133 per-phase rewrite. Both lanes now route it through the
    // `Relay::dbg` helper the port already had. No golden byte moves (no golden
    // captures a relay event log); the 17 gated `oracle: "r4133"` cases that
    // carry a relay and compare an event log (nine under `controls/relay/`, two
    // under `controls/combo/`, two under `controls/fuse/indmach_r4133/`, the
    // four TD21 decks) keep every other line
    // oracle-compared, because `harness::lane::expected_eventlog` drops these
    // lines from the capture in **both** lanes.
    (
        "RELAY_SAMPLE_TRACE_IGNORES_DEBUGTRACE",
        Kind::SplitAlias,
        // The engine kernel. The split form called `self.dbg(…)` too — in the
        // `else` arm of `if compat::…`, four spaces deeper — so per
        // [`Evidence::Site`]'s indentation convention the needle carries the
        // line break plus the twelve spaces of the trace block's own level: a
        // re-wrap in a lane branch re-indents the call past it. Single-line by
        // necessity (CRLF checkout), and the only `self.dbg(` call in the keyed
        // file.
        Evidence::Site(
            "crates/dss-core/src/elements/control/relay/mod.rs",
            &["\n            self.dbg(ctx, &el, &action);"],
        ),
        Some((
            "crates/dss-core/src/elements/control/relay/tests.rs",
            "sample_state_trace_follows_debugtrace",
        )),
    ),
    // G2.2d, row 2. Both `CTRL_RESET` arms of `TRelayObj.DoPendingAction` log
    // `'Recloser.' + Self.Name` (r4133 `Relay.pas:1196` and `:1212`) — verbatim
    // copies of `Recloser.pas:909`/`:924`, format strings and `ShowEventLog`
    // guard included — while all eight other events of the same procedure
    // (`:1087`-`:1176`) write `'Relay.' + Self.Name`, and both earlier revisions
    // of these two lines label them correctly (r4088 `Relay.pas:971`, the pinned
    // 0.14.5 `Relay.pas:1003` via `Self.FullName`). The log then attributes a
    // relay's reset to a recloser that does not exist — or, if the circuit holds
    // one of that name, to the wrong device. Only the label moves; the reset
    // itself (`OperationCount := 1`, the TD21 quiet window) is untouched. Both
    // lanes now name the emitting class, and `expected_eventlog` relabels the
    // capture in both — but only where the named device really is a Relay of
    // that circuit and not a Recloser, so a genuine recloser reset (identical
    // wording) is never rewritten.
    (
        "RELAY_RESET_EVENT_IS_LABELLED_RECLOSER",
        Kind::SplitAlias,
        // The engine kernel, and it discriminates on its own: the split form
        // was `let reset_device = if compat::… {` with the two `format!`s in
        // its arms, so the whole `let` on one line at the sixteen spaces of the
        // `ControlAction::Reset` arm exists only in the torn-down state.
        // Single-line by necessity (CRLF checkout).
        Evidence::Site(
            "crates/dss-core/src/elements/control/relay/mod.rs",
            &[
                "\n                let reset_device = format!(\"Relay.{}\", self.ccd.cd.obj.name());",
            ],
        ),
        Some((
            "crates/dss-core/src/elements/control/relay/tests.rs",
            "do_pending_reset_only_resets_opcount_d4",
        )),
    ),
    // G2.3. `DoNewtonSolution` increments `SolutionCount` *before* its
    // per-iteration `SumAllCurrents` ("SumAllCurrents Uses ITerminal So must
    // force a recalc", `.inputs/dss_capi/src/Common/Solution.pas:944`, the sum
    // at `:947-948`), so every element leaves that loop with `Iterminal`
    // stamped from the pre-final guess `NodeV_{n-1}` and *marked solved for the
    // live `SolutionCount`* (`CktElement.pas:542-550`, the mark at `:548`);
    // only then does `NodeV -= dV` run (`:965-968`). A post-solve
    // `Get_Powers`/`Get_Losses` therefore takes the cache-aware path, finds the
    // mark fresh, and multiplies the converged `NodeV_n` by the conjugate of
    // the *previous* step's current, while `CktElement.Currents` recomputes at
    // `NodeV_n` — one element, one read, `S != V·conj(I)`, which is the
    // identity `Powers` is defined by. Not escapable by bumping the oracle: the
    // EPRI channel reproduces it in v9.8 (r3723), v10.2 (r4088) and v11.0
    // (r4133) alike, all fingerprint 0.478 kVA (checked 2026-07-08). Both lanes
    // now call `refresh_iterminal` once and feed Powers, Losses and Currents
    // from that one current. Its only oracle-compared observable is the two
    // gated `modes/newton/` decks' element powers/losses (4.86e-4 and 2.46e-3
    // kVA on `Vsource.source` conductor 0, ~60x and ~35x their tier floors),
    // whose `LANE_SKIP_ELEM_POWERS` exclusion was default-lane-only and is now
    // unconditional; their currents, voltages, Y, discrete state and iteration
    // count stay oracle-compared, and no golden byte moves (no golden deck runs
    // a Newton solve).
    (
        "POWERS_REUSE_STALE_NEWTON_ITERMINAL",
        Kind::SplitAlias,
        // The harness exclusion, which is also the half that discriminates: the
        // needle is the unconditional `if`, which under the split read
        // `if !PARITY && LANE_SKIP_ELEM_POWERS.contains(&label) {`.
        //
        // Per the last paragraph of [`Evidence::Site`], the engine half is named
        // rather than left implied, because no needle over `exec/view.rs` can
        // carry it: the torn-down form is a bare
        // `elem.refresh_iterminal(&sys, &node_v);` at `snapshot_elements`'
        // sixteen spaces — byte-identical to the line the *Currents* read three
        // dozen lines below already had, split form included — so every
        // candidate slice matches a reverted tree too. What holds it instead is
        // the row's now-unconditional pin (Newton powers == the normal
        // algorithm's, asserted in both lanes, ten orders of magnitude away from
        // the stale reading) together with the tripwire next to it, which
        // asserts in both lanes that a Newton solve really does leave that stale
        // cache behind — so a re-split engine fails the pin whichever way its
        // branch is written.
        Evidence::Exclusion(
            "crates/dss-core/tests/harness/lane.rs",
            &["\n    if LANE_SKIP_ELEM_POWERS.contains(&label) {"],
        ),
        Some((
            "crates/dss-core/src/exec/tests/newton.rs",
            "newton_powers_match_the_normal_algorithm",
        )),
    ),
    // G2.4. The only row this module ever carried whose upstream was not Pascal:
    // the `[0.0]` the parity lane emitted is fabricated by the client-side
    // ByteStream decoders of both gating channels, not by any engine.
    // dss-python's `IMonitors.Channel` (`dss/IMonitors.py:28-55`) never calls
    // `Monitors_Get_Channel`: it pulls the raw `ByteStream` and short-circuits
    // `if cnt == 272: return np.zeros((1,), dtype=np.float32)`, 272 being the
    // header-only stream size. The `r4133` channel's captures come from our own
    // bridge, which decodes that same stream "exactly like dss-python"
    // (`crates/dss-epri/src/dss.rs:625-634`) — that decoder, not any Pascal
    // accessor, is the load-bearing evidence on that channel. So the parity lane
    // was reproducing a client library rather than an oracle engine, and the
    // teardown is mostly a reclassification: both lanes report the empty channel
    // (which is also what the neighbouring `dbl_hour` read of the same stream
    // has always reported), and `expected_monitor_channel` normalizes the
    // placeholder out of the capture lane-independently *and*
    // channel-independently — scoping it to `capi_v0145` was measured and reds
    // the three gated `modes/time/generaltime*` decks on `r4133`.
    //
    // **The engine half, corrected 2026-08-06 after audit.** Neither authority
    // returns the empty channel here either, so the teardown *also* declines an
    // upstream defect (2026-08-02 policy) rather than being purely a
    // reclassification. `Monitors_Get_Channel` keeps its empty `DefaultResult`
    // (`CAPI_Monitors.pas:304`) only for `SampleCount <= 0` (`:308`) or an
    // invalid index (`:313-320`); with samples taken and nothing flushed —
    // `TakeSample` increments `SampleCount` (`Monitor.pas:1195`), only `Save`
    // grows the stream (`:1122-1125`) — it returns `SampleCount` zeros read out
    // of a zero-filled `AllocMem` buffer whose reads all fail at EOF
    // (`:321-330`), and r4133's `DMonitors.pas:509-541` pads `[0]` only at
    // `SampleCount = 0` and otherwise walks that same unwritten region. Neither
    // is reachable through the two clients (both short-circuit at `cnt == 272`),
    // so the divergence is unobservable on either gating channel: no golden byte
    // moves (no golden deck leaves a monitor unflushed) and no ledger entry is
    // owed — a ledger entry must name a divergence the gate can see.
    (
        "MONITOR_CHANNEL_PADS_THE_UNFLUSHED_STREAM",
        Kind::SplitAlias,
        // The engine kernel, and it discriminates on its own: the split had two
        // separate early returns — an index guard, then `if self.flushed_records
        // == 0 { return if compat::… { vec![0.0] } else { Vec::new() }; }` — and
        // the torn-down form folds the second condition into the first, so this
        // whole `if` line at `channel`'s own eight spaces exists only after the
        // teardown. Single-line by necessity (CRLF checkout).
        //
        // The harness half — the now unconditional `expected_monitor_channel`,
        // which carries this row's `LANE-EXCLUSION` marker — is held by the pin
        // (the empty channel asserted in both lanes, so a re-split engine fails
        // it however its branch is written) together with two unit tests next to
        // the transform: `monitor_transform_is_the_unflushed_placeholder`
        // (the rewrite fires for the placeholder and for nothing else, in both
        // lanes) and `monitor_pad_liveness_is_asserted_not_assumed`, which holds
        // the counters `assert_monitor_pad_is_live` reads at the end of the
        // corpus gate. That last one exists because the shape guards alone can
        // no longer see one rot: a client that stops padding returns `[]`, which
        // since G2.4 is also the engine's answer, so it would pass silently.
        Evidence::Site(
            "crates/dss-core/src/elements/meter/monitor/mod.rs",
            &["\n        if i < 1 || i > self.record_size || self.flushed_records == 0 {"],
        ),
        Some((
            "crates/dss-core/src/elements/meter/monitor/mod.rs",
            "monitor_channel_of_an_unflushed_stream_is_empty",
        )),
    ),
    // G2.5, row 1 of 3. `TGICTransformerObj.RecalcElementData` derives winding
    // 2's conductance from `FPctR1` — the `G1` line copied with the base
    // renamed and the percentage not (`.inputs/dss_capi/src/PDElements/
    // GICTransformer.pas:441`; r4133 `Version8/Source/PDElements/
    // GICTransformer.pas:495`, the same line) — so a user's `%R2` is stored,
    // read back, and never reaches the admittance, while the same procedure's
    // `else` arm inverts `FPctR2` out of `G2` (`:446` / r4133 `:498`). Both lanes now
    // read `%R2`. This is a `WholeCase` row because the two gated decks that
    // see it are `type=Auto`, where the `BusX` side effect puts the `G1` and
    // `G2` blocks in series: the element admittance, the assembled Y and every
    // downstream node voltage move away from *both* oracles at once.
    (
        "GIC_TRANSFORMER_G2_SCALES_OFF_PCT_R1",
        Kind::WholeCase,
        // One key per (case, channel); the other three
        // (`…-gictransformer-r4133`, `…-midi-capi`, `…-midi-r4133`, plus the
        // two exact-pair property entries) live beside it in the same file and
        // are held by the ledger's own fail-on-stale accounting. The row names
        // the deck-and-channel pair whose loss is largest.
        Evidence::Ledger("gic-pct-r2-honoured-gictransformer-capi"),
        Some((
            "crates/dss-core/src/exec/tests/compat_quirks.rs",
            "gic_transformer_pct_r2_drives_winding_two",
        )),
    ),
    // G2.5, row 2 of 3. `TCapacitorObj.MakePosSequence`'s `CMatrix` arm computes
    // the positive-sequence `Cs - Cm` and then loses it: dss_capi 0.14.5 aims
    // the scalar `SetDouble` at the array property `Cuf`
    // (`.inputs/dss_capi/src/PDElements/Capacitor.pas:814` +
    // `src/General/DSSObjectHelper.pas:2812-2834`, three scalar arms and no
    // `else`) while still running the `SpecType := 2` side effect, so the bank
    // computes from stale `FC` and the user's `cmatrix` is switched out of
    // `MakeYprimWork` for good; r4133 formats the same value into a command
    // string (`Version8/Source/PDElements/Capacitor.pas:829`) but re-applies the
    // `1.0e-6` property scale (`:411`) to an already-farad value. Both lanes now
    // perform the array write in µF.
    (
        "MAKEPOSSEQ_CUF_LOST_ON_THE_SCALAR_SETTER",
        Kind::WholeCase,
        Evidence::Ledger("makeposseq-cuf-applied-capi"),
        Some((
            "crates/dss-core/src/elements/pd/capacitor/tests.rs",
            "make_pos_sequence_cmatrix_applies_the_positive_sequence_cuf",
        )),
    ),
    // G2.5, row 3 of 3. The memory-mapped LoadShape text reader filters each
    // column through an accept-set of bytes in `[46, 58)`
    // (`.inputs/dss_capi/src/General/LoadShape.pas:1374`; r4133
    // `Version8/Source/Common/Utilities.pas:834`), deleting the sign, the `+`,
    // the exponent letter and whitespace while keeping `/` inside the number —
    // so the class's two readers for the *same* file disagree. Both lanes now
    // take the column verbatim through the same aux parser the non-mapped twin
    // uses. `shape_mmf.dss` was written to observe the quirk, so it is the deck
    // that pays; its unrelated sng/dbl/`mult=(sngfile=)` MMF-reader coverage
    // moved to the sibling `shape_mmf_io.dss`, which gates clean.
    (
        "MMF_TEXT_ACCEPT_SET_DROPS_SIGN_AND_EXPONENT",
        Kind::WholeCase,
        Evidence::Ledger("mmf-accept-set-honoured-capi"),
        Some((
            "crates/dss-core/src/elements/general/load_shape/tests.rs",
            "mmf_text_reader_agrees_with_its_non_mapped_twin",
        )),
    ),
    // G2.6, and the only row this WP took out of the *rendering* seam rather
    // than out of a bug bullet — because it was never a rendering convention.
    // dss_capi's `SetMaxDeviceNameLength` zeroes the unit variable
    // (`.inputs/dss_capi/src/Common/ShowResults.pas:116`, declared `:82`) and
    // then accumulates the maximum inside `with DSS.ActiveCircuit do`
    // (`:117-121`), where the identifier resolves to the shadowing `TDSSCircuit`
    // field (`src/Common/Circuit.pas:100`, initialized to 30 at `:379`). The
    // writers read the unit variable, so the device-name column of every `Show`
    // table is formatted against **0** — the `Pad(…, width + 2)` cannot fire,
    // since two characters is below every `EncloseQuotes(name)`. r4133 does not
    // share it: `MaxDeviceNameLength` is a unit variable only there
    // (`Version8/Source/Common/ShowResults.pas:66`), `TDSSCircuit` declares no
    // such field, and the same loop (`:79-90`) leaves the honest width behind;
    // its `WriteTerminalPowerSeq` also writes the terminal as `j:3` (`:1160`)
    // rather than `IntToStr(j)`, so it could not glue even at width 0. Both
    // lanes now size the column from its own content.
    //
    // One tokenizable consequence, measured rather than assumed: only the
    // `IntToStr` site (`ShowResults.pas:1375`) glues, so only the three
    // `show_busflow*` goldens are affected — every other consumer pads with
    // spaces or `PadDots` runs, which `harness::split_fields` drops. Their
    // oracle text is de-glued unconditionally instead of being re-baselined, so
    // no golden byte moves.
    (
        "max_device_name_length",
        Kind::SplitAlias,
        // The engine call site the *goldens* observe — `Show BusFlow`'s writer,
        // the one place the width is tokenizable. The needle is its whole
        // statement at its own four spaces; the split form spelled the same line
        // `crate::compat::max_device_name_length(super::device_name_width(…))`,
        // so it cannot match a revert.
        //
        // Per the last paragraph of [`Evidence::Site`]: the six sibling writers
        // (`currents`, `losses`, `overloads`, `powers`, `elements`, `delta_v`)
        // are deliberately not listed, and that is not the "half the teardown"
        // hole the doc warns about. The alias itself is *deleted*, so a call
        // site cannot quietly re-route through it — restoring one means
        // restoring the split, which the census tie, the ghost check and the
        // pin-walk all fail on. What those six could still lose is padding
        // width, which no oracle-compared token can see (space pads and
        // `PadDots` runs are dropped by `harness::split_fields`) — it is
        // unobservable by construction, here as it was before the teardown.
        Evidence::Site(
            "crates/dss-core/src/report/show/bus_powers.rs",
            &["\n    let mdnl = super::device_name_width(classes, ckt);"],
        ),
        Some((
            "crates/dss-core/src/exec/tests/compat_quirks.rs",
            "device_name_column_is_sized_from_its_content",
        )),
    ),
];

/// One row of [`TORN_DOWN_ROWS`]: `(former row name, which census it left,
/// evidence in the tree, `Some((pin file, pin fn))`)`.
type TornDownRow = (
    &'static str,
    Kind,
    Evidence,
    Option<(&'static str, &'static str)>,
);

/// [`SPLIT_ALIAS_POPULATION`] at the WP-G2 start line, the fixed point the
/// register's arithmetic tie is anchored on (`4f977d9e`'s 31).
const SPLIT_ALIASES_AT_WP_G2_START: usize = 31;

/// The [`Escape::WholeCase`] bucket at the same start line.
const WHOLE_CASE_MARKERS_AT_WP_G2_START: usize = 4;

/// The comment marker that names a lane exclusion made unconditional by a
/// teardown, assembled at runtime so this file does not carry a literal
/// occurrence of its own needle (the [`compat_tag`] trick). Spelled out for
/// humans in `TESTING.md` §"Precision-compat rows still split by lane".
fn exclusion_marker() -> String {
    format!("LANE-EXCLUSIO{}(", "N")
}

/// The comment marker that names a torn-down row's expected-value pin. Same
/// runtime assembly, same reason.
fn pin_marker() -> String {
    format!("EXPECTED-VALUE-PI{}(", "N")
}

/// One teardown marker: `(marker, row, file, line)`.
type TeardownMarker = (String, String, String, usize);

/// Every teardown marker in the tree, plus the malformed occurrences.
///
/// Shape — `// <marker>(<row>): <why>` — is the [`markers_in_tree`] discipline
/// applied to the second kind of comment this plan introduces: the tag marks a
/// site that *reproduces* an upstream inexactness, these mark the sites where
/// one stopped. Malformed occurrences are returned rather than ignored, so a
/// typo cannot silently drop a site out of the register.
fn teardown_markers(root: &Path) -> (Vec<TeardownMarker>, Vec<String>) {
    let markers = [exclusion_marker(), pin_marker()];
    let mut out = Vec::new();
    let mut malformed = Vec::new();

    for path in rust_sources(root) {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        if !markers.iter().any(|m| text.contains(m)) {
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        for (i, line) in text.lines().enumerate() {
            for marker in &markers {
                for (col, _) in line.match_indices(marker.as_str()) {
                    let before = &line[..col];
                    let after = &line[col + marker.len()..];
                    let row = after.chars().take_while(|c| *c != ')').collect::<String>();
                    let closed = after[row.len()..].starts_with("):");
                    if !before.contains("//") || row.trim().is_empty() || !closed {
                        malformed.push(format!("    {rel}:{}: {}", i + 1, line.trim()));
                        continue;
                    }
                    out.push((
                        marker.trim_end_matches('(').to_string(),
                        row.trim().to_string(),
                        rel.clone(),
                        i + 1,
                    ));
                }
            }
        }
    }
    (out, malformed)
}

/// The brace-balanced block starting at the `{` at byte `open`.
///
/// A five-state scanner rather than a brace count, because the bodies it is
/// pointed at are *test* bodies: they hold deck text in raw strings, `assert!`
/// messages with `{}` placeholders, and commented-out code, any of which
/// unbalances a naive count and would silently hand back the rest of the file.
fn balanced_block(text: &str, open: usize) -> Option<&str> {
    let b = text.as_bytes();
    let ident = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    let mut i = open;
    let mut depth = 0usize;

    while i < b.len() {
        match b[i] {
            b'{' => {
                depth += 1;
                i += 1;
            }
            b'}' => {
                depth = depth.checked_sub(1)?;
                i += 1;
                if depth == 0 {
                    return Some(&text[open..i]);
                }
            }
            b'/' if b.get(i + 1) == Some(&b'/') => {
                i = text[i..].find('\n').map_or(b.len(), |n| i + n + 1);
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                i = text[i + 2..].find("*/").map_or(b.len(), |n| i + n + 4);
            }
            // A raw string: `r"…"`, `r#"…"#`, `br##"…"##`, … The `r` must not
            // be the tail of an identifier, or `for"` would start one.
            b'r' | b'b'
                if !i
                    .checked_sub(1)
                    .and_then(|p| b.get(p))
                    .copied()
                    .is_some_and(ident) =>
            {
                let mut j = i;
                if b[j] == b'b' {
                    j += 1;
                }
                if b.get(j) != Some(&b'r') {
                    i += 1;
                    continue;
                }
                j += 1;
                let hashes = b[j..].iter().take_while(|c| **c == b'#').count();
                if b.get(j + hashes) != Some(&b'"') {
                    i += 1;
                    continue;
                }
                let close = format!("\"{}", "#".repeat(hashes));
                let from = j + hashes + 1;
                i = text[from..]
                    .find(&close)
                    .map_or(b.len(), |n| from + n + close.len());
            }
            b'"' => {
                i += 1;
                while i < b.len() && b[i] != b'"' {
                    i += if b[i] == b'\\' { 2 } else { 1 };
                }
                i += 1;
            }
            // A char literal — but `'a` in `&'a str` is a lifetime, and eating
            // to the next quote there would swallow the body.
            b'\'' if b.get(i + 1) == Some(&b'\\') || b.get(i + 2) == Some(&b'\'') => {
                i += 1;
                while i < b.len() && b[i] != b'\'' {
                    i += if b[i] == b'\\' { 2 } else { 1 };
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    None
}

/// The spelling by which `body` still branches on the lane, if it does.
///
/// Three, and the third is why this is a function: the engine constant
/// `ORACLE_PARITY`, the cfg itself, and the **harness** constant
/// `harness::lane::PARITY` (`lane.rs:78`, `cfg!(feature = …)`), which is how the
/// integration tests that hold most of the exclusion-flavoured pins —
/// `golden_reports.rs` above all — read the lane. A check that knew only the
/// first would wave through exactly the pins WP-G2's largest sub-steps produce.
fn reads_the_lane(body: &str) -> Option<&'static str> {
    if names_token(body, "ORACLE_PARITY") {
        return Some("ORACLE_PARITY");
    }
    if names_token(body, "PARITY") {
        return Some("lane::PARITY");
    }
    body.contains(&needle()).then_some("the lane cfg")
}

/// The body of the `#[test] fn <func>` **declaration** inside `region`.
///
/// `None` when the region merely *mentions* the name. That distinction is the
/// point: a bare-token match is satisfied by a leftover `// superseded by
/// <func>` comment sitting where the test used to be, so the register would go
/// on reporting a pin that no longer runs. The attribute is required the same
/// way [`test_region`] requires one — an unattributed helper asserts nothing on
/// its own.
fn test_fn_body<'a>(region: &'a str, func: &str) -> Option<&'a str> {
    let ident = |c: char| c.is_alphanumeric() || c == '_';
    let mut from = 0usize;

    while let Some(rel) = region[from..].find("fn ") {
        let at = from + rel;
        from = at + 3;
        let name: String = region[at + 3..].chars().take_while(|c| ident(*c)).collect();
        if name != func {
            continue;
        }
        let head = &region[..at];
        // The nearest `#[test]` above must belong to *this* fn: no other `fn`
        // may sit between the two.
        let Some(attr) = head.rfind("#[test]") else {
            continue;
        };
        if head[attr..].contains("fn ") {
            continue;
        }
        let sig = at + 3 + name.len();
        let open = sig + region[sig..].find('{')?;
        return balanced_block(region, open);
    }
    None
}

/// Every torn-down row keeps its pin, its evidence, and its census arithmetic.
///
/// Six independent ways the record could rot, all checked:
///
/// 1. **The pin is gone.** A teardown's contract is that the row's
///    expected-value test survives it — the named test must still be *declared*
///    ([`test_fn_body`], not a bare mention), in a real test region, in a file
///    that could pin anything ([`is_pin_candidate`]: not a compat module's
///    kernel-vs-kernel test, not this bookkeeping file).
/// 2. **The pin is still conditional.** The contract is not "a test with that
///    name exists" but "it became *unconditional*" — one expected value
///    asserted in both lanes. A pin that still reads the lane in any of
///    [`reads_the_lane`]'s three spellings can assert nothing on one side, and
///    once the row leaves the census
///    [`every_lane_split_alias_is_pinned_by_an_expected_value_test`] stops
///    looking at it, so nothing else would notice. Checked on the pin's own
///    body, not its file: files like `exec/tests/compat_quirks.rs` legitimately
///    hold *still-split* rows' lane-branching pins next door.
/// 3. **The mechanism is gone.** An [`Evidence::Site`]/[`Evidence::Exclusion`]
///    slice that no longer matches means the unconditional kernel or exclusion
///    was reverted or reworded; an [`Evidence::Ledger`] id that no longer
///    exists means the divergence the fix opened is no longer pinned at all;
///    [`Evidence::None`] is refused outright until WP-G4.
/// 4. **A census moved without a row.** The two ties are the reason a row
///    cannot be skipped: dropping [`SPLIT_ALIAS_POPULATION`] by one without
///    adding a `SplitAlias` row fails here, and so does adding a row without
///    dropping the number.
/// 5. **A row that never left.** The ties are *counts*, so deleting alias A's
///    split while registering a still-split row B balances them. The names are
///    therefore checked against the live split set too.
/// 6. **Two rows with one name**, which would make the marker check below
///    ambiguous.
#[test]
fn every_torn_down_row_keeps_its_pin_and_its_evidence() {
    let root = repo_root();

    let mut names: Vec<&str> = TORN_DOWN_ROWS.iter().map(|(n, _, _, _)| *n).collect();
    names.sort_unstable();
    let before = names.len();
    names.dedup();
    assert_eq!(
        names.len(),
        before,
        "duplicate row name(s) in the teardown register: {names:?}"
    );
    assert!(
        TORN_DOWN_ROWS
            .iter()
            .all(|(n, _, _, _)| !n.trim().is_empty()),
        "a teardown row needs the name the marker comments refer to"
    );

    let mut problems: Vec<String> = Vec::new();

    for (name, _kind, evidence, pin) in TORN_DOWN_ROWS {
        match pin {
            None => problems.push(format!(
                "    {name}: no pin. Every WP-G2 teardown row records the \
                 expected-value test that became unconditional; a pinless row \
                 means the coverage evaporated with the alias"
            )),
            Some((file, func)) => {
                let path = root.join(file);
                match fs::read_to_string(&path) {
                    Err(e) => problems.push(format!("    {name}: pin file {file}: {e}")),
                    Ok(text) => match is_pin_candidate(&path, &root, &text) {
                        None => problems.push(format!(
                            "    {name}: {file} has no test region — it cannot hold a pin"
                        )),
                        Some((start, end)) => match test_fn_body(&text[start..end], func) {
                            None => problems.push(format!(
                                "    {name}: {file} declares no `#[test] fn {func}` inside \
                                 its test region — the pin was renamed, deleted, or is only \
                                 mentioned in a comment"
                            )),
                            Some(body) => {
                                if let Some(how) = reads_the_lane(body) {
                                    problems.push(format!(
                                        "    {name}: `{func}` in {file} still branches on the \
                                         lane (`{how}`). A teardown makes the pin \
                                         *unconditional* — one expected value asserted in both \
                                         lanes. Once the row left the census nothing else looks \
                                         at this test, so a parity arm that asserts nothing \
                                         would be invisible"
                                    ));
                                }
                            }
                        },
                    },
                }
            }
        }

        match evidence {
            Evidence::None => problems.push(format!(
                "    {name}: no evidence. Every WP-G2 teardown leaves a mechanism behind — \
                 the kernel or exclusion it made unconditional (`Evidence::Site` / \
                 `Evidence::Exclusion`) or the ledger entry pinning the divergence it \
                 opened (`Evidence::Ledger`). `Evidence::None` is reserved for WP-G4's \
                 rendering rows; the first of those lands by editing this arm"
            )),
            Evidence::Site(file, slices) | Evidence::Exclusion(file, slices) => {
                if slices.is_empty() {
                    problems.push(format!(
                        "    {name}: an evidence site with no slices proves nothing — every \
                         unconditional site the teardown left behind gets its own anchor"
                    ));
                }
                match fs::read_to_string(root.join(file)) {
                    Err(e) => problems.push(format!("    {name}: evidence file {file}: {e}")),
                    Ok(text) => {
                        for slice in *slices {
                            if !text.contains(*slice) {
                                problems.push(format!(
                                    "    {name}: {file} no longer contains {slice:?} — the \
                                     unconditional site this teardown left behind is gone"
                                ));
                            }
                        }
                    }
                }
            }
            Evidence::Ledger(key) => {
                let ledger = root.join("tests").join("corpus").join("ledger.json");
                let text = fs::read_to_string(&ledger).expect("read ledger.json");
                let value: serde_json::Value =
                    serde_json::from_str(&text).expect("parse ledger.json");
                let present = value["entries"]
                    .as_array()
                    .expect("ledger.json has an `entries` array")
                    .iter()
                    .any(|e| e["id"].as_str() == Some(*key));
                if !present {
                    problems.push(format!(
                        "    {name}: no `{key}` entry in tests/corpus/ledger.json — the \
                         divergence this fix opened is unpinned"
                    ));
                }
            }
        }
    }

    assert!(
        problems.is_empty(),
        "teardown register row(s) whose record no longer describes the tree:\n{}",
        problems.join("\n")
    );

    // The two arithmetic ties. Stated as subtractions from the WP-G2 start line
    // so that both halves of a teardown — the decrement and the row — have to
    // land in the same commit.
    let split_down = TORN_DOWN_ROWS
        .iter()
        .filter(|(_, k, _, _)| *k == Kind::SplitAlias)
        .count();
    let remaining = SPLIT_ALIASES_AT_WP_G2_START
        .checked_sub(split_down)
        .unwrap_or_else(|| {
            panic!(
                "{split_down} torn-down split aliases against a start line of \
                 {SPLIT_ALIASES_AT_WP_G2_START} — the register outgrew the census it decrements"
            )
        });
    assert_eq!(
        remaining, SPLIT_ALIAS_POPULATION,
        "the split-alias census and the teardown register disagree: \
         {SPLIT_ALIASES_AT_WP_G2_START} − {split_down} torn down ≠ {SPLIT_ALIAS_POPULATION}. \
         A row leaves the split only by landing here with its pin"
    );

    // …and the tie is a *count*, which a delete-one/register-another swap
    // satisfies. The names have to be gone from the live split set too.
    let still_split: Vec<String> = lane_aliases(&root)
        .into_iter()
        .filter(|(_, impls)| impls.len() == 2 && impls[0] != impls[1])
        .map(|(alias, _)| alias)
        .collect();
    let ghosts: Vec<&str> = TORN_DOWN_ROWS
        .iter()
        .filter(|(_, kind, _, _)| *kind == Kind::SplitAlias)
        .map(|(name, _, _, _)| *name)
        .filter(|name| still_split.iter().any(|a| a == name))
        .collect();
    assert!(
        ghosts.is_empty(),
        "teardown register row(s) whose alias still selects two different impls: \
         {ghosts:?}. The census arithmetic balances, so some *other* row's split was \
         deleted instead — the register would then credit the teardown to the wrong \
         row and leave a live split unpinned"
    );

    let whole_case_down = TORN_DOWN_ROWS
        .iter()
        .filter(|(_, k, _, _)| *k == Kind::WholeCase)
        .count();
    let whole_case_now = EXIT_POPULATION
        .iter()
        .find(|(owner, _)| *owner == Escape::WholeCase)
        .map(|(_, n)| *n)
        .expect("EXIT_POPULATION carries a WholeCase bucket");
    let remaining = WHOLE_CASE_MARKERS_AT_WP_G2_START
        .checked_sub(whole_case_down)
        .unwrap_or_else(|| {
            panic!(
                "{whole_case_down} torn-down WholeCase rows against a start line of \
                 {WHOLE_CASE_MARKERS_AT_WP_G2_START}"
            )
        });
    assert_eq!(
        remaining, whole_case_now,
        "the `Escape::WholeCase` bucket and the teardown register disagree: \
         {WHOLE_CASE_MARKERS_AT_WP_G2_START} − {whole_case_down} torn down ≠ {whole_case_now}"
    );
}

/// The teardown markers in the tree and the register name the same rows.
///
/// Checked **both ways**, exactly like
/// [`surviving_compat_markers_are_exactly_the_recorded_escape_register`]:
///
/// * marker → row: a marker naming a row the register does not carry is a
///   claim about a teardown that never happened (with the register empty, as it
///   is at G2.0, *every* marker fails — which is the point: the convention is
///   live before the first row lands);
/// * row → marker: a row's recorded pin file must carry the pin marker naming
///   it, so the grep `<pin marker><row>` finds the test the register promises;
///   and a row whose evidence is an [`Evidence::Exclusion`] must likewise carry
///   the exclusion marker in that file. A *blanket* exclusion-marker obligation
///   would be wrong — a row whose fix touched no harness has no exclusion — so
///   the row declares which of its evidence is one, and this is where the
///   declaration is cashed.
#[test]
fn teardown_markers_and_the_register_agree() {
    let root = repo_root();
    let (markers, malformed) = teardown_markers(&root);

    // Non-vacuity of the *walk*. With the register empty (its state through
    // G2.0) every assert below passes on zero data, so a `rust_sources`
    // regression that returned nothing would look exactly like a clean tree.
    assert!(
        rust_sources(&root).len() > 300,
        "the teardown-marker walk reached only {} `.rs` files — it is supposed to \
         cover the whole repository, and an empty walk makes every check below \
         vacuous",
        rust_sources(&root).len()
    );

    let (exclusion, pin) = (exclusion_marker(), pin_marker());
    let (ex_name, pin_name) = (
        exclusion.trim_end_matches('('),
        pin.trim_end_matches('(').to_string(),
    );

    assert!(
        malformed.is_empty(),
        "teardown marker(s) not in the documented shape `// {ex_name}(<row>): <why>` \
         (or the pin spelling) — a marker outside a comment, with an empty row name, or \
         without the closing `):` is invisible to the grep it exists for:\n{}",
        malformed.join("\n")
    );

    let unregistered: Vec<String> = markers
        .iter()
        .filter(|(_, row, _, _)| {
            !TORN_DOWN_ROWS
                .iter()
                .any(|(name, _, _, _)| *name == row.as_str())
        })
        .map(|(marker, row, file, line)| format!("    {file}:{line}: {marker}({row})"))
        .collect();
    assert!(
        unregistered.is_empty(),
        "teardown marker(s) naming a row with no entry in `TORN_DOWN_ROWS`. The marker \
         claims a lane row was dismantled here; the register is what proves it, so add \
         the row (with its pin and its census decrement) or drop the marker:\n{}",
        unregistered.join("\n")
    );

    let marked = |marker: &str, row: &str, file: &str| {
        markers
            .iter()
            .any(|(m, r, at, _)| m.as_str() == marker && r.as_str() == row && at.as_str() == file)
    };

    let missing: Vec<String> = TORN_DOWN_ROWS
        .iter()
        .filter_map(|(name, _, _, pin_at)| {
            let (file, func) = (*pin_at)?;
            (!marked(&pin_name, name, file)).then(|| format!("    {name}: {file} (pin `{func}`)"))
        })
        .collect();
    assert!(
        missing.is_empty(),
        "torn-down row(s) whose pin file carries no `{pin_name}` marker naming them — \
         the register knows where the pin is, but a reader grepping the tree does \
         not:\n{}",
        missing.join("\n")
    );

    let unmarked: Vec<String> = TORN_DOWN_ROWS
        .iter()
        .filter_map(|(name, _, evidence, _)| match evidence {
            Evidence::Exclusion(file, _) if !marked(ex_name, name, file) => {
                Some(format!("    {name}: {file}"))
            }
            _ => None,
        })
        .collect();
    assert!(
        unmarked.is_empty(),
        "torn-down row(s) whose exclusion file carries no `{ex_name}` marker naming \
         them — the register says the harness exclusion at that file became \
         unconditional for this row, so the site has to say so too:\n{}",
        unmarked.join("\n")
    );
}

/// The **sub-step** marker spellings: the same two words as [`pin_marker`] /
/// [`exclusion_marker`] but in the *loose* form — a space where the reserved one
/// has a hyphen, and `[row]` where it has `(row):`. Assembled at runtime like
/// the reserved pair, and for the same reason (this file must not carry a
/// literal occurrence of a needle it greps the tree for).
///
/// A sub-step that pins an expected value or drops a compare is not a WP-G2
/// teardown row, and registering it in [`TORN_DOWN_ROWS`] would be a false claim
/// about a lane row that never existed. RP3.8 therefore re-spelled its markers
/// one character away from the reserved ones — correct, but it created a second
/// vocabulary that nothing checked in either direction (RP3.8 audit-code finding
/// 2). [`substep_markers_are_tagged_and_do_not_shadow_the_register`] is that
/// check.
fn substep_marker_spellings() -> [String; 2] {
    [
        format!("EXPECTED-VALUE PI{} [", "N"),
        format!("LANE EXCLUSIO{} [", "N"),
    ]
}

/// Every sub-step marker in the tree: `(marker, row, file, line, line text)`.
fn substep_markers(root: &Path) -> Vec<(String, String, String, usize, String)> {
    let markers = substep_marker_spellings();
    let mut out = Vec::new();
    for path in rust_sources(root) {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        if !markers.iter().any(|m| text.contains(m)) {
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        for (i, line) in text.lines().enumerate() {
            for marker in &markers {
                for (col, _) in line.match_indices(marker.as_str()) {
                    let after = &line[col + marker.len()..];
                    let row = after.chars().take_while(|c| *c != ']').collect::<String>();
                    out.push((
                        marker.trim_end_matches(" [").to_string(),
                        row.trim().to_string(),
                        rel.clone(),
                        i + 1,
                        line.trim().to_string(),
                    ));
                }
            }
        }
    }
    out
}

/// The sub-step marker vocabulary is disciplined, and cannot shadow the
/// teardown register (RP3.8 audit settlement).
///
/// Three obligations, none of which existed when the spelling was introduced:
///
/// * **self-identifying** — the marker line must carry the sub-step tag
///   (`RP<n>.<n>`) that owns it, so the loose spelling always reads as sub-step
///   paperwork and never as a teardown claim;
/// * **not a teardown row** — a row name registered in [`TORN_DOWN_ROWS`] must
///   use the *reserved* spelling, which
///   [`teardown_markers_and_the_register_agree`] cross-checks against the
///   register. A registered row wearing the loose spelling would escape that
///   check silently — the copy-paste hazard this test closes;
/// * **the pin exists** — a loose *pin* marker must sit above a real `#[test]`,
///   so the marker names something a reader can run.
///
/// Non-vacuous by construction: the tree carries markers today, and the count is
/// asserted positive.
#[test]
fn substep_markers_are_tagged_and_do_not_shadow_the_register() {
    let root = repo_root();
    let markers = substep_markers(&root);
    assert!(
        !markers.is_empty(),
        "no sub-step markers found — either the walk broke or the spelling moved, \
         and every check below just went vacuous"
    );

    let tagged = |line: &str| {
        // `RP3.8`, `RP4.1`, … immediately identifying the owner.
        line.split_whitespace().any(|w| {
            let w = w.trim_start_matches("//").trim();
            w.len() >= 4
                && w.starts_with("RP")
                && w[2..].split('.').count() == 2
                && w[2..].chars().all(|c| c.is_ascii_digit() || c == '.')
        })
    };
    let untagged: Vec<String> = markers
        .iter()
        .filter(|(_, _, _, _, line)| !tagged(line))
        .map(|(m, row, file, at, line)| format!("    {file}:{at}: {m} [{row}] — {line}"))
        .collect();
    assert!(
        untagged.is_empty(),
        "sub-step marker(s) with no owning sub-step tag (`RP<n>.<n>`) on the line. \
         The reserved teardown spellings are one character away and are checked \
         against `TORN_DOWN_ROWS`; an untagged loose marker is indistinguishable \
         from a mis-typed teardown claim:\n{}",
        untagged.join("\n")
    );

    let shadowing: Vec<String> = markers
        .iter()
        .filter(|(_, row, _, _, _)| {
            TORN_DOWN_ROWS
                .iter()
                .any(|(name, _, _, _)| name.eq_ignore_ascii_case(row))
        })
        .map(|(m, row, file, at, _)| format!("    {file}:{at}: {m} [{row}]"))
        .collect();
    assert!(
        shadowing.is_empty(),
        "sub-step marker(s) naming a row that IS in `TORN_DOWN_ROWS`. A teardown \
         row must wear the reserved spelling, which the register cross-checks; \
         the loose one escapes that check:\n{}",
        shadowing.join("\n")
    );

    // marker -> test: an expected-value pin names a test that exists right below.
    let pin = substep_marker_spellings()[0]
        .trim_end_matches(" [")
        .to_string();
    let mut orphaned: Vec<String> = Vec::new();
    for (marker, row, file, at, _) in &markers {
        if *marker != pin {
            continue;
        }
        let text = fs::read_to_string(root.join(file)).expect("marker file is readable");
        let has_test = text
            .lines()
            .skip(*at) // the marker's own line is 1-based `at`
            .take(40)
            .any(|l| l.trim_start().starts_with("#[test]"));
        if !has_test {
            orphaned.push(format!("    {file}:{at}: {marker} [{row}]"));
        }
    }
    assert!(
        orphaned.is_empty(),
        "expected-value pin marker(s) with no `#[test]` within the 40 lines below \
         them — the marker promises a pin a reader can run:\n{}",
        orphaned.join("\n")
    );
}

// ---------------------------------------------------------------------------
// The citation surface: docs that point *into* the compat machinery
// ---------------------------------------------------------------------------

/// The **operational** documentation surface — files that describe what the
/// tree does *right now*, as opposed to what someone intended or what was once
/// true.
///
/// Why this exists: F.3 flipped 30 rows and resolved ~100 markers, and every
/// flip silently falsified whatever outside `crates/` had cited the old site.
/// Five had: `tests/TOLERANCE_NOTES.md` still sent a tolerance author looking
/// for a marker in `seq_currents.rs` and named the per-terminal slice as the
/// *unfinished* clean fix; `tests/corpus/modes/manifest.json` told a triager
/// that a marker in `exec/view.rs` was "what actually verifies Newton dispatch
/// is wired", after F.3j had made that read lane-split and moved the
/// verification to an in-engine tripwire; three golden generators described
/// captures whose meaning had become lane-dependent. Nothing objected, because
/// every Stage F gate before this one walks `.rs` files only — F.3ac's finding
/// (the instrument scoped narrower than the claim) in the last place it could
/// still hide.
///
/// **Deliberately excluded: plans and records.** `DE_PASCALIZE_PLAN.md`,
/// `PORTING_PLAN.md`, `STATUS.md` and everything under `docs/` state intent or
/// history; they are *allowed* to differ from HEAD, and mechanically demanding
/// otherwise would both burden every historical record and invite editing the
/// log to please a test. The one plan row whose content this stage disproved
/// was corrected by hand in F.3ae, which is the right shape for that class.
fn operational_docs(root: &Path) -> Vec<PathBuf> {
    // Named individually, because each is a specific promise about the current
    // tree. A missing one is a failure, not a skip.
    let fixed = [
        "CLAUDE.md",
        "TESTING.md",
        "tests/TOLERANCE_NOTES.md",
        "tests/corpus/ledger.json",
        // Ours, and inside the subtree whose walk filter (below) exists to keep
        // upstream's vendored `.md`/`.py` out: the r4133 property-census
        // evidence README (R4133_PROPS_PLAN.md RP0.1). Named individually
        // because the filter's reasoning — "not this project's documentation" —
        // is exactly false for it.
        "tests/corpus/props_r4133/README.md",
    ];
    let mut out: Vec<PathBuf> = Vec::new();
    for rel in fixed {
        let path = root.join(rel);
        assert!(
            path.is_file(),
            "{rel} is gone — it is part of the operational doc surface this \
             gate checks; if it moved, update this list"
        );
        out.push(path);
    }

    // Plus two walked subtrees, each with its OWN filter — the filter is not
    // cosmetic. `tests/corpus` contains the **vendored** `electricdss-tst`
    // checkout, 33 files of upstream's own `.py`/`.md`; those are not this
    // project's documentation, they make no claim about our tree, and gating on
    // them would fail the suite on a routine re-vendor. Only the manifests we
    // write are in scope there — plus the `props_r4133/README.md` named above,
    // which the extension filter would otherwise drop although it is ours.
    // Under `tools/` the reverse holds: the `.py`
    // generators and their `README`s are ours and are exactly where a capture's
    // meaning is explained.
    //
    // `SKIP_DIRS` keeps the walk out of the `.venv` and `.inputs` **junctions**
    // — following one reads main's checkout, and `tools/opendss/.venv` is
    // exactly such a link (CLAUDE.md, "Git worktrees").
    let roots: [(PathBuf, &[&str]); 2] = [
        (root.join("tests").join("corpus"), &["json"]),
        (root.join("tools"), &["md", "py"]),
    ];
    for (subtree, exts) in roots {
        let mut dirs = vec![subtree];
        while let Some(dir) = dirs.pop() {
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_lowercase();
                if path.is_dir() {
                    // `models` is the wasm reference user model: real Rust,
                    // already walked by `rust_sources`. `bin` holds the vendored
                    // r4133 binary; `__pycache__` is build output.
                    let skip = SKIP_DIRS.contains(&name.as_str())
                        || matches!(name.as_str(), "models" | "bin" | "__pycache__");
                    if !skip {
                        dirs.push(path);
                    }
                    continue;
                }
                let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase());
                let Some(ext) = ext else { continue };
                if !exts.contains(&ext.as_str()) {
                    continue;
                }
                // JSON is in scope only for the manifests we author.
                if ext == "json" && name != "manifest.json" {
                    continue;
                }
                out.push(path);
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Whether the file a documentation line points at still carries a compat
/// marker.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Cited {
    /// The named file must still carry a marker — the doc describes a live
    /// reproduction site.
    Present,
    /// The named file must **not** carry one — the doc records that the marker
    /// was resolved. Checked the same fail-on-stale way, so the sentence cannot
    /// quietly become false in the other direction either.
    Absent,
}

/// Every line in the operational surface that spells the compat tag *and* names
/// a Rust file: every documented claim about where a marker lives.
///
/// Keyed like [`ESCAPE_REGISTER`]: `(doc file, distinctive slice of the line,
/// the source file it names, what must be true of that file)`.
const TAG_PATH_CITATIONS: &[(&str, &str, &str, Cited)] = &[
    // The NCIM cause entry records a marker that WP-U2 removed when the port
    // dropped capi015's one-conductor VSource offset. `exec/view.rs` has been
    // marker-free ever since — F.3j routed the Newton read through a lane alias
    // rather than re-opening a marker there, and GOLDEN_REBASE G2.3 deleted that
    // alias too, leaving the file with one unconditional `refresh_iterminal`.
    (
        "tests/corpus/ledger.json",
        "was removed",
        "crates/dss-core/src/exec/view.rs",
        Cited::Absent,
    ),
    // The wasm reference user model's three surviving markers — `Escape::
    // WasmGuest` in the register above. Its README is the only doc that points
    // a reader at them, so these two rows are what keeps that pointer honest if
    // `WASM_USERMODELS_PLAN` ever resolves them.
    (
        "tools/wasm_usermodel/README.md",
        "0.866025403",
        "tools/wasm_usermodel/models/indmach012a/src/symcomp.rs",
        Cited::Present,
    ),
    (
        "tools/wasm_usermodel/README.md",
        "1.732",
        "tools/wasm_usermodel/models/indmach012a/src/model.rs",
        Cited::Present,
    ),
];

/// The Rust files a documentation line names, in any spelling docs actually
/// use: a full `crates/dss-core/src/util.rs`, a partial `exec/view.rs`, or a
/// **bare** `seq_currents.rs`.
///
/// The bare form is not an edge case — it is the one the real staleness took
/// (a compat-tagged heading naming `SeqCurrents`, then "(`seq_currents.rs`)"),
/// and a first cut of this helper required a `/` and therefore did not catch
/// the very citation it was written for. The probe caught that, and it is the
/// same lesson as F.3ac/F.3ad: the instrument, not the tree, was the narrow
/// part.
fn rust_paths_in(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (i, _) in line.match_indices(".rs") {
        let head = &line[..i];
        let start = head
            .rfind(|c: char| !(c.is_alphanumeric() || c == '_' || c == '/' || c == '.'))
            .map_or(0, |p| p + 1);
        let candidate = &line[start..i + ".rs".len()];
        // Reject a bare `.rs` with nothing in front of it (e.g. the literal
        // extension quoted in prose).
        if candidate.len() > ".rs".len() {
            out.push(candidate.to_string());
        }
    }
    out
}

/// Does the marker index contain `cited`, allowing the partial and bare
/// spellings [`rust_paths_in`] accepts?
fn marker_index_has(marked: &[String], cited: &str) -> bool {
    marked
        .iter()
        .any(|m| m == cited || m.ends_with(&format!("/{cited}")))
}

/// The operational docs' references into the compat machinery are accurate.
///
/// Two independent checks, because the two ways such a reference rots are
/// independent:
///
/// 1. **Marker locations.** A line that spells the tag and names a `.rs` file
///    is making a claim about that file; it must be registered above, and the
///    claim must hold. Everything it would have caught was fixed in the commit
///    that added it, so what remains is the tripwire for the next flip.
/// 2. **Alias names.** Every `compat::<ident>` a doc names must be declared by
///    a compat module. Inside `crates/` rustdoc links are compiler-checked;
///    Markdown, Python and JSON get no such help, so a rename leaves a lying
///    sentence behind. This half is populated *now* — the corrected citations
///    point at real rows — so it is load-bearing immediately, not a promise.
#[test]
fn operational_docs_cite_the_compat_machinery_accurately() {
    let root = repo_root();
    let tag = compat_tag();
    let docs = operational_docs(&root);
    assert!(
        docs.len() > 20,
        "the doc walk found only {} files — it is broken",
        docs.len()
    );

    let rel_of = |p: &Path| {
        p.strip_prefix(&root)
            .unwrap_or(p)
            .to_string_lossy()
            .replace('\\', "/")
    };
    let rels: Vec<String> = docs.iter().map(|p| rel_of(p)).collect();

    // Non-vacuity of the walk, both directions. It must REACH the surfaces the
    // corrected citations live in...
    for expected in [
        "tests/TOLERANCE_NOTES.md",
        "tests/corpus/modes/manifest.json",
        "tools/golden/gen_json.py",
    ] {
        assert!(
            rels.iter().any(|r| r == expected),
            "the doc walk no longer reaches {expected} — its compat citations \
             just stopped being checked"
        );
    }
    // ...and it must NOT reach the vendored upstream checkout, whose `.py`/`.md`
    // files are not ours to gate and would fail the suite on a re-vendor.
    let vendored: Vec<&String> = rels
        .iter()
        .filter(|r| r.contains("corpus/electricdss-tst/"))
        .collect();
    assert!(
        vendored.is_empty(),
        "the doc walk descended into the vendored corpus ({} files, e.g. {:?}) \
         — upstream's own files make no claim about this tree",
        vendored.len(),
        vendored.first()
    );

    // The files carrying a marker today, as repo-relative `/` paths.
    let marked: Vec<String> = markers_in_tree(&root)
        .into_iter()
        .map(|(rel, _)| rel)
        .collect();

    // Everything the compat modules declare (token match is enough to catch a
    // rename or a deletion, which is the failure this half exists for).
    let compat_text: String = COMPAT_MODULES
        .iter()
        .map(|m| fs::read_to_string(root.join(m)).unwrap_or_else(|e| panic!("{m}: {e}")))
        .collect::<Vec<_>>()
        .join("\n");

    let mut hits = vec![0usize; TAG_PATH_CITATIONS.len()];
    let mut unregistered = Vec::new();
    let mut wrong = Vec::new();
    let mut bad_alias = Vec::new();
    let mut alias_refs = 0usize;

    for path in &docs {
        let Ok(text) = fs::read_to_string(path) else {
            continue;
        };
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");

        for (i, line) in text.lines().enumerate() {
            // (2) alias references — every line.
            for (col, _) in line.match_indices("compat::") {
                let ident: String = line[col + "compat::".len()..]
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if ident.is_empty() {
                    continue;
                }
                alias_refs += 1;
                if !names_token(&compat_text, &ident) {
                    bad_alias.push(format!("    {rel}:{}: compat::{ident}", i + 1));
                }
            }

            // (1) marker-location claims.
            if !line.contains(&tag) {
                continue;
            }
            let named = rust_paths_in(line);
            if named.is_empty() {
                continue;
            }
            let matched: Vec<usize> = TAG_PATH_CITATIONS
                .iter()
                .enumerate()
                .filter(|(_, (doc, key, _, _))| *doc == rel && line.contains(key))
                .map(|(i, _)| i)
                .collect();
            match matched.len() {
                1 => {
                    hits[matched[0]] += 1;
                    let (_, _, src, want) = TAG_PATH_CITATIONS[matched[0]];
                    let is_marked = marker_index_has(&marked, src);
                    let ok = match want {
                        Cited::Present => is_marked,
                        Cited::Absent => !is_marked,
                    };
                    if !ok {
                        wrong.push(format!(
                            "    {rel}:{}: claims {want:?} of {src}, tree says {}",
                            i + 1,
                            if is_marked { "Present" } else { "Absent" }
                        ));
                    }
                }
                0 => unregistered.push(format!("    {rel}:{}: names {named:?}", i + 1)),
                n => panic!(
                    "{rel}:{}: matches {n} citation rows — sharpen their keys",
                    i + 1
                ),
            }
        }
    }

    assert!(
        unregistered.is_empty(),
        "documentation line(s) spelling the compat tag AND naming a Rust file, \
         with no row in `TAG_PATH_CITATIONS`. Such a line claims where a marker \
         lives, and a Stage F flip falsifies exactly that claim: either drop the \
         tag from the sentence (write \"the lane row `compat::X`\" instead) or \
         register the claim so it is checked:\n{}",
        unregistered.join("\n")
    );
    assert!(
        wrong.is_empty(),
        "documentation describing the marker population incorrectly:\n{}",
        wrong.join("\n")
    );
    assert!(
        bad_alias.is_empty(),
        "documentation naming a `compat::` alias no compat module declares. \
         Outside `crates/` nothing compiler-checks these references, so a \
         rename leaves the sentence lying:\n{}",
        bad_alias.join("\n")
    );

    let stale: Vec<String> = TAG_PATH_CITATIONS
        .iter()
        .enumerate()
        .filter(|(i, _)| hits[*i] != 1)
        .map(|(i, (doc, key, _, _))| format!("    {doc}: {key:?} — {} matches", hits[i]))
        .collect();
    assert!(
        stale.is_empty(),
        "citation-register row(s) matching other than exactly one line — if the \
         sentence was reworded, re-key the row; if it is gone, delete it:\n{}",
        stale.join("\n")
    );

    // Non-vacuity: the alias half must actually be reading references. The
    // citations corrected in this commit are the floor.
    assert!(
        alias_refs >= 4,
        "only {alias_refs} `compat::` reference(s) across the doc surface — the \
         walk shrank and this half of the gate went vacuous"
    );
}

/// The operational docs whose sections cite code by `file.rs:LINE`.
///
/// Both are already in [`operational_docs`] for the compat half above; the two
/// halves are split because the two ways such a sentence rots are independent.
/// That half asks *does the file this tag-line names still carry a marker*;
/// this one asks *does the line this sentence names still hold the thing the
/// sentence calls it*. R4133_PROPS RP5.1 wrote 46 fresh `file.rs:LINE`
/// citations into these two files and its audit round found the second question
/// asked by nobody — the compat walk's path extractor stops at `.rs` and never
/// reads a `:LINE` suffix.
///
/// `docs/` records and the plans stay out, for the reason
/// [`operational_docs`] already gives: they state history and are *allowed* to
/// differ from HEAD.
const LINE_CITED_DOCS: &[(&str, usize)] = &[
    // (doc, the floor its own citations must not fall below)
    ("TESTING.md", 40),
    ("tests/TOLERANCE_NOTES.md", 10),
];

/// Every file-shaped token on one documentation line, in reading order.
///
/// `None` as the line number is a mention without a `:LINE` suffix — it still
/// matters, because it is the antecedent a later bare `` `:LINE` `` inherits.
/// That inheritance is why non-Rust extensions are returned too: most bare
/// continuations in `tests/TOLERANCE_NOTES.md` hang off a `.pas` citation into
/// the Pascal spec, which lives outside this repository and must NOT be
/// resolved against it.
///
/// An empty path is the bare form itself.
///
/// The third element is the END of a `:A-B` range citation. G1.7's audit found
/// it unread: only the digits before the `-` were parsed, so a range end could
/// name a line past the file, or drift out from under the symbol the sentence
/// spells, without a word (four `harness/mod.rs:A-B` rows had done exactly
/// that). A range is a claim about a BLOCK, so the anchor must sit inside the
/// whole block — see [`operational_docs_line_citations_point_at_the_line_they_name`].
fn file_citations_in(line: &str) -> Vec<(String, Option<usize>, Option<usize>)> {
    let chars: Vec<char> = line.chars().collect();
    let is_path = |c: char| c.is_ascii_alphanumeric() || matches!(c, '_' | '/' | '.' | '-');
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < chars.len() {
        // The bare continuation, always backtick-anchored so a stray `:12` in
        // prose is not read as a citation.
        if chars[i] == '`' && chars.get(i + 1) == Some(&':') {
            let mut j = i + 2;
            while chars.get(j).is_some_and(char::is_ascii_digit) {
                j += 1;
            }
            if j > i + 2 {
                let n: String = chars[i + 2..j].iter().collect();
                let (end, j) = range_end(&chars, j);
                out.push((String::new(), n.parse::<usize>().ok(), end));
                i = j;
                continue;
            }
        }
        if !is_path(chars[i]) {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && is_path(chars[i]) {
            i += 1;
        }
        // Sentence punctuation ("... `props_norm.rs`.") is not part of the path.
        let mut end = i;
        while end > start && matches!(chars[end - 1], '.' | '-') {
            end -= 1;
        }
        let run: String = chars[start..end].iter().collect();
        let Some(dot) = run.rfind('.') else { continue };
        let ext = &run[dot + 1..];
        if dot == 0
            || !(2..=4).contains(&ext.len())
            || !ext.chars().all(|c| c.is_ascii_alphabetic())
        {
            continue;
        }
        // A `:LINE` suffix, if the run is immediately followed by one.
        let mut j = end;
        let mut lineno = None;
        let mut lineend = None;
        if chars.get(j) == Some(&':') {
            let mut k = j + 1;
            while chars.get(k).is_some_and(char::is_ascii_digit) {
                k += 1;
            }
            if k > j + 1 {
                let n: String = chars[j + 1..k].iter().collect();
                lineno = n.parse::<usize>().ok();
                let (e, k) = range_end(&chars, k);
                lineend = e;
                j = k;
            }
        }
        out.push((run, lineno, lineend));
        i = i.max(j);
    }
    out
}

/// The `-B` half of a `:A-B` range citation, read from just past the `A`.
///
/// Returns the end line and the new cursor; `(None, j)` when what follows is a
/// hyphen that is not a range (`props_norm.rs:895-ish` prose, a dashed word).
fn range_end(chars: &[char], j: usize) -> (Option<usize>, usize) {
    if chars.get(j) != Some(&'-') {
        return (None, j);
    }
    let mut k = j + 1;
    while chars.get(k).is_some_and(char::is_ascii_digit) {
        k += 1;
    }
    if k == j + 1 {
        return (None, j);
    }
    let n: String = chars[j + 1..k].iter().collect();
    (n.parse::<usize>().ok(), k)
}

/// The backticked Rust identifiers a documentation line names, `::`-tails only.
///
/// This is the *anchor* half of the citation check: a line number alone is a
/// weak claim (a 5 000-line harness file has a line 3 311 no matter how far the
/// thing it named drifted), so the citation must land near something the
/// sentence itself spells.
fn backticked_idents(line: &str) -> Vec<String> {
    line.split('`')
        .skip(1)
        .step_by(2)
        .filter(|span| {
            span.chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                && span
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == ':')
        })
        .map(|span| span.rsplit("::").next().unwrap_or(span).to_string())
        .filter(|s| s.len() > 2)
        .collect()
}

/// The tree files a documentation-spelled path can mean: an exact
/// repo-relative match, or any file whose path ends with it (`mod.rs`,
/// `harness/mod.rs`).
fn resolve_cited(by_base: &BTreeMap<String, Vec<String>>, cited: &str, base: &str) -> Vec<String> {
    let suffix = format!("/{cited}");
    by_base
        .get(base)
        .map(|v| {
            v.iter()
                .filter(|rel| *rel == cited || rel.ends_with(&suffix))
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

/// A `file.rs:LINE` citation in the operational docs still points at the line
/// the sentence names.
///
/// Three failures, all of them silent before this test: the file is gone or the
/// path is spelled too loosely to name one file; the line is past the end of
/// it; or the line drifted away from what the sentence calls it. The third is
/// the one that actually happens — harness files grow by hundreds of lines a
/// sub-step, and nothing in the tree read a `:LINE` suffix.
///
/// Resolution follows the reader's own rule: a path is matched against the tree
/// by suffix, and a shortened repeat (`mod.rs:3281` after the section spelled
/// `crates/dss-core/tests/harness/mod.rs:3243`) is disambiguated by the nearest
/// fully-qualified mention **earlier in the same document**. A citation that
/// resolves to neither fails rather than being skipped — an ambiguous citation
/// is a doc defect, not an exemption.
#[test]
fn operational_docs_line_citations_point_at_the_line_they_name() {
    let root = repo_root();

    let mut by_base: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for path in rust_sources(&root) {
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let Some(base) = rel.rsplit('/').next().map(str::to_string) else {
            continue;
        };
        by_base.entry(base).or_default().push(rel);
    }

    let mut cache: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut bad: Vec<String> = Vec::new();
    let mut counts: Vec<(&str, usize)> = Vec::new();

    for (doc, floor) in LINE_CITED_DOCS {
        let text = fs::read_to_string(root.join(doc)).unwrap_or_else(|e| panic!("{doc}: {e}"));
        let lines: Vec<&str> = text.lines().collect();
        let mut full_of_base: BTreeMap<String, String> = BTreeMap::new();
        let mut last_file: Option<String> = None;
        let mut checked = 0usize;

        for (i, line) in lines.iter().enumerate() {
            let mut idents = backticked_idents(line);
            if i > 0 {
                // Docs wrap: the name and its citation routinely straddle a
                // line break.
                idents.extend(backticked_idents(lines[i - 1]));
            }

            for (tok, lineno, lineend) in file_citations_in(line) {
                let cited = if tok.is_empty() {
                    match &last_file {
                        Some(f) => f.clone(),
                        None => continue,
                    }
                } else {
                    // Register a fully-qualified spelling so the section's
                    // later short repeats resolve.
                    if tok.contains('/') && tok.ends_with(".rs") {
                        let base = tok.rsplit('/').next().unwrap_or(&tok).to_string();
                        let hits = resolve_cited(&by_base, &tok, &base);
                        if hits.len() == 1 {
                            full_of_base.insert(base, hits[0].clone());
                        }
                    }
                    last_file = Some(tok.clone());
                    tok
                };
                let Some(ln) = lineno else { continue };
                if !cited.ends_with(".rs") {
                    continue;
                }

                let base = cited.rsplit('/').next().unwrap_or(&cited).to_string();
                let hits = resolve_cited(&by_base, &cited, &base);
                let target = match (hits.len(), full_of_base.get(&base)) {
                    (1, _) => hits[0].clone(),
                    (_, Some(full)) => full.clone(),
                    (0, None) => {
                        bad.push(format!(
                            "    {doc}:{}: `{cited}:{ln}` names no file in the tree",
                            i + 1
                        ));
                        continue;
                    }
                    (n, None) => {
                        bad.push(format!(
                            "    {doc}:{}: `{cited}:{ln}` matches {n} files and no \
                             fully-qualified spelling precedes it — spell enough of \
                             the path",
                            i + 1
                        ));
                        continue;
                    }
                };

                let src = cache.entry(target.clone()).or_insert_with(|| {
                    fs::read_to_string(root.join(&target))
                        .unwrap_or_else(|e| panic!("{target}: {e}"))
                        .lines()
                        .map(str::to_string)
                        .collect()
                });
                checked += 1;

                if ln == 0 || ln > src.len() {
                    bad.push(format!(
                        "    {doc}:{}: `{cited}:{ln}` is past the end of {target} \
                         ({} lines)",
                        i + 1,
                        src.len()
                    ));
                    continue;
                }
                if src[ln - 1].trim().is_empty() {
                    bad.push(format!(
                        "    {doc}:{}: `{cited}:{ln}` points at a blank line of {target}",
                        i + 1
                    ));
                    continue;
                }
                // A `:A-B` range: the END must be a real line of the same file
                // and must not precede the start. Before G1.7's audit settlement
                // the end was never parsed at all.
                if let Some(end) = lineend
                    && (end < ln || end > src.len())
                {
                    bad.push(format!(
                        "    {doc}:{}: `{cited}:{ln}-{end}` is not a range of \
                         {target} ({} lines)",
                        i + 1,
                        src.len()
                    ));
                    continue;
                }
                if idents.is_empty() {
                    // Before RP5.2's audit settlement this was a silent
                    // `continue`, which made the citation existence-only: any
                    // non-blank line of the right file passed. Six of the 58
                    // citations sat in that hole — including the four the
                    // R4133_PROPS counters lean on (the normalization table,
                    // the echo table, the display floor, the forced
                    // population) — so the doc claim "resolves *and anchors*
                    // all 58" was true of 52. Anchoring costs one backticked
                    // symbol on the citing line.
                    bad.push(format!(
                        "    {doc}:{}: `{cited}:{ln}` names no backticked symbol on \
                         its own line or the one above, so nothing but the line's \
                         existence can be checked — spell the symbol the cited line \
                         defines",
                        i + 1
                    ));
                    continue;
                }
                // A single line is anchored in a small window around it; a RANGE
                // is a claim about the whole block, so the symbol must sit inside
                // the block itself — the settlement rule that catches an end
                // drifting off the values the sentence names.
                let (lo, hi) = match lineend {
                    Some(end) => (ln - 1, end.min(src.len())),
                    None => (ln.saturating_sub(4), (ln + 3).min(src.len())),
                };
                let window = src[lo..hi].join("\n");
                if !idents.iter().any(|id| window.contains(id.as_str())) {
                    bad.push(format!(
                        "    {doc}:{}: `{cited}:{ln}` — {target} lines {}-{} name \
                         none of {:?} (a `:A-B` citation is anchored inside the \
                         range itself)",
                        i + 1,
                        lo + 1,
                        hi,
                        idents
                    ));
                }
            }
        }
        counts.push((doc, checked));
        assert!(
            checked >= *floor,
            "{doc} yielded only {checked} `file.rs:LINE` citations (floor {floor}) \
             — either the section that carries them is gone or the scanner stopped \
             seeing them, and this gate went vacuous"
        );
    }

    assert!(
        bad.is_empty(),
        "documentation citing a code line that no longer says what the sentence \
         claims. Re-read the cited file and re-point the citation (or re-word the \
         sentence); do NOT delete the line number:\n{}\nchecked: {:?}",
        bad.join("\n"),
        counts
    );
}

/// Every `` `path.md:LO[-HI]` `` citation on one line of Rust, in reading order.
///
/// Same shape as [`file_citations_in`], narrowed to Markdown targets and
/// widened to the `LO-HI` ranges a prose citation actually uses: a plan
/// sentence spans lines, and a one-line citation into it would be a claim about
/// the wrong half of the sentence.
fn md_citations_in(line: &str) -> Vec<(String, usize, usize)> {
    let chars: Vec<char> = line.chars().collect();
    let is_path = |c: char| c.is_ascii_alphanumeric() || matches!(c, '_' | '/' | '.' | '-');
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < chars.len() {
        if !is_path(chars[i]) {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && is_path(chars[i]) {
            i += 1;
        }
        let run: String = chars[start..i].iter().collect();
        if !run.ends_with(".md") || chars.get(i) != Some(&':') {
            continue;
        }
        let mut j = i + 1;
        while chars.get(j).is_some_and(char::is_ascii_digit) {
            j += 1;
        }
        let Ok(lo) = chars[i + 1..j].iter().collect::<String>().parse::<usize>() else {
            continue;
        };
        let mut hi = lo;
        if chars.get(j) == Some(&'-') {
            let mut k = j + 1;
            while chars.get(k).is_some_and(char::is_ascii_digit) {
                k += 1;
            }
            if let Ok(end) = chars[j + 1..k].iter().collect::<String>().parse::<usize>() {
                hi = end;
                j = k;
            }
        }
        out.push((run, lo, hi));
        i = i.max(j);
    }
    out
}

/// The section tokens a comment names (`WP-RP3`, `RP5.2`, `1.1`), each spelled
/// behind a section sign.
fn section_tokens_in(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    for (i, c) in chars.iter().enumerate() {
        if *c != '\u{a7}' {
            continue;
        }
        let mut j = i + 1;
        while chars
            .get(j)
            .is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
        {
            j += 1;
        }
        let tok: String = chars[i + 1..j].iter().collect();
        let tok = tok.trim_end_matches(['.', '-']).to_string();
        if tok.len() > 1 {
            out.push(tok);
        }
    }
    out
}

/// The 1-based span of every Markdown heading whose title names `token`, each
/// running to the next heading of the same or higher level.
fn heading_spans(lines: &[&str], token: &str) -> Vec<(usize, usize)> {
    let level = |l: &str| {
        let n = l.chars().take_while(|c| *c == '#').count();
        (n > 0 && l.chars().nth(n) == Some(' ')).then_some(n)
    };
    let mut out = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let Some(lv) = level(line) else { continue };
        if !line.contains(token) {
            continue;
        }
        let mut end = lines.len();
        for (j, later) in lines.iter().enumerate().skip(i + 1) {
            if level(later).is_some_and(|l| l <= lv) {
                end = j;
                break;
            }
        }
        out.push((i + 1, end));
    }
    out
}

/// Markdown prose reduced to the words it says: comment markers, emphasis and
/// backticks dropped, whitespace collapsed, so a quotation matches across a
/// line wrap and across `**bold**` the quoter kept or dropped.
fn normalized_md(text: &str) -> String {
    let plain: String = text
        .lines()
        .map(|l| l.trim_start().trim_start_matches('/').trim_start())
        .collect::<Vec<_>>()
        .join(" ")
        .replace(['`', '*'], "");
    plain.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The opening words of every quoted phrase a comment carries, cut at the first
/// ellipsis and capped at eight words.
///
/// A quotation is the strongest form of citation — it claims the passage says
/// *this* — so it is checked against the passage, which makes a citation that
/// drifts *within* a section fail too, not only one that leaves it.
fn quoted_prefixes(block: &str) -> Vec<String> {
    normalized_md(block)
        .split('"')
        .skip(1)
        .step_by(2)
        .filter_map(|quote| {
            let head = quote
                .split('\u{2026}')
                .next()
                .unwrap_or(quote)
                .split("...")
                .next()
                .unwrap_or(quote);
            let words: Vec<&str> = head.split_whitespace().take(8).collect();
            (words.len() >= 4).then(|| words.join(" "))
        })
        .collect()
}

/// The contiguous comment block a Rust line sits in — the sentence that makes
/// the claim, not an arbitrary window around it.
fn comment_block(lines: &[&str], at: usize) -> String {
    let is_comment = |l: &&str| l.trim_start().starts_with("//");
    let mut lo = at;
    while lo > 0 && is_comment(&lines[lo - 1]) {
        lo -= 1;
    }
    let mut hi = at;
    while hi + 1 < lines.len() && is_comment(&lines[hi + 1]) {
        hi += 1;
    }
    lines[lo..=hi].join("\n")
}

/// A `record.md:LO-HI` citation inside a Rust comment still points at the
/// passage the comment names.
///
/// The mirror image of [`operational_docs_line_citations_point_at_the_line_they_name`],
/// and the class that walk deliberately drops: it resolves `.rs` targets and
/// `continue`s past every other extension, so a comment citing a plan or a
/// phase record was checked by nothing at all. The rot is measured, not
/// hypothetical — R4133_PROPS RP5.2's only code edit was re-pointing the two
/// `props_r4133_replay.rs` citations of the WP-RP3 sanctioned-outcome sentence,
/// whose range had drifted onto the RP2.4 display-floor derivation while both
/// lanes stayed green. Its audit round (2026-09-04) found the repair itself
/// unguarded, which is this test.
///
/// Three checks: the record exists; the cited range is inside it and not all
/// blank; and the passage is **anchored** — either the comment names a section
/// whose span contains the range, or comment and passage share a backticked
/// symbol. An unanchored citation fails: a bare line number into a 2 800-line
/// plan is a claim no reader can check and no edit can invalidate.
#[test]
fn rust_comments_citing_a_record_line_point_at_the_passage_they_name() {
    let root = repo_root();
    let mut bad: Vec<String> = Vec::new();
    let mut cache: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut checked = 0usize;
    let mut citing_files: Vec<String> = Vec::new();

    for path in rust_sources(&root) {
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{rel}: {e}"));
        let lines: Vec<&str> = text.lines().collect();

        for (i, line) in lines.iter().enumerate() {
            for (doc, lo, hi) in md_citations_in(line) {
                if !root.join(&doc).is_file() {
                    bad.push(format!(
                        "    {rel}:{}: `{doc}:{lo}` names no file in the tree",
                        i + 1
                    ));
                    continue;
                }
                checked += 1;
                if !citing_files.contains(&rel) {
                    citing_files.push(rel.clone());
                }

                let doc_text = cache.entry(doc.clone()).or_insert_with(|| {
                    fs::read_to_string(root.join(&doc))
                        .unwrap_or_else(|e| panic!("{doc}: {e}"))
                        .lines()
                        .map(str::to_string)
                        .collect()
                });
                let doc_lines: Vec<&str> = doc_text.iter().map(String::as_str).collect();

                if lo == 0 || hi < lo || hi > doc_lines.len() {
                    bad.push(format!(
                        "    {rel}:{}: `{doc}:{lo}-{hi}` is not inside {doc} ({} lines)",
                        i + 1,
                        doc_lines.len()
                    ));
                    continue;
                }
                let cited = &doc_lines[lo - 1..hi];
                if cited.iter().all(|l| l.trim().is_empty()) {
                    bad.push(format!(
                        "    {rel}:{}: `{doc}:{lo}-{hi}` is blank in {doc}",
                        i + 1
                    ));
                    continue;
                }

                let block = comment_block(&lines, i);
                let passage = normalized_md(&cited.join("\n"));
                let quotes = quoted_prefixes(&block);
                let by_quote = quotes.iter().any(|q| passage.contains(q.as_str()));
                if !quotes.is_empty() && !by_quote {
                    bad.push(format!(
                        "    {rel}:{}: `{doc}:{lo}-{hi}` — the comment quotes {quotes:?} \
                         and the cited lines say none of it",
                        i + 1
                    ));
                    continue;
                }
                if by_quote {
                    continue;
                }
                let tokens = section_tokens_in(&block);
                let by_section = tokens.iter().any(|tok| {
                    heading_spans(&doc_lines, tok)
                        .iter()
                        .any(|(start, end)| *start <= lo && hi <= *end)
                });
                if by_section {
                    continue;
                }
                let named: Vec<String> = block.lines().flat_map(backticked_idents).collect();
                let passage: Vec<String> =
                    cited.iter().flat_map(|l| backticked_idents(l)).collect();
                if named.iter().any(|id| passage.contains(id)) {
                    continue;
                }
                bad.push(format!(
                    "    {rel}:{}: `{doc}:{lo}-{hi}` is unanchored — the comment names \
                     no section {tokens:?} whose span holds those lines, and the passage \
                     spells none of the symbols the comment does. Name the section, or \
                     quote a symbol the cited lines carry",
                    i + 1
                ));
            }
        }
    }

    assert!(
        checked >= 5 && citing_files.len() >= 2,
        "the walk found only {checked} `record.md:LINE` citations over {citing_files:?} \
         — either they were all deleted or the scanner stopped seeing them, and this \
         gate went vacuous"
    );
    assert!(
        bad.is_empty(),
        "a Rust comment cites a record line that no longer carries the passage it \
         names. Re-read the record and re-point the range (or re-word the comment); \
         do NOT delete the line number:\n{}\nchecked: {checked} citations over {:?}",
        bad.join("\n"),
        citing_files
    );
}

// ---------------------------------------------------------------------------
// GOLDEN_REBASE G1.9 — the pin names the operational docs cite must exist.
// ---------------------------------------------------------------------------

/// Every expected-value pin the G1.9 circuit-aggregates surface is documented
/// by, in `TESTING.md`, `tests/TOLERANCE_NOTES.md`, `GOLDEN_REBASE_PLAN.md` and
/// the phase record.
///
/// Why a hand-kept list: the `RP<n>.<n>` substep-marker tagger
/// ([`substep_markers_are_tagged_and_do_not_shadow_the_register`]) only accepts
/// the `R4133_PROPS` spelling and the reserved pin-marker spelling belongs to a
/// torn-down `compat` row, so a G-plan sub-step's pins were in no
/// machine-checked guard at all — renaming or deleting one left four documents
/// citing a test that no longer exists, and nothing reded (G1.9 audit T3).
const G1_9_PINS: [&str; 16] = [
    // `crates/dss-core/src/exec/tests/aggregates.rs`
    "circuit_losses_are_watts_not_kilowatts",
    "substation_losses_exclude_autotrans",
    "losses_skip_shunt_elements",
    "line_losses_sum_the_lines_list",
    "total_power_is_terminal_one_of_every_source",
    "total_iterations_is_an_alias_of_iterations",
    "all_element_losses_follow_creation_order",
    "the_two_boolean_solution_flags_take_both_values",
    // `crates/dss-epri/{src/capture.rs, tests/modes.rs}`
    "r4133_solution_flags_are_zero_one_ints",
    "the_five_circuit_aggregate_rows_are_impure",
    "complex_pair_refuses_a_reply_that_is_not_two_doubles",
    // the audit-settlement guards (`corpus_gate/ledger.rs`, `harness/aggregates.rs`)
    "the_aggregate_value_arms_inherit_exactly_the_recorded_element_scopes",
    "a_currents_only_scope_leaves_the_total_power_arm_running",
    // `crates/dss-core/tests/capture_order.rs`
    "capi_capture_reads_the_aggregates_before_any_currents_read",
    "r4133_capture_reads_the_aggregates_before_any_currents_read",
    "the_gate_rejects_a_swapped_or_renamed_capture",
];

/// The documents that cite the G1.9 pins by name. Each pin must be named by at
/// least one of them, so the guard is a tripwire in **both** directions: a
/// renamed test reds on the tree side, and a pin quietly dropped from the docs
/// reds on this side.
const G1_9_PIN_DOCS: [&str; 4] = [
    "TESTING.md",
    "tests/TOLERANCE_NOTES.md",
    "GOLDEN_REBASE_PLAN.md",
    "docs/phase-records/golden-rebase.md",
];

#[test]
fn the_g1_9_pins_the_docs_cite_exist_exactly_once() {
    let root = repo_root();
    let sources: Vec<String> = rust_sources(&root)
        .iter()
        .map(|p| fs::read_to_string(p).expect("source is readable"))
        .collect();
    let docs: Vec<String> = G1_9_PIN_DOCS
        .iter()
        .map(|rel| {
            fs::read_to_string(root.join(rel))
                .unwrap_or_else(|e| panic!("{rel} is part of the G1.9 doc surface: {e}"))
        })
        .collect();

    for pin in G1_9_PINS {
        let needle = format!("fn {pin}(");
        let defs: usize = sources.iter().map(|t| t.matches(&needle).count()).sum();
        assert_eq!(
            defs,
            1,
            "the G1.9 pin `{pin}` is defined {defs} times in the tree, expected \
             exactly 1 — {} cite it by name, so a rename or a deletion must red \
             here instead of leaving them stale",
            G1_9_PIN_DOCS.join(" / ")
        );
        assert!(
            docs.iter().any(|d| d.contains(pin)),
            "the G1.9 pin `{pin}` is in this registry but no longer named by any \
             of {} — either restore the citation or drop the pin from the list",
            G1_9_PIN_DOCS.join(" / ")
        );
    }
}

/// The G1.7 (topology surface) names the operational docs and the phase record
/// cite, with the number of definitions each one must have in the tree.
///
/// The G1.9 registry above is the precedent; G1.7's audit round found the twin
/// missing while three documents cited eighteen G1.7 identifiers by hand. The
/// count is part of the claim: `window_dedup` is **two** definitions on purpose
/// — the gate-side model in `harness/topology.rs` and its engine-side duplicate
/// in `exec/tests/topology.rs`, which live in different compilation targets and
/// must both keep the Pascal's indices (`DDLL/DTopology.pas:277-301`). Everything
/// else is one.
const G1_7_PINS: [(&str, usize); 14] = [
    // in-engine pins (`exec/tests/topology.rs`, `elements/ckt.rs`)
    ("a_gictransformer_is_a_tree_branch_and_can_close_a_loop", 1),
    ("a_second_makeposseq_keeps_the_circuit_connected", 1),
    ("a_no_op_set_nconds_keeps_the_terminal_state", 1),
    // the two both-numbers pins (`tests/topology_pins.rs`)
    ("topology_reads_a_freshly_built_tree", 1),
    ("looped_pairs_lose_the_straddling_window", 1),
    // the rails (`corpus_gate/scheduler.rs`, `capture_order.rs`)
    ("the_topology_forcing_rule_is_every_live_non_large_case", 1),
    ("assert_topology_declines_are_the_pinned_population", 1),
    (
        "the_r4133_bridge_exposes_only_the_six_order_free_topology_rows",
        1,
    ),
    // the comparator and the two transports' normalizers
    ("compare_topology", 1),
    ("normalize_topo_names", 1),
    ("per_pair_dedup", 1),
    ("topo_names", 1),
    ("topology_view", 1),
    // the deliberate twin — see the doc above
    ("window_dedup", 2),
];

/// The documents that cite the G1.7 names, same rule as [`G1_9_PIN_DOCS`].
const G1_7_PIN_DOCS: [&str; 4] = [
    "TESTING.md",
    "tests/TOLERANCE_NOTES.md",
    "GOLDEN_REBASE_PLAN.md",
    "docs/phase-records/golden-rebase.md",
];

#[test]
fn the_g1_7_pins_the_docs_cite_exist_exactly_once() {
    let root = repo_root();
    let sources: Vec<String> = rust_sources(&root)
        .iter()
        .map(|p| fs::read_to_string(p).expect("source is readable"))
        .collect();
    let docs: Vec<String> = G1_7_PIN_DOCS
        .iter()
        .map(|rel| {
            fs::read_to_string(root.join(rel))
                .unwrap_or_else(|e| panic!("{rel} is part of the G1.7 doc surface: {e}"))
        })
        .collect();

    for (pin, want) in G1_7_PINS {
        let needle = format!("fn {pin}(");
        let defs: usize = sources.iter().map(|t| t.matches(&needle).count()).sum();
        assert_eq!(
            defs,
            want,
            "the G1.7 name `{pin}` is defined {defs} times in the tree, expected \
             exactly {want} — {} cite it by name, so a rename, a deletion or an \
             undocumented second copy must red here instead of leaving them stale",
            G1_7_PIN_DOCS.join(" / ")
        );
        assert!(
            docs.iter().any(|d| d.contains(pin)),
            "the G1.7 name `{pin}` is in this registry but no longer named by any \
             of {} — either restore the citation or drop it from the list",
            G1_7_PIN_DOCS.join(" / ")
        );
    }
}

/// The G1.8 (incidence matrix / Laplacian surface) names the operational docs
/// and the phase record cite, with the number of definitions each one must have
/// in the tree — the [`G1_9_PINS`] / [`G1_7_PINS`] pattern, third instance.
///
/// G1.8 landed without its twin (its F9 handoff flagged the gap), so ~17
/// identifiers were cited by four documents with nothing to red on a rename.
/// Everything here is one definition: unlike G1.7's `window_dedup` there is no
/// deliberate engine-side duplicate, and `capture_inc_matrix` is deliberately
/// NOT in the list — `capture_order.rs` names it in two search needles and once
/// more in a synthetic fixture, so a count there would pin test scaffolding
/// rather than the capture.
const G1_8_PINS: [(&str, usize); 22] = [
    // the capture-order gates (`crates/dss-core/tests/capture_order.rs`)
    ("capi_capture_reads_the_incidence_surface_last", 1),
    ("r4133_capture_reads_the_incidence_surface_last", 1),
    (
        "the_incidence_capture_issues_calcincmatrix_then_calclaplacian",
        1,
    ),
    (
        "neither_capture_calls_calcincmatrix_o_or_reads_buslevels",
        1,
    ),
    ("the_incidence_gates_reject_a_swapped_or_early_capture", 1),
    // settlement S-INC, both numbers (`crates/dss-core/tests/inc_matrix_pins.rs`)
    ("the_incidence_row_cursor_skips_a_shunt_reactor", 1),
    ("the_row_cursor_settlement_holds_on_the_corpus_witness", 1),
    ("the_laplacian_is_blind_to_the_row_cursor", 1),
    // the transport / getter shape pins (same file)
    ("inc_matrix_cols_are_the_bus_list_when_unordered", 1),
    ("capi_incmatrix_carries_one_trailing_zero", 1),
    ("capi_and_r4133_incmatrix_lengths_differ_by_one", 1),
    ("an_empty_incidence_matrix_reads_back_as_no_rows", 1),
    ("calclaplacian_without_calcincmatrix_raises_8877", 1),
    // the two defects left reproduced (`exec/tests/inc_matrix.rs`) and the fix
    // that was not (`solution/inc_matrix/tests.rs`)
    (
        "a_one_terminal_reactor_becomes_a_phantom_branch_to_the_last_bus",
        1,
    ),
    ("a_disabled_series_reactor_is_still_a_row", 1),
    ("the_reactor_row_cursor_advances_only_on_an_emitted_row", 1),
    // the rails (`corpus_gate/scheduler.rs`, `harness/inc_matrix.rs`)
    (
        "the_inc_matrix_forcing_rule_is_every_live_non_large_case",
        1,
    ),
    (
        "the_inc_matrix_surface_is_declared_on_every_gating_channel",
        1,
    ),
    ("assert_declines_are_the_pinned_population", 1),
    // the comparator, the accessor and its two helpers
    ("compare_inc_matrix", 1),
    ("inc_matrix_view", 1),
    ("inc_matrix_cols", 1),
];

/// The documents that cite the G1.8 names, same rule as [`G1_9_PIN_DOCS`].
const G1_8_PIN_DOCS: [&str; 4] = [
    "TESTING.md",
    "tests/TOLERANCE_NOTES.md",
    "GOLDEN_REBASE_PLAN.md",
    "docs/phase-records/golden-rebase.md",
];

#[test]
fn the_g1_8_pins_the_docs_cite_exist_exactly_once() {
    let root = repo_root();
    let sources: Vec<String> = rust_sources(&root)
        .iter()
        .map(|p| fs::read_to_string(p).expect("source is readable"))
        .collect();
    let docs: Vec<String> = G1_8_PIN_DOCS
        .iter()
        .map(|rel| {
            fs::read_to_string(root.join(rel))
                .unwrap_or_else(|e| panic!("{rel} is part of the G1.8 doc surface: {e}"))
        })
        .collect();

    for (pin, want) in G1_8_PINS {
        let needle = format!("fn {pin}(");
        let defs: usize = sources.iter().map(|t| t.matches(&needle).count()).sum();
        assert_eq!(
            defs,
            want,
            "the G1.8 name `{pin}` is defined {defs} times in the tree, expected \
             exactly {want} — {} cite it by name, so a rename, a deletion or an \
             undocumented second copy must red here instead of leaving them stale",
            G1_8_PIN_DOCS.join(" / ")
        );
        assert!(
            docs.iter().any(|d| d.contains(pin)),
            "the G1.8 name `{pin}` is in this registry but no longer named by any \
             of {} — either restore the citation or drop it from the list",
            G1_8_PIN_DOCS.join(" / ")
        );
    }
}
