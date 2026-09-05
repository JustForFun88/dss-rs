//! `GOLDEN_REBASE_PLAN.md` WP-G1 sub-step **G1.9** — the five `Circuit`
//! aggregates and the ten `Solution` scalars, compared live on every gating
//! channel of the unified corpus gate.
//!
//! The surface is **unflagged and universal**: it carries no
//! `G1_SURFACE_FLAGS` entry, no `population_lock.rs::rigor()` token and no
//! forced population — every live case on every gating channel compares it
//! (TESTING.md, "adding a live surface"). Both transports emit the identical
//! JSON (`tools/oracle/oracle_server.py::capture_aggregates` /
//! `capture_solution_scalars`, `crates/dss-epri/src/capture.rs`), so the two
//! capture structs below deserialize one shape.
//!
//! # The three arms, and which one has the teeth
//!
//! * **P1 — membership.** Each loss aggregate is reconstructed from the
//!   **oracle's own** per-element capture over the **port's** summand list
//!   ([`Dss::aggregate_terms`]). A wrongly included or omitted element moves
//!   the reconstruction by that element's *whole* loss, so the arm reads as a
//!   structural error rather than a blurred number. Both sides are the same
//!   f64 sum of the same terms in the same (creation) order — r4133 walks
//!   `PDElements` / `Lines` / `Transformers` in list order
//!   (`Common/Circuit.pas:2436-2444`, `DDLL/DCircuit.pas:313-320`, `:335-342`)
//!   and the port mirrors it — so the admissible difference is
//!   reordering-free: see [`AGG_SUM_REL`] / [`AGG_SUM_ABS`].
//! * **P1b — `AllElementLosses` identity, length, order and value.** The
//!   oracle's own `AllElementLosses[i]` must equal its own
//!   `elements[i].Losses` (`DDLL/DCircuit.pas:466-473` is the same
//!   `Get_Losses` the element capture reads, times `0.001`), the port's
//!   element order must equal the oracle's `AllElementNames` order — coverage
//!   the gate did **not** have before (`runner.rs` compares element names as
//!   `BTreeSet`s) — and the port's own [`Dss::all_element_losses`] must match
//!   the oracle's array element by element.
//! * **P2 — value.** The port's aggregate against the oracle's, inside the
//!   **exact propagation** of the per-element loss floors the gate already
//!   accepts ([`element_loss_allowance_kw`], the envelope
//!   `compare_element_channels` has always used). This introduces no new
//!   tolerance constant. It is a *derived bound, not a calibrated floor*: on
//!   the stiff tiers it is wide, because the summands are already gated
//!   element-by-element upstream of the sum and what an aggregate adds is
//!   **membership, units and aggregation** — which is what P1/P1b and the
//!   in-engine pins (`dss_core::exec::tests::aggregates`) hold. See
//!   `tests/TOLERANCE_NOTES.md` §"G1.9 circuit aggregates".
//!
//! # Ledger-scoped summands
//!
//! An aggregate is a linear functional of per-element quantities the gate
//! already partitions: where `ledger.json` scopes an element's
//! `powers`/`losses` sub-channels on this (case, channel) — the port computes
//! the corrected value and the oracle does not — `LedgerView::element_rewrites`
//! hands the element comparator the **accepted** cap (the Rust values, after
//! the entry's own envelope/pin ran). The **value** arm consumes exactly those
//! accepted caps, so an already-excluded, already-pinned per-element
//! divergence is inherited field-by-field instead of being re-stated as its own
//! `aggregates` ledger row for every deck it touches (measured: 12 cases carry
//! a deck-wide element scope; re-pinning their echo would have cost ~14 rows
//! and tripped the §1.1(f) kill criterion for a divergence the ledger already
//! owns). On a deck where every summand's `losses` is scoped the value arm is
//! then a self-comparison, and that is **inherent, not a slip**: re-stating it
//! against the oracle's own aggregate with the accepted divergence added to the
//! envelope is a tautology by the triangle inequality
//! (`|Σ(r−o)| ≤ Σ|r−a| + |Σ(a−o)|`), so once the ledger owns every summand the
//! aggregate carries no oracle information any comparator could recover. What
//! must therefore not happen silently is the inheritance *spreading*: the
//! deck-wide `element` scopes it applies to are pinned by
//! `corpus_gate::ledger::the_aggregate_value_arms_inherit_exactly_the_recorded_element_scopes`,
//! so a new one reds until its author records it (the D11 visibility rule).
//! `TotalPower` cannot be reconstructed from the cap at all (terminal 1, and
//! the capture has no `nconds`), so instead of being dropped whenever a source
//! appears in the rewrite map — whatever sub-channel the entry scoped — its
//! envelope absorbs the accepted `powers` divergence summed over **all** of
//! that source's conductors, a documented conservative superset. An entry that
//! scopes only `currents` no longer switches the arm off.
//! The **membership and identity arms do not soften**: P1 and P1b run on
//! the raw oracle capture on every case, so no deck loses the arms that carry
//! the teeth. The two `newton*` decks are the lane-policy analogue: their
//! `Powers`/`Losses` sub-channels are dropped in both lanes
//! (`lane::elem_channels_for`, G2.3), so the value arms are dropped with them.
//!
//! Units are in the capture key names because upstream does not scale
//! uniformly: `Circuit.Losses` is raw **W/var** (`DDLL/DCircuit.pas:294` ->
//! `Common/Circuit.pas:2436-2443`), the other four carry their arm's own
//! `cmulreal(..., 0.001)` and are **kW/kvar**.

use std::collections::BTreeMap;

use dss_core::exec::{Dss, ElementSnapshot};
use serde::Deserialize;

use super::{ElemChannels, ElementCap, Tolerances, lane};

/// The five `Circuit` aggregates of one checkpoint, in the shape both
/// transports emit. `all_element_losses_kw` is the flat interleaved
/// `[re0, im0, re1, im1, ...]` of length `2 * NumDevices`, in
/// `AllElementNames` order.
#[derive(Debug, Deserialize)]
pub struct AggregatesCap {
    /// `Circuit.Losses` — **W/var** (`DDLL/DCircuit.pas:294`).
    pub losses_w: Vec<f64>,
    /// `Circuit.LineLosses` — kW/kvar (`DDLL/DCircuit.pas:305-325`).
    pub line_losses_kw: Vec<f64>,
    /// `Circuit.SubstationLosses` — kW/kvar (`DDLL/DCircuit.pas:327-347`).
    pub substation_losses_kw: Vec<f64>,
    /// `Circuit.TotalPower` — kW/kvar (`DDLL/DCircuit.pas:349-368`).
    pub total_power_kw: Vec<f64>,
    /// `Circuit.AllElementLosses` — kW/kvar (`DDLL/DCircuit.pas:458-479`).
    pub all_element_losses_kw: Vec<f64>,
}

/// The ten `Solution` scalars of one checkpoint.
///
/// `iterations` and `dbl_hour` are deliberately **absent** — the checkpoint
/// already carries and the gate already compares them
/// (`corpus_gate/runner.rs`). `total_iterations` is carried but never compared
/// against the port a second time: `SolutionI(40)` literally returns
/// `Solution.Iteration` (r4133 `DDLL/DSolution.pas:218-220`; capi
/// `CAPI_Solution.pas:731-738`, "Same as Iterations interface"), so the field
/// is used here only to assert that oracle-side alias live, corpus-wide. The
/// port-side alias is pinned in-engine by
/// `dss_core::exec::tests::aggregates::total_iterations_is_an_alias_of_iterations`.
#[derive(Debug, Deserialize)]
pub struct SolutionScalarsCap {
    /// `SolutionI(1)` `Solution.Mode` (`DDLL/DSolution.pas:29`).
    pub mode: i32,
    /// `SolutionI(3)` `Solution.Hour` (`:37`) — `DynaVars.intHour`.
    pub hour: i32,
    /// `SolutionI(5)` `Solution.Year` (`:47`).
    pub year: i32,
    /// `SolutionI(22)` `Solution.ControlIterations` (`:113`).
    pub control_iterations: i32,
    /// `SolutionI(40)` `Solution.Totaliterations` (`:218-220`).
    pub total_iterations: i32,
    /// `SolutionI(41)` `Solution.MostIterationsDone` (`:222`).
    pub most_iterations_done: i32,
    /// `SolutionI(42)` `Solution.ControlActionsDone` (`:226-230`) — a `0|1`
    /// int upstream, normalized to a JSON bool at the r4133 bridge.
    pub control_actions_done: bool,
    /// `SolutionI(37)` `Solution.SystemYChanged` (`:192-197`), same `0|1`
    /// normalization.
    pub system_y_changed: bool,
    /// `SolutionF(2)` `Solution.Seconds` (`:312`) — `DynaVars.t`.
    pub seconds: f64,
    /// `SolutionF(6)` `Solution.LoadMult` (`:336`).
    pub load_mult: f64,
}

/// P1's relative band on the aggregate-vs-its-own-summands reconstruction.
///
/// Derivation (`tests/TOLERANCE_NOTES.md` §"G1.9 circuit aggregates"): the
/// oracle's aggregate and the reconstruction are the same f64 accumulation of
/// the same terms in the same order, so the only admissible difference is
/// `N * eps` rounding — `4889 * 2.22e-16 ~= 1.1e-12` relative on the largest
/// corpus deck. Measured corpus-wide (3 493 checkpoints, both channels,
/// 2026-09-04): worst `|recon - oracle| / sum|term|` is `8.259e-16`
/// (`Circuit.Losses`), `1.610e-14` (`LineLosses`) and `1.497e-16`
/// (`SubstationLosses`) — 60x to 1200x inside the band. This is an identity
/// band, not a calibrated floor: it may only ever tighten.
pub const AGG_SUM_REL: f64 = 1e-12;

/// P1's absolute companion to [`AGG_SUM_REL`] — W for `Circuit.Losses`, kW for
/// the two kW-valued aggregates.
pub const AGG_SUM_ABS: f64 = 1e-9;

/// P1b's band on the oracle-internal `AllElementLosses[i] * 1000 ==
/// elements[i].Losses` identity (W). The two differ only by the `x 0.001`
/// round-trip upstream applies to one side and not the other
/// (`DDLL/DCircuit.pas:471` vs `Common/CktElement.pas:707-767`), i.e. one
/// multiply-rounding; measured corpus-wide worst `4.768e-7 W`
/// (`Test/Dynamic_Kundur.dss`, `1.935e-16` relative there) and worst relative
/// `6.449e-16` (`StorageControllerTechNote/PeakShave`) — 1 550x inside the
/// band.
pub const AEL_IDENT_REL: f64 = 1e-12;

/// P1b's absolute companion to [`AEL_IDENT_REL`] (W).
pub const AEL_IDENT_ABS: f64 = 1e-9;

/// `Solution.Seconds` (`DynaVars.t`) floor, seconds.
///
/// **Not a new floor**: it is the same `1e-9` the already-gated `dblHour`
/// compare uses in `corpus_gate/runner.rs` — the two are the same clock
/// (`Update_dblHour` maintains `dblHour` from `intHour` and `t`), so a second
/// value would be incoherent. Named here so the sibling relationship is
/// explicit rather than a repeated literal.
pub const CLOCK_ABS_S: f64 = 1e-9;

/// The per-element loss envelope the gate has always used, extracted so the
/// aggregate comparator propagates the **identical** allowance instead of
/// inventing a second one.
///
/// `losses = sum_k S_k`, so `|delta(losses)| <= sum_k (i_abs * |V_k| + i_rel *
/// |S_k|)` — `assert_power_close`'s voltage-scaled floor summed over the
/// element's conductors, with `|V_k|` recovered as `|S_k| / |I_k|` from the
/// capture (which carries no `nconds`-indexed voltage) and floored at 1 kV so a
/// de-energized conductor still gets the plain current floor. Byte-faithful
/// extraction of the block [`super::compare_element_channels`] ran inline; that
/// function still calls it, so the two can never drift apart.
pub fn element_loss_allowance_kw(exp: &ElementCap, tol: &Tolerances) -> f64 {
    let mut allowed_kw = 0.0;
    for k in 0..exp.p_kw.len() {
        let p_mag = (exp.p_kw[k].powi(2) + exp.p_kvar[k].powi(2)).sqrt();
        let i_mag = (exp.i_re[k].powi(2) + exp.i_im[k].powi(2)).sqrt();
        let vkv = if i_mag > 1e-12 { p_mag / i_mag } else { 1.0 };
        allowed_kw += tol.i_abs * vkv.max(1.0) + tol.i_rel * p_mag;
    }
    allowed_kw
}

/// One P1 reconstruction arm: the aggregate's label, the port's summand names,
/// the oracle's reported value and the W -> aggregate-unit scale.
type ReconArm<'a> = (&'a str, &'a [String], (f64, f64), f64);

/// `|a - b|` as a complex magnitude.
fn cdiff(a: (f64, f64), b: (f64, f64)) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// The oracle's `Get_Losses` (W) for one summand, by lower-cased `Class.name`.
///
/// A term the port walks but the oracle never captured is a **protocol**
/// failure, not a divergence: `runner.rs` has already asserted the two element
/// name sets are equal, so the only way to land here is a capture that dropped
/// an element or a `loss_w` the transport failed to fill.
fn term_loss_w(
    by_name: &BTreeMap<String, (f64, f64)>,
    term: &str,
    agg: &str,
    ctx: &str,
) -> (f64, f64) {
    *by_name.get(term).unwrap_or_else(|| {
        panic!(
            "{ctx}: {agg} summand `{term}` has no oracle per-element capture \
             (the element name sets matched, so this is a capture protocol \
             failure, not a divergence)"
        )
    })
}

/// Compare one checkpoint's five circuit aggregates — arms P1, P1b and P2.
///
/// `channels` is [`lane::elem_channels_for`]'s verdict for the case: on the two
/// `newton*` decks the per-element `Powers`/`Losses` sub-channels are dropped in
/// **both** lanes (`GOLDEN_REBASE_PLAN.md` G2.3 — no oracle rev reports them at
/// the converged `NodeV`), and an aggregate built out of exactly those numbers
/// must be dropped on the same decks rather than get a second skip list.
/// `accepted` is the runner's `LedgerView::element_rewrites` map (empty when no
/// ledger entry applies): the value arms sum the accepted per-element caps, so
/// a ledger-scoped element contributes the value the element comparator itself
/// accepted. Only the **value** arms consult either of the two: the membership,
/// identity, length and order arms are structure over the raw oracle capture
/// and run everywhere.
#[allow(clippy::too_many_arguments)]
pub fn compare_aggregates(
    dss: &mut Dss,
    cap: &AggregatesCap,
    snaps: &[ElementSnapshot],
    elements: &[ElementCap],
    accepted: &BTreeMap<String, ElementCap>,
    tol: &Tolerances,
    channels: ElemChannels,
    ctx: &str,
) {
    // --- shape -----------------------------------------------------------
    for (name, v) in [
        ("losses_w", &cap.losses_w),
        ("line_losses_kw", &cap.line_losses_kw),
        ("substation_losses_kw", &cap.substation_losses_kw),
        ("total_power_kw", &cap.total_power_kw),
    ] {
        assert_eq!(
            v.len(),
            2,
            "{ctx}: aggregate `{name}` is not a complex pair: {v:?}"
        );
    }
    assert_eq!(
        cap.all_element_losses_kw.len(),
        2 * elements.len(),
        "{ctx}: AllElementLosses holds {} doubles for {} captured elements \
         (upstream returns one complex per device, `DDLL/DCircuit.pas:461-473`)",
        cap.all_element_losses_kw.len(),
        elements.len()
    );

    // --- P1b, order ------------------------------------------------------
    // New coverage: `runner.rs` compares the element name SETS, so creation
    // order has never been checked. `AllElementLosses` is indexed by that order
    // on the oracle side (`AllElementNames`), so the port's `ckt_elements` order
    // must equal it or every value arm below compares the wrong pairs.
    assert_eq!(
        snaps.len(),
        elements.len(),
        "{ctx}: Rust snapshot holds {} elements, the oracle captured {}",
        snaps.len(),
        elements.len()
    );
    for (i, (s, e)) in snaps.iter().zip(elements).enumerate() {
        assert!(
            s.name.eq_ignore_ascii_case(&e.name),
            "{ctx}: element ORDER differs at index {i}: Rust `{}` vs oracle \
             `{}` (the port's `ckt_elements` creation order must equal the \
             oracle's `AllElementNames` order — `AllElementLosses` is indexed \
             by it)",
            s.name,
            e.name
        );
    }

    // --- P1b, oracle-internal identity ------------------------------------
    // `AllElementLosses[i]` and `elements[i].Losses` are the same `Get_Losses`
    // read one `x 0.001` apart, so they must agree to a multiply-rounding.
    for (i, e) in elements.iter().enumerate() {
        assert_eq!(
            e.loss_w.len(),
            2,
            "{ctx}: element `{}` has no `loss_w` in the live capture",
            e.name
        );
        let ael = (
            cap.all_element_losses_kw[2 * i] * 1000.0,
            cap.all_element_losses_kw[2 * i + 1] * 1000.0,
        );
        let own = (e.loss_w[0], e.loss_w[1]);
        let d = cdiff(ael, own);
        let bound = AEL_IDENT_REL * own.0.hypot(own.1) + AEL_IDENT_ABS;
        assert!(
            d <= bound,
            "{ctx}: oracle AllElementLosses[{i}] ({}) = ({}, {}) W disagrees \
             with its own CktElement.Losses ({}, {}) W: |diff| = {d:e} > {bound:e}",
            e.name,
            ael.0,
            ael.1,
            own.0,
            own.1
        );
    }

    // --- P1, membership ---------------------------------------------------
    let by_name: BTreeMap<String, (f64, f64)> = elements
        .iter()
        .map(|e| (e.name.to_lowercase(), (e.loss_w[0], e.loss_w[1])))
        .collect();
    let terms = dss.aggregate_terms();
    // (label, port summand names, the oracle's aggregate, W->unit scale)
    let recon_arms: [ReconArm<'_>; 3] = [
        (
            "Circuit.Losses",
            &terms.losses,
            (cap.losses_w[0], cap.losses_w[1]),
            1.0,
        ),
        (
            "Circuit.LineLosses",
            &terms.line_losses,
            (cap.line_losses_kw[0], cap.line_losses_kw[1]),
            0.001,
        ),
        (
            "Circuit.SubstationLosses",
            &terms.substation_losses,
            (cap.substation_losses_kw[0], cap.substation_losses_kw[1]),
            0.001,
        ),
    ];
    for (agg, term_names, oracle, scale) in recon_arms {
        let mut recon = (0.0f64, 0.0f64);
        let mut sum_abs = 0.0f64;
        for t in term_names {
            let (r, i) = term_loss_w(&by_name, t, agg, ctx);
            recon.0 += r * scale;
            recon.1 += i * scale;
            sum_abs += r.hypot(i) * scale;
        }
        let d = cdiff(recon, oracle);
        let bound = AGG_SUM_REL * sum_abs + AGG_SUM_ABS;
        assert!(
            d <= bound,
            "{ctx}: {agg} membership: reconstructing the ORACLE's aggregate \
             from its OWN per-element losses over the port's {} summand(s) \
             gives ({}, {}) but the oracle reports ({}, {}); |diff| = {d:e} > \
             {bound:e} (sum|term| = {sum_abs:e}). The summand SET differs — a \
             wrongly included or omitted element, not a numeric drift.",
            term_names.len(),
            recon.0,
            recon.1,
            oracle.0,
            oracle.1
        );
    }

    // --- P2, value --------------------------------------------------------
    // The reference is the ACCEPTED per-element capture: the oracle's own value
    // for every element, replaced by the ledger's rewrite where an entry scopes
    // that element's `powers`/`losses` sub-channels on this (case, channel).
    // That is the same cap `compare_element_channels` is handed one loop above,
    // so the aggregate inherits the partition instead of re-pinning its echo
    // (see the module doc). P1 has just proved, on the RAW capture, that the
    // oracle's aggregate IS the sum of its own summands to <=1e-12 relative, so
    // where no entry applies this reference is the oracle's aggregate.
    let cap_for = |i: usize| -> &ElementCap {
        accepted
            .get(&elements[i].name.to_lowercase())
            .unwrap_or(&elements[i])
    };
    let idx_of = |t: &str, agg: &str| -> usize {
        elements
            .iter()
            .position(|e| e.name.eq_ignore_ascii_case(t))
            .unwrap_or_else(|| {
                panic!("{ctx}: {agg} summand `{t}` has no oracle per-element capture")
            })
    };
    // The allowance is the exact propagation of the per-element floors the gate
    // already accepts: no new constant, and no aggregate can pass on a floor its
    // own summands would fail.
    let allowance_kw = |term_names: &[String], agg: &str| -> f64 {
        term_names
            .iter()
            .map(|t| element_loss_allowance_kw(cap_for(idx_of(t, agg)), tol))
            .sum()
    };
    // The accepted sum of one aggregate's summands, in the aggregate's own unit.
    let accepted_sum = |term_names: &[String], agg: &str, scale: f64| -> (f64, f64) {
        let mut s = (0.0f64, 0.0f64);
        for t in term_names {
            let e = cap_for(idx_of(t, agg));
            s.0 += e.loss_w[0] * scale;
            s.1 += e.loss_w[1] * scale;
        }
        s
    };
    if channels.losses {
        let rust_losses = dss.losses();
        let oracle_losses = accepted_sum(&terms.losses, "Circuit.Losses", 1.0);
        let allowed_w = allowance_kw(&terms.losses, "Circuit.Losses") * 1000.0;
        let d = cdiff(rust_losses, oracle_losses);
        assert!(
            d <= allowed_w,
            "{ctx}: Circuit.Losses differs: Rust ({}, {}) W vs oracle ({}, {}) W; \
             |diff| = {d:e} > allowed {allowed_w:e} (the propagated per-element \
             envelope over {} summands)",
            rust_losses.0,
            rust_losses.1,
            oracle_losses.0,
            oracle_losses.1,
            terms.losses.len()
        );

        let rust_line = dss.line_losses();
        let oracle_line = accepted_sum(&terms.line_losses, "Circuit.LineLosses", 0.001);
        let allowed = allowance_kw(&terms.line_losses, "Circuit.LineLosses");
        let d = cdiff(rust_line, oracle_line);
        assert!(
            d <= allowed,
            "{ctx}: Circuit.LineLosses differs: Rust ({}, {}) kW vs oracle \
             ({}, {}) kW; |diff| = {d:e} > allowed {allowed:e} ({} lines)",
            rust_line.0,
            rust_line.1,
            oracle_line.0,
            oracle_line.1,
            terms.line_losses.len()
        );

        let rust_sub = dss.substation_losses();
        let oracle_sub = accepted_sum(&terms.substation_losses, "Circuit.SubstationLosses", 0.001);
        let allowed = allowance_kw(&terms.substation_losses, "Circuit.SubstationLosses");
        let d = cdiff(rust_sub, oracle_sub);
        assert!(
            d <= allowed,
            "{ctx}: Circuit.SubstationLosses differs: Rust ({}, {}) kW vs oracle \
             ({}, {}) kW; |diff| = {d:e} > allowed {allowed:e} ({} substation \
             transformers; AutoTrans never contributes, `Common/Circuit.pas:2272-2273`)",
            rust_sub.0,
            rust_sub.1,
            oracle_sub.0,
            oracle_sub.1,
            terms.substation_losses.len()
        );

        // `AllElementLosses`, port vs oracle, element by element — the only
        // oracle-facing gate on `Dss::all_element_losses` itself.
        let rust_ael = dss.all_element_losses();
        assert_eq!(
            rust_ael.len(),
            elements.len(),
            "{ctx}: Rust AllElementLosses holds {} slots for {} elements",
            rust_ael.len(),
            elements.len()
        );
        for (i, (r, e)) in rust_ael.iter().zip(elements).enumerate() {
            let acc = cap_for(i);
            let o = (acc.loss_w[0] * 0.001, acc.loss_w[1] * 0.001);
            let allowed = element_loss_allowance_kw(acc, tol);
            let d = cdiff(*r, o);
            assert!(
                d <= allowed,
                "{ctx}: AllElementLosses[{i}] ({}) differs: Rust ({}, {}) kW vs \
                 oracle ({}, {}) kW; |diff| = {d:e} > allowed {allowed:e}",
                e.name,
                r.0,
                r.1,
                o.0,
                o.1
            );
        }
    }

    // `TotalPower` sums `Power[1]` — a per-TERMINAL quantity the capture does
    // not split out (it carries no `nconds`), so unlike the loss aggregates it
    // cannot be rebuilt from an accepted per-element cap. What the ledger
    // accepted for a scoped source is still *bounded*, though: the sum over
    // ALL of its conductors of |accepted - raw| dominates the terminal-1 part,
    // the same conservative superset the allowance itself uses. Adding that to
    // the envelope keeps the arm running field-by-field on a scoped deck,
    // where the earlier `tp_scoped` test dropped the whole comparison on
    // element PRESENCE — whatever sub-channel the entry actually scoped
    // (G1.9 audit CODE-2). Membership and the terminal-1/x3 semantics stay
    // pinned in-engine by
    // `exec::tests::aggregates::total_power_is_terminal_one_of_every_source`.
    let tp_slack: f64 = terms
        .total_power
        .iter()
        .map(|t| {
            let i = idx_of(t, "Circuit.TotalPower");
            let raw = &elements[i];
            let acc = cap_for(i);
            let n = raw
                .p_kw
                .len()
                .min(raw.p_kvar.len())
                .min(acc.p_kw.len())
                .min(acc.p_kvar.len());
            (0..n)
                .map(|k| cdiff((acc.p_kw[k], acc.p_kvar[k]), (raw.p_kw[k], raw.p_kvar[k])))
                .sum::<f64>()
        })
        .sum();
    if channels.powers {
        let rust_tp = dss.total_power();
        let oracle_tp = (cap.total_power_kw[0], cap.total_power_kw[1]);
        // The allowance uses each source's WHOLE conductor set — terminal 1 is
        // a subset, so this is a documented conservative superset.
        let allowed = allowance_kw(&terms.total_power, "Circuit.TotalPower") + tp_slack;
        let d = cdiff(rust_tp, oracle_tp);
        assert!(
            d <= allowed,
            "{ctx}: Circuit.TotalPower differs: Rust ({}, {}) kW vs oracle \
             ({}, {}) kW; |diff| = {d:e} > allowed {allowed:e} (the propagated \
             per-source envelope plus {tp_slack:e} the ledger accepted on \
             their `powers`) ({} sources)",
            rust_tp.0,
            rust_tp.1,
            oracle_tp.0,
            oracle_tp.1,
            terms.total_power.len()
        );
    }
}

/// Compare one checkpoint's ten `Solution` scalars.
///
/// `oracle_iterations` is the checkpoint's already-compared `Iterations`, used
/// only to assert the oracle-side `Totaliterations == Iterations` alias
/// (`DDLL/DSolution.pas:218-220`) live on every case — the port side of that
/// alias is an in-engine pin, so the port is never compared against
/// `Totaliterations` a second time. `iterations_exact` is the channel's
/// iteration contract (`EngineChannel::iterations_exact`).
///
/// Policy, all of it reused rather than invented:
/// * `mode`, `hour`, `year`, `control_actions_done`, `system_y_changed` —
///   discrete state, exact in **both** lanes (CLAUDE.md).
/// * `load_mult` — a user/mode-set scalar the engine never computes; exact.
/// * `seconds` — [`CLOCK_ABS_S`], the `dblHour` constant's sibling.
/// * `control_iterations` — exact on an exact-contract channel, `rust <=
///   oracle` on r4133, in **both** lanes: the `ITER_SLACK` drift model is about
///   the inner power-flow convergence boundary and does not transfer to
///   control-loop passes, so a difference here is a control-loop divergence.
/// * `most_iterations_done` — [`lane::compare_iterations`] /
///   [`lane::compare_iterations_le`], the existing helpers: it is a max over
///   the step's inner solves (`Common/Solution.pas:2568` resets it per step,
///   `:2701` raises it), so it inherits exactly the inner count's policy.
pub fn compare_solution_scalars(
    dss: &Dss,
    cap: &SolutionScalarsCap,
    oracle_iterations: i32,
    iterations_exact: bool,
    ctx: &str,
) {
    let ckt = dss.circuit().expect("circuit exists");
    let sol = &ckt.solution;

    // The oracle-side alias, asserted live on every case and channel instead of
    // comparing the port against `Totaliterations` a second time.
    assert_eq!(
        cap.total_iterations, oracle_iterations,
        "{ctx}: the oracle's Totaliterations ({}) is no longer its own \
         Iterations ({oracle_iterations}) — `DDLL/DSolution.pas:218-220` \
         returns `Solution.Iteration` verbatim, so this equivalence (recorded \
         in TESTING.md instead of being compared twice) has broken",
        cap.total_iterations
    );

    assert_eq!(
        sol.mode.ordinal(),
        cap.mode,
        "{ctx}: Solution.Mode {:?} (ordinal {}) vs oracle {}",
        sol.mode,
        sol.mode.ordinal(),
        cap.mode
    );
    assert_eq!(
        sol.int_hour, cap.hour,
        "{ctx}: Solution.Hour {} vs oracle {}",
        sol.int_hour, cap.hour
    );
    assert_eq!(
        sol.year, cap.year,
        "{ctx}: Solution.Year {} vs oracle {}",
        sol.year, cap.year
    );
    assert_eq!(
        sol.control_actions_done, cap.control_actions_done,
        "{ctx}: Solution.ControlActionsDone {} vs oracle {}",
        sol.control_actions_done, cap.control_actions_done
    );
    assert_eq!(
        sol.system_y_changed, cap.system_y_changed,
        "{ctx}: Solution.SystemYChanged {} vs oracle {} — the two engines \
         disagree on whether Y must be rebuilt after this solve, which is a \
         rebuild-scheduling divergence, not a numeric one",
        sol.system_y_changed, cap.system_y_changed
    );
    assert_eq!(
        ckt.load_multiplier, cap.load_mult,
        "{ctx}: Solution.LoadMult {} vs oracle {}",
        ckt.load_multiplier, cap.load_mult
    );
    assert!(
        (sol.t - cap.seconds).abs() < CLOCK_ABS_S,
        "{ctx}: Solution.Seconds {} vs oracle {} (floor {CLOCK_ABS_S:e} s — the \
         dblHour constant's sibling)",
        sol.t,
        cap.seconds
    );

    if iterations_exact {
        assert_eq!(
            sol.control_iteration, cap.control_iterations,
            "{ctx}: Solution.ControlIterations {} vs oracle {} — the control \
             loop took a different number of passes; the ITER_SLACK drift model \
             covers the inner power-flow boundary only and does not apply here",
            sol.control_iteration, cap.control_iterations
        );
        lane::compare_iterations(
            sol.most_iterations_done,
            cap.most_iterations_done,
            &format!("{ctx} MostIterationsDone"),
        );
    } else {
        assert!(
            sol.control_iteration <= cap.control_iterations,
            "{ctx}: Solution.ControlIterations {} > the r4133 oracle's {} — the \
             port may settle the control loop in FEWER passes, never more",
            sol.control_iteration,
            cap.control_iterations
        );
        lane::compare_iterations_le(
            sol.most_iterations_done,
            cap.most_iterations_done,
            &format!("{ctx} MostIterationsDone"),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cap(
        p_kw: &[f64],
        p_kvar: &[f64],
        i_re: &[f64],
        i_im: &[f64],
        loss: (f64, f64),
    ) -> ElementCap {
        ElementCap {
            name: "Line.l1".into(),
            i_re: i_re.to_vec(),
            i_im: i_im.to_vec(),
            p_kw: p_kw.to_vec(),
            p_kvar: p_kvar.to_vec(),
            loss_w: vec![loss.0, loss.1],
            // The G1.3a derived channels are not part of this fixture: it
            // exercises `compare_aggregates`, which never reads them. Spelled
            // out rather than defaulted so a future channel has to be
            // considered here too.
            enabled: None,
            cma_mag: Vec::new(),
            cma_ang: Vec::new(),
            res_mag: Vec::new(),
            res_ang: Vec::new(),
            vma_mag: Vec::new(),
            vma_ang: Vec::new(),
            // The G1.3d(i) element-extras channels are likewise unread by
            // `compare_aggregates`; spelled out for the same reason.
            n_terms: None,
            n_conds: None,
            n_phases: None,
            energy_meter: None,
            node_order: Vec::new(),
        }
    }

    fn tol() -> Tolerances {
        super::super::tol_for("feeder")
    }

    /// Build + solve a two-bus feeder and hand `compare_aggregates` a PERFECT
    /// oracle capture built from the port's own numbers, so only the arm under
    /// test can red.
    fn solved_case() -> (Dss, Vec<ElementSnapshot>, Vec<ElementCap>, AggregatesCap) {
        let mut dss = Dss::new();
        for line in [
            "Clear",
            "New Circuit.aggtest basekv=12.47 phases=3 bus1=sourcebus mvasc3=200 mvasc1=210",
            "New Line.l1 bus1=sourcebus bus2=b1 phases=3 r1=0.3 x1=0.9 r0=0.9 x0=2.7 length=2 units=km",
            "New Load.ld1 bus1=b1 phases=3 kv=12.47 kw=900 pf=0.92 model=1",
            "Set voltagebases=[12.47]",
            "Calcvoltagebases",
            "Solve",
        ] {
            dss.command(line);
            assert!(dss.errors().is_empty(), "`{line}` -> {:?}", dss.errors());
        }
        let snaps = dss.snapshot_elements();
        let elements: Vec<ElementCap> = snaps
            .iter()
            .map(|s| ElementCap {
                name: s.name.clone(),
                i_re: s.currents.iter().map(|c| c.re).collect(),
                i_im: s.currents.iter().map(|c| c.im).collect(),
                p_kw: s.powers.iter().map(|c| c.re).collect(),
                p_kvar: s.powers.iter().map(|c| c.im).collect(),
                loss_w: vec![s.loss_w.0, s.loss_w.1],
                // The G1.3a derived channels are not part of this fixture: it
                // exercises `compare_aggregates`, which never reads them. Spelled
                // out rather than defaulted so a future channel has to be
                // considered here too.
                enabled: None,
                cma_mag: Vec::new(),
                cma_ang: Vec::new(),
                res_mag: Vec::new(),
                res_ang: Vec::new(),
                vma_mag: Vec::new(),
                vma_ang: Vec::new(),
                // The G1.3d(i) element-extras channels are likewise unread by
                // `compare_aggregates`; spelled out for the same reason.
                n_terms: None,
                n_conds: None,
                n_phases: None,
                energy_meter: None,
                node_order: Vec::new(),
            })
            .collect();
        let losses = dss.losses();
        let line_losses = dss.line_losses();
        let sub = dss.substation_losses();
        let tp = dss.total_power();
        let mut ael = Vec::new();
        for (re, im) in dss.all_element_losses() {
            ael.push(re);
            ael.push(im);
        }
        let cap = AggregatesCap {
            losses_w: vec![losses.0, losses.1],
            line_losses_kw: vec![line_losses.0, line_losses.1],
            substation_losses_kw: vec![sub.0, sub.1],
            total_power_kw: vec![tp.0, tp.1],
            all_element_losses_kw: ael,
        };
        (dss, snaps, elements, cap)
    }

    /// A perfect capture passes every arm — the baseline the two drives below
    /// are measured against (without it a `should_panic` proves nothing).
    #[test]
    fn a_perfect_capture_passes_every_arm() {
        let (mut dss, snaps, elements, cap) = solved_case();
        compare_aggregates(
            &mut dss,
            &cap,
            &snaps,
            &elements,
            &BTreeMap::new(),
            &tol(),
            ElemChannels::ALL,
            "self-test",
        );
    }

    /// A ledger scope that rewrites a source's **currents** must NOT switch the
    /// `Circuit.TotalPower` value arm off: the arm keyed on the source merely
    /// appearing in the rewrite map until the G1.9 audit settlement (CODE-2),
    /// which is not field-by-field — `powers` was never excluded here. With the
    /// old test this drive passed silently.
    #[test]
    #[should_panic(expected = "Circuit.TotalPower differs")]
    fn a_currents_only_scope_leaves_the_total_power_arm_running() {
        let (mut dss, snaps, elements, mut cap) = solved_case();
        // The oracle's TotalPower is far from the port's.
        cap.total_power_kw[0] += 1.0e6;
        // …and the ledger rewrote the source's CURRENTS only.
        let src = elements
            .iter()
            .find(|e| e.name.to_lowercase().starts_with("vsource."))
            .expect("the deck has a Vsource");
        let mut rewritten = ElementCap {
            name: src.name.clone(),
            i_re: src.i_re.clone(),
            i_im: src.i_im.clone(),
            p_kw: src.p_kw.clone(),
            p_kvar: src.p_kvar.clone(),
            loss_w: src.loss_w.clone(),
            // The G1.3a derived channels are not part of this fixture: it
            // exercises `compare_aggregates`, which never reads them. Spelled
            // out rather than defaulted so a future channel has to be
            // considered here too.
            enabled: None,
            cma_mag: Vec::new(),
            cma_ang: Vec::new(),
            res_mag: Vec::new(),
            res_ang: Vec::new(),
            vma_mag: Vec::new(),
            vma_ang: Vec::new(),
            // The G1.3d(i) element-extras channels are likewise unread by
            // `compare_aggregates`; spelled out for the same reason.
            n_terms: None,
            n_conds: None,
            n_phases: None,
            energy_meter: None,
            node_order: Vec::new(),
        };
        for v in &mut rewritten.i_re {
            *v += 1.0;
        }
        let accepted = BTreeMap::from([(src.name.to_lowercase(), rewritten)]);
        compare_aggregates(
            &mut dss,
            &cap,
            &snaps,
            &elements,
            &accepted,
            &tol(),
            ElemChannels::ALL,
            "self-test",
        );
    }

    /// The extracted envelope must still be the formula
    /// `compare_element_channels` used inline: `sum_k (i_abs * max(1, |S_k| /
    /// |I_k|) + i_rel * |S_k|)`.
    #[test]
    fn the_loss_allowance_is_the_summed_voltage_scaled_conductor_floor() {
        let t = tol();
        // One conductor: |S| = 5 kVA at |I| = 0.5 A => |V| = 10 kV.
        let e = cap(&[3.0], &[4.0], &[0.5], &[0.0], (0.0, 0.0));
        let expected = t.i_abs * 10.0 + t.i_rel * 5.0;
        let got = element_loss_allowance_kw(&e, &t);
        assert!(
            (got - expected).abs() <= 1e-15 * expected.abs(),
            "{got} vs {expected}"
        );
    }

    /// A de-energized conductor (|I| below the 1e-12 A guard) keeps the plain
    /// `i_abs` floor rather than dividing by ~0.
    #[test]
    fn a_dead_conductor_falls_back_to_the_unscaled_floor() {
        let t = tol();
        let e = cap(&[0.0], &[0.0], &[0.0], &[0.0], (0.0, 0.0));
        assert_eq!(element_loss_allowance_kw(&e, &t), t.i_abs);
    }

    /// The floor scales with the conductor count — an aggregate over N
    /// elements can never be tighter than the sum of their own floors.
    #[test]
    fn the_allowance_accumulates_over_conductors() {
        let t = tol();
        let one = cap(&[3.0], &[4.0], &[0.5], &[0.0], (0.0, 0.0));
        let two = cap(
            &[3.0, 3.0],
            &[4.0, 4.0],
            &[0.5, 0.5],
            &[0.0, 0.0],
            (0.0, 0.0),
        );
        let a1 = element_loss_allowance_kw(&one, &t);
        let a2 = element_loss_allowance_kw(&two, &t);
        assert!(
            (a2 - 2.0 * a1).abs() <= 1e-15 * a2,
            "{a2} is not twice {a1}"
        );
    }
}
