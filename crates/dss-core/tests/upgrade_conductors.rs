//! WP-U1.4 (wt-u14cond) — the `Conductors` object-reference-array (Line prop 34,
//! LineGeometry prop 20), the 3-class `(WireData|CNData|TSData)` list.
//!
//! Text `Conductors=` was reproduced as **upstream-broken** from dss_capi 0.15.x
//! (whose `TProxyClass.GetDSSClass` compared the lowercased class token against
//! the original-case `TargetClassNames`, so every class-prefixed item errored
//! #10103 "Invalid class"). The 0.15.x-adoption sweep proved this is a
//! capi015-ONLY breakage: EPRI r4133 has no `TProxyClass` — it parses
//! `conductors=` NATIVELY with a **case-insensitive** `LowerCase(CondClass)`
//! dispatch AND solves (own epri-worker probe: `Conductors=[WireData.w wiredata.w]`
//! → converged, Line.l1 I1=(21.801759, 0.027069)). The port adopts r4133: a
//! class-prefixed item now resolves + solves; a bare item still #10103s; a
//! `Conductors=` before the spacing keeps the port's clean #402 (r4133 hits a
//! #303 AV — UB, not reproduced); an all-`none` geometry keeps the port's clean
//! reject (r4133 #303 AV — UB, own probe). See `parse_conductor_proxy`
//! (`obj/props/class_props/parse.rs`) and DIVERGENCES.md §"Line/LineGeometry
//! Conductors".

use dss_core::exec::Dss;
use num_complex::Complex64;

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
    dss.error_texts().join(" || ")
}

/// A class-prefixed item resolves by a case-insensitive class match (r4133
/// `LowerCase(CondClass)` dispatch) — no "Invalid class" error — and the line
/// SOLVES. Own r4133 probe (epri-worker): `Conductors=[WireData.w wiredata.w]`
/// on a 2-wire/1-phase spacing converges, Line.l1 currents
/// [21.801759, 0.027069, -21.801696, 0.038617]; the port matches to a
/// faer-vs-KLU floor. (Was pinned as the capi015 #10103 "Invalid class" breakage
/// — re-decided to r4133 by the 0.15.x-adoption sweep.)
#[test]
fn conductors_full_name_items_resolve_and_solve() {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.c basekv=12.47 phases=1 bus1=b1");
    dss.command(
        "new wiredata.w gmrac=0.0244 rac=0.306 runits=mi radunits=in gmrunits=ft diam=0.721 \
         normamps=530",
    );
    dss.command("new linespacing.sp nconds=2 nphases=1 x=(0 3) h=(29 29) units=ft");
    // Mixed-case class prefixes — the r4133 dispatch is case-insensitive.
    dss.command(
        "new line.l1 bus1=b1 bus2=b2 phases=1 length=1 units=mi spacing=sp \
         Conductors=[WireData.w wiredata.w]",
    );
    dss.command("new load.ld bus1=b2 phases=1 kv=7.2 kw=100 pf=1");
    dss.command("set voltagebases=[12.47]");
    dss.command("calcv");
    dss.command("solve");
    assert!(
        dss.errors().is_empty(),
        "class-prefixed Conductors must resolve + solve (r4133), got: {}",
        last_errors(&dss)
    );
    assert!(
        dss.circuit().is_some_and(|c| c.is_solved),
        "the conductors-built line must converge"
    );
    let snap = dss.snapshot_elements();
    let line = snap
        .iter()
        .find(|e| e.name.eq_ignore_ascii_case("Line.l1"))
        .expect("Line.l1 in snapshot");
    let r4133 = [
        Complex64::new(21.801_758_533_521_934, 0.027_068_608_997_069_532),
        Complex64::new(-21.801_696_277_965_675, 0.038_616_893_076_323_32),
    ];
    for (k, &want) in r4133.iter().enumerate() {
        let got = line.currents[k];
        assert!(
            (got.re - want.re).abs() < 1e-6 && (got.im - want.im).abs() < 1e-6,
            "Line.l1 current[{k}] {got} != r4133 {want}"
        );
    }
}

/// A bare (un-prefixed) item is rejected — r4133 keeps the bare-name error too
/// (`dotpos = 0` arm: LineGeometry #10103, Line #181023 — own r4133 probe). The
/// port emits its single generic-list #10103 "You must define the Conductor
/// class …" for both classes; the per-class code/wording differs but the
/// behavior (reject a class-less item) matches r4133 and is what this pins.
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

/// `Conductors=` before a spacing (conductor-array count `< 1`) → the port's
/// clean #402 "No objects are expected!", checked before any item validation.
/// r4133 instead hits a #303 Access Violation on this order (UB — own probe of
/// the sibling all-none-geometry case; the conductors-before-spacing path is the
/// same NIL-array deref), NOT reproduced per the project UB rule.
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

/// An all-`none` list parses on a Line (all NIL slots, no error). r4133 also
/// accepts the parse (own probe) — the resulting all-NIL line is degenerate and
/// does not converge, but the PARSE itself does not error on either engine.
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

/// LineGeometry rejects an all-`none` list with the port's clean #10103 "At
/// least one valid conductor". r4133 does NOT reject it cleanly — it hits a #303
/// Access Violation (UB — own epri-worker probe of exactly this deck), so the
/// port's upfront reject is the not-reproduced-UB choice (project rule), not a
/// capi015 pin.
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
