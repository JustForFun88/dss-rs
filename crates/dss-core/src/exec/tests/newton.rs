//! `Set algorithm=Newton` — the Stage F [`POWERS_REUSE_STALE_NEWTON_ITERMINAL`]
//! row (CLAUDE.md known upstream bug 5) and the in-engine dispatch tripwire that
//! replaces the gate signal the default lane gives up.
//!
//! **The quirk.** `DoNewtonSolution`'s final `SumAllCurrents` stamps every
//! element's `Iterminal` from the pre-final voltage guess `NodeV_{n-1}` and
//! marks it solved for the current `SolutionCount`; `NodeV -= dV` follows. So a
//! post-solve `Get_Powers`/`Get_Losses` (cache-aware `ComputeIterminal`) reads a
//! one-step-stale current while `Currents` (`GetCurrents`) recomputes fresh —
//! upstream reports `S != V·conj(I)` for the same element in the same read. The
//! parity lane reproduces it; the default lane recomputes all three reads at
//! `NodeV_n`.
//!
//! **Why the deck is the corpus feeder.** `modes/newton/newton.dss` is the gated
//! case whose Powers/Losses channel the default lane excludes
//! (`tests/harness/lane.rs::LANE_SKIP_ELEM_POWERS`), so the divergence is pinned
//! here on exactly the model that stops being oracle-compared there. Its content
//! is inlined rather than read from `tests/corpus/` because these are `src` unit
//! tests.
//!
//! [`POWERS_REUSE_STALE_NEWTON_ITERMINAL`]: crate::compat::POWERS_REUSE_STALE_NEWTON_ITERMINAL

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

/// The lane the engine was compiled in, read from the engine itself.
fn parity() -> bool {
    crate::compat::ORACLE_PARITY
}

/// The Stage F `POWERS_REUSE_STALE_NEWTON_ITERMINAL` row, pinned by expected
/// value against the algorithm that has no cache to be stale.
///
/// Newton and the normal fixed point land on the same voltages (measured: max
/// |ΔV| = 7.6e-12 V, both in 2 iterations), and the *normal* solve leaves no
/// valid `Iterminal` cache, so its reported powers are fresh by construction.
/// Newton-vs-normal on the same deck is therefore a direct read of the stale
/// current, in physical units:
///
/// | quantity | parity lane (quirk reproduced) | default lane (clean fix) |
/// |---|---|---|
/// | worst per-conductor \|ΔS\| | **5.283e-1 kVA** (`Vsource.source`) | 3.256e-11 kVA |
/// | worst \|Δlosses\| | **6.248e2 W** | 3.329e-8 W |
/// | worst \|ΔI\| | 4.547e-12 A | 4.547e-12 A |
///
/// Ten orders of magnitude apart, so the bounds below sit far from both sides.
/// The currents row is the control: `Currents` always recomputed fresh, so it is
/// lane-independent — which is exactly why upstream's `S != V·conj(I)` after a
/// Newton solve is a bug and not a convention.
#[test]
fn newton_powers_are_the_lane_kernel() {
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

    if parity() {
        assert!(
            worst_s > 1e-1 && worst_loss > 1e2,
            "the parity lane must REPRODUCE the upstream post-Newton staleness \
             (measured 5.283e-1 kVA at Vsource.source / 6.248e2 W); got \
             {worst_s:.3e} kVA at {worst_s_at} / {worst_loss:.3e} W"
        );
    } else {
        assert!(
            worst_s < 1e-8 && worst_loss < 1e-5,
            "the default lane must report S = V·conj(I) under Newton too \
             (measured 3.256e-11 kVA / 3.329e-8 W); got {worst_s:.3e} kVA at \
             {worst_s_at} / {worst_loss:.3e} W"
        );
    }
}

/// **The replacement Newton-dispatch tripwire** — stronger than the gate signal
/// the default lane gives up, and running in *both* lanes.
///
/// On the `newton*` decks Newton and the normal fixed point converge to the same
/// voltages in the same iteration count, so the stale-`Iterminal` split in the
/// reported powers was the ONLY channel there that would notice `Set
/// algorithm=Newton` silently falling back to `DoNormalSolution`. The default
/// lane no longer exposes it, so the property is asserted at its source instead:
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
