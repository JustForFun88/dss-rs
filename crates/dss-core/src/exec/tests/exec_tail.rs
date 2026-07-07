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
