use super::*;

use crate::elements::general::load_shape::{self, LoadShapeObj};
use crate::elements::traits::SysCtx;
use crate::obj::base::DssObject;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::PropEngine;
use crate::solution::{RandomType, SolveMode, USEDUTY, USENONE, USEYEARLY};
use crate::support::cmatrix::CMatrix;
use crate::support::mathutil::FpcRng;
use dss_parser::{Parser, ParserVars};

/// Build a populated `LoadShapeObj` through its real property engine (same
/// harness as `load_shape`'s own tests).
fn build_shape(edits: &[(&str, &str)]) -> LoadShapeObj {
    let enums = EnumRegistry::new();
    let cls = load_shape::class_props(&enums);
    let mut obj = LoadShapeObj::new("d");
    let mut parser = Parser::new();
    let vars = ParserVars::new();
    let mut errors = crate::diag::ErrorLog::new();
    for (name, value) in edits {
        let idx = cls.property_index(name).expect("known property");
        let mut eng = PropEngine {
            parser: &mut parser,
            vars: &vars,
            enums: &enums,
            errors: &mut errors,
            foreign: None,

            was_quoted: false,
        };
        cls.edit_property(&mut obj, idx, value, &mut eng).unwrap();
    }
    obj.end_edit(&crate::elements::traits::SysCtx::parse_default());
    assert!(errors.is_empty(), "{errors:?}");
    obj
}

/// Build a populated `GrowthShapeObj` through its real property engine.
fn build_growth_shape(
    edits: &[(&str, &str)],
) -> crate::elements::general::growth_shape::GrowthShapeObj {
    use crate::elements::general::growth_shape::{self, GrowthShapeObj};
    let enums = EnumRegistry::new();
    let cls = growth_shape::class_props();
    let mut obj = GrowthShapeObj::new("g");
    let mut parser = Parser::new();
    let vars = ParserVars::new();
    let mut errors = crate::diag::ErrorLog::new();
    for (name, value) in edits {
        let idx = cls.property_index(name).expect("known property");
        let mut eng = PropEngine {
            parser: &mut parser,
            vars: &vars,
            enums: &enums,
            errors: &mut errors,
            foreign: None,

            was_quoted: false,
        };
        cls.edit_property(&mut obj, idx, value, &mut eng).unwrap();
    }
    obj.end_edit(&crate::elements::traits::SysCtx::parse_default());
    assert!(errors.is_empty(), "{errors:?}");
    obj
}

/// B3-r3723: `Load.GrowthFactor` at Year=0 with a GrowthShape now tracks the
/// simulated hours (`calcYear = dblHour/8760`) instead of a flat 1.0 — so a
/// long (>8760 h) Year=0 run advances through the growth curve.
///
/// Oracle-validated against capi015 (backend 0.15.0b4 = SVN r4103, which carries
/// the r4088-era `GrowthFactor` rewrite; `/tmp/probe_b3*.py`, 2026-07-16;
/// growthshape `year=(0,1,2) mult=(1.2,1.5,2.0)`, a 100 kW pf 0.9 3-phase load):
///
/// | run (year=0)             | capi015 total kW | pre-B3 (0.14.5) |
/// |--------------------------|------------------|-----------------|
/// | snapshot, any hour       | 120 (factor 1.2) | 100 (flat 1.0)  |
/// | daily, dblHour≈8760      | 180 (GetMult 2)  | 100             |
///
/// The snapshot 120-vs-100 split is the deck-gated witness
/// (`modes/upgrade/upgrade_growth_year0.dss`, `oracle: "capi015"`); the exact
/// per-branch values below match the same oracle (`GetMultIdx(1)=1.2`,
/// `GetMult(2)=1.8`).
#[test]
fn growth_factor_year0_tracks_simulated_hours_with_growthshape() {
    let gs = build_growth_shape(&[("npts", "3"), ("year", "0 1 2"), ("mult", "1.2 1.5 2.0")]);
    let mut load = load_100kw_pf09(); // 33.333 kW/phase nominal
    load.growth_shape_obj = Some(gs);
    // Year 0, dblHour 100 → calcYear ≈ 0.011 < 1 AND firstY == 0 ⇒
    // factor = GetMultIdx(1) = 1.2 (pre-B3 this was a flat 1.0). capi015 probe:
    // snapshot year=0 hour=100 → 120 kW total = 40 kW/phase.
    load.set_nominal_load(&mode_ctx(SolveMode::Snapshot, 100.0));
    assert!(
        (load.w_nominal - 40000.0).abs() < 1.0,
        "w {}",
        load.w_nominal
    );
    // Year 0, dblHour 17520 → calcYear = 2.0 ≥ 1 ⇒ factor = GetMult(2) = 1.8.
    load.set_nominal_load(&mode_ctx(SolveMode::Snapshot, 17520.0));
    assert!(
        (load.w_nominal - 60000.0).abs() < 1.0,
        "w {}",
        load.w_nominal
    );
    // A NIL growthshape keeps the flat Year=0 factor of 1.0 (unchanged path).
    let mut plain = load_100kw_pf09();
    plain.set_nominal_load(&mode_ctx(SolveMode::Snapshot, 17520.0));
    assert!(
        (plain.w_nominal - 33333.333).abs() < 1.0,
        "w {}",
        plain.w_nominal
    );
}

fn mode_ctx(mode: SolveMode, dbl_hour: f64) -> SysCtx {
    SysCtx {
        mode,
        dbl_hour,
        ..default_recalc_ctx()
    }
}

/// `mode=Time` (GENERALTIME) with an explicit `ActiveLoadShapeClass`
/// (`Set LoadShapeClass=`).
fn time_class_ctx(class: i32, dbl_hour: f64) -> SysCtx {
    SysCtx {
        mode: SolveMode::Time,
        active_load_shape_class: class,
        dbl_hour,
        ..default_recalc_ctx()
    }
}

/// A load carrying three DELIBERATELY-DISTINCT shapes so the GENERALTIME
/// class-dispatch arm that fired is identifiable from `w_nominal` alone
/// (at hr 2: daily→0.6, yearly→0.7, duty→0.5).
fn load_with_three_shapes() -> Load {
    let mut load = load_100kw_pf09();
    load.daily_shape_obj = Some(build_shape(&[
        ("npts", "4"),
        ("interval", "1"),
        ("mult", "0.2 0.6 1.0 0.5"),
    ]));
    load.yearly_shape_obj = Some(build_shape(&[
        ("npts", "4"),
        ("interval", "1"),
        ("mult", "0.3 0.7 0.9 0.4"),
    ]));
    load.duty_shape_obj = Some(build_shape(&[
        ("npts", "4"),
        ("interval", "1"),
        ("mult", "0.1 0.5 0.8 0.6"),
    ]));
    load
}

/// Edit a fresh `Load` through its real property engine (string-edit path, so
/// the `PropFlags::REPLACE_ZERO` clamp in `set_obj_double` runs).
fn edit_load(edits: &[(&str, &str)]) -> Load {
    let enums = EnumRegistry::new();
    let cls = super::class_props(&enums);
    let mut load = Load::new("lz");
    let mut parser = Parser::new();
    let vars = ParserVars::new();
    let mut errors = crate::diag::ErrorLog::new();
    for (name, value) in edits {
        let idx = cls.property_index(name).expect("known property");
        let mut eng = PropEngine {
            parser: &mut parser,
            vars: &vars,
            enums: &enums,
            errors: &mut errors,
            foreign: None,

            was_quoted: false,
        };
        cls.edit_property(&mut load, idx, value, &mut eng).unwrap();
    }
    assert!(errors.is_empty(), "{errors:?}");
    load
}

/// UPGRADE_PLAN ledger L2 (WP-U1.1): `kW`/`kVA` parsed as 0 clamp to `1e-8`
/// (EPRI r4133 `DblValueNZ`), never kept as 0 and never an error. The whole
/// open band `(-1e-8, 1e-8)` maps to `+1e-8`; out-of-band values are untouched.
#[test]
fn zero_kw_kva_clamp_dblvaluenz() {
    let load = edit_load(&[("kw", "0"), ("kva", "0")]);
    assert_eq!(
        load.kw_base.to_bits(),
        1e-8f64.to_bits(),
        "kw {}",
        load.kw_base
    );
    assert_eq!(
        load.kva_base.to_bits(),
        1e-8f64.to_bits(),
        "kva {}",
        load.kva_base
    );
    // Tiny in-band (incl. negative) also clamps to +1e-8.
    assert_eq!(
        edit_load(&[("kw", "5e-9")]).kw_base.to_bits(),
        1e-8f64.to_bits()
    );
    assert_eq!(
        edit_load(&[("kw", "-3e-9")]).kw_base.to_bits(),
        1e-8f64.to_bits()
    );
    // Out-of-band values are unaffected; `kvar` (no flag) keeps a literal 0.
    let ld = edit_load(&[("kw", "100"), ("kvar", "0")]);
    assert_eq!(ld.kw_base, 100.0);
    assert_eq!(ld.kvar_base, 0.0);
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

/// Pascal `SetNominalLoad` GENERALTIME arm (Load.pas:1066): `Set
/// LoadShapeClass=Yearly` (USEYEARLY) drives the load from the YEARLY curve.
/// The three distinct curves make a copy-paste swap (`USEYEARLY =>
/// CalcDailyMult`/`CalcDutyMult`) fail: at hr 2 the yearly mult is 0.7 →
/// w = 1000·100·0.7/3 (daily 0.6 → 20000, duty 0.5 → 16666 would mismatch).
#[test]
fn time_loadshapeclass_yearly_uses_yearly_curve() {
    let mut load = load_with_three_shapes();
    load.set_nominal_load(&time_class_ctx(USEYEARLY, 2.0));
    assert!(
        (load.w_nominal - 23333.333).abs() < 1e-2,
        "w {} (expected the yearly 0.7 curve, not daily/duty)",
        load.w_nominal
    );
}

/// GENERALTIME arm (Load.pas:1068): `Set LoadShapeClass=Duty` (USEDUTY) drives
/// the load from the DUTY curve (hr 2 → 0.5 → w = 1000·100·0.5/3).
#[test]
fn time_loadshapeclass_duty_uses_duty_curve() {
    let mut load = load_with_three_shapes();
    load.set_nominal_load(&time_class_ctx(USEDUTY, 2.0));
    assert!(
        (load.w_nominal - 16666.667).abs() < 1e-2,
        "w {} (expected the duty 0.5 curve, not daily/yearly)",
        load.w_nominal
    );
}

/// GENERALTIME arm `else` (Load.pas:1071): the DEFAULT class `USENONE` leaves
/// `ShapeFactor = 1+j1`, so even a fully-shaped load stays at nominal in
/// `mode=Time` until `Set LoadShapeClass=` selects a class. (Re-pins the fact
/// the original degenerate deck accidentally covered before it forced =Daily.)
#[test]
fn time_loadshapeclass_none_stays_at_nominal() {
    let mut load = load_with_three_shapes();
    load.set_nominal_load(&time_class_ctx(USENONE, 2.0));
    // Nominal: w = 1000·100·1/3 = 33333.33 (no curve applied).
    assert!(
        (load.w_nominal - 33333.333).abs() < 1e-2,
        "w {} (USENONE must ignore all three shapes)",
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

// --- TLoadObj.Randomize (the MonteCarlo1 per-load RandomMult draw) ---
//
// Fixed-seed coverage of the RNG-driven arms the gating decks never reach
// (every Monte deck runs `random=none`). Expected values are derived
// externally from the seed-12345 draw sequence pinned in
// `support::mathutil::rng` — the canonical MT19937 stream verified there
// against `mt19937ar.out` / CPython — never captured from `randomize` itself
// (GAPS_PLAN.md §2.1). `Gauss(0,1) = Σ12 Random − 6.0` exactly, so
// `Gauss(m,s) = G01·s + m` and `QuasiLognormal(m) = exp(G01)·m` bit-for-bit.

/// First `next_f64()` draw for seed 12345 (`rng.rs`).
const RND_D0_BITS: u64 = 0x3fedbf6a3c400000;
/// First `Gauss(0,1)` result for seed 12345 (`rng.rs`).
const RND_G01_0_BITS: u64 = 0x3fc62af569800000;

#[test]
fn randomize_uniform_draws_next_f64() {
    let mut load = Load::new("lr");
    let mut rng = FpcRng::from_seed(12345);
    load.randomize(RandomType::Uniform, &mut rng);
    assert_eq!(load.random_mult.to_bits(), RND_D0_BITS);
}

#[test]
fn randomize_gaussian_no_yearly_uses_pu_mean_std() {
    let mut load = Load::new("lr");
    load.yearly_shape_obj = None;
    load.pu_mean = 0.8;
    load.pu_std_dev = 0.3;
    let mut rng = FpcRng::from_seed(12345);
    load.randomize(RandomType::Gaussian, &mut rng);
    let expected = f64::from_bits(RND_G01_0_BITS) * 0.3 + 0.8;
    assert_eq!(load.random_mult, expected);
}

#[test]
fn randomize_gaussian_with_yearly_uses_shape_mean_std() {
    // The yearly shape's mean/std-dev override pu_mean/pu_std_dev; the pu_*
    // fields are set DIFFERENTLY so the test fails if the fallback is taken.
    let mut load = Load::new("lr");
    load.pu_mean = 0.8;
    load.pu_std_dev = 0.3;
    load.yearly_shape_obj = Some(build_shape(&[("mean", "0.75"), ("stddev", "0.20")]));
    let mut rng = FpcRng::from_seed(12345);
    load.randomize(RandomType::Gaussian, &mut rng);
    let expected = f64::from_bits(RND_G01_0_BITS) * 0.20 + 0.75;
    assert_eq!(load.random_mult, expected);
}

// EXPECTED-VALUE-PIN(stddev_single_point): the second consumer of the torn-down
// value — the one that turns it into physics rather than into a printed field.
/// A one-point yearly shape randomizes to a **constant**: `Gauss(m, s)` is
/// `G01·s + m`, so at `s = 0` `RandomMult` is the multiplier itself whatever
/// the RNG draws.
///
/// Upstream's `StdDev := Data^[1];` (r4133
/// `Version8/Source/Shared/mathutil.pas:405`) makes the same shape draw
/// `G01·0.4 + 0.4` — a ±100 %-of-mean spread on a sample that has none, with a
/// tail of *negative* multipliers turning the load into a source. Not
/// reproduced in either lane (GOLDEN_REBASE G2.1a; `issue-11`), and pinned here
/// because no gated deck reaches this path: every Monte deck runs
/// `random=none`.
#[test]
fn randomize_gaussian_with_single_point_yearly_is_constant() {
    let mut load = Load::new("lr");
    // Set DIFFERENTLY from the shape, so the no-shape fallback fails the test.
    load.pu_mean = 0.8;
    load.pu_std_dev = 0.3;
    load.yearly_shape_obj = Some(build_shape(&[
        ("npts", "1"),
        ("interval", "1"),
        ("mult", "(0.4)"),
    ]));
    let mut rng = FpcRng::from_seed(12345);
    load.randomize(RandomType::Gaussian, &mut rng);
    assert_eq!(load.random_mult, 0.4);
}

#[test]
fn randomize_lognormal_no_yearly_uses_pu_mean() {
    let mut load = Load::new("lr");
    load.yearly_shape_obj = None;
    load.pu_mean = 2.0;
    let mut rng = FpcRng::from_seed(12345);
    load.randomize(RandomType::LogNormal, &mut rng);
    let expected = f64::from_bits(RND_G01_0_BITS).exp() * 2.0;
    assert_eq!(load.random_mult, expected);
}

#[test]
fn randomize_lognormal_with_yearly_uses_shape_mean() {
    let mut load = Load::new("lr");
    load.pu_mean = 2.0; // different from the shape mean below
    load.yearly_shape_obj = Some(build_shape(&[("mean", "3.0"), ("stddev", "0.20")]));
    let mut rng = FpcRng::from_seed(12345);
    load.randomize(RandomType::LogNormal, &mut rng);
    let expected = f64::from_bits(RND_G01_0_BITS).exp() * 3.0;
    assert_eq!(load.random_mult, expected);
}

#[test]
fn randomize_none_sets_one_and_draws_nothing() {
    let mut load = Load::new("lr");
    let mut rng = FpcRng::from_seed(12345);
    load.randomize(RandomType::None, &mut rng);
    assert_eq!(load.random_mult, 1.0);
    assert_eq!(
        rng.next_f64().to_bits(),
        RND_D0_BITS,
        "random=none must not consume a draw"
    );
}

// --- MakePosSequence (WPG.21) --------------------------------------------

use crate::elements::pos_seq::{PosSeqAction, PosSeqCtx};
use crate::elements::traits::CktElement;

/// Wye 3-phase load: V line-neutral (kV/√3), kW/kvar ÷ 3, no xfkVA (0).
#[test]
fn makeposseq_wye_three_phase() {
    let mut ld = Load::new("l");
    ld.connection = Connection::Wye;
    ld.cd.nphases = 3;
    ld.kv_load_base = 12.47;
    ld.kw_base = 400.0;
    ld.kvar_base = 131.55;
    ld.connected_kva = 0.0;

    let plan = ld.make_pos_sequence(&PosSeqCtx::default());
    assert!(plan.run_base);
    let v = 12.47 / 3.0_f64.sqrt();
    assert_eq!(
        plan.actions,
        vec![
            PosSeqAction::BeginEdit,
            PosSeqAction::SetI32(prop::PHASES, 1),
            PosSeqAction::SetI32(prop::CONN, 0),
            PosSeqAction::SetF64(prop::KV, v),
            PosSeqAction::SetF64(prop::KW, 400.0 / 3.0),
            PosSeqAction::SetF64(prop::KVAR, 131.55 / 3.0),
            PosSeqAction::EndEdit,
        ]
    );
    // Oracle cross-check: ld_wye kW 400 → 133.33 (and 44.44 after a 2nd pass).
    assert!((400.0_f64 / 3.0 - 133.3333).abs() < 1e-3);
}

/// Delta load with xfkVA>0: V line-neutral (Δ ⇒ conn≠Wye), and the xfkVA
/// (ConnectedKVA) ÷ 3 emitted as a 7th action.
#[test]
fn makeposseq_delta_with_xfkva() {
    let mut ld = Load::new("l");
    ld.connection = Connection::Delta;
    ld.cd.nphases = 3;
    ld.kv_load_base = 12.47;
    ld.kw_base = 300.0;
    ld.kvar_base = 100.0;
    ld.connected_kva = 500.0;

    let plan = ld.make_pos_sequence(&PosSeqCtx::default());
    let v = 12.47 / 3.0_f64.sqrt();
    assert_eq!(
        plan.actions,
        vec![
            PosSeqAction::BeginEdit,
            PosSeqAction::SetI32(prop::PHASES, 1),
            PosSeqAction::SetI32(prop::CONN, 0),
            PosSeqAction::SetF64(prop::KV, v),
            PosSeqAction::SetF64(prop::KW, 300.0 / 3.0),
            PosSeqAction::SetF64(prop::KVAR, 100.0 / 3.0),
            PosSeqAction::SetF64(prop::XFKVA, 500.0 / 3.0),
            PosSeqAction::EndEdit,
        ]
    );
}

/// 1-phase wye load: V stays line-line base (nphases==1 AND conn==Wye), and
/// the ÷3 (not ÷nphases) power split still applies.
#[test]
fn makeposseq_single_phase_wye_keeps_base_kv() {
    let mut ld = Load::new("l");
    ld.connection = Connection::Wye;
    ld.cd.nphases = 1;
    ld.kv_load_base = 7.2;
    ld.kw_base = 80.0;
    ld.kvar_base = 26.3;
    ld.connected_kva = 0.0;

    let plan = ld.make_pos_sequence(&PosSeqCtx::default());
    assert_eq!(
        plan.actions,
        vec![
            PosSeqAction::BeginEdit,
            PosSeqAction::SetI32(prop::PHASES, 1),
            PosSeqAction::SetI32(prop::CONN, 0),
            PosSeqAction::SetF64(prop::KV, 7.2), // NOT /√3
            PosSeqAction::SetF64(prop::KW, 80.0 / 3.0),
            PosSeqAction::SetF64(prop::KVAR, 26.3 / 3.0),
            PosSeqAction::EndEdit,
        ]
    );
}

/// Pascal `TPCElement.GetCurrents` `LastSolutionWasDirect` shortcut
/// (PCElement.pas l.137): after a direct solve the reported terminal current
/// is `CalcYPrimContribution` = `YPrim · Vterminal` (the frozen
/// shadow-admittance current), NOT the model compensation current
/// `YPrim·V − InjCurrent`; a snapshot solve after the direct one (flag
/// cleared, `SolutionCount` bumped) reverts to the model current.
#[test]
fn direct_shortcut_selects_yprim_currents() {
    use crate::elements::traits::CktElement;
    use num_complex::Complex64;

    let mut load = load_100kw_pf09();
    load.kv_load_base = 12.47;
    let snap = SysCtx {
        solution_count: 1,
        ..default_recalc_ctx()
    };
    load.set_nominal_load(&snap);
    CktElement::calc_yprim(&mut load, &snap);

    // 3-phase wye: yorder 4, node refs A/B/C + grounded neutral.
    load.cd.set_node_ref(1, &[1, 2, 3, 0]);
    let vmag = 12.47e3 / 3.0_f64.sqrt();
    let a = Complex64::from_polar(1.0, -2.0 * std::f64::consts::PI / 3.0);
    let node_v = vec![
        Complex64::ZERO, // ground slot
        Complex64::new(vmag, 0.0) * 0.98,
        Complex64::new(vmag, 0.0) * a * 0.98,
        Complex64::new(vmag, 0.0) * a * a * 0.98,
    ];

    // Model (compensation) current — the normal snapshot read.
    let mut i_model = vec![Complex64::ZERO; 4];
    load.get_currents(&snap, &node_v, &mut i_model);

    // Direct read: expect exactly YPrim · Vterminal.
    let direct = SysCtx {
        solution_count: 2,
        last_solution_was_direct: true,
        ..default_recalc_ctx()
    };
    let mut i_direct = vec![Complex64::ZERO; 4];
    load.get_currents(&direct, &node_v, &mut i_direct);
    let mut expected = vec![Complex64::ZERO; 4];
    load.cd
        .yprim
        .as_ref()
        .unwrap()
        .mv_mult(&mut expected, &load.cd.vterminal);
    for k in 0..4 {
        assert!(
            (i_direct[k] - expected[k]).norm() < 1e-12,
            "direct read [{k}] {} != YPrim·V {}",
            i_direct[k],
            expected[k]
        );
    }
    // Feature sensitivity: the shortcut differs from the model current by
    // whole amps on phase A (the escalated DIRECT-mode divergence).
    assert!(
        (i_direct[0] - i_model[0]).norm() > 0.1,
        "shortcut indistinguishable from model current: {} vs {}",
        i_direct[0],
        i_model[0]
    );

    // Dynamics/harmonics exclude the shortcut even with the flag set
    // (PCElement.pas l.137's `not (IsDynamicModel or IsHarmonicModel)`) — assert
    // BOTH arms of the OR so a regression dropping either term is caught.
    assert!(
        !SysCtx {
            last_solution_was_direct: true,
            is_harmonic_model: true,
            ..default_recalc_ctx()
        }
        .pc_direct_shortcut(),
        "harmonic model must exclude the direct shortcut"
    );
    assert!(
        !SysCtx {
            last_solution_was_direct: true,
            is_dynamic_model: true,
            ..default_recalc_ctx()
        }
        .pc_direct_shortcut(),
        "dynamic model must exclude the direct shortcut"
    );

    // Snapshot after direct: flag cleared (DoPFLOWsolution l.1022), new
    // SolutionCount → model current again.
    let snap2 = SysCtx {
        solution_count: 3,
        ..default_recalc_ctx()
    };
    let mut i_after = vec![Complex64::ZERO; 4];
    load.get_currents(&snap2, &node_v, &mut i_after);
    for k in 0..4 {
        assert!(
            (i_after[k] - i_model[k]).norm() < 1e-9,
            "post-direct snapshot read [{k}] {} != model {}",
            i_after[k],
            i_model[k]
        );
    }
}

#[test]
fn load_status_pins_enum_ordinals() {
    assert_eq!(LoadStatus::Variable.ordinal(), 0);
    assert_eq!(LoadStatus::Fixed.ordinal(), 1);
    assert_eq!(LoadStatus::Exempt.ordinal(), 2);
    assert_eq!(LoadStatus::from_ordinal(0), Some(LoadStatus::Variable));
    assert_eq!(LoadStatus::from_ordinal(1), Some(LoadStatus::Fixed));
    assert_eq!(LoadStatus::from_ordinal(2), Some(LoadStatus::Exempt));
    assert_eq!(LoadStatus::from_ordinal(3), None);
}
