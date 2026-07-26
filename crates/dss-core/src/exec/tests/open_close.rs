//! Exec `Open`/`Close` command tests (Pascal `DoOpenCmd`/`DoCloseCmd`,
//! ExecHelper.pas:1451/1484). `Open class.name term cond` forces a terminal (or
//! a single 1-based conductor) open; `Close` reverses it. Both raise
//! `SystemYChanged`, so the next solve rebuilds Y with the conductor open.
//! Oracle-pinned (dss-python 0.15.7) on a stiff source + short line + load.

use crate::exec::*;

/// Terminal-1 phase-current magnitudes (A) of `line.l1` after a solve.
fn line_phase_currents(dss: &mut Dss) -> [f64; 3] {
    let snap = dss.snapshot_elements();
    let l = snap
        .iter()
        .find(|e| e.name.eq_ignore_ascii_case("Line.l1"))
        .expect("line snapshot");
    let mag = |ph: usize| (l.currents[ph].re.powi(2) + l.currents[ph].im.powi(2)).sqrt();
    [mag(0), mag(1), mag(2)]
}

fn build(dss: &mut Dss) {
    dss.command("clear");
    dss.command("new circuit.t basekv=12.47 phases=3 bus1=src mvasc3=20000 mvasc1=21000");
    dss.command(
        "new line.l1 bus1=src bus2=b length=1 units=km r1=0.1 x1=0.3 r0=0.3 x0=0.9 c1=0 c0=0",
    );
    dss.command("new load.ld bus1=b phases=3 kv=12.47 kw=500 pf=0.95 model=1");
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
}

/// `Open Line.l1 1` then `Close Line.l1 1`: the whole terminal opens (downstream
/// collapses to 0 A) and recloses to the baseline current (24.384 A/phase,
/// oracle). The reclose round-trip proves `Close` un-does the open and that both
/// raise `SystemYChanged` (a stale Y would leave the line open after `Close`).
#[test]
fn open_close_whole_terminal_round_trip() {
    let mut dss = Dss::new();
    build(&mut dss);
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let base = line_phase_currents(&mut dss);
    for i in base {
        assert!((i - 24.3839).abs() < 1e-2, "baseline phase current {i}");
    }

    dss.command("open line.l1 1");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    for i in line_phase_currents(&mut dss) {
        assert!(i < 1e-6, "open: phase current {i} should be ~0");
    }

    dss.command("close line.l1 1");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    for i in line_phase_currents(&mut dss) {
        assert!((i - 24.3839).abs() < 1e-2, "reclosed phase current {i}");
    }
}

/// `Open Line.l1 1 2` opens **only** the 1-based conductor 2 (phase B): phase B
/// goes to 0 A while A/C still carry ~24.4 A (oracle [24.377, 0, 24.401]). Guards
/// the `cond>0` single-conductor path against opening the whole terminal.
#[test]
fn open_single_conductor() {
    let mut dss = Dss::new();
    build(&mut dss);
    dss.command("open line.l1 1 2");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let i = line_phase_currents(&mut dss);
    assert!((i[0] - 24.377).abs() < 5e-2, "phase A {}", i[0]);
    assert!(i[1] < 1e-6, "phase B (opened) {}", i[1]);
    assert!((i[2] - 24.401).abs() < 5e-2, "phase C {}", i[2]);
}

/// `Open transformer.tx 2` opens a transformer **winding** terminal (the
/// DG_Prot_Fdr `open transformer.tg 2` pattern): the downstream load collapses to
/// 0 V and recloses to its energized voltage (7156.7 V, oracle). Confirms the
/// generic terminal-open machinery works on a multi-winding element, not just a
/// Line.
#[test]
fn open_close_transformer_winding() {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.t basekv=115 phases=3 bus1=src mvasc3=20000 mvasc1=21000");
    dss.command(
        "new transformer.tx phases=3 windings=2 xhl=8 buses=[src.1.2.3 lv.1.2.3] \
         conns=[delta wye] kvs=[115 12.47] kvas=[5000 5000] %r=0.5",
    );
    dss.command(
        "new line.l1 bus1=lv bus2=b length=1 units=km r1=0.1 x1=0.3 r0=0.3 x0=0.9 c1=0 c0=0",
    );
    dss.command("new load.ld bus1=b phases=3 kv=12.47 kw=500 model=1");
    dss.command("set voltagebases=[115 12.47]");
    dss.command("calcvoltagebases");

    // Total load real power drawn (kW): ~500 energized, ~0 de-energized.
    let load_kw = |dss: &mut Dss| -> f64 {
        let snap = dss.snapshot_elements();
        let l = snap
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case("Load.ld"))
            .expect("load snapshot");
        l.powers.iter().map(|s| s.re).sum::<f64>()
    };

    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert!((load_kw(&mut dss) - 500.0).abs() < 5.0, "baseline load kW");

    dss.command("open transformer.tx 2");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert!(
        load_kw(&mut dss).abs() < 1e-3,
        "open wdg2: load should draw ~0 kW"
    );

    dss.command("close transformer.tx 2");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert!((load_kw(&mut dss) - 500.0).abs() < 5.0, "reclosed load kW");
}

/// A bad object name records the Pascal "Circuit Element not found" error (#259)
/// and leaves the circuit untouched. `Open circuit` likewise errors — the
/// dotless token resolves to an empty class, so the oracle (probed) reports the
/// same #259, not a no-op.
#[test]
fn open_errors_on_bad_element() {
    let mut dss = Dss::new();
    build(&mut dss);
    dss.command("open line.nope 1");
    assert!(
        dss.errors()
            .iter()
            .any(|e| e.contains("Circuit Element not found")),
        "expected #259 'Circuit Element not found', got {:?}",
        dss.errors()
    );

    let mut dss2 = Dss::new();
    build(&mut dss2);
    dss2.command("open circuit");
    assert!(
        dss2.errors().iter().any(|e| e.contains("not found")),
        "open circuit should error (oracle #259), got {:?}",
        dss2.errors()
    );
}
