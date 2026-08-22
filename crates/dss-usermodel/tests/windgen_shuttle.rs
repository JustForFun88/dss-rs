//! WindGen shuttle gate (`R4133_PROPS_PLAN.md` RP1.3): drives the COMMITTED
//! `tests/fixtures/wasm/wgturbine.wasm` through the `dss-usermodel` crate API
//! and pins the **`TWindGenVars` boundary record** end to end — host codec
//! (`records::WindGenVars`, ABI doc §2.6) → `dss_alloc` buffer → guest decoder →
//! guest write-back → host read-back.
//!
//! This is the WindGen analog of `fixture_self_gate.rs`, with one difference in
//! kind: `indmach012a` has a native twin, so its self-gate pins *math* against
//! an oracle. There is no WindGen user-model DLL anywhere upstream (r4133 ships
//! only the loader, `PCElements/WindGenUserModel.pas`), so the authored fixture
//! is its own numeric spec and this file pins the **contract**: which bytes land
//! where, which fields come back, and that a `TGeneratorVars` shuttle cannot be
//! substituted for a `TWindGenVars` one. The model uses only f64 `+ - * /` and
//! `sqrt` (IEEE-exact in wasm and on x86-64 SSE2), so every assertion here is
//! bit equality — a mismatch is a bug, never tolerance.
//!
//! The offsets under test are the P10 probe's
//! (`docs/wasm/probes/p10_offsets_windgenvars_r4133.txt`) with the managed
//! `PLoss: string` reference dropped and its hole closed. The single most
//! load-bearing assertion is on `Lamda`: the guest computes it from `ag`, the
//! first turbine-tail field, which sits at 244 **only because** the hole is
//! closed — re-open it and `Lamda` reads garbage.

use dss_usermodel::{
    DynamicsRec, HostConfig, InterfaceKind, NoCallbacks, Shuttle, UserModelHost, UserModelInstance,
    WindGenShuttle, WindGenVars,
};
use num_complex::Complex64;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Scenario inputs — the fixed deck-shaped state a 3-phase wye WindGen presents
// ---------------------------------------------------------------------------

/// Terminal order of a 3-phase wye WindGen (3 phases + neutral).
const YORDER: usize = 4;
/// `2*pi*60`, the same literal the engine's `w0` carries.
const W0: f64 = 376.99111843077515;
/// Pascal `TSolveMode` DYNAMICMODE = 14 (`Shared/Dynamics.pas`).
const DYNAMICMODE: i32 = 14;
/// Non-dynamics mode ordinal (SNAPSHOT = 0).
const SNAPSHOT: i32 = 0;

const V_PF: [Complex64; 3] = [
    Complex64::new(277.13, 0.0),
    Complex64::new(-140.02, -239.51),
    Complex64::new(-137.11, 240.05),
];
const V_DYN_A: [Complex64; 3] = [
    Complex64::new(276.4, 3.1),
    Complex64::new(-141.0, -238.2),
    Complex64::new(-135.9, 239.9),
];
const V_DYN_B: [Complex64; 3] = [
    Complex64::new(274.9, -2.4),
    Complex64::new(-139.4, -240.7),
    Complex64::new(-136.8, 238.6),
];

/// `UserData=` for the fixture's own parser (`wgturbine::Model::edit`).
const EDIT_STR: &str = "g=0.0021 b=-0.0007 slip=0.015 eta=0.93 tau=0.02";
const G: f64 = 0.0021;
const B: f64 = -0.0007;
const SLIP: f64 = 0.015;
const ETA: f64 = 0.93;
const TAU: f64 = 0.02;
/// Dynamics step, s.
const H: f64 = 0.001;

/// Gearbox ratio — the record's FIRST turbine-tail field (wasm offset 244).
const AG: f64 = 97.5;
const KVA_RATING: f64 = 2000.0;
const KV_BASE: f64 = 0.48;
const PNOMINAL: f64 = 555_000.0;
const QNOMINAL: f64 = -120_000.0;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn fixture_bytes(name: &str) -> Vec<u8> {
    let path = repo_root()
        .join("tests")
        .join("fixtures")
        .join("wasm")
        .join(name);
    std::fs::read(&path)
        .unwrap_or_else(|e| panic!("committed fixture must exist at {}: {e}", path.display()))
}

/// The element-side `TWindGenVars` the engine would hand the model: every
/// field distinct so a mis-decode cannot alias onto a neighbour.
fn record() -> WindGenVars {
    WindGenVars {
        theta: 0.125,
        pshaft: -111.0,
        speed: -1.0,
        w0: W0,
        hmass: 3.5,
        mmass: 37.0,
        d: 1.5,
        dpu: 1.0,
        kva_rating: KVA_RATING,
        kv_windgen_base: KV_BASE,
        xd: 1.1,
        xdp: 0.21,
        xdpp: 0.11,
        pu_xd: 1.05,
        pu_xdp: 0.2,
        pu_xdpp: 0.1,
        dtheta: 0.0,
        dspeed: -2.0,
        theta_history: 0.05,
        speed_history: 0.06,
        pnominalperphase: PNOMINAL,
        qnominalperphase: QNOMINAL,
        num_phases: 3,
        num_conductors: 4,
        conn: 0,
        vthev_mag: 277.0,
        vthev_harm: 276.0,
        theta_harm: 0.07,
        vtarget: 278.0,
        zthev: (0.02, 0.19),
        xrdp: 20.0,
        ag: AG,
        cp: -3.0,
        lamda: -4.0,
        poles: 4.0,
        pd: 1.225,
        rad: 45.0,
        v_cutin: 3.0,
        v_cutout: 25.0,
        pm: -5.0,
        ps: -6.0,
        pr: -7.0,
        pg: -8.0,
        s: -9.0,
    }
}

fn dyn_rec(mode: i32) -> DynamicsRec {
    DynamicsRec {
        h: H,
        t: 0.5,
        tstart: 0.0,
        tstop: 1.0,
        iteration_flag: 0,
        solution_mode: mode,
        int_hour: 0,
        dbl_hour: 0.5,
    }
}

fn host(kind: InterfaceKind) -> UserModelHost {
    UserModelHost::load(
        "wgturbine.wasm",
        &fixture_bytes("wgturbine.wasm"),
        kind,
        HostConfig::default(),
    )
    .expect("the committed WindGen fixture loads and validates")
}

/// Load + `new` + `edit(UserData)` — the Pascal load sequence (ABI doc §3).
fn bound(wg: &mut WindGenVars, host: &UserModelHost) -> UserModelInstance {
    let mut dr = dyn_rec(SNAPSHOT);
    let mut inst = UserModelInstance::new(
        host,
        YORDER,
        WindGenShuttle {
            wind_gen_vars: wg,
            dyn_rec: &mut dr,
            ctx: Box::new(NoCallbacks),
        },
    )
    .expect("instantiate");
    assert!(inst.exists(), "guest `new` must return a nonzero id");
    let mut dr = dyn_rec(SNAPSHOT);
    inst.edit(
        EDIT_STR,
        WindGenShuttle {
            wind_gen_vars: wg,
            dyn_rec: &mut dr,
            ctx: Box::new(NoCallbacks),
        },
    )
    .expect("edit");
    inst
}

/// The fixture's own complex product, reproduced bit-for-bit.
fn y_mul(v: Complex64) -> Complex64 {
    Complex64::new(G * v.re - B * v.im, G * v.im + B * v.re)
}

/// `Σ_k Re(V[k]·conj(I[k]))` in the guest's accumulation order.
fn total_p(v: &[Complex64; 3], i: &[Complex64; 3]) -> f64 {
    let mut pg = 0.0;
    for k in 0..3 {
        pg += v[k].re * i[k].re + v[k].im * i[k].im;
    }
    pg
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// The fixture validates as the 15-function `WindGenUserModel` interface —
/// `new(windgenvars, dynarec) -> id` plus the `TAIL_15` set in the Pascal
/// binding order (`WindGenUserModel.pas:180-194`).
#[test]
fn fixture_validates_as_the_windgen_interface() {
    let h = host(InterfaceKind::WindGenUserModel);
    assert_eq!(h.kind(), InterfaceKind::WindGenUserModel);
    assert_eq!(h.model(), "wgturbine.wasm");
}

/// Power flow (`WindGen.pas:1887`, `DoUserModel` → `FCalc`): the currents are
/// the admittance currents bit-exactly, the untouched neutral slot survives,
/// and every turbine output lands at its own offset on the way back.
///
/// `Lamda == ag·(1+slip)` is the ABI §2.6 witness: `ag` is only readable at
/// wasm offset 244 because the managed `PLoss` reference does not cross.
#[test]
fn power_flow_round_trips_the_whole_windgen_record() {
    let h = host(InterfaceKind::WindGenUserModel);
    let mut wg = record();
    let mut inst = bound(&mut wg, &h);

    // The record must survive `new` + `edit` untouched — neither guest export
    // writes it (Pascal `New`/`Edit` likewise only take the pointer/string).
    assert_eq!(wg, record(), "new/edit must not disturb the record");

    let v: Vec<Complex64> = V_PF.iter().copied().chain([Complex64::ZERO]).collect();
    let mut i = vec![Complex64::ZERO; YORDER];
    let mut dr = dyn_rec(SNAPSHOT);
    inst.calc(
        &v,
        &mut i,
        WindGenShuttle {
            wind_gen_vars: &mut wg,
            dyn_rec: &mut dr,
            ctx: Box::new(NoCallbacks),
        },
    )
    .expect("calc");

    let want_i: [Complex64; 3] = [y_mul(V_PF[0]), y_mul(V_PF[1]), y_mul(V_PF[2])];
    for k in 0..3 {
        assert_eq!(i[k], want_i[k], "phase {} current", k + 1);
    }
    assert_eq!(
        i[3],
        Complex64::ZERO,
        "the model writes NumPhases entries; the neutral slot stays as the host left it"
    );

    let pg = total_p(&V_PF, &want_i);
    let pm = pg / ETA;
    assert_eq!(wg.pg, pg, "Pg @332");
    assert_eq!(wg.ps, pg * (1.0 - SLIP), "Ps @316");
    assert_eq!(wg.pr, pg * SLIP, "Pr @324");
    assert_eq!(wg.pm, pm, "Pm @308");
    assert_eq!(wg.s, SLIP, "s @340 — the last field of the image");
    assert_eq!(wg.cp, pg / (KVA_RATING * 1000.0), "Cp @252");
    assert_eq!(
        wg.lamda,
        AG * (1.0 + SLIP),
        "Lamda @260 is computed from ag @244 — the first turbine-tail field, \
         which sits there only because the managed PLoss reference is dropped \
         and its hole closed (ABI doc §2.6)"
    );
    assert_eq!(wg.pshaft, -pm, "Pshaft @8");
    assert_eq!(wg.speed, 0.0, "Speed @16 — untouched by power flow");
    assert_eq!(wg.dspeed, 0.0, "dSpeed @136 — untouched by power flow");

    // Everything the model does NOT write comes back exactly as it went in —
    // including the turbine *inputs* (ag, Poles, pd, Rad, VCutin, VCutout),
    // which stay engine-owned.
    let orig = record();
    assert_eq!(wg.theta, orig.theta);
    assert_eq!(wg.w0, orig.w0);
    assert_eq!(wg.kva_rating, orig.kva_rating);
    assert_eq!(wg.kv_windgen_base, orig.kv_windgen_base);
    assert_eq!(wg.qnominalperphase, orig.qnominalperphase);
    assert_eq!(wg.num_phases, orig.num_phases);
    assert_eq!(wg.num_conductors, orig.num_conductors);
    assert_eq!(wg.conn, orig.conn);
    assert_eq!(wg.zthev, orig.zthev);
    assert_eq!(wg.xrdp, orig.xrdp);
    assert_eq!(wg.ag, orig.ag);
    assert_eq!(wg.poles, orig.poles);
    assert_eq!(wg.pd, orig.pd);
    assert_eq!(wg.rad, orig.rad);
    assert_eq!(wg.v_cutin, orig.v_cutin);
    assert_eq!(wg.v_cutout, orig.v_cutout);
}

/// The state-variable surface (`WindGen.pas:2735-2876`): count, 1-based names,
/// per-index reads, `get_all_vars`, `set_variable`, and the Pascal
/// out-of-range answer. Slots 4-9 echo the record head the guest decoded, so
/// this single call also witnesses the head offsets 24/64/72/176/180/184.
#[test]
fn variable_surface_echoes_the_decoded_record_head() {
    let h = host(InterfaceKind::WindGenUserModel);
    let mut wg = record();
    let mut inst = bound(&mut wg, &h);

    let v: Vec<Complex64> = V_PF.iter().copied().chain([Complex64::ZERO]).collect();
    let mut i = vec![Complex64::ZERO; YORDER];
    let mut dr = dyn_rec(SNAPSHOT);
    inst.calc(
        &v,
        &mut i,
        WindGenShuttle {
            wind_gen_vars: &mut wg,
            dyn_rec: &mut dr,
            ctx: Box::new(NoCallbacks),
        },
    )
    .expect("calc");

    let mut dr = dyn_rec(SNAPSHOT);
    let n = inst
        .num_vars(WindGenShuttle {
            wind_gen_vars: &mut wg,
            dyn_rec: &mut dr,
            ctx: Box::new(NoCallbacks),
        })
        .expect("num_vars");
    assert_eq!(n, 9);

    let names = [
        "WgIout1",
        "WgPg",
        "WgSlip",
        "WgKvaEcho",
        "WgKvBaseEcho",
        "WgW0Echo",
        "WgNphEcho",
        "WgNcondEcho",
        "WgConnEcho",
    ];
    for (k, want) in names.iter().enumerate() {
        let mut dr = dyn_rec(SNAPSHOT);
        let got = inst
            .get_var_name(
                k as i32 + 1,
                WindGenShuttle {
                    wind_gen_vars: &mut wg,
                    dyn_rec: &mut dr,
                    ctx: Box::new(NoCallbacks),
                },
            )
            .expect("get_var_name");
        assert_eq!(&got, want, "variable name {}", k + 1);
        // None of them may collide with a built-in WindGen variable name.
        assert!(got.starts_with("Wg"));
    }

    let want_i: [Complex64; 3] = [y_mul(V_PF[0]), y_mul(V_PF[1]), y_mul(V_PF[2])];
    let pg = total_p(&V_PF, &want_i);
    let want_vars = [
        (want_i[0].re * want_i[0].re + want_i[0].im * want_i[0].im).sqrt(),
        pg,
        SLIP,
        KVA_RATING,
        KV_BASE,
        W0,
        3.0,
        4.0,
        0.0,
    ];
    let mut all = vec![0.0f64; 9];
    let mut dr = dyn_rec(SNAPSHOT);
    inst.get_all_vars(
        &mut all,
        WindGenShuttle {
            wind_gen_vars: &mut wg,
            dyn_rec: &mut dr,
            ctx: Box::new(NoCallbacks),
        },
    )
    .expect("get_all_vars");
    assert_eq!(all, want_vars.to_vec());

    for (k, want) in want_vars.iter().enumerate() {
        let mut dr = dyn_rec(SNAPSHOT);
        let got = inst
            .get_variable(
                k as i32 + 1,
                WindGenShuttle {
                    wind_gen_vars: &mut wg,
                    dyn_rec: &mut dr,
                    ctx: Box::new(NoCallbacks),
                },
            )
            .expect("get_variable");
        assert_eq!(got, *want, "variable {}", k + 1);
    }

    // Out of range → the Pascal sentinel (`WindGen.pas:2730`).
    let mut dr = dyn_rec(SNAPSHOT);
    assert_eq!(
        inst.get_variable(
            10,
            WindGenShuttle {
                wind_gen_vars: &mut wg,
                dyn_rec: &mut dr,
                ctx: Box::new(NoCallbacks),
            },
        )
        .expect("get_variable"),
        -9999.99
    );

    // `set_variable` reaches the model: change the slip, re-`calc`, and the
    // record's `s`/`Lamda` move with it.
    let mut dr = dyn_rec(SNAPSHOT);
    inst.set_variable(
        3,
        0.031,
        WindGenShuttle {
            wind_gen_vars: &mut wg,
            dyn_rec: &mut dr,
            ctx: Box::new(NoCallbacks),
        },
    )
    .expect("set_variable");
    let mut dr = dyn_rec(SNAPSHOT);
    inst.calc(
        &v,
        &mut i,
        WindGenShuttle {
            wind_gen_vars: &mut wg,
            dyn_rec: &mut dr,
            ctx: Box::new(NoCallbacks),
        },
    )
    .expect("calc");
    assert_eq!(wg.s, 0.031);
    assert_eq!(wg.lamda, AG * (1.0 + 0.031));
}

/// Dynamics (`WindGen.pas:2568` `InitStateVars` → `:2663` `IntegrateStates` →
/// `:1993` the `GenModel=6` `FCalc`): `init` seeds the state and writes the
/// machine speed back, `calc` reports the integrated current (not the
/// admittance current), and `integrate` advances both states trapezoidally.
#[test]
fn dynamics_init_calc_integrate_round_trip() {
    let h = host(InterfaceKind::WindGenUserModel);
    let mut wg = record();
    let mut inst = bound(&mut wg, &h);

    let va: Vec<Complex64> = V_DYN_A.iter().copied().chain([Complex64::ZERO]).collect();
    let vb: Vec<Complex64> = V_DYN_B.iter().copied().chain([Complex64::ZERO]).collect();
    let mut i = vec![Complex64::ZERO; YORDER];

    // --- init: state current = Y·V_A, speed = w0·slip ------------------------
    let mut dr = dyn_rec(DYNAMICMODE);
    inst.init(
        &va,
        &mut i,
        WindGenShuttle {
            wind_gen_vars: &mut wg,
            dyn_rec: &mut dr,
            ctx: Box::new(NoCallbacks),
        },
    )
    .expect("init");
    assert_eq!(wg.speed, W0 * SLIP, "Speed @16 seeded by init");
    assert_eq!(wg.dspeed, 0.0, "dSpeed @136 seeded by init");
    assert_eq!(
        wg.pg,
        record().pg,
        "init must not invent turbine outputs — Pg is still the engine's"
    );
    assert!(
        i.iter().all(|c| *c == Complex64::ZERO),
        "init writes no currents"
    );

    // --- calc in DYNAMICMODE: report the state current, cache V_B ------------
    let is0: [Complex64; 3] = [y_mul(V_DYN_A[0]), y_mul(V_DYN_A[1]), y_mul(V_DYN_A[2])];
    let mut dr = dyn_rec(DYNAMICMODE);
    inst.calc(
        &vb,
        &mut i,
        WindGenShuttle {
            wind_gen_vars: &mut wg,
            dyn_rec: &mut dr,
            ctx: Box::new(NoCallbacks),
        },
    )
    .expect("calc");
    for k in 0..3 {
        assert_eq!(
            i[k], is0[k],
            "dynamics reports the integrated state current, not Y·V"
        );
        assert_ne!(
            i[k],
            y_mul(V_DYN_B[k]),
            "…and that is observably different from the power-flow answer"
        );
    }
    assert_eq!(
        wg.pg,
        total_p(&V_DYN_B, &is0),
        "Pg from V_B against the state"
    );
    assert_eq!(wg.speed, W0 * SLIP);

    // --- set a new slip target so the speed state actually moves -------------
    let new_slip = 0.031;
    let mut dr = dyn_rec(DYNAMICMODE);
    inst.set_variable(
        3,
        new_slip,
        WindGenShuttle {
            wind_gen_vars: &mut wg,
            dyn_rec: &mut dr,
            ctx: Box::new(NoCallbacks),
        },
    )
    .expect("set_variable");

    // --- integrate: one trapezoidal half-step on both states -----------------
    let mut dr = dyn_rec(DYNAMICMODE);
    inst.integrate(WindGenShuttle {
        wind_gen_vars: &mut wg,
        dyn_rec: &mut dr,
        ctx: Box::new(NoCallbacks),
    })
    .expect("integrate");

    // is_hist = is0 (dis was 0); dis = (Y·V_B − is0)/tau; is = is_hist + dis·h/2
    let mut want_is = [Complex64::ZERO; 3];
    for k in 0..3 {
        let target = y_mul(V_DYN_B[k]);
        let dis = Complex64::new(
            (target.re - is0[k].re) * (1.0 / TAU),
            (target.im - is0[k].im) * (1.0 / TAU),
        );
        want_is[k] = Complex64::new(
            is0[k].re + dis.re * (0.5 * H),
            is0[k].im + dis.im * (0.5 * H),
        );
    }
    // speed_hist = W0·SLIP (dspeed was 0); dspeed = (W0·new_slip − speed)/tau
    let speed0 = W0 * SLIP;
    let want_dspeed = (W0 * new_slip - speed0) * (1.0 / TAU);
    let want_speed = speed0 + 0.5 * H * want_dspeed;
    assert_eq!(wg.speed, want_speed, "integrate writes Speed @16 back");
    assert_eq!(wg.dspeed, want_dspeed, "integrate writes dSpeed @136 back");

    // --- calc again: the advanced state is what the terminal now sees --------
    let mut dr = dyn_rec(DYNAMICMODE);
    inst.calc(
        &vb,
        &mut i,
        WindGenShuttle {
            wind_gen_vars: &mut wg,
            dyn_rec: &mut dr,
            ctx: Box::new(NoCallbacks),
        },
    )
    .expect("calc");
    for k in 0..3 {
        assert_eq!(
            i[k],
            want_is[k],
            "phase {} state current after integrate",
            k + 1
        );
    }
    assert_eq!(wg.speed, want_speed);
    assert_eq!(wg.dspeed, want_dspeed);
    assert_eq!(wg.s, new_slip);
}

/// `UserData=` reaches the guest parser (Pascal `TWindGenUserModel.Set_Edit`,
/// `WindGenUserModel.pas:150-154`): with no edit the model runs its built-in
/// defaults, and the edited admittance is observably different.
#[test]
fn user_data_edit_changes_the_model() {
    let h = host(InterfaceKind::WindGenUserModel);
    let v: Vec<Complex64> = V_PF.iter().copied().chain([Complex64::ZERO]).collect();

    // Unedited: the fixture's defaults (g = 0.0015, b = -0.0004, slip = 0.02).
    let mut wg = record();
    let mut dr = dyn_rec(SNAPSHOT);
    let mut inst = UserModelInstance::new(
        &h,
        YORDER,
        WindGenShuttle {
            wind_gen_vars: &mut wg,
            dyn_rec: &mut dr,
            ctx: Box::new(NoCallbacks),
        },
    )
    .expect("instantiate");
    let mut i = vec![Complex64::ZERO; YORDER];
    let mut dr = dyn_rec(SNAPSHOT);
    inst.calc(
        &v,
        &mut i,
        WindGenShuttle {
            wind_gen_vars: &mut wg,
            dyn_rec: &mut dr,
            ctx: Box::new(NoCallbacks),
        },
    )
    .expect("calc");
    let default_i0 = i[0];
    assert_eq!(wg.s, 0.02, "the fixture's default slip");

    // Edited.
    let mut wg2 = record();
    let mut inst2 = bound(&mut wg2, &h);
    let mut i2 = vec![Complex64::ZERO; YORDER];
    let mut dr = dyn_rec(SNAPSHOT);
    inst2
        .calc(
            &v,
            &mut i2,
            WindGenShuttle {
                wind_gen_vars: &mut wg2,
                dyn_rec: &mut dr,
                ctx: Box::new(NoCallbacks),
            },
        )
        .expect("calc");
    assert_eq!(i2[0], y_mul(V_PF[0]));
    assert_ne!(i2[0], default_i0, "UserData= must reach the guest");
    assert_eq!(wg2.s, SLIP);
}

/// The two boundary records are **not** interchangeable. A `Shuttle` (which
/// carries `TGeneratorVars`) handed to a WindGen host is refused before a byte
/// is written, and vice versa — the kind check of `UserModelInstance::new`.
#[test]
fn the_wrong_boundary_record_is_refused() {
    let wg_host = host(InterfaceKind::WindGenUserModel);
    let mut gv = dss_usermodel::GeneratorVars::default();
    let mut dr = dyn_rec(SNAPSHOT);
    let err = UserModelInstance::new(
        &wg_host,
        YORDER,
        Shuttle {
            gen_vars: Some(&mut gv),
            dyn_rec: &mut dr,
            ctx: Box::new(NoCallbacks),
        },
    )
    .expect_err("a TGeneratorVars shuttle must not bind to a WindGen host");
    let msg = err.to_string();
    assert!(
        msg.contains("boundary record mismatch"),
        "unexpected error: {msg}"
    );

    // Same fixture, loaded under the Generator kind (the export sets are
    // identical, so load succeeds) — a WindGen shuttle is still refused.
    let gen_host = host(InterfaceKind::GenUserModel);
    let mut wg = record();
    let mut dr = dyn_rec(SNAPSHOT);
    let err = UserModelInstance::new(
        &gen_host,
        YORDER,
        WindGenShuttle {
            wind_gen_vars: &mut wg,
            dyn_rec: &mut dr,
            ctx: Box::new(NoCallbacks),
        },
    )
    .expect_err("a TWindGenVars shuttle must not bind to a Generator host");
    assert!(
        err.to_string().contains("boundary record mismatch"),
        "unexpected error: {err}"
    );
}

/// Binding this guest through the *Generator* kind allocates the 244-byte
/// `TGeneratorVars` buffer, and the guest — which decodes 348 bytes — traps on
/// its own bounds check. Proof that the record size is load-bearing and not a
/// cosmetic difference: the sizes cannot be quietly swapped.
#[test]
fn a_generator_sized_buffer_traps_the_windgen_guest() {
    let gen_host = host(InterfaceKind::GenUserModel);
    let mut gv = dss_usermodel::GeneratorVars {
        num_phases: 3,
        ..Default::default()
    };
    let mut dr = dyn_rec(SNAPSHOT);
    let mut inst = UserModelInstance::new(
        &gen_host,
        YORDER,
        Shuttle {
            gen_vars: Some(&mut gv),
            dyn_rec: &mut dr,
            ctx: Box::new(NoCallbacks),
        },
    )
    .expect("`new` only stores the pointers, so binding itself succeeds");

    let v: Vec<Complex64> = V_PF.iter().copied().chain([Complex64::ZERO]).collect();
    let mut i = vec![Complex64::ZERO; YORDER];
    let mut dr = dyn_rec(SNAPSHOT);
    let err = inst
        .calc(
            &v,
            &mut i,
            Shuttle {
                gen_vars: Some(&mut gv),
                dyn_rec: &mut dr,
                ctx: Box::new(NoCallbacks),
            },
        )
        .expect_err("decoding 348 bytes out of a 244-byte record buffer must trap");
    let msg = err.to_string();
    assert!(
        msg.contains("wgturbine") || msg.contains("calc"),
        "the failure must name the model or the function: {msg}"
    );
}
