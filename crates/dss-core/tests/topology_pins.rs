//! **GOLDEN_REBASE G1.7 — the two expected-value pins behind the topology
//! settlements** (coordinator decisions **D15** and **D16**,
//! `tmp/g1/BRIEF_COMMON.md`).
//!
//! Both settlements write **zero** `tests/corpus/ledger.json` rows: instead of
//! excluding a field they state upstream's mechanism and make the live gate
//! assert it (`crates/dss-core/tests/harness/topology.rs`). CLAUDE.md's
//! discipline still applies — every divergence from an oracle channel is pinned
//! by an expected-value test naming BOTH numbers — and these two are those
//! tests.
//!
//! They live in their own binary rather than in the engine's `#[cfg(test)]`
//! tree because each one drives the **gate-side** code that carries the
//! settlement: `harness::topology::compare_topology` against a capture built
//! from the measured oracle reply, and `harness::topology::window_dedup`, the
//! gate's model of upstream's dedup. The engine-side twins (`Dss::topology_view`
//! itself, the port's per-pair rule, the `IsShuntElement` tree routing) are
//! pinned in `dss_core::exec::tests::topology`.
//!
//! Every oracle number below was measured on the channel named at the pin: the
//! vendored EPRI r4133 DLL through `crates/dss-epri`
//! (`tmp/g17/r4133_topo.json`, `tmp/g17/stale_r4133.json`) and the pinned
//! dss-python 0.15.7 (`tmp/g17/capi_topo_merged.json`), both read after a forced
//! `FreeTopology` where a fresh tree was wanted.

mod harness;

use std::panic::AssertUnwindSafe;
use std::path::PathBuf;

use dss_core::exec::Dss;
use harness::topology::{IsolationSnapshot, TopologyCap, compare_topology, window_dedup};

/// Compile a vendored corpus deck (never `.inputs/`, CLAUDE.md) exactly as the
/// corpus gate does (`corpus_gate/runner.rs::run_rust_capture`: `clear`, then
/// `compile`, then a `solve` per checkpoint).
///
/// Both decks used here write no files — `recloser_perm.dss` has no
/// `Show`/`Export`/`Plot` verb and the LVTestCase master's only verb is
/// `solve` — so no directory guard is needed.
fn compile_corpus_deck(rel: &str) -> Dss {
    let deck: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "corpus",
        rel,
    ]
    .iter()
    .collect();
    assert!(deck.is_file(), "vendored corpus deck missing: {deck:?}");
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!("compile \"{}\"", deck.display()));
    assert!(dss.errors().is_empty(), "{rel}: {:?}", dss.errors());
    dss
}

fn names(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_string()).collect()
}

fn lower(v: &[String]) -> Vec<String> {
    v.iter().map(|s| s.to_ascii_lowercase()).collect()
}

fn lower_pairs(v: &[(String, String)]) -> Vec<(String, String)> {
    v.iter()
        .map(|(a, b)| (a.to_ascii_lowercase(), b.to_ascii_lowercase()))
        .collect()
}

fn pairs(v: &[(&str, &str)]) -> Vec<(String, String)> {
    v.iter()
        .map(|(a, b)| ((*a).to_ascii_lowercase(), (*b).to_ascii_lowercase()))
        .collect()
}

/// Run `f` and return its panic message — the corruption drives below require a
/// specific failure, not merely "something failed".
fn panic_message(f: impl FnOnce()) -> String {
    let payload = std::panic::catch_unwind(AssertUnwindSafe(f)).expect_err("the arm must panic");
    if let Some(m) = payload.downcast_ref::<&str>() {
        (*m).to_string()
    } else if let Some(m) = payload.downcast_ref::<String>() {
        m.clone()
    } else {
        "<non-string panic payload>".to_string()
    }
}

/// **D15 pin — the port answers from a freshly built tree; the oracle answers
/// from the one it memoized at step 0, and the gate asserts exactly that.**
///
/// Deck: `controls:recloser/recloser_perm.dss` (`engines: "r4133"`, 24 steps) —
/// a 3-phase permanent fault drives FAST trip → reclose → SLOW trip → reclose →
/// lockout, so the recloser opens `Line.feed` terminal 1 and strands the lateral
/// and its load.
///
/// **Both numbers, measured (`tmp/g17/stale_r4133.json`, one fresh compile per
/// step, and `tmp/g17/r4133_topo.json` for the live reply):**
///
/// * the **oracle**, at every one of the 24 steps: `NumIsolatedBranches = 0`,
///   `NumIsolatedLoads = 0`, both name lists the `NONE` sentinel (`[]` after the
///   transports' S1 decode) — the answer its `Branch_List` computed at step 0,
///   which nothing invalidates on an open/close (r4133
///   `Common/Circuit.pas:2932-2950`, freed only in `Destroy` `:703` and
///   `DoResetMeterZones` `:2308`; capi identical);
/// * the **port**, from step 3 on (and again at 4-7, 9-18, 20-23 — 19 of the 24
///   steps): `NumIsolatedBranches = 2` (`Line.feed`, `Line.lat`) and
///   `NumIsolatedLoads = 1` (`Load.l`), because `Dss::topology_view` rebuilds
///   the tree on every call (CLAUDE.md: an upstream defect is never reproduced
///   in any lane).
///
/// The settlement (D15) is that the gate does not exclude those steps: it
/// asserts `oracle(k) == port(step 0)`, the memoization contract itself. This
/// pin drives the real comparator over all 24 steps against the measured
/// capture, then shows both ways it can still fail — an oracle that does not
/// answer the step-0 topology, and the same capture compared fresh.
#[test]
fn topology_reads_a_freshly_built_tree() {
    const CASE: &str = "controls:recloser/recloser_perm.dss";
    // The r4133 reply, identical at every step, after the transports'
    // `["NONE"] -> []` decode (`crates/dss-epri/src/capture.rs::topo_names`).
    let stale = TopologyCap {
        num_loops: 0,
        num_isolated_branches: 0,
        num_isolated_loads: 0,
        looped_pairs: vec![],
        isolated_branches: vec![],
        isolated_loads: vec![],
    };

    let mut dss = compile_corpus_deck("controls/recloser/recloser_perm.dss");
    let mut step0: Option<IsolationSnapshot> = None;
    let mut declined = Vec::new();
    for step in 0..24usize {
        dss.command("solve");
        let view = dss.topology_view();
        let moved = step0.as_ref().is_some_and(|b| {
            b.num_isolated_branches != view.num_isolated_branches
                || b.num_isolated_loads != view.num_isolated_loads
        });
        if moved {
            declined.push(step);
        }
        if step == 0 {
            // Step 0: the port's fresh answer IS the oracle's — this is the tree
            // upstream memoizes, and the comparison is an ordinary one.
            assert_eq!(
                (view.num_isolated_branches, view.num_isolated_loads),
                (0, 0),
                "step 0 must agree with the oracle's 0 / 0 before anything trips"
            );
        }
        compare_topology(
            &mut dss,
            &stale,
            &format!("{CASE} step {step}"),
            CASE,
            step,
            &mut step0,
        );
    }
    assert_eq!(
        declined,
        vec![
            3, 4, 5, 6, 7, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 20, 21, 22, 23
        ],
        "the 19 steps whose fresh topology differs from step 0 — the recloser is \
         closed again at 8 and 19, and the comparison there is an ordinary one \
         (this is the per-case half of `TOPOLOGY_STALE_DECLINES`)"
    );

    // Both numbers, at the last step: the port's fresh answer ...
    let view = dss.topology_view();
    assert_eq!(view.num_isolated_branches, 2, "port, step 23");
    assert_eq!(
        lower(&view.isolated_branches),
        names(&["line.feed", "line.lat"])
    );
    assert_eq!(view.num_isolated_loads, 1, "port, step 23");
    assert_eq!(lower(&view.isolated_loads), names(&["load.l"]));
    // ... against the oracle's memoized 0 / 0 / NONE / NONE.
    assert_eq!(
        (
            stale.num_isolated_branches,
            stale.num_isolated_loads,
            stale.isolated_branches.len(),
            stale.isolated_loads.len()
        ),
        (0, 0, 0, 0),
        "r4133 at step 23, measured"
    );

    // Teeth 1: the rebase is not a free pass — an oracle that answers something
    // OTHER than the port's step-0 topology still fails, and says which
    // contract broke.
    let moved_oracle = TopologyCap {
        num_isolated_branches: 1,
        isolated_branches: names(&["Line.lat"]),
        ..stale.clone()
    };
    let msg = panic_message(|| {
        compare_topology(
            &mut dss,
            &moved_oracle,
            &format!("{CASE} step 23"),
            CASE,
            23,
            &mut step0,
        );
    });
    assert!(msg.contains("NumIsolatedBranches differs"), "{msg}");
    assert!(msg.contains("memoized `Branch_List`"), "{msg}");

    // Teeth 2: the divergence is real. Had the port's topology NOT moved since
    // step 0 (the slot below says so), the very same capture would be compared
    // fresh — and red with the two numbers this pin exists to state.
    let mut as_if_unchanged = Some(IsolationSnapshot {
        num_isolated_branches: view.num_isolated_branches,
        num_isolated_loads: view.num_isolated_loads,
        isolated_branches: view.isolated_branches.clone(),
        isolated_loads: view.isolated_loads.clone(),
    });
    let msg = panic_message(|| {
        compare_topology(
            &mut dss,
            &stale,
            &format!("{CASE} step 23"),
            CASE,
            23,
            &mut as_if_unchanged,
        );
    });
    assert!(
        msg.contains("NumIsolatedBranches differs: Rust 2 vs oracle 0"),
        "{msg}"
    );

    // And the loop half is compared at EVERY step, declines or not: this deck is
    // radial on both engines, and a stale `num_loops` would red here.
    assert_eq!(view.num_loops, 0);
    assert_eq!(stale.num_loops, 0);
}

/// **D16 pin — the oracles' `AllLoopedPairs` is the *window scan* of the port's
/// own candidate sequence, and that scan drops real pairs.**
///
/// Upstream reduces its candidate sequence by scanning the flat name buffer in
/// overlapping windows (`i := i + 1` over `(TStr[i-1], TStr[i])`; r4133
/// `Version8/Source/DDLL/DTopology.pas:286-296`, capi 0.14.5
/// `CAPI_Topology.pas:180-190`) while the comment two lines above says "see if
/// we already found this pair" — so a genuinely new candidate that happens to
/// equal a *straddling* window `(b_j, a_{j+1})` is dropped. Intent contradicts
/// behaviour ⇒ upstream defect (coordinator D4/D16); the port keeps the correct
/// per-pair dedup and the live gate asserts `oracle.looped_pairs ==
/// window_dedup(port candidates)`.
///
/// **Both numbers, measured on the gating channels:**
///
/// * `controls:combo/midi_protection.dss` (`engines: "r4133"`) — 7 candidates,
///   port **5** pairs, r4133 **4**; the lost pair is
///   `(Transformer.rbank2, Line.rb_bb11)`, the window `(TStr[3], TStr[4])` at
///   the moment of its check (`tmp/g17/r4133_topo.json`).
/// * `solvable_now:…/LVTestCaseNorthAmerican/Master.dss` (`engines: "both"`) —
///   1105 candidates, port **1025** pairs, capi and r4133 **968** each, byte
///   identical to one another (`tmp/g17/capi_topo_merged.json`,
///   `tmp/g17/r4133_topo.json`); `NumLoops` 552 on all three.
///
/// In both cases the window scan applied to the port's own candidates returns
/// the oracle's list exactly — so the whole divergence is that one defect and
/// nothing else in the walk (order, `IsLoopedHere`, `LoopLineObj`) differs.
#[test]
fn looped_pairs_lose_the_straddling_window() {
    const CASE: &str = "controls:combo/midi_protection.dss";
    let mut dss = compile_corpus_deck("controls/combo/midi_protection.dss");
    dss.command("solve");
    let view = dss.topology_view();
    assert_eq!(view.looped_pair_candidates.len(), 7, "raw candidates");
    assert_eq!(view.looped_pairs.len(), 5, "the port's per-pair dedup");
    assert_eq!(view.num_loops, 3, "7 IsLoopedHere nodes div 2, r4133: 3");

    // r4133's reply, verbatim (flat, as both transports send it).
    let oracle_flat = names(&[
        "Line.bb5_6p",
        "Line.bb5_6",
        "Transformer.rbank3",
        "Transformer.rbank2",
        "Line.rb_bb11",
        "Line.bb10_11",
        "Transformer.rbank1",
        "Line.rb_bb11",
    ]);
    let cap = TopologyCap {
        num_loops: 3,
        num_isolated_branches: 1,
        num_isolated_loads: 0,
        looped_pairs: oracle_flat.clone(),
        isolated_branches: names(&["Line.tie"]),
        isolated_loads: vec![],
    };

    // The whole comparator, end to end, on the real engine: green — 4 oracle
    // pairs against 5 port pairs, because the arm compares the window model.
    let mut step0: Option<IsolationSnapshot> = None;
    compare_topology(
        &mut dss,
        &cap,
        &format!("{CASE} step 0"),
        CASE,
        0,
        &mut step0,
    );

    // Both numbers, and the pair upstream loses.
    let window = window_dedup(&view.looped_pair_candidates);
    assert_eq!(window.len(), 4, "the window scan of the port's candidates");
    assert_eq!(oracle_flat.len() / 2, 4, "r4133's own pair count");
    assert_eq!(view.looped_pairs.len(), 5, "the port's correct list");
    assert_eq!(
        lower_pairs(&window),
        pairs(&[
            ("Line.bb5_6p", "Line.bb5_6"),
            ("Transformer.rbank3", "Transformer.rbank2"),
            ("Line.rb_bb11", "Line.bb10_11"),
            ("Transformer.rbank1", "Line.rb_bb11"),
        ]),
        "the model reproduces r4133's list, pair for pair and in order"
    );
    let dropped = ("transformer.rbank2".to_string(), "line.rb_bb11".to_string());
    assert!(
        lower_pairs(&view.looped_pairs).contains(&dropped),
        "the port keeps the pair upstream drops"
    );
    assert!(
        !lower_pairs(&window).contains(&dropped),
        "the window scan drops it — it is the straddling window (TStr[3], TStr[4])"
    );

    // Teeth: the arm is a full list compare against the model, so an oracle list
    // that is NOT that reduction reds — including one that merely lost one more
    // pair (which is what "upstream dedups a bit more" would look like).
    let mut short = cap.clone();
    short.looped_pairs.truncate(6);
    let msg = panic_message(|| {
        let mut slot: Option<IsolationSnapshot> = None;
        compare_topology(
            &mut dss,
            &short,
            &format!("{CASE} step 0"),
            CASE,
            0,
            &mut slot,
        );
    });
    assert!(msg.contains("`looped_pairs` length differs"), "{msg}");
    assert!(msg.contains("D16"), "{msg}");
    let mut swapped = cap.clone();
    swapped.looped_pairs.swap(0, 2);
    let msg = panic_message(|| {
        let mut slot: Option<IsolationSnapshot> = None;
        compare_topology(
            &mut dss,
            &swapped,
            &format!("{CASE} step 0"),
            CASE,
            0,
            &mut slot,
        );
    });
    assert!(msg.contains("`looped_pairs`[0] differs"), "{msg}");

    // --- The population-scale witness, on a `both`-gated case -----------------
    let mut dss = compile_corpus_deck(
        "electricdss-tst/Version8/Distrib/IEEETestCases/LVTestCaseNorthAmerican/Master.dss",
    );
    let view = dss.topology_view();
    assert_eq!(view.looped_pair_candidates.len(), 1105, "raw candidates");
    assert_eq!(view.looped_pairs.len(), 1025, "the port's per-pair dedup");
    assert_eq!(view.num_loops, 552, "capi and r4133 both measure 552");
    let window = window_dedup(&view.looped_pair_candidates);
    assert_eq!(
        window.len(),
        968,
        "capi and r4133 both report 968 pairs (1936 flat entries)"
    );
    // Order too, not only the count: the oracles' first three and last three
    // pairs, byte identical on the two channels.
    assert_eq!(
        lower_pairs(&window[..3]),
        pairs(&[
            ("Line.238_6", "Line.238_5"),
            ("Line.239_6", "Line.239_5"),
            ("Line.240_6", "Line.240_5"),
        ])
    );
    assert_eq!(
        lower_pairs(&window[window.len() - 3..]),
        pairs(&[
            ("Line.237_3", "Line.237_6"),
            ("Line.237_2", "Line.237_6"),
            ("Line.237", "Line.237_6"),
        ])
    );
    assert_eq!(
        view.looped_pairs.len() - window.len(),
        57,
        "the defect costs 57 real looped pairs on this feeder"
    );
}
