use super::common::*;
use crate::exec::*;

#[test]
fn line_fetches_sym_linecode() {
    // Oracle (dss-python 0.15.7): linecode in mi, line length 2000 ft.
    let mut dss = Dss::new();
    dss.command("New circuit.p");
    dss.command(
        "New linecode.mtx601 nphases=3 r1=0.1 x1=0.2 r0=0.3 x0=0.6 c1=3 c0=1 \
             units=mi normamps=500 emergamps=700",
    );
    dss.command("New line.l1 bus1=a bus2=b linecode=mtx601 length=2000 units=ft");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(query(&mut dss, "line.l1.linecode"), "mtx601");
    assert_eq!(query(&mut dss, "line.l1.normamps"), "500");
    assert_eq!(query(&mut dss, "line.l1.emergamps"), "700");
    assert_eq!(query(&mut dss, "line.l1.units"), "ft");
    // r1 getter divides by FUnitsConvert = ConvertLineUnits(mi, ft) = 5280.
    assert!((query_f64(&mut dss, "line.l1.r1") - 0.1 / 5280.0).abs() < 1e-12);
    // Unported scalar/array refs render like the oracle.
    assert_eq!(query(&mut dss, "line.l1.geometry"), "");
    assert_eq!(query(&mut dss, "line.l1.wires"), "[]");
}

#[test]
fn load_and_vsource_resolve_shape_refs() {
    // WP5.3: the shape refs became resolved `object_ref_class` props. The
    // ObjectRef getter renders the resolved object's name, and an unset
    // `yearly` is seeded from `daily` (Pascal `YearlyShapeObj := DailyShapeObj`).
    let mut dss = Dss::new();
    dss.command("New circuit.t basekv=12.47 bus1=src");
    dss.command("New loadshape.d1 npts=2 interval=1 mult=(0.4 0.8)");
    dss.command("New growthshape.g1 npts=2 year=(1 2) mult=(1.02 1.05)");
    dss.command("New load.la bus1=src phases=3 kv=12.47 kw=100 pf=1 daily=d1 growth=g1");
    dss.command("New vsource.v2 bus1=src basekv=12.47 daily=d1");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(query(&mut dss, "load.la.daily"), "d1");
    assert_eq!(query(&mut dss, "load.la.yearly"), "d1"); // seeded from daily
    assert_eq!(query(&mut dss, "load.la.growth"), "g1");
    assert_eq!(query(&mut dss, "vsource.v2.daily"), "d1");
    assert_eq!(query(&mut dss, "vsource.v2.yearly"), "d1");

    // A missing shape is the Pascal 401 ("object not found") and leaves the
    // reference empty — the edit continues.
    dss.command("New load.lb bus1=src daily=nope");
    assert!(
        dss.errors().iter().any(|e| e.contains("not found")),
        "expected a not-found error, got {:?}",
        dss.errors()
    );
}

#[test]
fn line_fetches_matrix_linecode() {
    let mut dss = Dss::new();
    dss.command("New circuit.p");
    dss.command(
        "New linecode.mx nphases=2 rmatrix=[0.1 | 0.05 0.1] \
             xmatrix=[0.2 | 0.07 0.2] cmatrix=[3 | -1 3] units=mi",
    );
    dss.command("New line.l3 bus1=a.1.2 bus2=b.1.2 linecode=mx length=1 units=mi");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(query(&mut dss, "line.l3.phases"), "2");
    assert_eq!(query(&mut dss, "line.l3.rmatrix"), "[0.1 |0.05 0.1 ]");
    // Matrix model hides the sym scalars (CONDITIONAL_VALUE).
    assert_eq!(query(&mut dss, "line.l3.r1"), "----");
}

#[test]
fn line_linecode_then_r1_override_keeps_fetched_matrix() {
    // Oracle: r1=0.5 overrides the scalar, but the dumped rmatrix still
    // reflects the code's Z (recalc is deferred to CalcYPrim).
    let mut dss = Dss::new();
    dss.command("New circuit.p");
    dss.command("New linecode.mtx601 nphases=3 r1=0.1 x1=0.2 r0=0.3 x0=0.6 units=mi");
    dss.command("New line.l4 bus1=a bus2=b linecode=mtx601 r1=0.5 length=1 units=mi");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(query(&mut dss, "line.l4.r1"), "0.5");
    // Zs.re = (2*0.1 + 0.3)/3 = 0.5/3, units_convert reset to 1 by r1.
    let rm = query(&mut dss, "line.l4.rmatrix");
    let first: f64 = rm
        .trim_start_matches('[')
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert!((first - 0.5 / 3.0).abs() < 1e-9, "{rm}");
}

#[test]
fn line_unknown_linecode_errors_and_continues() {
    let mut dss = Dss::new();
    dss.command("New circuit.p");
    dss.command("New line.l5 bus1=a bus2=b linecode=nosuch r1=0.1 length=1");
    assert!(
        dss.errors()
            .iter()
            .any(|e| e == "Line.l5.LineCode: LineCode object \"nosuch\" not found."),
        "{:?}",
        dss.errors()
    );
    // The edit continued: r1=0.1 was applied, phases stayed default.
    assert_eq!(query(&mut dss, "line.l5.r1"), "0.1");
    assert_eq!(query(&mut dss, "line.l5.phases"), "3");
}

#[test]
fn line_geometry_undefined_wire_in_array_aborts() {
    // Pascal `DSSObjectReferenceArrayProperty` Exits on the first unresolved
    // token: the "not found" is logged and the write function (SetWires)
    // never runs, so nothing is stored and no spurious "Unexpected number"
    // count error fires.
    let mut dss = Dss::new();
    dss.command("New circuit.p");
    dss.command("New WireData.acsr Rdc=0.0526 GMRac=0.0244 GMRunits=ft radius=0.0306 radunits=ft normamps=530 Runits=ft");
    dss.command("New LineGeometry.g1 nconds=3 nphases=3 wires=[acsr bad acsr]");
    let errs = dss.errors();
    assert!(
        errs.iter().any(|e| e.contains("object \"bad\" not found")),
        "{errs:?}"
    );
    assert!(
        !errs.iter().any(|e| e.contains("Unexpected number")),
        "{errs:?}"
    );
    // Exit before the write function: no conductors were stored.
    assert_eq!(query(&mut dss, "LineGeometry.g1.wires"), "[, , ]");
}

/// WP7.1 step 3a: a `geometry=`-specified Line resolves the `LineGeometry`
/// class end to end (parse → foreign-class resolve → `FetchGeometryCode`),
/// adopts the geometry's conductor count, and solves through the Carson
/// matrix path — the full pipeline the inline `geometry_tests` bypass.
#[test]
fn line_geometry_specified_resolves_and_solves() {
    let mut dss = Dss::new();
    dss.command("New circuit.geo basekv=12.47 phases=3");
    dss.command(
        "New WireData.w runits=m gmrunits=m radunits=m \
             rac=0.0003 gmrac=0.005 radius=0.01 normamps=400",
    );
    dss.command(
        "New LineGeometry.geo1 nconds=3 nphases=3 \
             cond=1 wire=w x=0 h=10 units=m cond=2 wire=w x=1 h=10 cond=3 wire=w x=2 h=10",
    );
    dss.command("New Line.l1 bus1=sourcebus bus2=b2 phases=3 geometry=geo1 length=1 units=km");
    dss.command("New Load.ld bus1=b2 phases=3 kv=12.47 kw=300 pf=0.95 model=1");
    dss.command("Set voltagebases=[12.47]");
    dss.command("CalcVoltageBases");
    dss.command("Set controlmode=off");
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    // The Line resolved the geometry and took its conductor count + type.
    assert_eq!(query(&mut dss, "Line.l1.geometry"), "geo1");
    assert_eq!(query(&mut dss, "Line.l1.phases"), "3");
    // The sym scalars are hidden (`----`) — a matrix/geometry model is active.
    assert_eq!(query(&mut dss, "Line.l1.r1"), "----");

    let ckt = dss.circuit().unwrap();
    assert!(ckt.solution.converged_flag, "geometry line should converge");
}

/// WP7.1 step 3a follow-up: `rmatrix`/`xmatrix`/`cmatrix` on a geometry line must
/// report the **per-unit-length** matrix — Pascal `GetZmatScale`/`GetYCScale`
/// divide the stored *total* `Z`/`Yc` by `Len` (the geometry folds length+units
/// in). Regression guard for both getter geometry branches (the pre-fix getter
/// divided by `units_convert` = 1.0 and echoed the total, off by a factor of
/// `Len`).
#[test]
fn line_geometry_rmatrix_is_per_unit_length() {
    let mut dss = Dss::new();
    dss.command("New circuit.geo basekv=12.47 phases=3");
    dss.command(
        "New WireData.w runits=m gmrunits=m radunits=m \
             rac=0.0003 gmrac=0.005 radius=0.01 normamps=400",
    );
    dss.command(
        "New LineGeometry.geo1 nconds=3 nphases=3 \
             cond=1 wire=w x=0 h=10 units=m cond=2 wire=w x=1 h=10 cond=3 wire=w x=2 h=10",
    );
    // length = 2 km, so the total Z (= per-metre × 1000 × 2) and Len (= 2) diverge:
    // the getter must divide by Len, yielding the per-km matrix.
    dss.command("New Line.l1 bus1=sourcebus bus2=b2 phases=3 geometry=geo1 length=2 units=km");
    dss.command("New Load.ld bus1=b2 phases=3 kv=12.47 kw=300 pf=0.95 model=1");
    dss.command("Set voltagebases=[12.47]");
    dss.command("CalcVoltageBases");
    dss.command("Set controlmode=off");
    // Solve so `CalcYPrim` runs `FMakeZFromGeometry`, populating the line's total Z.
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    // First (= [0][0]) entry of each matrix query. The oracle per-metre diagonal is
    // (3.525947626277e-4 + j 9.150978496084e-4) ohm/m, so the per-km getter value
    // is that × 1000 — NOT × 2000 (the total), which the pre-fix getter returned.
    let first = |s: &str| -> f64 {
        s.trim()
            .trim_start_matches('[')
            .split_whitespace()
            .next()
            .unwrap()
            .parse()
            .unwrap()
    };
    let rm = query(&mut dss, "Line.l1.rmatrix");
    let xm = query(&mut dss, "Line.l1.xmatrix");
    assert!(
        (first(&rm) - 3.525947626277e-04 * 1000.0).abs() < 1e-7,
        "rmatrix[0][0] should be per-unit-length (per km): {rm}"
    );
    assert!(
        (first(&xm) - 9.150978496084e-04 * 1000.0).abs() < 1e-7,
        "xmatrix[0][0] should be per-unit-length (per km): {xm}"
    );
    // cmatrix is reported in nF per unit length (Pascal `GetYCScale`): the oracle
    // per-metre C diagonal is 8.941431489720e-3 nF/m (the `C3_NF` reference), so
    // the per-km getter value is that × 1000 — NOT × 2000 (the stored total). The
    // base-frequency omega cancels between the stored susceptance and the getter
    // scale, so this anchor is exact regardless of the two-pi truncation.
    let cm = query(&mut dss, "Line.l1.cmatrix");
    assert!(
        (first(&cm) - 8.941431489720e-03 * 1000.0).abs() < 1e-6,
        "cmatrix[0][0] should be per-unit-length (per km): {cm}"
    );
}

/// WP7.1 step 3a follow-up: a geometry whose conductors share a position makes
/// `CalcYPrim` (via `FMakeZFromGeometry`) hit the Pascal `ELineGeometryProblem`.
/// The Y-build must surface the queued message and set `solution_abort` — the
/// faithful equivalent of Pascal's `SolutionAbort` + `Exit`.
#[test]
fn line_geometry_conductors_in_same_space_aborts_solve() {
    let mut dss = Dss::new();
    dss.command("New circuit.geo basekv=12.47 phases=3");
    dss.command(
        "New WireData.w runits=m gmrunits=m radunits=m \
             rac=0.0003 gmrac=0.005 radius=0.01 normamps=400",
    );
    // cond 1 and 2 occupy the same (x, h) — conductors-in-same-space.
    dss.command(
        "New LineGeometry.bad nconds=3 nphases=3 \
             cond=1 wire=w x=0 h=10 units=m cond=2 wire=w x=0 h=10 cond=3 wire=w x=2 h=10",
    );
    dss.command("New Line.l1 bus1=sourcebus bus2=b2 phases=3 geometry=bad length=1 units=km");
    dss.command("New Load.ld bus1=b2 phases=3 kv=12.47 kw=300 pf=0.95 model=1");
    dss.command("Set voltagebases=[12.47]");
    dss.command("CalcVoltageBases");
    dss.command("Solve");

    assert!(
        dss.circuit().unwrap().solution.solution_abort,
        "a geometry Zmatrix error must abort the solution"
    );
    assert!(
        dss.errors()
            .iter()
            .any(|e| e.contains("occupy the same space")),
        "the specific geometry failure (Pascal LineConstants.pas:287, \
         `Conductors %d and %d occupy the same space.`) must be surfaced: {:?}",
        dss.errors()
    );
}

/// Audit follow-up: a singular series impedance hits Pascal error 183
/// (`TLineObj.CalcYPrim`, Line.pas:1300). `DoErrorMsg` sets `SolutionAbort`
/// unconditionally, so the solve aborts under the oracle default
/// (`EARLY_ABORT = True`) — it does NOT silently embed `epsilon·I` and continue.
/// Confirmed live: dss-python raises `(#183) ... Aborting solution.`.
#[test]
fn line_singular_matrix_aborts_solve() {
    let mut dss = Dss::new();
    dss.command("New circuit.sing basekv=1 phases=1 bus1=src");
    // Zero series impedance ⇒ singular Z ⇒ matrix-inversion error in CalcYPrim.
    dss.command("New Line.l1 bus1=src bus2=b phases=1 rmatrix=[0] xmatrix=[0] cmatrix=[0]");
    dss.command("Solve");

    assert!(
        dss.circuit().unwrap().solution.solution_abort,
        "a singular line impedance must abort the solution"
    );
    assert!(
        dss.errors()
            .iter()
            .any(|e| e.contains("Matrix Inversion Error for Line")),
        "the matrix-inversion error must be surfaced: {:?}",
        dss.errors()
    );
}

/// Audit follow-up: changing the phase count on a matrix/geometry model is
/// illegal (Pascal Line.pas:639-644). The count is reverted and 18101 is logged
/// (a `DoSimpleMsg`, so the solution is NOT aborted). Confirmed live:
/// `(#18101) Illegal change of number of phases for "Line.l1"`.
#[test]
fn line_illegal_phase_change_reverts_and_logs() {
    let mut dss = Dss::new();
    dss.command("New circuit.p basekv=1 phases=1 bus1=src");
    // rmatrix ⇒ SymComponentsModel=false (a matrix model).
    dss.command("New Line.l1 bus1=src bus2=b phases=1 rmatrix=[0.1] xmatrix=[0.1] cmatrix=[3]");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    dss.command("Edit Line.l1 phases=3");
    assert!(
        dss.errors()
            .iter()
            .any(|e| e == "Illegal change of number of phases for \"Line.l1\""),
        "expected the 18101 message, got {:?}",
        dss.errors()
    );
    // The illegal change was rejected: the phase count stays at 1.
    assert_eq!(query(&mut dss, "Line.l1.phases"), "1");
}
