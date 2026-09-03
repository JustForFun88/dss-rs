//! Phase 8 report-router skeleton tests (WP8.1): the `Plot`/`Visualize`
//! headless no-op (PHASE8_PLAN §2.5), the faithful `Show` no-op that keeps the
//! `solvable_now` decks green, and the scoped `NOT_PORTED` stubs for the not-yet
//! ported `Export`/`Save`/`Dump` verbs.

use std::path::PathBuf;

use crate::exec::*;
use num_complex::Complex64;

/// A small solved 3-phase feeder. Returns `(name, currents)` per element so two
/// runs can be compared for *bit-identical* model state.
fn solve_currents(extra: &[&str]) -> Vec<(String, Vec<Complex64>)> {
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

/// `Plot` is a GUI command whose headless dispatch (`DoPlotCmd`,
/// `PlotOptions.pas:202-213`) exits before any guard — an exact no-op: the
/// solved model is bit-identical with and without it (PHASE8_PLAN §2.5).
#[test]
fn plot_visualize_are_headless_noops() {
    let base = solve_currents(&[]);
    assert!(!base.is_empty(), "fixture produced no elements to compare");
    let with_plot = solve_currents(&["plot type=circuit quantity=power", "plot daisy"]);
    assert_eq!(base, with_plot, "Plot changed the solved model");
}

/// `Visualize` (`DoVisualizeCmd`, `ExecHelper.pas:4099`) runs its guards
/// before the NIL-callback no-op, so they are engine-observable: #24722 on an
/// unsolved circuit (oracle-probed, WP8.1 audit), #282 not-found — including a
/// general-object reference (dead wrong-type arm: `Handle = 0` upstream). A
/// valid element reference on a solved circuit is a clean no-op.
#[test]
fn visualize_guards_match_oracle() {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.t basekv=12.47 phases=3 bus1=src mvasc3=20000 mvasc1=21000");
    dss.command(
        "new line.l1 bus1=src bus2=b length=1 units=km r1=0.1 x1=0.3 r0=0.3 x0=0.9 c1=0 c0=0",
    );
    dss.command("new load.ld bus1=b phases=3 kv=12.47 kw=500 pf=0.95 model=1");

    // Unsolved (NodeV unallocated — before even `calcvoltagebases`, whose
    // zero-load snapshot already allocates NodeV, same as Pascal) → #24722.
    dss.command("visualize element=line.l1");
    assert!(
        dss.errors()
            .iter()
            .any(|e| e.contains("must be solved before")),
        "unsolved Visualize must error #24722, got {:?}",
        dss.errors()
    );

    dss.errors.clear();
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    // Solved + existing circuit element → clean no-op.
    dss.command("visualize what=voltage element=line.l1");
    assert!(
        dss.errors().is_empty(),
        "valid Visualize must be a clean no-op, got {:?}",
        dss.errors()
    );

    // Missing element / bare command / general-object reference → #282.
    for cmd in [
        "visualize element=line.nope",
        "visualize",
        "visualize element=loadshape.default",
    ] {
        dss.errors.clear();
        dss.command(cmd);
        assert!(
            dss.errors()
                .iter()
                .any(|e| e.contains("Requested Circuit Element")),
            "{cmd:?} must error #282 not-found, got {:?}",
            dss.errors()
        );
    }
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

/// The export router's non-formatting outcomes: the WP8.2 solution guard
/// (#24712), the retired CDPSM profiles' fixed upstream message (#252), an
/// unknown keyword's Pascal 24713, and earliest-wins abbreviation resolution.
/// The *happy* path (a real report written + diffed vs the oracle) is the
/// `golden_reports.rs` gate. (`Export` decks all sit in `skipped_unsupported`,
/// so this never reaches the live gate.)
///
/// The router's `_ =>` "not ported yet" arm is now **unreachable** — every one of
/// the 64 `EXPORT_OPTIONS` keywords has a dispatch arm (`Estimation`(5), the last
/// holdout, landed with ORPHANED_GAPS §1.10). It stays as the safety valve for a
/// keyword added to the table without a route, and is therefore **untested by
/// construction**: a newly added keyword left unrouted lands on a path no test
/// exercises, so route it in the same change that adds it to `EXPORT_OPTIONS`.
#[test]
fn export_router_outcomes_match_oracle() {
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

    // (2) The retired CDPSM (CIM16) profile exports. Upstream deleted the four
    //     exporters plus `CDPSMAsset` and left each dispatch arm as a fixed
    //     `DoSimpleMsg(..., 252)` (`ExportOptions.pas:543` for 22, `:555-561` for
    //     28-31; identical in r4133 `Version8/Source/Executive/ExportOptions.pas
    //     :461`/`:467-470`). The keyword still resolves, so it passes the solve
    //     guard (all five are in the #24712 `1..24 | 28..32` set — hence the solve)
    //     and then emits exactly this message and writes no file. But `AbortExport`
    //     stays FALSE (only the unknown-keyword `else`, `:626`, sets it), so
    //     `DoExportCmd`'s tail (`:632-637`, r4133 `:514-516`) DOES overwrite
    //     `LastResultFile`/`@lastfile`/`@lastexportfile` with the empty default
    //     name resolved against the output directory — i.e. `<datapath>\t_`.
    //     Pinned live against the 0.14.5 oracle (`export cdpsmasset` after an
    //     `export voltages`: both vars go from `…\t_EXP_VOLTAGES.csv` to `…\t_`).
    //     (`Estimation`(5) used to be this case's NOT_PORTED example; it is now a
    //     real export gated by `golden_reports.rs::export_estimation_*`.)
    let dp = std::env::temp_dir();
    for (keyword, profile) in [
        ("cdpsmasset", "Asset"),
        ("cdpsmelec", "ElectricalProperties"),
        ("cdpsmgeo", "Geographical"),
        ("cdpsmtopo", "Topology"),
        ("cdpsmstatevar", "StateVariables"),
    ] {
        let mut dss = Dss::new();
        dss.command("new circuit.t basekv=12.47 phases=3 bus1=src");
        dss.command("solve");
        dss.command(&set_dp);
        let expect = dp.join("t_").to_string_lossy().into_owned();
        assert_ne!(dss.last_result_file(), expect);
        dss.command(&format!("export {keyword}"));
        assert_eq!(
            dss.error_texts(),
            vec![format!(
                "{profile} export no longer supported; use Export CIM100"
            )],
            "export {keyword}"
        );
        assert_eq!(
            dss.last_result_file(),
            expect,
            "export {keyword}: DoExportCmd tail must set the last-file state"
        );
        assert_eq!(
            dss.vars.get("@lastfile").unwrap_or(""),
            expect,
            "export {keyword} @lastfile"
        );
        assert_eq!(
            dss.vars.get("@lastexportfile").unwrap_or(""),
            expect,
            "export {keyword} @lastexportfile"
        );
        assert!(
            !std::path::Path::new(&expect).exists(),
            "export {keyword} must not write a file"
        );
    }

    // (3) Unknown keyword → Pascal 24713, with the lowercased keyword echoed back.
    //     This is the *one* arm that sets `AbortExport := TRUE` (`:626`), so —
    //     unlike (2) — the last-file tail is skipped and the state stays put.
    //     (Same oracle probe: `export cdpsmtopology`/`export bogusnotakeyword`
    //     leave `@lastfile` at whatever the previous export set.)
    let mut dss = Dss::new();
    dss.command("new circuit.t basekv=12.47 phases=3 bus1=src");
    dss.command(&set_dp);
    let before = dss.last_result_file().to_string();
    dss.command("export notareport");
    assert_eq!(
        dss.error_texts()[0],
        "Error: Unknown Export command: \"notareport\"",
        "{:?}",
        dss.errors()
    );
    assert_eq!(
        dss.last_result_file(),
        before,
        "an unknown keyword aborts before DoExportCmd's last-file tail"
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

/// `Save circuit` is ported (WP8.5 step 5): it writes a re-compilable script
/// tree (`Master.dss` + a file per class) with no error, and reports the
/// directory in `GlobalResult`. (The full round-trip / structural gate lives in
/// the `save_roundtrip.rs` integration test.)
#[test]
fn save_circuit_writes_master() {
    let dir = std::env::temp_dir().join(format!("dss_save_min_{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    let mut dss = Dss::new();
    dss.command("new circuit.t basekv=12.47 phases=3 bus1=src");
    dss.command(&format!(
        "save circuit dir=\"{}\"",
        dir.to_string_lossy().replace('\\', "/")
    ));
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert!(dir.join("Master.dss").is_file(), "no Master.dss emitted");
    assert!(
        dss.result().contains("Circuit saved in directory"),
        "GlobalResult: {:?}",
        dss.result()
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// Every holder of [`crate::obj::props::PropFlags::RENDERS_LIVE_RESULT`] in the
/// **whole registry** is a class `Dss::refresh_live_result_cache` dispatches on
/// (RP3.8 audit settlement).
///
/// The dispatch is a hardcoded two-arm `if` over StorageController / IndMach012.
/// A third class given the flag would get no refresh at all and would render
/// whatever its cache last held — silently, since a missing arm is not a
/// compile error. The two per-class tests
/// (`ind_mach012::tests::only_pf_renders_live`,
/// `storage_controller::tests::only_the_four_aggregates_render_live`) each check
/// only their own property table, so nothing tied the flag's *holder set* to the
/// dispatch. This walks the registry the executive actually builds and pins that
/// set; the debug-build twin is the `debug_assert!` at the end of
/// `refresh_live_result_cache`, which fires on a holder that reaches no arm.
#[test]
fn renders_live_result_holders_have_a_refresh_arm() {
    use crate::obj::props::PropFlags;

    let dss = Dss::new();
    let mut holders: Vec<String> = Vec::new();
    for cls in &dss.classes {
        for i in 1..=cls.props.num_properties() {
            if cls
                .props
                .prop(i)
                .flags
                .contains(PropFlags::RENDERS_LIVE_RESULT)
            {
                holders.push(format!(
                    "{}.{}",
                    cls.props.class_name(),
                    cls.props.property_name(i)
                ));
            }
        }
    }
    holders.sort();
    assert_eq!(
        holders,
        [
            "IndMach012.PF",
            "StorageController.kWActual",
            "StorageController.kWTotal",
            "StorageController.kWhActual",
            "StorageController.kWhTotal",
        ],
        "the flag's holder set moved. Every holder must have an arm in \
         `Dss::refresh_live_result_cache` (and a `Save`-side pass through \
         `refresh_render_caches_for_save`); a new one needs both, plus its own \
         r4133-sourced pin — not just the flag"
    );
}

// RP3.8 EXPECTED-VALUE PIN [SAVE_RENDERS_LIVE_RESULTS]: `Save` renders every
// marked live property from the live model, and renders it the same whether or
// not the session read it first. Dropping either
// `Dss::refresh_render_caches_for_save` call site makes the assertions below
// fail on the construction default (`PF=1`, `kWhTotal=0`, an all-zero
// `WdgCurrents`) — or, worse, pass only after some earlier `?`.
/// `Save` is the **fifth** `ClassProps::get_value` reader, and it renders the
/// live result like the other four (RP3.8 audit settlement, 2026-09-02).
///
/// `Save` emits only the properties a deck explicitly **set**, in the order it
/// set them (Pascal `GetNextPropertySet` → `TDSSObject.SaveWrite`) — and a write
/// to one of these read-only properties is silently ignored *yet still marks the
/// property set*, so `New IndMach012.m1 … pf=0.5` really does get a `PF=` back.
/// r4133 fills that slot from the same live getter it uses for `?`; measured
/// through `epri-worker` on these very decks (2026-09-02) it writes
/// `New "IndMach012.m1" … pf=0.886059`,
/// `New "StorageController.sc" … kWhTotal=6000` and the solved
/// `Transformer.t1 … WdgCurrents=`.
///
/// Before the settlement the port wrote the render caches unrefreshed: `PF=1` /
/// `kWhTotal=0` / an all-zero `WdgCurrents` by default, and the live number only
/// when some earlier `?` had happened to refresh it — a *read-history-dependent*
/// saved deck, which is the contamination shape the corpus gate's own three-run
/// artifact exists to forbid. Leg (2) is that regression's tripwire.
///
/// Legs (1)-(4) are RP3.8's own [`crate::obj::props::PropFlags::
/// RENDERS_LIVE_RESULT`] properties; leg (5) is the same latency on the older
/// [`crate::obj::props::PropFlags::READS_VTERMINAL`] marker (Transformer
/// `WdgCurrents`), which pre-dates RP3.8 and is fixed with it — the `Save` pass
/// runs the same per-object choke point, so all four of its jobs happen.
///
/// The port renders full precision like every other double property; r4133's own
/// `%.6g` / `%-.8g` digits are checked through [`crate::util::fmt_g`], the same
/// two-clause display class the other RP3.8 pins use. (`WdgCurrents` is an
/// r4133-formatted string on both engines, so it is compared byte-for-byte.)
#[test]
fn save_renders_the_live_result_properties() {
    use crate::util::fmt_g;

    /// The ` <Name>=<value>` token `Save` wrote for `prop` on `line`.
    fn token(line: &str, prop: &str) -> String {
        line.split_whitespace()
            .find_map(|t| {
                let (k, v) = t.split_once('=')?;
                k.eq_ignore_ascii_case(prop).then(|| v.to_string())
            })
            .unwrap_or_else(|| panic!("no {prop}= in the saved line {line:?}"))
    }
    fn saved_line(dir: &std::path::Path, file: &str, needle: &str) -> String {
        let text = std::fs::read_to_string(dir.join(file)).unwrap_or_else(|e| {
            let listing: Vec<String> = std::fs::read_dir(dir)
                .map(|rd| {
                    rd.flatten()
                        .map(|x| x.file_name().to_string_lossy().into_owned())
                        .collect()
                })
                .unwrap_or_default();
            panic!(
                "read saved {file} from {}: {e} (dir holds {listing:?})",
                dir.display()
            )
        });
        text.lines()
            .find(|l| l.contains(needle))
            .unwrap_or_else(|| panic!("no {needle} line in {file}: {text:?}"))
            .to_string()
    }

    // The machine's deck writes `pf=0.5`, which the engine ignores (read-only)
    // while still marking the property set.
    let machine = |dss: &mut Dss| {
        for c in [
            "clear",
            "new circuit.rp38save basekv=12.47 pu=1.0 phases=3 bus1=src",
            "new indmach012.m1 bus1=src kV=12.47 kW=100 kVA=150 pf=0.5",
            "set voltagebases=[12.47]",
            "calcv",
            "solve",
        ] {
            dss.command(c);
        }
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    };
    let scratch = |tag: &str| {
        let dir = std::env::temp_dir().join(format!("dss_rp38_save_{tag}_{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dir
    };
    let save_circuit = |dss: &mut Dss, dir: &std::path::Path| {
        dss.command(&format!(
            "save circuit dir=\"{}\"",
            dir.to_string_lossy().replace('\\', "/")
        ));
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    };

    // (1) `Save circuit`, no prior read of `pf`.
    let mut dss = Dss::new();
    machine(&mut dss);
    let cold = scratch("cold");
    save_circuit(&mut dss, &cold);
    let cold_line = saved_line(&cold, "IndMach012.dss", "IndMach012.m1");
    std::fs::remove_dir_all(&cold).ok();

    let pf = token(&cold_line, "PF");
    let v: f64 = pf
        .parse()
        .unwrap_or_else(|e| panic!("Save writes a number for PF, got {pf:?}: {e}"));
    assert_eq!(
        fmt_g(v, 6),
        "0.886059",
        "r4133's own byte for this deck's saved `pf=` (epri-worker, 2026-09-02); \
         the pre-settlement port wrote the construction default `1`: {cold_line}"
    );
    assert_eq!(
        pf, "0.886059022116548",
        "…and the port writes the full double, not r4133's six digits"
    );

    // (2) The same deck, `?`-read first: `Save` must not depend on read history.
    let mut dss = Dss::new();
    machine(&mut dss);
    dss.command("? indmach012.m1.pf");
    let warm = scratch("warm");
    save_circuit(&mut dss, &warm);
    let warm_line = saved_line(&warm, "IndMach012.dss", "IndMach012.m1");
    std::fs::remove_dir_all(&warm).ok();
    assert_eq!(
        warm_line, cold_line,
        "a `?` before the save must not change what `Save` writes"
    );

    // (3) `Save <class>` is the other serializer entry point.
    let mut dss = Dss::new();
    machine(&mut dss);
    let one = scratch("class");
    dss.command(&format!(
        "save class=indmach012 file=IndMach012.dss dir=\"{}\"",
        one.to_string_lossy().replace('\\', "/")
    ));
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let class_line = saved_line(&one, "IndMach012.dss", "IndMach012.m1");
    std::fs::remove_dir_all(&one).ok();
    assert_eq!(
        token(&class_line, "PF"),
        pf,
        "`Save <class>` renders the same live value as `Save circuit`"
    );

    // (4) The StorageController half: a deck writing `kWhTotal=42` gets the live
    // fleet sum back — r4133's `kWhTotal=6000` on this deck.
    let mut dss = Dss::new();
    for c in [
        "clear",
        "new circuit.rp38savesc basekv=12.47 phases=3 bus1=src basefreq=60",
        "new line.l1 bus1=src bus2=b phases=3 r1=0.1 x1=0.3 c1=0 length=1 units=km",
        "new load.ld bus1=b phases=3 kv=12.47 kw=6000 pf=1.0 model=1",
        "new storage.sa bus1=b phases=3 kv=12.47 kwrated=1500 kva=1500 kwhrated=6000 \
         %stored=80 %idlingkw=0 pf=1.0",
        "new storagecontroller.sc element=line.l1 terminal=1 modedis=peakshave \
         kwtarget=5200 kWhTotal=42",
        "set voltagebases=[12.47]",
        "calcv",
        "solve",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let sc_dir = scratch("sc");
    save_circuit(&mut dss, &sc_dir);
    let sc_line = saved_line(&sc_dir, "StorageController.dss", "StorageController.sc");
    std::fs::remove_dir_all(&sc_dir).ok();
    let total = token(&sc_line, "kWhTotal");
    assert_eq!(
        total, "6000",
        "the live fleet `kWhRating` sum — r4133's own saved byte on this deck \
         (epri-worker, 2026-09-02); the deck's ignored `kWhTotal=42` and the \
         pre-settlement `0` are both wrong: {sc_line}"
    );

    // (5) The same latency on the older `READS_VTERMINAL` marker, which
    // pre-dates RP3.8 and is fixed with it: a deck writing `wdgcurrents=` gets
    // the SOLVED winding currents back from r4133, and an all-zero buffer from
    // the pre-settlement port (the `&self` getter read a `Vterminal` nothing had
    // reloaded).
    let mut dss = Dss::new();
    for c in [
        "clear",
        "new circuit.rp38savewdg basekv=115 phases=3 bus1=src",
        "new transformer.t1 windings=2 buses=[src, b] conns=[wye, wye] \
         kvs=[115, 12.47] kvas=[10000, 10000] xhl=7 wdgcurrents=1",
        "new load.ld bus1=b phases=3 kv=12.47 kw=8000 pf=0.95 model=1",
        "set voltagebases=[115, 12.47]",
        "calcv",
        "solve",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let wdg_dir = scratch("wdg");
    save_circuit(&mut dss, &wdg_dir);
    let wdg_line = saved_line(&wdg_dir, "Transformer.dss", "Transformer.t1");
    std::fs::remove_dir_all(&wdg_dir).ok();
    let want = concat!(
        "WdgCurrents=\"43.41225, (-21.639), 400.3534, (158.36), ",
        "43.41225, (-141.64), 400.3534, (38.361), ",
        "43.41225, (98.361), 400.3534, (-81.639), \""
    );
    assert!(
        wdg_line.contains(want),
        "r4133's own saved `WdgCurrents=` on this deck (epri-worker, 2026-09-02); \
         the pre-settlement port saved an all-zero buffer: {wdg_line}"
    );
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

/// `dump solution` renders `Set LDCurve=<name>` from the `Set LDCurve=`
/// LoadShape (GAPS WPG.3; Pascal `Solution.pas:1816` `NameIfNotNil`) — the
/// render was hardcoded empty before the WPG.17 sweep. The unset (empty) case
/// stays byte-pinned by the `dump3_solution` golden. (The name renders in the
/// port's lowercase-normalized storage form — the identifier-case convention;
/// the comparator treats identifier case as non-significant.)
#[test]
fn dump_solution_renders_ldcurve_name() {
    let dir = std::env::temp_dir().join(format!("dss_dump_ldcurve_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut dss = Dss::new();
    dss.command("new circuit.t basekv=12.47 phases=3 bus1=src");
    dss.command("new loadshape.ldc npts=2 interval=1 mult=[1 0.5]");
    dss.command("set ldcurve=ldc");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss.command(&format!("set datapath=\"{}\"", dir.display()));
    dss.command("dump solution");
    let produced = std::fs::read_to_string(dss.last_result_file()).unwrap();
    assert!(
        produced.contains("Set LDCurve=ldc\n"),
        "LDCurve name must render: {produced:?}"
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// RP3.11 P1 — `Save circuit` writes `CalcVoltageBases` **uncommented**, the way
/// r4133 does (`Version8/Source/Common/Circuit.pas:2716-2740`:
/// `Writeln(F,'Set Voltagebases='+VBases); Writeln(F, 'CalcVoltagebases');`,
/// two plain `Writeln`s with no flag between them). dss_capi 0.14.5 is the only
/// engine that ever writes `! CalcVoltageBases`
/// (`.inputs/dss_capi/src/Common/Circuit.pas:2708-2731`) and only outside its
/// own `DSS_CAPI_NOCOMPATFLAGS` branch, i.e. as the compat-flag side of a flag
/// this port hard-codes to the ON side; the port followed 0.14.5 until RP3.11.
///
/// The comment is not cosmetic: without the `CalcVoltageBases` call the
/// re-compiled tree carries `kVBase = 0` on **every** bus, which is what this
/// test pins on the emitted circuit. RP3.11 measured the consequence on
/// `modes/ncim/ncim_pv_pq.dss`: `bus_kvbase(genbus)` 0.0 instead of
/// 7.199557856794634, the deck NOT CONVERGED at 15 iterations; and on
/// `controls/expcontrol/expcontrol_basic.dss` the PV moved from -0.0060 kvar /
/// 14 iterations to +307.94 kvar / 53 — per-unit-driven controls read those
/// bases (`tmp/rp311/out_bisect.txt` §A, §B: swapping this one file into
/// r4133's own otherwise-exact save is what breaks the round trip, on either
/// engine).
#[test]
fn save_writes_calcvoltagebases_like_r4133() {
    let dir = std::env::temp_dir().join(format!("dss_rp311_vbases_{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    let mut dss = Dss::new();
    for c in [
        "clear",
        "new circuit.rp311vb basekv=12.47 pu=1.0 phases=3 bus1=src",
        "new line.l1 bus1=src bus2=b length=1 units=km r1=0.1 x1=0.3 c1=0 c0=0",
        "new load.ld1 bus1=b phases=3 kv=12.47 kw=500 pf=0.95",
        "set voltagebases=[12.47]",
        "calcv",
        "solve",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss.command(&format!(
        "save circuit dir=\"{}\"",
        dir.to_string_lossy().replace('\\', "/")
    ));
    assert!(dss.errors().is_empty(), "save errors: {:?}", dss.errors());

    // r4133's two lines, ours: identical but for the property-name spelling
    // (`Set Voltagebases`/`CalcVoltagebases` there, `Set VoltageBases`/
    // `CalcVoltageBases` here — one spelling per name, re-parsed
    // case-insensitively).
    let text = std::fs::read_to_string(dir.join("BusVoltageBases.dss")).expect("BusVoltageBases");
    assert_eq!(
        text, "Set VoltageBases=(12.47, )\nCalcVoltageBases\n",
        "emitted BusVoltageBases.dss"
    );
    assert!(
        !text.contains("! CalcVoltageBases"),
        "the 0.14.5 compat-flag comment must be gone: {text:?}"
    );

    // The line has to *do* something: re-compiling the emitted tree restores a
    // non-zero base on every bus. Without it every `kv_base` reads 0.0.
    let mut back = Dss::new();
    back.command(&format!(
        "compile \"{}\"",
        dir.join("Master.dss").to_string_lossy().replace('\\', "/")
    ));
    assert!(
        back.errors().is_empty(),
        "re-compile errors: {:?}",
        back.errors()
    );
    let ckt = back.circuit.as_ref().expect("circuit");
    let ib = ckt.bus_list.find("b").expect("bus b");
    assert_eq!(
        ckt.buses[ib].kv_base,
        12.47 / f64::sqrt(3.0),
        "re-compiled bus kVBase (L-N); 0.0 is the `! CalcVoltageBases` symptom"
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// RP3.11 P2 — `TDSSObject.SaveWrite`'s LoadShape branch (r4133
/// `Version8/Source/General/DSSObject.pas:139-150` + `:163-172`, r4133-only:
/// neither dss_capi 0.14.5 nor this port had it). `npts` sizes the `Mult`/
/// `Hour` allocation on reload, so the Pascal starts the walk at property 1
/// instead of at the head of the chain and then restarts the chain ignoring
/// index 1 — *"created to guarantee that the npts property will be the first to
/// be declared when saving LoadShapes"*.
///
/// Both orders are pinned, because only the second one moved: `ls1` types
/// `npts` first (this port already emitted it first), `ls2` re-sets it last and
/// used to save as `Interval=1 Mult=[ 1 2 3] NPts=3` — a line that reloads a
/// 0-point shape and drops the multipliers. r4133 saves **both** as
/// `New "LoadShape.lsN" npts=3 interval=1 mult=[ 1 2 3]` (epri-worker probe,
/// OpenDSSDirect.dll 11.0.0.1 r4133, RP3.11 I1); the port writes the same
/// tokens under its own property-name spelling. The `matches("NPts=") == 1`
/// leg is the load-bearing one: r4133 reaches it through
/// `TLoadShapeObj.Set_NumPoints` re-stamping `PropertyValue[1]`
/// (`LoadShape.pas:1665-1677`, called at `:631-636` *"Keep Properties in order
/// for save command"*), which this port does not do, so the skip has to cover
/// the restart as well.
#[test]
fn save_write_puts_npts_first_for_loadshape() {
    let dir = std::env::temp_dir().join(format!("dss_rp311_lshp_{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    let mut dss = Dss::new();
    for c in [
        "clear",
        "new circuit.rp311ls basekv=12.47 pu=1.0 phases=3 bus1=src",
        // npts typed FIRST
        "new loadshape.ls1 npts=3 interval=1 mult=[1 2 3]",
        // npts re-set LAST: its sequence stamp moves to the end of the chain
        "new loadshape.ls2 npts=3 interval=1 mult=[1 2 3]",
        "edit loadshape.ls2 npts=3",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss.command(&format!(
        "save circuit dir=\"{}\"",
        dir.to_string_lossy().replace('\\', "/")
    ));
    assert!(dss.errors().is_empty(), "save errors: {:?}", dss.errors());

    let text = std::fs::read_to_string(dir.join("LoadShape.dss")).expect("LoadShape.dss");
    for name in ["LoadShape.ls1", "LoadShape.ls2"] {
        let line = text
            .lines()
            .find(|l| l.contains(name))
            .unwrap_or_else(|| panic!("no {name} line in {text:?}"));
        assert_eq!(
            line,
            format!("New \"{name}\" NPts=3 Interval=1 Mult=[ 1 2 3]"),
            "r4133 writes `New \"{name}\" npts=3 interval=1 mult=[ 1 2 3]`"
        );
        assert_eq!(
            line.matches("NPts=").count(),
            1,
            "npts must be written exactly once: {line:?}"
        );
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// RP3.11 §1 — **the `Save` verdict: KEEP_LIVE_PINNED.** `Save` renders the
/// *live* field, and the divergence from r4133's serializer is recorded here
/// instead of being reproduced.
///
/// Deck: `tests/corpus/modes/ncim/ncim_pv_pq.dss` — a model-3 (PV) generator
/// whose reactive target exceeds `maxkvar`, so the NCIM solve clamps Q and
/// converts the machine to model 4 (PQ). After the converged solve the two
/// engines serialize the same object differently. Both lines are measured
/// (2026-09-03, `epri-worker` / `OpenDSSDirect.dll` 11.0.0.1 rev r4133 against
/// this port on the same deck; transcripts
/// `tmp/rp311/out/{r4133,port}_ncim/save/Generator.dss`):
///
/// ```text
/// r4133: New "Generator.g1" bus1=genbus phases=3 kv=12.47 kW=800 model=3 maxkvar=1500 minkvar=-1500 Vpu=1.01
/// port : New "Generator.g1" PF=0.88 Bus1=genbus Phases=3 kV=12.47 kW=800 Model=4 Maxkvar=1500 Minkvar=-1500 Vpu=1.01
/// ```
///
/// **Why the port does not match, and must not.** `TDSSObject.SaveWrite` writes
/// `PropertyValue[iProp]` (`R4133:General/DSSObject.pas:156`), which is *not* a
/// parse store: it resolves to the **virtual** `GetPropertyValue` (`:45`,
/// `:117-120` — *"This is virtual function that may call routine"*), and 49
/// classes of the live `Version8/Source` tree override it to answer live on a
/// hand-picked index set.
/// `TGeneratorObj.GetPropertyValue` (`R4133:PCElements/generator.pas:3007-3038`)
/// has arms for 3 `kv`, 4 `kW`, 5 `pf`, 13 `kvar`, 19/20 `maxkvar`/`minkvar`,
/// 26/27 `kVA`/`MVA`, 34/36 and 37-46 — and **no arm 6**, which is the only
/// reason `model` falls through to the parsed token `3`. Storage's arm list
/// *contains* `propMODEL` (`R4133:PCElements/Storage.pas:1525-1596`): the same
/// property, the opposite treatment, in the same engine. The live/store split is
/// an artifact of which arms each class's author happened to write, not a rule.
///
/// So matching r4133 here means printing `model=3` while `gen_model == 4` in the
/// same process — reproducing the echo as *behaviour*, which the 2026-08-02
/// policy forbids. RP3.11's **kill criterion fires**, and `Save` keeps the live
/// render it shares with `Dump`, the property API, `?` and batchedit through the
/// one [`crate::obj::props::ClassProps`]`::get_value`. The compare-side twin of
/// this very cell is already excluded and pinned: the `generator.model` echo row
/// (`harness::props_norm::PROPS_ECHO_R4133`) →
/// `props_r4133_pins::generator_model_renders_the_live_pv2pq_conversion`.
///
/// **The cost, recorded rather than dressed up** (RP3.11 §1.4,
/// `tmp/rp311/out_bisect.txt` §A/§C1): `Save` renders live values over an
/// *authored* membership, so on a property the solve mutates it reproduces
/// neither the authored problem nor the full solved state. Re-compiling the
/// port's line yields a PQ machine at pf 0.88 — Q = 800·tan(acos 0.88) =
/// 431.79 kvar instead of the authored PV machine clamped at 1500 kvar — and
/// `|V| genbus.1` moves 7213.235350 → 7161.277193 V (both engines reproduce each
/// other's numbers on the substituted deck). The same hybrid is inherent to
/// r4133's 49 partial getters; the port's version is the one that never prints a
/// value the engine knows to be superseded.
///
/// The divergence runs both ways on this surface: r4133's own `SaveWrite` emits
/// `kvar=0` for a `windgen`, which on reload flattens `PFNominal` to 1.0 and
/// `kvarMax`/`kvarMin` to 0 (`tests/props_r4133_replay.rs:1197-1199`) — an
/// upstream defect the port does not have.
#[test]
fn save_renders_the_live_model_after_ncim_pv2pq() {
    let deck = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/modes/ncim/ncim_pv_pq.dss");
    assert!(deck.is_file(), "vendored corpus deck missing: {deck:?}");
    let dir = std::env::temp_dir().join(format!("dss_rp311_ncimsave_{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();

    let mut dss = Dss::new();
    dss.command(&format!("compile \"{}\"", deck.display()));
    assert!(dss.errors().is_empty(), "ncim_pv_pq: {:?}", dss.errors());

    // What the engine knows: the NCIM solve converted the machine to PQ.
    dss.command("? generator.g1.model");
    assert_eq!(
        dss.result(),
        "4",
        "the deck authors model=3; the converged NCIM solve clamps Q at maxkvar \
         and converts the generator to model 4"
    );

    dss.command(&format!(
        "save circuit dir=\"{}\"",
        dir.to_string_lossy().replace('\\', "/")
    ));
    assert!(dss.errors().is_empty(), "save errors: {:?}", dss.errors());
    let text = std::fs::read_to_string(dir.join("Generator.dss")).expect("Generator.dss");
    std::fs::remove_dir_all(&dir).ok();
    let line = text
        .lines()
        .find(|l| l.contains("Generator.g1"))
        .unwrap_or_else(|| panic!("no Generator.g1 line in {text:?}"));

    assert_eq!(
        line,
        "New \"Generator.g1\" PF=0.88 Bus1=genbus Phases=3 kV=12.47 kW=800 Model=4 \
         Maxkvar=1500 Minkvar=-1500 Vpu=1.01",
        "the port's serialization; r4133 writes `New \"Generator.g1\" bus1=genbus \
         phases=3 kv=12.47 kW=800 model=3 maxkvar=1500 minkvar=-1500 Vpu=1.01` \
         (measured), which is the parsed token for a machine the same process \
         already converted to model 4"
    );
    assert!(
        !line.to_ascii_lowercase().contains("model=3"),
        "printing r4133's stale `model=3` is the kill criterion: {line:?}"
    );
}

/// RP3.11 §2 — **the `Dump` verdict: KEEP_LIVE_PINNED**, for `Save`'s reasons
/// plus one of its own.
///
/// `Dump` renders through the same [`crate::obj::props::ClassProps`]`::get_value`
/// as `Save`, the property API, `?` and batchedit, so every argument of
/// [`save_renders_the_live_model_after_ncim_pv2pq`] applies unchanged — and
/// `TGeneratorObj.DumpProperties` (`R4133:PCElements/generator.pas:2489-2500`)
/// walks **all** properties with no `PrpSequence` filter, so answering the parse
/// store here would land on all 88 committed `dump*` artifacts (44 `.txt` + 44
/// `.meta.json`, 162 lines), every one of them captured from an oracle that
/// renders live.
///
/// Measured on `tests/corpus/modes/ncim/ncim_pv_pq.dss` after the converged NCIM
/// solve (2026-09-03; `tmp/rp311/out/{r4133,port}_ncim/dump_Generator_g1.txt`),
/// the store-vs-live axis is exactly **one** of the ten differing rows:
///
/// ```text
/// r4133: ~ pf=0.88     ~ kvar=431.794           ~ model=3
/// port : ~ PF=0.88     ~ kvar=431.79425771047   ~ Model=4
/// ```
///
/// (The other nine rows belong to axes with their own owners: r4133's deleted
/// `DumpProperties` overrides — `!DQDV=`, the double-paren wrap, `Refuel=False`,
/// itself an r4133 bug against its own getter `generator.pas:2495` vs `:3028` —
/// the RP3 name census, `fmt_g` float spelling and header quoting.)
#[test]
fn dump_renders_the_live_model_after_ncim_pv2pq() {
    let deck = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/modes/ncim/ncim_pv_pq.dss");
    assert!(deck.is_file(), "vendored corpus deck missing: {deck:?}");
    let dir = std::env::temp_dir().join(format!("dss_rp311_ncimdump_{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();

    let mut dss = Dss::new();
    dss.command(&format!("compile \"{}\"", deck.display()));
    assert!(dss.errors().is_empty(), "ncim_pv_pq: {:?}", dss.errors());
    dss.command(&format!("set datapath=\"{}\"", dir.display()));
    dss.command("dump generator.g1");
    assert!(dss.errors().is_empty(), "dump errors: {:?}", dss.errors());
    let produced = std::fs::read_to_string(dss.last_result_file()).unwrap();
    std::fs::remove_dir_all(&dir).ok();

    assert!(
        produced.contains("~ Model=4\n"),
        "`Dump` renders the live model; r4133 writes `~ model=3` here: {produced:?}"
    );
    assert!(
        !produced.to_ascii_lowercase().contains("~ model=3"),
        "r4133's stale `~ model=3` must not be reproduced: {produced:?}"
    );
    // The live `kvar` the same getter answers, for context: r4133 prints the
    // same number under its `%.6g` display (`~ kvar=431.794`).
    assert!(
        produced.contains("~ kvar=431.79425771047\n"),
        "the live dispatched kvar: {produced:?}"
    );
}

/// RP3.11 §3 — the `PF=0.88`-class item: **a membership (sequence) difference,
/// explained, not a port bug.** `Save`'s *values* are live (§1); its *membership
/// and order* are the explicitly-set chain, and that chain differs from r4133's
/// because this port carries dss_capi 0.14.5's **property tracking**, which
/// r4133 has no counterpart for.
///
/// ```text
/// r4133: New "Generator.g1" bus1=genbus phases=3 kv=12.47 kW=800 model=3 …   (no `pf` at all)
/// port : New "Generator.g1" PF=0.88 Bus1=genbus Phases=3 kV=12.47 kW=800 …   (`PF` first)
/// ```
///
/// **Mechanism.** In r4133 `PrpSequence` is stamped **only** by
/// `Set_PropertyValue` (`R4133:General/DSSObject.pas:213-221`) — the parser
/// storing a token — and `InitPropertyValues` ends in `ClearPropSeqArray`
/// (`:122-129` → `:62-69`), so no constructor mark can survive; `SetAsNextSeq`
/// does not exist in r4133 (0 hits over `Version8/Source`). dss_capi 0.14.5
/// introduced it as its documented property-tracking feature — 124 call sites —
/// guarded upstream by `DSSCompatFlag.NoPropertyTracking`
/// (`CAPI:PCElements/Generator.pas:953-958`), whose OFF state upstream documents
/// as *"following the original OpenDSS implementation"*. This port carries the
/// six creation seeds verbatim ([`crate::elements::pc::generator`] `mod.rs:587-588`
/// and the PVSystem/Storage/VSource/Fault/Transformer twins), and seeds take
/// sequence 1..n — which is exactly why `PF` is printed first.
///
/// **Why they stay** (RP3.11 §3.2, the evidence the earlier survey missed): the
/// same bitmap has a second product reader, the AltDSS JSON export
/// (`report/export/json/build.rs:61-70` ← `CAPI_Obj.pas:665-733`), and *there*
/// dss_capi is not an outdated oracle — r4133 has no JSON export at all, so its
/// goldens are the only specification that exists, and they encode
/// tracking-**on**. Leg (2) below is that reader, on the committed golden
/// `tests/golden/json/vsource_micro.json`'s own bytes: a deck typing neither
/// `mvasc3` nor `mvasc1` emits both, ahead of the typed `BasekV`. Dropping the
/// seeds would buy a partial alignment on a surface that stays divergent anyway
/// (§1) and pay for it with a measured loss on the only surface where 0.14.5
/// *is* the authority.
///
/// Nothing here is stale: `0.88` is the live `PFNominal` on **both** engines, and
/// r4133's own `Dump` prints `~ pf=0.88` for this machine. No sequence-axis row
/// has a measured consequence in either direction — substituting the port's
/// `Line.dss`, `Vsource.dss`, `ExpControl.dss`, `PVSystem.dss` or `Master.dss`
/// into r4133's own save changes nothing (`tmp/rp311/out_bisect.txt` §A). The
/// reverse direction is [`save_omits_the_tapwinding_that_r4133_stamps`].
#[test]
fn save_membership_follows_property_tracking_not_prpsequence() {
    use crate::report::export::json::JsonOpts;

    // (1) `Save`: the port's chain heads with the seeded `PF`; r4133's line has
    // no `pf` token at all.
    let deck = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/modes/ncim/ncim_pv_pq.dss");
    assert!(deck.is_file(), "vendored corpus deck missing: {deck:?}");
    let dir = std::env::temp_dir().join(format!("dss_rp311_seq_{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    let mut dss = Dss::new();
    dss.command(&format!("compile \"{}\"", deck.display()));
    assert!(dss.errors().is_empty(), "ncim_pv_pq: {:?}", dss.errors());
    dss.command(&format!(
        "save circuit dir=\"{}\"",
        dir.to_string_lossy().replace('\\', "/")
    ));
    assert!(dss.errors().is_empty(), "save errors: {:?}", dss.errors());
    let text = std::fs::read_to_string(dir.join("Generator.dss")).expect("Generator.dss");
    std::fs::remove_dir_all(&dir).ok();
    let line = text
        .lines()
        .find(|l| l.contains("Generator.g1"))
        .unwrap_or_else(|| panic!("no Generator.g1 line in {text:?}"));
    assert!(
        line.starts_with("New \"Generator.g1\" PF=0.88 "),
        "the 0.14.5 creation seed puts the never-typed `PF` first; r4133 writes \
         `New \"Generator.g1\" bus1=genbus phases=3 …` with no `pf` token: {line:?}"
    );

    // (2) The second reader that decides it: the AltDSS JSON export walks the
    // same `next_property_set` chain. These are the committed bytes of
    // `tests/golden/json/vsource_micro.json`'s `vsource.source` / `default`
    // capture — the pinned 0.14.5 oracle's own JSON, in seed order.
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.probe basekv=115 bus1=sourcebus");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let json = dss
        .obj_to_json("vsource.source", JsonOpts::NONE)
        .expect("vsource.source");
    // Membership and order only: the float *spelling* is the lane's display
    // kernel (parity renders the oracle's `2.0000000000000000E+003`, the default
    // lane the shortest `2e3`), and it is not what this pin is about.
    let keys: Vec<&str> = json
        .match_indices("\":")
        .map(|(i, _)| {
            let head = &json[..i];
            &head[head.rfind('"').map(|p| p + 1).unwrap_or(0)..]
        })
        .collect();
    assert_eq!(
        keys,
        ["Name", "MVASC3", "MVASC1", "BasekV", "Bus1"],
        "the deck types neither `mvasc3` nor `mvasc1`; the oracle's own golden \
         emits both, ahead of the typed `BasekV` — the seeded chain. Its bytes: \
         {{\"Name\":\"source\",\"MVASC3\":2.0000000000000000E+003,\
         \"MVASC1\":2.1000000000000000E+003,\"BasekV\":1.1500000000000000E+002,\
         \"Bus1\":\"sourcebus\"}}; ours: {json}"
    );
}

/// RP3.11 §3.3 — the sequence axis in the **other** direction: r4133 stamps a
/// property this port does not, and the port's line is still round-trip-safe.
///
/// r4133's `winding=` arm writes `PropertyValue[20] := Param`
/// (`R4133:Controls/RegControl.pas:480-483`), so `tapwinding` joins the chain
/// and `SaveWrite` prints it. dss_capi 0.14.5 deliberately dropped that stamp
/// (`CAPI:Controls/RegControl.pas:417` — *"not really required"*) and this port
/// followed. Measured on both engines (2026-09-03, `epri-worker` probe
/// `tmp/rp311/i1/probe_saveorder.py` on the deck below, and the corpus deck
/// `controls/autotrans/midi_autotrans.dss` in `tmp/rp311/out/*_autotrans/save/`):
///
/// ```text
/// r4133: New "RegControl.rc1" transformer=t1 winding=2 tapwinding=2 vreg=122 band=3 ptratio=20
/// port : New "RegControl.rc1" Transformer=t1 Winding=2 VReg=122 Band=3 PTRatio=20
/// r4133: New "RegControl.rat" transformer=at winding=2 tapwinding=2 vreg=123 band=1.5 ptratio=332 EventLog=yes
/// port : New "RegControl.rat" Transformer=at Winding=2 VReg=123 Band=1.5 PTRatio=332 EventLog=Yes
/// ```
///
/// The omission is harmless because re-parsing `Winding=2` re-fires the same
/// side effect r4133 was recording — `TapWinding := ElementTerminal`
/// (`elements/control/reg_control/accessors.rs`, the `WINDING` arm) — which leg
/// (2) below asserts on the re-compiled tree. (`Transformer=` coming first is
/// RP3.11 P4, r4133's own `SaveWrite` override, pinned separately by
/// `elements::control::reg_control::tests::regcontrol_save_write_puts_the_transformer_first`.)
#[test]
fn save_omits_the_tapwinding_that_r4133_stamps() {
    let dir = std::env::temp_dir().join(format!("dss_rp311_tapwdg_{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    let mut dss = Dss::new();
    for c in [
        "clear",
        "new circuit.rp311tw basekv=12.47 pu=1.0 phases=3 bus1=src",
        "new line.l1 bus1=src bus2=b2 r1=0.1 x1=0.2 c1=0 length=1",
        "new transformer.t1 windings=2 buses=[b2 b3] conns=[wye wye] kvs=[12.47 4.16] \
         kvas=[1000 1000] xhl=6",
        "new load.ld1 bus1=b3 phases=3 kv=4.16 kw=100 pf=0.95",
        "new regcontrol.rc1 winding=2 vreg=122 band=3 ptratio=20 transformer=t1",
        "set voltagebases=[12.47, 4.16]",
        "calcv",
        "solve",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss.command(&format!(
        "save circuit dir=\"{}\"",
        dir.to_string_lossy().replace('\\', "/")
    ));
    assert!(dss.errors().is_empty(), "save errors: {:?}", dss.errors());

    // (1) The port's bytes, and the absent stamp.
    let text = std::fs::read_to_string(dir.join("RegControl.dss")).expect("RegControl.dss");
    let line = text
        .lines()
        .find(|l| l.contains("RegControl.rc1"))
        .unwrap_or_else(|| panic!("no RegControl.rc1 line in {text:?}"))
        .to_string();
    assert_eq!(
        line, "New \"RegControl.rc1\" Transformer=t1 Winding=2 VReg=122 Band=3 PTRatio=20",
        "the port's serialization; r4133 writes `New \"RegControl.rc1\" \
         transformer=t1 winding=2 tapwinding=2 vreg=122 band=3 ptratio=20` \
         (measured), the extra token being its `winding=` side-effect stamp"
    );
    assert!(
        !line.to_ascii_lowercase().contains("tapwinding"),
        "0.14.5 dropped the stamp and this port follows: {line:?}"
    );

    // (2) …and it costs nothing: the re-compiled deck restores `TapWinding`,
    // because `Winding=2` re-fires the side effect on the way in.
    let mut back = Dss::new();
    back.command(&format!(
        "compile \"{}\"",
        dir.join("Master.dss").to_string_lossy().replace('\\', "/")
    ));
    assert!(
        back.errors().is_empty(),
        "re-compile errors: {:?}",
        back.errors()
    );
    back.command("? regcontrol.rc1.tapwinding");
    assert_eq!(
        back.result(),
        "2",
        "re-parsing `Winding=2` re-fires `TapWinding := ElementTerminal`, so the \
         token r4133 prints is redundant"
    );
    back.command("? regcontrol.rc1.winding");
    assert_eq!(back.result(), "2", "the winding itself round-trips");
    std::fs::remove_dir_all(&dir).ok();
}
