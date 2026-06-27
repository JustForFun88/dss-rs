use super::*;

use crate::elements::general::load_shape::{self, LoadShapeObj};
use crate::elements::traits::SysCtx;
use crate::obj::base::DssObject;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::PropEngine;
use crate::solution::SolveMode;
use crate::support::cmatrix::CMatrix;
use dss_parser::{Parser, ParserVars};

/// Build a populated `LoadShapeObj` through its real property engine (same
/// harness as `load_shape`'s own tests).
fn build_shape(edits: &[(&str, &str)]) -> LoadShapeObj {
    let enums = EnumRegistry::new();
    let cls = load_shape::class_props(&enums);
    let mut obj = LoadShapeObj::new("d");
    let mut parser = Parser::new();
    let vars = ParserVars::new();
    let mut errors = Vec::new();
    for (name, value) in edits {
        let idx = cls.property_index(name).expect("known property");
        let mut eng = PropEngine {
            parser: &mut parser,
            vars: &vars,
            enums: &enums,
            errors: &mut errors,
            foreign: None,
        };
        cls.edit_property(&mut obj, idx, value, &mut eng).unwrap();
    }
    obj.end_edit();
    assert!(errors.is_empty(), "{errors:?}");
    obj
}

fn mode_ctx(mode: SolveMode, dbl_hour: f64) -> SysCtx {
    SysCtx {
        mode,
        dbl_hour,
        ..default_recalc_ctx()
    }
}

/// A 100 kW / pf 0.9 three-phase load (the probed oracle scenario).
fn load_100kw_pf09() -> Load {
    let mut load = Load::new("lb");
    load.kw_base = 100.0;
    load.pf_nominal = 0.9;
    load.pf_specified = true;
    load.load_spec_type = LoadSpec::KwPf;
    load.recalc(&default_recalc_ctx()); // derives kvar_base from kW/pf
    load
}

#[test]
fn daily_mode_shape_factor_scales_nominal() {
    // Oracle (dss-python 0.15.7): kw=100 pf=0.9 3-phase, daily shape
    // mult=(0.2 0.6 1.0 0.5) interval=1, mode=daily — per-conductor power
    //   hr 1 → 0.2: P=6.666667 kW, Q=3.228814 kvar
    //   hr 3 → 1.0: P=33.333333,   Q=16.144070
    let shape = build_shape(&[
        ("npts", "4"),
        ("interval", "1"),
        ("mult", "0.2 0.6 1.0 0.5"),
    ]);
    let mut load = load_100kw_pf09();
    load.daily_shape_obj = Some(shape);

    load.set_nominal_load(&mode_ctx(SolveMode::Daily, 1.0));
    assert!(
        (load.w_nominal - 6666.6667).abs() < 1e-2,
        "w {}",
        load.w_nominal
    );
    assert!(
        (load.var_nominal - 3228.814).abs() < 1e-2,
        "var {}",
        load.var_nominal
    );

    load.set_nominal_load(&mode_ctx(SolveMode::Daily, 3.0));
    assert!(
        (load.w_nominal - 33333.333).abs() < 1e-2,
        "w {}",
        load.w_nominal
    );
    assert!(
        (load.var_nominal - 16144.070).abs() < 1e-2,
        "var {}",
        load.var_nominal
    );
}

#[test]
fn yearly_mode_matches_daily_shape_lookup() {
    // Oracle: identical to the daily case when the same curve is `yearly`.
    let shape = build_shape(&[
        ("npts", "4"),
        ("interval", "1"),
        ("mult", "0.2 0.6 1.0 0.5"),
    ]);
    let mut load = load_100kw_pf09();
    load.yearly_shape_obj = Some(shape);
    load.set_nominal_load(&mode_ctx(SolveMode::Yearly, 2.0)); // mult 0.6
    assert!(
        (load.w_nominal - 20000.0).abs() < 1e-2,
        "w {}",
        load.w_nominal
    );
}

#[test]
fn duty_mode_falls_back_to_daily_shape() {
    // No duty shape → CalcDutyMult defers to the daily shape.
    let shape = build_shape(&[
        ("npts", "4"),
        ("interval", "1"),
        ("mult", "0.2 0.6 1.0 0.5"),
    ]);
    let mut load = load_100kw_pf09();
    load.daily_shape_obj = Some(shape);
    load.set_nominal_load(&mode_ctx(SolveMode::DutyCycle, 4.0)); // mult 0.5
    assert!(
        (load.w_nominal - 16666.667).abs() < 1e-2,
        "w {}",
        load.w_nominal
    );
}

#[test]
fn snapshot_mode_ignores_shape() {
    // ShapeFactor stays (1,1) in snapshot mode even with a daily shape.
    let shape = build_shape(&[
        ("npts", "4"),
        ("interval", "1"),
        ("mult", "0.2 0.6 1.0 0.5"),
    ]);
    let mut load = load_100kw_pf09();
    load.daily_shape_obj = Some(shape);
    load.set_nominal_load(&mode_ctx(SolveMode::Snapshot, 1.0));
    assert!(
        (load.w_nominal - 33333.333).abs() < 1e-2,
        "w {}",
        load.w_nominal
    );
}

/// The Load's YPrim in harmonics mode is **not** the naive frequency-scaled
/// `Yeq`: it is the `%SeriesRL` split (a parallel R-L part with `Y.im /= h` and a
/// series R-L part with `Z.im *= h`). This pins that split entry-by-entry and,
/// as a discriminator, asserts it differs substantially from the naive path — a
/// regression to `Yeq; Y.im /= h` (the pre-WP7.6 placeholder) fails here. A
/// network solve dilutes the ~40% YPrim error down to ~0.1% on the node
/// voltages, so the offline golden caught it but only barely; this unit test
/// pins the admittance itself.
#[test]
fn harmonic_yprim_uses_series_rl_split_not_naive_yeq() {
    let mut load = load_100kw_pf09();
    load.kv_load_base = 12.47;
    // Establish `Yeq` from the fundamental snapshot, like the real solve does
    // before entering harmonics mode.
    load.set_nominal_load(&mode_ctx(SolveMode::Snapshot, 0.0));
    let yeq = load.yeq;
    assert!(yeq.norm() > 0.0, "Yeq not established");

    let h = 5.0_f64;
    let freq_mult = h; // base frequency 60 → 300 Hz
    let sys = SysCtx {
        frequency: 60.0 * h,
        fundamental: 60.0,
        is_harmonic_model: true,
        ..default_recalc_ctx()
    };

    let mut ym = CMatrix::new(load.cd.yorder);
    load.calc_yprim_matrix(&mut ym, &sys);
    let actual = ym.get(0, 0); // wye phase-A diagonal = the load admittance Y

    // Expected: parallel R-L (1 - %SeriesRL of Yeq, im/h) + series R-L
    // (Z = inv(%SeriesRL·Yeq), im·h, re-inverted). %SeriesRL = 0.5, puXharm = 0.
    let mut y_par = yeq * (1.0 - load.pu_series_rl);
    y_par.im /= freq_mult;
    let mut z_ser = (yeq * load.pu_series_rl).inv();
    z_ser.im *= freq_mult;
    let expected = z_ser.inv() + y_par;
    assert!(
        (actual - expected).norm() < 1e-9,
        "harmonic YPrim {actual} != series-RL split {expected}"
    );

    // Discriminator: the naive `Yeq; Y.im /= h` path (the bug) is far off.
    let mut naive = yeq;
    naive.im /= freq_mult;
    assert!(
        (actual - naive).norm() > 0.1 * naive.norm(),
        "harmonic YPrim {actual} is indistinguishable from the naive Yeq path {naive}"
    );
}

#[test]
fn use_actual_daily_sets_kw_kvar_and_seeds_yearly() {
    // Oracle: a UseActual daily shape sets kW/kvar to (MaxP, coincident MaxQ)
    // = (80, 20); the unset yearly shape is seeded with the daily one.
    let shape = build_shape(&[
        ("npts", "3"),
        ("interval", "1"),
        ("mult", "50 80 30"),
        ("qmult", "10 20 5"),
        ("useactual", "yes"),
    ]);
    let mut load = Load::new("lc");
    load.pf_specified = false;
    load.daily_shape_obj = Some(shape);
    load.side_effects(prop::DAILY, 0);
    assert!((load.kw_base - 80.0).abs() < 1e-9, "kw {}", load.kw_base);
    assert!(
        (load.kvar_base - 20.0).abs() < 1e-9,
        "kvar {}",
        load.kvar_base
    );
    assert_eq!(load.load_spec_type, LoadSpec::KwKvar);
    assert!(load.yearly_shape_obj.is_some(), "yearly seeded from daily");
}
