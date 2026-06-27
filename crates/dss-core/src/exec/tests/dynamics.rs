//! Dynamics solve-mode integration tests (WP7.7 step 1): the `Solve mode=dynamic`
//! predictor/corrector driver (`SolveDynamic`) and the `OK_for_Dynamics` entry
//! guard. The per-element dynamics state machinery (Generator/Storage/PVSystem/
//! IndMach012) is WP7.7 steps 2–3; with only static elements present the base
//! `TPCElement.IntegrateStates`/`InitStateVars` are no-ops, so the driver must
//! hold the snapshot operating point across every step and sample the monitors
//! once per step (the focused oracle gate over a real dynamics machine is step 4).

use crate::exec::*;

/// A static circuit (VSource + line + constant-PQ load, no dynamics machines)
/// solved at fundamental, ready to enter dynamics mode.
fn static_dyn_dss() -> Dss {
    let mut dss = Dss::new();
    dss.command("New circuit.dyn basekv=12.47 pu=1.0 phases=3 mvasc3=20000 mvasc1=21000");
    dss.command("New Line.l1 bus1=sourcebus bus2=loadbus length=1 units=km r1=0.1 x1=0.3");
    dss.command("New Load.ld1 bus1=loadbus phases=3 kv=12.47 kw=1000 pf=0.95 model=1");
    dss.command("New Monitor.m element=Line.l1 terminal=1 mode=0");
    dss.command("Set voltagebases=[12.47]");
    dss.command("CalcVoltageBases");
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "fundamental: {:?}", dss.errors());
    dss
}

/// The lowest node-voltage magnitude (V) — on this VSource→line→load circuit the
/// loadbus phases, so it tracks the load operating point.
fn min_node_vmag(dss: &Dss) -> f64 {
    let ckt = dss.circuit().unwrap();
    (1..=ckt.num_nodes)
        .map(|i| ckt.solution.node_v[i].norm())
        .fold(f64::INFINITY, f64::min)
}

#[test]
fn dynamic_mode_holds_snapshot_and_samples_each_step() {
    let mut dss = static_dyn_dss();
    // Capture the converged snapshot node voltages.
    let v_snap: Vec<num_complex::Complex64> = dss.circuit().unwrap().solution.node_v.clone();

    // h defaults to 1 ms in dynamics mode; pin it (and the step count) explicitly.
    dss.command("Set mode=dynamic stepsize=0.001 number=5");
    assert!(
        dss.errors().is_empty(),
        "set mode=dynamic: {:?}",
        dss.errors()
    );
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "dynamic solve: {:?}", dss.errors());

    let ckt = dss.circuit().unwrap();
    assert!(ckt.is_solved);
    // IncrementTime runs before each step: 5 steps × 1 ms advances the clock 5 ms
    // into the hour.
    assert_eq!(ckt.solution.int_hour, 0);
    assert!(
        (ckt.solution.t - 0.005).abs() < 1e-12,
        "t = {}",
        ckt.solution.t
    );

    // With no dynamics machines, IntegratePCStates is a no-op and the
    // predictor/corrector power flow reconverges to the snapshot fixpoint every
    // step: node voltages are unchanged to solver precision (1 ppm).
    assert_eq!(ckt.solution.node_v.len(), v_snap.len());
    for (i, (a, b)) in ckt.solution.node_v.iter().zip(&v_snap).enumerate() {
        let denom = b.norm().max(1.0);
        assert!(
            (*a - *b).norm() / denom < 1e-6,
            "node {i} drifted in dynamics mode: {a} vs {b}"
        );
    }

    // Pascal `MonitorClass.SampleAll` runs once per dynamics step (the mode change
    // first reset the buffer in the `Set_Mode` tail).
    assert_eq!(
        dss.monitor_view("m").expect("m").sample_count,
        5,
        "one monitor sample per dynamics step"
    );
}

#[test]
fn dynamic_mode_load_multiplier_moves_operating_point() {
    // The new Load `GENERALTIME`/`DYNAMICMODE` `SetNominalLoad` arm scales the load
    // by `LoadMultiplier` (unless Exempt). Doubling it in dynamics mode must drive
    // the predictor/corrector loop to a *heavier* operating point — the loadbus
    // voltage drops materially below the snapshot. This (a) exercises the new
    // multiplier arm (the other tests run at loadmult = 1, where the multiply is a
    // no-op) and (b) proves the driver actually re-solves the power flow each step:
    // a driver that skipped `SolveSnap` would leave the snapshot voltages frozen.
    // The exact magnitude is oracle-pinned in step 4.
    let mut dss = static_dyn_dss();
    let v_snap = min_node_vmag(&dss);

    dss.command("Set mode=dynamic stepsize=0.001 number=3");
    dss.command("Set loadmult=2");
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "dynamic solve: {:?}", dss.errors());
    assert!(dss.circuit().unwrap().is_solved);

    let v_heavy = min_node_vmag(&dss);
    assert!(
        v_heavy < v_snap - 1.0,
        "doubling loadmult must drop the loadbus voltage (the loop re-solved): \
         snapshot {v_snap} V vs heavy {v_heavy} V"
    );
}

#[test]
fn dynamic_mode_requires_a_solved_circuit() {
    // Pascal `OK_for_Dynamics` (Solution.pas l.2188): entering a dynamics mode
    // from an unsolved circuit is refused (error 486) — the machine-state init
    // would have nothing valid to capture. (No `Solve`/`CalcVoltageBases`, so
    // `IsSolved` stays false.)
    let mut dss = Dss::new();
    dss.command("New circuit.dyn basekv=12.47 pu=1.0 phases=3 mvasc3=20000");
    dss.command("New Line.l1 bus1=sourcebus bus2=b2 length=1 units=km r1=0.1 x1=0.3");
    dss.command("Set mode=dynamic");
    let errs = dss.errors().join("\n");
    assert!(
        errs.contains("must be solved in a non-dynamic mode"),
        "expected the OK_for_Dynamics guard error, got: {errs:?}"
    );
    // The refused entry leaves the mode unchanged (snapshot) — the dynamics state
    // init never ran.
    assert!(!dss.circuit().unwrap().solution.is_dynamic_model);
}
