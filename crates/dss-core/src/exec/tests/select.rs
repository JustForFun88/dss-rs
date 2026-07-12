//! Exec `Select` command tests (Pascal `DoSelectCmd`, ExecHelper.pas:670) and the
//! `Show Yprim` active-element surface (WP8.4 step 11). `Select class.name [term]`
//! makes a circuit element the `ActiveCktElement` (read by `Show Yprim`) and sets
//! its active terminal; a `DSS_OBJECT` / `circuit` / bare select does not; an
//! unknown class logs #903 and **falls back** to the previously-referenced class.
//! These cover the error/edge branches the `show_yprim` golden's happy path does
//! not (step-11 audit follow-up).

use crate::exec::*;

fn build(dss: &mut Dss) {
    dss.command("clear");
    dss.command("new circuit.sel basekv=12.47 phases=3 bus1=src");
    dss.command("new line.l1 bus1=src bus2=b phases=3 length=1 units=km r1=0.1 x1=0.3");
    dss.command("new line.l2 bus1=b bus2=c phases=3 length=1 units=km r1=0.1 x1=0.3");
}

/// The active terminal (0-based) of the currently-selected circuit element.
fn active_terminal(dss: &Dss) -> usize {
    let (ci, idx) = dss.active_ckt_element.expect("an element is selected");
    dss.classes[ci].objects[idx]
        .as_ckt_element()
        .expect("selected object is a circuit element")
        .cd()
        .active_terminal
}

#[test]
fn select_circuit_element_sets_active() {
    let mut dss = Dss::new();
    build(&mut dss);
    dss.command("select line.l1");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert!(dss.active_ckt_element.is_some());
}

/// The active-terminal parse (Pascal `if Length(Param)>0 then ActiveTerminalIdx :=
/// IntValue else 1`): a present, in-range terminal is adopted; an out-of-range one
/// leaves it unchanged (`Set_ActiveTerminal` rejects out-of-`1..Nterms`); an absent
/// one selects terminal 1.
#[test]
fn select_terminal_parse() {
    let mut dss = Dss::new();
    build(&mut dss);
    dss.command("select line.l1 2");
    assert_eq!(active_terminal(&dss), 1, "terminal 2 → 0-based 1");
    // Out-of-range terminal 99: unchanged (stays 1), NOT clamped to 1.
    dss.command("select line.l1 99");
    assert_eq!(active_terminal(&dss), 1, "OOR terminal leaves it unchanged");
    // Absent terminal → terminal 1 (0-based 0).
    dss.command("select line.l1");
    assert_eq!(active_terminal(&dss), 0, "absent terminal → 1");
}

/// An unknown *object name* in a valid class → Pascal #245; the active element is
/// not switched.
#[test]
fn select_unknown_name_errors_245() {
    let mut dss = Dss::new();
    build(&mut dss);
    dss.command("select line.doesnotexist");
    assert!(
        dss.errors().iter().any(|e| e.contains("not found")),
        "expected #245, got {:?}",
        dss.errors()
    );
    assert!(dss.active_ckt_element.is_none());
}

/// An unknown *class* logs Pascal #903 and **falls back** to the previously-
/// referenced class (`SetObjectClass` failure keeps `LastClassReferenced`): here
/// `select badclass.l2` after a Line select still finds `l2` in the Line class.
/// (Oracle-probed: `select badclass.l2` after `select line.l1` surfaces #903 and
/// selects `l2`.)
#[test]
fn select_unknown_class_903_and_fallback() {
    let mut dss = Dss::new();
    build(&mut dss);
    dss.command("select line.l1");
    dss.command("select badclass.l2");
    assert!(
        dss.errors()
            .iter()
            .any(|e| e.contains("Object Class") && e.contains("badclass")),
        "expected #903 'Object Class ... not found', got {:?}",
        dss.errors()
    );
    // Fell back to the Line class and found l2.
    let (ci, idx) = dss.active_ckt_element.expect("l2 selected via fallback");
    assert!(
        dss.classes[ci].objects[idx]
            .data()
            .name()
            .eq_ignore_ascii_case("l2"),
        "fallback should select l2 in the Line class"
    );
}

/// A general `DSS_OBJECT` (a loadshape) must NOT become the active circuit element
/// and must NOT error (Pascal: a `DSS_OBJECT` does nothing in the `SetActive` arm).
#[test]
fn select_dss_object_does_not_become_active() {
    let mut dss = Dss::new();
    build(&mut dss);
    dss.command("select line.l1");
    let before = dss.active_ckt_element;
    dss.command("select loadshape.default");
    assert!(
        dss.errors().is_empty(),
        "DSS_OBJECT select must not error: {:?}",
        dss.errors()
    );
    assert_eq!(
        dss.active_ckt_element, before,
        "a loadshape must not become the active ckt element"
    );
}

/// `Select circuit.<name>` is a no-op (single-circuit `SetActiveCircuit`); the
/// active element is unchanged and no error is raised. (The **dotted** form is the
/// real circuit branch: a bare `select circuit` parses `circuit` as the object
/// *name* — "no dot → last class assumed" — and errors #245, oracle-probed, same as
/// the port; only `class = circuit` hits `SetActiveCircuit`.)
#[test]
fn select_circuit_is_noop() {
    let mut dss = Dss::new();
    build(&mut dss);
    dss.command("select line.l1");
    let before = dss.active_ckt_element;
    dss.command("select circuit.sel");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(dss.active_ckt_element, before);
}

/// Regression (audit-code Major, `command.rs` `do_clear_cmd`): `Clear` must reset
/// `active_ckt_element`. Otherwise a stale `(cls, idx)` from a pre-`Clear` `Select`
/// indexes the now-emptied class objects when a fresh circuit is built and
/// `Show Yprim` runs → an OOB **panic** (or a foreign-element read).
#[test]
fn clear_resets_active_ckt_element_no_panic() {
    let mut dss = Dss::new();
    build(&mut dss);
    dss.command("select line.l1");
    assert!(dss.active_ckt_element.is_some());

    dss.command("clear");
    assert!(
        dss.active_ckt_element.is_none(),
        "Clear must reset ActiveCktElement"
    );

    // A fresh, smaller circuit (the Line class is now empty); `show yprim` with no
    // active element must be a clean no-op — before the fix, a stale `(line, 0)`
    // indexed the emptied objects here and panicked.
    dss.command("new circuit.fresh basekv=12.47 bus1=s");
    dss.command("show yprim"); // must not panic
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    // `@lastshowfile` defaults to the ParserVars sentinel `"null"`; the no-op arm
    // must not set it to a `*_Yprim.txt` path.
    assert!(
        !dss.last_show_file().to_lowercase().ends_with("_yprim.txt"),
        "no active element → no Yprim file, got {:?}",
        dss.last_show_file()
    );
}

/// `Show Yprim` with no prior `Select` is a silent no-op — no file, no error
/// (Pascal arm 25 nil-derefs `ActiveCktElement`; the port guards it).
#[test]
fn show_yprim_without_select_is_noop() {
    let mut dss = Dss::new();
    build(&mut dss);
    dss.command("show yprim");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert!(
        !dss.last_show_file().to_lowercase().ends_with("_yprim.txt"),
        "no prior Select → no Yprim file, got {:?}",
        dss.last_show_file()
    );
}

/// `Show Yprim`'s filename is `<ParentClass.Name>_<Name>_Yprim.txt` with **no**
/// `CircuitName_` prefix (Pascal `ShowOptions.pas:395`), unlike every other Show
/// report. Pins that convention (a regression routing it through the
/// `<CircuitName_>` path would produce `sel_Line_l1_Yprim.txt`).
#[test]
fn show_yprim_filename_has_no_circuit_prefix() {
    let mut dss = Dss::new();
    build(&mut dss);
    let scratch = std::env::temp_dir().join(format!("dss_select_yprim_{}", std::process::id()));
    std::fs::create_dir_all(&scratch).expect("mkdir scratch");
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    dss.command("solve");
    dss.command("select line.l1");
    dss.command("show yprim");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let base = std::path::Path::new(dss.last_show_file())
        .file_name()
        .expect("a produced filename")
        .to_string_lossy()
        .to_lowercase();
    assert_eq!(base, "line_l1_yprim.txt", "no CircuitName_ prefix");
    std::fs::remove_dir_all(&scratch).ok();
}

#[test]
fn set_class_activates_and_set_object_selects() {
    // WP-U1.1 item 5 (r3875 / C11): `Set Class=`/`Set Object=` were NOT_PORTED;
    // now `Set Class=X` activates class X (Pascal SetObjectClass — the r3875 fix
    // sets ActiveDSSClass, unified here as `active_class`) and `Set Object=`
    // selects an object, resolving a bare name against the active class.
    let mut dss = Dss::new();
    build(&mut dss);
    dss.command("new load.ld bus1=b kv=12.47 kw=5");

    // Set Class activates the Line class.
    dss.command("Set Class=Line");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let line_ci = dss.class_by_name["line"];
    assert_eq!(
        dss.active_class,
        Some(line_ci),
        "Set Class=Line activates Line"
    );

    // A bare Set Object resolves against the active class (Line) — the r3875
    // point: because Set Class activated Line, `Set Object=l2` (no class prefix)
    // finds line.l2 and makes it the ActiveCktElement (matches the capi015/0.14.5
    // oracle, which resolves `Set Class=Line; Set Object=l2`).
    dss.command("Set Object=l2");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let l2 = (line_ci, dss.classes[line_ci].name_to_idx["l2"]);
    assert_eq!(
        dss.active_ckt_element,
        Some(l2),
        "bare Set Object=l2 selects line.l2"
    );

    // `Set Type=`/`Set Element=` are aliases (Pascal 1,12 / 2,13).
    dss.command("Set Type=Load");
    let load_ci = dss.class_by_name["load"];
    assert_eq!(
        dss.active_class,
        Some(load_ci),
        "Set Type= aliases Set Class="
    );
    dss.command("Set Element=load.ld");
    let ld = (load_ci, dss.classes[load_ci].name_to_idx["ld"]);
    assert_eq!(
        dss.active_ckt_element,
        Some(ld),
        "Set Element= aliases Set Object="
    );
}

#[test]
fn set_class_unknown_errors_keeps_previous() {
    let mut dss = Dss::new();
    build(&mut dss);
    dss.command("Set Class=Line");
    let line_ci = dss.class_by_name["line"];
    dss.command("Set Class=NoSuchClass");
    assert!(
        dss.errors().iter().any(|e| e.contains("not found")),
        "expected class-not-found, got {:?}",
        dss.errors()
    );
    // Unknown class leaves the previously-referenced class in place.
    assert_eq!(dss.active_class, Some(line_ci));
}
