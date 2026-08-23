//! WindGen `UserModel`/`UserData` — EPRI r4133 properties 18/19 and the
//! `Model=6` behaviour they feed (`R4133_PROPS_PLAN.md` RP1.3).
//!
//! r4133 registers `PropertyName^[18] := 'UserModel'` / `[19] := 'UserData'`
//! (`Version8/Source/PCElements/WindGen.pas:391-395`, Edit arms `:641-642`),
//! answers 18 with `UserModel.Name` (`:2903`) and lets 19 fall through to the
//! inherited `FPropertyValue` echo (`:2929-2930`); `Model=6` then routes the
//! power flow through `DoUserModel` (`:1875-1898`), the dynamics through
//! `DoDynamicMode`'s GenModel-6 arm (`:1991-1998`), `InitStateVars` (`:2566-2570`)
//! and `IntegrateStates` (`:2662-2669`), and grows the state-variable surface by
//! the model's own variables (`:2793-2882`).
//!
//! **There is no oracle witness for any of it.** WindGen does not exist in the
//! pinned dss_capi 0.14.5 at all, and r4133 ships only the *loader*
//! (`PCElements/WindGenUserModel.pas`) — no native WindGen user model exists
//! anywhere upstream, so the epri-worker bridge cannot be driven through this
//! path the way `wasm_usermodels.rs` drives the Generator's (WM.3, where an FPC
//! `.dll` twin of the wasm fixture exists). The acceptance carrier for RP1.3 is
//! therefore this module plus `crates/dss-usermodel/tests/windgen_shuttle.rs`:
//! every number below is derived from the **committed guest fixture's own
//! documented law** (`tools/wasm_usermodel/models/wgturbine/src/lib.rs`) and from
//! the engine's own solved node voltages — never captured from a Rust run.
//!
//! The fixture is a constant-admittance source in power flow and a first-order
//! current/speed lag in dynamics, and it fills the `TWindGenVars` turbine tail
//! (`WindGenVars.pas:61-72`) from the terminal power:
//!
//! ```text
//! I[k]  = (G + jB)·V[k]          (power flow; dynamics reports the lagged state)
//! Pg    = Σ_k Re(V[k]·conj(I[k]))        Ps = Pg·(1 − slip)     Pr = Pg·slip
//! Pm    = Pg / eta      s = slip     Cp = Pg/(kVArating·1000)   Lamda = ag·(1+slip)
//! ```
//!
//! Every assertion here is that law re-evaluated in the test, so a broken
//! shuttle, a wrong record offset, a dropped write-back or a mis-routed variable
//! index fails on the cell that carries it.
//!
//! # Non-vacuity (§1.1(f)) — measured, 2026-08-23
//!
//! Thirteen mutations were applied in-tree, run, and reverted; each one turns the
//! tests that own it red (`cargo test -p dss-core --lib windgen_usermodel`). The
//! `Get_`/`Set_Variable` row was **split** by the audit settlement: the write
//! half used to be unpinned (reproducing it left all tests green, because the
//! fixture silently ignored the stray index), and the last four rows are the
//! settlement's new pins.
//!
//! | mutation | red |
//! |---|---|
//! | drop the turbine-tail write-back in `apply_wind_gen_vars` | 5 (tail, surface, StateVar, second-assign, `like=`) |
//! | reproduce the upstream **`Get_Variable`** mis-nesting (native block ← `FGetVariable(k ≤ 0)`) | 8, incl. the dedicated mis-nesting pin |
//! | reproduce the upstream **`Set_Variable`** mis-nesting (native write → `FSetVariable(i − 22)`) | 1 — `set_statevar_routes_only_the_tail_to_the_model` |
//! | dispatch `Model=6` into `do_constant_pq_gen` | 8, incl. the admittance law and #567 |
//! | never call `user_model_fintegrate` (WTG3 integrator instead) | 1 — the first-order-lag pin, alone |
//! | make `do_dynamic_mode` fall through to `CalcDynamic` for model 6 | 4 (both dynamics pins, the 1-phase arm, #5671) |
//! | restore the pre-fix clone-the-slot `make_like` | 1 — the `like=` pin, on the surface length |
//! | drop `conn` from `wind_gen_vars_from` (`conn: 0`) | 1 — `a_delta_windgen_echoes_its_own_connection` |
//! | delete the `update_user_models` call in `nominal.rs` | 1 — `recalc_element_data_updates_the_bound_model` |
//! | take `xdp` from the wrong element field (`xdp: g.xd`) | 1 — `the_record_fields_come_from_the_elements_own_sources` |
//! | route `GetAllVariables`' DynamicEq arm past the user-model tail | 1 — `a_dynamic_eq_windgen_still_reports_the_models_variables` |
//! | form `Edp` from the WTG3 `Zthev` instead of the record's | 1 — the same test, on the seeded `theta` |
//! | drop the `ensure_live` revive from `take_live_user_model` | 1 — `an_element_snapshot_revives_its_user_model` |

use num_complex::Complex64;

use crate::exec::Dss;

use super::common::query;

/// The committed guest fixture (`tools/wasm_usermodel/build_wgturbine_wasm.ps1`,
/// sha256 pinned in `tools/wasm_usermodel/PIN.txt`), as an absolute forward-slash
/// path so the §2.4 activation rule resolves it from any working directory.
fn fixture() -> String {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests/fixtures/wasm/wgturbine.wasm")
        .canonicalize()
        .expect("the committed wgturbine.wasm fixture must exist")
        .to_string_lossy()
        .trim_start_matches(r"\\?\")
        .replace('\\', "/")
}

// The `UserData=` operating point every deck below binds. **Every one of the five
// is off the fixture's own default** ([`DEFAULT_G`] and friends), so no cell
// asserted anywhere in this module could read correctly with the `UserData=`
// forwarding deleted — the model would answer with its defaults instead.
const G: f64 = 0.0021;
const B: f64 = -0.0006;
const SLIP: f64 = 0.035;
const ETA: f64 = 0.92;
const TAU: f64 = 0.04;

// The fixture's own `Model::default()` (`wgturbine/src/lib.rs`) — what a freshly
// `FNew`ed instance carries before any `Edit`.
const DEFAULT_G: f64 = 0.0015;
const DEFAULT_B: f64 = -0.0004;
const DEFAULT_SLIP: f64 = 0.02;

/// The guest's `UserData=` string for the constants above.
fn user_data() -> String {
    format!("g={G} b={B} slip={SLIP} eta={ETA} tau={TAU}")
}

/// The fixture's shunt admittance, `G + jB`.
const fn y() -> Complex64 {
    Complex64::new(G, B)
}

/// `kVArating` every deck pins (the `kva=` below), volt-amps.
const KVA_W: f64 = 1800.0 * 1000.0;

/// `TWindGenVars.ag` — the WindGen `Create` default (`WindGen.pas:975`), which no
/// deck here overrides. The guest READS it out of the turbine tail at offset 244,
/// so `Lamda` is the end-to-end witness for the ABI §2.6 layout decision (the
/// managed `PLoss` string does not cross and its hole is closed).
const AG: f64 = 1.0 / 90.0;

/// The 22 classic WindGen state-variable names, in order
/// (`WindGen.pas:2828-2856`, ported at `windgen/dynamics.rs`).
const NATIVE_VARS: [&str; 22] = [
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

/// The guest's own fifteen variables, in order (`wgturbine`'s `var_name`). None
/// of them can collide with a native name, so a mis-ordered concatenation is
/// visible by name alone.
const MODEL_VARS: [&str; 15] = [
    "WgIout1",
    "WgPg",
    "WgSlip",
    "WgKvaEcho",
    "WgKvBaseEcho",
    "WgW0Echo",
    "WgNphEcho",
    "WgNcondEcho",
    "WgConnEcho",
    "WgUpdCount",
    "WgBadSet",
    "WgXdpEcho",
    "WgVTargetEcho",
    "WgPolesEcho",
    "WgVCutInEcho",
];

/// Length of the bound surface: the 22 native variables ++ the guest's 15.
const BOUND_VARS: usize = NATIVE_VARS.len() + MODEL_VARS.len();

/// The value Pascal returns for an out-of-range state variable
/// (`WindGen.pas:2730`) and the fixture mirrors — the sentinel that would flood
/// the native block if the port reproduced the upstream `Get_Variable`
/// mis-nesting (see [`the_native_variables_stay_native_while_a_model_is_bound`]).
const VAR_OUT_OF_RANGE: f64 = -9999.99;

/// Feed a script, asserting every line is accepted (unless `expect_errors`).
fn feed(lines: &[String], expect_errors: bool) -> Dss {
    let mut dss = Dss::new();
    for line in lines {
        dss.command(line);
        if !expect_errors {
            assert!(dss.errors().is_empty(), "`{line}` -> {:?}", dss.errors());
        }
    }
    dss
}

/// The shared deck: a stiff 0.69 kV source, a short line and one 3-phase WindGen
/// on `wbus`.
///
/// `tolerance=1e-12` is deliberate. The terminal current a query reports is the
/// one the model returned during the **last injection iteration**
/// (`accessors.rs::get_currents` honours the `IterminalSolutionCount` cache, as
/// Pascal does), i.e. it belongs to `NodeV_{n-1}`; the convergence tolerance is
/// therefore exactly the band between it and `Y·NodeV_n`. At 1e-12 the deck
/// converges in 11 iterations and the band closes to ~5e-13 relative, so the
/// admittance law can be pinned as a law instead of as a captured number.
fn deck(model: &str, model_line_tail: &str) -> Vec<String> {
    vec![
        "Clear".to_string(),
        "New Circuit.wgum basekv=0.69 phases=3 bus1=srcbus".to_string(),
        "New Line.l1 bus1=srcbus bus2=wbus phases=3 r1=0.005 x1=0.02 length=1".to_string(),
        format!(
            "New WindGen.w1 bus1=wbus phases=3 kv=0.69 kW=1500 kva=1800 conn=wye \
             model={model} vwind=12{model_line_tail}"
        ),
        "Set voltagebases=[0.69]".to_string(),
        "Calcvoltagebases".to_string(),
        "Set tolerance=1e-12".to_string(),
        "Solve".to_string(),
    ]
}

/// The `UserModel=`/`UserData=` tail for [`deck`].
fn bound_tail() -> String {
    format!(" UserModel={} UserData=({})", fixture(), user_data())
}

/// A solved snapshot deck with the fixture bound as `Model=6`.
fn bound_snapshot() -> Dss {
    feed(&deck("6", &bound_tail()), false)
}

/// The three phase voltages of `wbus`, in phase order, off the solved solution.
fn wbus_voltages(dss: &mut Dss) -> [Complex64; 3] {
    let ckt = dss.circuit().expect("solved circuit");
    let mut v = [Complex64::ZERO; 3];
    for j in 1..=ckt.num_nodes {
        let name = ckt.node_name(j).to_ascii_uppercase();
        for (k, want) in ["WBUS.1", "WBUS.2", "WBUS.3"].iter().enumerate() {
            if name == *want {
                v[k] = ckt.solution.node_v[j];
            }
        }
    }
    v
}

/// A WindGen's terminal currents (all conductors, the neutral included).
fn currents_of(dss: &mut Dss, elem: &str) -> Vec<Complex64> {
    dss.snapshot_elements()
        .into_iter()
        .find(|s| s.name.eq_ignore_ascii_case(elem))
        .unwrap_or_else(|| panic!("{elem} is a circuit element"))
        .currents
}

/// `WindGen.w1`'s terminal currents.
fn w1_currents(dss: &mut Dss) -> Vec<Complex64> {
    currents_of(dss, "WindGen.w1")
}

/// `(names, values)` of a WindGen's state-variable surface.
fn variables_of(dss: &mut Dss, elem: &str) -> (Vec<String>, Vec<f64>) {
    let names = dss
        .element_variable_names(elem)
        .unwrap_or_else(|| panic!("{elem} has a variable surface"));
    let values = dss
        .element_variables(elem)
        .unwrap_or_else(|| panic!("{elem} has variable values"));
    assert_eq!(names.len(), values.len(), "name/value length mismatch");
    (names, values)
}

/// `(names, values)` of `WindGen.w1`'s state-variable surface.
fn w1_variables(dss: &mut Dss) -> (Vec<String>, Vec<f64>) {
    variables_of(dss, "WindGen.w1")
}

/// Assert `actual` matches `expected` to `rel`, naming the cell.
fn close(label: &str, actual: f64, expected: f64, rel: f64) {
    let gap = (actual - expected).abs();
    assert!(
        gap <= rel * expected.abs(),
        "{label}: {actual:.17e} vs {expected:.17e} (|Δ| {gap:.3e} > {rel:.0e} rel)"
    );
}

/// Pascal `DoUserModel` (`WindGen.pas:1875-1898`): `CalcYPrimContribution` seeds
/// `InjCurrent`, `UserModel.FCalc(Vterminal, Iterminal)` returns the terminal
/// currents and they are negated into the injection. The fixture's power-flow law
/// is `I[k] = (G + jB)·V[k]` over `NumPhases` phases, so the reported terminal
/// current must be the admittance current at the solved node voltages — a value
/// no built-in WindGen model can produce (models 1/2/4/5 all derive their current
/// from the aerodynamic `Pnominalperphase`/`Qnominalperphase`, which
/// [`a_bound_model_is_dormant_on_the_built_in_models`] shows is ~2000× larger
/// here).
///
/// The neutral conductor is asserted **exactly** zero: the guest writes only its
/// own three phases and the host copies the whole `Iterminal` buffer back, so a
/// host that over-wrote the tail (or mis-sized the buffer) shows up here.
#[test]
fn power_flow_currents_are_the_guests_admittance_law() {
    let mut dss = bound_snapshot();
    let v = wbus_voltages(&mut dss);
    let i = w1_currents(&mut dss);
    assert_eq!(i.len(), 4, "wye 3-phase WindGen has 4 conductors");
    for k in 0..3 {
        let want = y() * v[k];
        let gap = (i[k] - want).norm() / want.norm();
        assert!(
            gap <= 1e-11,
            "phase {k}: I = {:.17e}{:+.17e}j vs (G+jB)·V = {:.17e}{:+.17e}j (rel {gap:.3e})",
            i[k].re,
            i[k].im,
            want.re,
            want.im
        );
    }
    assert_eq!(
        i[3],
        Complex64::ZERO,
        "the guest fills only its NumPhases entries; the neutral slot must survive untouched"
    );
}

/// The `TWindGenVars` turbine tail the guest writes (`Pg`/`Ps`/`Pr`/`Pm`/`s`/
/// `Cp`/`Lamda` plus the `Pshaft` head) must land back **on the element**: the
/// native DLL shares the record through a retained pointer
/// (`WindGen.pas:992-995`), and the wasm host reproduces that by copying the full
/// image back (`user_model.rs::apply_wind_gen_vars`, ABI doc §2).
///
/// Read through two independent surfaces, so a write-back that reached one but
/// not the other cannot pass: `Pg`/`Ps`/`Pr`/`s` off the state-variable surface
/// (native indices 19-22, `WindGen.pas:2856-2870`) and `Cp`/`Lamda` off the `?`
/// property surface (ordinals 37/38).
///
/// `Lamda = ag·(1 + slip)` is the load-bearing one: `ag` is the first
/// **turbine-tail** field, which the guest reads at image offset 244 — an offset
/// that is only correct because the managed `PLoss: string` does not cross and
/// its native hole is closed (ABI §2.6b). A host that padded instead would feed
/// the guest garbage here.
#[test]
fn the_turbine_tail_crosses_back_onto_the_element() {
    let mut dss = bound_snapshot();
    let v = wbus_voltages(&mut dss);
    let i = w1_currents(&mut dss);
    // The guest's own Pg, recomputed from the engine's solved voltages and the
    // currents it returned: Pg = Σ Re(V·conj(I)).
    let pg: f64 = (0..3).map(|k| (v[k] * i[k].conj()).re).sum();

    let (names, values) = w1_variables(&mut dss);
    assert_eq!(names[18], "Pg");
    assert_eq!(names[19], "Ps");
    assert_eq!(names[20], "Pr");
    assert_eq!(names[21], "s");
    // The 1e-11 band is the last-injection lag of `Iterminal` (see `deck`), not a
    // model tolerance: Ps/Pr/s below are pinned EXACTLY against the reported Pg.
    close("Pg", values[18], pg, 1e-11);
    assert_eq!(values[19], values[18] * (1.0 - SLIP), "Ps = Pg·(1 − slip)");
    assert_eq!(values[20], values[18] * SLIP, "Pr = Pg·slip");
    assert_eq!(values[21], SLIP, "s = slip");

    // The `?` render round-trips ~15 significant digits, hence the 1e-12 band on
    // these two (the values themselves are exact functions of Pg / ag).
    let cp: f64 = query(&mut dss, "WindGen.w1.Cp").parse().expect("Cp");
    close("Cp = Pg/(kVArating·1000)", cp, values[18] / KVA_W, 1e-12);
    let lamda: f64 = query(&mut dss, "WindGen.w1.Lamda").parse().expect("Lamda");
    close("Lamda = ag·(1 + slip)", lamda, AG * (1.0 + SLIP), 1e-12);
}

/// Pascal `NumVariables`/`VariableName`/`GetAllVariables` (`WindGen.pas:2793-2819`,
/// `:2821-2882`): the surface is the 22 classic variables **followed by** the
/// user model's, addressed `i2 = i − NumWGenVariables`.
///
/// The six `…Echo` variables are the record-head witnesses — the guest reports
/// what it decoded out of the `TWindGenVars` image it was handed, spanning the
/// leading doubles (`kVArating`, `kVWindGenBase`, `w0`) and the integer block
/// (`NumPhases`/`NumConductors`/`Conn`). They are pinned against the deck's own
/// declarations, so any field slipping by one slot in the 348-byte image fails
/// here rather than silently perturbing a current.
#[test]
fn the_variable_surface_is_the_native_22_then_the_models_own() {
    let mut dss = bound_snapshot();
    let (names, values) = w1_variables(&mut dss);
    assert_eq!(
        names.len(),
        NATIVE_VARS.len() + MODEL_VARS.len(),
        "22 native ++ 15 model variables, got {names:?}"
    );
    for (k, want) in NATIVE_VARS.iter().chain(MODEL_VARS.iter()).enumerate() {
        assert_eq!(&names[k], want, "variable {} name", k + 1);
    }

    // The model's own three outputs, cross-checked against the native block the
    // same guest call filled.
    let i = w1_currents(&mut dss);
    close("WgIout1 = |I[0]|", values[22], i[0].norm(), 1e-11);
    assert_eq!(values[23], values[18], "WgPg mirrors the record's Pg");
    assert_eq!(values[24], SLIP, "WgSlip is the UserData= slip");

    // The record head, exactly as the deck declares it.
    assert_eq!(values[25], 1800.0, "WgKvaEcho = kVArating");
    assert_eq!(values[26], 0.69, "WgKvBaseEcho = kVWindGenBase");
    assert_eq!(
        values[27],
        2.0 * std::f64::consts::PI * 60.0,
        "WgW0Echo = w0 = 2π·BaseFreq"
    );
    assert_eq!(values[28], 3.0, "WgNphEcho = NumPhases");
    assert_eq!(values[29], 4.0, "WgNcondEcho = NumConductors (wye 3-phase)");
    assert_eq!(values[30], 0.0, "WgConnEcho = Conn (0 = wye)");
}

/// **Upstream bug, deliberately not reproduced** (CLAUDE.md — never, in any
/// lane): `TWindGenObj.Get_Variable` (`WindGen.pas:2735-2743`) and `Set_Variable`
/// (`:2777-2784`) put the user-model tail OUTSIDE the `if i < 19 … else case i of
/// …` chain instead of inside its `else` — contrast `generator.pas:2868-2884`,
/// where the identical block sits in the `Else`, and WindGen's own correct
/// `VariableName` (`:2856-2870`). With a model bound, every native index `1..=22`
/// therefore also satisfies `k = i − 22 ≤ N` and is **overwritten** by
/// `UserModel.FGetVariable(k)` with a non-positive `k`; `GetAllVariables`
/// (`:2799`) reads through it, so `Show Variables`, `Dump … debug`,
/// `AllVariableValues`, `Get StateVar` and monitor mode 3 all see it
/// (`investigations/to_opendss/38-windgen-get-set-variable-usermodel-tail.md`).
///
/// The fixture answers a non-positive index with Pascal's own out-of-range
/// sentinel −9999.99, so this test is the direct discriminator: under the
/// upstream nesting **all 22** native cells would read −9999.99. They must
/// instead carry their native sources — vars 19-22 the record's turbine outputs,
/// vars 3/10 the values `SetNominalGeneration` and the `vwind=` property left.
#[test]
fn the_native_variables_stay_native_while_a_model_is_bound() {
    let mut dss = bound_snapshot();
    let (names, values) = w1_variables(&mut dss);
    assert_eq!(values.len(), BOUND_VARS, "the model must really be bound");
    for k in 0..22 {
        assert_ne!(
            values[k],
            VAR_OUT_OF_RANGE,
            "native variable {} ({}) reads the user model's out-of-range sentinel — \
             the upstream Get_Variable mis-nesting is being reproduced",
            k + 1,
            names[k]
        );
    }
    assert!(values[18] > 0.0, "Pg must be the record's turbine output");
    assert_eq!(values[9], 12.0, "vwind is the property, not a model read");
}

/// `Set StateVar` / `Get StateVar` route the tail — and **only** the tail — to
/// the model (`WindGen.pas:2777-2784`, `k = i − NumWGenVariables`), the same
/// mis-nesting fix as the read side.
///
/// `set StateVar=x …` is the form that reaches the arm: the natural
/// `set StateVar <elem> <var> <value>` text syntax is an upstream positional-parse
/// quirk that never routes there (pinned by
/// `exec::tests::force_hooks::set_state_var_text_syntax_is_upstream_broken`).
///
/// Two directions, both discriminating: writing `WgSlip` (index 25 = 22 + 3, the
/// only settable slot the fixture exposes) must reach the guest and come back out
/// through the NATIVE `s`/`Pr` cells after a re-solve; writing the native `vwind`
/// (index 10) must NOT touch the guest — under the upstream nesting it would land
/// on `FSetVariable(10 − 22)`.
///
/// The second direction needs an observable of its own, because the fixture's
/// `set_variable` changes nothing for an index it does not own: it therefore
/// **counts** every out-of-range write in `WgBadSet` (variable 33), and the
/// assertion is that the counter stays 0. Without it the write-side half of the
/// not-reproduced mis-nesting had no pin at all — reproducing the upstream
/// nesting left the whole module green (RP1.3 audit finding, settled 2026-08-23;
/// re-measured after this test: the same mutation now reds it).
#[test]
fn set_statevar_routes_only_the_tail_to_the_model() {
    let mut lines = deck("6", &bound_tail());
    lines.push("set StateVar=x WindGen.w1 WgSlip 0.055".to_string());
    lines.push("Solve".to_string());
    let mut dss = feed(&lines, false);
    let (_, values) = w1_variables(&mut dss);
    assert_eq!(values[24], 0.055, "WgSlip took the write");
    assert_eq!(
        values[21], 0.055,
        "the native `s` reports the model's new slip"
    );
    assert_eq!(
        values[20],
        values[18] * 0.055,
        "Pr = Pg·slip at the new slip"
    );

    let mut lines = deck("6", &bound_tail());
    lines.push("set StateVar=x WindGen.w1 vwind 9".to_string());
    let mut dss = feed(&lines, false);
    let (names, values) = w1_variables(&mut dss);
    assert_eq!(values[9], 9.0, "the native slot took the write");
    assert_eq!(
        values[24], SLIP,
        "a native-index write must not reach the user model"
    );
    assert_eq!(names[32], "WgBadSet");
    assert_eq!(
        values[32],
        0.0,
        "a native-index write reached the guest as FSetVariable({}) — the upstream \
         Set_Variable mis-nesting is being reproduced",
        10i32 - 22
    );
}

/// The engine→record wiring (`user_model.rs::wind_gen_vars_from`) must take each
/// `TWindGenVars` field from the WindGen field that actually owns it. The codec
/// offsets are pinned in `dss-usermodel` (`records::tests`), but the *sources*
/// are only visible from here, and the guest can only see what it reads back.
///
/// Four echoes, one per region of the 348-byte image, each pinned against the
/// element's own declared or derived value:
///
/// * `Xdp` @88 (head doubles) — `puXdp·1000·kV²/kVArating`, the interchange
///   grouping of `WindGen.pas:1368-1370`, at the `Create` default `puXdp = 0.28`
///   (`:963`). Substituting the neighbouring `Xd` or `Xdpp` moves it by 4×/1.4×.
/// * `VTarget` @212 — the **unaligned** stretch above the integer block; the
///   element derives it as `1000·kV/√3` for a polyphase machine (`:1406-1408`).
/// * `Poles` @268 and `VCutin` @292 — turbine-tail properties, set by this deck
///   **off their `Create` defaults** (2 and 5, `:977`/`:982`) so neither cell can
///   pass on a default.
#[test]
fn the_record_fields_come_from_the_elements_own_sources() {
    let lines = deck("6", &format!("{} P=4 VCutIn=4", bound_tail()));
    let mut dss = feed(&lines, false);
    let (names, values) = w1_variables(&mut dss);

    assert_eq!(names[33], "WgXdpEcho");
    assert_eq!(
        values[33],
        0.28 * 1000.0 * 0.69_f64.powi(2) / 1800.0,
        "Xdp = puXdp·1000·kV²/kVArating"
    );
    assert_eq!(names[34], "WgVTargetEcho");
    assert_eq!(
        values[34],
        1000.0 * 0.69 / crate::util::sqrt3(),
        "VTarget = 1000·kV/√3"
    );
    assert_eq!(names[35], "WgPolesEcho");
    assert_eq!(values[35], 4.0, "Poles = the deck's P=");
    assert_eq!(names[36], "WgVCutInEcho");
    assert_eq!(values[36], 4.0, "VCutIn = the deck's VCutIn=");
}

/// `Conn` @184 crosses as the element's own connection, not as a zero.
///
/// The wye decks everywhere else in this module cannot say so: `Connection::Wye`
/// is ordinal 0, exactly what an unwritten integer carries, so dropping `conn`
/// from `wind_gen_vars_from` passed every test (RP1.3 audit finding, settled
/// 2026-08-23). A delta WindGen answers 1 — and its `NumConductors` drops to 3
/// with it, which is the second cell of the integer block.
#[test]
fn a_delta_windgen_echoes_its_own_connection() {
    let mut wye = bound_snapshot();
    let (_, wye_vars) = w1_variables(&mut wye);
    assert_eq!(wye_vars[30], 0.0, "wye is Conn = 0");
    assert_eq!(wye_vars[29], 4.0, "…with a neutral conductor");

    let mut lines = deck("6", &bound_tail());
    lines[3] = lines[3].replace("conn=wye", "conn=delta");
    let mut dss = feed(&lines, false);
    let (names, values) = w1_variables(&mut dss);
    assert_eq!(names[30], "WgConnEcho");
    assert_eq!(values[30], 1.0, "delta is Conn = 1");
    assert_eq!(
        values[29], 3.0,
        "…and a delta 3-phase terminal has 3 conductors"
    );
}

/// Pascal `RecalcElementData`'s tail (`WindGen.pas:1418`): `If UserModel.Exists
/// Then UserModel.FUpdateModel`, ported at `nominal.rs`'s
/// `update_user_models`. It is one of the five model-6 arms RP1.3 owes, and the
/// only one that leaves no numeric trace of its own — the fixture therefore
/// counts the calls in `WgUpdCount` (variable 32) and re-reads the boundary
/// record inside `update_model`, the way the `indmach012a` example does.
///
/// An `Edit` that re-derives the machine must therefore do two things: bump the
/// counter by exactly one, and let the guest see the NEW `kVArating` without any
/// re-solve (nothing else refreshes its copy — `calc` is not called by a
/// property edit).
#[test]
fn recalc_element_data_updates_the_bound_model() {
    let mut dss = bound_snapshot();
    let (names, before) = w1_variables(&mut dss);
    assert_eq!(names[31], "WgUpdCount");
    assert!(
        before[31] >= 1.0,
        "building the element must have run FUpdateModel at least once, got {}",
        before[31]
    );
    assert_eq!(before[25], 1800.0, "WgKvaEcho = the deck's kva=");

    dss.command("Edit WindGen.w1 kva=1900");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let (_, after) = w1_variables(&mut dss);
    assert_eq!(
        after[31],
        before[31] + 1.0,
        "exactly one FUpdateModel per RecalcElementData"
    );
    assert_eq!(
        after[25], 1900.0,
        "the model re-read the record inside update_model"
    );
}

/// The property table shape RP1.3 closes: `UserModel` at 18 and `UserData` at 19,
/// everything from `DutyStart` on shifted by two, and the inherited tail landing
/// at 45/46/47/48 — r4133's `NumPropsThisClass = 44` (`WindGen.pas:255`) plus the
/// four `TCktElementClass` rows. Ordinal 18 answers `UserModel.Name` (`:2903`)
/// and 19 the raw `UserData=` text (no `GetPropertyValue` case → the inherited
/// echo, `:2929-2930`).
#[test]
fn usermodel_and_userdata_sit_at_ordinals_18_and_19() {
    let mut dss = bound_snapshot();
    let props = dss
        .element_properties("WindGen.w1")
        .expect("WindGen.w1 exists");
    assert_eq!(props.len(), 48, "44 class rows + 4 inherited");
    let at = |i: usize| (props[i - 1].0.as_str(), props[i - 1].1.as_str());
    assert_eq!(at(17).0, "MVA", "the row before the insertion point");
    assert_eq!(at(18).0, "UserModel");
    assert_eq!(at(18).1, fixture(), "GetPropertyValue 18 = UserModel.Name");
    assert_eq!(at(19).0, "UserData");
    assert_eq!(at(19).1, user_data(), "the raw UserData= text echoes back");
    assert_eq!(at(20).0, "DutyStart", "the tail shifted by two");
    assert_eq!(at(44).0, "VCutOut", "the last class row");
    assert_eq!(at(45).0, "Spectrum");
    assert_eq!(at(46).0, "BaseFreq");
    assert_eq!(at(47).0, "Enabled");
    assert_eq!(at(48).0, "Like");
}

/// Pascal `InitPropertyValues` leaves both rows empty (`WindGen.pas:2453-2454`),
/// and a WindGen that names no model keeps the built-in 22-variable surface.
#[test]
fn usermodel_and_userdata_default_to_empty() {
    let mut dss = feed(&deck("1", ""), false);
    assert_eq!(query(&mut dss, "WindGen.w1.UserModel"), "");
    assert_eq!(query(&mut dss, "WindGen.w1.UserData"), "");
    let (names, _) = w1_variables(&mut dss);
    assert_eq!(names.len(), 22, "no model → the native surface only");
}

/// `UserData=` reaches a loaded model (Pascal `UserModel.Edit := …`,
/// `WindGen.pas:642` → `TWindGenUserModel.Edit`), and a **second** `UserModel=`
/// assignment is a `Set_Name` — it frees the running instance and `FNew`s a fresh
/// one (`WindGenUserModel.pas:158-220`), which therefore comes back at the
/// guest's OWN defaults while `PropertyValue[19]` keeps echoing the last
/// `UserData=` text (Pascal never re-sends it).
///
/// Three distinct slips separate the three states in one number: the deck's
/// `UserData=` 0.035, the later edit's 0.11, and the fixture's own default 0.02
/// that only a genuine re-instantiation can bring back.
#[test]
fn a_second_usermodel_assignment_reinstantiates_the_model() {
    let mut lines = deck("6", &bound_tail());
    lines.push("Edit WindGen.w1 UserData=(slip=0.11)".to_string());
    lines.push("Solve".to_string());
    let mut dss = feed(&lines, false);
    let (_, values) = w1_variables(&mut dss);
    assert_eq!(values[24], 0.11, "UserData= reached the running model");
    assert_eq!(values[21], 0.11, "and came back through the native `s`");

    lines.push(format!("Edit WindGen.w1 UserModel={}", fixture()));
    lines.push("Solve".to_string());
    let mut dss = feed(&lines, false);
    let (_, values) = w1_variables(&mut dss);
    assert_eq!(
        values[24], DEFAULT_SLIP,
        "the re-assignment must FNew a fresh instance at the guest's defaults"
    );
    assert_eq!(
        query(&mut dss, "WindGen.w1.UserData"),
        "slip=0.11",
        "…while the property echo keeps the last UserData= text"
    );
}

/// Pascal `MakeLike` (`WindGen.pas:829`): `UserModel.Name := Other.UserModel.Name`
/// is a `Set_Name`, i.e. free + `LoadLibrary` + `FNew` — the copy gets a **live**
/// model of its own, carrying the guest's defaults, while `:834-835` copies the
/// donor's whole property array so `? …userdata` still echoes the donor's text.
///
/// Regression pin for RP1.3 part C: `make_like` used to clone the donor's slot,
/// and the slot's `Clone` deliberately drops the live wasmi instance — so the
/// copy reported `exists() == false`, fell back to the native 22-variable surface
/// and silently injected nothing while still echoing `UserModel=<path>`
/// (measured).
///
/// Both halves separate cleanly because the donor's `UserData=` moves **every**
/// guest parameter off its default: on the shared bus the donor must draw its
/// edited admittance `(G + jB)` while the copy draws the fixture's own
/// `(DEFAULT_G + j·DEFAULT_B)`, and their slips differ the same way. A copy that
/// replayed the donor's `UserData=` (Pascal does not) would fail on the
/// admittance; a spec-only copy fails on the surface length.
#[test]
fn like_reinstantiates_the_model_with_the_guests_own_defaults() {
    let mut lines = deck("6", &bound_tail());
    lines.push("New WindGen.w2 like=w1 bus1=wbus".to_string());
    lines.push("Solve".to_string());
    let mut dss = feed(&lines, false);

    assert_eq!(query(&mut dss, "WindGen.w2.UserModel"), fixture());
    assert_eq!(
        query(&mut dss, "WindGen.w2.UserData"),
        user_data(),
        "MakeLike copies the donor's property array (`:834-835`)"
    );
    let (w2_names, w2) = variables_of(&mut dss, "WindGen.w2");
    assert_eq!(
        w2_names.len(),
        BOUND_VARS,
        "the copy must hold a LIVE model, not a spec-only slot"
    );
    let (_, w1) = w1_variables(&mut dss);
    assert_eq!(w1[21], SLIP, "the donor keeps its UserData= slip");
    assert_eq!(
        w2[21], DEFAULT_SLIP,
        "the copy runs a fresh instance at the guest's own default slip"
    );

    let v = wbus_voltages(&mut dss);
    let i1 = currents_of(&mut dss, "WindGen.w1");
    let i2 = currents_of(&mut dss, "WindGen.w2");
    for k in 0..3 {
        let want1 = y() * v[k];
        let want2 = Complex64::new(DEFAULT_G, DEFAULT_B) * v[k];
        close(
            &format!("w1 |I[{k}]| at the edited admittance"),
            i1[k].norm(),
            want1.norm(),
            1e-11,
        );
        close(
            &format!("w2 |I[{k}]| at the guest's default admittance"),
            i2[k].norm(),
            want2.norm(),
            1e-11,
        );
    }

    // A donor with no model at all copies clean — no queued load, no diagnostic.
    let mut lines = deck("1", "");
    lines.insert(4, "New WindGen.w2 like=w1 bus1=wbus".to_string());
    let mut dss = feed(&lines, false);
    assert_eq!(query(&mut dss, "WindGen.w2.UserModel"), "");
    assert_eq!(
        dss.element_variable_names("WindGen.w2").map(|v| v.len()),
        Some(22)
    );
}

/// Pascal `DoUserModel`'s else arm (`WindGen.pas:1895`): a `Model=6` WindGen with
/// no user model records message **567** per power-flow iteration and keeps
/// solving on its Yprim contribution alone — `DoSimpleMsg`, not `DoErrorMsg`, so
/// no `SolutionAbort`.
#[test]
fn model_6_without_a_user_model_logs_567_and_keeps_solving() {
    let dss = feed(&deck("6", ""), true);
    let hits: Vec<_> = dss
        .errors()
        .iter()
        .filter(|e| e.code == Some(567))
        .collect();
    assert!(
        !hits.is_empty(),
        "expected message 567, got {:?}",
        dss.errors()
    );
    assert_eq!(
        hits[0].message,
        "WindGen.w1 model designated to use user-written model, but user-written model is \
         not defined."
    );
    assert!(
        hits.iter().all(|e| !e.abort),
        "567 is a DoSimpleMsg — it must not request a solution abort"
    );
    assert!(
        dss.circuit().expect("circuit").is_solved,
        "the power flow still converges on the Yprim contribution"
    );
}

/// Pascal `DoDynamicMode`'s else arm (`WindGen.pas:1996-1997`): in dynamics the
/// missing model is message **5671** *and* `SolutionAbort := TRUE`.
#[test]
fn model_6_dynamics_without_a_user_model_logs_5671_and_aborts() {
    let mut lines = deck("6", "");
    lines.push("Set mode=dynamic stepsize=0.001 number=3".to_string());
    lines.push("Solve".to_string());
    let dss = feed(&lines, true);
    let hits: Vec<_> = dss
        .errors()
        .iter()
        .filter(|e| e.code == Some(5671))
        .collect();
    assert!(
        !hits.is_empty(),
        "expected message 5671, got {:?}",
        dss.errors()
    );
    assert_eq!(hits[0].message, "Dynamics model missing for WindGen.w1 ");
    assert!(
        hits.iter().all(|e| e.abort),
        "5671 carries `SolutionAbort := TRUE`"
    );
}

/// Dynamics round-trip: `InitStateVars` seeds the model (`WindGen.pas:2568`),
/// `DoDynamicMode` reads its current back every iteration (`:1993`) and
/// `IntegrateStates` advances it once per step (`:2663`).
///
/// The fixture's documented law is a trapezoidal first-order lag toward the
/// admittance target `Y·V`. At the corrector's fixed point one step of size `h`
/// scales the deviation `e = I − Y·V` by exactly
///
/// ```text
///     is_hist = is + (h/2τ)·(target − is)              (new step)
///     is      = is_hist + (h/2τ)·(target − is)         (corrector fixed point)
///  ⇒  e_next  = (1 − h/2τ)/(1 + h/2τ) · e
/// ```
///
/// so `|e_2n| / |e_n| = f^n` with `f = (1 − h/2τ)/(1 + h/2τ)` — a two-run
/// comparison in which the unknown seed cancels. It is a strong discriminator:
/// at `h = 1 ms`, `τ = 40 ms` (the deck's `UserData=`) `f = 0.975309` and
/// `f^20 = 0.606515`, and a model that was re-`Init`ed each step, never
/// integrated, or driven with the wrong `h` or `τ` misses it by orders of
/// magnitude. Measured: 0.606610, i.e. **1.6e-4 relative** — the residual is the
/// target itself drifting as the injection decays (the two-run identity assumes
/// a constant target), not a tolerance, and the 1e-3 band keeps ~6× margin.
///
/// The deviation is large to begin with because the dynamics Yprim is the WTG3
/// Thevenin admittance (`:1447-1448`), which pulls `wbus` far below its
/// power-flow voltage: the state current is seeded near the power-flow point and
/// then decays toward the dynamic one.
#[test]
fn dynamics_integrates_the_guests_first_order_lag() {
    let h = 0.001_f64;
    let a = h / (2.0 * TAU);
    let f = (1.0 - a) / (1.0 + a);

    // `(deviation from the admittance target, the target itself)`.
    let deviation = |steps: usize| -> (Complex64, Complex64) {
        let mut lines = deck("6", &bound_tail());
        lines.push(format!("Set mode=dynamic stepsize={h} number={steps}"));
        lines.push("Solve".to_string());
        let mut dss = feed(&lines, false);
        let v = wbus_voltages(&mut dss);
        let i = w1_currents(&mut dss);
        // The terminal current IS the guest's own state current — pin the tie
        // while both are in hand.
        let (_, values) = w1_variables(&mut dss);
        close("WgIout1 = |I[0]|", values[22], i[0].norm(), 1e-11);
        let target = y() * v[0];
        (i[0] - target, target)
    };

    let n = 20;
    let (e_n, target) = deviation(n);
    let (e_2n, _) = deviation(2 * n);
    // Guard: after n steps the state must still be far from its target,
    // otherwise the ratio below would be noise over noise (measured 0.97·|target|).
    assert!(
        e_n.norm() > 0.5 * target.norm(),
        "|e_n| = {:.3e} is not far enough from the target {:.3e} to discriminate",
        e_n.norm(),
        target.norm()
    );
    let ratio = e_2n.norm() / e_n.norm();
    let want = f.powi(n as i32);
    close("|e_2n|/|e_n| = f^n", ratio, want, 1e-3);
}

/// Under `Model=6` the embedded WTG3 model must not run at all: `DoDynamicMode`
/// takes the user-model arm instead of `WindModelDyn.CalcDynamic`, and
/// `IntegrateStates` advances the user model instead of `WindModelDyn.Integrate`
/// (`WindGen.pas:1991-1998`, `:2662-2669`).
///
/// Discriminating against the built-in path: after 20 ms of model-1 dynamics the
/// WTG3 controller states are firmly non-zero (`Pgen ≈ 1.0`, `Vmag ≈ 1.015`,
/// `WtAct ≈ 1.2`, `thetaPitch ≈ 4.38` — pinned in
/// `elements::pc::windgen::tests::dynamics_variables_match_capi015_reference`),
/// so every one of them reading exactly 0.0 is proof the WTG3 never stepped.
#[test]
fn model_6_dynamics_never_touches_the_wtg3_model() {
    let mut lines = deck("6", &bound_tail());
    lines.push("Set mode=dynamic stepsize=0.001 number=20".to_string());
    lines.push("Solve".to_string());
    let mut dss = feed(&lines, false);
    let (names, values) = w1_variables(&mut dss);
    for k in [3usize, 4, 5, 6, 7, 8, 10, 11, 12, 13, 15, 16, 17] {
        assert_eq!(
            values[k],
            0.0,
            "WTG3 state `{}` (variable {}) moved — the built-in dynamics ran",
            names[k],
            k + 1
        );
    }
    assert!(values[18] > 0.0, "…while the user model's Pg is live");
}

/// Pascal `InitStateVars`' 1-phase arm (`WindGen.pas:2516-2522`):
/// `Edp := (NodeV[NodeRef[1]] − NodeV[NodeRef[2]]) − Iterminal[1]·Zthev`.
/// It is reachable **only** for `Model=6`: the WTG3 model is 3-phase-only
/// (`WTG3_Model.pas:494` and every `CalcDynamic` read span `V[1..3]`), so a
/// 1-phase terminal would over-read the terminal array upstream — UB the port
/// does not reproduce (models 1/2/4/5 keep their abort).
#[test]
fn single_phase_dynamics_runs_the_user_model() {
    let mut dss = feed(
        &[
            "Clear".to_string(),
            "New Circuit.wg1p basekv=0.69 phases=3 bus1=srcbus".to_string(),
            "New Line.l1 bus1=srcbus bus2=wbus phases=3 r1=0.005 x1=0.02 length=1".to_string(),
            format!(
                "New WindGen.w1 bus1=wbus.1 phases=1 kv=0.4 kW=500 kva=600 conn=wye model=6 \
                 vwind=12 UserModel={} UserData=({})",
                fixture(),
                user_data()
            ),
            "Set voltagebases=[0.69]".to_string(),
            "Calcvoltagebases".to_string(),
            "Set tolerance=1e-12".to_string(),
            // A single-phase injection on a three-phase bus converges linearly
            // rather than quadratically here: 29 iterations at 1e-12 (10 at the
            // 1e-4 default), so the 15-iteration default cap would leave the
            // circuit unsolved and `Set mode=dynamic` would refuse the entry.
            "Set maxiterations=100".to_string(),
            "Solve".to_string(),
            "Set mode=dynamic stepsize=0.001 number=3".to_string(),
            "Solve".to_string(),
        ],
        false,
    );
    let (_, values) = w1_variables(&mut dss);
    assert_eq!(values.len(), BOUND_VARS, "the model is bound and running");
    assert_eq!(values[28], 1.0, "WgNphEcho = 1");
    assert_eq!(values[29], 2.0, "WgNcondEcho = 2 (1-phase wye)");
    // The record's `Pg` is the guest's own `Re(V·conj(I))` over its ONE phase —
    // so the tail write-back, the single-phase current and the record's
    // `NumPhases` all have to agree for this to hold.
    let v = wbus_voltages(&mut dss)[0];
    let i = w1_currents(&mut dss)[0];
    assert!(values[18] > 0.0, "the 1-phase dynamics arm produced power");
    close("Pg = Re(V·conj(I))", values[18], (v * i.conj()).re, 1e-9);
}

/// Anything but 1 or 3 phases takes Pascal's `Else` (`WindGen.pas:2538-2539`):
/// `DoSimpleMsg(…, 5672)` followed by `SolutionAbort := TRUE`.
#[test]
fn two_phase_dynamics_aborts_with_5672() {
    let dss = feed(
        &[
            "Clear".to_string(),
            "New Circuit.wg2p basekv=0.69 phases=3 bus1=srcbus".to_string(),
            "New Line.l1 bus1=srcbus bus2=wbus phases=3 r1=0.005 x1=0.02 length=1".to_string(),
            format!(
                "New WindGen.w1 bus1=wbus.1.2 phases=2 kv=0.4 kW=500 kva=600 conn=wye model=6 \
                 vwind=12 UserModel={} UserData=({})",
                fixture(),
                user_data()
            ),
            "Set voltagebases=[0.69]".to_string(),
            "Calcvoltagebases".to_string(),
            "Solve".to_string(),
            "Set mode=dynamic stepsize=0.001 number=1".to_string(),
            "Solve".to_string(),
        ],
        true,
    );
    let hit = dss
        .errors()
        .iter()
        .find(|e| e.code == Some(5672))
        .unwrap_or_else(|| panic!("expected message 5672, got {:?}", dss.errors()));
    assert_eq!(
        hit.message,
        "Dynamics mode is implemented only for 1- or 3-phase WindGens. WindGen.w1 has 2 phases."
    );
    assert!(hit.abort, "5672 is followed by `SolutionAbort := TRUE`");
}

/// The enum admits 1/2/4/5 and — since RP1.3 — 6, and still refuses 3 and 7
/// (`obj/dss_enum/registry/pc.rs`). r4133 dispatches both (`WindGen.pas:2112`,
/// `:2116`) but neither `DoPVTypeGen` nor `DoCurrentLimitedPQ` is ported, no
/// corpus deck sets them and no plan owns them (`R4133_PROPS_PLAN.md` §1.3; the
/// deferral is a named `ORPHANED_GAPS.md` row). Refusing at the parser is the
/// loud alternative to silently dispatching them into the constant-PQ arm.
#[test]
fn models_3_and_7_are_still_refused() {
    for bad in ["3", "7"] {
        let mut dss = feed(&deck(bad, ""), true);
        assert!(
            dss.errors().iter().any(|e| e
                .message
                .contains(&format!("Model: \"{bad}\" is not a valid value"))),
            "model={bad} must be refused, got {:?}",
            dss.errors()
        );
        assert_eq!(
            query(&mut dss, "WindGen.w1.Model"),
            "1",
            "a refused enum value must leave the property at its default"
        );
    }
    // …and 6 parses.
    let mut dss = bound_snapshot();
    assert_eq!(query(&mut dss, "WindGen.w1.Model"), "6");
}

/// `GetAllVariables` (`WindGen.pas:2793-2812`) appends the `UserModel` block in
/// BOTH arms — the `If UserModel.Exists` block sits at the same nesting level as
/// the `if DynamiceqObj = nil … else …`, not inside its `else`. The port used to
/// return early out of the `DynamicExp` arm, so a WindGen carrying both a
/// `DynamicEq=` and a `UserModel=` reported no model variables at all (RP1.3
/// audit finding, settled 2026-08-23).
///
/// The same deck settles the second half of that finding: the RP1.3 correction
/// that formed `Edp` from the **record** `Zthev = Xdp/XRdp + jXdp`
/// (`WindGen.pas:2500-2505`) instead of the WTG3 Thevenin impedance was
/// documented as unobservable. It is not: `theta = Edp` pairs the `edp` domain
/// code (9) to a user state variable, `InitStateVars` seeds it with `Cang(Edp)`
/// (`:2588`) and `DynamicExp` variables are readable. The equation here holds
/// both states still (`d/dt = Speed`, and `Speed` starts at 0), so the value read
/// after a dynamics step IS the initial seed, and it is re-derived below from the
/// engine's own solved terminal V/I. The WTG3 impedance — the `RThev=`/`XThev=`
/// pair, which this deck moves far off the record's — would answer a visibly
/// different angle.
#[test]
fn a_dynamic_eq_windgen_still_reports_the_models_variables() {
    let eq = "New DynamicExp.wgde nvariables=2 varnames=[Speed theta]               expression=[Speed dt = Speed; theta dt = Speed]"
        .to_string();
    let wgen = format!(
        "New WindGen.w1 bus1=wbus phases=3 kv=0.69 kW=1500 kva=1800 conn=wye model=6 \
         vwind=12 RThev=0.8 XThev=1.9 DynamicEq=wgde UserModel={} UserData=({})",
        fixture(),
        user_data()
    );
    let mut lines = vec![
        "Clear".to_string(),
        "New Circuit.wgdyneq basekv=0.69 phases=3 bus1=srcbus".to_string(),
        "New Line.l1 bus1=srcbus bus2=wbus phases=3 r1=0.005 x1=0.02 length=1".to_string(),
        eq,
        wgen,
        "~ theta = Edp DynOut=[Speed theta]".to_string(),
        "Set voltagebases=[0.69]".to_string(),
        "Calcvoltagebases".to_string(),
        "Set tolerance=1e-12".to_string(),
        "Set maxiterations=100".to_string(),
        "Solve".to_string(),
    ];

    // The power-flow operating point the dynamics initialisation starts from.
    let mut snap = feed(&lines, false);
    let v = wbus_voltages(&mut snap);
    let i = w1_currents(&mut snap);

    lines.push("Set mode=dynamic stepsize=0.001 number=1".to_string());
    lines.push("Solve".to_string());
    let mut dss = feed(&lines, false);

    // The surface: the equation's 2 variables x 2 memory slots, then the model's.
    let (names, values) = w1_variables(&mut dss);
    assert_eq!(
        names.len(),
        4 + MODEL_VARS.len(),
        "the DynamicExp dump must be FOLLOWED by the user model's variables, got {names:?}"
    );
    assert_eq!(names[..4], ["speed", "dspeed", "theta", "dtheta"]);
    for (k, want) in MODEL_VARS.iter().enumerate() {
        assert_eq!(&names[4 + k], want, "model variable {}", k + 1);
    }
    assert_eq!(
        values[4 + 3],
        1800.0,
        "WgKvaEcho — the model really answered"
    );
    assert_eq!(values[4 + 2], SLIP, "WgSlip — through the DynamicExp arm");

    // `theta` = Cang(Edp) at the record impedance, re-derived here.
    let sym = crate::support::mathutil::SymComp::default();
    let mut v012 = [Complex64::ZERO; 3];
    sym.phase_to_sym(&v, &mut v012);
    let mut i012 = [Complex64::ZERO; 3];
    sym.phase_to_sym(&i[..3], &mut i012);
    let xdp = 0.28 * 1000.0 * 0.69_f64.powi(2) / 1800.0;
    let want =
        crate::support::complexutil::cang(v012[1] - i012[1] * Complex64::new(xdp / 20.0, xdp));
    // …and what the WTG3 Thevenin impedance would have answered instead.
    let wtg3 = crate::support::complexutil::cang(v012[1] - i012[1] * Complex64::new(0.8, 1.9));
    close(
        "theta = Cang(Edp) at Zthev = Xdp/XRdp + jXdp",
        values[2],
        want,
        1e-9,
    );
    assert!(
        (values[2] - wtg3).abs() > 1e-3,
        "the record and WTG3 impedances must be distinguishable here: {} vs {wtg3}",
        values[2]
    );
}

/// **Dormancy** (`R4133_PROPS_PLAN.md` RP1.3 acceptance): the model-6 arms are
/// unreachable for the built-in models. A `Model=1` WindGen with the fixture
/// bound must solve **bit-identically** to the same deck with no `UserModel=` at
/// all — same node voltages, same terminal currents, to the last bit.
///
/// The two differ enormously if the arm leaks: the aerodynamic model injects
/// ~1.3 kA per phase here against the guest's ~0.6 A.
///
/// The variable surface is the one thing that legitimately grows: Pascal's
/// `NumVariables` adds `UserModel.FNumVars` regardless of `GenModel`
/// (`WindGen.pas:2814-2819`), so a bound-but-unused model still contributes its
/// fifteen names.
#[test]
fn a_bound_model_is_dormant_on_the_built_in_models() {
    let mut plain = feed(&deck("1", ""), false);
    let mut bound = feed(&deck("1", &bound_tail()), false);

    let (vp, vb) = (wbus_voltages(&mut plain), wbus_voltages(&mut bound));
    assert_eq!(vp, vb, "a dormant user model must not move a node voltage");
    let (ip, ib) = (w1_currents(&mut plain), w1_currents(&mut bound));
    assert_eq!(ip, ib, "a dormant user model must not move a current");
    assert!(
        ip[0].norm() > 100.0,
        "the built-in model's current is ~1.3 kA — the decks are not accidentally equal"
    );

    let (np, _) = w1_variables(&mut plain);
    let (nb, _) = w1_variables(&mut bound);
    assert_eq!(np.len(), 22);
    assert_eq!(
        nb.len(),
        BOUND_VARS,
        "NumVariables is model-independent upstream"
    );
}

/// An owned **element snapshot** keeps its user model alive.
///
/// `WindGen` derives `Clone` and the control dispatch takes an owned copy of a
/// monitored element when it is also the switched one
/// (`ClassArena::clone_ckt`, `solution/controls/dispatch.rs:623/693/784` —
/// Fuse/Recloser/Relay). The slot's `Clone` deliberately drops the live wasmi
/// instance (it is a `Store`, never shared), so the copy has to re-create it
/// from the spec on first use — `WindGenUserModelSlot::ensure_live`, replaying
/// the last `UserData=` so the revived instance carries the same parameters.
///
/// Until the audit settlement every call site guarded on `exists()` *before*
/// that revive could run, which made it unreachable code and any such snapshot a
/// silent fallback to the built-in model. This test drives the same
/// `clone_ckt` API the dispatch does: `WgSlip` must come back as the deck's
/// `UserData=` slip, not the 0.0 an unanswered model leaves behind.
#[test]
fn an_element_snapshot_revives_its_user_model() {
    let mut dss = bound_snapshot();
    let (_, live) = w1_variables(&mut dss);
    assert_eq!(
        live[24], SLIP,
        "the live element reports the UserData= slip"
    );

    let (sys, node_v) = {
        let ckt = dss.circuit().expect("solved circuit");
        (
            crate::solution::solution::sys_ctx(ckt),
            ckt.solution.node_v.clone(),
        )
    };
    let mut snapshot = {
        let class = dss
            .classes
            .iter()
            .find(|c| c.props.class_name().eq_ignore_ascii_case("WindGen"))
            .expect("the WindGen class is registered");
        class
            .arena
            .clone_ckt(0)
            .expect("a WindGen is a circuit element")
    };

    let elem = snapshot.as_mut();
    assert_eq!(
        elem.num_variables(),
        BOUND_VARS,
        "the snapshot keeps the bound surface"
    );
    let mut states = vec![0.0; elem.num_variables()];
    elem.get_all_variables(&sys, &node_v, &mut states);
    assert_eq!(
        states[24], SLIP,
        "the snapshot must re-create its own instance and replay `UserData=`,          not silently answer nothing"
    );
}

/// A `UserModel=` that names something the sandboxed host cannot load — a missing
/// file, or a native `.dll` (permanently out of reach under
/// `#![forbid(unsafe_code)]`) — warns **570** once and leaves the slot absent
/// (Pascal `WindGenUserModel.pas:187`, the warn-and-fallback path).
///
/// Two documented port conventions are pinned here rather than left implicit:
///
/// * `? …UserModel` echoes the name that was **attempted**, whereas r4133 assigns
///   `FName` only on success (`WindGenUserModel.pas:190`) and would answer `''`.
///   This is the established WM.3 convention (`generator/accessors.rs` does the
///   same) and the #567 suppression below depends on it. No corpus deck sets
///   `UserModel=` on a WindGen, so neither channel is exposed. Both conventions
///   are now recorded outside this file as well —
///   `docs/upgrade/DIVERGENCES.md` §L6 (RP1.3 audit settlement).
/// * No #567 follows. r4133 *would* emit one per iteration (its load failed too,
///   so `UserModel.Exists` is false), but for the port a designated-yet-unloadable
///   name is the native-DLL case, already reported loudly once as #570;
///   re-emitting #567 every iteration would add a port-specific artifact to the
///   error stream. The suppression is deliberate — see `solve.rs::do_user_model`.
#[test]
fn a_missing_wasm_warns_570_and_leaves_the_slot_absent() {
    for name in ["nosuch.wasm", "some_native.dll"] {
        let mut dss = feed(&deck("6", &format!(" UserModel={name}")), true);
        let hits: Vec<_> = dss
            .errors()
            .iter()
            .filter(|e| e.code == Some(570))
            .collect();
        assert_eq!(hits.len(), 1, "exactly one 570, got {:?}", dss.errors());
        assert!(!hits[0].abort, "570 is warn-and-fallback");
        assert!(
            dss.errors().iter().all(|e| e.code != Some(567)),
            "the 570 is the single diagnostic for an unloadable name: {:?}",
            dss.errors()
        );
        assert_eq!(query(&mut dss, "WindGen.w1.UserModel"), name);
        let (names, _) = w1_variables(&mut dss);
        assert_eq!(names.len(), 22, "no model is bound");
    }
}
