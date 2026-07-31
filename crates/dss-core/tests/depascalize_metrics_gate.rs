//! `DE_PASCALIZE_PLAN.md` §Verification — the plan's **success metrics**, as a
//! test instead of a paragraph (Stage F.5, the plan's last step).
//!
//! Every metric in that section was written as an `rg` invocation with a target
//! ("→ **zero**", "→ boundary accessors only", "shrinks to the keep-list
//! families"), re-measured by hand at each stage and recorded in prose. That
//! made them a *report*, not a contract: nothing re-ran them, so a later WP
//! could reintroduce a downcast, an `Rc`, or a flat-offset index and no gate
//! would notice — the whole point of Parts I–III was to make those shapes
//! impossible to have, and a de-Pascalized codebase that quietly re-Pascalizes
//! is exactly the failure mode this plan exists to prevent.
//!
//! Two shapes of assertion, matching the two shapes of metric:
//!
//! * **zero** — the pattern must not occur in *code* at all. These are the
//!   architectural invariants (`Part I`'s downcast elimination, `P7`'s
//!   shared-mutability ban, `P8`'s flat terminal indexing).
//! * **ceiling** — a population the plan deliberately left standing, each
//!   member audited as STAYS-by-design. The number may only go **down**; growth
//!   fails. (Lowering it is a normal, welcome diff: shrink the count in the same
//!   commit.)
//!
//! # Why "in code" needs saying
//!
//! The metrics are greps, and a grep counts prose too. `lib.rs` documents the
//! ban with the very words it bans ("nothing uses `Rc`/`RefCell`/statics"), and
//! `plot/tests.rs` explains what the pre-R1 design used. A gate that counted
//! those would either be permanently red or force the documentation to stop
//! naming what it forbids. So a hit counts only when it occurs **before** any
//! `//` on its line — the same "the marker must be a marker, not prose"
//! discipline `oracle_parity_cfg_gate.rs` applies to the compat tag.

use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "..", ".."].iter().collect()
}

/// Every `.rs` file under `crates/<crate>/src/`, for the crates named by
/// `only` (empty = all of them).
///
/// Deliberately `src` only: the metrics are about the **engine**, and test
/// code legitimately builds the shapes they ban (a `#[cfg(test)]` sink may hold
/// an `Rc<RefCell<_>>` if that is what a probe needs — `plot/tests.rs` explains
/// why the real one no longer does).
fn engine_sources(root: &Path, only: &[&str]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let crates = root.join("crates");
    for entry in fs::read_dir(&crates)
        .expect("crates/ is readable")
        .flatten()
    {
        let name = entry.file_name().to_string_lossy().to_string();
        if !only.is_empty() && !only.contains(&name.as_str()) {
            continue;
        }
        let src = entry.path().join("src");
        if src.is_dir() {
            collect_rs(&src, &mut out);
        }
    }
    out.sort();
    out
}

fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("directory is readable").flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// The code part of a line: everything before its first `//`.
///
/// Not a Rust tokenizer, and it does not need to be — a `//` inside a string
/// literal would only ever *under*-count, i.e. make this gate miss a hit on a
/// line that already contains a comment marker in a string. No such line exists
/// in the tree (checked by the non-vacuity anchors below, which are real code).
fn code_of(line: &str) -> &str {
    line.split_once("//").map_or(line, |(code, _)| code)
}

/// Every `(file, line number, line)` in `files` whose **code** contains any of
/// `needles`.
fn hits(root: &Path, files: &[PathBuf], needles: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    for path in files {
        let Ok(text) = fs::read_to_string(path) else {
            continue;
        };
        let rel = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        for (i, line) in text.lines().enumerate() {
            let code = code_of(line);
            if needles.iter().any(|n| code.contains(n)) {
                out.push(format!("    {rel}:{}: {}", i + 1, line.trim()));
            }
        }
    }
    out
}

/// Non-vacuity for every walk: the file set must be plausible and must contain
/// the module each metric is really about, so a broken walk fails loudly
/// instead of reporting a clean zero.
fn assert_walk_reaches(root: &Path, files: &[PathBuf], min: usize, anchor: &str) {
    assert!(
        files.len() >= min,
        "source walk found only {} files (expected ≥ {min}) — the walk is broken",
        files.len()
    );
    let want = root.join(anchor);
    assert!(
        files.contains(&want),
        "source walk no longer reaches {anchor} — the metric below is measuring \
         a different tree than the one it claims"
    );
}

// ---------------------------------------------------------------------------
// The "zero" metrics
// ---------------------------------------------------------------------------

/// **Part I / R3** — `rg "downcast_ref|downcast_mut|as_any" crates/dss-core/src`
/// → zero.
///
/// The typed-arena store made every element reachable by its own type, so the
/// `Any` round-trip that the 1:1 port inherited (369 downcasts / 716
/// `as_any|as_ckt_element` at the 2026-07-26 measurement) has no reason to
/// exist. Reintroducing one means a new heterogeneous store went in without an
/// `ElemId`, which is the regression this metric was written for.
#[test]
fn part1_metric_no_downcasting_in_the_engine() {
    let root = repo_root();
    let files = engine_sources(&root, &["dss-core"]);
    assert_walk_reaches(&root, &files, 300, "crates/dss-core/src/circuit/circuit.rs");
    let found = hits(&root, &files, &["downcast_ref", "downcast_mut", "as_any"]);
    assert!(
        found.is_empty(),
        "DE_PASCALIZE Part I metric broken: {} downcast site(s) in the engine. \
         Elements are stored in typed arenas and addressed by `ElemId`; if a new \
         call site needs a concrete type, take it from the arena rather than \
         through `Any`:\n{}",
        found.len(),
        found.join("\n")
    );
}

/// **P7 / Part V** — `rg "RefCell|Rc<|static mut|thread_local" crates/*/src`
/// → zero.
///
/// Send-readiness is a *structural* property: `MULTITHREADING` M3/M4 assume the
/// engine owns its state in plain fields, and one `Rc<RefCell<…>>` anywhere in
/// the graph makes `Dss` non-`Send` again — which `assert_send::<Dss>()` would
/// catch, but only after the design damage is done. This metric catches the
/// shape itself, in every product crate.
#[test]
fn p7_metric_no_shared_mutability_or_statics() {
    let root = repo_root();
    let files = engine_sources(&root, &[]);
    assert_walk_reaches(&root, &files, 400, "crates/dss-sparse/src/lib.rs");
    let found = hits(
        &root,
        &files,
        &["RefCell", "Rc<", "static mut", "thread_local"],
    );
    assert!(
        found.is_empty(),
        "DE_PASCALIZE P7 metric broken: {} shared-mutability/static site(s). The \
         engine keeps its state in owned fields so `Dss` stays `Send` for \
         MULTITHREADING M3/M4:\n{}",
        found.len(),
        found.join("\n")
    );
}

/// **P8** — `rg "term_ref\["` → zero outside `TermRef`.
///
/// The flat `(terminal-1)*ncond + conductor` arithmetic was the single most
/// error-prone shape in the port (it is the mechanism of the `Iresidual`
/// upstream bug this stage fixed). P8 replaced it with terminal×conductor views;
/// indexing the raw buffer again outside those accessors reopens the class.
#[test]
fn p8_metric_no_raw_term_ref_indexing() {
    let root = repo_root();
    let files = engine_sources(&root, &[]);
    assert_walk_reaches(&root, &files, 400, "crates/dss-core/src/elements/ckt.rs");
    let found = hits(&root, &files, &["term_ref["]);
    assert!(
        found.is_empty(),
        "DE_PASCALIZE P8 metric broken: {} raw `term_ref[…]` site(s). Use the \
         terminal/conductor views (`terminals_i()`, `TermRef`) — the flat offset \
         form is what produced the `Iresidual` class of bug:\n{}",
        found.len(),
        found.join("\n")
    );
}

/// **P1 (deferred tail)** — `rg "pub const .*: i32 = " crates/dss-core/src/elements`
/// → zero.
///
/// The plan's target was "shrinks to the keep-list families only" (7 lines at
/// the 2026-07-26 measurement); the P1 deferred tail closed all of them, so the
/// metric is now a plain zero. Every element-level integer family is an enum:
/// a new `pub const … : i32` in `elements/` is a Pascal ordinal coming back.
///
/// Scoped to `elements/` exactly as the plan wrote it — `support/`,
/// `solution/` and `compat.rs` legitimately keep i32 constants (unit codes,
/// Carson model selectors, the lane's rendered ordinals).
#[test]
fn p1_metric_no_i32_constant_families_in_elements() {
    let root = repo_root();
    let all = engine_sources(&root, &["dss-core"]);
    let files: Vec<PathBuf> = all
        .into_iter()
        .filter(|p| {
            p.to_string_lossy()
                .replace('\\', "/")
                .contains("/src/elements/")
        })
        .collect();
    assert_walk_reaches(
        &root,
        &files,
        150,
        "crates/dss-core/src/elements/pd/line/mod.rs",
    );
    let found = hits(&root, &files, &["pub const "])
        .into_iter()
        .filter(|l| l.contains(": i32 = "))
        .collect::<Vec<_>>();
    assert!(
        found.is_empty(),
        "DE_PASCALIZE P1 metric broken: {} `pub const …: i32` family member(s) \
         in `elements/`. Model these as enums (P1) — a bare ordinal is a Pascal \
         constant with a Rust type:\n{}",
        found.len(),
        found.join("\n")
    );
}

// ---------------------------------------------------------------------------
// The "ceiling" metrics — audited populations that may only shrink
// ---------------------------------------------------------------------------

/// **P14** — `for … in 1..=` in `elements/`: 106 at the P14 audit settle, every
/// one verified STAYS-by-design (report text, the 1-based user API, Pascal
/// state arrays that are 1-based on the wire).
const CEILING_ONE_BASED_LOOPS: usize = 106;

/// **P8/P10/P11** — the flat-offset multiplication forms (`* nconds`,
/// `* ncond`): 17, all accessor-internal (the views themselves have to compute
/// the offset once, somewhere).
const CEILING_FLAT_OFFSET: usize = 17;

/// The two audited populations Parts III left standing may shrink, never grow.
///
/// A ceiling rather than an equality on purpose: these are not a closed
/// register like the Stage F escapes (whose count *is* the exit criterion and
/// must move deliberately), they are leftovers whose disappearance is always an
/// improvement. Growth, on the other hand, means the de-indexing work is being
/// undone somewhere.
#[test]
fn part3_metrics_audited_populations_do_not_grow() {
    let root = repo_root();
    let all = engine_sources(&root, &["dss-core"]);
    let elements: Vec<PathBuf> = all
        .iter()
        .filter(|p| {
            p.to_string_lossy()
                .replace('\\', "/")
                .contains("/src/elements/")
        })
        .cloned()
        .collect();
    assert_walk_reaches(
        &root,
        &elements,
        150,
        "crates/dss-core/src/elements/pd/transformer/mod.rs",
    );

    let one_based = hits(&root, &elements, &["for "])
        .into_iter()
        .filter(|l| l.contains(" in 1..="))
        .count();
    assert!(
        one_based <= CEILING_ONE_BASED_LOOPS,
        "DE_PASCALIZE P14 metric regressed: {one_based} `for … in 1..=` loops in \
         `elements/`, ceiling {CEILING_ONE_BASED_LOOPS}. The port is 0-based \
         everywhere except the ground-node convention and the 1-based user API \
         boundary — a new 1-based loop is a Pascal index escaping into the \
         engine."
    );

    let flat = hits(&root, &all, &["* nconds", "* ncond"]).len();
    assert!(
        flat <= CEILING_FLAT_OFFSET,
        "DE_PASCALIZE P8/P10/P11 metric regressed: {flat} flat-offset \
         multiplication(s), ceiling {CEILING_FLAT_OFFSET}. The offset is \
         computed inside the terminal/conductor views; call sites index through \
         them."
    );

    // Non-vacuity of both counts: the populations exist, so a walk or a filter
    // that stopped matching would show up here rather than as a free pass.
    assert!(
        one_based > 0 && flat > 0,
        "both audited populations read zero ({one_based}, {flat}) — that is a \
         broken instrument, not a finished cleanup. If they really were emptied, \
         delete this assertion together with the ceilings."
    );
}
