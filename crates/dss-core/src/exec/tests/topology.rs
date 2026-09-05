//! GOLDEN_REBASE G1.7 — in-engine pins for [`Dss::topology_view`], the six
//! order-free `ITopology` quantities the live corpus gate compares
//! (`NumLoops` / `NumIsolatedBranches` / `NumIsolatedLoads` and their three name
//! lists; r4133 `Version8/Source/DDLL/DTopology.pas` `TopologyI` modes 0-2 and
//! `TopologyV` modes 0-2, capi 0.14.5 `CAPI_Topology.pas`).
//!
//! Every expected value below was measured on BOTH gating oracles — the pinned
//! dss-python 0.15.7 (`tmp/g17/capi_topo_merged.json`, 422/422 capi-gating cases)
//! and the vendored r4133 DLL (`tmp/g17/r4133_topo.json`) — at the *fresh* tree,
//! i.e. after a forced `FreeTopology` (`calcvoltagebases` ->
//! `Common/Solution.pas:2550` -> `DoResetMeterZones` -> `FreeTopology`), which is
//! the state the port answers from on every call. The two channels agreed byte
//! for byte on all six decks used here.

use crate::exec::*;

/// Compile a corpus deck by absolute path and return the live engine.
///
/// The decks read below write no files (the `Show`/`Export` lines of
/// `IEEE13Nodeckt.dss` are commented out; the three synthetic decks have none),
/// so no directory guard is needed. Never `.inputs/` — the vendored corpus tree
/// is the one the live gate reads (CLAUDE.md).
fn compile_corpus_deck(rel: &str) -> Dss {
    let deck = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus")
        .join(rel);
    assert!(deck.is_file(), "corpus deck missing: {deck:?}");
    let mut dss = Dss::new();
    dss.command(&format!("compile \"{}\"", deck.display()));
    assert!(dss.errors().is_empty(), "{rel}: {:?}", dss.errors());
    dss
}

/// Lower-case a name list for comparison — the harness convention (DSS
/// identifiers are case-insensitive; the oracles render `QualifiedName` /
/// `FullName` from the stored spelling).
fn lower(v: &[String]) -> Vec<String> {
    v.iter().map(|s| s.to_ascii_lowercase()).collect()
}

/// Lower-case a `(branch, loops-onto)` pair list.
fn lower_pairs(v: &[(String, String)]) -> Vec<(String, String)> {
    v.iter()
        .map(|(a, b)| (a.to_ascii_lowercase(), b.to_ascii_lowercase()))
        .collect()
}

/// **Pin 1 — `NumLoops` is the `IsLoopedHere` count HALVED, not the pair count.**
///
/// r4133 `DTopology.pas:67-77` counts every tree node whose `IsLoopedHere` is set
/// and returns `Result div 2` (`:77`; capi `CAPI_Topology.pas:81-98` identical),
/// while `AllLoopedPairs` (`:271-321`) deduplicates by identity — so the two are
/// independent numbers. IEEE13 is the witness both oracles measured: **3** pairs
/// and **1** loop (three `IsLoopedHere` nodes, `3 div 2 = 1`). A port that
/// returned `looped_pairs.len()`, or that skipped the halving, would report 3.
#[test]
fn num_loops_is_the_looped_here_count_halved() {
    let mut dss = compile_corpus_deck(
        "electricdss-tst/Version8/Distrib/IEEETestCases/13Bus/IEEE13Nodeckt.dss",
    );
    let t = dss.topology_view();
    assert_eq!(
        lower_pairs(&t.looped_pairs),
        vec![
            (
                "transformer.reg3".to_string(),
                "transformer.reg2".to_string()
            ),
            ("transformer.reg2".to_string(), "line.650632".to_string()),
            ("transformer.reg1".to_string(), "line.650632".to_string()),
        ],
        "IEEE13 looped pairs (both oracles, tree-walk order)"
    );
    assert_eq!(t.looped_pairs.len(), 3, "three deduplicated pairs");
    assert_eq!(t.num_loops, 1, "3 IsLoopedHere nodes div 2");
    // The deck is loop-carrying at all (guards against a vacuous 0 == 0 pass).
    assert!(t.num_loops > 0 && !t.looped_pairs.is_empty());
}

/// **Pin 2 — a pair is recorded once, in either orientation.**
///
/// `modes/reduce/reduce_laterals.dss` closes exactly one loop: the 1-phase
/// laterals `lat2a` (b2.3 -> x2.3) and `lat2b` (b2.1 -> x2.1) meet at bus `x2`, so
/// the tree marks `IsLoopedHere` at both ends and offers the candidate pair in
/// both orientations. Both oracles report **1** pair `(Line.lat2b, Line.lat2a)`
/// and `NumLoops = 1`; since `NumLoops = IsLoopedHere div 2 = 1` there were at
/// least **two** looped nodes, so at least one candidate WAS rejected as a
/// duplicate — without the both-orientation test (r4133 `DTopology.pas:286-296`,
/// capi `CAPI_Topology.pas:180-190`) the list would carry the reverse as well.
#[test]
fn looped_pairs_are_deduped_in_both_orientations() {
    let mut dss = compile_corpus_deck("modes/reduce/reduce_laterals.dss");
    let t = dss.topology_view();
    assert_eq!(
        lower_pairs(&t.looped_pairs),
        vec![("line.lat2b".to_string(), "line.lat2a".to_string())],
        "one pair, the walk's orientation"
    );
    assert_eq!(t.num_loops, 1, "two looped tree nodes, halved");
    // The reverse orientation must not appear a second time.
    assert!(
        !t.looped_pairs
            .iter()
            .any(|(a, b)| a.eq_ignore_ascii_case("Line.lat2a")
                && b.eq_ignore_ascii_case("Line.lat2b")),
        "the reverse orientation is a duplicate, not a second pair: {:?}",
        t.looped_pairs
    );
    // 2*num_loops (= at most the looped-node count) exceeds the pair count: the
    // dedup demonstrably fired on this deck.
    assert!(
        2 * t.num_loops as usize > t.looped_pairs.len(),
        "dedup must have rejected a candidate"
    );
}

/// **Pin 3 — `AllIsolatedBranches` walks `PDElements` in creation order.**
///
/// r4133 `DTopology.pas:322-356` (count `:79-88`) walks `ActiveCircuit.PDElements`
/// with `.First`/`.Next` and keeps the elements the tree never reached; capi
/// `CAPI_Topology.pas:114-151` / `:302-316` iterate the same list. The witness is
/// `asymmetric/gic/gic_midi.dss`, whose isolated set spans three PD classes and
/// whose measured order (both oracles) is the deck's **creation** order, not an
/// alphabetical or class-grouped one — `Line.seg45` precedes `GICTransformer.tg3`
/// although `G` sorts before `L`. `tg1` and `gg1` sit on the source bus `b1` and
/// are reached, so they are absent: the filter is proven in both directions.
#[test]
fn isolated_branches_walk_the_pd_list_in_creation_order() {
    let mut dss = compile_corpus_deck("asymmetric/gic/gic_midi.dss");
    let t = dss.topology_view();
    assert_eq!(
        lower(&t.isolated_branches),
        vec![
            "line.seg45",
            "line.seg56",
            "gictransformer.tg3",
            "gictransformer.tg5",
            "reactor.gg3",
            "reactor.gg5",
            "reactor.g2",
            "reactor.g4",
            "reactor.g6",
        ],
        "gic_midi isolated PD elements (both oracles, creation order)"
    );
    assert_eq!(t.num_isolated_branches, 9);
    assert_eq!(
        t.num_isolated_branches as usize,
        t.isolated_branches.len(),
        "count and list come from one filter"
    );
    // Reached elements are excluded (the deck HAS a tg1/gg1 on the source bus).
    assert!(
        !t.isolated_branches
            .iter()
            .any(|n| n.eq_ignore_ascii_case("GICTransformer.tg1")
                || n.eq_ignore_ascii_case("Reactor.gg1")),
        "source-bus elements are reached: {:?}",
        t.isolated_branches
    );
    // The isolated PC half of the same deck: the three stranded GICLines.
    assert_eq!(
        lower(&t.isolated_loads),
        vec!["gicline.gl23", "gicline.gl34", "gicline.gl61"],
        "gic_midi isolated PC elements"
    );
}

/// **Pin 4 — `AllIsolatedLoads` walks `PCElements`, stranded only.**
///
/// r4133 `DTopology.pas:357-392` (count `:89-98`; capi `CAPI_Topology.pas:369-405`
/// / `:448-462`) runs the isolated filter over `PCElements`.
/// `modes/reduce/reduce_remove.dss` ends with `remove line.l2 keepload=yes`, which
/// disables `l2`, `l3` and everything below `b2`; both oracles then report the
/// stranded PD pair `[Line.l2, Line.l3]` and the stranded loads
/// `[Load.ld3, Load.ld4]`, while `Load.ld2` (still on the energised `b2`) and the
/// `Remove`-created equivalent load at `b2` are reached and absent.
#[test]
fn isolated_loads_walk_the_pc_list() {
    let mut dss = compile_corpus_deck("modes/reduce/reduce_remove.dss");
    let t = dss.topology_view();
    assert_eq!(
        lower(&t.isolated_loads),
        vec!["load.ld3", "load.ld4"],
        "reduce_remove isolated PC elements (both oracles)"
    );
    assert_eq!(t.num_isolated_loads, 2);
    assert_eq!(
        lower(&t.isolated_branches),
        vec!["line.l2", "line.l3"],
        "reduce_remove isolated PD elements"
    );
    assert_eq!(t.num_isolated_branches, 2);
    // A reached load must not appear — the deck keeps `ld2` on the live bus b2,
    // and `Remove keepload=yes` creates an equivalent load there too.
    assert!(
        !t.isolated_loads
            .iter()
            .any(|n| n.eq_ignore_ascii_case("Load.ld2")),
        "the load on the energised bus is reached: {:?}",
        t.isolated_loads
    );
    assert!(t.num_loops == 0 && t.looped_pairs.is_empty(), "radial deck");
}

/// **Pin 5 — the port answers from the PRESENT conductor state (no memoization).**
///
/// Both oracles memoize `Branch_List` (r4133 `Common/Circuit.pas:2932-2950`; freed
/// only in `Destroy` `:703` and `DoResetMeterZones` `:2308`) and nothing on the
/// `Open` path invalidates it, so after `Open Line.650632 1` + `solve` they still
/// answer from the pre-trip tree: **`NumIsolatedBranches = 0`,
/// `NumIsolatedLoads = 0`** on IEEE13. Forcing `FreeTopology` through
/// `calcvoltagebases` on the very same state then yields **15 and 15** (measured
/// with the pinned dss-python by `tmp/g17/control_capi.py`, GOLDEN_REBASE G1.7
/// spec §3.3). The port caches nothing, so it reports the correct 15/15 at once —
/// the upstream staleness is never reproduced (CLAUDE.md; the corpus-side
/// settlement is coordinator decision D15).
#[test]
fn an_open_conductor_isolates_the_downstream_branch() {
    let mut dss = compile_corpus_deck(
        "electricdss-tst/Version8/Distrib/IEEETestCases/13Bus/IEEE13Nodeckt.dss",
    );
    let before = dss.topology_view();
    assert_eq!(
        (before.num_isolated_branches, before.num_isolated_loads),
        (0, 0),
        "the intact feeder strands nothing"
    );

    dss.command("open line.650632 1");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    let after = dss.topology_view();
    assert_eq!(
        (after.num_isolated_branches, after.num_isolated_loads),
        (15, 15),
        "everything below the opened conductor is stranded (oracle, fresh tree)"
    );
    assert_eq!(
        after.num_isolated_branches as usize,
        after.isolated_branches.len()
    );
    assert_eq!(
        after.num_isolated_loads as usize,
        after.isolated_loads.len()
    );
    // The opened line itself and one of the stranded loads, by name.
    assert!(
        after
            .isolated_branches
            .iter()
            .any(|n| n.eq_ignore_ascii_case("Line.650632")),
        "{:?}",
        after.isolated_branches
    );
    assert!(
        after
            .isolated_loads
            .iter()
            .any(|n| n.eq_ignore_ascii_case("Load.671")),
        "{:?}",
        after.isolated_loads
    );

    // Closing it back restores the intact answer in the same session — proof the
    // change is state-driven, not a one-way latch.
    dss.command("close line.650632 1");
    dss.command("solve");
    let reclosed = dss.topology_view();
    assert_eq!(
        (reclosed.num_isolated_branches, reclosed.num_isolated_loads),
        (0, 0)
    );
}

/// **Pin 6 — reading the topology does not disturb the active element.**
///
/// The six quantities in [`TopologyView`] are the *order-free* `ITopology` reads:
/// unlike `ActiveBranch` / `ActiveLevel` / `BranchName` and the `First`/`Next`
/// cursor modes (r4133 `DTopology.pas:42-53`, `:96-160`, `:170-186`), none of them
/// reassigns `ActiveCktElement`. The port must match, or a checkpoint that reads
/// topology beside the per-element capture would silently re-target the element
/// reads around it.
#[test]
fn topology_view_does_not_disturb_the_active_element() {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.t basekv=12.47 phases=3 bus1=src mvasc3=20000 mvasc1=21000");
    dss.command(
        "new line.l1 bus1=src bus2=b length=1 units=km r1=0.1 x1=0.3 r0=0.3 x0=0.9 c1=0 c0=0",
    );
    dss.command("new load.ld bus1=b phases=3 kv=12.47 kw=500 pf=0.95 model=1");
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    dss.command("select load.ld 1");
    let selected = dss.active_ckt_element.expect("an element is selected");
    let before = dss.snapshot_elements();

    let t = dss.topology_view();
    assert_eq!(t.num_loops, 0, "radial deck");
    assert!(t.isolated_branches.is_empty() && t.isolated_loads.is_empty());

    assert_eq!(
        dss.active_ckt_element,
        Some(selected),
        "topology_view must not re-target the active element"
    );
    let after = dss.snapshot_elements();
    assert_eq!(before.len(), after.len());
    for (b, a) in before.iter().zip(after.iter()) {
        assert_eq!(b.name, a.name);
        assert_eq!(b.currents, a.currents, "{} currents moved", b.name);
        assert_eq!(b.powers, a.powers, "{} powers moved", b.name);
    }
}

/// Upstream's looped-pair dedup, applied verbatim to a candidate sequence.
///
/// r4133 `Version8/Source/DDLL/DTopology.pas:277-301` (`k := -1`, the scan at
/// `:286-296`, the append at `:297-301`) and capi 0.14.5
/// `CAPI_Topology.pas:169-195` are character for character the same: the
/// accepted pairs live in ONE flat `TStr` buffer `[a0, b0, a1, b1, ...]`, and the
/// "see if we already found this pair" scan runs `i := 1; while (i <= k); ...;
/// i := i + 1` over `(TStr[i-1], TStr[i])` — stepping by **one**, so it tests
/// every *overlapping* window rather than every stored pair, and a new candidate
/// that happens to equal a straddling window `(b_j, a_{j+1})` is dropped.
///
/// The port does not reproduce that (CLAUDE.md; [`Dss::topology_view`]); this
/// helper is the gate's *model of the oracle*, so it is written to mirror the
/// Pascal indices literally (`k = buf.len() - 1`, hence `-1` on the empty
/// buffer, which is why the first candidate is always accepted).
fn window_dedup(candidates: &[(String, String)]) -> Vec<(String, String)> {
    let mut buf: Vec<String> = Vec::new();
    for (a, b) in candidates {
        let mut found = false;
        let k = buf.len() as isize - 1;
        let mut i: isize = 1;
        while i <= k && !found {
            let (p, q) = (&buf[(i - 1) as usize], &buf[i as usize]);
            if p == a && q == b {
                found = true;
            }
            if p == b && q == a {
                found = true;
            }
            i += 1;
        }
        if !found {
            buf.push(a.clone());
            buf.push(b.clone());
        }
    }
    buf.chunks(2)
        .map(|w| (w[0].clone(), w[1].clone()))
        .collect()
}

/// Build a `(String, String)` sequence from string literals.
fn pairs(v: &[(&str, &str)]) -> Vec<(String, String)> {
    v.iter()
        .map(|(a, b)| ((*a).to_string(), (*b).to_string()))
        .collect()
}

/// **Pin 7a — the mechanism: upstream's scan drops a pair that straddles two
/// stored pairs.**
///
/// The smallest sequence that exhibits it. After `(a,b)` and `(c,d)` are stored
/// the buffer is `[a, b, c, d]`; the third candidate `(b, c)` is a pair nobody
/// found, but it *is* the window `(TStr[1], TStr[2])`, so `:286-296` reports
/// `found` and drops it. The port's per-pair rule keeps it. This is the whole of
/// coordinator decision D16 in four lines, and it fails the moment
/// [`window_dedup`] stops mirroring the Pascal indices.
#[test]
fn the_window_scan_drops_a_straddling_pair() {
    let cands = pairs(&[("a", "b"), ("c", "d"), ("b", "c")]);
    assert_eq!(
        window_dedup(&cands),
        pairs(&[("a", "b"), ("c", "d")]),
        "the straddling window (TStr[1], TStr[2]) swallows the third pair"
    );
    // A genuine repeat is dropped by both rules, in either orientation — so the
    // helper is not simply "drop the third candidate".
    assert_eq!(
        window_dedup(&pairs(&[("a", "b"), ("c", "d"), ("d", "c")])),
        pairs(&[("a", "b"), ("c", "d")])
    );
    // ... and a candidate that matches no window at all survives.
    assert_eq!(
        window_dedup(&pairs(&[("a", "b"), ("c", "d"), ("a", "d")])),
        pairs(&[("a", "b"), ("c", "d"), ("a", "d")])
    );
}

/// **Pin 7b — the oracles' `AllLoopedPairs` IS the window scan of the port's own
/// candidate sequence** (coordinator decision D16, the positive form the live
/// gate asserts).
///
/// The port keeps the correct, larger list; upstream's window scan
/// ([`window_dedup`], r4133 `DTopology.pas:286-296`, capi
/// `CAPI_Topology.pas:180-190`) applied to
/// [`TopologyView::looped_pair_candidates`] reproduces the oracle exactly — so
/// the divergence is fully explained by that one defect and nothing else in the
/// port's walk (order, `IsLoopedHere`, `LoopLineObj`) differs from upstream.
///
/// Both numbers, measured on the oracles (`tmp/g17/capi_topo_merged.json`,
/// `tmp/g17/r4133_topo.json`) and reproduced here:
///
/// * `controls/combo/midi_protection.dss` (r4133-gated) — 7 candidates, port
///   **5** pairs, window **4** = r4133's four pairs verbatim; `NumLoops` 3 on
///   both (`7 div 2`).
/// * `IEEETestCases/LVTestCaseNorthAmerican/Master.dss` (both channels, byte
///   identical) — 1105 candidates, port **1025** pairs, window **968** = both
///   oracles' 968; `NumLoops` 552 on all three.
#[test]
fn the_oracle_pair_list_is_the_window_scan_of_the_candidates() {
    // --- midi_protection: the whole list is small enough to pin literally. ---
    let mut dss = compile_corpus_deck("controls/combo/midi_protection.dss");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let t = dss.topology_view();
    assert_eq!(t.looped_pair_candidates.len(), 7, "raw candidates");
    assert_eq!(t.looped_pairs.len(), 5, "the port's per-pair dedup");
    assert_eq!(t.num_loops, 3, "7 IsLoopedHere nodes div 2 (r4133: 3)");
    // r4133 `Topology.AllLoopedPairs`, flat, as captured through the DDLL API.
    let r4133 = pairs(&[
        ("Line.bb5_6p", "Line.bb5_6"),
        ("Transformer.rbank3", "Transformer.rbank2"),
        ("Line.rb_bb11", "Line.bb10_11"),
        ("Transformer.rbank1", "Line.rb_bb11"),
    ]);
    assert_eq!(
        lower_pairs(&window_dedup(&t.looped_pair_candidates)),
        lower_pairs(&r4133),
        "the window scan of the port's candidates == r4133's AllLoopedPairs"
    );
    // The one pair upstream loses, and WHY: it is the window (TStr[3], TStr[4])
    // straddling the second and third stored pairs at the moment of the check.
    let dropped = ("Transformer.rbank2".to_string(), "Line.rb_bb11".to_string());
    assert!(
        t.looped_pairs.contains(&dropped),
        "the port keeps the pair upstream drops"
    );
    assert!(
        !r4133.contains(&dropped),
        "r4133 does not report it (the straddling window)"
    );
    assert_eq!(
        t.looped_pairs.len() - window_dedup(&t.looped_pair_candidates).len(),
        1,
        "exactly one pair is lost on this deck"
    );

    // --- LVTestCase North American: the population-scale witness (both
    // channels agreed byte for byte on all 968 pairs).
    let mut dss = compile_corpus_deck(
        "electricdss-tst/Version8/Distrib/IEEETestCases/LVTestCaseNorthAmerican/Master.dss",
    );
    let t = dss.topology_view();
    assert_eq!(t.looped_pair_candidates.len(), 1105, "raw candidates");
    assert_eq!(t.looped_pairs.len(), 1025, "the port's per-pair dedup");
    assert_eq!(t.num_loops, 552, "both oracles measure 552");
    let w = window_dedup(&t.looped_pair_candidates);
    assert_eq!(w.len(), 968, "both oracles measure 968 pairs");
    // Order too, not just the count: the oracles' first three and last three.
    assert_eq!(
        lower_pairs(&w[..3]),
        lower_pairs(&pairs(&[
            ("Line.238_6", "Line.238_5"),
            ("Line.239_6", "Line.239_5"),
            ("Line.240_6", "Line.240_5"),
        ]))
    );
    assert_eq!(
        lower_pairs(&w[965..]),
        lower_pairs(&pairs(&[
            ("Line.237_3", "Line.237_6"),
            ("Line.237_2", "Line.237_6"),
            ("Line.237", "Line.237_6"),
        ]))
    );
    // Non-vacuity: the two reductions are strict and distinct on this deck, so
    // neither `window_dedup` nor the port's rule can pass as the identity.
    assert!(t.looped_pair_candidates.len() > t.looped_pairs.len());
    assert!(t.looped_pairs.len() > w.len());
}

/// **Pin 8 — a GICTransformer is a tree BRANCH, not a shunt object, and closes a
/// loop when both of its terminals land on one bus.**
///
/// The port used to route it by `CktElement::is_shunt` (`TPDElement.IsShunt`,
/// which the GICTransformer pins true in its constructor and both bus setters,
/// r4133 `PDElements/GICTransformer.pas:445` / `:217` / `:254`) and so put it on
/// the PC adjacency list as a shunt object. Upstream routes by `IsShuntElement`
/// (capi `Shared/CktTree.pas:522-528`, r4133 `Common/Utilities.pas:1262-1274`),
/// which answers `TRUE` only for a Capacitor or a Reactor; a `PD_ELEMENT`
/// GICTransformer (`GICTransformer.pas:95`) is bucketed by ALL its terminals
/// (`CktTree.pas:660-673`) and becomes a child branch
/// (`FindAllChildBranches`, `:566`). Fixed in
/// [`crate::circuit::ckt_tree::is_shunt_element`] and its three call sites.
///
/// The fixture moved at the merge into `update`: G1.4a's coordinator decision
/// **D14** split the single `new gictransformer.gt` line out of
/// `modes/makeposseq/makeposseq_shunt.dss` into the `r4133`-gated
/// `modes/makeposseq/makeposseq_gic.dss` (capi 0.14.5 is nondeterministic on any
/// deck that instantiates one — D12), so the pin now drives BOTH halves of the
/// class switch, one deck each.
///
/// (a) `makeposseq_gic.dss` puts `GICTransformer.gt` on `b1` (`busH=b1`,
/// `busNH=b1.4.4.4`) behind `Line.feed`, so the transformer's own second terminal
/// returns to a bus the tree has already checked. Measured on the r4133 DLL
/// one-shot (`tmp/merge/lane-s-G17/probe_gic.py`, identical after the deck and
/// after an extra solve): `NumLoops` **1**, `AllLoopedPairs`
/// `[Line.feed, GICTransformer.gt]`. Routed as a shunt the element leaves the
/// tree entirely and the deck has **0** loops — which is what the port answered
/// before the fix.
///
/// (b) `makeposseq_shunt.dss` is now the converse guard, and a sharper one than
/// it was: with the GICTransformer gone its three Capacitors and four Reactors
/// are the only elements with both terminals on one bus, and r4133 measures
/// `NumLoops` **0** with no pairs at all (same probe; the deck is capi-gated in
/// the live corpus gate, which compares the same six rows every run). If the
/// class switch were dropped in the other direction every one of those seven
/// would become a branch and contribute candidates.
#[test]
fn a_gictransformer_is_a_tree_branch_and_can_close_a_loop() {
    // (a) The branch half — the deck that carries the GICTransformer.
    let mut dss = compile_corpus_deck("modes/makeposseq/makeposseq_gic.dss");
    let t = dss.topology_view();
    assert_eq!(t.num_loops, 1, "r4133 measures NumLoops = 1");
    assert_eq!(
        lower_pairs(&t.looped_pairs),
        lower_pairs(&pairs(&[("Line.feed", "GICTransformer.gt")])),
        "r4133 measures exactly this pair"
    );
    // Both orientations reach the candidate list; the window scan folds them onto
    // the single pair the oracle reports, with no straddling window to lose.
    assert_eq!(t.looped_pair_candidates.len(), 2);
    assert_eq!(
        lower_pairs(&window_dedup(&t.looped_pair_candidates)),
        lower_pairs(&pairs(&[("Line.feed", "GICTransformer.gt")])),
        "the port and r4133 agree exactly"
    );
    assert_eq!(t.num_isolated_branches, 0);
    assert_eq!(t.num_isolated_loads, 0);

    // (b) The converse half — the shunt deck the GICTransformer was split out of
    // still carries seven shunt Capacitor / Reactor elements, several of them with
    // both terminals on one bus, and not one of them may become a branch (that is
    // the class switch, not the flag).
    let mut dss = compile_corpus_deck("modes/makeposseq/makeposseq_shunt.dss");
    let t = dss.topology_view();
    assert_eq!(t.num_loops, 0, "both oracles measure NumLoops = 0");
    assert!(t.looped_pairs.is_empty(), "{:?}", t.looped_pairs);
    assert!(
        t.looped_pair_candidates.is_empty(),
        "a shunt Capacitor/Reactor must never be a tree branch: {:?}",
        t.looped_pair_candidates
    );
    let shunts = dss
        .snapshot_elements()
        .iter()
        .map(|e| e.name.to_ascii_lowercase())
        .filter(|n| n.starts_with("capacitor.") || n.starts_with("reactor."))
        .count();
    assert_eq!(shunts, 7, "3 capacitors + 4 reactors are in this deck");
    assert_eq!(t.num_isolated_branches, 0);
    assert_eq!(t.num_isolated_loads, 0);
}

/// **Pin 9 — a SECOND `MakePosSequence` must not unwire the circuit**
/// (the port gap G1.7's live topology compare exposed, 2026-09-04).
///
/// `modes:makeposseq/makeposseq_pc.dss` converts, solves, converts a second time
/// and solves again — the deck's own idempotency check. The second pass re-sets
/// `Phases=1` on elements that are already 1-phase, which reaches
/// `CktElementData::set_nconds` with the value it already holds. Pascal's
/// `Set_NTerms` reallocates only when `(value <> FNterms) OR (Value * Fnconds <>
/// Yorder)` (r4133 `Common/CktElement.pas:386`), so upstream does nothing there;
/// the port used to force the reallocation, which reset every terminal's
/// `bus_ref` to "unwired" without raising `BusNameRedefined`, so no later
/// `ReProcessBusDefs` restored it. The topology walk then started at the source,
/// found no bus for its terminals and reported the whole circuit isolated.
///
/// Both numbers, measured on the gating channel (dss-python 0.15.7, this deck is
/// `engines: "capi_v0145"`; `tmp/g17/capi_topo_merged.json`): the oracle reports
/// `NumIsolatedBranches = 0` and `NumIsolatedLoads = 0`; the port reported
/// **3** and **10** (all three `Line`s plus every `Load`/`Generator`/`PVSystem`/
/// `Storage`) before the fix and reports 0 / 0 now.
#[test]
fn a_second_makeposseq_keeps_the_circuit_connected() {
    let mut dss = compile_corpus_deck("modes/makeposseq/makeposseq_pc.dss");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let t = dss.topology_view();
    assert_eq!(
        t.num_isolated_branches, 0,
        "the oracle reports 0: {:?}",
        t.isolated_branches
    );
    assert_eq!(
        t.num_isolated_loads, 0,
        "the oracle reports 0: {:?}",
        t.isolated_loads
    );
    // Not vacuous: the deck really does hold the three lines and the ten PC
    // elements that used to be reported isolated, and they are all reached.
    let names: Vec<String> = dss
        .snapshot_elements()
        .iter()
        .map(|e| e.name.to_ascii_lowercase())
        .collect();
    for n in [
        "line.feed",
        "line.l2",
        "line.l3",
        "load.ld_wye",
        "storage.st1",
    ] {
        assert!(names.iter().any(|e| e == n), "{n} is in this deck");
    }
    assert_eq!(t.num_loops, 0, "the feeder is radial on both engines");

    // The mechanism, in isolation: a third conversion changes nothing, and the
    // walk still answers the same — the `set_nconds` no-op path
    // (`CktElementData::a_no_op_set_nconds_keeps_the_terminal_state`).
    dss.command("makeposseq");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let t2 = dss.topology_view();
    assert_eq!(t2.num_isolated_branches, 0);
    assert_eq!(t2.num_isolated_loads, 0);
}
