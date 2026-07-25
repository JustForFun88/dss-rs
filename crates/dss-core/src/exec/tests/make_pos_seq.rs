//! `MakePosSeq` (Pascal `TExecHelper.DoMakePosSeq`) end-to-end gates: build a
//! circuit through the Text interface, `solve`, `makeposseq`, `solve` again, and
//! read back the converted properties. The full-model live compare lives in the
//! `modes` family (`tests/corpus/modes/makeposseq/makeposseq_*.dss`,
//! `modes_cases_match_oracle`); these pin the applier's structural behaviors and
//! the per-class conversion arithmetic against the oracle-validated deck-header
//! values (dss-python 0.15.7; see `docs/wpg21_makeposseq_probes.md`).

use crate::exec::tests::common::query;
use crate::exec::*;

/// `12.47 / √3` — the wye per-phase kV the conversion targets (deck headers).
fn approx(a: &str, want: f64) -> bool {
    a.parse::<f64>()
        .map(|v| (v - want).abs() < 5e-4)
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Shunt (Capacitor SpecType 1) + the `positive_sequence` flag + bus stripping.
// ---------------------------------------------------------------------------

#[test]
fn makeposseq_sets_positive_sequence_and_converts_capacitor() {
    let mut dss = Dss::new();
    dss.command("set defaultbasefrequency=60");
    dss.command(
        "new circuit.psq basekv=12.47 pu=1.0 phases=3 bus1=src r1=0.3 x1=1.2 r0=0.9 x0=3.6",
    );
    dss.command("new line.feed bus1=src bus2=b1 phases=3 r1=0.2 x1=0.5 c1=3 length=1 units=km");
    // Capacitor SpecType 1 (kvar+kV), default grounded bus2.
    dss.command("new capacitor.cap_kvar bus1=b1 phases=3 kvar=600 kv=12.47");
    dss.command("new load.ld1 bus1=b1 phases=3 kv=12.47 kw=400 pf=0.95 model=1");
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command("set maxiterations=100");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "pre-solve: {:?}", dss.errors());
    assert!(!dss.circuit().unwrap().positive_sequence);

    dss.command("makeposseq");
    assert!(dss.errors().is_empty(), "makeposseq: {:?}", dss.errors());

    // Pascal `DoMakePosSeq` sets `ActiveCircuit.PositiveSequence := TRUE`.
    assert!(
        dss.circuit().unwrap().positive_sequence,
        "makeposseq must flip the circuit to positive-sequence"
    );

    // Capacitor SpecType 1: phases 3→1, kvar 600 → per-phase [200], kv 12.47 →
    // 12.47/√3 = 7.19956 (deck `makeposseq_shunt.dss`, oracle-validated).
    assert_eq!(query(&mut dss, "capacitor.cap_kvar.phases"), "1");
    assert!(
        approx(&query(&mut dss, "capacitor.cap_kvar.kv"), 7.19956),
        "cap kv = {}",
        query(&mut dss, "capacitor.cap_kvar.kv")
    );
    let kvar = query(&mut dss, "capacitor.cap_kvar.kvar");
    assert!(kvar.contains("200"), "cap kvar per phase [200], got {kvar}");

    // Base bus rename: the default grounded Bus2 keeps its `.0` (IsGroundBus),
    // Bus1 is a dotless bus → unchanged.
    assert_eq!(query(&mut dss, "capacitor.cap_kvar.bus1"), "b1");
    assert_eq!(query(&mut dss, "capacitor.cap_kvar.bus2"), "b1.0");

    // The converted single-phase model re-solves and converges.
    dss.command("solve");
    assert!(dss.errors().is_empty(), "post-solve: {:?}", dss.errors());
    assert!(
        dss.circuit().unwrap().is_solved,
        "converted positive-sequence circuit must re-converge"
    );
}

// ---------------------------------------------------------------------------
// Transformer: multi-winding kV/kVA struct-array conversion + the 1-phase
// OnPhase1 disable path (`.2` winding → Enabled:=FALSE, dotted buses kept).
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

// ---------------------------------------------------------------------------
// Creation order: a control created AFTER its target sees the converted target
// (Pascal `DoMakePosSeq` walks `CktElements` in creation order). The CapControl
// adopts the already-converted 1-phase capacitor's phase count.
// ---------------------------------------------------------------------------

#[test]
fn makeposseq_control_after_target_sees_converted_target() {
    let mut dss = Dss::new();
    dss.command("set defaultbasefrequency=60");
    dss.command(
        "new circuit.psq_c basekv=115 pu=1.0 phases=3 bus1=src r1=0.2 x1=2.0 r0=0.6 x0=6.0",
    );
    dss.command("new transformer.reg phases=3 windings=2 xhl=7");
    dss.command("~ wdg=1 bus=src conn=delta kv=115 kva=20000");
    dss.command("~ wdg=2 bus=b1 conn=wye kv=12.47 kva=20000");
    dss.command("new line.l1 bus1=b1 bus2=b2 phases=3 r1=0.25 x1=0.6 c1=3 length=1 units=km");
    // Target capacitor created BEFORE its control.
    dss.command("new capacitor.cap1 bus1=b2 phases=3 kvar=600 kv=12.47");
    dss.command("new load.ld2 bus1=b2 phases=3 kv=12.47 kw=800 pf=0.92 model=1");
    // Control created AFTER the capacitor + the metered line.
    dss.command(
        "new capcontrol.cc element=line.l1 terminal=1 capacitor=cap1 type=current on=60 off=30",
    );
    dss.command("set voltagebases=[115, 12.47]");
    dss.command("calcvoltagebases");
    dss.command("set maxiterations=100");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "pre-solve: {:?}", dss.errors());

    dss.command("makeposseq");
    assert!(dss.errors().is_empty(), "makeposseq: {:?}", dss.errors());

    // The capacitor (created BEFORE its control) converted to 1-phase; the
    // CapControl converts after it in creation order and adopts the converted
    // capacitor's phase count. The CapControl exposes no queryable `phases`, so
    // read its internal `NPhases` straight off the circuit element (the precise
    // adoption is also pinned live by `makeposseq_ctrl.dss`).
    assert_eq!(query(&mut dss, "capacitor.cap1.phases"), "1");
    assert_eq!(
        control_nphases(&dss, "cc"),
        Some(1),
        "CapControl converted after its 1-phase capacitor must adopt NPhases=1"
    );

    dss.command("solve");
    assert!(dss.errors().is_empty(), "post-solve: {:?}", dss.errors());
    assert!(dss.circuit().unwrap().is_solved);
}

/// Read a control element's live `NPhases` by name (the CapControl exposes no
/// `phases` property to `?`). The test module is a descendant of `exec`, so it
/// may reach `Dss`'s private class registry.
fn control_nphases(dss: &Dss, name: &str) -> Option<usize> {
    for cls in &dss.classes {
        if let Some(&i) = cls.name_to_idx.get(name)
            && let Some(e) = cls.arena[i].as_ckt_element()
        {
            return Some(e.cd().nphases);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// A 1-phase line on `.1` has its node extension stripped by the base rename;
// idempotency: a second `makeposseq` + solve leaves the model converged.
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

    // Idempotent: a second conversion + solve keeps the model converged.
    dss.command("makeposseq");
    dss.command("solve");
    assert!(
        dss.errors().is_empty(),
        "2nd makeposseq/solve: {:?}",
        dss.errors()
    );
    assert!(dss.circuit().unwrap().is_solved);
}
