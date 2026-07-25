//! Unit tests for the WindGen PC element (Create defaults, props round-trip,
//! the aerodynamic power-flow path, the L2 zero-clamp, WTG3 dynamics
//! determinism/finiteness, and the disabled-harmonics abort). Live oracle
//! (capi015) coverage is in the `modes/windgen/` corpus family.

use super::wtg3::Wtg3Model;
use super::*;

use crate::exec::Dss;
use crate::obj::base::DssObject;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::PropEngine;
use crate::support::complexutil::pclx;
use dss_parser::{Parser, ParserVars};

const PI: f64 = std::f64::consts::PI;

/// Edit a fresh `WindGen` through its real property engine (string-edit path, so
/// the `PropFlags::REPLACE_ZERO` clamp in `set_obj_double` runs).
fn edit_windgen(edits: &[(&str, &str)]) -> WindGen {
    let enums = EnumRegistry::new();
    let cls = super::class_props(&enums);
    let mut g = WindGen::new("w1");
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
        };
        cls.edit_property(&mut g, idx, value, &mut eng).unwrap();
    }
    g.end_edit(&crate::elements::traits::SysCtx::parse_default());
    assert!(errors.is_empty(), "{errors:?}");
    g
}

#[test]
fn create_defaults_match_pascal() {
    // `new` no longer recalcs (no live ctx at construction); recalc explicitly
    // with the parse-time default (dyna_h=dyna_t=0), exactly as the executive does
    // at create — this re-derives kVArating and seeds the WTG3 model.
    let mut g = WindGen::new("w1");
    g.recalc(&crate::elements::traits::SysCtx::parse_default());
    assert_eq!(g.cd.nphases, 3);
    assert_eq!(g.cd.nconds, 4);
    assert_eq!(g.kw_base, 1000.0);
    assert_eq!(g.kvar_base, 60.0);
    assert_eq!(g.pf_nominal, 0.88);
    assert_eq!(g.gen_model, 1);
    assert_eq!(g.gen_class, 1);
    assert_eq!(g.kv_windgen_base, 12.47);
    // Create sets kVArating := kWBase*1.2 (1200), then RecalcElementData (called
    // at the end of Create) re-derives it from kW/PF because kVANotSet is true:
    // kVArating := kWBase/abs(PFNominal) = 1000/0.88.
    assert!((g.kva_rating - 1000.0 / 0.88).abs() < 1e-9);
    assert!(g.kva_not_set);
    // Aerodynamic defaults.
    assert_eq!(g.ag, 1.0 / 90.0);
    assert_eq!(g.cp, 0.41);
    assert_eq!(g.lamda, 7.95);
    assert_eq!(g.poles, 2.0);
    assert_eq!(g.pd, 1.225);
    assert_eq!(g.rad, 40.0);
    assert_eq!(g.v_cut_in, 5.0);
    assert_eq!(g.v_cut_out, 23.0);
    // WTG3 model: Create overrides VWind→12, QMode→0 after Initialize.
    assert_eq!(g.wind_model_dyn.vwind, 12.0);
    assert_eq!(g.wind_model_dyn.q_mode, 0);
    assert_eq!(g.wind_model_dyn.n_wtg, 1);
    assert_eq!(g.wind_model_dyn.sim_mech_flg, 1);
    assert_eq!(g.wind_model_dyn.q_flg, 1);
    assert_eq!(
        g.wind_model_dyn.zthev,
        num_complex::Complex64::new(0.0, 0.05)
    );
}

/// Props round-trip: parse a range of properties (aerodynamic, WTG3, ratings)
/// and read them back via the `?` dump. Feature-sensitive to the typed
/// getters/setters (no oracle exists at 0.14.5 — WindGen is a 0.15.x class).
#[test]
fn props_roundtrip_through_query() {
    let mut dss = Dss::new();
    for c in [
        "clear",
        "new circuit.t basekv=0.69",
        "new windgen.w1 bus1=b1 phases=3 kv=0.69 kW=1500 pf=0.95 conn=delta model=2 \
         vss=1.02 pss=0.9 qss=0.1 vwind=13 qmode=2 simmechflg=0 apcflg=1 qflg=0 \
         n_wtg=4 rthev=0.01 xthev=0.06 delt0=0.0001 ag=0.02 cp=0.44 lamda=8.1 p=4 \
         pd=1.2 rad=45 vcutin=4 vcutout=25 class=3",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "errors: {:?}", dss.errors());

    let dump = |dss: &mut Dss, p: &str| {
        dss.command(&format!("? windgen.w1.{p}"));
        dss.result().to_string()
    };
    assert_eq!(dump(&mut dss, "kW"), "1500");
    assert_eq!(dump(&mut dss, "conn"), "delta");
    assert_eq!(dump(&mut dss, "model"), "2");
    assert_eq!(dump(&mut dss, "vss"), "1.02");
    assert_eq!(dump(&mut dss, "pss"), "0.9");
    assert_eq!(dump(&mut dss, "qmode"), "2"); // VoltVar → ordinal 2 (JSONUseNumbers)
    assert_eq!(dump(&mut dss, "n_wtg"), "4");
    assert_eq!(dump(&mut dss, "rthev"), "0.01");
    assert_eq!(dump(&mut dss, "xthev"), "0.06");
    assert_eq!(dump(&mut dss, "cp"), "0.44");
    assert_eq!(dump(&mut dss, "p"), "4");
    assert_eq!(dump(&mut dss, "rad"), "45");
    assert_eq!(dump(&mut dss, "class"), "3");
}

/// The aerodynamic steady-state path: `Pm = 0.5·ρ·π·Rad²·v³·Cp`, curtailed to
/// `kWBase`, shared over the phases. Pins the formula against a hand
/// computation (feature-sensitive to the whole `SetNominalGeneration` aero path).
#[test]
fn aerodynamic_power_snapshot() {
    // kW=3000 keeps the aero power below the curtailment cap.
    let mut g = edit_windgen(&[
        ("phases", "3"),
        ("kv", "0.69"),
        ("kW", "3000"),
        ("vwind", "12"),
    ]);
    let sys = default_recalc_ctx();
    g.set_nominal_generation(&sys, &[]);

    let vwind = 12.0_f64;
    let pm = 0.5 * 1.225 * PI * 40.0_f64.powi(2) * vwind.powi(3) * 0.41;
    let pg = pm / 1e3; // kW, no losses
    assert!(pg < 3000.0, "expected uncapped ({pg} kW)");
    assert!((g.pm - pm).abs() < 1e-6);
    assert!((g.pg - pg).abs() < 1e-9);
    // p_nominal_per_phase = 1e3 * Pg / nphases.
    assert!((g.p_nominal_per_phase - 1e3 * pg / 3.0).abs() < 1e-6);

    // Above cut-out → zero aerodynamic power (Pnom = 0.001·kWBase).
    let mut g2 = edit_windgen(&[("kv", "0.69"), ("kW", "3000"), ("vwind", "30")]);
    g2.set_nominal_generation(&sys, &[]);
    assert_eq!(g2.pm, 0.0);
    assert_eq!(g2.pg, 0.0);
    assert!((g2.p_nominal_per_phase - 0.001 * 3000.0).abs() < 1e-9);
}

/// L2 (DIVERGENCES.md): WindGen `kW`/`kVA`/`MVA` parsed as `0` clamp to `1e-8`
/// (EPRI `DblValueNZ`), unlike Generator's `MVA` which does NOT clamp. Fails if
/// the `REPLACE_ZERO` flag is dropped.
#[test]
fn zero_kw_kva_mva_clamp_dblvaluenz() {
    let g = edit_windgen(&[("kv", "0.69"), ("kW", "0")]);
    assert_eq!(g.kw_base, 1e-8, "kW=0 must clamp to 1e-8");

    let g = edit_windgen(&[("kv", "0.69"), ("kVA", "0")]);
    assert_eq!(g.kva_rating, 1e-8, "kVA=0 must clamp to 1e-8");

    // MVA = DblValueNZ * 1000 → 0 clamps to 1e-8 (scaled: 1e-8*1000 = 1e-5).
    let g = edit_windgen(&[("kv", "0.69"), ("MVA", "0")]);
    assert_eq!(g.kva_rating, 1e-5, "MVA=0 must clamp (1e-8 * 1000)");
}

/// The 22 classic state-variable names (from the Pascal `TWindGenVariable`
/// enum), in order.
#[test]
fn variable_names_match_enum() {
    let g = WindGen::new("w1");
    assert_eq!(g.num_wgen_variables(), 22);
    let expected = [
        "userTrip",
        "wtgTrip",
        "Pcurtail",
        "Pcmd",
        "Pgen",
        "Qcmd",
        "Qgen",
        "Vref",
        "Vmag",
        "vwind",
        "WtRef",
        "WtAct",
        "dOmg",
        "dFrqPuTest",
        "QMode",
        "Qref",
        "PFref",
        "thetaPitch",
        "Pg",
        "Ps",
        "Pr",
        "s",
    ];
    for (i, name) in expected.iter().enumerate() {
        assert_eq!(&g.wgen_variable_name(i + 1), name, "variable {}", i + 1);
    }
}

/// A full snapshot solve with a WindGen converges cleanly and leaves the engine
/// error-free (the aerodynamic negative-load injection). Numbers are gated live
/// against capi015; this pins parse+solve+no-crash.
#[test]
fn snapshot_solve_converges() {
    let mut dss = Dss::new();
    for c in [
        "clear",
        "new circuit.wtg basekv=12.47 phases=3 bus1=srcbus",
        "new line.l1 bus1=srcbus bus2=wbus phases=3 r1=0.1 x1=0.5 length=1",
        "new windgen.w1 bus1=wbus phases=3 kv=12.47 kW=1500 pf=1.0 conn=wye model=1 vwind=12",
        "solve",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "errors: {:?}", dss.errors());
    let ckt = dss.circuit().expect("circuit");
    assert!(ckt.solution.converged_flag, "snapshot must converge");
}

/// A full dynamics solve (WTG3 sub-cycle integrator) runs without producing
/// NaNs or aborting. The WindGen sits at 0.69 kV so the per-unit measurement in
/// the WTG3 model is not saturated.
#[test]
fn dynamics_solve_no_nan() {
    let mut dss = Dss::new();
    for c in [
        "clear",
        "new circuit.wtg basekv=0.69 phases=3 bus1=srcbus",
        "new line.l1 bus1=srcbus bus2=wbus phases=3 r1=0.005 x1=0.02 length=1",
        "new windgen.w1 bus1=wbus phases=3 kv=0.69 kW=1500 kva=1800 conn=wye model=1 \
         vss=1 pss=1 qss=0 vwind=12",
        "solve",
        "set mode=dynamic stepsize=0.001 number=20",
        "solve",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "errors: {:?}", dss.errors());
    let ckt = dss.circuit().expect("circuit");
    assert!(ckt.solution.converged_flag, "dynamics steps must converge");
}

/// A 1-phase WindGen entering dynamics aborts cleanly (loud error, no panic).
/// The embedded WTG3 model is 3-phase-only — `Instrumentation`/`CalcDynamic`
/// read V[1..3]/i[1..3], so a 2-conductor terminal over-reads = heap UB
/// upstream (WindGen.pas:1868/1905 accept 1-phase then call `WindModelDyn.Init`
/// on a 2-element `Vterminal`). Per CLAUDE.md that UB is NOT reproduced: the
/// port aborts at `InitStateVars` instead of panicking on the OOB index.
#[test]
fn single_phase_dynamics_aborts_cleanly() {
    let mut dss = Dss::new();
    for c in [
        "clear",
        "new circuit.wtg basekv=0.69 phases=3 bus1=srcbus",
        "new line.l1 bus1=srcbus bus2=wbus phases=3 r1=0.005 x1=0.02 length=1",
        "new windgen.w1 bus1=wbus.1 phases=1 kv=0.4 kW=200 kva=300 conn=wye model=1 \
         vss=1 pss=1 qss=0 vwind=12",
        "solve",
        "set mode=dynamic stepsize=0.001 number=1",
        "solve",
    ] {
        dss.command(c);
    }
    assert!(
        dss.errors()
            .iter()
            .any(|e| e.contains("Dynamics mode requires a 3-phase WindGen")),
        "expected the 3-phase-only dynamics abort, got {:?}",
        dss.errors()
    );
}

/// Harmonics mode is disabled upstream in 0.15.x: entering it with a WindGen
/// raises the loud abort (`InitHarmonics` / `DoHarmonicMode`), reproduced 1:1.
#[test]
fn harmonics_disabled_aborts_loudly() {
    let mut g = WindGen::new("w1");
    g.init_harmonics_impl();
    let errs = g.cd.obj.take_errors();
    assert!(
        errs.iter()
            .any(|e| e.contains("harmonics model is not fully implemented")),
        "expected the disabled-harmonics abort, got {errs:?}"
    );
    assert!(
        g.cd.obj.take_abort(),
        "harmonics disable must request a solution abort"
    );
}

/// The WTG3 model is a pure deterministic f64 pipeline: two identical
/// init+step sequences produce bit-identical state, and the injection current
/// stays finite through the sub-cycle integrator.
#[test]
fn wtg3_dynamics_deterministic_and_finite() {
    fn run() -> ([num_complex::Complex64; 3], f64, f64) {
        let mut m = Wtg3Model::new();
        m.recalc_element_data(0.001, 0.0);
        // Balanced 1-pu terminal voltage at the WTG rated L-N.
        let vln = m.rated_vln;
        let v = [
            pclx(vln, 0.0),
            pclx(vln, -2.0 * PI / 3.0),
            pclx(vln, 2.0 * PI / 3.0),
        ];
        let mut i = [num_complex::Complex64::ZERO; 3];
        m.init(&v, &mut i);
        // Step a few outer time steps (corrector iterations advance the sub-cycle).
        for step in 1..=5 {
            let t = step as f64 * 0.001;
            m.calc_dynamic(
                &v,
                &mut i,
                0.001,
                t,
                crate::support::dynamics::IterationFlag::SameTimeStep,
            );
        }
        (i, m.pgen, m.wt)
    }
    let (i1, pgen1, wt1) = run();
    let (i2, pgen2, wt2) = run();
    assert_eq!(i1, i2, "WTG3 dynamics must be deterministic");
    assert_eq!(pgen1, pgen2);
    assert_eq!(wt1, wt2);
    for c in i1 {
        assert!(
            c.re.is_finite() && c.im.is_finite(),
            "injection current finite: {c}"
        );
    }
    assert!(
        pgen1.is_finite() && pgen1 > 0.0,
        "Pgen finite & generating: {pgen1}"
    );
    assert!(
        wt1.is_finite() && wt1 > 0.0,
        "rotor speed finite & positive: {wt1}"
    );
}

/// The full aerodynamic wind-speed sweep the daily wind shape drives: below
/// VCutIn (5) and above VCutOut (23) the WindGen delivers only 0.001*kWBase; in
/// between it follows Pm=0.5*rho*pi*Rad^2*v^3*Cp, curtailed to kWBase. This is
/// the offline twin of the (single-step, losses-quirk-limited) live daily deck.
#[test]
fn aerodynamic_wind_speed_sweep() {
    let sys = default_recalc_ctx();
    let aero_pg = |v: f64| 0.5 * 1.225 * PI * 40.0_f64.powi(2) * v.powi(3) * 0.41 / 1e3;
    for (vwind, kw_base) in [
        (4.0, 3000.0),
        (10.0, 3000.0),
        (15.0, 3000.0),
        (24.0, 3000.0),
    ] {
        let mut g = edit_windgen(&[
            ("kv", "0.69"),
            ("kW", &kw_base.to_string()),
            ("vwind", &vwind.to_string()),
        ]);
        g.set_nominal_generation(&sys, &[]);
        let expect_total = if !(5.0..=23.0).contains(&vwind) {
            // cut-in / cut-out: Pnominalperphase = 0.001*kWBase (watts), so the
            // 3-phase total is a tiny 0.001*kWBase*nphases/1e3 kW (~off).
            0.001 * kw_base * g.cd.nphases as f64 / 1e3
        } else {
            aero_pg(vwind).min(kw_base) // aero curve, curtailed to kWBase
        };
        let total_p = g.p_nominal_per_phase * g.cd.nphases as f64 / 1e3; // kW
        assert!(
            (total_p - expect_total).abs() < 1e-6,
            "vwind={vwind}: total P {total_p} kW vs expected {expect_total}"
        );
    }
}

/// Offline pin of the WTG3 dynamics state (the `windgen_dyn` deck run for 20x1ms
/// steps) against the capi015 reference values (probed on dss_capi 0.15.0b4).
/// The `modes/windgen/windgen_dyn.dss` live gate pins all 22 variables exactly;
/// this guards against a Rust-side regression even without the oracle installed.
#[test]
fn dynamics_variables_match_capi015_reference() {
    let mut dss = Dss::new();
    for c in [
        "clear",
        "new circuit.wtg_dyn basekv=0.69 phases=3 bus1=srcbus",
        "new line.l1 bus1=srcbus bus2=wbus phases=3 r1=0.005 x1=0.02 length=1",
        "new windgen.w1 bus1=wbus phases=3 kv=0.69 kW=1500 kva=1800 conn=wye model=1 vss=1 pss=1 qss=0 vwind=12",
        "set voltagebases=[0.69]",
        "calcvoltagebases",
        "solve",
        "set mode=dynamic stepsize=0.001 number=20",
        "solve",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "errors: {:?}", dss.errors());
    let v = dss.element_variables("WindGen.w1").expect("vars");
    // capi015 reference (probe_windgen.py, dss_capi 0.15.0b4, 20x1ms):
    // idx (0-based): 4=Pgen, 6=Qgen, 8=Vmag, 11=WtAct, 17=thetaPitch, 18=Pg,
    //                21=s.
    let check = |i: usize, name: &str, expect: f64, tol: f64| {
        assert!(
            (v[i] - expect).abs() < tol,
            "{name}: Rust {} vs capi015 {expect} (tol {tol})",
            v[i]
        );
    };
    check(4, "Pgen", 1.0021984, 1e-3);
    check(6, "Qgen", -0.0062978, 1e-3);
    check(8, "Vmag", 1.0152462, 1e-3);
    check(11, "WtAct", 1.1999977, 1e-4);
    check(17, "thetaPitch", 4.3769679, 1e-2);
    check(18, "Pg", 1584.0, 1e-6); // kVA*PF re-derive, not the parsed kW=1500
    check(21, "s", -0.1387536, 1e-6);
}
