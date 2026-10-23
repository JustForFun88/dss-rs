use crate::elements::pos_seq::balance;
use crate::exec::tests::common::query;
use crate::exec::*;

// ---------------------------------------------------------------------------
// A 1-phase line on `.1` has its node extension stripped by the base rename.
// A second `makeposseq` changes nothing on the reduced circuit, which solves.
// ---------------------------------------------------------------------------

#[test]
fn makeposseq_strips_line_node_extension_and_is_idempotent() {
    let mut dss = Dss::new();
    dss.command("set defaultbasefrequency=60");
    dss.command(
        "new circuit.psq_l basekv=12.47 pu=1.0 phases=3 bus1=src r1=0.3 x1=1.2 r0=0.9 x0=3.6",
    );
    dss.command("new line.l3 bus1=src bus2=b3 phases=3 r1=0.2 x1=0.5 c1=3 length=1 units=km");
    // 1-phase line on phase 1: buses carry a `.1` extension.
    dss.command(
        "new line.l_1ph bus1=b3.1 bus2=b4.1 phases=1 r1=0.3 x1=0.6 c1=3.4 length=0.2 units=km",
    );
    dss.command("new load.ld1 bus1=b3 phases=3 kv=12.47 kw=300 pf=0.95 model=1");
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command("set maxiterations=100");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "pre-solve: {:?}", dss.errors());

    dss.command("makeposseq");
    dss.command("solve");
    assert!(
        dss.errors().is_empty(),
        "1st makeposseq/solve: {:?}",
        dss.errors()
    );

    // The `.1` node extension is stripped from the 1-phase line's buses.
    assert_eq!(query(&mut dss, "line.l_1ph.bus1"), "b3");
    assert_eq!(query(&mut dss, "line.l_1ph.bus2"), "b4");

    // The second `makeposseq` changes nothing: the saved circuit stays
    // byte-identical, the load keeps a third of its 300 kW and the line its
    // stripped buses. The circuit solves again.
    let saved = balance::saved_circuit(&mut dss, "strip");
    dss.command("makeposseq");
    assert!(
        dss.errors().is_empty(),
        "2nd makeposseq: {:?}",
        dss.errors()
    );
    assert_eq!(
        balance::saved_circuit(&mut dss, "strip"),
        saved,
        "the saved circuit"
    );
    let kw = balance::query_f64(&mut dss, "load.ld1.kw");
    balance::assert_rel(kw, 100.0, 1e-12, "load.ld1.kw");
    assert_eq!(query(&mut dss, "line.l_1ph.bus1"), "b3");
    assert_eq!(query(&mut dss, "line.l_1ph.bus2"), "b4");
    dss.command("solve");
    assert!(
        dss.errors().is_empty(),
        "2nd makeposseq/solve: {:?}",
        dss.errors()
    );
    assert!(dss.circuit().unwrap().is_solved);
}
