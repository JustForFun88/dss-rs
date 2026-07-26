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
