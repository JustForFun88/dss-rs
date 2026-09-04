//! Source-order gate for the live gate's per-step capture
//! (`GOLDEN_REBASE_PLAN.md` §1.1(a), coordinator decision D3).
//!
//! The oracle reads inside one solved step are **not** interchangeable. They
//! partition into three groups:
//!
//! * **A** — cache-aware quantities that go through `ComputeIterminal`
//!   (`Powers`, `TotalPowers`, `Losses`, `PhaseLosses`, and every `Circuit`
//!   aggregate, which is a `Get_Losses`/`Get_Power` over the list it walks);
//! * **B** — reads that fill a scratch buffer via `GetCurrents` (`Currents`,
//!   `CurrentsMagAng`, `SeqCurrents`, `CplxSeqCurrents`, `SeqPowers`,
//!   `Residuals`);
//! * **C** — order-free reads (node voltages, the discrete control state, the
//!   plain `Solution` scalars).
//!
//! Group A must be read before group B. The rule is empirical, not stylistic:
//! it is the harmonics stale-`Iterminal` ordering CLAUDE.md records (a `Powers`
//! read after a `Currents` read returns the previous iteration's current on a
//! Thevenin-DER element), and both transports of the corpus gate must obey it
//! identically or the two channels would capture different numbers from the
//! same engine state.
//!
//! G1.9's circuit aggregates are group A, and they carry a second, independent
//! ordering constraint: each one walks a `TPointerList`
//! (`PDElements`/`Lines`/`Transformers`/`Sources`/`CktElements`) to exhaustion
//! and leaves its cursor at the end, while the discrete capture drives
//! `Transformers.First/Next` of its own. Reading the aggregates before any
//! `First/Next` walk removes that interaction by construction.
//!
//! A comment can be edited away; this file cannot. It reads the two capture
//! sources and asserts the call order inside their `run_case`, with no engine
//! and no oracle.
//!
//! *(D3 assigns the canonical home of this gate to G1.3a's lane. If that file
//! lands on the merged tree, fold these two cases into it and delete this one —
//! noted as "dedup at merge" in the G1.9 handoff.)*

use std::fs;
use std::path::PathBuf;

/// The four call sites the gate locates inside one transport's `run_case`.
struct Anchors {
    /// Start of the per-step capture function itself.
    run: &'static str,
    /// The G1.9 group-A read.
    aggregates: &'static str,
    /// The per-element capture — the group-B (`Currents`) read.
    elements: &'static str,
    /// The discrete-state capture, which drives `Transformers.First/Next`.
    discrete: &'static str,
}

fn repo_root() -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "..", ".."].iter().collect()
}

fn read(rel: &str) -> String {
    let path = repo_root().join(rel);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// Byte offset of `needle` at or after `from`, or an error naming what was
/// looked for — a renamed helper must fail loudly, never turn the comparisons
/// below into a vacuous `0 < 0`.
fn offset_after(hay: &str, needle: &str, from: usize, rel: &str) -> Result<usize, String> {
    hay[from..].find(needle).map(|i| from + i).ok_or_else(|| {
        format!(
            "{rel}: the capture-order gate could not find `{needle}`. If the call was \
             renamed, update this gate — do not delete the ordering it protects \
             (GOLDEN_REBASE_PLAN.md §1.1(a))."
        )
    })
}

/// The whole gate as a pure function of one transport's source text, so the
/// rule itself can be shown to have teeth
/// ([`the_gate_rejects_a_swapped_or_renamed_capture`]).
fn check_order(src: &str, a: &Anchors, rel: &str) -> Result<(), String> {
    let run = offset_after(src, a.run, 0, rel)?;
    let aggregates = offset_after(src, a.aggregates, run, rel)?;
    let elements = offset_after(src, a.elements, run, rel)?;
    let discrete = offset_after(src, a.discrete, run, rel)?;

    if aggregates >= elements {
        return Err(format!(
            "{rel}: the G1.9 aggregates (byte {aggregates}) are read AFTER the element \
             capture (byte {elements}), whose `Currents` read is group B — group A must \
             come first"
        ));
    }
    if aggregates >= discrete {
        return Err(format!(
            "{rel}: the G1.9 aggregates (byte {aggregates}) are read AFTER the discrete \
             capture (byte {discrete}), which drives Transformers.First/Next — the \
             aggregates must precede every First/Next walk"
        ));
    }
    Ok(())
}

/// The pinned dss-python transport (`capi_v0145` channel).
///
/// `capture_all_elements` reaches `Currents` through `gc.capture_element`
/// (`tools/golden/gen_checkpoints.py`, Powers then Currents).
const CAPI: Anchors = Anchors {
    run: "def run_case(",
    aggregates: "capture_aggregates(ckt",
    elements: "capture_all_elements(ckt",
    discrete: "gc.capture_discrete(ckt",
};

/// The EPRI r4133 transport (`r4133` channel), which must capture in the exact
/// same order or the two channels would not be comparing the same reads.
///
/// `capture_all_elements` reaches `Currents` through `Engine::element_pcl`
/// (powers, then currents, then losses).
const R4133: Anchors = Anchors {
    run: "pub fn run_case(",
    aggregates: "capture_aggregates(engine",
    elements: "capture_all_elements(engine",
    discrete: "capture_discrete(engine",
};

#[test]
fn capi_capture_reads_the_aggregates_before_any_currents_read() {
    let rel = "tools/oracle/oracle_server.py";
    check_order(&read(rel), &CAPI, rel).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn r4133_capture_reads_the_aggregates_before_any_currents_read() {
    let rel = "crates/dss-epri/src/capture.rs";
    check_order(&read(rel), &R4133, rel).unwrap_or_else(|e| panic!("{e}"));
}

/// Non-vacuity (§1.1(f)): the two gates above pass on the real sources, so this
/// one shows the rule they apply is not trivially satisfiable. Same predicate,
/// synthetic sources — an in-order one is accepted; the same text with the
/// aggregates moved after the element capture, after the discrete capture, or
/// with the aggregate call renamed away, is rejected.
#[test]
fn the_gate_rejects_a_swapped_or_renamed_capture() {
    let a = Anchors {
        run: "fn run_case(",
        aggregates: "capture_aggregates(",
        elements: "capture_all_elements(",
        discrete: "capture_discrete(",
    };
    let ok = "fn run_case( capture_aggregates(x); capture_discrete(x); capture_all_elements(x);";
    assert!(check_order(ok, &a, "synthetic").is_ok());

    let after_elements =
        "fn run_case( capture_all_elements(x); capture_discrete(x); capture_aggregates(x);";
    let err = check_order(after_elements, &a, "synthetic").expect_err("group B ran first");
    assert!(err.contains("element capture"), "{err}");

    let after_discrete =
        "fn run_case( capture_discrete(x); capture_aggregates(x); capture_all_elements(x);";
    let err = check_order(after_discrete, &a, "synthetic").expect_err("the walk ran first");
    assert!(err.contains("Transformers.First/Next"), "{err}");

    let renamed = "fn run_case( grab_the_aggregates(x); capture_discrete(x);                    capture_all_elements(x);";
    let err = check_order(renamed, &a, "synthetic").expect_err("the anchor is gone");
    assert!(err.contains("could not find"), "{err}");
}
