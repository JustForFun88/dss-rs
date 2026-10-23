use crate::exec::tests::common::query;
use crate::exec::*;

// ---------------------------------------------------------------------------
// Transformer: multi-winding kV/kVA struct-array conversion and the disable of
// a one-phase transformer off phase 1 (`.2` winding → disabled, dotted buses
// kept).
// ---------------------------------------------------------------------------

#[test]
fn makeposseq_converts_transformer_and_disables_off_phase1() {
    let mut dss = Dss::new();
    dss.command("set defaultbasefrequency=60");
    dss.command(
        "new circuit.psq_x basekv=115 pu=1.0 phases=3 bus1=src r1=0.2 x1=2.0 r0=0.6 x0=6.0",
    );
    dss.command("new transformer.sub phases=3 windings=2 xhl=8");
    dss.command("~ wdg=1 bus=src conn=delta kv=115 kva=20000");
    dss.command("~ wdg=2 bus=b1 conn=wye kv=12.47 kva=20000");
    // 1-phase transformer ON phase 1 (.1) → survives conversion.
    dss.command("new transformer.t1_ok phases=1 windings=2 xhl=2");
    dss.command("~ wdg=1 bus=b1.1 conn=wye kv=7.2 kva=100");
    dss.command("~ wdg=2 bus=b5.1 conn=wye kv=0.24 kva=100");
    // 1-phase transformer NOT on phase 1 (.2) → disabled by MakePosSequence.
    dss.command("new transformer.t1_off phases=1 windings=2 xhl=2");
    dss.command("~ wdg=1 bus=b1.2 conn=wye kv=7.2 kva=100");
    dss.command("~ wdg=2 bus=b6.2 conn=wye kv=0.24 kva=100");
    dss.command("new load.ld1 bus1=b1 phases=3 kv=12.47 kw=200 pf=0.95 model=1");
    dss.command("new load.ld5 bus1=b5.1 phases=1 kv=0.24 kw=20 pf=0.95 model=1");
    dss.command("set voltagebases=[115, 12.47, 0.24, 7.2]");
    dss.command("calcvoltagebases");
    dss.command("set maxiterations=100");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "pre-solve: {:?}", dss.errors());

    dss.command("makeposseq");
    assert!(dss.errors().is_empty(), "makeposseq: {:?}", dss.errors());

    // 2-winding sub: phases 3→1, per-winding kV=kVLL/√3 ([66.3953, 7.19956]),
    // kVA/phases ([6666.67, 6666.67]) — deck `makeposseq_xfmr.dss`.
    assert_eq!(query(&mut dss, "transformer.sub.phases"), "1");
    let kvs = query(&mut dss, "transformer.sub.kvs");
    assert!(
        kvs.contains("66.395") && kvs.contains("7.199"),
        "sub kvs = {kvs}"
    );
    let kvas = query(&mut dss, "transformer.sub.kvas");
    assert!(kvas.contains("6666.6"), "sub kvas = {kvas}");

    // 1-phase on `.1` survives (enabled), on `.2` is disabled and keeps its
    // dotted buses (the early-Exit path skips the base rename).
    assert_eq!(query(&mut dss, "transformer.t1_ok.enabled"), "Yes");
    assert_eq!(
        query(&mut dss, "transformer.t1_off.enabled"),
        "No",
        "a 1-phase winding off phase 1 must be disabled by MakePosSequence"
    );
    let buses = query(&mut dss, "transformer.t1_off.buses");
    assert!(
        buses.contains("b1.2") && buses.contains("b6.2"),
        "disabled transformer keeps its dotted buses, got {buses}"
    );

    dss.command("solve");
    assert!(dss.errors().is_empty(), "post-solve: {:?}", dss.errors());
    assert!(dss.circuit().unwrap().is_solved);
}
