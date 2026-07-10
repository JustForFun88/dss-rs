//! WP8.6 executive-tail verbs: `BatchEdit`, `MakeBusList`, `GISCoords`,
//! `SetBusXY`, `Interpolate` — the dispatch + error surfaces (all messages
//! oracle-probed 2026-07-07; the numeric behavior is gated by the
//! `tests/corpus/modes` batchedit live compares and the
//! `export_buscoords_interp` golden).

use super::common::{dss_with_circuit, query};
use crate::exec::*;

fn dss_with_loads() -> Dss {
    let mut dss = dss_with_circuit();
    for c in [
        "new load.la1 bus1=b1 kv=12.47 kw=100 pf=0.92",
        "new load.la2 bus1=b2 kv=12.47 kw=100 pf=0.95",
        "new load.lb1 bus1=b2 kv=12.47 kw=100 pf=0.90",
        "new load.xla1 bus1=b1 kv=12.47 kw=100 pf=0.88",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss
}

/// Pascal `DoBatchEditCmd`: the pattern is a regex matched case-insensitively
/// and UNANCHORED (`TRegExpr` ModifierI + `Exec` = search anywhere), so `LA`
/// hits la1/la2/xla1 but not lb1, and `^lb` (anchored) hits lb1 only. The
/// command is silent — no count message, no error.
#[test]
fn batchedit_selects_by_unanchored_case_insensitive_regex() {
    let mut dss = dss_with_loads();
    dss.command("batchedit load.LA kw=180");
    dss.command("batchedit load.^lb pf=0.85");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert!(dss.result().is_empty(), "BatchEdit must set no result");
    assert_eq!(query(&mut dss, "load.la1.kw"), "180");
    assert_eq!(query(&mut dss, "load.la2.kw"), "180");
    assert_eq!(query(&mut dss, "load.xla1.kw"), "180");
    assert_eq!(query(&mut dss, "load.lb1.kw"), "100");
    assert_eq!(query(&mut dss, "load.lb1.pf"), "0.85");
    assert_eq!(query(&mut dss, "load.la1.pf"), "0.92");
}

/// Pascal error 267: `BatchEdit Command: Object Type "%s" not found. %s` with
/// `CRLF + Parser.CmdString` (oracle: `(#267) … not found. \r\nbatchedit foo.bar kw=1 `).
#[test]
fn batchedit_unknown_class_error_267() {
    let mut dss = dss_with_circuit();
    dss.command("batchedit foo.bar kw=1");
    assert_eq!(
        dss.errors(),
        ["BatchEdit Command: Object Type \"foo\" not found. \nbatchedit foo.bar kw=1 "]
    );
}

/// The named-parameter form `batchedit object=load.LA kw=…` behaves exactly
/// like the positional form (`GetObjClassAndName` accepts an `object=`
/// prefix); a zero-match pattern is silent — no error, no load edited.
#[test]
fn batchedit_object_named_form_and_zero_match() {
    let mut dss = dss_with_loads();
    dss.command("batchedit object=load.LA kw=150");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(query(&mut dss, "load.la1.kw"), "150");
    assert_eq!(query(&mut dss, "load.la2.kw"), "150");
    assert_eq!(query(&mut dss, "load.xla1.kw"), "150");
    assert_eq!(query(&mut dss, "load.lb1.kw"), "100");

    dss.command("batchedit load.zzz kw=1");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    for name in ["la1", "la2", "xla1", "lb1"] {
        assert_ne!(
            query(&mut dss, &format!("load.{name}.kw")),
            "1",
            "load.{name} must not be edited by a zero-match pattern"
        );
    }
}

/// `batchedit circuit.…` is a documented silent no-op (Pascal "Do nothing").
#[test]
fn batchedit_circuit_class_is_noop() {
    let mut dss = dss_with_circuit();
    dss.command("batchedit circuit.x basekv=999");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
}

/// `MakeBusList` = `if BusNameRedefined then ReprocessBusDefs` — nothing else;
/// `SetBusXY` finds the bus in the (built) bus list and sets X/Y+CoordDefined,
/// error 28722 when it is not there. Before `MakeBusList` the bus list is
/// empty, so the same `SetBusXY` errors (oracle-probed: `(#28722) Error: Bus
/// "src" not found.` on an unbuilt list).
#[test]
fn setbusxy_needs_bus_list_then_sets_coords() {
    let mut dss = dss_with_circuit();
    dss.command("new line.l1 bus1=src bus2=b1");
    dss.command("setbusxy bus=b1 x=3 y=4");
    // The Pascal loop runs `BusList.Find` + the 28722 error once per
    // PARAMETER, so the three-parameter command logs it three times.
    assert_eq!(dss.errors(), ["Error: Bus \"b1\" not found."; 3]);
    dss.errors.clear();

    dss.command("makebuslist");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss.command("setbusxy bus=b1 x=3 y=4");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let ckt = dss.circuit.as_ref().expect("circuit");
    assert!(!ckt.bus_name_redefined, "MakeBusList reprocessed");
    let ib = ckt.bus_list.find("b1").expect("b1 in the bus list");
    let bus = &ckt.buses[ib];
    assert_eq!((bus.x, bus.y, bus.coord_defined), (3.0, 4.0, true));
}

/// Pascal error 28721: an unknown named parameter logs `Error: Unknown
/// Parameter on command line: %s` and the loop continues.
#[test]
fn setbusxy_unknown_parameter_error() {
    let mut dss = dss_with_circuit();
    dss.command("makebuslist");
    dss.command("setbusxy bus=sourcebus x=1 y=2 q=3");
    assert_eq!(
        dss.errors(),
        ["Error: Unknown Parameter on command line: 3"]
    );
    let ckt = dss.circuit.as_ref().expect("circuit");
    let ib = ckt.bus_list.find("sourcebus").expect("source bus");
    assert!(ckt.buses[ib].coord_defined);
}

/// `GISCoords` is the documented DSS C-API no-op ("Do nothing here on DSS
/// C-API") — no error, no result (oracle-probed).
#[test]
fn giscoords_is_silent_noop() {
    let mut dss = dss_with_circuit();
    dss.command("giscoords 1 2");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert!(dss.result().is_empty());
}

/// The NAMED-meter happy path: `interpolate em` on the interp fixture deck
/// (the `export_buscoords_interp` golden's deck, replayed inline) fills the
/// same coordinates as that golden's bare `interpolate` — b2/b3/b4/c1 pinned
/// from `tests/golden/reports/export_buscoords_interp.txt` (pure f64 anchor
/// arithmetic, exact equality).
#[test]
fn interpolate_named_meter_fills_zone_coordinates() {
    let mut dss = Dss::new();
    for c in [
        "Set DefaultBaseFrequency=60",
        "new circuit.itp basekv=12.47 pu=1.0 phases=3 bus1=src",
        "~ r1=0.4 x1=1.6 r0=1.2 x0=4.2",
        "new linecode.lc nphases=3 r1=0.301 x1=0.667 r0=0.882 x0=2.041 c1=3.4 c0=1.6",
        "~ units=km",
        "new line.lfeed bus1=src bus2=b1 linecode=lc length=0.4 units=km",
        "new line.l1 bus1=b1 bus2=b2 linecode=lc length=0.5 units=km",
        "new line.l2 bus1=b2 bus2=b3 linecode=lc length=0.5 units=km",
        "new line.l3 bus1=b3 bus2=b4 linecode=lc length=0.5 units=km",
        "new line.l4 bus1=b4 bus2=b5 linecode=lc length=0.5 units=km",
        "new line.lc1 bus1=b3 bus2=c1 linecode=lc length=0.3 units=km",
        "new line.lc2 bus1=c1 bus2=c2 linecode=lc length=0.3 units=km",
        "new load.ld5 bus1=b5 phases=3 conn=wye model=1 kv=12.47 kw=400 pf=0.92",
        "new load.ldc bus1=c2 phases=3 conn=wye model=1 kv=12.47 kw=200 pf=0.95",
        "new energymeter.em element=line.lfeed terminal=1",
        "set voltagebases=[12.47]",
        "calcvoltagebases",
        "setbusxy bus=src x=0 y=0",
        "setbusxy bus=b1 x=100 y=0",
        "setbusxy bus=b5 x=500 y=0",
        "setbusxy bus=c2 x=300 y=220",
        "Set maxiterations=100",
        "solve",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss.command("interpolate em");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let ckt = dss.circuit.as_ref().expect("circuit");
    for (name, x, y) in [
        ("b2", 150.0, 55.0),
        ("b3", 200.0, 110.0),
        ("b4", 350.0, 55.0),
        ("c1", 250.0, 165.0),
    ] {
        let ib = ckt
            .bus_list
            .find(name)
            .unwrap_or_else(|| panic!("bus {name} in the bus list"));
        let bus = &ckt.buses[ib];
        assert!(bus.coord_defined, "{name} must be coord-filled");
        assert_eq!((bus.x, bus.y), (x, y), "bus {name}");
    }
}

/// `Interpolate <name>` on a missing meter is Pascal error 277 (the name is
/// echoed uppercased); an existing meter whose zone was never built fails the
/// `CheckBranchList` guard (error 529).
#[test]
fn interpolate_named_meter_errors() {
    let mut dss = dss_with_circuit();
    dss.command("interpolate m7");
    assert_eq!(dss.errors(), ["EnergyMeter \"M7\" not found."]);
    dss.errors.clear();

    dss.command("new line.l1 bus1=sourcebus bus2=b1");
    dss.command("new energymeter.em element=line.l1 terminal=1");
    dss.command("interpolate em");
    assert_eq!(
        dss.errors(),
        ["Meter Zone Lists need to be built. Do Solve or Makebuslist first!"]
    );
}

// ---------------------------------------------------------------------------
// WP8.8 command-tail ports: Enable/Disable, SetkVBase, Losses, Summary, the
// step-solution commands, Reconductor. Every pinned value/message below was
// captured live from the pinned oracle (dss-python 0.15.7 / dss_capi 0.14.5)
// on 2026-07-10 (IEEE13 fixture where one is compiled).
// ---------------------------------------------------------------------------

/// Compile the vendored IEEE13 master (the live-gate corpus copy).
fn dss_with_ieee13() -> Dss {
    let master: std::path::PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "corpus",
        "electricdss-tst",
        "Version8",
        "Distrib",
        "IEEETestCases",
        "13Bus",
        "IEEE13Nodeckt.dss",
    ]
    .iter()
    .collect();
    assert!(master.is_file(), "IEEE13 master missing");
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!(
        "compile \"{}\"",
        master.to_string_lossy().replace('\\', "/")
    ));
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss
}

/// Pascal `DoDisableCmd`/`DoEnableCmd`: a named element goes through the edit
/// path (`Enabled=false`), `*` sets the whole class directly; both raise
/// `BusNameRedefined`. An unknown class and a non-circuit-element class are
/// SILENT no-ops (oracle-probed: no error, no result).
#[test]
fn enable_disable_named_star_and_silent_arms() {
    let mut dss = dss_with_loads();
    dss.command("disable load.la1");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(query(&mut dss, "load.la1.enabled"), "No");
    dss.command("enable load.la1");
    assert_eq!(query(&mut dss, "load.la1.enabled"), "Yes");

    dss.command("disable load.*");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    for name in ["la1", "la2", "lb1", "xla1"] {
        assert_eq!(
            query(&mut dss, &format!("load.{name}.enabled")),
            "No",
            "{name}"
        );
    }
    assert!(
        dss.circuit.as_ref().expect("circuit").bus_name_redefined,
        "Set_Enabled must raise BusNameRedefined"
    );
    dss.command("enable load.*");
    assert_eq!(query(&mut dss, "load.lb1.enabled"), "Yes");

    // Silent arms (oracle-probed 2026-07-10): unknown class, DSS_OBJECT class.
    dss.command("disable bogus.*");
    dss.command("disable loadshape.default");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
}

/// Pascal `DoSetkVBase`: `kVLL` (or positional) divides by √3, `kVLN` stores
/// as-is; a hit raises `VoltageBaseChanged`, a miss appends
/// `Bus <name> not found.` to GlobalResult (no error). Oracle-probed on
/// IEEE13 bus 675: kVLL=4.16 → 2.4017771198288433, kVLN=2.4 → 2.4.
#[test]
fn set_kv_base_kvll_kvln_positional_and_missing() {
    let mut dss = dss_with_ieee13();
    let kv_base = |dss: &Dss| {
        let ckt = dss.circuit.as_ref().expect("circuit");
        let ib = ckt.bus_list.find("675").expect("bus 675");
        ckt.buses[ib].kv_base
    };
    dss.command("SetkVBase bus=675 kVLL=4.16");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(kv_base(&dss), 2.4017771198288433);
    assert!(dss.circuit.as_ref().unwrap().solution.voltage_base_changed);

    dss.command("SetkVBase bus=675 kVLN=2.4");
    assert_eq!(kv_base(&dss), 2.4);

    dss.command("SetkVBase 675 4.16");
    assert_eq!(kv_base(&dss), 2.4017771198288433, "positional = kVLL");

    dss.command("SetkVBase bus=nosuchbus kVLL=1");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(dss.result(), "Bus nosuchbus not found.");
}

/// Pascal `DolossesCmd`: the ACTIVE circuit element's losses, kW/kvar,
/// `Format('%10.5g, %10.5g')`. Oracle-probed on solved IEEE13:
/// `select Line.650632` → `    60.729,     195.99`; `select Transformer.Sub`
/// → `  0.032284,    0.26244`. (The corpus use, `UPFC_test_3.dss`, is exactly
/// `select …` + `losses`; Pascal's ActiveCktElement-on-New side effect is not
/// reproduced — no ported consumer needs it, same inert class as
/// `DoOpenCmd`'s `SetActiveBus`.)
#[test]
fn losses_cmd_formats_active_element_losses() {
    let mut dss = dss_with_ieee13();
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss.command("select Line.650632");
    dss.command("Losses");
    assert_eq!(dss.result(), "    60.729,     195.99");
    dss.command("select Transformer.Sub");
    dss.command("Losses");
    assert_eq!(dss.result(), "  0.032284,    0.26244");
}

/// Pascal `DoSummaryCmd`: the full summary text into GlobalResult — pinned
/// byte-for-byte (LF line ends per the port convention) against the oracle's
/// capture on solved IEEE13, including the `Control Mode =Static` missing
/// space, the trailing spaces after the Year/Hour/voltage values, and the
/// ` \n - Circuit Summary -\n \n` separator block.
#[test]
fn summary_cmd_matches_oracle_text() {
    let mut dss = dss_with_ieee13();
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss.command("Summary");
    let expected = "Status = SOLVED\n\
        Solution Mode = Snap\n\
        Number = 100\n\
        Load Mult = 1.000\n\
        Devices = 38\n\
        Buses = 16\n\
        Nodes = 41\n\
        Control Mode =Static\n\
        Total Iterations = 2\n\
        Control Iterations = 1\n\
        Max Sol Iter = 2\n \n \
        - Circuit Summary -\n \n\
        Year = 0 \n\
        Hour = 0 \n\
        Max pu. voltage = 1.056 \n\
        Min pu. voltage = 0.96084 \n\
        Total Active Power:   3.56705 MW\n\
        Total Reactive Power: 1.73644 Mvar\n\
        Total Active Losses:   0.112392 MW, (3.151 %)\n\
        Total Reactive Losses: 0.327861 Mvar\n\
        Frequency = 60 Hz\n\
        Mode = Snap\n\
        Control Mode = Static\n\
        Load Model = PowerFlow\n";
    assert_eq!(dss.result(), expected);
}

/// The step-solution commands (`_InitSnap`/`_SolveNoControl`/`_SolveDirect`/
/// `_SolvePFlow`/`_SampleControls`/`_DoControlActions`): iteration counts
/// oracle-probed on IEEE13 — `solve` 2, `_SolveDirect` 1 (converged),
/// `_InitSnap` + `_SolveNoControl` 4, `_SolvePFlow` 2.
#[test]
fn step_solution_commands_match_oracle_iterations() {
    let mut dss = dss_with_ieee13();
    dss.command("solve");
    let iters = |dss: &Dss| dss.circuit.as_ref().unwrap().solution.iteration;
    assert_eq!(iters(&dss), 2);
    dss.command("_SolveDirect");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(iters(&dss), 1);
    assert!(dss.circuit.as_ref().unwrap().solution.converged_flag);
    dss.command("_InitSnap");
    dss.command("_SolveNoControl");
    assert_eq!(iters(&dss), 4);
    dss.command("_SampleControls");
    dss.command("_DoControlActions");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss.command("_SolvePFlow");
    assert_eq!(iters(&dss), 2);
}

/// Pascal `DoReconductorCmd` + `TraceAndEdit`: on metered IEEE13,
/// `Line1=632670 Line2=692675 Linecode=mtx601` re-linecodes the whole
/// traceback path 692675 → 671692 (the switch, previously bare) → 670671 →
/// 632670, leaving branches off the path (632633) untouched. Oracle-probed
/// 2026-07-10, including all five error surfaces (#28702-28707).
#[test]
fn reconductor_traces_path_and_errors() {
    let mut dss = dss_with_ieee13();
    dss.command("new energymeter.em1 element=Transformer.Sub terminal=1");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(query(&mut dss, "Line.692675.linecode"), "mtx606");
    assert_eq!(query(&mut dss, "Line.671692.linecode"), "");
    dss.command("Reconductor Line1=Line.632670 Line2=Line.692675 Linecode=mtx601");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    for ln in ["650632", "632670", "670671", "692675", "671692"] {
        assert_eq!(
            query(&mut dss, &format!("Line.{ln}.linecode")),
            "mtx601",
            "{ln}"
        );
    }
    assert_eq!(query(&mut dss, "Line.632633.linecode"), "mtx602");

    // Error surfaces.
    dss.command("Reconductor Linecode=mtx601");
    assert_eq!(dss.errors(), ["Both Line1 and Line2 must be specified!"]);
    dss.errors.clear();
    dss.command("Reconductor Line1=632670 Line2=692675");
    assert_eq!(
        dss.errors(),
        ["Either a new LineCode or a Geometry must be specified!"]
    );
    dss.errors.clear();
    dss.command("Reconductor Line1=zzz Line2=692675 linecode=mtx601");
    assert_eq!(dss.errors(), ["Line.zzz not found."]);
    dss.errors.clear();
    // Sibling branches: no traceback path in either direction.
    dss.command("Reconductor Line1=632633 Line2=692675 linecode=mtx601");
    assert_eq!(
        dss.errors(),
        ["Traceback path not found between Line1 and Line2."]
    );
    dss.errors.clear();

    // No meter zone: fresh unmetered compile.
    let mut dss = dss_with_ieee13();
    dss.command("solve");
    dss.command("Reconductor Line1=632670 Line2=692675 linecode=mtx601");
    assert_eq!(
        dss.errors(),
        [
            "Error: Both Lines must be in the same EnergyMeter zone. One or both are not in any meter zone."
        ]
    );
}

/// Pascal `DoVarCmd` (`var`, dispatched pre-circuit, `ExecCommands.pas:334`):
/// define (`var @x=…`), echo (`var @x`), list (bare `var` — the `Variable,
/// Value` header + the 7 pre-seeded intrinsics + user vars, `<name>. <value>`),
/// and the #28725 illegal-name error. All oracle-probed 2026-07-10.
#[test]
fn var_cmd_define_echo_list_and_illegal() {
    let mut dss = Dss::new();
    dss.command("var @myvar=3.14 @s=hello");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(dss.result(), "");
    dss.command("var @myvar");
    assert_eq!(dss.result(), "3.14");
    dss.command("var");
    let expected = "Variable, Value\n\
        @lastfile. null\n\
        @lastexportfile. null\n\
        @lastshowfile. null\n\
        @lastplotfile. null\n\
        @lastredirectfile. null\n\
        @lastcompilefile. null\n\
        @result. null\n\
        @myvar. 3.14\n\
        @s. hello\n";
    assert_eq!(dss.result(), expected);
    dss.command("var bogus=1");
    assert_eq!(
        dss.errors(),
        ["Illegal Variable Name: bogus; Must begin with \"@\""]
    );
}

/// The pre-circuit utility commands (`ExecCommands.pas:301-331`), all
/// oracle-probed 2026-07-10: `fileedit` on a missing file sets GlobalResult
/// (an existing file fires the GUI editor — headless no-op); `classes` lists
/// every intrinsic class; `userclasses` is the fixed banner; `cd` to a missing
/// directory is error #282; `doscmd` is the fixed disabled error #283.
#[test]
fn pre_circuit_utility_commands_match_oracle() {
    let mut dss = Dss::new();
    dss.command("fileedit nosuchfile.dss");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(dss.result(), "File \"nosuchfile.dss\" does not exist.");

    dss.command("classes");
    let r = dss.result().to_string();
    assert!(
        r.starts_with(
            "LineCode, LoadShape, TShape, PriceShape, XYcurve, GrowthShape, TCC_Curve, Spectrum, WireData"
        ),
        "{r}"
    );
    assert!(
        r.contains("Vsource, Isource, VCCS, Load, Transformer"),
        "{r}"
    );

    dss.command("userclasses");
    assert_eq!(dss.result(), "No User Classes Defined.");

    dss.command("cd \"Q:/nope\"");
    assert_eq!(dss.errors(), ["Directory \"Q:/nope\" not found."]);
    dss.errors.clear();

    dss.command("doscmd echo hi");
    assert_eq!(
        dss.errors(),
        [
            "DOScmd is disabled. Enable it via API or set the environment variable DSS_CAPI_ALLOW_DOSCMD=1 before starting the process."
        ]
    );
}

/// `Set/Get ShowExport` (`ExecOptions.pas:606/973`): the `AutoShowExport`
/// flag round-trips as `Yes`/`No` (oracle-probed 2026-07-10); its only
/// upstream consumer is the GUI editor auto-open — a headless no-op.
#[test]
fn show_export_option_round_trips() {
    let mut dss = dss_with_circuit();
    dss.command("get showexport");
    assert_eq!(dss.result(), "No");
    dss.command("set showexport=yes");
    dss.command("get showexport");
    assert_eq!(dss.result(), "Yes");
    dss.command("set showexport=no");
    dss.command("get showexport");
    assert_eq!(dss.result(), "No");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
}
