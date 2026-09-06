//! Unit tests for the IndMach012 construction defaults and slip handling. The
//! full power-flow/dynamics numerics are pinned by the oracle gate
//! (`exec/tests/dynamics.rs`) and `props_roundtrip`.

use super::*;

#[test]
fn create_defaults_match_pascal() {
    let m = IndMach012::new("m1");
    assert_eq!(m.cd.nphases, 3);
    assert_eq!(m.cd.nconds, 3); // delta default, no neutral
    assert_eq!(m.connection, Connection::Delta);
    assert_eq!(m.kw_base, 1000.0);
    assert_eq!(m.kv_generator_base, 12.47);
    assert_eq!(m.kva_rating, 1200.0);
    assert_eq!(m.pu_rs, 0.0053);
    assert_eq!(m.pu_xs, 0.106);
    assert_eq!(m.pu_rr, 0.007);
    assert_eq!(m.pu_xr, 0.12);
    assert_eq!(m.pu_xm, 4.0);
    assert_eq!(m.max_slip, 0.1);
    assert!(!m.fixed_slip);
    // Create sets slip 0.007 (within MaxSlip) → S2 = 2 - S1.
    assert_eq!(m.s1, 0.007);
    assert_eq!(m.s2, 2.0 - 0.007);
}

#[test]
fn set_local_slip_clamps_outside_dynamics() {
    let mut m = IndMach012::new("m1");
    m.set_local_slip(0.5); // > MaxSlip 0.1
    assert_eq!(m.s1, 0.1);
    m.set_local_slip(-0.5);
    assert_eq!(m.s1, -0.1);
    // In dynamics the slip floats freely.
    m.in_dynamics = true;
    m.set_local_slip(0.5);
    assert_eq!(m.s1, 0.5);
    assert_eq!(m.s2, 1.5);
}

#[test]
fn recalc_sets_impedances() {
    // `new` no longer recalcs (no live ctx at construction); the executive runs
    // the live recalc. Direct construction seeds it with the parse-time default.
    let mut m = IndMach012::new("m1");
    m.recalc(&SysCtx::parse_default());
    // ZBase = kV²/kVA·1000 = 12.47²/1200·1000.
    let z_base = 12.47_f64.powi(2) / 1200.0 * 1000.0;
    assert!((m.zs.re - m.pu_rs * z_base).abs() < 1e-9);
    assert!((m.zs.im - m.pu_xs * z_base).abs() < 1e-9);
    assert_eq!(m.zr.re, m.pu_rr * z_base);
    assert_eq!(m.zm.im, m.pu_xm * z_base);
    // Yeq is vars-only for power flow: -j/ZBase.
    assert!((m.yeq.im + 1.0 / z_base).abs() < 1e-12);
    assert_eq!(m.yeq.re, 0.0);
}

/// Pascal `SetNominalPower` GENERALTIME/DYNAMICMODE arm (IndMach012.pas:
/// 1091-1105): the `ActiveLoadShapeClass` (`Set LoadShapeClass=`) picks WHICH
/// of the three curves drives `ShapeFactor` in dynamics (at hr 2: daily→0.6,
/// yearly→0.7, duty→0.5); default `USENONE` leaves it 1+j1. Same family as the
/// CF2-G PVSystem dynamics load-shape fix.
#[test]
fn dynamics_loadshapeclass_selects_matching_curve() {
    use crate::elements::general::load_shape::{self, LoadShapeObj};
    use crate::obj::base::DssObject;
    use crate::obj::props::PropEngine;
    use crate::solution::{SolveMode, USEDAILY, USEDUTY, USENONE, USEYEARLY};
    use dss_parser::{Parser, ParserVars};

    /// Build a populated `LoadShapeObj` through its real property engine.
    fn build_shape(mult: &str) -> LoadShapeObj {
        let enums = EnumRegistry::new();
        let cls = load_shape::class_props(&enums);
        let mut obj = LoadShapeObj::new("s");
        let mut parser = Parser::new();
        let vars = ParserVars::new();
        let mut errors = crate::diag::ErrorLog::new();
        for (name, value) in [("npts", "4"), ("interval", "1"), ("mult", mult)] {
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

    let mut m = IndMach012::new("m1");
    m.daily_shape_obj = Some(build_shape("0.2 0.6 1.0 0.5"));
    m.yearly_shape_obj = Some(build_shape("0.3 0.7 0.9 0.4"));
    m.duty_shape_obj = Some(build_shape("0.1 0.5 0.8 0.6"));

    let dyn_ctx = |class: i32| SysCtx {
        mode: SolveMode::Dynamic,
        is_dynamic_model: true,
        active_load_shape_class: class,
        dbl_hour: 2.0,
        ..SysCtx::parse_default()
    };

    m.set_nominal_power(&dyn_ctx(USEDAILY));
    assert!(
        (m.shape_factor.re - 0.6).abs() < 1e-9,
        "daily: {}",
        m.shape_factor.re
    );
    m.set_nominal_power(&dyn_ctx(USEYEARLY));
    assert!(
        (m.shape_factor.re - 0.7).abs() < 1e-9,
        "yearly: {}",
        m.shape_factor.re
    );
    m.set_nominal_power(&dyn_ctx(USEDUTY));
    assert!(
        (m.shape_factor.re - 0.5).abs() < 1e-9,
        "duty: {}",
        m.shape_factor.re
    );
    m.set_nominal_power(&dyn_ctx(USENONE));
    assert_eq!(m.shape_factor, CDOUBLEONE, "USENONE must leave 1+j1");
}

/// Pascal `TPCElement.GetCurrents` `LastSolutionWasDirect` shortcut (PCElement.pas
/// l.137) wired into `IndMach012::get_currents`: after a direct solve the reported
/// terminal current is `YPrim·Vterminal` (frozen shadow-admittance); without the
/// flag it is the model current `YPrim·V − InjCurrent`. Guards the per-class
/// shortcut branch — IndMach012 inherits the base `GetCurrents`. (Delta default:
/// 3 conductors, no neutral.)
#[test]
fn direct_shortcut_selects_yprim_currents() {
    use crate::elements::pc::generator::default_recalc_ctx;
    use crate::elements::traits::{CktElement, SysCtx};
    use crate::support::cmatrix::CMatrix;
    use num_complex::Complex64;

    let node_v = vec![
        Complex64::ZERO, // ground slot (unused by a delta machine)
        Complex64::new(7200.0, 0.0),
        Complex64::new(-3600.0, -6235.0),
        Complex64::new(-3600.0, 6235.0),
    ];
    let inj = Complex64::new(11.0, -4.0);

    let build = || -> IndMach012 {
        let mut m = IndMach012::new("m1");
        CktElement::calc_yprim(&mut m, &default_recalc_ctx()); // sizes yorder + buffers
        let n = m.cd.yorder;
        let mut yp = CMatrix::new(n);
        for i in 0..n {
            yp.set(i, i, Complex64::new(0.01, -0.02));
        }
        m.cd.yprim = Some(yp);
        m.cd.set_node_ref(1, &[1, 2, 3]); // delta: 3 conductors
        m.cd.inj_current = vec![inj; n];
        m.cd.iterminal_solution_count = Some(0); // == solution_count → skip model recompute
        m
    };

    // Independent YPrim·Vterminal.
    let n = 3usize;
    let mut yp = CMatrix::new(n);
    for i in 0..n {
        yp.set(i, i, Complex64::new(0.01, -0.02));
    }
    let vterm: Vec<Complex64> = [1usize, 2, 3].iter().map(|&r| node_v[r]).collect();
    let mut yprim_v = vec![Complex64::ZERO; n];
    yp.mv_mult(&mut yprim_v, &vterm);

    // Direct read (flag set) → the shortcut YPrim·V.
    let mut m_d = build();
    let sys_direct = SysCtx {
        last_solution_was_direct: true,
        ncim: false,
        solution_count: 0,
        iteration: 0,
        ..default_recalc_ctx()
    };
    let mut i_d = vec![Complex64::ZERO; n];
    m_d.get_currents(&sys_direct, &node_v, &mut i_d);

    // Normal read (flag clear, model skipped, Vterminal preset) → YPrim·V − Inj.
    let mut m_n = build();
    m_n.cd.compute_vterminal(&node_v);
    let sys_normal = SysCtx {
        solution_count: 0,
        iteration: 0,
        ..default_recalc_ctx()
    };
    let mut i_n = vec![Complex64::ZERO; n];
    m_n.get_currents(&sys_normal, &node_v, &mut i_n);

    for k in 0..n {
        assert!(
            (i_d[k] - yprim_v[k]).norm() < 1e-9,
            "direct read [{k}] {} != YPrim·V {}",
            i_d[k],
            yprim_v[k]
        );
        assert!(
            (i_d[k] - i_n[k] - inj).norm() < 1e-9,
            "shortcut − model [{k}] {} != InjCurrent {}",
            i_d[k] - i_n[k],
            inj
        );
    }
    assert!(
        (i_d[0] - i_n[0]).norm() > 1.0,
        "shortcut indistinguishable from model current: {} vs {}",
        i_d[0],
        i_n[0]
    );
}

/// IndMach012 `MakePosSequence` is an EMPTY Pascal body (IndMach012.pas:1424-1426):
/// no property edits and no `inherited` call → `PosSeqPlan::no_base()`.
#[test]
fn makeposseq_indmach012_is_no_base_and_empty() {
    use crate::elements::pos_seq::PosSeqCtx;
    use crate::elements::traits::CktElement;
    let mut m = IndMach012::new("m1");
    let plan = m.make_pos_sequence(&PosSeqCtx::default());
    assert!(!plan.run_base);
    assert!(plan.actions.is_empty());
}

// ===========================================================================
// RP3.8 P1b — the LIVE read-only `pf` render
//
// r4133 renders it from the live solution:
// `5: Result := Format('%.6g', [PowerFactor(Power[1, ActiveActor])])`
// (`Version8/Source/PCElements/IndMach012.pas:1790`); `PowerFactor` is
// `Common/Utilities.pas:1821-1831` and `Power[1]` is
// `TDSSCktElement.Get_Power(1)` (`CktElement.pas:666-703`). The port answered
// the empty string only because dss_capi 0.14.5 flags the property
// `[SilentReadOnly, ReadByFunction]` and leaves its `PropertyOffset` at -1, so
// `GetObjPropertyValue`'s outer guard (`DSSObjectHelper.pas:2203-2204`)
// short-circuits before the read function runs. Every expected byte below was
// measured on the vendored EPRI r4133 DLL through `epri-worker` (RP3.8 P0
// probe, `tmp/rp38/out_r4133*.txt`); the capi oracle answers `''` on every one
// of them, which is why these pins exist and why the capi channel excludes the
// pair per-case.
// ===========================================================================

/// The P0 pin deck — also the deck of the `golden_reports` query pin, and the
/// self-contained twin of the `exec/tests/dynamics.rs` IndMach012 deck.
///
/// r4133 on it (measured): `? indmach012.m1.pf` = `'0.908343'`, state variable
/// "Power Factor" = `0.9083426903391937`.
fn pin_deck() -> crate::exec::Dss {
    deck(true)
}

/// The pin deck, optionally stopping at `calcvoltagebases`. Without the `solve`
/// the machine's `Iterminal` cache is NOT stamped, which is the state in which
/// answering `pf` has to recompute the machine model — see
/// [`pf_is_a_pure_read`].
fn deck(solve: bool) -> crate::exec::Dss {
    let mut dss = crate::exec::Dss::new();
    for c in [
        "Set DefaultBaseFrequency=60",
        "New Circuit.indtest basekv=12.47 pu=1.0 phases=3 bus1=src \
         mvasc3=20000 mvasc1=21000",
        "New Transformer.tg phases=3 windings=2 buses=(src, mbus) \
         conns=(delta,wye) kvs=(12.47,0.48) kvas=(1500,1500) xhl=5",
        "New Capacitor.cg conn=wye bus1=mbus phases=3 kvar=600 kv=0.48",
        "New IndMach012.m1 bus1=mbus kV=0.48 kW=1200 conn=delta kVA=1500 H=6 \
         puRs=0.048 puXs=0.075 puRr=0.018 puXr=0.12 puXm=3.8 slip=0.02 \
         SlipOption=variableslip",
        "set voltagebases=[12.47, 0.48]",
        "calcv",
    ] {
        dss.command(c);
    }
    if solve {
        dss.command("solve");
    }
    assert!(dss.errors().is_empty(), "{:?}", dss.error_texts());
    dss
}

/// `? <element>.<prop>` through the executive's own query path — the same
/// `refresh_vterminal_if_marked` + `get_value` pair `Dump` and
/// `element_properties` use.
fn query(dss: &mut crate::exec::Dss, what: &str) -> String {
    dss.command(&format!("? {what}"));
    dss.result().to_string()
}

// RP3.8 EXPECTED-VALUE PIN [INDMACH012_PF_RENDERS_LIVE]: `pf` renders the live power
// factor of terminal 1, in both lanes. A revert to the dss_capi 0.14.5
// empty-string suppression (dropping `RENDERS_LIVE_RESULT`, or re-widening the
// `SILENT_READ_ONLY` gate in `class_props/value.rs`) fails every assertion here
// on an empty string.
/// `pf` on a solved machine is `PowerFactor(Power[1])` — r4133 `'0.908343'`.
///
/// The port renders **full precision**, like every other double property
/// (`float_to_str_ex`, the capi 15-digit convention the whole property surface
/// uses), not r4133's 6 significant digits: the r4133 property channel absorbs
/// that difference through the measured display floor (`R4133_DISPLAY_FLOOR`,
/// `harness/props_norm.rs`) — this cell's gap is rel 4.3e-7, three orders under
/// it — and emitting `%.6g` from the engine would put a lossy string on
/// `Dump`/`Save`/export where every sibling is exact. So the cell is pinned two
/// ways: r4133's own bytes via `fmt_g(v, 6)`, and the full-precision spelling
/// via the `float_to_str_ex` round-trip.
///
/// It is also pinned against state variable #21, which r4133 computes with the
/// very same expression (`IndMach012.pas:1988`) — the property and the variable
/// can never disagree.
#[test]
fn pf_renders_the_live_power_factor() {
    use crate::util::{float_to_str_ex, fmt_g};
    let mut dss = pin_deck();

    let rendered = query(&mut dss, "indmach012.m1.pf");
    let v: f64 = rendered.parse().expect("pf renders a number");
    assert_eq!(fmt_g(v, 6), "0.908343", "pf at r4133's own precision");
    assert_eq!(rendered, float_to_str_ex(v), "pf renders at full precision");

    // ...and it is state variable #21 ("Power Factor"), the same expression.
    let vars = dss
        .element_variables("IndMach012.m1")
        .expect("the machine has state variables");
    assert_eq!(v, vars[20], "pf must equal state variable #21");

    // Non-vacuous: the machine really carries power (r4133's own terminal-1
    // reading on this deck is 3 conductors of ~400.23 kW + ~184.28 kvar).
    let s = dss.snapshot_elements();
    let e = s
        .iter()
        .find(|e| e.name.eq_ignore_ascii_case("IndMach012.m1"))
        .expect("the machine is in the circuit");
    let p: f64 = e.powers.iter().map(|c| c.re).sum();
    let q: f64 = e.powers.iter().map(|c| c.im).sum();
    assert!((p - 1200.6867).abs() < 1e-3, "terminal-1 P {p}");
    assert!((q - 552.8301).abs() < 1e-3, "terminal-1 Q {q}");
}

// RP3.8 EXPECTED-VALUE PIN [INDMACH012_PF_IS_A_PURE_READ]: reading `pf` changes
// nothing observable, in both lanes.
/// **Reading `pf` is a pure read of the model.**
///
/// The render has to run the machine's own `GetTerminalCurrents`, and while the
/// `Iterminal` cache is not stamped that recompute is *stateful*: `CalcPFlow`
/// advances the fixed-slope slip-Newton by one step and rewrites the sequence
/// currents (see `do_indmach_model`). r4133 keeps the advance — reading `pf`
/// moves the machine there — and the port does not reproduce it (the recompute
/// runs on a throwaway clone, [`IndMach012::refresh_live_pf`]).
///
/// The tripwire is `Slip`: it renders through the plain `&self` getter with no
/// refresh of its own, so a `?` query on it is itself non-perturbing. Without
/// the clone this test fails on the very first `pf` read — measured, and the
/// same regression moved `Slip` from `0.007` to `0.006947528894572309` in the
/// `spectrum_refs` JSON golden.
#[test]
fn pf_is_a_pure_read() {
    // (1) The stateful state: `calcvoltagebases` only, no solve.
    let mut dss = deck(false);
    assert_eq!(
        query(&mut dss, "indmach012.m1.slip"),
        "0.02",
        "the deck's own slip, before any power read"
    );
    // The port's own value on this deck (r4133 was not run on this exact
    // preamble); its `%.6g` is `0.909167`, which is what r4133 renders on the
    // corpus twin `asymmetric/indmach/indmach_asym.dss` — the same machine and
    // circuit head, also read after `calcv` alone (RP3.8 P0 probe). It is
    // asserted here only to prove the recompute really ran.
    let pf = query(&mut dss, "indmach012.m1.pf");
    assert_eq!(pf, "0.909167177168331", "the recompute did not run");
    assert_eq!(
        query(&mut dss, "indmach012.m1.slip"),
        "0.02",
        "a `pf` read advanced the slip-Newton"
    );
    assert_eq!(
        query(&mut dss, "indmach012.m1.pf"),
        pf,
        "two consecutive reads must agree"
    );

    // (2) The solved state: the whole reported model is bit-identical whether or
    // not `pf` was read first.
    fn state(dss: &mut crate::exec::Dss) -> (Vec<f64>, Vec<(f64, f64)>) {
        let vars = dss.element_variables("IndMach012.m1").expect("variables");
        let snap = dss.snapshot_elements();
        let e = snap
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case("IndMach012.m1"))
            .expect("the machine is in the circuit");
        (vars, e.powers.iter().map(|c| (c.re, c.im)).collect())
    }

    // A: the deck, never asked for `pf`. B: the same deck, asked twice first.
    let mut a = pin_deck();
    let untouched = state(&mut a);
    let mut b = pin_deck();
    let first = query(&mut b, "indmach012.m1.pf");
    let second = query(&mut b, "indmach012.m1.pf");
    let after_read = state(&mut b);

    assert_eq!(first, second, "two consecutive reads must agree");
    assert_eq!(first, "0.908342690386879", "the reads were not vacuous");
    assert_eq!(
        untouched.0, after_read.0,
        "a `pf` read moved a state variable"
    );
    assert_eq!(untouched.1, after_read.1, "a `pf` read moved the powers");

    // ...and every other rendered property of the machine is unmoved too.
    let before = b.element_properties("IndMach012.m1").expect("properties");
    query(&mut b, "indmach012.m1.pf");
    let after = b.element_properties("IndMach012.m1").expect("properties");
    assert_eq!(before, after, "a `pf` read moved a rendered property");
}

// RP3.8 EXPECTED-VALUE PIN [INDMACH012_PF_WITHOUT_POWER_IS_UNITY]: a machine that
// carries no power renders `1`, in both lanes — and never the upstream crash
// this query raises before the node references are assigned.
/// `PowerFactor(S = 0)` is `1.0` (`Utilities.pas:1821-1831`, the
/// `Else Result := 1.0` arm), so an unconnected, unsolved or disabled machine
/// renders `1`.
///
/// Measured on r4133 over all five `tests/golden/props/indmach012.json`
/// scenarios: after `calcvoltagebases` it answers `'1'` — the value pinned here.
/// With no `calcv` at all r4133 instead **access-violates** inside `Get_Power`
/// (`DSS error #641 ... Read of address 0000000000000000` — the getter guards
/// only on `FEnabled`, `CktElement.pas:679`, never on `NodeRef = nil`); that
/// crash is deliberately not reproduced, because
/// `CktElement::terminal_power` returns zero for an unassigned `node_ref`
/// (`elements/traits.rs:882-884`), so the port answers `1` there too — the
/// first leg below. Reported upstream as
/// `investigations/to_opendss/48-indmach012-getpropertyvalue-pf-access-violation.md`.
///
/// The third r4133 spelling — `'NAN'` after a `solve` that leaves the machine's
/// bus unenergized — is NOT a divergence and is not pinned here: the machine's
/// own state goes NaN in that solve on both engines (`slip` reads NaN too), and
/// the port renders the same NaN as `----` (`float_to_str_ex`'s NaN spelling)
/// where r4133's `Format('%.6g')` prints `NAN`. Measured 2026-09-02; it is a
/// render convention over identical state, not a value the engine chooses.
#[test]
fn pf_of_a_machine_without_power_is_unity() {
    // The `props_roundtrip` scenario shape (no calcv, no solve) and the same
    // circuit after `calcv` — r4133 crashes on the first, answers `'1'` on the
    // second, and the port answers `1` on both.
    let mut dss = crate::exec::Dss::new();
    for c in ["new circuit.propsprobe", "New IndMach012.m1 bus1=b"] {
        dss.command(c);
    }
    assert_eq!(query(&mut dss, "indmach012.m1.pf"), "1", "bare");
    dss.command("calcv");
    assert_eq!(query(&mut dss, "indmach012.m1.pf"), "1", "after calcv");

    // A disabled machine: r4133's `Get_Power` returns 0 for `not FEnabled`
    // (`CktElement.pas:679`), so `PowerFactor(0)` = 1 there too.
    let mut dss = pin_deck();
    assert_ne!(query(&mut dss, "indmach012.m1.pf"), "1", "energized first");
    dss.command("edit indmach012.m1 enabled=no");
    assert_eq!(query(&mut dss, "indmach012.m1.pf"), "1", "disabled");
}

/// The three JSON/schema surfaces the 0.14.5 `SilentReadOnly` convention still
/// governs are **unchanged** by RP3.8 — the text render is the only one r4133
/// disagrees with. Nothing here may move a JSON golden
/// (`tests/golden/json/spectrum_refs.json` pins the same omission for this very
/// class, and `schema_full_oracle.json` is the capi schema).
#[test]
fn pf_stays_out_of_the_json_surfaces() {
    use crate::report::export::json::JsonOpts;
    let mut dss = pin_deck();

    // 1. The FULL JSON export omits `PF` (`class_props/json.rs`), while its
    //    writable siblings are present — so the omission is the flag's doing.
    let json = dss
        .obj_to_json_mut("IndMach012.m1", JsonOpts::FULL)
        .expect("the machine exports");
    assert!(
        !json.contains("\"PF\""),
        "PF must stay out of the JSON export: {json}"
    );
    assert!(json.contains("puXm"), "the FULL sweep is not empty: {json}");

    // 2. The schema still marks it read-only
    //    (`report/export/json/schema/classes.rs`, keyed on SILENT_READ_ONLY).
    let schema = format!(
        "{:?}",
        dss.schema_class_def("IndMach012").expect("class def")
    );
    let at = schema.find("\"PF\"").expect("PF in the schema");
    let window = &schema[at..(at + 400).min(schema.len())];
    assert!(
        window.contains("readOnly"),
        "PF must stay readOnly in the schema: {window}"
    );
}

/// A JSON load still **ignores** `PF` (`class_props/json_set.rs`, keyed on
/// `SILENT_READ_ONLY`): a bogus value in the object body is dropped, while a
/// writable sibling in the same body applies — so the test cannot pass by the
/// walk aborting early.
#[test]
fn pf_is_ignored_by_a_json_load() {
    use crate::obj::base::DssObject;
    use crate::obj::props::PropEngine;
    use crate::report::export::json::Json;
    use dss_parser::{Parser, ParserVars};

    let enums = EnumRegistry::new();
    let cls = class_props(&enums);
    let mut m = IndMach012::new("m1");
    let mut parser = Parser::new();
    let vars = ParserVars::new();
    let mut errors = crate::diag::ErrorLog::new();
    let mut eng = PropEngine {
        parser: &mut parser,
        vars: &vars,
        enums: &enums,
        errors: &mut errors,
        foreign: None,
        was_quoted: false,
    };
    // The class's four `Required` properties must all be present, or the walk
    // aborts on the first missing one (`fill_from_json`, DSSObjectHelper 5021)
    // and the test would pass vacuously.
    let members: Vec<(String, Json)> = vec![
        ("Bus1".into(), Json::Str("b".into())),
        ("kV".into(), Json::Float(0.48)),
        ("kW".into(), Json::Float(500.0)),
        ("kVA".into(), Json::Float(1234.0)),
        ("PF".into(), Json::Float(0.5)),
    ];
    cls.fill_from_json(&mut m, &members, &mut eng);
    assert!(errors.is_empty(), "{:?}", errors);

    // The writable sibling landed: the walk really reached the read-only key
    // (`PF` sits between `kW` and `kVA` in the property order).
    assert_eq!(m.kva_rating, 1234.0);
    // ...and `PF` was not stored: the render cache is untouched, so a read still
    // reports `PowerFactor(0)` = 1.
    assert_eq!(m.live_pf, 1.0);
    assert_eq!(m.get_f64(prop::PF), 1.0);

    // …and the walk never even MARKED it set. This is the leg that actually
    // discriminates the flag: `PF` has no `set_f64` arm, so "nothing was stored"
    // holds with or without `SILENT_READ_ONLY` and the two assertions above pass
    // either way (RP3.8 audit-tests finding 1). Without the flag the key reaches
    // `set_json_value` → `edit_property`, whose applier VM stamps `PrpSequence`
    // (Pascal `SetAsNextSeq`) — and a marked property is one `Save` emits, so
    // the load would leak into the saved deck. The writable sibling proves the
    // stamp does happen on this very walk.
    let stamped: Vec<usize> = std::iter::successors(m.data().next_property_set(None), |&i| {
        m.data().next_property_set(Some(i))
    })
    .collect();
    assert!(
        stamped.contains(&prop::KVA),
        "the writable sibling must be stamped, or this leg is vacuous: {stamped:?}"
    );
    assert!(
        !stamped.contains(&prop::PF),
        "PF was stamped set by a JSON load — `Save` would emit it: {stamped:?}"
    );
}

/// The flag pair is carried by exactly `PF`, and the property table's shape
/// (names, order, count) is untouched — RP3.8 changes what a render *says*,
/// never the property list the oracle compares against.
#[test]
fn only_pf_renders_live() {
    use crate::obj::props::PropFlags;
    let enums = EnumRegistry::new();
    let cp = class_props(&enums);
    assert_eq!(cp.num_properties(), prop::NUM_PROPS);

    let live: Vec<&str> = (1..=cp.num_properties())
        .filter(|&i| cp.prop(i).flags.contains(PropFlags::RENDERS_LIVE_RESULT))
        .map(|i| cp.property_name(i))
        .collect();
    assert_eq!(live, ["PF"]);
    // Both flags, together: the render is live, the JSON/schema convention holds.
    assert!(
        cp.prop(prop::PF)
            .flags
            .contains(PropFlags::SILENT_READ_ONLY)
    );
    assert!(
        cp.prop(prop::PF)
            .flags
            .contains(PropFlags::RENDERS_LIVE_RESULT)
    );
    // `Slip` is the class's other function-backed double (`WriteByFunction`) and
    // must not drift into the set.
    assert!(
        !cp.prop(prop::SLIP)
            .flags
            .contains(PropFlags::RENDERS_LIVE_RESULT)
    );
}
