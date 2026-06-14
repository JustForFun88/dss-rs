//! Shared golden-comparison harness for integration tests.
//!
//! Goldens are produced by `tools/golden/generate.py` from dss-python (the
//! same dss_capi engine the Pascal source in `.inputs` builds) and committed
//! under `tests/golden/` at the repository root. See PORTING_PLAN.md §4.
//!
//! The structs mirror the full golden schema; each integration-test binary
//! uses only a subset, so dead-code analysis is suppressed module-wide.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use dss_core::exec::{Dss, ElementSnapshot};
use num_complex::Complex64;
use serde::Deserialize;

/// Schema of `tests/golden/<case>.json` (schema version 1).
#[derive(Debug, Deserialize)]
pub struct Golden {
    pub schema: u32,
    pub name: String,
    pub master: String,
    pub post: Vec<String>,
    pub solution: GoldenSolution,
    pub circuit: GoldenCircuit,
    /// Node names in Y-matrix order, e.g. `"SOURCEBUS.1"`.
    pub node_order: Vec<String>,
    /// Node voltages in volts, re/im interleaved, same order as `node_order`.
    pub node_voltages: Vec<f64>,
    pub total_power_kw_kvar: Vec<f64>,
    pub losses_w_var: Vec<f64>,
    pub transformers: BTreeMap<String, GoldenTransformer>,
    pub regcontrols: BTreeMap<String, GoldenRegControl>,
    pub capacitors: BTreeMap<String, GoldenCapacitor>,
    pub elements: BTreeMap<String, GoldenElement>,
}

#[derive(Debug, Deserialize)]
pub struct GoldenSolution {
    pub converged: bool,
    pub iterations: u32,
    pub mode: i32,
    pub tolerance: f64,
}

#[derive(Debug, Deserialize)]
pub struct GoldenCircuit {
    pub num_buses: u32,
    pub num_nodes: u32,
    pub num_ckt_elements: u32,
}

#[derive(Debug, Deserialize)]
pub struct GoldenTransformer {
    pub taps: Vec<f64>,
}

#[derive(Debug, Deserialize)]
pub struct GoldenRegControl {
    pub tap_number: i32,
}

#[derive(Debug, Deserialize)]
pub struct GoldenCapacitor {
    pub states: Vec<i32>,
}

#[derive(Debug, Deserialize)]
pub struct GoldenElement {
    pub enabled: bool,
    pub bus_names: Vec<String>,
    /// kW/kvar interleaved per conductor and terminal.
    pub powers: Vec<f64>,
    /// Amps, re/im interleaved per conductor and terminal.
    pub currents: Vec<f64>,
    /// Full property dump (name → display string); present when the case was
    /// generated with `props: true`.
    #[serde(default)]
    pub properties: BTreeMap<String, String>,
}

impl Golden {
    /// Load `tests/golden/<name>.json` from the repository root.
    pub fn load(name: &str) -> Golden {
        let path: PathBuf = [
            env!("CARGO_MANIFEST_DIR"),
            "..",
            "..",
            "tests",
            "golden",
            &format!("{name}.json"),
        ]
        .iter()
        .collect();
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read golden {}: {e}", path.display()));
        let golden: Golden = serde_json::from_str(&text)
            .unwrap_or_else(|e| panic!("cannot parse golden {}: {e}", path.display()));
        assert_eq!(golden.schema, 1, "golden schema version mismatch");
        golden
    }
}

/// Tolerance policy from PORTING_PLAN.md §4. Discrete states (taps, switch
/// positions, iteration counts) are compared exactly, not through this.
#[derive(Debug, Clone, Copy)]
pub struct Tolerances {
    /// Relative tolerance for node voltages.
    pub voltage_rel: f64,
    /// Absolute floor below which voltage differences are ignored (volts).
    pub voltage_abs_floor: f64,
    /// Relative tolerance for powers and losses.
    pub power_rel: f64,
    /// Relative tolerance for energy-meter accumulations.
    pub energy_rel: f64,
}

impl Default for Tolerances {
    fn default() -> Self {
        Self {
            voltage_rel: 1e-6,
            voltage_abs_floor: 1e-9,
            power_rel: 1e-6,
            energy_rel: 1e-4,
        }
    }
}

/// Split a value string into its non-numeric "skeleton" (each number replaced
/// by `#`) and the list of numbers it contains — the property-dump comparator
/// (PORTING_PLAN.md §4: numbers with tolerance, structure exactly).
pub fn numeric_skeleton(s: &str) -> (String, Vec<f64>) {
    let mut skeleton = String::new();
    let mut nums = Vec::new();
    let mut i = 0;
    while i < s.len() {
        if let Some((value, len)) = scan_number(&s[i..]) {
            nums.push(value);
            skeleton.push('#');
            i += len;
        } else {
            // values are ASCII; advance one byte
            skeleton.push(s.as_bytes()[i] as char);
            i += 1;
        }
    }
    (skeleton, nums)
}

/// Longest numeric prefix of `s` that parses as `f64` (and contains a digit).
fn scan_number(s: &str) -> Option<(f64, usize)> {
    let bytes = s.as_bytes();
    let first = bytes[0];
    if !(first.is_ascii_digit() || first == b'.' || first == b'+' || first == b'-') {
        return None;
    }
    let mut end = 0;
    while end < bytes.len() {
        let c = bytes[end];
        if c.is_ascii_digit() || matches!(c, b'.' | b'+' | b'-' | b'e' | b'E') {
            end += 1;
        } else {
            break;
        }
    }
    while end > 0 {
        let cand = &s[..end];
        if cand.bytes().any(|b| b.is_ascii_digit())
            && let Ok(v) = cand.parse::<f64>()
        {
            return Some((v, end));
        }
        end -= 1;
    }
    None
}

/// Assert two value strings match: identical skeletons, numbers within
/// `rel`/`abs` tolerance.
pub fn assert_value_matches_tol(actual: &str, expected: &str, rel: f64, abs: f64, ctx: &str) {
    let (askel, anums) = numeric_skeleton(actual);
    let (eskel, enums) = numeric_skeleton(expected);
    assert_eq!(
        askel, eskel,
        "{ctx}: structure differs (actual {actual:?} vs expected {expected:?})"
    );
    assert_eq!(
        anums.len(),
        enums.len(),
        "{ctx}: number count differs (actual {actual:?} vs expected {expected:?})"
    );
    for (i, (a, e)) in anums.iter().zip(&enums).enumerate() {
        let allowed = abs + rel * e.abs();
        assert!(
            (a - e).abs() <= allowed,
            "{ctx}: number {i} differs: actual {a} vs expected {e} \
             (from {actual:?} vs {expected:?})"
        );
    }
}

/// Compare two interleaved re/im arrays element-wise: passes when
/// `|actual − expected| ≤ abs_floor + rel · |expected|` per complex entry.
/// Panics with the first offending index and values.
pub fn assert_complex_close(
    actual: &[f64],
    expected: &[f64],
    rel: f64,
    abs_floor: f64,
    what: &str,
) {
    assert_eq!(
        actual.len(),
        expected.len(),
        "{what}: length mismatch ({} vs {})",
        actual.len(),
        expected.len()
    );
    assert_eq!(actual.len() % 2, 0, "{what}: odd interleaved length");
    for i in (0..actual.len()).step_by(2) {
        let (ar, ai) = (actual[i], actual[i + 1]);
        let (er, ei) = (expected[i], expected[i + 1]);
        let diff = ((ar - er).powi(2) + (ai - ei).powi(2)).sqrt();
        let mag = (er * er + ei * ei).sqrt();
        let allowed = abs_floor + rel * mag;
        assert!(
            diff <= allowed,
            "{what}: entry {} differs: actual ({ar}, {ai}) vs expected ({er}, {ei}); \
             |diff| = {diff:e} > allowed {allowed:e}",
            i / 2
        );
    }
}

// ---------------------------------------------------------------------------
// Assembled-model comparison layer.
//
// Shared by the checkpointed-model gate (`golden_checkpoints.rs`, against
// committed schema-2 goldens) and the live corpus gate (`corpus_live.rs`,
// against the pinned oracle at test time). Both compare the captured electrical
// model — system Y, element YPrim, injection vector, element currents/powers,
// and discrete control state — using the same tolerance policy (`Tol`/
// `tol_for`). See tests/TOLERANCE_NOTES.md for the per-field rationale.
// ---------------------------------------------------------------------------

/// Full assembled-Y CSC coordinates (micro/feeder scenarios); `(rows[k],
/// cols[k]) -> re[k] + j*im[k]`, 0-based, row i ↔ node order index i.
#[derive(Debug, Deserialize)]
pub struct YMat {
    pub n: usize,
    pub rows: Vec<usize>,
    pub cols: Vec<usize>,
    pub re: Vec<f64>,
    pub im: Vec<f64>,
}

/// A selected element's YPrim block, column-major flat (`out[col*yorder+row]`),
/// re/im split — the raw `CktElement.Yprim` / `CMatrix` storage order.
#[derive(Debug, Deserialize)]
pub struct YPrim {
    pub name: String,
    pub yorder: usize,
    pub re: Vec<f64>,
    pub im: Vec<f64>,
}

/// Compact fingerprint of the assembled system Y (large feeders): nnz above a
/// floor, Frobenius norm, complex trace, max|diagonal|.
#[derive(Debug, Deserialize)]
pub struct YFingerprint {
    pub nnz: usize,
    pub frob: f64,
    pub tr_re: f64,
    pub tr_im: f64,
    pub maxdiag: f64,
}

/// An element's terminal currents (A, re/im) and powers (kW/kvar).
#[derive(Debug, Deserialize)]
pub struct ElementCap {
    pub name: String,
    pub i_re: Vec<f64>,
    pub i_im: Vec<f64>,
    pub p_kw: Vec<f64>,
    pub p_kvar: Vec<f64>,
}

/// The node injection-current vector (RHS of Y*V=I), nodes 1..n.
#[derive(Debug, Deserialize)]
pub struct Injection {
    pub re: Vec<f64>,
    pub im: Vec<f64>,
}

/// Per-scenario-kind tolerances (see tests/TOLERANCE_NOTES.md). Discrete state
/// is always exact and never goes through this.
pub struct Tol {
    pub v_rel: f64,
    pub v_abs: f64,
    pub y_rel: f64,
    pub y_abs: f64,
    /// Currents (A) and powers (kW/kvar) of elements + the injection vector.
    pub i_rel: f64,
    pub i_abs: f64,
}

/// Map a scenario `kind` to its tolerance class.
pub fn tol_for(kind: &str) -> Tol {
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
        // Feeders / large networks: voltages/admittances/currents 1e-6 rel. The
        // abs floors absorb dead-end / cancellation quantities (µA branch
        // currents, kW that sum to ~0) where 1e-6-rel voltage agreement caps
        // absolute agreement.
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

/// Coordinate map of a captured Y matrix, keyed by (row, col).
fn y_coord_map(y: &YMat) -> BTreeMap<(usize, usize), Complex64> {
    let mut m = BTreeMap::new();
    for i in 0..y.rows.len() {
        m.insert((y.rows[i], y.cols[i]), Complex64::new(y.re[i], y.im[i]));
    }
    m
}

/// Compare the assembled system Y entry-by-entry over the union of both
/// patterns. Reports the worst offenders (largest |rel|) with node names, the
/// nnz on each side, and whether the sparsity pattern changed.
pub fn compare_system_y(dss: &mut Dss, exp: &YMat, node_order: &[String], tol: &Tol, ctx: &str) {
    let (n, coords) = dss
        .system_y_csc()
        .unwrap_or_else(|| panic!("{ctx}: Rust has no assembled system Y"));
    assert_eq!(n, exp.n, "{ctx}: system Y order differs ({n} vs {})", exp.n);

    let exp_map = y_coord_map(exp);
    let mut act_map: BTreeMap<(usize, usize), Complex64> = BTreeMap::new();
    for (r, c, v) in coords {
        // A CSC dump should not repeat (r,c), but sum defensively.
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
/// norm / trace / max|diag| at `y_rel`. The cheap structural+magnitude guard.
pub fn compare_fingerprint(dss: &mut Dss, exp: &YFingerprint, tol: &Tol, ctx: &str) {
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
pub fn compare_yprim(dss: &Dss, exp: &YPrim, tol: &Tol, ctx: &str) {
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
pub fn compare_injection(dss: &Dss, exp: &Injection, tol: &Tol, ctx: &str) {
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
    assert_complex_close(
        &actual,
        &expected,
        tol.i_rel,
        tol.i_abs,
        &format!("{ctx} injection currents"),
    );
}

/// Compare one element's terminal currents and powers against a capture.
pub fn compare_element(snaps: &[ElementSnapshot], exp: &ElementCap, tol: &Tol, ctx: &str) {
    let snap = snaps
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case(&exp.name))
        .unwrap_or_else(|| panic!("{ctx}: no element {}", exp.name));
    let mut ei = Vec::with_capacity(2 * exp.i_re.len());
    for (re, im) in exp.i_re.iter().zip(&exp.i_im) {
        ei.push(*re);
        ei.push(*im);
    }
    assert_complex_close(
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
    assert_complex_close(
        &snap.powers,
        &ep,
        tol.i_rel,
        tol.i_abs,
        &format!("{ctx} {} powers", exp.name),
    );
}

/// Compare per-step discrete control state EXACTLY: transformer taps (1e-12 rel
/// — see the feeder-gate note), RegControl tap numbers and capacitor states by
/// value. The three maps are the oracle capture keyed by element name.
pub fn compare_discrete(
    dss: &Dss,
    transformers: &BTreeMap<String, Vec<f64>>,
    regcontrols: &BTreeMap<String, i32>,
    capacitors: &BTreeMap<String, Vec<i32>>,
    ctx: &str,
) {
    let taps = dss.transformer_taps();
    assert_eq!(
        taps.len(),
        transformers.len(),
        "{ctx}: transformer count differs"
    );
    for (name, tr_taps) in &taps {
        let exp = transformers
            .get(name)
            .unwrap_or_else(|| panic!("{ctx}: oracle has no transformer {name}"));
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
        regcontrols.len(),
        "{ctx}: regcontrol count differs"
    );
    for (name, num) in &tap_numbers {
        let exp = regcontrols
            .get(name)
            .unwrap_or_else(|| panic!("{ctx}: oracle has no regcontrol {name}"));
        assert_eq!(num, exp, "{ctx}: regcontrol {name} tap number differs");
    }
    let states = dss.capacitor_states();
    assert_eq!(
        states.len(),
        capacitors.len(),
        "{ctx}: capacitor count differs"
    );
    for (name, cap_states) in &states {
        let exp = capacitors
            .get(name)
            .unwrap_or_else(|| panic!("{ctx}: oracle has no capacitor {name}"));
        assert_eq!(cap_states, exp, "{ctx}: capacitor {name} states differ");
    }
}
