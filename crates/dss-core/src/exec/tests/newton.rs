//! `Set algorithm=Newton` — the expected-value pins of CLAUDE.md upstream bug 5
//! (`GOLDEN_REBASE_PLAN.md` G2.3, repaired at its root by `RETRO_FIXES_PLAN.md`
//! RF-D00-01) and the in-engine dispatch tripwire that carries the gate signal
//! both lanes give up with it.
//!
//! **The bug.** r4133 `DoNewtonSolution` stamps every element's `Iterminal`
//! from the pre-final voltage guess `NodeV_{n-1}` in its last `SumAllCurrents`
//! and marks it solved for the live `SolutionCount`; `NodeV -= dV` follows
//! (`Common/Solution.pas:1199`, `:1203`, `:1221-1226`). So a post-solve
//! cache-aware read (`Get_Powers`/`Get_Losses` via `ComputeIterminal`) returns
//! a one-step-stale current while `Currents` (`GetCurrents`) recomputes fresh —
//! upstream reports `S != V·conj(I)` for the same element in the same read, in
//! every vendored official rev (v9.8/r3723, v10.2/r4088, v11.0/r4133, all
//! fingerprint 0.478 kVA, checked 2026-07-08).
//!
//! **The fix.** `do_newton_solution` drops, on exit, the stamps its last
//! `SumAllCurrents` left on every element whose `Iterminal` is not its own model
//! state (`solution::solution::power_flow::drop_stale_newton_iterminal_stamps`)
//! — the cache state the normal fixed point leaves — so every reader of the
//! engine (the snapshot, `Export`/`Show`, meters, monitors, control sampling)
//! sees after a Newton solve what it sees after a normal one. The report layer
//! is pinned surface by surface at the end of this file: `Export Powers`,
//! `Losses`, `Summary` and `Currents`, `Show Powers` and `Currents` cell by cell
//! against `V·conj(I)` of each element's own `GetCurrents`, and those plus the
//! other current-reading reports byte for byte against the normal algorithm.
//! The census covers the iteration-limit exit and every element class the drop
//! sorts, too.
//!
//! **After a direct solve** (the RF-D00-01 settlement). r4133's `SumAllCurrents`
//! also takes the PC direct-solve shortcut while `LastSolutionWasDirect` is still
//! set, so its first Newton solve after `Solve mode=direct` returns the direct
//! solution. The port's sum never does
//! (`newton_after_a_direct_solve_matches_a_fresh_newton_solve`).
//!
//! **Why the deck is the corpus feeder.** `modes/newton/newton.dss` is the gated
//! case whose Powers/Losses channel both lanes exclude
//! (`crates/dss-test-harness/src/harness/lane.rs::LANE_SKIP_ELEM_POWERS`,
//! unconditional since G2.3 — no oracle channel reports it correctly), so the fix
//! is pinned here on exactly the model that stopped being oracle-compared there.
//! Its content is inlined rather than read from `tests/corpus/` because these are
//! `src` unit tests.

use num_complex::Complex64;

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

/// Build the deck, `Solve` it under `algorithm`, and return the engine together
/// with how far that one `Solve` advanced `SolutionCount`.
fn solve_counting(algorithm: &str) -> (Dss, i32) {
    let mut dss = Dss::new();
    for line in DECK {
        dss.command(line);
        assert!(dss.errors().is_empty(), "`{line}` -> {:?}", dss.errors());
    }
    dss.command(&format!("Set algorithm={algorithm}"));
    let before = dss.circuit().unwrap().solution.solution_count;
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert!(
        dss.circuit().unwrap().is_solved,
        "{algorithm} did not solve"
    );
    let advance = dss.circuit().unwrap().solution.solution_count - before;
    (dss, advance)
}

/// Build and solve the deck under `algorithm`.
fn solve_with(algorithm: &str) -> Dss {
    solve_counting(algorithm).0
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
/// valid `Iterminal` stamp on a line or the source (its loads keep their
/// model-state current, as they do under Newton), so its reported powers carry
/// no Newton staleness by construction. Newton-vs-normal on the same deck is
/// therefore a direct read of any stale current, in physical units:
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
// channels (G1.3c F5, 2026-09-06;
// `crates/dss-test-harness/src/harness/lane.rs::elem_channels_for`). Both lanes
// therefore exclude the field on those two decks
// (`crates/dss-test-harness/src/harness/lane.rs::LANE_SKIP_ELEM_POWERS`) and this pin is what the
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
            let s: Complex64 = chunk.iter().sum();
            assert!(
                (e.total_powers[t] - s).norm() <= 1e-9 + 1e-12 * s.norm(),
                "{} terminal {t}: TotalPowers {} kVA vs Σ Powers {s} kVA",
                e.name,
                e.total_powers[t],
            );
        }
    }
}

/// **The Newton-dispatch tripwire** — the gate signal both lanes give up,
/// asserted at its source and in *both* lanes.
///
/// On the `newton*` decks Newton and the normal fixed point converge to the same
/// voltages in the same iteration count, and since RF-D00-01 they also leave the
/// same `Iterminal` cache, so no reported quantity notices `Set
/// algorithm=Newton` silently falling back to `DoNormalSolution` any more (the
/// signal G2.3's tripwire used — `Line.l1`'s cache left valid on a current
/// 7.378239e-2 A off its fresh recompute — was the bug itself, and is gone with
/// it). What stays Newton-only is the loop's own bookkeeping: r4133
/// `DoNewtonSolution` increments `SolutionCount` at the top of every iteration
/// (`Common/Solution.pas:1199`, "SumAllCurrents Uses ITerminal So must force a
/// recalc") on top of the single `Inc(SolutionCount)` of `DoPFLOWsolution`
/// (`:2441`), while `DoNormalSolution` (`:1036`) adds none.
///
/// So one `Solve` advances `SolutionCount` by `1 + Iteration` under Newton and
/// by exactly `1` under the normal algorithm — measured 3 and 1 on this deck,
/// both algorithms in 2 iterations. Engine state, independent of the lane and
/// of any report.
#[test]
fn newton_dispatch_advances_the_solution_count_once_per_iteration() {
    let (newton, newton_advance) = solve_counting("Newton");
    let (normal, normal_advance) = solve_counting("Normal");
    let newton_iterations = newton.circuit().unwrap().solution.iteration;
    let normal_iterations = normal.circuit().unwrap().solution.iteration;
    assert_eq!(
        (newton_iterations, normal_iterations),
        (2, 2),
        "both algorithms solve this deck in 2 iterations (both gating oracles \
         agree) — the premise of the counts below"
    );
    assert_eq!(
        normal_advance, 1,
        "the normal fixed point must advance SolutionCount once per Solve \
         (DoPFLOWsolution's Inc only)"
    );
    assert_eq!(
        newton_advance,
        1 + newton_iterations,
        "Set algorithm=Newton must advance SolutionCount once per Newton \
         iteration on top of DoPFLOWsolution's Inc (measured 3) — anything else \
         means DoNewtonSolution did not run (or fell back to DoNormalSolution)"
    );
}

// EXPECTED-VALUE-PIN(POWERS_REUSE_STALE_NEWTON_ITERMINAL): the torn-down row at
// its root — after a Newton solve no element's `Iterminal` cache is stale,
// asserted in both lanes, where upstream leaves three of six elements stale.
/// A Newton solve leaves exactly the `Iterminal` cache the normal fixed point
/// leaves, and nothing a cache-aware reader can see is stale (RF-D00-01).
///
/// Per energized element, straight after the solve:
///
/// * the current a cache-aware read returns (`compute_iterminal`, r4133
///   `ComputeIterminal` — what `Get_Power`, `Get_Losses`, Export/Show Powers,
///   meters and monitors read) equals the element's own scratch-buffer
///   `GetCurrents` **bit for bit**, under both algorithms;
/// * whether its stamp is valid for the live `SolutionCount` is the same under
///   both — `false` on the two lines and the source (the stamps the Newton
///   loop's last `SumAllCurrents` formed at the pre-update guess, which the
///   solver drops), `true` on the three loads (their model-state current,
///   `ITerminalUpdated`, which both algorithms leave from their last pass at
///   `NodeV_{n-1}`).
///
/// Measured with the solver's stamp drop disabled (the pre-RF-D00-01 engine —
/// r4133's `DoNewtonSolution` as ported): `Vsource.source`
/// 7.378239345614925e-2 A, `Line.l1` 7.378239345633905e-2 A (the G2.3
/// tripwire's 7.378239e-2 A) and `Line.l2` 1.9101628302694115e-2 A off their
/// own `GetCurrents`, the loads 0 — every Export/Show/meter/monitor read after
/// the solve reported those three.
#[test]
fn newton_leaves_the_iterminal_cache_the_normal_algorithm_leaves() {
    let newton = iterminal_cache_census(&mut solve_with("Newton"));
    let normal = iterminal_cache_census(&mut solve_with("Normal"));
    let names = |c: &[(String, bool, f64)]| c.iter().map(|e| e.0.clone()).collect::<Vec<_>>();
    assert_eq!(names(&newton), names(&normal), "element order");
    assert_eq!(newton.len(), 6, "the source, two lines and three loads");

    // Every element is checked before anything is reported, so a regression
    // names all the stale elements at once, with their gaps.
    let stale = |census: &[(String, bool, f64)]| {
        census
            .iter()
            .filter(|e| e.2 != 0.0)
            .map(|(name, _, gap)| format!("{name}: {gap:e} A"))
            .collect::<Vec<_>>()
    };
    assert!(
        stale(&newton).is_empty(),
        "after a Newton solve the cache-aware read is off the element's own \
         GetCurrents (the upstream stale read: Vsource.source \
         7.378239345614925e-2 A, Line.l1 7.378239345633905e-2 A, Line.l2 \
         1.9101628302694115e-2 A): {:?}",
        stale(&newton)
    );
    assert!(
        stale(&normal).is_empty(),
        "after a normal solve the cache-aware read is off the element's own \
         GetCurrents: {:?}",
        stale(&normal)
    );
    let parity: Vec<String> = newton
        .iter()
        .zip(&normal)
        .filter(|(n, o)| n.1 != o.1)
        .map(|(n, o)| format!("{}: Newton {} vs normal {}", n.0, n.1, o.1))
        .collect();
    assert!(
        parity.is_empty(),
        "Newton must leave the Iterminal stamps the normal algorithm leaves \
         (stamp valid for the live SolutionCount): {parity:?}"
    );

    // Both kinds of stamp are present, so the parity above is not vacuous.
    let valid = |who: &str| {
        newton
            .iter()
            .find(|e| e.0 == who)
            .unwrap_or_else(|| panic!("{who} not in the census {:?}", names(&newton)))
            .1
    };
    for who in ["Vsource.source", "Line.l1", "Line.l2"] {
        assert!(
            !valid(who),
            "{who}: the Newton loop's SumAllCurrents stamp must be dropped on exit"
        );
    }
    for who in ["Load.ld1", "Load.ld2", "Load.ld3"] {
        assert!(
            valid(who),
            "{who}: a load's model-state stamp must survive the Newton exit, as it \
             survives the normal fixed point"
        );
    }
}

/// `(element, stamp valid for the live SolutionCount, max |cache-aware read −
/// fresh GetCurrents| in A)` for every energized circuit element. The stamp is
/// read before anything touches it — a measurement of the solver's leftovers,
/// not of a reporting path — then the cache-aware read (`compute_iterminal`) is
/// set against the element's own `GetCurrents` into a scratch buffer: the two
/// read paths upstream's `Powers` and `Currents` take.
fn iterminal_cache_census(dss: &mut Dss) -> Vec<(String, bool, f64)> {
    let Dss {
        classes, circuit, ..
    } = dss;
    let ckt = circuit.as_ref().expect("solved circuit");
    let sys = crate::solution::solution::sys_ctx(ckt);
    let node_v = ckt.solution.node_v.clone();
    let mut out = Vec::new();
    for &r in &ckt.ckt_elements {
        let class_name = classes[r.class_ord()].props.class_name();
        let name = format!(
            "{}.{}",
            class_name,
            classes[r.class_ord()].arena[r.index()].data().name()
        );
        let elem = classes[r.class_ord()]
            .arena
            .try_ckt_elem_mut(r.index())
            .expect("ckt_elements refs are circuit elements");
        if !elem.cd().enabled || elem.cd().node_ref.is_empty() {
            continue;
        }
        let valid = elem.cd().iterminal_solved_for(sys.solution_count);
        elem.compute_iterminal(&sys, &node_v);
        let yorder = elem.cd().yorder;
        let cached = elem.cd().iterminal[..yorder].to_vec();
        let mut fresh = vec![Complex64::ZERO; yorder];
        elem.get_currents(&sys, &node_v, &mut fresh);
        let gap = cached
            .iter()
            .zip(&fresh)
            .map(|(a, b)| (a - b).norm())
            .fold(0.0_f64, f64::max);
        out.push((name, valid, gap));
    }
    out
}

/// The census entries whose cache-aware read is off the element's own
/// `GetCurrents`, as `name: gap A`.
fn stale_elements(census: &[(String, bool, f64)]) -> Vec<String> {
    census
        .iter()
        .filter(|e| e.2 != 0.0)
        .map(|(name, _, gap)| format!("{name}: {gap:e} A"))
        .collect()
}

/// The census entries whose stamp validity differs between the two algorithms.
fn stamp_parity_breaks(
    newton: &[(String, bool, f64)],
    normal: &[(String, bool, f64)],
) -> Vec<String> {
    let names = |c: &[(String, bool, f64)]| c.iter().map(|e| e.0.clone()).collect::<Vec<_>>();
    assert_eq!(names(newton), names(normal), "element order");
    newton
        .iter()
        .zip(normal)
        .filter(|(n, o)| n.1 != o.1)
        .map(|(n, o)| format!("{}: Newton {} vs normal {}", n.0, n.1, o.1))
        .collect()
}

/// Build `deck`, run `commands`, and return the engine (every command must be
/// accepted without an error).
fn run_deck(deck: &[&str], commands: &[&str]) -> Dss {
    let mut dss = Dss::new();
    for line in deck.iter().chain(commands) {
        dss.command(line);
        assert!(dss.errors().is_empty(), "`{line}` -> {:?}", dss.errors());
    }
    dss
}

/// The first Newton solve after a direct solve lands on the fresh Newton
/// solution, not on the direct one (RF-D00-01 settlement, audit AC-1).
///
/// `Solve mode=direct` (like a fault study, a harmonics solve or
/// `LoadModel=Admittance`) arms `LastSolutionWasDirect`, and r4133 clears it only
/// after the algorithm dispatch (`Common/Solution.pas:2481`, set by `SolveDirect`
/// at `:2782`). While it is set, `TPCElement.GetCurrents`
/// (`PCElements/PCElement.pas:284`) answers `Yprim·V` instead of the load model,
/// so r4133's `SumAllCurrents` sees every load as its constant-Z admittance and
/// Newton converges straight back to the direct solution. Live on this deck (the
/// settlement probe: r4133 through `epri-worker` and capi 0.14.5 through the
/// pinned dss-python, 60 Hz, 2 iterations each), direct→Newton sits **10.0756 V**
/// off a fresh Newton solve — `b2.1` 6978.4028 V against 6971.9402 V,
/// `Vsource.source` I1 80.2959 A against 82.7513 A, i.e. the direct solution
/// itself — while direct→normal lands on the fresh one (8.3e-12 V). The port's
/// Newton sum never takes that shortcut
/// (`solution::solution::power_flow::sum_all_currents`), so the upstream bug is
/// reproduced in neither lane. With that override removed the port read the
/// oracles' 10.0756 V.
#[test]
fn newton_after_a_direct_solve_matches_a_fresh_newton_solve() {
    let fresh = solve_with("Newton");
    let fresh_v = fresh.circuit().unwrap().solution.node_v.clone();

    let mut dss = run_deck(DECK, &["Solve mode=direct"]);
    let direct_v = dss.circuit().unwrap().solution.node_v.clone();
    assert!(
        dss.circuit().unwrap().solution.last_solution_was_direct,
        "premise: the direct solve arms the PC direct-solve shortcut"
    );
    for line in ["Set mode=snapshot", "Set algorithm=Newton", "Solve"] {
        dss.command(line);
        assert!(dss.errors().is_empty(), "`{line}` -> {:?}", dss.errors());
    }
    let ckt = dss.circuit().unwrap();
    assert!(
        ckt.is_solved && ckt.solution.converged_flag,
        "Newton after a direct solve did not converge"
    );
    let gap = |a: &[Complex64], b: &[Complex64]| {
        assert_eq!(a.len(), b.len(), "node count");
        a.iter()
            .zip(b)
            .map(|(x, y)| (x - y).norm())
            .fold(0.0_f64, f64::max)
    };
    let direct_gap = gap(&direct_v, &fresh_v);
    let newton_gap = gap(&ckt.solution.node_v, &fresh_v);
    // Measured: the direct solution sits 1.0075554317985269e1 V off the fresh
    // Newton one, and Newton after it 0 V (same start, same iterates, 2 iterations).
    assert!(
        direct_gap > 10.0,
        "premise: the direct solution sits 10.0756 V off the Newton one (got {direct_gap:e} V)"
    );
    assert!(
        newton_gap < 1e-9,
        "Newton after a direct solve must land on the fresh Newton solution, not on \
         the direct one (r4133 and capi 0.14.5 both read 10.0756 V here, the port \
         measured 0 V): max |dV| = {newton_gap:e} V"
    );
}

/// The iteration-limit exit of the Newton loop drops the stale stamps too
/// (RF-D00-01 settlement, audit AT-2). r4133 `DoNewtonSolution` leaves its loop
/// on convergence **or** on `Iteration >= MaxIterations` (`Common/Solution.pas:1228`),
/// and either way its last `SumAllCurrents` stamped every element at the
/// pre-update guess. With `Set maxiterations=1` both algorithms stop unconverged
/// after one iteration, and the Newton census must still hold what the normal
/// algorithm leaves under the same limit. Measured with the drop limited to the
/// converged exit: `Vsource.source` 2.3816185444273468 A, `Line.l1`
/// 2.381618544427303 A and `Line.l2` 0.6758916427676729 A off their own
/// `GetCurrents`, the stale current every reader after such a solve would print.
#[test]
fn newton_iteration_limit_exit_leaves_the_iterminal_cache_the_normal_algorithm_leaves() {
    let census = |algorithm: &str| {
        let mut dss = run_deck(
            DECK,
            &[&format!("Set algorithm={algorithm}"), "Set maxiterations=1"],
        );
        dss.command("Solve");
        assert!(dss.errors().is_empty(), "{algorithm}: {:?}", dss.errors());
        let sol = &dss.circuit().unwrap().solution;
        assert!(
            !sol.converged_flag && sol.iteration == 1,
            "{algorithm}: premise — one unconverged iteration (converged {}, iteration {})",
            sol.converged_flag,
            sol.iteration
        );
        iterminal_cache_census(&mut dss)
    };
    let newton = census("Newton");
    let normal = census("Normal");
    assert_eq!(newton.len(), 6, "the source, two lines and three loads");
    assert!(
        stale_elements(&newton).is_empty(),
        "after an iteration-limit Newton exit the cache-aware read is off the \
         element's own GetCurrents (refused: Vsource.source 2.3816185444273468 A, \
         Line.l1 2.381618544427303 A, Line.l2 0.6758916427676729 A): {:?}",
        stale_elements(&newton)
    );
    assert!(
        stale_elements(&normal).is_empty(),
        "after an iteration-limit normal exit: {:?}",
        stale_elements(&normal)
    );
    let parity = stamp_parity_breaks(&newton, &normal);
    assert!(
        parity.is_empty(),
        "the iteration-limit exit must leave the normal algorithm's stamps: {parity:?}"
    );
    // Both kinds of stamp are present (measured: the source and lines dropped,
    // the loads kept), so the parity above is not vacuous.
    let kept: Vec<&str> = newton
        .iter()
        .filter(|e| e.1)
        .map(|e| e.0.as_str())
        .collect();
    assert_eq!(
        kept,
        ["Load.ld1", "Load.ld2", "Load.ld3"],
        "the stamps an iteration-limit Newton exit keeps"
    );
}

/// [`DECK`] plus one element of every other class the solver's stamp drop sorts:
/// PD (a transformer, a shunt capacitor and reactor), a current source, and the
/// PC model classes whose `Iterminal` is their own model state (a fourth load,
/// generators in models 1 and 3, a PVSystem and a Storage).
const MULTI_CLASS_DECK: &[&str] = &[
    "New Transformer.t1 phases=3 windings=2 buses=[b2, b3] conns=[wye, wye] kvs=[12.47, 4.16] kvas=[1500, 1500] xhl=6",
    "New Capacitor.c1 bus1=b1 phases=3 kvar=300 kv=12.47",
    "New Reactor.r1 bus1=b2 phases=3 kvar=100 kv=12.47",
    "New Isource.is1 bus1=b3 phases=3 amps=2 angle=0",
    "New Load.ld4 bus1=b3 phases=3 kv=4.16 kw=200 pf=0.9 model=1",
    "New Generator.g1 bus1=b1 phases=3 kv=12.47 kw=150 pf=0.95 model=1",
    "New Generator.g3 bus1=b2 phases=3 kv=12.47 kw=100 model=3 maxkvar=80 minkvar=-80",
    "New PVSystem.pv1 bus1=b3 phases=3 kv=4.16 kva=80 pmpp=80 irradiance=1",
    "New Storage.st1 bus1=b3 phases=3 kv=4.16 kwrated=60 kwhrated=240 %stored=50 state=discharging",
    "Set voltagebases=[12.47, 4.16]",
    "Calcvoltagebases",
];

/// The cache census of [`newton_leaves_the_iterminal_cache_the_normal_algorithm_leaves`]
/// over every element class the stamp drop sorts (RF-D00-01 settlement, audit
/// AT-3). The drop keys on `ITerminalUpdated`: it keeps the model-state stamps
/// of Load, Generator, PVSystem and Storage and drops every other one (PD
/// elements, `Vsource`, `Isource`), so a class that sets the flag differently
/// (a PC model path leaving it false under Newton, or a PD override setting it)
/// would read after a Newton solve what it never reads after a normal one. Here
/// every class is checked at once: no stale read under either algorithm, the
/// same stamp validity under both, and the two kinds of stamp where the
/// mechanism puts them.
#[test]
fn newton_leaves_the_normal_iterminal_cache_on_every_element_class() {
    let census = |algorithm: &str| {
        let mut dss = run_deck(
            &[DECK, MULTI_CLASS_DECK].concat(),
            &[&format!("Set algorithm={algorithm}"), "Solve"],
        );
        let sol = &dss.circuit().unwrap().solution;
        // Measured: 4 iterations under both algorithms.
        assert!(sol.converged_flag, "{algorithm} did not converge");
        iterminal_cache_census(&mut dss)
    };
    let newton = census("Newton");
    let normal = census("Normal");
    assert_eq!(newton.len(), 15, "the pin deck's six elements plus nine");
    assert!(
        stale_elements(&newton).is_empty(),
        "after a Newton solve the cache-aware read is off the element's own \
         GetCurrents: {:?}",
        stale_elements(&newton)
    );
    assert!(
        stale_elements(&normal).is_empty(),
        "after a normal solve: {:?}",
        stale_elements(&normal)
    );
    let parity = stamp_parity_breaks(&newton, &normal);
    assert!(
        parity.is_empty(),
        "Newton must leave the normal algorithm's stamp on every class: {parity:?}"
    );
    // Where the mechanism puts each kind of stamp, class by class (measured
    // under both algorithms): the model-state stamps kept, every other dropped.
    let kept: Vec<&str> = newton
        .iter()
        .filter(|e| e.1)
        .map(|e| e.0.as_str())
        .collect();
    assert_eq!(
        kept,
        [
            "Load.ld1",
            "Load.ld2",
            "Load.ld3",
            "Load.ld4",
            "Generator.g1",
            "Generator.g3",
            "PVSystem.pv1",
            "Storage.st1",
        ],
        "the model-state stamps a Newton solve keeps (Vsource, the lines, the \
         transformer, capacitor, reactor and Isource must have theirs dropped)"
    );
}

/// `Set algorithm=Newton` under AutoAdd: the Newton loop adds the trial
/// device's current (r4133 `Common/Solution.pas:1213` `IF UseAuxCurrents THEN
/// AddInAuxCurrents(NEWTONSOLVE)`, capi 0.14.5 `:957`), so the capacity search
/// scores every candidate bus with its trial generator connected and picks the
/// bus the normal fixed point picks, with the same improvement. Without that
/// call each candidate solves the base case and scores no improvement.
///
/// The search reads its losses through the meters' cache-aware sampling, so it
/// also rides on the solver's stamp drop above: a stale post-Newton current
/// would score every candidate off the normal algorithm's figure.
#[test]
fn newton_autoadd_scores_the_trial_generator_like_the_normal_algorithm() {
    let search = |algorithm: &str| -> (String, f64) {
        // AutoAddLog / AutoAddedGenerators side files go to a scratch dir.
        let scratch = std::env::temp_dir().join(format!(
            "dss_newton_autoadd_{algorithm}_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&scratch).ok();
        let mut dss = Dss::new();
        for line in DECK {
            dss.command(line);
            assert!(dss.errors().is_empty(), "`{line}` -> {:?}", dss.errors());
        }
        dss.command("New EnergyMeter.m1 element=Line.l1 terminal=1");
        dss.command(&format!("set datapath=\"{}\"", scratch.display()));
        dss.command("Set addtype=generator genkw=300 genpf=1.0");
        dss.command("Set autobuslist=(b1, b2)");
        dss.command(&format!("Set algorithm={algorithm}"));
        dss.command("solve mode=autoadd");
        assert!(dss.errors().is_empty(), "{algorithm}: {:?}", dss.errors());
        let last = dss.result().to_string();
        std::fs::remove_dir_all(&scratch).ok();
        let (bus, figure) = last
            .split_once(", ")
            .unwrap_or_else(|| panic!("{algorithm}: GlobalResult not `<bus>, <figure>`: {last:?}"));
        let figure = figure
            .parse::<f64>()
            .unwrap_or_else(|e| panic!("{algorithm}: improvement {figure:?}: {e}"));
        (bus.to_string(), figure)
    };
    let (newton_bus, newton_gain) = search("Newton");
    let (normal_bus, normal_gain) = search("Normal");
    assert!(
        normal_gain > 0.0,
        "the trial generator must improve the losses under the normal algorithm \
         (got {normal_gain:e}) — else the comparison below is vacuous"
    );
    assert_eq!(
        newton_bus, normal_bus,
        "AutoAdd must pick the same bus under Newton ({newton_gain:e}) as under \
         the normal algorithm ({normal_gain:e})"
    );
    // Measured `b2` under both, 1.21581112543944e-2 vs 1.21581112543933e-2 —
    // 9e-14 relative, the cross-algorithm voltage floor seen through the
    // 15-digit `GlobalResult`. Refused: without the aux currents Newton picks
    // `b1` at -1.05519285051277e-6; with them but with the solver's stamp drop
    // disabled (the stale meter samples) it scores `b2` at 1.21128540007173e-2,
    // 3.7e-3 relative off. That refused figure is upstream's: r4133 prints
    // `b2, 0.0121128540006551` under Newton and `b2, 0.0121581112543737` under
    // the normal algorithm (capi 0.14.5 Newton `b2, 0.0121128540006772`, all from
    // the RF-D00-01 settlement's live probe), so this pin is a deliberate divergence
    // from r4133's Newton figure (CLAUDE.md upstream bug 5, never reproduced).
    assert!(
        (newton_gain - normal_gain).abs() <= 1e-10 * normal_gain.abs(),
        "AutoAdd improvement under Newton {newton_gain:e} vs normal {normal_gain:e} \
         (measured 9e-14 relative)"
    );
}

// ---------------------------------------------------------------------------
// The report layer (RF-D00-01 part 2). Every `Export`/`Show` surface below runs
// as the first reader after the solve, under both algorithms, and is checked
// three ways: each printed cell is the value rebuilt from `V·conj(I)`, `I` from
// the element's own scratch-buffer `GetCurrents` (the read r4133's `Export`/
// `Show` writers make, never the `Iterminal` cache), within half a unit of the
// cell's last printed digit; every cache-aware reader behind the reports holds
// `S == V·conj(I)` within 1e-8 kVA; and the Newton report is the normal
// algorithm's, byte for byte.
// ---------------------------------------------------------------------------

/// A fresh scratch datapath per report run, so pins running in parallel never
/// share (or delete) each other's files.
static REPORT_RUN: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Build and solve the deck under `algorithm`, then run `command` as the first
/// reader after the solve, its output routed to a scratch datapath. Returns the
/// engine it ran on and the text of the file it wrote.
fn report_after(algorithm: &str, command: &str) -> (Dss, String) {
    let run = REPORT_RUN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let scratch =
        std::env::temp_dir().join(format!("dss_newton_report_{}_{run}", std::process::id()));
    std::fs::create_dir_all(&scratch)
        .unwrap_or_else(|e| panic!("mkdir {}: {e}", scratch.display()));
    let mut dss = solve_with(algorithm);
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    dss.command(command);
    assert!(
        dss.errors().is_empty(),
        "{algorithm}: `{command}` -> {:?}",
        dss.errors()
    );
    let path = if command.to_ascii_lowercase().starts_with("show") {
        dss.last_show_file().to_string()
    } else {
        dss.last_result_file().to_string()
    };
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{algorithm}: `{command}` wrote no {path}: {e}"));
    std::fs::remove_dir_all(&scratch).ok();
    (dss, text)
}

/// `command` under `Set algorithm=Newton` and under the normal algorithm:
/// `(Newton engine, Newton report, normal report)`.
fn reports(command: &str) -> (Dss, String, String) {
    let (newton, newton_text) = report_after("Newton", command);
    let (_, normal_text) = report_after("Normal", command);
    assert!(
        newton_text.lines().count() > 1,
        "`{command}` wrote no rows: {newton_text:?}"
    );
    (newton, newton_text, normal_text)
}

/// The Newton report is the normal algorithm's, byte for byte, once `mask` has
/// blanked what legitimately differs between two runs.
fn assert_same_report(command: &str, newton: &str, normal: &str, mask: fn(&str) -> String) {
    let (n, o) = (mask(newton), mask(normal));
    if n != o {
        let first = n.lines().zip(o.lines()).find(|(a, b)| a != b);
        panic!(
            "`{command}` under Newton is not the normal algorithm's report; first \
             differing line:\n  Newton: {:?}\n  normal: {:?}",
            first.map(|p| p.0),
            first.map(|p| p.1)
        );
    }
}

/// One energized element's conductor flows, rebuilt at the converged voltages
/// from the element's own scratch-buffer `GetCurrents` — never from the
/// `Iterminal` cache. The deck is not a positive-sequence model, so no `×3`.
struct Flow {
    /// `class.name`, lower case.
    name: String,
    pd: bool,
    source: bool,
    /// Per terminal, per conductor: `(V, I)` in V and A (`V = 0` on ground).
    terms: Vec<Vec<(Complex64, Complex64)>>,
}

impl Flow {
    /// Complex power into terminal `t` (0-based), VA: `Σ V·conj(I)`.
    fn power(&self, t: usize) -> Complex64 {
        self.terms[t].iter().map(|(v, i)| v * i.conj()).sum()
    }

    /// Complex power into the whole element (a PD element's losses), VA.
    fn total(&self) -> Complex64 {
        (0..self.terms.len()).map(|t| self.power(t)).sum()
    }
}

/// The rebuilt flows of every energized circuit element, in circuit order.
fn fresh_flows(dss: &mut Dss) -> Vec<Flow> {
    let Dss {
        classes, circuit, ..
    } = dss;
    let ckt = circuit.as_ref().expect("solved circuit");
    assert!(!ckt.positive_sequence, "the flows carry no ×3");
    let sys = crate::solution::solution::sys_ctx(ckt);
    let node_v = ckt.solution.node_v.clone();
    let mut out = Vec::new();
    for &r in &ckt.ckt_elements {
        let name = format!(
            "{}.{}",
            classes[r.class_ord()].props.class_name(),
            classes[r.class_ord()].arena[r.index()].data().name()
        )
        .to_ascii_lowercase();
        let elem = classes[r.class_ord()]
            .arena
            .try_ckt_elem_mut(r.index())
            .expect("ckt_elements refs are circuit elements");
        if !elem.cd().enabled || elem.cd().node_ref.is_empty() {
            continue;
        }
        let mut fresh = vec![Complex64::ZERO; elem.cd().yorder];
        elem.get_currents(&sys, &node_v, &mut fresh);
        let cd = elem.cd();
        let terms = fresh
            .chunks(cd.nconds)
            .take(cd.nterms)
            .enumerate()
            .map(|(t, currents)| {
                cd.term_nodes(t)
                    .iter()
                    .zip(currents)
                    .map(|(&n, &i)| (if n > 0 { node_v[n] } else { Complex64::ZERO }, i))
                    .collect()
            })
            .collect();
        out.push(Flow {
            name,
            pd: ckt.pd_elements.contains(&r),
            source: ckt.sources.contains(&r),
            terms,
        });
    }
    out
}

/// The flow a report row names (`"Line.L1"`, `LINE.L1`, `Vsource.source`, ...).
fn flow<'a>(flows: &'a [Flow], printed: &str) -> &'a Flow {
    let name = printed.trim().trim_matches('"').to_ascii_lowercase();
    flows
        .iter()
        .find(|f| f.name == name)
        .unwrap_or_else(|| panic!("the report names {printed:?}, not an energized element"))
}

/// Half a unit of the last digit `printed` shows — the report's own rounding.
fn half_unit(printed: &str) -> f64 {
    let (digits, exp) = match printed.split_once(['e', 'E']) {
        Some((m, e)) => (m, e.parse::<i32>().expect("exponent")),
        None => (printed, 0),
    };
    let decimals = digits.split_once('.').map_or(0, |(_, d)| d.len() as i32);
    0.5 * 10f64.powi(exp - decimals)
}

/// A report cell against the value rebuilt from the flows: equal within half a
/// unit of the cell's last printed digit, and nothing more.
fn cell(what: &str, printed: &str, value: f64) {
    let p = printed.trim();
    let x: f64 = p
        .parse()
        .unwrap_or_else(|e| panic!("{what}: cell {p:?}: {e}"));
    let tol = half_unit(p);
    assert!(
        (x - value).abs() <= tol + 1e-9,
        "{what}: the report prints {p}, V·conj(I) rebuilt from the element's own \
         GetCurrents gives {value} (allowed: half a printed unit, {tol:e})"
    );
}

/// [`cell`] for an angle in degrees, compared modulo 360.
fn angle_cell(what: &str, printed: &str, degrees: f64) {
    let p = printed.trim();
    let x: f64 = p
        .parse()
        .unwrap_or_else(|e| panic!("{what}: cell {p:?}: {e}"));
    let tol = half_unit(p);
    let gap = ((x - degrees + 180.0).rem_euclid(360.0) - 180.0).abs();
    assert!(
        gap <= tol + 1e-9,
        "{what}: the report prints {p} deg, the element's own GetCurrents gives \
         {degrees} deg (allowed: half a printed unit, {tol:e})"
    );
}

/// Every cache-aware reader the reports format, read on the engine the report
/// just ran on, against the rebuilt flows: `Power[j]` (r4133
/// `Common/ExportResults.pas:1111`/`:1146`, and the meters, monitors and
/// `P_ByPhase` on the same getter), `Losses` (`GetLosses`, `:1192`) and the
/// circuit totals of `Export Summary`: the power the sources supply over every
/// terminal (`Dss::total_power`) and the losses (`Dss::losses`). Returns the
/// worst gap in kVA.
fn reader_gap_kva(dss: &mut Dss, flows: &[Flow]) -> f64 {
    let mut worst = 0.0_f64; // VA
    {
        let Dss {
            classes, circuit, ..
        } = &mut *dss;
        let ckt = circuit.as_ref().expect("solved circuit");
        let sys = crate::solution::solution::sys_ctx(ckt);
        let node_v = ckt.solution.node_v.clone();
        let mut next = flows.iter();
        for &r in &ckt.ckt_elements {
            let elem = classes[r.class_ord()]
                .arena
                .try_ckt_elem_mut(r.index())
                .expect("ckt_elements refs are circuit elements");
            if !elem.cd().enabled || elem.cd().node_ref.is_empty() {
                continue;
            }
            let f = next.next().expect("one flow per energized element");
            for t in 0..f.terms.len() {
                let s = elem.terminal_power(&sys, &node_v, t + 1);
                worst = worst.max((s - f.power(t)).norm());
            }
            worst = worst.max((elem.losses(&sys, &node_v) - f.total()).norm());
        }
        assert!(next.next().is_none(), "one flow per energized element");
    }
    let supply: Complex64 = flows.iter().filter(|f| f.source).map(Flow::total).sum();
    let (p_kw, q_kvar) = dss.total_power();
    worst = worst.max((Complex64::new(p_kw, q_kvar) * 1e3 - supply).norm());
    let losses: Complex64 = flows.iter().filter(|f| f.pd).map(Flow::total).sum();
    let (loss_w, loss_var) = dss.losses();
    worst = worst.max((Complex64::new(loss_w, loss_var) - losses).norm());
    worst * 1e-3
}

/// The reader leg every report pin carries: after a Newton solve and the report
/// the readers hold `S == V·conj(I)` — measured 5.820766091346741e-14 kVA on
/// every surface (summation-order rounding), against 5.283e-1 kVA per conductor
/// for the stale read (`newton_powers_match_the_normal_algorithm`).
fn assert_readers_fresh(dss: &mut Dss, flows: &[Flow], command: &str) {
    let gap = reader_gap_kva(dss, flows);
    assert!(
        gap < 1e-8,
        "after `{command}` under Newton a cache-aware reader is {gap:e} kVA off \
         V·conj(I) of the element's own GetCurrents (measured 5.82e-14)"
    );
}

/// The data rows of a CSV export (header and blank lines dropped), split and
/// trimmed.
fn csv_rows(text: &str) -> Vec<Vec<&str>> {
    text.lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.split(',').map(str::trim).collect())
        .collect()
}

// EXPECTED-VALUE-PIN(POWERS_REUSE_STALE_NEWTON_ITERMINAL): the torn-down row on
// the `Export Powers` surface, asserted in both lanes.
/// `Export Powers` after `Set algorithm=Newton` prints `S = V·conj(I)` on every
/// row and the normal algorithm's report. r4133 reads each row through the
/// cache-aware `Power[j]` (`Common/ExportResults.pas:1111`, `:1146`), so
/// upstream prints the one-Newton-step-stale current: r4133 prints
/// `"Line.L1", 1, 1338.9, 548.5` and `"Line.L1", 2, -1330.4, -523.1` under
/// Newton and `1339.6, 548.8` / `-1331.0, -523.4` under the normal algorithm
/// (kW and kvar, live `epri-worker` probe at the RF-D00-01 settlement, 60 Hz).
/// With the solver's stamp drop disabled the port printed r4133's Newton rows.
/// The rebuilt values asserted here are its normal-algorithm rows.
#[test]
fn newton_export_powers_match_the_normal_algorithm() {
    let command = "Export Powers";
    let (mut dss, newton, normal) = reports(command);
    let flows = fresh_flows(&mut dss);
    let mut rows = 0;
    for c in csv_rows(&newton) {
        let f = flow(&flows, c[0]);
        let t: usize = c[1].parse().expect("terminal");
        let s = f.power(t - 1) * 1e-3;
        cell(&format!("{} terminal {t} P(kW)", f.name), c[2], s.re);
        cell(&format!("{} terminal {t} Q(kvar)", f.name), c[3], s.im);
        rows += 1;
    }
    let terminals: usize = flows
        .iter()
        .filter(|f| !f.source)
        .map(|f| f.terms.len())
        .sum();
    assert_eq!(rows, terminals, "one row per PD/PC terminal");
    assert_readers_fresh(&mut dss, &flows, command);
    assert_same_report(command, &newton, &normal, str::to_owned);
}

// EXPECTED-VALUE-PIN(POWERS_REUSE_STALE_NEWTON_ITERMINAL): the torn-down row on
// the `Export Losses` surface, asserted in both lanes.
/// `Export Losses` after `Set algorithm=Newton`: each PD element's total the sum
/// of its terminal `V·conj(I)`, and the normal algorithm's report. r4133 reads it
/// through `GetLosses` (`Common/ExportResults.pas:1192`) over the cache-aware
/// `Losses` (`Common/CktElement.pas:524-533`): r4133 prints `Line.L1, 8580.566,
/// 25398.79` under Newton and `8586.077, 25413.53` under the normal algorithm
/// (W and var, the settlement's live probe). With the solver's stamp drop
/// disabled the port printed r4133's Newton row. The rebuilt value asserted here
/// is its normal-algorithm row.
#[test]
fn newton_export_losses_match_the_normal_algorithm() {
    let command = "Export Losses";
    let (mut dss, newton, normal) = reports(command);
    let flows = fresh_flows(&mut dss);
    let mut rows = 0;
    for c in csv_rows(&newton) {
        let f = flow(&flows, c[0]);
        let s = f.total();
        cell(&format!("{} Total(W)", f.name), c[1], s.re);
        cell(&format!("{} Total(var)", f.name), c[2], s.im);
        rows += 1;
    }
    let pd = flows.iter().filter(|f| f.pd).count();
    assert_eq!(rows, pd, "one row per PD element");
    assert_readers_fresh(&mut dss, &flows, command);
    assert_same_report(command, &newton, &normal, str::to_owned);
}

// EXPECTED-VALUE-PIN(POWERS_REUSE_STALE_NEWTON_ITERMINAL): the torn-down row on
// the `Export Summary` surface, asserted in both lanes.
/// `Export Summary` after `Set algorithm=Newton`: its supply the sources'
/// `V·conj(I)` over every terminal, its losses the PD elements', and the normal
/// algorithm's row bar the wall-clock `DateTime`. r4133 reads both through the
/// cache-aware getters (`GetTotalPowerFromSources` / `Losses`,
/// `Common/ExportResults.pas:3309`, `:3312`): r4133 prints `TotalMW 1.33894,
/// TotalMvar 0.548524, MWLosses 0.0101527, MvarLosses 0.029433` under Newton and
/// `1.33956, 0.5488, 0.0101586, 0.0294493` under the normal algorithm (the
/// settlement's live probe). With the solver's stamp drop disabled the port
/// printed r4133's Newton row. The rebuilt values asserted here are its
/// normal-algorithm row.
#[test]
fn newton_export_summary_matches_the_normal_algorithm() {
    let command = "Export Summary";
    let (mut dss, newton, normal) = reports(command);
    let flows = fresh_flows(&mut dss);
    let mut lines = newton.lines();
    let header: Vec<&str> = lines
        .next()
        .expect("header")
        .split(',')
        .map(str::trim)
        .collect();
    let row: Vec<&str> = lines
        .next()
        .expect("row")
        .split(',')
        .map(str::trim)
        .collect();
    assert_eq!(header.len(), row.len(), "{header:?} / {row:?}");
    let col = |name: &str| {
        let k = header
            .iter()
            .position(|h| *h == name)
            .unwrap_or_else(|| panic!("no {name} column in {header:?}"));
        row[k]
    };
    let supply: Complex64 = flows.iter().filter(|f| f.source).map(Flow::total).sum();
    let losses: Complex64 = flows.iter().filter(|f| f.pd).map(Flow::total).sum();
    cell("TotalMW", col("TotalMW"), -supply.re * 1e-6);
    cell("TotalMvar", col("TotalMvar"), -supply.im * 1e-6);
    cell("MWLosses", col("MWLosses"), losses.re * 1e-6);
    cell(
        "pctLosses",
        col("pctLosses"),
        losses.re / -supply.re * 100.0,
    );
    cell("MvarLosses", col("MvarLosses"), losses.im * 1e-6);
    assert_readers_fresh(&mut dss, &flows, command);
    // `DateTime` is the wall clock (`DateTimeToStr(Now)`), the one field two
    // runs may differ in.
    let mask = |text: &str| -> String {
        text.lines()
            .map(|l| match l.strip_prefix('"') {
                Some(rest) => rest.split_once(',').map_or(l, |(_, tail)| tail),
                None => l,
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_same_report(command, &newton, &normal, mask);
}

// EXPECTED-VALUE-PIN(POWERS_REUSE_STALE_NEWTON_ITERMINAL): the torn-down row on
// the `Show Powers` surface, asserted in both lanes.
/// `Show Powers` after `Set algorithm=Newton`, both forms: the per-conductor
/// `Elements` form prints every conductor, terminal total and the
/// circuit-losses footer at `V·conj(I)`, and both it and the
/// symmetrical-component form print the normal algorithm's report. r4133 reads
/// every element through a scratch `GetCurrents` (`Common/ShowResults.pas:785`,
/// `:844`, `:912`; `Elements` form `:988`, `:1023`, `:1074`), so there upstream
/// prints the fresh value and the port printed neither oracle's before
/// RF-D00-01: with the solver's stamp drop disabled, `SOURCEBUS 1 -542.9 +j
/// -236.2` against the rebuilt `-543.3 +j -236.4`, and `"Vsource.source" 1
/// -1339.0 -548.8` against `-1339.6 -549.1` in the sequence form.
#[test]
fn newton_show_powers_match_the_normal_algorithm() {
    let (_, newton, normal) = reports("Show Powers");
    assert_same_report("Show Powers", &newton, &normal, str::to_owned);

    let command = "Show Powers kVA Elements";
    let (mut dss, newton, normal) = reports(command);
    let flows = fresh_flows(&mut dss);
    let (mut current, mut t, mut k) = (None::<&Flow>, 0, 0);
    let (mut conductors, mut totals, mut footer) = (0, 0, false);
    for line in newton.lines() {
        let tok: Vec<&str> = line.split_whitespace().collect();
        // `kW +j kvar` around the `+j`, wherever the label columns end.
        let jay = tok.iter().position(|w| *w == "+j").filter(|&j| j > 0);
        if let Some(name) = line.trim().strip_prefix("ELEMENT = ") {
            (current, t, k) = (Some(flow(&flows, name)), 0, 0);
        } else if line.contains("TERMINAL TOTAL") {
            let j = jay.expect("`kW +j kvar` in a terminal total row");
            // The label is dot-padded up to the kW field in the parity lane
            // (`PadDots`: `TERMINAL TOTAL . -1339.6 +j ...`).
            let kw = tok[j - 1].trim_start_matches(|c: char| !(c.is_ascii_digit() || c == '-'));
            let f = current.expect("a terminal total inside an ELEMENT block");
            let s = f.power(t) * 1e-3;
            let at = format!("{} terminal {} total", f.name, t + 1);
            cell(&format!("{at} kW"), kw, s.re);
            cell(&format!("{at} kvar"), tok[j + 1], s.im);
            (t, k) = (t + 1, 0);
            totals += 1;
        } else if line.trim_start().starts_with("Total Circuit Losses") {
            let j = jay.expect("`kW +j kvar` in the losses footer");
            let losses: Complex64 = flows.iter().filter(|f| f.pd).map(Flow::total).sum();
            cell("Total Circuit Losses kW", tok[j - 1], losses.re * 1e-3);
            cell("Total Circuit Losses kvar", tok[j + 1], losses.im * 1e-3);
            footer = true;
        } else if let Some(j) = jay.filter(|&j| tok[j - 1].parse::<f64>().is_ok()) {
            let f = current.expect("a conductor row inside an ELEMENT block");
            let (v, i) = *f.terms[t].get(k).unwrap_or_else(|| {
                panic!(
                    "{}: more conductor rows than conductors at {line:?}",
                    f.name
                )
            });
            let s = v * i.conj() * 1e-3;
            let at = format!("{} terminal {} conductor {}", f.name, t + 1, k + 1);
            cell(&format!("{at} kW"), tok[j - 1], s.re);
            cell(&format!("{at} kvar"), tok[j + 1], s.im);
            k += 1;
            conductors += 1;
        }
    }
    let terminals = || flows.iter().flat_map(|f| &f.terms);
    assert_eq!(
        conductors,
        terminals().map(Vec::len).sum::<usize>(),
        "conductor rows"
    );
    assert_eq!(totals, terminals().count(), "terminal totals");
    assert!(footer, "no Total Circuit Losses footer");
    assert_readers_fresh(&mut dss, &flows, command);
    assert_same_report(command, &newton, &normal, str::to_owned);
}

// EXPECTED-VALUE-PIN(POWERS_REUSE_STALE_NEWTON_ITERMINAL): the torn-down row on
// the `Export Currents` surface, asserted in both lanes.
/// `Export Currents` after `Set algorithm=Newton` prints every conductor and
/// residual as the element's own `GetCurrents`, and the normal algorithm's
/// report. r4133 calls that scratch `GetCurrents` on every element
/// (`Common/ExportResults.pas:563`, `:574`, `:584`, `:594`; capi 0.14.5
/// `src/Common/ExportResults.pas:660`), so there upstream prints the fresh
/// current and the port printed neither oracle's before RF-D00-01: with the
/// solver's stamp drop disabled `Vsource.SOURCE` conductor 1 read `82.6776` A
/// against the fresh `82.7513` A.
#[test]
fn newton_export_currents_match_the_normal_algorithm() {
    let command = "Export Currents";
    let (mut dss, newton, normal) = reports(command);
    let flows = fresh_flows(&mut dss);
    let header: Vec<&str> = newton
        .lines()
        .next()
        .expect("header")
        .split(',')
        .map(str::trim)
        .collect();
    let max_cond = header.iter().filter(|h| h.starts_with("I1_")).count();
    let max_term = header.iter().filter(|h| h.starts_with("Iresid")).count();
    let cdang = crate::support::complexutil::cdang;
    let mut rows = 0;
    for c in csv_rows(&newton) {
        assert_eq!(c.len(), 1 + max_term * (max_cond + 1) * 2, "{c:?}");
        let f = flow(&flows, c[0]);
        for t in 0..max_term {
            let at = 1 + t * (max_cond + 1) * 2;
            let mut resid = Complex64::ZERO;
            for k in 0..max_cond {
                let i = f
                    .terms
                    .get(t)
                    .and_then(|term| term.get(k))
                    .map_or(Complex64::ZERO, |p| p.1);
                resid += i;
                let what = format!("{} I{}_{}", f.name, t + 1, k + 1);
                cell(&what, c[at + 2 * k], i.norm());
                angle_cell(&what, c[at + 2 * k + 1], cdang(i));
            }
            let what = format!("{} Iresid{}", f.name, t + 1);
            cell(&what, c[at + 2 * max_cond], resid.norm());
            angle_cell(&what, c[at + 2 * max_cond + 1], cdang(resid));
        }
        rows += 1;
    }
    assert_eq!(rows, flows.len(), "one row per energized element");
    assert_readers_fresh(&mut dss, &flows, command);
    assert_same_report(command, &newton, &normal, str::to_owned);
}

// EXPECTED-VALUE-PIN(POWERS_REUSE_STALE_NEWTON_ITERMINAL): the torn-down row on
// the `Show Currents` surface, asserted in both lanes.
/// `Show Currents` after `Set algorithm=Newton`, both forms: the per-conductor
/// `Elements` form prints every conductor as the element's own `GetCurrents`,
/// and both it and the symmetrical-component form print the normal algorithm's
/// report. r4133 reads that scratch `GetCurrents` on every element
/// (`Common/ShowResults.pas:572`, `:594`, `:616`, `:639`;
/// `WriteTerminalCurrents` `:492`), so the port printed neither oracle's before
/// RF-D00-01: with the solver's stamp drop disabled `SOURCEBUS 1 82.678` A
/// against the fresh `82.751` A, and `"VSOURCE.SOURCE" 1 67.29` against
/// `67.322` A (`I1`) in the sequence form.
#[test]
fn newton_show_currents_match_the_normal_algorithm() {
    let (_, newton, normal) = reports("Show Currents");
    assert_same_report("Show Currents", &newton, &normal, str::to_owned);

    let command = "Show Currents Elements";
    let (mut dss, newton, normal) = reports(command);
    let flows = fresh_flows(&mut dss);
    let cdang = crate::support::complexutil::cdang;
    let (mut current, mut t, mut k, mut conductors) = (None::<&Flow>, 0, 0, 0);
    for line in newton.lines() {
        let tok: Vec<&str> = line.split_whitespace().collect();
        if let Some(name) = line.trim().strip_prefix("ELEMENT = ") {
            (current, t, k) = (Some(flow(&flows, name)), 0, 0);
        } else if line.trim() == "------------" {
            (t, k) = (t + 1, 0);
        } else if tok.len() == 9 && tok[3] == "/_" && tok[5] == "=" {
            let f = current.expect("a conductor row inside an ELEMENT block");
            let i = f.terms[t][k].1;
            let at = format!("{} terminal {} conductor {}", f.name, t + 1, k + 1);
            cell(&format!("{at} |I|"), tok[2], i.norm());
            angle_cell(&format!("{at} angle"), tok[4], cdang(i));
            cell(&format!("{at} re"), tok[6], i.re);
            cell(&format!("{at} im"), tok[8], i.im);
            k += 1;
            conductors += 1;
        }
    }
    let all: usize = flows.iter().flat_map(|f| &f.terms).map(Vec::len).sum();
    assert_eq!(conductors, all, "conductor rows");
    assert_readers_fresh(&mut dss, &flows, command);
    assert_same_report(command, &newton, &normal, str::to_owned);
}

/// Every other report that reads element currents prints, after `Set
/// algorithm=Newton`, exactly the normal algorithm's report. Each surface below
/// printed something else with the solver's stamp drop disabled (RF-D00-01
/// part 2 probe); `Export Powers MVA`, `Show Powers MVA`, the two overload
/// reports and the voltage reports are left out because they cannot see the
/// stale current at their printed resolution, or at all. `Export ElemPowers` /
/// `Export ElemCurrents` read the cache-aware `ComputeIterminal` in r4133 as
/// well (`Common/ExportResults.pas:751`, `:977`), so there upstream prints the
/// stale current: r4133's `Export ElemCurrents` reads `Vsource.source`
/// conductor 1 at 82.6776 A under Newton and 82.7513 A under the normal
/// algorithm (the settlement's live `epri-worker` probe). This pin asserts the
/// latter.
#[test]
fn newton_reports_read_like_the_normal_algorithm() {
    for command in [
        "Export SeqCurrents",
        "Export SeqPowers",
        "Export P_ByPhase",
        "Export Capacity",
        "Export ElemPowers",
        "Export ElemCurrents",
        "Show Busflow b1 kVA",
        "Show Busflow b1 kVA Elements",
        "Show Losses",
    ] {
        let (_, newton, normal) = reports(command);
        assert_same_report(command, &newton, &normal, str::to_owned);
    }
}
