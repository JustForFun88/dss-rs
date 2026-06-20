//! WP7.1 step 3a — the `geometry=` Carson path. These drive a `Line` through
//! the property engine, attach a `LineGeometry`, and assert the resulting
//! `Z`/`Yc`/`YPrim` against the **same dss-python oracle reference**
//! (`deri_full_3cond`) the line-constants and LineGeometry matrix unit tests
//! pin — proving `FetchGeometryCode` + `FMakeZFromGeometry` forward
//! `f`/`len`/`units`/`earth_model` and embed the total matrices correctly.

use super::*;
use crate::elements::general::conductor_data::{WireDataObj, wire_data};
use crate::elements::general::line_geometry::{self, LineGeometryObj};
use crate::elements::traits::{CktElement, ElemRef, SysCtx};
use crate::obj::base::DssObject;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropEngine};
use crate::solution::SolveMode;
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
    let mut errors = Vec::new();
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
    assert!(errors.is_empty(), "edit {name}={value}: {errors:?}");
}

/// Mirror the executive's `edit_property` for a single (resolved) object
/// reference: set the reference, record the set order, run side effects.
fn set_ref(cls: &ClassProps, obj: &mut dyn DssObject, name: &str, target: &dyn DssObject) {
    let idx = cls.property_index(name).expect("known property");
    let r = ElemRef { cls: 0, idx: 0 };
    obj.set_object_ref(idx, target.data().name().to_string(), Some((r, target)));
    obj.data_mut().set_as_next_seq(idx);
    obj.side_effects(idx, 0);
}

fn test_sys() -> SysCtx {
    SysCtx {
        frequency: 60.0,
        fundamental: 60.0,
        is_harmonic_model: false,
        is_dynamic_model: false,
        load_model: 1,
        mode: SolveMode::Snapshot,
        load_multiplier: 1.0,
        gen_multiplier: 1.0,
        generator_dispatch_reference: 0.0,
        price_signal: 25.0,
        default_growth_factor: 1.0,
        year: 0,
        dbl_hour: 0.0,
        solution_count: 0,
        loads_need_updating: false,
        neglect_load_y: false,
        long_line_correction: false,
        positive_sequence: false,
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
        set_ref(&gcls, &mut g, "wire", &w);
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
    set_ref(&lcls, &mut line, "geometry", &geom);

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
    set_ref(&lcls, &mut line, "geometry", &geom);
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
    set_ref(&lcls, &mut line, "geometry", &geom);
    assert!(line.geometry_obj.is_some());

    scalar(&lcls, &mut line, "r1", "0.1");
    assert!(line.geometry_obj.is_none());
    assert_eq!(line.geometry_name, "");
    assert!(line.sym_components_model);
    assert!(line.sym_components_changed);
    assert_eq!(line.fz_frequency, -1.0);
}
