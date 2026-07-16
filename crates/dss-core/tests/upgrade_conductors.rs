//! WP-U1.4 (wt-u14cond) — the dss_capi 0.15.x `Conductors` object-reference-array
//! (Line prop 34, LineGeometry prop 20), the 3-class `(WireData|CNData|TSData)`
//! proxy with `fullNames=True`.
//!
//! Text `Conductors=` is **upstream-broken** on both classes: the proxy's
//! `TProxyClass.GetDSSClass` compares the parser's `AnsiLowerCase`d class token
//! against the *original-case* `TargetClassNames`, so every real (class-prefixed)
//! item errors #10103 "Invalid class", a bare item errors #10103 "You must define
//! the Conductor class", and the array is never populated. Only an all-`none`
//! list succeeds (Line) or errors "At least one valid conductor" (LineGeometry).
//! Every string below is pinned against capi015 (dss-python 0.16.0b2 / dss_capi
//! 0.15.0b4) — see `parse_conductor_proxy` (`obj/props/class_props/parse.rs`) and
//! DIVERGENCES.md §"Line/LineGeometry Conductors (text upstream-broken)".

use dss_core::exec::Dss;

/// Circuit with a wire, a CN cable, a TS cable and a 4-conductor/3-phase spacing.
fn setup() -> Dss {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.condprobe basekv=12.47 phases=3 bus1=sourcebus");
    dss.command("Set earthmodel=deri");
    dss.command(
        "New WireData.w1 Runits=m radunits=m gmrunits=m Rdc=1.0e-4 Rac=1.05e-4 \
         GMRac=0.004 radius=0.005 capradius=0.005",
    );
    dss.command(
        "New CNData.cn1 Runits=m radunits=m gmrunits=m Rdc=1.0e-4 Rac=1.05e-4 GMRac=0.004 \
         radius=0.005 capradius=0.005 EpsR=2.3 InsLayer=0.004 DiaIns=0.022 DiaCable=0.030 \
         k=16 DiaStrand=0.001 GmrStrand=0.0004 Rstrand=2.0e-3",
    );
    dss.command(
        "New TSData.ts1 Runits=m radunits=m gmrunits=m Rdc=1.0e-4 Rac=1.05e-4 GMRac=0.004 \
         radius=0.005 capradius=0.005 EpsR=2.3 InsLayer=0.004 DiaIns=0.022 DiaCable=0.030 \
         DiaShield=0.025 TapeLayer=0.0002 TapeLap=20.0",
    );
    dss.command(
        "New LineSpacing.sp nconds=4 nphases=3 units=m x=[0 0.1 0.2 0.3] h=[-1.2 -1.2 -1.2 -1.2]",
    );
    dss
}

fn last_errors(dss: &Dss) -> String {
    dss.errors().join(" || ")
}

/// A class-prefixed item (any case) hits the proxy `GetDSSClass` case bug →
/// #10103 "Invalid class (<lowercased>)". capi015-pinned, both classes.
#[test]
fn conductors_full_name_items_error_invalid_class() {
    let mut dss = setup();
    dss.command(
        "New Line.l1 bus1=sourcebus bus2=b2 phases=3 spacing=sp \
         Conductors=[WireData.w1, CNData.cn1, TSData.ts1, WireData.w1] length=1 units=km",
    );
    let errs = last_errors(&dss);
    assert!(
        errs.contains(
            "Line.l1.Conductors: Invalid class (wiredata) for item. \
             Valid classes: (WireData|CNData|TSData)"
        ),
        "Line: expected Invalid-class error, got: {errs}"
    );

    // LineGeometry — same proxy, same bug.
    let mut dss = setup();
    dss.command(
        "New LineGeometry.g nconds=4 nphases=3 reduce=y \
         Conductors=[WireData.w1, CNData.cn1, WireData.w1, WireData.w1]",
    );
    let errs = last_errors(&dss);
    assert!(
        errs.contains(
            "LineGeometry.g.Conductors: Invalid class (wiredata) for item. \
             Valid classes: (WireData|CNData|TSData)"
        ),
        "LineGeometry: expected Invalid-class error, got: {errs}"
    );
}

/// A bare (un-prefixed) item → #10103 "You must define the Conductor class …"
/// ("valid items", the `AllowNoneItem` wording).
#[test]
fn conductors_bare_name_items_error_missing_class() {
    let mut dss = setup();
    dss.command(
        "New Line.l1 bus1=sourcebus bus2=b2 phases=3 spacing=sp \
         Conductors=[w1, cn1, ts1, w1] length=1 units=km",
    );
    let errs = last_errors(&dss);
    assert!(
        errs.contains(
            "Line.l1.Conductors: You must define the Conductor class for all the \
             valid items in the array."
        ),
        "expected missing-class error, got: {errs}"
    );
}

/// `Conductors=` before a spacing (conductor-array count `< 1`) → #402 "No
/// objects are expected!" — checked before any item validation.
#[test]
fn conductors_without_spacing_error_no_objects() {
    let mut dss = setup();
    dss.command(
        "New Line.l1 bus1=sourcebus bus2=b2 phases=3 \
         Conductors=[WireData.w1, CNData.cn1, TSData.ts1, WireData.w1] length=1 units=km",
    );
    let errs = last_errors(&dss);
    assert!(
        errs.contains(
            "Line.l1.Conductors: No objects are expected! Check if the order of \
             property assignments is correct."
        ),
        "expected no-objects error, got: {errs}"
    );
}

/// An all-`none` list is the ONE text form that parses on a Line (all NIL slots,
/// no error) — the model switches to the spacing path. capi015-pinned (err#0).
#[test]
fn conductors_all_none_parses_on_line() {
    let mut dss = setup();
    dss.command(
        "New Line.l1 bus1=sourcebus bus2=b2 phases=3 spacing=sp \
         Conductors=[none, none, none, none] length=1 units=km",
    );
    assert!(
        dss.errors().is_empty(),
        "all-none Conductors must parse without error, got: {}",
        last_errors(&dss)
    );
}

/// LineGeometry rejects an all-`none` list — its side effect requires at least
/// one non-NIL conductor (#10103). capi015-pinned.
#[test]
fn conductors_all_none_rejected_on_line_geometry() {
    let mut dss = setup();
    dss.command(
        "New LineGeometry.g nconds=4 nphases=3 reduce=y Conductors=[none, none, none, none]",
    );
    let errs = last_errors(&dss);
    assert!(
        errs.contains("LineGeometry.g.Conductors: At least one valid conductor must be provided."),
        "expected at-least-one-conductor error, got: {errs}"
    );
}
