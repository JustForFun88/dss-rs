//! Circuit aggregates — `Circuit.Losses` / `LineLosses` / `SubstationLosses` /
//! `TotalPower` / `AllElementLosses` (`GOLDEN_REBASE_PLAN.md` G1.9).
//!
//! The live corpus gate compares these five numbers against both oracle
//! channels on every case, but a *number* only ever proves the sum; what an
//! aggregate really encodes is its **membership, its units and its walk order**.
//! Those three are pinned here, in-engine and without an oracle, so that a wrong
//! summand set survives neither the gate (arm P1 reconstructs each oracle
//! aggregate over [`crate::exec::view::AggregateTerms`]) nor this module.
//!
//! Upstream spec, re-read at r4133 (`.inputs/electricdss-code-r4133-trunk`) and
//! at the pinned dss_capi 0.14.5 (`.inputs/dss_capi`):
//! `DDLL/DCircuit.pas:294-368` + `:458-479` (`CircuitV` modes 0/1/2/3/8),
//! `Common/Circuit.pas:2428-2445` (`Get_Losses`), `Common/CktElement.pas:666-767`
//! (`Get_Power` / `Get_Losses`), and `CAPI_Circuit.pas:145-186/289-338/445-468`.

use crate::exec::Dss;
use crate::exec::view::ElementSnapshot;

/// Build a circuit from `deck`, asserting every command is accepted.
fn build(deck: &[&str]) -> Dss {
    let mut dss = Dss::new();
    for line in deck {
        dss.command(line);
        assert!(dss.errors().is_empty(), "`{line}` -> {:?}", dss.errors());
    }
    dss
}

/// Build and `Solve`, asserting the solve converged.
fn solve(deck: &[&str]) -> Dss {
    let mut dss = build(deck);
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert!(dss.circuit().unwrap().is_solved, "deck did not solve");
    dss
}

/// A 12.47 kV feeder: three lines (two of them parallel), one load. The common
/// base of the losses / line-losses pins.
const FEEDER: &[&str] = &[
    "Clear",
    "New Circuit.g19 basekv=12.47 phases=3 bus1=sourcebus mvasc3=200 mvasc1=210",
    "New Line.l1 bus1=sourcebus bus2=b1 phases=3 r1=0.30 x1=0.90 r0=0.9 x0=2.7 c1=3.0 c0=1.5 length=2 units=km",
    "New Line.l2 bus1=sourcebus bus2=b1 phases=3 r1=0.45 x1=1.10 r0=1.1 x0=3.0 c1=3.0 c0=1.5 length=2 units=km",
    "New Line.l3 bus1=b1 bus2=b2 phases=3 r1=0.35 x1=0.95 r0=1.0 x0=2.8 c1=3.0 c0=1.5 length=1.5 units=km",
    "New Load.ld1 bus1=b2 phases=3 kv=12.47 kw=900 pf=0.92 model=1",
    "Set voltagebases=[12.47]",
    "Calcvoltagebases",
];

/// The `(index, loss_w)` of the element named `full_name`, panicking when the
/// deck does not carry it (a pin must never pass vacuously).
fn find(snaps: &[ElementSnapshot], full_name: &str) -> (usize, (f64, f64)) {
    let i = snaps
        .iter()
        .position(|s| s.name.eq_ignore_ascii_case(full_name))
        .unwrap_or_else(|| panic!("{full_name} is not in the circuit"));
    (i, snaps[i].loss_w)
}

/// `|a - b| <= rel * max(|a|, |b|) + abs`, the assertion the pins share.
fn close(a: f64, b: f64, rel: f64, abs: f64, what: &str) {
    let bound = rel * a.abs().max(b.abs()) + abs;
    assert!(
        (a - b).abs() <= bound,
        "{what}: {a:.17e} vs {b:.17e} (|delta| = {:.3e} > {bound:.3e})",
        (a - b).abs()
    );
}

// Expected-value pin, circuit losses are watts: `Circuit.Losses` is the ONE
// aggregate upstream does not rescale — `CAPI_Circuit.pas:171-186` and
// `DDLL/DCircuit.pas:294-303` both hand back `Circuit.Losses` untouched, while
// LineLosses / SubstationLosses / TotalPower / AllElementLosses all carry a
// `x 0.001`. A stray rescale here is a factor-1000 error that no per-element
// floor upstream of the sum can see.
/// `Circuit.Losses` is W/var and sums exactly the enabled, non-shunt PD
/// elements' own losses (`Common/Circuit.pas:2428-2445`).
#[test]
fn circuit_losses_are_watts_not_kilowatts() {
    let mut dss = solve(FEEDER);
    let terms = dss.aggregate_terms();
    let snaps = dss.snapshot_elements();
    let ael = dss.all_element_losses();
    let (p_w, q_var) = dss.losses();

    // The membership: the three lines — no source (not a PD element), no load.
    assert_eq!(terms.losses, vec!["line.l1", "line.l2", "line.l3"]);

    // Sum over the terms, taken from the per-element kW capture and pushed back
    // to W: the units identity, in the direction the comparator uses.
    let (mut p_kw, mut q_kvar) = (0.0, 0.0);
    for name in &terms.losses {
        let (i, _) = find(&snaps, name);
        p_kw += ael[i].0;
        q_kvar += ael[i].1;
    }
    // Sanity: a real feeder loses real power, so the pin cannot pass on zeros.
    assert!(
        p_kw > 1.0,
        "the feeder must dissipate kilowatts, got {p_kw}"
    );
    close(
        p_w,
        1000.0 * p_kw,
        1e-12,
        1e-9,
        "Losses.re (W) vs 1000 x sum kW",
    );
    close(
        q_var,
        1000.0 * q_kvar,
        1e-12,
        1e-9,
        "Losses.im (var) vs 1000 x sum kvar",
    );
}

// Expected-value pin, SubstationLosses excludes AutoTrans: `AutoTrans` objects
// are registered on the `AutoTransformers` pointer list, NOT on `Transformers`
// (`Common/Circuit.pas:2272-2273`), and `Circuit.SubstationLosses` walks
// `Transformers` only (`DDLL/DCircuit.pas:335-341`, `CAPI_Circuit.pas:300-304`)
// — so an `AutoTrans ... sub=yes` contributes nothing, however substation-like
// it is. No corpus deck carries `sub=yes` on an AutoTrans, so this deck is the
// exclusion's only witness.
/// A `sub=yes` AutoTrans is invisible to `SubstationLosses`; the `sub=yes`
/// Transformer is the whole sum.
#[test]
fn substation_losses_exclude_autotrans() {
    let mut dss = solve(&[
        "Clear",
        "New Circuit.g19sub basekv=115 phases=3 bus1=sourcebus mvasc3=20000 mvasc1=18000",
        // The substation transformer: 115 -> 12.47 kV, sub=yes.
        "New Transformer.sub1 phases=3 windings=2 xhl=8 sub=yes %loadloss=0.7",
        "~ wdg=1 bus=sourcebus conn=delta kv=115 kva=20000",
        "~ wdg=2 bus=mv conn=wye kv=12.47 kva=20000",
        // A second transformer that is NOT a substation.
        "New Transformer.t2 phases=3 windings=2 xhl=6 sub=no %loadloss=0.9",
        "~ wdg=1 bus=mv conn=wye kv=12.47 kva=5000",
        "~ wdg=2 bus=lv conn=wye kv=4.16 kva=5000",
        // A sub=yes AUTOTRANS on its own island — same flag, different list.
        "New Vsource.s2 bus1=auto_hv basekv=115 pu=1.0 phases=3 mvasc3=15000 mvasc1=12000",
        "New AutoTrans.a1 phases=3 windings=2 xhx=8.5 sub=yes %loadloss=0.8",
        "~ wdg=1 bus=auto_hv conn=s kv=115 kva=10000",
        "~ wdg=2 bus=auto_lv conn=w kv=69 kva=10000",
        "New Load.ldmv bus1=lv phases=3 kv=4.16 kw=3000 pf=0.95 model=1",
        "New Load.ldauto bus1=auto_lv phases=3 kv=69 kw=6000 pf=0.95 model=1",
        "Set voltagebases=[115, 69, 12.47, 4.16]",
        "Calcvoltagebases",
    ]);

    let terms = dss.aggregate_terms();
    assert_eq!(
        terms.substation_losses,
        vec!["transformer.sub1"],
        "only the sub=yes Transformer is a term"
    );

    let snaps = dss.snapshot_elements();
    let (_, sub1) = find(&snaps, "Transformer.sub1");
    let (_, auto) = find(&snaps, "AutoTrans.a1");
    // Non-vacuity: the excluded AutoTrans is enabled and loaded, so including
    // it would move the answer by kilowatts, not by rounding.
    assert!(
        auto.0 > 1000.0,
        "the AutoTrans must dissipate real power for the exclusion to bite, got {} W",
        auto.0
    );

    let (p_kw, q_kvar) = dss.substation_losses();
    close(p_kw, sub1.0 * 0.001, 1e-12, 1e-12, "SubstationLosses.re");
    close(q_kvar, sub1.1 * 0.001, 1e-12, 1e-12, "SubstationLosses.im");
}

// Expected-value pin, Circuit.Losses skips shunts: `Get_Losses` skips shunt PD
// elements (`Common/Circuit.pas:2438-2440`, `{Ignore Shunt Elements}`). A shunt
// capacitor's "loss" is its whole reactive rating, so a missing `IsShunt` test
// is a kvar-scale error in the imaginary part.
/// A shunt capacitor is in `AllElementLosses` but never in `Circuit.Losses`.
#[test]
fn losses_skip_shunt_elements() {
    let mut deck: Vec<&str> = FEEDER.to_vec();
    deck.insert(
        deck.len() - 2,
        "New Capacitor.cap1 bus1=b2 phases=3 kvar=600 kv=12.47",
    );
    let mut dss = solve(&deck);

    let terms = dss.aggregate_terms();
    assert!(
        !terms.losses.iter().any(|t| t == "capacitor.cap1"),
        "the shunt capacitor must not be a Losses term: {:?}",
        terms.losses
    );

    let snaps = dss.snapshot_elements();
    let (icap, cap) = find(&snaps, "Capacitor.cap1");
    let ael = dss.all_element_losses();
    // It IS in AllElementLosses (that walk has no filter) and it carries the
    // bank's whole reactive output — the magnitude the exclusion is worth.
    close(
        ael[icap].0,
        cap.0 * 0.001,
        1e-12,
        1e-12,
        "AllElementLosses.re",
    );
    close(
        ael[icap].1,
        cap.1 * 0.001,
        1e-12,
        1e-12,
        "AllElementLosses.im",
    );
    assert!(
        cap.1 < -500_000.0,
        "the 600 kvar bank must show its reactive output, got {} var",
        cap.1
    );

    // And `Circuit.Losses` is the PD sum *without* it.
    let (_, q_var) = dss.losses();
    let q_terms: f64 = terms.losses.iter().map(|n| find(&snaps, n).1.1).sum();
    close(
        q_var,
        q_terms,
        1e-12,
        1e-9,
        "Losses.im vs sum of non-shunt terms",
    );
}

// Expected-value pin, LineLosses walks the Lines list: `Circuit.LineLosses`
// walks the raw `Lines` pointer list with no `enabled` filter
// (`DDLL/DCircuit.pas:313-320`, `CAPI_Circuit.pas:156-159`); a disabled line
// stays a term and contributes `CZERO` through `Get_Losses`'s own guard
// (`Common/CktElement.pas:707-712`).
/// `LineLosses` is kW/kvar over every `Lines` entry, disabled ones included as
/// exact zeros.
#[test]
fn line_losses_sum_the_lines_list() {
    let mut deck: Vec<&str> = FEEDER.to_vec();
    deck.push("Edit Line.l2 enabled=no");
    let mut dss = solve(&deck);

    let terms = dss.aggregate_terms();
    assert_eq!(
        terms.line_losses,
        vec!["line.l1", "line.l2", "line.l3"],
        "the disabled line stays a LineLosses term"
    );
    // ... while dropping out of `Circuit.Losses`, which does filter on enabled.
    assert_eq!(terms.losses, vec!["line.l1", "line.l3"]);

    let snaps = dss.snapshot_elements();
    let (_, l2) = find(&snaps, "Line.l2");
    assert_eq!(l2, (0.0, 0.0), "a disabled line's Get_Losses is CZERO");
    let (_, l1) = find(&snaps, "Line.l1");
    let (_, l3) = find(&snaps, "Line.l3");
    assert!(
        l1.0 > 1000.0 && l3.0 > 1000.0,
        "both live lines must dissipate"
    );

    let (p_kw, q_kvar) = dss.line_losses();
    close(p_kw, (l1.0 + l3.0) * 0.001, 1e-12, 1e-12, "LineLosses.re");
    close(q_kvar, (l1.1 + l3.1) * 0.001, 1e-12, 1e-12, "LineLosses.im");
}

// Expected-value pin, TotalPower is terminal one: `Circuit.TotalPower` sums
// `Power[1]` — terminal 1 only — over the `Sources` list and rescales by 0.001
// (`DDLL/DCircuit.pas:356-362`, `CAPI_Circuit.pas:329-333`), and `Get_Power`
// multiplies by 3 under `PositiveSequence` (`Common/CktElement.pas:666-703`).
/// `TotalPower` is terminal 1 of every source, in kW/kvar, x3 in a
/// positive-sequence circuit.
#[test]
fn total_power_is_terminal_one_of_every_source() {
    let mut dss = solve(&[
        "Clear",
        "New Circuit.g19tp basekv=12.47 phases=3 bus1=sourcebus mvasc3=200 mvasc1=210",
        "New Line.l1 bus1=sourcebus bus2=b1 phases=3 r1=0.30 x1=0.90 r0=0.9 x0=2.7 c1=3.0 c0=1.5 length=2 units=km",
        "New Load.ld1 bus1=b1 phases=3 kv=12.47 kw=900 pf=0.92 model=1",
        // A second source on its own island: `Sources` must be walked whole.
        "New Vsource.s2 bus1=sb2 basekv=12.47 pu=1.02 phases=3 mvasc3=150 mvasc1=140",
        "New Line.l2 bus1=sb2 bus2=b2 phases=3 r1=0.30 x1=0.90 r0=0.9 x0=2.7 c1=3.0 c0=1.5 length=1 units=km",
        "New Load.ld2 bus1=b2 phases=3 kv=12.47 kw=400 pf=0.95 model=1",
        "Set voltagebases=[12.47]",
        "Calcvoltagebases",
    ]);

    let terms = dss.aggregate_terms();
    assert_eq!(terms.total_power, vec!["vsource.source", "vsource.s2"]);

    // Hand-sum terminal 1 of each source out of the per-conductor snapshot
    // (kW/kvar already): the first `nconds` entries of `powers`.
    let snaps = dss.snapshot_elements();
    let (mut p_kw, mut q_kvar) = (0.0, 0.0);
    for name in &terms.total_power {
        let (i, _) = find(&snaps, name);
        let s = &snaps[i];
        let nconds = s.powers.len() / s.bus_names.len();
        assert!(nconds >= 3, "{name}: {nconds} conductors");
        for p in &s.powers[..nconds] {
            p_kw += p.re;
            q_kvar += p.im;
        }
    }
    assert!(
        p_kw < -1000.0,
        "the sources must deliver megawatts (negative kW into terminal 1), got {p_kw}"
    );

    let (tp_re, tp_im) = dss.total_power();
    close(tp_re, p_kw, 1e-12, 1e-9, "TotalPower.re");
    close(tp_im, q_kvar, 1e-12, 1e-9, "TotalPower.im");

    // The x3 of `Get_Power`: flipping `CktModel` scales the *report*, not the
    // solution (the flag is read at read time), so the factor is exact.
    dss.command("Set CktModel=Positive");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let (tp3_re, tp3_im) = dss.total_power();
    close(tp3_re, 3.0 * tp_re, 1e-15, 0.0, "TotalPower.re x3");
    close(tp3_im, 3.0 * tp_im, 1e-15, 0.0, "TotalPower.im x3");
}

// Expected-value pin, Totaliterations is an alias: `Solution.Totaliterations`
// (`DDLL/DSolution.pas:218-220` — `SolutionI` mode 40) literally returns
// `Solution.Iteration`, and dss_capi's `CAPI_Solution.pas:731-738` says so in
// its own comment ("Same as Iterations interface"). The name says "total"; the
// value is the LAST step's inner iteration count, never an accumulation — the
// engine's real accumulator (`TotalIterations`, `Common/Solution.pas:2703`) is
// exposed by NEITHER oracle surface. That is why G1.9 carries this field as an
// in-engine alias pin instead of a second oracle comparison.
/// `Totaliterations` is `Iteration`: a multi-step run reports the last step's
/// count, not the sum over steps.
#[test]
fn total_iterations_is_an_alias_of_iterations() {
    let mut deck: Vec<&str> = FEEDER.to_vec();
    deck.push("New Loadshape.ls npts=3 interval=1 mult=[0.6, 1.0, 0.8]");
    deck.push("Edit Load.ld1 daily=ls");
    deck.push("Set mode=daily stepsize=1h");

    // Three single-step solves: the per-step inner iteration counts.
    let mut stepwise = build(&deck);
    stepwise.command("Set number=1");
    let mut per_step = Vec::new();
    let mut per_step_most = Vec::new();
    for _ in 0..3 {
        stepwise.command("Solve");
        assert!(stepwise.errors().is_empty(), "{:?}", stepwise.errors());
        let sol = &stepwise.circuit().unwrap().solution;
        per_step.push(sol.iteration);
        per_step_most.push(sol.most_iterations_done);
    }
    let sum: i32 = per_step.iter().sum();
    let max = *per_step.iter().max().unwrap();
    assert!(
        sum > max,
        "the pin needs a run whose steps sum above their max, got {per_step:?}"
    );

    // The same three steps in one `Solve`.
    let mut oneshot = build(&deck);
    oneshot.command("Set number=3");
    oneshot.command("Solve");
    assert!(oneshot.errors().is_empty(), "{:?}", oneshot.errors());
    let sol = &oneshot.circuit().unwrap().solution;

    assert_eq!(
        sol.iteration,
        *per_step.last().unwrap(),
        "Iteration (== Totaliterations) is the LAST step's count; per-step {per_step:?}"
    );
    assert!(
        sol.iteration < sum,
        "an accumulating Totaliterations would read {sum}, not {}",
        sol.iteration
    );
    // Its companion scalar `MostIterationsDone` is a max too — but only over the
    // CONTROL iterations of the current step: `SnapShotInit` zeroes it at the
    // top of every step (`Common/Solution.pas:2568`, ported at
    // `solution::solution::state::SolutionState::snap_shot_init`) and
    // `Common/Solution.pas:2701` raises it inside the control loop
    // (`solution::solution::power_flow`). So a multi-step run reports the LAST
    // step's max, discarding a larger earlier step.
    let last_most = *per_step_most.last().unwrap();
    assert!(
        *per_step_most.iter().max().unwrap() > last_most,
        "this pin needs an earlier step with MORE iterations than the last one,          else the per-step reset is untested; per-step {per_step_most:?}"
    );
    assert_eq!(
        sol.most_iterations_done, last_most,
        "MostIterationsDone is reset per step, so it reports the last step's max;          per-step {per_step_most:?}"
    );
}

// Expected-value pin, AllElementLosses order: `Circuit.AllElementLosses`
// walks `CktElements` (`DDLL/DCircuit.pas:466-473`, `CAPI_Circuit.pas:461-465`),
// i.e. creation order — positionally aligned with `AllElementNames`
// (`CAPI_Circuit.pas:275-279`), which the corpus gate matches against
// `snapshot_elements`. Nothing but this pin and the gate's ordered-name
// assertion protects that alignment.
/// `AllElementLosses` is one `x 0.001` entry per device, in creation order.
#[test]
fn all_element_losses_follow_creation_order() {
    let mut deck: Vec<&str> = FEEDER.to_vec();
    // Appended last, after `Calcvoltagebases`: it must land in the last slot.
    deck.push("New Load.zlast bus1=b1 phases=3 kv=12.47 kw=250 pf=0.9 model=1");
    let mut dss = solve(&deck);

    let snaps = dss.snapshot_elements();
    let ael = dss.all_element_losses();
    assert_eq!(ael.len(), snaps.len());
    assert_eq!(
        ael.len(),
        dss.circuit().unwrap().num_devices,
        "AllElementLosses has one slot per device"
    );
    assert!(
        snaps
            .last()
            .unwrap()
            .name
            .eq_ignore_ascii_case("Load.zlast"),
        "the element created last must be last: {:?}",
        snaps.last().unwrap().name
    );

    for (i, s) in snaps.iter().enumerate() {
        close(
            ael[i].0,
            s.loss_w.0 * 0.001,
            1e-12,
            1e-12,
            &format!("AllElementLosses[{i}].re ({})", s.name),
        );
        close(
            ael[i].1,
            s.loss_w.1 * 0.001,
            1e-12,
            1e-12,
            &format!("AllElementLosses[{i}].im ({})", s.name),
        );
    }

    // Teeth: every entry is distinguishable, so a swapped pair reds the loop
    // above instead of cancelling out.
    for i in 0..ael.len() {
        for j in (i + 1)..ael.len() {
            assert!(
                (ael[i].0 - ael[j].0).abs() > 1e-9 || (ael[i].1 - ael[j].1).abs() > 1e-9,
                "{} and {} report the same losses; the order assertion would not \
                 catch a swap of the two",
                snaps[i].name,
                snaps[j].name
            );
        }
    }
}

// Expected-value pin, both boolean `Solution` scalars are two-sided. The live
// gate compares `ControlActionsDone` and `SystemYChanged` with an exact
// `assert_eq!` on every case, but every corpus checkpoint measured so far
// reports the same value for both (`true` / `false` — a converged solve settles
// its controls and leaves Y freshly built), so those two asserts have no corpus
// witness of the other value. This pin supplies one in-engine for each: the
// max-control-iteration exit (`solve_snap`, Pascal `SolveSnap`) leaves
// `ControlActionsDone` clear, and a structural edit after a solve raises
// `SystemYChanged` again. Without it a port that could never produce the second
// value would pass the gate unnoticed.
#[test]
fn the_two_boolean_solution_flags_take_both_values() {
    // (1) A converged solve: controls settled, Y rebuilt and the flag cleared.
    let mut dss = solve(FEEDER);
    {
        let sol = &dss.circuit().unwrap().solution;
        assert!(
            sol.control_actions_done,
            "a converged, control-free solve must end with ControlActionsDone set"
        );
        assert!(
            !sol.system_y_changed,
            "a converged solve must leave Y built, i.e. SystemYChanged clear"
        );
    }

    // (2) A structural edit dirties Y again — the `true` witness.
    dss.command("New Load.ld2 bus1=b1 phases=3 kv=12.47 kw=100 pf=1 model=1");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert!(
        dss.circuit().unwrap().solution.system_y_changed,
        "adding an element must raise SystemYChanged"
    );

    // (3) A regulator that cannot settle inside `MaxControlIter` — the `false`
    // witness. `solve_snap` breaks on the iteration limit with the flag clear
    // and reports "Max Control Iterations Exceeded".
    let mut dss = build(&[
        "Clear",
        "New Circuit.g19reg basekv=12.47 phases=3 bus1=sourcebus mvasc3=200 mvasc1=210",
        "New Line.l1 bus1=sourcebus bus2=b1 phases=3 r1=0.30 x1=0.90 r0=0.9 x0=2.7 length=2 units=km",
        "New Transformer.tx1 phases=3 windings=2 buses=[b1, b2] conns=[wye wye] \
         kvs=[12.47 12.47] kvas=[5000 5000] xhl=1",
        "New RegControl.rc1 transformer=tx1 winding=2 vreg=126 band=1 ptratio=60 delay=0",
        "New Load.ld1 bus1=b2 phases=3 kv=12.47 kw=3000 pf=0.95 model=1",
        "Set voltagebases=[12.47]",
        "Calcvoltagebases",
        "Set maxcontroliter=1",
    ]);
    dss.command("Solve");
    let sol = &dss.circuit().unwrap().solution;
    assert_eq!(
        sol.control_iteration, 1,
        "the deck must stop at MaxControlIter=1 for this pin to mean anything"
    );
    assert!(
        !sol.control_actions_done,
        "a solve that hits MaxControlIter must leave ControlActionsDone clear"
    );
}
