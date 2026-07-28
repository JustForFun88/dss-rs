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

/// Every `.rs` file under `crates/*/{src,tests,benches,examples}`.
fn rust_sources(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let crates = root.join("crates");
    let mut dirs: Vec<PathBuf> = fs::read_dir(&crates)
        .expect("crates/ is readable")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .flat_map(|p| {
            ["src", "tests", "benches", "examples"]
                .iter()
                .map(|sub| p.join(sub))
                .collect::<Vec<_>>()
        })
        .filter(|p| p.is_dir())
        .collect();

    while let Some(dir) = dirs.pop() {
        for entry in fs::read_dir(&dir).expect("directory is readable").flatten() {
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path);
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
    out
}

/// A file may carry the cfg string when it is part of a `compat` module
/// (`.../compat.rs` or anything under `.../compat/`) or when it is test code
/// (`crates/*/tests/**`, or a `tests.rs` unit-test module).
fn is_sanctioned(path: &Path, root: &Path) -> bool {
    let rel = path.strip_prefix(root).unwrap_or(path);
    let parts: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
        .collect();
    let file = parts.last().expect("a file name");

    let in_compat = file == "compat.rs" || parts.iter().any(|p| p == "compat");
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
    for expected in [
        "crates/dss-core/src/compat.rs",
        "crates/dss-parser/src/compat.rs",
        "crates/dss-sparse/src/compat.rs",
    ] {
        let path = root.join(expected);
        let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{expected}: {e}"));
        assert!(
            text.contains(&needle),
            "{expected} carries no `{needle}` — the Stage F seam is gone"
        );
    }
    assert!(sanctioned_hits.len() >= 3);
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
/// *measured* the cost of every one that does not. These three variants are the
/// three reasons a measurement came back "no": the fix is another step's job,
/// the fix needs an oracle re-baseline, or the fix costs a whole gated case its
/// default-lane oracle comparison.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Escape {
    /// **F.4 (`F-FMT`).** A rendering marker: it belongs to the `compat::fmt`
    /// seam and the table-layout step, which is where `DE_PASCALIZE_PLAN.md`
    /// Part IV.2 §F-FMT puts it. Not a numeric kernel.
    Ffmt,
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
    // ---- the rendering seam → F.4 (`F-FMT`) (7) ----
    (
        "crates/dss-core/src/util.rs",
        "the final cut to `sig` digits is **not correctly rounded**",
        Escape::Ffmt,
    ),
    (
        "crates/dss-core/src/report/format.rs",
        "reproduces FPC's two-stage decimal rounding",
        Escape::Ffmt,
    ),
    (
        "crates/dss-core/src/report/show/mod.rs",
        "empirically returns **0**",
        Escape::Ffmt,
    ),
    (
        "crates/dss-core/src/report/show/diagnostics.rs",
        "`%-.g` is FPC `ffGeneral`",
        Escape::Ffmt,
    ),
    (
        "crates/dss-core/src/report/export/json/circuit.rs",
        "one marker for the whole family below",
        Escape::Ffmt,
    ),
    (
        "crates/dss-core/src/report/export/json/mod.rs",
        "fpjson's fixed 17-significant-digit scientific",
        Escape::Ffmt,
    ),
    (
        "crates/dss-core/src/report/export/json/mod.rs",
        "fpjson `FormatJSON` writes",
        Escape::Ffmt,
    ),
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
];

/// The Stage F.3 exit population, per handoff owner.
///
/// Update these three numbers in the same commit that closes a row — that is
/// the point of stating them: the count is the plan's exit criterion, so it
/// should move only on purpose.
const EXIT_POPULATION: [(Escape, usize); 3] = [
    (Escape::UpgradeRung, 11),
    (Escape::Ffmt, 7),
    (Escape::WholeCase, 4),
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
