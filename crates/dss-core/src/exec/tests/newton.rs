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
//! sees after a Newton solve what it sees after a normal one.
//!
//! **Why the deck is the corpus feeder.** `modes/newton/newton.dss` is the gated
//! case whose Powers/Losses channel both lanes exclude
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
        let mut fresh = vec![num_complex::Complex64::ZERO; yorder];
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
    // 3.7e-3 relative off.
    assert!(
        (newton_gain - normal_gain).abs() <= 1e-10 * normal_gain.abs(),
        "AutoAdd improvement under Newton {newton_gain:e} vs normal {normal_gain:e} \
         (measured 9e-14 relative)"
    );
}
