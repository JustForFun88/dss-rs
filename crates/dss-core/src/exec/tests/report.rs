//! Phase 8 report-router skeleton tests (WP8.1): the `Plot`/`Visualize`
//! headless no-op (PHASE8_PLAN §2.5), the faithful `Show` no-op that keeps the
//! `solvable_now` decks green, and the scoped `NOT_PORTED` stubs for the not-yet
//! ported `Export`/`Save`/`Dump` verbs.

use std::path::PathBuf;

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

/// The still-unported `Show` reports are *silent* no-ops (no recorded error) —
/// this is what keeps the `solvable_now` decks that contain `Show Power`/`Show
/// meters`/`Show f` green (the live gate asserts `errors().is_empty()`).
///
/// Route the datapath to a scratch dir first: since WP8.4 several `Show` forms
/// (Voltages/Currents/Elements) are **real reports** that write a file, so a
/// no-datapath `Show Voltage … Nodes` here would leak into the source tree. The
/// keywords below are the ones *still* deferred (Powers element form → code-1
/// no-op; `meters` → the trailing `_ => {}`; `f` → an unmatched keyword), so they
/// write nothing — but the scratch datapath keeps the test tree-safe regardless.
#[test]
fn show_reports_are_silent_noops() {
    let set_dp = format!("set datapath=\"{}\"", std::env::temp_dir().display());
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.t basekv=12.47 phases=3 bus1=src");
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command("solve");
    dss.command(&set_dp);
    assert!(dss.errors().is_empty(), "setup: {:?}", dss.errors());
    for c in ["Show Power kVA elements", "show meters", "show f"] {
        dss.command(c);
        assert!(dss.errors().is_empty(), "{c:?}: {:?}", dss.errors());
    }
}

/// Build a small solved, **metered** feeder (meter `m1` on `line.l1`), datapath
/// pointed at the temp dir — the setup for the `Show Zone`/`Show Loops`
/// dispatcher tests. Zone lists build on the first `solve` after the meter is added.
fn solved_metered() -> Dss {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.zt basekv=12.47 phases=3 bus1=src");
    dss.command("new line.l1 bus1=src bus2=b length=1 units=km r1=0.1 x1=0.3 c1=0");
    dss.command("new load.ld bus1=b phases=3 kv=12.47 kw=100 model=1");
    dss.command("new energymeter.m1 element=line.l1 terminal=1");
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command("solve");
    dss.command(&format!(
        "set datapath=\"{}\"",
        std::env::temp_dir().display()
    ));
    assert!(dss.errors().is_empty(), "setup: {:?}", dss.errors());
    dss
}

/// `Show Zone`'s two reachable error branches (Pascal `ShowMeterZone`,
/// `ShowResults.pas:2450`/`:2494`) — which a golden can't cover, since each pushes
/// an error and would fail the golden's `errors().is_empty()`: an **empty** meter
/// name → #221 `Meter Name Not Specified.`; an **unknown** meter → #220
/// `EnergyMeter "<name>" not found.` In both, Pascal still creates the
/// `<Case_>ZoneOut_<name>.txt` file (empty) and sets `GlobalResult`, so the port
/// writes it too — pinned here via `last_result_file` (the filename carries the
/// (possibly empty) meter name). The **happy** path (a valid meter → the branch
/// tree) is the `show_zone`/`show_zone_mesh` golden.
#[test]
fn show_zone_error_paths() {
    // (1) empty meter name → #221; the `…ZoneOut_.txt` file is still written.
    let mut dss = solved_metered();
    dss.command("show zone");
    assert_eq!(dss.errors().len(), 1, "{:?}", dss.errors());
    assert!(
        dss.errors()[0].contains("Meter Name Not Specified"),
        "{:?}",
        dss.errors()
    );
    assert!(
        dss.last_result_file()
            .to_lowercase()
            .ends_with("zoneout_.txt"),
        "empty-name zone file: {:?}",
        dss.last_result_file()
    );

    // (2) unknown meter → #220; the `…ZoneOut_nosuch.txt` file is still written.
    let mut dss = solved_metered();
    dss.command("show zone nosuch");
    assert_eq!(dss.errors().len(), 1, "{:?}", dss.errors());
    assert!(
        dss.errors()[0].contains("EnergyMeter \"nosuch\" not found"),
        "{:?}",
        dss.errors()
    );
    assert!(
        dss.last_result_file()
            .to_lowercase()
            .ends_with("zoneout_nosuch.txt"),
        "not-found zone file: {:?}",
        dss.last_result_file()
    );

    // (3) happy micro path: a valid meter + `Show Loops` both run clean (the
    //     dispatcher wiring / solve-guard). `Show Loops` on this radial single-meter
    //     circuit emits only its header, no error.
    let mut dss = solved_metered();
    dss.command("show zone m1");
    dss.command("show loops");
    assert!(dss.errors().is_empty(), "happy path: {:?}", dss.errors());
}

/// A **found-but-disabled** EnergyMeter has `BranchList = NIL` (no zone is built for
/// a disabled meter): Pascal `ShowMeterZone` guards the entire body — header
/// included — on `BranchList <> NIL` (`ShowResults.pas:2453`), so it writes an
/// **empty** file and raises no error; `ShowLoops` skips it (`:3063`). Regression
/// guard for the audit-code step-12 Major fix (the port previously wrote the 2-line
/// header for this case — a silent, gate-invisible divergence, oracle-confirmed
/// 46 bytes vs 0). Not corpus-reachable (all corpus meters are enabled), so
/// synthesized (PHASE8_PLAN §1). Byte-checked here (a golden can't: `Show` sets no
/// diffable content and this asserts the *absence* of any).
#[test]
fn show_zone_disabled_meter_is_empty() {
    let scratch = std::env::temp_dir().join(format!("dss_zone_dis_{}", std::process::id()));
    std::fs::create_dir_all(&scratch).unwrap();
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.zt basekv=12.47 phases=3 bus1=src");
    dss.command("new line.l1 bus1=src bus2=b length=1 units=km r1=0.1 x1=0.3 c1=0");
    dss.command("new load.ld bus1=b phases=3 kv=12.47 kw=100 model=1");
    dss.command("new energymeter.m1 element=line.l1 terminal=1");
    dss.command("new energymeter.m2 element=line.l1 terminal=1 enabled=no");
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command("solve");
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    assert!(dss.errors().is_empty(), "setup: {:?}", dss.errors());

    // `show zone m2` (disabled): the meter is FOUND (no #220), but with no built
    // zone the file is written EMPTY — no header — and no error is raised.
    dss.command("show zone m2");
    assert!(dss.errors().is_empty(), "show zone m2: {:?}", dss.errors());
    let produced = PathBuf::from(dss.last_result_file());
    assert!(
        produced
            .to_string_lossy()
            .to_lowercase()
            .ends_with("zoneout_m2.txt"),
        "zone file: {produced:?}"
    );
    let bytes =
        std::fs::read(&produced).unwrap_or_else(|e| panic!("read {}: {e}", produced.display()));
    assert!(
        bytes.is_empty(),
        "disabled-meter zone file must be empty, got {} bytes: {:?}",
        bytes.len(),
        String::from_utf8_lossy(&bytes)
    );

    // `show loops` skips the disabled m2 (`BranchList = NIL`) — the radial m1 zone
    // has no loops, so the report is header-only, no error, no panic.
    dss.command("show loops");
    assert!(dss.errors().is_empty(), "show loops: {:?}", dss.errors());

    std::fs::remove_dir_all(&scratch).ok();
}

/// The export router's three non-formatting outcomes: the WP8.2 solution guard
/// (#24712), a still-unported keyword's scoped `NOT_PORTED` (loud, not a silent
/// fake), and an unknown keyword's Pascal 24713. The *happy* path (a real report
/// written + diffed vs the oracle) is the `golden_reports.rs` gate. (`Export`
/// decks all sit in `skipped_unsupported`, so this never reaches the live gate.)
#[test]
fn export_records_scoped_not_ported() {
    // Defensive: route any report write to a scratch dir, never the source tree.
    // Every branch below errors *before* a write, but a future write-reaching
    // branch must not pollute the tree (audit-tests WP8.2).
    let set_dp = format!("set datapath=\"{}\"", std::env::temp_dir().display());

    // (1) A solution-requiring export (`Voltages`) on an *unsolved* circuit hits
    //     the solution guard (#24712, `ExportOptions.pas:171`) before any output.
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.t basekv=12.47 phases=3 bus1=src");
    dss.command(&set_dp);
    dss.command("export voltages");
    assert_eq!(dss.errors().len(), 1, "{:?}", dss.errors());
    assert!(
        dss.errors()[0].contains("must be solved"),
        "{:?}",
        dss.errors()
    );

    // (2) A still-unported export records the scoped `NOT_PORTED`. `GICMvars`(36)
    //     is a Phase-9 GIC export (stays unported through all of Phase 8) and is
    //     not solution-guarded (36 ∉ the #24712 ptr set), so no solve is needed.
    //     (WP8.3 step 3a ported `EventLog`(33)/`ErrorLog`(52), so those are now
    //     real dumps gated by `golden_reports.rs`, not this NOT_PORTED test.)
    let mut dss = Dss::new();
    dss.command("new circuit.t basekv=12.47 phases=3 bus1=src");
    dss.command(&set_dp);
    dss.command("export gicmvars");
    assert_eq!(dss.errors().len(), 1, "{:?}", dss.errors());
    assert!(
        dss.errors()[0].contains("\"GICMvars\"") && dss.errors()[0].contains("not ported"),
        "{:?}",
        dss.errors()
    );

    // (3) Unknown keyword → Pascal 24713, with the lowercased keyword echoed back.
    let mut dss = Dss::new();
    dss.command("new circuit.t basekv=12.47 phases=3 bus1=src");
    dss.command(&set_dp);
    dss.command("export notareport");
    assert_eq!(
        dss.errors()[0],
        "Error: Unknown Export command: \"notareport\"",
        "{:?}",
        dss.errors()
    );

    // (4) The ambiguous `elem` prefix resolves *earliest-wins* like the oracle's
    //     `ExportCommands` — `ElemCurrents`(42), not `ElemVoltages`(43)/
    //     `ElemPowers`(44) — and is now a **real** report (WP8.2 step 2c), not a
    //     `NOT_PORTED` stub. On a solved circuit it emits `…_EXP_ElemCurrents.csv`
    //     with no error (the abbreviation-disambiguation coverage the golden's
    //     full keywords don't exercise; audit-tests WP8.2 step 2c).
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.t basekv=12.47 phases=3 bus1=src");
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command("solve");
    dss.command(&set_dp);
    dss.command("export elem");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert!(
        dss.last_result_file()
            .to_lowercase()
            .ends_with("exp_elemcurrents.csv"),
        "`export elem` should resolve to ElemCurrents: {:?}",
        dss.last_result_file()
    );
}

/// `Compile` moves the report `OutputDirectory` to the deck's directory (Pascal
/// `DoRedirect` calls `SetDataPath(DSS, CurrDir)` for Compile before *and* after
/// processing — `ExecHelper.pas:546/651`), so a default-named export lands next
/// to the compiled deck, an explicit *relative* filename resolves against the
/// DSS current dir (= the deck dir), and a nested Compile cannot leave the dirs
/// pointing at the inner deck. Plain `Redirect` moves neither. All four
/// behaviors oracle-verified (dss-python 0.15.7 probe, WP8.2 follow-up: the
/// original WP8.1 port kept `output_directory` pinned to the startup cwd and
/// resolved explicit names against the process cwd — files landed in the wrong
/// directory after any `Compile`).
#[test]
fn compile_moves_output_directory_redirect_does_not() {
    let root = std::env::temp_dir().join(format!("dss_outdir_{}", std::process::id()));
    let sub = root.join("sub");
    std::fs::create_dir_all(&sub).unwrap_or_else(|e| panic!("mkdir {}: {e}", sub.display()));
    let deck = "new circuit.t basekv=12.47 phases=3 bus1=src\n\
                set voltagebases=[12.47]\n\
                calcvoltagebases\n";
    let master = root.join("master.dss");
    std::fs::write(&master, deck).unwrap();

    // (1) Default-named export after Compile → next to the deck.
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!("compile \"{}\"", master.display()));
    dss.command("export counts");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let produced = PathBuf::from(dss.last_result_file());
    assert_eq!(
        produced.parent(),
        Some(root.as_path()),
        "default-named export must land in the deck dir"
    );

    // (2) Explicit *relative* filename → resolved against the deck dir too.
    dss.command("export counts relcounts.csv");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(
        PathBuf::from(dss.last_result_file()),
        root.join("relcounts.csv"),
        "explicit relative export filename must resolve against the DSS dir"
    );

    // (3) A nested Compile inside the outer deck: the outer Compile's exit
    //     re-assert (Pascal's `finally SetDataPath(CurrDir)`) wins.
    std::fs::write(sub.join("inner.dss"), deck).unwrap();
    std::fs::write(root.join("outer.dss"), "compile \"sub/inner.dss\"\n").unwrap();
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!("compile \"{}\"", root.join("outer.dss").display()));
    dss.command("export counts");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(
        PathBuf::from(dss.last_result_file()).parent(),
        Some(root.as_path()),
        "after a nested Compile the outer deck dir must be re-asserted"
    );

    // (4) Plain Redirect does NOT move the output dir: route it somewhere known
    //     first (`Set DataPath=`), redirect a deck from another dir, export —
    //     the file stays in the datapath dir.
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!("set datapath=\"{}\"", sub.display()));
    dss.command(&format!("redirect \"{}\"", master.display()));
    dss.command("export counts");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(
        PathBuf::from(dss.last_result_file()).parent(),
        Some(sub.as_path()),
        "Redirect must not move the report OutputDirectory"
    );

    std::fs::remove_dir_all(&root).ok();
}

/// `Export Faultstudy` on a **snapshot** circuit (a plain `solve`, no
/// `solve mode=faultstudy`): the per-bus `Ysc` is `None`, so the 1-phase and L-L
/// columns take the `if let Some(ysc)` *else* path and stay `0.00`, the same
/// degenerate output the oracle produces (`ExportFaultStudy` reads precomputed
/// state — "Isc has been previously computed" — and garbage/zeros it if none ran).
/// This guards that `Ysc == None` branch (the `golden_reports` fixture always runs
/// a faultstudy first, so it never exercises it; audit-tests WP8.3 step 3b). No
/// oracle capture needed — the zeros are structural (`max_1ph`/`max_ll` never
/// leave their `0.0` init), so a Rust-only assertion is faithful and non-vacuous.
#[test]
fn export_faultstudy_snapshot_ysc_none_is_zeroed() {
    let dir = std::env::temp_dir().join(format!("dss_fs_snap_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("mkdir {}: {e}", dir.display()));

    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.t basekv=12.47 phases=3 bus1=src mvasc3=20000 mvasc1=21000");
    dss.command(
        "new line.l1 bus1=src bus2=b length=1 units=km r1=0.1 x1=0.3 r0=0.3 x0=0.9 c1=0 c0=0",
    );
    dss.command("new load.ld bus1=b phases=3 kv=12.47 kw=500 pf=0.95 model=1");
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command("solve"); // snapshot only — NOT `mode=faultstudy`
    dss.command(&format!("set datapath=\"{}\"", dir.display()));
    dss.command("export faultstudy");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    let produced = dss.last_result_file();
    assert!(
        produced.to_lowercase().ends_with("exp_faults.csv"),
        "unexpected produced path: {produced:?}"
    );
    let content =
        std::fs::read_to_string(produced).unwrap_or_else(|e| panic!("read {produced}: {e}"));

    let mut lines = content.lines();
    assert_eq!(lines.next(), Some("Bus,  3-Phase,  1-Phase,  L-L"));
    let mut rows = 0;
    for line in lines.filter(|l| !l.trim().is_empty()) {
        rows += 1;
        let cols: Vec<&str> = line.split(',').map(|c| c.trim()).collect();
        assert_eq!(cols.len(), 4, "row must have 4 fields: {line:?}");
        // The 1-phase (col 2) and L-L (col 3) columns are structurally 0.00 when
        // no faultstudy ran (Ysc == None). The 3-phase column (col 1) reads the
        // zeroed `bus_current` — also 0.00 on a snapshot — but the load-bearing
        // guard is the None-branch producing exactly `0.00`, not garbage/panic.
        assert_eq!(cols[2], "0.00", "1-phase must be 0.00 (Ysc None): {line:?}");
        assert_eq!(cols[3], "0.00", "L-L must be 0.00 (Ysc None): {line:?}");
    }
    assert!(rows >= 2, "expected the src+b buses, got {rows} rows");

    std::fs::remove_dir_all(&dir).ok();
}

/// `Save` still records a scoped `NOT_PORTED` (WP8.5 step 2); `Dump`'s
/// single-object form is ported (WP8.5 step 1) — its whole-circuit / `solution` /
/// aux forms stay scoped for step 3.
#[test]
fn save_records_scoped_not_ported() {
    let mut dss = Dss::new();
    dss.command("new circuit.t basekv=12.47 phases=3 bus1=src");
    dss.command("save circuit");
    assert_eq!(dss.errors().len(), 1, "{:?}", dss.errors());
    assert!(dss.errors()[0].contains("Save"), "{:?}", dss.errors());
}

/// The **generic-base ordering** for the two `Dump` element kinds that have no
/// byte golden yet (their property names are not yet in the oracle display case —
/// tracked for the systematic name pass), pinned structurally by line position so
/// the `dump_generic` logic can't silently reorder (WP8.5 audit-tests #2). The
/// orderings are oracle-probe-confirmed: a **PCElement** (Load) writes
/// `! ENABLED` + (debug) Y-block + `! VARIABLES` **before** its `~ prop` lines;
/// a non-PC **CktElement** (CapControl) writes its `~ prop` lines **before**
/// `! ENABLED`. Plain-`TDSSObject` order is byte-pinned by `dump_loadshape`.
#[test]
fn dump_generic_base_ordering() {
    let dir = std::env::temp_dir().join("dss_dump_generic_order");
    std::fs::create_dir_all(&dir).ok();
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.gord basekv=12.47 bus1=src");
    dss.command("new line.l1 bus1=src bus2=b r1=0.1 x1=0.1");
    dss.command("new load.ld1 bus1=b kv=12.47 kw=100");
    dss.command("new capacitor.c1 bus1=b kv=12.47 kvar=200");
    dss.command(
        "new capcontrol.cc1 element=line.l1 capacitor=c1 type=current \
         ptratio=1 ctratio=1 onsetting=10 offsetting=5",
    );
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command("solve");
    dss.command(&format!("set datapath=\"{}\"", dir.display()));
    assert!(dss.errors().is_empty(), "setup: {:?}", dss.errors());

    let dump = |dss: &mut Dss, cmd: &str| -> Vec<String> {
        dss.command(cmd);
        assert!(dss.errors().is_empty(), "{cmd}: {:?}", dss.errors());
        std::fs::read_to_string(dss.last_result_file())
            .unwrap()
            .lines()
            .map(str::to_string)
            .collect()
    };
    let pos = |lines: &[String], pred: &dyn Fn(&str) -> bool| -> usize {
        lines.iter().position(|l| pred(l)).expect("line present")
    };

    // PCElement (Load), debug: ENABLED + VARIABLES both precede the first `~` prop.
    let load = dump(&mut dss, "dump load.ld1 debug");
    let en = pos(&load, &|l| l == "! ENABLED");
    let vars = pos(&load, &|l| l == "! VARIABLES");
    let first_prop = pos(&load, &|l| l.starts_with("~ "));
    assert!(en < vars && vars < first_prop, "PC order: {load:?}");

    // non-PC CktElement (CapControl): the first `~` prop precedes `! ENABLED`.
    let cc = dump(&mut dss, "dump capcontrol.cc1");
    let cc_en = pos(&cc, &|l| l == "! ENABLED");
    let cc_first_prop = pos(&cc, &|l| l.starts_with("~ "));
    assert!(cc_first_prop < cc_en, "non-PC order: {cc:?}");

    std::fs::remove_dir_all(&dir).ok();
}

/// `Dump <class>.<name>` error fidelity: an unknown class → Pascal `#903`
/// (`SetObjectClass` fail); a known class + unknown object → `#256`
/// (`Object … not found`). `dump solution` (whole-circuit family, WP8.5 step
/// 3b) succeeds and writes the `! OPTIONS` listing (byte fidelity is pinned by
/// the `dump3_solution` golden).
#[test]
fn dump_single_object_errors() {
    let dir = std::env::temp_dir().join("dss_dump_errors_test");
    std::fs::create_dir_all(&dir).unwrap();
    let mut dss = Dss::new();
    dss.command("new circuit.t basekv=12.47 phases=3 bus1=src");
    dss.command(&format!("set datapath=\"{}\"", dir.display()));
    // Errors accumulate across commands (only `clear` resets them), so check by
    // cumulative index.
    dss.command("dump badclass.foo"); // #903 SetObjectClass fail
    dss.command("dump reactor.foo"); // #256 known class, unknown object
    dss.command("dump solution"); // Solution.DumpProperties — no error
    let e = dss.errors();
    assert_eq!(e.len(), 2, "{e:?}");
    assert!(e[0].contains("Object Class"), "#903 expected: {e:?}");
    assert!(
        e[1].contains("Object \"foo\" not found"),
        "#256 expected: {e:?}"
    );
    let produced = std::fs::read_to_string(dss.last_result_file()).unwrap();
    assert!(produced.starts_with("! OPTIONS\n"), "{produced:?}");
    std::fs::remove_dir_all(&dir).ok();
}
