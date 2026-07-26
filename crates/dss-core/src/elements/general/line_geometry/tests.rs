use super::*;
use crate::elements::general::conductor_data::WireDataObj;
use crate::elements::general::conductor_data::wire_data;
use crate::elements::general::line_spacing::LineSpacingObj;
use crate::obj::arena::{ClassArena, ResolvedObj};
use crate::obj::base::DssObject;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropEngine};
use dss_parser::{Parser, ParserVars};

/// Apply a scalar property edit through the property engine (no foreign view
/// needed — object references are driven directly by [`set_ref`]/
/// [`set_ref_array`] below). The real parse + object-reference *resolution*
/// path is covered end-to-end against the oracle by the `props_roundtrip`
/// golden for the scalar `wire`/`cncable`/`tscable`, the `wires` array, and
/// `spacing`; the array-resolution abort is covered by the exec test
/// `line_geometry_undefined_wire_in_array_aborts`. (The plural `cncables=`/
/// `tscables=` forms are not yet golden-pinned — see STATUS: the oracle's
/// post-plural active-conductor value diverges and is under investigation.)
fn scalar(
    cls: &ClassProps,
    obj: &mut dyn DssObject,
    name: &str,
    value: &str,
) -> crate::diag::ErrorLog {
    let enums = EnumRegistry::new();
    let mut parser = Parser::new();
    let vars = ParserVars::new();
    let mut errors = crate::diag::ErrorLog::new();
    let idx = cls.property_index(name).expect("known property");
    let mut eng = PropEngine {
        parser: &mut parser,
        vars: &vars,
        enums: &enums,
        errors: &mut errors,
        foreign: None,
    };
    cls.edit_property(obj, idx, value, &mut eng).unwrap();
    errors.extend(obj.data_mut().take_errors());
    errors
}

/// Mirror the executive's `edit_property` for a single object reference: set
/// the (already-resolved) reference, record the set order, run side effects.
fn set_ref(cls: &ClassProps, obj: &mut dyn DssObject, name: &str, target: &ClassArena) {
    let idx = cls.property_index(name).expect("known property");
    let resolved = ResolvedObj::new(target, 0);
    obj.set_object_ref(idx, resolved.name().to_string(), Some(resolved));
    obj.data_mut().set_as_next_seq(idx);
    obj.side_effects(idx, 0);
}

/// A one-object arena holding a clone of `target` — the test stand-in for the
/// registry class arena a real `ObjectRef` resolve borrows from.
fn arena_of<T: crate::obj::arena::ArenaClass + Clone>(target: &T) -> ClassArena {
    let mut a = ClassArena::empty_for(T::CLASS_NAME).expect("registered class");
    a.push(target.clone()).expect("same class");
    a
}

/// Mirror `edit_property` for an object-reference array (`wires=`/...).
fn set_ref_array(cls: &ClassProps, obj: &mut dyn DssObject, name: &str, targets: &[ClassArena]) {
    let idx = cls.property_index(name).expect("known property");
    let refs: Vec<crate::obj::base::ObjectRefArrayItem> = targets
        .iter()
        .map(|a| {
            let r = ResolvedObj::new(a, 0);
            Some((r.name().to_string(), r))
        })
        .collect();
    obj.set_object_ref_array(idx, &refs);
    obj.data_mut().set_as_next_seq(idx);
    obj.side_effects(idx, 0);
}

fn build_wire(name: &str, edits: &[(&str, &str)]) -> WireDataObj {
    let enums = EnumRegistry::new();
    let cls = wire_data::class_props(&enums);
    let mut obj = WireDataObj::new(name);
    for (n, v) in edits {
        scalar(&cls, &mut obj, n, v);
    }
    obj
}

fn build_spacing(edits: &[(&str, &str)]) -> LineSpacingObj {
    let enums = EnumRegistry::new();
    let cls = crate::elements::general::line_spacing::class_props(&enums);
    let mut obj = LineSpacingObj::new("sp");
    for (n, v) in edits {
        scalar(&cls, &mut obj, n, v);
    }
    obj
}

fn get(cls: &ClassProps, obj: &dyn DssObject, name: &str) -> String {
    let enums = EnumRegistry::new();
    let idx = cls.property_index(name).unwrap();
    cls.get_value(obj, idx, &enums)
}

#[test]
fn defaults() {
    // Pascal Create: nconds=0, nphases=0, cond=1, reduce=No, linetype=oh,
    // empty object-ref arrays render `[]`, ratings `[ 0]`.
    let enums = EnumRegistry::new();
    let cls = class_props(&enums);
    let obj = LineGeometryObj::new("g1");
    assert_eq!(get(&cls, &obj, "nconds"), "0");
    assert_eq!(get(&cls, &obj, "nphases"), "0");
    assert_eq!(get(&cls, &obj, "cond"), "1");
    assert_eq!(get(&cls, &obj, "reduce"), "No");
    assert_eq!(get(&cls, &obj, "wires"), "[]");
    assert_eq!(get(&cls, &obj, "cncables"), "[]");
    assert_eq!(get(&cls, &obj, "normamps"), "0");
    assert_eq!(get(&cls, &obj, "ratings"), "[ 0]");
    assert_eq!(get(&cls, &obj, "linetype"), "oh");
}

#[test]
fn cond_wire_state_machine() {
    // The classic per-conductor edit: `cond=N wire=.. x=.. h=.. units=..`.
    // Only the active conductor's scalars are visible via `?`; the full
    // assignment shows through the `wires` array. NormAmps/EmergAmps default
    // from the first conductor.
    let enums = EnumRegistry::new();
    let cls = class_props(&enums);
    let acsr = build_wire("acsr", &[("normamps", "530"), ("radius", "0.0306")]);
    let mut g = LineGeometryObj::new("g1");
    scalar(&cls, &mut g, "nconds", "3");
    scalar(&cls, &mut g, "nphases", "3");
    scalar(&cls, &mut g, "cond", "1");
    set_ref(&cls, &mut g, "wire", &arena_of(&acsr));
    scalar(&cls, &mut g, "x", "-1.2909");
    scalar(&cls, &mut g, "h", "13.716");
    scalar(&cls, &mut g, "units", "m");
    scalar(&cls, &mut g, "cond", "2");
    set_ref(&cls, &mut g, "wire", &arena_of(&acsr));
    scalar(&cls, &mut g, "x", "0");
    scalar(&cls, &mut g, "h", "13.716");
    scalar(&cls, &mut g, "cond", "3");
    set_ref(&cls, &mut g, "wire", &arena_of(&acsr));
    scalar(&cls, &mut g, "x", "1.2909");
    scalar(&cls, &mut g, "h", "13.716");

    assert_eq!(get(&cls, &g, "cond"), "3"); // last active
    assert_eq!(get(&cls, &g, "wire"), "acsr");
    assert_eq!(get(&cls, &g, "x"), "1.2909");
    assert_eq!(get(&cls, &g, "h"), "13.716");
    assert_eq!(get(&cls, &g, "units"), "m"); // sticky from cond 1
    assert_eq!(get(&cls, &g, "normamps"), "530"); // defaulted from acsr
    assert_eq!(get(&cls, &g, "emergamps"), "795"); // 1.5 × 530 on the wire
    assert_eq!(get(&cls, &g, "wires"), "[acsr, acsr, acsr]");
    assert_eq!(get(&cls, &g, "cncables"), "[acsr, acsr, acsr]");
}

#[test]
fn wires_array_sets_active_to_last() {
    // The plural `wires=` form fills every slot and leaves ActiveCond at the
    // last conductor (Pascal `SetWires`).
    let enums = EnumRegistry::new();
    let cls = class_props(&enums);
    let acsr = build_wire("acsr", &[("normamps", "530")]);
    let mut g = LineGeometryObj::new("g1");
    scalar(&cls, &mut g, "nconds", "3");
    scalar(&cls, &mut g, "nphases", "3");
    set_ref_array(
        &cls,
        &mut g,
        "wires",
        &[arena_of(&acsr), arena_of(&acsr), arena_of(&acsr)],
    );
    assert_eq!(get(&cls, &g, "cond"), "3");
    assert_eq!(get(&cls, &g, "wires"), "[acsr, acsr, acsr]");
    assert_eq!(get(&cls, &g, "normamps"), "530");
}

#[test]
fn wires_wrong_count_errors() {
    // A count mismatch logs the "Unexpected number" error and fills nothing.
    let enums = EnumRegistry::new();
    let cls = class_props(&enums);
    let acsr = build_wire("acsr", &[]);
    let mut g = LineGeometryObj::new("g1");
    scalar(&cls, &mut g, "nconds", "3");
    scalar(&cls, &mut g, "nphases", "3");
    set_ref_array(&cls, &mut g, "wires", &[arena_of(&acsr), arena_of(&acsr)]);
    let errs = g.data_mut().take_errors();
    assert!(
        errs.iter().any(|e| e.contains("Unexpected number (2)")),
        "{errs:?}"
    );
    assert_eq!(get(&cls, &g, "wires"), "[, , ]"); // nothing filled
}

#[test]
fn spacing_copies_coordinates() {
    // `spacing=` copies the LineSpacing's coordinates/units into every
    // conductor.
    let enums = EnumRegistry::new();
    let cls = class_props(&enums);
    let acsr = build_wire("acsr", &[("normamps", "530")]);
    let sp = build_spacing(&[
        ("nconds", "3"),
        ("nphases", "3"),
        ("x", "-1.2909 0 1.2909"),
        ("h", "28.6 28.6 28.6"),
        ("units", "ft"),
    ]);
    let mut g = LineGeometryObj::new("g1");
    scalar(&cls, &mut g, "nconds", "3");
    scalar(&cls, &mut g, "nphases", "3");
    set_ref(&cls, &mut g, "spacing", &arena_of(&sp));
    set_ref_array(
        &cls,
        &mut g,
        "wires",
        &[arena_of(&acsr), arena_of(&acsr), arena_of(&acsr)],
    );
    assert!(g.data_mut().take_errors().is_empty());
    assert_eq!(get(&cls, &g, "spacing"), "sp");
    assert_eq!(get(&cls, &g, "cond"), "3");
    assert_eq!(get(&cls, &g, "x"), "1.2909"); // cond 3 from spacing
    assert_eq!(get(&cls, &g, "h"), "28.6");
    assert_eq!(get(&cls, &g, "units"), "ft");
}

#[test]
fn spacing_ratings_min_over_phase_conductors() {
    // WP-U1.2 row D3 (LineGeometry.pas:1060-1064): the Line's `makeZFromSpacing`
    // throwaway geometry derives NormAmps/EmergAmps as the MINIMUM over the PHASE
    // conductors (`j <= FNphases`), not conductor 1. Phase wires big/small/big +
    // a high-rated neutral: min over phases = small's 400/500; the neutral's 999
    // is excluded. Pinned to the capi015 probe (upgrade_spacing_ratings deck).
    let big = build_wire(
        "big",
        &[
            ("gmrac", "0.0244"),
            ("diam", "0.721"),
            ("rac", "0.306"),
            ("radunits", "in"),
            ("gmrunits", "ft"),
            ("runits", "mi"),
            ("normamps", "600"),
            ("emergamps", "800"),
        ],
    );
    let small = build_wire(
        "small",
        &[
            ("gmrac", "0.0244"),
            ("diam", "0.721"),
            ("rac", "0.306"),
            ("radunits", "in"),
            ("gmrunits", "ft"),
            ("runits", "mi"),
            ("normamps", "400"),
            ("emergamps", "500"),
        ],
    );
    let neut = build_wire(
        "neut",
        &[
            ("gmrac", "0.0244"),
            ("diam", "0.721"),
            ("rac", "0.306"),
            ("radunits", "in"),
            ("gmrunits", "ft"),
            ("runits", "mi"),
            ("normamps", "999"),
            ("emergamps", "999"),
        ],
    );
    let sp = build_spacing(&[
        ("nconds", "4"),
        ("nphases", "3"),
        ("x", "-1 0 1 0.5"),
        ("h", "28 28 28 24"),
        ("units", "ft"),
    ]);
    let wires: Vec<Option<ConductorObj>> = vec![
        Some(ConductorObj::Wire(big.clone())),
        Some(ConductorObj::Wire(small.clone())),
        Some(ConductorObj::Wire(big.clone())),
        Some(ConductorObj::Wire(neut.clone())),
    ];
    let mut g = LineGeometryObj::new("g_ratings");
    // eps=1.0, height_offset=0, height_unit=UNITS_M (4): the default medium.
    g.load_spacing_and_wires(&sp, &wires, 60.0, DERI, 1.0, 0.0, 4)
        .expect("spacing load succeeds");
    assert_eq!(g.norm_amps(), 400.0);
    assert_eq!(g.emerg_amps(), 500.0);
}

#[test]
fn spacing_wrong_wire_count_errors() {
    // A spacing with a different wire count logs error 10103.
    let enums = EnumRegistry::new();
    let cls = class_props(&enums);
    let sp = build_spacing(&[
        ("nconds", "2"),
        ("nphases", "2"),
        ("x", "0 1"),
        ("h", "10 10"),
    ]);
    let mut g = LineGeometryObj::new("g1");
    scalar(&cls, &mut g, "nconds", "3");
    scalar(&cls, &mut g, "nphases", "3");
    set_ref(&cls, &mut g, "spacing", &arena_of(&sp));
    let errs = g.data_mut().take_errors();
    assert!(
        errs.iter().any(|e| e.contains("wrong number of wires")),
        "{errs:?}"
    );
}

#[test]
fn make_like_copies_geometry_and_resets_active() {
    let enums = EnumRegistry::new();
    let cls = class_props(&enums);
    let acsr = build_wire("acsr", &[("normamps", "530")]);
    let mut src = LineGeometryObj::new("base");
    scalar(&cls, &mut src, "nconds", "3");
    scalar(&cls, &mut src, "nphases", "3");
    scalar(&cls, &mut src, "cond", "1");
    set_ref(&cls, &mut src, "wire", &arena_of(&acsr));
    scalar(&cls, &mut src, "x", "-1.29");
    scalar(&cls, &mut src, "h", "13.7");
    scalar(&cls, &mut src, "units", "m");
    scalar(&cls, &mut src, "cond", "2");
    set_ref(&cls, &mut src, "wire", &arena_of(&acsr));
    scalar(&cls, &mut src, "x", "0");
    scalar(&cls, &mut src, "h", "13.7");
    scalar(&cls, &mut src, "cond", "3");
    set_ref(&cls, &mut src, "wire", &arena_of(&acsr));
    scalar(&cls, &mut src, "x", "1.29");
    scalar(&cls, &mut src, "h", "13.7");
    scalar(&cls, &mut src, "reduce", "y");

    let mut dst = LineGeometryObj::new("g1");
    dst.make_like(&src);
    assert_eq!(get(&cls, &dst, "nconds"), "3");
    assert_eq!(get(&cls, &dst, "cond"), "1"); // reset by the nconds side effect
    assert_eq!(get(&cls, &dst, "x"), "-1.29"); // cond 1
    assert_eq!(get(&cls, &dst, "units"), "m");
    assert_eq!(get(&cls, &dst, "reduce"), "Yes");
    assert_eq!(get(&cls, &dst, "wires"), "[acsr, acsr, acsr]");
    assert_eq!(get(&cls, &dst, "normamps"), "530");
}

#[test]
fn wire_undefined_pushes_not_defined_error() {
    // An unresolved `wire=` leaves the active conductor NIL; the side effect
    // logs the Pascal 10103 "object was not defined" (the generic ObjectRef
    // parse logs its own 401 "not found" separately, upstream).
    let enums = EnumRegistry::new();
    let cls = class_props(&enums);
    let mut g = LineGeometryObj::new("g1");
    scalar(&cls, &mut g, "nconds", "3");
    scalar(&cls, &mut g, "nphases", "3");
    scalar(&cls, &mut g, "cond", "1");
    // Mirror the executive's edit for an unresolved scalar ObjectRef: NIL.
    let idx = cls.property_index("wire").unwrap();
    g.set_object_ref(idx, "doesnotexist".to_string(), None);
    g.data_mut().set_as_next_seq(idx);
    g.side_effects(idx, 0);
    let errs = g.data_mut().take_errors();
    assert!(
        errs.iter()
            .any(|e| e.contains("was not defined. Must be previously defined")),
        "{errs:?}"
    );
    assert_eq!(get(&cls, &g, "wire"), ""); // still empty
}

#[test]
fn wires_array_defaults_multi_season_ratings() {
    // The `wires=` array branch defaults the geometry's Seasons/Ratings from
    // the first conductor when its own are unset (the NumAmpRatings>1 /
    // AmpRatings-copy branches of `default_amps_from`).
    let enums = EnumRegistry::new();
    let cls = class_props(&enums);
    let w4 = build_wire(
        "w4",
        &[
            ("normamps", "530"),
            ("Seasons", "4"),
            ("Ratings", "400 450 500 550"),
        ],
    );
    let mut g = LineGeometryObj::new("g1");
    scalar(&cls, &mut g, "nconds", "3");
    scalar(&cls, &mut g, "nphases", "3");
    set_ref_array(
        &cls,
        &mut g,
        "wires",
        &[arena_of(&w4), arena_of(&w4), arena_of(&w4)],
    );
    assert_eq!(get(&cls, &g, "seasons"), "4");
    assert_eq!(get(&cls, &g, "ratings"), "[ 400 450 500 550]");
    assert_eq!(get(&cls, &g, "normamps"), "530");
    assert_eq!(get(&cls, &g, "emergamps"), "795");
}

// ----- matrix wiring (UpdateLineGeometryData / CalcMatrices) -------------
//
// These drive the full LineGeometry object path (nconds/cond/wire/x/h/units)
// and assert the resulting Z/Yc against the *same* dss-python oracle
// references the Carson-engine unit tests pin (support/line_constants/tests),
// proving the object→engine wiring (units, radius/GMR/Rdc/Rac, cable extras,
// Nphases, Reduce) is correct end to end.

use crate::elements::general::conductor_data::{CnDataObj, TsDataObj, cn_data, ts_data};
use crate::support::cmatrix::CMatrix;
use crate::support::line_constants::{DERI, SIMPLE_CARSON};

const M_UNIT: i32 = 4; // LineUnits::Meter code
const KM_UNIT: i32 = 3; // LineUnits::Km code (from_per_meter = 1000)
const MI_UNIT: i32 = 1; // LineUnits::Mile code
/// Truncated `Twopi * 60` — *exactly* the engine's `Fw` at 60 Hz (the engine
/// uses the same truncated `TWOPI = 6.283185307`), so dividing `Im(Yc)` by
/// `W60` here reuses the engine's own omega, not an independent `2*pi`.
#[allow(clippy::approx_constant)] // truncated upstream `Twopi`, mirrors the engine
const W60: f64 = 6.283185307 * 60.0;

fn assert_close(got: f64, want: f64, what: &str) {
    let tol = 1e-8 * want.abs().max(1e-12);
    assert!(
        (got - want).abs() <= tol,
        "{what}: got {got:.12e}, want {want:.12e}"
    );
}

fn assert_z(z: &CMatrix, z_ref: &[(f64, f64)], n: usize) {
    for i in 0..n {
        for j in 0..n {
            let (re, im) = z_ref[i * n + j];
            let g = z.get(i, j);
            assert_close(g.re, re, &format!("Z[{i}][{j}].re"));
            assert_close(g.im, im, &format!("Z[{i}][{j}].im"));
        }
    }
}

/// The shared 3-wire overhead wire (SI): rac = 3e-4 ohm/m, gmr = 0.005 m,
/// radius = 0.01 m. Rdc defaults from Rac (/1.02); capradius from radius.
fn build_si_wire() -> WireDataObj {
    build_wire(
        "w",
        &[
            ("runits", "m"),
            ("gmrunits", "m"),
            ("radunits", "m"),
            ("rac", "0.0003"),
            ("gmrac", "0.005"),
            ("radius", "0.01"),
        ],
    )
}

fn build_si_cn() -> CnDataObj {
    let enums = EnumRegistry::new();
    let cls = cn_data::class_props(&enums);
    let mut obj = CnDataObj::new("cn");
    // Same core conductor + insulation/strand data as the engine `build_cn`.
    for (n, v) in &[
        ("runits", "m"),
        ("gmrunits", "m"),
        ("radunits", "m"),
        ("rdc", "0.0001"),
        ("rac", "0.000105"),
        ("radius", "0.005"),
        ("gmrac", "0.004"),
        ("epsr", "2.3"),
        ("inslayer", "0.004"),
        ("diains", "0.022"),
        ("diacable", "0.030"),
        ("k", "16"),
        ("diastrand", "0.001"),
        ("gmrstrand", "0.0004"),
        ("rstrand", "0.002"),
    ] {
        scalar(&cls, &mut obj, n, v);
    }
    obj
}

fn build_si_ts() -> TsDataObj {
    let enums = EnumRegistry::new();
    let cls = ts_data::class_props(&enums);
    let mut obj = TsDataObj::new("ts");
    // Same core conductor + insulation/shield data as the engine `build_ts`.
    for (n, v) in &[
        ("runits", "m"),
        ("gmrunits", "m"),
        ("radunits", "m"),
        ("rdc", "0.0001"),
        ("rac", "0.000105"),
        ("radius", "0.005"),
        ("gmrac", "0.004"),
        ("epsr", "2.3"),
        ("inslayer", "0.004"),
        ("diains", "0.022"),
        ("diacable", "0.030"),
        ("diashield", "0.025"),
        ("tapelayer", "0.0002"),
        ("tapelap", "20"),
    ] {
        scalar(&cls, &mut obj, n, v);
    }
    obj
}

/// The canonical WP7.1 3-phase overhead geometry: `build_si_wire` at
/// x = 0/1/2 m, h = 10 m — the `deri_full_3cond` / `C3_NF` reference geometry,
/// driven entirely through the object editing path (nconds/cond/wire/x/h/units).
fn build_overhead_3() -> LineGeometryObj {
    let enums = EnumRegistry::new();
    let cls = class_props(&enums);
    let w = build_si_wire();
    let mut g = LineGeometryObj::new("g1");
    scalar(&cls, &mut g, "nconds", "3");
    scalar(&cls, &mut g, "nphases", "3");
    for (k, x) in ["0", "1", "2"].iter().enumerate() {
        scalar(&cls, &mut g, "cond", &(k + 1).to_string());
        set_ref(&cls, &mut g, "wire", &arena_of(&w));
        scalar(&cls, &mut g, "x", x);
        scalar(&cls, &mut g, "h", "10");
        scalar(&cls, &mut g, "units", "m");
    }
    g
}

#[test]
fn matrices_overhead_match_oracle() {
    // 3-phase overhead at x = 0/1/2 m, h = 10 m, DERI earth model — matches
    // the engine `deri_full_3cond` reference (Z) and `C3_NF` (capacitance).
    let mut g = build_overhead_3();

    let z = g.z_matrix(60.0, 1.0, M_UNIT, DERI).expect("z");
    let z_ref = [
        (3.525947626277e-04, 9.150978496084e-04),
        (5.807483299046e-05, 5.156141524879e-04),
        (5.807470316267e-05, 4.633520928126e-04),
        (5.807483299046e-05, 5.156141524879e-04),
        (3.525947626277e-04, 9.150978496084e-04),
        (5.807483299046e-05, 5.156141524879e-04),
        (5.807470316267e-05, 4.633520928126e-04),
        (5.807483299046e-05, 5.156141524879e-04),
        (3.525947626277e-04, 9.150978496084e-04),
    ];
    assert_z(&z, &z_ref, 3);

    let yc = g.yc_matrix(60.0, 1.0, M_UNIT, DERI).expect("yc");
    let c_ref_nf = [
        8.941431489720e-03,
        -2.907193315935e-03,
        -1.568246629107e-03,
        -2.907193315935e-03,
        9.611612264845e-03,
        -2.907193315935e-03,
        -1.568246629107e-03,
        -2.907193315935e-03,
        8.941431489720e-03,
    ];
    for i in 0..3 {
        for j in 0..3 {
            assert_close(
                yc.get(i, j).im / W60,
                c_ref_nf[i * 3 + j] * 1e-9,
                &format!("C[{i}][{j}]"),
            );
        }
    }
}

#[test]
fn matrices_equivalent_spacing_match_capi015() {
    // dss_capi 0.15.x equivalent-spacing model (WP-U1.4, LineSpacing.pas /
    // LineConstants.pas). A LineSpacing with `detailed=no` supplies the four
    // equivalent distances instead of per-conductor coordinates; the geometry
    // (nconds=4, nphases=3, reduce=y) Kron-reduces to a 3x3 phase matrix. The
    // reference is capi015 (0.15.0b4) reading `? line.l1.rmatrix/xmatrix` on the
    // identical deck at 60 Hz, DERI earth model, ohms/mi (probe 2026-07-16):
    //   rmatrix diag 0.410565535096229, off-diag 0.109545787814619
    //   xmatrix diag 0.988334662246234, off-diag 0.426587892673967
    let enums = EnumRegistry::new();
    let cls = class_props(&enums);
    let acsr = build_wire(
        "acsr",
        &[
            ("gmrac", "0.0244"),
            ("diam", "0.721"),
            ("rac", "0.306"),
            ("runits", "mi"),
            ("radunits", "in"),
            ("gmrunits", "ft"),
            ("normamps", "530"),
        ],
    );
    let sp = build_spacing(&[
        ("nconds", "4"),
        ("nphases", "3"),
        ("units", "ft"),
        ("detailed", "no"),
        ("EqDistPhPh", "2.5"),
        ("EqDistPhN", "4.272"),
        ("AvgPhaseHeight", "28.0"),
        ("AvgNeutralHeight", "24.0"),
    ]);
    let mut g = LineGeometryObj::new("g1");
    scalar(&cls, &mut g, "nconds", "4");
    scalar(&cls, &mut g, "nphases", "3");
    scalar(&cls, &mut g, "reduce", "y");
    set_ref(&cls, &mut g, "spacing", &arena_of(&sp));
    set_ref_array(
        &cls,
        &mut g,
        "wires",
        &[
            arena_of(&acsr),
            arena_of(&acsr),
            arena_of(&acsr),
            arena_of(&acsr),
        ],
    );
    assert!(g.data_mut().take_errors().is_empty());

    let z = g.z_matrix(60.0, 1.0, MI_UNIT, DERI).expect("z");
    // Reduced 3x3, ohms/mi.
    for i in 0..3 {
        for j in 0..3 {
            let want_re = if i == j {
                0.410565535096229
            } else {
                0.109545787814619
            };
            let want_im = if i == j {
                0.988334662246234
            } else {
                0.426587892673967
            };
            assert_close(z.get(i, j).re, want_re, &format!("Z[{i}][{j}].re"));
            assert_close(z.get(i, j).im, want_im, &format!("Z[{i}][{j}].im"));
        }
    }

    // Shunt capacitance (the equivalent-spacing Yc branch: dijp = 2*avg height /
    // avg_phase+avg_neutral, self term via Fcapradius directly). Reduced 3x3,
    // nF/mi. capi015 `? line.l1.cmatrix` on the identical deck (probe 2026-07-16):
    //   cmatrix diag 16.1618729173611, off-diag -4.08707678619493
    let yc = g.yc_matrix(60.0, 1.0, MI_UNIT, DERI).expect("yc");
    for i in 0..3 {
        for j in 0..3 {
            // Im(Yc)/omega = C (farads/mi); reference is nF/mi -> *1e-9.
            let want_nf = if i == j {
                16.1618729173611
            } else {
                -4.08707678619493
            };
            assert_close(
                yc.get(i, j).im / W60,
                want_nf * 1e-9,
                &format!("C[{i}][{j}]"),
            );
        }
    }
}

#[test]
fn matrices_overhead_km_scaled() {
    // The `length`/`units` args must scale the per-meter base: Z/Yc at
    // (length = 2, units = km) = the unit-length-meter result × 1000 × 2.
    // Guards the object→engine forward of `length`/`units` — a hardcoded 1.0,
    // a hardcoded meters, or a swapped length/units arg would all fail here.
    let mut g = build_overhead_3();
    let factor = 1000.0 * 2.0; // from_per_meter(km) * length

    let z_m = g.z_matrix(60.0, 1.0, M_UNIT, DERI).expect("z_m");
    let z_km = g.z_matrix(60.0, 2.0, KM_UNIT, DERI).expect("z_km");
    let yc_m = g.yc_matrix(60.0, 1.0, M_UNIT, DERI).expect("yc_m");
    let yc_km = g.yc_matrix(60.0, 2.0, KM_UNIT, DERI).expect("yc_km");
    for i in 0..3 {
        for j in 0..3 {
            assert_close(z_km.get(i, j).re, z_m.get(i, j).re * factor, "Zkm.re");
            assert_close(z_km.get(i, j).im, z_m.get(i, j).im * factor, "Zkm.im");
            assert_close(yc_km.get(i, j).re, yc_m.get(i, j).re * factor, "Yckm.re");
            assert_close(yc_km.get(i, j).im, yc_m.get(i, j).im * factor, "Yckm.im");
        }
    }
}

#[test]
fn matrices_overhead_simple_carson() {
    // The `earth_model` arg must reach the engine: the same geometry under
    // SIMPLE_CARSON yields the engine `simple_carson_full_3cond` reference,
    // distinct from the DERI matrix (a hardcoded earth model would fail this).
    let mut g = build_overhead_3();
    let z = g.z_matrix(60.0, 1.0, M_UNIT, SIMPLE_CARSON).expect("z");
    // capi015 reference (De 658.5 → 658.8530451057239, WP-U1.2 B2/D1); matches
    // the engine `simple_carson_full_3cond` capi015 reference.
    let z_ref = [
        (3.592176235097e-04, 9.081135553905e-04),
        (5.921762350974e-05, 5.086298569577e-04),
        (5.921762350974e-05, 4.563677933454e-04),
        (5.921762350974e-05, 5.086298569577e-04),
        (3.592176235097e-04, 9.081135553905e-04),
        (5.921762350974e-05, 5.086298569577e-04),
        (5.921762350974e-05, 4.563677933454e-04),
        (5.921762350974e-05, 5.086298569577e-04),
        (3.592176235097e-04, 9.081135553905e-04),
    ];
    assert_z(&z, &z_ref, 3);
}

#[test]
fn matrices_reduce_neutral_to_phases() {
    // 3 phases + 1 neutral at (1, 12); `reduce=y` Krons the neutral out, so
    // z_matrix returns the reduced 3×3 (engine `deri_reduce_4cond_to_3`).
    let enums = EnumRegistry::new();
    let cls = class_props(&enums);
    let w = build_si_wire();
    let mut g = LineGeometryObj::new("g1");
    scalar(&cls, &mut g, "nconds", "4");
    scalar(&cls, &mut g, "nphases", "3");
    scalar(&cls, &mut g, "reduce", "y");
    let coords = [("0", "10"), ("1", "10"), ("2", "10"), ("1", "12")];
    for (k, (x, h)) in coords.iter().enumerate() {
        scalar(&cls, &mut g, "cond", &(k + 1).to_string());
        set_ref(&cls, &mut g, "wire", &arena_of(&w));
        scalar(&cls, &mut g, "x", x);
        scalar(&cls, &mut g, "h", h);
        scalar(&cls, &mut g, "units", "m");
    }

    let z = g.z_matrix(60.0, 1.0, M_UNIT, DERI).expect("z");
    assert_eq!(z.order(), 3);
    let z_ref = [
        (3.770208623296e-04, 7.019407348531e-04),
        (8.343915794981e-05, 2.986360481570e-04),
        (8.250080286461e-05, 2.501949780572e-04),
        (8.343915794981e-05, 2.986360481570e-04),
        (3.789232335261e-04, 6.942314211641e-04),
        (8.343915794981e-05, 2.986360481570e-04),
        (8.250080286461e-05, 2.501949780572e-04),
        (8.343915794981e-05, 2.986360481570e-04),
        (3.770208623296e-04, 7.019407348531e-04),
    ];
    assert_z(&z, &z_ref, 3);

    // The reduced *shunt* Yc must also survive the Kron reduction through the
    // object path (engine `deri_reduce_4cond_to_3` capacitance reference).
    let yc = g.yc_matrix(60.0, 1.0, M_UNIT, DERI).expect("yc");
    let c_ref_nf = [
        9.210099265515e-03,
        -2.642486857248e-03,
        -1.299578853311e-03,
        -2.642486857248e-03,
        9.872415813254e-03,
        -2.642486857248e-03,
        -1.299578853311e-03,
        -2.642486857248e-03,
        9.210099265515e-03,
    ];
    for i in 0..3 {
        for j in 0..3 {
            assert_close(
                yc.get(i, j).im / W60,
                c_ref_nf[i * 3 + j] * 1e-9,
                &format!("Cr[{i}][{j}]"),
            );
        }
    }
}

#[test]
fn matrices_cn_cable_match_oracle() {
    // 3 buried concentric-neutral cables (h = -1.2 m, x = 0/0.1/0.2 m), DERI
    // — exercises the CN param transfer (k/DiaStrand/GMRStrand/RStrand,
    // EpsR/InsLayer/DiaIns/DiaCable). Engine `cn_cable_deri_3cond`.
    let enums = EnumRegistry::new();
    let cls = class_props(&enums);
    let cn = build_si_cn();
    let mut g = LineGeometryObj::new("g1");
    scalar(&cls, &mut g, "nconds", "3");
    scalar(&cls, &mut g, "nphases", "3");
    for (k, x) in ["0", "0.1", "0.2"].iter().enumerate() {
        scalar(&cls, &mut g, "cond", &(k + 1).to_string());
        set_ref(&cls, &mut g, "cncable", &arena_of(&cn));
        scalar(&cls, &mut g, "x", x);
        scalar(&cls, &mut g, "h", "-1.2");
        scalar(&cls, &mut g, "units", "m");
    }

    let z = g.z_matrix(60.0, 1.0, M_UNIT, DERI).expect("z");
    let z_ref = [
        (1.957766526264e-04, 1.402105660670e-04),
        (2.071515058850e-05, -1.736794561581e-05),
        (7.705339488750e-06, -1.452216497679e-05),
        (2.071515058850e-05, -1.736794561581e-05),
        (1.844408315520e-04, 1.418811316531e-04),
        (2.071515058850e-05, -1.736794561581e-05),
        (7.705339488750e-06, -1.452216497679e-05),
        (2.071515058850e-05, -1.736794561581e-05),
        (1.957766526264e-04, 1.402105660670e-04),
    ];
    assert_z(&z, &z_ref, 3);

    // Coaxial insulation capacitance: diagonal only, off-diagonals zero.
    let yc = g.yc_matrix(60.0, 1.0, M_UNIT, DERI).expect("yc");
    assert_close(yc.get(0, 0).im / W60, 2.830890564838e-01 * 1e-9, "C[0][0]");
    assert_close(yc.get(0, 1).im, 0.0, "C[0][1]");
}

#[test]
fn matrices_ts_cable_match_oracle() {
    // 3 buried tape-shield cables (h = -1.2 m, x = 0/0.1/0.2 m), DERI —
    // exercises the TS param transfer (DiaShield/TapeLayer/TapeLap,
    // EpsR/InsLayer/DiaIns/DiaCable). Engine `ts_cable_deri_3cond`.
    let enums = EnumRegistry::new();
    let cls = class_props(&enums);
    let ts = build_si_ts();
    let mut g = LineGeometryObj::new("g1");
    scalar(&cls, &mut g, "nconds", "3");
    scalar(&cls, &mut g, "nphases", "3");
    for (k, x) in ["0", "0.1", "0.2"].iter().enumerate() {
        scalar(&cls, &mut g, "cond", &(k + 1).to_string());
        set_ref(&cls, &mut g, "tscable", &arena_of(&ts));
        scalar(&cls, &mut g, "x", x);
        scalar(&cls, &mut g, "h", "-1.2");
        scalar(&cls, &mut g, "units", "m");
    }

    let z = g.z_matrix(60.0, 1.0, M_UNIT, DERI).expect("z");
    let z_ref = [
        (4.675825330004e-04, 4.983304721750e-04),
        (3.583518060982e-04, 2.468927740652e-04),
        (3.436688380811e-04, 2.058672035803e-04),
        (3.583518060982e-04, 2.468927740652e-04),
        (4.783159246306e-04, 4.782357442920e-04),
        (3.583518060982e-04, 2.468927740652e-04),
        (3.436688380811e-04, 2.058672035803e-04),
        (3.583518060982e-04, 2.468927740652e-04),
        (4.675825330004e-04, 4.983304721750e-04),
    ];
    assert_z(&z, &z_ref, 3);

    // Coaxial insulation capacitance: diagonal only, off-diagonals zero.
    let yc = g.yc_matrix(60.0, 1.0, M_UNIT, DERI).expect("yc");
    assert_close(yc.get(0, 0).im / W60, 2.830890564838e-01 * 1e-9, "C[0][0]");
    assert_close(yc.get(0, 1).im, 0.0, "C[0][1]");
}

#[test]
fn matrices_mixed_cn_ts_wire_match_capi015() {
    // dss_capi 0.15.x merged `TCableConstants`: one geometry with MIXED
    // per-conductor types — phase 1 CN, phase 2 TS, phase 3 CN, plus a bare
    // wire neutral (cond 4). This is impossible under the pre-0.15.x per-engine
    // CN/TS kind; the merged engine assigns each conductor's `FCondType` via
    // `SetCondType` in `UpdateLineGeometryData`. nconds=4/nphases=3/reduce=yes
    // → reduced 3x3. capi015 (dss-python 0.16.0b2 / dss_capi 0.15.0b4) reference
    // `? line.l1.rmatrix/xmatrix/cmatrix` on the identical deck (probe
    // 2026-07-16), ohm/m and nF/m.
    let enums = EnumRegistry::new();
    let cls = class_props(&enums);
    let cn = build_si_cn();
    let ts = build_si_ts();
    // Bare-wire neutral sharing the cable core (Rdc/Rac/GMR/radius) — matches
    // the probe's `WireData.w1`.
    let w = build_wire(
        "wn",
        &[
            ("runits", "m"),
            ("gmrunits", "m"),
            ("radunits", "m"),
            ("rdc", "0.0001"),
            ("rac", "0.000105"),
            ("radius", "0.005"),
            ("gmrac", "0.004"),
        ],
    );
    let mut g = LineGeometryObj::new("g1");
    scalar(&cls, &mut g, "nconds", "4");
    scalar(&cls, &mut g, "nphases", "3");
    scalar(&cls, &mut g, "reduce", "yes");
    let conds: [(&str, ClassArena, &str); 4] = [
        ("cncable", arena_of(&cn), "0"),
        ("tscable", arena_of(&ts), "0.1"),
        ("cncable", arena_of(&cn), "0.2"),
        ("wire", arena_of(&w), "0.3"),
    ];
    for (k, (prop_name, obj, x)) in conds.iter().enumerate() {
        scalar(&cls, &mut g, "cond", &(k + 1).to_string());
        set_ref(&cls, &mut g, prop_name, obj);
        scalar(&cls, &mut g, "x", x);
        scalar(&cls, &mut g, "h", "-1.2");
        scalar(&cls, &mut g, "units", "m");
    }

    let z = g.z_matrix(60.0, 1.0, M_UNIT, DERI).expect("z");
    let z_ref = [
        (2.043047617632e-04, 1.359764636804e-04),
        (5.238862037842e-05, 4.408760014295e-06),
        (1.340115107358e-05, -1.594581609220e-05),
        (5.238862037842e-05, 4.408760014295e-06),
        (1.649110726797e-04, 2.800828107382e-04),
        (4.559802639218e-05, 4.875809286083e-06),
        (1.340115107358e-05, -1.594581609220e-05),
        (4.559802639218e-05, 4.875809286083e-06),
        (1.936950836687e-04, 1.415056424162e-04),
    ];
    assert_z(&z, &z_ref, 3);

    // Capacitance: all three phases share identical insulation geometry and
    // the semicon default, so the reduced C is diagonal 0.2830890564838 nF/m.
    let yc = g.yc_matrix(60.0, 1.0, M_UNIT, DERI).expect("yc");
    for i in 0..3 {
        assert_close(
            yc.get(i, i).im / W60,
            2.830890564838e-01 * 1e-9,
            &format!("C[{i}][{i}]"),
        );
        for j in 0..3 {
            if i != j {
                assert_close(yc.get(i, j).im, 0.0, &format!("C[{i}][{j}]"));
            }
        }
    }
}

#[test]
fn conductors_array_matches_mixed_capi015() {
    // WP-U1.4 (wt-u14cond): the `Conductors=` array (prop 20). A real mixed list
    // reaches the object via the resolved-ref entry point
    // `set_object_ref_array(CONDUCTORS)` + the `CONDUCTORS` side effect — the path
    // the (now r4133-parity, case-insensitive) text parser AND JSON import call.
    // This drives that entry point directly (the `wires=` test precedent), gating
    // `apply_conductors` / per-conductor `change_line_constants_type` /
    // `default_amps_from` / `conductor_choice_of`.
    //
    // The identical MIXED geometry as `matrices_mixed_cn_ts_wire_match_capi015`
    // (phase-1 CN, phase-2 TS, phase-3 CN, cond-4 bare wire), but the per-
    // conductor kinds are assigned by ONE `Conductors=[cn, ts, cn, w]` list
    // instead of the `cond=/cncable=/tscable=/wire=` state machine. The reduced
    // 3x3 Z must equal the same capi015 reference.
    let enums = EnumRegistry::new();
    let cls = class_props(&enums);
    let cn = build_si_cn();
    let ts = build_si_ts();
    let w = build_wire(
        "wn",
        &[
            ("runits", "m"),
            ("gmrunits", "m"),
            ("radunits", "m"),
            ("rdc", "0.0001"),
            ("rac", "0.000105"),
            ("radius", "0.005"),
            ("gmrac", "0.004"),
        ],
    );
    let mut g = LineGeometryObj::new("g1");
    scalar(&cls, &mut g, "nconds", "4");
    scalar(&cls, &mut g, "nphases", "3");
    scalar(&cls, &mut g, "reduce", "yes");
    // Coordinates via the per-conductor scalars (no wire refs yet); the
    // conductor OBJECTS + engine kinds come from the `Conductors=` list below.
    for (k, x) in ["0", "0.1", "0.2", "0.3"].iter().enumerate() {
        scalar(&cls, &mut g, "cond", &(k + 1).to_string());
        scalar(&cls, &mut g, "x", x);
        scalar(&cls, &mut g, "h", "-1.2");
        scalar(&cls, &mut g, "units", "m");
    }
    set_ref_array(
        &cls,
        &mut g,
        "conductors",
        &[arena_of(&cn), arena_of(&ts), arena_of(&cn), arena_of(&w)],
    );
    assert!(g.data_mut().take_errors().is_empty());

    let z = g.z_matrix(60.0, 1.0, M_UNIT, DERI).expect("z");
    let z_ref = [
        (2.043047617632e-04, 1.359764636804e-04),
        (5.238862037842e-05, 4.408760014295e-06),
        (1.340115107358e-05, -1.594581609220e-05),
        (5.238862037842e-05, 4.408760014295e-06),
        (1.649110726797e-04, 2.800828107382e-04),
        (4.559802639218e-05, 4.875809286083e-06),
        (1.340115107358e-05, -1.594581609220e-05),
        (4.559802639218e-05, 4.875809286083e-06),
        (1.936950836687e-04, 1.415056424162e-04),
    ];
    assert_z(&z, &z_ref, 3);

    let yc = g.yc_matrix(60.0, 1.0, M_UNIT, DERI).expect("yc");
    for i in 0..3 {
        assert_close(
            yc.get(i, i).im / W60,
            2.830890564838e-01 * 1e-9,
            &format!("C[{i}][{i}]"),
        );
    }
}

#[test]
fn conductors_array_defaults_ratings_from_first_valid() {
    // The `Conductors=` list defaults the geometry ratings from the first valid
    // conductor (`apply_conductors` → `default_amps_from(first_valid)`), exactly
    // like the traditional `wires=` path. A wire with NormAmps=530 seeds the
    // geometry's NormAmps/EmergAmps (1.5× on the wire).
    let enums = EnumRegistry::new();
    let cls = class_props(&enums);
    let w = build_wire("acsr", &[("normamps", "530"), ("radius", "0.0306")]);
    let mut g = LineGeometryObj::new("g1");
    scalar(&cls, &mut g, "nconds", "3");
    scalar(&cls, &mut g, "nphases", "3");
    set_ref_array(
        &cls,
        &mut g,
        "conductors",
        &[arena_of(&w), arena_of(&w), arena_of(&w)],
    );
    assert!(g.data_mut().take_errors().is_empty());
    assert_eq!(get(&cls, &g, "normamps"), "530");
    assert_eq!(get(&cls, &g, "emergamps"), "795");
    // All-overhead list → every conductor's engine kind is Overhead.
    assert_eq!(get(&cls, &g, "wires"), "[acsr, acsr, acsr]");
}

#[test]
fn update_uninitialized_conductor_errors() {
    // A conductor slot left NIL is the Pascal "WireData is not correctly
    // initialized" hard error.
    let enums = EnumRegistry::new();
    let cls = class_props(&enums);
    let w = build_si_wire();
    let mut g = LineGeometryObj::new("g1");
    scalar(&cls, &mut g, "nconds", "3");
    scalar(&cls, &mut g, "nphases", "3");
    scalar(&cls, &mut g, "cond", "1");
    set_ref(&cls, &mut g, "wire", &arena_of(&w)); // only conductor 1 set
    let err = g.update_line_geometry_data(60.0, DERI).unwrap_err();
    assert!(err.contains("not correctly initialized"), "{err}");
}

#[test]
fn update_conductors_in_same_space_errors() {
    // Two fat conductors (radius 0.5 m) only 0.2 m apart overlap — the Pascal
    // ELineGeometryProblem / SolutionAbort path.
    let enums = EnumRegistry::new();
    let cls = class_props(&enums);
    let w = build_wire(
        "fat",
        &[
            ("runits", "m"),
            ("gmrunits", "m"),
            ("radunits", "m"),
            ("radius", "0.5"),
        ],
    );
    let mut g = LineGeometryObj::new("g1");
    scalar(&cls, &mut g, "nconds", "2");
    scalar(&cls, &mut g, "nphases", "2");
    for (k, x) in ["0", "0.2"].iter().enumerate() {
        scalar(&cls, &mut g, "cond", &(k + 1).to_string());
        set_ref(&cls, &mut g, "wire", &arena_of(&w));
        scalar(&cls, &mut g, "x", x);
        scalar(&cls, &mut g, "h", "10");
        scalar(&cls, &mut g, "units", "m");
    }
    let err = g.update_line_geometry_data(60.0, DERI).unwrap_err();
    assert!(err.contains("occupy the same space"), "{err}");
}

#[test]
fn make_like_cn_cable_recomputes() {
    // Pascal `MakeLike` rebuilds an *overhead* engine then runs
    // UpdateLineGeometryData, which `EInvalidCast`s for a cable source. We
    // clone the source engine instead, so `like=` a CN geometry does not
    // crash and reproduces the source's matrix (the documented divergence).
    let enums = EnumRegistry::new();
    let cls = class_props(&enums);
    let cn = build_si_cn();
    let mut src = LineGeometryObj::new("src");
    scalar(&cls, &mut src, "nconds", "3");
    scalar(&cls, &mut src, "nphases", "3");
    for (k, x) in ["0", "0.1", "0.2"].iter().enumerate() {
        scalar(&cls, &mut src, "cond", &(k + 1).to_string());
        set_ref(&cls, &mut src, "cncable", &arena_of(&cn));
        scalar(&cls, &mut src, "x", x);
        scalar(&cls, &mut src, "h", "-1.2");
        scalar(&cls, &mut src, "units", "m");
    }
    let z_src = src.z_matrix(60.0, 1.0, M_UNIT, DERI).expect("src z");

    let mut dst = LineGeometryObj::new("dst");
    dst.make_like(&src);
    // No crash (the Pascal bug averted), and the cloned CN engine reproduces
    // the matrix on first use (`data_changed` forces the recompute).
    let z_dst = dst.z_matrix(60.0, 1.0, M_UNIT, DERI).expect("dst z");
    for i in 0..3 {
        for j in 0..3 {
            assert_close(z_dst.get(i, j).re, z_src.get(i, j).re, "Z.re");
            assert_close(z_dst.get(i, j).im, z_src.get(i, j).im, "Z.im");
        }
    }
}

#[test]
fn z_matrix_recomputes_on_frequency_change() {
    // `f` must reach the engine through the object path and trigger a
    // recompute. Pin both endpoints to the oracle: Z at 60 Hz matches
    // `deri_full_3cond`; Z at 5 kHz matches `overhead_high_freq_radius_branch`
    // (the f >= 1 kHz radius / internal-reactance branch — an order-of-
    // magnitude-larger matrix, so a cached or hardcoded 60 Hz Z cannot pass).
    // Returning to 60 Hz must reproduce the original (recompute keys off `f`).
    let mut g = build_overhead_3();

    let z60 = g.z_matrix(60.0, 1.0, M_UNIT, DERI).expect("z60");
    let z60_ref = [
        (3.525947626277e-04, 9.150978496084e-04),
        (5.807483299046e-05, 5.156141524879e-04),
        (5.807470316267e-05, 4.633520928126e-04),
        (5.807483299046e-05, 5.156141524879e-04),
        (3.525947626277e-04, 9.150978496084e-04),
        (5.807483299046e-05, 5.156141524879e-04),
        (5.807470316267e-05, 4.633520928126e-04),
        (5.807483299046e-05, 5.156141524879e-04),
        (3.525947626277e-04, 9.150978496084e-04),
    ];
    assert_z(&z60, &z60_ref, 3);

    let z5000 = g.z_matrix(5000.0, 1.0, M_UNIT, DERI).expect("z5000");
    let z5000_ref = [
        (4.923757922619e-03, 5.945699655942e-02),
        (4.164436672700e-03, 2.984975434922e-02),
        (4.163753358720e-03, 2.549475346052e-02),
        (4.164436672700e-03, 2.984975434922e-02),
        (4.923757922619e-03, 5.945699655942e-02),
        (4.164436672700e-03, 2.984975434922e-02),
        (4.163753358720e-03, 2.549475346052e-02),
        (4.164436672700e-03, 2.984975434922e-02),
        (4.923757922619e-03, 5.945699655942e-02),
    ];
    assert_z(&z5000, &z5000_ref, 3);

    // Returning to 60 Hz reproduces the original (the recompute keys off `f`).
    let z60b = g.z_matrix(60.0, 1.0, M_UNIT, DERI).expect("z60b");
    assert_z(&z60b, &z60_ref, 3);
}

#[test]
fn cond_out_of_range_is_ignored() {
    // Pascal `set_ActiveCond` ignores values outside `1..=NConds`; the value
    // stays at the last valid conductor (the generic struct-index "Invalid
    // value" diagnostic is not reproduced — transformer wdg precedent).
    let enums = EnumRegistry::new();
    let cls = class_props(&enums);
    let mut g = LineGeometryObj::new("g1");
    scalar(&cls, &mut g, "nconds", "3");
    scalar(&cls, &mut g, "nphases", "3");
    scalar(&cls, &mut g, "cond", "2");
    scalar(&cls, &mut g, "cond", "99"); // above NConds -> ignored
    assert_eq!(get(&cls, &g, "cond"), "2");
    scalar(&cls, &mut g, "cond", "0"); // below 1 -> ignored
    assert_eq!(get(&cls, &g, "cond"), "2");
}
