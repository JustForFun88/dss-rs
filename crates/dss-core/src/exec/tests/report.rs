//! Phase 8 report-router skeleton tests (WP8.1): the `Plot`/`Visualize`
//! headless no-op (PHASE8_PLAN §2.5), the faithful `Show` no-op that keeps the
//! `solvable_now` decks green, and the scoped `NOT_PORTED` stubs for the not-yet
//! ported `Export`/`Save`/`Dump` verbs.

use crate::exec::*;

/// A small solved 3-phase feeder. Returns `(name, currents)` per element so two
/// runs can be compared for *bit-identical* model state.
fn solve_currents(extra: &[&str]) -> Vec<(String, Vec<f64>)> {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.t basekv=12.47 phases=3 bus1=src mvasc3=20000 mvasc1=21000");
    dss.command(
        "new line.l1 bus1=src bus2=b length=1 units=km r1=0.1 x1=0.3 r0=0.3 x0=0.9 c1=0 c0=0",
    );
    dss.command("new load.ld bus1=b phases=3 kv=12.47 kw=500 pf=0.95 model=1");
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    for c in extra {
        dss.command(c);
    }
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss.snapshot_elements()
        .into_iter()
        .map(|e| (e.name, e.currents))
        .collect()
}

/// `Plot`/`Visualize` are GUI commands; headless they must be exact no-ops —
/// the solved model is bit-identical with and without them (PHASE8_PLAN §2.5).
#[test]
fn plot_visualize_are_headless_noops() {
    let base = solve_currents(&[]);
    assert!(!base.is_empty(), "fixture produced no elements to compare");
    let with_plot = solve_currents(&[
        "plot type=circuit quantity=power",
        "plot daisy",
        "visualize element=line.l1",
    ]);
    assert_eq!(base, with_plot, "Plot/Visualize changed the solved model");
}

/// `Plot`/`Visualize`/`Show` are post-circuit verbs (Pascal `ProcessCommand`):
/// issued before any circuit, the engine's dispatch gate records the #301
/// "create a circuit first" error *before* the GUI/report routine runs —
/// oracle-probed (audit-code WP8.1). So they must NOT be clean no-ops here.
#[test]
fn report_verbs_before_circuit_error_301() {
    for verb in [
        "plot type=circuit",
        "visualize element=foo",
        "show voltages",
    ] {
        let mut dss = Dss::new();
        dss.command(verb);
        assert_eq!(dss.errors().len(), 1, "{verb:?}: {:?}", dss.errors());
        assert!(
            dss.errors()[0].contains("You must create a new circuit object first"),
            "{verb:?}: expected #301, got {:?}",
            dss.errors()
        );
    }
}

/// The unported `Show` reports are *silent* no-ops (no recorded error) — this is
/// what keeps the 44 `solvable_now` decks that contain `Show Power`/`Show
/// Voltage`/`Show f` green (the live gate asserts `errors().is_empty()`).
#[test]
fn show_reports_are_silent_noops() {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.t basekv=12.47 phases=3 bus1=src");
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "setup: {:?}", dss.errors());
    for c in ["Show Power kVA elements", "Show Voltage LN Nodes", "show f"] {
        dss.command(c);
        assert!(dss.errors().is_empty(), "{c:?}: {:?}", dss.errors());
    }
}

/// The unported `Export` verbs record a scoped `NOT_PORTED` (loud, not a silent
/// fake), and an unknown keyword gets the Pascal 24713 message. (`Export` decks
/// all sit in `skipped_unsupported`, so this never reaches the live gate.)
#[test]
fn export_records_scoped_not_ported() {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.t basekv=12.47 phases=3 bus1=src");

    dss.command("export voltages");
    assert_eq!(dss.errors().len(), 1);
    // Pin the *quoted* canonical name so the assertion can't be satisfied by a
    // sibling whose name merely contains "Voltages" (SeqVoltages /
    // VoltagesElements / YVoltages) — audit-tests WP8.1.
    assert!(
        dss.errors()[0].contains("\"Voltages\"") && dss.errors()[0].contains("not ported"),
        "{:?}",
        dss.errors()
    );

    // Abbreviation resolves like the oracle's ExportCommands (earliest-wins:
    // `v` → Voltages, not SeqVoltages/VoltagesElements/YVoltages).
    let mut dss = Dss::new();
    dss.command("new circuit.t basekv=12.47 phases=3 bus1=src");
    dss.command("export v");
    assert!(
        dss.errors()[0].contains("\"Voltages\""),
        "{:?}",
        dss.errors()
    );

    // Unknown keyword → Pascal 24713, with the lowercased keyword echoed back.
    let mut dss = Dss::new();
    dss.command("new circuit.t basekv=12.47 phases=3 bus1=src");
    dss.command("export notareport");
    assert_eq!(
        dss.errors()[0],
        "Error: Unknown Export command: \"notareport\"",
        "{:?}",
        dss.errors()
    );
}

/// `Save`/`Dump` record a scoped `NOT_PORTED` until WP8.5 (their corpus decks
/// are all in `skipped_unsupported`).
#[test]
fn save_dump_record_scoped_not_ported() {
    let mut dss = Dss::new();
    dss.command("new circuit.t basekv=12.47 phases=3 bus1=src");
    dss.command("save circuit");
    dss.command("dump line.foo");
    assert_eq!(dss.errors().len(), 2, "{:?}", dss.errors());
    assert!(dss.errors()[0].contains("Save"), "{:?}", dss.errors());
    assert!(dss.errors()[1].contains("Dump"), "{:?}", dss.errors());
}
