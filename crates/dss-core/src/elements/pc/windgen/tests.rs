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

            was_quoted: false,
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

/// RP3.10 — offline pin of the WTG3 dynamics state on BOTH `modes/windgen`
/// dynamics decks — `windgen_dyn.dss` (20x1ms, healthy) and
/// `windgen_dyn_fault.dss` (30x1ms, sustained 3-phase fault) — **naming both
/// engines' numbers**.
///
/// Neither deck carries a `QMode=` token, so both run the constant-Q arm this
/// port implements and r4133 lacks (`nominal.rs`; upstream falls through to
/// `Else kvarCalc := 0`, `WindGen.pas:1320-1321`). `WindGen.pas:1254` skips the
/// whole P/Q block in dynamics, so that arm reaches these runs through **one**
/// channel: the snapshot `solve` each deck performs before `Set mode=dynamic`,
/// which now injects `kvarBase = 854.95263026673` kvar
/// (`q_nominal_per_phase = 284984.21008891` var/phase) instead of 0 and moves
/// the WTG3 initial condition.
///
/// Three of the 22 state variables move out of the band they were pinned at on
/// the healthy deck and four on the faulted one (`thetaPitch` is the extra);
/// both engines' readings are named below — r4133's re-measured live on the
/// EPRI r4133 DLL (Version 11.0.0.1) through `epri-worker`. The other checked
/// ones still agree with the oracle at their original tolerances, which is what
/// proves the divergence is confined to the Q channel. The live gate excludes
/// exactly those three / four variables on the matching deck
/// (`tests/corpus/ledger.json`, `windgen-qmode0-constant-q-dyn-r4133` and
/// `windgen-qmode0-constant-q-dynfault-r4133`, cause `windgen-qmode0-no-arm`)
/// and keeps the other 19 / 18 compared.
///
/// Tolerances are the ones this test has always used (1e-3 / 1e-4 / 1e-2 /
/// 1e-6) — re-centred, never loosened. The faulted deck's `thetaPitch` is
/// pinned at 1e-4 rather than the healthy deck's 1e-2 because its divergence is
/// 2.99e-4: a looser band could not tell the two engines apart.
#[test]
fn dynamics_variables_match_the_qmode0_dispatch() {
    // The two decks, verbatim (`tests/corpus/modes/windgen/windgen_dyn.dss` and
    // `.../windgen_dyn_fault.dss`); the faulted one adds the sustained 3-phase
    // `Fault.f1` and runs 30 steps instead of 20.
    let run = |fault: bool| -> Vec<f64> {
        let mut dss = Dss::new();
        let mut cmds: Vec<String> = vec![
            "clear".into(),
            format!(
                "new circuit.{} basekv=0.69 phases=3 bus1=srcbus",
                if fault { "wtg_fault" } else { "wtg_dyn" }
            ),
            "new line.l1 bus1=srcbus bus2=wbus phases=3 r1=0.005 x1=0.02 length=1".into(),
            "new windgen.w1 bus1=wbus phases=3 kv=0.69 kW=1500 kva=1800 conn=wye \
             model=1 vss=1 pss=1 qss=0 vwind=12"
                .into(),
        ];
        if fault {
            cmds.push("new fault.f1 bus1=wbus phases=3 r=0.05".into());
        }
        cmds.push("set voltagebases=[0.69]".into());
        cmds.push("calcvoltagebases".into());
        cmds.push("solve".into());
        cmds.push(format!(
            "set mode=dynamic stepsize=0.001 number={}",
            if fault { 30 } else { 20 }
        ));
        cmds.push("solve".into());
        for c in &cmds {
            dss.command(c);
        }
        assert!(dss.errors().is_empty(), "errors: {:?}", dss.errors());
        dss.element_variables("WindGen.w1").expect("vars")
    };
    // idx (0-based): 4=Pgen, 6=Qgen, 8=Vmag, 11=WtAct, 12=dOmg,
    //                17=thetaPitch, 18=Pg, 21=s.
    let check = |v: &[f64], deck: &str, i: usize, name: &str, expect: f64, tol: f64| {
        assert!(
            (v[i] - expect).abs() < tol,
            "{deck} {name}: Rust {} vs capi015/r4133 {expect} (tol {tol})",
            v[i]
        );
    };
    // The variables the constant-Q dispatch moves. `port` is this engine's
    // measured value, `oracle` the number the upstream engine reports; the
    // second assert keeps the divergence deliberate — it fails the moment the
    // engine drifts back onto the upstream zero-var dispatch.
    let diverges =
        |v: &[f64], deck: &str, i: usize, name: &str, port: f64, oracle: f64, tol: f64| {
            assert!(
                (v[i] - port).abs() < tol,
                "{deck} {name}: Rust {} vs the RP3.10 constant-Q value {port} (tol {tol})",
                v[i]
            );
            assert!(
                (v[i] - oracle).abs() > tol,
                "{deck} {name}: Rust {} must NOT match the upstream zero-var \
                 reading {oracle} — r4133/capi015 dispatch 0 vars here (`Else \
                 kvarCalc := 0`, WindGen.pas:1320-1321), excluded per case in \
                 ledger.json, cause `windgen-qmode0-no-arm`",
                v[i]
            );
        };

    // ---- windgen_dyn.dss (healthy, 20x1ms) ------------------------------
    // Oracle: capi015 reference (probe_windgen.py, dss_capi 0.15.0b4); r4133
    // reproduces it (both engines take the `Else kvarCalc := 0` path), live
    // readings Pgen 1.002198412222774, Qgen -0.006297830900882717, dOmg
    // 0.010128443304934094.
    let v = run(false);
    diverges(
        &v,
        "windgen_dyn",
        4,
        "Pgen",
        1.0045538616795757,
        1.0021984,
        1e-3,
    );
    diverges(
        &v,
        "windgen_dyn",
        6,
        "Qgen",
        -0.00768950341712867,
        -0.0062978,
        1e-3,
    );
    diverges(
        &v,
        "windgen_dyn",
        12,
        "dOmg",
        0.012407481452484234,
        0.010128443304934094,
        1e-3,
    );
    // Unmoved (measured post-fix: Vmag 1.0151455298956373, WtAct
    // 1.2000018957179186, thetaPitch 4.377043172761117, s -0.13875361782251128)
    // — still the oracle's readings at the original tolerances.
    check(&v, "windgen_dyn", 8, "Vmag", 1.0152462, 1e-3);
    check(&v, "windgen_dyn", 11, "WtAct", 1.1999977, 1e-4);
    check(&v, "windgen_dyn", 17, "thetaPitch", 4.3769679, 1e-2);
    check(&v, "windgen_dyn", 18, "Pg", 1584.0, 1e-6); // kVA*PF re-derive, not kW=1500
    check(&v, "windgen_dyn", 21, "s", -0.1387536, 1e-6);

    // ---- windgen_dyn_fault.dss (sustained 3-phase fault, 30x1ms) --------
    // The fault holds the terminal near 0.898 pu, so LVPL/LVQL engage and the
    // moved snapshot state reaches one variable more than on the healthy deck.
    // Oracle values re-measured live on r4133 (epri-worker, Version 11.0.0.1).
    let v = run(true);
    diverges(
        &v,
        "windgen_dyn_fault",
        4,
        "Pgen",
        1.0086500012360733,
        1.0042629667839191,
        1e-3,
    );
    diverges(
        &v,
        "windgen_dyn_fault",
        6,
        "Qgen",
        0.0688589386657973,
        0.07121218770486548,
        1e-3,
    );
    diverges(
        &v,
        "windgen_dyn_fault",
        12,
        "dOmg",
        -0.08630408763613222,
        -0.0812032858835468,
        1e-3,
    );
    diverges(
        &v,
        "windgen_dyn_fault",
        17,
        "thetaPitch",
        4.37634056945387,
        4.376041805109156,
        1e-4,
    );
    // Unmoved on this deck too (port Vmag 0.8982642716641427, WtAct
    // 1.1999354709415782 — 3.3e-5 and 1.2e-5 from r4133, both inside the gate's
    // ~1.2e-4 floor, so they stay oracle-compared there as well).
    check(&v, "windgen_dyn_fault", 8, "Vmag", 0.8982967694080912, 1e-3);
    check(
        &v,
        "windgen_dyn_fault",
        11,
        "WtAct",
        1.1999230833079593,
        1e-4,
    );
    check(&v, "windgen_dyn_fault", 18, "Pg", 1584.0, 1e-6);
    check(&v, "windgen_dyn_fault", 21, "s", -0.13875361782251128, 1e-6);
}

/// RP3.10 — `QMode=0` dispatches the base kvar, in BOTH lanes.
///
/// `TWindGenObj.SetNominalGeneration`'s `case WindModelDyn.QMode`
/// (`WindGen.pas:1276-1322`) implements arm 1 (PF) and arm 2 (volt-var) and has
/// **no arm 0**, so mode 0 — `Create`'s default (`:1020`) — falls through to
/// `Else kvarCalc := 0` (`:1320-1321`) and a default-configured WindGen injects
/// **zero vars** however its `kvar=`/`pf=` reads. Both numbers, per token set:
/// r4133 dispatches `0` (probed live on the EPRI r4133 DLL, Version 11.0.0.1,
/// on all five `modes/windgen` decks and in every configuration), this engine
/// dispatches `kvarBase` — the reading the property help (`:429-430`, `0:Q`),
/// the WTG3 model (`WTG3_Model.pas:252`, `:1059-1061`), models 4/5 (`:1361`,
/// `:1797`), arm 1's saturation fallback (`:1284`) and arm 2's scale point
/// (`:1313-1316`) all agree on. The divergence is excluded per case in
/// `tests/corpus/ledger.json` (cause `windgen-qmode0-no-arm`).
#[test]
fn qmode0_dispatches_the_base_kvar() {
    let sys = default_recalc_ctx();
    let dispatch = |edits: &[(&str, &str)]| {
        let mut g = edit_windgen(edits);
        assert_eq!(g.wind_model_dyn.q_mode, 0, "the decks type no QMode=");
        g.set_nominal_generation(&sys, &[]);
        (g.kw_base, g.kvar_base, g.q_nominal_per_phase)
    };
    let msg = "r4133 dispatches 0 here (`Else kvarCalc := 0`, \
               WindGen.pas:1320-1321) — excluded per case in ledger.json, \
               cause `windgen-qmode0-no-arm`";

    // modes/windgen/windgen_snap_delta.dss
    let (kw, kvar, q) = dispatch(&[
        ("phases", "3"),
        ("kv", "12.47"),
        ("kW", "1500"),
        ("pf", "0.9"),
        ("conn", "delta"),
        ("model", "1"),
        ("vwind", "12"),
    ]);
    assert_eq!(kw, 1500.0);
    assert_eq!(kvar, 726.4831572567788, "snap_delta kvarBase");
    // The arm is a copy, not arithmetic: `Qnominalperphase = 1e3*kvarCalc*
    // LeadLag*Factor/Fnphases` (`:1325`) with LeadLag = Factor = 1.
    assert_eq!(q, 1e3 * 726.4831572567788 / 3.0, "snap_delta: {msg}");
    assert_eq!(q, 242161.05241892627);

    // modes/windgen/windgen_daily.dss
    let (kw, kvar, q) = dispatch(&[
        ("phases", "3"),
        ("kv", "12.47"),
        ("kW", "3000"),
        ("pf", "0.95"),
        ("conn", "wye"),
        ("model", "1"),
        ("vwind", "12"),
    ]);
    assert_eq!(kw, 3000.0);
    assert_eq!(kvar, 986.0523155365896, "daily kvarBase");
    assert_eq!(q, 1e3 * 986.0523155365896 / 3.0, "daily: {msg}");
    assert_eq!(q, 328684.1051788632);

    // modes/windgen/windgen_dyn.dss + windgen_dyn_fault.dss (kVA set, no PF:
    // RecalcElementData re-derives kWBase = kVA*|PF| and kvarBase =
    // sqrt(kVA^2 - kWBase^2), `WindGen.pas:1375-1384`).
    let (kw, kvar, q) = dispatch(&[
        ("phases", "3"),
        ("kv", "0.69"),
        ("kW", "1500"),
        ("kva", "1800"),
        ("conn", "wye"),
        ("model", "1"),
        ("vwind", "12"),
    ]);
    assert_eq!(kw, 1584.0, "kVA*|PF| re-derive, not the parsed kW=1500");
    assert_eq!(kvar, 854.95263026673, "dyn kvarBase");
    assert_eq!(q, 1e3 * 854.95263026673 / 3.0, "dyn: {msg}");
    assert_eq!(q, 284984.21008891);

    // modes/windgen/windgen_snap.dss — unity PF, so the new arm is a literal
    // no-op and this deck stays byte-identical to r4133 (no ledger entry).
    let (kw, kvar, q) = dispatch(&[
        ("phases", "3"),
        ("kv", "12.47"),
        ("kW", "1500"),
        ("pf", "1.0"),
        ("conn", "wye"),
        ("model", "1"),
        ("vwind", "12"),
    ]);
    assert_eq!(kw, 1500.0);
    assert_eq!(kvar, 0.0);
    assert_eq!(q, 0.0, "unity PF: nothing to dispatch");

    // Discriminator: the same daily object under `QMode=1` still takes arm 1,
    // which follows the wind (`Pg`) instead of the base. At the daily shape's
    // hour-1 10 m/s (Pg = 1262.291928212379 kW) arm 1 dispatches
    // 138298.43096632924 var/phase (414.89529289898772 kvar total) against the
    // base's 986.0523155365896 kvar — so this pin separates the arms rather
    // than hardwiring `kvarBase` into every mode.
    let mut g = edit_windgen(&[
        ("phases", "3"),
        ("kv", "12.47"),
        ("kW", "3000"),
        ("pf", "0.95"),
        ("conn", "wye"),
        ("model", "1"),
        ("vwind", "10"),
        ("qmode", "1"),
    ]);
    assert_eq!(g.wind_model_dyn.q_mode, 1);
    g.set_nominal_generation(&sys, &[]);
    assert_eq!(g.pg, 1262.291928212379);
    assert_eq!(
        g.q_nominal_per_phase, 138298.43096632924,
        "arm 1, Pg-driven"
    );
    // ... while mode 0 on the very same tokens ignores the wind entirely.
    let (_, _, q0) = dispatch(&[
        ("phases", "3"),
        ("kv", "12.47"),
        ("kW", "3000"),
        ("pf", "0.95"),
        ("conn", "wye"),
        ("model", "1"),
        ("vwind", "10"),
    ]);
    assert_eq!(q0, 1e3 * 986.0523155365896 / 3.0, "constant in wind: {msg}");
}

/// RP3.10 — the mode-0 dispatch carries `kvarBase`'s sign and scales with
/// `Factor` (`GenMultiplier`), exactly as arms 1/2 do.
///
/// Sign: `kvarBase` is already signed by the typed `kvar=`
/// (`WindGen.pas:3001`) or by `pf<0` (`:3028`), while arm 1 reaches the same
/// signed answer by putting the sign in `LeadLag` over a non-negative `sqrt`
/// (`:1286-1287`) — so the new arm leaves `LeadLag` at 1; re-applying it would
/// double-negate. Probed on r4133: `kW=1000 pf=-0.9` gives `+484.32` kvar at the
/// terminal through arm 1 (`QMode=1`) and through `model=4` alike.
/// `Factor`: `:1325` sits OUTSIDE the `case`, so every arm scales with it
/// (probed: `Set genmult=0.5` halves arm 1's dispatch), while `varBase` — the
/// models 4/5 injection, `:1361` — does not.
#[test]
fn qmode0_dispatch_carries_the_sign_and_scales_with_genmult() {
    let sys = default_recalc_ctx();
    let run = |edits: &[(&str, &str)], sys: &crate::elements::traits::SysCtx| {
        let mut g = edit_windgen(edits);
        g.set_nominal_generation(sys, &[]);
        (g.kvar_base, g.q_nominal_per_phase, g.var_base)
    };
    let neg_pf: &[(&str, &str)] = &[
        ("phases", "3"),
        ("kv", "0.69"),
        ("kW", "1000"),
        ("pf", "-0.9"),
        ("vwind", "12"),
    ];
    let (kvar_base, q_mode0, _) = run(neg_pf, &sys);
    assert_eq!(kvar_base, -484.3221048378525, "pf<0 signs kvarBase itself");
    assert_eq!(q_mode0, -161440.7016126175);
    assert_eq!(q_mode0, 1e3 * -484.3221048378525 / 3.0);
    // Arm 1 on the same tokens agrees to the last ulp — it just gets there
    // through `LeadLag = -1` on a non-negative `sqrt` (r4133 dispatches 0 in
    // mode 0; cause `windgen-qmode0-no-arm`).
    let mut arm1 = neg_pf.to_vec();
    arm1.push(("qmode", "1"));
    let (_, q_mode1, _) = run(&arm1, &sys);
    assert_eq!(q_mode1, -161440.70161261753);
    assert!(
        (q_mode0 - q_mode1).abs() <= f64::EPSILON * q_mode0.abs(),
        "mode 0 {q_mode0} vs arm 1 {q_mode1}: same signed answer, 1 ulp apart"
    );

    // A typed negative `kvar=` reaches the same convention exactly.
    let neg_kvar: &[(&str, &str)] = &[
        ("phases", "3"),
        ("kv", "0.69"),
        ("kW", "1000"),
        ("kvar", "-400"),
        ("vwind", "12"),
    ];
    let (kvar_base, q_mode0, _) = run(neg_kvar, &sys);
    assert_eq!(kvar_base, -400.0);
    assert_eq!(q_mode0, 1e3 * -400.0 / 3.0);
    assert_eq!(q_mode0, -133333.33333333334);

    // Factor: halve `GenMultiplier` and the mode-0 dispatch halves exactly,
    // while `varBase` (models 4/5) is untouched.
    let mut sys05 = default_recalc_ctx();
    sys05.gen_multiplier = 0.5;
    let pos_pf: &[(&str, &str)] = &[
        ("phases", "3"),
        ("kv", "0.69"),
        ("kW", "1000"),
        ("pf", "0.9"),
        ("vwind", "12"),
    ];
    let (_, q_full, var_base_full) = run(pos_pf, &sys);
    let (_, q_half, var_base_half) = run(pos_pf, &sys05);
    assert_eq!(q_full, 161440.7016126175);
    assert_eq!(q_half, 161440.7016126175 / 2.0);
    assert_eq!(q_half, 80720.35080630875);
    assert_eq!(
        var_base_full, var_base_half,
        "varBase ignores GenMultiplier"
    );
    assert_eq!(var_base_full, 161440.7016126175);
}

/// RP3.10 — mode 0 dispatches zero only where the base itself is zero, and the
/// paths that zero Q for every mode keep doing so.
///
/// This is the guard against "fixing" the missing arm into a new hardwired
/// constant: `pf=1.0` (the `windgen_snap.dss` deck) has `kvarBase = 0`, so it
/// stays byte-identical to r4133; above `VCutOut` the turbine-off block
/// (`WindGen.pas:1243-1250`) zeroes `Qnominalperphase` before the `case` is
/// ever reached; and an out-of-range `QMode` — reachable through
/// `SetVariable(15)` (`dynamics.rs`), which takes any integer — keeps
/// upstream's `Else`.
#[test]
fn qmode0_zero_only_when_the_base_is_zero() {
    let sys = default_recalc_ctx();

    // Unity PF: zero base, zero dispatch (both engines agree here).
    let mut g = edit_windgen(&[
        ("phases", "3"),
        ("kv", "12.47"),
        ("kW", "1500"),
        ("pf", "1.0"),
        ("vwind", "12"),
    ]);
    g.set_nominal_generation(&sys, &[]);
    assert_eq!(g.kvar_base, 0.0);
    assert_eq!(g.q_nominal_per_phase, 0.0);

    // Above VCutOut (23 m/s): the turbine is off, so Q is 0 in every mode even
    // though the base is 726.48 kvar.
    for mode in ["0", "1", "2"] {
        let mut g = edit_windgen(&[
            ("phases", "3"),
            ("kv", "12.47"),
            ("kW", "1500"),
            ("pf", "0.9"),
            ("vwind", "30"),
            ("qmode", mode),
        ]);
        g.set_nominal_generation(&sys, &[]);
        assert_eq!(g.kvar_base, 726.4831572567788);
        assert_eq!(g.pg, 0.0);
        assert_eq!(
            g.q_nominal_per_phase, 0.0,
            "above VCutOut QMode={mode} must dispatch nothing"
        );
    }

    // An out-of-range mode still falls through to `Else kvarCalc := 0`.
    let mut g = edit_windgen(&[
        ("phases", "3"),
        ("kv", "12.47"),
        ("kW", "1500"),
        ("pf", "0.9"),
        ("vwind", "12"),
    ]);
    g.set_wgen_variable(15, 7.0);
    assert_eq!(g.wind_model_dyn.q_mode, 7);
    g.set_nominal_generation(&sys, &[]);
    assert_eq!(g.kvar_base, 726.4831572567788);
    assert_eq!(
        g.q_nominal_per_phase, 0.0,
        "out-of-range QMode keeps the Else"
    );
}
