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
const SKIP_DIRS: &[&str] = &[
    ".git",
    ".claude",
    ".inputs",
    ".venv",
    "target",
    "node_modules",
];

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
                if !SKIP_DIRS.contains(&name.as_str()) {
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
    // F.4a took the six *number-rendering* rows through the `compat` seam:
    // `util.rs`'s `%g` two-stage rounding (`compat::fmt_g`),
    // `report/format.rs`'s script fixed-point (`compat::fixed_w_script`),
    // `show/diagnostics.rs`'s unobservable control-queue precision
    // (`compat::CONTROL_QUEUE_SEC_DIGITS`), `export/json/mod.rs`'s fpjson float
    // literal (`compat::json_float`) and platform line break
    // (`compat::JSON_LINE_BREAK`), and `export/json/circuit.rs`'s PostCommands
    // umbrella — whose two spellings are the `%g` and fixed rows above and whose
    // command set is IV.1 contract, not compat. F.4b took the seventh, the
    // `Show` device-name column width (`compat::max_device_name_length`).
    // ---- whole-case default-lane exclusions → not the executor's to grant (4) ----
    // `type=Auto` puts the G1 and G2 blocks in series, so honouring `%R2`
    // changes the element admittance and every node voltage downstream:
    // `gictransformer_gic.dss` 4.502e-4 (allowed 1.001e-6), `gic_midi.dss`
    // 1.021e-4 (allowed 1.074e-6).
    (
        "crates/dss-core/src/elements/pd/gic_transformer/solve.rs",
        "Pascal uses `FPctR1` here, NOT `FPctR2`",
        Escape::WholeCase,
    ),
    // The array write the parser would have made collapses `cap_cmat` to
    // `Cs - Cm` where parity reads the 10 µF diagonal: `makeposseq_shunt.dss`
    // 1.438e-1 V against an allowed 3.339e-6.
    (
        "crates/dss-core/src/elements/pd/capacitor/solve.rs",
        "Pascal `SetDouble(ord(TProp.Cuf), Cs - Cm)`",
        Escape::WholeCase,
    ),
    // `shape_mmf.dss` exists *to observe* this filter — its P column is
    // deliberately exponent notation — so the fix costs that deck 1.641e1 V
    // (allowed 8.179e-6) and with it its unrelated sng/dbl MMF-reader coverage.
    (
        "crates/dss-core/src/elements/general/load_shape/compute.rs",
        "the accept-set keeps only bytes in `[46, 58)`",
        Escape::WholeCase,
    ),
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
    (Escape::WholeCase, 4),
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
const SPLIT_ALIAS_POPULATION: usize = 31;

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
/// The only accepted form is a read of the lane constant `ORACLE_PARITY`.
/// Deriving the expectation from the row's **own** alias was accepted until
/// F-settle W4 and is now rejected: engine and test then read the same
/// constant, so the pin asserts "the engine agrees with the declaration" — true
/// by construction whichever way the alias points. Flipping five such rows'
/// `*_DEFAULT_IMPL` back to the parity value (a silently reverted fix) passed
/// the entire workspace suite, in both lanes. No oracle gate can catch that
/// class either: reverting the fix restores exactly what the oracles return.
fn branches_on_lane(text: &str, _alias: &str) -> bool {
    names_token(text, "ORACLE_PARITY")
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
         either through `ORACLE_PARITY` or by reading `compat::<alias>`. \
         Kernel-vs-kernel tests inside the compat module do not count: they \
         assert the two impls against each other, not the observable a deck \
         sees."
    );

    // Non-vacuity of the *walk*, in both shapes a pin is allowed to take — a
    // narrowing of `is_pin_candidate` that dropped either would otherwise show
    // up as "everything still passes".
    for (alias, expected) in [
        (
            "kv_base_search_scale",
            "crates/dss-core/src/solution/solution/dispatch.rs",
        ),
        (
            "IRESIDUAL_FROM_TERMINAL_1",
            "crates/dss-core/tests/golden_reports.rs",
        ),
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
    }
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
    // write are in scope there. Under `tools/` the reverse holds: the `.py`
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
    // marker-free ever since — including after F.3j, which routed the Newton
    // read through `compat::POWERS_REUSE_STALE_NEWTON_ITERMINAL` instead of
    // re-opening a marker there.
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
