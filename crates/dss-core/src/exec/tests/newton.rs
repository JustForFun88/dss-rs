//! `Set algorithm=Newton` — the expected-value pin of CLAUDE.md upstream bug 5
//! (`GOLDEN_REBASE_PLAN.md` G2.3) and the in-engine dispatch tripwire that
//! carries the gate signal both lanes give up with it.
//!
//! **The bug.** `DoNewtonSolution`'s final `SumAllCurrents` stamps every
//! element's `Iterminal` from the pre-final voltage guess `NodeV_{n-1}` and
//! marks it solved for the current `SolutionCount`; `NodeV -= dV` follows. So a
//! post-solve `Get_Powers`/`Get_Losses` (cache-aware `ComputeIterminal`) reads a
//! one-step-stale current while `Currents` (`GetCurrents`) recomputes fresh —
//! upstream reports `S != V·conj(I)` for the same element in the same read, in
//! every vendored official rev (v9.8/r3723, v10.2/r4088, v11.0/r4133, all
//! fingerprint 0.478 kVA, checked 2026-07-08). Both lanes now recompute all
//! three reads at `NodeV_n` (`exec::view::snapshot_elements`).
//!
//! **Why the deck is the corpus feeder.** `modes/newton/newton.dss` is the gated
//! case whose Powers/Losses channel both lanes now exclude
//! (`tests/harness/lane.rs::LANE_SKIP_ELEM_POWERS`, unconditional since G2.3 —
//! no oracle channel reports it correctly), so the fix is pinned here on exactly
//! the model that stopped being oracle-compared there. Its content is inlined
//! rather than read from `tests/corpus/` because these are `src` unit tests.

use crate::exec::Dss;

/// The `modes/newton/newton.dss` corpus feeder, inline (3-bus, two lines, three
/// load models — the deck both gating oracles solve in 2 Newton iterations).
const DECK: &[&str] = &[
    "Clear",
    "New Circuit.gaps_newton basekv=12.47 phases=3 bus1=sourcebus mvasc3=200 mvasc1=210",
    "New Line.l1 bus1=sourcebus bus2=b1 phases=3 r1=0.30 x1=0.90 r0=0.9 x0=2.7 c1=3.0 c0=1.5 length=2 units=km",
    "New Line.l2 bus1=b1 bus2=b2 phases=3 r1=0.35 x1=0.95 r0=1.0 x0=2.8 c1=3.0 c0=1.5 length=1.5 units=km",
    "New Load.ld1 bus1=b1 phases=3 kv=12.47 kw=800 pf=0.95 model=1",
    "New Load.ld2 bus1=b2 phases=3 kv=12.47 kw=400 pf=0.90 model=2",
    "New Load.ld3 bus1=b2 phases=1 kv=7.2 kw=150 pf=0.90 model=5",
    "Set voltagebases=[12.47]",
    "Calcvoltagebases",
];

/// Build and solve the deck under `algorithm`.
fn solve_with(algorithm: &str) -> Dss {
    let mut dss = Dss::new();
    for line in DECK {
        dss.command(line);
        assert!(dss.errors().is_empty(), "`{line}` -> {:?}", dss.errors());
    }
    dss.command(&format!("Set algorithm={algorithm}"));
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert!(
        dss.circuit().unwrap().is_solved,
        "{algorithm} did not solve"
    );
    dss
}

// EXPECTED-VALUE-PIN(POWERS_REUSE_STALE_NEWTON_ITERMINAL): after a Newton solve
// the reported powers and losses are the *normal* algorithm's — asserted
// outright in both lanes, where upstream and all three vendored official revs
// report a one-Newton-step-stale current instead.
/// The CLAUDE.md upstream bug #5, torn down by `GOLDEN_REBASE_PLAN.md` G2.3,
/// pinned against the algorithm that has no cache to be stale.
///
/// Newton and the normal fixed point land on the same voltages (measured: max
/// |ΔV| = 7.6e-12 V, both in 2 iterations), and the *normal* solve leaves no
/// valid `Iterminal` cache, so its reported powers are fresh by construction.
/// Newton-vs-normal on the same deck is therefore a direct read of the stale
/// current, in physical units:
///
/// | quantity | upstream / the pre-G2.3 parity lane | both lanes today |
/// |---|---|---|
/// | worst per-conductor \|ΔS\| | **5.283e-1 kVA** (`Vsource.source`) | 3.256e-11 kVA |
/// | worst \|Δlosses\| | **6.248e2 W** | 3.329e-8 W |
/// | worst \|ΔI\| | 4.547e-12 A | 4.547e-12 A |
///
/// Ten orders of magnitude apart, so the bounds below sit far from both the
/// value asserted and the value refused: a re-introduced stale read fails this
/// test in whichever lane it is compiled into. The currents row is the control
/// — `Currents` always recomputed fresh, in every lane and every revision, which
/// is exactly why upstream's `S != V·conj(I)` after a Newton solve is a bug and
/// not a convention.
#[test]
fn newton_powers_match_the_normal_algorithm() {
    let mut newton = solve_with("Newton");
    let mut normal = solve_with("Normal");
    assert_eq!(
        newton.circuit().unwrap().solution.iteration,
        normal.circuit().unwrap().solution.iteration,
        "the two algorithms must converge in the same iteration count on this \
         deck — the premise of the comparison below"
    );

    let sn = newton.snapshot_elements();
    let so = normal.snapshot_elements();
    assert_eq!(sn.len(), so.len());
    let (mut worst_s, mut worst_i, mut worst_loss) = (0.0_f64, 0.0_f64, 0.0_f64);
    let mut worst_s_at = String::new();
    for (a, b) in sn.iter().zip(&so) {
        assert_eq!(a.name, b.name, "snapshot order");
        for (x, y) in a.powers.iter().zip(&b.powers) {
            if (x - y).norm() > worst_s {
                worst_s = (x - y).norm();
                worst_s_at.clone_from(&a.name);
            }
        }
        for (x, y) in a.currents.iter().zip(&b.currents) {
            worst_i = worst_i.max((x - y).norm());
        }
        worst_loss = worst_loss
            .max((a.loss_w.0 - b.loss_w.0).abs())
            .max((a.loss_w.1 - b.loss_w.1).abs());
    }
    assert!(!worst_s_at.is_empty(), "no element was compared");

    // The control: terminal currents are fresh under both algorithms in both
    // lanes, so they agree at the cross-algorithm voltage floor.
    assert!(
        worst_i < 1e-9,
        "reported currents must be algorithm-independent in both lanes, got \
         {worst_i:.3e} A"
    );

    assert!(
        worst_s < 1e-8 && worst_loss < 1e-5,
        "both lanes must report S = V·conj(I) under Newton too (measured \
         3.256e-11 kVA / 3.329e-8 W; the upstream stale read is 5.283e-1 kVA at \
         Vsource.source / 6.248e2 W); got {worst_s:.3e} kVA at {worst_s_at} / \
         {worst_loss:.3e} W"
    );
}

// EXPECTED-VALUE-PIN(POWERS_REUSE_STALE_NEWTON_ITERMINAL): the same torn-down
// row on
// the `TotalPowers` surface (`GOLDEN_REBASE_PLAN.md` G1.3c). `TotalPowers` sums
// `GetPhasePower`'s conductor slots per terminal (r4133
// `DDLL/DCktElement.pas:1109-1139` over `Common/CktElement.pas:1041-1071`, capi
// `CAPI/CAPI_Alt.pas:1108-1141`), so it routes through the same cache-aware
// `ComputeIterminal` as `Powers`/`Losses` and carries the same one-Newton-step
// staleness upstream — and the terminal sum does NOT cancel it: measured on the
// pinned dss_capi 0.14.5 oracle (`tmp/g13c/probe_newton.py`, 2026-09-05, the
// reported `TotalPowers` against a terminal sum of `V·conj(I)` rebuilt from a
// fresh `Currents` + `VoltagesMagAng`, whose own reconstruction noise is
// ≤ 2.1e-7 kVA on the Load rows) it is **0.6831564014868734 kVA** on
// `modes/newton/newton.dss` (`Line.l1`, and 0.68315640148 kVA on
// `Vsource.source`) and **2.1850404626011652 kVA** on `newton_feeder.dss`
// (`Vsource.source` / `Transformer.sub`) — larger than the per-conductor
// `Powers` staleness on the same decks (0.528 / 0.755 kVA). Both figures are the
// compile-state reading; in the gate's own state (one `solve` further,
// `tools/oracle/oracle_server.py:612-632`) the same stale read is
// 4.5155082046702575e-4 / 5.213217790400988e-3 kVA off the port on
// `Vsource.source` terminal 0 — still ~20x the comparator's band on BOTH
// channels (G1.3c F5, 2026-09-06; `tests/harness/lane.rs::elem_channels_for`).
// Both lanes
// therefore exclude the field on those two decks
// (`tests/harness/lane.rs::LANE_SKIP_ELEM_POWERS`) and this pin is what the
// exclusion owes.
/// The Newton run's `TotalPowers` are the *normal* algorithm's — the surface no
/// oracle channel reports correctly after `Set algorithm=Newton`.
#[test]
fn newton_total_powers_match_the_normal_algorithm() {
    let mut newton = solve_with("Newton");
    let mut normal = solve_with("Normal");
    let sn = newton.snapshot_elements();
    let so = normal.snapshot_elements();
    assert_eq!(sn.len(), so.len());
    let (mut worst, mut worst_at) = (0.0_f64, String::new());
    let mut compared = 0usize;
    for (a, b) in sn.iter().zip(&so) {
        assert_eq!(a.name, b.name, "snapshot order");
        assert_eq!(
            a.total_powers.len(),
            b.total_powers.len(),
            "{}: TotalPowers length",
            a.name,
        );
        for (x, y) in a.total_powers.iter().zip(&b.total_powers) {
            compared += 1;
            if (x - y).norm() > worst {
                worst = (x - y).norm();
                worst_at.clone_from(&a.name);
            }
        }
    }
    assert!(compared > 0, "no terminal was compared");
    // Measured 3.320e-11 kVA (`Vsource.source`) — the cross-algorithm voltage
    // floor, ten orders below the 6.832e-1 kVA the stale read shows on this
    // very deck.
    assert!(
        worst < 1e-8,
        "both lanes must report the fresh terminal totals under Newton too \
         (measured 3.320e-11 kVA; the upstream stale read is 6.832e-1 kVA at \
         Line.l1 / Vsource.source); got {worst:.3e} kVA at {worst_at}",
    );
    // And the totals really are the terminal sums under Newton as well, so the
    // pin cannot be satisfied by two equally stale reads.
    for e in &sn {
        for (t, chunk) in e
            .powers
            .chunks(e.n_conds.max(1))
            .take(e.n_terms)
            .enumerate()
        {
            let s: num_complex::Complex64 = chunk.iter().sum();
            assert!(
                (e.total_powers[t] - s).norm() <= 1e-9 + 1e-12 * s.norm(),
                "{} terminal {t}: TotalPowers {} kVA vs Σ Powers {s} kVA",
                e.name,
                e.total_powers[t],
            );
        }
    }
}

/// **The replacement Newton-dispatch tripwire** — stronger than the gate signal
/// both lanes give up, and running in *both* lanes.
///
/// On the `newton*` decks Newton and the normal fixed point converge to the same
/// voltages in the same iteration count, so the stale-`Iterminal` divergence in
/// the reported powers was the ONLY channel there that would notice `Set
/// algorithm=Newton` silently falling back to `DoNormalSolution`. Neither lane
/// exposes it since G2.3 (nor did the default lane before that), so the property
/// is asserted at its source instead:
/// only `DoNewtonSolution`'s per-iteration `SumAllCurrents` stamps a **PD**
/// element's `Iterminal` at the live `SolutionCount`, and it does so from the
/// pre-update guess.
///
/// So after a Newton solve `Line.l1`'s cache is *valid and stale* (measured
/// 7.378e-2 A off a recompute at the converged `NodeV`), while after a normal
/// solve it is not marked for this `SolutionCount` at all — every read
/// recomputes, which is why no other algorithm can show the quirk. Both facts
/// are engine state, independent of the lane and of any report.
#[test]
fn newton_dispatch_leaves_a_valid_but_stale_iterminal_cache() {
    let (newton_valid, newton_gap) = iterminal_cache_state(&mut solve_with("Newton"), "l1");
    assert!(
        newton_valid,
        "Set algorithm=Newton did not stamp Line.l1's Iterminal for the live \
         SolutionCount — DoNewtonSolution's SumAllCurrents did not run, i.e. \
         Newton dispatch is broken (or fell back to DoNormalSolution)"
    );
    assert!(
        (newton_gap - 7.378_239e-2).abs() < 1e-6,
        "the stamped current must be the pre-update guess's (measured 7.378e-2 A \
         off the converged recompute); got {newton_gap:.6e} A"
    );

    let (normal_valid, _) = iterminal_cache_state(&mut solve_with("Normal"), "l1");
    assert!(
        !normal_valid,
        "the normal fixed point must leave no valid Iterminal cache on a PD \
         element — if it did, the tripwire above would no longer distinguish \
         the two algorithms"
    );
}

/// `(cache is valid for the live SolutionCount, max |cached − fresh| in A)` for
/// `Line.<name>`. Reading the cache before refreshing it is what makes this a
/// measurement of the solver's leftovers rather than of the reporting path.
fn iterminal_cache_state(dss: &mut Dss, line: &str) -> (bool, f64) {
    let ci = dss.class_by_name["line"];
    let oi = dss.classes[ci].name_to_idx[line];
    let Dss {
        classes, circuit, ..
    } = dss;
    let ckt = circuit.as_ref().expect("solved circuit");
    let sys = crate::solution::solution::sys_ctx(ckt);
    let node_v = ckt.solution.node_v.clone();
    let elem = classes[ci]
        .arena
        .try_ckt_elem_mut(oi)
        .expect("a Line is a circuit element");
    let yorder = elem.cd().yorder;
    let valid = elem.cd().iterminal_solved_for(sys.solution_count);
    let cached: Vec<num_complex::Complex64> = elem.cd().iterminal[..yorder].to_vec();
    elem.refresh_iterminal(&sys, &node_v);
    let gap = cached
        .iter()
        .zip(&elem.cd().iterminal[..yorder])
        .map(|(a, b)| (a - b).norm())
        .fold(0.0_f64, f64::max);
    (valid, gap)
}
