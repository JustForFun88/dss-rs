//! Shared golden-comparison harness for integration tests.
//!
//! Goldens are produced by `tools/golden/generate.py` from dss-python (the
//! same dss_capi engine the Pascal source in `.inputs` builds) and committed
//! under `tests/golden/` at the repository root. See PORTING_PLAN.md §4.
//!
//! The structs mirror the full golden schema; each integration-test binary
//! uses only a subset, so dead-code analysis is suppressed module-wide.
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
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
// and discrete control state — using the same tolerance policy
// (`Tolerances`/`tol_for`). See tests/TOLERANCE_NOTES.md for the per-field rationale.
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

/// Tolerance policy (PORTING_PLAN.md §4; per-field rationale in
/// tests/TOLERANCE_NOTES.md): the per-scenario-kind relative/absolute tolerances,
/// built by [`tol_for`]. Discrete state (taps, switch positions, iteration
/// counts) is always compared exactly and never goes through this.
///
/// DO NOT loosen these to make a failing oracle comparison pass. Every floor is
/// calibrated to *proven* f64/f32/faer-vs-KLU reality; a Rust↔oracle gap above its
/// floor is a porting **bug** — find the cause, never widen the band to hide it
/// (CLAUDE.md §"conditioning"). They tighten with proof, never loosen to mask a
/// divergence.
pub struct Tolerances {
    pub v_rel: f64,
    pub v_abs: f64,
    pub y_rel: f64,
    pub y_abs: f64,
    /// Currents (A) and powers (kW/kvar) of elements + the injection vector.
    pub i_rel: f64,
    pub i_abs: f64,
    /// EnergyMeter register accumulations (kWh/kvarh/...): the integration policy
    /// (PORTING_PLAN §4), looser than the per-step electrical quantities because
    /// rounding accumulates over the run.
    pub energy_rel: f64,
    pub energy_abs: f64,
}

/// Map a scenario `kind` to its tolerance class.
pub fn tol_for(kind: &str) -> Tolerances {
    match kind {
        // Micro circuits: everything to ~1e-9 rel; small abs floors below
        // physical significance (volts / siemens / amps).
        "micro" => Tolerances {
            v_rel: 1e-9,
            v_abs: 1e-6,
            y_rel: 1e-9,
            y_abs: 1e-6,
            i_rel: 1e-9,
            i_abs: 1e-6,
            energy_rel: 1e-4,
            energy_abs: 1e-4,
        },
        // Standard feeders (IEEE13/37/123 and the simpler Test/ circuits): the
        // assembled Y is deterministic (1e-8); voltages hold 1e-8 and
        // currents/powers 1e-7 rel / 1e-5 abs — empirically verified over every
        // feeder-kind corpus case (corpus_live). The stiff minority that can't make
        // this floor is reclassified `large` (below): 4Bus-YYD voltages match only
        // ~7e-8, and the PVSystem `CurrentkvarLimite` `Vsource` current differs
        // ~1.2e-4 A.
        "feeder" => Tolerances {
            v_rel: 1e-8,
            v_abs: 1e-6,
            y_rel: 1e-8,
            y_abs: 1e-6,
            i_rel: 1e-7,
            i_abs: 1e-5,
            energy_rel: 1e-4,
            energy_abs: 1e-4,
        },
        // Large / numerically-stiff networks (EPRI ckt5, 8500-node, A-Diakoptics
        // torn zones, inverter cases, 4Bus-YYD): voltages 1e-7; the deterministic Y
        // still holds 1e-8. Currents/powers stay 1e-6 rel / 1e-4 abs — their
        // faer-vs-KLU floor (EPRI ckt5 `Line.mdv201_c_1_266_abc8079` conductor power
        // ~1.3e-6; a PVSystem `Vsource` current ~1.2e-4 A; high-voltage near-zero
        // through-currents ×20 kV). The abs floors absorb dead-end / cancellation
        // quantities (µA branch currents, kW that sum to ~0). Also the safe default
        // for an unrecognized kind.
        _ => Tolerances {
            v_rel: 1e-7,
            v_abs: 1e-6,
            y_rel: 1e-8,
            y_abs: 1e-6,
            i_rel: 1e-6,
            i_abs: 1e-4,
            energy_rel: 1e-4,
            energy_abs: 1e-4,
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
pub fn compare_system_y(
    dss: &mut Dss,
    exp: &YMat,
    node_order: &[String],
    tol: &Tolerances,
    ctx: &str,
) {
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
pub fn compare_fingerprint(dss: &mut Dss, exp: &YFingerprint, tol: &Tolerances, ctx: &str) {
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
pub fn compare_yprim(dss: &Dss, exp: &YPrim, tol: &Tolerances, ctx: &str) {
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
pub fn compare_injection(dss: &Dss, exp: &Injection, tol: &Tolerances, ctx: &str) {
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

/// Compare terminal powers (kW/kvar) per conductor with a **voltage-scaled**
/// absolute floor, the image of the current floor through `P = V·conj(I)`.
///
/// A tolerated terminal-current error `abs_floor` (amps) maps to a power error of
/// `|V_terminal|·abs_floor` (VA) — so comparing power to a *flat* `abs_floor` kW
/// floor while comparing current to a flat `abs_floor` A floor is internally
/// inconsistent (they agree only at |V| = 1 V). Near-zero-impedance connector
/// lines (1.5 m `BUSBAR` segments, `switch=y`; |Yprim| ~ 1e6) make this bite: the
/// through-current `I = Yprim·(V1−V2)` is a catastrophic cancellation whose
/// ~1e-8-rel voltage roundoff (faer vs KLU on an ill-conditioned Y, cond ~ 1e7)
/// surfaces as a ~3e-6-rel current/power error. The current's own `abs_floor`
/// absorbs it; a flat kW power floor does not. So the power abs floor is
/// `abs_floor · max(1, |V_kv|)` with `|V_kv| = |P_kW| / |I_A|` (the terminal
/// voltage in kV, recovered from the captured power and current — self-consistent
/// under positive-sequence ×3, where `|P|` and the accepted `δP` scale together).
/// `max(1, …)` never tightens below the established floor. See
/// tests/TOLERANCE_NOTES.md.
pub fn assert_power_close(actual: &[f64], exp: &ElementCap, rel: f64, abs_floor: f64, what: &str) {
    let (p_kw, p_kvar) = (&exp.p_kw, &exp.p_kvar);
    let (i_re, i_im) = (&exp.i_re, &exp.i_im);
    assert_eq!(
        actual.len(),
        2 * p_kw.len(),
        "{what}: power length mismatch ({} vs {})",
        actual.len(),
        2 * p_kw.len()
    );
    assert_eq!(p_kw.len(), p_kvar.len(), "{what}: kW/kvar length mismatch");
    assert_eq!(
        p_kw.len(),
        i_re.len(),
        "{what}: power/current length mismatch"
    );
    for k in 0..p_kw.len() {
        let (ar, ai) = (actual[2 * k], actual[2 * k + 1]);
        let (er, ei) = (p_kw[k], p_kvar[k]);
        let diff = ((ar - er).powi(2) + (ai - ei).powi(2)).sqrt();
        let p_mag = (er * er + ei * ei).sqrt();
        let i_mag = (i_re[k] * i_re[k] + i_im[k] * i_im[k]).sqrt();
        // |V_kv| = |P_kW| / |I_A| (terminal kV); the power error is |V|·(δI).
        let vkv = if i_mag > 1e-12 { p_mag / i_mag } else { 1.0 };
        let allowed = abs_floor * vkv.max(1.0) + rel * p_mag;
        assert!(
            diff <= allowed,
            "{what}: conductor {k} differs: actual ({ar}, {ai}) vs expected ({er}, {ei}); \
             |diff| = {diff:e} > allowed {allowed:e} (|V| = {vkv:.4} kV)",
        );
    }
}

/// Compare one element's terminal currents and powers against a capture.
pub fn compare_element(snaps: &[ElementSnapshot], exp: &ElementCap, tol: &Tolerances, ctx: &str) {
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
    // Powers use a terminal-voltage-scaled abs floor (see `assert_power_close`):
    // `P = V·conj(I)` so the power floor must be the current floor times |V|, or
    // high-voltage near-cancellation through-power (switch/busbar connectors)
    // fails on solver roundoff the current floor already absorbs.
    assert_power_close(
        &snap.powers,
        exp,
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

/// A monitor's data-channel header, sample count, and per-channel sample arrays
/// (oracle capture; `header` is the data channels only — no leading hour/sec).
#[derive(Debug, Deserialize)]
pub struct MonitorCap {
    pub name: String,
    pub header: Vec<String>,
    pub sample_count: i32,
    pub channels: Vec<Vec<f64>>,
    /// 0-based channel indices to skip (e.g. the mode-5 wall-clock timing
    /// channels, which the port records as 0 — see `golden_phase6.rs`). Empty
    /// for the live gate, which only captures deterministic monitor modes.
    #[serde(default)]
    pub skip_channels: Vec<usize>,
}

/// An EnergyMeter's register names/values and zone branch/end/PCE counts.
/// `branches`/`ends`/`pce` are the zone member name lists; when non-empty (the
/// live gate captures them) membership is compared as a case-insensitive set,
/// strengthening the bare count check. `golden_phase6.rs`'s meter golden leaves
/// them empty (it pins micro-zone membership separately, ordered).
#[derive(Debug, Deserialize)]
pub struct MeterCap {
    pub name: String,
    pub register_names: Vec<String>,
    pub register_values: Vec<f64>,
    pub n_branches: usize,
    pub n_ends: usize,
    pub n_pce: usize,
    #[serde(default)]
    pub branches: Vec<String>,
    #[serde(default)]
    pub ends: Vec<String>,
    #[serde(default)]
    pub pce: Vec<String>,
}

/// Compare a monitor's header (data channels, exact), sample count (exact), and
/// every channel's sample array (`tol.i_rel`/`i_abs` on the f32 samples — the
/// same policy `golden_phase6.rs` uses). Channels listed in `exp.skip_channels`
/// (the mode-5 wall-clock timings) are skipped; the live gate leaves it empty
/// (it captures only deterministic modes, so every channel is compared).
pub fn compare_monitor(dss: &Dss, exp: &MonitorCap, tol: &Tolerances, ctx: &str) {
    let view = dss
        .monitor_view(&exp.name)
        .unwrap_or_else(|| panic!("{ctx}: no monitor {}", exp.name));
    // Our header leads with the hour / t(sec) columns; the oracle Header is the
    // data channels only (`Channel(i)` already skips the time slots).
    assert_eq!(
        &view.header[2..],
        exp.header.as_slice(),
        "{ctx}: monitor {} header differs",
        exp.name
    );
    assert_eq!(
        view.sample_count, exp.sample_count,
        "{ctx}: monitor {} sample count differs",
        exp.name
    );
    assert_eq!(
        view.channels.len(),
        exp.channels.len(),
        "{ctx}: monitor {} channel count differs",
        exp.name
    );
    for (ch, (act, e)) in view.channels.iter().zip(&exp.channels).enumerate() {
        if exp.skip_channels.contains(&ch) {
            continue;
        }
        assert_eq!(
            act.len(),
            e.len(),
            "{ctx}: monitor {} channel {} sample count differs",
            exp.name,
            ch + 1
        );
        for (k, (a, ev)) in act.iter().zip(e).enumerate() {
            let a = *a as f64;
            let allowed = tol.i_abs + tol.i_rel * ev.abs();
            assert!(
                (a - ev).abs() <= allowed,
                "{ctx}: monitor {} channel {} ({}) sample {k} differs: {a} vs {ev} \
                 (|diff|={:.3e} > allowed {allowed:.3e})",
                exp.name,
                ch + 1,
                exp.header.get(ch).map(String::as_str).unwrap_or("?"),
                (a - ev).abs()
            );
        }
    }
}

/// Compare an EnergyMeter's registers (names exact, values within the
/// energy-accumulation policy `tol.energy_rel`/`energy_abs` — PORTING_PLAN §4) and
/// zone branch/end/PCE counts (exact).
pub fn compare_meter(dss: &Dss, exp: &MeterCap, tol: &Tolerances, ctx: &str) {
    let regs = dss
        .meter_registers(&exp.name)
        .unwrap_or_else(|| panic!("{ctx}: no meter {}", exp.name));
    assert_eq!(
        regs.len(),
        exp.register_names.len(),
        "{ctx}: meter {} register count differs",
        exp.name
    );
    for (i, (name, value)) in regs.iter().enumerate() {
        assert_eq!(
            name, &exp.register_names[i],
            "{ctx}: meter {} register {i} name differs",
            exp.name
        );
        let e = exp.register_values[i];
        let allowed = tol.energy_abs + tol.energy_rel * e.abs();
        assert!(
            (value - e).abs() <= allowed,
            "{ctx}: meter {} register {name} differs: {value} vs {e} \
             (|diff|={:.3e} > allowed {allowed:.3e})",
            exp.name,
            (value - e).abs()
        );
    }
    let zone = dss
        .meter_zone(&exp.name)
        .unwrap_or_else(|| panic!("{ctx}: no meter zone {}", exp.name));
    assert_eq!(
        zone.all_branches_in_zone.len(),
        exp.n_branches,
        "{ctx}: meter {} branch count differs",
        exp.name
    );
    assert_eq!(
        zone.all_end_elements.len(),
        exp.n_ends,
        "{ctx}: meter {} end count differs",
        exp.name
    );
    assert_eq!(
        zone.zone_pce.len(),
        exp.n_pce,
        "{ctx}: meter {} PCE count differs",
        exp.name
    );
    // Membership (case-insensitive set) when the capture carries the lists — a
    // strictly stronger check than the counts above, order-independent so a
    // different zone-walk order is not a spurious failure.
    let set = |v: &[String]| -> BTreeSet<String> { v.iter().map(|s| s.to_lowercase()).collect() };
    let cmp_members = |act: &[String], e: &[String], what: &str| {
        if e.is_empty() {
            return;
        }
        let (a, b) = (set(act), set(e));
        assert!(
            a == b,
            "{ctx}: meter {} {what} membership differs (Rust∖oracle={:?}, oracle∖Rust={:?})",
            exp.name,
            a.difference(&b).collect::<Vec<_>>(),
            b.difference(&a).collect::<Vec<_>>(),
        );
    };
    cmp_members(&zone.all_branches_in_zone, &exp.branches, "branch");
    cmp_members(&zone.all_end_elements, &exp.ends, "end");
    cmp_members(&zone.zone_pce, &exp.pce, "PCE");
}

// ---------------------------------------------------------------------------
// Phase 8: text/CSV report comparison (PHASE8_PLAN §2.3).
//
// Compares an oracle-written report file against the Rust-written one after
// parsing numbers out (never a raw float-string diff): a fixed header block is
// matched verbatim, then each data row is split on the report's separator and
// compared field-by-field — numbers within tolerance, identifiers
// case-insensitively. Element/row ordering is the report's contract (Pascal
// iteration order is observable) unless `RustSubsetByKey` is used.
// ---------------------------------------------------------------------------

/// Row-set matching policy for [`compare_export`].
pub enum RowPolicy {
    /// Rust and oracle data rows are identical and in the same order (the
    /// report's contract — Pascal iteration order is observable).
    ExactOrdered,
    /// The Rust file's rows must be a **subset** of the oracle's, matched by the
    /// `key`-th field, with matching value fields. Used by `Export Counts`
    /// during the port: the Rust class registry is a *proper subset* of the
    /// oracle's (only a subset of classes is ported), so this pins every ported
    /// class's count against the oracle without failing on the classes we do not
    /// yet register. Documented in `tests/TOLERANCE_NOTES.md`.
    ///
    /// `require` lists lowercased key values that **must** appear in the Rust
    /// rows — without it the subset check (which iterates only the Rust rows)
    /// would silently pass an empty/under-reporting Rust body (a dropped class
    /// is just absent, not a mismatch). Pass the deck-created + default-item keys
    /// so a registry-walk regression cannot hide (audit-tests WP8.1).
    RustSubsetByKey { key: usize, require: Vec<String> },
}

/// How a [`ColTol`] selects the columns it applies to.
#[derive(Clone)]
pub enum ColSel {
    /// Columns whose (trimmed, lowercased) header name starts with this prefix.
    /// Used where every value column is named (the full-header reports — the
    /// `%…` ratio columns of `SeqCurrents`/`SeqVoltages`, the `Angle%d` columns of
    /// `Voltages`).
    Prefix(String),
    /// Columns at index `>= start` with `(index − start) % 2 == parity`. Used for
    /// the **truncated-header** paired magnitude/angle reports
    /// (`Currents`/`ElemCurrents`/`ElemVoltages`, whose header names only the
    /// first pair, e.g. `…, I_1, Ang_1, ...`): `parity = 1` selects the (mostly
    /// unnamed) angle columns, `parity = 0` the magnitudes.
    Parity { start: usize, parity: usize },
    /// The single column at this exact index. Used to target a specific
    /// non-deterministic column by position (the `Summary` `DateTime` column 0,
    /// masked via [`GateSpec::Mask`]).
    Index(usize),
    /// The column **immediately following** a token equal to `glyph` in the row.
    /// A *content-relative* selector (needs the row fields, not just the header):
    /// used for the fixed-width `Show` angle columns, which sit right after the
    /// `/_` angle glyph but at a **row-dependent index** — the first row of each
    /// bus carries an extra leading `..` dots token (`PadDots`) and the element
    /// forms split the parenthesised `(pu)`/`(nref)` into two tokens, so a fixed
    /// `Index`/`Parity` cannot target the angle across every row. Selecting "the
    /// column after `/_`" pins it regardless of the leading-token shift.
    AfterToken(String),
}

impl ColSel {
    fn matches(&self, j: usize, colnames: &[String], fields: &[String]) -> bool {
        match self {
            ColSel::Prefix(p) => colnames
                .get(j)
                .is_some_and(|n| n.trim().to_lowercase().starts_with(&p.to_lowercase())),
            ColSel::Parity { start, parity } => j >= *start && (j - start) % 2 == *parity,
            ColSel::Index(i) => j == *i,
            ColSel::AfterToken(glyph) => {
                j > 0 && fields.get(j - 1).is_some_and(|f| f.trim() == glyph)
            }
        }
    }
}

/// A per-column tolerance override for [`compare_export`], selecting columns by
/// [`ColSel`] (name prefix or index parity). The first matching override wins;
/// columns with no match use the policy's default `rel`/`abs`.
///
/// Used for the fixed-decimal `Angle` columns of the voltage/current reports:
/// `%.1f`/`%.2f` formatting plus two independent solves round the last printed
/// digit independently, a purely *additive* floor far coarser than the `%g`
/// magnitude/pu columns (so the override is `rel = 0`, `abs ≈ 0.11` / `0.011`). A
/// *formatting* floor (documented in `tests/TOLERANCE_NOTES.md`), NOT a
/// relaxation of the magnitude/pu checks — those stay tight. The angle is
/// `arg(V)`/`arg(I)`, independent of the magnitude; its engine-physics
/// correctness is gated by the live model compare (`corpus_live.rs`, which
/// compares the complex node voltages / terminal currents directly), so here it
/// is only a report-layout / printing-floor check.
pub struct ColTol {
    pub sel: ColSel,
    pub rel: f64,
    pub abs: f64,
    /// Optional **denominator gate**: skip this cell only when the oracle's
    /// denominator is a *near-zero but nonzero* cancellation residual (see
    /// [`GateSpec`]). Used where a value is a ratio (`%I2/I1`) or the phase angle
    /// (`AngResid`) of a near-zero quantity — a faer-vs-KLU noise form carrying no
    /// information. The **`= 0` case is deliberately NOT gated**: an exactly-zero
    /// denominator prints as `0` (Pascal `if I1 > 0`) / the angle of an exact zero
    /// is `0.00`, which `0 == 0` checks perfectly — so only genuine-noise rows are
    /// skipped, not the many exactly-zero rows. The magnitude columns stay tightly
    /// checked on every row. A proven cancellation floor, NOT a relaxation
    /// (tests/TOLERANCE_NOTES.md).
    pub gate: Option<GateSpec>,
}

/// The denominator a [`ColTol::gate`] tests to decide whether to skip a cell.
#[derive(Clone, Copy)]
pub enum GateSpec {
    /// Skip when the oracle's value in a **fixed** column `col` is a near-zero
    /// residual `0 < |oracle[col]| < threshold`. For the symmetrical-component
    /// ratio columns (`%I2/I1`, `%I0/I1`, `%NEMA`) whose denominator `I1` sits in
    /// one fixed column.
    Col(usize, f64),
    /// Skip when the oracle's value in the **immediately preceding** column is a
    /// near-zero residual `0 < |oracle[j-1]| < threshold`. For the paired
    /// magnitude/angle exports (`I, Ang, I, Ang, …`): the angle of a near-zero
    /// current/voltage (a residual or an open-terminal conductor) is faer-vs-KLU
    /// noise, gated on its own magnitude in the column just before it.
    PrevCol(f64),
    /// **Always** skip the matched column — a non-deterministic column that
    /// carries no comparable value (a wall-clock timestamp or an absolute path).
    /// Not a tolerance relaxation of any *value*: the column is genuinely
    /// unpinnable (`Summary`'s `DateTimeToStr(Now)`), documented in
    /// `tests/TOLERANCE_NOTES.md`.
    Mask,
}

/// Policy for [`compare_export`].
pub struct ExportPolicy {
    /// Field separator: `','` for CSV, `'='` for the `Counts` key=value text.
    pub sep: char,
    /// Leading non-blank lines compared **verbatim** (fixed headers / column
    /// row), no number parsing.
    pub header_lines: usize,
    /// Row-set policy.
    pub rows: RowPolicy,
    /// Per-number relative / absolute tolerance for value fields (the default;
    /// overridden per column by [`ExportPolicy::col_tol`]).
    pub rel: f64,
    pub abs: f64,
    /// Per-column tolerance overrides, selected by [`ColSel`].
    pub col_tol: Vec<ColTol>,
}

impl ExportPolicy {
    /// The (`rel`, `abs`) tolerance for the field in column `j`: the first
    /// [`ColTol`] whose [`ColSel`] matches, else the default. `colnames` is the
    /// last header line split on the separator.
    fn tol_for_col(&self, j: usize, colnames: &[String], oracle_fields: &[String]) -> (f64, f64) {
        for ct in &self.col_tol {
            if ct.sel.matches(j, colnames, oracle_fields) {
                return (ct.rel, ct.abs);
            }
        }
        (self.rel, self.abs)
    }

    /// Whether column `j`'s cell should be skipped for this row because its
    /// [`ColTol::gate`] denominator (the oracle's `fields[col]`) is below the
    /// gate threshold — a ratio/angle of near-zero cancellation residuals (see
    /// [`ColTol::gate`]). Only the *matching* `ColTol`'s gate applies.
    fn skip_col(&self, j: usize, colnames: &[String], oracle_fields: &[String]) -> bool {
        for ct in &self.col_tol {
            if ct.sel.matches(j, colnames, oracle_fields) {
                // Band-limit: skip only a *nonzero* sub-threshold denominator
                // (`0 < |v| < thresh`). An exactly-zero denominator prints the
                // ratio as `0` (Pascal `if I1 > 0`) / the angle of an exact zero as
                // `0.00`, which `0 == 0` checks — so those rows stay verified.
                let (col, thresh) = match ct.gate {
                    Some(GateSpec::Col(col, thresh)) => (col, thresh),
                    Some(GateSpec::PrevCol(thresh)) => (j.wrapping_sub(1), thresh),
                    Some(GateSpec::Mask) => return true,
                    None => return false,
                };
                return oracle_fields
                    .get(col)
                    .and_then(|f| f.trim().parse::<f64>().ok())
                    .is_some_and(|v| v != 0.0 && v.abs() < thresh);
            }
        }
        false
    }
}

/// Split a report line into fields for [`compare_export`].
///
/// * `sep == ' '` selects the **fixed-width `Show` table** mode: tokenize on any
///   run of whitespace **or** commas, dropping empty tokens. Pascal's
///   `ShowResults` reports are space-padded columns (`Pad`/`PadDots`) with the
///   occasional glued trailing comma (`ExportLosses`-style `%.5f,` fields), so a
///   single-char split cannot tokenize them — this collapses the padding and
///   strips the comma so each numeric cell parses cleanly (PHASE8_PLAN §2.3).
/// * any other `sep` (`','` CSV, `'='` Counts key=value) splits on exactly that
///   char, trimming each field — the `Export`/CSV path, unchanged.
fn split_fields(line: &str, sep: char) -> Vec<String> {
    if sep == ' ' {
        line.split(|c: char| c.is_whitespace() || c == ',')
            .filter(|f| !f.is_empty())
            .map(|f| f.to_string())
            .collect()
    } else {
        line.split(sep).map(|f| f.trim().to_string()).collect()
    }
}

/// Split a report into non-blank, `\r`-stripped lines.
fn report_lines(s: &str) -> Vec<String> {
    s.lines()
        .map(|l| l.trim_end().to_string())
        .filter(|l| !l.trim().is_empty())
        .collect()
}

/// A field is numeric iff it parses *whole* as `f64` (so a class name like
/// `IndMach012` stays text while a count `2` is a number).
fn field_eq(actual: &str, expected: &str, rel: f64, abs: f64, ctx: &str) {
    match (actual.trim().parse::<f64>(), expected.trim().parse::<f64>()) {
        (Ok(_), Ok(_)) => assert_value_matches_tol(actual.trim(), expected.trim(), rel, abs, ctx),
        _ => assert!(
            actual.trim().eq_ignore_ascii_case(expected.trim()),
            "{ctx}: text field differs (actual {:?} vs expected {:?})",
            actual.trim(),
            expected.trim()
        ),
    }
}

/// Compare two report bodies (oracle vs Rust) per [`ExportPolicy`] (§2.3).
pub fn compare_export(oracle: &str, rust: &str, policy: &ExportPolicy, ctx: &str) {
    let ol = report_lines(oracle);
    let rl = report_lines(rust);
    assert!(
        ol.len() >= policy.header_lines && rl.len() >= policy.header_lines,
        "{ctx}: file shorter than the {} header line(s)",
        policy.header_lines
    );
    // Fixed header block: verbatim.
    for i in 0..policy.header_lines {
        assert_eq!(rl[i], ol[i], "{ctx}: header line {i} differs");
    }
    let split = |line: &str| -> Vec<String> { split_fields(line, policy.sep) };
    // Column names = the last header line split on the separator (for the
    // per-column tolerance lookup). Empty when the report has no header block.
    let colnames: Vec<String> = if policy.header_lines >= 1 {
        split(&ol[policy.header_lines - 1])
    } else {
        Vec::new()
    };
    let odata = &ol[policy.header_lines..];
    let rdata = &rl[policy.header_lines..];

    match &policy.rows {
        RowPolicy::ExactOrdered => {
            assert_eq!(
                rdata.len(),
                odata.len(),
                "{ctx}: data row count differs (rust {} vs oracle {})",
                rdata.len(),
                odata.len()
            );
            for (i, (r, o)) in rdata.iter().zip(odata).enumerate() {
                let (rf, of) = (split(r), split(o));
                assert_eq!(rf.len(), of.len(), "{ctx}: row {i} field count differs");
                for (j, (a, e)) in rf.iter().zip(&of).enumerate() {
                    if policy.skip_col(j, &colnames, &of) {
                        continue;
                    }
                    let (rel, abs) = policy.tol_for_col(j, &colnames, &of);
                    field_eq(a, e, rel, abs, &format!("{ctx}: row {i} field {j}"));
                }
            }
        }
        RowPolicy::RustSubsetByKey { key, require } => {
            // Oracle rows keyed by the lowercased key field.
            let mut omap: BTreeMap<String, Vec<String>> = BTreeMap::new();
            for o in odata {
                let f = split(o);
                if let Some(k) = f.get(*key) {
                    omap.insert(k.to_lowercase(), f);
                }
            }
            let mut rust_keys: BTreeSet<String> = BTreeSet::new();
            for (i, r) in rdata.iter().enumerate() {
                let rf = split(r);
                let k = rf
                    .get(*key)
                    .unwrap_or_else(|| panic!("{ctx}: rust row {i} has no key field"));
                rust_keys.insert(k.to_lowercase());
                let of = omap.get(&k.to_lowercase()).unwrap_or_else(|| {
                    panic!("{ctx}: rust row {i} key {k:?} absent from the oracle file")
                });
                assert_eq!(
                    rf.len(),
                    of.len(),
                    "{ctx}: row key {k:?} field count differs"
                );
                for (j, (a, e)) in rf.iter().zip(of).enumerate() {
                    if policy.skip_col(j, &colnames, of) {
                        continue;
                    }
                    let (rel, abs) = policy.tol_for_col(j, &colnames, of);
                    field_eq(a, e, rel, abs, &format!("{ctx}: row {k:?} field {j}"));
                }
            }
            // Presence guard: a subset compare that iterates only the Rust rows
            // cannot see a *dropped* row, so an empty/under-reporting Rust body
            // would pass silently. Require the must-emit keys explicitly.
            for k in require {
                assert!(
                    rust_keys.contains(&k.to_lowercase()),
                    "{ctx}: required key {k:?} missing from the Rust output \
                     (a dropped class / empty report body)"
                );
            }
        }
    }
}
