//! WP7.1 step 3a — the `geometry=` Carson path. These drive a `Line` through
//! the property engine, attach a `LineGeometry`, and assert the resulting
//! `Z`/`Yc`/`YPrim` against the **same dss-python oracle reference**
//! (`deri_full_3cond`) the line-constants and LineGeometry matrix unit tests
//! pin — proving `FetchGeometryCode` + `FMakeZFromGeometry` forward
//! `f`/`len`/`units`/`earth_model` and embed the total matrices correctly.

use super::*;
use crate::elements::general::conductor_data::{CnDataObj, WireDataObj, cn_data, wire_data};
use crate::elements::general::line_geometry::{self, LineGeometryObj};
use crate::elements::general::line_spacing::{self, LineSpacingObj};
use crate::elements::traits::{CktElement, SysCtx};
use crate::obj::arena::{ClassArena, ResolvedObj};
use crate::obj::base::DssObject;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropEngine};
use crate::solution::{LoadSolutionModel, SolveMode};
use dss_parser::{Parser, ParserVars};

const M_UNIT: i32 = 4; // LineUnits::Meter code
const DERI: i32 = 3; // EarthModel::Deri code

fn assert_close(got: f64, want: f64, what: &str) {
    let tol = 1e-8 * want.abs().max(1e-12);
    assert!(
        (got - want).abs() <= tol,
        "{what}: got {got:.12e}, want {want:.12e}"
    );
}

/// Apply a scalar property edit through the property engine (object refs go
/// through [`set_ref`]). Asserts no deferred error was raised.
fn scalar(cls: &ClassProps, obj: &mut dyn DssObject, name: &str, value: &str) {
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

        was_quoted: false,
    };
    cls.edit_property(obj, idx, value, &mut eng).unwrap();
    errors.extend(obj.data_mut().take_errors());
    assert!(errors.is_empty(), "edit {name}={value}: {errors:?}");
}

/// Mirror the executive's `edit_property` for a single (resolved) object
/// reference: set the reference, record the set order, run side effects.
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

fn test_sys() -> SysCtx {
    SysCtx {
        frequency: 60.0,
        fundamental: 60.0,
        is_harmonic_model: false,
        is_dynamic_model: false,
        load_model: LoadSolutionModel::PowerFlow,
        mode: SolveMode::Snapshot,
        active_load_shape_class: crate::solution::USENONE,
        load_multiplier: 1.0,
        gen_multiplier: 1.0,
        generator_dispatch_reference: 0.0,
        price_signal: 25.0,
        default_growth_factor: 1.0,
        year: 0,
        dbl_hour: 0.0,
        solution_count: 0,
        iteration: 0,
        in_show_results: false,
        loads_need_updating: false,
        neglect_load_y: false,
        long_line_correction: false,
        positive_sequence: false,
        time_of_day: 0.0,
        dyna_h: 0.0,
        dyna_t: 0.0,
        iteration_flag: crate::support::dynamics::IterationFlag::NewTimeStep,
        last_solution_was_direct: false,
        ncim: false,
    }
}

/// The canonical WP7.1 3-phase overhead geometry (the `deri_full_3cond`
/// reference): `build_si_wire` at x = 0/1/2 m, h = 10 m, driven through the
/// LineGeometry editing path.
fn build_overhead_geometry() -> LineGeometryObj {
    let enums = EnumRegistry::new();
    let wcls = wire_data::class_props(&enums);
    let mut w = WireDataObj::new("w");
    for (n, v) in &[
        ("runits", "m"),
        ("gmrunits", "m"),
        ("radunits", "m"),
        ("rac", "0.0003"),
        ("gmrac", "0.005"),
        ("radius", "0.01"),
    ] {
        scalar(&wcls, &mut w, n, v);
    }
    let gcls = line_geometry::class_props(&enums);
    let mut g = LineGeometryObj::new("geo1");
    scalar(&gcls, &mut g, "nconds", "3");
    scalar(&gcls, &mut g, "nphases", "3");
    for (k, x) in ["0", "1", "2"].iter().enumerate() {
        scalar(&gcls, &mut g, "cond", &(k + 1).to_string());
        set_ref(&gcls, &mut g, "wire", &arena_of(&w));
        scalar(&gcls, &mut g, "x", x);
        scalar(&gcls, &mut g, "h", "10");
        scalar(&gcls, &mut g, "units", "m");
    }
    g
}

#[test]
fn geometry_path_builds_oracle_z_and_yc() {
    let enums = EnumRegistry::new();
    let lcls = class_props(&enums);

    let mut geom = build_overhead_geometry();
    // Oracle reference (the `deri_full_3cond` total matrices at 1 m): the
    // identical numbers the line_constants + LineGeometry unit tests pin.
    let z_ref = geom.z_matrix(60.0, 1.0, M_UNIT, DERI).expect("z_ref");
    let yc_ref = geom.yc_matrix(60.0, 1.0, M_UNIT, DERI).expect("yc_ref");

    let mut line = Line::new("l1");
    scalar(&lcls, &mut line, "length", "1");
    scalar(&lcls, &mut line, "units", "m");
    set_ref(&lcls, &mut line, "geometry", &arena_of(&geom));

    // FetchGeometryCode adopted the geometry.
    assert_eq!(line.geometry_name, "geo1");
    assert_eq!(line.cd.nphases, 3);
    assert_eq!(line.cd.nconds, 3);
    assert!(!line.sym_components_model);
    assert_eq!(line.line_type, geom.line_type());
    assert_eq!(line.norm_amps, geom.norm_amps());
    assert_eq!(line.emerg_amps, geom.emerg_amps());

    line.calc_yprim(&test_sys());

    // The geometry's TOTAL Z/Yc flowed into the Line (entry by entry) — the
    // sym branch would have produced a completely different matrix.
    let z = line.z.as_ref().expect("z");
    let yc = line.yc.as_ref().expect("yc");
    for i in 0..3 {
        for j in 0..3 {
            assert_close(z.get(i, j).re, z_ref.get(i, j).re, "Z.re");
            assert_close(z.get(i, j).im, z_ref.get(i, j).im, "Z.im");
            assert_close(yc.get(i, j).re, yc_ref.get(i, j).re, "Yc.re");
            assert_close(yc.get(i, j).im, yc_ref.get(i, j).im, "Yc.im");
        }
    }
    // Oracle anchor: Z[0][0] is the `deri_full_3cond` diagonal (length 1 m).
    assert_close(z.get(0, 0).re, 3.525947626277e-04, "Z00.re");
    assert_close(z.get(0, 0).im, 9.150978496084e-04, "Z00.im");
    // Independent oracle anchor for Yc[0][0] too, so the shunt is pinned to a
    // hardcoded reference and not merely compared against `yc_matrix`'s own
    // output: the `C3_NF` capacitance diagonal is 8.941431489720e-3 nF/m — the
    // same value `line_geometry::tests::matrices_overhead_match_oracle` pins —
    // and Yc is purely capacitive, so B = omega * C at the base frequency (omega
    // reuses the engine's truncated two-pi, exactly as that oracle test's `W60`).
    #[allow(clippy::approx_constant)] // truncated upstream `Twopi`, mirrors the engine
    let omega = 6.283185307 * 60.0;
    assert_close(
        yc.get(0, 0).im,
        8.941431489720e-03 * 1.0e-9 * omega,
        "Yc00.im",
    );

    // YPrim series embeds Zinv = Z^-1 in the 2-terminal Kron pattern: the
    // off-diagonal block [i][j+n] = -Zinv[i][j] (no CAP_EPSILON there).
    let mut zinv = z.clone();
    zinv.invert().expect("Z invertible");
    let yps = line.cd.yprim_series.as_ref().expect("yprim_series");
    for i in 0..3 {
        for j in 0..3 {
            assert_close(yps.get(i, j + 3).re, -zinv.get(i, j).re, "Yps.re");
            assert_close(yps.get(i, j + 3).im, -zinv.get(i, j).im, "Yps.im");
        }
    }
    // YPrim shunt = half the total Yc at the near-end block (already total —
    // not rescaled by length/frequency, unlike the sym path).
    let ypsh = line.cd.yprim_shunt.as_ref().expect("yprim_shunt");
    for i in 0..3 {
        for j in 0..3 {
            assert_close(ypsh.get(i, j).im, yc_ref.get(i, j).im / 2.0, "Ypsh.im");
        }
    }
}

#[test]
fn geometry_length_units_scale_the_total_z() {
    // `length`/`units` feed `Zmatrix[f, len, units]`, so a 2 km line is the
    // 1 m total × 1000 × 2 (guards the object→engine forward of len/units —
    // a hardcoded 1.0/meters or swapped arg would fail here).
    let enums = EnumRegistry::new();
    let lcls = class_props(&enums);
    let mut geom = build_overhead_geometry();
    let z_ref = geom.z_matrix(60.0, 1.0, M_UNIT, DERI).expect("z_ref");
    let factor = 1000.0 * 2.0; // from_per_meter(km) * length

    let mut line = Line::new("l1");
    scalar(&lcls, &mut line, "length", "2");
    scalar(&lcls, &mut line, "units", "km");
    set_ref(&lcls, &mut line, "geometry", &arena_of(&geom));
    line.calc_yprim(&test_sys());

    let z = line.z.as_ref().expect("z");
    for i in 0..3 {
        for j in 0..3 {
            assert_close(z.get(i, j).re, z_ref.get(i, j).re * factor, "Zkm.re");
            assert_close(z.get(i, j).im, z_ref.get(i, j).im * factor, "Zkm.im");
        }
    }
}

#[test]
fn sym_scalar_detaches_geometry() {
    // A sym-component edit after `geometry=` runs Pascal KillGeometrySpecified:
    // the geometry is dropped and the sym model takes over.
    let enums = EnumRegistry::new();
    let lcls = class_props(&enums);
    let geom = build_overhead_geometry();
    let mut line = Line::new("l1");
    set_ref(&lcls, &mut line, "geometry", &arena_of(&geom));
    assert!(line.geometry_obj.is_some());

    scalar(&lcls, &mut line, "r1", "0.1");
    assert!(line.geometry_obj.is_none());
    assert_eq!(line.geometry_name, "");
    assert!(line.sym_components_model);
    assert!(line.sym_components_changed);
    assert_eq!(line.fz_frequency, -1.0);
}

// --- WP7.1 step 3b — the `spacing=`/`wires=`/`cncables=` path ----------------
//
// Each test builds a spacing+conductor line and asserts its total `Z`/`Yc` equal
// the matrices the *equivalent* `geometry=` line produces — which the step-3a
// `geometry_tests` and the `line_geometry` unit tests pin entry-by-entry to the
// dss-python oracle. `probe_line_spacing.py` confirmed the two paths agree
// **exactly** (maxdiff 0) in the oracle for all three forms, so this transitively
// pins the spacing path; the hardcoded `Z[0][0]` anchors pin it directly too.

/// The shared 3-phase overhead wire `w` (matches `build_overhead_geometry`).
fn build_wire() -> WireDataObj {
    let enums = EnumRegistry::new();
    let wcls = wire_data::class_props(&enums);
    let mut w = WireDataObj::new("w");
    for (n, v) in &[
        ("runits", "m"),
        ("gmrunits", "m"),
        ("radunits", "m"),
        ("rac", "0.0003"),
        ("gmrac", "0.005"),
        ("radius", "0.01"),
    ] {
        scalar(&wcls, &mut w, n, v);
    }
    w
}

/// The shared CN cable `cn1` (the `probe_line_spacing.py` / line_constants
/// CN reference).
fn build_cn() -> CnDataObj {
    let enums = EnumRegistry::new();
    let ccls = cn_data::class_props(&enums);
    let mut c = CnDataObj::new("cn1");
    for (n, v) in &[
        ("runits", "m"),
        ("radunits", "m"),
        ("gmrunits", "m"),
        ("Rdc", "1.0e-4"),
        ("Rac", "1.05e-4"),
        ("GMRac", "0.004"),
        ("radius", "0.005"),
        ("capradius", "0.005"),
        ("EpsR", "2.3"),
        ("InsLayer", "0.004"),
        ("DiaIns", "0.022"),
        ("DiaCable", "0.030"),
        ("k", "16"),
        ("DiaStrand", "0.001"),
        ("GmrStrand", "0.0004"),
        ("Rstrand", "2.0e-3"),
    ] {
        scalar(&ccls, &mut c, n, v);
    }
    c
}

/// Build a `LineSpacing` with the given coordinates (all in meters).
fn build_spacing(
    name: &str,
    nconds: i32,
    nphases: i32,
    xs: &[&str],
    hs: &[&str],
) -> LineSpacingObj {
    let enums = EnumRegistry::new();
    let scls = line_spacing::class_props(&enums);
    let mut s = LineSpacingObj::new(name);
    scalar(&scls, &mut s, "nconds", &nconds.to_string());
    scalar(&scls, &mut s, "nphases", &nphases.to_string());
    // The `scalar` helper receives the value as the executive parser already
    // stripped the brackets — space-separated tokens, not `[...]`.
    scalar(&scls, &mut s, "x", &xs.join(" "));
    scalar(&scls, &mut s, "h", &hs.join(" "));
    scalar(&scls, &mut s, "units", "m");
    s
}

/// Apply an object-reference *array* edit (the `wires=`/`cncables=`/`tscables=`
/// forms), asserting no deferred error.
fn set_ref_array(cls: &ClassProps, obj: &mut dyn DssObject, name: &str, targets: &[ClassArena]) {
    let errs = try_ref_array(cls, obj, name, targets);
    assert!(errs.is_empty(), "set {name}: {errs:?}");
}

/// Like [`set_ref_array`] but returns the deferred errors (for the count-mismatch
/// path) instead of asserting.
fn try_ref_array(
    cls: &ClassProps,
    obj: &mut dyn DssObject,
    name: &str,
    targets: &[ClassArena],
) -> crate::diag::ErrorLog {
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
    obj.data_mut().take_errors().into()
}

/// Assert two 3×3 complex matrices match entry-by-entry, plus a hardcoded oracle
/// `Z[0][0]` anchor (a direct oracle pin, not merely self-comparison).
fn assert_zyc_match(line: &Line, z_ref: &CMatrix, yc_ref: &CMatrix, z00: (f64, f64), n: usize) {
    let z = line.z.as_ref().expect("z");
    let yc = line.yc.as_ref().expect("yc");
    for i in 0..n {
        for j in 0..n {
            assert_close(z.get(i, j).re, z_ref.get(i, j).re, "Z.re");
            assert_close(z.get(i, j).im, z_ref.get(i, j).im, "Z.im");
            assert_close(yc.get(i, j).re, yc_ref.get(i, j).re, "Yc.re");
            assert_close(yc.get(i, j).im, yc_ref.get(i, j).im, "Yc.im");
        }
    }
    assert_close(z.get(0, 0).re, z00.0, "Z00.re (oracle anchor)");
    assert_close(z.get(0, 0).im, z00.1, "Z00.im (oracle anchor)");
}

#[test]
fn spacing_wires_match_geometry_and_oracle() {
    let enums = EnumRegistry::new();
    let lcls = class_props(&enums);

    // Equivalent `geometry=` line → oracle-pinned total matrices (the same
    // `deri_full_3cond` reference the step-3a test anchors).
    let mut geom = build_overhead_geometry();
    let z_ref = geom.z_matrix(60.0, 1.0, M_UNIT, DERI).expect("z_ref");
    let yc_ref = geom.yc_matrix(60.0, 1.0, M_UNIT, DERI).expect("yc_ref");

    let w = build_wire();
    let s = build_spacing("sp3", 3, 3, &["0", "1", "2"], &["10", "10", "10"]);

    let mut line = Line::new("l1");
    scalar(&lcls, &mut line, "length", "1");
    scalar(&lcls, &mut line, "units", "m");
    set_ref(&lcls, &mut line, "spacing", &arena_of(&s));

    // FetchLineSpacing sized the Line to the spacing's phase count and allocated
    // the (still-empty) wire array; the spacing is not yet "specified".
    assert_eq!(line.cd.nphases, 3);
    assert_eq!(line.cd.nconds, 3);
    assert_eq!(line.line_wire_data.len(), 3);
    assert!(line.line_wire_data.iter().all(|w| w.is_none()));

    set_ref_array(
        &lcls,
        &mut line,
        "wires",
        &[arena_of(&w), arena_of(&w), arena_of(&w)],
    );
    assert!(line.spacing_specified);
    assert!(!line.sym_components_model);

    line.calc_yprim(&test_sys());
    assert_zyc_match(
        &line,
        &z_ref,
        &yc_ref,
        (3.525947626277e-04, 9.150978496084e-04),
        3,
    );
    // Independent Yc[0][0] oracle anchor (the overhead `C3_NF` diagonal × ω, the
    // step-3a trick): proves the shunt is pinned, not just compared to `yc_ref`.
    #[allow(clippy::approx_constant)] // truncated upstream `Twopi`, mirrors the engine
    let omega = 6.283185307 * 60.0;
    let yc = line.yc.as_ref().unwrap();
    assert_close(
        yc.get(0, 0).im,
        8.941431489720e-03 * 1.0e-9 * omega,
        "Yc00.im",
    );
}

#[test]
fn spacing_cncables_match_geometry() {
    let enums = EnumRegistry::new();
    let lcls = class_props(&enums);
    let gcls = line_geometry::class_props(&enums);

    let c = build_cn();
    // Equivalent `geometry=` line (CN, no reduce).
    let mut geom = LineGeometryObj::new("gcn");
    scalar(&gcls, &mut geom, "nconds", "3");
    scalar(&gcls, &mut geom, "nphases", "3");
    scalar(&gcls, &mut geom, "reduce", "no");
    for (k, x) in ["0", "0.1", "0.2"].iter().enumerate() {
        scalar(&gcls, &mut geom, "cond", &(k + 1).to_string());
        set_ref(&gcls, &mut geom, "cncable", &arena_of(&c));
        scalar(&gcls, &mut geom, "x", x);
        scalar(&gcls, &mut geom, "h", "-1.2");
        scalar(&gcls, &mut geom, "units", "m");
    }
    let z_ref = geom.z_matrix(60.0, 1.0, M_UNIT, DERI).expect("z_ref");
    let yc_ref = geom.yc_matrix(60.0, 1.0, M_UNIT, DERI).expect("yc_ref");

    let s = build_spacing("scn", 3, 3, &["0", "0.1", "0.2"], &["-1.2", "-1.2", "-1.2"]);
    let mut line = Line::new("l1");
    scalar(&lcls, &mut line, "length", "1");
    scalar(&lcls, &mut line, "units", "m");
    set_ref(&lcls, &mut line, "spacing", &arena_of(&s));
    set_ref_array(
        &lcls,
        &mut line,
        "cncables",
        &[arena_of(&c), arena_of(&c), arena_of(&c)],
    );

    // The cable form selected the ConcentricNeutral model.
    assert_eq!(line.fphase_choice, ConductorChoice::ConcentricNeutral);
    assert!(line.spacing_specified);

    line.calc_yprim(&test_sys());
    assert_zyc_match(
        &line,
        &z_ref,
        &yc_ref,
        (1.957766526264e-04, 1.402105660670e-04),
        3,
    );
}

/// The shared TS cable `ts1` (the `probe_line_constants.py` TS reference).
fn build_ts() -> crate::elements::general::conductor_data::TsDataObj {
    use crate::elements::general::conductor_data::{TsDataObj, ts_data};
    let enums = EnumRegistry::new();
    let tcls = ts_data::class_props(&enums);
    let mut t = TsDataObj::new("ts1");
    for (n, v) in &[
        ("runits", "m"),
        ("radunits", "m"),
        ("gmrunits", "m"),
        ("Rdc", "1.0e-4"),
        ("Rac", "1.05e-4"),
        ("GMRac", "0.004"),
        ("radius", "0.005"),
        ("capradius", "0.005"),
        ("EpsR", "2.3"),
        ("InsLayer", "0.004"),
        ("DiaIns", "0.022"),
        ("DiaCable", "0.030"),
        ("DiaShield", "0.025"),
        ("TapeLayer", "0.0002"),
        ("TapeLap", "20.0"),
    ] {
        scalar(&tcls, &mut t, n, v);
    }
    t
}

#[test]
fn spacing_tscables_match_geometry() {
    // The TapeShield form (the only conductor model with zero coverage otherwise):
    // tscables= over a spacing must equal the equivalent tscable= geometry.
    let enums = EnumRegistry::new();
    let lcls = class_props(&enums);
    let gcls = line_geometry::class_props(&enums);

    let t = build_ts();
    let mut geom = LineGeometryObj::new("gts");
    scalar(&gcls, &mut geom, "nconds", "3");
    scalar(&gcls, &mut geom, "nphases", "3");
    scalar(&gcls, &mut geom, "reduce", "no");
    for (k, x) in ["0", "0.1", "0.2"].iter().enumerate() {
        scalar(&gcls, &mut geom, "cond", &(k + 1).to_string());
        set_ref(&gcls, &mut geom, "tscable", &arena_of(&t));
        scalar(&gcls, &mut geom, "x", x);
        scalar(&gcls, &mut geom, "h", "-1.2");
        scalar(&gcls, &mut geom, "units", "m");
    }
    let z_ref = geom.z_matrix(60.0, 1.0, M_UNIT, DERI).expect("z_ref");
    let yc_ref = geom.yc_matrix(60.0, 1.0, M_UNIT, DERI).expect("yc_ref");

    let s = build_spacing("sts", 3, 3, &["0", "0.1", "0.2"], &["-1.2", "-1.2", "-1.2"]);
    let mut line = Line::new("l1");
    scalar(&lcls, &mut line, "length", "1");
    scalar(&lcls, &mut line, "units", "m");
    set_ref(&lcls, &mut line, "spacing", &arena_of(&s));
    set_ref_array(
        &lcls,
        &mut line,
        "tscables",
        &[arena_of(&t), arena_of(&t), arena_of(&t)],
    );

    assert_eq!(line.fphase_choice, ConductorChoice::TapeShield);
    line.calc_yprim(&test_sys());
    assert_zyc_match(
        &line,
        &z_ref,
        &yc_ref,
        (4.675825330004e-04, 4.983304721750e-04),
        3,
    );
}

#[test]
fn spacing_buried_neutral_via_cncables_then_wires() {
    // NWires=4, NPhases=3: 3 CN phases (`cncables=`) + 1 bare overhead neutral
    // (`wires=`). Validates the `SetWires` buried-neutral offset (the neutral must
    // land in slot 4 after the cable phases) and the 4→3 Kron reduce.
    let enums = EnumRegistry::new();
    let lcls = class_props(&enums);
    let gcls = line_geometry::class_props(&enums);

    let c = build_cn();
    let w = build_wire();
    let mut geom = LineGeometryObj::new("gbn");
    scalar(&gcls, &mut geom, "nconds", "4");
    scalar(&gcls, &mut geom, "nphases", "3");
    scalar(&gcls, &mut geom, "reduce", "yes");
    for (k, x) in ["0", "0.1", "0.2"].iter().enumerate() {
        scalar(&gcls, &mut geom, "cond", &(k + 1).to_string());
        set_ref(&gcls, &mut geom, "cncable", &arena_of(&c));
        scalar(&gcls, &mut geom, "x", x);
        scalar(&gcls, &mut geom, "h", "-1.2");
        scalar(&gcls, &mut geom, "units", "m");
    }
    scalar(&gcls, &mut geom, "cond", "4");
    set_ref(&gcls, &mut geom, "wire", &arena_of(&w));
    scalar(&gcls, &mut geom, "x", "0.1");
    scalar(&gcls, &mut geom, "h", "-1.0");
    scalar(&gcls, &mut geom, "units", "m");
    let z_ref = geom.z_matrix(60.0, 1.0, M_UNIT, DERI).expect("z_ref");
    let yc_ref = geom.yc_matrix(60.0, 1.0, M_UNIT, DERI).expect("yc_ref");

    let s = build_spacing(
        "sbn",
        4,
        3,
        &["0", "0.1", "0.2", "0.1"],
        &["-1.2", "-1.2", "-1.2", "-1.0"],
    );
    let mut line = Line::new("l1");
    scalar(&lcls, &mut line, "length", "1");
    scalar(&lcls, &mut line, "units", "m");
    set_ref(&lcls, &mut line, "spacing", &arena_of(&s));
    set_ref_array(
        &lcls,
        &mut line,
        "cncables",
        &[arena_of(&c), arena_of(&c), arena_of(&c)],
    );
    // The bare neutral: `wires=[w]` with FPhaseChoice=CN ⇒ istart=NPhases+1=4.
    set_ref_array(&lcls, &mut line, "wires", &[arena_of(&w)]);

    // Reduced to 3 phases (NConds=4 > NPhases=3 ⇒ FReduce).
    assert_eq!(line.cd.nphases, 3);
    assert!(line.line_wire_data[3].is_some(), "neutral landed in slot 4");

    line.calc_yprim(&test_sys());
    assert_zyc_match(
        &line,
        &z_ref,
        &yc_ref,
        (1.935274209913e-04, 1.418618628466e-04),
        3,
    );
}

#[test]
fn set_wires_wrong_count_errors() {
    // `wires=` with too few conductors (2 for a 3-wire spacing) is the Pascal
    // 18102 count error; the wire slots stay empty.
    let enums = EnumRegistry::new();
    let lcls = class_props(&enums);
    let w = build_wire();
    let s = build_spacing("sp3", 3, 3, &["0", "1", "2"], &["10", "10", "10"]);
    let mut line = Line::new("l1");
    set_ref(&lcls, &mut line, "spacing", &arena_of(&s));

    let errs = try_ref_array(&lcls, &mut line, "wires", &[arena_of(&w), arena_of(&w)]);
    assert!(
        errs.iter().any(|e| e.contains("Unexpected number")),
        "expected a count error, got {errs:?}"
    );
    assert!(line.line_wire_data.iter().all(|w| w.is_none()));
}

#[test]
fn sym_scalar_detaches_spacing() {
    // A sym edit after `spacing=`/`wires=` runs Pascal KillSpacingSpecified: the
    // spacing/wire array are dropped and the sym model takes over.
    let enums = EnumRegistry::new();
    let lcls = class_props(&enums);
    let w = build_wire();
    let s = build_spacing("sp3", 3, 3, &["0", "1", "2"], &["10", "10", "10"]);
    let mut line = Line::new("l1");
    set_ref(&lcls, &mut line, "spacing", &arena_of(&s));
    set_ref_array(
        &lcls,
        &mut line,
        "wires",
        &[arena_of(&w), arena_of(&w), arena_of(&w)],
    );
    assert!(line.spacing_specified);

    scalar(&lcls, &mut line, "r1", "0.1");
    assert!(!line.spacing_specified);
    assert!(line.line_spacing_obj.is_none());
    assert!(line.line_wire_data.is_empty());
    assert_eq!(line.fphase_choice, ConductorChoice::Unknown);
    assert!(line.sym_components_model);
}

#[test]
fn cncables_excess_count_drops_extras() {
    // `cncables=` with MORE cables than the spacing has wires: the oracle fills
    // the NWires slots and silently drops the extras (probed — `cncables=[4]` on a
    // 3-wire spacing solves identically to `cncables=[3]`). `set_cables` fills only
    // `k < NWires`, so the result equals the well-formed CN case.
    let enums = EnumRegistry::new();
    let lcls = class_props(&enums);
    let c = build_cn();
    let s = build_spacing("scn", 3, 3, &["0", "0.1", "0.2"], &["-1.2", "-1.2", "-1.2"]);
    let mut line = Line::new("l1");
    scalar(&lcls, &mut line, "length", "1");
    scalar(&lcls, &mut line, "units", "m");
    set_ref(&lcls, &mut line, "spacing", &arena_of(&s));
    // Four cables for a three-wire spacing — the fourth is dropped, no error.
    set_ref_array(
        &lcls,
        &mut line,
        "cncables",
        &[arena_of(&c), arena_of(&c), arena_of(&c), arena_of(&c)],
    );
    assert_eq!(line.line_wire_data.len(), 3);
    assert!(line.line_wire_data.iter().all(|w| w.is_some()));

    line.calc_yprim(&test_sys());
    let z = line.z.as_ref().expect("z");
    // Same CN diagonal as the well-formed `spacing_cncables_match_geometry` case.
    assert_close(z.get(0, 0).re, 1.957766526264e-04, "Z00.re");
    assert_close(z.get(0, 0).im, 1.402105660670e-04, "Z00.im");
}

#[test]
fn cncables_without_spacing_errors() {
    // `cncables=` before any `spacing=` leaves `LineWireData` unallocated — Pascal's
    // generic array fill raises error 402 (probe-confirmed). It must not silently
    // no-op into the sym model.
    let enums = EnumRegistry::new();
    let lcls = class_props(&enums);
    let c = build_cn();
    let mut line = Line::new("l1");
    let errs = try_ref_array(
        &lcls,
        &mut line,
        "cncables",
        &[arena_of(&c), arena_of(&c), arena_of(&c)],
    );
    assert!(
        errs.iter().any(|e| e.contains("No objects are expected")),
        "expected error 402, got {errs:?}"
    );
    assert!(line.line_wire_data.is_empty());
}

// --- WP-U1.4 (wt-u14cond) — the 0.15.x `Conductors=` array (Line prop 34) -----
//
// A real mixed `Conductors=` list reaches the object via the resolved-refs entry
// point `set_object_ref_array(CONDUCTORS)` + the `CONDUCTORS` side effect —
// exactly the path the (now r4133-parity, case-insensitive) text parser AND the
// JSON-import round-trip call. These whitebox tests drive that entry point
// directly (the established `wires=`/`cncables=` test precedent),
// gating `set_conductors` / `conductors_phase_choice` / `conductor_choice_of` /
// the `CONDUCTORS` side effect that were otherwise unreachable end-to-end.

/// A mixed `Conductors=[cn, cn, cn, wire]` over a 4-wire/3-phase spacing must
/// build the **same** total Z/Yc — and infer the same ConcentricNeutral model
/// with the neutral in slot 4 — as the traditional `cncables=[c,c,c]` +
/// `wires=[w]` buried-neutral sequence (`spacing_buried_neutral_via_*`), which
/// is oracle-pinned. Proves `conductors_phase_choice` (last valid phase
/// conductor → CN) and `set_conductors` (all four slots filled from index 0).
#[test]
fn conductors_array_matches_buried_neutral_and_oracle() {
    let enums = EnumRegistry::new();
    let lcls = class_props(&enums);
    let gcls = line_geometry::class_props(&enums);

    let c = build_cn();
    let w = build_wire();
    // Equivalent `geometry=` line → oracle-pinned total matrices (the same
    // reference the `spacing_buried_neutral_via_cncables_then_wires` case pins).
    let mut geom = LineGeometryObj::new("gbn");
    scalar(&gcls, &mut geom, "nconds", "4");
    scalar(&gcls, &mut geom, "nphases", "3");
    scalar(&gcls, &mut geom, "reduce", "yes");
    for (k, x) in ["0", "0.1", "0.2"].iter().enumerate() {
        scalar(&gcls, &mut geom, "cond", &(k + 1).to_string());
        set_ref(&gcls, &mut geom, "cncable", &arena_of(&c));
        scalar(&gcls, &mut geom, "x", x);
        scalar(&gcls, &mut geom, "h", "-1.2");
        scalar(&gcls, &mut geom, "units", "m");
    }
    scalar(&gcls, &mut geom, "cond", "4");
    set_ref(&gcls, &mut geom, "wire", &arena_of(&w));
    scalar(&gcls, &mut geom, "x", "0.1");
    scalar(&gcls, &mut geom, "h", "-1.0");
    scalar(&gcls, &mut geom, "units", "m");
    let z_ref = geom.z_matrix(60.0, 1.0, M_UNIT, DERI).expect("z_ref");
    let yc_ref = geom.yc_matrix(60.0, 1.0, M_UNIT, DERI).expect("yc_ref");

    let s = build_spacing(
        "sbn",
        4,
        3,
        &["0", "0.1", "0.2", "0.1"],
        &["-1.2", "-1.2", "-1.2", "-1.0"],
    );
    let mut line = Line::new("l1");
    scalar(&lcls, &mut line, "length", "1");
    scalar(&lcls, &mut line, "units", "m");
    set_ref(&lcls, &mut line, "spacing", &arena_of(&s));
    // The single mixed list — 3 CN phases + 1 bare neutral — instead of the
    // `cncables=` then `wires=` two-step. `set_ref_array` runs the resolved-ref
    // write + the `CONDUCTORS` side effect (what the parser would call).
    set_ref_array(
        &lcls,
        &mut line,
        "conductors",
        &[arena_of(&c), arena_of(&c), arena_of(&c), arena_of(&w)],
    );

    // The list inferred ConcentricNeutral (last valid phase conductor) and filled
    // every slot including the slot-4 neutral.
    assert_eq!(line.fphase_choice, ConductorChoice::ConcentricNeutral);
    assert!(line.spacing_specified);
    assert_eq!(line.cd.nphases, 3);
    assert!(line.line_wire_data[3].is_some(), "neutral landed in slot 4");
    // The redundant `wires=`/`cncables=`/`tscables=` set-marks are cleared.
    for p in [prop::WIRES, prop::CNCABLES, prop::TSCABLES] {
        assert!(!line.cd.obj.prp_specified(p), "prop {p} not cleared");
    }

    line.calc_yprim(&test_sys());
    assert_zyc_match(
        &line,
        &z_ref,
        &yc_ref,
        (1.935274209913e-04, 1.418618628466e-04),
        3,
    );
}

/// An all-overhead `Conductors=[w, w, w]` over a plain 3-wire spacing must build
/// the same matrices as the overhead `wires=[w,w,w]` case and default to the
/// Overhead model (the `phaseChoice = Unknown → Overhead` fallback).
#[test]
fn conductors_array_overhead_matches_wires_and_oracle() {
    let enums = EnumRegistry::new();
    let lcls = class_props(&enums);
    let mut geom = build_overhead_geometry();
    let z_ref = geom.z_matrix(60.0, 1.0, M_UNIT, DERI).expect("z_ref");
    let yc_ref = geom.yc_matrix(60.0, 1.0, M_UNIT, DERI).expect("yc_ref");

    let w = build_wire();
    let s = build_spacing("sp3", 3, 3, &["0", "1", "2"], &["10", "10", "10"]);
    let mut line = Line::new("l1");
    scalar(&lcls, &mut line, "length", "1");
    scalar(&lcls, &mut line, "units", "m");
    set_ref(&lcls, &mut line, "spacing", &arena_of(&s));
    set_ref_array(
        &lcls,
        &mut line,
        "conductors",
        &[arena_of(&w), arena_of(&w), arena_of(&w)],
    );

    // No cable in the phase set → Unknown, defaulted to Overhead by the side
    // effect's `if fphase_choice = Unknown then Overhead` fallback.
    assert_eq!(line.fphase_choice, ConductorChoice::Overhead);

    line.calc_yprim(&test_sys());
    assert_zyc_match(
        &line,
        &z_ref,
        &yc_ref,
        (3.525947626277e-04, 9.150978496084e-04),
        3,
    );
}

/// Redundancy / last-writer (minor audit item): an all-`none` `Conductors=`
/// (the one text-reachable form) after a `wires=` clears the redundant `Wires`
/// set-mark (Pascal `PrpSequence[Wires] := 0`) and NILs every slot, then
/// defaults the model to Overhead. Pins the `clear_seq(WIRES/CNCABLES/TSCABLES)`
/// in the `CONDUCTORS` side effect that dump/JSON emission ordering depends on.
#[test]
fn conductors_all_none_after_wires_clears_wires_seq() {
    let enums = EnumRegistry::new();
    let lcls = class_props(&enums);
    let w = build_wire();
    let s = build_spacing("sp3", 3, 3, &["0", "1", "2"], &["10", "10", "10"]);
    let mut line = Line::new("l1");
    set_ref(&lcls, &mut line, "spacing", &arena_of(&s));
    set_ref_array(
        &lcls,
        &mut line,
        "wires",
        &[arena_of(&w), arena_of(&w), arena_of(&w)],
    );
    // The `wires=` write recorded the Wires set-mark and filled every slot.
    assert!(line.cd.obj.prp_specified(prop::WIRES), "Wires mark set");
    assert!(line.line_wire_data.iter().all(|o| o.is_some()));

    // The all-`none` list (the only text-valid Conductors form) — NIL refs.
    let idx = lcls.property_index("conductors").expect("conductors prop");
    let none_refs: Vec<crate::obj::base::ObjectRefArrayItem> = vec![None, None, None];
    line.set_object_ref_array(idx, &none_refs);
    line.cd.obj.set_as_next_seq(idx);
    line.side_effects(idx, 0);

    // The redundant Wires mark is cleared and every slot is NIL again.
    assert!(
        !line.cd.obj.prp_specified(prop::WIRES),
        "Wires mark must be cleared by the Conductors last-writer"
    );
    assert!(line.line_wire_data.iter().all(|o| o.is_none()));
    // No cable/phase conductor left → Unknown, defaulted to Overhead.
    assert_eq!(line.fphase_choice, ConductorChoice::Overhead);
}

// --- WPG.21 — TLineObj.MakePosSequence (Line.pas:1531-1629) -------------------

use crate::elements::pos_seq::{PosSeqAction, PosSeqCtx};

/// The matrix branch: a 3-phase `rmatrix`/`xmatrix`/`cmatrix` line (deck
/// `makeposseq_line.dss` `l_mat`, units=km). The averaged positive-sequence
/// values match the oracle-observed `r1=0.21, x1=0.6` (and `c1=4.5` nF) — the
/// diagonal/off-diagonal averages of the input matrices.
#[test]
fn make_pos_sequence_matrix_branch() {
    let enums = EnumRegistry::new();
    let lcls = class_props(&enums);
    let mut line = Line::new("l_mat");
    scalar(&lcls, &mut line, "phases", "3");
    scalar(&lcls, &mut line, "length", "0.4");
    scalar(&lcls, &mut line, "units", "km");
    // The parser wraps the value in `[...]`, so pass the matrix rows unbracketed.
    scalar(
        &lcls,
        &mut line,
        "rmatrix",
        "0.3 | 0.09 0.3 | 0.09 0.09 0.3",
    );
    scalar(&lcls, &mut line, "xmatrix", "1.0 | 0.4 1.0 | 0.4 0.4 1.0");
    scalar(
        &lcls,
        &mut line,
        "cmatrix",
        "3.4 | -1.1 3.4 | -1.1 -1.1 3.4",
    );
    assert!(!line.sym_components_model, "matrix input → matrix model");
    // Z/Yc exist post-parse (RecalcElementData ran at edit time).
    assert!(line.z.is_some() && line.yc.is_some());

    // Pascal `ResetLengthUnits` fires when the matrices are set (Line.pas:677),
    // so `units=km` is reset to None — the stored Z is the raw per-unit matrix
    // and `units_convert` is 1.0.
    assert_eq!(
        line.length_units,
        crate::support::line_units::LineUnits::None
    );
    assert_eq!(line.units_convert, 1.0);

    let plan = line.make_pos_sequence(&PosSeqCtx::default());
    assert!(plan.run_base, "matrix branch ends with `inherited`");

    // Exact Pascal call shape: BeginEdit, R1/X1/C1/Phases, NormAmps/EmergAmps,
    // Units, EndEdit.
    use PosSeqAction::*;
    assert_eq!(plan.actions[0], BeginEdit);
    match plan.actions[1] {
        SetF64(idx, v) => {
            assert_eq!(idx, prop::R1);
            assert!((v - 0.21).abs() < 1e-12, "r1 got {v}");
        }
        ref a => panic!("expected SetF64(R1), got {a:?}"),
    }
    match plan.actions[2] {
        SetF64(idx, v) => {
            assert_eq!(idx, prop::X1);
            assert!((v - 0.6).abs() < 1e-12, "x1 got {v}");
        }
        ref a => panic!("expected SetF64(X1), got {a:?}"),
    }
    match plan.actions[3] {
        SetF64(idx, v) => {
            assert_eq!(idx, prop::C1);
            assert!((v - 4.5).abs() < 1e-9, "c1 got {v}");
        }
        ref a => panic!("expected SetF64(C1), got {a:?}"),
    }
    assert_eq!(plan.actions[4], SetI32(prop::PHASES, 1));
    assert_eq!(plan.actions[5], SetF64(prop::NORMAMPS, 400.0));
    assert_eq!(plan.actions[6], SetF64(prop::EMERGAMPS, 600.0));
    assert_eq!(plan.actions[7], SetI32(prop::UNITS, 0)); // None (reset by matrix)
    assert_eq!(plan.actions[8], EndEdit);
    assert_eq!(plan.actions.len(), 9);

    // PrpSequence marks were cleared (direct self-mutation).
    for p in [prop::R1, prop::X1, prop::R0, prop::X0, prop::C1, prop::C0] {
        assert!(!line.cd.obj.prp_specified(p), "prop {p} not cleared");
    }
    for p in [prop::RMATRIX, prop::XMATRIX, prop::CMATRIX, prop::LINECODE] {
        assert!(!line.cd.obj.prp_specified(p), "prop {p} not cleared");
    }
}

/// The symmetrical-components branch keeps the existing Z1 (R1, X1) and converts
/// C1 to nF (× 1e9). No matrix averaging.
#[test]
fn make_pos_sequence_symcomponents_branch() {
    let enums = EnumRegistry::new();
    let lcls = class_props(&enums);
    let mut line = Line::new("l_sym");
    scalar(&lcls, &mut line, "phases", "3");
    scalar(&lcls, &mut line, "r1", "0.3");
    scalar(&lcls, &mut line, "x1", "0.6");
    scalar(&lcls, &mut line, "c1", "3.4"); // nF → stored 3.4e-9 F
    assert!(line.sym_components_model);

    let plan = line.make_pos_sequence(&PosSeqCtx::default());
    use PosSeqAction::*;
    assert_eq!(plan.actions[0], BeginEdit);
    assert_eq!(plan.actions[1], SetF64(prop::R1, 0.3));
    assert_eq!(plan.actions[2], SetF64(prop::X1, 0.6));
    match plan.actions[3] {
        SetF64(idx, v) => {
            assert_eq!(idx, prop::C1);
            assert!((v - 3.4).abs() < 1e-9, "c1 nF got {v}");
        }
        ref a => panic!("expected SetF64(C1), got {a:?}"),
    }
    assert_eq!(plan.actions[4], SetI32(prop::PHASES, 1));
    assert_eq!(plan.actions.len(), 9);
    assert!(plan.run_base);
}

/// The `switch=yes` branch: fixed switch constants (R1=1, X1=1, C1=1.1,
/// Phases=1, Length=0.001).
#[test]
fn make_pos_sequence_switch_branch() {
    let enums = EnumRegistry::new();
    let lcls = class_props(&enums);
    let mut line = Line::new("l_sw");
    scalar(&lcls, &mut line, "phases", "3");
    scalar(&lcls, &mut line, "switch", "yes");
    assert!(line.is_switch);

    let plan = line.make_pos_sequence(&PosSeqCtx::default());
    use PosSeqAction::*;
    assert_eq!(
        plan.actions,
        vec![
            BeginEdit,
            SetF64(prop::R1, 1.0),
            SetF64(prop::X1, 1.0),
            SetF64(prop::C1, 1.1),
            SetI32(prop::PHASES, 1),
            SetF64(prop::LENGTH, 0.001),
            SetF64(prop::NORMAMPS, 400.0),
            SetF64(prop::EMERGAMPS, 600.0),
            SetI32(prop::UNITS, 0), // default units (None)
            EndEdit,
        ]
    );
    assert!(plan.run_base);
}

/// An already single-phase line is left alone — only the base bus rename runs
/// (`inherited`), no property actions.
#[test]
fn make_pos_sequence_single_phase_is_base_only() {
    let enums = EnumRegistry::new();
    let lcls = class_props(&enums);
    let mut line = Line::new("l1");
    scalar(&lcls, &mut line, "phases", "1");
    scalar(&lcls, &mut line, "r1", "0.3");

    let plan = line.make_pos_sequence(&PosSeqCtx::default());
    assert!(plan.actions.is_empty());
    assert!(plan.run_base);
}

// --- Long-line correction (Line.pas:1046 DoLongLine / :1199,1369) --------------

/// Build a 3-phase sym-components line (r0≠r1) matching the oracle reference
/// deck `llc_ref`: r1=0.05 x1=0.35 r0=0.15 x0=1.05 c1=3.4 c0=1.6 nF, 120 mi.
fn build_llc_line() -> Line {
    let enums = EnumRegistry::new();
    let lcls = class_props(&enums);
    let mut line = Line::new("ll");
    for (n, v) in &[
        ("phases", "3"),
        ("r1", "0.05"),
        ("x1", "0.35"),
        ("r0", "0.15"),
        ("x0", "1.05"),
        ("c1", "3.4"),
        ("c0", "1.6"),
        ("length", "120"),
        ("units", "mi"),
    ] {
        scalar(&lcls, &mut line, n, v);
    }
    line
}

/// The SymComponentsModel long-line correction matches the pinned oracle YPrim
/// (dss-python 0.14.5, `Set LongLineCorrection=yes`) entry-for-entry. Regression
/// for the ported `DoLongLine` + the `long_line` Z/Yc/shunt branches. The values
/// are the full YPrim (series + shunt + CAP_EPSILON) read from the oracle for
/// the `llc_ref` deck — a 120-mile r0≠r1 line where the correction is ~0.3 %.
#[test]
fn long_line_correction_matches_oracle_yprim() {
    let mut sys = test_sys();
    sys.long_line_correction = true;
    let mut line = build_llc_line();
    line.calc_yprim(&sys);

    let yp = line.cd.yprim.as_ref().expect("yprim built");
    // Oracle anchors (column-major full YPrim).
    let (r00, i00) = (2.592595398246e-03, -1.810590551924e-02);
    let (r03, i03) = (-2.592590137414e-03, 1.816927682269e-02);
    assert_close(yp.get(0, 0).re, r00, "Y00.re");
    assert_close(yp.get(0, 0).im, i00, "Y00.im");
    assert_close(yp.get(0, 3).re, r03, "Y03.re");
    assert_close(yp.get(0, 3).im, i03, "Y03.im");
    // Phase symmetry of a sym-components line: Y[1,1] == Y[0,0].
    assert_close(yp.get(1, 1).re, r00, "Y11.re");
    assert_close(yp.get(1, 1).im, i00, "Y11.im");
}

/// The correction is actually wired: with `long_line_correction` the series
/// YPrim differs from the uncorrected build for a long line (and the flag is
/// respected, not ignored). Guards against the pre-port state where the flag was
/// stored but never applied.
#[test]
fn long_line_correction_changes_the_long_line_yprim() {
    let mut off = build_llc_line();
    off.calc_yprim(&test_sys()); // long_line_correction = false
    let y_off = off.cd.yprim.as_ref().expect("yprim off").get(0, 0);

    let mut sys_on = test_sys();
    sys_on.long_line_correction = true;
    let mut on = build_llc_line();
    on.calc_yprim(&sys_on);
    let y_on = on.cd.yprim.as_ref().expect("yprim on").get(0, 0);

    let rel = (y_on - y_off).norm() / y_off.norm();
    assert!(
        rel > 1e-4,
        "long-line correction must move the 120-mi YPrim (rel change {rel:.3e})"
    );
}

/// WP-U1.5 E2 / 0.15.x-adoption sweep (EPRI r4133 PDElement.pas l.351):
/// `TPDElement.GetRatings` overrides the (norm, emerg) ratings with
/// `AmpRatings[SeasonalRatingIdx]` for ANY PDElement (0.14.5's `DI_Overloads`
/// path restricted this to lines) — but ONLY when the element carries more than
/// one season (`NumAmpRatings > 1`). Both norm and emerg take the same seasonal
/// value. Feature-sensitive: the `-1`/out-of-range guard must fall back to the
/// base ratings (a regression that dropped the guard would index the array).
#[test]
fn get_ratings_applies_seasonal_index() {
    let mut line = Line::new("l1");
    line.norm_amps = 100.0;
    line.emerg_amps = 120.0;
    line.num_amp_ratings = 4;
    line.amp_ratings = vec![100.0, 50.0, 40.0, 30.0];
    // In range → both norm & emerg == AmpRatings[idx].
    assert_eq!(line.get_ratings(2), (40.0, 40.0));
    assert_eq!(line.get_ratings(0), (100.0, 100.0));
    // Inactive (-1) or out of range → base NormAmps/EmergAmps.
    assert_eq!(line.get_ratings(-1), (100.0, 120.0));
    assert_eq!(line.get_ratings(4), (100.0, 120.0));
    // r4133's guard is `(RatingIdx <= NumAmpRatings) and (NumAmpRatings > 1)`.
    // A SINGLE-season element (`NumAmpRatings == 1`, the default) keeps its base
    // `(NormAmps, EmergAmps)` at every index — it never binds `AmpRatings[0]`.
    // dss_capi 0.15.x `55400a29` dropped the `> 1` guard (a single-season element
    // would silently take the stale constructor default `AmpRatings[0]` for BOTH
    // norm and emerg, hiding real overloads) — proven a bug vs r4133 + physics
    // (0.15.x-adoption sweep, DIVERGENCES L4/E2); the port follows r4133.
    line.num_amp_ratings = 1;
    line.amp_ratings = vec![50.0];
    assert_eq!(line.get_ratings(0), (100.0, 120.0));
    assert_eq!(line.get_ratings(1), (100.0, 120.0));
}

/// `ResetLengthUnits` clears `LengthUnits`, never `FUserLengthUnits` — r4133
/// `Version8/Source/PDElements/Line.pas:2326-2331` and dss_capi 0.14.5
/// `src/PDElements/Line.pas:2080-2085`, which carry the identical statement pair
/// and the identical comment "but do not erase FUserLengthUnits, in case of CIM
/// export".
///
/// The port cleared it too (RP3.5, 2026-08-28) — a port-authored divergence from
/// **both** oracles, so there was no authority question to weigh. It is not
/// theoretical: `FUserLengthUnits` is the units the CIM writer converts
/// `Conductor.length` with (r4133 `Common/ExportCIMXML.pas:3707`, `:3735`,
/// `:3877`), and a deck that types `units=` before its impedances exported a
/// length short by the whole unit factor —
/// [`crate::cim`]'s observable half of this pin lives in
/// `golden_cim::cim_conductor_length_uses_the_users_length_units`.
///
/// All three callers are exercised (`accessors.rs`' `R1..B0`,
/// `RMATRIX..CMATRIX` and `SWITCH` side-effect arms), because the field is
/// cleared — or not — in one shared helper and a per-arm regression would
/// otherwise hide behind whichever arm the pin happened to pick. The
/// discriminator is the pair: after each override `length_units` must be `None`
/// (the reset really ran) while `user_length_units` still reads `Kft` (the half
/// that must not move).
#[test]
fn reset_length_units_keeps_the_users_units() {
    for (prop_name, value) in [
        ("r1", "0.3"),        // side-effect arm R1|X1|R0|X0|C1|C0|B1|B0
        ("rmatrix", "[0.3]"), // side-effect arm RMATRIX|XMATRIX|CMATRIX
        ("switch", "yes"),    // side-effect arm SWITCH
    ] {
        let enums = EnumRegistry::new();
        let lcls = class_props(&enums);
        let mut line = Line::new("l_ulu");
        scalar(&lcls, &mut line, "phases", "1");
        scalar(&lcls, &mut line, "length", "2");
        scalar(&lcls, &mut line, "units", "kft");
        assert_eq!(line.length_units, LineUnits::Kft, "{prop_name}: setup");
        assert_eq!(
            line.user_length_units,
            LineUnits::Kft,
            "{prop_name}: the `units=` arm records the user's units \
             (r4133 Line.pas:629)"
        );

        scalar(&lcls, &mut line, prop_name, value);
        assert_eq!(
            line.length_units,
            LineUnits::None,
            "{prop_name}: ResetLengthUnits must still clear LengthUnits"
        );
        assert_eq!(
            line.units_convert, 1.0,
            "{prop_name}: ResetLengthUnits must still reset FUnitsConvert"
        );
        assert_eq!(
            line.user_length_units,
            LineUnits::Kft,
            "{prop_name}: FUserLengthUnits survives an impedance override on \
             both oracles (r4133 Line.pas:2330, dss_capi 0.14.5 Line.pas:2084)"
        );
    }
}
