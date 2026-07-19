//! Channel-1 hermetic replay gate for the WASM_USERMODELS WM.3 Generator
//! user-model path (`WASM_USERMODELS_PLAN.md` §2.5-1; recorded decision
//! 2026-07-19: the oracle channel is the in-house **r4133 bridge**, not pinned
//! dss-python).
//!
//! Each `tools/golden/wasm_decks/*.dss` deck is run on the **Rust** engine with
//! `@FIXTURE@` substituted by the committed `.wasm` fixture
//! (`tests/fixtures/wasm/indmach012a.wasm`), and the captured Generator
//! state-variable surface + node voltages are compared against the committed
//! golden (`tests/golden/wasm_usermodels/*.json`). Those goldens are generated
//! (manually) by `crates/dss-epri/tests/gen_wasm_usermodels.rs`, which runs the
//! **same decks** on the r4133 engine (`dss_epri::Engine`) driving the FPC-built
//! **native `.dll` twin** of the reference model. So this is a real cross-engine
//! oracle comparison — Rust engine + wasm fixture vs r4133 engine + native twin
//! — **never "Rust agrees with itself"** (the twin and the wasm fixture are
//! independent builds of the induction-machine math from the same Pascal, pinned
//! bit-exact to each other by the WM.2 fixture self-gate).
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
const FIXTURE_REL: &str = "tests/fixtures/wasm/indmach012a.wasm";

/// Every WM.3 gate deck (basename shared by `tools/golden/wasm_decks/<d>.dss` and
/// `tests/golden/wasm_usermodels/<d>.json`).
const DECKS: &[&str] = &[
    "wasm_gen_pflow",
    "wasm_gen_dyn",
    "wasm_gen_vars",
    "wasm_gen_edit",
];

/// The committed r4133-oracle golden schema (a subset of the generator's output).
///
/// The state-variable surface is two PARALLEL, ORDER-PRESERVING arrays — never a
/// name-keyed map: binding the same model as both `UserModel=` and `ShaftModel=`
/// (the dyn deck) yields DUPLICATE variable names (6 built-in + 14 UserModel + 14
/// ShaftModel = 34, the ShaftModel names duplicating the UserModel's). A map would
/// silently collapse the 14 duplicates to 20 keys, hiding the whole ShaftModel
/// surface; the arrays pin all 34 in order (Pascal `NumVariables`/`GetAllVariables`
/// concatenate built-in ++ UserModel ++ ShaftModel — `generator.pas:2713/2685`).
#[derive(Debug, Deserialize)]
struct WasmGolden {
    converged: bool,
    iterations: u32,
    /// Generator state-variable names in surface order (built-in ++ UserModel ++
    /// ShaftModel); may contain duplicates.
    variable_names: Vec<String>,
    /// Generator state-variable values, index-aligned with `variable_names`.
    variable_values: Vec<f64>,
    /// Node voltages in Y-node order: node name → [re, im] (volts).
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

/// Run a WM.3 deck on the Rust engine with `@FIXTURE@` → the committed `.wasm`.
/// The deck is fed line-by-line (comments/blanks skipped); the absolute fixture
/// path resolves regardless of the working directory (§2.4 activation rule).
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

/// Compare `actual` vs `expected` (a state variable / a voltage component) with
/// the `abs + rel*|e|` policy; accumulate the worst relative offender for a
/// calibration report. Returns the signed deviation for logging.
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
    // Track the worst RELATIVE gap over quantities above a physical floor (so a
    // near-zero cancellation residual does not dominate the calibration report).
    if expected.abs() > abs && relerr > worst.0 {
        *worst = (
            relerr,
            format!("{label} (|rel|={relerr:.3e}, |abs|={diff:.3e})"),
        );
    }
}

/// The replay comparison for one deck.
///
/// `numeric` selects how strong the oracle comparison is:
///
/// - `true` (the Model=User snapshot decks `wasm_gen_pflow`/`wasm_gen_vars`/
///   `wasm_gen_edit`): the FULL floor comparison — iteration count, every state
///   variable value, and every node voltage matched to the harness tier floors.
///   These prove the WASM `calc`/`edit` transport is numerically bit-exact vs the
///   r4133 oracle (they pass at ~1e-14).
///
/// - `false` (the GenModel=6 + ShaftModel DYNAMICS deck `wasm_gen_dyn`): a
///   STRUCTURAL comparison — convergence, error-free (no WASM trap/protocol fault
///   through any of the user *and* shaft dynamics call sites — `FInit`/
///   `FIntegrate`/`FCalc`), and the full ordered 34-variable surface (names +
///   count) matched to the oracle (proving `NumVariables` totals user ++ shaft,
///   `FGetVarName` for both, and the `GetAllVariables` plumbing). The multi-step
///   dynamics *trajectory* values/voltages are NOT floor-compared here because
///   they are a MEASURED dss_capi-0.14.5-vs-r4133 divergence, not a WASM-transport
///   effect (see the module note + STATUS §WASM-UM WM.3): (1) the Generator swing
///   damping default differs — dss_capi/`generator.pas:1006` `Dpu:=1.0` (D≈13263)
///   vs r4133/`generator.pas:968` sets `D:=1.0` in the ctor but never `Dpu`, so
///   `InitStateVars` recomputes `D:=Dpu*kVArating*1000/w0=0`; (2) with D matched
///   (D=1 on both) a residual ~5e-4 flux-transient gap remains, localized to the
///   dynamics solve (the snapshot terminal current/Is1/slip are bit-identical, so
///   it is not the WASM handoff). Rust ports dss_capi 0.14.5 (its pinned oracle),
///   so forcing a match to r4133's dynamics would DIVERGE from the spec — per the
///   brief's "if the two disagree, STOP and record" rule, it is recorded, never
///   masked by a loosened tolerance.
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

    // Tolerance floors. The Rust engine (faer) and the r4133 oracle (KLU) solve
    // the same nonlinear model on the same inputs, so the gap is the faer-vs-KLU
    // last-ulp floor propagated through the induction-machine slip fixpoint — the
    // `feeder` voltage class, and a `machine` class for the state variables
    // (empirically calibrated, see the assertion report below; NOT loosened to
    // pass — CLAUDE.md).
    let vtol = tol_for("feeder");
    // State-variable floor: the slip fixpoint amplifies the ~1e-8 terminal-voltage
    // floor into the machine currents/losses; 1e-6 rel / 1e-5 abs is the measured
    // faer-vs-KLU class here (calibration printed on any failure).
    let (var_rel, var_abs) = (1e-6, 1e-5);

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

    // --- Generator state-variable surface (ordered, index-aligned) ---
    // Compared position-by-position, NOT via a name-keyed map: the dyn deck's
    // surface has duplicate names (UserModel ++ ShaftModel), so the ordered arrays
    // are the only faithful comparison (a map would collapse the 14 ShaftModel vars
    // into the UserModel's and silently drop the ShaftModel surface).
    let var_names = dss
        .element_variable_names("Generator.g1")
        .unwrap_or_else(|| panic!("{deck}: Generator.g1 has no variable names"));
    let var_values = dss
        .element_variables("Generator.g1")
        .unwrap_or_else(|| panic!("{deck}: Generator.g1 has no variables"));
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
    for (k, (name, &ev)) in g
        .variable_names
        .iter()
        .zip(g.variable_values.iter())
        .enumerate()
    {
        assert!(
            var_names[k].eq_ignore_ascii_case(name),
            "{deck}: variable[{k}] NAME differs (Rust `{}` vs oracle `{name}`) — surface order mismatch",
            var_names[k]
        );
        if numeric {
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
        "[wasm_usermodels] {deck}: {} vars ({}) + {} nodes; worst rel gap = {}",
        g.variable_names.len(),
        if numeric {
            "values @floor"
        } else {
            "names only (dynamics trajectory recorded, not gated)"
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

#[test]
fn wasm_gen_pflow_matches_r4133_oracle() {
    gate_deck("wasm_gen_pflow", true);
}

#[test]
fn wasm_gen_dyn_matches_r4133_oracle() {
    gate_deck("wasm_gen_dyn", false);
}

#[test]
fn wasm_gen_vars_matches_r4133_oracle() {
    gate_deck("wasm_gen_vars", true);
    // Single-variable-by-name query path. NOTE: `? Generator.g1.<var>` is NOT this
    // path — the `?` query resolves PROPERTIES only; a state-variable name returns
    // "Property Unknown" in BOTH engines (verified against the r4133 oracle). The
    // real single-variable executive path is `Get StateVar <elem> <var>` (Pascal
    // Get_Variable → UserModel.FGetVariable, generator.pas:2618;
    // `exec/get_cmd.rs` STATE_VAR), which resolves the name to its 1-based index
    // and reads that state variable. Its value is compared to the committed
    // r4133-oracle golden (the wasm_gen_vars deck has no ShaftModel, so the names
    // are unique — a first-match lookup is unambiguous).
    let g = load_golden("wasm_gen_vars");
    let golden_of = |name: &str| -> f64 {
        let i = g
            .variable_names
            .iter()
            .position(|n| n.eq_ignore_ascii_case(name))
            .unwrap_or_else(|| panic!("golden has no variable {name}"));
        g.variable_values[i]
    };
    let mut dss = run_deck("wasm_gen_vars");
    for probe in ["Slip", "puRs", "MaxSlip"] {
        dss.command(&format!("Get StateVar Generator.g1 {probe}"));
        assert!(
            dss.errors().is_empty(),
            "Get StateVar Generator.g1 {probe} errored: {:?}",
            dss.errors()
        );
        let got: f64 = dss.result().trim().parse().unwrap_or_else(|e| {
            panic!(
                "Get StateVar Generator.g1 {probe} = {:?}: {e}",
                dss.result()
            )
        });
        let exp = golden_of(probe);
        let diff = (got - exp).abs();
        let allowed = 1e-5 + 1e-6 * exp.abs();
        assert!(
            diff <= allowed,
            "Get StateVar Generator.g1 {probe}: {got} vs oracle {exp} (|diff|={diff:.3e} > {allowed:.3e})"
        );
    }
}

#[test]
fn wasm_gen_edit_matches_r4133_oracle() {
    gate_deck("wasm_gen_edit", true);
}

/// Every committed golden parses, has index-aligned name/value arrays (never a
/// name-keyed map that could collapse the dyn deck's duplicate ShaftModel names),
/// and its deck template exists — catches golden/deck schema drift for all
/// [`DECKS`] in one place.
#[test]
fn all_wasm_decks_have_consistent_goldens() {
    for &deck in DECKS {
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
            "{deck}: golden name/value length mismatch (name-keyed collapse?)"
        );
        assert!(
            !g.variable_names.is_empty(),
            "{deck}: golden has no variable surface"
        );
    }
    // The dyn deck binds the same model as UserModel AND ShaftModel, so its
    // surface is the full 34 (6 built-in + 14 + 14) with duplicate ShaftModel
    // names — the array schema pins all 34 (a map would show 20).
    assert_eq!(
        load_golden("wasm_gen_dyn").variable_names.len(),
        34,
        "wasm_gen_dyn golden lost the ShaftModel surface (expected 6+14+14=34)"
    );
}
