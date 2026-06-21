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
            dss.errors().iter().any(|e| e
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
            .any(|e| e == "EnergyMeter \"NOPE\" not found."),
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

/// `Reduce` with a meter present passes the precondition but the zone
/// reduction (`Line.MergeWith`) is NOT_PORTED — the documented deferral.
#[test]
fn reduce_command_with_meter_deferred() {
    let mut dss = Dss::new();
    dss.command("New circuit.c1 basekv=12.47 bus1=src phases=3");
    dss.command("New line.l1 bus1=src bus2=b1 length=1 r1=0.3 x1=0.6");
    dss.command("New energymeter.m1 element=line.l1 terminal=1");
    dss.command("reduce");
    assert!(
        dss.errors()
            .iter()
            .any(|e| e.contains("reduction is not ported")),
        "{:?}",
        dss.errors()
    );
}
