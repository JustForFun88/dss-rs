//! MakePosSequence context / plan types — the return-value protocol for the
//! [`CktElement::make_pos_sequence`] override (Pascal per-class
//! `MakePosSequence`, dispatched by `TExecHelper.DoMakePosSeq`).
//!
//! Each element converts its *own* direct fields inside the method (bus/phase
//! resyncs, `PrpSequence` clears, buffer reallocs) and returns a
//! [`PosSeqPlan`]: the ordered property-system mutations the exec applier must
//! replay through the typed setter helpers (Pascal `SetDouble`/`SetInteger`/
//! `SetDoubles`/`SetIntegers`/`SetStrings`, which auto-wrap `BeginEdit`/
//! `EndEdit` when not already editing). The applier owns the editing-active VM,
//! the base bus rename (when `run_base`), and the resolution of the
//! monitored/controlled element info a control/meter reads while converting.
//!
//! [`CktElement::make_pos_sequence`]: crate::elements::traits::CktElement::make_pos_sequence

use std::ops::{Add, Div, Mul, Sub};

/// The positive-sequence self term `S(M)` of the `n`-conductor matrix `m`
/// (row-major `n²`), read on its symmetric part `(M + Mᵀ)/2`: every
/// off-diagonal pair enters as `(M_ij + M_ji)/2`.
///
/// - `n` = 2 or 3: `(Σ_i M_ii − Σ_{i<j} M_ij) / 3`. That is `(1/3)·v1ᴴ M v1`
///   of the matrix embedded in three phases, `v1 = [1, a², a]`: under a
///   balanced positive-sequence voltage an element of nodal matrix `Y` draws
///   exactly `3 |V1|² conj(S(Y))`, so a one-phase element carrying `S(Y)`
///   reports the multi-phase element's power under the ×3 positive-sequence
///   report. For three conductors it is the mean of the diagonal minus the
///   mean of the three off-diagonal pairs.
/// - `n` > 3: the mean of the diagonal minus the mean over all off-diagonal
///   pairs, the three-conductor rule.
/// - `n` = 1: the single entry (one conductor has nothing to reduce).
///
/// The capacitor, reactor and fault reductions use it, pinned by
/// `elements::pd::capacitor::tests::make_pos_sequence_cmatrix_applies_the_positive_sequence_cuf`
/// and its siblings.
pub fn pos_seq_self_term<T>(m: &[T], n: usize) -> T
where
    T: Copy
        + Default
        + Add<Output = T>
        + Sub<Output = T>
        + Mul<f64, Output = T>
        + Div<f64, Output = T>,
{
    if n <= 1 {
        return m.first().copied().unwrap_or_default();
    }
    let mut diag = T::default();
    let mut pairs = T::default();
    for i in 0..n {
        diag = diag + m[i * n + i];
        for j in (i + 1)..n {
            pairs = pairs + (m[i * n + j] + m[j * n + i]) * 0.5;
        }
    }
    if n <= 3 {
        (diag - pairs) / 3.0
    } else {
        let nf = n as f64;
        diag / nf - pairs / (nf * (nf - 1.0) / 2.0)
    }
}

/// A snapshot of a monitored or controlled element, resolved by the exec
/// applier and handed to a control/meter's [`make_pos_sequence`]. Pascal reads
/// the live `MonitoredElement`/`ControlledElement` fields (`NPhases`, `Yorder`,
/// `NConds`, `BusNames[1]`, `NumStateVars`, `Enabled`) mid-conversion; the
/// borrow checker forbids that here, so the applier copies them in up front.
///
/// [`make_pos_sequence`]: crate::elements::traits::CktElement::make_pos_sequence
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PosSeqElemInfo {
    /// `BusNames[1..NTerms]`, lowercased (1-based terminal → 0-based slot).
    pub bus_names: Vec<String>,
    /// `NPhases`.
    pub nphases: usize,
    /// `NConds`.
    pub nconds: usize,
    /// `Yorder`.
    pub yorder: usize,
    /// `NumStateVars` (`NumVariables`).
    pub num_variables: usize,
    /// `Enabled`.
    pub enabled: bool,
}

/// The read-only context passed into every [`make_pos_sequence`] call.
///
/// [`make_pos_sequence`]: crate::elements::traits::CktElement::make_pos_sequence
#[derive(Debug, Clone, Default)]
pub struct PosSeqCtx {
    /// This element's own per-terminal parsed node numbers, as the AuxParser
    /// splits each `GetBus(i)` (Pascal `AuxParser.ParseAsBusName`). Slot `i`
    /// (0-based) holds terminal `i+1`'s node list (`bus.1.2.3` → `[1, 2, 3]`;
    /// a bare `bus` → `[]`). The Transformer/AutoTrans OnPhase1 disable test
    /// reads `terminal_nodes[w][0]` to decide whether a 1/2-phase winding
    /// sits on phase 1.
    pub terminal_nodes: Vec<Vec<i32>>,
    /// The monitored element's snapshot, for controls/meters that read it
    /// (`None` for elements that monitor nothing, or when unresolved).
    pub monitored: Option<PosSeqElemInfo>,
    /// The controlled element's snapshot, for controls that read it.
    pub controlled: Option<PosSeqElemInfo>,
}

/// One property-system mutation a [`make_pos_sequence`] override requests of
/// the exec applier. `BeginEdit`/`EndEdit` bracket a multi-set block (a bare
/// `Set*` with no surrounding bracket is a single edit the applier wraps
/// itself); the `Set*` variants are the Pascal typed setters; `Disable` maps
/// the winding-not-on-phase-1 path (`Enabled := FALSE`).
///
/// [`make_pos_sequence`]: crate::elements::traits::CktElement::make_pos_sequence
#[derive(Debug, Clone, PartialEq)]
pub enum PosSeqAction {
    /// Pascal `BeginEdit(True)` — open an explicit multi-set edit.
    BeginEdit,
    /// Pascal `EndEdit(1)` — close it (runs the per-class recalc). Storage's
    /// trailing bare `EndEdit` with no matching `BeginEdit` forces one extra
    /// recalc; the applier reproduces that.
    EndEdit,
    /// Pascal `SetDouble(prop_idx, value)`.
    SetF64(usize, f64),
    /// Pascal `SetInteger(prop_idx, value)`.
    SetI32(usize, i32),
    /// Set a rating property (`NormAmps`/`EmergAmps`) to a rating or none.
    SetRating(usize, crate::obj::Rating),
    /// Pascal `SetDoubles(prop_idx, values)` onto a struct-array property
    /// (per-winding `kVs`/`kVAs`): `None` keeps the prior entry.
    SetStructF64s(usize, Vec<Option<f64>>),
    /// Pascal `SetIntegers(prop_idx, ordinals)` onto a struct-array enum
    /// property (per-winding `conns`).
    SetStructI32s(usize, Vec<i32>),
    /// Pascal `SetStrings(busesPropIdx, names)` onto the `buses` struct array.
    SetStructBuses(Vec<String>),
    /// Pascal `Enabled := FALSE` (the 1/2-phase-winding-off-phase-1 path).
    Disable,
}

/// The plan a [`make_pos_sequence`] override returns: the ordered property-set
/// actions plus whether the base bus rename ([`make_pos_sequence_base`]) still
/// runs afterwards. The default ([`PosSeqPlan::default`]) is "no actions, run
/// the base rename" — the behavior of every element without an override
/// (Pascal `inherited MakePosSequence`).
///
/// [`make_pos_sequence`]: crate::elements::traits::CktElement::make_pos_sequence
/// [`make_pos_sequence_base`]: crate::elements::ckt::CktElementData::make_pos_sequence_base
#[derive(Debug, Clone)]
pub struct PosSeqPlan {
    pub actions: Vec<PosSeqAction>,
    /// Whether the applier runs the base bus rename after the actions. `true`
    /// for the base behavior and every override that ends with `inherited
    /// MakePosSequence`; `false` for the empty overrides (UPFC/IndMach012) that
    /// have no `inherited` call.
    pub run_base: bool,
}

impl Default for PosSeqPlan {
    fn default() -> Self {
        Self {
            actions: Vec::new(),
            run_base: true,
        }
    }
}

impl PosSeqPlan {
    /// A plan that only runs the base rename (the trait default).
    pub fn base() -> Self {
        Self::default()
    }

    /// A plan with the given actions, still running the base rename after
    /// (Pascal override ending in `inherited MakePosSequence`).
    pub fn with_actions(actions: Vec<PosSeqAction>) -> Self {
        Self {
            actions,
            run_base: true,
        }
    }

    /// A plan that runs no base rename (the empty UPFC/IndMach012 overrides).
    pub fn no_base() -> Self {
        Self {
            actions: Vec::new(),
            run_base: false,
        }
    }
}

/// Power-balance helpers for the `make_pos_sequence` pins: a stiff source bus,
/// a reduction, and the element powers before and after it.
#[cfg(test)]
pub(crate) mod balance {
    use crate::exec::Dss;
    use num_complex::Complex64;

    /// The relative bound of a balance pin: headroom over the 1.6e-16 to
    /// 2e-11 the pins measure (2026-10-04). The upper end is the unbalanced
    /// elements, which unbalance the source bus a little.
    pub(crate) const BALANCE_REL: f64 = 1e-9;

    /// A circuit whose source bus `b` holds 12.47 kV line to line at 60 Hz
    /// behind a 1e12 MVA short-circuit level, so an element on `b` sees one
    /// balanced voltage before and after the reduction. An unbalanced element
    /// still unbalances the bus by about `|I| |Zs| / |V|`, which shows as up to
    /// 2e-11 in the balance pins (1e-9 at 1e10 MVA).
    pub(crate) fn stiff(name: &str) -> Dss {
        let mut dss = Dss::new();
        dss.command("clear");
        dss.command("set defaultbasefrequency=60");
        dss.command(&format!(
            "new circuit.{name} basekv=12.47 pu=1.0 phases=3 bus1=b mvasc3=1e12 mvasc1=1e12"
        ));
        dss
    }

    /// [`stiff`] with a line from `b` to a 100 kW load on bus `c`, and a monitor
    /// and an energy meter on the line: the readers a report walks.
    pub(crate) fn stiff_metered(name: &str) -> Dss {
        let mut dss = stiff(name);
        dss.command("new line.feed bus1=b bus2=c phases=3 r1=0.1 x1=0.3 length=1");
        dss.command("new load.ld bus1=c phases=3 kv=12.47 kw=100");
        dss.command("new monitor.mon element=line.feed terminal=1");
        dss.command("new energymeter.em element=line.feed terminal=1");
        dss
    }

    /// The solve error (code 482) of a Y build an element refuses.
    pub(crate) const REFUSED_SOLVE: &str =
        "Error Encountered in Solve: an element has no defined primitive admittance";

    /// After a refused solve, `summary`, `show currents`, `export summary` and
    /// `sample` each finish without an error of their own, the summary reads
    /// the circuit as not solved, and `export y` and `show y` answer that no Y
    /// matrix is built. The next solve builds the Y matrix again and refuses
    /// again: it logs the element's `message` once more beside
    /// [`REFUSED_SOLVE`].
    pub(crate) fn reports_after_refusal(dss: &mut Dss, message: &str, what: &str) {
        let dir = std::env::temp_dir().join(format!("dss_refused_{}_{what}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dss.command(&format!("set datapath=\"{}\"", dir.display()));
        for cmd in ["summary", "show currents", "export summary", "sample"] {
            let n = dss.error_texts().len();
            dss.command(cmd);
            let errors = dss.error_texts();
            assert_eq!(
                errors.len(),
                n,
                "{what}: `{cmd}` after the refusal: {:?}",
                &errors[n..]
            );
            if cmd == "summary" {
                assert!(
                    dss.result().contains("Status = NOT Solved"),
                    "{what}: {}",
                    dss.result()
                );
            }
        }
        for cmd in ["export y", "show y"] {
            let n = dss.error_texts().len();
            dss.command(cmd);
            let errors = dss.error_texts();
            assert!(
                errors[n..]
                    .iter()
                    .any(|t| t.contains("Y Matrix not Built.")),
                "{what}: `{cmd}` after the refusal: {:?}",
                &errors[n..]
            );
        }
        assert!(!dss.circuit().unwrap().is_solved, "{what}: solved");

        let n = dss.errors().len();
        dss.command("solve");
        let errors = &dss.errors()[n..];
        assert!(
            errors.iter().any(|d| d.message.contains(message)),
            "{what}: the second solve does not name the element: {errors:?}"
        );
        assert!(
            errors
                .iter()
                .any(|d| d.code == Some(482) && d.message == REFUSED_SOLVE),
            "{what}: the second solve did not fail with 482: {errors:?}"
        );
        assert!(
            !dss.circuit().unwrap().is_solved,
            "{what}: solved the second time"
        );
        // Drop the report files, keep the data path valid for the caller.
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
    }

    /// Set the 12.47 kV base and solve, asserting a clean, solved circuit.
    pub(crate) fn solve_clean(dss: &mut Dss, when: &str) {
        dss.command("set voltagebases=[12.47]");
        dss.command("calcvoltagebases");
        dss.command("solve");
        assert!(dss.errors().is_empty(), "{when}: {:?}", dss.errors());
        assert!(dss.circuit().unwrap().is_solved, "{when}: not solved");
    }

    /// The power (kW + j kvar) of `full` at its first terminal, the sum over
    /// its conductors. A positive-sequence circuit reports three times the
    /// one phase, so the reading before and after a reduction is one quantity.
    pub(crate) fn power(dss: &mut Dss, full: &str) -> Complex64 {
        let snap = dss.snapshot_elements();
        let e = snap
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case(full))
            .unwrap_or_else(|| panic!("{full} not in the snapshot"));
        e.powers[..e.n_conds].iter().sum()
    }

    /// The loss (W + j var) of `full`.
    pub(crate) fn loss(dss: &mut Dss, full: &str) -> Complex64 {
        let snap = dss.snapshot_elements();
        let e = snap
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case(full))
            .unwrap_or_else(|| panic!("{full} not in the snapshot"));
        Complex64::new(e.loss_w.0, e.loss_w.1)
    }

    /// Solve, read the powers of `names`, `makeposseq`, solve again and read
    /// them once more: `(before, after)` per element.
    pub(crate) fn reduce(dss: &mut Dss, names: &[&str]) -> Vec<(Complex64, Complex64)> {
        solve_clean(dss, "before makeposseq");
        let pre: Vec<Complex64> = names.iter().map(|n| power(dss, n)).collect();
        dss.command("makeposseq");
        dss.command("solve");
        assert!(
            dss.errors().is_empty(),
            "after makeposseq: {:?}",
            dss.errors()
        );
        assert!(
            dss.circuit().unwrap().is_solved,
            "after makeposseq: not solved"
        );
        let post: Vec<Complex64> = names.iter().map(|n| power(dss, n)).collect();
        pre.into_iter().zip(post).collect()
    }

    /// `after` equals `before` within [`BALANCE_REL`].
    pub(crate) fn assert_kept(before: Complex64, after: Complex64, what: &str) {
        assert!(
            after.re.is_finite() && after.im.is_finite(),
            "{what}: {after} is not finite"
        );
        assert!(
            (after - before).norm() <= BALANCE_REL * before.norm(),
            "{what}: {before} before makeposseq, {after} after (rel {:.3e})",
            (after - before).norm() / before.norm()
        );
    }

    /// The solved magnitude (V) of node 1 of bus `bus`.
    pub(crate) fn v1(dss: &Dss, bus: &str) -> f64 {
        dss.bus_voltages(bus)
            .unwrap_or_else(|| panic!("bus {bus}"))
            .vmag_angle[0]
            .0
    }

    /// The property `obj.prop` as one number (`[ 12]` reads 12).
    pub(crate) fn query_f64(dss: &mut Dss, what: &str) -> f64 {
        dss.command(&format!("? {what}"));
        let r = dss.result().to_string();
        r.trim_matches(|c: char| c == '[' || c == ']' || c.is_whitespace())
            .parse()
            .unwrap_or_else(|e| panic!("{what} = {r:?}: {e}"))
    }

    /// `a` within `rel` of `b`, relative to `|b|`.
    pub(crate) fn assert_rel(a: f64, b: f64, rel: f64, what: &str) {
        assert!(
            (a - b).abs() <= rel * b.abs(),
            "{what}: {a} vs {b} (rel {:.3e})",
            (a - b).abs() / b.abs()
        );
    }

    /// `(1/3)·v1ᴴ M v1` of the `n`-conductor matrix `m` (row-major) on phases
    /// `0..n`, with `v1 = [1, a², a]` and `a = e^{j2π/3}`: the balanced
    /// positive-sequence self term, computed from its definition.
    pub(crate) fn seq11(m: &[Complex64], n: usize) -> Complex64 {
        let a = Complex64::from_polar(1.0, 2.0 * std::f64::consts::PI / 3.0);
        let mut s = Complex64::ZERO;
        for i in 0..n {
            for j in 0..n {
                s += m[i * n + j] * a.powi(i as i32 - j as i32);
            }
        }
        s / 3.0
    }

    /// The inverse of the `n × n` complex matrix `m` (row-major), Gauss-Jordan
    /// with partial pivoting.
    pub(crate) fn inverse(m: &[Complex64], n: usize) -> Vec<Complex64> {
        let mut a = m.to_vec();
        let mut inv: Vec<Complex64> = (0..n * n)
            .map(|k| {
                if k / n == k % n {
                    Complex64::ONE
                } else {
                    Complex64::ZERO
                }
            })
            .collect();
        for col in 0..n {
            let piv = (col..n)
                .max_by(|&p, &q| a[p * n + col].norm().total_cmp(&a[q * n + col].norm()))
                .unwrap();
            for k in 0..n {
                a.swap(col * n + k, piv * n + k);
                inv.swap(col * n + k, piv * n + k);
            }
            let d = a[col * n + col];
            for k in 0..n {
                a[col * n + k] /= d;
                inv[col * n + k] /= d;
            }
            for row in 0..n {
                if row != col {
                    let f = a[row * n + col];
                    for k in 0..n {
                        let (ack, ick) = (a[col * n + k], inv[col * n + k]);
                        a[row * n + k] -= f * ack;
                        inv[row * n + k] -= f * ick;
                    }
                }
            }
        }
        inv
    }

    /// The matrix property `key` of `full` as the JSON export writes it, from
    /// its key to the end of its rows (lower case).
    pub(crate) fn json_matrix(dss: &Dss, full: &str, key: &str) -> String {
        let json = dss
            .obj_to_json(full, crate::report::export::json::JsonOpts::NONE)
            .unwrap_or_else(|| panic!("{full}: no JSON"))
            .to_ascii_lowercase();
        let at = json
            .find(&format!("\"{key}\":"))
            .unwrap_or_else(|| panic!("{key} in {json}"));
        let end = json[at..]
            .find("]]")
            .unwrap_or_else(|| panic!("{key} rows in {json}"));
        json[at..at + end + 2].to_string()
    }

    /// The row-major complex matrix `r + j x`.
    pub(crate) fn complex(r: &[f64], x: &[f64]) -> Vec<Complex64> {
        r.iter()
            .zip(x)
            .map(|(&r, &x)| Complex64::new(r, x))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::pos_seq_self_term;
    use num_complex::Complex64;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-12 * b.abs().max(1.0)
    }

    /// Three conductors: mean of the diagonal minus the mean of the pairs, for a
    /// balanced and an unbalanced matrix alike.
    #[test]
    fn three_conductors_take_mean_self_minus_mean_mutual() {
        let balanced = [10.0, -2.0, -2.0, -2.0, 10.0, -2.0, -2.0, -2.0, 10.0];
        assert!(close(pos_seq_self_term(&balanced, 3), 12.0));
        let unbalanced = [12.0, -3.0, -1.0, -3.0, 10.0, -2.0, -1.0, -2.0, 8.0];
        assert!(close(pos_seq_self_term(&unbalanced, 3), 12.0));
    }

    /// Two conductors embedded in three phases: `(M11 + M22 − M12) / 3`.
    #[test]
    fn two_conductors_take_the_embedded_self_term() {
        let m = [10.0, -2.0, -2.0, 10.0];
        assert!(close(pos_seq_self_term(&m, 2), 22.0 / 3.0));
    }

    /// Above three conductors: mean of the diagonal minus the mean over all
    /// pairs, which the `n` ≤ 3 sum would not give (16 here).
    #[test]
    fn four_conductors_take_mean_self_minus_mean_mutual() {
        let balanced = [
            10.0, -2.0, -2.0, -2.0, -2.0, 10.0, -2.0, -2.0, -2.0, -2.0, 10.0, -2.0, -2.0, -2.0,
            -2.0, 10.0,
        ];
        assert!(close(pos_seq_self_term(&balanced, 4), 12.0));
        let unbalanced = [
            12.0, -3.0, -1.0, -1.0, -3.0, 10.0, -2.0, -1.0, -1.0, -2.0, 8.0, -1.0, -1.0, -1.0,
            -1.0, 9.0,
        ];
        assert!(close(pos_seq_self_term(&unbalanced, 4), 11.25));
    }

    /// A non-symmetric matrix reduces like its symmetric part, the real part of
    /// `(1/3)·v1ᴴ M v1`. The two triangles differ (pair sums −6 above and −8
    /// below the diagonal), so the symmetric part's 37/3 is neither the 12 of
    /// the upper triangle nor the 38/3 of the lower one.
    #[test]
    fn a_non_symmetric_matrix_reduces_like_its_symmetric_part() {
        let m = [10.0, -1.0, -3.0, -3.0, 10.0, -2.0, -1.0, -4.0, 10.0];
        let sym = [10.0, -2.0, -2.0, -2.0, 10.0, -3.0, -2.0, -3.0, 10.0];
        let s = pos_seq_self_term(&m, 3);
        assert!(close(s, 37.0 / 3.0), "{s}");
        assert!(close(s, pos_seq_self_term(&sym, 3)), "{s}");
        let mc: Vec<Complex64> = m.iter().map(|&v| Complex64::new(v, 0.0)).collect();
        let defined = super::balance::seq11(&mc, 3);
        assert!(close(s, defined.re), "{s} vs {defined}");
    }

    /// Complex entries reduce part by part.
    #[test]
    fn complex_entries_reduce_part_by_part() {
        let m: Vec<Complex64> = (0..9)
            .map(|k| {
                if k % 4 == 0 {
                    Complex64::new(1.0, 10.0)
                } else {
                    Complex64::new(0.2, 1.0)
                }
            })
            .collect();
        let s = pos_seq_self_term(&m, 3);
        assert!(close(s.re, 0.8) && close(s.im, 9.0), "{s}");
    }

    /// One conductor has nothing to reduce.
    #[test]
    fn one_conductor_is_its_own_term() {
        assert_eq!(pos_seq_self_term(&[4.0], 1), 4.0);
    }
}
