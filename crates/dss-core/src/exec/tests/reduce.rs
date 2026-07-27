use crate::exec::*;

/// `Set ReduceOption/Zmag/KeepLoad=` defaults + round-trip through `Get`.
/// (ReduceOption's default string is empty, so `Get` elides it — exactly
/// like Pascal `AppendGlobalResult` on a zero-length string.)
#[test]
fn reduce_options_defaults_and_round_trip() {
    let mut dss = Dss::new();
    dss.command("New circuit.c1");
    dss.command("Get zmag keepload");
    assert_eq!(dss.result(), "0.02, Yes");
    dss.command("Get reduceoption");
    assert_eq!(dss.result(), "");

    dss.command("Set reduceoption=shortlines zmag=0.05 keepload=no");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    {
        let ckt = dss.circuit().unwrap();
        assert_eq!(
            ckt.reduction_strategy,
            crate::circuit::ReductionStrategy::ShortLines
        );
        assert_eq!(ckt.reduction_strategy_string, "shortlines");
        assert_eq!(ckt.reduction_zmag, 0.05);
        assert!(!ckt.reduce_laterals_keep_load);
    }
    dss.command("Get reduceoption zmag keepload");
    assert_eq!(dss.result(), "shortlines, 0.05, No");
}

/// `DoSetReduceStrategy` dispatches on the first character; `S` resolves to
/// Switch via `CompareTextShortest(S,'SWITCH')`, else ShortLines.
#[test]
fn reduce_strategy_first_char_dispatch() {
    use crate::circuit::ReductionStrategy as Rs;
    let mut dss = Dss::new();
    dss.command("New circuit.c1");
    let cases = [
        ("break", Rs::BreakLoop),
        ("default", Rs::Default),
        ("ends", Rs::Dangling),
        ("laterals", Rs::Laterals),
        ("merge", Rs::MergeParallel),
        ("switch", Rs::Switches),
        ("shortlines", Rs::ShortLines),
        ("s", Rs::Switches), // CompareTextShortest("s","SWITCH")=0 -> Switch
    ];
    for (opt, want) in cases {
        dss.command(&format!("Set reduceoption={opt}"));
        assert_eq!(dss.circuit().unwrap().reduction_strategy, want, "opt={opt}");
    }
    // Unknown strategy: error logged, strategy falls back to Default.
    dss.command("Set reduceoption=zzz");
    assert!(
        dss.errors()
            .iter()
            .any(|e| e.contains("Unknown Reduction Strategy")),
        "{:?}",
        dss.errors()
    );
    assert_eq!(dss.circuit().unwrap().reduction_strategy, Rs::Default);
}

/// `Reduce` with no energy meters reproduces Pascal error 1890, including
/// the full documentation URL (pinned so an edit can't silently drift it).
#[test]
fn reduce_command_requires_energy_meter() {
    let mut dss = Dss::new();
    dss.command("New circuit.c1 basekv=12.47 bus1=src phases=3");
    dss.command("reduce");
    assert!(
            dss.errors().iter().any(|e| e.text()
                == "An energy meter is required to use this feature. Please check \
                    https://sourceforge.net/p/electricdss/code/HEAD/tree/trunk/Version8/Doc/Circuit%20Reduction%20for%20Version8.docx \
                    for examples."),
            "{:?}",
            dss.errors()
        );
}

/// `Reduce <name>` with a meter present but no such meter reproduces Pascal
/// error 262 (echoing the *uppercased* name), not the generic deferral.
#[test]
fn reduce_named_meter_not_found_is_262() {
    let mut dss = Dss::new();
    dss.command("New circuit.c1 basekv=12.47 bus1=src phases=3");
    dss.command("New line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6");
    dss.command("New energymeter.m1 element=line.l1 terminal=1");
    dss.command("reduce nope");
    assert!(
        dss.errors()
            .iter()
            .any(|e| e.text() == "EnergyMeter \"NOPE\" not found."),
        "{:?}",
        dss.errors()
    );
    // The deferral must NOT fire for a name that did not resolve.
    assert!(
        !dss.errors().iter().any(|e| e.contains("not ported")),
        "{:?}",
        dss.errors()
    );
}

/// `Reduce` marks enabled shunt cap/reactor buses as keepers *before* the
/// meter check — so the marking happens even on the error-1890 path
/// (Pascal `MarkCapandReactorBuses` runs unconditionally).
#[test]
fn reduce_marks_cap_and_reactor_buses() {
    let mut dss = Dss::new();
    dss.command("New circuit.c1 basekv=12.47 bus1=src phases=3");
    dss.command("New line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6");
    dss.command("New line.l2 bus1=b1 bus2=b2 length=1 r1=0.3 x1=0.6");
    dss.command("New capacitor.c bus1=b1 phases=3 kvar=600 kv=12.47");
    dss.command("New reactor.r bus1=b2 phases=3 kvar=100 kv=12.47");
    dss.command("Set voltagebases=[12.47]");
    dss.command("CalcVoltageBases");
    // Bus refs are materialized at Y-build (solve) time in this port; a
    // real `Reduce` always runs post-solve (it needs metered zones).
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    // No energy meter → error 1890, but the marking still ran first.
    dss.command("reduce");
    let ckt = dss.circuit().unwrap();
    let keep = |name: &str| {
        ckt.buses
            .iter()
            .find(|b| b.name.eq_ignore_ascii_case(name))
            .map(|b| b.keep)
            .unwrap_or(false)
    };
    assert!(keep("b1"), "shunt capacitor bus should be a keeper");
    assert!(keep("b2"), "shunt reactor bus should be a keeper");
}

/// `Reduce` with a metered feeder runs the zone reduction (WP8.7): the DEFAULT
/// strategy merges an un-loaded in-line bus out (`Line.MergeWith` SERIES —
/// child renamed `Other.Name~Name`, parent disabled), and the node count drops.
/// No deferral error is emitted.
#[test]
fn reduce_command_merges_inline_lines() {
    let mut dss = Dss::new();
    dss.command("New circuit.c1 basekv=12.47 bus1=src phases=3");
    dss.command("New line.lfeed bus1=src bus2=b1 length=1 r1=0.3 x1=0.6");
    dss.command("New line.l1 bus1=b1 bus2=b2 length=1 r1=0.3 x1=0.6");
    dss.command("New line.l2 bus1=b2 bus2=b3 length=1 r1=0.3 x1=0.6");
    dss.command("New load.ld3 bus1=b3 phases=3 kv=12.47 kw=300 pf=0.95");
    dss.command("New energymeter.m1 element=line.lfeed terminal=1");
    dss.command("Set voltagebases=[12.47]");
    dss.command("CalcVoltageBases");
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let nodes_before = dss.circuit().unwrap().num_nodes;

    dss.command("reduce");
    dss.command("Solve");
    assert!(
        !dss.errors().iter().any(|e| e.contains("not ported")),
        "{:?}",
        dss.errors()
    );

    {
        let ckt = dss.circuit().unwrap();
        // b2 (un-loaded, single in-line child) was eliminated → node count drops.
        assert!(
            ckt.num_nodes < nodes_before,
            "reduction should have dropped nodes ({} !< {nodes_before})",
            ckt.num_nodes
        );
        assert!(
            ckt.buses.iter().all(|b| !b.name.eq_ignore_ascii_case("b2")),
            "bus b2 should be eliminated by the l1+l2 merge"
        );
    }
    // The l1/l2 merge disabled the parent (l1) and produced a merged line
    // named l1~l2 (child renamed) — observable via the `?` property query.
    dss.command("? line.l1.enabled");
    assert_eq!(dss.result(), "No", "the parent line l1 should be disabled");
    dss.command("? line.l1~l2.enabled");
    assert_eq!(
        dss.result(),
        "Yes",
        "the merged line l1~l2 should be enabled"
    );
}

/// Build a `shortlines`-reducible feeder whose **parent** branch carries a
/// capacitor at `b2` — optionally preceded by a load — and report whether the
/// reduction left `b2` standing.
///
/// Shape: `src —lfeed(long)→ b1 —l1(short)→ b2 —l2(short)→ b3`. `lfeed` is long
/// enough never to be flagged, `l1`'s own merge-with-child attempt is blocked by
/// the `b2` capacitor in every lane (that branch always scanned all shunts), so
/// the only decision left is `l2`'s merge **with its parent** — the site of the
/// quirk.
///
/// The shunt list is the bus PC-adjacency list, which
/// `build_active_bus_adjacency_lists` fills from `pc_elements` **first** and
/// only then appends the shunt PD elements (pinned by
/// `exec::tests::solve`'s `adj.pc[b2] == ["ld1", "cap1"]`). So a capacitor at a
/// bus that also carries any load/generator is *never* the first shunt — which
/// is exactly what makes upstream's one-element scan blind to the elements it
/// was written to find.
fn reduce_shortlines_keeps_b2(load_ahead_of_the_capacitor: bool) -> bool {
    let mut dss = Dss::new();
    dss.command("New circuit.c1 basekv=12.47 bus1=src phases=3");
    dss.command("New line.lfeed bus1=src bus2=b1 length=20 r1=0.3 x1=0.6");
    dss.command("New line.l1 bus1=b1 bus2=b2 length=1 r1=0.3 x1=0.6");
    dss.command("New line.l2 bus1=b2 bus2=b3 length=1 r1=0.3 x1=0.6");
    if load_ahead_of_the_capacitor {
        dss.command("New load.ldb2 bus1=b2 phases=3 kv=12.47 kw=50 pf=0.95");
    }
    dss.command("New capacitor.cb2 bus1=b2 phases=3 kvar=600 kv=12.47");
    // A shunt on l2's own TO bus keeps l2 off the "dangling, just discard it"
    // path, so it reaches the merge-with-parent branch.
    dss.command("New load.ld3 bus1=b3 phases=3 kv=12.47 kw=300 pf=0.95");
    dss.command("New energymeter.m1 element=line.lfeed terminal=1");
    dss.command("Set voltagebases=[12.47]");
    dss.command("CalcVoltageBases");
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    // |Z| = 0.671 for the 1-unit lines, 13.4 for lfeed: only l1/l2 are "short".
    dss.command("Set reduceoption=shortlines zmag=1.0");
    dss.command("reduce");
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let ckt = dss.circuit().unwrap();
    ckt.buses.iter().any(|b| b.name.eq_ignore_ascii_case("b2"))
}

/// Expected-value pin for the Stage F single-site quirk
/// `compat::REDUCE_SCANS_ONLY_THE_FIRST_PARENT_SHUNT`.
///
/// `DoReduceShortLines`' merge-with-parent branch (`ReduceAlgs.pas:200-210`)
/// opens its capacitor scan on `ParentNode.FirstShuntObject()` but advances it
/// with `PresentBranch.NextShuntObject()`, whose cursor is already past the end
/// — so upstream inspects exactly one parent shunt and a capacitor sitting
/// second is merged away (onto another bus) instead of blocking. The parity lane
/// reproduces that; the default lane scans them all, like the merge-with-child
/// branch of the same procedure (`:246-258`) already does.
///
/// Asserted against `compat::ORACLE_PARITY` so the test is load-bearing in
/// **both** builds.
#[test]
fn short_line_parent_shunt_scan_is_lane_split() {
    let kept = reduce_shortlines_keeps_b2(true);
    assert_eq!(
        kept,
        !crate::compat::ORACLE_PARITY,
        "with a load ahead of it the capacitor is the parent's SECOND shunt, so \
         upstream's one-element scan misses it and merges b2 out; the default \
         lane finds it and blocks the merge (lane parity = {})",
        crate::compat::ORACLE_PARITY
    );

    // Control: the same capacitor, alone at b2, is the FIRST shunt — both lanes
    // see it and both refuse the merge. That is what makes the case above a
    // scan-length difference and not a changed predicate.
    assert!(
        reduce_shortlines_keeps_b2(false),
        "a capacitor at the head of the parent's shunt list must block the \
         merge in every lane"
    );
}
