use super::common::*;
use crate::exec::*;
use num_complex::Complex64;

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
fn conductor_none_geometry_rejects_but_line_accepts_and_solves() {
    // 0.15.x-adoption sweep (re-decided from the WP-U1.1 AllowNoneItem adoption):
    // a `none` in a conductor list is a Line-level construct only. r4133
    // LineGeometry.pas:346-396 has NO `none` branch on the `wires`/`cncables`/
    // `tscables` arms → #10103; BOTH gating oracles reject a GEOMETRY-level `none`
    // (0.14.5 #40303, r4133 #10103). The Line-level lists (r4133 Line.pas
    // FetchWireList → NIL slot, compacted out of the Carson calc by
    // LoadSpacingAndWires) DO accept it and the circuit solves.

    // (1) GEOMETRY-level `none` rejects (was wrongly accepted via ALLOW_NONE_ITEM).
    let mut dss = Dss::new();
    dss.command("New circuit.p");
    dss.command("New wiredata.w Runits=mi Rac=0.1 GMRunits=mi GMRac=0.01 radunits=in diam=0.5");
    dss.command("New linegeometry.g nconds=2 nphases=2 reduce=n");
    dss.command("~ wires=(w none)");
    // Assert the SPECIFIC not-found message (not a bare error-exists smoke check):
    // the geometry `wires` arm no longer allows `none` (ALLOW_NONE_ITEM dropped), so
    // the token resolves as a WireData reference and misses — the port's
    // `WireData object "none" not found.` mirrors 0.14.5 #40303 (r4133 #10103
    // "not defined"). A discriminating pin: a future parser regression that errored
    // for an unrelated reason would not carry this message.
    assert!(
        dss.errors()
            .iter()
            .any(|e| e.contains("object \"none\" not found")),
        "a geometry-level `none` must reject with the WireData not-found message \
         (r4133 #10103), got {:?}",
        dss.errors()
    );

    // (2) LINE-level `none` is accepted AND the circuit solves — r4133 compacts
    // the NIL slot out (2-wire spacing, 1 valid conductor ⇒ a 1-conductor line).
    // Own r4133 probe (epri-worker, this exact deck): converged, Line.l1 currents
    // [21.802597, -0.001427, -21.802520, 0.057037] — the port matches to a
    // faer-vs-KLU floor.
    let mut dl = Dss::new();
    dl.command("clear");
    dl.command("new circuit.g basekv=12.47 phases=1 bus1=b1");
    dl.command(
        "new wiredata.w gmrac=0.0244 rac=0.306 runits=mi radunits=in gmrunits=ft diam=0.721 \
         normamps=530",
    );
    dl.command("new linespacing.sp nconds=2 nphases=1 x=(0 3) h=(29 29) units=ft");
    dl.command("new line.l1 bus1=b1 bus2=b2 phases=1 length=1 units=mi spacing=sp wires=(w none)");
    dl.command("new load.ld bus1=b2 phases=1 kv=7.2 kw=100 pf=1");
    dl.command("set voltagebases=[12.47]");
    dl.command("calcv");
    dl.command("solve");
    assert!(
        dl.errors().is_empty(),
        "line-level `none` must solve: {:?}",
        dl.errors()
    );
    assert!(
        dl.circuit().is_some_and(|c| c.is_solved),
        "line with a compacted `none` conductor must converge"
    );
    let snap = dl.snapshot_elements();
    let line = snap
        .iter()
        .find(|e| e.name.eq_ignore_ascii_case("Line.l1"))
        .expect("Line.l1 in snapshot");
    let r4133 = [
        Complex64::new(21.802_596_540_773_72, -0.001_426_872_519_914_468_3),
        Complex64::new(-21.802_520_352_484_407, 0.057_037_298_443_901_82),
    ];
    for (k, &want) in r4133.iter().enumerate() {
        let got = line.currents[k];
        assert!(
            (got.re - want.re).abs() < 1e-6 && (got.im - want.im).abs() < 1e-6,
            "Line.l1 current[{k}] {got} != r4133 {want}"
        );
    }

    // (3) Control (feature-sensitivity): a genuine missing wire name still errors —
    // only the reserved `none` is special (and only Line-level).
    let mut dss2 = Dss::new();
    dss2.command("New circuit.p");
    dss2.command("New wiredata.w Runits=mi Rac=0.1 GMRunits=mi GMRac=0.01 radunits=in diam=0.5");
    dss2.command("New linegeometry.g2 nconds=2 nphases=2 reduce=n");
    dss2.command("~ wires=(w nope)");
    assert!(
        dss2.errors()
            .iter()
            .any(|e| e.contains("object \"nope\" not found")),
        "a non-`none` missing wire must still error with the not-found message, got {:?}",
        dss2.errors()
    );
}

/// 0.15.x-adoption sweep item 4, coverage follow-up (settle round): the existing
/// `none`-compaction tests only exercise a **≤2-conductor** list (a 2-slot spacing,
/// 1 valid). This pins the audit-flagged **>2-conductor** compaction (a 4-conductor
/// spacing with `wires=(w w w none)`): the `none` at the neutral position must be
/// compacted out (r4133 `LoadSpacingAndWires`, actualNConds 4→3, spacing coords by
/// ORIGINAL position), leaving a 3-phase line whose Carson `Z` is bit-identical to
/// the explicit 3-conductor spacing built from the same phase-position coordinates.
/// The explicit path is itself oracle-validated (`line_spacing_specified_resolves_
/// and_solves`), so a compaction that mis-indexed the coordinates or mis-sized the
/// larger geometry would diverge here.
#[test]
fn line_none_conductor_compaction_over_two_conductors_matches_explicit() {
    let node_v = |dss: &Dss| dss.circuit().unwrap().solution.node_v.clone();
    let build = |spacing: &str, wires: &str| {
        let mut dss = Dss::new();
        dss.command("new circuit.g basekv=12.47 phases=3 bus1=src");
        dss.command(
            "new wiredata.w gmrac=0.0244 rac=0.306 runits=mi radunits=in gmrunits=ft \
             diam=0.721 normamps=530",
        );
        dss.command(spacing);
        dss.command(&format!(
            "new line.l1 bus1=src bus2=b2 phases=3 length=1 units=mi spacing=sp wires={wires}"
        ));
        dss.command("new load.ld bus1=b2 phases=3 kv=12.47 kw=900 pf=0.95 model=1");
        dss.command("set voltagebases=[12.47]");
        dss.command("calcv");
        dss.command("solve");
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        assert!(dss.circuit().is_some_and(|c| c.is_solved), "must converge");
        dss
    };

    // Compaction path: 4-conductor spacing (3 phase + 1 neutral), the neutral noned
    // out — exercises the >2-conductor skip-NIL copy + actualNConds recount.
    let compact = build(
        "new linespacing.sp nconds=4 nphases=3 x=(0 3 6 1.5) h=(29 29 29 35) units=ft",
        "(w w w none)",
    );
    // Explicit equivalent: a 3-conductor spacing at the SAME phase-position coords.
    let explicit = build(
        "new linespacing.sp nconds=3 nphases=3 x=(0 3 6) h=(29 29 29) units=ft",
        "(w w w)",
    );

    let (vc, ve) = (node_v(&compact), node_v(&explicit));
    assert_eq!(vc.len(), ve.len(), "same node count");
    let drift = vc
        .iter()
        .zip(&ve)
        .map(|(a, b)| (a - b).norm())
        .fold(0.0_f64, f64::max);
    assert!(
        drift < 1e-6,
        "a >2-conductor `none` compaction must yield the same solve as the explicit \
         3-conductor spacing; node V diverged by {drift:.3e} V"
    );
}

#[test]
fn incomplete_double_sym_matrix_rejected_keeps_default() {
    // WP-U1.1 item 2 settle: the DoubleSymMatrix parse arm (Reactor RMatrix/XMatrix,
    // Capacitor CMatrix, Fault GMatrix) shares the r4133 incomplete-matrix reject
    // with the SymMatrix arm, but only the SymMatrix arm was tested (line_code).
    // Probed 2026-07-12: capi015 zero-fills a reactor `rmatrix=(1 | 2 3)` to
    // `(1 |2 3 |0 0 0 )`; the port adopts the r4133 reject.
    //
    // Asserted on the reject error AND on the STORED value (`get_f64_array`),
    // which is what the arm's `return Ok(0)` skips — the stored array is the
    // direct evidence of reject-revert vs zero-fill, whatever the text getter
    // would render.
    let mut dss = Dss::new();
    dss.command("New circuit.p");
    dss.command("New reactor.rr bus1=a phases=3 rmatrix=(1 | 2 3) xmatrix=(1 | 2 3 | 4 5 6)");
    assert!(
        dss.errors()
            .iter()
            .any(|e| e.contains("does not match with the expected order")),
        "expected the r4133 reject for the incomplete rmatrix, got {:?}",
        dss.errors()
    );
    // Stored value reverted: the arm skipped `set_f64_array`, so rmatrix stays unset
    // (None) — NOT the zero-filled partial `[1, 0,0, 2,3,0, ...]` capi015 would store.
    let ci = dss.class_by_name["reactor"];
    let oi = dss.classes[ci].name_to_idx["rr"];
    let ridx = dss.classes[ci].props.property_index("RMatrix").unwrap();
    assert!(
        dss.classes[ci].arena[oi].get_f64_array(ridx).is_none(),
        "incomplete rmatrix must be rejected (unset), not zero-filled"
    );

    // Over-rejection + accept-path guard for the DoubleSymMatrix arm: a fully-complete
    // reactor matrix errors nothing and IS stored (get_f64_array Some with the values).
    let mut dss2 = Dss::new();
    dss2.command("New circuit.p");
    dss2.command(
        "New reactor.ok bus1=a phases=3 rmatrix=(1 | 2 3 | 4 5 6) xmatrix=(0.1 | 0.2 0.3 | 0.4 0.5 0.6)",
    );
    assert!(
        dss2.errors().is_empty(),
        "complete matrices must not error: {:?}",
        dss2.errors()
    );
    let ci2 = dss2.class_by_name["reactor"];
    let oi2 = dss2.classes[ci2].name_to_idx["ok"];
    let ridx2 = dss2.classes[ci2].props.property_index("RMatrix").unwrap();
    let stored = dss2.classes[ci2].arena[oi2]
        .get_f64_array(ridx2)
        .expect("a complete DoubleSymMatrix must be accepted and stored");
    assert!(
        stored.contains(&4.0),
        "the complete rmatrix values must be stored, got {stored:?}"
    );
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

/// DE_PASCALIZE R3.1 (iv): the statically-classed object-ref fields keep a typed
/// `Idx<T>`, not a class-erased `ElemId`. Two things need pinning:
///
/// 1. the narrowing on the write side is **total** — every one of these
///    properties declares its target class (`object_ref_class`), so a resolved
///    reference is never a foreign class and `Idx<T>` is `Some` exactly where
///    `ElemId` used to be (the field would silently go `None` otherwise);
/// 2. the handle still names the right arena slot — `ClassArena::get::<T>` on
///    the target class's own arena yields the object the deck named.
#[test]
fn typed_object_ref_handles_dereference_to_the_named_object() {
    use crate::elements::general::dynamic_exp::DynamicExpObj;
    use crate::elements::general::growth_shape::GrowthShapeObj;
    use crate::elements::general::line_code::LineCodeObj;
    use crate::elements::general::load_shape::LoadShapeObj;
    use crate::elements::general::temp_shape::TShapeObj;
    use crate::elements::general::xfmr_code::XfmrCodeObj;
    use crate::elements::general::xy_curve::XyCurveObj;
    use crate::elements::pc::generator::Generator;
    use crate::elements::pc::isource::Isource;
    use crate::elements::pc::load::Load;
    use crate::elements::pc::pvsystem::PVSystem;
    use crate::elements::pc::windgen::WindGen;
    use crate::elements::pd::line::Line;
    use crate::elements::pd::transformer::Transformer;

    let mut dss = Dss::new();
    dss.command("New circuit.t basekv=12.47 bus1=src");
    dss.command("New loadshape.d1 npts=2 interval=1 mult=(0.4 0.8)");
    dss.command("New growthshape.g1 npts=2 year=(1 2) mult=(1.02 1.05)");
    dss.command("New tshape.t1 npts=2 interval=1 temp=(25 30)");
    dss.command("New xycurve.eff npts=2 xarray=[0.1 1.0] yarray=[0.9 0.97]");
    dss.command("New linecode.mx nphases=3 r1=0.1 x1=0.2");
    dss.command("New xfmrcode.xc phases=3 windings=2 xhl=6");
    dss.command("New load.la bus1=src phases=3 kv=12.47 kw=100 pf=1 daily=d1 growth=g1");
    dss.command("New line.l1 bus1=src bus2=b2 linecode=mx length=1");
    dss.command(
        "New transformer.t1 phases=3 windings=2 buses=[src, b3] \
         kvs=[12.47, 4.16] kvas=[1000, 1000] xfmrcode=xc",
    );
    dss.command(
        "New pvsystem.pv bus1=b2 phases=3 kv=12.47 kva=500 pmpp=500 \
         irrad=0.8 temperature=25 pf=1 effcurve=eff tdaily=t1",
    );
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    // Resolve `<class>.<name>` to `(class arena slot, object index)`.
    let slot = |dss: &Dss, class: &str, name: &str| -> (usize, usize) {
        let ci = dss.class_by_name[class];
        (ci, dss.classes[ci].name_to_idx[name])
    };
    let (load_ci, load_oi) = slot(&dss, "load", "la");
    let (line_ci, line_oi) = slot(&dss, "line", "l1");
    let (xf_ci, xf_oi) = slot(&dss, "transformer", "t1");
    let (pv_ci, pv_oi) = slot(&dss, "pvsystem", "pv");

    // Every typed handle, paired with the arena it must be read through.
    let load = dss.classes[load_ci].arena.get::<Load>(load_oi).unwrap();
    let daily = load
        .daily_shape_ref
        .expect("daily= resolved → Idx<LoadShape>");
    let growth = load
        .growth_shape_ref
        .expect("growth= resolved → Idx<GrowthShape>");
    let code = dss.classes[line_ci]
        .arena
        .get::<Line>(line_oi)
        .unwrap()
        .line_code_ref
        .expect("linecode= resolved → Idx<LineCode>");
    let xfmr_code = dss.classes[xf_ci]
        .arena
        .get::<Transformer>(xf_oi)
        .unwrap()
        .xfmr_code_ref()
        .expect("xfmrcode= resolved → Idx<XfmrCode>");
    let pv = dss.classes[pv_ci].arena.get::<PVSystem>(pv_oi).unwrap();
    let tdaily = pv
        .daily_t_shape_ref
        .expect("tdaily= resolved → Idx<TShape>");
    let eff = pv
        .base
        .inverter_curve_ref
        .expect("effcurve= resolved → Idx<XYcurve>");

    let named = |dss: &Dss, class: &str, idx: usize| -> String {
        dss.classes[dss.class_by_name[class]].arena[idx]
            .data()
            .name()
            .to_string()
    };
    assert_eq!(
        dss.classes[dss.class_by_name["loadshape"]]
            .arena
            .get::<LoadShapeObj>(daily.get())
            .map(|s| s.data().name()),
        Some("d1"),
        "the typed LoadShape handle must read out of the LoadShape arena"
    );
    assert_eq!(named(&dss, "growthshape", growth.get()), "g1");
    assert_eq!(named(&dss, "linecode", code.get()), "mx");
    assert_eq!(named(&dss, "xfmrcode", xfmr_code.get()), "xc");
    assert_eq!(named(&dss, "tshape", tdaily.get()), "t1");
    assert_eq!(named(&dss, "xycurve", eff.get()), "eff");

    // The concrete-typed reads agree with the name reads (and prove the target
    // arenas really hold those classes).
    assert!(
        dss.classes[dss.class_by_name["growthshape"]]
            .arena
            .get::<GrowthShapeObj>(growth.get())
            .is_some()
            && dss.classes[dss.class_by_name["linecode"]]
                .arena
                .get::<LineCodeObj>(code.get())
                .is_some()
            && dss.classes[dss.class_by_name["xfmrcode"]]
                .arena
                .get::<XfmrCodeObj>(xfmr_code.get())
                .is_some()
            && dss.classes[dss.class_by_name["tshape"]]
                .arena
                .get::<TShapeObj>(tdaily.get())
                .is_some()
            && dss.classes[dss.class_by_name["xycurve"]]
                .arena
                .get::<XyCurveObj>(eff.get())
                .is_some(),
        "every typed handle must dereference in its own class arena"
    );

    // The rest of the 34: the yearly/duty siblings, the two non-`daily`
    // LoadShape refs, the second XYcurve ref on the inverter class, the WindGen
    // curve pair, and the shared `DynEqPCE` DynamicExp handle. Settler pass
    // 2026-07-26 (audit-tests finding 3 — the first cut asserted 6 of 34, so a
    // narrowing that silently yielded `None` on any of the rest would have
    // shown up only as a corpus/dump difference).
    dss.command("New loadshape.d2 npts=2 interval=1 mult=(0.5 0.9)");
    dss.command("New xycurve.pt npts=2 xarray=[0 75] yarray=[1.0 0.9]");
    dss.command("New xycurve.vv npts=2 xarray=[0.9 1.1] yarray=[1.0 -1.0]");
    dss.command("New xycurve.pl npts=2 xarray=[0 1] yarray=[0.0 0.1]");
    dss.command(
        "New dynamicexp.de nvariables=2 varnames=[Speed Mass] expression=[Speed dt = -1 Mass /]",
    );
    dss.command(
        "New load.lc bus1=src phases=3 kv=12.47 kw=100 pf=1          daily=d1 yearly=d2 duty=d2 cvrcurve=d2",
    );
    dss.command(
        "New generator.g1 bus1=b2 phases=3 kv=12.47 kw=100 pf=1          daily=d1 yearly=d2 duty=d2 DynamicEq=de",
    );
    dss.command(
        "New windgen.w1 bus1=b2 phases=3 kv=12.47 kw=100 pf=1          daily=d1 yearly=d2 duty=d2 VV_Curve=vv PLoss=pl DynamicEq=de",
    );
    dss.command("New isource.i1 bus1=b2 amps=1 daily=d1 yearly=d2 duty=d2");
    dss.command("Edit pvsystem.pv P-TCurve=pt daily=d1 yearly=d2 duty=d2");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    let (lc_ci, lc_oi) = slot(&dss, "load", "lc");
    let (g_ci, g_oi) = slot(&dss, "generator", "g1");
    let (w_ci, w_oi) = slot(&dss, "windgen", "w1");
    let (i_ci, i_oi) = slot(&dss, "isource", "i1");
    let lc = dss.classes[lc_ci].arena.get::<Load>(lc_oi).unwrap();
    let g1 = dss.classes[g_ci].arena.get::<Generator>(g_oi).unwrap();
    let w1 = dss.classes[w_ci].arena.get::<WindGen>(w_oi).unwrap();
    let i1 = dss.classes[i_ci].arena.get::<Isource>(i_oi).unwrap();
    let pv = dss.classes[pv_ci].arena.get::<PVSystem>(pv_oi).unwrap();

    // (a) every LoadShape handle, read through the LoadShape arena;
    for (h, want, what) in [
        (lc.daily_shape_ref, "d1", "load.lc daily"),
        (lc.yearly_shape_ref, "d2", "load.lc yearly"),
        (lc.duty_shape_ref, "d2", "load.lc duty"),
        (lc.cvr_shape_ref, "d2", "load.lc cvrcurve"),
        (g1.daily_shape_ref, "d1", "generator.g1 daily"),
        (g1.yearly_shape_ref, "d2", "generator.g1 yearly"),
        (g1.duty_shape_ref, "d2", "generator.g1 duty"),
        (w1.daily_shape_ref, "d1", "windgen.w1 daily"),
        (w1.yearly_shape_ref, "d2", "windgen.w1 yearly"),
        (w1.duty_shape_ref, "d2", "windgen.w1 duty"),
        (i1.daily_shape_ref, "d1", "isource.i1 daily"),
        (i1.yearly_shape_ref, "d2", "isource.i1 yearly"),
        (i1.duty_shape_ref, "d2", "isource.i1 duty"),
        (pv.base.daily_shape_ref, "d1", "pvsystem.pv daily"),
        (pv.base.yearly_shape_ref, "d2", "pvsystem.pv yearly"),
        (pv.base.duty_shape_ref, "d2", "pvsystem.pv duty"),
    ] {
        let h = h.unwrap_or_else(|| panic!("{what} must narrow to Idx<LoadShape>"));
        assert_eq!(
            dss.classes[dss.class_by_name["loadshape"]]
                .arena
                .get::<LoadShapeObj>(h.get())
                .map(|s| s.data().name()),
            Some(want),
            "{what}"
        );
    }

    // (b) the XYcurve handles;
    for (h, want, what) in [
        (pv.power_temp_curve_ref, "pt", "pvsystem.pv P-TCurve"),
        (w1.vv_curve_ref, "vv", "windgen.w1 VV_Curve"),
        (w1.loss_curve_ref, "pl", "windgen.w1 PLoss"),
    ] {
        let h = h.unwrap_or_else(|| panic!("{what} must narrow to Idx<XYcurve>"));
        assert_eq!(
            dss.classes[dss.class_by_name["xycurve"]]
                .arena
                .get::<XyCurveObj>(h.get())
                .map(|c| c.data().name()),
            Some(want),
            "{what}"
        );
    }

    // (c) the shared `DynEqPCE` DynamicExp handle (one field, four classes).
    for (h, what) in [
        (g1.dyneq.dynamic_eq_ref, "generator.g1 dynamicexp"),
        (w1.dyneq.dynamic_eq_ref, "windgen.w1 dynamicexp"),
    ] {
        let h = h.unwrap_or_else(|| panic!("{what} must narrow to Idx<DynamicExp>"));
        assert_eq!(
            dss.classes[dss.class_by_name["dynamicexp"]]
                .arena
                .get::<DynamicExpObj>(h.get())
                .map(|e| e.data().name()),
            Some("de"),
            "{what}"
        );
    }

    // GICsource's `Idx<Line>` is private (no getter, and none is added just for
    // a test): its narrowing is proven by the `dump_gicsource` golden — a `None`
    // handle raises Pascal error 333 and skips the `GIC_<name>` bus splice the
    // golden pins.

    // A miss leaves the handle `None` (the Pascal NIL reference), not a stale
    // or foreign one.
    dss.command("New load.lb bus1=src daily=nope");
    let (lb_ci, lb_oi) = slot(&dss, "load", "lb");
    assert!(
        dss.classes[lb_ci]
            .arena
            .get::<Load>(lb_oi)
            .unwrap()
            .daily_shape_ref
            .is_none(),
        "an unresolved daily= must leave the typed handle None"
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
            .any(|e| e.text() == "Line.l5.LineCode: LineCode object \"nosuch\" not found."),
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

/// WP7.1 step 3b: a `spacing=`+`wires=` Line resolves the `LineSpacing` and the
/// conductor array end to end through the executive (parse → foreign-class
/// resolve → `FetchLineSpacing`/`SetWires` → `FMakeZFromSpacing`) and solves —
/// the full pipeline the inline `spacing_*` tests bypass (they call
/// `set_object_ref_array` directly). Also pins the `spacing`/`wires` dump
/// round-trip (`get_string`/`get_object_ref_names`), which no inline test covers.
#[test]
fn line_spacing_specified_resolves_and_solves() {
    let mut dss = Dss::new();
    dss.command("New circuit.sp basekv=12.47 phases=3");
    dss.command(
        "New WireData.w runits=m gmrunits=m radunits=m \
             rac=0.0003 gmrac=0.005 radius=0.01 normamps=400",
    );
    dss.command("New LineSpacing.s nconds=3 nphases=3 x=[0 1 2] h=[10 10 10] units=m");
    dss.command("New Line.l1 bus1=sourcebus bus2=b2 spacing=s wires=[w w w] length=1 units=m");
    dss.command("New Load.ld bus1=b2 phases=3 kv=12.47 kw=300 pf=0.95 model=1");
    dss.command("Set voltagebases=[12.47]");
    dss.command("CalcVoltageBases");
    dss.command("Set controlmode=off");
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    // The Line resolved the spacing + conductors and switched off the sym model.
    assert_eq!(query(&mut dss, "Line.l1.spacing"), "s");
    assert_eq!(query(&mut dss, "Line.l1.wires"), "[w, w, w]");
    assert_eq!(query(&mut dss, "Line.l1.phases"), "3");
    assert_eq!(query(&mut dss, "Line.l1.r1"), "----");

    let ckt = dss.circuit().unwrap();
    assert!(ckt.solution.converged_flag, "spacing line should converge");
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

/// WP7.1 step 4: `Set EarthModel=` sets the context default (`DSS.DefaultEarthModel`,
/// Pascal ExecOptions.pas:630) that each subsequently created line copies into
/// `FEarthModel` (Line.pas:998); a per-line `earthmodel=` still overrides it, and
/// a line created before the `Set` keeps the prior default (DERI). Unblocks the
/// 4Bus-* / Cable* corpus feeders.
#[test]
fn set_earthmodel_seeds_new_line_default() {
    let mut dss = Dss::new();
    dss.command("New circuit.em basekv=12.47 phases=3");
    // A line created before the Set keeps the default DERI.
    dss.command("New line.l0 bus1=a bus2=b");
    dss.command("Set earthmodel=Carson");
    dss.command("New line.l1 bus1=b bus2=c");
    dss.command("New line.l2 bus1=c bus2=d earthmodel=Deri"); // per-line override
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(query(&mut dss, "line.l0.earthmodel"), "Deri");
    assert_eq!(query(&mut dss, "line.l1.earthmodel"), "Carson");
    assert_eq!(query(&mut dss, "line.l2.earthmodel"), "Deri");
}

/// WP7.1 step 4 audit follow-up: unlike a context-global such as
/// `DefaultBaseFrequency`, `Set EarthModel=` requires a circuit — probed against
/// the pinned oracle, which raises #301 ("You must create a new circuit object
/// first") for a pre-circuit `Set earthmodel=`. So the option lives only in the
/// circuit-ful `do_set_cmd`, not `do_set_cmd_no_circuit`; this pins that the
/// no-circuit path is the faithful behavior (not an oversight to "fix").
#[test]
fn set_earthmodel_requires_a_circuit() {
    let mut dss = Dss::new();
    dss.command("Set earthmodel=Carson"); // no `New circuit` yet
    assert!(
        dss.errors()
            .iter()
            .any(|e| e.contains("You must create a new circuit object first")),
        "pre-circuit Set EarthModel must raise the #301 message (oracle-confirmed), got {:?}",
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
            .any(|e| e.text() == "Illegal change of number of phases for \"Line.l1\""),
        "expected the 18101 message, got {:?}",
        dss.errors()
    );
    // The illegal change was rejected: the phase count stays at 1.
    assert_eq!(query(&mut dss, "Line.l1.phases"), "1");
}
/// r4133 `Version8/Source/PDElements/Line.pas`: `switch=yes` (side-effect arm
/// 15, `:694-700`) does **not** clear `FLineCodeSpecified`, while the
/// neighbouring impedance arms `6..11, 26..27` (`:685`) and `12..14` (`:691`)
/// each open with `FLineCodeSpecified := FALSE`. The flag drives two live
/// surfaces: the property render (`3: If FLineCodeSpecified Then Result :=
/// CondCode else Result := ''`, `:1357`) and the `units=` conversion branch
/// (`20: If FLineCodeSpecified Then FUnitsConvert := ConvertLineUnits(
/// FLineCodeUnits, NewLengthUnits) Else …`, `:626-627`). dss_capi 0.14.5 added
/// a `KillLineCodeSpecified()` to arm 15 and flagged it in its own source
/// (`src/PDElements/Line.pas:677`, `//TODO: check if this missing is relevant
/// bug`); the port had copied it. r4133 is the behavioral authority
/// (CLAUDE.md 2026-08-02), so the kill is gone from that arm only.
///
/// Every value below was measured live on the r4133 DLL (RP3.6 probe,
/// 2026-08-29) and the assertions come in two kinds, both load-bearing:
///
/// * **discriminators** — they fail against the pre-fix engine, proven by
///   re-running it with the kill restored: `swk.linecode` (`""`), `swk.r1`
///   (`"1"`, not `1/304.8`), `swk.r1` after `units=kft` (`"304.8"`, not `"1"`),
///   `swk.linecode` after it, and `261249.linecode` (`""`). `linecode` alone
///   would also pass against a port that merely stopped erasing the name, so
///   `r1` — a channel independent of the string under test — carries the claim
///   that the flag really selects the `FUnitsConvert` branch;
/// * **invariance controls** — they hold on BOTH engines and would fail an
///   over-broad fix: `swn` (a switch with no code renders empty and takes the
///   shared FALSE branch), `ovr` (arm 6 still clears, so one arm changed and not
///   the mechanism), `swb` (`FetchLineCode` re-arms the flag when the code is
///   typed last) and the corpus shape's `r1`/`units`/`length`, which must not
///   move because `ConvertLineUnits(m, m) = ConvertLineUnits(none, m) = 1.0`.
///
/// Both lanes, no oracle, no feature gate.
#[test]
fn switch_yes_keeps_the_linecode_and_its_units_conversion() {
    // (1) The discriminating shape the corpus never has: a code in kft, the
    // lines in m, so the two `FUnitsConvert` branches differ by 304.8.
    let mut dss = Dss::new();
    dss.command("New circuit.p");
    dss.command("New LineCode.lckft nphases=3 r1=0.1 x1=0.2 r0=0.3 x0=0.6 c1=3 c0=1 units=kft");
    // `swk`: code, then the switch, then the units — the corpus order.
    dss.command("New Line.swk bus1=a bus2=b phases=3 linecode=lckft Switch=True units=m");
    // `swn`: a switch with no code at all — the FALSE branch, shared with capi.
    dss.command("New Line.swn bus1=a bus2=c phases=3 Switch=True units=m");
    // `ovr`: a code then an arm-6 override — that arm still kills the flag.
    dss.command("New Line.ovr bus1=a bus2=d phases=3 linecode=lckft r1=0.5 units=m");
    // `swb`: the switch typed *before* the code — `FetchLineCode` re-arms the
    // flag (r4133 `:413`), so this one never differed between the engines.
    dss.command("New Line.swb bus1=a bus2=e phases=3 Switch=True linecode=lckft units=m");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    // The render survives the switch (r4133 `? Line.swk.linecode` = `lckft`).
    assert_eq!(query(&mut dss, "line.swk.linecode"), "lckft");
    // …and so does the code-relative conversion: `FUnitsConvert =
    // ConvertLineUnits(kft, m) = 304.8`, so the `r1` getter renders
    // `R1/FUnitsConvert = 1/304.8`. r4133 prints its `%-.7g` truncation
    // `0.00328084`; the port prints the same f64 in full (the pre-existing
    // render-width class), so the value is checked as a number and the string
    // is pinned literally next to it.
    let r1 = query(&mut dss, "line.swk.r1");
    assert_eq!(r1, "0.00328083989501312");
    assert!(
        (r1.parse::<f64>().unwrap() - 1.0 / 304.8).abs() < 1e-15,
        "swk.r1 = {r1}, expected 1/304.8 (r4133 renders 0.00328084)"
    );
    // Without the flag the port would take the FALSE branch — `FUnitsConvert *=
    // ConvertLineUnits(none, m) = 1` — and answer a bare `1`, which is exactly
    // what a switch with no code answers here:
    assert_eq!(query(&mut dss, "line.swn.linecode"), "");
    assert_eq!(query(&mut dss, "line.swn.r1"), "1");

    // (2) The branch is re-evaluated from the code's units on every `units=`,
    // not latched at the switch: `ConvertLineUnits(kft, kft) = 1`.
    dss.command("Edit Line.swk units=kft");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(query(&mut dss, "line.swk.r1"), "1");
    assert_eq!(query(&mut dss, "line.swk.linecode"), "lckft");
    // The no-code line takes the shared FALSE branch on the same edit —
    // `FUnitsConvert *= ConvertLineUnits(m, kft) = 1/304.8` ⇒ `1/(1/304.8)`.
    dss.command("Edit Line.swn units=kft");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(query(&mut dss, "line.swn.r1"), "304.8");

    // (3) Only arm 15 changed, not the mechanism: arm 6 still clears the flag.
    assert_eq!(query(&mut dss, "line.ovr.linecode"), "");
    assert_eq!(query(&mut dss, "line.ovr.r1"), "0.5");
    // …and a switch typed before the code keeps it on every engine.
    assert_eq!(query(&mut dss, "line.swb.linecode"), "lckft");

    // (4) The corpus shape (`…/EPRI_Ckt7-G/Torn_Circuit/zone_2/Branches.dss:93`,
    // `LineCode.99` from `zone_2/LineCode.DSS:153`): the name comes back and the
    // impedance does **not** move, because both branches evaluate to 1.0 there —
    // `ConvertLineUnits(m, m) = ConvertLineUnits(none, m) = 1.0`. This is the
    // shape of all five census cells the two `capi_v0145` ledger entries pin.
    let mut dss = Dss::new();
    dss.command("New circuit.p");
    dss.command(
        "New LineCode.99 nphases=3 r1=0.00018641 x1=0.00039768 r0=0.00045981              x0=0.0011868 c1=1.8694E-006 c0=1.8694E-006 units=m",
    );
    dss.command(
        "New Line.261249 bus1=a bus2=b phases=3 length=0.001 linecode=99 Switch=True units=m",
    );
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(query(&mut dss, "line.261249.linecode"), "99");
    assert_eq!(query(&mut dss, "line.261249.r1"), "1");
    assert_eq!(query(&mut dss, "line.261249.units"), "m");
    assert_eq!(query(&mut dss, "line.261249.length"), "0.001");
}
