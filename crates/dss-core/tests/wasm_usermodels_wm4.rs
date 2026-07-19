//! Channel-1 hermetic replay gate for the WASM_USERMODELS **WM.4** Storage
//! (`DynaDLL=`) and PVSystem (`UserModel=`) user-model paths (plan §2.5-1;
//! recorded decision: the oracle channel is the in-house **r4133 bridge**).
//!
//! Each `tools/golden/wasm_decks/wasm_{pv_pflow,storage_dyn}.dss` deck is run on
//! the **Rust** engine with `@FIXTURE@` substituted by the committed `.wasm`
//! fixture (`tests/fixtures/wasm/wm4model.wasm`), and the captured element
//! state-variable surface + node voltages are compared against the committed
//! golden (`tests/golden/wasm_usermodels/wasm_{pv_pflow,storage_dyn}.json`).
//! Those goldens are generated (manually) by `crates/dss-epri/tests/
//! gen_wasm_usermodels_wm4.rs`, which runs the **same decks** on the r4133 engine
//! driving the **native `.dll` twin** of the wm4model reference model. So this is
//! a real cross-engine oracle comparison — Rust engine + wasm fixture vs r4133
//! engine + native twin — **never "Rust agrees with itself"** (the twin and the
//! wasm fixture are independent builds of the SAME model core).
//!
//! Hermetic (plan §2.6): needs only the committed `.wasm` + the committed
//! goldens — no FPC, no native DLL, no wasm toolchain. Runs in every
//! `cargo test`.

mod harness;

use std::collections::BTreeMap;

use dss_core::exec::Dss;
use harness::tol_for;
use serde::Deserialize;

/// The committed reference fixture, workspace-root-relative.
const FIXTURE_REL: &str = "tests/fixtures/wasm/wm4model.wasm";

/// The committed r4133-oracle golden schema (a subset of the element output).
#[derive(Debug, Deserialize)]
struct WasmGolden {
    element: String,
    converged: bool,
    iterations: u32,
    variable_names: Vec<String>,
    variable_values: Vec<f64>,
    node_voltages: BTreeMap<String, [f64; 2]>,
}

fn workspace_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn load_golden(deck: &str) -> WasmGolden {
    let path = workspace_root().join(format!("tests/golden/wasm_usermodels/{deck}.json"));
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read golden {}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse golden {}: {e}", path.display()))
}

/// Run a WM.4 deck on the Rust engine with `@FIXTURE@` → the committed `.wasm`.
fn run_deck(deck: &str) -> Dss {
    let fixture = workspace_root()
        .join(FIXTURE_REL)
        .canonicalize()
        .unwrap_or_else(|e| panic!("canonicalize fixture: {e}"))
        .to_string_lossy()
        .trim_start_matches(r"\\?\")
        .replace('\\', "/");
    assert!(
        std::path::Path::new(&fixture).is_file(),
        "fixture missing: {fixture}"
    );
    let template = workspace_root().join(format!("tools/golden/wasm_decks/{deck}.dss"));
    let text = std::fs::read_to_string(&template)
        .unwrap_or_else(|e| panic!("read deck {}: {e}", template.display()));
    let deck_text = text.replace("@FIXTURE@", &fixture);

    let mut dss = Dss::new();
    dss.command("clear");
    for line in deck_text.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('!') || t.starts_with("//") {
            continue;
        }
        dss.command(t);
    }
    dss
}

fn check(
    label: &str,
    actual: f64,
    expected: f64,
    rel: f64,
    abs: f64,
    worst: &mut (f64, String),
    fails: &mut Vec<String>,
) {
    let diff = (actual - expected).abs();
    let allowed = abs + rel * expected.abs();
    let relerr = diff / expected.abs().max(f64::MIN_POSITIVE);
    if diff > allowed {
        fails.push(format!(
            "{label}: actual {actual:.12e} vs oracle {expected:.12e}  |diff|={diff:.3e} \
             |rel|={relerr:.3e} > allowed {allowed:.3e}"
        ));
    }
    if expected.abs() > abs && relerr > worst.0 {
        *worst = (
            relerr,
            format!("{label} (|rel|={relerr:.3e}, |abs|={diff:.3e})"),
        );
    }
}

/// The replay comparison for one deck. Both WM.4 decks gate `numeric = true` —
/// the FULL floor comparison: iteration count, the user-model state-variable
/// tail, and every node voltage matched to the harness tier floors.
///
/// - `wasm_pv_pflow` (PVSystem `UserModel=`, VoltageModel=3): a power-flow
///   snapshot — the user model is a pure function of the converged terminal
///   voltage, so it is bit-exact vs the r4133 oracle (~1e-13).
/// - `wasm_storage_dyn` (Storage `DynaDLL=`, TStoreDynaModel): a 2-step dynamics
///   run — and it ALSO matches the r4133 oracle at floor (~1e-13, MEASURED).
///   Unlike the WM.3 Generator dynamics (the D2 divergence in the swing-damping /
///   Zthev-Norton coupling), the Storage `DoDynaModel` is a **pure per-phase
///   current injection** (`StickCurrInTerminalArray(-DESSCurr)`, no swing damping,
///   no Zthev) — there is no cross-version coupling to diverge, so the multi-step
///   trajectory is gated numerically too. (The `numeric` parameter is retained
///   for a future dynamics deck that DOES surface a version divergence — it would
///   then gate structurally, per the WM.3 D2 precedent.)
///
/// The base element variable surface (indices 0..base) is the ENGINE's own — its
/// InvDynVars are a base-surface version divergence (the r4133 oracle sentinels
/// them with an EMPTY name + `-9999.99` outside dynamics, while the dss_capi-
/// 0.14.5 Rust port names/computes them) — NOT the WM.4 subject, so base var
/// values are not compared and base names only where the oracle is non-empty.
fn gate_deck(deck: &str, numeric: bool) {
    let g = load_golden(deck);
    let mut dss = run_deck(deck);

    // The deck loads a real `.wasm`, so there is no "Not Loaded" warning and no
    // trap: the engine must be error-free (a surfaced user-model trap/protocol
    // fault would land here — plan §2.9-5, never a silent fallback).
    assert!(
        dss.errors().is_empty(),
        "{deck}: unexpected engine errors: {:?}",
        dss.errors()
    );

    let (iteration, is_solved) = {
        let ckt = dss.circuit().expect("circuit built");
        (ckt.solution.iteration, ckt.is_solved)
    };
    assert!(g.converged, "{deck}: oracle golden did not converge");
    assert!(is_solved, "{deck}: Rust engine did not converge");
    if numeric {
        assert_eq!(
            iteration as u32, g.iterations,
            "{deck}: final-solve iteration count differs (Rust {iteration} vs oracle {})",
            g.iterations
        );
    }

    let vtol = tol_for("feeder");
    // State-variable floor: 1e-8 rel (the feeder voltage class the model vars
    // inherit through the current computation) / 1e-12 abs (near-zero floor).
    // NOT loosened to pass (CLAUDE.md); on the DynaModel dynamics deck only the
    // DynaModel's OWN four vars are gated numerically (the base-storage
    // trajectory is the version-divergence confound — see the doc above).
    let (var_rel, var_abs) = (1e-8, 1e-12);

    let mut worst = (0.0_f64, String::from("(none)"));
    let mut fails: Vec<String> = Vec::new();

    // --- Node voltages (by name, Y-node order) ---
    let names: Vec<String>;
    let volts: Vec<num_complex::Complex64>;
    {
        let ckt = dss.circuit().expect("circuit built");
        names = (1..=ckt.num_nodes).map(|j| ckt.node_name(j)).collect();
        volts = (1..=ckt.num_nodes)
            .map(|j| ckt.solution.node_v[j])
            .collect();
    }
    let actual_v: BTreeMap<String, num_complex::Complex64> = names
        .iter()
        .zip(&volts)
        .map(|(n, v)| (n.to_ascii_uppercase(), *v))
        .collect();
    for (name, ev) in &g.node_voltages {
        let key = name.to_ascii_uppercase();
        let av = actual_v
            .get(&key)
            .unwrap_or_else(|| panic!("{deck}: Rust missing node {name}"));
        if numeric {
            check(
                &format!("{deck} V[{name}].re"),
                av.re,
                ev[0],
                vtol.v_rel,
                vtol.v_abs,
                &mut worst,
                &mut fails,
            );
            check(
                &format!("{deck} V[{name}].im"),
                av.im,
                ev[1],
                vtol.v_rel,
                vtol.v_abs,
                &mut worst,
                &mut fails,
            );
        }
    }

    // --- Element state-variable surface (ordered, index-aligned) ---
    let var_names = dss
        .element_variable_names(&g.element)
        .unwrap_or_else(|| panic!("{deck}: {} has no variable names", g.element));
    let var_values = dss
        .element_variables(&g.element)
        .unwrap_or_else(|| panic!("{deck}: {} has no variables", g.element));
    assert_eq!(
        var_names.len(),
        var_values.len(),
        "{deck}: variable name/value length mismatch"
    );
    assert_eq!(
        var_names.len(),
        g.variable_names.len(),
        "{deck}: variable COUNT differs (Rust {} vs oracle {}) — surface mismatch",
        var_names.len(),
        g.variable_names.len()
    );
    // The user-model tail is the last 4 vars: `Iout1` (computed), then the three
    // static params `G`/`B`/`Tau` (echoing `UserData`, so version-independent
    // witnesses that `edit` reached the guest). The base element variable surface
    // (indices 0..base) is the ENGINE's own — and its InvDynVars are a base-
    // surface version divergence (the r4133 oracle sentinels them with an EMPTY
    // name + `-9999.99` outside dynamics, while the dss_capi-0.14.5 Rust port
    // names/computes them): NOT the WM.4 subject. So base var VALUES are not
    // floor-compared; base var NAMES are asserted only where the oracle reports a
    // non-empty name (the stable base names — proving the surface structure — and
    // the user tail), skipping the empty-named InvDynVars.
    let user_tail_start = g.variable_names.len().saturating_sub(4);
    // Iout1 (the first tail var) is a computed current: version-independent for
    // the PV pflow SNAPSHOT (a pure function of the converged voltage), but a
    // dynamics-trajectory quantity for the Storage DynaModel run (downstream of
    // the multi-step coupling — the recorded divergence). So it is floor-gated
    // only on the numeric (snapshot) deck; the static params G/B/Tau always.
    for (k, (name, &ev)) in g
        .variable_names
        .iter()
        .zip(g.variable_values.iter())
        .enumerate()
    {
        if !name.is_empty() {
            assert!(
                var_names[k].eq_ignore_ascii_case(name),
                "{deck}: variable[{k}] NAME differs (Rust `{}` vs oracle `{name}`) — surface order mismatch",
                var_names[k]
            );
        }
        let is_user_tail = k >= user_tail_start;
        let is_static_param = is_user_tail && k > user_tail_start; // G/B/Tau
        if (is_user_tail && numeric) || is_static_param {
            check(
                &format!("{deck} var[{k}:{name}]"),
                var_values[k],
                ev,
                var_rel,
                var_abs,
                &mut worst,
                &mut fails,
            );
        }
    }

    eprintln!(
        "[wasm_usermodels_wm4] {deck}: {} vars ({}) + {} nodes; worst rel gap = {}",
        g.variable_names.len(),
        if numeric {
            "user tail + voltages @floor"
        } else {
            "static params @floor; trajectory recorded, not gated"
        },
        g.node_voltages.len(),
        worst.1
    );
    assert!(
        fails.is_empty(),
        "{deck}: {} quantities out of tolerance (worst rel = {}):\n  {}",
        fails.len(),
        worst.1,
        fails.join("\n  ")
    );
}

/// PVSystem `UserModel=` (VoltageModel=3) power-flow snapshot: full numeric gate
/// vs the r4133 oracle (the strong "never Rust-agrees-with-itself" proof of the
/// new 15-function user-model transport).
#[test]
fn wasm_pv_pflow_matches_r4133_oracle() {
    gate_deck("wasm_pv_pflow", true);
}

/// Storage `DynaDLL=` (TStoreDynaModel, 13-fn) 2-step dynamics: full numeric gate
/// vs the r4133 oracle — the pure current-injection DoDynaModel matches at floor
/// (no swing-damping/Zthev coupling to diverge, unlike WM.3's Generator dynamics).
#[test]
fn wasm_storage_dyn_matches_r4133_oracle() {
    gate_deck("wasm_storage_dyn", true);
}

/// WM.3 precedent (no silent fallback): a `model=3` (UserModel) Storage with NO
/// `UserModel=` must SURFACE the missing-model diagnostic (#567) through the
/// solution error log — the inject path drains into `Dss::errors()` (never a
/// silent drop, plan §2.9-5).
#[test]
fn storage_model3_without_usermodel_surfaces_diagnostic() {
    let deck = "\
clear
new circuit.nostm basekv=13.8 phases=3 bus1=sb pu=1.0 R1=0.05 X1=0.15 R0=0.05 X0=0.15
new line.f phases=3 bus1=sb bus2=b3 length=1 units=km r1=0.1 x1=0.3 r0=0.3 x0=0.9 c1=0 c0=0
new storage.s1 bus1=b3 phases=3 conn=delta kv=13.8 kWrated=5000 kWhrated=10000 model=3
set voltagebases=[13.8]
calcvoltagebases
solve";
    let mut dss = Dss::new();
    for line in deck.lines() {
        let t = line.trim();
        if !t.is_empty() {
            dss.command(t);
        }
    }
    assert!(
        dss.errors().iter().any(|d| d.code == Some(567)),
        "Storage model=3 with no UserModel must surface #567 (not a silent fallback); got {:?}",
        dss.error_texts()
    );
}

/// WM.3 precedent (no silent fallback): a `model=3` (UserModel) PVSystem with NO
/// `UserModel=` must SURFACE the missing-model diagnostic (#567) through the
/// solution error log.
#[test]
fn pvsystem_model3_without_usermodel_surfaces_diagnostic() {
    let deck = "\
clear
new circuit.nopv basekv=13.8 phases=3 bus1=sb pu=1.0 R1=0.05 X1=0.15 R0=0.05 X0=0.15
new line.f phases=3 bus1=sb bus2=b3 length=1 units=km r1=0.1 x1=0.3 r0=0.3 x0=0.9 c1=0 c0=0
new pvsystem.pv1 bus1=b3 phases=3 conn=delta kv=13.8 kVA=5000 Pmpp=4000 irradiance=1 model=3
set voltagebases=[13.8]
calcvoltagebases
solve";
    let mut dss = Dss::new();
    for line in deck.lines() {
        let t = line.trim();
        if !t.is_empty() {
            dss.command(t);
        }
    }
    assert!(
        dss.errors().iter().any(|d| d.code == Some(567)),
        "PVSystem model=3 with no UserModel must surface #567 (not a silent fallback); got {:?}",
        dss.error_texts()
    );
}

/// Every committed WM.4 golden parses, has index-aligned name/value arrays, and
/// its deck template exists.
#[test]
fn all_wm4_decks_have_consistent_goldens() {
    for (deck, expect_vars) in [("wasm_pv_pflow", 26usize), ("wasm_storage_dyn", 38usize)] {
        let template = workspace_root().join(format!("tools/golden/wasm_decks/{deck}.dss"));
        assert!(
            template.is_file(),
            "missing deck template: {}",
            template.display()
        );
        let g = load_golden(deck);
        assert!(g.converged, "{deck}: golden not converged");
        assert_eq!(
            g.variable_names.len(),
            g.variable_values.len(),
            "{deck}: golden name/value length mismatch"
        );
        assert_eq!(
            g.variable_names.len(),
            expect_vars,
            "{deck}: golden variable count changed (expected {expect_vars})"
        );
        // The user-model tail is the last 4 (Iout1/G/B/Tau).
        assert!(
            g.variable_names[g.variable_names.len() - 4..]
                .iter()
                .map(|s| s.as_str())
                .eq(["Iout1", "G", "B", "Tau"]),
            "{deck}: user-model variable tail changed"
        );
    }
}
