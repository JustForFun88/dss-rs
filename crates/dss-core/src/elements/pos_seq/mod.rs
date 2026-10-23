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

use num_complex::Complex64;
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
    /// The property index and the value text, written through the edit path
    /// of a DSS `edit` with its side effects.
    SetText(usize, String),
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

/// What a leg of [`PosSeqLeg`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PosSeqLegKind {
    /// One phase of one transformer winding: an ideal-transformer leg of rated
    /// voltage `v_leg`.
    Winding,
    /// Two conductors that carry one value, as a Line conductor does: the
    /// conductor pairs of a series element from one terminal to the next.
    Through,
    /// A branch of a shunt or series element, or a leg of a power conversion
    /// element.
    Lumped,
}

/// One leg of an element, read by the MakePosSequence analysis
/// ([`CktElement::pos_seq_legs`]).
///
/// `plus` and `minus` are conductor indices over all terminals in order:
/// terminal 2's conductor `k` is `nconds + k`. `minus: None` is earth.
/// `winding` is 0 and `phase` the leg index for a leg of any kind but
/// [`PosSeqLegKind::Winding`]. `v_leg` is the rated leg voltage in volts, 0
/// where the element has none. `s_leg` is the complex power in volt-amperes
/// the leg draws at `v_leg` in the element's nominal admittance, generation
/// negative, 0 for a branch given by impedance: the power its own settings
/// give, before the load multiplier, growth and shape factors and the
/// switching state of its steps scale its Y stamp. A PVSystem and a Storage
/// read the admittance their stamp holds at their present output and state,
/// and a grid-forming unit reads 0, as their own docs state.
///
/// [`CktElement::pos_seq_legs`]: crate::elements::traits::CktElement::pos_seq_legs
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PosSeqLeg {
    pub kind: PosSeqLegKind,
    pub winding: usize,
    pub phase: usize,
    pub plus: usize,
    pub minus: Option<usize>,
    pub v_leg: f64,
    pub s_leg: Complex64,
}

pub mod legs;
pub mod project;

/// Power-balance helpers for the `make_pos_sequence` pins: a stiff source bus,
/// a reduction, and the element powers before and after it.
#[cfg(test)]
pub(crate) mod balance;

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
