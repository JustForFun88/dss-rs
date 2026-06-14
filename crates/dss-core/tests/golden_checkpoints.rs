//! Checkpointed-model gate (golden-infrastructure plan): run **every** scenario
//! file in `tests/golden/checkpoints/` (one file per scenario) and, at every
//! committed time step, compare the **assembled electrical model** against the
//! pinned oracle — not just the converged outputs. A checkpoint is the state
//! after one `solve` (number=1) returns. Each step pins:
//!
//!   - `dblHour`, iteration count, converged flag (exact);
//!   - node order (exact) and node voltages;
//!   - the assembled, unfactored system Y matrix (entry-by-entry on micro/feeder
//!     scenarios, fingerprint on large) — `Dss::system_y_csc` vs the oracle
//!     `YMatrix.getYSparse(False)`;
//!   - selected element YPrim blocks — `Dss::element_yprim` vs `CktElement.Yprim`;
//!   - injection vector, selected element currents/powers, discrete state.
//!
//! Adding a scenario is just adding a `<name>.json` to that directory (the
//! generator writes one per `SCENARIOS` entry). Regenerate only manually:
//! `python tools/golden/gen_checkpoints.py [scenario...]`.

mod harness;

use std::collections::BTreeMap;
use std::path::PathBuf;

use dss_core::exec::{Dss, ElementSnapshot};
use num_complex::Complex64;
use serde::Deserialize;

/// One scenario file: `{schema, oracle, scenario}` (the `oracle` provenance
/// block is ignored here).
#[derive(Debug, Deserialize)]
struct ScenarioFile {
    schema: u32,
    scenario: Scenario,
}

#[derive(Debug, Deserialize)]
struct Scenario {
    name: String,
    kind: String,
    /// Optional master to `compile` (relative to `.inputs/electricdss-tst`)
    /// before replaying `commands` — large feeders that are not inlined.
    #[serde(default)]
    master: Option<String>,
    commands: Vec<String>,
    n_steps: usize,
    node_order: Vec<String>,
    selected_elements: Vec<String>,
    checkpoints: Vec<Checkpoint>,
}

#[derive(Debug, Deserialize)]
struct Checkpoint {
    dbl_hour: f64,
    iterations: i32,
    converged: bool,
    v_re: Vec<f64>,
    v_im: Vec<f64>,
    /// Full assembled-Y CSC (micro/feeder); `None` on large feeders, which pin
    /// only the fingerprint + selected YPrim blocks.
    #[serde(default)]
    y: Option<YMat>,
    y_fingerprint: YFingerprint,
    yprims: Vec<YPrim>,
    elements: Vec<ElementCap>,
    injection: Injection,
    transformers: BTreeMap<String, Vec<f64>>,
    regcontrols: BTreeMap<String, i32>,
    capacitors: BTreeMap<String, Vec<i32>>,
}

/// Compact fingerprint of the assembled system Y (large feeders).
#[derive(Debug, Deserialize)]
struct YFingerprint {
    nnz: usize,
    frob: f64,
    tr_re: f64,
    tr_im: f64,
    maxdiag: f64,
}

/// A selected element's terminal currents (A, re/im) and powers (kW/kvar).
#[derive(Debug, Deserialize)]
struct ElementCap {
    name: String,
    i_re: Vec<f64>,
    i_im: Vec<f64>,
    p_kw: Vec<f64>,
    p_kvar: Vec<f64>,
}

/// The node injection-current vector (RHS of Y*V=I), nodes 1..n.
#[derive(Debug, Deserialize)]
struct Injection {
    re: Vec<f64>,
    im: Vec<f64>,
}

#[derive(Debug, Deserialize)]
struct YMat {
    n: usize,
    rows: Vec<usize>,
    cols: Vec<usize>,
    re: Vec<f64>,
    im: Vec<f64>,
}

#[derive(Debug, Deserialize)]
struct YPrim {
    name: String,
    yorder: usize,
    re: Vec<f64>,
    im: Vec<f64>,
}

/// Per-scenario-kind tolerances (the golden-infrastructure plan §4).
struct Tol {
    v_rel: f64,
    v_abs: f64,
    y_rel: f64,
    y_abs: f64,
    /// Currents (A) and powers (kW/kvar) of selected elements + injection vector.
    i_rel: f64,
    i_abs: f64,
}

fn tol_for(kind: &str) -> Tol {
    match kind {
        // Micro circuits: everything to ~1e-9 rel; small abs floors below
        // physical significance (volts / siemens / amps).
        "micro" => Tol {
            v_rel: 1e-9,
            v_abs: 1e-6,
            y_rel: 1e-9,
            y_abs: 1e-6,
            i_rel: 1e-9,
            i_abs: 1e-6,
        },
        // Large feeders: voltages/admittances/currents 1e-6 rel. The abs floors
        // absorb dead-end / cancellation quantities (µA branch currents, kW that
        // sum to ~0) where 1e-6-rel voltage agreement caps absolute agreement.
        _ => Tol {
            v_rel: 1e-6,
            v_abs: 1e-6,
            y_rel: 1e-6,
            y_abs: 1e-3,
            i_rel: 1e-6,
            i_abs: 1e-4,
        },
    }
}

/// Load every `*.json` scenario file from `tests/golden/checkpoints/`, sorted by
/// file name for deterministic run order.
fn load_scenarios() -> Vec<Scenario> {
    let dir: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "golden",
        "checkpoints",
    ]
    .iter()
    .collect();
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    assert!(
        !files.is_empty(),
        "no checkpoint scenarios in {}",
        dir.display()
    );
    files
        .iter()
        .map(|p| {
            let text = std::fs::read_to_string(p)
                .unwrap_or_else(|e| panic!("cannot read {}: {e}", p.display()));
            let f: ScenarioFile = serde_json::from_str(&text)
                .unwrap_or_else(|e| panic!("cannot parse {}: {e}", p.display()));
            assert_eq!(f.schema, 2, "{}: checkpoints schema mismatch", p.display());
            f.scenario
        })
        .collect()
}

/// Coordinate map of a golden Y matrix, keyed by (row, col).
fn golden_y_map(y: &YMat) -> BTreeMap<(usize, usize), Complex64> {
    let mut m = BTreeMap::new();
    for i in 0..y.rows.len() {
        m.insert((y.rows[i], y.cols[i]), Complex64::new(y.re[i], y.im[i]));
    }
    m
}

/// Compare the assembled system Y entry-by-entry over the union of both
/// patterns. Reports the worst offender (largest |diff|) with its node names.
fn compare_system_y(dss: &mut Dss, exp: &YMat, node_order: &[String], tol: &Tol, ctx: &str) {
    let (n, coords) = dss
        .system_y_csc()
        .unwrap_or_else(|| panic!("{ctx}: Rust has no assembled system Y"));
    assert_eq!(n, exp.n, "{ctx}: system Y order differs ({n} vs {})", exp.n);

    let exp_map = golden_y_map(exp);
    let mut act_map: BTreeMap<(usize, usize), Complex64> = BTreeMap::new();
    for (r, c, v) in coords {
        // Duplicate (r,c) should not occur from a CSC dump, but sum defensively.
        *act_map.entry((r, c)).or_insert(Complex64::ZERO) += v;
    }

    // Union of keys.
    let mut keys: Vec<(usize, usize)> = exp_map.keys().copied().collect();
    for k in act_map.keys() {
        if !exp_map.contains_key(k) {
            keys.push(*k);
        }
    }

    let name = |i: usize| node_order.get(i).map(String::as_str).unwrap_or("?");
    // Count nonzeros above the abs floor on each side (drops cancellation zeros).
    let above = |m: &BTreeMap<(usize, usize), Complex64>| {
        m.values().filter(|v| v.norm() > tol.y_abs).count()
    };
    let nnz_act = above(&act_map);
    let nnz_exp = above(&exp_map);

    // Collect every offender; report the worst few sorted by relative error.
    let mut offenders: Vec<(f64, String)> = Vec::new();
    let mut pattern_diffs = 0usize;
    for (r, c) in keys {
        let a = act_map.get(&(r, c)).copied().unwrap_or(Complex64::ZERO);
        let e = exp_map.get(&(r, c)).copied().unwrap_or(Complex64::ZERO);
        let diff = (a - e).norm();
        let allowed = tol.y_abs + tol.y_rel * e.norm();
        if diff > allowed {
            let in_exp = exp_map.contains_key(&(r, c));
            let in_act = act_map.contains_key(&(r, c));
            let pattern = if in_exp == in_act {
                "match"
            } else {
                pattern_diffs += 1;
                if in_act {
                    "EXTRA in actual"
                } else {
                    "MISSING in actual"
                }
            };
            let rel = diff / e.norm().max(f64::MIN_POSITIVE);
            offenders.push((
                rel,
                format!(
                    "Y[{r},{c}] ({}, {}): actual ({:.6e},{:.6e}) vs oracle ({:.6e},{:.6e})  \
                     |abs|={diff:.3e} |rel|={rel:.3e} > allowed {allowed:.3e} (pattern {pattern})",
                    name(r),
                    name(c),
                    a.re,
                    a.im,
                    e.re,
                    e.im,
                ),
            ));
        }
    }
    if offenders.is_empty() {
        return;
    }
    offenders.sort_by(|a, b| b.0.total_cmp(&a.0));
    let top: String = offenders
        .iter()
        .take(5)
        .map(|(_, m)| format!("\n    {m}"))
        .collect();
    panic!(
        "{ctx}: system Y mismatch — {} entries out of tolerance ({pattern_diffs} structural); \
         nnz actual={nnz_act} oracle={nnz_exp}{}\n  top offenders:{top}",
        offenders.len(),
        if nnz_act != nnz_exp {
            "  <-- SPARSITY PATTERN CHANGED"
        } else {
            ""
        },
    );
}

/// Compare the assembled-Y fingerprint: nnz (above floor) exact, Frobenius
/// norm / trace / max|diag| at `y_rel`. The cheap structural+magnitude guard
/// for large feeders; the precise stale-Y catch there is the selected YPrim
/// blocks.
fn compare_fingerprint(dss: &mut Dss, exp: &YFingerprint, tol: &Tol, ctx: &str) {
    const FLOOR: f64 = 1e-9;
    let (_n, coords) = dss
        .system_y_csc()
        .unwrap_or_else(|| panic!("{ctx}: Rust has no assembled system Y"));
    let mut nnz = 0usize;
    let mut frob = 0.0f64;
    let mut tr = Complex64::ZERO;
    let mut maxdiag = 0.0f64;
    for (r, c, v) in &coords {
        let m = v.norm();
        if m > FLOOR {
            nnz += 1;
        }
        frob += m * m;
        if r == c {
            tr += *v;
            maxdiag = maxdiag.max(m);
        }
    }
    let frob = frob.sqrt();
    assert_eq!(
        nnz, exp.nnz,
        "{ctx}: Y fingerprint nnz differs (actual {nnz} vs oracle {}) — SPARSITY PATTERN CHANGED",
        exp.nnz
    );
    let close = |a: f64, e: f64, what: &str| {
        let allowed = tol.y_abs + tol.y_rel * e.abs();
        assert!(
            (a - e).abs() <= allowed,
            "{ctx}: Y fingerprint {what} differs: actual {a:.9e} vs oracle {e:.9e} \
             (|diff|={:.3e} > allowed {allowed:.3e})",
            (a - e).abs()
        );
    };
    close(frob, exp.frob, "Frobenius norm");
    close(tr.re, exp.tr_re, "trace.re");
    close(tr.im, exp.tr_im, "trace.im");
    close(maxdiag, exp.maxdiag, "max|diag|");
}

/// Compare a selected element's YPrim block (column-major flat, same layout on
/// both sides).
fn compare_yprim(dss: &Dss, exp: &YPrim, tol: &Tol, ctx: &str) {
    let (yorder, flat) = dss
        .element_yprim(&exp.name)
        .unwrap_or_else(|| panic!("{ctx}: no element {} (or no Yprim)", exp.name));
    assert_eq!(
        yorder, exp.yorder,
        "{ctx}: {} yorder differs ({yorder} vs {})",
        exp.name, exp.yorder
    );
    assert_eq!(
        flat.len(),
        exp.re.len(),
        "{ctx}: {} Yprim length differs",
        exp.name
    );
    for (k, a) in flat.iter().enumerate() {
        let e = Complex64::new(exp.re[k], exp.im[k]);
        let diff = (a - e).norm();
        let allowed = tol.y_abs + tol.y_rel * e.norm();
        let row = k % yorder;
        let col = k / yorder;
        assert!(
            diff <= allowed,
            "{ctx}: {} Yprim[{row},{col}] differs: actual ({:.6e},{:.6e}) vs oracle ({:.6e},{:.6e}); \
             |diff|={diff:.3e} > allowed {allowed:.3e}",
            exp.name,
            a.re,
            a.im,
            e.re,
            e.im
        );
    }
}

/// Compare the node injection-current vector (nodes 1..n; slot 0 = ground).
fn compare_injection(dss: &Dss, exp: &Injection, tol: &Tol, ctx: &str) {
    let cur = dss.node_injection_currents();
    let n = exp.re.len();
    assert!(
        cur.len() > n,
        "{ctx}: injection vector too short ({} nodes, need > {n})",
        cur.len(),
    );
    let mut actual = Vec::with_capacity(2 * n);
    for c in cur.iter().take(n + 1).skip(1) {
        actual.push(c.re);
        actual.push(c.im);
    }
    let mut expected = Vec::with_capacity(2 * n);
    for (re, im) in exp.re.iter().zip(&exp.im) {
        expected.push(*re);
        expected.push(*im);
    }
    harness::assert_complex_close(
        &actual,
        &expected,
        tol.i_rel,
        tol.i_abs,
        &format!("{ctx} injection currents"),
    );
}

/// Compare a selected element's terminal currents and powers.
fn compare_element(snaps: &[ElementSnapshot], exp: &ElementCap, tol: &Tol, ctx: &str) {
    let snap = snaps
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case(&exp.name))
        .unwrap_or_else(|| panic!("{ctx}: no element {}", exp.name));
    let mut ei = Vec::with_capacity(2 * exp.i_re.len());
    for (re, im) in exp.i_re.iter().zip(&exp.i_im) {
        ei.push(*re);
        ei.push(*im);
    }
    harness::assert_complex_close(
        &snap.currents,
        &ei,
        tol.i_rel,
        tol.i_abs,
        &format!("{ctx} {} currents", exp.name),
    );
    let mut ep = Vec::with_capacity(2 * exp.p_kw.len());
    for (kw, kvar) in exp.p_kw.iter().zip(&exp.p_kvar) {
        ep.push(*kw);
        ep.push(*kvar);
    }
    harness::assert_complex_close(
        &snap.powers,
        &ep,
        tol.i_rel,
        tol.i_abs,
        &format!("{ctx} {} powers", exp.name),
    );
}

/// Compare per-step discrete control state EXACTLY (tap positions at 1e-12 rel
/// — see the feeder-gate note — RegControl tap numbers and capacitor states by
/// value).
fn compare_discrete(dss: &Dss, cp: &Checkpoint, ctx: &str) {
    let taps = dss.transformer_taps();
    assert_eq!(
        taps.len(),
        cp.transformers.len(),
        "{ctx}: transformer count differs"
    );
    for (name, tr_taps) in &taps {
        let exp = cp
            .transformers
            .get(name)
            .unwrap_or_else(|| panic!("{ctx}: golden has no transformer {name}"));
        assert_eq!(
            tr_taps.len(),
            exp.len(),
            "{ctx}: transformer {name} winding count differs"
        );
        for (w, (a, e)) in tr_taps.iter().zip(exp).enumerate() {
            assert!(
                (a - e).abs() <= 1e-12 * e.abs().max(1.0),
                "{ctx}: transformer {name} winding {} tap: {a} vs {e}",
                w + 1
            );
        }
    }
    let tap_numbers = dss.regcontrol_tap_numbers();
    assert_eq!(
        tap_numbers.len(),
        cp.regcontrols.len(),
        "{ctx}: regcontrol count differs"
    );
    for (name, num) in &tap_numbers {
        let exp = cp
            .regcontrols
            .get(name)
            .unwrap_or_else(|| panic!("{ctx}: golden has no regcontrol {name}"));
        assert_eq!(num, exp, "{ctx}: regcontrol {name} tap number differs");
    }
    let states = dss.capacitor_states();
    assert_eq!(
        states.len(),
        cp.capacitors.len(),
        "{ctx}: capacitor count differs"
    );
    for (name, cap_states) in &states {
        let exp = cp
            .capacitors
            .get(name)
            .unwrap_or_else(|| panic!("{ctx}: golden has no capacitor {name}"));
        assert_eq!(cap_states, exp, "{ctx}: capacitor {name} states differ");
    }
}

fn run_scenario(sc: &Scenario) {
    let tol = tol_for(&sc.kind);
    let mut dss = Dss::new();
    dss.command("clear");
    // Large feeders compile an unmodified master first (relative to
    // .inputs/electricdss-tst), exactly like golden_feeders_controls.rs.
    if let Some(master) = &sc.master {
        let path: PathBuf = [
            env!("CARGO_MANIFEST_DIR"),
            "..",
            "..",
            ".inputs",
            "electricdss-tst",
        ]
        .iter()
        .collect::<PathBuf>()
        .join(master);
        assert!(
            path.is_file(),
            "{}: master script missing: {}",
            sc.name,
            path.display()
        );
        dss.command(&format!(
            "compile \"{}\"",
            path.to_string_lossy().replace('\\', "/")
        ));
    }
    for cmd in &sc.commands {
        dss.command(cmd);
    }
    assert!(
        dss.errors().is_empty(),
        "{}: unexpected engine errors: {:?}",
        sc.name,
        dss.errors()
    );
    assert_eq!(
        sc.checkpoints.len(),
        sc.n_steps,
        "{}: malformed golden",
        sc.name
    );

    for (i, cp) in sc.checkpoints.iter().enumerate() {
        dss.command("solve");
        assert!(
            dss.errors().is_empty(),
            "{} step {i}: engine errors: {:?}",
            sc.name,
            dss.errors()
        );
        let ctx = format!("{} step {i}", sc.name);
        {
            let ckt = dss.circuit().expect("circuit exists");
            assert!(cp.converged, "{ctx}: oracle did not converge");
            assert!(ckt.is_solved, "{ctx}: Rust did not converge");
            assert!(
                (ckt.solution.dbl_hour - cp.dbl_hour).abs() < 1e-12,
                "{ctx}: dblHour {} vs {}",
                ckt.solution.dbl_hour,
                cp.dbl_hour
            );
            assert_eq!(
                ckt.solution.iteration, cp.iterations,
                "{ctx}: iteration count differs"
            );
            // Node order (captured once in the golden; constant across steps).
            let names: Vec<String> = (1..=ckt.num_nodes).map(|j| ckt.node_name(j)).collect();
            assert_eq!(names, sc.node_order, "{ctx}: node order differs");

            // Node voltages.
            let mut actual = Vec::with_capacity(2 * ckt.num_nodes);
            for j in 1..=ckt.num_nodes {
                actual.push(ckt.solution.node_v[j].re);
                actual.push(ckt.solution.node_v[j].im);
            }
            let mut expected = Vec::with_capacity(actual.len());
            for (re, im) in cp.v_re.iter().zip(&cp.v_im) {
                expected.push(*re);
                expected.push(*im);
            }
            harness::assert_complex_close(&actual, &expected, tol.v_rel, tol.v_abs, &ctx);
        }

        // Assembled system Y: full entry-by-entry when the golden carries the
        // CSC (micro/feeder); always the fingerprint (large feeders store only
        // that to keep the golden small).
        if let Some(y) = &cp.y {
            compare_system_y(&mut dss, y, &sc.node_order, &tol, &ctx);
        }
        compare_fingerprint(&mut dss, &cp.y_fingerprint, &tol, &ctx);

        // Selected element YPrim blocks.
        for yp in &cp.yprims {
            compare_yprim(&dss, yp, &tol, &ctx);
        }

        // Node injection vector (the solver RHS).
        compare_injection(&dss, &cp.injection, &tol, &ctx);

        // Selected element terminal currents/powers.
        let snaps = dss.snapshot_elements();
        for ec in &cp.elements {
            compare_element(&snaps, ec, &tol, &ctx);
        }

        // Per-step discrete control state (exact).
        compare_discrete(&dss, cp, &ctx);
    }

    // The selected element list is informational; make sure the golden's
    // YPrim captures cover it.
    for cp in &sc.checkpoints {
        let captured: Vec<&str> = cp.yprims.iter().map(|y| y.name.as_str()).collect();
        for sel in &sc.selected_elements {
            assert!(
                captured.iter().any(|c| c.eq_ignore_ascii_case(sel)),
                "{}: golden missing YPrim for selected element {sel}",
                sc.name
            );
        }
    }
}

#[test]
fn checkpoint_scenarios_match_oracle() {
    for sc in &load_scenarios() {
        run_scenario(sc);
    }
}
